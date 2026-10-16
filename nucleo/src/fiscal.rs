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
    let cfop = if matches!(csosn.as_str(), "500" | "201" | "202" | "203") { "5405" } else { "5102" }.to_string();
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
            ipi_selo_quantidade: numeros_ate(&self.ipi_selo_quantidade, "fiscal.ipiSeloQuantidade", "Quantidade de selos", 12)?,
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
