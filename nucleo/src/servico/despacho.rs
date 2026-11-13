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
//!
//! ## Regra das transações (contrato C5; dev.3.3, passo 3.5)
//!
//! - Quem abre a transação é o comando: o atendente (no serviço ou no
//!   submódulo `comandos` de cada domínio) abre, com `IMMEDIATE` (o padrão das
//!   conexões, veja `banco`), e passa `&tx` para o domínio. O domínio não abre
//!   outra nem faz `commit`.
//! - A auditoria vai na mesma transação.
//! - O comando que mexe no dinheiro ou no estoque de um registro que já existe
//!   tem a marca `dinheiro` ([`COMANDOS_DE_DINHEIRO`]), e a função de domínio
//!   dele chama o bloqueio de registro comprovadamente inconsistente
//!   (`sync::exigir_editavel`, R-05).
//! - A leitura pesada da tela vai por conexão só de leitura (passo 7.6).
//!
//! Os módulos de domínio que ainda abrem a própria transação (backup, faixas,
//! loja e sync) estão congelados em `tests/dependencias.rs`: módulo novo fora
//! da regra faz o teste falhar. Cada um passa a receber a transação do comando
//! quando for mexido na fase dele.

use rusqlite::Connection;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use crate::erro::Resultado;

/// Marca `dinheiro` (contrato C5, R-05; nome único no núcleo desde a RS-6, que
/// o separou das colunas em centavos da troca de dados): os comandos que mexem no dinheiro ou
/// no estoque de um registro que já existe, com a função de domínio que grava
/// (`módulo::função`). O teste R-05 do serviço é gerado desta lista: o
/// atendente chama a função, e a função chama o bloqueio de registro
/// comprovadamente inconsistente, menos as lacunas conhecidas (E04), que
/// ficam listadas no teste para a 6.3. Comando novo assim entra aqui.
pub const COMANDOS_DE_DINHEIRO: &[(&str, &str)] = &[
    ("receber_baixar", "receber::receber"),
    ("recebimento_estornar", "receber::estornar"),
    ("conta_cancelar", "receber::cancelar_conta"),
    ("cheque_depositar", "cheques::depositar"),
    ("cheque_repassar", "cheques::repassar"),
    ("cheque_devolver", "cheques::devolver"),
    ("cheque_desfazer", "cheques::desfazer"),
    ("pagar_pagar", "pagar::pagar"),
    ("pagamento_estornar", "pagar::estornar"),
    ("pagar_alterar", "pagar::alterar"),
    ("pagar_cancelar", "pagar::cancelar"),
    ("compra_atualizar", "compras::atualizar"),
    ("compra_lancar", "compras::lancar"),
    ("compra_estornar", "compras::estornar"),
    ("devolucao_desfazer", "devolucoes::desfazer"),
    ("movimento_estornar", "financeiro::estornar_movimento"),
    // Lacunas conhecidas (E04, para a 6.3): ainda não chamam o bloqueio.
    ("venda_cancelar", "vendas::cancelar"),
    ("devolucao_gravar", "devolucoes::devolver"),
    ("caixa_movimentar", "caixa::movimentar_com_conta"),
    ("caixa_fechar", "caixa::fechar"),
    ("comissao_fechar", "comissoes::fechar"),
    ("comissao_reabrir", "comissoes::reabrir"),
];

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
    /// R-2: a chave da última linha que a lista contínua de Peças já tem.
    pub continuar: Option<String>,
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
