//! Política das senhas novas (F-17).
//!
//! Vale só para a senha que a pessoa **digita para criar ou trocar**: entrar
//! com uma senha antiga continua funcionando, e nada aqui apaga, troca ou
//! tranca nada. As regras seguem o NIST SP 800-63B: tamanho mínimo, aceitar
//! qualquer caractere (espaços e Unicode inclusive), recusar o que é
//! previsível, sem exigir maiúscula/símbolo nem troca periódica.
//!
//! - Operador e gerente: pelo menos 4 caracteres (decisão do dono da loja).
//!   A senha do gerente também tranca os backups (veja `backup`).
//! - Senha nova: no máximo 128 caracteres. O login e a conferência de senhas
//!   antigas continuam aceitando até 256 (`MAXIMO`), para ninguém ficar de fora.
//!   Conta caracteres, não bytes: acento não pesa dobrado.
//! - Recusa: senhas comuns, sequências (`1234`, `abcdefgh`), repetições
//!   (`1111`, `abababab`), trechos do teclado (`qwertyui`) e o nome do
//!   usuário, da loja ou do computador.
//!
//! A senha não é normalizada (NFC/NFKC) antes de virar hash: mudar isso
//! mudaria a chave dos backups e da rede feitos com acentos decompostos.

use serde::Serialize;
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// Limite aceito no login e na conferência (senhas antigas podem ser longas).
pub const MAXIMO: usize = 256;
/// Limite para senha nova.
pub const MAXIMO_NOVA: usize = 128;
pub const MINIMO_OPERADOR: usize = 4;
pub const MINIMO_GERENTE: usize = 4;

/// Quem vai usar a senha e onde: serve para recusar a senha que repete o
/// nome do usuário, da loja ou do computador.
#[derive(Debug, Clone, Copy, Default)]
pub struct Contexto<'a> {
    pub usuario: &'a str,
    pub loja: &'a str,
    pub caixa: &'a str,
    pub gerente: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Problema {
    Curta,
    Comprida,
    Comum,
    Sequencia,
    Repetida,
    Teclado,
    ParecidaComNome,
}

impl Problema {
    /// Texto curto, sem a senha, para a auditoria e para o painel do gerente.
    pub fn codigo(self) -> &'static str {
        match self {
            Problema::Curta => "curta",
            Problema::Comprida => "comprida",
            Problema::Comum => "comum",
            Problema::Sequencia => "sequencia",
            Problema::Repetida => "repetida",
            Problema::Teclado => "teclado",
            Problema::ParecidaComNome => "nome",
        }
    }

    pub fn do_codigo(c: &str) -> Option<Problema> {
        [
            Problema::Curta,
            Problema::Comprida,
            Problema::Comum,
            Problema::Sequencia,
            Problema::Repetida,
            Problema::Teclado,
            Problema::ParecidaComNome,
        ]
        .into_iter()
        .find(|p| p.codigo() == c)
    }

    /// Mensagem para quem está criando a senha (diz o que mudar).
    pub fn mensagem(self, gerente: bool) -> String {
        match self {
            Problema::Curta if gerente => format!(
                "A senha de gerente precisa ter pelo menos {MINIMO_GERENTE} caracteres. Ela também protege os backups: \
                 quanto mais longa, mais segura (dica: junte palavras, como cafe-com-pao-2026)."
            ),
            Problema::Curta => format!(
                "A senha precisa ter pelo menos {MINIMO_OPERADOR} caracteres. Quanto mais longa, mais segura."
            ),
            Problema::Comprida => format!("Senha comprida demais: use no máximo {MAXIMO_NOVA} caracteres."),
            Problema::Comum => "Essa senha é muito comum. Junte palavras, como cafe-com-pao-2026.".into(),
            Problema::Sequencia => "Evite sequências como 1234 ou abcd. Junte palavras.".into(),
            Problema::Repetida => "Evite repetições como 1111 ou abab. Junte palavras.".into(),
            Problema::Teclado => "Evite trechos do teclado, como qwertyui. Junte palavras.".into(),
            Problema::ParecidaComNome => {
                "A senha não pode ser o nome do usuário, da loja ou deste computador (nem parecida com eles).".into()
            }
        }
    }

    /// Frase curta para o aviso de quem entrou com uma senha antiga.
    pub fn resumo(self) -> &'static str {
        match self {
            Problema::Curta => "é curta",
            Problema::Comprida => "é comprida demais",
            Problema::Comum => "é muito comum",
            Problema::Sequencia => "é uma sequência",
            Problema::Repetida => "é uma repetição",
            Problema::Teclado => "é um trecho do teclado",
            Problema::ParecidaComNome => "é parecida com um nome da loja",
        }
    }
}

