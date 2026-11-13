//! 7.6 (H09, ARQ-08): as consultas fora da linha da janela pelo serviço, com
//! o banco num arquivo (como no computador da loja).
//!
//! - T1: a mesma resposta do `chamar`, byte a byte;
//! - T2: a lista é fechada (só leituras sem gerente) e o resto é recusado
//!   sem rodar; sessão, usuário desativado e senha do gerente valem igual ou
//!   menos, nunca mais;
//! - T4: leitura e gravação juntas;
//! - T6/T7: restaurar um backup fecha e reabre a conexão, e nenhuma leitura
//!   acontece enquanto o banco é trocado;
//! - modo seguro: a consulta funciona como o `chamar`.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::consulta::{Evento, AGUARDE};
use crate::Servico;

fn pasta_nova(nome: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rustinho-consulta-{nome}-{}-{}",
        std::process::id(),
        crate::tempo::novo_id()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn chamar(s: &mut Servico, cmd: &str, d: Value) -> Value {
    serde_json::from_str(&crate::apoio_chave::chamar(crate::api::NAO_REPETIR, cmd, &d.to_string(), |x| {
        s.chamar(cmd, x)
    }))
    .unwrap()
}

fn ok(s: &mut Servico, cmd: &str, d: Value) -> Value {
    let r = chamar(s, cmd, d);
    assert_eq!(r["ok"], true, "{cmd}: {r}");
    r["dados"].clone()
}

/// A resposta crua (texto) de uma consulta fora da linha da janela.
fn consultar_texto(s: &mut Servico, canal: &str, cmd: &str, d: &Value) -> String {
    let (tx, rx) = mpsc::channel::<String>();
    s.consultar(
        canal,
        cmd,
        &d.to_string(),
        Box::new(move |r| {
            let _ = tx.send(r);
        }),
    );
    rx.recv_timeout(Duration::from_secs(60)).expect("a consulta respondeu")
}

fn consultar(s: &mut Servico, canal: &str, cmd: &str, d: Value) -> Value {
    serde_json::from_str(&consultar_texto(s, canal, cmd, &d)).unwrap()
}

fn id_de(s: &mut Servico, nome: &str) -> String {
    let lista = ok(s, "usuarios_entrada", json!({}));
    lista.as_array().unwrap().iter().find(|u| u["nome"] == nome).unwrap()["id"].as_str().unwrap().to_string()
}

fn entrar(s: &mut Servico, nome: &str, senha: &str) {
    ok(s, "sair", json!({}));
    let id = id_de(s, nome);
    ok(s, "entrar", json!({ "id": id, "senha": senha }));
}

fn peca(s: &mut Servico, nome: &str, marca: &str) -> String {
    let mut f = ok(s, "produto_novo", json!({}));
    f["nome"] = json!(nome);
    f["marca"] = json!(marca);
    f["custo"] = json!("10");
    f["estoqueInicial"] = json!("3");
    f["precos"][0]["preco"] = json!("25,00");
    f["precos"][0]["lucro"] = json!("");
    ok(s, "produto_salvar", f)["id"].as_str().unwrap().to_string()
}

/// Loja num arquivo: Ana (gerente, 1234), Bia (caixa, 1111) e `n` peças com
/// nomes variados. Ana conectada.
fn loja(nome: &str, n: usize) -> (PathBuf, PathBuf, Servico) {
    let pasta = pasta_nova(nome);
    let db = pasta.join("rustinho.db");
    let mut s = Servico::abrir_para_teste(&db, false).unwrap();
    ok(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
    );
    ok(
        &mut s,
        "usuario_salvar",
        json!({ "id": "", "nome": "Bia", "senha": "1111", "senha2": "1111", "gerente": false, "ativo": true }),
    );
    for i in 0..n {
        let nome = match i % 4 {
            0 => format!("Filtro de óleo {i:03}"),
            1 => format!("PASTILHA DE FREIO {i:03}"),
            2 => format!("Correia dentada 8PK{i:04}"),
            _ => format!("Água do radiador {i:03}"),
        };
        peca(&mut s, &nome, if i % 2 == 0 { "BOSCH" } else { "" });
    }
    (pasta, db, s)
}

/// As consultas da equivalência: textos de vários tipos, inativas, partes.
fn pedidos_de_busca() -> Vec<Value> {
    let mut v = Vec::new();
    for texto in ["", "filtro", "pastilha freio", "8PK", "8pk0002", "agua", "bosch", "nada que exista", "0", "a"] {
        for inativas in [false, true] {
            v.push(json!({ "texto": texto, "inativas": inativas, "limite": 100, "deslocamento": 0 }));
        }
        v.push(json!({ "texto": texto, "limite": 7, "deslocamento": 5 }));
    }
    v.push(json!({}));
    v.push(json!({ "texto": "filtro", "offset": 3 }));
    v
}

/// T1: para cada consulta da lista, o mesmo JSON do `chamar`, como gerente e
/// como caixa, antes e depois de gravar.
#[test]
fn t1_mesma_resposta_que_o_chamar() {
    let (pasta, _db, mut s) = loja("t1", 48);
    for quem in [("Ana", "1234"), ("Bia", "1111")] {
        entrar(&mut s, quem.0, quem.1);
        for rodada in 0..2 {
            for cmd in crate::api::CONSULTAS_FORA_DA_JANELA {
                for d in pedidos_de_busca() {
                    let normal = s.chamar(cmd, &d.to_string());
                    let fora = consultar_texto(&mut s, "t1", cmd, &d);
                    assert_eq!(fora, normal, "{} {cmd} {d}: a consulta fora deu outra resposta", quem.0);
                    assert_eq!(serde_json::from_str::<Value>(&normal).unwrap()["ok"], true, "{cmd} {d}: {normal}");
                }
            }
            if rodada == 0 && quem.0 == "Ana" {
                // Grava pela linha da janela e consulta de novo: a leitura
                // de fora vê o que foi confirmado.
                peca(&mut s, "Filtro de ar gravado agora", "MANN");
            }
        }
    }
    drop(s);
    let _ = std::fs::remove_dir_all(&pasta);
}

/// T2: a lista é fechada. Só comandos de classe `Leitura` de um domínio:
/// nada do gerente, nada sem sessão, nenhuma gravação.
#[test]
fn t2_lista_fechada_so_de_leituras() {
    use crate::despacho::Classe;
    for c in crate::api::CONSULTAS_FORA_DA_JANELA {
        assert_eq!(crate::api::classe(c), Some(Classe::Leitura), "{c}: só leitura de qualquer usuário conectado");
        assert!(crate::api::so_leitura(c), "{c}");
        assert!(!crate::api::sem_sessao(c), "{c}");
        assert!(crate::api::classe_do_comando(c) == Some("leitura"), "{c}");
    }
}

/// T2: tudo o que não está na lista é recusado na hora, sem rodar — também
/// para o gerente, e também as gravações com dados certos (nada é gravado).
#[test]
fn t2_o_que_nao_esta_na_lista_e_recusado_sem_rodar() {
    let (pasta, db, mut s) = loja("t2-recusa", 2);
    let pecas_antes: i64 =
        rusqlite::Connection::open(&db).unwrap().query_row("SELECT COUNT(*) FROM produto", [], |l| l.get(0)).unwrap();
    let mut recusados = 0;
    for c in crate::api::comandos() {
        if crate::api::CONSULTAS_FORA_DA_JANELA.contains(&c) {
            continue;
        }
        let r = consultar(&mut s, "t2", c, json!({}));
        assert_eq!(r["ok"], false, "{c} passou pela consulta: {r}");
        assert_eq!(
            r["erro"],
            format!("Erro interno: \"{c}\" não é uma consulta que roda fora da linha da janela"),
            "{c}: {r}"
        );
        recusados += 1;
    }
    assert!(recusados > 100, "{recusados}");
    // Uma gravação com dados certos (a ficha da peça) não grava nada.
    let mut f = ok(&mut s, "produto_novo", json!({}));
    f["nome"] = json!("Peça que não pode entrar pela consulta");
    let r = consultar(&mut s, "t2", "produto_salvar", f);
    assert_eq!(r["ok"], false, "{r}");
    let pecas_depois: i64 =
        rusqlite::Connection::open(&db).unwrap().query_row("SELECT COUNT(*) FROM produto", [], |l| l.get(0)).unwrap();
    assert_eq!(pecas_depois, pecas_antes, "nada gravado");
    // Nenhuma tarefa de leitura foi criada: nada chegou a rodar.
    assert!(s.leitor_para_teste().is_none());
    drop(s);
    let _ = std::fs::remove_dir_all(&pasta);
}

/// T2: sem ninguém conectado e com o usuário desativado em outro caixa, a
/// consulta responde como o `chamar` (a tela volta para a entrada).
#[test]
fn t2_sessao_e_usuario_desativado_como_o_chamar() {
    let (pasta, db, mut s) = loja("t2-sessao", 2);
    let d = json!({ "texto": "filtro" });
    ok(&mut s, "sair", json!({}));
    let normal = s.chamar("produtos_buscar", &d.to_string());
    assert_eq!(consultar_texto(&mut s, "t2", "produtos_buscar", &d), normal);
    assert!(normal.contains("\"campo\":\"sessao\""), "{normal}");

    entrar(&mut s, "Bia", "1111");
    assert_eq!(consultar(&mut s, "t2", "produtos_buscar", d.clone())["ok"], true);
    // Desativada "em outro caixa" (direto no banco).
    let c = rusqlite::Connection::open(&db).unwrap();
    crate::banco::registrar_funcoes(&c).unwrap();
    c.execute("UPDATE usuario SET ativo = 0 WHERE nome = 'Bia'", []).unwrap();
    drop(c);
    let r = consultar(&mut s, "t2", "produtos_buscar", d.clone());
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(r["campo"], "sessao", "{r}");
    assert_eq!(r["erro"], "Seu usuário foi desativado. Fale com o gerente.");
    // A sessão acabou: o chamar também recusa (sem sessão).
    let normal = s.chamar("produtos_buscar", &d.to_string());
    assert_eq!(consultar_texto(&mut s, "t2", "produtos_buscar", &d), normal);
    drop(s);
    let _ = std::fs::remove_dir_all(&pasta);
}

/// T2: a senha do gerente digitada (consulta de uma tela do gerente, ou para
/// uma gravação) vale no `chamar`, mas não na consulta fora da linha da
/// janela: lá só vale o que vale sem senha. A consulta nunca pode mais.
#[test]
fn t2_senha_do_gerente_digitada_nao_vale_na_consulta() {
    use crate::despacho::Acesso;
    let (pasta, _db, mut s) = loja("t2-senha", 1);
    entrar(&mut s, "Bia", "1111");
    ok(&mut s, "gerente_liberar", json!({ "senha": "1234", "acao": "resultados" }));
    // No chamar (linha da janela), a consulta dos resultados está liberada.
    let r = ok(&mut s, "permissao", json!({ "acao": "resultados", "consulta": true }));
    assert_eq!(r["pode"], true, "{r}");
    // Nas regras da consulta de fora, não.
    let conn = crate::banco::conexao_de_leitura(&pasta.join("rustinho.db")).unwrap();
    let mut a = crate::api::acesso_da_consulta_para_teste(&s);
    assert!(!a.pode_ler(&conn, "resultados").unwrap());
    assert!(!a.pode_sem_senha(&conn, "resultados").unwrap());
    let e = a.exigir(&conn, "resultados").unwrap_err();
    assert_eq!(e.nome_do_campo(), Some("gerente"), "{e}");
    // O gerente pode, nas duas.
    entrar(&mut s, "Ana", "1234");
    let mut a = crate::api::acesso_da_consulta_para_teste(&s);
    assert!(a.pode_ler(&conn, "resultados").unwrap());
    assert!(a.exigir(&conn, "precos").is_ok());
    drop(conn);
    drop(s);
    let _ = std::fs::remove_dir_all(&pasta);
}

/// T4: leitura e gravação juntas. A linha da janela grava peça a peça; entre
/// uma e outra pede uma consulta (cada uma no seu canal, para todas
/// responderem). Nenhum erro (nada de "database is locked"), e cada resposta
/// é um retrato válido: o total nunca volta para trás.
#[test]
fn t4_leitura_e_gravacao_juntas() {
    let (pasta, _db, mut s) = loja("t4", 10);
    let (tx, rx) = mpsc::channel::<String>();
    let gravar = 80;
    for i in 0..gravar {
        let tx = tx.clone();
        s.consultar(
            &format!("t4-{i}"),
            "produtos_buscar",
            &json!({ "texto": "", "limite": 50 }).to_string(),
            Box::new(move |r| {
                let _ = tx.send(r);
            }),
        );
        peca(&mut s, &format!("Peça gravada junto {i:03}"), "");
    }
    drop(tx);
    let mut totais = Vec::new();
    while let Ok(r) = rx.recv_timeout(Duration::from_secs(60)) {
        let v: Value = serde_json::from_str(&r).unwrap();
        assert_eq!(v["ok"], true, "{v}");
        totais.push(v["dados"]["total"].as_i64().unwrap());
    }
    assert_eq!(totais.len(), gravar, "todas responderam");
    assert!(totais.windows(2).all(|w| w[0] <= w[1]), "o total nunca volta: {totais:?}");
    assert!(*totais.first().unwrap() >= 10 && *totais.last().unwrap() <= 10 + gravar as i64, "{totais:?}");
    let depois = consultar(&mut s, "t4-fim", "produtos_buscar", json!({ "texto": "" }));
    assert_eq!(depois["dados"]["total"], 10 + gravar as i64);
    drop(s);
    let _ = std::fs::remove_dir_all(&pasta);
}

/// Entrar, sair e o usuário desativado: nenhuma resposta pedida numa sessão
/// chega depois dela (a tarefa esquece o que estava pedido).
#[test]
fn sessao_que_muda_esquece_as_consultas_pedidas() {
    let (pasta, _db, mut s) = loja("sessao-muda", 3);
    // Cria a tarefa e a deixa ocupada com uma leitura que espera um sinal.
    assert_eq!(consultar(&mut s, "a", "produtos_buscar", json!({}))["ok"], true);
    let alca = s.leitor_para_teste().unwrap().alca();
    let (comecou, espera_comecar) = mpsc::channel::<()>();
    let (solta, portao) = mpsc::channel::<()>();
    let (tx, rx) = mpsc::channel::<String>();
    let tx2 = tx.clone();
    alca.pedir(
        "a",
        Box::new(move |_| {
            comecou.send(()).unwrap();
            portao.recv().unwrap();
            "pedida antes de sair".into()
        }),
        Box::new(move |r| {
            let _ = tx2.send(r);
        }),
    );
    espera_comecar.recv_timeout(Duration::from_secs(10)).unwrap();
    // Uma consulta de verdade fica esperando atrás dela.
    let tx3 = tx.clone();
    s.consultar(
        "b",
        "produtos_buscar",
        "{}",
        Box::new(move |r| {
            let _ = tx3.send(r);
        }),
    );
    ok(&mut s, "sair", json!({}));
    solta.send(()).unwrap();
    assert!(rx.recv_timeout(Duration::from_millis(800)).is_err(), "nada pedido antes de sair chegou depois");
    drop(s);
    let _ = std::fs::remove_dir_all(&pasta);
}

/// T6/T7: restaurar um backup. Enquanto o banco é trocado, a tarefa de
/// leitura está parada e sem conexão: toda leitura acabou antes da pausa ou
/// começou depois dela, e a troca (`conn.restore`) acontece dentro da pausa.
/// Quem pede no meio recebe "aguarde". Depois, a conexão é outra e lê o banco
/// restaurado.
#[test]
fn t6_t7_restaurar_backup_para_a_leitura_e_reabre_a_conexao() {
    let _vez = crate::backup::testes::um_por_vez();
    let (pasta, db, mut s) = loja("t6", 4);
    peca(&mut s, "PECA DE ANTES DO BACKUP", "");
    let feito = crate::backup::fazer(&db, false).unwrap();
    peca(&mut s, "PECA DE DEPOIS DO BACKUP", "");
    let r = consultar(&mut s, "t6", "produtos_buscar", json!({ "texto": "depois do backup" }));
    assert_eq!(r["dados"]["itens"].as_array().unwrap().len(), 1, "{r}");
    assert_eq!(s.leitor_para_teste().unwrap().aberturas(), 1);

    // Uma "tela" pedindo leituras sem parar, de outra linha, durante a restauração.
    let alca = s.leitor_para_teste().unwrap().alca();
    let parar = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (tx, rx) = mpsc::channel::<String>();
    let martelo = {
        let parar = parar.clone();
        std::thread::spawn(move || {
            let mut i = 0u64;
            while !parar.load(std::sync::atomic::Ordering::SeqCst) {
                let tx = tx.clone();
                alca.pedir(
                    &format!("martelo-{i}"),
                    Box::new(|c: &mut rusqlite::Connection| {
                        let n: i64 = c.query_row("SELECT COUNT(*) FROM produto", [], |l| l.get(0)).unwrap();
                        format!("{{\"ok\":true,\"dados\":{n}}}")
                    }),
                    Box::new(move |r| {
                        let _ = tx.send(r);
                    }),
                );
                i += 1;
                std::thread::sleep(Duration::from_millis(2));
            }
            alca
        })
    };
    std::thread::sleep(Duration::from_millis(100));
    let r = ok(&mut s, "backup_restaurar", json!({ "arquivo": feito.arquivo, "nuvem": false, "senha": "1234" }));
    assert!(r.is_object(), "{r}");
    let troca = crate::backup::queda_de_teste::TROCA_EM.with(|t| t.get()).expect("a troca aconteceu");
    std::thread::sleep(Duration::from_millis(100));
    parar.store(true, std::sync::atomic::Ordering::SeqCst);
    let alca = martelo.join().unwrap();
    let respostas: Vec<String> = rx.try_iter().collect();

    let eventos = alca.eventos();
    let pausou = eventos.iter().find_map(|e| if let Evento::Pausou(t) = e { Some(*t) } else { None });
    let retomou = eventos.iter().find_map(|e| if let Evento::Retomou(t) = e { Some(*t) } else { None });
    let (pausou, retomou) = (pausou.expect("a leitura parou"), retomou.expect("a leitura voltou"));
    assert!(pausou < troca && troca < retomou, "a troca aconteceu dentro da pausa");
    let mut antes = 0;
    let mut depois = 0;
    for e in &eventos {
        if let Evento::Leu { inicio, fim, .. } = e {
            assert!(*fim <= pausou || *inicio >= retomou, "uma leitura cruzou a troca do banco: {e:?}");
            if *fim <= pausou {
                antes += 1;
            } else {
                depois += 1;
            }
        }
    }
    assert!(antes > 0, "o teste leu antes da troca ({eventos:?})");
    let fechou = eventos.iter().position(|e| *e == Evento::Fechou).expect("a conexão fechou");
    let pos_pausa = eventos.iter().position(|e| matches!(e, Evento::Pausou(_))).unwrap();
    assert!(fechou < pos_pausa, "fechou antes da troca");
    assert!(respostas.iter().any(|r| r.contains(AGUARDE)), "quem pediu durante a troca recebeu aguarde");

    // Depois: entra de novo (os usuários são os do backup) e lê o restaurado,
    // por uma conexão nova.
    let ana = id_de(&mut s, "Ana");
    ok(&mut s, "entrar", json!({ "id": ana, "senha": "1234" }));
    let r = consultar(&mut s, "t6", "produtos_buscar", json!({ "texto": "depois do backup" }));
    assert_eq!(r["dados"]["itens"].as_array().unwrap().len(), 0, "a peça de depois do backup sumiu: {r}");
    let r = consultar(&mut s, "t6", "produtos_buscar", json!({ "texto": "antes do backup" }));
    assert_eq!(r["dados"]["itens"].as_array().unwrap().len(), 1, "{r}");
    assert!(s.leitor_para_teste().unwrap().aberturas() >= 2, "conexão nova depois da troca");
    let eventos = s.leitor_para_teste().unwrap().alca().eventos();
    let abriu_depois = eventos.iter().rposition(|e| matches!(e, Evento::Abriu(_))).unwrap();
    let retomou_em = eventos.iter().position(|e| matches!(e, Evento::Retomou(_))).unwrap();
    assert!(
        abriu_depois > retomou_em,
        "a conexão foi aberta de novo depois da troca ({depois} leituras do martelo depois)"
    );
    drop(s);
    let _ = std::fs::remove_dir_all(&pasta);
}

/// Entrar numa loja (`sync_juntar`) e desistir dela também param a leitura e
/// trocam a conexão.
#[test]
fn juntar_e_desistir_trocam_a_conexao() {
    let (pasta, _db, mut s) = loja("juntar", 1);
    assert_eq!(consultar(&mut s, "j", "produtos_buscar", json!({}))["ok"], true);
    assert_eq!(s.leitor_para_teste().unwrap().aberturas(), 1);
    // Com usuários, a junção é recusada; mesmo assim a leitura parou e a
    // conexão foi trocada (a pausa vem antes do comando, como na junção de
    // verdade, num caixa novo).
    let r = chamar(&mut s, "sync_juntar", json!({ "caixa": "Caixa 2", "senha": "segredo-da-loja", "enderecos": "" }));
    assert_eq!(r["ok"], false, "{r}");
    let r = chamar(&mut s, "sync_desistir_juntar", json!({}));
    assert_eq!(r["ok"], false, "{r}");
    let pausas =
        s.leitor_para_teste().unwrap().alca().eventos().iter().filter(|e| matches!(e, Evento::Pausou(_))).count();
    assert_eq!(pausas, 2, "uma pausa para cada um");
    assert_eq!(consultar(&mut s, "j", "produtos_buscar", json!({}))["ok"], true);
    assert_eq!(s.leitor_para_teste().unwrap().aberturas(), 2, "conexão nova depois");
    drop(s);
    let _ = std::fs::remove_dir_all(&pasta);
}

/// Modo seguro (banco com defeito): a consulta lê pela conexão só de leitura e
/// responde como o `chamar`.
#[test]
fn modo_seguro_a_consulta_responde_como_o_chamar() {
    let (pasta, db, s) = loja("seguro", 6);
    drop(s);
    crate::testes_modo_seguro::estragar_lista_livre(&db);
    let mut s = Servico::abrir_para_teste(&db, false).unwrap();
    let e = ok(&mut s, "estado", json!({}));
    assert!(!e["modoSeguro"].is_null(), "o banco abriu em modo seguro: {e}");
    let ana = id_de(&mut s, "Ana");
    ok(&mut s, "entrar", json!({ "id": ana, "senha": "1234" }));
    for d in pedidos_de_busca() {
        let normal = s.chamar("produtos_buscar", &d.to_string());
        assert_eq!(consultar_texto(&mut s, "seguro", "produtos_buscar", &d), normal, "{d}");
    }
    assert_eq!(s.leitor_para_teste().unwrap().aberturas(), 1);
    drop(s);
    let _ = std::fs::remove_dir_all(&pasta);
}

/// Banco em memória (só nos testes): não há outra conexão; a consulta roda na
/// hora, como o `chamar`.
#[test]
fn banco_em_memoria_roda_na_hora() {
    let mut s = Servico::em_memoria().unwrap();
    ok(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
    );
    let d = json!({ "texto": "" });
    let normal = s.chamar("produtos_buscar", &d.to_string());
    assert_eq!(consultar_texto(&mut s, "m", "produtos_buscar", &d), normal);
    assert!(s.leitor_para_teste().is_none());
}

/// T1 na loja grande (as 20.000 peças da 7.5, a mesma do roteiro da digitação
/// rápida). Só à mão, numa CÓPIA da loja:
/// `RUSTINHO_LOJA_TESTE=/copia cargo test --release -p rustinho-nucleo --lib -- --ignored t1_loja_grande`.
/// Confere a mesma resposta do `chamar`, byte a byte, nos textos que a pessoa
/// digita (a primeira letra sozinha, cada prefixo, o texto inteiro) e mostra
/// quanto cada busca leva pelos dois caminhos.
#[test]
#[ignore = "só à mão: precisa da loja grande (RUSTINHO_LOJA_TESTE)"]
fn t1_loja_grande_mesma_resposta_e_tempos() {
    let Some(pasta) = std::env::var_os("RUSTINHO_LOJA_TESTE").map(PathBuf::from) else { return };
    let mut s = Servico::abrir_para_teste(&pasta.join("rustinho.db"), false).unwrap();
    let ana = id_de(&mut s, "Ana");
    ok(&mut s, "entrar", json!({ "id": ana, "senha": "1234" }));
    let mut textos: Vec<String> = Vec::new();
    for fim in 1..="pastilha lista 1111".len() {
        textos.push("pastilha lista 1111"[..fim].to_string());
    }
    textos.extend(["", "8pk", "correia generica", "agua lista 0000", "filtro lista 19995"].map(String::from));
    for texto in &textos {
        for (limite, deslocamento) in [(100, 0), (100, 100)] {
            let d = json!({ "texto": texto, "inativas": false, "limite": limite, "deslocamento": deslocamento });
            let t0 = std::time::Instant::now();
            let normal = s.chamar("produtos_buscar", &d.to_string());
            let t_chamar = t0.elapsed();
            let t0 = std::time::Instant::now();
            let fora = consultar_texto(&mut s, "grande", "produtos_buscar", &d);
            let t_fora = t0.elapsed();
            assert_eq!(fora, normal, "{d}: a consulta fora deu outra resposta");
            let v: Value = serde_json::from_str(&normal).unwrap();
            println!(
                "TEMPO\t{texto:?}\t{deslocamento}\tchamar {:.1} ms\tconsultar {:.1} ms\t{} itens de {}",
                t_chamar.as_secs_f64() * 1000.0,
                t_fora.as_secs_f64() * 1000.0,
                v["dados"]["itens"].as_array().map(|a| a.len()).unwrap_or(0),
                v["dados"]["totalFiltrado"]
            );
        }
    }
}
