//! Frente de segurança — R-01 (senha da rede) e D1 (entrada de caixa).
//!
//! **Comportamento seguro exigido** (`seguro_*`): o que o protocolo 4 garante.
//! Escritos antes (em vermelho, com `#[ignore]`) e ligados um a um até a T011 da
//! 0.9.19-dev.2, quando o último saiu do `#[ignore]`: agora são todos exigidos.
//!
//! No fim, testes das peças do protocolo 4 já prontas (etapas 1 a 3) contra a
//! troca de hoje: passam já, sem mudar o fio (`p4_*`).
//!
//! As demonstrações do ataque (`demo_*`) ficam em `demos_seguranca_caixas.rs`,
//! fora do patch público (decisão A10 pendente: elas ensinam o ataque).
//!
//! Todos os testes de rede usam portas sorteadas pelo sistema (loopback) e
//! devem rodar em série (`--test-threads=1`), como o resto da troca de dados.

use super::*;
use crate::banco::Banco;

pub(super) const SENHA_FRACA: &str = "2718"; // 4 algarismos: dentro da política atual (mínimo 4).

// ---------------------------------------------------------------------------
// Apoio
// ---------------------------------------------------------------------------

pub(super) fn temp(nome: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rustinho-seg-{nome}-{}", crate::tempo::novo_id()));
    std::fs::create_dir_all(&p).unwrap();
    p.join("rustinho.db")
}

/// Um motor com portas escolhidas pelo sistema (C8) e a porta TCP dele.
pub(super) fn motor(caminho: &Path) -> (Motor, u16) {
    let m = Motor::iniciar(caminho.to_path_buf(), 0, 0);
    let (tcp, _) = m.portas(Duration::from_secs(10)).expect("as portas do motor abriram");
    (m, tcp)
}

/// Loja num arquivo (gerente "Ana", este caixa distribui as faixas), com a
/// troca ligada e a senha fraca. Reabre o banco (como ao reabrir o programa).
pub(super) fn loja_em_arquivo(caminho: &Path, caixa: &str) -> (Banco, String) {
    let b = Banco::abrir(caminho).unwrap();
    let u = crate::faixas::para_testes::configurar(&b, caixa, 0);
    config::gravar(&b.conn, "loja_nome", "Auto Peças Teste").unwrap();
    salvar_ajustes(&b.conn, true, SENHA_FRACA, "").unwrap();
    drop(b);
    (Banco::abrir(caminho).unwrap(), u)
}

/// Caixa vazio (sem usuários) que liga a troca com a senha fraca.
pub(super) fn caixa_vazio_em_arquivo(caminho: &Path, caixa: &str) -> Banco {
    let b = Banco::abrir(caminho).unwrap();
    config::gravar(&b.conn, "caixa_nome", caixa).unwrap();
    config::gravar(&b.conn, "codigos_espera_teste", "0").unwrap();
    salvar_ajustes(&b.conn, true, SENHA_FRACA, "").unwrap();
    b
}

/// Cadastra um cliente (dado pessoal, LGPD) na loja.
pub(super) fn cliente(b: &Banco, usuario: &str, nome: &str) -> String {
    let mut f = crate::pessoas::ficha_nova(&b.conn, "cliente").unwrap();
    f.nome = nome.into();
    f.cliente = true;
    crate::pessoas::salvar(&b.conn, &b.dispositivo, usuario, &f).unwrap()
}

pub(super) fn tem_usuarios(b: &Banco) -> i64 {
    b.conn.query_row("SELECT COUNT(*) FROM usuario", [], |l| l.get(0)).unwrap()
}

pub(super) fn tem_cliente(b: &Banco, nome: &str) -> bool {
    b.conn.query_row("SELECT COUNT(*) FROM pessoa WHERE nome = ?1", [nome], |l| l.get::<_, i64>(0)).unwrap() > 0
}

// ===========================================================================
// 2. Comportamento seguro exigido (protocolo 4) — fora do `#[ignore]` desde a
//    T011 (0.9.19-dev.2). Os que montavam à mão o que o protocolo 3 fazia (o
//    pedido em JSON, o anúncio cifrado) miram agora a função real do protocolo 4,
//    com a mesma propriedade (como o plano 5.5 previa).
// ===========================================================================

