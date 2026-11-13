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

use rusqlite::{params, Connection, OptionalExtension};
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
    /// P-4: o carimbo da prévia que a tela mostrou. Se vier e não for o de
    /// agora (os arquivos ou os cadastros mudaram), nada é gravado.
    pub carimbo: String,
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
    /// Lote 2 (O-3, A03b): as opções desta prévia e o que a importação vai
    /// fazer com elas, pelas mesmas decisões (os campos de cima são, como
    /// antes, com todas as opções marcadas).
    pub opcoes: Opcoes,
    #[serde(flatten)]
    pub resumo: Resumo,
    /// P-4: mandar de volta no "Cadastrar".
    pub carimbo: String,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Importado {
    pub clientes: i64,
    pub fornecedores: i64,
    pub pecas: i64,
    pub codigos_ligados: i64,
    /// O resumo das decisões que foram aplicadas: o mesmo da prévia. Fica de
    /// fora da resposta (a trava T7 congela o resultado da importação); serve
    /// à trava "prévia = resultado".
    #[serde(skip)]
    pub resumo: Resumo,
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
    /// P-4: SHA-256 do conteúdo dos XMLs, na ordem em que foram lidos.
    assinatura: [u8; 32],
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
    let mut assinatura = {
        use sha2::Digest;
        sha2::Sha256::new()
    };
    let total = cada_xml(arquivos, |nome, xml| {
        {
            use sha2::Digest;
            assinatura.update(nome.as_bytes());
            assinatura.update([0u8]);
            assinatura.update(xml.as_bytes());
            assinatura.update([0u8]);
        }
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
    lido.assinatura = {
        use sha2::Digest;
        assinatura.finalize().into()
    };
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
    let codigos = codigos_da_busca(gtins, gtin_fallback);
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
    fn achar(&self, conn: &Connection, plano: &Plano, it: &ItemXml) -> Resultado<Option<String>> {
        let mut mapa = self.0.borrow_mut();
        if mapa.is_none() {
            let mut m: HashMap<String, String> = HashMap::new();
            let mut c = conn.prepare_cached("SELECT id, nome FROM produto")?;
            for linha in c.query_map([], |l| Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?)))? {
                let (id, nome) = linha?;
                m.entry(texto::compactar(&nome)).or_insert(id);
            }
            // As peças novas desta importação até aqui (na tabela, viriam depois).
            for (id, nome) in &plano.pecas {
                m.entry(texto::compactar(nome)).or_insert_with(|| id.clone());
            }
            *mapa = Some(m);
        }
        Ok(mapa.as_ref().and_then(|m| m.get(&descricao_chave(it)).cloned()))
    }

    /// Uma peça nova desta importação (com o nome que a ficha grava).
    fn nova(&self, id: &str, nome: &str) {
        if let Some(m) = self.0.borrow_mut().as_mut() {
            m.entry(texto::compactar(nome)).or_insert_with(|| id.to_string());
        }
    }
}

/// A peça da loja deste item já existe?
/// As opções de uma entrada.
fn opcoes_de(e: &Entrada) -> Opcoes {
    Opcoes {
        clientes: e.clientes,
        fornecedores: e.fornecedores,
        pecas_loja: e.pecas_loja,
        pecas_fornecedor: e.pecas_fornecedor,
    }
}

/// Todas as opções marcadas (P-5: a prévia sem opções supõe todas).
fn todas_as_opcoes(arquivos: &[String]) -> Entrada {
    Entrada {
        arquivos: arquivos.to_vec(),
        clientes: true,
        fornecedores: true,
        pecas_loja: true,
        pecas_fornecedor: true,
        carimbo: String::new(),
    }
}

/// A prévia com todas as opções marcadas (P-5).
pub fn previa(conn: &Connection, arquivos: &[String]) -> Resultado<Previa> {
    previa_com(conn, &todas_as_opcoes(arquivos))
}

