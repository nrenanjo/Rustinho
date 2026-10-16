//! Leitor de código de barras pela câmera (celular).
//!
//! A tela tira uma "foto" da câmera de tempos em tempos, guarda em PGM (tons
//! de cinza, o formato mais simples que existe) e pergunta aqui se tem um
//! código nela. EAN-13 / EAN-8 / UPC (o das embalagens), Code 128 / Code 39
//! (o das etiquetas) e QR.

use std::collections::HashSet;
use std::path::Path;

use rxing::{BarcodeFormat, DecodeHints};

use crate::erro::{Erro, Resultado};

/// Lê uma imagem PGM (P5) ou PPM (P6) de 8 bits: (largura, altura, cinza).
pub fn ler_pnm(dados: &[u8]) -> Resultado<(u32, u32, Vec<u8>)> {
    let ruim = || Erro::validacao("A foto da câmera veio num formato que não conheço.");
    let mut pos = 0usize;
    let mut campos: Vec<String> = Vec::new();
    while campos.len() < 4 {
        // Pula espaços e comentários.
        while pos < dados.len() && (dados[pos].is_ascii_whitespace() || dados[pos] == b'#') {
            if dados[pos] == b'#' {
                while pos < dados.len() && dados[pos] != b'\n' {
                    pos += 1;
                }
            } else {
                pos += 1;
            }
        }
        let inicio = pos;
        while pos < dados.len() && !dados[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if inicio == pos {
            return Err(ruim());
        }
        campos.push(String::from_utf8_lossy(&dados[inicio..pos]).into_owned());
    }
    pos += 1; // um espaço depois do valor máximo
    let canais = match campos[0].as_str() {
        "P5" => 1,
        "P6" => 3,
        _ => return Err(ruim()),
    };
    let largura: u32 = campos[1].parse().map_err(|_| ruim())?;
    let altura: u32 = campos[2].parse().map_err(|_| ruim())?;
    let maximo: u32 = campos[3].parse().map_err(|_| ruim())?;
    if largura == 0 || altura == 0 || maximo == 0 || maximo > 255 || largura > 8000 || altura > 8000 {
        return Err(ruim());
    }
    let total = largura as usize * altura as usize;
    let corpo = dados.get(pos..pos + total * canais).ok_or_else(ruim)?;
    let cinza = if canais == 1 {
        corpo.to_vec()
    } else {
        corpo
            .chunks_exact(3)
            .map(|p| ((p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000) as u8)
            .collect()
    };
    Ok((largura, altura, cinza))
}

/// O código que aparece na imagem, se tiver um. `None` = não achou (normal,
/// a câmera manda outra foto logo em seguida).
pub fn ler_imagem(largura: u32, altura: u32, cinza: Vec<u8>) -> Option<String> {
    let mut dicas = DecodeHints {
        TryHarder: Some(true),
        PossibleFormats: Some(HashSet::from([
            BarcodeFormat::EAN_13,
            BarcodeFormat::EAN_8,
            BarcodeFormat::UPC_A,
            BarcodeFormat::UPC_E,
            BarcodeFormat::CODE_128,
            BarcodeFormat::CODE_39,
            BarcodeFormat::ITF,
            BarcodeFormat::QR_CODE,
        ])),
        ..Default::default()
    };
    rxing::helpers::detect_in_luma_with_hints(cinza, largura, altura, None, &mut dicas)
        .ok()
        .map(|r| r.getText().trim().to_string())
        .filter(|t| !t.is_empty())
}

/// Lê o arquivo da foto e apaga em seguida.
pub fn ler_foto(caminho: &Path) -> Resultado<Option<String>> {
    let dados = std::fs::read(caminho).map_err(|e| Erro::validacao(format!("Não consegui ler a foto da câmera: {e}")))?;
    let _ = std::fs::remove_file(caminho);
    let (l, a, cinza) = ler_pnm(&dados)?;
    Ok(ler_imagem(l, a, cinza))
}

#[cfg(test)]
mod testes {
    use super::*;
    use rxing::Writer;

    /// Desenha o código numa imagem cinza com margem branca, como a câmera veria.
    fn desenhar(texto: &str, formato: BarcodeFormat) -> (u32, u32, Vec<u8>) {
        let m = rxing::MultiFormatWriter.encode(texto, &formato, 400, 120).unwrap();
        let (w, h) = (m.getWidth() + 80, m.getHeight() + 80);
        let mut px = vec![235u8; (w * h) as usize];
        for y in 0..m.getHeight() {
            for x in 0..m.getWidth() {
                if m.get(x, y) {
                    px[((y + 40) * w + x + 40) as usize] = 30;
                }
            }
        }
        (w, h, px)
    }

    #[test]
    fn le_ean_e_code128() {
        let (w, h, px) = desenhar("7891234567895", BarcodeFormat::EAN_13);
        assert_eq!(ler_imagem(w, h, px).as_deref(), Some("7891234567895"));
        let (w, h, px) = desenhar("8PK1515", BarcodeFormat::CODE_128);
        assert_eq!(ler_imagem(w, h, px).as_deref(), Some("8PK1515"));
    }

    #[test]
    fn foto_em_pgm_e_ppm() {
        let (w, h, px) = desenhar("7891234567895", BarcodeFormat::EAN_13);
        let mut pgm = format!("P5\n# Qt\n{w} {h}\n255\n").into_bytes();
        pgm.extend_from_slice(&px);
        let (l, a, c) = ler_pnm(&pgm).unwrap();
        assert_eq!((l, a), (w, h));
        assert_eq!(ler_imagem(l, a, c).as_deref(), Some("7891234567895"));

        let mut ppm = format!("P6 {w} {h} 255\n").into_bytes();
        for p in &px {
            ppm.extend_from_slice(&[*p, *p, *p]);
        }
        let (l, a, c) = ler_pnm(&ppm).unwrap();
        assert_eq!(ler_imagem(l, a, c).as_deref(), Some("7891234567895"));

        let pasta = std::env::temp_dir().join(format!("rustinho-codigo-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let arquivo = pasta.join("quadro.pgm");
        std::fs::write(&arquivo, &pgm).unwrap();
        assert_eq!(ler_foto(&arquivo).unwrap().as_deref(), Some("7891234567895"));
        assert!(!arquivo.exists());
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn sem_codigo_e_lixo() {
        assert_eq!(ler_imagem(200, 100, vec![200u8; 20000]), None);
        assert!(ler_pnm(b"P2 1 1 255\n0").is_err());
        assert!(ler_pnm(b"P5 10 10 255\n").is_err());
    }
}
