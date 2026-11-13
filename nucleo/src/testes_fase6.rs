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
    let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
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
    let v = vender(&l, d, "1", vec![pg("cheque", "300")], Some("000401"));
    let ch = texto(c, "SELECT id FROM cheque WHERE venda_id = ?1", [&v]);
    cheques::depositar(c, d, &l.u, &ch, &l.banco).unwrap();
    cheques::devolver(c, d, &l.u, &ch, "Sem fundos", true).unwrap();
    ch
}

/// DP-18 · retrato: devolução de cheque não tem volta. "Desfazer" recusa, e o
/// estorno do movimento manda "desfazer na lista de cheques", que recusa
/// (um círculo). A dívida e a saída da conta ficam.
#[test]
fn dp18_retrato_devolucao_de_cheque_nao_tem_volta() {
    let l = loja();
    let c = &l.b.conn;
    let d = &l.b.dispositivo;
    let ch = cheque_devolvido(&l);
    let e = cheques::desfazer(c, d, &l.u, &ch).unwrap_err().to_string();
    assert!(e.contains("Só dá para desfazer depósito ou repasse"), "{e}");
    let mov = texto(c, "SELECT id FROM movimento_conta WHERE origem_id = ?1 AND tipo = 'cheque_devolvido'", [&ch]);
    let e = financeiro::estornar_movimento(c, d, &l.u, &mov, "devolvido por engano").unwrap_err().to_string();
    assert!(e.contains("desfaça na lista de cheques"), "{e}");
    assert_eq!(receber::situacao(c, &l.cliente).unwrap().em_aberto, "300,00");
    assert_eq!(financeiro::saldo(c, &l.banco, "9999-12-31").unwrap(), 100_000, "1.000 + 300 − 300");
}

/// DP-18 · alvo (alternativa "o gerente desfaz, com auditoria"): o cheque
/// volta a depositado, a saída da conta é estornada, a dívida lançada sai
/// (sem pagamento) e o fluxo (E01) fica só com o depósito.
#[test]
#[ignore = "depende da DP-18: hoje a devolução de cheque não tem volta"]
fn dp18_alvo_gerente_desfaz_a_devolucao_do_cheque() {
    let l = loja();
    let c = &l.b.conn;
    let d = &l.b.dispositivo;
    let ch = cheque_devolvido(&l);
    cheques::desfazer(c, d, &l.u, &ch).unwrap();
    assert_eq!(texto(c, "SELECT situacao FROM cheque WHERE id = ?1", [&ch]), "depositado");
    assert_eq!(financeiro::saldo(c, &l.banco, "9999-12-31").unwrap(), 130_000);
    assert_eq!(receber::situacao(c, &l.cliente).unwrap().em_aberto, "0,00");
    let f = financeiro::fluxo(c, "", "", 30).unwrap();
    assert_eq!((f.entradas.as_str(), f.saidas.as_str()), ("300,00", "0,00"));
    assert_eq!(contar(c, "SELECT COUNT(*) FROM auditoria WHERE entidade_id = ?1 AND acao LIKE 'cheque.%'", [&ch]), 4);
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
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
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
    let bia = usuarios::criar(c, "disp", "Bia", "1234", true).unwrap().id;
    ok(&mut s, "caixa_abrir", json!({ "valor": "100" }));
    ok(&mut s, "entrar", json!({ "id": bia, "senha": "1234" }));
    (s, bia)
}

/// DP-19 · retrato (E09): para quem não vê os resultados, o "dinheiro parado"
/// esconde o custo (D-09), mas a lista do que comprar, a lista de compra
/// impressa e o "Comprado por fornecedor" do Painel mostram.
#[test]
fn dp19_retrato_custo_escondido_numa_tela_e_mostrado_em_outras() {
    let (mut s, _bia) = loja_com_bia();
    let parados = ok(&mut s, "estoque_parados", json!({ "texto": "" }));
    assert_eq!(parados["resultados"], false);
    assert_eq!(parados["valor"], "", "D-09: o valor parado (custo) escondido");
    let sug = ok(&mut s, "estoque_sugestao", json!({ "texto": "" }));
    assert_eq!(sug["itens"][0]["custo"], "150,00", "a lista do que comprar mostra o custo");
    assert_eq!(sug["custoTotal"], "300,00");
    let html =
        ok(&mut s, "impressao", json!({ "tipo": "lista_compra", "id": "" }))["html"].as_str().unwrap().to_string();
    assert!(html.contains("custo estimado R$ 300,00"), "a lista impressa mostra o custo");
    let forn = ok(&mut s, "painel_fornecedores", json!({ "dias": 30 }));
    assert_eq!(forn["total"], "1.234,00", "o comprado por fornecedor aparece");
}

