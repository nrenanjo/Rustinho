//! Frente de segurança — R-01 (senha da rede) e D1 (entrada de caixa).
//!
//! **Comportamento seguro desejado** (`seguro_*`, `#[ignore]`): descrevem o
//! que o protocolo 4 tem de garantir. **Falham hoje de propósito** (o
//! mecanismo não existe na troca de dados); cada um sai do `#[ignore]` na
//! etapa que o deixa verde (dita no motivo do `#[ignore]`) e passa a ser
//! exigido. Rode com `--ignored` para vê-los em vermelho.
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
// 2. Comportamento seguro desejado (protocolo 4) — FALHAM HOJE DE PROPÓSITO
// ===========================================================================

/// (a) A entrada de um caixa novo só vale com aprovação de um gerente, e a
/// aprovação fica registrada na auditoria (acceptance 13 de
/// `SIMULTANEIDADE-requisitos.md`). Hoje a entrada não registra aprovação
/// nenhuma: a senha da rede basta.
#[test]
#[ignore = "D1/R-01: protocolo 4, etapa 5 (entrada aprovada) — aprovação do gerente e auditoria da entrada"]
fn seguro_entrada_legitima_registrada_na_auditoria() {
    thread::spawn(|| {
        let (ca, cb) = (temp("sa-a"), temp("sa-b"));
        let (_a, _ua) = loja_em_arquivo(&ca, "Caixa 1");
        let b = caixa_vazio_em_arquivo(&cb, "Caixa 2");
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b).ok();
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

/// (b) Um dispositivo não autorizado (caixa vazio só com a senha da rede) deve
/// ser recusado ao puxar. Hoje a puxada tem sucesso.
#[test]
#[ignore = "D1/R-01: protocolo 4, etapa 4 (canal autenticado, A5) — rejeição de dispositivo não autorizado"]
fn seguro_dispositivo_nao_autorizado_e_recusado() {
    thread::spawn(|| {
        let (ca, cb) = (temp("sb-a"), temp("sb-b"));
        let (_a, _ua) = loja_em_arquivo(&ca, "Caixa 1");
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

/// (c) Um caixa retirado deve continuar recusado. Hoje a retirada é por
/// identificador declarado; o SEGURO é que, retirado o dispositivo, nem
/// declarando outro identificador ele volte a receber dados.
#[test]
#[ignore = "D1/R-01: protocolo 4, etapa 6 (revogação por chave) — retirada efetiva (não por identificador declarado)"]
fn seguro_caixa_retirado_recusado_mesmo_trocando_identificador() {
    thread::spawn(|| {
        let (ca, cb) = (temp("sc-a"), temp("sc-b"));
        let (a, _ua) = loja_em_arquivo(&ca, "Caixa 1");
        let b = caixa_vazio_em_arquivo(&cb, "Caixa Intruso");
        // O intruso entra uma vez (hoje basta a senha).
        {
            let (mut ma, pt_a) = motor(&ca);
            let (mut mb, pt_b) = motor(&cb);
            puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b).ok();
            puxar(&ca, &format!("127.0.0.1:{pt_b}"), pt_a).ok();
            assert!(ma.encerrar(Duration::from_secs(20)));
            assert!(mb.encerrar(Duration::from_secs(20)));
        }
        // O gerente retira o intruso.
        let id_b = config::ler(&b.conn, "dispositivo_id").unwrap().unwrap();
        retirar_caixa(&a.conn, &id_b, "saiu da loja", "Ana").ok();
        // O intruso troca o identificador declarado e tenta de novo.
        config::gravar(&b.conn, "dispositivo_id", "intruso-novo-id-aaaaaaaa").unwrap();
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        let r = puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b);
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        assert!(r.is_err(), "um caixa retirado não deve voltar declarando outro identificador");
    })
    .join()
    .unwrap();
}

/// (d) Revogação efetiva: uma chave revogada deve ser recusada mesmo que o
/// dispositivo troque o identificador. Hoje não há chave de dispositivo; a
/// identidade é um texto declarado, então a revogação não "cola" na máquina.
#[test]
#[ignore = "D1/R-01: protocolo 4, etapas 4 e 6 (chave do caixa no pedido, pelo canal) — revogação por chave de dispositivo"]
fn seguro_revogacao_efetiva_por_chave_de_dispositivo() {
    // Propriedade desejada: o pedido da troca deve trazer a chave pública do
    // dispositivo (verificável), não só o identificador declarado, para que a
    // revogação valha por máquina. Hoje o pedido não tem chave nenhuma.
    let (b, _u) = crate::faixas::para_testes::loja("Caixa 1");
    salvar_ajustes(&b.conn, true, SENHA_FRACA, "").unwrap();
    let a = ajustes(&b.conn).unwrap();
    let (loja_id, sou_novo) = crate::loja::campos(&b.conn).unwrap();
    // O pedido montado como em `puxar_por`.
    let pedido = json!({ "tipo": "puxar", "protocolo": PROTOCOLO, "dispositivo": a.dispositivo,
                         "nome": a.nome, "loja": loja_id, "novo": sou_novo });
    assert!(
        pedido.get("chavePublica").and_then(|v| v.as_str()).is_some(),
        "o pedido da troca deve trazer a chave pública do dispositivo para a revogação valer por máquina"
    );
}

/// (e) Proteção contra falsificação de identidade: o anúncio deve ser assinado
/// pela chave do dispositivo, para ninguém anunciar a identidade de outro. Hoje
/// o anúncio não é assinado (só `{dispositivo, nome, porta}`).
#[test]
#[ignore = "D1/R-01: protocolo 4, etapa 4 (o Motor passa a mandar o anúncio da etapa 3; A4/A5) — anúncio assinado (identidade não falsificável)"]
fn seguro_anuncio_assinado_contra_falsificacao() {
    let ola = anuncio("caixa-qualquer", "Caixa 1", PORTA_TCP);
    assert!(
        ola.get("assinatura").and_then(|v| v.as_str()).is_some(),
        "o anúncio deve levar uma assinatura da chave do dispositivo"
    );
}

/// (f) A loja deve ser protegida contra um banco vazio não autorizado: ao
/// atender um caixa sem aprovação, não entrega `usuario`/`pessoa`. Hoje entrega
/// a loja inteira a quem tem a senha da rede.
#[test]
#[ignore = "D1/R-01: protocolo 4, etapas 4 e 5 — loja não entrega dados a banco vazio não autorizado"]
fn seguro_loja_nao_entrega_a_banco_vazio_nao_autorizado() {
    thread::spawn(|| {
        let (ca, cb) = (temp("sf-a"), temp("sf-b"));
        let (a, ua) = loja_em_arquivo(&ca, "Caixa 1");
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

/// (g) Um anúncio capturado não deve permitir testar a senha offline: ele não
/// pode conter texto cifrado com uma chave derivada da senha. Hoje o anúncio é
/// exatamente isso (começa com a magia do protocolo e abre com a chave da
/// senha), que é a raiz do R-01.
#[test]
#[ignore = "D1/R-01: protocolo 4, etapas 4 e 7 (só quando o anúncio cifrado do protocolo 3 deixar de sair; A4) — anúncio sem material que permita teste offline da senha"]
fn seguro_anuncio_nao_permite_teste_offline_da_senha() {
    let (b, _u) = crate::faixas::para_testes::loja("Caixa 1");
    salvar_ajustes(&b.conn, true, SENHA_FRACA, "").unwrap();
    let a = ajustes(&b.conn).unwrap();
    let k = Chaves::de(&b.conn, &a.senha).unwrap();
    let pacote = cifrar(&k, anuncio(&a.dispositivo, &a.nome, PORTA_TCP).to_string().as_bytes()).unwrap();

    // Propriedade desejada: a chave derivada da senha NÃO deve abrir o anúncio
    // (porque o anúncio não deve ser cifrado com ela). Hoje abre.
    let mut sal = [0u8; 16];
    sal.copy_from_slice(&pacote[4..20]);
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&pacote[20..32]);
    let chave = derivar(&a.senha, &sal).unwrap();
    let abre = ChaCha20Poly1305::new_from_slice(&chave).unwrap().decrypt(&nonce.into(), &pacote[32..]).is_ok();
    assert!(!abre, "o anúncio não pode ser aberto com a chave derivada da senha (senão permite teste offline)");
}
