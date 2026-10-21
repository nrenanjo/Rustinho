//! Contas a pagar: o que a loja deve (duplicatas das compras, aluguel,
//! energia, comissões...) e os pagamentos.
//!
//! Como no crediário, o que já foi pago nunca fica gravado na parcela: sai
//! da soma dos pagamentos não estornados (vista `conta_pagar_saldo`).
//! O dinheiro sai de uma conta da loja (Caixa geral, banco) ou da gaveta do
//! caixa aberto (vira uma sangria).

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use time::Duration;

use crate::erro::{Erro, Resultado};
use crate::receber::{dias_entre, iso, ler_iso, parcelar, MAX_PARCELAS};
use crate::{auditoria, caixa, fiscal, nfe_xml, numeros, tempo, texto};

/// Formas de pagar uma conta: (código, nome).
pub const FORMAS: &[(&str, &str)] = &[
    ("dinheiro", "Dinheiro"),
    ("pix", "Pix"),
    ("boleto", "Boleto"),
    ("transferencia", "Transferência"),
    ("cartao", "Cartão"),
    ("cheque", "Cheque"),
    ("debito_automatico", "Débito automático"),
];

pub fn nome_forma(forma: &str) -> &'static str {
    FORMAS.iter().find(|f| f.0 == forma).map(|f| f.1).unwrap_or("Outro")
}

fn ler_valor(texto: &str, campo: &str, obrigatorio: bool) -> Resultado<i64> {
    match numeros::ler_dinheiro(texto).map_err(|m| Erro::campo(campo, m))? {
        Some(v) if v > 0 => Ok(v),
        Some(v) if v < 0 => Err(Erro::campo(campo, "O valor não pode ser negativo.")),
        _ if obrigatorio => Err(Erro::campo(campo, "Digite o valor.")),
        _ => Ok(0),
    }
}

fn ler_data_ou(conn: &Connection, texto: &str, campo: &str, padrao: Option<String>) -> Resultado<String> {
    match fiscal::ler_data(texto).map_err(|m| Erro::campo(campo, m))? {
        Some(d) => Ok(d),
        None => match padrao {
            Some(p) => Ok(p),
            None => fiscal::hoje(conn),
        },
    }
}

// ---------------------------------------------------------------------------
// Lançar.
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LancamentoEntrada {
    pub fornecedor_id: String,
    pub categoria_id: String,
    pub descricao: String,
    pub documento: String,
    pub valor: String,
    pub parcelas: i64,
    /// Vazio = hoje.
    pub primeiro_vencimento: String,
    pub observacoes: String,
    /// Mês a que a conta se refere ("10/2026"), para a DRE. Vazio = o mês
    /// do vencimento de cada parcela.
    pub competencia: String,
}

/// "10/2026", "10/26", "2026-10" → "2026-10". Vazio = sem competência.
pub fn ler_competencia(texto: &str) -> Resultado<Option<String>> {
    let t = texto.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let erro = || Erro::campo("competencia", "Competência inválida (use mês/ano, ex.: 10/2026).");
    let partes: Vec<&str> = t.split(['/', '-', '.']).collect();
    let (m, a) = match partes.as_slice() {
        [a, m] if a.len() == 4 => (*m, *a),
        [m, a] => (*m, *a),
        _ => return Err(erro()),
    };
    let (Ok(m), Ok(mut a)) = (m.parse::<u32>(), a.parse::<i32>()) else { return Err(erro()) };
    if a < 100 {
        a += 2000;
    }
    if !(1..=12).contains(&m) || !(2000..=2100).contains(&a) {
        return Err(erro());
    }
    Ok(Some(format!("{a:04}-{m:02}")))
}

