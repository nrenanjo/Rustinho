//! Caixa: abertura com o troco inicial, sangrias e suprimentos, fechamento
//! com a conferência do dinheiro da gaveta.
//!
//! Cada computador tem o seu caixa. Vender só com o caixa aberto.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::erro::{Erro, Resultado};
use crate::{auditoria, numeros, tempo, texto};

/// Formas de pagamento: (código, nome na tela, disponível nesta versão).
pub const FORMAS: &[(&str, &str, bool)] = &[
    ("dinheiro", "Dinheiro", true),
    ("pix", "Pix", true),
    ("debito", "Cartão de débito", true),
    ("credito", "Cartão de crédito", true),
    ("cheque", "Cheque", true),
    ("crediario", "Crediário", true),
    ("vale", "Vale-troca", true),
    ("outro", "Outro", true),
];

pub fn nome_forma(forma: &str) -> &'static str {
    if forma == "devolucao" {
        return "Devolução";
    }
    FORMAS.iter().find(|f| f.0 == forma).map(|f| f.1).unwrap_or("Outro")
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sessao {
    pub id: String,
    pub caixa_nome: String,
    pub aberto_em: String,
    pub aberto_por: String,
    pub valor_inicial: i64,
    pub fechado_em: Option<String>,
}

/// O caixa aberto deste computador, se houver.
pub fn sessao_aberta(conn: &Connection, dispositivo: &str) -> Resultado<Option<Sessao>> {
    Ok(conn
        .query_row(
            "SELECT s.id, s.caixa_nome, s.aberto_em, COALESCE(u.nome, ''), s.valor_inicial, s.fechado_em
               FROM caixa_sessao s LEFT JOIN usuario u ON u.id = s.aberto_por
              WHERE s.dispositivo = ?1 AND s.fechado_em IS NULL
              ORDER BY s.aberto_em DESC LIMIT 1",
            [dispositivo],
            ler_sessao,
        )
        .optional()?)
}

fn ler_sessao(l: &rusqlite::Row) -> rusqlite::Result<Sessao> {
    Ok(Sessao {
        id: l.get(0)?,
        caixa_nome: l.get(1)?,
        aberto_em: l.get(2)?,
        aberto_por: l.get(3)?,
        valor_inicial: l.get(4)?,
        fechado_em: l.get(5)?,
    })
}

/// Abre o caixa com o dinheiro que já está na gaveta (troco).
pub fn abrir(conn: &Connection, dispositivo: &str, usuario_id: &str, caixa_nome: &str, valor_inicial: &str) -> Resultado<String> {
    if sessao_aberta(conn, dispositivo)?.is_some() {
        return Err(Erro::validacao("O caixa deste computador já está aberto."));
    }
    let valor = numeros::ler_dinheiro(valor_inicial).map_err(|m| Erro::campo("valorInicial", m))?.unwrap_or(0);
    if valor < 0 {
        return Err(Erro::campo("valorInicial", "O valor não pode ser negativo."));
    }
    let id = tempo::novo_id();
    conn.execute(
        "INSERT INTO caixa_sessao (id, dispositivo, caixa_nome, aberto_em, aberto_por, valor_inicial)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, dispositivo, caixa_nome, tempo::agora(), usuario_id, valor],
    )?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "caixa.abrir",
        "caixa",
        &id,
        &[format!("Troco inicial: {}", numeros::escrever_dinheiro(valor))],
    )?;
    Ok(id)
}

/// Sangria (tira dinheiro da gaveta) ou suprimento (põe).
pub fn movimentar(conn: &Connection, dispositivo: &str, usuario_id: &str, tipo: &str, valor: &str, motivo: &str) -> Resultado<()> {
    movimentar_com_conta(conn, dispositivo, usuario_id, tipo, valor, motivo, None).map(|_| ())
}

