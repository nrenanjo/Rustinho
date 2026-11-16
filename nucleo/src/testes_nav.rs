//! Navegação contextual guiada (piloto: Peças › Importar XML › Configurações ›
//! Loja › volta a Peças). O que o núcleo garante:
//!
//! - a falta do CNPJ da loja na importação vem com o requisito que falta
//!   (`campo` = "requisito:loja.cnpj"), para a tela levar ao lugar certo sem
//!   depender do texto da mensagem;
//! - `requisito_conferir` diz, pelo banco, se o requisito foi satisfeito
//!   (CNPJ salvo e válido pela mesma regra de `config_salvar`); a tela só
//!   volta à origem quando ele diz que sim;
//! - `arquivos_conferir` diz quais arquivos escolhidos continuam no lugar,
//!   para a importação seguir sem perder as outras escolhas.
//!
//! Nada disso grava: os três comandos estão em `SO_LEITURA`.

use serde_json::{json, Value};

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

/// Loja nova sem CNPJ (gerente Ana conectada), e uma pasta com um XML.
fn loja() -> (crate::Servico, std::path::PathBuf) {
    let mut s = crate::Servico::em_memoria().unwrap();
    ok(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
    );
    let pasta = std::env::temp_dir().join(format!("rustinho-nav-{}", crate::tempo::novo_id()));
    std::fs::create_dir_all(&pasta).unwrap();
    std::fs::write(pasta.join("compra.xml"), crate::nfe_xml::EXEMPLO).unwrap();
    (s, pasta)
}

