//! Guarda segura da identidade deste caixa (D4, A7, P4-2) e gestão de convites
//! de uso único (A6) — protocolo 4 (`0.9.19-dev.2`).
//!
//! ## Onde fica a identidade (`P4-2`, M5)
//! - **Nunca no banco SQLite:** copiar `rustinho.db` para outro computador não
//!   leva a chave privada (D1).
//! - Em instalação normal (`eh_portatil = false`): na pasta de configuração local
//!   do usuário (`$XDG_CONFIG_HOME/rustinho/` ou `$HOME/.config/rustinho/` no Linux;
//!   `%APPDATA%\Rustinho\` no Windows), diferenciada pelo hash do caminho canônico
//!   da pasta do banco para que duas pastas no mesmo computador (`RD-B`) tenham
//!   identidades independentes sem colidir. Se não houver pasta de configuração
//!   disponível, falha explicitamente (nunca cai silenciosamente na pasta do banco).
//! - Em modo portátil (`eh_portatil = true`): na própria pasta do portátil
//!   (`identidade.p4`), protegida contra cópia por `I3` + `copia_detectada`.
//! - Em testes automatizados: `RUSTINHO_IDENTIDADE` fixa um caminho explícito.
//!
//! ## Como o segredo de selagem é guardado (`D4`, `A7`, M3)
//! A identidade é sempre gravada selada com Argon2id + ChaCha20-Poly1305
//! ([`Identidade::guardar_em`]) usando uma senha aleatória de 256 bits própria
//! desta máquina (independente da senha do gerente).
//! - **Linux:** tenta o serviço de senhas do sistema (`/usr/bin/secret-tool` ou
//!   caminhos absolutos fixos, com prazo de 3 s e checagem de D-Bus); quando
//!   ausente (ex.: ambiente headless/mínimo), usa reserva local ofuscada (`0600`)
//!   fora da pasta do banco e marca `cofre_sistema = false` para aviso explícito
//!   ao gerente (nunca silencioso).
//! - **Windows:** protege o segredo via DPAPI nativo (`CryptProtectData` /
//!   `CryptUnprotectData` de `crypt32.dll`, `cofre_sistema = true`).
//! - **Reserva local (`cofre_sistema = false`):** ofuscação local com permissão
//!   `0600` (fora do banco). Em modo instalado vincula-se ao caminho +
//!   `/etc/machine-id` (quando presente); em modo portátil não depende de
//!   `/etc/machine-id` para continuar abrindo em outro computador pelo pendrive
//!   (clonagem é detectada por `I3` + `copia_detectada`). Se a máquina perder o
//!   cofre/machine-id, [`regerar_identidade_local`] cria identidade limpa e exige
//!   nova aprovação do gerente.
//! - **Testes (`#[cfg(test)]` ou `RUSTINHO_COFRE_TESTE=arquivo`):** usa a reserva
//!   local isolada sem tocar no keyring do host.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::ChaCha20Poly1305;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::aprovados::{self, Aprovacao, Pedido};
use super::{aleatorio, apagar, hex, Chave, Identidade, Segredo};
use crate::erro::{Erro, Resultado};

/// Validade padrão de um convite de entrada de caixa novo: 10 minutos (A6).
pub const VALIDADE_CONVITE_SEG: i64 = 10 * 60;
/// Retenção máxima para purgar convites expirados antigos (24 h).
const RETENCAO_CONVITE_EXPIRADO_SEG: i64 = 24 * 60 * 60;
/// Alfabeto Base32 sem letras ambíguas (`I`, `O`, `0`, `1`), idêntico ao código de pedido.
const ALFABETO_CONVITE: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// Trava intra-processo que serializa criação/leitura inicial da identidade (A2).
fn trava_processo() -> &'static Mutex<()> {
    static M: OnceLock<Mutex<()>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(()))
}

/// Trava exclusiva entre processos (A2, B3-R2): no Unix usa `flock(LOCK_EX)`
/// (atômico no kernel, sem corrida de remoção de trava órfã); fora do Unix usa
/// arquivo `.lock` exclusivo com `create_new` e RAII `Drop`.
struct TravaArquivo {
    #[cfg(unix)]
    _arquivo: std::fs::File,
    #[cfg(not(unix))]
    caminho: PathBuf,
}

impl TravaArquivo {
    fn adquirir(caminho_id: &Path) -> Resultado<TravaArquivo> {
        let mut os = caminho_id.as_os_str().to_os_string();
        os.push(".lock");
        let caminho = PathBuf::from(os);
        let pasta = caminho.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        criar_pasta_restrita(pasta)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            use std::os::unix::io::AsRawFd;
            let arquivo = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .open(&caminho)
                .map_err(|e| {
                    Erro::Interno(format!("não foi possível abrir a trava da identidade do caixa ({:?})", e.kind()))
                })?;
            let fd = arquivo.as_raw_fd();
            loop {
                // SAFETY: `fd` pertence ao `File` aberto acima e `LOCK_EX` bloqueia até obter a trava exclusiva.
                let rc = unsafe { libc::flock(fd, libc::LOCK_EX) };
                if rc == 0 {
                    return Ok(TravaArquivo { _arquivo: arquivo });
                }
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(Erro::Interno(format!(
                    "não foi possível travar o arquivo da identidade do caixa ({:?})",
                    err.kind()
                )));
            }
        }

        #[cfg(not(unix))]
        {
            let fim = Instant::now() + Duration::from_secs(10);
            loop {
                match std::fs::OpenOptions::new().write(true).create_new(true).open(&caminho) {
                    Ok(mut f) => {
                        let _ = write!(f, "{}", std::process::id());
                        return Ok(TravaArquivo { caminho });
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        if Instant::now() >= fim {
                            return Err(Erro::Interno(
                                "tempo esgotado aguardando trava exclusiva da identidade do caixa".into(),
                            ));
                        }
                        std::thread::sleep(Duration::from_millis(15));
                    }
                    Err(e) => {
                        return Err(Erro::Interno(format!(
                            "não foi possível travar o arquivo da identidade do caixa ({:?})",
                            e.kind()
                        )));
                    }
                }
            }
        }
    }
}

#[cfg(not(unix))]
impl Drop for TravaArquivo {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.caminho);
    }
}

fn criar_pasta_restrita(pasta: &Path) -> Resultado<()> {
    let ja_existia = pasta.exists();
    std::fs::create_dir_all(pasta).map_err(|e| {
        Erro::Interno(format!("não foi possível criar a pasta da identidade do caixa ({:?})", e.kind()))
    })?;
    #[cfg(unix)]
    if !ja_existia {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(pasta, std::fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

/// Identidade carregada para este caixa, junto com a origem da proteção do segredo.
pub struct IdentidadeLocal {
    pub identidade: Identidade,
    /// `true` quando o segredo de selagem está protegido pelo cofre do SO
    /// (`secret-tool` no Linux ou DPAPI no Windows); `false` na reserva local (aviso D4).
    pub cofre_sistema: bool,
    pub caminho: PathBuf,
}

impl std::fmt::Debug for IdentidadeLocal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdentidadeLocal")
            .field("codigo", &self.identidade.codigo_de_pedido())
            .field("cofre_sistema", &self.cofre_sistema)
            .field("caminho", &self.caminho)
            .finish()
    }
}

/// Canonicaliza `pasta_banco` de forma determinística antes e depois de a pasta
/// existir (M5): cria a pasta se necessário ou canonicaliza o ancestral existente.
fn caminho_canonico_estavel(pasta_banco: &Path) -> Resultado<PathBuf> {
    if !pasta_banco.exists() {
        std::fs::create_dir_all(pasta_banco).map_err(|e| {
            Erro::Interno(format!("não foi possível preparar a pasta de dados para a identidade ({:?})", e.kind()))
        })?;
    }
    std::fs::canonicalize(pasta_banco)
        .map_err(|e| Erro::Interno(format!("não foi possível resolver o caminho da pasta de dados ({:?})", e.kind())))
}

/// Calcula o caminho do arquivo `identidade*.p4` para `pasta_banco` (P4-2, M5).
///
/// Sem depender de `crate::portatil` (zero ciclos no grafo de dependências).
/// Em modo instalado, exige pasta de configuração do usuário (nunca cai dentro
/// da pasta do banco em silêncio).
pub fn caminho_identidade(pasta_banco: &Path, eh_portatil: bool) -> Resultado<PathBuf> {
    if let Ok(explicito) = std::env::var("RUSTINHO_IDENTIDADE") {
        let limpo = explicito.trim();
        if !limpo.is_empty() {
            return Ok(PathBuf::from(limpo));
        }
    }
    caminho_identidade_com_base(pasta_banco, eh_portatil, None)
}

fn caminho_identidade_com_base(
    pasta_banco: &Path,
    eh_portatil: bool,
    base_cfg_override: Option<&Path>,
) -> Resultado<PathBuf> {
    let canon = caminho_canonico_estavel(pasta_banco)?;
    if eh_portatil {
        return Ok(canon.join("identidade.p4"));
    }
    let pasta_cfg = match base_cfg_override {
        Some(p) => p.to_path_buf(),
        None => pasta_config_usuario().ok_or_else(|| {
            Erro::validacao(
                "Não foi possível localizar a pasta de configuração do usuário (HOME/APPDATA) para guardar a \
                 identidade deste caixa fora da pasta do banco.",
            )
        })?,
    };
    let mut h = Sha256::new();
    h.update(b"rustinho/p4/pasta-identidade\0");
    h.update(canon.to_string_lossy().as_bytes());
    let dig: [u8; 32] = h.finalize().into();
    let sufixo = hex(&dig[..8]);
    Ok(pasta_cfg.join(format!("identidade-{sufixo}.p4")))
}

fn pasta_config_usuario() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Ok(ap) = std::env::var("APPDATA") {
            if !ap.trim().is_empty() {
                return Some(PathBuf::from(ap).join("Rustinho"));
            }
        }
        None
    }
    #[cfg(not(windows))]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            if !xdg.trim().is_empty() {
                return Some(PathBuf::from(xdg).join("rustinho"));
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            if !home.trim().is_empty() {
                return Some(PathBuf::from(home).join(".config").join("rustinho"));
            }
        }
        None
    }
}

