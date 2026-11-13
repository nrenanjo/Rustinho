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
        let Some((codigo_valido, tipo, origem)) = codigo_do_xml(g) else { continue };
        let de_outra: bool = conn
            .query_row(SQL_GTIN_DE_OUTRA_PECA, params![produto_id, codigo_valido], |_| Ok(true))
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

/// O código que o `registrar_gtins_xml` grava para um GTIN da nota, com o
/// tipo e a origem como ficam gravados; `None` quando ele não entra. O GTIN
/// válido (sem espaço e traço) entra; de `cBarra`/`cBarraTrib`, as letras e os
/// números, se forem 4 ou mais. Uma regra só para a gravação e para o plano da
/// importação de cadastros (lote 2, 03a), que precisa saber, sem gravar, o
/// que cada peça vai passar a ter.
pub(crate) fn codigo_do_xml(g: &GtinFicha) -> Option<(String, &'static str, &'static str)> {
    let tipo = if g.tipo == "tributavel" { "tributavel" } else { "comercial" };
    let origem = match g.origem.as_str() {
        "cEANTrib" => "cEANTrib",
        "cBarra" => "cBarra",
        "cBarraTrib" => "cBarraTrib",
        _ => "cEAN",
    };
    let limpo: String = g.gtin.chars().filter(|c| !matches!(c, ' ' | '-')).collect();
    if gtin::gtin_valido(&limpo) {
        return Some((limpo, tipo, origem));
    }
    if matches!(origem, "cBarra" | "cBarraTrib") {
        let alfa: String = limpo.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        if alfa.len() >= 4 {
            return Some((alfa, tipo, origem));
        }
    }
    None
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

/// Guarda um código que a peça teve (`produto_codigo_antigo`): a etiqueta
/// velha ainda acha a peça, e o código nunca vai para outra. O que já está
/// guardado fica como está.
pub fn guardar_codigo_antigo(conn: &Connection, codigo: &str, produto_id: &str, quando: &str) -> Resultado<()> {
    conn.execute(
        "INSERT OR IGNORE INTO produto_codigo_antigo (codigo, produto_id, codigo_compacto, trocado_em)
         VALUES (?1, ?2, ?3, ?4)",
        params![codigo, produto_id, texto::compactar(codigo), quando],
    )?;
    Ok(())
}

/// Grava o índice de busca da peça (`produto.busca` e `produto.codigos`), que
/// `produtos` monta a partir da peça inteira.
pub fn gravar_indice(conn: &Connection, produto_id: &str, (busca, codigos): &(String, String)) -> Resultado<()> {
    conn.execute("UPDATE produto SET busca = ?2, codigos = ?3 WHERE id = ?1", params![produto_id, busca, codigos])?;
    Ok(())
}

/// Troca o código interno da peça (o gerente resolve um código repetido) e o
/// índice de busca; o código novo vale já (`codigo_validado`).
pub fn trocar_codigo_interno(
    conn: &Connection,
    id: &str,
    novo: &str,
    (busca, codigos): &(String, String),
    agora: &str,
    usuario_id: &str,
    dispositivo: &str,
) -> Resultado<()> {
    conn.execute(
        "UPDATE produto SET codigo = ?2, busca = ?3, codigos = ?4, atualizado_em = ?5, atualizado_por = ?6, dispositivo = ?7,
                codigo_validado = 1
          WHERE id = ?1",
        params![id, novo, busca, codigos, agora, usuario_id, dispositivo],
    )?;
    Ok(())
}

/// Dá o código interno a uma peça que estava com código a definir, com o
/// índice de busca. Devolve se deu (`false` = a peça já tinha código).
pub fn definir_codigo_interno(
    conn: &Connection,
    id: &str,
    codigo: &str,
    (busca, codigos): &(String, String),
    agora: &str,
    dispositivo: &str,
) -> Resultado<bool> {
    let n = conn.execute(
        "UPDATE produto SET codigo = ?2, busca = ?3, codigos = ?4, atualizado_em = ?5, dispositivo = ?6,
                codigo_validado = 1
          WHERE id = ?1 AND codigo = ''",
        params![id, codigo, busca, codigos, agora, dispositivo],
    )?;
    Ok(n == 1)
}

// ---------------------------------------------------------------------------
// Consultas (as mesmas de antes; as mudanças de política são da fase 4)
// ---------------------------------------------------------------------------

/// O que uma busca de peça por código achou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BuscaId {
    Nenhum,
    Unico(String),
    Ambiguo(Vec<String>),
}

