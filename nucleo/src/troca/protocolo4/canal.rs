//! Canal autenticado TLS 1.3 com chaves públicas cruas Ed25519 (RFC 7250,
//! decisão `A5`) e prova de convite amarrada ao canal (*channel binding*,
//! decisão `A6`) — protocolo 4 (`0.9.19-dev.2`).
//!
//! ## Garantias de segurança
//! - `#![forbid(unsafe_code)]`: zero código inseguro neste módulo (e zero
//!   `unsafe` no próprio `rustls`).
//! - **Somente TLS 1.3** com provedor `ring` e autenticação **mútua** via
//!   chaves públicas cruas Ed25519 (RFC 7250, sem X.509 nem `rcgen`).
//! - **Dois tipos de canal (F-04):** o transporte TLS é interno ao módulo e só
//!   chega a quem chama como [`CanalAutenticado`] (o outro lado provou uma chave
//!   do conjunto autorizado, conferida pelo verificador **e** de novo depois do
//!   aperto de mão; só ele carrega dados da loja) ou como [`CanalIngresso`] (o
//!   outro lado provou só ter uma chave Ed25519 qualquer; carrega apenas as
//!   [`MensagemIngresso`] do convite, lidas de forma estrita). Não existe jeito de
//!   montar um ouvinte "aceita qualquer chave" que entregue um canal de dados.
//! - **Modo autorizado:** recusa qualquer par fora do conjunto de chaves
//!   autorizadas *durante o aperto de mão TLS*, antes de trocar qualquer byte de
//!   dados da loja. Conjunto vazio falha fechado (recusa tudo).
//! - **Modo ingresso (`A6`):** quando há convite ativo, o caixa novo prova a
//!   posse do segredo de 128 bits via `HMAC-SHA256` sobre o material exportado
//!   da sessão TLS (`export_keying_material`, RFC 5705), vinculando
//!   criptograficamente `chave_cliente`, `chave_servidor` e `Pedido` (I3) à
//!   transcrição TLS. O servidor prova em retorno o conhecimento do segredo
//!   (via `segredo_hash`) antes de o cliente confiar na raiz ou nas aprovações,
//!   impedindo servidor falso / MITM mesmo quando o convite é digitado sem QR.

#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use ring::hmac;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{ring as provedor, verify_tls13_signature_with_raw_key, WebPkiSupportedAlgorithms};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer, ServerName, SubjectPublicKeyInfoDer, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::sign::CertifiedKey;
use rustls::{
    ClientConfig, ClientConnection, ConnectionCommon, DigitallySignedStruct, DistinguishedName, Error as TlsError,
    ProtocolVersion, ServerConfig, ServerConnection, SideData, SignatureScheme,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::aprovados::{self, Pedido};
use super::cofre;
use super::{apagar, de_hex, hex, Chave, Identidade, Segredo};
use crate::erro::{Erro, Resultado};

/// Prefixo DER SubjectPublicKeyInfo (RFC 8410) de uma chave pública Ed25519 (12 bytes).
/// Seguido exatamente dos 32 bytes da chave pública (total 44 bytes).
const PREFIXO_SPKI_ED25519: [u8; 12] = [0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00];

/// Rótulo RFC 5705 do material exportado para amarrar o convite à sessão TLS 1.3.
const ROTULO_EXPORTADOR_CONVITE: &[u8] = b"EXPORTER-rustinho-p4-convite-v1";
const CONTEXTO_EXPORTADOR_CONVITE: &[u8] = b"rustinho/p4/ingresso/v1";

/// Limite máximo para o texto de entrada do QR code de convite.
const LIMITE_QR_BYTES: usize = 512;

/// Política de verificação da chave pública Ed25519 do outro lado no handshake TLS 1.3
/// (interna: quem chama escolhe o tipo de canal, F-04).
#[derive(Clone, Debug)]
enum PoliticaChaves {
    /// Aceita exclusivamente chaves pertencentes ao conjunto autorizado.
    /// Conjunto vazio falha fechado (recusa todas as conexões).
    SomenteAutorizadas(HashSet<Chave>),
    /// Aceita qualquer chave pública Ed25519 bem-formada (que prove posse da
    /// privada no `CertificateVerify` do TLS 1.3) para o fluxo de ingresso por
    /// convite; a autorização efetiva vem obrigatoriamente da prova HMAC do
    /// convite antes de qualquer dado da loja.
    QualquerChaveValidaParaIngresso,
}

fn montar_spki(publica: &Chave) -> Vec<u8> {
    let mut v = Vec::with_capacity(44);
    v.extend_from_slice(&PREFIXO_SPKI_ED25519);
    v.extend_from_slice(publica);
    v
}

/// Extrai os 32 bytes da chave pública Ed25519 de um SPKI DER (44 bytes exatos).
pub fn extrair_chave_spki(cert: &CertificateDer<'_>) -> Option<Chave> {
    let bytes = cert.as_ref();
    if bytes.len() != 44 || bytes[..12] != PREFIXO_SPKI_ED25519 {
        return None;
    }
    let mut chave = [0u8; 32];
    chave.copy_from_slice(&bytes[12..44]);
    Some(chave)
}

fn chave_certificada(id: &Identidade) -> Resultado<Arc<CertifiedKey>> {
    // M1: empresta diretamente do `Segredo` da `Identidade` sem clonar a chave privada no heap.
    let chave_der = PrivatePkcs8KeyDer::from(id.pkcs8_der());
    let assinador = provedor::sign::any_eddsa_type(&chave_der)
        .map_err(|_| Erro::Interno("não foi possível preparar a chave Ed25519 para o canal TLS".into()))?;
    Ok(Arc::new(CertifiedKey::new(vec![CertificateDer::from(montar_spki(&id.publica()))], assinador)))
}

#[derive(Debug)]
struct VerificadorRpk {
    politica: PoliticaChaves,
    algs: WebPkiSupportedAlgorithms,
}

impl VerificadorRpk {
    fn novo(politica: PoliticaChaves) -> Arc<Self> {
        Arc::new(Self { politica, algs: provedor::default_provider().signature_verification_algorithms })
    }

    fn conferir_chave(&self, cert: &CertificateDer<'_>) -> Result<(), TlsError> {
        let Some(chave) = extrair_chave_spki(cert) else {
            return Err(TlsError::General("chave pública crua Ed25519 inválida".into()));
        };
        match &self.politica {
            PoliticaChaves::SomenteAutorizadas(autorizadas) => {
                if autorizadas.contains(&chave) {
                    Ok(())
                } else {
                    Err(TlsError::General("chave do caixa não autorizada nesta loja".into()))
                }
            }
            PoliticaChaves::QualquerChaveValidaParaIngresso => Ok(()),
        }
    }

    fn conferir_assinatura(
        &self,
        mensagem: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        if dss.scheme != SignatureScheme::ED25519 {
            return Err(TlsError::General("somente assinatura Ed25519 é aceita no protocolo 4".into()));
        }
        verify_tls13_signature_with_raw_key(mensagem, &SubjectPublicKeyInfoDer::from(cert.as_ref()), dss, &self.algs)
    }
}

impl ServerCertVerifier for VerificadorRpk {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        if !intermediates.is_empty() {
            return Err(TlsError::General("chave pública crua não aceita cadeia intermediária".into()));
        }
        self.conferir_chave(end_entity).map(|_| ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Err(TlsError::General("somente TLS 1.3 é aceito no protocolo 4".into()))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        self.conferir_assinatura(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ED25519]
    }

    fn requires_raw_public_keys(&self) -> bool {
        true
    }
}

impl ClientCertVerifier for VerificadorRpk {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, TlsError> {
        if !intermediates.is_empty() {
            return Err(TlsError::General("chave pública crua não aceita cadeia intermediária".into()));
        }
        self.conferir_chave(end_entity).map(|_| ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Err(TlsError::General("somente TLS 1.3 é aceito no protocolo 4".into()))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        self.conferir_assinatura(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ED25519]
    }

    fn requires_raw_public_keys(&self) -> bool {
        true
    }
}

/// Constrói a configuração de servidor TLS 1.3 com RPK Ed25519 mútuo.
fn config_servidor(eu: &Identidade, politica: PoliticaChaves) -> Resultado<Arc<ServerConfig>> {
    let mut cfg = ServerConfig::builder_with_provider(Arc::new(provedor::default_provider()))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|e| Erro::Interno(format!("erro ao configurar TLS 1.3 no servidor: {e}")))?
        .with_client_cert_verifier(VerificadorRpk::novo(politica))
        .with_cert_resolver(Arc::new(rustls::server::AlwaysResolvesServerRawPublicKeys::new(chave_certificada(eu)?)));
    // Sem retomada de sessão por ticket: cada conexão revalida a chave contra o registro atual (revogação imediata!).
    cfg.send_tls13_tickets = 0;
    cfg.ticketer = Arc::new(SemTickets);
    cfg.session_storage = Arc::new(rustls::server::NoServerSessionStorage {});
    Ok(Arc::new(cfg))
}

/// Constrói a configuração de cliente TLS 1.3 com RPK Ed25519 mútuo.
fn config_cliente(eu: &Identidade, politica: PoliticaChaves) -> Resultado<Arc<ClientConfig>> {
    let mut cfg = ClientConfig::builder_with_provider(Arc::new(provedor::default_provider()))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|e| Erro::Interno(format!("erro ao configurar TLS 1.3 no cliente: {e}")))?
        .dangerous()
        .with_custom_certificate_verifier(VerificadorRpk::novo(politica))
        .with_client_cert_resolver(Arc::new(rustls::client::AlwaysResolvesClientRawPublicKeys::new(
            chave_certificada(eu)?,
        )));
    cfg.resumption = rustls::client::Resumption::disabled();
    Ok(Arc::new(cfg))
}

