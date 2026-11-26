//! Regras fiscais da peça (Simples Nacional): substituição tributária do
//! ICMS, CSOSN, CFOP e PIS/Cofins monofásico.
//!
//! A ST depende do estado, do CEST da peça e da data. As regras ficam na
//! tabela `regra_st`, que o contador pode mudar. Exemplo: em São Paulo as
//! autopeças (CEST 01) saíram da ST em 01/10/2026; a partir dessa data a
//! venda dentro do estado deixa de ser CSOSN 500 / CFOP 5405 e passa a ser
//! CSOSN 102 / CFOP 5102.
//!
//! Importante: o Rustinho sugere e avisa; quem decide é o contador.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::documento::UFS;
use crate::erro::{Erro, Resultado};
use crate::{auditoria, config, documento, tempo};

/// CSOSN que podem ser escolhidos na peça (vazio = automático).
pub const CSOSNS: &[(&str, &str)] = &[
    ("", "Automático (pela regra do estado)"),
    ("102", "102 · Sem ST, tributada pelo Simples"),
    ("500", "500 · ICMS já pago por substituição tributária"),
    ("103", "103 · Isenção para faixa de receita"),
    ("201", "201 · Com crédito e com ST"),
    ("202", "202 · Sem crédito e com ST"),
    ("203", "203 · Isenção para faixa de receita e com ST"),
    ("300", "300 · Imune"),
    ("400", "400 · Não tributada"),
    ("900", "900 · Outros"),
];

/// O que o Rustinho decide para a venda de uma peça dentro do estado.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tributacao {
    pub csosn: String,
    pub cfop: String,
    /// true = escolhido automaticamente pela regra
    pub automatico: bool,
    /// Some(true) com ST, Some(false) sem ST, None sem regra para o CEST.
    pub com_st: Option<bool>,
    /// Frase curta para mostrar na ficha.
    pub explicacao: String,
    /// Aviso quando o CSOSN escolhido à mão não combina com a regra.
    pub alerta: String,
}

/// Data de hoje no fuso do computador, "AAAA-MM-DD".
pub fn hoje(conn: &Connection) -> Resultado<String> {
    Ok(conn.query_row("SELECT date('now', 'localtime')", [], |l| l.get(0))?)
}

/// Data (no fuso do computador) de um momento gravado em UTC, "dd/mm/aaaa".
pub fn data_local_br(conn: &Connection, iso_utc: &str) -> Resultado<String> {
    let d: Option<String> = conn.query_row("SELECT date(?1, 'localtime')", [iso_utc], |l| l.get(0))?;
    Ok(d.map(|d| data_br(&d)).unwrap_or_default())
}

/// Estado da loja (configuração), em maiúsculas. Vazio se não informado.
pub fn uf_loja(conn: &Connection) -> Resultado<String> {
    Ok(config::ler(conn, "loja_uf")?.unwrap_or_default())
}

/// A peça tem ST nesse estado e nessa data? Vale a regra com o início de
/// CEST mais longo que combina (ex.: "01.001" vence "01").
pub fn situacao_st(conn: &Connection, uf: &str, cest: &str, data: &str) -> Resultado<Option<(bool, String)>> {
    let cest = documento::digitos(cest);
    if cest.is_empty() || uf.is_empty() {
        return Ok(None);
    }
    let r = conn
        .query_row(
            "SELECT com_st, descricao FROM regra_st
              WHERE uf = ?1 AND ?2 LIKE cest_inicio || '%'
                AND vigente_de <= ?3 AND (vigente_ate IS NULL OR vigente_ate >= ?3)
              ORDER BY length(cest_inicio) DESC, vigente_de DESC
              LIMIT 1",
            params![uf, cest, data],
            |l| Ok((l.get::<_, bool>(0)?, l.get::<_, String>(1)?)),
        )
        .optional()?;
    Ok(r)
}

/// Decide CSOSN e CFOP da venda dentro do estado.
pub fn tributacao(conn: &Connection, uf: &str, cest: &str, csosn_peca: &str, data: &str) -> Resultado<Tributacao> {
    let regra = situacao_st(conn, uf, cest, data)?;
    let com_st = regra.as_ref().map(|r| r.0);
    let (csosn_auto, explicacao) = match &regra {
        Some((true, d)) => ("500", d.clone()),
        Some((false, d)) => ("102", d.clone()),
        None if uf.is_empty() => ("102", "Informe o estado da loja em Configurações para conferir a ST.".to_string()),
        None if documento::digitos(cest).is_empty() => ("102", "Sem CEST: tratada como peça sem ST.".to_string()),
        None => ("102", format!("Nenhuma regra de ST para este CEST em {uf}.")),
    };
    let automatico = csosn_peca.is_empty();
    let csosn = if automatico { csosn_auto.to_string() } else { csosn_peca.to_string() };
    // 500: a ST já foi paga antes, a loja é a substituída (CFOP 5.405). 201, 202 e 203: a loja cobra a ST, é a
    // substituta (CFOP 5.403). Tabela de CFOP do CONFAZ; CSOSN do MOC 7.0, Anexo I.
    let cfop = match csosn.as_str() {
        "500" => "5405",
        "201" | "202" | "203" => "5403",
        _ => "5102",
    }
    .to_string();
    let alerta = match (csosn.as_str(), com_st) {
        ("500", Some(false)) => format!(
            "Esta peça está com CSOSN 500, mas pela regra de {uf} ela não tem mais ST nesta data. O certo deve ser 102 (CFOP 5102). Confirme com o contador."
        ),
        ("102", Some(true)) if !automatico => format!(
            "Esta peça está com CSOSN 102, mas pela regra de {uf} ela tem ST nesta data. O certo deve ser 500 (CFOP 5405). Confirme com o contador."
        ),
        _ => String::new(),
    };
    Ok(Tributacao { csosn, cfop, automatico, com_st, explicacao, alerta })
}

// ---------------------------------------------------------------------------
// Conferência fiscal: lista o que precisa de atenção.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PecaFiscal {
    pub id: String,
    pub codigo: String,
    pub nome: String,
    pub marca: String,
    pub ncm: String,
    pub cest: String,
    pub csosn: String,
    pub sugerido: String,
    pub cfop: String,
    pub motivo: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conferencia {
    pub uf: String,
    pub data: String,
    /// CSOSN escolhido à mão que não combina com a regra.
    pub conflitos: Vec<PecaFiscal>,
    /// Peças ativas sem NCM (a nota não sai sem NCM).
    pub sem_ncm: Vec<PecaFiscal>,
    /// Peças sem CEST cuja NCM está na lista oficial de autopeças ou pneus (Conv. 142/18): "a analisar".
    pub sem_cest: Vec<PecaFiscal>,
    pub monofasicas: i64,
    pub total_pecas: i64,
    pub regras: Vec<RegraSt>,
}

