//! Crediário, convênio e contas a receber.
//!
//! Uma venda no crediário vira parcelas (`conta_receber`). Quando o cliente
//! paga, o caixa lança um recebimento, que abate uma ou mais parcelas (da
//! mais antiga para a mais nova). O que já foi pago nunca é gravado na
//! parcela: sai da soma dos recebimentos não estornados. Assim dois caixas
//! podem receber ao mesmo tempo sem um apagar o do outro.
//!
//! Atraso: multa (uma vez) e juros por mês, configuráveis; sugeridos na tela
//! e sempre editáveis por quem recebe.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use time::{Date, Duration, Month};

use crate::erro::{Erro, Resultado};
use crate::{auditoria, caixa, config, fiscal, numeros, tempo, texto};

/// Máximo de parcelas numa venda ou lançamento.
pub const MAX_PARCELAS: i64 = 24;

// ---------------------------------------------------------------------------
// Datas.
// ---------------------------------------------------------------------------

pub(crate) fn ler_iso(iso: &str) -> Resultado<Date> {
    let p: Vec<&str> = iso.get(..10).unwrap_or(iso).split('-').collect();
    let erro = || Erro::Interno(format!("data inválida: {iso}"));
    if p.len() != 3 {
        return Err(erro());
    }
    let ano: i32 = p[0].parse().map_err(|_| erro())?;
    let mes: u8 = p[1].parse().map_err(|_| erro())?;
    let dia: u8 = p[2].parse().map_err(|_| erro())?;
    Date::from_calendar_date(ano, Month::try_from(mes).map_err(|_| erro())?, dia).map_err(|_| erro())
}

pub(crate) fn iso(d: Date) -> String {
    format!("{:04}-{:02}-{:02}", d.year(), d.month() as u8, d.day())
}

/// Soma meses mantendo o dia (31/01 + 1 mês = 28 ou 29/02).
pub(crate) fn mais_meses(d: Date, meses: i64, dia: u8) -> Date {
    let total = d.year() as i64 * 12 + (d.month() as i64 - 1) + meses;
    let ano = total.div_euclid(12) as i32;
    let mes = Month::try_from((total.rem_euclid(12) + 1) as u8).unwrap_or(Month::January);
    let dia = dia.clamp(1, time::util::days_in_month(mes, ano));
    Date::from_calendar_date(ano, mes, dia).unwrap_or(d)
}

