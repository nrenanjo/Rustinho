//! Testes dos achados da revisão R4 (Sonnet 5.5 xhigh, 24/11/2026;
//! `revisoes/04_REVISAO_2C_SONNET.md`, C2R4-1 a C2R4-3).

use std::sync::atomic::Ordering;

use super::testes_p4_r3::aprovacao_a_mao;
use super::testes_p4_troca::{loja_com_dois_caixas_aprovados, peca_nova};
use super::testes_seguranca_caixas::{motor, temp};
use super::*;
use crate::banco::Banco;
use crate::troca::protocolo4::aprovados::{self, Limites, Linhas, Situacao};
use crate::troca::protocolo4::{Dominio, Identidade};

/// Limites pequenos: 8 aprovações que contam, 3 caixas por aprovador.
pub(super) const PEQUENOS: Limites = Limites { teto: 8, por_aprovador: 3, conferencias: 1000 };

/// Uma loja só com a raiz (registrada), num banco novo: (banco, raiz, loja, gerente).
pub(super) fn loja_com_raiz(nome: &str) -> (Banco, Identidade, String, String) {
    let b = Banco::abrir(&temp(nome)).unwrap();
    let u = crate::faixas::para_testes::configurar(&b, "Caixa 1", 0);
    let loja = crate::loja::criar(&b.conn).unwrap();
    let raiz = Identidade::gerar().unwrap();
    aprovados::registrar_raiz(&b.conn, &raiz, &loja, "Caixa 1", &u).unwrap();
    (b, raiz, loja, u)
}

pub(super) fn retirada_a_mao(quem_assina: &Identidade, chave: &Identidade, loja: &str) -> aprovados::Retirada {
    retirada_da_chave(quem_assina, chave.publica(), loja)
}

pub(super) fn retirada_da_chave(
    quem_assina: &Identidade,
    chave: crate::troca::protocolo4::Chave,
    loja: &str,
) -> aprovados::Retirada {
    let mut r = aprovados::Retirada {
        chave,
        loja_id: loja.into(),
        motivo: "Computador roubado".into(),
        retirador: quem_assina.publica(),
        gerente: "Ana".into(),
        retirado_em: crate::tempo::agora(),
        assinatura: [0u8; 64],
    };
    r.assinatura = quem_assina.assinar(Dominio::Retirada, &r.corpo());
    r
}

fn juntar(
    b: &Banco,
    raiz: &Identidade,
    loja: &str,
    a: Vec<aprovados::Aprovacao>,
    r: Vec<aprovados::Retirada>,
) -> aprovados::Juntadas {
    aprovados::juntar_na_cadeia_com_limites(
        &b.conn,
        &raiz.publica(),
        loja,
        &Linhas { aprovacoes: a, retiradas: r },
        PEQUENOS,
    )
    .unwrap()
}

pub(super) fn situacao(b: &Banco, raiz: &Identidade, loja: &str, chave: &Identidade) -> Situacao {
    aprovados::avaliar(&b.conn, &raiz.publica(), loja).unwrap().situacao(&chave.publica())
}

pub(super) fn aviso_de_registro_cheio(b: &Banco) -> bool {
    crate::avisos::do_dia(&b.conn).unwrap().iter().any(|a| a.id == "p4.registro_cheio")
}

/// O caixa `ladrao` (aprovado pela raiz) aprova 3 caixas falsos, e cada um aprova mais 3:
/// 12 aprovações, todas na cadeia e bem assinadas.
pub(super) fn enchimento(ladrao: &Identidade, loja: &str) -> Vec<aprovados::Aprovacao> {
    let mut linhas = Vec::new();
    for i in 0..3 {
        let falso = Identidade::gerar().unwrap();
        linhas.push(aprovacao_a_mao(ladrao, &falso, loja, &format!("disp-falso-{i}")));
        for j in 0..3 {
            let outro = Identidade::gerar().unwrap();
            linhas.push(aprovacao_a_mao(&falso, &outro, loja, &format!("disp-falso-{i}-{j}")));
        }
    }
    linhas
}