pub fn minimo(gerente: bool) -> usize {
    if gerente {
        MINIMO_GERENTE
    } else {
        MINIMO_OPERADOR
    }
}

/// Texto de apoio do campo "senha" nas telas.
pub fn dica(gerente: bool) -> String {
    format!("Pelo menos {} caracteres", minimo(gerente))
}

/// Avalia a senha. `None` = dentro da política.
pub fn avaliar(senha: &str, ctx: &Contexto) -> Option<Problema> {
    let total = senha.chars().count();
    if total > MAXIMO_NOVA {
        return Some(Problema::Comprida);
    }
    if total < minimo(ctx.gerente) {
        return Some(Problema::Curta);
    }
    let f = forma(senha);
    let bruta: String = senha.chars().flat_map(char::to_lowercase).filter(|c| !c.is_whitespace()).collect();

    if bruta.is_empty() || tudo_igual(&bruta) || periodica(&f) || periodica(&bruta) {
        return Some(Problema::Repetida);
    }
    if comum(&f) {
        return Some(Problema::Comum);
    }
    if sequencia(&f) {
        return Some(Problema::Sequencia);
    }
    if teclado(&f) || teclado(&bruta) {
        return Some(Problema::Teclado);
    }
    for nome in [ctx.usuario, ctx.loja, ctx.caixa] {
        if parecida_com_nome(&f, &forma(nome)) {
            return Some(Problema::ParecidaComNome);
        }
    }
    None
}

/// Forma de comparação: sem acento, minúscula, só letras e números.
fn forma(s: &str) -> String {
    s.nfkd().filter(|c| !is_combining_mark(*c)).flat_map(char::to_lowercase).filter(|c| c.is_alphanumeric()).collect()
}

fn tudo_igual(s: &str) -> bool {
    let mut c = s.chars();
    match c.next() {
        Some(p) => c.all(|x| x == p),
        None => false,
    }
}

/// Um trecho que se repete até o fim (`abababab`, `12341234`, `passwordpassword`).
fn periodica(s: &str) -> bool {
    let v: Vec<char> = s.chars().collect();
    (1..=v.len() / 2).any(|unidade| (unidade..v.len()).all(|i| v[i] == v[i - unidade]))
}

/// Cada caractere é o anterior mais um (ou menos um): `12345678`, `hgfedcba`.
fn sequencia(s: &str) -> bool {
    let v: Vec<i64> = s.chars().map(|c| c as i64).collect();
    if v.len() < 4 {
        return false;
    }
    if v.windows(2).all(|p| p[1] - p[0] == 1) || v.windows(2).all(|p| p[0] - p[1] == 1) {
        return true;
    }
    // Só números: o 9 volta ao 0 (`12345678901234`, `9876543210987`).
    s.chars().all(|c| c.is_ascii_digit())
        && (v.windows(2).all(|p| (p[1] - p[0]).rem_euclid(10) == 1)
            || v.windows(2).all(|p| (p[0] - p[1]).rem_euclid(10) == 1))
}

