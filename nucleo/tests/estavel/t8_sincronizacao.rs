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

/// A troca de dados destes testes usa um servidor (o motor) DESTE processo.
/// Uma restauração de backup de outro teste do mesmo processo (t1) pausa a
/// troca (`sync::pausar`: a trava de escrita, que no Rust passa na frente de
/// quem lê): esperando por ela, o servidor daqui não atende, e o cliente fica
/// parado até o prazo de 30 s ("os error 11", visto em paralelo, 0.9.18). No
/// programa, cliente e servidor de uma troca são processos diferentes; nos
/// testes da própria biblioteca isso é evitado por `pausar_nos_testes`, que
/// não vale aqui (estes testes usam a biblioteca de verdade). Então os
/// testes de troca esperam a vez dos testes de backup.
fn vez_da_troca() -> std::sync::MutexGuard<'static, ()> {
    vez_do_backup()
}

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
    let _vez = vez_da_troca();
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
    // Portas escolhidas pelo sistema na hora de abrir (C8: sem intervalo
    // entre reservar e usar).
    let mut motor_a = sync::Motor::iniciar(pa.join("rustinho.db"), 0, 0);
    let (pt_a, _) = motor_a.portas(Duration::from_secs(10)).expect("as portas do motor abriram");
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
    let mut motor_b = sync::Motor::iniciar(pb.join("rustinho.db"), 0, 0);
    let (pt_b, _) = motor_b.portas(Duration::from_secs(10)).expect("as portas do motor abriram");
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

/// T8.3 — Achado do t2_5 (frente F89): o caixa que recebe aplica cada
/// resposta da troca numa transação só, segurando a escrita do banco enquanto
/// isso; o balcão espera a vez por 5 s (`Banco::preparar`, D-05) e depois
/// falha com "erro no banco de dados". Medido num computador carregado
/// (compilação de teste, 2 núcleos ocupados), uma resposta de 2000 alterações
/// segurou a escrita de 7,3 a 9,2 s, e a venda do balcão falhou nas 3 vezes.
/// Uma resposta, então, leva poucas alterações: aqui, no máximo 200 (a
/// transação de cada uma continua inteira; só vêm mais respostas).
#[test]
fn t8_3_uma_resposta_da_troca_cabe_na_espera_do_balcao() {
    if std::env::var_os(FILHO).is_some() {
        return;
    }
    let _vez = vez_da_troca();
    let pa = pasta_nova("t8-3-a");
    let pb = pasta_nova("t8-3-b");
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
    vender(&mut a, &ana, &peca, 150);
    let em_a = contar(&a, "SELECT COUNT(*) FROM alteracao");
    assert!(em_a > 400, "a loja de teste tem de ter mais alterações que uma resposta ({em_a})");
    let mut motor_a = sync::Motor::iniciar(pa.join("rustinho.db"), 0, 0);
    let (pt_a, _) = motor_a.portas(Duration::from_secs(10)).expect("as portas do motor abriram");
    let b = caixa_novo(&pb, "Caixa B");
    // B puxa uma resposta só (o que chega numa transação).
    config::gravar(&b.conn, "sync_lotes_teste", "1").unwrap();
    drop(b);
    let (_, novas) = puxar(&pb.join("rustinho.db"), &format!("127.0.0.1:{pt_a}"), 48693);
    assert!(novas > 0, "B não recebeu nada");
    assert!(
        novas <= 200,
        "uma resposta da troca trouxe {novas} das {em_a} alterações de A: aplicada numa transação só, ela segura \
         a escrita do caixa que recebe por tempo demais para a espera do balcão (achado do t2_5)"
    );
    assert!(motor_a.encerrar(Duration::from_secs(20)));
    drop(a);
    let _ = std::fs::remove_dir_all(&pa);
    let _ = std::fs::remove_dir_all(&pb);
}

