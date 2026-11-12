//! Cadastros a partir de XMLs de notas (NF-e e NFC-e).
//!
//! Serve para os dois casos:
//! - **Notas da própria loja** (o sistema antigo exporta os XMLs das notas
//!   emitidas): o cliente vem do destinatário e a peça vem com o código
//!   interno da loja e o último preço de venda.
//! - **Notas de fornecedores**: o fornecedor vem do emitente e a peça vem
//!   com o código do fornecedor (fica ligado à peça) e o custo.
//!
//! Nada que já existe é mudado: só cadastra o que falta. A mesma pessoa é
//! achada pelo CPF/CNPJ; a mesma peça pelo código interno (notas da loja),
//! pelo código do fornecedor ou pelo código de barras.
//!
//! Aceita arquivos .xml, .zip e .rar com XMLs dentro e pastas (com
//! subpastas).

use std::collections::{BTreeMap, HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::catalogo::codigos::{peca_por_fornecedor_inequivoco, BuscaId};
use crate::compras::pendencias::{registrar_pendencia_xml, resolver_pendencias_do_item};
use crate::erro::{Erro, Resultado};
use crate::nfe_xml::{self, ItemXml, NotaXml};
use crate::pessoas::{self, Emitente};
use crate::{arquivos, auditoria, documento, gtin, numeros, produtos, tempo, texto};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Entrada {
    pub arquivos: Vec<String>,
    pub clientes: bool,
    pub fornecedores: bool,
    pub pecas_loja: bool,
    pub pecas_fornecedor: bool,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Previa {
    /// XMLs encontrados (soltos, nas pastas e nos .zip).
    pub arquivos: i64,
    /// Notas lidas (NF-e ou NFC-e).
    pub notas: i64,
    /// Notas emitidas pela loja e notas de fornecedores.
    pub notas_loja: i64,
    pub notas_fornecedor: i64,
    /// Arquivos que não eram nota (eventos, CT-e, outros) ou com erro.
    pub ignorados: i64,
    pub avisos: Vec<String>,
    pub clientes_novos: i64,
    pub clientes_ja: i64,
    pub fornecedores_novos: i64,
    pub fornecedores_ja: i64,
    pub pecas_loja_novas: i64,
    pub pecas_fornecedor_novas: i64,
    pub pecas_ja: i64,
    pub exemplos_clientes: Vec<String>,
    pub exemplos_fornecedores: Vec<String>,
    pub exemplos_pecas: Vec<String>,
    /// D-13: códigos que aparecem em peças com descrições diferentes (cada
    /// descrição vira uma peça; o código não é dado a nenhuma delas).
    pub codigos_repetidos: i64,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Importado {
    pub clientes: i64,
    pub fornecedores: i64,
    pub pecas: i64,
    pub codigos_ligados: i64,
}

/// O que foi lido das notas, já sem repetição.
#[derive(Default)]
struct Lido {
    arquivos: i64,
    ignorados: i64,
    avisos: Vec<String>,
    notas_loja: i64,
    notas_fornecedor: i64,
    /// documento → (pessoa, emissão da nota mais nova)
    clientes: HashMap<String, (Emitente, String)>,
    fornecedores: HashMap<String, (Emitente, String)>,
    /// (código interno da loja, descrição compactada) → (item, emissão).
    /// D-13: a descrição entra na chave para não fundir peças diferentes que
    /// o sistema antigo deixou com o mesmo código.
    pecas_loja: BTreeMap<(String, String), (ItemXml, String)>,
    /// (documento do fornecedor, código do fornecedor, descrição compactada) → (item, emissão)
    pecas_fornecedor: BTreeMap<(String, String, String), (ItemXml, String)>,
    /// (documento do fornecedor, código do fornecedor, descrição compactada) → (chave_nfe, numero_nfe)
    notas_fornecedor_meta: BTreeMap<(String, String, String), (String, String)>,
}

impl Lido {
    /// Códigos da loja que estão em mais de uma descrição.
    fn repetidos_loja(&self) -> HashSet<String> {
        let mut vistos: HashMap<&str, usize> = HashMap::new();
        for (cod, _) in self.pecas_loja.keys() {
            *vistos.entry(cod.as_str()).or_default() += 1;
        }
        vistos.into_iter().filter(|(_, n)| *n > 1).map(|(c, _)| c.to_string()).collect()
    }

    /// (fornecedor, código compacto) que estão em mais de uma descrição. A02:
    /// pela chave compacta, a mesma do vínculo (como o `codigos_ambiguos` da
    /// compra): "AB-12" e "AB12" são o mesmo código.
    fn repetidos_fornecedor(&self) -> HashSet<(String, String)> {
        let mut vistos: HashMap<(&str, String), HashSet<&str>> = HashMap::new();
        for (f, cod, desc) in self.pecas_fornecedor.keys() {
            vistos.entry((f.as_str(), texto::compactar(cod))).or_default().insert(desc.as_str());
        }
        vistos.into_iter().filter(|(_, d)| d.len() > 1).map(|((f, c), _)| (f.to_string(), c)).collect()
    }

    /// Avisos dos códigos repetidos (até 5), com as descrições.
    fn avisos_repetidos(&self) -> Vec<String> {
        let mut r: Vec<String> = Vec::new();
        let mut loja: Vec<String> = self.repetidos_loja().into_iter().collect();
        loja.sort();
        for cod in loja {
            let nomes: Vec<String> = self
                .pecas_loja
                .iter()
                .filter(|((c, _), _)| *c == cod)
                .map(|(_, (it, _))| texto::limpar(&it.descricao))
                .collect();
            r.push(format!(
                "Código {cod} em {} peças diferentes nas notas da loja ({}). Entram separadas e nenhuma fica com o código {cod}; ele fica como código antigo de todas (a busca mostra as peças para escolher).",
                nomes.len(),
                nomes.join(" / ")
            ));
        }
        let mut forn: Vec<(String, String)> = self.repetidos_fornecedor().into_iter().collect();
        forn.sort();
        for (f, compacto) in forn {
            // O código como veio nas notas ("AB-12 / AB12" quando vieram as duas formas).
            let formas: std::collections::BTreeSet<&str> = self
                .pecas_fornecedor
                .keys()
                .filter(|(f2, c, _)| *f2 == f && texto::compactar(c) == compacto)
                .map(|(_, c, _)| c.as_str())
                .collect();
            let cod = formas.into_iter().collect::<Vec<_>>().join(" / ");
            r.push(format!(
                "Código {cod} de um fornecedor está em peças diferentes. Entram separadas e o código não fica ligado a nenhuma: na próxima compra, escolha a peça certa."
            ));
        }
        r
    }
}

/// Descrição para comparar (sem espaços, pontuação, acentos e maiúsculas).
fn descricao_chave(it: &ItemXml) -> String {
    texto::compactar(&it.descricao)
}

// ---------------------------------------------------------------------------
// Achar e ler os arquivos (soltos, pastas, .zip e .rar): ver `arquivos`.
// ---------------------------------------------------------------------------

/// Chama `cada` com (nome do arquivo, conteúdo) de todos os XMLs achados.
fn cada_xml(entradas: &[String], mut cada: impl FnMut(&str, &str)) -> Resultado<i64> {
    let lista = arquivos::xmls(entradas)?;
    let lidos = lista.len() as i64;
    for x in lista {
        cada(&x.nome, &x.texto);
    }
    Ok(lidos)
}

/// Vazio ou inválido: recusa com o requisito (a tela leva a Configurações › Loja).
fn cnpj_loja(conn: &Connection) -> Resultado<String> {
    crate::requisitos::exigir_cnpj_da_loja(conn, crate::requisitos::PARA_CADASTROS)
}

fn guardar_mais_nova<K: Ord + std::hash::Hash + Eq, V>(
    mapa: &mut HashMap<K, (V, String)>,
    chave: K,
    valor: V,
    emissao: &str,
) {
    match mapa.get(&chave) {
        Some((_, e)) if e.as_str() >= emissao => {}
        _ => {
            mapa.insert(chave, (valor, emissao.to_string()));
        }
    }
}

fn guardar_mais_nova_b<K: Ord, V>(mapa: &mut BTreeMap<K, (V, String)>, chave: K, valor: V, emissao: &str) {
    match mapa.get(&chave) {
        Some((_, e)) if e.as_str() >= emissao => {}
        _ => {
            mapa.insert(chave, (valor, emissao.to_string()));
        }
    }
}

fn ler_tudo(conn: &Connection, arquivos: &[String]) -> Resultado<Lido> {
    if arquivos.is_empty() {
        return Err(Erro::validacao("Escolha os arquivos XML, um .zip, um .rar ou uma pasta."));
    }
    let loja = cnpj_loja(conn)?;
    let mut lido = Lido::default();
    let mut chaves: HashSet<String> = HashSet::new();
    let total = cada_xml(arquivos, |nome, xml| {
        // PEND-09 (D2, D8): só cadastra a partir de nota com a assinatura do
        // emitente verificada; a autorização da SEFAZ não é exigida aqui
        // (cadastro não mexe em estoque, custo nem financeiro).
        let nota: NotaXml = match nfe_xml::ler_cadastro_verificada(xml, crate::nfe_assinatura::Ancoras::padrao()) {
            Ok(n) => n.nota,
            Err(e) => {
                lido.ignorados += 1;
                if lido.avisos.len() < 5 && !e.contains("evento") && !e.contains("CT-e") {
                    lido.avisos.push(format!("{nome}: {e}"));
                }
                return;
            }
        };
        if !nota.chave.is_empty() && !chaves.insert(nota.chave.clone()) {
            return; // a mesma nota em dois arquivos
        }
        let emitente = documento::limpar(&nota.emitente.documento);
        let destinatario = documento::limpar(&nota.destinatario);
        if emitente == loja {
            lido.notas_loja += 1;
            // Devolução para fornecedor: o destinatário não é cliente.
            if nota.finalidade == "4" {
                return;
            }
            if !destinatario.is_empty() && destinatario != loja {
                guardar_mais_nova(&mut lido.clientes, destinatario, nota.destinatario_dados.clone(), &nota.emissao);
            }
            for it in &nota.itens {
                let cod = it.codigo.trim().to_string();
                if !cod.is_empty() {
                    let chave = (cod, descricao_chave(it));
                    guardar_mais_nova_b(&mut lido.pecas_loja, chave, it.clone(), &nota.emissao);
                }
            }
        } else {
            lido.notas_fornecedor += 1;
            if emitente.is_empty() || nota.finalidade == "4" {
                return;
            }
            guardar_mais_nova(&mut lido.fornecedores, emitente.clone(), nota.emitente.clone(), &nota.emissao);
            for it in &nota.itens {
                let cod = it.codigo.trim().to_string();
                if !cod.is_empty() {
                    let chave = (emitente.clone(), cod, descricao_chave(it));
                    let atualizar_meta = match lido.pecas_fornecedor.get(&chave) {
                        Some((_, e)) => e.as_str() < nota.emissao.as_str(),
                        None => true,
                    };
                    guardar_mais_nova_b(&mut lido.pecas_fornecedor, chave.clone(), it.clone(), &nota.emissao);
                    if atualizar_meta {
                        lido.notas_fornecedor_meta.insert(chave, (nota.chave.clone(), nota.numero.clone()));
                    }
                }
            }
        }
    })?;
    lido.arquivos = total;
    let avisos = lido.avisos_repetidos();
    for a in avisos.into_iter().take(5) {
        lido.avisos.push(a);
    }
    Ok(lido)
}

// ---------------------------------------------------------------------------
// Conferir o que já existe
// ---------------------------------------------------------------------------

pub(crate) fn gtins_para_ficha(gtins: &[nfe_xml::GtinXml], gtin_fallback: &str) -> Vec<produtos::GtinFicha> {
    let mut r: Vec<produtos::GtinFicha> = gtins
        .iter()
        .map(|g| produtos::GtinFicha { gtin: g.codigo.clone(), tipo: g.tipo.clone(), origem: g.origem.clone() })
        .collect();
    if r.is_empty() && !gtin_fallback.is_empty() && gtin::gtin_valido(gtin_fallback) {
        r.push(produtos::GtinFicha {
            gtin: gtin_fallback.to_string(),
            tipo: "comercial".into(),
            origem: "cEAN".into(),
        });
    }
    r
}

pub(crate) fn peca_por_gtins_inequivoco(
    conn: &Connection,
    gtins: &[nfe_xml::GtinXml],
    gtin_fallback: &str,
) -> Resultado<BuscaId> {
    let mut codigos: Vec<String> = Vec::new();
    for g in gtins {
        let valido = gtin::gtin_valido(&g.codigo)
            || (matches!(g.origem.as_str(), "cBarra" | "cBarraTrib") && g.codigo.len() >= 4);
        if valido && !codigos.contains(&g.codigo) {
            codigos.push(g.codigo.clone());
        }
    }
    if codigos.is_empty() && !gtin_fallback.is_empty() && gtin::gtin_valido(gtin_fallback) {
        codigos.push(gtin_fallback.to_string());
    }
    if codigos.is_empty() {
        return Ok(BuscaId::Nenhum);
    }
    crate::catalogo::codigos::pecas_pelos_gtins(conn, &codigos)
}

fn pessoa_existe(conn: &Connection, doc: &str) -> Resultado<Option<String>> {
    // N07: `documento <> ''` deixa o SQLite usar o índice parcial; nunca vem vazio.
    Ok(conn
        .query_row("SELECT id FROM pessoa WHERE documento = ?1 AND documento <> ''", [doc], |l| l.get(0))
        .optional()?)
}

/// Como a pendência de um item de nota da própria loja aparece (não há fornecedor).
const NOTA_DA_LOJA: &str = "Nota da própria loja";

/// A04 (D-13): a peça que já tem este código da loja, quando a descrição dela
/// é outra. O nome cortado no cadastro pela nota (120 letras) conta como o mesmo.
fn peca_de_outra_descricao(conn: &Connection, codigo: &str, it: &ItemXml) -> Resultado<Option<String>> {
    let achada: Option<(String, String)> = conn
        .query_row("SELECT id, nome FROM produto WHERE codigo = ?1", [codigo], |l| Ok((l.get(0)?, l.get(1)?)))
        .optional()?;
    let Some((id, nome)) = achada else { return Ok(None) };
    let (nome_c, desc_c) = (texto::compactar(&nome), descricao_chave(it));
    let cortado = nome.chars().count() >= 120 && desc_c.starts_with(&nome_c);
    Ok((!desc_c.is_empty() && nome_c != desc_c && !cortado).then_some(id))
}

fn peca_por_codigo(conn: &Connection, codigo: &str) -> Resultado<Option<String>> {
    Ok(conn.query_row("SELECT id FROM produto WHERE codigo = ?1", [codigo], |l| l.get(0)).optional()?)
}

fn peca_por_barras(conn: &Connection, gtin_: &str) -> Resultado<Option<String>> {
    if gtin_.is_empty() || !gtin::gtin_valido(gtin_) {
        return Ok(None);
    }
    match peca_por_gtins_inequivoco(conn, &[], gtin_)? {
        BuscaId::Unico(id) => Ok(Some(id)),
        _ => Ok(None),
    }
}

/// D-13: peça de código repetido já cadastrada (mesmo código antigo ou
/// principal e a mesma descrição).
fn peca_repetida(conn: &Connection, codigo: &str, it: &ItemXml) -> Resultado<Option<String>> {
    let desc = descricao_chave(it);
    let mut c = conn.prepare_cached(
        "SELECT p.id, p.nome FROM produto p
          WHERE p.codigo = ?1
             OR EXISTS (SELECT 1 FROM produto_codigo_antigo a WHERE a.produto_id = p.id AND a.codigo = ?1)",
    )?;
    let lista: Vec<(String, String)> =
        c.query_map([codigo], |l| Ok((l.get(0)?, l.get(1)?)))?.collect::<Result<_, _>>()?;
    Ok(lista.into_iter().find(|(_, nome)| texto::compactar(nome) == desc).map(|(id, _)| id))
}

/// D-13: peça de fornecedor de código repetido já cadastrada (pela descrição).
///
/// A14: antes, cada consulta lia e compactava todas as peças. Agora o mapa
/// (descrição compactada → a primeira peça com ela, na ordem da tabela, como a
/// consulta de antes) é montado uma vez, na primeira consulta de cada prévia
/// ou importação, e cada peça nova da importação entra nele (`nova`).
#[derive(Default)]
struct PorDescricao(std::cell::RefCell<Option<HashMap<String, String>>>);

impl PorDescricao {
    fn achar(&self, conn: &Connection, it: &ItemXml) -> Resultado<Option<String>> {
        let mut mapa = self.0.borrow_mut();
        if mapa.is_none() {
            let mut m: HashMap<String, String> = HashMap::new();
            let mut c = conn.prepare_cached("SELECT id, nome FROM produto")?;
            for linha in c.query_map([], |l| Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?)))? {
                let (id, nome) = linha?;
                m.entry(texto::compactar(&nome)).or_insert(id);
            }
            *mapa = Some(m);
        }
        Ok(mapa.as_ref().and_then(|m| m.get(&descricao_chave(it)).cloned()))
    }

    /// Uma peça nova desta importação (com o nome como ficou gravado).
    fn nova(&self, conn: &Connection, id: &str) -> Resultado<()> {
        if let Some(m) = self.0.borrow_mut().as_mut() {
            let nome: String = conn.query_row("SELECT nome FROM produto WHERE id = ?1", [id], |l| l.get(0))?;
            m.entry(texto::compactar(&nome)).or_insert_with(|| id.to_string());
        }
        Ok(())
    }
}

/// A peça da loja deste item já existe?
fn peca_loja_existe(conn: &Connection, codigo: &str, it: &ItemXml, repetido: bool) -> Resultado<bool> {
    let gtin_existe = !matches!(peca_por_gtins_inequivoco(conn, &it.gtins, &it.gtin)?, BuscaId::Nenhum);
    if repetido {
        return Ok(peca_repetida(conn, codigo, it)?.is_some() || gtin_existe);
    }
    Ok(peca_por_codigo(conn, codigo)?.is_some() || gtin_existe)
}

pub fn previa(conn: &Connection, arquivos: &[String]) -> Resultado<Previa> {
    let lido = ler_tudo(conn, arquivos)?;
    let mut p = Previa {
        arquivos: lido.arquivos,
        notas: lido.notas_loja + lido.notas_fornecedor,
        notas_loja: lido.notas_loja,
        notas_fornecedor: lido.notas_fornecedor,
        ignorados: lido.ignorados,
        avisos: lido.avisos.clone(),
        ..Default::default()
    };
    let mut nomes: Vec<(String, String)> = Vec::new();
    for (doc, (e, _)) in &lido.clientes {
        if pessoa_existe(conn, doc)?.is_some() {
            p.clientes_ja += 1;
        } else {
            p.clientes_novos += 1;
            nomes.push((e.nome.clone(), doc.clone()));
        }
    }
    nomes.sort();
    p.exemplos_clientes = nomes.iter().take(6).map(|n| n.0.clone()).collect();
    let mut nomes: Vec<String> = Vec::new();
    for (doc, (e, _)) in &lido.fornecedores {
        if pessoa_existe(conn, doc)?.is_some() {
            p.fornecedores_ja += 1;
        } else {
            p.fornecedores_novos += 1;
            nomes.push(if e.fantasia.is_empty() { e.nome.clone() } else { e.fantasia.clone() });
        }
    }
    nomes.sort();
    p.exemplos_fornecedores = nomes.into_iter().take(6).collect();

    let repetidos = lido.repetidos_loja();
    let repetidos_f = lido.repetidos_fornecedor();
    let por_descricao = PorDescricao::default();
    p.codigos_repetidos = (repetidos.len() + repetidos_f.len()) as i64;
    let mut barras_novas: HashSet<String> = HashSet::new();
    let mut exemplos: Vec<String> = Vec::new();
    for ((codigo, _), (it, _)) in &lido.pecas_loja {
        if peca_loja_existe(conn, codigo, it, repetidos.contains(codigo))? {
            p.pecas_ja += 1;
        } else {
            p.pecas_loja_novas += 1;
            if !it.gtin.is_empty() {
                barras_novas.insert(it.gtin.clone());
            }
            for g in &it.gtins {
                if !g.codigo.is_empty() {
                    barras_novas.insert(g.codigo.clone());
                }
            }
            if exemplos.len() < 6 {
                exemplos.push(texto::limpar(&it.descricao));
            }
        }
    }
    for ((forn, codigo, _), (it, _)) in &lido.pecas_fornecedor {
        let ja = if repetidos_f.contains(&(forn.clone(), texto::compactar(codigo))) {
            por_descricao.achar(conn, it)?.is_some()
        } else {
            !matches!(peca_por_fornecedor_inequivoco(conn, forn, codigo)?, BuscaId::Nenhum)
        };
        let gtin_ja = !matches!(peca_por_gtins_inequivoco(conn, &it.gtins, &it.gtin)?, BuscaId::Nenhum)
            || (!it.gtin.is_empty() && barras_novas.contains(&it.gtin))
            || it.gtins.iter().any(|g| !g.codigo.is_empty() && barras_novas.contains(&g.codigo));
        if ja || gtin_ja {
            p.pecas_ja += 1;
        } else {
            p.pecas_fornecedor_novas += 1;
            if !it.gtin.is_empty() {
                barras_novas.insert(it.gtin.clone());
            }
            for g in &it.gtins {
                if !g.codigo.is_empty() {
                    barras_novas.insert(g.codigo.clone());
                }
            }
            if exemplos.len() < 6 {
                exemplos.push(texto::limpar(&it.descricao));
            }
        }
    }
    p.exemplos_pecas = exemplos;
    Ok(p)
}

// ---------------------------------------------------------------------------
// Cadastrar
// ---------------------------------------------------------------------------

/// Valor de uma unidade do item (em centavos): o que a nota cobrou por
/// unidade, já com frete, IPI e ST e menos o desconto.
pub(crate) fn valor_unitario(it: &ItemXml, com_impostos: bool) -> i64 {
    let total = if com_impostos {
        it.valor_produtos + it.frete + it.seguro + it.outras + it.ipi + it.icms_st - it.desconto
    } else {
        it.valor_produtos - it.desconto
    };
    if it.quantidade <= 0 {
        return 0;
    }
    crate::numeros::saturar((total as i128 * 1000 + it.quantidade as i128 / 2) / it.quantidade as i128)
}

fn ficha_do_item(conn: &Connection, it: &ItemXml) -> Resultado<produtos::ProdutoFicha> {
    let mut f = produtos::ficha_nova(conn)?;
    f.nome = texto::limpar(&it.descricao).chars().take(120).collect();
    if f.nome.is_empty() {
        f.nome = "Peça sem nome".into();
    }
    if !it.gtin.is_empty() && gtin::gtin_valido(&it.gtin) && peca_por_barras(conn, &it.gtin)?.is_none() {
        f.codigo_barras = it.gtin.clone();
    }
    f.gtins = gtins_para_ficha(&it.gtins, &it.gtin);
    f.unidade = crate::unidades::de_fora(&it.unidade);
    if it.ncm.len() == 8 && it.ncm.chars().all(|c| c.is_ascii_digit()) {
        f.ncm = it.ncm.clone();
    }
    if it.cest.len() == 7 && it.cest.chars().all(|c| c.is_ascii_digit()) {
        f.cest = it.cest.clone();
    }
    if (0..produtos::ORIGENS.len() as i64).contains(&it.origem) {
        f.origem = it.origem;
    }
    f.monofasico = it.cst_pis == "04";
    Ok(f)
}

/// Liga o código do fornecedor à peça, sem mexer num vínculo que já existe.
fn ligar_fornecedor(conn: &Connection, fornecedor_id: &str, codigo: &str, produto_id: &str) -> Resultado<bool> {
    let ligou = crate::catalogo::codigos::vincular_fornecedor(
        conn,
        fornecedor_id,
        codigo,
        produto_id,
        1000,
        crate::catalogo::codigos::JaLigado::Manter,
    )?;
    if ligou {
        produtos::atualizar_indice_busca(conn, produto_id)?;
    }
    Ok(ligou)
}

/// Quem grava, nas etapas da importação de cadastros.
#[derive(Clone, Copy)]
struct Gravacao<'a> {
    conn: &'a Connection,
    dispositivo: &'a str,
    usuario_id: &'a str,
}

