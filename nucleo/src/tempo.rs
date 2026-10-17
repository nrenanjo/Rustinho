//! Data, hora e identificadores.

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

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
