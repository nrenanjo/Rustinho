//! Busca de peças: um campo só, que acha por código, código de barras,
//! código original, equivalente, nome, marca ou carro e ano.
//!
//! Exemplos: "32206", "7891234567895", "pastilha gol 1.6 2015", "fag rolamento".

use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};
use serde::Serialize;

use crate::erro::Resultado;
use crate::{numeros, texto};

/// Uma linha da lista de resultados.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProdutoResumo {
    pub id: String,
    pub codigo: String,
    pub nome: String,
    pub marca: String,
    pub unidade: String,
    pub codigo_original: String,
    pub local: String,
    /// Preço da tabela Revenda, já escrito ("116,00").
    pub preco: String,
    pub estoque: String,
    pub estoque_valor: i64,
    /// Estoque no mínimo ou abaixo dele (ou zerado).
    pub estoque_baixo: bool,
    pub ativo: bool,
    /// O preço é o da oferta.
    pub oferta: bool,
    /// Serviço (não tem estoque).
    pub servico: bool,
    /// Resumo dos carros: "VW Gol G5 1.6 2009–2013 · Fiat Uno 1.0 · +2".
    pub aplicacoes: String,
    /// Achou pelo código exato (código, barras, original, similar ou do fornecedor).
    pub codigo_exato: bool,
    /// Por qual código achou, para o balão ao lado do nome: "Cód. barras",
    /// "Similar · Cofap", "Fornecedor · Dayco"… Vazio quando achou pelo nome.
    pub achou: String,
}

/// Um carro de uma peça, para filtrar por ano e montar o resumo.
struct Carro {
    texto: String,
    resumo: String,
    ano_inicio: Option<i64>,
    ano_fim: Option<i64>,
}

impl Carro {
    fn serve_no_ano(&self, ano: i64) -> bool {
        self.ano_inicio.is_none_or(|i| i <= ano) && self.ano_fim.is_none_or(|f| f >= ano)
    }
}

/// É um ano de carro? (4 números entre 1900 e 2100)
fn como_ano(token: &str) -> Option<i64> {
    if token.len() == 4 && token.chars().all(|c| c.is_ascii_digit()) {
        let ano: i64 = token.parse().ok()?;
        if (1900..=2100).contains(&ano) {
            return Some(ano);
        }
    }
    None
}

