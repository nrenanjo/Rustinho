//! Testes dos achados da revisão R5 (Sonnet 5.5 xhigh, 24/11/2026;
//! `revisoes/04_REVISAO_2C_SONNET.md`, C2R5-1 a C2R5-7).

use std::collections::BTreeSet;

use super::testes_p4_r3::aprovacao_a_mao;
use super::testes_p4_r4::{aviso_de_registro_cheio, enchimento, loja_com_raiz, retirada_a_mao, situacao, PEQUENOS};
use super::testes_p4_troca::loja_com_dois_caixas_aprovados;
use super::*;
use crate::banco::Banco;
use crate::troca::protocolo4::aprovados::{self, Limites, Linhas, Situacao};
use crate::troca::protocolo4::{hex, Chave, Dominio, Identidade};

fn juntar_com(
    b: &Banco,
    raiz: &Identidade,
    loja: &str,
    a: Vec<aprovados::Aprovacao>,
    r: Vec<aprovados::Retirada>,
    lim: Limites,
) -> aprovados::Juntadas {
    aprovados::juntar_na_cadeia_com_limites(
        &b.conn,
        &raiz.publica(),
        loja,
        &Linhas { aprovacoes: a, retiradas: r },
        lim,
    )
    .unwrap()
}

fn juntar(
    b: &Banco,
    raiz: &Identidade,
    loja: &str,
    a: Vec<aprovados::Aprovacao>,
    r: Vec<aprovados::Retirada>,
) -> aprovados::Juntadas {
    juntar_com(b, raiz, loja, a, r, PEQUENOS)
}

fn contar(b: &Banco, tabela: &str) -> i64 {
    b.conn.query_row(&format!("SELECT COUNT(*) FROM {tabela}"), [], |l| l.get(0)).unwrap()
}

fn nova() -> Identidade {
    Identidade::gerar().unwrap()
}

/// O ladrão (aprovado pela raiz) e um membro; o ladrão enche o registro (limites
/// pequenos).
fn loja_cheia(nome: &str) -> (Banco, Identidade, String, String, Identidade, Identidade) {
    let (b, raiz, loja, u) = loja_com_raiz(nome);
    let (ladrao, membro) = (nova(), nova());
    juntar(
        &b,
        &raiz,
        &loja,
        vec![
            aprovacao_a_mao(&raiz, &ladrao, &loja, "disp-ladrao"),
            aprovacao_a_mao(&raiz, &membro, &loja, "disp-membro"),
        ],
        vec![],
    );
    juntar(&b, &raiz, &loja, enchimento(&ladrao, &loja), vec![]);
    (b, raiz, loja, u, ladrao, membro)
}

/// C2R5-1, `poc5_a` do revisor: um membro retira o ladrão primeiro (vale, mas o lugar não
/// volta); a retirada da raiz chega depois e entra mesmo assim (antes: recusada, e o
/// registro ficava cheio para sempre), devolvendo o lugar.
#[test]
fn c2r5_1_a_raiz_libera_o_lugar_depois_de_outro_retirar() {
    aprovados::para_testes::usar_limites(PEQUENOS);
    let (b, raiz, loja, _u, ladrao, membro) = loja_cheia("c2r5-1");
    assert!(aviso_de_registro_cheio(&b));
    let j = juntar(&b, &raiz, &loja, vec![], vec![retirada_a_mao(&membro, &ladrao, &loja)]);
    assert_eq!(j.novas, 1, "{j:?}");
    assert_eq!(situacao(&b, &raiz, &loja, &ladrao), Situacao::Retirada);
    assert!(aviso_de_registro_cheio(&b), "o lugar só volta com a retirada da raiz");
    let legitimo = nova();
    let legitima = aprovacao_a_mao(&membro, &legitimo, &loja, "disp-legitimo");
    assert_eq!(juntar(&b, &raiz, &loja, vec![legitima.clone()], vec![]).sem_lugar, 1);
    let j = juntar(&b, &raiz, &loja, vec![], vec![retirada_a_mao(&raiz, &ladrao, &loja)]);
    assert_eq!(j.novas, 1, "a retirada da raiz entra com a chave já retirada: {j:?}");
    assert!(!aviso_de_registro_cheio(&b), "o aviso sai assim que o lugar volta (pelo estado)");
    let j = juntar(&b, &raiz, &loja, vec![legitima], vec![]);
    assert_eq!((j.novas, j.sem_lugar), (1, 0), "{j:?}");
    assert_eq!(situacao(&b, &raiz, &loja, &legitimo), Situacao::Autorizada);
}

