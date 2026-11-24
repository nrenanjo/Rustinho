//! Testes dos achados da revisão R2 (Sonnet 5.5 xhigh, 24/11/2026;
//! `revisoes/04_REVISAO_2C_SONNET.md`, C2R2-1 a C2R2-12): cada um reprova no código
//! de `4716596`/`b6eefc7` e passa com a correção.

use super::testes_p4_troca::{identidade, loja_com_dois_caixas_aprovados, peca_nova, tem_peca};
use super::testes_seguranca_caixas::{loja_em_arquivo, motor, temp};
use super::*;
use crate::banco::Banco;
use crate::troca::protocolo4::{aprovados, cofre, hex, Dominio, Identidade};
use crate::Servico;

fn chamar(s: &mut Servico, comando: &str, dados: Value) -> Value {
    let mut dados = dados;
    if crate::api::NAO_REPETIR.contains(&comando) {
        dados["chaveOperacao"] = json!(crate::tempo::novo_id());
    }
    serde_json::from_str(&s.chamar(comando, &dados.to_string())).expect("a resposta é JSON")
}

fn ok(s: &mut Servico, comando: &str, dados: Value) -> Value {
    let r = chamar(s, comando, dados.clone());
    assert_eq!(r["ok"], true, "{comando} {dados}: {r}");
    r["dados"].clone()
}

fn entrar(s: &mut Servico, id: &str) {
    ok(s, "entrar", json!({ "id": id, "senha": "1234" }));
}

fn primeiro_usuario(b: &Banco) -> String {
    b.conn.query_row("SELECT id FROM usuario ORDER BY criado_em LIMIT 1", [], |l| l.get(0)).unwrap()
}

/// Uma aprovação montada à mão (assinada por `quem_assina`).
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
        nome: "Caixa à mão".into(),
        aprovador: quem_assina.publica(),
        gerente: "Ana".into(),
        aprovado_em: crate::tempo::agora(),
        dispositivo: dispositivo.into(),
        assinatura: [0u8; 64],
    };
    a.assinatura = quem_assina.assinar(Dominio::Aprovacao, &a.corpo());
    a
}

/// C2R2-1: o caixa retirado pela chave volta: identidade nova (pelo registro que já
/// mostra a retirada, ou pelo gerente que diz que ele foi retirado), convite novo
/// pela tela da loja, e o gerente readmite (C2R3-4: o bloqueio antigo não sai sozinho).
#[test]
fn c2r2_1_caixa_retirado_volta_com_identidade_nova_e_convite() {
    thread::spawn(|| {
        let (ca, cb, a, b, loja, _ia, _ib) = loja_com_dois_caixas_aprovados("c2r2-1");
        let ua = primeiro_usuario(&a);
        let disp_b = config::ler(&b.conn, "dispositivo_id").unwrap().unwrap();
        retirar_caixa_por_chave(&a.conn, &ca, &disp_b, "retirado por engano", &ua, "Ana").unwrap();

        // O caixa 2 não fica sabendo: sem a confirmação do gerente, a identidade que
        // abre não é trocada.
        let mut sb = Servico::abrir_para_teste(&cb, false).unwrap();
        entrar(&mut sb, &primeiro_usuario(&b));
        assert_eq!(ok(&mut sb, "sync_ler", json!({}))["p4"]["retirado"], false);
        let r = chamar(&mut sb, "p4_gerar_identidade_nova", json!({}));
        assert_eq!(r["ok"], false, "a identidade abre e ninguém disse que ele foi retirado: {r}");
        // Quando o registro da loja chega a ele, a tela já mostra "retirado".
        aprovados::juntar(&b.conn, &loja, &aprovados::linhas(&a.conn, &loja).unwrap()).unwrap();
        assert_eq!(ok(&mut sb, "sync_ler", json!({}))["p4"]["retirado"], true);
        let nova = ok(&mut sb, "p4_gerar_identidade_nova", json!({}));
        let codigo = nova["codigoPedido"].as_str().unwrap().to_string();

        // O gerente aprova de novo pelo convite; o bloqueio antigo sai; a troca volta.
        peca_nova(&a, "Peça depois da volta");
        let (mut ma, pt_a) = motor(&ca);
        let mut sa = Servico::abrir_para_teste(&ca, false).unwrap();
        entrar(&mut sa, &ua);
        let conv = ok(&mut sa, "p4_gerar_convite", json!({ "codigoPedido": codigo, "nome": "Caixa 2" }));
        let r = ok(
            &mut sb,
            "p4_aprovar_este_caixa",
            json!({ "convite": conv["convite"], "enderecos": format!("127.0.0.1:{pt_a}") }),
        );
        assert_eq!(r["p4"]["aprovado"], true, "{r}");
        // C2R3-4: a aprovação nova não tira o bloqueio antigo sozinha; a lista mostra o caso
        // e o gerente readmite.
        assert!(config::ler(&a.conn, &format!("{PREFIXO_RETIRADO}{disp_b}")).unwrap().is_some());
        let lista = ok(&mut sa, "caixas_conhecidos", json!({}));
        let c = lista["caixas"].as_array().unwrap().iter().find(|c| c["dispositivo"] == disp_b).cloned().unwrap();
        assert_eq!((c["situacao"].as_str(), c["temChave"].as_bool()), (Some("retirado"), Some(true)), "{c}");
        ok(&mut sa, "caixa_readmitir", json!({ "dispositivo": disp_b }));
        let (mut mb, pt_b) = motor(&cb);
        let puxou = puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b);
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        puxou.expect("o caixa que voltou troca dados");
        assert!(tem_peca(&b, "Peça depois da volta"));
    })
    .join()
    .unwrap();
}