fn usar_so_arquivo_reserva() -> bool {
    cfg!(test) || std::env::var("RUSTINHO_COFRE_TESTE").ok().as_deref() == Some("arquivo")
}

fn conta_cofre(caminho: &Path) -> String {
    let mut h = Sha256::new();
    h.update(b"rustinho/p4/cofre-conta\0");
    h.update(caminho.to_string_lossy().as_bytes());
    let d: [u8; 32] = h.finalize().into();
    format!("caixa-{}", hex(&d[..12]))
}

fn caminho_reserva(caminho_id: &Path) -> PathBuf {
    let mut os = caminho_id.as_os_str().to_os_string();
    os.push(".segredo");
    PathBuf::from(os)
}

#[cfg(windows)]
fn caminho_dpapi(caminho_id: &Path) -> PathBuf {
    let mut os = caminho_id.as_os_str().to_os_string();
    os.push(".dpapi");
    PathBuf::from(os)
}

/// Carrega a identidade deste caixa ou gera e sela uma nova na primeira abertura.
/// Nunca substitui uma identidade já gravada e é seguro contra corrida entre
/// threads e processos (A2).
pub fn carregar_ou_criar(pasta_banco: &Path, eh_portatil: bool) -> Resultado<IdentidadeLocal> {
    let caminho = caminho_identidade(pasta_banco, eh_portatil)?;
    carregar_ou_criar_no_caminho(caminho, eh_portatil)
}

fn carregar_ou_criar_no_caminho(caminho: PathBuf, eh_portatil: bool) -> Resultado<IdentidadeLocal> {
    let _guarda_proc = trava_processo().lock().unwrap_or_else(|e| e.into_inner());
    let _guarda_arq = TravaArquivo::adquirir(&caminho)?;
    if caminho.exists() {
        let (senha, cofre_sistema) = carregar_segredo(&caminho, eh_portatil)?;
        let senha_str = std::str::from_utf8(&senha.0)
            .map_err(|_| Erro::validacao("O segredo protegido da identidade está danificado."))?;
        let identidade = Identidade::abrir_de(&caminho, senha_str)?;
        return Ok(IdentidadeLocal { identidade, cofre_sistema, caminho });
    }
    let identidade = Identidade::gerar()?;
    let mut bruto: [u8; 32] = aleatorio()?;
    let senha = Segredo(hex(&bruto).into_bytes());
    apagar(&mut bruto);
    let senha_str = std::str::from_utf8(&senha.0).map_err(|_| Erro::Interno("segredo aleatório não UTF-8".into()))?;
    let cofre_sistema = guardar_segredo(&caminho, senha_str, eh_portatil)?;
    if let Err(e) = identidade.guardar_em(&caminho, senha_str) {
        // Reverte o arquivo de reserva recém-escrito se a identidade falhou ao gravar.
        let _ = std::fs::remove_file(caminho_reserva(&caminho));
        return Err(e);
    }
    Ok(IdentidadeLocal { identidade, cofre_sistema, caminho })
}

/// Recuperação explícita (M3): arquiva uma identidade local ilegível (ex.: após
/// perda do cofre do SO ou troca de `machine-id` em VM) e gera uma identidade
/// nova limpa, que exigirá nova aprovação pelo gerente da loja.
pub fn regerar_identidade_local(pasta_banco: &Path, eh_portatil: bool) -> Resultado<IdentidadeLocal> {
    let caminho = caminho_identidade(pasta_banco, eh_portatil)?;
    regerar_no_caminho(caminho, eh_portatil)
}

fn regerar_no_caminho(caminho: PathBuf, eh_portatil: bool) -> Resultado<IdentidadeLocal> {
    let _guarda_proc = trava_processo().lock().unwrap_or_else(|e| e.into_inner());
    let _guarda_arq = TravaArquivo::adquirir(&caminho)?;
    let sufixo = crate::tempo::novo_id();
    if caminho.exists() {
        let mut backup = caminho.as_os_str().to_os_string();
        backup.push(format!(".antiga-{sufixo}"));
        std::fs::rename(&caminho, PathBuf::from(backup))
            .map_err(|e| Erro::Interno(format!("não foi possível arquivar a identidade anterior ({:?})", e.kind())))?;
    }
    let _ = std::fs::remove_file(caminho_reserva(&caminho));
    #[cfg(windows)]
    let _ = std::fs::remove_file(caminho_dpapi(&caminho));

    let identidade = Identidade::gerar()?;
    let mut bruto: [u8; 32] = aleatorio()?;
    let senha = Segredo(hex(&bruto).into_bytes());
    apagar(&mut bruto);
    let senha_str = std::str::from_utf8(&senha.0).map_err(|_| Erro::Interno("segredo aleatório não UTF-8".into()))?;
    let cofre_sistema = guardar_segredo(&caminho, senha_str, eh_portatil)?;
    if let Err(e) = identidade.guardar_em(&caminho, senha_str) {
        let _ = std::fs::remove_file(caminho_reserva(&caminho));
        return Err(e);
    }
    Ok(IdentidadeLocal { identidade, cofre_sistema, caminho })
}

/// Carrega apenas se já existir identidade neste caminho.
pub fn carregar_existente(pasta_banco: &Path, eh_portatil: bool) -> Resultado<Option<IdentidadeLocal>> {
    let caminho = caminho_identidade(pasta_banco, eh_portatil)?;
    let _guarda_proc = trava_processo().lock().unwrap_or_else(|e| e.into_inner());
    let _guarda_arq = TravaArquivo::adquirir(&caminho)?;
    if !caminho.exists() {
        return Ok(None);
    }
    let (senha, cofre_sistema) = carregar_segredo(&caminho, eh_portatil)?;
    let senha_str = std::str::from_utf8(&senha.0)
        .map_err(|_| Erro::validacao("O segredo protegido da identidade está danificado."))?;
    let identidade = Identidade::abrir_de(&caminho, senha_str)?;
    Ok(Some(IdentidadeLocal { identidade, cofre_sistema, caminho }))
}

fn guardar_segredo(caminho_id: &Path, senha: &str, eh_portatil: bool) -> Resultado<bool> {
    if !usar_so_arquivo_reserva() {
        #[cfg(target_os = "linux")]
        if secret_tool_guardar(&conta_cofre(caminho_id), senha) {
            return Ok(true);
        }
        #[cfg(windows)]
        if dpapi_guardar(&caminho_dpapi(caminho_id), senha).is_ok() {
            return Ok(true);
        }
    }
    guardar_reserva_local(&caminho_reserva(caminho_id), caminho_id, senha, eh_portatil)?;
    Ok(false)
}

