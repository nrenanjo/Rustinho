//! Chaves, derivação, cifrar e decifrar, o porteiro, enviar e receber.
//!
//! Parte do `troca::sync` (passo 5.1: o texto veio do sync.rs sem mudança,
//! fora a visibilidade `pub(super)`, que tem o mesmo alcance de antes).

use super::*;

/// Chaves da rede: a deste caixa (com o sal dele) e as dos outros, tiradas
/// da senha da rede por Argon2id e guardadas na memória (calcular demora).
#[derive(Clone)]
pub(super) struct Chaves {
    pub(super) senha: String,
    pub(super) sal: [u8; 16],
    pub(super) propria: [u8; 32],
}

/// (resumo da senha, sal) → chave.
pub(super) type CacheChaves = Mutex<HashMap<([u8; 32], [u8; 16]), [u8; 32]>>;

pub(super) fn cache() -> &'static CacheChaves {
    static CACHE: std::sync::OnceLock<CacheChaves> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn derivar(senha: &str, sal: &[u8; 16]) -> Resultado<[u8; 32]> {
    // O portátil tira a chave de outro jeito: nunca a mesma de um caixa
    // instalado, mesmo com a mesma senha (veja `portatil`).
    let senha = crate::portatil::senha_da_rede(senha);
    let senha = senha.as_ref();
    let id: [u8; 32] = Sha256::digest(senha.as_bytes()).into();
    if let Some(k) = cache().lock().unwrap_or_else(|e| e.into_inner()).get(&(id, *sal)) {
        return Ok(*k);
    }
    let mut k = [0u8; 32];
    crate::usuarios::argon2()
        .hash_password_into(senha.as_bytes(), sal, &mut k)
        .map_err(|e| Erro::Interno(format!("não foi possível proteger a rede: {e}")))?;
    let mut c = cache().lock().unwrap_or_else(|e| e.into_inner());
    if c.len() > 64 {
        c.clear();
    }
    c.insert((id, *sal), k);
    Ok(k)
}

impl Chaves {
    pub(super) fn de(conn: &Connection, senha: &str) -> Resultado<Chaves> {
        let sal =
            match config::ler(conn, "sync_sal")?.and_then(|t| de_hex(&t)).and_then(|v| <[u8; 16]>::try_from(v).ok()) {
                Some(s) => s,
                None => {
                    let mut s = [0u8; 16];
                    getrandom::fill(&mut s).map_err(|e| Erro::Interno(format!("sem números aleatórios: {e}")))?;
                    config::gravar(conn, "sync_sal", &s.iter().map(|b| format!("{b:02x}")).collect::<String>())?;
                    s
                }
            };
        Ok(Chaves { senha: senha.to_string(), sal, propria: derivar(senha, &sal)? })
    }
}

pub(super) fn de_hex(t: &str) -> Option<Vec<u8>> {
    if !t.len().is_multiple_of(2) {
        return None;
    }
    (0..t.len()).step_by(2).map(|i| u8::from_str_radix(t.get(i..i + 2)?, 16).ok()).collect()
}

/// Mensagem: "RST2", sal de quem mandou (16), nonce (12), texto cifrado.
pub(super) fn cifrar(chaves: &Chaves, dados: &[u8]) -> Resultado<Vec<u8>> {
    let cifra =
        ChaCha20Poly1305::new_from_slice(&chaves.propria).map_err(|_| Erro::Interno("chave inválida".into()))?;
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).map_err(|e| Erro::Interno(format!("sem números aleatórios: {e}")))?;
    let mut saida = MAGICA.to_vec();
    saida.extend(chaves.sal);
    saida.extend(nonce);
    let corpo = cifra.encrypt(&nonce.into(), dados).map_err(|_| Erro::Interno("falha ao criptografar".into()))?;
    saida.extend(corpo);
    Ok(saida)
}

