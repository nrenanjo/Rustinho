//! T010 (0.9.19, etapa 3; requisito §6, decisão do proprietário de 25/11/2026): a descoberta dos dados fiscais
//! nas notas que a própria loja já emitiu.
//!
//! Lê só XML **autorizados** (`protNFe` com cStat 100 ou 150) e devolve cada dado com a nota de onde veio. Nada é
//! gravado aqui: a tela mostra e pede confirmação item por item. Não usa o certificado (só lê arquivos).
//!
//! Campos (MOC 7.0, Anexo I): `emit/CNPJ` (C02), `emit/IE` (C17), `emit/enderEmit/UF` (C12), `emit/CRT` (C21),
//! `ide/mod` (B06), `ide/serie` (B07), `ide/nNF` (B08), `ide/tpAmb` (B24). O identificador do CSC vem do QR Code da
//! NFC-e (`infNFeSupl/qrCode`), versão 2: o 4º parâmetro na emissão online
//! (`p=<chave>|2|<tpAmb>|<identificador_csc>|<hash>`) e o 7º na off-line (Manual do DANFE NFC-e e QR Code v6.0,
//! tabelas 2 e 3). **O CSC (o segredo) não está no XML** e continua pedido ao proprietário.

use std::collections::BTreeMap;

use serde::Serialize;

/// Um dado achado: o valor e a nota de onde veio (a mais recente que o traz).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Achado {
    pub valor: String,
    pub chave_da_nota: String,
    /// Outros valores vistos em outras notas (o dado mudou com o tempo: a tela pergunta qual vale).
    pub outros: Vec<String>,
}

/// A série de um modelo num ambiente, com o maior número já autorizado.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Serie {
    pub ambiente: i64,
    pub modelo: i64,
    pub serie: i64,
    pub ultimo_numero: i64,
    pub notas: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Descoberta {
    pub cnpj: Option<Achado>,
    pub ie: Option<Achado>,
    pub uf: Option<Achado>,
    /// 1 Simples Nacional, 2 excesso de sublimite, 3 Regime Normal (4 MEI, NT 2025.002).
    pub crt: Option<Achado>,
    pub identificador_csc: Option<Achado>,
    pub series: Vec<Serie>,
    /// Quantos XML foram lidos como notas autorizadas da loja.
    pub lidas: i64,
    /// Arquivos deixados de lado, com o motivo (não autorizado, não é NF-e/NFC-e, de outro emitente...).
    pub ignorados: Vec<String>,
}

struct Nota {
    chave: String,
    cnpj: String,
    ie: String,
    uf: String,
    crt: String,
    modelo: i64,
    serie: i64,
    numero: i64,
    ambiente: i64,
    emissao: String,
    id_csc: String,
}

fn texto_de(no: Option<roxmltree::Node>) -> String {
    no.and_then(|n| n.text()).unwrap_or("").trim().to_string()
}

fn filho<'a, 'b>(no: roxmltree::Node<'a, 'b>, nome: &str) -> Option<roxmltree::Node<'a, 'b>> {
    no.children().find(|n| n.is_element() && n.tag_name().name() == nome)
}

fn descendente<'a, 'b>(no: roxmltree::Node<'a, 'b>, nome: &str) -> Option<roxmltree::Node<'a, 'b>> {
    no.descendants().find(|n| n.is_element() && n.tag_name().name() == nome)
}

/// O identificador do CSC no QR Code v2 (o 4º parâmetro online; o 7º off-line). Vazio se não houver.
pub fn identificador_csc_do_qrcode(qr: &str) -> String {
    let Some(p) = qr.split("p=").nth(1) else { return String::new() };
    let partes: Vec<&str> = p.split('|').collect();
    let campo = match partes.len() {
        5 => partes[3],
        8 => partes[6],
        _ => "",
    };
    campo.trim().trim_start_matches('0').to_string()
}

