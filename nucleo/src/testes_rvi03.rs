//! RVI-03 (revisão independente da 0.9.15): banco de uma versão anterior com
//! defeito de integridade, aberto por esta versão.
//!
//! Antes, a atualização da estrutura (cópia com `VACUUM INTO`, migração,
//! conferência) rodava **antes** da conferência que põe o banco em modo
//! seguro (A3). Um banco antigo com defeito não chegava ao modo seguro.
//!
//! Exigido: o banco original e as cópias de recuperação que já existiam
//! ficam como estão; nenhuma migração pela metade; mensagem clara; o banco
//! com defeito não vira "loja vazia"; dá para entrar e restaurar um backup
//! bom pelo fluxo do modo seguro.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde_json::{json, Value};

use crate::banco::{self, Banco};
use crate::testes_modo_seguro::{estragar_lista_livre, sha};
use crate::{backup, config, migracao, usuarios};

fn pasta_nova(nome: &str) -> PathBuf {
    let p =
        std::env::temp_dir().join(format!("rustinho-rvi03-{nome}-{}-{}", std::process::id(), crate::tempo::novo_id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn contar(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |l| l.get(0)).unwrap()
}

/// Uma loja feita por uma versão anterior (estrutura 22, a da 0.9.14): loja,
/// caixa, a gerente Ana (senha 1234) com a chave do backup, e 30 OS com 3
/// itens cada.
fn loja_v22(pasta: &Path) -> PathBuf {
    let db = pasta.join("rustinho.db");
    let mut c = Connection::open(&db).unwrap();
    c.pragma_update(None, "journal_mode", "WAL").unwrap();
    banco::registrar_funcoes(&c).unwrap();
    banco::migrar_ate(&mut c, 22).unwrap();
    assert_eq!(contar(&c, "PRAGMA user_version"), 22);
    config::gravar(&c, "dispositivo_id", "disp-v22").unwrap();
    config::gravar(&c, "loja_nome", "Loja Antiga").unwrap();
    config::gravar(&c, "caixa_nome", "Caixa 1").unwrap();
    let u = usuarios::criar(&c, "disp-v22", "Ana", "1234", true).unwrap();
    usuarios::definir_gerente(&c, &u.id, true).unwrap();
    backup::gerente_conferido(&c, &u.id, "1234", None).unwrap();
    for n in 1..=30 {
        c.execute(
            "INSERT INTO ordem_servico (id, numero, dispositivo, caixa_nome, situacao, cliente_nome, placa, criado_em, atualizado_em)
             VALUES (?1, ?2, 'disp-v22', 'Caixa 1', 'orcamento', 'Cliente ' || ?2, 'ABC1D23', '2026-10-01', '2026-10-01')",
            rusqlite::params![format!("os-{n}"), n],
        )
        .unwrap();
        for i in 0..3 {
            c.execute(
                "INSERT INTO ordem_servico_item (id, ordem_id, ordem, tipo, descricao, quantidade, preco)
                 VALUES (?1, ?2, ?3, 'servico', 'Serviço', 1000, ?4)",
                rusqlite::params![format!("it-{n}-{i}"), format!("os-{n}"), i, 1000 * n + i],
            )
            .unwrap();
        }
    }
    c.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(())).unwrap();
    drop(c);
    db
}

/// Cópias de recuperação de uma migração anterior (que nunca podem sumir
/// nem mudar).
fn copias_antigas(pasta: &Path) -> Vec<(PathBuf, String)> {
    let antes = pasta.join("rustinho.db.antes-v21-20261001120000-abc123.db");
    std::fs::write(&antes, b"copia de recuperacao de uma migracao anterior").unwrap();
    std::fs::create_dir_all(pasta.join("pre-migracao")).unwrap();
    let guardada = pasta.join("pre-migracao").join("rustinho.db.antes-v20-20260901120000-def456.db");
    std::fs::write(&guardada, b"outra copia guardada").unwrap();
    [antes, guardada].into_iter().map(|p| (p.clone(), sha(&p))).collect()
}

/// Os arquivos da pasta (e de `pre-migracao/`), para conferir que nada nasceu.
fn arquivos(pasta: &Path) -> Vec<String> {
    let mut v = Vec::new();
    for sub in ["", "pre-migracao"] {
        if let Ok(d) = std::fs::read_dir(pasta.join(sub)) {
            for e in d.flatten() {
                if e.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    v.push(format!("{sub}/{}", e.file_name().to_string_lossy()));
                }
            }
        }
    }
    v.sort();
    v
}

