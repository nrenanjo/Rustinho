//! DRE gerencial e indicadores de vendas (docs/DRE-gerencial.md).
//!
//! Não é a DRE contábil: é um "resultado gerencial estimado", por
//! competência, feito só com o que está registrado no Rustinho. Nada que
//! falta vira zero em silêncio: peça sem custo, despesa sem categoria, conta
//! sem competência e contas mensais fora do período aparecem como pendências.
//!
//! Como cada total é rastreável: tudo sai de uma lista de lançamentos
//! (`Lanc`), cada um com a sua origem (venda, devolução, conta...). Os totais
//! são a soma desses lançamentos, e o detalhe de uma linha é a lista deles.
//!
//! Valores em centavos; quantidades em milésimos; datas locais AAAA-MM-DD.

use std::collections::{BTreeMap, HashMap};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use time::{Date, Duration, Month, Weekday};

use crate::erro::{Erro, Resultado};
use crate::{auditoria, config, fiscal, numeros, receber, tempo};

// ---------------------------------------------------------------------------
// Linhas e grupos.
// ---------------------------------------------------------------------------

/// Grupos da classificação das categorias (o que vai em `categoria.dre_grupo`).
pub const GRUPOS: &[(&str, &str, &str)] = &[
    (
        "tributos_venda",
        "Tributos sobre as vendas",
        "Dedução da receita (ex.: DAS do Simples). Confirme com o contador.",
    ),
    ("variavel", "Despesas variáveis de venda", "Crescem com as vendas: comissões, taxas de cartão, fretes da loja."),
    ("fixa", "Despesas fixas", "Aluguel, salários, energia, internet, sistemas, contador, manutenção."),
    (
        "outra_despesa_operacional",
        "Outras despesas operacionais",
        "Gastos da operação que não são fixos nem variáveis.",
    ),
    ("outra_receita_operacional", "Outras receitas operacionais", "Entradas da operação que não são venda."),
    ("despesa_financeira", "Despesas financeiras", "Juros pagos, tarifas do banco, encargos."),
    ("receita_financeira", "Receitas financeiras", "Juros recebidos, rendimentos."),
    (
        "despesa_nao_operacional",
        "Despesas não operacionais",
        "Fora da atividade da loja (ex.: perda na venda de um bem).",
    ),
    ("receita_nao_operacional", "Receitas não operacionais", "Fora da atividade da loja (ex.: venda de um bem usado)."),
    ("estoque", "Compra de mercadoria", "Fica fora: o custo entra pelo CMV quando a peça é vendida."),
    ("fora", "Fora do resultado", "Empréstimos, aportes, retiradas, transferências."),
];

fn nome_grupo(g: &str) -> &'static str {
    GRUPOS.iter().find(|x| x.0 == g).map(|x| x.1).unwrap_or("Sem classificação")
}

/// Linha da DRE onde cai cada grupo, e se é linha de receita (soma) ou de
/// despesa/dedução (subtrai).
fn linha_do_grupo(grupo: Option<&str>) -> (&'static str, bool) {
    match grupo {
        Some("tributos_venda") => ("tributos", false),
        Some("variavel") => ("variaveis", false),
        Some("fixa") => ("fixas", false),
        Some("outra_despesa_operacional") => ("outras_despesas", false),
        Some("outra_receita_operacional") => ("outras_receitas", true),
        Some("despesa_financeira") => ("despesas_financeiras", false),
        Some("receita_financeira") => ("receitas_financeiras", true),
        Some("despesa_nao_operacional") => ("despesas_nao_operacionais", false),
        Some("receita_nao_operacional") => ("receitas_nao_operacionais", true),
        Some("estoque") | Some("fora") => ("fora", false),
        _ => ("sem_classificacao", false),
    }
}

/// Nome de cada linha (na ordem da DRE).
const NOMES: &[(&str, &str)] = &[
    ("receita_bruta", "Receita bruta de vendas"),
    ("descontos", "(−) Descontos concedidos"),
    ("devolucoes", "(−) Devoluções"),
    ("cancelamentos", "(−) Cancelamentos de vendas concluídas"),
    ("tributos", "(−) Tributos sobre as vendas"),
    ("receita_liquida", "= Receita líquida"),
    ("cmv", "(−) Custo das mercadorias vendidas (CMV)"),
    ("lucro_bruto", "= Lucro bruto"),
    ("variaveis", "(−) Despesas variáveis de venda"),
    ("margem_contribuicao", "= Margem de contribuição"),
    ("fixas", "(−) Despesas fixas"),
    ("outras_receitas", "(+) Outras receitas operacionais"),
    ("outras_despesas", "(−) Outras despesas operacionais"),
    ("sem_classificacao", "(−) Lançamentos sem classificação"),
    ("resultado_operacional", "= Resultado operacional estimado"),
    ("receitas_financeiras", "(+) Receitas financeiras"),
    ("despesas_financeiras", "(−) Despesas financeiras"),
    ("resultado_financeiro", "= Resultado financeiro"),
    ("receitas_nao_operacionais", "(+) Receitas não operacionais"),
    ("despesas_nao_operacionais", "(−) Despesas não operacionais"),
    ("resultado", "= Resultado gerencial estimado"),
];

fn nome_linha(id: &str) -> &'static str {
    NOMES.iter().find(|x| x.0 == id).map(|x| x.1).unwrap_or("")
}

// ---------------------------------------------------------------------------
// Períodos.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Periodo {
    pub tipo: String,
    pub de: String,
    pub ate: String,
    pub rotulo: String,
    /// O período ainda não terminou (hoje, mês atual, ano atual) ou é um
    /// pedaço de um mês.
    pub parcial: bool,
    /// Contas mensais (aluguel, salários...) podem entrar. Em "Hoje" e "7 dias", não.
    pub mensais: bool,
}

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

fn iso(d: Date) -> String {
    format!("{:04}-{:02}-{:02}", d.year(), d.month() as u8, d.day())
}

fn data(iso: &str) -> Resultado<Date> {
    receber::ler_iso(iso)
}

fn br(d: Date) -> String {
    format!("{:02}/{:02}/{:04}", d.day(), d.month() as u8, d.year())
}

fn primeiro_do_mes(d: Date) -> Date {
    d.replace_day(1).unwrap_or(d)
}

fn ultimo_do_mes(d: Date) -> Date {
    d.replace_day(d.month().length(d.year())).unwrap_or(d)
}

fn mes_anterior(d: Date) -> Date {
    primeiro_do_mes(primeiro_do_mes(d) - Duration::days(1))
}

/// O mesmo dia em outro mês (ou o último dia, se o mês for mais curto).
fn dia_no_mes(mes: Date, dia: u8) -> Date {
    mes.replace_day(dia.min(mes.month().length(mes.year()))).unwrap_or(mes)
}

fn periodo(tipo: &str, de: Date, ate: Date, rotulo: String, parcial: bool, mensais: bool) -> Periodo {
    Periodo { tipo: tipo.into(), de: iso(de), ate: iso(ate), rotulo, parcial, mensais }
}

fn rotulo_intervalo(de: Date, ate: Date) -> String {
    if de == ate {
        br(de)
    } else {
        format!("{} a {}", br(de), br(ate))
    }
}

/// O período pedido e o de comparação (veja docs/DRE-gerencial.md §3).
pub fn periodos(hoje: &str, tipo: &str, de: &str, ate: &str) -> Resultado<(Periodo, Periodo)> {
    let h = data(hoje)?;
    Ok(match tipo {
        "hoje" => {
            let o = h - Duration::days(1);
            (
                periodo("hoje", h, h, format!("Hoje, {}", br(h)), true, false),
                periodo("comparacao", o, o, format!("ontem, {}", br(o)), false, false),
            )
        }
        "7dias" => {
            let de = h - Duration::days(6);
            let (cd, ca) = (h - Duration::days(13), h - Duration::days(7));
            (
                periodo("7dias", de, h, format!("Últimos 7 dias ({})", rotulo_intervalo(de, h)), true, false),
                periodo(
                    "comparacao",
                    cd,
                    ca,
                    format!("7 dias anteriores ({})", rotulo_intervalo(cd, ca)),
                    false,
                    false,
                ),
            )
        }
        "mes_anterior" => {
            let m = mes_anterior(h);
            let a = mes_anterior(m);
            (
                periodo(
                    "mes_anterior",
                    m,
                    ultimo_do_mes(m),
                    format!("{} de {}", MESES_NOME[m.month() as usize - 1], m.year()),
                    false,
                    true,
                ),
                periodo(
                    "comparacao",
                    a,
                    ultimo_do_mes(a),
                    format!("{} de {}", MESES_NOME[a.month() as usize - 1], a.year()),
                    false,
                    true,
                ),
            )
        }
        "ano" => {
            let de = Date::from_calendar_date(h.year(), Month::January, 1).unwrap_or(h);
            let ade = Date::from_calendar_date(h.year() - 1, Month::January, 1).unwrap_or(de);
            let aate =
                Date::from_calendar_date(h.year() - 1, h.month(), 1).map(|m| dia_no_mes(m, h.day())).unwrap_or(ade);
            (
                periodo("ano", de, h, format!("{} até {}", h.year(), br(h)), true, true),
                periodo("comparacao", ade, aate, format!("{} até {}", h.year() - 1, br(aate)), true, true),
            )
        }
        "personalizado" => {
            let ler = |t: &str, campo: &str| -> Resultado<Date> {
                let d = fiscal::ler_data(t)
                    .map_err(|m| Erro::campo(campo, m))?
                    .ok_or_else(|| Erro::campo(campo, "Digite a data (dd/mm/aaaa)."))?;
                data(&d)
            };
            let (d1, d2) = (ler(de, "de")?, ler(ate, "ate")?);
            if d2 < d1 {
                return Err(Erro::campo("ate", "A data final é antes da inicial."));
            }
            if (d2 - d1).whole_days() > 366 * 5 {
                return Err(Erro::campo("ate", "Escolha um período de até 5 anos."));
            }
            let dias = (d2 - d1).whole_days() + 1;
            let (cd, ca) = (d1 - Duration::days(dias), d1 - Duration::days(1));
            (
                periodo("personalizado", d1, d2, rotulo_intervalo(d1, d2), d2 >= h, true),
                periodo("comparacao", cd, ca, format!("período anterior ({})", rotulo_intervalo(cd, ca)), false, true),
            )
        }
        // "mes" (padrão): do dia 1 até hoje; compara com o mesmo pedaço do mês anterior.
        _ => {
            let de = primeiro_do_mes(h);
            let m = mes_anterior(h);
            let ca = dia_no_mes(m, h.day());
            (
                periodo(
                    "mes",
                    de,
                    h,
                    format!("{} de {} (até o dia {})", MESES_NOME[h.month() as usize - 1], h.year(), h.day()),
                    h != ultimo_do_mes(h),
                    true,
                ),
                periodo(
                    "comparacao",
                    m,
                    ca,
                    format!("1 a {} de {}", ca.day(), MESES_NOME[m.month() as usize - 1]),
                    ca != ultimo_do_mes(m),
                    true,
                ),
            )
        }
    })
}