/// Sangria ou suprimento ligado a uma conta da loja: a sangria entra na
/// conta (ex.: dinheiro levado para o Caixa geral) e o suprimento sai dela.
/// Devolve o id do movimento.
pub fn movimentar_com_conta(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    tipo: &str,
    valor: &str,
    motivo: &str,
    conta_loja: Option<&str>,
) -> Resultado<String> {
    let sessao = sessao_aberta(conn, dispositivo)?.ok_or_else(|| Erro::validacao("O caixa está fechado."))?;
    if tipo != "sangria" && tipo != "suprimento" {
        return Err(Erro::Interno(format!("movimento desconhecido: {tipo}")));
    }
    let valor = numeros::ler_dinheiro(valor)
        .map_err(|m| Erro::campo("valor", m))?
        .filter(|v| *v > 0)
        .ok_or_else(|| Erro::campo("valor", "Digite o valor."))?;
    let motivo = texto::limpar(motivo);
    if motivo.is_empty() {
        return Err(Erro::campo("motivo", "Diga o motivo (ex.: depósito no banco, troco do cofre)."));
    }
    if tipo == "sangria" {
        let r = resumo(conn, &sessao.id)?;
        if valor > r.esperado_dinheiro {
            return Err(Erro::campo(
                "valor",
                format!("Pelo sistema, só há {} em dinheiro na gaveta.", numeros::escrever_dinheiro(r.esperado_dinheiro)),
            ));
        }
    }
    let conta_loja = conta_loja.filter(|c| !c.is_empty());
    if let Some(c) = conta_loja {
        let ativa: Option<bool> = conn
            .query_row("SELECT ativo FROM conta_loja WHERE id = ?1", [c], |l| l.get(0))
            .optional()?;
        if ativa != Some(true) {
            return Err(Erro::campo("conta", "Conta da loja não encontrada."));
        }
    }
    let id = tempo::novo_id();
    conn.execute(
        "INSERT INTO caixa_movimento (id, sessao_id, tipo, valor, motivo, usuario_id, criado_em, conta_loja_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, sessao.id, tipo, valor, motivo, usuario_id, tempo::agora(), conta_loja],
    )?;
    if let Some(c) = conta_loja {
        let (sinal, de_onde) = if tipo == "sangria" { (1, "Sangria do") } else { (-1, "Suprimento para o") };
        conn.execute(
            "INSERT INTO movimento_conta (id, conta_id, data, valor, tipo, descricao, origem_id, usuario_id, criado_em, dispositivo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                tempo::novo_id(),
                c,
                crate::fiscal::hoje(conn)?,
                sinal * valor,
                tipo,
                format!("{de_onde} {} · {motivo}", sessao.caixa_nome),
                id,
                usuario_id,
                tempo::agora(),
                dispositivo
            ],
        )?;
    }
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        &format!("caixa.{tipo}"),
        "caixa",
        &sessao.id,
        &[format!("{}: {} · {motivo}", if tipo == "sangria" { "Sangria" } else { "Suprimento" }, numeros::escrever_dinheiro(valor))],
    )?;
    Ok(id)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PorForma {
    pub forma: String,
    pub nome: String,
    pub valor: String,
    pub valor_centavos: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Movimento {
    pub tipo: String,
    pub valor: String,
    pub motivo: String,
    pub usuario: String,
    pub quando: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resumo {
    pub sessao: Sessao,
    pub valor_inicial: String,
    pub vendas: i64,
    pub canceladas: i64,
    pub total_vendido: String,
    pub por_forma: Vec<PorForma>,
    /// Pagamentos de crediário recebidos neste caixa.
    pub recebimentos: i64,
    pub total_recebido: String,
    pub recebido_por_forma: Vec<PorForma>,
    pub troco: String,
    pub sangrias: String,
    pub suprimentos: String,
    /// Dinheiro devolvido a clientes (trocas e devoluções).
    pub devolvido: String,
    pub movimentos: Vec<Movimento>,
    /// Dinheiro que deve haver na gaveta.
    pub esperado: String,
    #[serde(skip)]
    pub esperado_dinheiro: i64,
    pub contado: String,
    pub diferenca: String,
    pub diferenca_centavos: i64,
    pub fechado_por: String,
}

/// Resumo de uma sessão de caixa.
pub fn resumo(conn: &Connection, sessao_id: &str) -> Resultado<Resumo> {
    let sessao = conn
        .query_row(
            "SELECT s.id, s.caixa_nome, s.aberto_em, COALESCE(u.nome, ''), s.valor_inicial, s.fechado_em
               FROM caixa_sessao s LEFT JOIN usuario u ON u.id = s.aberto_por WHERE s.id = ?1",
            [sessao_id],
            ler_sessao,
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Caixa".into()))?;
    let (contado, fechado_por): (Option<i64>, String) = conn.query_row(
        "SELECT s.contado, COALESCE(u.nome, '') FROM caixa_sessao s LEFT JOIN usuario u ON u.id = s.fechado_por WHERE s.id = ?1",
        [sessao_id],
        |l| Ok((l.get(0)?, l.get(1)?)),
    )?;
    let (vendas, total_vendido, troco): (i64, i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(total), 0), COALESCE(SUM(troco), 0) FROM venda
          WHERE sessao_id = ?1 AND tipo = 'venda' AND situacao = 'concluida'",
        [sessao_id],
        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
    )?;
    let canceladas: i64 = conn.query_row(
        "SELECT COUNT(*) FROM venda WHERE sessao_id = ?1 AND tipo = 'venda' AND situacao = 'cancelada'",
        [sessao_id],
        |l| l.get(0),
    )?;
    let mut c = conn.prepare_cached(
        "SELECT p.forma, SUM(p.valor) FROM venda_pagamento p JOIN venda v ON v.id = p.venda_id
          WHERE v.sessao_id = ?1 AND v.tipo = 'venda' AND v.situacao = 'concluida' GROUP BY p.forma",
    )?;
    let somas: Vec<(String, i64)> = c.query_map([sessao_id], |l| Ok((l.get(0)?, l.get(1)?)))?.collect::<Result<_, _>>()?;
    let mut por_forma = Vec::new();
    for (forma, nome, _) in FORMAS {
        if let Some((_, v)) = somas.iter().find(|s| s.0 == *forma) {
            // No dinheiro, mostra o que ficou na gaveta (recebido menos troco).
            let valor = if *forma == "dinheiro" { v - troco } else { *v };
            por_forma.push(PorForma {
                forma: forma.to_string(),
                nome: nome.to_string(),
                valor: numeros::escrever_dinheiro(valor),
                valor_centavos: valor,
            });
        }
    }
    let dinheiro_vendas: i64 = somas.iter().find(|s| s.0 == "dinheiro").map(|s| s.1).unwrap_or(0) - troco;
    let (recebimentos, recebido) = crate::receber::por_forma_na_sessao(conn, sessao_id)?;
    let mut recebido_por_forma = Vec::new();
    for (forma, nome, _) in FORMAS {
        if let Some((_, v)) = recebido.iter().find(|s| s.0 == *forma) {
            recebido_por_forma.push(PorForma {
                forma: forma.to_string(),
                nome: nome.to_string(),
                valor: numeros::escrever_dinheiro(*v),
                valor_centavos: *v,
            });
        }
    }
    let dinheiro_recebido: i64 = recebido.iter().find(|s| s.0 == "dinheiro").map(|s| s.1).unwrap_or(0);
    let total_recebido: i64 = recebido.iter().map(|s| s.1).sum();
    let mut c = conn.prepare_cached(
        "SELECT m.tipo, m.valor, m.motivo, COALESCE(u.nome, ''), m.criado_em
           FROM caixa_movimento m LEFT JOIN usuario u ON u.id = m.usuario_id
          WHERE m.sessao_id = ?1 ORDER BY m.criado_em",
    )?;
    let movs: Vec<(String, i64, String, String, String)> = c
        .query_map([sessao_id], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?)))?
        .collect::<Result<_, _>>()?;
    let sangrias: i64 = movs.iter().filter(|m| m.0 == "sangria").map(|m| m.1).sum();
    let suprimentos: i64 = movs.iter().filter(|m| m.0 == "suprimento").map(|m| m.1).sum();
    let devolvido = crate::devolucoes::dinheiro_na_sessao(conn, sessao_id)?;
    let esperado = sessao.valor_inicial + dinheiro_vendas + dinheiro_recebido + suprimentos - sangrias - devolvido;
    let diferenca = contado.map(|c| c - esperado).unwrap_or(0);
    Ok(Resumo {
        valor_inicial: numeros::escrever_dinheiro(sessao.valor_inicial),
        sessao,
        vendas,
        canceladas,
        total_vendido: numeros::escrever_dinheiro(total_vendido),
        por_forma,
        recebimentos,
        total_recebido: numeros::escrever_dinheiro(total_recebido),
        recebido_por_forma,
        troco: numeros::escrever_dinheiro(troco),
        sangrias: numeros::escrever_dinheiro(sangrias),
        suprimentos: numeros::escrever_dinheiro(suprimentos),
        devolvido: numeros::escrever_dinheiro(devolvido),
        movimentos: movs
            .into_iter()
            .map(|m| Movimento {
                tipo: m.0,
                valor: numeros::escrever_dinheiro(m.1),
                motivo: m.2,
                usuario: m.3,
                quando: m.4,
            })
            .collect(),
        esperado: numeros::escrever_dinheiro(esperado),
        esperado_dinheiro: esperado,
        contado: contado.map(numeros::escrever_dinheiro).unwrap_or_default(),
        diferenca: if contado.is_some() { numeros::escrever_dinheiro(diferenca) } else { String::new() },
        diferenca_centavos: diferenca,
        fechado_por,
    })
}