/// "2026-10" → "10/2026".
pub fn escrever_competencia(c: &str) -> String {
    if c.len() == 7 {
        format!("{}/{}", &c[5..7], &c[..4])
    } else {
        c.to_string()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParcelaPrevia {
    pub numero: i64,
    pub vencimento: String,
    pub valor: String,
}

/// Parcelas mensais a partir do 1º vencimento (vazio = hoje).
pub fn previa(conn: &Connection, valor: i64, parcelas: i64, primeiro: &str) -> Resultado<Vec<(String, i64)>> {
    let n = if parcelas <= 0 { 1 } else { parcelas };
    if n > MAX_PARCELAS {
        return Err(Erro::campo("parcelas", format!("Use no máximo {MAX_PARCELAS} parcelas.")));
    }
    if valor <= 0 {
        return Ok(vec![]);
    }
    let inicio = ler_iso(&ler_data_ou(conn, primeiro, "primeiroVencimento", None)?)?;
    Ok(parcelar(inicio, inicio.day(), valor, n))
}

pub fn previa_texto(conn: &Connection, e: &LancamentoEntrada) -> Resultado<Vec<ParcelaPrevia>> {
    let valor = numeros::ler_dinheiro(&e.valor).map_err(|m| Erro::campo("valor", m))?.unwrap_or(0);
    Ok(previa(conn, valor, e.parcelas, &e.primeiro_vencimento)?
        .into_iter()
        .enumerate()
        .map(|(i, (v, x))| ParcelaPrevia {
            numero: i as i64 + 1,
            vencimento: fiscal::data_br(&v),
            valor: numeros::escrever_dinheiro(x),
        })
        .collect())
}

fn categoria_valida(conn: &Connection, id: &str) -> Resultado<Option<String>> {
    if id.is_empty() {
        return Ok(None);
    }
    let existe = conn.query_row("SELECT 1 FROM categoria WHERE id = ?1", [id], |_| Ok(())).optional()?.is_some();
    if !existe {
        return Err(Erro::campo("categoriaId", "Categoria não encontrada."));
    }
    Ok(Some(id.to_string()))
}

fn fornecedor_valido(conn: &Connection, id: &str) -> Resultado<Option<String>> {
    if id.is_empty() {
        return Ok(None);
    }
    let existe = conn.query_row("SELECT 1 FROM pessoa WHERE id = ?1", [id], |_| Ok(())).optional()?.is_some();
    if !existe {
        return Err(Erro::campo("fornecedorId", "Fornecedor não encontrado."));
    }
    Ok(Some(id.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn gravar(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    fornecedor_id: Option<&str>,
    compra_id: Option<&str>,
    categoria_id: Option<&str>,
    descricao: &str,
    documento: &str,
    observacoes: &str,
    competencia: Option<&str>,
    lista: &[(String, i64)],
) -> Resultado<Vec<String>> {
    let agora = tempo::agora();
    let mut ids = Vec::new();
    for (i, (venc, valor)) in lista.iter().enumerate() {
        let id = tempo::novo_id();
        conn.execute(
            "INSERT INTO conta_pagar (id, fornecedor_id, compra_id, categoria_id, descricao, documento, parcela, parcelas,
                                      valor, vencimento, observacoes, criado_em, criado_por, dispositivo, competencia)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                id,
                fornecedor_id,
                compra_id,
                categoria_id,
                descricao,
                documento,
                i as i64 + 1,
                lista.len() as i64,
                valor,
                venc,
                observacoes,
                agora,
                usuario_id,
                dispositivo,
                competencia
            ],
        )?;
        ids.push(id);
    }
    Ok(ids)
}

/// Lança uma conta a pagar (em uma ou mais parcelas).
pub fn lancar(conn: &Connection, dispositivo: &str, usuario_id: &str, e: &LancamentoEntrada) -> Resultado<Vec<String>> {
    let fornecedor = fornecedor_valido(conn, &e.fornecedor_id)?;
    let categoria = categoria_valida(conn, &e.categoria_id)?;
    let mut descricao = texto::limpar(&e.descricao);
    if descricao.is_empty() {
        if let Some(f) = &fornecedor {
            descricao = conn.query_row("SELECT nome FROM pessoa WHERE id = ?1", [f], |l| l.get(0))?;
        }
    }
    if descricao.is_empty() {
        return Err(Erro::campo("descricao", "Diga do que é a conta (ex.: aluguel de outubro)."));
    }
    if texto::tamanho(&descricao) > 80 {
        return Err(Erro::campo("descricao", "Use no máximo 80 letras."));
    }
    let documento: String = texto::limpar(&e.documento).chars().take(40).collect();
    let valor = ler_valor(&e.valor, "valor", true)?;
    let lista = previa(conn, valor, e.parcelas, &e.primeiro_vencimento)?;
    let observacoes: String = texto::limpar(&e.observacoes).chars().take(300).collect();
    let competencia = ler_competencia(&e.competencia)?;
    let ids = gravar(
        conn,
        dispositivo,
        usuario_id,
        fornecedor.as_deref(),
        None,
        categoria.as_deref(),
        &descricao,
        &documento,
        &observacoes,
        competencia.as_deref(),
        &lista,
    )?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "pagar.lancar",
        "conta_pagar",
        &ids[0],
        &[format!("{descricao}: R$ {} em {} parcela(s)", numeros::escrever_dinheiro(valor), lista.len())],
    )?;
    Ok(ids)
}

/// Duplicatas de uma nota de compra (ou nada, se a nota não traz parcelas).
pub fn duplicatas_da_compra(conn: &Connection, compra_id: &str) -> Resultado<Vec<nfe_xml::Duplicata>> {
    let xml: String = conn
        .query_row("SELECT xml FROM compra WHERE id = ?1", [compra_id], |l| l.get(0))
        .optional()?
        .unwrap_or_default();
    if xml.is_empty() {
        return Ok(vec![]);
    }
    Ok(nfe_xml::ler(&xml).map(|n| n.duplicatas).unwrap_or_default())
}

/// Ao lançar a compra: as duplicatas da nota viram contas a pagar.
/// Devolve quantas parcelas foram lançadas.
pub fn criar_da_compra(conn: &Connection, dispositivo: &str, usuario_id: &str, compra_id: &str) -> Resultado<usize> {
    let (fornecedor_id, numero, gerar): (Option<String>, String, bool) =
        conn.query_row("SELECT fornecedor_id, numero, gerar_pagar FROM compra WHERE id = ?1", [compra_id], |l| {
            Ok((l.get(0)?, l.get(1)?, l.get(2)?))
        })?;
    if !gerar {
        return Ok(0);
    }
    let ja: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conta_pagar WHERE compra_id = ?1 AND cancelado_em IS NULL",
        [compra_id],
        |l| l.get(0),
    )?;
    if ja > 0 {
        return Ok(0);
    }
    let dups = duplicatas_da_compra(conn, compra_id)?;
    if dups.is_empty() {
        return Ok(0);
    }
    let nome: String = match &fornecedor_id {
        Some(f) => {
            conn.query_row("SELECT nome FROM pessoa WHERE id = ?1", [f], |l| l.get(0)).optional()?.unwrap_or_default()
        }
        None => String::new(),
    };
    let descricao: String = format!("NF {numero} · {nome}").chars().take(80).collect();
    let lista: Vec<(String, i64)> = dups.iter().map(|d| (d.vencimento.clone(), d.valor)).collect();
    let ids = gravar(
        conn,
        dispositivo,
        usuario_id,
        fornecedor_id.as_deref(),
        Some(compra_id),
        Some("cat-fornecedores"),
        &descricao,
        &numero,
        "",
        None,
        &lista,
    )?;
    // Os números das duplicatas vão no documento ("12345/001").
    for (id, d) in ids.iter().zip(&dups) {
        if !d.numero.is_empty() {
            conn.execute(
                "UPDATE conta_pagar SET documento = ?2 WHERE id = ?1",
                params![id, format!("{numero}/{}", d.numero)],
            )?;
        }
    }
    Ok(ids.len())
}

/// Ao estornar a compra: as parcelas dela saem (se nenhuma foi paga).
pub fn cancelar_da_compra(conn: &Connection, usuario_id: &str, compra_id: &str) -> Resultado<()> {
    let pagas: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conta_pagar_saldo WHERE compra_id = ?1 AND cancelado_em IS NULL AND pago > 0",
        [compra_id],
        |l| l.get(0),
    )?;
    if pagas > 0 {
        return Err(Erro::validacao(
            "Esta compra tem parcela paga no Financeiro. Estorne o pagamento antes de estornar a compra.",
        ));
    }
    conn.execute(
        "UPDATE conta_pagar SET cancelado_em = ?2, cancelado_por = ?3, motivo_cancelamento = 'Compra estornada'
          WHERE compra_id = ?1 AND cancelado_em IS NULL",
        params![compra_id, tempo::agora(), usuario_id],
    )?;
    Ok(())
}