/// O que as etapas das peças do fornecedor precisam saber do lote lido.
struct LoteFornecedor<'a> {
    e: &'a Entrada,
    lido: &'a Lido,
    ids_fornecedor: &'a HashMap<String, String>,
    repetidos_f: HashSet<(String, String)>,
    gtins_repetidos_forn: HashSet<String>,
    por_descricao: PorDescricao,
}

/// "Trazer cadastros" dos XMLs, em etapas (ARQ-13): clientes, fornecedores,
/// peças da loja e peças dos fornecedores.
pub fn importar(conn: &Connection, dispositivo: &str, usuario_id: &str, e: &Entrada) -> Resultado<Importado> {
    let lido = ler_tudo(conn, &e.arquivos)?;
    gravar(conn, dispositivo, usuario_id, e, &lido)
}

/// As etapas de gravação sobre o que já foi lido. Separada da leitura para os
/// testes montarem casos sem nota assinada (as notas de teste vêm prontas).
fn gravar(conn: &Connection, dispositivo: &str, usuario_id: &str, e: &Entrada, lido: &Lido) -> Resultado<Importado> {
    let mut r = Importado::default();
    let gr = Gravacao { conn, dispositivo, usuario_id };
    if e.clientes {
        importar_clientes(gr, lido, &mut r)?;
    }
    let ids_fornecedor = importar_fornecedores(gr, e, lido, &mut r)?;
    if e.pecas_loja {
        importar_pecas_da_loja(gr, lido, &mut r)?;
    }
    let lote = LoteFornecedor {
        e,
        lido,
        ids_fornecedor: &ids_fornecedor,
        repetidos_f: lido.repetidos_fornecedor(),
        gtins_repetidos_forn: gtins_em_mais_de_uma_descricao(lido),
        por_descricao: PorDescricao::default(),
    };
    for (chave, item) in &lido.pecas_fornecedor {
        importar_peca_do_fornecedor(gr, &lote, chave, item, &mut r)?;
    }
    Ok(r)
}