/// Uma conta mensal (competência AAAA-MM) entra no período? Só se o
/// período tem o mês inteiro, ou é o mês corrente até hoje.
fn mes_entra(p: &Periodo, mes: &str, hoje: &str) -> bool {
    if !p.mensais {
        return false;
    }
    let Ok(primeiro) = data(&format!("{mes}-01")) else { return false };
    let ultimo = iso(ultimo_do_mes(primeiro));
    let primeiro = iso(primeiro);
    p.de <= primeiro && (p.ate >= ultimo || (p.ate == hoje && primeiro.as_str() <= hoje && hoje <= ultimo.as_str()))
}

// ---------------------------------------------------------------------------
// Lançamentos.
// ---------------------------------------------------------------------------

/// Um lançamento que compõe uma linha da DRE.
#[derive(Debug, Clone)]
struct Lanc {
    linha: &'static str,
    /// Positivo aumenta a linha (uma dedução maior, uma despesa maior...).
    valor: i64,
    /// Data local (AAAA-MM-DD). Nas contas mensais, o dia 1 da competência.
    data: String,
    /// Competência (AAAA-MM) das contas mensais.
    mes: Option<String>,
    descricao: String,
    origem: &'static str,
    origem_id: String,
    categoria: String,
    fornecedor: String,
    /// Conta sem competência (usado o mês do vencimento).
    sem_competencia: bool,
}

/// Peça vendida sem custo no cadastro: a receita dela fica separada.
#[derive(Debug, Clone)]
struct SemCusto {
    data: String,
    descricao: String,
    receita: i64,
    venda_id: String,
}

#[derive(Default)]
struct Lido {
    lancs: Vec<Lanc>,
    sem_custo: Vec<SemCusto>,
    /// Custo de peças devolvidas com defeito (não voltaram ao estoque).
    perda_defeito: i64,
}

/// Corte em UTC um dia antes, para usar o índice de `criado_em` sem perder
/// nada por causa do fuso.
fn utc_desde(de: &str) -> String {
    match data(de) {
        Ok(d) => format!("{}T00:00:00Z", iso(d - Duration::days(1))),
        Err(_) => String::new(),
    }
}

fn custo_total(custo: i64, quantidade: i64) -> i64 {
    numeros::saturar((custo as i128 * quantidade as i128 + 500) / 1000)
}

/// Lê tudo o que compõe a DRE entre `de` e `ate` (datas locais). As contas
/// mensais vêm de todos os meses que tocam o período; quem decide se entram
/// é `apurar`. Com vendedor, só o lado das vendas.
fn ler(conn: &Connection, de: &str, ate: &str, vendedor: Option<&str>) -> Resultado<Lido> {
    let mut l = Lido::default();
    let desde = utc_desde(de);
    let vend = vendedor.unwrap_or("");
    let filtro_v = "(?4 = '' OR v.vendedor_id = ?4)";

    // Vendas (data da venda), inclusive as canceladas depois.
    {
        let mut c = conn.prepare(&format!(
            "SELECT v.id, v.numero, v.caixa_nome, date(v.criado_em, 'localtime'), v.subtotal, v.desconto
               FROM venda v
              WHERE v.tipo = 'venda' AND v.situacao IN ('concluida', 'cancelada') AND v.criado_em >= ?3
                AND date(v.criado_em, 'localtime') BETWEEN ?1 AND ?2 AND {filtro_v}"
        ))?;
        let linhas = c.query_map(params![de, ate, desde, vend], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })?;
        for x in linhas {
            let (id, numero, caixa, dia, subtotal, desconto) = x?;
            let descricao = format!("Venda {numero} · {caixa}");
            l.lancs.push(lanc("receita_bruta", subtotal, &dia, &descricao, "venda", &id));
            if desconto != 0 {
                l.lancs.push(lanc("descontos", desconto, &dia, &descricao, "venda", &id));
            }
        }
    }

    // Itens vendidos: CMV (ou peça sem custo).
    {
        let mut c = conn.prepare(&format!(
            "SELECT v.id, v.numero, v.caixa_nome, date(v.criado_em, 'localtime'), vi.descricao, vi.quantidade, vi.custo,
                    vi.total, vi.produto_id IS NOT NULL AND COALESCE(p.tipo, 'produto') <> 'servico'
               FROM venda_item vi JOIN venda v ON v.id = vi.venda_id LEFT JOIN produto p ON p.id = vi.produto_id
              WHERE v.tipo = 'venda' AND v.situacao IN ('concluida', 'cancelada') AND v.criado_em >= ?3
                AND date(v.criado_em, 'localtime') BETWEEN ?1 AND ?2 AND {filtro_v}"
        ))?;
        let linhas = c.query_map(params![de, ate, desde, vend], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, bool>(8)?,
            ))
        })?;
        for x in linhas {
            let (id, numero, caixa, dia, item, q, custo, total, peca) = x?;
            if !peca {
                continue;
            }
            if custo > 0 {
                let d = format!("Venda {numero} · {caixa} · {item}");
                l.lancs.push(lanc("cmv", custo_total(custo, q), &dia, &d, "venda", &id));
            } else {
                l.sem_custo.push(SemCusto {
                    data: dia,
                    descricao: format!("Venda {numero} · {caixa} · {item}"),
                    receita: total,
                    venda_id: id,
                });
            }
        }
    }

    // Cancelamentos (data do cancelamento): estorno da receita e volta do CMV.
    {
        let mut c = conn.prepare(&format!(
            "SELECT v.id, v.numero, v.caixa_nome, date(v.cancelado_em, 'localtime'), v.total
               FROM venda v
              WHERE v.tipo = 'venda' AND v.situacao = 'cancelada' AND v.cancelado_em >= ?3
                AND date(v.cancelado_em, 'localtime') BETWEEN ?1 AND ?2 AND {filtro_v}"
        ))?;
        let linhas = c.query_map(params![de, ate, desde, vend], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?;
        for x in linhas {
            let (id, numero, caixa, dia, total) = x?;
            l.lancs.push(lanc(
                "cancelamentos",
                total,
                &dia,
                &format!("Cancelamento da venda {numero} · {caixa}"),
                "venda",
                &id,
            ));
        }
        let mut c = conn.prepare(&format!(
            "SELECT v.id, v.numero, v.caixa_nome, date(v.cancelado_em, 'localtime'), vi.descricao, vi.quantidade, vi.custo
               FROM venda_item vi JOIN venda v ON v.id = vi.venda_id LEFT JOIN produto p ON p.id = vi.produto_id
              WHERE v.tipo = 'venda' AND v.situacao = 'cancelada' AND v.cancelado_em >= ?3
                AND date(v.cancelado_em, 'localtime') BETWEEN ?1 AND ?2 AND {filtro_v}
                AND vi.produto_id IS NOT NULL AND COALESCE(p.tipo, 'produto') <> 'servico' AND vi.custo > 0"
        ))?;
        let linhas = c.query_map(params![de, ate, desde, vend], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, i64>(6)?,
            ))
        })?;
        for x in linhas {
            let (id, numero, caixa, dia, item, q, custo) = x?;
            l.lancs.push(lanc(
                "cmv",
                -custo_total(custo, q),
                &dia,
                &format!("Cancelamento da venda {numero} · {caixa} · {item} (voltou ao estoque)"),
                "venda",
                &id,
            ));
        }
    }

    // Devoluções (data da devolução) e devoluções desfeitas (data em que foram desfeitas).
    for (coluna, sinal, sufixo) in [("criado_em", 1_i64, ""), ("cancelado_em", -1_i64, " (desfeita)")] {
        let mut c = conn.prepare(&format!(
            "SELECT d.id, d.numero, d.caixa_nome, date(d.{coluna}, 'localtime'), d.valor
               FROM devolucao d JOIN venda v ON v.id = d.venda_id
              WHERE d.{coluna} IS NOT NULL AND d.{coluna} >= ?3 AND date(d.{coluna}, 'localtime') BETWEEN ?1 AND ?2
                AND {filtro_v}"
        ))?;
        let linhas = c.query_map(params![de, ate, desde, vend], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?;
        for x in linhas {
            let (id, numero, caixa, dia, valor) = x?;
            l.lancs.push(lanc(
                "devolucoes",
                sinal * valor,
                &dia,
                &format!("Devolução {numero} · {caixa}{sufixo}"),
                "devolucao",
                &id,
            ));
        }
        let mut c = conn.prepare(&format!(
            "SELECT d.id, d.numero, d.caixa_nome, date(d.{coluna}, 'localtime'), di.descricao, di.quantidade, di.custo,
                    di.volta_estoque
               FROM devolucao_item di JOIN devolucao d ON d.id = di.devolucao_id JOIN venda v ON v.id = d.venda_id
              WHERE d.{coluna} IS NOT NULL AND d.{coluna} >= ?3 AND date(d.{coluna}, 'localtime') BETWEEN ?1 AND ?2
                AND {filtro_v} AND di.custo > 0"
        ))?;
        let linhas = c.query_map(params![de, ate, desde, vend], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, bool>(7)?,
            ))
        })?;
        for x in linhas {
            let (id, numero, caixa, dia, item, q, custo, volta) = x?;
            if volta {
                l.lancs.push(lanc(
                    "cmv",
                    -sinal * custo_total(custo, q),
                    &dia,
                    &format!("Devolução {numero} · {caixa} · {item} (voltou ao estoque){sufixo}"),
                    "devolucao",
                    &id,
                ));
            } else {
                l.perda_defeito += sinal * custo_total(custo, q);
            }
        }
    }

    if vendedor.is_some() {
        return Ok(l);
    }

    // Contas a pagar por competência (as de compra de mercadoria ficam fora).
    {
        let mut c = conn.prepare(
            "SELECT c.id, c.descricao, c.valor, c.vencimento, c.competencia, g.dre_grupo, COALESCE(g.nome, ''),
                    COALESCE(p.nome, '')
               FROM conta_pagar c LEFT JOIN categoria g ON g.id = c.categoria_id
               LEFT JOIN pessoa p ON p.id = c.fornecedor_id
              WHERE c.cancelado_em IS NULL AND c.compra_id IS NULL
                AND COALESCE(c.competencia, substr(c.vencimento, 1, 7)) BETWEEN substr(?1, 1, 7) AND substr(?2, 1, 7)",
        )?;
        let linhas = c.query_map(params![de, ate], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
            ))
        })?;
        for x in linhas {
            let (id, descricao, valor, vencimento, competencia, grupo, categoria, fornecedor) = x?;
            let sem_competencia = competencia.is_none();
            let mes = competencia.unwrap_or_else(|| vencimento.chars().take(7).collect());
            let (linha, receita) = linha_do_grupo(grupo.as_deref());
            let mut x = lanc(
                linha,
                if receita { -valor } else { valor },
                &format!("{mes}-01"),
                &format!("{descricao} · vence {}", fiscal::data_br(&vencimento)),
                "conta_pagar",
                &id,
            );
            x.mes = Some(mes);
            x.categoria = if categoria.is_empty() { "Sem categoria".into() } else { categoria };
            x.fornecedor = fornecedor;
            x.sem_competencia = sem_competencia;
            l.lancs.push(x);
        }
    }

    // Entradas e saídas avulsas nas contas da loja (pela data).
    {
        let mut c = conn.prepare(
            "SELECT m.id, m.data, m.valor, m.descricao, g.dre_grupo, COALESCE(g.nome, ''), COALESCE(k.nome, '')
               FROM movimento_conta m LEFT JOIN categoria g ON g.id = m.categoria_id
               LEFT JOIN conta_loja k ON k.id = m.conta_id
              WHERE m.estornado_em IS NULL AND m.tipo IN ('entrada', 'saida') AND m.data BETWEEN ?1 AND ?2",
        )?;
        let linhas = c.query_map(params![de, ate], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
            ))
        })?;
        for x in linhas {
            let (id, dia, valor, descricao, grupo, categoria, conta) = x?;
            let (linha, receita) = linha_do_grupo(grupo.as_deref());
            // valor > 0 entrou; < 0 saiu.
            let v = if receita { valor } else { -valor };
            let d =
                if descricao.is_empty() { format!("Movimento · {conta}") } else { format!("{descricao} · {conta}") };
            let mut x = lanc(linha, v, &dia, &d, "movimento", &id);
            x.categoria = if categoria.is_empty() { "Sem categoria".into() } else { categoria };
            l.lancs.push(x);
        }
    }

    // Juros pagos e descontos obtidos nos pagamentos.
    {
        let mut c = conn.prepare(
            "SELECT p.id, p.data, p.juros, p.desconto, c.descricao, COALESCE(f.nome, '')
               FROM pagamento p JOIN conta_pagar c ON c.id = p.conta_pagar_id LEFT JOIN pessoa f ON f.id = c.fornecedor_id
              WHERE p.estornado_em IS NULL AND (p.juros <> 0 OR p.desconto <> 0) AND p.data BETWEEN ?1 AND ?2",
        )?;
        let linhas = c.query_map(params![de, ate], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })?;
        for x in linhas {
            let (id, dia, juros, desconto, descricao, fornecedor) = x?;
            if juros != 0 {
                let mut x =
                    lanc("despesas_financeiras", juros, &dia, &format!("Juros pagos · {descricao}"), "pagamento", &id);
                x.fornecedor = fornecedor.clone();
                x.categoria = "Juros pagos".into();
                l.lancs.push(x);
            }
            if desconto != 0 {
                let mut x = lanc(
                    "receitas_financeiras",
                    desconto,
                    &dia,
                    &format!("Desconto obtido · {descricao}"),
                    "pagamento",
                    &id,
                );
                x.fornecedor = fornecedor;
                x.categoria = "Descontos obtidos".into();
                l.lancs.push(x);
            }
        }
    }

    // Juros recebidos e descontos concedidos no crediário.
    {
        let mut c = conn.prepare(
            "SELECT r.id, r.numero, r.caixa_nome, date(r.criado_em, 'localtime'), r.juros, r.desconto,
                    COALESCE(p.nome, '')
               FROM recebimento r LEFT JOIN pessoa p ON p.id = r.cliente_id
              WHERE r.estornado_em IS NULL AND (r.juros <> 0 OR r.desconto <> 0) AND r.criado_em >= ?3
                AND date(r.criado_em, 'localtime') BETWEEN ?1 AND ?2",
        )?;
        let linhas = c.query_map(params![de, ate, desde], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, String>(6)?,
            ))
        })?;
        for x in linhas {
            let (id, numero, caixa, dia, juros, desconto, cliente) = x?;
            let d = format!("Recebimento {numero} · {caixa} · {cliente}");
            if juros != 0 {
                let mut x =
                    lanc("receitas_financeiras", juros, &dia, &format!("Juros recebidos · {d}"), "recebimento", &id);
                x.categoria = "Juros recebidos".into();
                l.lancs.push(x);
            }
            if desconto != 0 {
                let mut x = lanc(
                    "despesas_financeiras",
                    desconto,
                    &dia,
                    &format!("Desconto concedido · {d}"),
                    "recebimento",
                    &id,
                );
                x.categoria = "Descontos concedidos no crediário".into();
                l.lancs.push(x);
            }
        }
    }
    Ok(l)
}

