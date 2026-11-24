//! Testes dos achados da revisão R1 do Pacote 2C (Sonnet 5.5 xhigh, 24/11/2026;
//! `revisoes/04_REVISAO_2C_SONNET.md`): cada um reprova no código de `d45d63e` e passa
//! com a correção. Também as mutações M1–M7 que o revisor viu sobreviverem.

use super::testes_p4_troca::{
    caixa_novo_sem_senha, codigo_do_caixa_novo, identidade, loja_com_dois_caixas_aprovados, peca_nova, tem_peca,
};
use super::testes_seguranca_caixas::{caixa_vazio_em_arquivo, loja_em_arquivo, motor, temp};
use super::*;
use crate::banco::Banco;
use crate::troca::protocolo4::{aprovados, cofre, hex, Dominio, Identidade};

/// Uma aprovação montada à mão (assinada por `quem_assina`), para os casos de
/// registro estranho que a função `aprovar` não deixa fazer.
fn aprovacao_a_mao(
    quem_assina: &Identidade,
    chave: &Identidade,
    loja: &str,
    nome: &str,
    dispositivo: &str,
    quando: &str,
) -> aprovados::Aprovacao {
    let mut a = aprovados::Aprovacao {
        chave: chave.publica(),
        chave_troca: chave.publica_troca(),
        loja_id: loja.into(),
        nome: nome.into(),
        aprovador: quem_assina.publica(),
        gerente: "Ana".into(),
        aprovado_em: quando.into(),
        dispositivo: dispositivo.into(),
        assinatura: [0u8; 64],
    };
    a.assinatura = quem_assina.assinar(Dominio::Aprovacao, &a.corpo());
    a
}

/// C2-1: o prazo da convivência é **deste caixa** (não vai para os outros) e é lido
/// com desconfiança: data longe demais, texto que não é data e data sem hora não
/// valem; e o Início não entra em pânico com eles.
#[test]
fn c2_1_convivencia_e_deste_caixa_e_lida_com_desconfianca() {
    let ca = temp("c2-1");
    let (a, _ua) = loja_em_arquivo(&ca, "Caixa 1");
    p4::garantir_raiz(&a.conn, &ca).unwrap();
    assert!(!CONFIG_DA_LOJA.contains(&p4::CHAVE_CONVIVENCIA), "o prazo não vai para os outros caixas");
    for v in ["9999-12-31T23:59:59Z", "zzzzzzzzzéé", "2099-01-01", "  "] {
        config::gravar(&a.conn, p4::CHAVE_CONVIVENCIA, v).unwrap();
        assert_eq!(convivencia_p3_ate(&a.conn).unwrap(), None, "{v:?} não vale como convivência");
        assert!(!p4::p3_permitido(&a.conn), "{v:?}: o P3 continua recusado");
        let avisos = crate::avisos::do_dia(&a.conn).expect("o Início abre sem pânico");
        assert!(!avisos.iter().any(|x| x.id == "p4.convivencia"), "{v:?}: sem aviso de convivência");
    }
    assert_eq!(
        crate::fiscal::data_br("zzzzzzzzzéé"),
        "zzzzzzzzzéé",
        "texto que não é data volta como está, sem pânico"
    );
    let ate = definir_convivencia_p3(&a.conn, Some(30), "Ana").unwrap().unwrap();
    assert_eq!(convivencia_p3_ate(&a.conn).unwrap().as_deref(), Some(ate.as_str()), "o prazo de 30 dias vale");
}

