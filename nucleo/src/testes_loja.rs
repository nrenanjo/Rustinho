//! FALHA-01 (auditoria da 0.9.15): identidade da loja e abandono da junção.
//!
//! - V01a: desistir de entrar numa loja não deixa a troca de dados ligada,
//!   nem a senha da rede, nem dados da outra loja; criar a loja nova depois
//!   não herda nada da tentativa.
//! - V01b: duas lojas criadas separadamente, com a mesma senha da rede, não
//!   se fundem pela troca de dados (nem gerentes somados, nem faixas de
//!   códigos cruzadas); caixas da mesma loja, cópias registradas e bancos
//!   restaurados continuam trocando.
//!
//! Os testes de rede usam portas escolhidas pelo sistema (C8: nenhuma porta
//! fixa; o número 48799 que aparece é só a porta informada por quem puxa).

use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use rusqlite::Connection;
use serde_json::{json, Value};

use crate::banco::Banco;
use crate::faixas::para_testes;
use crate::sync::{self, Motor};
use crate::{config, usuarios};

const SENHA_DA_REDE: &str = "senha-da-loja";

fn temp(nome: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rustinho-loja-{nome}-{}", crate::tempo::novo_id()));
    std::fs::create_dir_all(&p).unwrap();
    p.join("rustinho.db")
}

fn contar(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |l| l.get(0)).unwrap()
}

/// Identidade da loja gravada no banco ("" = nenhuma).
fn loja_id(c: &Connection) -> String {
    config::ler(c, "loja_id").unwrap().unwrap_or_default()
}

/// Um motor da troca de dados com portas escolhidas pelo sistema (C8: nenhuma
/// porta fixa, que pode estar ocupada no computador), e a porta TCP dele.
fn motor(caminho: &Path) -> (Motor, u16) {
    let m = Motor::iniciar(caminho.to_path_buf(), 0, 0);
    let (tcp, _) = m.portas(Duration::from_secs(10)).expect("as portas do motor abriram");
    (m, tcp)
}

/// A puxada foi recusada por ser de outra loja (e não por falta de rede, que
/// faria o teste passar sem provar nada).
fn recusa_de_outra_loja<T: std::fmt::Debug>(r: &crate::erro::Resultado<T>) -> bool {
    matches!(r, Err(e) if e.to_string().contains("outra loja"))
}

fn gerentes(c: &Connection) -> i64 {
    contar(c, "SELECT COUNT(*) FROM usuario WHERE gerente = 1 AND ativo = 1")
}

fn chamar(s: &mut crate::Servico, cmd: &str, d: Value) -> Value {
    serde_json::from_str(&crate::apoio_chave::chamar(crate::api::NAO_REPETIR, cmd, &d.to_string(), |x| {
        s.chamar(cmd, x)
    }))
    .unwrap()
}

/// Loja configurada num arquivo, como na primeira tela (gerente "Ana", este
/// caixa distribui as faixas), com a troca de dados ligada. Reabre o banco
/// (como ao abrir o programa de novo). Devolve o banco e o gerente.
fn loja_em_arquivo(caminho: &Path, caixa: &str, loja: &str) -> (Banco, String) {
    let b = Banco::abrir(caminho).unwrap();
    let u = para_testes::configurar(&b, caixa, 0);
    config::gravar(&b.conn, "loja_nome", loja).unwrap();
    sync::salvar_ajustes(&b.conn, true, SENHA_DA_REDE, "").unwrap();
    drop(b);
    (Banco::abrir(caminho).unwrap(), u)
}

/// Caixa novo (sem usuários) que entra na loja: liga a troca com a senha.
fn caixa_novo_em_arquivo(caminho: &Path, caixa: &str) -> Banco {
    let b = Banco::abrir(caminho).unwrap();
    config::gravar(&b.conn, "caixa_nome", caixa).unwrap();
    config::gravar(&b.conn, "codigos_espera_teste", "0").unwrap();
    sync::salvar_ajustes(&b.conn, true, SENHA_DA_REDE, "").unwrap();
    b
}

// ---------------------------------------------------------------------------
// V01a
// ---------------------------------------------------------------------------

/// Caixa novo começa a entrar numa loja, recebe parte dos dados dela e
/// desiste: a tentativa sai inteira (troca desligada, sem senha, sem
/// endereço, sem par, sem dado da outra loja); a loja nova nasce limpa, com
/// um gerente só, e não herda a tentativa.
#[test]
fn v01a_desistir_da_juncao_e_comecar_loja_nova() {
    let mut s = crate::Servico::em_memoria().unwrap();
    let r = chamar(
        &mut s,
        "sync_juntar",
        json!({ "caixa": "Caixa 2", "senha": "rede-da-loja", "enderecos": "192.168.0.10" }),
    );
    assert_eq!(r["ok"], true, "{r}");
    // Chegou parte da outra loja (o nome dela, uma peça, um cliente, um par),
    // mas ainda nenhum usuário.
    let (outra, u) = para_testes::loja("Caixa da outra loja");
    para_testes::peca(&outra, &u, "Peça da outra loja");
    let (lista, _) = sync::alteracoes_desde(&outra.conn, 0, 10_000_000).unwrap();
    let sem_usuarios: Vec<_> = lista.into_iter().filter(|a| a.tabela != "usuario").collect();
    {
        let b = s.banco_para_teste();
        sync::aplicar(&mut b.conn, &sem_usuarios).unwrap();
        b.conn
            .execute("INSERT INTO sync_par (dispositivo, nome, endereco) VALUES ('outro', 'Caixa da outra loja', '192.168.0.10:47810')", [])
            .unwrap();
        assert!(contar(&b.conn, "SELECT COUNT(*) FROM produto") > 0, "a tentativa recebeu dados");
    }
    let r = chamar(&mut s, "sync_desistir_juntar", json!({}));
    assert_eq!(r["ok"], true, "desistir da junção: {r}");
    let r = chamar(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja nova", "caixa": "Caixa 2", "nome": "Zé", "senha": "1234", "senha2": "1234" }),
    );
    assert_eq!(r["ok"], true, "{r}");
    let l = chamar(&mut s, "sync_ler", json!({}));
    assert_eq!(l["dados"]["ligado"], false, "troca de dados desligada: {l}");
    assert_eq!(l["dados"]["temSenha"], false, "sem a senha da rede: {l}");
    assert_eq!(l["dados"]["enderecos"], "", "sem os endereços da tentativa: {l}");
    let b = s.banco_para_teste();
    assert_eq!(contar(&b.conn, "SELECT COUNT(*) FROM sync_par"), 0, "nenhum par da tentativa");
    assert_eq!(contar(&b.conn, "SELECT COUNT(*) FROM produto"), 0, "nenhum dado da outra loja");
    assert_eq!(contar(&b.conn, "SELECT COUNT(*) FROM faixa_codigo WHERE dispositivo <> (SELECT valor FROM config WHERE chave = 'dispositivo_id')"), 0);
    assert_eq!(config::nome_loja(&b.conn).unwrap(), "Loja nova");
    assert_eq!(gerentes(&b.conn), 1, "um gerente só: o desta loja");
    assert_eq!(usuarios::quantidade(&b.conn).unwrap(), 1);
    assert!(!loja_id(&b.conn).is_empty(), "a loja nova tem identidade própria");
}

