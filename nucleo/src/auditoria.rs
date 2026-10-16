//! Histórico de quem fez o quê (auditoria).

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::erro::Resultado;
use crate::tempo;

/// Registra uma ação. `detalhes` são frases curtas, como
/// "Preço Revenda: 85,00 → 89,90".
pub fn registrar(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: Option<&str>,
    acao: &str,
    entidade: &str,
    entidade_id: &str,
    detalhes: &[String],
) -> Resultado<()> {
    conn.execute(
        "INSERT INTO auditoria (id, quando, usuario_id, dispositivo, acao, entidade, entidade_id, detalhes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            tempo::novo_id(),
            tempo::agora(),
            usuario_id,
            dispositivo,
            acao,
            entidade,
            entidade_id,
            serde_json::to_string(detalhes).unwrap_or_default(),
        ],
    )?;
    Ok(())
}

/// Um item do histórico, pronto para mostrar.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Registro {
    pub quando: String,
    pub usuario: String,
    pub acao: String,
    /// Frase curta: "cadastrou a peça", "alterou"...
    pub descricao: String,
    pub detalhes: Vec<String>,
}

/// Últimos registros de uma entidade (uma peça, um usuário...).
pub fn historico(conn: &Connection, entidade: &str, entidade_id: &str, limite: i64) -> Resultado<Vec<Registro>> {
    let mut consulta = conn.prepare_cached(
        "SELECT a.quando, COALESCE(u.nome, ''), a.acao, a.detalhes
           FROM auditoria a LEFT JOIN usuario u ON u.id = a.usuario_id
          WHERE a.entidade = ?1 AND a.entidade_id = ?2
          ORDER BY a.quando DESC, a.id DESC
          LIMIT ?3",
    )?;
    let linhas = consulta.query_map(params![entidade, entidade_id, limite], |l| {
        let acao: String = l.get(2)?;
        let detalhes: String = l.get(3)?;
        Ok(Registro {
            quando: l.get(0)?,
            usuario: l.get(1)?,
            descricao: descrever(&acao).to_string(),
            acao,
            detalhes: serde_json::from_str(&detalhes).unwrap_or_default(),
        })
    })?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

fn descrever(acao: &str) -> &'static str {
    match acao {
        "produto.criar" => "cadastrou a peça",
        "produto.alterar" => "alterou",
        "estoque.ajustar" => "ajustou o estoque",
        "usuario.criar" => "criou o usuário",
        "pessoa.criar" => "cadastrou",
        "pessoa.alterar" => "alterou",
        "compra.importar" => "importou o XML",
        "compra.entrada" => "deu entrada pela compra",
        "compra.lancar" => "lançou no estoque",
        "compra.estornar" => "estornou",
        "fiscal.regras" => "mudou as regras de ST",
        "usuario.alterar" => "alterou o usuário",
        "receber.lancar" => "lançou dívida",
        "receber.receber" => "recebeu",
        "receber.estornar" => "estornou um pagamento",
        "receber.cancelar" => "tirou uma dívida",
        "venda.cancelar" => "cancelou a venda",
        "pagar.lancar" => "lançou conta a pagar",
        "pagar.alterar" => "corrigiu a conta",
        "pagar.cancelar" => "tirou a conta",
        "pagar.pagar" => "pagou",
        "pagar.estornar" => "estornou um pagamento",
        "financeiro.conta" => "salvou a conta da loja",
        "financeiro.movimento" => "lançou movimento",
        "financeiro.transferir" => "transferiu",
        "financeiro.conferir" => "conferiu o saldo",
        "financeiro.estornar" => "estornou movimento",
        "cheque.depositar" => "depositou o cheque",
        "cheque.repassar" => "repassou o cheque",
        "cheque.devolver" => "registrou cheque devolvido",
        "cheque.desfazer" => "voltou o cheque para a carteira",
        "comissao.fechar" => "fechou a comissão",
        "encomenda.criar" => "anotou a encomenda",
        "encomenda.pedir" => "pediu a encomenda",
        "encomenda.chegou" => "marcou que a encomenda chegou",
        "encomenda.entregar" => "entregou a encomenda",
        "encomenda.cancelar" => "cancelou a encomenda",
        "estoque.dias" => "mudou os dias da sugestão de compra",
        "entrar" => "entrou",
        "sair" => "saiu",
        _ => "registrou",
    }
}
