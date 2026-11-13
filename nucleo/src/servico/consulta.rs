//! Consultas fora da linha da janela (dev.3.3, passo 7.6: H09 e ARQ-08).
//!
//! Uma tela pede uma leitura e segue; a resposta chega depois, sem a espera
//! com a barra e sem parar a janela. Quem atende é o [`Leitor`]: uma tarefa à
//! parte, com a sua própria conexão **só de leitura** (`SQLITE_OPEN_READ_ONLY`
//! e `query_only`). O serviço (`api.rs`) continua sendo o único que grava, na
//! linha da janela: nenhuma gravação passa por aqui.
//!
//! - **A última vence, por canal** (um canal por tela): o pedido novo de um
//!   canal toma o lugar do que ainda não começou, e a resposta de um pedido
//!   que já não é o último do canal é descartada (nunca chega à tela).
//! - **Banco trocado** (restaurar um backup, entrar numa loja ou desistir):
//!   o serviço chama [`Leitor::pausar`] antes. A pausa espera a leitura em
//!   curso acabar, fecha a conexão e responde "aguarde" a quem pedir enquanto
//!   ela dura; ao acabar, a próxima leitura abre uma conexão nova. Assim a
//!   tarefa nunca lê um banco que está sendo substituído.
//! - **Sessão que muda** (entrar, sair, usuário desativado): o serviço chama
//!   [`Leitor::esquecer_tudo`]; uma resposta preparada numa sessão nunca
//!   chega depois dela.
//!
//! O que roda e com que permissões é do serviço (`Servico::consultar`): aqui
//! só a fila, a tarefa e a conexão. Este módulo não conhece o serviço (seria
//! um ciclo `api ↔ consulta`, que o `tests/dependencias.rs` não deixa).

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use rusqlite::Connection;

/// Quem recebe a resposta (o JSON de sempre: `{ok, dados}` ou `{ok: false,
/// erro, campo}`). É chamado uma vez, na tarefa de leitura (ou na hora, na
/// linha de quem pediu, quando o pedido é recusado); a ponte põe a resposta
/// na fila da janela. Um pedido que perdeu a vez é largado sem resposta.
pub type Responder = Box<dyn FnOnce(String) + Send>;

/// O trabalho de uma consulta: roda com a conexão só de leitura e devolve a
/// resposta em JSON.
pub type Trabalho = Box<dyn FnOnce(&mut Connection) -> String + Send>;

/// A resposta a quem pede enquanto o banco está sendo trocado (a mesma do
/// `chamar` durante uma operação demorada).
pub const AGUARDE: &str = "Aguarde: o Rustinho ainda está terminando a operação anterior.";

fn resposta_de_erro(mensagem: &str) -> String {
    serde_json::json!({ "ok": false, "erro": mensagem, "campo": null }).to_string()
}

struct Pedido {
    numero: u64,
    trabalho: Trabalho,
    responder: Responder,
}

#[derive(Default)]
struct Estado {
    /// Um lugar por canal: o pedido que ainda não começou.
    esperando: HashMap<String, Pedido>,
    /// A ordem de chegada dos canais que têm pedido esperando.
    ordem: VecDeque<String>,
    /// O número do último pedido de cada canal: a resposta de um mais velho é
    /// descartada.
    ultimo: HashMap<String, u64>,
    proximo: u64,
    /// Pausas abertas (o banco está sendo trocado): nada é lido.
    pausas: usize,
    /// Sobe a cada troca do banco: a conexão de uma geração velha é fechada.
    geracao: u64,
    /// A tarefa está rodando uma consulta agora.
    lendo: bool,
    /// A tarefa tem uma conexão aberta.
    conexao_aberta: bool,
    /// O leitor acabou (o serviço foi fechado).
    fim: bool,
    /// Quantas conexões a tarefa já abriu (para os testes).
    aberturas: u64,
    /// Para os testes: o que aconteceu, em ordem.
    #[cfg(test)]
    eventos: Vec<Evento>,
}

/// Para os testes: a sequência do que a tarefa e as pausas fizeram.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Evento {
    Pausou(std::time::Instant),
    Retomou(std::time::Instant),
    Abriu(u64),
    Leu { numero: u64, geracao: u64, inicio: std::time::Instant, fim: std::time::Instant },
    Fechou,
}