/// Sem desistir, a primeira tela não cria uma loja em cima de uma junção pela
/// metade (nem gerente local com a troca de dados ligada).
#[test]
fn v01a_configurar_inicio_nao_herda_a_juncao() {
    let mut s = crate::Servico::em_memoria().unwrap();
    let r = chamar(&mut s, "sync_juntar", json!({ "caixa": "Caixa 2", "senha": "rede-da-loja", "enderecos": "" }));
    assert_eq!(r["ok"], true, "{r}");
    let r = chamar(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja nova", "caixa": "Caixa 2", "nome": "Zé", "senha": "1234", "senha2": "1234" }),
    );
    assert_eq!(r["ok"], false, "não cria a loja em cima da junção: {r}");
    let b = s.banco_para_teste();
    assert_eq!(usuarios::quantidade(&b.conn).unwrap(), 0, "nenhum gerente criado");
    let e = chamar(&mut s, "estado", json!({}));
    assert_eq!(e["dados"]["juntando"], true, "a tela volta para a junção: {e}");
    // Desistir sem tentativa nenhuma não mexe em nada (rede de verdade fica).
    let mut t = crate::Servico::em_memoria().unwrap();
    let r = chamar(
        &mut t,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
    );
    assert_eq!(r["ok"], true, "{r}");
    let r = chamar(&mut t, "sync_salvar", json!({ "ligado": true, "senha": "rede-boa", "enderecos": "10.0.0.2" }));
    assert_eq!(r["ok"], true, "{r}");
    let r = chamar(&mut t, "sync_desistir_juntar", json!({}));
    assert_eq!(r["ok"], false, "loja configurada não desiste de junção: {r}");
    let l = chamar(&mut t, "sync_ler", json!({}));
    assert_eq!(l["dados"]["ligado"], true, "{l}");
    assert_eq!(l["dados"]["temSenha"], true, "{l}");
}

// ---------------------------------------------------------------------------
// Primeira ativação da rede
// ---------------------------------------------------------------------------

/// O primeiro caixa liga a troca de dados com o gerente, sem precisar de
/// outro caixa. Um usuário caixa não liga a troca nem troca a senha da rede
/// sem a senha do gerente, mesmo com "Configurações" liberada para todos.
#[test]
fn primeiro_caixa_liga_a_rede_com_o_gerente_local() {
    let mut s = crate::Servico::em_memoria().unwrap();
    let r = chamar(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
    );
    assert_eq!(r["ok"], true, "{r}");
    let r = chamar(&mut s, "sync_salvar", json!({ "ligado": true, "senha": "rede-da-loja", "enderecos": "" }));
    assert_eq!(r["ok"], true, "o gerente liga a rede sozinho: {r}");
    assert_eq!(r["dados"]["ligado"], true);
    // Bia é caixa; a loja libera Configurações para todos.
    let r =
        chamar(&mut s, "usuario_salvar", json!({ "nome": "Bia", "senha": "4321", "senha2": "4321", "ativo": true }));
    assert_eq!(r["ok"], true, "{r}");
    let bia = r["dados"]["id"].as_str().unwrap().to_string();
    let r = chamar(&mut s, "gerente_salvar", json!({ "acoes": { "configuracoes": false } }));
    assert_eq!(r["ok"], true, "{r}");
    let r = chamar(&mut s, "sync_salvar", json!({ "ligado": false, "senha": "", "enderecos": "" }));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(chamar(&mut s, "entrar", json!({ "id": bia, "senha": "4321" }))["ok"], true);
    // Endereços: Configurações (liberada) basta.
    let r = chamar(&mut s, "sync_salvar", json!({ "ligado": false, "senha": "", "enderecos": "10.0.0.5" }));
    assert_eq!(r["ok"], true, "endereços continuam em Configurações: {r}");
    // Ligar a troca: só com o gerente.
    let pedido = json!({ "ligado": true, "senha": "", "enderecos": "10.0.0.5" });
    let r = chamar(&mut s, "sync_salvar", pedido.clone());
    assert_eq!(r["ok"], false, "caixa não liga a troca sozinho: {r}");
    assert_eq!(r["campo"], "gerente", "{r}");
    let r = chamar(&mut s, "sync_salvar", json!({ "ligado": true, "senha": "outra-senha", "enderecos": "" }));
    assert_eq!(r["ok"], false, "nem troca a senha da rede: {r}");
    // Com a senha do gerente presa a esta chamada, liga.
    let r = chamar(&mut s, "gerente_liberar", json!({ "senha": "1234", "comando": "sync_salvar", "dados": pedido }));
    assert_eq!(r["ok"], true, "{r}");
    let r = chamar(&mut s, "sync_salvar", pedido);
    assert_eq!(r["ok"], true, "com a senha do gerente: {r}");
    assert_eq!(r["dados"]["ligado"], true);
}

