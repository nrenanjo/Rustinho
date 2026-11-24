//! T008 (0.9.19-dev.2, `roadmap.md` §5.6, DT-P4-29): a troca de dados pelo fio do
//! protocolo 4 entre caixas aprovados de verdade (raiz, aprovação assinada e
//! registro juntado nos dois bancos).
//!
//! A entrada pelo convite (etapa 5) é da T009; aqui o caixa 2 entra na loja como
//! hoje (protocolo 3) e é aprovado pelo gerente no caixa 1, como a A4 prevê para os
//! caixas que já trocavam dados.

use super::testes_seguranca_caixas::{caixa_vazio_em_arquivo, loja_em_arquivo, motor, temp};
use super::*;
use crate::banco::Banco;
use crate::troca::protocolo4::{aprovados, cofre, hex};

/// A identidade (deste processo) do caixa cujo banco está em `caminho`.
fn identidade(caminho: &Path) -> Arc<cofre::IdentidadeLocal> {
    cofre::identidade_do_processo(caminho.parent().unwrap(), &cofre::Modo::Instalado).unwrap()
}

/// O caixa 1 (distribuidor, gerente "Ana") vira a raiz; o caixa 2 entra na loja pelo
/// caminho de hoje e é aprovado no caixa 1; os dois juntam o registro. Devolve os
/// bancos, a loja e as identidades.
#[allow(clippy::type_complexity)]
fn loja_com_dois_caixas_aprovados(
    nome: &str,
) -> (PathBuf, PathBuf, Banco, Banco, String, Arc<cofre::IdentidadeLocal>, Arc<cofre::IdentidadeLocal>) {
    let (ca, cb) = (temp(&format!("{nome}-a")), temp(&format!("{nome}-b")));
    let (a, _ua) = loja_em_arquivo(&ca, "Caixa 1");

    // O caixa 2 já trocava dados com o 1 (protocolo 3), antes de o 1 virar raiz (A4:
    // os caixas que já trocavam dados são aprovados uma vez pelo gerente).
    let b = caixa_vazio_em_arquivo(&cb, "Caixa 2");
    {
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b).unwrap();
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
    }
    p4::garantir_raiz(&a.conn, &ca).unwrap();
    let loja = crate::loja::id(&a.conn).unwrap();
    let id_a = identidade(&ca);
    assert_eq!(cofre::raiz_atual(&a.conn, &loja).unwrap(), Some(id_a.identidade.publica()), "o caixa 1 é a raiz");
    assert_eq!(crate::loja::id(&b.conn).unwrap(), loja, "o caixa 2 recebeu a loja");

    // O gerente aprova o caixa 2 no caixa 1; o caixa 2 junta o registro e fixa a raiz.
    let id_b = identidade(&cb);
    let disp_b = config::ler(&b.conn, "dispositivo_id").unwrap().unwrap();
    let pedido = aprovados::Pedido::com_dispositivo(&id_b.identidade, &disp_b);
    aprovados::aprovar(&a.conn, &id_a.identidade, &id_a.identidade.publica(), &loja, &pedido, "Caixa 2", "Ana")
        .unwrap();
    aprovados::juntar(&b.conn, &loja, &aprovados::linhas(&a.conn, &loja).unwrap()).unwrap();
    cofre::garantir_raiz_se_aplicavel(&b.conn, &id_b.identidade, false, &loja, "Caixa 2", "", false).unwrap();
    assert_eq!(cofre::raiz_atual(&b.conn, &loja).unwrap(), Some(id_a.identidade.publica()), "o caixa 2 fixou a raiz");
    (ca, cb, a, b, loja, id_a, id_b)
}

/// Uma peça nova no caixa `b` (com o usuário que a loja tem).
fn peca_nova(b: &Banco, nome: &str) -> String {
    let u: String = b.conn.query_row("SELECT id FROM usuario LIMIT 1", [], |l| l.get(0)).unwrap();
    let mut f = crate::produtos::ficha_nova(&b.conn).unwrap();
    f.nome = nome.into();
    f.custo = "10".into();
    crate::produtos::salvar(&b.conn, &b.dispositivo, &u, &f).unwrap()
}

