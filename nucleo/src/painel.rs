//! Painel: como a loja está indo (hoje, no mês e no ano), e quem compra,
//! de quem a loja compra e como os clientes pagam.
//!
//! Tudo sai das vendas concluídas (orçamento e venda cancelada não contam).
//! As datas são as do computador (o banco guarda em UTC).

use std::collections::HashMap;

use rusqlite::{params, Connection};
use serde::Serialize;
use time::{Date, Month};

use crate::erro::Resultado;
use crate::{caixa, devolucoes, fiscal, numeros, receber, tempo};

const MESES: [&str; 12] = ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];
const MESES_NOME: [&str; 12] = [
    "janeiro",
    "fevereiro",
    "março",
    "abril",
    "maio",
    "junho",
    "julho",
    "agosto",
    "setembro",
    "outubro",
    "novembro",
    "dezembro",
];

/// Uma barra dos gráficos.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Barra {
    pub rotulo: String,
    /// Texto do balão ao passar o mouse (ex.: "15/10 · 4 vendas").
    pub dica: String,
    pub valor: String,
    /// 0 a 1, em relação à maior barra.
    pub parte: f64,
    pub destaque: bool,
    /// Dia que ainda não chegou.
    pub futuro: bool,
}

/// Uma linha de ranking (formas de pagamento, peças, vendedores).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Linha {
    pub nome: String,
    pub detalhe: String,
    pub valor: String,
    /// 0 a 1, em relação à maior.
    pub parte: f64,
    /// Percentual do total ("42%").
    pub percentual: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bloco {
    pub total: String,
    pub vendas: i64,
    pub ticket: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resumo {
    pub hoje: Bloco,
    pub mes: Bloco,
    pub mes_nome: String,
    /// "+12% que setembro, até o dia 15" (vazio se não houver base).
    pub comparacao: String,
    pub comparacao_subiu: bool,
    pub ano: Bloco,
    pub ano_numero: String,
    /// Lucro bruto do mês (DRE gerencial).
    pub lucro_mes: String,
    pub margem_mes: String,
    /// Peças vendidas no mês sem custo (o lucro fica incompleto).
    pub sem_custo: i64,
    /// Devoluções do mês (à parte das vendas líquidas).
    pub devolvido_mes: String,
    pub a_receber: String,
    pub receber_vencido: String,
    pub pagar_7_dias: String,
    pub pagar_vencido: String,
    pub por_dia: Vec<Barra>,
    pub por_mes: Vec<Barra>,
    pub por_hora: Vec<Barra>,
    pub formas: Vec<Linha>,
    pub mais_vendidos: Vec<Linha>,
    pub vendedores: Vec<Linha>,
}

fn data(iso: &str) -> Option<Date> {
    receber::ler_iso(iso).ok()
}

fn iso(d: Date) -> String {
    format!("{:04}-{:02}-{:02}", d.year(), d.month() as u8, d.day())
}

fn mes_anterior(ano: i32, mes: Month) -> (i32, Month) {
    if mes == Month::January {
        (ano - 1, Month::December)
    } else {
        (ano, mes.previous())
    }
}

fn percentual(parte: i64, total: i64) -> String {
    if total <= 0 {
        return String::new();
    }
    format!("{}%", (parte as i128 * 100 + total as i128 / 2) / total as i128)
}

fn proporcao(v: i64, maior: i64) -> f64 {
    if maior <= 0 || v <= 0 {
        0.0
    } else {
        (v as f64 / maior as f64).clamp(0.0, 1.0)
    }
}

fn bloco(total: i64, vendas: i64) -> Bloco {
    Bloco {
        total: numeros::escrever_dinheiro(total),
        vendas,
        // D-11: arredonda (como a DRE), não corta os centavos.
        ticket: if vendas > 0 { numeros::escrever_dinheiro((total + vendas / 2) / vendas) } else { String::new() },
    }
}

/// Soma e conta as vendas concluídas (e não canceladas) entre duas datas
/// locais (inclusive), já com os descontos: as "vendas líquidas" da DRE
/// gerencial. As devoluções aparecem à parte (veja `dre`).
fn somar(conn: &Connection, de: &str, ate: &str) -> Resultado<(i64, i64)> {
    let (total, n): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(total), 0), COUNT(*) FROM venda
          WHERE tipo = 'venda' AND situacao = 'concluida' AND criado_em >= ?3
            AND date(criado_em, 'localtime') BETWEEN ?1 AND ?2",
        params![de, ate, utc_desde(de)],
        |l| Ok((l.get(0)?, l.get(1)?)),
    )?;
    Ok((total, n))
}