fn carregar_segredo(caminho_id: &Path, eh_portatil: bool) -> Resultado<(Segredo, bool)> {
    if !usar_so_arquivo_reserva() {
        #[cfg(target_os = "linux")]
        if let Some(s) = secret_tool_ler(&conta_cofre(caminho_id)) {
            return Ok((s, true));
        }
        #[cfg(windows)]
        {
            let dp = caminho_dpapi(caminho_id);
            if dp.exists() {
                if let Ok(s) = dpapi_ler(&dp) {
                    return Ok((s, true));
                }
            }
        }
    }
    let r = caminho_reserva(caminho_id);
    if r.exists() {
        let s = ler_reserva_local(&r, caminho_id, eh_portatil)?;
        return Ok((s, false));
    }
    Err(Erro::validacao("Não foi possível abrir o cofre de senhas deste computador para ler a identidade do caixa."))
}

/// Caminhos absolutos confiáveis do `secret-tool` no Linux (B2: sem depender de `PATH`).
#[cfg(target_os = "linux")]
fn caminho_secret_tool() -> Option<&'static Path> {
    const CANDIDATOS: &[&str] = &["/usr/bin/secret-tool", "/usr/local/bin/secret-tool", "/bin/secret-tool"];
    CANDIDATOS.iter().map(Path::new).find(|p| p.is_file())
}

#[cfg(target_os = "linux")]
fn esperar_com_prazo(filho: &mut std::process::Child, prazo: Duration) -> Option<std::process::ExitStatus> {
    let fim = Instant::now() + prazo;
    loop {
        match filho.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if Instant::now() < fim => {
                std::thread::sleep(Duration::from_millis(25));
            }
            _ => {
                let _ = filho.kill();
                let _ = filho.wait();
                return None;
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn dbus_sessao_disponivel() -> bool {
    std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some_and(|v| !v.is_empty())
}

#[cfg(target_os = "linux")]
fn secret_tool_guardar(conta: &str, segredo: &str) -> bool {
    use std::process::{Command, Stdio};
    if !dbus_sessao_disponivel() {
        return false;
    }
    let Some(bin) = caminho_secret_tool() else {
        return false;
    };
    let Ok(mut filho) = Command::new(bin)
        .args(["store", "--label", "Rustinho (identidade do caixa)", "service", "rustinho-p4", "account", conta])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    if let Some(mut stdin) = filho.stdin.take() {
        if stdin.write_all(segredo.as_bytes()).is_err() {
            let _ = filho.kill();
            let _ = filho.wait();
            return false;
        }
    }
    esperar_com_prazo(&mut filho, Duration::from_secs(3)).is_some_and(|s| s.success())
}

#[cfg(target_os = "linux")]
fn secret_tool_ler(conta: &str) -> Option<Segredo> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    if !dbus_sessao_disponivel() {
        return None;
    }
    let bin = caminho_secret_tool()?;
    let mut filho = Command::new(bin)
        .args(["lookup", "service", "rustinho-p4", "account", conta])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let status = esperar_com_prazo(&mut filho, Duration::from_secs(3))?;
    if !status.success() {
        return None;
    }
    let mut saida = Vec::new();
    if let Some(mut stdout) = filho.stdout.take() {
        if stdout.read_to_end(&mut saida).is_err() {
            apagar(&mut saida);
            return None;
        }
    }
    while saida.last().is_some_and(|b| b.is_ascii_whitespace()) {
        saida.pop();
    }
    let inicio = saida.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(saida.len());
    let limpo = saida[inicio..].to_vec();
    apagar(&mut saida);
    if limpo.is_empty() || std::str::from_utf8(&limpo).is_err() {
        let mut lixo = limpo;
        apagar(&mut lixo);
        None
    } else {
        Some(Segredo(limpo))
    }
}

#[cfg(windows)]
mod dpapi {
    use super::*;
    use std::ptr;

    #[repr(C)]
    struct DataBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            p_data_in: *const DataBlob,
            sz_data_descr: *const u16,
            p_optional_entropy: *const DataBlob,
            pv_reserved: *mut core::ffi::c_void,
            p_prompt_struct: *mut core::ffi::c_void,
            dw_flags: u32,
            p_data_out: *mut DataBlob,
        ) -> i32;

        fn CryptUnprotectData(
            p_data_in: *const DataBlob,
            ppsz_data_descr: *mut *mut u16,
            p_optional_entropy: *const DataBlob,
            pv_reserved: *mut core::ffi::c_void,
            p_prompt_struct: *mut core::ffi::c_void,
            dw_flags: u32,
            p_data_out: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(h_mem: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
    }

    pub fn proteger(claro: &[u8]) -> Resultado<Vec<u8>> {
        let mut copia = claro.to_vec();
        let cb_data =
            u32::try_from(copia.len()).map_err(|_| Erro::Interno("segredo grande demais para DPAPI".into()))?;
        let entrada = DataBlob { cb_data, pb_data: copia.as_mut_ptr() };
        let mut saida = DataBlob { cb_data: 0, pb_data: ptr::null_mut() };
        let ok = unsafe {
            CryptProtectData(
                &entrada,
                ptr::null(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut saida,
            )
        };
        apagar(&mut copia);
        if ok == 0 || saida.pb_data.is_null() {
            return Err(Erro::Interno("DPAPI não conseguiu proteger a identidade do caixa".into()));
        }
        let bytes = unsafe { std::slice::from_raw_parts(saida.pb_data, saida.cb_data as usize) }.to_vec();
        unsafe {
            LocalFree(saida.pb_data as *mut core::ffi::c_void);
        }
        Ok(bytes)
    }

    pub fn desproteger(cifrado: &[u8]) -> Resultado<Segredo> {
        let mut copia = cifrado.to_vec();
        let cb_data = u32::try_from(copia.len())
            .map_err(|_| Erro::validacao("O segredo protegido da identidade está danificado."))?;
        let entrada = DataBlob { cb_data, pb_data: copia.as_mut_ptr() };
        let mut saida = DataBlob { cb_data: 0, pb_data: ptr::null_mut() };
        let ok = unsafe {
            CryptUnprotectData(
                &entrada,
                ptr::null_mut(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut saida,
            )
        };
        if ok == 0 || saida.pb_data.is_null() {
            return Err(Erro::validacao("DPAPI não conseguiu ler a identidade protegida deste caixa."));
        }
        let slice = unsafe { std::slice::from_raw_parts_mut(saida.pb_data, saida.cb_data as usize) };
        let bytes = slice.to_vec();
        apagar(slice);
        unsafe {
            LocalFree(saida.pb_data as *mut core::ffi::c_void);
        }
        Ok(Segredo(bytes))
    }
}

#[cfg(windows)]
fn dpapi_guardar(caminho: &Path, segredo: &str) -> Resultado<()> {
    let protegido = dpapi::proteger(segredo.as_bytes())?;
    gravar_restrito(caminho, &protegido)
}

#[cfg(windows)]
fn dpapi_ler(caminho: &Path) -> Resultado<Segredo> {
    let bytes = std::fs::read(caminho)
        .map_err(|e| Erro::Interno(format!("não foi possível ler o segredo DPAPI ({:?})", e.kind())))?;
    let claro = dpapi::desproteger(&bytes)?;
    if std::str::from_utf8(&claro.0).is_err() {
        return Err(Erro::validacao("O segredo protegido da identidade está danificado."));
    }
    Ok(claro)
}

/// Chave de ofuscação da reserva local (D4 / M3): usada **somente** quando o SO
/// não oferece serviço de senhas (`cofre_sistema = false`, aviso explícito ao
/// gerente). Não substitui um cofre do SO; protege via permissão `0600` e
/// separação da pasta do banco. Em modo portátil não inclui `/etc/machine-id`
/// para que o pendrive continue abrindo em outra máquina (`I3` detecta cópia).
fn chave_reserva_local(caminho_id: &Path, eh_portatil: bool) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"rustinho/p4/reserva-local/v1\0");
    if eh_portatil {
        h.update(b"portatil\0");
        if let Some(nome) = caminho_id.file_name() {
            h.update(nome.to_string_lossy().as_bytes());
        }
    } else {
        h.update(b"instalado\0");
        h.update(caminho_id.to_string_lossy().as_bytes());
        if let Ok(mid) = std::fs::read_to_string("/etc/machine-id") {
            h.update(mid.trim().as_bytes());
        }
    }
    h.finalize().into()
}

const MAGICA_RESERVA: &[u8; 4] = b"RSR1";

fn guardar_reserva_local(caminho: &Path, caminho_id: &Path, segredo: &str, eh_portatil: bool) -> Resultado<()> {
    let mut chave = chave_reserva_local(caminho_id, eh_portatil);
    let nonce: [u8; 12] = aleatorio()?;
    let mut cabecalho = Vec::with_capacity(16);
    cabecalho.extend_from_slice(MAGICA_RESERVA);
    cabecalho.extend_from_slice(&nonce);
    let cifra = ChaCha20Poly1305::new_from_slice(&chave).map_err(|_| Erro::Interno("chave reserva inválida".into()))?;
    apagar(&mut chave);
    let corpo = cifra
        .encrypt(&nonce.into(), Payload { msg: segredo.as_bytes(), aad: &cabecalho })
        .map_err(|_| Erro::Interno("falha ao gravar reserva local da identidade".into()))?;
    let mut tudo = cabecalho;
    tudo.extend_from_slice(&corpo);
    gravar_restrito(caminho, &tudo)
}

fn ler_reserva_local(caminho: &Path, caminho_id: &Path, eh_portatil: bool) -> Resultado<Segredo> {
    let bytes = std::fs::read(caminho)
        .map_err(|e| Erro::Interno(format!("não foi possível ler reserva da identidade ({:?})", e.kind())))?;
    if bytes.len() < 16 + 16 || bytes[..4] != MAGICA_RESERVA[..] {
        return Err(Erro::validacao("Arquivo de reserva da identidade inválido."));
    }
    let (cabecalho, corpo) = bytes.split_at(16);
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&cabecalho[4..16]);
    let mut chave = chave_reserva_local(caminho_id, eh_portatil);
    let cifra = ChaCha20Poly1305::new_from_slice(&chave).map_err(|_| Erro::Interno("chave reserva inválida".into()))?;
    apagar(&mut chave);
    let claro =
        Segredo(cifra.decrypt(&nonce.into(), Payload { msg: corpo, aad: cabecalho }).map_err(|_| {
            Erro::validacao("A reserva da identidade não pertence a este computador ou foi danificada.")
        })?);
    if std::str::from_utf8(&claro.0).is_err() {
        return Err(Erro::validacao("A reserva da identidade está danificada."));
    }
    Ok(claro)
}

fn gravar_restrito(caminho: &Path, bytes: &[u8]) -> Resultado<()> {
    let falha = |e: std::io::Error| {
        Erro::Interno(format!("não foi possível guardar o cofre local da identidade ({:?})", e.kind()))
    };
    let pasta = caminho.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    criar_pasta_restrita(pasta)?;
    let nome = caminho.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let temporario = pasta.join(format!(".{nome}.novo-{}", crate::tempo::novo_id()));
    let mut opcoes = std::fs::OpenOptions::new();
    opcoes.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opcoes.mode(0o600);
    }
    let gravou = opcoes.open(&temporario).and_then(|mut f| {
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&temporario, caminho)
    });
    if let Err(e) = gravou {
        let _ = std::fs::remove_file(&temporario);
        return Err(falha(e));
    }
    crate::arquivos::so_do_usuario(caminho, "cofre local da identidade");
    Ok(())
}

