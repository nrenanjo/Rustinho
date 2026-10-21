//! Testes de estabilidade (antes da versão estável): o que a loja precisa
//! poder confiar — backup, estoque, crediário, DRE, permissões, recuperação,
//! importação e troca de dados. O plano e o que cada teste prova estão em
//! `docs/TESTES-estabilidade.md`; os defeitos achados, em `docs/DEFEITOS.md`.

mod comum;
mod t1_backup;
mod t2_estoque;
mod t3_crediario;
mod t4_dre;
mod t5_permissoes;
mod t6_recuperacao;
mod t7_importacao;
mod t8_sincronizacao;

/// Só à mão: monta a loja de teste (a mesma de `povoar`) numa pasta, para
/// abrir o Rustinho nela e conferir as telas.
/// `RUSTINHO_LOJA_TESTE=/pasta cargo test -p rustinho-nucleo --test estavel -- --ignored montar_loja_de_teste`
#[test]
#[ignore]
fn montar_loja_de_teste() {
    let Some(pasta) = std::env::var_os("RUSTINHO_LOJA_TESTE").map(std::path::PathBuf::from) else { return };
    let mut l = comum::Loja::abrir_em(&pasta, false);
    l.ok(
        "configurar_inicio",
        serde_json::json!({ "loja": "Loja Teste Auto Peças", "caixa": "Caixa 1", "nome": "Ana",
                            "senha": comum::SENHA_ANA, "senha2": comum::SENHA_ANA }),
    );
    comum::povoar(&mut l);
}

/// Só à mão: desativa um usuário da loja de teste por fora, como se a
/// mudança chegasse de outro caixa (para ver a tela voltar à entrada, D-07).
/// `RUSTINHO_LOJA_TESTE=/pasta RUSTINHO_DESATIVAR=Bia cargo test ... -- --ignored desativar_por_fora`
#[test]
#[ignore]
fn desativar_por_fora() {
    let (Some(pasta), Ok(nome)) = (std::env::var_os("RUSTINHO_LOJA_TESTE"), std::env::var("RUSTINHO_DESATIVAR")) else {
        return;
    };
    let b = rustinho_nucleo::banco::Banco::abrir(&std::path::Path::new(&pasta).join("rustinho.db")).unwrap();
    b.conn.execute("UPDATE usuario SET ativo = 0 WHERE nome = ?1", [&nome]).unwrap();
}
