//! Busca de peças: um campo só, que acha por código, código de barras,
//! código original, equivalente, nome, marca ou carro e ano.
//!
//! Exemplos: "32206", "7891234567895", "pastilha gol 1.6 2015", "fag rolamento".

use std::collections::HashMap;

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
    /// O código externo que produziu a correspondência na busca.
    pub codigo_achado: String,
    /// Código do fornecedor (quando há apenas um fornecedor ou quando a busca
    /// casou com o código de um fornecedor específico; vazio quando não há
    /// fornecedor ou quando há mais de um sem correspondência específica).
    pub codigo_fornecedor: String,
    /// Nome curto do fornecedor correspondente (ou "N fornecedores" quando há vários).
    pub fornecedor_nome: String,
    /// Quantos códigos de fornecedor estão vinculados a esta peça.
    pub fornecedores_qtd: i64,
}

/// Uma página determinística da lista de peças controlada pelo núcleo.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginaProdutos {
    pub itens: Vec<ProdutoResumo>,
    pub total: i64,
    pub total_filtrado: i64,
    pub deslocamento: i64,
    pub limite: i64,
    pub tem_mais: bool,
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

/// Quantos termos da busca contam (o resto é ignorado).
const MAX_TERMOS: usize = 10;

/// Busca peças. Com o texto vazio, mostra as alteradas por último.
///
/// Cada termo tem que casar em algum lugar da peça (nome, marca, códigos,
/// carros, similares, código do fornecedor ou código antigo), em qualquer
/// ordem: "TCF 01" e "01 TCF" acham a mesma peça. Termo de 1 ou 2 letras ou
/// números só casa no começo de uma palavra ou numa fronteira dentro dela
/// (pontuação, ou passagem de letra para número e vice-versa): "rg" acha
/// "RG-08" e "rg08", mas não "cargo" nem "02530BRG". Termo de 3 ou mais
/// também casa no meio da palavra, com prioridade menor. O código inteiro
/// (ou o começo dele) também acha: "TCF" acha a peça do código "TCF-01".
///
/// Ordem: código exato; começo de código ou todos os termos no mesmo código
/// (com 3 ou mais letras/números, ou com número); nome que começa pelo
/// primeiro termo; todos os termos no começo de palavra; casamento no meio de
/// palavra. Dentro de cada grupo, a ativa antes da inativa e por nome.
pub fn buscar(
    conn: &Connection,
    texto_busca: &str,
    incluir_inativas: bool,
    limite: i64,
) -> Resultado<Vec<ProdutoResumo>> {
    Ok(buscar_pagina(conn, texto_busca, incluir_inativas, limite, 0)?.itens)
}

/// Busca peças com paginação determinística controlada pelo núcleo.
pub fn buscar_pagina(
    conn: &Connection,
    texto_busca: &str,
    incluir_inativas: bool,
    limite: i64,
    deslocamento: i64,
) -> Resultado<PaginaProdutos> {
    let limite = limite.clamp(1, 1000) as usize;
    let deslocamento = deslocamento.max(0) as usize;
    let total_geral = total(conn, incluir_inativas)?;
    let normalizado = texto::normalizar(texto_busca);
    let tokens: Vec<&str> = normalizado.split(' ').filter(|t| !t.is_empty()).take(MAX_TERMOS).collect();
    let anos: Vec<(&str, i64)> = tokens.iter().filter_map(|t| como_ano(t).map(|a| (*t, a))).collect();
    let palavras: Vec<&str> = tokens.iter().filter(|t| como_ano(t).is_none()).copied().collect();
    let compacto = texto::compactar(texto_busca);

    if tokens.is_empty() {
        let sql = if incluir_inativas {
            "SELECT p.id FROM produto p
              ORDER BY p.atualizado_em DESC, normalizar(p.nome) ASC, p.codigo ASC, p.id ASC
              LIMIT ?1 OFFSET ?2"
        } else {
            "SELECT p.id FROM produto p
              WHERE p.ativo = 1
              ORDER BY p.atualizado_em DESC, normalizar(p.nome) ASC, p.codigo ASC, p.id ASC
              LIMIT ?1 OFFSET ?2"
        };
        let mut c = conn.prepare_cached(sql)?;
        let ids: Vec<String> =
            c.query_map([limite as i64, deslocamento as i64], |l| l.get(0))?.collect::<Result<_, _>>()?;
        let mut carros = carros_das_pecas(conn, &ids.iter().map(String::as_str).collect::<Vec<_>>())?;
        let escolhidas = ids
            .into_iter()
            .map(|id| {
                let c = carros.remove(&id).unwrap_or_default();
                (Achada::recente(id), c)
            })
            .collect();
        let itens = montar(conn, escolhidas, &compacto, &[])?;
        let tem_mais = (deslocamento + itens.len()) < (total_geral.max(0) as usize);
        return Ok(PaginaProdutos {
            itens,
            total: total_geral,
            total_filtrado: total_geral,
            deslocamento: deslocamento as i64,
            limite: limite as i64,
            tem_mais,
        });
    }

    let termos: Vec<Termo> = tokens.iter().map(|t| Termo::novo(t)).collect();
    let de_fora = codigos_de_fora(conn, &termos)?;
    let sem_codigos_de_fora: Vec<String> = Vec::new();
    let mut texto_da_peca = TextoDaPeca::default();
    let mut achadas: Vec<Achada> = Vec::new();
    for linha in candidatas(conn, &termos, &de_fora, incluir_inativas)? {
        let codigos = de_fora.por_peca.get(&linha.id).unwrap_or(&sem_codigos_de_fora);
        if let Some(a) = classificar(linha, &termos, &compacto, codigos, &mut texto_da_peca) {
            achadas.push(a);
        }
    }

    if anos.is_empty() {
        let total_filtrado = achadas.len() as i64;
        let ate = deslocamento.saturating_add(limite).min(achadas.len());
        if deslocamento >= achadas.len() {
            return Ok(PaginaProdutos {
                itens: Vec::new(),
                total: total_geral,
                total_filtrado,
                deslocamento: deslocamento as i64,
                limite: limite as i64,
                tem_mais: false,
            });
        }
        ordenar_as_primeiras(&mut achadas, ate);
        let fatia: Vec<Achada> = achadas.into_iter().skip(deslocamento).take(limite).collect();
        let mut carros = carros_das_pecas(conn, &fatia.iter().map(|a| a.id.as_str()).collect::<Vec<_>>())?;
        let escolhidas: Vec<(Achada, Vec<Carro>)> = fatia
            .into_iter()
            .map(|a| {
                let c = carros.remove(&a.id).unwrap_or_default();
                (a, c)
            })
            .collect();
        let itens = montar(conn, escolhidas, &compacto, &termos)?;
        let tem_mais = (deslocamento + itens.len()) < (total_filtrado as usize);
        return Ok(PaginaProdutos {
            itens,
            total: total_geral,
            total_filtrado,
            deslocamento: deslocamento as i64,
            limite: limite as i64,
            tem_mais,
        });
    }

    achadas.sort_unstable_by(Achada::ordem);
    let mut escolhidas = Vec::new();
    let mut correspondentes = 0usize;
    let mut fila = achadas.into_iter();
    loop {
        let bloco: Vec<Achada> = fila.by_ref().take(limite.max(100)).collect();
        if bloco.is_empty() {
            break;
        }
        let mut carros = carros_das_pecas(conn, &bloco.iter().map(|a| a.id.as_str()).collect::<Vec<_>>())?;
        for a in bloco {
            let carros = carros.remove(&a.id).unwrap_or_default();
            if !anos.iter().all(|(token, ano)| serve_no_ano(&a.busca, &carros, &palavras, token, *ano)) {
                continue;
            }
            if correspondentes >= deslocamento && escolhidas.len() < limite {
                escolhidas.push((a, carros));
            }
            correspondentes += 1;
        }
    }
    let itens = montar(conn, escolhidas, &compacto, &termos)?;
    let total_filtrado = correspondentes as i64;
    let tem_mais = (deslocamento + itens.len()) < correspondentes;
    Ok(PaginaProdutos {
        itens,
        total: total_geral,
        total_filtrado,
        deslocamento: deslocamento as i64,
        limite: limite as i64,
        tem_mais,
    })
}

