//! O `Motor` (o laço da troca).
//!
//! Parte do `troca::sync` (passo 5.1: o texto veio do sync.rs sem mudança,
//! fora a visibilidade `pub(super)`, que tem o mesmo alcance de antes).

use super::*;

/// O motor da sincronização: três tarefas em segundo plano (servidor,
/// anúncio/escuta na rede e puxada periódica). Fica parado enquanto a
/// sincronização estiver desligada nas configurações.
///
/// R-06: as tarefas são guardadas e, ao encerrar, o motor espera todas
/// terminarem (inclusive os atendimentos em andamento) antes de devolver.
pub struct Motor {
    pub(super) parar: Arc<AtomicBool>,
    pub(super) tarefas: Vec<thread::JoinHandle<()>>,
    /// As portas em uso de verdade (TCP, UDP); 0 = ainda não abriu. Pedir a
    /// porta 0 deixa o sistema escolher uma livre na hora de abrir (testes):
    /// sem intervalo entre reservar e usar.
    pub(super) portas: Arc<(AtomicU16, AtomicU16)>,
    /// O que impediu o motor de funcionar por inteiro (ex.: porta ocupada).
    pub(super) problemas: Arc<Mutex<Vec<String>>>,
    /// Já pediram para encerrar com um prazo (quem pediu decide o que fazer
    /// se não deu tempo; o descarte não espera de novo).
    pub(super) encerrado: bool,
}

impl Drop for Motor {
    fn drop(&mut self) {
        if !self.encerrado {
            self.encerrar(Duration::from_secs(45));
        }
    }
}

/// Dorme aos pouquinhos, acordando logo se pedirem para parar.
pub(super) fn cochilar(parar: &AtomicBool, tempo: Duration) {
    let fim = Instant::now() + tempo;
    while !parar.load(Ordering::SeqCst) && Instant::now() < fim {
        thread::sleep(Duration::from_millis(100));
    }
}

