//! Prioridade 1 — DRE gerencial conferida com contas feitas à mão: descontos,
//! devoluções (boa e com defeito), cancelamento, peça sem custo, tributo e
//! despesas lançadas depois (no mês certo e no mês anterior).
//!
//! Conta à mão (mês atual):
//!
//! | Venda | O quê | Bruto | Desconto | CMV |
//! |---|---|---|---|---|
//! | V1 | 2 × A (custo 60, preço 100), dinheiro | 200,00 | — | 120,00 |
//! | V2 | 1 × B (custo 40, preço 90), desconto 10,00, pix | 90,00 | 10,00 | 40,00 |
//! | V3 | 1 × C (sem custo, preço 30) | 30,00 | — | (falta) |
//! | V4 | 1 × A, cancelada no mesmo mês | 100,00 | — | 60 − 60 = 0 |
//! | V5 | 2 × D (custo 20, preço 50); volta 1 boa e 1 com defeito | 100,00 | — | 40 − 20 = 20,00 |
//!
//! (Na DRE as deduções e despesas aparecem com sinal negativo.)
//!
//! Receita bruta 520,00 · descontos 10,00 · devoluções 100,00 · cancelamentos
//! 100,00 · DAS 25,00 → receita líquida 285,00 · CMV 180,00 → lucro bruto
//! 105,00 · aluguel 400,00 → resultado −295,00 (incompleto: peça sem custo).
//! Vendas líquidas (concluídas e não canceladas, com desconto): 200 + 80 + 30
//! + 100 = 410,00 em 4 vendas; ticket 102,50.

use serde_json::json;

use crate::comum::*;

fn mes(l: &Loja, desloca: &str) -> String {
    l.conexao()
        .query_row(&format!("SELECT strftime('%m/%Y', 'now', 'localtime', 'start of month', '{desloca}')"), [], |x| {
            x.get(0)
        })
        .unwrap()
}

fn despesa(l: &mut Loja, categoria: &str, descricao: &str, valor: &str, competencia: &str) -> String {
    let ids = l.ok(
        "pagar_lancar",
        json!({ "categoriaId": categoria, "descricao": descricao, "valor": valor, "parcelas": 1,
                "competencia": competencia }),
    );
    ids[0].as_str().unwrap().to_string()
}

