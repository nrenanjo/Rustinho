//! Etiquetas de preço com código de barras, em folhas A4 de etiquetas
//! adesivas (as comuns de papelaria, 65, 33 ou 21 por folha).
//!
//! Aqui se monta o que vai em cada etiqueta (nome, preço, código e as
//! barras já codificadas em 0 e 1); a janela (Qt) desenha no PDF.
//!
//! Código de barras: o da peça (EAN-13) quando ela tem; senão o próprio
//! código interno, em Code 128. O leitor do caixa lê os dois.

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::erro::{Erro, Resultado};
use crate::{config, gtin, numeros};

/// Um modelo de folha de etiquetas (medidas em milímetros).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Modelo {
    pub id: &'static str,
    pub nome: &'static str,
    pub detalhe: &'static str,
    pub colunas: i64,
    pub linhas: i64,
    pub largura: f64,
    pub altura: f64,
    pub topo: f64,
    pub esquerda: f64,
    pub espaco_h: f64,
    pub espaco_v: f64,
}

pub const MODELOS: &[Modelo] = &[
    Modelo {
        id: "65",
        nome: "65 por folha",
        detalhe: "38 × 21 mm (Pimaco A4351 / A4251)",
        colunas: 5,
        linhas: 13,
        largura: 38.1,
        altura: 21.2,
        topo: 10.7,
        esquerda: 4.7,
        espaco_h: 2.5,
        espaco_v: 0.0,
    },
    Modelo {
        id: "33",
        nome: "33 por folha",
        detalhe: "64 × 25 mm (Pimaco A4356 / A4256)",
        colunas: 3,
        linhas: 11,
        largura: 63.5,
        altura: 25.4,
        topo: 8.8,
        esquerda: 7.2,
        espaco_h: 2.5,
        espaco_v: 0.0,
    },
    Modelo {
        id: "21",
        nome: "21 por folha",
        detalhe: "64 × 38 mm (Pimaco A4360 / A4260)",
        colunas: 3,
        linhas: 7,
        largura: 63.5,
        altura: 38.1,
        topo: 15.15,
        esquerda: 7.2,
        espaco_h: 2.5,
        espaco_v: 0.0,
    },
];

pub fn modelo(id: &str) -> &'static Modelo {
    MODELOS.iter().find(|m| m.id == id).unwrap_or(&MODELOS[1])
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Item {
    pub produto_id: String,
    pub quantidade: i64,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Pedido {
    pub itens: Vec<Item>,
    pub modelo: String,
    /// Posição da primeira etiqueta livre na folha (1 = a primeira).
    pub inicio: i64,
    pub barras: bool,
    pub loja: bool,
    /// Ajuste fino da impressora, em décimos de milímetro.
    pub ajuste_x: i64,
    pub ajuste_y: i64,
}

/// O que a tela precisa para montar as etiquetas.
pub fn opcoes(conn: &Connection) -> Resultado<Value> {
    let ler = |c: &str| -> Resultado<i64> { Ok(config::ler(conn, c)?.and_then(|v| v.parse().ok()).unwrap_or(0)) };
    Ok(json!({
        "modelos": MODELOS,
        "modelo": config::ler(conn, "etiqueta_modelo")?.unwrap_or_else(|| "33".into()),
        "barras": config::ler(conn, "etiqueta_barras")?.as_deref() != Some("0"),
        "loja": config::ligado(conn, "etiqueta_loja")?,
        "ajusteX": ler("etiqueta_ajuste_x")?,
        "ajusteY": ler("etiqueta_ajuste_y")?,
    }))
}

/// Uma peça como sai na etiqueta (também para a prévia na tela).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Etiqueta {
    pub produto_id: String,
    pub nome: String,
    pub preco: String,
    pub codigo: String,
    /// Barras e espaços: "1" barra, "0" espaço, uma letra por módulo.
    pub barras: String,
    /// O número embaixo das barras.
    pub texto_barras: String,
    pub sem_preco: bool,
    /// Por que esta etiqueta não pode sair agora (código a definir, ou um
    /// código dela leva também a outra peça). Vazio = pode.
    pub bloqueio: String,
}

pub fn etiqueta(conn: &Connection, produto_id: &str) -> Resultado<Etiqueta> {
    let (nome, codigo, barras, preco): (String, String, String, Option<i64>) = conn
        .query_row(
            "SELECT p.nome, p.codigo, p.codigo_barras, (SELECT preco FROM produto_preco WHERE produto_id = p.id AND tabela = 1)
               FROM produto p WHERE p.id = ?1",
            [produto_id],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)),
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Peça".into()))?;
    let (modulos, texto) = if barras.len() == 13 && gtin::gtin_valido(&barras) {
        (ean13(&barras).unwrap_or_default(), barras.clone())
    } else if !barras.is_empty() {
        (code128(&barras).unwrap_or_default(), barras.clone())
    } else {
        (code128(&codigo).unwrap_or_default(), codigo.clone())
    };
    // Etiqueta com um código que leva a outra peça faria o leitor pegar a
    // peça errada: não sai até o gerente resolver.
    let bloqueio = match crate::faixas::problema_da_peca(conn, produto_id)? {
        Some(p) if codigo.is_empty() => format!("{p} Até lá, não sai etiqueta."),
        Some(p) => format!("{p} Resolva em Configurações › Gerente › Códigos das peças antes de imprimir."),
        None => String::new(),
    };
    Ok(Etiqueta {
        bloqueio,
        produto_id: produto_id.to_string(),
        nome,
        preco: preco.map(numeros::escrever_dinheiro).unwrap_or_default(),
        sem_preco: preco.unwrap_or(0) <= 0,
        codigo,
        barras: modulos,
        texto_barras: texto,
    })
}