/// Desabilita tickets de sessão no servidor para garantir que toda conexão TLS
/// passe sempre pelo `ClientCertVerifier` com o registro de revogação vigente.
#[derive(Debug)]
struct SemTickets;

impl rustls::server::ProducesTickets for SemTickets {
    fn enabled(&self) -> bool {
        false
    }
    fn lifetime(&self) -> u32 {
        0
    }
    fn encrypt(&self, _plain: &[u8]) -> Option<Vec<u8>> {
        None
    }
    fn decrypt(&self, _cipher: &[u8]) -> Option<Vec<u8>> {
        None
    }
}

// ---------------------------------------------------------------------------
// Conexão TLS 1.3 sobre `TcpStream` com enquadramento seguro, prazo e parada.
// ---------------------------------------------------------------------------

/// O transporte TLS 1.3 sobre TCP (cliente ou servidor). Interno ao módulo: quem
/// chama recebe [`CanalAutenticado`] ou [`CanalIngresso`] (F-04).
struct CanalTls<C> {
    fluxo: TcpStream,
    conn: C,
    chave_par: Chave,
    exportador_convite: [u8; 32],
}

fn abrir_cliente(
    fluxo: TcpStream,
    eu: &Identidade,
    politica: PoliticaChaves,
    prazo: Duration,
    parar: Option<&AtomicBool>,
) -> Resultado<CanalTls<ClientConnection>> {
    let cfg = config_cliente(eu, politica)?;
    let nome = ServerName::try_from("rustinho-caixa").map_err(|_| Erro::Interno("nome TLS inválido".into()))?;
    let conn = ClientConnection::new(cfg, nome)
        .map_err(|e| Erro::Interno(format!("falha ao iniciar cliente TLS 1.3: {e}")))?;
    finalizar_abertura(fluxo, conn, prazo, parar)
}

fn abrir_servidor(
    fluxo: TcpStream,
    eu: &Identidade,
    politica: PoliticaChaves,
    prazo: Duration,
    parar: Option<&AtomicBool>,
) -> Resultado<CanalTls<ServerConnection>> {
    let cfg = config_servidor(eu, politica)?;
    let conn =
        ServerConnection::new(cfg).map_err(|e| Erro::Interno(format!("falha ao iniciar servidor TLS 1.3: {e}")))?;
    finalizar_abertura(fluxo, conn, prazo, parar)
}

/// Canal TLS 1.3 em que o outro lado provou, no aperto de mão, a posse de uma chave
/// do conjunto autorizado (conferida pelo verificador e **de novo** depois do aperto
/// de mão). É o único canal que carrega dados da loja.
pub struct CanalAutenticado<C>(CanalTls<C>);
pub type CanalAutenticadoCliente = CanalAutenticado<ClientConnection>;
pub type CanalAutenticadoServidor = CanalAutenticado<ServerConnection>;

/// Canal TLS 1.3 de entrada por convite (A6): o outro lado provou só ter a privada de
/// uma chave Ed25519 qualquer. Carrega apenas as [`MensagemIngresso`] (leitura
/// estrita): nada da loja passa por ele.
pub struct CanalIngresso<C>(CanalTls<C>);
pub type CanalIngressoCliente = CanalIngresso<ClientConnection>;
pub type CanalIngressoServidor = CanalIngresso<ServerConnection>;

/// O que o servidor aceitou numa conexão em que convites estão ativos.
pub enum Aceito {
    /// A chave do outro lado é autorizada nesta loja: canal de dados.
    Autenticado(CanalAutenticadoServidor),
    /// Chave desconhecida: só o ingresso por convite.
    Ingresso(CanalIngressoServidor),
}

fn exigir_autorizada(chave: &Chave, autorizadas: &HashSet<Chave>) -> Resultado<()> {
    if autorizadas.contains(chave) {
        Ok(())
    } else {
        Err(Erro::validacao(TEXTO_RECUSA))
    }
}

/// Servidor: aceita só caixas autorizados (`autorizadas` vazio recusa todos).
pub fn aceitar_autorizado(
    fluxo: TcpStream,
    eu: &Identidade,
    autorizadas: &HashSet<Chave>,
    prazo: Duration,
    parar: Option<&AtomicBool>,
) -> Resultado<CanalAutenticadoServidor> {
    let canal = abrir_servidor(fluxo, eu, PoliticaChaves::SomenteAutorizadas(autorizadas.clone()), prazo, parar)?;
    exigir_autorizada(&canal.chave_par, autorizadas)?;
    Ok(CanalAutenticado(canal))
}

/// Servidor com convite ativo: o aperto de mão aceita qualquer chave Ed25519 válida;
/// depois, chave autorizada → [`Aceito::Autenticado`]; senão → [`Aceito::Ingresso`].
pub fn aceitar_com_ingresso(
    fluxo: TcpStream,
    eu: &Identidade,
    autorizadas: &HashSet<Chave>,
    prazo: Duration,
    parar: Option<&AtomicBool>,
) -> Resultado<Aceito> {
    let canal = abrir_servidor(fluxo, eu, PoliticaChaves::QualquerChaveValidaParaIngresso, prazo, parar)?;
    Ok(if autorizadas.contains(&canal.chave_par) {
        Aceito::Autenticado(CanalAutenticado(canal))
    } else {
        Aceito::Ingresso(CanalIngresso(canal))
    })
}

/// Cliente: conecta só a um servidor de chave autorizada (`autorizadas` vazio recusa todos).
pub fn conectar_autorizado(
    fluxo: TcpStream,
    eu: &Identidade,
    autorizadas: &HashSet<Chave>,
    prazo: Duration,
    parar: Option<&AtomicBool>,
) -> Resultado<CanalAutenticadoCliente> {
    let canal = abrir_cliente(fluxo, eu, PoliticaChaves::SomenteAutorizadas(autorizadas.clone()), prazo, parar)?;
    exigir_autorizada(&canal.chave_par, autorizadas)?;
    Ok(CanalAutenticado(canal))
}

/// Cliente (caixa novo) entrando por convite. `servidor_esperado`: a chave do caixa
/// aprovador quando veio no QR; `None` quando o convite foi digitado — aí a
/// autenticidade do servidor vem só da prova do convite
/// ([`verificar_prova_servidor`]), que tem de ser conferida antes de confiar em
/// qualquer coisa que ele mandar.
pub fn conectar_para_ingresso(
    fluxo: TcpStream,
    eu: &Identidade,
    servidor_esperado: Option<Chave>,
    prazo: Duration,
    parar: Option<&AtomicBool>,
) -> Resultado<CanalIngressoCliente> {
    let politica = match servidor_esperado {
        Some(k) => PoliticaChaves::SomenteAutorizadas(HashSet::from([k])),
        None => PoliticaChaves::QualquerChaveValidaParaIngresso,
    };
    Ok(CanalIngresso(abrir_cliente(fluxo, eu, politica, prazo, parar)?))
}

impl<C, Data> CanalAutenticado<C>
where
    Data: SideData,
    C: std::ops::DerefMut<Target = ConnectionCommon<Data>>,
{
    /// A chave pública Ed25519 autenticada do outro caixa (autorizada nesta loja).
    pub fn chave_do_par(&self) -> Chave {
        self.0.chave_par
    }

    /// Material exportado RFC 5705 desta sessão (para a retomada do ingresso).
    pub fn exportador_convite(&self) -> &[u8; 32] {
        &self.0.exportador_convite
    }

    /// Endereço IP remoto do socket TCP (para auditoria/registro).
    pub fn peer_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.0.fluxo.peer_addr()
    }

    /// Envia um quadro JSON (`u32` big-endian + corpo UTF-8).
    pub fn enviar_json(&mut self, valor: &Value, prazo: Duration, parar: Option<&AtomicBool>) -> Resultado<()> {
        self.0.enviar_json(valor, prazo, parar)
    }

    /// Recebe um quadro JSON respeitando estritamente `limite` bytes (recusa antes de alocar).
    pub fn receber_json(&mut self, limite: usize, prazo: Duration, parar: Option<&AtomicBool>) -> Resultado<Value> {
        self.0.receber_json(limite, prazo, parar)
    }

    /// Fecha graciosamente o canal TLS enviando `close_notify`.
    pub fn encerrar(self) {
        self.0.encerrar()
    }
}

impl<C, Data> CanalIngresso<C>
where
    Data: SideData,
    C: std::ops::DerefMut<Target = ConnectionCommon<Data>>,
{
    /// A chave Ed25519 do outro lado (provada no aperto de mão, **não** autorizada).
    pub fn chave_do_par(&self) -> Chave {
        self.0.chave_par
    }

    /// Material exportado RFC 5705 desta sessão (amarra a prova do convite ao canal).
    pub fn exportador_convite(&self) -> &[u8; 32] {
        &self.0.exportador_convite
    }

    /// Endereço IP remoto do socket TCP (para auditoria/registro).
    pub fn peer_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.0.fluxo.peer_addr()
    }

    /// Envia uma mensagem do ingresso.
    pub fn enviar(&mut self, msg: &MensagemIngresso, prazo: Duration, parar: Option<&AtomicBool>) -> Resultado<()> {
        self.0.enviar_json(&msg.para_json()?, prazo, parar)
    }

    /// Recebe uma mensagem do ingresso; qualquer outra coisa (um pedido da troca,
    /// campos a mais, tipo desconhecido) é recusada.
    pub fn receber(
        &mut self,
        limite: usize,
        prazo: Duration,
        parar: Option<&AtomicBool>,
    ) -> Resultado<MensagemIngresso> {
        MensagemIngresso::de_json(self.0.receber_json(limite, prazo, parar)?)
    }

    /// Fecha graciosamente o canal TLS enviando `close_notify`.
    pub fn encerrar(self) {
        self.0.encerrar()
    }
}