/// Monta a conferência das peças ativas.
pub fn conferir(conn: &Connection, data: &str) -> Resultado<Conferencia> {
    let uf = uf_loja(conn)?;
    let mut consulta = conn.prepare(
        "SELECT id, codigo, nome, marca, ncm, cest, csosn FROM produto WHERE ativo = 1 AND tipo <> 'servico'
          ORDER BY nome COLLATE NOCASE",
    )?;
    let pecas: Vec<(String, String, String, String, String, String, String)> = consulta
        .query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?, l.get(6)?)))?
        .collect::<Result<_, _>>()?;
    let mut conflitos = Vec::new();
    let mut sem_ncm = Vec::new();
    let mut sem_cest = Vec::new();
    for (id, codigo, nome, marca, ncm, cest, csosn) in &pecas {
        let t = tributacao(conn, &uf, cest, csosn, data)?;
        let item = |motivo: String| PecaFiscal {
            id: id.clone(),
            codigo: codigo.clone(),
            nome: nome.clone(),
            marca: marca.clone(),
            ncm: ncm.clone(),
            cest: cest.clone(),
            csosn: if csosn.is_empty() { format!("{} (automático)", t.csosn) } else { csosn.clone() },
            sugerido: match t.com_st {
                Some(true) => "500".into(),
                Some(false) => "102".into(),
                None => t.csosn.clone(),
            },
            cfop: t.cfop.clone(),
            motivo,
        };
        if !t.alerta.is_empty() {
            conflitos.push(item(t.explicacao.clone()));
        }
        if ncm.is_empty() {
            sem_ncm.push(item("Sem NCM".into()));
        } else if cest.is_empty() {
            let candidatos = cests_do_ncm(ncm);
            if !candidatos.is_empty() {
                sem_cest.push(item(format!(
                    "A analisar: o NCM está na lista oficial (CEST {})",
                    candidatos.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
                )));
            }
        }
    }
    let monofasicas: i64 =
        conn.query_row("SELECT COUNT(*) FROM produto WHERE ativo = 1 AND monofasico = 1", [], |l| l.get(0))?;
    Ok(Conferencia {
        uf,
        data: data.to_string(),
        conflitos,
        sem_ncm,
        sem_cest,
        monofasicas,
        total_pecas: pecas.len() as i64,
        regras: regras(conn)?,
    })
}

/// G4 (0.9.19, etapa 3): a lista oficial dos CEST de autopeças e de pneus
/// (Conv. ICMS 142/18, Anexos II e XVI, lida no CONFAZ em 25/11/2026), com o
/// NCM de cada um. Serve para lembrar de conferir o CEST: não decide nada
/// (a mesma NCM pode ser de uso automotivo ou não).
#[derive(Deserialize)]
struct ListaCest {
    itens: Vec<ItemCest>,
}

#[derive(Deserialize)]
struct ItemCest {
    cest: String,
    ncm: String,
}

fn lista_cest() -> &'static [(String, String)] {
    static LISTA: std::sync::OnceLock<Vec<(String, String)>> = std::sync::OnceLock::new();
    LISTA.get_or_init(|| {
        let lista: ListaCest =
            serde_json::from_str(include_str!("cest_142_18.json")).expect("cest_142_18.json é um arquivo do programa");
        lista
            .itens
            .into_iter()
            .flat_map(|i| {
                let cest = i.cest;
                i.ncm
                    .split_whitespace()
                    .map(|n| documento::digitos(n))
                    .filter(|n| !n.is_empty())
                    .map(move |n| (n, cest.clone()))
                    .collect::<Vec<_>>()
            })
            .collect()
    })
}

/// Os CEST da lista oficial cuja NCM começa a NCM da peça ("01.002.00").
pub fn cests_do_ncm(ncm: &str) -> Vec<String> {
    let ncm = documento::digitos(ncm);
    let mut v: Vec<String> = lista_cest()
        .iter()
        .filter(|(prefixo, _)| !ncm.is_empty() && ncm.starts_with(prefixo.as_str()))
        .map(|(_, cest)| cest.clone())
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Passa as peças indicadas para CSOSN automático (segue a regra do estado).
/// Devolve quantas mudaram.
pub fn usar_automatico(conn: &Connection, dispositivo: &str, usuario_id: &str, ids: &[String]) -> Resultado<usize> {
    let mut mudou = 0;
    for id in ids {
        let antes: Option<String> =
            conn.query_row("SELECT csosn FROM produto WHERE id = ?1", [id], |l| l.get(0)).optional()?;
        let Some(antes) = antes else { continue };
        if antes.is_empty() {
            continue;
        }
        conn.execute(
            "UPDATE produto SET csosn = '', atualizado_em = ?2, atualizado_por = ?3, dispositivo = ?4 WHERE id = ?1",
            params![id, tempo::agora(), usuario_id, dispositivo],
        )?;
        auditoria::registrar(
            conn,
            dispositivo,
            Some(usuario_id),
            "produto.alterar",
            "produto",
            id,
            &[format!("CSOSN: {antes} → automático (conferência fiscal)")],
        )?;
        mudou += 1;
    }
    Ok(mudou)
}

// ---------------------------------------------------------------------------
// Regras de ST (o contador pode mudar). FIS-P5 e FIS-P6 (0.9.19, etapa 3):
// cada regra tem a fonte; a regra que já valeu não se exclui nem se reescreve
// (encerra-se a vigência, e o histórico fica); a linha mudada à mão vira da
// loja, e uma atualização do programa não a sobrescreve.
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RegraSt {
    pub id: String,
    pub uf: String,
    pub cest_inicio: String,
    /// "dd/mm/aaaa"
    pub vigente_de: String,
    pub vigente_ate: String,
    pub com_st: bool,
    pub descricao: String,
    /// O ato e o endereço oficial.
    pub fonte: String,
    /// O artigo, anexo ou item.
    pub dispositivo: String,
    /// "dd/mm/aaaa": quando a fonte foi lida.
    pub consultada_em: String,
    /// "rustinho" (veio com o programa) ou "loja" (mudada à mão). Só leitura.
    pub origem: String,
    /// "vigente", "futura" ou "encerrada", na data de hoje. Só leitura.
    pub situacao: String,
    /// A regra já valeu (começou hoje ou antes): não se exclui, e o estado, o
    /// CEST, o início e o "com ST" não mudam. Só leitura.
    pub ja_valeu: bool,
}

fn situacao(de: &str, ate: Option<&str>, hoje: &str) -> &'static str {
    if de > hoje {
        "futura"
    } else if ate.is_some_and(|a| a < hoje) {
        "encerrada"
    } else {
        "vigente"
    }
}

/// O dia de ontem, "AAAA-MM-DD" (para encerrar uma vigência).
fn ontem(conn: &Connection) -> Resultado<String> {
    Ok(conn.query_row("SELECT date('now', 'localtime', '-1 day')", [], |l| l.get(0))?)
}