/// A prévia (T-1): as MESMAS decisões da importação (`decidir`), sem gravar.
/// Os campos de antes saem das decisões com todas as opções marcadas (o que as
/// notas trazem); o resumo, com as opções desta prévia.
pub fn previa_com(conn: &Connection, e: &Entrada) -> Resultado<Previa> {
    let lido = ler_tudo(conn, &e.arquivos)?;
    let todas = todas_as_opcoes(&e.arquivos);
    let d_todas = decidir(conn, &todas, &lido)?;
    let opcoes = opcoes_de(e);
    let resumo = if opcoes == opcoes_de(&todas) {
        resumir(conn, &lido, &d_todas)?
    } else {
        resumir(conn, &lido, &decidir(conn, e, &lido)?)?
    };
    let mut p = Previa {
        arquivos: lido.arquivos,
        notas: lido.notas_loja + lido.notas_fornecedor,
        notas_loja: lido.notas_loja,
        notas_fornecedor: lido.notas_fornecedor,
        ignorados: lido.ignorados,
        avisos: lido.avisos.clone(),
        opcoes,
        carimbo: carimbo(conn, &opcoes, &lido)?,
        resumo,
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
    p.codigos_repetidos = (lido.repetidos_loja().len() + lido.repetidos_fornecedor().len()) as i64;
    let mut exemplos: Vec<String> = Vec::new();
    for (_, it, _, dec) in &d_todas.loja {
        if matches!(dec, DecisaoLoja::Nova { .. }) {
            p.pecas_loja_novas += 1;
            if exemplos.len() < 6 {
                exemplos.push(texto::limpar(&it.descricao));
            }
        } else {
            p.pecas_ja += 1;
        }
    }
    for i in &d_todas.forn {
        match &i.decisao {
            DecisaoForn::Nova { .. } => {
                p.pecas_fornecedor_novas += 1;
                if exemplos.len() < 6 {
                    exemplos.push(texto::limpar(&i.item.0.descricao));
                }
            }
            DecisaoForn::Pendencia { motivo, .. }
                if !matches!(
                    *motivo,
                    "gtin_ambiguo" | "fornecedor_ambiguo" | "conflito_gtin_fornecedor" | "peca_pelo_gtin"
                ) => {}
            DecisaoForn::Nada => {}
            _ => p.pecas_ja += 1,
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

/// A ficha da peça nova do item, com o código de barras que a decisão deu
/// (`barras_da_peca_nova`).
fn ficha_do_item(conn: &Connection, it: &ItemXml, barras: &str) -> Resultado<produtos::ProdutoFicha> {
    let mut f = produtos::ficha_nova(conn)?;
    f.nome = nome_da_peca(it);
    f.codigo_barras = barras.to_string();
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

/// "Trazer cadastros" dos XMLs, em etapas (ARQ-13): clientes, fornecedores,
/// peças da loja e peças dos fornecedores.
pub fn importar(conn: &Connection, dispositivo: &str, usuario_id: &str, e: &Entrada) -> Resultado<Importado> {
    let lido = ler_tudo(conn, &e.arquivos)?;
    gravar(conn, dispositivo, usuario_id, e, &lido)
}

/// Decide tudo (a mesma função da prévia) e aplica, na ordem das etapas.
/// Separada da leitura para os testes montarem casos sem nota assinada.
fn gravar(conn: &Connection, dispositivo: &str, usuario_id: &str, e: &Entrada, lido: &Lido) -> Resultado<Importado> {
    let d = decidir(conn, e, lido)?;
    if !e.carimbo.is_empty() && carimbo(conn, &d.opcoes, lido)? != e.carimbo {
        return Err(Erro::validacao(CARIMBO_VENCIDO));
    }
    let resumo = resumir(conn, lido, &d)?;
    let mut r = aplicar(Gravacao { conn, dispositivo, usuario_id }, lido, &d)?;
    r.resumo = resumo;
    Ok(r)
}

/// Os ids do plano viram os ids gravados à medida que peças e pessoas são criadas.
#[derive(Default)]
struct Reais(HashMap<String, String>);

impl Reais {
    fn de(&self, id: &str) -> String {
        self.0.get(id).cloned().unwrap_or_else(|| id.to_string())
    }
}

/// Aplica as decisões, na mesma ordem em que foram tomadas.
fn aplicar(gr: Gravacao, lido: &Lido, d: &Decisoes) -> Resultado<Importado> {
    let Gravacao { conn, dispositivo, usuario_id } = gr;
    let mut r = Importado::default();
    let mut reais = Reais::default();

    // Etapa 1: os clientes das notas da loja que ainda não estão cadastrados.
    for p in &d.clientes {
        if let Some(plano) = &p.nova {
            let (pessoa, _) = &lido.clientes[&p.doc];
            let (id, _) = pessoas::garantir_pela_nota(
                conn,
                dispositivo,
                usuario_id,
                pessoa,
                true,
                "Cadastrado pelo XML de uma nota da loja",
            )?;
            reais.0.insert(plano.clone(), id);
            r.clientes += 1;
        }
    }

    // Etapa 2: os fornecedores que faltam.
    for p in &d.fornecedores {
        if let Some(plano) = &p.nova {
            let (pessoa, _) = &lido.fornecedores[&p.doc];
            let (id, _) = pessoas::garantir_pela_nota(
                conn,
                dispositivo,
                usuario_id,
                pessoa,
                false,
                "Cadastrado pelo XML de uma nota de compra",
            )?;
            reais.0.insert(plano.clone(), id);
            r.fornecedores += 1;
        }
    }

    // Etapa 3: as peças das notas da loja.
    for (_, it, emissao, dec) in &d.loja {
        let gtins_ficha = gtins_para_ficha(&it.gtins, &it.gtin);
        match dec {
            DecisaoLoja::OutraDescricaoPeloGtin { pid, .. } => {
                produtos::registrar_gtins_xml(conn, &reais.de(pid), &gtins_ficha)?;
            }
            DecisaoLoja::OutraDescricaoPendencia { outra } => {
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
                    &[reais.de(outra)],
                )?;
            }
            DecisaoLoja::Existente { pid, .. } => {
                if let Some(pid) = pid {
                    produtos::registrar_gtins_xml(conn, &reais.de(pid), &gtins_ficha)?;
                }
            }
            DecisaoLoja::Nova { id: plano, codigo: codigo_peca, barras, codigo_antigo } => {
                let mut f = ficha_do_item(conn, it, barras)?;
                if !codigo_peca.is_empty() {
                    f.codigo = codigo_peca.clone();
                }
                let preco = valor_unitario(it, false);
                if preco > 0 {
                    if let Some(p) = f.precos.iter_mut().find(|p| p.tabela == 1) {
                        p.preco = numeros::escrever_dinheiro(preco);
                        p.lucro = String::new();
                    }
                }
                let id = salvar_peca(conn, dispositivo, usuario_id, f)?;
                reais.0.insert(plano.clone(), id.clone());
                produtos::registrar_gtins_xml(conn, &id, &gtins_ficha)?;
                let mut detalhes = vec!["Cadastrada pelo XML de uma nota da loja".to_string()];
                if !codigo_antigo.is_empty() {
                    let cod = codigo_antigo;
                    crate::catalogo::codigos::guardar_codigo_antigo(conn, cod, &id, &tempo::agora())?;
                    produtos::atualizar_indice_busca(conn, &id)?;
                    detalhes.push(format!(
                        "O código {cod} do sistema antigo está em mais de uma peça: ficou como código antigo"
                    ));
                    // A faixa deste caixa pode ter dado justamente esse número a
                    // esta peça (antes de ele virar código antigo): troca.
                    let atual: String =
                        conn.query_row("SELECT codigo FROM produto WHERE id = ?1", [&id], |l| l.get(0))?;
                    if texto::compactar(&atual) == texto::compactar(cod) {
                        produtos::trocar_codigo(conn, dispositivo, usuario_id, &id)?;
                    }
                }
                auditoria::registrar(
                    conn,
                    dispositivo,
                    Some(usuario_id),
                    "produto.importar",
                    "produto",
                    &id,
                    &detalhes,
                )?;
                r.pecas += 1;
            }
        }
    }

    // Etapa 4: as peças das notas de fornecedor.
    for i in &d.forn {
        let (forn, codigo, _) = i.chave;
        let (it, emissao) = i.item;
        let gtins_ficha = gtins_para_ficha(&it.gtins, &it.gtin);
        let fornecedor_id = i.fornecedor_id.as_deref().map(|f| reais.de(f));
        match &i.decisao {
            DecisaoForn::Pendencia { motivo, candidatos } => {
                let candidatos: Vec<String> = candidatos.iter().map(|c| reais.de(c)).collect();
                registrar_pendencia_xml(
                    conn,
                    fornecedor_id.as_deref(),
                    forn,
                    &i.forn_nome,
                    &i.chave_nfe,
                    &i.numero_nfe,
                    emissao,
                    it,
                    valor_unitario(it, true),
                    motivo,
                    &candidatos,
                )?;
            }
            DecisaoForn::JaLigadoFecha { pid } => {
                if let Some(fid) = &fornecedor_id {
                    resolver_pendencias_do_item(conn, fid, codigo, &it.descricao, &reais.de(pid))?;
                }
            }
            DecisaoForn::PeloGtin { pid, ligar, .. } => {
                let pid = reais.de(pid);
                produtos::registrar_gtins_xml(conn, &pid, &gtins_ficha)?;
                if !i.repetido {
                    if let Some(fid) = &fornecedor_id {
                        if *ligar && ligar_fornecedor(conn, fid, codigo, &pid)? {
                            r.codigos_ligados += 1;
                        }
                        resolver_pendencias_do_item(conn, fid, codigo, &it.descricao, &pid)?;
                    }
                }
            }
            DecisaoForn::PeloVinculo { pid, gtins, .. } => {
                let pid = reais.de(pid);
                if *gtins {
                    produtos::registrar_gtins_xml(conn, &pid, &gtins_ficha)?;
                }
                if let Some(fid) = &fornecedor_id {
                    resolver_pendencias_do_item(conn, fid, codigo, &it.descricao, &pid)?;
                }
            }
            DecisaoForn::MesmaDescricao { pid, .. } => {
                produtos::registrar_gtins_xml(conn, &reais.de(pid), &gtins_ficha)?;
            }
            DecisaoForn::Nova { id: plano, barras, ligar } => {
                let mut f = ficha_do_item(conn, it, barras)?;
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
                reais.0.insert(plano.clone(), id.clone());
                produtos::registrar_gtins_xml(conn, &id, &gtins_ficha)?;
                if *ligar {
                    if let Some(fid) = &fornecedor_id {
                        ligar_fornecedor(conn, fid, codigo, &id)?;
                        resolver_pendencias_do_item(conn, fid, codigo, &it.descricao, &id)?;
                    }
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
            }
            DecisaoForn::Nada => {}
        }
    }
    Ok(r)
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

// ---------------------------------------------------------------------------
// Lote 2, item 03a (T-1, P-3): decidir × aplicar. A prévia e a importação
// tomam as MESMAS decisões, pela mesma função, sobre o banco de antes mais o
// que a própria importação vai criar ou ligar (o `Plano`), sem gravar nada. A
// importação aplica as decisões depois, na mesma ordem. Assim a prévia não
// promete uma coisa e a importação faz outra.
// ---------------------------------------------------------------------------

/// Prefixo do id de uma peça ou pessoa que a importação ainda vai criar.
const PLANEJADO: &str = "plano:";

/// O que a importação vai criar ou ligar, como as decisões seguintes o veriam
/// no banco se ele já estivesse gravado.
#[derive(Default)]
struct Plano {
    /// As peças novas, na ordem de criação: (id do plano, nome).
    pecas: Vec<(String, String)>,
    /// Código (como fica gravado) → peças que passam a tê-lo.
    gtins: HashMap<String, Vec<String>>,
    /// Código da loja → a peça nova que fica com ele.
    codigos: HashMap<String, String>,
    /// Código antigo (código repetido da loja) → as peças novas que o guardam.
    codigos_antigos: HashMap<String, Vec<String>>,
    /// (documento do fornecedor, código compacto) → a peça ligada.
    vinculos: HashMap<(String, String), String>,
    /// Documento → a pessoa nova.
    pessoas: HashMap<String, String>,
}

impl Plano {
    fn nova_peca(&mut self, nome: &str) -> String {
        let id = format!("{PLANEJADO}{:07}", self.pecas.len() + 1);
        self.pecas.push((id.clone(), nome.to_string()));
        id
    }

    fn nova_pessoa(&mut self, doc: &str) -> String {
        let id = format!("{PLANEJADO}pessoa:{doc}");
        self.pessoas.insert(doc.to_string(), id.clone());
        id
    }

    fn nome(&self, id: &str) -> Option<&str> {
        self.pecas.iter().find(|(p, _)| p == id).map(|(_, n)| n.as_str())
    }

    fn dar_codigo(&mut self, codigo: String, pid: &str) {
        let v = self.gtins.entry(codigo).or_default();
        if !v.iter().any(|p| p == pid) {
            v.push(pid.to_string());
        }
    }
}

fn ids_de(b: BuscaId) -> Vec<String> {
    match b {
        BuscaId::Nenhum => Vec::new(),
        BuscaId::Unico(id) => vec![id],
        BuscaId::Ambiguo(v) => v,
    }
}

/// A ordem em que o banco devolveria: as peças que já existem pelo id (as
/// consultas são `ORDER BY` id) e as novas depois, na ordem de criação (o id
/// novo é maior que os de antes).
fn na_ordem_do_banco(ids: Vec<String>) -> Vec<String> {
    let (mut reais, novas): (Vec<String>, Vec<String>) = ids.into_iter().partition(|p| !p.starts_with(PLANEJADO));
    reais.sort();
    reais.extend(novas);
    reais
}

/// Os códigos que a busca por GTIN usa (os de `peca_por_gtins_inequivoco`).
fn codigos_da_busca(gtins: &[nfe_xml::GtinXml], gtin_fallback: &str) -> Vec<String> {
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
    codigos
}

/// `peca_por_gtins_inequivoco` sobre o banco mais o plano.
fn plano_pelos_gtins(
    conn: &Connection,
    plano: &Plano,
    gtins: &[nfe_xml::GtinXml],
    gtin_fallback: &str,
) -> Resultado<BuscaId> {
    let mut achados: Vec<String> = Vec::new();
    let mut stmt = conn.prepare_cached(crate::catalogo::codigos::SQL_PECAS_PELO_GTIN)?;
    for cod in codigos_da_busca(gtins, gtin_fallback) {
        let mut deste: Vec<String> = stmt.query_map([&cod], |l| l.get(0))?.collect::<Result<_, _>>()?;
        if let Some(v) = plano.gtins.get(&cod) {
            for p in v {
                if !deste.contains(p) {
                    deste.push(p.clone());
                }
            }
        }
        for pid in na_ordem_do_banco(deste) {
            if !achados.contains(&pid) {
                achados.push(pid);
            }
        }
    }
    Ok(crate::catalogo::codigos::por_quantidade(achados))
}

/// `peca_por_fornecedor_inequivoco` sobre o banco mais o plano.
fn plano_pelo_fornecedor(conn: &Connection, plano: &Plano, doc: &str, codigo: &str) -> Resultado<BuscaId> {
    let compacto = texto::compactar(codigo);
    if doc.is_empty() || compacto.is_empty() {
        return Ok(BuscaId::Nenhum);
    }
    let mut ids = ids_de(peca_por_fornecedor_inequivoco(conn, doc, codigo)?);
    if let Some(p) = plano.vinculos.get(&(doc.to_string(), compacto)) {
        if !ids.contains(p) {
            ids.push(p.clone());
        }
    }
    Ok(crate::catalogo::codigos::por_quantidade(na_ordem_do_banco(ids)))
}

fn plano_pessoa(conn: &Connection, plano: &Plano, doc: &str) -> Resultado<Option<String>> {
    if let Some(p) = plano.pessoas.get(doc) {
        return Ok(Some(p.clone()));
    }
    pessoa_existe(conn, doc)
}

fn plano_por_codigo(conn: &Connection, plano: &Plano, codigo: &str) -> Resultado<Option<String>> {
    Ok(peca_por_codigo(conn, codigo)?.or_else(|| plano.codigos.get(codigo).cloned()))
}

/// `peca_de_outra_descricao` sobre o banco mais o plano.
fn plano_outra_descricao(conn: &Connection, plano: &Plano, codigo: &str, it: &ItemXml) -> Resultado<Option<String>> {
    let achada: Option<(String, String)> = conn
        .query_row("SELECT id, nome FROM produto WHERE codigo = ?1", [codigo], |l| Ok((l.get(0)?, l.get(1)?)))
        .optional()?
        .or_else(|| {
            let id = plano.codigos.get(codigo)?;
            Some((id.clone(), plano.nome(id)?.to_string()))
        });
    let Some((id, nome)) = achada else { return Ok(None) };
    let (nome_c, desc_c) = (texto::compactar(&nome), descricao_chave(it));
    let cortado = nome.chars().count() >= 120 && desc_c.starts_with(&nome_c);
    Ok((!desc_c.is_empty() && nome_c != desc_c && !cortado).then_some(id))
}

/// `peca_repetida` sobre o banco mais o plano (as do banco vêm antes, como na
/// ordem da tabela).
fn plano_repetida(conn: &Connection, plano: &Plano, codigo: &str, it: &ItemXml) -> Resultado<Option<String>> {
    if let Some(id) = peca_repetida(conn, codigo, it)? {
        return Ok(Some(id));
    }
    let desc = descricao_chave(it);
    let mut do_plano: Vec<&String> = plano.codigos.get(codigo).into_iter().collect();
    do_plano.extend(plano.codigos_antigos.get(codigo).into_iter().flatten());
    let mut do_plano: Vec<String> = do_plano.into_iter().cloned().collect();
    do_plano = na_ordem_do_banco(do_plano);
    Ok(do_plano.into_iter().find(|id| plano.nome(id).is_some_and(|n| texto::compactar(n) == desc)))
}

fn plano_por_barras(conn: &Connection, plano: &Plano, gtin_: &str) -> Resultado<Option<String>> {
    if gtin_.is_empty() || !gtin::gtin_valido(gtin_) {
        return Ok(None);
    }
    match plano_pelos_gtins(conn, plano, &[], gtin_)? {
        BuscaId::Unico(id) => Ok(Some(id)),
        _ => Ok(None),
    }
}

/// A peça (que já existe) tem este código, no código de barras ou num GTIN?
fn peca_tem_codigo(conn: &Connection, pid: &str, codigo: &str) -> Resultado<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM produto WHERE id = ?1 AND codigo_barras = ?2 AND codigo_barras <> ''
             UNION ALL SELECT 1 FROM produto_gtin WHERE produto_id = ?1 AND gtin = ?2 LIMIT 1",
            params![pid, codigo],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false))
}

/// O que o `registrar_gtins_xml` vai gravar em `pid`, posto no plano (a mesma
/// regra: `codigos::codigo_do_xml`, e nada que já seja de outra peça).
/// Devolve os códigos que a peça ainda não tinha (A03b).
fn plano_registrar(
    conn: &Connection,
    plano: &mut Plano,
    pid: &str,
    gtins: &[produtos::GtinFicha],
) -> Resultado<Vec<String>> {
    let mut novos: Vec<String> = Vec::new();
    for g in gtins {
        let Some((cod, _, _)) = crate::catalogo::codigos::codigo_do_xml(g) else { continue };
        let de_outra_no_banco: bool = conn
            .query_row(crate::catalogo::codigos::SQL_GTIN_DE_OUTRA_PECA, params![pid, cod], |_| Ok(true))
            .optional()?
            .unwrap_or(false);
        let de_outra_no_plano = plano.gtins.get(&cod).is_some_and(|v| v.iter().any(|p| p != pid));
        if de_outra_no_banco || de_outra_no_plano {
            continue;
        }
        let ja_tinha = plano.gtins.get(&cod).is_some_and(|v| v.iter().any(|p| p == pid))
            || (!pid.starts_with(PLANEJADO) && peca_tem_codigo(conn, pid, &cod)?);
        if !ja_tinha && !novos.contains(&cod) {
            novos.push(cod.clone());
        }
        plano.dar_codigo(cod, pid);
    }
    Ok(novos)
}

/// A peça nova no plano, com os códigos que o `produtos::salvar` (o código de
/// barras e os GTINs da ficha, sem espaço e traço) e o `registrar_gtins_xml`
/// lhe dão.
fn plano_peca_nova(
    conn: &Connection,
    plano: &mut Plano,
    nome: &str,
    codigo_barras: &str,
    gtins: &[produtos::GtinFicha],
) -> Resultado<String> {
    let id = plano.nova_peca(nome);
    if !codigo_barras.is_empty() {
        plano.dar_codigo(codigo_barras.to_string(), &id);
    }
    for g in gtins {
        let c: String = g.gtin.chars().filter(|c| !matches!(c, ' ' | '-')).collect();
        if !c.is_empty() {
            plano.dar_codigo(c, &id);
        }
    }
    plano_registrar(conn, plano, &id, gtins)?;
    Ok(id)
}

/// O nome da peça nova, como a ficha do item o grava.
fn nome_da_peca(it: &ItemXml) -> String {
    let nome: String = texto::limpar(&it.descricao).chars().take(120).collect();
    if nome.is_empty() {
        "Peça sem nome".into()
    } else {
        nome
    }
}

/// O código de barras que a ficha da peça nova recebe (o GTIN do item, se for
/// válido e de nenhuma outra peça).
fn barras_da_peca_nova(conn: &Connection, plano: &Plano, it: &ItemXml) -> Resultado<String> {
    Ok(if !it.gtin.is_empty() && gtin::gtin_valido(&it.gtin) && plano_por_barras(conn, plano, &it.gtin)?.is_none() {
        it.gtin.clone()
    } else {
        String::new()
    })
}

/// Uma pessoa das notas: o documento e se a importação a cadastra.
struct DecisaoPessoa {
    doc: String,
    /// Id do plano quando a importação a cadastra.
    nova: Option<String>,
}

/// O que acontece com uma peça das notas da loja.
enum DecisaoLoja {
    /// O código é de outra peça e o GTIN acha uma: os GTINs vão para ela.
    OutraDescricaoPeloGtin { pid: String, gtins_novos: Vec<String> },
    /// O código é de outra peça e o GTIN não decide: pendência.
    OutraDescricaoPendencia { outra: String },
    /// A peça já existe (pelo código, pela repetida ou pelo GTIN).
    Existente { pid: Option<String>, gtins_novos: Vec<String> },
    /// Peça nova.
    Nova { id: String, codigo: String, barras: String, codigo_antigo: String },
}

/// O que acontece com uma peça das notas de fornecedor (etapa 4).
enum DecisaoForn {
    Pendencia {
        motivo: &'static str,
        candidatos: Vec<String>,
    },
    /// Sem "Peças do fornecedor", o código já está ligado à mesma peça que o
    /// GTIN acha: nada a decidir; as pendências do item fecham (P-1).
    JaLigadoFecha {
        pid: String,
    },
    /// Com "Peças do fornecedor", pelo GTIN: os GTINs vão para a peça; o código
    /// é ligado quando ainda não há vínculo.
    PeloGtin {
        pid: String,
        ligar: bool,
        gtins_novos: Vec<String>,
    },
    /// Pelo vínculo que já existe.
    PeloVinculo {
        pid: String,
        gtins: bool,
        gtins_novos: Vec<String>,
    },
    /// Código repetido: a peça de mesma descrição recebe os GTINs.
    MesmaDescricao {
        pid: String,
        gtins_novos: Vec<String>,
    },
    /// Peça nova (e o vínculo, se o código não se repete).
    Nova {
        id: String,
        barras: String,
        ligar: bool,
    },
    /// Nada é feito (a opção desmarcada e o fornecedor não cadastrado).
    Nada,
}

/// Uma decisão da etapa 4, com o que o item precisa para ser aplicada.
struct ItemForn<'a> {
    chave: &'a (String, String, String),
    item: &'a (ItemXml, String),
    fornecedor_id: Option<String>,
    forn_nome: String,
    chave_nfe: String,
    numero_nfe: String,
    repetido: bool,
    decisao: DecisaoForn,
}

/// Todas as decisões de uma prévia ou importação, na ordem em que a
/// importação as aplica.
struct Decisoes<'a> {
    opcoes: Opcoes,
    /// Documento → fornecedor (gravado ou do plano).
    ids_fornecedor: HashMap<String, String>,
    clientes: Vec<DecisaoPessoa>,
    fornecedores: Vec<DecisaoPessoa>,
    loja: Vec<(&'a String, &'a ItemXml, &'a String, DecisaoLoja)>,
    forn: Vec<ItemForn<'a>>,
}

/// Decide tudo, sem gravar. É a mesma função para a prévia e para a importação.
fn decidir<'a>(conn: &Connection, e: &Entrada, lido: &'a Lido) -> Resultado<Decisoes<'a>> {
    let mut plano = Plano::default();
    let mut d = Decisoes {
        opcoes: opcoes_de(e),
        ids_fornecedor: HashMap::new(),
        clientes: Vec::new(),
        fornecedores: Vec::new(),
        loja: Vec::new(),
        forn: Vec::new(),
    };

    // Etapa 1: clientes.
    let mut lista: Vec<_> = lido.clientes.keys().collect();
    lista.sort();
    for doc in lista {
        let nova =
            if e.clientes && plano_pessoa(conn, &plano, doc)?.is_none() { Some(plano.nova_pessoa(doc)) } else { None };
        d.clientes.push(DecisaoPessoa { doc: doc.clone(), nova });
    }

    // Etapa 2: fornecedores (cadastrados também com "Peças do fornecedor": FI-1).
    let mut ids_fornecedor: HashMap<String, String> = HashMap::new();
    let mut lista: Vec<_> = lido.fornecedores.keys().collect();
    lista.sort();
    for doc in lista {
        let nova = match plano_pessoa(conn, &plano, doc)? {
            Some(id) => {
                ids_fornecedor.insert(doc.clone(), id);
                None
            }
            None if e.fornecedores || e.pecas_fornecedor => {
                let id = plano.nova_pessoa(doc);
                ids_fornecedor.insert(doc.clone(), id.clone());
                Some(id)
            }
            None => None,
        };
        d.fornecedores.push(DecisaoPessoa { doc: doc.clone(), nova });
    }

    // Etapa 3: peças das notas da loja.
    if e.pecas_loja {
        let repetidos = lido.repetidos_loja();
        for ((codigo, _), (it, emissao)) in &lido.pecas_loja {
            let repetido = repetidos.contains(codigo);
            let gtins_ficha = gtins_para_ficha(&it.gtins, &it.gtin);
            // A04 (D-13): o código da loja só identifica a peça se a descrição
            // também bater; com outra descrição, vale o GTIN; sem GTIN que ache
            // a peça, o item vira pendência, sem gravar nada em peça nenhuma.
            if let Some(outra) = if repetido { None } else { plano_outra_descricao(conn, &plano, codigo, it)? } {
                let dec = match plano_pelos_gtins(conn, &plano, &it.gtins, &it.gtin)? {
                    BuscaId::Unico(pid) => {
                        let gtins_novos = plano_registrar(conn, &mut plano, &pid, &gtins_ficha)?;
                        DecisaoLoja::OutraDescricaoPeloGtin { pid, gtins_novos }
                    }
                    _ => DecisaoLoja::OutraDescricaoPendencia { outra },
                };
                d.loja.push((codigo, it, emissao, dec));
                continue;
            }
            let pelo_codigo = if repetido {
                plano_repetida(conn, &plano, codigo, it)?
            } else {
                plano_por_codigo(conn, &plano, codigo)?
            };
            let pelo_gtin = plano_pelos_gtins(conn, &plano, &it.gtins, &it.gtin)?;
            if pelo_codigo.is_some() || !matches!(pelo_gtin, BuscaId::Nenhum) {
                let pid = pelo_codigo.or(match pelo_gtin {
                    BuscaId::Unico(id) => Some(id),
                    _ => None,
                });
                let gtins_novos =
                    if let Some(p) = &pid { plano_registrar(conn, &mut plano, p, &gtins_ficha)? } else { Vec::new() };
                d.loja.push((codigo, it, emissao, DecisaoLoja::Existente { pid, gtins_novos }));
                continue;
            }
            // O código da loja só se usar letras, números e traço, sem repetir.
            // D-13: código que o sistema antigo deu a peças diferentes não fica
            // com nenhuma (o leitor daria a peça errada); vira código antigo.
            let cod = texto::limpar(codigo);
            let codigo_peca = if !repetido
                && !cod.is_empty()
                && cod.chars().count() <= 20
                && plano_por_codigo(conn, &plano, &cod)?.is_none()
            {
                cod.clone()
            } else {
                String::new()
            };
            let barras = barras_da_peca_nova(conn, &plano, it)?;
            let id = plano_peca_nova(conn, &mut plano, &nome_da_peca(it), &barras, &gtins_ficha)?;
            if !codigo_peca.is_empty() {
                plano.codigos.insert(codigo_peca.clone(), id.clone());
            }
            let codigo_antigo = if repetido && !cod.is_empty() { cod.clone() } else { String::new() };
            if !codigo_antigo.is_empty() {
                plano.codigos_antigos.entry(codigo_antigo.clone()).or_default().push(id.clone());
            }
            d.loja.push((codigo, it, emissao, DecisaoLoja::Nova { id, codigo: codigo_peca, barras, codigo_antigo }));
        }
    }

    // Etapa 4: peças das notas de fornecedor.
    let repetidos_f = lido.repetidos_fornecedor();
    let gtins_repetidos_forn = gtins_em_mais_de_uma_descricao(lido);
    let por_descricao = PorDescricao::default();
    for (chave, item) in &lido.pecas_fornecedor {
        let (forn, codigo, desc_chave) = chave;
        let (it, _) = item;
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
            plano_pelos_gtins(conn, &plano, &it.gtins, &it.gtin)?
        };
        let forn_busca = if repetido { BuscaId::Nenhum } else { plano_pelo_fornecedor(conn, &plano, forn, codigo)? };
        let pendencia = |motivo: &'static str, candidatos: Vec<String>| DecisaoForn::Pendencia { motivo, candidatos };
        let decisao = 'decisao: {
            // 1. Ambiguidade de GTIN ou do código do fornecedor, ou conflito
            //    entre os dois: não associar sozinho; pendência.
            if let BuscaId::Ambiguo(cands) = &gtin_busca {
                break 'decisao pendencia("gtin_ambiguo", cands.clone());
            }
            if let BuscaId::Ambiguo(cands) = &forn_busca {
                break 'decisao pendencia("fornecedor_ambiguo", cands.clone());
            }
            if let (BuscaId::Unico(pid_g), BuscaId::Unico(pid_f)) = (&gtin_busca, &forn_busca) {
                if pid_g != pid_f {
                    break 'decisao pendencia("conflito_gtin_fornecedor", vec![pid_g.clone(), pid_f.clone()]);
                }
            }
            // 2. GTIN exato e inequívoco tem prioridade.
            if let BuscaId::Unico(pid) = &gtin_busca {
                // A03a (DP-11): sem "Peças do fornecedor", nada é gravado na
                // peça que já existe; o item fica como pendência, com a peça
                // como candidata, se o fornecedor existe ou está marcado. P-1:
                // o código já ligado à mesma peça não tem nada a decidir.
                if !e.pecas_fornecedor {
                    if matches!(&forn_busca, BuscaId::Unico(pf) if pf == pid) {
                        break 'decisao DecisaoForn::JaLigadoFecha { pid: pid.clone() };
                    }
                    if fornecedor_id.is_some() || e.fornecedores {
                        break 'decisao pendencia("peca_pelo_gtin", vec![pid.clone()]);
                    }
                    break 'decisao DecisaoForn::Nada;
                }
                let gtins_novos = plano_registrar(conn, &mut plano, pid, &gtins_ficha)?;
                let ligar = !repetido && fornecedor_id.is_some() && matches!(forn_busca, BuscaId::Nenhum);
                if ligar {
                    plano.vinculos.insert((forn.clone(), texto::compactar(codigo)), pid.clone());
                }
                break 'decisao DecisaoForn::PeloGtin { pid: pid.clone(), ligar, gtins_novos };
            }
            // 3. Vínculo que já existe pelo (documento do fornecedor, código).
            if let BuscaId::Unico(pid) = &forn_busca {
                // A03a (DP-11): os GTINs da nota só vão para a peça com "Peças do fornecedor".
                let gtins_novos =
                    if e.pecas_fornecedor { plano_registrar(conn, &mut plano, pid, &gtins_ficha)? } else { Vec::new() };
                break 'decisao DecisaoForn::PeloVinculo { pid: pid.clone(), gtins: e.pecas_fornecedor, gtins_novos };
            }
            // 4. Nada acha. A01: um código só de símbolos ("---") não identifica a
            //    peça nem pode ser ligado; vira pendência própria.
            let so_simbolos = texto::compactar(codigo).is_empty();
            if e.pecas_fornecedor && !so_simbolos {
                if fornecedor_id.is_none() {
                    break 'decisao DecisaoForn::Nada;
                }
                if repetido {
                    if let Some(pid) = por_descricao.achar(conn, &plano, it)? {
                        let gtins_novos = plano_registrar(conn, &mut plano, &pid, &gtins_ficha)?;
                        break 'decisao DecisaoForn::MesmaDescricao { pid, gtins_novos };
                    }
                }
                let barras = barras_da_peca_nova(conn, &plano, it)?;
                let nome = nome_da_peca(it);
                let id = plano_peca_nova(conn, &mut plano, &nome, &barras, &gtins_ficha)?;
                por_descricao.nova(&id, &nome);
                if !repetido {
                    plano.vinculos.insert((forn.clone(), texto::compactar(codigo)), id.clone());
                }
                break 'decisao DecisaoForn::Nova { id, barras, ligar: !repetido };
            }
            if fornecedor_id.is_some() || e.fornecedores {
                break 'decisao pendencia(
                    if so_simbolos {
                        "codigo_so_de_simbolos"
                    } else if repetido {
                        "codigo_repetido"
                    } else {
                        "sem_vinculo"
                    },
                    Vec::new(),
                );
            }
            DecisaoForn::Nada
        };
        d.forn.push(ItemForn { chave, item, fornecedor_id, forn_nome, chave_nfe, numero_nfe, repetido, decisao });
    }
    d.ids_fornecedor = ids_fornecedor;
    Ok(d)
}

