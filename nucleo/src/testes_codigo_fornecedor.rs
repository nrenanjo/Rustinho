//! Código nos fornecedores editável (pedido do proprietário, 16/11/2026): o código que cada fornecedor usa para a
//! peça é editado no próprio campo "Código nos fornecedores" da ficha, continua ligado ao mesmo fornecedor (e ao
//! mesmo fator), e nunca vai para o "Código original / fabricante" nem vem dele: um campo não passa nada ao outro.
//! Confere o salvamento, a edição posterior e a recuperação (abrir a peça de novo), a troca de dados entre caixas
//! (o código velho sai dos outros caixas), a busca e as recusas (nada muda quando recusa).
//!
//! Usa só a interface do serviço (a mesma antes e depois da correção): falha antes e passa depois.

use serde_json::{json, Value};

use crate::catalogo::codigos::{vincular_fornecedor, JaLigado};

fn chamar(s: &mut crate::Servico, cmd: &str, d: Value) -> Value {
    serde_json::from_str(&crate::apoio_chave::chamar(crate::api::NAO_REPETIR, cmd, &d.to_string(), |x| {
        s.chamar(cmd, x)
    }))
    .unwrap()
}

fn ok(s: &mut crate::Servico, cmd: &str, d: Value) -> Value {
    let r = chamar(s, cmd, d);
    assert_eq!(r["ok"], true, "{cmd}: {r}");
    r["dados"].clone()
}

struct Loja {
    s: crate::Servico,
    peca: String,
    outra: String,
    patral: String,
    ricartti: String,
}

/// A peça do pedido (com o código original do fabricante preenchido), dois fornecedores com o código de cada um
/// (o da Ricartti com caixa de 6) e outra peça com um código da Patral.
fn loja(original: &str) -> Loja {
    let mut s = crate::Servico::em_memoria().unwrap();
    ok(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "12345678", "senha2": "12345678" }),
    );
    let mut f = ok(&mut s, "produto_novo", json!({}));
    f["nome"] = json!("Conector macho 12x08");
    f["custo"] = json!("10");
    f["codigoOriginal"] = json!(original);
    let peca = ok(&mut s, "produto_salvar", f)["id"].as_str().unwrap().to_string();
    let mut f = ok(&mut s, "produto_novo", json!({}));
    f["nome"] = json!("Abraçadeira");
    f["custo"] = json!("10");
    let outra = ok(&mut s, "produto_salvar", f)["id"].as_str().unwrap().to_string();
    let mut fornecedor = |nome: &str| {
        let mut f = ok(&mut s, "pessoa_nova", json!({ "papel": "fornecedor" }));
        f["nome"] = json!(nome);
        ok(&mut s, "pessoa_salvar", f)["id"].as_str().unwrap().to_string()
    };
    let patral = fornecedor("Patral Peças");
    let ricartti = fornecedor("Di Ricartti Distribuidora");
    {
        let c = &s.banco_para_teste().conn;
        assert!(vincular_fornecedor(c, &patral, "00008373", &peca, 1000, JaLigado::Manter).unwrap());
        assert!(vincular_fornecedor(c, &ricartti, "25850-0", &peca, 6000, JaLigado::Manter).unwrap());
        assert!(vincular_fornecedor(c, &patral, "777", &outra, 1000, JaLigado::Manter).unwrap());
    }
    Loja { s, peca, outra, patral, ricartti }
}

impl Loja {
    fn abrir(&mut self) -> Value {
        ok(&mut self.s, "produto_abrir", json!({ "id": self.peca }))
    }

    /// O que a ficha mostra para o fornecedor: (código, fator), pelo id dele.
    fn na_ficha(ficha: &Value, fornecedor: &str) -> Vec<(String, String)> {
        ficha["fornecedores"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|x| x["fornecedorId"] == fornecedor)
            .map(|x| (x["codigo"].as_str().unwrap().to_string(), x["fator"].as_str().unwrap().to_string()))
            .collect()
    }

