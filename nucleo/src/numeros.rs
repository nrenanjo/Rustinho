//! Números no jeito brasileiro: dinheiro, percentuais e quantidades.
//!
//! Tudo é guardado como número inteiro, para não ter erro de arredondamento:
//! * dinheiro em centavos (R$ 85,50 = 8550);
//! * percentual em centésimos de ponto (46,55% = 4655);
//! * quantidade em milésimos (1,5 = 1500).

/// Casas decimais de cada tipo de número.
const CASAS_DINHEIRO: u32 = 2;
const CASAS_PERCENTUAL: u32 = 2;
const CASAS_QUANTIDADE: u32 = 3;

/// Lê dinheiro digitado: "85", "85,5", "1.234,56", "R$ 85,00" ou "85.50".
/// Devolve `None` se o campo estiver vazio.
pub fn ler_dinheiro(texto: &str) -> Result<Option<i64>, String> {
    ler_decimal(texto, CASAS_DINHEIRO)
}

/// Escreve centavos como "1.234,56".
pub fn escrever_dinheiro(centavos: i64) -> String {
    escrever_decimal(centavos, CASAS_DINHEIRO, CASAS_DINHEIRO)
}

/// Lê percentual digitado: "100", "46,6" ou "46,55%".
pub fn ler_percentual(texto: &str) -> Result<Option<i64>, String> {
    ler_decimal(texto, CASAS_PERCENTUAL)
}

/// Escreve percentual sem zeros sobrando: 10000 → "100", 4660 → "46,6".
pub fn escrever_percentual(centesimos: i64) -> String {
    escrever_decimal(centesimos, CASAS_PERCENTUAL, 0)
}

/// Lê quantidade digitada: "2", "1,5" ou "0,250".
pub fn ler_quantidade(texto: &str) -> Result<Option<i64>, String> {
    ler_decimal(texto, CASAS_QUANTIDADE)
}

/// Escreve quantidade sem zeros sobrando: 2000 → "2", 1500 → "1,5".
pub fn escrever_quantidade(milesimos: i64) -> String {
    escrever_decimal(milesimos, CASAS_QUANTIDADE, 0)
}

/// Preço de venda a partir do custo e do percentual de lucro sobre o custo.
/// Custo R$ 58,00 com 100% → R$ 116,00.
pub fn preco_por_lucro(custo: i64, lucro: i64) -> i64 {
    saturar(dividir_arredondando(custo as i128 * (10_000 + lucro as i128), 10_000))
}

/// Percentual de lucro sobre o custo a partir do preço de venda.
/// Custo R$ 58,00 e venda R$ 85,00 → 46,55%. Sem custo, não há percentual.
pub fn lucro_por_preco(custo: i64, preco: i64) -> Option<i64> {
    if custo <= 0 {
        return None;
    }
    Some(saturar(dividir_arredondando((preco as i128 - custo as i128) * 10_000, custo as i128)))
}

/// Lê um ano de aplicação: "2015" ou "15" (vira 2015) ou "98" (vira 1998).
pub fn ler_ano(texto: &str, ano_atual: i32) -> Result<Option<i32>, String> {
    let t = texto.trim();
    if t.is_empty() {
        return Ok(None);
    }
    if !t.chars().all(|c| c.is_ascii_digit()) {
        return Err("Ano inválido. Use 4 números, como 2015.".into());
    }
    let limite = ano_atual + 2;
    let ano: i32 = match t.len() {
        2 => {
            let n: i32 = t.parse().map_err(|_| "Ano inválido.".to_string())?;
            if 2000 + n <= limite {
                2000 + n
            } else {
                1900 + n
            }
        }
        4 => t.parse().map_err(|_| "Ano inválido.".to_string())?,
        _ => return Err("Ano inválido. Use 4 números, como 2015.".into()),
    };
    if !(1900..=limite).contains(&ano) {
        return Err(format!("Ano fora do intervalo (1900 a {limite})."));
    }
    Ok(Some(ano))
}

