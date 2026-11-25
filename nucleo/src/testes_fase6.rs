//! Fase 6 da dev.3.3 (finanças, comércio, backup e segurança): testes de
//! caracterização das decisões do Q-D (DP-16 a DP-24) e da preparação da 6.3.
//!
//! Três tipos de teste, com o tipo no nome:
//! - `*_retrato_*`: passa hoje e mostra o comportamento de hoje (é a evidência
//!   da decisão; quando a decisão mudar a regra, o retrato muda junto);
//! - `*_alvo_*`: `#[ignore]` com o motivo; é o comportamento de uma das
//!   alternativas (a recomendada) e falha hoje. Só sai do `#[ignore]` se o
//!   proprietário escolher essa alternativa;
//! - `e04_*`: `#[ignore]`; lacunas do R-05 (§4 de `docs/R-05-criterios.md`,
//!   regra já aprovada) que ficam para a 6.3. Falham hoje pelo motivo certo.
//!
//! Nada aqui decide regra comercial. Só dados sintéticos, banco em memória
//! (ou numa pasta temporária própria).

use rusqlite::{params, Connection};
use serde_json::{json, Value};

use crate::banco::Banco;
use crate::cheques::ChequesEntrada;
use crate::vendas::{self, ItemEntrada, PagamentoEntrada as Pg, VendaEntrada};
use crate::{caixa, cheques, comissoes, devolucoes, financeiro, pagar, pessoas, produtos, receber, usuarios};

// ---------------------------------------------------------------------------
// Apoio.
// ---------------------------------------------------------------------------

/// Os dois computadores da loja (o mesmo banco, como depois da troca de dados).
const A: &str = "disp-caixa-a";
const B: &str = "disp-caixa-b";

struct Loja {
    b: Banco,
    u: String,
    banco: String,
    peca: String,
    cliente: String,
}

/// Uma loja: Maria (usuária), a conta "Sicoob" com R$ 1.000, a peça "Bateria
/// 60Ah" (custo 150, preço 300, 10 no estoque) e o cliente "Oficina do Zé".
fn loja() -> Loja {
    let b = Banco::em_memoria().unwrap();
    let (c, d) = (&b.conn, &b.dispositivo);
    let u = usuarios::criar(c, d, "Maria", "12345678", true).unwrap().id;
    let banco = financeiro::salvar_conta(
        c,
        d,
        &u,
        &financeiro::ContaEntrada {
            nome: "Sicoob".into(),
            tipo: "banco".into(),
            saldo_inicial: "1.000".into(),
            ativo: true,
            ..Default::default()
        },
    )
    .unwrap();
    let mut f = produtos::ficha_nova(c).unwrap();
    f.nome = "Bateria 60Ah".into();
    f.custo = "150".into();
    f.estoque_inicial = "10".into();
    let peca = produtos::salvar(c, d, &u, &f).unwrap();
    let mut cli = pessoas::ficha_nova(c, "cliente").unwrap();
    cli.nome = "Oficina do Zé".into();
    let cliente = pessoas::salvar(c, d, &u, &cli).unwrap();
    Loja { b, u, banco, peca, cliente }
}

fn pg(forma: &str, valor: &str) -> Pg {
    Pg { forma: forma.into(), valor: valor.into(), ..Default::default() }
}

/// Uma venda de `q` baterias, no computador `disp`, com os pagamentos dados.
fn vender(l: &Loja, disp: &str, q: &str, pagamentos: Vec<Pg>, cheque: Option<&str>) -> String {
    let e = VendaEntrada {
        itens: vec![ItemEntrada { produto_id: l.peca.clone(), quantidade: q.into(), ..Default::default() }],
        tabela: 1,
        cliente_id: l.cliente.clone(),
        pagamentos,
        cheques: ChequesEntrada {
            quantidade: 1,
            banco: "Itaú".into(),
            numero: cheque.unwrap_or("").into(),
            ..Default::default()
        },
        ..Default::default()
    };
    vendas::concluir(&l.b.conn, disp, &l.u, &e).unwrap()
}

fn sessao(c: &Connection, disp: &str) -> String {
    caixa::sessao_aberta(c, disp).unwrap().unwrap().id
}

fn esperado(c: &Connection, sessao_id: &str) -> String {
    caixa::resumo(c, sessao_id).unwrap().esperado
}

fn contar(c: &Connection, sql: &str, p: impl rusqlite::Params) -> i64 {
    c.query_row(sql, p, |l| l.get(0)).unwrap()
}

fn texto(c: &Connection, sql: &str, p: impl rusqlite::Params) -> String {
    c.query_row(sql, p, |l| l.get(0)).unwrap()
}

