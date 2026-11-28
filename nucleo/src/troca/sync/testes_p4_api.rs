//! T012 (0.9.19-dev.2, `roadmap.md` §5.7): os comandos da API do protocolo 4, do
//! jeito que as telas os chamam (`Servico::chamar`), com bancos em arquivo e o
//! caixa da loja servindo pelo motor (portas sorteadas, loopback).
//!
//! Ficam dentro de `troca::sync` para rodar junto com os testes de rede (em série).

use super::testes_seguranca_caixas::{caixa_vazio_em_arquivo, loja_em_arquivo, motor, temp};
use super::*;
use crate::banco::Banco;
use crate::troca::protocolo4::{aprovados, cofre};
use crate::Servico;

/// Chama um comando como a tela chama (com a chave da operação quando o comando exige).
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

/// Entra com o usuário `id` (senha de teste dos apoios: "1234").
fn entrar(s: &mut Servico, id: &str) {
    ok(s, "entrar", json!({ "id": id, "senha": "1234" }));
}

/// O caixa 1 da loja (gerente "Ana", raiz do protocolo 4), com o motor servindo.
fn loja_raiz(nome: &str) -> (PathBuf, Banco, String, Motor, u16) {
    let ca = temp(nome);
    let (a, ua) = loja_em_arquivo(&ca, "Caixa 1");
    p4::garantir_raiz(&a.conn, &ca).unwrap();
    let (m, pt) = motor(&ca);
    (ca, a, ua, m, pt)
}

/// O caixa novo, sem usuários e com a troca desligada (como sai da instalação).
fn caixa_novo(caminho: &Path) -> Servico {
    let b = Banco::abrir(caminho).unwrap();
    config::gravar(&b.conn, "codigos_espera_teste", "0").unwrap();
    drop(b);
    Servico::abrir_para_teste(caminho, false).unwrap()
}

/// T012 (DT-P4-31/32/33) de ponta a ponta pela API: o caixa novo mostra o código; o
/// gerente gera o convite no caixa da loja; o caixa novo entra com o convite digitado
/// (sem a senha da rede), fica aprovado, com o nome que o gerente deu, e marcado como
/// "entrando na loja" (a tela de entrada espera os dados como antes).
#[test]
fn p4_api_caixa_novo_entra_pelo_convite_digitado() {
    thread::spawn(|| {
        let (ca, _a, ua, mut ma, pt_a) = loja_raiz("api-conv-a");
        let cb = temp("api-conv-b");
        let mut sb = caixa_novo(&cb);
        let id_b = ok(&mut sb, "p4_identidade", json!({}));
        let codigo = id_b["codigoPedido"].as_str().unwrap().to_string();
        assert_eq!(codigo.len(), 14, "12 caracteres em 3 grupos: {codigo}");
        assert_eq!(id_b["aprovado"], false);
        assert_eq!(id_b["cofreSistema"], false, "nos testes, a reserva local (DT-P4-09)");

        let mut sa = Servico::abrir_para_teste(&ca, false).unwrap();
        entrar(&mut sa, &ua);
        let conv = ok(&mut sa, "p4_gerar_convite", json!({ "codigoPedido": codigo, "nome": "Caixa Novo" }));
        let texto = conv["convite"].as_str().unwrap().to_string();
        assert!(conv["qr"].as_str().unwrap().starts_with("data:image/svg+xml;base64,"), "o QR vai como SVG");
        assert!(!conv["expiraEmTexto"].as_str().unwrap().is_empty());
        let lista = ok(&mut sa, "p4_convites", json!({}));
        assert_eq!(lista.as_array().unwrap().len(), 1, "o convite aparece na lista do gerente");
        assert!(!lista.to_string().contains(&texto), "a lista não mostra o segredo do convite");
        assert_eq!(lista[0]["gerente"], "Ana", "o nome de quem gerou (o identificador fica no banco)");

        // R1 M4 (T002 da 009): um nome digitado errado ao lado do endereço certo não impede a entrada.
        let _t = super::cliente::resolvedor_de_teste::UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        let _r = super::cliente::resolvedor_de_teste::Resolvedor::usar(|_| None);
        let r = ok(
            &mut sb,
            "p4_juntar_com_convite",
            json!({ "convite": texto, "enderecos": format!("nome-errado.loja.local, 127.0.0.1:{pt_a}") }),
        );
        drop(_r);
        assert_eq!(r["p4"]["aprovado"], true, "o caixa novo ficou aprovado: {r}");
        assert_eq!(r["p4"]["temRaiz"], true);
        assert_eq!(r["ligado"], true, "a troca ligou sem a senha da rede");
        assert_eq!(r["temSenha"], false);
        let estado = ok(&mut sb, "estado", json!({}));
        assert_eq!(estado["juntando"], true, "a tela de entrada espera os dados da loja");
        assert_eq!(estado["caixa"], "Caixa Novo", "o nome que o gerente deu no convite");
        assert_eq!(ok(&mut sa, "p4_convites", json!({})).as_array().unwrap().len(), 0, "o convite foi usado");
        // R2 B2: com o convite já usado, um nome errado ao lado do endereço certo: o erro mostrado é o da tentativa de
        // verdade (o convite), e não o do nome.
        {
            let cc = temp("api-conv-c");
            let mut sc = caixa_novo(&cc);
            let _r = super::cliente::resolvedor_de_teste::Resolvedor::usar(|_| None);
            let r = chamar(
                &mut sc,
                "p4_juntar_com_convite",
                json!({ "convite": texto, "enderecos": format!("nome-errado.loja.local, 127.0.0.1:{pt_a}") }),
            );
            assert_eq!(r["ok"], false, "{r}");
            assert!(
                !r["erro"].as_str().unwrap_or("").contains("não foi encontrado"),
                "o erro real, e não o do nome: {r}"
            );
        }

        // A auditoria do caixa 1 mostra quem gerou (o identificador do gerente).
        let aud = ok(&mut sa, "auditoria_listar", json!({}));
        let gerado = aud.as_array().unwrap().iter().find(|x| x["acao"] == "p4_convite_gerado").expect("auditado");
        assert_eq!(gerado["usuario"], "Ana", "a auditoria do convite tem o nome de quem gerou: {gerado}");
        assert!(ma.encerrar(Duration::from_secs(20)));
    })
    .join()
    .unwrap();
}