/// As únicas mensagens que passam pelo [`CanalIngresso`] (formato `{"tipo": ..., ...}`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MensagemIngresso {
    /// Caixa novo → loja: o pedido (chaves e `dispositivo`, assinado) e a prova do convite.
    Ingressar { pedido: Pedido, prova: ProvaIngressoCliente },
    /// Loja → caixa novo: aprovado. A prova de que a loja conhece o convite desta
    /// sessão, a raiz e a loja (conferidas pela prova) e o registro de caixas.
    Aprovado { prova_servidor: String, raiz: Chave, loja_id: String, linhas: aprovados::Linhas },
    /// Loja → caixa novo: recusado (texto para a tela).
    Recusado { motivo: String },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CorpoIngressar {
    pedido: Pedido,
    prova: ProvaIngressoCliente,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CorpoAprovado {
    prova_servidor: String,
    raiz: String,
    loja_id: String,
    linhas: aprovados::Linhas,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CorpoRecusado {
    motivo: String,
}

impl MensagemIngresso {
    /// A mensagem em JSON (`{"tipo": ..., ...campos}`).
    pub fn para_json(&self) -> Resultado<Value> {
        let (tipo, corpo) = match self {
            MensagemIngresso::Ingressar { pedido, prova } => {
                ("ingressar", serde_json::to_value(CorpoIngressar { pedido: pedido.clone(), prova: prova.clone() })?)
            }
            MensagemIngresso::Aprovado { prova_servidor, raiz, loja_id, linhas } => (
                "aprovado",
                serde_json::to_value(CorpoAprovado {
                    prova_servidor: prova_servidor.clone(),
                    raiz: hex(raiz),
                    loja_id: loja_id.clone(),
                    linhas: linhas.clone(),
                })?,
            ),
            MensagemIngresso::Recusado { motivo } => {
                ("recusado", serde_json::to_value(CorpoRecusado { motivo: motivo.clone() })?)
            }
        };
        let mut v = corpo;
        v.as_object_mut()
            .ok_or_else(|| Erro::Interno("mensagem de ingresso sem campos".into()))?
            .insert("tipo".into(), Value::String(tipo.into()));
        Ok(v)
    }

    /// Lê uma mensagem do ingresso de forma estrita: só os tipos conhecidos, só os
    /// campos de cada um (o resto é recusado, F-04 e observação 10 da revisão).
    pub fn de_json(v: Value) -> Resultado<MensagemIngresso> {
        let recusa = || Erro::validacao(TEXTO_MENSAGEM_INVALIDA);
        let Value::Object(mut obj) = v else {
            return Err(recusa());
        };
        let tipo = match obj.remove("tipo") {
            Some(Value::String(t)) => t,
            _ => return Err(recusa()),
        };
        let corpo = Value::Object(obj);
        match tipo.as_str() {
            "ingressar" => {
                let c: CorpoIngressar = serde_json::from_value(corpo).map_err(|_| recusa())?;
                Ok(MensagemIngresso::Ingressar { pedido: c.pedido, prova: c.prova })
            }
            "aprovado" => {
                let c: CorpoAprovado = serde_json::from_value(corpo).map_err(|_| recusa())?;
                let raiz = de_hex::<32>(&c.raiz).ok_or_else(recusa)?;
                Ok(MensagemIngresso::Aprovado {
                    prova_servidor: c.prova_servidor,
                    raiz,
                    loja_id: c.loja_id,
                    linhas: c.linhas,
                })
            }
            "recusado" => {
                let c: CorpoRecusado = serde_json::from_value(corpo).map_err(|_| recusa())?;
                Ok(MensagemIngresso::Recusado { motivo: c.motivo })
            }
            _ => Err(recusa()),
        }
    }
}

/// Texto para a tela quando a verificação de segurança recusa a conexão (U1). Vale
/// para os dois lados (quem recusou e quem foi recusado), sem acusar ninguém (N-11).
pub const TEXTO_RECUSA: &str = "A conexão com o outro computador foi recusada pela verificação de segurança: \
     os dois precisam ser caixas aprovados desta loja.";
/// Texto para a tela quando chega algo que não é uma mensagem de entrada na loja.
pub const TEXTO_MENSAGEM_INVALIDA: &str = "O outro computador mandou uma mensagem que não é de entrada na loja.";
/// Texto para a tela quando a conexão segura não termina (prazo, queda) (U1).
pub const TEXTO_CONEXAO_NAO_TERMINOU: &str =
    "A conexão segura com o outro caixa não terminou. Confira a rede e tente de novo.";

/// F-12: o motivo técnico (em inglês, do `rustls`) vai só para o registro técnico;
/// a tela recebe um texto simples.
fn erro_tls(e: TlsError) -> Erro {
    crate::registro::anotar(&format!("troca de dados (protocolo 4): conexão TLS recusada: {e}"));
    Erro::validacao(TEXTO_RECUSA)
}

/// N-11: o texto do sistema vai só para o registro técnico; a tela recebe um texto simples.
fn erro_rede(e: std::io::Error) -> Erro {
    crate::registro::anotar(&format!("troca de dados (protocolo 4): falha na rede: {e}"));
    Erro::validacao(TEXTO_CONEXAO_NAO_TERMINOU)
}

fn checar_parada_e_prazo(fim: Instant, parar: Option<&AtomicBool>) -> Resultado<()> {
    if parar.is_some_and(|p| p.load(Ordering::SeqCst)) {
        return Err(Erro::validacao("A troca de dados parou (o Rustinho está fechando)."));
    }
    if Instant::now() >= fim {
        return Err(erro_rede(std::io::ErrorKind::TimedOut.into()));
    }
    Ok(())
}

/// Drena todo o buffer de saída do `rustls` para o `TcpStream`, respeitando prazo e parada.
fn descarregar_tls<Data: SideData>(
    fluxo: &mut TcpStream,
    conn: &mut ConnectionCommon<Data>,
    fim: Instant,
    parar: Option<&AtomicBool>,
) -> Resultado<()> {
    while conn.wants_write() {
        checar_parada_e_prazo(fim, parar)?;
        match conn.write_tls(fluxo) {
            Ok(0) => return Err(erro_rede(std::io::ErrorKind::WriteZero.into())),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(erro_rede(e)),
        }
    }
    let _ = fluxo.flush();
    Ok(())
}

/// Lê um pacote TLS do socket e processa na máquina de estados `rustls`.
fn ler_um_passo_tls<Data: SideData>(
    fluxo: &mut TcpStream,
    conn: &mut ConnectionCommon<Data>,
    fim: Instant,
    parar: Option<&AtomicBool>,
) -> Resultado<()> {
    loop {
        checar_parada_e_prazo(fim, parar)?;
        match conn.read_tls(fluxo) {
            Ok(0) => return Err(erro_rede(std::io::ErrorKind::UnexpectedEof.into())),
            Ok(_) => {
                if let Err(e) = conn.process_new_packets() {
                    // Tenta enviar o alerta TLS de recusa antes de encerrar.
                    let _ = conn.write_tls(fluxo);
                    return Err(erro_tls(e));
                }
                return Ok(());
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(erro_rede(e)),
        }
    }
}

fn finalizar_abertura<C, Data>(
    mut fluxo: TcpStream,
    mut conn: C,
    prazo: Duration,
    parar: Option<&AtomicBool>,
) -> Resultado<CanalTls<C>>
where
    Data: SideData,
    C: std::ops::DerefMut<Target = ConnectionCommon<Data>>,
{
    let fim = Instant::now() + prazo;
    fluxo.set_nonblocking(false).ok();
    let passo_leitura = prazo.min(Duration::from_millis(200)).max(Duration::from_millis(1));
    let passo_escrita = prazo.min(Duration::from_millis(500)).max(Duration::from_millis(1));
    fluxo.set_read_timeout(Some(passo_leitura)).ok();
    fluxo.set_write_timeout(Some(passo_escrita)).ok();

    while conn.is_handshaking() {
        checar_parada_e_prazo(fim, parar)?;
        let escreveu = conn.wants_write();
        if escreveu {
            descarregar_tls(&mut fluxo, &mut conn, fim, parar)?;
        }
        if !conn.is_handshaking() {
            break;
        }
        if conn.wants_read() {
            ler_um_passo_tls(&mut fluxo, &mut conn, fim, parar)?;
        } else if !escreveu {
            crate::registro::anotar(
                "troca de dados (protocolo 4): aperto de mão TLS interrompido em estado inconsistente",
            );
            return Err(Erro::validacao(TEXTO_CONEXAO_NAO_TERMINOU));
        }
    }
    // Envia o último voo do aperto de mão (ex.: Certificate + CertificateVerify + Finished do cliente).
    descarregar_tls(&mut fluxo, &mut conn, fim, parar)?;

    if conn.protocol_version() != Some(ProtocolVersion::TLSv1_3) {
        crate::registro::anotar("troca de dados (protocolo 4): o outro lado não negociou TLS 1.3");
        return Err(Erro::validacao(TEXTO_RECUSA));
    }
    let chave_par = conn
        .peer_certificates()
        .and_then(|lista| lista.first())
        .and_then(extrair_chave_spki)
        .ok_or_else(|| Erro::validacao(TEXTO_RECUSA))?;

    let mut exportador_convite = [0u8; 32];
    conn.export_keying_material(&mut exportador_convite, ROTULO_EXPORTADOR_CONVITE, Some(CONTEXTO_EXPORTADOR_CONVITE))
        .map_err(|e| Erro::Interno(format!("falha ao exportar material da sessão TLS 1.3: {e}")))?;

    Ok(CanalTls { fluxo, conn, chave_par, exportador_convite })
}

impl<C, Data> CanalTls<C>
where
    Data: SideData,
    C: std::ops::DerefMut<Target = ConnectionCommon<Data>>,
{
    /// Envia um quadro JSON enquadrado (`u32` big-endian + corpo UTF-8) em um único envio pelo canal TLS 1.3.
    pub fn enviar_json(&mut self, valor: &Value, prazo: Duration, parar: Option<&AtomicBool>) -> Resultado<()> {
        let bytes = serde_json::to_vec(valor)?;
        let tam = u32::try_from(bytes.len()).map_err(|_| Erro::validacao("Mensagem grande demais."))?;
        let mut quadro = Vec::with_capacity(4 + bytes.len());
        quadro.extend_from_slice(&tam.to_be_bytes());
        quadro.extend_from_slice(&bytes);
        let fim = Instant::now() + prazo;
        self.escrever_tudo(&quadro, fim, parar)
    }

    /// Recebe um quadro JSON enquadrado respeitando estritamente `limite` bytes (rejeita antes de alocar).
    pub fn receber_json(&mut self, limite: usize, prazo: Duration, parar: Option<&AtomicBool>) -> Resultado<Value> {
        let fim = Instant::now() + prazo;
        let mut cab = [0u8; 4];
        self.ler_exato(&mut cab, fim, parar)?;
        let tam = u32::from_be_bytes(cab) as usize;
        if tam == 0 {
            return Err(Erro::validacao("Mensagem vazia recebida pela troca de dados."));
        }
        if tam > limite {
            return Err(Erro::validacao("Mensagem grande demais."));
        }
        let mut corpo = vec![0u8; tam];
        self.ler_exato(&mut corpo, fim, parar)?;
        Ok(serde_json::from_slice(&corpo)?)
    }

    /// Fecha graciosamente o canal TLS enviando `close_notify`.
    pub fn encerrar(mut self) {
        self.conn.send_close_notify();
        let fim = Instant::now() + Duration::from_millis(500);
        let _ = descarregar_tls(&mut self.fluxo, &mut self.conn, fim, None);
    }

    fn escrever_tudo(&mut self, mut dados: &[u8], fim: Instant, parar: Option<&AtomicBool>) -> Resultado<()> {
        while !dados.is_empty() {
            checar_parada_e_prazo(fim, parar)?;
            let n = self.conn.writer().write(dados).map_err(erro_rede)?;
            dados = &dados[n..];
            descarregar_tls(&mut self.fluxo, &mut self.conn, fim, parar)?;
        }
        Ok(())
    }

    fn ler_exato(&mut self, buf: &mut [u8], fim: Instant, parar: Option<&AtomicBool>) -> Resultado<()> {
        let mut lido = 0;
        while lido < buf.len() {
            checar_parada_e_prazo(fim, parar)?;
            match self.conn.reader().read(&mut buf[lido..]) {
                Ok(0) => return Err(erro_rede(std::io::ErrorKind::UnexpectedEof.into())),
                Ok(n) => lido += n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    ler_um_passo_tls(&mut self.fluxo, &mut self.conn, fim, parar)?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(erro_rede(e)),
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Prova de convite com Channel Binding (HMAC-SHA256 sobre `export_keying_material`, A6).
// ---------------------------------------------------------------------------

/// Provas enviadas pelo caixa novo no pedido `"ingressar"` sobre TLS 1.3.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProvaIngressoCliente {
    /// `HMAC-SHA256(segredo_norm, "cliente\0" || exporter || chave_cli || chave_srv || pedido_canonico)`:
    /// obrigatória no primeiro consumo do convite (não pode ser forjada a partir de `segredo_hash`).
    pub prova_segredo: String,
    /// `HMAC-SHA256(segredo_hash_bytes, "retomada\0" || exporter || chave_cli || chave_srv || pedido_canonico)`:
    /// usada somente na retomada idempotente em caso de queda logo após a aprovação (quando `segredo_ativo`
    /// já foi apagado no banco e o cliente já está autenticado no TLS 1.3 como `chave_aprovada`).
    pub prova_retomada: String,
}

fn corpo_canonico_pedido_prova(
    rotulo: &[u8],
    exporter: &[u8; 32],
    chave_cliente: &Chave,
    chave_servidor: &Chave,
    pedido: &Pedido,
) -> Vec<u8> {
    let mut v = Vec::with_capacity(rotulo.len() + 32 + 32 + 32 + 64 + 64 + pedido.dispositivo.len() + 16);
    v.extend_from_slice(rotulo);
    v.extend_from_slice(exporter);
    v.extend_from_slice(chave_cliente);
    v.extend_from_slice(chave_servidor);
    v.extend_from_slice(&pedido.chave);
    v.extend_from_slice(&pedido.chave_troca);
    v.extend_from_slice(&(pedido.dispositivo.len() as u32).to_be_bytes());
    v.extend_from_slice(pedido.dispositivo.as_bytes());
    v.extend_from_slice(&pedido.assinatura);
    v
}

fn corpo_canonico_servidor_prova(
    exporter: &[u8; 32],
    chave_cliente: &Chave,
    chave_servidor: &Chave,
    raiz: &Chave,
    loja_id: &str,
) -> Vec<u8> {
    let mut v = Vec::with_capacity(16 + 32 * 4 + 4 + loja_id.len());
    v.extend_from_slice(b"servidor\0");
    v.extend_from_slice(exporter);
    v.extend_from_slice(chave_cliente);
    v.extend_from_slice(chave_servidor);
    v.extend_from_slice(raiz);
    v.extend_from_slice(&(loja_id.len() as u32).to_be_bytes());
    v.extend_from_slice(loja_id.as_bytes());
    v
}

const ALFABETO_BASE32_CONVITE: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

fn segredo_convite_valido(norm: &str) -> bool {
    norm.len() == 26 && norm.is_ascii() && norm.bytes().all(|b| ALFABETO_BASE32_CONVITE.contains(&b))
}

/// Gera as provas HMAC-SHA256 do cliente para o convite amarradas à sessão TLS 1.3.
pub fn gerar_prova_cliente(
    segredo_convite: &str,
    exporter: &[u8; 32],
    chave_cliente: &Chave,
    chave_servidor: &Chave,
    pedido: &Pedido,
) -> Resultado<ProvaIngressoCliente> {
    let norm = Segredo(cofre::normalizar_segredo(segredo_convite).into_bytes());
    let norm_str = std::str::from_utf8(&norm.0).unwrap_or_default();
    if !segredo_convite_valido(norm_str) {
        return Err(Erro::campo(
            "convite",
            "O convite precisa ter 26 caracteres válidos (ex.: XXXXX-XXXXX-XXXXX-XXXXX-XXXXX-X).",
        ));
    }
    let hash_hex = cofre::hash_segredo(norm_str);
    let mut hash_bytes = de_hex::<32>(&hash_hex).ok_or_else(|| Erro::Interno("hash de convite inválido".into()))?;

    let corpo_seg = corpo_canonico_pedido_prova(b"cliente\0", exporter, chave_cliente, chave_servidor, pedido);
    let tag_seg = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, &norm.0), &corpo_seg);

    let corpo_ret = corpo_canonico_pedido_prova(b"retomada\0", exporter, chave_cliente, chave_servidor, pedido);
    let tag_ret = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, &hash_bytes), &corpo_ret);
    apagar(&mut hash_bytes);

    Ok(ProvaIngressoCliente { prova_segredo: hex(tag_seg.as_ref()), prova_retomada: hex(tag_ret.as_ref()) })
}

/// Confere em tempo constante a prova de primeiro consumo (`segredo_norm` ainda ativo).
pub fn verificar_prova_primeiro_consumo(
    segredo_norm: &str,
    exporter: &[u8; 32],
    chave_cliente: &Chave,
    chave_servidor: &Chave,
    pedido: &Pedido,
    prova: &ProvaIngressoCliente,
) -> bool {
    if pedido.chave != *chave_cliente
        || !pedido.confere()
        || aprovados::validar_dispositivo(&pedido.dispositivo).is_err()
    {
        return false;
    }
    let Some(tag) = de_hex::<32>(&prova.prova_segredo) else {
        return false;
    };
    let corpo = corpo_canonico_pedido_prova(b"cliente\0", exporter, chave_cliente, chave_servidor, pedido);
    hmac::verify(&hmac::Key::new(hmac::HMAC_SHA256, segredo_norm.as_bytes()), &corpo, &tag).is_ok()
}

/// Confere em tempo constante a prova de retomada idempotente (somente para chave já
/// autenticada no TLS 1.3 e registrada como `chave_aprovada` no convite dentro da validade).
pub fn verificar_prova_retomada(
    segredo_hash_hex: &str,
    exporter: &[u8; 32],
    chave_cliente: &Chave,
    chave_servidor: &Chave,
    pedido: &Pedido,
    prova: &ProvaIngressoCliente,
) -> bool {
    if pedido.chave != *chave_cliente
        || !pedido.confere()
        || aprovados::validar_dispositivo(&pedido.dispositivo).is_err()
    {
        return false;
    }
    let (Some(chave_hash), Some(tag)) = (de_hex::<32>(segredo_hash_hex), de_hex::<32>(&prova.prova_retomada)) else {
        return false;
    };
    let corpo = corpo_canonico_pedido_prova(b"retomada\0", exporter, chave_cliente, chave_servidor, pedido);
    hmac::verify(&hmac::Key::new(hmac::HMAC_SHA256, &chave_hash), &corpo, &tag).is_ok()
}

/// Gera a prova HMAC-SHA256 do servidor (autenticação mútua do convite: o cliente só
/// aceita `raiz` e `loja_id` se o servidor provar que conhece o convite desta sessão TLS).
pub fn gerar_prova_servidor(
    segredo_hash_hex: &str,
    exporter: &[u8; 32],
    chave_cliente: &Chave,
    chave_servidor: &Chave,
    raiz: &Chave,
    loja_id: &str,
) -> Resultado<String> {
    let chave_hash =
        de_hex::<32>(segredo_hash_hex).ok_or_else(|| Erro::Interno("hash de convite inválido no servidor".into()))?;
    let corpo = corpo_canonico_servidor_prova(exporter, chave_cliente, chave_servidor, raiz, loja_id);
    let tag = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, &chave_hash), &corpo);
    Ok(hex(tag.as_ref()))
}