/// C2R2-1: o gerente diz que o caixa foi retirado (o caixa não tinha como saber):
/// a identidade nova sai mesmo com a atual abrindo, e a antiga fica arquivada.
#[test]
fn c2r2_1_gerente_diz_que_o_caixa_foi_retirado() {
    let (_ca, cb, _a, b, _loja, _ia, ib) = loja_com_dois_caixas_aprovados("c2r2-1b");
    let mut sb = Servico::abrir_para_teste(&cb, false).unwrap();
    entrar(&mut sb, &primeiro_usuario(&b));
    let nova = ok(&mut sb, "p4_gerar_identidade_nova", json!({ "confirmarRetirado": true }));
    assert_ne!(nova["codigoPedido"].as_str().unwrap(), ib.identidade.codigo_de_pedido(), "outra identidade");
    let pasta = cofre::caminho_identidade(cb.parent().unwrap(), &cofre::Modo::Instalado).unwrap();
    let arquivadas = std::fs::read_dir(pasta.parent().unwrap())
        .unwrap()
        .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().contains(".antiga-"))
        .count();
    assert!(arquivadas >= 1, "a identidade de antes ficou arquivada");
}

/// C2R2-2: na raiz, a identidade nova pede a confirmação reforçada.
#[test]
fn c2r2_2_identidade_nova_na_raiz_pede_confirmacao() {
    let ca = temp("c2r2-2");
    let (a, ua) = loja_em_arquivo(&ca, "Caixa 1");
    p4::garantir_raiz(&a.conn, &ca).unwrap();
    drop(a);
    // A identidade da raiz deixa de abrir (como depois de perder o cofre).
    let pasta = ca.parent().unwrap();
    let arquivo = cofre::caminho_identidade(pasta, &cofre::Modo::Instalado).unwrap();
    cofre::esquecer_identidade_do_processo(pasta, &cofre::Modo::Instalado);
    std::fs::write(&arquivo, b"selo estragado").unwrap();
    let mut s = Servico::abrir_para_teste(&ca, false).unwrap();
    entrar(&mut s, &ua);
    let r = chamar(&mut s, "p4_gerar_identidade_nova", json!({}));
    assert_eq!(r["campo"], "confirmarRaiz", "{r}");
    assert_eq!(r["erro"], p4::TEXTO_IDENTIDADE_NOVA_NA_RAIZ);
    ok(&mut s, "p4_gerar_identidade_nova", json!({ "confirmarRaiz": true }));
}

