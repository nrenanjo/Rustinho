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

use crate::erro::{Erro, Resultado};
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
    /// T111 (H3): vincula se livre; se já estiver nesta mesma peça, atualiza
    /// apenas o fator; nunca rouba o código de outra peça.
    ManterPecaAtualizarFator,
}

/// R12 (DP-41): a chave `(fornecedor, código)` está em **disputa efetiva**: dois caixas sem rede a ligaram de jeitos
/// diferentes (há um conflito aberto da troca de dados nela) e a ligação que vale e a que perdeu ainda levam a peças
/// diferentes (pelas raízes: juntar as peças acaba com a disputa) ou ao mesmo com fatores diferentes. Devolve as
/// peças candidatas (raízes, a que vale primeiro), ou `None` sem disputa efetiva. Enquanto a disputa dura, ninguém
/// grava nessa chave (os escritores daqui) e as leituras a tratam como não ligada; o gerente escolhe nas Diferenças.
pub(crate) fn fornecedor_em_disputa(
    conn: &Connection,
    fornecedor_id: &str,
    compacto: &str,
) -> Resultado<Option<Vec<String>>> {
    Ok(match situacao_do_fornecedor(conn, fornecedor_id, compacto)? {
        SituacaoFornecedor::Disputa(c) => Some(c),
        _ => None,
    })
}

/// R12: a situação de um conflito aberto na chave `(fornecedor, código)`.
#[derive(Debug, PartialEq)]
pub(crate) enum SituacaoFornecedor {
    /// Sem conflito aberto na chave.
    SemConflito,
    /// As versões levam a peças diferentes (pelas raízes) ou a fatores diferentes: a disputa efetiva.
    Disputa(Vec<String>),
    /// Eram peças diferentes e agora têm a mesma raiz (juntaram as peças), com o mesmo fator: a disputa acabou.
    Corrigida,
    /// Outro conflito da ligação (apagar contra editar, só o texto do código): o conflito comum, como antes do R12.
    Comum,
}

/// R12: classifica o conflito aberto da chave (veja [`SituacaoFornecedor`]).
pub(crate) fn situacao_do_fornecedor(
    conn: &Connection,
    fornecedor_id: &str,
    compacto: &str,
) -> Resultado<SituacaoFornecedor> {
    let perdeu: Vec<Option<String>> = conn
        .prepare_cached(
            "SELECT dados_perdeu FROM sync_conflito WHERE tabela = 'produto_fornecedor' AND resolvido_em IS NULL
               AND chave = json_array(?1, ?2)",
        )?
        .query_map(params![fornecedor_id, compacto], |l| l.get(0))?
        .collect::<Result<_, _>>()?;
    if perdeu.is_empty() {
        return Ok(SituacaoFornecedor::SemConflito);
    }
    let atual: Option<(String, i64)> = conn
        .query_row(
            "SELECT produto_id, fator FROM produto_fornecedor WHERE fornecedor_id = ?1 AND codigo_compacto = ?2",
            params![fornecedor_id, compacto],
            |l| Ok((l.get(0)?, l.get(1)?)),
        )
        .optional()?;
    // A ligação que vale foi apagada: é o conflito comum (apagar contra editar), o gerente decide como antes.
    let Some((peca, fator)) = atual else {
        return Ok(SituacaoFornecedor::Comum);
    };
    let raiz = crate::estoque::raiz(conn, &peca)?;
    let mut candidatas = vec![raiz.clone()];
    let mut efetiva = false;
    let mut juntadas = false;
    let mut comum = false;
    for dados in perdeu {
        let Some(dados) = dados else {
            comum = true; // a versão que perdeu apagava a ligação
            continue;
        };
        let v: serde_json::Value = serde_json::from_str(&dados).unwrap_or_default();
        let Some(original) = v.get("produto_id").and_then(|x| x.as_str()).filter(|x| !x.is_empty()) else {
            comum = true;
            continue;
        };
        let outra = crate::estoque::raiz(conn, original)?;
        let outro_fator = v.get("fator").and_then(|x| x.as_i64()).unwrap_or(1000);
        if outra != raiz || outro_fator != fator {
            efetiva = true;
            if !candidatas.contains(&outra) {
                candidatas.push(outra);
            }
        } else if original != peca {
            juntadas = true; // peças diferentes que agora têm a mesma raiz
        } else {
            comum = true; // a mesma peça e o mesmo fator: a diferença é outra (o texto do código)
        }
    }
    Ok(if efetiva {
        SituacaoFornecedor::Disputa(candidatas)
    } else if juntadas && !comum {
        SituacaoFornecedor::Corrigida
    } else {
        SituacaoFornecedor::Comum
    })
}

