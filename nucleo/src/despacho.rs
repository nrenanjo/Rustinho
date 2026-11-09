//! Despacho dos comandos das telas (etapa 3 da modularização).
//!
//! A tela manda `Sistema.chamar("comando", json)` e o serviço (`api.rs`)
//! responde `{ok, dados, erro, campo}`. O serviço cuida da sessão, das
//! permissões, da senha do gerente, da chave da operação (`NAO_REPETIR`), do
//! modo seguro e do pânico; a classe de permissão de cada comando fica numa
//! tabela, ao lado de quem o atende.
//!
//! Cada domínio declara no próprio módulo a sua tabela de comandos
//! (`COMANDOS`: [`Comando`] com nome, classe de permissão e função); o
//! serviço procura o comando nas tabelas e chama a função com um [`Pedido`].
//! O nome, o JSON de entrada e o de saída de cada comando são os de antes da
//! etapa 3 (as funções vieram dos braços do `match` do `api.rs`).
//!
//! Este módulo fica embaixo dos domínios: não conhece nenhum deles nem o
//! serviço. As perguntas de permissão do meio de um comando chegam pelo
//! [`Acesso`], que o serviço implementa: se um domínio dependesse do
//! serviço, cada domínio fecharia um ciclo com ele (o teste
//! `tests/dependencias.rs` não deixa).

use rusqlite::Connection;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use crate::erro::Resultado;

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

/// Um comando das telas, na tabela do domínio que o atende.
pub struct Comando {
    /// O nome que a tela manda (`Sistema.chamar("nome", json)`).
    pub nome: &'static str,
    pub classe: Classe,
    /// Atende o comando: recebe os dados da tela e devolve o `dados` da
    /// resposta (o serviço monta o envelope `{ok, dados, erro, campo}`).
    pub funcao: fn(&mut Pedido<'_>, Value) -> Resultado<Value>,
}

/// As perguntas de permissão que um comando faz no meio do caminho (ex.: a
/// peça só pede a senha do gerente quando o preço muda). Quem responde é o
/// serviço, com a regra de sempre (senhas do gerente digitadas, ações que a
/// loja liberou, consulta da tela).
pub trait Acesso {
    /// Pode fazer esta ação do gerente sem senha? (é gerente, ou a loja
    /// liberou a ação para todos na pílula Gerente.)
    fn pode_sem_senha(&mut self, conn: &Connection, acao: &str) -> Resultado<bool>;
    /// Pode ler o que é desta ação? Sem senha, ou com a senha do gerente
    /// digitada para consultar a tela dela (renova a cada leitura).
    fn pode_ler(&mut self, conn: &Connection, acao: &str) -> Resultado<bool>;
    /// Esta chamada pode fazer esta ação do gerente? Se não, o erro que pede
    /// a senha do gerente.
    fn exigir(&mut self, conn: &Connection, acao: &str) -> Resultado<()>;
}

/// Uma chamada de um comando: o banco, este caixa, quem está conectado e as
/// perguntas de permissão. A transação, quando há, é aberta pelo próprio
/// comando, como sempre foi.
pub struct Pedido<'a> {
    pub conn: &'a mut Connection,
    /// Este caixa (`dispositivo_id`).
    pub disp: &'a str,
    /// Quem está conectado.
    pub usuario_id: &'a str,
    pub gerente: bool,
    /// A3: o banco abriu em modo seguro (com este defeito).
    pub modo_seguro: bool,
    pub defeito: Option<&'a str>,
    acesso: &'a mut dyn Acesso,
}

impl<'a> Pedido<'a> {
    pub fn novo(
        conn: &'a mut Connection,
        disp: &'a str,
        usuario_id: &'a str,
        gerente: bool,
        modo_seguro: bool,
        defeito: Option<&'a str>,
        acesso: &'a mut dyn Acesso,
    ) -> Pedido<'a> {
        Pedido { conn, disp, usuario_id, gerente, modo_seguro, defeito, acesso }
    }

    /// Veja [`Acesso::pode_sem_senha`].
    pub fn pode_sem_senha(&mut self, acao: &str) -> Resultado<bool> {
        self.acesso.pode_sem_senha(self.conn, acao)
    }

    /// Veja [`Acesso::pode_ler`].
    pub fn pode_ler(&mut self, acao: &str) -> Resultado<bool> {
        self.acesso.pode_ler(self.conn, acao)
    }

    /// Veja [`Acesso::exigir`].
    pub fn exigir(&mut self, acao: &str) -> Resultado<()> {
        self.acesso.exigir(self.conn, acao)
    }

    /// Pode ver custo e % de lucro das peças (D-09): quem vê os resultados
    /// (a mesma regra de `Servico::ve_custos`). Na consulta de uma tela de
    /// resultados com a senha do gerente, o cadastro não abre (é outra tela).
    pub fn ve_custos(&mut self) -> Resultado<bool> {
        self.pode_sem_senha("resultados")
    }
}

/// Converte os dados JSON no tipo esperado pelo comando.
pub fn ler<T: DeserializeOwned>(dados: Value) -> Resultado<T> {
    Ok(serde_json::from_value(dados)?)
}

// Formatos de entrada que telas de vários domínios usam (os de um domínio só
// ficam com ele).

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Id {
    pub id: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Estorno {
    pub id: String,
    pub motivo: String,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Filtro {
    pub texto: String,
    pub quais: String,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct BuscaProdutos {
    pub texto: String,
    pub inativas: bool,
    pub limite: Option<i64>,
    #[serde(alias = "offset")]
    pub deslocamento: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Periodo {
    pub de: String,
    pub ate: String,
    pub dias: Option<i64>,
    pub usuario_id: String,
    pub vencimento: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Arquivo {
    pub caminho: String,
}