/// Corte em UTC um dia antes da data local, para usar o índice de `criado_em`
/// sem perder venda por causa do fuso.
fn utc_desde(data_local: &str) -> String {
    match data(data_local) {
        Some(d) => format!("{}T00:00:00Z", iso(d.previous_day().unwrap_or(d))),
        None => String::new(),
    }
}

pub fn resumo(conn: &Connection) -> Resultado<Resumo> {
    let hoje_txt = fiscal::hoje(conn)?;
    let hoje = data(&hoje_txt).unwrap_or(Date::MIN);
    let (ano, mes, dia) = (hoje.year(), hoje.month(), hoje.day());
    let inicio_mes = Date::from_calendar_date(ano, mes, 1).unwrap_or(hoje);
    let inicio_ano = Date::from_calendar_date(ano, Month::January, 1).unwrap_or(hoje);

    let (t_hoje, n_hoje) = somar(conn, &hoje_txt, &hoje_txt)?;
    let (t_mes, n_mes) = somar(conn, &iso(inicio_mes), &hoje_txt)?;
    let (t_ano, n_ano) = somar(conn, &iso(inicio_ano), &hoje_txt)?;

    // Mesmo pedaço do mês passado (do dia 1 até o dia de hoje).
    let (ano_ant, mes_ant) = mes_anterior(ano, mes);
    let fim_ant = dia.min(mes_ant.length(ano_ant));
    let (t_ant, _) = somar(
        conn,
        &iso(Date::from_calendar_date(ano_ant, mes_ant, 1).unwrap_or(hoje)),
        &iso(Date::from_calendar_date(ano_ant, mes_ant, fim_ant).unwrap_or(hoje)),
    )?;
    let nome_ant = MESES_NOME[mes_ant as usize - 1];
    let (comparacao, subiu) = if t_ant > 0 {
        let p = crate::numeros::saturar((t_mes as i128 - t_ant as i128) * 100 / t_ant as i128);
        let ate = if dia == 1 { "no dia 1".to_string() } else { format!("até o dia {dia}") };
        if p == 0 {
            (format!("igual a {nome_ant}, {ate}"), true)
        } else if p > 0 {
            (format!("+{p}% que {nome_ant}, {ate}"), true)
        } else {
            (format!("{p}% que {nome_ant}, {ate}"), false)
        }
    } else {
        (String::new(), true)
    };

    // Lucro bruto do mês: o da DRE gerencial (peça sem custo deixa o lucro
    // incompleto; nunca conta como lucro inteiro).
    let desde_mes = utc_desde(&iso(inicio_mes));
    let dre_mes = crate::dre::apurar_dre(conn, &crate::dre::Filtro { periodo: "mes".into(), ..Default::default() })?;
    let sem_custo = dre_mes.pendencias.iter().find(|p| p.codigo == "sem_custo").map(|p| p.quantidade).unwrap_or(0);
    let (devolvido_mes, _) = devolucoes::no_periodo(conn, &iso(inicio_mes), &hoje_txt)?;

    // A receber (crediário) e a pagar.
    let (a_receber, receber_vencido): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(valor - pago), 0), COALESCE(SUM(CASE WHEN vencimento < ?1 THEN valor - pago ELSE 0 END), 0)
           FROM conta_receber_saldo WHERE cancelado_em IS NULL AND valor > pago",
        [&hoje_txt],
        |l| Ok((l.get(0)?, l.get(1)?)),
    )?;
    let pagar = crate::pagar::painel(conn, "", "nenhuma")?;

    // Vendas por dia do mês.
    let mut por_dia_mapa: HashMap<String, (i64, i64)> = HashMap::new();
    {
        let mut c = conn.prepare(
            "SELECT date(criado_em, 'localtime') AS d, SUM(total), COUNT(*) FROM venda
              WHERE tipo = 'venda' AND situacao = 'concluida' AND criado_em >= ?2
                AND date(criado_em, 'localtime') >= ?1
              GROUP BY d",
        )?;
        for l in c.query_map(params![iso(inicio_mes), desde_mes], |l| {
            Ok((l.get::<_, String>(0)?, l.get::<_, i64>(1)?, l.get::<_, i64>(2)?))
        })? {
            let (d, t, n) = l?;
            por_dia_mapa.insert(d, (t, n));
        }
    }
    let dias_mes = mes.length(ano);
    let maior_dia = por_dia_mapa.values().map(|v| v.0).max().unwrap_or(0);
    let mut por_dia = Vec::new();
    for d in 1..=dias_mes {
        let chave = format!("{:04}-{:02}-{:02}", ano, mes as u8, d);
        let (t, n) = por_dia_mapa.get(&chave).copied().unwrap_or((0, 0));
        por_dia.push(Barra {
            rotulo: d.to_string(),
            dica: format!("{:02}/{:02} · {} {}", d, mes as u8, n, if n == 1 { "venda" } else { "vendas" }),
            valor: numeros::escrever_dinheiro(t),
            parte: proporcao(t, maior_dia),
            destaque: d == dia,
            futuro: d > dia,
        });
    }

    // Últimos 12 meses.
    let mut meses: Vec<(i32, Month)> = Vec::new();
    let (mut a, mut m) = (ano, mes);
    for _ in 0..12 {
        meses.push((a, m));
        (a, m) = mes_anterior(a, m);
    }
    meses.reverse();
    let primeiro = meses.first().copied().unwrap_or((ano, mes));
    let inicio_12 = iso(Date::from_calendar_date(primeiro.0, primeiro.1, 1).unwrap_or(hoje));
    let mut por_mes_mapa: HashMap<String, (i64, i64)> = HashMap::new();
    {
        let mut c = conn.prepare(
            "SELECT strftime('%Y-%m', criado_em, 'localtime') AS m, SUM(total), COUNT(*) FROM venda
              WHERE tipo = 'venda' AND situacao = 'concluida' AND criado_em >= ?2
                AND date(criado_em, 'localtime') >= ?1
              GROUP BY m",
        )?;
        for l in c.query_map(params![inicio_12, utc_desde(&inicio_12)], |l| {
            Ok((l.get::<_, String>(0)?, l.get::<_, i64>(1)?, l.get::<_, i64>(2)?))
        })? {
            let (k, t, n) = l?;
            por_mes_mapa.insert(k, (t, n));
        }
    }
    let maior_mes = por_mes_mapa.values().map(|v| v.0).max().unwrap_or(0);
    let por_mes = meses
        .iter()
        .map(|(a, m)| {
            let (t, n) = por_mes_mapa.get(&format!("{:04}-{:02}", a, *m as u8)).copied().unwrap_or((0, 0));
            Barra {
                rotulo: MESES[*m as usize - 1].to_string(),
                dica: format!(
                    "{} de {} · {} {}",
                    MESES_NOME[*m as usize - 1],
                    a,
                    n,
                    if n == 1 { "venda" } else { "vendas" }
                ),
                valor: numeros::escrever_dinheiro(t),
                parte: proporcao(t, maior_mes),
                destaque: *a == ano && *m == mes,
                futuro: false,
            }
        })
        .collect();

    // Movimento por hora (últimos 30 dias): quantas vendas em cada hora.
    let mut horas: HashMap<i64, i64> = HashMap::new();
    {
        let mut c = conn.prepare(
            "SELECT CAST(strftime('%H', criado_em, 'localtime') AS INTEGER) AS h, COUNT(*) FROM venda
              WHERE tipo = 'venda' AND situacao = 'concluida' AND criado_em >= ?1 GROUP BY h",
        )?;
        for l in c.query_map([tempo::dias_atras(30)], |l| Ok((l.get::<_, i64>(0)?, l.get::<_, i64>(1)?)))? {
            let (h, n) = l?;
            horas.insert(h, n);
        }
    }
    let (h_min, h_max) = {
        let com_venda: Vec<i64> = horas.keys().copied().collect();
        let min = com_venda.iter().min().copied().unwrap_or(8).min(8);
        let max = com_venda.iter().max().copied().unwrap_or(18).max(18);
        (min, max)
    };
    let maior_hora = horas.values().copied().max().unwrap_or(0);
    let por_hora = (h_min..=h_max)
        .map(|h| {
            let n = horas.get(&h).copied().unwrap_or(0);
            Barra {
                rotulo: format!("{h}h"),
                dica: format!("das {h}h às {}h · {} {} em 30 dias", h + 1, n, if n == 1 { "venda" } else { "vendas" }),
                valor: n.to_string(),
                parte: proporcao(n, maior_hora),
                destaque: n > 0 && n == maior_hora,
                futuro: false,
            }
        })
        .collect();

    let formas = formas_periodo(conn, &iso(inicio_mes), &hoje_txt)?.0;
    let mais_vendidos = pecas_periodo(conn, &iso(inicio_mes), &hoje_txt, 5)?;
    let vendedores = vendedores_periodo(conn, &iso(inicio_mes), &hoje_txt)?;

    Ok(Resumo {
        hoje: bloco(t_hoje, n_hoje),
        mes: bloco(t_mes, n_mes),
        mes_nome: MESES_NOME[mes as usize - 1].to_string(),
        comparacao,
        comparacao_subiu: subiu,
        ano: bloco(t_ano, n_ano),
        ano_numero: ano.to_string(),
        lucro_mes: numeros::escrever_dinheiro(dre_mes.numeros.lucro_bruto),
        margem_mes: dre_mes.margem_bruta.clone(),
        sem_custo,
        devolvido_mes: numeros::escrever_dinheiro(devolvido_mes),
        a_receber: numeros::escrever_dinheiro(a_receber),
        receber_vencido: numeros::escrever_dinheiro(receber_vencido),
        pagar_7_dias: pagar.vence_7_dias,
        pagar_vencido: pagar.vencido,
        por_dia,
        por_mes,
        por_hora,
        formas,
        mais_vendidos,
        vendedores,
    })
}

