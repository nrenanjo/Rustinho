//! PEND-09 · Verificação da assinatura digital das NF-e recebidas.
//!
//! O que é provado: **o emitente assinou este conteúdo** com a chave de um
//! certificado ICP-Brasil tipo A1 ou A3 do próprio emitente, e nada mudou
//! depois. O que **não** é provado: que a SEFAZ autorizou a nota (o `protNFe`
//! dos arquivos distribuídos não é assinado); isso é o PEND-15 ou a
//! conferência do gerente (D7). Critérios: `docs/PEND-09-criterios.md`.
//!
//! Nenhuma criptografia própria: SHA-1/SHA-256 e RSA do `ring`, cadeia X.509
//! do `rustls-webpki`, leitura dos certificados do `x509-parser`. Próprios,
//! e fechados num perfil: a canonicalização (`c14n`) e a conferência estrita
//! da estrutura (aqui).
//!
//! Ordem (para na primeira falha): tamanho e profundidade → estrutura →
//! algoritmos → resumo → assinatura RSA → chave ≥ 2048 → uso do certificado
//! (U1–U7) → validade e cadeia em `dhEmi` **e** em `dhRecbto`, sem tolerância
//! → titular (raiz do CNPJ / CPF) → `tpAmb = 1`.

pub mod ancoras;
pub mod c14n;

#[cfg(test)]
mod testes;

use std::cell::RefCell;
use std::fmt;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use roxmltree::{Document, Node, NodeId, NodeType, ParsingOptions};
use rustls_pki_types::{CertificateDer, UnixTime};
use x509_parser::prelude::*;

pub use ancoras::{Ancoras, Categoria, Raiz};

/// Verdadeiro se o recurso `ancoras-de-teste` (raiz FICTÍCIA) foi compilado.
/// O aplicativo se recusa a abrir quando isso acontece.
pub const ANCORAS_DE_TESTE_COMPILADAS: bool = cfg!(feature = "ancoras-de-teste");

pub const NS_NFE: &str = "http://www.portalfiscal.inf.br/nfe";
pub const NS_DS: &str = "http://www.w3.org/2000/09/xmldsig#";

const C14N: &str = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315";
const ENVELOPED: &str = "http://www.w3.org/2000/09/xmldsig#enveloped-signature";
const SHA1: &str = "http://www.w3.org/2000/09/xmldsig#sha1";
const RSA_SHA1: &str = "http://www.w3.org/2000/09/xmldsig#rsa-sha1";
const SHA256: &str = "http://www.w3.org/2001/04/xmlenc#sha256";
const RSA_SHA256: &str = "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256";

/// Limites contra arquivos feitos para travar o programa.
pub const TAMANHO_MAXIMO: usize = 5 * 1024 * 1024;
pub const PROFUNDIDADE_MAXIMA: usize = 64;

/// Política de certificado ICP-Brasil (MOC: "tipo A1 ou A3").
const POLITICA_A1: &str = "2.16.76.1.2.1.";
const POLITICA_A3: &str = "2.16.76.1.2.3.";
const OID_CNPJ: &str = "2.16.76.1.3.3";
const OID_PESSOA_FISICA: &str = "2.16.76.1.3.1";

#[cfg(test)]
pub(crate) const EXEMPLO_TESTE: &str = include_str!("../../tests/nfe_exemplo.xml");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motivo {
    /// Não é XML, está truncado, grande ou profundo demais, tem DTD/entidades.
    Malformado,
    /// Fora do formato da NF-e distribuída (nfeProc → NFe → infNFe + Signature).
    Estrutura,
    /// Algoritmo ou transformação fora do padrão da NF-e.
    Algoritmo,
    /// O conteúdo não é o que foi assinado.
    Resumo,
    /// A assinatura não confere com a chave do certificado.
    Assinatura,
    /// Certificado ilegível ou com chave fraca.
    Certificado,
    /// Uso do certificado fora do perfil ICP-Brasil A1/A3 (KeyUsage, EKU, CA).
    Uso,
    /// Tipo de certificado não aceito para NF-e (política fora de A1/A3).
    Tipo,
    /// Não chega a uma raiz da ICP-Brasil aceita.
    Cadeia,
    /// Fora da validade na emissão ou no recebimento.
    Validade,
    /// O certificado não é do emitente da nota.
    Titular,
    /// Nota de homologação (sem valor fiscal).
    Homologacao,
}

#[derive(Debug, Clone)]
pub struct Recusa {
    pub motivo: Motivo,
    pub texto: String,
}

