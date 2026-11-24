//! Guarda segura da identidade deste caixa (D4, A7, P4-2) e gestão de convites
//! de uso único (A6) — protocolo 4 (`0.9.19-dev.2`).
//!
//! ## Onde fica a identidade (`P4-2`, M5)
//! - **Nunca no banco SQLite:** copiar `rustinho.db` para outro computador não
//!   leva a chave privada (D1).
//! - Em instalação normal ([`Modo::Instalado`]): na pasta de configuração **local**
//!   do usuário (`$XDG_CONFIG_HOME/rustinho/` ou `$HOME/.config/rustinho/` no Linux;
//!   `%LOCALAPPDATA%\Rustinho-identidade\` no Windows — local como o banco, que fica em
//!   `%LOCALAPPDATA%\Rustinho\`, e não viaja em perfil móvel de domínio, F-03), diferenciada pelo hash do caminho
//!   canônico da pasta do banco para que duas pastas no mesmo computador (`RD-B`)
//!   tenham identidades independentes sem colidir. Se não houver pasta de
//!   configuração disponível, falha explicitamente (nunca cai silenciosamente na
//!   pasta do banco).
//! - No Rustinho portátil ([`Modo::Portatil`], F-02): na própria pasta do portátil
//!   (o portátil não grava nada fora dela), **uma identidade por computador**
//!   (`identidade-<computador>.p4`). Levar o pendrive a outro computador dá outra
//!   chave, que precisa de aprovação do gerente como qualquer caixa novo; o
//!   computador de antes continua com a dele (os arquivos de cada computador ficam
//!   no pendrive; não são apagados sozinhos, N-12). Uma cópia do pendrive usada
//!   **no mesmo** computador leva a mesma chave: nem o protocolo nem o `I3`
//!   distinguem esse clone (limite declarado).
//! - Nas compilações de teste (`cfg(test)` ou o recurso `ancoras-de-teste` dos
//!   testes de integração, DT-P4-09): dentro da pasta (temporária) do banco do
//!   teste, nunca em `~/.config/rustinho/` nem no chaveiro do usuário.
//!
//! ## Como o segredo de selagem é guardado (`D4`, `A7`, M3)
//! A identidade é sempre gravada selada com Argon2id + ChaCha20-Poly1305
//! ([`Identidade::guardar_em`]) usando uma senha aleatória de 256 bits própria
//! desta máquina (independente da senha do gerente).
//! - **Linux instalado:** tenta o serviço de senhas do sistema (`secret-tool` por
//!   caminho absoluto fixo, com prazo de 3 s, só com sessão D-Bus); quando
//!   ausente (ex.: ambiente headless/mínimo) ou quando ele falha, usa a reserva
//!   local e marca `cofre_sistema = false` para aviso explícito ao gerente
//!   (nunca silencioso).
//! - **Windows instalado:** protege o segredo via DPAPI nativo (`CryptProtectData`
//!   / `CryptUnprotectData` de `crypt32.dll`, `cofre_sistema = true`).
//! - **Portátil:** sempre a reserva local na pasta do portátil
//!   (`cofre_sistema = false`): o cofre do sistema gravaria fora da pasta do
//!   portátil.
//! - **Reserva local (`cofre_sistema = false`):** é **ofuscação**, não proteção
//!   forte: arquivo `0600` fora do banco, cifrado com chave tirada do caminho e de
//!   um dado do computador que **não** fica no banco nem no pendrive
//!   (`crate::computador::segredo_local`, N-01 e N-14: no Linux sai do `machine-id`,
//!   no Windows do MachineGuid). Protege contra quem tem só a cópia da pasta ou do
//!   pendrive; **não** protege contra quem tem acesso ao próprio computador, nem
//!   quando o sistema não informa a identidade do computador (limites declarados).
//!   Por isso o gerente é avisado. Se a identidade ficar ilegível (cofre ou
//!   computador mudou), [`regerar_identidade_local`] cria identidade limpa (só
//!   quando a atual não abre) e exige nova aprovação do gerente.
//! - **Nenhum gancho por variável de ambiente** vale no programa (F-09).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
// Só o `secret-tool` (Linux) usa prazo.
#[cfg(target_os = "linux")]
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

/// Compilação de teste: os unitários (`cfg(test)`) e os de integração (`nucleo/tests/`,
/// que ligam o recurso `ancoras-de-teste`, que o programa recusa). DT-P4-09: nelas a
/// identidade fica na pasta do banco do teste e o cofre do sistema não é usado.
const COMPILACAO_DE_TESTE: bool = cfg!(any(test, feature = "ancoras-de-teste"));

/// Onde este Rustinho roda (P4-2, F-02).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Modo {
    /// Instalação normal: identidade na pasta de configuração local do usuário.
    Instalado,
    /// Rustinho portátil: identidade na pasta do portátil, uma por computador.
    /// Só se monta com [`Modo::portatil_deste_computador`] (N-13).
    Portatil(Portatil),
}

/// O portátil num computador. O campo é privado (N-13): o valor sai só de
/// [`Modo::portatil_deste_computador`], que usa o `computador::segredo_local` —
/// **nunca** a `computador::atual` (a impressão), que fica gravada no banco do
/// pendrive e deixaria a chave da reserva ao alcance de quem copia o pendrive (N-01).
#[derive(Clone, PartialEq, Eq)]
pub struct Portatil {
    /// `None` quando o sistema não informa a identidade do computador.
    computador: Option<String>,
}

/// O `Debug` não mostra o segredo local do computador.
impl std::fmt::Debug for Portatil {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Portatil").field("computador", &self.computador.as_ref().map(|_| "<oculto>")).finish()
    }
}

impl Modo {
    /// O Rustinho portátil neste computador (chamar depois de `computador::definir`,
    /// como o resto do programa).
    pub fn portatil_deste_computador() -> Modo {
        Modo::Portatil(Portatil { computador: crate::computador::segredo_local() })
    }

    /// Só nos testes: o portátil num "computador" escolhido.
    #[cfg(test)]
    fn portatil_com(computador: Option<&str>) -> Modo {
        Modo::Portatil(Portatil { computador: computador.map(str::to_string) })
    }
}

/// O cofre de senhas do sistema que guarda o segredo de selagem (D4, F-06).
#[derive(Clone, Debug, PartialEq, Eq)]
enum CofreSistema {
    /// Sem cofre do sistema: só a reserva local (portátil, testes, sistema sem serviço de senhas).
    Nenhum,
    /// `secret-tool` (Linux), pelo caminho absoluto do executável.
    #[cfg(target_os = "linux")]
    SecretTool(PathBuf),
    /// DPAPI do usuário (Windows), com o segredo protegido num arquivo `.dpapi`.
    #[cfg(windows)]
    Dpapi,
}

/// O cofre do sistema que vale para `modo` neste processo.
fn cofre_do_sistema(modo: &Modo) -> CofreSistema {
    if COMPILACAO_DE_TESTE || matches!(modo, Modo::Portatil(_)) {
        return CofreSistema::Nenhum;
    }
    #[cfg(target_os = "linux")]
    {
        match caminho_secret_tool() {
            Some(bin) if dbus_sessao_disponivel() => CofreSistema::SecretTool(bin.to_path_buf()),
            _ => CofreSistema::Nenhum,
        }
    }
    #[cfg(windows)]
    {
        CofreSistema::Dpapi
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        CofreSistema::Nenhum
    }
}

/// Trava intra-processo que serializa criação/leitura inicial da identidade (A2).
fn trava_processo() -> &'static Mutex<()> {
    static M: OnceLock<Mutex<()>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(()))
}

/// Trava exclusiva entre processos (A2, B3-R2, F-01): a trava do **sistema** sobre um
/// arquivo `.lock` aberto — `flock(LOCK_EX)` no Unix (inclui o Android) e
/// `File::lock` no Windows. O sistema solta a trava quando o processo termina, de
/// qualquer jeito (fim normal, "Finalizar tarefa", queda de energia): um `.lock`
/// que sobrou no disco não prende ninguém (antes, no Windows, a trava era o próprio
/// arquivo criado com `create_new` e um processo morto na seção crítica deixava a
/// identidade inacessível até alguém apagar o arquivo à mão).
struct TravaArquivo {
    _arquivo: std::fs::File,
}

/// O arquivo da trava da identidade em `caminho_id`.
fn caminho_da_trava(caminho_id: &Path) -> PathBuf {
    let mut os = caminho_id.as_os_str().to_os_string();
    os.push(".lock");
    PathBuf::from(os)
}

impl TravaArquivo {
    fn adquirir(caminho_id: &Path) -> Resultado<TravaArquivo> {
        let caminho = caminho_da_trava(caminho_id);
        let pasta = caminho.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        criar_pasta_restrita(pasta)?;
        let mut opcoes = std::fs::OpenOptions::new();
        opcoes.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opcoes.mode(0o600);
        }
        let arquivo = opcoes.open(&caminho).map_err(|e| {
            Erro::Interno(format!("não foi possível abrir a trava da identidade do caixa ({:?})", e.kind()))
        })?;
        travar_exclusivo(&arquivo)?;
        Ok(TravaArquivo { _arquivo: arquivo })
    }
}