/// Etapa 1: os clientes das notas da loja que ainda não estão cadastrados.
fn importar_clientes(gr: Gravacao, lido: &Lido, r: &mut Importado) -> Resultado<()> {
    let Gravacao { conn, dispositivo, usuario_id } = gr;
    let mut lista: Vec<_> = lido.clientes.iter().collect();
    lista.sort_by(|a, b| a.0.cmp(b.0));
    for (doc, (pessoa, _)) in lista {
        if pessoa_existe(conn, doc)?.is_none() {
            pessoas::garantir_pela_nota(
                conn,
                dispositivo,
                usuario_id,
                pessoa,
                true,
                "Cadastrado pelo XML de uma nota da loja",
            )?;
            r.clientes += 1;
        }
    }
    Ok(())
}

/// Etapa 2: os fornecedores. Cadastra os que faltam (sempre que for cadastrar
/// as peças deles também, para ligar o código). Devolve documento → id.
fn importar_fornecedores(
    gr: Gravacao,
    e: &Entrada,
    lido: &Lido,
    r: &mut Importado,
) -> Resultado<HashMap<String, String>> {
    let Gravacao { conn, dispositivo, usuario_id } = gr;
    let mut ids_fornecedor: HashMap<String, String> = HashMap::new();
    let mut lista: Vec<_> = lido.fornecedores.iter().collect();
    lista.sort_by(|a, b| a.0.cmp(b.0));
    for (doc, (pessoa, _)) in lista {
        match pessoa_existe(conn, doc)? {
            Some(id) => {
                ids_fornecedor.insert(doc.clone(), id);
            }
            None if e.fornecedores || e.pecas_fornecedor => {
                let (id, _) = pessoas::garantir_pela_nota(
                    conn,
                    dispositivo,
                    usuario_id,
                    pessoa,
                    false,
                    "Cadastrado pelo XML de uma nota de compra",
                )?;
                ids_fornecedor.insert(doc.clone(), id);
                r.fornecedores += 1;
            }
            None => {}
        }
    }
    Ok(ids_fornecedor)
}

/// Etapa 3: as peças das notas da loja.
fn importar_pecas_da_loja(gr: Gravacao, lido: &Lido, r: &mut Importado) -> Resultado<()> {
    let Gravacao { conn, dispositivo, usuario_id } = gr;
    let repetidos = lido.repetidos_loja();
    for ((codigo, _), (it, emissao)) in &lido.pecas_loja {
        let repetido = repetidos.contains(codigo);
        let gtins_ficha = gtins_para_ficha(&it.gtins, &it.gtin);
        // A04 (D-13): o código da loja só identifica a peça se a descrição
        // também bater. Com outra descrição, vale o GTIN; sem GTIN que ache a
        // peça, o item vira pendência, sem gravar nada em peça nenhuma.
        if let Some(outra) = if repetido { None } else { peca_de_outra_descricao(conn, codigo, it)? } {
            match peca_por_gtins_inequivoco(conn, &it.gtins, &it.gtin)? {
                BuscaId::Unico(pid) => {
                    produtos::registrar_gtins_xml(conn, &pid, &gtins_ficha)?;
                }
                _ => {
                    // Sem fornecedor: resolver esta pendência nunca liga código de fornecedor.
                    registrar_pendencia_xml(
                        conn,
                        None,
                        "",
                        NOTA_DA_LOJA,
                        "",
                        "",
                        emissao,
                        it,
                        valor_unitario(it, false),
                        "codigo_de_outra_peca",
                        &[outra],
                    )?;
                }
            }
            continue;
        }
        if peca_loja_existe(conn, codigo, it, repetido)? {
            let pid_existente =
                if repetido { peca_repetida(conn, codigo, it)? } else { peca_por_codigo(conn, codigo)? }.or_else(
                    || match peca_por_gtins_inequivoco(conn, &it.gtins, &it.gtin).ok()? {
                        BuscaId::Unico(id) => Some(id),
                        _ => None,
                    },
                );
            if let Some(pid) = pid_existente {
                produtos::registrar_gtins_xml(conn, &pid, &gtins_ficha)?;
            }
            continue;
        }
        let mut f = ficha_do_item(conn, it)?;
        // O código da loja só se usar letras, números e traço, sem repetir.
        // D-13: código que o sistema antigo deu a peças diferentes não fica
        // com nenhuma (o leitor daria a peça errada); vira código antigo.
        let cod = texto::limpar(codigo);
        if !repetido && !cod.is_empty() && cod.chars().count() <= 20 && peca_por_codigo(conn, &cod)?.is_none() {
            f.codigo = cod.clone();
        }
        let preco = valor_unitario(it, false);
        if preco > 0 {
            if let Some(p) = f.precos.iter_mut().find(|p| p.tabela == 1) {
                p.preco = numeros::escrever_dinheiro(preco);
                p.lucro = String::new();
            }
        }
        let id = salvar_peca(conn, dispositivo, usuario_id, f)?;
        produtos::registrar_gtins_xml(conn, &id, &gtins_ficha)?;
        let mut detalhes = vec!["Cadastrada pelo XML de uma nota da loja".to_string()];
        if repetido && !cod.is_empty() {
            crate::catalogo::codigos::guardar_codigo_antigo(conn, &cod, &id, &tempo::agora())?;
            produtos::atualizar_indice_busca(conn, &id)?;
            detalhes
                .push(format!("O código {cod} do sistema antigo está em mais de uma peça: ficou como código antigo"));
            // A faixa deste caixa pode ter dado justamente esse número a
            // esta peça (antes de ele virar código antigo): troca.
            let atual: String = conn.query_row("SELECT codigo FROM produto WHERE id = ?1", [&id], |l| l.get(0))?;
            if texto::compactar(&atual) == texto::compactar(&cod) {
                produtos::trocar_codigo(conn, dispositivo, usuario_id, &id)?;
            }
        }
        auditoria::registrar(conn, dispositivo, Some(usuario_id), "produto.importar", "produto", &id, &detalhes)?;
        r.pecas += 1;
    }
    Ok(())
}

