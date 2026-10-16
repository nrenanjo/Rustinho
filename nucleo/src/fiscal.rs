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
    let cfop = if csosn == "500" { "5405" } else { "5102" }.to_string();
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
    /// Peças com NCM de autopeça (capítulos 40, 68, 70, 73, 83, 84, 85, 87...) sem CEST.
    pub sem_cest: Vec<PecaFiscal>,
    pub monofasicas: i64,
    pub total_pecas: i64,
    pub regras: Vec<RegraSt>,
}

/// Monta a conferência das peças ativas.
pub fn conferir(conn: &Connection, data: &str) -> Resultado<Conferencia> {
    let uf = uf_loja(conn)?;
    let mut consulta = conn.prepare(
        "SELECT id, codigo, nome, marca, ncm, cest, csosn FROM produto WHERE ativo = 1 ORDER BY nome COLLATE NOCASE",
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
        } else if cest.is_empty() && provavel_autopeca(ncm) {
            sem_cest.push(item("NCM de autopeça sem CEST".into()));
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

/// Capítulos do NCM onde estão quase todas as autopeças. Só serve para
/// lembrar de conferir o CEST; não decide nada.
fn provavel_autopeca(ncm: &str) -> bool {
    matches!(&ncm[..2.min(ncm.len())], "40" | "68" | "70" | "73" | "83" | "84" | "85" | "87" | "90" | "91" | "94")
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
// Regras de ST (o contador pode mudar).
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
}

pub fn regras(conn: &Connection) -> Resultado<Vec<RegraSt>> {
    let mut c = conn.prepare(
        "SELECT id, uf, cest_inicio, vigente_de, vigente_ate, com_st, descricao FROM regra_st
          ORDER BY uf, cest_inicio, vigente_de",
    )?;
    let linhas = c.query_map([], |l| {
        let ate: Option<String> = l.get(4)?;
        Ok(RegraSt {
            id: l.get(0)?,
            uf: l.get(1)?,
            cest_inicio: l.get(2)?,
            vigente_de: data_br(&l.get::<_, String>(3)?),
            vigente_ate: ate.map(|a| data_br(&a)).unwrap_or_default(),
            com_st: l.get(5)?,
            descricao: l.get(6)?,
        })
    })?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

/// Troca todas as regras pelas da tela.
pub fn salvar_regras(conn: &Connection, dispositivo: &str, usuario_id: &str, lista: &[RegraSt]) -> Resultado<()> {
    let mut novas = Vec::new();
    for (i, r) in lista.iter().enumerate() {
        let campo = |n: &str| format!("regras.{i}.{n}");
        let uf = r.uf.trim().to_uppercase();
        let cest = documento::digitos(&r.cest_inicio);
        if uf.is_empty() && cest.is_empty() && r.vigente_de.trim().is_empty() {
            continue;
        }
        if !UFS.contains(&uf.as_str()) {
            return Err(Erro::campo(&campo("uf"), "Escolha o estado."));
        }
        if cest.is_empty() || cest.len() > 7 {
            return Err(Erro::campo(&campo("cestInicio"), "Digite o começo do CEST (ex.: 01 para autopeças)."));
        }
        let de = ler_data(&r.vigente_de).map_err(|m| Erro::campo(&campo("vigenteDe"), m))?
            .ok_or_else(|| Erro::campo(&campo("vigenteDe"), "Digite a data de início."))?;
        let ate = ler_data(&r.vigente_ate).map_err(|m| Erro::campo(&campo("vigenteAte"), m))?;
        if let Some(a) = &ate {
            if *a < de {
                return Err(Erro::campo(&campo("vigenteAte"), "A data final é antes da inicial."));
            }
        }
        let descricao: String = r.descricao.trim().chars().take(120).collect();
        novas.push((uf, cest, de, ate, r.com_st, descricao));
    }
    // Duas regras do mesmo CEST e estado não podem valer no mesmo dia.
    for (i, a) in novas.iter().enumerate() {
        for b in novas.iter().skip(i + 1) {
            if a.0 == b.0 && a.1 == b.1 {
                let fim_a = a.3.clone().unwrap_or_else(|| "9999-12-31".into());
                let fim_b = b.3.clone().unwrap_or_else(|| "9999-12-31".into());
                if a.2 <= fim_b && b.2 <= fim_a {
                    return Err(Erro::validacao(format!(
                        "Há duas regras para {} CEST {} valendo nas mesmas datas.",
                        a.0, a.1
                    )));
                }
            }
        }
    }
    conn.execute("DELETE FROM regra_st", [])?;
    for (uf, cest, de, ate, com_st, descricao) in &novas {
        conn.execute(
            "INSERT INTO regra_st (id, uf, cest_inicio, vigente_de, vigente_ate, com_st, descricao)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![tempo::novo_id(), uf, cest, de, ate, com_st, descricao],
        )?;
    }
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "fiscal.regras",
        "fiscal",
        "regra_st",
        &[format!("{} regras de ST salvas", novas.len())],
    )?;
    Ok(())
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
    let (Ok(d), Ok(m), Ok(mut a)) = (d.parse::<u32>(), m.parse::<u32>(), a.parse::<i32>()) else {
        return Err("Data inválida (use dd/mm/aaaa).".into());
    };
    if a < 100 {
        a += 2000;
    }
    let mes = time::Month::try_from(m as u8).map_err(|_| "Mês inválido.".to_string())?;
    time::Date::from_calendar_date(a, mes, d as u8).map_err(|_| "Data inválida.".to_string())?;
    Ok(Some(format!("{a:04}-{m:02}-{d:02}")))
}

/// "2026-10-01" → "01/10/2026".
pub fn data_br(iso: &str) -> String {
    let d = &iso[..10.min(iso.len())];
    let p: Vec<&str> = d.split('-').collect();
    if p.len() == 3 {
        format!("{}/{}/{}", p[2], p[1], p[0])
    } else {
        iso.to_string()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
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

    #[test]
    fn regras_editaveis_e_conferencia() {
        let b = Banco::em_memoria().unwrap();
        let u = crate::usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        // regra sobreposta é recusada
        let mut lista = regras(&b.conn).unwrap();
        assert_eq!(lista.len(), 4);
        lista.push(RegraSt { uf: "SP".into(), cest_inicio: "01".into(), vigente_de: "15/10/2026".into(), com_st: true, ..Default::default() });
        assert!(salvar_regras(&b.conn, &b.dispositivo, &u.id, &lista).is_err());
        // regra mais específica (01.003) vence a geral
        lista.pop();
        lista.push(RegraSt { uf: "SP".into(), cest_inicio: "01.003".into(), vigente_de: "01/10/2026".into(), com_st: true, descricao: "Exceção de teste".into(), ..Default::default() });
        salvar_regras(&b.conn, &b.dispositivo, &u.id, &lista).unwrap();
        assert_eq!(tributacao(&b.conn, "SP", "0100300", "", "2026-10-13").unwrap().csosn, "500");
        assert_eq!(tributacao(&b.conn, "SP", "0100100", "", "2026-10-13").unwrap().csosn, "102");

        assert_eq!(ler_data("1/10/26").unwrap().unwrap(), "2026-10-01");
        assert!(ler_data("31/02/2026").is_err());
        assert_eq!(data_br("2026-10-01"), "01/10/2026");
    }
}
