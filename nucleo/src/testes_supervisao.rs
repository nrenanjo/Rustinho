//! O-3: a tarefa da troca de dados volta sozinha depois de um erro interno,
//! uma por vez, avisa no Início, e para de vez (com aviso) se cair demais.
//! Portas escolhidas pelo sistema (nenhuma porta fixa; C8).

use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crate::banco::Banco;
use crate::faixas::para_testes;
use crate::supervisao;
use crate::sync::{self, Motor};
use crate::{avisos, config};

fn temp(nome: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rustinho-supervisao-{nome}-{}", crate::tempo::novo_id()));
    std::fs::create_dir_all(&p).unwrap();
    p.join("rustinho.db")
}

fn esperar(prazo: Duration, o_que: &str, mut cond: impl FnMut() -> bool) {
    let fim = Instant::now() + prazo;
    while !cond() {
        assert!(Instant::now() < fim, "não aconteceu em {prazo:?}: {o_que}");
        thread::sleep(Duration::from_millis(50));
    }
}

fn tarefa(banco: &Path, nome: &str) -> supervisao::Tarefa {
    supervisao::tarefas(banco).into_iter().find(|(n, _)| *n == nome).map(|(_, t)| t).unwrap_or_default()
}

#[test]
fn tarefa_da_troca_volta_depois_de_cair_e_para_se_cair_demais() {
    thread::spawn(|| {
        let (ca, cb) = (temp("a"), temp("b"));
        // A: a loja; B: caixa novo que entra nela. Portas escolhidas pelo
        // sistema (C8): B abre primeiro, e A fica sabendo o endereço dele.
        let a = Banco::abrir(&ca).unwrap();
        let u = para_testes::configurar(&a, "Caixa A", 0);
        let b = Banco::abrir(&cb).unwrap();
        config::gravar(&b.conn, "caixa_nome", "Caixa B").unwrap();
        sync::salvar_ajustes(&b.conn, true, "senha-da-loja", "").unwrap();
        let mut mb = Motor::iniciar(cb.clone(), 0, 0);
        let (pt_b, _) = mb.portas(Duration::from_secs(10)).expect("as portas de B abriram");
        sync::salvar_ajustes(&a.conn, true, "senha-da-loja", &format!("127.0.0.1:{pt_b}")).unwrap();
        // A puxada de A cai uma vez.
        supervisao::provocar_queda(&ca, "puxada", 1);
        let mut ma = Motor::iniciar(ca.clone(), 0, 0);
        let (pt_a, _) = ma.portas(Duration::from_secs(10)).expect("as portas de A abriram");
        sync::puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b).unwrap();
        let p = para_testes::peca(&b, &u, "Peça feita no caixa B");

        // Caiu, foi anotada, voltou (uma só rodando) e avisou.
        esperar(Duration::from_secs(30), "a puxada caiu e voltou", || {
            let t = tarefa(&ca, "puxada");
            t.quedas >= 1 && t.inicios >= 2
        });
        assert!(avisos::do_dia(&a.conn).unwrap().iter().any(|v| v.id == "troca_voltou"), "aviso no Início");
        assert!(supervisao::situacao(&ca).paradas.is_empty(), "não ficou parada");
        // A troca continua: A recebe o que B fez depois da queda.
        esperar(Duration::from_secs(40), "A recebeu a peça de B pela puxada", || {
            crate::produtos::ficha(&a.conn, &p).is_ok()
        });

        // O servidor de A cai muitas vezes seguidas: para de vez, com aviso crítico.
        supervisao::provocar_queda(&ca, "servidor", 20);
        esperar(Duration::from_secs(30), "o servidor ficou parado", || tarefa(&ca, "servidor").parada);
        let paradas: Vec<&str> = supervisao::situacao(&ca).paradas.into_iter().map(|(n, _)| n).collect();
        assert_eq!(paradas, vec!["servidor"]);
        let av = avisos::do_dia(&a.conn).unwrap();
        let parada = av.iter().find(|v| v.id == "troca_parada").expect("aviso crítico da troca parada");
        assert_eq!(parada.categoria, avisos::CRITICA);
        // Ele não ficou tentando sem fim: parou depois do limite.
        let t = tarefa(&ca, "servidor");
        assert!(t.quedas <= 5, "{t:?}");

        // Nunca duas cópias da mesma tarefa ao mesmo tempo.
        for (nome, t) in supervisao::tarefas(&ca).into_iter().chain(supervisao::tarefas(&cb)) {
            assert!(t.maximo_rodando <= 1, "{nome}: {t:?}");
        }
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
    })
    .join()
    .unwrap();
}