/// C2R4-1, o cenário da PoC do revisor: um membro enche o registro; a retirada dele,
/// assinada pela raiz, entra mesmo assim (antes: recusada, e o ladrão continuava
/// autorizado); a aprovação legítima que não cabia volta a caber; o Início avisa
/// enquanto o registro está cheio.
#[test]
fn c2r4_1_membro_enche_e_a_retirada_entra_mesmo_assim() {
    // C2R5-4: o aviso vem do estado do registro, com os limites deste teste.
    aprovados::para_testes::usar_limites(PEQUENOS);
    let (b, raiz, loja, _u) = loja_com_raiz("c2r4-1-poc");
    let (ladrao, membro, legitimo) =
        (Identidade::gerar().unwrap(), Identidade::gerar().unwrap(), Identidade::gerar().unwrap());
    let j = juntar(
        &b,
        &raiz,
        &loja,
        vec![
            aprovacao_a_mao(&raiz, &ladrao, &loja, "disp-ladrao"),
            aprovacao_a_mao(&raiz, &membro, &loja, "disp-membro"),
        ],
        vec![],
    );
    assert_eq!(j.novas, 2, "{j:?}");
    assert!(!aviso_de_registro_cheio(&b));
    // O ladrão manda 12 aprovações: cabem 8 (as da raiz não contam no teto).
    let do_ladrao = enchimento(&ladrao, &loja);
    let um_falso = do_ladrao[0].chave;
    let j = juntar(&b, &raiz, &loja, do_ladrao, vec![]);
    assert_eq!((j.novas, j.sem_lugar), (8, 4), "{j:?}");
    assert!(aviso_de_registro_cheio(&b), "o Início avisa o gerente");
    // A aprovação legítima de um membro não cabe.
    let legitima = aprovacao_a_mao(&membro, &legitimo, &loja, "disp-legitimo");
    let j = juntar(&b, &raiz, &loja, vec![legitima.clone()], vec![]);
    assert_eq!((j.novas, j.sem_lugar), (0, 1), "{j:?}");
    assert_eq!(situacao(&b, &raiz, &loja, &legitimo), Situacao::Desconhecida);
    // Com o registro cheio, a retirada feita por um membro (não a raiz: não devolve lugar)
    // entra mesmo assim.
    let j = juntar(&b, &raiz, &loja, vec![], vec![retirada_da_chave(&membro, um_falso, &loja)]);
    assert_eq!(j.novas, 1, "a retirada do membro entra com o registro cheio: {j:?}");
    let av = aprovados::avaliar(&b.conn, &raiz.publica(), &loja).unwrap();
    assert_eq!(av.situacao(&um_falso), Situacao::Retirada);
    // A retirada do ladrão, assinada pela raiz, entra com o registro cheio.
    let j = juntar(&b, &raiz, &loja, vec![], vec![retirada_a_mao(&raiz, &ladrao, &loja)]);
    assert_eq!(j.novas, 1, "a retirada entra com o registro cheio: {j:?}");
    assert_eq!(situacao(&b, &raiz, &loja, &ladrao), Situacao::Retirada, "o ladrão deixa de valer");
    // O lugar do ladrão voltou: a aprovação legítima cabe, e o aviso sai.
    let j = juntar(&b, &raiz, &loja, vec![legitima], vec![]);
    assert_eq!((j.novas, j.sem_lugar), (1, 0), "{j:?}");
    assert_eq!(situacao(&b, &raiz, &loja, &legitimo), Situacao::Autorizada);
    assert!(!aviso_de_registro_cheio(&b), "o aviso sai quando volta a caber");
}

/// C2R4-1: o registro inteiro do caixa da raiz numa mensagem só (o enchimento, a
/// retirada e a aprovação legítima), num caixa que ainda não tinha nada: nada é recusado
/// por tamanho, a retirada vale e o enchimento de quem já está fora nem é guardado.
#[test]
fn c2r4_1_mensagem_inteira_com_o_enchimento_e_a_retirada() {
    let (b, raiz, loja, _u) = loja_com_raiz("c2r4-1-inteira");
    let (ladrao, membro, legitimo) =
        (Identidade::gerar().unwrap(), Identidade::gerar().unwrap(), Identidade::gerar().unwrap());
    let mut aprovacoes = vec![
        aprovacao_a_mao(&raiz, &ladrao, &loja, "disp-ladrao"),
        aprovacao_a_mao(&raiz, &membro, &loja, "disp-membro"),
        aprovacao_a_mao(&membro, &legitimo, &loja, "disp-legitimo"),
    ];
    aprovacoes.extend(enchimento(&ladrao, &loja));
    let j = juntar(&b, &raiz, &loja, aprovacoes, vec![retirada_a_mao(&raiz, &ladrao, &loja)]);
    assert_eq!((j.novas, j.recusadas, j.sem_lugar), (4, 12, 0), "{j:?}");
    assert_eq!(situacao(&b, &raiz, &loja, &ladrao), Situacao::Retirada);
    assert_eq!(situacao(&b, &raiz, &loja, &legitimo), Situacao::Autorizada);
    let n: i64 = b.conn.query_row("SELECT COUNT(*) FROM caixa_aprovado", [], |l| l.get(0)).unwrap();
    assert_eq!(n, 4, "a raiz, o ladrão, o membro e o legítimo; o enchimento fica de fora");
}