fn tem_peca(b: &Banco, nome: &str) -> bool {
    b.conn.query_row("SELECT COUNT(*) FROM produto WHERE nome = ?1", [nome], |l| l.get::<_, i64>(0)).unwrap() > 0
}

/// Dois caixas aprovados trocam dados pelo fio autenticado, nos dois sentidos.
#[test]
fn p4_dois_caixas_aprovados_trocam_pelo_fio_autenticado() {
    thread::spawn(|| {
        let (ca, cb, a, b, _loja, _ia, _ib) = loja_com_dois_caixas_aprovados("p4-troca");
        peca_nova(&a, "Peça do caixa 1 pelo P4");
        peca_nova(&b, "Peça do caixa 2 pelo P4");
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        let antes = p4::ATENDIDAS_PELO_P4.load(Ordering::SeqCst);
        puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b).unwrap();
        puxar(&ca, &format!("127.0.0.1:{pt_b}"), pt_a).unwrap();
        let pelo_p4 = p4::ATENDIDAS_PELO_P4.load(Ordering::SeqCst) - antes;
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        assert!(tem_peca(&b, "Peça do caixa 1 pelo P4"), "a peça do caixa 1 chegou ao 2");
        assert!(tem_peca(&a, "Peça do caixa 2 pelo P4"), "a peça do caixa 2 chegou ao 1");
        assert!(pelo_p4 >= 2, "as duas puxadas passaram pelo fio do protocolo 4 ({pelo_p4})");
    })
    .join()
    .unwrap();
}

/// Uma aprovação nova feita no caixa 1 chega ao caixa 2 pelo canal autenticado (o
/// registro não viaja pelo protocolo 3).
#[test]
fn p4_registro_novo_chega_pelo_canal() {
    thread::spawn(|| {
        let (ca, cb, a, b, loja, ia, _ib) = loja_com_dois_caixas_aprovados("p4-registro");
        let c3 = crate::troca::protocolo4::Identidade::gerar().unwrap();
        let pedido = aprovados::Pedido::com_dispositivo(&c3, "disp-caixa-3-p4");
        aprovados::aprovar(&a.conn, &ia.identidade, &ia.identidade.publica(), &loja, &pedido, "Caixa 3", "Ana")
            .unwrap();
        let raiz = ia.identidade.publica();
        assert!(!aprovados::autorizada(&b.conn, &raiz, &loja, &c3.publica()).unwrap());
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b).unwrap();
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        assert!(
            aprovados::autorizada(&b.conn, &raiz, &loja, &c3.publica()).unwrap(),
            "o caixa 3 chegou ao registro do 2"
        );
        let n: i64 = b
            .conn
            .query_row("SELECT COUNT(*) FROM alteracao WHERE tabela IN ('caixa_aprovado', 'caixa_retirado')", [], |l| {
                l.get(0)
            })
            .unwrap();
        assert_eq!(n, 0, "o registro não entra no log da troca do protocolo 3");
    })
    .join()
    .unwrap();
}

/// Caixa retirado deixa de trocar na conexão seguinte, e não cai para o protocolo 3.
#[test]
fn p4_caixa_retirado_deixa_de_trocar_e_nao_cai_para_o_p3() {
    thread::spawn(|| {
        let (ca, cb, a, b, loja, ia, ib) = loja_com_dois_caixas_aprovados("p4-retirado");
        aprovados::retirar(
            &a.conn,
            &ia.identidade,
            &ia.identidade.publica(),
            &loja,
            &ib.identidade.publica(),
            "saiu da loja",
            "Ana",
        )
        .unwrap();
        peca_nova(&a, "Peça depois da retirada");
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        let r = puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b);
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        assert!(r.is_err(), "o caixa retirado não troca mais: {r:?}");
        assert!(!tem_peca(&b, "Peça depois da retirada"), "nada chegou depois da retirada (nem pelo protocolo 3)");
    })
    .join()
    .unwrap();
}

