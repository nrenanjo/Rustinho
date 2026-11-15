//! QT-1b: regressão dos achados da revisão independente (Fable, 15/11/2026; `revisao_independente/fable/
//! RELATORIO_QT1.md`). Cada teste reproduz um achado pela interface do serviço (a mesma na versão congelada e na
//! corrigida): **falha na versão congelada** e passa depois da correção (a rodada de antes fica nas evidências).

use std::path::PathBuf;

use serde_json::{json, Value};

fn chamar(s: &mut crate::Servico, cmd: &str, d: Value) -> Value {
    serde_json::from_str(&s.chamar(cmd, &d.to_string())).unwrap()
}

fn ok(s: &mut crate::Servico, cmd: &str, d: Value) -> Value {
    let r = chamar(s, cmd, d);
    assert_eq!(r["ok"], true, "{cmd}: {r}");
    r["dados"].clone()
}

fn temp(nome: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rustinho-qt1b-rev-{nome}-{}", crate::tempo::novo_id()));
    std::fs::create_dir_all(&p).unwrap();
    p.join("rustinho.db")
}

fn abrir(caminho: &std::path::Path) -> crate::Servico {
    crate::Servico::abrir_para_teste(caminho, false).unwrap()
}

fn sql_tolerante(s: &mut crate::Servico, comandos: &[&str]) {
    // Cada comando sozinho: o que não vale para o esquema desta versão (coluna ou tabela de outra versão) é
    // ignorado. Assim o mesmo teste roda na versão congelada e na corrigida.
    for c in comandos {
        let _ = s.banco_para_teste().conn.execute_batch(c);
    }
}

fn configurar(s: &mut crate::Servico) {
    ok(
        s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
    );
}

fn preparar_venda(s: &mut crate::Servico) -> String {
    let mut f = ok(s, "produto_novo", json!({}));
    f["nome"] = json!("Filtro");
    f["custo"] = json!("10");
    f["estoqueInicial"] = json!("10");
    f["chaveOperacao"] = json!("peca-1");
    let p = ok(s, "produto_salvar", f)["id"].as_str().unwrap().to_string();
    ok(s, "caixa_abrir", json!({ "valor": "0", "chaveOperacao": "abrir-1" }));
    p
}

fn venda(p: &str, chave: &str) -> Value {
    json!({ "itens": [{ "produtoId": p, "quantidade": "1" }], "tabela": 1,
            "pagamentos": [{ "forma": "dinheiro", "valor": "100" }], "chaveOperacao": chave })
}

fn entrar(s: &mut crate::Servico, nome: &str, senha: &str) {
    let lista = ok(s, "usuarios_entrada", json!({}));
    let id = lista.as_array().unwrap().iter().find(|u| u["nome"] == nome).unwrap()["id"].as_str().unwrap().to_string();
    ok(s, "entrar", json!({ "id": id, "senha": senha }));
}

fn aviso(s: &mut crate::Servico) -> Option<Value> {
    let avisos = ok(s, "avisos_do_dia", json!({}));
    avisos
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"].as_str().unwrap_or("").starts_with("fechamento_inesperado"))
        .cloned()
}

