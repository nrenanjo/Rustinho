//! Prioridade 1 — Estoque depois de vendas, compras, devoluções, estornos,
//! cancelamentos, ajustes e operações ao mesmo tempo (a troca de dados com
//! outro caixa gravando enquanto este vende).

use rustinho_nucleo::banco::Banco;
use rustinho_nucleo::{caixa, sync, vendas};
use serde_json::json;

use crate::comum::*;

/// Confere o estoque pela tela e pela soma dos movimentos.
fn confere(l: &mut Loja, peca: &str, esperado: i64, onde: &str) {
    assert_eq!(l.estoque_pelos_movimentos(peca), esperado, "{onde}: soma dos movimentos");
    assert_eq!(l.estoque(peca), esperado.to_string(), "{onde}: o que a tela mostra");
}

/// T2.1 — Venda, cancelamento, devolução (boa e com defeito) e desfazer a devolução.
#[test]
fn t2_1_venda_cancelamento_e_devolucao() {
    let mut l = Loja::nova("t2-1");
    let p = l.peca("Filtro", "60", "100", "10");
    l.abrir_caixa();
    confere(&mut l, &p, 10, "estoque inicial");

    let v1 = id(&l.vender(&[(&p, "3")], &[("dinheiro", "300")]));
    confere(&mut l, &p, 7, "vendeu 3");

    let v2 = id(&l.vender(&[(&p, "2")], &[("pix", "200")]));
    l.ok("venda_cancelar", json!({ "id": v2, "motivo": "cliente desistiu" }));
    confere(&mut l, &p, 7, "cancelou a venda de 2");
    // Cancelar de novo não devolve outra vez.
    l.erro("venda_cancelar", json!({ "id": v2, "motivo": "de novo" }));
    confere(&mut l, &p, 7, "cancelar duas vezes");

    // Devolve 1 boa (volta para a prateleira) e 1 com defeito (não volta).
    l.devolver(&v1, 0, "1", "dinheiro", false);
    confere(&mut l, &p, 8, "devolveu 1 boa");
    let r = l.devolver(&v1, 0, "1", "vale", true);
    confere(&mut l, &p, 8, "devolveu 1 com defeito");
    // Não dá para devolver mais do que foi vendido (sobrou 1).
    let d = l.ok("devolucao_venda", json!({ "id": v1 }));
    assert_eq!(d["itens"][0]["pode"], "1", "{d}");
    let item = d["itens"][0]["vendaItemId"].clone();
    l.erro(
        "devolucao_gravar",
        json!({ "vendaId": v1, "destino": "dinheiro", "motivo": "x",
                "itens": [{ "vendaItemId": item, "quantidade": "2", "defeito": false }] }),
    );
    confere(&mut l, &p, 8, "devolver a mais é recusado");

    // Desfazer a devolução com defeito: nada muda no estoque (ela não tinha voltado).
    let lista = l.ok("devolucoes_listar", json!({ "texto": "" }));
    let com_defeito = lista
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["valeId"].is_string() || x["id"] == r["id"])
        .map(|x| x["id"].as_str().unwrap().to_string())
        .unwrap_or_else(|| r["id"].as_str().unwrap().to_string());
    l.ok("devolucao_desfazer", json!({ "id": com_defeito, "motivo": "lançada errado" }));
    confere(&mut l, &p, 8, "desfez a devolução com defeito");
    // Uma venda com devolução não pode ser cancelada antes de desfazer a devolução.
    l.erro("venda_cancelar", json!({ "id": v1, "motivo": "teste" }));
    confere(&mut l, &p, 8, "cancelamento recusado");
}

