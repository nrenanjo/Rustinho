//! Âncoras de confiança e ACs intermediárias (docs/PEND-09-pacote-acs.md).
//!
//! * O pacote oficial "Cadeia Completa" do ITI (`ACcompactadox.zip`, SHA-512
//!   conferido com o publicado) vai **embutido** e serve **só** como fonte de
//!   ACs intermediárias.
//! * **Âncoras** são só as raízes escolhidas, identificadas pelo **SHA-256 do
//!   certificado inteiro** (nunca pelo nome): v5 e v12 (vigentes) e v2
//!   (histórica). Ficam de fora: v3 (revogada), v8 e v9 (revogadas, nem estão
//!   no pacote), v10 (SSL) e v11 (código) por finalidade, v1 e a raiz
//!   original (SHA-1 em certificado, sem suporte), v4, v6, v7 e v13
//!   (algoritmos sem suporte nesta etapa).
//! * Uma raiz histórica só vale se a nota foi emitida e recebida **dentro da
//!   validade dela** (o `rustls-webpki` não confere a validade da âncora; o
//!   Rustinho confere).
//! * Certificado que veio dentro do XML **nunca** vira âncora nem
//!   intermediária.

use std::sync::OnceLock;

use base64::Engine;
use rustls_pki_types::{CertificateDer, TrustAnchor};
use sha2::{Digest, Sha256, Sha512};
use x509_parser::prelude::*;

use super::BASE64;

/// O pacote oficial embutido (Cadeia Completa, inclui expiradas).
pub const PACOTE_ICP_BRASIL: &[u8] = include_bytes!("../../dados/icp-brasil/ACcompactadox.zip");
/// SHA-512 publicado pelo ITI em `hashsha512x.txt` (conferido em 20/10/2026).
pub const PACOTE_SHA512_PUBLICADO: &str = "48a9ed4bd1e2a81bf31e00b981501854b979cdee4eb402c8da658184ab6bd55f2522863ff1e7a97c18159dea73f21ae080f299613e2ef4413a548483b50b5b56";