fn ler_nota(xml: &str) -> Result<Nota, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|_| "não é um XML válido".to_string())?;
    let raiz = doc.root_element();
    let inf = descendente(raiz, "infNFe").ok_or("não é uma NF-e nem NFC-e")?;
    let cstat = descendente(raiz, "protNFe").and_then(|p| descendente(p, "cStat")).and_then(|n| n.text()).unwrap_or("");
    if !matches!(cstat.trim(), "100" | "150") {
        return Err("não está autorizada (sem protocolo de autorização)".into());
    }
    let ide = filho(inf, "ide").ok_or("sem o grupo ide")?;
    let emit = filho(inf, "emit").ok_or("sem o emitente")?;
    let num = |n: &str| texto_de(filho(ide, n)).parse::<i64>().map_err(|_| format!("{n} inválido"));
    let chave = inf.attribute("Id").unwrap_or("").trim_start_matches("NFe").to_string();
    let qr = descendente(raiz, "qrCode").and_then(|n| n.text()).unwrap_or("");
    Ok(Nota {
        chave,
        cnpj: texto_de(filho(emit, "CNPJ")),
        ie: texto_de(filho(emit, "IE")),
        uf: texto_de(filho(emit, "enderEmit").and_then(|e| filho(e, "UF"))),
        crt: texto_de(filho(emit, "CRT")),
        modelo: num("mod")?,
        serie: num("serie")?,
        numero: num("nNF")?,
        ambiente: num("tpAmb")?,
        emissao: texto_de(filho(ide, "dhEmi")).chars().take(10).collect(),
        id_csc: identificador_csc_do_qrcode(qr),
    })
}

/// Junta os valores de um campo: o da nota mais recente vale, e os diferentes ficam em `outros`.
fn achado(notas: &[&Nota], campo: impl Fn(&Nota) -> &str) -> Option<Achado> {
    let mut com: Vec<&&Nota> = notas.iter().filter(|n| !campo(n).is_empty()).collect();
    com.sort_by(|a, b| b.emissao.cmp(&a.emissao).then(b.numero.cmp(&a.numero)));
    let primeira = com.first()?;
    let valor = campo(primeira).to_string();
    let mut outros: Vec<String> = com.iter().map(|n| campo(n).to_string()).filter(|v| *v != valor).collect();
    outros.sort();
    outros.dedup();
    Some(Achado { valor, chave_da_nota: primeira.chave.clone(), outros })
}

/// Lê os XML (nome do arquivo, conteúdo). `cnpj_da_loja`: se informado, só as notas desse emitente contam; vazio,
/// o emitente mais frequente é tomado como a loja, e os outros ficam em `ignorados`.
pub fn descobrir_de_xml(arquivos: &[(String, String)], cnpj_da_loja: &str) -> Descoberta {
    let mut d = Descoberta::default();
    let mut notas = Vec::new();
    for (nome, conteudo) in arquivos {
        match ler_nota(conteudo) {
            Ok(n) => notas.push(n),
            Err(m) => d.ignorados.push(format!("{nome}: {m}")),
        }
    }
    let loja = if !cnpj_da_loja.trim().is_empty() {
        crate::documento::digitos(cnpj_da_loja)
    } else {
        let mut conta: BTreeMap<&str, usize> = BTreeMap::new();
        for n in &notas {
            *conta.entry(n.cnpj.as_str()).or_default() += 1;
        }
        conta.into_iter().max_by_key(|(_, q)| *q).map(|(c, _)| c.to_string()).unwrap_or_default()
    };
    let (da_loja, outras): (Vec<&Nota>, Vec<&Nota>) =
        notas.iter().partition(|n| crate::documento::digitos(&n.cnpj) == loja);
    for n in outras {
        d.ignorados.push(format!("{}: é de outro emitente ({})", n.chave, n.cnpj));
    }
    d.lidas = da_loja.len() as i64;
    d.cnpj = achado(&da_loja, |n| n.cnpj.as_str());
    d.ie = achado(&da_loja, |n| n.ie.as_str());
    d.uf = achado(&da_loja, |n| n.uf.as_str());
    d.crt = achado(&da_loja, |n| n.crt.as_str());
    d.identificador_csc = achado(&da_loja, |n| n.id_csc.as_str());
    let mut series: BTreeMap<(i64, i64, i64), (i64, i64)> = BTreeMap::new();
    for n in &da_loja {
        let e = series.entry((n.ambiente, n.modelo, n.serie)).or_default();
        e.0 = e.0.max(n.numero);
        e.1 += 1;
    }
    d.series = series
        .into_iter()
        .map(|((ambiente, modelo, serie), (ultimo_numero, notas))| Serie {
            ambiente,
            modelo,
            serie,
            ultimo_numero,
            notas,
        })
        .collect();
    d
}