/// Estraga a página raiz de uma tabela: o `quick_check` acusa e ler a tabela
/// (como o `VACUUM INTO` da cópia faz) dá erro.
fn estragar_tabela(db: &Path, tabela: &str) {
    let raiz: i64 = {
        let c = Connection::open(db).unwrap();
        c.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(())).unwrap();
        c.query_row("SELECT rootpage FROM sqlite_master WHERE type = 'table' AND name = ?1", [tabela], |l| l.get(0))
            .unwrap()
    };
    let mut bytes = std::fs::read(db).unwrap();
    let pagina = match u16::from_be_bytes([bytes[16], bytes[17]]) {
        1 => 65536usize,
        n => n as usize,
    };
    let inicio = (raiz as usize - 1) * pagina;
    // Tipo de página inválido.
    bytes[inicio] = 0xEE;
    std::fs::write(db, &bytes).unwrap();
    let c = Connection::open(db).unwrap();
    let r: String = c.query_row("PRAGMA quick_check", [], |l| l.get(0)).unwrap_or_else(|e| e.to_string());
    assert_ne!(r, "ok", "o banco devia estar com defeito");
    // Ler os dados da tabela (não só contar, que pode usar um índice).
    let ler = c.query_row(&format!("SELECT SUM(LENGTH(descricao)) FROM {tabela}"), [], |l| l.get::<_, i64>(0));
    assert!(ler.is_err(), "ler a tabela estragada devia falhar");
}

/// Confere que o banco com defeito ficou exatamente como estava, que as cópias
/// antigas continuam lá e iguais, e que nenhuma cópia, estado de migração ou
/// banco "que falhou" nasceu.
fn nada_mexido(db: &Path, sha_antes: &str, copias: &[(PathBuf, String)], lista_antes: &[String]) {
    assert_eq!(sha(db), sha_antes, "o arquivo do banco mudou");
    for (p, h) in copias {
        assert_eq!(&sha(p), h, "a cópia {} mudou ou sumiu", p.display());
    }
    let pasta = db.parent().unwrap();
    let novos: Vec<String> = arquivos(pasta)
        .into_iter()
        .filter(|a| !lista_antes.contains(a))
        // O SQLite cria os arquivos do diário ao abrir (sem conteúdo do banco),
        // e o serviço cria a trava de "um Rustinho por banco".
        .filter(|a| !a.ends_with("-wal") && !a.ends_with("-shm") && !a.ends_with(".trava"))
        .collect();
    assert!(novos.is_empty(), "nasceram arquivos: {novos:?}");
    assert!(!migracao::arquivo_de_estado(db).exists(), "ficou um estado de migração");
}

/// Banco antigo (estrutura 22) com defeito na lista de páginas livres: a
/// cópia com `VACUUM INTO` até sairia, mas o banco não é atualizado — abre em
/// modo seguro, na estrutura antiga, sem mexer em nada.
#[test]
fn rvi03_banco_antigo_com_defeito_abre_em_modo_seguro_sem_migrar() {
    let pasta = pasta_nova("lista-livre");
    let db = loja_v22(&pasta);
    estragar_lista_livre(&db);
    let copias = copias_antigas(&pasta);
    let (sha_antes, lista) = (sha(&db), arquivos(&pasta));
    let b = Banco::abrir(&db).expect("abre em modo seguro (não fica preso na atualização)");
    assert!(b.em_modo_seguro(), "devia estar em modo seguro");
    assert_eq!(contar(&b.conn, "PRAGMA user_version"), 22, "não migrou");
    assert_eq!(contar(&b.conn, "SELECT COUNT(*) FROM ordem_servico_item"), 90, "os dados continuam legíveis");
    drop(b);
    nada_mexido(&db, &sha_antes, &copias, &lista);
    // Abrir de novo: igual (não tenta atualizar, não muda nada).
    let b = Banco::abrir(&db).expect("abre de novo em modo seguro");
    assert!(b.em_modo_seguro());
    drop(b);
    nada_mexido(&db, &sha_antes, &copias, &lista);
    let _ = std::fs::remove_dir_all(&pasta);
}