/// C2R4-1: quem retira o ladrão pode ser um caixa que o próprio ladrão aprovou. A
/// aprovação dele (feita pelo ladrão, que fica fora) entra junto como apoio, mesmo com o
/// registro cheio, para a retirada valer aqui como vale lá.
#[test]
fn c2r4_1_retirada_por_quem_o_ladrao_aprovou_entra_com_o_apoio() {
    let (b, raiz, loja, _u) = loja_com_raiz("c2r4-1-apoio");
    let (ladrao, c3) = (Identidade::gerar().unwrap(), Identidade::gerar().unwrap());
    juntar(&b, &raiz, &loja, vec![aprovacao_a_mao(&raiz, &ladrao, &loja, "disp-ladrao")], vec![]);
    juntar(&b, &raiz, &loja, enchimento(&ladrao, &loja), vec![]);
    let j = juntar(
        &b,
        &raiz,
        &loja,
        vec![aprovacao_a_mao(&ladrao, &c3, &loja, "disp-c3")],
        vec![retirada_a_mao(&c3, &ladrao, &loja)],
    );
    assert_eq!(j.novas, 2, "a retirada e a aprovação de quem retirou (o apoio): {j:?}");
    assert_eq!(situacao(&b, &raiz, &loja, &ladrao), Situacao::Retirada, "a retirada vale aqui");
    assert_eq!(situacao(&b, &raiz, &loja, &c3), Situacao::SemCadeia, "o apoio não aprova ninguém");
}

/// C2R4-1: as aprovações feitas por quem já está fora não valem para nada e não enchem o
/// registro (só entram como apoio de uma retirada).
#[test]
fn c2r4_1_quem_esta_fora_nao_enche_o_registro() {
    // C2R5-4: o aviso vem do estado do registro, com os limites deste teste.
    aprovados::para_testes::usar_limites(PEQUENOS);
    let (b, raiz, loja, _u) = loja_com_raiz("c2r4-1-fora");
    let ladrao = Identidade::gerar().unwrap();
    juntar(
        &b,
        &raiz,
        &loja,
        vec![aprovacao_a_mao(&raiz, &ladrao, &loja, "disp-ladrao")],
        vec![retirada_a_mao(&raiz, &ladrao, &loja)],
    );
    let j = juntar(&b, &raiz, &loja, enchimento(&ladrao, &loja), vec![]);
    assert_eq!((j.novas, j.recusadas, j.sem_lugar), (0, 12, 0), "{j:?}");
    assert!(!aviso_de_registro_cheio(&b), "não é falta de lugar");
}

/// C2R4-1: no máximo `por_aprovador` caixas não retirados por aprovador; retirar um dos
/// que ele aprovou libera um lugar dele.
#[test]
fn c2r4_1_limite_por_aprovador() {
    let (b, raiz, loja, _u) = loja_com_raiz("c2r4-1-aprovador");
    let membro = Identidade::gerar().unwrap();
    juntar(&b, &raiz, &loja, vec![aprovacao_a_mao(&raiz, &membro, &loja, "disp-membro")], vec![]);
    let caixas: Vec<Identidade> = (0..5).map(|_| Identidade::gerar().unwrap()).collect();
    let linhas: Vec<_> =
        caixas.iter().enumerate().map(|(i, c)| aprovacao_a_mao(&membro, c, &loja, &format!("disp-{i}"))).collect();
    let j = juntar(&b, &raiz, &loja, linhas.clone(), vec![]);
    assert_eq!((j.novas, j.sem_lugar), (3, 2), "{j:?}");
    let entraram: Vec<&Identidade> =
        caixas.iter().filter(|c| situacao(&b, &raiz, &loja, c) == Situacao::Autorizada).collect();
    assert_eq!(entraram.len(), 3);
    // O membro retira um dos seus: um lugar volta (e só um).
    let j = juntar(&b, &raiz, &loja, linhas, vec![retirada_a_mao(&membro, entraram[0], &loja)]);
    assert_eq!((j.novas, j.sem_lugar), (2, 1), "a retirada e mais uma aprovação: {j:?}");
}

/// C2R4-1: a aprovação feita neste caixa respeita os mesmos limites (senão ela valeria
/// aqui e não nos outros): o limite por aprovador vale também para a raiz, e o teto
/// barra os membros, não a raiz.
#[test]
fn c2r4_1_aprovacao_local_respeita_os_limites() {
    let (b, raiz, loja, u) = loja_com_raiz("c2r4-1-local");
    let r = raiz.publica();
    let pedido = |i: usize| aprovados::Pedido::com_dispositivo(&Identidade::gerar().unwrap(), &format!("disp-{i}"));
    let aprovar = |quem: &Identidade, i: usize, lim: Limites| {
        aprovados::aprovar_com_limites(&b.conn, quem, &r, &loja, &pedido(i), &format!("Caixa {i}"), &u, lim)
    };
    for i in 0..3 {
        aprovar(&raiz, i, PEQUENOS).unwrap();
    }
    let e = aprovar(&raiz, 3, PEQUENOS).unwrap_err().to_string();
    assert!(e.contains("já aprovou 3 caixas"), "{e}");
    // Um membro aprovado pela raiz, com o teto de 2 (as da raiz não contam).
    let membro = Identidade::gerar().unwrap();
    aprovados::aprovar(&b.conn, &raiz, &r, &loja, &aprovados::Pedido::de(&membro), "Membro", &u).unwrap();
    let teto_2 = Limites { teto: 2, ..PEQUENOS };
    aprovar(&membro, 10, teto_2).unwrap();
    aprovar(&membro, 11, teto_2).unwrap();
    let e = aprovar(&membro, 12, teto_2).unwrap_err().to_string();
    assert!(e.contains("registro de caixas da loja está cheio"), "{e}");
}