// ---------------------------------------------------------------------------
// RVI-02 (revisão independente da 0.9.15): falha ao começar (porta ocupada) e
// tarefa que termina sem ninguém pedir não podem passar em silêncio; o limite
// de quedas é exato. Portas escolhidas pelo sistema (nenhuma porta fixa).
// ---------------------------------------------------------------------------

use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

/// Um caixa com a troca de dados ligada.
fn caixa_ligado(nome: &str) -> (PathBuf, Banco) {
    let c = temp(nome);
    let b = Banco::abrir(&c).unwrap();
    para_testes::configurar(&b, "Caixa", 0);
    sync::salvar_ajustes(&b.conn, true, "senha-da-loja", "").unwrap();
    (c, b)
}

fn aviso_da_troca(b: &Banco) -> Option<avisos::Aviso> {
    avisos::do_dia(&b.conn).unwrap().into_iter().find(|v| v.id.starts_with("troca_"))
}

/// Porta TCP ocupada: o servidor não começa. A falha aparece na situação
/// das tarefas, no aviso do Início (com o motivo) e, como a porta continua
/// ocupada, a tarefa para de vez depois do limite, com aviso crítico.
#[test]
fn rvi02_porta_tcp_ocupada_aparece_e_para_depois_do_limite() {
    let ocupada = TcpListener::bind("0.0.0.0:0").unwrap();
    let pt = ocupada.local_addr().unwrap().port();
    let pu = 0; // o sistema escolhe (sem intervalo entre reservar e usar)
    let (c, b) = caixa_ligado("tcp-ocupada");
    let mut m = Motor::iniciar(c.clone(), pt, pu);
    esperar(Duration::from_secs(20), "a falha do servidor aparece na situação", || {
        let s = supervisao::situacao(&c);
        s.paradas.iter().chain(&s.tentando).any(|(n, motivo)| *n == "servidor" && motivo.contains(&pt.to_string()))
    });
    let a = aviso_da_troca(&b).expect("aviso no Início");
    assert!(a.descricao.contains(&format!("TCP {pt}")), "{a:?}");
    esperar(Duration::from_secs(30), "o servidor para de vez", || tarefa(&c, "servidor").parada);
    let a = aviso_da_troca(&b).expect("aviso no Início");
    assert_eq!(a.id, "troca_parada");
    assert_eq!(a.categoria, avisos::CRITICA);
    assert!(a.descricao.contains(&format!("TCP {pt}")), "{a:?}");
    let t = tarefa(&c, "servidor");
    assert_eq!(t.rodando, 0, "{t:?}");
    assert!(t.maximo_rodando <= 1, "{t:?}");
    // A tarefa que achou a porta continua funcionando.
    assert!(!tarefa(&c, "anuncio").parada);
    assert!(m.encerrar(Duration::from_secs(20)));
    drop(ocupada);
}

/// Porta UDP ocupada: o anúncio não começa; mesma coisa.
#[test]
fn rvi02_porta_udp_ocupada_aparece_e_para_depois_do_limite() {
    let ocupada = UdpSocket::bind("0.0.0.0:0").unwrap();
    let pu = ocupada.local_addr().unwrap().port();
    let pt = 0; // o sistema escolhe (sem intervalo entre reservar e usar)
    let (c, b) = caixa_ligado("udp-ocupada");
    let mut m = Motor::iniciar(c.clone(), pt, pu);
    esperar(Duration::from_secs(30), "o anúncio para de vez", || tarefa(&c, "anuncio").parada);
    let s = supervisao::situacao(&c);
    assert!(s.paradas.iter().any(|(n, motivo)| *n == "anuncio" && motivo.contains(&format!("UDP {pu}"))), "{s:?}");
    let a = aviso_da_troca(&b).expect("aviso no Início");
    assert_eq!(a.id, "troca_parada");
    assert!(a.descricao.contains(&format!("UDP {pu}")), "{a:?}");
    assert!(!tarefa(&c, "servidor").parada);
    assert!(m.encerrar(Duration::from_secs(20)));
    drop(ocupada);
}