/// Monta as folhas: a resposta vai para a janela, que desenha o PDF.
pub fn montar(conn: &Connection, p: &Pedido) -> Resultado<Value> {
    let m = modelo(&p.modelo);
    let por_folha = m.colunas * m.linhas;
    // Cada quantidade é limitada antes de somar: número enorme vindo da
    // tela vira "passou do máximo", nunca uma conta que estoura (R-10).
    let total = numeros::soma(p.itens.iter().map(|i| i.quantidade.clamp(0, 5_001)))?;
    if total == 0 {
        return Err(Erro::validacao("Escolha as peças e quantas etiquetas de cada."));
    }
    if total > 5_000 {
        return Err(Erro::validacao("Use no máximo 5.000 etiquetas de uma vez."));
    }
    let inicio = p.inicio.clamp(1, por_folha);
    // Guarda as escolhas para a próxima vez.
    config::gravar(conn, "etiqueta_modelo", m.id)?;
    config::gravar(conn, "etiqueta_barras", if p.barras { "1" } else { "0" })?;
    config::gravar(conn, "etiqueta_loja", if p.loja { "1" } else { "0" })?;
    config::gravar(conn, "etiqueta_ajuste_x", &p.ajuste_x.clamp(-100, 100).to_string())?;
    config::gravar(conn, "etiqueta_ajuste_y", &p.ajuste_y.clamp(-100, 100).to_string())?;

    let loja = if p.loja { config::nome_loja(conn)? } else { String::new() };
    let mut lista = Vec::new();
    for it in &p.itens {
        if it.quantidade <= 0 {
            continue;
        }
        let e = etiqueta(conn, &it.produto_id)?;
        if !e.bloqueio.is_empty() {
            return Err(Erro::validacao(format!("{}: {}", e.nome, e.bloqueio)));
        }
        let v = json!({
            "nome": e.nome,
            "preco": if e.sem_preco { String::new() } else { format!("R$ {}", e.preco) },
            "codigo": format!("cód. {}", e.codigo),
            "barras": if p.barras { e.barras } else { String::new() },
            "textoBarras": if p.barras { e.texto_barras } else { String::new() },
            "loja": loja,
        });
        for _ in 0..it.quantidade {
            lista.push(v.clone());
        }
    }
    let folhas = (inicio - 1 + lista.len() as i64 + por_folha - 1) / por_folha;
    Ok(json!({
        "modelo": m,
        "inicio": inicio,
        "ajusteX": p.ajuste_x.clamp(-100, 100) as f64 / 10.0,
        "ajusteY": p.ajuste_y.clamp(-100, 100) as f64 / 10.0,
        "etiquetas": lista,
        "folhas": folhas,
    }))
}

// ---------------------------------------------------------------------------
// Códigos de barras
// ---------------------------------------------------------------------------

const EAN_L: [&str; 10] =
    ["0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011", "0110111", "0001011"];
const EAN_G: [&str; 10] =
    ["0100111", "0110011", "0011011", "0100001", "0011101", "0111001", "0000101", "0010001", "0001001", "0010111"];
const EAN_R: [&str; 10] =
    ["1110010", "1100110", "1101100", "1000010", "1011100", "1001110", "1010000", "1000100", "1001000", "1110100"];
const EAN_PARIDADE: [&str; 10] =
    ["LLLLLL", "LLGLGG", "LLGGLG", "LLGGGL", "LGLLGG", "LGGLLG", "LGGGLL", "LGLGLG", "LGLGGL", "LGGLGL"];

/// EAN-13 (13 dígitos, já com o dígito verificador) em módulos.
pub fn ean13(codigo: &str) -> Option<String> {
    let d: Vec<usize> = codigo.chars().map(|c| c.to_digit(10).map(|x| x as usize)).collect::<Option<_>>()?;
    if d.len() != 13 {
        return None;
    }
    let mut s = String::from("101");
    for (i, p) in EAN_PARIDADE[d[0]].chars().enumerate() {
        s.push_str(if p == 'L' { EAN_L[d[i + 1]] } else { EAN_G[d[i + 1]] });
    }
    s.push_str("01010");
    for x in &d[7..13] {
        s.push_str(EAN_R[*x]);
    }
    s.push_str("101");
    Some(s)
}

