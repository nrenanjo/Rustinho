//! Dev.3.3, passo 1.6 — linhagem da estrutura do banco (AUD33-N01, DP-50) e
//! ordem das migrações (AUD33-ARQ-15, DP-47). As regras estão em
//! `docs/MIGRACOES.md`.
//!
//! O Rustinho guarda só a CONTAGEM de migrações (`PRAGMA user_version`), e o
//! número 24 já tem duas estruturas publicadas: a 0024 desta linha
//! (dev.3.1/dev.3.2: GTINs e pendências XML) e a 0024 da linha paralela dev.3
//! (`v0.9.18-dev.3`, `8cb29af`), que ainda tem uma 0025 própria.
//!
//! - `estrutura_esperada(n)`: a estrutura que as migrações 0001..n deixam,
//!   gerada aplicando os `.sql` de `nucleo/src/migracoes/` em ordem, como
//!   `banco::aplicar_migracao` (a que recria tabelas roda sem chaves
//!   estrangeiras e com `legacy_alter_table`). Os gatilhos ficam de fora:
//!   nenhuma migração cria gatilho, o programa os cria ao abrir
//!   (`sync::criar_gatilhos`).
//! - Amostras: `amostras_dev3/` (veja o LEIA-ME de lá), sempre abertas em cópia.
//! - Travas (passam hoje): a estrutura esperada é a do programa; a amostra
//!   dev.3.2/24 é a 24 desta linha e abre; as amostras dev.3 não são; a
//!   estrutura 23 atualiza; a numeração é contínua e igual à lista do
//!   `banco.rs`; nenhuma migração reaproveita nome (índice redefinido, ou nome
//!   da linha dev.3 a partir da 0025).
//! - Recusas (eram caracterizações até o passo 2.4: falhavam em `4b253c6`):
//!   bancos dev.3/24, dev.3/25 e de estrutura desconhecida são recusados SEM
//!   GRAVAR (o SHA-256 do arquivo fica igual), por `banco::conferir_linhagem`,
//!   que compara a estrutura pelo conteúdo (colunas e índices), não pelo texto.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use rustinho_nucleo::banco::{versao_da_estrutura, Banco};
use sha2::{Digest, Sha256};

/// Primeira linha da migração que recria tabelas (`banco::SEM_CHAVES_ESTRANGEIRAS`).
const SEM_CHAVES_ESTRANGEIRAS: &str = "-- rustinho: sem_chaves_estrangeiras";

/// Índices redefinidos com o mesmo nome antes da regra "nunca reaproveitar
/// nome de índice": todos na 0003, publicada. Nenhum outro entra aqui.
const REDEFINIDOS_NA_HISTORIA: &[(usize, &str)] =
    &[(3, "compra_chave"), (3, "pessoa_codigo"), (3, "pessoa_documento"), (3, "produto_codigo")];

/// Nomes que as duas 0024 publicadas (desta linha e da dev.3) usam com
/// definições diferentes. Por isso a conferência é pelo conteúdo (DP-50), não
/// pelo número nem pelo nome.
const MESMO_NOME_NAS_DUAS_0024: &[(&str, &str)] = &[("table", "produto_gtin"), ("index", "produto_gtin_codigo")];

/// A partir desta migração (a primeira da dev.3.3) vale a regra dos nomes da linha dev.3.
const PRIMEIRA_DA_DEV33: usize = 25;

/// (tipo, nome) → (tabela, definição com os espaços normalizados).
type Estrutura = BTreeMap<(String, String), (String, String)>;

fn raiz_do_nucleo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Pasta temporária apagada no fim.
struct Pasta(PathBuf);

impl Pasta {
    fn nova(nome: &str) -> Pasta {
        let p = std::env::temp_dir().join(format!(
            "rustinho-linhagem-{nome}-{}-{}",
            std::process::id(),
            rustinho_nucleo::tempo::novo_id()
        ));
        std::fs::create_dir_all(&p).unwrap();
        Pasta(p)
    }
}