/// Falha que passa: a porta é solta depois da primeira tentativa; o servidor
/// volta sozinho (uma cópia só), atende de novo, e o aviso deixa de dizer
/// que está parado.
#[test]
fn rvi02_porta_que_libera_volta_sozinha() {
    let ocupada = TcpListener::bind("0.0.0.0:0").unwrap();
    let pt = ocupada.local_addr().unwrap().port();
    let pu = 0; // o sistema escolhe (sem intervalo entre reservar e usar)
    let (c, b) = caixa_ligado("porta-libera");
    let mut m = Motor::iniciar(c.clone(), pt, pu);
    esperar(Duration::from_secs(20), "a primeira falha", || tarefa(&c, "servidor").quedas >= 1);
    let a = aviso_da_troca(&b).expect("enquanto não volta, o Início avisa");
    assert_ne!(a.id, "troca_voltou", "não pode dizer que voltou antes de voltar: {a:?}");
    drop(ocupada);
    esperar(Duration::from_secs(20), "o servidor voltou", || {
        let t = tarefa(&c, "servidor");
        t.rodando == 1 && !t.parada
    });
    // Atende de verdade.
    esperar(Duration::from_secs(10), "a porta atende", || TcpStream::connect(("127.0.0.1", pt)).is_ok());
    let s = supervisao::situacao(&c);
    assert!(s.paradas.is_empty() && s.tentando.is_empty(), "{s:?}");
    assert!(s.voltaram.iter().any(|(n, _)| *n == "servidor"), "{s:?}");
    assert_eq!(aviso_da_troca(&b).map(|a| a.id).as_deref(), Some("troca_voltou"));
    assert!(tarefa(&c, "servidor").maximo_rodando <= 1);
    assert!(m.encerrar(Duration::from_secs(20)));
}

/// Limite exato: a tarefa que cai sempre para na queda número
/// `QUEDAS_MAXIMAS` (3 nos testes, 5 no programa), sem começar de novo.
#[test]
fn rvi02_limite_de_quedas_exato() {
    let banco = std::env::temp_dir().join(format!("rustinho-limite-{}", crate::tempo::novo_id()));
    let parar = Arc::new(AtomicBool::new(false));
    let voltas = Arc::new(AtomicUsize::new(0));
    let v = voltas.clone();
    let tarefa_h = supervisao::supervisionar(banco.clone(), "puxada", parar.clone(), move || {
        v.fetch_add(1, Ordering::SeqCst);
        panic!("queda de teste")
    });
    esperar(Duration::from_secs(20), "parou de vez", || tarefa(&banco, "puxada").parada);
    tarefa_h.join().unwrap();
    let t = tarefa(&banco, "puxada");
    assert_eq!(t.quedas, supervisao::QUEDAS_MAXIMAS, "{t:?}");
    assert_eq!(t.inicios, supervisao::QUEDAS_MAXIMAS, "{t:?}");
    assert_eq!(voltas.load(Ordering::SeqCst), supervisao::QUEDAS_MAXIMAS);
    assert_eq!(t.maximo_rodando, 1, "{t:?}");
}