/// N-15: o texto da tela quando outro Rustinho segura a identidade além do prazo.
pub const TEXTO_IDENTIDADE_OCUPADA: &str =
    "O Rustinho ainda está guardando a identidade deste caixa em outra janela. Tente de novo em instantes.";

/// Quanto se espera pela trava da identidade (N-02): um processo preso na seção
/// crítica não congela quem chama (a tela inclusive); depois disso, erro.
const PRAZO_DA_TRAVA: std::time::Duration = std::time::Duration::from_secs(10);

/// Pega a trava exclusiva do sistema sobre `arquivo` (solta ao fechar o arquivo),
/// tentando sem bloquear até [`PRAZO_DA_TRAVA`].
fn travar_exclusivo(arquivo: &std::fs::File) -> Resultado<()> {
    let fim = std::time::Instant::now() + PRAZO_DA_TRAVA;
    loop {
        match tentar_travar(arquivo) {
            Ok(()) => return Ok(()),
            Err(std::fs::TryLockError::WouldBlock) if std::time::Instant::now() < fim => {
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                crate::registro::anotar("protocolo 4: tempo esgotado esperando a trava da identidade do caixa");
                return Err(Erro::validacao(TEXTO_IDENTIDADE_OCUPADA));
            }
            Err(std::fs::TryLockError::Error(e)) => {
                return Err(Erro::Interno(format!(
                    "não foi possível travar o arquivo da identidade do caixa ({:?})",
                    e.kind()
                )));
            }
        }
    }
}