// ---------------------------------------------------------------------------
// V01b e os casos legítimos, pela rede de verdade
// ---------------------------------------------------------------------------

/// Duas lojas criadas separadamente, com a mesma senha da rede: nenhuma
/// recebe nada da outra, nos dois sentidos; nenhum gerente somado; nenhuma
/// faixa de códigos cruzada; as duas avisam; e depois de reabrir continua igual.
#[test]
fn v01b_duas_lojas_independentes_nao_se_fundem() {
    thread::spawn(|| {
        let (ca, cb) = (temp("v01b-a"), temp("v01b-b"));
        let (a, ua) = loja_em_arquivo(&ca, "Caixa A", "Loja A");
        let (b, ub) = loja_em_arquivo(&cb, "Caixa B", "Loja B");
        para_testes::peca(&a, &ua, "Peça da loja A");
        para_testes::peca(&b, &ub, "Peça da loja B");
        // Histórico parcial: a troca vem em lotes pequenos.
        config::gravar(&a.conn, "sync_lote_teste", "2").unwrap();
        config::gravar(&b.conn, "sync_lote_teste", "2").unwrap();
        let tentar = |rodada: &str| {
            let (mut ma, pt_a) = motor(&ca);
            let (mut mb, pt_b) = motor(&cb);
            let (para_a, para_b) = (format!("127.0.0.1:{pt_a}"), format!("127.0.0.1:{pt_b}"));
            let r1 = sync::puxar(&cb, &para_a, pt_b);
            let r2 = sync::puxar(&ca, &para_b, pt_a);
            assert!(ma.encerrar(Duration::from_secs(20)));
            assert!(mb.encerrar(Duration::from_secs(20)));
            // C8: recusado por ser de outra loja (não por falta de rede).
            assert!(recusa_de_outra_loja(&r1), "{rodada}: B não recebe da loja A: {r1:?}");
            assert!(recusa_de_outra_loja(&r2), "{rodada}: A não recebe da loja B: {r2:?}");
            for (quem, x, peca_dele) in [("A", &a, "Peça da loja B"), ("B", &b, "Peça da loja A")] {
                assert_eq!(gerentes(&x.conn), 1, "{rodada} {quem}: gerentes não se somam");
                assert_eq!(usuarios::quantidade(&x.conn).unwrap(), 1, "{rodada} {quem}");
                let n: i64 = x
                    .conn
                    .query_row("SELECT COUNT(*) FROM produto WHERE nome = ?1", [peca_dele], |l| l.get(0))
                    .unwrap();
                assert_eq!(n, 0, "{rodada} {quem}: nada da outra loja");
                assert!(crate::faixas::cruzadas(&x.conn).unwrap().is_empty(), "{rodada} {quem}: faixas não cruzam");
                assert_eq!(contar(&x.conn, "SELECT COUNT(*) FROM faixa_codigo"), 1, "{rodada} {quem}");
                let avisos = crate::avisos::do_dia(&x.conn).unwrap();
                assert!(avisos.iter().any(|v| v.id == "outra_loja"), "{rodada} {quem}: aviso da outra loja");
            }
        };
        tentar("primeira vez");
        // Reabrir os dois (programa fechado e aberto de novo): continua.
        drop(Banco::abrir(&ca).unwrap());
        drop(Banco::abrir(&cb).unwrap());
        tentar("depois de reabrir");
    })
    .join()
    .unwrap();
}

/// Caixa novo entra pelo fluxo da junção: recebe a loja (inclusive a
/// identidade e o gerente dela, sem criar gerente local) e os dois trocam
/// nos dois sentidos, também depois de reabrir.
#[test]
fn caixa_novo_entra_pela_juncao_e_troca_nos_dois_sentidos() {
    thread::spawn(|| {
        let (ca, cb) = (temp("juntar-a"), temp("juntar-b"));
        let (a, ua) = loja_em_arquivo(&ca, "Caixa A", "Loja A");
        para_testes::peca(&a, &ua, "Filtro");
        let b = caixa_novo_em_arquivo(&cb, "Caixa B");
        let rodar = |rodada: &str| {
            let (mut ma, pt_a) = motor(&ca);
            let (mut mb, pt_b) = motor(&cb);
            let (para_a, para_b) = (format!("127.0.0.1:{pt_a}"), format!("127.0.0.1:{pt_b}"));
            sync::puxar(&cb, &para_a, pt_b).unwrap_or_else(|e| panic!("{rodada}: B puxa de A: {e}"));
            sync::puxar(&ca, &para_b, pt_a).unwrap_or_else(|e| panic!("{rodada}: A puxa de B: {e}"));
            assert!(ma.encerrar(Duration::from_secs(20)));
            assert!(mb.encerrar(Duration::from_secs(20)));
        };
        rodar("junção");
        assert_eq!(usuarios::quantidade(&b.conn).unwrap(), 1, "o gerente da loja chegou");
        assert_eq!(gerentes(&b.conn), 1);
        assert_eq!(loja_id(&b.conn), loja_id(&a.conn), "a mesma loja");
        assert!(!loja_id(&a.conn).is_empty());
        // B grava algo; A recebe.
        let p = para_testes::peca(&b, &ua, "Correia (do caixa B)");
        drop(Banco::abrir(&ca).unwrap());
        drop(Banco::abrir(&cb).unwrap());
        rodar("depois de reabrir");
        assert!(crate::produtos::ficha(&a.conn, &p).is_ok(), "A recebeu de B");
        assert!(crate::avisos::do_dia(&a.conn).unwrap().iter().all(|v| v.id != "outra_loja"));
    })
    .join()
    .unwrap();
}

