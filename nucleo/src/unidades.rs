//! Unidades de medida (UN, KG, MT...). A lista vem pronta e a loja pode
//! criar as suas. As fracionadas (KG, LT, MT...) vendem quebrado: 0,350 kg.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::erro::{Erro, Resultado};
use crate::texto;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Unidade {
    pub sigla: String,
    pub descricao: String,
    pub fracionada: bool,
    /// Quantos produtos usam.
    pub produtos: i64,
}

pub fn listar(conn: &Connection) -> Resultado<Vec<Unidade>> {
    let mut c = conn.prepare(
        "SELECT u.sigla, u.descricao, u.fracionada, (SELECT COUNT(*) FROM produto p WHERE p.unidade = u.sigla) AS n
           FROM unidade u ORDER BY n DESC, u.sigla",
    )?;
    let lista = c
        .query_map([], |l| {
            Ok(Unidade { sigla: l.get(0)?, descricao: l.get(1)?, fracionada: l.get(2)?, produtos: l.get(3)? })
        })?
        .collect::<Result<_, _>>()?;
    Ok(lista)
}

/// A sigla como o Rustinho guarda: maiúsculas, sem espaço, até 6 letras.
pub fn conferir_sigla(sigla: &str) -> Resultado<String> {
    let s = texto::limpar(sigla).to_uppercase();
    if s.is_empty() {
        return Err(Erro::campo("unidade", "Escolha a unidade."));
    }
    if texto::tamanho(&s) > 6 || s.contains(' ') {
        return Err(Erro::campo("unidade", "A unidade tem no máximo 6 letras, sem espaço (ex.: UN, KG, MT)."));
    }
    Ok(s)
}

/// Cria ou muda uma unidade.
pub fn salvar(conn: &Connection, sigla: &str, descricao: &str, fracionada: bool) -> Resultado<String> {
    let sigla = conferir_sigla(sigla).map_err(|e| match e {
        Erro::Validacao { mensagem, .. } => Erro::campo("sigla", mensagem),
        outro => outro,
    })?;
    let descricao = texto::limpar(descricao);
    if texto::tamanho(&descricao) > 40 {
        return Err(Erro::campo("descricao", "Use no máximo 40 letras."));
    }
    let descricao = if descricao.is_empty() { sigla.clone() } else { descricao };
    conn.execute(
        "INSERT INTO unidade (sigla, descricao, fracionada) VALUES (?1, ?2, ?3)
         ON CONFLICT (sigla) DO UPDATE SET descricao = excluded.descricao, fracionada = excluded.fracionada",
        params![sigla, descricao, fracionada],
    )?;
    Ok(sigla)
}

/// Garante que a unidade existe na lista (uma digitada ou vinda de outro
/// sistema entra como da loja, inteira).
pub fn garantir(conn: &Connection, sigla: &str) -> Resultado<()> {
    conn.execute("INSERT OR IGNORE INTO unidade (sigla, descricao) VALUES (?1, ?1)", [sigla])?;
    Ok(())
}

/// A unidade vende quebrado?
pub fn fracionada(conn: &Connection, sigla: &str) -> Resultado<bool> {
    Ok(conn
        .query_row("SELECT fracionada FROM unidade WHERE sigla = ?1", [sigla], |l| l.get(0))
        .optional()?
        .unwrap_or(false))
}

/// Junta os nomes repetidos que vêm de outros sistemas (PCS, PEC → PC).
pub fn normalizar(sigla: &str) -> String {
    let s = sigla.trim().to_uppercase();
    let igual = match s.as_str() {
        "UND" | "UNID" | "UNI" | "UNIDADE" | "UNIT" | "U" => "UN",
        "PCS" | "PEC" | "PECA" | "PEÇA" | "PÇ" | "PÇS" => "PC",
        "KT" => "KIT",
        "JOGO" => "JG",
        "PARES" => "PAR",
        "CONJ" => "CJ",
        "CXA" | "CAIXA" => "CX",
        "PACOTE" | "PAC" => "PCT",
        "KGL" | "KILO" | "QUILO" | "KGS" => "KG",
        "GR" | "GRS" => "G",
        "L" | "LITRO" | "LTS" | "LTR" => "LT",
        "M" | "MET" | "METRO" | "MTS" => "MT",
        "M²" => "M2",
        "M³" => "M3",
        "ROLO" => "RL",
        "GALAO" | "GALÃO" => "GL",
        _ => "",
    };
    if igual.is_empty() {
        s
    } else {
        igual.to_string()
    }
}

/// Unidade vinda de uma nota ou de outro sistema: junta as repetidas e, se
/// não servir (longa, com espaço), usa UN.
pub fn de_fora(sigla: &str) -> String {
    let s = normalizar(sigla);
    if s.is_empty() || texto::tamanho(&s) > 6 || s.contains(' ') {
        "UN".into()
    } else {
        s
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;

    #[test]
    fn lista_pronta_e_da_loja() {
        let b = Banco::em_memoria().unwrap();
        let l = listar(&b.conn).unwrap();
        assert!(l.iter().any(|u| u.sigla == "KG" && u.fracionada));
        assert!(l.iter().any(|u| u.sigla == "UN" && !u.fracionada));
        assert!(!l.iter().any(|u| u.sigla == "L" || u.sigla == "KGL"));
        assert_eq!(salvar(&b.conn, " qd ", "Quadrado de 10x10", false).unwrap(), "QD");
        assert!(listar(&b.conn).unwrap().iter().any(|u| u.sigla == "QD" && u.descricao == "Quadrado de 10x10"));
        assert!(salvar(&b.conn, "MUITO LONGA", "", false).is_err());
        garantir(&b.conn, "610").unwrap();
        assert!(!fracionada(&b.conn, "610").unwrap());
        assert!(fracionada(&b.conn, "MT").unwrap());
    }

    #[test]
    fn junta_repetidas() {
        for (de, para) in [("PCS", "PC"), ("pec", "PC"), ("KGL", "KG"), ("L", "LT"), ("MET", "MT"), ("M", "MT"), ("KT", "KIT"), ("QD", "QD")] {
            assert_eq!(normalizar(de), para);
        }
    }
}