/// Fecha o caixa com o dinheiro contado na gaveta. Devolve o id da sessão.
pub fn fechar(conn: &Connection, dispositivo: &str, usuario_id: &str, contado: &str, observacoes: &str) -> Resultado<String> {
    let sessao = sessao_aberta(conn, dispositivo)?.ok_or_else(|| Erro::validacao("O caixa já está fechado."))?;
    let contado = numeros::ler_dinheiro(contado)
        .map_err(|m| Erro::campo("contado", m))?
        .ok_or_else(|| Erro::campo("contado", "Conte o dinheiro da gaveta e digite o total."))?;
    if contado < 0 {
        return Err(Erro::campo("contado", "O valor não pode ser negativo."));
    }
    let r = resumo(conn, &sessao.id)?;
    let observacoes = observacoes.trim().chars().take(500).collect::<String>();
    conn.execute(
        "UPDATE caixa_sessao SET fechado_em = ?2, fechado_por = ?3, contado = ?4, esperado = ?5, observacoes = ?6 WHERE id = ?1",
        params![sessao.id, tempo::agora(), usuario_id, contado, r.esperado_dinheiro, observacoes],
    )?;
    let diferenca = contado - r.esperado_dinheiro;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "caixa.fechar",
        "caixa",
        &sessao.id,
        &[
            format!("Esperado em dinheiro: {}", r.esperado),
            format!("Contado: {}", numeros::escrever_dinheiro(contado)),
            format!("Diferença: {}", numeros::escrever_dinheiro(diferenca)),
        ],
    )?;
    Ok(sessao.id)
}

/// Últimos caixas deste computador.
pub fn historico(conn: &Connection, dispositivo: &str, limite: i64) -> Resultado<Vec<Sessao>> {
    let mut c = conn.prepare(
        "SELECT s.id, s.caixa_nome, s.aberto_em, COALESCE(u.nome, ''), s.valor_inicial, s.fechado_em
           FROM caixa_sessao s LEFT JOIN usuario u ON u.id = s.aberto_por
          WHERE s.dispositivo = ?1 ORDER BY s.aberto_em DESC LIMIT ?2",
    )?;
    let linhas = c.query_map(params![dispositivo, limite], ler_sessao)?;
    Ok(linhas.collect::<Result<_, _>>()?)
}