fn linhas(a: &Value) -> Vec<String> {
    a["detalhes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| format!("{}: {}", d["rotulo"].as_str().unwrap(), d["valor"].as_str().unwrap()))
        .collect()
}

fn fim(s: crate::Servico, caminho: &std::path::Path) {
    drop(s);
    let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
}

/// F-01: duas quedas seguidas (a segunda antes de alguém entrar) não podem apagar do aviso a venda gravada antes
/// da primeira.
#[test]
fn rev_f01_duas_quedas_seguidas_mantem_a_venda() {
    let caminho = temp("f01");
    let mut s = abrir(&caminho);
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    let v = ok(&mut s, "venda_concluir", venda(&p, "venda-1"));
    s.cair_para_teste(); // 1ª queda: a tela não chegou a mostrar a venda
    let s = abrir(&caminho);
    s.cair_para_teste(); // 2ª queda, antes de alguém entrar
    let mut s = abrir(&caminho);
    entrar(&mut s, "Ana", "1234");
    let a = aviso(&mut s).expect("F-01: o aviso precisa continuar depois de duas quedas seguidas");
    let l = linhas(&a);
    assert!(l.iter().any(|x| x.starts_with(&format!("Gravada: Venda · nº {} · ", v["numero"]))), "{l:?}");
    fim(s, &caminho);
}

/// F-02: a venda que a tela já mostrou (houve outra chamada depois dela) não vira aviso de atenção quando o
/// processo morre depois (no Android: Início, e o sistema mata o app em segundo plano).
#[test]
fn rev_f02_venda_confirmada_na_tela_nao_vira_aviso() {
    let caminho = temp("f02");
    let mut s = abrir(&caminho);
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    ok(&mut s, "venda_concluir", venda(&p, "venda-1"));
    ok(&mut s, "entrega_confirmar", json!({})); // a tela desenhou a resposta da venda (N-01, parte 2)
    s.cair_para_teste();
    let mut s = abrir(&caminho);
    entrar(&mut s, "Ana", "1234");
    assert!(aviso(&mut s).is_none(), "F-02: o que a tela já mostrou não é motivo de atenção");
    fim(s, &caminho);
}

/// F-02 (controle): a venda gravada sem a tela ter seguido (morreu logo depois) continua no aviso.
#[test]
fn rev_f02_venda_sem_confirmacao_continua_no_aviso() {
    let caminho = temp("f02c");
    let mut s = abrir(&caminho);
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    let v = ok(&mut s, "venda_concluir", venda(&p, "venda-1"));
    s.cair_para_teste();
    let mut s = abrir(&caminho);
    entrar(&mut s, "Ana", "1234");
    let l = linhas(&aviso(&mut s).expect("a venda sem confirmação avisa"));
    assert!(l.iter().any(|x| x.starts_with(&format!("Gravada: Venda · nº {} · ", v["numero"]))), "{l:?}");
    fim(s, &caminho);
}

/// F-03: a abertura que caiu correu com o relógio adiantado 1 hora (acerto de hora depois): o aviso não pode
/// sumir.
#[test]
fn rev_f03_relogio_que_volta_nao_apaga_o_aviso() {
    let caminho = temp("f03");
    let mut s = abrir(&caminho);
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    let v = ok(&mut s, "venda_concluir", venda(&p, "venda-1"));
    // A abertura inteira "aconteceu" uma hora no futuro (as duas versões do esquema; o que não existe é ignorado).
    sql_tolerante(
        &mut s,
        &[
            "UPDATE programa_sessao SET aberta_ms = aberta_ms + 3600000",
            "UPDATE programa_abertura SET aberta_ms = aberta_ms + 3600000",
            "UPDATE operacao_feita SET feita_ms = feita_ms + 3600000",
            "UPDATE operacao_em_curso SET iniciada_ms = iniciada_ms + 3600000",
        ],
    );
    s.cair_para_teste();
    let mut s = abrir(&caminho);
    entrar(&mut s, "Ana", "1234");
    let l = linhas(&aviso(&mut s).expect("F-03: o relógio que volta não pode apagar o aviso"));
    assert!(l.iter().any(|x| x.starts_with(&format!("Gravada: Venda · nº {} · ", v["numero"]))), "{l:?}");
    fim(s, &caminho);
}

/// F-04: uma operação "em curso" que sobrou de outra abertura (de semanas atrás) não entra no aviso de uma queda
/// de hoje.
#[test]
fn rev_f04_em_curso_de_outra_abertura_nao_entra() {
    let caminho = temp("f04");
    let mut s = abrir(&caminho);
    configurar(&mut s);
    let usuario: String =
        s.banco_para_teste().conn.query_row("SELECT id FROM usuario WHERE nome = 'Ana'", [], |l| l.get(0)).unwrap();
    // A linha órfã, de 01/10 (abertura velha; na versão corrigida, a abertura 0 = nenhuma de agora).
    let orfa = format!(
        "INSERT INTO operacao_em_curso (chave, comando, usuario_id, iniciada_em, iniciada_ms)
         VALUES (randomblob(32), 'venda_concluir', '{usuario}', '2026-10-01T12:00:00Z', 1790000000000)"
    );
    sql_tolerante(&mut s, &[orfa.as_str()]);
    s.cair_para_teste();
    let mut s = abrir(&caminho);
    entrar(&mut s, "Ana", "1234");
    if let Some(a) = aviso(&mut s) {
        let l = linhas(&a);
        assert!(
            !l.iter().any(|x| x.contains("Venda") && x.starts_with("Não concluída")),
            "F-04: linha velha no aviso: {l:?}"
        );
    }
    fim(s, &caminho);
}

/// F-05: o aviso passa pelo mesmo caminho dos outros: minimizar pega (e fica depois da recarga) e o resumo não é
/// vazio (o leitor de tela).
#[test]
fn rev_f05_minimizar_o_aviso_pega() {
    let caminho = temp("f05");
    let mut s = abrir(&caminho);
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    ok(&mut s, "venda_concluir", venda(&p, "venda-1"));
    s.cair_para_teste();
    let mut s = abrir(&caminho);
    entrar(&mut s, "Ana", "1234");
    let a = aviso(&mut s).expect("o aviso");
    assert_eq!(a["minimizado"], false, "o aviso novo começa aberto");
    assert!(!a["resumo"].as_str().unwrap_or("").is_empty(), "F-05: resumo vazio");
    ok(&mut s, "aviso_minimizar", json!({ "id": a["id"], "minimizado": true }));
    assert_eq!(aviso(&mut s).unwrap()["minimizado"], true, "F-05: minimizar não pegou");
    fim(s, &caminho);
}

/// F-09: a mesma chave com outra senha (troca de senha repetida) não pode responder "ok" sem trocar.
#[test]
fn rev_f09_senha_nova_com_a_mesma_chave_nao_e_ok_silencioso() {
    let mut s = crate::Servico::em_memoria().unwrap();
    configurar(&mut s);
    let bia = |senha: &str| json!({ "nome": "Bia", "senha": senha, "senha2": senha, "ativo": true, "gerente": false, "chaveOperacao": "bia-1" });
    ok(&mut s, "usuario_salvar", bia("mesa-verde"));
    let r = chamar(&mut s, "usuario_salvar", bia("mesa-azul"));
    if r["ok"] == true {
        // Se aceitou, tem de ter trocado de verdade.
        ok(&mut s, "sair", json!({}));
        let lista = ok(&mut s, "usuarios_entrada", json!({}));
        let id = lista.as_array().unwrap().iter().find(|u| u["nome"] == "Bia").unwrap()["id"].clone();
        let entrou = chamar(&mut s, "entrar", json!({ "id": id, "senha": "mesa-azul" }));
        assert_eq!(entrou["ok"], true, "F-09: respondeu ok ({r}) e a senha nova não vale");
    }
}

/// F-10: o "Conferi" do caixa não tira do gerente a operação não concluída do caixa.
#[test]
fn rev_f10_conferi_do_caixa_nao_tira_do_gerente() {
    let caminho = temp("f10");
    let mut s = abrir(&caminho);
    configurar(&mut s);
    ok(
        &mut s,
        "usuario_salvar",
        json!({ "nome": "Bia", "senha": "mesa-verde", "senha2": "mesa-verde", "ativo": true, "gerente": false,
                "chaveOperacao": "bia-1" }),
    );
    let bia: String =
        s.banco_para_teste().conn.query_row("SELECT id FROM usuario WHERE nome = 'Bia'", [], |l| l.get(0)).unwrap();
    // Um recebimento da Bia que começou e não terminou nesta abertura (as duas versões do esquema).
    let v1 = format!(
        "INSERT INTO operacao_em_curso (chave, comando, usuario_id, iniciada_em, iniciada_ms)
         VALUES (randomblob(32), 'receber_baixar', '{bia}', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                 CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER))"
    );
    let v2 = format!(
        "INSERT INTO operacao_em_curso (chave, comando, usuario_id, iniciada_em, iniciada_ms, abertura)
         VALUES (randomblob(32), 'receber_baixar', '{bia}', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                 CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER),
                 (SELECT MAX(numero) FROM programa_abertura))"
    );
    sql_tolerante(&mut s, &[v2.as_str()]);
    let ja: i64 =
        s.banco_para_teste().conn.query_row("SELECT COUNT(*) FROM operacao_em_curso", [], |l| l.get(0)).unwrap();
    if ja == 0 {
        sql_tolerante(&mut s, &[v1.as_str()]);
    }
    s.cair_para_teste();
    let mut s = abrir(&caminho);
    entrar(&mut s, "Bia", "mesa-verde");
    let a = aviso(&mut s).expect("a Bia vê o recebimento dela");
    ok(&mut s, "aviso_resolver", json!({ "id": a["id"] }));
    ok(&mut s, "sair", json!({}));
    entrar(&mut s, "Ana", "1234");
    let l = linhas(&aviso(&mut s).expect("F-10: o gerente ainda tem de ver o recebimento interrompido da Bia"));
    assert!(l.iter().any(|x| x.starts_with("Não concluída: Recebimento")), "F-10: {l:?}");
    fim(s, &caminho);
}