#[cfg(test)]
mod testes {
    use super::*;

    const EXEMPLO: &str = include_str!("../../tests/nfe_exemplo.xml");

    fn nfce(numero: i64, serie: i64, qr: &str, crt: &str) -> String {
        format!(
            r#"<nfeProc xmlns="http://www.portalfiscal.inf.br/nfe" versao="4.00"><NFe><infNFe Id="NFe3526111122233300018165{serie:03}{numero:09}1000000019" versao="4.00">
<ide><cUF>35</cUF><mod>65</mod><serie>{serie}</serie><nNF>{numero}</nNF><dhEmi>2026-11-2{d}T10:00:00-03:00</dhEmi><tpAmb>1</tpAmb></ide>
<emit><CNPJ>11222333000181</CNPJ><enderEmit><UF>SP</UF></enderEmit><IE>123456789110</IE><CRT>{crt}</CRT></emit>
</infNFe><infNFeSupl><qrCode>https://www.homologacao.nfce.fazenda.sp.gov.br/qrcode?p={chave}|2|1|{qr}|ABCDEF</qrCode></infNFeSupl></NFe>
<protNFe versao="4.00"><infProt><cStat>100</cStat></infProt></protNFe></nfeProc>"#,
            d = numero % 9,
            chave = "3".repeat(44),
        )
    }

    #[test]
    fn t010_le_cnpj_ie_uf_crt_series_e_o_identificador_do_csc() {
        let arquivos = vec![
            ("a.xml".to_string(), EXEMPLO.to_string()),
            ("b.xml".to_string(), nfce(41, 1, "000001", "1")),
            ("c.xml".to_string(), nfce(42, 1, "000001", "1")),
            ("d.xml".to_string(), "<nao/>".to_string()),
        ];
        let d = descobrir_de_xml(&arquivos, "11.222.333/0001-81");
        assert_eq!(d.lidas, 3);
        assert_eq!(d.cnpj.as_ref().unwrap().valor, "11222333000181");
        assert_eq!(d.uf.as_ref().unwrap().valor, "SP");
        assert_eq!(d.ie.as_ref().unwrap().valor, "123456789110");
        // O CRT mudou: as NFC-e mais novas dizem 1; a nota de exemplo (mais antiga) diz 3.
        let crt = d.crt.as_ref().unwrap();
        assert_eq!((crt.valor.as_str(), crt.outros.clone()), ("1", vec!["3".to_string()]));
        assert_eq!(d.identificador_csc.as_ref().unwrap().valor, "1", "o identificador, não o CSC");
        assert!(d.series.contains(&Serie { ambiente: 1, modelo: 65, serie: 1, ultimo_numero: 42, notas: 2 }));
        assert!(d.series.iter().any(|s| s.modelo == 55 && s.ultimo_numero == 12345));
        assert_eq!(d.ignorados.len(), 1);
    }

    #[test]
    fn t010_so_notas_autorizadas_e_da_loja() {
        let nao_autorizada = nfce(43, 1, "000001", "1").replace("<cStat>100</cStat>", "<cStat>225</cStat>");
        let outra = nfce(44, 1, "000001", "1").replace("11222333000181", "99888777000166");
        let d = descobrir_de_xml(
            &[("x.xml".into(), nao_autorizada), ("y.xml".into(), outra), ("z.xml".into(), nfce(45, 2, "000002", "1"))],
            "11222333000181",
        );
        assert_eq!(d.lidas, 1);
        assert_eq!(d.series, vec![Serie { ambiente: 1, modelo: 65, serie: 2, ultimo_numero: 45, notas: 1 }]);
        assert!(d.ignorados.iter().any(|i| i.contains("não está autorizada")));
        assert!(d.ignorados.iter().any(|i| i.contains("outro emitente")));
    }

    #[test]
    fn t010_identificador_do_csc_online_e_offline() {
        assert_eq!(identificador_csc_do_qrcode("https://x/qrcode?p=CHAVE|2|1|000003|HASH"), "3");
        assert_eq!(identificador_csc_do_qrcode("https://x/qrcode?p=CHAVE|2|1|25|10.00|DIGVAL|000004|HASH"), "4");
        assert_eq!(identificador_csc_do_qrcode("sem parametro"), "");
    }
}