impl fmt::Display for Recusa {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inicio = match self.motivo {
            Motivo::Malformado => "O arquivo não é um XML de NF-e legível",
            Motivo::Estrutura => "O XML não está no formato da NF-e assinada",
            Motivo::Algoritmo => "A assinatura usa um padrão que a NF-e não usa",
            Motivo::Resumo => "O conteúdo da nota não é o que foi assinado: ela foi alterada depois da assinatura",
            Motivo::Assinatura => "A assinatura digital não confere com o certificado do emitente",
            Motivo::Certificado => "O certificado digital do emitente não pôde ser aceito",
            Motivo::Uso => "O certificado do emitente não é de assinatura de NF-e (uso fora do perfil ICP-Brasil)",
            Motivo::Tipo => "O tipo de certificado do emitente não é aceito para NF-e (precisa ser A1 ou A3)",
            Motivo::Cadeia => {
                "O certificado do emitente não é de uma autoridade certificadora da ICP-Brasil reconhecida"
            }
            Motivo::Validade => "O certificado do emitente não estava válido na emissão ou na autorização da nota",
            Motivo::Titular => "O certificado da assinatura não é do emitente da nota",
            Motivo::Homologacao => "Esta nota é de homologação (teste da SEFAZ), sem valor fiscal",
        };
        if self.texto.is_empty() {
            write!(f, "{inicio}.")
        } else {
            write!(f, "{inicio} ({}).", self.texto)
        }
    }
}

fn recusa(motivo: Motivo, texto: impl Into<String>) -> Recusa {
    Recusa { motivo, texto: texto.into() }
}

/// O que ficou provado.
#[derive(Debug, Clone)]
pub struct Prova {
    /// CNPJ (14) ou CPF (11) do titular do certificado.
    pub titular: String,
    /// Nome do titular no certificado.
    pub sujeito: String,
    /// ACs do caminho (da emissora para cima), sem a raiz.
    pub cadeia: Vec<String>,
    pub raiz: String,
    pub categoria_raiz: Categoria,
    /// "rsa-sha1" (padrão da NF-e 4.00) ou "rsa-sha256".
    pub algoritmo: &'static str,
    /// "A1" ou "A3" (pela política do certificado).
    pub tipo: &'static str,
    /// Observações técnicas que não recusam (ex.: fuso errado no recebimento).
    pub avisos: Vec<String>,
    /// `dhEmi` e `dhRecbto` (segundos desde 1970, UTC). Sem protocolo
    /// (só no cadastro, D8), não há `dhRecbto`.
    pub emissao: i64,
    pub recebimento: Option<i64>,
}

/// Nota verificada: o documento e **o nó `infNFe` que foi verificado** (a
/// leitura dos dados usa este nó, nunca uma nova busca por nome).
#[derive(Debug)]
pub struct Verificado<'a> {
    pub doc: Document<'a>,
    pub inf: NodeId,
    pub prova: Prova,
}

fn elementos<'a, 'i>(no: Node<'a, 'i>) -> Vec<Node<'a, 'i>> {
    no.children().filter(Node::is_element).collect()
}

fn e(no: Node, ns: &str, nome: &str) -> bool {
    no.has_tag_name((ns, nome))
}

/// Só pode ter texto de espaço entre os filhos (nada de texto solto).
fn so_espacos(no: Node) -> bool {
    no.children().all(|f| match f.node_type() {
        NodeType::Element => true,
        NodeType::Text => f.text().unwrap_or("").trim().is_empty(),
        _ => false,
    })
}

fn texto_limpo(no: Node) -> String {
    no.children()
        .filter(|f| f.is_text())
        .filter_map(|f| f.text())
        .collect::<String>()
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect()
}

/// O elemento só tem `Algorithm` e nenhum filho elemento (sem parâmetros).
fn metodo<'a, 'i>(no: Node<'a, 'i>) -> Result<&'a str, Recusa> {
    if no.attributes().len() != 1 || !elementos(no).is_empty() || !so_espacos(no) {
        return Err(recusa(Motivo::Algoritmo, format!("{} com parâmetros", no.tag_name().name())));
    }
    no.attribute("Algorithm").ok_or_else(|| recusa(Motivo::Estrutura, "método sem Algorithm"))
}

fn nomes(lista: &[Node], ns: &str, esperado: &[&str]) -> bool {
    lista.len() == esperado.len() && lista.iter().zip(esperado).all(|(n, x)| e(*n, ns, x))
}

