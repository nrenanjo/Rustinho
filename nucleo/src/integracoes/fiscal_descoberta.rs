//! T010 (0.9.19, etapa 3; requisito §6, decisão do proprietário de 25/11/2026): a descoberta dos dados fiscais
//! nas notas que a própria loja já emitiu.
//!
//! Lê os XML **autorizados** (`protNFe` com cStat 100, 120 ou 150) e devolve cada dado com a nota de onde veio; a
//! nota **denegada** (110, 301, 302, 303) entra só no último número da série, porque gastou o número. Nada é
//! gravado aqui: a tela mostra e pede confirmação item por item. Não usa o certificado (só lê arquivos).
//!
//! Campos (MOC 7.0, Anexo I): `emit/CNPJ` (C02), `emit/IE` (C17), `emit/enderEmit/UF` (C12), `emit/CRT` (C21),
//! `ide/mod` (B06), `ide/serie` (B07), `ide/nNF` (B08), `ide/tpAmb` (B24). O identificador do CSC vem do QR Code da
//! NFC-e (`infNFeSupl/qrCode`), **só na versão 2**: o 4º parâmetro na emissão online
//! (`p=<chave>|2|<tpAmb>|<identificador_csc>|<hash>`) e o 7º na off-line (Manual do DANFE NFC-e e QR Code v6.0,
//! tabelas 2 e 3). **O CSC (o segredo) não está no XML** e continua pedido ao proprietário. Na versão 3 (item 4.4 do
//! manual: `p=<chave>|3|<tpAmb>` online; 8 campos off-line, o 7º é o documento do destinatário) não há CSC: a
//! autenticidade vem da assinatura do próprio QR Code (C-M8 da R1).
//!
//! Sem o CNPJ da loja informado e com mais de um emitente na pasta (a pasta pode ter notas de compra), a descoberta
//! não escolhe: devolve os candidatos e a tela pergunta qual é a loja.

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

/// A série de um modelo num ambiente, com o maior número já usado (autorizado ou denegado) e quantas notas
/// autorizadas.
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
    /// Alguma nota da loja usa o QR Code versão 3 (sem CSC).
    pub qr_versao_3: bool,
    pub series: Vec<Serie>,
    /// Sem o CNPJ da loja e com mais de um emitente: os emitentes achados (CNPJ, quantas autorizadas e quantas
    /// denegadas), para a tela
    /// perguntar qual é a loja. Enquanto isso, nenhum dado sai.
    pub candidatos: Vec<Candidato>,
    /// Quantos XML foram lidos como notas autorizadas da loja.
    pub lidas: i64,
    /// Arquivos deixados de lado, com o motivo (não autorizado, não é NF-e/NFC-e, de outro emitente...).
    pub ignorados: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Candidato {
    pub cnpj: String,
    /// As autorizadas (C-6 da R6).
    pub notas: i64,
    /// N-3 da R8: as denegadas, para a tela não mostrar "0 notas" de um emitente que só tem denegadas.
    pub denegadas: i64,
}