/// Pânico controlado no meio: cai uma vez, volta (uma cópia só) e fica
/// rodando até pedirem para parar; parar não conta como queda.
#[test]
fn rvi02_panico_uma_vez_volta_e_parar_nao_e_queda() {
    let banco = std::env::temp_dir().join(format!("rustinho-panico-{}", crate::tempo::novo_id()));
    let parar = Arc::new(AtomicBool::new(false));
    let voltas = Arc::new(AtomicUsize::new(0));
    let (v, p) = (voltas.clone(), parar.clone());
    let tarefa_h = supervisao::supervisionar(banco.clone(), "puxada", parar.clone(), move || {
        if v.fetch_add(1, Ordering::SeqCst) == 0 {
            panic!("queda de teste");
        }
        while !p.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    });
    esperar(Duration::from_secs(10), "caiu e voltou a rodar", || {
        let t = tarefa(&banco, "puxada");
        t.quedas == 1 && t.inicios == 2 && t.rodando == 1
    });
    let s = supervisao::situacao(&banco);
    assert!(s.voltaram.iter().any(|(n, _)| *n == "puxada"), "{s:?}");
    parar.store(true, Ordering::SeqCst);
    tarefa_h.join().unwrap();
    let t = tarefa(&banco, "puxada");
    assert_eq!(t.quedas, 1, "parar não é queda: {t:?}");
    assert!(!t.parada);
    assert_eq!(t.maximo_rodando, 1, "{t:?}");
}

fn chamar_ok(s: &mut crate::Servico, c: &str, d: serde_json::Value) -> serde_json::Value {
    let r: serde_json::Value =
        serde_json::from_str(&crate::apoio_chave::chamar(crate::api::NAO_REPETIR, c, &d.to_string(), |x| {
            s.chamar(c, x)
        }))
        .unwrap();
    assert_eq!(r["ok"], true, "{c}: {r}");
    r["dados"].clone()
}

/// RVI-02, pela API que as telas usam: a tarefa que falhou aparece em
/// `sync_ler` (com o motivo e se está parada ou tentando) e no aviso do
/// Início (`avisos_do_dia`).
#[test]
fn rvi02_falha_chega_a_api_das_telas() {
    let db = temp("api");
    let mut s = crate::Servico::abrir_para_teste(&db, false).unwrap();
    chamar_ok(
        &mut s,
        "configurar_inicio",
        serde_json::json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "12345678", "senha2": "12345678" }),
    );
    chamar_ok(&mut s, "sync_salvar", serde_json::json!({ "ligado": true, "senha": "rede-da-loja", "enderecos": "" }));
    let antes = chamar_ok(&mut s, "sync_ler", serde_json::json!({}));
    assert_eq!(antes["tarefasComFalha"], serde_json::json!([]), "{antes}");
    let parar = Arc::new(AtomicBool::new(false));
    let h = supervisao::supervisionar(db.clone(), "servidor", parar.clone(), || {
        Err("não foi possível abrir a porta TCP 47810 (teste); outro programa pode estar usando essa porta".into())
    });
    esperar(Duration::from_secs(10), "parou de vez", || tarefa(&db, "servidor").parada);
    h.join().unwrap();
    let r = chamar_ok(&mut s, "sync_ler", serde_json::json!({}));
    let falhas = r["tarefasComFalha"].as_array().unwrap();
    assert_eq!(falhas.len(), 1, "{r}");
    assert_eq!(falhas[0]["situacao"], "parada");
    assert_eq!(falhas[0]["tarefa"], "responder aos outros caixas");
    assert!(falhas[0]["motivo"].as_str().unwrap().contains("porta TCP 47810"), "{r}");
    assert_eq!(r["tarefasParadas"], serde_json::json!(["servidor"]));
    let avisos = chamar_ok(&mut s, "avisos_do_dia", serde_json::json!({}));
    let parada = avisos.as_array().unwrap().iter().find(|a| a["id"] == "troca_parada").expect("aviso no Início");
    assert_eq!(parada["categoria"], "critica");
    assert!(parada["descricao"].as_str().unwrap().contains("porta TCP 47810"), "{parada}");
    assert!(parada["resumo"].as_str().unwrap().contains("ocupada"), "{parada}");
}