const LINHAS_DO_TECLADO: &[&str] = &[
    "qwertyuiop",
    "asdfghjkl",
    "zxcvbnm",
    "1234567890",
    "qwertyuiopasdfghjklzxcvbnm",
    "1q2w3e4r5t6y7u8i9o0p",
    "1qaz2wsx3edc4rfv5tgb",
    "qazwsxedcrfv",
    "azertyuiop",
    "qsdfghjklm",
    "!@#$%^&*()",
    "1q2w3e4r",
];

fn teclado(s: &str) -> bool {
    s.chars().count() >= 6
        && LINHAS_DO_TECLADO.iter().any(|linha| {
            let invertida: String = linha.chars().rev().collect();
            linha.contains(s) || invertida.contains(s)
        })
}

/// Senhas muito usadas (já sem acento e sem separadores).
const COMUNS: &[&str] = &[
    "12345678",
    "123456789",
    "1234567890",
    "12345678910",
    "123123123",
    "11223344",
    "987654321",
    "000000000",
    "senha123",
    "senha1234",
    "minhasenha",
    "senhasecreta",
    "mudar123",
    "mudarsenha",
    "trocar123",
    "password",
    "password1",
    "passw0rd",
    "p4ssw0rd",
    "qwerty123",
    "qwertyuiop",
    "1q2w3e4r",
    "1q2w3e4r5t",
    "q1w2e3r4",
    "abc12345",
    "abcd1234",
    "abcd12345",
    "a1b2c3d4",
    "admin123",
    "admin1234",
    "administrador",
    "letmein1",
    "welcome1",
    "iloveyou",
    "bemvindo",
    "bemvindo1",
    "brasil123",
    "brasil2020",
    "brasil2026",
    "rustinho",
    "rustinho1",
    "fanti123",
    "fantiauto",
    "autopecas",
    "autopecas1",
    "loja1234",
    "caixa123",
    "gerente123",
    "gerente1",
    "vendas123",
    "teste123",
    "teste1234",
    "usuario123",
    "flamengo",
    "corinthians",
    "palmeiras",
    "saopaulo",
    "gremio123",
    "cruzeiro",
    "vasco1898",
    "amor1234",
    "deusefiel",
    "jesus123",
    "familia123",
];

/// Palavras muito usadas seguidas de poucos números (`senha2026`, `brasil10`).
const BASES: &[&str] = &[
    "senha",
    "password",
    "admin",
    "mudar",
    "brasil",
    "rustinho",
    "fanti",
    "teste",
    "loja",
    "caixa",
    "gerente",
    "vendas",
    "qwerty",
    "abc",
    "welcome",
    "bemvindo",
    "usuario",
    "flamengo",
    "corinthians",
    "palmeiras",
    "saopaulo",
    "amor",
    "deus",
    "jesus",
];

fn comum(f: &str) -> bool {
    if COMUNS.contains(&f) {
        return true;
    }
    let base = f.trim_end_matches(|c: char| c.is_ascii_digit());
    let digitos = f.len() - base.len();
    digitos > 0 && digitos <= 6 && BASES.contains(&base)
}

