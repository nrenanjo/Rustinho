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
    /// C-M7 da R1: false = sem regra (sem UF, sem CEST ou sem regra para o CEST nesta data) e sem CSOSN escolhido à
    /// mão: o Rustinho não decide; o CSOSN e o CFOP ficam vazios ("a definir").
    pub decidido: bool,
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
    // C-M7 da R1: sem regra, o Rustinho não presume (nem "sem ST"): o CSOSN fica a definir com o contador.
    let (csosn_auto, explicacao) = match &regra {
        Some((true, d)) => ("500", d.clone()),
        Some((false, d)) => ("102", d.clone()),
        None if uf.is_empty() => {
            ("", "Informe o estado da loja em Configurações: sem ele, o Rustinho não sabe se a peça tem ST.".to_string())
        }
        None if documento::digitos(cest).is_empty() => {
            ("", "Sem CEST: o Rustinho não sabe se a peça tem ST. Confira o NCM e o CEST com o contador.".to_string())
        }
        None => (
            "",
            format!("Nenhuma regra de ST para este CEST em {uf} nesta data: o Rustinho não decide. Confirme com o contador."),
        ),
    };
    let automatico = csosn_peca.is_empty();
    let csosn = if automatico { csosn_auto.to_string() } else { csosn_peca.to_string() };
    let decidido = !csosn.is_empty();
    // 500: a ST já foi paga antes, a loja é a substituída (CFOP 5.405). 201, 202 e 203: a loja cobra a ST, é a
    // substituta (CFOP 5.403). Tabela de CFOP do CONFAZ; CSOSN do MOC 7.0, Anexo I.
    let cfop = match csosn.as_str() {
        "" => "",
        "500" => "5405",
        "201" | "202" | "203" => "5403",
        _ => "5102",
    }
    .to_string();
    let alerta = match (csosn.as_str(), com_st) {
        // F2-4 da R1 (MOC 7.0, Anexo I: N12a-20, rejeição 383; N12a-40 e N12a-44; N12a-70): o substituto só existe na
        // NF-e para contribuinte; na NFC-e e na NF-e para não contribuinte (indIEDest = 9) a SEFAZ recusa.
        ("201" | "202" | "203", _) => format!(
            "CSOSN {csosn} (a loja cobra a ST) só vale na NF-e para empresa contribuinte. A SEFAZ recusa na NFC-e do balcão (rejeição 383) e na NF-e para quem não é contribuinte do ICMS, por exemplo uma pessoa física (rejeição 600). Confirme com o contador."
        ),
        ("500", Some(false)) => format!(
            "Esta peça está com CSOSN 500, mas pela regra de {uf} ela não tem mais ST nesta data. O certo deve ser 102 (CFOP 5102). Confirme com o contador."
        ),
        ("102", Some(true)) if !automatico => format!(
            "Esta peça está com CSOSN 102, mas pela regra de {uf} ela tem ST nesta data. O certo deve ser 500 (CFOP 5405). Confirme com o contador."
        ),
        _ => String::new(),
    };
    Ok(Tributacao { csosn, cfop, automatico, com_st, decidido, explicacao, alerta })
}

/// G5 da R1: o último dia do PIS/COFINS monofásico das autopeças e dos pneus (Lei 10.485/2002). A LC 214/2025, art.
/// 542, XVI, revoga os arts. 1º a 3º, 5º e 6º da Lei 10.485 a partir de 01/01/2027 (lida no Planalto; artefato 02b).
pub const FIM_DO_MONOFASICO: &str = "2026-12-31";

/// O PIS/COFINS monofásico ainda vale nessa data ("AAAA-MM-DD")?
pub fn monofasico_vale(data: &str) -> bool {
    data <= FIM_DO_MONOFASICO
}

/// G6 da R1 (Ajuste SINIEF 23/26, cl. primeira, efeitos a partir de 05/10/2026): na venda do varejista que permite ao
/// comprador aproveitar o crédito do ICMS, a NF-e (55) é obrigatória no lugar da NFC-e (65); o DANFE pode ser o
/// Simplificado Tipo 2 (§ 1º), e a legislação da UF pode fixar limites e condições (§ 3º). `data`: "AAAA-MM-DD".
pub fn modelo_da_venda(comprador_aproveita_credito: bool, data: &str) -> i64 {
    if comprador_aproveita_credito && data >= "2026-10-05" {
        55
    } else {
        65
    }
}

