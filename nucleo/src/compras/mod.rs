//! Compras: entrada de mercadoria pelo XML da NF-e do fornecedor.
//!
//! Caminho:
//! 1. `importar_xml` lê a nota, cadastra o fornecedor (se for novo) e cria um
//!    rascunho. Cada item já tenta se ligar sozinho a uma peça: pelo código
//!    que o fornecedor usou da última vez, pelo código de barras ou pelo
//!    código original/equivalente.
//! 2. Na tela, cada item fica "ligado" a uma peça, "nova" (cadastrar) ou
//!    "ignorar" (não entra no estoque). Dá para informar o frete pago à parte.
//! 3. `lancar` põe tudo no estoque, atualiza o custo (e o preço, se pedido)
//!    e guarda o código do fornecedor para a próxima nota.
//! 4. `estornar` desfaz a entrada no estoque (o custo não volta).
//!
//! Custo de cada peça = (valor dos produtos − desconto + frete + seguro +
//! outras despesas + IPI + ICMS-ST + parte do frete pago à parte) ÷
//! quantidade de peças. O Simples Nacional não aproveita crédito de IPI nem
//! de ICMS-ST, por isso eles entram no custo.

pub mod importar;
pub mod nfe_xml;
pub mod pendencias;
pub mod reposicao;

use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::erro::{Erro, Resultado};
use crate::nfe_xml::{ItemXml, NotaXml};
use crate::{auditoria, config, documento, estoque, fiscal, gtin, numeros, pessoas, produtos, tempo, texto};

// ---------------------------------------------------------------------------
// Importar.
// ---------------------------------------------------------------------------

/// Lê o XML e cria a compra em rascunho. Devolve o id da compra.
/// Aviso quando a nota de compra não é da loja (sem bloquear). Sem os
/// números do CNPJ, para não aparecer na tela com o olho fechado.
pub fn aviso_cnpj(conn: &Connection, destinatario: &str) -> Resultado<String> {
    let loja_cnpj = config::ler(conn, "loja_cnpj")?.unwrap_or_default();
    let destinatario = documento::limpar(destinatario);
    if destinatario.is_empty() {
        return Ok(String::new());
    }
    if loja_cnpj.is_empty() {
        return Ok(
            "Preencha o CNPJ da loja em Configurações para o Rustinho conferir se a nota é mesmo da loja.".into()
        );
    }
    if destinatario == loja_cnpj {
        return Ok(String::new());
    }
    let loja = config::nome_loja(conn)?;
    let loja = if loja.is_empty() { "da loja".to_string() } else { format!("da {loja}") };
    Ok(format!("Esta nota não é {loja}: foi emitida para outro CNPJ. Confira antes de lançar."))
}

/// A nota serve para entrada de compra? (Autorizada pela SEFAZ, de saída do
/// fornecedor, normal, e para o CNPJ da loja quando ele está preenchido.)
pub fn conferir_nota(conn: &Connection, nota: &NotaXml) -> Resultado<()> {
    if !nota.autorizada {
        return Err(Erro::validacao(
            "Este XML não tem a autorização da SEFAZ (ou a nota foi negada). Peça ao fornecedor o XML autorizado, o que vem com o protocolo.",
        ));
    }
    if !nota.incoerencia.is_empty() {
        return Err(Erro::validacao(format!(
            "Este XML parece alterado ou montado à mão: {}. Baixe de novo o XML no portal da SEFAZ ou peça ao fornecedor.",
            nota.incoerencia
        )));
    }
    if nota.tipo == "0" {
        return Err(Erro::validacao("Esta é uma nota de entrada, não uma venda do fornecedor para a loja."));
    }
    match nota.finalidade.as_str() {
        "" | "1" => {}
        "4" => return Err(Erro::validacao("Esta é uma nota de devolução, não de compra.")),
        _ => {
            return Err(Erro::validacao(
                "Esta é uma nota complementar ou de ajuste: ela não traz mercadoria para dar entrada.",
            ))
        }
    }
    let loja = documento::limpar(&config::ler(conn, "loja_cnpj")?.unwrap_or_default());
    let destinatario = documento::limpar(&nota.destinatario);
    if !loja.is_empty() && !destinatario.is_empty() && destinatario != loja {
        let nome = config::nome_loja(conn)?;
        let nome = if nome.is_empty() { "da loja".to_string() } else { format!("da {nome}") };
        return Err(Erro::campo(
            CAMPO_OUTRO_CNPJ,
            format!(
                "Esta nota não é {nome}: foi emitida para outro CNPJ. Confira o CNPJ em Configurações ou peça a nota certa ao fornecedor."
            ),
        ));
    }
    Ok(())
}

/// `campo` da recusa de uma nota emitida para outro CNPJ: a tela mostra a
/// janela de conflito com os dois documentos (`conflito_cnpj`). Nada é gravado.
pub const CAMPO_OUTRO_CNPJ: &str = "nota.outro_cnpj";

/// O que a janela de conflito mostra de uma nota emitida para outro CNPJ:
/// os documentos para comparar e a nota para conferir. Só leitura.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflitoCnpj {
    /// O nome do arquivo (dentro de um .zip: "pacote.zip › nota.xml").
    pub arquivo: String,
    /// D-IMG-1: chave de acesso da NF-e (44 dígitos) para deduplicar cópias da mesma nota na tela.
    pub chave: String,
    pub loja_nome: String,
    pub loja_documento: String,
    pub emitente_nome: String,
    pub emitente_documento: String,
    pub destinatario_nome: String,
    pub destinatario_documento: String,
    pub numero: String,
    pub serie: String,
    /// "dd/mm/aaaa"
    pub emissao: String,
    pub total: String,
    pub itens: Vec<ItemConflito>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemConflito {
    pub codigo: String,
    pub descricao: String,
    pub quantidade: String,
    pub unidade: String,
    pub total: String,
}

/// A nota recusada por ser de outro CNPJ, para a janela de conflito. Não
/// muda o cadastro da loja nem grava nada.
pub fn conflito_cnpj(conn: &Connection, arquivo: &str, xml: &str) -> Resultado<ConflitoCnpj> {
    let nota = nfe_xml::ler(xml).map_err(Erro::validacao)?;
    Ok(ConflitoCnpj {
        arquivo: arquivo.to_string(),
        chave: nota.chave.clone(),
        loja_nome: config::nome_loja(conn)?,
        loja_documento: documento::escrever(&config::ler(conn, "loja_cnpj")?.unwrap_or_default()),
        emitente_nome: nota.emitente.nome.clone(),
        emitente_documento: documento::escrever(&nota.emitente.documento),
        destinatario_nome: nota.destinatario_dados.nome.clone(),
        destinatario_documento: documento::escrever(&nota.destinatario),
        numero: nota.numero.clone(),
        serie: nota.serie.clone(),
        emissao: fiscal::data_br(&nota.emissao),
        total: numeros::escrever_dinheiro(nota.total),
        itens: nota
            .itens
            .iter()
            .map(|i| ItemConflito {
                codigo: i.codigo.clone(),
                descricao: i.descricao.clone(),
                quantidade: numeros::escrever_quantidade(i.quantidade),
                unidade: i.unidade.clone(),
                total: numeros::escrever_dinheiro(i.valor_produtos),
            })
            .collect(),
    })
}

/// Importa a NF-e como rascunho de compra. PEND-09: a assinatura do emitente
/// é verificada **antes de qualquer escrita** (nem o fornecedor é cadastrado
/// se a nota for recusada) e a compra nasce com `autorizacao = 'aguardando'`:
/// só pode ser lançada depois de comprovada a autorização da SEFAZ (D7).
pub fn importar_xml(conn: &Connection, dispositivo: &str, usuario_id: &str, xml: &str) -> Resultado<String> {
    let verificada = verificar_para_compra(conn, xml)?;
    if let Some(e) = ja_importada(conn, &verificada.nota)? {
        return Err(e);
    }
    criar_compra(conn, dispositivo, usuario_id, xml, &verificada)
}

/// `campo` da recusa de uma nota que já está em Compras (em rascunho ou lançada). T108 (texto R do proprietário,
/// 21/11/2026): num pacote (pasta, .zip, .rar) ela é contada à parte, como "já estava em Compras", e não é erro.
pub const CAMPO_JA_IMPORTADA: &str = "nota.ja_importada";

/// T108: a nota com a assinatura e a autorização verificadas (PEND-09) e conferida (`conferir_nota`): serve para
/// entrada de compra. Não olha se ela já está em Compras (`ja_importada`). Nada é gravado.
pub(crate) fn verificar_para_compra(conn: &Connection, xml: &str) -> Resultado<nfe_xml::NotaVerificada> {
    let verificada = nfe_xml::ler_verificada(xml, crate::nfe_assinatura::Ancoras::padrao()).map_err(Erro::validacao)?;
    conferir_nota(conn, &verificada.nota)?;
    Ok(verificada)
}

/// T108: a recusa da nota que já está em Compras (não estornada), com quando entrou; `None` se ainda não está.
pub(crate) fn ja_importada(conn: &Connection, nota: &NotaXml) -> Resultado<Option<Erro>> {
    if nota.chave.len() != 44 {
        return Ok(None);
    }
    let ja: Option<(String, String)> = conn
        .query_row(
            "SELECT situacao, criado_em FROM compra WHERE chave = ?1 AND situacao <> 'estornada'",
            [&nota.chave],
            |l| Ok((l.get(0)?, l.get(1)?)),
        )
        .optional()?;
    let Some((situacao, quando)) = ja else { return Ok(None) };
    let como = if situacao == "lancada" { "lançada" } else { "importada (em rascunho)" };
    Ok(Some(Erro::campo(
        CAMPO_JA_IMPORTADA,
        format!("A nota {} deste fornecedor já foi {como} em {}.", nota.numero, fiscal::data_local_br(conn, &quando)?),
    )))
}

/// T108: o que a nota de fornecedor é para Compras, pelas MESMAS regras da importação de Compras
/// (`verificar_para_compra` e `ja_importada`), para a importação de cadastros (Peças). Os filtros baratos vêm antes
/// da verificação completa (a assinatura já foi conferida para o cadastro). Nada é gravado.
pub(crate) fn classificar_para_compras(
    conn: &Connection,
    xml: &str,
    nota: &NotaXml,
) -> Resultado<crate::importar::ParaCompras> {
    use crate::importar::ParaCompras;
    if nota.modelo != "55" || !nota.autorizada {
        return Ok(ParaCompras::NaoServe);
    }
    match conferir_nota(conn, nota) {
        Ok(()) => {}
        Err(Erro::Validacao { .. }) => return Ok(ParaCompras::NaoServe),
        Err(e) => return Err(e),
    }
    if ja_importada(conn, nota)?.is_some() {
        return Ok(ParaCompras::Ja);
    }
    // A9 da revisão da T108: as contas do rateio que o `criar_compra` faz; a nota em que elas não fecham não serve
    // (em vez de derrubar a importação de cadastros inteira).
    if ratear_do_cabecalho(nota, nota.itens.clone()).is_err() {
        return Ok(ParaCompras::NaoServe);
    }
    match verificar_para_compra(conn, xml) {
        Ok(v) => Ok(ParaCompras::Nova(Box::new(v))),
        Err(Erro::Validacao { .. }) => Ok(ParaCompras::NaoServe),
        Err(e) => Err(e),
    }
}

/// T111 (A4 — antiduplicidade em lote): códigos e GTINs que aparecem com
/// descrições diferentes entre notas distintas do mesmo pacote importado.
#[derive(Debug, Default)]
pub(crate) struct AmbiguidadesLote {
    /// (documento limpo do fornecedor, código do fornecedor compactado)
    pub codigos: HashSet<(String, String)>,
    /// GTINs (como em `gtins_para_ficha`) com mais de uma descrição no lote.
    pub gtins: HashSet<String>,
}

/// Pré-analisa (somente leitura) todas as notas válidas de um pacote antes de
/// gravar qualquer compra ou peça, detectando códigos de fornecedor e GTINs
/// usados em descrições diferentes entre notas do mesmo pacote.
pub(crate) fn analisar_lote<'a>(loja_cnpj: &str, xmls: impl Iterator<Item = &'a str>) -> AmbiguidadesLote {
    let loja = documento::limpar(loja_cnpj);
    let mut vistos_cod: HashMap<(String, String), HashSet<String>> = HashMap::new();
    let mut vistos_gtin: HashMap<String, HashSet<String>> = HashMap::new();
    for xml in xmls {
        let Ok(nota) = nfe_xml::ler(xml) else { continue };
        if nota.modelo != "55"
            || !nota.autorizada
            || !nota.incoerencia.is_empty()
            || nota.tipo == "0"
            || !matches!(nota.finalidade.as_str(), "" | "1")
        {
            continue;
        }
        let dest = documento::limpar(&nota.destinatario);
        if !loja.is_empty() && !dest.is_empty() && dest != loja {
            continue;
        }
        let doc = documento::limpar(&nota.emitente.documento);
        for item in &nota.itens {
            let desc = texto::compactar(&item.descricao);
            let comp = texto::compactar(&item.codigo);
            if !doc.is_empty() && !comp.is_empty() {
                vistos_cod.entry((doc.clone(), comp)).or_default().insert(desc.clone());
            }
            for g in crate::importar::gtins_para_ficha(&item.gtins, &item.gtin) {
                if !g.gtin.is_empty() {
                    vistos_gtin.entry(g.gtin).or_default().insert(desc.clone());
                }
            }
        }
    }
    AmbiguidadesLote {
        codigos: vistos_cod.into_iter().filter(|(_, d)| d.len() > 1).map(|(k, _)| k).collect(),
        gtins: vistos_gtin.into_iter().filter(|(_, d)| d.len() > 1).map(|(g, _)| g).collect(),
    }
}

/// Resultado da criação de um rascunho de compra (T111 — Opção C).
pub(crate) struct CompraCriada {
    pub id: String,
    pub numero: String,
    pub pecas_cadastradas: i64,
    pub pendencias_identificacao: i64,
}

/// T108: cria o rascunho da compra da nota já verificada e conferida (`verificar_para_compra`) e que ainda não está
/// em Compras (`ja_importada`). Mantém o modo sem cadastro automático para o `importar_xml` interno e para o gancho
/// de `importar_cadastros` (que já cadastrou as peças antes).
pub(crate) fn criar_compra(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    xml: &str,
    verificada: &nfe_xml::NotaVerificada,
) -> Resultado<String> {
    criar_compra_com_opcoes(conn, dispositivo, usuario_id, xml, verificada, false, &AmbiguidadesLote::default())
        .map(|c| c.id)
}

/// T111 (Decisão D-T108 — Opção C): cria o rascunho da compra; quando
/// `cadastrar_novas == true` (importação pela tela Compras), cada item novo com
/// identidade forte e sem conflito/ambiguidade é cadastrado automaticamente com
/// estoque 0 e vínculo ao fornecedor; identificações duvidosas viram registros
/// provisórios em `xml_pendencia_identificacao` preservando todos os dados do XML.
pub(crate) fn criar_compra_com_opcoes(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    xml: &str,
    verificada: &nfe_xml::NotaVerificada,
    cadastrar_novas: bool,
    lote: &AmbiguidadesLote,
) -> Resultado<CompraCriada> {
    let nota = &verificada.nota;
    let prova = &verificada.prova;
    let (fornecedor_id, _) = pessoas::garantir_fornecedor(conn, dispositivo, usuario_id, &nota.emitente)?;
    let id = tempo::novo_id();
    let NotaXml { itens, .. } = nota.clone();
    let itens = ratear_do_cabecalho(nota, itens)?;

    conn.execute(
        "INSERT INTO compra (id, fornecedor_id, chave, numero, serie, emissao, natureza, total_produtos, frete, seguro,
                             desconto, outras, ipi, icms_st, total, destinatario, xml, criado_em, criado_por, dispositivo,
                             xml_verificacao, xml_prova, autorizacao)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                 'verificada', ?21, 'aguardando')",
        params![
            id, fornecedor_id, nota.chave, nota.numero, nota.serie, nota.emissao, nota.natureza, nota.total_produtos,
            nota.frete, nota.seguro, nota.desconto, nota.outras, nota.ipi, nota.icms_st, nota.total,
            nota.destinatario, xml, tempo::agora(), usuario_id, dispositivo,
            serde_json::json!({
                "titular": prova.titular, "sujeito": prova.sujeito, "tipo": prova.tipo, "algoritmo": prova.algoritmo,
                "cadeia": prova.cadeia, "raiz": prova.raiz, "avisos": prova.avisos,
            }).to_string()
        ],
    )?;
    let ambiguos = codigos_ambiguos(itens.iter().map(|i| (i.codigo.as_str(), i.descricao.as_str())));
    let gtins_amb = gtins_ambiguos(&itens);
    let forn_doc = documento::limpar(&nota.emitente.documento);
    let forn_nome =
        if nota.emitente.fantasia.is_empty() { nota.emitente.nome.as_str() } else { nota.emitente.fantasia.as_str() };
    let mut pecas_cadastradas = 0i64;
    let mut pendencias_identificacao = 0i64;

    for (ordem, item) in itens.iter().enumerate() {
        let compacto = texto::compactar(&item.codigo);
        let gtins_ficha = crate::importar::gtins_para_ficha(&item.gtins, &item.gtin);
        let ambiguo = ambiguos.contains(&compacto)
            || (!forn_doc.is_empty() && lote.codigos.contains(&(forn_doc.clone(), compacto.clone())));
        let gtin_ambiguo = gtins_ficha.iter().any(|g| gtins_amb.contains(&g.gtin) || lote.gtins.contains(&g.gtin));
        let mut lig = ligar_sozinho(conn, &fornecedor_id, item, ambiguo, gtin_ambiguo)?;

        if cadastrar_novas && lig.acao.is_empty() && lig.motivo_pendencia == "sem_vinculo" {
            let tem_gtin_valido = (!item.gtin.is_empty() && gtin::gtin_valido(&item.gtin))
                || gtins_ficha
                    .iter()
                    .any(|g| crate::catalogo::codigos::codigo_do_xml(g).is_some_and(|(c, _, _)| gtin::gtin_valido(&c)));
            if compacto.is_empty() && !tem_gtin_valido {
                lig.motivo_pendencia = "codigo_so_de_simbolos".into();
            } else if !crate::catalogo::codigos::tem_identidade_forte(&gtins_ficha, &item.gtin, &item.codigo) {
                lig.motivo_pendencia = "codigo_fraco_sem_gtin".into();
            } else {
                let mesmas_desc = crate::catalogo::codigos::pecas_por_descricao_compacta(conn, &item.descricao)?;
                if !mesmas_desc.is_empty() {
                    lig.motivo_pendencia = "possivel_duplicata_descricao".into();
                    lig.candidatos = mesmas_desc;
                } else if !pendencia_aberta_do_item(conn, &forn_doc, &compacto, &texto::compactar(&item.descricao))? {
                    let custo_item = numeros::soma([
                        item.valor_produtos,
                        -item.desconto,
                        item.frete,
                        item.seguro,
                        item.outras,
                        item.ipi,
                        item.icms_st,
                    ])?;
                    let custo_inicial = custo_unitario(custo_item, item.quantidade);
                    let pid = cadastrar_peca_da_importacao(
                        conn,
                        dispositivo,
                        usuario_id,
                        &fornecedor_id,
                        forn_nome,
                        &nota.numero,
                        item,
                        &gtins_ficha,
                        custo_inicial,
                    )?;
                    // Achado 8: fecha eventual pendência antiga `sem_vinculo` deste item agora cadastrado.
                    crate::compras::pendencias::resolver_pendencias_do_item(
                        conn,
                        &fornecedor_id,
                        &item.codigo,
                        &item.descricao,
                        &pid,
                    )?;
                    lig = LigacaoAutomatica {
                        acao: "nova".into(),
                        produto_id: Some(pid),
                        achado_por: "importacao".into(),
                        fator: 1000,
                        motivo_pendencia: String::new(),
                        candidatos: Vec::new(),
                    };
                    pecas_cadastradas += 1;
                }
            }
        }

        let gtins_json = serde_json::to_string(&item.gtins).unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            "INSERT INTO compra_item (id, compra_id, ordem, codigo_fornecedor, gtin, gtin_tributavel, origem_gtin,
                                      origem_gtin_tributavel, gtins_json, descricao, ncm, cest, cfop, unidade,
                                      origem, cst_icms, cst_pis, quantidade, valor_produtos, frete, seguro, desconto,
                                      outras, ipi, icms_st, acao, produto_id, achado_por, fator)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21,
                     ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29)",
            params![
                tempo::novo_id(),
                id,
                ordem as i64,
                item.codigo,
                item.gtin,
                item.gtin_tributavel,
                item.origem_comercial,
                item.origem_tributavel,
                gtins_json,
                item.descricao,
                item.ncm,
                item.cest,
                item.cfop,
                item.unidade,
                item.origem,
                item.cst_icms,
                item.cst_pis,
                item.quantidade,
                item.valor_produtos,
                item.frete,
                item.seguro,
                item.desconto,
                item.outras,
                item.ipi,
                item.icms_st,
                lig.acao,
                lig.produto_id,
                lig.achado_por,
                lig.fator
            ],
        )?;
        if lig.acao.is_empty() {
            crate::compras::pendencias::registrar_pendencia_xml(
                conn,
                Some(&fornecedor_id),
                &nota.emitente.documento,
                forn_nome,
                &nota.chave,
                &nota.numero,
                &nota.emissao,
                item,
                crate::importar::valor_unitario(item, true),
                &lig.motivo_pendencia,
                &lig.candidatos,
            )?;
            pendencias_identificacao += 1;
        }
    }
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "compra.importar",
        "compra",
        &id,
        &[format!("NF {} · {} itens · {}", nota.numero, itens.len(), numeros::escrever_dinheiro(nota.total))],
    )?;
    Ok(CompraCriada { id, numero: nota.numero.clone(), pecas_cadastradas, pendencias_identificacao })
}