/// Divisão inteira arredondando a metade para longe do zero.
/// Resultado de conta com i128 de volta para i64 sem "dar a volta" (um
/// número enorme não vira negativo: fica no máximo).
pub fn saturar(v: i128) -> i64 {
    v.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

fn dividir_arredondando(numerador: i128, divisor: i128) -> i128 {
    let metade = divisor / 2;
    if (numerador >= 0) == (divisor >= 0) {
        (numerador + metade) / divisor
    } else {
        (numerador - metade) / divisor
    }
}

/// Lê um número decimal no formato brasileiro e devolve como inteiro com
/// `casas` casas decimais. Aceita vírgula ou ponto como separador decimal
/// ("85,50" ou "85.50") e ponto como separador de milhar ("1.234,56").
fn ler_decimal(texto: &str, casas: u32) -> Result<Option<i64>, String> {
    let limpo: String = texto
        .replace("R$", "")
        .replace('%', "")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if limpo.is_empty() {
        return Ok(None);
    }
    let (negativo, numero) = match limpo.strip_prefix('-') {
        Some(resto) => (true, resto.to_string()),
        None => (false, limpo),
    };
    if numero.is_empty() || !numero.chars().all(|c| c.is_ascii_digit() || c == ',' || c == '.') {
        return Err("Digite só números, como 85,50.".into());
    }

    let (inteira, fracao) = if numero.contains(',') {
        let partes: Vec<&str> = numero.split(',').collect();
        if partes.len() > 2 {
            return Err("Use só uma vírgula para os centavos.".into());
        }
        if partes[1].contains('.') {
            return Err("Número inválido.".into());
        }
        // Pontos de milhar só de 3 em 3: "1.234,56" sim, "1.2.3,45" não.
        if partes[0].contains('.') {
            let grupos: Vec<&str> = partes[0].split('.').collect();
            if grupos[0].is_empty() || grupos[0].len() > 3 || grupos[1..].iter().any(|g| g.len() != 3) {
                return Err("Número inválido.".into());
            }
        }
        (partes[0].replace('.', ""), partes[1].to_string())
    } else if numero.contains('.') {
        let partes: Vec<&str> = numero.split('.').collect();
        if partes.len() == 2 && partes[1].len() != 3 {
            // Um único ponto que não separa milhar: é o separador decimal.
            (partes[0].to_string(), partes[1].to_string())
        } else {
            if partes[0].is_empty() || partes[0].len() > 3 || partes[1..].iter().any(|p| p.len() != 3) {
                return Err("Número inválido.".into());
            }
            (partes.concat(), String::new())
        }
    } else {
        (numero, String::new())
    };

    let inteira = if inteira.is_empty() { "0".to_string() } else { inteira };
    if inteira.len() > 13 {
        return Err("Número grande demais.".into());
    }
    let parte_inteira: i64 = inteira.parse().map_err(|_| "Número inválido.".to_string())?;

    // Parte decimal com exatamente `casas` dígitos, arredondando o excesso.
    let mut digitos: Vec<u32> = fracao.chars().filter_map(|c| c.to_digit(10)).collect();
    let arredondar_para_cima = digitos.len() > casas as usize && digitos[casas as usize] >= 5;
    digitos.resize(casas as usize, 0);
    let mut parte_decimal: i64 = digitos.iter().fold(0, |acc, d| acc * 10 + *d as i64);
    if arredondar_para_cima {
        parte_decimal += 1;
    }

    let valor = parte_inteira * 10_i64.pow(casas) + parte_decimal;
    Ok(Some(if negativo { -valor } else { valor }))
}

/// Escreve um inteiro com `casas` casas decimais no formato brasileiro,
/// mostrando pelo menos `minimo` casas (as demais só se não forem zero).
#[allow(clippy::manual_is_multiple_of)]
fn escrever_decimal(valor: i64, casas: u32, minimo: u32) -> String {
    let negativo = valor < 0;
    let absoluto = valor.unsigned_abs();
    let fator = 10_u64.pow(casas);
    let inteira = absoluto / fator;
    let mut fracao = format!("{:0width$}", absoluto % fator, width = casas as usize);
    while fracao.len() > minimo as usize && fracao.ends_with('0') {
        fracao.pop();
    }

    // Separador de milhar.
    let digitos = inteira.to_string();
    let mut com_milhar = String::new();
    for (i, c) in digitos.chars().enumerate() {
        if i > 0 && (digitos.len() - i) % 3 == 0 {
            com_milhar.push('.');
        }
        com_milhar.push(c);
    }

    let mut texto = String::new();
    if negativo && absoluto != 0 {
        texto.push('-');
    }
    texto.push_str(&com_milhar);
    if !fracao.is_empty() {
        texto.push(',');
        texto.push_str(&fracao);
    }
    texto
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn milhar_de_tres_em_tres() {
        assert_eq!(ler_dinheiro("1.234,56").unwrap(), Some(123_456));
        assert_eq!(ler_dinheiro("1.234.567,00").unwrap(), Some(123_456_700));
        assert!(ler_dinheiro("1.2.3,45").is_err());
        assert!(ler_dinheiro("12.34,50").is_err());
        assert!(ler_dinheiro("1234.567,00").is_err());
        assert!(ler_dinheiro(".123,00").is_err());
        assert!(ler_dinheiro("1234.567.890").is_err());
        assert_eq!(saturar(i128::MAX), i64::MAX);
        assert_eq!(saturar(-5), -5);
    }

    #[test]
    fn le_dinheiro() {
        assert_eq!(ler_dinheiro(""), Ok(None));
        assert_eq!(ler_dinheiro("  "), Ok(None));
        assert_eq!(ler_dinheiro("85"), Ok(Some(8500)));
        assert_eq!(ler_dinheiro("85,5"), Ok(Some(8550)));
        assert_eq!(ler_dinheiro("85,50"), Ok(Some(8550)));
        assert_eq!(ler_dinheiro("85,"), Ok(Some(8500)));
        assert_eq!(ler_dinheiro(",99"), Ok(Some(99)));
        assert_eq!(ler_dinheiro("R$ 1.234,56"), Ok(Some(123456)));
        assert_eq!(ler_dinheiro("1.234"), Ok(Some(123400)));
        assert_eq!(ler_dinheiro("1.234.567"), Ok(Some(123456700)));
        assert_eq!(ler_dinheiro("85.50"), Ok(Some(8550)));
        assert_eq!(ler_dinheiro("85.5"), Ok(Some(8550)));
        assert_eq!(ler_dinheiro("0,005"), Ok(Some(1)));
        assert_eq!(ler_dinheiro("0,004"), Ok(Some(0)));
        assert_eq!(ler_dinheiro("-10"), Ok(Some(-1000)));
        assert!(ler_dinheiro("abc").is_err());
        assert!(ler_dinheiro("1,2,3").is_err());
        assert!(ler_dinheiro("1.23.4").is_err());
    }

    #[test]
    fn escreve_dinheiro() {
        assert_eq!(escrever_dinheiro(0), "0,00");
        assert_eq!(escrever_dinheiro(8550), "85,50");
        assert_eq!(escrever_dinheiro(123456), "1.234,56");
        assert_eq!(escrever_dinheiro(100000000), "1.000.000,00");
        assert_eq!(escrever_dinheiro(-1999), "-19,99");
    }

    #[test]
    fn percentuais() {
        assert_eq!(ler_percentual("100"), Ok(Some(10000)));
        assert_eq!(ler_percentual("46,6%"), Ok(Some(4660)));
        assert_eq!(ler_percentual("46,555"), Ok(Some(4656)));
        assert_eq!(escrever_percentual(10000), "100");
        assert_eq!(escrever_percentual(4660), "46,6");
        assert_eq!(escrever_percentual(4655), "46,55");
        assert_eq!(escrever_percentual(-2500), "-25");
    }

    #[test]
    fn quantidades() {
        assert_eq!(ler_quantidade("2"), Ok(Some(2000)));
        assert_eq!(ler_quantidade("1,5"), Ok(Some(1500)));
        assert_eq!(ler_quantidade("0,250"), Ok(Some(250)));
        assert_eq!(escrever_quantidade(2000), "2");
        assert_eq!(escrever_quantidade(1500), "1,5");
        assert_eq!(escrever_quantidade(1234500), "1.234,5");
        assert_eq!(escrever_quantidade(0), "0");
    }

    #[test]
    fn preco_e_lucro() {
        // custo 58,00 com 100% de lucro = 116,00
        assert_eq!(preco_por_lucro(5800, 10000), 11600);
        // custo 58,00 e venda 85,00 = 46,55%
        assert_eq!(lucro_por_preco(5800, 8500), Some(4655));
        assert_eq!(lucro_por_preco(0, 8500), None);
        // ida e volta
        assert_eq!(preco_por_lucro(5800, 4655), 8500);
        // arredondamento: 33,33 com 50% = 49,995 → 50,00
        assert_eq!(preco_por_lucro(3333, 5000), 5000);
        // venda abaixo do custo
        assert_eq!(lucro_por_preco(1000, 800), Some(-2000));
    }

    #[test]
    fn anos() {
        assert_eq!(ler_ano("2015", 2026), Ok(Some(2015)));
        assert_eq!(ler_ano("15", 2026), Ok(Some(2015)));
        assert_eq!(ler_ano("98", 2026), Ok(Some(1998)));
        assert_eq!(ler_ano("28", 2026), Ok(Some(2028)));
        assert_eq!(ler_ano("", 2026), Ok(None));
        assert!(ler_ano("2030", 2026).is_err());
        assert!(ler_ano("1800", 2026).is_err());
        assert!(ler_ano("20a5", 2026).is_err());
        assert!(ler_ano("201", 2026).is_err());
    }
}
