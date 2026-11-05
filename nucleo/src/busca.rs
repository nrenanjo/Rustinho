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
                                           WHERE f.produto_id = p.id AND f.codigo_compacto = ?1)
                               OR EXISTS (SELECT 1 FROM produto_codigo_antigo a
                                           WHERE a.produto_id = p.id AND a.codigo_compacto = ?1)) THEN 0
                     WHEN ?2 <> '' AND p.busca LIKE ?2 || '%' THEN 1
                     ELSE 2 END AS ordem,
                p.busca, p.codigo_barras, {}, p.tipo = 'servico',
                (p.codigo_validado = 0 AND p.codigo <> '')
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
                     WHERE f.produto_id = p.id AND f.codigo_compacto = ?1)
                  OR EXISTS (SELECT 1 FROM produto_codigo_antigo a
                     WHERE a.produto_id = p.id AND a.codigo_compacto = ?1))))",
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
        // Código com letras que ainda espera a confirmação do caixa que
        // distribui as faixas: o leitor não acha a peça por ele.
        let esperando: bool = l.get(16)?;
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
    let mut c =
        conn.prepare_cached("SELECT codigo_compacto, codigo FROM produto_codigo_antigo WHERE produto_id = ?1")?;
    for linha in c.query_map([&item.id], |l| Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?)))? {
        let (compacto, _) = linha?;
        candidatos.push((compacto, "Cód. antigo".to_string()));
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

    // -----------------------------------------------------------------------
    // 0.9.18 — busca por código em qualquer ordem, por prefixo, e sem casar
    // termo curto no meio de palavra (capturas do dono, base de 3.598 peças).
    // -----------------------------------------------------------------------

    struct Vitrine {
        b: Banco,
        u: String,
        hidraflux: String,
    }

    impl Vitrine {
        fn nova() -> Vitrine {
            let b = Banco::em_memoria().unwrap();
            // Este caixa distribui os códigos: as peças saem com código interno.
            crate::faixas::assumir(&b.conn).unwrap();
            let u = usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap().id;
            let mut forn = pessoas::ficha_nova(&b.conn, "fornecedor").unwrap();
            forn.nome = "HIDRAFLUX INDUSTRIAL LTDA".into();
            let hidraflux = pessoas::salvar(&b.conn, &b.dispositivo, &u, &forn).unwrap();
            Vitrine { b, u, hidraflux }
        }

        fn peca(&self, nome: &str, ajuste: impl FnOnce(&mut produtos::ProdutoFicha)) -> String {
            let mut f = produtos::ficha_nova(&self.b.conn).unwrap();
            f.nome = nome.into();
            f.custo = "10".into();
            ajuste(&mut f);
            produtos::salvar(&self.b.conn, &self.b.dispositivo, &self.u, &f).unwrap()
        }

        fn fornecedor(&self, nome: &str) -> String {
            let mut forn = pessoas::ficha_nova(&self.b.conn, "fornecedor").unwrap();
            forn.nome = nome.into();
            pessoas::salvar(&self.b.conn, &self.b.dispositivo, &self.u, &forn).unwrap()
        }

        /// Código que o fornecedor usa para a peça (como a compra grava).
        fn codigo_do_fornecedor(&self, fornecedor: &str, produto: &str, codigo: &str) {
            self.b
                .conn
                .execute(
                    "INSERT INTO produto_fornecedor (fornecedor_id, codigo_compacto, codigo, produto_id, atualizado_em)
                     VALUES (?1, ?2, ?3, ?4, '2026-10-15')",
                    [fornecedor, &texto::compactar(codigo), codigo, produto],
                )
                .unwrap();
        }

        /// Código que a peça já teve (etiqueta velha).
        fn codigo_antigo(&self, produto: &str, codigo: &str) {
            self.b
                .conn
                .execute(
                    "INSERT INTO produto_codigo_antigo (codigo, produto_id, codigo_compacto, trocado_em)
                     VALUES (?1, ?2, ?3, '2026-10-15')",
                    [codigo, produto, &texto::compactar(codigo)],
                )
                .unwrap();
        }

        fn ids(&self, texto: &str) -> Vec<String> {
            buscar(&self.b.conn, texto, false, 50).unwrap().into_iter().map(|p| p.id).collect()
        }

        fn nomes(&self, texto: &str) -> Vec<String> {
            buscar(&self.b.conn, texto, false, 50).unwrap().into_iter().map(|p| p.nome).collect()
        }

        fn primeira(&self, texto: &str) -> ProdutoResumo {
            buscar(&self.b.conn, texto, false, 50).unwrap().into_iter().next().unwrap_or_else(|| panic!("{texto}: nada"))
        }
    }

    /// As peças das seis capturas: o terminal (código do fornecedor TCF-01),
    /// uma válvula com código de fornecedor que começa com RG-08, e as cinco
    /// peças que apareciam em "rg 08" sem relação nenhuma ("rg" no meio de
    /// "cargo", "descarga", "emergencia", "02530brg"; "08" no meio de números).
    struct Capturas {
        v: Vitrine,
        terminal: String,
        valvula: String,
        sem_relacao: Vec<String>,
    }

    fn capturas() -> Capturas {
        let v = Vitrine::nova();
        let terminal = v.peca("TERMINAL FG 3/4\" UNF 20 FIOS X 3/8\"", |_| {});
        v.codigo_do_fornecedor(&v.hidraflux, &terminal, "TCF-01");
        let valvula = v.peca("VALVULA REGULADORA DE AR", |_| {});
        v.codigo_do_fornecedor(&v.hidraflux, &valvula, "RG-0815");
        let sem_relacao = vec![
            v.peca("02530BRG RETENTOR CAIXA MUDANCA - SAB", |f| f.codigo_original = "2108".into()),
            v.peca("FR99285 JOGO REPARO VALVULA DESCARGA RAPIDA - AMX", |f| f.codigo_original = "40812".into()),
            v.peca("FR99375 JOGO REPARO VALVULA DESCARGA - AMX", |f| f.codigo_original = "50813".into()),
            v.peca("RELE PISCA EMERGENCIA OU ALTERNADO 24V 5T", |f| f.codigo_original = "70815".into()),
            v.peca("TECFIL AS836 AR FORD CARGO 1114 / 1117 - TEG129620A", |f| f.codigo_original = "90817".into()),
        ];
        Capturas { v, terminal, valvula, sem_relacao }
    }

    /// Mostra o que cada consulta das capturas acha (para o relatório):
    /// `cargo test -p rustinho-nucleo --lib busca::testes::mostrar_capturas -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn mostrar_capturas() {
        let c = capturas();
        for texto in ["TCF", "TCF 01", "01 TCF", "rg08", "rg-08", "rg 08", "08", "terminal tcf", "carg"] {
            let r = buscar(&c.v.b.conn, texto, false, 50).unwrap();
            println!("{texto:?} → {} peça(s)", r.len());
            for p in r {
                println!("    {} | exata={} | balão={:?}", p.nome, p.codigo_exato, p.achou);
            }
        }
    }

    /// Capturas 1 a 3: "TCF" e "01 TCF" não achavam nada; só "TCF 01" achava.
    #[test]
    fn captura_tcf_em_qualquer_ordem_e_por_prefixo() {
        let c = capturas();
        for texto in ["TCF", "tcf", "TCF 01", "01 TCF", "tcf-01", "TCF01", "01 tcf", "tcf 0"] {
            assert_eq!(c.v.ids(texto), vec![c.terminal.clone()], "{texto}");
        }
        // Exata só com o código inteiro; prefixo e ordem trocada não são "exata".
        assert!(c.v.primeira("TCF 01").codigo_exato);
        assert!(!c.v.primeira("TCF").codigo_exato);
        assert!(!c.v.primeira("01 TCF").codigo_exato);
        // O balão diz por qual código achou, também no prefixo e na ordem trocada.
        for texto in ["TCF 01", "TCF", "01 TCF"] {
            assert_eq!(c.v.primeira(texto).achou, "Fornecedor · HIDRAFLUX INDUSTRIAL", "{texto}");
        }
    }

    /// Capturas 4 a 6: "rg08" e "rg-08" não achavam nada e "rg 08" achava 5
    /// peças sem relação. As três escritas agora dão o mesmo resultado.
    #[test]
    fn captura_rg08_coerente_e_sem_falsos_positivos() {
        let c = capturas();
        for texto in ["rg08", "rg-08", "rg 08", "RG 08", "RG-08", "rg"] {
            let achou = c.v.ids(texto);
            assert_eq!(achou, vec![c.valvula.clone()], "{texto}: {:?}", c.v.nomes(texto));
            for id in &c.sem_relacao {
                assert!(!achou.contains(id), "{texto}");
            }
        }
        assert_eq!(c.v.primeira("rg 08").achou, "Fornecedor · HIDRAFLUX INDUSTRIAL");
    }

    /// Termo curto (1 ou 2 letras/números) só casa no começo de palavra ou numa
    /// fronteira letra↔número/pontuação; termo de 3 ou mais pode casar no meio,
    /// mas fica depois dos que casam no começo.
    #[test]
    fn termo_curto_nao_casa_no_meio_da_palavra() {
        let c = capturas();
        // "08": só na fronteira de "RG-0815"; não no meio de 2108, 40812...
        assert_eq!(c.v.ids("08"), vec![c.valvula.clone()], "{:?}", c.v.nomes("08"));
        // "ar": palavra inteira; não no meio de "cargo", "reparo", "descarga".
        assert_eq!(c.v.nomes("ar"), ["TECFIL AS836 AR FORD CARGO 1114 / 1117 - TEG129620A", "VALVULA REGULADORA DE AR"]);
        // "36" está no meio de "836": não casa.
        assert!(c.v.ids("36").is_empty(), "{:?}", c.v.nomes("36"));
        // "836" e "129620": fronteira letra→número em "as836" e "teg129620a".
        assert_eq!(c.v.nomes("836"), ["TECFIL AS836 AR FORD CARGO 1114 / 1117 - TEG129620A"]);
        assert_eq!(c.v.nomes("129620"), ["TECFIL AS836 AR FORD CARGO 1114 / 1117 - TEG129620A"]);
        // Quatro letras casam no meio, depois de quem casa no começo.
        assert_eq!(
            c.v.nomes("carg"),
            [
                "TECFIL AS836 AR FORD CARGO 1114 / 1117 - TEG129620A",
                "FR99285 JOGO REPARO VALVULA DESCARGA RAPIDA - AMX",
                "FR99375 JOGO REPARO VALVULA DESCARGA - AMX",
            ]
        );
        // "brg" começa depois dos números de "02530BRG" (fronteira número→letra).
        assert_eq!(c.v.nomes("brg"), ["02530BRG RETENTOR CAIXA MUDANCA - SAB"]);
    }

    /// Termos de campos diferentes juntos: nome + código do fornecedor.
    #[test]
    fn termos_misturados_entre_campos() {
        let c = capturas();
        for texto in ["terminal tcf", "tcf terminal", "terminal 01 tcf", "unf tcf-01", "valvula rg", "reguladora 0815"] {
            let achou = c.v.ids(texto);
            assert_eq!(achou.len(), 1, "{texto}: {:?}", c.v.nomes(texto));
        }
        assert_eq!(c.v.ids("terminal tcf"), vec![c.terminal.clone()]);
        assert_eq!(c.v.ids("valvula rg"), vec![c.valvula.clone()]);
        // Todos os termos têm que casar em algum campo.
        assert!(c.v.ids("terminal rg08").is_empty());
        assert!(c.v.ids("valvula tcf").is_empty());
    }

    /// Ordem: código exato > prefixo de código / todos os termos no mesmo
    /// código > nome começando pelo 1º termo > todos os termos no começo de
    /// palavra > casamento no meio de palavra. Inativa depois da ativa.
    #[test]
    fn ordem_de_relevancia() {
        let v = Vitrine::nova();
        let p5 = v.peca("BUCHA XABC12", |_| {});
        let p4 = v.peca("BUCHA ABC12", |_| {});
        let p3 = v.peca("ABC12 BUCHA", |_| {});
        let p2 = v.peca("COXIM CAMBIO", |f| f.codigo_original = "ABC12X".into());
        let p1 = v.peca("SUPORTE MOTOR", |_| {});
        v.codigo_do_fornecedor(&v.hidraflux, &p1, "ABC-12");
        let inativa = v.peca("ABC12 INATIVA", |f| f.ativo = false);
        let _fora = v.peca("ABC BUCHA 12", |_| {});

        assert_eq!(v.ids("abc12"), vec![p1.clone(), p2.clone(), p3.clone(), p4.clone(), p5.clone()]);
        assert_eq!(v.ids("ABC-12"), v.ids("abc12"));
        let r = buscar(&v.b.conn, "abc12", true, 50).unwrap();
        let ids: Vec<&str> = r.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, [&p1, &p2, &p3, &inativa, &p4, &p5].map(String::as_str));
        assert!(r[0].codigo_exato);
        assert!(r[1..].iter().all(|p| !p.codigo_exato));
        assert_eq!(r[1].achou, "Cód. original");
        // Qualquer ordem dos termos acha as mesmas; a ordem dos resultados
        // segue o nome que começa pelo primeiro termo.
        assert_eq!(v.ids("bucha abc12"), vec![p4.clone(), p3.clone(), p5.clone()]);
        assert_eq!(v.ids("abc12 bucha"), vec![p3.clone(), p4.clone(), p5.clone()]);
        // Limite.
        assert_eq!(buscar(&v.b.conn, "abc12", false, 2).unwrap().len(), 2);
    }

    /// As modalidades de antes continuam: código interno, barras, original,
    /// similar (equivalente), antigo, fornecedor, nome, marca, carro e ano,
    /// modelo escrito junto, inativas, limite e texto vazio.
    #[test]
    fn modalidades_preservadas() {
        let v = Vitrine::nova();
        let rol = v.peca("Rolamento roda traseira", |f| {
            f.marca = "SKF".into();
            f.codigo_barras = "7891234567895".into();
            f.codigo_original = "32206A".into();
            f.equivalentes = vec![produtos::EquivalenteFicha { marca: "Cofap".into(), codigo: "BR-32206".into() }];
            f.aplicacoes = vec![produtos::AplicacaoFicha {
                montadora: "VW".into(),
                modelo: "Gol G5".into(),
                motor: "1.6".into(),
                ano_inicio: "2009".into(),
                ano_fim: "2013".into(),
                observacao: String::new(),
            }];
        });
        let pas = v.peca("Pastilha de freio dianteira", |f| {
            f.marca = "Fras-le".into();
            f.aplicacoes = vec![
                produtos::AplicacaoFicha {
                    montadora: "Fiat".into(),
                    modelo: "Uno".into(),
                    ano_inicio: "2010".into(),
                    ano_fim: "2014".into(),
                    ..Default::default()
                },
                produtos::AplicacaoFicha {
                    montadora: "VW".into(),
                    modelo: "Gol".into(),
                    ano_inicio: "2014".into(),
                    ano_fim: "2018".into(),
                    ..Default::default()
                },
            ];
        });
        v.codigo_antigo(&pas, "OLD-555");
        let correia = v.peca("Correia poli V 8PK 1515", |_| {});
        let dayco = v.fornecedor("Distribuidora Dayco Brasil");
        v.codigo_do_fornecedor(&dayco, &correia, "DY-5530");
        let inativa = v.peca("Pastilha traseira velha", |f| f.ativo = false);

        // Código interno.
        let codigo = buscar(&v.b.conn, "", false, 50).unwrap().into_iter().find(|p| p.id == rol).unwrap().codigo;
        let r = v.primeira(&codigo);
        assert!(!codigo.is_empty());
        assert_eq!((r.id.as_str(), r.codigo_exato), (rol.as_str(), true), "código {codigo}");
        // Código de barras, original, similar, antigo, fornecedor: exato e balão.
        for (texto, id, balao) in [
            ("7891234567895", &rol, "Cód. barras"),
            ("32206a", &rol, "Cód. original"),
            ("br-32206", &rol, "Similar · Cofap"),
            ("old 555", &pas, "Cód. antigo"),
            ("DY 5530", &correia, "Fornecedor · Distribuidora Dayco"),
        ] {
            let r = v.primeira(texto);
            assert_eq!((&r.id, r.codigo_exato, r.achou.as_str()), (id, true, balao), "{texto}");
        }
        // Começo de código: acha, com o balão, sem ser exata.
        for (texto, id, balao) in [
            ("32206", &rol, "Cód. original"),
            ("789123456", &rol, "Cód. barras"),
            ("old5", &pas, "Cód. antigo"),
            ("555 old", &pas, "Cód. antigo"),
            ("dy55", &correia, "Fornecedor · Distribuidora Dayco"),
        ] {
            let r = v.primeira(texto);
            assert_eq!((&r.id, r.codigo_exato, r.achou.as_str()), (id, false, balao), "{texto}");
        }
        // Nome, marca, carro e ano (o ano do carro certo).
        assert_eq!(v.ids("rolamento"), vec![rol.clone()]);
        assert_eq!(v.primeira("rolamento").achou, "");
        assert_eq!(v.ids("skf"), vec![rol.clone()]);
        assert_eq!(v.ids("cofap"), vec![rol.clone()]);
        assert_eq!(v.ids("fras-le"), vec![pas.clone()]);
        assert_eq!(v.ids("volkswagen gol 2010"), vec![rol.clone()]);
        assert_eq!(v.ids("vw gol 2016"), vec![pas.clone()]);
        assert_eq!(v.ids("uno 2012"), vec![pas.clone()]);
        assert!(v.ids("uno 2016").is_empty(), "o Uno da pastilha é até 2014");
        assert!(v.ids("rolamento uno").is_empty());
        assert_eq!(v.ids("gol 1.6"), vec![rol.clone()]);
        // Modelo escrito junto ou separado.
        for texto in ["8pk1515", "8pk 1515", "8PK-1515", "correia 8pk", "1515 8pk"] {
            assert_eq!(v.ids(texto), vec![correia.clone()], "{texto}");
        }
        assert_eq!(v.primeira("8pk1515").achou, "");
        // Inativas só quando pedidas.
        assert_eq!(v.ids("pastilha"), vec![pas.clone()]);
        let r: Vec<String> = buscar(&v.b.conn, "pastilha", true, 50).unwrap().into_iter().map(|p| p.id).collect();
        assert_eq!(r, vec![pas.clone(), inativa.clone()]);
        // Texto vazio: as alteradas por último, até o limite.
        v.b.conn.execute("UPDATE produto SET atualizado_em = '2099-01-01T00:00:00Z' WHERE id = ?1", [&pas]).unwrap();
        let r = buscar(&v.b.conn, "", false, 2).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].id, pas);
        assert_eq!(buscar(&v.b.conn, "  ", true, 50).unwrap().len(), 4);
        // Texto sem letras nem números = vazio.
        assert_eq!(buscar(&v.b.conn, "-- .", false, 50).unwrap()[0].id, pas);
    }

    /// O leitor (e o Enter com código) só entra com o código inteiro, e
    /// nunca escolhe sozinho entre várias peças com o mesmo código.
    #[test]
    fn leitor_so_com_codigo_inteiro() {
        let c = capturas();
        let v = &c.v;
        assert_eq!(pelo_codigo_exato(&v.b.conn, "TCF-01").unwrap().len(), 1);
        assert_eq!(pelo_codigo_exato(&v.b.conn, "tcf 01").unwrap().len(), 1);
        assert!(pelo_codigo_exato(&v.b.conn, "TCF").unwrap().is_empty(), "prefixo não entra direto");
        assert!(pelo_codigo_exato(&v.b.conn, "01 TCF").unwrap().is_empty(), "ordem trocada não entra direto");
        assert!(pelo_codigo_exato(&v.b.conn, "rg08").unwrap().is_empty());
        // Outro fornecedor usa o mesmo código para outra peça: as duas, para escolher.
        let outro = v.fornecedor("Outro Fornecedor");
        let rele = c.sem_relacao[3].clone();
        v.codigo_do_fornecedor(&outro, &rele, "TCF-01");
        let achou = pelo_codigo_exato(&v.b.conn, "TCF01").unwrap();
        assert_eq!(achou.len(), 2);
        assert!(achou.iter().any(|p| p.id == c.terminal) && achou.iter().any(|p| p.id == rele));
    }
}