/// A loja do caixa 1 já com a troca ligada há um tempo: ele é a raiz do protocolo
/// 4 (o motor faz isso na puxada; aqui, na hora).
fn loja_raiz(caminho: &Path, caixa: &str) -> (Banco, String) {
    let (b, u) = loja_em_arquivo(caminho, caixa);
    super::p4::garantir_raiz(&b.conn, caminho).unwrap();
    assert!(super::p4::tem_raiz_fixada(&b.conn));
    (b, u)
}

/// O caixa novo (banco vazio), sem a senha da rede, entra pelo convite que o gerente
/// gerou no caixa 1 com o código dele. Devolve o banco.
fn caixa_entra_pelo_convite(ca: &Path, cb: &Path, pt_a: u16, a: &Banco) -> Banco {
    use crate::troca::protocolo4::{aprovados, cofre};
    let b = Banco::abrir(cb).unwrap();
    config::gravar(&b.conn, "caixa_nome", "Caixa 2").unwrap();
    config::gravar(&b.conn, "codigos_espera_teste", "0").unwrap();
    let id_b = cofre::identidade_do_processo(cb.parent().unwrap(), &cofre::Modo::Instalado).unwrap();
    let disp_b = config::ler(&b.conn, "dispositivo_id").unwrap().unwrap();
    let codigo = aprovados::Pedido::com_dispositivo(&id_b.identidade, &disp_b).codigo();
    let id_a = cofre::identidade_do_processo(ca.parent().unwrap(), &cofre::Modo::Instalado).unwrap();
    let loja = crate::loja::id(&a.conn).unwrap();
    let conv = cofre::gerar_convite(
        &a.conn,
        &id_a.identidade,
        &id_a.identidade.publica(),
        &loja,
        &codigo,
        "Caixa 2",
        "Ana",
        &format!("127.0.0.1:{pt_a}"),
    )
    .unwrap();
    juntar_com_convite(cb, &conv.payload_qr, &[]).unwrap();
    b
}

/// (a) A entrada de um caixa novo só vale com aprovação de um gerente, e a
/// aprovação fica registrada na auditoria (acceptance 13 de
/// `SIMULTANEIDADE-requisitos.md`) — e chega ao caixa novo com a loja.
#[test]
fn seguro_entrada_legitima_registrada_na_auditoria() {
    thread::spawn(|| {
        let (ca, cb) = (temp("sa-a"), temp("sa-b"));
        let (a, _ua) = loja_raiz(&ca, "Caixa 1");
        let (mut ma, pt_a) = motor(&ca);
        let b = caixa_entra_pelo_convite(&ca, &cb, pt_a, &a);
        let (mut mb, pt_b) = motor(&cb);
        puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b).unwrap();
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        let aprovacoes: i64 = b
            .conn
            .query_row("SELECT COUNT(*) FROM auditoria WHERE acao LIKE '%aprova%caixa%'", [], |l| l.get(0))
            .unwrap_or(0);
        assert!(aprovacoes >= 1, "a entrada do caixa deve ser aprovada por um gerente e ficar na auditoria");
    })
    .join()
    .unwrap();
}

/// (b) Um dispositivo não autorizado (caixa vazio só com a senha da rede) é
/// recusado ao puxar.
#[test]
fn seguro_dispositivo_nao_autorizado_e_recusado() {
    thread::spawn(|| {
        let (ca, cb) = (temp("sb-a"), temp("sb-b"));
        let (_a, _ua) = loja_raiz(&ca, "Caixa 1");
        let _b = caixa_vazio_em_arquivo(&cb, "Caixa Intruso");
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        let r = puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b);
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        assert!(r.is_err(), "um dispositivo sem aprovação do gerente não deve conseguir puxar a loja");
    })
    .join()
    .unwrap();
}