/// DT-P4-32: o convite exige o gerente (a classe do comando), a troca ligada e este
/// caixa aprovado; e o caixa que já tem usuários não "entra" como caixa novo.
#[test]
fn p4_api_convite_e_entrada_recusados_fora_das_regras() {
    thread::spawn(|| {
        let (ca, a, ua, mut ma, _pt) = loja_raiz("api-regras-a");
        let caixa = crate::usuarios::criar(&a.conn, &a.dispositivo, "Bia", "1234", true).unwrap().id;
        let mut sa = Servico::abrir_para_teste(&ca, false).unwrap();
        entrar(&mut sa, &caixa);
        let r = chamar(&mut sa, "p4_gerar_convite", json!({ "codigoPedido": "ABCD-EFGH-JKLM", "nome": "X" }));
        assert_eq!(r["campo"], "gerente", "quem não é gerente não gera convite: {r}");
        let r = chamar(&mut sa, "p4_convites", json!({}));
        assert_eq!(r["ok"], false, "nem vê a lista: {r}");

        // Troca desligada: o convite não teria quem atendesse.
        entrar(&mut sa, &ua);
        salvar_ajustes(&a.conn, false, "", "").unwrap();
        let r = chamar(&mut sa, "p4_gerar_convite", json!({ "codigoPedido": "ABCD-EFGH-JKLM", "nome": "X" }));
        assert_eq!(r["erro"], p4::TEXTO_CONVITE_SEM_TROCA, "{r}");

        // O caixa da loja (com usuários) não entra como caixa novo.
        let r = chamar(&mut sa, "p4_juntar_com_convite", json!({ "convite": "AAAAA-BBBBB-CCCCC-DDDDD-EEEEE-F" }));
        assert_eq!(r["ok"], false, "{r}");
        // Endereço digitado errado: o campo certo.
        let cn = temp("api-regras-novo");
        let mut sn = caixa_novo(&cn);
        // R1 B10: sem depender do DNS de verdade (um DNS que responde a tudo faria o nome "existir").
        let _t = super::cliente::resolvedor_de_teste::UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        let _r = super::cliente::resolvedor_de_teste::Resolvedor::usar(|_| None);
        let inicio = std::time::Instant::now();
        let r = chamar(
            &mut sn,
            "p4_juntar_com_convite",
            json!({ "convite": "AAAAA-BBBBB-CCCCC-DDDDD-EEEEE-F", "enderecos": "caixa-da-frente" }),
        );
        assert_eq!(r["campo"], "enderecos", "{r}");
        // R2 B1: só o nome errado: o erro sai na hora, sem os 12 s da procura pelos anúncios.
        assert!(inicio.elapsed() < Duration::from_secs(6), "sem a procura de 12 s: {:?}", inicio.elapsed());
        // E6-1 (T002 da 009): um nome que não está na rede também vai ao campo, com o motivo.
        assert!(r["erro"].as_str().unwrap_or("").contains("caixa-da-frente"), "{r}");
        // R1 M5: o texto novo (o de antes dizia só "Endereço inválido").
        assert!(r["erro"].as_str().unwrap_or("").contains("não foi encontrado na rede"), "{r}");
        drop(_r);
        // E o endereço com a forma errada (o sublinhado não vale em nome), também.
        let r = chamar(
            &mut sn,
            "p4_juntar_com_convite",
            json!({ "convite": "AAAAA-BBBBB-CCCCC-DDDDD-EEEEE-F", "enderecos": "caixa_da_frente" }),
        );
        assert_eq!(r["campo"], "enderecos", "{r}");
        assert!(ma.encerrar(Duration::from_secs(20)));
    })
    .join()
    .unwrap();
}

