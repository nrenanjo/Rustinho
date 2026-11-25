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
    /// 10b (C8, dev3.3): a conta a pagar do fechamento ("Ver no A pagar").
    pub conta_pagar_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Relatorio {
    pub de: String,
    pub ate: String,
    pub vendedores: Vec<Vendedor>,
    pub total: String,
    pub fechamentos: Vec<Fechamento>,
    /// R3: o período mostrado termina até ontem (só assim dá para fechar).
    pub pode_fechar: bool,
    /// R3: o período "até ontem" que a tela oferece quando este não pode ser fechado.
    pub de_ate_ontem: String,
    pub ontem: String,
}

/// Ontem no fuso do computador, "AAAA-MM-DD".
fn ontem(conn: &Connection) -> Resultado<String> {
    Ok(conn.query_row("SELECT date('now', 'localtime', '-1 day')", [], |l| l.get(0))?)
}

/// R3 (D-AUD-4, decisão do proprietário de 25/11/2026): a comissão fecha só
/// até ontem. Fechando até hoje, uma venda feita depois do fechamento, no
/// mesmo dia, ficava sem comissão para sempre (o dia já estava fechado). Sem
/// data final, vai até ontem; sem data inicial, começa no dia 1º do mês da
/// data final.
fn periodo_para_fechar(conn: &Connection, de: &str, ate: &str) -> Resultado<(String, String)> {
    let ontem = ontem(conn)?;
    let de = fiscal::ler_data(de).map_err(|m| Erro::campo("de", m))?;
    let ate = fiscal::ler_data(ate).map_err(|m| Erro::campo("ate", m))?.unwrap_or_else(|| ontem.clone());
    if ate > ontem {
        return Err(Erro::campo(
            "ate",
            format!(
                "A comissão fecha só até ontem ({}): as vendas de hoje ainda podem entrar. Termine o período ontem \
                 ou antes.",
                fiscal::data_br(&ontem)
            ),
        ));
    }
    let de = de.unwrap_or_else(|| format!("{}-01", &ate[..7]));
    if ate < de {
        return Err(Erro::campo("ate", "A data final é antes da inicial."));
    }
    Ok((de, ate))
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
fn calcular(
    conn: &Connection,
    usuario_id: &str,
    padrao: Option<i64>,
    de: &str,
    ate: &str,
) -> Resultado<(i64, i64, i64)> {
    // R23 (DP-22, decisão do proprietário de 25/11/2026): a devolução feita depois de fechada a comissão da venda
    // (um fechamento ativo que cobre o dia da venda, feito antes da devolução) não muda o período fechado: ela
    // desconta no período em que foi feita (a segunda consulta, abaixo). As outras devoluções descontam no
    // período da venda, como sempre. Desfeito o fechamento, a devolução volta ao período da venda.
    const DEPOIS_DO_FECHAMENTO: &str = "EXISTS (SELECT 1 FROM comissao_fechamento f
              WHERE f.usuario_id = v.vendedor_id AND f.cancelado_em IS NULL
                AND date(v.criado_em, 'localtime') BETWEEN f.de AND f.ate AND f.criado_em <= d.criado_em)";
    let mut c = conn.prepare_cached(&format!(
        "SELECT v.id, (SELECT COALESCE(SUM(i2.total), 0) FROM venda_item i2 WHERE i2.venda_id = v.id),
                v.total, i.total, i.comissao,
                COALESCE((SELECT SUM(di.valor_item) FROM devolucao_item di JOIN devolucao d ON d.id = di.devolucao_id
                           WHERE di.venda_item_id = i.id AND d.cancelado_em IS NULL AND NOT {DEPOIS_DO_FECHAMENTO}), 0),
                COALESCE((SELECT SUM(di.valor) FROM devolucao_item di JOIN devolucao d ON d.id = di.devolucao_id
                           WHERE di.venda_item_id = i.id AND d.cancelado_em IS NULL AND NOT {DEPOIS_DO_FECHAMENTO}), 0)
           FROM venda v JOIN venda_item i ON i.venda_id = v.id
          WHERE v.tipo = 'venda' AND v.situacao = 'concluida' AND v.vendedor_id = ?1
            AND date(v.criado_em, 'localtime') BETWEEN ?2 AND ?3"
    ))?;
    #[allow(clippy::type_complexity)]
    let linhas: Vec<(String, i64, i64, i64, Option<i64>, i64, i64)> = c
        .query_map(params![usuario_id, de, ate], |l| {
            Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?, l.get(6)?))
        })?
        .collect::<Result<_, _>>()?;
    let mut vendas: Vec<&str> = Vec::new();
    let (mut vendido, mut comissao) = (0_i64, 0_i128);
    for (venda, soma_itens, total, item, pct, devolvido_item, devolvido) in &linhas {
        if !vendas.contains(&venda.as_str()) {
            vendas.push(venda);
            vendido += total;
        }
        // O que o cliente devolveu não conta.
        vendido -= devolvido;
        // Parte do item no total da venda (já com o desconto da venda). O total do item já tem o desconto
        // do item; a proporção é pela soma dos totais dos itens (T069: pelo subtotal, de antes dos descontos
        // dos itens, o desconto do item era tirado duas vezes), como no rateio do DRE e da devolução.
        let item = item - devolvido_item;
        let base = if *soma_itens > 0 { item as i128 * *total as i128 / *soma_itens as i128 } else { 0 };
        let pct = pct.or(padrao).unwrap_or(0) as i128;
        comissao += base * pct;
    }
    // R23: as devoluções feitas neste período de vendas cuja comissão já estava fechada: descontam aqui.
    let mut d = conn.prepare_cached(&format!(
        "SELECT (SELECT COALESCE(SUM(i2.total), 0) FROM venda_item i2 WHERE i2.venda_id = v.id), v.total, i.comissao,
                SUM(di.valor_item), SUM(di.valor)
           FROM devolucao_item di JOIN devolucao d ON d.id = di.devolucao_id
           JOIN venda_item i ON i.id = di.venda_item_id JOIN venda v ON v.id = i.venda_id
          WHERE d.cancelado_em IS NULL AND v.tipo = 'venda' AND v.situacao = 'concluida' AND v.vendedor_id = ?1
            AND date(d.criado_em, 'localtime') BETWEEN ?2 AND ?3 AND {DEPOIS_DO_FECHAMENTO}
          GROUP BY i.id"
    ))?;
    #[allow(clippy::type_complexity)]
    let devolvidas: Vec<(i64, i64, Option<i64>, i64, i64)> = d
        .query_map(params![usuario_id, de, ate], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?)))?
        .collect::<Result<_, _>>()?;
    for (soma_itens, total, pct, devolvido_item, devolvido) in &devolvidas {
        vendido -= devolvido;
        let base = if *soma_itens > 0 { *devolvido_item as i128 * *total as i128 / *soma_itens as i128 } else { 0 };
        comissao -= base * pct.or(padrao).unwrap_or(0) as i128;
    }
    // pct em centésimos de ponto: base * pct / 10000, arredondado (o desconto do R23 pode deixar negativo:
    // arredonda do mesmo jeito, longe do zero no meio centavo).
    let arredondado = if comissao < 0 { -((-comissao + 5000) / 10000) } else { (comissao + 5000) / 10000 };
    let comissao = crate::numeros::saturar(arredondado);
    Ok((vendas.len() as i64, vendido, comissao))
}