/// C2R5-1, `poc5_l`: a retirada do próprio ladrão, com data antiga, e a da raiz na mesma
/// mensagem: as duas entram (a da raiz não disputa a vaga "uma por chave").
#[test]
fn c2r5_1_a_retirada_da_raiz_nao_perde_a_vaga_para_a_do_ladrao() {
    aprovados::para_testes::usar_limites(PEQUENOS);
    let (b, raiz, loja, _u, ladrao, _membro) = loja_cheia("c2r5-1-vaga");
    let mut propria = retirada_a_mao(&ladrao, &ladrao, &loja);
    propria.retirado_em = "2020-01-01T00:00:00Z".into();
    propria.assinatura = ladrao.assinar(Dominio::Retirada, &propria.corpo());
    let j = juntar(&b, &raiz, &loja, vec![], vec![propria, retirada_a_mao(&raiz, &ladrao, &loja)]);
    assert_eq!(j.novas, 2, "{j:?}");
    assert!(!aviso_de_registro_cheio(&b));
}

/// C2R5-1: no próprio caixa, só a raiz retira de novo a chave que outro já retirou (uma
/// vez); o membro continua recebendo "já foi retirado".
#[test]
fn c2r5_1_so_a_raiz_retira_de_novo_e_uma_vez() {
    aprovados::para_testes::usar_limites(PEQUENOS);
    let (b, raiz, loja, u, ladrao, membro) = loja_cheia("c2r5-1-local");
    let r = raiz.publica();
    juntar(&b, &raiz, &loja, vec![], vec![retirada_a_mao(&membro, &ladrao, &loja)]);
    let e = aprovados::retirar(&b.conn, &membro, &r, &loja, &ladrao.publica(), "Roubado", &u).unwrap_err();
    assert!(e.to_string().contains("já foi retirado"), "{e}");
    aprovados::retirar(&b.conn, &raiz, &r, &loja, &ladrao.publica(), "Roubado", &u).unwrap();
    assert!(!aviso_de_registro_cheio(&b), "o lugar voltou");
    let e = aprovados::retirar(&b.conn, &raiz, &r, &loja, &ladrao.publica(), "De novo", &u).unwrap_err();
    assert!(e.to_string().contains("já foi retirado"), "{e}");
}

/// C2R5-1: a lista de caixas oferece "Liberar o lugar" só no primeiro caixa e só com o
/// registro cheio; o caminho da tela (retirar a identidade pela raiz) devolve o lugar.
#[test]
fn c2r5_1_liberar_o_lugar_pela_lista_de_caixas() {
    thread::spawn(|| {
        aprovados::para_testes::usar_limites(PEQUENOS);
        let (ca, _cb, a, b, loja, ia, _ib) = loja_com_dois_caixas_aprovados("c2r5-1-lista");
        let r = ia.identidade.publica();
        let (ladrao, membro) = (nova(), nova());
        for (id, disp) in [(&ladrao, "disp-ladrao"), (&membro, "disp-membro")] {
            let p = aprovados::Pedido::com_dispositivo(id, disp);
            aprovados::aprovar(&a.conn, &ia.identidade, &r, &loja, &p, disp, "Ana").unwrap();
        }
        let linhas = Linhas { aprovacoes: enchimento(&ladrao, &loja), retiradas: vec![] };
        aprovados::juntar_na_cadeia_com_limites(&a.conn, &r, &loja, &linhas, PEQUENOS).unwrap();
        let linhas = Linhas { aprovacoes: vec![], retiradas: vec![retirada_a_mao(&membro, &ladrao, &loja)] };
        aprovados::juntar_na_cadeia_com_limites(&a.conn, &r, &loja, &linhas, PEQUENOS).unwrap();
        let liberar = |x: &Banco| -> Vec<String> {
            caixas_conhecidos(&x.conn)
                .unwrap()
                .into_iter()
                .find(|c| c.dispositivo == "disp-ladrao")
                .map(|c| c.liberar_lugar)
                .unwrap_or_default()
        };
        assert!(registro_de_caixas_cheio(&a.conn).unwrap());
        assert_eq!(liberar(&a), vec![hex(&ladrao.publica())], "no primeiro caixa, com o registro cheio");
        // No caixa 2 (não é a raiz), nunca.
        aprovados::juntar(&b.conn, &loja, &aprovados::linhas(&a.conn, &loja).unwrap()).unwrap();
        assert!(liberar(&b).is_empty(), "fora do primeiro caixa, não aparece");
        retirar_identidade_do_caixa(&a.conn, &ca, "disp-ladrao", &hex(&ladrao.publica()), "Roubado", "Ana").unwrap();
        assert!(!registro_de_caixas_cheio(&a.conn).unwrap(), "o lugar voltou");
        assert!(liberar(&a).is_empty());
    })
    .join()
    .unwrap();
}