/// R12: o texto da chave em disputa (ficha, troca de código).
pub const TEXTO_EM_DISPUTA: &str = "dois caixas sem rede ligaram este código a peças diferentes. O gerente escolhe \
     qual vale em Simultaneidade › Duplicatas › Diferenças entre caixas; até lá, este código não muda nem liga sozinho.";

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
    // R12: chave em disputa efetiva: não grava (o lançar segue sem ligar, só desta vez).
    if fornecedor_em_disputa(conn, fornecedor_id, &compacto)?.is_some() {
        return Ok(false);
    }
    // R13: ligar um código a uma peça juntada liga à peça que ficou.
    let raiz = crate::estoque::raiz(conn, produto_id)?;
    let produto_id = raiz.as_str();
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
        JaLigado::ManterPecaAtualizarFator => {
            "INSERT INTO produto_fornecedor (fornecedor_id, codigo_compacto, codigo, produto_id, fator, atualizado_em)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (fornecedor_id, codigo_compacto) DO UPDATE
               SET fator = excluded.fator, atualizado_em = excluded.atualizado_em
               WHERE produto_fornecedor.produto_id = excluded.produto_id"
        }
    };
    let n = conn.execute(sql, params![fornecedor_id, compacto, codigo, produto_id, fator, tempo::agora()])?;
    Ok(n > 0)
}

/// Uma linha de "Código nos fornecedores" como a ficha da peça a manda ao salvar (pedido do proprietário,
/// 16/11/2026): o fornecedor, o código que a tela abriu (`antes`) e o que está escrito agora.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LinhaCodigoFornecedor {
    pub fornecedor_id: String,
    pub antes: String,
    pub codigo: String,
}

/// Um código de fornecedor que a ficha troca, já conferido (veja [`conferir_codigos_fornecedor`]).
#[derive(Debug, Clone)]
pub struct TrocaCodigoFornecedor {
    pub fornecedor_id: String,
    pub fornecedor: String,
    pub antes: String,
    antes_compacto: String,
    pub codigo: String,
    compacto: String,
}

impl TrocaCodigoFornecedor {
    /// A frase do histórico da peça.
    pub fn detalhe(&self) -> String {
        format!("Código no fornecedor {}: {} → {}", self.fornecedor, self.antes, self.codigo)
    }
}