/// Confere em tempo constante a prova HMAC-SHA256 devolvida pelo servidor no ingresso.
pub fn verificar_prova_servidor(
    segredo_convite: &str,
    exporter: &[u8; 32],
    chave_cliente: &Chave,
    chave_servidor: &Chave,
    raiz: &Chave,
    loja_id: &str,
    prova_servidor_hex: &str,
) -> bool {
    let norm = Segredo(cofre::normalizar_segredo(segredo_convite).into_bytes());
    let norm_str = std::str::from_utf8(&norm.0).unwrap_or_default();
    if !segredo_convite_valido(norm_str) {
        return false;
    }
    let hash_hex = cofre::hash_segredo(norm_str);
    let (Some(mut chave_hash), Some(tag)) = (de_hex::<32>(&hash_hex), de_hex::<32>(prova_servidor_hex)) else {
        return false;
    };
    let corpo = corpo_canonico_servidor_prova(exporter, chave_cliente, chave_servidor, raiz, loja_id);
    let ok = hmac::verify(&hmac::Key::new(hmac::HMAC_SHA256, &chave_hash), &corpo, &tag).is_ok();
    apagar(&mut chave_hash);
    ok
}

/// Localiza o convite correspondente à prova apresentada no canal TLS 1.3 (primeiro
/// consumo via `segredo_normalizado` ou retomada idempotente via `segredo_hash` +
/// chave já aprovada).
pub fn casar_convite_da_prova<'a>(
    candidatos: &'a [cofre::ConviteCandidato],
    exporter: &[u8; 32],
    chave_cliente: &Chave,
    chave_servidor: &Chave,
    pedido: &Pedido,
    prova: &ProvaIngressoCliente,
) -> Option<&'a cofre::ConviteCandidato> {
    let chave_hex = hex(chave_cliente);
    for c in candidatos {
        if c.codigo_pedido != pedido.codigo() {
            continue;
        }
        if c.usado_em.is_none() {
            if let Some(seg) = &c.segredo_normalizado {
                if verificar_prova_primeiro_consumo(seg, exporter, chave_cliente, chave_servidor, pedido, prova) {
                    return Some(c);
                }
            }
        } else if c.chave_aprovada.as_deref() == Some(chave_hex.as_str())
            && verificar_prova_retomada(&c.segredo_hash, exporter, chave_cliente, chave_servidor, pedido, prova)
        {
            return Some(c);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// QR code do convite (A6): geração SVG via `rxing` (`encoders`) + leitura do payload.
// ---------------------------------------------------------------------------

/// Dados estruturados extraídos de um payload QR `RUST4|segredo|loja_id|aprovador_hex|endereco`.
/// O `Debug` não mostra o segredo (F-10).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConviteQr {
    pub segredo: String,
    pub loja_id: String,
    pub aprovador_chave: Option<Chave>,
    pub endereco: String,
}

impl std::fmt::Debug for ConviteQr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConviteQr")
            .field("segredo", &"<oculto>")
            .field("loja_id", &self.loja_id)
            .field("aprovador_chave", &self.aprovador_chave.as_ref().map(|c| hex(c)))
            .field("endereco", &self.endereco)
            .finish()
    }
}

