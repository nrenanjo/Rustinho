//! R13 (DP-42, a peça cadastrada em dobro): o gerente junta a peça D na peça K. Exigido (roadmap da 007, desenho v3
//! e "Definições finais"):
//!
//! - só o gerente junta; a prévia mostra onde D vai ficar (a raiz de K) e o que diverge; divergência fiscal pede a
//!   confirmação do gerente;
//! - o estoque de D passa para K por duas linhas com id pelo conteúdo; os códigos de D passam a achar K; D fica
//!   desativada; o registro guarda o que foi feito;
//! - depois, todo movimento de estoque de D (devolução, cancelamento, estorno) entra em K;
//! - completar é idempotente: reaplica só o que faltou (o que chegou de outro caixa depois);
//! - recusas: a mesma peça, D já juntada, K desativada, peça com serviço, unidades diferentes, ciclo;
//! - dois caixas sem rede que juntam o mesmo par com a mesma visão gravam as mesmas linhas: a troca não cria conflito.

use serde_json::{json, Value};

use crate::banco::Banco;
use crate::{config, estoque, produtos, sync, usuarios};

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

fn recusa(r: &Value, trecho: &str) {
    assert_eq!(r["ok"], false, "{r}");
    assert!(r["erro"].as_str().unwrap_or_default().contains(trecho), "{trecho}: {r}");
}

fn loja() -> crate::Servico {
    let mut s = crate::Servico::em_memoria().unwrap();
    ok(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "12345678", "senha2": "12345678" }),
    );
    s
}

fn peca(s: &mut crate::Servico, nome: &str, estoque: &str, ncm: &str) -> String {
    let mut f = ok(s, "produto_novo", json!({}));
    f["nome"] = json!(nome);
    f["custo"] = json!("10");
    f["estoqueInicial"] = json!(estoque);
    f["ncm"] = json!(ncm);
    ok(s, "produto_salvar", f)["id"].as_str().unwrap().to_string()
}

fn conn(s: &mut crate::Servico) -> &rusqlite::Connection {
    &s.banco_para_teste().conn
}

fn saldo(c: &rusqlite::Connection, id: &str) -> i64 {
    estoque::saldo(c, id).unwrap()
}

fn contar(c: &rusqlite::Connection, sql: &str, p: impl rusqlite::Params) -> i64 {
    c.query_row(sql, p, |l| l.get(0)).unwrap()
}

#[test]
fn r13_juntar_passa_estoque_codigos_e_desativa_a_peca() {
    let mut s = loja();
    let d = peca(&mut s, "Filtro de óleo PSL55", "3", "84212300");
    let k = peca(&mut s, "Filtro óleo PSL 55", "2", "84212300");
    let codigo_d: String =
        conn(&mut s).query_row("SELECT codigo FROM produto WHERE id = ?1", [&d], |l| l.get(0)).unwrap();

    let p = ok(&mut s, "produto_juntar_previa", json!({ "produtoId": d, "paraId": k }));
    assert_eq!(p["paraId"], json!(k), "{p}");
    assert_eq!(p["divergencias"]["fiscal"], false, "{p}");

    let r = ok(&mut s, "produto_juntar", json!({ "produtoId": d, "paraId": k }));
    assert!(r["feito"][1].as_str().unwrap_or_default().contains("Estoque passado: 3"), "{r}");
    let c = conn(&mut s);
    assert_eq!((saldo(c, &k), saldo(c, &d)), (5000, 0));
    assert_eq!(contar(c, "SELECT ativo FROM produto WHERE id = ?1", [&d]), 0);
    assert_eq!(contar(c, "SELECT COUNT(*) FROM produto_juncao WHERE produto_id = ?1 AND para_id = ?2", [&d, &k]), 1);
    assert_eq!(contar(c, "SELECT COUNT(*) FROM auditoria WHERE acao = 'produto.juntar' AND entidade_id = ?1", [&d]), 1);
    assert_eq!(
        contar(c, "SELECT COUNT(*) FROM produto_codigo_antigo WHERE produto_id = ?1 AND codigo = ?2", [&k, &codigo_d]),
        1,
        "o código de D passa a achar K"
    );

    // F3: a devolução de uma venda antiga de D entra em K.
    estoque::lancar(c, "disp", None, &d, 1000, "devolucao", "Devolução").unwrap();
    assert_eq!((saldo(c, &k), saldo(c, &d)), (6000, 0));

    // Completar de novo não passa nada a mais.
    let r = ok(&mut s, "produto_juntar_completar", json!({ "produtoId": d }));
    assert_eq!(r["paraId"], json!(k), "{r}");
    let c = conn(&mut s);
    assert_eq!((saldo(c, &k), saldo(c, &d)), (6000, 0));
    assert_eq!(contar(c, "SELECT COUNT(*) FROM estoque_movimento WHERE tipo = 'juncao'", []), 2);
}