/// Banco copiado (registrado como caixa novo pelo gerente) e banco restaurado
/// de uma cópia antiga: continuam trocando com a loja, sem recusa por "outra loja".
#[test]
fn copia_registrada_e_banco_restaurado_continuam_na_loja() {
    thread::spawn(|| {
        let (ca, cc) = (temp("copia-a"), temp("copia-c"));
        let (a, ua) = loja_em_arquivo(&ca, "Caixa A", "Loja A");
        para_testes::peca(&a, &ua, "Filtro");
        // Cópia do banco de A para outro computador.
        a.conn.backup(rusqlite::MAIN_DB, &cc, None).unwrap();
        let c = Banco::abrir(&cc).unwrap();
        config::gravar(&c.conn, "computador_id", &crate::computador::impressao("outro-pc")).unwrap();
        crate::faixas::conferir_computador(&c.conn, crate::computador::atual().as_deref()).unwrap();
        crate::faixas::registrar_caixa_novo(&c.conn, crate::computador::atual().as_deref(), Some("Caixa C")).unwrap();
        let p = para_testes::peca(&c, &ua, "Peça do caixa C");
        let trocar = |rodada: &str| {
            let (mut ma, pt_a) = motor(&ca);
            let (mut mc, pt_c) = motor(&cc);
            let (para_a, para_c) = (format!("127.0.0.1:{pt_a}"), format!("127.0.0.1:{pt_c}"));
            sync::puxar(&ca, &para_c, pt_a).unwrap_or_else(|e| panic!("{rodada}: A puxa da cópia: {e}"));
            sync::puxar(&cc, &para_a, pt_c).unwrap_or_else(|e| panic!("{rodada}: a cópia puxa de A: {e}"));
            assert!(ma.encerrar(Duration::from_secs(20)));
            assert!(mc.encerrar(Duration::from_secs(20)));
        };
        trocar("cópia registrada");
        assert!(crate::produtos::ficha(&a.conn, &p).is_ok());
        // A volta de uma cópia antiga (restauração no mesmo computador).
        let antiga = ca.with_file_name("antiga.db");
        a.conn.backup(rusqlite::MAIN_DB, &antiga, None).unwrap();
        let q = para_testes::peca(&a, &ua, "Feita antes de restaurar");
        // A cópia (caixa C) recebe essa peça antes de A voltar para trás.
        trocar("antes de restaurar");
        let seq: i64 = a.conn.query_row("SELECT MAX(seq) FROM alteracao", [], |l| l.get(0)).unwrap();
        drop(a);
        for fim in ["", "-wal", "-shm"] {
            let mut x = ca.as_os_str().to_owned();
            x.push(fim);
            let _ = std::fs::remove_file(PathBuf::from(x));
        }
        std::fs::copy(&antiga, &ca).unwrap();
        {
            let a = Banco::abrir(&ca).unwrap();
            sync::depois_de_restaurar(&a.conn, seq).unwrap();
        }
        let a = Banco::abrir(&ca).unwrap();
        trocar("banco restaurado");
        assert!(crate::produtos::ficha(&a.conn, &q).is_ok(), "o restaurado recebe de volta o que perdeu");
        for x in [&a, &c] {
            assert!(crate::avisos::do_dia(&x.conn).unwrap().iter().all(|v| v.id != "outra_loja"));
            assert_eq!(gerentes(&x.conn), 1);
        }
    })
    .join()
    .unwrap();
}

/// As telas: "Voltar" na entrada da loja desfaz a tentativa, e o programa
/// aberto no meio de uma tentativa volta para ela (não para a loja nova).
#[test]
fn telas_desfazem_e_retomam_a_juncao() {
    let qml = |nome: &str| {
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../app/qml").join(nome)).unwrap()
    };
    let juntar = qml("TelaJuntar.qml");
    let voltar = juntar.split("text: \"Voltar\"").nth(1).expect("botão Voltar");
    let clique = voltar.lines().find(|l| l.contains("onClicked")).unwrap();
    assert!(clique.contains("tela.desistir()"), "Voltar desfaz a tentativa: {clique}");
    let desistir = juntar.split("function desistir()").nth(1).expect("função desistir");
    assert!(desistir.split("function ").next().unwrap().contains("Api.chamar(\"sync_desistir_juntar\")"));
    let main = qml("Main.qml");
    let iniciar = main.split("function iniciar()").nth(1).unwrap().split("function ").next().unwrap();
    assert!(iniciar.contains("estado.juntando ? \"TelaJuntar\" : \"TelaBoasVindas\""), "{iniciar}");
}

// ---------------------------------------------------------------------------
// A2 (complemento da 0.9.15): a senha da rede só muda com o gerente, ligada
// ou desligada, e sempre com o mínimo de 4 caracteres.
// ---------------------------------------------------------------------------

fn senha_gravada(s: &mut crate::Servico) -> String {
    config::ler(&s.banco_para_teste().conn, "sync_senha").unwrap().unwrap_or_default()
}

fn rede_ligada(s: &mut crate::Servico) -> bool {
    config::ler(&s.banco_para_teste().conn, "sync_ligado").unwrap().as_deref() == Some("1")
}

/// Loja com o gerente Ana (1234), a rede configurada e depois desligada, e a
/// caixa Bia (4321) com Configurações liberada para todos, já entrada.
fn loja_com_caixa_bia() -> crate::Servico {
    let mut s = crate::Servico::em_memoria().unwrap();
    let r = chamar(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
    );
    assert_eq!(r["ok"], true, "{r}");
    let r = chamar(&mut s, "sync_salvar", json!({ "ligado": true, "senha": "rede-da-loja", "enderecos": "" }));
    assert_eq!(r["ok"], true, "{r}");
    let r = chamar(&mut s, "sync_salvar", json!({ "ligado": false, "senha": "", "enderecos": "" }));
    assert_eq!(r["ok"], true, "{r}");
    let r =
        chamar(&mut s, "usuario_salvar", json!({ "nome": "Bia", "senha": "4321", "senha2": "4321", "ativo": true }));
    assert_eq!(r["ok"], true, "{r}");
    let bia = r["dados"]["id"].as_str().unwrap().to_string();
    let r = chamar(&mut s, "gerente_salvar", json!({ "acoes": { "configuracoes": false } }));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(chamar(&mut s, "entrar", json!({ "id": bia, "senha": "4321" }))["ok"], true);
    assert_eq!(senha_gravada(&mut s), "rede-da-loja");
    assert!(!rede_ligada(&mut s));
    s
}