/// C2-3: o registro (que só cresce) passa do limite do pedido (64 KiB) e a troca
/// continua, nos dois sentidos, e o pedido de faixa de códigos também.
#[test]
fn c2_3_registro_grande_nao_para_a_troca() {
    thread::spawn(|| {
        let (ca, cb, a, mut b, loja, ia, _ib) = loja_com_dois_caixas_aprovados("c2-3");
        // C2R4-1: um caixa aprova no máximo 100 caixas que continuam na loja; as 150
        // aprovações ficam divididas entre a raiz e um caixa aprovado por ela.
        let raiz = ia.identidade.publica();
        let outro = Identidade::gerar().unwrap();
        let p = aprovados::Pedido::com_dispositivo(&outro, "disp-grande-aprovador");
        aprovados::aprovar(&a.conn, &ia.identidade, &raiz, &loja, &p, "Caixa aprovador", "Ana").unwrap();
        for i in 0..150 {
            let c = Identidade::gerar().unwrap();
            let p = aprovados::Pedido::com_dispositivo(&c, &format!("disp-grande-{i:03}"));
            let quem = if i % 2 == 0 { &ia.identidade } else { &outro };
            aprovados::aprovar(&a.conn, quem, &raiz, &loja, &p, &format!("Caixa {i}"), "Ana").unwrap();
        }
        aprovados::juntar(&b.conn, &loja, &aprovados::linhas(&a.conn, &loja).unwrap()).unwrap();
        let tamanho = serde_json::to_vec(&aprovados::linhas(&b.conn, &loja).unwrap()).unwrap().len();
        assert!(tamanho > TAMANHO_PEDIDO, "o registro passa do limite do pedido ({tamanho} bytes)");
        peca_nova(&a, "Peça com registro grande no 1");
        peca_nova(&b, "Peça com registro grande no 2");
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        let r1 = puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b);
        let r2 = puxar(&ca, &format!("127.0.0.1:{pt_b}"), pt_a);
        let disp_a = config::ler(&a.conn, "dispositivo_id").unwrap().unwrap();
        let ctx_b = p4::contexto(&b.conn, &cb);
        let alvo: SocketAddr = format!("127.0.0.1:{pt_a}").parse().unwrap();
        let faixa = pedir_faixa(&mut b.conn, None, &alvo, None, ctx_b.as_ref(), &disp_a);
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        r1.expect("o caixa 2 puxa do 1 com o registro grande");
        r2.expect("o caixa 1 puxa do 2 com o registro grande");
        assert!(tem_peca(&b, "Peça com registro grande no 1") && tem_peca(&a, "Peça com registro grande no 2"));
        faixa.expect("o pedido de faixa pelo P4 com o registro grande");
    })
    .join()
    .unwrap();
}

/// C2-5 (servidor): com o registro ambíguo para o `dispositivo` do caixa 2 (duas chaves
/// autorizadas), um caixa só do P3 que diz ser o caixa 2 continua recusado, mesmo
/// com a convivência ligada (o erro conta como "tem chave").
#[test]
fn c2_5_rebaixamento_recusado_com_registro_ambiguo() {
    thread::spawn(|| {
        let (ca, _cb, a, b, loja, ia, _ib) = loja_com_dois_caixas_aprovados("c2-5");
        let disp_b = config::ler(&b.conn, "dispositivo_id").unwrap().unwrap();
        let outra = Identidade::gerar().unwrap();
        let linha = aprovacao_a_mao(&ia.identidade, &outra, &loja, "Caixa 2 de novo", &disp_b, &crate::tempo::agora());
        aprovados::juntar(&a.conn, &loja, &aprovados::Linhas { aprovacoes: vec![linha], retiradas: vec![] }).unwrap();
        let raiz = ia.identidade.publica();
        assert!(aprovados::chave_do_dispositivo(&a.conn, &raiz, &loja, &disp_b).is_err(), "o registro ficou ambíguo");
        peca_nova(&a, "Peça Que Não Pode Vazar C25");
        definir_convivencia_p3(&a.conn, Some(7), "Ana").unwrap();

        let cc = temp("c2-5-p3");
        let c = caixa_vazio_em_arquivo(&cc, "Caixa Falso");
        config::gravar(&c.conn, "dispositivo_id", &disp_b).unwrap();
        let (mut ma, pt_a) = motor(&ca);
        let (mut mc, pt_c) = motor(&cc);
        let _ = puxar(&cc, &format!("127.0.0.1:{pt_a}"), pt_c);
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mc.encerrar(Duration::from_secs(20)));
        assert!(
            !tem_peca(&c, "Peça Que Não Pode Vazar C25"),
            "o caixa do P3 que diz ser o caixa 2 recebeu dados da loja"
        );
    })
    .join()
    .unwrap();
}