struct Nota {
    /// C-8 da R5: denegada (o número foi gasto): conta só no piso da série.
    denegada: bool,
    qr_versao: String,
    chave: String,
    cnpj: String,
    /// B-3 da R12: o emitente tem CPF (e não CNPJ): o motivo exato quando a nota fica de lado.
    com_cpf: bool,
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

/// A versão do QR Code da NFC-e (o 2º parâmetro), ou vazio.
pub fn versao_do_qrcode(qr: &str) -> String {
    qr.split("p=").nth(1).and_then(|p| p.split('|').nth(1)).unwrap_or("").trim().to_string()
}

/// O identificador do CSC no QR Code **v2** (o 4º parâmetro online; o 7º off-line). Vazio se não houver, ou se o QR
/// for de outra versão (na v3, o 7º off-line é o documento do destinatário).
pub fn identificador_csc_do_qrcode(qr: &str) -> String {
    let Some(p) = qr.split("p=").nth(1) else { return String::new() };
    let partes: Vec<&str> = p.split('|').collect();
    if partes.get(1).map(|v| v.trim()) != Some("2") {
        return String::new();
    }
    let campo = match partes.len() {
        5 => partes[3],
        8 => partes[6],
        _ => "",
    };
    campo.trim().trim_start_matches('0').to_string()
}

fn ler_nota(xml: &str) -> Result<Nota, String> {
    // B-2 da R4 do BUG-20261128-XDCL: a mesma proteção do verificador (o leitor é recursivo, e um arquivo hostil
    // profundo demais derrubaria o programa).
    if crate::nfe_assinatura::profundidade_excessiva(xml, crate::nfe_assinatura::PROFUNDIDADE_MAXIMA) {
        return Err("não é um XML válido".to_string());
    }
    let doc = roxmltree::Document::parse(xml).map_err(|_| "não é um XML válido".to_string())?;
    let raiz = doc.root_element();
    let inf = descendente(raiz, "infNFe").ok_or("não é uma NF-e nem NFC-e")?;
    let cstat = descendente(raiz, "protNFe").and_then(|p| descendente(p, "cStat")).and_then(|n| n.text()).unwrap_or("");
    // N-1 da R4: 120 é "Autorizado o uso da NF-e, com alerta" (NT 2026.002): a nota vale, e o número dela conta no piso.
    // C-8 da R5: a denegada (110, 301, 302, 303) gasta o número: entra só no piso da numeração.
    let denegada = crate::nfe_xml::cstat_denegado(cstat);
    if !crate::nfe_xml::cstat_autorizado(cstat) && !denegada {
        return Err("não está autorizada (sem protocolo de autorização)".into());
    }
    let ide = filho(inf, "ide").ok_or("sem o grupo ide")?;
    let emit = filho(inf, "emit").ok_or("sem o emitente")?;
    let num = |n: &str| texto_de(filho(ide, n)).parse::<i64>().map_err(|_| format!("{n} inválido"));
    let chave = inf.attribute("Id").unwrap_or("").trim_start_matches("NFe").to_string();
    let qr = descendente(raiz, "qrCode").and_then(|n| n.text()).unwrap_or("");
    Ok(Nota {
        denegada,
        qr_versao: versao_do_qrcode(qr),
        chave,
        cnpj: texto_de(filho(emit, "CNPJ")),
        com_cpf: !crate::documento::limpar(&texto_de(filho(emit, "CPF"))).is_empty(),
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

/// Lê os XML (nome do arquivo, conteúdo). `cnpj_da_loja`: se informado, só as notas desse emitente contam. Vazio:
/// com um emitente só, ele é a loja; com mais de um, nenhum dado sai e os emitentes vão em `candidatos` (a tela
/// pergunta qual é a loja e lê de novo com o escolhido).
pub fn descobrir_de_xml(arquivos: &[(String, String)], cnpj_da_loja: &str) -> Descoberta {
    let mut d = Descoberta::default();
    let mut notas = Vec::new();
    for (nome, conteudo) in arquivos {
        match ler_nota(conteudo) {
            // B-3 da R10: a nota sem o CNPJ do emitente (por exemplo, emitida com CPF) não é da loja e não vira
            // candidato com o CNPJ vazio ("É o da loja" com o CNPJ vazio lia de novo sem escolha: a pergunta voltava,
            // num laço).
            Ok(n) if crate::documento::limpar(&n.cnpj).is_empty() => {
                // B-1 da R11: o Rustinho só emite com o CNPJ da loja (a loja com CPF é recusada antes da SEFAZ,
                // `nota_fiscal::pode_falar_com_a_sefaz`): a nota de CPF fica de lado, com o motivo exato.
                d.ignorados.push(if n.com_cpf {
                    format!("{nome}: sem o CNPJ do emitente (nota de CPF; o Rustinho emite só com o CNPJ da loja)")
                } else {
                    // B-3 da R12: sem CNPJ nem CPF no emitente, não se afirma "nota de CPF".
                    format!("{nome}: sem o CNPJ do emitente (o Rustinho emite só com o CNPJ da loja)")
                })
            }
            Ok(n) => notas.push(n),
            Err(m) => d.ignorados.push(format!("{nome}: {m}")),
        }
    }
    let loja = if !cnpj_da_loja.trim().is_empty() {
        // F3-3 da R3: `limpar` (o CNPJ alfanumérico tem letras).
        crate::documento::limpar(cnpj_da_loja)
    } else {
        let mut conta: BTreeMap<String, i64> = BTreeMap::new();
        // C-6 da R6: conta só as autorizadas (a tela diz "N notas autorizadas lidas").
        for n in notas.iter().filter(|n| !n.denegada) {
            *conta.entry(crate::documento::limpar(&n.cnpj)).or_default() += 1;
        }
        // B-4 da R7 e E6 da R8 (T030): todo emitente da pasta é candidato, também o que só tem denegadas (com 0
        // autorizadas). Só com um emitente ele é a loja; com mais de um, a tela pergunta. Antes, o emitente só de
        // denegadas ficava de fora quando havia autorizadas de outro: uma denegada da loja com notas de compra de um
        // fornecedor fazia o fornecedor virar "a loja" sem perguntar.
        for n in &notas {
            conta.entry(crate::documento::limpar(&n.cnpj)).or_default();
        }
        if conta.len() > 1 {
            let denegadas = |cnpj: &str| {
                notas.iter().filter(|n| n.denegada && crate::documento::limpar(&n.cnpj) == cnpj).count() as i64
            };
            let mut c: Vec<Candidato> =
                conta.into_iter().map(|(cnpj, notas)| Candidato { denegadas: denegadas(&cnpj), cnpj, notas }).collect();
            c.sort_by(|a, b| b.notas.cmp(&a.notas).then(a.cnpj.cmp(&b.cnpj)));
            d.candidatos = c;
            return d;
        }
        conta.into_keys().next().unwrap_or_default()
    };
    let (da_loja, outras): (Vec<&Nota>, Vec<&Nota>) =
        notas.iter().partition(|n| crate::documento::limpar(&n.cnpj) == loja);
    for n in outras {
        d.ignorados.push(format!("{}: é de outro emitente ({})", n.chave, n.cnpj));
    }
    // C-8 da R5: os dados saem só das autorizadas; a denegada entra só no piso da série (o número dela foi gasto).
    let autorizadas: Vec<&Nota> = da_loja.iter().copied().filter(|n| !n.denegada).collect();
    d.lidas = autorizadas.len() as i64;
    d.cnpj = achado(&autorizadas, |n| n.cnpj.as_str());
    d.ie = achado(&autorizadas, |n| n.ie.as_str());
    d.uf = achado(&autorizadas, |n| n.uf.as_str());
    d.crt = achado(&autorizadas, |n| n.crt.as_str());
    d.identificador_csc = achado(&autorizadas, |n| n.id_csc.as_str());
    d.qr_versao_3 = autorizadas.iter().any(|n| n.qr_versao == "3");
    let mut series: BTreeMap<(i64, i64, i64), (i64, i64)> = BTreeMap::new();
    for n in &da_loja {
        let e = series.entry((n.ambiente, n.modelo, n.serie)).or_default();
        e.0 = e.0.max(n.numero);
        if !n.denegada {
            e.1 += 1;
        }
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

/// O que a tela pode gravar com o "Usar este" (chaves só deste computador). O CNPJ, a UF e a IE não: eles ficam em
/// Configurações › Loja (a tela só confere e aponta para lá).
pub fn chave_de(campo: &str) -> Option<&'static str> {
    match campo {
        "serieNfce" => Some("fiscal_serie_nfce"),
        "serieNfe" => Some("fiscal_serie_nfe"),
        "crt" => Some("fiscal_crt"),
        "identificadorCsc" => Some("fiscal_csc_id"),
        _ => None,
    }
}

/// Grava um dado confirmado na tela, com registro. Na série, `ultimo` traz o ambiente e o maior número já usado nela
/// (autorizado ou denegado, das notas lidas): vira o piso da numeração (`nota_fiscal::chave_do_piso`), para o Rustinho não
/// repetir um número que o sistema de antes já usou (C-M4 da R1). O piso só sobe.
pub fn usar(
    tx: &rusqlite::Connection,
    dispositivo: &str,
    usuario_id: &str,
    campo: &str,
    valor: &str,
    ultimo: Option<(i64, i64)>,
) -> crate::erro::Resultado<()> {
    let chave = chave_de(campo).ok_or_else(|| crate::erro::Erro::validacao("Este dado não se grava por aqui."))?;
    let valor: String = valor.trim().chars().take(20).collect();
    let valido = match campo {
        "serieNfce" | "serieNfe" => valor.parse::<i64>().is_ok_and(|n| (0..=999).contains(&n)),
        "crt" => matches!(valor.as_str(), "1" | "2" | "3" | "4"),
        _ => !valor.is_empty() && valor.chars().all(|c| c.is_ascii_digit()),
    };
    if !valido {
        return Err(crate::erro::Erro::validacao("Valor inválido para este dado."));
    }
    if matches!(campo, "serieNfce" | "serieNfe") {
        // C-M4 da R1: a série passa a ser deste computador (recusa a de outro).
        let modelo = if campo == "serieNfce" { 65 } else { 55 };
        crate::nota_fiscal::reservar_serie(tx, dispositivo, modelo, valor.parse().unwrap_or(0))?;
    }
    crate::config::gravar(tx, chave, &valor)?;
    let mut detalhes = vec![format!("Dado fiscal confirmado a partir das notas da loja: {campo} = {valor}")];
    if let (Some((ambiente, numero)), "serieNfce" | "serieNfe") = (ultimo, campo) {
        if !matches!(ambiente, 1 | 2) || !(0..=999_999_999).contains(&numero) {
            return Err(crate::erro::Erro::validacao("Numeração inválida para esta série."));
        }
        let modelo = if campo == "serieNfce" { 65 } else { 55 };
        let serie: i64 = valor.parse().unwrap_or(0);
        let piso = crate::nota_fiscal::chave_do_piso(ambiente, modelo, serie);
        let antes: i64 = crate::config::ler(tx, &piso)?.and_then(|v| v.trim().parse().ok()).unwrap_or(0);
        if numero > antes {
            crate::config::gravar(tx, &piso, &numero.to_string())?;
            detalhes.push(format!("A próxima nota desta série sai depois da {numero}"));
        }
    }
    crate::auditoria::registrar(tx, dispositivo, Some(usuario_id), "fiscal.descoberta", "config", chave, &detalhes)
}

pub use comandos::COMANDOS;

mod comandos {
    use serde::Deserialize;
    use serde_json::{json, Value};

    use crate::despacho::{ler, Classe, Comando, Pedido};
    use crate::erro::Resultado;

    pub const COMANDOS: &[Comando] = &[
        Comando {
            nome: "fiscal_descobrir",
            classe: Classe::ConsultaDoGerente("configuracoes"),
            funcao: fiscal_descobrir,
        },
        Comando {
            nome: "fiscal_descoberta_usar",
            classe: Classe::Gerente("configuracoes"),
            funcao: fiscal_descoberta_usar,
        },
    ];

    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Pasta {
        entradas: Vec<String>,
        /// O emitente que a tela confirmou ser a loja (quando a pasta tinha mais de um).
        cnpj: String,
    }

    fn fiscal_descobrir(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: Pasta = ler(dados)?;
        let achados = crate::arquivos::xmls(&d.entradas)?;
        let mut arquivos = Vec::new();
        let mut sem_leitura = Vec::new();
        for x in achados {
            if x.texto.is_empty() {
                sem_leitura.push(format!("{}: {}", x.nome, x.erro));
            } else {
                arquivos.push((x.nome, x.texto));
            }
        }
        let cnpj = crate::config::ler(p.conn, "loja_cnpj")?.unwrap_or_default();
        let escolhido = if cnpj.trim().is_empty() { d.cnpj.clone() } else { cnpj.clone() };
        let mut descoberta = super::descobrir_de_xml(&arquivos, &escolhido);
        descoberta.ignorados.extend(sem_leitura);
        Ok(json!({
            "descoberta": descoberta,
            "loja": {
                "cnpj": cnpj,
                "uf": crate::config::ler(p.conn, "loja_uf")?.unwrap_or_default(),
            },
        }))
    }

    #[derive(Deserialize, Default)]
    #[serde(default, rename_all = "camelCase")]
    struct Usar {
        campo: String,
        valor: String,
        /// Na série: o ambiente e o maior número já usado nela, autorizado ou denegado (0 = sem).
        ambiente: i64,
        ultimo_numero: i64,
    }

    fn fiscal_descoberta_usar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: Usar = ler(dados)?;
        let tx = p.conn.transaction()?;
        let ultimo = (d.ultimo_numero > 0).then_some((d.ambiente, d.ultimo_numero));
        super::usar(&tx, p.disp, p.usuario_id, &d.campo, &d.valor, ultimo)?;
        tx.commit()?;
        Ok(json!({ "ok": true }))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// B-2 da R4 do BUG-20261128-XDCL: um arquivo hostil profundo demais é recusado na descoberta fiscal, e não derruba
    /// o programa (o leitor é recursivo).
    #[test]
    fn xdcl_arquivo_profundo_demais_nao_derruba() {
        let profundo = format!("{}{}", "<NFe>".repeat(200_000), "</NFe>".repeat(200_000));
        assert!(ler_nota(&profundo).is_err());
    }

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

    /// T010b: só os dados de "Usar este" se gravam (chaves deste computador), com valor válido e registro; o CNPJ,
    /// a UF e a IE não se gravam por aqui.
    #[test]
    fn t010b_usar_grava_so_o_permitido() {
        let b = crate::banco::Banco::em_memoria().unwrap();
        let u = crate::usuarios::criar(&b.conn, &b.dispositivo, "Ana", "senha-da-ana-1", true).unwrap().id;
        usar(&b.conn, &b.dispositivo, &u, "serieNfce", "1", None).unwrap();
        usar(&b.conn, &b.dispositivo, &u, "crt", "1", None).unwrap();
        assert_eq!(crate::config::ler(&b.conn, "fiscal_serie_nfce").unwrap().as_deref(), Some("1"));
        assert_eq!(crate::config::ler(&b.conn, "fiscal_crt").unwrap().as_deref(), Some("1"));
        assert!(usar(&b.conn, &b.dispositivo, &u, "cnpj", "11222333000181", None).is_err());
        assert!(usar(&b.conn, &b.dispositivo, &u, "serieNfe", "1000", None).is_err());
        assert!(usar(&b.conn, &b.dispositivo, &u, "crt", "9", None).is_err());
        assert!(usar(&b.conn, &b.dispositivo, &u, "identificadorCsc", "abc", None).is_err());
        let registros: i64 = b
            .conn
            .query_row("SELECT COUNT(*) FROM auditoria WHERE acao = 'fiscal.descoberta'", [], |l| l.get(0))
            .unwrap();
        assert_eq!(registros, 2);
        assert!(!crate::sync::chave_da_loja("fiscal_serie_nfce"), "a série fica só neste computador");
    }

    /// B-4 da R7: sem o CNPJ da loja e com uma pasta só de notas denegadas do mesmo emitente, ele é a loja e o número
    /// gasto vira o piso (a denegada não vira "outro emitente").
    #[test]
    fn r7_b4_so_denegadas_sem_cnpj_da_loja() {
        let denegada = nfce(45, 1, "000001", "3").replace("<cStat>100</cStat>", "<cStat>302</cStat>");
        let d = descobrir_de_xml(&[("b.xml".into(), denegada)], "");
        assert!(d.ignorados.is_empty(), "{:?}", d.ignorados);
        assert_eq!(d.lidas, 0);
        assert_eq!(d.series, vec![Serie { ambiente: 1, modelo: 65, serie: 1, ultimo_numero: 45, notas: 0 }]);
    }

    /// B-3 da R7: na pergunta "qual é a loja", a contagem de cada emitente é só das autorizadas.
    #[test]
    fn r7_b3_candidatos_contam_so_as_autorizadas() {
        let loja = |n: i64| nfce(n, 1, "000001", "1");
        let denegada = nfce(43, 1, "000001", "1").replace("<cStat>100</cStat>", "<cStat>302</cStat>");
        let fornecedor = nfce(7, 1, "000001", "3").replace("11222333000181", "99888777000166");
        let d = descobrir_de_xml(
            &[
                ("a.xml".into(), loja(41)),
                ("b.xml".into(), loja(42)),
                ("c.xml".into(), denegada),
                ("d.xml".into(), fornecedor),
            ],
            "",
        );
        assert_eq!(
            d.candidatos,
            vec![
                Candidato { cnpj: "11222333000181".into(), notas: 2, denegadas: 1 },
                Candidato { cnpj: "99888777000166".into(), notas: 1, denegadas: 0 }
            ]
        );
    }

    /// E6 da R8 (T030): sem o CNPJ da loja, uma denegada da loja e autorizadas de um fornecedor: a tela pergunta qual é
    /// a loja (antes, o fornecedor virava "a loja" sozinho, e os dados dele sairiam como os da loja).
    #[test]
    fn t030_e6_denegada_da_loja_e_autorizadas_de_fornecedor_pergunta() {
        let denegada = nfce(45, 1, "000001", "1").replace("<cStat>100</cStat>", "<cStat>302</cStat>");
        let fornecedor = |n: i64| nfce(n, 1, "000001", "3").replace("11222333000181", "99888777000166");
        let d = descobrir_de_xml(
            &[("a.xml".into(), denegada), ("b.xml".into(), fornecedor(7)), ("c.xml".into(), fornecedor(8))],
            "",
        );
        assert_eq!(
            d.candidatos,
            vec![
                Candidato { cnpj: "99888777000166".into(), notas: 2, denegadas: 0 },
                Candidato { cnpj: "11222333000181".into(), notas: 0, denegadas: 1 }
            ]
        );
        assert!(d.cnpj.is_none() && d.crt.is_none() && d.series.is_empty(), "nada sai antes da escolha: {d:?}");
    }

    /// B-3 da R10: uma nota emitida sem CNPJ (com CPF) vai para os ignorados e não vira candidato com o CNPJ vazio.
    #[test]
    fn t031_b3_emitente_sem_cnpj_fica_de_lado() {
        let com_cpf = nfce(9, 1, "000001", "1").replace("<CNPJ>11222333000181</CNPJ>", "<CPF>12345678909</CPF>");
        let d = descobrir_de_xml(&[("a.xml".into(), nfce(41, 1, "000001", "1")), ("b.xml".into(), com_cpf)], "");
        assert!(d.candidatos.is_empty(), "{:?}", d.candidatos);
        assert_eq!(
            d.cnpj.as_ref().map(|a| a.valor.as_str()),
            Some("11222333000181"),
            "a loja é o único emitente com CNPJ"
        );
        assert_eq!(d.ignorados.len(), 1);
        assert!(d.ignorados[0].contains("sem o CNPJ do emitente"), "{:?}", d.ignorados);
    }

    /// B-2 da R11: com o CNPJ da loja informado, a nota de CPF fica de lado sem mudar os dados da loja (lidas, séries).
    #[test]
    fn t032_b2_com_cnpj_informado_a_nota_de_cpf_fica_de_lado() {
        let com_cpf = nfce(9, 1, "000001", "1").replace("<CNPJ>11222333000181</CNPJ>", "<CPF>12345678909</CPF>");
        let d = descobrir_de_xml(
            &[("a.xml".into(), nfce(41, 1, "000001", "1")), ("b.xml".into(), com_cpf)],
            "11.222.333/0001-81",
        );
        assert_eq!(d.lidas, 1);
        assert_eq!(d.series, vec![Serie { ambiente: 1, modelo: 65, serie: 1, ultimo_numero: 41, notas: 1 }]);
        assert_eq!(
            d.ignorados,
            vec!["b.xml: sem o CNPJ do emitente (nota de CPF; o Rustinho emite só com o CNPJ da loja)"]
        );
    }

    /// B-3 da R12: o emitente sem CNPJ e sem CPF fica de lado sem ser chamado de "nota de CPF".
    #[test]
    fn t033_b3_emitente_sem_cnpj_nem_cpf_nao_e_nota_de_cpf() {
        let vazio = nfce(9, 1, "000001", "1").replace("<CNPJ>11222333000181</CNPJ>", "<CNPJ></CNPJ>");
        let d = descobrir_de_xml(&[("a.xml".into(), nfce(41, 1, "000001", "1")), ("b.xml".into(), vazio)], "");
        assert_eq!(d.ignorados, vec!["b.xml: sem o CNPJ do emitente (o Rustinho emite só com o CNPJ da loja)"]);
    }

    /// B-4 da R10 (EA): um emitente só, com autorizadas e denegadas, sem o CNPJ da loja: ele é a loja, sem perguntar; o
    /// piso conta a denegada e a contagem, só as autorizadas.
    #[test]
    fn t031_b4_ea_um_emitente_com_autorizada_e_denegada_segue_sozinho() {
        let denegada = nfce(45, 1, "000001", "1").replace("<cStat>100</cStat>", "<cStat>302</cStat>");
        let d = descobrir_de_xml(&[("a.xml".into(), nfce(41, 1, "000001", "1")), ("b.xml".into(), denegada)], "");
        assert!(d.candidatos.is_empty(), "{:?}", d.candidatos);
        assert_eq!(d.series, vec![Serie { ambiente: 1, modelo: 65, serie: 1, ultimo_numero: 45, notas: 1 }]);
    }

    /// B-4 da R10 (EB, mudança da E6 registrada na T031): a loja com autorizadas e um fornecedor só com denegada, sem o
    /// CNPJ da loja: agora a tela pergunta (antes da T030, o fornecedor ficava de fora sem pergunta). É o lado seguro.
    #[test]
    fn t031_b4_eb_autorizadas_e_fornecedor_so_denegado_pergunta() {
        let fornecedor = nfce(7, 1, "000001", "3")
            .replace("11222333000181", "99888777000166")
            .replace("<cStat>100</cStat>", "<cStat>302</cStat>");
        let d = descobrir_de_xml(&[("a.xml".into(), nfce(41, 1, "000001", "1")), ("b.xml".into(), fornecedor)], "");
        assert_eq!(
            d.candidatos,
            vec![
                Candidato { cnpj: "11222333000181".into(), notas: 1, denegadas: 0 },
                Candidato { cnpj: "99888777000166".into(), notas: 0, denegadas: 1 }
            ]
        );
    }

    /// N-3 da R8: dois emitentes só com notas denegadas: os dois viram candidatos, com 0 autorizadas e as denegadas
    /// contadas (a tela diz "só notas denegadas", e não "0 notas"); nenhum dado sai antes da escolha.
    #[test]
    fn r8_n3_dois_emitentes_so_denegadas() {
        let denegada = |n: i64, cnpj: &str| {
            nfce(n, 1, "000001", "1")
                .replace("<cStat>100</cStat>", "<cStat>302</cStat>")
                .replace("11222333000181", cnpj)
        };
        let d = descobrir_de_xml(
            &[
                ("a.xml".into(), denegada(41, "11222333000181")),
                ("b.xml".into(), denegada(42, "11222333000181")),
                ("c.xml".into(), denegada(7, "99888777000166")),
            ],
            "",
        );
        assert_eq!(
            d.candidatos,
            vec![
                Candidato { cnpj: "11222333000181".into(), notas: 0, denegadas: 2 },
                Candidato { cnpj: "99888777000166".into(), notas: 0, denegadas: 1 }
            ]
        );
        assert!(d.series.is_empty() && d.cnpj.is_none() && d.lidas == 0, "nada sai antes da escolha: {d:?}");
    }

    /// C-8 da R5: a nota denegada gastou o número: entra no piso da série (o último número), mas não nos dados.
    #[test]
    fn r5_c8_denegada_entra_no_piso() {
        let autorizada = nfce(41, 1, "000001", "1");
        let denegada = nfce(45, 1, "000001", "3").replace("<cStat>100</cStat>", "<cStat>302</cStat>");
        let d = descobrir_de_xml(&[("a.xml".into(), autorizada), ("b.xml".into(), denegada)], "11222333000181");
        assert_eq!(d.lidas, 1, "a denegada não conta como lida");
        assert_eq!(d.crt.unwrap().valor, "1", "os dados vêm só da autorizada");
        assert_eq!(d.series[0].ultimo_numero, 45, "o número da denegada foi gasto");
        assert_eq!(d.series[0].notas, 1, "C-6 da R6: só as autorizadas na contagem");
    }

    /// N-1 da R4: a nota autorizada com alerta (cStat 120, NT 2026.002) conta como autorizada (e o número dela, no piso).
    #[test]
    fn r4_n1_nota_120_e_lida() {
        let com_alerta = nfce(50, 1, "000001", "1").replace("<cStat>100</cStat>", "<cStat>120</cStat>");
        let d = descobrir_de_xml(&[("a.xml".into(), com_alerta)], "11222333000181");
        assert_eq!(d.lidas, 1);
        assert_eq!(d.series[0].ultimo_numero, 50);
    }

    #[test]
    fn t010_identificador_do_csc_online_e_offline() {
        assert_eq!(identificador_csc_do_qrcode("https://x/qrcode?p=CHAVE|2|1|000003|HASH"), "3");
        assert_eq!(identificador_csc_do_qrcode("https://x/qrcode?p=CHAVE|2|1|25|10.00|DIGVAL|000004|HASH"), "4");
        assert_eq!(identificador_csc_do_qrcode("sem parametro"), "");
    }

    /// C-M8 da R1: no QR Code versão 3 não há CSC; o 7º campo off-line é o documento do destinatário (Manual do DANFE
    /// NFC-e v6.0, 4.4.1 e 4.4.2), e não pode virar "identificador do CSC".
    #[test]
    fn r1_cm8_qr_versao_3_nao_tem_csc() {
        let off = "https://x/qrcode?p=CHAVE|3|1|25|10.00|2|12345678909|ASSINATURA";
        assert_eq!(identificador_csc_do_qrcode(off), "", "o 7º é o CPF do destinatário");
        assert_eq!(identificador_csc_do_qrcode("https://x/qrcode?p=CHAVE|3|1"), "");
        assert_eq!(versao_do_qrcode(off), "3");
        let v3 = nfce(46, 1, "x", "1").replace("|2|1|x|ABCDEF", "|3|1|25|10.00|2|12345678909|ASSINATURA");
        let d = descobrir_de_xml(&[("v3.xml".into(), v3)], "11222333000181");
        assert_eq!(d.lidas, 1);
        assert!(d.identificador_csc.is_none());
        assert!(d.qr_versao_3);
    }

    /// C-M8 da R1: sem o CNPJ da loja e com mais de um emitente (notas de compra na pasta), a descoberta não escolhe
    /// pela maioria: devolve os candidatos e nenhum dado; com o escolhido, lê.
    #[test]
    fn r1_cm8_mais_de_um_emitente_pergunta_qual_e_a_loja() {
        let loja = nfce(47, 1, "000001", "1");
        let fornecedor = |n: i64| nfce(n, 1, "000001", "3").replace("11222333000181", "99888777000166");
        let arquivos = vec![("a.xml".into(), loja), ("b.xml".into(), fornecedor(48)), ("c.xml".into(), fornecedor(49))];
        let d = descobrir_de_xml(&arquivos, "");
        assert_eq!(
            d.candidatos,
            vec![
                Candidato { cnpj: "99888777000166".into(), notas: 2, denegadas: 0 },
                Candidato { cnpj: "11222333000181".into(), notas: 1, denegadas: 0 }
            ]
        );
        assert!(d.cnpj.is_none() && d.crt.is_none() && d.series.is_empty() && d.lidas == 0);
        let escolhida = descobrir_de_xml(&arquivos, "11222333000181");
        assert!(escolhida.candidatos.is_empty());
        assert_eq!(escolhida.crt.unwrap().valor, "1");
        // Um emitente só: é a loja.
        let so = descobrir_de_xml(&arquivos[..1], "");
        assert_eq!(so.lidas, 1);
    }

    /// C-M4 da R1: "Usar esta série" grava também o piso da numeração (a última nota do sistema de antes), e o piso só
    /// sobe.
    #[test]
    fn r1_cm4_a_serie_da_descoberta_grava_o_piso() {
        let b = crate::banco::Banco::em_memoria().unwrap();
        let u = crate::usuarios::criar(&b.conn, &b.dispositivo, "Ana", "senha-da-ana-1", true).unwrap().id;
        usar(&b.conn, &b.dispositivo, &u, "serieNfce", "1", Some((1, 3452))).unwrap();
        let piso = crate::nota_fiscal::chave_do_piso(1, 65, 1);
        assert_eq!(crate::config::ler(&b.conn, &piso).unwrap().as_deref(), Some("3452"));
        usar(&b.conn, &b.dispositivo, &u, "serieNfce", "1", Some((1, 100))).unwrap();
        assert_eq!(crate::config::ler(&b.conn, &piso).unwrap().as_deref(), Some("3452"), "só sobe");
        assert!(usar(&b.conn, &b.dispositivo, &u, "serieNfe", "2", Some((3, 10))).is_err(), "ambiente inválido");
        assert!(!crate::sync::chave_da_loja(&piso), "o piso fica só neste computador");
    }
}