// ---------------------------------------------------------------------------
// Inicialização de raiz e persistência da âncora fixada (`p4_raiz`, A1).
// ---------------------------------------------------------------------------

/// Lê a âncora de confiança fixada em `config` (`p4_raiz`).
fn pino_raiz(conn: &Connection) -> Resultado<Option<Chave>> {
    if let Some(txt) = crate::config::ler(conn, "p4_raiz")? {
        if let Some(c) = super::de_hex::<32>(txt.trim()) {
            return Ok(Some(c));
        }
    }
    Ok(None)
}

/// Garante que a chave pública local esteja anotada em `config` (nunca a privada)
/// e, se ainda não houver âncora fixada (`p4_raiz`), inicializa-a a partir do
/// registro único da loja ou registrando este distribuidor de faixas como raiz.
///
/// **Regra de âncora fixa (A1):** se `p4_raiz` já está fixado em `config`, nunca
/// é sobrescrito por linhas recebidas em `caixa_aprovado`.
pub fn garantir_raiz_se_aplicavel(
    conn: &Connection,
    id: &Identidade,
    cofre_sistema: bool,
    loja_id: &str,
    nome_caixa: &str,
    gerente: &str,
    eh_distribuidor: bool,
) -> Resultado<Option<Chave>> {
    crate::config::gravar(conn, "p4_chave_publica", &hex(&id.publica()))?;
    crate::config::gravar(conn, "p4_cofre_sistema", if cofre_sistema { "1" } else { "0" })?;
    if loja_id.trim().is_empty() {
        return Ok(None);
    }
    // A1: âncora local já fixada vence sempre e nunca é sobrescrita.
    if let Some(pino) = pino_raiz(conn)? {
        return Ok(Some(pino));
    }
    // Sem pino ainda: só adota raiz existente se houver exatamente UMA raiz válida.
    if let Some(raiz) = aprovados::raiz_registrada(conn, loja_id)? {
        crate::config::gravar(conn, "p4_raiz", &hex(&raiz))?;
        return Ok(Some(raiz));
    }
    if eh_distribuidor && !gerente.trim().is_empty() {
        let nome = if nome_caixa.trim().is_empty() { "Caixa 1" } else { nome_caixa.trim() };
        aprovados::registrar_raiz(conn, id, loja_id, nome, gerente)?;
        crate::config::gravar(conn, "p4_raiz", &hex(&id.publica()))?;
        return Ok(Some(id.publica()));
    }
    Ok(None)
}

/// Lê a âncora de confiança atual da loja (A1): **sempre** prefere o pino
/// `p4_raiz` em `config`. Só consulta `caixa_aprovado` quando o pino ainda não
/// existe (e falha fechado se houver mais de uma raiz distinta).
pub fn raiz_atual(conn: &Connection, loja_id: &str) -> Resultado<Option<Chave>> {
    if let Some(pino) = pino_raiz(conn)? {
        return Ok(Some(pino));
    }
    aprovados::raiz_registrada(conn, loja_id)
}

// ---------------------------------------------------------------------------
// Convites de uso único (A6: 128 bits, 26 caracteres Base32, validade 10 min).
// ---------------------------------------------------------------------------

/// Convite recém-gerado para o gerente repassar ao caixa novo (texto ou QR).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConviteGerado {
    pub id: String,
    /// Código formatado em grupos (26 caracteres úteis Base32, 128 bits).
    pub segredo: String,
    /// Código de pedido de 12 chars conferido pelo gerente (ex.: `4F2K-9QXA-7MPC`).
    pub codigo_pedido: String,
    pub nome_caixa: String,
    pub expira_em: String,
    /// Texto compacto para QR code (`RUST4|segredo|loja_id|aprovador_hex|endereco`).
    pub payload_qr: String,
}

/// Resumo de convite ativo (nunca expõe o segredo após expirar/consumir).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConviteAtivo {
    pub id: String,
    pub segredo: String,
    pub codigo_pedido: String,
    pub nome_caixa: String,
    pub gerente: String,
    pub criado_em: String,
    pub expira_em: String,
}

/// Codifica 16 bytes (128 bits) em 26 caracteres Base32 sem ambiguidade,
/// agrupados em blocos de 5 para digitação confortável (`XXXXX-XXXXX-XXXXX-XXXXX-XXXXX-X`).
pub fn formatar_segredo_convite(bytes: &[u8; 16]) -> String {
    let mut bits: u128 = u128::from_be_bytes(*bytes);
    let mut chars = ['A'; 26];
    // 26 × 5 = 130 bits (2 bits extras zero no topo).
    for i in (0..26).rev() {
        chars[i] = ALFABETO_CONVITE[(bits & 0x1F) as usize] as char;
        bits >>= 5;
    }
    let mut saida = String::with_capacity(31);
    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && i % 5 == 0 {
            saida.push('-');
        }
        saida.push(*ch);
    }
    saida
}

/// Normaliza o convite digitado pelo usuário (maiúsculas, sem traços/espaços).
pub fn normalizar_segredo(texto: &str) -> String {
    texto.chars().filter(|c| !c.is_whitespace() && *c != '-').flat_map(|c| c.to_uppercase()).collect()
}

/// Normaliza o código de pedido (12 caracteres, maiúsculas, formato `XXXX-XXXX-XXXX`),
/// operando estritamente em `char` (M1: nunca faz slice de bytes em texto UTF-8).
pub fn normalizar_codigo_pedido(texto: &str) -> String {
    let chars: Vec<char> =
        texto.chars().filter(|c| !c.is_whitespace() && *c != '-').flat_map(|c| c.to_uppercase()).collect();
    if chars.len() == 12 {
        let mut s = String::with_capacity(14);
        for (i, ch) in chars.into_iter().enumerate() {
            if i == 4 || i == 8 {
                s.push('-');
            }
            s.push(ch);
        }
        s
    } else {
        chars.into_iter().collect()
    }
}