/// C2-5 (cliente): sem o contexto do P4 (a identidade deste caixa não abriu agora), o
/// par que tem chave no registro não é procurado pelo P3: erro, sem rebaixar.
#[test]
fn c2_5_par_com_chave_sem_identidade_nao_cai_para_o_p3() {
    let (_ca, _cb, a, b, _loja, _ia, _ib) = loja_com_dois_caixas_aprovados("c2-5-cliente");
    definir_convivencia_p3(&b.conn, Some(7), "Ana").unwrap();
    let disp_a = config::ler(&a.conn, "dispositivo_id").unwrap().unwrap();
    let k = Chaves::de(&b.conn, &ajustes(&b.conn).unwrap().senha).unwrap();
    let alvo: SocketAddr = "127.0.0.1:9".parse().unwrap();
    let r = abrir_fio(&b.conn, Some(&k), &alvo, None, None, &disp_a, None);
    let e = r.err().expect("sem o P4, o par com chave não é procurado").to_string();
    assert_eq!(e, p4::TEXTO_P4_INDISPONIVEL);
}

/// C2-7: um pacote com a mágica `RSA4` que não é anúncio válido (lixo, assinatura
/// errada, outra loja) não faz avaliar o registro.
#[test]
fn c2_7_anuncio_invalido_nao_avalia_o_registro() {
    let (ca, _cb, a, _b, loja, ia, _ib) = loja_com_dois_caixas_aprovados("c2-7");
    let ctx = p4::contexto(&a.conn, &ca).expect("o caixa 1 fala o P4");
    let mut rep = crate::troca::protocolo4::anuncio::Repeticoes::nova();
    let estranho = Identidade::gerar().unwrap();
    let mut lixo = crate::troca::protocolo4::anuncio::montar(&estranho, &loja, 4242, agora_ms()).unwrap();
    let n = lixo.len();
    lixo[n - 1] ^= 0xFF;
    let outra_loja = crate::troca::protocolo4::anuncio::montar(&ia.identidade, "outra-loja", 4242, agora_ms()).unwrap();
    let mut so_magica = crate::troca::protocolo4::anuncio::MAGICA_ANUNCIO.to_vec();
    so_magica.extend_from_slice(&[0u8; 100]);
    let antes = p4::AVALIACOES_DO_REGISTRO.with(|c| c.get());
    for pacote in [&lixo, &outra_loja, &so_magica] {
        assert_eq!(p4::ler_anuncio(&a.conn, &ctx, pacote, &mut rep), None);
    }
    assert_eq!(p4::AVALIACOES_DO_REGISTRO.with(|c| c.get()), antes, "nenhum pacote inválido avaliou o registro");
}

