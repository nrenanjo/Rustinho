//! Comissão dos vendedores.
//!
//! Cada item vendido paga a comissão da peça (se a peça tiver %) ou a do
//! vendedor. O desconto da venda inteira é dividido entre os itens. Venda
//! cancelada não conta. "Fechar" o período vira uma conta a pagar.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::erro::{Erro, Resultado};
use crate::{auditoria, fiscal, numeros, pagar, tempo};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vendedor {
    pub usuario_id: String,
    pub nome: String,
    /// % padrão do vendedor ("" = sem).
    pub percentual: String,
    pub vendas: i64,
    pub vendido: String,
    pub comissao: String,
    /// Período já fechado que cruza com o escolhido ("01/10 a 15/10").
    pub ja_fechado: String,
    #[serde(skip)]
    pub v_vendido: i64,
    #[serde(skip)]
    pub v_comissao: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fechamento {
    pub id: String,
    pub nome: String,
    pub periodo: String,
    pub vendas: i64,
    pub vendido: String,
    pub valor: String,
    pub criado_em: String,
    pub pago: bool,
    pub cancelado: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Relatorio {
    pub de: String,
    pub ate: String,
    pub vendedores: Vec<Vendedor>,
    pub total: String,
    pub fechamentos: Vec<Fechamento>,
}

fn periodo(conn: &Connection, de: &str, ate: &str) -> Resultado<(String, String)> {
    let hoje = fiscal::hoje(conn)?;
    let de = fiscal::ler_data(de).map_err(|m| Erro::campo("de", m))?.unwrap_or_else(|| format!("{}-01", &hoje[..7]));
    let ate = fiscal::ler_data(ate).map_err(|m| Erro::campo("ate", m))?.unwrap_or(hoje);
    if ate < de {
        return Err(Erro::campo("ate", "A data final é antes da inicial."));
    }
    Ok((de, ate))
}

/// (vendas, vendido, comissão) de um vendedor no período.
fn calcular(conn: &Connection, usuario_id: &str, padrao: Option<i64>, de: &str, ate: &str) -> Resultado<(i64, i64, i64)> {
    let mut c = conn.prepare_cached(
        "SELECT v.id, v.subtotal, v.total, i.total, i.comissao,
                COALESCE((SELECT SUM(di.valor_item) FROM devolucao_item di JOIN devolucao d ON d.id = di.devolucao_id
                           WHERE di.venda_item_id = i.id AND d.cancelado_em IS NULL), 0),
                COALESCE((SELECT SUM(di.valor) FROM devolucao_item di JOIN devolucao d ON d.id = di.devolucao_id
                           WHERE di.venda_item_id = i.id AND d.cancelado_em IS NULL), 0)
           FROM venda v JOIN venda_item i ON i.venda_id = v.id
          WHERE v.tipo = 'venda' AND v.situacao = 'concluida' AND v.vendedor_id = ?1
            AND date(v.criado_em, 'localtime') BETWEEN ?2 AND ?3",
    )?;
    #[allow(clippy::type_complexity)]
    let linhas: Vec<(String, i64, i64, i64, Option<i64>, i64, i64)> = c
        .query_map(params![usuario_id, de, ate], |l| {
            Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?, l.get(6)?))
        })?
        .collect::<Result<_, _>>()?;
    let mut vendas: Vec<&str> = Vec::new();
    let (mut vendido, mut comissao) = (0_i64, 0_i128);
    for (venda, subtotal, total, item, pct, devolvido_item, devolvido) in &linhas {
        if !vendas.contains(&venda.as_str()) {
            vendas.push(venda);
            vendido += total;
        }
        // O que o cliente devolveu não conta.
        vendido -= devolvido;
        // Parte do item no total da venda (já com o desconto da venda).
        let item = item - devolvido_item;
        let base = if *subtotal > 0 { item as i128 * *total as i128 / *subtotal as i128 } else { 0 };
        let pct = pct.or(padrao).unwrap_or(0) as i128;
        comissao += base * pct;
    }
    // pct em centésimos de ponto: base * pct / 10000, arredondado
    let comissao = ((comissao + 5000) / 10000) as i64;
    Ok((vendas.len() as i64, vendido, comissao))
}

fn ja_fechado(conn: &Connection, usuario_id: &str, de: &str, ate: &str) -> Resultado<Option<(String, String)>> {
    Ok(conn
        .query_row(
            "SELECT de, ate FROM comissao_fechamento
              WHERE usuario_id = ?1 AND cancelado_em IS NULL AND de <= ?3 AND ate >= ?2 LIMIT 1",
            params![usuario_id, de, ate],
            |l| Ok((l.get(0)?, l.get(1)?)),
        )
        .optional()?)
}

fn curto(d: &str) -> String {
    let b = fiscal::data_br(d);
    b.get(..5).unwrap_or(&b).to_string()
}