#[test]
fn r13_so_o_gerente_junta() {
    let mut s = loja();
    let d = peca(&mut s, "Vela A", "1", "");
    let k = peca(&mut s, "Vela B", "1", "");
    let bia = usuarios::criar(conn(&mut s), "disp", "Bia", "12345678", true).unwrap().id;
    ok(&mut s, "entrar", json!({ "id": bia, "senha": "12345678" }));
    let r = chamar(&mut s, "produto_juntar", json!({ "produtoId": d, "paraId": k }));
    assert_eq!((r["ok"].as_bool(), r["campo"].as_str()), (Some(false), Some("gerente")), "{r}");
    let r = chamar(&mut s, "produto_juntar_completar", json!({ "produtoId": d }));
    assert_eq!((r["ok"].as_bool(), r["campo"].as_str()), (Some(false), Some("gerente")), "{r}");
    assert_eq!(contar(conn(&mut s), "SELECT COUNT(*) FROM produto_juncao", []), 0);
}

#[test]
fn r13_divergencia_fiscal_pede_confirmacao() {
    let mut s = loja();
    let d = peca(&mut s, "Pastilha A", "1", "87083090");
    let k = peca(&mut s, "Pastilha B", "1", "68132000");
    let p = ok(&mut s, "produto_juntar_previa", json!({ "produtoId": d, "paraId": k }));
    assert_eq!(p["divergencias"]["fiscal"], true, "{p}");
    recusa(&chamar(&mut s, "produto_juntar", json!({ "produtoId": d, "paraId": k })), "fiscal");
    assert_eq!(contar(conn(&mut s), "SELECT COUNT(*) FROM produto_juncao", []), 0);
    ok(&mut s, "produto_juntar", json!({ "produtoId": d, "paraId": k, "confirmaFiscal": true }));
    assert_eq!(contar(conn(&mut s), "SELECT COUNT(*) FROM produto_juncao", []), 1);
}

#[test]
fn r13_recusas() {
    let mut s = loja();
    let a = peca(&mut s, "Correia A", "1", "");
    let b = peca(&mut s, "Correia B", "1", "");
    let c = peca(&mut s, "Correia C", "1", "");
    recusa(&chamar(&mut s, "produto_juntar", json!({ "produtoId": a, "paraId": a })), "diferentes");
    recusa(&chamar(&mut s, "produto_juntar_completar", json!({ "produtoId": a })), "não foi juntada");
    ok(&mut s, "produto_juntar", json!({ "produtoId": a, "paraId": b }));
    recusa(&chamar(&mut s, "produto_juntar", json!({ "produtoId": a, "paraId": c })), "já foi juntada");
    // B na A: a raiz de A é B, então seria juntar B nela mesma (o ciclo).
    recusa(&chamar(&mut s, "produto_juntar", json!({ "produtoId": b, "paraId": a })), "diferentes");
    conn(&mut s).execute("UPDATE produto SET ativo = 0 WHERE id = ?1", [&c]).unwrap();
    recusa(&chamar(&mut s, "produto_juntar", json!({ "produtoId": b, "paraId": c })), "desativada");
    conn(&mut s).execute("UPDATE produto SET ativo = 1, unidade = 'M' WHERE id = ?1", [&c]).unwrap();
    recusa(&chamar(&mut s, "produto_juntar", json!({ "produtoId": b, "paraId": c })), "mesma unidade");
    conn(&mut s).execute("UPDATE produto SET unidade = 'UN', tipo = 'servico' WHERE id = ?1", [&c]).unwrap();
    recusa(&chamar(&mut s, "produto_juntar", json!({ "produtoId": b, "paraId": c })), "peça com peça");
}

