//! CPF, CNPJ, CEP e placa: conferência e escrita.
//!
//! O CNPJ pode ter letras nas 12 primeiras posições (CNPJ alfanumérico, que a
//! Receita começou a dar em julho de 2026). Os 2 dígitos finais continuam
//! números. A conta é a mesma de sempre, usando o código ASCII de cada
//! caractere menos 48 (assim '0'..'9' valem 0..9 e 'A'..'Z' valem 17..42).

/// Deixa só números e letras maiúsculas.
pub fn limpar(texto: &str) -> String {
    texto.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect()
}

/// Só os números.
pub fn digitos(texto: &str) -> String {
    texto.chars().filter(|c| c.is_ascii_digit()).collect()
}

fn todos_iguais(s: &str) -> bool {
    let mut c = s.chars();
    match c.next() {
        Some(primeiro) => c.all(|x| x == primeiro),
        None => true,
    }
}

/// CPF válido? (11 números, com os dígitos verificadores certos)
pub fn cpf_valido(cpf: &str) -> bool {
    let d: Vec<u32> = cpf.chars().filter_map(|c| c.to_digit(10)).collect();
    if d.len() != 11 || cpf.chars().count() != 11 || todos_iguais(cpf) {
        return false;
    }
    let dv = |n: usize| {
        let soma: u32 = (0..n).map(|i| d[i] * (n as u32 + 1 - i as u32)).sum();
        let resto = (soma * 10) % 11;
        if resto == 10 {
            0
        } else {
            resto
        }
    };
    dv(9) == d[9] && dv(10) == d[10]
}

/// Formato do CNPJ (sem conferir os dígitos): 14 caracteres, os 12
/// primeiros números ou letras maiúsculas, os 2 últimos números.
pub fn cnpj_formato(cnpj: &str) -> bool {
    let c: Vec<char> = cnpj.chars().collect();
    c.len() == 14
        && c[..12].iter().all(|x| x.is_ascii_digit() || x.is_ascii_uppercase())
        && c[12..].iter().all(|x| x.is_ascii_digit())
}

/// Valor de um caractere na conta do dígito (código ASCII − 48): '0'..'9' =
/// 0..9, 'A'..'Z' = 17..42. Só aceita número ou letra maiúscula.
pub fn valor_no_digito(c: char) -> Option<u32> {
    (c.is_ascii_digit() || c.is_ascii_uppercase()).then(|| c as u32 - 48)
}

/// CNPJ válido? (14 caracteres; os 12 primeiros podem ter letras)
pub fn cnpj_valido(cnpj: &str) -> bool {
    let c: Vec<char> = cnpj.chars().collect();
    if !cnpj_formato(cnpj) || todos_iguais(cnpj) {
        return false;
    }
    let valor = |x: char| x as u32 - 48;
    let dv = |n: usize| {
        // pesos 2..9 da direita para a esquerda
        let soma: u32 = (0..n)
            .map(|i| {
                let peso = ((n - 1 - i) % 8) as u32 + 2;
                valor(c[i]) * peso
            })
            .sum();
        let resto = soma % 11;
        if resto < 2 {
            0
        } else {
            11 - resto
        }
    };
    dv(12) == valor(c[12]) && dv(13) == valor(c[13])
}

/// "12345678909" → "123.456.789-09"; "11222333000181" → "11.222.333/0001-81".
/// Se não tiver o tamanho certo, devolve como veio.
pub fn escrever(documento: &str) -> String {
    let c: Vec<char> = documento.chars().collect();
    let s = |a: usize, b: usize| c[a..b].iter().collect::<String>();
    match c.len() {
        11 => format!("{}.{}.{}-{}", s(0, 3), s(3, 6), s(6, 9), s(9, 11)),
        14 => format!("{}.{}.{}/{}-{}", s(0, 2), s(2, 5), s(5, 8), s(8, 12), s(12, 14)),
        _ => documento.to_string(),
    }
}

