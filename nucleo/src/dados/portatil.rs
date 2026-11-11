//! Rustinho portátil (o ZIP para testar no Windows sem instalar).
//!
//! Um arquivo `rustinho-portatil.txt` ao lado do executável liga o modo
//! portátil. Nele o Rustinho fica todo dentro da própria pasta e não encosta
//! em nenhuma outra instalação do computador:
//! - os dados ficam em `dados/` ao lado do executável (nunca na pasta de
//!   sempre, `%LOCALAPPDATA%\Rustinho` ou `~/.local/share/rustinho`, nem na
//!   de `RUSTINHO_DADOS`);
//! - os backups só podem ir para uma pasta dentro da pasta portátil;
//! - o Google Drive usa uma configuração própria (`dados/rclone.conf`), não a
//!   do Rustinho instalado (que poderia mandar cópias de teste para a pasta da
//!   loja e apagar backups antigos de lá);
//! - a troca de dados usa outras portas (47820/47821) e uma chave da rede
//!   derivada de outro jeito: um portátil só conversa com outro portátil,
//!   nunca com um caixa instalado, mesmo com a mesma senha da rede.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// O arquivo que marca a pasta do Rustinho portátil (ao lado do executável).
pub const MARCA: &str = "rustinho-portatil.txt";

/// Pasta dos dados dentro da pasta portátil.
pub const PASTA_DADOS: &str = "dados";

static RAIZ: OnceLock<Option<PathBuf>> = OnceLock::new();

/// A pasta do Rustinho portátil (a do executável), se este é o portátil.
pub fn raiz() -> Option<&'static Path> {
    RAIZ.get_or_init(|| {
        let exe = std::env::current_exe().ok()?;
        let pasta = exe.parent()?.to_path_buf();
        pasta.join(MARCA).is_file().then_some(pasta)
    })
    .as_deref()
}

/// Este Rustinho é o portátil?
pub fn ligado() -> bool {
    raiz().is_some()
}

/// Onde fica o banco no portátil.
pub fn caminho_do_banco(raiz: &Path) -> PathBuf {
    raiz.join(PASTA_DADOS).join("rustinho.db")
}

/// A configuração do rclone (Google Drive) do portátil.
pub fn rclone_conf() -> Option<PathBuf> {
    raiz().map(|r| r.join(PASTA_DADOS).join("rclone.conf"))
}

/// `caminho` fica dentro da pasta portátil `raiz`? (Compara as pastas como
/// estão no disco, sem `..` nem atalhos; uma pasta que ainda não existe vale
/// pelo caminho escrito, depois de tirar `..`.)
pub fn dentro_de(raiz: &Path, caminho: &Path) -> bool {
    /// Tira `.` e `..` pelo caminho escrito; depois, a parte que já existe
    /// no disco é trocada pela forma do sistema (atalhos resolvidos; no
    /// Windows, `\\?\C:\...`), igual para a pasta portátil e para o caminho.
    fn limpo(p: &Path) -> PathBuf {
        let mut escrito = PathBuf::new();
        for parte in p.components() {
            match parte {
                std::path::Component::ParentDir => {
                    escrito.pop();
                }
                std::path::Component::CurDir => {}
                outra => escrito.push(outra.as_os_str()),
            }
        }
        let mut resto = Vec::new();
        let mut base = escrito.as_path();
        loop {
            if let Ok(c) = std::fs::canonicalize(base) {
                let mut saida = c;
                for r in resto.iter().rev() {
                    saida.push(r);
                }
                return saida;
            }
            match (base.file_name(), base.parent()) {
                (Some(nome), Some(pai)) => {
                    resto.push(nome.to_os_string());
                    base = pai;
                }
                _ => return escrito,
            }
        }
    }
    limpo(caminho).starts_with(limpo(raiz))
}

/// A senha da rede como entra na chave: no portátil, misturada com um texto
/// próprio, para nunca dar a mesma chave de um caixa instalado.
pub fn senha_da_rede(senha: &str) -> std::borrow::Cow<'_, str> {
    if ligado() {
        std::borrow::Cow::Owned(format!("rustinho-portatil-teste\u{0}{senha}"))
    } else {
        std::borrow::Cow::Borrowed(senha)
    }
}

/// Portas da troca de dados (TCP, UDP): as de sempre, ou as do portátil.
pub fn portas() -> (u16, u16) {
    if ligado() {
        (PORTA_TCP, PORTA_UDP)
    } else {
        (crate::sync::PORTA_TCP, crate::sync::PORTA_UDP)
    }
}

/// Portas do portátil.
pub const PORTA_TCP: u16 = 47820;
pub const PORTA_UDP: u16 = 47821;

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn dentro_da_pasta_portatil() {
        let base = std::env::temp_dir().join(format!("rustinho-portatil-{}", crate::tempo::novo_id()));
        std::fs::create_dir_all(base.join("dados/backups")).unwrap();
        assert!(dentro_de(&base, &base.join("dados/backups")));
        assert!(dentro_de(&base, &base.join("dados/ainda-nao-existe")));
        assert!(!dentro_de(&base, &base.join("../outra")));
        assert!(!dentro_de(&base, &base.join("dados/../../fora")));
        assert!(!dentro_de(&base, &std::env::temp_dir()));
        // Pasta com nome que começa igual não é a mesma pasta.
        let vizinha = PathBuf::from(format!("{}-vizinha", base.display()));
        assert!(!dentro_de(&base, &vizinha));
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn fora_do_portatil_nada_muda() {
        // O executável dos testes não tem a marca ao lado.
        assert!(!ligado());
        assert_eq!(portas(), (crate::sync::PORTA_TCP, crate::sync::PORTA_UDP));
        assert_eq!(senha_da_rede("abcd"), "abcd");
        assert!(rclone_conf().is_none());
    }
}