fn lanc(linha: &'static str, valor: i64, dia: &str, descricao: &str, origem: &'static str, id: &str) -> Lanc {
    Lanc {
        linha,
        valor,
        data: dia.to_string(),
        mes: None,
        descricao: descricao.to_string(),
        origem,
        origem_id: id.to_string(),
        categoria: String::new(),
        fornecedor: String::new(),
        sem_competencia: false,
    }
}

// ---------------------------------------------------------------------------
// Apuração.
// ---------------------------------------------------------------------------

/// Os números da DRE (centavos), para as telas e os testes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Numeros {
    pub receita_bruta: i64,
    pub descontos: i64,
    pub devolucoes: i64,
    pub cancelamentos: i64,
    pub tributos: i64,
    pub receita_liquida: i64,
    pub cmv: i64,
    pub lucro_bruto: i64,
    /// Margem bruta em centésimos de ponto (2550 = 25,50%). `None` sem receita líquida.
    pub margem_bruta: Option<i64>,
    pub variaveis: i64,
    pub margem_contribuicao: i64,
    pub margem_contribuicao_pct: Option<i64>,
    pub fixas: i64,
    pub outras_receitas: i64,
    pub outras_despesas: i64,
    pub sem_classificacao: i64,
    pub resultado_operacional: i64,
    pub receitas_financeiras: i64,
    pub despesas_financeiras: i64,
    pub resultado_financeiro: i64,
    pub receitas_nao_operacionais: i64,
    pub despesas_nao_operacionais: i64,
    pub resultado: i64,
    /// Despesas registradas (variáveis, fixas, outras, sem classificação,
    /// financeiras e não operacionais). Tributos são dedução, não entram.
    pub despesas: i64,
    /// Informativos (não entram no resultado).
    pub fora_resultado: i64,
    pub mensais_fora: i64,
    pub perda_defeito: i64,
    pub receita_sem_custo: i64,
}

impl Numeros {
    fn valor(&self, linha: &str) -> i64 {
        match linha {
            "receita_bruta" => self.receita_bruta,
            "descontos" => self.descontos,
            "devolucoes" => self.devolucoes,
            "cancelamentos" => self.cancelamentos,
            "tributos" => self.tributos,
            "receita_liquida" => self.receita_liquida,
            "cmv" => self.cmv,
            "lucro_bruto" => self.lucro_bruto,
            "variaveis" => self.variaveis,
            "margem_contribuicao" => self.margem_contribuicao,
            "fixas" => self.fixas,
            "outras_receitas" => self.outras_receitas,
            "outras_despesas" => self.outras_despesas,
            "sem_classificacao" => self.sem_classificacao,
            "resultado_operacional" => self.resultado_operacional,
            "receitas_financeiras" => self.receitas_financeiras,
            "despesas_financeiras" => self.despesas_financeiras,
            "resultado_financeiro" => self.resultado_financeiro,
            "receitas_nao_operacionais" => self.receitas_nao_operacionais,
            "despesas_nao_operacionais" => self.despesas_nao_operacionais,
            "resultado" => self.resultado,
            _ => 0,
        }
    }
}

/// Uma pendência que deixa o resultado incompleto.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pendencia {
    pub codigo: String,
    pub texto: String,
    pub quantidade: i64,
    pub valor: String,
}

/// Uma linha da DRE para a tela.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinhaDre {
    pub id: String,
    pub nome: String,
    pub valor: String,
    /// Valor com sinal na conta (deduções e despesas negativas).
    pub v: i64,
    /// % da receita líquida ("" se não houver receita líquida).
    pub percentual: String,
    pub subtotal: bool,
    /// Tem lançamentos para abrir.
    pub detalhavel: bool,
}

struct Apurado {
    numeros: Numeros,
    incluidos: Vec<Lanc>,
    sem_custo: Vec<SemCusto>,
    pendencias: Vec<Pendencia>,
    meses_incluidos: usize,
}

fn somar_linha(lancs: &[Lanc], linha: &str) -> i64 {
    numeros::saturar(lancs.iter().filter(|l| l.linha == linha).map(|l| l.valor as i128).sum())
}

/// Centésimos de ponto: parte ÷ total × 100 (com arredondamento).
fn pct(parte: i64, total: i64) -> Option<i64> {
    if total <= 0 {
        return None;
    }
    let (p, t) = (parte as i128 * 10_000, total as i128);
    let r = if p >= 0 { (p + t / 2) / t } else { (p - t / 2) / t };
    Some(numeros::saturar(r))
}

fn dinheiro(v: i64) -> String {
    numeros::escrever_dinheiro(v)
}

fn texto_pct(p: Option<i64>) -> String {
    p.map(|x| format!("{}%", numeros::escrever_percentual(x))).unwrap_or_default()
}