/// Caixa, com a rede desligada: não troca a senha (nem por uma curta, nem
/// apagando com espaços); endereços continuam liberados; nada muda.
#[test]
fn a2_caixa_nao_troca_a_senha_com_a_rede_desligada() {
    let mut s = loja_com_caixa_bia();
    for senha in ["outra-senha", "ab", "x"] {
        let r = chamar(&mut s, "sync_salvar", json!({ "ligado": false, "senha": senha, "enderecos": "" }));
        assert_eq!(r["ok"], false, "caixa trocou a senha da rede para {senha:?} com a rede desligada: {r}");
        assert_eq!(r["campo"], "gerente", "{r}");
        assert_eq!(senha_gravada(&mut s), "rede-da-loja", "a senha gravada mudou ({senha:?})");
    }
    // Só espaços = campo vazio = manter a que está (não apaga a senha).
    let r = chamar(&mut s, "sync_salvar", json!({ "ligado": false, "senha": "   ", "enderecos": "" }));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(senha_gravada(&mut s), "rede-da-loja", "espaços apagaram a senha da rede");
    // Só endereços: continua em Configurações.
    let r = chamar(&mut s, "sync_salvar", json!({ "ligado": false, "senha": "", "enderecos": "10.0.0.5" }));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(senha_gravada(&mut s), "rede-da-loja");
    assert!(!rede_ligada(&mut s));
}

/// Gerente (pela senha presa à chamada), com a rede desligada: troca a
/// senha, mas não por uma curta.
#[test]
fn a2_gerente_troca_a_senha_desligada_com_o_minimo() {
    let mut s = loja_com_caixa_bia();
    let curta = json!({ "ligado": false, "senha": "ab", "enderecos": "" });
    let r = chamar(&mut s, "gerente_liberar", json!({ "senha": "1234", "comando": "sync_salvar", "dados": curta }));
    assert_eq!(r["ok"], true, "{r}");
    let r = chamar(&mut s, "sync_salvar", curta);
    assert_eq!(r["ok"], false, "senha da rede de 2 caracteres gravada com a rede desligada: {r}");
    assert_eq!(r["campo"], "senha", "{r}");
    assert_eq!(senha_gravada(&mut s), "rede-da-loja");
    let boa = json!({ "ligado": false, "senha": "nova-da-rede", "enderecos": "" });
    let r = chamar(&mut s, "gerente_liberar", json!({ "senha": "1234", "comando": "sync_salvar", "dados": boa }));
    assert_eq!(r["ok"], true, "{r}");
    let r = chamar(&mut s, "sync_salvar", boa);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(senha_gravada(&mut s), "nova-da-rede");
    assert!(!rede_ligada(&mut s), "trocar a senha não liga a rede");
    // Ligar com a senha já gravada (campo vazio): só com o gerente; e liga.
    let ligar = json!({ "ligado": true, "senha": "", "enderecos": "" });
    let r = chamar(&mut s, "sync_salvar", ligar.clone());
    assert_eq!(r["campo"], "gerente", "caixa ligou a rede: {r}");
    let r = chamar(&mut s, "gerente_liberar", json!({ "senha": "1234", "comando": "sync_salvar", "dados": ligar }));
    assert_eq!(r["ok"], true, "{r}");
    let r = chamar(&mut s, "sync_salvar", ligar);
    assert_eq!(r["ok"], true, "{r}");
    assert!(rede_ligada(&mut s));
    assert_eq!(senha_gravada(&mut s), "nova-da-rede");
}

/// Senha curta já gravada (banco antigo, ou gravada pelo caminho que esta
/// correção fecha): ligar a rede com ela é recusado, mesmo pelo gerente; com
/// a rede já ligada, nada é forçado (a loja continua funcionando).
#[test]
fn a2_senha_curta_ja_gravada_nao_liga_a_rede() {
    let mut s = loja_com_caixa_bia();
    config::gravar(&s.banco_para_teste().conn, "sync_senha", "ab").unwrap();
    let ligar = json!({ "ligado": true, "senha": "", "enderecos": "" });
    let r = chamar(&mut s, "gerente_liberar", json!({ "senha": "1234", "comando": "sync_salvar", "dados": ligar }));
    assert_eq!(r["ok"], true, "{r}");
    let r = chamar(&mut s, "sync_salvar", ligar);
    assert_eq!(r["ok"], false, "ligou a rede com a senha curta já gravada: {r}");
    assert_eq!(r["campo"], "senha", "{r}");
    assert!(!rede_ligada(&mut s));
    // Rede já ligada com uma senha curta antiga: salvar endereços continua.
    config::gravar(&s.banco_para_teste().conn, "sync_ligado", "1").unwrap();
    let r = chamar(&mut s, "sync_salvar", json!({ "ligado": true, "senha": "", "enderecos": "10.0.0.7" }));
    assert_eq!(r["ok"], true, "nada é forçado com a rede já ligada: {r}");
    assert_eq!(senha_gravada(&mut s), "ab");
}

// ---------------------------------------------------------------------------
// FALHA-01 (complemento): caixa novo, ainda sem usuários, numa rede com duas
// lojas independentes com a mesma senha, recebendo lotes pequenos das duas,
// em ordens diferentes, com o programa reaberto no meio.
// ---------------------------------------------------------------------------

fn produtos_com_nome(c: &Connection, nome: &str) -> i64 {
    c.query_row("SELECT COUNT(*) FROM produto WHERE nome = ?1", [nome], |l| l.get(0)).unwrap()
}

