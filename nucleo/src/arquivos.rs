//! Arquivos escolhidos na tela: caminhos (inclusive "file:///C:/..." no
//! Windows) e XMLs de notas soltos, em pastas, em .zip ou em .rar.

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::erro::{Erro, Resultado};

/// Limites para não travar com uma pasta enorme por engano.
pub const MAX_ARQUIVOS: usize = 30_000;
/// Tamanho máximo de um XML de nota.
pub const MAX_TAMANHO: u64 = 20 * 1024 * 1024;
/// Tamanho máximo de um .zip ou .rar.
const MAX_PACOTE: u64 = 1024 * 1024 * 1024;

/// "file:///home/x" → "/home/x"; "file:///C:/x" → "C:/x"; "%20" → " ".
pub fn caminho_local(t: &str) -> PathBuf {
    let t = t.trim();
    let sem = match t.strip_prefix("file://") {
        Some(resto) => {
            let b = resto.as_bytes();
            // Windows: "/C:/..." (letra do disco depois da barra).
            if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' {
                &resto[1..]
            } else {
                resto
            }
        }
        None => t,
    };
    PathBuf::from(decodificar_url(sem))
}

fn decodificar_url(t: &str) -> String {
    let bytes = t.as_bytes();
    let mut saida = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&t[i + 1..i + 3], 16) {
                saida.push(v);
                i += 3;
                continue;
            }
        }
        saida.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&saida).into_owned()
}

/// Extensões aceitas onde se pede XML.
pub fn aceito(nome: &str) -> bool {
    let n = nome.to_lowercase();
    n.ends_with(".xml") || n.ends_with(".zip") || n.ends_with(".rar")
}

fn extensao(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
}

fn nome_curto(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn juntar(caminho: &Path, lista: &mut Vec<PathBuf>, profundidade: u32) {
    if lista.len() >= MAX_ARQUIVOS {
        return;
    }
    if caminho.is_dir() {
        if profundidade > 6 {
            return;
        }
        let Ok(dir) = std::fs::read_dir(caminho) else { return };
        let mut filhos: Vec<PathBuf> = dir.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        filhos.sort();
        for f in filhos {
            juntar(&f, lista, profundidade + 1);
        }
    } else if aceito(&nome_curto(caminho)) {
        lista.push(caminho.to_path_buf());
    }
}

/// XMLs antigos às vezes vêm em ISO-8859-1 (Latin-1).
pub fn texto_do_xml(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(t) => t.to_string(),
        Err(_) => {
            let t: String = bytes.iter().map(|&b| b as char).collect();
            t.replacen("encoding=\"ISO-8859-1\"", "encoding=\"UTF-8\"", 1)
                .replacen("encoding=\"iso-8859-1\"", "encoding=\"UTF-8\"", 1)
        }
    }
}

/// Um XML achado. `texto` vazio = não deu para ler (o motivo em `erro`).
#[derive(Debug, Clone)]
pub struct Xml {
    /// "nota.xml" ou "pacote.zip › nota.xml"
    pub nome: String,
    pub texto: String,
    pub erro: String,
}

fn ruim(nome: String, erro: &str) -> Xml {
    Xml { nome, texto: String::new(), erro: erro.into() }
}

/// Todos os XMLs dos arquivos e pastas escolhidos, abrindo .zip e .rar
/// (inclusive .zip dentro de .zip, um nível).
pub fn xmls(entradas: &[String]) -> Resultado<Vec<Xml>> {
    let mut lista = Vec::new();
    for e in entradas {
        let p = caminho_local(e);
        if !p.exists() {
            return Err(Erro::validacao(format!("Não achei {}.", p.display())));
        }
        if p.is_file() && !aceito(&nome_curto(&p)) {
            return Err(Erro::validacao(format!(
                "{} não é XML, ZIP nem RAR.",
                nome_curto(&p)
            )));
        }
        juntar(&p, &mut lista, 0);
    }
    let mut saida = Vec::new();
    for p in lista {
        let nome = nome_curto(&p);
        let Ok(meta) = std::fs::metadata(&p) else { continue };
        match extensao(&p).as_str() {
            "xml" => {
                if meta.len() > MAX_TAMANHO {
                    saida.push(ruim(nome, "grande demais para ser uma nota"));
                    continue;
                }
                match std::fs::read(&p) {
                    Ok(b) => saida.push(Xml { nome, texto: texto_do_xml(&b), erro: String::new() }),
                    Err(e) => saida.push(ruim(nome, &format!("não abriu ({e})"))),
                }
            }
            "zip" | "rar" if meta.len() > MAX_PACOTE => saida.push(ruim(nome, "pacote grande demais")),
            "zip" => match std::fs::read(&p) {
                Ok(b) => de_zip(&nome, b, &mut saida, true),
                Err(e) => saida.push(ruim(nome, &format!("não abriu ({e})"))),
            },
            "rar" => de_rar(&nome, &p, &mut saida),
            _ => {}
        }
        if saida.len() >= MAX_ARQUIVOS {
            break;
        }
    }
    Ok(saida)
}