/// C2-9: o caixa aprovado que ainda não trocou dados (só existe no registro) aparece
/// na lista de caixas e pode ser retirado pela chave.
#[test]
fn c2_9_caixa_so_do_registro_aparece_e_pode_ser_retirado() {
    let ca = temp("c2-9");
    let (a, ua) = loja_em_arquivo(&ca, "Caixa 1");
    p4::garantir_raiz(&a.conn, &ca).unwrap();
    let loja = crate::loja::id(&a.conn).unwrap();
    let ia = identidade(&ca);
    let c3 = Identidade::gerar().unwrap();
    let p = aprovados::Pedido::com_dispositivo(&c3, "disp-perdido-c29");
    aprovados::aprovar(&a.conn, &ia.identidade, &ia.identidade.publica(), &loja, &p, "Caixa Perdido", &ua).unwrap();
    let lista = caixas_conhecidos(&a.conn).unwrap();
    let c = lista.iter().find(|c| c.dispositivo == "disp-perdido-c29").expect("o caixa do registro aparece");
    assert_eq!(c.nome, "Caixa Perdido");
    assert!(c.tem_chave);
    retirar_caixa_por_chave(&a.conn, &ca, "disp-perdido-c29", "perdido antes da primeira troca", &ua, "Ana").unwrap();
    let av = aprovados::avaliar(&a.conn, &ia.identidade.publica(), &loja).unwrap();
    assert_eq!(av.situacao(&c3.publica()), aprovados::Situacao::Retirada);
}

/// C2-13: depois de uma falha da identidade, o motor espera antes de tentar abrir de
/// novo (cada conexão repetiria o cofre e o Argon2); passada a espera, tenta.
#[test]
fn c2_13_espera_depois_de_falha_da_identidade() {
    let (ca, _cb, a, _b, _loja, _ia, _ib) = loja_com_dois_caixas_aprovados("c2-13");
    assert!(p4::contexto(&a.conn, &ca).is_some());
    p4::marcar_falha_do_contexto(&ca, Duration::from_secs(0));
    assert!(p4::contexto(&a.conn, &ca).is_none(), "logo depois da falha, não tenta de novo");
    p4::marcar_falha_do_contexto(&ca, Duration::from_secs(31));
    assert!(p4::contexto(&a.conn, &ca).is_some(), "passada a espera, tenta de novo");
}

/// C2-14: o distribuidor que já ouve caixas atualizados da loja (anúncio assinado) não
/// cria uma raiz nova: a loja já tem uma.
#[test]
fn c2_14_raiz_nao_nasce_onde_a_loja_ja_tem_caixas_atualizados() {
    let ca = temp("c2-14");
    let (a, _ua) = loja_em_arquivo(&ca, "Caixa 1");
    let loja = crate::loja::id(&a.conn).unwrap();
    let outro = Identidade::gerar().unwrap();
    let pacote = crate::troca::protocolo4::anuncio::montar(&outro, &loja, 4242, agora_ms()).unwrap();
    p4::guardar_ouvido(&ca, "127.0.0.1".parse().unwrap(), &pacote);
    p4::garantir_raiz(&a.conn, &ca).unwrap();
    assert!(!p4::tem_raiz_fixada(&a.conn), "não criou raiz nova");
    let n: i64 = a.conn.query_row("SELECT COUNT(*) FROM caixa_aprovado", [], |l| l.get(0)).unwrap();
    assert_eq!(n, 0);
}

/// C2-15 (DP-31): sem a senha da rede, as portas da troca abrem só no caixa do P4
/// (raiz fixada); sem raiz e sem senha, ficam fechadas.
#[test]
fn c2_15_portas_sem_senha_so_com_raiz() {
    thread::spawn(|| {
        for (nome, com_raiz) in [("c2-15-raiz", true), ("c2-15-sem", false)] {
            let c = temp(nome);
            let b = caixa_novo_sem_senha(&c, "Caixa");
            if com_raiz {
                config::gravar(&b.conn, "p4_raiz", &hex(&Identidade::gerar().unwrap().publica())).unwrap();
            }
            salvar_ajustes(&b.conn, true, "", "").ok();
            config::gravar(&b.conn, "sync_ligado", "1").unwrap();
            let mut m = Motor::iniciar(c.clone(), 0, 0);
            let portas = m.portas(Duration::from_secs(3));
            assert!(m.encerrar(Duration::from_secs(20)));
            assert_eq!(portas.is_some(), com_raiz, "{nome}: portas {portas:?}");
        }
    })
    .join()
    .unwrap();
}

