//! Data, hora e identificadores.

use time::format_description::well_known::Rfc3339;
use time::{Date, Month, OffsetDateTime};

use crate::erro::{Erro, Resultado};

/// Data e hora atuais em UTC, no formato "2026-10-13T17:30:05Z".
pub fn agora() -> String {
    OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .format(&Rfc3339)
        .unwrap_or_default()
}

/// O momento de `dias` atrás, no mesmo formato de [`agora`] (para comparar
/// com as datas gravadas).
pub fn dias_atras(dias: i64) -> String {
    (OffsetDateTime::now_utc() - time::Duration::days(dias))
        .replace_nanosecond(0)
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .format(&Rfc3339)
        .unwrap_or_default()
}

/// Ano atual (UTC).
pub fn ano_atual() -> i32 {
    OffsetDateTime::now_utc().year()
}

// Datas "AAAA-MM-DD" (vieram do `receber` na etapa 4 da modularização: o
// crediário, os cheques, as contas a pagar, o financeiro, a DRE, o Painel, a
// reposição e as encomendas usam; nenhum deles precisa depender do
// crediário só por elas).

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

/// Divide o valor em `n` parcelas mensais a partir de `inicio`, sempre no
/// `dia` do mês. O centavo que sobra vai na primeira. (vencimento, valor)
pub(crate) fn parcelar(inicio: Date, dia: u8, valor: i64, n: i64) -> Vec<(String, i64)> {
    let n = n.max(1);
    let base = valor / n;
    let resto = valor - base * n;
    (0..n).map(|i| (iso(mais_meses(inicio, i, dia)), base + if i == 0 { resto } else { 0 })).collect()
}

/// Novo identificador único (UUID v7, que já vem em ordem de criação).
pub fn novo_id() -> String {
    #[cfg(test)]
    {
        let atraso = RELOGIO_DE_TESTE.with(|c| c.get());
        if atraso != 0 {
            return id_com_relogio(atraso);
        }
    }
    uuid::Uuid::now_v7().to_string()
}

#[cfg(test)]
thread_local! {
    // Nos testes: o relógio "deste caixa" adiantado (ou atrasado, negativo)
    // em milissegundos, para simular caixas com a hora errada (R-08).
    pub static RELOGIO_DE_TESTE: std::cell::Cell<i64> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn id_com_relogio(atraso_ms: i64) -> String {
    let agora = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    let ms = (agora + atraso_ms).max(0) as u64;
    let ts = uuid::Timestamp::from_unix(uuid::NoContext, ms / 1000, ((ms % 1000) * 1_000_000) as u32);
    uuid::Uuid::new_v7(ts).to_string()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn formato_da_hora() {
        let h = agora();
        assert_eq!(h.len(), 20, "{h}");
        assert!(h.ends_with('Z'));
    }

    #[test]
    fn ids_unicos_e_ordenados() {
        let a = novo_id();
        let b = novo_id();
        assert_ne!(a, b);
        assert!(a < b);
    }
}