/// Formas de pagamento das vendas do período: (linhas, total, vendas).
fn formas_periodo(conn: &Connection, de: &str, ate: &str) -> Resultado<(Vec<Linha>, i64, i64)> {
    let desde = utc_desde(de);
    let mut c = conn.prepare(
        "SELECT p.forma, SUM(p.valor), COUNT(DISTINCT v.id) FROM venda_pagamento p JOIN venda v ON v.id = p.venda_id
          WHERE v.tipo = 'venda' AND v.situacao = 'concluida' AND v.criado_em >= ?3
            AND date(v.criado_em, 'localtime') BETWEEN ?1 AND ?2
          GROUP BY p.forma",
    )?;
    let mut somas: Vec<(String, i64, i64)> =
        c.query_map(params![de, ate, desde], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?.collect::<Result<_, _>>()?;
    // O dinheiro recebido inclui o troco: tira para ficar o que entrou.
    let troco: i64 = conn.query_row(
        "SELECT COALESCE(SUM(troco), 0) FROM venda
          WHERE tipo = 'venda' AND situacao = 'concluida' AND criado_em >= ?3
            AND date(criado_em, 'localtime') BETWEEN ?1 AND ?2",
        params![de, ate, desde],
        |l| l.get(0),
    )?;
    if let Some(d) = somas.iter_mut().find(|s| s.0 == "dinheiro") {
        d.1 -= troco;
    }
    let (_, vendas) = somar(conn, de, ate)?;
    somas.sort_by_key(|s| std::cmp::Reverse(s.1));
    let total = numeros::soma(somas.iter().map(|s| s.1))?;
    let maior = somas.first().map(|s| s.1).unwrap_or(0);
    let linhas = somas
        .into_iter()
        .filter(|s| s.1 > 0)
        .map(|(forma, valor, n)| Linha {
            nome: caixa::nome_forma(&forma).to_string(),
            detalhe: format!("{} {}", n, if n == 1 { "venda" } else { "vendas" }),
            valor: numeros::escrever_dinheiro(valor),
            parte: proporcao(valor, maior),
            percentual: percentual(valor, total),
        })
        .collect();
    Ok((linhas, total, vendas))
}

fn pecas_periodo(conn: &Connection, de: &str, ate: &str, limite: i64) -> Resultado<Vec<Linha>> {
    let mut c = conn.prepare(
        "SELECT COALESCE(p.nome, vi.descricao), COALESCE(p.marca, ''), COALESCE(p.unidade, vi.unidade),
                SUM(vi.quantidade - COALESCE(dv.q, 0)), SUM(vi.total - COALESCE(dv.v, 0))
           FROM venda_item vi JOIN venda v ON v.id = vi.venda_id
           LEFT JOIN produto p ON p.id = vi.produto_id
           LEFT JOIN (SELECT di.venda_item_id, SUM(di.quantidade) AS q, SUM(di.valor_item) AS v
                        FROM devolucao_item di JOIN devolucao d ON d.id = di.devolucao_id
                       WHERE d.cancelado_em IS NULL GROUP BY di.venda_item_id) dv ON dv.venda_item_id = vi.id
          WHERE v.tipo = 'venda' AND v.situacao = 'concluida' AND v.criado_em >= ?3
            AND date(v.criado_em, 'localtime') BETWEEN ?1 AND ?2
          GROUP BY COALESCE(vi.produto_id, vi.descricao)
         HAVING SUM(vi.quantidade - COALESCE(dv.q, 0)) > 0
          ORDER BY SUM(vi.total - COALESCE(dv.v, 0)) DESC
          LIMIT ?4",
    )?;
    let linhas: Vec<(String, String, String, i64, i64)> = c
        .query_map(params![de, ate, utc_desde(de), limite], |l| {
            Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?))
        })?
        .collect::<Result<_, _>>()?;
    let total = numeros::soma(linhas.iter().map(|l| l.4))?;
    let maior = linhas.first().map(|l| l.4).unwrap_or(0);
    Ok(linhas
        .into_iter()
        .map(|(nome, marca, unidade, q, t)| Linha {
            nome: if marca.is_empty() { nome } else { format!("{nome} · {marca}") },
            detalhe: format!("{} {}", numeros::escrever_quantidade(q), unidade),
            valor: numeros::escrever_dinheiro(t),
            parte: proporcao(t, maior),
            percentual: percentual(t, total),
        })
        .collect())
}