#[test]
fn r13_cadeia_vai_para_a_raiz() {
    let mut s = loja();
    let a = peca(&mut s, "Junta A", "1", "");
    let b = peca(&mut s, "Junta B", "2", "");
    let c = peca(&mut s, "Junta C", "4", "");
    ok(&mut s, "produto_juntar", json!({ "produtoId": a, "paraId": b }));
    ok(&mut s, "produto_juntar", json!({ "produtoId": b, "paraId": c }));
    let cx = conn(&mut s);
    assert_eq!(estoque::raiz(cx, &a).unwrap(), c);
    // Definição final 1: juntar nunca reescreve outra junção (A continua apontando para B; a raiz segue a cadeia).
    assert_eq!(contar(cx, "SELECT COUNT(*) FROM produto_juncao WHERE produto_id = ?1 AND para_id = ?2", [&a, &b]), 1);
    assert_eq!((saldo(cx, &a), saldo(cx, &b), saldo(cx, &c)), (0, 0, 7000));
    // Juntar em A (já juntada) vai para a raiz.
    let d = peca(&mut s, "Junta D", "1", "");
    let p = ok(&mut s, "produto_juntar_previa", json!({ "produtoId": d, "paraId": a }));
    assert_eq!(p["paraId"], json!(c), "{p}");
}

// ---------------------------------------------------------------- dois caixas

fn levar(de: &Banco, para: &mut Banco) -> usize {
    let (mut desde, mut total) = (0, 0);
    loop {
        let (lista, mais) = sync::alteracoes_desde(&de.conn, desde, 40).unwrap();
        if let Some(u) = lista.last() {
            desde = u.seq;
        }
        total += sync::aplicar(&mut para.conn, &lista).unwrap();
        if !mais {
            return total;
        }
    }
}

fn trocar(a: &mut Banco, b: &mut Banco) {
    for _ in 0..6 {
        if levar(a, b) + levar(b, a) == 0 {
            return;
        }
    }
    panic!("os caixas não pararam de trocar");
}

/// Dois caixas que já trocaram: D com 3 e K com 2.
fn dois() -> (Banco, Banco, String, String, String) {
    let mut a = Banco::em_memoria().unwrap();
    let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "1234", true).unwrap().id;
    usuarios::definir_gerente(&a.conn, &u, true).unwrap();
    config::gravar(&a.conn, "caixa_nome", "Caixa 1").unwrap();
    let mut ids = Vec::new();
    for (nome, q) in [("Amortecedor D", "3"), ("Amortecedor K", "2")] {
        let mut f = produtos::ficha_nova(&a.conn).unwrap();
        f.nome = nome.into();
        f.custo = "10".into();
        f.estoque_inicial = q.into();
        ids.push(produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap());
    }
    let mut b = Banco::em_memoria().unwrap();
    config::gravar(&b.conn, "caixa_nome", "Caixa 2").unwrap();
    trocar(&mut a, &mut b);
    let k = ids.pop().unwrap();
    let d = ids.pop().unwrap();
    (a, b, u, d, k)
}

