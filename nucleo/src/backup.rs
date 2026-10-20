//! Backup: cópias do banco no computador e no Google Drive.
//!
//! Como funciona:
//! - Uma vez por dia (no horário escolhido, ou na hora se passou um dia
//!   inteiro sem cópia) e ao fechar o Rustinho, o banco é copiado inteiro
//!   (`VACUUM INTO`, que tira uma "foto" sem parar o caixa), comprimido em
//!   zip e criptografado (ChaCha20-Poly1305), em pedaços de 1 MB: nem o
//!   banco grande inteiro precisa caber na memória.
//! - Chave (desde a 0.9.11): a loja tem um par de chaves X25519. Fazer o
//!   backup usa só a parte pública (fica no banco: o backup automático
//!   funciona sem ninguém digitar nada). Abrir o backup precisa da parte
//!   secreta, que só fica guardada trancada com a senha de cada gerente
//!   (Argon2). Quem copiar o banco não abre os backups sem a senha de um
//!   gerente. Cada arquivo leva junto os "cadeados" dos gerentes daquele dia,
//!   para restaurar até num computador novo.
//!   Atenção: a proteção é a da senha do gerente. Senha curta (4 números)
//!   se descobre testando todas; para proteger de verdade, 8 ou mais.
//! - Depois de gravar, o Rustinho abre a cópia e confere se ela está inteira.
//!   Cópia que não abre não conta como backup e vira aviso na tela inicial.
//! - Guarda os últimos N dias (um por dia; do dia de hoje, todos) e nunca
//!   apaga as 3 cópias mais novas de cada caixa.
//! - Google Drive: pelo programa `rclone` (grátis, `sudo pacman -S rclone`).
//!   A pessoa entra na conta do Google no navegador; o Rustinho nunca vê a
//!   senha do Google. Na nuvem só vão cópias criptografadas.
//! - Restaurar: guarda antes uma cópia do que existe, troca o conteúdo do
//!   banco e mantém o que é deste computador (nome do caixa, sincronização,
//!   pasta do backup). Com a sincronização ligada, este caixa recebe de novo
//!   dos outros tudo o que mudou depois da data do backup.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::ChaCha20Poly1305;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::erro::{Erro, Resultado};
use crate::{banco, config, sync, tempo, texto};

/// Começo de todo arquivo de backup do Rustinho.
const MAGICA: &[u8; 8] = b"RUSTBKP1";
const EXTENSAO: &str = "rbk";
/// Nome da conexão do rclone com o Google Drive.
pub const REMOTO: &str = "rustinho-drive";
/// Pasta dentro do Google Drive.
const PASTA_NUVEM: &str = "Rustinho Backup";
/// Dias guardados se ninguém mudar.
pub const DIAS_PADRAO: i64 = 30;
/// Horário do backup do dia se ninguém mudar.
pub const HORA_PADRAO: &str = "18:00";
/// Cópias mais novas de cada caixa que nunca são apagadas.
const SEMPRE_GUARDAR: usize = 3;
/// Dias sem backup até aparecer o aviso.
pub const DIAS_ATE_AVISAR: i64 = 2;

// ---------------------------------------------------------------------------
// Trabalho em segundo plano (um de cada vez).
// ---------------------------------------------------------------------------

static OCUPADO: AtomicBool = AtomicBool::new(false);
static FAZENDO: Mutex<String> = Mutex::new(String::new());

/// Enquanto existir, ninguém mais começa um backup.
struct Vez;

impl Vez {
    fn pegar(texto: &str) -> Option<Vez> {
        if OCUPADO.swap(true, Ordering::SeqCst) {
            return None;
        }
        dizer(texto);
        Some(Vez)
    }
}

impl Drop for Vez {
    fn drop(&mut self) {
        dizer("");
        OCUPADO.store(false, Ordering::SeqCst);
    }
}

fn dizer(texto: &str) {
    if let Ok(mut f) = FAZENDO.lock() {
        *f = texto.to_string();
    }
}

/// O que o backup está fazendo agora ("" = nada).
pub fn fazendo() -> String {
    FAZENDO.lock().map(|f| f.clone()).unwrap_or_default()
}

pub fn ocupado() -> bool {
    OCUPADO.load(Ordering::SeqCst)
}

// ---------------------------------------------------------------------------
// Ajustes (deste computador, menos a senha, que é da loja).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Ajustes {
    pub pasta: PathBuf,
    pub dias: i64,
    /// "HH:MM"
    pub hora: String,
    pub nuvem: bool,
    /// Parte pública da chave da loja. None = ainda não tem (cópia sem
    /// criptografia, até um gerente entrar).
    pub publica: Option<[u8; 32]>,
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn de_hex(t: &str) -> Option<Vec<u8>> {
    if !t.len().is_multiple_of(2) {
        return None;
    }
    (0..t.len()).step_by(2).map(|i| u8::from_str_radix(t.get(i..i + 2)?, 16).ok()).collect()
}

/// Tamanho do "cadeado" de um gerente: sal (16), nonce (12) e a parte
/// secreta da chave trancada (32 + 16 de conferência).
const ACESSO: usize = 76;

/// Parte pública da chave da loja (vai para os outros caixas).
pub fn publica_da_loja(conn: &Connection) -> Resultado<Option<[u8; 32]>> {
    Ok(config::ler(conn, "backup_publica")?.and_then(|t| de_hex(&t)).and_then(|v| v.try_into().ok()))
}

fn publica_de(segredo: &[u8; 32]) -> [u8; 32] {
    x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from(*segredo)).to_bytes()
}

/// Tranca a parte secreta com a senha (Argon2 + ChaCha20-Poly1305).
fn trancar(segredo: &[u8; 32], senha: &str) -> Resultado<Vec<u8>> {
    let sal: [u8; 16] = aleatorio()?;
    let nonce: [u8; 12] = aleatorio()?;
    let cifra = ChaCha20Poly1305::new_from_slice(&chave_da_senha(senha, &sal)?)
        .map_err(|_| Erro::Interno("chave inválida".into()))?;
    let corpo = cifra
        .encrypt(&nonce.into(), segredo.as_slice())
        .map_err(|_| Erro::Interno("falha ao proteger a chave do backup".into()))?;
    Ok([sal.as_slice(), nonce.as_slice(), corpo.as_slice()].concat())
}

fn destrancar(acesso: &[u8], senha: &str) -> Option<[u8; 32]> {
    if acesso.len() != ACESSO || senha.is_empty() {
        return None;
    }
    let k = chave_da_senha(senha, &acesso[..16]).ok()?;
    let cifra = ChaCha20Poly1305::new_from_slice(&k).ok()?;
    let nonce: [u8; 12] = acesso[16..28].try_into().ok()?;
    cifra.decrypt(&nonce.into(), &acesso[28..]).ok()?.try_into().ok()
}

/// Cadeados dos gerentes ativos (para ir junto em cada backup).
fn acessos_da_loja(conn: &Connection) -> Resultado<Vec<Vec<u8>>> {
    let mut c = conn.prepare(
        "SELECT backup_acesso FROM usuario WHERE gerente = 1 AND ativo = 1 AND backup_acesso IS NOT NULL ORDER BY id",
    )?;
    let lista: Vec<String> = c.query_map([], |l| l.get(0))?.collect::<Result<_, _>>()?;
    Ok(lista.iter().filter_map(|t| de_hex(t)).filter(|v| v.len() == ACESSO).collect())
}

fn gravar_acesso(conn: &Connection, usuario_id: &str, acesso: Option<&[u8]>) -> Resultado<()> {
    conn.execute(
        "UPDATE usuario SET backup_acesso = ?2, atualizado_em = ?3 WHERE id = ?1",
        params![usuario_id, acesso.map(hex), tempo::agora()],
    )?;
    Ok(())
}

/// A senha de um gerente acabou de ser conferida (entrar, aperto, primeiro
/// usuário). Abre a parte secreta com ela (ou cria a chave da loja, se ainda
/// não tem; ou tranca para ele a que já está na memória). Devolve a parte
/// secreta para ficar na memória do programa enquanto ele estiver aberto.
pub fn gerente_conferido(
    conn: &Connection,
    usuario_id: &str,
    senha: &str,
    memoria: Option<[u8; 32]>,
) -> Resultado<Option<[u8; 32]>> {
    let publica = publica_da_loja(conn)?;
    let atual: Option<String> = conn
        .query_row("SELECT backup_acesso FROM usuario WHERE id = ?1", [usuario_id], |l| l.get(0))
        .optional()?
        .flatten();
    if let (Some(p), Some(a)) = (publica, atual.as_deref().and_then(de_hex)) {
        if let Some(s) = destrancar(&a, senha).filter(|s| publica_de(s) == p) {
            return Ok(Some(s));
        }
    }
    let segredo = match (publica, memoria) {
        (None, _) => {
            let s: [u8; 32] = aleatorio()?;
            config::gravar(conn, "backup_publica", &hex(&publica_de(&s)))?;
            s
        }
        (Some(p), Some(m)) if publica_de(&m) == p => m,
        // Sem como abrir agora: fica para quando outro gerente entrar.
        _ => return Ok(memoria),
    };
    gravar_acesso(conn, usuario_id, Some(&trancar(&segredo, senha)?))?;
    Ok(Some(segredo))
}

/// A senha de um usuário mudou. Se ele é gerente, o cadeado passa para a
/// senha nova (a antiga deixa de abrir os backups novos).
pub fn senha_trocada(conn: &Connection, usuario_id: &str, nova: &str, memoria: Option<[u8; 32]>) -> Resultado<()> {
    let gerente: bool = conn
        .query_row("SELECT gerente FROM usuario WHERE id = ?1", [usuario_id], |l| l.get(0))
        .optional()?
        .unwrap_or(false);
    if !gerente {
        return Ok(());
    }
    match (publica_da_loja(conn)?, memoria) {
        (Some(p), Some(m)) if publica_de(&m) == p => gravar_acesso(conn, usuario_id, Some(&trancar(&m, nova)?)),
        _ => gravar_acesso(conn, usuario_id, None),
    }
}

/// Quem deixa de ser gerente (ou é desativado) não abre mais os backups novos.
pub fn deixou_de_ser_gerente(conn: &Connection, usuario_id: &str) -> Resultado<()> {
    gravar_acesso(conn, usuario_id, None)
}

/// Até a 0.9.11 a loja guardava a senha do backup (até a 0.9.1) ou a chave
/// tirada dela. As duas saem do banco (e dos outros caixas). Os backups
/// antigos continuam abrindo com a senha do backup de então.
pub fn limpar_chave_antiga(conn: &Connection) -> Resultado<()> {
    conn.execute("DELETE FROM config WHERE chave IN ('backup_senha', 'backup_chave')", [])?;
    Ok(())
}

/// Pasta padrão: `backups` ao lado do banco.
pub fn pasta_padrao(caminho_db: Option<&Path>) -> PathBuf {
    caminho_db.and_then(|c| c.parent()).map(|p| p.join("backups")).unwrap_or_else(|| PathBuf::from("backups"))
}

pub fn ajustes(conn: &Connection, caminho_db: Option<&Path>) -> Resultado<Ajustes> {
    let pasta = config::ler(conn, "backup_pasta")?
        .filter(|p| !p.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| pasta_padrao(caminho_db));
    let dias = config::ler(conn, "backup_dias")?
        .and_then(|d| d.parse().ok())
        .filter(|d| (1..=365).contains(d))
        .unwrap_or(DIAS_PADRAO);
    let hora = config::ler(conn, "backup_hora")?.filter(|h| hora_valida(h)).unwrap_or_else(|| HORA_PADRAO.to_string());
    Ok(Ajustes { pasta, dias, hora, nuvem: config::ligado(conn, "backup_nuvem")?, publica: publica_da_loja(conn)? })
}

