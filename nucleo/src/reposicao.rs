//! Estoque que orienta: o que comprar (e quanto), o que mais vende e o que
//! está parado.
//!
//! A sugestão olha o ritmo de venda dos últimos 90 dias de cada peça e quer
//! estoque para os próximos N dias (N escolhido pela loja, conforme a
//! demanda). As encomendas de clientes ficam à parte: não entram no ritmo de
//! venda, nem na conta da reposição.

use std::collections::HashMap;

use rusqlite::Connection;
use serde::Serialize;

use crate::erro::{Erro, Resultado};
use crate::{auditoria, config, encomendas, fiscal, numeros, tempo, texto};

/// Quantos dias de venda a sugestão olha para trás.
pub const JANELA: i64 = 90;
pub const DIAS_COMPRA_PADRAO: i64 = 30;
pub const DIAS_PARADO_PADRAO: i64 = 180;

fn ler_dias(conn: &Connection, chave: &str, padrao: i64) -> Resultado<i64> {
    Ok(config::ler(conn, chave)?.and_then(|v| v.parse().ok()).unwrap_or(padrao))
}

/// Para quantos dias de venda comprar.
pub fn dias_compra(conn: &Connection) -> Resultado<i64> {
    ler_dias(conn, "estoque_dias_compra", DIAS_COMPRA_PADRAO)
}

/// A partir de quantos dias sem vender a peça conta como parada.
pub fn dias_parado(conn: &Connection) -> Resultado<i64> {
    ler_dias(conn, "estoque_dias_parado", DIAS_PARADO_PADRAO)
}

/// Guarda os dias (compra: 7 a 180; parado: 30 a 730).
pub fn salvar_dias(conn: &Connection, dispositivo: &str, usuario_id: &str, qual: &str, dias: &str) -> Resultado<i64> {
    let (chave, min, max, campo) = match qual {
        "compra" => ("estoque_dias_compra", 7, 180, "diasCompra"),
        "parado" => ("estoque_dias_parado", 30, 730, "diasParado"),
        _ => return Err(Erro::Interno(format!("dias desconhecidos: {qual}"))),
    };
    let n: i64 = dias
        .trim()
        .parse()
        .map_err(|_| Erro::campo(campo, "Digite o número de dias."))?;
    if !(min..=max).contains(&n) {
        return Err(Erro::campo(campo, format!("Use de {min} a {max} dias.")));
    }
    let antes = ler_dias(conn, chave, if qual == "compra" { DIAS_COMPRA_PADRAO } else { DIAS_PARADO_PADRAO })?;
    if antes != n {
        config::gravar(conn, chave, &n.to_string())?;
        auditoria::registrar(
            conn,
            dispositivo,
            Some(usuario_id),
            "estoque.dias",
            "config",
            chave,
            &[format!("{}: {antes} → {n} dias", if qual == "compra" { "Comprar para" } else { "Parada depois de" })],
        )?;
    }
    Ok(n)
}

/// Arredonda a quantidade (milésimos) para cima, em unidades inteiras.
fn inteiro_acima(q: f64) -> i64 {
    ((q / 1000.0 - 1e-9).ceil() as i64).max(0) * 1000
}

/// Último fornecedor de cada peça (pela última compra lançada) e o código
/// da peça nesse fornecedor.
fn fornecedores(conn: &Connection) -> Resultado<HashMap<String, (String, String, String)>> {
    let mut c = conn.prepare(
        "SELECT ci.produto_id, COALESCE(c.fornecedor_id, ''), COALESCE(f.fantasia, ''), COALESCE(f.nome, ''),
                COALESCE((SELECT pf.codigo FROM produto_fornecedor pf
                           WHERE pf.produto_id = ci.produto_id AND pf.fornecedor_id = c.fornecedor_id
                           ORDER BY pf.atualizado_em DESC LIMIT 1), ci.codigo_fornecedor)
           FROM compra_item ci
           JOIN compra c ON c.id = ci.compra_id
           LEFT JOIN pessoa f ON f.id = c.fornecedor_id
          WHERE c.situacao = 'lancada' AND ci.produto_id IS NOT NULL
          ORDER BY c.lancada_em",
    )?;
    let mut mapa = HashMap::new();
    let linhas = c.query_map([], |l| {
        Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?, l.get::<_, String>(2)?, l.get::<_, String>(3)?, l.get::<_, String>(4)?))
    })?;
    for linha in linhas {
        let (pid, fid, fantasia, nome, codigo) = linha?;
        let nome = if fantasia.trim().is_empty() { nome } else { fantasia };
        // A ordem é da mais antiga para a mais nova: fica a última.
        mapa.insert(pid, (fid, nome, codigo));
    }
    Ok(mapa)
}