fn eh_codigo_pedido_valido(cod: &str) -> bool {
    let chars: Vec<char> = cod.chars().collect();
    if chars.len() != 14 || chars[4] != '-' || chars[9] != '-' {
        return false;
    }
    chars.iter().enumerate().all(|(i, &c)| {
        if i == 4 || i == 9 {
            c == '-'
        } else {
            c.is_ascii() && ALFABETO_CONVITE.contains(&(c as u8))
        }
    })
}

pub fn hash_segredo(segredo_normalizado: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"rustinho/p4/convite-hash\0");
    h.update(segredo_normalizado.as_bytes());
    let d: [u8; 32] = h.finalize().into();
    hex(&d)
}

/// Expira convites vencidos apagando `segredo_ativo`.
pub fn limpar_convites_expirados(conn: &Connection, agora_iso: &str) -> Resultado<()> {
    conn.execute(
        "UPDATE p4_convite SET segredo_ativo = NULL
          WHERE segredo_ativo IS NOT NULL AND expira_em <= ?1",
        [agora_iso],
    )?;
    Ok(())
}

/// Purga periódica / na abertura (B4): limpa `segredo_ativo` de todos os convites
/// vencidos e remove linhas expiradas há mais de 24 h.
pub fn purgar_convites_expirados(conn: &Connection) -> Resultado<usize> {
    let agora = crate::tempo::agora();
    limpar_convites_expirados(conn, &agora)?;
    let corte = crate::tempo::somar_segundos(&agora, -RETENCAO_CONVITE_EXPIRADO_SEG).unwrap_or(agora);
    let removidos = conn.execute("DELETE FROM p4_convite WHERE expira_em <= ?1", [corte])?;
    Ok(removidos)
}

/// Gera um convite de uso único (128 bits, validade 10 min) para aprovar um caixa novo.
#[allow(clippy::too_many_arguments)]
pub fn gerar_convite(
    conn: &Connection,
    aprovador: &Identidade,
    raiz: &Chave,
    loja_id: &str,
    codigo_pedido: &str,
    nome_caixa: &str,
    gerente: &str,
    endereco_dica: &str,
) -> Resultado<ConviteGerado> {
    if gerente.trim().is_empty() {
        return Err(Erro::validacao("Só o gerente pode gerar convite para caixa novo."));
    }
    if loja_id.trim().is_empty() {
        return Err(Erro::validacao("A loja ainda não tem identidade."));
    }
    let nome = crate::texto::limpar(nome_caixa);
    if nome.is_empty() {
        return Err(Erro::campo("nome", "Dê um nome ao caixa novo."));
    }
    if nome.chars().count() > 30 {
        return Err(Erro::campo("nome", "Use no máximo 30 caracteres para o nome do caixa."));
    }
    let cod = normalizar_codigo_pedido(codigo_pedido);
    if !eh_codigo_pedido_valido(&cod) {
        return Err(Erro::campo(
            "codigoPedido",
            "Informe o código de 12 caracteres que aparece na tela do caixa novo (ex.: 4F2K-9QXA-7MPC).",
        ));
    }
    if !aprovados::autorizada(conn, raiz, loja_id, &aprovador.publica())? {
        return Err(Erro::validacao("Este caixa não está autorizado a aprovar outros caixas nesta loja."));
    }
    let agora_iso = crate::tempo::agora();
    limpar_convites_expirados(conn, &agora_iso)?;
    let expira_iso = crate::tempo::somar_segundos(&agora_iso, VALIDADE_CONVITE_SEG).unwrap_or_else(crate::tempo::agora);

    let mut bruto: [u8; 16] = aleatorio()?;
    let segredo = formatar_segredo_convite(&bruto);
    apagar(&mut bruto);
    let norm = normalizar_segredo(&segredo);
    let shash = hash_segredo(&norm);
    let id = crate::tempo::novo_id();
    let disp = crate::config::ler(conn, "dispositivo_id")?.unwrap_or_default();

    conn.execute_batch("SAVEPOINT p4_convite_gerar")?;
    let res = (|| -> Resultado<()> {
        conn.execute(
            "INSERT INTO p4_convite (id, loja_id, segredo_hash, segredo_ativo, codigo_pedido, nome_caixa, gerente, criado_em, expira_em)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![id, loja_id, shash, norm, cod, nome, gerente, agora_iso, expira_iso],
        )?;
        crate::auditoria::registrar(
            conn,
            &disp,
            Some(gerente),
            "p4_convite_gerado",
            "caixa",
            &id,
            &[format!("Caixa: {nome}"), format!("Código do pedido: {cod}"), format!("Expira em: {expira_iso}")],
        )?;
        Ok(())
    })();
    match res {
        Ok(()) => conn.execute_batch("RELEASE p4_convite_gerar")?,
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK TO p4_convite_gerar; RELEASE p4_convite_gerar");
            return Err(e);
        }
    }

    let payload_qr = format!("RUST4|{}|{}|{}|{}", segredo, loja_id, hex(&aprovador.publica()), endereco_dica.trim());
    Ok(ConviteGerado { id, segredo, codigo_pedido: cod, nome_caixa: nome, expira_em: expira_iso, payload_qr })
}

