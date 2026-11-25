//! R9-A1 (revisão R9 da 007, 25/11/2026): "O que comprar", "Peças paradas" e "Mais vendidos" ficam lineares no tamanho
//! do catálogo. A soma pela peça que ficou (R13, `produto_raiz`) não pode estar no `GROUP BY` das partes que a lista
//! junta por peça: com ela o SQLite perde o índice automático do `LEFT JOIN` e varre a parte inteira para cada peça
//! (8000 peças: 197 ms → 19 s). O texto das consultas é o de `nucleo/src/compras/reposicao.rs`.

use std::path::PathBuf;

use rustinho_nucleo::banco::Banco;

fn consulta(funcao: &str) -> String {
    let fonte =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/compras/reposicao.rs")).unwrap();
    let inicio = fonte.find(&format!("pub fn {funcao}(")).unwrap_or_else(|| panic!("reposicao::{funcao}"));
    let marca = "conn.prepare(\n        \"";
    let de = inicio + fonte[inicio..].find(marca).unwrap() + marca.len();
    let ate = de + fonte[de..].find("\",\n    )?;").unwrap();
    fonte[de..ate].to_string()
}

fn plano(sql: &str, n: usize) -> Vec<String> {
    let b = Banco::em_memoria().unwrap();
    let mut st = b.conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
    let params: Vec<&dyn rusqlite::ToSql> = (0..n).map(|_| &"2026-01-01" as &dyn rusqlite::ToSql).collect();
    st.query_map(params.as_slice(), |l| l.get::<_, String>(3)).unwrap().map(Result::unwrap).collect()
}

fn sem_varredura(funcao: &str, partes: &[&str], n: usize) {
    let p = plano(&consulta(funcao), n);
    for parte in partes {
        assert!(
            !p.iter().any(|l| l.trim() == format!("SCAN {parte}") || l.starts_with(&format!("SCAN {parte} "))),
            "{funcao}: a parte `{parte}` é varrida para cada peça (perdeu o índice): {p:#?}"
        );
    }
}

#[test]
fn r9_a1_o_que_comprar_e_linear() {
    sem_varredura("sugestao", &["vend", "enc"], 1);
}

#[test]
fn r9_a1_pecas_paradas_e_linear() {
    sem_varredura("parados", &["uv", "ue"], 1);
}