pub fn regras(conn: &Connection) -> Resultado<Vec<RegraSt>> {
    let hoje = hoje(conn)?;
    let mut c = conn.prepare(
        "SELECT id, uf, cest_inicio, vigente_de, vigente_ate, com_st, descricao, fonte, dispositivo, consultada_em,
                origem
           FROM regra_st ORDER BY uf, cest_inicio, vigente_de",
    )?;
    let linhas = c.query_map([], |l| {
        let de: String = l.get(3)?;
        let ate: Option<String> = l.get(4)?;
        let consultada: String = l.get(9)?;
        Ok(RegraSt {
            id: l.get(0)?,
            uf: l.get(1)?,
            cest_inicio: l.get(2)?,
            vigente_de: data_br(&de),
            vigente_ate: ate.as_deref().map(data_br).unwrap_or_default(),
            com_st: l.get(5)?,
            descricao: l.get(6)?,
            fonte: l.get(7)?,
            dispositivo: l.get(8)?,
            consultada_em: if consultada.is_empty() { String::new() } else { data_br(&consultada) },
            origem: l.get(10)?,
            situacao: situacao(&de, ate.as_deref(), &hoje).to_string(),
            ja_valeu: de <= hoje,
        })
    })?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

/// Uma regra como está no banco (datas "AAAA-MM-DD").
struct Guardada {
    uf: String,
    cest: String,
    de: String,
    ate: Option<String>,
    com_st: bool,
    descricao: String,
    fonte: String,
    dispositivo: String,
}

fn guardada(conn: &Connection, id: &str) -> Resultado<Guardada> {
    conn.query_row(
        "SELECT uf, cest_inicio, vigente_de, vigente_ate, com_st, descricao, fonte, dispositivo FROM regra_st WHERE id = ?1",
        [id],
        |l| {
            Ok(Guardada {
                uf: l.get(0)?,
                cest: l.get(1)?,
                de: l.get(2)?,
                ate: l.get(3)?,
                com_st: l.get(4)?,
                descricao: l.get(5)?,
                fonte: l.get(6)?,
                dispositivo: l.get(7)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| Erro::NaoEncontrado("Regra de ST".into()))
}

/// Outra regra do mesmo estado e CEST vale em algum dia do período? (Duas
/// regras para a mesma combinação não podem valer no mesmo dia.)
fn sobreposta(conn: &Connection, id: &str, uf: &str, cest: &str, de: &str, ate: Option<&str>) -> Resultado<bool> {
    let fim = ate.unwrap_or("9999-12-31");
    Ok(conn.query_row(
        "SELECT COUNT(*) > 0 FROM regra_st
          WHERE id <> ?1 AND uf = ?2 AND cest_inicio = ?3
            AND vigente_de <= ?5 AND COALESCE(vigente_ate, '9999-12-31') >= ?4",
        params![id, uf, cest, de, fim],
        |l| l.get(0),
    )?)
}

/// Salva uma regra (nova, sem id, ou a mudança de uma que existe). Devolve o id.
/// - A regra que já valeu não muda o estado, o CEST, o início nem o "com ST"
///   (para isso, encerra-se e cria-se outra), e a data final não pode voltar
///   para antes de ontem (o que já valeu fica como valeu).
/// - Duas regras do mesmo estado e CEST não valem no mesmo dia.
/// - Toda linha salva por aqui fica como "da loja".
pub fn salvar_regra(conn: &Connection, dispositivo: &str, usuario_id: &str, r: &RegraSt) -> Resultado<String> {
    let uf = r.uf.trim().to_uppercase();
    let cest = documento::digitos(&r.cest_inicio);
    if !UFS.contains(&uf.as_str()) {
        return Err(Erro::campo("uf", "Escolha o estado."));
    }
    if cest.is_empty() || cest.len() > 7 {
        return Err(Erro::campo("cestInicio", "Digite o começo do CEST (ex.: 01 para autopeças)."));
    }
    let de = ler_data(&r.vigente_de)
        .map_err(|m| Erro::campo("vigenteDe", m))?
        .ok_or_else(|| Erro::campo("vigenteDe", "Digite a data de início."))?;
    let ate = ler_data(&r.vigente_ate).map_err(|m| Erro::campo("vigenteAte", m))?;
    if ate.as_ref().is_some_and(|a| *a < de) {
        return Err(Erro::campo("vigenteAte", "A data final é antes da inicial."));
    }
    let descricao: String = r.descricao.trim().chars().take(120).collect();
    let fonte: String = r.fonte.trim().chars().take(200).collect();
    let disp_legal: String = r.dispositivo.trim().chars().take(200).collect();
    let hoje = hoje(conn)?;
    let id = r.id.trim().to_string();
    let antes = if id.is_empty() { None } else { Some(guardada(conn, &id)?) };
    if let Some(a) = &antes {
        if a.de <= hoje {
            if a.uf != uf || a.cest != cest || a.de != de || a.com_st != r.com_st {
                return Err(Erro::validacao(
                    "Esta regra já valeu: o estado, o CEST, o início e o \"com ST\" ficam como estão. Para mudar, encerre a vigência e crie outra regra.",
                ));
            }
            let limite = ontem(conn)?;
            let ate_antes = a.ate.clone().unwrap_or_else(|| "9999-12-31".into());
            let ate_nova = ate.clone().unwrap_or_else(|| "9999-12-31".into());
            if ate_nova < ate_antes && ate_nova < limite {
                return Err(Erro::campo(
                    "vigenteAte",
                    format!("A regra já valeu: a data final não pode ficar antes de ontem ({}).", data_br(&limite)),
                ));
            }
        }
    }
    let alvo = if id.is_empty() { tempo::novo_id() } else { id.clone() };
    if sobreposta(conn, &alvo, &uf, &cest, &de, ate.as_deref())? {
        return Err(Erro::validacao(format!(
            "Já há uma regra para {uf} CEST {cest} valendo em parte desse período. Encerre a outra antes."
        )));
    }
    let mut mudancas = Vec::new();
    match &antes {
        None => {
            conn.execute(
                "INSERT INTO regra_st (id, uf, cest_inicio, vigente_de, vigente_ate, com_st, descricao, fonte, dispositivo,
                                       consultada_em, origem)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '', 'loja')",
                params![alvo, uf, cest, de, ate, r.com_st, descricao, fonte, disp_legal],
            )?;
            mudancas.push(format!(
                "Nova regra: {uf} CEST {cest}, {} a {}, {}",
                data_br(&de),
                ate.as_deref().map(data_br).unwrap_or_else(|| "sem fim".into()),
                if r.com_st { "com ST" } else { "sem ST" }
            ));
        }
        Some(a) => {
            let datas = |d: &Option<String>| d.as_deref().map(data_br).unwrap_or_else(|| "sem fim".into());
            if a.uf != uf || a.cest != cest {
                mudancas.push(format!("Estado e CEST: {} {} → {uf} {cest}", a.uf, a.cest));
            }
            if a.de != de {
                mudancas.push(format!("Início: {} → {}", data_br(&a.de), data_br(&de)));
            }
            if a.ate != ate {
                mudancas.push(format!("Fim: {} → {}", datas(&a.ate), datas(&ate)));
            }
            if a.com_st != r.com_st {
                mudancas.push(format!("ST: {} → {}", sim(a.com_st), sim(r.com_st)));
            }
            if a.descricao != descricao {
                mudancas.push("Descrição alterada".into());
            }
            if a.fonte != fonte || a.dispositivo != disp_legal {
                mudancas.push("Fonte alterada".into());
            }
            if mudancas.is_empty() {
                return Ok(alvo);
            }
            conn.execute(
                "UPDATE regra_st SET uf = ?2, cest_inicio = ?3, vigente_de = ?4, vigente_ate = ?5, com_st = ?6,
                                     descricao = ?7, fonte = ?8, dispositivo = ?9, origem = 'loja'
                  WHERE id = ?1",
                params![alvo, uf, cest, de, ate, r.com_st, descricao, fonte, disp_legal],
            )?;
            mudancas.insert(0, format!("Regra {} CEST {}", a.uf, a.cest));
        }
    }
    auditoria::registrar(conn, dispositivo, Some(usuario_id), "fiscal.regras", "regra_st", &alvo, &mudancas)?;
    Ok(alvo)
}

fn sim(com_st: bool) -> &'static str {
    if com_st {
        "com ST"
    } else {
        "sem ST"
    }
}

/// Encerra a vigência de uma regra: o último dia passa a ser ontem (ou o
/// próprio início, se ela começou hoje). Uma regra futura não se encerra:
/// exclui-se. Já encerrada antes de ontem, fica como está.
pub fn encerrar_regra(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str) -> Resultado<()> {
    let a = guardada(conn, id)?;
    let hoje = hoje(conn)?;
    if a.de > hoje {
        return Err(Erro::validacao("Esta regra ainda não começou: exclua em vez de encerrar."));
    }
    let fim = ontem(conn)?.max(a.de.clone());
    if a.ate.as_ref().is_some_and(|x| *x <= fim) {
        return Ok(());
    }
    conn.execute("UPDATE regra_st SET vigente_ate = ?2, origem = 'loja' WHERE id = ?1", params![id, fim])?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "fiscal.regras",
        "regra_st",
        id,
        &[format!("Regra {} CEST {} encerrada: vale até {}", a.uf, a.cest, data_br(&fim))],
    )
}