// ---------------------------------------------------------------------------
// Lote 2, itens 03b e 04 (O-3, A03b, P-4, P-5): o que a importação vai fazer,
// contado das MESMAS decisões. A prévia mostra este resumo; a importação
// devolve o dela, calculado do mesmo jeito; a trava "prévia = resultado" e o
// carimbo garantem que é o mesmo.
// ---------------------------------------------------------------------------

/// As opções que valeram numa prévia.
#[derive(Debug, Default, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Opcoes {
    pub clientes: bool,
    pub fornecedores: bool,
    pub pecas_loja: bool,
    pub pecas_fornecedor: bool,
}

/// Uma categoria do que a importação faz (código estável, texto da tela,
/// quantidade e até 6 exemplos). A especificação está em
/// `preparacao/ESPEC_O3_CLASSIFICACAO_PREVIA.md`.
#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Categoria {
    pub codigo: String,
    pub texto: String,
    pub quantidade: i64,
    pub exemplos: Vec<String>,
}

/// Quantos itens vão para pendência por um motivo, e quantos deles só
/// atualizam a pendência que já estava aberta.
#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MotivoPendencia {
    pub motivo: String,
    pub texto: String,
    pub quantidade: i64,
    pub ja_abertas: i64,
}

/// O resumo das decisões: o mesmo na prévia e no resultado da importação.
#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Resumo {
    pub categorias: Vec<Categoria>,
    pub vao_para_pendencias: i64,
    pub pendencias_por_motivo: Vec<MotivoPendencia>,
    /// Peças que JÁ existiam e recebem GTIN novo ou o código do fornecedor (A03b).
    pub pecas_que_mudam: i64,
    pub exemplos_pecas_que_mudam: Vec<String>,
    /// Peças desativadas que a importação acha (pelo GTIN ou pelo vínculo).
    /// Só informação: a regra para elas é a decisão N06, ainda aberta.
    pub pecas_desativadas: i64,
    pub exemplos_pecas_desativadas: Vec<String>,
}