/// `primeira`: a loja de que o caixa novo recebe o primeiro lote (a escolhida).
/// `a_sem_identidade`: a loja A é de antes da 0.9.15 (sem `loja_id`, nem no
/// banco nem no histórico).
fn juncao_com_duas_lojas(primeira: char, a_sem_identidade: bool) {
    let (ca, cb, cn) = (temp("2lojas-a"), temp("2lojas-b"), temp("2lojas-novo"));
    let (a, ua) = loja_em_arquivo(&ca, "Caixa A", "Loja A");
    if a_sem_identidade {
        a.conn
            .execute_batch(
                "DELETE FROM alteracao WHERE tabela = 'config' AND chave = '[\"loja_id\"]';
                 DELETE FROM config WHERE chave = 'loja_id';",
            )
            .unwrap();
        assert!(loja_id(&a.conn).is_empty());
    }
    let (b, ub) = loja_em_arquivo(&cb, "Caixa B", "Loja B");
    for i in 1..=3 {
        para_testes::peca(&a, &ua, &format!("Peça da loja A {i}"));
        para_testes::peca(&b, &ub, &format!("Peça da loja B {i}"));
    }
    // Lotes de uma alteração: o caixa novo fica sem usuários depois do
    // primeiro lote (o usuário é a 2ª alteração destas lojas).
    for x in [&a, &b] {
        config::gravar(&x.conn, "sync_lote_teste", "1").unwrap();
    }
    let n = caixa_novo_em_arquivo(&cn, "Caixa novo");
    config::gravar(&n.conn, "sync_lotes_teste", "1").unwrap();
    crate::loja::comecar_juncao(&n.conn).unwrap();
    drop(n);
    let (mut ma, pt_a) = motor(&ca);
    let (mut mb, pt_b) = motor(&cb);
    let (para_a, para_b) = (format!("127.0.0.1:{pt_a}"), format!("127.0.0.1:{pt_b}"));
    let (escolhida, outra, para_escolhida, para_outra, banco_escolhida, banco_outra) =
        if primeira == 'A' { ("A", "B", &para_a, &para_b, &a, &b) } else { ("B", "A", &para_b, &para_a, &b, &a) };
    let mut aceitas_da_outra = 0;
    let mut terminou = false;
    for volta in 0..200 {
        if volta == 3 {
            // O programa do caixa novo fecha e abre no meio da entrada.
            drop(Banco::abrir(&cn).unwrap());
        }
        let r = sync::puxar(&cn, para_escolhida, 48799);
        let (_, total) = r.unwrap_or_else(|e| panic!("loja {escolhida} (a escolhida), volta {volta}: {e}"));
        if volta == 0 {
            // A janela de risco existe: depois do 1º lote da loja escolhida, o
            // caixa novo ainda não tem nenhum usuário.
            let x = Banco::abrir(&cn).unwrap();
            assert_eq!(usuarios::quantidade(&x.conn).unwrap(), 0, "o teste não chegou à janela de risco");
        }
        match sync::puxar(&cn, para_outra, 48799) {
            Ok(_) => aceitas_da_outra += 1,
            // C8: recusada por ser de outra loja (não por falta de rede).
            Err(e) => assert!(
                e.to_string().contains("outra loja"),
                "loja {outra}, volta {volta}: devia ser a recusa por outra loja: {e}"
            ),
        }
        if total == 0 && volta > 0 {
            terminou = true;
            break;
        }
    }
    assert!(ma.encerrar(Duration::from_secs(20)));
    assert!(mb.encerrar(Duration::from_secs(20)));
    assert!(terminou, "o caixa novo não terminou de receber a loja {escolhida}");
    let n = Banco::abrir(&cn).unwrap();
    let rotulo = format!("escolhida {escolhida}");
    // Tudo o que estiver errado, junto (para o registro mostrar o tamanho do estrago).
    let mut erros: Vec<String> = Vec::new();
    let mut conferir = |ok: bool, o_que: String| {
        if !ok {
            erros.push(o_que);
        }
    };
    conferir(aceitas_da_outra == 0, format!("{aceitas_da_outra} puxadas da loja {outra} foram aceitas"));
    for i in 1..=3 {
        let dela = produtos_com_nome(&n.conn, &format!("Peça da loja {outra} {i}"));
        conferir(dela == 0, format!("peça {i} da loja {outra} entrou ({dela})"));
        let minha = produtos_com_nome(&n.conn, &format!("Peça da loja {escolhida} {i}"));
        conferir(minha == 1, format!("peça {i} da loja {escolhida}: {minha}"));
    }
    let usuarios_n = usuarios::quantidade(&n.conn).unwrap();
    conferir(usuarios_n == 1, format!("{usuarios_n} usuários no caixa novo (devia ser só o gerente da escolhida)"));
    let gerentes_n = gerentes(&n.conn);
    conferir(gerentes_n == 1, format!("{gerentes_n} gerentes no caixa novo"));
    let u_escolhida: String = banco_escolhida.conn.query_row("SELECT id FROM usuario", [], |l| l.get(0)).unwrap();
    let u_outra: String = banco_outra.conn.query_row("SELECT id FROM usuario", [], |l| l.get(0)).unwrap();
    let tem = |id: &str| -> bool {
        n.conn.query_row("SELECT EXISTS (SELECT 1 FROM usuario WHERE id = ?1)", [id], |l| l.get(0)).unwrap()
    };
    conferir(tem(&u_escolhida), "o gerente da loja escolhida não chegou".into());
    conferir(!tem(&u_outra), format!("o gerente da loja {outra} entrou no caixa novo"));
    conferir(
        loja_id(&n.conn) == loja_id(&banco_escolhida.conn),
        format!(
            "identidade da loja no caixa novo: {:?} (escolhida {:?})",
            loja_id(&n.conn),
            loja_id(&banco_escolhida.conn)
        ),
    );
    let nome_loja = config::nome_loja(&n.conn).unwrap();
    conferir(nome_loja == format!("Loja {escolhida}"), format!("nome da loja no caixa novo: {nome_loja:?}"));
    let cruzadas = crate::faixas::cruzadas(&n.conn).unwrap();
    conferir(cruzadas.is_empty(), format!("faixas cruzadas: {}", cruzadas.len()));
    let disp_outra = config::ler(&banco_outra.conn, "dispositivo_id").unwrap().unwrap();
    let dela: i64 =
        n.conn.query_row("SELECT COUNT(*) FROM alteracao WHERE dispositivo = ?1", [&disp_outra], |l| l.get(0)).unwrap();
    conferir(dela == 0, format!("{dela} alterações da loja {outra} no caixa novo"));
    let aviso = crate::avisos::do_dia(&n.conn).unwrap().iter().any(|v| v.id == "outra_loja");
    conferir(aviso, "sem o aviso \"Troca de dados recusada\" no caixa novo".into());
    // A outra loja também não recebeu nada (nem gerente, nem peça).
    conferir(gerentes(&banco_outra.conn) == 1, format!("gerentes na loja {outra}: {}", gerentes(&banco_outra.conn)));
    conferir(
        produtos_com_nome(&banco_outra.conn, &format!("Peça da loja {escolhida} 1")) == 0,
        format!("peça da loja {escolhida} entrou na loja {outra}"),
    );
    assert!(erros.is_empty(), "{rotulo}:\n  - {}", erros.join("\n  - "));
    // Reabrindo o caixa novo (agora com o gerente), a loja de fora continua recusada.
    drop(n);
    let (mut ma, pt_a) = motor(&ca);
    let (mut mb, pt_b) = motor(&cb);
    let (para_a, para_b) = (format!("127.0.0.1:{pt_a}"), format!("127.0.0.1:{pt_b}"));
    let (para_escolhida, para_outra) = if primeira == 'A' { (&para_a, &para_b) } else { (&para_b, &para_a) };
    let r = sync::puxar(&cn, para_outra, 48799);
    assert!(recusa_de_outra_loja(&r), "{rotulo}: depois de reabrir, a outra loja continua fora: {r:?}");
    assert!(sync::puxar(&cn, para_escolhida, 48799).is_ok(), "{rotulo}: e a escolhida continua");
    assert!(ma.encerrar(Duration::from_secs(20)));
    assert!(mb.encerrar(Duration::from_secs(20)));
}

