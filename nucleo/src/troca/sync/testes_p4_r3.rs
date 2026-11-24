//! Testes dos achados da revisão R3 (Sonnet 5.5 xhigh, 24/11/2026;
//! `revisoes/04_REVISAO_2C_SONNET.md`, C2R3-1 a C2R3-8).

use super::testes_p4_troca::{caixa_novo_sem_senha, codigo_do_caixa_novo, loja_com_dois_caixas_aprovados};
use super::testes_seguranca_caixas::temp;
use super::*;
use crate::banco::Banco;
use crate::troca::protocolo4::{aprovados, cofre, hex, Dominio, Identidade};

fn aprovacao_a_mao(
    quem_assina: &Identidade,
    chave: &Identidade,
    loja: &str,
    dispositivo: &str,
) -> aprovados::Aprovacao {
    let mut a = aprovados::Aprovacao {
        chave: chave.publica(),
        chave_troca: chave.publica_troca(),
        loja_id: loja.into(),
        nome: format!("Caixa {dispositivo}"),
        aprovador: quem_assina.publica(),
        gerente: "Ana".into(),
        aprovado_em: crate::tempo::agora(),
        dispositivo: dispositivo.into(),
        assinatura: [0u8; 64],
    };
    a.assinatura = quem_assina.assinar(Dominio::Aprovacao, &a.corpo());
    a
}

/// Uma loja só com a raiz (registrada), num banco novo.
fn loja_so_com_raiz(nome: &str) -> (Banco, Identidade, String) {
    let c = temp(nome);
    let b = Banco::abrir(&c).unwrap();
    let u = crate::faixas::para_testes::configurar(&b, "Caixa 1", 0);
    let loja = crate::loja::criar(&b.conn).unwrap();
    let raiz = Identidade::gerar().unwrap();
    aprovados::registrar_raiz(&b.conn, &raiz, &loja, "Caixa 1", &u).unwrap();
    (b, raiz, loja)
}

/// C2R3-2: uma cadeia comprida mandada de trás para a frente (o pior caso do ponto fixo
/// de antes, que crescia com o quadrado da profundidade) entra inteira numa passada.
#[test]
fn c2r3_2_cadeia_comprida_de_tras_para_frente_entra_numa_passada() {
    let (b, raiz, loja) = loja_so_com_raiz("c2r3-2");
    let mut ids = vec![raiz];
    let mut linhas = Vec::new();
    for i in 0..320 {
        let nova = Identidade::gerar().unwrap();
        linhas.push(aprovacao_a_mao(ids.last().unwrap(), &nova, &loja, &format!("disp-{i}")));
        ids.push(nova);
    }
    linhas.reverse();
    let inicio = std::time::Instant::now();
    let j = aprovados::juntar_na_cadeia(
        &b.conn,
        &ids[0].publica(),
        &loja,
        &aprovados::Linhas { aprovacoes: linhas, retiradas: vec![] },
    )
    .unwrap();
    let demorou = inicio.elapsed();
    assert_eq!((j.novas, j.recusadas), (320, 0), "{j:?}");
    assert!(aprovados::autorizada(&b.conn, &ids[0].publica(), &loja, &ids[320].publica()).unwrap());
    assert!(
        demorou < Duration::from_secs(8),
        "a junção levou {demorou:?} (antes: cresce com o quadrado da profundidade)"
    );
}

/// C2R3-5: o teto de linhas do registro vale (aqui, com um teto pequeno), e uma mensagem
/// com mais linhas que o teto é recusada inteira.
#[test]
fn c2r3_5_teto_do_registro() {
    let (b, raiz, loja) = loja_so_com_raiz("c2r3-5-teto");
    let r = raiz.publica();
    let linhas: Vec<_> = (0..8)
        .map(|i| aprovacao_a_mao(&raiz, &Identidade::gerar().unwrap(), &loja, &format!("disp-teto-{i}")))
        .collect();
    let j = aprovados::juntar_na_cadeia_com_teto(
        &b.conn,
        &r,
        &loja,
        &aprovados::Linhas { aprovacoes: linhas.clone(), retiradas: vec![] },
        6,
    )
    .unwrap();
    assert_eq!(j.recusadas, 8, "mensagem maior que o teto: recusada inteira ({j:?})");
    // 5 linhas cabem na mensagem (teto 5), mas o registro já tem a raiz: entram 4.
    let j = aprovados::juntar_na_cadeia_com_teto(
        &b.conn,
        &r,
        &loja,
        &aprovados::Linhas { aprovacoes: linhas[..5].to_vec(), retiradas: vec![] },
        5,
    )
    .unwrap();
    let total: i64 = b.conn.query_row("SELECT COUNT(*) FROM caixa_aprovado", [], |l| l.get(0)).unwrap();
    assert_eq!((total, j.novas, j.recusadas), (5, 4, 1), "o registro parou no teto (1 da raiz + 4): {j:?}");
}