/// R-09: um pacote com um sal nunca visto obriga a uma conta cara (Argon2)
/// antes de saber se a senha confere. Para ninguém na rede ocupar o caixa
/// mandando sais aleatórios, só `SAIS_NOVOS_POR_MINUTO` contas dessas por
/// minuto (somando UDP e TCP); os sais que já deram certo ficam guardados e
/// não gastam nada. A conta do próprio sal não entra aqui.
pub(super) const SAIS_NOVOS_POR_MINUTO: u32 = 10;
pub(super) const SAIS_GUARDADOS: usize = 64;

/// Contas caras que um mesmo endereço pode pedir por minuto. Um aparelho
/// sozinho não gasta o limite de todos; um caixa novo na rede continua
/// entrando. (Antes de conferir a senha não há como saber quem mandou de
/// verdade: o endereço é o que existe. Quem trocar de endereço esbarra no
/// limite de todos, `SAIS_NOVOS_POR_MINUTO`.)
pub(super) const SAIS_NOVOS_POR_ORIGEM: u32 = 3;
/// Endereços diferentes lembrados por minuto (o controle não cresce sem fim).
pub(super) const ORIGENS_POR_MINUTO: usize = 256;

pub(super) struct Porteiro {
    pub(super) inicio: Instant,
    pub(super) usados: u32,
    pub(super) por_origem: HashMap<Option<std::net::IpAddr>, u32>,
}

impl Porteiro {
    pub(super) fn novo() -> Porteiro {
        Porteiro { inicio: Instant::now(), usados: 0, por_origem: HashMap::new() }
    }

    /// Pode fazer mais uma conta agora, para este endereço?
    pub(super) fn permitir(&mut self, agora: Instant, origem: Option<std::net::IpAddr>) -> bool {
        if agora.duration_since(self.inicio) >= Duration::from_secs(60) {
            self.inicio = agora;
            self.usados = 0;
            self.por_origem.clear();
        }
        if self.usados >= SAIS_NOVOS_POR_MINUTO {
            return false;
        }
        let ja = self.por_origem.get(&origem).copied().unwrap_or(0);
        if ja >= SAIS_NOVOS_POR_ORIGEM || (ja == 0 && self.por_origem.len() >= ORIGENS_POR_MINUTO) {
            return false;
        }
        self.por_origem.insert(origem, ja + 1);
        self.usados += 1;
        true
    }
}

/// Pode fazer mais uma conta cara agora? No programa, um controle só para
/// todas as tarefas; nos testes, um por tarefa (os testes rodam juntos e um
/// não pode gastar o limite do outro).
pub(super) fn pode_contar(origem: Option<std::net::IpAddr>) -> bool {
    #[cfg(not(test))]
    {
        static P: std::sync::OnceLock<Mutex<Porteiro>> = std::sync::OnceLock::new();
        P.get_or_init(|| Mutex::new(Porteiro::novo()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .permitir(Instant::now(), origem)
    }
    #[cfg(test)]
    {
        thread_local! {
            static P: std::cell::RefCell<Porteiro> = std::cell::RefCell::new(Porteiro::novo());
        }
        P.with(|p| p.borrow_mut().permitir(Instant::now(), origem))
    }
}

/// Chaves dos outros caixas que já conferiram (sal e senha certos).
pub(super) fn conhecidos() -> &'static CacheChaves {
    static C: std::sync::OnceLock<CacheChaves> = std::sync::OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Erro quando o outro caixa fala outra versão (ou a senha não confere).
pub(super) fn decifrar(
    chaves: &Chaves,
    dados: &[u8],
    origem: Option<std::net::IpAddr>,
) -> Result<Vec<u8>, &'static str> {
    if dados.len() < 4 + 16 + 12 + 16 || &dados[..4] != MAGICA {
        return Err("O outro caixa está com outra versão do Rustinho. Atualize todos os caixas para a mesma versão.");
    }
    let mut sal = [0u8; 16];
    sal.copy_from_slice(&dados[4..20]);
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&dados[20..32]);
    let abrir = |chave: &[u8; 32]| -> Option<Vec<u8>> {
        let cifra = ChaCha20Poly1305::new_from_slice(chave).ok()?;
        cifra.decrypt(&nonce.into(), &dados[32..]).ok()
    };
    let senha_errada = "A senha da rede não confere com a deste caixa.";
    if sal == chaves.sal {
        return abrir(&chaves.propria).ok_or(senha_errada);
    }
    let id: [u8; 32] = Sha256::digest(chaves.senha.as_bytes()).into();
    let guardada = conhecidos().lock().unwrap_or_else(|e| e.into_inner()).get(&(id, sal)).copied();
    if let Some(k) = guardada {
        return abrir(&k).ok_or(senha_errada);
    }
    if !pode_contar(origem) {
        return Err("Muitos pedidos de outros aparelhos agora; a troca de dados tenta de novo em instantes.");
    }
    let mut k = [0u8; 32];
    crate::usuarios::argon2()
        .hash_password_into(chaves.senha.as_bytes(), &sal, &mut k)
        .map_err(|_| "chave inválida")?;
    let claro = abrir(&k).ok_or(senha_errada)?;
    // Só guarda depois de conferir: sal inventado nunca ocupa lugar.
    let mut c = conhecidos().lock().unwrap_or_else(|e| e.into_inner());
    if c.len() >= SAIS_GUARDADOS {
        if let Some(qualquer) = c.keys().next().copied() {
            c.remove(&qualquer);
        }
    }
    c.insert((id, sal), k);
    Ok(claro)
}

