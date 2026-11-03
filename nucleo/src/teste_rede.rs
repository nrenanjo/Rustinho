//! Só para os testes da PEND-10: simula "caminhos" de rede neste computador.
//!
//! Cada caminho é um encaminhador TCP que escuta num endereço 127.0.0.x e
//! repassa para o servidor de um caixa. Dá para fechá-lo (cabo solto), abri-lo
//! de novo, cortar a resposta em qualquer byte e ver de qual IP chegou a
//! conexão. Nada aqui sai desta máquina.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Como o encaminhador trata as conexões.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    /// Repassa tudo.
    Aberto,
    /// Repassa só os primeiros `n` bytes da resposta e corta.
    CortaResposta(usize),
    /// Atende a primeira conexão por inteiro; as seguintes são derrubadas.
    UmaSo,
    /// A conexão de número `k` (1, 2, ...) chega ao servidor, que responde, mas
    /// a resposta se perde; as outras passam normalmente.
    PerdeNaConexao(usize),
}

pub struct Encaminhador {
    pub escuta: SocketAddr,
    destino: SocketAddr,
    modo: Arc<Mutex<Modo>>,
    parar: Arc<AtomicBool>,
    laco: Option<JoinHandle<()>>,
    /// Conexões aceitas desde que abriu (zera ao abrir de novo).
    pub conexoes: Arc<AtomicUsize>,
    /// De qual IP chegou cada conexão.
    pub origens: Arc<Mutex<Vec<Ipv4Addr>>>,
    /// Bytes da última resposta repassada por inteiro.
    pub bytes_da_resposta: Arc<AtomicUsize>,
}

/// Uma porta que ninguém está usando agora (pequena chance de corrida).
pub fn porta_livre() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

impl Encaminhador {
    pub fn novo(ip: [u8; 4], destino: SocketAddr) -> Encaminhador {
        // Porta escolhida pelo sistema ao abrir (C8: sem intervalo entre
        // reservar e usar); "cabo de volta" abre de novo a mesma.
        let escuta = SocketAddr::from((ip, 0));
        let mut e = Encaminhador {
            escuta,
            destino,
            modo: Arc::new(Mutex::new(Modo::Aberto)),
            parar: Arc::new(AtomicBool::new(false)),
            laco: None,
            conexoes: Arc::new(AtomicUsize::new(0)),
            origens: Arc::new(Mutex::new(Vec::new())),
            bytes_da_resposta: Arc::new(AtomicUsize::new(0)),
        };
        e.abrir();
        e
    }

    pub fn endereco(&self) -> String {
        self.escuta.to_string()
    }

    pub fn modo(&self, m: Modo) {
        *self.modo.lock().unwrap() = m;
        self.conexoes.store(0, Ordering::SeqCst);
    }

    /// Cabo solto: ninguém escuta mais (a conexão é recusada na hora).
    pub fn fechar(&mut self) {
        self.parar.store(true, Ordering::SeqCst);
        if let Some(l) = self.laco.take() {
            let _ = l.join();
        }
    }

    /// Cabo de volta.
    pub fn abrir(&mut self) {
        if self.laco.is_some() {
            return;
        }
        self.parar = Arc::new(AtomicBool::new(false));
        self.conexoes.store(0, Ordering::SeqCst);
        let mut ouvinte = None;
        for _ in 0..50 {
            match TcpListener::bind(self.escuta) {
                Ok(o) => {
                    if self.escuta.port() == 0 {
                        self.escuta = o.local_addr().expect("endereço do encaminhador");
                    }
                    ouvinte = Some(o);
                    break;
                }
                Err(_) => thread::sleep(Duration::from_millis(20)),
            }
        }
        let ouvinte = ouvinte.expect("o encaminhador não conseguiu abrir a porta");
        ouvinte.set_nonblocking(true).unwrap();
        let (parar, modo, destino) = (self.parar.clone(), self.modo.clone(), self.destino);
        let (conexoes, origens, bytes) = (self.conexoes.clone(), self.origens.clone(), self.bytes_da_resposta.clone());
        self.laco = Some(thread::spawn(move || {
            while !parar.load(Ordering::SeqCst) {
                match ouvinte.accept() {
                    Ok((cliente, de)) => {
                        let n = conexoes.fetch_add(1, Ordering::SeqCst) + 1;
                        if let SocketAddr::V4(v4) = de {
                            origens.lock().unwrap().push(*v4.ip());
                        }
                        let m = *modo.lock().unwrap();
                        if m == Modo::UmaSo && n > 1 {
                            let _ = cliente.shutdown(Shutdown::Both);
                            continue;
                        }
                        let m = match m {
                            Modo::PerdeNaConexao(k) if k == n => Modo::CortaResposta(0),
                            Modo::PerdeNaConexao(_) => Modo::Aberto,
                            outro => outro,
                        };
                        let bytes = bytes.clone();
                        thread::spawn(move || repassar(cliente, destino, m, &bytes));
                    }
                    Err(_) => thread::sleep(Duration::from_millis(5)),
                }
            }
            // `ouvinte` cai aqui: a porta fecha.
        }));
    }
}

impl Drop for Encaminhador {
    fn drop(&mut self) {
        self.fechar();
    }
}

fn repassar(cliente: TcpStream, destino: SocketAddr, modo: Modo, bytes: &AtomicUsize) {
    cliente.set_nonblocking(false).ok();
    let Ok(servidor) = TcpStream::connect_timeout(&destino, Duration::from_secs(3)) else { return };
    let (Ok(mut c_ler), Ok(mut s_escrever)) = (cliente.try_clone(), servidor.try_clone()) else { return };
    // cliente → servidor
    let ida = thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while let Ok(n) = c_ler.read(&mut buf) {
            if n == 0 || s_escrever.write_all(&buf[..n]).is_err() {
                break;
            }
        }
        let _ = s_escrever.shutdown(Shutdown::Write);
    });
    // servidor → cliente
    let (mut s_ler, mut c_escrever) = (servidor, cliente);
    let mut passou = 0usize;
    let mut buf = [0u8; 8192];
    while let Ok(n) = s_ler.read(&mut buf) {
        if n == 0 {
            if modo == Modo::Aberto {
                bytes.store(passou, Ordering::SeqCst);
            }
            break;
        }
        let quanto = match modo {
            Modo::CortaResposta(limite) => n.min(limite.saturating_sub(passou)),
            _ => n,
        };
        if quanto > 0 && c_escrever.write_all(&buf[..quanto]).is_err() {
            break;
        }
        passou += quanto;
        if let Modo::CortaResposta(limite) = modo {
            if passou >= limite {
                break;
            }
        }
    }
    let _ = c_escrever.shutdown(Shutdown::Both);
    let _ = s_ler.shutdown(Shutdown::Both);
    let _ = ida.join();
}