/// Busca peças. Com o texto vazio, mostra as alteradas por último.
pub fn buscar(
    conn: &Connection,
    texto_busca: &str,
    incluir_inativas: bool,
    limite: i64,
) -> Resultado<Vec<ProdutoResumo>> {
    let limite = limite.clamp(1, 1000) as usize;
    let normalizado = texto::normalizar(texto_busca);
    let tokens: Vec<&str> = normalizado.split(' ').filter(|t| !t.is_empty()).take(10).collect();
    let anos: Vec<(&str, i64)> = tokens.iter().filter_map(|t| como_ano(t).map(|a| (*t, a))).collect();
    let palavras: Vec<&str> = tokens.iter().filter(|t| como_ano(t).is_none()).copied().collect();
    let compacto = texto::compactar(texto_busca);

    let mut sql = format!(
        "SELECT p.id, p.codigo, p.nome, p.marca, p.unidade, p.codigo_original, p.local, p.estoque_minimo, p.ativo,
                {},
                (SELECT COALESCE(SUM(quantidade), 0) FROM estoque_movimento WHERE produto_id = p.id),
                CASE WHEN ?1 <> '' AND (instr(p.codigos, ' ' || ?1 || ' ') > 0
                               OR EXISTS (SELECT 1 FROM produto_fornecedor f
                                           WHERE f.produto_id = p.id AND f.codigo_compacto = ?1)) THEN 0
                     WHEN ?2 <> '' AND p.busca LIKE ?2 || '%' THEN 1
                     ELSE 2 END AS ordem,
                p.busca, p.codigo_barras, {}, p.tipo = 'servico'
           FROM produto p
          WHERE 1 = 1",
        crate::produtos::SQL_PRECO_1,
        crate::produtos::SQL_EM_OFERTA
    );
    let mut valores: Vec<Value> =
        vec![Value::Text(compacto.clone()), Value::Text(tokens.first().map(|t| t.to_string()).unwrap_or_default())];

    if !incluir_inativas {
        sql.push_str(" AND p.ativo = 1");
    }
    let mut grupo: Vec<String> = Vec::new();
    for token in &tokens {
        let mut condicao = format!("p.busca LIKE ?{}", valores.len() + 1);
        valores.push(Value::Text(format!("%{token}%")));
        let token_compacto = texto::compactar(token);
        if token_compacto != *token && !token_compacto.is_empty() {
            condicao.push_str(&format!(" OR p.busca LIKE ?{}", valores.len() + 1));
            valores.push(Value::Text(format!("%{token_compacto}%")));
        }
        // Modelo escrito junto ("8pk1515") acha o nome escrito separado ("8PK 1515").
        if token_compacto.len() >= 4 && token_compacto.chars().any(|c| c.is_ascii_digit()) {
            condicao.push_str(&format!(" OR replace(p.busca, ' ', '') LIKE ?{}", valores.len() + 1));
            valores.push(Value::Text(format!("%{token_compacto}%")));
        }
        if let Some(ano) = como_ano(token) {
            // Filtro grosso aqui; o fino (ano do carro certo) é feito abaixo.
            condicao.push_str(&format!(
                " OR EXISTS (SELECT 1 FROM produto_aplicacao a WHERE a.produto_id = p.id
                     AND COALESCE(a.ano_inicio, 0) <= ?{n} AND COALESCE(a.ano_fim, 9999) >= ?{n})",
                n = valores.len() + 1
            ));
            valores.push(Value::Integer(ano));
        }
        grupo.push(format!("({condicao})"));
    }
    if !grupo.is_empty() {
        // Ou acha pelas palavras, ou pelo código que o fornecedor usa.
        sql.push_str(&format!(
            " AND (({}) OR (?1 <> '' AND EXISTS (SELECT 1 FROM produto_fornecedor f
                     WHERE f.produto_id = p.id AND f.codigo_compacto = ?1)))",
            grupo.join(" AND ")
        ));
    }

    if tokens.is_empty() {
        sql.push_str(" ORDER BY p.atualizado_em DESC");
    } else {
        sql.push_str(" ORDER BY ordem, p.ativo DESC, p.nome COLLATE NOCASE");
    }
    // Com ano na busca, pega mais candidatos, porque alguns vão sair no filtro fino.
    let candidatos = if anos.is_empty() { limite } else { (limite * 5).min(5000) };
    sql.push_str(&format!(" LIMIT {candidatos}"));

    let mut consulta = conn.prepare(&sql)?;
    let linhas = consulta.query_map(params_from_iter(valores), |l| {
        let estoque_minimo: i64 = l.get(7)?;
        let preco: Option<i64> = l.get(9)?;
        let saldo: i64 = l.get(10)?;
        let ordem: i64 = l.get(11)?;
        let busca: String = l.get(12)?;
        let codigo_barras: String = l.get(13)?;
        let oferta: bool = l.get(14)?;
        let servico: bool = l.get(15)?;
        Ok((
            ProdutoResumo {
                id: l.get(0)?,
                codigo: l.get(1)?,
                nome: l.get(2)?,
                marca: l.get(3)?,
                unidade: l.get(4)?,
                codigo_original: l.get(5)?,
                local: l.get(6)?,
                ativo: l.get(8)?,
                preco: preco.map(numeros::escrever_dinheiro).unwrap_or_default(),
                estoque: numeros::escrever_quantidade(saldo),
                estoque_valor: saldo,
                estoque_baixo: !servico && (saldo <= 0 || saldo <= estoque_minimo),
                oferta,
                servico,
                aplicacoes: String::new(),
                codigo_exato: ordem == 0 && !compacto.is_empty(),
                achou: String::new(),
            },
            busca,
            codigo_barras,
        ))
    })?;
    let candidatos: Vec<(ProdutoResumo, String, String)> = linhas.collect::<Result<_, _>>()?;
    let parece_codigo = compacto.len() >= 3 && compacto.chars().any(|c| c.is_ascii_digit());

    let mut resultado = Vec::new();
    for (mut item, busca, codigo_barras) in candidatos {
        let carros = carros_da_peca(conn, &item.id)?;
        if !anos.iter().all(|(token, ano)| serve_no_ano(&busca, &carros, &palavras, token, *ano)) {
            continue;
        }
        item.aplicacoes = resumo(&carros);
        if parece_codigo {
            item.achou = achou_por(conn, &item, &codigo_barras, &compacto)?;
        }
        resultado.push(item);
        if resultado.len() >= limite {
            break;
        }
    }
    Ok(resultado)
}

/// Por qual código a peça foi achada. Primeiro o código igual ao digitado;
/// se nenhum for igual, o código que começa com o que foi digitado.
fn achou_por(conn: &Connection, item: &ProdutoResumo, codigo_barras: &str, compacto: &str) -> Resultado<String> {
    let mut candidatos: Vec<(String, String)> = vec![
        (texto::compactar(&item.codigo), "Cód. interno".to_string()),
        (texto::compactar(codigo_barras), "Cód. barras".to_string()),
        (texto::compactar(&item.codigo_original), "Cód. original".to_string()),
    ];
    let mut c = conn.prepare_cached(
        "SELECT codigo_compacto, marca FROM produto_equivalente WHERE produto_id = ?1 ORDER BY ordem",
    )?;
    for linha in c.query_map([&item.id], |l| Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?)))? {
        let (codigo, marca) = linha?;
        let nome = if marca.trim().is_empty() { "Similar".to_string() } else { format!("Similar · {}", marca.trim()) };
        candidatos.push((codigo, nome));
    }
    let mut c = conn.prepare_cached(
        "SELECT f.codigo_compacto, COALESCE(NULLIF(p.fantasia, ''), p.nome)
           FROM produto_fornecedor f JOIN pessoa p ON p.id = f.fornecedor_id
          WHERE f.produto_id = ?1 ORDER BY f.atualizado_em DESC",
    )?;
    for linha in c.query_map([&item.id], |l| Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?)))? {
        let (codigo, nome) = linha?;
        let curto: String = nome.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
        candidatos.push((codigo, format!("Fornecedor · {curto}")));
    }
    let igual = candidatos.iter().find(|(codigo, _)| codigo == compacto);
    let comeca = || candidatos.iter().find(|(codigo, _)| !codigo.is_empty() && codigo.starts_with(compacto));
    Ok(igual.or_else(comeca).map(|(_, nome)| nome.clone()).unwrap_or_default())
}

