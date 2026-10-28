//! A3 (complemento da 0.9.15): banco com defeito de integridade → modo seguro.
//!
//! Decisão do dono: quando a conferência de integridade (`quick_check`) falha
//! ao abrir — ou nem consegue rodar —, o Rustinho entra em modo seguro: dá
//! para consultar, mas nada comercial é gravado (venda, recebimento,
//! estoque, caixa, cadastros...), a troca de dados não aplica nem entrega
//! nada, o banco original fica como está e os backups bons não são apagados
//! nem trocados por uma cópia possivelmente estragada. A saída é restaurar um
//! backup bom (ou o suporte).

use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

use crate::banco::Banco;
use crate::{backup, config, sync};

fn pasta_nova(nome: &str) -> PathBuf {
    let p =
        std::env::temp_dir().join(format!("rustinho-seguro-{nome}-{}-{}", std::process::id(), crate::tempo::novo_id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn chamar(s: &mut crate::Servico, cmd: &str, d: Value) -> Value {
    serde_json::from_str(&crate::apoio_chave::chamar(crate::api::NAO_REPETIR, cmd, &d.to_string(), |x| {
        s.chamar(cmd, x)
    }))
    .unwrap()
}

fn ok(s: &mut crate::Servico, cmd: &str, d: Value) -> Value {
    let r = chamar(s, cmd, d);
    assert_eq!(r["ok"], true, "{cmd}: {r}");
    r["dados"].clone()
}

pub(crate) fn sha(p: &Path) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(std::fs::read(p).unwrap()).iter().map(|b| format!("{b:02x}")).collect()
}

/// O que há no -wal ao lado do banco (vazio = nada gravado depois do arquivo).
fn tamanho_do_wal(db: &Path) -> u64 {
    let mut w = db.as_os_str().to_owned();
    w.push("-wal");
    std::fs::metadata(PathBuf::from(w)).map(|m| m.len()).unwrap_or(0)
}

struct Loja {
    db: PathBuf,
    peca: String,
    cliente: String,
    parcela: String,
}

/// Loja de verdade num arquivo: gerente Ana (1234), uma peça com estoque, o
/// caixa aberto, um cliente com uma parcela no crediário, a troca de dados
/// ligada e um backup bom feito antes de qualquer defeito.
fn loja_em_arquivo(pasta: &Path) -> Loja {
    let db = pasta.join("rustinho.db");
    let mut s = crate::Servico::abrir_para_teste(&db, false).unwrap();
    ok(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
    );
    let mut f = ok(&mut s, "produto_novo", json!({}));
    f["nome"] = json!("Filtro");
    f["custo"] = json!("10");
    f["estoqueInicial"] = json!("10");
    let peca = ok(&mut s, "produto_salvar", f)["id"].as_str().unwrap().to_string();
    ok(&mut s, "caixa_abrir", json!({ "valor": "100" }));
    let mut c = ok(&mut s, "pessoa_nova", json!({ "papel": "cliente" }));
    c["nome"] = json!("Zé");
    let cliente = ok(&mut s, "pessoa_salvar", c)["id"].as_str().unwrap().to_string();
    ok(
        &mut s,
        "receber_lancar",
        json!({ "clienteId": cliente, "descricao": "caderno", "valor": "50", "parcelas": 1, "primeiroVencimento": "" }),
    );
    let parcela: String =
        s.banco_para_teste().conn.query_row("SELECT id FROM conta_receber", [], |l| l.get(0)).unwrap();
    ok(&mut s, "sync_salvar", json!({ "ligado": true, "senha": "rede-da-loja", "enderecos": "" }));
    drop(s);
    backup::fazer(&db, false).expect("backup bom, antes do defeito");
    Loja { db, peca, cliente, parcela }
}

/// Estraga a lista de páginas livres do banco: o `quick_check` acusa, mas as
/// tabelas continuam legíveis e a cópia (`VACUUM INTO`) ainda sai. É o caso
/// "conferência falha, cópia sai" que faltava testar (PEND-28).
pub(crate) fn estragar_lista_livre(db: &Path) {
    {
        let c = rusqlite::Connection::open(db).unwrap();
        c.execute_batch(
            "PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;
             CREATE TABLE lixo_do_teste (x TEXT);",
        )
        .unwrap();
        for _ in 0..200 {
            c.execute("INSERT INTO lixo_do_teste VALUES (?1)", ["x".repeat(3000)]).unwrap();
        }
        c.execute_batch("DROP TABLE lixo_do_teste;").unwrap();
        let livres: i64 = c.query_row("PRAGMA freelist_count", [], |l| l.get(0)).unwrap();
        assert!(livres > 10, "o teste precisa de páginas livres ({livres})");
    }
    let mut bytes = std::fs::read(db).unwrap();
    let pagina = match u16::from_be_bytes([bytes[16], bytes[17]]) {
        1 => 65536usize,
        n => n as usize,
    };
    let tronco = u32::from_be_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]) as usize;
    assert!(tronco > 0);
    let inicio = (tronco - 1) * pagina;
    bytes[inicio + 4..inicio + 8].copy_from_slice(&5000u32.to_be_bytes());
    std::fs::write(db, &bytes).unwrap();
    let c = rusqlite::Connection::open(db).unwrap();
    let r: String = c.query_row("PRAGMA quick_check", [], |l| l.get(0)).unwrap();
    assert_ne!(r, "ok", "o banco devia estar com defeito");
}