fn vendedores_periodo(conn: &Connection, de: &str, ate: &str) -> Resultado<Vec<Linha>> {
    let mut c = conn.prepare(
        "SELECT COALESCE(u.nome, 'Sem vendedor'), COUNT(*),
                SUM(v.total - COALESCE((SELECT SUM(d.valor) FROM devolucao d WHERE d.venda_id = v.id AND d.cancelado_em IS NULL), 0))
           FROM venda v LEFT JOIN usuario u ON u.id = v.vendedor_id
          WHERE v.tipo = 'venda' AND v.situacao = 'concluida' AND v.criado_em >= ?3
            AND date(v.criado_em, 'localtime') BETWEEN ?1 AND ?2
          GROUP BY v.vendedor_id
          ORDER BY SUM(v.total) DESC",
    )?;
    let linhas: Vec<(String, i64, i64)> = c
        .query_map(params![de, ate, utc_desde(de)], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let total = numeros::soma(linhas.iter().map(|l| l.2))?;
    let maior = linhas.first().map(|l| l.2).unwrap_or(0);
    Ok(linhas
        .into_iter()
        .map(|(nome, n, t)| Linha {
            nome,
            detalhe: format!("{} {}", n, if n == 1 { "venda" } else { "vendas" }),
            valor: numeros::escrever_dinheiro(t),
            parte: proporcao(t, maior),
            percentual: percentual(t, total),
        })
        .collect())
}

/// Começo do período de `dias` dias que termina hoje (data local).
fn periodo(conn: &Connection, dias: i64) -> Resultado<(String, String)> {
    let hoje = fiscal::hoje(conn)?;
    let d = data(&hoje).unwrap_or(Date::MIN);
    let de = d.checked_sub(time::Duration::days(dias.clamp(1, 3650) - 1)).unwrap_or(d);
    Ok((iso(de), hoje))
}

// ---------------------------------------------------------------------------
// Clientes
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClienteLinha {
    pub posicao: i64,
    pub cliente_id: String,
    pub nome: String,
    pub telefone: String,
    pub compras: i64,
    pub total: String,
    pub ticket: String,
    pub ultima: String,
    pub deve: String,
    pub vencido: bool,
    pub parte: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Clientes {
    pub dias: i64,
    pub total_com_cliente: String,
    /// Percentual das vendas (em valor) com cliente identificado.
    pub parte_com_cliente: String,
    pub clientes: i64,
    pub novos: i64,
    pub ticket: String,
    pub itens: Vec<ClienteLinha>,
}

pub fn clientes(conn: &Connection, dias: i64) -> Resultado<Clientes> {
    let (de, ate) = periodo(conn, dias)?;
    let desde = utc_desde(&de);
    let hoje = ate.clone();
    let mut c = conn.prepare(
        "SELECT v.cliente_id, COALESCE(MAX(p.nome), MAX(v.cliente_nome)), COALESCE(MAX(NULLIF(p.celular, '')), MAX(p.telefone), ''),
                COUNT(*), SUM(v.total), MAX(v.criado_em)
           FROM venda v LEFT JOIN pessoa p ON p.id = v.cliente_id
          WHERE v.tipo = 'venda' AND v.situacao = 'concluida' AND v.cliente_id IS NOT NULL AND v.criado_em >= ?3
            AND date(v.criado_em, 'localtime') BETWEEN ?1 AND ?2
          GROUP BY v.cliente_id
          ORDER BY SUM(v.total) DESC
          LIMIT 200",
    )?;
    let linhas: Vec<(String, String, String, i64, i64, String)> = c
        .query_map(params![de, ate, desde], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?)))?
        .collect::<Result<_, _>>()?;
    let (total_com, n_com, clientes_n): (i64, i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(total), 0), COUNT(*), COUNT(DISTINCT cliente_id) FROM venda
          WHERE tipo = 'venda' AND situacao = 'concluida' AND cliente_id IS NOT NULL AND criado_em >= ?3
            AND date(criado_em, 'localtime') BETWEEN ?1 AND ?2",
        params![de, ate, desde],
        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
    )?;
    let (total_geral, _) = somar(conn, &de, &ate)?;
    let novos: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pessoa WHERE cliente = 1 AND criado_em >= ?2 AND date(criado_em, 'localtime') >= ?1",
        params![de, desde],
        |l| l.get(0),
    )?;
    let mut devidas: HashMap<String, (i64, i64)> = HashMap::new();
    {
        let mut c = conn.prepare(
            "SELECT cliente_id, SUM(valor - pago), SUM(CASE WHEN vencimento < ?1 THEN valor - pago ELSE 0 END)
               FROM conta_receber_saldo WHERE cancelado_em IS NULL AND valor > pago GROUP BY cliente_id",
        )?;
        for l in c.query_map([&hoje], |l| Ok((l.get::<_, String>(0)?, l.get::<_, i64>(1)?, l.get::<_, i64>(2)?)))? {
            let (id, a, v) = l?;
            devidas.insert(id, (a, v));
        }
    }
    let maior = linhas.first().map(|l| l.4).unwrap_or(0);
    let mut itens = Vec::new();
    for (i, (id, nome, telefone, n, t, ultima)) in linhas.into_iter().enumerate() {
        let (deve, vencido) = devidas.get(&id).copied().unwrap_or((0, 0));
        itens.push(ClienteLinha {
            posicao: i as i64 + 1,
            cliente_id: id,
            nome,
            telefone,
            compras: n,
            total: numeros::escrever_dinheiro(t),
            ticket: numeros::escrever_dinheiro(if n > 0 { (t + n / 2) / n } else { 0 }),
            ultima: fiscal::data_local_br(conn, &ultima)?,
            deve: if deve > 0 { numeros::escrever_dinheiro(deve) } else { String::new() },
            vencido: vencido > 0,
            parte: proporcao(t, maior),
        });
    }
    Ok(Clientes {
        dias,
        total_com_cliente: numeros::escrever_dinheiro(total_com),
        parte_com_cliente: percentual(total_com, total_geral),
        clientes: clientes_n,
        novos,
        ticket: if n_com > 0 { numeros::escrever_dinheiro((total_com + n_com / 2) / n_com) } else { String::new() },
        itens,
    })
}