/// T2.2 — Compra lançada entra no estoque; o estorno tira.
#[test]
fn t2_2_compra_e_estorno() {
    let mut l = Loja::nova("t2-2");
    let compra = lancar_compra_de_exemplo(&mut l);
    let c = l.ok("compra_abrir", json!({ "id": compra }));
    let pecas: Vec<(String, i64)> = c["itens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|it| {
            let q: f64 = it["quantidadePecas"].as_str().unwrap().replace(',', ".").parse().unwrap();
            let pid: String = l
                .conexao()
                .query_row("SELECT produto_id FROM compra_item WHERE id = ?1", [it["id"].as_str().unwrap()], |x| {
                    x.get(0)
                })
                .unwrap();
            // D-06: a ficha da compra lançada mostra a peça criada.
            assert_eq!(it["peca"]["id"].as_str(), Some(pid.as_str()), "{it}");
            (pid, q as i64)
        })
        .collect();
    assert_eq!(pecas.iter().map(|(_, q)| *q).collect::<Vec<_>>(), vec![10, 4, 12], "{c}");
    for (p, q) in &pecas {
        confere(&mut l, p, *q, "compra lançada");
    }
    // Vende 2 do primeiro e estorna a compra: fica -2 (vendeu o que não existe mais) e avisa.
    l.abrir_caixa();
    l.vender(&[(&pecas[0].0, "2")], &[("dinheiro", "500")]);
    l.ok("compra_estornar", json!({ "id": compra, "motivo": "nota devolvida ao fornecedor" }));
    confere(&mut l, &pecas[0].0, -2, "estornou depois de vender 2");
    confere(&mut l, &pecas[1].0, 0, "estornou");
    confere(&mut l, &pecas[2].0, 0, "estornou");
    // Estornar duas vezes não tira de novo.
    l.erro("compra_estornar", json!({ "id": compra, "motivo": "de novo" }));
    confere(&mut l, &pecas[1].0, 0, "estorno repetido");
}

/// T2.3 — Ajuste de estoque (contagem): fica o contado, com o movimento da diferença.
#[test]
fn t2_3_ajuste() {
    let mut l = Loja::nova("t2-3");
    let p = l.peca("Pastilha", "40", "90", "5");
    l.ok("estoque_ajustar", json!({ "produtoId": p, "quantidade": "3", "motivo": "contagem" }));
    confere(&mut l, &p, 3, "ajustou para 3");
    l.erro("estoque_ajustar", json!({ "produtoId": p, "quantidade": "-1", "motivo": "contagem" }));
    l.erro("estoque_ajustar", json!({ "produtoId": p, "quantidade": "4", "motivo": "" }));
    confere(&mut l, &p, 3, "ajustes recusados");
    // O caixa não ajusta.
    l.usuario_caixa("Bia", SENHA_BIA);
    l.entrar("Bia", SENHA_BIA);
    l.erro("estoque_ajustar", json!({ "produtoId": p, "quantidade": "10", "motivo": "contagem" }));
    confere(&mut l, &p, 3, "caixa não ajusta");
}

/// T2.4 — Vender mais do que tem: a venda passa (o balcão não para), o
/// estoque fica negativo e a tela avisa a falta antes de concluir.
#[test]
fn t2_4_vender_mais_do_que_tem() {
    let mut l = Loja::nova("t2-4");
    let p = l.peca("Correia", "30", "80", "1");
    l.abrir_caixa();
    let calc = l.ok(
        "venda_calcular",
        json!({ "itens": [{ "produtoId": p, "quantidade": "3" }], "tabela": 1, "pagamentos": [] }),
    );
    assert_eq!(calc["itens"][0]["faltaEstoque"], true, "{calc}");
    l.vender(&[(&p, "3")], &[("dinheiro", "240")]);
    confere(&mut l, &p, -2, "vendeu 3 com 1");
}

