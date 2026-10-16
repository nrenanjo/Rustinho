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
    uuid::Uuid::now_v7().to_string()
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