/// DP-19 · alvo (alternativa "aplicar a D-09"): quem não vê os resultados não
/// recebe custo na lista do que comprar nem na impressa, nem o total comprado.
#[test]
#[ignore = "depende da DP-19: hoje a lista de compra e o Painel mandam o custo a qualquer usuário"]
fn dp19_alvo_d09_tambem_na_lista_de_compra_e_no_painel() {
    let (mut s, _bia) = loja_com_bia();
    let sug = ok(&mut s, "estoque_sugestao", json!({ "texto": "" }));
    assert_eq!((sug["itens"][0]["custo"].as_str(), sug["custoTotal"].as_str()), (Some(""), Some("")));
    let html =
        ok(&mut s, "impressao", json!({ "tipo": "lista_compra", "id": "" }))["html"].as_str().unwrap().to_string();
    assert!(!html.contains("150,00") && !html.contains("300,00"), "{html}");
    assert_eq!(ok(&mut s, "painel_fornecedores", json!({ "dias": 30 }))["total"], "");
}

/// DP-20 e DP-24 · retrato (H15, H18): quem não é gerente vê o dinheiro
/// esperado na gaveta antes de contar (a conferência não é cega), e o olho
/// do sigilo não chega à tela do Caixa (nenhum valor dela é "sensível").
#[test]
fn dp20_dp24_retrato_esperado_a_vista_e_caixa_fora_do_sigilo() {
    let (mut s, _bia) = loja_com_bia();
    let e = ok(&mut s, "caixa_estado", json!({}));
    assert_eq!(e["resumo"]["esperado"], "100,00", "o esperado vai para a tela antes de contar");
    let tela = include_str!("../../app/qml/TelaCaixa.qml");
    assert!(tela.contains("titulo: \"Dinheiro na gaveta\""));
    assert!(tela.contains("\"R$ \" + tela.r.esperado"), "o esperado à vista, grande, no resumo");
    assert!(tela.contains("Conte o dinheiro da gaveta e digite o total. O Rustinho mostra a diferença"));
    for marca in ["sensivel", "Tema.valor(", "Tema.sigilo", "Tema.oculto", "Olho"] {
        assert!(!tela.contains(marca), "a tela do Caixa já usa o sigilo ({marca})");
    }
    // Controle: o olho existe e já esconde outras telas de dinheiro.
    assert!(include_str!("../../app/qml/SecaoFluxo.qml").contains("sensivel: true"));
}