/// Raízes da ICP-Brasil aceitas como âncora (SHA-256 do certificado inteiro).
const ESCOLHIDAS: &[(&str, Categoria)] = &[
    // Autoridade Certificadora Raiz Brasileira v5 (2016–2029, RSA-4096/SHA-512)
    ("caa53fc6091c6951887c976e378f6ef89aa6377c55d97b6475422b71ed7e9b17", Categoria::Vigente),
    // Autoridade Certificadora Raiz Brasileira v12 (2024–2037, RSA-4096/SHA-512)
    ("d8478e37ce19c690cf657381e68fe600e4e1a042536830f06847e03e554c4b01", Categoria::Vigente),
    // Autoridade Certificadora Raiz Brasileira v2 (2010–2023, expirada): histórica
    ("fb47d92a9909fd4fa9bec02737543e1f3514ced747407a8d9cfa397b0915067c", Categoria::Historica),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Categoria {
    Vigente,
    /// Só vale para notas emitidas e recebidas dentro da validade da raiz.
    Historica,
}

pub struct Raiz {
    pub nome: String,
    pub categoria: Categoria,
    pub sha256: String,
    /// Validade (segundos desde 1970, UTC).
    pub de: i64,
    pub ate: i64,
    pub(super) ancora: TrustAnchor<'static>,
}

pub struct Ancoras {
    raizes: Vec<Raiz>,
    pub(super) intermediarias: Vec<CertificateDer<'static>>,
    no_pacote: usize,
}

fn sha256_hex(d: &[u8]) -> String {
    Sha256::digest(d).iter().map(|b| format!("{b:02x}")).collect()
}

fn nome_comum(c: &X509Certificate) -> String {
    c.subject()
        .iter_common_name()
        .next()
        .and_then(|n| n.as_str().ok())
        .map(str::to_string)
        .unwrap_or_else(|| c.subject().to_string())
}

/// Lê um arquivo `.crt` do pacote (DER ou PEM).
fn der_do_arquivo(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.starts_with(b"-----BEGIN") {
        let t = std::str::from_utf8(bytes).ok()?;
        let corpo: String = t.lines().filter(|l| !l.starts_with("-----")).collect::<Vec<_>>().join("");
        let corpo: String = corpo.chars().filter(|c| !c.is_whitespace()).collect();
        BASE64.decode(corpo).ok()
    } else {
        Some(bytes.to_vec())
    }
}

impl Ancoras {
    /// As âncoras de verdade (ICP-Brasil), do pacote embutido.
    pub fn icp_brasil() -> &'static Ancoras {
        static A: OnceLock<Ancoras> = OnceLock::new();
        A.get_or_init(|| Ancoras::do_pacote(PACOTE_ICP_BRASIL).expect("pacote de ACs da ICP-Brasil embutido"))
    }

    /// As âncoras que o programa usa. No programa final, sempre as da
    /// ICP-Brasil; só nos testes, as fictícias de `tests/pki`.
    pub fn padrao() -> &'static Ancoras {
        #[cfg(any(test, feature = "ancoras-de-teste"))]
        {
            Ancoras::de_teste()
        }
        #[cfg(not(any(test, feature = "ancoras-de-teste")))]
        {
            Ancoras::icp_brasil()
        }
    }

    /// Raiz e ACs FICTÍCIAS (SEM VALIDADE – SÓ TESTE).
    #[cfg(any(test, feature = "ancoras-de-teste"))]
    pub fn de_teste() -> &'static Ancoras {
        static A: OnceLock<Ancoras> = OnceLock::new();
        A.get_or_init(|| {
            let d = |n: &str| -> Vec<u8> {
                match n {
                    "raiz_teste.der" => include_bytes!("../../tests/pki/raiz_teste.der").to_vec(),
                    "raiz_historica_teste.der" => include_bytes!("../../tests/pki/raiz_historica_teste.der").to_vec(),
                    "ac_teste.der" => include_bytes!("../../tests/pki/ac_teste.der").to_vec(),
                    "ac_sem_keycertsign.der" => include_bytes!("../../tests/pki/ac_sem_keycertsign.der").to_vec(),
                    "ac_vence_no_recebimento.der" => {
                        include_bytes!("../../tests/pki/ac_vence_no_recebimento.der").to_vec()
                    }
                    _ => include_bytes!("../../tests/pki/ac_raiz_historica.der").to_vec(),
                }
            };
            Ancoras::montar(
                vec![
                    (d("raiz_teste.der"), Categoria::Vigente),
                    (d("raiz_historica_teste.der"), Categoria::Historica),
                ],
                vec![
                    d("ac_teste.der"),
                    d("ac_sem_keycertsign.der"),
                    d("ac_vence_no_recebimento.der"),
                    d("ac_raiz_historica.der"),
                ],
            )
            .expect("PKI de teste")
        })
    }

    /// Monta a partir de certificados já escolhidos (testes).
    pub fn montar(raizes: Vec<(Vec<u8>, Categoria)>, intermediarias: Vec<Vec<u8>>) -> Result<Ancoras, String> {
        let mut r = Vec::new();
        for (der, categoria) in raizes {
            r.push(Ancoras::raiz(der, categoria)?);
        }
        let n = intermediarias.len();
        Ok(Ancoras {
            raizes: r,
            intermediarias: intermediarias.into_iter().map(CertificateDer::from).collect(),
            no_pacote: n,
        })
    }

    fn raiz(der: Vec<u8>, categoria: Categoria) -> Result<Raiz, String> {
        let (_, c) = X509Certificate::from_der(&der).map_err(|e| format!("raiz ilegível: {e}"))?;
        let nome = nome_comum(&c);
        let de = c.validity().not_before.timestamp();
        let ate = c.validity().not_after.timestamp();
        let sha256 = sha256_hex(&der);
        let ancora = webpki::anchor_from_trusted_cert(&CertificateDer::from(der.as_slice()))
            .map_err(|e| format!("raiz {nome}: {e:?}"))?
            .to_owned();
        Ok(Raiz { nome, categoria, sha256, de, ate, ancora })
    }

    /// Lê o pacote do ITI: confere o SHA-512, separa as âncoras escolhidas
    /// (por SHA-256) e guarda as ACs intermediárias (as demais raízes,
    /// autoassinadas, ficam de fora: nunca servem de elo).
    pub fn do_pacote(zip: &[u8]) -> Result<Ancoras, String> {
        let h: String = Sha512::digest(zip).iter().map(|b| format!("{b:02x}")).collect();
        if h != PACOTE_SHA512_PUBLICADO {
            return Err("o pacote de ACs não confere com o SHA-512 publicado pelo ITI".into());
        }
        let mut arquivo = zip::ZipArchive::new(std::io::Cursor::new(zip)).map_err(|e| e.to_string())?;
        if arquivo.len() > 5000 {
            return Err("pacote de ACs com arquivos demais".into());
        }
        let mut raizes = Vec::new();
        let mut inter: Vec<CertificateDer<'static>> = Vec::new();
        let mut vistos = std::collections::HashSet::new();
        for i in 0..arquivo.len() {
            let mut f = arquivo.by_index(i).map_err(|e| e.to_string())?;
            if f.is_dir() || f.size() > 256 * 1024 {
                continue;
            }
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut f, &mut bytes).map_err(|e| e.to_string())?;
            let Some(der) = der_do_arquivo(&bytes) else { continue };
            let sha = sha256_hex(&der);
            if !vistos.insert(sha.clone()) {
                continue;
            }
            let Ok((_, c)) = X509Certificate::from_der(&der) else { continue };
            if let Some((_, cat)) = ESCOLHIDAS.iter().find(|(s, _)| *s == sha) {
                raizes.push(Ancoras::raiz(der, *cat)?);
            } else if c.subject().as_raw() != c.issuer().as_raw() {
                inter.push(CertificateDer::from(der));
            }
        }
        if raizes.len() != ESCOLHIDAS.len() {
            return Err("alguma raiz escolhida não está no pacote".into());
        }
        let no_pacote = vistos.len();
        Ok(Ancoras { raizes, intermediarias: inter, no_pacote })
    }

    pub fn raizes(&self) -> impl Iterator<Item = &Raiz> {
        self.raizes.iter()
    }

    /// Certificados distintos lidos do pacote (âncoras + intermediárias + raízes ignoradas).
    pub fn quantidade_no_pacote(&self) -> usize {
        self.no_pacote
    }

    pub(super) fn ancoras_webpki(&self) -> Vec<TrustAnchor<'static>> {
        self.raizes.iter().map(|r| r.ancora.clone()).collect()
    }

    pub(super) fn raiz_da_ancora(&self, a: &TrustAnchor) -> Option<&Raiz> {
        self.raizes
            .iter()
            .find(|r| r.ancora.subject.as_ref() == a.subject.as_ref() && r.ancora.subject_public_key_info.as_ref() == a.subject_public_key_info.as_ref())
    }
}