// ---------------------------------------------------------------------------
// Fornecedores
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FornecedorLinha {
    pub posicao: i64,
    pub fornecedor_id: String,
    pub nome: String,
    pub cidade: String,
    pub notas: i64,
    pub pecas: i64,
    pub total: String,
    pub ultima: String,
    pub percentual: String,
    pub parte: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fornecedores {
    pub dias: i64,
    pub total: String,
    pub notas: i64,
    pub fornecedores: i64,
    pub itens: Vec<FornecedorLinha>,
}

pub fn fornecedores(conn: &Connection, dias: i64) -> Resultado<Fornecedores> {
    let (de, ate) = periodo(conn, dias)?;
    let desde = utc_desde(&de);
    let mut c = conn.prepare(
        "SELECT c.fornecedor_id, COALESCE(MAX(NULLIF(p.fantasia, '')), MAX(p.nome), ''),
                COALESCE(MAX(p.municipio), '') || CASE WHEN MAX(p.uf) <> '' THEN '/' || MAX(p.uf) ELSE '' END,
                COUNT(*), SUM(c.total + c.frete_extra), MAX(c.lancada_em),
                (SELECT COUNT(DISTINCT ci.produto_id) FROM compra_item ci JOIN compra c2 ON c2.id = ci.compra_id
                  WHERE c2.fornecedor_id = c.fornecedor_id AND c2.situacao = 'lancada' AND c2.lancada_em >= ?3
                    AND date(c2.lancada_em, 'localtime') BETWEEN ?1 AND ?2 AND ci.produto_id IS NOT NULL)
           FROM compra c LEFT JOIN pessoa p ON p.id = c.fornecedor_id
          WHERE c.situacao = 'lancada' AND c.fornecedor_id IS NOT NULL AND c.lancada_em >= ?3
            AND date(c.lancada_em, 'localtime') BETWEEN ?1 AND ?2
          GROUP BY c.fornecedor_id
          ORDER BY SUM(c.total + c.frete_extra) DESC",
    )?;
    let linhas: Vec<(String, String, String, i64, i64, String, i64)> = c
        .query_map(params![de, ate, desde], |l| {
            Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?, l.get(6)?))
        })?
        .collect::<Result<_, _>>()?;
    let total = numeros::soma(linhas.iter().map(|l| l.4))?;
    let notas = numeros::soma(linhas.iter().map(|l| l.3))?;
    let maior = linhas.first().map(|l| l.4).unwrap_or(0);
    let mut itens = Vec::new();
    let n = linhas.len() as i64;
    for (i, (id, nome, cidade, notas_f, t, ultima, pecas)) in linhas.into_iter().enumerate() {
        itens.push(FornecedorLinha {
            posicao: i as i64 + 1,
            fornecedor_id: id,
            nome,
            cidade: if cidade == "/" { String::new() } else { cidade },
            notas: notas_f,
            pecas,
            total: numeros::escrever_dinheiro(t),
            ultima: fiscal::data_local_br(conn, &ultima)?,
            percentual: percentual(t, total),
            parte: proporcao(t, maior),
        });
    }
    Ok(Fornecedores { dias, total: numeros::escrever_dinheiro(total), notas, fornecedores: n, itens })
}