/// O `dispositivo` declarado tem de ser o da aprovação da chave do canal (I3).
#[test]
fn p4_dispositivo_declarado_diferente_da_chave_e_recusado() {
    thread::spawn(|| {
        let (ca, cb, a, b, _loja, _ia, _ib) = loja_com_dois_caixas_aprovados("p4-dispositivo");
        peca_nova(&a, "Peça que não pode ir");
        // O caixa 2 passa a declarar o identificador de outro caixa.
        config::gravar(&b.conn, "dispositivo_id", "outro-caixa-qualquer-0001").unwrap();
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        let r = puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b);
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        assert!(r.is_err(), "o pedido com outro dispositivo é recusado: {r:?}");
        assert!(!tem_peca(&b, "Peça que não pode ir"));
    })
    .join()
    .unwrap();
}

/// DT-P4-14/28 (anti-rebaixamento): para o caixa que o registro diz ter chave, a
/// troca é só pelo P4 — nem quando quem responde no endereço dele só fala o
/// protocolo 3 (o que alguém faria para se passar por ele com a senha da rede).
#[test]
fn p4_par_com_chave_nao_cai_para_o_p3() {
    thread::spawn(|| {
        let (ca, cb, a, b, _loja, _ia, _ib) = loja_com_dois_caixas_aprovados("p4-rebaixar");
        // A convivência ligada nos dois (T010): o P3 em geral é permitido, e o que
        // barra é só o anti-rebaixamento (o caixa 1 tem chave).
        definir_convivencia_p3(&a.conn, Some(7), "Ana").unwrap();
        definir_convivencia_p3(&b.conn, Some(7), "Ana").unwrap();
        // Um terceiro caixa da mesma loja, que entrou pela senha da rede (só fala o P3).
        let cc = temp("p4-rebaixar-c");
        let c = caixa_vazio_em_arquivo(&cc, "Caixa 3");
        {
            let (mut ma, pt_a) = motor(&ca);
            let (mut mc, pt_c) = motor(&cc);
            puxar(&cc, &format!("127.0.0.1:{pt_a}"), pt_c).unwrap();
            assert!(ma.encerrar(Duration::from_secs(20)));
            assert!(mc.encerrar(Duration::from_secs(20)));
        }
        peca_nova(&c, "Peça de quem só fala o P3");
        let (mut mc, pt_c) = motor(&cc);
        let (mut mb, pt_b) = motor(&cb);
        // No caixa 2, o endereço do caixa 1 passa a ser o do caixa 3 (quem responde ali).
        let disp_a = config::ler(&a.conn, "dispositivo_id").unwrap().unwrap();
        b.conn
            .execute(
                "UPDATE sync_par SET endereco = ?2 WHERE dispositivo = ?1",
                params![disp_a, format!("127.0.0.1:{pt_c}")],
            )
            .unwrap();
        let r = puxar(&cb, &format!("127.0.0.1:{pt_c}"), pt_b);
        assert!(mc.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        assert!(r.is_err(), "o caixa com chave não pode ser trocado por quem só fala o P3: {r:?}");
        assert!(!tem_peca(&b, "Peça de quem só fala o P3"), "nada chegou pelo protocolo 3");
    })
    .join()
    .unwrap();
}

/// DT-P4-17: o anúncio assinado (`RSA4`) de um caixa aprovado vira par com o
/// `dispositivo` e o nome da aprovação; repetição, chave não aprovada, outra loja e o
/// próprio anúncio são recusados; e nada nele vem da senha da rede.
#[test]
fn p4_anuncio_assinado_vira_par_so_de_caixa_aprovado() {
    let (ca, cb, a, b, loja, _ia, _ib) = loja_com_dois_caixas_aprovados("p4-anuncio");
    let ctx_a = p4::contexto(&a.conn, &ca).expect("o caixa 1 fala o P4");
    let ctx_b = p4::contexto(&b.conn, &cb).expect("o caixa 2 fala o P4");
    let disp_a = config::ler(&a.conn, "dispositivo_id").unwrap().unwrap();
    let mut rep = crate::troca::protocolo4::anuncio::Repeticoes::nova();

    let pacote = p4::pacote_de_anuncio(&ctx_a, 4242).unwrap();
    assert_eq!(
        p4::ler_anuncio(&b.conn, &ctx_b, &pacote, &mut rep),
        Some((disp_a, "Caixa 1".to_string(), 4242)),
        "o anúncio do caixa 1 vira par no caixa 2"
    );
    assert_eq!(p4::ler_anuncio(&b.conn, &ctx_b, &pacote, &mut rep), None, "o mesmo anúncio de novo é repetição");

    // Chave que não foi aprovada, mesmo dizendo a loja certa.
    let estranho = crate::troca::protocolo4::Identidade::gerar().unwrap();
    let falso = crate::troca::protocolo4::anuncio::montar(&estranho, &loja, 4242, agora_ms()).unwrap();
    assert_eq!(p4::ler_anuncio(&b.conn, &ctx_b, &falso, &mut rep), None, "chave não aprovada");
    // A chave certa, mas de outra loja.
    let outra = crate::troca::protocolo4::anuncio::montar(&ctx_a.identidade.identidade, "outra-loja", 4242, agora_ms())
        .unwrap();
    assert_eq!(p4::ler_anuncio(&b.conn, &ctx_b, &outra, &mut rep), None, "outra loja");
    // O próprio anúncio (de volta por outra interface) não vira par.
    let meu = p4::pacote_de_anuncio(&ctx_b, 4242).unwrap();
    assert_eq!(p4::ler_anuncio(&b.conn, &ctx_b, &meu, &mut rep), None, "o próprio anúncio");

    // Nada da senha da rede no anúncio: nem o sal nem a chave tirada dela.
    let ajustes_a = ajustes(&a.conn).unwrap();
    let k = Chaves::de(&a.conn, &ajustes_a.senha).unwrap();
    let contem = |p: &[u8], agulha: &[u8]| p.windows(agulha.len()).any(|j| j == agulha);
    let novo = p4::pacote_de_anuncio(&ctx_a, 4242).unwrap();
    assert!(!contem(&novo, &k.sal) && !contem(&novo, &k.propria), "o anúncio assinado não leva nada da senha");
}

/// O caixa novo (banco vazio, **sem a senha da rede**) com a troca ainda desligada.
fn caixa_novo_sem_senha(caminho: &Path, nome: &str) -> Banco {
    let b = Banco::abrir(caminho).unwrap();
    config::gravar(&b.conn, "caixa_nome", nome).unwrap();
    config::gravar(&b.conn, "codigos_espera_teste", "0").unwrap();
    b
}

/// O código de pedido que a tela do caixa novo mostra (o gerente confere).
fn codigo_do_caixa_novo(b: &Banco, caminho: &Path) -> String {
    let id = identidade(caminho);
    let disp = config::ler(&b.conn, "dispositivo_id").unwrap().unwrap();
    aprovados::Pedido::com_dispositivo(&id.identidade, &disp).codigo()
}

/// T009 (etapa 5, A6, D1) de ponta a ponta: o gerente gera o convite no caixa 1 com o
/// código do caixa novo; o caixa novo, sem a senha da rede, entra pelo convite (o
/// endereço vem no QR), fica aprovado (com a auditoria), e recebe a loja pelo fio do
/// protocolo 4. O mesmo convite não serve de novo; um convite errado é recusado.
#[test]
fn p4_caixa_novo_entra_pelo_convite_sem_a_senha_da_rede() {
    thread::spawn(|| {
        let (ca, cb, cc) = (temp("conv-a"), temp("conv-b"), temp("conv-c"));
        let (a, ua) = loja_em_arquivo(&ca, "Caixa 1");
        crate::troca::sync::testes_seguranca_caixas::cliente(&a, &ua, "Cliente Da Loja Pelo Convite");
        p4::garantir_raiz(&a.conn, &ca).unwrap();
        let loja = crate::loja::id(&a.conn).unwrap();
        let ia = identidade(&ca);
        let (mut ma, pt_a) = motor(&ca);

        // O caixa novo mostra o código; o gerente gera o convite no caixa 1.
        let b = caixa_novo_sem_senha(&cb, "Caixa 2");
        let codigo = codigo_do_caixa_novo(&b, &cb);
        let conv = cofre::gerar_convite(
            &a.conn,
            &ia.identidade,
            &ia.identidade.publica(),
            &loja,
            &codigo,
            "Caixa 2",
            "Ana",
            &format!("127.0.0.1:{pt_a}"),
        )
        .unwrap();

        // Convite errado: recusado, e o caixa novo continua sem loja.
        let errado =
            format!("RUST4|AAAAA-BBBBB-CCCCC-DDDDD-EEEEE-F|{loja}|{}|127.0.0.1:{pt_a}", hex(&ia.identidade.publica()));
        assert!(juntar_com_convite(&cb, &errado, &[]).is_err(), "convite errado entra");
        assert!(!p4::tem_raiz_fixada(&b.conn));
        let id_b = identidade(&cb);
        assert!(
            !aprovados::autorizada(&a.conn, &ia.identidade.publica(), &loja, &id_b.identidade.publica()).unwrap(),
            "com o convite errado, o caixa 1 não pode ter aprovado o caixa novo"
        );

        // O convite certo (o QR, com o endereço do caixa 1).
        assert_eq!(juntar_com_convite(&cb, &conv.payload_qr, &[]).unwrap(), loja);
        assert!(p4::tem_raiz_fixada(&b.conn), "a raiz da loja ficou fixada no caixa novo");
        assert!(ajustes(&b.conn).unwrap().senha.is_empty(), "sem a senha da rede");
        let aprovacoes: i64 =
            a.conn.query_row("SELECT COUNT(*) FROM auditoria WHERE acao = 'aprovar_caixa'", [], |l| l.get(0)).unwrap();
        assert_eq!(aprovacoes, 1, "a entrada foi aprovada e auditada no caixa 1");

        // O caixa novo recebe a loja pelo fio do protocolo 4.
        let (mut mb, pt_b) = motor(&cb);
        let antes = p4::ATENDIDAS_PELO_P4.load(Ordering::SeqCst);
        puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b).unwrap();
        assert!(p4::ATENDIDAS_PELO_P4.load(Ordering::SeqCst) > antes, "pelo fio do protocolo 4");
        assert!(crate::troca::sync::testes_seguranca_caixas::tem_usuarios(&b) >= 1, "os usuários chegaram");
        assert!(crate::troca::sync::testes_seguranca_caixas::tem_cliente(&b, "Cliente Da Loja Pelo Convite"));
        let auditada: i64 = b
            .conn
            .query_row("SELECT COUNT(*) FROM auditoria WHERE acao LIKE '%aprova%caixa%'", [], |l| l.get(0))
            .unwrap();
        assert!(auditada >= 1, "a aprovação está na auditoria do caixa novo (veio com a loja)");

        // O mesmo convite não serve para outro caixa.
        let c = caixa_novo_sem_senha(&cc, "Caixa 3");
        let _ = codigo_do_caixa_novo(&c, &cc);
        assert!(juntar_com_convite(&cc, &conv.payload_qr, &[]).is_err(), "o convite já foi usado");
        assert!(!p4::tem_raiz_fixada(&c.conn));
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
    })
    .join()
    .unwrap();
}

