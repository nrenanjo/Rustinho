//! Erro do banco na tela (dev.3.3, passo 2.4: G02).
//!
//! Um erro do SQLite ("no such table: …", "database is locked") não vai cru
//! para a tela: a pessoa recebe uma mensagem curta e o detalhe técnico fica
//! no registro (rustinho.log), para o suporte.
//!
//! Dados de mentira.

#[path = "apoio/chave.rs"]
mod apoio_chave;
#[path = "estavel/comum.rs"]
mod comum;

use comum::*;
use serde_json::json;

/// G02: com uma tabela apagada de propósito, o comando falha com a mensagem
/// curta, sem o texto do SQLite.
#[test]
fn g02_erro_do_banco_chega_curto_na_tela() {
    let mut l = Loja::nova("erro-do-banco");
    l.ok("categorias", json!({}));
    l.executar("DROP TABLE categoria");
    let r = l.chamar("categorias", json!({}));
    assert_eq!(r["ok"], false, "{r}");
    let erro = r["erro"].as_str().unwrap();
    assert!(
        !erro.contains("no such table") && !erro.contains("categoria"),
        "o erro do SQLite foi cru para a tela: {erro}"
    );
    assert!(erro.contains("registro técnico"), "a mensagem não diz onde ficou o detalhe: {erro}");
}