fn hora_valida(h: &str) -> bool {
    let b = h.as_bytes();
    b.len() == 5
        && b[2] == b':'
        && h[..2].parse::<u8>().is_ok_and(|x| x < 24)
        && h[3..].parse::<u8>().is_ok_and(|x| x < 60)
        && b[..2].iter().chain(&b[3..]).all(u8::is_ascii_digit)
}

/// O que vem da tela.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AjustesEntrada {
    pub pasta: String,
    pub dias: String,
    pub hora: String,
}

pub fn salvar_ajustes(conn: &Connection, caminho_db: Option<&Path>, e: &AjustesEntrada) -> Resultado<()> {
    let pasta = e.pasta.trim();
    if !pasta.is_empty() && !Path::new(pasta).is_absolute() {
        return Err(Erro::campo("pasta", "Escolha uma pasta do computador."));
    }
    let dias: i64 = e.dias.trim().parse().map_err(|_| Erro::campo("dias", "Escreva quantos dias (de 1 a 365)."))?;
    if !(1..=365).contains(&dias) {
        return Err(Erro::campo("dias", "Use de 1 a 365 dias."));
    }
    let mut hora = e.hora.trim().to_string();
    if hora.len() == 4 && hora.as_bytes()[1] == b':' {
        hora.insert(0, '0');
    }
    if !hora_valida(&hora) {
        return Err(Erro::campo("hora", "Escreva o horário assim: 18:00."));
    }
    let padrao = pasta_padrao(caminho_db);
    config::gravar(conn, "backup_pasta", if Path::new(pasta) == padrao { "" } else { pasta })?;
    config::gravar(conn, "backup_dias", &dias.to_string())?;
    config::gravar(conn, "backup_hora", &hora)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Situação (gravada neste computador).
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Situacao {
    /// Último backup que deu certo (UTC) e o seu tamanho.
    pub ultimo: String,
    pub ultimo_tamanho: i64,
    pub ultimo_arquivo: String,
    /// Erro da última tentativa ("" = deu certo).
    pub erro: String,
    pub erro_quando: String,
    pub nuvem_ultimo: String,
    pub nuvem_erro: String,
    /// Desde quando está ligado (para o aviso de "2 dias sem backup").
    pub desde: String,
    pub nuvem_desde: String,
}

pub fn situacao(conn: &Connection) -> Resultado<Situacao> {
    let l = |k: &str| -> Resultado<String> { Ok(config::ler(conn, k)?.unwrap_or_default()) };
    Ok(Situacao {
        ultimo: l("backup_ultimo")?,
        ultimo_tamanho: l("backup_ultimo_tamanho")?.parse().unwrap_or(0),
        ultimo_arquivo: l("backup_ultimo_arquivo")?,
        erro: l("backup_erro")?,
        erro_quando: l("backup_erro_quando")?,
        nuvem_ultimo: l("backup_nuvem_ultimo")?,
        nuvem_erro: l("backup_nuvem_erro")?,
        desde: l("backup_desde")?,
        nuvem_desde: l("backup_nuvem_desde")?,
    })
}

// ---------------------------------------------------------------------------
// Nome dos arquivos: rustinho-AAAAMMDD-HHMMSS-<caixa>[-antes].rbk
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NomeBackup {
    pub nome: String,
    /// "AAAAMMDD"
    pub dia: String,
    /// "HHMMSS"
    pub hora: String,
    pub caixa: String,
    /// Cópia feita logo antes de uma restauração.
    pub antes: bool,
}

pub fn ler_nome(nome: &str) -> Option<NomeBackup> {
    let meio = nome.strip_prefix("rustinho-")?.strip_suffix(&format!(".{EXTENSAO}"))?;
    let mut partes = meio.splitn(3, '-');
    let dia = partes.next()?;
    let hora = partes.next()?;
    let resto = partes.next()?;
    if dia.len() != 8 || hora.len() != 6 || !dia.bytes().chain(hora.bytes()).all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (caixa, antes) = match resto.strip_suffix("-antes") {
        Some(c) => (c, true),
        None => (resto, false),
    };
    if caixa.is_empty() || !caixa.chars().all(char::is_alphanumeric) {
        return None;
    }
    Some(NomeBackup { nome: nome.to_string(), dia: dia.into(), hora: hora.into(), caixa: caixa.into(), antes })
}

fn montar_nome(conn: &Connection, antes: bool) -> Resultado<String> {
    let (dia, hora): (String, String) = conn.query_row(
        "SELECT strftime('%Y%m%d', 'now', 'localtime'), strftime('%H%M%S', 'now', 'localtime')",
        [],
        |l| Ok((l.get(0)?, l.get(1)?)),
    )?;
    let mut caixa = texto::compactar(&config::nome_caixa(conn)?);
    if caixa.is_empty() {
        caixa = "caixa".into();
    }
    Ok(format!("rustinho-{dia}-{hora}-{caixa}{}.{EXTENSAO}", if antes { "-antes" } else { "" }))
}

/// Quais cópias apagar: de cada caixa, guarda todas de hoje, a mais nova de
/// cada dia até `dias` atrás e, sempre, as 3 mais novas.
pub fn para_apagar(nomes: &[String], hoje: &str, dias: i64) -> Vec<String> {
    let limite = dia_menos(hoje, dias);
    let mut lista: Vec<NomeBackup> = nomes.iter().filter_map(|n| ler_nome(n)).collect();
    // Mais novas primeiro.
    lista.sort_by(|a, b| (&b.dia, &b.hora).cmp(&(&a.dia, &a.hora)));
    let mut apagar = Vec::new();
    let mut vistos: Vec<(String, usize, Vec<String>)> = Vec::new(); // (caixa, quantas, dias já guardados)
    for b in lista {
        let pos = match vistos.iter().position(|v| v.0 == b.caixa) {
            Some(p) => p,
            None => {
                vistos.push((b.caixa.clone(), 0, Vec::new()));
                vistos.len() - 1
            }
        };
        let v = &mut vistos[pos];
        v.1 += 1;
        if v.1 <= SEMPRE_GUARDAR {
            v.2.push(b.dia.clone());
            continue;
        }
        let guardar = b.dia.as_str() == hoje || (b.dia.as_str() > limite.as_str() && !v.2.contains(&b.dia));
        if guardar {
            v.2.push(b.dia.clone());
        } else {
            apagar.push(b.nome);
        }
    }
    apagar
}

/// "AAAAMMDD" menos `dias`.
fn dia_menos(dia: &str, dias: i64) -> String {
    let ler = || -> Option<time::Date> {
        let mes = time::Month::try_from(dia.get(4..6)?.parse::<u8>().ok()?).ok()?;
        time::Date::from_calendar_date(dia.get(..4)?.parse().ok()?, mes, dia.get(6..8)?.parse().ok()?).ok()
    };
    match ler() {
        Some(d) => {
            let d = d - time::Duration::days(dias);
            format!("{:04}{:02}{:02}", d.year(), u8::from(d.month()), d.day())
        }
        None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Formato do arquivo: MAGICA, 1 byte, depois:
//   0 = sem senha: o zip (rustinho.db + info.json) como está;
//   1 = com senha, até a 0.9.1: sal 16, nonce 12, o zip criptografado inteiro;
//   2 = com senha, desde a 0.9.11: sal 16, começo do nonce 7, e o zip
//       criptografado em pedaços de 1 MB (cada um com o número do pedaço e
//       a marca de último; nada pode ser cortado, trocado de lugar ou mudado).
// ---------------------------------------------------------------------------

const PEDACO: usize = 1024 * 1024;

fn chave_da_senha(senha: &str, sal: &[u8]) -> Resultado<[u8; 32]> {
    let mut k = [0u8; 32];
    crate::usuarios::argon2()
        .hash_password_into(senha.as_bytes(), sal, &mut k)
        .map_err(|e| Erro::Interno(format!("não foi possível proteger o backup: {e}")))?;
    Ok(k)
}

fn aleatorio<const N: usize>() -> Resultado<[u8; N]> {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).map_err(|e| Erro::Interno(format!("sem números aleatórios: {e}")))?;
    Ok(b)
}

fn nonce_do_pedaco(comeco: &[u8; 7], numero: u32, ultimo: bool) -> [u8; 12] {
    let mut n = [0u8; 12];
    n[..7].copy_from_slice(comeco);
    n[7..11].copy_from_slice(&numero.to_be_bytes());
    n[11] = ultimo as u8;
    n
}

/// Lê até encher o buffer (ou acabar o arquivo). Devolve quantos leu.
fn ler_cheio(r: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..])? {
            0 => break,
            k => n += k,
        }
    }
    Ok(n)
}

/// Chave de um arquivo do formato 3 (a mesma dos dois lados da troca).
fn chave_do_arquivo(
    segredo_ou_efemero: &[u8; 32],
    outra_publica: &[u8; 32],
    efemera: &[u8; 32],
    publica: &[u8; 32],
) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let comum = x25519_dalek::StaticSecret::from(*segredo_ou_efemero)
        .diffie_hellman(&x25519_dalek::PublicKey::from(*outra_publica));
    let mut h = Sha256::new();
    h.update(b"rustinho-backup-3");
    h.update(comum.as_bytes());
    h.update(efemera);
    h.update(publica);
    h.finalize().into()
}

fn escrever_pedacos(
    entrada: &mut impl Read,
    destino: &mut impl Write,
    chave: &[u8; 32],
    comeco: &[u8; 7],
) -> Resultado<()> {
    let falha = |e: std::io::Error| Erro::validacao(format!("Não deu para gravar o backup: {e}"));
    let cifra = ChaCha20Poly1305::new_from_slice(chave).map_err(|_| Erro::Interno("chave inválida".into()))?;
    let mut atual = vec![0u8; PEDACO];
    let mut proximo = vec![0u8; PEDACO];
    let mut n = ler_cheio(entrada, &mut atual).map_err(falha)?;
    let mut numero: u32 = 0;
    loop {
        let m = if n == PEDACO { ler_cheio(entrada, &mut proximo).map_err(falha)? } else { 0 };
        let ultimo = m == 0;
        let corpo = cifra
            .encrypt(&nonce_do_pedaco(comeco, numero, ultimo).into(), &atual[..n])
            .map_err(|_| Erro::Interno("falha ao criptografar o backup".into()))?;
        destino.write_all(&corpo).map_err(falha)?;
        if ultimo {
            return Ok(());
        }
        std::mem::swap(&mut atual, &mut proximo);
        n = m;
        numero = numero.checked_add(1).ok_or_else(|| Erro::validacao("O backup ficou grande demais."))?;
    }
}