/// T009: um servidor que conhece o convite (a prova dele confere) mas manda um
/// registro **sem** a aprovação deste caixa não faz o caixa novo "entrar": nada é
/// fixado (nem a raiz, nem a loja) e a troca não é ligada.
#[test]
fn p4_entrada_sem_a_propria_aprovacao_no_registro_e_recusada() {
    use crate::troca::protocolo4::{canal, Identidade};
    thread::spawn(|| {
        let cb = temp("conv-sem-aprovacao");
        let b = caixa_novo_sem_senha(&cb, "Caixa 2");
        let _ = codigo_do_caixa_novo(&b, &cb);
        let falsa_raiz = Arc::new(Identidade::gerar().unwrap());
        let loja = "loja-do-servidor-que-sabe-o-convite";
        let segredo = "AAAAA-BBBBB-CCCCC-DDDDD-EEEEE-F";
        // A raiz falsa se registra (linha autoassinada), mas não aprova o caixa novo.
        let raiz_banco = temp("conv-sem-aprovacao-raiz");
        let r = Banco::abrir(&raiz_banco).unwrap();
        let ur = crate::faixas::para_testes::configurar(&r, "Raiz", 0);
        aprovados::registrar_raiz(&r.conn, &falsa_raiz, loja, "Raiz", &ur).unwrap();
        let linhas = aprovados::linhas(&r.conn, loja).unwrap();
        let ouvinte = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endereco = ouvinte.local_addr().unwrap();
        let fr = falsa_raiz.clone();
        let servidor = thread::spawn(move || {
            let (fluxo, _) = ouvinte.accept().unwrap();
            let canal::Aceito::Ingresso(mut c) =
                canal::aceitar_com_ingresso(fluxo, &fr, &Default::default(), Duration::from_secs(5), None).unwrap()
            else {
                panic!("chave desconhecida cai no ingresso");
            };
            let _ = c.receber_ingressar(64 * 1024, Duration::from_secs(5), None).unwrap();
            let hash = cofre::hash_segredo(&cofre::normalizar_segredo(segredo));
            let prova = canal::gerar_prova_servidor(
                &hash,
                c.exportador_convite(),
                &c.chave_do_par(),
                &fr.publica(),
                &fr.publica(),
                loja,
            )
            .unwrap();
            let resposta = canal::MensagemIngresso::Aprovado {
                prova_servidor: prova,
                raiz: fr.publica(),
                loja_id: loja.into(),
                linhas,
            };
            c.responder(&resposta, Duration::from_secs(5), None).unwrap();
            c.encerrar();
        });
        let r = juntar_com_convite(&cb, segredo, &[endereco.to_string()]);
        servidor.join().unwrap();
        assert!(r.is_err(), "sem a própria aprovação no registro, a entrada não vale: {r:?}");
        assert!(!p4::tem_raiz_fixada(&b.conn), "nenhuma raiz foi fixada");
        assert!(crate::loja::da_entrada(&b.conn).unwrap().is_empty(), "nenhuma loja foi escolhida");
        assert!(!ajustes(&b.conn).unwrap().ligado, "a troca não foi ligada");
    })
    .join()
    .unwrap();
}

