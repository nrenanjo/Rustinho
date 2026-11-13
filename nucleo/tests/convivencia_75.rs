//! Passo 7.5 com o 5.2 (DP-15, D75-1): a coluna nova de `produto`
//! (`nome_normalizado`, migração 0026, provisória) é derivada da peça e NÃO
//! viaja: está em `dados::esquema::COLUNAS` (`Derivado::IndiceDaPeca`), os
//! gatilhos da troca saem sem ela, quem recebe a ignora e a refaz
//! (`catalogo::codigos::refazer_indice`), como `busca` e `codigos`.
//!
//! Na preparação (ramo `prod-f75`, sem a 5.2) estes testes eram outros e
//! fixavam o comportamento de então; na integração eles caíram de propósito
//! com a D75-1 e foram trocados (registro em `RELATORIO_INTEGRACAO.md`):
//!
//! - `a_coluna_nova_vai_no_log_da_troca_ate_a_5_2` (a coluna ia no log) virou
//!   [`a_coluna_nova_nao_vai_no_log_da_troca`];
//! - `peca_de_um_caixa_anterior_entra_sem_a_coluna_nova` (a coluna ficava
//!   vazia em quem recebia) virou [`quem_recebe_refaz_a_coluna_nova`];
//! - `caixa_da_estrutura_24_recebe_a_peca_com_a_coluna_nova` (o código de
//!   recebimento de hoje num banco da estrutura 24) saiu: com a 5.2 um caixa
//!   de versão anterior não troca dados com este (`classes-v1`, teste
//!   `troca::sync::classes_caixa_de_versao_anterior_nao_troca_dados`), e o
//!   programa atualiza o banco ao abrir, antes de qualquer troca, então o
//!   código de hoje nunca recebe num banco da estrutura 24. O que ele
//!   conferia do lado de quem recebe (a coluna que o caixa não tem é
//!   ignorada) ficou em [`coluna_que_o_caixa_nao_tem_e_ignorada_ao_receber`].

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
    json!({ "id": id, "codigo": format!("C{id}"), "nome": nome, "custo": 1000, "ativo": 1,
            "criado_em": "2026-11-13T10:00:00Z", "atualizado_em": "2026-11-13T10:00:00Z",
            "dispositivo": "caixa-outro" })
}

fn um<T: rusqlite::types::FromSql>(c: &Connection, sql: &str) -> T {
    c.query_row(sql, [], |l| l.get(0)).unwrap()
}

/// O caixa que salva a peça grava o nome sem acento (o dono, `produtos`), mas
/// a coluna não vai no conteúdo da alteração, e mudar só ela não gera
/// alteração (o gatilho é `UPDATE OF` só das colunas que viajam).
#[test]
fn a_coluna_nova_nao_vai_no_log_da_troca() {
    let b = Banco::em_memoria().unwrap();
    rustinho_nucleo::faixas::assumir(&b.conn).unwrap();
    let u = rustinho_nucleo::usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap().id;
    let mut f = rustinho_nucleo::produtos::ficha_nova(&b.conn).unwrap();
    f.nome = "Ácido".into();
    f.custo = "10".into();
    let id = rustinho_nucleo::produtos::salvar(&b.conn, &b.dispositivo, &u, &f).unwrap();
    let gravado: String =
        b.conn.query_row("SELECT nome_normalizado FROM produto WHERE id = ?1", [&id], |l| l.get(0)).unwrap();
    assert_eq!(gravado, "acido", "o dono grava a coluna ao salvar");
    let dados: String = b
        .conn
        .query_row(
            "SELECT dados FROM alteracao WHERE tabela = 'produto' AND chave = json_array(?1) ORDER BY ordem DESC LIMIT 1",
            [&id],
            |l| l.get(0),
        )
        .unwrap();
    let v: Value = serde_json::from_str(&dados).unwrap();
    assert_eq!(v["nome"], "Ácido", "a peça vai no log");
    assert!(v.get("nome_normalizado").is_none(), "a coluna derivada foi no log: {dados}");

    let antes: i64 = um(&b.conn, "SELECT COUNT(*) FROM alteracao");
    b.conn.execute("UPDATE produto SET nome_normalizado = 'outra coisa' WHERE id = ?1", [&id]).unwrap();
    let depois: i64 = um(&b.conn, "SELECT COUNT(*) FROM alteracao");
    assert_eq!(depois, antes, "mudar só a coluna derivada gerou alteração");
}

/// Quem recebe a peça refaz o nome sem acento a partir do nome que chegou: a
/// peça que vem sem a coluna (a regra da troca) e a que viesse com um valor
/// qualquer nela (ignorado) ficam com o mesmo valor que o dono gravaria.
#[test]
fn quem_recebe_refaz_a_coluna_nova() {
    let mut b = Banco::em_memoria().unwrap();
    let mut com_valor_errado = peca("p2", "Água de bateria");
    com_valor_errado["nome_normalizado"] = json!("valor que não sai do nome");
    let n = sync::aplicar(
        &mut b.conn,
        &[
            alteracao("01990000-0000-7000-8000-0000000000a1", peca("p1", "Óleo do caixa vizinho")),
            alteracao("01990000-0000-7000-8000-0000000000a2", com_valor_errado),
        ],
    )
    .unwrap();
    assert_eq!(n, 2);
    let p1: String = um(&b.conn, "SELECT nome_normalizado FROM produto WHERE id = 'p1'");
    let p2: String = um(&b.conn, "SELECT nome_normalizado FROM produto WHERE id = 'p2'");
    assert_eq!(p1, "oleo do caixa vizinho");
    assert_eq!(p2, "agua de bateria");
    let pendentes: i64 = um(&b.conn, "SELECT COUNT(*) FROM sync_pendencia");
    assert_eq!(pendentes, 0, "nada ficou pendente");
    let conflitos: i64 = um(&b.conn, "SELECT COUNT(*) FROM sync_conflito");
    assert_eq!(conflitos, 0, "a coluna derivada não conta como conflito");
}

/// Uma coluna que este caixa não tem (de uma versão de estrutura mais nova)
/// é ignorada ao receber: a peça entra e nada fica pendente. É o lado de quem
/// recebe que o teste da estrutura 24 conferia na preparação.
#[test]
fn coluna_que_o_caixa_nao_tem_e_ignorada_ao_receber() {
    let mut b = Banco::em_memoria().unwrap();
    let mut dados = peca("p3", "Correia do caixa novo");
    dados["coluna_de_uma_versao_futura"] = json!("qualquer");
    let n = sync::aplicar(&mut b.conn, &[alteracao("01990000-0000-7000-8000-0000000000a3", dados)]).unwrap();
    assert_eq!(n, 1);
    let nome: String = um(&b.conn, "SELECT nome FROM produto WHERE id = 'p3'");
    assert_eq!(nome, "Correia do caixa novo");
    let pendentes: i64 = um(&b.conn, "SELECT COUNT(*) FROM sync_pendencia");
    assert_eq!(pendentes, 0, "nada ficou pendente");
}