/// Interpreta tanto um payload completo de QR (`RUST4|segredo|loja|aprovador|endereco`)
/// quanto um segredo digitado à mão (`XXXXX-XXXXX-XXXXX-XXXXX-XXXXX-X`).
pub fn interpretar_convite_entrada(texto: &str) -> Resultado<ConviteQr> {
    let limpo = texto.trim();
    if limpo.is_empty() {
        return Err(Erro::campo("convite", "Digite o convite de 26 caracteres ou leia o QR do outro caixa."));
    }
    if limpo.len() > LIMITE_QR_BYTES {
        return Err(Erro::campo("convite", "Convite grande demais."));
    }
    if let Some(resto) = limpo.strip_prefix("RUST4|") {
        let partes: Vec<&str> = resto.split('|').collect();
        if partes.len() < 3 || partes.len() > 4 {
            return Err(Erro::campo("convite", "QR de convite inválido."));
        }
        let norm = cofre::normalizar_segredo(partes[0]);
        if !segredo_convite_valido(&norm) {
            return Err(Erro::campo("convite", "Convite dentro do QR tem formato inválido."));
        }
        let loja_id = partes[1].trim();
        if loja_id.is_empty() {
            return Err(Erro::campo("convite", "QR de convite sem identidade da loja."));
        }
        let aprovador_chave = if partes[2].trim().is_empty() {
            None
        } else {
            Some(
                de_hex::<32>(partes[2].trim())
                    .ok_or_else(|| Erro::campo("convite", "Chave do aprovador inválida no QR."))?,
            )
        };
        return Ok(ConviteQr {
            segredo: partes[0].trim().to_string(),
            loja_id: loja_id.to_string(),
            aprovador_chave,
            endereco: partes.get(3).map(|s| s.trim().to_string()).unwrap_or_default(),
        });
    }
    let norm = cofre::normalizar_segredo(limpo);
    if !segredo_convite_valido(&norm) {
        return Err(Erro::campo(
            "convite",
            "O convite precisa ter 26 caracteres válidos (ou leia o QR gerado pelo gerente no outro caixa).",
        ));
    }
    Ok(ConviteQr { segredo: limpo.to_string(), loja_id: String::new(), aprovador_chave: None, endereco: String::new() })
}

/// Gera uma data URI SVG (`data:image/svg+xml;base64,...`) com o QR code de `texto`
/// usando `rxing` (`encoders`), sem dependência externa adicional (A6).
/// O `rxing::QRCodeWriter` já inclui a zona silenciosa padrão de 4 módulos.
pub fn qr_svg_data_uri(texto: &str) -> Resultado<String> {
    use rxing::{BarcodeFormat, Writer};
    if texto.is_empty() || texto.len() > LIMITE_QR_BYTES {
        return Err(Erro::validacao("Texto do QR code fora do tamanho permitido."));
    }
    let matriz = rxing::MultiFormatWriter
        .encode(texto, &BarcodeFormat::QR_CODE, 0, 0)
        .map_err(|e| Erro::Interno(format!("não foi possível gerar o QR code do convite: {e}")))?;
    let w = matriz.getWidth();
    let h = matriz.getHeight();

    let mut path = String::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            if matriz.get(x, y) {
                use std::fmt::Write as _;
                let _ = write!(path, "M{x},{y}h1v1h-1z");
            }
        }
    }
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" shape-rendering=\"crispEdges\">\
         <rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>\
         <path fill=\"#000000\" d=\"{path}\"/></svg>"
    );
    let b64 = base64::engine::general_purpose::STANDARD.encode(svg.as_bytes());
    Ok(format!("data:image/svg+xml;base64,{b64}"))
}

#[cfg(test)]
mod testes {
    use std::net::TcpListener;
    use std::thread;

    use serde_json::json;

    use super::*;

    const LOJA: &str = "loja-teste-canal";

    fn par_tcp() -> (TcpStream, TcpStream) {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = ouvinte.local_addr().unwrap();
        let h = thread::spawn(move || TcpStream::connect(addr).unwrap());
        let (srv, _) = ouvinte.accept().unwrap();
        let cli = h.join().unwrap();
        (cli, srv)
    }

    fn conjunto(chaves: &[Chave]) -> HashSet<Chave> {
        chaves.iter().copied().collect()
    }

    const PRAZO: Duration = Duration::from_secs(5);

    // 1. Dois caixas autorizados: aperto de mão mútuo TLS 1.3 RPK, dados nos dois
    // sentidos, chave do par provada e `export_keying_material` idêntico nos dois lados.
    #[test]
    fn canal_mutuo_autorizado_troca_mensagens_e_exporta_mesmo_material() {
        let srv_id = Identidade::gerar().unwrap();
        let cli_id = Identidade::gerar().unwrap();
        let (cli_tcp, srv_tcp) = par_tcp();
        let cli_pub = cli_id.publica();
        let srv_pub = srv_id.publica();

        let h_srv = thread::spawn(move || {
            let mut srv = aceitar_autorizado(srv_tcp, &srv_id, &conjunto(&[cli_pub]), PRAZO, None).unwrap();
            assert_eq!(srv.chave_do_par(), cli_pub);
            let exp_srv = *srv.exportador_convite();
            let req = srv.receber_json(64 * 1024, PRAZO, None).unwrap();
            assert_eq!(req["tipo"], "puxar");
            srv.enviar_json(&json!({ "ok": true, "eco": req["seq"] }), PRAZO, None).unwrap();
            srv.encerrar();
            exp_srv
        });

        let mut cli = conectar_autorizado(cli_tcp, &cli_id, &conjunto(&[srv_pub]), PRAZO, None).unwrap();
        assert_eq!(cli.chave_do_par(), srv_pub);
        let exp_cli = *cli.exportador_convite();
        cli.enviar_json(&json!({ "tipo": "puxar", "seq": 42 }), PRAZO, None).unwrap();
        let resp = cli.receber_json(64 * 1024, PRAZO, None).unwrap();
        assert_eq!(resp, json!({ "ok": true, "eco": 42 }));
        cli.encerrar();

        let exp_srv = h_srv.join().unwrap();
        assert_eq!(exp_cli, exp_srv, "RFC 5705 exporter deve coincidir nos dois lados");
        assert_ne!(exp_cli, [0u8; 32]);
    }