/// Banco antigo com uma tabela estragada: a cópia de antes da atualização nem
/// sairia. Abre em modo seguro, sem cópia pela metade e sem estado de
/// atualização preso (antes: erro a cada abertura).
#[test]
fn rvi03_banco_antigo_que_nem_copia_abre_em_modo_seguro() {
    let pasta = pasta_nova("tabela");
    let db = loja_v22(&pasta);
    estragar_tabela(&db, "ordem_servico_item");
    let copias = copias_antigas(&pasta);
    let (sha_antes, lista) = (sha(&db), arquivos(&pasta));
    for vez in 1..=2 {
        let b = Banco::abrir(&db).unwrap_or_else(|e| panic!("abertura {vez}: devia abrir em modo seguro: {e}"));
        assert!(b.em_modo_seguro(), "abertura {vez}");
        assert_eq!(contar(&b.conn, "PRAGMA user_version"), 22);
        drop(b);
        nada_mexido(&db, &sha_antes, &copias, &lista);
    }
    let _ = std::fs::remove_dir_all(&pasta);
}

/// Banco que nem abre (o começo do arquivo estragado): erro claro, o arquivo
/// fica como está, e ele **não** vira uma loja vazia (nada é criado nele).
#[test]
fn rvi03_banco_que_nem_abre_nao_vira_loja_vazia() {
    let pasta = pasta_nova("nem-abre");
    let db = loja_v22(&pasta);
    let mut bytes = std::fs::read(&db).unwrap();
    bytes[..16].copy_from_slice(b"nao e um banco!!");
    std::fs::write(&db, &bytes).unwrap();
    let copias = copias_antigas(&pasta);
    let (sha_antes, lista) = (sha(&db), arquivos(&pasta));
    let e = Banco::abrir(&db).err().expect("não abre");
    let texto = e.to_string();
    assert!(!texto.is_empty());
    match crate::Servico::abrir_para_teste(&db, false) {
        Ok(_) => panic!("o serviço não pode abrir um banco que não é banco (viraria loja vazia)"),
        Err(e) => assert!(!e.to_string().is_empty()),
    }
    nada_mexido(&db, &sha_antes, &copias, &lista);
    let _ = std::fs::remove_dir_all(&pasta);
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

/// O caminho de recuperação, pela API das telas, com um banco antigo com
/// defeito: o programa não acha que é a primeira vez (loja vazia), mostra o
/// modo seguro e o aviso, deixa entrar, lista os backups, faz a cópia para o
/// suporte e restaura um backup bom; ao abrir de novo, a loja volta inteira.
#[test]
fn rvi03_recuperacao_pelo_modo_seguro_com_banco_antigo() {
    let pasta = pasta_nova("recupera");
    let db = loja_v22(&pasta);
    // Um backup bom desta loja (estrutura 22), feito antes do defeito.
    backup::fazer(&db, false).expect("backup bom, antes do defeito");
    let bom = backup::listar(&pasta.join("backups")).into_iter().next().expect("backup bom").caminho;
    let bom = PathBuf::from(bom);
    let sha_bom = sha(&bom);
    estragar_lista_livre(&db);
    let sha_antes = sha(&db);

    let mut s = crate::Servico::abrir_para_teste(&db, false).expect("abre em modo seguro");
    let e = ok(&mut s, "estado", json!({}));
    assert_eq!(e["primeiraVez"], false, "banco com defeito não é loja vazia: {e}");
    assert!(e["modoSeguro"].as_str().is_some_and(|t| !t.is_empty()), "{e}");
    let entrada = ok(&mut s, "usuarios_entrada", json!({}));
    assert!(entrada.to_string().contains("Ana"), "{entrada}");
    let ana = s.banco_para_teste().conn.query_row("SELECT id FROM usuario", [], |l| l.get::<_, String>(0)).unwrap();
    ok(&mut s, "entrar", json!({ "id": ana, "senha": "1234" }));
    let avisos = ok(&mut s, "avisos_do_dia", json!({}));
    assert!(avisos.as_array().unwrap().iter().any(|a| a["id"] == "banco_defeito"), "aviso do modo seguro: {avisos}");
    ok(&mut s, "menu", json!({}));
    ok(&mut s, "estado", json!({}));
    // O que a tela de Configurações lê ao abrir (a seção Backup fica nela).
    for c in ["config_ler", "sync_ler", "gerente_ler"] {
        ok(&mut s, c, json!({}));
    }
    let lista = ok(&mut s, "backup_ler", json!({}));
    assert!(lista.to_string().contains(&bom.file_name().unwrap().to_string_lossy().to_string()), "{lista}");
    // Venda e outras gravações continuam recusadas.
    let r = chamar(&mut s, "caixa_abrir", json!({ "valor": "10" }));
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(sha(&db), sha_antes, "nada gravado no banco com defeito");
    // A cópia para o suporte sai com nome próprio; o backup bom continua igual.
    let _ = chamar(&mut s, "backup_agora", json!({}));
    // A cópia roda em segundo plano (como na tela, que espera terminar).
    let fim = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while backup::ocupado() {
        assert!(std::time::Instant::now() < fim, "a cópia para o suporte não terminou");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert_eq!(sha(&bom), sha_bom, "o backup bom não mudou");
    assert_eq!(sha(&db), sha_antes, "a cópia para o suporte não mexe no banco com defeito");
    // Restaurar o backup bom (com a senha do gerente, como na tela).
    // (Outro teste pode estar fazendo backup ao mesmo tempo: espera a vez,
    // como a tela, que só deixa restaurar sem backup em andamento.)
    let pedido = json!({ "arquivo": bom.display().to_string(), "nuvem": false, "senha": "1234" });
    let mut r = chamar(&mut s, "backup_restaurar", pedido.clone());
    let fim = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while r["ok"] != true && r["erro"].as_str().unwrap_or_default().contains("backup em andamento") {
        assert!(std::time::Instant::now() < fim, "{r}");
        std::thread::sleep(std::time::Duration::from_millis(200));
        r = chamar(&mut s, "backup_restaurar", pedido.clone());
    }
    assert_eq!(r["ok"], true, "backup_restaurar: {r}");
    let r = r["dados"].clone();
    assert_eq!(r["reabrir"], true, "{r}");
    drop(s);
    // Abrir de novo: o banco restaurado é conferido, atualizado e abre normal.
    let mut s = crate::Servico::abrir_para_teste(&db, false).expect("abre normal depois de restaurar");
    let e = ok(&mut s, "estado", json!({}));
    assert!(e["modoSeguro"].is_null(), "saiu do modo seguro: {e}");
    ok(&mut s, "entrar", json!({ "id": ana, "senha": "1234" }));
    let b = s.banco_para_teste();
    assert_eq!(contar(&b.conn, "PRAGMA user_version"), banco::versao_da_estrutura() as i64);
    assert_eq!(contar(&b.conn, "SELECT COUNT(*) FROM ordem_servico_item"), 90, "a loja voltou inteira");
    // O banco com defeito ficou guardado (cópia para o suporte, de antes de restaurar).
    let guardados: Vec<String> = std::fs::read_dir(pasta.join("backups"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("rustinho-defeito-"))
        .collect();
    assert!(!guardados.is_empty(), "a cópia do banco com defeito ficou guardada");
    assert_eq!(sha(&bom), sha_bom, "o backup bom não mudou");
    let _ = std::fs::remove_dir_all(&pasta);
}

/// Só à mão / para o teste de tela (`app/testes/telas_modo_seguro.py`): monta
/// em `RUSTINHO_LOJA_TESTE` uma loja de versão anterior (estrutura 22) com
/// defeito na lista de páginas livres, e um backup bom dela em `backups/`.
#[test]
#[ignore]
fn montar_loja_v22_com_defeito() {
    let Some(pasta) = std::env::var_os("RUSTINHO_LOJA_TESTE").map(PathBuf::from) else { return };
    std::fs::create_dir_all(&pasta).unwrap();
    let db = loja_v22(&pasta);
    backup::fazer(&db, false).expect("backup bom, antes do defeito");
    estragar_lista_livre(&db);
}

/// 0915-P (registrado, não corrigido): um banco zerado (0 bytes) é, para o
/// SQLite, um banco novo: o Rustinho abre como primeira vez, mesmo com backups
/// da loja na pasta. Este teste só fixa o comportamento de hoje (e que abrir
/// não apaga os backups); mudar é decisão de fluxo (ver PENDENCIAS).
#[test]
fn p0915_banco_zerado_abre_como_primeira_vez_e_nao_apaga_backups() {
    let pasta = pasta_nova("zerado");
    let db = loja_v22(&pasta);
    backup::fazer(&db, false).expect("backup");
    let backups: Vec<(PathBuf, String)> = std::fs::read_dir(pasta.join("backups"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .map(|p| (p.clone(), sha(&p)))
        .collect();
    assert!(!backups.is_empty());
    for fim in ["", "-wal", "-shm"] {
        let mut p = db.as_os_str().to_owned();
        p.push(fim);
        let p = PathBuf::from(p);
        if p.exists() {
            std::fs::write(&p, b"").unwrap();
        }
    }
    let mut s = crate::Servico::abrir_para_teste(&db, false).expect("abre");
    let e = ok(&mut s, "estado", json!({}));
    assert_eq!(e["primeiraVez"], true, "hoje: vira primeira vez ({e})");
    drop(s);
    for (p, h) in &backups {
        assert_eq!(&sha(p), h, "o backup {} continua", p.display());
    }
    let _ = std::fs::remove_dir_all(&pasta);
}
