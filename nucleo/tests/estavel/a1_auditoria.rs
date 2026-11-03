//! Auditoria técnica da 0.9.14 (docs/AUDITORIA-0.9.14.md).
//!
//! D-14: cancelar uma venda, estornar um recebimento ou desfazer uma
//! devolução de um caixa já fechado não muda o fechamento daquele caixa; o
//! dinheiro sai (ou entra) na gaveta aberta deste computador ou, sem caixa
//! aberto, no Caixa geral, ligado à operação de origem, uma vez só.

use serde_json::{json, Value};

use crate::comum::*;

/// Fechamento de uma sessão: (esperado, diferença, vendas, canceladas,
/// total vendido, recebido, devolvido) em centavos.
fn fechamento(l: &mut Loja, sessao: &str) -> Value {
    let r = l.ok("caixa_resumo", json!({ "id": sessao }));
    json!({
        "esperado": centavos(r["esperado"].as_str().unwrap()),
        "diferenca": centavos(r["diferenca"].as_str().unwrap()),
        "contado": centavos(r["contado"].as_str().unwrap()),
        "vendas": r["vendas"],
        "canceladas": r["canceladas"],
        "vendido": centavos(r["totalVendido"].as_str().unwrap()),
        "recebido": centavos(r["totalRecebido"].as_str().unwrap()),
        "recebimentos": r["recebimentos"],
        "devolvido": centavos(r["devolvido"].as_str().unwrap()),
        "porForma": r["porForma"],
    })
}

fn sessao_aberta(l: &mut Loja) -> String {
    l.ok("caixa_estado", json!({}))["resumo"]["sessao"]["id"].as_str().unwrap().to_string()
}