impl Motor {
    /// Pede para parar e espera as tarefas até o prazo. true = todas
    /// terminaram (ninguém mais usa o banco). Pode ser chamado de novo.
    pub fn encerrar(&mut self, prazo: Duration) -> bool {
        self.encerrado = true;
        self.parar.store(true, Ordering::SeqCst);
        let fim = Instant::now() + prazo;
        while self.tarefas.iter().any(|t| !t.is_finished()) && Instant::now() < fim {
            thread::sleep(Duration::from_millis(50));
        }
        let (prontas, presas): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.tarefas).into_iter().partition(|t| t.is_finished());
        for t in prontas {
            let _ = t.join();
        }
        let ok = presas.is_empty();
        self.tarefas = presas;
        ok
    }

    /// As portas (TCP, UDP) que o motor abriu de verdade, esperando até
    /// `prazo` que as duas abram. None = não abriram nesse prazo.
    pub fn portas(&self, prazo: Duration) -> Option<(u16, u16)> {
        let fim = Instant::now() + prazo;
        loop {
            let (t, u) = (self.portas.0.load(Ordering::SeqCst), self.portas.1.load(Ordering::SeqCst));
            if t != 0 && u != 0 {
                return Some((t, u));
            }
            if Instant::now() >= fim {
                return None;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// O que está impedindo o motor de funcionar (ex.: porta ocupada). Também
    /// vai para o registro, uma vez.
    pub fn problemas(&self) -> Vec<String> {
        self.problemas.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Só nos testes: uma tarefa a mais para o motor esperar ao encerrar
    /// (simula uma tarefa travada).
    #[cfg(test)]
    pub(crate) fn esperar_tambem(&mut self, tarefa: thread::JoinHandle<()>) {
        self.tarefas.push(tarefa);
    }

    pub fn iniciar(caminho: PathBuf, porta_tcp: u16, porta_udp: u16) -> Motor {
        let parar = Arc::new(AtomicBool::new(false));
        let mut tarefas = Vec::new();
        let problemas: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        // Com a porta 0, a de verdade só se sabe depois de abrir; quem anuncia
        // e quem puxa usam a de verdade. Depois de uma queda, a tarefa abre a
        // mesma porta de novo (os outros caixas já a conhecem).
        let portas: Arc<(AtomicU16, AtomicU16)> = Arc::new((AtomicU16::new(0), AtomicU16::new(0)));
        let porta_tcp_de = move |p: &Arc<(AtomicU16, AtomicU16)>| match p.0.load(Ordering::SeqCst) {
            0 => porta_tcp,
            aberta => aberta,
        };
        let porta_udp_de = move |p: &Arc<(AtomicU16, AtomicU16)>| match p.1.load(Ordering::SeqCst) {
            0 => porta_udp,
            aberta => aberta,
        };

        // 1. Servidor: atende os outros caixas. (O-3: cada tarefa roda
        // supervisionada e volta sozinha depois de um erro interno; veja
        // `supervisao`.)
        {
            let caminho = caminho.clone();
            let parar = parar.clone();
            let problemas = problemas.clone();
            let parar_tarefa = parar.clone();
            let portas = portas.clone();
            tarefas.push(crate::supervisao::supervisionar(caminho.clone(), "servidor", parar_tarefa, move || {
                // RVI-02: porta ocupada é falha da tarefa (o supervisor tenta de
                // novo, com limite, e a situação mostra o motivo); o registro
                // recebe a falha pelo supervisor.
                let porta_tcp = porta_tcp_de(&portas);
                let ouvinte = match TcpListener::bind(("0.0.0.0", porta_tcp)) {
                    Ok(o) => o,
                    Err(e) => {
                        guardar_problema(&problemas, texto_da_porta("TCP", porta_tcp, &e.to_string()));
                        return Err(motivo_da_porta("TCP", porta_tcp, &e.to_string()));
                    }
                };
                if let Ok(a) = ouvinte.local_addr() {
                    portas.0.store(a.port(), Ordering::SeqCst);
                }
                ouvinte.set_nonblocking(true).ok();
                let vagas = Arc::new(AtomicUsize::new(0));
                let mut atendendo: Vec<Atendimento> = Vec::new();
                // Quem conecta espera na fila até mandar alguma coisa: conexão
                // muda nunca ocupa vaga nem tarefa. Um caixa de verdade manda o
                // pedido na hora e passa logo para o atendimento.
                let mut fila: Vec<NaFila> = Vec::new();
                while !parar.load(Ordering::SeqCst) {
                    crate::supervisao::ponto_de_queda(&caminho, "servidor");
                    atendendo.retain(|a| !a.tarefa.is_finished());
                    let mut mexeu = false;
                    // 1. Aceita quem chegou.
                    while let Ok((fluxo, origem)) = ouvinte.accept() {
                        mexeu = true;
                        let ip = origem.ip();
                        // Um endereço não tem mais que `VAGAS_POR_ORIGEM` na fila.
                        if fila.iter().filter(|f| f.ip == ip).count() >= VAGAS_POR_ORIGEM {
                            continue;
                        }
                        if fluxo.set_nonblocking(true).is_err() {
                            continue;
                        }
                        // Fila cheia: sai a mais antiga (está muda há mais tempo).
                        if fila.len() >= NA_FILA_MAXIMO {
                            fila.remove(0);
                        }
                        fila.push(NaFila { fluxo, ip, desde: Instant::now() });
                    }
                    // 2. Quem já mandou alguma coisa passa para o atendimento (se
                    // houver vaga, e no máximo `VAGAS_POR_ORIGEM` por endereço);
                    // quem passou do prazo calado, ou fechou, sai da fila.
                    let mut i = 0;
                    while i < fila.len() {
                        let mut espiada = [0u8; 4];
                        let estado = fila[i].fluxo.peek(&mut espiada);
                        let sair = match estado {
                            Ok(0) => true,
                            Ok(_) => {
                                let do_ip = atendendo.iter().filter(|a| a.ip == fila[i].ip).count();
                                if do_ip >= VAGAS_POR_ORIGEM {
                                    false
                                } else if let Some(vaga) = Vaga::pegar_de(&vagas) {
                                    let NaFila { fluxo, ip, .. } = fila.remove(i);
                                    mexeu = true;
                                    fluxo.set_nonblocking(false).ok();
                                    let caminho = caminho.clone();
                                    let parar_tarefa = parar.clone();
                                    let tarefa = thread::Builder::new().name("sync-atender".into()).spawn(move || {
                                        let _vaga = vaga;
                                        if let Err(e) = atender(&caminho, fluxo, &parar_tarefa) {
                                            if matches!(e, Erro::Banco(_) | Erro::Interno(_)) {
                                                crate::registro::anotar(&format!("sync atender: {e}"));
                                            }
                                        }
                                    });
                                    // Sem tarefa nova, a vaga já voltou (ela foi junto e caiu).
                                    match tarefa {
                                        Ok(t) => atendendo.push(Atendimento { ip, tarefa: t }),
                                        Err(e) => crate::registro::anotar(&format!("sync: não abriu tarefa: {e}")),
                                    }
                                    continue;
                                } else {
                                    false
                                }
                            }
                            Err(e)
                                if matches!(
                                    e.kind(),
                                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                                ) =>
                            {
                                fila[i].desde.elapsed() > PRAZO_DO_PEDIDO
                            }
                            Err(_) => true,
                        };
                        if sair {
                            fila.remove(i);
                            mexeu = true;
                        } else {
                            i += 1;
                        }
                    }
                    if !mexeu {
                        thread::sleep(Duration::from_millis(10));
                    }
                }
                // Parou de aceitar; espera quem já estava sendo atendido (cada
                // conexão tem prazo de leitura e escrita, então termina).
                drop(ouvinte);
                for a in atendendo {
                    let _ = a.tarefa.join();
                }
                Ok(())
            }));
        }

        // 2. Descoberta: anuncia este caixa e escuta os outros (UDP).
        {
            let caminho = caminho.clone();
            let parar = parar.clone();
            let problemas = problemas.clone();
            let parar_tarefa = parar.clone();
            let portas = portas.clone();
            tarefas.push(crate::supervisao::supervisionar(caminho.clone(), "anuncio", parar_tarefa, move || {
                let porta_udp = porta_udp_de(&portas);
                let socket = match UdpSocket::bind(("0.0.0.0", porta_udp)) {
                    Ok(s) => s,
                    Err(e) => {
                        guardar_problema(&problemas, texto_da_porta("UDP", porta_udp, &e.to_string()));
                        return Err(motivo_da_porta("UDP", porta_udp, &e.to_string()));
                    }
                };
                if let Ok(a) = socket.local_addr() {
                    portas.1.store(a.port(), Ordering::SeqCst);
                }
                let porta_udp = porta_udp_de(&portas);
                socket.set_broadcast(true).ok();
                socket.set_read_timeout(Some(Duration::from_secs(1))).ok();
                let mut ultimo_anuncio = Instant::now() - Duration::from_secs(60);
                let mut ultima_lista = Instant::now() - Duration::from_secs(60);
                // As interfaces são lidas de novo a cada segundo; uma mudança
                // (cabo plugado, Wi-Fi que caiu, IP novo) anuncia na hora.
                let mut interfaces: Option<Vec<Interface>> = None;
                let mut conhecidas: Vec<Interface> = Vec::new();
                let mut erros_de_envio: Vec<(std::net::Ipv4Addr, String)> = Vec::new();
                let mut buf = vec![0u8; 2048];
                while !parar.load(Ordering::SeqCst) {
                    crate::supervisao::ponto_de_queda(&caminho, "anuncio");
                    let Ok(conn) = abrir_conexao(&caminho) else {
                        cochilar(&parar, Duration::from_secs(5));
                        continue;
                    };
                    let Ok(a) = ajustes(&conn) else {
                        cochilar(&parar, Duration::from_secs(1));
                        continue;
                    };
                    if !a.ligado || a.senha.is_empty() {
                        cochilar(&parar, Duration::from_secs(3));
                        continue;
                    }
                    // Gravações deste laço (sal novo, par visto) respeitam a
                    // pausa da restauração de backup, como as outras tarefas.
                    let chaves = {
                        let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
                        Chaves::de(&conn, &a.senha)
                    };
                    let Ok(k) = chaves else {
                        cochilar(&parar, Duration::from_secs(1));
                        continue;
                    };
                    let mudou = if ultima_lista.elapsed() >= Duration::from_secs(1) {
                        ultima_lista = Instant::now();
                        interfaces = rede_local::lista_ou_nada(rede_local::listar());
                        let agora = interfaces.clone().unwrap_or_default();
                        let mudou = rede_local::interfaces_mudaram(&conhecidas, &agora);
                        conhecidas = agora;
                        mudou
                    } else {
                        false
                    };
                    if mudou || ultimo_anuncio.elapsed() > Duration::from_secs(10) {
                        let ola = anuncio(&a.dispositivo, &a.nome, porta_tcp_de(&portas));
                        if let Ok(pacote) = cifrar(&k, ola.to_string().as_bytes()) {
                            // Um anúncio por interface apropriada (um broadcast só
                            // sai por uma), mais o limitado de sempre. Se o sistema
                            // não soube listar, só o limitado.
                            let erros = rede_local::anunciar(&conhecidas, &a.interfaces, &pacote, porta_udp);
                            for (ip, e) in rede_local::erros_novos(&mut erros_de_envio, erros) {
                                crate::registro::anotar(&format!("troca de dados: anúncio pela interface {ip}: {e}"));
                            }
                        }
                        ultimo_anuncio = Instant::now();
                    }
                    if let Ok((n, origem)) = socket.recv_from(&mut buf) {
                        // R-09: sal desconhecido só passa pela conta cara
                        // (Argon2) poucas vezes por minuto (veja `decifrar`).
                        if let Ok(claro) = decifrar(&k, &buf[..n], Some(origem.ip())) {
                            if let Ok(v) = serde_json::from_slice::<Value>(&claro) {
                                if let Some((d, nome, porta)) = ler_anuncio(&v) {
                                    let endereco = format!("{}:{porta}", origem.ip());
                                    // Anúncio recebido vale de qualquer interface, mas o
                                    // caminho que sai de VPN/virtual/celular só entra se
                                    // o gerente liberou essa interface.
                                    if aceitar_anuncio(&d, &a, interfaces.as_deref(), &endereco) {
                                        let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
                                        let _ = registrar_par(&conn, &d, &nome, &endereco);
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(())
            }));
        }

        // 3. Puxada: a cada 4 segundos, pede o que há de novo a cada caixa,
        // pelo melhor caminho de cada um (veja `caminhos::ciclo`).
        {
            let parar = parar.clone();
            let parar_tarefa = parar.clone();
            let portas = portas.clone();
            tarefas.push(crate::supervisao::supervisionar(caminho.clone(), "puxada", parar_tarefa, move || {
                let mut ultima_limpeza = Instant::now() - Duration::from_secs(50 * 60);
                let mut ultima_reaplicacao = Instant::now() - Duration::from_secs(60);
                while !parar.load(Ordering::SeqCst) {
                    cochilar(&parar, Duration::from_secs(4));
                    if parar.load(Ordering::SeqCst) {
                        break;
                    }
                    crate::supervisao::ponto_de_queda(&caminho, "puxada");
                    let Ok(mut conn) = abrir_conexao(&caminho) else { continue };
                    if ultima_limpeza.elapsed() > Duration::from_secs(60 * 60) {
                        let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
                        let _ = compactar(&conn);
                        ultima_limpeza = Instant::now();
                    }
                    // Códigos das peças (mesmo com a troca de dados desligada):
                    // o distribuidor cria a faixa dele quando falta e as peças
                    // com código a definir recebem código.
                    {
                        let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = crate::faixas::manter(&mut conn) {
                            if matches!(e, Erro::Banco(_) | Erro::Interno(_)) {
                                crate::registro::anotar(&format!("códigos das peças: {e}"));
                            }
                        }
                    }
                    // Pendências guardadas: tenta de novo, da cópia guardada, a
                    // cada minuto (depois de uma atualização, na primeira volta).
                    if ultima_reaplicacao.elapsed() >= Duration::from_secs(60) {
                        let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = reaplicar_pendencias(&mut conn) {
                            crate::registro::anotar(&format!("troca de dados: pendências: {e}"));
                        }
                        ultima_reaplicacao = Instant::now();
                    }
                    let Ok(a) = ajustes(&conn) else { continue };
                    if !a.ligado || a.senha.is_empty() {
                        continue;
                    }
                    {
                        let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
                        let _ = caminhos::digitar(&conn, &digitados_completos(&a), agora_ms());
                    }
                    // A lista de interfaces é lida a cada ciclo: um cabo
                    // desconectado deixa de ser caminho já no ciclo seguinte.
                    let ambiente = caminhos::Ambiente {
                        proprio: a.dispositivo.clone(),
                        interfaces: rede_local::lista_ou_nada(rede_local::listar()),
                        liberadas: a.interfaces.clone(),
                        agora_ms: agora_ms(),
                        orcamento: caminhos::ORCAMENTO,
                    };
                    let (caminho_banco, parar_ciclo) = (caminho.clone(), parar.clone());
                    let porta_tcp = porta_tcp_de(&portas);
                    let mut puxar = |endereco: &str, saida: Option<&Saida>| {
                        puxar_por(&caminho_banco, endereco, porta_tcp, Some(&parar_ciclo), saida)
                    };
                    caminhos::ciclo(&conn, &ambiente, &|| parar.load(Ordering::SeqCst), &mut puxar);
                }
                Ok(())
            }));
        }

        Motor { parar, tarefas, portas, problemas, encerrado: false }
    }
}