/// Banco com defeito: abre em modo seguro, consulta funciona, entrar
/// funciona, e nenhuma gravação comercial passa — o arquivo do banco não muda.
#[test]
fn a3_banco_com_defeito_entra_em_modo_seguro_e_nao_grava() {
    let pasta = pasta_nova("grava");
    let l = loja_em_arquivo(&pasta);
    estragar_lista_livre(&l.db);
    let antes = sha(&l.db);
    let mut s = crate::Servico::abrir_para_teste(&l.db, false).expect("abre, em modo seguro");
    let e = ok(&mut s, "estado", json!({}));
    assert!(e["modoSeguro"].as_str().is_some_and(|t| !t.is_empty()), "o estado diz que é modo seguro: {e}");
    let bia = s.banco_para_teste().conn.query_row("SELECT id FROM usuario", [], |l| l.get::<_, String>(0)).unwrap();
    ok(&mut s, "entrar", json!({ "id": bia, "senha": "1234" }));
    // Consultas continuam.
    ok(&mut s, "produtos_buscar", json!({ "texto": "filtro" }));
    ok(&mut s, "receber_painel", json!({ "texto": "" }));
    ok(&mut s, "venda_calcular", json!({ "itens": [{ "produtoId": l.peca, "quantidade": "1" }], "tabela": 1 }));
    // Gravações: todas recusadas, com o motivo.
    let tentativas: Vec<(&str, Value)> = vec![
        (
            "venda_concluir",
            json!({ "itens": [{ "produtoId": l.peca, "quantidade": "1" }], "tabela": 1,
                    "pagamentos": [{ "forma": "dinheiro", "valor": "100" }] }),
        ),
        ("receber_baixar", json!({ "clienteId": l.cliente, "contas": [l.parcela], "forma": "dinheiro" })),
        ("estoque_ajustar", json!({ "produtoId": l.peca, "quantidade": "3", "motivo": "contagem" })),
        ("caixa_movimentar", json!({ "tipo": "suprimento", "valor": "10", "motivo": "troco" })),
        ("caixa_fechar", json!({ "contado": "100" })),
        ("pagar_lancar", json!({ "descricao": "Aluguel", "valor": "100", "parcelas": 1, "primeiroVencimento": "" })),
        ("aviso_esconder", json!({ "id": "banco_defeito" })),
        ("computador_salvar", json!({ "caixa": "Caixa 9" })),
    ];
    for (cmd, d) in tentativas {
        let r = chamar(&mut s, cmd, d);
        assert_eq!(r["ok"], false, "{cmd} gravou em modo seguro: {r}");
        assert_eq!(r["campo"], "modoSeguro", "{cmd}: {r}");
    }
    drop(s);
    assert_eq!(sha(&l.db), antes, "o arquivo do banco com defeito não mudou");
    assert_eq!(tamanho_do_wal(&l.db), 0, "nada ficou gravado no -wal");
    let c = rusqlite::Connection::open_with_flags(&l.db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let vendas: i64 = c.query_row("SELECT COUNT(*) FROM venda WHERE tipo = 'venda'", [], |l| l.get(0)).unwrap();
    assert_eq!(vendas, 0);
    let _ = std::fs::remove_dir_all(&pasta);
}

/// Banco com defeito: o programa não liga a troca de dados; e mesmo pedindo
/// direto, este caixa não puxa (não aplica nada) nem entrega nada a outro.
#[test]
fn a3_sem_troca_de_dados_com_banco_com_defeito() {
    thread::spawn(|| {
        let pasta = pasta_nova("troca");
        let l = loja_em_arquivo(&pasta);
        // Um segundo caixa da mesma loja (cópia registrada como caixa novo).
        let gdb = pasta.join("outro").join("rustinho.db");
        std::fs::create_dir_all(gdb.parent().unwrap()).unwrap();
        {
            let a = Banco::abrir(&l.db).unwrap();
            a.conn.backup(rusqlite::MAIN_DB, &gdb, None).unwrap();
        }
        {
            let g = Banco::abrir(&gdb).unwrap();
            config::gravar(&g.conn, "computador_id", &crate::computador::impressao("outro-pc")).unwrap();
            crate::faixas::conferir_computador(&g.conn, crate::computador::atual().as_deref()).unwrap();
            crate::faixas::registrar_caixa_novo(&g.conn, crate::computador::atual().as_deref(), Some("Caixa 2"))
                .unwrap();
        }
        estragar_lista_livre(&l.db);
        let antes = sha(&l.db);
        // O programa abrindo o banco com defeito não liga a troca de dados.
        let s = crate::Servico::abrir_para_teste(&l.db, true).unwrap();
        assert!(!s.tem_motor(), "modo seguro: sem troca de dados");
        drop(s);
        // Pedindo direto: este caixa não puxa do outro.
        // (Portas escolhidas pelo sistema, C8; o teste espera elas abrirem.)
        let mut mg = sync::Motor::iniciar(gdb.clone(), 0, 0);
        let (pt_g, _) = mg.portas(Duration::from_secs(10)).expect("as portas do outro caixa abriram");
        let r = sync::puxar(&l.db, &format!("127.0.0.1:{pt_g}"), 1);
        // C8: recusado pelo modo seguro (não por falta de rede).
        let e = r.expect_err("o caixa com defeito puxou").to_string();
        assert!(e.contains("modo seguro"), "{e}");
        assert!(mg.encerrar(Duration::from_secs(20)));
        // Nem entrega: o outro caixa não recebe nada dele.
        let antes_g: i64 = {
            let g = rusqlite::Connection::open(&gdb).unwrap();
            g.query_row("SELECT COUNT(*) FROM alteracao", [], |l| l.get(0)).unwrap()
        };
        // O motor do banco com defeito abre a porta, mas não entrega nada.
        let mut md = sync::Motor::iniciar(l.db.clone(), 0, 0);
        let (pt_d, _) = md.portas(Duration::from_secs(10)).expect("as portas do caixa com defeito abriram");
        let r = sync::puxar(&gdb, &format!("127.0.0.1:{pt_d}"), pt_g);
        assert!(md.encerrar(Duration::from_secs(20)));
        match &r {
            Ok(x) => assert_eq!(x.1, 0, "o banco com defeito entregou alterações: {r:?}"),
            // C8: a conexão chegou ao servidor (não é "Connection refused") e
            // ele encerrou sem entregar nada.
            Err(e) => assert!(
                !e.to_string().contains("Connection refused") && !e.to_string().contains("os error 111"),
                "o servidor do caixa com defeito tinha de estar no ar: {e}"
            ),
        }
        let depois_g: i64 = {
            let g = rusqlite::Connection::open(&gdb).unwrap();
            g.query_row("SELECT COUNT(*) FROM alteracao", [], |l| l.get(0)).unwrap()
        };
        assert_eq!(antes_g, depois_g, "o outro caixa não recebeu nada");
        assert_eq!(sha(&l.db), antes, "o banco com defeito não mudou");
        let _ = std::fs::remove_dir_all(&pasta);
    })
    .join()
    .unwrap();
}

/// Backup de um banco com defeito: sai como erro, a cópia (que pode estar
/// incompleta) fica com nome próprio de cópia para o suporte — não aparece
/// para restaurar, não conta na retenção e nunca a retenção a apaga — e os
/// backups bons ficam. O backup automático e o de fechar não rodam.
#[test]
fn a3_backup_de_banco_com_defeito_nao_vira_backup_bom() {
    let pasta = pasta_nova("backup");
    let l = loja_em_arquivo(&pasta);
    let backups = pasta.join("backups");
    let bons: Vec<String> = backup::listar(&backups).into_iter().map(|c| c.nome).collect();
    assert_eq!(bons.len(), 1, "{bons:?}");
    estragar_lista_livre(&l.db);
    let antes = sha(&l.db);
    {
        // O programa abriu o banco (e achou o defeito).
        let _s = crate::Servico::abrir_para_teste(&l.db, false).unwrap();
        let e = backup::fazer(&l.db, false).expect_err("backup de banco com defeito sai como erro");
        assert!(e.to_string().contains("defeito"), "{e}");
    }
    let nomes: Vec<String> = std::fs::read_dir(&backups)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| !n.starts_with('.'))
        .collect();
    let suporte: Vec<&String> = nomes.iter().filter(|n| !bons.contains(n)).collect();
    assert_eq!(suporte.len(), 1, "uma cópia para o suporte: {nomes:?}");
    assert!(suporte[0].contains("defeito"), "o nome diz que é de um banco com defeito: {}", suporte[0]);
    assert!(backup::ler_nome(suporte[0]).is_none(), "não é um backup comum (não entra na lista nem na retenção)");
    let lista: Vec<String> = backup::listar(&backups).into_iter().map(|c| c.nome).collect();
    assert_eq!(lista, bons, "para restaurar, só os backups bons");
    // Muitos dias depois, com vários backups novos: a retenção nunca conta
    // nem apaga a cópia do suporte.
    let mut todos = nomes.clone();
    for d in 10..=28 {
        todos.push(format!("rustinho-209911{d}-120000-caixa1.rbk"));
    }
    let apagar = backup::para_apagar(&todos, "20991128", 7);
    assert!(!apagar.contains(suporte[0]), "a retenção apagaria a cópia do suporte");
    // Automático e ao fechar: nada.
    backup::automatico(l.db.clone());
    thread::sleep(Duration::from_secs(2));
    assert!(!backup::ao_fechar(&l.db).unwrap_or(false), "backup ao fechar não roda em modo seguro");
    let depois: usize = std::fs::read_dir(&backups)
        .unwrap()
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .count();
    assert_eq!(depois, nomes.len(), "nenhuma cópia nova");
    for b in &bons {
        assert!(backups.join(b).exists(), "o backup bom {b} ficou");
    }
    assert_eq!(sha(&l.db), antes, "o banco com defeito não mudou");
    let _ = std::fs::remove_dir_all(&pasta);
}