/// (c) Um caixa retirado continua recusado: retirado o dispositivo, nem
/// declarando outro identificador ele volta a receber dados.
#[test]
fn seguro_caixa_retirado_recusado_mesmo_trocando_identificador() {
    thread::spawn(|| {
        let (ca, cb) = (temp("sc-a"), temp("sc-b"));
        let (a, ua) = loja_raiz(&ca, "Caixa 1");
        // O caixa entra de verdade (convite) e troca dados uma vez.
        let b = {
            let (mut ma, pt_a) = motor(&ca);
            let b = caixa_entra_pelo_convite(&ca, &cb, pt_a, &a);
            let (mut mb, pt_b) = motor(&cb);
            puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b).unwrap();
            assert!(ma.encerrar(Duration::from_secs(20)));
            assert!(mb.encerrar(Duration::from_secs(20)));
            b
        };
        // O gerente retira o caixa.
        let id_b = config::ler(&b.conn, "dispositivo_id").unwrap().unwrap();
        retirar_caixa_por_chave(&a.conn, &ca, &id_b, "saiu da loja", "id-da-ana", "Ana").unwrap();
        cliente(&a, &ua, "Cliente Depois Da Retirada");
        // O retirado troca o identificador declarado e tenta de novo.
        config::gravar(&b.conn, "dispositivo_id", "intruso-novo-id-aaaaaaaa").unwrap();
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        let r = puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b);
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        assert!(r.is_err(), "um caixa retirado não deve voltar declarando outro identificador");
        assert!(!tem_cliente(&b, "Cliente Depois Da Retirada"));
    })
    .join()
    .unwrap();
}

/// (d) Revogação efetiva: a chave revogada é recusada mesmo que o dispositivo
/// troque o identificador. No protocolo 4 a chave não vem declarada no pedido: ela
/// é provada no aperto de mão do canal (mais forte que um campo do JSON), e a
/// retirada é feita **só pela chave** (sem o bloqueio de antes por identificador).
#[test]
fn seguro_revogacao_efetiva_por_chave_de_dispositivo() {
    use crate::troca::protocolo4::{aprovados, cofre};
    thread::spawn(|| {
        let (ca, cb) = (temp("sd-a"), temp("sd-b"));
        let (a, ua) = loja_raiz(&ca, "Caixa 1");
        let b = {
            let (mut ma, pt_a) = motor(&ca);
            let b = caixa_entra_pelo_convite(&ca, &cb, pt_a, &a);
            assert!(ma.encerrar(Duration::from_secs(20)));
            b
        };
        let id_a = cofre::identidade_do_processo(ca.parent().unwrap(), &cofre::Modo::Instalado).unwrap();
        let id_b = cofre::identidade_do_processo(cb.parent().unwrap(), &cofre::Modo::Instalado).unwrap();
        let loja = crate::loja::id(&a.conn).unwrap();
        let raiz = id_a.identidade.publica();
        aprovados::retirar(&a.conn, &id_a.identidade, &raiz, &loja, &id_b.identidade.publica(), "roubado", &ua)
            .unwrap();
        assert!(!retirado(&a.conn, &config::ler(&b.conn, "dispositivo_id").unwrap().unwrap()).unwrap());
        config::gravar(&b.conn, "dispositivo_id", "outra-identificacao-qualquer").unwrap();
        cliente(&a, &ua, "Cliente Depois Da Revogacao");
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        let r = puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b);
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        assert!(r.is_err(), "a revogação vale por máquina (pela chave), não pelo identificador declarado");
        assert!(!tem_cliente(&b, "Cliente Depois Da Revogacao"));
    })
    .join()
    .unwrap();
}

/// (e) Proteção contra falsificação de identidade: o anúncio que o motor manda é
/// assinado pela chave do dispositivo, e ninguém anuncia a identidade de outro.
#[test]
fn seguro_anuncio_assinado_contra_falsificacao() {
    use crate::troca::protocolo4::anuncio;
    let ca = temp("se-a");
    let (a, _u) = loja_raiz(&ca, "Caixa 1");
    let ctx = super::p4::contexto(&a.conn, &ca).expect("o caixa raiz fala o P4");
    let pacote = super::p4::pacote_de_anuncio(&ctx, PORTA_TCP).unwrap();
    let lido = anuncio::abrir(&pacote, agora_ms()).expect("o anúncio é assinado e confere");
    assert_eq!(lido.chave, ctx.identidade.identidade.publica(), "a assinatura é da chave deste caixa");
    // Trocar qualquer coisa (a porta, por exemplo) derruba a assinatura.
    let mut adulterado = pacote.clone();
    let ultimo = adulterado.len() - 1;
    adulterado[ultimo] ^= 0x01;
    assert!(anuncio::abrir(&adulterado, agora_ms()).is_err(), "anúncio adulterado não confere");
    // Outra chave não assina pela deste caixa: um anúncio montado por ela traz a chave dela.
    let outra = crate::troca::protocolo4::Identidade::gerar().unwrap();
    let falso = anuncio::montar(&outra, &ctx.loja, PORTA_TCP, agora_ms()).unwrap();
    assert_ne!(anuncio::abrir(&falso, agora_ms()).unwrap().chave, lido.chave);
}