/// C2R5-2, `poc5_j` do revisor: com 2.000 aprovações de dispositivos diferentes (uma cadeia
/// de 500 e 1.500 folhas), a lista de caixas e o Início conferem cada assinatura umas
/// poucas vezes (antes: uma vez por caixa autorizado, quase 2.000 × 2.000). Quem cai junto
/// com o primeiro da cadeia aparece com 30 nomes e "e mais N caixas".
#[test]
fn c2r5_2_lista_de_caixas_e_inicio_conferem_o_registro_poucas_vezes() {
    let (b, raiz, loja, _u) = loja_com_raiz("c2r5-2");
    let mut cadeia = vec![nova()];
    let mut linhas = vec![aprovacao_a_mao(&raiz, &cadeia[0], &loja, "disp-c0")];
    for i in 1..500 {
        let n = nova();
        linhas.push(aprovacao_a_mao(&cadeia[i - 1], &n, &loja, &format!("disp-c{i}")));
        cadeia.push(n);
    }
    for i in 0..1500 {
        linhas.push(aprovacao_a_mao(&cadeia[i % 100], &nova(), &loja, &format!("disp-f{i}")));
    }
    aprovados::juntar(&b.conn, &loja, &Linhas { aprovacoes: linhas, retiradas: vec![] }).unwrap();
    let n = contar(&b, "caixa_aprovado") as usize;
    assert_eq!(n, 2001);
    aprovados::conferencias_de_teste::tirar();
    let inicio = std::time::Instant::now();
    let lista = caixas_conhecidos(&b.conn).unwrap();
    let na_lista = aprovados::conferencias_de_teste::tirar();
    assert!(na_lista <= 2 * n, "a lista conferiu {na_lista} assinaturas num registro de {n} linhas");
    let primeiro = lista.iter().find(|c| c.dispositivo == "disp-c0").expect("o primeiro da cadeia");
    assert_eq!(primeiro.dependentes.len(), p4::QUEM_CAI_JUNTO_NA_LISTA + 1, "{:?}", primeiro.dependentes);
    assert_eq!(
        primeiro.dependentes.last().map(String::as_str),
        Some(format!("e mais {} caixas", 1999 - p4::QUEM_CAI_JUNTO_NA_LISTA).as_str())
    );
    crate::avisos::do_dia(&b.conn).unwrap();
    let no_inicio = aprovados::conferencias_de_teste::tirar();
    assert!(no_inicio <= 6 * n, "o Início conferiu {no_inicio} assinaturas num registro de {n} linhas");
    let demorou = inicio.elapsed();
    assert!(demorou < Duration::from_secs(60), "a lista e o Início levaram {demorou:?}");
}

/// C2R5-2: quem cai junto calculado de uma vez (dominadores) é o mesmo do cálculo de um por
/// um, numa loja com caminhos que se cruzam e caixas retirados.
#[test]
fn c2r5_2_quem_cai_junto_igual_ao_calculo_de_cada_um() {
    let (b, raiz, loja, _u) = loja_com_raiz("c2r5-2-igual");
    let mut x: u64 = 0x2C2_7E5;
    let mut sorteio = move |n: usize| -> usize {
        x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        ((x >> 33) as usize) % n
    };
    let mut chaves: Vec<Identidade> = Vec::new();
    let mut linhas = Vec::new();
    for i in 0..40 {
        let n = nova();
        let mut aprovadores = vec![sorteio(i + 1)];
        if i > 2 && sorteio(3) == 0 {
            aprovadores.push(sorteio(i + 1));
        }
        aprovadores.dedup();
        for p in aprovadores {
            let quem = if p == 0 { &raiz } else { &chaves[p - 1] };
            linhas.push(aprovacao_a_mao(quem, &n, &loja, &format!("disp-{i}")));
        }
        chaves.push(n);
    }
    let retiradas = vec![retirada_a_mao(&raiz, &chaves[5], &loja), retirada_a_mao(&raiz, &chaves[17], &loja)];
    aprovados::juntar(&b.conn, &loja, &Linhas { aprovacoes: linhas, retiradas }).unwrap();
    let reg = aprovados::RegistroConferido::carregar(&b.conn, &raiz.publica(), &loja).unwrap();
    let todos = reg.quem_cai_junto(usize::MAX);
    let mut com_dependentes = 0;
    for c in &chaves {
        let um_por_um: BTreeSet<Chave> = reg.dependentes_de(&c.publica()).iter().map(|a| a.chave).collect();
        let (lista, total) = todos.get(&c.publica()).cloned().unwrap_or_default();
        let de_uma_vez: BTreeSet<Chave> = lista.iter().map(|a| a.chave).collect();
        assert_eq!(um_por_um, de_uma_vez);
        assert_eq!(lista.len(), total);
        com_dependentes += usize::from(total > 0);
    }
    assert!(com_dependentes >= 3, "a loja de teste tem caixas com dependentes ({com_dependentes})");
}