/// Movimentos da gaveta ligados à operação: (sessão, tipo, valor).
fn movimentos_da_gaveta(l: &Loja, origem: &str) -> Vec<(String, String, i64)> {
    l.conexao()
        .prepare("SELECT sessao_id, tipo, valor FROM caixa_movimento WHERE origem_id = ?1 ORDER BY criado_em")
        .unwrap()
        .query_map([origem], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// Movimentos do Caixa geral ligados à operação: (conta, tipo, valor).
fn movimentos_da_conta(l: &Loja, origem: &str) -> Vec<(String, String, i64)> {
    l.conexao()
        .prepare("SELECT conta_id, tipo, valor FROM movimento_conta WHERE origem_id = ?1 ORDER BY criado_em")
        .unwrap()
        .query_map([origem], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn auditoria_tem(l: &Loja, acao: &str, trecho: &str) -> bool {
    l.numero(&format!(
        "SELECT COUNT(*) FROM auditoria WHERE acao = '{acao}' AND detalhes LIKE '%{}%'",
        trecho.replace('\'', "''")
    )) > 0
}

/// Loja com o caixa de "ontem" fechado certo: R$ 50 de troco inicial.
fn caixa_de_ontem(nome: &str) -> (Loja, String, String) {
    let mut l = Loja::nova(nome);
    let p = l.peca("Filtro", "60", "100", "10");
    l.abrir_caixa();
    let ontem = sessao_aberta(&mut l);
    (l, ontem, p)
}

// ---------------------------------------------------------------------------
// 1. Cancelar a venda.
// ---------------------------------------------------------------------------

#[test]
fn d14_cancelar_venda_de_caixa_fechado_sai_da_gaveta_de_hoje() {
    let (mut l, ontem, p) = caixa_de_ontem("d14-cancelar");
    // Dinheiro 120, troco 20: ficaram 100 na gaveta.
    let v = id(&l.vender(&[(&p, "1")], &[("dinheiro", "120")]));
    l.ok("caixa_fechar", json!({ "valor": "150", "motivo": "" }));
    let antes = fechamento(&mut l, &ontem);
    assert_eq!((antes["esperado"].as_i64(), antes["diferenca"].as_i64()), (Some(15000), Some(0)));

    // Hoje: caixa aberto com R$ 200.
    l.ok("caixa_abrir", json!({ "valor": "200" }));
    let hoje = sessao_aberta(&mut l);
    l.ok("venda_cancelar", json!({ "id": v, "motivo": "cliente voltou" }));

    assert_eq!(fechamento(&mut l, &ontem), antes, "o fechamento de ontem não muda");
    let r = l.ok("caixa_resumo", json!({ "id": hoje }));
    assert_eq!(centavos(r["esperado"].as_str().unwrap()), 10000, "200 − 100 devolvidos");
    assert_eq!(movimentos_da_gaveta(&l, &v), vec![(hoje.clone(), "sangria".into(), 10000)]);
    assert!(movimentos_da_conta(&l, &v).is_empty());
    assert!(auditoria_tem(&l, "venda.cancelar", "já estava fechado"));
    assert!(auditoria_tem(&l, "caixa.sangria", "Cancelamento da venda"));
    // A venda continua cancelada para tudo o mais (estoque de volta).
    assert_eq!(l.estoque(&p), "10");
    // Repetir não devolve de novo.
    let msg = l.erro("venda_cancelar", json!({ "id": v, "motivo": "de novo" }));
    assert!(msg.contains("já foi cancelada"), "{msg}");
    assert_eq!(movimentos_da_gaveta(&l, &v).len(), 1, "sem movimento duplicado");
}

#[test]
fn d14_cancelar_venda_de_caixa_fechado_sem_caixa_aberto_vai_para_o_caixa_geral() {
    let (mut l, ontem, p) = caixa_de_ontem("d14-cancelar-geral");
    let v = id(&l.vender(&[(&p, "1")], &[("dinheiro", "100")]));
    l.ok("caixa_fechar", json!({ "valor": "150", "motivo": "" }));
    let antes = fechamento(&mut l, &ontem);
    let geral = |l: &mut Loja| {
        let c = l.ok("contas_loja", json!({ "quais": "" }));
        centavos(c.as_array().unwrap().iter().find(|c| c["id"] == "caixa-geral").unwrap()["saldo"].as_str().unwrap())
    };
    let geral_antes = geral(&mut l);
    l.ok("venda_cancelar", json!({ "id": v, "motivo": "cliente voltou" }));
    assert_eq!(fechamento(&mut l, &ontem), antes, "o fechamento de ontem não muda");
    assert_eq!(movimentos_da_conta(&l, &v), vec![("caixa-geral".into(), "caixa_fechado".into(), -10000)]);
    assert!(movimentos_da_gaveta(&l, &v).is_empty());
    assert_eq!(geral(&mut l), geral_antes - 10000);
    assert!(auditoria_tem(&l, "caixa.caixa_fechado", "Caixa geral"));
    // Fluxo de caixa: a venda já sai do dia dela; o acerto não conta de novo.
    let f = l.ok("fluxo_caixa", json!({ "de": "", "ate": "" }));
    assert_eq!(centavos(f["saidas"].as_str().unwrap()), 0, "{f}");
}

#[test]
fn d14_cancelar_venda_em_pix_de_caixa_fechado_nao_mexe_em_dinheiro() {
    let (mut l, ontem, p) = caixa_de_ontem("d14-cancelar-pix");
    let v = id(&l.vender(&[(&p, "1")], &[("pix", "100")]));
    l.ok("caixa_fechar", json!({ "valor": "50", "motivo": "" }));
    let antes = fechamento(&mut l, &ontem);
    l.abrir_caixa();
    l.ok("venda_cancelar", json!({ "id": v, "motivo": "cliente voltou" }));
    assert_eq!(fechamento(&mut l, &ontem), antes, "o Pix do fechamento de ontem não muda");
    assert!(movimentos_da_gaveta(&l, &v).is_empty() && movimentos_da_conta(&l, &v).is_empty());
}

#[test]
fn d14_gaveta_sem_dinheiro_para_devolver_recusa_tudo() {
    let (mut l, ontem, p) = caixa_de_ontem("d14-sem-dinheiro");
    let v = id(&l.vender(&[(&p, "1")], &[("dinheiro", "100")]));
    l.ok("caixa_fechar", json!({ "valor": "150", "motivo": "" }));
    let antes = fechamento(&mut l, &ontem);
    l.ok("caixa_abrir", json!({ "valor": "30" }));
    let msg = l.erro("venda_cancelar", json!({ "id": v, "motivo": "cliente voltou" }));
    assert!(msg.contains("gaveta"), "{msg}");
    // Nada mudou: a venda continua concluída, o estoque e os fechamentos também.
    assert_eq!(l.ok("venda_abrir", json!({ "id": v }))["situacao"], "concluida");
    assert_eq!(l.estoque(&p), "9");
    assert_eq!(fechamento(&mut l, &ontem), antes);
    assert!(movimentos_da_gaveta(&l, &v).is_empty());
}

/// Venda do caixa que ainda está aberto: como sempre (sai do total dele).
#[test]
fn d14_cancelar_venda_do_caixa_aberto_continua_igual() {
    let (mut l, hoje, p) = caixa_de_ontem("d14-aberto");
    let v = id(&l.vender(&[(&p, "1")], &[("dinheiro", "100")]));
    l.ok("venda_cancelar", json!({ "id": v, "motivo": "cliente desistiu" }));
    let r = fechamento(&mut l, &hoje);
    assert_eq!(
        (r["esperado"].as_i64(), r["vendas"].as_i64(), r["canceladas"].as_i64()),
        (Some(5000), Some(0), Some(1))
    );
    assert!(movimentos_da_gaveta(&l, &v).is_empty(), "sem movimento a mais");
}

// ---------------------------------------------------------------------------
// 2. Estornar o recebimento.
// ---------------------------------------------------------------------------

#[test]
fn d14_estornar_recebimento_de_caixa_fechado() {
    let (mut l, ontem, p) = caixa_de_ontem("d14-estorno");
    let ze = l.pessoa("cliente", "Zé");
    l.vender_com(&[(&p, "1")], &[("crediario", "100")], json!({ "clienteId": ze, "parcelas": 1 }));
    let rec = l.ok("receber_baixar", json!({ "clienteId": ze, "contas": [], "valor": "40", "forma": "dinheiro" }));
    let rid = rec["id"].as_str().unwrap().to_string();
    l.ok("caixa_fechar", json!({ "valor": "90", "motivo": "" }));
    let antes = fechamento(&mut l, &ontem);
    assert_eq!((antes["esperado"].as_i64(), antes["recebido"].as_i64()), (Some(9000), Some(4000)));

    // Com a gaveta de hoje aberta.
    l.ok("caixa_abrir", json!({ "valor": "100" }));
    let hoje = sessao_aberta(&mut l);
    l.ok("recebimento_estornar", json!({ "id": rid, "motivo": "lançado no cliente errado" }));
    assert_eq!(fechamento(&mut l, &ontem), antes, "o fechamento de ontem não muda");
    assert_eq!(movimentos_da_gaveta(&l, &rid), vec![(hoje.clone(), "sangria".into(), 4000)]);
    let r = l.ok("caixa_resumo", json!({ "id": hoje }));
    assert_eq!(centavos(r["esperado"].as_str().unwrap()), 6000);
    // A dívida volta inteira.
    let s = l.ok("situacao_cliente", json!({ "id": ze }));
    assert_eq!(centavos(s["emAberto"].as_str().unwrap()), 10000);
    assert!(auditoria_tem(&l, "receber.estornar", "já estava fechado"));
    let msg = l.erro("recebimento_estornar", json!({ "id": rid, "motivo": "de novo" }));
    assert!(msg.contains("já foi estornado"), "{msg}");
    assert_eq!(movimentos_da_gaveta(&l, &rid).len(), 1, "sem movimento duplicado");
}

#[test]
fn d14_estornar_recebimento_de_caixa_fechado_sem_caixa_aberto() {
    let (mut l, ontem, p) = caixa_de_ontem("d14-estorno-geral");
    let ze = l.pessoa("cliente", "Zé");
    l.vender_com(&[(&p, "1")], &[("crediario", "100")], json!({ "clienteId": ze, "parcelas": 1 }));
    let rec = l.ok("receber_baixar", json!({ "clienteId": ze, "contas": [], "valor": "40", "forma": "dinheiro" }));
    let rid = rec["id"].as_str().unwrap().to_string();
    l.ok("caixa_fechar", json!({ "valor": "90", "motivo": "" }));
    let antes = fechamento(&mut l, &ontem);
    l.ok("recebimento_estornar", json!({ "id": rid, "motivo": "lançado errado" }));
    assert_eq!(fechamento(&mut l, &ontem), antes);
    assert_eq!(movimentos_da_conta(&l, &rid), vec![("caixa-geral".into(), "caixa_fechado".into(), -4000)]);
}

// ---------------------------------------------------------------------------
// 3. Desfazer a devolução.
// ---------------------------------------------------------------------------

#[test]
fn d14_desfazer_devolucao_de_caixa_fechado_entra_na_gaveta_de_hoje() {
    let (mut l, ontem, p) = caixa_de_ontem("d14-devolucao");
    let v = id(&l.vender(&[(&p, "2")], &[("dinheiro", "200")]));
    let d = l.devolver(&v, 0, "1", "dinheiro", false);
    let did = d["id"].as_str().unwrap().to_string();
    l.ok("caixa_fechar", json!({ "valor": "150", "motivo": "" }));
    let antes = fechamento(&mut l, &ontem);
    assert_eq!((antes["esperado"].as_i64(), antes["devolvido"].as_i64()), (Some(15000), Some(10000)));

    l.ok("caixa_abrir", json!({ "valor": "20" }));
    let hoje = sessao_aberta(&mut l);
    l.ok("devolucao_desfazer", json!({ "id": did, "motivo": "cliente ficou com a peça" }));
    assert_eq!(fechamento(&mut l, &ontem), antes, "o fechamento de ontem não muda");
    assert_eq!(movimentos_da_gaveta(&l, &did), vec![(hoje.clone(), "suprimento".into(), 10000)]);
    let r = l.ok("caixa_resumo", json!({ "id": hoje }));
    assert_eq!(centavos(r["esperado"].as_str().unwrap()), 12000);
    assert_eq!(l.estoque(&p), "8", "a peça sai de novo do estoque");
    assert!(auditoria_tem(&l, "venda.devolucao_desfeita", "já estava fechado"));
    let msg = l.erro("devolucao_desfazer", json!({ "id": did, "motivo": "de novo" }));
    assert!(msg.contains("já foi desfeita"), "{msg}");
    assert_eq!(movimentos_da_gaveta(&l, &did).len(), 1, "sem movimento duplicado");
}

#[test]
fn d14_desfazer_devolucao_de_caixa_fechado_sem_caixa_aberto() {
    let (mut l, ontem, p) = caixa_de_ontem("d14-devolucao-geral");
    let v = id(&l.vender(&[(&p, "2")], &[("dinheiro", "200")]));
    let d = l.devolver(&v, 0, "1", "dinheiro", false);
    let did = d["id"].as_str().unwrap().to_string();
    l.ok("caixa_fechar", json!({ "valor": "150", "motivo": "" }));
    let antes = fechamento(&mut l, &ontem);
    l.ok("devolucao_desfazer", json!({ "id": did, "motivo": "cliente ficou com a peça" }));
    assert_eq!(fechamento(&mut l, &ontem), antes);
    assert_eq!(movimentos_da_conta(&l, &did), vec![("caixa-geral".into(), "caixa_fechado".into(), 10000)]);
}

/// Depois de reabrir o Rustinho (e de um backup restaurado, que é o mesmo
/// banco), o fechamento antigo continua o mesmo.
#[test]
fn d14_fechamento_antigo_igual_depois_de_reabrir() {
    let (mut l, ontem, p) = caixa_de_ontem("d14-reabrir");
    let v = id(&l.vender(&[(&p, "1")], &[("dinheiro", "100")]));
    l.ok("caixa_fechar", json!({ "valor": "150", "motivo": "" }));
    let antes = fechamento(&mut l, &ontem);
    l.ok("venda_cancelar", json!({ "id": v, "motivo": "cliente voltou" }));
    l.reabrir();
    l.entrar("Ana", SENHA_ANA);
    assert_eq!(fechamento(&mut l, &ontem), antes);
}

/// Só à mão (e para `app/testes/telas_d14_portatil.py`): a loja de teste com
/// um caixa já fechado que tem uma venda em dinheiro (R$ 100,00) e um caixa
/// novo aberto com R$ 300,00. Os ids vão em `d14.json`, na pasta.
/// `RUSTINHO_LOJA_TESTE=/pasta cargo test -p rustinho-nucleo --test estavel -- --ignored --exact a1_auditoria::montar_loja_d14`
#[test]
#[ignore]
fn montar_loja_d14() {
    let Some(pasta) = std::env::var_os("RUSTINHO_LOJA_TESTE").map(std::path::PathBuf::from) else { return };
    let mut l = Loja::abrir_em(&pasta, false);
    l.ok(
        "configurar_inicio",
        json!({ "loja": "Loja Teste Auto Peças", "caixa": "Caixa 1", "nome": "Ana",
                "senha": SENHA_ANA, "senha2": SENHA_ANA }),
    );
    let povo = povoar(&mut l);
    let ontem = sessao_aberta(&mut l);
    let v = id(&l.vender(&[(&povo.filtro, "1")], &[("dinheiro", "100")]));
    let r = l.ok("caixa_resumo", json!({ "id": ontem }));
    let esperado = r["esperado"].as_str().unwrap().to_string();
    l.ok("caixa_fechar", json!({ "valor": esperado, "motivo": "" }));
    let antes = fechamento(&mut l, &ontem);
    l.ok("caixa_abrir", json!({ "valor": "300" }));
    let hoje = sessao_aberta(&mut l);
    std::fs::write(
        pasta.join("d14.json"),
        json!({ "venda": v, "fechado": ontem, "aberto": hoje, "fechamento": antes }).to_string(),
    )
    .unwrap();
}