/// Confere as linhas de "Código nos fornecedores" da ficha antes de gravar qualquer coisa. Só o código muda: o
/// fornecedor, a peça e o fator ficam, e nada vai para o código original nem vem dele. Linha igual à que a tela
/// abriu não é trocada (nem a que o campo cortou em 60 caracteres). Recusa (com a linha, `codigosFornecedor.<n>`):
/// código vazio, sem letras nem números ou comprido demais, a ligação que não está mais nesta peça (outro caixa mudou
/// com a tela aberta), a mesma ligação em duas linhas, o código que já é de outra peça nesse fornecedor, o que esta
/// peça já tem nele (com texto próprio quando ele também está sendo trocado: uma troca de cada vez), e o mesmo código
/// novo em duas linhas.
pub fn conferir_codigos_fornecedor(
    conn: &Connection,
    produto_id: &str,
    linhas: &[LinhaCodigoFornecedor],
) -> Resultado<Vec<TrocaCodigoFornecedor>> {
    // A linha não mudou: igual à que a tela abriu, ou o mesmo código cortado em 60 caracteres pelo campo (um código
    // gravado mais comprido não é cortado em silêncio: B-6 da revisão do Fable).
    let mudou = |l: &LinhaCodigoFornecedor| {
        let codigo = texto::limpar(&l.codigo);
        codigo != texto::limpar(&l.antes)
            && !(texto::tamanho(&l.antes) > 60
                && codigo == texto::limpar(&l.antes.chars().take(60).collect::<String>()))
    };
    // Os códigos que saem desta peça neste mesmo salvar (para a mensagem do B-4).
    let saindo: Vec<(&str, String)> = linhas
        .iter()
        .filter(|l| mudou(l) && texto::compactar(&l.codigo) != texto::compactar(&l.antes))
        .map(|l| (l.fornecedor_id.as_str(), texto::compactar(&l.antes)))
        .collect();
    let mut trocas: Vec<TrocaCodigoFornecedor> = Vec::new();
    for (i, l) in linhas.iter().enumerate() {
        let campo = format!("codigosFornecedor.{i}");
        if !mudou(l) {
            continue;
        }
        let codigo = texto::limpar(&l.codigo);
        let fornecedor: String = conn
            .query_row("SELECT nome FROM pessoa WHERE id = ?1", [&l.fornecedor_id], |r| r.get(0))
            .optional()?
            .unwrap_or_else(|| "um fornecedor sem cadastro".into());
        if texto::tamanho(&codigo) > 60 {
            return Err(Erro::campo(&campo, "Código no fornecedor: use no máximo 60 caracteres."));
        }
        let compacto = texto::compactar(&codigo);
        if codigo.is_empty() {
            return Err(Erro::campo(&campo, "O código no fornecedor não pode ficar vazio."));
        }
        if compacto.is_empty() {
            return Err(Erro::campo(&campo, "O código no fornecedor precisa ter letras ou números."));
        }
        let antes_compacto = texto::compactar(&l.antes);
        // A mesma ligação em duas linhas (a tela não manda assim): recusa com texto, e não o erro do banco (O-4).
        if trocas.iter().any(|t| t.fornecedor_id == l.fornecedor_id && t.antes_compacto == antes_compacto) {
            return Err(Erro::campo(
                &campo,
                format!("O código {} de {fornecedor} veio em duas linhas. Abra a peça de novo.", l.antes),
            ));
        }
        // R12: a ficha avisa e não grava a chave em disputa efetiva.
        for chave in [&antes_compacto, &compacto] {
            if fornecedor_em_disputa(conn, &l.fornecedor_id, chave)?.is_some() {
                return Err(Erro::campo(&campo, format!("O código de {fornecedor}: {TEXTO_EM_DISPUTA}")));
            }
        }
        let ligada: Option<String> = conn
            .query_row(
                "SELECT produto_id FROM produto_fornecedor WHERE fornecedor_id = ?1 AND codigo_compacto = ?2",
                params![l.fornecedor_id, antes_compacto],
                |r| r.get(0),
            )
            .optional()?;
        if ligada.as_deref() != Some(produto_id) {
            return Err(Erro::campo(
                &campo,
                format!("O código {} de {fornecedor} não está mais nesta peça. Abra a peça de novo.", l.antes),
            ));
        }
        if compacto != antes_compacto {
            // LEFT JOIN: a ligação de uma peça que ainda não chegou pela troca de dados também conta (O-4).
            let de_quem: Option<(String, String, Option<String>)> = conn
                .query_row(
                    "SELECT f.produto_id, f.codigo, p.nome FROM produto_fornecedor f LEFT JOIN produto p ON p.id = f.produto_id
                      WHERE f.fornecedor_id = ?1 AND f.codigo_compacto = ?2",
                    params![l.fornecedor_id, compacto],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            match de_quem {
                Some((peca, ja, _)) if peca == produto_id => {
                    // Trocar dois códigos entre si no mesmo salvar (B-4): o outro ainda está gravado aqui.
                    if saindo.iter().any(|(f, c)| *f == l.fornecedor_id && *c == compacto) {
                        return Err(Erro::campo(
                            &campo,
                            format!(
                                "O código {ja} de {fornecedor} também está sendo trocado neste salvar. Salve uma troca de cada vez."
                            ),
                        ));
                    }
                    return Err(Erro::campo(&campo, format!("Esta peça já tem o código {ja} de {fornecedor}.")));
                }
                Some((_, ja, Some(nome))) => {
                    return Err(Erro::campo(&campo, format!("O código {ja} de {fornecedor} já é da peça \"{nome}\".")));
                }
                Some((_, ja, None)) => {
                    return Err(Erro::campo(&campo, format!("O código {ja} de {fornecedor} já é de outra peça.")));
                }
                None => {}
            }
            if trocas.iter().any(|t| t.fornecedor_id == l.fornecedor_id && t.compacto == compacto) {
                return Err(Erro::campo(&campo, format!("O código {codigo} de {fornecedor} está em duas linhas.")));
            }
        }
        trocas.push(TrocaCodigoFornecedor {
            fornecedor_id: l.fornecedor_id.clone(),
            fornecedor,
            antes: l.antes.clone(),
            antes_compacto,
            codigo,
            compacto,
        });
    }
    Ok(trocas)
}

/// Grava um código de fornecedor trocado (conferido por [`conferir_codigos_fornecedor`]). Mesma forma compacta:
/// só o texto muda. Outra: apaga a ligação velha e grava a nova, com o mesmo fornecedor, peça, fator e data (a
/// chave muda, e a troca de dados registra o UPDATE só com a chave nova: o código velho ficaria nos outros
/// caixas). A data fica a de antes: corrigir o código não faz deste o código mais recente do fornecedor (o "O que
/// comprar" mostra o mais recente).
pub fn trocar_codigo_fornecedor(conn: &Connection, t: &TrocaCodigoFornecedor) -> Resultado<()> {
    // R12: nenhuma das duas chaves pode estar em disputa efetiva (o gerente escolhe antes).
    for chave in [&t.antes_compacto, &t.compacto] {
        if fornecedor_em_disputa(conn, &t.fornecedor_id, chave)?.is_some() {
            return Err(Erro::validacao(TEXTO_EM_DISPUTA));
        }
    }
    if t.compacto == t.antes_compacto {
        conn.execute(
            "UPDATE produto_fornecedor SET codigo = ?3 WHERE fornecedor_id = ?1 AND codigo_compacto = ?2",
            params![t.fornecedor_id, t.antes_compacto, t.codigo],
        )?;
        return Ok(());
    }
    let (produto_id, fator, quando): (String, i64, String) = conn.query_row(
        "SELECT produto_id, fator, atualizado_em FROM produto_fornecedor WHERE fornecedor_id = ?1 AND codigo_compacto = ?2",
        params![t.fornecedor_id, t.antes_compacto],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    conn.execute(
        "DELETE FROM produto_fornecedor WHERE fornecedor_id = ?1 AND codigo_compacto = ?2",
        params![t.fornecedor_id, t.antes_compacto],
    )?;
    conn.execute(
        "INSERT INTO produto_fornecedor (fornecedor_id, codigo_compacto, codigo, produto_id, fator, atualizado_em)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![t.fornecedor_id, t.compacto, t.codigo, produto_id, fator, quando],
    )?;
    Ok(())
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
/// T111 (Achado 5): atribuir código automático da faixa não é edição manual da
/// peça — `atualizado_em` permanece intacto (`codigo` e `codigo_validado` já
/// disparam o gatilho de sincronização).
pub fn definir_codigo_interno(
    conn: &Connection,
    id: &str,
    codigo: &str,
    (busca, codigos): &(String, String),
    _agora: &str,
    dispositivo: &str,
) -> Resultado<bool> {
    let n = conn.execute(
        "UPDATE produto SET codigo = ?2, busca = ?3, codigos = ?4, dispositivo = ?5,
                codigo_validado = 1
          WHERE id = ?1 AND codigo = ''",
        params![id, codigo, busca, codigos, dispositivo],
    )?;
    Ok(n == 1)
}

// ---------------------------------------------------------------------------
// Índice de busca da peça (`produto.busca` e `produto.codigos`)
// ---------------------------------------------------------------------------

/// Uma aplicação (carro) da peça, como entra no índice de busca.
#[derive(Debug, Default, Clone)]
pub struct CarroDoIndice {
    pub montadora: String,
    pub modelo: String,
    pub motor: String,
    pub observacao: String,
}

/// O que entra no índice de busca de uma peça: a peça inteira (nome, códigos,
/// GTINs, carros e equivalências). Quem tem a peça na mão (a ficha, ao
/// salvar) monta isto; quem só tem o id usa [`refazer_indice`].
#[derive(Debug, Default, Clone)]
pub struct TextoDaPeca {
    pub nome: String,
    pub marca: String,
    pub codigo: String,
    pub codigo_barras: String,
    pub codigo_original: String,
    pub categoria: String,
    pub local: String,
    pub variacao: String,
    pub observacoes: String,
    /// Os GTINs na ordem da peça (o código de barras primeiro, se não estiver
    /// entre os comerciais; veja [`refazer_indice`]).
    pub gtins: Vec<String>,
    pub aplicacoes: Vec<CarroDoIndice>,
    /// (marca, código) de cada equivalência.
    pub equivalentes: Vec<(String, String)>,
}

/// Texto de busca da peça: nome primeiro (para a ordenação), depois o resto.
/// Devolve (`produto.busca`, `produto.codigos`). Veio de `produtos` (passo
/// 5.2, "mover sem mudar"): a troca de dados refaz o índice da peça que
/// recebe, e `produtos` não pode ser chamado de lá (ciclo).
pub fn indice(d: &TextoDaPeca) -> (String, String) {
    let mut partes: Vec<String> = vec![
        d.nome.clone(),
        d.marca.clone(),
        d.codigo.clone(),
        d.codigo_barras.clone(),
        d.codigo_original.clone(),
        d.categoria.clone(),
        d.local.clone(),
        d.variacao.clone(),
        d.observacoes.clone(),
    ];
    for g in &d.gtins {
        partes.push(g.clone());
    }
    for a in &d.aplicacoes {
        partes.push(a.montadora.clone());
        for s in texto::sinonimos_montadora(&texto::normalizar(&a.montadora)) {
            partes.push(s.to_string());
        }
        partes.push(a.modelo.clone());
        partes.push(a.motor.clone());
        partes.push(a.observacao.clone());
    }
    for (marca, codigo) in &d.equivalentes {
        partes.push(marca.clone());
        partes.push(codigo.clone());
    }
    let mut codigos: Vec<String> = Vec::new();
    for c in [&d.codigo, &d.codigo_barras, &d.codigo_original]
        .into_iter()
        .chain(d.gtins.iter())
        .chain(d.equivalentes.iter().map(|q| &q.1))
    {
        let compacto = texto::compactar(c);
        if !compacto.is_empty() && !codigos.contains(&compacto) {
            codigos.push(compacto);
        }
    }
    let busca = format!("{} {}", texto::normalizar(&partes.join(" ")), codigos.join(" "));
    (busca.trim().to_string(), format!(" {} ", codigos.join(" ")))
}

/// Lê a peça gravada e refaz o índice de busca dela e o nome sem acento
/// (`nome_normalizado`, 0025), o mesmo que a ficha grava ao salvar: são as
/// colunas derivadas da peça (`dados::esquema::COLUNAS`), que não viajam e
/// que quem recebe a peça refaz. `false` = a peça não existe (nada a fazer).
pub fn refazer_indice(conn: &Connection, produto_id: &str) -> Resultado<bool> {
    let Some(mut d) = conn
        .query_row(
            "SELECT codigo, nome, marca, codigo_barras, codigo_original, categoria, local, variacao, observacoes
               FROM produto WHERE id = ?1",
            [produto_id],
            |l| {
                Ok(TextoDaPeca {
                    codigo: l.get(0)?,
                    nome: l.get(1)?,
                    marca: l.get(2)?,
                    codigo_barras: l.get(3)?,
                    codigo_original: l.get(4)?,
                    categoria: l.get(5)?,
                    local: l.get(6)?,
                    variacao: l.get(7)?,
                    observacoes: l.get(8)?,
                    ..Default::default()
                })
            },
        )
        .optional()?
    else {
        return Ok(false);
    };
    let mut c = conn.prepare_cached(
        "SELECT montadora, modelo, motor, observacao FROM produto_aplicacao WHERE produto_id = ?1 ORDER BY ordem",
    )?;
    d.aplicacoes = c
        .query_map([produto_id], |l| {
            Ok(CarroDoIndice { montadora: l.get(0)?, modelo: l.get(1)?, motor: l.get(2)?, observacao: l.get(3)? })
        })?
        .collect::<Result<_, _>>()?;
    let mut c =
        conn.prepare_cached("SELECT marca, codigo FROM produto_equivalente WHERE produto_id = ?1 ORDER BY ordem")?;
    d.equivalentes = c.query_map([produto_id], |l| Ok((l.get(0)?, l.get(1)?)))?.collect::<Result<_, _>>()?;
    // Como a ficha lê os GTINs: o código de barras entra primeiro, como
    // comercial, se não estiver lá.
    let mut c =
        conn.prepare_cached("SELECT gtin, tipo FROM produto_gtin WHERE produto_id = ?1 ORDER BY ordem, rowid")?;
    let gtins: Vec<(String, String)> =
        c.query_map([produto_id], |l| Ok((l.get(0)?, l.get(1)?)))?.collect::<Result<_, _>>()?;
    let barras_entre_os_comerciais = gtins.iter().any(|(g, t)| *g == d.codigo_barras && t == "comercial");
    d.gtins = gtins.into_iter().map(|(g, _)| g).collect();
    if !d.codigo_barras.is_empty() && !barras_entre_os_comerciais {
        d.gtins.insert(0, d.codigo_barras.clone());
    }
    gravar_indice(conn, produto_id, &indice(&d))?;
    conn.execute(
        "UPDATE produto SET nome_normalizado = ?2 WHERE id = ?1",
        params![produto_id, texto::normalizar(&d.nome)],
    )?;
    Ok(true)
}

/// Refaz o índice de busca (e o nome sem acento) de todas as peças. Só mexe
/// nas colunas derivadas, que não viajam: não grava alteração da troca. Usado
/// uma vez, na primeira abertura com a 5.2 (R-1, veja
/// `troca::sync::registro`). Devolve quantas peças.
pub fn refazer_todos_os_indices(conn: &Connection) -> Resultado<usize> {
    let ids: Vec<String> = conn
        .prepare("SELECT id FROM produto ORDER BY rowid")?
        .query_map([], |l| l.get(0))?
        .collect::<Result<_, _>>()?;
    for id in &ids {
        refazer_indice(conn, id)?;
    }
    Ok(ids.len())
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
        "SELECT f.fornecedor_id, f.produto_id FROM produto_fornecedor f JOIN pessoa p ON p.id = f.fornecedor_id
          WHERE p.documento = ?1 AND f.codigo_compacto = ?2
          ORDER BY f.produto_id, f.fornecedor_id",
    )?;
    let linhas: Vec<(String, String)> =
        stmt.query_map(params![fornecedor_doc, compacto], |l| Ok((l.get(0)?, l.get(1)?)))?.collect::<Result<_, _>>()?;
    // R12: a chave em disputa efetiva não liga sozinha: as candidatas viram ambiguidade (pendência).
    let mut achados: Vec<String> = Vec::new();
    let mut disputa = false;
    for (fornecedor_id, peca) in linhas {
        let pecas = match fornecedor_em_disputa(conn, &fornecedor_id, &compacto)? {
            Some(c) => {
                disputa = true;
                c
            }
            None => vec![peca],
        };
        for p in pecas {
            if !achados.contains(&p) {
                achados.push(p);
            }
        }
    }
    if disputa && achados.len() > 1 {
        return Ok(BuscaId::Ambiguo(achados));
    }
    Ok(por_quantidade(achados))
}

/// A peça e o fator do vínculo do fornecedor (pelo id dele), pela forma
/// compacta do código.
pub(crate) fn vinculo_do_fornecedor(
    conn: &Connection,
    fornecedor_id: &str,
    compacto: &str,
) -> Resultado<Option<(String, i64)>> {
    // R12: a chave em disputa efetiva é tratada como não ligada.
    if fornecedor_em_disputa(conn, fornecedor_id, compacto)?.is_some() {
        return Ok(None);
    }
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

/// T111 (Decisão D-T108 Opção C): o item de uma nota de fornecedor tem
/// identidade forte para cadastro automático? Exige GTIN válido (comercial ou
/// tributável) OU código do fornecedor com pelo menos 3 caracteres
/// alfanuméricos compactados. Código vazio, só símbolos ou muito curto sem
/// GTIN vai para registro provisório (`xml_pendencia_identificacao`).
pub(crate) fn tem_identidade_forte(gtins: &[GtinFicha], gtin_fallback: &str, codigo_fornecedor: &str) -> bool {
    if !gtin_fallback.is_empty() && gtin::gtin_valido(gtin_fallback) {
        return true;
    }
    if gtins.iter().any(|g| codigo_do_xml(g).is_some_and(|(cod, _, _)| gtin::gtin_valido(&cod))) {
        return true;
    }
    texto::compactar(codigo_fornecedor).len() >= 3
}

/// T111 (Achado 10): consulta sobre o índice de cobertura `produto_texto_de_busca`
/// (`busca`, `codigos`, `nome`, `id`, `ativo`) pelo 1º caractere alfanumérico de
/// `busca`, sem varrer a tabela `produto`.
pub(crate) const SQL_PECAS_POR_PREFIXO_BUSCA: &str =
    "SELECT id, nome, ativo FROM produto WHERE busca >= ?1 AND busca < ?2";

/// T111 (Regra 4 — antiduplicidade): peças existentes cuja descrição compacta
/// é idêntica à do item da nota (limitado a `CANDIDATOS_MAXIMOS`, priorizando
/// peças ativas). Nunca funde automaticamente: serve para encaminhar o item a
/// registro provisório com as peças candidatas quando não há GTIN nem vínculo.
pub(crate) fn pecas_por_descricao_compacta(conn: &Connection, descricao: &str) -> Resultado<Vec<String>> {
    // O cadastro guarda até 120 caracteres limpos do nome da peça; compara na
    // mesma janela para não deixar escapar duplicatas com descrição comprida.
    let alvo_120: String = texto::limpar(descricao).chars().take(120).collect();
    let alvo = texto::compactar(&alvo_120);
    let Some(primeiro) = alvo.chars().next() else {
        return Ok(Vec::new());
    };
    let inicio = primeiro.to_string();
    let mut achados: Vec<(bool, String)> = Vec::new();
    let mut avaliar = |id: String, nome: String, ativo: i64| -> Resultado<()> {
        if texto::compactar(&nome) == alvo {
            if ativo == 0 {
                // V2-2: peça provisória desfeita em ambiente multi-caixa não é cadastro
                // real e não deve bloquear reimportação futura do mesmo XML.
                let desfeita: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM auditoria
                       WHERE entidade = 'produto' AND entidade_id = ?1 AND acao = 'produto.desfazer_provisorio')
                       AND NOT EXISTS(SELECT 1 FROM auditoria
                       WHERE entidade = 'produto' AND entidade_id = ?1
                         AND acao IN ('produto.alterar', 'compra.entrada'))",
                    [&id],
                    |l| l.get(0),
                )?;
                if desfeita {
                    return Ok(());
                }
            }
            achados.push((ativo != 0, id));
        }
        Ok(())
    };
    if let Some(proximo) = char::from_u32(primeiro as u32 + 1) {
        let fim = proximo.to_string();
        let mut stmt = conn.prepare_cached(SQL_PECAS_POR_PREFIXO_BUSCA)?;
        let linhas = stmt.query_map(params![inicio, fim], |l| {
            Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?, l.get::<_, i64>(2)?))
        })?;
        for linha in linhas {
            let (id, nome, ativo) = linha?;
            avaliar(id, nome, ativo)?;
        }
    } else {
        let mut stmt = conn.prepare_cached("SELECT id, nome, ativo FROM produto WHERE busca >= ?1")?;
        let linhas = stmt.query_map(params![inicio], |l| {
            Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?, l.get::<_, i64>(2)?))
        })?;
        for linha in linhas {
            let (id, nome, ativo) = linha?;
            avaliar(id, nome, ativo)?;
        }
    }
    achados.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    achados.truncate(CANDIDATOS_MAXIMOS);
    Ok(achados.into_iter().map(|(_, id)| id).collect())
}