fn juntar(b: &mut Banco, u: &str, d: &str, k: &str, completar: bool) {
    let tx = b.conn.transaction().unwrap();
    let disp = b.dispositivo.clone();
    crate::api::juntar_pecas(&tx, &disp, u, d, k, completar).unwrap();
    tx.commit().unwrap();
}

fn conflitos(b: &Banco) -> i64 {
    contar(&b.conn, "SELECT COUNT(*) FROM sync_conflito", [])
}

#[test]
fn r13_dois_caixas_com_a_mesma_visao_nao_criam_conflito() {
    let (mut a, mut b, u, d, k) = dois();
    juntar(&mut a, &u, &d, &k, false);
    juntar(&mut b, &u, &d, &k, false);
    trocar(&mut a, &mut b);
    for x in [&a, &b] {
        assert_eq!((saldo(&x.conn, &k), saldo(&x.conn, &d)), (5000, 0));
        assert_eq!(contar(&x.conn, "SELECT COUNT(*) FROM estoque_movimento WHERE tipo = 'juncao'", []), 2);
    }
    assert_eq!((conflitos(&a), conflitos(&b)), (0, 0));
}

#[test]
fn r13_venda_de_d_sem_rede_e_acertada_ao_completar() {
    let (mut a, mut b, u, d, k) = dois();
    // O caixa 2, sem saber da junção, vende 1 de D (antes da troca).
    estoque::lancar(&b.conn, &b.dispositivo.clone(), None, &d, -1000, "venda", "Venda").unwrap();
    juntar(&mut a, &u, &d, &k, false);
    trocar(&mut a, &mut b);
    // A venda chegou em D depois da junção: D fica com -1 até completar.
    assert_eq!((saldo(&a.conn, &k), saldo(&a.conn, &d)), (5000, -1000));
    juntar(&mut a, &u, &d, &k, true);
    trocar(&mut a, &mut b);
    for x in [&a, &b] {
        assert_eq!((saldo(&x.conn, &k), saldo(&x.conn, &d)), (4000, 0));
    }
    assert_eq!((conflitos(&a), conflitos(&b)), (0, 0));
}