/// T009 (etapa 6; E5, P4-3, P4-4, S-10): retirar pela chave lista antes quem fica
/// sem cadeia; o retirado não troca mais, nem trocando o identificador declarado; a
/// raiz não é retirada (e nada é gravado); a retirada de antes vira assinada.
#[test]
fn p4_retirar_pela_chave_vale_por_maquina() {
    thread::spawn(|| {
        let (ca, cb, a, b, loja, ia, ib) = loja_com_dois_caixas_aprovados("p4-retirar");
        let raiz = ia.identidade.publica();
        let disp_a = config::ler(&a.conn, "dispositivo_id").unwrap().unwrap();
        let disp_b = config::ler(&b.conn, "dispositivo_id").unwrap().unwrap();
        // O caixa 2 aprovou um caixa 3: retirar o 2 deixaria o 3 sem cadeia (P4-4).
        let c3 = crate::troca::protocolo4::Identidade::gerar().unwrap();
        aprovados::aprovar(
            &a.conn,
            &ib.identidade,
            &raiz,
            &loja,
            &aprovados::Pedido::com_dispositivo(&c3, "disp-caixa-3-ret"),
            "Caixa 3",
            "Ana",
        )
        .unwrap();
        assert_eq!(dependentes_da_retirada(&a.conn, &ca, &disp_b).unwrap(), vec!["Caixa 3".to_string()]);

        // P4-3: a raiz não sai, e nada é gravado (nem o bloqueio de antes).
        assert!(retirar_caixa_por_chave(&b.conn, &cb, &disp_a, "teste", "Ana").is_err());
        assert!(!retirado(&b.conn, &disp_a).unwrap(), "o bloqueio de antes não ficou gravado sozinho");

        retirar_caixa_por_chave(&a.conn, &ca, &disp_b, "saiu da loja", "Ana").unwrap();
        assert!(retirado(&a.conn, &disp_b).unwrap(), "o bloqueio de antes (S-10) também vale");
        let av = aprovados::avaliar(&a.conn, &raiz, &loja).unwrap();
        assert_eq!(av.situacao(&ib.identidade.publica()), aprovados::Situacao::Retirada);
        assert_eq!(av.situacao(&c3.publica()), aprovados::Situacao::SemCadeia);

        // O retirado troca o identificador declarado e tenta de novo: recusado.
        peca_nova(&a, "Peça depois de retirar pela chave");
        config::gravar(&b.conn, "dispositivo_id", "outro-id-depois-da-retirada").unwrap();
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        let r = puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b);
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        assert!(r.is_err(), "o retirado não volta trocando o identificador: {r:?}");
        assert!(!tem_peca(&b, "Peça depois de retirar pela chave"));
    })
    .join()
    .unwrap();
}