pub(crate) fn por_quantidade(mut achados: Vec<String>) -> BuscaId {
    match achados.len() {
        0 => BuscaId::Nenhum,
        1 => BuscaId::Unico(achados.remove(0)),
        _ => BuscaId::Ambiguo(achados),
    }
}

/// N07: as peças com este código como código de barras principal ou como GTIN.
/// O índice de `produto.codigo_barras` é parcial (`WHERE codigo_barras <> ''`):
/// o SQLite só o usa se a consulta também disser `codigo_barras <> ''`; com o
/// `OR EXISTS` de antes, lia a tabela de peças inteira a cada código. Mesmo
/// resultado para código não vazio (o único que os chamadores mandam).
pub(crate) const SQL_PECAS_PELO_GTIN: &str = "SELECT id FROM produto WHERE codigo_barras = ?1 AND codigo_barras <> ''
     UNION SELECT produto_id FROM produto_gtin WHERE gtin = ?1
     ORDER BY 1";

/// N07: há outra peça (que não ?1) com este código ?2, no código de barras ou
/// num GTIN? Cada metade pelo seu índice (veja `SQL_PECAS_PELO_GTIN`).
pub(crate) const SQL_GTIN_DE_OUTRA_PECA: &str =
    "SELECT 1 FROM produto WHERE id <> ?1 AND codigo_barras = ?2 AND codigo_barras <> ''
     UNION ALL SELECT 1 FROM produto_gtin WHERE produto_id <> ?1 AND gtin = ?2
     LIMIT 1";

/// As peças que têm algum destes códigos como código de barras principal ou
/// como GTIN, sem repetir, na ordem em que aparecem.
pub(crate) fn pecas_pelos_gtins(conn: &Connection, codigos: &[String]) -> Resultado<BuscaId> {
    let mut achados: Vec<String> = Vec::new();
    let mut stmt = conn.prepare_cached(SQL_PECAS_PELO_GTIN)?;
    for cod in codigos {
        if cod.is_empty() {
            continue;
        }
        let linhas: Vec<String> = stmt.query_map([cod], |l| l.get(0))?.collect::<Result<_, _>>()?;
        for pid in linhas {
            if !achados.contains(&pid) {
                achados.push(pid);
            }
        }
    }
    Ok(por_quantidade(achados))
}

/// A peça ligada ao código do fornecedor identificado pelo documento (CNPJ ou
/// CPF), pela forma compacta do código.
pub(crate) fn peca_por_fornecedor_inequivoco(
    conn: &Connection,
    fornecedor_doc: &str,
    codigo: &str,
) -> Resultado<BuscaId> {
    let compacto = texto::compactar(codigo);
    if fornecedor_doc.is_empty() || compacto.is_empty() {
        return Ok(BuscaId::Nenhum);
    }
    let mut stmt = conn.prepare_cached(
        "SELECT DISTINCT f.produto_id FROM produto_fornecedor f JOIN pessoa p ON p.id = f.fornecedor_id
          WHERE p.documento = ?1 AND f.codigo_compacto = ?2
          ORDER BY f.produto_id",
    )?;
    let achados: Vec<String> =
        stmt.query_map(params![fornecedor_doc, compacto], |l| l.get(0))?.collect::<Result<_, _>>()?;
    Ok(por_quantidade(achados))
}