    // 2. Intruso não autorizado (cliente fora da lista) é recusado no handshake TLS 1.3
    // ANTES de enviar qualquer dado; conjunto vazio também recusa (fail-closed).
    #[test]
    fn intruso_e_conjunto_vazio_recusados_antes_de_qualquer_dado() {
        let srv_id = Identidade::gerar().unwrap();
        let aprovado = Identidade::gerar().unwrap();
        let intruso = Identidade::gerar().unwrap();
        let srv_pub = srv_id.publica();
        let aprovado_pub = aprovado.publica();

        let (cli_tcp, srv_tcp) = par_tcp();
        let h_srv =
            thread::spawn(move || aceitar_autorizado(srv_tcp, &srv_id, &conjunto(&[aprovado_pub]), PRAZO, None));
        let r_cli = conectar_autorizado(cli_tcp, &intruso, &conjunto(&[srv_pub]), PRAZO, None);
        let r_srv = h_srv.join().unwrap();
        assert!(r_srv.is_err(), "servidor deve recusar cliente fora da lista autorizada");
        // No cliente, ou falha no handshake ou no primeiro envio/leitura ao receber o alerta do servidor.
        if let Ok(mut c) = r_cli {
            let _ = c.enviar_json(&json!({ "tipo": "puxar" }), Duration::from_secs(2), None);
            assert!(c.receber_json(1024, Duration::from_secs(2), None).is_err());
        }

        // Conjunto vazio (fail-closed): recusa até mesmo o caixa antes aprovado.
        let srv_id2 = Identidade::gerar().unwrap();
        let srv_pub2 = srv_id2.publica();
        let (cli_tcp2, srv_tcp2) = par_tcp();
        let h_srv2 = thread::spawn(move || aceitar_autorizado(srv_tcp2, &srv_id2, &HashSet::new(), PRAZO, None));
        let _ = conectar_autorizado(cli_tcp2, &aprovado, &conjunto(&[srv_pub2]), PRAZO, None);
        assert!(h_srv2.join().unwrap().is_err(), "conjunto autorizado vazio deve falhar fechado");
    }

    /// 3. Servidor falso (MITM / chave fora da lista do cliente) é recusado pelo cliente.
    #[test]
    fn servidor_falso_e_recusado_pelo_cliente() {
        let srv_legitimo = Identidade::gerar().unwrap();
        let srv_falso = Identidade::gerar().unwrap();
        let cli_id = Identidade::gerar().unwrap();
        let cli_pub = cli_id.publica();
        let srv_leg_pub = srv_legitimo.publica();

        let (cli_tcp, srv_tcp) = par_tcp();
        let h_srv = thread::spawn(move || aceitar_autorizado(srv_tcp, &srv_falso, &conjunto(&[cli_pub]), PRAZO, None));
        let r_cli = conectar_autorizado(cli_tcp, &cli_id, &conjunto(&[srv_leg_pub]), PRAZO, None);
        assert!(r_cli.is_err(), "cliente deve recusar servidor com chave não esperada");
        assert!(h_srv.join().unwrap().is_err());
        // E o caixa novo que conhece a chave do aprovador pelo QR também recusa o falso.
        let (cli_tcp2, srv_tcp2) = par_tcp();
        let falso2 = Identidade::gerar().unwrap();
        let h_srv2 = thread::spawn(move || aceitar_com_ingresso(srv_tcp2, &falso2, &HashSet::new(), PRAZO, None));
        assert!(conectar_para_ingresso(cli_tcp2, &cli_id, Some(srv_leg_pub), PRAZO, None).is_err());
        let _ = h_srv2.join().unwrap();
    }

    // 4. Impostor apresenta a chave pública de um caixa aprovado mas assina o TLS
    // com outra chave privada: recusado pela verificação de assinatura Ed25519 (BadSignature).
    #[test]
    fn chave_publica_sem_privada_e_recusada_no_handshake() {
        let srv_id = Identidade::gerar().unwrap();
        let vitima = Identidade::gerar().unwrap();
        let impostor = Identidade::gerar().unwrap();
        let vitima_pub = vitima.publica();
        let srv_pub = srv_id.publica();

        // Monta um CertifiedKey adulterado: SPKI da vítima + assinador privado do impostor!
        let assinador_impostor =
            provedor::sign::any_eddsa_type(&PrivatePkcs8KeyDer::from(impostor.pkcs8_der().to_vec())).unwrap();
        let cert_forjado =
            Arc::new(CertifiedKey::new(vec![CertificateDer::from(montar_spki(&vitima_pub))], assinador_impostor));
        let cfg_forjado = Arc::new(
            ClientConfig::builder_with_provider(Arc::new(provedor::default_provider()))
                .with_protocol_versions(&[&rustls::version::TLS13])
                .unwrap()
                .dangerous()
                .with_custom_certificate_verifier(VerificadorRpk::novo(PoliticaChaves::SomenteAutorizadas(conjunto(
                    &[srv_pub],
                ))))
                .with_client_cert_resolver(Arc::new(rustls::client::AlwaysResolvesClientRawPublicKeys::new(
                    cert_forjado,
                ))),
        );

        let (cli_tcp, srv_tcp) = par_tcp();
        let h_srv = thread::spawn(move || aceitar_autorizado(srv_tcp, &srv_id, &conjunto(&[vitima_pub]), PRAZO, None));
        let conn_forjada = ClientConnection::new(cfg_forjado, ServerName::try_from("rustinho-caixa").unwrap()).unwrap();
        let _ = finalizar_abertura(cli_tcp, conn_forjada, PRAZO, None);
        assert!(h_srv.join().unwrap().is_err(), "chave pública sem a privada correspondente deve falhar no TLS 1.3");
    }