/// T8.4 — Passo 5.4 (PEND-35): pela rede, uma resposta da troca termina no
/// fim de uma operação comercial. A faz vendas de 1 a 3 itens (uma operação
/// cada) e responde com um tamanho que, contado só pela quantidade, cortaria
/// uma venda no meio; B puxa UMA resposta e cada operação de A que chegou a B
/// chegou inteira (nenhuma venda pela metade até a resposta seguinte). A
/// resposta continua com no máximo o tamanho dado (T8.3).
#[test]
fn t8_4_uma_resposta_nao_corta_uma_venda() {
    if std::env::var_os(FILHO).is_some() {
        return;
    }
    let _vez = vez_da_troca();
    let pa = pasta_nova("t8-4-a");
    let pb = pasta_nova("t8-4-b");
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
    for n in 0..12 {
        let itens = 1 + n % 3;
        let e = vendas::VendaEntrada {
            itens: (0..itens)
                .map(|_| vendas::ItemEntrada { produto_id: peca.clone(), quantidade: "1".into(), ..Default::default() })
                .collect(),
            tabela: 1,
            pagamentos: vec![vendas::PagamentoEntrada {
                forma: "dinheiro".into(),
                valor: format!("{}", 10 * itens),
                ..Default::default()
            }],
            ..Default::default()
        };
        let tx = a.conn.transaction().unwrap();
        vendas::concluir(&tx, &a.dispositivo, &ana, &e).unwrap();
        tx.commit().unwrap();
    }
    // O log de A: (seq, operação). O tamanho da resposta fica logo depois da
    // primeira alteração de uma venda do meio (uma operação de 2 ou mais).
    let log: Vec<(i64, Option<String>)> = {
        let mut c = a.conn.prepare("SELECT seq, operacao_id FROM alteracao ORDER BY seq").unwrap();
        let l = c.query_map([], |l| Ok((l.get(0)?, l.get(1)?))).unwrap();
        l.collect::<Result<_, _>>().unwrap()
    };
    let tamanho_de = |o: &Option<String>| log.iter().filter(|x| x.1.is_some() && x.1 == *o).count();
    let comeco = (log.len() / 2..log.len() - 1)
        .find(|&i| log[i].1.is_some() && log[i].1 != log[i - 1].1 && tamanho_de(&log[i].1) >= 2)
        .expect("uma venda no meio do log");
    let limite = comeco + 1;
    config::gravar(&a.conn, "sync_lote_teste", &limite.to_string()).unwrap();
    let mut motor_a = sync::Motor::iniciar(pa.join("rustinho.db"), 0, 0);
    let (pt_a, _) = motor_a.portas(Duration::from_secs(10)).expect("as portas do motor abriram");
    let b = caixa_novo(&pb, "Caixa B");
    // B puxa uma resposta só.
    config::gravar(&b.conn, "sync_lotes_teste", "1").unwrap();
    drop(b);
    let (_, novas) = puxar(&pb.join("rustinho.db"), &format!("127.0.0.1:{pt_a}"), 48694);
    assert!(novas > 0, "B não recebeu nada");
    assert!(novas <= limite, "a resposta passou do tamanho dado: {novas} de {limite}");
    let b = Banco::abrir(&pb.join("rustinho.db")).unwrap();
    let de_a: std::collections::BTreeMap<String, usize> =
        log.iter().filter_map(|x| x.1.clone()).fold(Default::default(), |mut m, o| {
            *m.entry(o).or_insert(0) += 1;
            m
        });
    let em_b: Vec<(String, i64)> = {
        let mut c = b
            .conn
            .prepare("SELECT operacao_id, COUNT(*) FROM alteracao WHERE operacao_id IS NOT NULL GROUP BY 1")
            .unwrap();
        let l = c.query_map([], |l| Ok((l.get(0)?, l.get(1)?))).unwrap();
        l.collect::<Result<_, _>>().unwrap()
    };
    let pela_metade: Vec<_> =
        em_b.iter().filter(|(o, n)| de_a.get(o).is_some_and(|t| *t as i64 != *n)).cloned().collect();
    assert!(
        pela_metade.is_empty(),
        "B ficou com operação de A pela metade (resposta de {novas}, tamanho {limite}): {pela_metade:?}"
    );
    assert!(motor_a.encerrar(Duration::from_secs(20)));
    drop((a, b));
    let _ = std::fs::remove_dir_all(&pa);
    let _ = std::fs::remove_dir_all(&pb);
}