/// Um termo da busca (já normalizado: minúsculas, sem acento).
///
/// Pontuação entre uma letra e um número não conta ("rg-08" = "rg08"), mas
/// entre duas letras ou dois números ela separa ("1.6" não é "16"; "3/4"
/// não é "34"), como sempre foi.
struct Termo {
    /// Só letras e números ("rg-08" → "rg08").
    compacto: String,
    /// Posições (bytes de `compacto`) onde havia pontuação entre duas letras
    /// ou dois números ("1.6" → [1]; "fras-le" → [4]).
    cortes: Vec<usize>,
    /// Filtro grosso do SQLite: os pedaços de letras e de números, em ordem
    /// ("rg-08" → "%rg%08%", "1.6" → "%1%6%"). Um pedaço nunca tem espaço nem
    /// pontuação dentro, no texto de busca, quando o termo casa.
    padrao: String,
    /// O termo é um ano de carro (filtro fino depois).
    ano: Option<i64>,
    /// Até 2 letras/números: só casa no começo da palavra ou numa fronteira.
    curto: bool,
    /// Pode atravessar o espaço entre palavras ("8pk1515" acha "8PK 1515"):
    /// 4 ou mais letras/números, com algum número.
    junta: bool,
}

impl Termo {
    fn novo(token: &str) -> Termo {
        let mut compacto = String::with_capacity(token.len());
        let mut cortes = Vec::new();
        let mut padrao = String::from("%");
        let mut anterior_numero: Option<bool> = None;
        let mut depois_de_sinal = false;
        for c in token.chars() {
            if !c.is_alphanumeric() {
                depois_de_sinal = true;
                continue;
            }
            let numero = c.is_numeric();
            match anterior_numero {
                Some(a) if a != numero => padrao.push('%'),
                Some(_) if depois_de_sinal => {
                    cortes.push(compacto.len());
                    padrao.push('%');
                }
                _ => {}
            }
            compacto.push(c);
            padrao.push(c);
            anterior_numero = Some(numero);
            depois_de_sinal = false;
        }
        padrao.push('%');
        let tamanho = compacto.chars().count();
        let junta = tamanho >= 4 && compacto.chars().any(char::is_numeric);
        Termo { ano: como_ano(token), curto: tamanho < 3, junta, compacto, cortes, padrao }
    }

    /// Está num código (compacto, sem pontuação) no começo dele ou numa
    /// passagem de letra para número (ou o contrário)? "01" e "tcf" em "tcf01".
    fn no_codigo(&self, codigo: &str) -> bool {
        self.cortes.is_empty() && no_codigo(codigo, &self.compacto)
    }
}

/// Onde um termo casou.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Lugar {
    /// No começo de uma palavra ou numa fronteira dentro dela.
    Comeco,
    /// No meio de uma palavra (termo de 3 ou mais).
    Meio,
}

/// Como um termo casou numa peça.
#[derive(Default)]
struct Casamento {
    /// O melhor lugar no texto de busca da peça (nome, marca, códigos, carros...).
    no_texto: Option<Lugar>,
    /// O melhor lugar nos códigos de fornecedor e antigos.
    nos_de_fora: Option<Lugar>,
    /// O texto da peça (o nome) começa com o termo.
    no_comeco: bool,
}