/// (f) A loja não entrega `usuario`/`pessoa` a um banco vazio sem aprovação.
#[test]
fn seguro_loja_nao_entrega_a_banco_vazio_nao_autorizado() {
    thread::spawn(|| {
        let (ca, cb) = (temp("sf-a"), temp("sf-b"));
        let (a, ua) = loja_raiz(&ca, "Caixa 1");
        cliente(&a, &ua, "Cliente Sigiloso LGPDF");
        let b = caixa_vazio_em_arquivo(&cb, "Caixa Intruso");
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b).ok();
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        assert_eq!(tem_usuarios(&b), 0, "a loja não deve entregar usuários a um banco vazio não autorizado");
        assert!(!tem_cliente(&b, "Cliente Sigiloso LGPDF"), "nem os clientes (LGPD)");
    })
    .join()
    .unwrap();
}

/// (g) Um anúncio capturado não permite testar a senha offline. Fora da
/// convivência (o padrão), o caixa do protocolo 4 não manda o anúncio cifrado com a
/// chave da senha; o que ele manda (o assinado) não abre com a chave da senha.
#[test]
fn seguro_anuncio_nao_permite_teste_offline_da_senha() {
    let ca = temp("sg-a");
    let (b, _u) = loja_raiz(&ca, "Caixa 1");
    assert!(!super::p4::p3_permitido(&b.conn), "fora da convivência, o anúncio cifrado com a senha não sai");
    let a = ajustes(&b.conn).unwrap();
    let ctx = super::p4::contexto(&b.conn, &ca).unwrap();
    let pacote = super::p4::pacote_de_anuncio(&ctx, PORTA_TCP).unwrap();
    // O procedimento do atacante, com a senha CERTA: a chave tirada dela não abre
    // nada do anúncio que sai.
    let mut sal = [0u8; 16];
    sal.copy_from_slice(&pacote[4..20]);
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&pacote[20..32]);
    let chave = derivar(&a.senha, &sal).unwrap();
    let abre = ChaCha20Poly1305::new_from_slice(&chave).unwrap().decrypt(&nonce.into(), &pacote[32..]).is_ok();
    assert!(!abre, "o anúncio não pode ser aberto com a chave derivada da senha (senão permite teste offline)");
}

// ===========================================================================
// 3. Protocolo 4, peças prontas (ainda não ligadas ao fio) — PASSAM HOJE
// ===========================================================================