// ---------------------------------------------------------------------------
// O que comprar
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemCompra {
    pub produto_id: String,
    pub codigo: String,
    pub nome: String,
    pub marca: String,
    pub unidade: String,
    pub local: String,
    pub estoque: String,
    /// "tem 3 UN" ou "sem estoque".
    pub tem: String,
    /// "12 vendidas em 90 dias".
    pub vendido: String,
    /// "≈ 4 por mês".
    pub ritmo: String,
    pub sugerido: String,
    pub custo: String,
    pub custo_total: String,
    pub fornecedor: String,
    pub codigo_fornecedor: String,
    /// "acabou", "acabando" ou "minimo".
    pub prioridade: String,
    /// "Acabou", "Acaba em 5 dias", "Abaixo do mínimo".
    pub nome_prioridade: String,
    /// "1 guardada para encomenda" etc.
    pub observacao: String,
    #[serde(skip)]
    pub v_sugerido: i64,
    #[serde(skip)]
    pub v_custo_total: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncomendaPedir {
    pub id: String,
    pub numero: i64,
    pub cliente: String,
    pub produto_id: String,
    pub codigo: String,
    pub descricao: String,
    pub quantidade: String,
    pub unidade: String,
    pub desde: String,
    pub fornecedor: String,
    pub codigo_fornecedor: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sugestao {
    pub dias_compra: i64,
    pub janela: i64,
    pub acabou: i64,
    pub acabando: i64,
    pub abaixo_minimo: i64,
    pub custo_total: String,
    pub encomendas: Vec<EncomendaPedir>,
    pub itens: Vec<ItemCompra>,
}

pub fn sugestao(conn: &Connection, busca: &str) -> Resultado<Sugestao> {
    let dias = dias_compra(conn)?;
    let desde = tempo::dias_atras(JANELA);
    let reservas: HashMap<String, (i64, i64)> =
        encomendas::reservas(conn)?.into_iter().map(|(p, a, b)| (p, (a, b))).collect();
    let forn = fornecedores(conn)?;
    let filtro = texto::normalizar(busca);
    let mut c = conn.prepare(
        "WITH vend AS (
              SELECT vi.produto_id,
                     SUM(vi.quantidade - COALESCE((SELECT SUM(di.quantidade) FROM devolucao_item di JOIN devolucao d ON d.id = di.devolucao_id
                                                    WHERE di.venda_item_id = vi.id AND d.cancelado_em IS NULL), 0)) AS q
                FROM venda_item vi JOIN venda v ON v.id = vi.venda_id
               WHERE v.tipo = 'venda' AND v.situacao = 'concluida' AND v.criado_em >= ?1 AND vi.produto_id IS NOT NULL
               GROUP BY vi.produto_id),
              enc AS (
              SELECT produto_id, SUM(quantidade) AS q FROM encomenda
               WHERE venda_id IS NOT NULL AND situacao <> 'cancelada' AND criado_em >= ?1 AND produto_id IS NOT NULL
               GROUP BY produto_id),
              est AS (SELECT produto_id, SUM(quantidade) AS q FROM estoque_movimento GROUP BY produto_id)
         SELECT p.id, p.codigo, p.nome, p.marca, p.unidade, p.local, p.custo, p.estoque_minimo, COALESCE(est.q, 0),
                MAX(COALESCE(vend.q, 0) - COALESCE(enc.q, 0), 0),
                CAST(julianday('now') - julianday(p.criado_em) AS INTEGER)
           FROM produto p
           LEFT JOIN vend ON vend.produto_id = p.id
           LEFT JOIN enc ON enc.produto_id = p.id
           LEFT JOIN est ON est.produto_id = p.id
          WHERE p.ativo = 1 AND p.tipo <> 'servico' AND (COALESCE(vend.q, 0) > 0 OR p.estoque_minimo > 0)",
    )?;
    struct Bruto {
        id: String,
        codigo: String,
        nome: String,
        marca: String,
        unidade: String,
        local: String,
        custo: i64,
        minimo: i64,
        estoque: i64,
        vendido: i64,
        idade: i64,
    }
    let brutos: Vec<Bruto> = c
        .query_map([&desde], |l| {
            Ok(Bruto {
                id: l.get(0)?,
                codigo: l.get(1)?,
                nome: l.get(2)?,
                marca: l.get(3)?,
                unidade: l.get(4)?,
                local: l.get(5)?,
                custo: l.get(6)?,
                minimo: l.get(7)?,
                estoque: l.get(8)?,
                vendido: l.get(9)?,
                idade: l.get(10)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    let mut itens: Vec<(i64, f64, ItemCompra)> = Vec::new();
    for b in brutos {
        let (pendente, guardada) = reservas.get(&b.id).copied().unwrap_or((0, 0));
        // O que está livre para vender: as encomendas pagas já saíram do
        // estoque (e vão chegar); as que chegaram sem pagar estão guardadas.
        // Estoque negativo (vendeu sem dar entrada) conta como zero.
        let disponivel = (b.estoque + pendente).max(0) - guardada;
        // Peça nova: o ritmo conta só desde o cadastro (no mínimo 30 dias).
        let base = b.idade.clamp(30, JANELA) as f64;
        let por_dia = b.vendido as f64 / base;
        let alvo = inteiro_acima(por_dia * dias as f64).max(b.minimo);
        let falta = alvo - disponivel;
        if falta <= 0 {
            continue;
        }
        let sugerido = inteiro_acima(falta as f64);
        let restam = if por_dia > 0.0 { disponivel as f64 / por_dia } else { f64::INFINITY };
        let (rank, prioridade, nome_prioridade) = if disponivel <= 0 {
            (0, "acabou", "Acabou".to_string())
        } else if restam < dias as f64 {
            let n = restam.floor() as i64;
            let t = match n {
                0 => "Acaba hoje".to_string(),
                1 => "Acaba amanhã".to_string(),
                n => format!("Acaba em {n} dias"),
            };
            (1, "acabando", t)
        } else if disponivel < b.minimo {
            (2, "minimo", "Abaixo do mínimo".to_string())
        } else {
            // Só arredondamento: não vale a pena.
            continue;
        };
        if !filtro.is_empty() {
            let f = forn.get(&b.id).map(|x| x.1.as_str()).unwrap_or("");
            let alvo_busca = texto::normalizar(&format!("{} {} {} {}", b.codigo, b.nome, b.marca, f));
            if !alvo_busca.contains(&filtro) {
                continue;
            }
        }
        let custo_total = ((b.custo as i128 * sugerido as i128 + 500) / 1000) as i64;
        let por_mes = por_dia * 30.0 / 1000.0;
        let ritmo = if b.vendido == 0 {
            "sem vendas".to_string()
        } else if por_mes < 1.0 {
            "menos de 1 por mês".to_string()
        } else {
            format!("≈ {} por mês", por_mes.round() as i64)
        };
        let mut obs = Vec::new();
        if guardada > 0 {
            obs.push(format!("{} guardada para encomenda", numeros::escrever_quantidade(guardada)));
        }
        if b.minimo > 0 {
            obs.push(format!("mínimo {}", numeros::escrever_quantidade(b.minimo)));
        }
        let (_, fornecedor, codigo_fornecedor) = forn.get(&b.id).cloned().unwrap_or_default();
        let tem = if b.estoque <= 0 {
            "sem estoque".to_string()
        } else {
            format!("tem {} {}", numeros::escrever_quantidade(b.estoque), b.unidade)
        };
        itens.push((
            rank,
            if rank == 0 { -(b.vendido as f64) } else if rank == 1 { restam } else { 0.0 },
            ItemCompra {
                produto_id: b.id,
                codigo: b.codigo,
                nome: b.nome,
                marca: b.marca,
                unidade: b.unidade,
                local: b.local,
                estoque: numeros::escrever_quantidade(b.estoque),
                tem,
                vendido: format!("{} em {JANELA} dias", numeros::escrever_quantidade(b.vendido)),
                ritmo,
                sugerido: numeros::escrever_quantidade(sugerido),
                custo: numeros::escrever_dinheiro(b.custo),
                custo_total: numeros::escrever_dinheiro(custo_total),
                fornecedor,
                codigo_fornecedor,
                prioridade: prioridade.into(),
                nome_prioridade,
                observacao: obs.join(" · "),
                v_sugerido: sugerido,
                v_custo_total: custo_total,
            },
        ));
    }
    itens.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.2.nome.cmp(&b.2.nome))
    });
    let acabou = itens.iter().filter(|i| i.0 == 0).count() as i64;
    let acabando = itens.iter().filter(|i| i.0 == 1).count() as i64;
    let abaixo_minimo = itens.iter().filter(|i| i.0 == 2).count() as i64;
    let total: i64 = itens.iter().map(|i| i.2.v_custo_total).sum();

    // Encomendas que falta pedir
    let mut c = conn.prepare(
        "SELECT e.id, e.numero, e.cliente_nome, COALESCE(e.produto_id, ''), COALESCE(p.codigo, ''), e.descricao,
                e.quantidade, COALESCE(p.unidade, 'UN'), date(e.criado_em, 'localtime')
           FROM encomenda e LEFT JOIN produto p ON p.id = e.produto_id
          WHERE e.situacao = 'aberta' ORDER BY e.criado_em",
    )?;
    let hoje = fiscal::hoje(conn)?;
    let mut lista_enc = Vec::new();
    let linhas = c.query_map([], |l| {
        Ok((
            l.get::<_, String>(0)?,
            l.get::<_, i64>(1)?,
            l.get::<_, String>(2)?,
            l.get::<_, String>(3)?,
            l.get::<_, String>(4)?,
            l.get::<_, String>(5)?,
            l.get::<_, i64>(6)?,
            l.get::<_, String>(7)?,
            l.get::<_, String>(8)?,
        ))
    })?;
    for linha in linhas {
        let (id, numero, cliente, pid, codigo, descricao, q, unidade, criado) = linha?;
        let (_, fornecedor, codigo_fornecedor) = forn.get(&pid).cloned().unwrap_or_default();
        if !filtro.is_empty() && !texto::normalizar(&format!("{codigo} {descricao} {cliente} {fornecedor}")).contains(&filtro) {
            continue;
        }
        let d = crate::receber::dias_entre(&criado, &hoje);
        lista_enc.push(EncomendaPedir {
            id,
            numero,
            cliente,
            produto_id: pid,
            codigo,
            descricao,
            quantidade: numeros::escrever_quantidade(q),
            unidade,
            desde: match d {
                d if d <= 0 => "hoje".into(),
                1 => "ontem".into(),
                d => format!("há {d} dias"),
            },
            fornecedor,
            codigo_fornecedor,
        });
    }

    Ok(Sugestao {
        dias_compra: dias,
        janela: JANELA,
        acabou,
        acabando,
        abaixo_minimo,
        custo_total: numeros::escrever_dinheiro(total),
        encomendas: lista_enc,
        itens: itens.into_iter().map(|i| i.2).collect(),
    })
}

// ---------------------------------------------------------------------------
// Mais vendidos (com curva ABC)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vendido {
    pub posicao: i64,
    pub produto_id: String,
    pub codigo: String,
    pub nome: String,
    pub marca: String,
    pub unidade: String,
    pub quantidade: String,
    pub vendas: i64,
    pub total: String,
    pub lucro: String,
    pub margem: String,
    /// "A", "B" ou "C".
    pub curva: String,
    pub estoque: String,
    pub estoque_baixo: bool,
    /// Parte do faturamento, de 0 a 1 (para a barrinha).
    pub parte: f64,
    #[serde(skip)]
    pub v_quantidade: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaisVendidos {
    pub dias: i64,
    pub total: String,
    pub lucro: String,
    pub pecas: i64,
    pub curva_a: i64,
    pub curva_b: i64,
    pub curva_c: i64,
    pub itens: Vec<Vendido>,
}

/// `ordem`: "valor" (faturamento) ou "quantidade".
pub fn mais_vendidos(conn: &Connection, dias: i64, ordem: &str, busca: &str) -> Resultado<MaisVendidos> {
    let dias = dias.clamp(1, 3650);
    let desde = tempo::dias_atras(dias);
    let mut c = conn.prepare(
        "WITH est AS (SELECT produto_id, SUM(quantidade) AS q FROM estoque_movimento GROUP BY produto_id)
         SELECT p.id, p.codigo, p.nome, p.marca, p.unidade, SUM(vi.quantidade - COALESCE(dv.q, 0)), COUNT(DISTINCT v.id),
                SUM(vi.total - COALESCE(dv.v, 0)),
                SUM((vi.custo * (vi.quantidade - COALESCE(dv.q, 0)) + 500) / 1000), COALESCE(MAX(est.q), 0), p.estoque_minimo
           FROM venda_item vi
           JOIN venda v ON v.id = vi.venda_id
           JOIN produto p ON p.id = vi.produto_id
           LEFT JOIN est ON est.produto_id = p.id
           LEFT JOIN (SELECT di.venda_item_id, SUM(di.quantidade) AS q, SUM(di.valor_item) AS v
                        FROM devolucao_item di JOIN devolucao d ON d.id = di.devolucao_id
                       WHERE d.cancelado_em IS NULL GROUP BY di.venda_item_id) dv ON dv.venda_item_id = vi.id
          WHERE v.tipo = 'venda' AND v.situacao = 'concluida' AND v.criado_em >= ?1
          GROUP BY p.id
         HAVING SUM(vi.quantidade - COALESCE(dv.q, 0)) > 0
          ORDER BY SUM(vi.total - COALESCE(dv.v, 0)) DESC",
    )?;
    struct Linha {
        id: String,
        codigo: String,
        nome: String,
        marca: String,
        unidade: String,
        q: i64,
        vendas: i64,
        total: i64,
        custo: i64,
        estoque: i64,
        minimo: i64,
    }
    let linhas: Vec<Linha> = c
        .query_map([&desde], |l| {
            Ok(Linha {
                id: l.get(0)?,
                codigo: l.get(1)?,
                nome: l.get(2)?,
                marca: l.get(3)?,
                unidade: l.get(4)?,
                q: l.get(5)?,
                vendas: l.get(6)?,
                total: l.get(7)?,
                custo: l.get(8)?,
                estoque: l.get(9)?,
                minimo: l.get(10)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    let soma: i64 = linhas.iter().map(|l| l.total).sum();
    let lucro_total: i64 = linhas.iter().map(|l| l.total - l.custo).sum();
    let maior = linhas.iter().map(|l| l.total).max().unwrap_or(0).max(1);
    let mut acumulado = 0i64;
    let mut curvas = (0, 0, 0);
    let mut itens = Vec::new();
    for l in linhas {
        // A curva é pelo faturamento, na ordem do maior para o menor: a peça
        // é "A" se o que vem antes dela ainda não chegou a 80%.
        let antes = acumulado;
        acumulado += l.total;
        let curva = if soma == 0 || antes * 100 < soma * 80 {
            curvas.0 += 1;
            "A"
        } else if antes * 100 < soma * 95 {
            curvas.1 += 1;
            "B"
        } else {
            curvas.2 += 1;
            "C"
        };
        let lucro = l.total - l.custo;
        itens.push(Vendido {
            posicao: 0,
            produto_id: l.id,
            codigo: l.codigo,
            nome: l.nome,
            marca: l.marca,
            unidade: l.unidade,
            quantidade: numeros::escrever_quantidade(l.q),
            vendas: l.vendas,
            total: numeros::escrever_dinheiro(l.total),
            lucro: numeros::escrever_dinheiro(lucro),
            margem: if l.total > 0 && l.custo > 0 {
                format!("{}%", numeros::escrever_percentual(((lucro as i128 * 10_000) / l.total as i128) as i64))
            } else {
                String::new()
            },
            curva: curva.into(),
            estoque: numeros::escrever_quantidade(l.estoque),
            estoque_baixo: l.estoque <= 0 || l.estoque <= l.minimo,
            parte: l.total as f64 / maior as f64,
            v_quantidade: l.q,
        });
    }
    let pecas = itens.len() as i64;
    if ordem == "quantidade" {
        itens.sort_by_key(|i| std::cmp::Reverse(i.v_quantidade));
        let maior_q = itens.first().map(|i| i.v_quantidade).unwrap_or(1).max(1);
        for i in &mut itens {
            i.parte = i.v_quantidade as f64 / maior_q as f64;
        }
    }
    let filtro = texto::normalizar(busca);
    let mut saida: Vec<Vendido> = Vec::new();
    for (n, mut i) in itens.into_iter().enumerate() {
        i.posicao = n as i64 + 1;
        if !filtro.is_empty() && !texto::normalizar(&format!("{} {} {}", i.codigo, i.nome, i.marca)).contains(&filtro) {
            continue;
        }
        saida.push(i);
        if saida.len() >= 300 {
            break;
        }
    }
    Ok(MaisVendidos {
        dias,
        total: numeros::escrever_dinheiro(soma),
        lucro: numeros::escrever_dinheiro(lucro_total),
        pecas,
        curva_a: curvas.0,
        curva_b: curvas.1,
        curva_c: curvas.2,
        itens: saida,
    })
}

// ---------------------------------------------------------------------------
// Parados
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Parado {
    pub produto_id: String,
    pub codigo: String,
    pub nome: String,
    pub marca: String,
    pub unidade: String,
    pub local: String,
    pub estoque: String,
    pub valor: String,
    /// "Última venda em 12/03/2026" ou "Nunca vendeu".
    pub ultima_venda: String,
    /// "há 214 dias".
    pub ha: String,
    pub ultima_compra: String,
    #[serde(skip)]
    pub v_valor: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Parados {
    pub dias: i64,
    pub quantidade: i64,
    pub valor: String,
    pub nunca_venderam: i64,
    pub itens: Vec<Parado>,
}

pub fn parados(conn: &Connection, busca: &str) -> Resultado<Parados> {
    let dias = dias_parado(conn)?;
    let limite = tempo::dias_atras(dias);
    let hoje = fiscal::hoje(conn)?;
    let mut c = conn.prepare(
        "WITH est AS (SELECT produto_id, SUM(quantidade) AS q FROM estoque_movimento GROUP BY produto_id),
              uv AS (SELECT vi.produto_id, MAX(v.criado_em) AS u FROM venda_item vi JOIN venda v ON v.id = vi.venda_id
                      WHERE v.tipo = 'venda' AND v.situacao = 'concluida' AND vi.produto_id IS NOT NULL
                      GROUP BY vi.produto_id),
              ue AS (SELECT produto_id, MAX(criado_em) AS u FROM estoque_movimento
                      WHERE tipo IN ('compra', 'inicial') GROUP BY produto_id)
         SELECT p.id, p.codigo, p.nome, p.marca, p.unidade, p.local, est.q, p.custo,
                COALESCE(date(uv.u, 'localtime'), ''), COALESCE(date(ue.u, 'localtime'), ''),
                date(MAX(COALESCE(uv.u, ''), COALESCE(ue.u, ''), p.criado_em), 'localtime')
           FROM produto p
           JOIN est ON est.produto_id = p.id AND est.q > 0
           LEFT JOIN uv ON uv.produto_id = p.id
           LEFT JOIN ue ON ue.produto_id = p.id
          WHERE p.ativo = 1 AND p.tipo <> 'servico' AND MAX(COALESCE(uv.u, ''), COALESCE(ue.u, ''), p.criado_em) < ?1",
    )?;
    let filtro = texto::normalizar(busca);
    let mut itens = Vec::new();
    let mut nunca = 0;
    let linhas = c.query_map([&limite], |l| {
        Ok((
            l.get::<_, String>(0)?,
            l.get::<_, String>(1)?,
            l.get::<_, String>(2)?,
            l.get::<_, String>(3)?,
            l.get::<_, String>(4)?,
            l.get::<_, String>(5)?,
            l.get::<_, i64>(6)?,
            l.get::<_, i64>(7)?,
            l.get::<_, String>(8)?,
            l.get::<_, String>(9)?,
            l.get::<_, String>(10)?,
        ))
    })?;
    for linha in linhas {
        let (id, codigo, nome, marca, unidade, local, q, custo, uv, ue, referencia) = linha?;
        if uv.is_empty() {
            nunca += 1;
        }
        if !filtro.is_empty() && !texto::normalizar(&format!("{codigo} {nome} {marca} {local}")).contains(&filtro) {
            continue;
        }
        let valor = ((custo as i128 * q as i128 + 500) / 1000) as i64;
        let ha = crate::receber::dias_entre(&referencia, &hoje);
        itens.push(Parado {
            produto_id: id,
            codigo,
            nome,
            marca,
            unidade,
            local,
            estoque: numeros::escrever_quantidade(q),
            valor: numeros::escrever_dinheiro(valor),
            ultima_venda: if uv.is_empty() { "Nunca vendeu".into() } else { format!("Última venda em {}", fiscal::data_br(&uv)) },
            ha: format!("parada há {ha} dias"),
            ultima_compra: if ue.is_empty() { String::new() } else { format!("entrou em {}", fiscal::data_br(&ue)) },
            v_valor: valor,
        });
    }
    itens.sort_by(|a, b| b.v_valor.cmp(&a.v_valor).then_with(|| a.nome.cmp(&b.nome)));
    let total: i64 = itens.iter().map(|i| i.v_valor).sum();
    Ok(Parados {
        dias,
        quantidade: itens.len() as i64,
        valor: numeros::escrever_dinheiro(total),
        nunca_venderam: nunca,
        itens,
    })
}

/// Lista de compra para imprimir, por fornecedor.
pub fn lista_compra_html(conn: &Connection) -> Resultado<String> {
    use crate::impressao::escapar;
    let s = sugestao(conn, "")?;
    let mut grupos: Vec<(String, Vec<String>)> = Vec::new();
    let mut add = |forn: &str, linha: String| {
        let nome = if forn.is_empty() { "Sem fornecedor anterior".to_string() } else { forn.to_string() };
        match grupos.iter_mut().find(|g| g.0 == nome) {
            Some(g) => g.1.push(linha),
            None => grupos.push((nome, vec![linha])),
        }
    };
    for e in &s.encomendas {
        add(
            &e.fornecedor,
            format!(
                "<tr><td>{}</td><td>{}</td><td><b>{}</b><br/><span class=\"nota\">Encomenda nº {} · {}</span></td><td class=\"num\"><b>{} {}</b></td><td class=\"num\">—</td></tr>",
                escapar(&e.codigo),
                escapar(&e.codigo_fornecedor),
                escapar(&e.descricao),
                e.numero,
                escapar(&e.cliente),
                e.quantidade,
                escapar(&e.unidade)
            ),
        );
    }
    for i in &s.itens {
        add(
            &i.fornecedor,
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}{}<br/><span class=\"nota\">{} · {} · {}</span></td><td class=\"num\"><b>{} {}</b></td><td class=\"num\">{}</td></tr>",
                escapar(&i.codigo),
                escapar(&i.codigo_fornecedor),
                escapar(&i.nome),
                if i.marca.is_empty() { String::new() } else { format!(" · {}", escapar(&i.marca)) },
                escapar(&i.nome_prioridade),
                escapar(&i.tem),
                escapar(&i.ritmo),
                i.sugerido,
                escapar(&i.unidade),
                i.custo_total
            ),
        );
    }
    let mut h = String::new();
    h.push_str(&format!(
        "<h1>Lista de compra</h1><p class=\"nota\">Para {} dias de venda · {} itens · custo estimado R$ {} (sem as encomendas)</p>",
        s.dias_compra,
        s.itens.len() + s.encomendas.len(),
        s.custo_total
    ));
    if grupos.is_empty() {
        h.push_str("<p>Nada a comprar agora.</p>");
    }
    for (forn, linhas) in grupos {
        h.push_str(&format!(
            "<p style=\"font-family:'Cormorant Garamond Ajustada'; font-size:15pt; font-weight:600; margin:12pt 0 2pt 0\">{}</p>",
            escapar(&forn)
        ));
        h.push_str(
            "<table class=\"itens\" width=\"100%\" cellspacing=\"0\"><tr><th width=\"9%\">CÓDIGO</th><th width=\"14%\">CÓD. FORNEC.</th>\
             <th width=\"55%\">PEÇA</th><th class=\"num\" width=\"11%\">COMPRAR</th><th class=\"num\" width=\"11%\">CUSTO</th></tr>",
        );
        for l in linhas {
            h.push_str(&l);
        }
        h.push_str("</table>");
    }
    Ok(h)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::vendas::{self, ItemEntrada, PagamentoEntrada, VendaEntrada};
    use crate::{caixa, produtos, usuarios};

    fn peca(b: &Banco, u: &str, nome: &str, custo: &str, estoque: &str) -> String {
        let mut f = produtos::ficha_nova(&b.conn).unwrap();
        f.nome = nome.into();
        f.custo = custo.into();
        f.estoque_inicial = estoque.into();
        produtos::salvar(&b.conn, &b.dispositivo, u, &f).unwrap()
    }

    fn vender(b: &Banco, u: &str, p: &str, q: &str, valor: &str) {
        let e = VendaEntrada {
            itens: vec![ItemEntrada { produto_id: p.into(), quantidade: q.into(), ..Default::default() }],
            tabela: 1,
            pagamentos: vec![PagamentoEntrada { forma: "pix".into(), valor: valor.into(), ..Default::default() }],
            ..Default::default()
        };
        vendas::concluir(&b.conn, &b.dispositivo, u, &e).unwrap();
    }

    #[test]
    fn sugere_compra_mostra_mais_vendidos_e_parados() {
        let b = Banco::em_memoria().unwrap();
        let (c, d) = (&b.conn, b.dispositivo.as_str());
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        caixa::abrir(c, d, &u, "Caixa 1", "0").unwrap();
        let filtro = peca(&b, &u, "Filtro de óleo", "10", "12"); // preço 20
        let vela = peca(&b, &u, "Vela", "5", "4"); // preço 10
        let farol = peca(&b, &u, "Farol Gol G3", "200", "2");

        // 9 filtros vendidos (peça nova: o ritmo conta 30 dias → 0,3 por dia).
        vender(&b, &u, &filtro, "9", "180");
        vender(&b, &u, &vela, "4", "40");
        let s = sugestao(c, "").unwrap();
        assert_eq!(s.dias_compra, 30);
        assert_eq!(s.itens.len(), 2);
        assert_eq!((s.itens[0].nome.as_str(), s.itens[0].nome_prioridade.as_str()), ("Vela", "Acabou"));
        assert_eq!(s.itens[0].sugerido, "4");
        assert_eq!(s.itens[1].nome_prioridade, "Acaba em 10 dias");
        // quer 9 para 30 dias, tem 3: comprar 6 × 10,00
        assert_eq!((s.itens[1].sugerido.as_str(), s.itens[1].custo_total.as_str()), ("6", "60,00"));
        assert_eq!(s.custo_total, "80,00");
        assert_eq!(sugestao(c, "vela").unwrap().itens.len(), 1);

        // Para 15 dias, o filtro (3 para 10 dias) pede só 2.
        assert!(salvar_dias(c, d, &u, "compra", "3").is_err());
        salvar_dias(c, d, &u, "compra", "15").unwrap();
        let s = sugestao(c, "filtro").unwrap();
        assert_eq!(s.itens[0].sugerido, "2");

        let m = mais_vendidos(c, 30, "valor", "").unwrap();
        assert_eq!((m.total.as_str(), m.pecas), ("220,00", 2));
        assert_eq!((m.itens[0].nome.as_str(), m.itens[0].curva.as_str()), ("Filtro de óleo", "A"));
        assert_eq!(m.itens[1].curva, "B");
        assert_eq!(m.itens[0].lucro, "90,00");
        let m = mais_vendidos(c, 30, "quantidade", "").unwrap();
        assert_eq!(m.itens[0].nome, "Filtro de óleo");

        // Farol sem vender: só fica "parado" depois de 180 dias.
        assert!(parados(c, "").unwrap().itens.is_empty());
        c.execute("UPDATE produto SET criado_em = '2025-01-10T12:00:00Z' WHERE id = ?1", [&farol]).unwrap();
        c.execute("UPDATE estoque_movimento SET criado_em = '2025-01-10T12:00:00Z' WHERE produto_id = ?1", [&farol]).unwrap();
        let p = parados(c, "").unwrap();
        assert_eq!(p.itens.len(), 1);
        assert_eq!((p.valor.as_str(), p.itens[0].ultima_venda.as_str()), ("400,00", "Nunca vendeu"));
        assert!(lista_compra_html(c).unwrap().contains("Filtro de óleo"));
    }
}
