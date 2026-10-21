//! Prioridade 1 — Recuperação depois de o Rustinho fechar de repente (falta
//! de luz, travou, foi fechado à força) no meio de vendas, recebimentos,
//! devoluções, backup e restauração.
//!
//! Como: o próprio programa de testes é aberto de novo como "filho", que fica
//! vendendo sem parar; o teste mata o filho de uma vez (sem fechar nada) e
//! confere o banco. Sem a variável do filho, os testes "filho" não fazem nada.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use serde_json::json;

use crate::comum::*;

const FILHO: &str = "RUSTINHO_T6_FILHO";
const MODO: &str = "RUSTINHO_T6_MODO";

fn abrir_filho(pasta: &Path, modo: &str, teste: &str) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", teste, "--nocapture", "--test-threads=1"])
        .env(FILHO, pasta)
        .env(MODO, modo)
        // Só o filho da restauração cai de propósito (veja `backup::restaurar`).
        .env("RUSTINHO_TESTE_CAIR_RESTAURANDO", if modo == "restaurar" { "1" } else { "" })
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap()
}

fn feitas(pasta: &Path) -> i64 {
    std::fs::read_to_string(pasta.join("feitas.txt")).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0)
}

fn anotar_feitas(pasta: &Path, n: i64) {
    let tmp = pasta.join("feitas.tmp");
    std::fs::write(&tmp, n.to_string()).unwrap();
    std::fs::rename(&tmp, pasta.join("feitas.txt")).unwrap();
}

/// Espera o filho fazer `quantas` operações e mata de uma vez.
fn matar_depois_de(filho: &mut Child, pasta: &Path, quantas: i64) -> i64 {
    let inicio = Instant::now();
    while feitas(pasta) < quantas {
        if let Ok(Some(s)) = filho.try_wait() {
            panic!("o filho terminou sozinho ({s}) com {} feitas", feitas(pasta));
        }
        assert!(inicio.elapsed() < Duration::from_secs(120), "o filho não andou");
        std::thread::sleep(Duration::from_millis(20));
    }
    // Um pouco mais, para cair no meio de uma operação.
    std::thread::sleep(Duration::from_millis(37));
    filho.kill().unwrap(); // SIGKILL / TerminateProcess: sem fechar nada.
    filho.wait().unwrap();
    feitas(pasta)
}

/// O banco depois da queda: inteiro, sem venda pela metade, estoque e
/// crediário batendo com as vendas.
fn conferir_banco(l: &Loja, peca: &str, inicial: i64) {
    let c = l.conexao();
    let ok: String = c.query_row("PRAGMA integrity_check", [], |x| x.get(0)).unwrap();
    assert_eq!(ok, "ok");
    let chaves: i64 = c.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |x| x.get(0)).unwrap();
    assert_eq!(chaves, 0, "referência quebrada");
    let n = |sql: &str| -> i64 { c.query_row(sql, [], |x| x.get(0)).unwrap() };
    assert_eq!(
        n("SELECT COUNT(*) FROM venda v WHERE tipo = 'venda' AND NOT EXISTS (SELECT 1 FROM venda_item i WHERE i.venda_id = v.id)"),
        0,
        "venda sem itens"
    );
    assert_eq!(
        n("SELECT COUNT(*) FROM venda v WHERE tipo = 'venda' AND NOT EXISTS (SELECT 1 FROM venda_pagamento p WHERE p.venda_id = v.id)"),
        0,
        "venda sem pagamento"
    );
    // Cada item vendido baixou o estoque uma vez (nem zero, nem duas).
    let itens = n(&format!("SELECT COUNT(*) FROM venda_item i JOIN venda v ON v.id = i.venda_id WHERE v.tipo = 'venda' AND i.produto_id = '{peca}'"));
    let baixas = n(&format!("SELECT COUNT(*) FROM estoque_movimento WHERE produto_id = '{peca}' AND tipo = 'venda'"));
    assert_eq!(itens, baixas, "itens vendidos x baixas de estoque");
    let vendido = n(&format!(
        "SELECT COALESCE(SUM(i.quantidade), 0) FROM venda_item i JOIN venda v ON v.id = i.venda_id
          WHERE v.tipo = 'venda' AND v.situacao = 'concluida' AND i.produto_id = '{peca}'"
    ));
    let voltou = n(&format!(
        "SELECT COALESCE(SUM(quantidade), 0) FROM estoque_movimento WHERE produto_id = '{peca}' AND tipo NOT IN ('venda', 'inicial')"
    ));
    let saldo = n(&format!("SELECT COALESCE(SUM(quantidade), 0) FROM estoque_movimento WHERE produto_id = '{peca}'"));
    assert_eq!(saldo, inicial * 1000 - vendido + voltou, "estoque = inicial − vendido + o que voltou");
    // Crediário: cada venda no crediário tem as parcelas no valor certo.
    assert_eq!(
        n("SELECT COUNT(*) FROM venda v JOIN venda_pagamento p ON p.venda_id = v.id AND p.forma = 'crediario'
            WHERE v.tipo = 'venda' AND p.valor <> (SELECT COALESCE(SUM(r.valor), 0) FROM conta_receber r WHERE r.venda_id = v.id)"),
        0,
        "venda no crediário sem as parcelas certas"
    );
    // Recebimento sem parcela abatida, ou parcela abatida sem recebimento.
    assert_eq!(
        n("SELECT COUNT(*) FROM recebimento r WHERE NOT EXISTS (SELECT 1 FROM recebimento_parcela p WHERE p.recebimento_id = r.id)"),
        0,
        "recebimento pela metade"
    );
    // Tudo o que foi gravado está na fila da troca de dados (os outros caixas vão receber).
    assert_eq!(
        n("SELECT COUNT(*) FROM venda v WHERE NOT EXISTS (SELECT 1 FROM alteracao a WHERE a.tabela = 'venda' AND a.chave = json_array(v.id))"),
        0,
        "venda fora da fila da troca de dados"
    );
}