/// S-10: a retirada de antes (texto na configuração, como a de um caixa do protocolo
/// 3) de um caixa que tem chave vira retirada assinada.
#[test]
fn p4_retirada_de_antes_vira_assinada() {
    let (ca, _cb, a, b, loja, ia, ib) = loja_com_dois_caixas_aprovados("p4-s10");
    let disp_b = config::ler(&b.conn, "dispositivo_id").unwrap().unwrap();
    retirar_caixa(&a.conn, &disp_b, "saiu na 0.9.18", "Ana").unwrap();
    let av = aprovados::avaliar(&a.conn, &ia.identidade.publica(), &loja).unwrap();
    assert_eq!(av.situacao(&ib.identidade.publica()), aprovados::Situacao::Autorizada, "ainda só o bloqueio de antes");
    assert_eq!(p4::converter_retiradas_antigas(&a.conn, &ca).unwrap(), 1);
    let av = aprovados::avaliar(&a.conn, &ia.identidade.publica(), &loja).unwrap();
    assert_eq!(av.situacao(&ib.identidade.publica()), aprovados::Situacao::Retirada, "virou retirada assinada");
    assert_eq!(p4::converter_retiradas_antigas(&a.conn, &ca).unwrap(), 0, "uma vez só");
}

/// O caixa 1 com loja; o caixa 2 entra pelo protocolo 3 (como um caixa antigo) antes
/// de o caixa 1 virar raiz. Devolve os dois e as portas dos motores não ficam abertas.
fn loja_com_caixa_antigo(nome: &str) -> (PathBuf, PathBuf, Banco, Banco) {
    let (ca, cb) = (temp(&format!("{nome}-a")), temp(&format!("{nome}-b")));
    let (a, _ua) = loja_em_arquivo(&ca, "Caixa 1");
    let b = caixa_vazio_em_arquivo(&cb, "Caixa Antigo");
    {
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b).unwrap();
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
    }
    p4::garantir_raiz(&a.conn, &ca).unwrap();
    assert!(p4::tem_raiz_fixada(&a.conn) && !p4::tem_raiz_fixada(&b.conn));
    (ca, cb, a, b)
}