impl Casamento {
    fn lugar(&self) -> Option<Lugar> {
        match (self.no_texto, self.nos_de_fora) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

/// O texto de uma peça preparado para casar os termos: só letras e números,
/// com as palavras coladas (para "8pk1515" achar "8PK 1515"), e onde cada
/// palavra ou pedaço de palavra começa (posições em bytes de `letras`, em
/// ordem; listas curtas, para não marcar byte a byte em milhares de peças).
#[derive(Default)]
struct TextoDaPeca {
    letras: String,
    /// Onde começa cada palavra (separada por espaço). Cada código de
    /// fornecedor ou antigo conta como uma palavra.
    palavras: Vec<u32>,
    /// Onde começa cada pedaço separado por espaço ou pontuação. Também é
    /// começo a passagem de letra para número e vice-versa (`comeco`).
    pedacos: Vec<u32>,
    /// Onde havia espaço ou pontuação entre duas letras ou dois números
    /// ("1.6", "3/4", "11 14"). O termo só passa por um corte se tiver o
    /// mesmo corte.
    cortes: Vec<u32>,
    /// `letras[..fim_do_texto]` vem do texto de busca; o resto, dos códigos
    /// de fornecedor e antigos (cada um isolado, sem colar com o vizinho).
    fim_do_texto: usize,
}

impl TextoDaPeca {
    /// Prepara o texto de uma peça (reaproveita a memória da anterior).
    fn preencher(&mut self, busca: &str, de_fora: &[String]) {
        self.letras.clear();
        self.palavras.clear();
        self.pedacos.clear();
        self.cortes.clear();
        let mut ultimo_numero = None;
        for palavra in busca.split(' ') {
            self.juntar(palavra, &mut ultimo_numero);
        }
        self.fim_do_texto = self.letras.len();
        for codigo in de_fora {
            // O espaço nunca está num termo: nada casa atravessando de um código para outro.
            self.letras.push(' ');
            self.juntar(codigo, &mut None);
        }
    }

    /// Acrescenta as letras e números de uma palavra. `ultimo_numero`: o
    /// último caractere acrescentado era número?
    fn juntar(&mut self, palavra: &str, ultimo_numero: &mut Option<bool>) {
        let mut separado = true; // começo da palavra ou depois de pontuação
        let mut primeiro = true;
        for c in palavra.chars() {
            if !c.is_alphanumeric() {
                separado = true;
                continue;
            }
            let numero = c.is_numeric();
            let i = self.letras.len() as u32;
            if primeiro {
                self.palavras.push(i);
                primeiro = false;
            }
            if separado {
                self.pedacos.push(i);
                if *ultimo_numero == Some(numero) {
                    self.cortes.push(i);
                }
            }
            self.letras.push(c);
            *ultimo_numero = Some(numero);
            separado = false;
        }
    }

    /// Começa ali uma palavra ou um pedaço dela?
    fn comeco(&self, i: usize) -> bool {
        if self.pedacos.binary_search(&(i as u32)).is_ok() {
            return true;
        }
        match (self.letras[..i].chars().next_back(), self.letras[i..].chars().next()) {
            (Some(antes), Some(c)) => antes != ' ' && antes.is_numeric() != c.is_numeric(),
            _ => true,
        }
    }

    fn casar(&self, termo: &Termo) -> Casamento {
        let mut r = Casamento::default();
        for i in posicoes(&self.letras, &termo.compacto) {
            let fim = i + termo.compacto.len();
            // Os cortes do texto dentro do trecho têm que ser os do termo.
            let de = self.cortes.partition_point(|&c| c as usize <= i);
            let ate = self.cortes.partition_point(|&c| (c as usize) < fim);
            let no_trecho = &self.cortes[de..ate];
            if no_trecho.len() != termo.cortes.len()
                || !no_trecho.iter().zip(&termo.cortes).all(|(&c, &o)| c as usize - i == o)
            {
                continue;
            }
            let primeira_palavra = self.palavras.partition_point(|&p| p as usize <= i);
            let atravessa = self.palavras.get(primeira_palavra).is_some_and(|&p| (p as usize) < fim);
            let comeco = self.comeco(i);
            let lugar = if atravessa {
                // Atravessa o espaço entre palavras: só o modelo escrito junto.
                (termo.junta && comeco).then_some(Lugar::Comeco)
            } else if comeco {
                Some(Lugar::Comeco)
            } else if !termo.curto {
                Some(Lugar::Meio)
            } else {
                None
            };
            let Some(lugar) = lugar else { continue };
            let alvo = if i < self.fim_do_texto { &mut r.no_texto } else { &mut r.nos_de_fora };
            *alvo = Some(alvo.map_or(lugar, |a| a.min(lugar)));
            r.no_comeco |= i == 0 && lugar == Lugar::Comeco && i < self.fim_do_texto;
        }
        r
    }
}

/// Todas as posições (em bytes) onde `termo` aparece em `texto`, inclusive
/// as que se sobrepõem.
fn posicoes<'a>(texto: &'a str, termo: &'a str) -> impl Iterator<Item = usize> + 'a {
    let mut desde = 0;
    std::iter::from_fn(move || {
        if termo.is_empty() || desde > texto.len() {
            return None;
        }
        let i = desde + texto[desde..].find(termo)?;
        desde = i + texto[i..].chars().next().map_or(1, char::len_utf8);
        Some(i)
    })
}

/// O termo está no código (compacto) começando no começo dele ou numa
/// passagem de letra para número (ou o contrário)? "01" e "tcf" em "tcf01".
fn no_codigo(codigo: &str, termo: &str) -> bool {
    posicoes(codigo, termo).any(|i| match codigo[..i].chars().next_back() {
        None => true,
        Some(antes) => codigo[i..].chars().next().is_some_and(|c| c.is_numeric() != antes.is_numeric()),
    })
}

/// Os códigos de fornecedor e antigos que contêm algum termo.
struct CodigosDeFora {
    /// Por peça, os códigos (compactos).
    por_peca: HashMap<String, Vec<String>>,
    /// Por termo (na ordem dos termos), as peças com um código que o contém.
    por_termo: Vec<Vec<String>>,
}

fn codigos_de_fora(conn: &Connection, termos: &[Termo]) -> Resultado<CodigosDeFora> {
    // Um termo que contém outro já é coberto por ele.
    let mut padroes: Vec<&str> = Vec::new();
    let mut por_tamanho: Vec<&str> = termos.iter().map(|t| t.compacto.as_str()).collect();
    por_tamanho.sort_by_key(|c| c.len());
    for c in por_tamanho {
        if !padroes.iter().any(|p| c.contains(p)) {
            padroes.push(c);
        }
    }
    let condicao: Vec<String> = (1..=padroes.len()).map(|i| format!("codigo_compacto LIKE ?{i}")).collect();
    let condicao = condicao.join(" OR ");
    let condicao_gtin: Vec<String> = (1..=padroes.len()).map(|i| format!("lower(gtin) LIKE ?{i}")).collect();
    let condicao_gtin = condicao_gtin.join(" OR ");
    let sql = format!(
        "SELECT produto_id, codigo_compacto FROM produto_fornecedor WHERE {condicao}
          UNION ALL
         SELECT produto_id, codigo_compacto FROM produto_codigo_antigo WHERE {condicao}
          UNION ALL
         SELECT produto_id, lower(gtin) FROM produto_gtin WHERE {condicao_gtin}"
    );
    let valores: Vec<String> = padroes.iter().map(|p| format!("%{p}%")).collect();
    let mut c = conn.prepare_cached(&sql)?;
    let mut por_peca: HashMap<String, Vec<String>> = HashMap::new();
    let mut por_termo: Vec<Vec<String>> = vec![Vec::new(); termos.len()];
    let mut linhas = c.query(params_from_iter(valores.iter()))?;
    while let Some(l) = linhas.next()? {
        let id: String = l.get(0)?;
        let codigo: String = l.get(1)?;
        for (termo, ids) in termos.iter().zip(por_termo.iter_mut()) {
            if codigo.contains(termo.compacto.as_str()) && ids.last() != Some(&id) {
                ids.push(id.clone());
            }
        }
        let lista = por_peca.entry(id).or_default();
        if !lista.contains(&codigo) {
            lista.push(codigo);
        }
    }
    Ok(CodigosDeFora { por_peca, por_termo })
}

/// Uma peça que passou no filtro grosso do SQLite.
struct Linha {
    id: String,
    nome: String,
    ativo: bool,
    busca: String,
    codigos: String,
}

/// O filtro grosso: os pedaços de cada termo aparecem em ordem no texto de
/// busca (ou o ano está num carro da peça), ou a peça tem um código de
/// fornecedor ou antigo que contém algum termo.
/// O filtro fino (onde cada termo casou) é o `classificar`.
fn candidatas(
    conn: &Connection,
    termos: &[Termo],
    de_fora: &CodigosDeFora,
    incluir_inativas: bool,
) -> Resultado<Vec<Linha>> {
    let mut valores: Vec<Value> = Vec::new();
    // O termo mais comprido primeiro: é o que mais elimina.
    let mut ordem: Vec<(&Termo, &Vec<String>)> = termos.iter().zip(&de_fora.por_termo).collect();
    ordem.sort_by_key(|(t, _)| std::cmp::Reverse(t.compacto.len()));
    let mut grupo: Vec<String> = Vec::new();
    for (termo, ids_de_fora) in ordem {
        valores.push(Value::Text(termo.padrao.clone()));
        let mut condicao = format!("p.busca LIKE ?{}", valores.len());
        if let Some(ano) = termo.ano {
            // Filtro grosso aqui; o fino (ano do carro certo) é feito depois.
            valores.push(Value::Integer(ano));
            condicao.push_str(&format!(
                " OR EXISTS (SELECT 1 FROM produto_aplicacao a WHERE a.produto_id = p.id
                     AND COALESCE(a.ano_inicio, 0) <= ?{n} AND COALESCE(a.ano_fim, 9999) >= ?{n})",
                n = valores.len()
            ));
        }
        if !ids_de_fora.is_empty() {
            // Ou está num código de fornecedor ou antigo da peça.
            valores.push(Value::Text(serde_json::to_string(ids_de_fora)?));
            condicao.push_str(&format!(" OR p.id IN (SELECT value FROM json_each(?{}))", valores.len()));
        }
        grupo.push(format!("({condicao})"));
    }
    let onde = grupo.join(" AND ");
    let sql = format!(
        "SELECT p.id, p.nome, p.ativo, p.busca, p.codigos
           FROM produto p
          WHERE {}{onde}",
        if incluir_inativas { "" } else { "p.ativo = 1 AND " }
    );
    let mut c = conn.prepare(&sql)?;
    let linhas = c.query_map(params_from_iter(valores), |l| {
        Ok(Linha { id: l.get(0)?, nome: l.get(1)?, ativo: l.get(2)?, busca: l.get(3)?, codigos: l.get(4)? })
    })?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

/// Uma peça achada, com o grupo da ordem.
struct Achada {
    id: String,
    busca: String,
    ativo: bool,
    /// Para ordenar (sem diferença de maiúsculas no ASCII, como o `COLLATE NOCASE`).
    nome: String,
    /// 0 código exato; 1 começo de código ou todos os termos no mesmo
    /// código (3 ou mais letras/números, ou com número); 2 nome começa pelo
    /// primeiro termo; 3 todos os termos no começo de palavra (ou começo de
    /// código com 1 ou 2 letras); 4 algum termo no meio de palavra.
    nivel: u8,
    /// Achou por um código de fornecedor ou antigo (que não está no texto de
    /// busca da peça): mostra o balão mesmo sem cara de código.
    por_codigo_de_fora: bool,
    /// Termos (pela posição) que só casaram num código de fornecedor ou antigo.
    termos_de_fora: Vec<usize>,
    /// Algum código da peça daria o balão (`achou_por`): sem isso, nem procura.
    tem_balao: bool,
}

impl Achada {
    fn recente(id: String) -> Achada {
        Achada {
            id,
            busca: String::new(),
            ativo: true,
            nome: String::new(),
            nivel: 5,
            por_codigo_de_fora: false,
            termos_de_fora: Vec::new(),
            tem_balao: false,
        }
    }

    fn ordem(&self, outra: &Achada) -> std::cmp::Ordering {
        (self.nivel, !self.ativo)
            .cmp(&(outra.nivel, !outra.ativo))
            .then_with(|| {
                let (a, b) = (self.nome.bytes(), outra.nome.bytes());
                a.map(|x| x.to_ascii_lowercase()).cmp(b.map(|x| x.to_ascii_lowercase()))
            })
            .then_with(|| self.id.cmp(&outra.id))
    }
}

/// O filtro fino: onde cada termo casou, e em que grupo da ordem a peça fica.
/// None = não serve (algum termo não casou, e o texto inteiro não é o começo
/// de um código dela).
fn classificar(
    l: Linha,
    termos: &[Termo],
    compacto: &str,
    de_fora: &[String],
    texto_da_peca: &mut TextoDaPeca,
) -> Option<Achada> {
    texto_da_peca.preencher(&l.busca, de_fora);
    let casamentos: Vec<Casamento> = termos.iter().map(|t| texto_da_peca.casar(t)).collect();
    // Ano que não está no texto: casou pelo carro (o filtro fino confere).
    let lugar = |k: usize| casamentos[k].lugar().or(termos[k].ano.map(|_| Lugar::Comeco));

    let dentro = || l.codigos.split(' ').filter(|c| !c.is_empty());
    let fora = || de_fora.iter().map(String::as_str);
    let mesmo_codigo = |c: &str| termos.len() > 1 && termos.iter().all(|t| t.no_codigo(c));
    let exato_dentro = dentro().any(|c| c == compacto);
    let exato_fora = fora().any(|c| c == compacto);
    let codigo_dentro = dentro().any(|c| c.starts_with(compacto) || mesmo_codigo(c));
    let codigo_fora = fora().any(|c| c.starts_with(compacto) || mesmo_codigo(c));

    // Começo de código só passa à frente do nome com 3 ou mais letras/números
    // (ou com número): com "pa", as "PASTILHA..." vêm antes dos códigos "PA...".
    let codigo_forte = compacto.chars().count() >= 3 || compacto.chars().any(char::is_numeric);
    let termos_casaram = (0..termos.len()).all(|k| lugar(k).is_some());

    let (nivel, pelo_codigo_de_fora) = if exato_dentro || exato_fora {
        (0, !exato_dentro)
    } else if (codigo_dentro || codigo_fora) && (codigo_forte || !termos_casaram) {
        (if codigo_forte { 1 } else { 3 }, !codigo_dentro)
    } else if !termos_casaram {
        return None;
    } else if (0..termos.len()).all(|k| lugar(k) == Some(Lugar::Comeco)) {
        (if casamentos[0].no_comeco { 2 } else { 3 }, false)
    } else {
        (4, false)
    };
    let termos_de_fora: Vec<usize> = casamentos
        .iter()
        .enumerate()
        .filter(|(_, c)| c.no_texto.is_none() && c.nos_de_fora.is_some())
        .map(|(i, _)| i)
        .collect();
    // As mesmas regras do `achou_por`, nos mesmos códigos (os da peça estão
    // em `codigos`; os de fornecedor e antigos que importam, em `de_fora`).
    let balao = |c: &str| {
        c.starts_with(compacto)
            || termos.iter().all(|t| t.no_codigo(c))
            || termos_de_fora.iter().any(|i| c.contains(termos[*i].compacto.as_str()))
    };
    let tem_balao = dentro().any(balao) || fora().any(balao);
    Some(Achada {
        nome: l.nome,
        id: l.id,
        busca: l.busca,
        ativo: l.ativo,
        nivel,
        por_codigo_de_fora: pelo_codigo_de_fora || !termos_de_fora.is_empty(),
        termos_de_fora,
        tem_balao,
    })
}

/// Deixa em ordem só as `quantas` primeiras (o resto é descartado).
fn ordenar_as_primeiras(achadas: &mut Vec<Achada>, quantas: usize) {
    if quantas == 0 {
        achadas.clear();
        return;
    }
    if achadas.len() > quantas {
        achadas.select_nth_unstable_by(quantas - 1, Achada::ordem);
        achadas.truncate(quantas);
    }
    achadas.sort_unstable_by(Achada::ordem);
}

/// As linhas da lista, na ordem das escolhidas.
fn montar(
    conn: &Connection,
    escolhidas: Vec<(Achada, Vec<Carro>)>,
    compacto: &str,
    termos: &[Termo],
) -> Resultado<Vec<ProdutoResumo>> {
    if escolhidas.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<&str> = escolhidas.iter().map(|(a, _)| a.id.as_str()).collect();
    let sql = format!(
        "SELECT p.id, p.codigo, p.nome, p.marca, p.unidade, p.codigo_original, p.local, p.estoque_minimo, p.ativo,
                {},
                (SELECT COALESCE(SUM(quantidade), 0) FROM estoque_movimento WHERE produto_id = p.id),
                p.codigo_barras, {}, p.tipo = 'servico', (p.codigo_validado = 0 AND p.codigo <> '')
           FROM produto p
          WHERE p.id IN (SELECT value FROM json_each(?1))",
        crate::produtos::SQL_PRECO_1,
        crate::produtos::SQL_EM_OFERTA
    );
    let mut c = conn.prepare_cached(&sql)?;
    let linhas = c.query_map([serde_json::to_string(&ids)?], |l| {
        let estoque_minimo: i64 = l.get(7)?;
        let preco: Option<i64> = l.get(9)?;
        let saldo: i64 = l.get(10)?;
        let servico: bool = l.get(13)?;
        let id: String = l.get(0)?;
        let item = ProdutoResumo {
            id: id.clone(),
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
            oferta: l.get(12)?,
            servico,
            aplicacoes: String::new(),
            codigo_exato: false,
            achou: String::new(),
            codigo_achado: String::new(),
            codigo_fornecedor: String::new(),
            fornecedor_nome: String::new(),
            fornecedores_qtd: 0,
        };
        let codigo_barras: String = l.get(11)?;
        // Código com letras que ainda espera a confirmação do caixa que
        // distribui as faixas: o leitor não acha a peça por ele.
        let esperando: bool = l.get(14)?;
        Ok((id, (item, codigo_barras, esperando)))
    })?;
    let mut detalhes: HashMap<String, (ProdutoResumo, String, bool)> = linhas.collect::<Result<_, _>>()?;
    let mut fornecedores_por_peca = fornecedores_das_pecas(conn, &ids)?;

    let parece_codigo = compacto.len() >= 3 && compacto.chars().any(|c| c.is_ascii_digit());
    let mut resultado = Vec::with_capacity(escolhidas.len());
    for (a, carros) in escolhidas {
        let Some((mut item, codigo_barras, esperando)) = detalhes.remove(&a.id) else { continue };
        let fornecedores = fornecedores_por_peca.remove(&a.id).unwrap_or_default();
        item.fornecedores_qtd = fornecedores.len() as i64;
        let exato_pelo_interno_esperando = esperando && texto::compactar(&item.codigo) == compacto;
        item.codigo_exato = a.nivel == 0 && !compacto.is_empty() && !exato_pelo_interno_esperando;
        item.aplicacoes = resumo(&carros);
        let mut fornecedor_achado: Option<(String, String)> = None;
        if (parece_codigo || a.por_codigo_de_fora) && a.tem_balao {
            let de_fora: Vec<&Termo> = a.termos_de_fora.iter().filter_map(|i| termos.get(*i)).collect();
            let (achou, codigo_achado, forn) =
                achou_por(conn, &item, &codigo_barras, &fornecedores, compacto, termos, &de_fora)?;
            item.achou = achou;
            item.codigo_achado = codigo_achado;
            fornecedor_achado = forn;
        }
        if let Some((cod_forn, nome_forn)) = fornecedor_achado {
            item.codigo_fornecedor = cod_forn;
            item.fornecedor_nome = nome_forn;
        } else if fornecedores.len() == 1 {
            item.codigo_fornecedor = fornecedores[0].codigo.clone();
            item.fornecedor_nome = fornecedores[0].curto.clone();
        } else if fornecedores.len() > 1 {
            item.codigo_fornecedor = String::new();
            item.fornecedor_nome = format!("{} fornecedores", fornecedores.len());
        }
        resultado.push(item);
    }
    Ok(resultado)
}

#[derive(Clone)]
struct FornecedorResumo {
    codigo_compacto: String,
    codigo: String,
    curto: String,
}

fn fornecedores_das_pecas(conn: &Connection, ids: &[&str]) -> Resultado<HashMap<String, Vec<FornecedorResumo>>> {
    let mut por_peca: HashMap<String, Vec<FornecedorResumo>> = HashMap::new();
    if ids.is_empty() {
        return Ok(por_peca);
    }
    let mut c = conn.prepare_cached(
        "SELECT f.produto_id, f.codigo_compacto, f.codigo, COALESCE(NULLIF(p.fantasia, ''), p.nome)
           FROM produto_fornecedor f JOIN pessoa p ON p.id = f.fornecedor_id
          WHERE f.produto_id IN (SELECT value FROM json_each(?1))
          ORDER BY f.produto_id, f.atualizado_em DESC, f.codigo ASC",
    )?;
    let linhas = c.query_map([serde_json::to_string(ids)?], |l| {
        let produto_id: String = l.get(0)?;
        let codigo_compacto: String = l.get(1)?;
        let codigo: String = l.get(2)?;
        let nome: String = l.get(3)?;
        let curto: String = nome.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
        Ok((produto_id, FornecedorResumo { codigo_compacto, codigo, curto }))
    })?;
    for linha in linhas {
        let (produto_id, f) = linha?;
        por_peca.entry(produto_id).or_default().push(f);
    }
    Ok(por_peca)
}

/// Por qual código a peça foi achada, para o balão: o código igual ao
/// digitado; senão, o que começa com o digitado; senão, o que tem todos os
/// termos ("01 TCF" → "TCF-01"); senão, o código de fornecedor ou antigo que
/// tem um termo que não está no resto da peça ("terminal tcf").
fn achou_por(
    conn: &Connection,
    item: &ProdutoResumo,
    codigo_barras: &str,
    fornecedores: &[FornecedorResumo],
    compacto: &str,
    termos: &[Termo],
    termos_de_fora: &[&Termo],
) -> Resultado<(String, String, Option<(String, String)>)> {
    struct Candidato {
        compacto: String,
        rotulo: String,
        codigo_exibido: String,
        fornecedor: Option<(String, String)>,
    }
    let mut candidatos: Vec<Candidato> = vec![
        Candidato {
            compacto: texto::compactar(&item.codigo),
            rotulo: "Cód. interno".to_string(),
            codigo_exibido: item.codigo.clone(),
            fornecedor: None,
        },
        Candidato {
            compacto: texto::compactar(codigo_barras),
            rotulo: "Cód. barras".to_string(),
            codigo_exibido: codigo_barras.to_string(),
            fornecedor: None,
        },
        Candidato {
            compacto: texto::compactar(&item.codigo_original),
            rotulo: "Cód. original".to_string(),
            codigo_exibido: item.codigo_original.clone(),
            fornecedor: None,
        },
    ];
    let mut c_gtin =
        conn.prepare_cached("SELECT gtin FROM produto_gtin WHERE produto_id = ?1 ORDER BY ordem, rowid")?;
    for linha in c_gtin.query_map([&item.id], |l| l.get::<_, String>(0))? {
        let g = linha?;
        let comp = texto::compactar(&g);
        if !comp.is_empty() && !candidatos.iter().any(|x| x.compacto == comp && x.rotulo == "Cód. barras") {
            candidatos.push(Candidato {
                compacto: comp,
                rotulo: "Cód. barras".to_string(),
                codigo_exibido: g,
                fornecedor: None,
            });
        }
    }
    let mut c = conn.prepare_cached(
        "SELECT codigo_compacto, codigo, marca FROM produto_equivalente WHERE produto_id = ?1 ORDER BY ordem",
    )?;
    for linha in
        c.query_map([&item.id], |l| Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?, l.get::<_, String>(2)?)))?
    {
        let (comp, codigo, marca) = linha?;
        let nome = if marca.trim().is_empty() { "Similar".to_string() } else { format!("Similar · {}", marca.trim()) };
        candidatos.push(Candidato { compacto: comp, rotulo: nome, codigo_exibido: codigo, fornecedor: None });
    }
    for f in fornecedores {
        candidatos.push(Candidato {
            compacto: f.codigo_compacto.clone(),
            rotulo: format!("Fornecedor · {}", f.curto),
            codigo_exibido: f.codigo.clone(),
            fornecedor: Some((f.codigo.clone(), f.curto.clone())),
        });
    }
    let mut c =
        conn.prepare_cached("SELECT codigo_compacto, codigo FROM produto_codigo_antigo WHERE produto_id = ?1")?;
    for linha in c.query_map([&item.id], |l| Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?)))? {
        let (comp, codigo) = linha?;
        candidatos.push(Candidato {
            compacto: comp,
            rotulo: "Cód. antigo".to_string(),
            codigo_exibido: codigo,
            fornecedor: None,
        });
    }
    let validos = || candidatos.iter().filter(|cand| !cand.compacto.is_empty());
    let igual = validos().find(|cand| cand.compacto == compacto);
    let comeca = || validos().find(|cand| cand.compacto.starts_with(compacto));
    let todos = || validos().find(|cand| !termos.is_empty() && termos.iter().all(|t| t.no_codigo(&cand.compacto)));
    let de_fora = || validos().find(|cand| termos_de_fora.iter().any(|t| cand.compacto.contains(t.compacto.as_str())));
    let achado = igual.or_else(comeca).or_else(todos).or_else(de_fora);
    Ok(match achado {
        Some(cand) => (cand.rotulo.clone(), cand.codigo_exibido.clone(), cand.fornecedor.clone()),
        None => (String::new(), String::new(), None),
    })
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