/// T111 (A5 — limpeza anti-órfão): remove o vínculo `(fornecedor_id, codigo)`
/// somente se ainda apontar para `produto_id`. Fica neste módulo porque
/// `codigos` é o único escritor autorizado de `produto_fornecedor`.
pub(crate) fn desvincular_fornecedor_se_produto(
    conn: &Connection,
    fornecedor_id: &str,
    codigo: &str,
    produto_id: &str,
) -> Resultado<bool> {
    let compacto = texto::compactar(codigo);
    if fornecedor_id.is_empty() || compacto.is_empty() {
        return Ok(false);
    }
    // R12: chave em disputa efetiva fica como está até o gerente escolher.
    if fornecedor_em_disputa(conn, fornecedor_id, &compacto)?.is_some() {
        return Ok(false);
    }
    let n = conn.execute(
        "DELETE FROM produto_fornecedor
          WHERE fornecedor_id = ?1 AND codigo_compacto = ?2 AND produto_id = ?3",
        params![fornecedor_id, compacto, produto_id],
    )?;
    Ok(n > 0)
}

/// T111 (V2-2): remove os GTINs auxiliares (`produto_gtin`) de uma peça
/// provisória desativada ao desfazer rascunho em ambiente multi-caixa para não
/// bloquear a reimportação do mesmo XML. Fica em `codigos` (único escritor de
/// `produto_gtin`).
/// R13 (DP-42): os códigos da peça juntada (D) passam para a raiz (K), sem repetir (idempotente; dois caixas com a
/// mesma visão fazem o mesmo): as ligações do código do fornecedor, os GTINs, o código de barras principal (vira GTIN
/// de K se K já tem o seu), os códigos antigos e o código interno de D (vira código antigo de K). D fica sem GTIN nem
/// código de barras. Refaz o índice de busca das duas.
pub fn passar_para_juncao(conn: &Connection, d: &str, k: &str) -> Resultado<()> {
    let agora = tempo::agora();
    conn.execute(
        "UPDATE produto_fornecedor SET produto_id = ?2, atualizado_em = ?3 WHERE produto_id = ?1",
        params![d, k, agora],
    )?;
    // GTINs: o que K já tem igual sai de D; o resto passa.
    conn.execute(
        "DELETE FROM produto_gtin WHERE produto_id = ?1 AND EXISTS (SELECT 1 FROM produto_gtin x
           WHERE x.produto_id = ?2 AND x.gtin = produto_gtin.gtin AND x.tipo = produto_gtin.tipo
             AND x.origem = produto_gtin.origem)",
        params![d, k],
    )?;
    conn.execute("UPDATE produto_gtin SET produto_id = ?2 WHERE produto_id = ?1", params![d, k])?;
    // O código de barras principal de D: K fica com ele se não tem o seu; senão, vira GTIN manual de K.
    let (barras_d, codigo_d): (String, String) =
        conn.query_row("SELECT codigo_barras, codigo FROM produto WHERE id = ?1", [d], |l| Ok((l.get(0)?, l.get(1)?)))?;
    if !barras_d.is_empty() {
        let barras_k: String = conn.query_row("SELECT codigo_barras FROM produto WHERE id = ?1", [k], |l| l.get(0))?;
        if barras_k.is_empty() {
            conn.execute("UPDATE produto SET codigo_barras = ?2 WHERE id = ?1", params![k, barras_d])?;
        } else if barras_k != barras_d {
            conn.execute(
                "INSERT OR IGNORE INTO produto_gtin (id, produto_id, gtin, tipo, origem, ordem, criado_em)
                 VALUES (?1, ?2, ?3, 'comercial', 'manual', 99, ?4)",
                params![format!("juntar:{d}:{barras_d}"), k, barras_d, agora],
            )?;
        }
        conn.execute("UPDATE produto SET codigo_barras = '' WHERE id = ?1", [d])?;
    }
    // Os códigos antigos de D e o código interno dele viram códigos antigos de K.
    conn.execute(
        "DELETE FROM produto_codigo_antigo WHERE produto_id = ?1
           AND codigo IN (SELECT codigo FROM produto_codigo_antigo WHERE produto_id = ?2)",
        params![d, k],
    )?;
    conn.execute("UPDATE produto_codigo_antigo SET produto_id = ?2 WHERE produto_id = ?1", params![d, k])?;
    let codigo_k: String = conn.query_row("SELECT codigo FROM produto WHERE id = ?1", [k], |l| l.get(0))?;
    if !codigo_d.is_empty() && codigo_d != codigo_k {
        guardar_codigo_antigo(conn, &codigo_d, k, &agora)?;
    }
    refazer_indice(conn, k)?;
    refazer_indice(conn, d)?;
    Ok(())
}

