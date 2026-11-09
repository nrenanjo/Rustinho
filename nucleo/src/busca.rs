//! Busca de peças: um campo só, que acha por código, código de barras,
//! código original, equivalente, nome, marca ou carro e ano.
//!
//! Exemplos: "32206", "7891234567895", "pastilha gol 1.6 2015", "fag rolamento".

use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};
use serde::Serialize;
use std::collections::HashSet;

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
    /// Código do fornecedor explicitamente escolhido como principal.
    pub codigo_fornecedor_principal: String,
    pub fornecedor_principal: String,
    /// Código textual que produziu uma correspondência exata.
    pub codigo_encontrado: String,
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
    Ok(buscar_pagina(conn, texto_busca, incluir_inativas, 0, limite)?.0)
}

/// Busca uma página e devolve a contagem exata dos resultados. A contagem
/// respeita o filtro por ano do veículo, que precisa ser conferido por peça.
pub fn buscar_pagina(
    conn: &Connection,
    texto_busca: &str,
    incluir_inativas: bool,
    deslocamento: i64,
    limite: i64,
) -> Resultado<(Vec<ProdutoResumo>, i64)> {
    let limite = limite.clamp(1, 200) as usize;
    let deslocamento = deslocamento.max(0) as usize;
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
                                           WHERE f.produto_id = p.id AND f.codigo_compacto = ?1)
                               OR EXISTS (SELECT 1 FROM produto_fornecedor_alternativo f
                                           WHERE f.produto_id = p.id AND f.codigo_compacto = ?1)
                               OR EXISTS (SELECT 1 FROM produto_gtin g
                                           WHERE g.produto_id = p.id AND g.gtin = ?1)
                               OR EXISTS (SELECT 1 FROM compra_item ci
                                           WHERE ci.produto_id = p.id AND
                                                 (ci.gtin = ?1 OR ci.gtin_tributavel = ?1
                                                  OR ci.codigo_barra = ?1 OR ci.codigo_barra_tributavel = ?1))
                               OR EXISTS (SELECT 1 FROM produto_codigo_antigo a
                                           WHERE a.produto_id = p.id AND a.codigo_compacto = ?1)) THEN 0
                     WHEN (?2 <> '' AND p.busca LIKE ?2 || '%') OR
                          (?1 <> '' AND (EXISTS (SELECT 1 FROM produto_fornecedor f
                                                  WHERE f.produto_id = p.id AND f.codigo_compacto LIKE ?1 || '%')
                                     OR EXISTS (SELECT 1 FROM produto_fornecedor_alternativo f
                                                  WHERE f.produto_id = p.id AND f.codigo_compacto LIKE ?1 || '%')
                                     OR EXISTS (SELECT 1 FROM produto_gtin g
                                                  WHERE g.produto_id = p.id AND g.gtin LIKE ?1 || '%')
                                     OR EXISTS (SELECT 1 FROM produto_equivalente e
                                                  WHERE e.produto_id = p.id AND e.codigo_compacto LIKE ?1 || '%')
                                     OR EXISTS (SELECT 1 FROM produto_codigo_antigo a
                                                  WHERE a.produto_id = p.id AND a.codigo_compacto LIKE ?1 || '%'))) THEN 1
                     ELSE 2 END AS ordem,
                p.busca, p.codigo_barras, {}, p.tipo = 'servico',
                (p.codigo_validado = 0 AND p.codigo <> ''),
                CASE WHEN p.fornecedor_principal_id <> '' THEN p.fornecedor_principal_codigo ELSE '' END,
                COALESCE((SELECT COALESCE(NULLIF(fantasia, ''), nome) FROM pessoa
                           WHERE id = p.fornecedor_principal_id), '')
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
        // Ou acha pelas palavras, ou pelo código que o fornecedor usa, ou
        // por um código que a peça já teve (etiqueta velha).
        sql.push_str(&format!(
            " AND (({}) OR (?1 <> '' AND (EXISTS (SELECT 1 FROM produto_fornecedor f
                     WHERE f.produto_id = p.id AND f.codigo_compacto LIKE ?1 || '%')
                  OR EXISTS (SELECT 1 FROM produto_fornecedor_alternativo f
                     WHERE f.produto_id = p.id AND f.codigo_compacto LIKE ?1 || '%')
                  OR EXISTS (SELECT 1 FROM produto_gtin g
                     WHERE g.produto_id = p.id AND g.gtin LIKE ?1 || '%')
                  OR EXISTS (SELECT 1 FROM produto_equivalente e
                     WHERE e.produto_id = p.id AND e.codigo_compacto LIKE ?1 || '%')
                  OR EXISTS (SELECT 1 FROM compra_item ci
                     WHERE ci.produto_id = p.id AND
                           (ci.gtin LIKE ?1 || '%' OR ci.gtin_tributavel LIKE ?1 || '%'
                            OR ci.codigo_barra LIKE ?1 || '%' OR ci.codigo_barra_tributavel LIKE ?1 || '%'))
                  OR EXISTS (SELECT 1 FROM produto_codigo_antigo a
                     WHERE a.produto_id = p.id AND a.codigo_compacto LIKE ?1 || '%'))))",
            grupo.join(" AND ")
        ));
    }

    let sql_contagem = format!("SELECT COUNT(*) FROM ({sql})");
    let total_sem_ano: i64 = conn.query_row(&sql_contagem, params_from_iter(valores.clone()), |l| l.get(0))?;

    if tokens.is_empty() {
        sql.push_str(" ORDER BY p.atualizado_em DESC, p.id");
    } else {
        // Desempate estável entre nomes iguais para que páginas sucessivas
        // não repitam nem percam peças quando há muitas correspondências.
        sql.push_str(" ORDER BY ordem, p.ativo DESC, p.nome COLLATE NOCASE, p.id");
    }
    // Sem ano, a consulta SQL já expressa o resultado final e pode paginar.
    // Com ano, a aplicação correta depende do carro correspondente, então
    // filtramos todos os candidatos antes de calcular deslocamento e total.
    if anos.is_empty() {
        sql.push_str(&format!(" LIMIT {limite} OFFSET {deslocamento}"));
    }

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
        // Código com letras que ainda espera a confirmação do caixa que
        // distribui as faixas: o leitor não acha a peça por ele.
        let esperando: bool = l.get(16)?;
        let codigo_fornecedor_principal: String = l.get(17)?;
        let fornecedor_principal: String = l.get(18)?;
        let codigo: String = l.get(1)?;
        let exato_pelo_interno_esperando = esperando && texto::compactar(&codigo) == compacto;
        Ok((
            ProdutoResumo {
                id: l.get(0)?,
                codigo,
                nome: l.get(2)?,
                marca: l.get(3)?,
                unidade: l.get(4)?,
                codigo_original: l.get(5)?,
                codigo_fornecedor_principal,
                fornecedor_principal,
                codigo_encontrado: String::new(),
                local: l.get(6)?,
                ativo: l.get(8)?,
                preco: preco.map(numeros::escrever_dinheiro).unwrap_or_default(),
                estoque: numeros::escrever_quantidade(saldo),
                estoque_valor: saldo,
                estoque_baixo: !servico && (saldo <= 0 || saldo <= estoque_minimo),
                oferta,
                servico,
                aplicacoes: String::new(),
                codigo_exato: ordem == 0 && !compacto.is_empty() && !exato_pelo_interno_esperando,
                achou: String::new(),
            },
            busca,
            codigo_barras,
        ))
    })?;
    let candidatos: Vec<(ProdutoResumo, String, String)> = linhas.collect::<Result<_, _>>()?;
    let parece_codigo = compacto.len() >= 3 && compacto.chars().any(|c| c.is_ascii_digit());

    let mut resultado = Vec::new();
    let mut total_com_ano = 0_i64;
    for (mut item, busca, codigo_barras) in candidatos {
        preencher_fornecedor_unico(conn, &mut item)?;
        let carros = carros_da_peca(conn, &item.id)?;
        if !anos.iter().all(|(token, ano)| serve_no_ano(&busca, &carros, &palavras, token, *ano)) {
            continue;
        }
        total_com_ano += 1;
        if !anos.is_empty() && ((total_com_ano as usize) <= deslocamento || resultado.len() >= limite) {
            continue;
        }
        item.aplicacoes = resumo(&carros);
        if parece_codigo {
            (item.achou, item.codigo_encontrado) = achou_por(conn, &item, &codigo_barras, &compacto)?;
        }
        resultado.push(item);
    }
    Ok((resultado, if anos.is_empty() { total_sem_ano } else { total_com_ano }))
}