/// C2R2-3: linhas bem assinadas de quem não está na cadeia da raiz (lixo) não entram
/// no registro de quem puxa; as da cadeia entram em qualquer ordem.
#[test]
fn c2r2_3_registro_nao_aceita_lixo_fora_da_cadeia() {
    thread::spawn(|| {
        let (ca, cb, a, b, loja, ia, _ib) = loja_com_dois_caixas_aprovados("c2r2-3");
        let contar =
            |b: &Banco| -> i64 { b.conn.query_row("SELECT COUNT(*) FROM caixa_aprovado", [], |l| l.get(0)).unwrap() };
        let antes = contar(&a);
        let mut lixo = Vec::new();
        for i in 0..100 {
            let x = Identidade::gerar().unwrap();
            let y = Identidade::gerar().unwrap();
            lixo.push(aprovacao_a_mao(&x, &y, &loja, &format!("disp-lixo-{i}")));
        }
        aprovados::juntar(&b.conn, &loja, &aprovados::Linhas { aprovacoes: lixo, retiradas: vec![] }).unwrap();
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        let r = puxar(&ca, &format!("127.0.0.1:{pt_b}"), pt_a);
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        r.unwrap();
        assert_eq!(contar(&a), antes, "o lixo do caixa 2 não entrou no registro do caixa 1");

        // A ordem não importa: o neto chega antes do filho, os dois entram.
        let cn = temp("c2r2-3-novo");
        let n = Banco::abrir(&cn).unwrap();
        let raiz = ia.identidade.publica();
        let linhas_a = aprovados::linhas(&a.conn, &loja).unwrap();
        let filho = Identidade::gerar().unwrap();
        let neto = Identidade::gerar().unwrap();
        let l_filho = aprovacao_a_mao(&ia.identidade, &filho, &loja, "disp-filho");
        let l_neto = aprovacao_a_mao(&filho, &neto, &loja, "disp-neto");
        let mut todas = linhas_a.aprovacoes.clone();
        todas.insert(0, l_neto);
        todas.push(l_filho);
        let j = aprovados::juntar_na_cadeia(
            &n.conn,
            &raiz,
            &loja,
            &aprovados::Linhas { aprovacoes: todas, retiradas: vec![] },
        )
        .unwrap();
        assert_eq!(j.recusadas, 0, "{j:?}");
        assert!(aprovados::autorizada(&n.conn, &raiz, &loja, &neto.publica()).unwrap());
    })
    .join()
    .unwrap();
}

/// C2R2-4: o motor manda de fato (no ponto do envio) só o anúncio assinado fora da
/// convivência, e os dois durante ela.
#[test]
fn c2r2_4_motor_manda_os_anuncios_certos() {
    thread::spawn(|| {
        let (ca, _cb, a, _b, _loja, _ia, _ib) = loja_com_dois_caixas_aprovados("c2r2-4");
        let chave = std::fs::canonicalize(&ca).unwrap_or(ca.clone());
        let mandados = |c: &PathBuf| -> Vec<&'static str> {
            let m = p4::anuncios_mandados().lock().unwrap_or_else(|e| e.into_inner());
            m.get(c).cloned().or_else(|| m.get(&ca).cloned()).unwrap_or_default()
        };
        let rodar = || {
            p4::anuncios_mandados().lock().unwrap_or_else(|e| e.into_inner()).clear();
            let (mut m, _pt) = motor(&ca);
            thread::sleep(Duration::from_secs(3));
            assert!(m.encerrar(Duration::from_secs(20)));
        };
        rodar();
        let fora = mandados(&chave);
        assert!(fora.contains(&"RSA4"), "o anúncio assinado sai: {fora:?}");
        assert!(!fora.contains(&"RST3"), "fora da convivência, nada tirado da senha sai: {fora:?}");
        definir_convivencia_p3(&a.conn, Some(7), "Ana").unwrap();
        rodar();
        let dentro = mandados(&chave);
        assert!(dentro.contains(&"RSA4") && dentro.contains(&"RST3"), "na convivência, os dois: {dentro:?}");
    })
    .join()
    .unwrap();
}

/// C2R2-5: o caixa com duas identidades aprovadas aparece com as duas na lista, e o
/// gerente retira a que não vale mais (o caixa continua com a outra).
#[test]
fn c2r2_5_retirar_uma_identidade_do_caixa_ambiguo() {
    thread::spawn(|| {
        let (ca, _cb, a, b, loja, ia, ib) = loja_com_dois_caixas_aprovados("c2r2-5");
        let ua = primeiro_usuario(&a);
        let disp_b = config::ler(&b.conn, "dispositivo_id").unwrap().unwrap();
        let outra = Identidade::gerar().unwrap();
        let linha = aprovacao_a_mao(&ia.identidade, &outra, &loja, &disp_b);
        aprovados::juntar(&a.conn, &loja, &aprovados::Linhas { aprovacoes: vec![linha], retiradas: vec![] }).unwrap();
        let raiz = ia.identidade.publica();
        assert!(aprovados::chave_do_dispositivo(&a.conn, &raiz, &loja, &disp_b).is_err());

        let mut sa = Servico::abrir_para_teste(&ca, false).unwrap();
        entrar(&mut sa, &ua);
        let lista = ok(&mut sa, "caixas_conhecidos", json!({}));
        let c = lista["caixas"].as_array().unwrap().iter().find(|c| c["dispositivo"] == disp_b).cloned().unwrap();
        assert_eq!(c["identidades"].as_array().map(|l| l.len()), Some(2), "{c}");
        ok(
            &mut sa,
            "caixa_retirar",
            json!({ "dispositivo": disp_b, "motivo": "aprovação em dobro", "chave": hex(&outra.publica()) }),
        );
        assert_eq!(
            aprovados::chave_do_dispositivo(&a.conn, &raiz, &loja, &disp_b).unwrap(),
            Some(ib.identidade.publica()),
            "sobrou a identidade de verdade"
        );
        assert!(config::ler(&a.conn, &format!("{PREFIXO_RETIRADO}{disp_b}")).unwrap().is_none(), "o caixa continua");
    })
    .join()
    .unwrap();
}