/// Tira uma parcela (lançada errada, paga por fora...). Sem pagamento.
pub fn cancelar(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str, motivo: &str) -> Resultado<()> {
    crate::sync::exigir_editavel(conn, "conta_pagar", id)?;
    let (pago, cancelada, descricao): (i64, Option<String>, String) = conn
        .query_row("SELECT pago, cancelado_em, descricao FROM conta_pagar_saldo WHERE id = ?1", [id], |l| {
            Ok((l.get(0)?, l.get(1)?, l.get(2)?))
        })
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Conta".into()))?;
    if cancelada.is_some() {
        return Err(Erro::validacao("Esta conta já foi tirada."));
    }
    if pago > 0 {
        return Err(Erro::validacao("Esta conta já tem pagamento. Estorne o pagamento antes."));
    }
    let motivo = texto::limpar(motivo);
    if motivo.is_empty() {
        return Err(Erro::campo("motivo", "Diga o motivo."));
    }
    conn.execute(
        "UPDATE conta_pagar SET cancelado_em = ?2, cancelado_por = ?3, motivo_cancelamento = ?4 WHERE id = ?1",
        params![id, tempo::agora(), usuario_id, motivo],
    )?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "pagar.cancelar",
        "conta_pagar",
        id,
        &[format!("{descricao} · {motivo}")],
    )?;
    Ok(())
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AlteracaoEntrada {
    pub id: String,
    pub descricao: String,
    pub categoria_id: String,
    pub documento: String,
    pub valor: String,
    pub vencimento: String,
    pub observacoes: String,
    /// Mês de competência ("10/2026"). Ausente = não muda; vazio = tira.
    pub competencia: Option<String>,
}