#[test]
fn falha01_caixa_novo_com_duas_lojas_primeiro_a() {
    thread::spawn(|| juncao_com_duas_lojas('A', false)).join().unwrap();
}

#[test]
fn falha01_caixa_novo_com_duas_lojas_primeiro_b() {
    thread::spawn(|| juncao_com_duas_lojas('B', false)).join().unwrap();
}

/// A loja escolhida é de antes da identidade da loja: a entrada funciona (a
/// escolha fica pela origem) e a outra loja continua de fora.
#[test]
fn falha01_caixa_novo_com_loja_antiga_escolhida() {
    thread::spawn(|| juncao_com_duas_lojas('A', true)).join().unwrap();
}

// ---------------------------------------------------------------------------
// A4 (complemento da 0.9.15): "Voltar" na entrada só desfaz uma tentativa de
// verdade, numa transação só, sem apagar nada que seja deste computador.
// ---------------------------------------------------------------------------

/// Tudo o que a desistência poderia mexer: configurações, alterações,
/// faixas, pares e dados, numa fotografia comparável.
fn retrato(c: &Connection) -> String {
    let mut partes = Vec::new();
    for sql in [
        "SELECT group_concat(chave || '=' || valor, '|') FROM (SELECT chave, valor FROM config ORDER BY chave)",
        "SELECT group_concat(id, '|') FROM (SELECT id FROM alteracao ORDER BY seq)",
        "SELECT group_concat(dispositivo || ':' || inicio || '-' || fim, '|') FROM (SELECT * FROM faixa_codigo ORDER BY rowid)",
        "SELECT group_concat(dispositivo, '|') FROM (SELECT dispositivo FROM sync_par ORDER BY dispositivo)",
        "SELECT group_concat(id || ':' || nome, '|') FROM (SELECT id, nome FROM produto ORDER BY id)",
        "SELECT group_concat(id || ':' || nome, '|') FROM (SELECT id, nome FROM pessoa ORDER BY id)",
    ] {
        partes.push(c.query_row(sql, [], |l| l.get::<_, Option<String>>(0)).unwrap().unwrap_or_default());
    }
    partes.join("\n")
}

/// Caixa novo num arquivo, no meio de uma entrada: a senha e o endereço da
/// tentativa, parte da outra loja aplicada (sem usuários) e preferências
/// deste computador. Devolve o serviço e a pasta.
fn juncao_pela_metade(nome: &str) -> (crate::Servico, PathBuf) {
    let db = temp(nome);
    let pasta = db.parent().unwrap().to_path_buf();
    let mut s = crate::Servico::abrir_para_teste(&db, false).unwrap();
    let r = chamar(&mut s, "aparencia_salvar", json!({ "escuro": true }));
    assert_eq!(r["ok"], true, "{r}");
    let r = chamar(
        &mut s,
        "sync_juntar",
        json!({ "caixa": "Caixa 2", "senha": "rede-da-loja", "enderecos": "192.168.0.10" }),
    );
    assert_eq!(r["ok"], true, "{r}");
    let (outra, u) = para_testes::loja("Caixa da outra loja");
    para_testes::peca(&outra, &u, "Peça da outra loja");
    let (lista, _) = sync::alteracoes_desde(&outra.conn, 0, 10_000_000).unwrap();
    let sem_usuarios: Vec<_> = lista.into_iter().filter(|a| a.tabela != "usuario").collect();
    sync::aplicar(&mut s.banco_para_teste().conn, &sem_usuarios).unwrap();
    // Backups deste computador (de antes) na pasta.
    std::fs::create_dir_all(pasta.join("backups")).unwrap();
    std::fs::write(pasta.join("backups").join("rustinho-20261001-120000-caixa2.rbk"), b"backup de antes").unwrap();
    (s, pasta)
}

