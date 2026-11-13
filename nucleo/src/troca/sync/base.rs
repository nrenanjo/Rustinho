//! Constantes do protocolo, a trava da troca (pausar/travar) e os lotes.
//!
//! Parte do `troca::sync` (passo 5.1: o texto veio do sync.rs sem mudança,
//! fora a visibilidade `pub(super)`, que tem o mesmo alcance de antes).

use super::*;

pub const PORTA_TCP: u16 = 47810;
pub const PORTA_UDP: u16 = 47811;
pub(super) const LOTE: i64 = 2000;
/// Resposta com um lote de alterações.
pub(super) const TAMANHO_MAXIMO: usize = 64 * 1024 * 1024;
/// Pedido de outro caixa (é sempre pequeno).
pub(super) const TAMANHO_PEDIDO: usize = 64 * 1024;
/// Conexões atendidas ao mesmo tempo (as outras esperam a próxima rodada).
pub(super) const CONEXOES_MAXIMAS: usize = 8;
/// Diferença de relógio entre caixas que já vira aviso.
pub(super) const RELOGIO_TOLERANCIA_MS: i64 = 2 * 60 * 1000;
/// Começo de toda mensagem do protocolo 3 (ordem e história das alterações).
pub(super) const MAGICA: &[u8; 4] = b"RST3";
/// Versão do protocolo, também dentro de cada pedido e resposta.
pub const PROTOCOLO: i64 = 3;
pub(super) const PROTOCOLO_DIFERENTE: &str =
    "O outro caixa está com outra versão do Rustinho. Atualize todos os caixas para a mesma versão.";
/// Recursos da troca de dados que este Rustinho entende. Vão no pedido e na
/// resposta (campo novo: um caixa de antes da 0.9.15 não manda e ignora).
/// "pendencias-v1": o caixa guarda inteira, em `sync_pendencia`, a alteração
/// que não consegue aplicar, e a numeração dele avança sem perdê-la.
/// "loja-v1" (FALHA-01): o caixa manda a identidade da loja e confere a do
/// outro antes de entregar ou aplicar qualquer coisa (veja `loja`).
pub const RECURSOS: &[&str] = &["pendencias-v1", crate::loja::RECURSO];
pub const RECURSO_PENDENCIAS: &str = "pendencias-v1";
/// Estrutura do banco (user_version) que entende o rascunho de OS (0.9.15).
pub const ESQUEMA_RASCUNHO: i64 = 23;
/// Caixa que não informa a estrutura é de antes da 0.9.15: tratado como a
/// 0.9.14 (estrutura 22).
pub const ESQUEMA_LEGADO: i64 = 22;
/// Depois de quantos dias sem confirmar a versão um caixa deixa de contar como
/// compatível (bloqueia rascunhos de OS novos até confirmar de novo).
pub const PRAZO_COMPATIBILIDADE_DIAS: i64 = 7;
/// Pendências recebidas guardadas de um mesmo caixa, no máximo. Passou disso,
/// a troca com ele para (sem avançar a numeração) até alguém olhar.
pub const PENDENCIAS_MAXIMAS: i64 = 1000;
/// Prefixo da configuração que diz que um caixa foi retirado da loja (vai
/// para todos os caixas; um caixa antigo ignora).
pub const PREFIXO_RETIRADO: &str = "caixa_retirado:";

/// Quantas versões anteriores cada alteração leva na história. É só um
/// atalho: a relação certa entre versões vem da cadeia de bases no log (as
/// versões compactadas continuam lá). Pouca coisa para não pesar no banco
/// (medido no teste `volume_do_historico`).
pub const HISTORIA_MAXIMA: usize = 8;
/// Marca no fim da história: ela não chega até o começo do registro.
pub(super) const INCOMPLETA: &str = "+";

/// Enquanto um backup é restaurado, a troca de dados espera (e vice-versa).
pub(super) static TRAVA: RwLock<()> = RwLock::new(());

/// Uma vaga de atendimento. Ao sair de cena (fim normal, erro ou pânico da
/// tarefa) a vaga volta (C-01): o contador nunca fica preso.
pub(super) struct Vaga(pub(super) Arc<AtomicUsize>);

impl Vaga {
    pub(super) fn pegar_de(contador: &Arc<AtomicUsize>) -> Option<Vaga> {
        let mut atual = contador.load(Ordering::SeqCst);
        loop {
            if atual >= CONEXOES_MAXIMAS {
                return None;
            }
            match contador.compare_exchange(atual, atual + 1, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => return Some(Vaga(contador.clone())),
                Err(outro) => atual = outro,
            }
        }
    }
}