pub(super) fn enviar(fluxo: &mut TcpStream, chaves: &Chaves, v: &Value) -> Resultado<()> {
    let corpo = cifrar(chaves, v.to_string().as_bytes())?;
    let tam = (corpo.len() as u32).to_be_bytes();
    fluxo.write_all(&tam).and_then(|_| fluxo.write_all(&corpo)).map_err(rede)?;
    Ok(())
}

/// Lê exatamente `buf.len()` bytes em até `prazo`, aos pedaços: se pedirem
/// para parar (fechar o programa), desiste logo em vez de esperar o prazo.
pub(super) fn ler_tudo(
    fluxo: &mut TcpStream,
    buf: &mut [u8],
    prazo: Duration,
    parar: Option<&AtomicBool>,
) -> Resultado<()> {
    let fim = Instant::now() + prazo;
    fluxo.set_read_timeout(Some(Duration::from_millis(200))).ok();
    let mut lido = 0;
    while lido < buf.len() {
        if parar.is_some_and(|p| p.load(Ordering::SeqCst)) {
            return Err(Erro::validacao("A troca de dados parou (o Rustinho está fechando)."));
        }
        match fluxo.read(&mut buf[lido..]) {
            Ok(0) => return Err(rede(std::io::ErrorKind::UnexpectedEof.into())),
            Ok(n) => lido += n,
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                if Instant::now() >= fim {
                    return Err(rede(e));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(rede(e)),
        }
    }
    Ok(())
}

pub(super) fn receber(
    fluxo: &mut TcpStream,
    chaves: &Chaves,
    limite: usize,
    prazo: Duration,
    parar: Option<&AtomicBool>,
) -> Resultado<Value> {
    let origem = fluxo.peer_addr().ok().map(|a| a.ip());
    let mut tam = [0u8; 4];
    ler_tudo(fluxo, &mut tam, prazo, parar)?;
    let tam = u32::from_be_bytes(tam) as usize;
    if tam > limite {
        return Err(Erro::validacao("Mensagem grande demais."));
    }
    let mut corpo = vec![0u8; tam];
    ler_tudo(fluxo, &mut corpo, prazo, parar)?;
    let claro = decifrar(chaves, &corpo, origem).map_err(Erro::validacao)?;
    Ok(serde_json::from_slice(&claro)?)
}

pub(super) fn agora_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub(super) fn rede(e: std::io::Error) -> Erro {
    Erro::validacao(format!("Falha na rede: {e}"))
}

// ---------------------------------------------------------------------------
// Configuração e estado.
// ---------------------------------------------------------------------------