/// "18320095" → "18320-095".
pub fn escrever_cep(cep: &str) -> String {
    if cep.len() == 8 {
        format!("{}-{}", &cep[..5], &cep[5..])
    } else {
        cep.to_string()
    }
}

/// Placa no padrão antigo (ABC1234) ou Mercosul (ABC1D23)? Devolve a placa
/// limpa em maiúsculas, ou erro.
pub fn ler_placa(texto: &str) -> Result<String, String> {
    let p = limpar(texto);
    if p.is_empty() {
        return Ok(p);
    }
    let c: Vec<char> = p.chars().collect();
    let ok = c.len() == 7
        && c[..3].iter().all(|x| x.is_ascii_alphabetic())
        && c[3].is_ascii_digit()
        && c[4].is_ascii_alphanumeric()
        && c[5..].iter().all(|x| x.is_ascii_digit());
    if ok {
        Ok(p)
    } else {
        Err("Placa inválida. Use 3 letras e 4 números (ex.: ABC1234 ou ABC1D23).".into())
    }
}

/// "ABC1234" → "ABC-1234"; placa Mercosul fica sem traço.
pub fn escrever_placa(placa: &str) -> String {
    let c: Vec<char> = placa.chars().collect();
    if c.len() == 7 && c[4].is_ascii_digit() {
        format!("{}-{}", &placa[..3], &placa[3..])
    } else {
        placa.to_string()
    }
}

/// "15912345678" → "(15) 91234-5678"; "1532320000" → "(15) 3232-0000".
/// Outros formatos ficam como vieram.
pub fn escrever_telefone(texto: &str) -> String {
    let d = digitos(texto);
    match d.len() {
        10 => format!("({}) {}-{}", &d[..2], &d[2..6], &d[6..]),
        11 => format!("({}) {}-{}", &d[..2], &d[2..7], &d[7..]),
        _ => texto.trim().to_string(),
    }
}

/// Siglas dos estados.
pub const UFS: &[&str] = &[
    "AC", "AL", "AM", "AP", "BA", "CE", "DF", "ES", "GO", "MA", "MG", "MS", "MT", "PA", "PB", "PE", "PI", "PR", "RJ",
    "RN", "RO", "RR", "RS", "SC", "SE", "SP", "TO",
];

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn confere_cpf() {
        assert!(cpf_valido("52998224725"));
        assert!(cpf_valido("12345678909"));
        assert!(!cpf_valido("12345678900"));
        assert!(!cpf_valido("11111111111"));
        assert!(!cpf_valido("1234567890"));
    }

    #[test]
    fn confere_cnpj_numerico_e_alfanumerico() {
        assert!(cnpj_valido("11222333000181"));
        assert!(!cnpj_valido("11222333000182"));
        assert!(!cnpj_valido("00000000000000"));
        // exemplo publicado pela Receita para o CNPJ alfanumérico
        assert!(cnpj_valido("12ABC34501DE35"));
        assert!(!cnpj_valido("12ABC34501DE36"));
    }

    #[test]
    fn escreve_documentos() {
        assert_eq!(escrever("12345678909"), "123.456.789-09");
        assert_eq!(escrever("11222333000181"), "11.222.333/0001-81");
        assert_eq!(escrever_cep("18320095"), "18320-095");
        assert_eq!(escrever_telefone("1532320000"), "(15) 3232-0000");
        assert_eq!(escrever_telefone("15 91234 5678"), "(15) 91234-5678");
        assert_eq!(limpar("12.abc.345/01de-35"), "12ABC34501DE35");
    }

    #[test]
    fn confere_placas() {
        assert_eq!(ler_placa("abc-1234").unwrap(), "ABC1234");
        assert_eq!(ler_placa("BRA2E19").unwrap(), "BRA2E19");
        assert!(ler_placa("AB12345").is_err());
        assert_eq!(escrever_placa("ABC1234"), "ABC-1234");
        assert_eq!(escrever_placa("BRA2E19"), "BRA2E19");
    }
}
