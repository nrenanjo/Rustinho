//! Leitura do XML da NF-e (nota de compra que o fornecedor manda).
//!
//! Lê só o que a entrada no estoque precisa: emitente, número, itens e
//! totais. Aceita o arquivo "procNFe" (nota autorizada, o mais comum) ou só
//! a "NFe".

use roxmltree::{Document, Node};

use crate::pessoas::Emitente;

/// Uma nota lida do XML.
#[derive(Debug, Default, Clone)]
pub struct NotaXml {
    pub chave: String,
    pub modelo: String,
    pub numero: String,
    pub serie: String,
    /// "AAAA-MM-DD"
    pub emissao: String,
    pub natureza: String,
    /// 0 = entrada, 1 = saída (para o emitente)
    pub tipo: String,
    /// finNFe: 1 normal, 2 complementar, 3 ajuste, 4 devolução
    pub finalidade: String,
    pub emitente: Emitente,
    /// CNPJ/CPF de quem recebeu a nota
    pub destinatario: String,
    /// Nome, endereço e contato de quem recebeu (para cadastrar o cliente).
    pub destinatario_dados: Emitente,
    pub autorizada: bool,
    pub itens: Vec<ItemXml>,
    pub total_produtos: i64,
    pub frete: i64,
    pub seguro: i64,
    pub desconto: i64,
    pub outras: i64,
    pub ipi: i64,
    pub icms_st: i64,
    pub total: i64,
    /// Parcelas da nota (grupo cobr/dup), se o fornecedor vendeu a prazo.
    pub duplicatas: Vec<Duplicata>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Duplicata {
    pub numero: String,
    /// "AAAA-MM-DD"
    pub vencimento: String,
    pub valor: i64,
}

#[derive(Debug, Default, Clone)]
pub struct ItemXml {
    pub codigo: String,
    pub gtin: String,
    pub descricao: String,
    pub ncm: String,
    pub cest: String,
    pub cfop: String,
    pub unidade: String,
    /// em milésimos
    pub quantidade: i64,
    pub valor_produtos: i64,
    pub frete: i64,
    pub seguro: i64,
    pub desconto: i64,
    pub outras: i64,
    pub ipi: i64,
    pub icms_st: i64,
    pub origem: i64,
    pub cst_icms: String,
    pub cst_pis: String,
}

fn filho<'a, 'i>(no: Node<'a, 'i>, nome: &str) -> Option<Node<'a, 'i>> {
    no.children().find(|n| n.is_element() && n.tag_name().name() == nome)
}

fn descendente<'a, 'i>(no: Node<'a, 'i>, nome: &str) -> Option<Node<'a, 'i>> {
    no.descendants().find(|n| n.is_element() && n.tag_name().name() == nome)
}

fn texto(no: Option<Node>, nome: &str) -> String {
    no.and_then(|n| filho(n, nome)).and_then(|n| n.text()).map(|t| t.trim().to_string()).unwrap_or_default()
}

/// "123.4567" → valor inteiro com `casas` casas decimais (arredondado).
fn decimal(texto: &str, casas: u32) -> Result<i64, String> {
    let t = texto.trim();
    if t.is_empty() {
        return Ok(0);
    }
    let negativo = t.starts_with('-');
    let t = t.trim_start_matches(['-', '+']);
    let (inteira, fracao) = t.split_once('.').unwrap_or((t, ""));
    if inteira.is_empty() && fracao.is_empty()
        || !inteira.chars().all(|c| c.is_ascii_digit())
        || !fracao.chars().all(|c| c.is_ascii_digit())
    {
        return Err(format!("número inválido no XML: {texto}"));
    }
    let base = 10_i64.pow(casas);
    let inteira: i64 =
        if inteira.is_empty() { 0 } else { inteira.parse().map_err(|_| format!("número grande demais: {texto}"))? };
    let mut f: String = fracao.chars().take(casas as usize + 1).collect();
    while f.len() < casas as usize + 1 {
        f.push('0');
    }
    let f: i64 = f.parse().unwrap_or(0);
    let arredondado = (f + 5) / 10; // meia unidade para cima
    let valor = inteira
        .checked_mul(base)
        .and_then(|v| v.checked_add(arredondado))
        .ok_or_else(|| format!("número grande demais: {texto}"))?;
    Ok(if negativo { -valor } else { valor })
}

fn dinheiro(no: Option<Node>, nome: &str) -> Result<i64, String> {
    decimal(&texto(no, nome), 2)
}

/// Lê o XML. Devolve uma mensagem clara quando não é uma NF-e.
pub fn ler(xml: &str) -> Result<NotaXml, String> {
    ler_nota(xml, false)
}

/// Lê o XML de uma NF-e ou NFC-e para cadastrar clientes, fornecedores e
/// peças (aceita também o cupom, modelo 65).
pub fn ler_cadastro(xml: &str) -> Result<NotaXml, String> {
    ler_nota(xml, true)
}

fn pessoa_do_grupo(grupo: Option<Node>, endereco: &str) -> Emitente {
    let ender = grupo.and_then(|e| filho(e, endereco));
    let doc = {
        let c = texto(grupo, "CNPJ");
        if c.is_empty() {
            texto(grupo, "CPF")
        } else {
            c
        }
    };
    Emitente {
        documento: doc,
        nome: texto(grupo, "xNome"),
        fantasia: texto(grupo, "xFant"),
        ie: texto(grupo, "IE"),
        email: texto(grupo, "email"),
        telefone: texto(ender, "fone"),
        cep: texto(ender, "CEP"),
        logradouro: texto(ender, "xLgr"),
        numero: texto(ender, "nro"),
        complemento: texto(ender, "xCpl"),
        bairro: texto(ender, "xBairro"),
        municipio: texto(ender, "xMun"),
        municipio_ibge: texto(ender, "cMun"),
        uf: texto(ender, "UF"),
    }
}

fn ler_nota(xml: &str, aceita_nfce: bool) -> Result<NotaXml, String> {
    // Alguns arquivos vêm com BOM ou espaços antes da declaração.
    let xml = xml.trim_start_matches('\u{feff}').trim_start();
    let doc = Document::parse(xml).map_err(|e| format!("O arquivo não é um XML válido ({e})."))?;
    let raiz = doc.root_element();
    let inf = descendente(raiz, "infNFe").ok_or_else(|| {
        let nome = raiz.tag_name().name();
        if nome.contains("resNFe") || nome.contains("procEvento") || nome.contains("evento") {
            "Este XML é um resumo ou um evento, não a nota completa. Baixe o XML completo da NF-e.".to_string()
        } else if descendente(raiz, "infCte").is_some() {
            "Este XML é de um CT-e (frete), não de uma NF-e. Digite o valor do frete na compra.".to_string()
        } else {
            "Este XML não é de uma NF-e.".to_string()
        }
    })?;

    let ide = filho(inf, "ide");
    let modelo = texto(ide, "mod");
    if modelo == "65" && !aceita_nfce {
        return Err("Este XML é de uma NFC-e (cupom de consumidor). A entrada de compra usa NF-e (modelo 55).".into());
    }
    let id = inf.attribute("Id").unwrap_or_default();
    let chave: String = id.chars().filter(|c| c.is_ascii_digit()).collect();
    let emissao = {
        let dh = texto(ide, "dhEmi");
        let dh = if dh.is_empty() { texto(ide, "dEmi") } else { dh };
        dh.chars().take(10).collect::<String>()
    };

    let emitente = pessoa_do_grupo(filho(inf, "emit"), "enderEmit");
    let destinatario_dados = pessoa_do_grupo(filho(inf, "dest"), "enderDest");
    let destinatario = destinatario_dados.documento.clone();

    let mut itens = Vec::new();
    for det in inf.children().filter(|n| n.is_element() && n.tag_name().name() == "det") {
        let prod = filho(det, "prod");
        let imposto = filho(det, "imposto");
        // ICMS: o grupo tem nome variável (ICMS00, ICMSSN500...); pega o primeiro.
        let icms = imposto.and_then(|i| filho(i, "ICMS")).and_then(|i| i.children().find(|n| n.is_element()));
        let cst_icms = {
            let c = texto(icms, "CSOSN");
            if c.is_empty() {
                texto(icms, "CST")
            } else {
                c
            }
        };
        let icms_st = dinheiro(icms, "vICMSST")? + dinheiro(icms, "vFCPST")?;
        let ipi = imposto.and_then(|i| filho(i, "IPI")).and_then(|i| filho(i, "IPITrib"));
        let pis = imposto.and_then(|i| filho(i, "PIS")).and_then(|i| i.children().find(|n| n.is_element()));
        let gtin = {
            let g = texto(prod, "cEAN");
            if g.chars().all(|c| c.is_ascii_digit()) {
                g
            } else {
                String::new()
            }
        };
        let cfop = texto(prod, "CFOP");
        let item = ItemXml {
            codigo: texto(prod, "cProd"),
            gtin,
            descricao: texto(prod, "xProd"),
            ncm: texto(prod, "NCM"),
            cest: texto(prod, "CEST"),
            cfop,
            unidade: texto(prod, "uCom").to_uppercase(),
            quantidade: decimal(&texto(prod, "qCom"), 3)?,
            valor_produtos: dinheiro(prod, "vProd")?,
            frete: dinheiro(prod, "vFrete")?,
            seguro: dinheiro(prod, "vSeg")?,
            desconto: dinheiro(prod, "vDesc")?,
            outras: dinheiro(prod, "vOutro")?,
            ipi: dinheiro(ipi, "vIPI")?,
            icms_st,
            origem: texto(icms, "orig").parse().unwrap_or(0),
            cst_icms,
            cst_pis: texto(pis, "CST"),
        };
        if item.quantidade <= 0 {
            return Err(format!("O item \"{}\" está com quantidade zero no XML.", item.descricao));
        }
        itens.push(item);
    }
    if itens.is_empty() {
        return Err("A nota não tem itens.".into());
    }

    let mut duplicatas = Vec::new();
    if let Some(cobr) = filho(inf, "cobr") {
        for dup in cobr.children().filter(|n| n.is_element() && n.tag_name().name() == "dup") {
            let valor = dinheiro(Some(dup), "vDup")?;
            let vencimento: String = texto(Some(dup), "dVenc").chars().take(10).collect();
            if valor > 0 && vencimento.len() == 10 {
                duplicatas.push(Duplicata { numero: texto(Some(dup), "nDup"), vencimento, valor });
            }
        }
    }

    let tot = filho(inf, "total").and_then(|t| filho(t, "ICMSTot"));
    let autorizada = descendente(raiz, "protNFe")
        .and_then(|p| descendente(p, "cStat"))
        .and_then(|n| n.text())
        .map(|c| matches!(c.trim(), "100" | "150"))
        .unwrap_or(false);
    Ok(NotaXml {
        chave,
        modelo,
        numero: texto(ide, "nNF"),
        serie: texto(ide, "serie"),
        emissao,
        natureza: texto(ide, "natOp"),
        tipo: texto(ide, "tpNF"),
        finalidade: texto(ide, "finNFe"),
        emitente,
        destinatario,
        destinatario_dados,
        autorizada,
        itens,
        total_produtos: dinheiro(tot, "vProd")?,
        frete: dinheiro(tot, "vFrete")?,
        seguro: dinheiro(tot, "vSeg")?,
        desconto: dinheiro(tot, "vDesc")?,
        outras: dinheiro(tot, "vOutro")?,
        ipi: dinheiro(tot, "vIPI")?,
        icms_st: dinheiro(tot, "vST")? + dinheiro(tot, "vFCPST")?,
        total: dinheiro(tot, "vNF")?,
        duplicatas,
    })
}

/// XML de exemplo para os testes (uma nota com 3 itens).
#[cfg(test)]
pub const EXEMPLO: &str = include_str!("../tests/nfe_exemplo.xml");

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn decimais() {
        assert_eq!(decimal("123.45", 2).unwrap(), 12345);
        assert_eq!(decimal("10", 2).unwrap(), 1000);
        assert_eq!(decimal("0.005", 2).unwrap(), 1);
        assert_eq!(decimal("2.0000", 3).unwrap(), 2000);
        assert_eq!(decimal("1.2345", 3).unwrap(), 1235);
        assert!(decimal("1,5", 2).is_err());
    }

