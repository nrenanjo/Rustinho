//! Testes dos achados da revisão R6 (Sonnet 5.5 xhigh, 24/11/2026;
//! `revisoes/04_REVISAO_2C_SONNET.md`, C2R6-1 e C2R6-2).

use super::testes_p4_r3::aprovacao_a_mao;
use super::testes_p4_r4::{enchimento, loja_com_raiz, retirada_a_mao, PEQUENOS};
use super::testes_p4_troca::loja_com_dois_caixas_aprovados;
use super::*;
use crate::banco::Banco;
use crate::troca::protocolo4::aprovados::{self, Linhas};
use crate::troca::protocolo4::{hex, Identidade};

fn nova() -> Identidade {
    Identidade::gerar().unwrap()
}

/// Uma loja em que o caixa `m` (aprovado pela raiz) aprovou `n` caixas, com os
/// dispositivos `d000`, `d001`… (os nomes vêm deles: "Caixa d000"…).
fn loja_com_um_caixa_e_dependentes(nome: &str, n: usize) -> (Banco, Identidade, String, Identidade) {
    let (b, raiz, loja, _u) = loja_com_raiz(nome);
    let m = nova();
    let mut linhas = vec![aprovacao_a_mao(&raiz, &m, &loja, "disp-m")];
    // Fora de ordem de propósito.
    for i in (0..n).rev() {
        linhas.push(aprovacao_a_mao(&m, &nova(), &loja, &format!("d{i:03}")));
    }
    aprovados::juntar(&b.conn, &loja, &Linhas { aprovacoes: linhas, retiradas: vec![] }).unwrap();
    (b, raiz, loja, m)
}

/// C2R6-1, `poc6_k` do revisor: com mais de 30 dependentes, a lista é a mesma a cada
/// consulta (antes, o conjunto escolhido dependia da ordem de um `HashMap` e mudava a
/// cada consulta): os mais perto da raiz e, no mesmo nível, por nome.
#[test]
fn c2r6_1_quem_cai_junto_e_a_mesma_lista_a_cada_consulta() {
    let (b, raiz, loja, m) = loja_com_um_caixa_e_dependentes("c2r6-1", 100);
    let esperados: Vec<String> = (0..30).map(|i| format!("Caixa d{i:03}")).collect();
    for _ in 0..6 {
        let reg = aprovados::RegistroConferido::carregar(&b.conn, &raiz.publica(), &loja).unwrap();
        let (lista, total) = reg.quem_cai_junto(30).remove(&m.publica()).unwrap();
        let nomes: Vec<String> = lista.into_iter().map(|a| a.nome).collect();
        assert_eq!(nomes, esperados);
        assert_eq!(total, 100);
    }
}

/// C2R6-1: "e mais 1 caixa", no singular (antes, "e mais 1 caixas").
#[test]
fn c2r6_1_um_caixa_a_mais_no_singular() {
    let (b, _raiz, _loja, _m) = loja_com_um_caixa_e_dependentes("c2r6-1-singular", p4::QUEM_CAI_JUNTO_NA_LISTA + 1);
    let lista = caixas_conhecidos(&b.conn).unwrap();
    let m = lista.iter().find(|c| c.dispositivo == "disp-m").expect("o caixa m");
    assert_eq!(m.dependentes.len(), p4::QUEM_CAI_JUNTO_NA_LISTA + 1);
    assert_eq!(m.dependentes.last().map(String::as_str), Some("e mais 1 caixa"));
}

/// O que a lista de caixas mostra no botão "Liberar o lugar" para o dispositivo `disp`.
fn liberar(a: &Banco, disp: &str) -> Vec<String> {
    caixas_conhecidos(&a.conn)
        .unwrap()
        .into_iter()
        .find(|c| c.dispositivo == disp)
        .map(|c| c.liberar_lugar)
        .unwrap_or_default()
}