/// O fechamento que já cobre o período: (id, de, até).
fn ja_fechado(conn: &Connection, usuario_id: &str, de: &str, ate: &str) -> Resultado<Option<(String, String, String)>> {
    Ok(conn
        .query_row(
            "SELECT id, de, ate FROM comissao_fechamento
              WHERE usuario_id = ?1 AND cancelado_em IS NULL AND de <= ?3 AND ate >= ?2 LIMIT 1",
            params![usuario_id, de, ate],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
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
        // B3 da revisão (R23): também quem só tem, no período, devolução a descontar (de venda de outro período).
        "SELECT u.id, u.nome, u.comissao FROM usuario u
          WHERE u.ativo = 1 OR EXISTS (SELECT 1 FROM venda v WHERE v.vendedor_id = u.id
                                        AND date(v.criado_em, 'localtime') BETWEEN ?1 AND ?2)
             OR EXISTS (SELECT 1 FROM devolucao d JOIN devolucao_item di ON di.devolucao_id = d.id
                          JOIN venda_item i ON i.id = di.venda_item_id JOIN venda v ON v.id = i.venda_id
                         WHERE v.vendedor_id = u.id AND d.cancelado_em IS NULL
                           AND date(d.criado_em, 'localtime') BETWEEN ?1 AND ?2)
          ORDER BY u.nome COLLATE NOCASE",
    )?;
    let usuarios: Vec<(String, String, Option<i64>)> =
        c.query_map(params![de, ate], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?.collect::<Result<_, _>>()?;
    let mut vendedores = Vec::new();
    let mut total = 0;
    for (id, nome, padrao) in usuarios {
        let (vendas, vendido, comissao) = calcular(conn, &id, padrao, &de, &ate)?;
        let fechado = ja_fechado(conn, &id, &de, &ate)?
            .map(|(_, a, b)| format!("{} a {}", curto(&a), curto(&b)))
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
                f.cancelado_em IS NOT NULL, COALESCE(f.conta_pagar_id, '')
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
                conta_pagar_id: l.get(10)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    let mut fechamentos = fechamentos;
    for f in &mut fechamentos {
        f.criado_em = fiscal::data_local_br(conn, &f.criado_em)?;
    }
    let ontem = ontem(conn)?;
    let de_ate_ontem = if de <= ontem { de.clone() } else { format!("{}-01", &ontem[..7]) };
    Ok(Relatorio {
        de: fiscal::data_br(&de),
        ate: fiscal::data_br(&ate),
        vendedores,
        total: numeros::escrever_dinheiro(total),
        fechamentos,
        pode_fechar: ate <= ontem,
        de_ate_ontem: fiscal::data_br(&de_ate_ontem),
        ontem: fiscal::data_br(&ontem),
    })
}

/// Fecha a comissão do período: vira uma conta a pagar (vence `vencimento`,
/// vazio = hoje). Devolve o id da conta.
pub fn fechar(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    vendedor_id: &str,
    de: &str,
    ate: &str,
    vencimento: &str,
) -> Resultado<String> {
    let (de, ate) = periodo_para_fechar(conn, de, ate)?;
    let (nome, padrao): (String, Option<i64>) = conn
        .query_row("SELECT nome, comissao FROM usuario WHERE id = ?1", [vendedor_id], |l| Ok((l.get(0)?, l.get(1)?)))
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Vendedor".into()))?;
    if let Some((id, a, b)) = ja_fechado(conn, vendedor_id, &de, &ate)? {
        // 10b (C8): a recusa diz qual fechamento já cobre o período (a tela o mostra destacado).
        return Err(Erro::campo(
            &format!("fechamento:{id}"),
            format!(
                "A comissão de {nome} já foi fechada de {} a {}. Escolha outro período.",
                fiscal::data_br(&a),
                fiscal::data_br(&b)
            ),
        ));
    }
    let (vendas, vendido, comissao) = calcular(conn, vendedor_id, padrao, &de, &ate)?;
    if comissao < 0 {
        // R23: as devoluções deste período (de vendas com a comissão já fechada) passam da comissão dele.
        return Err(Erro::validacao(format!(
            "As devoluções deste período deixam a comissão de {nome} negativa (R$ -{}). Feche um período maior, que \
             inclua vendas.",
            numeros::escrever_dinheiro(-comissao)
        )));
    }
    if comissao == 0 {
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
            // A comissão é despesa do mês em que o período fecha (DRE).
            competencia: ate.chars().take(7).collect(),
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
    #[allow(clippy::type_complexity)]
    let (conta, cancelado, vendedor_id, de, ate, valor): (
        Option<String>,
        Option<String>,
        String,
        String,
        String,
        i64,
    ) = conn
        .query_row(
            "SELECT conta_pagar_id, cancelado_em, usuario_id, de, ate, valor FROM comissao_fechamento WHERE id = ?1",
            [id],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?)),
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Fechamento".into()))?;
    if cancelado.is_some() {
        return Err(Erro::validacao("Este fechamento já foi desfeito."));
    }
    // B3 da revisão (R23): uma devolução de venda deste período, feita depois deste fechamento, já foi descontada
    // por um fechamento mais novo, ainda ativo. Desfazer este a faria voltar para cá e descontar duas vezes: primeiro
    // desfaz-se o mais novo.
    let mais_novo: Option<(String, String)> = conn
        .query_row(
            "SELECT f2.de, f2.ate FROM comissao_fechamento f
               JOIN venda v ON v.vendedor_id = f.usuario_id AND v.tipo = 'venda'
                AND date(v.criado_em, 'localtime') BETWEEN f.de AND f.ate
               JOIN venda_item i ON i.venda_id = v.id
               JOIN devolucao_item di ON di.venda_item_id = i.id
               JOIN devolucao d ON d.id = di.devolucao_id AND d.cancelado_em IS NULL AND f.criado_em <= d.criado_em
               JOIN comissao_fechamento f2 ON f2.usuario_id = f.usuario_id AND f2.id <> f.id AND f2.cancelado_em IS NULL
                AND date(d.criado_em, 'localtime') BETWEEN f2.de AND f2.ate AND d.criado_em <= f2.criado_em
              WHERE f.id = ?1
              ORDER BY f2.ate LIMIT 1",
            [id],
            |l| Ok((l.get(0)?, l.get(1)?)),
        )
        .optional()?;
    if let Some((a, b)) = mais_novo {
        return Err(Erro::validacao(format!(
            "O fechamento de {} a {} já descontou uma devolução de venda deste período. Desfaça antes o fechamento de \
             {} a {}.",
            fiscal::data_br(&a),
            fiscal::data_br(&b),
            fiscal::data_br(&a),
            fiscal::data_br(&b)
        )));
    }
    if let Some(c) = conta {
        let cancelada: Option<String> =
            conn.query_row("SELECT cancelado_em FROM conta_pagar WHERE id = ?1", [&c], |l| l.get(0))?;
        if cancelada.is_none() {
            pagar::cancelar(conn, dispositivo, usuario_id, &c, "Fechamento de comissão desfeito").map_err(
                |e| match e {
                    // M-1 da revisão (C8): a recusa do registro inconsistente (caso 7) passa como está; só a de "conta
                    // com pagamento" vira "já foi paga".
                    Erro::Validacao { campo: Some(ref campo), .. }
                        if campo.starts_with(crate::sync::CAMPO_REFERENCIA) =>
                    {
                        e
                    }
                    Erro::Validacao { .. } => {
                        // 10b (C8): a recusa diz qual é a conta paga (a tela oferece "Ver no A pagar").
                        Erro::campo(
                            &format!("conta_paga:{c}"),
                            "A comissão já foi paga. Estorne o pagamento no A pagar antes.",
                        )
                    }
                    outro => outro,
                },
            )?;
        }
    }
    conn.execute("UPDATE comissao_fechamento SET cancelado_em = ?2 WHERE id = ?1", params![id, tempo::agora()])?;
    // E10: desfazer o fechamento fica no histórico do vendedor (antes, só a
    // conta cancelada aparecia, sem dizer de qual fechamento).
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "comissao.reabrir",
        "usuario",
        &vendedor_id,
        &[format!(
            "Fechamento de {} a {} desfeito (comissão R$ {})",
            fiscal::data_br(&de),
            fiscal::data_br(&ate),
            numeros::escrever_dinheiro(valor)
        )],
    )?;
    Ok(())
}

// ------------------------------------------------------------- comandos das telas

pub use comandos::COMANDOS;

/// Os comandos das comissões (etapa 3 da modularização: vieram do `match` do
/// `api.rs` sem mudar nome, entrada, resposta nem permissão).
mod comandos {
    use serde_json::{json, Value};

    use crate::comissoes;
    use crate::despacho::{ler, marcar_feita, Classe, Comando, Id, Pedido, Periodo};
    use crate::erro::Resultado;

    pub const COMANDOS: &[Comando] = &[
        Comando { nome: "comissoes", classe: Classe::ConsultaDoGerente("financeiro"), funcao: comissoes },
        Comando { nome: "comissao_fechar", classe: Classe::Gerente("financeiro"), funcao: comissao_fechar },
        Comando { nome: "comissao_reabrir", classe: Classe::Gerente("financeiro"), funcao: comissao_reabrir },
    ];

    fn comissoes(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: Periodo = ler(dados)?;
        Ok(json!(comissoes::relatorio(p.conn, &d.de, &d.ate)?))
    }

    fn comissao_fechar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: Periodo = ler(dados)?;
        let tx = p.conn.transaction()?;
        comissoes::fechar(&tx, p.disp, p.usuario_id, &d.usuario_id, &d.de, &d.ate, &d.vencimento)?;
        let resposta = json!(comissoes::relatorio(&tx, &d.de, &d.ate)?);
        marcar_feita(&tx, p.operacao.as_ref(), &resposta)?;
        tx.commit()?;
        Ok(resposta)
    }

    fn comissao_reabrir(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: Id = ler(dados)?;
        let tx = p.conn.transaction()?;
        comissoes::reabrir(&tx, p.disp, p.usuario_id, &d.id)?;
        let resposta = Value::Null;
        marcar_feita(&tx, p.operacao.as_ref(), &resposta)?;
        tx.commit()?;
        Ok(resposta)
    }
}