    // 5. Fluxo completo de convite com *channel binding* mútuo (A6) sobre canais TLS 1.3 reais:
    // - primeiro ingresso com convite válido aprova (com a prova QUE PASSOU PELO FIO, F-08)
    //   e devolve a prova mútua do servidor;
    // - MITM (exporter diferente), intruso, convite expirado e servidor falso são recusados;
    // - queda após aprovação e retomada idempotente em 2º canal TLS 1.3 (agora autorizado)
    //   funcionam sem duplicar, com o convite digitado (sem a chave do aprovador).
    #[test]
    fn convite_channel_binding_mutuo_mitm_e_retomada_idempotente() {
        let (b, gerente) = crate::faixas::para_testes::loja("Caixa 1");
        let raiz = Arc::new(Identidade::gerar().unwrap());
        let novo = Identidade::gerar().unwrap();
        let intruso = Identidade::gerar().unwrap();
        let raiz_pub = raiz.publica();
        let novo_pub = novo.publica();
        cofre::garantir_raiz_se_aplicavel(&b.conn, &raiz, false, LOJA, "Caixa 1", &gerente, true).unwrap();

        let pedido_novo = Pedido::com_dispositivo(&novo, "disp-caixa-2");
        let conv = cofre::gerar_convite(
            &b.conn,
            &raiz,
            &raiz_pub,
            LOJA,
            &pedido_novo.codigo(),
            "Caixa 2",
            &gerente,
            "127.0.0.1:47810",
        )
        .unwrap();
        let qr = interpretar_convite_entrada(&conv.payload_qr).unwrap();
        assert_eq!(qr.aprovador_chave, Some(raiz_pub));
        assert_eq!(qr.loja_id, LOJA);
        let autorizadas_antes = aprovados::avaliar(&b.conn, &raiz_pub, LOJA).unwrap().autorizadas().clone();

        // --- 1º canal TLS 1.3 real: ingresso completo, com a chave do aprovador vinda do QR ---
        let (cli_tcp, srv_tcp) = par_tcp();
        let raiz_srv = raiz.clone();
        let h_srv = thread::spawn(move || {
            let Aceito::Ingresso(mut srv) =
                aceitar_com_ingresso(srv_tcp, &raiz_srv, &autorizadas_antes, PRAZO, None).unwrap()
            else {
                panic!("chave desconhecida tem de cair no canal de ingresso");
            };
            let MensagemIngresso::Ingressar { pedido, prova } = srv.receber(64 * 1024, PRAZO, None).unwrap() else {
                panic!("esperava o pedido de ingresso");
            };
            let exp = *srv.exportador_convite();
            let cli_k = srv.chave_do_par();
            (srv, pedido, prova, exp, cli_k)
        });

        let mut cli = conectar_para_ingresso(cli_tcp, &novo, qr.aprovador_chave, PRAZO, None).unwrap();
        let prova1 =
            gerar_prova_cliente(&qr.segredo, cli.exportador_convite(), &novo_pub, &raiz_pub, &pedido_novo).unwrap();
        cli.enviar(&MensagemIngresso::Ingressar { pedido: pedido_novo.clone(), prova: prova1.clone() }, PRAZO, None)
            .unwrap();

        let (mut srv, ped_rx, prova_rx, exp_srv, cli_k) = h_srv.join().unwrap();
        assert_eq!(cli_k, novo_pub);
        assert_eq!(exp_srv, *cli.exportador_convite());
        assert_eq!((&ped_rx, &prova_rx), (&pedido_novo, &prova1), "o que passou pelo fio é o que foi mandado");

        let agora = crate::tempo::agora();
        let candidatos = cofre::convites_validos_agora(&b.conn, LOJA, &agora).unwrap();

        // MITM (exporter de outro túnel TLS) é recusado.
        let mut exporter_mitm = exp_srv;
        exporter_mitm[0] ^= 0x80;
        assert!(casar_convite_da_prova(&candidatos, &exporter_mitm, &novo_pub, &raiz_pub, &ped_rx, &prova_rx).is_none());
        // Intruso com chave TLS diferente é recusado.
        assert!(
            casar_convite_da_prova(&candidatos, &exp_srv, &intruso.publica(), &raiz_pub, &ped_rx, &prova_rx).is_none()
        );
        // Convite expirado no futuro não entra nos candidatos válidos (e a consulta não apaga nada, F-08).
        let futuro = crate::tempo::somar_segundos(&agora, cofre::VALIDADE_CONVITE_SEG + 10).unwrap();
        assert!(cofre::convites_validos_agora(&b.conn, LOJA, &futuro).unwrap().is_empty(), "vencido não casa");
        // F-08: a prova que passou pelo fio casa com o convite certo.
        let candidatos = cofre::convites_validos_agora(&b.conn, LOJA, &agora).unwrap();
        let casado = casar_convite_da_prova(&candidatos, &exp_srv, &novo_pub, &raiz_pub, &ped_rx, &prova_rx)
            .expect("a prova legítima que veio pelo canal deve casar");
        assert_eq!(casado.id, conv.id);
        let apr1 =
            cofre::consumir_convite_e_aprovar(&b.conn, &raiz, &raiz_pub, LOJA, &casado.id, &ped_rx, &agora).unwrap();
        let p_srv =
            gerar_prova_servidor(&casado.segredo_hash, &exp_srv, &novo_pub, &raiz_pub, &raiz_pub, LOJA).unwrap();
        let linhas = aprovados::linhas(&b.conn, LOJA).unwrap();
        srv.enviar(
            &MensagemIngresso::Aprovado { prova_servidor: p_srv, raiz: raiz_pub, loja_id: LOJA.into(), linhas },
            PRAZO,
            None,
        )
        .unwrap();
        srv.encerrar();

        let MensagemIngresso::Aprovado { prova_servidor, raiz: raiz_rx, loja_id, linhas: linhas_rx } =
            cli.receber(64 * 1024, PRAZO, None).unwrap()
        else {
            panic!("esperava a aprovação");
        };
        assert_eq!((raiz_rx, loja_id.as_str()), (raiz_pub, LOJA));
        assert!(linhas_rx.aprovacoes.iter().any(|a| a.chave == novo_pub), "o registro com a aprovação veio junto");
        let confere = |raiz_alegada: &Chave, loja: &str| {
            verificar_prova_servidor(
                &qr.segredo,
                cli.exportador_convite(),
                &novo_pub,
                &raiz_pub,
                raiz_alegada,
                loja,
                &prova_servidor,
            )
        };
        assert!(confere(&raiz_pub, LOJA));
        assert!(!confere(&intruso.publica(), LOJA), "raiz trocada no caminho não confere");
        assert!(!confere(&raiz_pub, "outra-loja"), "loja trocada no caminho não confere");
        cli.encerrar();

        // --- 2º canal: queda após aprovação e retomada idempotente (convite digitado, sem QR) ---
        let autorizadas_depois = aprovados::avaliar(&b.conn, &raiz_pub, LOJA).unwrap().autorizadas().clone();
        assert!(autorizadas_depois.contains(&novo_pub));
        let (cli_tcp2, srv_tcp2) = par_tcp();
        let raiz_srv2 = raiz.clone();
        let h_srv2 = thread::spawn(move || {
            let Aceito::Autenticado(mut srv2) =
                aceitar_com_ingresso(srv_tcp2, &raiz_srv2, &autorizadas_depois, PRAZO, None).unwrap()
            else {
                panic!("o caixa já aprovado entra como autenticado");
            };
            let MensagemIngresso::Ingressar { pedido, prova } =
                MensagemIngresso::de_json(srv2.receber_json(64 * 1024, PRAZO, None).unwrap()).unwrap()
            else {
                panic!("esperava a retomada do ingresso");
            };
            let exp2 = *srv2.exportador_convite();
            (srv2, pedido, prova, exp2)
        });

        let mut cli2 = conectar_para_ingresso(cli_tcp2, &novo, None, PRAZO, None).unwrap();
        assert_ne!(cli2.exportador_convite(), &exp_srv, "nova sessão TLS gera exporter novo");
        let p_retry =
            gerar_prova_cliente(&conv.segredo, cli2.exportador_convite(), &novo_pub, &raiz_pub, &pedido_novo).unwrap();
        cli2.enviar(&MensagemIngresso::Ingressar { pedido: pedido_novo.clone(), prova: p_retry }, PRAZO, None).unwrap();

        let (mut srv2, ped_rx2, prova_rx2, exp2) = h_srv2.join().unwrap();
        let cands2 = cofre::convites_validos_agora(&b.conn, LOJA, &agora).unwrap();
        let c_retry = casar_convite_da_prova(&cands2, &exp2, &novo_pub, &raiz_pub, &ped_rx2, &prova_rx2)
            .expect("mesmo caixa retomando após queda casa via prova_retomada");
        assert!(c_retry.segredo_normalizado.is_none(), "segredo em claro já apagado");
        let apr2 =
            cofre::consumir_convite_e_aprovar(&b.conn, &raiz, &raiz_pub, LOJA, &c_retry.id, &ped_rx2, &agora).unwrap();
        assert_eq!(apr1, apr2, "retomada é idempotente e não duplica aprovação");
        let p_srv2 = gerar_prova_servidor(&c_retry.segredo_hash, &exp2, &novo_pub, &raiz_pub, &raiz_pub, LOJA).unwrap();
        let linhas2 = aprovados::linhas(&b.conn, LOJA).unwrap();
        let resposta2 = MensagemIngresso::Aprovado {
            prova_servidor: p_srv2,
            raiz: raiz_pub,
            loja_id: LOJA.into(),
            linhas: linhas2,
        };
        srv2.enviar_json(&resposta2.para_json().unwrap(), PRAZO, None).unwrap();
        srv2.encerrar();

        let MensagemIngresso::Aprovado { prova_servidor: prova2, .. } = cli2.receber(64 * 1024, PRAZO, None).unwrap()
        else {
            panic!("esperava a aprovação da retomada");
        };
        assert!(verificar_prova_servidor(
            &conv.segredo,
            cli2.exportador_convite(),
            &novo_pub,
            &raiz_pub,
            &raiz_pub,
            LOJA,
            &prova2
        ));
        cli2.encerrar();

        // Outro caixa (intruso) tentando reusar o convite já consumido é recusado!
        let ped_int = Pedido::com_dispositivo(&intruso, "disp-int");
        let p_int = gerar_prova_cliente(&conv.segredo, &exp2, &intruso.publica(), &raiz_pub, &ped_int).unwrap();
        assert!(casar_convite_da_prova(&cands2, &exp2, &intruso.publica(), &raiz_pub, &ped_int, &p_int).is_none());
    }

    /// F-04: no servidor com convite ativo, a chave desconhecida só ganha o canal de
    /// ingresso, que recusa qualquer coisa que não seja uma mensagem do convite (um
    /// pedido da troca, por exemplo); a chave autorizada ganha o canal de dados.
    #[test]
    fn f04_chave_desconhecida_so_ganha_o_canal_de_ingresso() {
        let srv_id = Identidade::gerar().unwrap();
        let srv_pub = srv_id.publica();
        let estranho = Identidade::gerar().unwrap();
        let aprovado = Identidade::gerar().unwrap();
        let autorizadas = conjunto(&[aprovado.publica()]);

        // Estranho manda um pedido da troca: o servidor só tem o canal de ingresso, que recusa.
        let (cli_tcp, srv_tcp) = par_tcp();
        let a = autorizadas.clone();
        let srv_id = Arc::new(srv_id);
        let s1 = srv_id.clone();
        let h_srv = thread::spawn(move || match aceitar_com_ingresso(srv_tcp, &s1, &a, PRAZO, None).unwrap() {
            Aceito::Autenticado(_) => panic!("chave desconhecida virou canal de dados"),
            Aceito::Ingresso(mut c) => c.receber(64 * 1024, PRAZO, None),
        });
        let mut cli = conectar_autorizado(cli_tcp, &estranho, &conjunto(&[srv_pub]), PRAZO, None).unwrap();
        cli.enviar_json(&json!({ "tipo": "puxar", "desde": 0 }), PRAZO, None).unwrap();
        assert!(h_srv.join().unwrap().is_err(), "o canal de ingresso não aceita um pedido da troca");

        // O caixa autorizado, na mesma situação, recebe o canal de dados.
        let (cli_tcp, srv_tcp) = par_tcp();
        let s2 = srv_id.clone();
        let h_srv =
            thread::spawn(move || match aceitar_com_ingresso(srv_tcp, &s2, &autorizadas, PRAZO, None).unwrap() {
                Aceito::Autenticado(mut c) => c.receber_json(64 * 1024, PRAZO, None).unwrap(),
                Aceito::Ingresso(_) => panic!("caixa autorizado não pode cair no ingresso"),
            });
        let mut cli = conectar_autorizado(cli_tcp, &aprovado, &conjunto(&[srv_pub]), PRAZO, None).unwrap();
        cli.enviar_json(&json!({ "tipo": "puxar", "desde": 0 }), PRAZO, None).unwrap();
        assert_eq!(h_srv.join().unwrap()["tipo"], "puxar");
    }