/// Se houver exatamente um fornecedor/código vinculado, ele é seguro para
/// exibição. Com mais de uma referência, só a escolha explícita é principal.
fn preencher_fornecedor_unico(conn: &Connection, item: &mut ProdutoResumo) -> Resultado<()> {
    if !item.fornecedor_principal.is_empty() || !item.codigo_fornecedor_principal.is_empty() {
        return Ok(());
    }
    let linhas: Vec<(String, String, String, String)> = conn
        .prepare_cached(
            "SELECT f.fornecedor_id, f.codigo_compacto, f.codigo, COALESCE(NULLIF(p.fantasia, ''), p.nome)
               FROM produto_fornecedor f JOIN pessoa p ON p.id = f.fornecedor_id
              WHERE f.produto_id = ?1
             UNION
             SELECT f.fornecedor_id, f.codigo_compacto, f.codigo, COALESCE(NULLIF(p.fantasia, ''), p.nome)
               FROM produto_fornecedor_alternativo f JOIN pessoa p ON p.id = f.fornecedor_id
              WHERE f.produto_id = ?1",
        )?
        .query_map([&item.id], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)))?
        .collect::<Result<_, _>>()?;
    let unicos: HashSet<(String, String)> =
        linhas.iter().map(|(id, codigo, _, _)| (id.clone(), codigo.clone())).collect();
    if unicos.len() == 1 {
        if let Some((_, _, codigo, fornecedor)) = linhas.first() {
            item.codigo_fornecedor_principal = codigo.clone();
            item.fornecedor_principal = fornecedor.clone();
        }
    }
    Ok(())
}