/// M1: o cliente confere que o caixa que respondeu é o da chave do canal (I3 do lado de
/// quem puxa): um servidor aprovado que diz ser outro caixa é recusado.
#[test]
fn m1_cliente_recusa_servidor_que_diz_ser_outro_caixa() {
    thread::spawn(|| {
        let (ca, cb, a, b, _loja, _ia, _ib) = loja_com_dois_caixas_aprovados("m1");
        peca_nova(&b, "Peça do servidor que mente");
        config::gravar(&b.conn, "dispositivo_id", "disp-de-outro-caixa-m1").unwrap();
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        let r = puxar(&ca, &format!("127.0.0.1:{pt_b}"), pt_a);
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        assert_eq!(r.err().map(|e| e.to_string()).as_deref(), Some(p4::TEXTO_CAIXA_NAO_CONFERE));
        assert!(!tem_peca(&a, "Peça do servidor que mente"));
    })
    .join()
    .unwrap();
}

/// M2 e M7: o caixa do P4 anuncia sempre com a chave (`RSA4`); o anúncio cifrado com a
/// senha da rede (`RST3`) só sai durante a convivência.
#[test]
fn m2_m7_anuncios_que_saem() {
    let (ca, _cb, a, _b, _loja, _ia, _ib) = loja_com_dois_caixas_aprovados("m2-m7");
    let aj = ajustes(&a.conn).unwrap();
    let ctx = p4::contexto(&a.conn, &ca).expect("o caixa 1 fala o P4");
    let k = Chaves::de(&a.conn, &aj.senha).unwrap();
    let nomes = |k: Option<&Chaves>| -> Vec<&str> {
        p4::pacotes_de_anuncio(k, Some(&ctx), &aj, 4242).into_iter().map(|(n, _)| n).collect()
    };
    assert!(!p4::anuncio_p3_permitido(&a.conn, &aj), "fora da convivência, nada tirado da senha");
    assert_eq!(nomes(None), vec!["RSA4"], "fora da convivência, só o anúncio assinado");
    definir_convivencia_p3(&a.conn, Some(7), "Ana").unwrap();
    assert!(p4::anuncio_p3_permitido(&a.conn, &aj));
    assert_eq!(nomes(Some(&k)), vec!["RST3", "RSA4"], "na convivência, os dois");
}

/// M3: o caixa novo só fixa a raiz que o registro da loja diz ser a raiz.
#[test]
fn m3_raiz_recebida_tem_de_ser_a_do_registro() {
    let (_ca, _cb, a, _b, loja, _ia, _ib) = loja_com_dois_caixas_aprovados("m3");
    let cn = temp("m3-novo");
    let n = Banco::abrir(&cn).unwrap();
    aprovados::juntar(&n.conn, &loja, &aprovados::linhas(&a.conn, &loja).unwrap()).unwrap();
    let falsa = Identidade::gerar().unwrap().publica();
    assert!(cofre::fixar_raiz_recebida(&n.conn, &falsa, &loja).is_err(), "raiz que não é a do registro");
    assert!(!p4::tem_raiz_fixada(&n.conn));
}

/// M4: o anúncio de uma chave autorizada cuja aprovação mais nova diz outro
/// `dispositivo` (que aponta para outra chave) não vira par (I3 do anúncio).
#[test]
fn m4_anuncio_com_dispositivo_de_outra_chave_nao_vira_par() {
    let (ca, cb, a, b, loja, ia, ib) = loja_com_dois_caixas_aprovados("m4");
    let disp_a = config::ler(&a.conn, "dispositivo_id").unwrap().unwrap();
    let estranho = Identidade::gerar().unwrap();
    // Uma aprovação mais nova do caixa 2 dizendo ser o caixa 1, assinada por quem não
    // está autorizado (a linha confere sozinha, mas não entra na cadeia).
    let linha = aprovacao_a_mao(&estranho, &ib.identidade, &loja, "Caixa 1?", &disp_a, "2999-01-01T00:00:00Z");
    aprovados::juntar(&a.conn, &loja, &aprovados::Linhas { aprovacoes: vec![linha], retiradas: vec![] }).unwrap();
    let ctx_a = p4::contexto(&a.conn, &ca).unwrap();
    let ctx_b = p4::contexto(&b.conn, &cb).unwrap();
    let pacote = p4::pacote_de_anuncio(&ctx_b, 4242).unwrap();
    let mut rep = crate::troca::protocolo4::anuncio::Repeticoes::nova();
    assert_eq!(p4::ler_anuncio(&a.conn, &ctx_a, &pacote, &mut rep), None, "o caixa 2 não vira o caixa 1");
    let _ = ia;
}

