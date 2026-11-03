//! Prioridade 1 — Crediário: pagamentos parciais, quitação, atraso (multa e
//! juros), estorno, devolução abatendo a dívida e cancelamento.

use serde_json::{json, Value};

use crate::comum::*;

fn situacao(l: &mut Loja, cliente: &str) -> Value {
    l.ok("receber_cliente", json!({ "id": cliente, "todas": true }))
}

fn em_aberto(l: &mut Loja, cliente: &str) -> i64 {
    centavos(situacao(l, cliente)["situacao"]["emAberto"].as_str().unwrap())
}

/// Saldos das parcelas, em ordem (centavos).
fn saldos(l: &mut Loja, cliente: &str) -> Vec<i64> {
    let c = situacao(l, cliente);
    let mut v: Vec<(String, i64)> = c["parcelas"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["situacao"] != "cancelada")
        .map(|p| (p["parcela"].as_str().unwrap().to_string(), centavos(p["saldo"].as_str().unwrap())))
        .collect();
    v.sort();
    v.into_iter().map(|(_, s)| s).collect()
}

/// Dinheiro do caixa aberto (o que deve ter na gaveta), em centavos.
fn gaveta(l: &mut Loja) -> i64 {
    let e = l.ok("caixa_estado", json!({}));
    let r = &e["resumo"];
    for campo in ["esperado", "dinheiroEsperado", "saldo", "emDinheiro"] {
        if let Some(t) = r[campo].as_str() {
            return centavos(t);
        }
    }
    panic!("não achei o dinheiro esperado no resumo do caixa: {r}");
}

fn venda_no_crediario(l: &mut Loja, cliente: &str, peca: &str, qtd: &str, total: &str, parcelas: i64) -> String {
    id(&l.vender_com(&[(peca, qtd)], &[("crediario", total)], json!({ "clienteId": cliente, "parcelas": parcelas })))
}

/// T3.1 — Parcial, parcial que pega duas parcelas, quitação; o caixa recebe o dinheiro.
#[test]
fn t3_1_parciais_e_quitacao() {
    let mut l = Loja::nova("t3-1");
    let ze = l.pessoa("cliente", "Zé");
    let p = l.peca("Amortecedor", "100", "100", "10");
    l.abrir_caixa();
    let g0 = gaveta(&mut l);
    venda_no_crediario(&mut l, &ze, &p, "3", "300", 3);
    assert_eq!(em_aberto(&mut l, &ze), 30000);
    assert_eq!(saldos(&mut l, &ze), vec![10000, 10000, 10000]);
    assert_eq!(gaveta(&mut l), g0, "venda no crediário não põe dinheiro na gaveta");

    let r = l.ok("receber_baixar", json!({ "clienteId": ze, "contas": [], "valor": "50", "forma": "dinheiro" }));
    assert_eq!(centavos(r["emAberto"].as_str().unwrap()), 25000, "{r}");
    assert_eq!(saldos(&mut l, &ze), vec![5000, 10000, 10000], "abate da mais antiga");
    assert_eq!(gaveta(&mut l), g0 + 5000);

    l.ok("receber_baixar", json!({ "clienteId": ze, "contas": [], "valor": "150", "forma": "pix" }));
    assert_eq!(saldos(&mut l, &ze), vec![0, 0, 10000]);
    assert_eq!(gaveta(&mut l), g0 + 5000, "pix não entra na gaveta");

    // Quitação (vazio = tudo); pagou a mais em dinheiro: troco.
    let r = l.ok("receber_baixar", json!({ "clienteId": ze, "contas": [], "valor": "120", "forma": "dinheiro" }));
    assert_eq!(r["troco"], "20,00", "{r}");
    assert_eq!(em_aberto(&mut l, &ze), 0);
    assert_eq!(gaveta(&mut l), g0 + 5000 + 10000, "entra o pago, não o entregue");
    // Nada mais a receber.
    l.erro("receber_baixar", json!({ "clienteId": ze, "contas": [], "valor": "10", "forma": "dinheiro" }));
}