/// Exclui uma regra da loja que ainda não começou. A que já valeu não se
/// exclui (encerra-se): o histórico fica. A que veio com o programa também não
/// (muda-se): o catálogo a poria de volta num caixa que ainda não a tivesse.
pub fn excluir_regra(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str) -> Resultado<()> {
    let a = guardada(conn, id)?;
    if a.de <= hoje(conn)? {
        return Err(Erro::validacao(
            "Esta regra já valeu: encerre a vigência em vez de excluir, para o histórico ficar.",
        ));
    }
    let origem: String = conn.query_row("SELECT origem FROM regra_st WHERE id = ?1", [id], |l| l.get(0))?;
    if origem == "rustinho" {
        return Err(Erro::validacao(
            "Esta regra veio com o Rustinho, conferida na fonte oficial: ela não se exclui. Se ela não vale para a loja, mude as datas com o contador.",
        ));
    }
    conn.execute("DELETE FROM regra_st WHERE id = ?1", [id])?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "fiscal.regras",
        "regra_st",
        id,
        &[format!("Regra futura {} CEST {} (a partir de {}) excluída", a.uf, a.cest, data_br(&a.de))],
    )
}

/// O catálogo das regras que vêm com o programa (`regras_st_programa.json`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Catalogo {
    versao: i64,
    regras: Vec<RegraDoPrograma>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegraDoPrograma {
    id: String,
    uf: String,
    cest_inicio: String,
    vigente_de: String,
    vigente_ate: Option<String>,
    com_st: bool,
    descricao: String,
    fonte: String,
    dispositivo: String,
    consultada_em: String,
    desde_versao: i64,
}

/// A versão do catálogo já aplicada neste computador (chave só deste
/// computador: cada caixa aplica o seu ao abrir).
pub const CHAVE_CATALOGO: &str = "regras_st_catalogo";

/// Aplica o catálogo das regras do programa ao abrir o banco (FIS-P6).
/// - Uma regra entra uma vez só: a que é nova desde a versão já aplicada aqui.
///   A que a loja já mudou não muda de novo, e a excluída numa versão já
///   aplicada não volta.
/// - Nunca sobrescreve uma linha da loja, e não entra se a loja já tem uma
///   regra do mesmo estado e CEST valendo no mesmo período (a da loja vale).
/// - Não é alteração da loja (cada caixa faz o mesmo): quem chama grava por
///   `sync::sem_registrar`, na transação dele (`banco`, ao abrir).
pub fn carregar_regras_do_programa(tx: &Connection) -> Resultado<usize> {
    let catalogo: Catalogo = serde_json::from_str(include_str!("regras_st_programa.json"))
        .map_err(|e| Erro::Interno(format!("regras_st_programa.json: {e}")))?;
    let aplicada: i64 = config::ler(tx, CHAVE_CATALOGO)?.and_then(|v| v.parse().ok()).unwrap_or(0);
    if aplicada >= catalogo.versao {
        return Ok(0);
    }
    let mut entraram = 0;
    for r in catalogo.regras.iter().filter(|r| r.desde_versao > aplicada) {
        if sobreposta(tx, &r.id, &r.uf, &r.cest_inicio, &r.vigente_de, r.vigente_ate.as_deref())? {
            continue;
        }
        entraram += tx.execute(
            "INSERT OR IGNORE INTO regra_st (id, uf, cest_inicio, vigente_de, vigente_ate, com_st, descricao, fonte,
                                             dispositivo, consultada_em, origem)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'rustinho')",
            params![
                r.id,
                r.uf,
                r.cest_inicio,
                r.vigente_de,
                r.vigente_ate,
                r.com_st,
                r.descricao,
                r.fonte,
                r.dispositivo,
                r.consultada_em
            ],
        )?;
    }
    config::gravar(tx, CHAVE_CATALOGO, &catalogo.versao.to_string())?;
    Ok(entraram)
}

/// "dd/mm/aaaa" (ou "aaaa-mm-dd") → "aaaa-mm-dd".
pub fn ler_data(texto: &str) -> Result<Option<String>, String> {
    let t = texto.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let partes: Vec<&str> = t.split(['/', '-', '.']).collect();
    let (d, m, a) = match partes.as_slice() {
        [a, m, d] if a.len() == 4 => (*d, *m, *a),
        [d, m, a] => (*d, *m, *a),
        _ => return Err("Data inválida (use dd/mm/aaaa).".into()),
    };
    // E11 (dev.3.3): o ano com 4 algarismos, ou 2 (que vira 20xx). Com 3 (ou
    // 1) era aceito e virava o ano 0202 (ou 2006).
    if !(a.len() == 4 || a.len() == 2) || !a.bytes().all(|c| c.is_ascii_digit()) {
        return Err("Ano inválido (use 4 algarismos, como 2026).".into());
    }
    let (Ok(d), Ok(m), Ok(mut a)) = (d.parse::<u32>(), m.parse::<u32>(), a.parse::<i32>()) else {
        return Err("Data inválida (use dd/mm/aaaa).".into());
    };
    if a < 100 {
        a += 2000;
    }
    // T079 (OPUS-C-8): sem truncar (257 `as u8` virava 1 e passava a conferência).
    let mes =
        u8::try_from(m).ok().and_then(|m| time::Month::try_from(m).ok()).ok_or_else(|| "Mês inválido.".to_string())?;
    let dia = u8::try_from(d).map_err(|_| "Data inválida.".to_string())?;
    time::Date::from_calendar_date(a, mes, dia).map_err(|_| "Data inválida.".to_string())?;
    Ok(Some(format!("{a:04}-{m:02}-{d:02}")))
}

/// "2026-10-01" → "01/10/2026".
pub fn data_br(iso: &str) -> String {
    // C2-1 (R1 da 2C): sem pânico com texto que não é data (o byte 10 pode cair no
    // meio de um caractere).
    let d = iso.get(..10.min(iso.len())).unwrap_or(iso);
    let p: Vec<&str> = d.split('-').collect();
    if p.len() == 3 {
        format!("{}/{}/{}", p[2], p[1], p[0])
    } else {
        iso.to_string()
    }
}

// ---------------------------------------------------------------------------
// Dados da peça para a nota: IPI, PIS/Cofins, combustível (ANP) e serviço.
// Ficam guardados juntos (JSON) e só são usados na emissão da nota.
// ---------------------------------------------------------------------------

pub const CST_IPI: &[(&str, &str)] = &[
    ("", "Sem IPI"),
    ("00", "00 · Entrada com recuperação de crédito"),
    ("01", "01 · Entrada tributada com alíquota zero"),
    ("02", "02 · Entrada isenta"),
    ("03", "03 · Entrada não tributada"),
    ("04", "04 · Entrada imune"),
    ("05", "05 · Entrada com suspensão"),
    ("49", "49 · Outras entradas"),
    ("50", "50 · Saída tributada"),
    ("51", "51 · Saída tributada com alíquota zero"),
    ("52", "52 · Saída isenta"),
    ("53", "53 · Saída não tributada"),
    ("54", "54 · Saída imune"),
    ("55", "55 · Saída com suspensão"),
    ("99", "99 · Outras saídas"),
];

/// A mesma tabela serve para o PIS e para a Cofins.
pub const CST_PIS: &[(&str, &str)] = &[
    ("", "Automático (monofásica = 04)"),
    ("01", "01 · Tributável com alíquota básica"),
    ("02", "02 · Tributável com alíquota diferenciada"),
    ("03", "03 · Tributável com alíquota por unidade de medida"),
    ("04", "04 · Monofásica, revenda a alíquota zero"),
    ("05", "05 · Tributável por substituição tributária"),
    ("06", "06 · Tributável a alíquota zero"),
    ("07", "07 · Isenta da contribuição"),
    ("08", "08 · Sem incidência da contribuição"),
    ("09", "09 · Com suspensão da contribuição"),
    ("49", "49 · Outras operações de saída"),
    ("50", "50 · Crédito: só receita tributada no mercado interno"),
    ("51", "51 · Crédito: só receita não tributada no mercado interno"),
    ("52", "52 · Crédito: só receita de exportação"),
    ("53", "53 · Crédito: receitas tributadas e não tributadas no mercado interno"),
    ("54", "54 · Crédito: receitas tributadas no mercado interno e de exportação"),
    ("55", "55 · Crédito: receitas não tributadas no mercado interno e de exportação"),
    ("56", "56 · Crédito: receitas tributadas e não tributadas, mercado interno e exportação"),
    ("60", "60 · Crédito presumido: só receita tributada no mercado interno"),
    ("61", "61 · Crédito presumido: só receita não tributada no mercado interno"),
    ("62", "62 · Crédito presumido: só receita de exportação"),
    ("63", "63 · Crédito presumido: receitas tributadas e não tributadas no mercado interno"),
    ("64", "64 · Crédito presumido: receitas tributadas no mercado interno e de exportação"),
    ("65", "65 · Crédito presumido: receitas não tributadas no mercado interno e de exportação"),
    ("66", "66 · Crédito presumido: receitas tributadas e não tributadas, mercado interno e exportação"),
    ("67", "67 · Crédito presumido: outras operações"),
    ("70", "70 · Aquisição sem direito a crédito"),
    ("71", "71 · Aquisição com isenção"),
    ("72", "72 · Aquisição com suspensão"),
    ("73", "73 · Aquisição a alíquota zero"),
    ("74", "74 · Aquisição sem incidência da contribuição"),
    ("75", "75 · Aquisição por substituição tributária"),
    ("98", "98 · Outras operações de entrada"),
    ("99", "99 · Outras operações"),
];