/// M5: restaurar um backup apaga os segredos dos convites que vieram nele (um convite
/// antigo não volta a valer).
#[test]
fn m5_restaurar_backup_apaga_os_segredos_dos_convites() {
    let _vez = crate::backup::testes::um_por_vez();
    let caminho = temp("m5");
    let b = Banco::abrir(&caminho).unwrap();
    let u = crate::faixas::para_testes::configurar(&b, "Caixa 1", 0);
    let loja = crate::loja::criar(&b.conn).unwrap();
    let raiz = Identidade::gerar().unwrap();
    aprovados::registrar_raiz(&b.conn, &raiz, &loja, "Caixa 1", &u).unwrap();
    let codigo = Identidade::gerar().unwrap().codigo_de_pedido();
    cofre::gerar_convite(&b.conn, &raiz, &raiz.publica(), &loja, &codigo, "Caixa 3", &u, "").unwrap();
    let ativos = |b: &Banco| -> i64 {
        b.conn.query_row("SELECT COUNT(*) FROM p4_convite WHERE segredo_ativo IS NOT NULL", [], |l| l.get(0)).unwrap()
    };
    assert_eq!(ativos(&b), 1);
    drop(b);
    let feito = crate::backup::fazer(&caminho, false).unwrap();
    let mut b = Banco::abrir(&caminho).unwrap();
    crate::backup::restaurar(&mut b.conn, Some(&caminho), &feito.arquivo, false, "1234").unwrap();
    assert_eq!(ativos(&b), 0, "o convite do backup não volta a valer");
}

/// M6: a resposta de entrada de uma loja diferente da que o QR do convite diz é
/// recusada (nada fica fixado), mesmo com a prova do convite e a aprovação certas.
#[test]
fn m6_entrada_com_loja_diferente_da_do_qr_e_recusada() {
    use crate::troca::protocolo4::canal;
    thread::spawn(|| {
        let cb = temp("m6-novo");
        let b = caixa_novo_sem_senha(&cb, "Caixa 2");
        let _ = codigo_do_caixa_novo(&b, &cb);
        let raiz = std::sync::Arc::new(Identidade::gerar().unwrap());
        let loja = "loja-do-servidor-m6";
        let segredo = "AAAAA-BBBBB-CCCCC-DDDDD-EEEEE-F";
        let rb = temp("m6-servidor");
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
                linhas: aprovados::linhas(&r.conn, loja).unwrap(),
            };
            c.responder(&resposta, Duration::from_secs(5), None).unwrap();
            c.encerrar();
        });
        let qr = format!("RUST4|{segredo}|loja-que-o-qr-diz-m6|{}|{endereco}", hex(&raiz.publica()));
        let resultado = juntar_com_convite(&cb, &qr, &[]);
        servidor.join().unwrap();
        assert!(resultado.is_err(), "a loja da resposta não é a do QR: {resultado:?}");
        assert!(!p4::tem_raiz_fixada(&b.conn), "nenhuma raiz foi fixada");
        assert!(crate::loja::da_entrada(&b.conn).unwrap().is_empty());
    })
    .join()
    .unwrap();
}