/// C2R6-2 (mutação n9 do revisor): com o registro **sem** estar cheio, o caixa já
/// retirado por um membro não mostra "Liberar o lugar" (não há lugar a liberar).
#[test]
fn c2r6_2_sem_registro_cheio_nao_ha_liberar_o_lugar() {
    thread::spawn(|| {
        aprovados::para_testes::usar_limites(PEQUENOS);
        let (_ca, _cb, a, _b, loja, ia, _ib) = loja_com_dois_caixas_aprovados("c2r6-2-livre");
        let r = ia.identidade.publica();
        let (ladrao, membro) = (nova(), nova());
        for (id, disp) in [(&ladrao, "disp-ladrao"), (&membro, "disp-membro")] {
            let p = aprovados::Pedido::com_dispositivo(id, disp);
            aprovados::aprovar(&a.conn, &ia.identidade, &r, &loja, &p, disp, "Ana").unwrap();
        }
        let linhas = Linhas { aprovacoes: vec![], retiradas: vec![retirada_a_mao(&membro, &ladrao, &loja)] };
        aprovados::juntar_na_cadeia_com_limites(&a.conn, &r, &loja, &linhas, PEQUENOS).unwrap();
        assert!(!registro_de_caixas_cheio(&a.conn).unwrap());
        assert!(liberar(&a, "disp-ladrao").is_empty(), "sem registro cheio, sem botão");
    })
    .join()
    .unwrap();
}

/// C2R6-2 (mutação n10 do revisor): o primeiro ladrão já foi liberado pela raiz; um
/// segundo ladrão enche o registro de novo. O primeiro não volta a mostrar "Liberar o
/// lugar" (a raiz já o retirou; o clique daria "já foi retirado").
#[test]
fn c2r6_2_quem_a_raiz_ja_liberou_nao_volta_a_ter_o_botao() {
    thread::spawn(|| {
        aprovados::para_testes::usar_limites(PEQUENOS);
        let (ca, _cb, a, _b, loja, ia, _ib) = loja_com_dois_caixas_aprovados("c2r6-2-dois");
        let r = ia.identidade.publica();
        let (ladrao1, ladrao2, membro) = (nova(), nova(), nova());
        for (id, disp) in [(&ladrao1, "disp-ladrao1"), (&ladrao2, "disp-ladrao2"), (&membro, "disp-membro")] {
            let p = aprovados::Pedido::com_dispositivo(id, disp);
            aprovados::aprovar(&a.conn, &ia.identidade, &r, &loja, &p, disp, "Ana").unwrap();
        }
        let juntar = |l: Linhas| {
            aprovados::juntar_na_cadeia_com_limites(&a.conn, &r, &loja, &l, PEQUENOS).unwrap();
        };
        juntar(Linhas { aprovacoes: enchimento(&ladrao1, &loja), retiradas: vec![] });
        juntar(Linhas { aprovacoes: vec![], retiradas: vec![retirada_a_mao(&membro, &ladrao1, &loja)] });
        assert_eq!(liberar(&a, "disp-ladrao1"), vec![hex(&ladrao1.publica())]);
        retirar_identidade_do_caixa(&a.conn, &ca, "disp-ladrao1", &hex(&ladrao1.publica()), "Roubado", "Ana").unwrap();
        assert!(!registro_de_caixas_cheio(&a.conn).unwrap(), "o lugar voltou");
        // O segundo ladrão enche de novo.
        juntar(Linhas { aprovacoes: enchimento(&ladrao2, &loja), retiradas: vec![] });
        assert!(registro_de_caixas_cheio(&a.conn).unwrap(), "cheio outra vez");
        assert!(liberar(&a, "disp-ladrao1").is_empty(), "a raiz já liberou o primeiro: sem botão");
        assert!(liberar(&a, "disp-ladrao2").is_empty(), "o segundo ainda não foi retirado: sem botão");
    })
    .join()
    .unwrap();
}