fn ler_pedacos(
    f: &mut impl Read,
    destino: &mut impl Write,
    chave: &[u8; 32],
    comeco: &[u8; 7],
    senha_errada: fn() -> Erro,
) -> Resultado<()> {
    let ler_falha = |e: std::io::Error| Erro::validacao(format!("Não deu para ler o backup: {e}"));
    let gravar_falha = |e: std::io::Error| Erro::validacao(format!("Não deu para abrir o backup: {e}"));
    let incompleto = || Erro::validacao("O backup está incompleto (o arquivo foi cortado).");
    let cifra = ChaCha20Poly1305::new_from_slice(chave).map_err(|_| Erro::Interno("chave inválida".into()))?;
    let tamanho = PEDACO + 16;
    let mut atual = vec![0u8; tamanho];
    let mut proximo = vec![0u8; tamanho];
    let mut n = ler_cheio(f, &mut atual).map_err(ler_falha)?;
    let mut numero: u32 = 0;
    loop {
        if n < 16 {
            return Err(incompleto());
        }
        let m = if n == tamanho { ler_cheio(f, &mut proximo).map_err(ler_falha)? } else { 0 };
        let ultimo = m == 0;
        let claro = cifra.decrypt(&nonce_do_pedaco(comeco, numero, ultimo).into(), &atual[..n]).map_err(|_| {
            if numero == 0 {
                senha_errada()
            } else {
                Erro::validacao("O backup está estragado.")
            }
        })?;
        destino.write_all(&claro).map_err(gravar_falha)?;
        if ultimo {
            return Ok(());
        }
        std::mem::swap(&mut atual, &mut proximo);
        n = m;
        numero = numero.checked_add(1).ok_or_else(|| Erro::validacao("O backup está estragado."))?;
    }
}

/// Grava o zip (do arquivo `origem`) no formato do backup, em `destino`.
/// Com a chave pública da loja: formato 3 (criptografado), levando junto
/// os cadeados dos gerentes. Devolve a chave do arquivo (para conferir a
/// cópia logo depois, sem precisar de senha).
fn embrulhar(
    origem: &Path,
    destino: &mut impl Write,
    publica: Option<&[u8; 32]>,
    acessos: &[Vec<u8>],
) -> Resultado<Option<[u8; 32]>> {
    let falha = |e: std::io::Error| Erro::validacao(format!("Não deu para gravar o backup: {e}"));
    let mut entrada = std::fs::File::open(origem).map_err(falha)?;
    destino.write_all(MAGICA).map_err(falha)?;
    let Some(p) = publica else {
        destino.write_all(&[0]).map_err(falha)?;
        std::io::copy(&mut entrada, destino).map_err(falha)?;
        return Ok(None);
    };
    let efemero: [u8; 32] = aleatorio()?;
    let efemera = publica_de(&efemero);
    let chave = chave_do_arquivo(&efemero, p, &efemera, p);
    let comeco: [u8; 7] = aleatorio()?;
    let acessos: Vec<&Vec<u8>> = acessos.iter().filter(|a| a.len() == ACESSO).take(255).collect();
    destino.write_all(&[3]).map_err(falha)?;
    destino.write_all(p).map_err(falha)?;
    destino.write_all(&efemera).map_err(falha)?;
    destino.write_all(&comeco).map_err(falha)?;
    destino.write_all(&[acessos.len() as u8]).map_err(falha)?;
    for a in acessos {
        destino.write_all(a).map_err(falha)?;
    }
    escrever_pedacos(&mut entrada, destino, &chave, &comeco)?;
    Ok(Some(chave))
}

/// O arquivo pede senha?
pub fn tem_senha(bytes: &[u8]) -> Option<bool> {
    if bytes.len() < 9 || &bytes[..8] != MAGICA {
        return None;
    }
    Some(bytes[8] != 0)
}

fn senha_errada() -> Erro {
    Erro::campo("senha", "A senha não abre este backup (ou o arquivo está estragado).")
}

fn senha_de_gerente_errada() -> Erro {
    Erro::campo(
        "senha",
        "Essa senha não abre este backup. Use a senha de um gerente: a atual deste caixa ou a de quando o backup foi feito.",
    )
}

/// Maior backup do formato antigo (que é lido inteiro na memória).
#[cfg(target_os = "android")]
const TAMANHO_FORMATO_ANTIGO: u64 = 128 * 1024 * 1024;
#[cfg(not(target_os = "android"))]
const TAMANHO_FORMATO_ANTIGO: u64 = 1024 * 1024 * 1024;

/// Maior backup do formato antigo, com ajuste só para os testes (F-11): o
/// teste de "grande demais" não precisa de um arquivo de gigabytes.
fn limite_formato_antigo() -> u64 {
    #[cfg(test)]
    {
        let v = limites_de_teste::FORMATO_ANTIGO.with(|c| c.get());
        if v != 0 {
            return v;
        }
    }
    TAMANHO_FORMATO_ANTIGO
}

/// Maior banco dentro de um backup, com o mesmo ajuste para os testes.
fn limite_do_banco() -> u64 {
    #[cfg(test)]
    {
        let v = limites_de_teste::BANCO.with(|c| c.get());
        if v != 0 {
            return v;
        }
    }
    TAMANHO_BANCO
}

/// Ajuste dos limites por teste (cada teste roda na sua própria thread, então
/// um teste não muda o limite dos outros).
#[cfg(test)]
mod limites_de_teste {
    use std::cell::Cell;
    thread_local! {
        pub static FORMATO_ANTIGO: Cell<u64> = const { Cell::new(0) };
        pub static BANCO: Cell<u64> = const { Cell::new(0) };
    }
}

/// Como abrir um backup.
pub enum Como<'a> {
    /// A chave do arquivo (logo depois de fazer, para conferir).
    Chave([u8; 32]),
    /// Senha digitada: de um gerente (formato 3) ou do backup (antigos),
    /// e os cadeados dos gerentes de agora deste caixa.
    Senha { senha: &'a str, acessos: &'a [Vec<u8>] },
}

/// Tira o zip de dentro do backup e grava em `destino`.
fn desembrulhar(arquivo: &Path, destino: &mut impl Write, como: &Como) -> Resultado<()> {
    let ler_falha = |e: std::io::Error| Erro::validacao(format!("Não deu para ler o backup: {e}"));
    let gravar_falha = |e: std::io::Error| Erro::validacao(format!("Não deu para abrir o backup: {e}"));
    let mut f = std::fs::File::open(arquivo).map_err(ler_falha)?;
    let mut cabeca = [0u8; 9];
    if ler_cheio(&mut f, &mut cabeca).map_err(ler_falha)? < 9 || tem_senha(&cabeca).is_none() {
        return Err(Erro::validacao("Este arquivo não é um backup do Rustinho."));
    }
    let tipo = cabeca[8];
    if tipo == 0 {
        std::io::copy(&mut f, destino).map_err(gravar_falha)?;
        return Ok(());
    }
    let incompleto = || Erro::validacao("O backup está incompleto (o arquivo foi cortado).");
    let (senha, acessos_aqui) = match como {
        Como::Senha { senha, acessos } => (*senha, *acessos),
        Como::Chave(_) => ("", &[][..]),
    };
    if tipo == 3 {
        let mut fixo = [0u8; 32 + 32 + 7 + 1];
        if ler_cheio(&mut f, &mut fixo).map_err(ler_falha)? < fixo.len() {
            return Err(incompleto());
        }
        let publica: [u8; 32] = fixo[..32].try_into().unwrap_or_default();
        let efemera: [u8; 32] = fixo[32..64].try_into().unwrap_or_default();
        let comeco: [u8; 7] = fixo[64..71].try_into().unwrap_or_default();
        let mut no_arquivo = vec![0u8; fixo[71] as usize * ACESSO];
        if ler_cheio(&mut f, &mut no_arquivo).map_err(ler_falha)? < no_arquivo.len() {
            return Err(incompleto());
        }
        let chave = match como {
            Como::Chave(k) => *k,
            Como::Senha { .. } => {
                if senha.is_empty() {
                    return Err(Erro::campo("senha", "Digite a senha de um gerente para abrir este backup."));
                }
                let mut tentados: Vec<&[u8]> = Vec::new();
                let segredo = no_arquivo
                    .chunks(ACESSO)
                    .chain(acessos_aqui.iter().map(|a| a.as_slice()))
                    .filter(|a| {
                        let novo = !tentados.contains(a);
                        tentados.push(a);
                        novo
                    })
                    .find_map(|a| destrancar(a, senha).filter(|s| publica_de(s) == publica))
                    .ok_or_else(senha_de_gerente_errada)?;
                chave_do_arquivo(&segredo, &efemera, &efemera, &publica)
            }
        };
        return ler_pedacos(&mut f, destino, &chave, &comeco, senha_errada);
    }
    if !matches!(tipo, 1 | 2) {
        return Err(Erro::validacao("Este backup é de uma versão mais nova do Rustinho. Atualize o Rustinho antes."));
    }
    // Formatos antigos (até a 0.9.11): a senha do backup de então.
    let mut sal = [0u8; 16];
    if ler_cheio(&mut f, &mut sal).map_err(ler_falha)? < 16 {
        return Err(incompleto());
    }
    if senha.is_empty() {
        return Err(Erro::campo("senha", "Este backup é de antes da 0.9.11: digite a senha do backup daquela época."));
    }
    let chave = chave_da_senha(senha, &sal)?;
    if tipo == 1 {
        // Formato antigo: lido inteiro na memória, então tem tamanho máximo.
        let tamanho = std::fs::metadata(arquivo).map(|m| m.len()).unwrap_or(u64::MAX);
        if tamanho > limite_formato_antigo() {
            return Err(Erro::validacao(
                "Este backup antigo é grande demais para abrir neste aparelho. Abra num computador.",
            ));
        }
        let cifra = ChaCha20Poly1305::new_from_slice(&chave).map_err(|_| Erro::Interno("chave inválida".into()))?;
        let mut nonce = [0u8; 12];
        if ler_cheio(&mut f, &mut nonce).map_err(ler_falha)? < 12 {
            return Err(incompleto());
        }
        let mut corpo = Vec::new();
        f.read_to_end(&mut corpo).map_err(ler_falha)?;
        let claro = cifra.decrypt(&nonce.into(), corpo.as_slice()).map_err(|_| senha_errada())?;
        destino.write_all(&claro).map_err(gravar_falha)?;
        return Ok(());
    }
    let mut comeco = [0u8; 7];
    if ler_cheio(&mut f, &mut comeco).map_err(ler_falha)? < 7 {
        return Err(incompleto());
    }
    ler_pedacos(&mut f, destino, &chave, &comeco, senha_errada)
}

/// Tipo de um arquivo de backup (0 = sem criptografia).
fn tipo_do_arquivo(arquivo: &Path) -> Option<u8> {
    let mut cabeca = [0u8; 9];
    std::fs::File::open(arquivo).ok()?.read_exact(&mut cabeca).ok()?;
    tem_senha(&cabeca).map(|_| cabeca[8])
}

// ---------------------------------------------------------------------------
// Fazer e conferir.
// ---------------------------------------------------------------------------

fn abrir_conexao(caminho_db: &Path) -> Resultado<Connection> {
    let conn = Connection::open(caminho_db)?;
    conn.busy_timeout(Duration::from_secs(20))?;
    banco::registrar_funcoes(&conn)?;
    Ok(conn)
}

/// Pasta temporária própria, apagada ao sair.
struct Temporaria(PathBuf);

impl Temporaria {
    fn nova(perto_de: &Path) -> Resultado<Temporaria> {
        let base = if perto_de.is_dir() { perto_de.to_path_buf() } else { std::env::temp_dir() };
        let p = base.join(format!(".rustinho-tmp-{}", tempo::novo_id()));
        std::fs::create_dir_all(&p)
            .map_err(|e| Erro::validacao(format!("Não deu para usar a pasta {}: {e}", base.display())))?;
        // Aqui fica, por um instante, a cópia do banco sem criptografia.
        crate::arquivos::so_do_usuario(&p, "pasta temporária do backup");
        Ok(Temporaria(p))
    }
}