fn apurar(conn: &Connection, lido: Lido, p: &Periodo, hoje: &str, vendedor: bool) -> Resultado<Apurado> {
    let mut incluidos = Vec::new();
    let mut mensais_fora = 0_i64;
    let mut meses = std::collections::BTreeSet::new();
    for l in lido.lancs {
        match &l.mes {
            Some(m) if !mes_entra(p, m, hoje) => {
                if l.linha != "fora" {
                    mensais_fora += l.valor;
                }
            }
            Some(m) => {
                meses.insert(m.clone());
                incluidos.push(l);
            }
            None => incluidos.push(l),
        }
    }
    let s = |linha: &str| somar_linha(&incluidos, linha);
    let mut n = Numeros {
        receita_bruta: s("receita_bruta"),
        descontos: s("descontos"),
        devolucoes: s("devolucoes"),
        cancelamentos: s("cancelamentos"),
        tributos: s("tributos"),
        cmv: s("cmv"),
        variaveis: s("variaveis"),
        fixas: s("fixas"),
        outras_receitas: s("outras_receitas"),
        outras_despesas: s("outras_despesas"),
        sem_classificacao: s("sem_classificacao"),
        receitas_financeiras: s("receitas_financeiras"),
        despesas_financeiras: s("despesas_financeiras"),
        receitas_nao_operacionais: s("receitas_nao_operacionais"),
        despesas_nao_operacionais: s("despesas_nao_operacionais"),
        fora_resultado: s("fora"),
        mensais_fora,
        perda_defeito: lido.perda_defeito,
        receita_sem_custo: lido.sem_custo.iter().map(|x| x.receita).sum(),
        ..Default::default()
    };
    let c = |v: i128| numeros::saturar(v);
    n.receita_liquida = c(n.receita_bruta as i128
        - n.descontos as i128
        - n.devolucoes as i128
        - n.cancelamentos as i128
        - n.tributos as i128);
    n.lucro_bruto = c(n.receita_liquida as i128 - n.cmv as i128);
    n.margem_bruta = pct(n.lucro_bruto, n.receita_liquida);
    n.margem_contribuicao = c(n.lucro_bruto as i128 - n.variaveis as i128);
    n.margem_contribuicao_pct = pct(n.margem_contribuicao, n.receita_liquida);
    n.resultado_operacional = c(n.margem_contribuicao as i128 - n.fixas as i128 + n.outras_receitas as i128
        - n.outras_despesas as i128
        - n.sem_classificacao as i128);
    n.resultado_financeiro = c(n.receitas_financeiras as i128 - n.despesas_financeiras as i128);
    n.resultado =
        c(n.resultado_operacional as i128 + n.resultado_financeiro as i128 + n.receitas_nao_operacionais as i128
            - n.despesas_nao_operacionais as i128);
    n.despesas = c(n.variaveis as i128
        + n.fixas as i128
        + n.outras_despesas as i128
        + n.sem_classificacao.max(0) as i128
        + n.despesas_financeiras as i128
        + n.despesas_nao_operacionais as i128);

    // Pendências.
    let mut pend = Vec::new();
    if !lido.sem_custo.is_empty() {
        let q = lido.sem_custo.len() as i64;
        pend.push(Pendencia {
            codigo: "sem_custo".into(),
            texto: format!(
                "{q} {} sem custo no cadastro (R$ {} de venda): o CMV e o lucro estão incompletos (essas peças não entram como lucro).",
                if q == 1 { "peça vendida" } else { "peças vendidas" },
                dinheiro(n.receita_sem_custo)
            ),
            quantidade: q,
            valor: dinheiro(n.receita_sem_custo),
        });
    }
    let sem_class: Vec<&Lanc> = incluidos.iter().filter(|l| l.linha == "sem_classificacao").collect();
    if !sem_class.is_empty() {
        let v: i64 = sem_class.iter().map(|l| l.valor).sum();
        pend.push(Pendencia {
            codigo: "sem_classificacao".into(),
            texto: format!(
                "{} {} sem categoria ou com categoria sem classificação (R$ {}): contados como despesa até serem classificados.",
                sem_class.len(),
                if sem_class.len() == 1 { "lançamento" } else { "lançamentos" },
                dinheiro(v)
            ),
            quantidade: sem_class.len() as i64,
            valor: dinheiro(v),
        });
    }
    let sem_comp: Vec<&Lanc> = incluidos.iter().filter(|l| l.sem_competencia && l.linha != "fora").collect();
    if !sem_comp.is_empty() {
        let v: i64 = sem_comp.iter().map(|l| l.valor.abs()).sum();
        pend.push(Pendencia {
            codigo: "sem_competencia".into(),
            texto: format!(
                "{} {} sem competência (R$ {}): foi usado o mês do vencimento. Informe o mês a que cada conta se refere.",
                sem_comp.len(),
                if sem_comp.len() == 1 { "conta" } else { "contas" },
                dinheiro(v)
            ),
            quantidade: sem_comp.len() as i64,
            valor: dinheiro(v),
        });
    }
    if vendedor {
        pend.push(Pendencia {
            codigo: "vendedor".into(),
            texto: "Filtro por vendedor: despesas, tributos e financeiro não se dividem por vendedor e ficam de fora."
                .into(),
            quantidade: 0,
            valor: String::new(),
        });
    } else {
        if mensais_fora != 0 {
            pend.push(Pendencia {
                codigo: "mensais_fora".into(),
                texto: format!(
                    "Contas mensais (R$ {}) não entram neste período: elas valem para o mês inteiro. Veja o mês para o resultado com as despesas.",
                    dinheiro(mensais_fora)
                ),
                quantidade: 0,
                valor: dinheiro(mensais_fora),
            });
        }
        if config::ler(conn, "dre_classificacao_confirmada")?.as_deref() != Some("1") {
            pend.push(Pendencia {
                codigo: "classificacao".into(),
                texto: "A classificação das categorias ainda não foi confirmada. Confira com o contador e confirme em \"Classificar categorias\".".into(),
                quantidade: 0,
                valor: String::new(),
            });
        }
        if !meses.is_empty() && n.receita_bruta > 0 && n.tributos == 0 {
            pend.push(Pendencia {
                codigo: "sem_tributos".into(),
                texto: "Nenhum tributo sobre as vendas lançado no período (ex.: o DAS do mês). O resultado não desconta impostos que não foram lançados.".into(),
                quantidade: 0,
                valor: String::new(),
            });
        }
    }
    Ok(Apurado { numeros: n, incluidos, sem_custo: lido.sem_custo, pendencias: pend, meses_incluidos: meses.len() })
}

// ---------------------------------------------------------------------------
// Indicadores comerciais.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comercial {
    pub vendas_liquidas: String,
    pub quantidade: i64,
    pub ticket: String,
    pub itens_por_venda: String,
    pub descontos: String,
    pub canceladas: i64,
    pub canceladas_valor: String,
    pub devolucoes: i64,
    pub devolucoes_valor: String,
    pub cartao: String,
    #[serde(skip)]
    pub v_vendas: i64,
    #[serde(skip)]
    pub v_cartao: i64,
}