pub const EXIGIBILIDADE_ISS: &[(&str, &str)] = &[
    ("1", "1 · Exigível"),
    ("2", "2 · Não incidência"),
    ("3", "3 · Isenção"),
    ("4", "4 · Exportação"),
    ("5", "5 · Imunidade"),
    ("6", "6 · Suspensa por decisão judicial"),
    ("7", "7 · Suspensa por processo administrativo"),
    ("8", "8 · ISS fixo"),
];

/// Tudo como texto já arrumado ("12,50"), do jeito que a tela mostra.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FiscalPeca {
    // IPI
    pub ipi_cst: String,
    pub ipi_aliquota: String,
    /// R$ por unidade (no lugar do percentual).
    pub ipi_valor: String,
    pub ipi_enquadramento: String,
    pub ipi_selo: String,
    pub ipi_selo_quantidade: String,
    // PIS e Cofins
    pub pis_cst: String,
    pub pis_aliquota: String,
    pub pis_valor: String,
    pub cofins_cst: String,
    pub cofins_aliquota: String,
    pub cofins_valor: String,
    pub natureza_receita: String,
    // Combustível (o código e a descrição ANP ficam na peça)
    pub glp: String,
    pub gn_nacional: String,
    pub gn_importado: String,
    pub valor_partida: String,
    pub codif: String,
    pub temperatura: String,
    pub importado: bool,
    pub uf_origem: String,
    pub percentual_origem: String,
    pub cide: String,
    pub biodiesel: String,
    // Serviço
    pub servico_codigo: String,
    pub cnae: String,
    pub nbs: String,
    pub codigo_municipio: String,
    pub iss_aliquota: String,
    pub iss_exigibilidade: String,
    pub incentivo_fiscal: bool,
    pub servico_descricao: String,
}

fn escolha(valor: &str, lista: &[(&str, &str)], campo: &str, rotulo: &str) -> Resultado<String> {
    let v = valor.trim();
    if lista.iter().any(|(c, _)| *c == v) {
        Ok(v.to_string())
    } else {
        Err(Erro::campo(campo, format!("Escolha o {rotulo} da lista.")))
    }
}

fn pct(valor: &str, campo: &str) -> Resultado<String> {
    match crate::numeros::ler_percentual(valor).map_err(|m| Erro::campo(campo, m))? {
        None => Ok(String::new()),
        Some(x) if (0..=10_000).contains(&x) => Ok(crate::numeros::escrever_percentual(x)),
        Some(_) => Err(Erro::campo(campo, "Use um percentual entre 0 e 100.")),
    }
}

fn reais(valor: &str, campo: &str) -> Resultado<String> {
    match crate::numeros::ler_dinheiro(valor).map_err(|m| Erro::campo(campo, m))? {
        None => Ok(String::new()),
        Some(x) if x >= 0 => Ok(crate::numeros::escrever_dinheiro(x)),
        Some(_) => Err(Erro::campo(campo, "O valor não pode ser negativo.")),
    }
}

fn numeros_ate(valor: &str, campo: &str, rotulo: &str, maximo: usize) -> Resultado<String> {
    let v = documento::digitos(valor);
    if v.len() > maximo {
        return Err(Erro::campo(campo, format!("{rotulo}: no máximo {maximo} números.")));
    }
    Ok(v)
}

fn curto(valor: &str, campo: &str, rotulo: &str, maximo: usize) -> Resultado<String> {
    let v = crate::texto::limpar(valor);
    if crate::texto::tamanho(&v) > maximo {
        return Err(Erro::campo(campo, format!("{rotulo}: use no máximo {maximo} caracteres.")));
    }
    Ok(v)
}

fn rotulo_fiscal(chave: &str) -> &str {
    match chave {
        "ipiCst" => "CST do IPI",
        "ipiAliquota" => "IPI (%)",
        "ipiValor" => "IPI (R$)",
        "ipiEnquadramento" => "Enquadramento do IPI",
        "ipiSelo" => "Selo do IPI",
        "ipiSeloQuantidade" => "Quantidade de selos",
        "pisCst" => "CST do PIS",
        "pisAliquota" => "PIS (%)",
        "pisValor" => "PIS (R$)",
        "cofinsCst" => "CST da Cofins",
        "cofinsAliquota" => "Cofins (%)",
        "cofinsValor" => "Cofins (R$)",
        "naturezaReceita" => "Natureza da receita",
        "glp" => "GLP (%)",
        "gnNacional" => "Gás natural nacional (%)",
        "gnImportado" => "Gás natural importado (%)",
        "valorPartida" => "Valor de partida",
        "codif" => "CODIF",
        "temperatura" => "Quantidade à temperatura ambiente",
        "importado" => "Combustível importado",
        "ufOrigem" => "Estado de origem",
        "percentualOrigem" => "Percentual originário",
        "cide" => "CIDE (R$)",
        "biodiesel" => "Biodiesel (%)",
        "servicoCodigo" => "Código do serviço",
        "cnae" => "CNAE",
        "nbs" => "NBS",
        "codigoMunicipio" => "Código no município",
        "issAliquota" => "ISS (%)",
        "issExigibilidade" => "Exigibilidade do ISS",
        "incentivoFiscal" => "Incentivo fiscal",
        outro => outro,
    }
}