/// A saída do modo seguro: o gerente restaura um backup bom. A cópia de antes
/// (do banco com defeito) fica guardada como cópia para o suporte; ao abrir
/// de novo, o banco é conferido e o caixa volta a vender.
#[test]
fn a3_restaurar_backup_bom_tira_do_modo_seguro() {
    let pasta = pasta_nova("restaurar");
    let l = loja_em_arquivo(&pasta);
    let backups = pasta.join("backups");
    let bom = backup::listar(&backups).into_iter().next().expect("backup bom").caminho;
    estragar_lista_livre(&l.db);
    {
        let mut s = crate::Servico::abrir_para_teste(&l.db, false).unwrap();
        let ana = s.banco_para_teste().conn.query_row("SELECT id FROM usuario", [], |l| l.get::<_, String>(0)).unwrap();
        ok(&mut s, "entrar", json!({ "id": ana, "senha": "1234" }));
        // Como na tela: o gerente digita a senha dele para abrir o backup.
        let r = ok(&mut s, "backup_restaurar", json!({ "arquivo": bom, "nuvem": false, "senha": "1234" }));
        assert_eq!(r["reabrir"], true, "pede para fechar e abrir: {r}");
        // Até fechar e abrir, continua sem gravar.
        let ana = s.banco_para_teste().conn.query_row("SELECT id FROM usuario", [], |l| l.get::<_, String>(0)).unwrap();
        ok(&mut s, "entrar", json!({ "id": ana, "senha": "1234" }));
        let r = chamar(&mut s, "caixa_movimentar", json!({ "tipo": "suprimento", "valor": "10", "motivo": "troco" }));
        assert_eq!(r["campo"], "modoSeguro", "{r}");
    }
    let suporte: Vec<String> = std::fs::read_dir(&backups)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.contains("defeito") && n.contains("antes"))
        .collect();
    assert_eq!(suporte.len(), 1, "a cópia de antes de restaurar (do banco com defeito) ficou: {suporte:?}");
    let mut s = crate::Servico::abrir_para_teste(&l.db, false).unwrap();
    let e = ok(&mut s, "estado", json!({}));
    assert!(e["modoSeguro"].is_null(), "banco restaurado e conferido: fora do modo seguro: {e}");
    let ana = s.banco_para_teste().conn.query_row("SELECT id FROM usuario", [], |l| l.get::<_, String>(0)).unwrap();
    ok(&mut s, "entrar", json!({ "id": ana, "senha": "1234" }));
    ok(
        &mut s,
        "venda_concluir",
        json!({ "itens": [{ "produtoId": l.peca, "quantidade": "1" }], "tabela": 1,
                "pagamentos": [{ "forma": "dinheiro", "valor": "100" }] }),
    );
    let _ = std::fs::remove_dir_all(&pasta);
}

