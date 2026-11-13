//! Protótipo do resumo por tabela da troca (passo 5.4, parte 1; frente F54),
//! só para medir: não é código do programa. Desenho da F5 (`RELATORIO_F5.md`
//! §7.2): para cada tabela que viaja, as colunas que viajam em JSON
//! (`json_array`, na ordem da tabela; na tabela com chave natural, sem o id;
//! na `config`, só as configurações da loja), SHA-256 de cada linha e a soma
//! dos 8 primeiros bytes, módulo 2^64 (não depende da ordem das linhas).

// Cada teste que inclui este arquivo usa só uma parte dele.
#![allow(dead_code)]

use rustinho_nucleo::dados::esquema;
use rustinho_nucleo::sync;
use sha2::{Digest, Sha256};

/// As colunas de uma tabela que entram no resumo.
pub fn colunas_do_resumo(c: &rusqlite::Connection, tabela: &str) -> Vec<String> {
    let mut st = c.prepare(&format!("PRAGMA table_info(\"{tabela}\")")).unwrap();
    let todas: Vec<(String, i64)> =
        st.query_map([], |l| Ok((l.get(1)?, l.get(5)?))).unwrap().collect::<Result<_, _>>().unwrap();
    todas
        .into_iter()
        .filter(|(n, pk)| esquema::coluna_viaja(tabela, n) && !(esquema::chave_natural(tabela).is_some() && *pk > 0))
        .map(|(n, _)| n)
        .collect()
}

/// (tabela, linhas, soma, milissegundos) de cada tabela que viaja.
pub fn resumo(c: &rusqlite::Connection) -> Vec<(String, u64, u64, f64)> {
    let mut saida = Vec::new();
    for tabela in sync::TABELAS {
        let t = std::time::Instant::now();
        let cols = colunas_do_resumo(c, tabela);
        let sql = format!(
            "SELECT json_array({}){} FROM \"{tabela}\"",
            cols.iter().map(|n| format!("\"{n}\"")).collect::<Vec<_>>().join(", "),
            if *tabela == "config" { ", chave" } else { "" }
        );
        let mut st = c.prepare(&sql).unwrap();
        let mut linhas = st.query([]).unwrap();
        let (mut n, mut soma) = (0u64, 0u64);
        while let Some(l) = linhas.next().unwrap() {
            if *tabela == "config" && !sync::chave_da_loja(&l.get::<_, String>(1).unwrap()) {
                continue;
            }
            let texto: String = l.get(0).unwrap();
            let h = Sha256::digest(texto.as_bytes());
            soma = soma.wrapping_add(u64::from_le_bytes(h[..8].try_into().unwrap()));
            n += 1;
        }
        saida.push((tabela.to_string(), n, soma, t.elapsed().as_secs_f64() * 1000.0));
    }
    saida
}

/// As tabelas em que dois resumos diferem: (tabela, linhas e soma de um, do outro).
pub fn diferentes(a: &[(String, u64, u64, f64)], b: &[(String, u64, u64, f64)]) -> Vec<String> {
    a.iter()
        .zip(b)
        .filter(|(x, y)| x.0 != y.0 || x.1 != y.1 || x.2 != y.2)
        .map(|(x, y)| format!("{}: {} linhas {:016x} × {} linhas {:016x}", x.0, x.1, x.2, y.1, y.2))
        .collect()
}