/// Conta o aninhamento **antes** de ler o XML. O leitor (`roxmltree`) é
/// recursivo e não tem limite: um arquivo com milhares de níveis estoura a
/// pilha e derruba o programa (não dá para interceptar). Também é usado pelo
/// leitor de NF-e de antes (`nfe_xml`). Arquivo malformado devolve `false`
/// (o leitor recusa em seguida).
pub fn profundidade_excessiva(texto: &str, limite: usize) -> bool {
    let b = texto.as_bytes();
    let achar =
        |de: usize, alvo: &[u8]| b[de.min(b.len())..].windows(alvo.len()).position(|w| w == alvo).map(|p| p + de);
    let (mut i, mut d) = (0usize, 0usize);
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        let resto = &b[i..];
        let pular = if resto.starts_with(b"<!--") {
            achar(i + 4, b"-->").map(|j| j + 3)
        } else if resto.starts_with(b"<![CDATA[") {
            achar(i + 9, b"]]>").map(|j| j + 3)
        } else if resto.starts_with(b"<?") {
            achar(i + 2, b"?>").map(|j| j + 2)
        } else if resto.starts_with(b"<!") || resto.starts_with(b"</") {
            if resto.starts_with(b"</") {
                d = d.saturating_sub(1);
            }
            achar(i + 2, b">").map(|j| j + 1)
        } else {
            // Abertura: vai até o '>' fora de aspas.
            let (mut j, mut aspas) = (i + 1, 0u8);
            while j < b.len() {
                let c = b[j];
                if aspas != 0 {
                    if c == aspas {
                        aspas = 0;
                    }
                } else if c == b'"' || c == b'\'' {
                    aspas = c;
                } else if c == b'>' {
                    break;
                }
                j += 1;
            }
            if j >= b.len() {
                return false;
            }
            if b[j - 1] != b'/' {
                d += 1;
                if d > limite {
                    return true;
                }
            }
            Some(j + 1)
        };
        match pular {
            Some(j) => i = j,
            None => return false,
        }
    }
    false
}

fn profundidade_maxima(doc: &Document) -> usize {
    let mut maior = 0;
    let mut pilha = vec![(doc.root(), 0usize)];
    while let Some((n, d)) = pilha.pop() {
        maior = maior.max(d);
        if d > PROFUNDIDADE_MAXIMA {
            break;
        }
        for f in n.children().filter(Node::is_element) {
            pilha.push((f, d + 1));
        }
    }
    maior
}

/// O nome como está escrito no arquivo (para pegar prefixos).
fn nome_escrito<'i>(texto: &'i str, no: Node) -> &'i str {
    let r = no.range();
    let resto = &texto[r.start + 1..r.end.min(texto.len())];
    let fim = resto.find(|c: char| c.is_whitespace() || c == '>' || c == '/').unwrap_or(resto.len());
    &resto[..fim]
}

/// Perfil fechado da região assinada (`infNFe`) e da `Signature`.
fn conferir_perfil(texto: &str, raiz: Node, proibido_ds: bool) -> Result<(), Recusa> {
    for n in raiz.descendants() {
        match n.node_type() {
            NodeType::Element => {
                if nome_escrito(texto, n).contains(':') {
                    return Err(recusa(Motivo::Estrutura, "elemento com prefixo"));
                }
                if n.attributes().any(|a| a.namespace().is_some()) {
                    return Err(recusa(Motivo::Estrutura, "atributo com prefixo"));
                }
                if proibido_ds && n.tag_name().namespace() == Some(NS_DS) {
                    return Err(recusa(Motivo::Estrutura, "assinatura dentro do conteúdo assinado"));
                }
            }
            NodeType::Comment => return Err(recusa(Motivo::Estrutura, "comentário na região assinada")),
            NodeType::PI => return Err(recusa(Motivo::Estrutura, "instrução de processamento na região assinada")),
            _ => {}
        }
    }
    Ok(())
}

fn data(s: &str) -> Option<i64> {
    ::time::OffsetDateTime::parse(s.trim(), &::time::format_description::well_known::Rfc3339)
        .ok()
        .map(|d| d.unix_timestamp())
}

fn texto_do_filho(no: Node, nome: &str) -> Option<String> {
    no.children().find(|f| e(*f, NS_NFE, nome)).and_then(|f| f.text()).map(|t| t.trim().to_string())
}

/// Aceita qualquer EKU nas ACs e na folha: o EKU da folha é conferido como
/// **perfil** (U3), não como permissão de TLS. A regra `clientAuth` é do
/// transporte (PEND-15/emissão) e não é usada aqui.
struct EkuDoPerfilNfe;

