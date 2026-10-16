//! Erros do Rustinho, com mensagens prontas para mostrar ao usuário.

use std::fmt;

/// Tudo o que pode dar errado no núcleo.
#[derive(Debug)]
pub enum Erro {
    /// Problema no que foi digitado. `campo` diz qual campo destacar na tela.
    Validacao {
        campo: Option<String>,
        mensagem: String,
    },
    /// O comando precisa de alguém conectado (tela de entrada).
    SemSessao,
    /// Registro procurado não existe.
    NaoEncontrado(String),
    /// Falha do SQLite.
    Banco(rusqlite::Error),
    /// Erro de programação ou de dados inesperados.
    Interno(String),
}

/// Atalho para `Result<T, Erro>`.
pub type Resultado<T> = Result<T, Erro>;

impl Erro {
    /// Erro de validação ligado a um campo da tela.
    pub fn campo(campo: &str, mensagem: impl Into<String>) -> Self {
        Erro::Validacao {
            campo: Some(campo.to_string()),
            mensagem: mensagem.into(),
        }
    }

    /// Erro de validação sem campo específico.
    pub fn validacao(mensagem: impl Into<String>) -> Self {
        Erro::Validacao {
            campo: None,
            mensagem: mensagem.into(),
        }
    }

    /// Nome do campo com problema, se houver.
    pub fn nome_do_campo(&self) -> Option<&str> {
        match self {
            Erro::Validacao { campo, .. } => campo.as_deref(),
            _ => None,
        }
    }
}

impl fmt::Display for Erro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Erro::Validacao { mensagem, .. } => write!(f, "{mensagem}"),
            Erro::SemSessao => write!(f, "Entre com seu usuário para continuar."),
            Erro::NaoEncontrado(o_que) => write!(f, "{o_que} não encontrado."),
            Erro::Banco(e) => write!(f, "Erro no banco de dados: {e}"),
            Erro::Interno(msg) => write!(f, "Erro interno: {msg}"),
        }
    }
}

impl std::error::Error for Erro {}

impl From<rusqlite::Error> for Erro {
    fn from(e: rusqlite::Error) -> Self {
        Erro::Banco(e)
    }
}

impl From<serde_json::Error> for Erro {
    fn from(e: serde_json::Error) -> Self {
        Erro::Interno(format!("dados em formato inesperado ({e})"))
    }
}