/// RVI-02 + DP-31: com a troca de dados desligada, o motor não abre as portas
/// nem gera falha de porta ocupada; ligar a troca faz o motor tentar abrir a
/// porta ocupada e mostrar o aviso no Início.
#[test]
fn rvi02_troca_desligada_sem_alarme_falso() {
    let ocupada = TcpListener::bind("0.0.0.0:0").unwrap();
    let pt = ocupada.local_addr().unwrap().port();
    let pu = 0; // o sistema escolhe (sem intervalo entre reservar e usar)
    let c = temp("desligada");
    let b = Banco::abrir(&c).unwrap();
    para_testes::configurar(&b, "Caixa", 0);
    let mut m = Motor::iniciar(c.clone(), pt, pu);
    // DP-31: enquanto a troca está desligada, nenhuma porta é aberta e não há queda.
    assert!(m.portas(Duration::from_millis(300)).is_none(), "troca desligada não pode abrir portas");
    assert_eq!(tarefa(&c, "servidor").quedas, 0, "troca desligada não pode tentar abrir porta");
    assert!(aviso_da_troca(&b).is_none(), "{:?}", aviso_da_troca(&b));
    // Ao ligar a troca, o servidor tenta abrir a porta ocupada, para depois do limite e avisa.
    sync::salvar_ajustes(&b.conn, true, "senha-da-loja", "").unwrap();
    esperar(Duration::from_secs(30), "o servidor para de vez", || tarefa(&c, "servidor").parada);
    let a = aviso_da_troca(&b).expect("com a troca ligada, avisa");
    assert_eq!(a.id, "troca_parada");
    assert_eq!(a.resolver, "Tentar de novo");
    assert!(m.encerrar(Duration::from_secs(20)));
    drop(ocupada);
}

/// DP-31 (0.9.19-dev.1, Pacote 2): as portas TCP e UDP da troca de dados só
/// abrem quando a sincronização está ligada e com senha, e fecham logo que a
/// sincronização é desligada (liberando as portas no sistema operacional).
#[test]
fn dp31_portas_so_abrem_com_troca_ligada_e_fecham_ao_desligar() {
    // Escolhe duas portas livres no sistema e solta antes de iniciar o motor desligado.
    let s_tcp = TcpListener::bind("0.0.0.0:0").unwrap();
    let s_udp = UdpSocket::bind("0.0.0.0:0").unwrap();
    let pt = s_tcp.local_addr().unwrap().port();
    let pu = s_udp.local_addr().unwrap().port();
    drop(s_tcp);
    drop(s_udp);

    let c = temp("dp31-ciclo");
    let b = Banco::abrir(&c).unwrap();
    para_testes::configurar(&b, "Caixa 1", 0);
    let mut m = Motor::iniciar(c.clone(), pt, pu);

    // 1. Com a troca desligada: nenhuma porta é ocupada (outro socket vincula sem erro).
    assert!(m.portas(Duration::from_millis(300)).is_none(), "troca desligada não pode abrir portas");
    assert!(!m.alguma_porta_aberta(), "nenhuma das duas portas pode estar aberta");
    let sonda_tcp = TcpListener::bind(("0.0.0.0", pt)).expect("porta TCP deve estar livre com troca desligada");
    let sonda_udp = UdpSocket::bind(("0.0.0.0", pu)).expect("porta UDP deve estar livre com troca desligada");
    drop(sonda_tcp);
    drop(sonda_udp);

    // 2. Ligando a troca: o motor abre as portas TCP e UDP imediatamente.
    sync::salvar_ajustes(&b.conn, true, "senha-forte-loja", "").unwrap();
    supervisao::reiniciar(&c);
    let abertas = m.portas(Duration::from_secs(10)).expect("ao ligar a troca, as portas abrem");
    assert_eq!(abertas, (pt, pu));
    assert!(TcpStream::connect(("127.0.0.1", pt)).is_ok(), "porta TCP atende conexões");
    assert!(TcpListener::bind(("0.0.0.0", pt)).is_err(), "porta TCP está ocupada pelo motor");

    // 3. Desligando a troca: o motor fecha os sockets (TCP e UDP) e libera as portas no sistema.
    sync::salvar_ajustes(&b.conn, false, "", "").unwrap();
    supervisao::reiniciar(&c);
    esperar(Duration::from_secs(10), "o motor fecha ambas as portas ao desligar a troca", || !m.alguma_porta_aberta());
    let sonda_tcp2 = TcpListener::bind(("0.0.0.0", pt)).expect("porta TCP liberada após desligar a troca");
    let sonda_udp2 = UdpSocket::bind(("0.0.0.0", pu)).expect("porta UDP liberada após desligar a troca");
    drop(sonda_tcp2);
    drop(sonda_udp2);

    assert!(m.encerrar(Duration::from_secs(20)));
}