/// C2R5-5: as retiradas e as outras versões das linhas gravadas também têm orçamento de
/// conferências, e a linha repetida na mesma mensagem é conferida uma vez.
#[test]
fn c2r5_5_retiradas_e_outras_versoes_tem_orcamento() {
    let (b, raiz, loja, _u) = loja_com_raiz("c2r5-5");
    let (membro, k) = (nova(), nova());
    let gravada = aprovacao_a_mao(&membro, &k, &loja, "disp-k");
    juntar(&b, &raiz, &loja, vec![aprovacao_a_mao(&raiz, &membro, &loja, "disp-membro"), gravada.clone()], vec![]);
    let gravadas = contar(&b, "caixa_aprovado") as usize;
    let lim = Limites { conferencias: 3, ..PEQUENOS };
    // Retiradas de k pelo membro, todas com a assinatura errada e diferentes entre si.
    let ruins: Vec<_> = (0..10u8)
        .map(|i| {
            let mut t = retirada_a_mao(&membro, &k, &loja);
            t.assinatura[0] ^= i + 1;
            t
        })
        .collect();
    aprovados::conferencias_de_teste::tirar();
    let j = juntar_com(&b, &raiz, &loja, vec![], ruins, lim);
    let conferidas = aprovados::conferencias_de_teste::tirar();
    assert_eq!(j.novas, 0);
    assert!(conferidas <= gravadas + 3, "{conferidas} conferências (orçamento 3)");
    // Outras versões da aprovação gravada, todas com a assinatura errada.
    let versoes: Vec<_> = (0..10u8)
        .map(|i| {
            let mut a = gravada.clone();
            a.nome = format!("Versão {i}");
            a.assinatura[0] ^= i + 1;
            a
        })
        .collect();
    let j = juntar_com(&b, &raiz, &loja, versoes, vec![], lim);
    let conferidas = aprovados::conferencias_de_teste::tirar();
    assert_eq!((j.novas, j.trocadas), (0, 0));
    assert!(conferidas <= gravadas + 3, "{conferidas} conferências (orçamento 3)");
    // A mesma retirada ruim repetida: conferida uma vez (antes de gastar o orçamento).
    let mut ruim = retirada_a_mao(&membro, &k, &loja);
    ruim.assinatura[0] ^= 0x55;
    let j = juntar_com(&b, &raiz, &loja, vec![], vec![ruim; 10], lim);
    let conferidas = aprovados::conferencias_de_teste::tirar();
    assert_eq!(j.novas, 0);
    assert!(conferidas <= gravadas + 1, "{conferidas} conferências (a repetida, uma vez)");
    // A mesma retirada repetida: conferida uma vez.
    let boa = retirada_a_mao(&membro, &k, &loja);
    let j = juntar_com(&b, &raiz, &loja, vec![], vec![boa; 10], lim);
    let conferidas = aprovados::conferencias_de_teste::tirar();
    assert_eq!(j.novas, 1);
    assert!(conferidas <= gravadas + 1 + 1, "{conferidas} conferências (a repetida, uma vez)");
}

// ---------------------------------------------------------------------------
// C2R5-6: um teste para cada mutação do revisor que sobreviveu.
// ---------------------------------------------------------------------------

/// Uma loja com a raiz e dois membros aprovados por ela.
fn loja_com_membros(nome: &str) -> (Banco, Identidade, String, Identidade, Identidade) {
    let (b, raiz, loja, _u) = loja_com_raiz(nome);
    let (m1, m2) = (nova(), nova());
    juntar(
        &b,
        &raiz,
        &loja,
        vec![aprovacao_a_mao(&raiz, &m1, &loja, "disp-m1"), aprovacao_a_mao(&raiz, &m2, &loja, "disp-m2")],
        vec![],
    );
    (b, raiz, loja, m1, m2)
}

