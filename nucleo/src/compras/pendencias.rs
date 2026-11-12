//! Pendências de identificação dos itens das notas de compra (dev.3.3, passo
//! 3.4; contrato C4 da `ARQUITETURA_ALVO_RUSTINHO.md`): o único módulo que
//! grava em `xml_pendencia_identificacao`, com o mesmo comportamento de antes
//! (as funções vieram do `importar`). A lista, a contagem e a resolução pela
//! tela continuam no `importar`, que chama estas. O descarte com motivo e o
//! fechamento pela decisão de outro caixa são da fase 4 (4.5) e 5 (5.3).

use rusqlite::{params, Connection, OptionalExtension};

use crate::erro::Resultado;
use crate::nfe_xml::ItemXml;
use crate::{documento, tempo, texto};

/// Abre a pendência de identificação de um item da nota (ou atualiza a que
/// já está aberta para o mesmo fornecedor, código e descrição: não duplica).
/// `valor_unitario`: o do item com os impostos (`importar::valor_unitario`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn registrar_pendencia_xml(
    conn: &Connection,
    fornecedor_id: Option<&str>,
    fornecedor_doc: &str,
    fornecedor_nome: &str,
    chave_nfe: &str,
    numero_nfe: &str,
    emissao_nfe: &str,
    item: &ItemXml,
    valor_unitario: i64,
    motivo: &str,
    candidatos: &[String],
) -> Resultado<String> {
    let doc_limpo = documento::limpar(fornecedor_doc);
    let compacto = texto::compactar(&item.codigo);
    let desc_comp = texto::compactar(&item.descricao);
    let gtins_json = serde_json::to_string(&item.gtins).unwrap_or_else(|_| "[]".to_string());
    let candidatos_json = serde_json::to_string(candidatos).unwrap_or_else(|_| "[]".to_string());
    let v_unit = valor_unitario;

    // Se já foi resolvida anteriormente para o mesmo fornecedor + código (sem ambiguidade),
    // ou se já existe pendência aberta para o mesmo fornecedor + código + descrição, não duplica.
    let mut stmt = conn.prepare_cached(
        "SELECT id, descricao FROM xml_pendencia_identificacao
          WHERE situacao = 'pendente'
            AND fornecedor_documento = ?1
            AND codigo_compacto = ?2",
    )?;
    let abertas: Vec<(String, String)> =
        stmt.query_map(params![doc_limpo, compacto], |l| Ok((l.get(0)?, l.get(1)?)))?.collect::<Result<_, _>>()?;
    if let Some((id_existente, _)) = abertas.into_iter().find(|(_, d)| texto::compactar(d) == desc_comp) {
        conn.execute(
            "update xml_pendencia_identificacao
                set fornecedor_id = COALESCE(?2, fornecedor_id),
                    fornecedor_nome = CASE WHEN ?3 <> '' THEN ?3 ELSE fornecedor_nome END,
                    chave_nfe = CASE WHEN ?4 <> '' THEN ?4 ELSE chave_nfe END,
                    numero_nfe = CASE WHEN ?5 <> '' THEN ?5 ELSE numero_nfe END,
                    emissao_nfe = CASE WHEN ?6 <> '' THEN ?6 ELSE emissao_nfe END,
                    gtin_comercial = ?7,
                    origem_comercial = ?8,
                    gtin_tributavel = ?9,
                    origem_tributavel = ?10,
                    gtins_json = ?11,
                    ncm = ?12,
                    cest = ?13,
                    cfop = ?14,
                    unidade = ?15,
                    origem_fiscal = ?16,
                    cst_icms = ?17,
                    cst_pis = ?18,
                    quantidade = ?19,
                    valor_unitario = ?20,
                    motivo = ?21,
                    candidatos_json = ?22
              where id = ?1",
            params![
                id_existente,
                fornecedor_id,
                fornecedor_nome,
                chave_nfe,
                numero_nfe,
                emissao_nfe,
                item.gtin_comercial,
                item.origem_comercial,
                item.gtin_tributavel,
                item.origem_tributavel,
                gtins_json,
                item.ncm,
                item.cest,
                item.cfop,
                item.unidade,
                item.origem,
                item.cst_icms,
                item.cst_pis,
                item.quantidade,
                v_unit,
                motivo,
                candidatos_json,
            ],
        )?;
        return Ok(id_existente);
    }

    let id = tempo::novo_id();
    conn.execute(
        "insert into xml_pendencia_identificacao (
            id, fornecedor_id, fornecedor_documento, fornecedor_nome, chave_nfe, numero_nfe, emissao_nfe,
            codigo_fornecedor, codigo_compacto, gtin_comercial, origem_comercial, gtin_tributavel, origem_tributavel,
            gtins_json, descricao, descricao_compacta, ncm, cest, cfop, unidade, origem_fiscal, cst_icms, cst_pis, quantidade,
            valor_unitario, motivo, candidatos_json, situacao, criado_em
         ) values (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7,
            ?8, ?9, ?10, ?11, ?12, ?13,
            ?14, ?15, ?28, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23,
            ?24, ?25, ?26, 'pendente', ?27
         )",
        params![
            id,
            fornecedor_id,
            doc_limpo,
            fornecedor_nome,
            chave_nfe,
            numero_nfe,
            emissao_nfe,
            item.codigo.trim(),
            compacto,
            item.gtin_comercial,
            item.origem_comercial,
            item.gtin_tributavel,
            item.origem_tributavel,
            gtins_json,
            texto::limpar(&item.descricao),
            item.ncm,
            item.cest,
            item.cfop,
            item.unidade,
            item.origem,
            item.cst_icms,
            item.cst_pis,
            item.quantidade,
            v_unit,
            motivo,
            candidatos_json,
            tempo::agora(),
            desc_comp,
        ],
    )?;
    Ok(id)
}