/// O que a tela Configurações manda ao salvar: o que leu, com o CNPJ trocado.
fn salvar_cnpj(s: &mut crate::Servico, cnpj: &str) -> Value {
    let c = ok(s, "config_ler", json!({}));
    let tabelas: Vec<Value> = c["tabelas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| json!({ "numero": t["numero"], "nome": t["nome"], "lucroPadrao": t["lucroPadrao"] }))
        .collect();
    chamar(
        s,
        "config_salvar",
        json!({ "loja": c["loja"], "caixa": c["caixa"], "cnpj": cnpj, "uf": c["uf"], "endereco": c["endereco"],
                "telefone": c["telefone"], "imprimirDireto": c["imprimirDireto"], "tabelas": tabelas,
                "crediarioMulta": c["crediarioMulta"], "crediarioJuros": c["crediarioJuros"],
                "crediarioCarencia": c["crediarioCarencia"], "orcamentoValidade": c["orcamentoValidade"],
                "rodapeVenda": c["rodapeVenda"], "rodapeOrcamento": c["rodapeOrcamento"], "rodapeOs": c["rodapeOs"],
                "imprimirLocal": c["imprimirLocal"] }),
    )
}

fn satisfeito(s: &mut crate::Servico) -> bool {
    ok(s, "requisito_conferir", json!({ "id": "loja.cnpj" }))["satisfeito"].as_bool().unwrap()
}

/// Sem CNPJ, a prévia da importação recusa (como antes) e diz qual requisito
/// falta. Com o CNPJ salvo, a mesma prévia passa.
#[test]
fn nav01_previa_sem_cnpj_diz_o_requisito() {
    let (mut s, pasta) = loja();
    let arquivos = json!([pasta.display().to_string()]);
    let r = chamar(&mut s, "importar_previa", json!({ "arquivos": arquivos }));
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(r["campo"], "requisito:loja.cnpj", "{r}");
    assert!(r["erro"].as_str().unwrap().contains("CNPJ da loja"), "{r}");
    assert_eq!(salvar_cnpj(&mut s, "11.444.777/0001-61")["ok"], true);
    let p = ok(&mut s, "importar_previa", json!({ "arquivos": arquivos }));
    assert_eq!(p["notas"], 1, "{p}");
}

/// O requisito só fica satisfeito com o CNPJ salvo e válido pela regra de
/// `config_salvar`. Campo inválido é recusado ao salvar e o requisito
/// continua pendente; vazio também.
#[test]
fn nav01_requisito_cnpj_so_com_o_salvamento_valido() {
    let (mut s, _) = loja();
    assert!(!satisfeito(&mut s), "loja nova sem CNPJ");
    let r = salvar_cnpj(&mut s, "11.444.777/0001-00");
    assert_eq!((r["ok"].clone(), r["campo"].clone()), (json!(false), json!("cnpj")), "{r}");
    assert!(!satisfeito(&mut s), "CNPJ inválido não foi salvo: continua pendente");
    assert_eq!(salvar_cnpj(&mut s, "")["ok"], true, "vazio é aceito ao salvar (regra de hoje)");
    assert!(!satisfeito(&mut s), "mas não satisfaz o requisito");
    assert_eq!(salvar_cnpj(&mut s, "11.444.777/0001-61")["ok"], true);
    assert!(satisfeito(&mut s), "CNPJ válido salvo");
    // Requisito que o Rustinho não conhece: recusa clara (nada de "satisfeito").
    let r = chamar(&mut s, "requisito_conferir", json!({ "id": "nao.existe" }));
    assert_eq!(r["ok"], false, "{r}");
}

/// RNI-02: um CNPJ gravado mas inválido (de outra versão, de um backup
/// restaurado; `config_salvar` não deixa gravar assim) não passa pela
/// importação: a prévia recusa com o requisito, como sem CNPJ, em vez de
/// tratar as notas da loja como notas de fornecedores.
#[test]
fn nav02_cnpj_gravado_invalido_tambem_e_requisito() {
    let (mut s, pasta) = loja();
    crate::config::gravar(&s.banco_para_teste().conn, "loja_cnpj", "11.444.777/0001-00").unwrap();
    let arquivos = json!([pasta.display().to_string()]);
    let r = chamar(&mut s, "importar_previa", json!({ "arquivos": arquivos }));
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(r["campo"], "requisito:loja.cnpj", "{r}");
    assert!(r["erro"].as_str().unwrap().contains("não é válido"), "{r}");
    let c = ok(&mut s, "requisito_conferir", json!({ "id": "loja.cnpj" }));
    assert_eq!((c["satisfeito"].clone(), c["falta"].clone()), (json!(false), json!("O CNPJ salvo não é válido.")));
    assert_eq!(salvar_cnpj(&mut s, "11.444.777/0001-61")["ok"], true);
    assert_eq!(ok(&mut s, "importar_previa", json!({ "arquivos": arquivos }))["notas"], 1);
}

fn contar(s: &mut crate::Servico, tabela: &str) -> i64 {
    s.banco_para_teste().conn.query_row(&format!("SELECT COUNT(*) FROM {tabela}"), [], |l| l.get(0)).unwrap()
}

/// Etapa 1 (0.9.17): Compras sem o CNPJ da loja, ou com um inválido gravado,
/// não importa: recusa com o requisito (a tela leva a Configurações › Loja),
/// sem gravar nada. Salvo o CNPJ válido, o mesmo arquivo importa — quando a
/// tela mandar de novo (nada é importado sozinho).
#[test]
fn nav03_compras_sem_cnpj_e_requisito() {
    let (mut s, pasta) = loja();
    let caminho = pasta.join("compra.xml").display().to_string();
    let antes = (contar(&mut s, "compra"), contar(&mut s, "pessoa"));
    let r = chamar(&mut s, "compra_importar", json!({ "caminho": caminho }));
    assert_eq!((r["ok"].clone(), r["campo"].clone()), (json!(false), json!("requisito:loja.cnpj")), "{r}");
    assert!(r["erro"].as_str().unwrap().contains("nota de compra"), "{r}");
    crate::config::gravar(&s.banco_para_teste().conn, "loja_cnpj", "11.444.777/0001-00").unwrap();
    let r = chamar(&mut s, "compra_importar", json!({ "caminho": caminho }));
    assert_eq!(r["campo"], "requisito:loja.cnpj", "inválido gravado também: {r}");
    assert!(r["erro"].as_str().unwrap().contains("não é válido"), "{r}");
    assert_eq!((contar(&mut s, "compra"), contar(&mut s, "pessoa")), antes, "nada gravado");
    // Inválido digitado na tela: recusado ao salvar, continua pendente.
    assert_eq!(salvar_cnpj(&mut s, "11.444.777/0001-99")["campo"], "cnpj");
    assert!(!satisfeito(&mut s));
    assert_eq!(salvar_cnpj(&mut s, "11.444.777/0001-61")["ok"], true);
    assert!(satisfeito(&mut s));
    assert_eq!(contar(&mut s, "compra"), antes.0, "salvar o CNPJ não importa nada");
    let r = ok(&mut s, "compra_importar", json!({ "caminho": caminho }));
    assert_eq!(r["compras"].as_array().unwrap().len(), 1, "{r}");
    assert_eq!(r["conflitos"], json!([]), "{r}");
}

/// Etapa 1: nota emitida para outro CNPJ. Não entra (regra de hoje) e não é
/// só um erro de texto: vem em `conflitos`, com os dois documentos e a nota
/// para conferir. Nada é gravado e o CNPJ da loja não muda.
#[test]
fn nav04_nota_de_outro_cnpj_vem_como_conflito() {
    let (mut s, pasta) = loja();
    assert_eq!(salvar_cnpj(&mut s, "11.222.333/0001-81")["ok"], true);
    let antes = (contar(&mut s, "compra"), contar(&mut s, "pessoa"));
    let r = ok(&mut s, "compra_importar", json!({ "caminho": pasta.join("compra.xml").display().to_string() }));
    assert_eq!((r["compras"].clone(), r["erros"].clone()), (json!([]), json!([])), "{r}");
    let c = &r["conflitos"][0];
    assert_eq!(r["conflitos"].as_array().unwrap().len(), 1, "{r}");
    assert_eq!(c["arquivo"], "compra.xml");
    assert_eq!(c["lojaDocumento"], "11.222.333/0001-81");
    assert_eq!(c["destinatarioDocumento"], "11.444.777/0001-61");
    assert_ne!(c["emitenteDocumento"], "", "{c}");
    assert!(!c["itens"].as_array().unwrap().is_empty(), "a nota para conferir: {c}");
    for campo in ["numero", "emissao", "total", "lojaNome", "emitenteNome", "destinatarioNome"] {
        assert!(c[campo].is_string(), "{campo}: {c}");
    }
    assert_eq!((contar(&mut s, "compra"), contar(&mut s, "pessoa")), antes, "nada gravado");
    let loja: String = crate::config::ler(&s.banco_para_teste().conn, "loja_cnpj").unwrap().unwrap();
    assert_eq!(crate::documento::limpar(&loja), "11222333000181", "o CNPJ da loja não muda");
}

/// Etapa 1: o requisito do Fiscal (estado e CNPJ da loja), conferido pelo
/// banco, com o que falta dito; só satisfeito com os dois salvos e válidos.
#[test]
fn nav05_requisito_do_fiscal_uf_e_cnpj() {
    let (mut s, _) = loja();
    let conferir = |s: &mut crate::Servico| ok(s, "requisito_conferir", json!({ "id": "loja.uf_cnpj" }));
    let c = conferir(&mut s);
    assert_eq!(c["satisfeito"], false);
    let falta = c["falta"].as_str().unwrap().to_string();
    assert!(falta.contains("UF") && falta.contains("CNPJ"), "{falta}");
    assert_eq!(salvar_cnpj(&mut s, "11.444.777/0001-61")["ok"], true);
    let c = conferir(&mut s);
    assert_eq!(
        (c["satisfeito"].clone(), c["falta"].clone()),
        (json!(false), json!("O estado (UF) da loja ainda não está salvo."))
    );
    crate::config::gravar(&s.banco_para_teste().conn, "loja_uf", "ZZ").unwrap();
    assert_eq!(conferir(&mut s)["satisfeito"], false, "UF que não existe");
    crate::config::gravar(&s.banco_para_teste().conn, "loja_uf", "SP").unwrap();
    assert_eq!(conferir(&mut s)["satisfeito"], true);
    crate::config::gravar(&s.banco_para_teste().conn, "loja_cnpj", "").unwrap();
    let c = conferir(&mut s);
    assert_eq!(
        (c["satisfeito"].clone(), c["falta"].clone()),
        (json!(false), json!("O CNPJ da loja ainda não está salvo."))
    );
}

/// Arquivos que sumiram enquanto o usuário estava nas Configurações: a
/// conferência separa os que continuam dos que faltam, sem recusar tudo.
#[test]
fn nav01_arquivos_conferir_separa_os_que_faltam() {
    let (mut s, pasta) = loja();
    let um = pasta.join("compra.xml").display().to_string();
    let dois = pasta.join("sumiu.xml").display().to_string();
    let r = ok(&mut s, "arquivos_conferir", json!({ "arquivos": [um, dois, pasta.display().to_string()] }));
    assert_eq!(r["existem"], json!([um, pasta.display().to_string()]), "{r}");
    assert_eq!(r["faltam"], json!([dois]), "{r}");
}

/// Os comandos novos são só de leitura (o teste geral `so_leitura_nao_grava`
/// também passa por eles) e não precisam de gerente.
#[test]
fn nav01_comandos_novos_sao_so_leitura() {
    for c in ["requisito_conferir", "arquivos_conferir"] {
        assert!(crate::api::so_leitura(c), "{c} não é só leitura");
    }
}

/// Caso 6 (C8, 0.9.18): o rascunho de OS novo bloqueado por um caixa sem
/// versão confirmada vem com o requisito (a tela oferece "Ver caixas", sem
/// desviar sozinha); `requisito_conferir` só diz satisfeito quando nenhum
/// caixa bloqueia (aqui: o gerente retirou o caixa da loja). Conferir não
/// grava nada e o rascunho não é salvo sozinho.
#[test]
fn nav06_rascunho_bloqueado_e_requisito_dos_caixas() {
    let (mut s, _) = loja();
    let conferir = |s: &mut crate::Servico| ok(s, "requisito_conferir", json!({ "id": "caixas.rascunho" }));
    assert_eq!(conferir(&mut s)["satisfeito"], true, "sozinho, pode");
    s.banco_para_teste()
        .conn
        .execute(
            "INSERT INTO sync_par (dispositivo, nome, endereco) VALUES ('caixa-2', 'Caixa 2', '10.0.0.2:47810')",
            [],
        )
        .unwrap();
    let os = json!({ "id": "", "clienteNome": "Carlos", "placa": "ABC1", "defeito": "Barulho",
                     "itens": [{ "tipo": "servico", "descricao": "Revisão", "preco": "50", "quantidade": "1" }] });
    let r = chamar(&mut s, "os_rascunho_salvar", os.clone());
    assert_eq!((r["ok"].clone(), r["campo"].clone()), (json!(false), json!("requisito:caixas.rascunho")), "{r}");
    assert!(r["erro"].as_str().unwrap().contains("Simultaneidade › Caixas"), "{r}");
    let c = conferir(&mut s);
    assert_eq!(c["satisfeito"], false);
    assert!(c["falta"].as_str().unwrap().contains("Caixa 2"), "{c}");
    let rascunhos = |s: &mut crate::Servico| -> i64 {
        s.banco_para_teste()
            .conn
            .query_row("SELECT COUNT(*) FROM ordem_servico WHERE situacao = 'rascunho'", [], |l| l.get(0))
            .unwrap()
    };
    assert_eq!(rascunhos(&mut s), 0);
    ok(&mut s, "caixa_retirar", json!({ "dispositivo": "caixa-2", "motivo": "Computador vendido" }));
    assert_eq!(conferir(&mut s)["satisfeito"], true);
    assert_eq!(rascunhos(&mut s), 0, "conferir não salva o rascunho sozinho");
    assert_eq!(ok(&mut s, "os_rascunho_salvar", os)["situacao"], "rascunho", "com o clique, salva");
    assert_eq!(rascunhos(&mut s), 1);
}

/// Caso 7 (C8, dev3.3): a operação com dinheiro de um registro comprovadamente inconsistente é recusada com o
/// requisito do registro EXATO (para a tela destacá-lo em Simultaneidade › Caixas); `requisito_conferir` diz, pelo
/// banco, quando ele deixou de bloquear (decisão do gerente); conferir não refaz a operação, só o clique refaz.
#[test]
fn nav07_registro_inconsistente_e_requisito_com_o_registro_exato() {
    let (mut s, _) = loja();
    let mut f = ok(&mut s, "pessoa_nova", json!({ "papel": "cliente" }));
    f["nome"] = json!("Oficina do Zé");
    let cliente = ok(&mut s, "pessoa_salvar", f)["id"].as_str().unwrap().to_string();
    ok(
        &mut s,
        "receber_lancar",
        json!({ "clienteId": cliente, "descricao": "caderno", "valor": "200", "parcelas": 2, "primeiroVencimento": "" }),
    );
    let ids: Vec<String> = ok(&mut s, "receber_cliente", json!({ "id": cliente, "todas": false }))["parcelas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap().to_string())
        .collect();
    ok(&mut s, "caixa_abrir", json!({ "valor": "50" }));
    let plantar = |s: &mut crate::Servico, id: &str| {
        s.banco_para_teste()
            .conn
            .execute(
                "INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo)
                 VALUES ('conta_receber', json_array(?1), 'pessoa', '[\"sumiu\"]', 'inconsistente', '2026-01-01T00:00:00Z', 'log')",
                [id],
            )
            .unwrap();
    };
    plantar(&mut s, &ids[0]);
    plantar(&mut s, "outra-parcela-de-outro-cliente");
    let recebimentos = |s: &mut crate::Servico| -> i64 {
        s.banco_para_teste().conn.query_row("SELECT COUNT(*) FROM recebimento", [], |l| l.get(0)).unwrap()
    };
    let baixar = json!({ "clienteId": cliente, "contas": [ids[0]], "valor": "100", "forma": "dinheiro" });
    let r = chamar(&mut s, "receber_baixar", baixar.clone());
    let esperado = format!("requisito:caixas.referencia|conta_receber|[\"{}\"]", ids[0]);
    assert_eq!((r["ok"].clone(), r["campo"].clone()), (json!(false), json!(esperado)), "{r}");
    assert!(r["erro"].as_str().unwrap().contains("Caixas da loja"), "o texto de antes continua: {r}");
    assert_eq!(recebimentos(&mut s), 0, "nada recebido");

    let id = esperado.trim_start_matches("requisito:").to_string();
    let conferir = |s: &mut crate::Servico| ok(s, "requisito_conferir", json!({ "id": id }));
    let c = conferir(&mut s);
    assert_eq!(c["satisfeito"], false, "{c}");
    assert!(c["falta"].as_str().unwrap().contains("gerente"), "{c}");

    // A decisão do gerente: aceitar como está (o outro registro inconsistente continua, e não segura este).
    ok(
        &mut s,
        "referencia_aceitar",
        json!({ "tabela": "conta_receber", "chave": format!("[\"{}\"]", ids[0]), "motivo": "Caixa de origem perdido" }),
    );
    assert_eq!(conferir(&mut s)["satisfeito"], true, "o registro exato deixou de bloquear");
    assert_eq!(recebimentos(&mut s), 0, "conferir não recebe sozinho");
    ok(&mut s, "receber_baixar", baixar);
    assert_eq!(recebimentos(&mut s), 1, "com o clique, recebe");

    // Ids malformados não passam por satisfeitos.
    for ruim in ["caixas.referencia|", "caixas.referencia|conta_receber", "caixas.referencia||[\"x\"]"] {
        assert_eq!(chamar(&mut s, "requisito_conferir", json!({ "id": ruim }))["ok"], false, "{ruim}");
    }
}

/// Caso 7: toda operação que exige registro editável recusa com o requisito (não só o recebimento): o campo
/// vem de um lugar só (`exigir_sem_inconsistencia`), também para os itens (filhos).
#[test]
fn nav07_o_requisito_vem_do_bloqueio_comum() {
    let b = crate::banco::Banco::em_memoria().unwrap();
    b.conn
        .execute(
            "INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo)
             VALUES ('cheque', '[\"ch1\"]', 'pessoa', '[\"p\"]', 'inconsistente', '2026-01-01T00:00:00Z', 'pares')",
            [],
        )
        .unwrap();
    let e = crate::sync::exigir_editavel(&b.conn, "cheque", "ch1").unwrap_err();
    assert_eq!(e.nome_do_campo(), Some("requisito:caixas.referencia|cheque|[\"ch1\"]"));
    assert!(crate::sync::exigir_editavel(&b.conn, "cheque", "ch2").is_ok());
    assert!(crate::requisitos::conferir(&b.conn, "caixas.referencia|cheque|[\"ch1\"]").map(|s| !s.satisfeito).unwrap());
    assert!(crate::requisitos::conferir(&b.conn, "caixas.referencia|cheque|[\"ch2\"]").map(|s| s.satisfeito).unwrap());
}

/// 9a, 9c e 10a (C8, dev3.3): a notificação sobre UM registro leva o registro exato (`alvo`), para a tela
/// destacá-lo; com mais de um, leva só à lista (alvo vazio). Ler as notificações não muda nada.
#[test]
fn nav09_notificacao_de_um_registro_leva_o_registro_exato() {
    let (mut s, _) = loja();
    let alvo_de = |s: &mut crate::Servico, id: &str| -> Option<String> {
        crate::avisos::do_dia(&s.banco_para_teste().conn).unwrap().into_iter().find(|a| a.id == id).map(|a| a.alvo)
    };
    let sql = |s: &mut crate::Servico, q: &str| s.banco_para_teste().conn.execute_batch(q).unwrap();
    let conflito = "INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando)
                    VALUES ('produto', '[\"p1\"]', 'v1', 'v2', 'caixa-2', datetime('now'));";

    // 9a: um conflito → o conflito; dois → a lista.
    sql(&mut s, conflito);
    let id: i64 = s.banco_para_teste().conn.last_insert_rowid();
    assert_eq!(alvo_de(&mut s, "conflitos"), Some(format!("conflito:{id}")));
    sql(&mut s, conflito);
    assert_eq!(alvo_de(&mut s, "conflitos"), Some(String::new()), "dois: a lista");

    // 9c: um registro incompleto → ele; dois → a lista.
    let referencia = |chave: &str| {
        format!(
            "INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo)
             VALUES ('conta_receber', '[\"{chave}\"]', 'pessoa', '[\"p\"]', 'apagado', '2026-01-01T00:00:00Z', '');
             INSERT OR REPLACE INTO config (chave, valor) VALUES ('sync_quebradas', '1');"
        )
    };
    sql(&mut s, &referencia("c1"));
    assert_eq!(alvo_de(&mut s, "referencias"), Some("referencia:conta_receber|[\"c1\"]".to_string()));
    sql(&mut s, &referencia("c2"));
    assert_eq!(alvo_de(&mut s, "referencias"), Some(String::new()));

    // 9c: o aviso de um caixa leva o caixa (o que bloqueia o rascunho de OS).
    sql(&mut s, "INSERT INTO sync_par (dispositivo, nome, endereco) VALUES ('caixa-2', 'Caixa 2', '10.0.0.2:47810')");
    let avisos = crate::avisos::do_dia(&s.banco_para_teste().conn).unwrap();
    assert!(
        avisos.iter().any(|a| a.alvo == "caixa:caixa-2" && a.secao == "caixas"),
        "{:?}",
        avisos.iter().map(|a| (&a.id, &a.alvo)).collect::<Vec<_>>()
    );

    // 10a: uma encomenda → a linha dela; duas → a lista.
    let encomenda = |s: &mut crate::Servico, d: &str| {
        ok(
            s,
            "encomenda_criar",
            json!({ "clienteNome": "Maria Souza", "telefone": "(15) 99808-0000", "descricao": d, "quantidade": "1" }),
        )
    };
    let e = encomenda(&mut s, "Pastilha dianteira");
    let id = e["id"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| s.banco_para_teste().conn.query_row("SELECT id FROM encomenda", [], |l| l.get(0)).unwrap());
    assert_eq!(alvo_de(&mut s, "encomendas"), Some(format!("encomenda:{id}")));
    encomenda(&mut s, "Filtro de óleo");
    assert_eq!(alvo_de(&mut s, "encomendas"), Some(String::new()));
}

/// 9e (C8, dev3.3): notificações do fiscal. UF/CNPJ faltando numa loja que já tem nota de compra pelo XML (Auto
/// na tela, pela guia `loja.uf_cnpj`); notas de compra aguardando a conferência da autorização (Opções: a lista
/// de compras). Nenhuma diz que o Rustinho emite nota; ler não grava nada.
#[test]
fn nav10_notificacoes_do_fiscal() {
    let (mut s, pasta) = loja();
    let avisos = |s: &mut crate::Servico| crate::avisos::do_dia(&s.banco_para_teste().conn).unwrap();
    let ids = |s: &mut crate::Servico| avisos(s).into_iter().map(|a| a.id).collect::<Vec<_>>();
    assert!(!ids(&mut s).iter().any(|i| i.starts_with("fiscal_")), "loja sem notas: nada de fiscal");
    // Com o CNPJ (sem a UF), uma nota importada.
    assert_eq!(salvar_cnpj(&mut s, "11.444.777/0001-61")["ok"], true);
    let caminho = pasta.join("compra.xml").display().to_string();
    ok(&mut s, "compra_importar", json!({ "caminho": caminho }));
    let todos = avisos(&mut s);
    let loja = todos.iter().find(|a| a.id == "fiscal_loja").expect("falta a UF: avisa");
    assert_eq!(
        (loja.funcao.as_str(), loja.secao.as_str(), loja.acao.as_str()),
        ("configuracoes", "loja", "Informar UF e CNPJ")
    );
    assert!(loja.descricao.contains("estado (UF)"), "{}", loja.descricao);
    let notas = todos.iter().find(|a| a.id == "fiscal_notas").expect("a nota aguarda a conferência");
    assert_eq!((notas.funcao.as_str(), notas.acao.as_str()), ("compras", "Ver compras"));
    for a in todos.iter().filter(|a| a.id.starts_with("fiscal_")) {
        let tudo = format!("{} {} {}", a.texto, a.atraso, a.descricao).to_lowercase();
        assert!(!tudo.contains("emitir") && !tudo.contains("emissão"), "não sugere emissão: {tudo}");
    }
    // A UF salva: a do fiscal da loja some (a das notas continua até a conferência).
    crate::config::gravar(&s.banco_para_teste().conn, "loja_uf", "SP").unwrap();
    let agora = ids(&mut s);
    assert!(!agora.contains(&"fiscal_loja".to_string()) && agora.contains(&"fiscal_notas".to_string()), "{agora:?}");
}

/// 10b (C8, dev3.3): comissão e conta finalizada. Fechar de novo um período já fechado é recusado com o
/// fechamento exato (a tela o destaca); desfazer o fechamento de uma comissão já paga é recusado com a conta
/// exata (a tela oferece "Ver no A pagar"). Nenhuma das duas mexe em valor.
#[test]
fn nav11_comissao_fechada_e_conta_paga_dizem_o_registro() {
    let (mut s, _) = loja();
    s.banco_para_teste().conn.execute("UPDATE usuario SET comissao = 1000 WHERE nome = 'Ana'", []).unwrap();
    let mut f = ok(&mut s, "produto_novo", json!({}));
    f["nome"] = json!("Filtro");
    f["custo"] = json!("10");
    f["estoqueInicial"] = json!("10");
    let p = ok(&mut s, "produto_salvar", f)["id"].as_str().unwrap().to_string();
    ok(&mut s, "caixa_abrir", json!({ "valor": "100" }));
    ok(
        &mut s,
        "venda_concluir",
        json!({ "itens": [{ "produtoId": p, "quantidade": "1" }], "tabela": 1,
                "pagamentos": [{ "forma": "dinheiro", "valor": "100" }] }),
    );
    let ana: String =
        s.banco_para_teste().conn.query_row("SELECT id FROM usuario WHERE nome = 'Ana'", [], |l| l.get(0)).unwrap();
    let periodo = json!({ "usuarioId": ana, "de": "", "ate": "", "vencimento": "" });
    let r = ok(&mut s, "comissao_fechar", periodo.clone());
    let fech = r["fechamentos"][0].clone();
    let (fid, conta) = (fech["id"].as_str().unwrap().to_string(), fech["contaPagarId"].as_str().unwrap().to_string());
    assert!(!conta.is_empty() && fech["pago"] == false, "{fech}");

    let r = chamar(&mut s, "comissao_fechar", periodo);
    assert_eq!((r["ok"].clone(), r["campo"].clone()), (json!(false), json!(format!("fechamento:{fid}"))), "{r}");
    assert!(r["erro"].as_str().unwrap().contains("já foi fechada"), "{r}");

    ok(
        &mut s,
        "pagar_pagar",
        json!({ "contaId": conta, "valor": "", "juros": "", "desconto": "", "data": "", "forma": "dinheiro",
                "origem": "gaveta", "chequeNumero": "", "observacoes": "" }),
    );
    let valores = |s: &mut crate::Servico| -> (i64, i64) {
        s.banco_para_teste()
            .conn
            .query_row("SELECT valor, pago FROM conta_pagar_saldo WHERE id = ?1", [&conta], |l| {
                Ok((l.get(0)?, l.get(1)?))
            })
            .unwrap()
    };
    let antes = valores(&mut s);
    let r = chamar(&mut s, "comissao_reabrir", json!({ "id": fid }));
    assert_eq!((r["ok"].clone(), r["campo"].clone()), (json!(false), json!(format!("conta_paga:{conta}"))), "{r}");
    assert_eq!(valores(&mut s), antes, "nada mudou de valor");
    let lista = ok(&mut s, "comissoes", json!({ "de": "", "ate": "" }));
    assert_eq!(
        (lista["fechamentos"][0]["pago"].clone(), lista["fechamentos"][0]["cancelado"].clone()),
        (json!(true), json!(false))
    );
}

/// 9b (C8, dev3.3): o conflito em Duplicatas leva a tabela e o id do registro (só quando a chave é de um id
/// só), e cada evento do Histórico leva a entidade e o id dele: a tela filtra pelo registro exato quando a
/// ligação é inequívoca. Só consulta.
#[test]
fn nav12_conflito_e_historico_ligados_pelo_registro() {
    let (mut s, _) = loja();
    let mut f = ok(&mut s, "produto_novo", json!({}));
    f["nome"] = json!("Filtro");
    f["custo"] = json!("10");
    let p = ok(&mut s, "produto_salvar", f)["id"].as_str().unwrap().to_string();
    s.banco_para_teste()
        .conn
        .execute(
            "INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando)
             VALUES ('produto', json_array(?1), 'v1', 'v2', 'caixa-2', datetime('now')),
                    ('produto_aplicacao', '[\"a\",\"b\"]', 'v3', 'v4', 'caixa-2', datetime('now'))",
            [&p],
        )
        .unwrap();
    let conflitos = ok(&mut s, "conflitos_listar", json!({}));
    let do_produto = conflitos.as_array().unwrap().iter().find(|c| c["tabela"] == "produto").unwrap();
    assert_eq!(do_produto["registroId"], json!(p), "{do_produto}");
    let composto = conflitos.as_array().unwrap().iter().find(|c| c["tabela"] == "produto_aplicacao").unwrap();
    assert_eq!(composto["registroId"], "", "chave de mais de um id: a lista inteira");
    let historico = ok(&mut s, "auditoria_listar", json!({}));
    assert!(
        historico.as_array().unwrap().iter().any(|r| r["entidade"] == "produto" && r["entidadeId"] == json!(p)),
        "o cadastro da peça aparece ligado a ela: {historico}"
    );
}

/// M-1 da revisão do Fable (C8): desfazer o fechamento de uma comissão cuja conta está INCONSISTENTE (não paga) não
/// pode dizer "já foi paga": a recusa é a do caso 7, com o registro exato.
#[test]
fn rev_m1_comissao_com_conta_inconsistente_recusa_pelo_caso_7() {
    let (mut s, _) = loja();
    s.banco_para_teste().conn.execute("UPDATE usuario SET comissao = 1000 WHERE nome = 'Ana'", []).unwrap();
    let mut f = ok(&mut s, "produto_novo", json!({}));
    f["nome"] = json!("Filtro");
    f["custo"] = json!("10");
    f["estoqueInicial"] = json!("10");
    let p = ok(&mut s, "produto_salvar", f)["id"].as_str().unwrap().to_string();
    ok(&mut s, "caixa_abrir", json!({ "valor": "100" }));
    ok(
        &mut s,
        "venda_concluir",
        json!({ "itens": [{ "produtoId": p, "quantidade": "1" }], "tabela": 1,
                "pagamentos": [{ "forma": "dinheiro", "valor": "100" }] }),
    );
    let ana: String =
        s.banco_para_teste().conn.query_row("SELECT id FROM usuario WHERE nome = 'Ana'", [], |l| l.get(0)).unwrap();
    let r = ok(&mut s, "comissao_fechar", json!({ "usuarioId": ana, "de": "", "ate": "", "vencimento": "" }));
    let (fid, conta) = (
        r["fechamentos"][0]["id"].as_str().unwrap().to_string(),
        r["fechamentos"][0]["contaPagarId"].as_str().unwrap().to_string(),
    );
    s.banco_para_teste()
        .conn
        .execute(
            "INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo)
             VALUES ('conta_pagar', json_array(?1), 'pessoa', '[\"p\"]', 'inconsistente', '2026-01-01T00:00:00Z', 'log')",
            [&conta],
        )
        .unwrap();
    let r = chamar(&mut s, "comissao_reabrir", json!({ "id": fid }));
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(r["campo"], json!(format!("requisito:caixas.referencia|conta_pagar|[\"{conta}\"]")), "M-1: {r}");
    assert!(!r["erro"].as_str().unwrap().contains("já foi paga"), "M-1: mensagem falsa: {r}");
}

/// B-6 da revisão do Fable (C8): a notificação das notas aguardando a conferência não conta compra já lançada.
#[test]
fn rev_b6_fiscal_notas_nao_conta_compra_lancada() {
    let (mut s, pasta) = loja();
    assert_eq!(salvar_cnpj(&mut s, "11.444.777/0001-61")["ok"], true);
    let caminho = pasta.join("compra.xml").display().to_string();
    ok(&mut s, "compra_importar", json!({ "caminho": caminho }));
    // A compra importada passa a "lançada" (por fora), ainda com a autorização aguardando.
    s.banco_para_teste().conn.execute("UPDATE compra SET situacao = 'lancada'", []).unwrap();
    let ids = crate::avisos::do_dia(&s.banco_para_teste().conn).unwrap().into_iter().map(|a| a.id).collect::<Vec<_>>();
    assert!(!ids.contains(&"fiscal_notas".to_string()), "B-6: compra lançada contada: {ids:?}");
}

/// I1 (decisão do proprietário, 15/11; 0915-M): o mesmo número de encomenda em caixas diferentes vira um aviso de
/// atenção próprio, que leva à encomenda do OUTRO caixa (destacada em Estoque › Encomendas). Um caixa só, nada;
/// cancelada a outra, o aviso sai.
#[test]
fn nav13_encomenda_com_o_mesmo_numero_em_caixas_diferentes() {
    let (mut s, _pasta) = loja();
    let sql = |s: &mut crate::Servico, q: &str| s.banco_para_teste().conn.execute_batch(q).unwrap();
    let criar = |s: &mut crate::Servico, d: &str| {
        ok(
            s,
            "encomenda_criar",
            json!({ "clienteNome": "Maria Souza", "telefone": "(15) 99808-0000", "descricao": d, "quantidade": "1" }),
        )
    };
    let repetido = |s: &mut crate::Servico| {
        crate::avisos::do_dia(&s.banco_para_teste().conn)
            .unwrap()
            .into_iter()
            .find(|a| a.id == "encomendas_numero_repetido")
    };
    criar(&mut s, "Pastilha dianteira");
    criar(&mut s, "Filtro de óleo");
    assert!(repetido(&mut s).is_none(), "números diferentes, um caixa só: nada");
    // A segunda veio de outro caixa com o mesmo número (como a troca de dados traria).
    sql(&mut s, "UPDATE encomenda SET numero = 1, dispositivo = 'caixa-2' WHERE numero = 2");
    let outra: String = s
        .banco_para_teste()
        .conn
        .query_row("SELECT id FROM encomenda WHERE dispositivo = 'caixa-2'", [], |l| l.get(0))
        .unwrap();
    let a = repetido(&mut s).expect("o mesmo número em dois caixas: avisa");
    assert_eq!(a.categoria, crate::avisos::ATENCAO);
    assert_eq!(a.alvo, format!("encomenda:{outra}"), "leva à encomenda do outro caixa");
    assert_eq!((a.funcao.as_str(), a.acao.as_str()), ("estoque", "Ver encomendas"));
    assert!(a.texto.contains("nº 1"), "{}", a.texto);
    sql(&mut s, "UPDATE encomenda SET situacao = 'cancelada' WHERE dispositivo = 'caixa-2'");
    assert!(repetido(&mut s).is_none(), "cancelada: não conta");
}