/// Sem tentativa nenhuma (banco novo, sem marca e sem nada de outro caixa):
/// "Voltar" não mexe em nada.
#[test]
fn a4_desistir_sem_tentativa_nao_mexe_em_nada() {
    let db = temp("a4-sem-tentativa");
    let mut s = crate::Servico::abrir_para_teste(&db, false).unwrap();
    assert_eq!(chamar(&mut s, "aparencia_salvar", json!({ "escuro": true }))["ok"], true);
    config::gravar(&s.banco_para_teste().conn, "caixa_nome", "Caixa 7").unwrap();
    let antes = retrato(&s.banco_para_teste().conn);
    let r = chamar(&mut s, "sync_desistir_juntar", json!({}));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(r["dados"]["saiu"], 0, "{r}");
    assert_eq!(retrato(&s.banco_para_teste().conn), antes, "nada mudou sem uma tentativa");
}

/// Com a tentativa: uma falha no meio da limpeza desfaz tudo (nada sai);
/// depois sai só o que veio da tentativa, as preferências e os backups deste
/// computador ficam; e repetir não faz mais nada.
#[test]
fn a4_desistir_atomico_e_sem_efeito_repetido() {
    let (mut s, pasta) = juncao_pela_metade("a4-atomico");
    let antes = retrato(&s.banco_para_teste().conn);
    assert!(antes.contains("Peça da outra loja"));
    config::gravar(&s.banco_para_teste().conn, "juntar_falhar_teste", "1").unwrap();
    let antes = retrato(&s.banco_para_teste().conn);
    let r = chamar(&mut s, "sync_desistir_juntar", json!({}));
    assert_eq!(r["ok"], false, "a falha simulada aparece: {r}");
    assert_eq!(retrato(&s.banco_para_teste().conn), antes, "falha no meio: nada saiu (transação desfeita)");
    s.banco_para_teste().conn.execute("DELETE FROM config WHERE chave = 'juntar_falhar_teste'", []).unwrap();
    let r = chamar(&mut s, "sync_desistir_juntar", json!({}));
    assert_eq!(r["ok"], true, "{r}");
    assert!(r["dados"]["saiu"].as_i64().unwrap() > 0, "{r}");
    {
        let c = &s.banco_para_teste().conn;
        assert_eq!(contar(c, "SELECT COUNT(*) FROM produto"), 0, "o que veio da tentativa saiu");
        assert_eq!(config::ler(c, "tela_escura").unwrap().as_deref(), Some("1"), "preferência deste computador fica");
        assert_eq!(config::nome_caixa(c).unwrap(), "Caixa 2", "o nome deste caixa fica");
        assert!(config::ler(c, "dispositivo_id").unwrap().is_some_and(|d| !d.is_empty()));
    }
    assert!(pasta.join("backups").join("rustinho-20261001-120000-caixa2.rbk").exists(), "o backup de antes ficou");
    let depois = retrato(&s.banco_para_teste().conn);
    let r = chamar(&mut s, "sync_desistir_juntar", json!({}));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(r["dados"]["saiu"], 0, "{r}");
    assert_eq!(retrato(&s.banco_para_teste().conn), depois, "repetir não faz mais nada");
    // Banco já configurado: a desistência é recusada e nada muda.
    let r = chamar(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja nova", "caixa": "Caixa 2", "nome": "Zé", "senha": "1234", "senha2": "1234" }),
    );
    assert_eq!(r["ok"], true, "{r}");
    let configurado = retrato(&s.banco_para_teste().conn);
    let r = chamar(&mut s, "sync_desistir_juntar", json!({}));
    assert_eq!(r["ok"], false, "banco configurado: recusada: {r}");
    assert_eq!(retrato(&s.banco_para_teste().conn), configurado);
}

/// Um registro que veio da outra loja e também foi mexido neste computador:
/// não dá para saber de quem é. Nada é apagado; a desistência é recusada
/// (e a loja nova continua bloqueada até a recuperação).
#[test]
fn a4_desistir_com_origem_incerta_nao_apaga() {
    let (mut s, _pasta) = juncao_pela_metade("a4-incerta");
    s.banco_para_teste().conn.execute("UPDATE produto SET local = 'Prateleira A'", []).unwrap();
    let antes = retrato(&s.banco_para_teste().conn);
    let r = chamar(&mut s, "sync_desistir_juntar", json!({}));
    assert_eq!(r["ok"], false, "origem incerta: recusada: {r}");
    assert_eq!(retrato(&s.banco_para_teste().conn), antes, "nada apagado");
    let r = chamar(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja nova", "caixa": "Caixa 2", "nome": "Zé", "senha": "1234", "senha2": "1234" }),
    );
    assert_eq!(r["ok"], false, "a loja nova não nasce em cima disso: {r}");
}

/// 0915-K (revisão dirigida): salvar a troca de dados é tudo ou nada — com um
/// endereço inválido, nem as interfaces liberadas ficam gravadas.
#[test]
fn k_salvar_a_troca_de_dados_e_tudo_ou_nada() {
    let mut s = loja_com_caixa_bia();
    let antes = config::ler(&s.banco_para_teste().conn, "sync_interfaces").unwrap().unwrap_or_default();
    let r = chamar(
        &mut s,
        "sync_salvar",
        json!({ "ligado": false, "senha": "", "enderecos": "isto-nao-e-endereco", "interfaces": "tun0" }),
    );
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(r["campo"], "enderecos", "{r}");
    let depois = config::ler(&s.banco_para_teste().conn, "sync_interfaces").unwrap().unwrap_or_default();
    assert_eq!(depois, antes, "as interfaces ficaram gravadas pela metade");
}