pub(crate) fn limpar_gtins_do_produto(conn: &Connection, produto_id: &str) -> Resultado<()> {
    conn.execute("DELETE FROM produto_gtin WHERE produto_id = ?1", [produto_id])?;
    Ok(())
}

#[cfg(test)]
mod testes_indice {
    //! O índice refeito pelo id é o mesmo que a ficha grava ao salvar (passo
    //! 5.2: o texto do índice veio de `produtos` sem mudar).
    use super::*;
    use crate::banco::Banco;
    use crate::{produtos, usuarios};

    #[test]
    fn refazer_pelo_id_e_igual_ao_da_ficha() {
        let b = Banco::em_memoria().unwrap();
        let (c, d) = (&b.conn, b.dispositivo.as_str());
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        crate::faixas::comecar_loja(c).unwrap();
        let mut f = produtos::ficha_nova(c).unwrap();
        f.nome = "Pastilha de Freio Dianteira".into();
        f.marca = "Cobreq".into();
        f.codigo_barras = "7891000000090".into();
        f.codigo_original = "N-1234/A".into();
        f.categoria = "Freios".into();
        f.local = "Prateleira 3".into();
        f.observacoes = "Jogo com 4".into();
        f.precos[0].preco = "10,00".into();
        f.gtins = vec![
            GtinFicha { gtin: "7891000000113".into(), tipo: "comercial".into(), origem: "manual".into() },
            GtinFicha { gtin: "7891000000120".into(), tipo: "tributavel".into(), origem: "manual".into() },
        ];
        f.aplicacoes = vec![
            produtos::AplicacaoFicha {
                montadora: "VW".into(),
                modelo: "Gol".into(),
                motor: "1.6".into(),
                ..Default::default()
            },
            produtos::AplicacaoFicha { montadora: "Chevrolet".into(), modelo: "Corsa".into(), ..Default::default() },
        ];
        f.equivalentes = vec![produtos::EquivalenteFicha { marca: "Fras-le".into(), codigo: "PD/580".into() }];
        let id = produtos::salvar(c, d, &u, &f).unwrap();
        let ler = || -> (String, String) {
            c.query_row("SELECT busca, codigos FROM produto WHERE id = ?1", [&id], |l| Ok((l.get(0)?, l.get(1)?)))
                .unwrap()
        };
        let da_ficha = ler();
        assert!(da_ficha.1.contains("7891000000113") && da_ficha.0.contains("corsa"), "{da_ficha:?}");
        c.execute("UPDATE produto SET busca = '', codigos = '' WHERE id = ?1", [&id]).unwrap();
        assert!(refazer_indice(c, &id).unwrap());
        assert_eq!(ler(), da_ficha);
        assert!(!refazer_indice(c, "nao-existe").unwrap());
    }
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
            (SQL_PECAS_POR_PREFIXO_BUSCA, 2, &["produto_texto_de_busca"][..]),
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