/// Defeito numa página de dados: uma consulta que não consegue ler devolve
/// um erro com mensagem (sem derrubar o programa); o modo seguro vale.
#[test]
fn a3_consulta_que_nao_le_devolve_erro_sem_derrubar() {
    let pasta = pasta_nova("leitura");
    let l = loja_em_arquivo(&pasta);
    {
        let c = rusqlite::Connection::open(&l.db).unwrap();
        crate::banco::registrar_funcoes(&c).unwrap();
        c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;").unwrap();
        for i in 0..3000 {
            c.execute(
                "INSERT INTO auditoria (id, quando, dispositivo, usuario_id, acao, entidade, entidade_id, detalhes)
                 VALUES (?1, '2026-10-01', 'd', NULL, 'teste', 'teste', ?1, ?2)",
                rusqlite::params![format!("a{i}"), "x".repeat(200)],
            )
            .unwrap();
        }
    }
    let pagina: u64 = {
        let c = rusqlite::Connection::open(&l.db).unwrap();
        c.query_row("PRAGMA page_size", [], |l| l.get::<_, i64>(0)).unwrap() as u64
    };
    let tamanho = std::fs::metadata(&l.db).unwrap().len();
    {
        use std::io::{Seek, SeekFrom, Write};
        let mut f = std::fs::OpenOptions::new().write(true).open(&l.db).unwrap();
        f.seek(SeekFrom::Start(tamanho - 3 * pagina)).unwrap();
        f.write_all(&[0xFFu8; 16]).unwrap();
    }
    let mut s = crate::Servico::abrir_para_teste(&l.db, false).unwrap();
    let ana = s.banco_para_teste().conn.query_row("SELECT id FROM usuario", [], |l| l.get::<_, String>(0)).unwrap();
    ok(&mut s, "entrar", json!({ "id": ana, "senha": "1234" }));
    let r = chamar(&mut s, "auditoria_listar", json!({}));
    if r["ok"] == false {
        assert!(r["erro"].as_str().is_some_and(|t| !t.is_empty()), "erro com mensagem: {r}");
    }
    let r = chamar(&mut s, "caixa_movimentar", json!({ "tipo": "suprimento", "valor": "10", "motivo": "troco" }));
    assert_eq!(r["campo"], "modoSeguro", "{r}");
    let _ = std::fs::remove_dir_all(&pasta);
}