/// GTINs que aparecem em mais de uma descrição nas notas de fornecedor.
fn gtins_em_mais_de_uma_descricao(lido: &Lido) -> HashSet<String> {
    let mut vistos: HashMap<String, HashSet<String>> = HashMap::new();
    for ((_, _, desc), (it, _)) in &lido.pecas_fornecedor {
        for g in gtins_para_ficha(&it.gtins, &it.gtin) {
            if !g.gtin.is_empty() {
                vistos.entry(g.gtin).or_default().insert(desc.clone());
            }
        }
    }
    vistos.into_iter().filter(|(_, d)| d.len() > 1).map(|(g, _)| g).collect()
}

/// Etapa 4: uma peça das notas de fornecedor: liga ao que já existe, deixa
/// pendente ou cadastra, conforme o que o GTIN e o código do fornecedor acham.
fn importar_peca_do_fornecedor(
    gr: Gravacao,
    lote: &LoteFornecedor,
    (forn, codigo, desc_chave): &(String, String, String),
    (it, emissao): &(ItemXml, String),
    r: &mut Importado,
) -> Resultado<()> {
    let Gravacao { conn, dispositivo, usuario_id } = gr;
    let LoteFornecedor { e, lido, ids_fornecedor, repetidos_f, gtins_repetidos_forn, por_descricao } = lote;
    let fornecedor_id = ids_fornecedor.get(forn).cloned();
    let forn_nome = lido
        .fornecedores
        .get(forn)
        .map(|(em, _)| if em.fantasia.is_empty() { em.nome.clone() } else { em.fantasia.clone() })
        .unwrap_or_default();
    let (chave_nfe, numero_nfe) = lido
        .notas_fornecedor_meta
        .get(&(forn.clone(), codigo.clone(), desc_chave.clone()))
        .cloned()
        .unwrap_or_default();
    let repetido = repetidos_f.contains(&(forn.clone(), texto::compactar(codigo)));
    let gtins_ficha = gtins_para_ficha(&it.gtins, &it.gtin);
    let gtin_repetido_no_lote = gtins_ficha.iter().any(|g| gtins_repetidos_forn.contains(&g.gtin));
    let gtin_busca = if gtin_repetido_no_lote {
        BuscaId::Ambiguo(Vec::new())
    } else {
        peca_por_gtins_inequivoco(conn, &it.gtins, &it.gtin)?
    };
    let forn_busca = if repetido { BuscaId::Nenhum } else { peca_por_fornecedor_inequivoco(conn, forn, codigo)? };

    // 1. Ambiguidade de GTIN ou conflito entre GTIN e código do fornecedor:
    //    não associar automaticamente; registrar pendência persistente.
    if let BuscaId::Ambiguo(cands) = &gtin_busca {
        registrar_pendencia_xml(
            conn,
            fornecedor_id.as_deref(),
            forn,
            &forn_nome,
            &chave_nfe,
            &numero_nfe,
            emissao,
            it,
            valor_unitario(it, true),
            "gtin_ambiguo",
            cands,
        )?;
        return Ok(());
    }
    if let BuscaId::Ambiguo(cands) = &forn_busca {
        registrar_pendencia_xml(
            conn,
            fornecedor_id.as_deref(),
            forn,
            &forn_nome,
            &chave_nfe,
            &numero_nfe,
            emissao,
            it,
            valor_unitario(it, true),
            "fornecedor_ambiguo",
            cands,
        )?;
        return Ok(());
    }
    if let (BuscaId::Unico(pid_g), BuscaId::Unico(pid_f)) = (&gtin_busca, &forn_busca) {
        if pid_g != pid_f {
            registrar_pendencia_xml(
                conn,
                fornecedor_id.as_deref(),
                forn,
                &forn_nome,
                &chave_nfe,
                &numero_nfe,
                emissao,
                it,
                valor_unitario(it, true),
                "conflito_gtin_fornecedor",
                &[pid_g.clone(), pid_f.clone()],
            )?;
            return Ok(());
        }
    }

    // 2. GTIN exato e inequívoco tem prioridade antes de outras formas.
    if let BuscaId::Unico(pid) = gtin_busca {
        // A03a (DP-11): sem "Peças do fornecedor" marcada, nada é gravado na peça
        // que já existe (nem GTIN, nem vínculo). Pela leitura técnica da DP-11
        // (sem veto), o item fica como pendência, com a peça como candidata.
        if !e.pecas_fornecedor {
            // O código do fornecedor já está ligado a esta mesma peça: o item
            // já está identificado e não há nada a decidir. Sem a opção, nada é
            // gravado na peça; as pendências do item fecham, como no passo 3.
            if matches!(&forn_busca, BuscaId::Unico(pf) if *pf == pid) {
                if let Some(fid) = &fornecedor_id {
                    resolver_pendencias_do_item(conn, fid, codigo, &it.descricao, &pid)?;
                }
                return Ok(());
            }
            if fornecedor_id.is_some() || e.fornecedores {
                registrar_pendencia_xml(
                    conn,
                    fornecedor_id.as_deref(),
                    forn,
                    &forn_nome,
                    &chave_nfe,
                    &numero_nfe,
                    emissao,
                    it,
                    valor_unitario(it, true),
                    "peca_pelo_gtin",
                    &[pid],
                )?;
            }
            return Ok(());
        }
        produtos::registrar_gtins_xml(conn, &pid, &gtins_ficha)?;
        if !repetido {
            if let Some(fid) = &fornecedor_id {
                if matches!(forn_busca, BuscaId::Nenhum) && ligar_fornecedor(conn, fid, codigo, &pid)? {
                    r.codigos_ligados += 1;
                }
                resolver_pendencias_do_item(conn, fid, codigo, &it.descricao, &pid)?;
            }
        }
        return Ok(());
    }

    // 3. Associação já existente pelo (CNPJ do fornecedor, cProd).
    if let BuscaId::Unico(pid) = forn_busca {
        // A03a (DP-11): os GTINs da nota só vão para a peça com "Peças do fornecedor".
        if e.pecas_fornecedor {
            produtos::registrar_gtins_xml(conn, &pid, &gtins_ficha)?;
        }
        if let Some(fid) = &fornecedor_id {
            resolver_pendencias_do_item(conn, fid, codigo, &it.descricao, &pid)?;
        }
        return Ok(());
    }

    // 4. Sem GTIN correspondente e sem vínculo prévio do fornecedor.
    // A01: um código só de símbolos ("---") não identifica a peça nem pode ser
    // ligado; vira pendência própria em vez de peça nova (que se repetiria a
    // cada importação da mesma nota).
    let so_simbolos = texto::compactar(codigo).is_empty();
    if e.pecas_fornecedor && !so_simbolos {
        let Some(fid) = &fornecedor_id else { return Ok(()) };
        if repetido {
            if let Some(pid_desc) = por_descricao.achar(conn, it)? {
                produtos::registrar_gtins_xml(conn, &pid_desc, &gtins_ficha)?;
                return Ok(());
            }
        }
        let mut f = ficha_do_item(conn, it)?;
        let custo = valor_unitario(it, true);
        if custo > 0 {
            f.custo = numeros::escrever_dinheiro(custo);
            if let Some(p) = f.precos.iter_mut().find(|p| p.tabela == 1) {
                if p.lucro.is_empty() {
                    p.lucro = "100".into();
                }
            }
        }
        let id = salvar_peca(conn, dispositivo, usuario_id, f)?;
        por_descricao.nova(conn, &id)?;
        produtos::registrar_gtins_xml(conn, &id, &gtins_ficha)?;
        if !repetido {
            ligar_fornecedor(conn, fid, codigo, &id)?;
            resolver_pendencias_do_item(conn, fid, codigo, &it.descricao, &id)?;
        }
        auditoria::registrar(
            conn,
            dispositivo,
            Some(usuario_id),
            "produto.importar",
            "produto",
            &id,
            &["Cadastrada pelo XML de uma nota de compra".into()],
        )?;
        r.pecas += 1;
    } else {
        // Sem cadastrar peça nova (a opção desmarcada ou o código só de
        // símbolos): preserva como pendência de identificação caso o
        // fornecedor tenha sido selecionado ou já exista.
        if fornecedor_id.is_some() || e.fornecedores {
            registrar_pendencia_xml(
                conn,
                fornecedor_id.as_deref(),
                forn,
                &forn_nome,
                &chave_nfe,
                &numero_nfe,
                emissao,
                it,
                valor_unitario(it, true),
                if so_simbolos {
                    "codigo_so_de_simbolos"
                } else if repetido {
                    "codigo_repetido"
                } else {
                    "sem_vinculo"
                },
                &[],
            )?;
        }
    }
    Ok(())
}

/// Salva a peça; se o código de barras ou o código já existirem (corrida
/// com outra peça da mesma importação), tenta sem eles.
fn salvar_peca(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    mut f: produtos::ProdutoFicha,
) -> Resultado<String> {
    match produtos::salvar(conn, dispositivo, usuario_id, &f) {
        Ok(id) => Ok(id),
        Err(Erro::Validacao { campo: Some(c), .. }) if c == "codigoBarras" || c == "codigo" => {
            f.codigo_barras = String::new();
            f.codigo = String::new();
            f.gtins.clear();
            produtos::salvar(conn, dispositivo, usuario_id, &f)
        }
        Err(e) => Err(e),
    }
}