/// O filho: vende, recebe e devolve sem parar (só roda quando o teste pede).
#[test]
fn t6_filho_vende() {
    let Some(pasta) = std::env::var_os(FILHO).map(PathBuf::from) else { return };
    let mut l = Loja::abrir_em(&pasta, false);
    l.entrar("Ana", SENHA_ANA);
    let peca = std::fs::read_to_string(pasta.join("peca.txt")).unwrap();
    let ze = std::fs::read_to_string(pasta.join("cliente.txt")).unwrap();
    let mut n = feitas(&pasta);
    loop {
        match n % 4 {
            0 => {
                l.vender(&[(&peca, "1")], &[("dinheiro", "10")]);
            }
            1 => {
                l.vender_com(&[(&peca, "2")], &[("crediario", "20")], json!({ "clienteId": ze, "parcelas": 2 }));
            }
            2 => {
                l.ok("receber_baixar", json!({ "clienteId": ze, "contas": [], "valor": "5", "forma": "dinheiro" }));
            }
            _ => {
                let v = id(&l.vender(&[(&peca, "1")], &[("pix", "10")]));
                l.devolver(&v, 0, "1", "dinheiro", false);
            }
        }
        n += 1;
        anotar_feitas(&pasta, n);
    }
}

/// T6.1 — Morre no meio das vendas: o banco abre, está inteiro, nada que foi
/// concluído se perdeu e nada ficou pela metade. Duas quedas seguidas.
#[test]
fn t6_1_queda_durante_as_vendas() {
    if std::env::var_os(FILHO).is_some() {
        return;
    }
    let mut l = Loja::nova("t6-1");
    let peca = l.peca("Fusível", "1", "10", "100000");
    let ze = l.pessoa("cliente", "Zé");
    l.abrir_caixa();
    std::fs::write(l.pasta.join("peca.txt"), &peca).unwrap();
    std::fs::write(l.pasta.join("cliente.txt"), &ze).unwrap();
    l.fechar();

    let mut total = 0;
    for rodada in 1..=2 {
        let mut filho = abrir_filho(&l.pasta, "vender", "t6_recuperacao::t6_filho_vende");
        let quantas = matar_depois_de(&mut filho, &l.pasta, total + 40);
        // Abre de novo (a trava do banco foi solta pelo sistema).
        l.reabrir();
        conferir_banco(&l, &peca, 100000);
        let concluidas =
            l.numero("SELECT COUNT(*) FROM venda WHERE tipo = 'venda'") + l.numero("SELECT COUNT(*) FROM recebimento");
        let esperadas_no_minimo = quantas / 4 * 4; // vendas+recebimentos anotados como feitos
        assert!(
            concluidas >= esperadas_no_minimo * 3 / 4,
            "rodada {rodada}: o filho anotou {quantas} feitas e o banco tem {concluidas}"
        );
        total = quantas;
        // Continua trabalhando: o caixa aberto continua aberto, e vende.
        l.entrar("Ana", SENHA_ANA);
        let e = l.ok("caixa_estado", json!({}));
        assert_eq!(e["aberto"], true, "o caixa continua aberto depois da queda");
        l.vender(&[(&peca, "1")], &[("dinheiro", "10")]);
        conferir_banco(&l, &peca, 100000);
        l.fechar();
    }
}

/// Filho que faz backups sem parar.
#[test]
fn t6_filho_backup() {
    let Some(pasta) = std::env::var_os(FILHO).map(PathBuf::from) else { return };
    if std::env::var(MODO).as_deref() != Ok("backup") {
        return;
    }
    let db = pasta.join("rustinho.db");
    let mut n = feitas(&pasta);
    loop {
        rustinho_nucleo::backup::fazer(&db, false).unwrap();
        n += 1;
        anotar_feitas(&pasta, n);
    }
}

