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

/// Um item da auditoria geral (Configurações › Simultaneidade › Histórico).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistroGeral {
    /// "27/10/2026 08:18" no horário daqui.
    pub quando: String,
    pub usuario: String,
    pub dispositivo: String,
    pub acao: String,
    pub descricao: String,
    pub detalhes: Vec<String>,
}

/// Os últimos registros da loja toda (de todos os caixas que já trocaram
/// dados com este), do mais novo para o mais velho.
pub fn recentes(conn: &Connection, limite: i64) -> Resultado<Vec<RegistroGeral>> {
    let mut consulta = conn.prepare_cached(
        "SELECT COALESCE(strftime('%d/%m/%Y %H:%M', a.quando, 'localtime'), a.quando),
                COALESCE(u.nome, ''), a.dispositivo, a.acao, a.detalhes
           FROM auditoria a LEFT JOIN usuario u ON u.id = a.usuario_id
          ORDER BY a.quando DESC, a.id DESC
          LIMIT ?1",
    )?;
    let linhas = consulta.query_map(params![limite], |l| {
        let acao: String = l.get(3)?;
        let detalhes: String = l.get(4)?;
        Ok(RegistroGeral {
            quando: l.get(0)?,
            usuario: l.get(1)?,
            dispositivo: l.get(2)?,
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
        "usuario.senha_fora_da_politica" => "entrou com senha fora da política",
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
        "backup.restaurar" => "restaurou um backup",
        "caixa.abrir" => "abriu o caixa",
        "caixa.fechar" => "fechou o caixa",
        "caixa.caixa_fechado" => "lançou no Caixa geral (caixa fechado)",
        "caixa.retirar" => "retirou um caixa da loja",
        "caixa.readmitir" => "readmitiu um caixa na loja",
        "codigos.assumir" => "passou a distribuir as faixas de códigos",
        "codigos.caixa_novo" => "registrou este banco como caixa novo",
        "codigos.computador" => "confirmou o computador deste caixa",
        "codigos.definir" => "deu código aos cadastros a definir",
        "compra.autorizacao" => "conferiu a nota no portal da NF-e",
        "conflito.resolver" => "resolveu uma diferença entre caixas",
        "conflito.manter_todos" => "deixou as diferenças entre caixas como estão",
        "dre.classificacao" => "classificou as categorias do resultado",
        "dre.meta" => "mudou uma meta do resultado",
        "gerente.liberar" => "digitou a senha do gerente",
        "gerente.salvar" => "mudou quem é gerente ou o que só o gerente pode",
        "os.criar" => "abriu a OS",
        "os.situacao" => "mudou a etapa da OS",
        "os.entregar" => "entregou a OS",
        "os.cancelar" => "cancelou a OS",
        "os.rascunho" => "salvou um rascunho de OS",
        "os.rascunho_excluir" => "excluiu um rascunho de OS",
        "pendencia.arquivar" => "arquivou uma pendência da troca de dados",
        "pessoa.codigo" => "deu código novo ao cadastro",
        "precos.regras" => "mudou a regra de preço",
        "produto.codigo" => "deu código novo à peça",
        "produto.defeito" => "registrou peça com defeito",
        "produto.importar" => "importou cadastros",
        "referencia.aceitar" => "aceitou um registro incompleto como está",
        "venda.devolucao" => "registrou troca ou devolução",
        "venda.devolucao_desfeita" => "desfez a troca ou devolução",
        "entrar" => "entrou",
        "sair" => "saiu",
        _ => "registrou",
    }
}
