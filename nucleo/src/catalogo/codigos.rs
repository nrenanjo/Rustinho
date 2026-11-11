//! Identificação da peça (dev.3.3, passo 3.3; contrato C3 da
//! `ARQUITETURA_ALVO_RUSTINHO.md`): o único módulo que grava os códigos que
//! identificam uma peça. Cada função reproduz o que o caminho de antes fazia
//! (a ficha da peça, a importação de cadastros, a compra), sem mudar regra:
//! as mudanças de política são da fase 4 (4.1 a 4.3).
//!
//! Só grava. O texto do índice de busca sai da peça inteira, por isso quem o
//! monta é `produtos`, que chama este módulo (e não o contrário).
//!
//! O `INSERT`/`UPDATE` da peça inteira (`produtos::salvar`) continua lá: as
//! colunas de código vão no mesmo comando que as outras, e separá-las faria a
//! troca de dados registrar duas alterações por gravação.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::erro::Resultado;
use crate::{gtin, tempo, texto};

/// Um código GTIN / barras associado à peça, com tipo (`comercial` ou
/// `tributavel`) e origem (`cEAN`, `cEANTrib`, `cBarra`, `cBarraTrib` ou `manual`).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GtinFicha {
    pub gtin: String,
    pub tipo: String,
    pub origem: String,
}