fn de_zip(pacote: &str, bytes: Vec<u8>, saida: &mut Vec<Xml>, entrar_em_zip: bool) {
    let Ok(mut zip) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) else {
        saida.push(ruim(pacote.into(), "o .zip está estragado ou não é zip"));
        return;
    };
    for i in 0..zip.len() {
        let Ok(mut f) = zip.by_index(i) else { continue };
        if !f.is_file() {
            continue;
        }
        let interno = f.name().rsplit('/').next().unwrap_or_default().to_string();
        let baixo = interno.to_lowercase();
        let nome = format!("{pacote} › {interno}");
        if baixo.ends_with(".xml") {
            if f.size() > MAX_TAMANHO {
                saida.push(ruim(nome, "grande demais para ser uma nota"));
                continue;
            }
            let mut b = Vec::new();
            if f.read_to_end(&mut b).is_ok() {
                saida.push(Xml { nome, texto: texto_do_xml(&b), erro: String::new() });
            } else {
                saida.push(ruim(nome, "não deu para descompactar"));
            }
        } else if entrar_em_zip && baixo.ends_with(".zip") && f.size() <= MAX_PACOTE {
            let mut b = Vec::new();
            if f.read_to_end(&mut b).is_ok() {
                de_zip(&nome, b, saida, false);
            }
        }
    }
}

fn de_rar(pacote: &str, caminho: &Path, saida: &mut Vec<Xml>) {
    let arquivo = match rars::ArchiveReader::read_path(caminho) {
        Ok(a) => a,
        Err(_) => {
            saida.push(ruim(pacote.into(), "o .rar está estragado, tem senha ou é de várias partes"));
            return;
        }
    };
    let membros: Vec<(usize, String, bool)> = arquivo
        .members()
        .enumerate()
        .map(|(i, m)| (i, m.meta.name_lossy(), m.meta.is_directory))
        .collect();
    for (i, nome_interno, pasta) in membros {
        if pasta {
            continue;
        }
        let interno = nome_interno.rsplit(['/', '\\']).next().unwrap_or_default().to_string();
        if !interno.to_lowercase().ends_with(".xml") {
            continue;
        }
        let nome = format!("{pacote} › {interno}");
        match arquivo.read_member_at(i, None) {
            Ok(Some(b)) if b.len() as u64 <= MAX_TAMANHO => {
                saida.push(Xml { nome, texto: texto_do_xml(&b), erro: String::new() })
            }
            Ok(Some(_)) => saida.push(ruim(nome, "grande demais para ser uma nota")),
            Ok(None) => {}
            Err(_) => saida.push(ruim(nome, "não deu para descompactar (senha?)")),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn caminhos() {
        assert_eq!(caminho_local("file:///home/ana/nota%20x.xml"), PathBuf::from("/home/ana/nota x.xml"));
        assert_eq!(caminho_local("file:///C:/Users/Ana/nota.xml"), PathBuf::from("C:/Users/Ana/nota.xml"));
        assert_eq!(caminho_local("/tmp/a.xml"), PathBuf::from("/tmp/a.xml"));
        assert!(aceito("A.XML") && aceito("b.Zip") && aceito("c.rar") && !aceito("d.pdf"));
    }

    #[test]
    fn latin1_e_bom() {
        assert_eq!(texto_do_xml(&[0xEF, 0xBB, 0xBF, b'<', b'a', b'>']), "<a>");
        let t = texto_do_xml(b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><x>S\xC3O</x>");
        assert!(t.contains("UTF-8") && t.contains("SÃO"));
    }

    #[test]
    fn zip_dentro_de_zip_e_pasta() {
        use std::io::Write;
        let pasta = std::env::temp_dir().join(format!("rustinho-arquivos-{}", crate::tempo::novo_id()));
        std::fs::create_dir_all(pasta.join("sub")).unwrap();
        let fazer_zip = |itens: &[(&str, &[u8])]| {
            let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            for (n, b) in itens {
                z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
                z.write_all(b).unwrap();
            }
            z.finish().unwrap().into_inner()
        };
        let dentro = fazer_zip(&[("c.xml", b"<c/>")]);
        std::fs::write(pasta.join("sub/pacote.zip"), fazer_zip(&[("a.xml", b"<a/>"), ("leia.txt", b"x"), ("outro.zip", &dentro)])).unwrap();
        std::fs::write(pasta.join("b.xml"), b"<b/>").unwrap();
        std::fs::write(pasta.join("foto.jpg"), b"x").unwrap();
        let l = xmls(&[pasta.display().to_string()]).unwrap();
        let mut nomes: Vec<String> = l.iter().map(|x| format!("{}={}", x.nome, x.texto)).collect();
        nomes.sort();
        assert_eq!(nomes, ["b.xml=<b/>", "pacote.zip › a.xml=<a/>", "pacote.zip › outro.zip › c.xml=<c/>"]);
        assert!(xmls(&[pasta.join("foto.jpg").display().to_string()]).is_err());
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn rar_de_verdade() {
        // Feitos com o RAR oficial (normal e sólido), em base64.
        for (nome, b64) in [("rar5", RAR5), ("solido", RAR5_SOLIDO)] {
            let bytes = base64_decodificar(b64);
            let pasta = std::env::temp_dir().join(format!("rustinho-rar-{nome}-{}", crate::tempo::novo_id()));
            std::fs::create_dir_all(&pasta).unwrap();
            let p = pasta.join(format!("{nome}.rar"));
            std::fs::write(&p, bytes).unwrap();
            let l = xmls(&[p.display().to_string()]).unwrap();
            assert_eq!(l.len(), 2, "{nome}: {l:?}");
            assert!(l.iter().all(|x| x.erro.is_empty() && x.texto.contains("<nfeProc")), "{nome}: {l:?}");
            let _ = std::fs::remove_dir_all(&pasta);
        }
    }

    fn base64_decodificar(t: &str) -> Vec<u8> {
        let tabela = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut saida = Vec::new();
        let mut buf = 0u32;
        let mut bits = 0;
        for c in t.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
            let v = tabela.iter().position(|x| *x == c).unwrap() as u32;
            buf = (buf << 6) | v;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                saida.push((buf >> bits) as u8);
                buf &= (1 << bits) - 1;
            }
        }
        saida
    }

    include!("arquivos_teste_rar.rs");
}