fn puxa_do_caixa_1(ca: &Path, cb: &Path) -> Resultado<(String, usize)> {
    let (mut ma, pt_a) = motor(ca);
    let (mut mb, pt_b) = motor(cb);
    let r = puxar(cb, &format!("127.0.0.1:{pt_a}"), pt_b);
    assert!(ma.encerrar(Duration::from_secs(20)));
    assert!(mb.encerrar(Duration::from_secs(20)));
    r
}

/// T010 (A4): por padrão o caixa do protocolo 4 recusa o caixa do protocolo 3 (e não
/// manda o anúncio cifrado com a senha); o gerente libera por 1 a 30 dias (auditoria e
/// aviso no Início); desligada ou vencida, a recusa volta.
#[test]
fn p4_convivencia_com_caixas_antigos_por_prazo() {
    thread::spawn(|| {
        let (ca, cb, a, b) = loja_com_caixa_antigo("conviver");
        assert!(!p4::p3_permitido(&a.conn), "o padrão é recusar o protocolo 3");
        assert!(p4::p3_permitido(&b.conn), "o caixa que não é do P4 segue como antes");

        peca_nova(&a, "Peça para o caixa antigo");
        let r = puxa_do_caixa_1(&ca, &cb);
        assert!(r.is_err(), "sem a convivência, o caixa antigo é recusado: {r:?}");
        assert!(!tem_peca(&b, "Peça para o caixa antigo"));

        assert!(definir_convivencia_p3(&a.conn, Some(0), "Ana").is_err());
        assert!(definir_convivencia_p3(&a.conn, Some(CONVIVENCIA_MAXIMA_DIAS + 1), "Ana").is_err());
        assert!(definir_convivencia_p3(&a.conn, Some(7), "").is_err(), "só o gerente");
        let ate = definir_convivencia_p3(&a.conn, Some(7), "Ana").unwrap().expect("ficou ligada");
        assert_eq!(convivencia_p3_ate(&a.conn).unwrap().as_deref(), Some(ate.as_str()));
        let auditada: i64 = a
            .conn
            .query_row("SELECT COUNT(*) FROM auditoria WHERE acao = 'p4_convivencia_p3'", [], |l| l.get(0))
            .unwrap();
        assert_eq!(auditada, 1);
        let avisos = crate::avisos::do_dia(&a.conn).unwrap();
        assert!(avisos.iter().any(|v| v.id == "p4.convivencia"), "o Início avisa enquanto durar");

        puxa_do_caixa_1(&ca, &cb).unwrap();
        assert!(tem_peca(&b, "Peça para o caixa antigo"), "com a convivência, o caixa antigo troca dados");

        // Desligada: a recusa volta.
        definir_convivencia_p3(&a.conn, None, "Ana").unwrap();
        peca_nova(&a, "Peça depois de desligar");
        assert!(puxa_do_caixa_1(&ca, &cb).is_err());
        assert!(!tem_peca(&b, "Peça depois de desligar"));
        // Vencida: também.
        config::gravar(&a.conn, p4::CHAVE_CONVIVENCIA, "2020-01-01T00:00:00Z").unwrap();
        assert!(convivencia_p3_ate(&a.conn).unwrap().is_none());
        assert!(puxa_do_caixa_1(&ca, &cb).is_err());
        assert!(!crate::avisos::do_dia(&a.conn).unwrap().iter().any(|v| v.id == "p4.convivencia"));
    })
    .join()
    .unwrap();
}