/// A peça e o fator do vínculo do fornecedor (pelo id dele), pela forma
/// compacta do código.
pub(crate) fn vinculo_do_fornecedor(
    conn: &Connection,
    fornecedor_id: &str,
    compacto: &str,
) -> Resultado<Option<(String, i64)>> {
    Ok(conn
        .query_row(
            "SELECT f.produto_id, f.fator FROM produto_fornecedor f JOIN produto p ON p.id = f.produto_id
              WHERE f.fornecedor_id = ?1 AND f.codigo_compacto = ?2",
            params![fornecedor_id, compacto],
            |l| Ok((l.get(0)?, l.get(1)?)),
        )
        .optional()?)
}

/// No máximo quantos candidatos por código original a compra mostra (a
/// partir de 2, é ambíguo e vira pendência).
const CANDIDATOS_MAXIMOS: usize = 5;

/// As peças cujo código original ou um equivalente é este código (forma
/// compacta), de qualquer fornecedor. PEND-40: filtra **antes** de limitar.
/// Antes, olhava só as 5 primeiras peças do índice que tinham o código e
/// filtrava depois: a peça certa podia ficar de fora (falso "sem vínculo") ou
/// sobrar uma só (falso "único").
pub(crate) fn candidatos_por_codigo_original(conn: &Connection, compacto: &str) -> Resultado<Vec<String>> {
    let mut c = conn.prepare_cached(
        "SELECT p.id, p.codigo_original FROM produto p
          WHERE instr(p.codigos, ' ' || ?1 || ' ') > 0
          ORDER BY p.id",
    )?;
    let linhas: Vec<(String, String)> =
        c.query_map([compacto], |l| Ok((l.get(0)?, l.get(1)?)))?.collect::<Result<_, _>>()?;
    let mut candidatos = Vec::new();
    for (pid, orig) in linhas {
        if texto::compactar(&orig) == compacto {
            candidatos.push(pid);
        } else {
            let tem_equiv: bool = conn
                .query_row(
                    "SELECT 1 FROM produto_equivalente WHERE produto_id = ?1 AND codigo_compacto = ?2 LIMIT 1",
                    params![pid, compacto],
                    |_| Ok(true),
                )
                .optional()?
                .unwrap_or(false);
            if tem_equiv {
                candidatos.push(pid);
            }
        }
        if candidatos.len() == CANDIDATOS_MAXIMOS {
            break;
        }
    }
    Ok(candidatos)
}

// D04: as perguntas sobre GTIN repetido. Contam o código de barras da peça e
// os GTINs guardados dela (`produto_gtin`); antes, as duplicatas e o bloqueio
// da etiqueta olhavam só o código de barras.

/// Os GTINs que levam a mais de uma peça: (GTIN, peça), por GTIN e nome.
pub(crate) fn gtins_em_mais_de_uma_peca(conn: &Connection) -> Resultado<Vec<(String, String)>> {
    let mut c = conn.prepare_cached(
        "WITH g AS (SELECT codigo_barras AS gtin, id AS produto_id FROM produto WHERE codigo_barras <> ''
                    UNION SELECT gtin, produto_id FROM produto_gtin WHERE gtin <> '')
         SELECT g.gtin, g.produto_id FROM g JOIN produto p ON p.id = g.produto_id
          WHERE g.gtin IN (SELECT gtin FROM g GROUP BY gtin HAVING COUNT(DISTINCT produto_id) > 1)
          ORDER BY g.gtin, p.nome, g.produto_id",
    )?;
    let linhas = c.query_map([], |l| Ok((l.get(0)?, l.get(1)?)))?.collect::<Result<_, _>>()?;
    Ok(linhas)
}

/// Um GTIN desta peça (o código de barras ou um guardado) que também leva a
/// outra peça: (GTIN, nome da outra).
pub(crate) fn gtin_de_outra_peca(conn: &Connection, produto_id: &str) -> Resultado<Option<(String, String)>> {
    Ok(conn
        .query_row(
            "WITH meus AS (SELECT codigo_barras AS gtin FROM produto WHERE id = ?1 AND codigo_barras <> ''
                           UNION SELECT gtin FROM produto_gtin WHERE produto_id = ?1 AND gtin <> ''),
                  outros AS (SELECT codigo_barras AS gtin, id AS produto_id FROM produto
                              WHERE codigo_barras <> '' AND id <> ?1
                             UNION SELECT gtin, produto_id FROM produto_gtin WHERE produto_id <> ?1)
             SELECT o.gtin, p.nome FROM outros o JOIN meus m ON m.gtin = o.gtin JOIN produto p ON p.id = o.produto_id
              ORDER BY o.gtin, p.nome LIMIT 1",
            [produto_id],
            |l| Ok((l.get(0)?, l.get(1)?)),
        )
        .optional()?)
}