/// DT-P4-30 (N-16): a leitura da troca de dados e a lista de caixas (linha da tela)
/// não abrem a identidade: num caixa novo, nada é criado até o comando demorado.
#[test]
fn p4_api_leituras_da_tela_nao_abrem_a_identidade() {
    let cb = temp("api-leitura");
    let mut sb = caixa_novo(&cb);
    let pasta = cb.parent().unwrap();
    let arquivo = cofre::caminho_identidade(pasta, &cofre::Modo::Instalado).unwrap();
    let r = ok(&mut sb, "sync_ler", json!({}));
    assert_eq!(r["p4"]["aprovado"], false);
    assert_eq!(r["p4"]["convivenciaMaximaDias"], CONVIVENCIA_MAXIMA_DIAS);
    assert!(!arquivo.exists(), "a leitura da tela não criou a identidade");
    ok(&mut sb, "p4_identidade", json!({}));
    assert!(arquivo.exists(), "o comando demorado cria a identidade do caixa novo");
}

/// T012 (DT-P4-33): o caixa que já é da loja (trocava dados antes de o caixa 1 virar a
/// raiz) recebe a aprovação pelo convite, pela API, com a senha do gerente; o
/// convite de outra loja é recusado antes de qualquer conexão.
#[test]
fn p4_api_caixa_da_loja_recebe_a_aprovacao_pelo_convite() {
    thread::spawn(|| {
        let (ca, cb) = (temp("api-aprovar-a"), temp("api-aprovar-b"));
        let (a, ua) = loja_em_arquivo(&ca, "Caixa 1");
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
        assert_eq!(crate::loja::id(&b.conn).unwrap(), loja);
        drop(b);
        let (mut ma, pt_a) = motor(&ca);

        let mut sb = Servico::abrir_para_teste(&cb, false).unwrap();
        entrar(&mut sb, &ua);
        let id_b = ok(&mut sb, "p4_identidade", json!({}));
        assert_eq!(id_b["aprovado"], false);
        // DT-P4-41: sem ouvir caixas atualizados, nada no Início; ouvindo o anúncio do
        // caixa 1 (atualizado), o Início pede a aprovação.
        let tem_aviso = |s: &mut Servico| {
            ok(s, "avisos_do_dia", json!({})).as_array().unwrap().iter().any(|a| a["id"] == "p4.aprovar_este_caixa")
        };
        assert!(!tem_aviso(&mut sb), "sem caixa atualizado na rede, sem aviso");
        let pacote = p4::pacote_de_anuncio(&p4::contexto(&a.conn, &ca).unwrap(), pt_a).unwrap();
        p4::guardar_ouvido(&cb, "127.0.0.1".parse().unwrap(), &pacote);
        assert!(tem_aviso(&mut sb), "o caixa da loja sem aprovação, com caixas atualizados na rede, vê o aviso");
        // Convite de outra loja (o QR diz a loja): recusado na hora.
        let outro = format!("RUST4|AAAAA-BBBBB-CCCCC-DDDDD-EEEEE-F|outra-loja|{}|127.0.0.1:{pt_a}", "00".repeat(32));
        let r = chamar(&mut sb, "p4_aprovar_este_caixa", json!({ "convite": outro }));
        assert_eq!(r["erro"], p4::TEXTO_CONVITE_DE_OUTRA_LOJA, "{r}");

        let mut sa = Servico::abrir_para_teste(&ca, false).unwrap();
        entrar(&mut sa, &ua);
        let conv = ok(&mut sa, "p4_gerar_convite", json!({ "codigoPedido": id_b["codigoPedido"], "nome": "Caixa 2" }));
        let r = ok(
            &mut sb,
            "p4_aprovar_este_caixa",
            json!({ "convite": conv["convite"], "enderecos": format!("127.0.0.1:{pt_a}") }),
        );
        assert_eq!(r["p4"]["aprovado"], true, "{r}");
        assert_eq!(ok(&mut sb, "estado", json!({}))["juntando"], false, "nada muda nos dados do caixa");
        assert!(!tem_aviso(&mut sb), "aprovado: o aviso sai");
        let r = chamar(&mut sb, "p4_aprovar_este_caixa", json!({ "convite": conv["convite"] }));
        assert_eq!(r["erro"], p4::TEXTO_JA_APROVADO, "{r}");
        assert!(ma.encerrar(Duration::from_secs(20)));
    })
    .join()
    .unwrap();
}