struct Comum {
    estado: Mutex<Estado>,
    sinal: Condvar,
    caminho: PathBuf,
}

impl Comum {
    fn travar(&self) -> MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn esperar<'a>(&self, e: MutexGuard<'a, Estado>) -> MutexGuard<'a, Estado> {
        self.sinal.wait(e).unwrap_or_else(|e| e.into_inner())
    }

    fn pedir(&self, canal: &str, trabalho: Trabalho, responder: Responder) {
        let mut e = self.travar();
        if e.pausas > 0 || e.fim {
            drop(e);
            responder(resposta_de_erro(AGUARDE));
            return;
        }
        e.proximo += 1;
        let numero = e.proximo;
        e.ultimo.insert(canal.to_string(), numero);
        let pedido = Pedido { numero, trabalho, responder };
        if e.esperando.insert(canal.to_string(), pedido).is_none() {
            e.ordem.push_back(canal.to_string());
        }
        drop(e);
        self.sinal.notify_all();
    }
}

/// Só nos testes: pede consultas de outra linha (enquanto o serviço está
/// ocupado com outro comando, como na tela).
#[cfg(test)]
#[derive(Clone)]
pub(crate) struct Alca(Arc<Comum>);

#[cfg(test)]
impl Alca {
    pub(crate) fn pedir(&self, canal: &str, trabalho: Trabalho, responder: Responder) {
        self.0.pedir(canal, trabalho, responder)
    }

    pub(crate) fn eventos(&self) -> Vec<Evento> {
        self.0.travar().eventos.clone()
    }
}

/// A tarefa de leitura de um banco (veja o começo deste arquivo).
pub struct Leitor {
    comum: Arc<Comum>,
    tarefa: Option<std::thread::JoinHandle<()>>,
}

/// Enquanto existir, o leitor está parado e sem conexão aberta (veja
/// [`Leitor::pausar`]).
pub struct Pausa {
    comum: Arc<Comum>,
}

impl Drop for Pausa {
    fn drop(&mut self) {
        let mut e = self.comum.travar();
        e.pausas = e.pausas.saturating_sub(1);
        e.geracao += 1;
        #[cfg(test)]
        e.eventos.push(Evento::Retomou(std::time::Instant::now()));
        drop(e);
        self.comum.sinal.notify_all();
    }
}

impl Leitor {
    /// O leitor do banco em `caminho`. A tarefa começa agora; a conexão, só
    /// na primeira consulta.
    pub fn novo(caminho: PathBuf) -> Leitor {
        let comum = Arc::new(Comum { estado: Mutex::new(Estado::default()), sinal: Condvar::new(), caminho });
        let da_tarefa = comum.clone();
        let tarefa = std::thread::Builder::new()
            .name("rustinho-leitor".into())
            .spawn(move || trabalhar(&da_tarefa))
            .map_err(|e| crate::registro::anotar(&format!("leitor: a tarefa de leitura não começou: {e}")))
            .ok();
        if tarefa.is_none() {
            comum.travar().fim = true;
        }
        Leitor { comum, tarefa }
    }

    /// Pede uma consulta no `canal`. A última vence: um pedido do mesmo canal
    /// que ainda não começou é largado (sem resposta), e a resposta de um que
    /// já começou é descartada quando chegar.
    pub fn pedir(&self, canal: &str, trabalho: Trabalho, responder: Responder) {
        self.comum.pedir(canal, trabalho, responder)
    }

    /// O banco vai ser trocado: espera a leitura em curso acabar, fecha a
    /// conexão e responde "aguarde" aos pedidos que esperavam. Até a pausa
    /// sair de cena, nenhum pedido é lido (responde "aguarde" na hora); depois,
    /// a próxima leitura abre uma conexão nova.
    pub fn pausar(&self) -> Pausa {
        let mut e = self.comum.travar();
        e.pausas += 1;
        e.geracao += 1;
        let largados: Vec<Pedido> = e.esperando.drain().map(|(_, p)| p).collect();
        e.ordem.clear();
        self.comum.sinal.notify_all();
        while e.lendo || e.conexao_aberta {
            e = self.comum.esperar(e);
        }
        #[cfg(test)]
        e.eventos.push(Evento::Pausou(std::time::Instant::now()));
        drop(e);
        for p in largados {
            (p.responder)(resposta_de_erro(AGUARDE));
        }
        Pausa { comum: self.comum.clone() }
    }

