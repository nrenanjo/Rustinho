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
    /// A chave autenticada do outro caixa, quando o fio é o do protocolo 4.
    fn chave_do_par(&self) -> Option<crate::troca::protocolo4::Chave> {
        None
    }
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

/// O fio até `alvo` (DT-P4-28):
/// - este caixa fala o P4 e o par (`dono`) tem chave no registro → **só** P4, com a
///   chave do canal tendo de ser a dele; falhou, erro (sem cair para o P3:
///   anti-rebaixamento, DT-P4-14);
/// - este caixa fala o P4 e o par ainda é desconhecido → tenta o P4 com todas as
///   chaves autorizadas; se o outro lado não fala o P4, o P3;
/// - senão, o P3 (até a T010, que põe a convivência na frente).
pub(super) fn abrir_fio<'a>(
    conn: &Connection,
    chaves: Option<&'a Chaves>,
    alvo: &SocketAddr,
    saida: Option<&Saida>,
    ctx: Option<&p4::ContextoP4>,
    dono: &str,
    parar: Option<&AtomicBool>,
) -> Resultado<(Box<dyn Fio + 'a>, rede_local::Confirmacao)> {
    if let Some(ctx) = ctx {
        let chave_dono = if dono.is_empty() { None } else { ctx.chave_de(conn, dono)? };
        if chave_dono.is_some() || dono.is_empty() {
            let outras = ctx.outras_autorizadas(conn)?;
            let (fluxo, conf) = rede_local::conectar(*alvo, saida, Duration::from_secs(3)).map_err(rede)?;
            match crate::troca::protocolo4::canal::conectar_autorizado(
                fluxo,
                &ctx.identidade.identidade,
                &outras,
                p4::PRAZO_DO_APERTO,
                parar,
            ) {
                Ok(canal) => {
                    if chave_dono.is_some_and(|k| k != canal.chave_do_par()) {
                        p4::anotar_recusa_de_identidade(dono, &canal.chave_do_par());
                        return Err(Erro::validacao(p4::TEXTO_CAIXA_NAO_CONFERE));
                    }
                    let fio = p4::FioP4::novo(canal, p4::registro_em_json(conn, &ctx.loja)?);
                    return Ok((Box::new(fio), conf));
                }
                Err(e) if chave_dono.is_some() => return Err(e),
                Err(_) => {}
            }
        }
    }
    // O protocolo 3 precisa da senha da rede e, num caixa do protocolo 4, da
    // convivência ligada (T010, A4).
    let Some(chaves) = chaves else {
        return Err(Erro::validacao(TEXTO_SEM_SENHA_NEM_P4));
    };
    if !p4::p3_permitido(conn) {
        return Err(Erro::validacao(p4::TEXTO_P3_RECUSADO));
    }
    let (f, conf) = abrir_fio_p3(chaves, alvo, saida)?;
    Ok((Box::new(f), conf))
}

/// Texto para a tela: este caixa não tem a senha da rede e o outro não fala o P4.
pub(super) const TEXTO_SEM_SENHA_NEM_P4: &str =
    "Este caixa entrou na loja pelo convite e só troca dados com caixas atualizados. Atualize o outro caixa.";

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
