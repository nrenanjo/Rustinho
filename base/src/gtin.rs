//! Código de barras (GTIN/EAN): conferência do dígito verificador.

/// Confere se o código de barras é um GTIN válido (8, 12, 13 ou 14 números
/// com o último dígito verificador correto).
pub fn gtin_valido(codigo: &str) -> bool {
    let digitos: Vec<u32> = codigo.chars().filter_map(|c| c.to_digit(10)).collect();
    if digitos.len() != codigo.len() || ![8, 12, 13, 14].contains(&digitos.len()) {
        return false;
    }
    let (corpo, verificador) = digitos.split_at(digitos.len() - 1);
    // Da direita para a esquerda, pesos 3, 1, 3, 1...
    let soma: u32 = corpo.iter().rev().enumerate().map(|(i, d)| if i % 2 == 0 { d * 3 } else { *d }).sum();
    (10 - soma % 10) % 10 == verificador[0]
}

#[cfg(test)]
mod testes {
    use super::gtin_valido;

    #[test]
    fn confere_digito() {
        assert!(gtin_valido("7891234567895"));
        assert!(gtin_valido("4006381333931"));
        assert!(gtin_valido("96385074")); // GTIN-8
        assert!(gtin_valido("036000291452")); // GTIN-12 (UPC)
        assert!(!gtin_valido("7891234567890"));
        assert!(!gtin_valido("123"));
        assert!(!gtin_valido("789123456789A"));
        assert!(!gtin_valido(""));
    }
}