impl FiscalPeca {
    /// Confere e arruma o que foi digitado.
    pub fn conferir(&self) -> Resultado<FiscalPeca> {
        let uf = self.uf_origem.trim().to_uppercase();
        if !uf.is_empty() && !UFS.contains(&uf.as_str()) {
            return Err(Erro::campo("fiscal.ufOrigem", "Escolha o estado de origem."));
        }
        let quantidade = |v: &str, campo: &str| -> Resultado<String> {
            match crate::numeros::ler_quantidade(v).map_err(|m| Erro::campo(campo, m))? {
                None => Ok(String::new()),
                Some(x) if x >= 0 => Ok(crate::numeros::escrever_quantidade(x)),
                Some(_) => Err(Erro::campo(campo, "Não pode ser negativo.")),
            }
        };
        let f = FiscalPeca {
            ipi_cst: escolha(&self.ipi_cst, CST_IPI, "fiscal.ipiCst", "CST do IPI")?,
            ipi_aliquota: pct(&self.ipi_aliquota, "fiscal.ipiAliquota")?,
            ipi_valor: reais(&self.ipi_valor, "fiscal.ipiValor")?,
            ipi_enquadramento: numeros_ate(&self.ipi_enquadramento, "fiscal.ipiEnquadramento", "Enquadramento", 3)?,
            ipi_selo: curto(&self.ipi_selo, "fiscal.ipiSelo", "Selo", 60)?,
            ipi_selo_quantidade: numeros_ate(
                &self.ipi_selo_quantidade,
                "fiscal.ipiSeloQuantidade",
                "Quantidade de selos",
                12,
            )?,
            pis_cst: escolha(&self.pis_cst, CST_PIS, "fiscal.pisCst", "CST do PIS")?,
            pis_aliquota: pct(&self.pis_aliquota, "fiscal.pisAliquota")?,
            pis_valor: reais(&self.pis_valor, "fiscal.pisValor")?,
            cofins_cst: escolha(&self.cofins_cst, CST_PIS, "fiscal.cofinsCst", "CST da Cofins")?,
            cofins_aliquota: pct(&self.cofins_aliquota, "fiscal.cofinsAliquota")?,
            cofins_valor: reais(&self.cofins_valor, "fiscal.cofinsValor")?,
            natureza_receita: numeros_ate(&self.natureza_receita, "fiscal.naturezaReceita", "Natureza da receita", 3)?,
            glp: pct(&self.glp, "fiscal.glp")?,
            gn_nacional: pct(&self.gn_nacional, "fiscal.gnNacional")?,
            gn_importado: pct(&self.gn_importado, "fiscal.gnImportado")?,
            valor_partida: reais(&self.valor_partida, "fiscal.valorPartida")?,
            codif: numeros_ate(&self.codif, "fiscal.codif", "CODIF", 21)?,
            temperatura: quantidade(&self.temperatura, "fiscal.temperatura")?,
            importado: self.importado,
            uf_origem: uf,
            percentual_origem: pct(&self.percentual_origem, "fiscal.percentualOrigem")?,
            cide: reais(&self.cide, "fiscal.cide")?,
            biodiesel: pct(&self.biodiesel, "fiscal.biodiesel")?,
            servico_codigo: curto(&self.servico_codigo, "fiscal.servicoCodigo", "Código do serviço", 10)?,
            cnae: numeros_ate(&self.cnae, "fiscal.cnae", "CNAE", 7)?,
            nbs: numeros_ate(&self.nbs, "fiscal.nbs", "NBS", 9)?,
            codigo_municipio: curto(&self.codigo_municipio, "fiscal.codigoMunicipio", "Código no município", 20)?,
            iss_aliquota: pct(&self.iss_aliquota, "fiscal.issAliquota")?,
            iss_exigibilidade: if self.iss_exigibilidade.trim().is_empty() {
                "1".into()
            } else {
                escolha(&self.iss_exigibilidade, EXIGIBILIDADE_ISS, "fiscal.issExigibilidade", "tipo de ISS")?
            },
            incentivo_fiscal: self.incentivo_fiscal,
            servico_descricao: {
                let d = self.servico_descricao.trim().to_string();
                if crate::texto::tamanho(&d) > 2000 {
                    return Err(Erro::campo("fiscal.servicoDescricao", "Use no máximo 2000 caracteres."));
                }
                d
            },
        };
        Ok(f)
    }

    pub fn de_json(texto: &str) -> FiscalPeca {
        serde_json::from_str(texto).unwrap_or_default()
    }

    pub fn para_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }

    /// Frases com o que mudou, para o histórico da peça.
    pub fn diferencas(&self, depois: &FiscalPeca) -> Vec<String> {
        let a = serde_json::to_value(self).unwrap_or_default();
        let b = serde_json::to_value(depois).unwrap_or_default();
        let mut v = Vec::new();
        if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
            for (k, va) in a {
                let vb = &b[k];
                if va != vb {
                    let t = |x: &serde_json::Value| match x {
                        serde_json::Value::String(s) if s.is_empty() => "—".to_string(),
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Bool(true) => "sim".into(),
                        serde_json::Value::Bool(false) => "não".into(),
                        outro => outro.to_string(),
                    };
                    if k == "servicoDescricao" {
                        v.push("Descrição do serviço alterada".into());
                    } else {
                        v.push(format!("{}: {} → {}", rotulo_fiscal(k), t(va), t(vb)));
                    }
                }
            }
        }
        v
    }
}

// ------------------------------------------------------------- comandos das telas

pub use comandos::COMANDOS;

/// Os comandos da conferência fiscal das peças (etapa 3 da modularização: vieram do `match` do
/// `api.rs` sem mudar nome, entrada, resposta nem permissão).
mod comandos {
    use serde::Deserialize;
    use serde_json::{json, Value};

    use crate::despacho::{ler, Classe, Comando, Pedido};
    use crate::erro::Resultado;
    use crate::fiscal;

    pub const COMANDOS: &[Comando] = &[
        Comando { nome: "fiscal_conferir", classe: Classe::Leitura, funcao: fiscal_conferir },
        Comando { nome: "fiscal_automatico", classe: Classe::Caixa, funcao: fiscal_automatico },
        // FIS-P5 (0.9.19, etapa 3): uma regra de cada vez; a que já valeu se encerra, não se exclui.
        Comando { nome: "fiscal_regra_salvar", classe: Classe::Gerente("configuracoes"), funcao: fiscal_regra_salvar },
        Comando {
            nome: "fiscal_regra_encerrar",
            classe: Classe::Gerente("configuracoes"),
            funcao: fiscal_regra_encerrar,
        },
        Comando {
            nome: "fiscal_regra_excluir",
            classe: Classe::Gerente("configuracoes"),
            funcao: fiscal_regra_excluir,
        },
        Comando { nome: "fiscal_tributacao", classe: Classe::Leitura, funcao: fiscal_tributacao },
    ];

    fn fiscal_conferir(p: &mut Pedido, _dados: Value) -> Resultado<Value> {
        Ok(json!(fiscal::conferir(p.conn, &fiscal::hoje(p.conn)?)?))
    }

    fn fiscal_automatico(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: Ids = ler(dados)?;
        // R24 (DP-23): trocar o CSOSN das peças (aqui, pelo automático) é do
        // gerente; cada troca fica no registro da peça.
        p.exigir("csosn")?;
        let tx = p.conn.transaction()?;
        let n = fiscal::usar_automatico(&tx, p.disp, p.usuario_id, &d.ids)?;
        tx.commit()?;
        Ok(json!({ "alteradas": n, "conferencia": fiscal::conferir(p.conn, &fiscal::hoje(p.conn)?)? }))
    }