/// Dias entre duas datas "AAAA-MM-DD" (positivo se `ate` for depois).
pub(crate) fn dias_entre(de: &str, ate: &str) -> i64 {
    match (ler_iso(de), ler_iso(ate)) {
        (Ok(a), Ok(b)) => (b - a).whole_days(),
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Parcelas de uma venda.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParcelaPrevia {
    pub numero: i64,
    pub vencimento: String,
    pub valor: String,
    #[serde(skip)]
    pub v_vencimento: String,
    #[serde(skip)]
    pub v_valor: i64,
}

/// Divide o valor em parcelas mensais. O 1º vencimento: o digitado; senão o
/// dia do convênio do cliente; senão daqui a 30 dias.
pub fn previa(
    conn: &Connection,
    cliente_id: &str,
    valor: i64,
    parcelas: i64,
    primeiro: &str,
) -> Resultado<Vec<ParcelaPrevia>> {
    let n = if parcelas <= 0 { 1 } else { parcelas };
    if n > MAX_PARCELAS {
        return Err(Erro::campo("parcelas", format!("Use no máximo {MAX_PARCELAS} parcelas.")));
    }
    if valor <= 0 {
        return Ok(vec![]);
    }
    let hoje = ler_iso(&fiscal::hoje(conn)?)?;
    let dia_convenio: Option<i64> = if cliente_id.is_empty() {
        None
    } else {
        conn.query_row("SELECT dia_vencimento FROM pessoa WHERE id = ?1", [cliente_id], |l| l.get(0))
            .optional()?
            .flatten()
    };
    let digitado = fiscal::ler_data(primeiro).map_err(|m| Erro::campo("primeiroVencimento", m))?;
    let (inicio, dia) = match (digitado, dia_convenio) {
        (Some(d), _) => {
            let d = ler_iso(&d)?;
            if d < hoje {
                return Err(Erro::campo("primeiroVencimento", "O primeiro vencimento já passou."));
            }
            (d, d.day())
        }
        (None, Some(dia)) => {
            let dia = dia.clamp(1, 31) as u8;
            let este_mes = mais_meses(hoje, 0, dia);
            (if este_mes > hoje { este_mes } else { mais_meses(hoje, 1, dia) }, dia)
        }
        (None, None) => {
            let d = hoje + Duration::days(30);
            (d, d.day())
        }
    };
    Ok(parcelar(inicio, dia, valor, n)
        .into_iter()
        .enumerate()
        .map(|(i, (venc, v))| {
            let i = i as i64;
            ParcelaPrevia {
                numero: i + 1,
                vencimento: fiscal::data_br(&venc),
                valor: numeros::escrever_dinheiro(v),
                v_vencimento: venc,
                v_valor: v,
            }
        })
        .collect())
}

/// Divide o valor em `n` parcelas mensais a partir de `inicio`, sempre no
/// `dia` do mês. O centavo que sobra vai na primeira. (vencimento, valor)
pub(crate) fn parcelar(inicio: Date, dia: u8, valor: i64, n: i64) -> Vec<(String, i64)> {
    let n = n.max(1);
    let base = valor / n;
    let resto = valor - base * n;
    (0..n).map(|i| (iso(mais_meses(inicio, i, dia)), base + if i == 0 { resto } else { 0 })).collect()
}

fn gravar_parcelas(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    cliente_id: &str,
    venda_id: Option<&str>,
    descricao: &str,
    lista: &[ParcelaPrevia],
) -> Resultado<Vec<String>> {
    let agora = tempo::agora();
    let mut ids = Vec::new();
    for p in lista {
        let id = tempo::novo_id();
        conn.execute(
            "INSERT INTO conta_receber (id, cliente_id, venda_id, parcela, parcelas, descricao, valor, vencimento,
                                        criado_em, criado_por, dispositivo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                id,
                cliente_id,
                venda_id,
                p.numero,
                lista.len() as i64,
                descricao,
                p.v_valor,
                p.v_vencimento,
                agora,
                usuario_id,
                dispositivo
            ],
        )?;
        ids.push(id);
    }
    Ok(ids)
}

/// Parcelas de uma venda no crediário (chamado ao concluir a venda).
#[allow(clippy::too_many_arguments)]
pub fn criar_da_venda(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    venda_id: &str,
    descricao: &str,
    cliente_id: &str,
    valor: i64,
    parcelas: i64,
    primeiro: &str,
) -> Resultado<()> {
    let lista = previa(conn, cliente_id, valor, parcelas, primeiro)?;
    gravar_parcelas(conn, dispositivo, usuario_id, cliente_id, Some(venda_id), descricao, &lista)?;
    Ok(())
}

/// Cancela as parcelas de uma venda que está sendo cancelada. Não deixa se
/// alguma já foi paga (estorne o recebimento antes).
pub fn cancelar_da_venda(conn: &Connection, usuario_id: &str, venda_id: &str, motivo: &str) -> Resultado<()> {
    let pago: i64 = conn.query_row(
        "SELECT COALESCE(SUM(pago), 0) FROM conta_receber_saldo WHERE venda_id = ?1 AND cancelado_em IS NULL",
        [venda_id],
        |l| l.get(0),
    )?;
    if pago > 0 {
        return Err(Erro::validacao(
            "Esta venda tem parcelas do crediário já pagas. Estorne os recebimentos em Receber antes de cancelar.",
        ));
    }
    conn.execute(
        "UPDATE conta_receber SET cancelado_em = ?2, cancelado_por = ?3, motivo_cancelamento = ?4
          WHERE venda_id = ?1 AND cancelado_em IS NULL",
        params![venda_id, tempo::agora(), usuario_id, motivo],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Situação do cliente (limite, em aberto, vencido).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Situacao {
    pub limite: String,
    pub em_aberto: String,
    pub vencido: String,
    /// Quanto ainda pode comprar no crediário (vazio = sem limite).
    pub disponivel: String,
    pub vencido_desde: String,
    pub dia_vencimento: String,
    #[serde(skip)]
    pub v_limite: Option<i64>,
    #[serde(skip)]
    pub v_em_aberto: i64,
    #[serde(skip)]
    pub v_vencido: i64,
}

pub fn situacao(conn: &Connection, cliente_id: &str) -> Resultado<Situacao> {
    let hoje = fiscal::hoje(conn)?;
    let (limite, dia): (Option<i64>, Option<i64>) = conn
        .query_row("SELECT limite_credito, dia_vencimento FROM pessoa WHERE id = ?1", [cliente_id], |l| {
            Ok((l.get(0)?, l.get(1)?))
        })
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Cliente".into()))?;
    let (aberto, vencido, desde): (i64, i64, Option<String>) = conn.query_row(
        "SELECT COALESCE(SUM(valor - pago), 0),
                COALESCE(SUM(CASE WHEN vencimento < ?2 THEN valor - pago ELSE 0 END), 0),
                MIN(CASE WHEN vencimento < ?2 THEN vencimento END)
           FROM conta_receber_saldo WHERE cliente_id = ?1 AND cancelado_em IS NULL AND valor > pago",
        params![cliente_id, hoje],
        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
    )?;
    Ok(Situacao {
        limite: limite.map(numeros::escrever_dinheiro).unwrap_or_default(),
        em_aberto: numeros::escrever_dinheiro(aberto),
        vencido: numeros::escrever_dinheiro(vencido),
        disponivel: limite.map(|l| numeros::escrever_dinheiro((l - aberto).max(0))).unwrap_or_default(),
        vencido_desde: desde.map(|d| fiscal::data_br(&d)).unwrap_or_default(),
        dia_vencimento: dia.map(|d| d.to_string()).unwrap_or_default(),
        v_limite: limite,
        v_em_aberto: aberto,
        v_vencido: vencido,
    })
}

/// Avisos ao vender no crediário (não impedem a venda).
pub fn avisos_venda(conn: &Connection, cliente_id: &str, valor: i64) -> Resultado<Vec<String>> {
    let s = situacao(conn, cliente_id)?;
    let mut avisos = Vec::new();
    if s.v_vencido > 0 {
        avisos.push(format!("Tem R$ {} vencido desde {}.", s.vencido, s.vencido_desde));
    }
    if let Some(limite) = s.v_limite {
        if numeros::soma([s.v_em_aberto, valor])? > limite {
            avisos.push(format!("Passa do limite de crédito (limite R$ {}, já deve R$ {}).", s.limite, s.em_aberto));
        }
    }
    Ok(avisos)
}

// ---------------------------------------------------------------------------
// Juros por atraso.
// ---------------------------------------------------------------------------

/// Multa e juros por atraso.
#[derive(Debug, Clone, Copy, Default)]
pub struct Taxas {
    /// Multa (uma vez), em centésimos de percentual (200 = 2%).
    pub multa: i64,
    /// Juros ao mês, proporcionais aos dias, em centésimos de percentual.
    pub juros_mes: i64,
    /// Dias de atraso sem multa nem juros (tolerância).
    pub carencia: i64,
}

fn taxas(conn: &Connection) -> Resultado<Taxas> {
    let ler =
        |chave: &str| -> Resultado<i64> { Ok(config::ler(conn, chave)?.and_then(|v| v.parse().ok()).unwrap_or(0)) };
    Ok(Taxas {
        multa: ler("crediario_multa")?,
        juros_mes: ler("crediario_juros")?,
        carencia: ler("crediario_carencia")?,
    })
}

/// Multa e juros de um saldo com `dias` de atraso. Dentro da tolerância não
/// cobra nada; passou dela, conta desde o vencimento.
fn juros_de(saldo: i64, dias: i64, t: Taxas) -> i64 {
    if dias <= 0 || saldo <= 0 || dias <= t.carencia {
        return 0;
    }
    let m = saldo as i128 * t.multa as i128;
    let j = saldo as i128 * t.juros_mes as i128 * dias as i128 / 30;
    crate::numeros::saturar((m + j + 5_000) / 10_000)
}

/// Taxas para a tela de configurações ("2,00" = 2%; tolerância em dias).
pub fn taxas_texto(conn: &Connection) -> Resultado<(String, String, String)> {
    let t = taxas(conn)?;
    let p = |v: i64| if v == 0 { String::new() } else { numeros::escrever_percentual(v) };
    Ok((p(t.multa), p(t.juros_mes), if t.carencia == 0 { String::new() } else { t.carencia.to_string() }))
}

pub fn salvar_taxas(conn: &Connection, multa: &str, juros: &str, carencia: &str) -> Resultado<()> {
    let ler = |texto: &str, campo: &str, max: i64| -> Resultado<i64> {
        let v = numeros::ler_percentual(texto).map_err(|m| Erro::campo(campo, m))?.unwrap_or(0);
        if !(0..=max).contains(&v) {
            return Err(Erro::campo(campo, format!("Use de 0 a {}%.", max / 100)));
        }
        Ok(v)
    };
    let m = ler(multa, "crediarioMulta", 2_000)?;
    let j = ler(juros, "crediarioJuros", 2_000)?;
    let c: i64 = if carencia.trim().is_empty() {
        0
    } else {
        carencia
            .trim()
            .parse()
            .ok()
            .filter(|v| (0..=60).contains(v))
            .ok_or_else(|| Erro::campo("crediarioCarencia", "Use de 0 a 60 dias."))?
    };
    config::gravar(conn, "crediario_multa", &m.to_string())?;
    config::gravar(conn, "crediario_juros", &j.to_string())?;
    config::gravar(conn, "crediario_carencia", &c.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Lançamento à mão (dívida antiga, caderninho) e cancelamento.
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LancamentoEntrada {
    pub cliente_id: String,
    pub descricao: String,
    pub valor: String,
    pub parcelas: i64,
    pub primeiro_vencimento: String,
}

pub fn lancar(conn: &Connection, dispositivo: &str, usuario_id: &str, e: &LancamentoEntrada) -> Resultado<Vec<String>> {
    situacao(conn, &e.cliente_id)?;
    let descricao = texto::limpar(&e.descricao);
    if descricao.is_empty() {
        return Err(Erro::campo("descricao", "Diga do que é a dívida (ex.: caderno de setembro)."));
    }
    if texto::tamanho(&descricao) > 80 {
        return Err(Erro::campo("descricao", "Use no máximo 80 letras."));
    }
    let valor = numeros::ler_dinheiro(&e.valor)
        .map_err(|m| Erro::campo("valor", m))?
        .filter(|v| *v > 0)
        .ok_or_else(|| Erro::campo("valor", "Digite o valor."))?;
    let lista = previa(conn, &e.cliente_id, valor, e.parcelas, &e.primeiro_vencimento)?;
    let ids = gravar_parcelas(conn, dispositivo, usuario_id, &e.cliente_id, None, &descricao, &lista)?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "receber.lancar",
        "pessoa",
        &e.cliente_id,
        &[format!("{descricao}: R$ {} em {} parcela(s)", numeros::escrever_dinheiro(valor), lista.len())],
    )?;
    Ok(ids)
}

/// Cancela uma parcela lançada à mão (as de venda saem cancelando a venda).
pub fn cancelar_conta(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str, motivo: &str) -> Resultado<()> {
    crate::sync::exigir_editavel(conn, "conta_receber", id)?;
    let (cliente_id, venda_id, pago, cancelada, descricao): (String, Option<String>, i64, Option<String>, String) =
        conn.query_row(
            "SELECT cliente_id, venda_id, pago, cancelado_em, descricao FROM conta_receber_saldo WHERE id = ?1",
            [id],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?)),
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Parcela".into()))?;
    if cancelada.is_some() {
        return Err(Erro::validacao("Esta parcela já foi cancelada."));
    }
    if venda_id.is_some() {
        return Err(Erro::validacao("Parcela de venda: para tirar, cancele a venda."));
    }
    if pago > 0 {
        return Err(Erro::validacao("Esta parcela já tem pagamento. Estorne o recebimento antes."));
    }
    let motivo = texto::limpar(motivo);
    if motivo.is_empty() {
        return Err(Erro::campo("motivo", "Diga o motivo."));
    }
    conn.execute(
        "UPDATE conta_receber SET cancelado_em = ?2, cancelado_por = ?3, motivo_cancelamento = ?4 WHERE id = ?1",
        params![id, tempo::agora(), usuario_id, motivo],
    )?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "receber.cancelar",
        "pessoa",
        &cliente_id,
        &[format!("{descricao} · {motivo}")],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Listas.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Devedor {
    pub cliente_id: String,
    pub nome: String,
    pub telefone: String,
    pub em_aberto: String,
    pub vencido: String,
    pub parcelas: i64,
    pub proximo_vencimento: String,
    pub dias_atraso: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Painel {
    pub total: String,
    pub vencido: String,
    pub vence_hoje: String,
    pub vence_7_dias: String,
    pub clientes: Vec<Devedor>,
}

/// Quem deve, com quem está atrasado primeiro.
pub fn painel(conn: &Connection, busca: &str) -> Resultado<Painel> {
    let hoje = fiscal::hoje(conn)?;
    let semana = iso(ler_iso(&hoje)? + Duration::days(7));
    let (total, vencido, vence_hoje, vence_7): (i64, i64, i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(valor - pago), 0),
                COALESCE(SUM(CASE WHEN vencimento < ?1 THEN valor - pago END), 0),
                COALESCE(SUM(CASE WHEN vencimento = ?1 THEN valor - pago END), 0),
                COALESCE(SUM(CASE WHEN vencimento > ?1 AND vencimento <= ?2 THEN valor - pago END), 0)
           FROM conta_receber_saldo WHERE cancelado_em IS NULL AND valor > pago",
        params![hoje, semana],
        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)),
    )?;
    let filtro = texto::normalizar(busca);
    let mut c = conn.prepare(
        "SELECT p.id, p.nome, CASE WHEN p.celular <> '' THEN p.celular ELSE p.telefone END,
                SUM(c.valor - c.pago), COALESCE(SUM(CASE WHEN c.vencimento < ?1 THEN c.valor - c.pago END), 0),
                COUNT(*), MIN(c.vencimento)
           FROM conta_receber_saldo c JOIN pessoa p ON p.id = c.cliente_id
          WHERE c.cancelado_em IS NULL AND c.valor > c.pago AND (?2 = '' OR p.busca LIKE '%' || ?2 || '%')
          GROUP BY p.id
          ORDER BY MIN(c.vencimento), p.nome
          LIMIT 500",
    )?;
    let clientes = c
        .query_map(params![hoje, filtro], |l| {
            let primeiro: String = l.get(6)?;
            Ok(Devedor {
                cliente_id: l.get(0)?,
                nome: l.get(1)?,
                telefone: l.get(2)?,
                em_aberto: numeros::escrever_dinheiro(l.get(3)?),
                vencido: numeros::escrever_dinheiro(l.get(4)?),
                parcelas: l.get(5)?,
                proximo_vencimento: fiscal::data_br(&primeiro),
                dias_atraso: dias_entre(&primeiro, &hoje).max(0),
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(Painel {
        total: numeros::escrever_dinheiro(total),
        vencido: numeros::escrever_dinheiro(vencido),
        vence_hoje: numeros::escrever_dinheiro(vence_hoje),
        vence_7_dias: numeros::escrever_dinheiro(vence_7),
        clientes,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Parcela {
    pub id: String,
    pub venda_id: String,
    pub descricao: String,
    /// "2/3"
    pub parcela: String,
    pub vencimento: String,
    pub valor: String,
    pub pago: String,
    pub saldo: String,
    pub juros: String,
    pub dias_atraso: i64,
    /// aberta, hoje, vencida, paga, cancelada
    pub situacao: String,
    pub manual: bool,
    #[serde(skip)]
    pub v_saldo: i64,
    #[serde(skip)]
    pub v_juros: i64,
    #[serde(skip)]
    pub v_vencimento: String,
    #[serde(skip)]
    pub v_parcela: i64,
}

fn ler_parcelas(conn: &Connection, filtro: &str, valor: &str) -> Resultado<Vec<Parcela>> {
    let hoje = fiscal::hoje(conn)?;
    let taxas = taxas(conn)?;
    let sql = format!(
        "SELECT c.id, COALESCE(c.venda_id, ''), c.descricao, c.parcela, c.parcelas, c.vencimento, c.valor, c.pago,
                c.cancelado_em IS NOT NULL
           FROM conta_receber_saldo c WHERE {filtro}
          ORDER BY c.vencimento, c.criado_em, c.parcela"
    );
    let mut c = conn.prepare(&sql)?;
    let linhas = c.query_map([valor], |l| {
        Ok((
            l.get::<_, String>(0)?,
            l.get::<_, String>(1)?,
            l.get::<_, String>(2)?,
            l.get::<_, i64>(3)?,
            l.get::<_, i64>(4)?,
            l.get::<_, String>(5)?,
            l.get::<_, i64>(6)?,
            l.get::<_, i64>(7)?,
            l.get::<_, bool>(8)?,
        ))
    })?;
    let mut lista = Vec::new();
    for linha in linhas {
        let (id, venda_id, descricao, parcela, parcelas, vencimento, valor, pago, cancelada) = linha?;
        let saldo = (valor - pago).max(0);
        let atraso = dias_entre(&vencimento, &hoje).max(0);
        let juros = if saldo > 0 && !cancelada { juros_de(saldo, atraso, taxas) } else { 0 };
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
            manual: venda_id.is_empty(),
            venda_id,
            descricao,
            parcela: format!("{parcela}/{parcelas}"),
            vencimento: fiscal::data_br(&vencimento),
            valor: numeros::escrever_dinheiro(valor),
            pago: numeros::escrever_dinheiro(pago),
            saldo: numeros::escrever_dinheiro(saldo),
            juros: if juros > 0 { numeros::escrever_dinheiro(juros) } else { String::new() },
            dias_atraso: if saldo > 0 { atraso } else { 0 },
            situacao: situacao.into(),
            v_saldo: saldo,
            v_juros: juros,
            v_vencimento: vencimento,
            v_parcela: parcela,
        });
    }
    Ok(lista)
}

/// Parcelas de uma venda (para a ficha e o comprovante).
pub fn parcelas_da_venda(conn: &Connection, venda_id: &str) -> Resultado<Vec<Parcela>> {
    ler_parcelas(conn, "c.venda_id = ?1", venda_id)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecebimentoLinha {
    pub id: String,
    pub numero: i64,
    pub caixa: String,
    pub quando: String,
    pub forma: String,
    pub valor: String,
    pub juros: String,
    pub desconto: String,
    pub usuario: String,
    pub estornado: bool,
    pub motivo_estorno: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClienteConta {
    pub cliente_id: String,
    pub nome: String,
    pub documento: String,
    pub telefone: String,
    pub situacao: Situacao,
    pub parcelas: Vec<Parcela>,
    pub recebimentos: Vec<RecebimentoLinha>,
}

/// A conta de um cliente: parcelas em aberto (ou todas) e recebimentos.
pub fn cliente(conn: &Connection, cliente_id: &str, todas: bool) -> Resultado<ClienteConta> {
    let (nome, documento, telefone): (String, String, String) = conn
        .query_row(
            "SELECT nome, documento, CASE WHEN celular <> '' THEN celular ELSE telefone END FROM pessoa WHERE id = ?1",
            [cliente_id],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Cliente".into()))?;
    let filtro =
        if todas { "c.cliente_id = ?1" } else { "c.cliente_id = ?1 AND c.cancelado_em IS NULL AND c.valor > c.pago" };
    let mut parcelas = ler_parcelas(conn, filtro, cliente_id)?;
    // Nas "todas": em aberto primeiro, depois as pagas e canceladas (mais novas antes).
    if todas {
        parcelas.sort_by(|a, b| {
            let fechada = |p: &Parcela| p.situacao == "paga" || p.situacao == "cancelada";
            fechada(a).cmp(&fechada(b)).then_with(|| {
                if fechada(a) {
                    b.v_vencimento.cmp(&a.v_vencimento)
                } else {
                    a.v_vencimento.cmp(&b.v_vencimento)
                }
            })
        });
    }
    let mut c = conn.prepare(
        "SELECT r.id, r.numero, r.caixa_nome, r.criado_em, r.forma, r.valor, r.juros, r.desconto, COALESCE(u.nome, ''),
                r.estornado_em IS NOT NULL, r.motivo_estorno
           FROM recebimento r LEFT JOIN usuario u ON u.id = r.usuario_id
          WHERE r.cliente_id = ?1 ORDER BY r.criado_em DESC LIMIT 100",
    )?;
    let recebimentos = c
        .query_map([cliente_id], |l| {
            Ok(RecebimentoLinha {
                id: l.get(0)?,
                numero: l.get(1)?,
                caixa: l.get(2)?,
                quando: l.get(3)?,
                forma: caixa::nome_forma(&l.get::<_, String>(4)?).to_string(),
                valor: numeros::escrever_dinheiro(l.get(5)?),
                juros: numeros::escrever_dinheiro(l.get(6)?),
                desconto: numeros::escrever_dinheiro(l.get(7)?),
                usuario: l.get(8)?,
                estornado: l.get(9)?,
                motivo_estorno: l.get(10)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut recebimentos = recebimentos;
    for r in &mut recebimentos {
        r.quando = conn
            .query_row("SELECT strftime('%d/%m/%Y %H:%M', ?1, 'localtime')", [&r.quando], |l| {
                l.get::<_, Option<String>>(0)
            })?
            .unwrap_or_default();
    }
    Ok(ClienteConta {
        cliente_id: cliente_id.to_string(),
        nome,
        documento: crate::documento::escrever(&documento),
        telefone,
        situacao: situacao(conn, cliente_id)?,
        parcelas,
        recebimentos,
    })
}

// ---------------------------------------------------------------------------
// Receber.
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ReceberEntrada {
    pub cliente_id: String,
    /// Parcelas escolhidas (vazio = todas em aberto).
    pub contas: Vec<String>,
    /// O que o cliente entregou (vazio = tudo).
    pub valor: String,
    /// Juros (vazio = o sugerido).
    pub juros: String,
    pub desconto: String,
    pub forma: String,
    pub observacoes: String,
    /// Forma "cheque": os dados dos cheques.
    pub cheques: crate::cheques::ChequesEntrada,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Abate {
    pub conta_id: String,
    pub descricao: String,
    pub parcela: String,
    pub vencimento: String,
    pub valor: String,
    /// Quanto fica faltando nesta parcela depois do pagamento.
    pub resta: String,
    #[serde(skip)]
    pub v_valor: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceberCalculo {
    pub saldo: String,
    pub juros_sugerido: String,
    pub juros: String,
    pub desconto: String,
    /// saldo + juros − desconto
    pub total: String,
    pub recebido: String,
    pub troco: String,
    /// Quanto continua devendo das parcelas escolhidas.
    pub continua_devendo: String,
    pub abates: Vec<Abate>,
    pub cheques: crate::vendas::ChequesCalculo,
    pub erro: String,
    pub campo: String,
    #[serde(skip)]
    pub v_recebido: i64,
    #[serde(skip)]
    pub v_juros: i64,
    #[serde(skip)]
    pub v_desconto: i64,
    #[serde(skip)]
    pub v_troco: i64,
}

pub fn calcular(conn: &Connection, e: &ReceberEntrada) -> Resultado<ReceberCalculo> {
    let abertas =
        ler_parcelas(conn, "c.cliente_id = ?1 AND c.cancelado_em IS NULL AND c.valor > c.pago", &e.cliente_id)?;
    let escolhidas: Vec<&Parcela> = if e.contas.is_empty() {
        abertas.iter().collect()
    } else {
        abertas.iter().filter(|p| e.contas.contains(&p.id)).collect()
    };
    let saldo = numeros::somar(escolhidas.iter().map(|p| p.v_saldo)).ok_or_else(numeros::grande_demais)?;
    let sugerido = numeros::somar(escolhidas.iter().map(|p| p.v_juros)).ok_or_else(numeros::grande_demais)?;
    let mut falha: Option<(&str, String)> = None;
    fn falhou<'a>(f: &mut Option<(&'a str, String)>, c: &'a str, m: String) {
        if f.is_none() {
            *f = Some((c, m));
        }
    }
    let juros = match numeros::ler_dinheiro(&e.juros) {
        Ok(Some(v)) if v >= 0 => v,
        Ok(Some(_)) => {
            falhou(&mut falha, "juros", "Os juros não podem ser negativos.".into());
            0
        }
        Ok(None) => sugerido,
        Err(m) => {
            falhou(&mut falha, "juros", m);
            0
        }
    };
    let desconto = match numeros::ler_dinheiro(&e.desconto) {
        Ok(Some(v)) if v >= 0 => v,
        Ok(Some(_)) => {
            falhou(&mut falha, "desconto", "O desconto não pode ser negativo.".into());
            0
        }
        Ok(None) => 0,
        Err(m) => {
            falhou(&mut falha, "desconto", m);
            0
        }
    };
    if desconto > numeros::soma([saldo, juros])? {
        falhou(&mut falha, "desconto", "O desconto é maior que a dívida.".into());
    }
    let total = numeros::soma([saldo, juros, -desconto])?.max(0);
    let recebido = match numeros::ler_dinheiro(&e.valor) {
        Ok(Some(v)) if v > 0 => v,
        Ok(Some(_)) => {
            falhou(&mut falha, "valor", "Digite quanto o cliente pagou.".into());
            0
        }
        Ok(None) => total,
        Err(m) => {
            falhou(&mut falha, "valor", m);
            0
        }
    };
    // Paga primeiro os juros; o resto (mais o desconto) abate as parcelas.
    if recebido > 0 && recebido < juros && falha.is_none() {
        falhou(
            &mut falha,
            "valor",
            format!(
                "O valor não cobre os juros (R$ {}). Diminua os juros ou receba mais.",
                numeros::escrever_dinheiro(juros)
            ),
        );
    }
    let principal = numeros::soma([recebido, -juros, desconto])?.clamp(0, saldo);
    let troco = (recebido - total).max(0);
    let mut resta = principal;
    let mut abates = Vec::new();
    for p in &escolhidas {
        if resta <= 0 {
            break;
        }
        let v = resta.min(p.v_saldo);
        resta -= v;
        abates.push(Abate {
            conta_id: p.id.clone(),
            descricao: p.descricao.clone(),
            parcela: p.parcela.clone(),
            vencimento: p.vencimento.clone(),
            valor: numeros::escrever_dinheiro(v),
            resta: numeros::escrever_dinheiro(p.v_saldo - v),
            v_valor: v,
        });
    }
    if saldo == 0 && falha.is_none() {
        falhou(&mut falha, "contas", "Escolha pelo menos uma parcela em aberto.".into());
    }
    let cheques =
        crate::vendas::calcular_cheques(conn, if e.forma == "cheque" { recebido.min(total) } else { 0 }, &e.cheques)?;
    let (campo, erro) = falha.map(|(c, m)| (c.to_string(), m)).unwrap_or_default();
    Ok(ReceberCalculo {
        cheques,
        saldo: numeros::escrever_dinheiro(saldo),
        juros_sugerido: numeros::escrever_dinheiro(sugerido),
        juros: numeros::escrever_dinheiro(juros),
        desconto: numeros::escrever_dinheiro(desconto),
        total: numeros::escrever_dinheiro(total),
        recebido: numeros::escrever_dinheiro(recebido),
        troco: numeros::escrever_dinheiro(troco),
        continua_devendo: numeros::escrever_dinheiro(saldo - principal),
        abates,
        erro,
        campo,
        v_recebido: recebido,
        v_juros: juros,
        v_desconto: desconto,
        v_troco: troco,
    })
}

/// Lança o recebimento no caixa aberto deste computador.
pub fn receber(conn: &Connection, dispositivo: &str, usuario_id: &str, e: &ReceberEntrada) -> Resultado<String> {
    let sessao =
        caixa::sessao_aberta(conn, dispositivo)?.ok_or_else(|| Erro::validacao("Abra o caixa antes de receber."))?;
    match caixa::FORMAS.iter().find(|f| f.0 == e.forma) {
        None => return Err(Erro::campo("forma", "Escolha como o cliente pagou.")),
        Some((f, ..)) if *f == "crediario" => return Err(Erro::campo("forma", "Crediário não paga crediário.")),
        Some((f, ..)) if *f == "vale" => return Err(Erro::campo("forma", "Vale-troca é só para compras.")),
        Some((_, nome, false)) => return Err(Erro::campo("forma", format!("{nome} ainda não está disponível."))),
        _ => {}
    }
    let c = calcular(conn, e)?;
    if !c.erro.is_empty() {
        return Err(Erro::campo(&c.campo, c.erro));
    }
    // R-05: só as parcelas que seriam baixadas contam; as outras seguem.
    for a in &c.abates {
        crate::sync::exigir_editavel(conn, "conta_receber", &a.conta_id)?;
    }
    if c.v_troco > 0 && e.forma != "dinheiro" {
        return Err(Erro::campo("valor", "Pagamento a mais só em dinheiro (para dar troco)."));
    }
    if !c.cheques.erro.is_empty() {
        return Err(Erro::campo(&c.cheques.campo, c.cheques.erro.clone()));
    }
    let id = tempo::novo_id();
    let numero: i64 = conn.query_row(
        "SELECT COALESCE(MAX(numero), 0) + 1 FROM recebimento WHERE dispositivo = ?1",
        [dispositivo],
        |l| l.get(0),
    )?;
    conn.execute(
        "INSERT INTO recebimento (id, cliente_id, numero, dispositivo, caixa_nome, sessao_id, forma, valor, juros, desconto,
                                  troco, observacoes, usuario_id, criado_em)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            id,
            e.cliente_id,
            numero,
            dispositivo,
            sessao.caixa_nome,
            sessao.id,
            e.forma,
            c.v_recebido - c.v_troco,
            c.v_juros,
            c.v_desconto,
            c.v_troco,
            texto::limpar(&e.observacoes).chars().take(300).collect::<String>(),
            usuario_id,
            tempo::agora()
        ],
    )?;
    for a in &c.abates {
        conn.execute(
            "INSERT INTO recebimento_parcela (id, recebimento_id, conta_id, valor) VALUES (?1, ?2, ?3, ?4)",
            params![tempo::novo_id(), id, a.conta_id, a.v_valor],
        )?;
    }
    if e.forma == "cheque" {
        crate::cheques::criar(
            conn,
            dispositivo,
            usuario_id,
            Some(&e.cliente_id),
            None,
            Some(&id),
            c.v_recebido,
            &e.cheques,
        )?;
    }
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "receber.receber",
        "pessoa",
        &e.cliente_id,
        &[format!(
            "Recebimento {numero} · {} · R$ {}",
            sessao.caixa_nome,
            numeros::escrever_dinheiro(c.v_recebido - c.v_troco)
        )],
    )?;
    Ok(id)
}

/// Desfaz um recebimento: as parcelas voltam a ficar em aberto.
pub fn estornar(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str, motivo: &str) -> Resultado<()> {
    crate::sync::exigir_editavel(conn, "recebimento", id)?;
    crate::sync::exigir_filhos_editaveis(conn, "recebimento_parcela", "recebimento_id", id)?;
    #[allow(clippy::type_complexity)]
    let (cliente_id, numero, caixa_nome, estornado, sessao_id, forma, valor): (
        String,
        i64,
        String,
        Option<String>,
        Option<String>,
        String,
        i64,
    ) = conn
        .query_row(
            "SELECT cliente_id, numero, caixa_nome, estornado_em, sessao_id, forma, valor FROM recebimento WHERE id = ?1",
            [id],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?, l.get(6)?)),
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Recebimento".into()))?;
    if estornado.is_some() {
        return Err(Erro::validacao("Este recebimento já foi estornado."));
    }
    let motivo = texto::limpar(motivo);
    if motivo.is_empty() {
        return Err(Erro::campo("motivo", "Diga o motivo do estorno."));
    }
    crate::cheques::cancelar_de(conn, usuario_id, "recebimento_id", id)?;
    // D-14: recebimento de um caixa já fechado. O fechamento dele não muda; o
    // dinheiro sai da gaveta aberta (ou do Caixa geral).
    let fechado = caixa::sessao_fechada(conn, sessao_id.as_deref())?;
    conn.execute(
        "UPDATE recebimento SET estornado_em = ?2, estornado_por = ?3, motivo_estorno = ?4, estornado_apos_fechar = ?5
          WHERE id = ?1",
        params![id, tempo::agora(), usuario_id, motivo, fechado],
    )?;
    let mut detalhes = vec![format!("Estorno do recebimento {numero} · {caixa_nome} · {motivo}")];
    if fechado {
        let onde = caixa::compensar_caixa_fechado(
            conn,
            dispositivo,
            usuario_id,
            if forma == "dinheiro" { -valor.max(0) } else { 0 },
            "recebimento_estornado",
            id,
            &format!("Estorno do recebimento {numero} · {caixa_nome} (caixa já fechado) · {motivo}"),
        )?;
        detalhes.push(match onde {
            Some(o) => format!("O caixa do recebimento já estava fechado (o fechamento não muda): {o}"),
            None => "O caixa do recebimento já estava fechado (o fechamento não muda); não era dinheiro".into(),
        });
    }
    auditoria::registrar(conn, dispositivo, Some(usuario_id), "receber.estornar", "pessoa", &cliente_id, &detalhes)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Recibo.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recibo {
    pub id: String,
    pub numero: i64,
    pub caixa: String,
    pub criado_em: String,
    pub cliente: String,
    pub cliente_documento: String,
    pub forma: String,
    pub valor: String,
    pub juros: String,
    pub desconto: String,
    pub troco: String,
    pub usuario: String,
    pub estornado: bool,
    pub partes: Vec<Abate>,
    /// Quanto o cliente continua devendo (tudo, não só estas parcelas).
    pub em_aberto: String,
}

pub fn recibo(conn: &Connection, id: &str) -> Resultado<Recibo> {
    let r = conn
        .query_row(
            "SELECT r.numero, r.caixa_nome, r.criado_em, p.nome, p.documento, r.forma, r.valor, r.juros, r.desconto, r.troco,
                    COALESCE(u.nome, ''), r.estornado_em IS NOT NULL, r.cliente_id
               FROM recebimento r JOIN pessoa p ON p.id = r.cliente_id LEFT JOIN usuario u ON u.id = r.usuario_id
              WHERE r.id = ?1",
            [id],
            |l| {
                Ok(Recibo {
                    id: id.to_string(),
                    numero: l.get(0)?,
                    caixa: l.get(1)?,
                    criado_em: l.get(2)?,
                    cliente: l.get(3)?,
                    cliente_documento: crate::documento::escrever(&l.get::<_, String>(4)?),
                    forma: caixa::nome_forma(&l.get::<_, String>(5)?).to_string(),
                    valor: numeros::escrever_dinheiro(l.get(6)?),
                    juros: numeros::escrever_dinheiro(l.get(7)?),
                    desconto: numeros::escrever_dinheiro(l.get(8)?),
                    troco: numeros::escrever_dinheiro(l.get(9)?),
                    usuario: l.get(10)?,
                    estornado: l.get(11)?,
                    partes: vec![],
                    em_aberto: l.get(12)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Recebimento".into()))?;
    let mut r = r;
    let cliente_id = std::mem::take(&mut r.em_aberto);
    let mut c = conn.prepare(
        "SELECT rp.conta_id, c.descricao, c.parcela, c.parcelas, c.vencimento, rp.valor, c.valor - c.pago
           FROM recebimento_parcela rp JOIN conta_receber_saldo c ON c.id = rp.conta_id
          WHERE rp.recebimento_id = ?1 ORDER BY c.vencimento, c.parcela",
    )?;
    r.partes = c
        .query_map([id], |l| {
            let v: i64 = l.get(5)?;
            Ok(Abate {
                conta_id: l.get(0)?,
                descricao: l.get(1)?,
                parcela: format!("{}/{}", l.get::<_, i64>(2)?, l.get::<_, i64>(3)?),
                vencimento: fiscal::data_br(&l.get::<_, String>(4)?),
                valor: numeros::escrever_dinheiro(v),
                resta: numeros::escrever_dinheiro(l.get::<_, i64>(6)?.max(0)),
                v_valor: v,
            })
        })?
        .collect::<Result<_, _>>()?;
    r.em_aberto = situacao(conn, &cliente_id)?.em_aberto;
    Ok(r)
}

/// Recebimentos de uma sessão de caixa, por forma: (forma, soma).
pub fn por_forma_na_sessao(conn: &Connection, sessao_id: &str) -> Resultado<(i64, Vec<(String, i64)>)> {
    let qtd: i64 = conn.query_row(
        "SELECT COUNT(*) FROM recebimento WHERE sessao_id = ?1 AND (estornado_em IS NULL OR estornado_apos_fechar = 1)",
        [sessao_id],
        |l| l.get(0),
    )?;
    let mut c = conn.prepare_cached(
        "SELECT forma, SUM(valor) FROM recebimento
          WHERE sessao_id = ?1 AND (estornado_em IS NULL OR estornado_apos_fechar = 1) GROUP BY forma",
    )?;
    let somas = c.query_map([sessao_id], |l| Ok((l.get(0)?, l.get(1)?)))?.collect::<Result<_, _>>()?;
    Ok((qtd, somas))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::vendas::{self, ItemEntrada, PagamentoEntrada, VendaEntrada};
    use crate::{pessoas, produtos, usuarios};

    #[test]
    fn crediario_do_comeco_ao_fim() {
        let b = Banco::em_memoria().unwrap();
        let c = &b.conn;
        let d = &b.dispositivo;
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        let mut f = produtos::ficha_nova(c).unwrap();
        f.nome = "Bateria 60Ah".into();
        f.custo = "150".into();
        f.estoque_inicial = "3".into();
        let p = produtos::salvar(c, d, &u, &f).unwrap(); // preço 300,00
        let mut cli = pessoas::ficha_nova(c, "cliente").unwrap();
        cli.nome = "Oficina do Zé".into();
        cli.limite_credito = "400".into();
        let zé = pessoas::salvar(c, d, &u, &cli).unwrap();
        caixa::abrir(c, d, &u, "Caixa 1", "50").unwrap();

        // 300 = 100 entrada no Pix + 200 no crediário em 2 vezes
        let mut e = VendaEntrada {
            itens: vec![ItemEntrada { produto_id: p.clone(), quantidade: "1".into(), ..Default::default() }],
            tabela: 1,
            pagamentos: vec![
                PagamentoEntrada { forma: "pix".into(), valor: "100".into(), ..Default::default() },
                PagamentoEntrada { forma: "crediario".into(), valor: "200".into(), ..Default::default() },
            ],
            parcelas: 2,
            ..Default::default()
        };
        // sem cliente, não
        let calc = vendas::calcular(c, &e).unwrap();
        assert_eq!(calc.crediario.campo, "cliente");
        e.cliente_id = zé.clone();
        let calc = vendas::calcular(c, &e).unwrap();
        assert!(calc.crediario.erro.is_empty());
        assert_eq!(calc.crediario.parcelas.len(), 2);
        assert!(calc.crediario.avisos.is_empty());
        e.primeiro_vencimento = "01/01/2020".into();
        assert_eq!(vendas::concluir(c, d, &u, &e).unwrap_err().nome_do_campo(), Some("primeiroVencimento"));
        e.primeiro_vencimento = String::new();
        let v = vendas::concluir(c, d, &u, &e).unwrap();
        let ficha = vendas::ficha(c, &v).unwrap();
        assert_eq!(ficha.parcelas.len(), 2);
        assert_eq!(ficha.parcelas[0].valor, "100,00");
        assert_eq!(situacao(c, &zé).unwrap().em_aberto, "200,00");

        // outra compra passa do limite: avisa, não impede
        e.pagamentos = vec![PagamentoEntrada { forma: "crediario".into(), valor: "300".into(), ..Default::default() }];
        e.parcelas = 3;
        let calc = vendas::calcular(c, &e).unwrap();
        assert_eq!(calc.crediario.avisos.len(), 1, "{:?}", calc.crediario.avisos);
        assert_eq!(calc.crediario.parcelas[0].valor, "100,00");

        // paga 150 em dinheiro: quita a 1ª e abate 50 da 2ª
        let r = ReceberEntrada {
            cliente_id: zé.clone(),
            valor: "150".into(),
            forma: "dinheiro".into(),
            ..Default::default()
        };
        let calc = calcular(c, &r).unwrap();
        assert_eq!((calc.abates.len(), calc.continua_devendo.as_str()), (2, "50,00"));
        let rec = receber(c, d, &u, &r).unwrap();
        assert_eq!(situacao(c, &zé).unwrap().em_aberto, "50,00");
        let conta = cliente(c, &zé, false).unwrap();
        assert_eq!(conta.parcelas.len(), 1);
        assert_eq!(conta.parcelas[0].saldo, "50,00");

        // cancelar a venda com parcela paga não deixa
        assert!(vendas::cancelar(c, d, &u, &v, "Desistiu").is_err());

        // caixa: 50 inicial + 150 recebido em dinheiro
        let s = caixa::sessao_aberta(c, d).unwrap().unwrap();
        let res = caixa::resumo(c, &s.id).unwrap();
        assert_eq!((res.esperado.as_str(), res.recebimentos), ("200,00", 1));

        // pago a mais em Pix não (troco só em dinheiro)
        let r2 =
            ReceberEntrada { cliente_id: zé.clone(), valor: "60".into(), forma: "pix".into(), ..Default::default() };
        assert!(receber(c, d, &u, &r2).is_err());
        // estorno: volta a dever 200
        assert!(estornar(c, d, &u, &rec, "").is_err());
        estornar(c, d, &u, &rec, "Lançado no cliente errado").unwrap();
        assert_eq!(situacao(c, &zé).unwrap().em_aberto, "200,00");
        assert_eq!(caixa::resumo(c, &s.id).unwrap().esperado, "50,00");
        // agora a venda pode ser cancelada e as parcelas somem
        vendas::cancelar(c, d, &u, &v, "Desistiu").unwrap();
        assert_eq!(situacao(c, &zé).unwrap().em_aberto, "0,00");

        // dívida antiga lançada à mão, vencida, com juros
        salvar_taxas(c, "2", "1", "").unwrap();
        let ids = lancar(
            c,
            d,
            &u,
            &LancamentoEntrada {
                cliente_id: zé.clone(),
                descricao: "Caderno de setembro".into(),
                valor: "100".into(),
                parcelas: 1,
                primeiro_vencimento: String::new(),
            },
        )
        .unwrap();
        c.execute(
            "UPDATE conta_receber SET vencimento = date('now', 'localtime', '-15 days') WHERE id = ?1",
            [&ids[0]],
        )
        .unwrap();
        let conta = cliente(c, &zé, false).unwrap();
        assert_eq!((conta.parcelas[0].situacao.as_str(), conta.parcelas[0].juros.as_str()), ("vencida", "2,50"));
        let p = painel(c, "").unwrap();
        assert_eq!((p.vencido.as_str(), p.clientes.len()), ("100,00", 1));
        assert_eq!(painel(c, "ze").unwrap().clientes.len(), 1);
        // recebe tudo com os juros sugeridos e 2,50 de desconto
        let r3 = ReceberEntrada {
            cliente_id: zé.clone(),
            desconto: "2,50".into(),
            forma: "debito".into(),
            ..Default::default()
        };
        let calc = calcular(c, &r3).unwrap();
        assert_eq!((calc.total.as_str(), calc.continua_devendo.as_str()), ("100,00", "0,00"));
        let rec3 = receber(c, d, &u, &r3).unwrap();
        assert!(crate::impressao::recibo(c, &rec3).unwrap().contains("quitada"));
        assert!(painel(c, "").unwrap().clientes.is_empty());
        // convênio: vence sempre no dia 10
        c.execute("UPDATE pessoa SET dia_vencimento = 10 WHERE id = ?1", [&zé]).unwrap();
        let pv = previa(c, &zé, 9_000, 3, "").unwrap();
        assert!(pv.iter().all(|x| x.vencimento.starts_with("10/")), "{pv:?}");
        assert_eq!(pv.iter().map(|x| x.v_valor).sum::<i64>(), 9_000);
        // parcela paga não se cancela
        assert!(cancelar_conta(c, d, &u, &ids[0], "erro").is_err());
    }

    #[test]
    fn meses_e_juros() {
        let d = Date::from_calendar_date(2026, Month::January, 31).unwrap();
        assert_eq!(iso(mais_meses(d, 1, 31)), "2026-02-28");
        assert_eq!(iso(mais_meses(d, 2, 31)), "2026-03-31");
        assert_eq!(iso(mais_meses(d, 12, 31)), "2027-01-31");
        assert_eq!(dias_entre("2026-10-01", "2026-10-14"), 13);
        // R$ 100 com 2% de multa e 1% ao mês, 15 dias de atraso: 2,00 + 0,50
        let t = Taxas { multa: 200, juros_mes: 100, carencia: 0 };
        assert_eq!(juros_de(10_000, 15, t), 250);
        assert_eq!(juros_de(10_000, 0, t), 0);
        // Com 3 dias de tolerância: até 3 dias não cobra; no 4º conta tudo.
        let t = Taxas { carencia: 3, ..t };
        assert_eq!(juros_de(10_000, 3, t), 0);
        assert_eq!(juros_de(10_000, 4, t), 213);
    }
}