/// A loja com o caixa 2 aprovado pelo convite (pela API) e o caixa 1 servindo.
fn loja_com_caixa_aprovado(nome: &str) -> (PathBuf, Banco, String, String, Motor) {
    let (ca, a, ua, ma, pt_a) = loja_raiz(&format!("{nome}-a"));
    let cb = temp(&format!("{nome}-b"));
    let mut sb = caixa_novo(&cb);
    let codigo = ok(&mut sb, "p4_identidade", json!({}))["codigoPedido"].clone();
    let mut sa = Servico::abrir_para_teste(&ca, false).unwrap();
    entrar(&mut sa, &ua);
    let conv = ok(&mut sa, "p4_gerar_convite", json!({ "codigoPedido": codigo, "nome": "Caixa 2" }));
    ok(
        &mut sb,
        "p4_juntar_com_convite",
        json!({ "convite": conv["convite"], "enderecos": format!("127.0.0.1:{pt_a}") }),
    );
    let disp_b = config::ler(&Banco::abrir(&cb).unwrap().conn, "dispositivo_id").unwrap().unwrap();
    // O caixa 2 aparece para o caixa 1 (como depois da primeira troca).
    a.conn.execute("INSERT OR IGNORE INTO sync_par (dispositivo, nome) VALUES (?1, 'Caixa 2')", [&disp_b]).unwrap();
    (ca, a, ua, disp_b, ma)
}

/// DT-P4-35: a lista de caixas diz quem tem chave, quem é a raiz e quem cairia junto;
/// a retirada pela API é assinada; o retirado pela chave não volta pelo "Readmitir".
#[test]
fn p4_api_retirada_pela_chave_e_readmitir() {
    thread::spawn(|| {
        let (ca, a, ua, disp_b, mut ma) = loja_com_caixa_aprovado("api-retirar");
        let mut sa = Servico::abrir_para_teste(&ca, false).unwrap();
        entrar(&mut sa, &ua);
        let lista = ok(&mut sa, "caixas_conhecidos", json!({}));
        let c2 = lista["caixas"].as_array().unwrap().iter().find(|c| c["dispositivo"] == disp_b).cloned().unwrap();
        assert_eq!(c2["temChave"], true, "{c2}");
        assert_eq!(c2["raiz"], false);
        assert_eq!(c2["dependentes"], json!([]));

        ok(&mut sa, "caixa_retirar", json!({ "dispositivo": disp_b, "motivo": "saiu da loja" }));
        let loja = crate::loja::id(&a.conn).unwrap();
        let raiz = cofre::raiz_atual(&a.conn, &loja).unwrap().unwrap();
        let situacoes = aprovados::situacoes_por_dispositivo(&a.conn, &raiz, &loja).unwrap();
        assert!(
            situacoes[&disp_b].iter().all(|(_, s)| *s == aprovados::Situacao::Retirada),
            "a retirada foi assinada pela chave"
        );
        let lista = ok(&mut sa, "caixas_conhecidos", json!({}));
        let c2 = lista["caixas"].as_array().unwrap().iter().find(|c| c["dispositivo"] == disp_b).cloned().unwrap();
        assert_eq!(c2["retiradoPelaChave"], true);
        assert_eq!(c2["situacao"], "retirado");

        let r = chamar(&mut sa, "caixa_readmitir", json!({ "dispositivo": disp_b }));
        assert_eq!(r["erro"], p4::TEXTO_RETIRADO_PELA_CHAVE, "{r}");
        assert!(
            config::ler(&a.conn, &format!("{PREFIXO_RETIRADO}{disp_b}")).unwrap().is_some(),
            "o bloqueio de antes continua"
        );
        assert!(ma.encerrar(Duration::from_secs(20)));
    })
    .join()
    .unwrap();
}

