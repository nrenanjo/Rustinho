//! O fio da troca de dados (DT-P4-12, 0.9.19-dev.2): por onde passam o pedido e
//! a resposta.
//!
//! As regras da troca (loja, classes, recomeço, lotes, faixas, resumo) falam com
//! um [`Fio`] e não sabem o que há embaixo: hoje, a cifra da senha da rede
//! (protocolo 3, [`FioP3`]); com o protocolo 4, o canal autenticado. Este passo só
//! muda o lugar do código ("mover sem mudar"): o que vai e volta pela rede é
//! exatamente o mesmo de antes.

use super::*;

/// Por onde vão e voltam as mensagens de uma conexão da troca.
pub(super) trait Fio {
    /// Manda uma mensagem.
    fn enviar(&mut self, v: &Value) -> Resultado<()>;
    /// Recebe uma mensagem de até `limite` bytes em até `prazo` (desiste logo se
    /// pedirem para parar).
    fn receber(&mut self, limite: usize, prazo: Duration, parar: Option<&AtomicBool>) -> Resultado<Value>;
    /// O endereço do outro lado (para guardar o par e para o aviso de outra loja).
    fn origem(&self) -> Option<SocketAddr>;
}

/// O fio do protocolo 3: cada mensagem cifrada com a chave tirada da senha da rede.
pub(super) struct FioP3<'a> {
    pub(super) fluxo: TcpStream,
    pub(super) chaves: &'a Chaves,
}

impl Fio for FioP3<'_> {
    fn enviar(&mut self, v: &Value) -> Resultado<()> {
        enviar(&mut self.fluxo, self.chaves, v)
    }

    fn receber(&mut self, limite: usize, prazo: Duration, parar: Option<&AtomicBool>) -> Resultado<Value> {
        receber(&mut self.fluxo, self.chaves, limite, prazo, parar)
    }

    fn origem(&self) -> Option<SocketAddr> {
        self.fluxo.peer_addr().ok()
    }
}

/// Conecta a `alvo` (pela `saida` planejada, quando houver) com os prazos de
/// sempre da puxada e devolve o fio do protocolo 3 e a confirmação do caminho.
pub(super) fn abrir_fio_p3<'a>(
    chaves: &'a Chaves,
    alvo: &SocketAddr,
    saida: Option<&Saida>,
) -> Resultado<(FioP3<'a>, rede_local::Confirmacao)> {
    let (fluxo, conf) = rede_local::conectar(*alvo, saida, Duration::from_secs(3)).map_err(rede)?;
    fluxo.set_read_timeout(Some(Duration::from_secs(30))).ok();
    fluxo.set_write_timeout(Some(Duration::from_secs(10))).ok();
    Ok((FioP3 { fluxo, chaves }, conf))
}