/// (1) A retirada assinada por quem não está na cadeia não entra.
#[test]
fn c2r5_6_retirada_de_quem_nao_esta_na_cadeia_nao_entra() {
    let (b, raiz, loja, m1, _m2) = loja_com_membros("c2r5-6-1");
    let j = juntar(&b, &raiz, &loja, vec![], vec![retirada_a_mao(&nova(), &m1, &loja)]);
    assert_eq!((j.novas, j.recusadas), (0, 1), "{j:?}");
    assert_eq!(situacao(&b, &raiz, &loja, &m1), Situacao::Autorizada);
    assert_eq!(contar(&b, "caixa_retirado"), 0);
}

/// (2) A retirada de uma chave que o registro não conhece não entra.
#[test]
fn c2r5_6_retirada_de_chave_desconhecida_nao_entra() {
    let (b, raiz, loja, _m1, _m2) = loja_com_membros("c2r5-6-2");
    let j = juntar(&b, &raiz, &loja, vec![], vec![retirada_a_mao(&raiz, &nova(), &loja)]);
    assert_eq!((j.novas, j.recusadas), (0, 1), "{j:?}");
    assert_eq!(contar(&b, "caixa_retirado"), 0);
}

/// (3) A retirada de uma chave que já está fora (sem cadeia) só entra pela raiz.
#[test]
fn c2r5_6_retirada_de_chave_sem_cadeia_so_entra_pela_raiz() {
    let (b, raiz, loja, m1, m2) = loja_com_membros("c2r5-6-3");
    let k = nova();
    juntar(&b, &raiz, &loja, vec![aprovacao_a_mao(&m1, &k, &loja, "disp-k")], vec![]);
    juntar(&b, &raiz, &loja, vec![], vec![retirada_a_mao(&raiz, &m1, &loja)]);
    assert_eq!(situacao(&b, &raiz, &loja, &k), Situacao::SemCadeia);
    let j = juntar(&b, &raiz, &loja, vec![], vec![retirada_a_mao(&m2, &k, &loja)]);
    assert_eq!((j.novas, j.recusadas), (0, 1), "a do membro fica para depois: {j:?}");
    let j = juntar(&b, &raiz, &loja, vec![], vec![retirada_a_mao(&raiz, &k, &loja)]);
    assert_eq!(j.novas, 1, "a da raiz entra: {j:?}");
}

/// (4) Uma retirada por chave e por mensagem (fora a da raiz).
#[test]
fn c2r5_6_uma_retirada_por_chave_por_mensagem() {
    let (b, raiz, loja, m1, m2) = loja_com_membros("c2r5-6-4");
    let k = nova();
    juntar(&b, &raiz, &loja, vec![aprovacao_a_mao(&raiz, &k, &loja, "disp-k")], vec![]);
    let j = juntar(&b, &raiz, &loja, vec![], vec![retirada_a_mao(&m1, &k, &loja), retirada_a_mao(&m2, &k, &loja)]);
    assert_eq!((j.novas, j.recusadas), (1, 1), "{j:?}");
}

/// (5) O orçamento de conferências das aprovações vale.
#[test]
fn c2r5_6_orcamento_das_aprovacoes() {
    let (b, raiz, loja, m1, _m2) = loja_com_membros("c2r5-6-5");
    let gravadas = contar(&b, "caixa_aprovado") as usize;
    let linhas: Vec<_> = (0..20).map(|i| aprovacao_a_mao(&m1, &nova(), &loja, &format!("disp-{i}"))).collect();
    let lim = Limites { teto: 100, por_aprovador: 100, conferencias: 5 };
    aprovados::conferencias_de_teste::tirar();
    let j = juntar_com(&b, &raiz, &loja, linhas, vec![], lim);
    let conferidas = aprovados::conferencias_de_teste::tirar();
    assert_eq!(j.novas, 5, "{j:?}");
    assert!(conferidas <= gravadas + 5 + 5, "{conferidas} conferências (orçamento 5)");
}

