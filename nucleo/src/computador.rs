//! Identidade deste computador, que não depende dos arquivos do Rustinho.
//!
//! Copiar a pasta dos dados (o banco e o arquivo `rustinho.db.codigos`) para
//! outro computador leva junto tudo o que é do Rustinho, inclusive o
//! identificador do caixa. O que não vai junto é a identidade do próprio
//! computador (o identificador que o sistema operacional dá a ele). O banco
//! guarda a identidade do computador onde nasceu; se ela mudar, o Rustinho
//! sabe que o banco foi copiado (veja `faixas::conferir_computador`).
//!
//! De onde vem a identidade:
//! - O programa pergunta ao Qt (`QSysInfo::machineUniqueId`) e entrega aqui
//!   com `definir`, antes de abrir o banco (Windows, Android e Linux).
//! - Sem isso (testes, ferramentas), no Linux: `/etc/machine-id` (ou o de
//!   `/var/lib/dbus`); nos outros sistemas, sem identidade.
//!
//! O que se guarda no banco é só uma impressão digital (SHA-256), nunca o
//! identificador do sistema.

use std::sync::OnceLock;

use sha2::{Digest, Sha256};

static DO_PROGRAMA: OnceLock<String> = OnceLock::new();

/// O programa entrega o identificador do computador que o Qt informa. Vale a
/// primeira chamada; vazio é ignorado.
pub fn definir(identificador: &str) {
    let i = identificador.trim();
    if !i.is_empty() {
        let _ = DO_PROGRAMA.set(i.to_string());
    }
}

/// Nos testes o "computador" é sempre o mesmo (não depende de onde os testes
/// rodam); os testes de outro computador usam `Banco::abrir_em`.
#[cfg(test)]
fn do_sistema() -> Option<String> {
    Some("computador-de-teste".into())
}

#[cfg(all(target_os = "linux", not(test)))]
fn do_sistema() -> Option<String> {
    ["/etc/machine-id", "/var/lib/dbus/machine-id"]
        .iter()
        .filter_map(|c| std::fs::read_to_string(c).ok())
        .map(|t| t.trim().to_string())
        .find(|t| !t.is_empty())
}

#[cfg(all(not(target_os = "linux"), not(test)))]
fn do_sistema() -> Option<String> {
    None
}

/// A impressão digital de um identificador de computador.
pub fn impressao(identificador: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"rustinho-computador|");
    h.update(identificador.trim().to_lowercase().as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// A identidade deste computador. None = o sistema não informa (nesse caso
/// o Rustinho não consegue confirmar que o banco está no computador de
/// sempre).
pub fn atual() -> Option<String> {
    DO_PROGRAMA.get().cloned().or_else(do_sistema).map(|i| impressao(&i))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn impressao_nao_mostra_o_identificador_e_ignora_caixa_e_espacos() {
        let a = impressao("ABC123");
        assert_eq!(a.len(), 64);
        assert!(!a.contains("abc123"));
        assert_eq!(a, impressao("  abc123\n"));
        assert_ne!(a, impressao("abc124"));
    }

    #[test]
    fn nos_testes_a_identidade_e_fixa() {
        assert_eq!(atual(), Some(impressao("computador-de-teste")));
    }
}