/// T2.5 — Ao mesmo tempo: este caixa vende enquanto a troca de dados grava
/// as vendas do outro caixa no mesmo banco. Nada se perde nem se repete.
#[test]
fn t2_5_vendas_simultaneas_com_a_troca_de_dados() {
    let mut a = Loja::nova("t2-5");
    let p = a.peca("Lâmpada H4", "10", "25", "100");
    a.abrir_caixa();

    // O outro caixa (B) recebe tudo de A e vende 30, uma por vez.
    let mut b = Banco::em_memoria().unwrap();
    {
        let mut lado_a = Banco::abrir(&a.db).unwrap();
        trocar(&mut lado_a, &mut b);
    }
    let ana = b.conn.query_row("SELECT id FROM usuario WHERE nome = 'Ana'", [], |l| l.get::<_, String>(0)).unwrap();
    caixa::abrir(&b.conn, &b.dispositivo, &ana, "Caixa 2", "0").unwrap();
    for _ in 0..30 {
        let e = vendas::VendaEntrada {
            itens: vec![vendas::ItemEntrada { produto_id: p.clone(), quantidade: "1".into(), ..Default::default() }],
            tabela: 1,
            pagamentos: vec![vendas::PagamentoEntrada {
                forma: "dinheiro".into(),
                valor: "25".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let tx = b.conn.transaction().unwrap();
        vendas::concluir(&tx, &b.dispositivo, &ana, &e).unwrap();
        tx.commit().unwrap();
    }

    // A troca de dados aplica as vendas de B em A, em lotes pequenos, numa
    // tarefa à parte (como o Rustinho faz), enquanto A vende 30 no balcão.
    let db = a.db.clone();
    let lotes: Vec<Vec<sync::Alteracao>> = {
        let mut todas = Vec::new();
        let mut desde = 0;
        loop {
            let (lista, mais) = sync::alteracoes_desde(&b.conn, desde, 5).unwrap();
            if let Some(u) = lista.last() {
                desde = u.seq;
            }
            todas.push(lista);
            if !mais {
                break;
            }
        }
        todas
    };
    let tarefa = std::thread::spawn(move || {
        let mut lado = Banco::abrir(&db).unwrap();
        for lote in lotes {
            sync::aplicar(&mut lado.conn, &lote).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        sync::verificar_depois(&lado.conn).unwrap();
    });
    for _ in 0..30 {
        a.vender(&[(&p, "1")], &[("dinheiro", "25")]);
    }
    tarefa.join().unwrap();

    confere(&mut a, &p, 40, "100 - 30 de A - 30 de B");
    let vendas_mov =
        a.numero(&format!("SELECT COUNT(*) FROM estoque_movimento WHERE produto_id = '{p}' AND tipo = 'venda'"));
    assert_eq!(vendas_mov, 60, "cada venda baixou uma vez só");
    assert_eq!(a.numero("SELECT COUNT(*) FROM venda WHERE situacao = 'concluida'"), 60);
    // Trocar de novo não repete nada (B recebe as de A; A não recebe as de B outra vez).
    {
        let mut lado_a = Banco::abrir(&a.db).unwrap();
        trocar(&mut lado_a, &mut b);
    }
    confere(&mut a, &p, 40, "depois de trocar de novo");
    let em_b: i64 = b
        .conn
        .query_row(
            "SELECT COALESCE(SUM(quantidade), 0) / 1000 FROM estoque_movimento WHERE produto_id = ?1",
            [&p],
            |l| l.get(0),
        )
        .unwrap();
    assert_eq!(em_b, 40, "o outro caixa chega ao mesmo estoque");
}

/// Leva para `para` tudo o que `de` tem e ele ainda não (como `tests/caixas.rs`).
pub fn levar(de: &Banco, para: &mut Banco) -> usize {
    let mut desde = 0;
    let mut total = 0;
    loop {
        let (lista, mais) = sync::alteracoes_desde(&de.conn, desde, 50).unwrap();
        if let Some(u) = lista.last() {
            desde = u.seq;
        }
        total += sync::aplicar(&mut para.conn, &lista).unwrap();
        if !mais {
            break;
        }
    }
    sync::verificar_depois(&para.conn).unwrap();
    total
}

pub fn trocar(a: &mut Banco, b: &mut Banco) {
    for _ in 0..5 {
        if levar(a, b) + levar(b, a) == 0 {
            return;
        }
    }
    panic!("os caixas não pararam de trocar");
}