pub fn relatorio(conn: &Connection, de: &str, ate: &str) -> Resultado<Relatorio> {
    let (de, ate) = periodo(conn, de, ate)?;
    let mut c = conn.prepare(
        "SELECT u.id, u.nome, u.comissao FROM usuario u
          WHERE u.ativo = 1 OR EXISTS (SELECT 1 FROM venda v WHERE v.vendedor_id = u.id
                                        AND date(v.criado_em, 'localtime') BETWEEN ?1 AND ?2)
          ORDER BY u.nome COLLATE NOCASE",
    )?;
    let usuarios: Vec<(String, String, Option<i64>)> = c
        .query_map(params![de, ate], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let mut vendedores = Vec::new();
    let mut total = 0;
    for (id, nome, padrao) in usuarios {
        let (vendas, vendido, comissao) = calcular(conn, &id, padrao, &de, &ate)?;
        let fechado = ja_fechado(conn, &id, &de, &ate)?
            .map(|(a, b)| format!("{} a {}", curto(&a), curto(&b)))
            .unwrap_or_default();
        total += comissao;
        vendedores.push(Vendedor {
            usuario_id: id,
            nome,
            percentual: padrao.map(numeros::escrever_percentual).unwrap_or_default(),
            vendas,
            vendido: numeros::escrever_dinheiro(vendido),
            comissao: numeros::escrever_dinheiro(comissao),
            ja_fechado: fechado,
            v_vendido: vendido,
            v_comissao: comissao,
        });
    }
    let mut c = conn.prepare(
        "SELECT f.id, u.nome, f.de, f.ate, f.vendas, f.total_vendido, f.valor, f.criado_em,
                COALESCE((SELECT pago > 0 FROM conta_pagar_saldo WHERE id = f.conta_pagar_id), 0),
                f.cancelado_em IS NOT NULL
           FROM comissao_fechamento f JOIN usuario u ON u.id = f.usuario_id
          ORDER BY f.criado_em DESC LIMIT 30",
    )?;
    let fechamentos: Vec<Fechamento> = c
        .query_map([], |l| {
            let de: String = l.get(2)?;
            let ate: String = l.get(3)?;
            Ok(Fechamento {
                id: l.get(0)?,
                nome: l.get(1)?,
                periodo: format!("{} a {}", fiscal::data_br(&de), fiscal::data_br(&ate)),
                vendas: l.get(4)?,
                vendido: numeros::escrever_dinheiro(l.get(5)?),
                valor: numeros::escrever_dinheiro(l.get(6)?),
                criado_em: l.get(7)?,
                pago: l.get(8)?,
                cancelado: l.get(9)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    let mut fechamentos = fechamentos;
    for f in &mut fechamentos {
        f.criado_em = fiscal::data_local_br(conn, &f.criado_em)?;
    }
    Ok(Relatorio {
        de: fiscal::data_br(&de),
        ate: fiscal::data_br(&ate),
        vendedores,
        total: numeros::escrever_dinheiro(total),
        fechamentos,
    })
}

/// Fecha a comissão do período: vira uma conta a pagar (vence `vencimento`,
/// vazio = hoje). Devolve o id da conta.
pub fn fechar(conn: &Connection, dispositivo: &str, usuario_id: &str, vendedor_id: &str, de: &str, ate: &str, vencimento: &str) -> Resultado<String> {
    let (de, ate) = periodo(conn, de, ate)?;
    let (nome, padrao): (String, Option<i64>) = conn
        .query_row("SELECT nome, comissao FROM usuario WHERE id = ?1", [vendedor_id], |l| Ok((l.get(0)?, l.get(1)?)))
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Vendedor".into()))?;
    if let Some((a, b)) = ja_fechado(conn, vendedor_id, &de, &ate)? {
        return Err(Erro::validacao(format!(
            "A comissão de {nome} já foi fechada de {} a {}. Escolha outro período.",
            fiscal::data_br(&a),
            fiscal::data_br(&b)
        )));
    }
    let (vendas, vendido, comissao) = calcular(conn, vendedor_id, padrao, &de, &ate)?;
    if comissao <= 0 {
        return Err(Erro::validacao(format!("{nome} não tem comissão neste período.")));
    }
    let ids = pagar::lancar(
        conn,
        dispositivo,
        usuario_id,
        &pagar::LancamentoEntrada {
            categoria_id: "cat-comissoes".into(),
            descricao: format!("Comissão de {nome} · {} a {}", curto(&de), curto(&ate)),
            valor: numeros::escrever_dinheiro(comissao),
            parcelas: 1,
            primeiro_vencimento: vencimento.to_string(),
            ..Default::default()
        },
    )?;
    conn.execute(
        "INSERT INTO comissao_fechamento (id, usuario_id, de, ate, vendas, total_vendido, valor, conta_pagar_id, criado_em, criado_por, dispositivo)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![tempo::novo_id(), vendedor_id, de, ate, vendas, vendido, comissao, ids[0], tempo::agora(), usuario_id, dispositivo],
    )?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "comissao.fechar",
        "usuario",
        vendedor_id,
        &[format!(
            "{} a {}: {vendas} vendas, R$ {} vendido, comissão R$ {}",
            fiscal::data_br(&de),
            fiscal::data_br(&ate),
            numeros::escrever_dinheiro(vendido),
            numeros::escrever_dinheiro(comissao)
        )],
    )?;
    Ok(ids[0].clone())
}

/// Desfaz um fechamento (a conta a pagar sai, se não foi paga).
pub fn reabrir(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str) -> Resultado<()> {
    let (conta, cancelado): (Option<String>, Option<String>) = conn
        .query_row("SELECT conta_pagar_id, cancelado_em FROM comissao_fechamento WHERE id = ?1", [id], |l| Ok((l.get(0)?, l.get(1)?)))
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Fechamento".into()))?;
    if cancelado.is_some() {
        return Err(Erro::validacao("Este fechamento já foi desfeito."));
    }
    if let Some(c) = conta {
        let cancelada: Option<String> = conn.query_row("SELECT cancelado_em FROM conta_pagar WHERE id = ?1", [&c], |l| l.get(0))?;
        if cancelada.is_none() {
            pagar::cancelar(conn, dispositivo, usuario_id, &c, "Fechamento de comissão desfeito")
                .map_err(|e| match e {
                    Erro::Validacao { .. } => Erro::validacao("A comissão já foi paga. Estorne o pagamento no A pagar antes."),
                    outro => outro,
                })?;
        }
    }
    conn.execute("UPDATE comissao_fechamento SET cancelado_em = ?2 WHERE id = ?1", params![id, tempo::agora()])?;
    Ok(())
}