/// O texto de cada categoria, na ordem em que a tela mostra.
const CATEGORIAS: &[(&str, &str)] = &[
    ("cliente.novo", "Clientes novos"),
    ("cliente.ja", "Clientes já cadastrados"),
    ("cliente.nao_cadastrado", "Clientes que não entram (opção desmarcada)"),
    ("fornecedor.novo", "Fornecedores novos"),
    ("fornecedor.novo_implicito", "Fornecedores novos que entram porque \"Peças do fornecedor\" está marcada"),
    ("fornecedor.ja", "Fornecedores já cadastrados"),
    ("fornecedor.nao_cadastrado", "Fornecedores que não entram (opções desmarcadas)"),
    ("loja.nova", "Peças novas (notas da loja)"),
    ("loja.ja", "Peças da loja já cadastradas (nada muda)"),
    ("loja.ja_gtin_novo", "Peças da loja já cadastradas que ganham GTIN"),
    ("loja.pendencia", "Peças da loja com código de outra peça (vão para pendências)"),
    ("forn.nova", "Peças novas (notas de fornecedor), com o código do fornecedor ligado"),
    ("forn.nova_codigo_repetido", "Peças novas de código repetido (o código não fica ligado)"),
    ("forn.mesma_descricao", "Código repetido: entram na peça de mesma descrição"),
    ("forn.ja_ligado", "Já ligadas ao código do fornecedor (nada muda)"),
    ("forn.ja_pelo_vinculo", "Já ligadas ao código do fornecedor"),
    ("forn.ja_pelo_vinculo_gtin_novo", "Já ligadas ao código do fornecedor, ganham GTIN"),
    ("forn.ja_pelo_gtin", "Achadas pelo GTIN"),
    ("forn.ja_pelo_gtin_liga", "Achadas pelo GTIN: o código do fornecedor será ligado"),
    ("forn.pendencia", "Peças de fornecedor que vão para pendências"),
    ("forn.ignorada", "Peças de fornecedor que não entram (opções desmarcadas)"),
];