/// Os itens de "Conferência entre caixas" (resumo por tabela, passo 5.4) do caixa.
fn conferencias(b: &Banco) -> Vec<sync::Conflito> {
    sync::conflitos(&b.conn).unwrap().into_iter().filter(|c| c.tipo == "resumo").collect()
}

/// T8.5 — Passo 5.4, parte 1 (PEND-35): resumo por tabela. Dois caixas EM DIA
/// um com o outro (cada um já recebeu tudo do outro) comparam o resumo das
/// tabelas que viajam (contagem e soma de verificação) ao trocar dados:
/// - em dia e iguais: nada vai para o gerente (o Caixa geral, que cada banco
///   cria ao nascer com a sua hora, não conta como diferença);
/// - uma linha mexida por fora do Rustinho em B (sem passar pela troca): os
///   dois caixas avisam o gerente, com a tabela, e nada é mudado sozinho;
/// - caixas que ainda não estão em dia não comparam (não é diferença, é o que
///   ainda vai chegar).
#[test]
#[ignore = "passo 5.4: caracterização do resumo por tabela (falha hoje: ninguém percebe a linha mexida)"]
fn t8_5_resumo_por_tabela_acusa_linha_mexida_a_mao() {
    if std::env::var_os(FILHO).is_some() {
        return;
    }
    let _vez = vez_da_troca();
    let pa = pasta_nova("t8-5-a");
    let pb = pasta_nova("t8-5-b");
    let mut a = caixa_novo(&pa, "Caixa A");
    let ana = usuarios::criar(&a.conn, &a.dispositivo, "Ana", SENHA_ANA, true).unwrap().id;
    usuarios::definir_gerente(&a.conn, &ana, true).unwrap();
    config::gravar(&a.conn, "loja_nome", "Loja Teste").unwrap();
    let peca = {
        let mut f = rustinho_nucleo::produtos::ficha_nova(&a.conn).unwrap();
        f.nome = "Fusível".into();
        f.custo = "1".into();
        f.estoque_inicial = "100".into();
        f.precos[0].preco = "10".into();
        f.precos[0].lucro = String::new();
        rustinho_nucleo::produtos::salvar(&a.conn, &a.dispositivo, &ana, &f).unwrap()
    };
    caixa::abrir(&a.conn, &a.dispositivo, &ana, "Caixa A", "0").unwrap();
    vender(&mut a, &ana, &peca, 3);
    // O banco de B nasce num segundo depois do de A (o Caixa geral de cada um
    // fica com a sua hora, como em dois computadores de verdade).
    std::thread::sleep(Duration::from_millis(1100));
    drop(caixa_novo(&pb, "Caixa B"));
    // Só nos testes: comparar a cada troca (o programa compara de tempos em tempos).
    for c in [&pa, &pb] {
        let x = Banco::abrir(&c.join("rustinho.db")).unwrap();
        config::gravar(&x.conn, "sync_resumo_teste", "sempre").unwrap();
    }
    let mut motor_a = sync::Motor::iniciar(pa.join("rustinho.db"), 0, 0);
    let (pt_a, _) = motor_a.portas(Duration::from_secs(10)).expect("as portas do motor abriram");
    let mut motor_b = sync::Motor::iniciar(pb.join("rustinho.db"), 0, 0);
    let (pt_b, _) = motor_b.portas(Duration::from_secs(10)).expect("as portas do motor abriram");
    let (end_a, end_b) = (format!("127.0.0.1:{pt_a}"), format!("127.0.0.1:{pt_b}"));
    // Trocam até ficar em dia um com o outro.
    puxar(&pb.join("rustinho.db"), &end_a, pt_b);
    puxar(&pa.join("rustinho.db"), &end_b, pt_a);
    puxar(&pb.join("rustinho.db"), &end_a, pt_b);
    puxar(&pa.join("rustinho.db"), &end_b, pt_a);
    let b = Banco::abrir(&pb.join("rustinho.db")).unwrap();
    assert_eq!(contar(&b, "SELECT COUNT(*) FROM venda"), 3, "B recebeu tudo de A");
    for (nome, x) in [("A", &a), ("B", &b)] {
        assert!(conferencias(x).is_empty(), "{nome}: em dia e iguais, nada para o gerente: {:#?}", conferencias(x));
    }
    // Uma linha mexida por fora em B (sem passar pela troca).
    b.conn.execute("UPDATE sync_estado SET aplicando = 1 WHERE id = 1", []).unwrap();
    b.conn.execute("UPDATE produto SET nome = 'Fusível (mexido à mão)' WHERE id = ?1", [&peca]).unwrap();
    b.conn.execute("UPDATE sync_estado SET aplicando = 0 WHERE id = 1", []).unwrap();
    puxar(&pb.join("rustinho.db"), &end_a, pt_b);
    for (nome, x) in [("A", &a), ("B", &b)] {
        let k = conferencias(x);
        assert_eq!(k.len(), 1, "{nome}: a diferença vai para o gerente uma vez: {k:#?}");
        assert!(k[0].texto.contains("Peça"), "{nome}: diz a tabela: {}", k[0].texto);
        assert!(!k[0].pode_usar, "não decide nada sozinha");
    }
    // Nada é mudado sozinho.
    let nome = |x: &Banco| -> String {
        x.conn.query_row("SELECT nome FROM produto WHERE id = ?1", [&peca], |l| l.get(0)).unwrap()
    };
    assert_eq!(nome(&a), "Fusível");
    assert_eq!(nome(&b), "Fusível (mexido à mão)");
    // A mesma diferença na troca seguinte não vira outro item.
    puxar(&pa.join("rustinho.db"), &end_b, pt_a);
    assert_eq!(conferencias(&a).len(), 1);
    // Caixas que não estão em dia não comparam: A vende e B ainda não recebeu.
    let antes = (conferencias(&a).len(), conferencias(&b).len());
    vender(&mut a, &ana, &peca, 1);
    puxar(&pa.join("rustinho.db"), &end_b, pt_a);
    assert_eq!((conferencias(&a).len(), conferencias(&b).len()), antes, "sem estar em dia, não compara");
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
    let _vez = vez_da_troca();
    let pa = pasta_nova("t8-2a");
    let pb = pasta_nova("t8-2b");
    let a = caixa_novo(&pa, "Caixa A");
    let ana = usuarios::criar(&a.conn, &a.dispositivo, "Ana", SENHA_ANA, true).unwrap().id;
    usuarios::definir_gerente(&a.conn, &ana, true).unwrap();
    let mut motor = sync::Motor::iniciar(pa.join("rustinho.db"), 0, 0);
    let (pt_a, _) = motor.portas(Duration::from_secs(10)).expect("as portas do motor abriram");
    let b = Banco::abrir(&pb.join("rustinho.db")).unwrap();
    config::gravar(&b.conn, "caixa_nome", "Caixa B").unwrap();
    sync::salvar_ajustes(&b.conn, true, "outra-senha", "").unwrap();
    let r = sync::puxar(&pb.join("rustinho.db"), &format!("127.0.0.1:{pt_a}"), 48686);
    // C8: recusado pela senha, com A no ar (não "Connection refused").
    assert!(
        matches!(&r, Err(e) if !e.to_string().contains("Connection refused") && !e.to_string().contains("os error 111")),
        "{r:?}"
    );
    assert_eq!(contar(&b, "SELECT COUNT(*) FROM usuario"), 0, "nada passou com a senha errada");
    sync::salvar_ajustes(&b.conn, true, SENHA_REDE, "").unwrap();
    puxar(&pb.join("rustinho.db"), &format!("127.0.0.1:{pt_a}"), 48686);
    assert_eq!(contar(&b, "SELECT COUNT(*) FROM usuario"), 1);
    assert!(motor.encerrar(Duration::from_secs(20)));
    drop((a, b));
    let _ = std::fs::remove_dir_all(&pa);
    let _ = std::fs::remove_dir_all(&pb);
}