/// Uma tentativa, sem esperar: `flock(LOCK_EX | LOCK_NB)` em todo Unix (o mesmo da
/// trava do banco, `api::tentar_travar`; a biblioteca do Android não tem `try_lock`).
#[cfg(unix)]
fn tentar_travar(arquivo: &std::fs::File) -> Result<(), std::fs::TryLockError> {
    use std::os::unix::io::AsRawFd;
    loop {
        // SAFETY: o descritor é de `arquivo`, aberto durante toda a chamada; `flock`
        // só recebe o número e não mexe em memória do Rust.
        if unsafe { libc::flock(arquivo.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return Ok(());
        }
        let e = std::io::Error::last_os_error();
        match e.kind() {
            std::io::ErrorKind::Interrupted => continue,
            std::io::ErrorKind::WouldBlock => return Err(std::fs::TryLockError::WouldBlock),
            _ => return Err(std::fs::TryLockError::Error(e)),
        }
    }
}

/// Uma tentativa, sem esperar (Windows: a da biblioteca).
#[cfg(not(unix))]
fn tentar_travar(arquivo: &std::fs::File) -> Result<(), std::fs::TryLockError> {
    arquivo.try_lock()
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

/// O nome do arquivo da identidade do portátil **neste computador** (F-02).
fn nome_identidade_portatil(computador: &Option<String>) -> Resultado<String> {
    let Some(c) = computador.as_deref().map(str::trim).filter(|c| !c.is_empty()) else {
        return Err(Erro::validacao(
            "Este computador não informa a própria identidade, então o Rustinho portátil não consegue guardar a \
             chave deste caixa nele.",
        ));
    };
    let mut h = Sha256::new();
    h.update(b"rustinho/p4/portatil-computador\0");
    h.update(c.as_bytes());
    let d: [u8; 32] = h.finalize().into();
    Ok(format!("identidade-{}.p4", hex(&d[..8])))
}

/// Calcula o caminho do arquivo `identidade*.p4` para `pasta_banco` (P4-2, M5,
/// F-02, DT-P4-09). Cria a pasta do banco se ela ainda não existe.
///
/// Sem depender de `crate::portatil` (zero ciclos no grafo de dependências): quem
/// chama diz o [`Modo`]. Em modo instalado, exige pasta de configuração do usuário
/// (nunca cai dentro da pasta do banco em silêncio).
pub fn caminho_identidade(pasta_banco: &Path, modo: &Modo) -> Resultado<PathBuf> {
    let canon = caminho_canonico_estavel(pasta_banco)?;
    caminho_identidade_em(&canon, modo, None)
}

/// Como [`caminho_identidade`], mas sem criar nada (F-16): `None` se a pasta do
/// banco ainda não existe.
fn caminho_identidade_se_existir(pasta_banco: &Path, modo: &Modo) -> Resultado<Option<PathBuf>> {
    let Ok(canon) = std::fs::canonicalize(pasta_banco) else {
        return Ok(None);
    };
    caminho_identidade_em(&canon, modo, None).map(Some)
}

/// O caminho da identidade para a pasta do banco já canônica `canon`.
fn caminho_identidade_em(canon: &Path, modo: &Modo, base_cfg_override: Option<&Path>) -> Resultado<PathBuf> {
    if let Modo::Portatil(p) = modo {
        return Ok(canon.join(nome_identidade_portatil(&p.computador)?));
    }
    let pasta_cfg = match base_cfg_override {
        Some(p) => p.to_path_buf(),
        // DT-P4-09: nos testes, nunca a pasta de configuração real do usuário.
        None if COMPILACAO_DE_TESTE => canon.join(".identidade-teste"),
        None => pasta_config_usuario().ok_or_else(|| {
            Erro::validacao(
                "Não foi possível localizar a pasta de configuração do usuário (HOME/LOCALAPPDATA) para guardar a \
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

/// Só nos testes: o caminho da identidade com a base de configuração escolhida.
#[cfg(test)]
fn caminho_identidade_com_base(pasta_banco: &Path, modo: &Modo, base: Option<&Path>) -> Resultado<PathBuf> {
    let canon = caminho_canonico_estavel(pasta_banco)?;
    caminho_identidade_em(&canon, modo, base)
}

fn pasta_config_usuario() -> Option<PathBuf> {
    pasta_config_de(&|v| std::env::var_os(v), cfg!(windows))
}

/// A pasta de configuração **local** do usuário para a identidade (F-03).
/// - Windows: `%LOCALAPPDATA%\Rustinho-identidade` — local como o banco (que fica em
///   `%LOCALAPPDATA%\Rustinho`), nunca `%APPDATA%` (*Roaming*: num perfil móvel de
///   domínio a identidade viajaria e dois computadores teriam a mesma chave), e
///   fora da pasta do banco (copiar a pasta dos dados não leva a identidade).
/// - Outros: `$XDG_CONFIG_HOME/rustinho` ou `$HOME/.config/rustinho` (o banco fica em
///   `~/.local/share/rustinho`).
fn pasta_config_de(var: &dyn Fn(&str) -> Option<std::ffi::OsString>, windows: bool) -> Option<PathBuf> {
    let nao_vazia = |n: &str| var(n).filter(|v| !v.to_string_lossy().trim().is_empty()).map(PathBuf::from);
    if windows {
        return nao_vazia("LOCALAPPDATA").map(|p| p.join("Rustinho-identidade"));
    }
    nao_vazia("XDG_CONFIG_HOME")
        .map(|p| p.join("rustinho"))
        .or_else(|| nao_vazia("HOME").map(|p| p.join(".config").join("rustinho")))
}

/// A conta do segredo no serviço de senhas (só o `secret-tool` usa conta). N-04:
/// depois de uma identidade nova (recuperação), a conta leva também a geração
/// dela ([`caminho_geracao`]): o segredo novo nunca vai por cima do antigo.
#[cfg(target_os = "linux")]
fn conta_cofre(caminho: &Path) -> String {
    let mut h = Sha256::new();
    h.update(b"rustinho/p4/cofre-conta\0");
    h.update(caminho.to_string_lossy().as_bytes());
    if let Ok(g) = std::fs::read_to_string(caminho_geracao(caminho)) {
        let g = g.trim();
        if !g.is_empty() {
            h.update(b"\0geracao\0");
            h.update(g.as_bytes());
        }
    }
    let d: [u8; 32] = h.finalize().into();
    format!("caixa-{}", hex(&d[..12]))
}

/// N-04: o arquivo com a geração da identidade em `caminho_id` (só existe depois
/// de uma identidade nova gerada pela recuperação).
fn caminho_geracao(caminho_id: &Path) -> PathBuf {
    let mut os = caminho_id.as_os_str().to_os_string();
    os.push(".conta");
    PathBuf::from(os)
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
/// threads e processos (A2). Se a identidade existe e não abre, devolve erro
/// (falha fechada): quem chama trata como "protocolo 4 indisponível agora".
pub fn carregar_ou_criar(pasta_banco: &Path, modo: &Modo) -> Resultado<IdentidadeLocal> {
    let caminho = caminho_identidade(pasta_banco, modo)?;
    carregar_ou_criar_no_caminho(caminho, modo, &cofre_do_sistema(modo))
}

fn carregar_ou_criar_no_caminho(caminho: PathBuf, modo: &Modo, cofre: &CofreSistema) -> Resultado<IdentidadeLocal> {
    let _guarda_proc = trava_processo().lock().unwrap_or_else(|e| e.into_inner());
    let _guarda_arq = TravaArquivo::adquirir(&caminho)?;
    if caminho.exists() {
        return abrir_no_caminho(caminho, modo, cofre);
    }
    criar_no_caminho(caminho, modo, cofre)
}

/// Abre a identidade gravada em `caminho` (com as travas já tomadas por quem chama).
fn abrir_no_caminho(caminho: PathBuf, modo: &Modo, cofre: &CofreSistema) -> Resultado<IdentidadeLocal> {
    let (senha, cofre_sistema) = carregar_segredo(&caminho, modo, cofre)?;
    let senha_str = std::str::from_utf8(&senha.0)
        .map_err(|_| Erro::validacao("O segredo protegido da identidade está danificado."))?;
    let identidade = Identidade::abrir_de(&caminho, senha_str)?;
    Ok(IdentidadeLocal { identidade, cofre_sistema, caminho })
}

/// Gera, sela e grava uma identidade nova em `caminho` (com as travas já tomadas).
fn criar_no_caminho(caminho: PathBuf, modo: &Modo, cofre: &CofreSistema) -> Resultado<IdentidadeLocal> {
    let identidade = Identidade::gerar()?;
    let mut bruto: [u8; 32] = aleatorio()?;
    let senha = Segredo(hex(&bruto).into_bytes());
    apagar(&mut bruto);
    let senha_str = std::str::from_utf8(&senha.0).map_err(|_| Erro::Interno("segredo aleatório não UTF-8".into()))?;
    let cofre_sistema = guardar_segredo(&caminho, senha_str, modo, cofre)?;
    if let Err(e) = identidade.guardar_em(&caminho, senha_str) {
        // Reverte o segredo recém-guardado fora do cofre se a identidade falhou ao gravar.
        let _ = std::fs::remove_file(caminho_reserva(&caminho));
        #[cfg(windows)]
        let _ = std::fs::remove_file(caminho_dpapi(&caminho));
        return Err(e);
    }
    Ok(IdentidadeLocal { identidade, cofre_sistema, caminho })
}

/// As identidades já abertas neste processo, por caminho (DT-P4-08).
fn cache_identidades() -> &'static Mutex<std::collections::HashMap<PathBuf, std::sync::Arc<IdentidadeLocal>>> {
    static CACHE: OnceLock<Mutex<std::collections::HashMap<PathBuf, std::sync::Arc<IdentidadeLocal>>>> =
        OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// DT-P4-08 (N-16): a identidade deste caixa para o motor e os comandos, aberta
/// **uma vez por processo** (o selo custa um Argon2id e o cofre do sistema pode
/// levar segundos): as chamadas seguintes devolvem a mesma, da memória. Erro não
/// fica guardado (quem chama trata como "protocolo 4 indisponível agora" e tenta
/// depois; nunca chama [`regerar_identidade_local`] sozinho). Não chamar na
/// linha de execução da tela.
pub fn identidade_do_processo(pasta_banco: &Path, modo: &Modo) -> Resultado<std::sync::Arc<IdentidadeLocal>> {
    let caminho = caminho_identidade(pasta_banco, modo)?;
    if let Some(id) = cache_identidades().lock().unwrap_or_else(|e| e.into_inner()).get(&caminho) {
        return Ok(id.clone());
    }
    let nova = std::sync::Arc::new(carregar_ou_criar_no_caminho(caminho.clone(), modo, &cofre_do_sistema(modo))?);
    Ok(cache_identidades().lock().unwrap_or_else(|e| e.into_inner()).entry(caminho).or_insert(nova).clone())
}

/// Recuperação explícita (M3): arquiva uma identidade local **ilegível** (ex.: após
/// perda do cofre do SO ou troca de `machine-id` em VM) e gera uma identidade
/// nova limpa, que exigirá nova aprovação pelo gerente da loja.
///
/// F-07: se a identidade atual ainda abre, recusa (gerar outra só jogaria fora uma
/// chave boa e aprovada; o arquivo antigo arquivado ficaria ilegível para sempre,
/// porque o segredo dele é substituído no cofre). A autorização do gerente para
/// usar isto fica com a camada de cima.
pub fn regerar_identidade_local(pasta_banco: &Path, modo: &Modo) -> Resultado<std::sync::Arc<IdentidadeLocal>> {
    let caminho = caminho_identidade(pasta_banco, modo)?;
    let nova = std::sync::Arc::new(regerar_no_caminho(caminho.clone(), modo, &cofre_do_sistema(modo))?);
    // DT-P4-08: o processo passa a usar a identidade nova.
    cache_identidades().lock().unwrap_or_else(|e| e.into_inner()).insert(caminho, nova.clone());
    Ok(nova)
}

/// N-04: a identidade antiga sai do lugar **com o segredo dela** (nada é apagado
/// nem sobrescrito): o arquivo, a reserva local, o arquivo do DPAPI e a geração vão
/// para `<identidade>.antiga-<id>…`, com um bilhete para o suporte que diz onde o
/// segredo ficou (a conta do cofre do sistema, que não é mexida). A identidade nova
/// ganha uma geração nova: o segredo dela vai para **outra** conta do cofre. Assim,
/// se era só o cofre fora do ar, a identidade antiga ainda pode ser recuperada.
fn regerar_no_caminho(caminho: PathBuf, modo: &Modo, cofre: &CofreSistema) -> Resultado<IdentidadeLocal> {
    let _guarda_proc = trava_processo().lock().unwrap_or_else(|e| e.into_inner());
    let _guarda_arq = TravaArquivo::adquirir(&caminho)?;
    let arquivar = |de: &Path, para: &Path| -> Resultado<()> {
        if de.exists() {
            std::fs::rename(de, para).map_err(|e| {
                Erro::Interno(format!("não foi possível arquivar a identidade anterior ({:?})", e.kind()))
            })?;
        }
        Ok(())
    };
    if caminho.exists() {
        if abrir_no_caminho(caminho.clone(), modo, cofre).is_ok() {
            return Err(Erro::validacao("A identidade deste caixa continua funcionando: não é preciso gerar outra."));
        }
        let mut nome = caminho.as_os_str().to_os_string();
        nome.push(format!(".antiga-{}", crate::tempo::novo_id()));
        let arquivada = PathBuf::from(nome);
        #[cfg(target_os = "linux")]
        let conta = conta_cofre(&caminho);
        #[cfg(not(target_os = "linux"))]
        let conta = String::from("(sem conta: DPAPI ou reserva local)");
        arquivar(&caminho, &arquivada)?;
        arquivar(&caminho_reserva(&caminho), &caminho_reserva(&arquivada))?;
        #[cfg(windows)]
        arquivar(&caminho_dpapi(&caminho), &caminho_dpapi(&arquivada))?;
        arquivar(&caminho_geracao(&caminho), &caminho_geracao(&arquivada))?;
        let mut bilhete = arquivada.as_os_str().to_os_string();
        bilhete.push(".leia-me.txt");
        let _ = std::fs::write(
            PathBuf::from(bilhete),
            format!(
                "Identidade antiga deste caixa, arquivada em {} porque não abria.\n\
                 Caminho original: {}\n\
                 O segredo dela continua onde estava: no cofre de senhas do sistema (serviço rustinho-p4, conta {conta}),\n\
                 ou no arquivo .segredo / .dpapi arquivado ao lado (cifrado para o caminho original).\n",
                crate::tempo::agora(),
                caminho.display()
            ),
        );
    } else {
        // Sem identidade no lugar: o que sobrou dela (reserva, geração) também não é apagado.
        let sobra = format!(".sobra-{}", crate::tempo::novo_id());
        for de in [caminho_reserva(&caminho), caminho_geracao(&caminho)] {
            let mut para = de.as_os_str().to_os_string();
            para.push(&sobra);
            arquivar(&de, &PathBuf::from(para))?;
        }
    }
    // N-04: a geração nova (o segredo novo vai para outra conta do cofre do sistema).
    let mut geracao: [u8; 16] = aleatorio()?;
    std::fs::write(caminho_geracao(&caminho), hex(&geracao))
        .map_err(|e| Erro::Interno(format!("não foi possível gravar a geração da identidade ({:?})", e.kind())))?;
    apagar(&mut geracao);
    criar_no_caminho(caminho, modo, cofre)
}

/// Carrega apenas se já existir identidade. F-16: quando não existe, não cria
/// nada (nem a pasta do banco, nem a de configuração, nem a trava).
pub fn carregar_existente(pasta_banco: &Path, modo: &Modo) -> Resultado<Option<IdentidadeLocal>> {
    let Some(caminho) = caminho_identidade_se_existir(pasta_banco, modo)? else {
        return Ok(None);
    };
    carregar_existente_no_caminho(caminho, modo, &cofre_do_sistema(modo))
}

fn carregar_existente_no_caminho(
    caminho: PathBuf,
    modo: &Modo,
    cofre: &CofreSistema,
) -> Resultado<Option<IdentidadeLocal>> {
    if !caminho.exists() {
        return Ok(None);
    }
    let _guarda_proc = trava_processo().lock().unwrap_or_else(|e| e.into_inner());
    let _guarda_arq = TravaArquivo::adquirir(&caminho)?;
    if !caminho.exists() {
        return Ok(None);
    }
    abrir_no_caminho(caminho, modo, cofre).map(Some)
}

/// Guarda o segredo de selagem: no cofre do sistema quando há um e ele aceita;
/// senão na reserva local. Devolve `true` quando ficou no cofre do sistema.
fn guardar_segredo(caminho_id: &Path, senha: &str, modo: &Modo, cofre: &CofreSistema) -> Resultado<bool> {
    match cofre {
        CofreSistema::Nenhum => {}
        #[cfg(target_os = "linux")]
        CofreSistema::SecretTool(bin) => {
            if secret_tool_guardar(bin, &conta_cofre(caminho_id), senha) {
                return Ok(true);
            }
        }
        #[cfg(windows)]
        CofreSistema::Dpapi => {
            if dpapi_guardar(&caminho_dpapi(caminho_id), senha).is_ok() {
                return Ok(true);
            }
        }
    }
    guardar_reserva_local(&caminho_reserva(caminho_id), caminho_id, senha, modo)?;
    Ok(false)
}

/// Lê o segredo de selagem: primeiro o cofre do sistema, depois a reserva local.
/// Sem nenhum dos dois, erro (falha fechada: nunca cria identidade por cima).
fn carregar_segredo(caminho_id: &Path, modo: &Modo, cofre: &CofreSistema) -> Resultado<(Segredo, bool)> {
    match cofre {
        CofreSistema::Nenhum => {}
        #[cfg(target_os = "linux")]
        CofreSistema::SecretTool(bin) => {
            if let Some(s) = secret_tool_ler(bin, &conta_cofre(caminho_id)) {
                return Ok((s, true));
            }
        }
        #[cfg(windows)]
        CofreSistema::Dpapi => {
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
        let s = ler_reserva_local(&r, caminho_id, modo)?;
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

/// Guarda `segredo` no serviço de senhas pelo `secret-tool` em `bin` (stdin, prazo de 3 s).
/// `false` em qualquer falha (quem chama usa a reserva local e avisa o gerente).
#[cfg(target_os = "linux")]
fn secret_tool_guardar(bin: &Path, conta: &str, segredo: &str) -> bool {
    use std::process::{Command, Stdio};
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

/// Lê o segredo do serviço de senhas pelo `secret-tool` em `bin` (prazo de 3 s).
#[cfg(target_os = "linux")]
fn secret_tool_ler(bin: &Path, conta: &str) -> Option<Segredo> {
    use std::io::Read;
    use std::process::{Command, Stdio};
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
            return Err(Erro::Interno(
                "o cofre de senhas do Windows não conseguiu proteger a identidade do caixa".into(),
            ));
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
            return Err(Erro::validacao("O cofre de senhas do Windows não conseguiu abrir a identidade deste caixa."));
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

/// Chave de ofuscação da reserva local (D4 / M3, F-02, N-01): usada **somente**
/// quando não há cofre do sistema (`cofre_sistema = false`, aviso explícito ao
/// gerente). É ofuscação, não proteção forte: deriva do caminho e de um dado do
/// computador que não fica no banco nem no pendrive (`/etc/machine-id`; no
/// portátil, o `computador::segredo_local`). Protege contra quem tem só a cópia da
/// pasta ou do pendrive e contra a leitura casual; não contra quem tem acesso ao
/// próprio computador. A permissão `0600` e a pasta separada do banco fazem o resto.
fn chave_reserva_local(caminho_id: &Path, modo: &Modo) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"rustinho/p4/reserva-local/v1\0");
    match modo {
        Modo::Portatil(p) => {
            h.update(b"portatil\0");
            h.update(p.computador.as_deref().unwrap_or_default().trim().as_bytes());
            h.update(b"\0");
            if let Some(nome) = caminho_id.file_name() {
                h.update(nome.to_string_lossy().as_bytes());
            }
        }
        Modo::Instalado => {
            h.update(b"instalado\0");
            h.update(caminho_id.to_string_lossy().as_bytes());
            h.update(b"\0");
            // N-14: o mesmo dado do computador do portátil (no Linux, o `machine-id`;
            // no Windows, o MachineGuid), que não fica no banco nem na pasta. Sem a
            // identidade do sistema, só o caminho: aí a reserva não protege a cópia.
            if let Some(local) = crate::computador::segredo_local() {
                h.update(local.as_bytes());
            }
        }
    }
    h.finalize().into()
}

const MAGICA_RESERVA: &[u8; 4] = b"RSR1";

fn guardar_reserva_local(caminho: &Path, caminho_id: &Path, segredo: &str, modo: &Modo) -> Resultado<()> {
    let mut chave = chave_reserva_local(caminho_id, modo);
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

fn ler_reserva_local(caminho: &Path, caminho_id: &Path, modo: &Modo) -> Resultado<Segredo> {
    let bytes = std::fs::read(caminho)
        .map_err(|e| Erro::Interno(format!("não foi possível ler reserva da identidade ({:?})", e.kind())))?;
    if bytes.len() < 16 + 16 || bytes[..4] != MAGICA_RESERVA[..] {
        return Err(Erro::validacao("Arquivo de reserva da identidade inválido."));
    }
    let (cabecalho, corpo) = bytes.split_at(16);
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&cabecalho[4..16]);
    let mut chave = chave_reserva_local(caminho_id, modo);
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

/// T009: o caixa novo fixa a raiz que a loja provou (pela prova do convite) depois
/// de juntar o registro dela. A1: um pino já fixado nunca é trocado (outra raiz é
/// recusada); e a raiz tem de ser a única raiz do registro desta loja.
pub fn fixar_raiz_recebida(conn: &Connection, raiz: &Chave, loja_id: &str) -> Resultado<()> {
    if let Some(pino) = pino_raiz(conn)? {
        if &pino == raiz {
            return Ok(());
        }
        return Err(Erro::validacao("Este caixa já pertence a outra loja (outra raiz). A entrada foi recusada."));
    }
    if aprovados::raiz_registrada(conn, loja_id)?.as_ref() != Some(raiz) {
        return Err(Erro::validacao("O registro de caixas que a loja mandou não confere com a raiz dela."));
    }
    crate::config::gravar(conn, "p4_raiz", &hex(raiz))
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
/// O `Debug` não mostra o segredo nem o QR (F-10).
#[derive(Clone, Serialize, Deserialize)]
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
/// O `Debug` não mostra o segredo (F-10).
#[derive(Clone, Serialize)]
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

impl std::fmt::Debug for ConviteGerado {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConviteGerado")
            .field("id", &self.id)
            .field("segredo", &"<oculto>")
            .field("codigo_pedido", &self.codigo_pedido)
            .field("nome_caixa", &self.nome_caixa)
            .field("expira_em", &self.expira_em)
            .field("payload_qr", &"<oculto>")
            .finish()
    }
}

impl std::fmt::Debug for ConviteAtivo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConviteAtivo")
            .field("id", &self.id)
            .field("segredo", &"<oculto>")
            .field("codigo_pedido", &self.codigo_pedido)
            .field("nome_caixa", &self.nome_caixa)
            .field("gerente", &self.gerente)
            .field("criado_em", &self.criado_em)
            .field("expira_em", &self.expira_em)
            .finish()
    }
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

/// T009: depois de restaurar um backup, nenhum convite que veio nele volta a valer
/// (o segredo em claro é apagado). Devolve quantos foram apagados.
pub fn apagar_segredos_de_convites(conn: &Connection) -> Resultado<usize> {
    Ok(conn.execute("UPDATE p4_convite SET segredo_ativo = NULL WHERE segredo_ativo IS NOT NULL", [])?)
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
/// O `Debug` não mostra o segredo nem o hash, e o segredo é apagado da memória ao sair de cena (F-10).
#[derive(Clone)]
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

impl std::fmt::Debug for ConviteCandidato {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConviteCandidato")
            .field("id", &self.id)
            .field("loja_id", &self.loja_id)
            .field("segredo_normalizado", &self.segredo_normalizado.as_ref().map(|_| "<oculto>"))
            .field("segredo_hash", &"<oculto>")
            .field("codigo_pedido", &self.codigo_pedido)
            .field("nome_caixa", &self.nome_caixa)
            .field("gerente", &self.gerente)
            .field("expira_em", &self.expira_em)
            .field("usado_em", &self.usado_em)
            .field("chave_aprovada", &self.chave_aprovada)
            .finish()
    }
}

impl Drop for ConviteCandidato {
    fn drop(&mut self) {
        if let Some(segredo) = self.segredo_normalizado.take() {
            let mut bytes = segredo.into_bytes();
            apagar(&mut bytes);
        }
    }
}

/// Busca convites não expirados da loja `loja_id` (ativos ou recentemente consumidos pela
/// mesma chave para idempotência em queda).
///
/// F-08: é só leitura (não apaga nada com o relógio de quem chama); a limpeza dos
/// vencidos fica com [`limpar_convites_expirados`] e [`purgar_convites_expirados`].
pub fn convites_validos_agora(conn: &Connection, loja_id: &str, agora_iso: &str) -> Resultado<Vec<ConviteCandidato>> {
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
    /// real do desenvolvedor).
    #[test]
    fn identidade_cria_reabre_e_copia_do_banco_nao_leva_a_chave() {
        let raiz = PastaTemp::nova("isolado");
        let cfg = raiz.0.join("cfg");
        let p1 = raiz.0.join("pc1");
        let p2 = raiz.0.join("pc2");

        let c1 = caminho_identidade_com_base(&p1, &Modo::Instalado, Some(&cfg)).unwrap();
        let id1 = carregar_ou_criar_no_caminho(c1.clone(), &Modo::Instalado, &CofreSistema::Nenhum).unwrap();
        assert!(!id1.caminho.starts_with(&p1), "identidade normal fica fora da pasta do banco");
        assert!(id1.caminho.starts_with(&cfg), "identidade gravada na base isolada");

        let id1_re = carregar_ou_criar_no_caminho(c1.clone(), &Modo::Instalado, &CofreSistema::Nenhum).unwrap();
        assert_eq!(id1.identidade.publica(), id1_re.identidade.publica(), "reabre a mesma identidade");

        // Outro caminho de banco (simula banco copiado para outra pasta/PC): recebe outra identidade!
        let c2 = caminho_identidade_com_base(&p2, &Modo::Instalado, Some(&cfg)).unwrap();
        let id2 = carregar_ou_criar_no_caminho(c2, &Modo::Instalado, &CofreSistema::Nenhum).unwrap();
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
        assert!(
            carregar_ou_criar_no_caminho(c1.clone(), &Modo::Instalado, &CofreSistema::Nenhum).is_err(),
            "reserva adulterada deve falhar"
        );
        let novo = regerar_no_caminho(c1, &Modo::Instalado, &CofreSistema::Nenhum).unwrap();
        assert_ne!(novo.identidade.publica(), id1.identidade.publica(), "recuperação cria nova identidade");
    }

    fn portatil_em(computador: &str) -> Modo {
        Modo::portatil_com(Some(computador))
    }

    #[test]
    fn modo_portatil_guarda_identidade_na_pasta_do_portatil() {
        let p = PastaTemp::nova("portatil");
        let modo = portatil_em("pc-da-loja");
        let c = caminho_identidade_com_base(&p.0, &modo, None).unwrap();
        let id = carregar_ou_criar_no_caminho(c.clone(), &modo, &CofreSistema::Nenhum).unwrap();
        let canon = std::fs::canonicalize(&p.0).unwrap();
        assert_eq!(id.caminho.parent(), Some(canon.as_path()), "a identidade fica na pasta do portátil");
        assert!(!id.cofre_sistema, "o portátil usa só a reserva na própria pasta (aviso ao gerente)");
        let id2 = carregar_ou_criar_no_caminho(c, &modo, &CofreSistema::Nenhum).unwrap();
        assert_eq!(id.identidade.publica(), id2.identidade.publica());
    }

    /// F-02: o pendrive levado a outro computador não fica preso numa identidade que
    /// não abre ali: o outro computador tem a própria (que precisará de aprovação), e
    /// a do primeiro computador continua intacta e abrindo nele.
    #[test]
    fn f02_portatil_tem_uma_identidade_por_computador() {
        let p = PastaTemp::nova("portatil-dois-pcs");
        let (pc_a, pc_b) = (portatil_em("pc-a"), portatil_em("pc-b"));
        let ca = caminho_identidade_com_base(&p.0, &pc_a, None).unwrap();
        let cb = caminho_identidade_com_base(&p.0, &pc_b, None).unwrap();
        assert_ne!(ca, cb, "cada computador tem o próprio arquivo de identidade no pendrive");
        let ida = carregar_ou_criar_no_caminho(ca.clone(), &pc_a, &CofreSistema::Nenhum).unwrap();
        let idb = carregar_ou_criar_no_caminho(cb, &pc_b, &CofreSistema::Nenhum).unwrap();
        assert_ne!(ida.identidade.publica(), idb.identidade.publica(), "outro computador, outra chave");
        let de_novo_a = carregar_ou_criar_no_caminho(ca.clone(), &pc_a, &CofreSistema::Nenhum).unwrap();
        assert_eq!(de_novo_a.identidade.publica(), ida.identidade.publica(), "o primeiro computador segue igual");
        // A reserva de um computador não abre com a identidade de outro (a chave da
        // reserva amarra o computador).
        assert!(ler_reserva_local(&caminho_reserva(&ca), &ca, &pc_b).is_err());
        // Sem a identidade do computador, o portátil não inventa uma: recusa.
        let sem = Modo::portatil_com(None);
        assert!(caminho_identidade_com_base(&p.0, &sem, None).is_err());
    }

    /// A2: 6 threads competindo simultaneamente em `carregar_ou_criar` na mesma pasta
    /// obtêm SEMPRE a mesma identidade e a persistência reabre sem falha (20 rodadas).
    #[test]
    fn a2_corrida_concorrente_em_carregar_ou_criar_nao_corrompe() {
        let raiz = PastaTemp::nova("corrida");
        for rodada in 0..20 {
            let pasta = raiz.0.join(format!("r{rodada}"));
            let cfg = raiz.0.join(format!("cfg{rodada}"));
            let caminho = caminho_identidade_com_base(&pasta, &Modo::Instalado, Some(&cfg)).unwrap();
            let barreira = std::sync::Arc::new(std::sync::Barrier::new(6));
            let handles: Vec<_> = (0..6)
                .map(|_| {
                    let b = barreira.clone();
                    let c = caminho.clone();
                    std::thread::spawn(move || {
                        b.wait();
                        carregar_ou_criar_no_caminho(c, &Modo::Instalado, &CofreSistema::Nenhum)
                            .unwrap()
                            .identidade
                            .publica()
                    })
                })
                .collect();
            let chaves: Vec<Chave> = handles.into_iter().map(|h| h.join().unwrap()).collect();
            for ch in &chaves[1..] {
                assert_eq!(*ch, chaves[0], "rodada {rodada}: todas as threads têm de obter a mesma chave");
            }
            let reaberta = carregar_ou_criar_no_caminho(caminho, &Modo::Instalado, &CofreSistema::Nenhum)
                .unwrap()
                .identidade
                .publica();
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

    // -----------------------------------------------------------------------
    // Revisão de entrada (Sonnet, 24/11): F-01, F-03, F-06, F-07, F-10, F-16 e DT-P4-09.
    // -----------------------------------------------------------------------

    /// F-01: um `.lock` que sobrou no disco (de um processo que morreu na seção
    /// crítica: "Finalizar tarefa", queda de energia) não prende a identidade. A
    /// trava é a do sistema, que o sistema solta quando o processo termina.
    #[test]
    fn f01_trava_que_sobrou_no_disco_nao_prende_a_identidade() {
        let raiz = PastaTemp::nova("trava-que-sobrou");
        let cfg = raiz.0.join("cfg");
        let c = caminho_identidade_com_base(&raiz.0.join("pc"), &Modo::Instalado, Some(&cfg)).unwrap();
        std::fs::create_dir_all(c.parent().unwrap()).unwrap();
        std::fs::write(caminho_da_trava(&c), b"4242").unwrap();
        let id = carregar_ou_criar_no_caminho(c.clone(), &Modo::Instalado, &CofreSistema::Nenhum)
            .expect("o .lock que sobrou não pode impedir criar a identidade");
        std::fs::write(caminho_da_trava(&c), b"4242").unwrap();
        let de_novo = carregar_ou_criar_no_caminho(c.clone(), &Modo::Instalado, &CofreSistema::Nenhum)
            .expect("nem impedir abrir de novo");
        assert_eq!(id.identidade.publica(), de_novo.identidade.publica());
        assert!(carregar_existente_no_caminho(c, &Modo::Instalado, &CofreSistema::Nenhum).unwrap().is_some());
    }

    /// O `flock` do util-linux com `--no-fork` (2.35+): o próprio processo vira o
    /// comando, sem filho que herde a trava (N-16).
    #[cfg(target_os = "linux")]
    fn flock_sem_fork() -> bool {
        std::process::Command::new("/usr/bin/flock")
            .arg("--help")
            .output()
            .map(|s| String::from_utf8_lossy(&s.stdout).contains("--no-fork"))
            .unwrap_or(false)
    }

    /// DT-P4-08: a identidade do processo abre uma vez (a segunda chamada devolve a
    /// mesma, da memória) e a recuperação explícita troca a do processo.
    #[test]
    fn dt_p4_08_identidade_do_processo_abre_uma_vez() {
        let p = PastaTemp::nova("cache-identidade");
        let a = identidade_do_processo(&p.0, &Modo::Instalado).unwrap();
        let b = identidade_do_processo(&p.0, &Modo::Instalado).unwrap();
        assert!(std::sync::Arc::ptr_eq(&a, &b), "a segunda chamada não pode abrir o selo de novo");
        // Com a reserva estragada, a do processo continua (está na memória)...
        let r = caminho_reserva(&a.caminho);
        let mut bytes = std::fs::read(&r).unwrap();
        let ult = bytes.len() - 1;
        bytes[ult] ^= 0xFF;
        std::fs::write(&r, &bytes).unwrap();
        assert!(std::sync::Arc::ptr_eq(&a, &identidade_do_processo(&p.0, &Modo::Instalado).unwrap()));
        // ...e a recuperação explícita troca a do processo pela nova.
        let nova = regerar_identidade_local(&p.0, &Modo::Instalado).unwrap();
        assert_ne!(nova.identidade.publica(), a.identidade.publica());
        let depois = identidade_do_processo(&p.0, &Modo::Instalado).unwrap();
        assert!(std::sync::Arc::ptr_eq(&nova, &depois));
    }

    /// N-13: o `Debug` do modo portátil não mostra o segredo local do computador.
    #[test]
    fn n13_debug_do_portatil_nao_mostra_o_segredo_local() {
        let segredo = crate::computador::segredo_local().unwrap();
        let texto = format!("{:?}", Modo::portatil_deste_computador());
        assert!(!texto.contains(&segredo), "{texto}");
    }

    /// N-14: a reserva do modo instalado também se amarra ao computador.
    #[test]
    fn n14_reserva_instalada_depende_do_computador() {
        let raiz = PastaTemp::nova("reserva-instalada");
        let c = caminho_identidade_com_base(&raiz.0.join("pc"), &Modo::Instalado, Some(&raiz.0.join("cfg"))).unwrap();
        let com = chave_reserva_local(&c, &Modo::Instalado);
        let mut h = Sha256::new();
        h.update(b"rustinho/p4/reserva-local/v1\0");
        h.update(b"instalado\0");
        h.update(c.to_string_lossy().as_bytes());
        h.update(b"\0");
        let so_caminho: [u8; 32] = h.finalize().into();
        assert_ne!(com, so_caminho, "a chave da reserva não pode depender só do caminho");
    }

    /// F-01 (no Linux): a trava presa por outro **processo** que morreu é solta pelo
    /// sistema na hora (o processo filho segura a trava e é morto com SIGKILL).
    #[cfg(target_os = "linux")]
    #[test]
    fn f01_trava_de_processo_morto_e_solta_pelo_sistema() {
        if !flock_sem_fork() {
            eprintln!("sem /usr/bin/flock com --no-fork neste sistema: teste não se aplica");
            return;
        }
        let raiz = PastaTemp::nova("trava-processo-morto");
        let c = caminho_identidade_com_base(&raiz.0.join("pc"), &Modo::Instalado, Some(&raiz.0.join("cfg"))).unwrap();
        std::fs::create_dir_all(c.parent().unwrap()).unwrap();
        let trava = caminho_da_trava(&c);
        // `--no-fork`: o próprio processo que pega a trava vira o `sleep` (sem filho que
        // herde a trava e continue vivo depois do SIGKILL).
        let mut filho = std::process::Command::new("/usr/bin/flock")
            .args(["--no-fork", "-x"])
            .arg(&trava)
            .args(["sleep", "60"])
            .spawn()
            .unwrap();
        // Espera o filho pegar a trava (o arquivo aparece e um `flock -n` falha).
        let fim = Instant::now() + Duration::from_secs(10);
        loop {
            let ocupada = std::process::Command::new("/usr/bin/flock")
                .args(["-n", "-x"])
                .arg(&trava)
                .arg("true")
                .status()
                .map(|s| !s.success())
                .unwrap_or(false);
            if ocupada {
                break;
            }
            assert!(Instant::now() < fim, "o processo filho não pegou a trava");
            std::thread::sleep(Duration::from_millis(20));
        }
        filho.kill().unwrap();
        filho.wait().unwrap();
        assert!(trava.exists(), "o arquivo da trava fica no disco depois da morte do processo");
        carregar_ou_criar_no_caminho(c, &Modo::Instalado, &CofreSistema::Nenhum)
            .expect("com o dono morto, a trava do sistema já está solta");
    }

    /// F-03: no Windows a identidade vai para a pasta **local** do usuário (como o
    /// banco), fora da pasta do banco; nunca para a *Roaming* (`%APPDATA%`).
    #[test]
    fn f03_no_windows_a_identidade_fica_na_pasta_local_fora_da_do_banco() {
        use std::ffi::OsString;
        let local = "C:\\Users\\caixa\\AppData\\Local";
        let roaming = "C:\\Users\\caixa\\AppData\\Roaming";
        let as_duas = |n: &str| match n {
            "LOCALAPPDATA" => Some(OsString::from(local)),
            "APPDATA" => Some(OsString::from(roaming)),
            _ => None,
        };
        let p = pasta_config_de(&as_duas, true).unwrap();
        assert!(p.starts_with(local), "a identidade fica na pasta local: {}", p.display());
        assert!(!p.to_string_lossy().contains("Roaming"));
        assert_ne!(p, PathBuf::from(local).join("Rustinho"), "e fora da pasta do banco (%LOCALAPPDATA%\\Rustinho)");
        let so_roaming = |n: &str| (n == "APPDATA").then(|| OsString::from(roaming));
        assert_eq!(pasta_config_de(&so_roaming, true), None, "sem pasta local, falha (nunca a Roaming)");
        let linux = |n: &str| (n == "HOME").then(|| OsString::from("/home/caixa"));
        assert_eq!(pasta_config_de(&linux, false), Some(PathBuf::from("/home/caixa/.config/rustinho")));
        let xdg = |n: &str| match n {
            "XDG_CONFIG_HOME" => Some(OsString::from("/cfg")),
            "HOME" => Some(OsString::from("/home/caixa")),
            _ => None,
        };
        assert_eq!(pasta_config_de(&xdg, false), Some(PathBuf::from("/cfg/rustinho")));
    }

    /// DT-P4-09: nas compilações de teste a identidade "instalada" fica dentro da
    /// pasta do banco do teste — nunca em `~/.config/rustinho/` (área do Rustinho
    /// real) — e o cofre do sistema nunca é usado (nem o chaveiro do usuário).
    #[test]
    fn dt_p4_09_testes_nao_tocam_a_configuracao_nem_o_chaveiro_do_usuario() {
        let p = PastaTemp::nova("isolamento-testes");
        let c = caminho_identidade(&p.0, &Modo::Instalado).unwrap();
        assert!(c.starts_with(std::fs::canonicalize(&p.0).unwrap()), "fora da pasta do teste: {}", c.display());
        assert_eq!(cofre_do_sistema(&Modo::Instalado), CofreSistema::Nenhum);
        let id = carregar_ou_criar(&p.0, &Modo::Instalado).unwrap();
        assert!(!id.cofre_sistema);
        assert!(id.caminho.starts_with(std::fs::canonicalize(&p.0).unwrap()));
    }

    /// F-16: ler a identidade que não existe não cria nada (nem a pasta do banco,
    /// nem a de configuração, nem a trava).
    #[test]
    fn f16_carregar_existente_sem_identidade_nao_cria_nada() {
        let p = PastaTemp::nova("leitura-sem-efeito");
        let inexistente = p.0.join("nao-existe");
        assert!(carregar_existente(&inexistente, &Modo::Instalado).unwrap().is_none());
        assert!(!inexistente.exists(), "a pasta do banco não pode ser criada por uma leitura");
        assert!(carregar_existente(&p.0, &Modo::Instalado).unwrap().is_none());
        let entradas: Vec<_> = std::fs::read_dir(&p.0).unwrap().collect();
        assert!(entradas.is_empty(), "a leitura deixou arquivos ou pastas para trás: {entradas:?}");
    }

    /// F-07: com a identidade legível, a recuperação recusa (não joga fora uma chave
    /// boa e aprovada); com ela ilegível, gera outra.
    #[test]
    fn f07_regerar_recusa_identidade_que_abre() {
        let raiz = PastaTemp::nova("regerar-guarda");
        let c = caminho_identidade_com_base(&raiz.0.join("pc"), &Modo::Instalado, Some(&raiz.0.join("cfg"))).unwrap();
        let id = carregar_ou_criar_no_caminho(c.clone(), &Modo::Instalado, &CofreSistema::Nenhum).unwrap();
        assert!(regerar_no_caminho(c.clone(), &Modo::Instalado, &CofreSistema::Nenhum).is_err());
        let igual = carregar_ou_criar_no_caminho(c.clone(), &Modo::Instalado, &CofreSistema::Nenhum).unwrap();
        assert_eq!(igual.identidade.publica(), id.identidade.publica(), "a identidade boa continua a mesma");
        let pasta = c.parent().unwrap();
        let antigas = std::fs::read_dir(pasta)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".antiga-"))
            .count();
        assert_eq!(antigas, 0, "nada foi arquivado");
    }

    /// F-10: o `Debug` dos convites não mostra o segredo, o hash nem o QR.
    #[test]
    fn f10_debug_dos_convites_nao_mostra_o_segredo() {
        let (b, gerente) = crate::faixas::para_testes::loja("Caixa 1");
        let raiz = Identidade::gerar().unwrap();
        aprovados::registrar_raiz(&b.conn, &raiz, LOJA, "Caixa 1", &gerente).unwrap();
        let novo = Identidade::gerar().unwrap();
        let pedido = Pedido::com_dispositivo(&novo, "disp-novo-0001");
        let conv =
            gerar_convite(&b.conn, &raiz, &raiz.publica(), LOJA, &pedido.codigo(), "Caixa 2", &gerente, "").unwrap();
        let norm = normalizar_segredo(&conv.segredo);
        let textos = [
            format!("{conv:?}"),
            format!("{:?}", listar_convites_ativos(&b.conn, LOJA).unwrap()),
            format!("{:?}", convites_validos_agora(&b.conn, LOJA, &crate::tempo::agora()).unwrap()),
        ];
        for t in &textos {
            assert!(!t.contains(&conv.segredo) && !t.contains(&norm), "o segredo apareceu: {t}");
            assert!(!t.contains(&hash_segredo(&norm)), "o hash apareceu: {t}");
            assert!(!t.contains("RUST4|"), "o QR apareceu: {t}");
        }
    }

    /// F-08: procurar os convites candidatos não grava nada (antes, a leitura com um
    /// relógio no futuro apagava o segredo de um convite ainda válido).
    #[test]
    fn f08_procurar_candidatos_nao_apaga_o_segredo_de_convite_valido() {
        let (b, gerente) = crate::faixas::para_testes::loja("Caixa 1");
        let raiz = Identidade::gerar().unwrap();
        aprovados::registrar_raiz(&b.conn, &raiz, LOJA, "Caixa 1", &gerente).unwrap();
        let pedido = Pedido::com_dispositivo(&Identidade::gerar().unwrap(), "disp-novo-0002");
        gerar_convite(&b.conn, &raiz, &raiz.publica(), LOJA, &pedido.codigo(), "Caixa 2", &gerente, "").unwrap();
        let futuro = crate::tempo::somar_segundos(&crate::tempo::agora(), VALIDADE_CONVITE_SEG + 60).unwrap();
        assert!(convites_validos_agora(&b.conn, LOJA, &futuro).unwrap().is_empty());
        let agora = convites_validos_agora(&b.conn, LOJA, &crate::tempo::agora()).unwrap();
        assert_eq!(agora.len(), 1);
        assert!(agora[0].segredo_normalizado.is_some(), "a leitura com outro relógio apagou o segredo");
    }

    // --- F-06: o cofre do sistema de verdade (o `secret-tool`), com um executável falso. ---

    /// Um `secret-tool` de mentira (o mesmo jeito de chamar do de verdade): `store`
    /// lê o segredo do stdin e guarda num arquivo por conta; `lookup` devolve com
    /// quebra de linha. `jeito`: "funciona", "falha" (sai com 1) ou "trava" (dorme 60 s).
    #[cfg(target_os = "linux")]
    fn secret_tool_falso(pasta: &Path, jeito: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let chaveiro = pasta.join("chaveiro");
        std::fs::create_dir_all(&chaveiro).unwrap();
        let corpo = match jeito {
            "funciona" => format!(
                "acao=\"$1\"\nconta=\"\"\nwhile [ $# -gt 0 ]; do [ \"$1\" = account ] && conta=\"$2\"; shift; done\n\
                 arq=\"{}/$conta\"\ncase \"$acao\" in\n  store) cat > \"$arq\" ;;\n  \
                 lookup) [ -f \"$arq\" ] || exit 1; cat \"$arq\"; echo ;;\n  *) exit 2 ;;\nesac\n",
                chaveiro.display()
            ),
            // N-04: o cofre que voltou só para gravar (a leitura do segredo antigo falha).
            "so_guarda" => format!(
                "acao=\"$1\"\nconta=\"\"\nwhile [ $# -gt 0 ]; do [ \"$1\" = account ] && conta=\"$2\"; shift; done\n\
                 arq=\"{}/$conta\"\ncase \"$acao\" in\n  store) cat > \"$arq\" ;;\n  *) exit 1 ;;\nesac\n",
                chaveiro.display()
            ),
            "falha" => "cat > /dev/null 2>&1; exit 1\n".to_string(),
            "trava" => "exec sleep 60\n".to_string(),
            outro => panic!("jeito desconhecido: {outro}"),
        };
        let bin = pasta.join(format!("secret-tool-{jeito}"));
        let temporario = pasta.join(format!(".secret-tool-{jeito}.novo"));
        std::fs::write(&temporario, format!("#!/bin/sh\n{corpo}")).unwrap();
        std::fs::set_permissions(&temporario, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::rename(&temporario, &bin).unwrap();
        // Outro teste pode ter herdado o arquivo aberto num `fork` (ETXTBSY): espera ele largar.
        let fim = Instant::now() + Duration::from_secs(10);
        loop {
            match std::process::Command::new(&bin)
                .arg("--sonda")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .spawn()
            {
                Ok(mut filho) => {
                    let _ = filho.kill();
                    let _ = filho.wait();
                    break;
                }
                Err(e) => {
                    assert!(Instant::now() < fim, "o secret-tool falso não executa: {e}");
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
        bin
    }

    /// N-04: o cofre do sistema não devolveu o segredo (fora do ar na hora) e o gerente
    /// gerou uma identidade nova. O segredo novo vai para **outra** conta do cofre: o
    /// antigo continua lá, e a identidade antiga (arquivada, com o bilhete para o
    /// suporte) ainda abre com ele.
    #[cfg(target_os = "linux")]
    #[test]
    fn n04_identidade_nova_nao_apaga_o_segredo_da_antiga() {
        let raiz = PastaTemp::nova("cofre-n04");
        let funciona = CofreSistema::SecretTool(secret_tool_falso(&raiz.0, "funciona"));
        let so_guarda = CofreSistema::SecretTool(secret_tool_falso(&raiz.0, "so_guarda"));
        let pasta_banco = raiz.0.join("dados");
        std::fs::create_dir_all(&pasta_banco).unwrap();
        let c = caminho_identidade_com_base(&pasta_banco, &Modo::Instalado, Some(&raiz.0.join("cfg"))).unwrap();
        let antiga = carregar_ou_criar_no_caminho(c.clone(), &Modo::Instalado, &funciona).unwrap();
        let conta_antiga = conta_cofre(&c);
        let chaveiro = raiz.0.join("chaveiro");
        let segredo_antigo = std::fs::read_to_string(chaveiro.join(&conta_antiga)).unwrap();

        // O cofre não devolve o segredo agora: a identidade não abre; o gerente gera outra.
        assert!(abrir_no_caminho(c.clone(), &Modo::Instalado, &so_guarda).is_err());
        let nova = regerar_no_caminho(c.clone(), &Modo::Instalado, &so_guarda).unwrap();
        assert_ne!(nova.identidade.publica(), antiga.identidade.publica());
        assert_ne!(conta_cofre(&c), conta_antiga, "o segredo novo vai para outra conta do cofre");
        assert_eq!(
            std::fs::read_to_string(chaveiro.join(&conta_antiga)).unwrap(),
            segredo_antigo,
            "o segredo da identidade antiga continua no cofre, sem mudança"
        );
        let arquivadas: Vec<PathBuf> = std::fs::read_dir(c.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.to_string_lossy().ends_with(".leia-me.txt"))
            .collect();
        assert_eq!(arquivadas.len(), 1, "um bilhete para o suporte");
        let bilhete = std::fs::read_to_string(&arquivadas[0]).unwrap();
        assert!(bilhete.contains(&conta_antiga), "o bilhete diz a conta do segredo antigo: {bilhete}");
        let arquivada = PathBuf::from(arquivadas[0].to_string_lossy().trim_end_matches(".leia-me.txt").to_string());
        let reaberta = Identidade::abrir_de(&arquivada, segredo_antigo.trim()).unwrap();
        assert_eq!(
            reaberta.publica(),
            antiga.identidade.publica(),
            "a identidade antiga ainda abre com o segredo dela"
        );
        // A nova reabre (com a geração dela) pelo cofre que funciona.
        let de_novo = abrir_no_caminho(c.clone(), &Modo::Instalado, &funciona).unwrap();
        assert_eq!(de_novo.identidade.publica(), nova.identidade.publica());
    }

    /// F-06: com o cofre do sistema funcionando, o segredo vai para ele (não para a
    /// reserva), a identidade reabre lendo dele (com a quebra de linha que o
    /// `secret-tool` devolve) e nem o segredo nem a chave privada vão para o banco.
    #[cfg(target_os = "linux")]
    #[test]
    fn f06_cofre_do_sistema_guarda_o_segredo_fora_do_banco_e_da_reserva() {
        let raiz = PastaTemp::nova("cofre-funciona");
        let bin = secret_tool_falso(&raiz.0, "funciona");
        let cofre = CofreSistema::SecretTool(bin);
        let pasta_banco = raiz.0.join("dados");
        std::fs::create_dir_all(&pasta_banco).unwrap();
        let c = caminho_identidade_com_base(&pasta_banco, &Modo::Instalado, Some(&raiz.0.join("cfg"))).unwrap();
        let id = carregar_ou_criar_no_caminho(c.clone(), &Modo::Instalado, &cofre).unwrap();
        assert!(id.cofre_sistema, "o segredo tem de ficar no cofre do sistema");
        assert!(!caminho_reserva(&c).exists(), "com o cofre funcionando, não há reserva local");
        let guardado = std::fs::read_to_string(raiz.0.join("chaveiro").join(conta_cofre(&c))).unwrap();
        assert_eq!(guardado.len(), 64, "o segredo de 256 bits (hex) foi para o cofre");
        let reaberta = carregar_ou_criar_no_caminho(c.clone(), &Modo::Instalado, &cofre).unwrap();
        assert!(reaberta.cofre_sistema);
        assert_eq!(reaberta.identidade.publica(), id.identidade.publica());

        // O banco da loja guarda só a chave pública (nada do segredo nem da privada).
        let caminho_db = pasta_banco.join("rustinho.db");
        let b = Banco::abrir(&caminho_db).unwrap();
        let gerente = crate::faixas::para_testes::configurar(&b, "Caixa 1", 0);
        garantir_raiz_se_aplicavel(&b.conn, &id.identidade, id.cofre_sistema, LOJA, "Caixa 1", &gerente, true).unwrap();
        b.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").unwrap();
        drop(b);
        let (priv_ed, priv_x) = id.identidade.privadas_para_teste();
        let mut bytes = Vec::new();
        for e in std::fs::read_dir(&pasta_banco).unwrap().filter_map(|e| e.ok()) {
            if e.file_name().to_string_lossy().starts_with("rustinho.db") {
                bytes.extend(std::fs::read(e.path()).unwrap());
            }
        }
        let contem = |agulha: &[u8]| bytes.windows(agulha.len()).any(|w| w == agulha);
        assert!(contem(hex(&id.identidade.publica()).as_bytes()), "a chave pública fica no banco");
        assert!(!contem(guardado.as_bytes()), "o segredo de selagem foi parar no banco");
        assert!(!contem(&priv_ed) && !contem(&priv_x), "a chave privada foi parar no banco");
        assert!(!contem(hex(&priv_x).as_bytes()), "a chave privada (hex) foi parar no banco");
    }

    /// F-06: o cofre do sistema que falha (sai com erro) ou trava (passa do prazo de
    /// 3 s) não impede criar a identidade: vai para a reserva, com o aviso
    /// (`cofre_sistema = false`), e reabre depois do mesmo jeito.
    #[cfg(target_os = "linux")]
    #[test]
    fn f06_cofre_do_sistema_que_falha_ou_trava_cai_na_reserva_com_aviso() {
        for jeito in ["falha", "trava"] {
            let raiz = PastaTemp::nova(&format!("cofre-{jeito}"));
            let cofre = CofreSistema::SecretTool(secret_tool_falso(&raiz.0, jeito));
            let c =
                caminho_identidade_com_base(&raiz.0.join("pc"), &Modo::Instalado, Some(&raiz.0.join("cfg"))).unwrap();
            // O prazo do cofre (3 s) vale para guardar e para ler: o "trava" dorme 60 s.
            if let CofreSistema::SecretTool(bin) = &cofre {
                let inicio = Instant::now();
                assert!(!secret_tool_guardar(bin, "conta-prazo", "segredo"), "{jeito}");
                assert!(secret_tool_ler(bin, "conta-prazo").is_none(), "{jeito}");
                assert!(inicio.elapsed() < Duration::from_secs(20), "{jeito}: o prazo do cofre não foi respeitado");
            }
            let id = carregar_ou_criar_no_caminho(c.clone(), &Modo::Instalado, &cofre).unwrap();
            assert!(!id.cofre_sistema, "{jeito}: sem cofre, o aviso tem de aparecer");
            assert!(caminho_reserva(&c).exists(), "{jeito}: a reserva local guardou o segredo");
            let de_novo = carregar_ou_criar_no_caminho(c, &Modo::Instalado, &cofre).unwrap();
            assert_eq!(de_novo.identidade.publica(), id.identidade.publica(), "{jeito}");
            assert!(!de_novo.cofre_sistema, "{jeito}");
        }
    }

    /// F-06: a identidade cujo segredo está só no cofre do sistema, com o cofre fora
    /// do ar, não abre — e **não** é substituída por outra (falha fechada).
    #[cfg(target_os = "linux")]
    #[test]
    fn f06_identidade_no_cofre_fora_do_ar_falha_fechada_sem_criar_outra() {
        let raiz = PastaTemp::nova("cofre-fora-do-ar");
        let c = caminho_identidade_com_base(&raiz.0.join("pc"), &Modo::Instalado, Some(&raiz.0.join("cfg"))).unwrap();
        let funciona = CofreSistema::SecretTool(secret_tool_falso(&raiz.0, "funciona"));
        let id = carregar_ou_criar_no_caminho(c.clone(), &Modo::Instalado, &funciona).unwrap();
        let antes = std::fs::read(&c).unwrap();
        let fora = CofreSistema::SecretTool(secret_tool_falso(&raiz.0, "falha"));
        assert!(carregar_ou_criar_no_caminho(c.clone(), &Modo::Instalado, &fora).is_err());
        assert!(carregar_existente_no_caminho(c.clone(), &Modo::Instalado, &fora).is_err());
        assert_eq!(std::fs::read(&c).unwrap(), antes, "a identidade gravada não pode ser trocada");
        assert!(!caminho_reserva(&c).exists(), "nem ganhar uma reserva por cima");
        let volta = carregar_ou_criar_no_caminho(c, &Modo::Instalado, &funciona).unwrap();
        assert_eq!(volta.identidade.publica(), id.identidade.publica(), "com o cofre de volta, abre a mesma");
    }

    /// F-06 (Windows): a identidade instalada guarda o segredo no DPAPI (arquivo
    /// `.dpapi`, sem reserva) e reabre por ele.
    #[cfg(windows)]
    #[test]
    fn f06_dpapi_guarda_o_segredo_da_identidade_e_reabre() {
        let raiz = PastaTemp::nova("cofre-dpapi");
        let c = caminho_identidade_com_base(&raiz.0.join("pc"), &Modo::Instalado, Some(&raiz.0.join("cfg"))).unwrap();
        let id = carregar_ou_criar_no_caminho(c.clone(), &Modo::Instalado, &CofreSistema::Dpapi).unwrap();
        assert!(id.cofre_sistema);
        assert!(caminho_dpapi(&c).exists());
        assert!(!caminho_reserva(&c).exists());
        let de_novo = carregar_ou_criar_no_caminho(c, &Modo::Instalado, &CofreSistema::Dpapi).unwrap();
        assert_eq!(de_novo.identidade.publica(), id.identidade.publica());
    }

    /// N-01: no portátil, a reserva se amarra ao `segredo_local` do computador, que
    /// não fica no pendrive: com a impressão do computador (o que o banco do pendrive
    /// guarda em `computador_id`), a reserva não abre.
    #[test]
    fn n01_reserva_do_portatil_nao_abre_com_o_que_esta_no_pendrive() {
        let p = PastaTemp::nova("portatil-n01");
        let modo = Modo::portatil_deste_computador();
        let c = caminho_identidade_com_base(&p.0, &modo, None).unwrap();
        carregar_ou_criar_no_caminho(c.clone(), &modo, &CofreSistema::Nenhum).unwrap();
        let so_com_o_pendrive = Modo::portatil_com(crate::computador::atual().as_deref());
        assert!(
            ler_reserva_local(&caminho_reserva(&c), &c, &so_com_o_pendrive).is_err(),
            "quem tem só o pendrive (o computador_id do banco) não pode abrir a reserva"
        );
        assert!(ler_reserva_local(&caminho_reserva(&c), &c, &modo).is_ok(), "no próprio computador, abre");
    }

    /// N-02: a trava presa por outro processo (que não morreu) não congela quem chama:
    /// erro claro no prazo.
    #[cfg(target_os = "linux")]
    #[test]
    fn n02_trava_presa_por_outro_processo_da_erro_no_prazo() {
        if !flock_sem_fork() {
            eprintln!("sem /usr/bin/flock com --no-fork neste sistema: teste não se aplica");
            return;
        }
        let raiz = PastaTemp::nova("trava-presa");
        let c = caminho_identidade_com_base(&raiz.0.join("pc"), &Modo::Instalado, Some(&raiz.0.join("cfg"))).unwrap();
        std::fs::create_dir_all(c.parent().unwrap()).unwrap();
        let trava = caminho_da_trava(&c);
        let mut filho = std::process::Command::new("/usr/bin/flock")
            .args(["--no-fork", "-x"])
            .arg(&trava)
            .args(["sleep", "120"])
            .spawn()
            .unwrap();
        let fim = Instant::now() + Duration::from_secs(10);
        while std::process::Command::new("/usr/bin/flock")
            .args(["-n", "-x"])
            .arg(&trava)
            .arg("true")
            .status()
            .map(|s| s.success())
            .unwrap_or(true)
        {
            assert!(Instant::now() < fim, "o processo filho não pegou a trava");
            std::thread::sleep(Duration::from_millis(20));
        }
        let inicio = Instant::now();
        let r = carregar_ou_criar_no_caminho(c, &Modo::Instalado, &CofreSistema::Nenhum);
        let levou = inicio.elapsed();
        let _ = filho.kill();
        let _ = filho.wait();
        assert!(r.is_err(), "com a trava presa por outro processo, tem de dar erro");
        assert!(levou < Duration::from_secs(30), "esperou demais pela trava: {levou:?}");
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