/// R-07 (C8): um backup feito com a `zip` 2.4 (versões anteriores do
/// Rustinho) abre e restaura com a `zip` 9. Só à mão, com a pasta do backup
/// antigo em `C8_BACKUP_ZIP2` (a loja do `loja_em_arquivo`, feita pela versão
/// de antes).
#[test]
#[ignore]
fn c8_backup_da_zip2_restaura_com_a_zip9() {
    let Some(pasta) = std::env::var_os("C8_BACKUP_ZIP2").map(PathBuf::from) else { return };
    let antigo = backup::listar(&pasta.join("backups")).into_iter().next().expect("backup antigo").caminho;
    let db = pasta.join("rustinho.db");
    let mut s = crate::Servico::abrir_para_teste(&db, false).unwrap();
    let ana = s.banco_para_teste().conn.query_row("SELECT id FROM usuario", [], |l| l.get::<_, String>(0)).unwrap();
    ok(&mut s, "entrar", json!({ "id": ana, "senha": "1234" }));
    ok(&mut s, "backup_conferir", json!({ "arquivo": antigo, "senha": "1234" }));
    // Uma peça nova depois do backup: restaurar a tira (volta ao backup).
    let mut f = ok(&mut s, "produto_novo", json!({}));
    f["nome"] = json!("Depois do backup");
    f["custo"] = json!("1");
    ok(&mut s, "produto_salvar", f);
    let r = ok(&mut s, "backup_restaurar", json!({ "arquivo": antigo, "nuvem": false, "senha": "1234" }));
    eprintln!("C8 restaurar backup da zip 2.4: {r}");
    drop(s);
    let s = crate::Servico::abrir_para_teste(&db, false).unwrap();
    let c = |sql: &str| -> i64 { rusqlite::Connection::open(&db).unwrap().query_row(sql, [], |l| l.get(0)).unwrap() };
    assert_eq!(c("SELECT COUNT(*) FROM produto WHERE nome = 'Filtro'"), 1, "a peça do backup voltou");
    assert_eq!(c("SELECT COUNT(*) FROM produto WHERE nome = 'Depois do backup'"), 0, "o que veio depois saiu");
    assert_eq!(c("SELECT COUNT(*) FROM pessoa WHERE nome = 'Zé'"), 1);
    drop(s);
}