    #[test]
    fn le_nota_de_exemplo() {
        let n = ler(EXEMPLO).unwrap();
        assert_eq!(n.chave.len(), 44);
        assert_eq!(n.numero, "12345");
        assert_eq!(n.emissao, "2026-10-08");
        assert_eq!(n.emitente.documento, "11222333000181");
        assert_eq!(n.emitente.uf, "SP");
        assert!(n.autorizada);
        assert_eq!(n.itens.len(), 3);
        assert_eq!(n.itens[0].quantidade, 10_000);
        assert_eq!(n.itens[0].gtin, "7891234567895");
        assert_eq!(n.itens[1].gtin, "");
        assert_eq!(n.itens[2].cst_pis, "04");
        assert_eq!(n.total, 1_10050);
        assert_eq!(n.duplicatas.len(), 2);
        assert_eq!(n.duplicatas[0], Duplicata { numero: "001".into(), vencimento: "2026-11-07".into(), valor: 55_025 });
        assert_eq!(n.duplicatas[1].vencimento, "2026-12-07");
    }

    #[test]
    fn recusa_o_que_nao_e_nfe() {
        assert!(ler("isto não é xml").is_err());
        assert!(ler("<cteProc><CTe><infCte/></CTe></cteProc>").unwrap_err().contains("CT-e"));
        assert!(ler("<resNFe><chNFe>1</chNFe></resNFe>").unwrap_err().contains("resumo"));
    }
}
