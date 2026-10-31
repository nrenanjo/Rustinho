//! Testes de estabilidade (antes da versão estável): o que a loja precisa
//! poder confiar — backup, estoque, crediário, DRE, permissões, recuperação,
//! importação e troca de dados. O plano e o que cada teste prova estão em
//! `docs/TESTES-estabilidade.md`; os defeitos achados, em `docs/DEFEITOS.md`.

mod a1_auditoria;
#[path = "../apoio/chave.rs"]
mod apoio_chave;
mod comum;
mod d09_custos;
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
    let povo = comum::povoar(&mut l);
    // RVI-02 (app/testes/telas_rvi02.py): a mesma loja com a troca de dados ligada.
    if std::env::var_os("RUSTINHO_LOJA_TROCA").is_some() {
        l.ok("sync_salvar", serde_json::json!({ "ligado": true, "senha": "rede-da-loja-teste", "enderecos": "" }));
    }
    // Notificações (app/testes/telas_notificacoes.py) no Início: informativas
    // (parcela vencida, conta a pagar vencida, encomenda em aberto), atenção
    // (diferença entre caixas) e duas críticas (computador de outra loja
    // recusado; banco que veio de outro computador — o Rustinho confere ao
    // abrir e bloqueia os códigos).
    if std::env::var_os("RUSTINHO_LOJA_AVISOS").is_some() {
        l.ok(
            "receber_lancar",
            serde_json::json!({ "clienteId": povo.ze, "descricao": "Caderno", "valor": "50", "parcelas": 1,
                                "primeiroVencimento": "" }),
        );
        l.ok(
            "pagar_lancar",
            serde_json::json!({ "categoriaId": "cat-aluguel", "descricao": "Aluguel de novembro", "valor": "300",
                                "parcelas": 1, "competencia": "", "fornecedorId": povo.fornecedor }),
        );
        l.ok(
            "encomenda_criar",
            serde_json::json!({ "clienteNome": "Maria Souza", "telefone": "(15) 99808-0000",
                                "descricao": "Pastilha dianteira", "quantidade": "1" }),
        );
        let recusa = serde_json::json!({ "nome": "Caixa da outra loja", "endereco": "192.168.0.9",
                                         "quando": rustinho_nucleo::tempo::agora() });
        l.executar(&format!(
            "UPDATE conta_receber SET vencimento = date('now', '-3 days');
             UPDATE conta_pagar SET vencimento = date('now', '-2 days');
             UPDATE config SET valor = 'outro-computador' WHERE chave = 'computador_id';
             INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando)
             VALUES ('produto', '{}', 'v1', 'v2', 'caixa-2', datetime('now'));
             INSERT OR REPLACE INTO config (chave, valor) VALUES ('{}', '{}');",
            povo.filtro,
            rustinho_nucleo::loja::OUTRA_LOJA,
            recusa
        ));
    }
}

/// Só à mão: muda a loja de teste por fora, como num banco antigo ou
/// restaurado (para as telas da navegação guiada, 0.9.17). Pelo núcleo, para
/// os gatilhos da troca de dados terem as funções do Rustinho.
/// `RUSTINHO_LOJA_TESTE=/pasta RUSTINHO_SQL="UPDATE config ..." cargo test ... -- --ignored executar_por_fora`
#[test]
#[ignore]
fn executar_por_fora() {
    let (Some(pasta), Ok(sql)) = (std::env::var_os("RUSTINHO_LOJA_TESTE"), std::env::var("RUSTINHO_SQL")) else {
        return;
    };
    let b = rustinho_nucleo::banco::Banco::abrir(&std::path::Path::new(&pasta).join("rustinho.db")).unwrap();
    b.conn.execute_batch(&sql).unwrap();
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
