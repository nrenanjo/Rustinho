//! Política das senhas novas (F-17).
//!
//! Vale só para a senha que a pessoa **digita para criar ou trocar**: entrar
//! com uma senha antiga continua funcionando, e nada aqui apaga, troca ou
//! tranca nada.
//!
//! Decisão do dono da loja (0.9.13): a única exigência é o tamanho.
//! - Operador, gerente e senha da rede: pelo menos 4 caracteres.
//! - Senha nova: no máximo 128 caracteres. O login e a conferência de senhas
//!   antigas continuam aceitando até 256 (`MAXIMO`), para ninguém ficar de fora.
//!   Conta caracteres, não bytes: acento não pesa dobrado.
//! - Qualquer caractere vale (espaços, acentos, emoji). Não recusa senha
//!   comum, sequência, repetição, trecho do teclado nem nome parecido.
//!
//! A senha não é normalizada (NFC/NFKC) antes de virar hash: mudar isso
//! mudaria a chave dos backups e da rede feitos com acentos decompostos.

use serde::Serialize;

/// Limite aceito no login e na conferência (senhas antigas podem ser longas).
pub const MAXIMO: usize = 256;
/// Limite para senha nova.
pub const MAXIMO_NOVA: usize = 128;
pub const MINIMO_OPERADOR: usize = 4;
pub const MINIMO_GERENTE: usize = 4;

/// Quem vai usar a senha e onde. Hoje só o papel (gerente) muda a mensagem;
/// os nomes ficam para não mudar quem chama.
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
}

impl Problema {
    /// Texto curto, sem a senha, para a auditoria e para o painel do gerente.
    pub fn codigo(self) -> &'static str {
        match self {
            Problema::Curta => "curta",
            Problema::Comprida => "comprida",
        }
    }

    /// Códigos antigos ("comum", "sequencia"...) gravados por versões
    /// anteriores não valem mais: voltam `None` e o aviso some sozinho.
    pub fn do_codigo(c: &str) -> Option<Problema> {
        [Problema::Curta, Problema::Comprida].into_iter().find(|p| p.codigo() == c)
    }

    /// Mensagem para quem está criando a senha (diz o que mudar).
    pub fn mensagem(self, gerente: bool) -> String {
        match self {
            Problema::Curta if gerente => format!(
                "A senha de gerente precisa ter pelo menos {MINIMO_GERENTE} caracteres. Ela também protege os backups: \
                 quanto mais longa, mais segura."
            ),
            Problema::Curta => {
                format!("A senha precisa ter pelo menos {MINIMO_OPERADOR} caracteres. Quanto mais longa, mais segura.")
            }
            Problema::Comprida => format!("Senha comprida demais: use no máximo {MAXIMO_NOVA} caracteres."),
        }
    }

    /// Frase curta para o aviso de quem entrou com uma senha antiga.
    pub fn resumo(self) -> &'static str {
        match self {
            Problema::Curta => "é curta",
            Problema::Comprida => "é comprida demais",
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

/// Avalia a senha. `None` = dentro da política (só o tamanho conta).
pub fn avaliar(senha: &str, ctx: &Contexto) -> Option<Problema> {
    let total = senha.chars().count();
    if total > MAXIMO_NOVA {
        return Some(Problema::Comprida);
    }
    if total < minimo(ctx.gerente) {
        return Some(Problema::Curta);
    }
    None
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
        assert_eq!(op(""), Some(Problema::Curta));
        let max: String = (0..)
            .map(|i| format!("açúcar{i}-"))
            .flat_map(|w| w.chars().collect::<Vec<_>>())
            .take(MAXIMO_NOVA)
            .collect();
        assert_eq!(max.chars().count(), 128);
        assert_eq!(op(&max), None, "128 caracteres passa");
        assert_eq!(op(&format!("{max}z")), Some(Problema::Comprida));
        assert_eq!(ger(&format!("{max}z")), Some(Problema::Comprida));
        assert_eq!(MAXIMO, 256, "o login continua aceitando senhas antigas longas");
    }

    /// S-02: acento conta como um caractere; espaços, emoji e outros alfabetos valem.
    #[test]
    fn s02_unicode_e_espacos() {
        assert_eq!(op("áéíó"), None, "4 caracteres, 8 bytes");
        assert_eq!(op("áéí"), Some(Problema::Curta));
        assert_eq!(op("pão de queijo 2026"), None);
        assert_eq!(op("    "), None, "4 espaços: só o tamanho conta");
        assert_eq!(op("日本語の"), None);
        assert_eq!(op("🙂🔑🙂🔒"), None);
    }

    /// S-03: sem nenhuma outra restrição (decisão do dono da loja, 0.9.13).
    #[test]
    fn s03_sem_restricoes_alem_do_tamanho() {
        for s in [
            "1234",
            "0000",
            "1111",
            "abcd",
            "senha",
            "12345678",
            "password",
            "qwerty",
            "abababab",
            "rustinho",
            "CAFECOMPAO",
        ] {
            assert_eq!(op(s), None, "{s}");
            assert_eq!(ger(s), None, "{s}");
        }
        assert_eq!(com("mariasouza", "Maria Souza", "", ""), None, "nome do usuário vale");
        assert_eq!(com("fantiautopecas", "Ana", "Fanti Auto Peças", "Caixa 1"), None, "nome da loja vale");
        assert_eq!(com("caixa1", "Ana", "Loja", "Caixa 1"), None, "nome do computador vale");
    }

    /// S-04: mensagens claras, sem a senha; códigos antigos não voltam mais como problema.
    #[test]
    fn s04_mensagens_e_codigos() {
        for p in [Problema::Curta, Problema::Comprida] {
            assert!(!p.mensagem(false).is_empty() && !p.mensagem(true).is_empty() && !p.resumo().is_empty());
            assert_eq!(Problema::do_codigo(p.codigo()), Some(p));
        }
        assert!(Problema::Curta.mensagem(true).contains('4'));
        assert!(Problema::Curta.mensagem(true).contains("backups"));
        assert!(Problema::Curta.mensagem(false).contains('4'));
        assert!(Problema::Comprida.mensagem(false).contains("128"));
        for antigo in ["comum", "sequencia", "repetida", "teclado", "nome", "nada"] {
            assert_eq!(Problema::do_codigo(antigo), None, "{antigo}");
        }
        assert_eq!(dica(false), "Pelo menos 4 caracteres");
        assert_eq!(dica(true), "Pelo menos 4 caracteres");
    }
}