/// Põe um registro no estado da troca de dados pedido (`esperando` ou
/// `inconsistente`), como a conferência da troca faria (R-05).
fn plantar(c: &Connection, tabela: &str, id: &str, estado: &str) {
    c.execute(
        "INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo)
         VALUES (?1, json_array(?2), 'pessoa', '[\"sumiu\"]', ?3, '2026-01-01T00:00:00Z', 'log')",
        params![tabela, id, estado],
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// DP-16 (E02, E04, 0915-L): cancelamentos entre caixas e cascatas.
// ---------------------------------------------------------------------------

/// DP-16 · retrato (E02, R-02): a venda foi feita no caixa A, que continua
/// aberto; o cancelamento é feito no computador B. Hoje o resumo do caixa A
/// perde a venda (o esperado de lá cai R$ 300) e nada fica registrado no B
/// (o dinheiro devolvido sai de uma gaveta sem registro).
#[test]
fn dp16_retrato_cancelar_aqui_venda_da_gaveta_aberta_em_outro_caixa() {
    let l = loja();
    let c = &l.b.conn;
    caixa::abrir(c, A, &l.u, "Caixa A", "100").unwrap();
    caixa::abrir(c, B, &l.u, "Caixa B", "500").unwrap();
    let (sa, sb) = (sessao(c, A), sessao(c, B));
    let v = vender(&l, A, "1", vec![pg("dinheiro", "300")], None);
    assert_eq!(esperado(c, &sa), "400,00");
    vendas::cancelar(c, B, &l.u, &v, "desistiu").unwrap();
    assert_eq!(esperado(c, &sa), "100,00", "hoje a sessão do OUTRO computador muda");
    assert_eq!(esperado(c, &sb), "500,00", "hoje nada fica na gaveta deste computador");
    assert_eq!(contar(c, "SELECT cancelado_apos_fechar FROM venda WHERE id = ?1", [&v]), 0);
    assert_eq!(contar(c, "SELECT COUNT(*) FROM caixa_movimento WHERE origem_id = ?1", [&v]), 0);
}

/// DP-16 · alvo (proposta R-02/0915-L, §3): a sessão do outro computador nunca
/// muda; o dinheiro devolvido sai da gaveta aberta deste computador, ligado à
/// venda. Falha hoje.
#[test]
#[ignore = "depende da DP-16 (proposta R-02/0915-L): hoje a sessão do outro caixa muda e nada fica neste"]
fn dp16_alvo_r02_a_sessao_do_outro_caixa_nunca_muda() {
    let l = loja();
    let c = &l.b.conn;
    caixa::abrir(c, A, &l.u, "Caixa A", "100").unwrap();
    caixa::abrir(c, B, &l.u, "Caixa B", "500").unwrap();
    let (sa, sb) = (sessao(c, A), sessao(c, B));
    let v = vender(&l, A, "1", vec![pg("dinheiro", "300")], None);
    vendas::cancelar(c, B, &l.u, &v, "desistiu").unwrap();
    assert_eq!(esperado(c, &sa), "400,00", "a sessão do caixa A não muda");
    assert_eq!(esperado(c, &sb), "200,00", "os R$ 300 saem da gaveta do B");
    assert_eq!(contar(c, "SELECT COUNT(*) FROM caixa_movimento WHERE origem_id = ?1", [&v]), 1);
}

/// DP-16 · retrato (E02 quebrando a D-14 pela troca de dados): o caixa A
/// fechou com o dinheiro certo; depois chega pela troca o cancelamento feito
/// no B enquanto, para o B, o A ainda estava aberto (marca
/// `cancelado_apos_fechar = 0`). O fechamento do A, que a D-14 diz que nunca
/// muda, passa a mostrar sobra de R$ 300.
#[test]
fn dp16_retrato_fechamento_muda_quando_o_cancelamento_chega_pela_troca() {
    let l = loja();
    let c = &l.b.conn;
    caixa::abrir(c, A, &l.u, "Caixa A", "100").unwrap();
    let sa = sessao(c, A);
    let v = vender(&l, A, "1", vec![pg("dinheiro", "300")], None);
    caixa::fechar(c, A, &l.u, "400", "").unwrap();
    assert_eq!(caixa::resumo(c, &sa).unwrap().diferenca, "0,00");
    // O que a troca aplica: a linha da venda como o B a gravou.
    c.execute(
        "UPDATE venda SET situacao = 'cancelada', cancelado_em = '2026-11-13T12:00:00Z', cancelado_apos_fechar = 0
          WHERE id = ?1",
        [&v],
    )
    .unwrap();
    let r = caixa::resumo(c, &sa).unwrap();
    assert_eq!((r.esperado.as_str(), r.diferenca.as_str()), ("100,00", "300,00"), "o caixa fechado mudou");
    assert_eq!(contar(c, "SELECT esperado FROM caixa_sessao WHERE id = ?1", [&sa]), 40_000, "o gravado no fechamento");
}

/// DP-16 · retrato (proposta §6, item 2): o pagamento saiu da gaveta do
/// caixa A (aberto); o estorno feito no B vai para o Caixa geral, mesmo com a
/// gaveta do B aberta. A sangria continua tirando o dinheiro do A.
#[test]
fn dp16_retrato_estorno_de_pagamento_da_gaveta_de_outro_caixa_vai_ao_caixa_geral() {
    let l = loja();
    let c = &l.b.conn;
    caixa::abrir(c, A, &l.u, "Caixa A", "100").unwrap();
    caixa::abrir(c, B, &l.u, "Caixa B", "100").unwrap();
    let (sa, sb) = (sessao(c, A), sessao(c, B));
    let conta = pagar::lancar(
        c,
        A,
        &l.u,
        &pagar::LancamentoEntrada {
            descricao: "Frete".into(),
            categoria_id: "cat-aluguel".into(),
            valor: "60".into(),
            ..Default::default()
        },
    )
    .unwrap()[0]
        .clone();
    let pago = pagar::pagar(
        c,
        A,
        &l.u,
        &pagar::PagamentoEntrada {
            conta_id: conta,
            valor: "60".into(),
            forma: "dinheiro".into(),
            origem: "gaveta".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(esperado(c, &sa), "40,00");
    let geral = |c: &Connection| financeiro::saldo(c, "caixa-geral", "9999-12-31").unwrap();
    let antes = geral(c);
    pagar::estornar(c, B, &l.u, &pago, "lançado errado").unwrap();
    assert_eq!(geral(c) - antes, 6_000, "hoje o dinheiro volta ao Caixa geral");
    assert_eq!(esperado(c, &sb), "100,00", "e não à gaveta aberta deste computador");
    assert_eq!(esperado(c, &sa), "40,00", "a sangria do A continua");
}

/// DP-16 · retrato (0915-L, proposta §4): o cheque da venda ainda espera a
/// troca de dados (`esperando`); hoje o cancelamento em cascata passa e
/// cancela o cheque.
#[test]
fn dp16_retrato_cascata_com_filho_esperando_passa() {
    let l = loja();
    let c = &l.b.conn;
    caixa::abrir(c, A, &l.u, "Caixa A", "0").unwrap();
    let v = vender(&l, A, "1", vec![pg("cheque", "300")], Some("000301"));
    let ch = texto(c, "SELECT id FROM cheque WHERE venda_id = ?1", [&v]);
    plantar(c, "cheque", &ch, "esperando");
    vendas::cancelar(c, A, &l.u, &v, "desistiu").unwrap();
    assert_eq!(texto(c, "SELECT situacao FROM cheque WHERE id = ?1", [&ch]), "cancelado");
}

/// DP-16 · alvo (0915-L, proposta §4 e §6 item 3): filho esperando a troca
/// recusa a cascata inteira, dizendo qual registro espera; nada muda.
#[test]
#[ignore = "depende da DP-16 (0915-L, item 3): hoje a cascata cancela o filho que espera a troca"]
fn dp16_alvo_0915l_cascata_recusa_filho_esperando() {
    let l = loja();
    let c = &l.b.conn;
    caixa::abrir(c, A, &l.u, "Caixa A", "0").unwrap();
    let v = vender(&l, A, "1", vec![pg("cheque", "300")], Some("000302"));
    let ch = texto(c, "SELECT id FROM cheque WHERE venda_id = ?1", [&v]);
    plantar(c, "cheque", &ch, "esperando");
    let e = vendas::cancelar(c, A, &l.u, &v, "desistiu").unwrap_err().to_string();
    assert!(e.contains("troca"), "{e}");
    assert_eq!(texto(c, "SELECT situacao FROM venda WHERE id = ?1", [&v]), "concluida");
    assert_eq!(texto(c, "SELECT situacao FROM cheque WHERE id = ?1", [&ch]), "carteira");
}

// --- E04 (R-05 §4, regra já aprovada): cascatas sobre filho INCONSISTENTE ---

/// O que não pode ter mudado quando a operação é recusada.
fn venda_intacta(c: &Connection, v: &str) {
    assert_eq!(texto(c, "SELECT situacao FROM venda WHERE id = ?1", [v]), "concluida");
}

/// E04 · o §4 do R-05 bloqueia "cancelar" cheque inconsistente; o
/// cancelamento da venda cancela o cheque dela sem conferir (`cheques::cancelar_de`).
#[test]
#[ignore = "lacuna E04 (R-05 §4) para a 6.3: cheques::cancelar_de não chama o bloqueio"]
fn e04_cancelar_venda_com_cheque_inconsistente_e_recusado() {
    let l = loja();
    let c = &l.b.conn;
    caixa::abrir(c, A, &l.u, "Caixa A", "0").unwrap();
    let v = vender(&l, A, "1", vec![pg("cheque", "300")], Some("000303"));
    let ch = texto(c, "SELECT id FROM cheque WHERE venda_id = ?1", [&v]);
    plantar(c, "cheque", &ch, "inconsistente");
    let e = vendas::cancelar(c, A, &l.u, &v, "desistiu").unwrap_err().to_string();
    assert!(e.contains("inconsistente"), "{e}");
    venda_intacta(c, &v);
    assert_eq!(texto(c, "SELECT situacao FROM cheque WHERE id = ?1", [&ch]), "carteira");
}

/// E04 · o §4 bloqueia `cancelar_conta` de parcela inconsistente; o
/// cancelamento da venda cancela as parcelas dela sem conferir
/// (`receber::cancelar_da_venda`).
#[test]
#[ignore = "lacuna E04 (R-05 §4) para a 6.3: receber::cancelar_da_venda não chama o bloqueio"]
fn e04_cancelar_venda_com_parcela_inconsistente_e_recusado() {
    let l = loja();
    let c = &l.b.conn;
    caixa::abrir(c, A, &l.u, "Caixa A", "0").unwrap();
    let v = vender(&l, A, "1", vec![pg("crediario", "300")], None);
    let parcela = texto(c, "SELECT id FROM conta_receber WHERE venda_id = ?1 LIMIT 1", [&v]);
    plantar(c, "conta_receber", &parcela, "inconsistente");
    let e = vendas::cancelar(c, A, &l.u, &v, "desistiu").unwrap_err().to_string();
    assert!(e.contains("inconsistente"), "{e}");
    venda_intacta(c, &v);
    assert_eq!(contar(c, "SELECT COUNT(*) FROM conta_receber WHERE id = ?1 AND cancelado_em IS NULL", [&parcela]), 1);
}

/// E04 · o §4 bloqueia receber parcela inconsistente; o abatimento do
/// crediário na devolução paga a parcela sem conferir (`devolucoes::devolver`).
#[test]
#[ignore = "lacuna E04 (R-05 §4) para a 6.3: o abatimento em devolucoes::devolver não chama o bloqueio"]
fn e04_devolucao_que_abate_parcela_inconsistente_e_recusada() {
    let l = loja();
    let c = &l.b.conn;
    caixa::abrir(c, A, &l.u, "Caixa A", "0").unwrap();
    let v = vender(&l, A, "1", vec![pg("crediario", "300")], None);
    let parcela = texto(c, "SELECT id FROM conta_receber WHERE venda_id = ?1 LIMIT 1", [&v]);
    plantar(c, "conta_receber", &parcela, "inconsistente");
    let item = devolucoes::da_venda(c, &v).unwrap().itens[0].venda_item_id.clone();
    let e = devolucoes::devolver(
        c,
        A,
        &l.u,
        &devolucoes::Entrada {
            venda_id: v.clone(),
            itens: vec![devolucoes::ItemEntrada { venda_item_id: item, quantidade: "1".into(), defeito: false }],
            destino: "crediario".into(),
            motivo: "Não serviu".into(),
            ..Default::default()
        },
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("inconsistente"), "{e}");
    assert_eq!(contar(c, "SELECT COUNT(*) FROM devolucao", []), 0);
}

/// E04 · o §4 bloqueia "cancelar" cheque inconsistente; o estorno do
/// recebimento cancela o cheque dele sem conferir (o estorno confere o
/// recebimento e as parcelas, não o cheque).
#[test]
#[ignore = "lacuna E04 (R-05 §4) para a 6.3: receber::estornar não confere o cheque do recebimento"]
fn e04_estornar_recebimento_com_cheque_inconsistente_e_recusado() {
    let l = loja();
    let c = &l.b.conn;
    caixa::abrir(c, A, &l.u, "Caixa A", "0").unwrap();
    vender(&l, A, "1", vec![pg("crediario", "300")], None);
    let rec = receber::receber(
        c,
        A,
        &l.u,
        &receber::ReceberEntrada {
            cliente_id: l.cliente.clone(),
            forma: "cheque".into(),
            cheques: ChequesEntrada { numero: "777".into(), ..Default::default() },
            ..Default::default()
        },
    )
    .unwrap();
    let ch = texto(c, "SELECT id FROM cheque WHERE recebimento_id = ?1", [&rec]);
    plantar(c, "cheque", &ch, "inconsistente");
    let e = receber::estornar(c, A, &l.u, &rec, "errado").unwrap_err().to_string();
    assert!(e.contains("inconsistente"), "{e}");
    assert_eq!(contar(c, "SELECT COUNT(*) FROM recebimento WHERE id = ?1 AND estornado_em IS NULL", [&rec]), 1);
}

// ---------------------------------------------------------------------------
// DP-17 (E05, H14): orçamento vencido.
// ---------------------------------------------------------------------------

/// Um orçamento de 1 bateria (R$ 300) que venceu há 5 dias; depois a
/// bateria passou a custar R$ 400.
fn orcamento_vencido(l: &Loja, desconto_maximo: &str) -> String {
    let c = &l.b.conn;
    let d = &l.b.dispositivo;
    let mut f = produtos::ficha(c, &l.peca).unwrap();
    f.desconto_maximo = desconto_maximo.into();
    produtos::salvar(c, d, &l.u, &f).unwrap();
    let e = VendaEntrada {
        itens: vec![ItemEntrada { produto_id: l.peca.clone(), quantidade: "1".into(), ..Default::default() }],
        tabela: 1,
        cliente_id: l.cliente.clone(),
        validade_dias: 10,
        ..Default::default()
    };
    let o = vendas::salvar_orcamento(c, d, &l.u, "Caixa 1", &e).unwrap();
    c.execute("UPDATE venda SET validade = date('now', 'localtime', '-5 days') WHERE id = ?1", [&o]).unwrap();
    let mut f = produtos::ficha(c, &l.peca).unwrap();
    f.precos[0].preco = "400,00".into();
    f.precos[0].lucro = String::new();
    produtos::salvar(c, d, &l.u, &f).unwrap();
    o
}

/// DP-17 · retrato: o orçamento vencido vira venda pelos preços dele (R$ 300),
/// não pelos de hoje (R$ 400). A ficha diz "vencido" e a tela promete
/// conferir os preços de hoje (H14), o que não acontece.
#[test]
fn dp17_retrato_orcamento_vencido_vira_venda_pelos_precos_dele() {
    let l = loja();
    let c = &l.b.conn;
    let o = orcamento_vencido(&l, "");
    assert!(vendas::ficha(c, &o).unwrap().vencido);
    let carrinho = VendaEntrada {
        itens: vec![ItemEntrada { produto_id: l.peca.clone(), quantidade: "1".into(), ..Default::default() }],
        tabela: 1,
        ..Default::default()
    };
    assert_eq!(vendas::calcular(c, &carrinho).unwrap().total, "400,00", "o preço de hoje");
    let mut venda = vendas::orcamento_para_venda(c, &o).unwrap();
    assert_eq!(venda.itens[0].preco, "300,00", "o carrinho vem com o preço do orçamento");
    caixa::abrir(c, &l.b.dispositivo, &l.u, "Caixa 1", "0").unwrap();
    venda.pagamentos = vec![pg("pix", "300")];
    let v = vendas::concluir(c, &l.b.dispositivo, &l.u, &venda).unwrap();
    assert_eq!(vendas::ficha(c, &v).unwrap().total, "300,00");
    let tela = include_str!("../../app/qml/TelaVenda.qml");
    assert!(tela.contains("Ao vender, os preços de hoje serão conferidos."), "o texto da H14 mudou");
}

/// DP-17 · retrato (efeito lateral de "manter"): com desconto máximo na peça,
/// o preço antigo do orçamento conta como desconto sobre o preço de hoje, e a
/// venda do orçamento vencido é recusada.
#[test]
fn dp17_retrato_desconto_maximo_recusa_o_orcamento_vencido() {
    let l = loja();
    let c = &l.b.conn;
    let o = orcamento_vencido(&l, "5");
    let venda = vendas::orcamento_para_venda(c, &o).unwrap();
    let calc = vendas::calcular(c, &venda).unwrap();
    assert!(calc.itens[0].erro.contains("Desconto acima do máximo"), "{:?}", calc.itens[0].erro);
}

// ---------------------------------------------------------------------------
// DP-18 (E08): desfazer devolução de cheque.
// ---------------------------------------------------------------------------

/// Venda em cheque de R$ 300, depositado no Sicoob e devolvido pelo banco,
/// com a dívida lançada no Receber. Devolve o id do cheque.
fn cheque_devolvido(l: &Loja) -> String {
    let c = &l.b.conn;
    let d = &l.b.dispositivo;
    caixa::abrir(c, d, &l.u, "Caixa 1", "0").unwrap();
    let v = vender(l, d, "1", vec![pg("cheque", "300")], Some("000401"));
    let ch = texto(c, "SELECT id FROM cheque WHERE venda_id = ?1", [&v]);
    cheques::depositar(c, d, &l.u, &ch, &l.banco).unwrap();
    cheques::devolver(c, d, &l.u, &ch, "Sem fundos", true).unwrap();
    ch
}

/// DP-18 (decidida pelo proprietário em 25/11/2026, regra 19: "só o gerente
/// desfaz, com registro"). O cheque volta a depositado, a saída da conta é
/// estornada, a dívida lançada sai (sem pagamento) e o fluxo (E01) fica só com
/// o depósito. O estorno do movimento continua mandando "desfazer na lista de
/// cheques", que agora desfaz. O retrato de antes ("a devolução não tem volta")
/// saiu com a decisão. A permissão está em `r19_so_o_gerente_desfaz_a_devolucao`.
#[test]
fn dp18_gerente_desfaz_a_devolucao_do_cheque() {
    let l = loja();
    let c = &l.b.conn;
    let d = &l.b.dispositivo;
    let ch = cheque_devolvido(&l);
    let mov = texto(c, "SELECT id FROM movimento_conta WHERE origem_id = ?1 AND tipo = 'cheque_devolvido'", [&ch]);
    let e = financeiro::estornar_movimento(c, d, &l.u, &mov, "devolvido por engano").unwrap_err().to_string();
    assert!(e.contains("desfaça na lista de cheques"), "{e}");
    cheques::desfazer(c, d, &l.u, &ch).unwrap();
    assert_eq!(texto(c, "SELECT situacao FROM cheque WHERE id = ?1", [&ch]), "depositado");
    assert_eq!(texto(c, "SELECT conta_loja_id FROM cheque WHERE id = ?1", [&ch]), l.banco);
    assert_eq!(financeiro::saldo(c, &l.banco, "9999-12-31").unwrap(), 130_000);
    assert_eq!(receber::situacao(c, &l.cliente).unwrap().em_aberto, "0,00");
    let f = financeiro::fluxo(c, "", "", 30).unwrap();
    assert_eq!((f.entradas.as_str(), f.saidas.as_str()), ("300,00", "0,00"));
    // O registro: o depósito, a devolução e a devolução desfeita (um registro com o que foi desfeito; o alvo de
    // antes contava 4). A dívida cancelada tem o registro dela no Receber, com o motivo.
    assert_eq!(contar(c, "SELECT COUNT(*) FROM auditoria WHERE entidade_id = ?1 AND acao LIKE 'cheque.%'", [&ch]), 3);
    assert_eq!(
        contar(
            c,
            "SELECT COUNT(*) FROM conta_receber WHERE motivo_cancelamento = 'Devolução do cheque nº 000401 desfeita'",
            []
        ),
        1
    );
    assert_eq!(contar(c, "SELECT COUNT(*) FROM auditoria WHERE acao = 'receber.cancelar'", []), 1);
    let reg =
        texto(c, "SELECT detalhes FROM auditoria WHERE entidade_id = ?1 AND acao = 'cheque.desfazer_devolucao'", [&ch]);
    assert!(
        reg.contains("Devolução do cheque nº 000401 desfeita")
            && reg.contains("A dívida de R$ 300,00 no Receber foi cancelada")
            && reg.contains("Os R$ 300,00 voltaram para a conta"),
        "{reg}"
    );
    // Desfeita a devolução, o cheque segue o caminho normal: desfazer de novo tira o depósito.
    cheques::desfazer(c, d, &l.u, &ch).unwrap();
    assert_eq!(texto(c, "SELECT situacao FROM cheque WHERE id = ?1", [&ch]), "carteira");
    assert_eq!(financeiro::saldo(c, &l.banco, "9999-12-31").unwrap(), 100_000);
}

/// R19: o cheque devolvido em carteira (sem depósito) volta para a carteira, e a
/// dívida lançada na devolução sai. Sem dívida lançada na devolução, nenhuma
/// outra dívida do cliente é tocada, nem a de mesma descrição lançada à mão antes.
#[test]
fn r19_devolvido_em_carteira_e_so_a_divida_da_devolucao() {
    let l = loja();
    let c = &l.b.conn;
    let d = &l.b.dispositivo;
    caixa::abrir(c, d, &l.u, "Caixa 1", "0").unwrap();
    let v = vender(&l, d, "1", vec![pg("cheque", "300")], Some("000402"));
    let ch = texto(c, "SELECT id FROM cheque WHERE venda_id = ?1", [&v]);
    cheques::devolver(c, d, &l.u, &ch, "Sem fundos", true).unwrap();
    assert_eq!(receber::situacao(c, &l.cliente).unwrap().em_aberto, "300,00");
    cheques::desfazer(c, d, &l.u, &ch).unwrap();
    assert_eq!(texto(c, "SELECT situacao FROM cheque WHERE id = ?1", [&ch]), "carteira");
    assert_eq!(receber::situacao(c, &l.cliente).unwrap().em_aberto, "0,00");
    assert_eq!(financeiro::saldo(c, &l.banco, "9999-12-31").unwrap(), 100_000, "nenhum movimento na conta");
    // A dívida "igual", lançada à mão uma hora antes, e a devolução sem dívida.
    receber::lancar(
        c,
        d,
        &l.u,
        &receber::LancamentoEntrada {
            cliente_id: l.cliente.clone(),
            descricao: "Cheque nº 000402 devolvido".into(),
            valor: "300".into(),
            parcelas: 1,
            primeiro_vencimento: String::new(),
        },
    )
    .unwrap();
    c.execute(
        "UPDATE conta_receber SET criado_em = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-1 hour') WHERE cancelado_em IS NULL",
        [],
    )
    .unwrap();
    cheques::devolver(c, d, &l.u, &ch, "Sem fundos", false).unwrap();
    cheques::desfazer(c, d, &l.u, &ch).unwrap();
    assert_eq!(texto(c, "SELECT situacao FROM cheque WHERE id = ?1", [&ch]), "carteira");
    assert_eq!(receber::situacao(c, &l.cliente).unwrap().em_aberto, "300,00", "a dívida lançada à mão fica");
}

/// R19: a dívida da devolução já tem pagamento: nada é desfeito, e a mensagem diz
/// o que fazer antes (estornar o recebimento).
#[test]
fn r19_divida_paga_recusa_desfazer_a_devolucao() {
    let l = loja();
    let c = &l.b.conn;
    let d = &l.b.dispositivo;
    let ch = cheque_devolvido(&l);
    let pagar = receber::ReceberEntrada {
        cliente_id: l.cliente.clone(),
        valor: "100".into(),
        forma: "dinheiro".into(),
        ..Default::default()
    };
    receber::receber(c, d, &l.u, &pagar).unwrap();
    let e = cheques::desfazer(c, d, &l.u, &ch).unwrap_err().to_string();
    assert!(e.contains("já tem pagamento") && e.contains("Estorne o recebimento antes"), "{e}");
    assert_eq!(texto(c, "SELECT situacao FROM cheque WHERE id = ?1", [&ch]), "devolvido");
    assert_eq!(financeiro::saldo(c, &l.banco, "9999-12-31").unwrap(), 100_000, "1.000 + 300 − 300");
    assert_eq!(receber::situacao(c, &l.cliente).unwrap().em_aberto, "200,00");
    assert_eq!(contar(c, "SELECT COUNT(*) FROM auditoria WHERE acao = 'cheque.desfazer_devolucao'", []), 0);
}

// ---------------------------------------------------------------------------
// Pelo serviço (as permissões de verdade): DP-19, DP-20, DP-23.
// ---------------------------------------------------------------------------

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

/// A loja pelo serviço: Ana (gerente), uma peça que acabou e tem mínimo 2
/// (entra na lista de compra), uma parada no estoque e uma compra lançada de
/// um fornecedor. Depois entra Bia, que não é gerente. Devolve o serviço e
/// o id da Bia.
fn loja_com_bia() -> (crate::Servico, String) {
    let mut s = crate::Servico::em_memoria().unwrap();
    ok(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "12345678", "senha2": "12345678" }),
    );
    for (nome, estoque, minimo) in [("Filtro de óleo", "0", "2"), ("Lâmpada H4", "5", "")] {
        let mut f = ok(&mut s, "produto_novo", json!({}));
        f["nome"] = json!(nome);
        f["custo"] = json!("150");
        f["estoqueInicial"] = json!(estoque);
        f["estoqueMinimo"] = json!(minimo);
        ok(&mut s, "produto_salvar", f);
    }
    let mut f = ok(&mut s, "pessoa_nova", json!({ "papel": "fornecedor" }));
    f["nome"] = json!("Distribuidora Sul");
    let forn = ok(&mut s, "pessoa_salvar", f)["id"].as_str().unwrap().to_string();
    let c = &s.banco_para_teste().conn;
    c.execute(
        "INSERT INTO compra (id, fornecedor_id, total, situacao, lancada_em, criado_em, dispositivo)
         VALUES ('compra-dp19', ?1, 123400, 'lancada', ?2, ?2, 'disp')",
        params![forn, crate::tempo::agora()],
    )
    .unwrap();
    let bia = usuarios::criar(c, "disp", "Bia", "12345678", true).unwrap().id;
    ok(&mut s, "caixa_abrir", json!({ "valor": "100" }));
    ok(&mut s, "entrar", json!({ "id": bia, "senha": "12345678" }));
    (s, bia)
}

/// DP-19 (decidida pelo proprietário em 25/11/2026: "aplicar a D-09"; regra 20): quem
/// não vê os resultados não recebe custo na lista do que comprar nem na impressa, nem o
/// total comprado. O retrato do comportamento de antes saiu com a decisão.
#[test]
fn dp19_alvo_d09_tambem_na_lista_de_compra_e_no_painel() {
    let (mut s, _bia) = loja_com_bia();
    let sug = ok(&mut s, "estoque_sugestao", json!({ "texto": "" }));
    assert_eq!((sug["itens"][0]["custo"].as_str(), sug["custoTotal"].as_str()), (Some(""), Some("")));
    assert_eq!(sug["itens"][0]["custoTotal"], "", "{sug}");
    assert_eq!(sug["resultados"], false, "a tela mostra \"só o gerente\" (M3 da revisão)");
    let html =
        ok(&mut s, "impressao", json!({ "tipo": "lista_compra", "id": "" }))["html"].as_str().unwrap().to_string();
    assert!(!html.contains("150,00") && !html.contains("300,00"), "{html}");
    let painel = ok(&mut s, "painel_fornecedores", json!({ "dias": 30 }));
    assert_eq!((&painel["total"], &painel["resultados"]), (&json!(""), &json!(false)), "{painel}");
    assert!(painel["itens"].as_array().unwrap().iter().all(|i| i["total"] == ""), "B1 da revisão: {painel}");
    // Controle (B1 da revisão): o gerente continua vendo o custo nas três.
    let ana: String =
        s.banco_para_teste().conn.query_row("SELECT id FROM usuario WHERE nome = 'Ana'", [], |l| l.get(0)).unwrap();
    ok(&mut s, "entrar", json!({ "id": ana, "senha": "12345678" }));
    let sug = ok(&mut s, "estoque_sugestao", json!({ "texto": "" }));
    assert_eq!((sug["custoTotal"].as_str(), sug["resultados"].as_bool()), (Some("300,00"), Some(true)), "{sug}");
    let html =
        ok(&mut s, "impressao", json!({ "tipo": "lista_compra", "id": "" }))["html"].as_str().unwrap().to_string();
    assert!(html.contains("300,00"), "{html}");
    let painel = ok(&mut s, "painel_fornecedores", json!({ "dias": 30 }));
    assert_eq!((&painel["total"], &painel["resultados"]), (&json!("1.234,00"), &json!(true)), "{painel}");
    assert!(painel["itens"].as_array().unwrap().iter().all(|i| i["total"] != ""), "{painel}");
}

/// DP-20 e DP-24 (decididas pelo proprietário em 25/11/2026). DP-20 → regra 21,
/// conferência tradicional: quem não é gerente vê o dinheiro esperado na gaveta
/// antes de contar. DP-24 → OLHO-1: o olho do sigilo chega à tela do Caixa (o saldo
/// e os totais ficam discretos com o olho fechado), só na tela, sem mudar o que o
/// núcleo manda. O retrato de antes ("o Caixa fica fora do sigilo") saiu com a
/// decisão.
#[test]
fn dp20_dp24_esperado_a_vista_e_caixa_sob_o_olho() {
    let (mut s, _bia) = loja_com_bia();
    let e = ok(&mut s, "caixa_estado", json!({}));
    assert_eq!(
        e["resumo"]["esperado"], "100,00",
        "o esperado vai para a tela antes de contar, com o olho fechado ou aberto"
    );
    let tela = include_str!("../../app/qml/TelaCaixa.qml");
    assert!(tela.contains("titulo: \"Dinheiro na gaveta\""));
    assert!(tela.contains("\"R$ \" + Tema.valor(tela.r.esperado)"), "o esperado no resumo, sob o olho");
    assert!(tela.contains("sensivel: true"), "o Caixa tem o botão do olho");
    // R21: no diálogo de fechar, o esperado aparece antes da contagem e sem o olho.
    assert!(tela.contains("\"Esperado na gaveta: R$ \" + tela.r.esperado"), "o esperado no fechamento, fora do olho");
    assert!(tela.contains("Conte o dinheiro da gaveta e digite o total. O Rustinho mostra a diferença"));
}

/// R8 (D-AUD-8, decidida pelo proprietário em 25/11/2026, regra 8): o Enter não conclui sem querer.
/// (1) Fechar o caixa pede confirmação: a prévia mostra o contado, o esperado e a diferença, os mesmos que o
/// fechamento grava, e não fecha nada. (2) Nos campos de valor do Receber da venda e no "Dinheiro contado", o Enter
/// passa ao campo seguinte. (3) A pergunta de perigo não abre com o foco no botão de confirmar.
#[test]
fn r8_previa_do_fechamento_e_o_enter_que_nao_conclui() {
    let (mut s, _bia) = loja_com_bia();
    let p = ok(&mut s, "caixa_fechar_previa", json!({ "valor": "130" }));
    assert_eq!(
        (p["contado"].as_str(), p["esperado"].as_str(), p["diferenca"].as_str(), p["diferencaCentavos"].as_i64()),
        (Some("130,00"), Some("100,00"), Some("30,00"), Some(3000)),
        "{p}"
    );
    assert_eq!(ok(&mut s, "caixa_estado", json!({}))["aberto"], true, "a prévia não fecha o caixa");
    let r = chamar(&mut s, "caixa_fechar_previa", json!({ "valor": "" }));
    assert_eq!((r["ok"].as_bool(), r["campo"].as_str()), (Some(false), Some("contado")), "{r}");
    let f = ok(&mut s, "caixa_fechar", json!({ "valor": "130", "motivo": "" }));
    assert_eq!((&f["diferenca"], &f["diferencaCentavos"]), (&p["diferenca"], &p["diferencaCentavos"]), "{f}");
    // As telas: o Enter passa ao campo seguinte; fechar pede a prévia; a pergunta de perigo não confirma com o Enter.
    let vender = include_str!("../../app/qml/TelaVender.qml");
    assert!(!vender.contains("onConfirmado: botaoConcluir.clicked()"), "o Enter num valor ainda conclui a venda");
    assert!(
        vender.contains("onConfirmado: receber.seguinte(valor)")
            && vender.contains("onConfirmado: receber.seguinte(campoVale)")
    );
    let caixa = include_str!("../../app/qml/TelaCaixa.qml");
    assert!(caixa.contains("onConfirmado: obs.focar()") && caixa.contains("Api.chamar(\"caixa_fechar_previa\""));
    let dialogo = include_str!("../../app/qml/Dialogo.qml");
    assert!(dialogo.contains("dialogo.tipoConfirmar !== \"perigo\""), "a pergunta de perigo confirma com o Enter");
}

/// A recusa que pede a senha do gerente, com o nome do que é dele na mensagem.
fn so_o_gerente(r: &Value, o_que: &str) {
    assert_eq!((r["ok"].as_bool(), r["campo"].as_str()), (Some(false), Some("gerente")), "{r}");
    assert!(r["erro"].as_str().unwrap_or_default().contains(o_que), "{o_que}: {r}");
}

fn liberar(s: &mut crate::Servico, comando: &str, dados: &Value) {
    ok(s, "gerente_liberar", json!({ "senha": "12345678", "comando": comando, "dados": dados }));
}

/// R19 (DP-18): desfazer a devolução de um cheque é sempre do gerente, mesmo com
/// "estornar" liberado para todos na pílula Gerente; com a senha dele, desfaz. O
/// controle: com "estornar" livre, a caixa ainda desfaz um depósito sem senha.
#[test]
fn r19_so_o_gerente_desfaz_a_devolucao() {
    let (mut s, _bia) = loja_com_bia();
    let (devolvido, depositado) = {
        let b = s.banco_para_teste();
        let (c, disp) = (&b.conn, b.dispositivo.clone());
        let ana: String = c.query_row("SELECT id FROM usuario WHERE nome = 'Ana'", [], |l| l.get(0)).unwrap();
        // "cheque_devolucao" na lista dos livres (uma configuração adulterada ou vinda de fora): nem assim.
        crate::config::gravar(c, "gerente_livres", "estornar,financeiro,cheque_devolucao").unwrap();
        let conta = financeiro::salvar_conta(
            c,
            &disp,
            &ana,
            &financeiro::ContaEntrada {
                nome: "Sicoob".into(),
                tipo: "banco".into(),
                ativo: true,
                ..Default::default()
            },
        )
        .unwrap();
        let mut cli = pessoas::ficha_nova(c, "cliente").unwrap();
        cli.nome = "Oficina do Zé".into();
        let cliente = pessoas::salvar(c, &disp, &ana, &cli).unwrap();
        let mut ids = vec![];
        for (id, numero) in [("ch-r19-a", "000501"), ("ch-r19-b", "000502")] {
            c.execute(
                "INSERT INTO cheque (id, cliente_id, numero, valor, bom_para, criado_em, dispositivo)
                 VALUES (?1, ?2, ?3, 30000, date('now'), ?4, ?5)",
                params![id, cliente, numero, crate::tempo::agora(), disp],
            )
            .unwrap();
            cheques::depositar(c, &disp, &ana, id, &conta).unwrap();
            ids.push(id.to_string());
        }
        cheques::devolver(c, &disp, &ana, &ids[0], "Sem fundos", true).unwrap();
        (ids[0].clone(), ids[1].clone())
    };
    let situacao = |s: &mut crate::Servico, id: &str| -> String {
        s.banco_para_teste().conn.query_row("SELECT situacao FROM cheque WHERE id = ?1", [id], |l| l.get(0)).unwrap()
    };
    let r = chamar(&mut s, "cheque_desfazer", json!({ "id": devolvido }));
    assert_eq!((r["ok"].as_bool(), r["campo"].as_str()), (Some(false), Some("gerente")), "{r}");
    assert!(r["erro"].as_str().unwrap().contains("desfazer a devolução de um cheque"), "{r}");
    assert_eq!(situacao(&mut s, &devolvido), "devolvido", "nada mudou");
    // Controle: "estornar" está livre, e desfazer o depósito não pede a senha.
    ok(&mut s, "cheque_desfazer", json!({ "id": depositado }));
    assert_eq!(situacao(&mut s, &depositado), "carteira");
    // Com a senha do gerente, desfaz, e a liberação fica no registro.
    liberar(&mut s, "cheque_desfazer", &json!({ "id": devolvido }));
    ok(&mut s, "cheque_desfazer", json!({ "id": devolvido }));
    assert_eq!(situacao(&mut s, &devolvido), "depositado");
    let c = &s.banco_para_teste().conn;
    let lib = texto(c, "SELECT detalhes FROM auditoria WHERE acao = 'gerente.liberar' ORDER BY rowid DESC LIMIT 1", []);
    assert!(lib.contains("desfazer o depósito, o repasse ou a devolução de um cheque"), "{lib}");
    assert_eq!(contar(c, "SELECT COUNT(*) FROM auditoria WHERE acao = 'cheque.desfazer_devolucao'", []), 1);
}

/// DP-23 (decidida pelo proprietário em 25/11/2026, regra 24: "só o gerente, com
/// registro"; E15, G5, PEND-31): mudar o limite de crédito do cliente, criar uma
/// unidade fracionada e mudar o CSOSN da peça pedem a senha do gerente; com ela, a
/// mudança passa e fica no registro do cadastro. Salvar o cadastro sem mexer nisso
/// continua livre. O retrato de antes (a Bia mudava tudo sem o gerente) saiu com a
/// decisão.
#[test]
fn dp23_sensiveis_so_com_o_gerente_e_com_registro() {
    let (mut s, _bia) = loja_com_bia();

    // Limite de crédito. O cliente novo sem limite é livre; com limite, é do gerente.
    let mut f = ok(&mut s, "pessoa_nova", json!({ "papel": "cliente" }));
    f["nome"] = json!("Oficina do Zé");
    let mut com_limite = f.clone();
    com_limite["limiteCredito"] = json!("300");
    so_o_gerente(&chamar(&mut s, "pessoa_salvar", com_limite), "limite de crédito");
    let cli = ok(&mut s, "pessoa_salvar", f)["id"].as_str().unwrap().to_string();
    let mut f = ok(&mut s, "pessoa_abrir", json!({ "id": cli }));
    f["limiteCredito"] = json!("500");
    so_o_gerente(&chamar(&mut s, "pessoa_salvar", f.clone()), "limite de crédito");
    let c = &s.banco_para_teste().conn;
    assert_eq!(contar(c, "SELECT COUNT(*) FROM pessoa WHERE id = ?1 AND limite_credito IS NOT NULL", [&cli]), 0);
    liberar(&mut s, "pessoa_salvar", &f);
    ok(&mut s, "pessoa_salvar", f);
    let c = &s.banco_para_teste().conn;
    let det = texto(
        c,
        "SELECT COALESCE(MAX(detalhes), '') FROM auditoria WHERE entidade_id = ?1 AND acao = 'pessoa.alterar'",
        [&cli],
    );
    assert!(det.contains("Limite de crédito"), "{det}");
    let lib = texto(c, "SELECT COALESCE(MAX(detalhes), '') FROM auditoria WHERE acao = 'gerente.liberar'", []);
    assert!(lib.contains("mudar o limite de crédito do cliente (uma vez, com estes dados)"), "{lib}");
    // A senha valeu para aquela gravação; salvar de novo sem mexer no limite é livre.
    let mut f = ok(&mut s, "pessoa_abrir", json!({ "id": cli }));
    f["observacoes"] = json!("paga em dia");
    ok(&mut s, "pessoa_salvar", f.clone());
    f["limiteCredito"] = json!("");
    so_o_gerente(&chamar(&mut s, "pessoa_salvar", f), "limite de crédito");

    // CSOSN pela conferência fiscal ("passar para automático").
    let c = &s.banco_para_teste().conn;
    let peca = texto(c, "SELECT id FROM produto WHERE nome = 'Lâmpada H4'", []);
    c.execute("UPDATE produto SET csosn = '500' WHERE id = ?1", [&peca]).unwrap();
    let pedido = json!({ "ids": [peca] });
    so_o_gerente(&chamar(&mut s, "fiscal_automatico", pedido.clone()), "CSOSN");
    let c = &s.banco_para_teste().conn;
    assert_eq!(texto(c, "SELECT csosn FROM produto WHERE id = ?1", [&peca]), "500");
    liberar(&mut s, "fiscal_automatico", &pedido);
    assert_eq!(ok(&mut s, "fiscal_automatico", pedido)["alteradas"], 1);
    let c = &s.banco_para_teste().conn;
    let det = texto(
        c,
        "SELECT COALESCE(MAX(detalhes), '') FROM auditoria WHERE entidade_id = ?1 AND acao = 'produto.alterar'",
        [&peca],
    );
    assert!(det.contains("CSOSN: 500 → automático"), "{det}");

    // CSOSN pela ficha da peça. Salvar a ficha sem mexer nele é livre.
    let mut f = ok(&mut s, "produto_abrir", json!({ "id": peca }));
    f["local"] = json!("prateleira 3");
    ok(&mut s, "produto_salvar", f);
    let mut f = ok(&mut s, "produto_abrir", json!({ "id": peca }));
    f["csosn"] = json!("500");
    so_o_gerente(&chamar(&mut s, "produto_salvar", f.clone()), "CSOSN");
    let c = &s.banco_para_teste().conn;
    assert_eq!(texto(c, "SELECT csosn FROM produto WHERE id = ?1", [&peca]), "");
    liberar(&mut s, "produto_salvar", &f);
    ok(&mut s, "produto_salvar", f);
    let c = &s.banco_para_teste().conn;
    assert_eq!(texto(c, "SELECT csosn FROM produto WHERE id = ?1", [&peca]), "500");
    let det = texto(
        c,
        "SELECT detalhes FROM auditoria WHERE entidade_id = ?1 AND acao = 'produto.alterar' ORDER BY rowid DESC LIMIT 1",
        [&peca],
    );
    assert!(det.contains("CSOSN"), "{det}");

    // Unidade: a inteira é livre; a fracionada é do gerente e entra no registro (E10).
    ok(&mut s, "unidade_salvar", json!({ "sigla": "CX2", "descricao": "Caixa com 2", "fracionada": false }));
    let litro = json!({ "sigla": "LTS2", "descricao": "Litro avulso", "fracionada": true });
    so_o_gerente(&chamar(&mut s, "unidade_salvar", litro.clone()), "unidade fracionada");
    let c = &s.banco_para_teste().conn;
    assert_eq!(contar(c, "SELECT COUNT(*) FROM unidade WHERE sigla = 'LTS2'", []), 0);
    liberar(&mut s, "unidade_salvar", &litro);
    ok(&mut s, "unidade_salvar", litro);
    let c = &s.banco_para_teste().conn;
    assert_eq!(
        contar(c, "SELECT COUNT(*) FROM auditoria WHERE acao = 'unidade.criar' AND entidade_id = 'LTS2'", []),
        1
    );
    // A unidade que existe nunca muda de "vende quebrado", nem com a senha (a sigla repetida é recusada).
    let un = json!({ "sigla": "UN", "descricao": "Unidade", "fracionada": true });
    liberar(&mut s, "unidade_salvar", &un);
    let r = chamar(&mut s, "unidade_salvar", un);
    assert_eq!((r["ok"].as_bool(), r["campo"].as_str()), (Some(false), Some("sigla")), "{r}");
    let c = &s.banco_para_teste().conn;
    assert_eq!(contar(c, "SELECT fracionada FROM unidade WHERE sigla = 'UN'", []), 0);
    // As telas dão à caixa o caminho da senha do gerente nas três gravações.
    for (tela, comando) in [
        (include_str!("../../app/qml/TelaPessoa.qml"), "pessoa_salvar"),
        (include_str!("../../app/qml/EscolherUnidade.qml"), "unidade_salvar"),
        (include_str!("../../app/qml/TelaFiscal.qml"), "fiscal_automatico"),
    ] {
        assert!(tela.contains(&format!("janela.chamarGerente(\"{comando}\"")), "{comando}");
        assert!(
            !tela.contains(&format!("Api.gravar(\"{comando}\""))
                && !tela.contains(&format!("Api.chamar(\"{comando}\""))
        );
    }
}

/// B2 da revisão R1 da 006: a senha do gerente digitada errada para uma gravação que só às vezes é dele fica no
/// registro com o nome certo do que se pedia (antes, "mudar o preço de venda" para o limite de crédito, a unidade
/// fracionada e o CSOSN).
#[test]
fn b2_senha_errada_registra_o_que_se_pedia() {
    let (mut s, _bia) = loja_com_bia();
    let mut f = ok(&mut s, "pessoa_nova", json!({ "papel": "cliente" }));
    f["nome"] = json!("Oficina do Zé");
    f["limiteCredito"] = json!("300");
    let r =
        chamar(&mut s, "gerente_liberar", json!({ "senha": "senha-errada", "comando": "pessoa_salvar", "dados": f }));
    assert_eq!(r["campo"], "senha", "{r}");
    let c = &s.banco_para_teste().conn;
    let det = texto(c, "SELECT COALESCE(MAX(detalhes), '') FROM auditoria WHERE acao = 'gerente.senha_errada'", []);
    assert!(det.contains("Para: mudar o limite de crédito do cliente (comando pessoa_salvar)"), "{det}");
    assert!(!det.contains("preço de venda"), "{det}");
}

/// R4 (D-AUD-4, decidida pelo proprietário em 25/11/2026, regra 4): mudar o desconto
/// máximo ou a comissão da peça pede a senha do gerente (na peça nova, preencher um
/// dos dois também); salvar a ficha sem mexer neles continua livre.
#[test]
fn r4_desconto_maximo_e_comissao_so_com_o_gerente() {
    let (mut s, _bia) = loja_com_bia();
    let c = &s.banco_para_teste().conn;
    let peca = texto(c, "SELECT id FROM produto WHERE nome = 'Lâmpada H4'", []);
    for (campo, valor, coluna) in [("descontoMaximo", "10", "desconto_maximo"), ("comissao", "5", "comissao")] {
        let mut f = ok(&mut s, "produto_abrir", json!({ "id": peca }));
        f[campo] = json!(valor);
        so_o_gerente(&chamar(&mut s, "produto_salvar", f.clone()), "desconto máximo ou a comissão");
        let c = &s.banco_para_teste().conn;
        let sql = format!("SELECT COUNT(*) FROM produto WHERE id = ?1 AND {coluna} IS NOT NULL");
        assert_eq!(contar(c, &sql, [&peca]), 0, "{campo}");
        liberar(&mut s, "produto_salvar", &f);
        ok(&mut s, "produto_salvar", f);
        let c = &s.banco_para_teste().conn;
        assert_eq!(contar(c, &sql, [&peca]), 1, "{campo}");
    }
    // Salvar de novo sem mexer neles (os dois preenchidos): livre.
    let mut f = ok(&mut s, "produto_abrir", json!({ "id": peca }));
    assert_eq!((f["descontoMaximo"].as_str(), f["comissao"].as_str()), (Some("10"), Some("5")), "{f}");
    f["local"] = json!("gaveta 2");
    ok(&mut s, "produto_salvar", f.clone());
    // Tirar a comissão também é mudar.
    f["comissao"] = json!("");
    so_o_gerente(&chamar(&mut s, "produto_salvar", f), "desconto máximo ou a comissão");
    // Na peça nova, preencher a comissão é do gerente; sem ela, a peça nova é livre.
    let mut f = ok(&mut s, "produto_novo", json!({}));
    f["nome"] = json!("Palheta");
    f["precos"][0]["preco"] = json!("39,90");
    let mut com_comissao = f.clone();
    com_comissao["comissao"] = json!("5");
    so_o_gerente(&chamar(&mut s, "produto_salvar", com_comissao), "desconto máximo ou a comissão");
    ok(&mut s, "produto_salvar", f);
}

// ---------------------------------------------------------------------------
// DP-21 (E06): estoque negativo. DP-22 (E16): comissão e devolução tardia.
// ---------------------------------------------------------------------------

/// DP-21 · retrato: vender mais do que tem não para a venda; o item é marcado
/// como falta e o estoque fica negativo.
#[test]
fn dp21_retrato_venda_sem_estoque_conclui_e_o_estoque_fica_negativo() {
    let l = loja();
    let c = &l.b.conn;
    let d = &l.b.dispositivo;
    caixa::abrir(c, d, &l.u, "Caixa 1", "0").unwrap();
    let e = VendaEntrada {
        itens: vec![ItemEntrada { produto_id: l.peca.clone(), quantidade: "12".into(), ..Default::default() }],
        tabela: 1,
        pagamentos: vec![pg("pix", "3600")],
        ..Default::default()
    };
    let calc = vendas::calcular(c, &e).unwrap();
    assert!(calc.itens[0].falta_estoque && calc.itens[0].erro.is_empty());
    vendas::concluir(c, d, &l.u, &e).unwrap();
    assert_eq!(crate::estoque::saldo(c, &l.peca).unwrap(), -2_000);
}

/// DP-22 (decidida pelo proprietário em 25/11/2026, regra 23: "descontar no período seguinte"; E16): a comissão de
/// outubro foi fechada em 01/11 (R$ 15,00 = 5% de R$ 300). Depois o cliente devolve a peça. Outubro não muda (o
/// fechamento e o relatório continuam R$ 15,00); a devolução desconta no período em que foi feita. Sem comissão ali
/// para descontar, fechar recusa e explica; com uma venda no período, desconta dela. Desfeito o fechamento de
/// outubro, a devolução volta para outubro. O retrato de antes (a devolução não descontava em lugar nenhum) saiu
/// com a decisão.
#[test]
fn dp22_devolucao_depois_do_fechamento_desconta_no_periodo_seguinte() {
    let l = loja();
    let c = &l.b.conn;
    let d = &l.b.dispositivo;
    usuarios::definir_comissao(c, &l.u, "5").unwrap();
    caixa::abrir(c, d, &l.u, "Caixa 1", "0").unwrap();
    let v = vender(&l, d, "1", vec![pg("pix", "300")], None);
    c.execute("UPDATE venda SET criado_em = '2026-10-15T15:00:00Z' WHERE id = ?1", [&v]).unwrap();
    let outubro = || comissoes::relatorio(c, "01/10/2026", "31/10/2026").unwrap();
    assert_eq!(outubro().vendedores[0].comissao, "15,00");
    comissoes::fechar(c, d, &l.u, &l.u, "01/10/2026", "31/10/2026", "").unwrap();
    c.execute("UPDATE comissao_fechamento SET criado_em = '2026-11-01T12:00:00Z'", []).unwrap();
    let item = devolucoes::da_venda(c, &v).unwrap().itens[0].venda_item_id.clone();
    devolucoes::devolver(
        c,
        d,
        &l.u,
        &devolucoes::Entrada {
            venda_id: v.clone(),
            itens: vec![devolucoes::ItemEntrada { venda_item_id: item, quantidade: "1".into(), defeito: false }],
            destino: "vale".into(),
            motivo: "Não serviu".into(),
            ..Default::default()
        },
    )
    .unwrap();
    // A devolução foi ontem (depois do fechamento de 01/11).
    c.execute("UPDATE devolucao SET criado_em = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-1 day')", []).unwrap();
    let ontem: String =
        c.query_row("SELECT strftime('%d/%m/%Y', 'now', 'localtime', '-1 day')", [], |l| l.get(0)).unwrap();
    let r = outubro();
    assert_eq!(r.vendedores[0].comissao, "15,00", "o período fechado não muda");
    assert_eq!(r.fechamentos[0].valor, "15,00", "o fechamento fica");
    let dia = || {
        let v = comissoes::relatorio(c, &ontem, &ontem).unwrap().vendedores.remove(0);
        (v.comissao, v.vendido)
    };
    assert_eq!(dia(), ("-15,00".to_string(), "-300,00".to_string()), "desconta no dia da devolução");
    let e = comissoes::fechar(c, d, &l.u, &l.u, &ontem, &ontem, "").unwrap_err().to_string();
    assert!(e.contains("negativa (R$ -15,00)") && e.contains("período maior"), "{e}");
    // Com uma venda no mesmo dia (R$ 15,00 de comissão), o desconto sai dela.
    let v2 = vender(&l, d, "1", vec![pg("pix", "300")], None);
    c.execute("UPDATE venda SET criado_em = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-1 day') WHERE id = ?1", [&v2])
        .unwrap();
    assert_eq!(dia().0, "0,00");
    // Desfeito o fechamento de outubro, a devolução volta para outubro (e sai do dia dela).
    let f = outubro().fechamentos[0].id.clone();
    comissoes::reabrir(c, d, &l.u, &f).unwrap();
    assert_eq!(outubro().vendedores[0].comissao, "0,00");
    assert_eq!(dia().0, "15,00");
}

/// B3 da revisão R1 da 006 (R23), o cenário da DP-22: venda de R$ 300 em 15/10 (5% = R$ 15,00), outubro fechado em
/// 01/11 e a peça devolvida ontem. Devolve a loja e ontem ("dd/mm/aaaa").
fn cenario_devolucao_depois_do_fechamento() -> (Loja, String) {
    let l = loja();
    {
        let c = &l.b.conn;
        let d = &l.b.dispositivo;
        usuarios::definir_comissao(c, &l.u, "5").unwrap();
        caixa::abrir(c, d, &l.u, "Caixa 1", "0").unwrap();
        let v = vender(&l, d, "1", vec![pg("pix", "300")], None);
        c.execute("UPDATE venda SET criado_em = '2026-10-15T15:00:00Z' WHERE id = ?1", [&v]).unwrap();
        comissoes::fechar(c, d, &l.u, &l.u, "01/10/2026", "31/10/2026", "").unwrap();
        c.execute("UPDATE comissao_fechamento SET criado_em = '2026-11-01T12:00:00Z'", []).unwrap();
        let item = devolucoes::da_venda(c, &v).unwrap().itens[0].venda_item_id.clone();
        devolucoes::devolver(
            c,
            d,
            &l.u,
            &devolucoes::Entrada {
                venda_id: v.clone(),
                itens: vec![devolucoes::ItemEntrada { venda_item_id: item, quantidade: "1".into(), defeito: false }],
                destino: "vale".into(),
                motivo: "Não serviu".into(),
                ..Default::default()
            },
        )
        .unwrap();
        c.execute("UPDATE devolucao SET criado_em = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-1 day')", []).unwrap();
    }
    let ontem: String =
        l.b.conn.query_row("SELECT strftime('%d/%m/%Y', 'now', 'localtime', '-1 day')", [], |x| x.get(0)).unwrap();
    (l, ontem)
}

/// B3 da revisão R1 da 006 (R23).
/// (a) O vendedor desativado que só tem, no período, a devolução a descontar aparece no relatório. Antes sumia, e o
/// desconto junto.
/// (b) Desfazer o fechamento de outubro depois de um fechamento mais novo já ter descontado a devolução é recusado,
/// dizendo qual desfazer antes. Antes, a devolução voltava para outubro e descontava duas vezes.
#[test]
fn b3_r23_vendedor_desativado_e_fechamento_mais_novo() {
    let (l, ontem) = cenario_devolucao_depois_do_fechamento();
    let c = &l.b.conn;
    let d = &l.b.dispositivo;
    // (a)
    c.execute("UPDATE usuario SET ativo = 0 WHERE id = ?1", [&l.u]).unwrap();
    let r = comissoes::relatorio(c, &ontem, &ontem).unwrap();
    assert_eq!(r.vendedores.iter().map(|v| v.comissao.as_str()).collect::<Vec<_>>(), ["-15,00"], "o desconto sumiu");
    c.execute("UPDATE usuario SET ativo = 1 WHERE id = ?1", [&l.u]).unwrap();
    // (b) Uma venda de R$ 600 ontem (R$ 30,00) e o fechamento de ontem: R$ 15,00, já com a devolução descontada.
    let v2 = vender(&l, d, "2", vec![pg("pix", "600")], None);
    c.execute("UPDATE venda SET criado_em = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-1 day') WHERE id = ?1", [&v2])
        .unwrap();
    comissoes::fechar(c, d, &l.u, &l.u, &ontem, &ontem, "").unwrap();
    let fechamentos = comissoes::relatorio(c, "01/10/2026", "31/10/2026").unwrap().fechamentos;
    let id_de = |periodo: &str| fechamentos.iter().find(|f| f.periodo == periodo).unwrap().id.clone();
    let (outubro, novo) = (id_de("01/10/2026 a 31/10/2026"), id_de(&format!("{ontem} a {ontem}")));
    assert_eq!(fechamentos.iter().find(|f| f.id == novo).unwrap().valor, "15,00");
    let e = comissoes::reabrir(c, d, &l.u, &outubro).unwrap_err().to_string();
    assert!(e.contains(&format!("Desfaça antes o fechamento de {ontem} a {ontem}")), "{e}");
    // Desfeito o mais novo antes, outubro desfaz, e a devolução volta para outubro (uma vez só).
    comissoes::reabrir(c, d, &l.u, &novo).unwrap();
    comissoes::reabrir(c, d, &l.u, &outubro).unwrap();
    assert_eq!(comissoes::relatorio(c, "01/10/2026", "31/10/2026").unwrap().vendedores[0].comissao, "0,00");
    assert_eq!(comissoes::relatorio(c, &ontem, &ontem).unwrap().vendedores[0].comissao, "30,00");
}

// ---------------------------------------------------------------------------
// 6.6: regras e limites já aprovados, fixados por teste (C03, C05/DP-34, C06).
// ---------------------------------------------------------------------------

fn pasta_nova(nome: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("rustinho-f6-{nome}-{}-{}", std::process::id(), crate::tempo::novo_id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn arquivos_em(pasta: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(pasta)
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// C05 · DP-34 (regra aprovada, Grupo I): as cópias de antes da atualização
/// (`pre-migracao/`) e os bancos de uma atualização que falhou nunca são
/// apagados: nem pela atualização seguinte, nem pela retenção dos backups
/// (mesmo com a pasta do backup igual à dos dados e retenção de 1 dia), nem
/// ao abrir de novo.
#[test]
fn dp34_copias_de_antes_da_atualizacao_nunca_sao_apagadas() {
    let pasta = pasta_nova("dp34");
    let db = pasta.join("rustinho.db");
    {
        let mut c = Connection::open(&db).unwrap();
        c.pragma_update(None, "journal_mode", "WAL").unwrap();
        crate::banco::registrar_funcoes(&c).unwrap();
        crate::banco::migrar_ate(&mut c, 22).unwrap();
        c.execute("INSERT INTO config (chave, valor) VALUES ('dispositivo_id', 'disp-dp34')", []).unwrap();
    }
    // O que ficou de atualizações anteriores.
    let pre = pasta.join("pre-migracao");
    std::fs::create_dir_all(&pre).unwrap();
    let antiga = "rustinho.db.antes-v20-20250101120000-abc123.db";
    std::fs::write(pre.join(antiga), b"copia de antes da 0.9.12").unwrap();
    let falha = "falha-migracao-20250102120000-def456.db";
    std::fs::write(pasta.join(falha), b"banco de uma atualizacao que falhou").unwrap();
    // A atualização de agora (22 → a desta versão) guarda mais uma cópia.
    drop(Banco::abrir(&db).unwrap());
    let copias = arquivos_em(&pre);
    assert_eq!(copias.len(), 2, "a cópia nova e a antiga: {copias:?}");
    assert!(copias.iter().any(|c| c == antiga));
    // Backup na pasta dos dados, retenção de 1 dia, com backups velhos.
    {
        let b = Banco::abrir(&db).unwrap();
        crate::config::gravar(&b.conn, "backup_pasta", &pasta.display().to_string()).unwrap();
        crate::config::gravar(&b.conn, "backup_dias", "1").unwrap();
    }
    let velhos: Vec<String> = (1..=6).map(|d| format!("rustinho-2025010{d}-120000-caixa.rbk")).collect();
    for v in &velhos {
        std::fs::write(pasta.join(v), b"backup antigo").unwrap();
    }
    crate::backup::fazer(&db, false).unwrap();
    assert!(velhos.iter().any(|v| !pasta.join(v).exists()), "a retenção dos backups rodou");
    assert_eq!(arquivos_em(&pre), copias, "a retenção dos backups não mexe nas cópias de antes da atualização");
    assert!(pasta.join(falha).exists(), "o banco da atualização que falhou fica");
    drop(Banco::abrir(&db).unwrap());
    assert_eq!(arquivos_em(&pre), copias);
    assert!(pasta.join(falha).exists());
    let _ = std::fs::remove_dir_all(&pasta);
}

fn bruto(s: &mut crate::Servico, cmd: &str, d: Value) -> Value {
    serde_json::from_str(&s.chamar(cmd, &d.to_string())).unwrap()
}

/// C03 (o limite antigo, resolvido pelo R0 do QT-1): a chave da operação
/// (FALHA-04) ficava só na memória e, fechado e aberto o programa, voltava a
/// valer. Com o R0 ela fica no banco, na transação da própria operação: a
/// mesma tentativa repetida não grava de novo, nem com o programa aberto nem
/// depois de reabrir, e devolve o resultado da primeira vez (DR-7 (b)). As
/// telas continuam gerando chave nova ao abrir cada janela de gravação.
#[test]
fn c03_chave_da_operacao_vale_tambem_depois_de_reabrir() {
    let pasta = pasta_nova("c03");
    let db = pasta.join("rustinho.db");
    let sup = json!({ "tipo": "suprimento", "valor": "10", "motivo": "troco", "chaveOperacao": "sup-1" });
    {
        let mut s = crate::Servico::abrir_para_teste(&db, false).unwrap();
        let r = bruto(
            &mut s,
            "configurar_inicio",
            json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "12345678", "senha2": "12345678" }),
        );
        assert_eq!(r["ok"], true, "{r}");
        assert_eq!(bruto(&mut s, "caixa_abrir", json!({ "valor": "0", "chaveOperacao": "abrir-1" }))["ok"], true);
        assert_eq!(bruto(&mut s, "caixa_movimentar", sup.clone())["ok"], true);
        let r = bruto(&mut s, "caixa_movimentar", sup.clone());
        assert_eq!((r["ok"].clone(), r["repetida"].clone()), (json!(true), json!(true)), "{r}");
    }
    let mut s = crate::Servico::abrir_para_teste(&db, false).unwrap();
    let ana = texto(&s.banco_para_teste().conn, "SELECT id FROM usuario WHERE nome = 'Ana'", []);
    assert_eq!(bruto(&mut s, "entrar", json!({ "id": ana, "senha": "12345678" }))["ok"], true);
    let r = bruto(&mut s, "caixa_movimentar", sup);
    assert_eq!(
        (r["ok"].clone(), r["repetida"].clone()),
        (json!(true), json!(true)),
        "programa aberto de novo: a chave antiga é lembrada (R0): {r}"
    );
    assert_eq!(
        contar(&s.banco_para_teste().conn, "SELECT COUNT(*) FROM caixa_movimento WHERE tipo = 'suprimento'", []),
        1,
        "um suprimento só"
    );
    drop(s);
    // As janelas de gravação do Caixa pegam chave nova ao abrir.
    let tela = include_str!("../../app/qml/TelaCaixa.qml");
    assert!(tela.matches("chave = Api.novaChave()").count() >= 2, "movimento e fechamento");
    let _ = std::fs::remove_dir_all(&pasta);
}

/// C06 (decisão técnica): "leitura não grava" só é fiscalizado nos testes
/// (`LEITURA_ESTRITA`); por isso a suíte `so_leitura_nao_grava` tem de rodar
/// no portão da CI: o filtro dos testes sem rede não pode pulá-la.
#[test]
fn c06_leitura_nao_grava_roda_no_portao_da_ci() {
    let ci = include_str!("../../.github/workflows/validar.yml");
    assert!(include_str!("servico/api.rs").contains("fn so_leitura_nao_grava()"));
    let nome = "servico::api::testes::so_leitura_nao_grava";
    let pular = ci.lines().find(|l| l.trim_start().starts_with("PULAR_REDE:")).expect("PULAR_REDE na CI");
    let filtros: Vec<&str> = pular.split("--skip").skip(1).map(|f| f.trim().trim_matches('"').trim()).collect();
    assert!(!filtros.is_empty());
    for f in filtros {
        assert!(!nome.contains(f), "a CI pula a suíte so_leitura_nao_grava pelo filtro {f:?}");
    }
    assert!(ci.contains("cargo test -p rustinho-base -p rustinho-nucleo -- $PULAR_REDE"));
}

// ---------------------------------------------------------------------------
// Revisão de consistência das correções já feitas (E01, E10, E13, E14, F02).
// ---------------------------------------------------------------------------

fn auditado(c: &Connection, entidade: &str, id: &str, acao: &str) -> i64 {
    contar(
        c,
        "SELECT COUNT(*) FROM auditoria WHERE entidade = ?1 AND entidade_id = ?2 AND acao = ?3",
        params![entidade, id, acao],
    )
}

/// E10 · revisão (achado novo): o que a VENDA faz nas encomendas e na OS
/// fica sem registro no histórico delas (a encomenda entregue pela venda; a
/// encomenda paga na venda cancelada junto com ela; a encomenda entregue que
/// volta a "chegou" e a OS entregue que volta a "pronta" quando a venda é
/// cancelada), e o orçamento cancelado também. O E10 cobriu as mesmas
/// mudanças feitas pela compra e pelas telas da encomenda e da OS.
#[test]
fn e10_revisao_a_venda_muda_encomenda_e_os_com_registro() {
    let l = loja();
    let c = &l.b.conn;
    let d = l.b.dispositivo.clone();
    caixa::abrir(c, &d, &l.u, "Caixa 1", "0").unwrap();
    // 1. Encomenda paga na venda (faltaram 2) e cancelada com ela.
    let e = VendaEntrada {
        itens: vec![ItemEntrada {
            produto_id: l.peca.clone(),
            quantidade: "12".into(),
            encomendar: true,
            ..Default::default()
        }],
        tabela: 1,
        cliente_id: l.cliente.clone(),
        pagamentos: vec![pg("pix", "3600")],
        ..Default::default()
    };
    let v1 = vendas::concluir(c, &d, &l.u, &e).unwrap();
    let enc1 = texto(c, "SELECT id FROM encomenda WHERE venda_id = ?1", [&v1]);
    vendas::cancelar(c, &d, &l.u, &v1, "desistiu").unwrap();
    assert_eq!(texto(c, "SELECT situacao FROM encomenda WHERE id = ?1", [&enc1]), "cancelada");
    assert_eq!(auditado(c, "encomenda", &enc1, "encomenda.cancelar"), 1, "encomenda cancelada pela venda");
    // 2. Encomenda que chegou, entregue na venda, e a venda cancelada.
    let enc2 = crate::encomendas::criar(
        c,
        &d,
        &l.u,
        &crate::encomendas::EncomendaEntrada {
            cliente_id: l.cliente.clone(),
            produto_id: l.peca.clone(),
            quantidade: "1".into(),
            ..Default::default()
        },
    )
    .unwrap();
    crate::encomendas::chegou(c, &d, &l.u, &enc2).unwrap();
    let e = VendaEntrada {
        itens: vec![ItemEntrada { produto_id: l.peca.clone(), quantidade: "1".into(), ..Default::default() }],
        tabela: 1,
        cliente_id: l.cliente.clone(),
        pagamentos: vec![pg("pix", "300")],
        encomenda_id: enc2.clone(),
        ..Default::default()
    };
    let v2 = vendas::concluir(c, &d, &l.u, &e).unwrap();
    assert_eq!(auditado(c, "encomenda", &enc2, "encomenda.entregar"), 1, "encomenda entregue pela venda");
    vendas::cancelar(c, &d, &l.u, &v2, "engano").unwrap();
    assert_eq!(texto(c, "SELECT situacao FROM encomenda WHERE id = ?1", [&enc2]), "chegou");
    assert_eq!(auditado(c, "encomenda", &enc2, "encomenda.voltou_venda"), 1, "voltou a chegou pela venda cancelada");
    // 3. OS entregue na venda, e a venda cancelada.
    let os = crate::oficina::salvar(
        c,
        &d,
        &l.u,
        &crate::oficina::Entrada {
            cliente_id: l.cliente.clone(),
            placa: "ABC1D23".into(),
            defeito: "Barulho".into(),
            itens: vec![crate::oficina::ItemEntradaOs {
                tipo: "servico".into(),
                descricao: "Revisão".into(),
                preco: "50".into(),
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .unwrap();
    let pv = vendas::os_para_venda(c, &os).unwrap();
    let e = VendaEntrada {
        itens: pv.itens,
        tabela: 1,
        cliente_id: pv.cliente_id,
        os_id: os.clone(),
        pagamentos: vec![pg("pix", "50")],
        ..Default::default()
    };
    let v3 = vendas::concluir(c, &d, &l.u, &e).unwrap();
    assert_eq!(auditado(c, "ordem_servico", &os, "os.entregar"), 1);
    vendas::cancelar(c, &d, &l.u, &v3, "engano").unwrap();
    assert_eq!(texto(c, "SELECT situacao FROM ordem_servico WHERE id = ?1", [&os]), "pronta");
    assert_eq!(auditado(c, "ordem_servico", &os, "os.situacao"), 1, "a OS voltou a pronta pela venda cancelada");
    // 4. Orçamento cancelado.
    let o = vendas::salvar_orcamento(
        c,
        &d,
        &l.u,
        "Caixa 1",
        &VendaEntrada {
            itens: vec![ItemEntrada { produto_id: l.peca.clone(), quantidade: "1".into(), ..Default::default() }],
            tabela: 1,
            ..Default::default()
        },
    )
    .unwrap();
    vendas::cancelar(c, &d, &l.u, &o, "cliente desistiu").unwrap();
    assert_eq!(auditado(c, "venda", &o, "orcamento.cancelar"), 1, "orçamento cancelado");
}

/// E13 · revisão (achado novo): a lista de compra impressa agrupa as peças
/// pelo NOME do fornecedor que a tela mostra. Dois fornecedores diferentes
/// com o mesmo nome (duas filiais com a mesma fantasia, por exemplo) saem
/// juntos, num grupo só, e quem compra não sabe de quem pedir cada peça.
#[test]
fn e13_revisao_lista_de_compra_separa_fornecedores_de_mesmo_nome() {
    let l = loja();
    let c = &l.b.conn;
    let d = &l.b.dispositivo;
    let mut pecas = Vec::new();
    for nome in ["Filtro de ar", "Filtro de cabine"] {
        let mut f = produtos::ficha_nova(c).unwrap();
        f.nome = nome.into();
        f.custo = "20".into();
        f.estoque_minimo = "2".into();
        pecas.push(produtos::salvar(c, d, &l.u, &f).unwrap());
    }
    for (i, peca) in pecas.iter().enumerate() {
        let mut f = pessoas::ficha_nova(c, "fornecedor").unwrap();
        f.nome = format!("Distribuidora Sul Filial {i}");
        f.fantasia = "Distribuidora Sul".into();
        let forn = pessoas::salvar(c, d, &l.u, &f).unwrap();
        let compra = format!("compra-e13-{i}");
        c.execute(
            "INSERT INTO compra (id, fornecedor_id, total, situacao, lancada_em, criado_em, dispositivo)
             VALUES (?1, ?2, 4000, 'lancada', ?3, ?3, ?4)",
            params![compra, forn, crate::tempo::agora(), d],
        )
        .unwrap();
        c.execute(
            "INSERT INTO compra_item (id, compra_id, ordem, codigo_fornecedor, quantidade, valor_produtos, produto_id)
             VALUES (?1, ?2, 1, ?3, 2000, 4000, ?4)",
            params![format!("{compra}-item"), compra, format!("SUL-{i}"), peca],
        )
        .unwrap();
    }
    let s = crate::reposicao::sugestao(c, "").unwrap();
    assert_eq!(s.itens.len(), 2);
    assert!(s.itens.iter().all(|i| i.fornecedor == "Distribuidora Sul"), "os dois aparecem com o mesmo nome");
    let html = crate::reposicao::lista_compra_html(c).unwrap();
    assert_eq!(html.matches(">Distribuidora Sul</p>").count(), 2, "um grupo para cada fornecedor: {html}");
}

/// E01 · DA-F6-2, decidida pelo proprietário em 25/11/2026 (regra 25): venda em
/// dinheiro de 3 dias atrás, caixa fechado, cancelada hoje sem gaveta aberta (D-14:
/// os R$ 300 saem do Caixa geral). A entrada fica no dia da venda (o dia fechado
/// não muda) e o cancelamento aparece hoje, à parte, com o mesmo valor. O retrato
/// de antes (a entrada sumia e a saída não aparecia) saiu com a decisão. O
/// recebimento estornado e a devolução desfeita não fazem parte desta decisão.
#[test]
fn e01_r25_venda_cancelada_depois_do_fechamento_fica_no_dia_dela() {
    let l = loja();
    let c = &l.b.conn;
    let d = &l.b.dispositivo;
    caixa::abrir(c, d, &l.u, "Caixa 1", "0").unwrap();
    let v = vender(&l, d, "1", vec![pg("dinheiro", "300")], None);
    let dia: String = c.query_row("SELECT date('now', 'localtime', '-3 days')", [], |x| x.get(0)).unwrap();
    c.execute("UPDATE venda SET criado_em = datetime(?1 || ' 15:00:00', 'utc') WHERE id = ?2", params![dia, v])
        .unwrap();
    let centavos = |t: &str| crate::numeros::ler_dinheiro(t).unwrap().unwrap_or(0);
    let fluxo = || {
        let f = financeiro::fluxo(c, &crate::fiscal::data_br(&dia), "", 30).unwrap();
        (centavos(&f.entradas), centavos(&f.saidas))
    };
    assert_eq!(fluxo(), (30_000, 0), "a venda no dia dela");
    caixa::fechar(c, d, &l.u, "300", "").unwrap();
    let geral = || financeiro::saldo(c, "caixa-geral", "9999-12-31").unwrap();
    let antes = geral();
    vendas::cancelar(c, d, &l.u, &v, "devolveu o dinheiro").unwrap();
    assert_eq!(geral() - antes, -30_000, "os R$ 300 saíram do Caixa geral hoje (D-14)");
    assert_eq!(fluxo(), (30_000, 30_000), "a entrada fica no dia da venda e o cancelamento entra hoje");
}

/// E14 · DA-F6-3, decidida pelo proprietário em 25/11/2026 (regra 26: "manter; aviso
/// no manual"): o recibo do crediário reimpresso mostra o nome e o documento do
/// cadastro de HOJE (o recebimento não guarda o nome); a venda reimpressa mostra o
/// nome gravado nela. O manual avisa (R26).
#[test]
fn e14_r26_recibo_reimpresso_com_o_cadastro_de_hoje_e_aviso_no_manual() {
    let l = loja();
    let c = &l.b.conn;
    let d = &l.b.dispositivo;
    caixa::abrir(c, d, &l.u, "Caixa 1", "0").unwrap();
    let v = vender(&l, d, "1", vec![pg("crediario", "300")], None);
    let rec = receber::receber(
        c,
        d,
        &l.u,
        &receber::ReceberEntrada {
            cliente_id: l.cliente.clone(),
            forma: "dinheiro".into(),
            valor: "300".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let mut f = pessoas::ficha(c, &l.cliente).unwrap();
    f.nome = "Auto Center Zé Ltda".into();
    pessoas::salvar(c, d, &l.u, &f).unwrap();
    assert_eq!(receber::recibo(c, &rec).unwrap().cliente, "Auto Center Zé Ltda", "o recibo antigo com o nome de hoje");
    assert!(crate::impressao::recibo(c, &rec).unwrap().contains("Auto Center Zé Ltda"));
    assert_eq!(vendas::ficha(c, &v).unwrap().cliente, "Oficina do Zé", "a venda guarda o nome dela");
    let manual = include_str!("../../MANUAL.md");
    assert!(
        manual.contains("**Segunda via do recibo do crediário:** sai com o cadastro **de hoje** do cliente."),
        "o manual avisa a regra 26"
    );
}

/// F02 · revisão (observação): depois que um gerente entra e a loja ganha a
/// chave, o aviso do dia some, mas os backups feitos antes (sem tranca)
/// continuam na pasta até a retenção apagá-los; a lista do Backup mostra
/// cada um como "sem trancar". Nenhum outro caminho grava backup sem tranca
/// com a chave criada.
#[test]
fn f02_revisao_backups_de_antes_da_chave_continuam_sem_tranca() {
    let pasta = pasta_nova("f02");
    let db = pasta.join("rustinho.db");
    let u = {
        let b = Banco::abrir(&db).unwrap();
        let u = usuarios::criar(&b.conn, &b.dispositivo, "Ana", "12345678", true).unwrap().id;
        // Ana é gerente: só o cadeado de um gerente ativo vai dentro do backup (T073: um backup trancado sem
        // nenhum cadeado sai como erro, porque ninguém o abre).
        usuarios::definir_gerente(&b.conn, &u, true).unwrap();
        u
    };
    crate::backup::fazer(&db, false).unwrap();
    let backups = pasta.join("backups");
    let aviso = |c: &Connection| crate::avisos::do_dia(c).unwrap().iter().any(|a| a.id == "backup_sem_tranca");
    {
        let b = Banco::abrir(&db).unwrap();
        assert!(aviso(&b.conn), "sem a chave: avisa");
        crate::backup::gerente_conferido(&b.conn, &u, "12345678", None).unwrap();
        assert!(!aviso(&b.conn), "com a chave: o aviso some");
    }
    // Garante um nome diferente para o backup novo (o nome tem a hora em segundos).
    std::thread::sleep(std::time::Duration::from_millis(1100));
    crate::backup::fazer(&db, false).unwrap();
    let lista = crate::backup::listar(&backups);
    assert_eq!(lista.len(), 2, "{:?}", lista.iter().map(|c| &c.nome).collect::<Vec<_>>());
    assert_eq!(lista.iter().filter(|c| !c.tem_senha).count(), 1, "o de antes da chave continua sem tranca");
    assert_eq!(lista.iter().filter(|c| c.tem_senha).count(), 1, "o novo sai trancado");
    let _ = std::fs::remove_dir_all(&pasta);
}