#[cfg(test)]
mod testes_n07 {
    //! N07: as buscas por GTIN e por documento usam os índices parciais e dão
    //! o mesmo resultado das consultas de antes (que liam a tabela inteira).
    use super::*;
    use crate::banco::Banco;
    use crate::{produtos, usuarios};

    /// As consultas de antes, como estavam em `24ceb31` (só para comparar).
    const ANTES_PECAS_PELO_GTIN: &str = "SELECT DISTINCT p.id FROM produto p
          WHERE p.codigo_barras = ?1
             OR EXISTS (SELECT 1 FROM produto_gtin pg WHERE pg.produto_id = p.id AND pg.gtin = ?1)
          ORDER BY p.id";
    const ANTES_GTIN_DE_OUTRA_PECA: &str = "SELECT 1 FROM produto p
                  WHERE p.id <> ?1
                    AND (p.codigo_barras = ?2 OR EXISTS (SELECT 1 FROM produto_gtin pg WHERE pg.produto_id = p.id AND pg.gtin = ?2))
                  LIMIT 1";
    const ANTES_BARRAS_DE_OUTRA_PECA: &str = "SELECT nome FROM produto p
                  WHERE p.id <> ?2
                    AND (p.codigo_barras = ?1 OR EXISTS (SELECT 1 FROM produto_gtin g WHERE g.produto_id = p.id AND g.gtin = ?1))";

    fn ean(base12: u64) -> String {
        let b = format!("{base12:012}");
        let s: u32 = b.chars().enumerate().map(|(i, c)| c.to_digit(10).unwrap() * if i % 2 == 1 { 3 } else { 1 }).sum();
        format!("{b}{}", (10 - s % 10) % 10)
    }

    fn plano(c: &Connection, sql: &str, n: usize) -> String {
        let args: Vec<String> = (0..n).map(|i| format!("x{i}")).collect();
        let mut s = c.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
        let linhas: Vec<String> = s
            .query_map(rusqlite::params_from_iter(args.iter()), |l| l.get::<_, String>(3))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
        linhas.join(" | ")
    }

    #[test]
    fn n07_as_buscas_usam_os_indices() {
        let b = Banco::em_memoria().unwrap();
        let c = &b.conn;
        for (sql, n, indices) in [
            (SQL_PECAS_PELO_GTIN, 1, &["produto_codigo_barras", "produto_gtin_codigo"][..]),
            (SQL_GTIN_DE_OUTRA_PECA, 2, &["produto_codigo_barras", "produto_gtin_codigo"][..]),
            (produtos::SQL_BARRAS_DE_OUTRA_PECA, 2, &["produto_codigo_barras", "produto_gtin_codigo"][..]),
            (crate::pessoas::SQL_PESSOA_PELO_DOCUMENTO, 1, &["pessoa_documento"][..]),
            (crate::pessoas::SQL_DOCUMENTO_DE_OUTRA, 2, &["pessoa_documento"][..]),
        ] {
            let p = plano(c, sql, n);
            for i in indices {
                assert!(
                    p.contains(&format!("USING INDEX {i}")) || p.contains(&format!("USING COVERING INDEX {i}")),
                    "{sql}\n→ {p}"
                );
            }
            assert!(
                !p.contains("SCAN produto") && !p.contains("SCAN p ") && !p.contains("SCAN pessoa"),
                "lê a tabela inteira: {sql}\n→ {p}"
            );
        }
    }