/// Conexões atendidas ao mesmo tempo vindas de um mesmo endereço.
pub(super) const VAGAS_POR_ORIGEM: usize = 2;
/// Prazo para quem conectou mandar o pedido (um caixa de verdade manda na
/// hora; o pedido é pequeno).
pub(super) const PRAZO_DO_PEDIDO: Duration = Duration::from_secs(2);
/// Conexões que chegaram e ainda não mandaram nada (esperam numa fila, sem
/// tarefa e sem vaga; a mais antiga sai quando a fila enche).
pub(super) const NA_FILA_MAXIMO: usize = 64;

/// Uma conexão sendo atendida pelo servidor.
pub(super) struct Atendimento {
    pub(super) ip: std::net::IpAddr,
    pub(super) tarefa: thread::JoinHandle<()>,
}

/// Uma conexão na fila, esperando o pedido chegar.
pub(super) struct NaFila {
    pub(super) fluxo: TcpStream,
    pub(super) ip: std::net::IpAddr,
    pub(super) desde: Instant,
}

impl Drop for Vaga {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Para a troca de dados enquanto a guarda existir (restaurar backup).
pub fn pausar() -> RwLockWriteGuard<'static, ()> {
    #[cfg(test)]
    return pausar_nos_testes();
    #[cfg(not(test))]
    TRAVA.write().unwrap_or_else(|e| e.into_inner())
}

/// Só nos testes (0.9.18, C1): no programa há um banco por processo e o
/// outro caixa é outro processo. Nos testes, um teste que restaura backup e
/// outro que troca dados com um servidor do MESMO processo dividem esta
/// trava. A `RwLock` dá a vez a quem espera para escrever: com a restauração
/// esperando, o servidor do próprio processo não consegue ler, o cliente que
/// segura a leitura espera por ele, e os dois ficam parados até o prazo de
/// 30 s do cliente (os "os error 11" dos testes em paralelo). Aqui a
/// restauração tenta sem bloquear quem lê; ela entra no primeiro intervalo
/// sem leitores. Depois de pegar a trava, o efeito é o mesmo do programa:
/// ninguém troca dados até a restauração terminar.
#[cfg(test)]
pub(super) fn pausar_nos_testes() -> RwLockWriteGuard<'static, ()> {
    loop {
        match TRAVA.try_write() {
            Ok(g) => return g,
            Err(std::sync::TryLockError::Poisoned(e)) => return e.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => thread::sleep(Duration::from_millis(5)),
        }
    }
}

/// Só nos testes: a troca de dados está parada (alguém segura a pausa)?
#[cfg(test)]
pub(crate) fn pausada_para_teste() -> bool {
    matches!(TRAVA.try_read(), Err(std::sync::TryLockError::WouldBlock))
}

/// Para quem grava no meio da troca de dados (as tarefas deste módulo e o
/// ciclo de caminhos): respeita a pausa da restauração de backup.
pub(crate) fn travar() -> RwLockReadGuard<'static, ()> {
    TRAVA.read().unwrap_or_else(|e| e.into_inner())
}

/// Quantas alterações vão em cada resposta. Só os testes mudam isso, por banco
/// (chave `sync_lote_teste`), para provar a troca de caminho entre lotes.
/// Só nos testes: no máximo N lotes por puxada (a troca que para no meio e
/// continua depois, inclusive com outra origem no intervalo). O programa
/// final nunca tem isso.
pub(super) fn lotes_maximos(_conn: &Connection) -> Option<usize> {
    #[cfg(any(test, feature = "ancoras-de-teste"))]
    if let Ok(Some(v)) = config::ler(_conn, "sync_lotes_teste") {
        if let Ok(n) = v.parse::<usize>() {
            return Some(n.max(1));
        }
    }
    None
}

pub(super) fn lote(_conn: &Connection) -> i64 {
    // Também nos testes de fora do núcleo (tests/estavel, T8), que ligam o
    // recurso de teste; o programa final nunca tem isso.
    #[cfg(any(test, feature = "ancoras-de-teste"))]
    if let Ok(Some(v)) = config::ler(_conn, "sync_lote_teste") {
        if let Ok(n) = v.parse::<i64>() {
            return n.max(1);
        }
    }
    LOTE
}