/// Por qual código a peça foi achada. Primeiro o código igual ao digitado;
/// se nenhum for igual, o código que começa com o que foi digitado.
fn achou_por(
    conn: &Connection,
    item: &ProdutoResumo,
    codigo_barras: &str,
    compacto: &str,
) -> Resultado<(String, String)> {
    let mut candidatos: Vec<(String, String, String)> = vec![
        (texto::compactar(&item.codigo), "Cód. interno".to_string(), item.codigo.clone()),
        (texto::compactar(codigo_barras), "Cód. barras".to_string(), codigo_barras.to_string()),
        (texto::compactar(&item.codigo_original), "Cód. original".to_string(), item.codigo_original.clone()),
    ];
    let mut c = conn.prepare_cached("SELECT gtin FROM produto_gtin WHERE produto_id = ?1")?;
    for linha in c.query_map([&item.id], |l| l.get::<_, String>(0))? {
        let codigo = linha?;
        candidatos.push((texto::compactar(&codigo), "Cód. barras".to_string(), codigo));
    }
    let mut c = conn.prepare_cached(
        "SELECT gtin FROM compra_item WHERE produto_id = ?1 AND gtin <> ''
         UNION SELECT gtin_tributavel FROM compra_item WHERE produto_id = ?1 AND gtin_tributavel <> ''
         UNION SELECT codigo_barra FROM compra_item WHERE produto_id = ?1 AND codigo_barra <> ''
         UNION SELECT codigo_barra_tributavel FROM compra_item WHERE produto_id = ?1 AND codigo_barra_tributavel <> ''",
    )?;
    for linha in c.query_map([&item.id], |l| l.get::<_, String>(0))? {
        let codigo = linha?;
        candidatos.push((texto::compactar(&codigo), "Cód. barras".to_string(), codigo));
    }
    let mut c = conn.prepare_cached(
        "SELECT codigo_compacto, marca, codigo FROM produto_equivalente WHERE produto_id = ?1 ORDER BY ordem",
    )?;
    for linha in
        c.query_map([&item.id], |l| Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?, l.get::<_, String>(2)?)))?
    {
        let (compacto_codigo, marca, codigo) = linha?;
        let nome = if marca.trim().is_empty() { "Similar".to_string() } else { format!("Similar · {}", marca.trim()) };
        candidatos.push((compacto_codigo, nome, codigo));
    }
    for tabela in ["produto_fornecedor", "produto_fornecedor_alternativo"] {
        let sql = format!(
            "SELECT f.codigo_compacto, f.codigo, COALESCE(NULLIF(p.fantasia, ''), p.nome)
               FROM {tabela} f JOIN pessoa p ON p.id = f.fornecedor_id
              WHERE f.produto_id = ?1 ORDER BY f.atualizado_em DESC"
        );
        let mut c = conn.prepare(&sql)?;
        for linha in
            c.query_map([&item.id], |l| Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?, l.get::<_, String>(2)?)))?
        {
            let (compacto_codigo, codigo, nome) = linha?;
            let curto: String = nome.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
            candidatos.push((compacto_codigo, format!("Fornecedor · {curto} · {codigo}"), codigo));
        }
    }
    let mut c =
        conn.prepare_cached("SELECT codigo_compacto, codigo FROM produto_codigo_antigo WHERE produto_id = ?1")?;
    for linha in c.query_map([&item.id], |l| Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?)))? {
        let (compacto_codigo, codigo) = linha?;
        candidatos.push((compacto_codigo, "Cód. antigo".to_string(), codigo));
    }
    let igual = candidatos.iter().find(|(codigo, _, _)| codigo == compacto);
    let encontrado =
        igual.or_else(|| candidatos.iter().find(|(codigo, _, _)| !codigo.is_empty() && codigo.starts_with(compacto)));
    Ok(encontrado.map(|(_, nome, codigo)| (nome.clone(), codigo.clone())).unwrap_or_default())
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