/// C2R4-3: a mensagem só com linhas iguais às gravadas não confere assinatura nenhuma (as
/// gravadas já foram conferidas quando entraram); com uma linha nova, confere.
#[test]
fn c2r4_3_linha_igual_nao_e_conferida_de_novo() {
    let (b, raiz, loja, _u) = loja_com_raiz("c2r4-3");
    let (x, y) = (Identidade::gerar().unwrap(), Identidade::gerar().unwrap());
    juntar(
        &b,
        &raiz,
        &loja,
        vec![aprovacao_a_mao(&raiz, &x, &loja, "disp-x"), aprovacao_a_mao(&x, &y, &loja, "disp-y")],
        vec![retirada_a_mao(&raiz, &y, &loja)],
    );
    let iguais = aprovados::linhas(&b.conn, &loja).unwrap();
    aprovados::conferencias_de_teste::tirar();
    let j = aprovados::juntar_na_cadeia(&b.conn, &raiz.publica(), &loja, &iguais).unwrap();
    assert_eq!((j.ja_tinha, j.novas), (4, 0), "{j:?}");
    assert_eq!(aprovados::conferencias_de_teste::tirar(), 0, "nenhuma assinatura conferida de novo");
    let mut com_nova = iguais;
    com_nova.aprovacoes.push(aprovacao_a_mao(&x, &Identidade::gerar().unwrap(), &loja, "disp-z"));
    let j = aprovados::juntar_na_cadeia(&b.conn, &raiz.publica(), &loja, &com_nova).unwrap();
    assert_eq!(j.novas, 1, "{j:?}");
    assert!(aprovados::conferencias_de_teste::tirar() > 0, "a linha nova é conferida");
}

/// C2R4-2: o resumo das tabelas também é comparado pelo fio do protocolo 4: dois caixas
/// aprovados, em dia, e uma linha mexida por fora no caixa 1 (sem passar pela troca): a
/// diferença vai para o gerente nos dois lados.
#[test]
fn c2r4_2_resumo_das_tabelas_pelo_p4() {
    thread::spawn(|| {
        let (ca, cb, a, b, _loja, _ia, _ib) = loja_com_dois_caixas_aprovados("c2r4-2");
        let peca = peca_nova(&a, "Fusível pelo P4");
        for x in [&a, &b] {
            config::gravar(&x.conn, "sync_resumo_teste", "sempre").unwrap();
        }
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        let (end_a, end_b) = (format!("127.0.0.1:{pt_a}"), format!("127.0.0.1:{pt_b}"));
        let antes = p4::ATENDIDAS_PELO_P4.load(Ordering::SeqCst);
        for _ in 0..2 {
            puxar(&cb, &end_a, pt_b).unwrap();
            puxar(&ca, &end_b, pt_a).unwrap();
        }
        let resumos = |x: &Banco| -> i64 {
            x.conn.query_row("SELECT COUNT(*) FROM sync_conflito WHERE tipo = 'resumo'", [], |l| l.get(0)).unwrap()
        };
        assert_eq!((resumos(&a), resumos(&b)), (0, 0), "em dia e iguais: nada para o gerente");
        a.conn.execute("UPDATE sync_estado SET aplicando = 1 WHERE id = 1", []).unwrap();
        a.conn.execute("UPDATE produto SET nome = 'Fusível (mexido à mão)' WHERE id = ?1", [&peca]).unwrap();
        a.conn.execute("UPDATE sync_estado SET aplicando = 0 WHERE id = 1", []).unwrap();
        puxar(&cb, &end_a, pt_b).unwrap();
        let pelo_p4 = p4::ATENDIDAS_PELO_P4.load(Ordering::SeqCst) - antes;
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        assert_eq!((resumos(&a), resumos(&b)), (1, 1), "a diferença aparece nos dois caixas");
        assert!(pelo_p4 >= 5, "todas as puxadas passaram pelo fio do protocolo 4 ({pelo_p4})");
    })
    .join()
    .unwrap();
}