/// Grava os GTINs da ficha da peça (`produtos::salvar`), na ordem da ficha. O
/// que já existe com o mesmo tipo e a mesma origem fica como está.
pub fn registrar_gtins_da_ficha(
    conn: &Connection,
    produto_id: &str,
    gtins: &[GtinFicha],
    agora: &str,
) -> Resultado<()> {
    for (ordem, g) in gtins.iter().enumerate() {
        conn.execute(
            "insert into produto_gtin (id, produto_id, gtin, tipo, origem, ordem, criado_em)
             values (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             on conflict (produto_id, gtin, tipo, origem) do nothing",
            params![tempo::novo_id(), produto_id, g.gtin, g.tipo, g.origem, ordem as i64, agora],
        )?;
    }
    Ok(())
}

/// Grava os GTINs / códigos de barras vindos do XML (`cEAN`, `cEANTrib`,
/// `cBarra`, `cBarraTrib`) para `produto_id`, diferenciando comercial e
/// tributável e preservando a origem, sem sobrescrever identificadores que já
/// existam na peça nem associar código que já pertença a outra peça. Se a peça
/// não tem código de barras, o primeiro GTIN válido (comercial, se houver)
/// vira o principal. Não refaz o índice de busca: quem chama é
/// `produtos::registrar_gtins_xml`, que o refaz depois.
pub fn registrar_gtins_xml(conn: &Connection, produto_id: &str, gtins: &[GtinFicha]) -> Resultado<usize> {
    if gtins.is_empty() {
        return Ok(0);
    }
    let agora = tempo::agora();
    let mut gravados = 0usize;
    let mut primeiro_valido_comercial = String::new();
    let mut primeiro_valido_qualquer = String::new();
    let ordem_base: i64 = conn
        .query_row("SELECT COALESCE(MAX(ordem), -1) + 1 FROM produto_gtin WHERE produto_id = ?1", [produto_id], |l| {
            l.get(0)
        })
        .unwrap_or(0);

    for (idx, g) in gtins.iter().enumerate() {
        let tipo = if g.tipo == "tributavel" { "tributavel" } else { "comercial" };
        let origem = match g.origem.as_str() {
            "cEAN" | "cEANTrib" | "cBarra" | "cBarraTrib" => g.origem.as_str(),
            _ => "cEAN",
        };
        let limpo: String = g.gtin.chars().filter(|c| !matches!(c, ' ' | '-')).collect();
        let codigo_valido = if gtin::gtin_valido(&limpo) {
            limpo
        } else if matches!(origem, "cBarra" | "cBarraTrib") {
            let alfa: String = limpo.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
            if alfa.len() >= 4 {
                alfa
            } else {
                continue;
            }
        } else {
            continue;
        };
        let de_outra: bool = conn
            .query_row(
                "SELECT 1 FROM produto p
                  WHERE p.id <> ?1
                    AND (p.codigo_barras = ?2 OR EXISTS (SELECT 1 FROM produto_gtin pg WHERE pg.produto_id = p.id AND pg.gtin = ?2))
                  LIMIT 1",
                params![produto_id, codigo_valido],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);
        if de_outra {
            continue;
        }
        let n = conn.execute(
            "insert into produto_gtin (id, produto_id, gtin, tipo, origem, ordem, criado_em)
             values (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             on conflict (produto_id, gtin, tipo, origem) do nothing",
            params![tempo::novo_id(), produto_id, codigo_valido, tipo, origem, ordem_base + idx as i64, agora],
        )?;
        if n > 0 {
            gravados += n;
        }
        if gtin::gtin_valido(&codigo_valido) {
            if tipo == "comercial" && primeiro_valido_comercial.is_empty() {
                primeiro_valido_comercial = codigo_valido.clone();
            }
            if primeiro_valido_qualquer.is_empty() {
                primeiro_valido_qualquer = codigo_valido;
            }
        }
    }

    let principal =
        if !primeiro_valido_comercial.is_empty() { primeiro_valido_comercial } else { primeiro_valido_qualquer };
    if !principal.is_empty() {
        conn.execute(
            "UPDATE produto SET codigo_barras = ?2 WHERE id = ?1 AND codigo_barras = ''",
            params![produto_id, principal],
        )?;
    }
    Ok(gravados)
}

/// O que fazer quando o fornecedor já tem esse código ligado a uma peça.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JaLigado {
    /// Fica como está (a importação de cadastros).
    Manter,
    /// Passa para esta peça e o fator fica (resolver a pendência à mão:
    /// PEND-36, que a 4.2 muda).
    PassarPeca,
    /// Passa para esta peça com o fator novo (lançar a compra: A08, que a 4.2
    /// muda).
    PassarPecaEFator,
}

/// Liga o código que o fornecedor usa a uma peça (`produto_fornecedor`), pela
/// forma compacta do código (os zeros à esquerda ficam). O código é guardado
/// como veio. Código vazio não liga nada. Devolve se gravou (ligou agora ou,
/// conforme `ja_ligado`, passou de peça). Não refaz o índice de busca: quem
/// chama refaz, como antes.
pub fn vincular_fornecedor(
    conn: &Connection,
    fornecedor_id: &str,
    codigo: &str,
    produto_id: &str,
    fator: i64,
    ja_ligado: JaLigado,
) -> Resultado<bool> {
    let compacto = texto::compactar(codigo);
    if compacto.is_empty() {
        return Ok(false);
    }
    let sql = match ja_ligado {
        JaLigado::Manter => {
            "INSERT INTO produto_fornecedor (fornecedor_id, codigo_compacto, codigo, produto_id, fator, atualizado_em)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (fornecedor_id, codigo_compacto) DO NOTHING"
        }
        JaLigado::PassarPeca => {
            "INSERT INTO produto_fornecedor (fornecedor_id, codigo_compacto, codigo, produto_id, fator, atualizado_em)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (fornecedor_id, codigo_compacto) DO UPDATE
               SET codigo = excluded.codigo, produto_id = excluded.produto_id, atualizado_em = excluded.atualizado_em"
        }
        JaLigado::PassarPecaEFator => {
            "INSERT INTO produto_fornecedor (fornecedor_id, codigo_compacto, codigo, produto_id, fator, atualizado_em)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (fornecedor_id, codigo_compacto) DO UPDATE
               SET codigo = excluded.codigo, produto_id = excluded.produto_id, fator = excluded.fator,
                   atualizado_em = excluded.atualizado_em"
        }
    };
    let n = conn.execute(sql, params![fornecedor_id, compacto, codigo, produto_id, fator, tempo::agora()])?;
    Ok(n > 0)
}