/// Corrige uma parcela (vencimento, valor, descrição, categoria).
pub fn alterar(conn: &Connection, dispositivo: &str, usuario_id: &str, e: &AlteracaoEntrada) -> Resultado<()> {
    crate::sync::exigir_editavel(conn, "conta_pagar", &e.id)?;
    #[allow(clippy::type_complexity)]
    let (pago, cancelada, antes_valor, antes_venc, antes_desc, antes_comp): (
        i64,
        Option<String>,
        i64,
        String,
        String,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT pago, cancelado_em, valor, vencimento, descricao, competencia FROM conta_pagar_saldo WHERE id = ?1",
            [&e.id],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?)),
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Conta".into()))?;
    if cancelada.is_some() {
        return Err(Erro::validacao("Esta conta foi tirada."));
    }
    let descricao = texto::limpar(&e.descricao);
    if descricao.is_empty() {
        return Err(Erro::campo("descricao", "Diga do que é a conta."));
    }
    if texto::tamanho(&descricao) > 80 {
        return Err(Erro::campo("descricao", "Use no máximo 80 letras."));
    }
    let categoria = categoria_valida(conn, &e.categoria_id)?;
    let valor = ler_valor(&e.valor, "valor", true)?;
    if valor < pago {
        return Err(Erro::campo(
            "valor",
            format!("Já foram pagos R$ {}. O valor não pode ser menor.", numeros::escrever_dinheiro(pago)),
        ));
    }
    let vencimento = fiscal::ler_data(&e.vencimento)
        .map_err(|m| Erro::campo("vencimento", m))?
        .ok_or_else(|| Erro::campo("vencimento", "Digite o vencimento."))?;
    let competencia = match &e.competencia {
        Some(t) => ler_competencia(t)?,
        None => antes_comp.clone(),
    };
    conn.execute("UPDATE conta_pagar SET competencia = ?2 WHERE id = ?1", params![e.id, competencia])?;
    conn.execute(
        "UPDATE conta_pagar SET descricao = ?2, categoria_id = ?3, documento = ?4, valor = ?5, vencimento = ?6, observacoes = ?7
          WHERE id = ?1",
        params![
            e.id,
            descricao,
            categoria,
            texto::limpar(&e.documento).chars().take(40).collect::<String>(),
            valor,
            vencimento,
            texto::limpar(&e.observacoes).chars().take(300).collect::<String>()
        ],
    )?;
    let mut detalhes = Vec::new();
    if antes_desc != descricao {
        detalhes.push(format!("Descrição: {antes_desc} → {descricao}"));
    }
    if antes_valor != valor {
        detalhes.push(format!(
            "Valor: {} → {}",
            numeros::escrever_dinheiro(antes_valor),
            numeros::escrever_dinheiro(valor)
        ));
    }
    if antes_venc != vencimento {
        detalhes.push(format!("Vencimento: {} → {}", fiscal::data_br(&antes_venc), fiscal::data_br(&vencimento)));
    }
    if antes_comp != competencia {
        let t = |c: &Option<String>| c.as_deref().map(escrever_competencia).unwrap_or_else(|| "pelo vencimento".into());
        detalhes.push(format!("Competência: {} → {}", t(&antes_comp), t(&competencia)));
    }
    if !detalhes.is_empty() {
        auditoria::registrar(conn, dispositivo, Some(usuario_id), "pagar.alterar", "conta_pagar", &e.id, &detalhes)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Listas.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Parcela {
    pub id: String,
    pub descricao: String,
    pub fornecedor: String,
    pub fornecedor_id: String,
    pub categoria: String,
    pub categoria_id: String,
    pub documento: String,
    pub compra_id: String,
    /// "2/3"
    pub parcela: String,
    pub vencimento: String,
    pub valor: String,
    pub pago: String,
    pub saldo: String,
    pub dias_atraso: i64,
    /// aberta, hoje, vencida, paga, cancelada
    pub situacao: String,
    pub observacoes: String,
    /// Mês de competência ("10/2026"); vazio = pelo vencimento.
    pub competencia: String,
    #[serde(skip)]
    pub v_saldo: i64,
}

const SELECAO: &str = "SELECT c.id, c.descricao, COALESCE(p.nome, ''), COALESCE(c.fornecedor_id, ''), COALESCE(g.nome, ''),
                              COALESCE(c.categoria_id, ''), c.documento, COALESCE(c.compra_id, ''), c.parcela, c.parcelas,
                              c.vencimento, c.valor, c.pago, c.cancelado_em IS NOT NULL, c.observacoes,
                              COALESCE(c.competencia, '')
                         FROM conta_pagar_saldo c
                         LEFT JOIN pessoa p ON p.id = c.fornecedor_id
                         LEFT JOIN categoria g ON g.id = c.categoria_id";

fn ler_linhas(conn: &Connection, sql: &str, valores: &[&dyn rusqlite::ToSql]) -> Resultado<Vec<Parcela>> {
    let hoje = fiscal::hoje(conn)?;
    let mut c = conn.prepare(sql)?;
    let linhas = c.query_map(valores, |l| {
        let vencimento: String = l.get(10)?;
        let valor: i64 = l.get(11)?;
        let pago: i64 = l.get(12)?;
        let cancelada: bool = l.get(13)?;
        Ok((
            l.get::<_, String>(0)?,
            l.get::<_, String>(1)?,
            l.get::<_, String>(2)?,
            l.get::<_, String>(3)?,
            l.get::<_, String>(4)?,
            l.get::<_, String>(5)?,
            l.get::<_, String>(6)?,
            l.get::<_, String>(7)?,
            format!("{}/{}", l.get::<_, i64>(8)?, l.get::<_, i64>(9)?),
            vencimento,
            valor,
            pago,
            cancelada,
            l.get::<_, String>(14)?,
            l.get::<_, String>(15)?,
        ))
    })?;
    let mut lista = Vec::new();
    for linha in linhas {
        let (
            id,
            descricao,
            fornecedor,
            fornecedor_id,
            categoria,
            categoria_id,
            documento,
            compra_id,
            parcela,
            vencimento,
            valor,
            pago,
            cancelada,
            observacoes,
            competencia,
        ) = linha?;
        let saldo = (valor - pago).max(0);
        let situacao = if cancelada {
            "cancelada"
        } else if saldo == 0 {
            "paga"
        } else if vencimento < hoje {
            "vencida"
        } else if vencimento == hoje {
            "hoje"
        } else {
            "aberta"
        };
        lista.push(Parcela {
            id,
            descricao,
            fornecedor,
            fornecedor_id,
            categoria,
            categoria_id,
            documento,
            compra_id,
            parcela,
            dias_atraso: if saldo > 0 && !cancelada { dias_entre(&vencimento, &hoje).max(0) } else { 0 },
            vencimento: fiscal::data_br(&vencimento),
            valor: numeros::escrever_dinheiro(valor),
            pago: numeros::escrever_dinheiro(pago),
            saldo: numeros::escrever_dinheiro(saldo),
            situacao: situacao.into(),
            observacoes,
            competencia: escrever_competencia(&competencia),
            v_saldo: saldo,
        });
    }
    Ok(lista)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Painel {
    pub total: String,
    pub vencido: String,
    pub vence_hoje: String,
    pub vence_7_dias: String,
    pub parcelas: Vec<Parcela>,
}

/// Contas: "abertas" (em aberto, vencidas primeiro), "pagas" (as últimas
/// pagas) ou "todas". O texto procura na descrição, fornecedor, documento
/// ou categoria.
pub fn painel(conn: &Connection, busca: &str, quais: &str) -> Resultado<Painel> {
    let hoje = fiscal::hoje(conn)?;
    let semana = iso(ler_iso(&hoje)? + Duration::days(7));
    let (total, vencido, vence_hoje, vence_7): (i64, i64, i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(valor - pago), 0),
                COALESCE(SUM(CASE WHEN vencimento < ?1 THEN valor - pago END), 0),
                COALESCE(SUM(CASE WHEN vencimento = ?1 THEN valor - pago END), 0),
                COALESCE(SUM(CASE WHEN vencimento > ?1 AND vencimento <= ?2 THEN valor - pago END), 0)
           FROM conta_pagar_saldo WHERE cancelado_em IS NULL AND valor > pago",
        params![hoje, semana],
        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)),
    )?;
    let filtro = format!("%{}%", texto::normalizar(busca));
    let (onde, ordem) = match quais {
        "pagas" => ("c.cancelado_em IS NULL AND c.valor <= c.pago", "c.vencimento DESC"),
        "todas" => ("1 = 1", "c.vencimento DESC"),
        "nenhuma" => ("0 = 1", "c.vencimento"),
        _ => ("c.cancelado_em IS NULL AND c.valor > c.pago", "c.vencimento, c.descricao"),
    };
    let sql = format!(
        "{SELECAO} WHERE {onde}
            AND (?1 = '%%' OR normalizar(c.descricao || ' ' || COALESCE(p.nome, '') || ' ' || c.documento || ' ' || COALESCE(g.nome, '')) LIKE ?1)
          ORDER BY {ordem} LIMIT 500"
    );
    let parcelas = ler_linhas(conn, &sql, &[&filtro])?;
    Ok(Painel {
        total: numeros::escrever_dinheiro(total),
        vencido: numeros::escrever_dinheiro(vencido),
        vence_hoje: numeros::escrever_dinheiro(vence_hoje),
        vence_7_dias: numeros::escrever_dinheiro(vence_7),
        parcelas,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PagamentoLinha {
    pub id: String,
    pub data: String,
    pub forma: String,
    pub origem: String,
    pub valor: String,
    pub juros: String,
    pub desconto: String,
    pub total: String,
    pub cheque_numero: String,
    pub usuario: String,
    pub estornado: bool,
    pub motivo_estorno: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ficha {
    pub parcela: Parcela,
    pub pagamentos: Vec<PagamentoLinha>,
    pub motivo_cancelamento: String,
}

pub fn ficha(conn: &Connection, id: &str) -> Resultado<Ficha> {
    let parcela = ler_linhas(conn, &format!("{SELECAO} WHERE c.id = ?1"), &[&id])?
        .into_iter()
        .next()
        .ok_or_else(|| Erro::NaoEncontrado("Conta".into()))?;
    let motivo_cancelamento: String =
        conn.query_row("SELECT motivo_cancelamento FROM conta_pagar WHERE id = ?1", [id], |l| l.get(0))?;
    let mut c = conn.prepare(
        "SELECT g.id, g.data, g.forma, COALESCE(l.nome, CASE WHEN g.sessao_id IS NOT NULL THEN 'Gaveta do caixa' ELSE '' END),
                g.valor, g.juros, g.desconto, g.cheque_numero, COALESCE(u.nome, ''), g.estornado_em IS NOT NULL, g.motivo_estorno
           FROM pagamento g
           LEFT JOIN conta_loja l ON l.id = g.conta_loja_id
           LEFT JOIN usuario u ON u.id = g.usuario_id
          WHERE g.conta_pagar_id = ?1 ORDER BY g.criado_em DESC",
    )?;
    let pagamentos = c
        .query_map([id], |l| {
            let valor: i64 = l.get(4)?;
            let juros: i64 = l.get(5)?;
            let desconto: i64 = l.get(6)?;
            Ok(PagamentoLinha {
                id: l.get(0)?,
                data: fiscal::data_br(&l.get::<_, String>(1)?),
                forma: nome_forma(&l.get::<_, String>(2)?).to_string(),
                origem: l.get(3)?,
                valor: numeros::escrever_dinheiro(valor),
                juros: numeros::escrever_dinheiro(juros),
                desconto: numeros::escrever_dinheiro(desconto),
                total: numeros::escrever_dinheiro(numeros::somar([valor, juros, -desconto]).unwrap_or(0)),
                cheque_numero: l.get(7)?,
                usuario: l.get(8)?,
                estornado: l.get(9)?,
                motivo_estorno: l.get(10)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(Ficha { parcela, pagamentos, motivo_cancelamento })
}

// ---------------------------------------------------------------------------
// Pagar.
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PagamentoEntrada {
    pub conta_id: String,
    /// Quanto abate da parcela (vazio = tudo o que falta).
    pub valor: String,
    pub juros: String,
    pub desconto: String,
    /// Vazio = hoje. No cheque, o "bom para".
    pub data: String,
    pub forma: String,
    /// Id da conta da loja, ou "gaveta" (o caixa aberto deste computador).
    pub origem: String,
    pub cheque_numero: String,
    pub observacoes: String,
}

/// Paga uma parcela (inteira ou em parte). Devolve o id do pagamento.
pub fn pagar(conn: &Connection, dispositivo: &str, usuario_id: &str, e: &PagamentoEntrada) -> Resultado<String> {
    crate::sync::exigir_editavel(conn, "conta_pagar", &e.conta_id)?;
    let (descricao, valor_conta, pago, cancelada): (String, i64, i64, Option<String>) = conn
        .query_row(
            "SELECT descricao, valor, pago, cancelado_em FROM conta_pagar_saldo WHERE id = ?1",
            [&e.conta_id],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)),
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Conta".into()))?;
    if cancelada.is_some() {
        return Err(Erro::validacao("Esta conta foi tirada."));
    }
    let saldo = valor_conta - pago;
    if saldo <= 0 {
        return Err(Erro::validacao("Esta conta já está paga."));
    }
    if !FORMAS.iter().any(|f| f.0 == e.forma) {
        return Err(Erro::campo("forma", "Escolha como foi pago."));
    }
    let valor = match ler_valor(&e.valor, "valor", false)? {
        0 => saldo,
        v => v,
    };
    if valor > saldo {
        return Err(Erro::campo(
            "valor",
            format!("Falta pagar só R$ {}. Juros vão no campo de juros.", numeros::escrever_dinheiro(saldo)),
        ));
    }
    let juros = ler_valor(&e.juros, "juros", false)?;
    let desconto = ler_valor(&e.desconto, "desconto", false)?;
    if desconto > numeros::soma([valor, juros])? {
        return Err(Erro::campo("desconto", "O desconto é maior que o pagamento."));
    }
    let hoje = fiscal::hoje(conn)?;
    let data = ler_data_ou(conn, &e.data, "data", None)?;
    if e.forma != "cheque" && data > hoje {
        return Err(Erro::campo(
            "data",
            "A data do pagamento não pode ser depois de hoje (só no cheque, o “bom para”).",
        ));
    }
    let cheque_numero: String = texto::limpar(&e.cheque_numero).chars().take(20).collect();
    if e.forma == "cheque" && cheque_numero.is_empty() {
        return Err(Erro::campo("chequeNumero", "Digite o número do cheque."));
    }
    let total = numeros::soma([valor, juros, -desconto])?;
    let id = tempo::novo_id();
    let (conta_loja, sessao, movimento): (Option<String>, Option<String>, Option<String>) = if e.origem == "gaveta" {
        if e.forma != "dinheiro" {
            return Err(Erro::campo("origem", "Da gaveta, só em dinheiro."));
        }
        if data != hoje {
            return Err(Erro::campo("data", "Da gaveta, o pagamento é hoje."));
        }
        let s = caixa::sessao_aberta(conn, dispositivo)?
            .ok_or_else(|| Erro::campo("origem", "O caixa deste computador está fechado."))?;
        let mov = caixa::movimentar_com_conta(
            conn,
            dispositivo,
            usuario_id,
            "sangria",
            &numeros::escrever_dinheiro(total),
            &format!("Pagamento: {descricao}"),
            None,
        )
        .map_err(|e| match e {
            Erro::Validacao { mensagem, .. } => Erro::campo("valor", mensagem),
            outro => outro,
        })?;
        (None, Some(s.id), Some(mov))
    } else {
        let ativa: Option<bool> =
            conn.query_row("SELECT ativo FROM conta_loja WHERE id = ?1", [&e.origem], |l| l.get(0)).optional()?;
        match ativa {
            Some(true) => (Some(e.origem.clone()), None, None),
            Some(false) => return Err(Erro::campo("origem", "Esta conta da loja está desativada.")),
            None => return Err(Erro::campo("origem", "Escolha de onde saiu o dinheiro.")),
        }
    };
    conn.execute(
        "INSERT INTO pagamento (id, conta_pagar_id, data, valor, juros, desconto, forma, conta_loja_id, sessao_id,
                                caixa_movimento_id, cheque_numero, observacoes, usuario_id, criado_em, dispositivo)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            id,
            e.conta_id,
            data,
            valor,
            juros,
            desconto,
            e.forma,
            conta_loja,
            sessao,
            movimento,
            cheque_numero,
            texto::limpar(&e.observacoes).chars().take(300).collect::<String>(),
            usuario_id,
            tempo::agora(),
            dispositivo
        ],
    )?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "pagar.pagar",
        "conta_pagar",
        &e.conta_id,
        &[format!("{descricao}: pagou R$ {} ({})", numeros::escrever_dinheiro(total), nome_forma(&e.forma))],
    )?;
    Ok(id)
}

/// Desfaz um pagamento: a parcela volta a ficar em aberto. Se saiu da
/// gaveta, o dinheiro volta para ela (caixa ainda aberto) ou para o Caixa
/// geral (caixa já fechado).
pub fn estornar(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str, motivo: &str) -> Resultado<()> {
    crate::sync::exigir_editavel(conn, "pagamento", id)?;
    let (conta_id, valor, juros, desconto, sessao, estornado, descricao): (
        String,
        i64,
        i64,
        i64,
        Option<String>,
        Option<String>,
        String,
    ) = conn
        .query_row(
            "SELECT g.conta_pagar_id, g.valor, g.juros, g.desconto, g.sessao_id, g.estornado_em, c.descricao
               FROM pagamento g JOIN conta_pagar c ON c.id = g.conta_pagar_id WHERE g.id = ?1",
            [id],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?, l.get(6)?)),
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Pagamento".into()))?;
    if estornado.is_some() {
        return Err(Erro::validacao("Este pagamento já foi estornado."));
    }
    let motivo = texto::limpar(motivo);
    if motivo.is_empty() {
        return Err(Erro::campo("motivo", "Diga o motivo do estorno."));
    }
    let total = numeros::soma([valor, juros, -desconto])?;
    if let Some(sessao_id) = sessao {
        let aberta = caixa::sessao_aberta(conn, dispositivo)?.filter(|s| s.id == sessao_id);
        if aberta.is_some() {
            caixa::movimentar_com_conta(
                conn,
                dispositivo,
                usuario_id,
                "suprimento",
                &numeros::escrever_dinheiro(total),
                &format!("Estorno do pagamento: {descricao}"),
                None,
            )?;
        } else {
            conn.execute(
                "INSERT INTO movimento_conta (id, conta_id, data, valor, tipo, descricao, origem_id, usuario_id, criado_em, dispositivo)
                 VALUES (?1, 'caixa-geral', ?2, ?3, 'ajuste', ?4, ?5, ?6, ?7, ?8)",
                params![
                    tempo::novo_id(),
                    fiscal::hoje(conn)?,
                    total,
                    format!("Estorno do pagamento (saiu da gaveta): {descricao}"),
                    id,
                    usuario_id,
                    tempo::agora(),
                    dispositivo
                ],
            )?;
        }
    }
    conn.execute(
        "UPDATE pagamento SET estornado_em = ?2, estornado_por = ?3, motivo_estorno = ?4 WHERE id = ?1",
        params![id, tempo::agora(), usuario_id, motivo],
    )?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "pagar.estornar",
        "conta_pagar",
        &conta_id,
        &[format!("{descricao}: estorno de R$ {} · {motivo}", numeros::escrever_dinheiro(total))],
    )?;
    Ok(())
}

/// Contas que vencem hoje e vencidas: (quantas hoje, valor hoje, valor vencido, quantas vencidas).
pub fn do_dia(conn: &Connection) -> Resultado<(i64, i64, i64, i64)> {
    let hoje = fiscal::hoje(conn)?;
    Ok(conn.query_row(
        "SELECT COUNT(CASE WHEN vencimento = ?1 THEN 1 END),
                COALESCE(SUM(CASE WHEN vencimento = ?1 THEN valor - pago END), 0),
                COALESCE(SUM(CASE WHEN vencimento < ?1 THEN valor - pago END), 0),
                COUNT(CASE WHEN vencimento < ?1 THEN 1 END)
           FROM conta_pagar_saldo WHERE cancelado_em IS NULL AND valor > pago",
        [hoje],
        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)),
    )?)
}