/// Verifica se já existe registro provisório pendente de ambiguidade/conflito
/// para o mesmo fornecedor, código compactado e descrição compactada.
/// Achado 8: ignora pendência antiga `sem_vinculo` (vinda de Peças sem
/// "Peças do fornecedor"), que na Opção C não trava o cadastro automático.
fn pendencia_aberta_do_item(
    conn: &Connection,
    fornecedor_doc: &str,
    codigo_compacto: &str,
    descricao_compacta: &str,
) -> Resultado<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM xml_pendencia_identificacao
          WHERE situacao = 'pendente'
            AND motivo <> 'sem_vinculo'
            AND fornecedor_documento = ?1
            AND codigo_compacto = ?2
            AND descricao_compacta = ?3)",
        params![fornecedor_doc, codigo_compacto, descricao_compacta],
        |l| l.get(0),
    )?)
}

/// T111 (Opção C — Regra 2): cadastra automaticamente uma peça nova a partir
/// de um item inequívoco da NF-e de compra, preservando todos os dados do XML,
/// estoque 0, vínculo ao fornecedor e auditoria `produto.importar_compra`.
#[allow(clippy::too_many_arguments)]
fn cadastrar_peca_da_importacao(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    fornecedor_id: &str,
    fornecedor_nome: &str,
    numero_nfe: &str,
    item: &ItemXml,
    gtins_ficha: &[produtos::GtinFicha],
    custo: i64,
) -> Resultado<String> {
    let mut f = produtos::ficha_nova(conn)?;
    f.nome = texto::limpar(&item.descricao).chars().take(120).collect();
    if f.nome.is_empty() {
        f.nome = "Peça sem nome".into();
    }
    let barras_livre = !item.gtin.is_empty()
        && gtin::gtin_valido(&item.gtin)
        && matches!(
            crate::importar::peca_por_gtins_inequivoco(conn, &[], &item.gtin)?,
            crate::catalogo::codigos::BuscaId::Nenhum
        );
    if barras_livre {
        f.codigo_barras = item.gtin.clone();
    }
    f.unidade = crate::unidades::de_fora(&item.unidade);
    if item.ncm.len() == 8 && item.ncm.chars().all(|c| c.is_ascii_digit()) {
        f.ncm = item.ncm.clone();
    }
    if item.cest.len() == 7 && item.cest.chars().all(|c| c.is_ascii_digit()) {
        f.cest = item.cest.clone();
    }
    if (0..produtos::ORIGENS.len() as i64).contains(&item.origem) {
        f.origem = item.origem;
    }
    f.monofasico = item.cst_pis == "04";
    if custo > 0 {
        f.custo = numeros::escrever_dinheiro(custo);
        if let Some(p) = f.precos.iter_mut().find(|p| p.tabela == 1) {
            if p.lucro.is_empty() {
                p.lucro = "100".into();
            }
        }
    } else {
        // Achado 1 (Crítico): bonificação/brinde (custo 0) nasce com preço na hora da venda.
        f.preco_aberto = true;
    }
    let pid = produtos::salvar(conn, dispositivo, usuario_id, &f)?;
    produtos::registrar_gtins_xml(conn, &pid, gtins_ficha)?;
    let compacto = texto::compactar(&item.codigo);
    if !fornecedor_id.is_empty() && !compacto.is_empty() {
        crate::catalogo::codigos::vincular_fornecedor(
            conn,
            fornecedor_id,
            item.codigo.trim(),
            &pid,
            1000,
            crate::catalogo::codigos::JaLigado::Manter,
        )?;
        produtos::atualizar_indice_busca(conn, &pid)?;
    }
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "produto.importar_compra",
        "produto",
        &pid,
        &[format!(
            "Cadastrada na importação da NF {} · fornecedor {} · código {}",
            numero_nfe,
            fornecedor_nome,
            if item.codigo.trim().is_empty() { "—" } else { item.codigo.trim() }
        )],
    )?;
    Ok(pid)
}

/// Se a nota trouxe frete/seguro/desconto/outras só no total, divide entre
/// os itens pelo valor de cada um.
fn ratear_do_cabecalho(nota: &NotaXml, mut itens: Vec<ItemXml>) -> Resultado<Vec<ItemXml>> {
    let pesos: Vec<i64> = itens.iter().map(|i| i.valor_produtos.max(0)).collect();
    let mut aplicar = |total: i64, pegar: fn(&mut ItemXml) -> &mut i64| -> Resultado<()> {
        let soma = numeros::soma(itens.iter_mut().map(|i| *pegar(i)))?;
        if soma == 0 && total > 0 {
            for (i, parte) in ratear(total, &pesos).into_iter().enumerate() {
                *pegar(&mut itens[i]) = parte;
            }
        }
        Ok(())
    };
    aplicar(nota.frete, |i| &mut i.frete)?;
    aplicar(nota.seguro, |i| &mut i.seguro)?;
    aplicar(nota.desconto, |i| &mut i.desconto)?;
    aplicar(nota.outras, |i| &mut i.outras)?;
    Ok(itens)
}

/// Divide `total` em partes proporcionais aos pesos, sem perder centavos
/// (as sobras vão para os maiores restos).
pub fn ratear(total: i64, pesos: &[i64]) -> Vec<i64> {
    // Somas em i128: nenhuma quantidade de itens "dá a volta" (R-10).
    let soma: i128 = pesos.iter().map(|p| *p as i128).sum();
    if pesos.is_empty() {
        return vec![];
    }
    if soma <= 0 {
        // sem valores: divide igual
        let n = pesos.len() as i64;
        return (0..n).map(|i| total / n + if i < total % n { 1 } else { 0 }).collect();
    }
    let mut partes: Vec<i64> =
        pesos.iter().map(|p| crate::numeros::saturar(total as i128 * *p as i128 / soma)).collect();
    let mut sobra = total as i128 - partes.iter().map(|p| *p as i128).sum::<i128>();
    let mut restos: Vec<(usize, i128)> =
        pesos.iter().enumerate().map(|(i, p)| (i, (total as i128 * *p as i128) % soma)).collect();
    restos.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (i, _) in restos {
        if sobra <= 0 {
            break;
        }
        partes[i] += 1;
        sobra -= 1;
    }
    partes
}

/// D-13: códigos (compactados) que aparecem na mesma nota em itens com
/// descrições diferentes. Esses não ligam sozinhos pelo código do
/// fornecedor nem gravam a ligação ao lançar: são peças diferentes.
fn codigos_ambiguos<'a>(itens: impl Iterator<Item = (&'a str, &'a str)>) -> HashSet<String> {
    let mut vistos: HashMap<String, HashSet<String>> = HashMap::new();
    for (codigo, descricao) in itens {
        let c = texto::compactar(codigo);
        if !c.is_empty() {
            vistos.entry(c).or_default().insert(texto::compactar(descricao));
        }
    }
    vistos.into_iter().filter(|(_, d)| d.len() > 1).map(|(c, _)| c).collect()
}

fn gtins_ambiguos(itens: &[ItemXml]) -> HashSet<String> {
    let mut vistos: HashMap<String, HashSet<String>> = HashMap::new();
    for item in itens {
        let desc = texto::compactar(&item.descricao);
        for g in crate::importar::gtins_para_ficha(&item.gtins, &item.gtin) {
            if !g.gtin.is_empty() {
                vistos.entry(g.gtin).or_default().insert(desc.clone());
            }
        }
    }
    vistos.into_iter().filter(|(_, d)| d.len() > 1).map(|(g, _)| g).collect()
}

/// N06-b (a): a entrada de compra não liga sozinha uma peça desativada (o
/// estoque entraria numa peça que não aparece na busca nem na reposição). O
/// item vira pendência `peca_desativada`, com a peça como candidata; resolver
/// segue o A07 (reativar a peça ou escolher outra).
fn ligar_se_ativa(conn: &Connection, pid: String, achado_por: &str, fator: i64) -> Resultado<LigacaoAutomatica> {
    let ativa: bool = conn
        .query_row("SELECT ativo FROM produto WHERE id = ?1", [&pid], |l| l.get::<_, i64>(0))
        .optional()?
        .is_some_and(|a| a != 0);
    if !ativa {
        return Ok(LigacaoAutomatica {
            acao: String::new(),
            produto_id: None,
            achado_por: String::new(),
            fator: 1000,
            motivo_pendencia: "peca_desativada".into(),
            candidatos: vec![pid],
        });
    }
    Ok(LigacaoAutomatica {
        acao: "ligar".into(),
        produto_id: Some(pid),
        achado_por: achado_por.into(),
        fator,
        motivo_pendencia: String::new(),
        candidatos: Vec::new(),
    })
}

struct LigacaoAutomatica {
    acao: String,
    produto_id: Option<String>,
    achado_por: String,
    fator: i64,
    motivo_pendencia: String,
    candidatos: Vec<String>,
}

/// Tenta achar a peça do item: GTIN exato e inequívoco e vínculo do fornecedor
/// (sem conflito entre eles) → código original/equivalente.
/// Nunca associa automaticamente quando houver ambiguidade nem confunde `cProd`
/// com código interno.
fn ligar_sozinho(
    conn: &Connection,
    fornecedor_id: &str,
    item: &ItemXml,
    ambiguo: bool,
    gtin_ambiguo: bool,
) -> Resultado<LigacaoAutomatica> {
    use crate::catalogo::codigos::BuscaId;

    let compacto = texto::compactar(&item.codigo);
    let gtin_busca = if gtin_ambiguo {
        BuscaId::Ambiguo(Vec::new())
    } else {
        crate::importar::peca_por_gtins_inequivoco(conn, &item.gtins, &item.gtin)?
    };
    let forn_ligado: Option<(String, i64)> = if !compacto.is_empty() && !ambiguo {
        crate::catalogo::codigos::vinculo_do_fornecedor(conn, fornecedor_id, &compacto)?
    } else {
        None
    };

    if let BuscaId::Ambiguo(cands) = gtin_busca {
        return Ok(LigacaoAutomatica {
            acao: String::new(),
            produto_id: None,
            achado_por: String::new(),
            fator: 1000,
            motivo_pendencia: "gtin_ambiguo".into(),
            candidatos: cands,
        });
    }

    if let (BuscaId::Unico(pid_gtin), Some((pid_forn, _))) = (&gtin_busca, &forn_ligado) {
        if pid_gtin != pid_forn {
            return Ok(LigacaoAutomatica {
                acao: String::new(),
                produto_id: None,
                achado_por: String::new(),
                fator: 1000,
                motivo_pendencia: "conflito_gtin_fornecedor".into(),
                candidatos: vec![pid_gtin.clone(), pid_forn.clone()],
            });
        }
    }

    // Vínculo prévio do mesmo fornecedor (com o seu fator) quando concorda com o GTIN ou quando não há GTIN.
    if let Some((pid, fator)) = forn_ligado {
        return ligar_se_ativa(conn, pid, "fornecedor", fator);
    }

    // GTIN exato e inequívoco antes de qualquer outra forma.
    if let BuscaId::Unico(pid) = gtin_busca {
        return ligar_se_ativa(conn, pid, "barras", 1000);
    }

    if ambiguo {
        return Ok(LigacaoAutomatica {
            acao: String::new(),
            produto_id: None,
            achado_por: String::new(),
            fator: 1000,
            motivo_pendencia: "codigo_repetido".into(),
            candidatos: Vec::new(),
        });
    }

    if compacto.len() >= 3 {
        let mut candidatos_cod = crate::catalogo::codigos::candidatos_por_codigo_original(conn, &compacto)?;
        if candidatos_cod.len() == 1 {
            return ligar_se_ativa(conn, candidatos_cod.remove(0), "codigo", 1000);
        } else if candidatos_cod.len() > 1 {
            return Ok(LigacaoAutomatica {
                acao: String::new(),
                produto_id: None,
                achado_por: String::new(),
                fator: 1000,
                motivo_pendencia: "codigo_ambiguo".into(),
                candidatos: candidatos_cod,
            });
        }
    }

    Ok(LigacaoAutomatica {
        acao: String::new(),
        produto_id: None,
        achado_por: String::new(),
        fator: 1000,
        motivo_pendencia: "sem_vinculo".into(),
        candidatos: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// PEND-09: assinatura verificada e autorização da SEFAZ (D5, D7).
// ---------------------------------------------------------------------------

/// Predicado SQL para compras em rascunho (de XML) cuja autorização na SEFAZ
/// ainda não foi comprovada nem negada. Fonte única entre a tela inicial
/// (`avisos::do_dia`) e a tela de Compras (`compras_listar`).
const SQL_AGUARDANDO_AUTORIZACAO: &str = "situacao = 'rascunho' AND xml <> '' \
     AND autorizacao NOT IN ('confirmada_gerente', 'comprovada_sefaz', 'anterior', 'negada')";

/// Diz se a situação de autorização permite lançar a compra no estoque.
pub(crate) fn autorizacao_permite_lancar(tem_xml: bool, autorizacao: &str) -> bool {
    !tem_xml || matches!(autorizacao, "confirmada_gerente" | "comprovada_sefaz" | "anterior")
}

/// Quantas notas de compra em rascunho (de XML) aguardam a conferência da autorização.
pub fn contar_aguardando_autorizacao(conn: &Connection) -> Resultado<i64> {
    Ok(conn.query_row(&format!("SELECT COUNT(*) FROM compra WHERE {SQL_AGUARDANDO_AUTORIZACAO}"), [], |l| l.get(0))?)
}

/// Contagem exata das compras pendentes de conferência ou bloqueadas (O(1) em SQL,
/// sem limite de listagem).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComprasContagem {
    pub aguardando_autorizacao: i64,
    pub negadas: i64,
}

/// Devolve a contagem exata de notas aguardando conferência da autorização e notas bloqueadas.
pub fn contagem(conn: &Connection) -> Resultado<ComprasContagem> {
    let aguardando_autorizacao = contar_aguardando_autorizacao(conn)?;
    let negadas: i64 = conn.query_row(
        "SELECT COUNT(*) FROM compra WHERE situacao = 'rascunho' AND xml <> '' AND autorizacao = 'negada'",
        [],
        |l| l.get(0),
    )?;
    Ok(ComprasContagem { aguardando_autorizacao, negadas })
}

/// Quando houver exatamente uma nota aguardando conferência da autorização,
/// devolve o `id` dela (para a notificação levar direto à conferência).
pub fn unica_aguardando_autorizacao(conn: &Connection) -> Resultado<Option<String>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT id FROM compra WHERE {SQL_AGUARDANDO_AUTORIZACAO} ORDER BY emissao DESC, criado_em DESC LIMIT 2"
    ))?;
    let ids: Vec<String> = stmt.query_map([], |l| l.get(0))?.collect::<Result<_, _>>()?;
    Ok(if ids.len() == 1 { ids.into_iter().next() } else { None })
}

/// O que se sabe da nota da compra.
pub struct SituacaoFiscal {
    pub tem_xml: bool,
    pub verificacao: String,
    pub prova: serde_json::Value,
    pub autorizacao: String,
    pub dados: serde_json::Value,
}

impl SituacaoFiscal {
    /// Por que a compra não pode ser lançada (None = pode).
    pub fn bloqueio(&self) -> Option<String> {
        if autorizacao_permite_lancar(self.tem_xml, &self.autorizacao) {
            return None;
        }
        match self.autorizacao.as_str() {
            "negada" => Some(
                "Esta nota foi conferida no portal como rejeitada, cancelada ou denegada: ela não pode entrar no estoque nem no financeiro. Descarte a importação."
                    .into(),
            ),
            _ => Some(
                "A autorização desta nota pela SEFAZ ainda não foi comprovada. O gerente precisa conferir a chave no portal da NF-e e registrar a conferência antes de lançar."
                    .into(),
            ),
        }
    }

    pub fn assinatura_texto(&self) -> String {
        if !self.tem_xml {
            return String::new();
        }
        if self.verificacao == "verificada" {
            let tipo = self.prova["tipo"].as_str().unwrap_or("");
            format!(
                "Assinatura digital do emitente conferida{}",
                if tipo.is_empty() { String::new() } else { format!(" · certificado {tipo}") }
            )
        } else {
            "Importada antes da verificação da assinatura digital".into()
        }
    }

    pub fn autorizacao_texto(&self) -> String {
        if !self.tem_xml {
            return String::new();
        }
        match self.autorizacao.as_str() {
            "aguardando" => "Autorização da SEFAZ ainda não comprovada".into(),
            "confirmada_gerente" => format!(
                "Conferência humana do gerente {} no portal da NF-e em {} (não é prova automática da SEFAZ)",
                self.dados["gerente"].as_str().unwrap_or(""),
                self.dados["conferido_em_br"].as_str().unwrap_or("")
            ),
            "negada" => format!(
                "Conferida no portal como {} pelo gerente {}: não pode ser lançada",
                self.dados["situacao"].as_str().unwrap_or("negada"),
                self.dados["gerente"].as_str().unwrap_or("")
            ),
            "comprovada_sefaz" => "Autorização comprovada na SEFAZ".into(),
            _ => String::new(),
        }
    }
}

pub fn situacao_fiscal(conn: &Connection, id: &str) -> Resultado<SituacaoFiscal> {
    let (xml_vazio, verificacao, prova, autorizacao, dados): (bool, String, String, String, String) = conn
        .query_row(
            "SELECT xml = '', xml_verificacao, xml_prova, autorizacao, autorizacao_dados FROM compra WHERE id = ?1",
            [id],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?)),
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Compra".into()))?;
    let json = |t: &str| serde_json::from_str(t).unwrap_or(serde_json::Value::Null);
    Ok(SituacaoFiscal { tem_xml: !xml_vazio, verificacao, prova: json(&prova), autorizacao, dados: json(&dados) })
}

/// "dd/mm/aaaa hh:mm" → "aaaa-mm-dd hh:mm" (hora local), ou None se inválida.
fn data_hora_local(t: &str) -> Option<String> {
    let (d, h) = t.trim().split_once(' ')?;
    let p: Vec<&str> = d.split('/').collect();
    let q: Vec<&str> = h.trim().split(':').collect();
    if p.len() != 3
        || q.len() != 2
        || p[0].len() != 2
        || p[1].len() != 2
        || p[2].len() != 4
        || q[0].len() != 2
        || q[1].len() != 2
    {
        return None;
    }
    let n = |x: &str| x.parse::<u16>().ok();
    let (dia, mes, ano, hora, minuto) = (n(p[0])?, n(p[1])?, n(p[2])?, n(q[0])?, n(q[1])?);
    let mes_t = time::Month::try_from(mes as u8).ok()?;
    time::Date::from_calendar_date(ano as i32, mes_t, dia as u8).ok()?;
    if hora > 23 || minuto > 59 {
        return None;
    }
    Some(format!("{ano:04}-{mes:02}-{dia:02} {hora:02}:{minuto:02}"))
}

/// O que o gerente conferiu no portal da NF-e (D7).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conferencia {
    pub id: String,
    /// "autorizada", "rejeitada", "cancelada" ou "denegada".
    pub situacao: String,
    /// Número do protocolo visto no portal (obrigatório para "autorizada").
    #[serde(default)]
    pub protocolo: String,
    /// Data e hora da conferência: "dd/mm/aaaa hh:mm" (hora deste computador).
    pub conferido_em: String,
}