/// T3.2 — Atraso: multa (uma vez) e juros por mês, conferidos à mão.
#[test]
fn t3_2_atraso_multa_e_juros() {
    let mut l = Loja::nova("t3-2");
    let ze = l.pessoa("cliente", "Zé");
    let p = l.peca("Bomba d'água", "100", "100", "10");
    l.abrir_caixa();
    // 2% de multa, 1% ao mês, sem tolerância.
    l.executar(
        "INSERT INTO config (chave, valor) VALUES ('crediario_multa', '200'), ('crediario_juros', '100'), ('crediario_carencia', '0')
         ON CONFLICT (chave) DO UPDATE SET valor = excluded.valor",
    );
    venda_no_crediario(&mut l, &ze, &p, "2", "200", 2);
    // A 1ª parcela venceu há 40 dias.
    l.executar(
        "UPDATE conta_receber SET vencimento = date('now', 'localtime', '-40 day')
          WHERE id = (SELECT id FROM conta_receber ORDER BY vencimento LIMIT 1)",
    );
    let c = situacao(&mut l, &ze);
    let vencida =
        c["parcelas"].as_array().unwrap().iter().find(|x| x["diasAtraso"] == 40).expect("parcela de 40 dias").clone();
    // À mão: 100,00 × 2% = 2,00 de multa; 100,00 × 1% × 40/30 = 1,333… de juros; total 3,33.
    assert_eq!(vencida["juros"], "3,33", "{vencida}");
    assert_eq!(c["situacao"]["vencido"], "100,00", "{c}");
    let calc = l.ok(
        "receber_calcular",
        json!({ "clienteId": ze, "contas": [vencida["id"]], "valor": "", "forma": "dinheiro" }),
    );
    assert_eq!(calc["juros"], "3,33", "{calc}");
    assert_eq!(calc["total"], "103,33", "{calc}");
    // Recebe a vencida com os juros.
    let r =
        l.ok("receber_baixar", json!({ "clienteId": ze, "contas": [vencida["id"]], "valor": "", "forma": "dinheiro" }));
    assert_eq!((r["valor"].as_str(), r["juros"].as_str()), (Some("103,33"), Some("3,33")), "{r}");
    assert_eq!(em_aberto(&mut l, &ze), 10000, "sobrou a 2ª, sem juros");
    // Os juros recebidos aparecem como receita financeira na DRE, não como venda.
    let d = dre_do_mes(&mut l);
    assert_eq!(linha_dre(&d, "receitas_financeiras"), 333, "juros recebidos");
    assert_eq!(linha_dre(&d, "receita_bruta"), 20000, "a venda continua 200,00 (juros não é venda)");
}

/// T3.3 — Estorno de um recebimento: a dívida volta, a gaveta também; só o gerente estorna.
#[test]
fn t3_3_estorno() {
    let mut l = Loja::nova("t3-3");
    let ze = l.pessoa("cliente", "Zé");
    let p = l.peca("Disco de freio", "50", "100", "10");
    l.usuario_caixa("Bia", SENHA_BIA);
    l.abrir_caixa();
    let g0 = gaveta(&mut l);
    venda_no_crediario(&mut l, &ze, &p, "2", "200", 2);
    let r = l.ok("receber_baixar", json!({ "clienteId": ze, "contas": [], "valor": "130", "forma": "dinheiro" }));
    let recibo = r["id"].as_str().unwrap().to_string();
    assert_eq!(saldos(&mut l, &ze), vec![0, 7000]);
    assert_eq!(gaveta(&mut l), g0 + 13000);

    // O caixa não estorna.
    l.entrar("Bia", SENHA_BIA);
    l.abrir_caixa();
    let msg = l.erro("recebimento_estornar", json!({ "id": recibo, "motivo": "errado" }));
    assert!(msg.contains("gerente"), "{msg}");
    assert_eq!(saldos(&mut l, &ze), vec![0, 7000], "nada mudou");

    l.entrar("Ana", SENHA_ANA);
    l.erro("recebimento_estornar", json!({ "id": recibo, "motivo": "" }));
    l.ok("recebimento_estornar", json!({ "id": recibo, "motivo": "recebi do cliente errado" }));
    assert_eq!(saldos(&mut l, &ze), vec![10000, 10000], "a dívida voltou inteira");
    assert_eq!(gaveta(&mut l), g0, "o dinheiro saiu da gaveta");
    // Estornar duas vezes não devolve de novo.
    l.erro("recebimento_estornar", json!({ "id": recibo, "motivo": "de novo" }));
    assert_eq!(em_aberto(&mut l, &ze), 20000);
    // O recibo mostra que foi estornado.
    let rec = l.ok("receber_cliente", json!({ "id": ze, "todas": true }));
    let achado = rec["recebimentos"].as_array().unwrap().iter().find(|x| x["id"] == recibo.as_str()).cloned();
    assert!(achado.is_some_and(|x| x["estornado"] == true), "{rec}");
}