/// Lista convites ainda ativos da loja (não usados e dentro da validade de 10 min).
pub fn listar_convites_ativos(conn: &Connection, loja_id: &str) -> Resultado<Vec<ConviteAtivo>> {
    let agora_iso = crate::tempo::agora();
    limpar_convites_expirados(conn, &agora_iso)?;
    let mut c = conn.prepare(
        "SELECT id, segredo_ativo, codigo_pedido, nome_caixa, gerente, criado_em, expira_em
           FROM p4_convite
          WHERE loja_id = ?1 AND usado_em IS NULL AND segredo_ativo IS NOT NULL AND expira_em > ?2
          ORDER BY criado_em DESC",
    )?;
    let linhas = c
        .query_map(params![loja_id, &agora_iso], |l| {
            let norm: String = l.get(1)?;
            let mut formatado = String::with_capacity(31);
            for (i, ch) in norm.chars().enumerate() {
                if i > 0 && i % 5 == 0 {
                    formatado.push('-');
                }
                formatado.push(ch);
            }
            Ok(ConviteAtivo {
                id: l.get(0)?,
                segredo: formatado,
                codigo_pedido: l.get(2)?,
                nome_caixa: l.get(3)?,
                gerente: l.get(4)?,
                criado_em: l.get(5)?,
                expira_em: l.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(linhas)
}

/// Cancela um convite ativo antes de ser usado, zerando `segredo_ativo`.
pub fn cancelar_convite(conn: &Connection, id: &str, gerente: &str) -> Resultado<()> {
    if gerente.trim().is_empty() {
        return Err(Erro::validacao("Só o gerente pode cancelar convite."));
    }
    let agora = crate::tempo::agora();
    let n = conn.execute(
        "UPDATE p4_convite SET segredo_ativo = NULL, usado_em = ?2
          WHERE id = ?1 AND usado_em IS NULL",
        params![id, agora],
    )?;
    if n == 0 {
        return Err(Erro::NaoEncontrado("Convite".into()));
    }
    let disp = crate::config::ler(conn, "dispositivo_id")?.unwrap_or_default();
    crate::auditoria::registrar(conn, &disp, Some(gerente), "p4_convite_cancelado", "caixa", id, &[])?;
    Ok(())
}

/// Registro interno de um convite candidato (para verificação de *channel binding* no canal TLS).
#[derive(Clone, Debug)]
pub struct ConviteCandidato {
    pub id: String,
    pub loja_id: String,
    pub segredo_normalizado: Option<String>,
    pub segredo_hash: String,
    pub codigo_pedido: String,
    pub nome_caixa: String,
    pub gerente: String,
    pub expira_em: String,
    pub usado_em: Option<String>,
    pub chave_aprovada: Option<String>,
}

/// Busca convites não expirados da loja `loja_id` (ativos ou recentemente consumidos pela
/// mesma chave para idempotência em queda).
pub fn convites_validos_agora(conn: &Connection, loja_id: &str, agora_iso: &str) -> Resultado<Vec<ConviteCandidato>> {
    limpar_convites_expirados(conn, agora_iso)?;
    let mut c = conn.prepare(
        "SELECT id, loja_id, segredo_ativo, segredo_hash, codigo_pedido, nome_caixa, gerente, expira_em, usado_em, chave_aprovada
           FROM p4_convite
          WHERE loja_id = ?1 AND expira_em > ?2",
    )?;
    let linhas = c
        .query_map(params![loja_id, agora_iso], |l| {
            Ok(ConviteCandidato {
                id: l.get(0)?,
                loja_id: l.get(1)?,
                segredo_normalizado: l.get(2)?,
                segredo_hash: l.get(3)?,
                codigo_pedido: l.get(4)?,
                nome_caixa: l.get(5)?,
                gerente: l.get(6)?,
                expira_em: l.get(7)?,
                usado_em: l.get(8)?,
                chave_aprovada: l.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(linhas)
}

/// Consome atomicamente o convite `convite_id` e aprova o `pedido` do caixa novo.
///
/// Regras de segurança e idempotência (`docs/PROPOSTA-D1-R01-protocolo-4.md` §3.3):
/// - Exige assinatura válida e `dispositivo` canônico não vazio (I3, M4).
/// - Exige que o convite pertença à mesma `loja_id` e não tenha expirado (B5).
/// - Recusa se o `codigo_pedido` do convite não bate com `pedido.codigo()`.
/// - Se já foi consumido pela **mesma** chave pública (`pedido.chave`), ainda dentro
///   da validade **e** a chave continua autorizada na cadeia (não retirada pós-consumo,
///   B5), devolve a aprovação existente sem duplicar (idempotente em queda).
/// - Se já foi consumido por **outra** chave (ou cancelado, ou retirado), recusa.
/// - Apaga `segredo_ativo = NULL` na mesma transação em que grava `caixa_aprovado`.
pub fn consumir_convite_e_aprovar(
    conn: &Connection,
    aprovador: &Identidade,
    raiz: &Chave,
    loja_id: &str,
    convite_id: &str,
    pedido: &Pedido,
    agora_iso: &str,
) -> Resultado<Aprovacao> {
    aprovados::validar_dispositivo(&pedido.dispositivo)?;
    if !pedido.confere() {
        return Err(Erro::validacao("O pedido do caixa novo não confere."));
    }
    struct LinhaConvite {
        loja_convite: String,
        codigo_esperado: String,
        nome_caixa: String,
        gerente: String,
        expira_em: String,
        usado_em: Option<String>,
        chave_aprovada: Option<String>,
    }
    let linha: Option<LinhaConvite> = conn
        .query_row(
            "SELECT loja_id, codigo_pedido, nome_caixa, gerente, expira_em, usado_em, chave_aprovada
               FROM p4_convite WHERE id = ?1",
            [convite_id],
            |l| {
                Ok(LinhaConvite {
                    loja_convite: l.get(0)?,
                    codigo_esperado: l.get(1)?,
                    nome_caixa: l.get(2)?,
                    gerente: l.get(3)?,
                    expira_em: l.get(4)?,
                    usado_em: l.get(5)?,
                    chave_aprovada: l.get(6)?,
                })
            },
        )
        .optional()?;
    let Some(LinhaConvite { loja_convite, codigo_esperado, nome_caixa, gerente, expira_em, usado_em, chave_aprovada }) =
        linha
    else {
        return Err(Erro::validacao("Convite inválido ou não encontrado."));
    };
    if loja_convite != loja_id {
        return Err(Erro::validacao("Este convite é de outra loja."));
    }
    if expira_em.as_str() <= agora_iso {
        let _ = conn.execute("UPDATE p4_convite SET segredo_ativo = NULL WHERE id = ?1", [convite_id]);
        return Err(Erro::validacao("Este convite venceu. Peça ao gerente para gerar um novo convite."));
    }
    if pedido.codigo() != codigo_esperado {
        return Err(Erro::validacao("O código deste caixa não confere com o código que o gerente aprovou no convite."));
    }
    let chave_hex = hex(&pedido.chave);
    if usado_em.is_some() {
        if chave_aprovada.as_deref() == Some(chave_hex.as_str()) {
            // B5: reavalia autorização atual antes de devolver aprovação idempotente
            // (se o caixa foi retirado após o consumo, a repetição é recusada!).
            if !aprovados::autorizada(conn, raiz, loja_id, &pedido.chave)? {
                return Err(Erro::validacao(
                    "Este caixa não está mais autorizado nesta loja. Peça ao gerente para aprovar novamente.",
                ));
            }
            let linhas = aprovados::linhas(conn, loja_id)?;
            if let Some(a) =
                linhas.aprovacoes.into_iter().find(|a| a.chave == pedido.chave && a.aprovador == aprovador.publica())
            {
                if a.dispositivo != pedido.dispositivo {
                    return Err(Erro::validacao(
                        "O identificador do dispositivo não confere com a aprovação já gravada para este convite.",
                    ));
                }
                return Ok(a);
            }
        }
        return Err(Erro::validacao("Este convite já foi usado ou cancelado. Peça um novo convite ao gerente."));
    }

    conn.execute_batch("SAVEPOINT p4_consumir_convite")?;
    let res = (|| -> Resultado<Aprovacao> {
        let aprovacao = aprovados::aprovar(conn, aprovador, raiz, loja_id, pedido, &nome_caixa, &gerente)?;
        let atualizados = conn.execute(
            "UPDATE p4_convite
                SET usado_em = ?2, chave_aprovada = ?3, segredo_ativo = NULL
              WHERE id = ?1 AND usado_em IS NULL",
            params![convite_id, agora_iso, chave_hex],
        )?;
        if atualizados != 1 {
            return Err(Erro::validacao("Este convite já foi usado."));
        }
        Ok(aprovacao)
    })();
    match res {
        Ok(a) => {
            conn.execute_batch("RELEASE p4_consumir_convite")?;
            Ok(a)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK TO p4_consumir_convite; RELEASE p4_consumir_convite");
            Err(e)
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;

    const LOJA: &str = "loja-teste-cofre";

    struct PastaTemp(PathBuf);
    impl PastaTemp {
        fn nova(nome: &str) -> Self {
            let p = std::env::temp_dir().join(format!("rustinho-p4-cofre-{nome}-{}", crate::tempo::novo_id()));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for PastaTemp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// B6: usa base de configuração isolada em pasta temporária (nunca toca no `$HOME`
    /// real do desenvolvedor nem quebra se `RUSTINHO_IDENTIDADE` estiver definida).
    #[test]
    fn identidade_cria_reabre_e_copia_do_banco_nao_leva_a_chave() {
        let raiz = PastaTemp::nova("isolado");
        let cfg = raiz.0.join("cfg");
        let p1 = raiz.0.join("pc1");
        let p2 = raiz.0.join("pc2");

        let c1 = caminho_identidade_com_base(&p1, false, Some(&cfg)).unwrap();
        let id1 = carregar_ou_criar_no_caminho(c1.clone(), false).unwrap();
        assert!(!id1.caminho.starts_with(&p1), "identidade normal fica fora da pasta do banco");
        assert!(id1.caminho.starts_with(&cfg), "identidade gravada na base isolada");

        let id1_re = carregar_ou_criar_no_caminho(c1.clone(), false).unwrap();
        assert_eq!(id1.identidade.publica(), id1_re.identidade.publica(), "reabre a mesma identidade");

        // Outro caminho de banco (simula banco copiado para outra pasta/PC): recebe outra identidade!
        let c2 = caminho_identidade_com_base(&p2, false, Some(&cfg)).unwrap();
        let id2 = carregar_ou_criar_no_caminho(c2, false).unwrap();
        assert_ne!(id1.identidade.publica(), id2.identidade.publica());

        // Nenhuma chave privada aparece em claro nos arquivos gravados.
        let (priv_ed, priv_x) = id1.identidade.privadas_para_teste();
        let bytes_id = std::fs::read(&id1.caminho).unwrap();
        assert!(!bytes_id.windows(priv_ed.len()).any(|w| w == priv_ed.as_slice()));
        assert!(!bytes_id.windows(priv_x.len()).any(|w| w == priv_x.as_slice()));

        // M3: reserva corrompida falha com erro claro; regerar recupera com identidade nova.
        let res = caminho_reserva(&id1.caminho);
        let mut corrompido = std::fs::read(&res).unwrap();
        let ult = corrompido.len() - 1;
        corrompido[ult] ^= 0xFF;
        std::fs::write(&res, &corrompido).unwrap();
        assert!(carregar_ou_criar_no_caminho(c1.clone(), false).is_err(), "reserva adulterada deve falhar");
        let novo = regerar_no_caminho(c1, false).unwrap();
        assert_ne!(novo.identidade.publica(), id1.identidade.publica(), "recuperação cria nova identidade");
    }

    #[test]
    fn modo_portatil_guarda_identidade_na_pasta_do_portatil() {
        let p = PastaTemp::nova("portatil");
        let c = caminho_identidade_com_base(&p.0, true, None).unwrap();
        let id = carregar_ou_criar_no_caminho(c.clone(), true).unwrap();
        assert_eq!(id.caminho, std::fs::canonicalize(&p.0).unwrap().join("identidade.p4"));
        let id2 = carregar_ou_criar_no_caminho(c, true).unwrap();
        assert_eq!(id.identidade.publica(), id2.identidade.publica());
    }

    /// A2: 6 threads competindo simultaneamente em `carregar_ou_criar` na mesma pasta
    /// obtêm SEMPRE a mesma identidade e a persistência reabre sem falha (20 rodadas).
    #[test]
    fn a2_corrida_concorrente_em_carregar_ou_criar_nao_corrompe() {
        let raiz = PastaTemp::nova("corrida");
        for rodada in 0..20 {
            let pasta = raiz.0.join(format!("r{rodada}"));
            let cfg = raiz.0.join(format!("cfg{rodada}"));
            let caminho = caminho_identidade_com_base(&pasta, false, Some(&cfg)).unwrap();
            let barreira = std::sync::Arc::new(std::sync::Barrier::new(6));
            let handles: Vec<_> = (0..6)
                .map(|_| {
                    let b = barreira.clone();
                    let c = caminho.clone();
                    std::thread::spawn(move || {
                        b.wait();
                        carregar_ou_criar_no_caminho(c, false).unwrap().identidade.publica()
                    })
                })
                .collect();
            let chaves: Vec<Chave> = handles.into_iter().map(|h| h.join().unwrap()).collect();
            for ch in &chaves[1..] {
                assert_eq!(*ch, chaves[0], "rodada {rodada}: todas as threads têm de obter a mesma chave");
            }
            let reaberta = carregar_ou_criar_no_caminho(caminho, false).unwrap().identidade.publica();
            assert_eq!(reaberta, chaves[0], "rodada {rodada}: identidade persistida reabre igual");
        }
    }

    /// A1: âncora de confiança fixada (`p4_raiz`) vence linha autoassinada falsa recebida via `juntar`
    /// e nunca é sobrescrita por `garantir_raiz_se_aplicavel`.
    #[test]
    fn a1_ancora_fixada_nao_e_sequestrada_por_raiz_falsa_juntada() {
        let (b, gerente) = crate::faixas::para_testes::loja("Caixa 1");
        let raiz_legitima = Identidade::gerar().unwrap();
        let r = garantir_raiz_se_aplicavel(&b.conn, &raiz_legitima, false, LOJA, "Caixa 1", &gerente, true)
            .unwrap()
            .unwrap();
        assert_eq!(r, raiz_legitima.publica());

        // Atacante cria linha autoassinada válida para a mesma loja e entra via `juntar`.
        let estranho = Identidade::gerar().unwrap();
        let falsa_raiz = aprovados::Aprovacao {
            chave: estranho.publica(),
            chave_troca: estranho.publica_troca(),
            loja_id: LOJA.into(),
            nome: "Raiz Falsa".into(),
            aprovador: estranho.publica(),
            gerente: gerente.clone(),
            aprovado_em: crate::tempo::agora(),
            dispositivo: "disp-atacante".into(),
            assinatura: [0u8; 64],
        };
        let mut falsa_raiz = falsa_raiz;
        falsa_raiz.assinatura = estranho.assinar(super::super::Dominio::Aprovacao, &falsa_raiz.corpo());
        aprovados::juntar(&b.conn, LOJA, &aprovados::Linhas { aprovacoes: vec![falsa_raiz], retiradas: Vec::new() })
            .unwrap();

        // A1: `raiz_atual` continua devolvendo o pino legítimo e `garantir_raiz_se_aplicavel` não o sobrescreve!
        assert_eq!(raiz_atual(&b.conn, LOJA).unwrap(), Some(raiz_legitima.publica()));
        assert_eq!(
            garantir_raiz_se_aplicavel(&b.conn, &raiz_legitima, false, LOJA, "Caixa 1", &gerente, true).unwrap(),
            Some(raiz_legitima.publica())
        );
        assert_eq!(
            crate::config::ler(&b.conn, "p4_raiz").unwrap().as_deref(),
            Some(hex(&raiz_legitima.publica()).as_str())
        );
    }

    /// M1: entrada não-ASCII em `normalizar_codigo_pedido` e `gerar_convite` nunca dá panic
    /// e é recusada com erro de validação (inclusive caracteres cuja conversão `as u8` colidiria).
    #[test]
    fn m1_codigo_pedido_nao_ascii_nao_da_panic_nem_passa() {
        let _ = normalizar_codigo_pedido("AÁÁÁÁÁA");
        let _ = normalizar_codigo_pedido("ŁŁŁŁ-ŁŁŁŁ-ŁŁŁŁ");
        assert!(!eh_codigo_pedido_valido(&normalizar_codigo_pedido("AÁÁÁÁÁA")));
        assert!(!eh_codigo_pedido_valido(&normalizar_codigo_pedido("ŁŁŁŁ-ŁŁŁŁ-ŁŁŁŁ")));

        let (b, gerente) = crate::faixas::para_testes::loja("Caixa 1");
        let raiz = Identidade::gerar().unwrap();
        garantir_raiz_se_aplicavel(&b.conn, &raiz, false, LOJA, "Caixa 1", &gerente, true).unwrap();
        assert!(gerar_convite(&b.conn, &raiz, &raiz.publica(), LOJA, "AÁÁÁÁÁA", "Caixa 2", &gerente, "").is_err());
        assert!(
            gerar_convite(&b.conn, &raiz, &raiz.publica(), LOJA, "ŁŁŁŁ-ŁŁŁŁ-ŁŁŁŁ", "Caixa 2", &gerente, "").is_err()
        );
    }

    #[test]
    fn convite_128_bits_uso_unico_expiracao_e_idempotencia_em_queda() {
        let (b, gerente) = crate::faixas::para_testes::loja("Caixa 1");
        let raiz = Identidade::gerar().unwrap();
        garantir_raiz_se_aplicavel(&b.conn, &raiz, false, LOJA, "Caixa 1", &gerente, true).unwrap();
        assert_eq!(raiz_atual(&b.conn, LOJA).unwrap(), Some(raiz.publica()));

        let novo = Identidade::gerar().unwrap();
        let pedido_novo = Pedido::com_dispositivo(&novo, "disp-caixa-2");
        let conv = gerar_convite(
            &b.conn,
            &raiz,
            &raiz.publica(),
            LOJA,
            &pedido_novo.codigo(),
            "Caixa 2",
            &gerente,
            "192.168.0.10:47810",
        )
        .unwrap();
        assert_eq!(normalizar_segredo(&conv.segredo).len(), 26, "128 bits em 26 chars Base32");
        assert!(conv.payload_qr.starts_with("RUST4|"));
        assert_eq!(listar_convites_ativos(&b.conn, LOJA).unwrap().len(), 1);
        assert!(listar_convites_ativos(&b.conn, "outra-loja").unwrap().is_empty());

        let agora = crate::tempo::agora();

        // M4: pedido sem `dispositivo` (vazio) é recusado no ingresso P4.
        let pedido_sem_disp = Pedido::de(&novo);
        assert!(
            consumir_convite_e_aprovar(&b.conn, &raiz, &raiz.publica(), LOJA, &conv.id, &pedido_sem_disp, &agora)
                .is_err(),
            "M4: dispositivo vazio deve ser recusado"
        );

        // B5: loja errada é recusada.
        assert!(
            consumir_convite_e_aprovar(&b.conn, &raiz, &raiz.publica(), "outra-loja", &conv.id, &pedido_novo, &agora)
                .is_err(),
            "B5: convite de outra loja deve ser recusado"
        );

        // Outro caixa (código de pedido diferente) é recusado com o mesmo convite.
        let intruso = Identidade::gerar().unwrap();
        let pedido_intruso = Pedido::com_dispositivo(&intruso, "disp-intruso");
        assert!(consumir_convite_e_aprovar(&b.conn, &raiz, &raiz.publica(), LOJA, &conv.id, &pedido_intruso, &agora)
            .is_err());

        // O caixa certo consome o convite com sucesso e apaga segredo_ativo.
        let a1 =
            consumir_convite_e_aprovar(&b.conn, &raiz, &raiz.publica(), LOJA, &conv.id, &pedido_novo, &agora).unwrap();
        assert_eq!(a1.chave, novo.publica());
        assert_eq!(a1.dispositivo, "disp-caixa-2");
        let seg_ativo: Option<String> =
            b.conn.query_row("SELECT segredo_ativo FROM p4_convite WHERE id = ?1", [&conv.id], |l| l.get(0)).unwrap();
        assert!(seg_ativo.is_none(), "segredo em claro limpo após consumo");
        assert!(listar_convites_ativos(&b.conn, LOJA).unwrap().is_empty());

        // Queda no meio da confirmação: mesmo caixa repetindo o mesmo convite antes de expirar é idempotente.
        let a2 =
            consumir_convite_e_aprovar(&b.conn, &raiz, &raiz.publica(), LOJA, &conv.id, &pedido_novo, &agora).unwrap();
        assert_eq!(a1, a2);
        // Replay com o mesmo par de chaves mas `dispositivo` divergente é recusado!
        let pedido_disp_outro = Pedido::com_dispositivo(&novo, "disp-outro");
        assert!(
            consumir_convite_e_aprovar(&b.conn, &raiz, &raiz.publica(), LOJA, &conv.id, &pedido_disp_outro, &agora)
                .is_err(),
            "replay idempotente exige mesmo dispositivo"
        );

        // B5: após retirar o caixa recém-aprovado, replay do mesmo convite é RECUSADO!
        aprovados::retirar(&b.conn, &raiz, &raiz.publica(), LOJA, &novo.publica(), "revogado logo após", &gerente)
            .unwrap();
        assert!(
            consumir_convite_e_aprovar(&b.conn, &raiz, &raiz.publica(), LOJA, &conv.id, &pedido_novo, &agora).is_err(),
            "B5: replay pós-retirada deve ser recusado"
        );

        // Convite expirado é recusado e purgado por `purgar_convites_expirados`.
        let novo3 = Identidade::gerar().unwrap();
        let p3 = Pedido::com_dispositivo(&novo3, "disp-3");
        let c3 = gerar_convite(&b.conn, &raiz, &raiz.publica(), LOJA, &p3.codigo(), "Caixa 3", &gerente, "").unwrap();
        let futuro = crate::tempo::somar_segundos(&agora, VALIDADE_CONVITE_SEG + 5).unwrap();
        assert!(
            consumir_convite_e_aprovar(&b.conn, &raiz, &raiz.publica(), LOJA, &c3.id, &p3, &futuro).is_err(),
            "convite vencido tem de ser recusado"
        );
        // Simula convite expirado há mais de 24 h e confere purga automática (B4).
        let antigo = crate::tempo::somar_segundos(&agora, -(RETENCAO_CONVITE_EXPIRADO_SEG + 60)).unwrap();
        b.conn.execute("UPDATE p4_convite SET expira_em = ?1 WHERE id = ?2", params![antigo, &c3.id]).unwrap();
        assert!(purgar_convites_expirados(&b.conn).unwrap() >= 1);
        let ainda: i64 =
            b.conn.query_row("SELECT COUNT(*) FROM p4_convite WHERE id = ?1", [&c3.id], |l| l.get(0)).unwrap();
        assert_eq!(ainda, 0, "convite expirado há >24 h removido na purga");

        // Cancelamento limpa segredo_ativo e impede uso.
        let c4 = gerar_convite(&b.conn, &raiz, &raiz.publica(), LOJA, &p3.codigo(), "Caixa 4", &gerente, "").unwrap();
        cancelar_convite(&b.conn, &c4.id, &gerente).unwrap();
        assert!(consumir_convite_e_aprovar(&b.conn, &raiz, &raiz.publica(), LOJA, &c4.id, &p3, &agora).is_err());

        // Garante que nenhuma tabela de convite vai para `alteracao` (estritamente local).
        let alt: i64 =
            b.conn.query_row("SELECT COUNT(*) FROM alteracao WHERE tabela = 'p4_convite'", [], |l| l.get(0)).unwrap();
        assert_eq!(alt, 0);
    }

    /// B7: banco migrado até a estrutura 28 com uma aprovação legada (sem coluna `dispositivo`)
    /// atualiza para a estrutura 29 via `Banco::abrir` preservando a validade da assinatura legada!
    #[test]
    fn b7_migracao_28_para_29_preserva_assinatura_legada() {
        let pasta = PastaTemp::nova("mig28-29");
        let db = pasta.0.join("rustinho.db");
        let raiz = Identidade::gerar().unwrap();
        let c2 = Identidade::gerar().unwrap();
        {
            let conn = Connection::open(&db).unwrap();
            crate::banco::registrar_funcoes(&conn).unwrap();
            // Aplica 0001..0028 diretamente dos arquivos reais (sem 0029 ainda).
            let mut arqs: Vec<PathBuf> = std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/migracoes"))
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("sql"))
                .collect();
            arqs.sort();
            assert!(arqs.len() >= 29);
            for (idx, arq) in arqs.iter().take(28).enumerate() {
                let sql = std::fs::read_to_string(arq).unwrap();
                let sem = sql.starts_with("-- rustinho: sem_chaves_estrangeiras");
                conn.pragma_update(None, "foreign_keys", !sem).unwrap();
                conn.pragma_update(None, "legacy_alter_table", sem).unwrap();
                conn.execute_batch(&sql).unwrap();
                conn.pragma_update(None, "user_version", (idx + 1) as i64).unwrap();
            }
            conn.pragma_update(None, "foreign_keys", true).unwrap();
            conn.pragma_update(None, "legacy_alter_table", false).unwrap();
            // Insere linhas assinadas no formato legado da estrutura 26..28 (sem coluna `dispositivo`).
            let mut a_raiz = aprovados::Aprovacao {
                chave: raiz.publica(),
                chave_troca: raiz.publica_troca(),
                loja_id: LOJA.into(),
                nome: "Caixa 1".into(),
                aprovador: raiz.publica(),
                gerente: "ana".into(),
                aprovado_em: "2026-10-01T12:00:00Z".into(),
                dispositivo: String::new(),
                assinatura: [0u8; 64],
            };
            a_raiz.assinatura = raiz.assinar(super::super::Dominio::Aprovacao, &a_raiz.corpo());
            let mut a_c2 = aprovados::Aprovacao {
                chave: c2.publica(),
                chave_troca: c2.publica_troca(),
                loja_id: LOJA.into(),
                nome: "Caixa 2".into(),
                aprovador: raiz.publica(),
                gerente: "ana".into(),
                aprovado_em: "2026-10-02T12:00:00Z".into(),
                dispositivo: String::new(),
                assinatura: [0u8; 64],
            };
            a_c2.assinatura = raiz.assinar(super::super::Dominio::Aprovacao, &a_c2.corpo());
            for a in [&a_raiz, &a_c2] {
                conn.execute(
                    "INSERT INTO caixa_aprovado (chave, aprovador, chave_troca, loja_id, nome, gerente, aprovado_em, assinatura)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        hex(&a.chave),
                        hex(&a.aprovador),
                        hex(&a.chave_troca),
                        a.loja_id,
                        a.nome,
                        a.gerente,
                        a.aprovado_em,
                        hex(&a.assinatura),
                    ],
                )
                .unwrap();
            }
        }
        // Abre pelo programa: aplica migração 0029 (`dispositivo DEFAULT ''` + `p4_convite`).
        let b = Banco::abrir(&db).unwrap();
        assert!(!b.em_modo_seguro());
        assert_eq!(b.conn.query_row("PRAGMA user_version", [], |l| l.get::<_, i64>(0)).unwrap(), 29);
        assert!(
            aprovados::autorizada(&b.conn, &raiz.publica(), LOJA, &c2.publica()).unwrap(),
            "B7: aprovação legada da 28 continua válida após migração 0029"
        );
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_protege_e_desprotege_ida_e_volta() {
        let segredo = "segredo-dpapi-teste-0919-dev2";
        let protegido = dpapi::proteger(segredo.as_bytes()).unwrap();
        assert_ne!(protegido, segredo.as_bytes());
        let aberto = dpapi::desproteger(&protegido).unwrap();
        assert_eq!(aberto.0, segredo.as_bytes());
    }
}
