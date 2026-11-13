//! Dois (ou três) caixas trocando dados sem rede de verdade: as alterações
//! de um são aplicadas no outro como a sincronização faz (Frente D do
//! briefing de correção da 0.9.11). Tudo com dados de mentira e sem esperar
//! tempo: o resultado é sempre o mesmo.

use rustinho_nucleo::banco::Banco;
use rustinho_nucleo::{caixa, config, estoque, produtos, sync, usuarios, vendas};

/// Leva para `para` tudo o que `de` tem e ele ainda não (em lotes, como pela rede).
fn levar(de: &Banco, para: &mut Banco) -> usize {
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

/// Os dois trocam até não ter mais nada novo.
fn trocar(a: &mut Banco, b: &mut Banco) {
    for _ in 0..5 {
        let n = levar(a, b) + levar(b, a);
        if n == 0 {
            return;
        }
    }
    panic!("os caixas não pararam de trocar");
}

fn contar(b: &Banco, sql: &str) -> i64 {
    b.conn.query_row(sql, [], |l| l.get(0)).unwrap()
}

fn loja() -> (Banco, Banco, String) {
    let a = Banco::em_memoria().unwrap();
    let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "balcao-azul-2026", true).unwrap();
    usuarios::definir_gerente(&a.conn, &u.id, true).unwrap();
    config::gravar(&a.conn, "loja_nome", "Loja de Teste").unwrap();
    let mut b = Banco::em_memoria().unwrap();
    let mut a = a;
    trocar(&mut a, &mut b);
    (a, b, u.id)
}

fn peca(b: &Banco, u: &str, nome: &str, estoque: &str) -> String {
    let mut f = produtos::ficha_nova(&b.conn).unwrap();
    f.nome = nome.into();
    f.custo = "40".into();
    f.estoque_inicial = estoque.into();
    produtos::salvar(&b.conn, &b.dispositivo, u, &f).unwrap()
}

fn renomear(b: &Banco, u: &str, id: &str, nome: &str) {
    let mut f = produtos::ficha(&b.conn, id).unwrap();
    f.nome = nome.into();
    produtos::salvar(&b.conn, &b.dispositivo, u, &f).unwrap();
}

/// D.2 e C-02: o mesmo cadastro mexido nos dois caixas sem rede.
/// Início: peça igual nos dois. Evento: cada um troca o nome; depois trocam.
/// Esperado: os dois ficam com o mesmo nome (o mais novo), os dois anotam o
/// conflito, o nome que perdeu fica guardado; o gerente escolhe "usar a
/// que perdeu" e os dois caixas ficam com ela, e o conflito do outro caixa
/// se resolve sozinho.
#[test]
fn mesmo_cadastro_em_dois_caixas() {
    let (mut a, mut b, u) = loja();
    let p = peca(&a, &u, "Pastilha", "5");
    trocar(&mut a, &mut b);
    renomear(&a, &u, &p, "Pastilha dianteira");
    std::thread::sleep(std::time::Duration::from_millis(3));
    renomear(&b, &u, &p, "Pastilha traseira");
    trocar(&mut a, &mut b);
    let nome_a = produtos::ficha(&a.conn, &p).unwrap().nome;
    let nome_b = produtos::ficha(&b.conn, &p).unwrap().nome;
    assert_eq!(nome_a, nome_b, "os dois caixas ficam iguais");
    assert_eq!(nome_a, "Pastilha traseira", "vale a mais nova");
    // Os dois caixas anotaram o mesmo conflito.
    let ka = sync::conflitos(&a.conn).unwrap();
    let kb = sync::conflitos(&b.conn).unwrap();
    assert_eq!((ka.len(), kb.len()), (1, 1), "{ka:?} {kb:?}");
    assert!(ka[0].pode_usar);
    let campo = ka[0].campos.iter().find(|c| c["campo"] == "nome").expect("o nome aparece");
    assert_eq!(
        (campo["ficou"].as_str(), campo["perdeu"].as_str()),
        (Some("Pastilha traseira"), Some("Pastilha dianteira"))
    );
    // Trocar de novo não anota outra vez.
    trocar(&mut a, &mut b);
    assert_eq!(sync::conflitos_abertos(&a.conn).unwrap(), 1);
    // O gerente (no caixa A) volta a que perdeu.
    sync::resolver_conflito(&a.conn, ka[0].id, true, &u).unwrap();
    assert_eq!(produtos::ficha(&a.conn, &p).unwrap().nome, "Pastilha dianteira");
    assert!(sync::resolver_conflito(&a.conn, ka[0].id, false, &u).is_err(), "já resolvido");
    trocar(&mut a, &mut b);
    assert_eq!(produtos::ficha(&b.conn, &p).unwrap().nome, "Pastilha dianteira");
    assert_eq!(sync::conflitos_abertos(&a.conn).unwrap(), 0);
    assert_eq!(sync::conflitos_abertos(&b.conn).unwrap(), 0, "resolvido no outro caixa");
    // O estoque não mudou com tudo isso.
    assert_eq!(estoque::saldo(&a.conn, &p).unwrap(), 5000);
    assert_eq!(estoque::saldo(&b.conn, &p).unwrap(), 5000);
}