/// Uma pendência aberta para o mesmo item (fornecedor, código e descrição)?
fn pendencia_aberta(conn: &Connection, doc: &str, codigo: &str, descricao: &str) -> Resultado<bool> {
    let desc = texto::compactar(descricao);
    let mut c = conn.prepare_cached(
        "SELECT descricao FROM xml_pendencia_identificacao
          WHERE situacao = 'pendente' AND fornecedor_documento = ?1 AND codigo_compacto = ?2",
    )?;
    let abertas: Vec<String> = c
        .query_map(params![documento::limpar(doc), texto::compactar(codigo)], |l| l.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(abertas.iter().any(|d| texto::compactar(d) == desc))
}

fn nome_e_ativo(conn: &Connection, pid: &str) -> Resultado<Option<(String, bool)>> {
    Ok(conn
        .query_row("SELECT nome, ativo FROM produto WHERE id = ?1", [pid], |l| {
            Ok((l.get::<_, String>(0)?, l.get::<_, i64>(1)? != 0))
        })
        .optional()?)
}

/// O resumo das decisões (a prévia e a importação chamam a mesma função).
fn resumir(conn: &Connection, lido: &Lido, d: &Decisoes) -> Resultado<Resumo> {
    let mut cat: BTreeMap<&'static str, (i64, Vec<String>)> = BTreeMap::new();
    let mut conta = |c: &'static str, exemplo: String| {
        let e = cat.entry(c).or_default();
        e.0 += 1;
        if e.1.len() < 6 && !exemplo.is_empty() && !e.1.contains(&exemplo) {
            e.1.push(exemplo);
        }
    };
    let nome_pessoa = |e: &Emitente| if e.fantasia.is_empty() { e.nome.clone() } else { e.fantasia.clone() };
    for p in &d.clientes {
        let (em, _) = &lido.clientes[&p.doc];
        if p.nova.is_some() {
            conta("cliente.novo", nome_pessoa(em));
        } else if pessoa_existe(conn, &p.doc)?.is_some() {
            conta("cliente.ja", String::new());
        } else {
            conta("cliente.nao_cadastrado", nome_pessoa(em));
        }
    }
    for p in &d.fornecedores {
        let (em, _) = &lido.fornecedores[&p.doc];
        match &p.nova {
            Some(_) if d.opcoes.fornecedores => conta("fornecedor.novo", nome_pessoa(em)),
            Some(_) => conta("fornecedor.novo_implicito", nome_pessoa(em)),
            None if d.ids_fornecedor.contains_key(&p.doc) => conta("fornecedor.ja", String::new()),
            None => conta("fornecedor.nao_cadastrado", nome_pessoa(em)),
        }
    }
    let mut que_mudam: Vec<String> = Vec::new();
    let mut desativadas: Vec<String> = Vec::new();
    let muda = |pid: &str, que_mudam: &mut Vec<String>| {
        if !pid.starts_with(PLANEJADO) && !que_mudam.iter().any(|p| p == pid) {
            que_mudam.push(pid.to_string());
        }
    };
    let achada = |pid: &str, desativadas: &mut Vec<String>| -> Resultado<()> {
        if !pid.starts_with(PLANEJADO) && !desativadas.iter().any(|p| p == pid) {
            if let Some((_, false)) = nome_e_ativo(conn, pid)? {
                desativadas.push(pid.to_string());
            }
        }
        Ok(())
    };
    let mut motivos: BTreeMap<&'static str, (i64, i64)> = BTreeMap::new();
    for (codigo, it, _, dec) in &d.loja {
        let desc = texto::limpar(&it.descricao);
        match dec {
            DecisaoLoja::Nova { .. } => conta("loja.nova", desc),
            DecisaoLoja::OutraDescricaoPendencia { .. } => {
                conta("loja.pendencia", desc);
                let m = motivos.entry("codigo_de_outra_peca").or_default();
                m.0 += 1;
                if pendencia_aberta(conn, "", codigo, &it.descricao)? {
                    m.1 += 1;
                }
            }
            DecisaoLoja::OutraDescricaoPeloGtin { pid, gtins_novos }
            | DecisaoLoja::Existente { pid: Some(pid), gtins_novos } => {
                if gtins_novos.is_empty() {
                    conta("loja.ja", String::new());
                } else {
                    conta("loja.ja_gtin_novo", desc);
                    muda(pid, &mut que_mudam);
                }
            }
            DecisaoLoja::Existente { pid: None, .. } => conta("loja.ja", String::new()),
        }
    }
    for i in &d.forn {
        let (it, _) = i.item;
        let desc = texto::limpar(&it.descricao);
        match &i.decisao {
            DecisaoForn::Pendencia { motivo, .. } => {
                conta("forn.pendencia", desc);
                let m = motivos.entry(motivo).or_default();
                m.0 += 1;
                if pendencia_aberta(conn, &i.chave.0, &i.chave.1, &it.descricao)? {
                    m.1 += 1;
                }
            }
            DecisaoForn::JaLigadoFecha { pid } => {
                conta("forn.ja_ligado", String::new());
                achada(pid, &mut desativadas)?;
            }
            DecisaoForn::PeloGtin { pid, ligar, gtins_novos } => {
                conta(if *ligar { "forn.ja_pelo_gtin_liga" } else { "forn.ja_pelo_gtin" }, desc);
                if *ligar || !gtins_novos.is_empty() {
                    muda(pid, &mut que_mudam);
                }
                achada(pid, &mut desativadas)?;
            }
            DecisaoForn::PeloVinculo { pid, gtins_novos, .. } => {
                if gtins_novos.is_empty() {
                    conta("forn.ja_pelo_vinculo", String::new());
                } else {
                    conta("forn.ja_pelo_vinculo_gtin_novo", desc);
                    muda(pid, &mut que_mudam);
                }
                achada(pid, &mut desativadas)?;
            }
            DecisaoForn::MesmaDescricao { pid, gtins_novos } => {
                conta("forn.mesma_descricao", desc);
                if !gtins_novos.is_empty() {
                    muda(pid, &mut que_mudam);
                }
            }
            DecisaoForn::Nova { ligar, .. } => {
                conta(if *ligar { "forn.nova" } else { "forn.nova_codigo_repetido" }, desc)
            }
            DecisaoForn::Nada => conta("forn.ignorada", desc),
        }
    }
    let nomes = |ids: &[String]| -> Resultado<Vec<String>> {
        let mut v = Vec::new();
        for p in ids.iter().take(6) {
            if let Some((n, _)) = nome_e_ativo(conn, p)? {
                v.push(n);
            }
        }
        Ok(v)
    };
    let categorias = CATEGORIAS
        .iter()
        .filter_map(|(c, t)| {
            cat.get(c).map(|(n, ex)| Categoria {
                codigo: c.to_string(),
                texto: t.to_string(),
                quantidade: *n,
                exemplos: ex.clone(),
            })
        })
        .collect();
    let pendencias_por_motivo: Vec<MotivoPendencia> = motivos
        .iter()
        .map(|(m, (n, abertas))| MotivoPendencia {
            motivo: m.to_string(),
            texto: motivo_legivel(m).to_string(),
            quantidade: *n,
            ja_abertas: *abertas,
        })
        .collect();
    Ok(Resumo {
        categorias,
        vao_para_pendencias: pendencias_por_motivo.iter().map(|m| m.quantidade).sum(),
        pendencias_por_motivo,
        pecas_que_mudam: que_mudam.len() as i64,
        exemplos_pecas_que_mudam: nomes(&que_mudam)?,
        pecas_desativadas: desativadas.len() as i64,
        exemplos_pecas_desativadas: nomes(&desativadas)?,
    })
}

