//! Código nos fornecedores entre dois caixas: as decisões F-2 e F-3 do proprietário (16/11/2026), tiradas da
//! revisão do Fable (B-2 e B-3). O ambiente de dois caixas segue o das demonstrações do revisor
//! (`prod13/rev_fable_fornec/saidas/testes_rev_fornec.rs`).
//!
//! - F-2: com duas ligações gravadas no mesmo segundo, a ordem da ficha é fixa e igual em todos os caixas (não
//!   depende da ordem das linhas no banco, nem muda quando um código é corrigido).
//! - F-3: quando o gerente resolve um conflito e "usa a versão que perdeu", a ligação volta com a data dela, e não
//!   com a de agora. Nas outras tabelas, a regra geral (a data de agora) continua.

use crate::banco::Banco;
use crate::catalogo::codigos::{vincular_fornecedor, JaLigado, LinhaCodigoFornecedor};
use crate::{config, pessoas, produtos, sync, usuarios};

const DATA_VELHA: &str = "2026-01-02T03:04:05Z";

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

struct Dois {
    a: Banco,
    b: Banco,
    u: String,
    p: String,
    f: String,
}

/// Dois caixas iguais: a peça P com o código "00008373" da Patral (caixa com 6), com a data velha.
fn dois() -> Dois {
    let mut a = Banco::em_memoria().unwrap();
    let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "1234", true).unwrap().id;
    config::gravar(&a.conn, "caixa_nome", "Caixa 1").unwrap();
    let mut fp = produtos::ficha_nova(&a.conn).unwrap();
    fp.nome = "Conector macho 12x08".into();
    fp.custo = "10".into();
    fp.codigo_original = "FAB-123".into();
    let p = produtos::salvar(&a.conn, &a.dispositivo, &u, &fp).unwrap();
    let mut ff = pessoas::ficha_nova(&a.conn, "fornecedor").unwrap();
    ff.nome = "Patral Peças".into();
    let f = pessoas::salvar(&a.conn, &a.dispositivo, &u, &ff).unwrap();
    assert!(vincular_fornecedor(&a.conn, &f, "00008373", &p, 6000, JaLigado::Manter).unwrap());
    a.conn.execute("UPDATE produto_fornecedor SET atualizado_em = ?1", [DATA_VELHA]).unwrap();
    let mut b = Banco::em_memoria().unwrap();
    config::gravar(&b.conn, "caixa_nome", "Caixa 2").unwrap();
    trocar(&mut a, &mut b);
    Dois { a, b, u, p, f }
}

/// Como a tela: a ficha aberta agora, com as linhas como estão, trocando (fornecedor, antes) → novo.
fn editar(b: &Banco, u: &str, p: &str, trocas: &[(&str, &str, &str)]) {
    let mut f = produtos::ficha(&b.conn, p).unwrap();
    f.codigos_fornecedor = f
        .fornecedores
        .iter()
        .map(|x| {
            let novo = trocas
                .iter()
                .find(|(forn, antes, _)| x.fornecedor_id == *forn && x.codigo == *antes)
                .map(|t| t.2.to_string())
                .unwrap_or_else(|| x.codigo.clone());
            LinhaCodigoFornecedor { fornecedor_id: x.fornecedor_id.clone(), antes: x.codigo.clone(), codigo: novo }
        })
        .collect();
    produtos::salvar(&b.conn, &b.dispositivo, u, &f).unwrap();
}

/// Os códigos da ficha, na ordem em que a ficha os mostra.
fn ordem(b: &Banco, p: &str) -> Vec<String> {
    produtos::ficha(&b.conn, p).unwrap().fornecedores.into_iter().map(|x| x.codigo).collect()
}