impl webpki::ExtendedKeyUsageValidator for EkuDoPerfilNfe {
    fn validate(&self, iter: webpki::KeyPurposeIdIter<'_, '_>) -> Result<(), webpki::Error> {
        for id in iter {
            id?;
        }
        Ok(())
    }
}

static ALGORITMOS_DA_CADEIA: &[&dyn rustls_pki_types::SignatureVerificationAlgorithm] = &[
    webpki::ring::RSA_PKCS1_2048_8192_SHA256,
    webpki::ring::RSA_PKCS1_2048_8192_SHA384,
    webpki::ring::RSA_PKCS1_2048_8192_SHA512,
    webpki::ring::RSA_PKCS1_2048_8192_SHA256_ABSENT_PARAMS,
    webpki::ring::RSA_PKCS1_2048_8192_SHA384_ABSENT_PARAMS,
    webpki::ring::RSA_PKCS1_2048_8192_SHA512_ABSENT_PARAMS,
    webpki::ring::RSA_PKCS1_3072_8192_SHA384,
];

/// U1–U5 e U7 (§3.6). Devolve o tipo (A1/A3) e avisos.
fn conferir_uso(c: &X509Certificate) -> Result<(&'static str, Vec<String>), Recusa> {
    let mut avisos = Vec::new();
    let ilegivel = |_| recusa(Motivo::Certificado, "extensão ilegível");
    // U1: KeyUsage presente, crítica, com digitalSignature e nonRepudiation.
    let ku = c.key_usage().map_err(ilegivel)?.ok_or_else(|| recusa(Motivo::Uso, "sem KeyUsage"))?;
    if !ku.critical {
        return Err(recusa(Motivo::Uso, "KeyUsage não crítica"));
    }
    if !ku.value.digital_signature() {
        return Err(recusa(Motivo::Uso, "sem digitalSignature"));
    }
    if !ku.value.non_repudiation() {
        return Err(recusa(Motivo::Uso, "sem nonRepudiation"));
    }
    // U2: keyEncipherment não participa da assinatura; só registra.
    if !ku.value.key_encipherment() {
        avisos.push("certificado sem keyEncipherment (fora do perfil A1/A3, irrelevante para a assinatura)".into());
    }
    // U3: EKU presente com clientAuth ou emailProtection (perfil, não TLS).
    let eku = c.extended_key_usage().map_err(ilegivel)?.ok_or_else(|| recusa(Motivo::Uso, "sem ExtendedKeyUsage"))?;
    if !(eku.value.client_auth || eku.value.email_protection) {
        return Err(recusa(Motivo::Uso, "ExtendedKeyUsage sem clientAuth nem emailProtection"));
    }
    // U5: certificado de usuário final.
    if let Some(bc) = c.basic_constraints().map_err(ilegivel)? {
        if bc.value.ca {
            return Err(recusa(Motivo::Uso, "certificado de AC usado como de emitente"));
        }
    }
    // U4: política A1 ou A3.
    let mut tipo = None;
    for x in c.extensions() {
        if let ParsedExtension::CertificatePolicies(p) = x.parsed_extension() {
            for pol in p {
                let id = pol.policy_id.to_id_string();
                if id.starts_with(POLITICA_A1) {
                    tipo = Some("A1");
                } else if id.starts_with(POLITICA_A3) {
                    tipo = tipo.or(Some("A3"));
                }
            }
        }
    }
    let tipo = tipo.ok_or_else(|| recusa(Motivo::Tipo, "política do certificado fora de A1/A3"))?;
    Ok((tipo, avisos))
}

/// Conteúdo de um `otherName` ICP-Brasil (OCTET STRING, PrintableString ou
/// UTF8String dentro do `[0]` explícito).
fn conteudo_outro_nome(bruto: &[u8]) -> Option<String> {
    fn tlv(b: &[u8]) -> Option<(u8, &[u8])> {
        let (&tag, resto) = b.split_first()?;
        let (&l, resto) = resto.split_first()?;
        let (n, resto) = if l < 0x80 {
            (l as usize, resto)
        } else {
            let k = (l & 0x7f) as usize;
            if k == 0 || k > 2 || resto.len() < k {
                return None;
            }
            let n = resto[..k].iter().fold(0usize, |a, &x| (a << 8) | x as usize);
            (n, &resto[k..])
        };
        Some((tag, resto.get(..n)?))
    }
    let (tag, dentro) = tlv(bruto)?;
    let (tag, valor) = if tag == 0xA0 { tlv(dentro)? } else { (tag, dentro) };
    if !matches!(tag, 0x04 | 0x13 | 0x0C) {
        return None;
    }
    String::from_utf8(valor.to_vec()).ok()
}