    #[test]
    fn n07_mesmo_resultado_que_as_consultas_de_antes() {
        let b = Banco::em_memoria().unwrap();
        let (c, d) = (&b.conn, b.dispositivo.as_str());
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        crate::faixas::comecar_loja(c).unwrap();
        // 60 peças: 40 com código de barras, 20 sem; GTINs extras em algumas,
        // inclusive repetindo o código de barras de outra (estado que notas
        // antigas deixaram) e GTINs que só existem em `produto_gtin`.
        let mut ids = Vec::new();
        for i in 0..60u64 {
            let mut f = produtos::ficha_nova(c).unwrap();
            f.nome = format!("PECA N07 {i}");
            f.precos[0].preco = "10,00".into();
            if i < 40 {
                f.codigo_barras = ean(789_100_000_000 + i);
            }
            ids.push(produtos::salvar(c, d, &u, &f).unwrap());
        }
        let extra = |pid: &str, g: &str, ordem: i64| {
            c.execute(
                "INSERT INTO produto_gtin (id, produto_id, gtin, tipo, origem, ordem, criado_em)
                 VALUES (?1, ?2, ?3, 'comercial', 'cEAN', ?4, '2026-11-12')",
                params![tempo::novo_id(), pid, g, ordem],
            )
            .unwrap();
        };
        for i in 0..20usize {
            extra(&ids[40 + i], &ean(789_200_000_000 + i as u64), 10);
            if i % 4 == 0 {
                extra(&ids[40 + i], &ean(789_100_000_000 + i as u64), 11); // o código de barras de outra peça
            }
            if i % 5 == 0 {
                extra(&ids[i], &ean(789_200_000_000 + i as u64), 12); // o mesmo GTIN em duas peças
            }
        }
        let mut codigos: Vec<String> = (0..45u64).map(|i| ean(789_100_000_000 + i)).collect();
        codigos.extend((0..25u64).map(|i| ean(789_200_000_000 + i)));
        let todos = |sql: &str, args: &[&dyn rusqlite::ToSql]| -> Vec<String> {
            let mut s = c.prepare(sql).unwrap();
            s.query_map(args, |l| l.get::<_, rusqlite::types::Value>(0).map(|v| format!("{v:?}")))
                .unwrap()
                .map(|x| x.unwrap())
                .collect()
        };
        let mut comparados = 0;
        for g in &codigos {
            assert_eq!(todos(ANTES_PECAS_PELO_GTIN, &[g]), todos(SQL_PECAS_PELO_GTIN, &[g]), "pelo GTIN {g}");
            for pid in ids.iter().step_by(3) {
                let antes = !todos(ANTES_GTIN_DE_OUTRA_PECA, &[pid, g]).is_empty();
                let agora = !todos(SQL_GTIN_DE_OUTRA_PECA, &[pid, g]).is_empty();
                assert_eq!(antes, agora, "de outra peça: {pid} {g}");
                let antes = !todos(ANTES_BARRAS_DE_OUTRA_PECA, &[g, pid]).is_empty();
                let agora = !todos(produtos::SQL_BARRAS_DE_OUTRA_PECA, &[g, pid]).is_empty();
                assert_eq!(antes, agora, "barras de outra peça: {g} {pid}");
                comparados += 2;
            }
        }
        assert!(comparados > 2000, "{comparados}");
        // A função inteira: o mesmo BuscaId de antes para cada código.
        for g in &codigos {
            let antes = todos(ANTES_PECAS_PELO_GTIN, &[g]);
            let agora = pecas_pelos_gtins(c, std::slice::from_ref(g)).unwrap();
            let quantos = match &agora {
                BuscaId::Nenhum => 0,
                BuscaId::Unico(_) => 1,
                BuscaId::Ambiguo(v) => v.len(),
            };
            assert_eq!(quantos, antes.len(), "{g}: {agora:?}");
        }
        // Código vazio não acha nada (as peças sem código de barras não contam).
        assert!(matches!(pecas_pelos_gtins(c, &[String::new()]).unwrap(), BuscaId::Nenhum));
    }
}