/// DT-P4-35 (falha fechada): se o caixa a retirar tem chave e este caixa não consegue
/// assinar agora (a identidade deste processo não é a aprovada), a retirada é recusada
/// inteira: nunca fica só o bloqueio de antes, que o protocolo 4 não respeita.
#[test]
fn p4_retirada_pela_chave_nao_cai_para_a_antiga_sem_identidade() {
    thread::spawn(|| {
        let (_ca, a, _ua, disp_b, mut ma) = loja_com_caixa_aprovado("api-retirar-sem-id");
        // Outra pasta: a identidade aberta para ela não é a do caixa 1 (como depois de
        // perder o cofre e gerar outra).
        let outra = temp("api-retirar-sem-id-outra");
        let r = retirar_caixa_por_chave(&a.conn, &outra, &disp_b, "saiu da loja", "id-da-ana", "Ana");
        assert_eq!(r.unwrap_err().to_string(), p4::TEXTO_RETIRADA_SEM_IDENTIDADE);
        assert!(
            config::ler(&a.conn, &format!("{PREFIXO_RETIRADO}{disp_b}")).unwrap().is_none(),
            "nem o bloqueio de antes ficou"
        );
        let loja = crate::loja::id(&a.conn).unwrap();
        let raiz = cofre::raiz_atual(&a.conn, &loja).unwrap().unwrap();
        assert!(aprovados::chave_do_dispositivo(&a.conn, &raiz, &loja, &disp_b).unwrap().is_some());
        assert!(ma.encerrar(Duration::from_secs(20)));
    })
    .join()
    .unwrap();
}

/// DT-P4-36: o caixa do protocolo 4 (raiz fixada, entrou pelo convite sem a senha
/// da rede) desliga e liga a troca de novo sem senha; uma senha digitada segue a
/// política; o caixa que ainda não é do P4 continua precisando dela.
#[test]
fn p4_caixa_do_p4_liga_a_troca_sem_a_senha_da_rede() {
    let cb = temp("sem-senha-p4");
    let b = Banco::abrir(&cb).unwrap();
    config::gravar(&b.conn, "p4_raiz", &"ab".repeat(32)).unwrap();
    salvar_ajustes(&b.conn, false, "", "").unwrap();
    salvar_ajustes(&b.conn, true, "", "").expect("ligar sem senha no caixa do P4");
    assert!(ajustes(&b.conn).unwrap().ligado);
    assert!(salvar_ajustes(&b.conn, true, "12", "").is_err(), "senha digitada curta segue recusada");
    let cc = temp("sem-senha-p3");
    let c = Banco::abrir(&cc).unwrap();
    assert!(salvar_ajustes(&c.conn, true, "", "").is_err(), "sem a raiz, a senha continua obrigatória");
}

/// DT-P4-34: a convivência pela API (só gerente), de 1 a 30 dias, e desligar.
#[test]
fn p4_api_convivencia_com_os_caixas_antigos() {
    thread::spawn(|| {
        let (ca, _a, ua, mut ma, _pt) = loja_raiz("api-convivencia");
        let mut sa = Servico::abrir_para_teste(&ca, false).unwrap();
        entrar(&mut sa, &ua);
        let r = chamar(&mut sa, "p4_convivencia_p3", json!({ "dias": 31 }));
        assert_eq!(r["campo"], "dias", "{r}");
        let r = ok(&mut sa, "p4_convivencia_p3", json!({ "dias": 7 }));
        assert!(r["p4"]["convivenciaAte"].is_string(), "{r}");
        let r = ok(&mut sa, "p4_convivencia_p3", json!({ "dias": null }));
        assert!(r["p4"]["convivenciaAte"].is_null(), "{r}");
        assert!(ma.encerrar(Duration::from_secs(20)));
    })
    .join()
    .unwrap();
}