/// Os carros de várias peças de uma vez, por peça (na ordem da ficha).
fn carros_das_pecas(conn: &Connection, ids: &[&str]) -> Resultado<HashMap<String, Vec<Carro>>> {
    let mut por_peca: HashMap<String, Vec<Carro>> = HashMap::new();
    if ids.is_empty() {
        return Ok(por_peca);
    }
    let mut consulta = conn.prepare_cached(
        "SELECT produto_id, montadora, modelo, motor, observacao, ano_inicio, ano_fim FROM produto_aplicacao
          WHERE produto_id IN (SELECT value FROM json_each(?1)) ORDER BY produto_id, ordem",
    )?;
    let linhas = consulta.query_map([serde_json::to_string(ids)?], |l| {
        let produto_id: String = l.get(0)?;
        let montadora: String = l.get(1)?;
        let modelo: String = l.get(2)?;
        let motor: String = l.get(3)?;
        let observacao: String = l.get(4)?;
        let ano_inicio: Option<i64> = l.get(5)?;
        let ano_fim: Option<i64> = l.get(6)?;
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
        let carro = Carro {
            texto: texto::normalizar(&format!("{montadora} {sinonimos} {modelo} {motor} {observacao}")),
            resumo,
            ano_inicio,
            ano_fim,
        };
        Ok((produto_id, carro))
    })?;
    for linha in linhas {
        let (produto_id, carro) = linha?;
        por_peca.entry(produto_id).or_default().push(carro);
    }
    Ok(por_peca)
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

// ------------------------------------------------------------- comandos das telas

pub use comandos::COMANDOS;

/// Os comandos da busca de peças e do leitor de código de barras (etapa 3 da modularização: vieram do `match` do
/// `api.rs` sem mudar nome, entrada, resposta nem permissão).
mod comandos {
    use serde_json::{json, Value};

    use crate::busca;
    use crate::despacho::{ler, BuscaProdutos, Classe, Comando, Pedido};
    use crate::erro::Resultado;

    pub const COMANDOS: &[Comando] = &[
        Comando { nome: "produtos_buscar", classe: Classe::Leitura, funcao: produtos_buscar },
        Comando { nome: "produto_por_codigo", classe: Classe::Leitura, funcao: produto_por_codigo },
    ];

    fn produtos_buscar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: BuscaProdutos = ler(dados)?;
        let pagina =
            busca::buscar_pagina(p.conn, &d.texto, d.inativas, d.limite.unwrap_or(200), d.deslocamento.unwrap_or(0))?;
        Ok(serde_json::to_value(pagina)?)
    }

    fn produto_por_codigo(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        // Leitor de código de barras: só o código exato, e só se for
        // de uma peça. Duas ou mais com o mesmo código: a tela mostra
        // as peças e a pessoa escolhe (nunca entra uma sozinha).
        let d: BuscaProdutos = ler(dados)?;
        let lista = busca::pelo_codigo_exato(p.conn, &d.texto)?;
        // Peças com este código que ainda esperam a confirmação do
        // caixa que distribui as faixas: o leitor não as acha.
        let esperando = busca::esperando_pelo_codigo(p.conn, &d.texto)?;
        Ok(match lista.as_slice() {
            [p] => json!({ "id": p.id, "repetidos": 1, "esperando": esperando }),
            [] => json!({ "id": null, "repetidos": 0, "esperando": esperando }),
            varias => {
                json!({ "id": null, "repetidos": varias.len(), "opcoes": varias, "esperando": esperando })
            }
        })
    }
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
            buscar(&self.b.conn, texto, false, 50)
                .unwrap()
                .into_iter()
                .next()
                .unwrap_or_else(|| panic!("{texto}: nada"))
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
        assert_eq!(
            c.v.nomes("ar"),
            ["TECFIL AS836 AR FORD CARGO 1114 / 1117 - TEG129620A", "VALVULA REGULADORA DE AR"]
        );
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
        for texto in ["terminal tcf", "tcf terminal", "terminal 01 tcf", "unf tcf-01", "valvula rg", "reguladora 0815"]
        {
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

        // Com 1 ou 2 letras, o começo de um código não passa à frente do nome
        // (quem digita "pa" quer ver as pastilhas); com 3 ou com número, passa.
        let pastilha = v.peca("PASTILHA DIANTEIRA", |_| {});
        let disco = v.peca("DISCO DE FREIO", |f| f.codigo_original = "PA6264".into());
        assert_eq!(v.ids("pa"), vec![pastilha.clone(), disco.clone()]);
        assert_eq!(v.ids("pa6"), vec![disco.clone()]);
        assert_eq!(v.ids("pa62"), vec![disco.clone()]);
        assert_eq!(v.primeira("pa62").achou, "Cód. original");
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

    /// O que NÃO pode aparecer, e a pontuação: entre letra e número ela não
    /// conta ("rg-08" = "rg08"); entre dois números ou duas letras ela separa
    /// ("1.6" não é "16", "3/4" não é "34"), como sempre foi.
    #[test]
    fn casos_negativos_e_pontuacao() {
        let v = Vitrine::nova();
        let anel = v.peca("ANEL CARGO 08 VEDACAO", |_| {});
        let junta = v.peca("JUNTA CABECOTE MOTOR 1.6", |f| f.codigo_original = "JC-3344".into());
        let valvula = v.peca("VALVULA ADMISSAO 16V", |_| {});
        let terminal = v.peca("TERMINAL 3/4 UNF", |_| {});
        let filtro = v.peca("FILTRO TECFIL PSL55", |_| {});

        // "rg08" e "go08" não começam no meio de "cargo" para atravessar até o "08".
        assert!(v.ids("rg08").is_empty(), "{:?}", v.nomes("rg08"));
        assert!(v.ids("go08").is_empty(), "{:?}", v.nomes("go08"));
        assert_eq!(v.ids("cargo08"), vec![anel.clone()], "escrito junto, começando na palavra");
        assert_eq!(v.ids("08"), vec![anel.clone()]);
        // "tcf" não é "tecfil".
        assert!(v.ids("tcf").is_empty(), "{:?}", v.nomes("tcf"));
        assert_eq!(v.ids("tecf"), vec![filtro.clone()]);
        assert_eq!(v.ids("psl"), vec![filtro.clone()]);
        assert_eq!(v.ids("55"), vec![filtro.clone()], "fronteira letra→número em PSL55");
        // Dois números separados por ponto ou barra: o termo precisa da mesma separação.
        assert_eq!(v.ids("1.6"), vec![junta.clone()]);
        assert_eq!(v.ids("16"), vec![valvula.clone()], "{:?}", v.nomes("16"));
        assert_eq!(v.ids("3/4"), vec![terminal.clone()]);
        assert!(v.ids("34").is_empty(), "{:?}", v.nomes("34"));
        assert_eq!(v.ids("unf 3/4"), vec![terminal.clone()]);
        // Código com letras e números: inteiro, em pedaços, em qualquer ordem.
        for texto in ["jc3344", "JC-3344", "jc 3344", "3344 jc", "33", "motor jc"] {
            assert_eq!(v.ids(texto), vec![junta.clone()], "{texto}: {:?}", v.nomes(texto));
        }
        assert!(v.primeira("jc3344").codigo_exato);
        assert_eq!(v.primeira("3344 jc").achou, "Cód. original");
        // "44" está no meio de "3344": não casa.
        assert!(v.ids("44").is_empty(), "{:?}", v.nomes("44"));
        // Todos os termos têm que casar.
        assert!(v.ids("junta 16v").is_empty());
        assert!(v.ids("anel tecfil").is_empty());
        // Ano sem carro cadastrado não acha.
        assert!(v.ids("junta 2010").is_empty());
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

    /// 0.9.18: paginação real determinística, sem repetir nem omitir peças
    /// entre páginas, busca por descrição/observações e múltiplos GTINs, e
    /// regras de exibição de código do fornecedor (nunca escolhe fornecedor
    /// arbitrário quando há vários; mostra o que correspondeu à busca).
    #[test]
    fn paginacao_deterministica_descricao_multiplos_gtins_e_fornecedor() {
        let v = Vitrine::nova();
        let forn2 = v.fornecedor("AUTO PECAS SUL LTDA");
        let mut criadas = Vec::new();
        for i in 1..=7 {
            let id = v.peca(&format!("PECA TESTE PAGINACAO {i:02}"), |f| {
                f.categoria = "Suspensão".into();
                f.observacoes = format!(" Aplicação especial eixo traseiro lotex{i:02} ");
            });
            criadas.push(id);
        }
        // Peça 0 tem 1 fornecedor: mostra o código dele na listagem.
        v.codigo_do_fornecedor(&v.hidraflux, &criadas[0], "HID-101");
        // Peça 1 tem 2 fornecedores: sem busca por código de fornecedor, não escolhe um arbitrariamente.
        v.codigo_do_fornecedor(&v.hidraflux, &criadas[1], "HID-202");
        v.codigo_do_fornecedor(&forn2, &criadas[1], "SUL-909");

        // Múltiplos GTINs (comercial e tributável) na peça 2.
        produtos::registrar_gtins_xml(
            &v.b.conn,
            &criadas[2],
            &[
                produtos::GtinFicha { gtin: "7891234567895".into(), tipo: "comercial".into(), origem: "cEAN".into() },
                produtos::GtinFicha {
                    gtin: "17891234567892".into(),
                    tipo: "tributavel".into(),
                    origem: "cEANTrib".into(),
                },
            ],
        )
        .unwrap();

        // Busca por descrição (observacoes) e por categoria.
        assert_eq!(v.ids("lotex04"), vec![criadas[3].clone()]);
        assert_eq!(v.ids("suspensao").len(), 7);

        // Busca pelos dois GTINs da mesma peça.
        let r_com = v.primeira("7891234567895");
        assert_eq!(r_com.id, criadas[2]);
        assert!(r_com.codigo_exato);
        assert_eq!(r_com.achou, "Cód. barras");
        assert_eq!(r_com.codigo_achado, "7891234567895");

        let r_trib = v.primeira("17891234567892");
        assert_eq!(r_trib.id, criadas[2]);
        assert!(r_trib.codigo_exato);
        assert_eq!(r_trib.achou, "Cód. barras");
        assert_eq!(r_trib.codigo_achado, "17891234567892");

        // Listagem sem busca por fornecedor: peça com 1 fornecedor traz o código;
        // peça com 2 fornecedores não escolhe um principal arbitrariamente;
        // peça sem fornecedor traz vazio (nunca o código interno).
        let todas = buscar_pagina(&v.b.conn, "paginacao", false, 10, 0).unwrap();
        let p0 = todas.itens.iter().find(|p| p.id == criadas[0]).unwrap();
        assert_eq!(p0.codigo_fornecedor, "HID-101");
        assert_eq!(p0.fornecedor_nome, "HIDRAFLUX INDUSTRIAL");
        let p1 = todas.itens.iter().find(|p| p.id == criadas[1]).unwrap();
        assert_eq!(
            p1.codigo_fornecedor, "",
            "múltiplos fornecedores sem busca específica: não escolhe arbitrariamente"
        );
        assert_eq!(p1.fornecedores_qtd, 2);
        let p2 = todas.itens.iter().find(|p| p.id == criadas[2]).unwrap();
        assert_eq!(p2.codigo_fornecedor, "", "sem fornecedor: nunca exibe código interno como fornecedor");

        // Ao pesquisar pelo código de um dos fornecedores da peça 1, mostra exatamente o fornecedor e o código que casaram.
        let r_sul = v.primeira("SUL-909");
        assert_eq!(r_sul.id, criadas[1]);
        assert_eq!(r_sul.achou, "Fornecedor · AUTO PECAS");
        assert_eq!(r_sul.codigo_achado, "SUL-909");
        assert_eq!(r_sul.codigo_fornecedor, "SUL-909");
        assert_eq!(r_sul.fornecedor_nome, "AUTO PECAS");

        // Paginação real controlada pelo núcleo (com termo e sem termo):
        for consulta in ["paginacao", ""] {
            let completa = buscar_pagina(&v.b.conn, consulta, false, 100, 0).unwrap();
            assert_eq!(completa.total_filtrado, 7);
            let pag1 = buscar_pagina(&v.b.conn, consulta, false, 3, 0).unwrap();
            let pag2 = buscar_pagina(&v.b.conn, consulta, false, 3, 3).unwrap();
            let pag3 = buscar_pagina(&v.b.conn, consulta, false, 3, 6).unwrap();
            assert_eq!(pag1.itens.len(), 3);
            assert!(pag1.tem_mais);
            assert_eq!(pag2.itens.len(), 3);
            assert!(pag2.tem_mais);
            assert_eq!(pag3.itens.len(), 1);
            assert!(!pag3.tem_mais);
            let juntos: Vec<String> =
                pag1.itens.into_iter().chain(pag2.itens).chain(pag3.itens).map(|p| p.id).collect();
            let esperados: Vec<String> = completa.itens.into_iter().map(|p| p.id).collect();
            assert_eq!(juntos, esperados, "páginas não repetem nem omitem produtos ({consulta:?})");
        }
    }
}

#[cfg(test)]
mod testes_regressao {
    //! Consultas que NÃO devem mudar de resultado. As respostas esperadas foram geradas pela busca da
    //! versão 0.9.17 (commit 67d006f) com este mesmo cadastro, e conferidas iguais na busca desta versão.
    use super::*;
    use crate::banco::Banco;
    use crate::{produtos, usuarios};

    fn cadastrar(c: &rusqlite::Connection, d: &str, u: &str, codigo: &str, nome: &str, marca: &str, original: &str) {
        let mut f = produtos::ficha_nova(c).unwrap();
        f.codigo = codigo.into();
        f.nome = nome.into();
        f.marca = marca.into();
        f.codigo_original = original.into();
        f.custo = "20".into();
        produtos::salvar(c, d, u, &f).unwrap();
    }

    #[test]
    fn consultas_que_nao_mudam() {
        let b = Banco::em_memoria().unwrap();
        let c = &b.conn;
        let d = &b.dispositivo;
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        cadastrar(c, d, &u, "PF-100", "Pastilha de freio dianteira", "Fras-le", "GDB1330");
        cadastrar(c, d, &u, "FO-200", "Filtro de óleo", "Mahle", "OC-263");
        cadastrar(c, d, &u, "OL-300", "Óleo 5W30 sintético", "Lubrax", "");
        cadastrar(c, d, &u, "CD-400", "Correia dentada", "Gates", "5PK1234");
        cadastrar(c, d, &u, "AM-500", "Amortecedor traseiro", "Cofap", "");
        let esperado: &[(&str, &str)] = &[
            ("pastilha", "PF-100=Pastilha de freio dianteira"),
            ("pastilha freio", "PF-100=Pastilha de freio dianteira"),
            ("freio pastilha", "PF-100=Pastilha de freio dianteira"),
            ("filtro oleo", "FO-200=Filtro de óleo"),
            ("oleo 5w30", "OL-300=Óleo 5W30 sintético"),
            ("5w30", "OL-300=Óleo 5W30 sintético"),
            ("PF-100", "PF-100=Pastilha de freio dianteira"),
            ("pf 100", "PF-100=Pastilha de freio dianteira"),
            ("GDB1330", "PF-100=Pastilha de freio dianteira!"),
            ("OC-263", "FO-200=Filtro de óleo!"),
            ("Gates", "CD-400=Correia dentada"),
            ("gates correia", "CD-400=Correia dentada"),
            ("amort", "AM-500=Amortecedor traseiro"),
            ("traseiro", "AM-500=Amortecedor traseiro"),
            ("fras", "PF-100=Pastilha de freio dianteira"),
            ("xyzzy", ""),
            ("Mahle filtro", "FO-200=Filtro de óleo"),
            ("ol", "OL-300=Óleo 5W30 sintético ; FO-200=Filtro de óleo"),
            ("CD-400", "CD-400=Correia dentada"),
        ];
        for (consulta, linha) in esperado {
            let r = buscar(c, consulta, false, 50).unwrap();
            let obtido: Vec<String> =
                r.iter().map(|p| format!("{}={}{}", p.codigo, p.nome, if p.codigo_exato { "!" } else { "" })).collect();
            assert_eq!(obtido.join(" ; "), *linha, "consulta {:?}", consulta);
        }
    }
}