/// (CNPJ, CPF) do titular, pelos `otherName` 2.16.76.1.3.3 e 2.16.76.1.3.1.
fn titular_do_certificado(c: &X509Certificate) -> (Option<String>, Option<String>) {
    let (mut cnpj, mut cpf) = (None, None);
    if let Ok(Some(san)) = c.subject_alternative_name() {
        for g in &san.value.general_names {
            if let GeneralName::OtherName(oid, bruto) = g {
                let id = oid.to_id_string();
                let Some(v) = conteudo_outro_nome(bruto) else { continue };
                if id == OID_CNPJ && v.len() == 14 && v.bytes().all(|b| b.is_ascii_digit()) {
                    cnpj = Some(v);
                } else if id == OID_PESSOA_FISICA && v.len() >= 19 {
                    let x = &v[8..19];
                    if x.bytes().all(|b| b.is_ascii_digit()) {
                        cpf = Some(x.to_string());
                    }
                }
            }
        }
    }
    (cnpj, cpf)
}

fn nome_comum(c: &X509Certificate) -> String {
    c.subject()
        .iter_common_name()
        .next()
        .and_then(|n| n.as_str().ok())
        .map(str::to_string)
        .unwrap_or_else(|| c.subject().to_string())
}

fn iguais(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Verifica a nota distribuída (`nfeProc` com o protocolo): entrada de compra.
pub fn verificar<'a>(xml: &'a str, ancoras: &Ancoras) -> Result<Verificado<'a>, Recusa> {
    verificar_com(xml, ancoras, true)
}

/// Para cadastrar pessoas e peças (D8): a assinatura do emitente é exigida
/// do mesmo jeito; o protocolo da SEFAZ não (aceita também a `NFe` sozinha).
/// Havendo protocolo, a validade também é conferida no recebimento.
pub fn verificar_para_cadastro<'a>(xml: &'a str, ancoras: &Ancoras) -> Result<Verificado<'a>, Recusa> {
    verificar_com(xml, ancoras, false)
}