    /// F-04 / observação 10: a leitura das mensagens do ingresso é estrita.
    #[test]
    fn f04_mensagem_de_ingresso_lida_de_forma_estrita() {
        let id = Identidade::gerar().unwrap();
        let pedido = Pedido::com_dispositivo(&id, "disp-x");
        let prova = ProvaIngressoCliente { prova_segredo: "aa".into(), prova_retomada: "bb".into() };
        let ok = MensagemIngresso::Ingressar { pedido: pedido.clone(), prova: prova.clone() };
        let v = ok.para_json().unwrap();
        assert_eq!(MensagemIngresso::de_json(v.clone()).unwrap(), ok, "ida e volta");
        let mut sobrando = v.clone();
        sobrando["desde"] = json!(0);
        assert!(MensagemIngresso::de_json(sobrando).is_err(), "campo a mais é recusado");
        let mut prova_sobrando = v.clone();
        prova_sobrando["prova"]["extra"] = json!(1);
        assert!(MensagemIngresso::de_json(prova_sobrando).is_err(), "campo a mais na prova é recusado");
        assert!(MensagemIngresso::de_json(json!({ "tipo": "puxar" })).is_err(), "tipo desconhecido");
        assert!(MensagemIngresso::de_json(json!({ "pedido": pedido })).is_err(), "sem tipo");
        assert!(MensagemIngresso::de_json(json!([1, 2])).is_err(), "não é objeto");
        let ap = MensagemIngresso::Aprovado {
            prova_servidor: "cc".into(),
            raiz: id.publica(),
            loja_id: LOJA.into(),
            linhas: aprovados::Linhas::default(),
        };
        assert_eq!(MensagemIngresso::de_json(ap.para_json().unwrap()).unwrap(), ap);
        let mut raiz_ruim = ap.para_json().unwrap();
        raiz_ruim["raiz"] = json!("nao-e-hex");
        assert!(MensagemIngresso::de_json(raiz_ruim).is_err());
    }

    /// F-12: os textos que vão para a tela não trazem jargão nem o inglês da biblioteca.
    #[test]
    fn f12_textos_da_tela_sem_jargao() {
        let rede = erro_rede(std::io::ErrorKind::ConnectionReset.into());
        assert!(matches!(rede, Erro::Validacao { ref mensagem, .. } if mensagem == TEXTO_CONEXAO_NAO_TERMINOU));
        for t in [TEXTO_RECUSA, TEXTO_MENSAGEM_INVALIDA, TEXTO_CONEXAO_NAO_TERMINOU] {
            for proibido in ["TLS", "rustls", "handshake", "Aperto de mão", "I3", "Ed25519"] {
                assert!(!t.contains(proibido), "{t:?} contém {proibido:?}");
            }
        }
        let e = erro_tls(TlsError::General("chave do caixa não autorizada nesta loja".into()));
        assert!(matches!(e, Erro::Validacao { ref mensagem, .. } if mensagem == TEXTO_RECUSA));
    }

    /// 6. Limite de quadro (recusa antes de alocar mensagem gigante) e cancelamento cooperativo.
    #[test]
    fn limite_de_quadro_recusa_mensagem_gigante() {
        let srv_id = Identidade::gerar().unwrap();
        let cli_id = Identidade::gerar().unwrap();
        let (cli_tcp, srv_tcp) = par_tcp();
        let (cli_pub, srv_pub) = (cli_id.publica(), srv_id.publica());

        let h_srv = thread::spawn(move || {
            let mut srv = aceitar_autorizado(srv_tcp, &srv_id, &conjunto(&[cli_pub]), PRAZO, None).unwrap();
            srv.receber_json(64, PRAZO, None)
        });

        let mut cli = conectar_autorizado(cli_tcp, &cli_id, &conjunto(&[srv_pub]), PRAZO, None).unwrap();
        cli.enviar_json(&json!({ "grande": "x".repeat(200) }), PRAZO, None).unwrap();
        let r = h_srv.join().unwrap();
        assert!(r.is_err(), "quadro acima do limite tem de ser recusado");
    }

    // 7. QR code gerado pelo `rxing` (`encoders`) é SVG válido em base64 e decodifica
    // de volta para o payload exato do convite `RUST4|...`.
    #[test]
    fn qr_code_do_convite_gera_svg_e_interpreta_ida_e_volta() {
        let raiz = Identidade::gerar().unwrap();
        let payload =
            format!("RUST4|AAAAA-BBBBB-CCCCC-DDDDD-EEEEE-F|{LOJA}|{}|192.168.0.10:47810", hex(&raiz.publica()));
        let uri = qr_svg_data_uri(&payload).unwrap();
        assert!(uri.starts_with("data:image/svg+xml;base64,"));
        let raw = base64::engine::general_purpose::STANDARD
            .decode(uri.trim_start_matches("data:image/svg+xml;base64,"))
            .unwrap();
        let svg = String::from_utf8(raw).unwrap();
        assert!(svg.starts_with("<svg ") && svg.contains("<path ") && svg.ends_with("</svg>"));

        // Verifica também decodificação real do BitMatrix via rxing!
        use rxing::Writer as _;
        let m = rxing::MultiFormatWriter.encode(&payload, &rxing::BarcodeFormat::QR_CODE, 0, 0).unwrap();
        let escala = 4u32;
        let margem = 16u32;
        let w = m.getWidth() * escala + 2 * margem;
        let h = m.getHeight() * escala + 2 * margem;
        let mut px = vec![255u8; (w * h) as usize];
        for y in 0..m.getHeight() {
            for x in 0..m.getWidth() {
                if m.get(x, y) {
                    for dy in 0..escala {
                        for dx in 0..escala {
                            px[((y * escala + dy + margem) * w + (x * escala + dx + margem)) as usize] = 0;
                        }
                    }
                }
            }
        }
        let lido = crate::codigo::ler_imagem(w, h, px).expect("QR code gerado deve ser lido pelo leitor do Rustinho");
        assert_eq!(lido, payload);

        let qr = interpretar_convite_entrada(&lido).unwrap();
        assert_eq!(qr.loja_id, LOJA);
        assert_eq!(qr.aprovador_chave, Some(raiz.publica()));
        assert_eq!(qr.endereco, "192.168.0.10:47810");

        // Convite digitado à mão (26 chars sem QR).
        let manual = interpretar_convite_entrada("AAAAA-BBBBB-CCCCC-DDDDD-EEEEE-F").unwrap();
        assert_eq!(manual.aprovador_chave, None);
        assert!(interpretar_convite_entrada("curto").is_err());
        assert!(interpretar_convite_entrada("RUST4|curto|loja|ab").is_err());
        // Caracteres fora do alfabeto Base32 (I, O, 0, 1 ou não-ASCII), loja vazia em QR e entrada >512 bytes são recusados (B1).
        assert!(interpretar_convite_entrada("IIIII-OOOOO-00000-11111-AAAAA-A").is_err());
        assert!(interpretar_convite_entrada("ÁÁÁÁÁ-BBBBB-CCCCC-DDDDD-EEEEE-F").is_err());
        assert!(interpretar_convite_entrada(&format!(
            "RUST4|AAAAA-BBBBB-CCCCC-DDDDD-EEEEE-F||{}",
            hex(&raiz.publica())
        ))
        .is_err());
        assert!(interpretar_convite_entrada(&"A".repeat(LIMITE_QR_BYTES + 1)).is_err());
    }

    // 8. Cancelamento cooperativo (`parar = true`) encerra leitura bloqueada sem esperar
    // o prazo inteiro, e `Pedido`/`Linhas` serializam e desserializam em JSON com fidelidade.
    #[test]
    fn cancelamento_cooperativo_e_serde_registro_p4() {
        let srv_id = Identidade::gerar().unwrap();
        let cli_id = Identidade::gerar().unwrap();
        let (cli_tcp, srv_tcp) = par_tcp();
        let (cli_pub, srv_pub) = (cli_id.publica(), srv_id.publica());

        let parar = Arc::new(AtomicBool::new(false));
        let parar_srv = parar.clone();
        let h_srv = thread::spawn(move || {
            let mut srv = aceitar_autorizado(srv_tcp, &srv_id, &conjunto(&[cli_pub]), PRAZO, None).unwrap();
            let t0 = Instant::now();
            let res = srv.receber_json(1024, Duration::from_secs(10), Some(&parar_srv));
            (res, t0.elapsed())
        });

        let _cli = conectar_autorizado(cli_tcp, &cli_id, &conjunto(&[srv_pub]), PRAZO, None).unwrap();
        thread::sleep(Duration::from_millis(120));
        parar.store(true, Ordering::SeqCst);
        let (res, dt) = h_srv.join().unwrap();
        assert!(res.is_err(), "leitura cancelada por parar deve falhar");
        assert!(dt < Duration::from_secs(3), "cancelamento cooperativo levou tempo demais: {dt:?}");

        // Round-trip JSON de `Pedido` e `Linhas` (usado no ingresso e na replicação P4).
        let (b, gerente) = crate::faixas::para_testes::loja("Caixa 1");
        let raiz = Identidade::gerar().unwrap();
        let ped = Pedido::com_dispositivo(&cli_id, "disp-json-1");
        let ped_json = serde_json::to_value(&ped).unwrap();
        let ped_volta: Pedido = serde_json::from_value(ped_json).unwrap();
        assert_eq!(ped_volta, ped);
        assert!(ped_volta.confere());

        aprovados::registrar_raiz(&b.conn, &raiz, LOJA, "Caixa 1", &gerente).unwrap();
        aprovados::aprovar(&b.conn, &raiz, &raiz.publica(), LOJA, &ped, "Caixa 2", &gerente).unwrap();
        let linhas = aprovados::linhas(&b.conn, LOJA).unwrap();
        let linhas_json = serde_json::to_value(&linhas).unwrap();
        let linhas_volta: aprovados::Linhas = serde_json::from_value(linhas_json).unwrap();
        assert_eq!(linhas_volta, linhas);
    }
}