/// Leitor de código de barras (ou código digitado e Enter): as peças com
/// este código exato (interno, de barras, original, similar, do fornecedor
/// ou antigo). Mais de uma = a tela pergunta qual; nunca escolhe sozinha.
pub fn pelo_codigo_exato(conn: &Connection, codigo: &str) -> Resultado<Vec<ProdutoResumo>> {
    if texto::compactar(codigo).is_empty() {
        return Ok(Vec::new());
    }
    Ok(buscar(conn, codigo, false, 50)?.into_iter().filter(|p| p.codigo_exato).collect())
}

/// Quantas peças esperam a confirmação do código interno e têm exatamente
/// este código (o leitor não as acha por ele; veja `faixas`).
pub fn esperando_pelo_codigo(conn: &Connection, codigo: &str) -> Resultado<usize> {
    let compacto = texto::compactar(codigo);
    if compacto.is_empty() {
        return Ok(0);
    }
    let mut c = conn.prepare_cached(
        "SELECT codigo FROM produto WHERE codigo_validado = 0 AND codigo <> '' AND instr(codigos, ' ' || ?1 || ' ') > 0",
    )?;
    let n = c
        .query_map([&compacto], |l| l.get::<_, String>(0))?
        .filter_map(Result::ok)
        .filter(|c| texto::compactar(c) == compacto)
        .count();
    Ok(n)
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
    fn busca_devolve_contagem_exata_e_pagina_separada() {
        let b = Banco::em_memoria().unwrap();
        let c = &b.conn;
        let u = usuarios::criar(c, &b.dispositivo, "Maria", "1234", true).unwrap().id;
        for nome in ["Filtro óleo Alpha", "Filtro óleo Beta", "Pastilha Gamma"] {
            let mut f = produtos::ficha_nova(c).unwrap();
            f.nome = nome.into();
            f.custo = "10".into();
            produtos::salvar(c, &b.dispositivo, &u, &f).unwrap();
        }

        let (primeira, total) = buscar_pagina(c, "filtro", false, 0, 1).unwrap();
        assert_eq!(primeira.len(), 1);
        assert_eq!(total, 2);
        let (segunda, total_segunda) = buscar_pagina(c, "filtro", false, 1, 1).unwrap();
        assert_eq!(segunda.len(), 1);
        assert_eq!(total_segunda, 2);
        assert_ne!(primeira[0].id, segunda[0].id);
        let (vazia, total_vazio) = buscar_pagina(c, "inexistente", false, 0, 10).unwrap();
        assert!(vazia.is_empty());
        assert_eq!(total_vazio, 0);
    }

    #[test]
    fn paginacao_tem_desempate_estavel_e_nao_repete_pecas_com_nome_igual() {
        let b = Banco::em_memoria().unwrap();
        let c = &b.conn;
        let u = usuarios::criar(c, &b.dispositivo, "Maria", "1234", true).unwrap().id;
        for _ in 0..7 {
            let mut f = produtos::ficha_nova(c).unwrap();
            f.nome = "Filtro igual".into();
            f.custo = "10".into();
            produtos::salvar(c, &b.dispositivo, &u, &f).unwrap();
        }

        let paginas = || {
            (0..4)
                .map(|n| buscar_pagina(c, "Filtro igual", false, n * 2, 2).unwrap().0)
                .flatten()
                .map(|p| p.id)
                .collect::<Vec<_>>()
        };
        let primeira_leitura = paginas();
        let segunda_leitura = paginas();
        assert_eq!(primeira_leitura.len(), 7);
        assert_eq!(primeira_leitura, segunda_leitura, "a ordem entre páginas permanece estável");
        assert_eq!(primeira_leitura.iter().collect::<HashSet<_>>().len(), 7, "nenhuma peça se repete");
    }

    #[test]
    fn acha_pelo_codigo_do_fornecedor() {
        let b = Banco::em_memoria().unwrap();
        let c = &b.conn;
        let d = &b.dispositivo;
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        let mut f = produtos::ficha_nova(c).unwrap();
        f.nome = "Óleo 5W30 sintético".into();
        f.codigo_barras = "036000291452".into();
        f.codigo_original = "OEM-22".into();
        f.custo = "30".into();
        let p = produtos::salvar(c, d, &u, &f).unwrap();
        assert!(produtos::definir_codigo(c, d, &p, "12345").unwrap());
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
        assert_eq!(r[0].achou, "Fornecedor · Distribuidora Dayco · DY-5530");
        assert_eq!(r[0].codigo_encontrado, "DY-5530");
        assert_eq!(r[0].codigo_fornecedor_principal, "DY-5530");
        let (parcial, total) = buscar_pagina(c, "DY-55", false, 0, 10).unwrap();
        assert_eq!(total, 1);
        assert_eq!(parcial.len(), 1);
        assert_eq!(parcial[0].codigo_encontrado, "DY-5530");
        // pelo nome, sem acento e sem balão
        let r = buscar(c, "oleo 5w30", false, 10).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].achou, "");
        // código interno
        let r = buscar(c, "12345", false, 10).unwrap();
        assert_eq!(r[0].achou, "Cód. interno");

        let por_gtin = buscar(c, "036000291452", false, 10).unwrap();
        assert_eq!(por_gtin[0].achou, "Cód. barras");
        assert_eq!(por_gtin[0].codigo_encontrado, "036000291452");
        let por_original = buscar(c, "OEM 22", false, 10).unwrap();
        assert_eq!(por_original[0].achou, "Cód. original");
        assert_eq!(por_original[0].codigo_encontrado, "OEM-22");
    }

    #[test]
    fn acha_gtin_importado_e_mapeamento_alternativo_sem_escolher_um_produto() {
        let b = Banco::em_memoria().unwrap();
        let c = &b.conn;
        let d = &b.dispositivo;
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        let mut f = produtos::ficha_nova(c).unwrap();
        f.nome = "Filtro com GTIN alternativo".into();
        f.custo = "1,00".into();
        let p1 = produtos::salvar(c, d, &u, &f).unwrap();
        c.execute("INSERT INTO produto_gtin (produto_id, gtin) VALUES (?1, '036000291452')", [&p1]).unwrap();
        let por_gtin = buscar(c, "036000291452", false, 10).unwrap();
        assert_eq!(por_gtin.len(), 1);
        assert!(por_gtin[0].codigo_exato);
        assert_eq!(por_gtin[0].achou, "Cód. barras");

        let mut f = produtos::ficha_nova(c).unwrap();
        f.nome = "Outro produto com código ambíguo".into();
        f.custo = "1,00".into();
        let p2 = produtos::salvar(c, d, &u, &f).unwrap();
        let mut forn = pessoas::ficha_nova(c, "fornecedor").unwrap();
        forn.nome = "Fornecedor Exemplo".into();
        let forn = pessoas::salvar(c, d, &u, &forn).unwrap();
        c.execute(
            "INSERT INTO produto_fornecedor (fornecedor_id, codigo_compacto, codigo, produto_id, atualizado_em)
             VALUES (?1, 'ref01', 'REF-01', ?2, '2026-10-15')",
            [&forn, &p1],
        )
        .unwrap();
        c.execute(
            "INSERT INTO produto_fornecedor_alternativo
                (fornecedor_id, codigo_compacto, codigo, produto_id, atualizado_em)
             VALUES (?1, 'ref01', 'REF-01', ?2, '2026-10-16')",
            [&forn, &p2],
        )
        .unwrap();
        let ambiguos = buscar(c, "REF-01", false, 10).unwrap();
        assert_eq!(ambiguos.len(), 2);
        assert!(ambiguos.iter().all(|p| p.codigo_exato));
        assert!(ambiguos.iter().all(|p| p.achou.starts_with("Fornecedor ·")));
        assert!(ambiguos.iter().all(|p| p.codigo_encontrado == "REF-01"));
    }
}