/// (6) O apoio de uma retirada vai até APOIO_MAXIMO linhas (4): com 4 entra, com 5 não.
#[test]
fn c2r5_6_apoio_tem_profundidade_maxima() {
    for (comprimento, entra) in [(aprovados::APOIO_MAXIMO, true), (aprovados::APOIO_MAXIMO + 1, false)] {
        let (b, raiz, loja, _u) = loja_com_raiz(&format!("c2r5-6-6-{comprimento}"));
        let ladrao = nova();
        juntar(&b, &raiz, &loja, vec![aprovacao_a_mao(&raiz, &ladrao, &loja, "disp-ladrao")], vec![]);
        let mut caminho = vec![ladrao];
        let mut linhas = Vec::new();
        for i in 0..comprimento {
            let n = nova();
            linhas.push(aprovacao_a_mao(&caminho[i], &n, &loja, &format!("disp-a{i}")));
            caminho.push(n);
        }
        let quem_retira = caminho.last().unwrap();
        let j = juntar(&b, &raiz, &loja, linhas, vec![retirada_a_mao(quem_retira, &caminho[0], &loja)]);
        let retirado = situacao(&b, &raiz, &loja, &caminho[0]) == Situacao::Retirada;
        assert_eq!(retirado, entra, "apoio de {comprimento} linhas: {j:?}");
    }
}

/// (7) Com pouco lugar, entram primeiro as aprovações dos mais perto da raiz.
#[test]
fn c2r5_6_primeiro_os_mais_perto_da_raiz() {
    let (b, raiz, loja, _u) = loja_com_raiz("c2r5-6-7");
    let (a, x, f) = (nova(), nova(), nova());
    let gravadas = vec![
        aprovacao_a_mao(&raiz, &a, &loja, "disp-a"),
        aprovacao_a_mao(&raiz, &x, &loja, "disp-x"),
        aprovacao_a_mao(&x, &f, &loja, "disp-f"),
    ];
    aprovados::juntar(&b.conn, &loja, &Linhas { aprovacoes: gravadas, retiradas: vec![] }).unwrap();
    let (longe, perto) = (nova(), nova());
    let lim = Limites { teto: 2, por_aprovador: 10, conferencias: 100 };
    let j = juntar_com(
        &b,
        &raiz,
        &loja,
        vec![aprovacao_a_mao(&f, &longe, &loja, "disp-longe"), aprovacao_a_mao(&a, &perto, &loja, "disp-perto")],
        vec![],
        lim,
    );
    assert_eq!((j.novas, j.sem_lugar), (1, 1), "{j:?}");
    assert_eq!(situacao(&b, &raiz, &loja, &perto), Situacao::Autorizada, "a de quem está mais perto da raiz");
    assert_eq!(situacao(&b, &raiz, &loja, &longe), Situacao::Desconhecida);
}

/// (8) A outra versão de uma linha gravada (a mesma chave primária) passa pelo desempate:
/// a mais nova fica.
#[test]
fn c2r5_6_outra_versao_da_linha_gravada_passa_pelo_desempate() {
    let (b, raiz, loja, _u) = loja_com_raiz("c2r5-6-8");
    let k = nova();
    let velha = aprovacao_a_mao(&raiz, &k, &loja, "disp-k");
    juntar(&b, &raiz, &loja, vec![velha.clone()], vec![]);
    let mut nova_versao = velha;
    nova_versao.nome = "Caixa de nome novo".into();
    nova_versao.aprovado_em = "2999-01-01T00:00:00Z".into();
    nova_versao.assinatura = raiz.assinar(Dominio::Aprovacao, &nova_versao.corpo());
    let j = juntar(&b, &raiz, &loja, vec![nova_versao], vec![]);
    assert_eq!(j.trocadas, 1, "{j:?}");
    let nome: String = b
        .conn
        .query_row("SELECT nome FROM caixa_aprovado WHERE chave = ?1", [hex(&k.publica())], |l| l.get(0))
        .unwrap();
    assert_eq!(nome, "Caixa de nome novo");
}

/// (9) Aprovar a si mesmo só vale para a raiz: a linha de um membro aprovando a si mesmo
/// não entra.
#[test]
fn c2r5_6_aprovacao_de_si_mesmo_so_vale_para_a_raiz() {
    let (b, raiz, loja, m1, _m2) = loja_com_membros("c2r5-6-9");
    let antes = contar(&b, "caixa_aprovado");
    let j = juntar(&b, &raiz, &loja, vec![aprovacao_a_mao(&m1, &m1, &loja, "disp-m1")], vec![]);
    assert_eq!((j.novas, j.recusadas), (0, 1), "{j:?}");
    assert_eq!(contar(&b, "caixa_aprovado"), antes);
}
