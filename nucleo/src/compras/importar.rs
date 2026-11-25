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
    /// T108 (texto R do proprietário, 21/11/2026): as notas de compra também vão para Compras, em rascunho.
    pub compras: bool,
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
    /// M1 da revisão do R17: quantos já cadastrados passam a ser cliente (FI-3) ou fornecedor (FI-2) com a opção
    /// marcada, contados com todas as opções ligadas (como os novos): a caixa de marcar não some ao desmarcar.
    pub clientes_viram: i64,
    pub fornecedores_viram: i64,
    pub pecas_loja_novas: i64,
    pub pecas_fornecedor_novas: i64,
    pub pecas_ja: i64,
    pub exemplos_clientes: Vec<String>,
    pub exemplos_fornecedores: Vec<String>,
    pub exemplos_pecas: Vec<String>,
    /// D-13: códigos que aparecem em peças com descrições diferentes (cada
    /// descrição vira uma peça; o código não é dado a nenhuma delas).
    pub codigos_repetidos: i64,
    /// T108: as notas de compra que vão para Compras (em rascunho) e as que já estavam lá.
    pub compras_novas: i64,
    pub compras_ja: i64,
    /// A8 da revisão: as notas de fornecedor que não servem para a entrada de compra (só os cadastros).
    pub compras_nao_serve: i64,
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
    /// T108: as notas de compra que foram para Compras, em rascunho. Fora da resposta quando 0 (a resposta de quem
    /// não pede as compras fica como era: a trava T7).
    #[serde(skip_serializing_if = "nenhuma")]
    pub compras: i64,
    /// O resumo das decisões que foram aplicadas: o mesmo da prévia. Fica de
    /// fora da resposta (a trava T7 congela o resultado da importação); serve
    /// à trava "prévia = resultado".
    #[serde(skip)]
    pub resumo: Resumo,
}

fn nenhuma(n: &i64) -> bool {
    *n == 0
}

/// O que foi lido das notas, já sem repetição.
#[derive(Default)]
struct Lido {
    arquivos: i64,
    ignorados: i64,
    /// Quantos dos ignorados têm o motivo nos avisos (T086).
    ignorados_listados: i64,
    /// Quantos dos ignorados eram eventos ou CT-e (não são notas; T101).
    eventos_e_cte: i64,
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
    /// T108: as notas de compra que servem para Compras e ainda não estão lá: (arquivo, XML, nota verificada para a
    /// compra), na ordem em que foram lidas.
    compras: Vec<(String, String, nfe_xml::NotaVerificada)>,
    /// T108: as notas de compra que já estavam em Compras (não entram de novo).
    compras_ja: i64,
    /// A8 da revisão da T108: as notas de fornecedor que não servem para a entrada de compra (só os cadastros).
    compras_nao_serve: i64,
}

/// T108 (texto R do proprietário, 21/11/2026): o que a nota de fornecedor é para Compras.
pub enum ParaCompras {
    /// Serve para a entrada de compra e ainda não está em Compras.
    Nova(Box<nfe_xml::NotaVerificada>),
    /// Já está em Compras (em rascunho ou lançada).
    Ja,
    /// Não serve para a entrada (sem a autorização da SEFAZ, para outro CNPJ, devolução, NFC-e...): só os cadastros.
    NaoServe,
}

/// T108: as regras de Compras que a importação de cadastros usa para levar as notas de compra a Compras. Quem chama
/// é o módulo de Compras (ele conhece as regras dele; este módulo não depende dele): `classificar` diz o que a nota
/// é para Compras (sem gravar) e `criar` cria o rascunho da compra. Sem o gancho, nada vai para Compras.
pub struct GanchoCompras<'a> {
    pub classificar: &'a dyn Fn(&Connection, &str, &NotaXml) -> Resultado<ParaCompras>,
    pub criar: &'a dyn Fn(&Connection, &str, &nfe_xml::NotaVerificada) -> Resultado<()>,
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