/// C2R2-6: com duas raízes no registro e sem pino, a situação do P4, o Início e a
/// lista de caixas continuam abrindo.
#[test]
fn c2r2_6_leituras_toleram_registro_com_duas_raizes() {
    let c = temp("c2r2-6");
    let b = Banco::abrir(&c).unwrap();
    let u = crate::faixas::para_testes::configurar(&b, "Caixa 1", 0);
    let loja = crate::loja::criar(&b.conn).unwrap();
    aprovados::registrar_raiz(&b.conn, &Identidade::gerar().unwrap(), &loja, "Caixa", &u).unwrap();
    // Uma segunda raiz (linha autoassinada) que chegou de fora.
    let outra = Identidade::gerar().unwrap();
    let linha = aprovacao_a_mao(&outra, &outra, &loja, "disp-outra-raiz");
    aprovados::juntar(&b.conn, &loja, &aprovados::Linhas { aprovacoes: vec![linha], retiradas: vec![] }).unwrap();
    config::gravar(&b.conn, "p4_chave_publica", &hex(&Identidade::gerar().unwrap().publica())).unwrap();
    assert!(cofre::raiz_atual(&b.conn, &loja).is_err(), "o registro está com duas raízes");
    let s = situacao_p4(&b.conn).expect("a situação do P4 abre");
    assert!(!s.aprovado);
    crate::avisos::do_dia(&b.conn).expect("o Início abre");
    caixas_conhecidos(&b.conn).expect("a lista de caixas abre");
}

/// C2R2-11: o gerente cancela o convite pela API; o convite cancelado não serve mais.
#[test]
fn c2r2_11_cancelar_convite_pela_api() {
    let ca = temp("c2r2-11");
    let (a, ua) = loja_em_arquivo(&ca, "Caixa 1");
    p4::garantir_raiz(&a.conn, &ca).unwrap();
    let mut sa = Servico::abrir_para_teste(&ca, false).unwrap();
    entrar(&mut sa, &ua);
    let codigo = Identidade::gerar().unwrap().codigo_de_pedido();
    let conv = ok(&mut sa, "p4_gerar_convite", json!({ "codigoPedido": codigo, "nome": "Caixa 3" }));
    let lista = ok(&mut sa, "p4_cancelar_convite", json!({ "id": conv["id"] }));
    assert_eq!(lista.as_array().unwrap().len(), 0, "o convite saiu da lista");
    let n: i64 =
        a.conn.query_row("SELECT COUNT(*) FROM p4_convite WHERE segredo_ativo IS NOT NULL", [], |l| l.get(0)).unwrap();
    assert_eq!(n, 0, "o segredo do convite cancelado saiu");
    let aud: i64 = a
        .conn
        .query_row("SELECT COUNT(*) FROM auditoria WHERE acao = 'p4_convite_cancelado'", [], |l| l.get(0))
        .unwrap();
    assert_eq!(aud, 1);
}

/// C2R2-12: um erro passageiro do banco não deixa o P4 parado (a espera de 30 s é só
/// para a falha da identidade).
#[test]
fn c2r2_12_erro_do_banco_nao_para_o_p4() {
    let (ca, _cb, a, _b, _loja, _ia, _ib) = loja_com_dois_caixas_aprovados("c2r2-12");
    assert!(p4::contexto(&a.conn, &ca).is_some());
    a.conn.execute_batch("ALTER TABLE caixa_aprovado RENAME TO caixa_aprovado_fora").unwrap();
    assert!(p4::contexto(&a.conn, &ca).is_none(), "com o registro fora do lugar, sem contexto");
    a.conn.execute_batch("ALTER TABLE caixa_aprovado_fora RENAME TO caixa_aprovado").unwrap();
    assert!(p4::contexto(&a.conn, &ca).is_some(), "com o banco de volta, o P4 volta na hora");
    let _ = identidade(&ca);
}

// ---------------------------------------------------------------------------
// R2 §6.3 (DT-P4-19, mínimo para a T015): a troca de negócio pelo fio do P4.
// ---------------------------------------------------------------------------