// ---------------------------------------------------------------------------
// Pendências persistentes de identificação XML (Fase 4.D)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidatoPendencia {
    pub id: String,
    pub codigo: String,
    pub nome: String,
    pub marca: String,
    pub codigo_barras: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendenciaXml {
    pub id: String,
    pub fornecedor_id: String,
    pub fornecedor_documento: String,
    pub fornecedor_nome: String,
    pub chave_nfe: String,
    pub numero_nfe: String,
    pub emissao_nfe: String,
    pub codigo_fornecedor: String,
    pub gtin_comercial: String,
    pub origem_comercial: String,
    pub gtin_tributavel: String,
    pub origem_tributavel: String,
    pub gtins: Vec<produtos::GtinFicha>,
    pub descricao: String,
    pub ncm: String,
    pub cest: String,
    pub cfop: String,
    pub unidade: String,
    pub quantidade: String,
    pub valor_unitario: String,
    pub motivo: String,
    pub motivo_texto: String,
    pub candidatos: Vec<CandidatoPendencia>,
    pub criado_em: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ResolverPendencia {
    pub id: String,
    /// "ligar" (associar a `produto_id` existente) ou "nova" (cadastrar peça nova com os dados do XML).
    pub acao: String,
    pub produto_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendenciaResolvida {
    pub produto_id: String,
    pub pendentes: i64,
}

/// Quantas pendências de identificação estão abertas; 0 se a leitura falhar
/// (como sempre foi: o selo do menu e a medição usam assim). O aviso da Início
/// usa `contar_pendencias_ou_erro`, que não esconde a falha (A13).
pub fn contar_pendencias(conn: &Connection) -> i64 {
    contar_pendencias_ou_erro(conn).unwrap_or(0)
}

/// A13: a contagem com a falha de leitura como erro (antes ela virava zero, e o
/// aviso da Início sumia sem motivo).
pub fn contar_pendencias_ou_erro(conn: &Connection) -> Resultado<i64> {
    Ok(conn
        .query_row("SELECT COUNT(*) FROM xml_pendencia_identificacao WHERE situacao = 'pendente'", [], |l| l.get(0))?)
}

fn motivo_legivel(motivo: &str) -> &'static str {
    match motivo {
        "gtin_ambiguo" => "GTIN/código de barras corresponde a mais de uma peça cadastrada",
        "codigo_so_de_simbolos" => "O código do fornecedor só tem símbolos e não identifica a peça",
        "fornecedor_ambiguo" => "Código do fornecedor está associado a mais de uma peça",
        "conflito_gtin_fornecedor" => "GTIN aponta para uma peça e o código do fornecedor para outra",
        "codigo_repetido" => "O fornecedor usou o mesmo código em peças com descrições diferentes na nota",
        "codigo_ambiguo" => "Código original/equivalente corresponde a mais de uma peça",
        "codigo_de_outra_peca" => "O código desta nota da loja já é de uma peça com outra descrição",
        "peca_pelo_gtin" => {
            "O GTIN aponta para uma peça cadastrada; a importação não estava marcada para mexer em peças"
        }
        _ => "Peça ainda não identificada no cadastro para este fornecedor",
    }
}

pub fn listar_pendencias(conn: &Connection) -> Resultado<Vec<PendenciaXml>> {
    let mut stmt = conn.prepare(
        "SELECT id, COALESCE(fornecedor_id, ''), fornecedor_documento, fornecedor_nome, chave_nfe, numero_nfe,
                emissao_nfe, codigo_fornecedor, gtin_comercial, origem_comercial, gtin_tributavel, origem_tributavel,
                gtins_json, descricao, ncm, cest, cfop, unidade, quantidade, valor_unitario, motivo, candidatos_json,
                criado_em
           FROM xml_pendencia_identificacao
          WHERE situacao = 'pendente'
          ORDER BY criado_em DESC, id ASC",
    )?;
    /// A linha da pendência como a lista lê (A16: no lugar da tupla de 23 campos).
    struct LinhaPendente {
        id: String,
        fornecedor_id: String,
        fornecedor_doc: String,
        fornecedor_nome: String,
        chave_nfe: String,
        numero_nfe: String,
        emissao_nfe: String,
        codigo_fornecedor: String,
        gtin_comercial: String,
        origem_comercial: String,
        gtin_tributavel: String,
        origem_tributavel: String,
        gtins_json: String,
        descricao: String,
        ncm: String,
        cest: String,
        cfop: String,
        unidade: String,
        quantidade: i64,
        valor_unitario: i64,
        motivo: String,
        candidatos_json: String,
        criado_em: String,
    }
    let rows: Vec<LinhaPendente> = stmt
        .query_map([], |l| {
            Ok(LinhaPendente {
                id: l.get(0)?,
                fornecedor_id: l.get(1)?,
                fornecedor_doc: l.get(2)?,
                fornecedor_nome: l.get(3)?,
                chave_nfe: l.get(4)?,
                numero_nfe: l.get(5)?,
                emissao_nfe: l.get(6)?,
                codigo_fornecedor: l.get(7)?,
                gtin_comercial: l.get(8)?,
                origem_comercial: l.get(9)?,
                gtin_tributavel: l.get(10)?,
                origem_tributavel: l.get(11)?,
                gtins_json: l.get(12)?,
                descricao: l.get(13)?,
                ncm: l.get(14)?,
                cest: l.get(15)?,
                cfop: l.get(16)?,
                unidade: l.get(17)?,
                quantidade: l.get(18)?,
                valor_unitario: l.get(19)?,
                motivo: l.get(20)?,
                candidatos_json: l.get(21)?,
                criado_em: l.get(22)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    let mut lista = Vec::with_capacity(rows.len());
    for LinhaPendente {
        id,
        fornecedor_id,
        fornecedor_doc,
        fornecedor_nome,
        chave_nfe,
        numero_nfe,
        emissao_nfe,
        codigo_fornecedor,
        gtin_comercial,
        origem_comercial,
        gtin_tributavel,
        origem_tributavel,
        gtins_json,
        descricao,
        ncm,
        cest,
        cfop,
        unidade,
        quantidade,
        valor_unitario,
        motivo,
        candidatos_json,
        criado_em,
    } in rows
    {
        let gtins_xml: Vec<nfe_xml::GtinXml> = serde_json::from_str(&gtins_json).unwrap_or_default();
        let fallback = if !gtin_comercial.is_empty() { &gtin_comercial } else { &gtin_tributavel };
        let gtins = gtins_para_ficha(&gtins_xml, fallback);
        let ids_cand: Vec<String> = serde_json::from_str(&candidatos_json).unwrap_or_default();
        let mut candidatos = Vec::new();
        for cid in ids_cand {
            if let Some((codigo, nome, marca, codigo_barras)) = conn
                .query_row("SELECT codigo, nome, marca, codigo_barras FROM produto WHERE id = ?1", [&cid], |l| {
                    Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?))
                })
                .optional()?
            {
                candidatos.push(CandidatoPendencia { id: cid, codigo, nome, marca, codigo_barras });
            }
        }
        lista.push(PendenciaXml {
            id,
            fornecedor_id,
            fornecedor_documento: documento::escrever(&fornecedor_doc),
            fornecedor_nome,
            chave_nfe,
            numero_nfe,
            emissao_nfe: crate::fiscal::data_br(&emissao_nfe),
            codigo_fornecedor,
            gtin_comercial,
            origem_comercial,
            gtin_tributavel,
            origem_tributavel,
            gtins,
            descricao,
            ncm,
            cest,
            cfop,
            unidade,
            quantidade: numeros::escrever_quantidade(quantidade),
            valor_unitario: numeros::escrever_dinheiro(valor_unitario),
            motivo_texto: motivo_legivel(&motivo).to_string(),
            motivo,
            candidatos,
            criado_em,
        });
    }
    Ok(lista)
}

pub fn resolver_pendencia(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    d: &ResolverPendencia,
) -> Resultado<PendenciaResolvida> {
    if d.id.trim().is_empty() {
        return Err(Erro::validacao("Escolha a pendência de identificação."));
    }
    /// A pendência como a resolução lê (A16: no lugar da tupla de 16 campos).
    struct PendenciaGravada {
        fornecedor_id: Option<String>,
        fornecedor_doc: String,
        fornecedor_nome: String,
        codigo_fornecedor: String,
        gtin_comercial: String,
        gtin_tributavel: String,
        gtins_json: String,
        descricao: String,
        ncm: String,
        cest: String,
        cfop: String,
        unidade: String,
        origem_fiscal: i64,
        cst_pis: String,
        valor_unitario: i64,
        situacao: String,
    }
    let linha: Option<PendenciaGravada> = conn
        .query_row(
            "SELECT fornecedor_id, fornecedor_documento, fornecedor_nome, codigo_fornecedor, gtin_comercial,
                    gtin_tributavel, gtins_json, descricao, ncm, cest, cfop, unidade, origem_fiscal, cst_pis,
                    valor_unitario, situacao
               FROM xml_pendencia_identificacao
              WHERE id = ?1",
            [&d.id],
            |l| {
                Ok(PendenciaGravada {
                    fornecedor_id: l.get(0)?,
                    fornecedor_doc: l.get(1)?,
                    fornecedor_nome: l.get(2)?,
                    codigo_fornecedor: l.get(3)?,
                    gtin_comercial: l.get(4)?,
                    gtin_tributavel: l.get(5)?,
                    gtins_json: l.get(6)?,
                    descricao: l.get(7)?,
                    ncm: l.get(8)?,
                    cest: l.get(9)?,
                    cfop: l.get(10)?,
                    unidade: l.get(11)?,
                    origem_fiscal: l.get(12)?,
                    cst_pis: l.get(13)?,
                    valor_unitario: l.get(14)?,
                    situacao: l.get(15)?,
                })
            },
        )
        .optional()?;
    let Some(PendenciaGravada {
        fornecedor_id,
        fornecedor_doc,
        fornecedor_nome,
        codigo_fornecedor,
        gtin_comercial,
        gtin_tributavel,
        gtins_json,
        descricao,
        ncm,
        cest,
        cfop: _cfop,
        unidade,
        origem_fiscal,
        cst_pis,
        valor_unitario,
        situacao,
    }) = linha
    else {
        return Err(Erro::NaoEncontrado("Pendência de identificação".into()));
    };
    if situacao == "resolvida" {
        return Err(Erro::validacao("Esta pendência já foi resolvida."));
    }

    let gtins_xml: Vec<nfe_xml::GtinXml> = serde_json::from_str(&gtins_json).unwrap_or_default();
    let fallback = if !gtin_comercial.is_empty() { &gtin_comercial } else { &gtin_tributavel };
    let gtins_ficha = gtins_para_ficha(&gtins_xml, fallback);

    let acao = d.acao.trim();
    let produto_id = match acao {
        "ligar" => {
            if d.produto_id.trim().is_empty() {
                return Err(Erro::campo("produtoId", "Escolha a peça para associar."));
            }
            let peca: Option<(String, String, bool)> = conn
                .query_row("SELECT nome, codigo, ativo FROM produto WHERE id = ?1", [&d.produto_id], |l| {
                    Ok((l.get(0)?, l.get(1)?, l.get::<_, i64>(2)? != 0))
                })
                .optional()?;
            let Some((nome, codigo, ativa)) = peca else {
                return Err(Erro::NaoEncontrado("Peça".into()));
            };
            // A07: ligar a uma peça desativada a faria voltar pelas próximas notas, sem ninguém ver.
            if !ativa {
                return Err(Erro::campo(
                    "produtoId",
                    format!("A peça \"{nome}\" (código {codigo}) está desativada. Reative a peça ou escolha outra."),
                ));
            }
            d.produto_id.trim().to_string()
        }
        "nova" => {
            let mut f = produtos::ficha_nova(conn)?;
            f.nome = texto::limpar(&descricao).chars().take(120).collect();
            if f.nome.is_empty() {
                f.nome = "Peça sem nome".into();
            }
            if !fallback.is_empty() && gtin::gtin_valido(fallback) && peca_por_barras(conn, fallback)?.is_none() {
                f.codigo_barras = fallback.to_string();
            }
            f.gtins = gtins_ficha.clone();
            f.unidade = crate::unidades::de_fora(&unidade);
            if ncm.len() == 8 && ncm.chars().all(|c| c.is_ascii_digit()) {
                f.ncm = ncm;
            }
            if cest.len() == 7 && cest.chars().all(|c| c.is_ascii_digit()) {
                f.cest = cest;
            }
            if (0..produtos::ORIGENS.len() as i64).contains(&origem_fiscal) {
                f.origem = origem_fiscal;
            }
            f.monofasico = cst_pis == "04";
            if valor_unitario > 0 {
                f.custo = numeros::escrever_dinheiro(valor_unitario);
                if let Some(p) = f.precos.iter_mut().find(|p| p.tabela == 1) {
                    if p.lucro.is_empty() {
                        p.lucro = "100".into();
                    }
                }
            }
            salvar_peca(conn, dispositivo, usuario_id, f)?
        }
        _ => {
            return Err(Erro::validacao("Escolha se quer associar a uma peça existente ou cadastrar como peça nova."));
        }
    };

    // Garante o vínculo do fornecedor (se houver fornecedor identificado) e os GTINs.
    let fid = match fornecedor_id {
        Some(id) if !id.is_empty() => Some(id),
        _ if !fornecedor_doc.is_empty() => pessoa_existe(conn, &fornecedor_doc)?,
        _ => None,
    };
    if let Some(fid) = &fid {
        let compacto = texto::compactar(&codigo_fornecedor);
        if !compacto.is_empty() {
            crate::catalogo::codigos::vincular_fornecedor(
                conn,
                fid,
                codigo_fornecedor.trim(),
                &produto_id,
                1000,
                crate::catalogo::codigos::JaLigado::PassarPeca,
            )?;
            produtos::atualizar_indice_busca(conn, &produto_id)?;
        }
    }
    produtos::registrar_gtins_xml(conn, &produto_id, &gtins_ficha)?;

    let agora = tempo::agora();
    crate::compras::pendencias::marcar_resolvida(conn, &d.id, &produto_id, &agora)?;
    if let Some(fid) = &fid {
        resolver_pendencias_do_item(conn, fid, &codigo_fornecedor, &descricao, &produto_id)?;
    }

    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "produto.importar",
        "produto",
        &produto_id,
        &[format!(
            "Pendência XML resolvida ({}) · fornecedor {} · código {}",
            if acao == "nova" { "peça nova" } else { "associada" },
            if fornecedor_nome.is_empty() { &fornecedor_doc } else { &fornecedor_nome },
            if codigo_fornecedor.is_empty() { "—" } else { &codigo_fornecedor }
        )],
    )?;

    Ok(PendenciaResolvida { produto_id, pendentes: contar_pendencias(conn) })
}

// ------------------------------------------------------------- comandos das telas

pub use comandos::COMANDOS;

/// Os comandos dos cadastros pelo XML das notas (etapa 3 da modularização: vieram do `match` do
/// `api.rs` sem mudar nome, entrada, resposta nem permissão).
mod comandos {
    use serde_json::{json, Value};

    use crate::despacho::{ler, Classe, Comando, Pedido};
    use crate::erro::Resultado;
    use crate::{importar, requisitos};

    pub const COMANDOS: &[Comando] = &[
        Comando { nome: "importar_previa", classe: Classe::Leitura, funcao: importar_previa },
        Comando { nome: "importar_cadastros", classe: Classe::Gerente("configuracoes"), funcao: importar_cadastros },
        Comando { nome: "arquivos_conferir", classe: Classe::Leitura, funcao: arquivos_conferir },
        Comando { nome: "xml_pendencias_listar", classe: Classe::Leitura, funcao: xml_pendencias_listar },
        Comando { nome: "xml_pendencia_resolver", classe: Classe::Caixa, funcao: xml_pendencia_resolver },
    ];

    fn importar_previa(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: importar::Entrada = ler(dados)?;
        Ok(json!(importar::previa(p.conn, &d.arquivos)?))
    }

    fn importar_cadastros(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: importar::Entrada = ler(dados)?;
        let tx = p.conn.transaction()?;
        let r = importar::importar(&tx, p.disp, p.usuario_id, &d)?;
        tx.commit()?;
        Ok(json!(r))
    }

    fn arquivos_conferir(_p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: importar::Entrada = ler(dados)?;
        Ok(json!(requisitos::arquivos(&d.arquivos)))
    }

    fn xml_pendencias_listar(p: &mut Pedido, _dados: Value) -> Resultado<Value> {
        Ok(json!(importar::listar_pendencias(p.conn)?))
    }

    fn xml_pendencia_resolver(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: importar::ResolverPendencia = ler(dados)?;
        let tx = p.conn.transaction()?;
        let r = importar::resolver_pendencia(&tx, p.disp, p.usuario_id, &d)?;
        tx.commit()?;
        Ok(json!(r))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::{config, usuarios};

    /// A01 + G04 — um código só de símbolos ("---"), sem GTIN que ache a peça,
    /// vira pendência própria, com ou sem "Peças do fornecedor": nenhuma peça
    /// nova, nenhum vínculo, e importar a mesma nota de novo não duplica nada.
    /// Não há nota assinada com esse código (e as chaves da PKI de teste foram
    /// descartadas): o caso é montado como sairia da leitura.
    #[test]
    fn a01_codigo_so_de_simbolos_vira_pendencia_e_nao_peca() {
        const FORN: &str = "11222333000181";
        for pecas_fornecedor in [false, true] {
            let b = Banco::em_memoria().unwrap();
            let (c, d) = (&b.conn, b.dispositivo.as_str());
            let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
            crate::faixas::comecar_loja(c).unwrap();
            config::gravar(c, "loja_cnpj", "11444777000161").unwrap();
            let emitente =
                Emitente { documento: FORN.into(), nome: "FORNECEDOR DE TESTE".into(), ..Default::default() };
            let mut lido = Lido::default();
            lido.fornecedores.insert(FORN.into(), (emitente, "2026-11-01".into()));
            let it = ItemXml {
                codigo: "---".into(),
                descricao: "PECA SO DE SIMBOLOS".into(),
                quantidade: 1000,
                valor_produtos: 5000,
                ..Default::default()
            };
            lido.pecas_fornecedor.insert((FORN.into(), "---".into(), descricao_chave(&it)), (it, "2026-11-01".into()));
            let e = Entrada { fornecedores: true, pecas_fornecedor, ..Default::default() };
            let conta = |sql: &str| -> i64 { c.query_row(sql, [], |l| l.get(0)).unwrap() };
            for vez in 1..=2 {
                gravar(c, d, &u, &e, &lido).unwrap();
                let pendencias = conta(
                    "SELECT COUNT(*) FROM xml_pendencia_identificacao
                      WHERE situacao = 'pendente' AND motivo = 'codigo_so_de_simbolos'",
                );
                assert_eq!(
                    (
                        conta("SELECT COUNT(*) FROM produto"),
                        conta("SELECT COUNT(*) FROM produto_fornecedor"),
                        pendencias
                    ),
                    (0, 0, 1),
                    "\"Peças do fornecedor\" = {pecas_fornecedor}, {vez}ª importação: (peças, vínculos, pendências)"
                );
            }
        }
    }

    /// Regressão do A03a (lote 1, `24ceb31`; não acontecia em `310cee0`): sem
    /// "Peças do fornecedor", o item cujo código do fornecedor JÁ está ligado à
    /// mesma peça que o GTIN acha virava pendência `peca_pelo_gtin` (nada a
    /// decidir: a peça e o vínculo já estão certos). A matriz confere também
    /// que as pendências legítimas da DP-11 continuam: o item achado só pelo
    /// GTIN (sem vínculo), a peça nova sem a opção, o código só de símbolos e o
    /// conflito entre GTIN e vínculo. Cada caso num banco novo.
    #[test]
    fn a03a_item_ja_ligado_sem_a_opcao_nao_vira_pendencia() {
        const FORN: &str = "11222333000181";
        const G_P: &str = "7891234567895"; // GTIN da peça P (ligada ao código C1)
        const G_Q: &str = "7890000000017"; // GTIN da peça Q (sem vínculo)

        // (caso, código, GTIN do item, com a opção, sem a opção); o resultado é
        // (peças novas, vínculos novos, GTINs novos em P ou Q, pendências por motivo).
        // Com a opção, o GTIN da nota é registrado na peça (origem cEAN) mesmo
        // quando é igual ao código de barras dela: é o comportamento de hoje
        // (A03/H02), fora da regressão.
        type Esperado = (i64, i64, i64, &'static [(&'static str, i64)]);
        let casos: &[(&str, &str, &str, Esperado, Esperado)] = &[
            ("já ligado à mesma peça", "C1", G_P, (0, 0, 1, &[]), (0, 0, 0, &[])),
            ("achado só pelo GTIN", "C2", G_Q, (0, 1, 1, &[]), (0, 0, 0, &[("peca_pelo_gtin", 1)])),
            ("peça nova", "C3", "", (1, 1, 0, &[]), (0, 0, 0, &[("sem_vinculo", 1)])),
            (
                "código só de símbolos",
                "---",
                "",
                (0, 0, 0, &[("codigo_so_de_simbolos", 1)]),
                (0, 0, 0, &[("codigo_so_de_simbolos", 1)]),
            ),
            (
                "conflito GTIN × vínculo",
                "C1",
                G_Q,
                (0, 0, 0, &[("conflito_gtin_fornecedor", 1)]),
                (0, 0, 0, &[("conflito_gtin_fornecedor", 1)]),
            ),
        ];
        let mut erros = Vec::new();
        for (nome, codigo, gtin, com, sem) in casos {
            for (pecas_fornecedor, esperado) in [(true, com), (false, sem)] {
                let b = Banco::em_memoria().unwrap();
                let (c, d) = (&b.conn, b.dispositivo.as_str());
                let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
                crate::faixas::comecar_loja(c).unwrap();
                config::gravar(c, "loja_cnpj", "11444777000161").unwrap();
                let emitente =
                    Emitente { documento: FORN.into(), nome: "FORNECEDOR DE TESTE".into(), ..Default::default() };
                let (fid, _) = crate::pessoas::garantir_pela_nota(c, d, &u, &emitente, false, "teste").unwrap();
                let peca = |nome: &str, barras: &str| {
                    let mut f = produtos::ficha_nova(c).unwrap();
                    f.nome = nome.into();
                    f.codigo_barras = barras.into();
                    f.precos[0].preco = "10,00".into();
                    produtos::salvar(c, d, &u, &f).unwrap()
                };
                let p = peca("PECA P LIGADA", G_P);
                peca("PECA Q SEM VINCULO", G_Q);
                crate::catalogo::codigos::vincular_fornecedor(
                    c,
                    &fid,
                    "C1",
                    &p,
                    1000,
                    crate::catalogo::codigos::JaLigado::Manter,
                )
                .unwrap();
                let conta = |sql: &str| -> i64 { c.query_row(sql, [], |l| l.get(0)).unwrap() };
                let antes = (
                    conta("SELECT COUNT(*) FROM produto"),
                    conta("SELECT COUNT(*) FROM produto_fornecedor"),
                    conta("SELECT COUNT(*) FROM produto_gtin"),
                );
                let mut lido = Lido::default();
                lido.fornecedores.insert(FORN.into(), (emitente, "2026-11-01".into()));
                let mut it = ItemXml {
                    codigo: (*codigo).into(),
                    descricao: format!("ITEM {nome}").to_uppercase(),
                    quantidade: 1000,
                    valor_produtos: 5000,
                    ..Default::default()
                };
                if !gtin.is_empty() {
                    it.gtin = (*gtin).into();
                    it.gtins = vec![nfe_xml::GtinXml {
                        codigo: (*gtin).into(),
                        tipo: "comercial".into(),
                        origem: "cEAN".into(),
                    }];
                }
                lido.pecas_fornecedor
                    .insert((FORN.into(), (*codigo).into(), descricao_chave(&it)), (it, "2026-11-01".into()));
                let e = Entrada { fornecedores: true, pecas_fornecedor, ..Default::default() };
                gravar(c, d, &u, &e, &lido).unwrap();
                let mut pend: Vec<(String, i64)> = c
                    .prepare("SELECT motivo, COUNT(*) FROM xml_pendencia_identificacao WHERE situacao = 'pendente' GROUP BY motivo ORDER BY motivo")
                    .unwrap()
                    .query_map([], |l| Ok((l.get(0)?, l.get(1)?)))
                    .unwrap()
                    .map(|x| x.unwrap())
                    .collect();
                pend.sort();
                let obtido = (
                    conta("SELECT COUNT(*) FROM produto") - antes.0,
                    conta("SELECT COUNT(*) FROM produto_fornecedor") - antes.1,
                    conta("SELECT COUNT(*) FROM produto_gtin") - antes.2,
                    pend,
                );
                let quer: Vec<(String, i64)> = esperado.3.iter().map(|(m, n)| (m.to_string(), *n)).collect();
                if (obtido.0, obtido.1, obtido.2, &obtido.3) != (esperado.0, esperado.1, esperado.2, &quer) {
                    erros.push(format!(
                        "{nome}, \"Peças do fornecedor\" = {pecas_fornecedor}: esperado {:?}, obtido {:?}",
                        (esperado.0, esperado.1, esperado.2, &quer),
                        obtido
                    ));
                }
            }
        }
        assert!(erros.is_empty(), "\n{}", erros.join("\n"));
    }

    /// A14: o mapa das descrições (montado uma vez) acha também a peça criada
    /// NESTA importação depois de montado, como a consulta de antes (que lia a
    /// tabela a cada item). Os códigos K1 e K2 se repetem em descrições
    /// diferentes; K2 "AAA" tem de achar a peça "AAA" que K1 acabou de criar.
    #[test]
    fn a14_descricao_acha_a_peca_criada_na_mesma_importacao() {
        const FORN: &str = "11222333000181";
        let b = Banco::em_memoria().unwrap();
        let (c, d) = (&b.conn, b.dispositivo.as_str());
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        crate::faixas::comecar_loja(c).unwrap();
        config::gravar(c, "loja_cnpj", "11444777000161").unwrap();
        let emitente = Emitente { documento: FORN.into(), nome: "FORNECEDOR DE TESTE".into(), ..Default::default() };
        let mut lido = Lido::default();
        lido.fornecedores.insert(FORN.into(), (emitente, "2026-11-01".into()));
        for (codigo, descricao) in [("K1", "PECA AAA"), ("K1", "PECA BBB"), ("K2", "PECA AAA"), ("K2", "PECA CCC")] {
            let it = ItemXml {
                codigo: codigo.into(),
                descricao: descricao.into(),
                quantidade: 1000,
                valor_produtos: 5000,
                ..Default::default()
            };
            lido.pecas_fornecedor.insert((FORN.into(), codigo.into(), descricao_chave(&it)), (it, "2026-11-01".into()));
        }
        let e = Entrada { fornecedores: true, pecas_fornecedor: true, ..Default::default() };
        gravar(c, d, &u, &e, &lido).unwrap();
        let nomes: Vec<String> = c
            .prepare("SELECT nome FROM produto ORDER BY nome")
            .unwrap()
            .query_map([], |l| l.get(0))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
        assert_eq!(nomes, ["PECA AAA", "PECA BBB", "PECA CCC"], "uma peça por descrição, sem a AAA repetida");
    }

    /// Nota de venda da própria loja, assinada pela PKI de teste, sem
    /// protocolo (cadastro por XML exige só a assinatura do emitente, D8).
    const NOTA_LOJA: &str = include_str!("../../tests/pki/xml/venda_da_loja_sem_protocolo.xml");

    #[test]
    fn importa_clientes_pecas_e_fornecedores() {
        let b = Banco::em_memoria().unwrap();
        let (c, d) = (&b.conn, b.dispositivo.as_str());
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        // Loja configurada (como na primeira tela): este caixa tem a faixa 1 a 99.999.
        crate::faixas::comecar_loja(c).unwrap();
        let pasta = std::env::temp_dir().join(format!("rustinho-importar-{}", tempo::novo_id()));
        std::fs::create_dir_all(pasta.join("sub")).unwrap();
        std::fs::write(pasta.join("venda.xml"), NOTA_LOJA).unwrap();
        std::fs::write(pasta.join("sub").join("compra.xml"), nfe_xml::EXEMPLO).unwrap();
        std::fs::write(pasta.join("evento.xml"), "<procEventoNFe><evento/></procEventoNFe>").unwrap();
        let arquivos = vec![pasta.display().to_string()];

        // Sem o CNPJ da loja não dá para separar as notas.
        assert!(previa(c, &arquivos).is_err());
        config::gravar(c, "loja_cnpj", "11444777000161").unwrap();

        let p = previa(c, &arquivos).unwrap();
        assert_eq!((p.arquivos, p.notas, p.notas_loja, p.notas_fornecedor, p.ignorados), (3, 2, 1, 1, 1));
        assert_eq!((p.clientes_novos, p.fornecedores_novos), (1, 1));
        assert_eq!(p.pecas_loja_novas, 1);
        assert_eq!(p.pecas_fornecedor_novas, 3);
        assert_eq!(p.exemplos_clientes, vec!["JOAO DA SILVA".to_string()]);

        let e = Entrada {
            arquivos: arquivos.clone(),
            clientes: true,
            fornecedores: true,
            pecas_loja: true,
            pecas_fornecedor: true,
        };
        let r = importar(c, d, &u, &e).unwrap();
        assert_eq!((r.clientes, r.fornecedores, r.pecas), (1, 1, 4));

        // A peça da loja ficou com o código e o preço da nota.
        let (codigo, preco): (String, i64) = c
            .query_row(
                "SELECT p.codigo, pp.preco FROM produto p JOIN produto_preco pp ON pp.produto_id = p.id AND pp.tabela = 1
                  WHERE p.nome = 'FILTRO DE OLEO PSL55'",
                [],
                |l| Ok((l.get(0)?, l.get(1)?)),
            )
            .unwrap();
        assert_eq!((codigo.as_str(), preco), ("4455", 3000));
        // O cliente veio com o e-mail e a cidade.
        let (cliente, cidade, email): (bool, String, String) = c
            .query_row("SELECT cliente, municipio, email FROM pessoa WHERE documento = '52998224725'", [], |l| {
                Ok((l.get(0)?, l.get(1)?, l.get(2)?))
            })
            .unwrap();
        assert!(cliente);
        assert_eq!((cidade.as_str(), email.as_str()), ("AVARE", "joao@exemplo.com"));

        // De novo: nada para cadastrar.
        let p = previa(c, &arquivos).unwrap();
        assert_eq!(
            (p.clientes_novos, p.fornecedores_novos, p.pecas_loja_novas, p.pecas_fornecedor_novas),
            (0, 0, 0, 0)
        );
        assert_eq!(p.pecas_ja, 4);
        let r = importar(c, d, &u, &e).unwrap();
        assert_eq!((r.clientes, r.fornecedores, r.pecas), (0, 0, 0));

        // Os mesmos XMLs dentro de um .zip (e a mesma nota não conta duas vezes).
        let zip_caminho = pasta.join("notas.zip");
        {
            use std::io::Write;
            let mut z = zip::ZipWriter::new(std::fs::File::create(&zip_caminho).unwrap());
            let op = zip::write::SimpleFileOptions::default();
            z.start_file("2026/09/venda.xml", op).unwrap();
            z.write_all(NOTA_LOJA.as_bytes()).unwrap();
            z.start_file("leia-me.txt", op).unwrap();
            z.write_all(b"nada").unwrap();
            z.finish().unwrap();
        }
        let p = previa(c, &[format!("file://{}", zip_caminho.display())]).unwrap();
        assert_eq!((p.arquivos, p.notas_loja, p.clientes_ja, p.pecas_ja), (1, 1, 1, 1));
        let p = previa(c, &arquivos).unwrap();
        assert_eq!((p.arquivos, p.notas), (4, 2));
        std::fs::remove_dir_all(&pasta).ok();
    }
    /// A-43 (D8): cadastro por XML exige a assinatura do emitente (não a
    /// autorização) e não mexe em estoque, custo nem financeiro. A nota
    /// adulterada não cadastra nada.
    #[test]
    fn a43_cadastro_por_xml_so_com_assinatura_e_sem_efeito_financeiro() {
        let b = Banco::em_memoria().unwrap();
        let (c, d) = (&b.conn, b.dispositivo.as_str());
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        crate::faixas::comecar_loja(c).unwrap();
        config::gravar(c, "loja_cnpj", "11444777000161").unwrap();
        let pasta = std::env::temp_dir().join(format!("rustinho-a43-{}", tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let adulterada = NOTA_LOJA.replace("JOAO DA SILVA", "JOAO DA SILVA FALSO");
        std::fs::write(pasta.join("adulterada.xml"), &adulterada).unwrap();
        let arquivos = vec![pasta.display().to_string()];
        let e = Entrada {
            arquivos: arquivos.clone(),
            clientes: true,
            fornecedores: true,
            pecas_loja: true,
            pecas_fornecedor: true,
        };
        let conta = |t: &str| -> i64 { c.query_row(&format!("SELECT count(*) FROM {t}"), [], |l| l.get(0)).unwrap() };
        let financeiro = || {
            (
                conta("estoque_movimento"),
                conta("conta_pagar"),
                conta("conta_receber"),
                conta("compra"),
                c.query_row("SELECT COALESCE(sum(custo), 0) FROM produto", [], |l| l.get::<_, i64>(0)).unwrap(),
            )
        };
        let (pessoas, pecas, fin) = (conta("pessoa"), conta("produto"), financeiro());
        let p = previa(c, &arquivos).unwrap();
        assert_eq!((p.notas, p.ignorados), (0, 1), "adulterada é ignorada");
        assert!(p.avisos.iter().any(|a| a.contains("alterada depois da assinatura")), "{:?}", p.avisos);
        let r = importar(c, d, &u, &e).unwrap();
        assert_eq!((r.clientes, r.fornecedores, r.pecas), (0, 0, 0));
        assert_eq!((conta("pessoa"), conta("produto")), (pessoas, pecas));
        // A original (assinada, sem protocolo): cadastra, sem efeito no financeiro.
        std::fs::remove_file(pasta.join("adulterada.xml")).unwrap();
        std::fs::write(pasta.join("venda.xml"), NOTA_LOJA).unwrap();
        let r = importar(c, d, &u, &e).unwrap();
        assert_eq!((r.clientes, r.pecas), (1, 1));
        assert_eq!(financeiro(), fin, "cadastro não muda estoque, custo nem financeiro");
        std::fs::remove_dir_all(&pasta).ok();
    }

    #[test]
    fn pendencias_xml_multiplos_gtins_e_fornecedores() {
        let b = Banco::em_memoria().unwrap();
        let (c, d) = (&b.conn, b.dispositivo.as_str());
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        crate::faixas::comecar_loja(c).unwrap();
        config::gravar(c, "loja_cnpj", "11444777000161").unwrap();

        let pasta = std::env::temp_dir().join(format!("rustinho-pend-xml-{}", tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        std::fs::write(pasta.join("compra.xml"), nfe_xml::EXEMPLO).unwrap();
        let arquivos = vec![pasta.display().to_string()];

        // Importa só o fornecedor (sem cadastrar peças novas automaticamente):
        // os 3 itens da nota sem vínculo viram pendências persistentes de identificação!
        let r = importar(
            c,
            d,
            &u,
            &Entrada {
                arquivos: arquivos.clone(),
                clientes: false,
                fornecedores: true,
                pecas_loja: false,
                pecas_fornecedor: false,
            },
        )
        .unwrap();
        assert_eq!((r.fornecedores, r.pecas, r.codigos_ligados), (1, 0, 0));
        assert_eq!(contar_pendencias(c), 3);

        // Reimportar o mesmo XML não cria duplicações de pendências.
        let r2 = importar(
            c,
            d,
            &u,
            &Entrada {
                arquivos: arquivos.clone(),
                clientes: false,
                fornecedores: true,
                pecas_loja: false,
                pecas_fornecedor: false,
            },
        )
        .unwrap();
        assert_eq!((r2.fornecedores, r2.pecas, r2.codigos_ligados), (0, 0, 0));
        assert_eq!(contar_pendencias(c), 3, "reimportar não duplica pendências");

        // Apenas listar/abrir as pendências não resolve nenhuma.
        let lista = listar_pendencias(c).unwrap();
        assert_eq!(lista.len(), 3);
        assert_eq!(contar_pendencias(c), 3);

        // Resolve a 1ª pendência criando peça nova: não confunde cProd com código original.
        let p0 = lista.iter().find(|p| p.codigo_fornecedor == "FAG-32206").unwrap();
        assert_eq!(p0.gtin_comercial, "7891234567895");
        assert_eq!(p0.origem_comercial, "cEAN");
        let res0 = resolver_pendencia(
            c,
            d,
            &u,
            &ResolverPendencia { id: p0.id.clone(), acao: "nova".into(), produto_id: String::new() },
        )
        .unwrap();
        assert_eq!(res0.pendentes, 2);
        let ficha0 = produtos::ficha(c, &res0.produto_id).unwrap();
        assert_eq!(ficha0.codigo_original, "", "cProd nunca vira código original do fabricante");
        assert_eq!(ficha0.fornecedores.len(), 1);
        assert_eq!(ficha0.fornecedores[0].codigo, "FAG-32206");
        assert!(ficha0.gtins.iter().any(|g| g.gtin == "7891234567895" && g.tipo == "comercial" && g.origem == "cEAN"));

        // Outro fornecedor com código diferente para a mesma peça: preserva ambos os vínculos.
        let mut forn2 = pessoas::ficha_nova(c, "fornecedor").unwrap();
        forn2.nome = "OUTRO FORNECEDOR LTDA".into();
        forn2.documento = "33.444.555/0001-81".into();
        let forn2_id = pessoas::salvar(c, d, &u, &forn2).unwrap();
        ligar_fornecedor(c, &forn2_id, "OUTRO-999", &res0.produto_id).unwrap();
        let ficha0 = produtos::ficha(c, &res0.produto_id).unwrap();
        assert_eq!(ficha0.fornecedores.len(), 2, "mesma peça com códigos diferentes em fornecedores diferentes");

        // Resolve a 2ª pendência ligando a uma peça já existente.
        let mut f_pas = produtos::ficha_nova(c).unwrap();
        f_pas.nome = "Pastilha dianteira existente".into();
        f_pas.codigo_original = "FAB-ORIG-1".into();
        f_pas.custo = "30,00".into();
        let pas_id = produtos::salvar(c, d, &u, &f_pas).unwrap();

        let p1 = lista.iter().find(|p| p.codigo_fornecedor == "N1100").unwrap();
        let res1 = resolver_pendencia(
            c,
            d,
            &u,
            &ResolverPendencia { id: p1.id.clone(), acao: "ligar".into(), produto_id: pas_id.clone() },
        )
        .unwrap();
        assert_eq!(res1.pendentes, 1);
        let ficha_pas = produtos::ficha(c, &pas_id).unwrap();
        assert_eq!(ficha_pas.codigo_original, "FAB-ORIG-1", "preserva código original existente");
        assert_eq!(ficha_pas.fornecedores.len(), 1);
        assert_eq!(ficha_pas.fornecedores[0].codigo, "N1100");

        std::fs::remove_dir_all(&pasta).ok();
    }

    /// A02 — "AB-12" e "AB12" do mesmo fornecedor, com descrições diferentes,
    /// são o mesmo código (o vínculo é pela forma compacta). Não há nota
    /// assinada com esse par (e as chaves da PKI de teste foram descartadas):
    /// o caso é montado como sairia da leitura. A peça já ligada ao AB12 não
    /// recebe GTIN de nenhum dos dois; sem "Peças do fornecedor", os dois viram
    /// pendência de código repetido; com ela, o vínculo do AB12 não muda.
    #[test]
    fn a02_ab12_e_ab_12_sao_o_mesmo_codigo() {
        const FORN: &str = "11222333000181";
        let caso = |pecas_fornecedor: bool| {
            let b = Banco::em_memoria().unwrap();
            let (c, d) = (&b.conn, b.dispositivo.as_str());
            let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
            crate::faixas::comecar_loja(c).unwrap();
            config::gravar(c, "loja_cnpj", "11444777000161").unwrap();
            let emitente =
                Emitente { documento: FORN.into(), nome: "FORNECEDOR DE TESTE".into(), ..Default::default() };
            let (fid, _) = pessoas::garantir_pela_nota(c, d, &u, &emitente, false, "teste").unwrap();
            let mut f = produtos::ficha_nova(c).unwrap();
            f.nome = "PECA JA LIGADA AO AB12".into();
            if let Some(p) = f.precos.iter_mut().find(|p| p.tabela == 1) {
                p.preco = "10,00".into();
                p.lucro = String::new();
            }
            let ligada = produtos::salvar(c, d, &u, &f).unwrap();
            crate::catalogo::codigos::vincular_fornecedor(
                c,
                &fid,
                "AB12",
                &ligada,
                1000,
                crate::catalogo::codigos::JaLigado::Manter,
            )
            .unwrap();
            let mut lido = Lido::default();
            lido.fornecedores.insert(FORN.into(), (emitente, "2026-11-01".into()));
            for (cod, desc, g) in [("AB-12", "ROLAMENTO A", "7891000000090"), ("AB12", "ROLAMENTO B", "7891000000113")]
            {
                let it = ItemXml {
                    codigo: cod.into(),
                    descricao: desc.into(),
                    gtin: g.into(),
                    quantidade: 1000,
                    valor_produtos: 5000,
                    ..Default::default()
                };
                lido.pecas_fornecedor
                    .insert((FORN.into(), cod.into(), descricao_chave(&it)), (it, "2026-11-01".into()));
            }
            let e = Entrada { fornecedores: true, pecas_fornecedor, ..Default::default() };
            gravar(c, d, &u, &e, &lido).unwrap();
            let conta = |sql: &str| -> i64 { c.query_row(sql, [&ligada], |l| l.get(0)).unwrap() };
            let gtins_na_ligada = conta("SELECT COUNT(*) FROM produto_gtin WHERE produto_id = ?1");
            let ab12_na_ligada =
                conta("SELECT COUNT(*) FROM produto_fornecedor WHERE produto_id = ?1 AND codigo_compacto = 'ab12'");
            let repetidas = conta(
                "SELECT COUNT(*) FROM xml_pendencia_identificacao
                  WHERE situacao = 'pendente' AND motivo = 'codigo_repetido' AND ?1 <> ''",
            );
            (gtins_na_ligada, ab12_na_ligada, repetidas)
        };
        assert_eq!(
            caso(false),
            (0, 1, 2),
            "sem \"Peças do fornecedor\": (GTINs na peça ligada, vínculo AB12, pendências repetidas)"
        );
        let (gtins, vinculo, _) = caso(true);
        assert_eq!((gtins, vinculo), (0, 1), "com \"Peças do fornecedor\": (GTINs na peça ligada, vínculo AB12)");
    }
}