/// RD-B (0.9.19-dev.1, Pacote 2 — todas as 6 regras):
/// 1. Mesma pasta de dados: 2ª abertura impedida pela trava.
/// 2. Portas só com a troca ligada: uma cópia de treino com troca desligada não rouba portas.
/// 3. Pastas diferentes convivem (inclusive instalado 47810/47811 e portátil 47820/47821).
/// 4. No conflito de portas entre duas cópias com troca ligada, a 2ª trabalha localmente (cadastra e vende).
/// 5. O aviso e o `sync_ler` identificam a loja e o caixa do outro Rustinho e oferecem "Tentar de novo",
///    que retoma a sincronização sem reiniciar quando a porta libera.
/// 6. A troca nunca aparece como "funcionando" quando está parada ou tentando de novo.
#[test]
fn rdb_duas_instancias_conflito_identifica_loja_caixa_trabalha_local_e_tenta_de_novo() {
    let db_a = temp("rdb-loja-a");
    let db_b = temp("rdb-loja-b");
    let db_treino = temp("rdb-treino");

    // Regra 1: duas instâncias na mesma pasta são impedidas.
    let mut sa = crate::Servico::abrir_para_teste(&db_a, false).unwrap();
    let dup = crate::Servico::abrir_para_teste(&db_a, false);
    assert!(
        matches!(&dup, Err(e) if e.to_string().contains("já está aberto neste computador")),
        "Regra 1: mesma pasta tem de ser impedida"
    );
    drop(dup);

    // Regra 3 (parte portátil × instalado): portas distintas.
    assert_eq!((sync::PORTA_TCP, sync::PORTA_UDP), (47810, 47811));
    assert_eq!((crate::portatil::PORTA_TCP, crate::portatil::PORTA_UDP), (47820, 47821));

    // Configura Loja A ("Auto Peças Central", "Balcão Principal") e Cópia de Treino (troca desligada).
    chamar_ok(
        &mut sa,
        "configurar_inicio",
        serde_json::json!({
            "loja": "Auto Peças Central",
            "caixa": "Balcão Principal",
            "nome": "Ana",
            "senha": "12345678",
            "senha2": "12345678"
        }),
    );
    let mut s_treino = crate::Servico::abrir_para_teste(&db_treino, false).unwrap();
    chamar_ok(
        &mut s_treino,
        "configurar_inicio",
        serde_json::json!({
            "loja": "Loja Treinamento",
            "caixa": "Caixa Treino",
            "nome": "Ana",
            "senha": "12345678",
            "senha2": "12345678"
        }),
    );

    // Regra 2: a cópia de treino (troca desligada) inicia seu motor na mesma porta sem ocupar nada.
    let s_tcp = TcpListener::bind("0.0.0.0:0").unwrap();
    let s_udp = UdpSocket::bind("0.0.0.0:0").unwrap();
    let pt = s_tcp.local_addr().unwrap().port();
    let pu = s_udp.local_addr().unwrap().port();
    drop(s_tcp);
    drop(s_udp);
    let mut m_treino = Motor::iniciar(db_treino.clone(), pt, pu);
    assert!(m_treino.portas(Duration::from_millis(250)).is_none(), "Regra 2: treino desligado não ocupa portas");

    // Loja A liga a troca e assume (pt, pu) sem ser bloqueada pela cópia de treino.
    chamar_ok(&mut sa, "sync_salvar", serde_json::json!({ "ligado": true, "senha": "rede-central", "enderecos": "" }));
    let mut ma = Motor::iniciar(db_a.clone(), pt, pu);
    assert_eq!(ma.portas(Duration::from_secs(10)), Some((pt, pu)), "Loja A assumiu as portas");

    // Configura Loja B ("Filial Sul", "Caixa 2") em outra pasta e liga a troca disputando (pt, pu).
    let mut sb = crate::Servico::abrir_para_teste(&db_b, false).unwrap();
    chamar_ok(
        &mut sb,
        "configurar_inicio",
        serde_json::json!({
            "loja": "Filial Sul",
            "caixa": "Caixa 2",
            "nome": "Beto",
            "senha": "12345678",
            "senha2": "12345678"
        }),
    );
    chamar_ok(&mut sb, "sync_salvar", serde_json::json!({ "ligado": true, "senha": "rede-filial", "enderecos": "" }));
    let mut mb = Motor::iniciar(db_b.clone(), pt, pu);

    // Espera B detectar o conflito de portas e parar.
    esperar(Duration::from_secs(30), "servidor de B para por conflito de porta com A", || {
        tarefa(&db_b, "servidor").parada
    });

    // Regra 4: B continua trabalhando localmente (cadastra peça, abre caixa, faz venda).
    let mut ficha = chamar_ok(&mut sb, "produto_novo", serde_json::json!({}));
    ficha["nome"] = serde_json::json!("Vela Ignição Local");
    ficha["custo"] = serde_json::json!("20,00");
    ficha["estoqueInicial"] = serde_json::json!("5");
    ficha["precos"][0]["preco"] = serde_json::json!("45,00");
    let salvo_p = chamar_ok(&mut sb, "produto_salvar", ficha);
    assert!(!salvo_p["id"].as_str().unwrap_or("").is_empty(), "Regra 4: cadastro local funciona em B");

    // Regra 5 e Regra 6: `sync_ler` e `avisos_do_dia` nunca dizem que está funcionando,
    // identificam "Auto Peças Central" e "Balcão Principal" e oferecem "Tentar de novo".
    let estado_b = chamar_ok(&mut sb, "sync_ler", serde_json::json!({}));
    assert_eq!(estado_b["ligado"], true);
    assert_eq!(estado_b["funcionando"], false, "Regra 6: nunca mostra funcionando quando parada: {estado_b}");
    let falhas_b = estado_b["tarefasComFalha"].as_array().unwrap();
    assert!(!falhas_b.is_empty(), "{estado_b}");
    let motivo_b = falhas_b[0]["motivo"].as_str().unwrap();
    assert!(
        motivo_b.contains("Auto Peças Central") && motivo_b.contains("Balcão Principal"),
        "Regra 5: motivo deve identificar loja e caixa do outro Rustinho: {motivo_b}"
    );

    let avisos_b = chamar_ok(&mut sb, "avisos_do_dia", serde_json::json!({}));
    let av_parada =
        avisos_b.as_array().unwrap().iter().find(|a| a["id"] == "troca_parada").expect("aviso crítico no Início de B");
    assert_eq!(av_parada["resolver"], "Tentar de novo", "Regra 5: aviso oferece Tentar de novo: {av_parada}");
    assert!(
        av_parada["descricao"].as_str().unwrap().contains("Auto Peças Central")
            && av_parada["descricao"].as_str().unwrap().contains("Balcão Principal"),
        "Regra 5: aviso no Início nomeia loja e caixa do outro Rustinho: {av_parada}"
    );

    // Agora Loja A fecha (liberando as portas) e o operador em B aciona "Tentar de novo".
    assert!(ma.encerrar(Duration::from_secs(20)));
    assert!(mb.encerrar(Duration::from_secs(20)));
    chamar_ok(&mut sb, "aviso_resolver", serde_json::json!({ "id": "troca_parada" }));
    let mut mb2 = Motor::iniciar(db_b.clone(), pt, pu);
    assert_eq!(
        mb2.portas(Duration::from_secs(10)),
        Some((pt, pu)),
        "Regra 5: após Tentar de novo, B assume as portas sem conflito"
    );
    let estado_b2 = chamar_ok(&mut sb, "sync_ler", serde_json::json!({}));
    assert_eq!(estado_b2["funcionando"], true, "B passa a funcionando: {estado_b2}");
    assert_eq!(estado_b2["tarefasComFalha"], serde_json::json!([]), "{estado_b2}");

    assert!(mb2.encerrar(Duration::from_secs(20)));
    assert!(m_treino.encerrar(Duration::from_secs(20)));
}