/// É o nome (com ou sem números no fim) ou tem o nome inteiro ocupando mais
/// da metade da senha. Nomes de menos de 4 letras só valem se a senha for
/// o próprio nome mais números ("ana12345"), para não recusar "banana-verde".
fn parecida_com_nome(f: &str, nome: &str) -> bool {
    if nome.is_empty() {
        return false;
    }
    let sem_numeros = f.trim_end_matches(|c: char| c.is_ascii_digit());
    if sem_numeros == nome || f == nome {
        return true;
    }
    nome.chars().count() >= 4 && f.contains(nome) && nome.chars().count() * 2 > f.chars().count()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn op(s: &str) -> Option<Problema> {
        avaliar(s, &Contexto::default())
    }
    fn ger(s: &str) -> Option<Problema> {
        avaliar(s, &Contexto { gerente: true, ..Contexto::default() })
    }
    fn com(s: &str, usuario: &str, loja: &str, caixa: &str) -> Option<Problema> {
        avaliar(s, &Contexto { usuario, loja, caixa, gerente: false })
    }

    /// S-01: limites de tamanho (operador 4, gerente 4, senha nova até 128; login aceita até 256).
    #[test]
    fn s01_tamanhos() {
        assert_eq!(op("caf"), Some(Problema::Curta));
        assert_eq!(op("café"), None, "4 caracteres");
        assert_eq!(ger("caf"), Some(Problema::Curta), "3 caracteres");
        assert_eq!(ger("café"), None, "4 caracteres, gerente igual ao operador");
        assert_eq!(op(&format!("éé{}", "cafe-com-pao")), None);
        let max: String =
            (0..).map(|i| format!("açúcar{i}-")).flat_map(|w| w.chars().collect::<Vec<_>>()).take(MAXIMO_NOVA).collect();
        assert_eq!(max.chars().count(), 128);
        assert_eq!(op(&max), None, "128 caracteres passa");
        assert_eq!(op(&format!("{max}z")), Some(Problema::Comprida));
        assert_eq!(ger(&format!("{max}z")), Some(Problema::Comprida));
        assert!(MAXIMO >= 256, "o login continua aceitando senhas antigas longas");
    }

    /// S-02: acento conta como um caractere; espaços, emoji e outros alfabetos valem.
    #[test]
    fn s02_unicode_e_espacos() {
        assert_eq!(op("áéíóúãõçü"), None, "9 caracteres, 18 bytes");
        assert_eq!(op("pão de queijo 2026"), None);
        assert_eq!(op("  espaços  nas pontas "), None);
        assert_eq!(op("日本語のパスワードです"), None);
        assert_eq!(op("🙂🔑🙂🔒🌟🌞🌙⭐"), None);
        assert_eq!(ger("frase longa com espaços"), None);
        assert_eq!(op("áéí"), Some(Problema::Curta));
    }

    /// S-03: não exige maiúscula, número nem símbolo.
    #[test]
    fn s03_sem_regras_de_composicao() {
        assert_eq!(op("cafecompaoquente"), None);
        assert_eq!(op("CAFECOMPAOQUENTE"), None);
        assert_eq!(op("Trem Azul da Serra 77"), None);
        assert_eq!(ger("correct horse battery staple"), None);
        assert_eq!(op("minha-senha-secreta-de-loja"), None, "frase que só contém palavras da lista");
    }

    /// S-04: senhas comuns, sem diferenciar maiúscula, acento ou separador.
    #[test]
    fn s04_comuns() {
        for s in [
            "12345678",
            "Senha123",
            "PASSWORD",
            "Sénha123",
            "brasil123",
            "senha2026",
            "Pass-Word",
            "Admin_123",
            "RUSTINHO",
        ] {
            assert!(matches!(op(s), Some(Problema::Comum) | Some(Problema::Sequencia)), "{s}: {:?}", op(s));
        }
        assert_eq!(op("senha123"), Some(Problema::Comum));
        assert_eq!(op("Brasil 2026"), Some(Problema::Comum));
        assert_eq!(op("senha-2026"), Some(Problema::Comum));
        assert_eq!(ger("rustinho1234"), Some(Problema::Comum));
        assert_eq!(op("senha-do-balcao-cinco"), None);
    }

    /// S-05: sequências crescentes e decrescentes.
    #[test]
    fn s05_sequencias() {
        for s in ["23456789", "87654321", "cdefghij", "hgfedcba", "0123456789"] {
            assert_eq!(op(s), Some(Problema::Sequencia), "{s}");
        }
        assert_eq!(op("abcdefgh"), Some(Problema::Sequencia));
        assert_eq!(ger("abcdefghijkl"), Some(Problema::Sequencia));
        assert_eq!(op("abcdefgx"), None, "quase sequência, mas não é");
        assert_eq!(op("12345678901234"), Some(Problema::Sequencia), "o 9 volta ao 0");
        assert_eq!(op("9876543210987"), Some(Problema::Sequencia));
        assert_eq!(op("12345678a"), None, "mistura de letra e número não é sequência pura");
    }

    /// S-06: repetições.
    #[test]
    fn s06_repeticoes() {
        for s in [
            "11111111",
            "aaaaaaaaaaaa",
            "abababab",
            "12341234",
            "!!!!!!!!",
            "        ",
            "xyzxyzxyz",
            "passwordpassword",
        ] {
            assert_eq!(op(s), Some(Problema::Repetida), "{s:?}");
        }
        assert_eq!(op("abababac"), None);
        assert_eq!(op("🙂🙂🙂🙂🙂🙂🙂🙂"), Some(Problema::Repetida));
    }

    /// S-07: trechos do teclado.
    #[test]
    fn s07_teclado() {
        assert_eq!(op("qwertyui"), Some(Problema::Teclado));
        assert_eq!(op("asdfghjk"), Some(Problema::Teclado));
        assert_eq!(op("1q2w3e4r5t6y"), Some(Problema::Teclado));
        assert_eq!(op("poiuytrewq"), Some(Problema::Teclado));
        assert_eq!(op("!@#$%^&*"), Some(Problema::Teclado));
        assert_eq!(op("QWERTYUIOP"), Some(Problema::Comum), "na lista e também no teclado");
        assert_eq!(op("qwertyuz"), None);
    }

    /// S-08: nome do usuário, da loja e do computador.
    #[test]
    fn s08_nomes() {
        let u = "Maria Souza";
        assert_eq!(com("mariasouza", u, "", ""), Some(Problema::ParecidaComNome));
        assert_eq!(com("MARIA SOUZA", u, "", ""), Some(Problema::ParecidaComNome));
        assert_eq!(com("mariasouza2026", u, "", ""), Some(Problema::ParecidaComNome));
        assert_eq!(com("Maria-Souza-77", u, "", ""), Some(Problema::ParecidaComNome));
        assert_eq!(com("cafe-com-pao-quente-da-maria-souza", u, "", ""), None, "o nome é só uma parte pequena");
        assert_eq!(com("fantiautopecas", "Ana", "Fanti Auto Peças", "Caixa 1"), Some(Problema::ParecidaComNome));
        assert_eq!(com("caixa1x2y3", "Ana", "Loja", "Caixa 1"), Some(Problema::ParecidaComNome));
        assert_eq!(com("ana12345", "Ana", "", ""), Some(Problema::ParecidaComNome), "nome curto + números");
        assert_eq!(com("banana-verde-2026", "Ana", "", ""), None, "nome curto dentro de outra palavra");
        assert_eq!(com("cafe-com-pao-2026", "Ana", "Loja Central", "Caixa 1"), None);
    }

    /// S-09: mensagens claras, sem a senha, e códigos que voltam ao mesmo problema.
    #[test]
    fn s09_mensagens() {
        for p in [
            Problema::Curta,
            Problema::Comprida,
            Problema::Comum,
            Problema::Sequencia,
            Problema::Repetida,
            Problema::Teclado,
            Problema::ParecidaComNome,
        ] {
            assert!(!p.mensagem(false).is_empty() && !p.mensagem(true).is_empty() && !p.resumo().is_empty());
            assert_eq!(Problema::do_codigo(p.codigo()), Some(p));
        }
        assert!(Problema::Curta.mensagem(true).contains('4'));
        assert!(Problema::Curta.mensagem(true).contains("backups"));
        assert!(Problema::Curta.mensagem(false).contains('4'));
        assert_eq!(Problema::do_codigo("nada"), None);
        assert_eq!(dica(false), "Pelo menos 4 caracteres");
        assert_eq!(dica(true), "Pelo menos 4 caracteres");
    }

    /// S-10: senhas boas de verdade passam, para operador e gerente.
    #[test]
    fn s10_boas_passam() {
        for s in ["cafe-com-pao-2026", "Trem Azul da Serra 77", "chuva de manhã na oficina", "7 gatos & 3 cachorros!"]
        {
            assert_eq!(op(s), None, "{s}");
            assert_eq!(ger(s), None, "{s}");
        }
    }
}