/// (código, data) das ligações do fornecedor, pela ordem do código compacto.
fn datas(b: &Banco, f: &str) -> Vec<(String, String)> {
    let mut c = b
        .conn
        .prepare(
            "SELECT codigo, atualizado_em FROM produto_fornecedor WHERE fornecedor_id = ?1 ORDER BY codigo_compacto",
        )
        .unwrap();
    c.query_map([f], |l| Ok((l.get(0)?, l.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap()
}

fn abertos(b: &Banco) -> Vec<sync::Conflito> {
    sync::conflitos(&b.conn).unwrap().into_iter().filter(|k| k.resolvido.is_empty()).collect()
}

/// F-2: o caixa 1 liga o "ZZ-2" e o caixa 2 o "AA-1", à peça P, no mesmo segundo do "00008373". Depois da troca,
/// cada caixa tem as linhas numa ordem diferente no banco; a ficha mostra a mesma ordem nos dois, e corrigir um
/// código (outra forma: a ligação é regravada no fim da tabela) não muda a ordem.
#[test]
fn empate_de_data_ordem_fixa_na_ficha_em_todos_os_caixas() {
    let mut d = dois();
    assert!(vincular_fornecedor(&d.a.conn, &d.f, "ZZ-2", &d.p, 1000, JaLigado::Manter).unwrap());
    assert!(vincular_fornecedor(&d.b.conn, &d.f, "AA-1", &d.p, 1000, JaLigado::Manter).unwrap());
    for c in [&d.a, &d.b] {
        c.conn.execute("UPDATE produto_fornecedor SET atualizado_em = ?1", [DATA_VELHA]).unwrap();
    }
    trocar(&mut d.a, &mut d.b);
    let rowids = |b: &Banco| -> Vec<String> {
        let mut c = b.conn.prepare("SELECT codigo FROM produto_fornecedor ORDER BY rowid").unwrap();
        c.query_map([], |l| l.get(0)).unwrap().collect::<Result<_, _>>().unwrap()
    };
    assert_ne!(rowids(&d.a), rowids(&d.b), "as linhas estão em ordens diferentes nos dois caixas (o cenário vale)");
    let esperada = ordem(&d.a, &d.p);
    assert_eq!(ordem(&d.b, &d.p), esperada, "a mesma ordem nos dois caixas");
    // A ordem fixa é a da chave da ligação (o mesmo fornecedor: o código compacto).
    assert_eq!(esperada, vec!["00008373".to_string(), "AA-1".to_string(), "ZZ-2".to_string()]);
    // Corrigir o 00008373 → 00008374 no caixa 1 (outra forma): a ordem não muda, nos dois caixas.
    editar(&d.a, &d.u, &d.p, &[(&d.f, "00008373", "00008374")]);
    trocar(&mut d.a, &mut d.b);
    let depois = vec!["00008374".to_string(), "AA-1".to_string(), "ZZ-2".to_string()];
    assert_eq!(ordem(&d.a, &d.p), depois);
    assert_eq!(ordem(&d.b, &d.p), depois);
}

/// F-3: o caixa 2 corrige só a forma do código (a data fica); o caixa 1, sem ver, troca o código por outro (apaga a
/// ligação velha). Na troca, o gerente do caixa 1 usa a versão que perdeu: a ligação corrigida pelo caixa 2 volta
/// com a data DELA (a de antes), e não com a de agora, nos dois caixas.
#[test]
fn usar_a_versao_que_perdeu_mantem_a_data_da_ligacao() {
    let mut d = dois();
    editar(&d.b, &d.u, &d.p, &[(&d.f, "00008373", "0000-8373")]);
    assert_eq!(datas(&d.b, &d.f), vec![("0000-8373".to_string(), DATA_VELHA.to_string())], "só a forma: a data fica");
    std::thread::sleep(std::time::Duration::from_millis(5));
    editar(&d.a, &d.u, &d.p, &[(&d.f, "00008373", "00008374")]);
    trocar(&mut d.a, &mut d.b);
    let k = abertos(&d.a);
    assert_eq!(k.len(), 1, "um conflito: {:?}", k.iter().map(|x| &x.texto).collect::<Vec<_>>());
    sync::resolver_conflito(&d.a.conn, k[0].id, true, &d.u).unwrap();
    trocar(&mut d.a, &mut d.b);
    assert_eq!(datas(&d.a, &d.f), datas(&d.b, &d.f), "os dois caixas iguais");
    let voltou: Vec<_> = datas(&d.a, &d.f).into_iter().filter(|x| x.0 == "0000-8373").collect();
    assert_eq!(voltou.len(), 1, "a ligação corrigida pelo caixa 2 voltou: {:?}", datas(&d.a, &d.f));
    assert_eq!(voltou[0].1, DATA_VELHA, "com a data dela, e não a de agora (F-3)");
}

/// A exceção da F-3 é só da ligação do código do fornecedor: num conflito de outra tabela (o nome da peça, mudado
/// nos dois caixas), "usar a versão que perdeu" continua gravando a data de agora (a regra geral).
#[test]
fn nas_outras_tabelas_a_versao_que_volta_leva_a_data_de_agora() {
    let mut d = dois();
    for (c, nome) in [(&d.a, "Conector A"), (&d.b, "Conector B")] {
        let mut f = produtos::ficha(&c.conn, &d.p).unwrap();
        f.nome = nome.into();
        produtos::salvar(&c.conn, &c.dispositivo, &d.u, &f).unwrap();
    }
    d.a.conn.execute("UPDATE produto SET atualizado_em = ?1 WHERE id = ?2", [DATA_VELHA, d.p.as_str()]).unwrap();
    d.b.conn.execute("UPDATE produto SET atualizado_em = ?1 WHERE id = ?2", [DATA_VELHA, d.p.as_str()]).unwrap();
    trocar(&mut d.a, &mut d.b);
    let k = abertos(&d.a);
    assert!(!k.is_empty(), "há conflito do nome da peça");
    sync::resolver_conflito(&d.a.conn, k[0].id, true, &d.u).unwrap();
    let data: String =
        d.a.conn.query_row("SELECT atualizado_em FROM produto WHERE id = ?1", [&d.p], |l| l.get(0)).unwrap();
    assert_ne!(data, DATA_VELHA, "a peça que voltou leva a data de agora (a regra geral)");
}

/// F-2, N-6 e N-7 da conferência do 5º congelamento: o mesmo código "777" em DOIS fornecedores, gravado no mesmo
/// segundo, numa ordem em cada caixa (o caixa 1 liga o da Patral; o caixa 2, o da Global). Depois da troca, a ficha
/// (a ordem dos fornecedores) e a busca pelo código (o fornecedor que o balão mostra) dão o mesmo resultado nos dois
/// caixas.
#[test]
fn empate_entre_fornecedores_mesmo_resultado_em_todos_os_caixas() {
    let mut d = dois();
    let mut fg = pessoas::ficha_nova(&d.a.conn, "fornecedor").unwrap();
    fg.nome = "Global Autopeças".into();
    let g = pessoas::salvar(&d.a.conn, &d.a.dispositivo, &d.u, &fg).unwrap();
    trocar(&mut d.a, &mut d.b);
    assert!(vincular_fornecedor(&d.a.conn, &d.f, "777", &d.p, 1000, JaLigado::Manter).unwrap());
    assert!(vincular_fornecedor(&d.b.conn, &g, "777", &d.p, 1000, JaLigado::Manter).unwrap());
    for c in [&d.a, &d.b] {
        c.conn.execute("UPDATE produto_fornecedor SET atualizado_em = ?1", [DATA_VELHA]).unwrap();
    }
    trocar(&mut d.a, &mut d.b);
    let rowids = |b: &Banco| -> Vec<String> {
        let mut c =
            b.conn.prepare("SELECT fornecedor_id || ':' || codigo FROM produto_fornecedor ORDER BY rowid").unwrap();
        c.query_map([], |l| l.get(0)).unwrap().collect::<Result<_, _>>().unwrap()
    };
    assert_ne!(rowids(&d.a), rowids(&d.b), "as linhas estão em ordens diferentes nos dois caixas (o cenário vale)");
    // A ficha: a mesma ordem, a da chave da ligação (o fornecedor, depois o código compacto).
    let ficha = |b: &Banco| -> Vec<(String, String)> {
        produtos::ficha(&b.conn, &d.p).unwrap().fornecedores.into_iter().map(|x| (x.fornecedor_id, x.codigo)).collect()
    };
    assert_eq!(ficha(&d.a), ficha(&d.b), "a ficha na mesma ordem nos dois caixas");
    let mut esperada = ficha(&d.a);
    esperada.sort_by(|x, y| {
        (x.0.as_str(), crate::texto::compactar(&x.1)).cmp(&(y.0.as_str(), crate::texto::compactar(&y.1)))
    });
    assert_eq!(ficha(&d.a), esperada, "a ordem da chave da ligação");
    // A busca pelo código: o balão mostra o mesmo fornecedor nos dois caixas.
    let balao = |b: &Banco| -> Option<String> {
        crate::catalogo::busca::buscar(&b.conn, "777", false, 10)
            .unwrap()
            .into_iter()
            .find(|x| x.id == d.p)
            .map(|x| x.fornecedor_nome)
    };
    assert!(balao(&d.a).is_some(), "a busca acha a peça pelo código");
    assert_eq!(balao(&d.a), balao(&d.b), "o mesmo fornecedor no balão nos dois caixas");
}

/// B2 da revisão R7: os dois caixas corrigem só a forma do mesmo código (a mesma peça, o mesmo fator), cada um de um
/// jeito. É o conflito comum da ligação: continua aberto para o gerente decidir (não é "Corrigido", que é só quando
/// juntaram as peças).
#[test]
fn r7_b2_so_o_texto_do_codigo_e_conflito_comum() {
    let mut d = dois();
    editar(&d.a, &d.u, &d.p, &[(&d.f, "00008373", "0000-8373")]);
    std::thread::sleep(std::time::Duration::from_millis(5));
    editar(&d.b, &d.u, &d.p, &[(&d.f, "00008373", "000.083.73")]);
    trocar(&mut d.a, &mut d.b);
    for x in [&d.a, &d.b] {
        let k = abertos(x);
        assert_eq!(
            k.len(),
            1,
            "o conflito comum continua aberto: {:?}",
            k.iter().map(|c| &c.texto).collect::<Vec<_>>()
        );
        assert_ne!(k[0].o_que, "Código do fornecedor em disputa");
        assert_eq!(sync::conflitos_abertos(&x.conn).unwrap(), 1);
    }
}