/// A peça serve no ano buscado? O ano tem que ser do carro certo: em
/// "gol 2010", vale o ano de uma aplicação do Gol, não de outro carro.
fn serve_no_ano(busca: &str, carros: &[Carro], palavras: &[&str], token: &str, ano: i64) -> bool {
    // O número pode estar no nome ou num código.
    if busca.split(' ').any(|p| p == token) {
        return true;
    }
    // Carros que combinam com alguma palavra da busca; se nenhum combina
    // (ex.: "pastilha 2016"), vale qualquer carro da peça.
    let combinam: Vec<&Carro> = carros.iter().filter(|c| palavras.iter().any(|p| c.texto.contains(p))).collect();
    let considerar: Vec<&Carro> = if combinam.is_empty() { carros.iter().collect() } else { combinam };
    considerar.iter().any(|c| c.serve_no_ano(ano))
}

fn carros_da_peca(conn: &Connection, produto_id: &str) -> Resultado<Vec<Carro>> {
    let mut consulta = conn.prepare_cached(
        "SELECT montadora, modelo, motor, observacao, ano_inicio, ano_fim FROM produto_aplicacao
          WHERE produto_id = ?1 ORDER BY ordem",
    )?;
    let linhas = consulta.query_map([produto_id], |l| {
        let montadora: String = l.get(0)?;
        let modelo: String = l.get(1)?;
        let motor: String = l.get(2)?;
        let observacao: String = l.get(3)?;
        let ano_inicio: Option<i64> = l.get(4)?;
        let ano_fim: Option<i64> = l.get(5)?;
        let sinonimos = texto::sinonimos_montadora(&texto::normalizar(&montadora)).join(" ");
        let anos = match (ano_inicio, ano_fim) {
            (Some(i), Some(f)) if i == f => i.to_string(),
            (Some(i), Some(f)) => format!("{i}–{f}"),
            (Some(i), None) => format!("{i} em diante"),
            (None, Some(f)) => format!("até {f}"),
            (None, None) => String::new(),
        };
        let resumo = [montadora.as_str(), modelo.as_str(), motor.as_str(), anos.as_str()]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        Ok(Carro {
            texto: texto::normalizar(&format!("{montadora} {sinonimos} {modelo} {motor} {observacao}")),
            resumo,
            ano_inicio,
            ano_fim,
        })
    })?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

/// Resumo curto dos carros em que a peça serve.
fn resumo(carros: &[Carro]) -> String {
    const MOSTRAR: usize = 3;
    let mut partes: Vec<String> = carros.iter().take(MOSTRAR).map(|c| c.resumo.clone()).collect();
    if carros.len() > MOSTRAR {
        partes.push(format!("+{}", carros.len() - MOSTRAR));
    }
    partes.join(" · ")
}

/// Quantas peças existem.
pub fn total(conn: &Connection, incluir_inativas: bool) -> Resultado<i64> {
    let sql =
        if incluir_inativas { "SELECT COUNT(*) FROM produto" } else { "SELECT COUNT(*) FROM produto WHERE ativo = 1" };
    Ok(conn.query_row(sql, [], |l| l.get(0))?)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::{pessoas, produtos, usuarios};

    #[test]
    fn acha_pelo_codigo_do_fornecedor() {
        let b = Banco::em_memoria().unwrap();
        let c = &b.conn;
        let d = &b.dispositivo;
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        let mut f = produtos::ficha_nova(c).unwrap();
        f.nome = "Óleo 5W30 sintético".into();
        f.custo = "30".into();
        let p = produtos::salvar(c, d, &u, &f).unwrap();
        let mut forn = pessoas::ficha_nova(c, "fornecedor").unwrap();
        forn.nome = "Distribuidora Dayco Brasil".into();
        let forn = pessoas::salvar(c, d, &u, &forn).unwrap();
        c.execute(
            "INSERT INTO produto_fornecedor (fornecedor_id, codigo_compacto, codigo, produto_id, atualizado_em)
             VALUES (?1, 'dy5530', 'DY-5530', ?2, '2026-10-15')",
            [&forn, &p],
        )
        .unwrap();

        let r = buscar(c, "DY 5530", false, 10).unwrap();
        assert_eq!(r.len(), 1);
        assert!(r[0].codigo_exato);
        assert_eq!(r[0].achou, "Fornecedor · Distribuidora Dayco");
        // pelo nome, sem acento e sem balão
        let r = buscar(c, "oleo 5w30", false, 10).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].achou, "");
        // código interno
        let r = buscar(c, &r[0].codigo.clone(), false, 10).unwrap();
        assert_eq!(r[0].achou, "");
    }
}