impl Drop for Temporaria {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Informações guardadas dentro do backup.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Info {
    pub versao: String,
    pub estrutura: i64,
    pub loja: String,
    pub caixa: String,
    pub dispositivo: String,
    pub quando: String,
}

/// Cópia pronta.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Feito {
    pub arquivo: String,
    pub tamanho: i64,
}

fn erro_pasta(pasta: &Path, e: std::io::Error) -> Erro {
    use std::io::ErrorKind as K;
    let motivo = match e.kind() {
        K::PermissionDenied => "sem permissão para gravar lá".to_string(),
        K::NotFound => "a pasta não existe (o pen drive ou HD foi tirado?)".to_string(),
        K::StorageFull => "o disco está cheio".to_string(),
        K::ReadOnlyFilesystem => "o disco só deixa ler".to_string(),
        K::NotADirectory | K::AlreadyExists => "no lugar da pasta tem um arquivo".to_string(),
        _ => e.to_string(),
    };
    Erro::validacao(format!("Não deu para gravar na pasta {}: {motivo}.", pasta.display()))
}

/// Faz uma cópia agora (no computador). `antes` = cópia de antes de restaurar.
pub fn fazer(caminho_db: &Path, antes: bool) -> Resultado<Feito> {
    let conn = abrir_conexao(caminho_db)?;
    let r = fazer_com(&conn, caminho_db, antes);
    anotar(&conn, &r)?;
    r
}

fn anotar(conn: &Connection, r: &Resultado<Feito>) -> Resultado<()> {
    match r {
        Ok(f) => {
            config::gravar(conn, "backup_ultimo", &tempo::agora())?;
            config::gravar(conn, "backup_ultimo_tamanho", &f.tamanho.to_string())?;
            config::gravar(conn, "backup_ultimo_arquivo", &f.arquivo)?;
            config::gravar(conn, "backup_erro", "")?;
        }
        Err(e) => {
            config::gravar(conn, "backup_erro", &e.to_string())?;
            config::gravar(conn, "backup_erro_quando", &tempo::agora())?;
        }
    }
    Ok(())
}

fn fazer_com(conn: &Connection, caminho_db: &Path, antes: bool) -> Resultado<Feito> {
    let a = ajustes(conn, Some(caminho_db))?;
    std::fs::create_dir_all(&a.pasta).map_err(|e| erro_pasta(&a.pasta, e))?;
    // Restos de uma cópia interrompida (Rustinho fechado no meio).
    if let Ok(lista) = std::fs::read_dir(&a.pasta) {
        for e in lista.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with(".rustinho-tmp-") {
                let _ = std::fs::remove_dir_all(e.path());
            } else if n.starts_with(".rustinho-") && n.ends_with(".parcial") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let tmp = Temporaria::nova(&a.pasta)?;

    // 1. Foto do banco.
    let foto = tmp.0.join("rustinho.db");
    conn.execute("VACUUM INTO ?1", [foto.display().to_string()])?;
    let estrutura: i64 = conn.query_row("PRAGMA user_version", [], |l| l.get(0))?;
    let info = Info {
        versao: crate::VERSAO.into(),
        estrutura,
        loja: config::nome_loja(conn)?,
        caixa: config::nome_caixa(conn)?,
        dispositivo: config::ler(conn, "dispositivo_id")?.unwrap_or_default(),
        quando: tempo::agora(),
    };

    // 2. Zip (num arquivo, não na memória).
    let caminho_zip = tmp.0.join("backup.zip");
    let arquivo_zip = std::fs::File::create(&caminho_zip).map_err(|e| erro_pasta(&a.pasta, e))?;
    let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(arquivo_zip));
    let opcoes =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(true);
    let erro_zip = |e: zip::result::ZipError| Erro::Interno(format!("falha ao comprimir: {e}"));
    zip.start_file("info.json", opcoes).map_err(erro_zip)?;
    zip.write_all(serde_json::to_string_pretty(&info)?.as_bytes()).map_err(|e| erro_pasta(&a.pasta, e))?;
    zip.start_file("rustinho.db", opcoes).map_err(erro_zip)?;
    let mut f = std::fs::File::open(&foto).map_err(|e| erro_pasta(&a.pasta, e))?;
    std::io::copy(&mut f, &mut zip).map_err(|e| erro_pasta(&a.pasta, e))?;
    drop(f);
    let mut fim = zip.finish().map_err(erro_zip)?;
    fim.flush().map_err(|e| erro_pasta(&a.pasta, e))?;
    drop(fim);
    let _ = std::fs::remove_file(&foto);

    // 3. Criptografia e gravação (primeiro com outro nome, para não deixar meio arquivo).
    let acessos = acessos_da_loja(conn)?;
    let nome = montar_nome(conn, antes)?;
    let destino = a.pasta.join(&nome);
    let parcial = a.pasta.join(format!(".{nome}.parcial"));
    let chave_do_arquivo;
    {
        let saida = std::fs::File::create(&parcial).map_err(|e| erro_pasta(&a.pasta, e))?;
        crate::arquivos::so_do_usuario(&parcial, "backup");
        let mut escrita = std::io::BufWriter::new(saida);
        chave_do_arquivo = embrulhar(&caminho_zip, &mut escrita, a.publica.as_ref(), &acessos)?;
        let saida = escrita.into_inner().map_err(|e| erro_pasta(&a.pasta, e.into_error()))?;
        saida.sync_all().map_err(|e| erro_pasta(&a.pasta, e))?;
    }
    std::fs::rename(&parcial, &destino).map_err(|e| erro_pasta(&a.pasta, e))?;
    let tamanho = std::fs::metadata(&destino).map(|m| m.len() as i64).unwrap_or(0);

    // 4. Conferir lendo de volta do disco.
    let como = match chave_do_arquivo {
        Some(k) => Como::Chave(k),
        None => Como::Senha { senha: "", acessos: &[] },
    };
    if let Err(e) = conferir_com(&destino, &como) {
        let _ = std::fs::remove_file(&destino);
        return Err(Erro::validacao(format!("A cópia foi gravada mas não abriu ({e}). Ela foi apagada.")));
    }

    // 5. Apagar as antigas.
    let hoje: String = conn.query_row("SELECT strftime('%Y%m%d', 'now', 'localtime')", [], |l| l.get(0))?;
    for velho in para_apagar(&nomes_na_pasta(&a.pasta), &hoje, a.dias) {
        let _ = std::fs::remove_file(a.pasta.join(velho));
    }

    Ok(Feito { arquivo: destino.display().to_string(), tamanho })
}

fn nomes_na_pasta(pasta: &Path) -> Vec<String> {
    std::fs::read_dir(pasta)
        .map(|l| {
            l.filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| ler_nome(n).is_some())
                .collect()
        })
        .unwrap_or_default()
}

/// Abre o backup numa pasta temporária e confere. Devolve o caminho do
/// banco tirado de dentro (vale enquanto `tmp` existir) e as informações.
fn abrir(arquivo: &Path, como: &Como, tmp: &Temporaria) -> Resultado<(PathBuf, Info)> {
    let caminho_zip = tmp.0.join("aberto.zip");
    {
        let saida = std::fs::File::create(&caminho_zip)
            .map_err(|e| Erro::validacao(format!("Não deu para abrir o backup: {e}")))?;
        let mut escrita = std::io::BufWriter::new(saida);
        desembrulhar(arquivo, &mut escrita, como)?;
        escrita.flush().map_err(|e| Erro::validacao(format!("Não deu para abrir o backup: {e}")))?;
    }
    let arquivo_zip =
        std::fs::File::open(&caminho_zip).map_err(|e| Erro::validacao(format!("Não deu para abrir o backup: {e}")))?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(arquivo_zip))
        .map_err(|_| Erro::validacao("O backup está estragado (o zip não abre)."))?;
    let mut info = Info::default();
    if let Ok(mut f) = zip.by_name("info.json") {
        let mut t = String::new();
        f.read_to_string(&mut t).map_err(|_| Erro::validacao("O backup está estragado."))?;
        info = serde_json::from_str(&t).unwrap_or_default();
    }
    let db = tmp.0.join("restaurar.db");
    {
        let f = zip.by_name("rustinho.db").map_err(|_| Erro::validacao("O backup não tem o banco de dados."))?;
        // O tamanho escrito no zip pode mentir: o limite vale também ao descomprimir.
        let limite = limite_do_banco();
        if f.size() > limite {
            return Err(Erro::validacao("O banco dentro do backup é grande demais para abrir neste aparelho."));
        }
        let mut saida =
            std::fs::File::create(&db).map_err(|e| Erro::validacao(format!("Não deu para abrir o backup: {e}")))?;
        let copiado = std::io::copy(&mut f.take(limite + 1), &mut saida)
            .map_err(|_| Erro::validacao("O backup está estragado (falha ao descomprimir)."))?;
        if copiado > limite {
            return Err(Erro::validacao("O banco dentro do backup é grande demais para abrir neste aparelho."));
        }
    }
    let c = Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let ok: String = c
        .query_row("PRAGMA quick_check", [], |l| l.get(0))
        .map_err(|_| Erro::validacao("O banco dentro do backup está estragado."))?;
    if ok != "ok" {
        return Err(Erro::validacao("O banco dentro do backup está estragado."));
    }
    let estrutura: i64 = c.query_row("PRAGMA user_version", [], |l| l.get(0))?;
    let tem_config: Option<i64> = c
        .query_row("SELECT COUNT(*) FROM config", [], |l| l.get(0))
        .optional()
        .map_err(|_| Erro::validacao("O arquivo não tem os dados do Rustinho."))?;
    if estrutura < 1 || tem_config.is_none() {
        return Err(Erro::validacao("O arquivo não tem os dados do Rustinho."));
    }
    info.estrutura = estrutura;
    Ok((db, info))
}

/// Maior banco dentro de um backup.
#[cfg(target_os = "android")]
const TAMANHO_BANCO: u64 = 1024 * 1024 * 1024;
#[cfg(not(target_os = "android"))]
const TAMANHO_BANCO: u64 = 16 * 1024 * 1024 * 1024;

/// Confere se um backup abre com a senha de um gerente (ou, nos antigos, a
/// senha do backup). Devolve as informações de dentro dele.
pub fn conferir(conn: &Connection, arquivo: &Path, senha: &str) -> Resultado<Info> {
    let acessos = acessos_da_loja(conn)?;
    conferir_com(arquivo, &Como::Senha { senha, acessos: &acessos })
}

fn conferir_com(arquivo: &Path, como: &Como) -> Resultado<Info> {
    let tmp = Temporaria::nova(arquivo.parent().unwrap_or(Path::new(".")))?;
    abrir(arquivo, como, &tmp).map(|(_, i)| i)
}

/// Cópias que estão na pasta, mais novas primeiro.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Copia {
    pub nome: String,
    pub caminho: String,
    /// "15/10/2026 18:30"
    pub quando: String,
    pub caixa: String,
    pub tamanho: i64,
    pub tem_senha: bool,
    pub antes: bool,
    /// Está no Google Drive (e não neste computador).
    pub nuvem: bool,
}

fn quando_de(b: &NomeBackup) -> String {
    format!("{}/{}/{} {}:{}", &b.dia[6..8], &b.dia[4..6], &b.dia[..4], &b.hora[..2], &b.hora[2..4])
}