fn comercial(conn: &Connection, de: &str, ate: &str, vendedor: Option<&str>) -> Resultado<Comercial> {
    let desde = utc_desde(de);
    let vend = vendedor.unwrap_or("");
    let filtro = "v.tipo = 'venda' AND v.criado_em >= ?3 AND date(v.criado_em, 'localtime') BETWEEN ?1 AND ?2
                  AND (?4 = '' OR v.vendedor_id = ?4)";
    let (q, total, desconto): (i64, i64, i64) = conn.query_row(
        &format!(
            "SELECT COUNT(*), COALESCE(SUM(v.total), 0), COALESCE(SUM(v.desconto), 0) FROM venda v
              WHERE {filtro} AND v.situacao = 'concluida'"
        ),
        params![de, ate, desde, vend],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let unidades: i64 = conn.query_row(
        &format!(
            "SELECT COALESCE(SUM(vi.quantidade), 0) FROM venda_item vi JOIN venda v ON v.id = vi.venda_id
               LEFT JOIN produto p ON p.id = vi.produto_id
              WHERE {filtro} AND v.situacao = 'concluida' AND vi.produto_id IS NOT NULL
                AND COALESCE(p.tipo, 'produto') <> 'servico'"
        ),
        params![de, ate, desde, vend],
        |r| r.get(0),
    )?;
    let (canc, canc_valor): (i64, i64) = conn.query_row(
        &format!("SELECT COUNT(*), COALESCE(SUM(v.total), 0) FROM venda v WHERE {filtro} AND v.situacao = 'cancelada'"),
        params![de, ate, desde, vend],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let (dev, dev_valor): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(d.valor), 0) FROM devolucao d JOIN venda v ON v.id = d.venda_id
          WHERE d.cancelado_em IS NULL AND d.criado_em >= ?3 AND date(d.criado_em, 'localtime') BETWEEN ?1 AND ?2
            AND (?4 = '' OR v.vendedor_id = ?4)",
        params![de, ate, desde, vend],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let cartao: i64 = conn.query_row(
        &format!(
            "SELECT COALESCE(SUM(p.valor), 0) FROM venda_pagamento p JOIN venda v ON v.id = p.venda_id
              WHERE {filtro} AND v.situacao = 'concluida' AND p.forma IN ('debito', 'credito')"
        ),
        params![de, ate, desde, vend],
        |r| r.get(0),
    )?;
    Ok(Comercial {
        vendas_liquidas: dinheiro(total),
        quantidade: q,
        ticket: if q > 0 { dinheiro((total + q / 2) / q) } else { String::new() },
        itens_por_venda: if q > 0 {
            numeros::escrever_quantidade(((unidades / q + 50) / 100) * 100)
        } else {
            String::new()
        },
        descontos: dinheiro(desconto),
        canceladas: canc,
        canceladas_valor: dinheiro(canc_valor),
        devolucoes: dev,
        devolucoes_valor: dinheiro(dev_valor),
        cartao: dinheiro(cartao),
        v_vendas: total,
        v_cartao: cartao,
    })
}

// ---------------------------------------------------------------------------
// A DRE pronta.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Filtro {
    /// hoje, 7dias, mes (padrão), mes_anterior, ano, personalizado
    pub periodo: String,
    pub de: String,
    pub ate: String,
    pub vendedor_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Variacao {
    pub nome: String,
    pub atual: String,
    pub anterior: String,
    /// "+12%", "−3 p.p." (vazio se não houver base).
    pub variacao: String,
    pub subiu: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dre {
    pub periodo: Periodo,
    pub comparacao: Periodo,
    pub base: String,
    pub vendedor: String,
    pub linhas: Vec<LinhaDre>,
    pub numeros: Numeros,
    pub comparado: Numeros,
    pub variacoes: Vec<Variacao>,
    /// O resultado só se compara quando os dois períodos tratam as contas mensais igual.
    pub resultado_comparavel: bool,
    /// "completo", "incompleto" ou "sem_dados".
    pub estado: String,
    pub estado_texto: String,
    pub pendencias: Vec<Pendencia>,
    pub avisos: Vec<String>,
    pub comercial: Comercial,
    pub comercial_anterior: Comercial,
    pub margem_bruta: String,
    pub margem_contribuicao: String,
}

fn nome_vendedor(conn: &Connection, id: &str) -> Resultado<String> {
    Ok(conn.query_row("SELECT nome FROM usuario WHERE id = ?1", [id], |r| r.get(0)).optional()?.unwrap_or_default())
}

fn variacao_valor(nome: &str, atual: i64, anterior: i64) -> Variacao {
    let (variacao, subiu) = if anterior > 0 {
        let p = numeros::saturar((atual as i128 - anterior as i128) * 100 / anterior as i128);
        (
            if p > 0 {
                format!("+{p}%")
            } else if p == 0 {
                "igual".into()
            } else {
                format!("{p}%")
            },
            p >= 0,
        )
    } else {
        (String::new(), true)
    };
    Variacao { nome: nome.into(), atual: dinheiro(atual), anterior: dinheiro(anterior), variacao, subiu }
}

fn variacao_pontos(nome: &str, atual: Option<i64>, anterior: Option<i64>) -> Variacao {
    let (variacao, subiu) = match (atual, anterior) {
        (Some(a), Some(b)) => {
            let d = a - b;
            let t = numeros::escrever_percentual(d.abs());
            (
                if d > 0 {
                    format!("+{t} p.p.")
                } else if d == 0 {
                    "igual".into()
                } else {
                    format!("−{t} p.p.")
                },
                d >= 0,
            )
        }
        _ => (String::new(), true),
    };
    Variacao { nome: nome.into(), atual: texto_pct(atual), anterior: texto_pct(anterior), variacao, subiu }
}

fn variacao_numero(nome: &str, atual: i64, anterior: i64) -> Variacao {
    let (variacao, subiu) = if anterior > 0 {
        let p = (atual - anterior) * 100 / anterior;
        (
            if p > 0 {
                format!("+{p}%")
            } else if p == 0 {
                "igual".into()
            } else {
                format!("{p}%")
            },
            p >= 0,
        )
    } else {
        (String::new(), true)
    };
    Variacao { nome: nome.into(), atual: atual.to_string(), anterior: anterior.to_string(), variacao, subiu }
}

fn tem_dados(a: &Apurado, com: &Comercial) -> bool {
    !a.incluidos.is_empty() || !a.sem_custo.is_empty() || com.quantidade > 0 || com.canceladas > 0
}

fn montar_linhas(n: &Numeros, incluidos: &[Lanc], vendedor: bool) -> Vec<LinhaDre> {
    let subtotais = [
        "receita_liquida",
        "lucro_bruto",
        "margem_contribuicao",
        "resultado_operacional",
        "resultado_financeiro",
        "resultado",
    ];
    let positivas = ["receita_bruta", "outras_receitas", "receitas_financeiras", "receitas_nao_operacionais"];
    let ate_lucro =
        ["receita_bruta", "descontos", "devolucoes", "cancelamentos", "receita_liquida", "cmv", "lucro_bruto"];
    NOMES
        .iter()
        .filter(|(id, _)| !vendedor || ate_lucro.contains(id))
        .filter(|(id, _)| {
            // Linhas vazias opcionais somem (as principais ficam sempre).
            let opcional = matches!(
                *id,
                "cancelamentos"
                    | "outras_receitas"
                    | "outras_despesas"
                    | "sem_classificacao"
                    | "receitas_nao_operacionais"
                    | "despesas_nao_operacionais"
            );
            !opcional || n.valor(id) != 0 || incluidos.iter().any(|l| l.linha == *id)
        })
        .map(|(id, nome)| {
            let bruto = n.valor(id);
            let subtotal = subtotais.contains(id);
            let v = if subtotal || positivas.contains(id) { bruto } else { -bruto };
            LinhaDre {
                id: id.to_string(),
                nome: nome.to_string(),
                valor: dinheiro(v),
                v,
                percentual: if *id == "receita_bruta" { String::new() } else { texto_pct(pct(v, n.receita_liquida)) },
                subtotal,
                detalhavel: !subtotal && incluidos.iter().any(|l| l.linha == *id),
            }
        })
        .collect()
}

/// A DRE do período, com a comparação e as pendências.
pub fn apurar_dre(conn: &Connection, f: &Filtro) -> Resultado<Dre> {
    let hoje = fiscal::hoje(conn)?;
    let (p, cp) = periodos(&hoje, &f.periodo, &f.de, &f.ate)?;
    let vend = if f.vendedor_id.is_empty() { None } else { Some(f.vendedor_id.as_str()) };
    let a = apurar(conn, ler(conn, &p.de, &p.ate, vend)?, &p, &hoje, vend.is_some())?;
    let b = apurar(conn, ler(conn, &cp.de, &cp.ate, vend)?, &cp, &hoje, vend.is_some())?;
    let com = comercial(conn, &p.de, &p.ate, vend)?;
    let com_b = comercial(conn, &cp.de, &cp.ate, vend)?;
    let n = &a.numeros;
    let linhas = montar_linhas(n, &a.incluidos, vend.is_some());

    let mut variacoes = vec![
        variacao_valor("Vendas líquidas", com.v_vendas, com_b.v_vendas),
        variacao_numero("Quantidade de vendas", com.quantidade, com_b.quantidade),
        variacao_valor(
            "Ticket médio",
            if com.quantidade > 0 { com.v_vendas / com.quantidade } else { 0 },
            if com_b.quantidade > 0 { com_b.v_vendas / com_b.quantidade } else { 0 },
        ),
        variacao_valor("Receita líquida", n.receita_liquida, b.numeros.receita_liquida),
        variacao_valor("Lucro bruto", n.lucro_bruto, b.numeros.lucro_bruto),
        variacao_pontos("Margem bruta", n.margem_bruta, b.numeros.margem_bruta),
    ];
    let comparavel = vend.is_none()
        && a.numeros.mensais_fora == 0
        && b.numeros.mensais_fora == 0
        && a.meses_incluidos == b.meses_incluidos
        && p.mensais;
    if comparavel {
        variacoes.push(variacao_valor("Despesas registradas", n.despesas, b.numeros.despesas));
        let mut r = variacao_valor("Resultado estimado", n.resultado, b.numeros.resultado);
        if b.numeros.resultado <= 0 {
            r.variacao = String::new();
        }
        variacoes.push(r);
    }

    let mut avisos = Vec::new();
    if p.parcial || cp.parcial {
        avisos.push(format!(
            "Comparação entre períodos {}: \"{}\" × \"{}\".",
            if p.parcial && cp.parcial { "parciais" } else { "de tamanhos diferentes de conclusão" },
            p.rotulo,
            cp.rotulo
        ));
    }
    if com.v_cartao > 0 && vend.is_none() {
        avisos.push(format!(
            "R$ {} vendidos no cartão: as taxas da maquininha não são registradas sozinhas. Lance-as como despesa variável para entrarem no resultado.",
            com.cartao
        ));
    }
    if n.perda_defeito != 0 {
        avisos.push(format!(
            "Peças devolvidas com defeito (não voltaram ao estoque): o custo delas (R$ {}) continua no CMV.",
            dinheiro(n.perda_defeito)
        ));
    }
    if n.fora_resultado != 0 {
        avisos.push(format!(
            "R$ {} em lançamentos fora do resultado (compra de mercadoria, empréstimos, aportes...). Não entram na DRE.",
            dinheiro(n.fora_resultado.abs())
        ));
    }
    avisos.push(
        "Base: competência gerencial. Venda a prazo entra uma vez, na data da venda; receber a parcela não é receita nova. Compras de mercadoria entram pelo CMV quando a peça é vendida."
            .into(),
    );

    let tem = tem_dados(&a, &com);
    let (estado, estado_texto) = if !tem {
        ("sem_dados", "Sem lançamentos no período: não é um resultado zero comprovado.".to_string())
    } else if a.pendencias.is_empty() {
        ("completo", "Completo com os dados registrados (estimado; não é a DRE contábil).".to_string())
    } else {
        ("incompleto", format!("Incompleto: {} pendência(s). Resultado estimado.", a.pendencias.len()))
    };
    Ok(Dre {
        base: "Competência gerencial · valores registrados no Rustinho".into(),
        vendedor: match vend {
            Some(v) => nome_vendedor(conn, v)?,
            None => String::new(),
        },
        margem_bruta: texto_pct(n.margem_bruta),
        margem_contribuicao: texto_pct(n.margem_contribuicao_pct),
        linhas,
        numeros: a.numeros.clone(),
        comparado: b.numeros.clone(),
        variacoes,
        resultado_comparavel: comparavel,
        estado: estado.into(),
        estado_texto,
        pendencias: a.pendencias,
        avisos,
        comercial: com,
        comercial_anterior: com_b,
        periodo: p,
        comparacao: cp,
    })
}

// ---------------------------------------------------------------------------
// Detalhe de uma linha (rastreabilidade).
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LancamentoTela {
    pub data: String,
    pub descricao: String,
    pub categoria: String,
    pub fornecedor: String,
    pub valor: String,
    pub origem: String,
    pub origem_id: String,
    pub sem_competencia: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detalhe {
    pub linha: String,
    pub nome: String,
    pub total: String,
    pub quantidade: i64,
    pub itens: Vec<LancamentoTela>,
    /// Mais lançamentos do que a lista mostra.
    pub cortado: bool,
}

const LIMITE_DETALHE: usize = 500;

fn tela_lanc(l: &Lanc) -> LancamentoTela {
    LancamentoTela {
        data: match &l.mes {
            Some(m) => format!("competência {}/{}", &m[5..7.min(m.len())], &m[..4.min(m.len())]),
            None => fiscal::data_br(&l.data),
        },
        descricao: l.descricao.clone(),
        categoria: l.categoria.clone(),
        fornecedor: l.fornecedor.clone(),
        valor: dinheiro(l.valor),
        origem: l.origem.into(),
        origem_id: l.origem_id.clone(),
        sem_competencia: l.sem_competencia,
    }
}

/// Os lançamentos de uma linha (ou "sem_custo": as peças vendidas sem custo).
pub fn detalhe(conn: &Connection, f: &Filtro, linha: &str) -> Resultado<Detalhe> {
    let hoje = fiscal::hoje(conn)?;
    let (p, _) = periodos(&hoje, &f.periodo, &f.de, &f.ate)?;
    let vend = if f.vendedor_id.is_empty() { None } else { Some(f.vendedor_id.as_str()) };
    let a = apurar(conn, ler(conn, &p.de, &p.ate, vend)?, &p, &hoje, vend.is_some())?;
    if linha == "sem_custo" {
        let mut v = a.sem_custo;
        v.sort_by(|x, y| x.data.cmp(&y.data));
        let total: i64 = v.iter().map(|x| x.receita).sum();
        return Ok(Detalhe {
            linha: linha.into(),
            nome: "Peças vendidas sem custo (receita)".into(),
            total: dinheiro(total),
            quantidade: v.len() as i64,
            cortado: v.len() > LIMITE_DETALHE,
            itens: v
                .iter()
                .take(LIMITE_DETALHE)
                .map(|x| LancamentoTela {
                    data: fiscal::data_br(&x.data),
                    descricao: x.descricao.clone(),
                    categoria: String::new(),
                    fornecedor: String::new(),
                    valor: dinheiro(x.receita),
                    origem: "venda".into(),
                    origem_id: x.venda_id.clone(),
                    sem_competencia: false,
                })
                .collect(),
        });
    }
    if nome_linha(linha).is_empty() && linha != "fora" {
        return Err(Erro::validacao("Linha da DRE desconhecida."));
    }
    let mut v: Vec<&Lanc> = a.incluidos.iter().filter(|l| l.linha == linha).collect();
    v.sort_by(|x, y| x.data.cmp(&y.data).then_with(|| x.descricao.cmp(&y.descricao)));
    let total = somar_linha(&a.incluidos, linha);
    Ok(Detalhe {
        linha: linha.into(),
        nome: if linha == "fora" { "Fora do resultado".into() } else { nome_linha(linha).into() },
        total: dinheiro(total),
        quantidade: v.len() as i64,
        cortado: v.len() > LIMITE_DETALHE,
        itens: v.into_iter().take(LIMITE_DETALHE).map(tela_lanc).collect(),
    })
}

// ---------------------------------------------------------------------------
// Despesas por categoria.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DespesaCategoria {
    pub categoria: String,
    pub grupo: String,
    pub valor: String,
    pub quantidade: i64,
    pub parte: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Despesas {
    pub total: String,
    pub categorias: Vec<DespesaCategoria>,
    pub lancamentos: Vec<LancamentoTela>,
}

const LINHAS_DESPESA: &[&str] = &[
    "tributos",
    "variaveis",
    "fixas",
    "outras_despesas",
    "sem_classificacao",
    "despesas_financeiras",
    "despesas_nao_operacionais",
];

/// Despesas (e tributos) do período por categoria, com os lançamentos.
/// `categoria` vazia = todas.
pub fn despesas(conn: &Connection, f: &Filtro, categoria: &str) -> Resultado<Despesas> {
    let hoje = fiscal::hoje(conn)?;
    let (p, _) = periodos(&hoje, &f.periodo, &f.de, &f.ate)?;
    let a = apurar(conn, ler(conn, &p.de, &p.ate, None)?, &p, &hoje, false)?;
    let lista: Vec<&Lanc> = a.incluidos.iter().filter(|l| LINHAS_DESPESA.contains(&l.linha)).collect();
    let mut por: BTreeMap<(String, String), (i64, i64)> = BTreeMap::new();
    for l in &lista {
        let nome = if l.categoria.is_empty() { "Sem categoria".to_string() } else { l.categoria.clone() };
        let e = por.entry((nome, nome_linha(l.linha).trim_start_matches("(−) ").to_string())).or_insert((0, 0));
        e.0 += l.valor;
        e.1 += 1;
    }
    let maior = por.values().map(|x| x.0).max().unwrap_or(0);
    let mut categorias: Vec<DespesaCategoria> = por
        .into_iter()
        .map(|((categoria, grupo), (v, q))| DespesaCategoria {
            categoria,
            grupo,
            valor: dinheiro(v),
            quantidade: q,
            parte: if maior > 0 && v > 0 { (v as f64 / maior as f64).clamp(0.0, 1.0) } else { 0.0 },
        })
        .collect();
    categorias.sort_by(|x, y| y.parte.partial_cmp(&x.parte).unwrap_or(std::cmp::Ordering::Equal));
    let total: i64 = lista.iter().map(|l| l.valor).sum();
    let mut filtrados: Vec<&Lanc> = lista
        .into_iter()
        .filter(|l| {
            categoria.is_empty() || l.categoria == categoria || (categoria == "Sem categoria" && l.categoria.is_empty())
        })
        .collect();
    filtrados.sort_by(|x, y| x.data.cmp(&y.data));
    Ok(Despesas {
        total: dinheiro(total),
        categorias,
        lancamentos: filtrados.into_iter().take(LIMITE_DETALHE).map(tela_lanc).collect(),
    })
}

// ---------------------------------------------------------------------------
// Peças e vendedores.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Produto {
    pub nome: String,
    pub quantidade: String,
    pub receita: String,
    pub lucro: String,
    pub margem: String,
    pub sem_custo: bool,
    #[serde(skip)]
    v_q: i64,
    #[serde(skip)]
    v_receita: i64,
    #[serde(skip)]
    v_lucro: Option<i64>,
    #[serde(skip)]
    v_margem: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vendedor {
    pub id: String,
    pub nome: String,
    pub vendas: i64,
    pub receita: String,
    pub ticket: String,
    pub descontos: String,
    pub lucro: String,
    pub margem: String,
    pub sem_custo: bool,
    pub devolucoes: i64,
    pub devolucoes_valor: String,
    pub canceladas: i64,
    pub canceladas_valor: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Indicadores {
    pub periodo: Periodo,
    pub por_quantidade: Vec<Produto>,
    pub por_receita: Vec<Produto>,
    pub por_lucro: Vec<Produto>,
    pub por_margem: Vec<Produto>,
    pub vendedores: Vec<Vendedor>,
    pub criterio_vendedor: String,
    pub nota: String,
}

#[derive(Default)]
struct Acumulado {
    nome: String,
    q: i64,
    receita: i64,
    cmv: i64,
    sem_custo: bool,
}

/// Rateio do desconto geral da venda pelo total de cada item (o que sobra do
/// arredondamento fica no último item).
fn ratear(desconto: i64, totais: &[i64]) -> Vec<i64> {
    let soma: i128 = totais.iter().map(|t| *t as i128).sum();
    if desconto == 0 || soma <= 0 {
        return vec![0; totais.len()];
    }
    let mut partes: Vec<i64> = totais.iter().map(|t| numeros::saturar(desconto as i128 * *t as i128 / soma)).collect();
    let resto = desconto - partes.iter().sum::<i64>();
    if let Some(u) = partes.last_mut() {
        *u += resto;
    }
    partes
}

/// Rankings de peças e o desempenho dos vendedores.
pub fn indicadores(conn: &Connection, f: &Filtro, busca: &str, limite: usize) -> Resultado<Indicadores> {
    let hoje = fiscal::hoje(conn)?;
    let (p, _) = periodos(&hoje, &f.periodo, &f.de, &f.ate)?;
    let desde = utc_desde(&p.de);
    let vend = f.vendedor_id.as_str();

    // Itens das vendas concluídas (não canceladas) do período.
    struct It {
        venda: String,
        vendedor: String,
        produto: String,
        nome: String,
        q: i64,
        total: i64,
        custo: i64,
        peca: bool,
    }
    let mut itens: Vec<It> = Vec::new();
    let mut desconto_geral: HashMap<String, i64> = HashMap::new();
    {
        let mut c = conn.prepare(
            "SELECT v.id, COALESCE(v.vendedor_id, ''), COALESCE(vi.produto_id, ''),
                    COALESCE(p.nome, vi.descricao) || CASE WHEN COALESCE(p.marca, '') <> '' THEN ' · ' || p.marca ELSE '' END,
                    vi.quantidade, vi.total, vi.custo,
                    vi.produto_id IS NOT NULL AND COALESCE(p.tipo, 'produto') <> 'servico',
                    v.desconto - (SELECT COALESCE(SUM(x.desconto), 0) FROM venda_item x WHERE x.venda_id = v.id)
               FROM venda_item vi JOIN venda v ON v.id = vi.venda_id LEFT JOIN produto p ON p.id = vi.produto_id
              WHERE v.tipo = 'venda' AND v.situacao = 'concluida' AND v.criado_em >= ?3
                AND date(v.criado_em, 'localtime') BETWEEN ?1 AND ?2 AND (?4 = '' OR v.vendedor_id = ?4)
              ORDER BY v.id, vi.ordem",
        )?;
        let linhas = c.query_map(params![p.de, p.ate, desde, vend], |r| {
            Ok((
                It {
                    venda: r.get(0)?,
                    vendedor: r.get(1)?,
                    produto: r.get(2)?,
                    nome: r.get(3)?,
                    q: r.get(4)?,
                    total: r.get(5)?,
                    custo: r.get(6)?,
                    peca: r.get(7)?,
                },
                r.get::<_, i64>(8)?,
            ))
        })?;
        for x in linhas {
            let (it, dg) = x?;
            desconto_geral.insert(it.venda.clone(), dg);
            itens.push(it);
        }
    }
    // Receita líquida de cada item: total − parte do desconto geral.
    let mut liquido: Vec<i64> = vec![0; itens.len()];
    {
        let mut i = 0;
        while i < itens.len() {
            let mut j = i;
            while j < itens.len() && itens[j].venda == itens[i].venda {
                j += 1;
            }
            let totais: Vec<i64> = itens[i..j].iter().map(|x| x.total).collect();
            let partes = ratear(desconto_geral.get(&itens[i].venda).copied().unwrap_or(0), &totais);
            for (k, parte) in partes.into_iter().enumerate() {
                liquido[i + k] = itens[i + k].total - parte;
            }
            i = j;
        }
    }

    let mut produtos: HashMap<String, Acumulado> = HashMap::new();
    struct V {
        vendas: std::collections::HashSet<String>,
        receita: i64,
        cmv: i64,
        sem_custo: bool,
    }
    let mut vendedores: HashMap<String, V> = HashMap::new();
    for (it, liq) in itens.iter().zip(&liquido) {
        let cmv = if it.peca && it.custo > 0 { custo_total(it.custo, it.q) } else { 0 };
        let sem = it.peca && it.custo <= 0;
        if it.peca {
            let e = produtos
                .entry(it.produto.clone())
                .or_insert_with(|| Acumulado { nome: it.nome.clone(), ..Default::default() });
            e.q += it.q;
            e.receita += liq;
            e.cmv += cmv;
            e.sem_custo |= sem;
        }
        let v = vendedores.entry(it.vendedor.clone()).or_insert(V {
            vendas: Default::default(),
            receita: 0,
            cmv: 0,
            sem_custo: false,
        });
        v.vendas.insert(it.venda.clone());
        v.receita += liq;
        v.cmv += cmv;
        v.sem_custo |= sem;
    }
    // Devoluções do período: tiram quantidade, receita e (se voltou ao estoque) custo.
    {
        let mut c = conn.prepare(
            "SELECT COALESCE(di.produto_id, ''), di.quantidade, di.valor, di.custo, di.volta_estoque
               FROM devolucao_item di JOIN devolucao d ON d.id = di.devolucao_id JOIN venda v ON v.id = d.venda_id
              WHERE d.cancelado_em IS NULL AND d.criado_em >= ?3 AND date(d.criado_em, 'localtime') BETWEEN ?1 AND ?2
                AND (?4 = '' OR v.vendedor_id = ?4) AND di.produto_id IS NOT NULL",
        )?;
        let linhas = c.query_map(params![p.de, p.ate, desde, vend], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, bool>(4)?,
            ))
        })?;
        for x in linhas {
            let (pid, q, valor, custo, volta) = x?;
            if let Some(e) = produtos.get_mut(&pid) {
                e.q -= q;
                e.receita -= valor;
                if volta && custo > 0 {
                    e.cmv -= custo_total(custo, q);
                }
            }
        }
    }

    let busca_n = crate::texto::normalizar(busca);
    let mut lista: Vec<Produto> = produtos
        .into_values()
        .filter(|a| busca_n.is_empty() || crate::texto::normalizar(&a.nome).contains(&busca_n))
        .map(|a| {
            let lucro = if a.sem_custo { None } else { Some(a.receita - a.cmv) };
            let margem = lucro.and_then(|l| pct(l, a.receita));
            Produto {
                nome: a.nome,
                quantidade: numeros::escrever_quantidade(a.q),
                receita: dinheiro(a.receita),
                lucro: lucro.map(dinheiro).unwrap_or_else(|| "sem custo".into()),
                margem: texto_pct(margem),
                sem_custo: a.sem_custo,
                v_q: a.q,
                v_receita: a.receita,
                v_lucro: lucro,
                v_margem: margem,
            }
        })
        .collect();
    lista.sort_by(|a, b| a.nome.cmp(&b.nome));
    let top = |mut v: Vec<Produto>, chave: &dyn Fn(&Produto) -> Option<i64>| -> Vec<Produto> {
        v.retain(|x| chave(x).is_some());
        v.sort_by_key(|x| std::cmp::Reverse(chave(x).unwrap_or(i64::MIN)));
        v.truncate(limite);
        v
    };
    let por_quantidade = top(lista.clone(), &|x| (x.v_q > 0).then_some(x.v_q));
    let por_receita = top(lista.clone(), &|x| (x.v_receita > 0).then_some(x.v_receita));
    let por_lucro = top(lista.clone(), &|x| x.v_lucro);
    let por_margem = top(lista, &|x| if x.v_receita > 0 { x.v_margem } else { None });

    // Vendedores: descontos, devoluções e cancelamentos à parte.
    let mut saida = Vec::new();
    {
        let mut c = conn.prepare(
            "SELECT COALESCE(v.vendedor_id, ''), COUNT(*), COALESCE(SUM(v.total), 0), COALESCE(SUM(v.desconto), 0)
               FROM venda v
              WHERE v.tipo = 'venda' AND v.situacao = 'concluida' AND v.criado_em >= ?3
                AND date(v.criado_em, 'localtime') BETWEEN ?1 AND ?2 AND (?4 = '' OR v.vendedor_id = ?4)
              GROUP BY 1",
        )?;
        let base: Vec<(String, i64, i64, i64)> = c
            .query_map(params![p.de, p.ate, desde, vend], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<Result<_, _>>()?;
        for (id, q, total, descontos) in base {
            let (dq, dv): (i64, i64) = conn.query_row(
                "SELECT COUNT(*), COALESCE(SUM(d.valor), 0) FROM devolucao d JOIN venda v ON v.id = d.venda_id
                  WHERE d.cancelado_em IS NULL AND d.criado_em >= ?3 AND date(d.criado_em, 'localtime') BETWEEN ?1 AND ?2
                    AND COALESCE(v.vendedor_id, '') = ?4",
                params![p.de, p.ate, desde, id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let (cq, cv): (i64, i64) = conn.query_row(
                "SELECT COUNT(*), COALESCE(SUM(v.total), 0) FROM venda v
                  WHERE v.tipo = 'venda' AND v.situacao = 'cancelada' AND v.criado_em >= ?3
                    AND date(v.criado_em, 'localtime') BETWEEN ?1 AND ?2 AND COALESCE(v.vendedor_id, '') = ?4",
                params![p.de, p.ate, desde, id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let acc = vendedores.get(&id);
            let receita = acc.map(|a| a.receita).unwrap_or(total);
            let sem = acc.map(|a| a.sem_custo).unwrap_or(false);
            let lucro = acc.map(|a| a.receita - a.cmv);
            let nome = if id.is_empty() { "Sem vendedor".to_string() } else { nome_vendedor(conn, &id)? };
            saida.push((
                total,
                Vendedor {
                    id: id.clone(),
                    nome,
                    vendas: q,
                    receita: dinheiro(total),
                    ticket: if q > 0 { dinheiro((total + q / 2) / q) } else { String::new() },
                    descontos: dinheiro(descontos),
                    // Com peça sem custo o lucro fica incompleto (`sem_custo`): a tela marca.
                    lucro: lucro.map(dinheiro).unwrap_or_default(),
                    margem: if sem { String::new() } else { texto_pct(lucro.and_then(|l| pct(l, receita))) },
                    sem_custo: sem,
                    devolucoes: dq,
                    devolucoes_valor: dinheiro(dv),
                    canceladas: cq,
                    canceladas_valor: dinheiro(cv),
                },
            ));
        }
    }
    saida.sort_by_key(|x| std::cmp::Reverse(x.0));
    Ok(Indicadores {
        periodo: p,
        por_quantidade,
        por_receita,
        por_lucro,
        por_margem,
        vendedores: saida.into_iter().map(|x| x.1).collect(),
        criterio_vendedor:
            "Cada venda conta para quem a concluiu no caixa (uma venda tem um vendedor só). Lucro do vendedor: receita menos o custo das peças, antes dos tributos. Devoluções e cancelamentos aparecem à parte."
                .into(),
        nota: "Peças: vendas concluídas e não canceladas do período, menos as devoluções do período. O desconto geral da venda é dividido entre os itens pelo valor de cada um. Serviços (mão de obra) não entram nos rankings de peças. Maior venda não é o mesmo que maior lucro."
            .into(),
    })
}

// ---------------------------------------------------------------------------
// Série dos últimos 12 meses.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MesSerie {
    pub rotulo: String,
    pub mes: String,
    pub vendas: String,
    pub lucro_bruto: String,
    pub despesas: String,
    pub resultado: String,
    /// "completo", "incompleto", "sem_dados" ou "antes" (antes do primeiro registro).
    pub estado: String,
    pub parcial: bool,
    pub v_vendas: i64,
    pub v_resultado: i64,
}

pub fn serie(conn: &Connection) -> Resultado<Vec<MesSerie>> {
    const CURTOS: [&str; 12] = ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];
    let hoje = fiscal::hoje(conn)?;
    let h = data(&hoje)?;
    let primeiro: Option<String> = conn.query_row(
        "SELECT MIN(d) FROM (SELECT MIN(date(criado_em, 'localtime')) AS d FROM venda WHERE tipo = 'venda'
                              UNION ALL SELECT MIN(COALESCE(competencia, substr(vencimento, 1, 7)) || '-01') FROM conta_pagar
                              UNION ALL SELECT MIN(data) FROM movimento_conta)",
        [],
        |r| r.get(0),
    )?;
    let mut meses = Vec::new();
    let mut m = primeiro_do_mes(h);
    for _ in 0..12 {
        meses.push(m);
        m = mes_anterior(m);
    }
    meses.reverse();
    let mut saida = Vec::new();
    for m in meses {
        let fim = if m == primeiro_do_mes(h) { h } else { ultimo_do_mes(m) };
        let p = periodo("mes", m, fim, String::new(), fim != ultimo_do_mes(m), true);
        let a = apurar(conn, ler(conn, &p.de, &p.ate, None)?, &p, &hoje, false)?;
        let com = comercial(conn, &p.de, &p.ate, None)?;
        let antes = primeiro.as_deref().map(|x| iso(ultimo_do_mes(m)).as_str() < x).unwrap_or(true);
        let estado = if !tem_dados(&a, &com) {
            if antes {
                "antes"
            } else {
                "sem_dados"
            }
        } else if a.pendencias.iter().any(|x| x.codigo != "classificacao") {
            "incompleto"
        } else {
            "completo"
        };
        let vazio = matches!(estado, "antes" | "sem_dados");
        let n = &a.numeros;
        saida.push(MesSerie {
            rotulo: format!("{}/{:02}", CURTOS[m.month() as usize - 1], m.year() % 100),
            mes: format!("{:04}-{:02}", m.year(), m.month() as u8),
            vendas: if vazio { String::new() } else { dinheiro(com.v_vendas) },
            lucro_bruto: if vazio { String::new() } else { dinheiro(n.lucro_bruto) },
            despesas: if vazio { String::new() } else { dinheiro(n.despesas) },
            resultado: if vazio { String::new() } else { dinheiro(n.resultado) },
            estado: estado.into(),
            parcial: p.parcial,
            v_vendas: com.v_vendas,
            v_resultado: n.resultado,
        });
    }
    Ok(saida)
}

// ---------------------------------------------------------------------------
// Classificação das categorias.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoriaDre {
    pub id: String,
    pub nome: String,
    pub tipo: String,
    pub grupo: String,
    pub grupo_nome: String,
    pub ativo: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrupoTela {
    pub id: String,
    pub nome: String,
    pub descricao: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Classificacao {
    pub categorias: Vec<CategoriaDre>,
    pub grupos: Vec<GrupoTela>,
    pub confirmada: bool,
    pub nota: String,
}

pub fn classificacao(conn: &Connection) -> Resultado<Classificacao> {
    let mut c = conn.prepare(
        "SELECT id, nome, tipo, COALESCE(dre_grupo, ''), ativo FROM categoria ORDER BY ativo DESC, tipo, ordem, nome",
    )?;
    let categorias = c
        .query_map([], |r| {
            let grupo: String = r.get(3)?;
            Ok(CategoriaDre {
                id: r.get(0)?,
                nome: r.get(1)?,
                tipo: r.get(2)?,
                grupo_nome: nome_grupo(&grupo).into(),
                grupo,
                ativo: r.get(4)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(Classificacao {
        categorias,
        grupos: GRUPOS
            .iter()
            .map(|(id, nome, descricao)| GrupoTela {
                id: (*id).into(),
                nome: (*nome).into(),
                descricao: (*descricao).into(),
            })
            .collect(),
        confirmada: config::ler(conn, "dre_classificacao_confirmada")?.as_deref() == Some("1"),
        nota: "A classificação de fábrica é só uma sugestão do Rustinho. Confira com o contador antes de confirmar."
            .into(),
    })
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ClassificacaoEntrada {
    /// [{"id": "cat-aluguel", "grupo": "fixa"}, ...]
    pub categorias: Vec<CategoriaGrupo>,
    pub confirmar: bool,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CategoriaGrupo {
    pub id: String,
    pub grupo: String,
}

/// Grava a classificação (com auditoria do que mudou) e, se pedido, a confirma.
pub fn salvar_classificacao(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    e: &ClassificacaoEntrada,
) -> Resultado<()> {
    let mut mudou = Vec::new();
    for c in &e.categorias {
        if !c.grupo.is_empty() && !GRUPOS.iter().any(|g| g.0 == c.grupo) {
            return Err(Erro::validacao(format!("Grupo da DRE desconhecido: {}", c.grupo)));
        }
        let atual: Option<(String, Option<String>)> = conn
            .query_row("SELECT nome, dre_grupo FROM categoria WHERE id = ?1", [&c.id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        let Some((nome, antes)) = atual else {
            return Err(Erro::NaoEncontrado("Categoria".into()));
        };
        let novo = if c.grupo.is_empty() { None } else { Some(c.grupo.clone()) };
        if antes != novo {
            conn.execute(
                "UPDATE categoria SET dre_grupo = ?2, atualizado_em = ?3 WHERE id = ?1",
                params![c.id, novo, tempo::agora()],
            )?;
            mudou.push(format!(
                "{nome}: {} → {}",
                nome_grupo(antes.as_deref().unwrap_or("")),
                nome_grupo(novo.as_deref().unwrap_or(""))
            ));
        }
    }
    let ja = config::ler(conn, "dre_classificacao_confirmada")?.as_deref() == Some("1");
    if e.confirmar && !ja {
        config::gravar(conn, "dre_classificacao_confirmada", "1")?;
        mudou.push("Classificação confirmada pelo gerente".into());
    }
    if !mudou.is_empty() {
        auditoria::registrar(conn, dispositivo, Some(usuario_id), "dre.classificacao", "categoria", "", &mudou)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Metas.
// ---------------------------------------------------------------------------

pub const INDICADORES_META: &[(&str, &str)] = &[
    ("vendas", "Vendas líquidas"),
    ("lucro_bruto", "Lucro bruto"),
    ("margem", "Margem bruta"),
    ("quantidade", "Quantidade de vendas"),
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaLinha {
    pub id: String,
    pub periodo: String,
    pub periodo_rotulo: String,
    pub indicador: String,
    pub indicador_nome: String,
    pub vendedor_id: String,
    pub vendedor: String,
    pub meta: String,
    pub realizado: String,
    pub diferenca: String,
    pub percentual: String,
    pub parte: f64,
    pub atingida: bool,
    pub unidade: String,
}

fn id_meta(periodo: &str, indicador: &str, vendedor: &str) -> String {
    format!("meta:{periodo}:{indicador}:{}", if vendedor.is_empty() { "loja" } else { vendedor })
}

fn escrever_meta(indicador: &str, v: i64) -> String {
    match indicador {
        "margem" => format!("{}%", numeros::escrever_percentual(v)),
        "quantidade" => v.to_string(),
        _ => format!("R$ {}", dinheiro(v)),
    }
}

/// Período da meta: mês corrente (até hoje) ou semana corrente (segunda até hoje).
fn periodo_meta(hoje: Date, periodo: &str) -> Periodo {
    if periodo == "semana" {
        let mut d = hoje;
        while d.weekday() != Weekday::Monday {
            d -= Duration::days(1);
        }
        periodo_simples("semana", d, hoje, format!("semana de {}", br(d)))
    } else {
        let de = primeiro_do_mes(hoje);
        periodo_simples("mes", de, hoje, format!("{} de {}", MESES_NOME[hoje.month() as usize - 1], hoje.year()))
    }
}

fn periodo_simples(tipo: &str, de: Date, ate: Date, rotulo: String) -> Periodo {
    periodo(tipo, de, ate, rotulo, true, tipo == "mes")
}

/// Realizado de um indicador num período (vendedor vazio = loja).
fn realizado(conn: &Connection, p: &Periodo, hoje: &str, indicador: &str, vendedor: &str) -> Resultado<Option<i64>> {
    let vend = if vendedor.is_empty() { None } else { Some(vendedor) };
    Ok(match indicador {
        "vendas" => Some(comercial(conn, &p.de, &p.ate, vend)?.v_vendas),
        "quantidade" => Some(comercial(conn, &p.de, &p.ate, vend)?.quantidade),
        "lucro_bruto" => {
            Some(apurar(conn, ler(conn, &p.de, &p.ate, vend)?, p, hoje, vend.is_some())?.numeros.lucro_bruto)
        }
        "margem" => apurar(conn, ler(conn, &p.de, &p.ate, vend)?, p, hoje, vend.is_some())?.numeros.margem_bruta,
        _ => None,
    })
}

pub fn metas(conn: &Connection) -> Resultado<Vec<MetaLinha>> {
    let hoje = fiscal::hoje(conn)?;
    let h = data(&hoje)?;
    let mut c = conn.prepare(
        "SELECT m.id, m.periodo, m.indicador, COALESCE(m.vendedor_id, ''), m.valor, COALESCE(u.nome, '')
           FROM meta m LEFT JOIN usuario u ON u.id = m.vendedor_id
          WHERE m.ativo = 1 ORDER BY m.periodo, m.vendedor_id IS NOT NULL, u.nome, m.indicador",
    )?;
    let lista: Vec<(String, String, String, String, i64, String)> = c
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
        .collect::<Result<_, _>>()?;
    let mut saida = Vec::new();
    for (id, periodo, indicador, vendedor_id, valor, vendedor) in lista {
        let meta_txt = escrever_meta(&indicador, valor);
        let p = periodo_meta(h, &periodo);
        let r = realizado(conn, &p, &hoje, &indicador, &vendedor_id)?;
        let (realizado_txt, diferenca, percentual, parte, atingida) = match r {
            Some(r) => {
                let pc = pct(r, valor);
                (
                    escrever_meta(&indicador, r),
                    escrever_meta(&indicador, r - valor),
                    texto_pct(pc),
                    pc.map(|x| (x as f64 / 10_000.0).clamp(0.0, 1.0)).unwrap_or(0.0),
                    valor > 0 && r >= valor,
                )
            }
            None => ("sem dados".into(), String::new(), String::new(), 0.0, false),
        };
        saida.push(MetaLinha {
            id,
            periodo_rotulo: p.rotulo.clone(),
            periodo,
            indicador_nome: INDICADORES_META.iter().find(|x| x.0 == indicador).map(|x| x.1).unwrap_or("").into(),
            unidade: match indicador.as_str() {
                "margem" => "%".into(),
                "quantidade" => "vendas".into(),
                _ => "R$".into(),
            },
            indicador,
            vendedor: if vendedor_id.is_empty() { "Loja".into() } else { vendedor },
            vendedor_id,
            meta: meta_txt,
            realizado: realizado_txt,
            diferenca,
            percentual,
            parte,
            atingida,
        });
    }
    Ok(saida)
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MetaEntrada {
    /// "mes" (padrão) ou "semana".
    pub periodo: String,
    pub indicador: String,
    pub vendedor_id: String,
    /// Vazio ou zero tira a meta.
    pub valor: String,
}

/// Grava (ou tira) uma meta. Devolve o id.
pub fn salvar_meta(conn: &Connection, dispositivo: &str, usuario_id: &str, e: &MetaEntrada) -> Resultado<String> {
    let periodo = if e.periodo == "semana" { "semana" } else { "mes" };
    if !INDICADORES_META.iter().any(|x| x.0 == e.indicador) {
        return Err(Erro::campo("indicador", "Escolha o indicador da meta."));
    }
    if !e.vendedor_id.is_empty() {
        if !matches!(e.indicador.as_str(), "vendas" | "quantidade" | "lucro_bruto") {
            return Err(Erro::campo(
                "indicador",
                "Por vendedor, a meta pode ser de vendas, quantidade ou lucro bruto.",
            ));
        }
        let existe =
            conn.query_row("SELECT 1 FROM usuario WHERE id = ?1", [&e.vendedor_id], |_| Ok(())).optional()?.is_some();
        if !existe {
            return Err(Erro::NaoEncontrado("Vendedor".into()));
        }
    }
    let valor = match e.indicador.as_str() {
        "margem" => numeros::ler_percentual(&e.valor).map_err(|m| Erro::campo("valor", m))?,
        "quantidade" => {
            let t = e.valor.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.parse::<i64>().map_err(|_| Erro::campo("valor", "Digite um número inteiro de vendas."))?)
            }
        }
        _ => numeros::ler_dinheiro(&e.valor).map_err(|m| Erro::campo("valor", m))?,
    }
    .unwrap_or(0);
    if valor < 0 {
        return Err(Erro::campo("valor", "A meta não pode ser negativa."));
    }
    if e.indicador == "margem" && valor > 10_000 {
        return Err(Erro::campo("valor", "A margem vai de 0 a 100%."));
    }
    let id = id_meta(periodo, &e.indicador, &e.vendedor_id);
    let antes: Option<(i64, bool)> = conn
        .query_row("SELECT valor, ativo FROM meta WHERE id = ?1", [&id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    let ativo = valor > 0;
    conn.execute(
        "INSERT INTO meta (id, periodo, indicador, vendedor_id, valor, ativo, atualizado_em, atualizado_por)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT (id) DO UPDATE SET valor = ?5, ativo = ?6, atualizado_em = ?7, atualizado_por = ?8",
        params![
            id,
            periodo,
            e.indicador,
            if e.vendedor_id.is_empty() { None } else { Some(e.vendedor_id.as_str()) },
            valor,
            ativo,
            tempo::agora(),
            usuario_id
        ],
    )?;
    let nome = INDICADORES_META.iter().find(|x| x.0 == e.indicador).map(|x| x.1).unwrap_or("");
    let quem = if e.vendedor_id.is_empty() { "loja".to_string() } else { nome_vendedor(conn, &e.vendedor_id)? };
    let detalhe = match (antes, ativo) {
        (Some((v, true)), true) => format!(
            "Meta {} de {nome} ({quem}): {} → {}",
            if periodo == "mes" { "mensal" } else { "semanal" },
            escrever_meta(&e.indicador, v),
            escrever_meta(&e.indicador, valor)
        ),
        (_, true) => format!(
            "Meta {} de {nome} ({quem}): {}",
            if periodo == "mes" { "mensal" } else { "semanal" },
            escrever_meta(&e.indicador, valor)
        ),
        (_, false) => format!("Meta {} de {nome} ({quem}) tirada", if periodo == "mes" { "mensal" } else { "semanal" }),
    };
    auditoria::registrar(conn, dispositivo, Some(usuario_id), "dre.meta", "meta", &id, &[detalhe])?;
    Ok(id)
}

// ---------------------------------------------------------------------------
// Resumo do Painel.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumoPainel {
    pub periodo: String,
    pub vendas_liquidas: String,
    pub devolucoes: String,
    pub lucro_bruto: String,
    pub margem_bruta: String,
    pub despesas: String,
    pub resultado: String,
    pub resultado_negativo: bool,
    pub estado: String,
    pub estado_texto: String,
    pub ticket: String,
    pub avisos: Vec<String>,
    pub metas: Vec<MetaLinha>,
}

/// O essencial para o Painel (mês atual).
pub fn resumo_painel(conn: &Connection) -> Resultado<ResumoPainel> {
    let d = apurar_dre(conn, &Filtro { periodo: "mes".into(), ..Default::default() })?;
    let mut avisos: Vec<String> =
        d.pendencias.iter().filter(|p| p.codigo != "classificacao").map(|p| p.texto.clone()).collect();
    let metas: Vec<MetaLinha> = metas(conn)?.into_iter().filter(|m| m.vendedor_id.is_empty()).collect();
    for m in &metas {
        if !m.atingida && m.periodo == "mes" {
            avisos.push(format!(
                "Meta de {} do mês: {} de {} ({}).",
                m.indicador_nome.to_lowercase(),
                m.realizado,
                m.meta,
                m.percentual
            ));
        }
    }
    Ok(ResumoPainel {
        periodo: d.periodo.rotulo.clone(),
        vendas_liquidas: d.comercial.vendas_liquidas.clone(),
        devolucoes: d.comercial.devolucoes_valor.clone(),
        lucro_bruto: dinheiro(d.numeros.lucro_bruto),
        margem_bruta: d.margem_bruta.clone(),
        despesas: dinheiro(d.numeros.despesas),
        resultado: dinheiro(d.numeros.resultado),
        resultado_negativo: d.numeros.resultado < 0,
        estado: d.estado.clone(),
        estado_texto: d.estado_texto.clone(),
        ticket: d.comercial.ticket.clone(),
        avisos,
        metas,
    })
}

// ---------------------------------------------------------------------------
// Impresso.
// ---------------------------------------------------------------------------

/// A DRE em HTML (para imprimir ou salvar em PDF).
pub fn html(conn: &Connection, f: &Filtro) -> Resultado<String> {
    use crate::impressao::escapar;
    let d = apurar_dre(conn, f)?;
    let mut h = String::new();
    h.push_str(&format!(
        "<h1>DRE gerencial · {}</h1><p class=\"nota\">{} · {}{}</p>",
        escapar(&d.periodo.rotulo),
        escapar(&d.base),
        escapar(&d.estado_texto),
        if d.vendedor.is_empty() { String::new() } else { format!(" · vendedor: {}", escapar(&d.vendedor)) }
    ));
    h.push_str("<table class=\"itens\" width=\"100%\" cellspacing=\"0\"><tr><th>LINHA</th><th class=\"num\">VALOR (R$)</th><th class=\"num\">% DA RECEITA LÍQUIDA</th></tr>");
    for l in &d.linhas {
        let (a, b) = if l.subtotal { ("<b>", "</b>") } else { ("", "") };
        h.push_str(&format!(
            "<tr><td>{a}{}{b}</td><td class=\"num\">{a}{}{b}</td><td class=\"num\">{}</td></tr>",
            escapar(&l.nome),
            l.valor,
            l.percentual
        ));
    }
    h.push_str("</table>");
    if !d.pendencias.is_empty() {
        h.push_str("<p style=\"margin-top:10pt\"><b>Pendências</b></p><ul>");
        for p in &d.pendencias {
            h.push_str(&format!("<li>{}</li>", escapar(&p.texto)));
        }
        h.push_str("</ul>");
    }
    h.push_str("<p class=\"nota\" style=\"margin-top:10pt\">");
    for a in &d.avisos {
        h.push_str(&format!("{}<br/>", escapar(a)));
    }
    h.push_str("Não substitui a escrituração contábil nem a DRE oficial. A classificação das categorias deve ser validada pelo contador.</p>");
    Ok(h)
}

#[cfg(test)]
mod testes;