#[test]
fn t4_1_dre_do_mes_confere_com_a_conta_a_mao() {
    let mut l = Loja::nova("t4-1");
    let a = l.peca("Peça A", "60", "100", "10");
    let b = l.peca("Peça B", "40", "90", "10");
    let c = l.peca("Peça C", "", "30", "10");
    let d = l.peca("Peça D", "20", "50", "10");
    l.abrir_caixa();
    let este = mes(&l, "+0 day");
    let anterior = mes(&l, "-1 month");

    l.vender(&[(&a, "2")], &[("dinheiro", "200")]);
    l.vender_com(&[(&b, "1")], &[("pix", "80")], json!({ "desconto": "10" }));
    l.vender(&[(&c, "1")], &[("dinheiro", "30")]);
    let v4 = id(&l.vender(&[(&a, "1")], &[("dinheiro", "100")]));
    l.ok("venda_cancelar", json!({ "id": v4, "motivo": "teste" }));
    let v5 = id(&l.vender(&[(&d, "2")], &[("dinheiro", "100")]));
    l.devolver(&v5, 0, "1", "dinheiro", false);
    l.devolver(&v5, 0, "1", "dinheiro", true);
    despesa(&mut l, "cat-impostos", "DAS", "25", &este);
    let aluguel = despesa(&mut l, "cat-aluguel", "Aluguel", "400", &este);

    let r = dre_do_mes(&mut l);
    let esperado = [
        ("receita_bruta", 52000),
        ("descontos", -1000),
        ("devolucoes", -10000),
        ("cancelamentos", -10000),
        ("tributos", -2500),
        ("receita_liquida", 28500),
        ("cmv", -18000),
        ("lucro_bruto", 10500),
        ("fixas", -40000),
        ("resultado_operacional", -29500),
        ("resultado", -29500),
    ];
    for (linha, v) in esperado {
        assert_eq!(linha_dre(&r, linha), v, "linha {linha}: {}", r["linhas"]);
    }
    assert_ne!(r["estado"], "completo", "com peça sem custo, a DRE não pode se dizer completa: {}", r["estado"]);
    let pend = r["pendencias"].to_string();
    assert!(pend.to_lowercase().contains("custo"), "pendência da peça sem custo: {pend}");
    let com = &r["comercial"];
    assert_eq!(com["vendasLiquidas"], "410,00", "{com}");
    assert_eq!(com["quantidade"], 4, "{com}");
    assert_eq!(com["ticket"], "102,50", "{com}");
    assert_eq!(com["canceladas"], 1, "{com}");

    // Pagar o aluguel não cria outra despesa.
    l.ok("pagar_pagar", json!({ "contaId": aluguel, "valor": "", "forma": "pix", "origem": "caixa-geral" }));
    let r2 = dre_do_mes(&mut l);
    assert_eq!(linha_dre(&r2, "fixas"), -40000, "pagar não é despesa nova");
    assert_eq!(linha_dre(&r2, "resultado"), -29500);

    // Despesa do mês passado lançada hoje: não muda este mês, entra no anterior.
    let antes_anterior =
        l.ok("dre_apurar", json!({ "periodo": "mes_anterior", "de": "", "ate": "", "vendedorId": "" }));
    let fixas_ant = linha_dre(&antes_anterior, "fixas");
    despesa(&mut l, "cat-energia", "Luz do mês passado", "90", &anterior);
    let r3 = dre_do_mes(&mut l);
    assert_eq!(linha_dre(&r3, "fixas"), -40000, "a luz do mês passado não entra neste mês");
    let ant = l.ok("dre_apurar", json!({ "periodo": "mes_anterior", "de": "", "ate": "", "vendedorId": "" }));
    assert_eq!(linha_dre(&ant, "fixas"), fixas_ant - 9000, "entra no mês anterior");

    // Despesa deste mês lançada depois: entra (e o resultado cai junto).
    despesa(&mut l, "cat-telefone", "Internet", "60", &este);
    let r4 = dre_do_mes(&mut l);
    assert_eq!(linha_dre(&r4, "fixas"), -46000);
    assert_eq!(linha_dre(&r4, "resultado"), -35500);

    // Custo cadastrado depois na peça C não muda a venda já feita (custo histórico).
    let mut f = l.ok("produto_abrir", json!({ "id": c }));
    f["custo"] = json!("12");
    l.ok("produto_salvar", f);
    let r5 = dre_do_mes(&mut l);
    assert_eq!(linha_dre(&r5, "cmv"), -18000, "a venda de C continua sem custo");

    // Desfazer a devolução boa: volta a receita e o custo (na data de hoje).
    let boa: String = l
        .conexao()
        .query_row(
            "SELECT d.id FROM devolucao d JOIN devolucao_item i ON i.devolucao_id = d.id
              WHERE d.venda_id = ?1 AND i.volta_estoque = 1",
            [&v5],
            |x| x.get(0),
        )
        .unwrap();
    l.ok("devolucao_desfazer", json!({ "id": boa, "motivo": "lançada errado" }));
    let r6 = dre_do_mes(&mut l);
    assert_eq!(linha_dre(&r6, "devolucoes"), -5000, "sobrou a devolução com defeito");
    assert_eq!(linha_dre(&r6, "cmv"), -20000, "o custo da boa voltou ao CMV");
}

/// T4.2 — O caixa não vê a DRE nem o lucro, por nenhuma porta.
#[test]
fn t4_2_dre_so_para_quem_pode() {
    let mut l = Loja::nova("t4-2");
    let a = l.peca("Peça A", "60", "100", "10");
    l.abrir_caixa();
    l.vender(&[(&a, "1")], &[("dinheiro", "100")]);
    l.usuario_caixa("Bia", SENHA_BIA);
    l.entrar("Bia", SENHA_BIA);
    for c in ["dre_apurar", "dre_resumo", "dre_indicadores", "dre_serie", "dre_detalhe", "dre_despesas", "dre_metas"] {
        let m = l.erro(c, json!({}));
        assert!(m.contains("gerente"), "{c}: {m}");
    }
    let m = l.erro("impressao", json!({ "tipo": "dre", "id": "" }));
    assert!(m.contains("gerente"), "{m}");
    let p = l.ok("painel_resumo", json!({}));
    assert_eq!((p["lucroMes"].as_str(), p["margemMes"].as_str()), (Some(""), Some("")), "{p}");
}

/// T4.3 — O ticket médio é o mesmo no Painel e na DRE, arredondado
/// (380,00 ÷ 3 = 126,666… → 126,67), D-11.
#[test]
fn t4_3_ticket_medio_arredondado_igual_nas_telas() {
    let mut l = Loja::nova("t4-3");
    let a = l.peca("Peça A", "10", "100", "10");
    let b = l.peca("Peça B", "10", "180", "10");
    l.abrir_caixa();
    l.vender(&[(&a, "1")], &[("dinheiro", "100")]);
    l.vender(&[(&a, "1")], &[("dinheiro", "100")]);
    l.vender(&[(&b, "1")], &[("dinheiro", "180")]);
    let p = l.ok("painel_resumo", json!({}));
    assert_eq!((p["hoje"]["ticket"].as_str(), p["mes"]["ticket"].as_str()), (Some("126,67"), Some("126,67")), "{p}");
    let r = l.ok("dre_resumo", json!({}));
    assert_eq!(r["ticket"], "126,67", "{r}");
}
