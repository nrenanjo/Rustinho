//! Estoque: o saldo de cada peça é a soma dos seus movimentos.
//!
//! Nunca se "troca o número" do estoque. Cada entrada ou saída vira um
//! movimento (estoque inicial, ajuste, venda, compra...). Assim dá para saber
//! sempre de onde veio cada unidade, e os caixas poderão juntar seus
//! movimentos sem perder nada.

use rusqlite::{params, Connection, OptionalExtension};

use crate::erro::{Erro, Resultado};
use crate::{auditoria, numeros, tempo, texto};

/// Saldo atual da peça, em milésimos.
pub fn saldo(conn: &Connection, produto_id: &str) -> Resultado<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(quantidade), 0) FROM estoque_movimento WHERE produto_id = ?1",
        [produto_id],
        |l| l.get(0),
    )?)
}

/// Lança um movimento de estoque (positivo entra, negativo sai). Serviço não
/// tem estoque: para ele, não lança nada.
pub fn lancar(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: Option<&str>,
    produto_id: &str,
    quantidade: i64,
    tipo: &str,
    motivo: &str,
) -> Resultado<()> {
    conn.execute(
        "INSERT INTO estoque_movimento (id, produto_id, quantidade, tipo, motivo, usuario_id, dispositivo, criado_em)
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8
          WHERE NOT EXISTS (SELECT 1 FROM produto WHERE id = ?2 AND tipo = 'servico')",
        params![tempo::novo_id(), produto_id, quantidade, tipo, motivo, usuario_id, dispositivo, tempo::agora()],
    )?;
    Ok(())
}

/// Acerta o estoque para a quantidade contada agora. Lança só a diferença.
/// Devolve o novo saldo.
pub fn ajustar(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    produto_id: &str,
    contado: &str,
    motivo: &str,
) -> Resultado<i64> {
    let contado = numeros::ler_quantidade(contado)
        .map_err(|m| Erro::campo("quantidade", m))?
        .ok_or_else(|| Erro::campo("quantidade", "Digite a quantidade contada."))?;
    if contado < 0 {
        return Err(Erro::campo("quantidade", "A quantidade não pode ser negativa."));
    }
    let motivo = texto::limpar(motivo);
    if motivo.is_empty() {
        return Err(Erro::campo("motivo", "Diga o motivo do ajuste."));
    }
    if texto::tamanho(&motivo) > 120 {
        return Err(Erro::campo("motivo", "Use no máximo 120 letras."));
    }
    let servico: bool = conn
        .query_row("SELECT tipo = 'servico' FROM produto WHERE id = ?1", [produto_id], |l| l.get(0))
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Peça".into()))?;
    if servico {
        return Err(Erro::validacao("Serviço não tem estoque."));
    }
    let atual = saldo(conn, produto_id)?;
    let diferenca = contado - atual;
    if diferenca == 0 {
        return Err(Erro::campo("quantidade", "O estoque já está com essa quantidade."));
    }
    lancar(conn, dispositivo, Some(usuario_id), produto_id, diferenca, "ajuste", &motivo)?;
    let sinal = if diferenca > 0 { "+" } else { "" };
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "estoque.ajustar",
        "produto",
        produto_id,
        &[
            format!(
                "Estoque: {} → {} ({sinal}{})",
                numeros::escrever_quantidade(atual),
                numeros::escrever_quantidade(contado),
                numeros::escrever_quantidade(diferenca)
            ),
            format!("Motivo: {motivo}"),
        ],
    )?;
    Ok(contado)
}
