//! Rustinho portátil (o ZIP de teste): fica todo na própria pasta e não
//! encosta em nenhuma outra instalação (veja `nucleo/src/portatil.rs`).
//!
//! O teste copia o próprio executável dos testes para uma pasta temporária,
//! põe a marca `rustinho-portatil.txt` ao lado e roda a cópia (outro
//! processo): lá dentro o Rustinho se vê como portátil. As pastas "de
//! sempre" (HOME, XDG_DATA_HOME, LOCALAPPDATA, RUSTINHO_DADOS) apontam para
//! uma "instalação de verdade" de mentira, com um banco que não pode mudar.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

const FILHO: &str = "RUSTINHO_TESTE_PORTATIL_FILHO";

fn pasta_nova(nome: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rustinho-{nome}-{}-{}",
        std::process::id(),
        rustinho_nucleo::tempo::novo_id()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn resumo(p: &Path) -> String {
    use sha2::{Digest, Sha256};
    let b = std::fs::read(p).unwrap();
    Sha256::digest(&b).iter().map(|x| format!("{x:02x}")).collect()
}

/// Todos os arquivos de uma pasta (com subpastas), em ordem.
fn arquivos(p: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(l) = std::fs::read_dir(p) {
        for e in l.flatten() {
            let c = e.path();
            if c.is_dir() {
                v.extend(arquivos(&c));
            } else {
                v.push(c);
            }
        }
    }
    v.sort();
    v
}

#[test]
fn portatil_fica_na_propria_pasta_e_nao_mexe_na_instalacao() {
    // A "instalação de verdade": um banco na pasta de sempre e outro na de RUSTINHO_DADOS.
    let casa = pasta_nova("portatil-casa");
    let sempre = if cfg!(windows) { casa.join("AppData/Local/Rustinho") } else { casa.join(".local/share/rustinho") };
    let outra = casa.join("rustinho-dados-env");
    for p in [&sempre, &outra] {
        std::fs::create_dir_all(p).unwrap();
        std::fs::write(p.join("rustinho.db"), b"banco da loja de verdade (nao pode mudar)").unwrap();
    }
    let antes: Vec<(PathBuf, String)> = arquivos(&casa).into_iter().map(|a| (a.clone(), resumo(&a))).collect();

    // O portátil: cópia do executável + a marca.
    let raiz = pasta_nova("portatil-raiz");
    let exe = std::env::current_exe().unwrap();
    let copia = raiz.join(exe.file_name().unwrap());
    std::fs::copy(&exe, &copia).unwrap();
    std::fs::write(raiz.join("rustinho-portatil.txt"), "teste").unwrap();

    let saida = Command::new(&copia)
        .args(["--exact", "filho_portatil", "--ignored", "--nocapture", "--test-threads=1"])
        .env(FILHO, "1")
        .env("HOME", &casa)
        .env("XDG_DATA_HOME", casa.join(".local/share"))
        .env("LOCALAPPDATA", casa.join("AppData/Local"))
        .env("APPDATA", casa.join("AppData/Roaming"))
        .env("RUSTINHO_DADOS", &outra)
        .output()
        .unwrap();
    let texto = format!("{}\n{}", String::from_utf8_lossy(&saida.stdout), String::from_utf8_lossy(&saida.stderr));
    assert!(saida.status.success(), "o portátil falhou:\n{texto}");
    assert!(texto.contains("PORTATIL-OK"), "o teste de dentro não rodou:\n{texto}");

    // Os dados do portátil ficaram na pasta dele.
    assert!(raiz.join("dados/rustinho.db").is_file());
    // Nada mudou na "instalação de verdade" (nem arquivo novo).
    let depois: Vec<(PathBuf, String)> = arquivos(&casa).into_iter().map(|a| (a.clone(), resumo(&a))).collect();
    assert_eq!(antes, depois, "o portátil mexeu fora da pasta dele");

    std::fs::remove_dir_all(&raiz).ok();
    std::fs::remove_dir_all(&casa).ok();
}

fn chamar(s: &mut rustinho_nucleo::Servico, comando: &str, dados: Value) -> Value {
    serde_json::from_str(&s.chamar(comando, &dados.to_string())).unwrap()
}

/// Roda dentro da cópia (o processo portátil).
#[test]
#[ignore]
fn filho_portatil() {
    if std::env::var(FILHO).as_deref() != Ok("1") {
        return;
    }
    use rustinho_nucleo::portatil;
    let raiz = portatil::raiz().expect("a cópia devia ser portátil").to_path_buf();
    assert!(portatil::ligado());
    // 1. O banco fica em dados/, mesmo com RUSTINHO_DADOS e as pastas de sempre.
    let banco = rustinho_nucleo::caminho_padrao();
    assert_eq!(banco, raiz.join("dados").join("rustinho.db"));
    let mut s = rustinho_nucleo::Servico::abrir(&banco).unwrap();
    let r = chamar(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja Teste", "caixa": "TESTE", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
    );
    assert_eq!(r["ok"], true, "{r}");
    // 2. Troca de dados: outras portas.
    let r = chamar(&mut s, "sync_ler", json!({}));
    assert_eq!(r["dados"]["porta"], 47820, "{r}");
    assert_eq!(portatil::portas(), (47820, 47821));
    // 3. A chave da rede sai de outro jeito (outra senha por dentro).
    assert_ne!(portatil::senha_da_rede("1234"), "1234");
    // 4. Backup: só dentro da pasta portátil.
    let fora = std::env::var_os("HOME").map(PathBuf::from).unwrap().join("backups-fora");
    let r =
        chamar(&mut s, "backup_salvar", json!({ "pasta": fora.display().to_string(), "dias": "30", "hora": "18:00" }));
    assert_eq!(r["ok"], false, "aceitou uma pasta de fora: {r}");
    assert!(r["erro"].as_str().unwrap().contains("portátil"), "{r}");
    assert!(!fora.exists());
    let dentro = raiz.join("dados").join("meus-backups");
    let r = chamar(
        &mut s,
        "backup_salvar",
        json!({ "pasta": dentro.display().to_string(), "dias": "30", "hora": "18:00" }),
    );
    assert_eq!(r["ok"], true, "{r}");
    let r = chamar(&mut s, "backup_agora", json!({}));
    assert_eq!(r["ok"], true, "{r}");
    // O backup roda em segundo plano: espera o arquivo aparecer.
    let mut feito = false;
    for _ in 0..200 {
        if std::fs::read_dir(&dentro)
            .map(|l| l.flatten().any(|e| e.file_name().to_string_lossy().ends_with(".rbk")))
            .unwrap_or(false)
        {
            feito = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let lista: Vec<String> = std::fs::read_dir(&dentro)
        .map(|l| l.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    assert!(feito, "o backup não apareceu dentro da pasta portátil: {lista:?}");
    // 5. Google Drive: configuração própria.
    assert_eq!(portatil::rclone_conf(), Some(raiz.join("dados").join("rclone.conf")));
    drop(s);
    println!("PORTATIL-OK");
}