/// Chama `cada` com (nome do arquivo, conteúdo, erro de leitura) de todos os XMLs achados. O erro de leitura (não
/// vazio) é o motivo de quem leu os arquivos: o pacote estragado, grande demais, o lote cortado (T080).
fn cada_xml(entradas: &[String], mut cada: impl FnMut(&str, &str, &str)) -> Resultado<i64> {
    let lista = arquivos::xmls(entradas)?;
    let lidos = lista.len() as i64;
    for x in lista {
        cada(&x.nome, &x.texto, &x.erro);
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

/// Junta a nota ao que foi lido (pessoas e peças, a mais nova de cada). `false` se a mesma nota já veio em outro
/// arquivo (nada muda).
fn acumular(lido: &mut Lido, chaves: &mut HashSet<String>, loja: &str, nota: &NotaXml) -> bool {
    if !nota.chave.is_empty() && !chaves.insert(nota.chave.clone()) {
        return false;
    }
    let emitente = documento::limpar(&nota.emitente.documento);
    let destinatario = documento::limpar(&nota.destinatario);
    if emitente == loja {
        lido.notas_loja += 1;
        // Devolução para fornecedor: o destinatário não é cliente.
        if nota.finalidade == "4" {
            return true;
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
            return true;
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
    true
}

fn ler_tudo(conn: &Connection, arquivos: &[String], compras: Option<&GanchoCompras>) -> Resultado<Lido> {
    if arquivos.is_empty() {
        return Err(Erro::validacao("Escolha os arquivos XML, um .zip, um .rar ou uma pasta."));
    }
    let loja = cnpj_loja(conn)?;
    let mut lido = Lido::default();
    let mut chaves: HashSet<String> = HashSet::new();
    let mut falha: Option<Erro> = None;
    // N3: as chaves cuja 1ª cópia não serviu para Compras (a outra cópia ainda pode servir).
    let mut nao_serviram: HashSet<String> = HashSet::new();
    let mut assinatura = {
        use sha2::Digest;
        sha2::Sha256::new()
    };
    let total = cada_xml(arquivos, |nome, xml, erro_de_leitura| {
        {
            use sha2::Digest;
            assinatura.update(nome.as_bytes());
            assinatura.update([0u8]);
            assinatura.update(xml.as_bytes());
            assinatura.update([0u8]);
        }
        // T080 (OPUS-C-2): o arquivo que nem foi lido aparece com o motivo verdadeiro (antes, "não é um XML
        // válido"). O aviso de lote cortado (só parte dos arquivos foi olhada) vem sempre, e primeiro.
        if !erro_de_leitura.is_empty() {
            let aviso = format!("{nome}: {erro_de_leitura}");
            if erro_de_leitura.contains("arquivos demais")
                || erro_de_leitura.contains("escolha menos arquivos")
                || erro_de_leitura.contains("não foram olhados")
            {
                lido.avisos.insert(0, aviso);
            } else {
                lido.ignorados += 1;
                if lido.avisos.len() < 5 {
                    lido.avisos.push(aviso);
                    lido.ignorados_listados += 1;
                }
            }
            return;
        }
        // PEND-09 (D2, D8): só cadastra a partir de nota com a assinatura do
        // emitente verificada; a autorização da SEFAZ não é exigida aqui
        // (cadastro não mexe em estoque, custo nem financeiro).
        let nota: NotaXml = match nfe_xml::ler_cadastro_verificada(xml, crate::nfe_assinatura::Ancoras::padrao()) {
            Ok(n) => n.nota,
            Err(e) => {
                lido.ignorados += 1;
                if e.contains("evento") || e.contains("CT-e") {
                    lido.eventos_e_cte += 1;
                } else if lido.avisos.len() < 5 {
                    lido.avisos.push(format!("{nome}: {e}"));
                    lido.ignorados_listados += 1;
                }
                return;
            }
        };
        let primeira_copia = acumular(&mut lido, &mut chaves, &loja, &nota);
        // N3 da 2ª conferência da T108: a mesma chave em dois arquivos conta uma vez para os cadastros; para Compras
        // vale a melhor cópia. Se a 1ª não serviu (por exemplo, a NF-e sem o protocolo de autorização), a outra
        // (a procNFe, com ele) é classificada.
        let outra_chance = !primeira_copia && nao_serviram.contains(&nota.chave);
        if !primeira_copia && !outra_chance {
            return; // a mesma nota em dois arquivos
        }
        // T108 (texto R): a nota de compra também vai para Compras, em rascunho, se servir para a entrada (as regras
        // da importação de Compras) e ainda não estiver lá.
        let gancho = compras.filter(|_| falha.is_none() && documento::limpar(&nota.emitente.documento) != loja);
        if let Some(g) = gancho {
            let resultado = (g.classificar)(conn, xml, &nota);
            if outra_chance && matches!(resultado, Ok(ParaCompras::Nova(_) | ParaCompras::Ja)) {
                nao_serviram.remove(&nota.chave);
                lido.compras_nao_serve -= 1;
            }
            match resultado {
                Ok(ParaCompras::Nova(v)) => lido.compras.push((nome.to_string(), xml.to_string(), *v)),
                Ok(ParaCompras::Ja) => lido.compras_ja += 1,
                Ok(ParaCompras::NaoServe) if !outra_chance => {
                    lido.compras_nao_serve += 1;
                    if !nota.chave.is_empty() {
                        nao_serviram.insert(nota.chave.clone());
                    }
                }
                Ok(ParaCompras::NaoServe) => {}
                Err(e) => falha = Some(e),
            }
        }
    })?;
    if let Some(e) = falha {
        return Err(e);
    }
    lido.arquivos = total;
    lido.assinatura = {
        use sha2::Digest;
        assinatura.finalize().into()
    };
    // T086 (AUD-X-2): os motivos vão só dos 5 primeiros; uma linha diz quantos mais ficaram sem motivo listado.
    // T101 (N5 da 3ª conferência): os eventos e CT-e, com o motivo, à parte; o "sem o motivo" é só o resto.
    let sem_motivo = lido.ignorados - lido.ignorados_listados - lido.eventos_e_cte;
    if sem_motivo > 0 {
        lido.avisos.push(format!(
            "E mais {sem_motivo} {} sem o motivo listado aqui.",
            if sem_motivo == 1 { "arquivo ignorado" } else { "arquivos ignorados" }
        ));
    }
    if lido.eventos_e_cte > 0 {
        // T104 (N11 da 4ª conferência): o resumo de NF-e (resNFe) também cai aqui.
        lido.avisos.push(if lido.eventos_e_cte == 1 {
            "1 arquivo era um evento, um CT-e ou um resumo de NF-e (não é a nota completa) e ficou de fora; de um \
             resumo, baixe o XML completo da NF-e."
                .to_string()
        } else {
            format!(
                "{} arquivos eram eventos, CT-e ou resumos de NF-e (não são a nota completa) e ficaram de fora; dos \
                 resumos, baixe o XML completo da NF-e.",
                lido.eventos_e_cte
            )
        });
    }
    let avisos = lido.avisos_repetidos();
    let repetidos = avisos.len();
    for a in avisos.into_iter().take(5) {
        lido.avisos.push(a);
    }
    if repetidos > 5 {
        lido.avisos.push(format!(
            "E mais {} {} em peças diferentes, como os de cima.",
            repetidos - 5,
            if repetidos - 5 == 1 { "código repetido" } else { "códigos repetidos" }
        ));
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
        compras: e.compras,
    }
}

impl Entrada {
    /// Todas as opções marcadas, sem arquivos (P-5: a prévia sem opções supõe todas).
    pub fn todas() -> Entrada {
        todas_as_opcoes(&[])
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
        compras: true,
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
    previa_com_compras(conn, e, None)
}

/// T108: a prévia com as notas de compra que vão para Compras (o gancho é do módulo de Compras).
pub fn previa_com_compras(conn: &Connection, e: &Entrada, compras: Option<&GanchoCompras>) -> Resultado<Previa> {
    let lido = ler_tudo(conn, &e.arquivos, compras)?;
    previa_do_lido(conn, e, &lido)
}

/// A prévia sobre o que já foi lido (os testes montam o lote sem nota assinada).
fn previa_do_lido(conn: &Connection, e: &Entrada, lido: &Lido) -> Resultado<Previa> {
    let todas = todas_as_opcoes(&e.arquivos);
    let d_todas = decidir(conn, &todas, lido)?;
    let opcoes = opcoes_de(e);
    let resumo = if opcoes == opcoes_de(&todas) {
        resumir(conn, lido, &d_todas)?
    } else {
        resumir(conn, lido, &decidir(conn, e, lido)?)?
    };
    let mut p = Previa {
        arquivos: lido.arquivos,
        notas: lido.notas_loja + lido.notas_fornecedor,
        notas_loja: lido.notas_loja,
        notas_fornecedor: lido.notas_fornecedor,
        ignorados: lido.ignorados,
        avisos: lido.avisos.clone(),
        opcoes,
        carimbo: carimbo(conn, &opcoes, lido)?,
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
    p.clientes_viram = d_todas.clientes.iter().filter(|x| x.vira_cliente).count() as i64;
    p.fornecedores_viram = d_todas.fornecedores.iter().filter(|x| x.vira_fornecedor).count() as i64;
    p.codigos_repetidos = (lido.repetidos_loja().len() + lido.repetidos_fornecedor().len()) as i64;
    p.compras_novas = lido.compras.len() as i64;
    p.compras_ja = lido.compras_ja;
    p.compras_nao_serve = lido.compras_nao_serve;
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
                    "gtin_ambiguo"
                        | "fornecedor_ambiguo"
                        | "conflito_gtin_fornecedor"
                        | "peca_pelo_gtin"
                        | "peca_desativada"
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
    importar_com_compras(conn, dispositivo, usuario_id, e, None)
}

/// T108: a importação que também leva as notas de compra a Compras, em rascunho (o gancho é do módulo de Compras).
pub fn importar_com_compras(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    e: &Entrada,
    compras: Option<&GanchoCompras>,
) -> Resultado<Importado> {
    let lido = ler_tudo(conn, &e.arquivos, compras)?;
    gravar_com(conn, dispositivo, usuario_id, e, &lido, compras)
}

/// Decide tudo (a mesma função da prévia) e aplica, na ordem das etapas.
/// Separada da leitura para os testes montarem casos sem nota assinada.
#[cfg(test)]
fn gravar(conn: &Connection, dispositivo: &str, usuario_id: &str, e: &Entrada, lido: &Lido) -> Resultado<Importado> {
    gravar_com(conn, dispositivo, usuario_id, e, lido, None)
}

fn gravar_com(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    e: &Entrada,
    lido: &Lido,
    compras: Option<&GanchoCompras>,
) -> Resultado<Importado> {
    let d = decidir(conn, e, lido)?;
    if !e.carimbo.is_empty() && carimbo(conn, &d.opcoes, lido)? != e.carimbo {
        return Err(Erro::validacao(CARIMBO_VENCIDO));
    }
    let resumo = resumir(conn, lido, &d)?;
    let mut r = aplicar(Gravacao { conn, dispositivo, usuario_id }, lido, &d)?;
    // T108 (texto R): as notas de compra vão para Compras, em rascunho, DEPOIS dos cadastros (os itens já nascem
    // ligados às peças cadastradas agora). As regras de Compras continuam: o estoque, o custo e o A pagar só mudam
    // quando a compra for lançada. Na mesma transação: ou tudo, ou nada.
    if let Some(g) = compras.filter(|_| d.opcoes.compras) {
        for (nome, xml, v) in &lido.compras {
            // A9 da revisão da T108: se uma compra não puder ser criada, nada é gravado (a mesma transação), e o
            // aviso diz de qual arquivo.
            (g.criar)(conn, xml, v).map_err(|e| match e {
                Erro::Validacao { campo, mensagem } => {
                    Erro::Validacao { campo, mensagem: format!("{nome}: {mensagem}") }
                }
                outro => outro,
            })?;
            r.compras += 1;
        }
    }
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
    let mut gtin_novo = GtinsEmPecasExistentes::default();

    // Etapa 1: os clientes das notas da loja que ainda não estão cadastrados; FI-3 (R17): quem já existe só como
    // fornecedor passa a ser também cliente, com o motivo no registro.
    for p in &d.clientes {
        if p.vira_cliente {
            if let Some(id) = pessoa_existe(conn, &p.doc)? {
                pessoas::passar_a_cliente(
                    conn,
                    dispositivo,
                    usuario_id,
                    &id,
                    "Apareceu como cliente numa nota da loja (Trazer cadastros)",
                )?;
            }
        }
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

    // Etapa 2: os fornecedores que faltam; FI-2: quem já existe e passa a ser
    // fornecedor (a mesma função da compra: "Passou a ser fornecedor").
    for p in &d.fornecedores {
        if p.vira_fornecedor {
            let (pessoa, _) = &lido.fornecedores[&p.doc];
            pessoas::garantir_pela_nota(conn, dispositivo, usuario_id, pessoa, false, "")?;
        }
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
            DecisaoLoja::OutraDescricaoPeloGtin { pid, gtins_novos } => {
                produtos::registrar_gtins_xml(conn, &reais.de(pid), &gtins_ficha)?;
                gtin_novo.anotar(pid, gtins_novos, NOTA_DA_LOJA);
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
            DecisaoLoja::Existente { pid, gtins_novos } => {
                if let Some(pid) = pid {
                    produtos::registrar_gtins_xml(conn, &reais.de(pid), &gtins_ficha)?;
                    gtin_novo.anotar(pid, gtins_novos, NOTA_DA_LOJA);
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
                } else {
                    f.preco_aberto = true;
                }
                let id = salvar_peca(conn, dispositivo, usuario_id, f)?;
                reais.0.insert(plano.clone(), id.clone());
                produtos::registrar_gtins_xml(conn, &id, &gtins_ficha)?;
                let mut detalhes = vec!["Cadastrada pelo XML de uma nota da loja".to_string()];
                // T074 (OPUS-C-1): o código da nota que a faixa deste caixa recusou fica como código antigo da
                // peça (a busca, o leitor e a reimportação continuam achando-a por ele).
                if !codigo_peca.is_empty() {
                    let atual: String =
                        conn.query_row("SELECT codigo FROM produto WHERE id = ?1", [&id], |l| l.get(0))?;
                    if texto::compactar(&atual) != texto::compactar(codigo_peca) {
                        crate::catalogo::codigos::guardar_codigo_antigo(conn, codigo_peca, &id, &tempo::agora())?;
                        produtos::atualizar_indice_busca(conn, &id)?;
                        detalhes.push(format!(
                            "O código {codigo_peca} do sistema antigo não é da faixa de códigos deste caixa: ficou como \
                             código antigo"
                        ));
                    }
                }
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
        let nota = if i.numero_nfe.is_empty() {
            format!("nota de {}", i.forn_nome)
        } else {
            format!("NF-e {} de {}", i.numero_nfe, i.forn_nome)
        };
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
            DecisaoForn::PeloGtin { pid, ligar, gtins_novos } => {
                gtin_novo.anotar(pid, gtins_novos, &nota);
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
            DecisaoForn::PeloVinculo { pid, gtins, gtins_novos } => {
                gtin_novo.anotar(pid, gtins_novos, &nota);
                let pid = reais.de(pid);
                if *gtins {
                    produtos::registrar_gtins_xml(conn, &pid, &gtins_ficha)?;
                }
                if let Some(fid) = &fornecedor_id {
                    resolver_pendencias_do_item(conn, fid, codigo, &it.descricao, &pid)?;
                }
            }
            DecisaoForn::MesmaDescricao { pid, gtins_novos } => {
                produtos::registrar_gtins_xml(conn, &reais.de(pid), &gtins_ficha)?;
                gtin_novo.anotar(pid, gtins_novos, &nota);
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
                } else {
                    f.preco_aberto = true;
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
    // A03b (lote 2, item 04): uma linha de auditoria por peça que JÁ existia e
    // recebeu GTIN novo, com os GTINs e as notas. Antes não ficava rastro.
    for (pid, gtins, notas) in &gtin_novo.0 {
        auditoria::registrar(
            conn,
            dispositivo,
            Some(usuario_id),
            "produto.gtin_importado",
            "produto",
            pid,
            &[format!("GTIN novo pelo XML: {}", gtins.join(", ")), format!("Origem: {}", notas.join("; "))],
        )?;
    }
    Ok(r)
}

/// A03b: os GTINs novos de cada peça que já existia, com as notas de onde
/// vieram, na ordem em que aparecem.
#[derive(Default)]
struct GtinsEmPecasExistentes(Vec<(String, Vec<String>, Vec<String>)>);

impl GtinsEmPecasExistentes {
    fn anotar(&mut self, pid: &str, gtins: &[String], nota: &str) {
        if gtins.is_empty() || pid.starts_with(PLANEJADO) {
            return;
        }
        let i = match self.0.iter().position(|(p, _, _)| p == pid) {
            Some(i) => i,
            None => {
                self.0.push((pid.to_string(), Vec::new(), Vec::new()));
                self.0.len() - 1
            }
        };
        let (_, g, n) = &mut self.0[i];
        for x in gtins {
            if !g.contains(x) {
                g.push(x.clone());
            }
        }
        if !n.iter().any(|x| x == nota) {
            n.push(nota.to_string());
        }
    }
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

/// T111 (Achado 4 — antiduplicidade Regra 4): peças já cadastradas no banco ou
/// planejadas neste lote com a mesma descrição compacta (até 5 candidatas).
fn plano_por_descricao_compacta(conn: &Connection, plano: &Plano, it: &ItemXml) -> Resultado<Vec<String>> {
    let mut achados = crate::catalogo::codigos::pecas_por_descricao_compacta(conn, &it.descricao)?;
    let alvo = descricao_chave(it);
    if !alvo.is_empty() && achados.len() < 5 {
        for (pid, nome) in &plano.pecas {
            if texto::compactar(nome) == alvo && !achados.contains(pid) {
                achados.push(pid.clone());
                if achados.len() == 5 {
                    break;
                }
            }
        }
    }
    Ok(achados)
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

/// A3 da revisão da T108: o texto quando "Notas de compra em Compras" vem sem "Fornecedores".
const COMPRAS_EXIGEM_FORNECEDORES: &str =
    "Para levar as notas de compra a Compras, marque também \"Fornecedores\": cada compra é de um fornecedor.";

/// FI-1 (a): o texto quando "Peças do fornecedor" vem sem "Fornecedores".
const PECAS_EXIGEM_FORNECEDORES: &str = "\"Peças do fornecedor\" precisa de \"Fornecedores\" marcada: as peças ficam \
     ligadas ao fornecedor, e ele só é cadastrado com a opção dele.";

/// N06 (a): a peça está desativada? (A peça do plano é nova, então ativa.)
fn desativada(conn: &Connection, pid: &str) -> Resultado<bool> {
    if pid.starts_with(PLANEJADO) {
        return Ok(false);
    }
    Ok(matches!(nome_e_ativo(conn, pid)?, Some((_, false))))
}

/// FI-3 (R17): a pessoa já cadastrada é cliente? (Uma pessoa do plano é nova: ainda não.)
fn plano_e_cliente(conn: &Connection, id: &str) -> Resultado<bool> {
    if id.starts_with(PLANEJADO) {
        return Ok(false);
    }
    Ok(conn
        .query_row("SELECT cliente FROM pessoa WHERE id = ?1", [id], |l| l.get::<_, i64>(0))
        .optional()?
        .is_some_and(|c| c != 0))
}

/// FI-2: a pessoa já é fornecedora? A do plano foi cadastrada como cliente
/// nesta importação (etapa 1), então ainda não é.
fn plano_e_fornecedor(conn: &Connection, id: &str) -> Resultado<bool> {
    if id.starts_with(PLANEJADO) {
        return Ok(false);
    }
    Ok(conn
        .query_row("SELECT fornecedor FROM pessoa WHERE id = ?1", [id], |l| l.get::<_, i64>(0))
        .optional()?
        .is_some_and(|f| f != 0))
}

/// Uma pessoa das notas: o documento e se a importação a cadastra.
struct DecisaoPessoa {
    doc: String,
    /// Id do plano quando a importação a cadastra.
    nova: Option<String>,
    /// FI-2 (a): a pessoa já cadastrada (ou que esta importação cadastra como
    /// cliente) que ainda não é fornecedora e passa a ser, com "Fornecedores"
    /// marcada, como a entrada de compra pelo XML faz.
    vira_fornecedor: bool,
    /// FI-3 (a), R17 (decisão do proprietário de 25/11/2026, regra 17): a pessoa
    /// já cadastrada só como fornecedora que aparece como cliente numa nota da
    /// loja passa a ser também cliente, com "Clientes" marcada, com registro de
    /// quando e por quê.
    vira_cliente: bool,
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
    // FI-1 (a): "Peças do fornecedor" exige "Fornecedores". As peças ficam
    // ligadas ao fornecedor, que tem de estar marcado para ser cadastrado:
    // nenhum fornecedor entra sem a opção dele (nada implícito).
    if e.pecas_fornecedor && !e.fornecedores {
        return Err(Erro::campo("fornecedores", PECAS_EXIGEM_FORNECEDORES));
    }
    // A3 da revisão da T108: as notas de compra também exigem "Fornecedores" (a compra é de um fornecedor, que entra
    // pela opção dele, com a prévia contando; nada implícito).
    if e.compras && !e.fornecedores {
        return Err(Erro::campo("fornecedores", COMPRAS_EXIGEM_FORNECEDORES));
    }
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
        let (nova, vira_cliente) = if !e.clientes {
            (None, false)
        } else {
            match plano_pessoa(conn, &plano, doc)? {
                Some(id) => (None, !plano_e_cliente(conn, &id)?),
                None => (Some(plano.nova_pessoa(doc)), false),
            }
        };
        d.clientes.push(DecisaoPessoa { doc: doc.clone(), nova, vira_fornecedor: false, vira_cliente });
    }

    // Etapa 2: fornecedores (só com "Fornecedores"; FI-1). FI-2: a pessoa que
    // já existe e não é fornecedora passa a ser.
    let mut ids_fornecedor: HashMap<String, String> = HashMap::new();
    let mut lista: Vec<_> = lido.fornecedores.keys().collect();
    lista.sort();
    for doc in lista {
        let (nova, vira_fornecedor) = match plano_pessoa(conn, &plano, doc)? {
            Some(id) => {
                let vira = e.fornecedores && !plano_e_fornecedor(conn, &id)?;
                ids_fornecedor.insert(doc.clone(), id);
                (None, vira)
            }
            None if e.fornecedores => {
                let id = plano.nova_pessoa(doc);
                ids_fornecedor.insert(doc.clone(), id.clone());
                (Some(id), false)
            }
            None => (None, false),
        };
        d.fornecedores.push(DecisaoPessoa { doc: doc.clone(), nova, vira_fornecedor, vira_cliente: false });
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
                // T074: o código que a faixa recusou ficou como código antigo (com a mesma descrição).
                match plano_por_codigo(conn, &plano, codigo)? {
                    Some(id) => Some(id),
                    None => peca_repetida(conn, codigo, it)?,
                }
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
            // N06 (a): a peça achada (pelo GTIN ou pelo vínculo) está desativada.
            // Nada é ligado nem gravado nela, e nenhuma pendência fecha nela; o
            // item vira pendência, com ela como candidata. Resolver segue o A07:
            // reativar a peça ou escolher outra.
            let achada = match (&gtin_busca, &forn_busca) {
                (BuscaId::Unico(p), _) | (_, BuscaId::Unico(p)) => Some(p.clone()),
                _ => None,
            };
            if let Some(pid) = achada {
                if desativada(conn, &pid)? {
                    if fornecedor_id.is_some() || e.fornecedores {
                        break 'decisao pendencia("peca_desativada", vec![pid]);
                    }
                    break 'decisao DecisaoForn::Nada;
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
                        // N06 (a): a peça de mesma descrição está desativada.
                        if desativada(conn, &pid)? {
                            break 'decisao pendencia("peca_desativada", vec![pid]);
                        }
                        let gtins_novos = plano_registrar(conn, &mut plano, &pid, &gtins_ficha)?;
                        break 'decisao DecisaoForn::MesmaDescricao { pid, gtins_novos };
                    }
                } else {
                    // T111 (Achado 4 — Regra 4): sem GTIN nem vínculo, bloqueia
                    // duplicar peça existente de mesma descrição compacta e abre
                    // registro provisório com as candidatas.
                    let mesmas_desc = plano_por_descricao_compacta(conn, &plano, it)?;
                    if !mesmas_desc.is_empty() {
                        break 'decisao pendencia("possivel_duplicata_descricao", mesmas_desc);
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
    /// T108: as notas de compra vão para Compras, em rascunho.
    pub compras: bool,
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
    /// Peças desativadas que a importação acha (pelo GTIN, pelo vínculo ou pela
    /// descrição). N06 (a): nada é gravado nelas; os itens vão para pendências
    /// (`peca_desativada`).
    pub pecas_desativadas: i64,
    pub exemplos_pecas_desativadas: Vec<String>,
}

/// O texto de cada categoria, na ordem em que a tela mostra.
const CATEGORIAS: &[(&str, &str)] = &[
    ("cliente.novo", "Clientes novos"),
    ("cliente.ja", "Clientes já cadastrados"),
    ("cliente.ja_sem_papel", "Já cadastrados (como fornecedor) que passam a ser clientes"),
    ("cliente.nao_cadastrado", "Clientes que não entram (opção desmarcada)"),
    ("fornecedor.novo", "Fornecedores novos"),
    ("fornecedor.ja", "Fornecedores já cadastrados"),
    ("fornecedor.ja_sem_papel", "Já cadastrados (como cliente) que passam a ser fornecedores"),
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
    ("compra.nova", "Notas de compra que vão para Compras, em rascunho"),
    ("compra.ja", "Notas de compra que já estavam em Compras (não entram de novo)"),
    ("compra.nao_importada", "Notas de compra que não vão para Compras (opção desmarcada)"),
    (
        "compra.nao_serve",
        "Notas de fornecedor que não servem para Compras (sem a autorização da SEFAZ, para outro CNPJ, devolução, \
         NFC-e ou outro motivo): só os cadastros",
    ),
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
        } else if p.vira_cliente {
            conta("cliente.ja_sem_papel", nome_pessoa(em));
        } else if pessoa_existe(conn, &p.doc)?.is_some() {
            conta("cliente.ja", String::new());
        } else {
            conta("cliente.nao_cadastrado", nome_pessoa(em));
        }
    }
    for p in &d.fornecedores {
        let (em, _) = &lido.fornecedores[&p.doc];
        match &p.nova {
            Some(_) => conta("fornecedor.novo", nome_pessoa(em)),
            None if p.vira_fornecedor => conta("fornecedor.ja_sem_papel", nome_pessoa(em)),
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
            DecisaoForn::Pendencia { motivo, candidatos } => {
                conta("forn.pendencia", desc);
                if *motivo == "peca_desativada" {
                    for c in candidatos {
                        achada(c, &mut desativadas)?;
                    }
                }
                let m = motivos.entry(motivo).or_default();
                m.0 += 1;
                if pendencia_aberta(conn, &i.chave.0, &i.chave.1, &it.descricao)? {
                    m.1 += 1;
                }
            }
            DecisaoForn::JaLigadoFecha { .. } => conta("forn.ja_ligado", String::new()),
            DecisaoForn::PeloGtin { pid, ligar, gtins_novos } => {
                conta(if *ligar { "forn.ja_pelo_gtin_liga" } else { "forn.ja_pelo_gtin" }, desc);
                if *ligar || !gtins_novos.is_empty() {
                    muda(pid, &mut que_mudam);
                }
            }
            DecisaoForn::PeloVinculo { pid, gtins_novos, .. } => {
                if gtins_novos.is_empty() {
                    conta("forn.ja_pelo_vinculo", String::new());
                } else {
                    conta("forn.ja_pelo_vinculo_gtin_novo", desc);
                    muda(pid, &mut que_mudam);
                }
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
    // T108: as notas de compra.
    for (_, _, v) in &lido.compras {
        let exemplo = format!("NF {} de {}", v.nota.numero, nome_pessoa(&v.nota.emitente));
        conta(if d.opcoes.compras { "compra.nova" } else { "compra.nao_importada" }, exemplo);
    }
    for _ in 0..lido.compras_ja {
        conta("compra.ja", String::new());
    }
    for _ in 0..lido.compras_nao_serve {
        conta("compra.nao_serve", String::new());
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

/// R7 (D-AUD-6, decisão do proprietário de 25/11/2026): as tabelas cujas mudanças vencem a prévia: peças (com
/// preços, aplicações, equivalentes e unidades), pessoas e códigos (GTIN, códigos de fornecedor, códigos antigos e
/// faixas de códigos). Uma venda, um recebimento ou qualquer outra gravação não vence mais.
const TABELAS_DA_PREVIA: &str = "'produto', 'produto_preco', 'produto_aplicacao', 'produto_equivalente', \
     'produto_fornecedor', 'produto_codigo_antigo', 'produto_gtin', 'faixa_codigo', 'unidade', 'pessoa', \
     'pessoa_veiculo'";

/// P-4: o carimbo de uma prévia. Muda se mudam as opções, o conteúdo dos
/// arquivos lidos ou os cadastros (R7: só as peças, as pessoas e os códigos; a
/// fila de alterações da troca anda a cada gravação delas).
fn carimbo(conn: &Connection, e: &Opcoes, lido: &Lido) -> Resultado<String> {
    use sha2::Digest;
    let seq: i64 = conn.query_row(
        &format!("SELECT COALESCE(MAX(seq), 0) FROM alteracao WHERE tabela IN ({TABELAS_DA_PREVIA})"),
        [],
        |l| l.get(0),
    )?;
    let mut h = sha2::Sha256::new();
    h.update([e.clientes as u8, e.fornecedores as u8, e.pecas_loja as u8, e.pecas_fornecedor as u8, e.compras as u8]);
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
    // T074 (OPUS-C-1): só o que foi recusado sai (antes, um código recusado levava junto o código de barras e
    // os GTINs, e a peça não era mais achada na reimportação). Quem chama guarda o código como código antigo.
    for _ in 0..2 {
        match produtos::salvar(conn, dispositivo, usuario_id, &f) {
            Err(Erro::Validacao { campo: Some(c), .. }) if c == "codigo" && !f.codigo.is_empty() => f.codigo.clear(),
            Err(Erro::Validacao { campo: Some(c), .. })
                if c == "codigoBarras" && (!f.codigo_barras.is_empty() || !f.gtins.is_empty()) =>
            {
                f.codigo_barras.clear();
                f.gtins.clear();
            }
            r => return r,
        }
    }
    produtos::salvar(conn, dispositivo, usuario_id, &f)
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
    /// T111 (Decisão D-T108 Opção C — Regra 3): correções opcionais de identificação ao cadastrar
    /// a peça a partir do registro provisório, preservando NCM/CEST/origem/custo do XML sem redigitar.
    /// Quando `corrigir == true`, os campos de identificação abaixo valem literalmente (inclusive
    /// `codigo_fornecedor == ""` ou `codigo_barras == ""` para limpar valores errados do XML).
    pub corrigir: bool,
    pub nome: String,
    pub codigo_fornecedor: String,
    pub codigo_barras: String,
    pub unidade: String,
    /// R6: o motivo de "descartar".
    pub motivo: String,
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

pub(crate) fn motivo_legivel(motivo: &str) -> &'static str {
    match motivo {
        "gtin_ambiguo" => "GTIN/código de barras corresponde a mais de uma peça cadastrada",
        "codigo_so_de_simbolos" => "O código do fornecedor só tem símbolos e não identifica a peça",
        "codigo_fraco_sem_gtin" => "Código do fornecedor muito curto e sem GTIN válido: confira antes de cadastrar",
        "possivel_duplicata_descricao" => {
            "Já existe peça com a mesma descrição no cadastro (código diferente, sem GTIN): confira para evitar duplicidade"
        }
        "fornecedor_ambiguo" => "Código do fornecedor está associado a mais de uma peça",
        "conflito_gtin_fornecedor" => "GTIN aponta para uma peça e o código do fornecedor para outra",
        "ligacao_em_disputa" => {
            "Dois caixas sem rede ligaram este código do fornecedor a peças diferentes: o gerente escolhe qual vale nas Diferenças entre caixas"
        }
        "codigo_repetido" => "O fornecedor usou o mesmo código em peças com descrições diferentes na nota",
        "codigo_ambiguo" => "Código original/equivalente corresponde a mais de uma peça",
        "codigo_de_outra_peca" => "O código desta nota da loja já é de uma peça com outra descrição",
        "peca_desativada" => "A peça achada pelo GTIN ou pelo código está desativada: reative-a ou escolha outra",
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
    // R6 (D-AUD-5): descartar, com o motivo (a permissão do gerente é conferida no comando).
    if d.acao == "descartar" {
        crate::compras::pendencias::descartar(conn, dispositivo, usuario_id, &d.id, &d.motivo)?;
        return Ok(PendenciaResolvida { produto_id: String::new(), pendentes: contar_pendencias_ou_erro(conn)? });
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
    let mut gtins_ficha = gtins_para_ficha(&gtins_xml, fallback);

    // Resolve o fornecedor antes da validação para conferir conflito de código (Achado 3c).
    let fid = match fornecedor_id {
        Some(id) if !id.is_empty() => Some(id),
        _ if !fornecedor_doc.is_empty() => pessoa_existe(conn, &fornecedor_doc)?,
        _ => None,
    };

    // T111 (Regra 3 + Achado 3): correções de identificação preservando os demais dados do XML.
    let acao = d.acao.trim();
    let nome_corrigido = texto::limpar(&d.nome);
    if acao == "nova" && d.corrigir && nome_corrigido.is_empty() {
        return Err(Erro::campo("nome", "Informe o nome da peça."));
    }
    let cod_forn_corrigido = texto::limpar(&d.codigo_fornecedor);
    if !cod_forn_corrigido.is_empty() {
        if texto::tamanho(&cod_forn_corrigido) > 60 {
            return Err(Erro::campo("codigoFornecedor", "Código no fornecedor: use no máximo 60 caracteres."));
        }
        if texto::compactar(&cod_forn_corrigido).is_empty() {
            return Err(Erro::campo("codigoFornecedor", "O código no fornecedor precisa ter letras ou números."));
        }
    }
    let cod_para_vincular = if d.corrigir || !cod_forn_corrigido.is_empty() {
        cod_forn_corrigido.as_str()
    } else {
        codigo_fornecedor.trim()
    };
    // V2-4: nunca transferir vínculo de outra peça em silêncio ao criar nova peça (com ou sem `corrigir`).
    if acao == "nova" && !cod_para_vincular.is_empty() {
        if let Some(fid_ref) = &fid {
            let comp = texto::compactar(cod_para_vincular);
            if let Some((outra_id, _)) = crate::catalogo::codigos::vinculo_do_fornecedor(conn, fid_ref, &comp)? {
                let outra_nome: String = conn
                    .query_row("SELECT nome FROM produto WHERE id = ?1", [&outra_id], |l| l.get(0))
                    .optional()?
                    .unwrap_or_else(|| "outra peça".into());
                return Err(Erro::campo(
                    "codigoFornecedor",
                    format!(
                        "O código {} deste fornecedor já está ligado à peça \"{}\". Use \"Escolher peça existente\" ou informe outro código.",
                        cod_para_vincular, outra_nome
                    ),
                ));
            }
        }
    }

    let orig_barras: String = gtin_comercial.trim().chars().filter(|c| !matches!(c, ' ' | '-')).collect();
    let barras_corrigido: String = d.codigo_barras.trim().chars().filter(|c| !matches!(c, ' ' | '-')).collect();
    let barras_alterado = d.corrigir && barras_corrigido != orig_barras;
    if d.corrigir && acao == "nova" {
        // Achado 3b + V2-3: só descarta GTINs adicionais/tributáveis do XML se o usuário
        // efetivamente alterou ou limpou o código de barras; se manteve o mesmo GTIN
        // comercial (ex.: editou só o nome), preserva todos os GTINs do XML.
        if barras_alterado {
            gtins_ficha.clear();
        }
        if !barras_corrigido.is_empty() {
            if !gtin::gtin_valido(&barras_corrigido) {
                return Err(Erro::campo("codigoBarras", "Código de barras (GTIN) inválido. Confira os dígitos."));
            }
            if peca_por_barras(conn, &barras_corrigido)?.is_some() {
                return Err(Erro::campo("codigoBarras", "Este código de barras já está cadastrado em outra peça."));
            }
            if barras_alterado || !gtins_ficha.iter().any(|g| g.gtin == barras_corrigido) {
                gtins_ficha.insert(
                    0,
                    produtos::GtinFicha {
                        gtin: barras_corrigido.clone(),
                        tipo: "comercial".into(),
                        origem: "manual".into(),
                    },
                );
            }
        }
    } else if !barras_corrigido.is_empty() {
        if !gtin::gtin_valido(&barras_corrigido) {
            return Err(Erro::campo("codigoBarras", "Código de barras (GTIN) inválido. Confira os dígitos."));
        }
        if peca_por_barras(conn, &barras_corrigido)?.is_some() {
            return Err(Erro::campo("codigoBarras", "Este código de barras já está cadastrado em outra peça."));
        }
        if !gtins_ficha.iter().any(|g| g.gtin == barras_corrigido) {
            gtins_ficha.insert(
                0,
                produtos::GtinFicha {
                    gtin: barras_corrigido.clone(),
                    tipo: "comercial".into(),
                    origem: "manual".into(),
                },
            );
        }
    }
    let unidade_corrigida = texto::limpar(&d.unidade);
    let houve_correcao = d.corrigir
        || !nome_corrigido.is_empty()
        || !cod_forn_corrigido.is_empty()
        || !barras_corrigido.is_empty()
        || !unidade_corrigida.is_empty();

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
            // Lote 2, item 06: o campo próprio diz à tela que é a peça
            // desativada (ela oferece "Abrir a peça"); a mensagem é a mesma.
            if !ativa {
                return Err(Erro::campo(
                    "pecaDesativada",
                    format!("A peça \"{nome}\" (código {codigo}) está desativada. Reative a peça ou escolha outra."),
                ));
            }
            d.produto_id.trim().to_string()
        }
        "nova" => {
            let mut f = produtos::ficha_nova(conn)?;
            let nome_base = if !nome_corrigido.is_empty() { &nome_corrigido } else { &descricao };
            f.nome = texto::limpar(nome_base).chars().take(120).collect();
            if f.nome.is_empty() {
                f.nome = "Peça sem nome".into();
            }
            if d.corrigir || !barras_corrigido.is_empty() {
                f.codigo_barras = barras_corrigido.clone();
            } else if !fallback.is_empty() && gtin::gtin_valido(fallback) && peca_por_barras(conn, fallback)?.is_none()
            {
                f.codigo_barras = fallback.to_string();
            }
            f.gtins = gtins_ficha.clone();
            f.unidade =
                crate::unidades::de_fora(if !unidade_corrigida.is_empty() { &unidade_corrigida } else { &unidade });
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
            } else {
                // Achado 1 (Crítico): item sem custo (bonificação/brinde) nasce com preço na hora da venda.
                f.preco_aberto = true;
            }
            salvar_peca(conn, dispositivo, usuario_id, f)?
        }
        _ => {
            return Err(Erro::validacao("Escolha se quer associar a uma peça existente ou cadastrar como peça nova."));
        }
    };

    // Garante o vínculo do fornecedor (se houver fornecedor identificado e código válido) e os GTINs.
    if let Some(fid) = &fid {
        let compacto = texto::compactar(cod_para_vincular);
        if !compacto.is_empty() {
            let ja_ligado = if acao == "nova" {
                crate::catalogo::codigos::JaLigado::Manter
            } else {
                crate::catalogo::codigos::JaLigado::PassarPeca
            };
            crate::catalogo::codigos::vincular_fornecedor(conn, fid, cod_para_vincular, &produto_id, 1000, ja_ligado)?;
            produtos::atualizar_indice_busca(conn, &produto_id)?;
        }
    }
    if !(d.corrigir && acao == "nova" && barras_alterado) {
        produtos::registrar_gtins_xml(conn, &produto_id, &gtins_ficha)?;
    }

    let agora = tempo::agora();
    if d.corrigir && acao == "nova" {
        let gtins_corr_json = if !barras_alterado {
            gtins_json.clone()
        } else if barras_corrigido.is_empty() {
            "[]".to_string()
        } else {
            serde_json::to_string(&vec![nfe_xml::GtinXml {
                codigo: barras_corrigido.clone(),
                tipo: "comercial".into(),
                origem: "manual".into(),
            }])
            .unwrap_or_else(|_| "[]".to_string())
        };
        crate::compras::pendencias::marcar_resolvida_com_correcao(
            conn,
            &d.id,
            &produto_id,
            &agora,
            cod_para_vincular,
            &gtins_corr_json,
        )?;
    } else {
        crate::compras::pendencias::marcar_resolvida(conn, &d.id, &produto_id, &agora)?;
    }
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
            if acao == "nova" {
                if houve_correcao {
                    "peça nova com identificação corrigida"
                } else {
                    "peça nova"
                }
            } else {
                "associada"
            },
            if fornecedor_nome.is_empty() { &fornecedor_doc } else { &fornecedor_nome },
            if cod_para_vincular.is_empty() { "—" } else { cod_para_vincular }
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

    use crate::despacho::{ler, marcar_feita, Classe, Comando, Pedido};
    use crate::erro::Resultado;
    use crate::{importar, requisitos};

    // T108 (texto R): o `importar_previa` e o `importar_cadastros` foram para os comandos de Compras (`compras::
    // comandos`), com o mesmo nome, entrada, resposta e permissão: eles também levam as notas de compra a Compras,
    // e só o módulo de Compras conhece as regras dele (este módulo não depende dele).
    pub const COMANDOS: &[Comando] = &[
        Comando { nome: "arquivos_conferir", classe: Classe::Leitura, funcao: arquivos_conferir },
        Comando { nome: "xml_pendencias_listar", classe: Classe::Leitura, funcao: xml_pendencias_listar },
        Comando { nome: "xml_pendencia_resolver", classe: Classe::Caixa, funcao: xml_pendencia_resolver },
    ];

    fn arquivos_conferir(_p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: importar::Entrada = ler(dados)?;
        Ok(json!(requisitos::arquivos(&d.arquivos)))
    }

    fn xml_pendencias_listar(p: &mut Pedido, _dados: Value) -> Resultado<Value> {
        Ok(json!(importar::listar_pendencias(p.conn)?))
    }

    fn xml_pendencia_resolver(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: importar::ResolverPendencia = ler(dados)?;
        // R6 (D-AUD-5) e DP-13 (alternativa recomendada): descartar a pendência é do gerente, com motivo. O motivo
        // vazio é recusado antes de pedir a senha (B-N1 da revisão R2: o gerente não digita a senha à toa).
        if d.acao == "descartar" {
            let motivo = crate::texto::limpar(&d.motivo);
            if motivo.is_empty() {
                return Err(crate::erro::Erro::campo("motivo", "Diga por que descartar esta pendência."));
            }
            // B-R3-3 da revisão R3: o motivo comprido também é recusado antes da senha.
            if crate::texto::tamanho(&motivo) > 200 {
                return Err(crate::erro::Erro::campo("motivo", "Use no máximo 200 letras."));
            }
            p.exigir("pendencia_xml")?;
        }
        let tx = p.conn.transaction()?;
        let r = importar::resolver_pendencia(&tx, p.disp, p.usuario_id, &d)?;
        let resposta = json!(r);
        marcar_feita(&tx, p.operacao.as_ref(), &resposta)?;
        tx.commit()?;
        Ok(resposta)
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

    /// EAN-13 com o dígito certo.
    fn ean13(base12: u64) -> String {
        let b = format!("{base12:012}");
        let s: u32 = b.chars().enumerate().map(|(i, c)| c.to_digit(10).unwrap() * if i % 2 == 1 { 3 } else { 1 }).sum();
        format!("{b}{}", (10 - s % 10) % 10)
    }

    /// CNPJ com os dígitos certos.
    fn cnpj(base12: &str) -> String {
        let dv = |n: &str, pesos: &[u32]| {
            let s: u32 = n.chars().zip(pesos).map(|(c, p)| c.to_digit(10).unwrap() * p).sum();
            let r = s % 11;
            if r < 2 {
                0
            } else {
                11 - r
            }
        };
        let d1 = dv(base12, &[5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2]);
        let n13 = format!("{base12}{d1}");
        let d2 = dv(&n13, &[6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2]);
        format!("{n13}{d2}")
    }

    /// Lote 2 (T-1, O-3, A03b), a trava "prévia = resultado": um lote com um
    /// item de cada caminho, nas 16 combinações das opções, cada uma num banco
    /// novo. A prévia roda com o banco SÓ PARA LEITURA (não grava nada) e o
    /// que ela anuncia é conferido no banco depois da importação: peças,
    /// vínculos, pessoas, pendências, peças que já existiam e mudam, e a
    /// auditoria do GTIN novo (uma linha por peça que já existia).
    #[test]
    fn o3_previa_igual_ao_resultado() {
        let f1 = cnpj("112223330001");
        let f2 = cnpj("998887770001");
        // FI-2: f3 já é cliente (só cliente) e vende para a loja; f4 aparece
        // nas notas da loja (cliente) e nas de compra (fornecedor), sem cadastro.
        let f3 = cnpj("554443330001");
        let f4 = cnpj("776665550001");
        // FI-3 (R17): f5 já é fornecedor (só fornecedor) e aparece como cliente nas notas da loja.
        let f5 = cnpj("334445550001");
        let cliente = "52998224725";
        let (g1, g2, g2b, g3, g4, g5, g6) = (
            ean13(789_100_000_001),
            ean13(789_100_000_002),
            ean13(789_100_000_022),
            ean13(789_100_000_003),
            ean13(789_100_000_004),
            ean13(789_100_000_005),
            ean13(789_100_000_006),
        );
        let gtin =
            |g: &str| vec![nfe_xml::GtinXml { codigo: g.into(), tipo: "comercial".into(), origem: "cEAN".into() }];
        let item = |codigo: &str, descricao: &str, gtins: Vec<nfe_xml::GtinXml>| ItemXml {
            codigo: codigo.into(),
            descricao: descricao.into(),
            gtins,
            quantidade: 1000,
            valor_produtos: 5000,
            ..Default::default()
        };
        let mut lido = Lido::default();
        for (doc, nome) in [
            (cliente.to_string(), "CLIENTE NOVO"),
            (f4.clone(), "CLIENTE QUE TAMBEM VENDE"),
            (f5.clone(), "FORNECEDOR QUE COMPRA"),
        ] {
            lido.clientes.insert(
                doc.clone(),
                (Emitente { documento: doc, nome: nome.into(), ..Default::default() }, "2026-11-01".into()),
            );
        }
        for (doc, nome) in [
            (&f1, "FORNECEDOR UM"),
            (&f2, "FORNECEDOR NOVO"),
            (&f3, "CLIENTE ANTIGO QUE VENDE"),
            (&f4, "CLIENTE QUE TAMBEM VENDE"),
        ] {
            lido.fornecedores.insert(
                doc.clone(),
                (Emitente { documento: doc.clone(), nome: nome.into(), ..Default::default() }, "2026-11-01".into()),
            );
        }
        let itens_f1 = [
            item("A1", "PECA ACHADA PELO GTIN", gtin(&g1)),
            item("B1", "PECA LIGADA COM GTIN NOVO", vec![gtin(&g2)[0].clone(), gtin(&g2b)[0].clone()]),
            item("C1", "PECA DESATIVADA", gtin(&g3)),
            item("D1", "PECA NOVA", gtin(&g4)),
            item("---", "SO SIMBOLOS", Vec::new()),
            item("R1", "PECA MESMA DESCRICAO", Vec::new()),
            item("R1", "OUTRA PECA R1", Vec::new()),
            item("B2", "CONFLITO GTIN E VINCULO", gtin(&g1)),
            item("K1", "PECA K", gtin(&g5)),
            item("K2", "PECA K", gtin(&g5)),
        ];
        for it in itens_f1 {
            lido.pecas_fornecedor
                .insert((f1.clone(), it.codigo.clone(), descricao_chave(&it)), (it, "2026-11-01".into()));
        }
        for (doc, it) in [
            (&f2, item("N1", "PECA DO FORNECEDOR NOVO", Vec::new())),
            (&f3, item("N3", "PECA DO CLIENTE ANTIGO", Vec::new())),
            (&f4, item("N4", "PECA DO CLIENTE NOVO", Vec::new())),
        ] {
            lido.pecas_fornecedor
                .insert((doc.clone(), it.codigo.clone(), descricao_chave(&it)), (it, "2026-11-01".into()));
        }
        for it in [item("LJ1", "OUTRA COISA NA LOJA", Vec::new()), item("LJ2", "PECA LOJA NOVA", gtin(&g6))] {
            lido.pecas_loja.insert((it.codigo.clone(), descricao_chave(&it)), (it, "2026-11-01".into()));
        }

        let mut erros = Vec::new();
        for n in 0..16u8 {
            let e = Entrada {
                clientes: n & 8 != 0,
                fornecedores: n & 4 != 0,
                pecas_loja: n & 2 != 0,
                pecas_fornecedor: n & 1 != 0,
                ..Default::default()
            };
            let b = Banco::em_memoria().unwrap();
            let (c, d) = (&b.conn, b.dispositivo.as_str());
            let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
            crate::faixas::comecar_loja(c).unwrap();
            config::gravar(c, "loja_cnpj", "11444777000161").unwrap();
            let em = Emitente { documento: f1.clone(), nome: "FORNECEDOR UM".into(), ..Default::default() };
            let (fid, _) = crate::pessoas::garantir_pela_nota(c, d, &u, &em, false, "teste").unwrap();
            let em3 = Emitente { documento: f3.clone(), nome: "CLIENTE ANTIGO QUE VENDE".into(), ..Default::default() };
            crate::pessoas::garantir_pela_nota(c, d, &u, &em3, true, "teste").unwrap();
            let em5 = Emitente { documento: f5.clone(), nome: "FORNECEDOR QUE COMPRA".into(), ..Default::default() };
            crate::pessoas::garantir_pela_nota(c, d, &u, &em5, false, "teste").unwrap();
            let peca = |nome: &str, barras: &str, codigo: &str| {
                let mut f = produtos::ficha_nova(c).unwrap();
                f.nome = nome.into();
                f.codigo_barras = barras.into();
                if !codigo.is_empty() {
                    f.codigo = codigo.into();
                }
                f.precos[0].preco = "10,00".into();
                produtos::salvar(c, d, &u, &f).unwrap()
            };
            peca("PECA P1", &g1, "");
            let p2 = peca("PECA P2", &g2, "");
            let p3 = peca("PECA P3 DESATIVADA", &g3, "");
            peca("PECA MESMA DESCRICAO", "", "");
            peca("PECA DA LOJA ANTIGA", "", "LJ1");
            c.execute("UPDATE produto SET ativo = 0 WHERE id = ?1", [&p3]).unwrap();
            for (cod, pid) in [("B1", &p2), ("B2", &p2)] {
                crate::catalogo::codigos::vincular_fornecedor(
                    c,
                    &fid,
                    cod,
                    pid,
                    1000,
                    crate::catalogo::codigos::JaLigado::Manter,
                )
                .unwrap();
            }
            let conta = |sql: &str| -> i64 { c.query_row(sql, [], |l| l.get(0)).unwrap() };
            let codigos_de = |pid: &str| -> std::collections::BTreeSet<String> {
                let mut s: std::collections::BTreeSet<String> = c
                    .prepare("SELECT gtin FROM produto_gtin WHERE produto_id = ?1")
                    .unwrap()
                    .query_map([pid], |l| l.get(0))
                    .unwrap()
                    .map(|x| x.unwrap())
                    .collect();
                let barras: String =
                    c.query_row("SELECT codigo_barras FROM produto WHERE id = ?1", [pid], |l| l.get(0)).unwrap();
                if !barras.is_empty() {
                    s.insert(barras);
                }
                s
            };
            let existentes: Vec<String> = c
                .prepare("SELECT id FROM produto")
                .unwrap()
                .query_map([], |l| l.get(0))
                .unwrap()
                .map(|x| x.unwrap())
                .collect();
            let antes_codigos: Vec<_> = existentes.iter().map(|p| codigos_de(p)).collect();
            let vinculos_de = |pid: &str| -> i64 {
                c.query_row("SELECT COUNT(*) FROM produto_fornecedor WHERE produto_id = ?1", [pid], |l| l.get(0))
                    .unwrap()
            };
            let antes_vinculos: Vec<i64> = existentes.iter().map(|p| vinculos_de(p)).collect();
            let antes = (
                conta("SELECT COUNT(*) FROM produto"),
                conta("SELECT COUNT(*) FROM produto_fornecedor"),
                conta("SELECT COUNT(*) FROM pessoa WHERE cliente = 1"),
                conta("SELECT COUNT(*) FROM pessoa WHERE fornecedor = 1"),
                conta("SELECT COUNT(*) FROM xml_pendencia_identificacao WHERE situacao = 'pendente'"),
            );
            c.pragma_update(None, "query_only", true).unwrap();
            let p = previa_do_lido(c, &e, &lido);
            c.pragma_update(None, "query_only", false).unwrap();
            // FI-1 (a): sem "Fornecedores", "Peças do fornecedor" é recusada, na
            // prévia e na importação, com a mesma mensagem, sem gravar nada.
            if e.pecas_fornecedor && !e.fornecedores {
                let ep = p.err().map(|x| x.to_string()).unwrap_or_default();
                let ei = gravar(c, d, &u, &e, &lido).err().map(|x| x.to_string()).unwrap_or_default();
                let nada = antes
                    == (
                        conta("SELECT COUNT(*) FROM produto"),
                        conta("SELECT COUNT(*) FROM produto_fornecedor"),
                        conta("SELECT COUNT(*) FROM pessoa WHERE cliente = 1"),
                        conta("SELECT COUNT(*) FROM pessoa WHERE fornecedor = 1"),
                        conta("SELECT COUNT(*) FROM xml_pendencia_identificacao WHERE situacao = 'pendente'"),
                    );
                if !ep.contains("Fornecedores") || ep != ei || !nada {
                    erros.push(format!("{n:04b}: FI-1: prévia {ep:?}, importação {ei:?}, nada gravado: {nada}"));
                }
                continue;
            }
            let p = p.unwrap_or_else(|x| panic!("a prévia falhou (gravou?) com {n:04b}: {x}"));
            // M1 da revisão R1 (B1 da R2): quem passa de papel é contado com todas as opções ligadas, em toda
            // combinação: f5 vira cliente; f3 e f4 viram fornecedores. A caixa de marcar da tela depende disso.
            if (p.clientes_viram, p.fornecedores_viram) != (1, 2) {
                erros.push(format!(
                    "{n:04b}: viram cliente {}, viram fornecedor {} (esperado 1 e 2, com qualquer opção)",
                    p.clientes_viram, p.fornecedores_viram
                ));
            }
            let r = gravar(c, d, &u, &e, &lido).unwrap();
            let cat =
                |codigo: &str| p.resumo.categorias.iter().find(|x| x.codigo == codigo).map_or(0, |x| x.quantidade);
            let depois = (
                conta("SELECT COUNT(*) FROM produto"),
                conta("SELECT COUNT(*) FROM produto_fornecedor"),
                conta("SELECT COUNT(*) FROM pessoa WHERE cliente = 1"),
                conta("SELECT COUNT(*) FROM pessoa WHERE fornecedor = 1"),
                conta("SELECT COUNT(*) FROM xml_pendencia_identificacao WHERE situacao = 'pendente'"),
            );
            let abertas: i64 = p.resumo.pendencias_por_motivo.iter().map(|m| m.ja_abertas).sum();
            let anunciado = (
                cat("loja.nova") + cat("forn.nova") + cat("forn.nova_codigo_repetido"),
                cat("forn.nova") + cat("forn.ja_pelo_gtin_liga"),
                cat("cliente.novo") + cat("cliente.ja_sem_papel"),
                cat("fornecedor.novo") + cat("fornecedor.ja_sem_papel"),
                p.resumo.vao_para_pendencias - abertas,
            );
            let feito =
                (depois.0 - antes.0, depois.1 - antes.1, depois.2 - antes.2, depois.3 - antes.3, depois.4 - antes.4);
            if anunciado != feito {
                erros.push(format!("{n:04b}: anunciado (peças, vínculos, clientes, fornecedores, pendências) {anunciado:?}, feito {feito:?}"));
            }
            if p.resumo != r.resumo {
                erros.push(format!("{n:04b}: o resumo da prévia e o da importação diferem"));
            }
            let mut ganharam_gtin = std::collections::BTreeSet::new();
            let mut mudaram = std::collections::BTreeSet::new();
            for (i, pid) in existentes.iter().enumerate() {
                if codigos_de(pid).difference(&antes_codigos[i]).next().is_some() {
                    ganharam_gtin.insert(pid.clone());
                    mudaram.insert(pid.clone());
                }
                if vinculos_de(pid) > antes_vinculos[i] {
                    mudaram.insert(pid.clone());
                }
            }
            let auditados: std::collections::BTreeSet<String> = c
                .prepare("SELECT entidade_id FROM auditoria WHERE acao = 'produto.gtin_importado'")
                .unwrap()
                .query_map([], |l| l.get(0))
                .unwrap()
                .map(|x| x.unwrap())
                .collect();
            let linhas = conta("SELECT COUNT(*) FROM auditoria WHERE acao = 'produto.gtin_importado'");
            if auditados != ganharam_gtin || linhas != ganharam_gtin.len() as i64 {
                erros.push(format!("{n:04b}: auditoria do GTIN {auditados:?} ({linhas} linhas), peças que ganharam GTIN {ganharam_gtin:?}"));
            }
            if p.resumo.pecas_que_mudam != mudaram.len() as i64 {
                erros.push(format!(
                    "{n:04b}: peças que mudam: anunciado {}, no banco {}",
                    p.resumo.pecas_que_mudam,
                    mudaram.len()
                ));
            }
            // O plano: o K2 acha pelo GTIN a peça que o K1 cria na MESMA importação
            // (como na gravação item a item de antes) e o código K2 é ligado a ela.
            if e.pecas_fornecedor {
                let k = conta("SELECT COUNT(*) FROM produto WHERE nome = 'PECA K'");
                let k2 = conta("SELECT COUNT(*) FROM produto_fornecedor f JOIN produto p ON p.id = f.produto_id WHERE f.codigo = 'K2' AND p.nome = 'PECA K'");
                if (k, k2) != (1, 1) {
                    erros.push(format!(
                        "{n:04b}: o K2 não achou a peça do K1 pelo plano: {k} peças PECA K, {k2} vínculo do K2"
                    ));
                }
            }
            // N06 (a): a peça desativada achada pelo GTIN não ganha nada e vira
            // pendência própria, com ela como candidata, em toda combinação.
            if p.resumo.pecas_desativadas != 1 {
                erros.push(format!("{n:04b}: a peça desativada achada pelo GTIN não foi anunciada"));
            }
            let i3 = existentes.iter().position(|x| *x == p3).unwrap();
            let pend3 = conta(&format!(
                "SELECT COUNT(*) FROM xml_pendencia_identificacao WHERE situacao = 'pendente' AND motivo = 'peca_desativada' AND candidatos_json LIKE '%{p3}%'"
            ));
            if codigos_de(&p3) != antes_codigos[i3] || vinculos_de(&p3) != antes_vinculos[i3] || pend3 != 1 {
                erros
                    .push(format!("{n:04b}: N06: a peça desativada mudou ou ficou sem pendência ({pend3} pendências)"));
            }
            // FI-2 (a): com "Fornecedores", quem já era só cliente (f3) passa a
            // fornecedor, com a auditoria da compra; o f4, cadastrado como
            // cliente nesta importação, também.
            let e_forn =
                |doc: &str| conta(&format!("SELECT COUNT(*) FROM pessoa WHERE documento = '{doc}' AND fornecedor = 1"));
            let passou = conta("SELECT COUNT(*) FROM auditoria WHERE acao = 'pessoa.alterar' AND detalhes LIKE '%Passou a ser fornecedor%'");
            let esperado_f4 = e.fornecedores as i64;
            if e_forn(&f3) != e.fornecedores as i64 || e_forn(&f4) != esperado_f4 {
                erros.push(format!("{n:04b}: FI-2: f3 fornecedor {}, f4 fornecedor {}", e_forn(&f3), e_forn(&f4)));
            }
            let esperado_passou = e.fornecedores as i64 * (1 + e.clientes as i64);
            if passou != esperado_passou {
                erros.push(format!(
                    "{n:04b}: FI-2: {passou} \"Passou a ser fornecedor\" na auditoria, esperado {esperado_passou}"
                ));
            }
            // FI-3 (a), R17: com "Clientes", quem já era só fornecedor (f5) passa a cliente, anunciado na prévia, com o
            // registro de quando e por quê; sem "Clientes", nada muda.
            let e_cli =
                |doc: &str| conta(&format!("SELECT COUNT(*) FROM pessoa WHERE documento = '{doc}' AND cliente = 1"));
            let virou = conta(
                "SELECT COUNT(*) FROM auditoria WHERE acao = 'pessoa.alterar' AND detalhes LIKE '%Passou a ser cliente%' \
                 AND detalhes LIKE '%Apareceu como cliente numa nota da loja (Trazer cadastros)%'",
            );
            let esperado = e.clientes as i64;
            if (e_cli(&f5), virou, cat("cliente.ja_sem_papel")) != (esperado, esperado, esperado) {
                erros.push(format!(
                    "{n:04b}: FI-3: f5 cliente {}, registro {virou}, anunciado {}, esperado {esperado}",
                    e_cli(&f5),
                    cat("cliente.ja_sem_papel")
                ));
            }
        }
        assert!(erros.is_empty(), "{}", erros.join("\n"));
    }

    /// N06 (a), decisão do proprietário de 12/11: "Trazer cadastros" nunca usa
    /// uma peça desativada como destino. Achada pelo GTIN, pelo vínculo do
    /// fornecedor ou pela descrição (código repetido), ela não ganha vínculo
    /// nem GTIN, e nenhuma pendência fecha nela; o item vira pendência
    /// `peca_desativada`, com ela como candidata. Resolver segue o A07: ligar à
    /// desativada é recusado; reativada, liga. A peça ativa não muda nada.
    #[test]
    fn n06_importacao_nao_usa_peca_desativada() {
        const FORN: &str = "11222333000181";
        let g1 = ean13(789_200_000_001);
        let g2 = ean13(789_200_000_002);
        let gtin =
            |g: &str| vec![nfe_xml::GtinXml { codigo: g.into(), tipo: "comercial".into(), origem: "cEAN".into() }];
        let item = |codigo: &str, descricao: &str, gtins: Vec<nfe_xml::GtinXml>| ItemXml {
            codigo: codigo.into(),
            descricao: descricao.into(),
            gtins,
            quantidade: 1000,
            valor_produtos: 5000,
            ..Default::default()
        };
        for ativa in [false, true] {
            let b = Banco::em_memoria().unwrap();
            let (c, d) = (&b.conn, b.dispositivo.as_str());
            let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
            crate::faixas::comecar_loja(c).unwrap();
            config::gravar(c, "loja_cnpj", "11444777000161").unwrap();
            let em = Emitente { documento: FORN.into(), nome: "FORNECEDOR".into(), ..Default::default() };
            let (fid, _) = crate::pessoas::garantir_pela_nota(c, d, &u, &em, false, "teste").unwrap();
            let peca = |nome: &str, barras: &str| {
                let mut f = produtos::ficha_nova(c).unwrap();
                f.nome = nome.into();
                f.codigo_barras = barras.into();
                f.precos[0].preco = "10,00".into();
                produtos::salvar(c, d, &u, &f).unwrap()
            };
            let pelo_gtin = peca("PECA PELO GTIN", &g1);
            let pelo_vinculo = peca("PECA PELO VINCULO", "");
            let pela_descricao = peca("PECA REPETIDA", "");
            crate::catalogo::codigos::vincular_fornecedor(
                c,
                &fid,
                "V1",
                &pelo_vinculo,
                1000,
                crate::catalogo::codigos::JaLigado::Manter,
            )
            .unwrap();
            if !ativa {
                c.execute("UPDATE produto SET ativo = 0", []).unwrap();
            }
            let mut lido = Lido::default();
            lido.fornecedores.insert(FORN.into(), (em.clone(), "2026-11-01".into()));
            for it in [
                item("G1", "PECA PELO GTIN", gtin(&g1)),
                item("V1", "PECA PELO VINCULO", gtin(&g2)),
                item("R1", "PECA REPETIDA", Vec::new()),
                item("R1", "OUTRA COISA R1", Vec::new()),
            ] {
                lido.pecas_fornecedor
                    .insert((FORN.into(), it.codigo.clone(), descricao_chave(&it)), (it, "2026-11-01".into()));
            }
            let conta = |sql: &str| -> i64 { c.query_row(sql, [], |l| l.get(0)).unwrap() };
            let gtins_de = |pid: &str| {
                conta(&format!("SELECT COUNT(*) FROM produto_gtin WHERE produto_id = '{pid}'"))
                    + conta(&format!("SELECT COUNT(*) FROM produto WHERE id = '{pid}' AND codigo_barras <> ''"))
            };
            let antes: Vec<i64> = [&pelo_gtin, &pelo_vinculo, &pela_descricao].iter().map(|p| gtins_de(p)).collect();
            let e = Entrada { fornecedores: true, pecas_fornecedor: true, ..Default::default() };
            let p = previa_do_lido(c, &e, &lido).unwrap();
            gravar(c, d, &u, &e, &lido).unwrap();
            let vinculos_de =
                |pid: &str| conta(&format!("SELECT COUNT(*) FROM produto_fornecedor WHERE produto_id = '{pid}'"));
            let pendencia_de = |pid: &str| {
                conta(&format!(
                    "SELECT COUNT(*) FROM xml_pendencia_identificacao WHERE situacao = 'pendente'
                      AND motivo = 'peca_desativada' AND candidatos_json LIKE '%{pid}%'"
                ))
            };
            if ativa {
                // Como antes: o GTIN liga o G1, o V1 ganha o GTIN da nota, o R1
                // repetido entra na peça de mesma descrição.
                assert_eq!(vinculos_de(&pelo_gtin), 1, "o G1 liga pelo GTIN");
                assert!(gtins_de(&pelo_vinculo) > antes[1], "o V1 ganha o GTIN");
                assert_eq!(
                    conta("SELECT COUNT(*) FROM xml_pendencia_identificacao WHERE motivo = 'peca_desativada'"),
                    0
                );
                assert_eq!(p.resumo.pecas_desativadas, 0);
                continue;
            }
            assert_eq!(vinculos_de(&pelo_gtin), 0, "o G1 não liga na desativada");
            assert_eq!(gtins_de(&pelo_gtin), antes[0], "o G1 não grava GTIN na desativada");
            assert_eq!(gtins_de(&pelo_vinculo), antes[1], "o V1 não grava GTIN na desativada");
            assert_eq!(vinculos_de(&pelo_vinculo), 1, "o vínculo de antes fica como está");
            assert_eq!(gtins_de(&pela_descricao), antes[2]);
            for (nome, pid) in [("G1", &pelo_gtin), ("V1", &pelo_vinculo), ("R1", &pela_descricao)] {
                assert_eq!(pendencia_de(pid), 1, "{nome}: a pendência peca_desativada com a peça candidata");
            }
            assert_eq!(p.resumo.pecas_desativadas, 3, "a prévia anuncia as 3");
            let motivo = p.resumo.pendencias_por_motivo.iter().find(|m| m.motivo == "peca_desativada").unwrap();
            assert_eq!(motivo.quantidade, 3);
            assert!(motivo.texto.contains("desativada"), "{}", motivo.texto);

            // Resolver segue o A07: ligar à desativada é recusado; reativada, liga.
            let pend: String = c
                .query_row(
                    "SELECT id FROM xml_pendencia_identificacao WHERE codigo_fornecedor = 'G1' AND situacao = 'pendente'",
                    [],
                    |l| l.get(0),
                )
                .unwrap();
            let pedido = ResolverPendencia {
                id: pend.clone(),
                acao: "ligar".into(),
                produto_id: pelo_gtin.clone(),
                ..Default::default()
            };
            assert!(resolver_pendencia(c, d, &u, &pedido).is_err(), "ligar à desativada é recusado");
            c.execute("UPDATE produto SET ativo = 1 WHERE id = ?1", [&pelo_gtin]).unwrap();
            resolver_pendencia(c, d, &u, &pedido).unwrap();
            assert_eq!(vinculos_de(&pelo_gtin), 1, "reativada, liga");
        }
    }

    /// N06-c (c), R16 (decisão do proprietário de 25/11/2026, regra 16: "deixar
    /// como está"): nas notas da **própria loja**, "Trazer cadastros" continua
    /// gravando o GTIN da nota na peça desativada achada pelo código (é o código
    /// da própria venda). Não liga nada, não mexe no estoque, não reativa a peça e
    /// não abre pendência; a prévia anuncia a peça que ganha GTIN.
    #[test]
    fn r16_n06c_nota_da_loja_grava_o_gtin_na_peca_desativada() {
        let g = ean13(789_300_000_001);
        let b = Banco::em_memoria().unwrap();
        let (c, d) = (&b.conn, b.dispositivo.as_str());
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        crate::faixas::comecar_loja(c).unwrap();
        config::gravar(c, "loja_cnpj", "11444777000161").unwrap();
        let mut f = produtos::ficha_nova(c).unwrap();
        f.nome = "PECA DA LOJA DESATIVADA".into();
        f.codigo = "LJ9".into();
        f.estoque_inicial = "3".into();
        f.precos[0].preco = "10,00".into();
        let pid = produtos::salvar(c, d, &u, &f).unwrap();
        c.execute("UPDATE produto SET ativo = 0 WHERE id = ?1", [&pid]).unwrap();
        let item = ItemXml {
            codigo: "LJ9".into(),
            descricao: "PECA DA LOJA DESATIVADA".into(),
            gtins: vec![nfe_xml::GtinXml { codigo: g.clone(), tipo: "comercial".into(), origem: "cEAN".into() }],
            quantidade: 1000,
            valor_produtos: 5000,
            ..Default::default()
        };
        let mut lido = Lido::default();
        lido.pecas_loja.insert((item.codigo.clone(), descricao_chave(&item)), (item, "2026-11-01".into()));
        let conta = |sql: &str| -> i64 { c.query_row(sql, [], |l| l.get(0)).unwrap() };
        let estoque_antes = crate::estoque::saldo(c, &pid).unwrap();
        let e = Entrada { pecas_loja: true, ..Default::default() };
        let p = previa_do_lido(c, &e, &lido).unwrap();
        let cat = |codigo: &str| p.resumo.categorias.iter().find(|x| x.codigo == codigo).map_or(0, |x| x.quantidade);
        assert_eq!(cat("loja.ja_gtin_novo"), 1, "a prévia anuncia o GTIN novo: {:?}", p.resumo.categorias);
        gravar(c, d, &u, &e, &lido).unwrap();
        assert_eq!(
            conta(&format!("SELECT COUNT(*) FROM produto_gtin WHERE produto_id = '{pid}' AND gtin = '{g}'")),
            1,
            "o GTIN da nota da loja fica gravado na peça desativada"
        );
        assert_eq!(conta(&format!("SELECT ativo FROM produto WHERE id = '{pid}'")), 0, "a peça continua desativada");
        assert_eq!(crate::estoque::saldo(c, &pid).unwrap(), estoque_antes, "o estoque não muda");
        assert_eq!(conta("SELECT COUNT(*) FROM produto_fornecedor"), 0, "nada é ligado");
        assert_eq!(conta("SELECT COUNT(*) FROM xml_pendencia_identificacao"), 0, "nenhuma pendência");
    }

    /// FI-1 (a), decisão do proprietário de 12/11: as 16 combinações das opções,
    /// num lote com um cliente novo, uma peça nova da loja e um fornecedor novo
    /// com uma peça nova. "Peças do fornecedor" sem "Fornecedores" é recusada
    /// (antes cadastrava o fornecedor em silêncio); nas outras, o fornecedor só
    /// entra com "Fornecedores".
    #[test]
    fn fi1_as_16_combinacoes_das_opcoes() {
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
            let gravou = gravar(c, d, &u, &e, &lido);
            let conta = |sql: &str| -> i64 { c.query_row(sql, [], |l| l.get(0)).unwrap() };
            if pf && !fo {
                let nada = conta("SELECT COUNT(*) FROM pessoa") + conta("SELECT COUNT(*) FROM produto") == 0;
                if !matches!(&gravou, Err(x) if x.to_string().contains("Fornecedores")) || !nada {
                    erros.push(format!(
                        "opções c{} f0 l{} p1: não foi recusada ({gravou:?}) ou gravou",
                        cl as u8, lj as u8
                    ));
                }
                continue;
            }
            gravou.unwrap();
            let obtido = (
                conta("SELECT COUNT(*) FROM pessoa WHERE cliente = 1"),
                conta("SELECT COUNT(*) FROM pessoa WHERE fornecedor = 1"),
                conta("SELECT COUNT(*) FROM produto"),
                conta("SELECT COUNT(*) FROM produto_fornecedor"),
                conta("SELECT COUNT(*) FROM xml_pendencia_identificacao WHERE situacao = 'pendente' AND motivo = 'sem_vinculo'"),
            );
            let esperado = (
                cl as i64,
                fo as i64, // FI-1 (a): só com "Fornecedores" (e "Peças do fornecedor" exige a opção)
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

    /// T086 (AUD-X-2 da auditoria da 0.9.18-dev.3.3.3): a prévia conta todos os arquivos ignorados, mas mostra o
    /// motivo só dos 5 primeiros, sem dizer que há mais (com 16 ignorados, 11 ficavam sem nenhuma menção). Agora
    /// uma linha diz quantos mais foram ignorados sem o motivo listado.
    #[test]
    fn previa_diz_quantos_ignorados_ficaram_sem_motivo_listado() {
        let b = Banco::em_memoria().unwrap();
        let c = &b.conn;
        config::gravar(c, "loja_cnpj", "11444777000161").unwrap();
        let pasta = std::env::temp_dir().join(format!("rustinho-importar-t086-{}", tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        for i in 0..8 {
            std::fs::write(pasta.join(format!("nada{i}.xml")), "<nada/>").unwrap();
        }
        let p = previa(c, &[pasta.display().to_string()]).unwrap();
        assert_eq!(p.ignorados, 8);
        assert!(
            p.avisos.iter().any(|a| a.contains("E mais 3 arquivos ignorados")),
            "nada diz que há mais ignorados do que os motivos listados: {:?}",
            p.avisos
        );
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// T100 (N4 da 3ª conferência, cobertura da T087): no Trazer cadastros, o aviso das subpastas fundas demais
    /// vem primeiro e não conta como arquivo ignorado.
    #[test]
    fn previa_avisa_primeiro_das_subpastas_fundas() {
        let b = Banco::em_memoria().unwrap();
        let c = &b.conn;
        config::gravar(c, "loja_cnpj", "11444777000161").unwrap();
        let pasta = std::env::temp_dir().join(format!("rustinho-importar-t100-{}", tempo::novo_id()));
        let fundo = pasta.join("a/b/c/d/e/f/g/h");
        std::fs::create_dir_all(&fundo).unwrap();
        std::fs::write(fundo.join("nota.xml"), NOTA_LOJA).unwrap();
        for i in 0..6 {
            std::fs::write(pasta.join(format!("nada{i}.xml")), "<nada/>").unwrap();
        }
        let p = previa(c, &[pasta.display().to_string()]).unwrap();
        assert!(p.avisos.first().is_some_and(|a| a.contains("não foram olhados")), "{:?}", p.avisos);
        assert_eq!(p.ignorados, 6, "o aviso das subpastas contou como arquivo ignorado");
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// T101 (N5 da 3ª conferência, sobre a T086): os eventos e CT-e (que não são notas) são contados à parte, com o
    /// motivo; a linha "sem o motivo listado" fica só para os outros (antes, um evento comum já dizia que havia um
    /// arquivo ignorado sem motivo).
    #[test]
    fn previa_conta_eventos_e_cte_a_parte() {
        let b = Banco::em_memoria().unwrap();
        let c = &b.conn;
        config::gravar(c, "loja_cnpj", "11444777000161").unwrap();
        let pasta = std::env::temp_dir().join(format!("rustinho-importar-t101-{}", tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        std::fs::write(pasta.join("venda.xml"), NOTA_LOJA).unwrap();
        std::fs::write(pasta.join("evento.xml"), "<procEventoNFe><evento/></procEventoNFe>").unwrap();
        let p = previa(c, &[pasta.display().to_string()]).unwrap();
        assert!(!p.avisos.iter().any(|a| a.contains("sem o motivo listado")), "{:?}", p.avisos);
        assert!(p.avisos.iter().any(|a| a.contains("evento, um CT-e")), "o evento não foi dito: {:?}", p.avisos);
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// T104 (N11 da 4ª conferência, sobre a T101): o resumo de NF-e (o resNFe, comum quando se baixa da SEFAZ sem a
    /// ciência) entra na conta dos eventos e CT-e; o aviso tem de dizer que pode ser um resumo, e que dele se baixa o
    /// XML completo (antes dizia "era um evento ou CT-e, não é uma nota", falso para o resumo).
    #[test]
    fn previa_fala_do_resumo_de_nfe() {
        let b = Banco::em_memoria().unwrap();
        let c = &b.conn;
        config::gravar(c, "loja_cnpj", "11444777000161").unwrap();
        let pasta = std::env::temp_dir().join(format!("rustinho-importar-t104-{}", tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        std::fs::write(pasta.join("venda.xml"), NOTA_LOJA).unwrap();
        std::fs::write(pasta.join("resumo.xml"), "<resNFe><chNFe>1</chNFe></resNFe>").unwrap();
        let p = previa(c, &[pasta.display().to_string()]).unwrap();
        assert!(
            p.avisos.iter().any(|a| a.contains("resumo") && a.contains("baixe o XML completo")),
            "o aviso não fala do resumo: {:?}",
            p.avisos
        );
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// T080 (OPUS-C-2 = AUD-C-5 da auditoria da 0.9.18-dev.3.3.3): o arquivo que nem chegou a ser lido (um .zip
    /// estragado, pacote grande demais, lote cortado) aparecia como "O arquivo não é um XML válido", e o motivo
    /// verdadeiro sumia. Agora o aviso traz o motivo de quem leu os arquivos.
    #[test]
    fn previa_mostra_o_motivo_verdadeiro_do_arquivo_nao_lido() {
        let b = Banco::em_memoria().unwrap();
        let c = &b.conn;
        config::gravar(c, "loja_cnpj", "11444777000161").unwrap();
        let pasta = std::env::temp_dir().join(format!("rustinho-importar-t080-{}", tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        std::fs::write(pasta.join("venda.xml"), NOTA_LOJA).unwrap();
        std::fs::write(pasta.join("estragado.zip"), b"isto nao e um zip de verdade").unwrap();
        let p = previa(c, &[pasta.display().to_string()]).unwrap();
        let aviso = p.avisos.iter().find(|a| a.contains("estragado.zip")).cloned().unwrap_or_default();
        assert!(aviso.contains("estragado ou não é zip"), "o motivo verdadeiro sumiu: {:?}", p.avisos);
        assert!(!aviso.contains("não é um XML válido"), "{aviso}");
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// T074 (OPUS-C-1 da auditoria da 0.9.18-dev.3.3.3): num caixa em que a faixa de códigos recusa o código
    /// numérico da nota (aqui, uma geração nova, como depois de restaurar um backup), a peça nascia sem o código
    /// do sistema antigo (nem como código antigo) e cada reimportação das mesmas notas criava outra peça.
    #[test]
    fn codigo_recusado_pela_faixa_vira_codigo_antigo_e_nao_duplica() {
        let b = Banco::em_memoria().unwrap();
        let (c, d) = (&b.conn, b.dispositivo.as_str());
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        crate::faixas::comecar_loja(c).unwrap();
        crate::faixas::nova_geracao(c, "como depois de restaurar um backup").unwrap();
        config::gravar(c, "loja_cnpj", "11444777000161").unwrap();
        let pasta = std::env::temp_dir().join(format!("rustinho-importar-t074-{}", tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        std::fs::write(pasta.join("venda.xml"), NOTA_LOJA).unwrap();
        let e = Entrada { arquivos: vec![pasta.display().to_string()], pecas_loja: true, ..Default::default() };
        importar(c, d, &u, &e).unwrap();
        importar(c, d, &u, &e).unwrap();
        let ids: Vec<String> = c
            .prepare("SELECT id FROM produto WHERE nome = 'FILTRO DE OLEO PSL55'")
            .unwrap()
            .query_map([], |l| l.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(ids.len(), 1, "a reimportação das mesmas notas duplicou a peça");
        let (codigo, antigo): (String, i64) = c
            .query_row(
                "SELECT codigo, (SELECT COUNT(*) FROM produto_codigo_antigo WHERE produto_id = ?1 AND codigo = '4455')
                   FROM produto WHERE id = ?1",
                [&ids[0]],
                |l| Ok((l.get(0)?, l.get(1)?)),
            )
            .unwrap();
        assert!(codigo == "4455" || antigo == 1, "o código 4455 do sistema antigo se perdeu (código {codigo:?})");
        let _ = std::fs::remove_dir_all(&pasta);
    }

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
            &ResolverPendencia {
                id: p0.id.clone(),
                acao: "nova".into(),
                produto_id: String::new(),
                ..Default::default()
            },
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
            &ResolverPendencia {
                id: p1.id.clone(),
                acao: "ligar".into(),
                produto_id: pas_id.clone(),
                ..Default::default()
            },
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