/// P-4: o carimbo de uma prévia. Muda se mudam as opções, o conteúdo dos
/// arquivos lidos ou os cadastros (a fila de alterações da troca anda a cada
/// gravação de peça, pessoa, GTIN ou vínculo).
fn carimbo(conn: &Connection, e: &Opcoes, lido: &Lido) -> Resultado<String> {
    use sha2::Digest;
    let seq: i64 = conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM alteracao", [], |l| l.get(0))?;
    let mut h = sha2::Sha256::new();
    h.update([e.clientes as u8, e.fornecedores as u8, e.pecas_loja as u8, e.pecas_fornecedor as u8]);
    h.update(lido.assinatura);
    h.update(seq.to_le_bytes());
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// O texto do carimbo vencido (a tela mostra e refaz a prévia).
const CARIMBO_VENCIDO: &str =
    "Os arquivos ou os cadastros mudaram desde a prévia. Nada foi gravado: confira a prévia de novo e cadastre.";

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
        // P-5: sem nenhuma opção no pedido, a prévia supõe todas marcadas.
        let com_opcoes =
            ["clientes", "fornecedores", "pecasLoja", "pecasFornecedor"].iter().any(|k| dados.get(k).is_some());
        let d: importar::Entrada = ler(dados)?;
        if com_opcoes {
            Ok(json!(importar::previa_com(p.conn, &d)?))
        } else {
            Ok(json!(importar::previa(p.conn, &d.arquivos)?))
        }
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

    /// Fornecedor implícito (achado da preparação de 12/11): as 16 combinações
    /// das opções, num lote com um cliente novo, uma peça nova da loja e um
    /// fornecedor novo com uma peça nova. É o RETRATO DE HOJE: "Peças do
    /// fornecedor" cadastra o fornecedor mesmo com "Fornecedores" desmarcada
    /// (`importar_fornecedores`). A regra é do proprietário (decisão FI); quando
    /// ela vier, muda só a linha do fornecedor aqui, no mesmo commit da mudança.
    #[test]
    fn fi_as_16_combinacoes_das_opcoes_retrato_de_hoje() {
        const FORN: &str = "11222333000181";
        const CLIENTE: &str = "52998224725";
        let mut erros = Vec::new();
        for n in 0..16u8 {
            let (cl, fo, lj, pf) = (n & 8 != 0, n & 4 != 0, n & 2 != 0, n & 1 != 0);
            let b = Banco::em_memoria().unwrap();
            let (c, d) = (&b.conn, b.dispositivo.as_str());
            let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
            crate::faixas::comecar_loja(c).unwrap();
            config::gravar(c, "loja_cnpj", "11444777000161").unwrap();
            let mut lido = Lido::default();
            let cliente = Emitente { documento: CLIENTE.into(), nome: "CLIENTE DE TESTE".into(), ..Default::default() };
            lido.clientes.insert(CLIENTE.into(), (cliente, "2026-11-01".into()));
            let forn = Emitente { documento: FORN.into(), nome: "FORNECEDOR DE TESTE".into(), ..Default::default() };
            lido.fornecedores.insert(FORN.into(), (forn, "2026-11-01".into()));
            let item = |codigo: &str, descricao: &str| ItemXml {
                codigo: codigo.into(),
                descricao: descricao.into(),
                quantidade: 1000,
                valor_produtos: 5000,
                ..Default::default()
            };
            let il = item("L100", "PECA DA LOJA");
            lido.pecas_loja.insert(("L100".into(), descricao_chave(&il)), (il, "2026-11-01".into()));
            let ifo = item("F-200", "PECA DO FORNECEDOR");
            lido.pecas_fornecedor
                .insert((FORN.into(), "F-200".into(), descricao_chave(&ifo)), (ifo, "2026-11-01".into()));
            let e =
                Entrada { clientes: cl, fornecedores: fo, pecas_loja: lj, pecas_fornecedor: pf, ..Default::default() };
            gravar(c, d, &u, &e, &lido).unwrap();
            let conta = |sql: &str| -> i64 { c.query_row(sql, [], |l| l.get(0)).unwrap() };
            let obtido = (
                conta("SELECT COUNT(*) FROM pessoa WHERE cliente = 1"),
                conta("SELECT COUNT(*) FROM pessoa WHERE fornecedor = 1"),
                conta("SELECT COUNT(*) FROM produto"),
                conta("SELECT COUNT(*) FROM produto_fornecedor"),
                conta("SELECT COUNT(*) FROM xml_pendencia_identificacao WHERE situacao = 'pendente' AND motivo = 'sem_vinculo'"),
            );
            let esperado = (
                cl as i64,
                (fo || pf) as i64, // ← o implícito: "Peças do fornecedor" cadastra o fornecedor
                lj as i64 + pf as i64,
                pf as i64,
                (fo && !pf) as i64,
            );
            if obtido != esperado {
                erros.push(format!(
                    "opções c{} f{} l{} p{}: esperado {esperado:?}, obtido {obtido:?}",
                    cl as u8, fo as u8, lj as u8, pf as u8
                ));
            }
        }
        assert!(
            erros.is_empty(),
            "(clientes, fornecedores, peças, vínculos, pendências sem vínculo)\n{}",
            erros.join("\n")
        );
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
            ..Default::default()
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
            ..Default::default()
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
                ..Default::default()
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
                ..Default::default()
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