/// F-12: a sangria aparece com o tipo e o valor (a resposta dela é vazia: sem isso, "Movimento do caixa" não diz
/// qual foi).
#[test]
fn rev_f12_sangria_aparece_com_tipo_e_valor() {
    let caminho = temp("f12");
    let mut s = abrir(&caminho);
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    ok(&mut s, "venda_concluir", venda(&p, "venda-1")); // põe dinheiro na gaveta (o caixa abre com zero)
    ok(
        &mut s,
        "caixa_movimentar",
        json!({ "tipo": "sangria", "valor": "5", "motivo": "troco", "chaveOperacao": "sg-1" }),
    );
    s.cair_para_teste();
    let mut s = abrir(&caminho);
    entrar(&mut s, "Ana", "1234");
    let l = linhas(&aviso(&mut s).expect("o aviso"));
    assert!(l.iter().any(|x| x.contains("angria") && x.contains("5,00")), "F-12: {l:?}");
    fim(s, &caminho);
}

/// F-14: a recusa da chave reusada não manda refazer: manda conferir (a primeira FOI gravada).
#[test]
fn rev_f14_recusa_da_chave_reusada_manda_conferir() {
    let mut s = crate::Servico::em_memoria().unwrap();
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    ok(&mut s, "venda_concluir", venda(&p, "venda-1"));
    let mut outra = venda(&p, "venda-1");
    outra["itens"][0]["quantidade"] = json!("2");
    let r = chamar(&mut s, "venda_concluir", outra);
    let texto = r["erro"].as_str().unwrap_or("").to_lowercase();
    assert_eq!(r["ok"], false, "{r}");
    assert!(texto.contains("confira") && !texto.contains("tente de novo"), "F-14: {texto}");
}

/// F-11: só fica em `VARIAS_TRANSACOES` (o "confira" do aviso) o comando que grava em mais de uma transação; e
/// todo comando que grava em mais de uma está lá (lido no código dos atendentes).
#[test]
fn rev_f11_varias_transacoes_bate_com_o_codigo() {
    let fontes = [include_str!("compras/mod.rs"), include_str!("compras/importar.rs")];
    let corpo = |nome: &str| -> Option<String> {
        for f in fontes {
            if let Some(i) = f.find(&format!("fn {nome}(p: &mut Pedido")) {
                let resto = &f[i..];
                let fim = resto.find("\n    fn ").unwrap_or(resto.len());
                return Some(resto[..fim].to_string());
            }
        }
        None
    };
    for c in crate::operacoes::VARIAS_TRANSACOES {
        let b = corpo(c).unwrap_or_else(|| panic!("atendente de {c} não achado"));
        // Mais de uma transação: mais de um `transaction()` no atendente (o pacote de notas abre uma por nota e
        // outra para a marca).
        assert!(b.matches("transaction()").count() >= 2, "F-11: {c} grava numa transação só");
    }
}