// ---------------------------------------------------------------------------
// Formas de pagamento
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagamentos {
    pub dias: i64,
    pub total: String,
    pub vendas: i64,
    /// Dinheiro, Pix e débito (entram na hora).
    pub na_hora: String,
    pub na_hora_parte: String,
    /// Crédito, cheque e crediário (entram depois).
    pub depois: String,
    pub depois_parte: String,
    pub itens: Vec<Linha>,
}

pub fn pagamentos(conn: &Connection, dias: i64) -> Resultado<Pagamentos> {
    let (de, ate) = periodo(conn, dias)?;
    let (itens, total, vendas) = formas_periodo(conn, &de, &ate)?;
    let mut na_hora = 0i64;
    let mut depois = 0i64;
    // Recalcula pelas formas (já sem o troco).
    for f in caixa::FORMAS {
        let nome = f.1;
        let v: i64 = itens
            .iter()
            .find(|l| l.nome == nome)
            .and_then(|l| numeros::ler_dinheiro(&l.valor).ok().flatten())
            .unwrap_or(0);
        match f.0 {
            "dinheiro" | "pix" | "debito" => na_hora += v,
            "credito" | "cheque" | "crediario" => depois += v,
            _ => {}
        }
    }

    Ok(Pagamentos {
        dias,
        total: numeros::escrever_dinheiro(total),
        vendas,
        na_hora: numeros::escrever_dinheiro(na_hora),
        na_hora_parte: percentual(na_hora, total),
        depois: numeros::escrever_dinheiro(depois),
        depois_parte: percentual(depois, total),
        itens,
    })
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::vendas::{self, ItemEntrada, PagamentoEntrada, VendaEntrada};
    use crate::{caixa, produtos, usuarios};

    fn peca(b: &Banco, u: &str, nome: &str, custo: &str) -> String {
        let mut f = produtos::ficha_nova(&b.conn).unwrap();
        f.nome = nome.into();
        f.custo = custo.into();
        f.estoque_inicial = "10".into();
        produtos::salvar(&b.conn, &b.dispositivo, u, &f).unwrap()
    }

    fn vender(b: &Banco, u: &str, p: &str, q: &str, forma: &str, valor: &str) {
        let e = VendaEntrada {
            itens: vec![ItemEntrada { produto_id: p.into(), quantidade: q.into(), ..Default::default() }],
            tabela: 1,
            pagamentos: vec![PagamentoEntrada { forma: forma.into(), valor: valor.into(), ..Default::default() }],
            ..Default::default()
        };
        vendas::concluir(&b.conn, &b.dispositivo, u, &e).unwrap();
    }

    #[test]
    fn resume_o_dia_o_mes_e_as_formas() {
        let b = Banco::em_memoria().unwrap();
        let (c, d) = (&b.conn, b.dispositivo.as_str());
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        caixa::abrir(c, d, &u, "Caixa 1", "0").unwrap();
        let filtro = peca(&b, &u, "Filtro", "10"); // preço 20
        let vela = peca(&b, &u, "Vela", "5"); // preço 10
        vender(&b, &u, &filtro, "2", "pix", "40");
        vender(&b, &u, &vela, "1", "dinheiro", "50"); // troco 40

        let r = resumo(c).unwrap();
        assert_eq!((r.hoje.total.as_str(), r.hoje.vendas), ("50,00", 2));
        assert_eq!(r.mes.total, "50,00");
        assert_eq!(r.hoje.ticket, "25,00");
        assert_eq!(r.lucro_mes, "25,00");
        assert_eq!(r.margem_mes, "50%");
        assert_eq!(r.formas.len(), 2);
        assert_eq!((r.formas[0].nome.as_str(), r.formas[0].valor.as_str()), ("Pix", "40,00"));
        // O dinheiro aparece sem o troco.
        assert_eq!((r.formas[1].nome.as_str(), r.formas[1].valor.as_str()), ("Dinheiro", "10,00"));
        assert_eq!(r.formas[0].percentual, "80%");
        assert!(r.por_dia.iter().any(|b| b.destaque && b.valor == "50,00" && b.parte == 1.0));
        assert_eq!(r.por_mes.len(), 12);
        assert!(r.por_mes.last().unwrap().destaque);
        assert_eq!(r.mais_vendidos[0].nome, "Filtro");
        assert_eq!(r.vendedores[0].nome, "Maria");

        let p = pagamentos(c, 30).unwrap();
        assert_eq!((p.total.as_str(), p.na_hora.as_str(), p.na_hora_parte.as_str()), ("50,00", "50,00", "100%"));
        assert_eq!(pagamentos(c, 365).unwrap().itens.len(), 2);

        let cl = clientes(c, 30).unwrap();
        assert_eq!((cl.clientes, cl.itens.len()), (0, 0));
        assert_eq!(fornecedores(c, 30).unwrap().itens.len(), 0);
    }
}