    /// T111 (Achado 10): `pecas_por_descricao_compacta` encontra variações de
    /// pontuação/acento pelo índice de cobertura e prioriza peças ativas.
    #[test]
    fn t111_h10_descricao_compacta_prioriza_ativas_e_casa_pontuacao() {
        let b = Banco::em_memoria().unwrap();
        let (c, d) = (&b.conn, b.dispositivo.as_str());
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        crate::faixas::comecar_loja(c).unwrap();

        let mut f1 = produtos::ficha_nova(c).unwrap();
        f1.nome = "Filtro de Óleo PH-5949".into();
        f1.precos[0].preco = "25,00".into();
        let id_inativa = produtos::salvar(c, d, &u, &f1).unwrap();
        c.execute("UPDATE produto SET ativo = 0 WHERE id = ?1", [&id_inativa]).unwrap();

        let mut f2 = produtos::ficha_nova(c).unwrap();
        f2.nome = "FILTRO-DE-OLEO PH5949".into();
        f2.precos[0].preco = "25,00".into();
        let id_ativa = produtos::salvar(c, d, &u, &f2).unwrap();

        let achados = pecas_por_descricao_compacta(c, "FILTRO DE OLEO PH 5949").unwrap();
        assert_eq!(achados, vec![id_ativa, id_inativa]);
    }
}