/// T6.2 — Morre no meio de um backup: o banco não é afetado, as cópias
/// prontas abrem e o pedaço interrompido é limpo no backup seguinte.
#[test]
fn t6_2_queda_durante_o_backup() {
    if std::env::var_os(FILHO).is_some() {
        return;
    }
    let _vez = vez_do_backup();
    let mut l = Loja::nova("t6-2");
    povoar(&mut l);
    let foto = foto_do_negocio(&l.conexao());
    l.fechar();
    let mut filho = abrir_filho(&l.pasta, "backup", "t6_recuperacao::t6_filho_backup");
    matar_depois_de(&mut filho, &l.pasta, 2);
    l.reabrir();
    assert!(diferencas(&foto, &foto_do_negocio(&l.conexao())).is_empty(), "o backup mudou os dados");
    let pasta_backup = rustinho_nucleo::backup::ajustes(&l.conexao(), Some(&l.db)).unwrap().pasta;
    // Faz mais um: limpa o que sobrou do interrompido.
    rustinho_nucleo::backup::fazer(&l.db, false).unwrap();
    for e in std::fs::read_dir(&pasta_backup).unwrap().flatten() {
        let nome = e.file_name().to_string_lossy().to_string();
        assert!(!nome.starts_with(".rustinho-"), "sobrou pedaço de backup: {nome}");
    }
    for c in rustinho_nucleo::backup::listar(&pasta_backup) {
        rustinho_nucleo::backup::conferir(&l.conexao(), Path::new(&c.caminho), SENHA_ANA)
            .unwrap_or_else(|e| panic!("a cópia {} não abre: {e}", c.nome));
    }
}

/// Filho que restaura um backup e cai logo depois de copiar o banco (antes
/// de terminar de arrumar o que é deste computador).
#[test]
fn t6_filho_restaura() {
    let Some(pasta) = std::env::var_os(FILHO).map(PathBuf::from) else { return };
    if std::env::var(MODO).as_deref() != Ok("restaurar") {
        return;
    }
    let arquivo = std::fs::read_to_string(pasta.join("arquivo.txt")).unwrap();
    let mut l = Loja::abrir_em(&pasta, false);
    l.entrar("Ana", SENHA_ANA);
    anotar_feitas(&pasta, 1);
    l.chamar("backup_restaurar", json!({ "arquivo": arquivo, "nuvem": false, "senha": SENHA_ANA }));
    // Se chegou aqui, a queda simulada não aconteceu.
    anotar_feitas(&pasta, 99);
}

/// T6.3 — Morre no meio de uma restauração: ao abrir de novo, o banco é o do
/// backup e o que é deste computador (nome do caixa, identidade, caminhos dos
/// outros caixas) volta a ser deste computador; a troca de dados recomeça.
#[test]
fn t6_3_queda_durante_a_restauracao() {
    if std::env::var_os(FILHO).is_some() {
        return;
    }
    let _vez = vez_do_backup();
    let mut a = Loja::nova("t6-3a");
    povoar(&mut a);
    let feito = rustinho_nucleo::backup::fazer(&a.db, false).unwrap();
    let foto_a = foto_do_negocio(&a.conexao());

    let mut b = Loja::nova("t6-3b");
    b.executar("UPDATE config SET valor = 'Caixa do Fundo' WHERE chave = 'caixa_nome'");
    let id_b: String =
        b.conexao().query_row("SELECT valor FROM config WHERE chave = 'dispositivo_id'", [], |x| x.get(0)).unwrap();
    let geracao_antes: Option<String> =
        b.conexao().query_row("SELECT valor FROM config WHERE chave = 'sync_geracao'", [], |x| x.get(0)).ok();
    std::fs::write(b.pasta.join("arquivo.txt"), &feito.arquivo).unwrap();
    b.fechar();
    let mut filho = abrir_filho(&b.pasta, "restaurar", "t6_recuperacao::t6_filho_restaura");
    let st = filho.wait().unwrap();
    assert!(!st.success(), "o filho devia cair no meio da restauração");
    assert_ne!(feitas(&b.pasta), 99, "a queda simulada não aconteceu");

    b.reabrir();
    let c = b.conexao();
    let ler = |k: &str| -> Option<String> {
        c.query_row("SELECT valor FROM config WHERE chave = ?1", [k], |x| x.get(0)).ok()
    };
    assert_eq!(ler("dispositivo_id").as_deref(), Some(id_b.as_str()), "a identidade deste computador voltou");
    assert_eq!(ler("caixa_nome").as_deref(), Some("Caixa do Fundo"), "o nome deste caixa voltou");
    assert_ne!(ler("sync_geracao"), geracao_antes, "a troca de dados recomeça (geração nova)");
    let d = diferencas(&foto_a, &foto_do_negocio(&c));
    assert!(d.is_empty(), "os dados são os do backup: {d:?}");
    assert!(!b.pasta.join("rustinho.db.restaurando").exists(), "a marca da restauração foi tirada");
    drop(c);
    b.entrar("Ana", SENHA_ANA);
    b.abrir_caixa();
    let p = b.peca("Depois da queda", "1", "2", "5");
    b.vender(&[(&p, "1")], &[("dinheiro", "2")]);
}