pub fn listar(pasta: &Path) -> Vec<Copia> {
    let mut lista: Vec<(NomeBackup, Copia)> = nomes_na_pasta(pasta)
        .into_iter()
        .filter_map(|n| {
            let b = ler_nome(&n)?;
            let caminho = pasta.join(&n);
            let tamanho = std::fs::metadata(&caminho).ok()?.len() as i64;
            let mut cabeca = [0u8; 9];
            let senha = std::fs::File::open(&caminho)
                .ok()
                .and_then(|mut f| f.read_exact(&mut cabeca).ok())
                .and_then(|_| tem_senha(&cabeca))?;
            let c = Copia {
                nome: n,
                caminho: caminho.display().to_string(),
                quando: quando_de(&b),
                caixa: b.caixa.clone(),
                tamanho,
                tem_senha: senha,
                antes: b.antes,
                nuvem: false,
            };
            Some((b, c))
        })
        .collect();
    lista.sort_by(|a, b| (&b.0.dia, &b.0.hora).cmp(&(&a.0.dia, &a.0.hora)));
    lista.into_iter().map(|x| x.1).collect()
}

// ---------------------------------------------------------------------------
// Automático.
// ---------------------------------------------------------------------------

/// Está na hora do backup do dia? (Ainda não teve hoje e já passou do
/// horário, ou ficou um dia inteiro sem.)
pub fn esta_na_hora(conn: &Connection, caminho_db: Option<&Path>) -> Resultado<bool> {
    let a = ajustes(conn, caminho_db)?;
    let s = situacao(conn)?;
    if s.ultimo.is_empty() {
        return Ok(true);
    }
    let (hoje_teve, ontem_ou_hoje, passou): (bool, bool, bool) = conn.query_row(
        "SELECT date(?1, 'localtime') = date('now', 'localtime'),
                date(?1, 'localtime') >= date('now', 'localtime', '-1 day'),
                strftime('%H:%M', 'now', 'localtime') >= ?2",
        params![s.ultimo, a.hora],
        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
    )?;
    Ok(!hoje_teve && (passou || !ontem_ou_hoje))
}

/// Faz o que estiver pendente: o backup do dia e o envio para o Google
/// Drive. Roda em segundo plano (a tela chama de tempos em tempos).
pub fn automatico(caminho_db: PathBuf) {
    std::thread::spawn(move || {
        let Some(_vez) = Vez::pegar("Fazendo o backup…") else { return };
        let Ok(conn) = abrir_conexao(&caminho_db) else { return };
        if config::ler(&conn, "backup_desde").ok().flatten().is_none() {
            let _ = config::gravar(&conn, "backup_desde", &tempo::agora());
        }
        if esta_na_hora(&conn, Some(&caminho_db)).unwrap_or(false) {
            let r = fazer_com(&conn, &caminho_db, false);
            let _ = anotar(&conn, &r);
        }
        let _ = enviar_pendentes(&conn, &caminho_db);
    });
}

/// "Fazer backup agora": cópia no computador e envio para o Drive.
pub fn agora(caminho_db: PathBuf) -> Resultado<()> {
    let vez = Vez::pegar("Fazendo o backup…")
        .ok_or_else(|| Erro::validacao("Já tem um backup em andamento. Espere terminar."))?;
    std::thread::spawn(move || {
        let _vez = vez;
        let Ok(conn) = abrir_conexao(&caminho_db) else { return };
        let r = fazer_com(&conn, &caminho_db, false);
        let _ = anotar(&conn, &r);
        if r.is_ok() {
            let _ = enviar_pendentes(&conn, &caminho_db);
        }
    });
    Ok(())
}

/// Ao fechar o Rustinho: cópia no computador (espera terminar), se a última
/// tem mais de meia hora. O Drive fica para a próxima vez que abrir.
pub fn ao_fechar(caminho_db: &Path) -> Resultado<bool> {
    // Se um backup está no meio, espera ele acabar (até 2 minutos).
    let inicio = Instant::now();
    let _vez = loop {
        if let Some(v) = Vez::pegar("Fazendo o backup antes de fechar…") {
            break v;
        }
        if inicio.elapsed() > Duration::from_secs(120) {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    let conn = abrir_conexao(caminho_db)?;
    let s = situacao(&conn)?;
    let recente: bool = !s.ultimo.is_empty()
        && conn
            .query_row("SELECT ?1 > strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-30 minutes')", [&s.ultimo], |l| l.get(0))?;
    if recente {
        return Ok(false);
    }
    let r = fazer_com(&conn, caminho_db, false);
    anotar(&conn, &r)?;
    r.map(|_| true)
}

// ---------------------------------------------------------------------------
// Google Drive pelo rclone.
// ---------------------------------------------------------------------------

/// O rclone que vem junto com o Rustinho (instalador do Windows, AppImage)
/// ou, se não vier, o do sistema.
fn programa_rclone() -> String {
    if let Some(p) = std::env::var("RUSTINHO_RCLONE").ok().filter(|p| !p.is_empty()) {
        return p;
    }
    let nome = if cfg!(windows) { "rclone.exe" } else { "rclone" };
    if let Some(junto) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join(nome))) {
        if junto.is_file() {
            return junto.display().to_string();
        }
    }
    "rclone".into()
}

/// Comando do rclone sem abrir janela preta no Windows.
fn comando_rclone() -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(programa_rclone());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const SEM_JANELA: u32 = 0x0800_0000;
        c.creation_flags(SEM_JANELA);
    }
    c
}

/// Onde ficam as cópias na nuvem ("rustinho-drive:Rustinho Backup").
/// `RUSTINHO_REMOTO` troca (testes).
fn base_nuvem() -> String {
    std::env::var("RUSTINHO_REMOTO").ok().filter(|p| !p.is_empty()).unwrap_or_else(|| format!("{REMOTO}:{PASTA_NUVEM}"))
}

/// Roda o rclone esperando no máximo `limite`. Devolve a saída.
fn rclone(args: &[&str], limite: Duration) -> Resultado<String> {
    let mut filho = comando_rclone()
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| Erro::validacao(falta_rclone()))?;
    let inicio = Instant::now();
    // Lê a saída em outra linha para o cano não encher.
    let mut out = filho.stdout.take();
    let mut err = filho.stderr.take();
    let leitor = std::thread::spawn(move || {
        let mut s = String::new();
        let mut e = String::new();
        if let Some(o) = out.as_mut() {
            let _ = o.read_to_string(&mut s);
        }
        if let Some(x) = err.as_mut() {
            let _ = x.read_to_string(&mut e);
        }
        (s, e)
    });
    let estado = loop {
        if let Some(st) = filho.try_wait().map_err(|e| Erro::Interno(format!("rclone: {e}")))? {
            break st;
        }
        if inicio.elapsed() > limite {
            let _ = filho.kill();
            let _ = filho.wait();
            return Err(Erro::validacao("O Google Drive demorou demais para responder. Confira a internet."));
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let (saida, erro) = leitor.join().unwrap_or_default();
    if estado.success() {
        Ok(saida)
    } else {
        Err(Erro::validacao(explicar_rclone(&erro)))
    }
}

fn explicar_rclone(erro: &str) -> String {
    let e = erro.to_lowercase();
    if e.contains("didn't find section")
        || e.contains("not found in config")
        || (e.contains("couldn't find") && e.contains("remote"))
    {
        "O Google Drive não está conectado neste computador.".into()
    } else if e.contains("token") && (e.contains("expired") || e.contains("invalid") || e.contains("revoked")) {
        "A conexão com o Google Drive venceu. Clique em \"Conectar ao Google Drive\" de novo.".into()
    } else if e.contains("quota") || (e.contains("storage") && e.contains("exceeded")) {
        "O Google Drive está cheio.".into()
    } else if e.contains("no such host") || e.contains("dial tcp") || e.contains("network") || e.contains("timeout") {
        "Sem internet para falar com o Google Drive.".into()
    } else {
        let ultima = erro.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("erro desconhecido");
        format!("O Google Drive recusou: {}", ultima.trim().chars().take(160).collect::<String>())
    }
}

fn falta_rclone() -> &'static str {
    if cfg!(windows) {
        "Falta o programa rclone, que vem com o instalador. Instale o Rustinho de novo."
    } else {
        "Falta instalar o programa rclone. No terminal: sudo pacman -S rclone"
    }
}

