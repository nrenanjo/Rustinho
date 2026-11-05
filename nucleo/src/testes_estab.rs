//! Frente estab (0.9.18): estabilidade de backup e restauração.
//!
//! - Restaurar do Drive: baixar e conferir o backup (que pode levar até meia
//!   hora) não param a troca de dados; ela para só em volta da troca do banco.
//! - Queda (falta de luz) antes da troca do banco, depois da troca e depois de
//!   terminar: ao abrir de novo, o banco fica certo e nada é feito duas vezes.
//! - M-04 (auditoria 0.9.14): o registro "restaurou um backup" vai na mesma
//!   transação que termina a restauração (também quando ela termina ao abrir).
//! - Erro depois de trocar o banco (antes de pôr de volta o que é deste
//!   computador): a troca de dados não roda com o banco pela metade e o
//!   serviço pede para fechar e abrir (a restauração termina ao abrir).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rusqlite::Connection;
use serde_json::{json, Value};

use crate::backup::{self, queda_de_teste};

fn pasta_nova(nome: &str) -> PathBuf {
    let p =
        std::env::temp_dir().join(format!("rustinho-estab-{nome}-{}-{}", std::process::id(), crate::tempo::novo_id()));
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

fn peca(s: &mut crate::Servico, nome: &str) {
    let mut f = ok(s, "produto_novo", json!({}));
    f["nome"] = json!(nome);
    f["custo"] = json!("10");
    f["estoqueInicial"] = json!("3");
    ok(s, "produto_salvar", f);
}

/// Uma loja num arquivo e um backup dela.
struct Loja {
    pasta: PathBuf,
    db: PathBuf,
    ana: String,
    backup: String,
}

/// Gerente Ana (1234), "Caixa 1", a peça "Antes do backup", a troca de dados
/// ligada e um outro caixa conhecido (contador 7); o backup; e, depois dele,
/// a peça "Depois do backup" e o nome deste caixa trocado para "Caixa Nova"
/// (é deste computador: tem de ficar depois de restaurar).
fn loja(nome: &str) -> Loja {
    let pasta = pasta_nova(nome);
    let db = pasta.join("rustinho.db");
    let mut s = crate::Servico::abrir_para_teste(&db, false).unwrap();
    ok(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
    );
    peca(&mut s, "Antes do backup");
    ok(&mut s, "sync_salvar", json!({ "ligado": true, "senha": "rede-da-loja", "enderecos": "" }));
    s.banco_para_teste()
        .conn
        .execute(
            "INSERT INTO sync_par (dispositivo, nome, endereco, cursor) VALUES ('outro', 'Caixa 2', '10.0.0.2:47810', 7)",
            [],
        )
        .unwrap();
    let ana: String = s.banco_para_teste().conn.query_row("SELECT id FROM usuario", [], |l| l.get(0)).unwrap();
    drop(s);
    let feito = backup::fazer(&db, false).expect("backup");
    let mut s = crate::Servico::abrir_para_teste(&db, false).unwrap();
    ok(&mut s, "entrar", json!({ "id": ana, "senha": "1234" }));
    peca(&mut s, "Depois do backup");
    crate::config::gravar(&s.banco_para_teste().conn, "caixa_nome", "Caixa Nova").unwrap();
    drop(s);
    Loja { pasta, db, ana, backup: feito.arquivo }
}

fn ler(db: &Path, chave: &str) -> Option<String> {
    let c = Connection::open(db).unwrap();
    c.query_row("SELECT valor FROM config WHERE chave = ?1", [chave], |l| l.get(0)).ok()
}

fn numero(db: &Path, sql: &str) -> i64 {
    Connection::open(db).unwrap().query_row(sql, [], |l| l.get(0)).unwrap()
}

fn tem_peca(db: &Path, nome: &str) -> bool {
    numero(db, &format!("SELECT COUNT(*) FROM produto WHERE nome = '{nome}'")) > 0
}

/// O banco inteiro e sem referência quebrada.
fn conferir_banco(db: &Path) {
    let c = Connection::open(db).unwrap();
    let ok: String = c.query_row("PRAGMA integrity_check", [], |x| x.get(0)).unwrap();
    assert_eq!(ok, "ok");
    let chaves: i64 = c.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |x| x.get(0)).unwrap();
    assert_eq!(chaves, 0, "referência quebrada");
}

/// Item 2: restaurar uma cópia do Google Drive. O download (aqui, um rclone
/// que leva 3 s para "baixar") NÃO para a troca de dados com os outros caixas
/// (antes, ela ficava parada durante todo o download, que pode levar até meia
/// hora); na hora de trocar o banco, ela está parada.
#[cfg(unix)]
#[test]
fn estab_baixar_do_drive_nao_para_a_troca_de_dados() {
    use std::os::unix::fs::PermissionsExt;
    let _vez = backup::testes::um_por_vez();
    if !backup::rclone_instalado() {
        eprintln!("rclone não instalado: teste pulado");
        return;
    }
    let l = loja("drive-pausa");
    let nuvem = l.pasta.join("drive");
    std::fs::create_dir_all(&nuvem).unwrap();
    let nome = Path::new(&l.backup).file_name().unwrap().to_string_lossy().to_string();
    std::fs::copy(&l.backup, nuvem.join(&nome)).unwrap();
    let lento = l.pasta.join("rclone-lento.sh");
    std::fs::write(&lento, "#!/bin/sh\nif [ \"$1\" = copyto ]; then sleep 3; fi\nexec rclone \"$@\"\n").unwrap();
    std::fs::set_permissions(&lento, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var("RUSTINHO_REMOTO", nuvem.display().to_string());
    std::env::set_var("RUSTINHO_RCLONE", lento.display().to_string());
    let db = l.db.clone();
    let tarefa = std::thread::spawn(move || {
        let mut b = crate::banco::Banco::abrir(&db).unwrap();
        let r = backup::restaurar(&mut b.conn, Some(&db), &nome, true, "1234");
        (r.map(|_| ()).map_err(|e| e.to_string()), queda_de_teste::PAUSADA_NA_TROCA.with(|p| p.get()))
    });
    let inicio = Instant::now();
    while backup::fazendo() != "Baixando do Google Drive…" {
        assert!(inicio.elapsed() < Duration::from_secs(60), "não começou a baixar");
        std::thread::sleep(Duration::from_millis(20));
    }
    // Durante o download, a troca de dados consegue a vez dela.
    let (avisa, espera) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _t = crate::sync::travar();
        let _ = avisa.send(());
    });
    let livre = espera.recv_timeout(Duration::from_millis(1500)).is_ok();
    let (r, pausada) = tarefa.join().unwrap();
    std::env::remove_var("RUSTINHO_RCLONE");
    std::env::remove_var("RUSTINHO_REMOTO");
    r.unwrap();
    assert!(livre, "a troca de dados ficou parada durante o download do Drive");
    assert_eq!(pausada, Some(true), "na troca do banco, a troca de dados estava parada");
    assert!(!tem_peca(&l.db, "Depois do backup") && tem_peca(&l.db, "Antes do backup"));
    assert_eq!(ler(&l.db, "caixa_nome").as_deref(), Some("Caixa Nova"));
    conferir_banco(&l.db);
    let _ = std::fs::remove_dir_all(&l.pasta);
}