/// Larguras das barras e espaços de cada símbolo do Code 128 (0 a 106).
const CODE128: [&str; 107] = [
    "212222", "222122", "222221", "121223", "121322", "131222", "122213", "122312", "132212", "221213", "221312",
    "231212", "112232", "122132", "122231", "113222", "123122", "123221", "223211", "221132", "221231", "213212",
    "223112", "312131", "311222", "321122", "321221", "312212", "322112", "322211", "212123", "212321", "232121",
    "111323", "131123", "131321", "112313", "132113", "132311", "211313", "231113", "231311", "112133", "112331",
    "132131", "113123", "113321", "133121", "313121", "211331", "231131", "213113", "213311", "213131", "311123",
    "311321", "331121", "312113", "312311", "332111", "314111", "221411", "431111", "111224", "111422", "121124",
    "121421", "141122", "141221", "112214", "112412", "122114", "122411", "142112", "142211", "241211", "221114",
    "413111", "241112", "134111", "111242", "121142", "121241", "114212", "124112", "124211", "411212", "421112",
    "421211", "212141", "214121", "412121", "111143", "111341", "131141", "114113", "114311", "411113", "411311",
    "113141", "114131", "311141", "411131", "211412", "211214", "211232", "2331112",
];
const START_B: usize = 104;
const START_C: usize = 105;
const STOP: usize = 106;

fn larguras(simbolo: usize, saida: &mut String) {
    for (i, w) in CODE128[simbolo].chars().enumerate() {
        let n = w.to_digit(10).unwrap_or(1) as usize;
        let c = if i % 2 == 0 { '1' } else { '0' };
        for _ in 0..n {
            saida.push(c);
        }
    }
}

/// Code 128 em módulos. Só números (quantidade par): conjunto C, mais
/// compacto; o resto: conjunto B (letras, números e símbolos comuns).
pub fn code128(texto: &str) -> Option<String> {
    if texto.is_empty() || texto.len() > 40 {
        return None;
    }
    let mut valores: Vec<usize> = Vec::new();
    if texto.len().is_multiple_of(2) && texto.chars().all(|c| c.is_ascii_digit()) {
        valores.push(START_C);
        let b = texto.as_bytes();
        for par in b.chunks(2) {
            valores.push(((par[0] - b'0') * 10 + (par[1] - b'0')) as usize);
        }
    } else {
        valores.push(START_B);
        for c in texto.chars() {
            let v = c as u32;
            if !(32..=126).contains(&v) {
                return None;
            }
            valores.push((v - 32) as usize);
        }
    }
    let soma: usize = valores[0] + valores.iter().enumerate().skip(1).map(|(i, v)| i * v).sum::<usize>();
    valores.push(soma % 103);
    valores.push(STOP);
    let mut s = String::new();
    for v in valores {
        larguras(v, &mut s);
    }
    Some(s)
}

#[cfg(test)]
mod testes {
    use super::*;

    /// R-10: quantidade absurda vinda da tela não estoura a soma.
    #[test]
    fn quantidade_enorme_nao_estoura() {
        let b = crate::banco::Banco::em_memoria().unwrap();
        let it = Item { produto_id: "x".into(), quantidade: i64::MAX };
        let p = Pedido { itens: vec![it.clone(), it], modelo: "a4".into(), inicio: 1, ..Default::default() };
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| montar(&b.conn, &p)))
            .expect("não pode dar pânico");
        assert!(r.unwrap_err().to_string().contains("5.000"));
    }

    #[test]
    fn simbolos_do_code128_tem_11_modulos() {
        for (i, p) in CODE128.iter().enumerate() {
            let soma: u32 = p.chars().map(|c| c.to_digit(10).unwrap()).sum();
            assert_eq!(soma, if i == STOP { 13 } else { 11 }, "símbolo {i}");
        }
    }

    #[test]
    fn codigos_iguais_aos_de_referencia() {
        // Conferidos com a biblioteca python-barcode.
        assert_eq!(
            ean13("7891234567895").unwrap(),
            "10101101110010111001100100110110111101001110101010100111010100001000100100100011101001001110101"
        );
        assert_eq!(code128("4455").unwrap(), "110100111001000110111011101000110110111011101100011101011");
        assert_eq!(
            code128("12345678").unwrap(),
            "1101001110010110011100100010110001110001011011000010100100011101101100011101011"
        );
        assert_eq!(
            code128("ABC-1").unwrap(),
            "110100100001010001100010001011000100010001101001101110010011100110100010001101100011101011"
        );
        assert_eq!(
            code128("Rustinho").unwrap(),
            "110100100001100010111010011110010101111001001001111010010000110100110000101001001100001010001111010111101010001100011101011"
        );
        assert_eq!(
            code128("XY 9").unwrap(),
            "1101001000011100010110111011010001101100110011100101100100101100001100011101011"
        );
        assert!(ean13("789").is_none());
        assert!(code128("").is_none());
    }
}