/// O rclone está instalado?
pub fn rclone_instalado() -> bool {
    comando_rclone().arg("version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

/// O Google Drive já foi conectado neste computador?
pub fn drive_conectado() -> bool {
    if std::env::var("RUSTINHO_REMOTO").is_ok_and(|r| !r.is_empty()) {
        return true;
    }
    rclone(&["listremotes"], Duration::from_secs(15))
        .map(|s| s.lines().any(|l| l.trim() == format!("{REMOTO}:")))
        .unwrap_or(false)
}

/// Abre o navegador para a pessoa entrar na conta do Google. Espera até 5
/// minutos. Só dá acesso aos arquivos que o próprio Rustinho criar.
pub fn conectar_drive(caminho_db: PathBuf) -> Resultado<()> {
    if !rclone_instalado() {
        return Err(Erro::validacao(falta_rclone()));
    }
    let vez = Vez::pegar("Esperando você entrar na conta do Google no navegador…")
        .ok_or_else(|| Erro::validacao("Já tem um backup em andamento. Espere terminar."))?;
    std::thread::spawn(move || {
        let _vez = vez;
        let Ok(conn) = abrir_conexao(&caminho_db) else { return };
        let r = if drive_conectado() {
            Ok(String::new())
        } else {
            rclone(&["config", "create", REMOTO, "drive", "scope=drive.file"], Duration::from_secs(300))
        };
        match r.and_then(|_| rclone(&["mkdir", &base_nuvem()], Duration::from_secs(60))) {
            Ok(_) => {
                let _ = config::gravar(&conn, "backup_nuvem", "1");
                let _ = config::gravar(&conn, "backup_nuvem_desde", &tempo::agora());
                let _ = config::gravar(&conn, "backup_nuvem_erro", "");
                dizer("Enviando para o Google Drive…");
                let _ = enviar_pendentes(&conn, &caminho_db);
            }
            Err(e) => {
                let _ = config::gravar(&conn, "backup_nuvem_erro", &e.to_string());
            }
        }
    });
    Ok(())
}

/// Para de mandar para o Drive (as cópias que já estão lá ficam).
pub fn desligar_drive(conn: &Connection) -> Resultado<()> {
    config::gravar(conn, "backup_nuvem", "0")?;
    config::gravar(conn, "backup_nuvem_erro", "")?;
    Ok(())
}

fn nomes_na_nuvem() -> Resultado<Vec<(String, i64)>> {
    match rclone(&["lsf", "--files-only", "--format", "ps", "--separator", "|", &base_nuvem()], Duration::from_secs(60))
    {
        Ok(s) => Ok(s
            .lines()
            .filter_map(|l| {
                let (n, t) = l.split_once('|')?;
                ler_nome(n)?;
                Some((n.to_string(), t.trim().parse().unwrap_or(0)))
            })
            .collect()),
        Err(e) if e.to_string().contains("directory not found") => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

/// Manda para o Drive a cópia mais nova que ainda não está lá e apaga as
/// antigas de lá. Só cópias com senha vão para a nuvem.
fn enviar_pendentes(conn: &Connection, caminho_db: &Path) -> Resultado<()> {
    let a = ajustes(conn, Some(caminho_db))?;
    if !a.nuvem {
        return Ok(());
    }
    let r = (|| -> Resultado<()> {
        let mais_nova = listar(&a.pasta).into_iter().find(|c| !c.antes);
        let Some(c) = mais_nova else { return Ok(()) };
        if !c.tem_senha {
            return Err(Erro::validacao(
                "Um gerente precisa entrar no Rustinho antes: sem a chave da loja nada vai para o Google Drive.",
            ));
        }
        let la = nomes_na_nuvem()?;
        if !la.iter().any(|(n, _)| *n == c.nome) {
            dizer("Enviando para o Google Drive…");
            rclone(
                &["copyto", "--retries", "3", &c.caminho, &format!("{}/{}", base_nuvem(), c.nome)],
                Duration::from_secs(1800),
            )?;
        }
        let hoje: String = conn.query_row("SELECT strftime('%Y%m%d', 'now', 'localtime')", [], |l| l.get(0))?;
        let mut nomes: Vec<String> = la.into_iter().map(|x| x.0).collect();
        if !nomes.contains(&c.nome) {
            nomes.push(c.nome.clone());
        }
        for velho in para_apagar(&nomes, &hoje, a.dias) {
            let _ = rclone(&["deletefile", &format!("{}/{}", base_nuvem(), velho)], Duration::from_secs(60));
        }
        Ok(())
    })();
    match &r {
        Ok(()) => {
            config::gravar(conn, "backup_nuvem_ultimo", &tempo::agora())?;
            config::gravar(conn, "backup_nuvem_erro", "")?;
        }
        Err(e) => config::gravar(conn, "backup_nuvem_erro", &e.to_string())?,
    }
    r
}

/// Cópias que estão no Google Drive, mais novas primeiro.
pub fn listar_nuvem() -> Resultado<Vec<Copia>> {
    let mut lista: Vec<(NomeBackup, Copia)> = nomes_na_nuvem()?
        .into_iter()
        .filter_map(|(n, tamanho)| {
            let b = ler_nome(&n)?;
            let c = Copia {
                nome: n,
                caminho: String::new(),
                quando: quando_de(&b),
                caixa: b.caixa.clone(),
                tamanho,
                tem_senha: true,
                antes: b.antes,
                nuvem: true,
            };
            Some((b, c))
        })
        .collect();
    lista.sort_by(|a, b| (&b.0.dia, &b.0.hora).cmp(&(&a.0.dia, &a.0.hora)));
    Ok(lista.into_iter().map(|x| x.1).collect())
}

// ---------------------------------------------------------------------------
// Restaurar.
// ---------------------------------------------------------------------------

/// Volta o banco para o que está no backup. `arquivo` é o caminho no
/// computador ou, com `nuvem`, o nome do arquivo no Google Drive.
/// Antes, guarda uma cópia do que existe agora (`caminho_db`, se houver).
pub fn restaurar(
    conn: &mut Connection,
    caminho_db: Option<&Path>,
    arquivo: &str,
    nuvem: bool,
    senha: &str,
) -> Resultado<Info> {
    let _vez = Vez::pegar("Restaurando…")
        .ok_or_else(|| Erro::validacao("Tem um backup em andamento. Espere terminar e tente de novo."))?;
    // A troca de dados com os outros caixas espera a restauração terminar.
    let _pausa = sync::pausar();
    let tmp = Temporaria::nova(&std::env::temp_dir())?;

    let local = if nuvem {
        if ler_nome(arquivo).is_none() {
            return Err(Erro::validacao("Esse arquivo não é um backup do Rustinho."));
        }
        dizer("Baixando do Google Drive…");
        let destino = tmp.0.join(arquivo);
        rclone(
            &["copyto", &format!("{}/{arquivo}", base_nuvem()), &destino.display().to_string()],
            Duration::from_secs(1800),
        )?;
        destino
    } else {
        PathBuf::from(arquivo)
    };

    dizer("Conferindo o backup…");
    // Backup sem criptografia (de antes de ter gerente): mesmo assim só com
    // a senha de um gerente deste caixa.
    if tipo_do_arquivo(&local) == Some(0)
        && crate::usuarios::quantidade(conn)? > 0
        && crate::usuarios::conferir_gerente(conn, senha)?.is_none()
    {
        return Err(Erro::campo("senha", "Digite a senha de um gerente para restaurar."));
    }
    let acessos = acessos_da_loja(conn)?;
    let (db, info) = abrir(&local, &Como::Senha { senha, acessos: &acessos }, &tmp)?;
    if info.estrutura as usize > banco::versao_da_estrutura() {
        return Err(Erro::validacao("Esse backup é de uma versão mais nova do Rustinho. Atualize o Rustinho antes."));
    }

    // Cópia do que existe agora, por segurança.
    if let Some(c) = caminho_db {
        dizer("Guardando uma cópia de agora…");
        let r = fazer_com(conn, c, true);
        anotar(conn, &r)?;
        r.map_err(|e| Erro::validacao(format!("Não deu para guardar a cópia de antes, então nada foi mudado. {e}")))?;
    }

    // O que é deste computador fica como está.
    let da_loja: Vec<&str> = sync::CONFIG_DA_LOJA.to_vec();
    let mut locais: Vec<(String, String)> = Vec::new();
    {
        let mut c = conn.prepare("SELECT chave, valor FROM config")?;
        for l in c.query_map([], |l| Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?)))? {
            let (k, v) = l?;
            if !da_loja.contains(&k.as_str()) {
                locais.push((k, v));
            }
        }
    }
    let pares: Vec<(String, String, String)> = {
        let mut c = conn.prepare("SELECT dispositivo, nome, endereco FROM sync_par")?;
        let l = c.query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?;
        l.collect::<Result<_, _>>()?
    };
    let seq: i64 = conn
        .query_row("SELECT seq FROM sqlite_sequence WHERE name = 'alteracao'", [], |l| l.get(0))
        .optional()?
        .unwrap_or(0);

    dizer("Restaurando…");
    conn.restore(rusqlite::MAIN_DB, &db, None::<fn(rusqlite::backup::Progress)>)?;
    banco::migrar(conn)?;
    sync::criar_gatilhos(conn)?;

    let tx = conn.transaction()?;
    tx.execute("UPDATE sync_estado SET aplicando = 1 WHERE id = 1", [])?;
    let lista_loja = da_loja.iter().map(|c| format!("'{c}'")).collect::<Vec<_>>().join(", ");
    tx.execute(&format!("DELETE FROM config WHERE chave NOT IN ({lista_loja})"), [])?;
    tx.execute("UPDATE sync_estado SET aplicando = 0 WHERE id = 1", [])?;
    for (k, v) in &locais {
        config::gravar(&tx, k, v)?;
    }
    // Recebe tudo de novo dos outros caixas (fica com o mais novo de cada registro).
    tx.execute("DELETE FROM sync_par", [])?;
    for (d, n, e) in &pares {
        tx.execute(
            "INSERT INTO sync_par (dispositivo, nome, endereco, cursor) VALUES (?1, ?2, ?3, 0)",
            params![d, n, e],
        )?;
    }
    // A numeração das alterações deste caixa continua de onde estava; este
    // caixa recomeça do zero com os outros e ganha geração nova (os outros
    // também recomeçam com ele).
    sync::depois_de_restaurar(&tx, seq)?;
    tx.commit()?;
    Ok(info)
}

/// Resumo para a tela.
pub fn resumo(conn: &Connection, caminho_db: Option<&Path>, com_drive: bool) -> Resultado<Value> {
    let a = ajustes(conn, caminho_db)?;
    let s = situacao(conn)?;
    let (rclone_ok, drive) =
        if com_drive { (rclone_instalado(), a.nuvem && drive_conectado()) } else { (false, false) };
    Ok(json!({
        "pasta": a.pasta.display().to_string(),
        "pastaPadrao": pasta_padrao(caminho_db).display().to_string(),
        "dias": a.dias.to_string(),
        "hora": a.hora,
        "temSenha": a.publica.is_some(),
        "nuvem": a.nuvem,
        "rclone": rclone_ok,
        "drive": drive,
        "situacao": s,
        "ocupado": ocupado(),
        "fazendo": fazendo(),
        "lista": listar(&a.pasta),
        "sincronizacao": sync::ajustes(conn)?.ligado,
    }))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;

    /// Restaurar usa a vez única do backup: um teste desses de cada vez.
    static UM_POR_VEZ: Mutex<()> = Mutex::new(());

    fn pasta_teste(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("rustinho-teste-backup-{nome}-{}", tempo::novo_id()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn nomes() {
        let b = ler_nome("rustinho-20261015-183005-caixa1.rbk").unwrap();
        assert_eq!(
            (b.dia.as_str(), b.hora.as_str(), b.caixa.as_str(), b.antes),
            ("20261015", "183005", "caixa1", false)
        );
        assert!(ler_nome("rustinho-20261015-183005-caixa1-antes.rbk").unwrap().antes);
        assert!(ler_nome("rustinho-2026101-183005-caixa1.rbk").is_none());
        assert!(ler_nome("foto.rbk").is_none());
        assert!(ler_nome("rustinho-20261015-183005-.rbk").is_none());
        assert_eq!(dia_menos("20261001", 1), "20260930");
        assert!(hora_valida("18:00") && !hora_valida("24:00") && !hora_valida("8:00"));
    }

    #[test]
    fn quais_apagar() {
        let n = |d: &str, h: &str, c: &str| format!("rustinho-{d}-{h}-{c}.rbk");
        let nomes = vec![
            n("20261015", "090000", "caixa1"),
            n("20261015", "180000", "caixa1"),
            n("20261015", "190000", "caixa1"),
            n("20261015", "200000", "caixa1"),
            n("20261014", "100000", "caixa1"),
            n("20261014", "180000", "caixa1"),
            n("20261001", "180000", "caixa1"),
            n("20260901", "180000", "caixa1"),
            n("20260901", "180000", "caixa2"),
            "outro-arquivo.txt".to_string(),
        ];
        let mut apagar = para_apagar(&nomes, "20261015", 30);
        apagar.sort();
        // Hoje: todos ficam. Dia 14: só o mais novo. Dia 01/09: passou de 30
        // dias. caixa2 tem uma só cópia: fica (as 3 mais novas sempre ficam).
        assert_eq!(apagar, vec![n("20260901", "180000", "caixa1"), n("20261014", "100000", "caixa1")]);
    }

    fn embrulhar_bytes(dados: &[u8], publica: Option<&[u8; 32]>, acessos: &[Vec<u8>]) -> (Vec<u8>, Option<[u8; 32]>) {
        let pasta = pasta_teste("embrulho");
        let origem = pasta.join("zip");
        std::fs::write(&origem, dados).unwrap();
        let mut saida = Vec::new();
        let k = embrulhar(&origem, &mut saida, publica, acessos).unwrap();
        let _ = std::fs::remove_dir_all(&pasta);
        (saida, k)
    }

    fn desembrulhar_bytes(bytes: &[u8], como: &Como) -> Resultado<Vec<u8>> {
        let pasta = pasta_teste("desembrulho");
        let arquivo = pasta.join("b.rbk");
        std::fs::write(&arquivo, bytes).unwrap();
        let mut saida = Vec::new();
        let r = desembrulhar(&arquivo, &mut saida, como).map(|_| saida);
        let _ = std::fs::remove_dir_all(&pasta);
        r
    }

    fn senha<'a>(s: &'a str, acessos: &'a [Vec<u8>]) -> Como<'a> {
        Como::Senha { senha: s, acessos }
    }

    /// C-03: o arquivo abre com a senha de um gerente (pelo cadeado que vai
    /// junto ou pelo do caixa), nunca sem senha, e estrago é percebido.
    #[test]
    fn chave_da_loja_e_estrago() {
        let segredo = [5u8; 32];
        let publica = publica_de(&segredo);
        let cadeado_ana = trancar(&segredo, "senha-ana").unwrap();
        let cadeado_bia = trancar(&segredo, "senha-bia").unwrap();
        for tamanho in [20, PEDACO, PEDACO * 2 + 333] {
            let dados: Vec<u8> = (0..tamanho).map(|i| (i % 251) as u8).collect();
            let (com, k) = embrulhar_bytes(&dados, Some(&publica), std::slice::from_ref(&cadeado_ana));
            assert_eq!(tem_senha(&com), Some(true));
            assert_eq!(desembrulhar_bytes(&com, &Como::Chave(k.unwrap())).unwrap(), dados);
            // Pelo cadeado que veio no arquivo (computador novo).
            assert_eq!(desembrulhar_bytes(&com, &senha("senha-ana", &[])).unwrap(), dados);
            // Pelo cadeado de agora deste caixa (gerente novo ou senha nova).
            assert_eq!(
                desembrulhar_bytes(&com, &senha("senha-bia", std::slice::from_ref(&cadeado_bia))).unwrap(),
                dados
            );
            assert_eq!(desembrulhar_bytes(&com, &senha("senha-bia", &[])).unwrap_err().nome_do_campo(), Some("senha"));
            assert_eq!(desembrulhar_bytes(&com, &senha("", &[])).unwrap_err().nome_do_campo(), Some("senha"));
            let mut estragado = com.clone();
            let fim = estragado.len() - 3;
            estragado[fim] ^= 1;
            assert!(desembrulhar_bytes(&estragado, &senha("senha-ana", &[])).is_err());
            if tamanho > PEDACO {
                let cortado = &com[..9 + 72 + ACESSO + PEDACO + 16];
                assert!(desembrulhar_bytes(cortado, &senha("senha-ana", &[])).is_err());
            }
        }
        // O segredo nunca está no arquivo.
        let (com, _) = embrulhar_bytes(b"PK x", Some(&publica), std::slice::from_ref(&cadeado_ana));
        assert!(!com.windows(32).any(|w| w == segredo));
        // Sem chave da loja: cópia simples.
        let (sem, k) = embrulhar_bytes(b"PK conteudo", None, &[]);
        assert!(k.is_none());
        assert_eq!(desembrulhar_bytes(&sem, &senha("", &[])).unwrap(), b"PK conteudo");
        assert!(desembrulhar_bytes(b"nada a ver", &senha("", &[])).is_err());
    }

    /// Um backup (formato sem senha: magia + 0 + zip) com um "banco" de
    /// `tamanho` bytes dentro, gravado em `pasta`.
    fn backup_com_banco_de(pasta: &Path, tamanho: usize, mentir_tamanho: Option<u32>) -> PathBuf {
        use std::io::Write as _;
        let mut zip_bytes = std::io::Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut zip_bytes);
            let opcoes = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            z.start_file("rustinho.db", opcoes).unwrap();
            z.write_all(&vec![7u8; tamanho]).unwrap();
            z.finish().unwrap();
        }
        let mut zip_bytes = zip_bytes.into_inner();
        if let Some(falso) = mentir_tamanho {
            // O tamanho "descomprimido" escrito na lista do zip passa a mentir.
            let pos = zip_bytes.windows(4).position(|w| w == b"PK\x01\x02").expect("diretório central");
            zip_bytes[pos + 24..pos + 28].copy_from_slice(&falso.to_le_bytes());
        }
        let mut arquivo = MAGICA.to_vec();
        arquivo.push(0);
        arquivo.extend(zip_bytes);
        let caminho = pasta.join("grande.rbk");
        std::fs::write(&caminho, arquivo).unwrap();
        caminho
    }

    fn sem_senha() -> Como<'static> {
        Como::Senha { senha: "", acessos: &[] }
    }

    /// F-11: o banco dentro do backup que passa do limite é recusado, e o
    /// motivo é o limite (um banco do mesmo tamanho dentro do limite passa
    /// desta etapa e só falha adiante, por não ser um banco de verdade).
    #[test]
    fn f11_banco_dentro_do_backup_maior_que_o_limite_e_recusado() {
        let pasta = pasta_teste("f11-banco");
        let arquivo = backup_com_banco_de(&pasta, 5000, None);
        limites_de_teste::BANCO.with(|c| c.set(1000));
        let e = conferir_com(&arquivo, &sem_senha()).unwrap_err().to_string();
        assert!(e.contains("grande demais"), "{e}");
        // Controle: com o limite acima do tamanho, o erro é outro (não é banco).
        limites_de_teste::BANCO.with(|c| c.set(10_000));
        let e = conferir_com(&arquivo, &sem_senha()).unwrap_err().to_string();
        assert!(!e.contains("grande demais"), "o limite não deveria disparar: {e}");
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// F-11: o tamanho escrito no zip pode mentir. O limite vale também ao
    /// descomprimir (o que foi copiado de fato), não só pelo que o zip diz.
    #[test]
    fn f11_zip_que_mente_o_tamanho_nao_passa_do_limite() {
        let pasta = pasta_teste("f11-mentira");
        let arquivo = backup_com_banco_de(&pasta, 5000, Some(100));
        limites_de_teste::BANCO.with(|c| c.set(1000));
        let e = conferir_com(&arquivo, &sem_senha()).unwrap_err().to_string();
        // O que importa: não grava 5000 bytes como se coubessem em 1000.
        assert!(e.contains("grande demais"), "{e}");
        let temp = std::fs::read_dir(&pasta).unwrap().filter_map(|x| x.ok()).count();
        assert_eq!(temp, 1, "a pasta temporária foi apagada (sobrou só o backup)");
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// F-11: o formato antigo (lido inteiro na memória) acima do limite é
    /// recusado antes de ler o corpo.
    #[test]
    fn f11_formato_antigo_acima_do_limite_e_recusado_sem_ler_o_corpo() {
        let pasta = pasta_teste("f11-antigo");
        let mut b = MAGICA.to_vec();
        b.push(1);
        b.extend([3u8; 16]); // sal
        b.extend(vec![9u8; 4000]); // "corpo"
        let arquivo = pasta.join("antigo.rbk");
        std::fs::write(&arquivo, &b).unwrap();
        limites_de_teste::FORMATO_ANTIGO.with(|c| c.set(1000));
        let como = Como::Senha { senha: "qualquer", acessos: &[] };
        let e = conferir_com(&arquivo, &como).unwrap_err().to_string();
        assert!(e.contains("grande demais"), "{e}");
        // Controle: dentro do limite o erro é outro (senha errada/estragado).
        limites_de_teste::FORMATO_ANTIGO.with(|c| c.set(1_000_000));
        let e = conferir_com(&arquivo, &como).unwrap_err().to_string();
        assert!(!e.contains("grande demais"), "{e}");
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// Backups antigos (formato 2, da senha do backup) continuam abrindo com
    /// aquela senha.
    #[test]
    fn formato_antigo_abre_com_a_senha_de_entao() {
        let sal = [7u8; 16];
        let k = chave_da_senha("segredo1", &sal).unwrap();
        let comeco = [1u8; 7];
        let mut b = MAGICA.to_vec();
        b.push(2);
        b.extend(sal);
        b.extend(comeco);
        escrever_pedacos(&mut &b"PK antigo"[..], &mut b, &k, &comeco).unwrap();
        assert_eq!(desembrulhar_bytes(&b, &senha("segredo1", &[])).unwrap(), b"PK antigo");
        assert_eq!(desembrulhar_bytes(&b, &senha("errada", &[])).unwrap_err().nome_do_campo(), Some("senha"));
        assert_eq!(desembrulhar_bytes(&b, &senha("", &[])).unwrap_err().nome_do_campo(), Some("senha"));
    }

    /// A chave antiga (guardada em claro) sai do banco ao abrir.
    #[test]
    fn chave_antiga_sai_do_banco() {
        let b = Banco::em_memoria().unwrap();
        config::gravar(&b.conn, "backup_senha", "segredo1").unwrap();
        config::gravar(&b.conn, "backup_chave", "aa:bb").unwrap();
        limpar_chave_antiga(&b.conn).unwrap();
        assert!(config::ler(&b.conn, "backup_senha").unwrap().is_none());
        assert!(config::ler(&b.conn, "backup_chave").unwrap().is_none());
    }

    /// C-03: cadeados dos gerentes: o primeiro cria a chave, a senha nova
    /// passa a abrir, quem deixa de ser gerente não abre.
    #[test]
    fn cadeados_dos_gerentes() {
        let b = Banco::em_memoria().unwrap();
        let ana = crate::usuarios::criar(&b.conn, &b.dispositivo, "Ana", "1111", true).unwrap();
        crate::usuarios::definir_gerente(&b.conn, &ana.id, true).unwrap();
        let bia = crate::usuarios::criar(&b.conn, &b.dispositivo, "Bia", "2222", true).unwrap();
        crate::usuarios::definir_gerente(&b.conn, &bia.id, true).unwrap();
        assert!(publica_da_loja(&b.conn).unwrap().is_none());
        let m = gerente_conferido(&b.conn, &ana.id, "1111", None).unwrap().unwrap();
        let p = publica_da_loja(&b.conn).unwrap().unwrap();
        assert_eq!(publica_de(&m), p);
        // Ninguém mais guarda o segredo em claro.
        let tudo: String = b.conn.query_row("SELECT group_concat(valor, '|') FROM config", [], |l| l.get(0)).unwrap();
        assert!(!tudo.contains(&hex(&m)));
        // Bia entra: com o segredo na memória, ganha o cadeado dela.
        assert_eq!(gerente_conferido(&b.conn, &bia.id, "2222", Some(m)).unwrap(), Some(m));
        assert_eq!(acessos_da_loja(&b.conn).unwrap().len(), 2);
        // Sem a memória e com a senha certa: abre pelo próprio cadeado.
        assert_eq!(gerente_conferido(&b.conn, &bia.id, "2222", None).unwrap(), Some(m));
        // Senha nova: só a nova abre.
        senha_trocada(&b.conn, &bia.id, "3333", Some(m)).unwrap();
        assert_eq!(gerente_conferido(&b.conn, &bia.id, "3333", None).unwrap(), Some(m));
        // Senha trocada sem o segredo na memória: o cadeado sai (volta quando outro gerente entrar).
        senha_trocada(&b.conn, &bia.id, "4444", None).unwrap();
        assert_eq!(gerente_conferido(&b.conn, &bia.id, "4444", None).unwrap(), None);
        assert_eq!(gerente_conferido(&b.conn, &bia.id, "4444", Some(m)).unwrap(), Some(m));
        deixou_de_ser_gerente(&b.conn, &bia.id).unwrap();
        assert_eq!(acessos_da_loja(&b.conn).unwrap().len(), 1);
    }

    #[test]
    fn fazer_conferir_e_restaurar() {
        let _trava = UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        let pasta = pasta_teste("ciclo");
        let caminho = pasta.join("rustinho.db");
        let mut b = Banco::abrir(&caminho).unwrap();
        config::gravar(&b.conn, "loja_nome", "Loja Teste").unwrap();
        config::gravar(&b.conn, "caixa_nome", "Caixa 1").unwrap();
        let ana = crate::usuarios::criar(&b.conn, &b.dispositivo, "Ana", "1111", true).unwrap();
        crate::usuarios::definir_gerente(&b.conn, &ana.id, true).unwrap();
        let m = gerente_conferido(&b.conn, &ana.id, "1111", None).unwrap();
        assert!(m.is_some());
        b.conn.execute("INSERT INTO sync_par (dispositivo, nome, endereco, cursor) VALUES ('outro', 'Caixa 2', '10.0.0.2:47810', 999)", []).unwrap();

        let feito = fazer(&caminho, false).unwrap();
        let arquivo = PathBuf::from(&feito.arquivo);
        assert!(arquivo.exists());
        assert_eq!(tipo_do_arquivo(&arquivo), Some(3));
        assert_eq!(conferir(&b.conn, &arquivo, "1111").unwrap().loja, "Loja Teste");
        assert!(conferir(&b.conn, &arquivo, "").is_err(), "sem senha não abre");
        assert!(conferir(&b.conn, &arquivo, "outra").is_err());
        let s = situacao(&b.conn).unwrap();
        assert!(!s.ultimo.is_empty() && s.erro.is_empty());
        assert!(!esta_na_hora(&b.conn, Some(&caminho)).unwrap());

        // Depois do backup: nome da loja muda e este caixa ganha outro nome.
        config::gravar(&b.conn, "loja_nome", "Nome Errado").unwrap();
        config::gravar(&b.conn, "caixa_nome", "Caixa 3").unwrap();
        let seq_antes: i64 =
            b.conn.query_row("SELECT seq FROM sqlite_sequence WHERE name = 'alteracao'", [], |l| l.get(0)).unwrap();

        // Senha errada ou vazia: não muda nada.
        assert!(restaurar(&mut b.conn, Some(&caminho), &feito.arquivo, false, "errada").is_err());
        assert!(restaurar(&mut b.conn, Some(&caminho), &feito.arquivo, false, "").is_err());
        assert_eq!(config::nome_loja(&b.conn).unwrap(), "Nome Errado");

        let info = restaurar(&mut b.conn, Some(&caminho), &feito.arquivo, false, "1111").unwrap();
        assert_eq!(info.loja, "Loja Teste");
        assert_eq!(config::nome_loja(&b.conn).unwrap(), "Loja Teste");
        // O que é deste computador ficou.
        assert_eq!(config::nome_caixa(&b.conn).unwrap(), "Caixa 3");
        assert_eq!(config::ler(&b.conn, "dispositivo_id").unwrap().unwrap(), b.dispositivo);
        let cursor: i64 =
            b.conn.query_row("SELECT cursor FROM sync_par WHERE dispositivo = 'outro'", [], |l| l.get(0)).unwrap();
        assert_eq!(cursor, 0);
        let seq_depois: i64 =
            b.conn.query_row("SELECT seq FROM sqlite_sequence WHERE name = 'alteracao'", [], |l| l.get(0)).unwrap();
        assert!(seq_depois >= seq_antes);
        // E uma cópia de antes de restaurar foi guardada.
        let lista = listar(&ajustes(&b.conn, Some(&caminho)).unwrap().pasta);
        assert!(lista.iter().any(|c| c.antes));
        // O banco continua funcionando depois.
        config::gravar(&b.conn, "loja_nome", "Depois").unwrap();
        drop(b);
        let b = Banco::abrir(&caminho).unwrap();
        assert_eq!(config::nome_loja(&b.conn).unwrap(), "Depois");

        // Computador novo (outro banco, outra chave): abre com a senha da Ana de então.
        let pasta2 = pasta_teste("novo");
        let b2 = Banco::abrir(&pasta2.join("rustinho.db")).unwrap();
        assert_eq!(conferir(&b2.conn, &arquivo, "1111").unwrap().loja, "Loja Teste");
        let _ = std::fs::remove_dir_all(&pasta);
        let _ = std::fs::remove_dir_all(&pasta2);
    }

    /// d) Caixa restaurado cria peças. O backup tinha a faixa do caixa, mas
    /// o banco perdido pode ter dado códigos dela depois do backup (e as
    /// etiquetas deles podem estar na prateleira): a faixa não é mais usada
    /// por ninguém, o caixa recebe outra e nada se repete.
    #[test]
    fn caixa_restaurado_nao_reusa_codigos_da_faixa() {
        use crate::faixas::para_testes::*;
        let _trava = UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        let (mut a, u) = loja("Caixa A");
        let pasta = pasta_teste("faixa");
        let caminho = pasta.join("rustinho.db");
        let mut b = Banco::abrir(&caminho).unwrap();
        config::gravar(&b.conn, "caixa_nome", "Caixa B").unwrap();
        config::gravar(&b.conn, "codigos_espera_teste", "0").unwrap();
        pedir_faixa(&mut b, &mut a);
        assert_eq!(minhas(&b), vec![(100_000, 199_999)]);
        assert!(gerente_conferido(&b.conn, &u, "1234", None).unwrap().is_some());
        let p1 = peca(&b, &u, "P1");
        trocar(&mut a, &mut b);
        let feito = fazer(&caminho, false).unwrap();
        // Depois do backup: P2 vai para A; P3 fica só em B (e a etiqueta sai).
        let p2 = peca(&b, &u, "P2");
        trocar(&mut a, &mut b);
        let p3 = peca(&b, &u, "P3");
        assert!(crate::etiquetas::etiqueta(&b.conn, &p3).unwrap().bloqueio.is_empty());
        assert_eq!(
            (codigo(&b, &p1), codigo(&b, &p2), codigo(&b, &p3)),
            ("100000".into(), "100001".into(), "100002".into())
        );

        restaurar(&mut b.conn, Some(&caminho), &feito.arquivo, false, "1234").unwrap();
        // O backup tinha a faixa de B, mas ela ficou para trás.
        assert!(crate::faixas::faixas(&b.conn).unwrap().iter().any(|f| f.inicio == 100_000));
        assert!(minhas(&b).is_empty());
        assert!(crate::faixas::precisa(&b.conn).unwrap());
        // Ainda sem troca: a definir; e o 100002 à mão não passa.
        let p4 = peca(&b, &u, "P4");
        assert_eq!(codigo(&b, &p4), "");
        let e = peca_com(&b, &u, "À mão", "100002").unwrap_err().to_string();
        assert!(e.contains("antes de restaurar"), "{e}");
        // A troca volta: faixa nova; P2 volta com o mesmo código.
        pedir_faixa(&mut b, &mut a);
        trocar(&mut a, &mut b);
        assert_eq!(minhas(&b), vec![(200_000, 299_999)]);
        assert_eq!(codigo(&b, &p4), "200000");
        assert_eq!(codigo(&b, &p2), "100001");
        // O 100002 (da peça que se perdeu) não é de ninguém e não volta.
        for x in [&a, &b] {
            assert!(todas(x).iter().all(|(_, c)| c != "100002"));
        }
        let e = peca_com(&a, &u, "À mão", "100002").unwrap_err().to_string();
        assert!(e.contains("100000 a 199999"), "{e}");
        let p5 = peca(&b, &u, "P5");
        assert_eq!(codigo(&b, &p5), "200001");
        trocar(&mut a, &mut b);
        assert_eq!(todas(&a), todas(&b));
        assert!(codigos_repetidos(&a).is_empty());
        // Fechar e abrir de novo não muda nada.
        drop(b);
        let b = Banco::abrir(&caminho).unwrap();
        assert_eq!(minhas(&b), vec![(200_000, 299_999)]);
        let p6 = peca(&b, &u, "P6");
        assert_eq!(codigo(&b, &p6), "200002");
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn pasta_que_nao_da() {
        let pasta = pasta_teste("ruim");
        let caminho = pasta.join("rustinho.db");
        let b = Banco::abrir(&caminho).unwrap();
        // Um arquivo no lugar da pasta: não dá para criar.
        let bloqueio = pasta.join("bloqueio");
        std::fs::write(&bloqueio, "x").unwrap();
        config::gravar(&b.conn, "backup_pasta", &bloqueio.join("sub").display().to_string()).unwrap();
        let e = fazer(&caminho, false).unwrap_err();
        assert!(e.to_string().contains("Não deu para"), "{e}");
        assert!(!situacao(&b.conn).unwrap().erro.is_empty());
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn ajustes_validos() {
        let b = Banco::em_memoria().unwrap();
        let e = |p: &str, d: &str, h: &str| AjustesEntrada { pasta: p.into(), dias: d.into(), hora: h.into() };
        assert_eq!(salvar_ajustes(&b.conn, None, &e("rel", "30", "18:00")).unwrap_err().nome_do_campo(), Some("pasta"));
        assert_eq!(salvar_ajustes(&b.conn, None, &e("", "0", "18:00")).unwrap_err().nome_do_campo(), Some("dias"));
        assert_eq!(salvar_ajustes(&b.conn, None, &e("", "30", "25:00")).unwrap_err().nome_do_campo(), Some("hora"));
        salvar_ajustes(&b.conn, None, &e("/tmp/bk", "15", "9:30")).unwrap();
        let a = ajustes(&b.conn, None).unwrap();
        assert_eq!((a.dias, a.hora.as_str()), (15, "09:30"));
    }

    #[test]
    fn nuvem_pelo_rclone() {
        let _trava = UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        // Usa uma pasta comum no lugar do Google Drive (o rclone trata igual).
        if !rclone_instalado() {
            eprintln!("rclone não instalado: teste da nuvem pulado");
            return;
        }
        let pasta = pasta_teste("nuvem");
        let nuvem = pasta.join("drive");
        std::env::set_var("RUSTINHO_REMOTO", nuvem.display().to_string());
        let caminho = pasta.join("rustinho.db");
        let mut b = Banco::abrir(&caminho).unwrap();
        config::gravar(&b.conn, "loja_nome", "Na Nuvem").unwrap();
        config::gravar(&b.conn, "backup_nuvem", "1").unwrap();

        // Sem chave da loja (nenhum gerente entrou ainda) não vai para a nuvem.
        fazer(&caminho, false).unwrap();
        assert!(enviar_pendentes(&b.conn, &caminho).is_err());
        assert!(!situacao(&b.conn).unwrap().nuvem_erro.is_empty());

        let ana = crate::usuarios::criar(&b.conn, &b.dispositivo, "Ana", "1111", true).unwrap();
        crate::usuarios::definir_gerente(&b.conn, &ana.id, true).unwrap();
        gerente_conferido(&b.conn, &ana.id, "1111", None).unwrap();
        std::thread::sleep(Duration::from_millis(1100));
        fazer(&caminho, false).unwrap();
        enviar_pendentes(&b.conn, &caminho).unwrap();
        let la = listar_nuvem().unwrap();
        assert_eq!(la.len(), 1);
        assert!(situacao(&b.conn).unwrap().nuvem_erro.is_empty());
        // Enviar de novo não duplica.
        enviar_pendentes(&b.conn, &caminho).unwrap();
        assert_eq!(listar_nuvem().unwrap().len(), 1);

        config::gravar(&b.conn, "loja_nome", "Mudou").unwrap();
        restaurar(&mut b.conn, Some(&caminho), &la[0].nome, true, "1111").unwrap();
        assert_eq!(config::nome_loja(&b.conn).unwrap(), "Na Nuvem");
        std::env::remove_var("RUSTINHO_REMOTO");
        let _ = std::fs::remove_dir_all(&pasta);
    }
}