/// DP-23 · retrato (E15, G5, PEND-31): sem senha do gerente, a Bia muda o
/// limite de crédito do cliente e tira o CSOSN próprio de uma peça; as duas
/// mudanças já ficam na auditoria. A unidade que já existe não muda de
/// "vende quebrado" (a PEND-31 descreve um caminho que não existe mais: a
/// sigla repetida é recusada); a unidade nova entra na auditoria (E10).
#[test]
fn dp23_retrato_sensiveis_mudam_sem_gerente_e_com_auditoria() {
    let (mut s, _bia) = loja_com_bia();
    // Limite de crédito.
    let mut f = ok(&mut s, "pessoa_nova", json!({ "papel": "cliente" }));
    f["nome"] = json!("Oficina do Zé");
    let cli = ok(&mut s, "pessoa_salvar", f)["id"].as_str().unwrap().to_string();
    let mut f = ok(&mut s, "pessoa_abrir", json!({ "id": cli }));
    f["limiteCredito"] = json!("500");
    ok(&mut s, "pessoa_salvar", f);
    let c = &s.banco_para_teste().conn;
    let det = texto(
        c,
        "SELECT COALESCE(MAX(detalhes), '') FROM auditoria WHERE entidade_id = ?1 AND acao = 'pessoa.alterar'",
        [&cli],
    );
    assert!(det.contains("Limite de crédito"), "{det}");
    // CSOSN automático.
    let peca = texto(c, "SELECT id FROM produto WHERE nome = 'Lâmpada H4'", []);
    c.execute("UPDATE produto SET csosn = '500' WHERE id = ?1", [&peca]).unwrap();
    let r = ok(&mut s, "fiscal_automatico", json!({ "ids": [peca] }));
    assert_eq!(r["alteradas"], 1);
    let c = &s.banco_para_teste().conn;
    let det = texto(
        c,
        "SELECT COALESCE(MAX(detalhes), '') FROM auditoria WHERE entidade_id = ?1 AND acao = 'produto.alterar'",
        [&peca],
    );
    assert!(det.contains("CSOSN: 500 → automático"), "{det}");
    // Unidade: a que existe não muda; a nova entra na auditoria.
    let r = chamar(&mut s, "unidade_salvar", json!({ "sigla": "UN", "descricao": "Unidade", "fracionada": true }));
    assert_eq!(r["ok"], false, "{r}");
    let c = &s.banco_para_teste().conn;
    assert_eq!(contar(c, "SELECT fracionada FROM unidade WHERE sigla = 'UN'", []), 0);
    ok(&mut s, "unidade_salvar", json!({ "sigla": "LTS2", "descricao": "Litro avulso", "fracionada": true }));
    let c = &s.banco_para_teste().conn;
    assert_eq!(
        contar(c, "SELECT COUNT(*) FROM auditoria WHERE acao = 'unidade.criar' AND entidade_id = 'LTS2'", []),
        1
    );
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

/// DP-22 · retrato (E16): a comissão de outubro foi fechada (R$ 15,00 = 5% de
/// R$ 300). Em novembro o cliente devolve a peça. O relatório de outubro passa
/// a mostrar R$ 0,00 (o fechamento continua de R$ 15,00) e o de novembro não
/// desconta nada: a devolução não chega a período nenhum.
#[test]
fn dp22_retrato_devolucao_depois_do_fechamento_nao_desconta_em_lugar_nenhum() {
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
    let r = outubro();
    assert_eq!(r.vendedores[0].comissao, "0,00", "o período fechado recalcula");
    assert_eq!(r.fechamentos[0].valor, "15,00", "o fechamento fica");
    let novembro = comissoes::relatorio(c, "01/11/2026", "30/11/2026").unwrap();
    assert_eq!(novembro.vendedores[0].comissao, "0,00", "e o período seguinte não desconta");
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

/// C03 (limite documentado): a chave da operação (FALHA-04) fica só na
/// memória. Enquanto o programa está aberto, a mesma tentativa repetida não
/// grava duas vezes; fechado e aberto de novo, a mesma chave volta a valer.
/// Não há reenvio da operação antiga: as telas geram chave nova ao abrir
/// cada janela de gravação.
#[test]
fn c03_chave_da_operacao_vale_enquanto_o_programa_esta_aberto() {
    let pasta = pasta_nova("c03");
    let db = pasta.join("rustinho.db");
    let sup = json!({ "tipo": "suprimento", "valor": "10", "motivo": "troco", "chaveOperacao": "sup-1" });
    {
        let mut s = crate::Servico::abrir_para_teste(&db, false).unwrap();
        let r = bruto(
            &mut s,
            "configurar_inicio",
            json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
        );
        assert_eq!(r["ok"], true, "{r}");
        assert_eq!(bruto(&mut s, "caixa_abrir", json!({ "valor": "0", "chaveOperacao": "abrir-1" }))["ok"], true);
        assert_eq!(bruto(&mut s, "caixa_movimentar", sup.clone())["ok"], true);
        let r = bruto(&mut s, "caixa_movimentar", sup.clone());
        assert_eq!(r["campo"], "repetida", "{r}");
    }
    let mut s = crate::Servico::abrir_para_teste(&db, false).unwrap();
    let ana = texto(&s.banco_para_teste().conn, "SELECT id FROM usuario WHERE nome = 'Ana'", []);
    assert_eq!(bruto(&mut s, "entrar", json!({ "id": ana, "senha": "1234" }))["ok"], true);
    let r = bruto(&mut s, "caixa_movimentar", sup);
    assert_eq!(r["ok"], true, "programa aberto de novo: a chave antiga não é lembrada (limite C03): {r}");
    assert_eq!(
        contar(&s.banco_para_teste().conn, "SELECT COUNT(*) FROM caixa_movimento WHERE tipo = 'suprimento'", []),
        2
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
#[ignore = "achado da revisão do E10: a venda muda encomenda e OS (e o orçamento é cancelado) sem auditoria"]
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