    /// A sessão mudou: os pedidos que esperavam saem sem resposta, e a do que
    /// está rodando é descartada.
    pub fn esquecer_tudo(&self) {
        let mut e = self.comum.travar();
        e.esperando.clear();
        e.ordem.clear();
        e.ultimo.clear();
    }

    /// Quantas conexões a tarefa já abriu.
    pub fn aberturas(&self) -> u64 {
        self.comum.travar().aberturas
    }

    /// A tarefa tem uma conexão aberta agora?
    pub fn conexao_aberta(&self) -> bool {
        self.comum.travar().conexao_aberta
    }

    #[cfg(test)]
    pub(crate) fn eventos(&self) -> Vec<Evento> {
        self.comum.travar().eventos.clone()
    }

    #[cfg(test)]
    pub(crate) fn alca(&self) -> Alca {
        Alca(self.comum.clone())
    }
}

impl Drop for Leitor {
    /// O serviço vai fechar: os pedidos que esperavam saem, a leitura em curso
    /// termina e a conexão fecha antes de o serviço soltar o banco.
    fn drop(&mut self) {
        let mut e = self.comum.travar();
        e.fim = true;
        e.esperando.clear();
        e.ordem.clear();
        e.ultimo.clear();
        drop(e);
        self.comum.sinal.notify_all();
        if let Some(t) = self.tarefa.take() {
            let _ = t.join();
        }
    }
}

/// A tarefa: espera pedidos e lê, um de cada vez, com a sua conexão.
fn trabalhar(comum: &Comum) {
    let mut conexao: Option<(Connection, u64)> = None;
    loop {
        let mut e = comum.travar();
        let (canal, pedido, geracao) = loop {
            let velha = conexao.as_ref().is_some_and(|(_, g)| *g != e.geracao);
            if conexao.is_some() && (e.fim || e.pausas > 0 || velha) {
                conexao = None;
                e.conexao_aberta = false;
                #[cfg(test)]
                e.eventos.push(Evento::Fechou);
                comum.sinal.notify_all();
            }
            if e.fim {
                return;
            }
            if e.pausas == 0 {
                if let Some(canal) = e.ordem.pop_front() {
                    if let Some(p) = e.esperando.remove(&canal) {
                        let g = e.geracao;
                        break (canal, p, g);
                    }
                    continue;
                }
            }
            e = comum.esperar(e);
        };
        e.lendo = true;
        drop(e);

        #[cfg(test)]
        let inicio = std::time::Instant::now();
        let resposta = ler(comum, &mut conexao, geracao, pedido.trabalho);

        let mut e = comum.travar();
        e.lendo = false;
        #[cfg(test)]
        e.eventos.push(Evento::Leu { numero: pedido.numero, geracao, inicio, fim: std::time::Instant::now() });
        // A última vence: só responde quem ainda é o último pedido do canal.
        let vale = e.ultimo.get(&canal) == Some(&pedido.numero);
        if vale {
            e.ultimo.remove(&canal);
        }
        drop(e);
        comum.sinal.notify_all();
        if vale {
            (pedido.responder)(resposta);
        }
    }
}