/// T3.4 — Devolução de uma venda no crediário abate a dívida; venda com
/// pagamento recebido não é cancelada sem antes estornar.
#[test]
fn t3_4_devolucao_e_cancelamento() {
    let mut l = Loja::nova("t3-4");
    let ze = l.pessoa("cliente", "Zé");
    let p = l.peca("Vela", "10", "50", "10");
    l.abrir_caixa();
    let v = venda_no_crediario(&mut l, &ze, &p, "4", "200", 2);
    assert_eq!(em_aberto(&mut l, &ze), 20000);
    l.devolver(&v, 0, "1", "crediario", false);
    assert_eq!(em_aberto(&mut l, &ze), 15000, "devolveu 1 de 50,00");
    assert_eq!(l.estoque_pelos_movimentos(&p), 7);

    // Outra venda no crediário, com parte recebida.
    let v2 = venda_no_crediario(&mut l, &ze, &p, "2", "100", 1);
    let c = situacao(&mut l, &ze);
    let da_v2 = c["parcelas"].as_array().unwrap().iter().find(|x| x["vendaId"] == v2.as_str()).unwrap()["id"].clone();
    let r = l.ok("receber_baixar", json!({ "clienteId": ze, "contas": [da_v2], "valor": "20", "forma": "dinheiro" }));
    let antes = em_aberto(&mut l, &ze);
    let resp = l.chamar("venda_cancelar", json!({ "id": v2, "motivo": "teste" }));
    if resp["ok"] == true {
        // Se cancelar, a dívida da venda cancelada não pode continuar aberta,
        // e o que já foi pago não pode sumir.
        let depois = em_aberto(&mut l, &ze);
        panic!("cancelou uma venda com recebimento: em aberto {antes} → {depois}; recibo {r}");
    }
    assert_eq!(em_aberto(&mut l, &ze), antes, "recusou e nada mudou");
    assert_eq!(l.estoque_pelos_movimentos(&p), 5);
}

/// T3.5 — Crediário só com cliente cadastrado; caixa fechado não recebe.
#[test]
fn t3_5_regras_do_balcao() {
    let mut l = Loja::nova("t3-5");
    let ze = l.pessoa("cliente", "Zé");
    let p = l.peca("Junta", "5", "20", "10");
    l.abrir_caixa();
    let r = l.chamar(
        "venda_concluir",
        json!({ "itens": [{ "produtoId": p, "quantidade": "1" }], "tabela": 1,
                "pagamentos": [{ "forma": "crediario", "valor": "20" }] }),
    );
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(l.estoque_pelos_movimentos(&p), 10, "venda recusada não baixa");
    venda_no_crediario(&mut l, &ze, &p, "1", "20", 1);
    let e = l.ok("caixa_estado", json!({}));
    let _ = e;
    l.ok("caixa_fechar", json!({ "valor": "50", "motivo": "" }));
    let msg = l.erro("receber_baixar", json!({ "clienteId": ze, "contas": [], "valor": "20", "forma": "dinheiro" }));
    assert!(msg.to_lowercase().contains("caixa"), "{msg}");
    assert_eq!(em_aberto(&mut l, &ze), 2000);
}
