//! Registro técnico de falhas (`registro/rustinho.log`, na pasta dos dados).
//!
//! Só entra o que ajuda a achar um defeito: a hora, o comando e o erro
//! interno. Nunca entram os dados digitados (nomes, CPF/CNPJ, senhas,
//! valores). O arquivo é trocado ao passar de 512 KB (guarda os 3 últimos).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, Once};

const TAMANHO_MAXIMO: u64 = 512 * 1024;
const ANTIGOS: u32 = 3;

static PASTA: Mutex<Option<PathBuf>> = Mutex::new(None);
static GANCHO: Once = Once::new();

/// Começa a registrar na pasta `registro`, ao lado do banco.
pub fn iniciar(pasta_dados: &Path) {
    let pasta = pasta_dados.join("registro");
    if std::fs::create_dir_all(&pasta).is_err() {
        return;
    }
    *PASTA.lock().unwrap_or_else(|e| e.into_inner()) = Some(pasta);
    // Pânico (defeito no programa): anota onde foi, antes da mensagem
    // genérica que aparece na tela.
    GANCHO.call_once(|| {
        let anterior = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let onde = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
            let msg = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_default();
            anotar(&format!("PÂNICO em {onde}: {}", curto(&msg)));
            anterior(info);
        }));
    });
}

fn curto(t: &str) -> String {
    let t: String = t.chars().take(300).collect();
    t.replace(['\n', '\r'], " ")
}

/// Acrescenta uma linha (com a hora). Falhar aqui nunca atrapalha o caixa.
pub fn anotar(texto: &str) {
    let Some(pasta) = PASTA.lock().unwrap_or_else(|e| e.into_inner()).clone() else { return };
    let arquivo = pasta.join("rustinho.log");
    if std::fs::metadata(&arquivo).map(|m| m.len() > TAMANHO_MAXIMO).unwrap_or(false) {
        for n in (1..ANTIGOS).rev() {
            let _ =
                std::fs::rename(pasta.join(format!("rustinho.log.{n}")), pasta.join(format!("rustinho.log.{}", n + 1)));
        }
        let _ = std::fs::rename(&arquivo, pasta.join("rustinho.log.1"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&arquivo) {
        let _ = writeln!(f, "{} v{} {}", crate::tempo::agora(), crate::VERSAO, curto(texto));
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn grava_e_troca_o_arquivo() {
        let pasta = std::env::temp_dir().join(format!("rustinho-registro-{}", crate::tempo::novo_id()));
        iniciar(&pasta);
        anotar("comando x: erro interno de teste");
        let t = std::fs::read_to_string(pasta.join("registro/rustinho.log")).unwrap();
        assert!(t.contains("erro interno de teste"));
        std::fs::write(pasta.join("registro/rustinho.log"), vec![b'a'; (TAMANHO_MAXIMO + 1) as usize]).unwrap();
        anotar("depois");
        assert!(pasta.join("registro/rustinho.log.1").exists());
        assert!(std::fs::read_to_string(pasta.join("registro/rustinho.log")).unwrap().contains("depois"));
        let _ = std::fs::remove_dir_all(&pasta);
    }
}