/// Abre a conexão, se precisar, e roda o trabalho. Um erro inesperado vira
/// resposta de erro, e a conexão é fechada (a próxima leitura abre outra).
fn ler(comum: &Comum, conexao: &mut Option<(Connection, u64)>, geracao: u64, trabalho: Trabalho) -> String {
    if conexao.is_none() {
        match crate::banco::conexao_de_leitura(&comum.caminho) {
            Ok(c) => {
                *conexao = Some((c, geracao));
                let mut e = comum.travar();
                e.conexao_aberta = true;
                e.aberturas += 1;
                #[cfg(test)]
                {
                    let n = e.aberturas;
                    e.eventos.push(Evento::Abriu(n));
                }
            }
            Err(erro) => {
                crate::registro::anotar(&format!("leitor: não deu para abrir o banco só para leitura: {erro}"));
                return resposta_de_erro("Não deu para ler o banco agora. Tente de novo.");
            }
        }
    }
    let Some((conn, _)) = conexao.as_mut() else {
        return resposta_de_erro("Não deu para ler o banco agora. Tente de novo.");
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| trabalho(conn))) {
        Ok(r) => r,
        Err(_) => {
            crate::registro::anotar("leitor: erro interno inesperado numa consulta (a conexão de leitura foi fechada)");
            *conexao = None;
            comum.travar().conexao_aberta = false;
            comum.sinal.notify_all();
            resposta_de_erro("Erro interno inesperado na consulta. Tente de novo.")
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    /// Um banco qualquer num arquivo, em WAL, com uma conexão de gravação
    /// aberta (como o serviço deixa).
    fn banco(nome: &str) -> (PathBuf, Connection) {
        let pasta = std::env::temp_dir().join(format!(
            "rustinho-leitor-{nome}-{}-{}",
            std::process::id(),
            crate::tempo::novo_id()
        ));
        std::fs::create_dir_all(&pasta).unwrap();
        let db = pasta.join("rustinho.db");
        let c = Connection::open(&db).unwrap();
        c.execute_batch("PRAGMA journal_mode = WAL; CREATE TABLE t (x INTEGER); INSERT INTO t VALUES (1);").unwrap();
        (db, c)
    }

    fn contar() -> Trabalho {
        Box::new(|c: &mut Connection| {
            let n: i64 = c.query_row("SELECT COUNT(*) FROM t", [], |l| l.get(0)).unwrap();
            format!("{n}")
        })
    }

    fn para(tx: &mpsc::Sender<String>) -> Responder {
        let tx = tx.clone();
        Box::new(move |r| {
            let _ = tx.send(r);
        })
    }

    /// T3: três pedidos no mesmo canal, o primeiro ainda lendo. O segundo
    /// perde o lugar para o terceiro antes de começar; a resposta do primeiro
    /// chega depois do terceiro pedido e é descartada. Só o terceiro responde.
    #[test]
    fn t3_a_ultima_vence_no_mesmo_canal() {
        let (db, _c) = banco("t3");
        let l = Leitor::novo(db);
        let (tx, rx) = mpsc::channel::<String>();
        let (comecou, espera_comecar) = mpsc::channel::<()>();
        let (solta, portao) = mpsc::channel::<()>();
        l.pedir(
            "pecas",
            Box::new(move |_| {
                comecou.send(()).unwrap();
                portao.recv().unwrap();
                "1".into()
            }),
            para(&tx),
        );
        espera_comecar.recv_timeout(Duration::from_secs(10)).expect("o primeiro começou");
        l.pedir("pecas", Box::new(|_| "2".into()), para(&tx));
        l.pedir("pecas", Box::new(|_| "3".into()), para(&tx));
        solta.send(()).unwrap();
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), "3");
        assert!(rx.recv_timeout(Duration::from_millis(300)).is_err(), "só a última responde");
        let lidas: Vec<u64> = l
            .eventos()
            .iter()
            .filter_map(|e| if let Evento::Leu { numero, .. } = e { Some(*numero) } else { None })
            .collect();
        assert_eq!(lidas, vec![1, 3], "o segundo nunca rodou");
    }

    /// Canais diferentes não se atrapalham: cada um tem a sua última.
    #[test]
    fn t3_canais_diferentes_respondem_os_dois() {
        let (db, _c) = banco("canais");
        let l = Leitor::novo(db);
        let (tx, rx) = mpsc::channel::<String>();
        l.pedir("a", Box::new(|_| "a".into()), para(&tx));
        l.pedir("b", Box::new(|_| "b".into()), para(&tx));
        let mut r =
            vec![rx.recv_timeout(Duration::from_secs(10)).unwrap(), rx.recv_timeout(Duration::from_secs(10)).unwrap()];
        r.sort();
        assert_eq!(r, vec!["a", "b"]);
    }

    /// A conexão só lê: uma gravação por ela é recusada pelo SQLite.
    #[test]
    fn a_conexao_do_leitor_nao_grava() {
        let (db, c) = banco("so-le");
        let l = Leitor::novo(db);
        let (tx, rx) = mpsc::channel::<String>();
        l.pedir(
            "x",
            Box::new(|c: &mut Connection| match c.execute("INSERT INTO t VALUES (2)", []) {
                Ok(_) => "gravou".into(),
                Err(e) => format!("recusou: {e}"),
            }),
            para(&tx),
        );
        let r = rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(r.starts_with("recusou"), "{r}");
        let n: i64 = c.query_row("SELECT COUNT(*) FROM t", [], |l| l.get(0)).unwrap();
        assert_eq!(n, 1);
    }

    /// A pausa fecha a conexão, responde "aguarde" na hora enquanto dura e,
    /// ao acabar, a próxima leitura abre outra conexão (a geração subiu).
    #[test]
    fn pausa_fecha_a_conexao_e_a_proxima_abre_outra() {
        let (db, _c) = banco("pausa");
        let l = Leitor::novo(db);
        let (tx, rx) = mpsc::channel::<String>();
        l.pedir("x", contar(), para(&tx));
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), "1");
        assert!(l.conexao_aberta());
        assert_eq!(l.aberturas(), 1);
        let p = l.pausar();
        assert!(!l.conexao_aberta(), "a pausa fechou a conexão");
        l.pedir("x", contar(), para(&tx));
        let r = rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(r.contains(AGUARDE), "durante a pausa: aguarde ({r})");
        drop(p);
        l.pedir("x", contar(), para(&tx));
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), "1");
        assert_eq!(l.aberturas(), 2, "depois da pausa, uma conexão nova");
    }

    /// A pausa espera a leitura em curso acabar (nunca troca o banco com uma
    /// leitura no meio); a resposta dela chega.
    #[test]
    fn pausa_espera_a_leitura_em_curso() {
        let (db, _c) = banco("pausa-espera");
        let l = std::sync::Arc::new(Leitor::novo(db));
        let (tx, rx) = mpsc::channel::<String>();
        let (comecou, espera_comecar) = mpsc::channel::<()>();
        let (solta, portao) = mpsc::channel::<()>();
        l.pedir(
            "x",
            Box::new(move |_| {
                comecou.send(()).unwrap();
                portao.recv().unwrap();
                "lida".into()
            }),
            para(&tx),
        );
        espera_comecar.recv_timeout(Duration::from_secs(10)).unwrap();
        let (pausou, espera_pausa) = mpsc::channel::<()>();
        let l2 = l.clone();
        let t = std::thread::spawn(move || {
            let p = l2.pausar();
            pausou.send(()).unwrap();
            drop(p);
        });
        assert!(espera_pausa.recv_timeout(Duration::from_millis(300)).is_err(), "a pausa esperou a leitura");
        solta.send(()).unwrap();
        espera_pausa.recv_timeout(Duration::from_secs(10)).expect("a pausa veio depois da leitura");
        t.join().unwrap();
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), "lida");
    }

    /// Esquecer tudo (a sessão mudou): o que esperava sai sem resposta e a
    /// resposta do que estava rodando é descartada.
    #[test]
    fn esquecer_tudo_descarta_o_que_estava_pedido() {
        let (db, _c) = banco("esquecer");
        let l = Leitor::novo(db);
        let (tx, rx) = mpsc::channel::<String>();
        let (comecou, espera_comecar) = mpsc::channel::<()>();
        let (solta, portao) = mpsc::channel::<()>();
        l.pedir(
            "a",
            Box::new(move |_| {
                comecou.send(()).unwrap();
                portao.recv().unwrap();
                "velha".into()
            }),
            para(&tx),
        );
        espera_comecar.recv_timeout(Duration::from_secs(10)).unwrap();
        l.pedir("b", Box::new(|_| "esperando".into()), para(&tx));
        l.esquecer_tudo();
        solta.send(()).unwrap();
        assert!(rx.recv_timeout(Duration::from_millis(500)).is_err(), "nada da sessão anterior chega");
        l.pedir("a", Box::new(|_| "nova".into()), para(&tx));
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), "nova");
    }
}
