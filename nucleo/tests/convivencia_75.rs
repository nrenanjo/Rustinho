//! Passo 7.5 (DP-15): a coluna nova de `produto` (`nome_normalizado`, migração
//! 0025, provisória) e a convivência com caixas de versão anterior na troca de
//! dados. Lido no código da troca (`troca/sync/registro.rs` e `aplicacao.rs`):
//!
//! - quem manda: os gatilhos gravam no log TODAS as colunas da tabela (as que a
//!   tabela tem quando o programa abre), então a coluna nova vai junto, como
//!   `busca` e `codigos` vão hoje (até o passo 5.2, que aplica a DP-15);
//! - quem recebe grava só as colunas que existem nele ("um caixa com versão mais
//!   nova pode ter mais", `aplicacao.rs`), e a matriz de incompatibilidade só
//!   olha tabela desconhecida e o rascunho de OS: nenhum dos dois lados recusa.
//!
//! Estes testes provam os dois sentidos com o código de recebimento, que é o
//! mesmo de `b3a171c` (a 7.5 não mexe na troca).

use rusqlite::Connection;
use rustinho_nucleo::banco::Banco;
use rustinho_nucleo::sync::{self, Alteracao};
use serde_json::{json, Value};

fn alteracao(id: &str, dados: Value) -> Alteracao {
    Alteracao {
        seq: 1,
        id: id.into(),
        tabela: "produto".into(),
        chave: json!([dados["id"]]).to_string(),
        operacao: "gravar".into(),
        dados: Some(dados.to_string()),
        dispositivo: "caixa-outro".into(),
        quando: "2026-11-13T10:00:00.000Z".into(),
        base: None,
        ordem: 1,
        historia: None,
        compactada: false,
        operacao_id: None,
        incompativel: None,
    }
}

/// Uma peça como a troca manda (as colunas obrigatórias e algumas outras).
fn peca(id: &str, nome: &str) -> Value {
    json!({ "id": id, "codigo": format!("C{id}"), "nome": nome, "busca": nome.to_lowercase(),
            "codigos": format!(" c{id} "), "custo": 1000, "ativo": 1, "criado_em": "2026-11-13T10:00:00Z",
            "atualizado_em": "2026-11-13T10:00:00Z", "dispositivo": "caixa-outro" })
}

fn um(c: &Connection, sql: &str) -> String {
    c.query_row(sql, [], |l| l.get(0)).unwrap()
}

/// Um caixa desta versão (0025) recebe a peça de um caixa da estrutura 24 (sem
/// a coluna nova): grava; a coluna derivada fica vazia (e a peça só desempata
/// fora do lugar na lista "alteradas por último") até a 5.2 refazer as colunas
/// derivadas ao receber. Este teste fixa o comportamento de hoje.
#[test]
fn peca_de_um_caixa_anterior_entra_sem_a_coluna_nova() {
    let mut b = Banco::em_memoria().unwrap();
    let n = sync::aplicar(
        &mut b.conn,
        &[alteracao("01990000-0000-7000-8000-0000000000a1", peca("p24", "Óleo do caixa 24"))],
    )
    .unwrap();
    assert_eq!(n, 1);
    assert_eq!(um(&b.conn, "SELECT nome FROM produto WHERE id = 'p24'"), "Óleo do caixa 24");
    assert_eq!(um(&b.conn, "SELECT nome_normalizado FROM produto WHERE id = 'p24'"), "");
}

/// Um caixa da estrutura 24 (montada pelas migrações 0001 a 0024, com o código
/// de recebimento de hoje, que é o de `b3a171c`) recebe a peça de um caixa da
/// 0025, com `nome_normalizado`: grava a peça e ignora a coluna que não tem.
#[test]
fn caixa_da_estrutura_24_recebe_a_peca_com_a_coluna_nova() {
    let raiz = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/migracoes");
    let mut arquivos: Vec<_> = std::fs::read_dir(&raiz).unwrap().flatten().map(|e| e.path()).collect();
    arquivos.sort();
    let mut c = Connection::open_in_memory().unwrap();
    rustinho_nucleo::banco::registrar_funcoes(&c).unwrap();
    for sql in arquivos.iter().take(24).map(|p| std::fs::read_to_string(p).unwrap()) {
        let sem = sql.starts_with("-- rustinho: sem_chaves_estrangeiras");
        c.pragma_update(None, "foreign_keys", !sem).unwrap();
        c.pragma_update(None, "legacy_alter_table", sem).unwrap();
        c.execute_batch(&sql).unwrap();
    }
    c.pragma_update(None, "foreign_keys", true).unwrap();
    c.pragma_update(None, "legacy_alter_table", false).unwrap();
    c.pragma_update(None, "user_version", 24).unwrap();
    let colunas = um(&c, "SELECT group_concat(name) FROM pragma_table_info('produto')");
    assert!(!colunas.contains("nome_normalizado"), "a estrutura 24 não tem a coluna");
    let mut dados = peca("p25", "Água do caixa 25");
    dados["nome_normalizado"] = json!("agua do caixa 25");
    let n = sync::aplicar(&mut c, &[alteracao("01990000-0000-7000-8000-0000000000a2", dados)]).unwrap();
    assert_eq!(n, 1);
    assert_eq!(um(&c, "SELECT nome FROM produto WHERE id = 'p25'"), "Água do caixa 25");
    let pendentes: i64 = c.query_row("SELECT COUNT(*) FROM sync_pendencia", [], |l| l.get(0)).unwrap();
    assert_eq!(pendentes, 0, "nada ficou pendente");
}

/// E um caixa desta versão manda a coluna nova (até a 5.2): o gatilho grava
/// todas as colunas da peça no log da troca.
#[test]
fn a_coluna_nova_vai_no_log_da_troca_ate_a_5_2() {
    let b = Banco::em_memoria().unwrap();
    rustinho_nucleo::faixas::assumir(&b.conn).unwrap();
    let u = rustinho_nucleo::usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap().id;
    let mut f = rustinho_nucleo::produtos::ficha_nova(&b.conn).unwrap();
    f.nome = "Ácido".into();
    f.custo = "10".into();
    let id = rustinho_nucleo::produtos::salvar(&b.conn, &b.dispositivo, &u, &f).unwrap();
    let dados: String = b
        .conn
        .query_row(
            "SELECT dados FROM alteracao WHERE tabela = 'produto' AND chave = json_array(?1) ORDER BY ordem DESC LIMIT 1",
            [&id],
            |l| l.get(0),
        )
        .unwrap();
    let v: Value = serde_json::from_str(&dados).unwrap();
    assert_eq!(v["nome_normalizado"], "acido");
}