    /// A ficha como a tela a manda de volta, com as linhas de "Código nos fornecedores" como estão na tela
    /// (`trocas`: (fornecedor, código de antes) → código novo; as outras linhas vão como estão).
    fn salvar(&mut self, ficha: &Value, trocas: &[(&str, &str, &str)]) -> Value {
        let mut f = ficha.clone();
        let linhas: Vec<Value> = ficha["fornecedores"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| {
                let antes = x["codigo"].as_str().unwrap();
                let novo = trocas
                    .iter()
                    .find(|(forn, a, _)| x["fornecedorId"] == *forn && *a == antes)
                    .map(|t| t.2)
                    .unwrap_or(antes);
                json!({ "fornecedorId": x["fornecedorId"], "antes": antes, "codigo": novo })
            })
            .collect();
        f["codigosFornecedor"] = json!(linhas);
        chamar(&mut self.s, "produto_salvar", f)
    }

    /// As ligações gravadas: (fornecedor, código, compacto, peça, fator), em ordem.
    fn ligacoes(&mut self) -> Vec<(String, String, String, String, i64)> {
        let c = &self.s.banco_para_teste().conn;
        let mut q = c
            .prepare(
                "SELECT fornecedor_id, codigo, codigo_compacto, produto_id, fator FROM produto_fornecedor
                  ORDER BY fornecedor_id, codigo_compacto",
            )
            .unwrap();
        q.query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    fn original_gravado(&mut self) -> String {
        let id = self.peca.clone();
        self.s
            .banco_para_teste()
            .conn
            .query_row("SELECT codigo_original FROM produto WHERE id = ?1", [&id], |l| l.get(0))
            .unwrap()
    }

    fn ultima_alteracao(&mut self) -> i64 {
        self.s
            .banco_para_teste()
            .conn
            .query_row("SELECT COALESCE(MAX(seq), 0) FROM alteracao", [], |l| l.get(0))
            .unwrap()
    }

    /// As alterações da troca de dados de `produto_fornecedor` depois de `seq`: (operação, chave).
    fn alteracoes_depois(&mut self, seq: i64) -> Vec<(String, String)> {
        let c = &self.s.banco_para_teste().conn;
        let mut q = c
            .prepare(
                "SELECT operacao, chave FROM alteracao WHERE tabela = 'produto_fornecedor' AND seq > ?1 ORDER BY seq",
            )
            .unwrap();
        q.query_map([seq], |l| Ok((l.get(0)?, l.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap()
    }

    fn acha(&mut self, texto: &str) -> bool {
        let r = ok(&mut self.s, "produtos_buscar", json!({ "texto": texto }));
        r["itens"].as_array().unwrap().iter().any(|x| x["id"] == self.peca)
    }
}

/// 1, 2 e 5 do pedido: editar no próprio campo, salvar, abrir de novo, editar outra vez. O código fica com o mesmo
/// fornecedor e o mesmo fator, com os zeros à esquerda, e o "Código original / fabricante" não muda.
#[test]
fn codigo_do_fornecedor_editado_salvo_e_recuperado() {
    let mut l = loja("FAB-123");
    let (patral, ricartti) = (l.patral.clone(), l.ricartti.clone());
    let ficha = l.abrir();
    assert_eq!(Loja::na_ficha(&ficha, &patral), vec![("00008373".into(), "1".into())], "a ficha diz o fornecedor");
    assert_eq!(Loja::na_ficha(&ficha, &ricartti), vec![("25850-0".into(), "6".into())]);

    // Só o código do fornecedor muda (nenhum outro campo): grava assim mesmo.
    let r = l.salvar(&ficha, &[(&patral, "00008373", "00008374")]);
    assert_eq!(r["ok"], true, "{r}");
    let depois = &r["dados"];
    assert_eq!(Loja::na_ficha(depois, &patral), vec![("00008374".into(), "1".into())]);
    assert_eq!(Loja::na_ficha(depois, &ricartti), vec![("25850-0".into(), "6".into())], "o outro fornecedor fica");
    assert_eq!(depois["codigoOriginal"], "FAB-123", "o código original não muda");

    // Recuperação: abrir a peça de novo.
    let aberta = l.abrir();
    assert_eq!(Loja::na_ficha(&aberta, &patral), vec![("00008374".into(), "1".into())]);
    assert_eq!(aberta["codigoOriginal"], "FAB-123");
    assert_eq!(aberta["equivalentes"].as_array().unwrap().len(), 0, "o código não vira equivalência");
    let peca = l.peca.clone();
    let outra = l.outra.clone();
    assert_eq!(
        l.ligacoes().into_iter().filter(|x| x.0 == patral).collect::<Vec<_>>(),
        vec![
            (patral.clone(), "00008374".into(), "00008374".into(), peca.clone(), 1000),
            (patral.clone(), "777".into(), "777".into(), outra.clone(), 1000),
        ],
        "uma ligação só da Patral com esta peça, com o código novo e o mesmo fator"
    );
    assert_eq!(l.original_gravado(), "FAB-123");
    let historico = aberta["historico"].to_string();
    assert!(historico.contains("00008373 → 00008374"), "o histórico diz a troca: {historico}");
    assert!(historico.contains("Patral Peças"), "o histórico diz o fornecedor: {historico}");

    // Edição posterior: o código novo e, no mesmo salvar, o do outro fornecedor só na forma (mesmo compacto).
    let r = l.salvar(&aberta, &[(&patral, "00008374", "PT-0008375"), (&ricartti, "25850-0", "25850 0")]);
    assert_eq!(r["ok"], true, "{r}");
    let aberta = l.abrir();
    assert_eq!(Loja::na_ficha(&aberta, &patral), vec![("PT-0008375".into(), "1".into())]);
    assert_eq!(Loja::na_ficha(&aberta, &ricartti), vec![("25850 0".into(), "6".into())]);
    assert_eq!(aberta["codigoOriginal"], "FAB-123");
    assert_eq!(l.original_gravado(), "FAB-123");
    assert_eq!(l.ligacoes().len(), 3, "nenhuma ligação a mais nem a menos");
}

/// 3 e 4 do pedido: com o código original vazio (a tela do pedido), editar o do fornecedor não preenche o original;
/// mudar o original não mexe no do fornecedor.
#[test]
fn codigo_original_e_codigo_do_fornecedor_independentes() {
    let mut l = loja("");
    let patral = l.patral.clone();
    let ficha = l.abrir();
    let r = l.salvar(&ficha, &[(&patral, "00008373", "00008380")]);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(r["dados"]["codigoOriginal"], "", "o original continua vazio");
    assert_eq!(l.original_gravado(), "");

    // Só o original muda; as linhas dos fornecedores vão como estão.
    let mut ficha = l.abrir();
    ficha["codigoOriginal"] = json!("FAB-9");
    let r = l.salvar(&ficha, &[]);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(l.original_gravado(), "FAB-9");
    assert_eq!(Loja::na_ficha(&l.abrir(), &patral), vec![("00008380".into(), "1".into())]);

    // Os dois no mesmo salvar: cada um no seu lugar.
    let mut ficha = l.abrir();
    ficha["codigoOriginal"] = json!("FAB-10");
    let r = l.salvar(&ficha, &[(&patral, "00008380", "00008381")]);
    assert_eq!(r["ok"], true, "{r}");
    let aberta = l.abrir();
    assert_eq!(aberta["codigoOriginal"], "FAB-10");
    assert_eq!(Loja::na_ficha(&aberta, &patral), vec![("00008381".into(), "1".into())]);
}

/// Os outros caixas: a troca de dados leva o "apagar" do código velho e o "gravar" do novo (o UPDATE da chave só
/// levaria o novo, e o velho ficaria nos outros caixas). Nada muda: nada é gravado.
#[test]
fn troca_do_codigo_vai_para_os_outros_caixas() {
    let mut l = loja("FAB-123");
    let (patral, ricartti) = (l.patral.clone(), l.ricartti.clone());
    let ficha = l.abrir();

    let seq = l.ultima_alteracao();
    let r = l.salvar(&ficha, &[]);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(l.ultima_alteracao(), seq, "sem mudança, nada vai para a troca de dados");

    let r = l.salvar(&ficha, &[(&patral, "00008373", "00008374")]);
    assert_eq!(r["ok"], true, "{r}");
    let alt = l.alteracoes_depois(seq);
    assert_eq!(alt.len(), 2, "{alt:?}");
    assert_eq!(alt[0].0, "apagar");
    assert!(alt[0].1.contains(&patral) && alt[0].1.contains("\"00008373\""), "{alt:?}");
    assert_eq!(alt[1].0, "gravar");
    assert!(alt[1].1.contains(&patral) && alt[1].1.contains("\"00008374\""), "{alt:?}");

    // Só a forma (o mesmo compacto): um "gravar" da mesma chave.
    let seq = l.ultima_alteracao();
    let f = l.abrir();
    let r = l.salvar(&f, &[(&ricartti, "25850-0", "25850.0")]);
    assert_eq!(r["ok"], true, "{r}");
    let alt = l.alteracoes_depois(seq);
    assert_eq!(alt.len(), 1, "{alt:?}");
    assert_eq!(alt[0].0, "gravar");
    assert!(alt[0].1.contains(&ricartti) && alt[0].1.contains("\"258500\""), "{alt:?}");
}

/// A busca acha a peça pelo código novo e não mais pelo velho.
#[test]
fn busca_pelo_codigo_novo() {
    let mut l = loja("FAB-123");
    let patral = l.patral.clone();
    assert!(l.acha("00008373"));
    let ficha = l.abrir();
    let r = l.salvar(&ficha, &[(&patral, "00008373", "00009999")]);
    assert_eq!(r["ok"], true, "{r}");
    assert!(l.acha("00009999"), "acha pelo código novo");
    assert!(!l.acha("00008373"), "não acha mais pelo velho");
}

/// Recusas: vazio, de outra peça do mesmo fornecedor, repetido nesta peça, a ligação que não está mais na peça e
/// comprido demais. Cada uma diz o porquê, aponta a linha e não grava nada (nem os outros campos).
#[test]
fn recusas_nao_gravam_nada() {
    let mut l = loja("FAB-123");
    let (patral, ricartti) = (l.patral.clone(), l.ricartti.clone());
    // Mais um código da Patral nesta peça (a peça pode ter dois do mesmo fornecedor).
    {
        let peca = l.peca.clone();
        let c = &l.s.banco_para_teste().conn;
        assert!(vincular_fornecedor(c, &patral, "00008399", &peca, 1000, JaLigado::Manter).unwrap());
    }
    let antes = l.ligacoes();
    let seq = l.ultima_alteracao();
    let mut ficha = l.abrir();
    ficha["nome"] = json!("Nome que não pode ser gravado");
    let linha_patral = ficha["fornecedores"]
        .as_array()
        .unwrap()
        .iter()
        .position(|x| x["fornecedorId"] == patral && x["codigo"] == "00008373")
        .unwrap();

    let comprido = "9".repeat(61);
    let casos: Vec<(Vec<(&str, &str, &str)>, &str)> = vec![
        (vec![(&patral, "00008373", "   ")], "vazio"),
        (vec![(&patral, "00008373", "77-7")], "Abraçadeira"),
        (vec![(&patral, "00008373", "0000-8399")], "já tem"),
        (vec![(&patral, "00008373", "00008399")], "já tem"),
        (vec![(&ricartti, "25850-0", &comprido)], "60"),
    ];
    for (trocas, motivo) in &casos {
        let r = l.salvar(&ficha, trocas);
        assert_eq!(r["ok"], false, "{trocas:?}: {r}");
        assert!(r["erro"].as_str().unwrap().contains(motivo), "{trocas:?}: {r}");
        assert!(r["campo"].as_str().unwrap().starts_with("codigosFornecedor."), "{trocas:?}: {r}");
    }
    let r = l.salvar(&ficha, &[(&patral, "00008373", "")]);
    assert_eq!(r["campo"], format!("codigosFornecedor.{linha_patral}"), "aponta a linha: {r}");

    // Dois códigos desta peça trocados para o mesmo número no mesmo salvar.
    let r = l.salvar(&ficha, &[(&patral, "00008373", "123"), (&patral, "00008399", "1-23")]);
    assert_eq!(r["ok"], false, "{r}");

    // A ligação saiu da peça (outro caixa passou o código para outra peça) com a tela ainda aberta.
    {
        let outra = l.outra.clone();
        let c = &l.s.banco_para_teste().conn;
        c.execute("UPDATE produto_fornecedor SET produto_id = ?1 WHERE codigo_compacto = '258500'", [&outra]).unwrap();
    }
    let r = l.salvar(&ficha, &[(&ricartti, "25850-0", "25851-0")]);
    assert_eq!(r["ok"], false, "{r}");
    assert!(r["erro"].as_str().unwrap().contains("não está mais"), "{r}");
    {
        let peca = l.peca.clone();
        let c = &l.s.banco_para_teste().conn;
        c.execute("UPDATE produto_fornecedor SET produto_id = ?1 WHERE codigo_compacto = '258500'", [&peca]).unwrap();
    }

    assert_eq!(l.ligacoes(), antes, "nenhuma ligação mudou");
    assert_eq!(l.original_gravado(), "FAB-123");
    let nome = l.abrir()["nome"].as_str().unwrap().to_string();
    assert_eq!(nome, "Conector macho 12x08", "o resto da ficha também não foi gravado");
    let alt = l.alteracoes_depois(seq);
    assert!(alt.iter().all(|(op, _)| op == "gravar"), "só as duas mudanças feitas à mão neste teste: {alt:?}");
    assert_eq!(alt.len(), 2, "{alt:?}");
}

/// Entra como outra pessoa (sai antes).
fn entrar(s: &mut crate::Servico, nome: &str, senha: &str) {
    ok(s, "sair", json!({}));
    let lista = ok(s, "usuarios_entrada", json!({}));
    let id = lista.as_array().unwrap().iter().find(|u| u["nome"] == nome).unwrap()["id"].as_str().unwrap().to_string();
    ok(s, "entrar", json!({ "id": id, "senha": senha }));
}

impl Loja {
    /// A data gravada na ligação (fornecedor, forma compacta do código).
    fn data_da_ligacao(&mut self, fornecedor: &str, compacto: &str) -> String {
        self.s
            .banco_para_teste()
            .conn
            .query_row(
                "SELECT atualizado_em FROM produto_fornecedor WHERE fornecedor_id = ?1 AND codigo_compacto = ?2",
                [fornecedor, compacto],
                |l| l.get(0),
            )
            .unwrap()
    }
}

/// Decisão CF-1 do proprietário (16/11/2026): o caixa (não gerente) edita o código do fornecedor, como o gerente.
/// Não é função só do gerente: corrigir o código não pede a senha dele (só o preço pede, D-09).
#[test]
fn o_caixa_e_o_gerente_editam_o_codigo_do_fornecedor() {
    let mut l = loja("FAB-123");
    let (patral, ricartti) = (l.patral.clone(), l.ricartti.clone());
    ok(
        &mut l.s,
        "usuario_salvar",
        json!({ "id": "", "nome": "Bia", "senha": "1111", "senha2": "1111", "gerente": false, "ativo": true }),
    );

    // O caixa: grava, sem o gerente, e a peça aberta de novo mostra o código novo.
    entrar(&mut l.s, "Bia", "1111");
    let ficha = l.abrir();
    let r = l.salvar(&ficha, &[(&patral, "00008373", "00008374")]);
    assert_eq!(r["ok"], true, "o caixa grava o código do fornecedor sem o gerente: {r}");
    let aberta = l.abrir();
    assert_eq!(Loja::na_ficha(&aberta, &patral), vec![("00008374".into(), "1".into())]);
    assert_eq!(aberta["codigoOriginal"], "FAB-123", "o código original não muda");
    // A edição posterior, também pelo caixa (só a forma do código).
    let r = l.salvar(&aberta, &[(&ricartti, "25850-0", "25850 0")]);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(Loja::na_ficha(&l.abrir(), &ricartti), vec![("25850 0".into(), "6".into())]);

    // O gerente também.
    entrar(&mut l.s, "Ana", "12345678");
    let ficha = l.abrir();
    let r = l.salvar(&ficha, &[(&patral, "00008374", "PT-0008375")]);
    assert_eq!(r["ok"], true, "o gerente grava o código do fornecedor: {r}");
    assert_eq!(Loja::na_ficha(&l.abrir(), &patral), vec![("PT-0008375".into(), "1".into())]);
    assert_eq!(l.original_gravado(), "FAB-123");
}

/// Decisão CF-2 do proprietário (16/11/2026): corrigir o código do fornecedor não muda a data da ligação nem a
/// reinicia. Vale nos dois jeitos de trocar (outra forma compacta: apaga a ligação velha e grava a nova; a mesma
/// forma: só o texto), na edição posterior e no que vai aos outros caixas.
#[test]
fn corrigir_o_codigo_nao_muda_a_data_da_ligacao() {
    let mut l = loja("FAB-123");
    let (patral, ricartti) = (l.patral.clone(), l.ricartti.clone());
    // Datas antigas e diferentes de hoje, para a troca não coincidir com a data de agora por acaso.
    let (data_patral, data_ricartti) = ("2025-03-04T05:06:07.000Z", "2024-12-01T08:00:00.000Z");
    {
        let c = &l.s.banco_para_teste().conn;
        c.execute(
            "UPDATE produto_fornecedor SET atualizado_em = ?1 WHERE fornecedor_id = ?2 AND codigo_compacto = '00008373'",
            [data_patral, patral.as_str()],
        )
        .unwrap();
        c.execute(
            "UPDATE produto_fornecedor SET atualizado_em = ?1 WHERE fornecedor_id = ?2 AND codigo_compacto = '258500'",
            [data_ricartti, ricartti.as_str()],
        )
        .unwrap();
    }
    let seq = l.ultima_alteracao();
    let ficha = l.abrir();
    // Outra forma compacta (Patral) e a mesma forma (Ricartti), no mesmo salvar.
    let r = l.salvar(&ficha, &[(&patral, "00008373", "00008374"), (&ricartti, "25850-0", "25850 0")]);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(l.data_da_ligacao(&patral, "00008374"), data_patral, "outra forma: a data fica a de antes");
    assert_eq!(l.data_da_ligacao(&ricartti, "258500"), data_ricartti, "a mesma forma: a data fica a de antes");

    // O que vai aos outros caixas: a ligação nova leva a mesma data.
    let dados: Vec<(String, String)> = {
        let c = &l.s.banco_para_teste().conn;
        let mut q = c
            .prepare(
                "SELECT chave, COALESCE(dados, '') FROM alteracao
                  WHERE tabela = 'produto_fornecedor' AND operacao = 'gravar' AND seq > ?1 ORDER BY seq",
            )
            .unwrap();
        q.query_map([seq], |x| Ok((x.get(0)?, x.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap()
    };
    let da_patral =
        dados.iter().find(|(chave, _)| chave.contains("00008374")).expect("a ligação nova vai aos outros caixas");
    assert!(da_patral.1.contains(data_patral), "a data que vai aos outros caixas é a de antes: {da_patral:?}");
    let da_ricartti =
        dados.iter().find(|(chave, _)| chave.contains("258500")).expect("a troca de forma vai aos outros caixas");
    assert!(da_ricartti.1.contains(data_ricartti), "{da_ricartti:?}");
    assert!(dados.iter().all(|(_, d)| !d.is_empty()), "{dados:?}");

    // Edição posterior: a data continua a primeira.
    let aberta = l.abrir();
    let r = l.salvar(&aberta, &[(&patral, "00008374", "PT-0008375")]);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(l.data_da_ligacao(&patral, "pt0008375"), data_patral, "a edição posterior também não muda a data");
    assert_eq!(l.data_da_ligacao(&ricartti, "258500"), data_ricartti, "a linha que não mudou fica igual");
}

// ---- Revisão do Fable (16/11/2026, RELATORIO_REVISAO_FORNEC.md): B-1, B-4, B-6, O-3 e O-4 ----

impl Loja {
    /// Liga mais um código de fornecedor a esta peça (fator em milésimos).
    fn ligar(&mut self, fornecedor: &str, codigo: &str, fator: i64) {
        let peca = self.peca.clone();
        let c = &self.s.banco_para_teste().conn;
        assert!(vincular_fornecedor(c, fornecedor, codigo, &peca, fator, JaLigado::Manter).unwrap());
    }

    /// O índice da linha de "Código nos fornecedores" na ficha (fornecedor, código).
    fn linha(ficha: &Value, fornecedor: &str, codigo: &str) -> usize {
        ficha["fornecedores"]
            .as_array()
            .unwrap()
            .iter()
            .position(|x| x["fornecedorId"] == fornecedor && x["codigo"] == codigo)
            .unwrap()
    }

    /// A ficha com linhas escritas à mão (o que a tela não manda), para as recusas.
    fn salvar_linhas(&mut self, ficha: &Value, linhas: Value) -> Value {
        let mut f = ficha.clone();
        f["codigosFornecedor"] = linhas;
        chamar(&mut self.s, "produto_salvar", f)
    }
}

/// B-1: trocar por outra forma compacta o código da ligação de caixa com 6: o fator fica (no banco, na ficha e no
/// que vai aos outros caixas).
#[test]
fn troca_de_outra_forma_mantem_o_fator() {
    let mut l = loja("FAB-123");
    let ricartti = l.ricartti.clone();
    let peca = l.peca.clone();
    let seq = l.ultima_alteracao();
    let ficha = l.abrir();
    let r = l.salvar(&ficha, &[(&ricartti, "25850-0", "RC-77")]);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(Loja::na_ficha(&l.abrir(), &ricartti), vec![("RC-77".into(), "6".into())], "a ficha diz caixa com 6");
    assert!(
        l.ligacoes().contains(&(ricartti.clone(), "RC-77".into(), "rc77".into(), peca, 6000)),
        "a ligação nova com o fator 6000: {:?}",
        l.ligacoes()
    );
    let dados: String =
        l.s.banco_para_teste()
            .conn
            .query_row(
                "SELECT dados FROM alteracao WHERE tabela = 'produto_fornecedor' AND operacao = 'gravar' AND seq > ?1
              AND chave LIKE '%rc77%'",
                [seq],
                |x| x.get(0),
            )
            .unwrap();
    assert!(dados.contains("\"fator\":6000"), "o fator vai aos outros caixas: {dados}");
}

/// B-1: a recusa de duas linhas com o mesmo código novo diz o motivo e aponta a segunda linha.
#[test]
fn recusa_duas_linhas_diz_o_motivo_e_a_linha() {
    let mut l = loja("FAB-123");
    let patral = l.patral.clone();
    l.ligar(&patral, "222", 1000);
    let antes = l.ligacoes();
    let ficha = l.abrir();
    let (a, b) = (Loja::linha(&ficha, &patral, "00008373"), Loja::linha(&ficha, &patral, "222"));
    let r = l.salvar(&ficha, &[(&patral, "00008373", "999"), (&patral, "222", "9-99")]);
    assert_eq!(r["ok"], false, "{r}");
    assert!(r["erro"].as_str().unwrap().contains("está em duas linhas"), "{r}");
    assert_eq!(r["campo"], format!("codigosFornecedor.{}", a.max(b)), "a segunda linha: {r}");
    assert_eq!(l.ligacoes(), antes, "nada gravado");
}

/// B-4: trocar dois códigos entre si no mesmo salvar é recusado com o motivo certo (uma troca de cada vez), sem
/// gravar nada; em salvares separados, dá certo.
#[test]
fn trocar_dois_codigos_entre_si_pede_uma_troca_de_cada_vez() {
    let mut l = loja("FAB-123");
    let patral = l.patral.clone();
    l.ligar(&patral, "222", 1000);
    let antes = l.ligacoes();
    let ficha = l.abrir();
    let r = l.salvar(&ficha, &[(&patral, "00008373", "222"), (&patral, "222", "00008373")]);
    assert_eq!(r["ok"], false, "{r}");
    assert!(r["erro"].as_str().unwrap().contains("Salve uma troca de cada vez"), "{r}");
    assert_eq!(l.ligacoes(), antes, "nada gravado");
    // Uma de cada vez, com um código de passagem.
    for (de, para) in [("00008373", "TMP-1"), ("222", "00008373"), ("TMP-1", "222")] {
        let f = l.abrir();
        let r = l.salvar(&f, &[(&patral, de, para)]);
        assert_eq!(r["ok"], true, "{de} → {para}: {r}");
    }
    let f = l.abrir();
    let mut codigos: Vec<String> = Loja::na_ficha(&f, &patral).into_iter().map(|x| x.0).collect();
    codigos.sort();
    assert_eq!(codigos, vec!["00008373".to_string(), "222".to_string()]);
}

/// B-6: um código gravado com mais de 60 caracteres (o campo da tela corta em 60) não é cortado em silêncio quando
/// a peça é salva por outro motivo.
#[test]
fn codigo_gravado_com_mais_de_60_nao_e_cortado() {
    let mut l = loja("FAB-123");
    let patral = l.patral.clone();
    let comprido: String = "ABC".repeat(24); // 72 caracteres
    l.ligar(&patral, &comprido, 1000);
    let mut ficha = l.abrir();
    ficha["codigoOriginal"] = json!("FAB-124");
    // A tela manda a linha como o campo a mostra: os 60 primeiros.
    let cortado: String = comprido.chars().take(60).collect();
    let r = l.salvar(&ficha, &[(&patral, &comprido, &cortado)]);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(l.original_gravado(), "FAB-124", "o resto da ficha grava");
    assert!(
        l.ligacoes().iter().any(|x| x.1 == comprido),
        "o código de 72 caracteres continua inteiro: {:?}",
        l.ligacoes()
    );
    assert!(!l.ligacoes().iter().any(|x| x.1 == cortado), "nenhuma ligação cortada");
}

/// O-3: um código só de sinais é recusado com o motivo certo (não "vazio"), sem gravar nada.
#[test]
fn so_sinais_tem_motivo_proprio() {
    let mut l = loja("FAB-123");
    let patral = l.patral.clone();
    let antes = l.ligacoes();
    let ficha = l.abrir();
    let r = l.salvar(&ficha, &[(&patral, "00008373", "-/.")]);
    assert_eq!(r["ok"], false, "{r}");
    assert!(r["erro"].as_str().unwrap().contains("precisa ter letras ou números"), "{r}");
    assert_eq!(l.ligacoes(), antes, "nada gravado");
}

/// O-3: com um fornecedor que não está no cadastro, a frase fica certa ("de um fornecedor sem cadastro").
#[test]
fn fornecedor_sem_cadastro_tem_frase_certa() {
    let mut l = loja("FAB-123");
    let ficha = l.abrir();
    let r = l.salvar_linhas(&ficha, json!([{ "fornecedorId": "nao-existe", "antes": "1", "codigo": "2" }]));
    assert_eq!(r["ok"], false, "{r}");
    let erro = r["erro"].as_str().unwrap();
    assert!(erro.contains("de um fornecedor sem cadastro") && !erro.contains("de o fornecedor"), "{r}");
}

/// O-4: a mesma ligação em duas linhas, com códigos novos diferentes (a tela não manda assim): recusa com texto, e
/// não o erro do banco; nada gravado.
#[test]
fn mesma_ligacao_em_duas_linhas_recusa_com_texto() {
    let mut l = loja("FAB-123");
    let patral = l.patral.clone();
    let antes = l.ligacoes();
    let ficha = l.abrir();
    let r = l.salvar_linhas(
        &ficha,
        json!([
            { "fornecedorId": patral, "antes": "00008373", "codigo": "111" },
            { "fornecedorId": patral, "antes": "00008373", "codigo": "112" }
        ]),
    );
    assert_eq!(r["ok"], false, "{r}");
    assert!(r["erro"].as_str().unwrap().contains("veio em duas linhas"), "{r}");
    assert_eq!(r["campo"], "codigosFornecedor.1", "{r}");
    assert_eq!(l.ligacoes(), antes, "nada gravado");
}

/// O-4: o código novo já é de uma peça que ainda não chegou neste caixa (a ligação chegou antes dela): recusa com
/// texto ("já é de outra peça"), e não o erro do banco; nada gravado.
#[test]
fn codigo_de_peca_que_nao_chegou_recusa_com_texto() {
    let mut l = loja("FAB-123");
    let patral = l.patral.clone();
    {
        let c = &l.s.banco_para_teste().conn;
        c.execute_batch("PRAGMA foreign_keys = OFF").unwrap();
        c.execute(
            "INSERT INTO produto_fornecedor (fornecedor_id, codigo_compacto, codigo, produto_id, fator, atualizado_em)
             VALUES (?1, '555', '555', 'peca-que-nao-chegou', 1000, '2025-01-01T00:00:00Z')",
            [&patral],
        )
        .unwrap();
        c.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        // T106 (a CI 44930016320 do C5): a ficha lista os códigos do mais recente para o mais antigo. Os dois
        // vínculos da peça nascem um logo depois do outro: no mesmo segundo, a Patral vinha primeiro; com a virada do
        // segundo no meio, a Di Ricartti. O teste supunha a Patral na linha 0 e falhava por acaso. Agora a Di Ricartti
        // é a mais recente de propósito, e a linha da Patral vem da própria ficha.
        c.execute("UPDATE produto_fornecedor SET atualizado_em = '2099-01-01T00:00:00Z' WHERE codigo = '25850-0'", [])
            .unwrap();
    }
    let antes = l.ligacoes();
    let ficha = l.abrir();
    let linha_patral = ficha["fornecedores"]
        .as_array()
        .unwrap()
        .iter()
        .position(|x| x["fornecedorId"] == patral.as_str() && x["codigo"] == "00008373")
        .unwrap();
    assert_eq!(linha_patral, 1, "a Di Ricartti, a mais recente, vem primeiro");
    let r = l.salvar(&ficha, &[(&patral, "00008373", "555")]);
    assert_eq!(r["ok"], false, "{r}");
    assert!(r["erro"].as_str().unwrap().contains("já é de outra peça"), "{r}");
    assert_eq!(r["campo"], format!("codigosFornecedor.{linha_patral}"), "{r}");
    assert_eq!(l.ligacoes(), antes, "nada gravado");
}