/// As entradas pela raiz: depois de juntar D em K, a encomenda, a OS, a ligação de código de fornecedor e o item de
/// uma compra em rascunho (ligado a D antes da junção) gravam K.
#[test]
fn r13_entradas_vao_para_a_peca_que_ficou() {
    use crate::catalogo::codigos::{vincular_fornecedor, JaLigado};
    use crate::{compras, encomendas, nfe_xml, oficina, pessoas};
    let (mut a, _b, u, d, k) = dois();
    let disp = a.dispositivo.clone();
    let compra = {
        let c = &a.conn;
        let compra = compras::importar_xml(c, &disp, &u, nfe_xml::EXEMPLO).unwrap();
        let f = compras::ficha(c, &compra).unwrap();
        let itens = f
            .itens
            .iter()
            .enumerate()
            .map(|(i, it)| compras::ItemEntrada {
                id: it.id.clone(),
                acao: if i == 0 { "ligar".into() } else { "ignorar".into() },
                produto_id: if i == 0 { d.clone() } else { String::new() },
                fator: "1".into(),
                baixar_preco: false,
            })
            .collect();
        compras::atualizar(c, &compras::CompraEntrada { id: compra.clone(), itens, ..Default::default() }).unwrap();
        compra
    };
    juntar(&mut a, &u, &d, &k, false);
    let c = &a.conn;

    let enc = encomendas::criar(
        c,
        &disp,
        &u,
        &encomendas::EncomendaEntrada {
            cliente_nome: "Zé".into(),
            telefone: "11999990000".into(),
            produto_id: d.clone(),
            quantidade: "1".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(texto(c, "SELECT produto_id FROM encomenda WHERE id = ?1", &enc), k);

    let os = oficina::salvar_rascunho(
        c,
        &disp,
        &u,
        &oficina::Entrada {
            cliente_nome: "Carlos".into(),
            itens: vec![oficina::ItemEntradaOs {
                tipo: "peca".into(),
                produto_id: d.clone(),
                quantidade: "1".into(),
                preco: "10".into(),
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(texto(c, "SELECT produto_id FROM ordem_servico_item WHERE ordem_id = ?1", &os), k);

    let mut ff = pessoas::ficha_nova(c, "fornecedor").unwrap();
    ff.nome = "Patral Peças".into();
    let f = pessoas::salvar(c, &disp, &u, &ff).unwrap();
    assert!(vincular_fornecedor(c, &f, "AMR-9", &d, 1000, JaLigado::Manter).unwrap());
    assert_eq!(texto(c, "SELECT produto_id FROM produto_fornecedor WHERE codigo = 'AMR-9'", ""), k);

    compras::liberar_para_teste(c, &disp, &compra);
    compras::lancar(c, &disp, &u, &compra).unwrap();
    assert_eq!(
        texto(c, "SELECT produto_id FROM compra_item WHERE compra_id = ?1 AND acao = 'ligar'", &compra),
        k,
        "o item ligado a D antes da junção entra em K"
    );
    assert_eq!(saldo(c, &d), 0);
    assert!(saldo(c, &k) > 5000);
}

fn texto(c: &rusqlite::Connection, sql: &str, p: &str) -> String {
    if sql.contains("?1") {
        c.query_row(sql, [p], |l| l.get(0)).unwrap()
    } else {
        c.query_row(sql, [], |l| l.get(0)).unwrap()
    }
}

// ---------------------------------------------------------------- Duplicatas, ficha, disputa e círculo

/// A lista de Duplicatas: mesmo código original + mesma marca vira grupo (outra marca não); a peça juntada sai dos
/// grupos; a juntada com sobra aparece em "juntadas para ver" até completar.
#[test]
fn r13_duplicatas_pelo_original_e_sem_as_juntadas() {
    let mut s = loja();
    let mut ids = Vec::new();
    for (nome, original, marca) in
        [("Pivô A", "PV-1020", "Nakata"), ("Pivô B", "pv 1020", "NAKATA"), ("Pivô C", "PV-1020", "Viemar")]
    {
        let mut f = ok(&mut s, "produto_novo", json!({}));
        f["nome"] = json!(nome);
        f["codigoOriginal"] = json!(original);
        f["marca"] = json!(marca);
        f["custo"] = json!("10");
        f["estoqueInicial"] = json!("1");
        ids.push(ok(&mut s, "produto_salvar", f)["id"].as_str().unwrap().to_string());
    }
    let grupos = crate::faixas::repetidos(conn(&mut s)).unwrap();
    let originais: Vec<_> = grupos.iter().filter(|g| g.tipo == "original").collect();
    assert_eq!(originais.len(), 1, "{grupos:?}");
    let mut no_grupo: Vec<&str> = originais[0].pecas.iter().map(|p| p.id.as_str()).collect();
    no_grupo.sort();
    let mut esperado = vec![ids[0].as_str(), ids[1].as_str()];
    esperado.sort();
    assert_eq!(no_grupo, esperado, "a outra marca não é a mesma peça");

    ok(&mut s, "produto_juntar", json!({ "produtoId": ids[1], "paraId": ids[0] }));
    let c = conn(&mut s);
    assert!(crate::faixas::repetidos(c).unwrap().iter().all(|g| g.tipo != "original"));
    assert!(crate::faixas::juntadas_para_ver(c).unwrap().is_empty(), "juntada completa não pede nada");
    // Chega de outro caixa um movimento de B (a juntada): sobra até completar.
    c.execute(
        "INSERT INTO estoque_movimento (id, produto_id, quantidade, tipo, motivo, dispositivo, criado_em)
         VALUES ('mov-de-fora', ?1, -1000, 'venda', 'Venda', 'disp-caixa-2', '2026-11-25T10:00:00Z')",
        [&ids[1]],
    )
    .unwrap();
    let j = crate::faixas::juntadas_para_ver(c).unwrap();
    assert_eq!((j.len(), j[0].id.as_str(), j[0].sobras.clone()), (1, ids[1].as_str(), vec!["Estoque: -1".to_string()]));
    ok(&mut s, "produto_juntar_completar", json!({ "produtoId": ids[1] }));
    let c = conn(&mut s);
    assert!(crate::faixas::juntadas_para_ver(c).unwrap().is_empty());
    assert_eq!(saldo(c, &ids[0]), 1000);
}

/// A ficha: a juntada diz com quem e não volta a ficar ativa; a que ficou lista as juntadas e mostra o histórico
/// delas.
#[test]
fn r13_ficha_da_juntada_e_da_que_ficou() {
    let mut s = loja();
    let d = peca(&mut s, "Bomba A", "1", "");
    let k = peca(&mut s, "Bomba B", "1", "");
    ok(&mut s, "produto_juntar", json!({ "produtoId": d, "paraId": k }));
    let fd = ok(&mut s, "produto_abrir", json!({ "id": d }));
    assert!(fd["juntadaCom"].as_str().unwrap_or_default().contains("Bomba B"), "{fd}");
    let fk = ok(&mut s, "produto_abrir", json!({ "id": k }));
    assert!(fk["juntadas"][0].as_str().unwrap_or_default().contains("Bomba A"), "{fk}");
    assert!(
        fk["historico"].as_array().unwrap().iter().any(|r| r["acao"] == "produto.juntar"),
        "o histórico de K mostra a junção: {fk}"
    );
    let mut f = fd.clone();
    f["ativo"] = json!(true);
    recusa(&chamar(&mut s, "produto_salvar", f), "foi juntada com");
    assert_eq!(contar(conn(&mut s), "SELECT ativo FROM produto WHERE id = ?1", [&d]), 0);
}

/// Dois caixas sem rede juntam D em peças diferentes: a troca cria a "Junção em disputa" (sem "Usar a do outro"),
/// o Completar recusa até o gerente escolher, e o "Deixar todas" não a fecha.
#[test]
fn r13_juncao_em_disputa_entre_dois_caixas() {
    let (mut a, mut b, u, d, k) = dois();
    let k2 = {
        let mut f = produtos::ficha_nova(&a.conn).unwrap();
        f.nome = "Amortecedor K2".into();
        f.custo = "10".into();
        produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap()
    };
    trocar(&mut a, &mut b);
    juntar(&mut a, &u, &d, &k, false);
    juntar(&mut b, &u, &d, &k2, false);
    trocar(&mut a, &mut b);
    for x in [&a, &b] {
        let abertos = contar(
            &x.conn,
            "SELECT COUNT(*) FROM sync_conflito WHERE tabela = 'produto_juncao' AND resolvido_em IS NULL",
            [],
        );
        assert_eq!(abertos, 1, "a disputa aparece nos dois caixas");
        let lista = sync::conflitos(&x.conn).unwrap();
        let disputa = lista.iter().find(|c| c.tabela == "produto_juncao").unwrap();
        assert_eq!((disputa.o_que.as_str(), disputa.pode_usar), ("Junção em disputa", false));
        assert_eq!(disputa.juncao["produtoId"], json!(d));
        let mut opcoes: Vec<String> = disputa.juncao["opcoes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["id"].as_str().unwrap().to_string())
            .collect();
        opcoes.sort();
        let mut esperado = vec![k.clone(), k2.clone()];
        esperado.sort();
        assert_eq!(opcoes, esperado);
        assert!(crate::faixas::juntadas_para_ver(&x.conn).unwrap().iter().any(|j| j.id == d && j.disputa));
        // Cada caixa passou o estoque de D com um par equilibrado: D fica negativa até o gerente escolher, e o total
        // da loja não muda. Nenhum conflito no estoque.
        assert_eq!((saldo(&x.conn, &d), saldo(&x.conn, &k), saldo(&x.conn, &k2)), (-3000, 5000, 3000));
        assert_eq!(contar(&x.conn, "SELECT COUNT(*) FROM sync_conflito WHERE tabela <> 'produto_juncao'", []), 0);
    }
    assert_eq!(sync::manter_todos(&a.conn, &u).unwrap(), 0, "Deixar todas não fecha a disputa");
    let e = crate::api::juncao_para_completar(&a.conn, &d).unwrap_err();
    assert!(e.to_string().contains("Junção em disputa"), "{e}");

    // O gerente escolhe K2 no caixa 1: o par que foi para K volta para D, e o Completar leva D para K2.
    let mut s = crate::Servico::com_banco_para_teste(a);
    ok(&mut s, "entrar", json!({ "id": u, "senha": "1234" }));
    ok(&mut s, "produto_juncao_escolher", json!({ "produtoId": d, "paraId": k2 }));
    ok(&mut s, "produto_juntar_completar", json!({ "produtoId": d }));
    let a = s.banco_para_teste();
    trocar(a, &mut b);
    for x in [&*a, &b] {
        assert_eq!((saldo(&x.conn, &d), saldo(&x.conn, &k), saldo(&x.conn, &k2)), (0, 2000, 3000));
        assert_eq!(estoque::raiz(&x.conn, &d).unwrap(), k2);
        assert_eq!(contar(&x.conn, "SELECT COUNT(*) FROM sync_conflito WHERE resolvido_em IS NULL", []), 0);
    }
}

/// O gerente escolhe qual vale (pelo serviço, com a disputa plantada como a troca a deixa).
#[test]
fn r13_gerente_escolhe_a_juncao_em_disputa() {
    let mut s = loja();
    let d = peca(&mut s, "Coxim A", "2", "");
    let k = peca(&mut s, "Coxim B", "1", "");
    let k2 = peca(&mut s, "Coxim C", "1", "");
    ok(&mut s, "produto_juntar", json!({ "produtoId": d, "paraId": k }));
    let c = conn(&mut s);
    c.execute(
        "INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando, tipo, dados_perdeu)
         VALUES ('produto_juncao', json_array(?1), 'v1', 'v2', 'disp-caixa-2', '2026-11-25T10:00:00Z', 'registro',
                 json_object('produto_id', ?1, 'para_id', ?2, 'em', '2026-11-25T09:00:00Z', 'motivo', ''))",
        [&d, &k2],
    )
    .unwrap();
    // O "Deixar como está" genérico não serve aqui (o estoque não seguiria a escolha).
    let id: i64 = conn(&mut s).query_row("SELECT id FROM sync_conflito", [], |l| l.get(0)).unwrap();
    recusa(&chamar(&mut s, "conflito_resolver", json!({ "id": id, "usar": false })), "Fica em");
    let estranha = peca(&mut s, "Coxim D", "1", "");
    recusa(&chamar(&mut s, "produto_juncao_escolher", json!({ "produtoId": d, "paraId": estranha })), "uma das duas");
    recusa(&chamar(&mut s, "produto_juntar_completar", json!({ "produtoId": d })), "Junção em disputa");
    ok(&mut s, "produto_juncao_escolher", json!({ "produtoId": d, "paraId": k2 }));
    let c = conn(&mut s);
    assert_eq!(estoque::raiz(c, &d).unwrap(), k2);
    assert_eq!(
        contar(c, "SELECT COUNT(*) FROM sync_conflito WHERE tabela = 'produto_juncao' AND resolvido_em IS NULL", []),
        0
    );
    assert_eq!(contar(c, "SELECT COUNT(*) FROM auditoria WHERE acao = 'produto.juncao_escolher'", []), 1);
    recusa(&chamar(&mut s, "produto_juncao_escolher", json!({ "produtoId": d, "paraId": k })), "não está em disputa");
    // O par que foi para K voltou para D, e o Completar leva tudo para K2.
    ok(&mut s, "produto_juntar_completar", json!({ "produtoId": d }));
    let c = conn(&mut s);
    assert_eq!((saldo(c, &d), saldo(c, &k), saldo(c, &k2)), (0, 1000, 3000));
}

/// Duas peças juntadas uma na outra em caixas sem rede formam um círculo: as duas aparecem; "Reativar" uma delas
/// desfaz o círculo e ela vira a raiz.
#[test]
fn r13_circulo_entre_dois_caixas_e_reativar() {
    let (mut a, mut b, u, d, k) = dois();
    juntar(&mut a, &u, &d, &k, false);
    juntar(&mut b, &u, &k, &d, false);
    trocar(&mut a, &mut b);
    let j = crate::faixas::juntadas_para_ver(&a.conn).unwrap();
    assert_eq!(j.iter().filter(|x| x.circulo).count(), 2, "{j:?}");
    assert_eq!(contar(&a.conn, "SELECT COUNT(*) FROM sync_conflito", []), 0, "são linhas diferentes: sem conflito");

    let mut s = crate::Servico::com_banco_para_teste(a);
    ok(&mut s, "entrar", json!({ "id": u, "senha": "1234" }));
    recusa(&chamar(&mut s, "produto_juncao_reativar", json!({ "produtoId": "nao-existe" })), "não está num círculo");
    ok(&mut s, "produto_juncao_reativar", json!({ "produtoId": k }));
    let c = conn(&mut s);
    assert_eq!((estoque::raiz(c, &d).unwrap(), estoque::raiz(c, &k).unwrap()), (k.clone(), k.clone()));
    assert_eq!(contar(c, "SELECT ativo FROM produto WHERE id = ?1", [&k]), 1);
    ok(&mut s, "produto_juntar_completar", json!({ "produtoId": d }));
    let c = conn(&mut s);
    assert_eq!((saldo(c, &d), saldo(c, &k)), (0, 5000));
    assert!(crate::faixas::juntadas_para_ver(c).unwrap().is_empty());
}

/// Mutação R13-compra-raiz (sobreviveu ao teste das entradas): o item de compra em rascunho ligado a D que chega de
/// outro caixa DEPOIS da junção (a passagem da junção não o viu) entra em K ao lançar.
#[test]
fn r13_rascunho_que_chega_depois_da_juncao_lanca_na_que_ficou() {
    use crate::{compras, nfe_xml};
    let (mut a, _b, u, d, k) = dois();
    let disp = a.dispositivo.clone();
    juntar(&mut a, &u, &d, &k, false);
    let c = &a.conn;
    let compra = compras::importar_xml(c, &disp, &u, nfe_xml::EXEMPLO).unwrap();
    let f = compras::ficha(c, &compra).unwrap();
    let itens = f
        .itens
        .iter()
        .enumerate()
        .map(|(i, it)| compras::ItemEntrada {
            id: it.id.clone(),
            acao: if i == 0 { "ligar".into() } else { "ignorar".into() },
            produto_id: if i == 0 { k.clone() } else { String::new() },
            fator: "1".into(),
            baixar_preco: false,
        })
        .collect();
    compras::atualizar(c, &compras::CompraEntrada { id: compra.clone(), itens, ..Default::default() }).unwrap();
    // Como se o item tivesse sido ligado a D em outro caixa, antes de saber da junção, e chegado agora pela troca.
    c.execute("UPDATE compra_item SET produto_id = ?2 WHERE compra_id = ?1 AND acao = 'ligar'", [&compra, &d]).unwrap();
    compras::liberar_para_teste(c, &disp, &compra);
    compras::lancar(c, &disp, &u, &compra).unwrap();
    assert_eq!(texto(c, "SELECT produto_id FROM compra_item WHERE compra_id = ?1 AND acao = 'ligar'", &compra), k);
    assert_eq!(saldo(c, &d), 0);
}