    fn fiscal_regra_salvar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: fiscal::RegraSt = ler(dados)?;
        let tx = p.conn.transaction()?;
        let id = fiscal::salvar_regra(&tx, p.disp, p.usuario_id, &d)?;
        tx.commit()?;
        Ok(json!({ "id": id, "conferencia": fiscal::conferir(p.conn, &fiscal::hoje(p.conn)?)? }))
    }

    fn fiscal_regra_encerrar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: crate::despacho::Id = ler(dados)?;
        let tx = p.conn.transaction()?;
        fiscal::encerrar_regra(&tx, p.disp, p.usuario_id, &d.id)?;
        tx.commit()?;
        Ok(json!(fiscal::conferir(p.conn, &fiscal::hoje(p.conn)?)?))
    }

    fn fiscal_regra_excluir(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: crate::despacho::Id = ler(dados)?;
        let tx = p.conn.transaction()?;
        fiscal::excluir_regra(&tx, p.disp, p.usuario_id, &d.id)?;
        tx.commit()?;
        Ok(json!(fiscal::conferir(p.conn, &fiscal::hoje(p.conn)?)?))
    }

    fn fiscal_tributacao(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: Tributacao = ler(dados)?;
        let uf = fiscal::uf_loja(p.conn)?;
        Ok(json!(fiscal::tributacao(p.conn, &uf, &d.cest, &d.csosn, &fiscal::hoje(p.conn)?)?))
    }

    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Ids {
        ids: Vec<String>,
    }

    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Tributacao {
        cest: String,
        csosn: String,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// T079 (OPUS-C-8 da auditoria da 0.9.18-dev.3.3.3): dia ou mês com 3 algarismos passava a conferência pelo
    /// truncamento (`as u8`: 257 vira 1) e gravava uma data inválida ("2026-12-257"), mostrada como outra.
    #[test]
    fn data_com_dia_ou_mes_grande_demais_e_recusada() {
        assert!(ler_data("257/12/2026").is_err(), "{:?}", ler_data("257/12/2026"));
        assert!(ler_data("10/257/2026").is_err(), "{:?}", ler_data("10/257/2026"));
        assert!(ler_data("2026-269-01").is_err(), "{:?}", ler_data("2026-269-01"));
        assert_eq!(ler_data("25/12/2026"), Ok(Some("2026-12-25".to_string())));
    }
    use crate::banco::Banco;

    #[test]
    fn autopecas_sp_saem_da_st_em_outubro_de_2026() {
        let b = Banco::em_memoria().unwrap();
        let antes = tributacao(&b.conn, "SP", "01.001.00", "", "2026-09-30").unwrap();
        assert_eq!((antes.csosn.as_str(), antes.cfop.as_str()), ("500", "5405"));
        assert_eq!(antes.com_st, Some(true));

        let depois = tributacao(&b.conn, "SP", "0100100", "", "2026-10-01").unwrap();
        assert_eq!((depois.csosn.as_str(), depois.cfop.as_str()), ("102", "5102"));
        assert!(depois.alerta.is_empty());

        // CSOSN 500 fixado à mão (como vem do sistema antigo): avisa
        let fixo = tributacao(&b.conn, "SP", "01.001.00", "500", "2026-10-13").unwrap();
        assert_eq!(fixo.csosn, "500");
        assert!(fixo.alerta.contains("102"));

        // outro estado sem regra: sem ST, sem alerta
        let mg = tributacao(&b.conn, "MG", "01.001.00", "500", "2026-10-13").unwrap();
        assert!(mg.alerta.is_empty());
        assert_eq!(mg.com_st, None);

        // pneus (CEST 16) também
        assert_eq!(tributacao(&b.conn, "SP", "16.001.00", "", "2026-10-02").unwrap().csosn, "102");
    }

    /// T013 (etapa 3, conferência do artefato 10): os CSOSN 201, 202 e 203 são "com cobrança do ICMS por Substituição
    /// Tributária" (MOC 7.0, Anexo I): quem vende é o substituto. O CFOP da revenda de mercadoria de terceiros nessa
    /// condição é o 5.403 ("na condição de contribuinte substituto"); o 5.405 é só do substituído (CSOSN 500). Tabela
    /// de CFOP vigente do CONFAZ (redação de 01/06/2024). Antes o Rustinho sugeria 5405 para os quatro.
    #[test]
    fn t013_cfop_do_substituto_e_5403_e_o_do_substituido_e_5405() {
        let b = Banco::em_memoria().unwrap();
        for csosn in ["201", "202", "203"] {
            let t = tributacao(&b.conn, "SP", "01.001.00", csosn, "2026-09-30").unwrap();
            assert_eq!((t.csosn.as_str(), t.cfop.as_str()), (csosn, "5403"), "CSOSN {csosn}");
        }
        let substituido = tributacao(&b.conn, "SP", "01.001.00", "500", "2026-09-30").unwrap();
        assert_eq!(substituido.cfop, "5405");
        for csosn in ["102", "103", "300", "400", "900"] {
            assert_eq!(
                tributacao(&b.conn, "SP", "01.001.00", csosn, "2026-10-13").unwrap().cfop,
                "5102",
                "CSOSN {csosn}"
            );
        }
    }

    /// G4: a lista oficial do Conv. 142/18 (141 CEST vigentes) acha o CEST pela NCM; o revogado não está.
    #[test]
    fn g4_cest_pela_lista_oficial_do_convenio_142() {
        let todos: std::collections::BTreeSet<&String> = lista_cest().iter().map(|(_, c)| c).collect();
        assert_eq!(todos.len(), 140, "141 vigentes, menos o 01.999.00, que não tem NCM");
        assert!(!todos.iter().any(|c| c.as_str() == "01.110.00"), "revogado pelo Conv. ICMS 165/19");
        assert_eq!(cests_do_ncm("3815.12.10"), vec!["01.001.00".to_string()]);
        assert_eq!(
            cests_do_ncm("40111000"),
            vec!["16.001.00".to_string(), "16.002.00".to_string(), "16.004.00".to_string()]
        );
        assert!(cests_do_ncm("22021000").is_empty(), "refrigerante não é autopeça");
        assert!(cests_do_ncm("").is_empty());
    }

    /// FIS-P6: o catálogo do programa entra uma vez por versão, não vai para o log da troca, não passa por cima de
    /// uma regra da loja no mesmo período, e a regra do programa não se exclui (muda-se).
    #[test]
    fn fis_p6_o_catalogo_do_programa_entra_uma_vez_e_respeita_a_loja() {
        let b = Banco::em_memoria().unwrap();
        let u = crate::usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        assert_eq!(config::ler(&b.conn, CHAVE_CATALOGO).unwrap().as_deref(), Some("1"));
        assert_eq!(carregar_regras_do_programa(&b.conn).unwrap(), 0, "a mesma versão não aplica de novo");
        // A regra que saiu deste banco (por exemplo, a exclusão que veio de outro caixa) não volta na mesma versão.
        b.conn.execute("DELETE FROM regra_st WHERE id = 'ba-pneus-com-st'", []).unwrap();
        assert_eq!(crate::sync::sem_registrar(&b.conn, carregar_regras_do_programa).unwrap(), 0);
        assert!(!regras(&b.conn).unwrap().iter().any(|r| r.id == "ba-pneus-com-st"));
        // Um caixa que ainda não aplicou: sem a regra de GO pneus e com uma regra da loja para RS 16 no período.
        b.conn
            .execute("DELETE FROM regra_st WHERE id IN ('go-pneus-com-st', 'rs-pneus-com-st', 'rs-pneus-sem-st')", [])
            .unwrap();
        let loja = RegraSt {
            uf: "RS".into(),
            cest_inicio: "16".into(),
            vigente_de: "01/01/2026".into(),
            com_st: true,
            ..Default::default()
        };
        salvar_regra(&b.conn, &b.dispositivo, &u.id, &loja).unwrap();
        config::gravar(&b.conn, CHAVE_CATALOGO, "0").unwrap();
        let log = |b: &Banco| -> i64 { b.conn.query_row("SELECT COUNT(*) FROM alteracao", [], |l| l.get(0)).unwrap() };
        let antes = log(&b);
        // Um caixa que ainda não aplicou a versão 1: GO e BA entram; RS 16 do programa não (a da loja, aberta desde
        // 01/01/2026, vale no período).
        assert_eq!(crate::sync::sem_registrar(&b.conn, carregar_regras_do_programa).unwrap(), 2);
        assert_eq!(log(&b), antes, "o catálogo não vai para o log da troca");
        let lista = regras(&b.conn).unwrap();
        assert!(lista.iter().any(|r| r.id == "go-pneus-com-st" && r.origem == "rustinho"));
        assert!(lista.iter().any(|r| r.id == "ba-pneus-com-st" && r.origem == "rustinho"));
        assert!(!lista.iter().any(|r| r.id == "rs-pneus-com-st" || r.id == "rs-pneus-sem-st"));
        let aplicando: i64 =
            b.conn.query_row("SELECT aplicando FROM sync_estado WHERE id = 1", [], |l| l.get(0)).unwrap();
        assert_eq!(aplicando, 0, "o registro da troca volta a ficar ligado");
    }

    #[test]
    fn fis_p5_a_regra_futura_do_programa_nao_se_exclui() {
        let b = Banco::em_memoria().unwrap();
        let u = crate::usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        // RS, pneus fora da ST a partir de 01/01/2027: futura, do programa.
        let r = achar(&regras(&b.conn).unwrap(), "rs-pneus-sem-st").clone();
        assert_eq!(r.situacao, "futura");
        assert!(excluir_regra(&b.conn, &b.dispositivo, &u.id, "rs-pneus-sem-st").is_err());
    }

    /// O dia `dias` a partir de hoje, "dd/mm/aaaa".
    fn dia(b: &Banco, dias: i64) -> String {
        let d: String =
            b.conn.query_row("SELECT date('now', 'localtime', ?1)", [format!("{dias} days")], |l| l.get(0)).unwrap();
        data_br(&d)
    }

    fn achar<'a>(lista: &'a [RegraSt], id: &str) -> &'a RegraSt {
        lista.iter().find(|r| r.id == id).unwrap()
    }

    /// FIS-P6 (0.9.19, etapa 3): as regras que vêm com o programa têm a fonte, e são só de UFs com a vigência
    /// confirmada na fonte oficial (migração 0032). Sem regra para a UF, o Rustinho não presume.
    #[test]
    fn fis_p6_as_regras_do_programa_tem_fonte() {
        let b = Banco::em_memoria().unwrap();
        let lista = regras(&b.conn).unwrap();
        assert_eq!(lista.len(), 37);
        for r in &lista {
            assert_eq!(r.origem, "rustinho", "{}", r.id);
            assert!(!r.fonte.is_empty() && !r.dispositivo.is_empty() && !r.consultada_em.is_empty(), "{}", r.id);
        }
        let sp = achar(&lista, "sp-autopecas-sem-st");
        assert!(sp.fonte.contains("SRE 34/2026"));
        // UF sem fonte confirmada (ex.: AL): sem regra, sem presumir.
        assert_eq!(tributacao(&b.conn, "AL", "01.001.00", "", "2026-11-26").unwrap().com_st, None);
        // RS: os pneus saem da ST em 01/01/2027; RO: só os pneus de moto e bicicleta continuam (CEST mais longo vence).
        assert_eq!(tributacao(&b.conn, "RS", "16.001.00", "", "2026-12-31").unwrap().com_st, Some(true));
        assert_eq!(tributacao(&b.conn, "RS", "16.001.00", "", "2027-01-01").unwrap().com_st, Some(false));
        assert_eq!(tributacao(&b.conn, "RO", "16.001.00", "", "2026-11-26").unwrap().com_st, Some(false));
        assert_eq!(tributacao(&b.conn, "RO", "16.003.00", "", "2026-11-26").unwrap().com_st, Some(true));
        assert_eq!(tributacao(&b.conn, "RO", "16.007.01", "", "2026-11-26").unwrap().com_st, Some(true));
        assert_eq!(tributacao(&b.conn, "RO", "16.007.00", "", "2026-11-26").unwrap().com_st, Some(false));
    }

    /// FIS-P5: nova regra, a mais específica vence, e duas regras do mesmo estado e CEST não valem no mesmo dia.
    #[test]
    fn fis_p5_nova_regra_e_periodos_em_conflito() {
        let b = Banco::em_memoria().unwrap();
        let u = crate::usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        // SP 01 já tem regra aberta desde 01/10/2026: outra SP 01 no período é recusada.
        let mut nova = RegraSt {
            uf: "SP".into(),
            cest_inicio: "01".into(),
            vigente_de: "15/10/2026".into(),
            com_st: true,
            ..Default::default()
        };
        assert!(salvar_regra(&b.conn, &b.dispositivo, &u.id, &nova).is_err());
        // a mais específica (01.003) é outra combinação e vence a geral
        nova.cest_inicio = "01.003".into();
        nova.vigente_de = "01/10/2026".into();
        nova.descricao = "Exceção de teste".into();
        let id = salvar_regra(&b.conn, &b.dispositivo, &u.id, &nova).unwrap();
        assert_eq!(tributacao(&b.conn, "SP", "0100300", "", "2026-10-13").unwrap().csosn, "500");
        assert_eq!(tributacao(&b.conn, "SP", "0100100", "", "2026-10-13").unwrap().csosn, "102");
        let lista = regras(&b.conn).unwrap();
        assert_eq!(achar(&lista, &id).origem, "loja");
        assert_eq!(lista.len(), 38);

        assert_eq!(ler_data("1/10/26").unwrap().unwrap(), "2026-10-01");
        assert!(ler_data("31/02/2026").is_err());
        assert_eq!(data_br("2026-10-01"), "01/10/2026");
    }

    /// FIS-P5: a regra que já valeu não se exclui nem se reescreve; encerra-se, e fica encerrada.
    #[test]
    fn fis_p5_a_regra_que_ja_valeu_se_encerra_e_nao_se_exclui() {
        let b = Banco::em_memoria().unwrap();
        let u = crate::usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        let id = "sp-autopecas-sem-st";
        assert!(achar(&regras(&b.conn).unwrap(), id).ja_valeu);
        assert!(excluir_regra(&b.conn, &b.dispositivo, &u.id, id).is_err());
        let mut r = achar(&regras(&b.conn).unwrap(), id).clone();
        r.com_st = true;
        assert!(salvar_regra(&b.conn, &b.dispositivo, &u.id, &r).is_err(), "o \"com ST\" de quem já valeu não muda");
        r.com_st = false;
        r.vigente_ate = dia(&b, -30);
        assert!(salvar_regra(&b.conn, &b.dispositivo, &u.id, &r).is_err(), "o fim não volta para antes de ontem");
        // a descrição pode mudar (e a linha vira da loja)
        r.vigente_ate = String::new();
        r.descricao = "Conferida com o contador".into();
        salvar_regra(&b.conn, &b.dispositivo, &u.id, &r).unwrap();
        assert_eq!(achar(&regras(&b.conn).unwrap(), id).origem, "loja");
        // encerrar: vale até ontem, e fica encerrada
        encerrar_regra(&b.conn, &b.dispositivo, &u.id, id).unwrap();
        let e = achar(&regras(&b.conn).unwrap(), id).clone();
        assert_eq!(e.vigente_ate, dia(&b, -1));
        assert_eq!(e.situacao, "encerrada");
        assert!(excluir_regra(&b.conn, &b.dispositivo, &u.id, id).is_err(), "encerrada também não se exclui");
    }

    /// FIS-P5: a regra futura se exclui; não se encerra (ainda não valeu).
    #[test]
    fn fis_p5_a_regra_futura_se_exclui() {
        let b = Banco::em_memoria().unwrap();
        let u = crate::usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        let futura = RegraSt {
            uf: "AL".into(),
            cest_inicio: "01".into(),
            vigente_de: dia(&b, 10),
            com_st: true,
            ..Default::default()
        };
        let id = salvar_regra(&b.conn, &b.dispositivo, &u.id, &futura).unwrap();
        let r = achar(&regras(&b.conn).unwrap(), &id).clone();
        assert_eq!(r.situacao, "futura");
        assert!(!r.ja_valeu);
        assert!(encerrar_regra(&b.conn, &b.dispositivo, &u.id, &id).is_err());
        excluir_regra(&b.conn, &b.dispositivo, &u.id, &id).unwrap();
        assert!(regras(&b.conn).unwrap().iter().all(|x| x.id != id));
    }

    /// E11 (dev.3.3, 2.4): o ano tem 4 algarismos, ou 2 (= 20xx). Com 3 (ou 1)
    /// era aceito e virava o ano 0202 (ou 2006).
    #[test]
    fn e11_ano_com_3_algarismos_e_recusado() {
        assert_eq!(ler_data("01/10/2026").unwrap().unwrap(), "2026-10-01");
        assert_eq!(ler_data("01/10/26").unwrap().unwrap(), "2026-10-01");
        assert_eq!(ler_data("2026-10-01").unwrap().unwrap(), "2026-10-01");
        for errada in ["01/10/202", "01/10/6", "01/10/20265", "01/10/+26", "202-10-01"] {
            assert!(ler_data(errada).is_err(), "{errada} foi aceita: {:?}", ler_data(errada));
        }
    }
}
