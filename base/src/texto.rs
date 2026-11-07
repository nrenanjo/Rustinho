//! Tratamento de texto para busca: sem acentos, minúsculas, códigos compactos.

use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// Prepara um texto para busca: minúsculas, sem acentos, só letras, números
/// e os sinais `.`, `-` e `/` dentro das palavras.
///
/// "Pastilha de Freio Diant. (VW)" → "pastilha de freio diant vw"
/// "Motor 1.6 8V" → "motor 1.6 8v"
pub fn normalizar(texto: &str) -> String {
    let sem_acento: String = texto
        .nfkd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .map(|c| if c.is_alphanumeric() || matches!(c, '.' | '-' | '/') { c } else { ' ' })
        .collect();
    sem_acento
        .split_whitespace()
        .map(|palavra| palavra.trim_matches(|c| matches!(c, '.' | '-' | '/')))
        .filter(|palavra| !palavra.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Deixa só letras e números, em minúsculas e sem acentos.
/// Serve para comparar códigos: "KP-1017A" e "kp 1017a" viram "kp1017a".
pub fn compactar(texto: &str) -> String {
    texto
        .nfkd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Tira espaços das pontas e espaços repetidos no meio.
pub fn limpar(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Quantidade de caracteres (não de bytes).
pub fn tamanho(texto: &str) -> usize {
    texto.chars().count()
}

/// Nomes alternativos de montadoras, para a busca achar "vw" e "volkswagen".
pub fn sinonimos_montadora(montadora_normalizada: &str) -> &'static [&'static str] {
    match montadora_normalizada {
        "vw" | "volks" | "volkswagen" => &["vw", "volkswagen"],
        "gm" | "chevrolet" | "chevy" => &["gm", "chevrolet"],
        "mb" | "mercedes" | "mercedes-benz" | "mercedes benz" => &["mb", "mercedes", "benz"],
        "citroen" => &["citroen"],
        "land rover" | "land-rover" => &["land rover", "landrover"],
        _ => &[],
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn normaliza_acentos_e_pontuacao() {
        assert_eq!(normalizar("Pastilha de Freio Diant. (VW)"), "pastilha de freio diant vw");
        assert_eq!(normalizar("  Coração  ÇÃO ção "), "coracao cao cao");
        assert_eq!(normalizar("Motor 1.6 8V"), "motor 1.6 8v");
        assert_eq!(normalizar("KP-1017A"), "kp-1017a");
        assert_eq!(normalizar(" - . / "), "");
        assert_eq!(normalizar("1ª linha"), "1a linha");
    }

    #[test]
    fn compacta_codigos() {
        assert_eq!(compactar("KP-1017A"), "kp1017a");
        assert_eq!(compactar("8482.20.10"), "84822010");
        assert_eq!(compactar(" 32206 a "), "32206a");
    }

    #[test]
    fn limpa_espacos() {
        assert_eq!(limpar("  Gol   G5 "), "Gol G5");
    }
}
