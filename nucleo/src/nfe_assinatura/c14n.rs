//! Canonicalização XML 1.0 **inclusiva, sem comentários** (W3C
//! `REC-xml-c14n-20010315`) de **um elemento e tudo o que está dentro dele**,
//! no contexto do documento. É o único algoritmo de canonicalização que a NF-e
//! usa (MOC, "Padrão de Assinatura Digital").
//!
//! Perfil fechado (o que estiver fora dele é recusado, nunca interpretado):
//! nomes de elementos e atributos sem prefixo; nenhuma instrução de
//! processamento; nenhum atributo `xml:*` no elemento ou acima dele. Medido
//! no acervo real: nenhuma das 537 notas usa nada disso.
//!
//! Conferido byte a byte contra o libxml2 (`tests/pki/c14n`) e contra o resumo
//! de todas as assinaturas reais do acervo (teste ignorado
//! `acervo_a03_c14n_reproduz_o_resumo_de_todas_as_assinaturas`).

use std::collections::BTreeMap;

use roxmltree::{Node, NodeType};

const NS_XML: &str = "http://www.w3.org/XML/1998/namespace";

type Escopo = BTreeMap<String, String>;

/// Os bytes canônicos de `apice` (o elemento e seu conteúdo).
pub fn canonicalizar(apice: Node) -> Result<Vec<u8>, String> {
    if !apice.is_element() {
        return Err("o alvo da canonicalização não é um elemento".into());
    }
    for a in apice.ancestors().filter(Node::is_element) {
        if a.attributes().any(|x| x.namespace() == Some(NS_XML)) {
            return Err("atributo xml:* fora do perfil".into());
        }
    }
    let mut saida = Vec::with_capacity(4096);
    escrever(apice, None, &mut saida)?;
    Ok(saida)
}

/// Namespaces em escopo no elemento (o roxmltree já inclui os herdados).
/// A chave "" é o namespace padrão; o prefixo `xml` nunca é escrito.
fn escopo(no: Node) -> Escopo {
    let mut m = Escopo::new();
    for ns in no.namespaces() {
        let prefixo = ns.name().unwrap_or("");
        if prefixo == "xml" {
            continue;
        }
        m.insert(prefixo.to_string(), ns.uri().to_string());
    }
    m
}

fn escrever(no: Node, pai: Option<&Escopo>, s: &mut Vec<u8>) -> Result<(), String> {
    let meu = escopo(no);
    let nome = no.tag_name().name();
    // Sem prefixo: o namespace do elemento tem de ser o padrão em escopo.
    let padrao = meu.get("").map(String::as_str).filter(|u| !u.is_empty());
    if no.tag_name().namespace().filter(|u| !u.is_empty()) != padrao {
        return Err(format!("elemento com prefixo fora do perfil: {nome}"));
    }
    s.push(b'<');
    s.extend_from_slice(nome.as_bytes());

    // Declarações de namespace, em ordem de prefixo ("" primeiro).
    let padrao_pai = pai.and_then(|p| p.get("")).map(String::as_str).filter(|u| !u.is_empty());
    for (prefixo, uri) in &meu {
        let repetido = pai.and_then(|p| p.get(prefixo)) == Some(uri);
        if prefixo.is_empty() {
            if !uri.is_empty() && padrao_pai != Some(uri.as_str()) {
                declarar(s, "", uri);
            }
        } else if !repetido {
            declarar(s, prefixo, uri);
        }
    }
    if padrao.is_none() && padrao_pai.is_some() {
        declarar(s, "", "");
    }

    // Atributos: sem namespace (perfil), em ordem de nome.
    let mut atributos: Vec<(&str, &str)> = Vec::new();
    for a in no.attributes() {
        if a.namespace().is_some() {
            return Err(format!("atributo com prefixo fora do perfil: {}", a.name()));
        }
        atributos.push((a.name(), a.value()));
    }
    atributos.sort_by(|x, y| x.0.cmp(y.0));
    for (n, v) in atributos {
        s.push(b' ');
        s.extend_from_slice(n.as_bytes());
        s.extend_from_slice(b"=\"");
        escapar_atributo(v, s);
        s.push(b'"');
    }
    s.push(b'>');

    for filho in no.children() {
        match filho.node_type() {
            NodeType::Element => escrever(filho, Some(&meu), s)?,
            NodeType::Text => escapar_texto(filho.text().unwrap_or(""), s),
            NodeType::Comment => {}
            NodeType::PI => return Err("instrução de processamento fora do perfil".into()),
            NodeType::Root => {}
        }
    }
    s.extend_from_slice(b"</");
    s.extend_from_slice(nome.as_bytes());
    s.push(b'>');
    Ok(())
}

fn declarar(s: &mut Vec<u8>, prefixo: &str, uri: &str) {
    if prefixo.is_empty() {
        s.extend_from_slice(b" xmlns=\"");
    } else {
        s.extend_from_slice(b" xmlns:");
        s.extend_from_slice(prefixo.as_bytes());
        s.extend_from_slice(b"=\"");
    }
    escapar_atributo(uri, s);
    s.push(b'"');
}

fn escapar_texto(t: &str, s: &mut Vec<u8>) {
    for c in t.chars() {
        match c {
            '&' => s.extend_from_slice(b"&amp;"),
            '<' => s.extend_from_slice(b"&lt;"),
            '>' => s.extend_from_slice(b"&gt;"),
            '\r' => s.extend_from_slice(b"&#xD;"),
            _ => {
                let mut b = [0u8; 4];
                s.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
            }
        }
    }
}

fn escapar_atributo(t: &str, s: &mut Vec<u8>) {
    for c in t.chars() {
        match c {
            '&' => s.extend_from_slice(b"&amp;"),
            '<' => s.extend_from_slice(b"&lt;"),
            '"' => s.extend_from_slice(b"&quot;"),
            '\t' => s.extend_from_slice(b"&#x9;"),
            '\n' => s.extend_from_slice(b"&#xA;"),
            '\r' => s.extend_from_slice(b"&#xD;"),
            _ => {
                let mut b = [0u8; 4];
                s.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
            }
        }
    }
}