/// Fecha as pendências abertas do mesmo fornecedor, código e descrição com a
/// peça escolhida. A06 (D-13): antes, a descrição só contava quando o motivo era
/// "código repetido", e uma resolução fechava as pendências de outra peça.
pub(crate) fn resolver_pendencias_do_item(
    conn: &Connection,
    fornecedor_id: &str,
    codigo_fornecedor: &str,
    descricao: &str,
    produto_id: &str,
) -> Resultado<()> {
    let compacto = texto::compactar(codigo_fornecedor);
    // A01 (e G04, sem mudar o esquema): um código só de símbolos ("---")
    // compacta para vazio e não identifica item nenhum; fechar "as do mesmo
    // código" fecharia as de outros itens. Quem chamou fecha a sua à parte.
    if compacto.is_empty() {
        return Ok(());
    }
    let desc_comp = texto::compactar(descricao);
    let doc: String = conn
        .query_row("SELECT documento FROM pessoa WHERE id = ?1", [fornecedor_id], |l| l.get(0))
        .optional()?
        .unwrap_or_default();
    let mut stmt = conn.prepare_cached(
        "SELECT id, descricao FROM xml_pendencia_identificacao
          WHERE situacao = 'pendente'
            AND (fornecedor_id = ?1 OR (?2 <> '' AND fornecedor_documento = ?2))
            AND codigo_compacto = ?3",
    )?;
    let linhas: Vec<(String, String)> = stmt
        .query_map(params![fornecedor_id, doc, compacto], |l| Ok((l.get(0)?, l.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let agora = tempo::agora();
    for (pid_pend, desc_pend) in linhas {
        if texto::compactar(&desc_pend) == desc_comp {
            conn.execute(
                "update xml_pendencia_identificacao
                    set situacao = 'resolvida', produto_id = ?2, resolvido_em = ?3
                  where id = ?1",
                params![pid_pend, produto_id, agora],
            )?;
        }
    }
    Ok(())
}

/// Marca a pendência como resolvida com a peça escolhida.
pub(crate) fn marcar_resolvida(conn: &Connection, id: &str, produto_id: &str, agora: &str) -> Resultado<()> {
    conn.execute(
        "update xml_pendencia_identificacao
            set situacao = 'resolvida', produto_id = ?2, resolvido_em = ?3
          where id = ?1",
        params![id, produto_id, agora],
    )?;
    Ok(())
}