impl Drop for Pasta {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sha256(p: &Path) -> String {
    Sha256::digest(std::fs::read(p).unwrap()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Uma cópia da amostra numa pasta temporária (a amostra nunca é aberta).
fn copia(nome: &str) -> (Pasta, PathBuf) {
    let pasta = raiz_do_nucleo().join("tests/amostras_dev3");
    let registro = std::fs::read_to_string(pasta.join("SHA256SUMS")).unwrap();
    let esperado = registro
        .lines()
        .find_map(|l| l.split_once("  ").filter(|(_, n)| *n == nome).map(|(h, _)| h.to_string()))
        .unwrap_or_else(|| panic!("{nome} não está em amostras_dev3/SHA256SUMS"));
    assert_eq!(sha256(&pasta.join(nome)), esperado, "a amostra {nome} mudou (veja amostras_dev3/LEIA-ME.md)");
    let p = Pasta::nova(nome.trim_end_matches(".db"));
    let db = p.0.join("rustinho.db");
    std::fs::copy(pasta.join(nome), &db).unwrap();
    (p, db)
}

/// Os arquivos de migração (número, nome, SQL), na ordem dos números.
fn migracoes() -> Vec<(usize, String, String)> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(raiz_do_nucleo().join("src/migracoes")).unwrap().flatten() {
        let nome = e.file_name().to_string_lossy().to_string();
        if !nome.ends_with(".sql") {
            continue;
        }
        let numero: usize = nome
            .get(..4)
            .filter(|n| nome.as_bytes().get(4) == Some(&b'_') && n.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|n| n.parse().ok())
            .unwrap_or_else(|| panic!("migração sem número de 4 algarismos e _: {nome}"));
        v.push((numero, nome, std::fs::read_to_string(e.path()).unwrap()));
    }
    v.sort();
    v
}

fn aplicar(c: &Connection, sql: &str) {
    // As funções do Rustinho (`normalizar`...), como em toda conexão do
    // programa: a 0025 preenche o nome sem acento com ela.
    rustinho_nucleo::banco::registrar_funcoes(c).unwrap();
    let sem = sql.starts_with(SEM_CHAVES_ESTRANGEIRAS);
    c.pragma_update(None, "foreign_keys", !sem).unwrap();
    c.pragma_update(None, "legacy_alter_table", sem).unwrap();
    c.execute_batch(sql).unwrap();
}

/// Um banco (em arquivo ou na memória) com as migrações 0001..`ate`.
fn montar(c: &Connection, ate: usize) {
    for (_, _, sql) in migracoes().iter().take(ate) {
        aplicar(c, sql);
    }
    c.pragma_update(None, "foreign_keys", true).unwrap();
    c.pragma_update(None, "legacy_alter_table", false).unwrap();
    c.pragma_update(None, "user_version", ate as i64).unwrap();
}

fn estrutura(c: &Connection) -> Estrutura {
    let mut st = c
        .prepare("SELECT type, name, tbl_name, coalesce(sql, '') FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'")
        .unwrap();
    st.query_map([], |l| Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?, l.get::<_, String>(2)?, l.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .filter(|(tipo, _, _, _)| tipo != "trigger")
        .map(|(tipo, nome, tabela, sql): (String, String, String, String)| {
            ((tipo, nome), (tabela, sql.split_whitespace().collect::<Vec<_>>().join(" ")))
        })
        .collect()
}

fn estrutura_esperada(ate: usize) -> Estrutura {
    let c = Connection::open_in_memory().unwrap();
    montar(&c, ate);
    estrutura(&c)
}

/// (tabela, único, colunas) de um índice que existe no banco.
fn definicao_do_indice(c: &Connection, nome: &str) -> Option<(String, bool, Vec<String>)> {
    let tabela: String = c
        .query_row("SELECT tbl_name FROM sqlite_master WHERE type = 'index' AND name = ?1", [nome], |l| l.get(0))
        .ok()?;
    let unico: bool = c
        .query_row("SELECT \"unique\" FROM pragma_index_list(?1) WHERE name = ?2", [&tabela, nome], |l| l.get(0))
        .unwrap();
    let colunas: Vec<String> = c
        .prepare("SELECT coalesce(name, '(expressão)') FROM pragma_index_info(?1) ORDER BY seqno")
        .unwrap()
        .query_map([nome], |l| l.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    Some((tabela, unico, colunas))
}

fn estrutura_de(db: &Path) -> Estrutura {
    estrutura(&Connection::open(db).unwrap())
}

fn versao(db: &Path) -> i64 {
    Connection::open(db).unwrap().query_row("PRAGMA user_version", [], |l| l.get(0)).unwrap()
}

fn diferencas(esperada: &Estrutura, real: &Estrutura) -> Vec<String> {
    let mut d = Vec::new();
    for (k, v) in esperada {
        match real.get(k) {
            None => d.push(format!("só na esperada: {} {}", k.0, k.1)),
            Some(r) if r != v => d.push(format!("diferente: {} {}", k.0, k.1)),
            _ => {}
        }
    }
    for k in real.keys().filter(|k| !esperada.contains_key(*k)) {
        d.push(format!("só no banco: {} {}", k.0, k.1));
    }
    d
}

/// Abre pelo programa (como o caixa abre). Devolve o erro (None = abriu) e se
/// o arquivo mudou.
fn abrir_pelo_programa(db: &Path) -> (Option<String>, bool) {
    let antes = sha256(db);
    let erro = Banco::abrir(db).map(drop).err().map(|e| e.to_string());
    (erro, sha256(db) != antes)
}

// ------------------------------------------------------------------ travas

/// A estrutura gerada dos `.sql` é a que o programa deixa num banco novo.
#[test]
fn estrutura_esperada_e_a_do_programa() {
    let n = versao_da_estrutura();
    let c = Connection::open_in_memory().unwrap();
    montar(&c, n);
    let gatilhos: i64 =
        c.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger'", [], |l| l.get(0)).unwrap();
    assert_eq!(gatilhos, 0, "uma migração criou gatilho: a comparação sem gatilhos deixa de valer");
    let p = Pasta::nova("programa");
    let db = p.0.join("rustinho.db");
    drop(Banco::abrir(&db).unwrap());
    assert_eq!(versao(&db), n as i64);
    let d = diferencas(&estrutura(&c), &estrutura_de(&db));
    assert!(d.is_empty(), "o banco novo do programa não tem a estrutura esperada {n}: {d:#?}");
}

/// A amostra da dev.3.2 é a estrutura 24 desta linha.
#[test]
fn amostra_dev32_e_a_estrutura_24_desta_linha() {
    let (_p, db) = copia("dev32_estrutura24.db");
    assert_eq!(versao(&db), 24);
    let d = diferencas(&estrutura_esperada(24), &estrutura_de(&db));
    assert!(d.is_empty(), "a amostra dev.3.2/24 não bate com a estrutura 24 desta linha: {d:#?}");
}

/// As amostras da dev.3 não são a estrutura desta linha, e a diferença é a
/// que a investigação achou (o número não basta para distinguir).
#[test]
fn amostras_dev3_nao_sao_a_estrutura_desta_linha() {
    for (nome, numero) in [("dev3_estrutura24.db", 24), ("dev3_estrutura25.db", 25)] {
        let (_p, db) = copia(nome);
        assert_eq!(versao(&db), numero, "{nome}");
        let d = diferencas(&estrutura_esperada(24), &estrutura_de(&db));
        for marca in [
            "só na esperada: table xml_pendencia_identificacao",
            "só no banco: table produto_fornecedor_alternativo",
            "diferente: table produto_gtin",
            "diferente: index produto_gtin_codigo",
        ] {
            assert!(d.iter().any(|x| x == marca), "{nome}: faltou a diferença «{marca}»: {d:#?}");
        }
        if numero == 25 {
            assert!(d.iter().any(|x| x == "só no banco: table venda_cancelamento"), "{nome}: {d:#?}");
        }
    }
}

/// O banco da dev.3.2 na estrutura 24 abre normalmente (e atualiza, quando
/// esta linha tiver migrações depois da 24), sem perder os dados.
#[test]
fn banco_dev32_estrutura24_abre_normalmente() {
    let (_p, db) = copia("dev32_estrutura24.db");
    let b = Banco::abrir(&db).expect("o banco da dev.3.2 na estrutura 24 tem de abrir");
    assert!(!b.em_modo_seguro(), "abriu em modo seguro: {:?}", b.defeito);
    drop(b);
    let n = versao_da_estrutura();
    assert_eq!(versao(&db), n as i64);
    let d = diferencas(&estrutura_esperada(n), &estrutura_de(&db));
    assert!(d.is_empty(), "{d:#?}");
    let c = Connection::open(&db).unwrap();
    let ana: i64 = c.query_row("SELECT COUNT(*) FROM usuario WHERE nome = 'Ana'", [], |l| l.get(0)).unwrap();
    let peca: i64 =
        c.query_row("SELECT COUNT(*) FROM produto WHERE nome = 'Peça feita no caixa B'", [], |l| l.get(0)).unwrap();
    assert_eq!((ana, peca), (1, 1), "os dados da amostra sumiram");
}

/// Um banco na estrutura 23 (0.9.15 a 0.9.17) atualiza para a atual.
#[test]
fn banco_na_estrutura_23_atualiza_para_a_atual() {
    let p = Pasta::nova("e23");
    let db = p.0.join("rustinho.db");
    montar(&Connection::open(&db).unwrap(), 23);
    drop(Banco::abrir(&db).expect("o banco na estrutura 23 tem de abrir e atualizar"));
    let n = versao_da_estrutura();
    assert_eq!(versao(&db), n as i64);
    let d = diferencas(&estrutura_esperada(n), &estrutura_de(&db));
    assert!(d.is_empty(), "{d:#?}");
}

/// ARQ-15 / DP-47: números 0001..N sem buraco nem repetição; a lista
/// `MIGRACOES` do `banco.rs` é a lista dos arquivos, na mesma ordem; a 0024 é
/// a publicada desta linha; os arquivos da linha dev.3 nunca entram.
#[test]
fn numeracao_das_migracoes_continua_e_igual_a_lista_do_banco() {
    let m = migracoes();
    for (i, (numero, nome, _)) in m.iter().enumerate() {
        assert_eq!(*numero, i + 1, "numeração com buraco ou repetida em {nome}");
    }
    let arquivos: Vec<&str> = m.iter().map(|x| x.1.as_str()).collect();
    let fonte = std::fs::read_to_string(raiz_do_nucleo().join("src/dados/banco.rs")).unwrap();
    let lista: Vec<&str> =
        fonte.split("include_str!(\"../migracoes/").skip(1).map(|r| r.split('"').next().unwrap()).collect();
    assert_eq!(lista, arquivos, "a lista MIGRACOES do banco.rs e os arquivos de nucleo/src/migracoes/");
    assert_eq!(m.len(), versao_da_estrutura());
    assert_eq!(arquivos.get(23), Some(&"0024_gtins_e_pendencias_xml.sql"), "a 0024 publicada desta linha");
    for da_dev3 in ["0024_identificadores_produto.sql", "0025_cancelamento_idempotente.sql"] {
        assert!(!arquivos.contains(&da_dev3), "{da_dev3} é da linha dev.3 (v0.9.18-dev.3)");
    }
}

/// ARQ-15 / DP-47 e N01: nenhuma migração reaproveita nome.
/// 1. Índice (ou visão) nunca é redefinido com o mesmo nome — nem apagado e
///    criado de novo com outra definição. Só a história da 0003 fica de fora.
/// 2. Da 0025 em diante, nenhum objeto NOVO usa um nome da linha dev.3 (as
///    amostras dev.3/24 e dev.3/25): a estrutura desta linha tem de continuar
///    distinguível pelo conteúdo.
#[test]
fn nenhuma_migracao_reaproveita_nome() {
    let mut dev3 = Estrutura::new();
    for nome in ["dev3_estrutura24.db", "dev3_estrutura25.db"] {
        let (_p, db) = copia(nome);
        dev3.extend(estrutura_de(&db));
    }
    let c = Connection::open_in_memory().unwrap();
    let mut visto: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut historia = Vec::new();
    let mut erros = Vec::new();
    for (numero, arquivo, sql) in migracoes() {
        let antes = estrutura(&c);
        aplicar(&c, &sql);
        for (k, (_, def)) in estrutura(&c) {
            if antes.get(&k).is_some_and(|(_, d)| *d == def) {
                continue;
            }
            let (tipo, nome) = &k;
            if tipo != "table" && visto.get(&k).is_some_and(|d| *d != def) {
                if REDEFINIDOS_NA_HISTORIA.contains(&(numero, nome.as_str())) {
                    historia.push((numero, nome.clone()));
                } else {
                    erros.push(format!("{arquivo}: {tipo} {nome} reaproveita o nome com outra definição"));
                }
            }
            if numero >= PRIMEIRA_DA_DEV33 && !antes.contains_key(&k) && dev3.keys().any(|(_, n)| n == nome) {
                erros.push(format!(
                    "{arquivo}: {tipo} {nome} usa um nome da linha dev.3 (v0.9.18-dev.3); a estrutura deixaria de \
                     ser distinguível pelo conteúdo"
                ));
            }
            visto.insert(k.clone(), def);
        }
    }
    assert!(erros.is_empty(), "{erros:#?}");
    let registrados: Vec<(usize, String)> = REDEFINIDOS_NA_HISTORIA.iter().map(|(n, x)| (*n, x.to_string())).collect();
    historia.sort();
    assert_eq!(historia, registrados, "REDEFINIDOS_NA_HISTORIA tem de ser exatamente o que a história tem");
    // O que a regra 2 previne já aconteceu uma vez, nas duas 0024 publicadas.
    let nossa = estrutura_esperada(24);
    for (tipo, nome) in MESMO_NOME_NAS_DUAS_0024 {
        let k = (tipo.to_string(), nome.to_string());
        assert!(nossa.contains_key(&k) && dev3.contains_key(&k) && nossa[&k] != dev3[&k], "{tipo} {nome}");
    }
}

/// B01 (passo 7.5): o índice composto do estoque `(produto_id, tipo,
/// criado_em)`, que a 0006 quis criar com um nome já usado, existe com o nome
/// novo num banco novo e num banco que vem da estrutura anterior.
#[test]
fn b01_indice_composto_do_estoque_com_nome_novo() {
    let n = versao_da_estrutura();
    let esperado =
        Some(("estoque_movimento".to_string(), false, vec!["produto_id".into(), "tipo".into(), "criado_em".into()]));
    let c = Connection::open_in_memory().unwrap();
    montar(&c, n);
    assert_eq!(
        definicao_do_indice(&c, "estoque_movimento_produto_tipo_data"),
        esperado,
        "banco montado pelas migrações"
    );
    let p = Pasta::nova("b01");
    let db = p.0.join("rustinho.db");
    montar(&Connection::open(&db).unwrap(), n - 1);
    drop(Banco::abrir(&db).expect("a estrutura anterior abre e atualiza"));
    let c = Connection::open(&db).unwrap();
    assert_eq!(definicao_do_indice(&c, "estoque_movimento_produto_tipo_data"), esperado, "banco atualizado");
}

/// B01 (passo 7.5): a conta dos "parados" usa o índice composto para a última
/// entrada de cada peça (`ue`: compras e estoque inicial). O texto da consulta
/// é o de `nucleo/src/compras/reposicao.rs` (conferido abaixo: mudou lá, este
/// teste avisa).
#[test]
fn b01_parados_usa_o_indice_composto() {
    let fonte = std::fs::read_to_string(raiz_do_nucleo().join("src/compras/reposicao.rs")).unwrap();
    let inicio = fonte.find("pub fn parados(").expect("reposicao::parados");
    let de = inicio + fonte[inicio..].find("conn.prepare(\n        \"").unwrap() + "conn.prepare(\n        \"".len();
    let ate = de + fonte[de..].find("\",\n    )?;").unwrap();
    let sql = &fonte[de..ate];
    assert!(
        sql.contains("WHERE tipo IN ('compra', 'inicial') GROUP BY produto_id"),
        "a consulta dos parados mudou: {sql}"
    );
    let b = Banco::em_memoria().unwrap();
    let plano: Vec<String> = b
        .conn
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .unwrap()
        .query_map(["2026-01-01"], |l| l.get::<_, String>(3))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(
        plano.iter().any(|l| l.contains("USING COVERING INDEX estoque_movimento_produto_tipo_data")),
        "os parados não usam o índice composto: {plano:#?}"
    );
}

// ------------------------------------- recusas (caracterizações até o passo 2.4)

/// N01 (DP-50): o banco dev.3/24 é recusado sem gravar.
#[test]
fn banco_dev3_estrutura24_e_recusado_sem_gravar() {
    let (_p, db) = copia("dev3_estrutura24.db");
    let (erro, mudou) = abrir_pelo_programa(&db);
    assert!(
        erro.is_some(),
        "o banco da linha dev.3 na estrutura 24 foi ACEITO como a estrutura 24 desta linha{}",
        if mudou { ", e o arquivo foi alterado" } else { "" }
    );
    assert!(!mudou, "recusou, mas gravou no arquivo (o SHA-256 mudou): {erro:?}");
}

/// N01 (DP-50): o banco dev.3/25 é recusado sem gravar, como banco de outra
/// linha — não como "versão mais nova", que é o número falando: com uma 0025
/// desta linha, o mesmo número seria aceito.
#[test]
fn banco_dev3_estrutura25_e_recusado_como_outra_linha() {
    let (_p, db) = copia("dev3_estrutura25.db");
    let (erro, mudou) = abrir_pelo_programa(&db);
    let erro = erro.unwrap_or_else(|| panic!("o banco da linha dev.3 na estrutura 25 foi ACEITO (mudou: {mudou})"));
    assert!(!mudou, "recusou, mas gravou no arquivo (o SHA-256 mudou): {erro}");
    assert!(
        !erro.contains("mais nova"),
        "recusado só pelo número, como se fosse de um Rustinho mais novo (com uma 0025 desta linha seria aceito): {erro}"
    );
}

/// N01 (DP-50): número desta linha com outra estrutura é recusado sem gravar.
#[test]
fn banco_de_estrutura_desconhecida_e_recusado_sem_gravar() {
    let p = Pasta::nova("desconhecida");
    let db = p.0.join("rustinho.db");
    {
        let c = Connection::open(&db).unwrap();
        montar(&c, versao_da_estrutura());
        c.execute_batch("ALTER TABLE produto ADD COLUMN coluna_que_nenhuma_migracao_cria TEXT").unwrap();
    }
    let (erro, mudou) = abrir_pelo_programa(&db);
    assert!(
        erro.is_some(),
        "um banco com o número {} e uma estrutura que nenhuma migração deixa foi ACEITO{}",
        versao_da_estrutura(),
        if mudou { ", e o arquivo foi alterado" } else { "" }
    );
    assert!(!mudou, "recusou, mas gravou no arquivo (o SHA-256 mudou): {erro:?}");
}
