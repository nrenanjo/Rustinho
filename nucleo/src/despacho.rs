//! Despacho dos comandos das telas (etapa 3 da modularização).
//!
//! A tela manda `Sistema.chamar("comando", json)` e o serviço (`api.rs`)
//! responde `{ok, dados, erro, campo}`. O serviço cuida da sessão, das
//! permissões, da senha do gerente, da chave da operação (`NAO_REPETIR`), do
//! modo seguro e do pânico; a classe de permissão de cada comando fica numa
//! tabela, ao lado de quem o atende.
//!
//! Este módulo fica embaixo dos domínios: não conhece nenhum deles nem o
//! serviço.

/// Classe de permissão de um comando: quem pode usar e se ele só lê.
///
/// É a mesma regra das listas de antes da etapa 3 (`SO_LEITURA`, `DO_CAIXA`,
/// `SEM_SESSAO` e as ações do gerente de cada comando), agora declarada junto
/// de cada comando. A lista congelada em `tests/contratos_comandos.rs`
/// confere comando a comando.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Classe {
    /// Só lê (não grava nada no banco); qualquer usuário conectado. Depois
    /// de um erro interno grave, só os comandos que leem continuam (veja
    /// `Servico::chamar`).
    Leitura,
    /// Grava; qualquer usuário conectado pode usar.
    Caixa,
    /// Só lê, e é de uma ação do gerente (as da pílula Gerente,
    /// "simultaneidade" ou "gerente"): o gerente, quem a loja liberou, ou a
    /// consulta da tela com a senha do gerente.
    ConsultaDoGerente(&'static str),
    /// Grava, e é de uma ação do gerente: o gerente, quem a loja liberou, ou
    /// a senha do gerente digitada para esta gravação.
    Gerente(&'static str),
    /// Só lê; funciona sem ninguém conectado.
    LeituraSemSessao,
    /// Grava; funciona sem ninguém conectado (entrar, sair, configurar...).
    SemSessao,
}

impl Classe {
    /// A ação do gerente que o comando exige, se exige.
    pub fn acao(self) -> Option<&'static str> {
        match self {
            Classe::ConsultaDoGerente(a) | Classe::Gerente(a) => Some(a),
            _ => None,
        }
    }

    /// Só lê.
    pub fn leitura(self) -> bool {
        matches!(self, Classe::Leitura | Classe::ConsultaDoGerente(_) | Classe::LeituraSemSessao)
    }

    /// Funciona sem ninguém conectado.
    pub fn sem_sessao(self) -> bool {
        matches!(self, Classe::LeituraSemSessao | Classe::SemSessao)
    }

    /// O nome da classe nos testes de permissão: a ação do gerente,
    /// "leitura" ou "caixa".
    pub fn nome(self) -> &'static str {
        self.acao().unwrap_or(if self.leitura() { "leitura" } else { "caixa" })
    }
}