/// T010 + DT-P4-14: mesmo com a convivência ligada, o caixa que tem chave no registro
/// não é atendido pelo protocolo 3 (sem rebaixamento pelo lado do servidor).
#[test]
fn p4_caixa_com_chave_nao_e_atendido_pelo_p3_nem_na_convivencia() {
    thread::spawn(|| {
        let (ca, cb, a, b, _loja, _ia, _ib) = loja_com_dois_caixas_aprovados("p3-com-chave");
        definir_convivencia_p3(&a.conn, Some(7), "Ana").unwrap();
        peca_nova(&a, "Peça que não vai pelo P3");
        // O caixa 2 volta a ser um caixa sem nada do protocolo 4 (fala só o P3), mas a
        // chave dele continua no registro do 1.
        b.conn
            .execute_batch(
                "DELETE FROM config WHERE chave IN ('p4_raiz', 'p4_chave_publica');
                 DELETE FROM caixa_aprovado; DELETE FROM caixa_retirado;",
            )
            .unwrap();
        assert!(p4::contexto(&b.conn, &cb).is_none(), "o caixa 2 não fala mais o P4");
        let r = puxa_do_caixa_1(&ca, &cb);
        assert!(r.is_err(), "quem tem chave só troca pelo P4: {r:?}");
        assert!(!tem_peca(&b, "Peça que não vai pelo P3"));
    })
    .join()
    .unwrap();
}
