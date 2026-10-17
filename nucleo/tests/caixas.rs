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
    let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "1234", true).unwrap();
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
    let bia = usuarios::criar(&a.conn, &a.dispositivo, "Bia", "5678", true).unwrap();
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