/// C2R3-5: a entrada pelo convite junta só o que entra na cadeia da raiz da loja (o
/// lixo bem assinado que o servidor mandar junto fica de fora).
#[test]
fn c2r3_5_entrada_pelo_convite_nao_guarda_lixo() {
    use crate::troca::protocolo4::canal;
    thread::spawn(|| {
        let cb = temp("c2r3-5-novo");
        let b = caixa_novo_sem_senha(&cb, "Caixa 2");
        let _ = codigo_do_caixa_novo(&b, &cb);
        let raiz = std::sync::Arc::new(Identidade::gerar().unwrap());
        let loja = "loja-c2r3-5";
        let segredo = "AAAAA-BBBBB-CCCCC-DDDDD-EEEEE-F";
        let rb = temp("c2r3-5-servidor");
        let r = Banco::abrir(&rb).unwrap();
        let ur = crate::faixas::para_testes::configurar(&r, "Raiz", 0);
        aprovados::registrar_raiz(&r.conn, &raiz, loja, "Raiz", &ur).unwrap();
        let ouvinte = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endereco = ouvinte.local_addr().unwrap();
        let rr = raiz.clone();
        let servidor = thread::spawn(move || {
            let (fluxo, _) = ouvinte.accept().unwrap();
            let canal::Aceito::Ingresso(mut c) =
                canal::aceitar_com_ingresso(fluxo, &rr, &Default::default(), Duration::from_secs(5), None).unwrap()
            else {
                panic!("chave desconhecida cai no ingresso");
            };
            let (pedido, _prova) = c.receber_ingressar(64 * 1024, Duration::from_secs(5), None).unwrap();
            aprovados::aprovar(&r.conn, &rr, &rr.publica(), loja, &pedido, "Caixa 2", &ur).unwrap();
            let mut linhas = aprovados::linhas(&r.conn, loja).unwrap();
            for i in 0..30 {
                let x = Identidade::gerar().unwrap();
                let y = Identidade::gerar().unwrap();
                linhas.aprovacoes.push(aprovacao_a_mao(&x, &y, loja, &format!("disp-lixo-{i}")));
            }
            let hash = cofre::hash_segredo(&cofre::normalizar_segredo(segredo));
            let prova = canal::gerar_prova_servidor(
                &hash,
                c.exportador_convite(),
                &c.chave_do_par(),
                &rr.publica(),
                &rr.publica(),
                loja,
            )
            .unwrap();
            let resposta = canal::MensagemIngresso::Aprovado {
                prova_servidor: prova,
                raiz: rr.publica(),
                loja_id: loja.into(),
                linhas,
            };
            c.responder(&resposta, Duration::from_secs(5), None).unwrap();
            c.encerrar();
        });
        let qr = format!("RUST4|{segredo}|{loja}|{}|{endereco}", hex(&raiz.publica()));
        let resultado = juntar_com_convite(&cb, &qr, &[]);
        servidor.join().unwrap();
        resultado.expect("a entrada legítima vale");
        let n: i64 = b.conn.query_row("SELECT COUNT(*) FROM caixa_aprovado", [], |l| l.get(0)).unwrap();
        assert_eq!(n, 2, "só a raiz e a aprovação deste caixa; o lixo ficou de fora");
    })
    .join()
    .unwrap();
}

/// C2R3-5: retirar uma identidade que não é deste caixa é recusado (e nada muda).
#[test]
fn c2r3_5_retirar_identidade_de_outro_caixa_e_recusado() {
    let (ca, _cb, a, b, loja, ia, ib) = loja_com_dois_caixas_aprovados("c2r3-5-chave");
    let disp_a = config::ler(&a.conn, "dispositivo_id").unwrap().unwrap();
    let r = retirar_identidade_do_caixa(&a.conn, &ca, &disp_a, &hex(&ib.identidade.publica()), "engano", "Ana");
    assert!(r.is_err(), "a chave do caixa 2 não é uma identidade do caixa 1");
    let raiz = ia.identidade.publica();
    assert!(aprovados::autorizada(&a.conn, &raiz, &loja, &ib.identidade.publica()).unwrap(), "nada mudou");
    let _ = b;
}

/// C2R3-7: a mesma anotação só uma vez a cada 10 minutos.
#[test]
fn c2r3_7_anotacao_repetida_sai_uma_vez() {
    let texto = format!("teste c2r3-7 {}", crate::tempo::novo_id());
    let antes = crate::registro::ler_teste_contagem(&texto);
    for _ in 0..5 {
        p4::anotar_sem_repetir(&texto);
    }
    assert_eq!(crate::registro::ler_teste_contagem(&texto) - antes, 1);
}

/// C2R3-8: na lista de caixas, cada identidade do caixa ambíguo diz quem cai junto.
#[test]
fn c2r3_8_identidade_do_caixa_ambiguo_diz_quem_cai_junto() {
    let (_ca, _cb, a, b, loja, ia, _ib) = loja_com_dois_caixas_aprovados("c2r3-8");
    let disp_b = config::ler(&b.conn, "dispositivo_id").unwrap().unwrap();
    let outra = Identidade::gerar().unwrap();
    let neto = Identidade::gerar().unwrap();
    aprovados::juntar(
        &a.conn,
        &loja,
        &aprovados::Linhas {
            aprovacoes: vec![
                aprovacao_a_mao(&ia.identidade, &outra, &loja, &disp_b),
                aprovacao_a_mao(&outra, &neto, &loja, "disp-neto-c2r3-8"),
            ],
            retiradas: vec![],
        },
    )
    .unwrap();
    let lista = caixas_conhecidos(&a.conn).unwrap();
    let c = lista.iter().find(|c| c.dispositivo == disp_b).unwrap();
    let da_outra =
        c.identidades.iter().find(|i| i["chave"] == hex(&outra.publica())).expect("a outra identidade aparece");
    assert_eq!(da_outra["dependentes"], json!(["Caixa disp-neto-c2r3-8"]), "{da_outra}");
}