/// Verifica a nota (§3.3, §3.5, §3.6 dos critérios).
fn verificar_com<'a>(xml: &'a str, ancoras: &Ancoras, exigir_protocolo: bool) -> Result<Verificado<'a>, Recusa> {
    // ---- tamanho, leitura, profundidade
    let x = xml.trim_start_matches('\u{feff}').trim_start();
    if x.is_empty() {
        return Err(recusa(Motivo::Malformado, "arquivo vazio"));
    }
    if x.len() > TAMANHO_MAXIMO {
        return Err(recusa(Motivo::Malformado, "maior que 5 MB"));
    }
    if profundidade_excessiva(x, PROFUNDIDADE_MAXIMA) {
        return Err(recusa(Motivo::Malformado, "aninhamento profundo demais"));
    }
    let opcoes = ParsingOptions { allow_dtd: false, nodes_limit: 1_000_000, ..ParsingOptions::default() };
    let doc = Document::parse_with_options(x, opcoes).map_err(|e| recusa(Motivo::Malformado, e.to_string()))?;
    if profundidade_maxima(&doc) > PROFUNDIDADE_MAXIMA {
        return Err(recusa(Motivo::Malformado, "aninhamento profundo demais"));
    }

    // ---- estrutura: nfeProc → [NFe, protNFe]; NFe → [infNFe, (infNFeSupl), Signature]
    let raiz = doc.root_element();
    let (nfe, prot) = if e(raiz, NS_NFE, "nfeProc") {
        let filhos = elementos(raiz);
        if !nomes(&filhos, NS_NFE, &["NFe", "protNFe"]) {
            return Err(recusa(Motivo::Estrutura, "nfeProc deve ter NFe e protNFe"));
        }
        (filhos[0], Some(filhos[1]))
    } else if !exigir_protocolo && e(raiz, NS_NFE, "NFe") {
        (raiz, None)
    } else {
        return Err(recusa(Motivo::Estrutura, "a raiz não é nfeProc"));
    };
    let nf = elementos(nfe);
    let ok_nf = match nf.len() {
        2 => e(nf[0], NS_NFE, "infNFe") && e(nf[1], NS_DS, "Signature"),
        3 => e(nf[0], NS_NFE, "infNFe") && e(nf[1], NS_NFE, "infNFeSupl") && e(nf[2], NS_DS, "Signature"),
        _ => false,
    };
    if !ok_nf {
        return Err(recusa(Motivo::Estrutura, "NFe deve ter infNFe e uma Signature"));
    }
    let (inf, sig) = (nf[0], nf[nf.len() - 1]);
    let id = inf.attribute("Id").unwrap_or("").to_string();
    if id.is_empty() {
        return Err(recusa(Motivo::Estrutura, "infNFe sem Id"));
    }
    if doc.descendants().filter(|n| e(*n, NS_NFE, "infNFe")).count() != 1
        || doc.descendants().filter(|n| e(*n, NS_NFE, "NFe")).count() != 1
    {
        return Err(recusa(Motivo::Estrutura, "mais de uma nota no arquivo"));
    }
    let com_id = doc
        .descendants()
        .filter(Node::is_element)
        .filter(|n| n.attributes().any(|a| matches!(a.name(), "Id" | "ID" | "id") && a.value() == id))
        .count();
    if com_id != 1 {
        return Err(recusa(Motivo::Estrutura, "identificador repetido"));
    }
    // Outras assinaturas só dentro do protNFe (ignoradas: não provam nada aqui).
    for s in doc.descendants().filter(|n| e(*n, NS_DS, "Signature")) {
        if s != sig && !s.ancestors().any(|a| Some(a) == prot) {
            return Err(recusa(Motivo::Estrutura, "assinatura fora do lugar"));
        }
    }
    conferir_perfil(x, inf, true)?;
    conferir_perfil(x, sig, false)?;
    for a in inf.ancestors().filter(Node::is_element) {
        if nome_escrito(x, a).contains(':') || a.attributes().any(|t| t.namespace().is_some()) {
            return Err(recusa(Motivo::Estrutura, "prefixo acima do conteúdo assinado"));
        }
    }

    // ---- Signature
    let s = elementos(sig);
    if !nomes(&s, NS_DS, &["SignedInfo", "SignatureValue", "KeyInfo"]) || !so_espacos(sig) {
        return Err(recusa(Motivo::Estrutura, "Signature fora do padrão"));
    }
    let (si, sv, ki) = (s[0], s[1], s[2]);
    let sif = elementos(si);
    if !nomes(&sif, NS_DS, &["CanonicalizationMethod", "SignatureMethod", "Reference"]) || !so_espacos(si) {
        return Err(recusa(Motivo::Estrutura, "SignedInfo fora do padrão"));
    }
    if metodo(sif[0])? != C14N {
        return Err(recusa(Motivo::Algoritmo, "canonicalização"));
    }
    let metodo_assinatura = metodo(sif[1])?.to_string();
    let referencia = sif[2];
    if referencia.attributes().len() != 1 || referencia.attribute("URI") != Some(format!("#{id}").as_str()) {
        return Err(recusa(Motivo::Estrutura, "a assinatura não aponta para esta nota"));
    }
    let rf = elementos(referencia);
    if !nomes(&rf, NS_DS, &["Transforms", "DigestMethod", "DigestValue"]) || !so_espacos(referencia) {
        return Err(recusa(Motivo::Estrutura, "Reference fora do padrão"));
    }
    let tr = elementos(rf[0]);
    if !so_espacos(rf[0]) || tr.iter().any(|t| !e(*t, NS_DS, "Transform")) {
        return Err(recusa(Motivo::Estrutura, "Transforms fora do padrão"));
    }
    let algs: Vec<&str> = tr.iter().map(|t| metodo(*t)).collect::<Result<_, _>>()?;
    if algs != [ENVELOPED, C14N] {
        return Err(recusa(Motivo::Algoritmo, "transformações"));
    }
    let metodo_resumo = metodo(rf[1])?.to_string();
    let (algoritmo, resumo_alg, rsa_alg): (&'static str, _, &'static dyn ring::signature::VerificationAlgorithm) =
        match (metodo_assinatura.as_str(), metodo_resumo.as_str()) {
            (RSA_SHA1, SHA1) => (
                "rsa-sha1",
                &ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
                &ring::signature::RSA_PKCS1_2048_8192_SHA1_FOR_LEGACY_USE_ONLY,
            ),
            (RSA_SHA256, SHA256) => ("rsa-sha256", &ring::digest::SHA256, &ring::signature::RSA_PKCS1_2048_8192_SHA256),
            _ => return Err(recusa(Motivo::Algoritmo, "par de algoritmos")),
        };
    let kif = elementos(ki);
    if !nomes(&kif, NS_DS, &["X509Data"]) || !so_espacos(ki) {
        return Err(recusa(Motivo::Estrutura, "KeyInfo fora do padrão"));
    }
    let xd = elementos(kif[0]);
    if !nomes(&xd, NS_DS, &["X509Certificate"]) || !so_espacos(kif[0]) {
        return Err(recusa(Motivo::Estrutura, "X509Data fora do padrão"));
    }

    // ---- resumo
    let declarado = BASE64.decode(texto_limpo(rf[2])).map_err(|_| recusa(Motivo::Estrutura, "DigestValue inválido"))?;
    let canonico = c14n::canonicalizar(inf).map_err(|e| recusa(Motivo::Estrutura, e))?;
    let calculado = ring::digest::digest(resumo_alg, &canonico);
    if !iguais(calculado.as_ref(), &declarado) {
        return Err(recusa(Motivo::Resumo, ""));
    }

    // ---- certificado e assinatura RSA
    let der =
        BASE64.decode(texto_limpo(xd[0])).map_err(|_| recusa(Motivo::Certificado, "certificado em Base64 inválido"))?;
    let (sobra, cert) =
        X509Certificate::from_der(&der).map_err(|_| recusa(Motivo::Certificado, "certificado ilegível"))?;
    if !sobra.is_empty() {
        return Err(recusa(Motivo::Certificado, "bytes a mais depois do certificado"));
    }
    let bits = match cert.public_key().parsed() {
        Ok(x509_parser::public_key::PublicKey::RSA(k)) => k.key_size(),
        _ => return Err(recusa(Motivo::Certificado, "a chave não é RSA")),
    };
    if bits < 2048 {
        return Err(recusa(Motivo::Certificado, format!("chave RSA de {bits} bits; o mínimo é 2048")));
    }
    let valor = BASE64.decode(texto_limpo(sv)).map_err(|_| recusa(Motivo::Estrutura, "SignatureValue inválido"))?;
    let info = c14n::canonicalizar(si).map_err(|e| recusa(Motivo::Estrutura, e))?;
    ring::signature::UnparsedPublicKey::new(rsa_alg, cert.public_key().subject_public_key.data.as_ref())
        .verify(&info, &valor)
        .map_err(|_| recusa(Motivo::Assinatura, ""))?;

    // ---- uso do certificado (U1–U5)
    let (tipo, mut avisos) = conferir_uso(&cert)?;

    // ---- instantes: dhEmi (assinado) e dhRecbto (do protocolo, não assinado)
    let ide = inf.children().find(|n| e(*n, NS_NFE, "ide")).ok_or_else(|| recusa(Motivo::Estrutura, "sem ide"))?;
    let emissao = texto_do_filho(ide, "dhEmi")
        .and_then(|t| data(&t))
        .ok_or_else(|| recusa(Motivo::Estrutura, "data de emissão (dhEmi) ausente ou inválida"))?;
    let recebimento = match prot {
        Some(prot) => {
            let inf_prot = prot
                .children()
                .find(|n| e(*n, NS_NFE, "infProt"))
                .ok_or_else(|| recusa(Motivo::Estrutura, "protocolo sem infProt"))?;
            let r = texto_do_filho(inf_prot, "dhRecbto")
                .and_then(|t| data(&t))
                .ok_or_else(|| recusa(Motivo::Estrutura, "protocolo sem data de recebimento (dhRecbto)"))?;
            if r < emissao - 5 * 60 {
                avisos.push(
                    "data de recebimento anterior à emissão no protocolo (fuso horário errado no carimbo da SEFAZ)"
                        .into(),
                );
            }
            Some(r)
        }
        None => None,
    };
    let mut instantes = vec![(emissao, "na emissão")];
    if let Some(r) = recebimento {
        instantes.push((r, "no recebimento"));
    }
    let (de, ate) = (cert.validity().not_before.timestamp(), cert.validity().not_after.timestamp());
    for &(t, qual) in &instantes {
        if t < de || t > ate {
            return Err(recusa(Motivo::Validade, format!("certificado do emitente fora da validade {qual}")));
        }
    }

    // ---- cadeia até uma âncora, nos dois instantes
    let folha = CertificateDer::from(der.as_slice());
    let ee = webpki::EndEntityCert::try_from(&folha).map_err(|e| recusa(Motivo::Certificado, format!("{e:?}")))?;
    let lista = ancoras.ancoras_webpki();
    let mut cadeia = Vec::new();
    let mut raiz_usada = None;
    for &(t, qual) in &instantes {
        let problema: RefCell<Option<Recusa>> = RefCell::new(None);
        let achado: RefCell<Option<(Vec<String>, String, Categoria)>> = RefCell::new(None);
        let conferir = |p: &webpki::VerifiedPath<'_>| -> Result<(), webpki::Error> {
            let mut nomes_ac = Vec::new();
            for ac in p.intermediate_certificates() {
                let der_ac = ac.der();
                let Ok((_, c)) = X509Certificate::from_der(der_ac.as_ref()) else {
                    *problema.borrow_mut() = Some(recusa(Motivo::Cadeia, "AC ilegível"));
                    return Err(webpki::Error::UnknownIssuer);
                };
                // U8: AC com keyCertSign (a biblioteca ignora o KeyUsage).
                let assina = matches!(c.key_usage(), Ok(Some(k)) if k.value.key_cert_sign());
                if !assina {
                    *problema.borrow_mut() =
                        Some(recusa(Motivo::Cadeia, format!("AC sem keyCertSign: {}", nome_comum(&c))));
                    return Err(webpki::Error::UnknownIssuer);
                }
                nomes_ac.push(nome_comum(&c));
            }
            let Some(r) = ancoras.raiz_da_ancora(p.anchor()) else {
                return Err(webpki::Error::UnknownIssuer);
            };
            // A biblioteca não confere a validade da âncora; o Rustinho confere.
            if t < r.de || t > r.ate {
                *problema.borrow_mut() =
                    Some(recusa(Motivo::Validade, format!("raiz {} fora da validade {qual}", r.nome)));
                return Err(webpki::Error::UnknownIssuer);
            }
            *achado.borrow_mut() = Some((nomes_ac, r.nome.clone(), r.categoria));
            Ok(())
        };
        let quando = UnixTime::since_unix_epoch(Duration::from_secs(t.max(0) as u64));
        let r = ee.verify_for_usage(
            ALGORITMOS_DA_CADEIA,
            &lista,
            &ancoras.intermediarias,
            quando,
            EkuDoPerfilNfe,
            None,
            Some(&conferir),
        );
        if let Err(erro) = r {
            if let Some(p) = problema.into_inner() {
                return Err(p);
            }
            return Err(match erro {
                webpki::Error::CertExpired { .. } | webpki::Error::CertNotValidYet { .. } => {
                    recusa(Motivo::Validade, format!("certificado da cadeia fora da validade {qual}"))
                }
                outro => recusa(Motivo::Cadeia, format!("{outro:?}")),
            });
        }
        let (n, r, c) = achado.into_inner().ok_or_else(|| recusa(Motivo::Cadeia, "caminho sem âncora"))?;
        cadeia = n;
        raiz_usada = Some((r, c));
    }
    let (raiz_nome, categoria_raiz) = raiz_usada.ok_or_else(|| recusa(Motivo::Cadeia, ""))?;
    if categoria_raiz == Categoria::Historica {
        avisos.push(format!("cadeia histórica: {raiz_nome} (válida na emissão e no recebimento)"));
    }

    // ---- titular (regras F03 / F03A da NT 2026.001)
    let emit = inf.children().find(|n| e(*n, NS_NFE, "emit")).ok_or_else(|| recusa(Motivo::Estrutura, "sem emit"))?;
    let (cnpj_cert, cpf_cert) = titular_do_certificado(&cert);
    let titular = if let Some(cnpj) = texto_do_filho(emit, "CNPJ").filter(|c| !c.is_empty()) {
        match cnpj_cert {
            Some(c) if c.len() == 14 && cnpj.len() == 14 && c[..8] == cnpj[..8] => c,
            Some(_) => return Err(recusa(Motivo::Titular, "a raiz do CNPJ do certificado é diferente da do emitente")),
            None => return Err(recusa(Motivo::Titular, "o certificado não traz CNPJ")),
        }
    } else if let Some(cpf) = texto_do_filho(emit, "CPF").filter(|c| !c.is_empty()) {
        match cpf_cert {
            Some(c) if c == cpf => c,
            Some(_) => return Err(recusa(Motivo::Titular, "o CPF do certificado é diferente do emitente")),
            None => return Err(recusa(Motivo::Titular, "o certificado não traz CPF")),
        }
    } else {
        return Err(recusa(Motivo::Estrutura, "emitente sem CNPJ nem CPF"));
    };

    // ---- ambiente
    if texto_do_filho(ide, "tpAmb").as_deref() != Some("1") {
        return Err(recusa(Motivo::Homologacao, ""));
    }

    let prova = Prova {
        titular,
        sujeito: nome_comum(&cert),
        cadeia,
        raiz: raiz_nome,
        categoria_raiz,
        algoritmo,
        tipo,
        avisos,
        emissao,
        recebimento,
    };
    let inf = inf.id();
    Ok(Verificado { doc, inf, prova })
}
