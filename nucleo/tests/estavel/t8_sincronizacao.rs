//! Prioridade 2 — Troca de dados entre computadores: pela rede de verdade
//! (no mesmo computador, em portas diferentes), com queda no meio, retomada e
//! sem nada em dobro.
//!
//! Limite: aqui os "computadores" são pastas e portas do mesmo computador. A
//! validação com computadores e redes de verdade (FV-1 a FV-13) continua
//! pendente até ser feita na loja.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use rustinho_nucleo::banco::Banco;
use rustinho_nucleo::{caixa, config, sync, usuarios, vendas};

use crate::comum::*;

const SENHA_REDE: &str = "rede-da-loja";
const FILHO: &str = "RUSTINHO_T8_FILHO";

fn caixa_novo(pasta: &Path, nome: &str) -> Banco {
    let b = Banco::abrir(&pasta.join("rustinho.db")).unwrap();
    config::gravar(&b.conn, "caixa_nome", nome).unwrap();
    sync::salvar_ajustes(&b.conn, true, SENHA_REDE, "").unwrap();
    b
}

fn vender(b: &mut Banco, usuario: &str, peca: &str, n: usize) {
    for _ in 0..n {
        let e = vendas::VendaEntrada {
            itens: vec![vendas::ItemEntrada { produto_id: peca.into(), quantidade: "1".into(), ..Default::default() }],
            tabela: 1,
            pagamentos: vec![vendas::PagamentoEntrada {
                forma: "dinheiro".into(),
                valor: "10".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let tx = b.conn.transaction().unwrap();
        vendas::concluir(&tx, &b.dispositivo, usuario, &e).unwrap();
        tx.commit().unwrap();
    }
}

/// Uma porta livre agora (o sistema escolhe).
fn porta_livre() -> u16 {
    std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap().local_addr().unwrap().port()
}

/// Puxa como o Rustinho faz: se o outro caixa disser que está ocupado
/// (ainda atendendo a conexão que caiu, ou esperando depois de senha errada),
/// tenta de novo em instantes.
fn puxar(caminho: &Path, endereco: &str, porta_local: u16) -> (String, usize) {
    let inicio = Instant::now();
    loop {
        match sync::puxar(caminho, endereco, porta_local) {
            Ok(r) => return r,
            // Ocupado, ou conexão fechada sem resposta (o outro caixa não diz
            // nada antes de conferir a senha): o Rustinho tenta de novo no
            // próximo ciclo; aqui, em meio segundo.
            Err(e)
                if (e.to_string().contains("tenta de novo") || e.to_string().contains("end of file"))
                    && inicio.elapsed() < Duration::from_secs(150) =>
            {
                std::thread::sleep(Duration::from_millis(500));
            }
            Err(e) => panic!("puxar de {endereco}: {e}"),
        }
    }
}

fn contar(b: &Banco, sql: &str) -> i64 {
    b.conn.query_row(sql, [], |l| l.get(0)).unwrap()
}

/// Nada em dobro: cada venda uma vez, cada item uma baixa, números de venda
/// sem repetir dentro de cada caixa.
fn sem_dobro(b: &Banco, peca: &str) {
    assert_eq!(
        contar(b, "SELECT COUNT(*) FROM (SELECT dispositivo, numero FROM venda WHERE tipo = 'venda' GROUP BY 1, 2 HAVING COUNT(*) > 1)"),
        0,
        "número de venda repetido no mesmo caixa"
    );
    let itens = contar(b, &format!("SELECT COUNT(*) FROM venda_item WHERE produto_id = '{peca}'"));
    let baixas =
        contar(b, &format!("SELECT COUNT(*) FROM estoque_movimento WHERE produto_id = '{peca}' AND tipo = 'venda'"));
    assert_eq!(itens, baixas, "cada item vendido baixou uma vez");
}

/// O filho: puxa tudo do caixa A pela rede (lote de 1 alteração, devagar) até ser morto.
#[test]
fn t8_filho_puxa() {
    let Some(pasta) = std::env::var_os(FILHO).map(PathBuf::from) else { return };
    let endereco = std::fs::read_to_string(pasta.join("endereco.txt")).unwrap();
    let r = sync::puxar(&pasta.join("rustinho.db"), &endereco, 48691);
    std::fs::write(pasta.join("terminou.txt"), format!("{r:?}")).unwrap();
}

/// T8.1 — Pela rede: B puxa de A, cai no meio (processo morto), volta e
/// termina sem nada em dobro; depois os dois vendem e trocam até ficar iguais.
#[test]
fn t8_1_rede_com_queda_no_meio_e_retomada() {
    if std::env::var_os(FILHO).is_some() {
        return;
    }
    let (pt_a, pu_a) = (porta_livre(), porta_livre());
    let pa = pasta_nova("t8-a");
    let pb = pasta_nova("t8-b");
    let mut a = caixa_novo(&pa, "Caixa A");
    let ana = usuarios::criar(&a.conn, &a.dispositivo, "Ana", SENHA_ANA, true).unwrap().id;
    usuarios::definir_gerente(&a.conn, &ana, true).unwrap();
    config::gravar(&a.conn, "loja_nome", "Loja Teste").unwrap();
    let peca = {
        let mut f = rustinho_nucleo::produtos::ficha_nova(&a.conn).unwrap();
        f.nome = "Fusível".into();
        f.custo = "1".into();
        f.estoque_inicial = "1000".into();
        f.precos[0].preco = "10".into();
        f.precos[0].lucro = String::new();
        rustinho_nucleo::produtos::salvar(&a.conn, &a.dispositivo, &ana, &f).unwrap()
    };
    caixa::abrir(&a.conn, &a.dispositivo, &ana, "Caixa A", "0").unwrap();
    vender(&mut a, &ana, &peca, 40);
    // A responde com 1 alteração por vez (para a queda de B cair no meio).
    config::gravar(&a.conn, "sync_lote_teste", "1").unwrap();
    let mut motor_a = sync::Motor::iniciar(pa.join("rustinho.db"), pt_a, pu_a);
    std::thread::sleep(Duration::from_millis(400));
    assert!(motor_a.problemas().is_empty(), "{:?}", motor_a.problemas());
    assert!(std::net::TcpStream::connect(("127.0.0.1", pt_a)).is_ok(), "A não está ouvindo");
    let endereco = format!("127.0.0.1:{pt_a}");

    drop(caixa_novo(&pb, "Caixa B"));
    std::fs::write(pb.join("endereco.txt"), &endereco).unwrap();
    let mut filho = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "t8_sincronizacao::t8_filho_puxa", "--nocapture", "--test-threads=1"])
        .env(FILHO, &pb)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let inicio = Instant::now();
    let lado = || {
        let c = rusqlite::Connection::open(pb.join("rustinho.db")).unwrap();
        c.busy_timeout(Duration::from_secs(5)).unwrap();
        c.query_row("SELECT COUNT(*) FROM venda", [], |l| l.get::<_, i64>(0)).unwrap_or(0)
    };
    while lado() < 5 {
        assert!(inicio.elapsed() < Duration::from_secs(60), "B não recebeu nada");
        if filho.try_wait().unwrap().is_some() {
            panic!("o filho terminou antes de cair: {:?}", std::fs::read_to_string(pb.join("terminou.txt")));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    filho.kill().unwrap();
    filho.wait().unwrap();
    let recebidas = lado();
    assert!(recebidas < 40, "a queda tinha de ser no meio ({recebidas} de 40)");

    // B abre de novo e continua de onde parou.
    let mut b = Banco::abrir(&pb.join("rustinho.db")).unwrap();
    config::gravar(&a.conn, "sync_lote_teste", "50").unwrap();
    puxar(&pb.join("rustinho.db"), &endereco, 48692);
    assert_eq!(contar(&b, "SELECT COUNT(*) FROM venda"), 40, "B tem as 40 vendas de A, nenhuma em dobro");
    sem_dobro(&b, &peca);
    let estoque_b =
        contar(&b, &format!("SELECT SUM(quantidade) / 1000 FROM estoque_movimento WHERE produto_id = '{peca}'"));
    assert_eq!(estoque_b, 960);

    // Puxar de novo não traz nada novo.
    let (_, novas) = puxar(&pb.join("rustinho.db"), &endereco, 48692);
    assert_eq!(novas, 0, "puxar de novo repetiu alterações");

    // Os dois vendem e trocam (B também atende): ficam iguais.
    let pt_b = porta_livre();
    let mut motor_b = sync::Motor::iniciar(pb.join("rustinho.db"), pt_b, porta_livre());
    std::thread::sleep(Duration::from_millis(400));
    caixa::abrir(&b.conn, &b.dispositivo, &ana, "Caixa B", "0").unwrap();
    vender(&mut b, &ana, &peca, 7);
    vender(&mut a, &ana, &peca, 5);
    puxar(&pa.join("rustinho.db"), &format!("127.0.0.1:{pt_b}"), pt_a);
    puxar(&pb.join("rustinho.db"), &endereco, pt_b);
    for (nome, x) in [("A", &a), ("B", &b)] {
        assert_eq!(contar(x, "SELECT COUNT(*) FROM venda"), 52, "{nome}: 40 + 7 + 5");
        sem_dobro(x, &peca);
        assert_eq!(
            contar(x, &format!("SELECT SUM(quantidade) / 1000 FROM estoque_movimento WHERE produto_id = '{peca}'")),
            1000 - 52,
            "{nome}: estoque"
        );
    }
    let mut fa = foto_do_negocio(&a.conn);
    let mut fb = foto_do_negocio(&b.conn);
    // "Caixa geral" nasce em cada banco (com a hora de cada um) e só viaja
    // quando alguém muda: compara sem as horas.
    for f in [&mut fa, &mut fb] {
        let sem_hora: Vec<String> =
            f["conta_loja"].iter().map(|l| l.split('|').take(6).collect::<Vec<_>>().join("|")).collect();
        f.insert("conta_loja".into(), sem_hora);
    }
    let d = diferencas(&fa, &fb);
    assert!(d.is_empty(), "A e B ficaram diferentes: {d:?}");
    assert!(motor_a.encerrar(Duration::from_secs(20)));
    assert!(motor_b.encerrar(Duration::from_secs(20)));
    drop((a, b));
    let _ = std::fs::remove_dir_all(&pa);
    let _ = std::fs::remove_dir_all(&pb);
}

/// T8.2 — Senha da rede errada: nada passa; com a certa, passa.
#[test]
fn t8_2_senha_da_rede_errada_nao_troca() {
    if std::env::var_os(FILHO).is_some() {
        return;
    }
    let (pt_a, pu_a) = (porta_livre(), porta_livre());
    let pa = pasta_nova("t8-2a");
    let pb = pasta_nova("t8-2b");
    let a = caixa_novo(&pa, "Caixa A");
    let ana = usuarios::criar(&a.conn, &a.dispositivo, "Ana", SENHA_ANA, true).unwrap().id;
    usuarios::definir_gerente(&a.conn, &ana, true).unwrap();
    let mut motor = sync::Motor::iniciar(pa.join("rustinho.db"), pt_a, pu_a);
    std::thread::sleep(Duration::from_millis(400));
    let b = Banco::abrir(&pb.join("rustinho.db")).unwrap();
    config::gravar(&b.conn, "caixa_nome", "Caixa B").unwrap();
    sync::salvar_ajustes(&b.conn, true, "outra-senha", "").unwrap();
    let r = sync::puxar(&pb.join("rustinho.db"), &format!("127.0.0.1:{pt_a}"), 48686);
    assert!(r.is_err(), "{r:?}");
    assert_eq!(contar(&b, "SELECT COUNT(*) FROM usuario"), 0, "nada passou com a senha errada");
    sync::salvar_ajustes(&b.conn, true, SENHA_REDE, "").unwrap();
    puxar(&pb.join("rustinho.db"), &format!("127.0.0.1:{pt_a}"), 48686);
    assert_eq!(contar(&b, "SELECT COUNT(*) FROM usuario"), 1);
    assert!(motor.encerrar(Duration::from_secs(20)));
    drop((a, b));
    let _ = std::fs::remove_dir_all(&pa);
    let _ = std::fs::remove_dir_all(&pb);
}