/// T012: sem endereço, o caixa novo acha a loja pelo anúncio assinado (`RSA4`) que
/// chega na porta de anúncios (aqui uma porta sorteada, no loopback); o anúncio de
/// outra loja não serve.
#[test]
fn p4_procura_a_loja_pelos_anuncios() {
    let ca = temp("api-procura");
    let (a, _ua) = loja_em_arquivo(&ca, "Caixa 1");
    p4::garantir_raiz(&a.conn, &ca).unwrap();
    let ctx = p4::contexto(&a.conn, &ca).expect("o caixa 1 fala o P4");
    let pacote = p4::pacote_de_anuncio(&ctx, 4242).unwrap();
    let porta = std::net::UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let emissor = |pacote: Vec<u8>| {
        thread::spawn(move || {
            let s = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            for _ in 0..15 {
                let _ = s.send_to(&pacote, ("127.0.0.1", porta));
                thread::sleep(Duration::from_millis(100));
            }
        })
    };
    let e = emissor(pacote.clone());
    let achados = p4::procurar_pelos_anuncios_em(&ctx.loja, Duration::from_secs(5), ("127.0.0.1", porta));
    e.join().unwrap();
    assert_eq!(achados, vec!["127.0.0.1:4242".parse::<SocketAddr>().unwrap()]);
    let e = emissor(pacote);
    let nada = p4::procurar_pelos_anuncios_em("outra-loja", Duration::from_millis(1200), ("127.0.0.1", porta));
    e.join().unwrap();
    assert!(nada.is_empty(), "o anúncio de outra loja não serve: {nada:?}");
}

/// DT-P4-42 (N-04) pela API: a identidade nova só sai quando a deste computador não
/// abre, com o gerente; a tela fica sabendo pelo campo "identidade"; a auditoria
/// guarda quem fez; o código passa a ser o da identidade nova.
#[test]
fn p4_api_identidade_nova_so_quando_a_atual_nao_abre() {
    // A identidade que sobrou num arquivo que não abre (o segredo dela não está aqui).
    let cb = temp("api-identidade-nova");
    let (b, ub) = loja_em_arquivo(&cb, "Caixa 1");
    drop(b);
    let arquivo = cofre::caminho_identidade(cb.parent().unwrap(), &cofre::Modo::Instalado).unwrap();
    std::fs::create_dir_all(arquivo.parent().unwrap()).unwrap();
    std::fs::write(&arquivo, b"identidade de antes, sem o segredo").unwrap();
    let mut s = Servico::abrir_para_teste(&cb, false).unwrap();
    let r = chamar(&mut s, "p4_identidade", json!({}));
    assert_eq!(r["campo"], "identidade", "a tela pode oferecer a identidade nova: {r}");

    entrar(&mut s, &ub);
    let nova = ok(&mut s, "p4_gerar_identidade_nova", json!({}));
    let codigo = nova["codigoPedido"].as_str().unwrap().to_string();
    assert_eq!(ok(&mut s, "p4_identidade", json!({}))["codigoPedido"], codigo.as_str(), "o código é o da nova");
    let arquivadas = std::fs::read_dir(arquivo.parent().unwrap())
        .unwrap()
        .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().contains(".antiga-"))
        .count();
    assert!(arquivadas >= 1, "a identidade de antes ficou arquivada, não apagada");
    let r = chamar(&mut s, "p4_gerar_identidade_nova", json!({}));
    assert_eq!(r["ok"], false, "com a identidade abrindo, não gera outra: {r}");
    let aud = ok(&mut s, "auditoria_listar", json!({}));
    let feito = aud.as_array().unwrap().iter().find(|x| x["acao"] == "p4_identidade_nova").expect("auditado");
    assert_eq!(feito["usuario"], "Ana", "{feito}");
    assert!(feito["detalhes"].to_string().contains(&codigo));
}