/// Mexer depois de ver a do outro não é conflito.
#[test]
fn em_sequencia_nao_e_conflito() {
    let (mut a, mut b, u) = loja();
    let p = peca(&a, &u, "Filtro", "1");
    trocar(&mut a, &mut b);
    renomear(&a, &u, &p, "Filtro de óleo");
    trocar(&mut a, &mut b);
    renomear(&b, &u, &p, "Filtro de óleo PH");
    trocar(&mut a, &mut b);
    assert_eq!(produtos::ficha(&a.conn, &p).unwrap().nome, "Filtro de óleo PH");
    assert_eq!(sync::conflitos_abertos(&a.conn).unwrap(), 0);
    assert_eq!(sync::conflitos_abertos(&b.conn).unwrap(), 0);
}

/// D.3 e D.5: lote repetido, fora de ordem e cortado no meio (queda da
/// rede antes de confirmar). Nada é aplicado duas vezes.
#[test]
fn lote_repetido_fora_de_ordem_e_cortado() {
    let (mut a, mut b, u) = loja();
    caixa::abrir(&a.conn, &a.dispositivo, &u, "Caixa 1", "0").unwrap();
    let p = peca(&a, &u, "Óleo", "10");
    let e = vendas::VendaEntrada {
        itens: vec![vendas::ItemEntrada { produto_id: p.clone(), quantidade: "2".into(), ..Default::default() }],
        tabela: 1,
        pagamentos: vec![vendas::PagamentoEntrada {
            forma: "dinheiro".into(),
            valor: "200".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    vendas::concluir(&a.conn, &a.dispositivo, &u, &e).unwrap();
    let (lista, _) = sync::alteracoes_desde(&a.conn, 0, 10_000).unwrap();
    // Cortado: só a primeira metade, ao contrário (filhos antes dos pais).
    let metade: Vec<_> = lista[..lista.len() / 2].iter().rev().cloned().collect();
    sync::aplicar(&mut b.conn, &metade).unwrap();
    // Depois tudo, duas vezes, de trás para frente.
    let invertida: Vec<_> = lista.iter().rev().cloned().collect();
    sync::aplicar(&mut b.conn, &invertida).unwrap();
    assert_eq!(sync::aplicar(&mut b.conn, &invertida).unwrap(), 0, "repetido não conta");
    assert_eq!(contar(&b, "SELECT COUNT(*) FROM venda WHERE tipo = 'venda'"), 1);
    assert_eq!(contar(&b, "SELECT COUNT(*) FROM venda_item"), 1);
    assert_eq!(estoque::saldo(&b.conn, &p).unwrap(), 8000);
    assert_eq!(sync::pendencias(&b.conn).unwrap(), 0, "no fim, nada aponta para o que não existe");
    trocar(&mut a, &mut b);
    assert_eq!(estoque::saldo(&a.conn, &p).unwrap(), 8000);
}

/// D.4: item que aponta para uma peça que ainda não chegou fica como
/// pendência (sem travar a troca) e some quando a peça chega.
#[test]
fn referencia_que_ainda_nao_chegou() {
    let (mut a, mut b, u) = loja();
    let p = peca(&a, &u, "Correia", "3");
    let (lista, _) = sync::alteracoes_desde(&a.conn, 0, 10_000).unwrap();
    let sem_a_peca: Vec<_> = lista.iter().filter(|x| x.tabela != "produto").cloned().collect();
    sync::aplicar(&mut b.conn, &sem_a_peca).unwrap();
    assert!(sync::pendencias(&b.conn).unwrap() > 0);
    trocar(&mut a, &mut b);
    assert_eq!(sync::pendencias(&b.conn).unwrap(), 0);
    assert_eq!(estoque::saldo(&b.conn, &p).unwrap(), 3000);
}

/// F-06 e D.1: caixa sem rede por muito tempo; o outro mexe muito, apaga e
/// compacta o histórico. Ao voltar, o atrasado fica igual, sem ressuscitar
/// o que foi apagado.
#[test]
fn caixa_atrasado_depois_de_compactar() {
    let (mut a, mut b, u) = loja();
    let p = peca(&a, &u, "Vela", "4");
    let fica = peca(&a, &u, "Cabo", "1");
    trocar(&mut a, &mut b);
    for i in 0..40 {
        renomear(&a, &u, &p, &format!("Vela {i}"));
    }
    // Apaga um registro (a aplicação da peça: tabela com "apagar").
    let mut f = produtos::ficha(&a.conn, &fica).unwrap();
    f.aplicacoes =
        vec![produtos::AplicacaoFicha { montadora: "Fiat".into(), modelo: "Uno".into(), ..Default::default() }];
    produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
    let mut f = produtos::ficha(&a.conn, &fica).unwrap();
    f.aplicacoes.clear();
    produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
    assert!(sync::compactar(&a.conn).unwrap() >= 39);
    // Um terceiro caixa, novo, recebe só o histórico compactado.
    let mut c = Banco::em_memoria().unwrap();
    trocar(&mut a, &mut b);
    trocar(&mut a, &mut c);
    for x in [&a, &b, &c] {
        assert_eq!(produtos::ficha(&x.conn, &p).unwrap().nome, "Vela 39");
        assert!(produtos::ficha(&x.conn, &fica).unwrap().aplicacoes.is_empty(), "o apagado não volta");
        assert_eq!(estoque::saldo(&x.conn, &p).unwrap(), 4000);
    }
    assert_eq!(sync::conflitos_abertos(&b.conn).unwrap(), 0);
}

/// F-12: o mesmo orçamento vendido em dois caixas sem rede. Os dois caixas
/// mostram o conflito para o gerente (as duas vendas existem de verdade).
#[test]
fn mesmo_orcamento_em_dois_caixas() {
    let (mut a, mut b, u) = loja();
    let p = peca(&a, &u, "Amortecedor", "5");
    caixa::abrir(&a.conn, &a.dispositivo, &u, "Caixa 1", "0").unwrap();
    let mut e = vendas::VendaEntrada {
        itens: vec![vendas::ItemEntrada { produto_id: p.clone(), quantidade: "1".into(), ..Default::default() }],
        tabela: 1,
        ..Default::default()
    };
    let orc = vendas::salvar_orcamento(&a.conn, &a.dispositivo, &u, "Caixa 1", &e).unwrap();
    trocar(&mut a, &mut b);
    caixa::abrir(&b.conn, &b.dispositivo, &u, "Caixa 2", "0").unwrap();
    e.orcamento_id = orc.clone();
    e.pagamentos = vec![vendas::PagamentoEntrada { forma: "pix".into(), valor: "80".into(), ..Default::default() }];
    vendas::concluir(&a.conn, &a.dispositivo, &u, &e).unwrap();
    vendas::concluir(&b.conn, &b.dispositivo, &u, &e).unwrap();
    // No mesmo caixa, a segunda vez é recusada.
    assert!(vendas::concluir(&a.conn, &a.dispositivo, &u, &e).is_err());
    trocar(&mut a, &mut b);
    for x in [&a, &b] {
        let k = sync::conflitos(&x.conn).unwrap();
        assert!(k.iter().any(|c| c.tipo == "orcamento" && c.texto.contains("mais de uma venda")), "{k:?}");
        assert!(!k.iter().find(|c| c.tipo == "orcamento").unwrap().pode_usar);
    }
    assert_eq!(estoque::saldo(&a.conn, &p).unwrap(), estoque::saldo(&b.conn, &p).unwrap());
}

/// C-06: cada caixa tira um gerente sem ver o outro. Depois de trocar, a
/// loja não fica sem gerente, e os dois caixas escolhem o mesmo.
#[test]
fn loja_nunca_fica_sem_gerente() {
    let (mut a, mut b, ana) = loja();
    let bia = usuarios::criar(&a.conn, &a.dispositivo, "Bia", "oficina-verde-77", true).unwrap();
    usuarios::definir_gerente(&a.conn, &bia.id, true).unwrap();
    trocar(&mut a, &mut b);
    usuarios::definir_gerente(&a.conn, &ana, false).unwrap();
    usuarios::definir_gerente(&b.conn, &bia.id, false).unwrap();
    trocar(&mut a, &mut b);
    for x in [&a, &b] {
        assert_eq!(contar(x, "SELECT COUNT(*) FROM usuario WHERE ativo = 1 AND gerente = 1"), 1);
        assert!(usuarios::buscar(&x.conn, &ana).unwrap().gerente, "o mais antigo volta a ser gerente");
    }
    // Fica anotado no caixa que percebeu (para o gerente conferir).
    let anotado = |x: &Banco| sync::conflitos(&x.conn).unwrap().iter().any(|c| c.tipo == "gerente");
    assert!(anotado(&a) || anotado(&b));
}

/// F-16: três caixas criam um usuário com o mesmo nome (com e sem acento).
/// Depois de trocar, os nomes ficam diferentes e iguais em todos os caixas.
#[test]
fn tres_usuarios_com_o_mesmo_nome() {
    let (mut a, mut b, _) = loja();
    let mut c = Banco::em_memoria().unwrap();
    trocar(&mut a, &mut c);
    usuarios::criar(&a.conn, &a.dispositivo, "José", "1111", true).unwrap();
    usuarios::criar(&b.conn, &b.dispositivo, "Jose", "2222", true).unwrap();
    usuarios::criar(&c.conn, &c.dispositivo, "JOSÉ", "3333", true).unwrap();
    for _ in 0..2 {
        trocar(&mut a, &mut b);
        trocar(&mut b, &mut c);
        trocar(&mut a, &mut c);
        for x in [&a, &b, &c] {
            usuarios::resolver_nomes_repetidos(&x.conn).unwrap();
        }
    }
    let nomes = |x: &Banco| -> Vec<String> {
        let mut v: Vec<String> = usuarios::listar(&x.conn, false).unwrap().into_iter().map(|u| u.nome).collect();
        v.sort();
        v
    };
    let n = nomes(&a);
    assert_eq!(n.len(), 4);
    let mut unicos: Vec<String> = n
        .iter()
        .map(|x| x.to_lowercase().replace('é', "e"))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    unicos.dedup();
    assert_eq!(unicos.len(), 4, "{n:?}");
    assert_eq!(n, nomes(&b));
    assert_eq!(n, nomes(&c));
}

/// D.6: enquanto um backup é restaurado, a troca de dados espera.
#[test]
fn restaurar_pausa_a_troca() {
    let guarda = sync::pausar();
    let (tx, rx) = std::sync::mpsc::channel();
    let t = std::thread::spawn(move || {
        // puxar pega a mesma trava (leitura) antes de qualquer coisa.
        let r = sync::puxar(std::path::Path::new("/caminho/que/nao/existe.db"), "127.0.0.1:1", 1);
        tx.send(r.is_err()).unwrap();
    });
    assert!(rx.recv_timeout(std::time::Duration::from_millis(300)).is_err(), "esperou a restauração");
    drop(guarda);
    assert!(rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap());
    t.join().unwrap();
}

// ---------------------------------------------------------------------------
// Passo 5.2 da dev.3.3 (DP-15, PEND-35, D01, D02, D05, C07): as classes da
// troca. Caracterização: cada teste descreve o comportamento CERTO e por isso
// falha em `b3a171c`, pelo motivo do achado (`#[ignore]` com o motivo). O
// commit que corrige tira o `#[ignore]`.
// ---------------------------------------------------------------------------

/// Destinatário das notas de teste do N02 (`tests/pki/xml/codigos`).
const LOJA_CNPJ: &str = "11444777000161";
/// O GTIN da nota nf601 (PATRAL, cProd 00008373).
const GTIN_PATRAL: &str = "7908168151578";
/// GTINs válidos de teste (os mesmos de `caracterizacao_dev33.rs`).
const GTIN_1: &str = "7891000000090";
const GTIN_2: &str = "7891000000113";
const NF601: &str = include_str!("pki/xml/codigos/nf601_patral_00008373.xml");

/// Um GTIN que chega por nota (como a importação grava: `registrar_gtins_xml`).
fn gtin_da_nota(b: &Banco, id: &str, gtin: &str) {
    let g = produtos::GtinFicha { gtin: gtin.into(), tipo: "comercial".into(), origem: "cEAN".into() };
    produtos::registrar_gtins_xml(&b.conn, id, &[g]).unwrap();
}

/// Peça com código de barras próprio e um GTIN a mais (só em `produto_gtin`).
fn peca_com_gtin(b: &Banco, u: &str, nome: &str, barras: &str, gtin: &str) -> String {
    let mut f = produtos::ficha_nova(&b.conn).unwrap();
    f.nome = nome.into();
    f.custo = "40".into();
    f.codigo_barras = barras.into();
    f.gtins = vec![produtos::GtinFicha { gtin: gtin.into(), tipo: "comercial".into(), origem: "manual".into() }];
    produtos::salvar(&b.conn, &b.dispositivo, u, &f).unwrap()
}

fn achou_pelo_codigo(b: &Banco, codigo: &str) -> Vec<String> {
    rustinho_nucleo::busca::pelo_codigo_exato(&b.conn, codigo).unwrap().into_iter().map(|p| p.id).collect()
}

fn achou_pelo_texto(b: &Banco, texto: &str) -> Vec<String> {
    rustinho_nucleo::busca::buscar(&b.conn, texto, false, 50).unwrap().into_iter().map(|p| p.id).collect()
}

/// (busca, codigos) gravados na peça.
fn indice(b: &Banco, id: &str) -> (String, String) {
    b.conn.query_row("SELECT busca, codigos FROM produto WHERE id = ?1", [id], |l| Ok((l.get(0)?, l.get(1)?))).unwrap()
}

fn gtins_da_peca(b: &Banco, id: &str) -> Vec<String> {
    let mut st = b.conn.prepare("SELECT gtin FROM produto_gtin WHERE produto_id = ?1 ORDER BY gtin").unwrap();
    let mut v: Vec<String> = st.query_map([id], |l| l.get(0)).unwrap().map(Result::unwrap).collect();
    v.dedup();
    v
}

/// PEND-35 e D01, cenário de 2 caixas da aceitação da 5.2: "GTIN achado em B".
/// A peça ganha em A um GTIN a mais (além do código de barras). Depois de
/// trocar, B a acha pelo GTIN, e continua achando depois de ele mesmo mexer
/// na peça (o índice que B refaz sai da peça inteira, inclusive os GTINs).
/// Em `b3a171c` o GTIN não viaja: o índice que veio de A tinha o GTIN, mas o
/// índice que B refaz ao salvar não tem, e a peça some da busca pelo GTIN em
/// B (e, pela troca, o índice sem o GTIN volta para A).
#[test]
#[ignore = "caracterização D01/PEND-35: o GTIN não viaja em b3a171c; passa no passo 5.2 (produto_gtin viaja)"]
fn d01_gtin_achado_em_b_mesmo_depois_de_b_mexer_na_peca() {
    let (mut a, mut b, u) = loja();
    let p = peca_com_gtin(&a, &u, "Rolamento roda", GTIN_1, GTIN_PATRAL);
    trocar(&mut a, &mut b);
    renomear(&b, &u, &p, "Rolamento roda traseira");
    trocar(&mut a, &mut b);
    assert_eq!(achou_pelo_codigo(&b, GTIN_PATRAL), vec![p.clone()], "B não acha a peça pelo GTIN que veio de A");
    assert_eq!(gtins_da_peca(&b, &p), vec![GTIN_1.to_string(), GTIN_PATRAL.to_string()], "os GTINs da peça em B");
    assert!(indice(&b, &p).1.contains(GTIN_PATRAL), "o índice de B sem o GTIN: {:?}", indice(&b, &p));
    assert_eq!(achou_pelo_codigo(&a, GTIN_PATRAL), vec![p.clone()]);
    assert!(indice(&a, &p).1.contains(GTIN_PATRAL), "o índice de A perdeu o GTIN: {:?}", indice(&a, &p));
}

/// D01, cenário de 2 caixas da aceitação da 5.2: "ligação automática da
/// compra em B". O GTIN da nota está na peça só como GTIN a mais (o código de
/// barras é outro), cadastrado em A. Em B, a compra da nota cai na peça pelo
/// GTIN, como cairia em A. Em `b3a171c` o GTIN não chegou a B: o item não é
/// ligado sozinho.
#[test]
#[ignore = "caracterização D01: a compra em B não acha a peça pelo GTIN em b3a171c; passa no passo 5.2"]
fn d01_ligacao_automatica_da_compra_em_b_pelo_gtin() {
    let (mut a, mut b, u) = loja();
    config::gravar(&a.conn, "loja_cnpj", LOJA_CNPJ).unwrap();
    let p = peca_com_gtin(&a, &u, "Rolamento PATRAL", GTIN_1, GTIN_PATRAL);
    trocar(&mut a, &mut b);
    // Em A, a mesma nota cairia na peça pelo GTIN (a referência).
    let em_a = rustinho_nucleo::compras::importar_xml(&a.conn, &a.dispositivo, &u, NF601).unwrap();
    let item_a = &rustinho_nucleo::compras::ficha(&a.conn, &em_a).unwrap().itens[0];
    assert_eq!((item_a.acao.as_str(), item_a.achado_por.as_str()), ("ligar", "barras"), "referência em A");
    rustinho_nucleo::compras::excluir_rascunho(&a.conn, &a.dispositivo, &u, &em_a).unwrap();
    let em_b = rustinho_nucleo::compras::importar_xml(&b.conn, &b.dispositivo, &u, NF601).unwrap();
    let item_b = &rustinho_nucleo::compras::ficha(&b.conn, &em_b).unwrap().itens[0];
    assert_eq!(
        (item_b.acao.as_str(), item_b.achado_por.as_str(), item_b.peca.as_ref().map(|x| x.id.as_str())),
        ("ligar", "barras", Some(p.as_str())),
        "em B a compra não caiu na peça pelo GTIN"
    );
}

/// D02, cenário de 2 caixas da aceitação da 5.2: "edição simultânea sem
/// conflito de derivado". Sem rede, uma nota põe um GTIN na peça em A e outra
/// nota põe outro GTIN em B (a peça já tinha código de barras). Ao trocar,
/// nada a decidir: os dois GTINs ficam nos dois caixas, e o índice também.
/// Em `b3a171c` o índice refeito em cada caixa viaja como alteração da peça,
/// e o campo derivado `codigos` vira conflito para o gerente (e cada caixa
/// fica só com o seu GTIN).
#[test]
#[ignore = "caracterização D02: conflito falso em `codigos` em b3a171c; passa no passo 5.2 (derivados não viajam)"]
fn d02_gtins_diferentes_nos_dois_caixas_sem_conflito_de_derivado() {
    let (mut a, mut b, u) = loja();
    let p = peca_com_gtin(&a, &u, "Pastilha", GTIN_PATRAL, GTIN_PATRAL);
    trocar(&mut a, &mut b);
    gtin_da_nota(&a, &p, GTIN_1);
    std::thread::sleep(std::time::Duration::from_millis(3));
    gtin_da_nota(&b, &p, GTIN_2);
    trocar(&mut a, &mut b);
    for x in [&a, &b] {
        let k = sync::conflitos(&x.conn).unwrap();
        let derivado = k.iter().any(|c| c.campos.iter().any(|f| f["campo"] == "codigos" || f["campo"] == "busca"));
        assert!(!derivado, "conflito em campo derivado: {k:?}");
        assert!(k.is_empty(), "nada a decidir: {k:?}");
    }
    let todos = vec![GTIN_1.to_string(), GTIN_2.to_string(), GTIN_PATRAL.to_string()];
    for x in [&a, &b] {
        assert_eq!(gtins_da_peca(x, &p), todos);
        assert_eq!(achou_pelo_codigo(x, GTIN_1), vec![p.clone()]);
        assert_eq!(achou_pelo_codigo(x, GTIN_2), vec![p.clone()]);
    }
    assert_eq!(indice(&a, &p), indice(&b, &p), "o índice é o mesmo nos dois caixas");
}

/// D05: "usar a versão que perdeu" deixa o índice coerente com a peça como
/// ela fica (aplicações, equivalentes e GTINs). Sem rede, A troca o nome; B
/// põe uma aplicação (Corsa) e troca o nome depois (a de B vale). O gerente,
/// em A, volta a de A: a peça fica com o nome de A e a aplicação de B. Em
/// `b3a171c` a versão que volta traz o índice velho de A (sem o Corsa): a
/// busca por "corsa" não acha a peça.
#[test]
fn d05_usar_a_versao_que_perdeu_refaz_o_indice() {
    let (mut a, mut b, u) = loja();
    let p = peca(&a, &u, "Pastilha", "5");
    trocar(&mut a, &mut b);
    renomear(&a, &u, &p, "Pastilha dianteira");
    std::thread::sleep(std::time::Duration::from_millis(3));
    let mut f = produtos::ficha(&b.conn, &p).unwrap();
    f.nome = "Pastilha traseira".into();
    f.aplicacoes =
        vec![produtos::AplicacaoFicha { montadora: "Chevrolet".into(), modelo: "Corsa".into(), ..Default::default() }];
    produtos::salvar(&b.conn, &b.dispositivo, &u, &f).unwrap();
    trocar(&mut a, &mut b);
    let k = sync::conflitos(&a.conn).unwrap();
    let c = k.iter().find(|c| c.campos.iter().any(|f| f["campo"] == "nome")).expect("o conflito do nome");
    sync::resolver_conflito(&a.conn, c.id, true, &u).unwrap();
    trocar(&mut a, &mut b);
    for x in [&a, &b] {
        let ficha = produtos::ficha(&x.conn, &p).unwrap();
        assert_eq!(ficha.nome, "Pastilha dianteira");
        assert_eq!(ficha.aplicacoes.len(), 1, "a aplicação de B fica");
        assert_eq!(achou_pelo_texto(x, "corsa"), vec![p.clone()], "a busca pela aplicação não acha a peça");
        let gravado = indice(x, &p);
        produtos::atualizar_indice_busca(&x.conn, &p).unwrap();
        assert_eq!(gravado, indice(x, &p), "o índice gravado não é o da peça como ela ficou");
    }
}

/// C07: entrar no caixa não gera alteração que viaja (`ultimo_acesso` é deste
/// caixa). Em `b3a171c`, cada entrada grava uma alteração de `usuario` que
/// vai para todos os caixas.
#[test]
fn c07_entrar_nao_gera_alteracao_que_viaja() {
    let (mut a, mut b, u) = loja();
    let antes = contar(&a, "SELECT COUNT(*) FROM alteracao");
    usuarios::registrar_acesso(&a.conn, &u).unwrap();
    assert_eq!(contar(&a, "SELECT COUNT(*) FROM alteracao"), antes, "entrar gravou uma alteração que viaja");
    trocar(&mut a, &mut b);
    let ultimo = |x: &Banco| -> Option<String> {
        x.conn.query_row("SELECT ultimo_acesso FROM usuario WHERE id = ?1", [&u], |l| l.get(0)).unwrap()
    };
    assert!(ultimo(&a).is_some());
    assert_eq!(ultimo(&b), None, "o último acesso de A chegou a B");
    // O resto do usuário continua viajando.
    let bia = usuarios::criar(&a.conn, &a.dispositivo, "Bia", "oficina-verde-77", true).unwrap();
    trocar(&mut a, &mut b);
    assert!(usuarios::buscar(&b.conn, &bia.id).is_ok(), "o usuário novo chegou a B");
}

/// PEND-35: os GTINs que já existiam antes de a tabela passar a viajar (bancos
/// de antes da 5.2: cada caixa tem os seus) vão para os outros caixas uma vez,
/// ao abrir; o mesmo GTIN, já gravado nos dois caixas com ids diferentes,
/// fica uma vez só em cada um, sem travar a troca. Em `b3a171c` nada disso
/// viaja.
#[test]
#[ignore = "caracterização PEND-35: produto_gtin é local em b3a171c; passa no passo 5.2"]
fn pend35_gtins_de_antes_viajam_uma_vez_sem_repetir() {
    let (mut a, mut b, u) = loja();
    let p = peca(&a, &u, "Filtro de ar", "1");
    trocar(&mut a, &mut b);
    // Como num banco de antes da 5.2: gravados sem passar pela troca.
    let gravar_sem_troca = |x: &Banco, gtin: &str| {
        x.conn.execute("UPDATE sync_estado SET aplicando = 1 WHERE id = 1", []).unwrap();
        x.conn
            .execute(
                "INSERT INTO produto_gtin (id, produto_id, gtin, tipo, origem, ordem, criado_em)
                 VALUES (?1, ?2, ?3, 'comercial', 'cEAN', 5, '2026-11-01T00:00:00Z')",
                [rustinho_nucleo::tempo::novo_id(), p.clone(), gtin.to_string()],
            )
            .unwrap();
        x.conn.execute("UPDATE sync_estado SET aplicando = 0 WHERE id = 1", []).unwrap();
    };
    gravar_sem_troca(&a, GTIN_1);
    gravar_sem_troca(&b, GTIN_1);
    gravar_sem_troca(&b, GTIN_2);
    // O que o Rustinho faz ao abrir o banco.
    for x in [&a, &b] {
        sync::criar_gatilhos(&x.conn).unwrap();
        sync::registrar_existentes(&x.conn).unwrap();
    }
    trocar(&mut a, &mut b);
    for x in [&a, &b] {
        assert_eq!(gtins_da_peca(x, &p), vec![GTIN_1.to_string(), GTIN_2.to_string()]);
        assert_eq!(contar(x, "SELECT COUNT(*) FROM produto_gtin"), 2, "o mesmo GTIN duas vezes");
        assert_eq!(contar(x, "SELECT COUNT(*) FROM sync_pendencia"), 0, "a troca travou num GTIN repetido");
        assert_eq!(achou_pelo_codigo(x, GTIN_2), vec![p.clone()]);
    }
    // Abrir de novo não manda outra vez.
    let antes = contar(&a, "SELECT COUNT(*) FROM alteracao");
    sync::registrar_existentes(&a.conn).unwrap();
    assert_eq!(contar(&a, "SELECT COUNT(*) FROM alteracao"), antes);
}