/// Etapa 3 (R-01): o anúncio **assinado** do protocolo 4 não tem nada da
/// senha da rede. É o procedimento do atacante de `demo_r01_…` (ler o sal e o
/// nonce do cabeçalho, derivar a chave com a senha e abrir o corpo) aplicado
/// ao anúncio novo, com a senha **certa** e a chave **real** deste caixa: não
/// há o que abrir. O anúncio é o mesmo com qualquer senha da rede (não depende
/// dela) e se confere sem chave nenhuma (é público e assinado).
///
/// Isto **não** deixa verde o `seguro_anuncio_nao_permite_teste_offline_da_senha`:
/// ele confere o anúncio que o `Motor` manda hoje (`anuncio` + `cifrar`, do
/// protocolo 3), que só muda quando o anúncio novo for ligado (etapa 4, com o
/// canal da decisão A5, e a convivência da A4). Até lá o R-01 continua aberto.
#[test]
fn p4_anuncio_assinado_nao_tem_nada_da_senha_da_rede() {
    use crate::troca::protocolo4::{anuncio, Identidade};
    const AGORA: i64 = 1_793_000_000_000;
    let contem = |palheiro: &[u8], agulha: &[u8]| palheiro.windows(agulha.len()).any(|j| j == agulha);

    let (b, _u) = crate::faixas::para_testes::loja("Caixa 1");
    salvar_ajustes(&b.conn, true, SENHA_FRACA, "").unwrap();
    let a = ajustes(&b.conn).unwrap();
    let k = Chaves::de(&b.conn, &a.senha).unwrap();
    let loja = crate::loja::criar(&b.conn).unwrap();
    let id = Identidade::gerar().unwrap();
    let pacote = anuncio::montar(&id, &loja, PORTA_TCP, AGORA).unwrap();

    // Nem o sal desta rede nem a chave tirada da senha estão no anúncio.
    assert!(!contem(&pacote, &k.sal), "o sal da senha da rede foi para o anúncio");
    assert!(!contem(&pacote, &k.propria), "a chave da senha da rede foi para o anúncio");
    assert_ne!(&pacote[..4], MAGICA, "não é uma mensagem cifrada do protocolo 3");

    // O procedimento do atacante, com a senha CERTA, não abre nada: nem com
    // o "sal" do lugar de sempre, nem com a chave real deste caixa.
    let mut sal = [0u8; 16];
    sal.copy_from_slice(&pacote[4..20]);
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&pacote[20..32]);
    for chave in [derivar(&a.senha, &sal).unwrap(), k.propria] {
        let abre = ChaCha20Poly1305::new_from_slice(&chave).unwrap().decrypt(&nonce.into(), &pacote[32..]).is_ok();
        assert!(!abre, "a chave da senha da rede abriu o anúncio assinado");
    }

    // Não depende da senha: com o mesmo momento e nonce, trocar a senha da
    // rede (ou desligar a troca) não muda um byte.
    let antes = anuncio::montar_com(&id, &loja, PORTA_TCP, AGORA, [9; 16]).unwrap();
    salvar_ajustes(&b.conn, true, "outra-senha-bem-maior", "").unwrap();
    let com_outra = anuncio::montar_com(&id, &loja, PORTA_TCP, AGORA, [9; 16]).unwrap();
    assert_eq!(antes, com_outra);

    // E se confere sem chave nenhuma: tudo nele é público e assinado.
    let lido = anuncio::abrir(&pacote, AGORA).unwrap();
    assert_eq!((lido.chave, lido.loja_id.as_str(), lido.porta), (id.publica(), loja.as_str(), PORTA_TCP));
}

// ===========================================================================
// 4. Preparação da 5.5 (13/11): critério de aceitação da etapa 6 — FALHA HOJE
// ===========================================================================

/// (g) Restaurar um backup de antes de uma retirada não traz o caixa retirado
/// de volta: a restauração **junta** o registro de caixas (nunca o substitui).
/// Até a 0.9.19-dev.1 o banco do backup entrava inteiro e a retirada feita
/// depois dele sumia (risco registrado pela frente-p4; B.5 da análise unificada
/// de 05/11). Desde a T009 (0.9.19-dev.2) a restauração (`backup::restaurar`, o
/// `Pendente` que guarda "o que é deste computador") guarda as linhas do registro
/// antes e as devolve com `protocolo4::aprovados::juntar` depois.
#[test]
fn seguro_restaurar_backup_nao_desfaz_retirada() {
    use crate::troca::protocolo4::{aprovados, Identidade};
    let _vez = crate::backup::testes::um_por_vez();
    let caminho = temp("restaurar");
    let b = Banco::abrir(&caminho).unwrap();
    let u = crate::faixas::para_testes::configurar(&b, "Caixa 1", 0);
    let loja = crate::loja::criar(&b.conn).unwrap();
    let (raiz, c2) = (Identidade::gerar().unwrap(), Identidade::gerar().unwrap());
    let r = raiz.publica();
    aprovados::registrar_raiz(&b.conn, &raiz, &loja, "Caixa 1", &u).unwrap();
    aprovados::aprovar(&b.conn, &raiz, &r, &loja, &aprovados::Pedido::de(&c2), "Caixa 2", &u).unwrap();
    drop(b);
    let feito = crate::backup::fazer(&caminho, false).unwrap();
    let mut b = Banco::abrir(&caminho).unwrap();
    aprovados::retirar(&b.conn, &raiz, &r, &loja, &c2.publica(), "computador roubado", &u).unwrap();
    assert!(!aprovados::autorizada(&b.conn, &r, &loja, &c2.publica()).unwrap(), "retirado antes de restaurar");
    crate::backup::restaurar(&mut b.conn, Some(&caminho), &feito.arquivo, false, "1234").unwrap();
    assert!(
        !aprovados::autorizada(&b.conn, &r, &loja, &c2.publica()).unwrap(),
        "restaurar o backup de antes trouxe de volta o caixa retirado"
    );
}