/// G8 da R1: o CFOP da devolução pela operação de origem (tabela de CFOP do CONFAZ, vigente, lida em 26/11/2026:
/// "cujas saídas tenham sido classificadas como…" / "cujas entradas tenham sido classificadas como…"). `None`: a
/// operação de origem não está no mapa (a definir com o contador; o Rustinho não presume).
pub fn cfop_da_devolucao(cfop_original: &str) -> Option<&'static str> {
    Some(match documento::digitos(cfop_original).as_str() {
        // devolução de venda (entrada na loja)
        "5102" => "1202",
        "6102" => "2202",
        "5403" | "5405" => "1411",
        "6403" | "6404" => "2411",
        // devolução de compra (saída para o fornecedor)
        "1102" => "5202",
        "2102" => "6202",
        "1403" => "5411",
        "2403" => "6411",
        _ => return None,
    })
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
    /// C-M7 da R1: peças sem CSOSN escolhido à mão e sem regra que decida (o CSOSN fica a definir com o contador).
    pub a_definir: Vec<PecaFiscal>,
    /// K9 da R2: peças com o CSOSN do substituto (201, 202, 203): valem só na NF-e para contribuinte. Lista própria, sem
    /// o "Passar para automático" (a 201 pode ser certa para a loja que é substituta).
    pub substituto: Vec<PecaFiscal>,
    /// C-M6 da R1: as regras do programa que não entraram (a loja tem regra própria no mesmo período).
    pub regras_puladas: Vec<String>,
    pub monofasicas: i64,
    /// G5 da R1: o monofásico ainda vale na data da conferência (até 31/12/2026).
    pub monofasico_vale: bool,
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
    let mut a_definir = Vec::new();
    let mut substituto = Vec::new();
    for (id, codigo, nome, marca, ncm, cest, csosn) in &pecas {
        let t = tributacao(conn, &uf, cest, csosn, data)?;
        let item = |motivo: String| PecaFiscal {
            id: id.clone(),
            codigo: codigo.clone(),
            nome: nome.clone(),
            marca: marca.clone(),
            ncm: ncm.clone(),
            cest: cest.clone(),
            csosn: if !csosn.is_empty() {
                csosn.clone()
            } else if t.decidido {
                format!("{} (automático)", t.csosn)
            } else {
                "a definir".to_string()
            },
            sugerido: match t.com_st {
                Some(true) => "500".into(),
                Some(false) => "102".into(),
                None => t.csosn.clone(),
            },
            cfop: t.cfop.clone(),
            motivo,
        };
        if matches!(t.csosn.as_str(), "201" | "202" | "203") {
            substituto.push(item(t.alerta.clone()));
        } else if !t.alerta.is_empty() {
            conflitos.push(item(t.explicacao.clone()));
        }
        if !t.decidido {
            a_definir.push(item(t.explicacao.clone()));
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
        a_definir,
        substituto,
        regras_puladas: config::ler(conn, CHAVE_PULADAS)?
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(str::to_string)
            .collect(),
        monofasicas,
        monofasico_vale: monofasico_vale(data),
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
                    .map(documento::digitos)
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
    /// F2-3 (b) da R1: o início é a data legal (true) ou só a data em que a regra foi conferida valendo, porque a fonte
    /// lida não informa o início (false). Só leitura (a regra da loja tem o início que o gerente digitou: true).
    pub inicio_informado: bool,
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
                origem, inicio_informado
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
            inicio_informado: l.get(11)?,
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

/// Se a linha com este id está no banco.
fn existe_regra(conn: &Connection, id: &str) -> Resultado<bool> {
    Ok(conn.query_row("SELECT COUNT(*) > 0 FROM regra_st WHERE id = ?1", [id], |l| l.get(0))?)
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
            // B3 da R1: a que já terminou não volta a valer (estender o fim reabriria dias que já passaram).
            if ate_nova > ate_antes && ate_antes < limite {
                return Err(Erro::campo(
                    "vigenteAte",
                    "Esta regra já terminou e não volta a valer. Para valer de novo, crie outra regra a partir de hoje.",
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
    /// N-1 da R5: linhas do programa que saíram do catálogo numa versão. Num banco que ainda não aplicou essa versão,
    /// a linha sai se continua do programa (a que a loja mudou fica).
    #[serde(default)]
    retiradas: Vec<Retirada>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Retirada {
    id: String,
    na_versao: i64,
    /// B-1 da R7: a UF e o CEST da regra retirada, para tirar o aviso de "não entrou" mesmo quando a linha nunca entrou
    /// no banco (foi pulada por causa de uma regra da loja).
    #[serde(default)]
    uf: String,
    #[serde(default)]
    cest_inicio: String,
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
    /// F2-3 (b): a fonte informa o início (o `vigente_de` é a data legal).
    #[serde(default)]
    inicio_informado: bool,
    /// C-M6 da R1: a versão do catálogo em que o conteúdo da linha foi definido pela última vez (0 = a `desde_versao`).
    /// Maior que a versão aplicada aqui: a linha do programa que a loja não mudou é corrigida.
    #[serde(default)]
    versao_da_linha: i64,
}

/// As regras do programa que não entraram porque a loja já tem regra do mesmo estado e CEST no mesmo período (só
/// deste computador; a conferência mostra o aviso). Texto: uma linha por regra.
pub const CHAVE_PULADAS: &str = "regras_st_catalogo_puladas";

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
    aplicar_catalogo(tx, &catalogo)
}

/// B-2 da R7: a carga de um catálogo (o do programa ou, nos testes, um feito à mão).
fn aplicar_catalogo(tx: &Connection, catalogo: &Catalogo) -> Resultado<usize> {
    let aplicada: i64 = config::ler(tx, CHAVE_CATALOGO)?.and_then(|v| v.parse().ok()).unwrap_or(0);
    if aplicada >= catalogo.versao {
        return Ok(0);
    }
    let mut entraram = 0;
    let mut corrigidas = Vec::new();
    let mut puladas: Vec<String> = config::ler(tx, CHAVE_PULADAS)?
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::to_string)
        .collect();
    // N-1 da R8: as retiradas vêm ANTES das regras. Assim, quando uma versão troca o id de uma regra no mesmo estado
    // e CEST (sai a X, entra a Y), a linha velha X sai primeiro e não "sobrepõe" a Y (antes, a Y era pulada pela X e
    // depois a X saía: o estado ficava sem regra e sem aviso).
    // C-2 da R6: a retirada não vale para um id que voltou ao catálogo (retirado numa versão e reposto noutra).
    let vivas: std::collections::HashSet<&str> = catalogo.regras.iter().map(|r| r.id.as_str()).collect();
    // Os pares (estado, CEST) cujo aviso de "não entrou" foi limpo aqui: o aviso de uma regra viva do mesmo par é
    // refeito abaixo, se ela continua de fora (N-1 da R8).
    let mut limpos: Vec<(String, String)> = Vec::new();
    for r in catalogo.retiradas.iter().filter(|r| r.na_versao > aplicada && !vivas.contains(r.id.as_str())) {
        // C-3 da R6: o registro diz a UF, o CEST e a regra, e não o id interno.
        let antes: Option<(String, String, String)> = tx
            .query_row(
                "SELECT uf, cest_inicio, descricao FROM regra_st WHERE id = ?1 AND origem = 'rustinho'",
                [&r.id],
                |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
            )
            .optional()?;
        // C-4 da R6 e B-1 da R7: o aviso de "não entrou" sobre a regra retirada sai, mesmo se a linha nunca entrou
        // (o estado e o CEST da retirada; sem eles, os da linha do banco).
        let par = if !r.uf.is_empty() && !r.cest_inicio.is_empty() {
            Some((r.uf.clone(), r.cest_inicio.clone()))
        } else {
            antes.as_ref().map(|(uf, cest, _)| (uf.clone(), cest.clone()))
        };
        if let Some((uf, cest)) = par {
            let marca = format!("{uf} CEST {cest} (");
            puladas.retain(|p| !p.starts_with(&marca));
            limpos.push((uf, cest));
        }
        let Some((uf, cest, descricao)) = antes else { continue };
        tx.execute("DELETE FROM regra_st WHERE id = ?1 AND origem = 'rustinho'", [&r.id])?;
        corrigidas.push(format!("{uf} CEST {cest}: regra do programa retirada ({descricao})"));
    }
    for r in catalogo.regras.iter() {
        let versao_da_linha = r.versao_da_linha.max(r.desde_versao);
        let refazer_aviso = r.desde_versao <= aplicada
            && limpos.iter().any(|(uf, cest)| *uf == r.uf && *cest == r.cest_inicio)
            && !existe_regra(tx, &r.id)?;
        if refazer_aviso && sobreposta(tx, &r.id, &r.uf, &r.cest_inicio, &r.vigente_de, r.vigente_ate.as_deref())? {
            let aviso = format!(
                "{} CEST {} ({}): a loja já tem regra própria no mesmo período",
                r.uf, r.cest_inicio, r.descricao
            );
            if !puladas.contains(&aviso) {
                puladas.push(aviso);
            }
        }
        if r.desde_versao > aplicada {
            if sobreposta(tx, &r.id, &r.uf, &r.cest_inicio, &r.vigente_de, r.vigente_ate.as_deref())? {
                // C-M6 da R1: a pulada não some em silêncio.
                let aviso = format!(
                    "{} CEST {} ({}): a loja já tem regra própria no mesmo período",
                    r.uf, r.cest_inicio, r.descricao
                );
                if !puladas.contains(&aviso) {
                    puladas.push(aviso);
                }
                continue;
            }
            entraram += tx.execute(
                "INSERT OR IGNORE INTO regra_st (id, uf, cest_inicio, vigente_de, vigente_ate, com_st, descricao, fonte,
                                                 dispositivo, consultada_em, origem, inicio_informado)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'rustinho', ?11)",
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
                    r.consultada_em,
                    r.inicio_informado
                ],
            )?;
        }
        // B-2 da R7: também na linha nova desta versão, se o id já estava no banco (retirado e reposto): a do programa
        // fica com o conteúdo novo (na linha recém-inserida, nada muda).
        if versao_da_linha > aplicada {
            // C-M6 da R1: a correção do programa vale para a linha que a loja não mudou (origem "rustinho").
            let n = tx.execute(
                "UPDATE regra_st SET uf = ?2, cest_inicio = ?3, vigente_de = ?4, vigente_ate = ?5, com_st = ?6,
                                     descricao = ?7, fonte = ?8, dispositivo = ?9, consultada_em = ?10,
                                     inicio_informado = ?11
                  WHERE id = ?1 AND origem = 'rustinho'
                    AND (uf, cest_inicio, vigente_de, COALESCE(vigente_ate, ''), com_st, descricao, fonte, dispositivo,
                         consultada_em, inicio_informado)
                        IS NOT (?2, ?3, ?4, COALESCE(?5, ''), ?6, ?7, ?8, ?9, ?10, ?11)",
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
                    r.consultada_em,
                    r.inicio_informado
                ],
            )?;
            if n > 0 {
                corrigidas.push(format!("{} CEST {}: {}", r.uf, r.cest_inicio, r.descricao));
            }
        }
    }
    if !corrigidas.is_empty() {
        let disp = config::ler(tx, "dispositivo_id")?.unwrap_or_default();
        corrigidas.insert(0, format!("Regras do programa corrigidas pela versão {} do catálogo", catalogo.versao));
        auditoria::registrar(tx, &disp, None, "fiscal.catalogo", "regra_st", "", &corrigidas)?;
    }
    config::gravar(tx, CHAVE_PULADAS, &puladas.join("\n"))?;
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

    /// C-M7 da R1: sem regra (sem UF, sem CEST, UF sem regra para o CEST, data fora das vigências), o Rustinho não
    /// presume: o CSOSN e o CFOP ficam a definir; o CSOSN escolhido à mão continua valendo. A conferência lista as
    /// peças a definir.
    #[test]
    fn r1_cm7_sem_regra_nao_presume() {
        let b = Banco::em_memoria().unwrap();
        for (uf, cest, data, o_que) in [
            ("AL", "01.001.00", "2026-11-26", "UF sem regra"),
            ("", "01.001.00", "2026-11-26", "sem UF"),
            ("SP", "", "2026-11-26", "sem CEST"),
            ("RS", "16.001.00", "2020-01-01", "data antes das vigências"),
        ] {
            let t = tributacao(&b.conn, uf, cest, "", data).unwrap();
            assert_eq!((t.csosn.as_str(), t.cfop.as_str(), t.decidido), ("", "", false), "{o_que}: {t:?}");
            assert!(t.alerta.is_empty(), "{o_que}: não é conflito, é falta de regra");
        }
        let manual = tributacao(&b.conn, "AL", "01.001.00", "500", "2026-11-26").unwrap();
        assert_eq!((manual.csosn.as_str(), manual.cfop.as_str(), manual.decidido), ("500", "5405", true));
        let com_regra = tributacao(&b.conn, "SP", "01.001.00", "", "2026-11-26").unwrap();
        assert_eq!((com_regra.csosn.as_str(), com_regra.decidido), ("102", true));

        use serde_json::{json, Value};
        let mut s = crate::Servico::em_memoria().unwrap();
        let chamar = |s: &mut crate::Servico, cmd: &str, d: Value| -> Value {
            let r: Value = serde_json::from_str(&s.chamar(cmd, &d.to_string())).unwrap();
            assert_eq!(r["ok"], true, "{cmd}: {r}");
            r["dados"].clone()
        };
        chamar(
            &mut s,
            "configurar_inicio",
            json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "12345678", "senha2": "12345678" }),
        );
        let mut f = chamar(&mut s, "produto_novo", json!({}));
        f["nome"] = json!("Filtro sem CEST");
        f["custo"] = json!("10");
        f["chaveOperacao"] = json!("peca-cm7");
        chamar(&mut s, "produto_salvar", f);
        let c = chamar(&mut s, "fiscal_conferir", json!({}));
        let lista = c["aDefinir"].as_array().unwrap();
        assert!(lista.iter().any(|p| p["nome"] == "Filtro sem CEST" && p["csosn"] == "a definir"), "{c}");
        // K9 da R2: a peça com CSOSN do substituto vai para a lista própria, e não para "não combina com a regra"
        // (onde o "Passar todas para automático" a trocaria).
        let mut f = chamar(&mut s, "produto_novo", json!({}));
        f["nome"] = json!("Peça substituta");
        f["custo"] = json!("10");
        f["csosn"] = json!("201");
        f["chaveOperacao"] = json!("peca-k9");
        chamar(&mut s, "produto_salvar", f);
        let c = chamar(&mut s, "fiscal_conferir", json!({}));
        let sub = c["substituto"].as_array().unwrap();
        assert!(
            sub.iter().any(|p| p["nome"] == "Peça substituta" && p["motivo"].as_str().unwrap().contains("600")),
            "{c}"
        );
        assert!(!c["conflitos"].as_array().unwrap().iter().any(|p| p["nome"] == "Peça substituta"), "{c}");
    }

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
        assert_eq!(config::ler(&b.conn, CHAVE_CATALOGO).unwrap().as_deref(), Some("6"));
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
        // C-M6 da R1: as duas do programa que a regra da loja impediu aparecem como aviso, sem sumir em silêncio.
        let puladas = conferir(&b.conn, "2026-11-26").unwrap().regras_puladas;
        assert_eq!(puladas.len(), 2, "{puladas:?}");
        assert!(puladas.iter().all(|p| p.starts_with("RS CEST 16") && p.contains("regra própria")), "{puladas:?}");
        let aplicando: i64 =
            b.conn.query_row("SELECT aplicando FROM sync_estado WHERE id = 1", [], |l| l.get(0)).unwrap();
        assert_eq!(aplicando, 0, "o registro da troca volta a ficar ligado");
    }

    /// F2-3 (a) da R1: o segmento 16 não é inteiro onde a lista da UF deixa itens de fora (PR, SC, PB, BA e RN: os
    /// recauchutados; GO e RR: também os de bicicleta). RR traz os protetores de bicicleta (o art. 783, III, exclui só
    /// pneus e câmaras). Onde a fonte lida só traz o segmento (PE), o segmento vale inteiro.
    #[test]
    fn r1_f23a_excecoes_da_lista_de_pneus_de_cada_uf() {
        let b = Banco::em_memoria().unwrap();
        let hoje = hoje(&b.conn).unwrap();
        let st = |uf: &str, cest: &str| tributacao(&b.conn, uf, cest, "", &hoje).unwrap().com_st;
        for uf in ["PR", "SC", "PB", "BA", "RN", "GO", "RR"] {
            assert_eq!(st(uf, "16.001.00"), Some(true), "{uf}: pneu de automóvel");
            assert_eq!(st(uf, "16.006.00"), Some(false), "{uf}: recauchutado fora da lista");
        }
        for uf in ["GO", "RR", "PB", "RN"] {
            assert_eq!(st(uf, "16.005.00"), Some(false), "{uf}: pneu de bicicleta");
            assert_eq!(st(uf, "16.009.00"), Some(false), "{uf}: câmara de bicicleta");
        }
        // N1 da R2: PB e RN também deixam de fora o protetor de bicicleta (RR e GO o mantêm).
        for uf in ["PB", "RN"] {
            assert_eq!(st(uf, "16.007.01"), Some(false), "{uf}: protetor de bicicleta");
            assert_eq!(st(uf, "16.007.00"), Some(true), "{uf}: protetor comum continua");
        }
        assert_eq!(st("BA", "16.005.00"), Some(true), "BA traz o de bicicleta (item 10.5)");
        // F3-2 da R3: RS (Item V, até 31/12/2026: só os itens 1 a 6) e ES (art. 265, § 6º, III: pneus e câmaras de
        // bicicleta).
        let rs = |cest: &str| tributacao(&b.conn, "RS", cest, "", "2026-12-15").unwrap().com_st;
        for cest in ["16.005.00", "16.006.00", "16.007.01", "16.009.00"] {
            assert_eq!(rs(cest), Some(false), "RS {cest}");
        }
        assert_eq!(rs("16.001.00"), Some(true));
        assert_eq!(rs("16.007.00"), Some(true));
        for cest in ["16.005.00", "16.009.00"] {
            assert_eq!(st("ES", cest), Some(false), "ES {cest}");
        }
        assert_eq!(st("ES", "16.001.00"), Some(true));
        let lista = regras(&b.conn).unwrap();
        for id in ["es-pneus-com-st", "pe-pneus-com-st", "ms-pneus-com-st"] {
            assert!(achar(&lista, id).descricao.contains("não conferimos peça por peça"), "{id}");
        }
        assert!(achar(&lista, "rn-pneus-16005-sem-st").dispositivo.contains("art. 14, § 7º"));
        // N-1 da R5 (PROVISÓRIO até a tabela inteira do art. 28 ser lida na fonte oficial): a tabela do PR guardada está,
        // provavelmente, incompleta (o § 5º cita a posição 125; o Anexo Único do Prot. ICMS 97/2010, revogado pelo Prot.
        // 96/22, tinha 125 itens): o PR fica com o segmento 01 inteiro com ST e o aviso.
        let pr = |cest: &str| st("PR", cest);
        for cest in ["01.001.00", "01.058.00", "01.070.00", "01.099.00", "01.999.00"] {
            assert_eq!(pr(cest), Some(true), "PR {cest}");
        }
        assert!(!lista.iter().any(|r| r.id.starts_with("pr-autopecas-") && r.id.ends_with("-sem-st")));
        for id in [
            "pr-autopecas-com-st",
            "ba-autopecas-com-st",
            "pe-autopecas-com-st",
            "ms-autopecas-com-st",
            "mt-autopecas-com-st",
        ] {
            assert!(achar(&lista, id).descricao.contains("não conferimos peça por peça"), "{id}");
        }
        assert_eq!(st("RR", "16.007.01"), Some(true), "RR: protetor de bicicleta continua");
        assert_eq!(st("RR", "16.008.00"), Some(true));
        assert_eq!(st("PR", "16.005.00"), Some(true), "PR traz o de bicicleta");
        assert_eq!(st("PE", "16.006.00"), Some(true), "PE: o segmento inteiro");
        let rr = tributacao(&b.conn, "RR", "16.005.00", "", &hoje).unwrap();
        assert_eq!((rr.csosn.as_str(), rr.cfop.as_str()), ("102", "5102"));
    }

    /// F2-3 (b) da R1: a data da regra é legal só quando a fonte informa o início; senão, é a data em que foi conferida
    /// valendo, e a tela não a mostra como "desde".
    #[test]
    fn r1_f23b_inicio_informado_so_com_a_fonte() {
        let b = Banco::em_memoria().unwrap();
        let lista = regras(&b.conn).unwrap();
        for (id, informado) in [
            ("pr-pneus-com-st", false),
            ("pb-autopecas-com-st", false),
            ("mt-autopecas-com-st", false),
            ("sp-autopecas-com-st", false),
            ("sp-autopecas-sem-st", true),
            ("rs-pneus-sem-st", true),
            ("sc-autopecas-sem-st", true),
            ("rr-pneus-16005-sem-st", false),
        ] {
            assert_eq!(achar(&lista, id).inicio_informado, informado, "{id}");
        }
    }

    /// C-M5 da R1: a 0032 só carimba a fonte oficial na regra de SP que está exatamente como a 0002 a criou; a que a
    /// loja mudou continua da loja, sem fonte.
    #[test]
    fn r1_cm5_a_0032_nao_carimba_a_regra_que_a_loja_mudou() {
        let mut c = rusqlite::Connection::open_in_memory().unwrap();
        crate::banco::registrar_funcoes(&c).unwrap();
        crate::banco::migrar_ate(&mut c, 31).unwrap();
        c.execute("UPDATE regra_st SET descricao = 'Mudada pelo contador' WHERE id = 'sp-autopecas-com-st'", [])
            .unwrap();
        c.execute("UPDATE regra_st SET vigente_ate = '2027-06-30' WHERE id = 'sp-pneus-sem-st'", []).unwrap();
        crate::banco::migrar(&mut c).unwrap();
        let linha = |id: &str| -> (String, String) {
            c.query_row("SELECT origem, fonte FROM regra_st WHERE id = ?1", [id], |l| Ok((l.get(0)?, l.get(1)?)))
                .unwrap()
        };
        assert_eq!(linha("sp-autopecas-com-st"), ("loja".to_string(), String::new()));
        assert_eq!(linha("sp-pneus-sem-st"), ("loja".to_string(), String::new()));
        assert_eq!(linha("sp-autopecas-sem-st").0, "rustinho");
        assert!(linha("sp-pneus-com-st").1.contains("SRE 34/2026"));
    }

    /// C-M6 da R1: a versão nova do catálogo corrige a linha do programa que a loja não mudou (com registro), e não
    /// toca na que a loja mudou.
    #[test]
    fn r1_cm6_o_catalogo_corrige_a_linha_do_programa_e_nao_a_da_loja() {
        let b = Banco::em_memoria().unwrap();
        b.conn
            .execute(
                "UPDATE regra_st SET descricao = 'versão errada', inicio_informado = 1 WHERE id = 'pr-pneus-com-st'",
                [],
            )
            .unwrap();
        b.conn
            .execute("UPDATE regra_st SET descricao = 'da loja', origem = 'loja' WHERE id = 'rr-pneus-com-st'", [])
            .unwrap();
        config::gravar(&b.conn, CHAVE_CATALOGO, "1").unwrap();
        crate::sync::sem_registrar(&b.conn, carregar_regras_do_programa).unwrap();
        let lista = regras(&b.conn).unwrap();
        let pr = achar(&lista, "pr-pneus-com-st");
        assert_eq!((pr.descricao.as_str(), pr.inicio_informado), ("Pneus e câmaras com ST", false));
        assert_eq!(achar(&lista, "rr-pneus-com-st").descricao, "da loja");
        let registro: String = b
            .conn
            .query_row("SELECT detalhes FROM auditoria WHERE acao = 'fiscal.catalogo'", [], |l| l.get(0))
            .unwrap();
        assert!(registro.contains("PR CEST 16") && !registro.contains("RR CEST 16 "), "{registro}");
        // De novo, nada muda (a mesma versão).
        assert_eq!(crate::sync::sem_registrar(&b.conn, carregar_regras_do_programa).unwrap(), 0);
    }

    /// C-2 e C-8 da R6: as listas `regras` e `retiradas` do catálogo não se cruzam; e um banco que aplicou a versão 5 (com
    /// as 16 linhas do PR) fica sem elas ao aplicar a 6, com o registro legível e sem o aviso de "não entrou".
    #[test]
    fn r6_c2_retiradas_do_catalogo() {
        let catalogo: Catalogo = serde_json::from_str(include_str!("regras_st_programa.json")).unwrap();
        let vivas: std::collections::HashSet<&str> = catalogo.regras.iter().map(|r| r.id.as_str()).collect();
        assert!(
            catalogo.retiradas.iter().all(|r| !vivas.contains(r.id.as_str())),
            "uma retirada também está em regras"
        );
        assert_eq!(catalogo.retiradas.len(), 16);
        let b = Banco::em_memoria().unwrap();
        for r in &catalogo.retiradas {
            let cest = r.id.trim_start_matches("pr-autopecas-").trim_end_matches("-sem-st");
            b.conn
                .execute(
                    "INSERT INTO regra_st (id, uf, cest_inicio, vigente_de, vigente_ate, com_st, descricao, origem)
                     VALUES (?1, 'PR', ?2, '2026-11-26', NULL, 0, 'Autopeças fora da lista do PR', 'rustinho')",
                    params![r.id, cest],
                )
                .unwrap();
        }
        config::gravar(&b.conn, CHAVE_PULADAS, "PR CEST 0108 (x): a loja já tem regra própria no mesmo período")
            .unwrap();
        config::gravar(&b.conn, CHAVE_CATALOGO, "5").unwrap();
        crate::sync::sem_registrar(&b.conn, carregar_regras_do_programa).unwrap();
        let lista = regras(&b.conn).unwrap();
        assert!(!lista.iter().any(|r| r.id.starts_with("pr-autopecas-") && r.id.ends_with("-sem-st")));
        let registro: String = b
            .conn
            .query_row("SELECT detalhes FROM auditoria WHERE acao = 'fiscal.catalogo'", [], |l| l.get(0))
            .unwrap();
        assert!(registro.contains("PR CEST 0108: regra do programa retirada"), "{registro}");
        assert!(!registro.contains("pr-autopecas-"), "C-3: sem o id interno: {registro}");
        assert!(conferir(&b.conn, "2026-11-26").unwrap().regras_puladas.is_empty(), "C-4: o aviso da retirada sai");
    }

    /// B-1 e B-2 da R7, com catálogos feitos à mão: (1) um id retirado numa versão e reposto noutra fica no banco com o
    /// conteúdo novo, mesmo num banco que pula versões; (2) o aviso de "não entrou" de uma regra retirada sai mesmo se a
    /// linha nunca entrou (foi pulada por causa da regra da loja). E toda retirada do catálogo real tem UF e CEST.
    #[test]
    fn r7_b1_b2_retiradas_com_catalogo_feito_a_mao() {
        let real: Catalogo = serde_json::from_str(include_str!("regras_st_programa.json")).unwrap();
        assert!(real.retiradas.iter().all(|r| !r.uf.is_empty() && !r.cest_inicio.is_empty()));
        // (1) reposta: o banco aplicou a 5 e tem a linha velha; o catálogo 7 a retira na 6 e a repõe na 7.
        let b = Banco::em_memoria().unwrap();
        b.conn
            .execute(
                "INSERT INTO regra_st (id, uf, cest_inicio, vigente_de, vigente_ate, com_st, descricao, origem)
                 VALUES ('x-reposta', 'AL', '99', '2026-01-01', NULL, 1, 'velha', 'rustinho')",
                [],
            )
            .unwrap();
        config::gravar(&b.conn, CHAVE_CATALOGO, "5").unwrap();
        let cat: Catalogo = serde_json::from_value(serde_json::json!({
            "versao": 7,
            "regras": [{ "id": "x-reposta", "uf": "AL", "cestInicio": "99", "vigenteDe": "2026-01-01", "vigenteAte": null,
                         "comSt": false, "descricao": "nova", "fonte": "f", "dispositivo": "d", "consultadaEm": "2026-11-26",
                         "desdeVersao": 7, "versaoDaLinha": 7 }],
            "retiradas": [{ "id": "x-reposta", "naVersao": 6, "uf": "AL", "cestInicio": "99" }]
        }))
        .unwrap();
        crate::sync::sem_registrar(&b.conn, |tx| aplicar_catalogo(tx, &cat)).unwrap();
        let (desc, com_st): (String, bool) = b
            .conn
            .query_row("SELECT descricao, com_st FROM regra_st WHERE id = 'x-reposta'", [], |l| {
                Ok((l.get(0)?, l.get(1)?))
            })
            .unwrap();
        assert_eq!((desc.as_str(), com_st), ("nova", false), "a reposta fica, com o conteúdo novo");
        // (2) a pulada sem linha: a regra retirada nunca entrou no banco (a loja tinha a dela).
        let b = Banco::em_memoria().unwrap();
        config::gravar(&b.conn, CHAVE_PULADAS, "PR CEST 0108 (x): a loja já tem regra própria no mesmo período")
            .unwrap();
        config::gravar(&b.conn, CHAVE_CATALOGO, "5").unwrap();
        let cat: Catalogo = serde_json::from_value(serde_json::json!({
            "versao": 6, "regras": [],
            "retiradas": [{ "id": "pr-autopecas-0108-sem-st", "naVersao": 6, "uf": "PR", "cestInicio": "0108" }]
        }))
        .unwrap();
        crate::sync::sem_registrar(&b.conn, |tx| aplicar_catalogo(tx, &cat)).unwrap();
        assert!(conferir(&b.conn, "2026-11-26").unwrap().regras_puladas.is_empty(), "o aviso da regra retirada saiu");
    }

    /// N-1 e N-2 da R8: a limpeza do aviso pega só a regra retirada, e a troca do id no mesmo estado e CEST não deixa
    /// o estado sem regra nem sem aviso.
    #[test]
    fn r8_n1_n2_retirada_nao_atinge_regra_viva() {
        let regra = |id: &str, uf: &str, cest: &str, desde: i64| {
            serde_json::json!({ "id": id, "uf": uf, "cestInicio": cest, "vigenteDe": "2026-01-01", "vigenteAte": null,
                                "comSt": true, "descricao": id, "fonte": "f", "dispositivo": "d",
                                "consultadaEm": "2026-11-26", "desdeVersao": desde, "versaoDaLinha": desde })
        };
        let puladas = |b: &Banco| -> Vec<String> {
            config::ler(&b.conn, CHAVE_PULADAS).unwrap().unwrap_or_default().lines().map(str::to_string).collect()
        };
        // (N-2) Os avisos de outro CEST que começa com o retirado e de outro estado com o mesmo CEST ficam; o da
        // retirada de outro estado (SC 0109) sai também (a marca usa o estado da retirada, e não um fixo).
        let b = Banco::em_memoria().unwrap();
        let outros = [
            "PR CEST 010800 (y): a loja já tem regra própria no mesmo período",
            "SC CEST 0108 (z): a loja já tem regra própria no mesmo período",
        ];
        config::gravar(
            &b.conn,
            CHAVE_PULADAS,
            &format!(
                "PR CEST 0108 (x): a loja já tem regra própria no mesmo período\n{}\n\
                 SC CEST 0109 (w): a loja já tem regra própria no mesmo período",
                outros.join("\n")
            ),
        )
        .unwrap();
        config::gravar(&b.conn, CHAVE_CATALOGO, "5").unwrap();
        let cat: Catalogo = serde_json::from_value(serde_json::json!({
            "versao": 6, "regras": [],
            "retiradas": [{ "id": "pr-autopecas-0108-sem-st", "naVersao": 6, "uf": "PR", "cestInicio": "0108" },
                          { "id": "sc-0109", "naVersao": 6, "uf": "SC", "cestInicio": "0109" }]
        }))
        .unwrap();
        crate::sync::sem_registrar(&b.conn, |tx| aplicar_catalogo(tx, &cat)).unwrap();
        assert_eq!(puladas(&b), outros.to_vec(), "só os avisos das regras retiradas saem");
        // (N-1, E10) O id trocado no mesmo estado e CEST: sai a X, entra a Y (antes, a X "sobrepunha" a Y, que era
        // pulada, e depois a X saía: o AL ficava sem regra e sem aviso).
        let b = Banco::em_memoria().unwrap();
        b.conn
            .execute(
                "INSERT INTO regra_st (id, uf, cest_inicio, vigente_de, vigente_ate, com_st, descricao, origem)
                 VALUES ('x-velha', 'AL', '98', '2026-01-01', NULL, 1, 'x-velha', 'rustinho')",
                [],
            )
            .unwrap();
        config::gravar(&b.conn, CHAVE_CATALOGO, "5").unwrap();
        let cat: Catalogo = serde_json::from_value(serde_json::json!({
            "versao": 7, "regras": [regra("y-nova", "AL", "98", 7)],
            "retiradas": [{ "id": "x-velha", "naVersao": 7, "uf": "AL", "cestInicio": "98" }]
        }))
        .unwrap();
        crate::sync::sem_registrar(&b.conn, |tx| aplicar_catalogo(tx, &cat)).unwrap();
        let ids: Vec<String> = regras(&b.conn).unwrap().into_iter().filter(|r| r.uf == "AL").map(|r| r.id).collect();
        assert_eq!(ids, vec!["y-nova".to_string()], "a nova entra no lugar da velha");
        assert!(puladas(&b).is_empty(), "{:?}", puladas(&b));
        // (N-1, E4) Uma regra viva antiga do mesmo estado e CEST, que continua de fora (a loja tem a dela), fica com o
        // aviso, mesmo com a retirada de outra regra daquele par.
        let b = Banco::em_memoria().unwrap();
        b.conn
            .execute(
                "INSERT INTO regra_st (id, uf, cest_inicio, vigente_de, vigente_ate, com_st, descricao, origem)
                 VALUES ('da-loja', 'AL', '97', '2025-01-01', NULL, 0, 'da loja', 'loja')",
                [],
            )
            .unwrap();
        let aviso = "AL CEST 97 (z-viva): a loja já tem regra própria no mesmo período";
        config::gravar(&b.conn, CHAVE_PULADAS, aviso).unwrap();
        config::gravar(&b.conn, CHAVE_CATALOGO, "5").unwrap();
        let cat: Catalogo = serde_json::from_value(serde_json::json!({
            "versao": 7, "regras": [regra("z-viva", "AL", "97", 3)],
            "retiradas": [{ "id": "x-retirada", "naVersao": 7, "uf": "AL", "cestInicio": "97" }]
        }))
        .unwrap();
        crate::sync::sem_registrar(&b.conn, |tx| aplicar_catalogo(tx, &cat)).unwrap();
        assert_eq!(puladas(&b), vec![aviso.to_string()], "o aviso da regra viva fica");
        // (B-2 da R9) A regra viva antiga do mesmo par, que NÃO se sobrepõe mais (a regra da loja acabou antes do
        // começo dela), não ganha o aviso de volta: o aviso só volta para quem continua de fora por sobreposição.
        let b = Banco::em_memoria().unwrap();
        b.conn
            .execute(
                "INSERT INTO regra_st (id, uf, cest_inicio, vigente_de, vigente_ate, com_st, descricao, origem)
                 VALUES ('da-loja', 'AL', '97', '2024-01-01', '2024-12-31', 0, 'da loja', 'loja')",
                [],
            )
            .unwrap();
        config::gravar(&b.conn, CHAVE_PULADAS, aviso).unwrap();
        config::gravar(&b.conn, CHAVE_CATALOGO, "5").unwrap();
        crate::sync::sem_registrar(&b.conn, |tx| aplicar_catalogo(tx, &cat)).unwrap();
        assert!(puladas(&b).is_empty(), "sem sobreposição, sem aviso: {:?}", puladas(&b));
        // E a regra da loja continua no banco (a retirada nunca apaga a da loja).
        assert!(existe_regra(&b.conn, "da-loja").unwrap());
    }

    /// N-1 da R5: a linha do programa retirada numa versão sai do banco que aplicou a versão anterior; a que a loja mudou
    /// fica.
    #[test]
    fn r5_n1_linha_retirada_do_catalogo_sai_do_banco() {
        let b = Banco::em_memoria().unwrap();
        for (id, origem) in [("pr-autopecas-0108-sem-st", "rustinho"), ("pr-autopecas-0109-sem-st", "loja")] {
            b.conn
                .execute(
                    "INSERT INTO regra_st (id, uf, cest_inicio, vigente_de, vigente_ate, com_st, descricao, origem)
                     VALUES (?1, 'PR', substr(?1, 14, 4), '2026-11-26', NULL, 0, 'x', ?2)",
                    params![id, origem],
                )
                .unwrap();
        }
        config::gravar(&b.conn, CHAVE_CATALOGO, "5").unwrap();
        crate::sync::sem_registrar(&b.conn, carregar_regras_do_programa).unwrap();
        let lista = regras(&b.conn).unwrap();
        assert!(!lista.iter().any(|r| r.id == "pr-autopecas-0108-sem-st"), "a do programa saiu");
        assert!(lista.iter().any(|r| r.id == "pr-autopecas-0109-sem-st"), "a da loja ficou");
        let registro: String = b
            .conn
            .query_row("SELECT detalhes FROM auditoria WHERE acao = 'fiscal.catalogo'", [], |l| l.get(0))
            .unwrap();
        assert!(registro.contains("PR CEST 0108: regra do programa retirada"), "{registro}");
    }

    /// B3 da R1: a regra que já terminou não volta a valer (estender o fim reabriria dias que já passaram); a que está
    /// valendo pode ter o fim mudado para depois de ontem.
    #[test]
    fn r1_b3_a_regra_que_ja_terminou_nao_volta_a_valer() {
        let b = Banco::em_memoria().unwrap();
        let u = crate::usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        let velha = RegraSt {
            uf: "AL".into(),
            cest_inicio: "01".into(),
            vigente_de: dia(&b, -60),
            vigente_ate: dia(&b, -30),
            com_st: true,
            ..Default::default()
        };
        let id = salvar_regra(&b.conn, &b.dispositivo, &u.id, &velha).unwrap();
        let mut estendida = achar(&regras(&b.conn).unwrap(), &id).clone();
        estendida.vigente_ate = dia(&b, 30);
        assert!(salvar_regra(&b.conn, &b.dispositivo, &u.id, &estendida).is_err());
        estendida.vigente_ate = String::new();
        assert!(salvar_regra(&b.conn, &b.dispositivo, &u.id, &estendida).is_err(), "nem sem fim");
        assert_eq!(achar(&regras(&b.conn).unwrap(), &id).vigente_ate, dia(&b, -30));
        // A que está valendo: o fim muda para a frente.
        let atual = RegraSt {
            uf: "AL".into(),
            cest_inicio: "16".into(),
            vigente_de: dia(&b, -10),
            vigente_ate: dia(&b, 10),
            com_st: true,
            ..Default::default()
        };
        let id2 = salvar_regra(&b.conn, &b.dispositivo, &u.id, &atual).unwrap();
        let mut mais = achar(&regras(&b.conn).unwrap(), &id2).clone();
        mais.vigente_ate = dia(&b, 40);
        salvar_regra(&b.conn, &b.dispositivo, &u.id, &mais).unwrap();
    }

    /// B6 da R1: um erro ao aplicar o catálogo não impede abrir o banco (fica anotado; a próxima abertura tenta de novo).
    #[test]
    fn r1_b6_erro_no_catalogo_nao_impede_abrir_o_banco() {
        let pasta = std::env::temp_dir().join(format!("rustinho-b6-{}", crate::tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let caminho = pasta.join("rustinho.db");
        {
            let b = Banco::abrir(&caminho).unwrap();
            config::gravar(&b.conn, CHAVE_CATALOGO, "0").unwrap();
            b.conn
                .execute_batch("CREATE TEMP TABLE x (y); DELETE FROM regra_st WHERE id = 'go-pneus-com-st';")
                .unwrap();
        }
        {
            let c = rusqlite::Connection::open(&caminho).unwrap();
            c.execute_batch(
                "CREATE TRIGGER t_b6 BEFORE INSERT ON regra_st BEGIN SELECT RAISE(ABORT, 'falha de teste'); END;",
            )
            .unwrap();
        }
        let b = Banco::abrir(&caminho).expect("o banco abre mesmo com o catálogo falhando");
        assert_eq!(config::ler(&b.conn, CHAVE_CATALOGO).unwrap().as_deref(), Some("0"), "nada do catálogo gravado");
        b.conn.execute_batch("DROP TRIGGER t_b6;").unwrap();
        drop(b);
        let b = Banco::abrir(&caminho).unwrap();
        assert_eq!(config::ler(&b.conn, CHAVE_CATALOGO).unwrap().as_deref(), Some("6"), "a próxima abertura aplica");
        assert!(regras(&b.conn).unwrap().iter().any(|r| r.id == "go-pneus-com-st"));
        drop(b);
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// F2-4, G5, G6 e G8 da R1.
    #[test]
    fn r1_t019_substituto_monofasico_modelo_e_devolucao() {
        let b = Banco::em_memoria().unwrap();
        for csosn in ["201", "202", "203"] {
            let t = tributacao(&b.conn, "SP", "01.001.00", csosn, "2026-11-26").unwrap();
            assert_eq!(t.cfop, "5403");
            assert!(
                t.alerta.contains("só vale na NF-e") && t.alerta.contains("383") && t.alerta.contains("600"),
                "{csosn}: {}",
                t.alerta
            );
            // C-8 da R6: o texto simples, com o exemplo certo do não contribuinte.
            assert!(t.alerta.contains("por exemplo uma pessoa física"), "{}", t.alerta);
        }
        assert!(tributacao(&b.conn, "SP", "01.001.00", "102", "2026-11-26").unwrap().alerta.is_empty());
        // G5: o monofásico vale até 31/12/2026.
        assert!(monofasico_vale("2026-12-31"));
        assert!(!monofasico_vale("2027-01-01"));
        assert!(!conferir(&b.conn, "2027-01-02").unwrap().monofasico_vale);
        // G6: crédito do ICMS para o comprador → NF-e, desde 05/10/2026.
        assert_eq!(modelo_da_venda(true, "2026-11-26"), 55);
        assert_eq!(modelo_da_venda(false, "2026-11-26"), 65);
        assert_eq!(modelo_da_venda(true, "2026-10-04"), 65, "antes dos efeitos do ajuste");
        // G8: o CFOP da devolução pela operação de origem; fora do mapa, nada é presumido.
        for (de, para) in [
            ("5.102", "1202"),
            ("6102", "2202"),
            ("5405", "1411"),
            ("5403", "1411"),
            ("6404", "2411"),
            ("1102", "5202"),
            ("2.102", "6202"),
            ("1403", "5411"),
            ("2403", "6411"),
        ] {
            assert_eq!(cfop_da_devolucao(de), Some(para), "{de}");
        }
        assert_eq!(cfop_da_devolucao("5949"), None);
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
        assert_eq!(lista.len(), 60, "4 de SP (0002/0032) e 56 do catálogo, versão 6");
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
        assert!(achar(&lista, &id).inicio_informado, "a regra da loja tem o início que o gerente digitou");
        assert_eq!(lista.len(), 61);

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