/// Volta em vários lotes pelo P4 (lote de 5, 20 peças): tudo chega, cada lote por
/// uma conexão autenticada.
#[test]
fn p4_negocio_volta_em_varios_lotes() {
    thread::spawn(|| {
        let (ca, cb, a, b, _loja, _ia, _ib) = loja_com_dois_caixas_aprovados("p4-lotes");
        for i in 0..20 {
            peca_nova(&a, &format!("Peça em lote {i:02}"));
        }
        config::gravar(&a.conn, "sync_lote_teste", "5").unwrap();
        let (mut ma, pt_a) = motor(&ca);
        let antes = p4::ATENDIDAS_PELO_P4.load(Ordering::SeqCst);
        let r = puxar(&cb, &format!("127.0.0.1:{pt_a}"), 0);
        let conexoes = p4::ATENDIDAS_PELO_P4.load(Ordering::SeqCst) - antes;
        assert!(ma.encerrar(Duration::from_secs(20)));
        let (_, aplicadas) = r.unwrap();
        assert!(aplicadas >= 20, "aplicou {aplicadas}");
        for i in 0..20 {
            assert!(tem_peca(&b, &format!("Peça em lote {i:02}")), "faltou a peça {i}");
        }
        assert!(conexoes >= 4, "os lotes foram pelo fio do P4 ({conexoes} conexões)");
    })
    .join()
    .unwrap();
}

/// Recomeço depois de restaurar um backup (o resumo do servidor), pelo P4: o caixa
/// volta ao backup, perde o que recebeu depois, e recebe de novo pelo fio do P4.
#[test]
fn p4_negocio_recomeco_depois_de_restaurar_backup() {
    thread::spawn(|| {
        let _vez = crate::backup::testes::um_por_vez();
        let (ca, cb, a, b, _loja, _ia, _ib) = loja_com_dois_caixas_aprovados("p4-recomeco");
        drop(b);
        let feito = crate::backup::fazer(&cb, false).unwrap();
        peca_nova(&a, "Peça depois do backup do caixa 2");
        let (mut ma, pt_a) = motor(&ca);
        puxar(&cb, &format!("127.0.0.1:{pt_a}"), 0).unwrap();
        let mut b = Banco::abrir(&cb).unwrap();
        assert!(tem_peca(&b, "Peça depois do backup do caixa 2"));
        crate::backup::restaurar(&mut b.conn, Some(&cb), &feito.arquivo, false, "1234").unwrap();
        assert!(!tem_peca(&b, "Peça depois do backup do caixa 2"), "o backup é de antes da peça");
        assert!(p4::tem_raiz_fixada(&b.conn), "o caixa continua do P4 depois de restaurar");
        let antes = p4::ATENDIDAS_PELO_P4.load(Ordering::SeqCst);
        let r = puxar(&cb, &format!("127.0.0.1:{pt_a}"), 0);
        let pelo_p4 = p4::ATENDIDAS_PELO_P4.load(Ordering::SeqCst) - antes;
        assert!(ma.encerrar(Duration::from_secs(20)));
        r.unwrap();
        assert!(tem_peca(&b, "Peça depois do backup do caixa 2"), "a peça voltou pelo recomeço");
        assert!(pelo_p4 >= 1, "pelo fio do P4");
    })
    .join()
    .unwrap();
}

/// Sonda de endereço desconhecido pelo P4: o caixa da loja muda de porta (endereço
/// que o outro nunca viu); a puxada tenta o P4 com as chaves autorizadas e acha o
/// caixa certo — sem cair para o P3.
#[test]
fn p4_negocio_endereco_desconhecido() {
    thread::spawn(|| {
        let (ca, cb, a, b, _loja, _ia, _ib) = loja_com_dois_caixas_aprovados("p4-sonda");
        let (mut ma, pt_a) = motor(&ca);
        puxar(&cb, &format!("127.0.0.1:{pt_a}"), 0).unwrap();
        assert!(ma.encerrar(Duration::from_secs(20)));
        peca_nova(&a, "Peça no endereço novo");
        let (mut ma, pt_novo) = motor(&ca);
        assert_ne!(pt_novo, pt_a, "outra porta");
        let antes = p4::ATENDIDAS_PELO_P4.load(Ordering::SeqCst);
        let r = puxar(&cb, &format!("127.0.0.1:{pt_novo}"), 0);
        let pelo_p4 = p4::ATENDIDAS_PELO_P4.load(Ordering::SeqCst) - antes;
        assert!(ma.encerrar(Duration::from_secs(20)));
        r.unwrap();
        assert!(tem_peca(&b, "Peça no endereço novo"));
        assert!(pelo_p4 >= 1, "pelo fio do P4");
    })
    .join()
    .unwrap();
}