/// D7 (provisória, até o PEND-15): registra a conferência humana do gerente.
/// `gerente_id` tem de ser de um gerente ativo autenticado **no ato** (a API
/// confere a senha). Só "autorizada" com chave e protocolo iguais aos do XML
/// libera; rejeitada/cancelada/denegada gravam `negada`, que nunca mais é
/// liberada por este caminho.
pub fn registrar_conferencia(conn: &Connection, dispositivo: &str, gerente_id: &str, c: &Conferencia) -> Resultado<()> {
    crate::sync::exigir_editavel(conn, "compra", &c.id)?;
    let gerente: Option<String> = conn
        .query_row("SELECT nome FROM usuario WHERE id = ?1 AND gerente = 1 AND ativo = 1", [gerente_id], |l| l.get(0))
        .optional()?;
    let Some(gerente) = gerente else {
        return Err(Erro::campo("senha", "Só o gerente pode registrar a conferência da nota no portal."));
    };
    let cab = cabecalho(conn, &c.id)?;
    if cab.situacao != "rascunho" {
        return Err(Erro::validacao("Esta compra não está mais em rascunho."));
    }
    let atual = situacao_fiscal(conn, &c.id)?;
    if !atual.tem_xml {
        return Err(Erro::validacao("Esta compra não veio de um XML."));
    }
    match atual.autorizacao.as_str() {
        "aguardando" => {}
        "negada" => {
            return Err(Erro::validacao(
                "Esta nota já foi registrada como rejeitada, cancelada ou denegada e não pode ser liberada.",
            ))
        }
        _ => return Err(Erro::validacao("A autorização desta nota já está registrada.")),
    }
    let situacao = c.situacao.trim();
    if !matches!(situacao, "autorizada" | "rejeitada" | "cancelada" | "denegada") {
        return Err(Erro::campo("situacao", "Escolha o que o portal mostrou."));
    }
    // Data e hora da conferência: não pode ser no futuro.
    let quando = data_hora_local(&c.conferido_em)
        .ok_or_else(|| Erro::campo("conferidoEm", "Informe a data e a hora da conferência (dd/mm/aaaa hh:mm)."))?;
    let limite: String =
        conn.query_row("SELECT strftime('%Y-%m-%d %H:%M', 'now', 'localtime', '+5 minutes')", [], |l| l.get(0))?;
    if quando > limite {
        return Err(Erro::campo("conferidoEm", "A conferência não pode estar no futuro."));
    }
    let xml: String = conn.query_row("SELECT xml FROM compra WHERE id = ?1", [&c.id], |l| l.get(0))?;
    let nota = nfe_xml::ler(&xml).map_err(Erro::validacao)?;
    let protocolo: String = c.protocolo.chars().filter(|x| x.is_ascii_digit()).collect();
    if situacao == "autorizada" {
        if protocolo.is_empty() {
            return Err(Erro::campo("protocolo", "Digite o número do protocolo de autorização que o portal mostrou."));
        }
        if protocolo != nota.protocolo {
            return Err(Erro::campo(
                "protocolo",
                "Este protocolo não é o desta nota. Confira no portal da NF-e pela chave de acesso.",
            ));
        }
    }
    let novo = if situacao == "autorizada" { "confirmada_gerente" } else { "negada" };
    let dados = serde_json::json!({
        "gerente_id": gerente_id, "gerente": gerente, "situacao": situacao, "chave": nota.chave,
        "protocolo": protocolo, "conferido_em": quando, "conferido_em_br": c.conferido_em.trim(),
        "registrado_em": tempo::agora(), "tipo": "conferencia_humana",
    });
    conn.execute(
        "UPDATE compra SET autorizacao = ?2, autorizacao_dados = ?3 WHERE id = ?1",
        params![c.id, novo, dados.to_string()],
    )?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(gerente_id),
        "compra.autorizacao",
        "compra",
        &c.id,
        &[format!(
            "Conferência humana no portal da NF-e (não é prova automática da SEFAZ): {situacao}; chave {}; protocolo {}; conferido em {}; gerente {gerente}",
            nota.chave,
            if protocolo.is_empty() { "-".into() } else { protocolo.clone() },
            c.conferido_em.trim()
        )],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Ler para a tela.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompraFicha {
    pub id: String,
    pub situacao: String,
    pub fornecedor_id: String,
    pub fornecedor: String,
    pub fornecedor_documento: String,
    pub fornecedor_cidade: String,
    pub chave: String,
    pub numero: String,
    pub serie: String,
    pub emissao: String,
    pub natureza: String,
    pub total_produtos: String,
    pub frete: String,
    pub ipi: String,
    pub icms_st: String,
    pub desconto: String,
    pub outras: String,
    pub total: String,
    pub frete_extra: String,
    pub atualizar_precos: bool,
    pub observacoes: String,
    /// Aviso quando a nota não é para o CNPJ da loja.
    pub aviso_destinatario: String,
    /// PEND-09: "verificada" (assinatura do emitente conferida) ou
    /// "nao_verificada" (importada antes da verificação, D5).
    pub verificacao: String,
    /// Linha curta sobre a assinatura (ex.: "Assinatura do emitente conferida · A1").
    pub assinatura_texto: String,
    /// "anterior", "aguardando", "confirmada_gerente", "negada", "comprovada_sefaz".
    pub autorizacao: String,
    /// Linha curta sobre a autorização da SEFAZ.
    pub autorizacao_texto: String,
    /// A compra pode ser lançada agora? (só a autorização; os itens à parte)
    pub pode_lancar: bool,
    pub motivo_bloqueio: String,
    pub itens: Vec<ItemFicha>,
    pub pendentes: i64,
    pub novas: i64,
    pub ignorados: i64,
    pub custo_total: String,
    pub lancada_em: String,
    pub lancada_por: String,
    pub historico: Vec<auditoria::Registro>,
    /// Parcelas da nota (duplicatas do XML): "001 · vence 07/11/2026 · R$ 550,25".
    pub duplicatas: Vec<String>,
    /// Lançar as duplicatas no A pagar ao lançar a compra.
    pub gerar_pagar: bool,
    /// Já lançada: parcelas no A pagar (quantas).
    pub parcelas_pagar: i64,
    /// T111 (Achado 9c): em rascunho, há pelo menos uma peça provisória criada
    /// na importação que será removida/desativada ao descartar esta compra?
    pub tem_provisorias_descartaveis: bool,
    /// R2 (`REC-1`): assinatura determinística do estado atual da compra e seus itens
    /// para detectar alterações concorrentes ao retomar um rascunho.
    pub revisao_base: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemFicha {
    pub id: String,
    pub ordem: i64,
    pub codigo_fornecedor: String,
    pub gtin: String,
    pub descricao: String,
    pub ncm: String,
    pub cest: String,
    pub cfop: String,
    pub unidade: String,
    pub quantidade: String,
    pub valor_unitario: String,
    pub valor_produtos: String,
    pub despesas: String,
    pub acao: String,
    pub achado_por: String,
    /// T111 (Achado 9b): motivo legível da pendência de identificação quando `acao == ""`.
    pub motivo_pendencia: String,
    pub fator: String,
    /// Quantas peças entram no estoque.
    pub quantidade_pecas: String,
    /// Custo de cada peça, já com frete, IPI e ST.
    pub custo_unitario: String,
    pub monofasico: bool,
    pub peca: Option<PecaLigada>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PecaLigada {
    pub id: String,
    pub codigo: String,
    pub nome: String,
    pub marca: String,
    pub unidade: String,
    pub estoque: String,
    pub custo_atual: String,
    pub preco_atual: String,
    /// Preço de revenda depois da compra (se "atualizar preços" estiver marcado).
    pub preco_novo: String,
    /// Variação do custo em % ("+12,5" / "-3").
    pub variacao_custo: String,
    pub custo_subiu: bool,
    /// O custo novo é menor: a tela pergunta se o preço baixa junto.
    pub custo_baixou: bool,
    /// Escolha da tela: baixar o preço pelo % (true) ou manter (false).
    pub baixar_preco: bool,
    /// Preço se baixar pelo % (para mostrar na pergunta).
    pub preco_se_baixar: String,
}

struct ItemBanco {
    id: String,
    ordem: i64,
    codigo_fornecedor: String,
    gtin: String,
    gtins_json: String,
    descricao: String,
    ncm: String,
    cest: String,
    cfop: String,
    unidade: String,
    origem: i64,
    cst_pis: String,
    quantidade: i64,
    valor_produtos: i64,
    frete: i64,
    seguro: i64,
    desconto: i64,
    outras: i64,
    ipi: i64,
    icms_st: i64,
    acao: String,
    produto_id: Option<String>,
    achado_por: String,
    fator: i64,
    baixar_preco: bool,
}

impl ItemBanco {
    /// Valor que a loja pagou pelo item, sem o frete pago à parte.
    fn custo_nota(&self) -> Resultado<i64> {
        numeros::soma([
            self.valor_produtos,
            -self.desconto,
            self.frete,
            self.seguro,
            self.outras,
            self.ipi,
            self.icms_st,
        ])
    }
    /// Quantidade de peças (milésimos).
    fn pecas(&self) -> i64 {
        crate::numeros::saturar((self.quantidade as i128 * self.fator as i128 + 500) / 1000)
    }
    fn gtins_ficha(&self) -> Vec<produtos::GtinFicha> {
        serde_json::from_str::<Vec<nfe_xml::GtinXml>>(&self.gtins_json)
            .unwrap_or_default()
            .into_iter()
            .map(|g| produtos::GtinFicha { gtin: g.codigo, tipo: g.tipo, origem: g.origem })
            .collect()
    }
}

fn itens_banco(conn: &Connection, compra_id: &str) -> Resultado<Vec<ItemBanco>> {
    let mut c = conn.prepare_cached(
        "SELECT id, ordem, codigo_fornecedor, gtin, descricao, ncm, cest, cfop, unidade, origem, cst_pis, quantidade,
                valor_produtos, frete, seguro, desconto, outras, ipi, icms_st, acao, produto_id, achado_por, fator,
                baixar_preco, COALESCE(gtins_json, '[]')
           FROM compra_item WHERE compra_id = ?1 ORDER BY ordem",
    )?;
    let linhas = c.query_map([compra_id], |l| {
        Ok(ItemBanco {
            id: l.get(0)?,
            ordem: l.get(1)?,
            codigo_fornecedor: l.get(2)?,
            gtin: l.get(3)?,
            gtins_json: l.get(24)?,
            descricao: l.get(4)?,
            ncm: l.get(5)?,
            cest: l.get(6)?,
            cfop: l.get(7)?,
            unidade: l.get(8)?,
            origem: l.get(9)?,
            cst_pis: l.get(10)?,
            quantidade: l.get(11)?,
            valor_produtos: l.get(12)?,
            frete: l.get(13)?,
            seguro: l.get(14)?,
            desconto: l.get(15)?,
            outras: l.get(16)?,
            ipi: l.get(17)?,
            icms_st: l.get(18)?,
            acao: l.get(19)?,
            produto_id: l.get(20)?,
            achado_por: l.get(21)?,
            fator: l.get(22)?,
            baixar_preco: l.get(23)?,
        })
    })?;
    let mut itens: Vec<ItemBanco> = linhas.collect::<Result<_, _>>()?;
    if itens.iter().any(|i| i.acao.is_empty() && i.produto_id.is_none()) {
        if let Some((situacao, fornecedor_id, chave_compra)) = conn
            .query_row(
                "SELECT situacao, COALESCE(fornecedor_id, ''), chave FROM compra WHERE id = ?1",
                [compra_id],
                |l| Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?, l.get::<_, String>(2)?)),
            )
            .optional()?
        {
            if situacao == "rascunho" && !fornecedor_id.is_empty() {
                for it in &mut itens {
                    if !it.acao.is_empty() || it.produto_id.is_some() {
                        continue;
                    }
                    let cod_comp = texto::compactar(&it.codigo_fornecedor);
                    let desc_comp = texto::compactar(&it.descricao);
                    // R9: se o código tem alfanuméricos, religa qualquer rascunho pendente do mesmo
                    // fornecedor + código + descrição (não só a última chave_nfe gravada); código só
                    // de símbolos (A01) continua restrito à chave_nfe exata.
                    let resolvido: Option<String> = conn
                        .query_row(
                            "SELECT x.produto_id FROM xml_pendencia_identificacao x
                               JOIN produto p ON p.id = x.produto_id AND p.ativo = 1
                              WHERE x.fornecedor_id = ?1
                                AND (x.chave_nfe = ?2 OR x.codigo_compacto <> '')
                                AND x.codigo_compacto = ?3 AND x.descricao_compacta = ?4
                                AND x.situacao = 'resolvida' AND x.produto_id IS NOT NULL
                              ORDER BY CASE WHEN x.chave_nfe = ?2 THEN 0 ELSE 1 END, x.resolvido_em DESC
                              LIMIT 1",
                            params![fornecedor_id, chave_compra, cod_comp, desc_comp],
                            |l| l.get(0),
                        )
                        .optional()?;
                    if let Some(pid) = resolvido {
                        it.acao = "ligar".into();
                        it.produto_id = Some(pid);
                        it.achado_por = "fornecedor".into();
                    }
                }
            }
        }
    }
    Ok(itens)
}

/// Frete pago à parte dividido entre os itens que entram no estoque.
fn frete_extra_por_item(itens: &[ItemBanco], frete_extra: i64) -> Vec<i64> {
    let pesos: Vec<i64> = itens.iter().map(|i| if i.acao == "ignorar" { 0 } else { i.valor_produtos.max(0) }).collect();
    if frete_extra == 0 || pesos.iter().all(|p| *p == 0) {
        return vec![0; itens.len()];
    }
    ratear(frete_extra, &pesos)
}

/// Custo de uma peça (centavos), arredondado.
fn custo_unitario(custo_total: i64, pecas_milesimos: i64) -> i64 {
    if pecas_milesimos <= 0 {
        return 0;
    }
    crate::numeros::saturar((custo_total as i128 * 1000 + pecas_milesimos as i128 / 2) / pecas_milesimos as i128)
}

struct Cabecalho {
    situacao: String,
    fornecedor_id: String,
    chave: String,
    frete_extra: i64,
    atualizar_precos: bool,
    numero: String,
}

fn cabecalho(conn: &Connection, id: &str) -> Resultado<Cabecalho> {
    conn.query_row(
        "SELECT situacao, COALESCE(fornecedor_id, ''), chave, frete_extra, atualizar_precos, numero
           FROM compra WHERE id = ?1",
        [id],
        |l| {
            Ok(Cabecalho {
                situacao: l.get(0)?,
                fornecedor_id: l.get(1)?,
                chave: l.get(2)?,
                frete_extra: l.get(3)?,
                atualizar_precos: l.get(4)?,
                numero: l.get(5)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| Erro::NaoEncontrado("Compra".into()))
}

/// Peça já foi editada manualmente ou movimentada desde o cadastro?
fn peca_ja_tocada(conn: &Connection, produto_id: &str) -> Resultado<bool> {
    let tempos: Option<(String, String)> = conn
        .query_row("SELECT criado_em, atualizado_em FROM produto WHERE id = ?1", [produto_id], |l| {
            Ok((l.get(0)?, l.get(1)?))
        })
        .optional()?;
    let Some((criado_em, atualizado_em)) = tempos else {
        return Ok(false);
    };
    if criado_em != atualizado_em {
        return Ok(true);
    }
    if conn
        .query_row("SELECT EXISTS(SELECT 1 FROM estoque_movimento WHERE produto_id = ?1)", [produto_id], |l| l.get(0))?
    {
        return Ok(true);
    }
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM auditoria
          WHERE entidade = 'produto' AND entidade_id = ?1
            AND acao IN ('produto.alterar', 'compra.entrada', 'compra.estornar'))",
        [produto_id],
        |l| l.get(0),
    )?)
}

/// T111 (Achado 9c): esta peça provisória seria removida/desativada ao descartar `compra_id`?
fn peca_provisoria_descartavel(conn: &Connection, compra_id: &str, produto_id: &str) -> Resultado<bool> {
    let ativa: bool = conn
        .query_row("SELECT ativo FROM produto WHERE id = ?1", [produto_id], |l| Ok(l.get::<_, i64>(0)? != 0))
        .optional()?
        .unwrap_or(false);
    if !ativa || peca_ja_tocada(conn, produto_id)? {
        return Ok(false);
    }
    let veio_de_compra: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM auditoria
          WHERE entidade = 'produto' AND entidade_id = ?1 AND acao = 'produto.importar_compra')",
        [produto_id],
        |l| l.get(0),
    )?;
    if !veio_de_compra {
        return Ok(false);
    }
    let em_outra_compra: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM compra_item WHERE produto_id = ?1 AND compra_id <> ?2)",
        params![produto_id, compra_id],
        |l| l.get(0),
    )?;
    if em_outra_compra {
        return Ok(false);
    }
    for sql in [
        "SELECT EXISTS(SELECT 1 FROM venda_item WHERE produto_id = ?1)",
        "SELECT EXISTS(SELECT 1 FROM devolucao_item WHERE produto_id = ?1)",
        "SELECT EXISTS(SELECT 1 FROM ordem_servico_item WHERE produto_id = ?1)",
        "SELECT EXISTS(SELECT 1 FROM encomenda WHERE produto_id = ?1)",
        "SELECT EXISTS(SELECT 1 FROM produto_codigo_antigo WHERE produto_id = ?1)",
        "SELECT EXISTS(SELECT 1 FROM xml_pendencia_identificacao WHERE produto_id = ?1)",
    ] {
        if conn.query_row(sql, [produto_id], |l| l.get::<_, bool>(0))? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// T111 (Achado 9b): texto legível do motivo da pendência aberta deste item.
fn motivo_pendencia_do_item(
    conn: &Connection,
    fornecedor_id: &str,
    fornecedor_doc: &str,
    chave_nfe: &str,
    codigo_fornecedor: &str,
    descricao: &str,
) -> Resultado<String> {
    let compacto = texto::compactar(codigo_fornecedor);
    let desc_comp = texto::compactar(descricao);
    let motivo: Option<String> = conn
        .query_row(
            "SELECT motivo FROM xml_pendencia_identificacao
              WHERE situacao = 'pendente'
                AND (fornecedor_id = ?1 OR (?2 <> '' AND fornecedor_documento = ?2))
                AND (chave_nfe = ?3 OR codigo_compacto <> '')
                AND codigo_compacto = ?4
                AND descricao_compacta = ?5
              ORDER BY CASE WHEN chave_nfe = ?3 THEN 0 ELSE 1 END, criado_em DESC
              LIMIT 1",
            params![fornecedor_id, fornecedor_doc, chave_nfe, compacto, desc_comp],
            |l| l.get(0),
        )
        .optional()?;
    Ok(motivo.map(|m| crate::importar::motivo_legivel(&m).to_string()).unwrap_or_default())
}

/// T111 (Achado 3a/3b): devolve o código de fornecedor corrigido (e se houve
/// correção de GTIN) quando o item veio de uma pendência resolvida com
/// `corrigir: true`, para `lancar()` não regravar código/GTIN antigos do XML.
fn correcao_da_pendencia(
    conn: &Connection,
    fornecedor_id: &str,
    chave_nfe: &str,
    it: &ItemBanco,
    produto_id: &str,
) -> Resultado<Option<(String, bool)>> {
    if fornecedor_id.is_empty() {
        return Ok(None);
    }
    let cod_comp = texto::compactar(&it.codigo_fornecedor);
    let desc_comp = texto::compactar(&it.descricao);
    let achada: Option<(String, String)> = conn
        .query_row(
            "SELECT codigo_fornecedor, gtins_json FROM xml_pendencia_identificacao
              WHERE fornecedor_id = ?1
                AND (chave_nfe = ?2 OR codigo_compacto <> '')
                AND codigo_compacto = ?3
                AND descricao_compacta = ?4
                AND situacao = 'resolvida'
                AND produto_id = ?5
              ORDER BY CASE WHEN chave_nfe = ?2 THEN 0 ELSE 1 END, resolvido_em DESC
              LIMIT 1",
            params![fornecedor_id, chave_nfe, cod_comp, desc_comp, produto_id],
            |l| Ok((l.get(0)?, l.get(1)?)),
        )
        .optional()?;
    let Some((cod_pend, gtins_pend)) = achada else {
        return Ok(None);
    };
    let mudou_codigo = cod_pend.trim() != it.codigo_fornecedor.trim();
    let mudou_gtin = gtins_pend != it.gtins_json;
    if mudou_codigo || mudou_gtin {
        Ok(Some((cod_pend, mudou_gtin)))
    } else {
        Ok(None)
    }
}

/// Ficha completa da compra.
pub fn ficha(conn: &Connection, id: &str) -> Resultado<CompraFicha> {
    /// A linha da compra, com o fornecedor, como a ficha lê (A16: no lugar da tupla de 21 campos).
    struct LinhaDaCompra {
        situacao: String,
        fornecedor_id: String,
        fornecedor_nome: String,
        fornecedor_documento: String,
        cidade: String,
        chave: String,
        numero: String,
        serie: String,
        total_produtos: i64,
        frete: i64,
        ipi: i64,
        icms_st: i64,
        desconto: i64,
        outras: i64,
        total: i64,
        frete_extra: i64,
        atualizar_precos: i64,
        emissao: String,
        natureza: String,
        lancada_em: Option<String>,
        destinatario: String,
    }
    let c = conn
        .query_row(
            "SELECT c.situacao, COALESCE(c.fornecedor_id, ''), COALESCE(p.nome, ''), COALESCE(p.documento, ''),
                    COALESCE(p.municipio || CASE WHEN p.uf <> '' THEN ' · ' || p.uf ELSE '' END, ''),
                    c.chave, c.numero, c.serie, c.total_produtos, c.frete, c.ipi, c.icms_st, c.desconto, c.outras,
                    c.total, c.frete_extra, c.atualizar_precos, c.emissao, c.natureza, c.lancada_em,
                    c.destinatario
               FROM compra c LEFT JOIN pessoa p ON p.id = c.fornecedor_id WHERE c.id = ?1",
            [id],
            |l| {
                Ok(LinhaDaCompra {
                    situacao: l.get(0)?,
                    fornecedor_id: l.get(1)?,
                    fornecedor_nome: l.get(2)?,
                    fornecedor_documento: l.get(3)?,
                    cidade: l.get(4)?,
                    chave: l.get(5)?,
                    numero: l.get(6)?,
                    serie: l.get(7)?,
                    total_produtos: l.get(8)?,
                    frete: l.get(9)?,
                    ipi: l.get(10)?,
                    icms_st: l.get(11)?,
                    desconto: l.get(12)?,
                    outras: l.get(13)?,
                    total: l.get(14)?,
                    frete_extra: l.get(15)?,
                    atualizar_precos: l.get(16)?,
                    emissao: l.get(17)?,
                    natureza: l.get(18)?,
                    lancada_em: l.get(19)?,
                    destinatario: l.get(20)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Compra".into()))?;
    let LinhaDaCompra {
        situacao,
        fornecedor_id,
        fornecedor_nome: fornecedor,
        fornecedor_documento: fdoc,
        cidade,
        chave,
        numero,
        serie,
        total_produtos: tprod,
        frete,
        ipi,
        icms_st: st,
        desconto: desc,
        outras,
        total,
        frete_extra,
        atualizar_precos: atualizar,
        emissao,
        natureza,
        lancada_em,
        destinatario,
    } = c;
    let atualizar_precos = atualizar != 0;
    let (observacoes, lancada_por): (String, String) = conn.query_row(
        "SELECT c.observacoes, COALESCE(u.nome, '') FROM compra c LEFT JOIN usuario u ON u.id = c.lancada_por WHERE c.id = ?1",
        [id],
        |l| Ok((l.get(0)?, l.get(1)?)),
    )?;

    let itens = itens_banco(conn, id)?;
    let extras = frete_extra_por_item(&itens, frete_extra);
    let mut lista = Vec::new();
    let (mut pendentes, mut novas, mut ignorados, mut custo_total) = (0, 0, 0, 0_i64);
    let mut tem_provisorias_descartaveis = false;
    for (i, it) in itens.iter().enumerate() {
        let custo_item = numeros::soma([it.custo_nota()?, extras[i]])?;
        if it.acao != "ignorar" {
            custo_total = numeros::somar([custo_total, custo_item]).ok_or_else(numeros::grande_demais)?;
        }
        match it.acao.as_str() {
            "" => pendentes += 1,
            "nova" => novas += 1,
            "ignorar" => ignorados += 1,
            _ => {}
        }
        let pecas = it.pecas();
        let cu = custo_unitario(custo_item, pecas);
        let peca = match (&it.acao[..], &it.produto_id) {
            ("ligar", Some(pid)) => peca_ligada(conn, pid, cu, atualizar_precos, it.baixar_preco)?,
            // Achado 9d: em rascunho, prévia da peça recém-criada na importação acompanha
            // exatamente `finalizar_peca_importada` (se virgem) ou `atualizar_custo` (se já tocada).
            ("nova", Some(pid)) if situacao == "rascunho" && it.achado_por == "importacao" => {
                if peca_ja_tocada(conn, pid)? {
                    peca_ligada(conn, pid, cu, atualizar_precos, it.baixar_preco)?
                } else {
                    // V2-1: peça provisória virgem ainda não foi consolidada nem editada;
                    // o preço inicial acompanha sempre o custo real unitário (com fator e frete) +
                    // lucro padrão, sem exibir um falso "antes R$ ..." / "O custo baixou" / "era R$ ...".
                    peca_ligada(conn, pid, cu, true, true)?.map(|mut p| {
                        p.custo_atual.clear();
                        p.variacao_custo.clear();
                        p.custo_subiu = false;
                        p.custo_baixou = false;
                        p.baixar_preco = false;
                        p.preco_atual = p.preco_novo.clone();
                        p
                    })
                }
            }
            // D-06: depois de lançada, o item "peça nova" mostra a peça criada.
            ("nova", Some(pid)) => peca_ligada(conn, pid, cu, false, false)?,
            _ => None,
        };
        if situacao == "rascunho" && !tem_provisorias_descartaveis {
            if let Some(pid) = &it.produto_id {
                if peca_provisoria_descartavel(conn, id, pid)? {
                    tem_provisorias_descartaveis = true;
                }
            }
        }
        let motivo_pendencia = if it.acao.is_empty() {
            motivo_pendencia_do_item(conn, &fornecedor_id, &fdoc, &chave, &it.codigo_fornecedor, &it.descricao)?
        } else {
            String::new()
        };
        lista.push(ItemFicha {
            id: it.id.clone(),
            ordem: it.ordem + 1,
            codigo_fornecedor: it.codigo_fornecedor.clone(),
            gtin: it.gtin.clone(),
            descricao: it.descricao.clone(),
            ncm: it.ncm.clone(),
            cest: it.cest.clone(),
            cfop: it.cfop.clone(),
            unidade: it.unidade.clone(),
            quantidade: numeros::escrever_quantidade(it.quantidade),
            valor_unitario: numeros::escrever_dinheiro(custo_unitario(it.valor_produtos, it.quantidade)),
            valor_produtos: numeros::escrever_dinheiro(it.valor_produtos),
            despesas: numeros::escrever_dinheiro(custo_item - it.valor_produtos),
            acao: it.acao.clone(),
            achado_por: it.achado_por.clone(),
            motivo_pendencia,
            fator: numeros::escrever_quantidade(it.fator),
            quantidade_pecas: numeros::escrever_quantidade(pecas),
            custo_unitario: numeros::escrever_dinheiro(cu),
            monofasico: it.cst_pis == "04",
            peca,
        });
    }
    let aviso_destinatario = aviso_cnpj(conn, &destinatario)?;
    let fiscal_xml = situacao_fiscal(conn, id)?;
    Ok(CompraFicha {
        id: id.to_string(),
        situacao,
        fornecedor_id,
        fornecedor,
        fornecedor_documento: documento::escrever(&fdoc),
        fornecedor_cidade: cidade,
        chave,
        numero,
        serie,
        emissao: fiscal::data_br(&emissao),
        natureza,
        total_produtos: numeros::escrever_dinheiro(tprod),
        frete: numeros::escrever_dinheiro(frete),
        ipi: numeros::escrever_dinheiro(ipi),
        icms_st: numeros::escrever_dinheiro(st),
        desconto: numeros::escrever_dinheiro(desc),
        outras: numeros::escrever_dinheiro(outras),
        total: numeros::escrever_dinheiro(total),
        frete_extra: if frete_extra > 0 { numeros::escrever_dinheiro(frete_extra) } else { String::new() },
        atualizar_precos,
        observacoes,
        aviso_destinatario,
        verificacao: fiscal_xml.verificacao.clone(),
        assinatura_texto: fiscal_xml.assinatura_texto(),
        autorizacao: fiscal_xml.autorizacao.clone(),
        autorizacao_texto: fiscal_xml.autorizacao_texto(),
        pode_lancar: fiscal_xml.bloqueio().is_none(),
        motivo_bloqueio: fiscal_xml.bloqueio().unwrap_or_default(),
        itens: lista,
        pendentes,
        novas,
        ignorados,
        custo_total: numeros::escrever_dinheiro(custo_total),
        lancada_em: lancada_em.unwrap_or_default(),
        lancada_por,
        historico: auditoria::historico(conn, "compra", id, 20)?,
        duplicatas: crate::pagar::duplicatas_da_compra(conn, id)?
            .iter()
            .map(|d| {
                [
                    d.numero.clone(),
                    format!("vence {}", fiscal::data_br(&d.vencimento)),
                    format!("R$ {}", numeros::escrever_dinheiro(d.valor)),
                ]
                .into_iter()
                .filter(|x| !x.is_empty())
                .collect::<Vec<_>>()
                .join(" · ")
            })
            .collect(),
        gerar_pagar: conn.query_row("SELECT gerar_pagar FROM compra WHERE id = ?1", [id], |l| l.get(0))?,
        parcelas_pagar: conn.query_row(
            "SELECT COUNT(*) FROM conta_pagar WHERE compra_id = ?1 AND cancelado_em IS NULL",
            [id],
            |l| l.get(0),
        )?,
        tem_provisorias_descartaveis,
        revisao_base: conn
            .query_row(
                "SELECT c.situacao || '|' || COALESCE(c.lancada_em, '') || '|' || COALESCE(c.criado_em, '') || '|' ||
                        c.frete_extra || '|' || c.atualizar_precos || '|' || c.observacoes || '|' ||
                        c.autorizacao || '|' || COALESCE(c.autorizacao_dados, '') || '|' ||
                        COALESCE((SELECT GROUP_CONCAT(
                            ci.ordem || ':' || ci.acao || ':' || COALESCE(ci.produto_id, '') || ':' || ci.fator || ':' || ci.baixar_preco,
                            ';'
                        ) FROM (SELECT ordem, acao, produto_id, fator, baixar_preco FROM compra_item WHERE compra_id = c.id ORDER BY ordem) ci), '')
                   FROM compra c WHERE c.id = ?1",
                [id],
                |l| l.get(0),
            )
            .unwrap_or_default(),
    })
}

fn peca_ligada(
    conn: &Connection,
    produto_id: &str,
    custo_novo: i64,
    atualizar_precos: bool,
    baixar_preco: bool,
) -> Resultado<Option<PecaLigada>> {
    let p: Option<(String, String, String, String, i64)> = conn
        .query_row("SELECT codigo, nome, marca, unidade, custo FROM produto WHERE id = ?1", [produto_id], |l| {
            Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?))
        })
        .optional()?;
    let Some((codigo, nome, marca, unidade, custo)) = p else { return Ok(None) };
    let preco: Option<(i64, Option<i64>)> = conn
        .query_row("SELECT preco, lucro FROM produto_preco WHERE produto_id = ?1 AND tabela = 1", [produto_id], |l| {
            Ok((l.get(0)?, l.get(1)?))
        })
        .optional()?;
    let preco_atual = preco.map(|p| p.0).unwrap_or(0);
    let custo_baixou = custo > 0 && custo_novo > 0 && custo_novo < custo;
    let pelo_lucro = match preco {
        Some((_, Some(lucro))) if custo_novo > 0 => numeros::preco_por_lucro(custo_novo, lucro),
        _ => preco_atual,
    };
    let preco_novo = if !atualizar_precos || (custo_baixou && !baixar_preco) { preco_atual } else { pelo_lucro };
    let variacao = if custo > 0 && custo_novo > 0 {
        let v = (custo_novo - custo) as f64 * 100.0 / custo as f64;
        let v = (v * 10.0).round() / 10.0;
        if v == 0.0 {
            String::new()
        } else {
            let t = format!("{v:+.1}").replace('.', ",");
            t.trim_end_matches(",0").to_string()
        }
    } else {
        String::new()
    };
    Ok(Some(PecaLigada {
        id: produto_id.to_string(),
        codigo,
        nome,
        marca,
        unidade,
        estoque: numeros::escrever_quantidade(estoque::saldo(conn, produto_id)?),
        custo_atual: if custo > 0 { numeros::escrever_dinheiro(custo) } else { String::new() },
        preco_atual: if preco_atual > 0 { numeros::escrever_dinheiro(preco_atual) } else { String::new() },
        preco_novo: if preco_novo > 0 { numeros::escrever_dinheiro(preco_novo) } else { String::new() },
        custo_subiu: custo > 0 && custo_novo > custo,
        variacao_custo: variacao,
        custo_baixou,
        baixar_preco,
        preco_se_baixar: if pelo_lucro > 0 { numeros::escrever_dinheiro(pelo_lucro) } else { String::new() },
    }))
}

// ---------------------------------------------------------------------------
// Mudar o rascunho.
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CompraEntrada {
    pub id: String,
    pub frete_extra: String,
    pub atualizar_precos: bool,
    pub observacoes: String,
    pub itens: Vec<ItemEntrada>,
    pub gerar_pagar: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ItemEntrada {
    pub id: String,
    pub acao: String,
    pub produto_id: String,
    pub fator: String,
    /// Quando o custo baixou: baixar o preço junto?
    pub baixar_preco: bool,
}

/// Peças atualmente ligadas aos itens de uma compra em rascunho: (produto_id, codigo_fornecedor).
pub(crate) fn ligados_no_rascunho(conn: &Connection, compra_id: &str) -> Resultado<Vec<(String, String)>> {
    let mut stmt = conn.prepare_cached(
        "SELECT produto_id, codigo_fornecedor FROM compra_item WHERE compra_id = ?1 AND produto_id IS NOT NULL",
    )?;
    let linhas = stmt.query_map([compra_id], |l| Ok((l.get(0)?, l.get(1)?)))?.collect::<Result<_, _>>()?;
    Ok(linhas)
}

/// T111 (A5): após `atualizar`, remove peças provisórias (criadas na importação
/// de compra) que deixaram de ser referenciadas por esta compra e ficaram órfãs.
pub(crate) fn limpar_provisorios_removidos(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    compra_id: &str,
    antes: &[(String, String)],
) -> Resultado<()> {
    if antes.is_empty() {
        return Ok(());
    }
    let cab = cabecalho(conn, compra_id)?;
    let mut vistos = HashSet::new();
    for (pid, cod_forn) in antes {
        if vistos.insert(pid.clone()) && crate::sync::exigir_editavel(conn, "produto", pid).is_ok() {
            produtos::remover_provisorio_se_orfao(conn, dispositivo, usuario_id, pid, &cab.fornecedor_id, cod_forn)?;
        }
    }
    Ok(())
}

/// Grava as escolhas da tela (só em rascunho), preservando peças criadas na
/// importação (Opção C) quando o item continua como `nova`.
pub fn atualizar(conn: &Connection, e: &CompraEntrada) -> Resultado<()> {
    crate::sync::exigir_editavel(conn, "compra", &e.id)?;
    crate::sync::exigir_filhos_editaveis(conn, "compra_item", "compra_id", &e.id)?;
    let cab = cabecalho(conn, &e.id)?;
    if cab.situacao != "rascunho" {
        return Err(Erro::validacao("Esta compra já foi lançada e não pode mais ser mudada."));
    }
    let frete_extra = numeros::ler_dinheiro(&e.frete_extra).map_err(|m| Erro::campo("freteExtra", m))?.unwrap_or(0);
    if frete_extra < 0 {
        return Err(Erro::campo("freteExtra", "O frete não pode ser negativo."));
    }
    let observacoes = e.observacoes.trim().to_string();
    if texto::tamanho(&observacoes) > 1000 {
        return Err(Erro::campo("observacoes", "Use no máximo 1000 caracteres."));
    }
    conn.execute(
        "UPDATE compra SET frete_extra = ?2, atualizar_precos = ?3, observacoes = ?4, gerar_pagar = ?5 WHERE id = ?1",
        params![e.id, frete_extra, e.atualizar_precos, observacoes, e.gerar_pagar],
    )?;
    for (i, it) in e.itens.iter().enumerate() {
        let acao = it.acao.as_str();
        if !["", "ligar", "nova", "ignorar"].contains(&acao) {
            return Err(Erro::Interno(format!("ação desconhecida: {acao}")));
        }
        let fator = numeros::ler_quantidade(&it.fator)
            .map_err(|m| Erro::campo(&format!("itens.{i}.fator"), m))?
            .unwrap_or(1000);
        if fator <= 0 || fator > 10_000_000 {
            return Err(Erro::campo(
                &format!("itens.{i}.fator"),
                "Quantas peças vêm em cada unidade da nota? (1 ou mais)",
            ));
        }
        let atual: Option<(String, Option<String>, String)> = conn
            .query_row(
                "SELECT acao, produto_id, achado_por FROM compra_item WHERE id = ?1 AND compra_id = ?2",
                params![it.id, e.id],
                |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
            )
            .optional()?;
        let Some((acao_antes, prod_antes, achado_antes)) = atual else {
            return Err(Erro::Interno("item não pertence à compra".into()));
        };
        let (produto_id, achado_por) = if acao == "ligar" {
            if it.produto_id.is_empty() {
                return Err(Erro::campo(&format!("itens.{i}.produto"), "Escolha a peça."));
            }
            let existe: bool = conn
                .query_row("SELECT 1 FROM produto WHERE id = ?1", [&it.produto_id], |_| Ok(true))
                .optional()?
                .unwrap_or(false);
            if !existe {
                return Err(Erro::NaoEncontrado("Peça".into()));
            }
            let ach = if prod_antes.as_deref() == Some(it.produto_id.as_str()) {
                achado_antes.clone()
            } else {
                String::new()
            };
            (Some(it.produto_id.clone()), ach)
        } else if acao == "nova" && acao_antes == "nova" && achado_antes == "importacao" {
            if let Some(pid) = prod_antes.as_ref().filter(|pid| {
                conn.query_row("SELECT 1 FROM produto WHERE id = ?1", [pid.as_str()], |_| Ok(true))
                    .optional()
                    .ok()
                    .flatten()
                    .unwrap_or(false)
            }) {
                (Some(pid.clone()), "importacao".into())
            } else {
                (None, String::new())
            }
        } else {
            (None, String::new())
        };
        conn.execute(
            "UPDATE compra_item SET acao = ?3, produto_id = ?4, fator = ?5, baixar_preco = ?6, achado_por = ?7
              WHERE id = ?1 AND compra_id = ?2",
            params![it.id, e.id, acao, produto_id, fator, it.baixar_preco, achado_por],
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Lançar e estornar.
// ---------------------------------------------------------------------------

/// Resumo do que o lançamento fez.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lancamento {
    pub pecas_novas: i64,
    pub itens_estoque: i64,
    pub precos_atualizados: i64,
    /// Parcelas lançadas no A pagar.
    pub parcelas_pagar: i64,
    /// Encomendas de clientes que chegaram nesta compra.
    pub encomendas: i64,
}

/// Põe a compra no estoque.
pub fn lancar(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str) -> Resultado<Lancamento> {
    crate::sync::exigir_editavel(conn, "compra", id)?;
    crate::sync::exigir_filhos_editaveis(conn, "compra_item", "compra_id", id)?;
    let cab = cabecalho(conn, id)?;
    if cab.situacao != "rascunho" {
        return Err(Erro::validacao("Esta compra já foi lançada."));
    }
    // PEND-09 / D7: porta única do estoque, do custo e do financeiro.
    if let Some(motivo) = situacao_fiscal(conn, id)?.bloqueio() {
        return Err(Erro::validacao(motivo));
    }
    let itens = itens_banco(conn, id)?;
    if let Some(p) = itens.iter().find(|i| i.acao.is_empty()) {
        return Err(Erro::validacao(format!(
            "Falta decidir o item {} ({}): ligar a uma peça, cadastrar como nova ou não lançar.",
            p.ordem + 1,
            p.descricao
        )));
    }
    if itens.iter().all(|i| i.acao == "ignorar") {
        return Err(Erro::validacao("Nenhum item vai entrar no estoque."));
    }
    let fornecedor: String = conn
        .query_row("SELECT nome FROM pessoa WHERE id = ?1", [&cab.fornecedor_id], |l| l.get(0))
        .optional()?
        .unwrap_or_default();
    let extras = frete_extra_por_item(&itens, cab.frete_extra);
    let motivo = format!("Compra NF {} · {}", cab.numero, fornecedor);
    let tabelas = config::tabelas_preco(conn)?;
    let mut resumo =
        Lancamento { pecas_novas: 0, itens_estoque: 0, precos_atualizados: 0, parcelas_pagar: 0, encomendas: 0 };
    let mut entraram: Vec<(String, i64)> = Vec::new();
    let ambiguos = codigos_ambiguos(itens.iter().map(|i| (i.codigo_fornecedor.as_str(), i.descricao.as_str())));

    for (i, it) in itens.iter().enumerate() {
        if it.acao == "ignorar" {
            // Achado 8: fecha pendência de identificação exclusiva de item ignorado no lançamento.
            crate::compras::pendencias::fechar_pendencia_item_ignorado(
                conn,
                &cab.fornecedor_id,
                &cab.chave,
                &it.codigo_fornecedor,
                &it.descricao,
            )?;
            continue;
        }
        let pecas = it.pecas();
        let custo = custo_unitario(numeros::soma([it.custo_nota()?, extras[i]])?, pecas);
        let mut detalhes = Vec::new();
        let produto_id = if it.acao == "nova" {
            if it.achado_por == "importacao" {
                if let Some(pid) = &it.produto_id {
                    let estado: Option<(bool, String)> = conn
                        .query_row("SELECT ativo, nome FROM produto WHERE id = ?1", [pid], |l| {
                            Ok((l.get::<_, i64>(0)? != 0, l.get(1)?))
                        })
                        .optional()?;
                    if let Some((ativa, nome)) = estado {
                        if !ativa {
                            return Err(Erro::validacao(format!(
                                "A peça \"{nome}\" cadastrada na importação está desativada. Reative-a ou troque a peça antes de lançar."
                            )));
                        }
                        if !peca_ja_tocada(conn, pid)? {
                            resumo.pecas_novas += 1;
                            detalhes =
                                finalizar_peca_importada(conn, dispositivo, usuario_id, pid, it, custo, &tabelas)?;
                        } else {
                            // A1 / A6 (D-09): peça já movimentada por outra nota ou editada manualmente;
                            // nunca recria e respeita a regra de atualização de preços do gerente.
                            let (d, mudou_preco) = atualizar_custo(
                                conn,
                                dispositivo,
                                usuario_id,
                                pid,
                                custo,
                                cab.atualizar_precos,
                                it.baixar_preco,
                                &tabelas,
                            )?;
                            if mudou_preco {
                                resumo.precos_atualizados += 1;
                            }
                            detalhes = d;
                        }
                        pid.clone()
                    } else {
                        resumo.pecas_novas += 1;
                        let pid = cadastrar_peca(conn, dispositivo, usuario_id, it, custo)?;
                        conn.execute("UPDATE compra_item SET produto_id = ?2 WHERE id = ?1", params![it.id, pid])?;
                        pid
                    }
                } else {
                    resumo.pecas_novas += 1;
                    let pid = cadastrar_peca(conn, dispositivo, usuario_id, it, custo)?;
                    conn.execute("UPDATE compra_item SET produto_id = ?2 WHERE id = ?1", params![it.id, pid])?;
                    pid
                }
            } else {
                resumo.pecas_novas += 1;
                let pid = cadastrar_peca(conn, dispositivo, usuario_id, it, custo)?;
                conn.execute("UPDATE compra_item SET produto_id = ?2 WHERE id = ?1", params![it.id, pid])?;
                pid
            }
        } else {
            let pid = it.produto_id.clone().ok_or_else(|| Erro::Interno("item ligado sem peça".into()))?;
            conn.execute("UPDATE compra_item SET acao = 'ligar', produto_id = ?2 WHERE id = ?1", params![it.id, pid])?;
            let (d, mudou_preco) = atualizar_custo(
                conn,
                dispositivo,
                usuario_id,
                &pid,
                custo,
                cab.atualizar_precos,
                it.baixar_preco,
                &tabelas,
            )?;
            if mudou_preco {
                resumo.precos_atualizados += 1;
            }
            detalhes = d;
            pid
        };
        // Achado 3a/3b: se o usuário corrigiu identificação na pendência, respeita o
        // código e os GTINs corrigidos em vez de regravar os valores antigos do XML.
        let correcao = correcao_da_pendencia(conn, &cab.fornecedor_id, &cab.chave, it, &produto_id)?;
        let pular_gtin_xml = correcao.as_ref().is_some_and(|(_, mudou_gtin)| *mudou_gtin);
        if !pular_gtin_xml {
            produtos::registrar_gtins_xml(conn, &produto_id, &it.gtins_ficha())?;
        }
        crate::compras::pendencias::resolver_pendencias_do_item(
            conn,
            &cab.fornecedor_id,
            &it.codigo_fornecedor,
            &it.descricao,
            &produto_id,
        )?;
        let antes = estoque::saldo(conn, &produto_id)?;
        estoque::lancar(conn, dispositivo, Some(usuario_id), &produto_id, pecas, "compra", &motivo)?;
        detalhes.insert(
            0,
            format!(
                "Estoque: {} → {} (+{})",
                numeros::escrever_quantidade(antes),
                numeros::escrever_quantidade(antes + pecas),
                numeros::escrever_quantidade(pecas)
            ),
        );
        detalhes.push(motivo.clone());
        auditoria::registrar(conn, dispositivo, Some(usuario_id), "compra.entrada", "produto", &produto_id, &detalhes)?;
        resumo.itens_estoque += 1;
        match entraram.iter_mut().find(|e| e.0 == produto_id) {
            Some(e) => e.1 += pecas,
            None => entraram.push((produto_id.clone(), pecas)),
        }
        let (cod_vincular, foi_corrigido) = match &correcao {
            Some((cod_corr, _)) => (cod_corr.as_str(), true),
            None => (it.codigo_fornecedor.as_str(), false),
        };
        let compacto = texto::compactar(cod_vincular);
        if !cab.fornecedor_id.is_empty() && !compacto.is_empty() && (foi_corrigido || !ambiguos.contains(&compacto)) {
            crate::catalogo::codigos::vincular_fornecedor(
                conn,
                &cab.fornecedor_id,
                cod_vincular,
                &produto_id,
                it.fator,
                if foi_corrigido {
                    crate::catalogo::codigos::JaLigado::ManterPecaAtualizarFator
                } else {
                    crate::catalogo::codigos::JaLigado::PassarPecaEFator
                },
            )?;
            produtos::atualizar_indice_busca(conn, &produto_id)?;
        }
    }
    conn.execute(
        "UPDATE compra SET situacao = 'lancada', lancada_em = ?2, lancada_por = ?3 WHERE id = ?1",
        params![id, tempo::agora(), usuario_id],
    )?;
    resumo.parcelas_pagar = crate::pagar::criar_da_compra(conn, dispositivo, usuario_id, id)? as i64;
    resumo.encomendas = crate::encomendas::chegou_na_compra(conn, dispositivo, usuario_id, id, &entraram)?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "compra.lancar",
        "compra",
        id,
        &[format!(
            "{} itens no estoque · {} peças novas · {} preços atualizados",
            resumo.itens_estoque, resumo.pecas_novas, resumo.precos_atualizados
        )],
    )?;
    Ok(resumo)
}

/// Troca o custo da peça e, se pedido, o preço das tabelas que têm % de
/// lucro. Se o custo baixou, o preço só baixa quando `baixar_preco`.
/// Devolve as frases do que mudou e se algum preço mudou.
#[allow(clippy::too_many_arguments)]
fn atualizar_custo(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    produto_id: &str,
    custo: i64,
    atualizar_precos: bool,
    baixar_preco: bool,
    tabelas: &[config::TabelaPreco],
) -> Resultado<(Vec<String>, bool)> {
    let antes: i64 = conn.query_row("SELECT custo FROM produto WHERE id = ?1", [produto_id], |l| l.get(0))?;
    // R5 (D-AUD-4, decisão do proprietário de 25/11/2026): a compra com custo zero (bonificação do fornecedor) não
    // troca o custo da peça, nem o preço, nem o % de lucro. Antes o custo ia a zero e o lucro no DRE ficava maior
    // do que é. A entrada no estoque continua; o registro da compra diz que foi bonificação.
    if custo == 0 && antes > 0 {
        return Ok((
            vec![format!(
                "Bonificação (custo zero): o custo da peça continua R$ {}",
                numeros::escrever_dinheiro(antes)
            )],
            false,
        ));
    }
    let baixou = antes > 0 && custo < antes;
    let atualizar_precos = atualizar_precos && (!baixou || baixar_preco);
    let mut detalhes = Vec::new();
    if antes != custo {
        detalhes.push(format!(
            "Custo: {} → {}",
            if antes > 0 { numeros::escrever_dinheiro(antes) } else { "—".into() },
            numeros::escrever_dinheiro(custo)
        ));
    }
    let mut mudou_preco = false;
    if atualizar_precos && custo > 0 {
        let precos: Vec<(i64, i64, Option<i64>)> = conn
            .prepare_cached("SELECT tabela, preco, lucro FROM produto_preco WHERE produto_id = ?1 ORDER BY tabela")?
            .query_map([produto_id], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?
            .collect::<Result<_, _>>()?;
        for (tabela, preco, lucro) in precos {
            let Some(lucro) = lucro else { continue };
            let novo = numeros::preco_por_lucro(custo, lucro);
            if novo != preco {
                conn.execute(
                    "UPDATE produto_preco SET preco = ?3 WHERE produto_id = ?1 AND tabela = ?2",
                    params![produto_id, tabela, novo],
                )?;
                let nome = tabelas.iter().find(|t| t.numero == tabela).map(|t| t.nome.clone()).unwrap_or_default();
                detalhes.push(format!(
                    "Preço {nome}: {} → {}",
                    numeros::escrever_dinheiro(preco),
                    numeros::escrever_dinheiro(novo)
                ));
                mudou_preco = true;
            }
        }
    } else if custo > 0 {
        // Sem atualizar o preço, o % de lucro muda (o preço fica). D-15: a
        // mesma conta do cadastro (`numeros::lucro_por_preco`), que arredonda
        // certo também com o preço abaixo do custo.
        let precos: Vec<(i64, Option<i64>)> = conn
            .prepare_cached("SELECT tabela, preco FROM produto_preco WHERE produto_id = ?1 ORDER BY tabela")?
            .query_map([produto_id], |l| Ok((l.get(0)?, l.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (tabela, preco) in precos {
            conn.execute(
                "UPDATE produto_preco SET lucro = ?3 WHERE produto_id = ?1 AND tabela = ?2",
                params![produto_id, tabela, preco.and_then(|p| numeros::lucro_por_preco(custo, p))],
            )?;
        }
    }
    if !detalhes.is_empty() {
        conn.execute(
            "UPDATE produto SET custo = ?2, atualizado_em = ?3, atualizado_por = ?4, dispositivo = ?5 WHERE id = ?1",
            params![produto_id, custo, tempo::agora(), usuario_id, dispositivo],
        )?;
    }
    Ok((detalhes, mudou_preco))
}

/// T111 (Opção C): finaliza a peça recém-criada na importação ao lançar sua
/// primeira compra (sem movimentos nem edição manual): aplica unidade UN se
/// houve fator > 1, custo real com frete/rateio e preço inicial pela regra
/// padrão (% de lucro), sem acionar D-09 de peça já consolidada.
fn finalizar_peca_importada(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    produto_id: &str,
    it: &ItemBanco,
    custo: i64,
    tabelas: &[config::TabelaPreco],
) -> Resultado<Vec<String>> {
    let (antes, unidade_antes): (i64, String) =
        conn.query_row("SELECT custo, unidade FROM produto WHERE id = ?1", [produto_id], |l| {
            Ok((l.get(0)?, l.get(1)?))
        })?;
    let mut detalhes = Vec::new();
    let mudar_unidade = it.fator != 1000 && unidade_antes != "UN";
    if antes != custo {
        detalhes.push(format!(
            "Custo: {} → {}",
            if antes > 0 { numeros::escrever_dinheiro(antes) } else { "—".into() },
            numeros::escrever_dinheiro(custo)
        ));
    }
    if custo > 0 {
        let precos: Vec<(i64, i64, Option<i64>)> = conn
            .prepare_cached("SELECT tabela, preco, lucro FROM produto_preco WHERE produto_id = ?1 ORDER BY tabela")?
            .query_map([produto_id], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?
            .collect::<Result<_, _>>()?;
        for (tabela, preco, lucro) in precos {
            let Some(lucro) = lucro else { continue };
            let novo = numeros::preco_por_lucro(custo, lucro);
            if novo != preco {
                conn.execute(
                    "UPDATE produto_preco SET preco = ?3 WHERE produto_id = ?1 AND tabela = ?2",
                    params![produto_id, tabela, novo],
                )?;
                let nome = tabelas.iter().find(|t| t.numero == tabela).map(|t| t.nome.clone()).unwrap_or_default();
                detalhes.push(format!(
                    "Preço {nome}: {} → {}",
                    numeros::escrever_dinheiro(preco),
                    numeros::escrever_dinheiro(novo)
                ));
            }
        }
    }
    if mudar_unidade || antes != custo || !detalhes.is_empty() {
        conn.execute(
            "UPDATE produto SET custo = ?2, unidade = CASE WHEN ?6 THEN 'UN' ELSE unidade END,
                    atualizado_em = ?3, atualizado_por = ?4, dispositivo = ?5
              WHERE id = ?1",
            params![produto_id, custo, tempo::agora(), usuario_id, dispositivo, mudar_unidade],
        )?;
    }
    Ok(detalhes)
}

/// Cadastra uma peça nova com os dados do item da nota.
fn cadastrar_peca(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    it: &ItemBanco,
    custo: i64,
) -> Resultado<String> {
    let mut f = produtos::ficha_nova(conn)?;
    f.nome = texto::limpar(&it.descricao).chars().take(120).collect();
    let barras_livre = !it.gtin.is_empty()
        && gtin::gtin_valido(&it.gtin)
        && matches!(
            crate::importar::peca_por_gtins_inequivoco(conn, &[], &it.gtin)?,
            crate::catalogo::codigos::BuscaId::Nenhum
        );
    if barras_livre {
        f.codigo_barras = it.gtin.clone();
    }
    f.gtins = it.gtins_ficha();
    f.unidade = if it.fator == 1000 { crate::unidades::de_fora(&it.unidade) } else { "UN".into() };
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
    if custo > 0 {
        f.custo = numeros::escrever_dinheiro(custo);
        if let Some(p) = f.precos.iter_mut().find(|p| p.tabela == 1) {
            if p.lucro.is_empty() {
                p.lucro = "100".into();
            }
        }
    } else {
        // Achado 1 (Crítico): bonificação/brinde (custo 0) nasce com preço na hora da venda.
        f.preco_aberto = true;
    }
    let id = produtos::salvar(conn, dispositivo, usuario_id, &f)?;
    produtos::registrar_gtins_xml(conn, &id, &f.gtins)?;
    Ok(id)
}

/// Desfaz a entrada no estoque. O custo e os preços não voltam (podem ter
/// mudado de novo depois).
pub fn estornar(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str, motivo: &str) -> Resultado<()> {
    crate::sync::exigir_editavel(conn, "compra", id)?;
    crate::sync::exigir_filhos_editaveis(conn, "compra_item", "compra_id", id)?;
    let cab = cabecalho(conn, id)?;
    if cab.situacao != "lancada" {
        return Err(Erro::validacao("Só dá para estornar uma compra lançada."));
    }
    let motivo = texto::limpar(motivo);
    if motivo.is_empty() {
        return Err(Erro::campo("motivo", "Diga o motivo do estorno."));
    }
    // 0915-L (R14): as contas a pagar da compra conferidas antes de mudar qualquer coisa.
    crate::sync::exigir_filhos_prontos(conn, "conta_pagar", "compra_id", id)?;
    crate::pagar::cancelar_da_compra(conn, usuario_id, id)?;
    crate::encomendas::desfazer_da_compra(conn, dispositivo, usuario_id, id)?;
    let itens = itens_banco(conn, id)?;
    for it in &itens {
        if it.acao == "ignorar" {
            continue;
        }
        if let Some(pid) = &it.produto_id {
            let texto_motivo = format!("Estorno da compra NF {} · {motivo}", cab.numero);
            let antes = estoque::saldo(conn, pid)?;
            estoque::lancar(conn, dispositivo, Some(usuario_id), pid, -it.pecas(), "estorno_compra", &texto_motivo)?;
            auditoria::registrar(
                conn,
                dispositivo,
                Some(usuario_id),
                "compra.estornar",
                "produto",
                pid,
                &[
                    format!(
                        "Estoque: {} → {} (−{})",
                        numeros::escrever_quantidade(antes),
                        numeros::escrever_quantidade(antes - it.pecas()),
                        numeros::escrever_quantidade(it.pecas())
                    ),
                    texto_motivo,
                ],
            )?;
        }
    }
    conn.execute("UPDATE compra SET situacao = 'estornada' WHERE id = ?1", [id])?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "compra.estornar",
        "compra",
        id,
        &[format!("Motivo: {motivo}")],
    )?;
    Ok(())
}

/// Apaga uma compra em rascunho (nada foi para o estoque). O-4: a auditoria
/// dela fica (quem importou e quando) e ganha uma linha dizendo que o
/// rascunho foi descartado; o rascunho não vira compra lançada.
pub fn excluir_rascunho(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str) -> Resultado<()> {
    // Achado 8a: bloqueia descarte se a compra ou algum item tiver referência inconsistente no sync.
    crate::sync::exigir_editavel(conn, "compra", id)?;
    crate::sync::exigir_filhos_editaveis(conn, "compra_item", "compra_id", id)?;
    let cab = cabecalho(conn, id)?;
    if cab.situacao != "rascunho" {
        return Err(Erro::validacao("Só dá para descartar uma compra que ainda não foi lançada."));
    }
    // T111 (A5): coleta antes do DELETE as peças ligadas neste rascunho; após
    // apagar a compra, remove somente as que nasceram de importação de compra e
    // ficaram órfãs (sem outros rascunhos, movimentos, vendas ou edições).
    let ligadas: Vec<(String, String)> = conn
        .prepare(
            "SELECT produto_id, codigo_fornecedor FROM compra_item WHERE compra_id = ?1 AND produto_id IS NOT NULL",
        )?
        .query_map([id], |l| Ok((l.get(0)?, l.get(1)?)))?
        .collect::<Result<_, _>>()?;
    conn.execute("DELETE FROM compra WHERE id = ?1", [id])?;
    // Achado 8b: remove pendências abertas exclusivas da nota descartada.
    crate::compras::pendencias::limpar_pendencias_da_nota_descartada(conn, &cab.chave)?;
    let mut vistos = HashSet::new();
    for (pid, cod_forn) in ligadas {
        if vistos.insert(pid.clone()) && crate::sync::exigir_editavel(conn, "produto", &pid).is_ok() {
            produtos::remover_provisorio_se_orfao(conn, dispositivo, usuario_id, &pid, &cab.fornecedor_id, &cod_forn)?;
        }
    }
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "compra.descartar",
        "compra",
        id,
        &[format!("Rascunho da NF {} descartado (nada foi para o estoque)", cab.numero)],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Lista.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompraResumo {
    pub id: String,
    pub fornecedor: String,
    pub numero: String,
    pub emissao: String,
    pub itens: i64,
    pub pendentes: i64,
    pub total: String,
    pub situacao: String,
    pub autorizacao: String,
    pub pode_lancar: bool,
}

pub fn listar(conn: &Connection, busca: &str, limite: i64) -> Resultado<Vec<CompraResumo>> {
    listar_filtrada(conn, busca, "", limite)
}

pub fn listar_filtrada(conn: &Connection, busca: &str, filtro: &str, limite: i64) -> Resultado<Vec<CompraResumo>> {
    let clausula_filtro = match filtro.trim() {
        "" | "todas" => "1 = 1",
        "aguardando_autorizacao" => {
            "c.situacao = 'rascunho' AND c.xml <> '' \
             AND c.autorizacao NOT IN ('confirmada_gerente', 'comprovada_sefaz', 'anterior', 'negada')"
        }
        "rascunho" => "c.situacao = 'rascunho'",
        "lancada" => "c.situacao = 'lancada'",
        "negada" => "c.situacao = 'rascunho' AND c.xml <> '' AND c.autorizacao = 'negada'",
        _ => return Err(Erro::validacao("Filtro de compras inválido.")),
    };
    let termo = format!("%{}%", texto::normalizar(busca));
    let sql = format!(
        "SELECT c.id, COALESCE(p.nome, ''), c.numero, c.emissao,
                (SELECT COUNT(*) FROM compra_item i WHERE i.compra_id = c.id),
                (SELECT COUNT(*) FROM compra_item i WHERE i.compra_id = c.id AND i.acao = ''),
                c.total, c.situacao, c.autorizacao, c.xml <> ''
           FROM compra c LEFT JOIN pessoa p ON p.id = c.fornecedor_id
          WHERE ({clausula_filtro})
            AND (?1 = '%%' OR lower(COALESCE(p.busca, '')) LIKE ?1 OR c.numero LIKE ?1 OR c.chave LIKE ?1
                 OR EXISTS (SELECT 1 FROM compra_item i WHERE i.compra_id = c.id AND normalizar(i.descricao) LIKE ?1))
          ORDER BY CASE c.situacao WHEN 'rascunho' THEN 0 ELSE 1 END, c.emissao DESC, c.criado_em DESC
          LIMIT ?2"
    );
    let mut c = conn.prepare(&sql)?;
    let linhas = c.query_map(params![termo, limite.clamp(1, 5000)], |l| {
        let autorizacao: String = l.get(8)?;
        let tem_xml: bool = l.get(9)?;
        let pode_lancar = autorizacao_permite_lancar(tem_xml, &autorizacao);
        Ok(CompraResumo {
            id: l.get(0)?,
            fornecedor: l.get(1)?,
            numero: l.get(2)?,
            emissao: fiscal::data_br(&l.get::<_, String>(3)?),
            itens: l.get(4)?,
            pendentes: l.get(5)?,
            total: numeros::escrever_dinheiro(l.get(6)?),
            situacao: l.get(7)?,
            autorizacao,
            pode_lancar,
        })
    })?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

/// Só nos testes: libera uma compra como faria o gerente (D7), pelo mesmo
/// caminho do programa (`registrar_conferencia`).
#[cfg(test)]
pub(crate) fn liberar_para_teste(c: &Connection, d: &str, id: &str) {
    let g: String = match c
        .query_row("SELECT id FROM usuario WHERE gerente = 1 AND ativo = 1 LIMIT 1", [], |l| l.get(0))
        .optional()
        .unwrap()
    {
        Some(g) => g,
        None => {
            let g = crate::usuarios::criar(c, d, "Gerente de teste", "Gerente-teste-123", true).unwrap().id;
            crate::usuarios::definir_gerente(c, &g, true).unwrap();
            g
        }
    };
    let agora: String = c.query_row("SELECT strftime('%d/%m/%Y %H:%M', 'now', 'localtime')", [], |l| l.get(0)).unwrap();
    let conf = Conferencia {
        id: id.into(),
        situacao: "autorizada".into(),
        protocolo: "135260000000001".into(),
        conferido_em: agora,
    };
    registrar_conferencia(c, d, &g, &conf).unwrap();
}

// ------------------------------------------------------------- comandos das telas

pub use comandos::COMANDOS;

/// Os comandos das compras (notas de entrada) (etapa 3 da modularização: vieram do `match` do
/// `api.rs` sem mudar nome, entrada, resposta nem permissão).
mod comandos {
    use serde_json::{json, Value};

    use crate::despacho::{ler, marcar_feita, Classe, Comando, Estorno, Id, Pedido};
    use crate::erro::{Erro, Resultado};
    use crate::nfe_xml;
    use crate::{arquivos, compras, importar};

    #[derive(Default, serde::Deserialize)]
    #[serde(rename_all = "camelCase", default)]
    struct EntradaComprasListar {
        texto: String,
        limite: Option<i64>,
        filtro: String,
    }

    #[derive(Default, serde::Deserialize)]
    #[serde(rename_all = "camelCase", default)]
    struct EntradaCompraImportar {
        caminho: String,
        caminhos: Vec<String>,
    }

    pub const COMANDOS: &[Comando] = &[
        Comando { nome: "compras_contagem", classe: Classe::Leitura, funcao: compras_contagem },
        Comando { nome: "compras_listar", classe: Classe::Leitura, funcao: compras_listar },
        Comando { nome: "compra_importar", classe: Classe::Caixa, funcao: compra_importar },
        Comando { nome: "compra_abrir", classe: Classe::Leitura, funcao: compra_abrir },
        Comando { nome: "compra_atualizar", classe: Classe::Caixa, funcao: compra_atualizar },
        Comando { nome: "compra_lancar", classe: Classe::Caixa, funcao: compra_lancar },
        Comando { nome: "compra_estornar", classe: Classe::Gerente("estornar"), funcao: compra_estornar },
        Comando { nome: "compra_descartar", classe: Classe::Caixa, funcao: compra_descartar },
        // T-2 (a): a prévia lê os arquivos da importação, que é do gerente;
        // só com a senha (a da tela, DP-52 (b)) ou quem a loja liberou.
        Comando {
            nome: "importar_previa",
            classe: Classe::ConsultaDoGerente("configuracoes"),
            funcao: importar_previa,
        },
        Comando { nome: "importar_cadastros", classe: Classe::Gerente("configuracoes"), funcao: importar_cadastros },
    ];

    fn compras_contagem(p: &mut Pedido, _dados: Value) -> Resultado<Value> {
        Ok(json!(compras::contagem(p.conn)?))
    }

    fn compras_listar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: EntradaComprasListar = ler(dados)?;
        Ok(json!(compras::listar_filtrada(p.conn, &d.texto, &d.filtro, d.limite.unwrap_or(300))?))
    }

    // Um arquivo escolhido: um XML, um .zip ou .rar com várias notas,
    // ou uma pasta. Cada nota vira uma compra; o que falhar vem em "erros".
    //
    // T108 (texto R do proprietário, 21/11/2026): num pacote (pasta, .zip, .rar), a nota que já está em Compras é
    // contada à parte ("jaEmCompras"), não é erro; o XML sozinho repetido continua recusado. O que é de Peças (o
    // fornecedor; o item sem peça vai para as pendências que a tela de Peças mostra) segue como antes.
    fn compra_importar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: EntradaCompraImportar = ler(dados)?;
        // Achado 2: aceita tanto `{ caminho }` quanto `{ caminhos: [...] }` (seleção múltipla/arraste)
        // para que `analisar_lote` e `pacote` vejam o lote inteiro de uma vez.
        let entradas: Vec<String> = if !d.caminhos.is_empty() {
            d.caminhos.into_iter().filter(|c| !c.trim().is_empty()).collect()
        } else if !d.caminho.trim().is_empty() {
            vec![d.caminho]
        } else {
            Vec::new()
        };
        if entradas.is_empty() {
            return Err(Erro::validacao("Escolha o arquivo XML da nota."));
        }
        // 0.9.17: sem o CNPJ da loja (ou com um inválido) não importa:
        // a tela leva a Configurações › Loja e volta com os arquivos.
        let loja_cnpj = crate::requisitos::exigir_cnpj_da_loja(p.conn, crate::requisitos::PARA_COMPRAS)?;
        let lista = arquivos::xmls(&entradas)?;
        // T111 (A4): pré-analisa o pacote inteiro para detectar códigos e GTINs ambíguos entre notas.
        let lote =
            compras::analisar_lote(&loja_cnpj, lista.iter().filter(|x| x.erro.is_empty()).map(|x| x.texto.as_str()));
        // T108 / T111: múltiplos arquivos, pasta ou arquivo compactado são tratados como pacote.
        let pacote = entradas.len() > 1
            || lista.len() > 1
            || lista.iter().any(|x| x.nome.contains(" › "))
            // N4 da 2ª conferência: o caminho como o `arquivos` o lê (aceita `file://` e `%xx`).
            || entradas.iter().any(|c| arquivos::caminho_local(c).is_dir());
        let mut fichas = Vec::new();
        let mut erros = Vec::new();
        // Notas emitidas para outro CNPJ: não entram; a tela mostra a
        // janela de conflito para conferir ou descartar.
        let mut conflitos = Vec::new();
        let mut ja_em_compras = 0i64;
        let mut pecas_cadastradas = 0i64;
        let mut pendencias_identificacao = 0i64;
        for x in lista {
            if !x.erro.is_empty() {
                erros.push(format!("{}: {}", x.nome, x.erro));
                continue;
            }
            let tx = p.conn.transaction()?;
            let criada = compras::verificar_para_compra(&tx, &x.texto).and_then(|v| {
                match compras::ja_importada(&tx, &v.nota)? {
                    Some(e) => Err(e),
                    None => compras::criar_compra_com_opcoes(&tx, p.disp, p.usuario_id, &x.texto, &v, true, &lote),
                }
            });
            match criada {
                Ok(c) => {
                    tx.commit()?;
                    pecas_cadastradas += c.pecas_cadastradas;
                    pendencias_identificacao += c.pendencias_identificacao;
                    // T110-N8: em pacote (pasta, .zip/.rar ou seleção múltipla), devolve só o resumo
                    // enxuto (`id` + `numero`) para evitar carregar centenas/milhares de fichas completas
                    // na memória de uma só vez.
                    if pacote {
                        fichas.push(json!({ "id": c.id, "numero": c.numero }));
                    } else {
                        fichas.push(json!(compras::ficha(p.conn, &c.id)?));
                    }
                }
                // Dentro de um pacote, eventos (cancelamento, carta de
                // correção) e outros XMLs que não são nota são pulados. T081
                // (OPUS-C-3): o XML que nem abre (a nota cortada ao meio, o
                // arquivo estragado) não é pulado: vem em `erros`.
                Err(e)
                    if pacote
                        && ["não é de uma NF-e", "resumo ou um evento", "CT-e"]
                            .iter()
                            .any(|t| e.to_string().contains(t)) => {}
                Err(e) if e.nome_do_campo() == Some(compras::CAMPO_OUTRO_CNPJ) => {
                    conflitos.push(compras::conflito_cnpj(&tx, &x.nome, &x.texto)?);
                }
                // T108: a nota que já está em Compras, num pacote, é contada (não é erro).
                Err(e) if pacote && e.nome_do_campo() == Some(compras::CAMPO_JA_IMPORTADA) => ja_em_compras += 1,
                // O-2 (7.3): como no "Trazer cadastros", o erro do banco vai curto
                // para a tela; o detalhe (o texto do SQLite) fica no registro técnico.
                Err(e) => {
                    if matches!(e, crate::erro::Erro::Banco(_) | crate::erro::Erro::Interno(_)) {
                        crate::registro::anotar(&format!("compra_importar: {e}"));
                    }
                    erros.push(format!("{}: {}", x.nome, e.para_a_tela()));
                }
            }
        }
        if fichas.is_empty() && erros.is_empty() && conflitos.is_empty() && ja_em_compras == 0 {
            return Err(Erro::validacao("Não achei nenhuma NF-e nesse arquivo."));
        }
        let mut resposta = json!({ "compras": fichas, "erros": erros, "conflitos": conflitos });
        // T108 / T111: só quando > 0 (a resposta de antes fica igual).
        if ja_em_compras > 0 {
            resposta["jaEmCompras"] = json!(ja_em_compras);
        }
        if pecas_cadastradas > 0 {
            resposta["pecasCadastradas"] = json!(pecas_cadastradas);
        }
        if pendencias_identificacao > 0 {
            resposta["pendenciasIdentificacao"] = json!(pendencias_identificacao);
        }
        // R0, EXCEÇÃO documentada: um pacote grava uma transação por nota; a marca vai numa transação
        // própria no fim. Cada nota já é recusada se repetida (a chave de acesso de 44 dígitos, acima).
        // T059-R9: notas recusadas por outro CNPJ nunca são gravadas no banco, nem por 30 dias em
        // `operacao_feita` (se só houve conflitos/erros, não cria linha; em lote misto, grava sem o
        // payload de `conflitos`), honrando literalmente "A nota não foi importada e nada foi gravado".
        if !fichas.is_empty() || pecas_cadastradas > 0 || pendencias_identificacao > 0 {
            let mut resposta_registro = resposta.clone();
            resposta_registro["conflitos"] = json!([]);
            let tx = p.conn.transaction()?;
            marcar_feita(&tx, p.operacao.as_ref(), &resposta_registro)?;
            tx.commit()?;
        }
        Ok(resposta)
    }

    // T108 (texto R): o "Trazer cadastros pelo XML" (Peças), aqui porque também leva as notas de compra a Compras
    // (só este módulo conhece as regras de Compras; o `importar` não depende dele). Mesmo nome, entrada, resposta e
    // permissão de antes (vieram do `importar::comandos`).
    fn importar_previa(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        // P-5: sem nenhuma opção no pedido, a prévia supõe todas marcadas.
        let com_opcoes = ["clientes", "fornecedores", "pecasLoja", "pecasFornecedor", "compras"]
            .iter()
            .any(|k| dados.get(k).is_some());
        let mut d: importar::Entrada = ler(dados)?;
        if !com_opcoes {
            d = importar::Entrada { arquivos: d.arquivos, ..importar::Entrada::todas() };
        }
        let classificar = compras::classificar_para_compras;
        let criar = |_: &rusqlite::Connection, _: &str, _: &nfe_xml::NotaVerificada| -> Resultado<()> {
            Err(Erro::Interno("a prévia não grava".into()))
        };
        let gancho = importar::GanchoCompras { classificar: &classificar, criar: &criar };
        Ok(json!(importar::previa_com_compras(p.conn, &d, Some(&gancho))?))
    }

    fn importar_cadastros(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: importar::Entrada = ler(dados)?;
        let (disp, usuario) = (p.disp, p.usuario_id);
        let tx = p.conn.transaction()?;
        let classificar = compras::classificar_para_compras;
        let criar = |c: &rusqlite::Connection, xml: &str, v: &nfe_xml::NotaVerificada| -> Resultado<()> {
            compras::criar_compra(c, disp, usuario, xml, v).map(|_| ())
        };
        let gancho = importar::GanchoCompras { classificar: &classificar, criar: &criar };
        let r = importar::importar_com_compras(&tx, disp, usuario, &d, Some(&gancho))?;
        let resposta = json!(r);
        marcar_feita(&tx, p.operacao.as_ref(), &resposta)?;
        tx.commit()?;
        Ok(resposta)
    }

    fn compra_abrir(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: Id = ler(dados)?;
        Ok(json!(compras::ficha(p.conn, &d.id)?))
    }

    fn compra_atualizar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: compras::CompraEntrada = ler(dados)?;
        let tx = p.conn.transaction()?;
        let ligadas_antes = compras::ligados_no_rascunho(&tx, &d.id)?;
        compras::atualizar(&tx, &d)?;
        compras::limpar_provisorios_removidos(&tx, p.disp, p.usuario_id, &d.id, &ligadas_antes)?;
        tx.commit()?;
        Ok(json!(compras::ficha(p.conn, &d.id)?))
    }

    fn compra_lancar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: compras::CompraEntrada = ler(dados)?;
        // 0915-N / D-09: a compra que muda o preço de venda de uma
        // peça já cadastrada ("os preços seguem o % de lucro") é do
        // gerente, como no cadastro. Confere antes (sem efeito) e
        // decide depois de saber se algum preço mudou: sem a senha,
        // nada é lançado (a transação volta inteira).
        let sem_precos = p.exigir("precos").err();
        let tx = p.conn.transaction()?;
        let ligadas_antes = compras::ligados_no_rascunho(&tx, &d.id)?;
        compras::atualizar(&tx, &d)?;
        compras::limpar_provisorios_removidos(&tx, p.disp, p.usuario_id, &d.id, &ligadas_antes)?;
        let r = compras::lancar(&tx, p.disp, p.usuario_id, &d.id)?;
        if r.precos_atualizados > 0 {
            if let Some(e) = sem_precos {
                return Err(e);
            }
        }
        let resposta = json!({ "resumo": r, "compra": compras::ficha(&tx, &d.id)? });
        marcar_feita(&tx, p.operacao.as_ref(), &resposta)?;
        tx.commit()?;
        Ok(resposta)
    }

    fn compra_estornar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: Estorno = ler(dados)?;
        let tx = p.conn.transaction()?;
        compras::estornar(&tx, p.disp, p.usuario_id, &d.id, &d.motivo)?;
        let resposta = json!(compras::ficha(&tx, &d.id)?);
        marcar_feita(&tx, p.operacao.as_ref(), &resposta)?;
        tx.commit()?;
        Ok(resposta)
    }

    fn compra_descartar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: Id = ler(dados)?;
        let tx = p.conn.transaction()?;
        compras::excluir_rascunho(&tx, p.disp, p.usuario_id, &d.id)?;
        tx.commit()?;
        Ok(Value::Null)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::usuarios;

    /// R-10: pesos enormes no rateio não estouram e não perdem centavo.
    #[test]
    fn rateio_com_pesos_enormes() {
        let partes = ratear(4000, &[i64::MAX, i64::MAX, 1]);
        assert_eq!(partes.iter().sum::<i64>(), 4000);
        let partes = ratear(numeros::SOMA_MAXIMA, &vec![i64::MAX / 2; 1000]);
        assert_eq!(partes.iter().map(|p| *p as i128).sum::<i128>(), numeros::SOMA_MAXIMA as i128);
    }

    #[test]
    fn so_entra_nota_autorizada_de_compra_para_a_loja() {
        let b = Banco::em_memoria().unwrap();
        let ex = nfe_xml::EXEMPLO;
        let ler = |x: &str| nfe_xml::ler(x).unwrap();
        assert_eq!(ler(ex).incoerencia, "");
        assert!(conferir_nota(&b.conn, &ler(ex)).is_ok());
        let sem_protocolo = ex.replace("<cStat>100</cStat>", "<cStat>110</cStat>");
        assert!(conferir_nota(&b.conn, &ler(&sem_protocolo)).unwrap_err().to_string().contains("autorização"));
        let entrada = ex.replace("<tpNF>1</tpNF>", "<tpNF>0</tpNF>");
        assert!(conferir_nota(&b.conn, &ler(&entrada)).unwrap_err().to_string().contains("entrada"));
        let devolucao = ex.replace("<tpNF>1</tpNF>", "<tpNF>1</tpNF><finNFe>4</finNFe>");
        // XML alterado ou montado: o que não bate é recusado (C-04).
        for (troca, por, motivo) in [
            ("<nNF>12345</nNF>", "<nNF>12346</nNF>", "número"),
            ("<CNPJ>11222333000181</CNPJ>", "<CNPJ>11222333000262</CNPJ>", "CNPJ do fornecedor"),
            (
                "<chNFe>35261011222333000181550010000123451000000014",
                "<chNFe>35261011222333000181550010000123461000000010",
                "outra nota",
            ),
            ("<digVal>", "<digVal>AAAA", "resumo"),
            ("<nProt>135260000000001</nProt>", "", "sem número"),
            ("1000000014\" versao", "1000000015\" versao", "dígito"),
        ] {
            let x = ex.replace(troca, por);
            assert_ne!(x, ex, "{troca}");
            let e = conferir_nota(&b.conn, &ler(&x)).unwrap_err().to_string();
            assert!(e.contains("alterado") && e.contains(motivo), "{motivo}: {e}");
        }
        let inicio = ex.find("<Signature").unwrap();
        let fim = ex.find("</Signature>").unwrap() + "</Signature>".len();
        let sem_assinatura = format!("{}{}", &ex[..inicio], &ex[fim..]);
        assert!(conferir_nota(&b.conn, &ler(&sem_assinatura)).unwrap_err().to_string().contains("assinatura"));
        assert_eq!(nfe_xml::digito_da_chave("3526101122233300018155001000012345100000001"), Some(4));
        assert!(conferir_nota(&b.conn, &ler(&devolucao)).unwrap_err().to_string().contains("devolução"));
        // CNPJ da loja preenchido e diferente do destinatário.
        config::gravar(&b.conn, "loja_cnpj", "11222333000181").unwrap();
        assert!(conferir_nota(&b.conn, &ler(ex)).unwrap_err().to_string().contains("outro CNPJ"));
        config::gravar(&b.conn, "loja_cnpj", "11444777000161").unwrap();
        assert!(conferir_nota(&b.conn, &ler(ex)).is_ok());
    }

    /// A-20 (PEND-09): o antigo "limite conhecido" agora é RECUSADO. 10 peças
    /// a R$ 52,00 viram 1000 peças a R$ 1,00, mantendo a chave, o
    /// `DigestValue`, o `digVal` e o protocolo: a coerência entre campos não
    /// vê a troca, mas o resumo do conteúdo assinado vê. Nada é gravado.
    #[test]
    fn a20_xml_adulterado_com_resumos_mantidos_e_recusado_sem_gravar_nada() {
        let (b, u) = base();
        let original = nfe_xml::EXEMPLO;
        let adulterado = original.replace("<qCom>10.0000</qCom>", "<qCom>1000.0000</qCom>").replace(
            "<vUnCom>52.0000000000</vUnCom><vProd>520.00</vProd>",
            "<vUnCom>1.0000000000</vUnCom><vProd>1000.00</vProd>",
        );
        assert_ne!(adulterado, original);
        // A coerência entre campos continua sem ver a troca (por isso não é prova).
        assert_eq!(nfe_xml::ler(&adulterado).unwrap().incoerencia, "");
        let antes = retrato(&b.conn);
        let e = importar_xml(&b.conn, &b.dispositivo, &u, &adulterado).unwrap_err().to_string();
        assert!(e.contains("alterada depois da assinatura"), "{e}");
        assert_eq!(retrato(&b.conn), antes, "nada pode ser gravado");
    }

    /// Contagem de tudo que uma importação ou um lançamento poderia mudar.
    fn retrato(c: &Connection) -> Vec<(String, i64)> {
        let mut v = Vec::new();
        for t in
            ["compra", "compra_item", "pessoa", "produto", "estoque_movimento", "conta_pagar", "auditoria", "alteracao"]
        {
            v.push((t.to_string(), c.query_row(&format!("SELECT count(*) FROM {t}"), [], |l| l.get(0)).unwrap()));
        }
        v.push(("custo".into(), c.query_row("SELECT COALESCE(sum(custo), 0) FROM produto", [], |l| l.get(0)).unwrap()));
        v.push((
            "estoque".into(),
            c.query_row("SELECT COALESCE(sum(quantidade), 0) FROM estoque_movimento", [], |l| l.get(0)).unwrap(),
        ));
        v
    }

    fn fixture(nome: &str) -> String {
        std::fs::read_to_string(format!("{}/tests/pki/xml/{nome}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    /// A-24: cada nota recusada pelo verificador não grava NADA (nem o
    /// fornecedor, que antes era cadastrado logo no começo da importação).
    #[test]
    fn a24_atomicidade_de_toda_recusa_da_assinatura() {
        let (b, u) = base();
        let recusadas = [
            "ku_ausente.xml",
            "ku_nao_critica.xml",
            "ku_sem_ds.xml",
            "ku_sem_nr.xml",
            "eku_ausente.xml",
            "eku_so_servidor.xml",
            "politica_a4.xml",
            "politica_ausente.xml",
            "politica_ssl.xml",
            "folha_ca.xml",
            "cnpj_outra_raiz.xml",
            "sem_cnpj.xml",
            "vencido_antes_emissao.xml",
            "comeca_depois_emissao.xml",
            "vence_antes_recebimento.xml",
            "rsa1024.xml",
            "homologacao.xml",
            "autoassinado.xml",
            "raiz_falsa_mesmo_nome.xml",
            "ac_sem_keycertsign.xml",
            "ac_vence_no_recebimento.xml",
            "raiz_vence_no_recebimento.xml",
            "certificado_trocado.xml",
            "adulterada_reassinada_propria.xml",
            "cpf_outro.xml",
        ];
        let antes = retrato(&b.conn);
        for nome in recusadas {
            let e = importar_xml(&b.conn, &b.dispositivo, &u, &fixture(nome));
            assert!(e.is_err(), "{nome} deveria ser recusada");
            assert_eq!(retrato(&b.conn), antes, "{nome} gravou algo");
        }
        // Malformados também.
        for x in ["", "<nfeProc/>", "lixo", &nfe_xml::EXEMPLO[..800]] {
            assert!(importar_xml(&b.conn, &b.dispositivo, &u, x).is_err());
            assert_eq!(retrato(&b.conn), antes);
        }
    }

    /// A-26: nota válida vira rascunho verificado, aguardando a autorização.
    #[test]
    fn a26_nota_valida_vira_rascunho_verificado_aguardando_autorizacao() {
        let (b, u) = base();
        let id = importar_xml(&b.conn, &b.dispositivo, &u, nfe_xml::EXEMPLO).unwrap();
        let f = ficha(&b.conn, &id).unwrap();
        assert_eq!(f.verificacao, "verificada");
        assert_eq!(f.autorizacao, "aguardando");
        assert!(!f.pode_lancar);
        assert!(f.assinatura_texto.contains("conferida"), "{}", f.assinatura_texto);
        assert!(f.autorizacao_texto.contains("não comprovada"), "{}", f.autorizacao_texto);
        let prova: String =
            b.conn.query_row("SELECT xml_prova FROM compra WHERE id = ?1", [&id], |l| l.get(0)).unwrap();
        assert!(prova.contains("11222333000181") && prova.contains("AC de TESTE"), "{prova}");
    }

    /// PEND-19: nota de fornecedor com CNPJ alfanumérico entra de ponta a
    /// ponta: verificação, fornecedor cadastrado com letras, conferência e lançamento.
    #[test]
    fn pend19_compra_de_fornecedor_com_cnpj_alfanumerico() {
        let (b, u) = base();
        let c = &b.conn;
        let (g, _) = gerente_e_operador(&b);
        let id = importar_xml(c, &b.dispositivo, &u, &fixture("alfa_ok.xml")).unwrap();
        let f = ficha(c, &id).unwrap();
        assert_eq!(f.verificacao, "verificada");
        assert_eq!(f.chave, "35261012ABC34501DE35550010000123451000000015");
        let doc: String = c
            .query_row(
                "SELECT p.documento FROM compra k JOIN pessoa p ON p.id = k.fornecedor_id WHERE k.id = ?1",
                [&id],
                |l| l.get(0),
            )
            .unwrap();
        assert_eq!(doc, "12ABC34501DE35");
        assert_eq!(crate::documento::escrever(&doc), "12.ABC.345/01DE-35");
        // A mesma nota de novo é reconhecida pela chave com letras.
        assert!(importar_xml(c, &b.dispositivo, &u, &fixture("alfa_ok.xml")).is_err());
        for it in f.itens {
            c.execute("UPDATE compra_item SET acao = 'nova' WHERE id = ?1", [&it.id]).unwrap();
        }
        registrar_conferencia(c, &b.dispositivo, &g, &conferencia(&id, "autorizada", "135260000000001", &agora_br(c)))
            .unwrap();
        lancar(c, &b.dispositivo, &u, &id).unwrap();
        // Outra raiz no certificado: recusada sem gravar nada.
        let antes = retrato(c);
        assert!(importar_xml(c, &b.dispositivo, &u, &fixture("alfa_cert_outra_raiz.xml")).is_err());
        assert_eq!(retrato(c), antes);
    }

    fn gerente_e_operador(b: &Banco) -> (String, String) {
        let g = usuarios::criar(&b.conn, &b.dispositivo, "Gerente Ana", "Ana-senha-12345", true).unwrap();
        usuarios::definir_gerente(&b.conn, &g.id, true).unwrap();
        let o = usuarios::criar(&b.conn, &b.dispositivo, "Operador Bia", "Bia-senha-1234", true).unwrap();
        (g.id, o.id)
    }

    fn conferencia(id: &str, situacao: &str, protocolo: &str, quando: &str) -> Conferencia {
        Conferencia {
            id: id.into(),
            situacao: situacao.into(),
            protocolo: protocolo.into(),
            conferido_em: quando.into(),
        }
    }

    fn agora_br(c: &Connection) -> String {
        c.query_row("SELECT strftime('%d/%m/%Y %H:%M', 'now', 'localtime')", [], |l| l.get(0)).unwrap()
    }

    /// A-34, A-35: `lancar` é a porta única e respeita a autorização.
    #[test]
    fn a34_a35_lancar_exige_autorizacao_comprovada_e_negada_nunca_libera() {
        let (b, u) = base();
        let c = &b.conn;
        let (g, _) = gerente_e_operador(&b);
        let id = importar_xml(c, &b.dispositivo, &u, nfe_xml::EXEMPLO).unwrap();
        for it in ficha(c, &id).unwrap().itens {
            c.execute("UPDATE compra_item SET acao = 'nova' WHERE id = ?1", [&it.id]).unwrap();
        }
        let antes = retrato(c);
        let e = lancar(c, &b.dispositivo, &u, &id).unwrap_err().to_string();
        assert!(e.contains("autorização desta nota pela SEFAZ ainda não foi comprovada"), "{e}");
        assert_eq!(retrato(c), antes, "lançamento recusado não muda estoque, custo nem financeiro");
        // Rejeitada no portal: negada para sempre.
        registrar_conferencia(c, &b.dispositivo, &g, &conferencia(&id, "cancelada", "", &agora_br(c))).unwrap();
        assert_eq!(ficha(c, &id).unwrap().autorizacao, "negada");
        let e = lancar(c, &b.dispositivo, &u, &id).unwrap_err().to_string();
        assert!(e.contains("não pode entrar no estoque"), "{e}");
        let e = registrar_conferencia(
            c,
            &b.dispositivo,
            &g,
            &conferencia(&id, "autorizada", "135260000000001", &agora_br(c)),
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("não pode ser liberada"), "{e}");
        assert!(lancar(c, &b.dispositivo, &u, &id).is_err());
    }

    /// A-36, A-44, A-45, A-46: a conferência humana do gerente (D7).
    #[test]
    fn a36_a46_conferencia_do_gerente() {
        let (b, u) = base();
        let c = &b.conn;
        let (g, o) = gerente_e_operador(&b);
        let id = importar_xml(c, &b.dispositivo, &u, nfe_xml::EXEMPLO).unwrap();
        let agora = agora_br(c);
        let erro =
            |quem: &str, x: Conferencia| registrar_conferencia(c, &b.dispositivo, quem, &x).unwrap_err().to_string();
        // Operador não registra (A-44).
        assert!(erro(&o, conferencia(&id, "autorizada", "135260000000001", &agora)).contains("Só o gerente"));
        // Protocolo ausente, diferente do XML (A-44).
        assert!(erro(&g, conferencia(&id, "autorizada", "", &agora)).contains("protocolo"));
        assert!(erro(&g, conferencia(&id, "autorizada", "135260000000002", &agora)).contains("não é o desta nota"));
        // Data e hora: ausente, inválida, no futuro (A-44).
        assert!(erro(&g, conferencia(&id, "autorizada", "135260000000001", "")).contains("data e a hora"));
        assert!(
            erro(&g, conferencia(&id, "autorizada", "135260000000001", "31/02/2026 10:00")).contains("data e a hora")
        );
        assert!(erro(&g, conferencia(&id, "autorizada", "135260000000001", "01/01/2099 10:00")).contains("futuro"));
        // Situação desconhecida.
        assert!(erro(&g, conferencia(&id, "talvez", "135260000000001", &agora)).contains("portal mostrou"));
        // Nada disso mudou a compra.
        assert_eq!(ficha(c, &id).unwrap().autorizacao, "aguardando");
        // Certo: libera, com rótulo e auditoria (A-36, A-46).
        registrar_conferencia(c, &b.dispositivo, &g, &conferencia(&id, "autorizada", "1352 6000 0000 001", &agora))
            .unwrap();
        let f = ficha(c, &id).unwrap();
        assert_eq!(f.autorizacao, "confirmada_gerente");
        assert!(f.pode_lancar);
        assert!(f.autorizacao_texto.contains("Conferência humana do gerente Gerente Ana"), "{}", f.autorizacao_texto);
        assert!(f.autorizacao_texto.contains("não é prova automática da SEFAZ"));
        let trilha: String = c
            .query_row(
                "SELECT detalhes FROM auditoria WHERE acao = 'compra.autorizacao' AND entidade_id = ?1",
                [&id],
                |l| l.get(0),
            )
            .unwrap();
        assert!(
            trilha.contains("Conferência humana")
                && trilha.contains("135260000000001")
                && trilha.contains("Gerente Ana"),
            "{trilha}"
        );
        assert!(trilha.contains("35261011222333000181550010000123451000000014"), "{trilha}");
        let dados: String =
            c.query_row("SELECT autorizacao_dados FROM compra WHERE id = ?1", [&id], |l| l.get(0)).unwrap();
        assert!(
            dados.contains(&g) && dados.contains("conferencia_humana") && dados.contains("registrado_em"),
            "{dados}"
        );
        // Já registrada: não registra de novo.
        assert!(erro(&g, conferencia(&id, "cancelada", "", &agora)).contains("já está registrada"));
        // E agora lança (os itens ainda precisam de decisão, como antes).
        for it in &f.itens {
            c.execute("UPDATE compra_item SET acao = 'nova' WHERE id = ?1", [&it.id]).unwrap();
        }
        lancar(c, &b.dispositivo, &u, &id).unwrap();
    }

    /// D5: compras de antes desta versão não são bloqueadas.
    #[test]
    fn d5_compra_anterior_nao_e_bloqueada() {
        let (b, u) = base();
        let c = &b.conn;
        let id = importar_xml(c, &b.dispositivo, &u, nfe_xml::EXEMPLO).unwrap();
        c.execute(
            "UPDATE compra SET xml_verificacao = 'nao_verificada', autorizacao = 'anterior' WHERE id = ?1",
            [&id],
        )
        .unwrap();
        let f = ficha(c, &id).unwrap();
        assert!(f.pode_lancar);
        assert!(f.assinatura_texto.contains("antes da verificação"), "{}", f.assinatura_texto);
    }

    /// T111 (divergência Início ↔ Compras): a listagem devolve `autorizacao` e
    /// `podeLancar` reais e filtra `aguardando_autorizacao` pela mesma regra do aviso.
    #[test]
    fn t111_lista_e_filtro_aguardando_autorizacao_coincidem_com_aviso() {
        let (b, u) = base();
        let c = &b.conn;
        let g = usuarios::criar(c, &b.dispositivo, "Gerente Ana", "Gerente-123", true).unwrap().id;
        usuarios::definir_gerente(c, &g, true).unwrap();
        let id1 = importar_xml(c, &b.dispositivo, &u, nfe_xml::EXEMPLO).unwrap();
        // Resolve todos os itens da nota 1 (pendentes == 0), mantendo autorizacao = 'aguardando':
        // a lista NÃO pode marcar podeLancar = true.
        c.execute("UPDATE compra_item SET acao = 'nova' WHERE compra_id = ?1", [&id1]).unwrap();
        let todas = listar(c, "", 300).unwrap();
        assert_eq!(todas.len(), 1);
        assert_eq!(todas[0].pendentes, 0);
        assert_eq!(todas[0].autorizacao, "aguardando");
        assert!(!todas[0].pode_lancar, "sem conferência, não pode aparecer pronta para lançar");
        assert_eq!(contar_aguardando_autorizacao(c).unwrap(), 1);
        let ct1 = contagem(c).unwrap();
        assert_eq!((ct1.aguardando_autorizacao, ct1.negadas), (1, 0));
        assert_eq!(unica_aguardando_autorizacao(c).unwrap().as_deref(), Some(id1.as_str()));
        let filtradas = listar_filtrada(c, "", "aguardando_autorizacao", 300).unwrap();
        assert_eq!(filtradas.len(), 1);
        assert_eq!(filtradas[0].id, id1);

        // Segunda nota em rascunho aguardando conferência: `unica_aguardando_autorizacao` vira None.
        let id2 = "cp-t111-segunda".to_string();
        c.execute(
            "INSERT INTO compra (id, fornecedor_id, chave, numero, serie, emissao, natureza, total_produtos, frete,
                                 seguro, desconto, outras, ipi, icms_st, total, destinatario, xml, criado_em,
                                 criado_por, dispositivo, xml_verificacao, xml_prova, autorizacao, situacao)
             SELECT ?1, fornecedor_id, '35261011222333000181550010000123461000000022', '12346', serie, emissao,
                    natureza, total_produtos, frete, seguro, desconto, outras, ipi, icms_st, total, destinatario,
                    xml, criado_em, criado_por, dispositivo, xml_verificacao, xml_prova, 'aguardando', 'rascunho'
               FROM compra WHERE id = ?2",
            params![&id2, &id1],
        )
        .unwrap();
        assert_eq!(contar_aguardando_autorizacao(c).unwrap(), 2);
        assert_eq!(unica_aguardando_autorizacao(c).unwrap(), None);
        assert_eq!(listar_filtrada(c, "", "aguardando_autorizacao", 300).unwrap().len(), 2);

        // Confirma nota 1 pelo gerente: sai de aguardando_autorizacao e vira pode_lancar = true.
        let agora: String =
            c.query_row("SELECT strftime('%d/%m/%Y %H:%M', 'now', 'localtime')", [], |l| l.get(0)).unwrap();
        registrar_conferencia(
            c,
            &b.dispositivo,
            &g,
            &Conferencia {
                id: id1.clone(),
                situacao: "autorizada".into(),
                protocolo: "135260000000001".into(),
                conferido_em: agora.clone(),
            },
        )
        .unwrap();
        assert_eq!(contar_aguardando_autorizacao(c).unwrap(), 1);
        assert_eq!(unica_aguardando_autorizacao(c).unwrap().as_deref(), Some(id2.as_str()));
        let apos = listar(c, "", 300).unwrap();
        let n1 = apos.iter().find(|x| x.id == id1).unwrap();
        assert_eq!(n1.autorizacao, "confirmada_gerente");
        assert!(n1.pode_lancar);

        // Confere nota 2 como cancelada: sai de aguardando_autorizacao, vai para `negada` e segue bloqueada.
        registrar_conferencia(
            c,
            &b.dispositivo,
            &g,
            &Conferencia {
                id: id2.clone(),
                situacao: "cancelada".into(),
                protocolo: String::new(),
                conferido_em: agora,
            },
        )
        .unwrap();
        assert_eq!(contar_aguardando_autorizacao(c).unwrap(), 0);
        let ct_fim = contagem(c).unwrap();
        assert_eq!((ct_fim.aguardando_autorizacao, ct_fim.negadas), (0, 1));
        assert!(listar_filtrada(c, "", "aguardando_autorizacao", 300).unwrap().is_empty());
        let negadas = listar_filtrada(c, "", "negada", 300).unwrap();
        assert_eq!(negadas.len(), 1);
        assert_eq!(negadas[0].id, id2);
        assert_eq!(negadas[0].autorizacao, "negada");
        assert!(!negadas[0].pode_lancar);

        // Filtro desconhecido é recusado com erro de validação.
        assert!(listar_filtrada(c, "", "inexistente", 300).is_err());
    }

    #[test]
    fn rateio_sem_perder_centavos() {
        assert_eq!(ratear(100, &[1, 1, 1]), vec![34, 33, 33]);
        assert_eq!(ratear(4000, &[52000, 16000, 36000]).iter().sum::<i64>(), 4000);
        assert_eq!(ratear(1000, &[52000, 16000, 36000]), vec![500, 154, 346]);
        assert_eq!(ratear(10, &[0, 0]), vec![5, 5]);
        assert!(ratear(10, &[]).is_empty());
    }

    #[test]
    fn custo_por_peca() {
        assert_eq!(custo_unitario(54050, 10_000), 5405);
        assert_eq!(custo_unitario(1000, 3000), 333);
        assert_eq!(custo_unitario(1000, 0), 0);
    }

    fn base() -> (Banco, String) {
        let b = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        (b, u.id)
    }

    /// A11 da revisão da T108: a prévia e a importação de cadastros com as notas de compra dão o MESMO resumo (as
    /// categorias `compra.*` incluídas), com a opção marcada e desmarcada.
    #[test]
    fn t108_previa_igual_ao_resultado_com_as_compras() {
        for compras in [true, false] {
            let (b, u) = base();
            config::gravar(&b.conn, "loja_cnpj", "11444777000161").unwrap();
            let pasta = std::env::temp_dir().join(format!("rustinho-t108-resumo-{}", tempo::novo_id()));
            std::fs::create_dir_all(&pasta).unwrap();
            std::fs::write(pasta.join("nota.xml"), nfe_xml::EXEMPLO).unwrap();
            let e = crate::importar::Entrada {
                arquivos: vec![pasta.display().to_string()],
                fornecedores: true,
                pecas_fornecedor: true,
                compras,
                ..Default::default()
            };
            let classificar = classificar_para_compras;
            let criar = |c: &Connection, xml: &str, v: &nfe_xml::NotaVerificada| -> Resultado<()> {
                criar_compra(c, &b.dispositivo, &u, xml, v).map(|_| ())
            };
            let gancho = crate::importar::GanchoCompras { classificar: &classificar, criar: &criar };
            let previa = crate::importar::previa_com_compras(&b.conn, &e, Some(&gancho)).unwrap();
            let feito = crate::importar::importar_com_compras(&b.conn, &b.dispositivo, &u, &e, Some(&gancho)).unwrap();
            assert_eq!(previa.resumo, feito.resumo, "compras = {compras}");
            let codigo = if compras { "compra.nova" } else { "compra.nao_importada" };
            assert!(previa.resumo.categorias.iter().any(|c| c.codigo == codigo && c.quantidade == 1), "{compras}");
            let n: i64 = b.conn.query_row("SELECT COUNT(*) FROM compra", [], |l| l.get(0)).unwrap();
            assert_eq!(n, i64::from(compras), "compras = {compras}");
            std::fs::remove_dir_all(pasta).ok();
        }
    }

    /// N2 da 2ª conferência da T108: "prévia = resultado" também com a nota que já está em Compras (`compra.ja`) e
    /// com a que não serve para Compras (`compra.nao_serve`, a nota para outro CNPJ).
    #[test]
    fn t108_previa_igual_ao_resultado_com_ja_e_nao_serve() {
        for caso in ["ja", "nao_serve"] {
            let (b, u) = base();
            let cnpj = if caso == "ja" { "11444777000161" } else { "12345678000195" };
            config::gravar(&b.conn, "loja_cnpj", cnpj).unwrap();
            if caso == "ja" {
                importar_xml(&b.conn, &b.dispositivo, &u, nfe_xml::EXEMPLO).unwrap();
            }
            let pasta = std::env::temp_dir().join(format!("rustinho-t108-{caso}-{}", tempo::novo_id()));
            std::fs::create_dir_all(&pasta).unwrap();
            std::fs::write(pasta.join("nota.xml"), nfe_xml::EXEMPLO).unwrap();
            let e = crate::importar::Entrada {
                arquivos: vec![pasta.display().to_string()],
                fornecedores: true,
                pecas_fornecedor: true,
                compras: true,
                ..Default::default()
            };
            let classificar = classificar_para_compras;
            let criar = |c: &Connection, xml: &str, v: &nfe_xml::NotaVerificada| -> Resultado<()> {
                criar_compra(c, &b.dispositivo, &u, xml, v).map(|_| ())
            };
            let gancho = crate::importar::GanchoCompras { classificar: &classificar, criar: &criar };
            let previa = crate::importar::previa_com_compras(&b.conn, &e, Some(&gancho)).unwrap();
            let feito = crate::importar::importar_com_compras(&b.conn, &b.dispositivo, &u, &e, Some(&gancho)).unwrap();
            assert_eq!(previa.resumo, feito.resumo, "{caso}");
            let codigo = format!("compra.{caso}");
            assert!(previa.resumo.categorias.iter().any(|c| c.codigo == codigo && c.quantidade == 1), "{caso}");
            assert_eq!(feito.compras, 0, "{caso}");
            std::fs::remove_dir_all(pasta).ok();
        }
    }

    /// A9 da revisão da T108 (N2 da 2ª conferência): a nota em que as contas do rateio não fecham não serve para
    /// Compras (em vez de derrubar a importação de cadastros inteira na hora de criar a compra).
    #[test]
    fn t108_nota_que_nao_fecha_a_conta_nao_serve_para_compras() {
        let (b, _u) = base();
        config::gravar(&b.conn, "loja_cnpj", "11444777000161").unwrap();
        let mut nota = nfe_xml::ler(nfe_xml::EXEMPLO).unwrap();
        assert!(matches!(
            classificar_para_compras(&b.conn, nfe_xml::EXEMPLO, &nota).unwrap(),
            crate::importar::ParaCompras::Nova(_)
        ));
        nota.frete = 100;
        for it in nota.itens.iter_mut() {
            it.frete = numeros::SOMA_MAXIMA;
        }
        assert!(ratear_do_cabecalho(&nota, nota.itens.clone()).is_err(), "o caso precisa estourar a conta");
        assert!(matches!(
            classificar_para_compras(&b.conn, nfe_xml::EXEMPLO, &nota).unwrap(),
            crate::importar::ParaCompras::NaoServe
        ));
    }

    /// A9 da revisão da T108 (N2 da 2ª conferência): se a compra de uma nota não puder ser criada, nada é gravado e
    /// o aviso diz de qual arquivo.
    #[test]
    fn t108_erro_da_compra_diz_o_arquivo_e_nada_fica() {
        let (b, u) = base();
        config::gravar(&b.conn, "loja_cnpj", "11444777000161").unwrap();
        let pasta = std::env::temp_dir().join(format!("rustinho-t108-erro-{}", tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        std::fs::write(pasta.join("nota-ruim.xml"), nfe_xml::EXEMPLO).unwrap();
        let e = crate::importar::Entrada {
            arquivos: vec![pasta.display().to_string()],
            fornecedores: true,
            pecas_fornecedor: true,
            compras: true,
            ..Default::default()
        };
        let classificar = classificar_para_compras;
        let criar = |_: &Connection, _: &str, _: &nfe_xml::NotaVerificada| -> Resultado<()> {
            Err(Erro::validacao("a conta não fecha"))
        };
        let gancho = crate::importar::GanchoCompras { classificar: &classificar, criar: &criar };
        b.conn.execute_batch("SAVEPOINT t108").unwrap();
        let erro = crate::importar::importar_com_compras(&b.conn, &b.dispositivo, &u, &e, Some(&gancho)).unwrap_err();
        b.conn.execute_batch("ROLLBACK TO t108; RELEASE t108").unwrap();
        assert!(erro.to_string().starts_with("nota-ruim.xml: "), "{erro}");
        let pecas: i64 = b.conn.query_row("SELECT COUNT(*) FROM produto", [], |l| l.get(0)).unwrap();
        assert_eq!(pecas, 0, "a transação de quem chama volta inteira");
        std::fs::remove_dir_all(pasta).ok();
    }

    fn peca(b: &Banco, u: &str, nome: &str, barras: &str, original: &str, custo: &str) -> String {
        let mut f = produtos::ficha_nova(&b.conn).unwrap();
        f.nome = nome.into();
        f.codigo_barras = barras.into();
        f.codigo_original = original.into();
        f.custo = custo.into();
        produtos::salvar(&b.conn, &b.dispositivo, u, &f).unwrap()
    }

    /// R5 (D-AUD-4, decisão do proprietário de 25/11/2026): a compra com custo zero (bonificação) não troca o custo
    /// da peça, nem o preço, nem o % de lucro, com ou sem "atualizar os preços"; o registro diz que foi bonificação.
    /// Antes o custo ia a zero (e o lucro no DRE ficava maior do que é).
    #[test]
    fn r5_bonificacao_com_custo_zero_nao_troca_o_custo() {
        let (b, u) = base();
        let c = &b.conn;
        let pid = peca(&b, &u, "Palheta", "", "", "50");
        let tabelas = config::tabelas_preco(c).unwrap();
        let ler = || -> (i64, Vec<(i64, Option<i64>)>) {
            let custo = c.query_row("SELECT custo FROM produto WHERE id = ?1", [&pid], |l| l.get(0)).unwrap();
            let precos = c
                .prepare("SELECT preco, lucro FROM produto_preco WHERE produto_id = ?1 ORDER BY tabela")
                .unwrap()
                .query_map([&pid], |l| Ok((l.get(0)?, l.get(1)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            (custo, precos)
        };
        let antes = ler();
        assert_eq!(antes.0, 5_000);
        for atualizar_precos in [false, true] {
            let (detalhes, mudou_preco) =
                atualizar_custo(c, &b.dispositivo, &u, &pid, 0, atualizar_precos, true, &tabelas).unwrap();
            assert!(!mudou_preco);
            assert_eq!(detalhes, ["Bonificação (custo zero): o custo da peça continua R$ 50,00"]);
            assert_eq!(ler(), antes, "atualizar os preços: {atualizar_precos}");
        }
        // Um custo de verdade continua trocando o custo.
        atualizar_custo(c, &b.dispositivo, &u, &pid, 6_000, false, false, &tabelas).unwrap();
        assert_eq!(ler().0, 6_000);
    }

    /// D-15: o % de lucro refeito quando a compra muda o custo e o preço
    /// fica é a mesma conta do cadastro, também abaixo do custo.
    #[test]
    fn d15_lucro_abaixo_do_custo() {
        let (b, u) = base();
        let c = &b.conn;
        let mut f = produtos::ficha_nova(c).unwrap();
        f.nome = "Peça de teste".into();
        f.custo = "5,00".into();
        f.precos[0].preco = "8,00".into();
        f.precos[0].lucro = String::new();
        let pid = produtos::salvar(c, &b.dispositivo, &u, &f).unwrap();
        let tabelas = config::tabelas_preco(c).unwrap();
        for (custo, esperado) in [(1000, -2000), (1100, -2727), (300, 16667), (800, 0), (1, 7_990_000)] {
            atualizar_custo(c, &b.dispositivo, &u, &pid, custo, false, false, &tabelas).unwrap();
            let lucro: i64 = c
                .query_row("SELECT lucro FROM produto_preco WHERE produto_id = ?1 AND tabela = 1", [&pid], |l| l.get(0))
                .unwrap();
            assert_eq!(Some(lucro), numeros::lucro_por_preco(custo, 800), "custo {custo}");
            assert_eq!(lucro, esperado, "custo {custo}");
        }
    }

    /// O-4 (R-12 da 0.9.14): descartar um rascunho de compra guarda a
    /// auditoria dele (e mais uma linha dizendo que foi descartado), e o
    /// descartado não conta como compra lançada.
    #[test]
    fn o4_descartar_rascunho_preserva_a_auditoria() {
        let (b, u) = base();
        let c = &b.conn;
        let id = importar_xml(c, &b.dispositivo, &u, nfe_xml::EXEMPLO).unwrap();
        let contar = |sql: &str| -> i64 { c.query_row(sql, [&id], |l| l.get(0)).unwrap() };
        let antes = contar("SELECT COUNT(*) FROM auditoria WHERE entidade = 'compra' AND entidade_id = ?1");
        assert!(antes >= 1, "a importação deixou auditoria");
        excluir_rascunho(c, &b.dispositivo, &u, &id).unwrap();
        assert_eq!(
            contar("SELECT COUNT(*) FROM auditoria WHERE entidade = 'compra' AND entidade_id = ?1 AND acao = 'compra.importar'"),
            1,
            "a auditoria da importação continua"
        );
        assert_eq!(
            contar("SELECT COUNT(*) FROM auditoria WHERE entidade = 'compra' AND entidade_id = ?1 AND acao = 'compra.descartar'"),
            1,
            "o descarte fica registrado"
        );
        assert_eq!(contar("SELECT COUNT(*) FROM compra WHERE id = ?1"), 0, "o rascunho saiu");
        assert!(listar(c, "", 50).unwrap().is_empty(), "não aparece como compra");
        let lancadas: i64 =
            c.query_row("SELECT COUNT(*) FROM compra WHERE situacao <> 'rascunho'", [], |l| l.get(0)).unwrap();
        assert_eq!(lancadas, 0, "nada virou compra lançada");
        assert_eq!(c.query_row("SELECT COUNT(*) FROM estoque_movimento", [], |l| l.get::<_, i64>(0)).unwrap(), 0);
        assert_eq!(c.query_row("SELECT COUNT(*) FROM conta_pagar", [], |l| l.get::<_, i64>(0)).unwrap(), 0);
    }

    #[test]
    fn importa_liga_lanca_e_estorna() {
        let (b, u) = base();
        let c = &b.conn;
        let rolamento = peca(&b, &u, "Rolamento roda traseira", "7891234567895", "32206", "50,00");
        let pastilha = peca(&b, &u, "Pastilha dianteira Cobreq", "", "N-1100", "38,00");

        let id = importar_xml(c, &b.dispositivo, &u, nfe_xml::EXEMPLO).unwrap();
        // a mesma nota de novo é recusada
        let e = importar_xml(c, &b.dispositivo, &u, nfe_xml::EXEMPLO).unwrap_err();
        assert!(e.to_string().contains("já foi"), "{e}");

        let f = ficha(c, &id).unwrap();
        assert_eq!(f.situacao, "rascunho");
        assert_eq!(f.fornecedor, "DISTRIBUIDORA DE AUTOPECAS EXEMPLO LTDA");
        assert_eq!(f.itens.len(), 3);
        assert_eq!(f.itens[0].acao, "ligar");
        assert_eq!(f.itens[0].achado_por, "barras");
        assert_eq!(f.itens[1].acao, "ligar");
        assert_eq!(f.itens[1].achado_por, "codigo");
        assert_eq!(f.itens[2].acao, "");
        assert_eq!(f.pendentes, 1);
        // rolamento: (520 + 20 frete + 20,50 IPI) / 10 = 56,05
        assert_eq!(f.itens[0].custo_unitario, "56,05");
        assert_eq!(f.itens[0].peca.as_ref().unwrap().preco_novo, "112,10");
        assert!(f.itens[0].peca.as_ref().unwrap().custo_subiu);
        assert!(f.itens[2].monofasico);

        // não lança com item pendente
        assert!(lancar(c, &b.dispositivo, &u, &id).is_err());

        // óleo vira peça nova; caixa da pastilha vem com 2 jogos; frete à parte de R$ 10,40
        atualizar(
            c,
            &CompraEntrada {
                id: id.clone(),
                frete_extra: "10,40".into(),
                atualizar_precos: true,
                observacoes: String::new(),
                gerar_pagar: true,
                itens: vec![
                    ItemEntrada {
                        id: f.itens[0].id.clone(),
                        acao: "ligar".into(),
                        produto_id: rolamento.clone(),
                        fator: "1".into(),
                        baixar_preco: false,
                    },
                    ItemEntrada {
                        id: f.itens[1].id.clone(),
                        acao: "ligar".into(),
                        produto_id: pastilha.clone(),
                        fator: "2".into(),
                        baixar_preco: false,
                    },
                    ItemEntrada {
                        id: f.itens[2].id.clone(),
                        acao: "nova".into(),
                        produto_id: String::new(),
                        fator: "1".into(),
                        baixar_preco: false,
                    },
                ],
            },
        )
        .unwrap();
        let f = ficha(c, &id).unwrap();
        assert_eq!(f.itens[1].quantidade_pecas, "8");
        // frete extra 10,40 rateado: 520/1040 → 5,20 ; 160/1040 → 1,60 ; 360/1040 → 3,60
        assert_eq!(f.itens[0].custo_unitario, "56,57");
        assert_eq!(f.itens[1].custo_unitario, "21,45");
        assert_eq!(f.custo_total, "1.110,90");

        assert_eq!(f.duplicatas, ["001 · vence 07/11/2026 · R$ 550,25", "002 · vence 07/12/2026 · R$ 550,25"]);
        assert!(f.gerar_pagar);
        liberar_para_teste(c, &b.dispositivo, &id);
        let r = lancar(c, &b.dispositivo, &u, &id).unwrap();
        assert_eq!((r.itens_estoque, r.pecas_novas), (3, 1));
        // as duplicatas foram para o A pagar
        assert_eq!(r.parcelas_pagar, 2);
        let a_pagar = crate::pagar::painel(c, "12345", "abertas").unwrap();
        assert_eq!(a_pagar.parcelas.len(), 2);
        assert_eq!(a_pagar.parcelas[0].documento, "12345/001");
        assert_eq!(a_pagar.parcelas[0].categoria, "Fornecedores (peças)");
        assert_eq!(a_pagar.parcelas[1].parcela, "2/2");
        // com parcela paga, não estorna a compra
        let pago = crate::pagar::pagar(
            c,
            &b.dispositivo,
            &u,
            &crate::pagar::PagamentoEntrada {
                conta_id: a_pagar.parcelas[0].id.clone(),
                forma: "boleto".into(),
                origem: "caixa-geral".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(estornar(c, &b.dispositivo, &u, &id, "Nota devolvida")
            .unwrap_err()
            .to_string()
            .contains("parcela paga"));
        crate::pagar::estornar(c, &b.dispositivo, &u, &pago, "engano").unwrap();
        assert_eq!(estoque::saldo(c, &rolamento).unwrap(), 10_000);
        assert_eq!(estoque::saldo(c, &pastilha).unwrap(), 8_000);
        let ficha_rol = produtos::ficha(c, &rolamento).unwrap();
        assert_eq!(ficha_rol.custo, "56,57");
        assert_eq!(ficha_rol.precos[0].preco, "113,14");
        // a pastilha ficou mais barata (38,00 → 21,45) e o preço foi mantido
        let ficha_pas = produtos::ficha(c, &pastilha).unwrap();
        assert_eq!(ficha_pas.custo, "21,45");
        assert_eq!(ficha_pas.precos[0].preco, "76,00");
        assert!(ficha_rol.historico[0].detalhes.iter().any(|d| d.contains("Compra NF 12345")));
        assert!(ficha_rol.historico[0].detalhes[0].starts_with("Estoque: 0 → 10"));
        assert_eq!(ficha_rol.fornecedores.len(), 1);

        // peça nova do óleo: monofásica, NCM e CEST da nota, custo e preço pelo lucro padrão (100%)
        let oleo_id: String = c
            .query_row("SELECT produto_id FROM compra_item WHERE compra_id = ?1 AND ordem = 2", [&id], |l| l.get(0))
            .unwrap();
        let oleo = produtos::ficha(c, &oleo_id).unwrap();
        assert!(oleo.monofasico);
        assert_eq!(oleo.ncm, "27101932");
        // (360 + 10 frete + 3,60 do frete à parte) / 12 = 31,13
        assert_eq!(oleo.custo, "31,13");
        assert_eq!(oleo.precos[0].preco, "62,26");
        assert_eq!(oleo.estoque, "12");

        // a próxima nota do mesmo fornecedor liga sozinha pelo código dele
        estornar(c, &b.dispositivo, &u, &id, "Nota devolvida").unwrap();
        assert_eq!(estoque::saldo(c, &pastilha).unwrap(), 0);
        assert!(crate::pagar::painel(c, "12345", "abertas").unwrap().parcelas.is_empty(), "parcelas saíram");
        let id2 = importar_xml(c, &b.dispositivo, &u, nfe_xml::EXEMPLO).unwrap();
        let f2 = ficha(c, &id2).unwrap();
        assert!(f2.itens.iter().all(|i| i.acao == "ligar" && i.achado_por == "fornecedor"));
        assert_eq!(f2.itens[1].fator, "2");
        assert_eq!(listar(c, "", 50).unwrap().len(), 2);
        assert_eq!(listar(c, "pastilha", 50).unwrap().len(), 2);
        excluir_rascunho(c, &b.dispositivo, &u, &id2).unwrap();
        assert_eq!(listar(c, "", 50).unwrap().len(), 1);
    }

    #[test]
    fn aviso_de_cnpj_de_outra_empresa() {
        let (b, _) = base();
        assert!(aviso_cnpj(&b.conn, "11.222.333/0001-81").unwrap().starts_with("Preencha"));
        config::gravar(&b.conn, "loja_cnpj", "11222333000181").unwrap();
        config::gravar(&b.conn, "loja_nome", "LOJA MODELO").unwrap();
        assert_eq!(aviso_cnpj(&b.conn, "11.222.333/0001-81").unwrap(), "");
        let a = aviso_cnpj(&b.conn, "99888777000100").unwrap();
        assert!(a.contains("não é da LOJA MODELO") && !a.contains("9988"), "{a}");
        assert_eq!(aviso_cnpj(&b.conn, "").unwrap(), "");
    }

    /// N06-b (a), decisão do proprietário de 12/11: a entrada de compra pelo
    /// XML não liga sozinha uma peça desativada, achada pelo GTIN, pelo código
    /// original ou pelo vínculo do fornecedor. O item fica sem peça (a compra
    /// não lança assim) e vira pendência `peca_desativada`, com a peça como
    /// candidata. A peça ativa continua ligando como antes.
    #[test]
    fn n06b_compra_nao_liga_sozinha_peca_desativada() {
        let (b, u) = base();
        let c = &b.conn;
        let rolamento = peca(&b, &u, "Rolamento roda traseira", "7891234567895", "32206", "50,00");
        let pastilha = peca(&b, &u, "Pastilha dianteira Cobreq", "", "N-1100", "38,00");
        c.execute("UPDATE produto SET ativo = 0", []).unwrap();

        let id = importar_xml(c, &b.dispositivo, &u, nfe_xml::EXEMPLO).unwrap();
        let f = ficha(c, &id).unwrap();
        assert_eq!(f.itens.len(), 3);
        for i in 0..2 {
            assert_eq!(f.itens[i].acao, "", "item {i} ligou sozinho numa peça desativada");
        }
        assert!(lancar(c, &b.dispositivo, &u, &id).is_err(), "a compra não lança com o item sem peça");
        let pend = |pid: &str| -> i64 {
            c.query_row(
                "SELECT COUNT(*) FROM xml_pendencia_identificacao
                  WHERE situacao = 'pendente' AND motivo = 'peca_desativada' AND candidatos_json LIKE ?1",
                [format!("%{pid}%")],
                |l| l.get(0),
            )
            .unwrap()
        };
        assert_eq!(pend(&rolamento), 1, "pelo GTIN");
        assert_eq!(pend(&pastilha), 1, "pelo código original");

        // Pelo vínculo do fornecedor (sem GTIN na nota).
        let fid: String = c.query_row("SELECT fornecedor_id FROM compra WHERE id = ?1", [&id], |l| l.get(0)).unwrap();
        crate::catalogo::codigos::vincular_fornecedor(
            c,
            &fid,
            "XYZ-1",
            &rolamento,
            1000,
            crate::catalogo::codigos::JaLigado::Manter,
        )
        .unwrap();
        let item = ItemXml { codigo: "XYZ-1".into(), descricao: "ROLAMENTO".into(), ..Default::default() };
        let l = ligar_sozinho(c, &fid, &item, false, false).unwrap();
        assert_eq!((l.acao.as_str(), l.motivo_pendencia.as_str()), ("", "peca_desativada"));
        assert_eq!(l.candidatos, vec![rolamento.clone()]);

        // Reativada, a mesma busca liga como antes.
        c.execute("UPDATE produto SET ativo = 1 WHERE id = ?1", [&rolamento]).unwrap();
        let l = ligar_sozinho(c, &fid, &item, false, false).unwrap();
        assert_eq!((l.acao.as_str(), l.produto_id.as_deref()), ("ligar", Some(rolamento.as_str())));
        assert_eq!(l.achado_por, "fornecedor");
    }

    /// T111 (Opção C — Regra 3/4 + A01): item com código só de símbolos ("---") sem GTIN não cadastra peça
    /// automática; vira registro provisório `codigo_so_de_simbolos`. E descrição > 120 caracteres com peça já
    /// existente no banco vai para `possivel_duplicata_descricao` sem duplicar.
    #[test]
    fn t111_simbolos_e_descricao_longa_viram_registro_provisorio() {
        let (b, u) = base();
        let c = &b.conn;
        let nome_longo = format!("{} FIM", "A".repeat(125));
        let existente = peca(&b, &u, &nome_longo[..120], "", "", "20,00");

        let mut verificada =
            nfe_xml::ler_verificada(nfe_xml::EXEMPLO, crate::nfe_assinatura::Ancoras::padrao()).unwrap();
        verificada.nota.itens = vec![
            ItemXml {
                codigo: "---".into(),
                descricao: "PECA SO DE SIMBOLOS TESTE".into(),
                unidade: "UN".into(),
                quantidade: 1000,
                valor_produtos: 5000,
                ..Default::default()
            },
            ItemXml {
                codigo: "COD-NOVO-123".into(),
                descricao: nome_longo,
                unidade: "UN".into(),
                quantidade: 1000,
                valor_produtos: 2000,
                ..Default::default()
            },
        ];
        let criada = criar_compra_com_opcoes(
            c,
            &b.dispositivo,
            &u,
            nfe_xml::EXEMPLO,
            &verificada,
            true,
            &AmbiguidadesLote::default(),
        )
        .unwrap();
        assert_eq!((criada.pecas_cadastradas, criada.pendencias_identificacao), (0, 2));
        let total_pecas: i64 = c.query_row("SELECT COUNT(*) FROM produto", [], |l| l.get(0)).unwrap();
        assert_eq!(total_pecas, 1, "nenhuma peça nova foi criada para itens duvidosos");
        let motivo_simb: String = c
            .query_row("SELECT motivo FROM xml_pendencia_identificacao WHERE codigo_fornecedor = '---'", [], |l| {
                l.get(0)
            })
            .unwrap();
        assert_eq!(motivo_simb, "codigo_so_de_simbolos");
        let (motivo_dup, cands): (String, String) = c
            .query_row(
                "SELECT motivo, candidatos_json FROM xml_pendencia_identificacao WHERE codigo_fornecedor = 'COD-NOVO-123'",
                [],
                |l| Ok((l.get(0)?, l.get(1)?)),
            )
            .unwrap();
        assert_eq!(motivo_dup, "possivel_duplicata_descricao");
        assert!(cands.contains(&existente), "candidata de 120 chars: {cands}");
    }

    /// Achado 1 (Crítico — Hydrogen) e Achado 7.1 (Dois fornecedores pelo mesmo GTIN):
    /// - item com custo == 0 (bonificação/brinde) cadastra com `preco_aberto = 1` sem abortar a NF-e;
    /// - resolver pendência com `valor_unitario == 0` também cadastra com `preco_aberto = 1`;
    /// - dois fornecedores distintos compartilhando o mesmo GTIN: descartar a 1ª e depois a 2ª nota limpa a peça sem zumbi.
    #[test]
    fn t111_h1_custo_zero_e_h7_dois_fornecedores_mesmo_gtin() {
        let (b, u) = base();
        let c = &b.conn;

        // H1a: bonificação (custo == 0) + item pago na mesma NF-e não aborta a transação!
        let mut ver = nfe_xml::ler_verificada(nfe_xml::EXEMPLO, crate::nfe_assinatura::Ancoras::padrao()).unwrap();
        ver.nota.itens = vec![
            ItemXml {
                codigo: "BRINDE-01".into(),
                descricao: "ADESIVO PROMOCIONAL BONIFICACAO".into(),
                unidade: "UN".into(),
                quantidade: 1000,
                valor_produtos: 0,
                ..Default::default()
            },
            ItemXml {
                codigo: "ITEM-PAGO-02".into(),
                descricao: "FILTRO DE COMBUSTIVEL NORMAL".into(),
                unidade: "UN".into(),
                quantidade: 1000,
                valor_produtos: 5000,
                ..Default::default()
            },
        ];
        let criada =
            criar_compra_com_opcoes(c, &b.dispositivo, &u, nfe_xml::EXEMPLO, &ver, true, &AmbiguidadesLote::default())
                .expect("H1: item de custo zero não pode derrubar a NF-e!");
        assert_eq!(criada.pecas_cadastradas, 2);
        let preco_aberto: i64 = c
            .query_row("SELECT preco_aberto FROM produto WHERE nome = 'ADESIVO PROMOCIONAL BONIFICACAO'", [], |l| {
                l.get(0)
            })
            .unwrap();
        assert_eq!(preco_aberto, 1, "H1: brinde sem custo nasce com preço na hora da venda");

        // H1b: resolver pendência com valor_unitario == 0 como "nova" funciona!
        let pend_id = crate::compras::pendencias::registrar_pendencia_xml(
            c,
            None,
            "11222333000181",
            "FORN TESTE",
            "35261011222333000181550010000123451000000014",
            "12345",
            "2026-10-08",
            &ItemXml {
                codigo: "X1".into(),
                descricao: "AMOSTRA GRATIS TESTE".into(),
                unidade: "UN".into(),
                quantidade: 1000,
                valor_produtos: 0,
                ..Default::default()
            },
            0,
            "codigo_fraco_sem_gtin",
            &[],
        )
        .unwrap();
        let res = crate::importar::resolver_pendencia(
            c,
            &b.dispositivo,
            &u,
            &crate::importar::ResolverPendencia {
                id: pend_id,
                acao: "nova".into(),
                produto_id: String::new(),
                ..Default::default()
            },
        )
        .expect("H1b: resolver pendência com valor_unitario == 0 não pode falhar!");
        let pa_pend: i64 =
            c.query_row("SELECT preco_aberto FROM produto WHERE id = ?1", [&res.produto_id], |l| l.get(0)).unwrap();
        assert_eq!(pa_pend, 1);

        // H7.1: Dois fornecedores distintos pelo mesmo GTIN; descarta 1ª e depois 2ª → limpa sem zumbi!
        let (b2, u2) = base();
        let c2 = &b2.conn;
        let mut ver_a = nfe_xml::ler_verificada(nfe_xml::EXEMPLO, crate::nfe_assinatura::Ancoras::padrao()).unwrap();
        ver_a.nota.chave = "35261011222333000181550010000000011000000011".into();
        ver_a.nota.emitente.documento = "11222333000181".into();
        ver_a.nota.itens = vec![ItemXml {
            codigo: "A-100".into(),
            gtin: "7891234567895".into(),
            gtin_comercial: "7891234567895".into(),
            descricao: "ROLAMENTO COMPARTILHADO GTIN".into(),
            unidade: "UN".into(),
            quantidade: 1000,
            valor_produtos: 5000,
            ..Default::default()
        }];
        let ca = criar_compra_com_opcoes(
            c2,
            &b2.dispositivo,
            &u2,
            nfe_xml::EXEMPLO,
            &ver_a,
            true,
            &AmbiguidadesLote::default(),
        )
        .unwrap()
        .id;
        let mut ver_b = ver_a.clone();
        ver_b.nota.chave = "35261033444555000181550010000000021000000022".into();
        ver_b.nota.emitente.documento = "33444555000181".into();
        ver_b.nota.emitente.nome = "OUTRO FORNECEDOR LTDA".into();
        ver_b.nota.itens[0].codigo = "B-200".into();
        let cb = criar_compra_com_opcoes(
            c2,
            &b2.dispositivo,
            &u2,
            nfe_xml::EXEMPLO,
            &ver_b,
            true,
            &AmbiguidadesLote::default(),
        )
        .unwrap()
        .id;
        assert_eq!(c2.query_row::<i64, _, _>("SELECT COUNT(*) FROM produto", [], |l| l.get(0)).unwrap(), 1);
        excluir_rascunho(c2, &b2.dispositivo, &u2, &ca).unwrap();
        assert_eq!(
            c2.query_row::<i64, _, _>("SELECT COUNT(*) FROM produto", [], |l| l.get(0)).unwrap(),
            1,
            "peça preservada enquanto compra B está aberta"
        );
        excluir_rascunho(c2, &b2.dispositivo, &u2, &cb).unwrap();
        assert_eq!(
            c2.query_row::<i64, _, _>("SELECT COUNT(*) FROM produto", [], |l| l.get(0)).unwrap(),
            0,
            "H7.1: zumbi ficou no banco ao descartar a 2ª compra de outro fornecedor!"
        );
    }
}
