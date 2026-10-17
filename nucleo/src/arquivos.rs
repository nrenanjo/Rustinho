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
/// Tamanho máximo de um .zip dentro de outro (é lido na memória).
const MAX_ZIP_INTERNO: u64 = 100 * 1024 * 1024;
/// Total descompactado numa escolha de arquivos (protege de "bombas" de zip).
const MAX_TOTAL: u64 = 512 * 1024 * 1024;
/// Itens olhados dentro dos pacotes, somando todos.
const MAX_MEMBROS: usize = 100_000;

/// Quanto ainda pode ser descompactado nesta escolha de arquivos.
struct Orcamento {
    bytes: u64,
    membros: usize,
}

impl Orcamento {
    fn novo() -> Orcamento {
        Orcamento { bytes: MAX_TOTAL, membros: MAX_MEMBROS }
    }
    /// Conta mais um item; false = acabou o limite.
    fn membro(&mut self) -> bool {
        if self.membros == 0 {
            return false;
        }
        self.membros -= 1;
        true
    }
    fn gastar(&mut self, n: u64) -> bool {
        if n > self.bytes {
            self.bytes = 0;
            return false;
        }
        self.bytes -= n;
        true
    }
}

/// Lê no máximo `limite` bytes; None se passou (o tamanho declarado mentiu).
fn ler_ate(r: &mut impl Read, limite: u64) -> Option<Vec<u8>> {
    let mut b = Vec::new();
    r.take(limite + 1).read_to_end(&mut b).ok()?;
    if b.len() as u64 > limite {
        return None;
    }
    Some(b)
}

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
    let mut orc = Orcamento::novo();
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
            "zip" => match std::fs::File::open(&p) {
                Ok(f) => de_zip(&nome, std::io::BufReader::new(f), &mut saida, true, &mut orc),
                Err(e) => saida.push(ruim(nome, &format!("não abriu ({e})"))),
            },
            "rar" => de_rar(&nome, &p, &mut saida, &mut orc),
            _ => {}
        }
        if saida.len() >= MAX_ARQUIVOS {
            break;
        }
    }
    Ok(saida)
}

fn de_zip<R: Read + std::io::Seek>(pacote: &str, leitor: R, saida: &mut Vec<Xml>, entrar_em_zip: bool, orc: &mut Orcamento) {
    let Ok(mut zip) = zip::ZipArchive::new(leitor) else {
        saida.push(ruim(pacote.into(), "o .zip está estragado ou não é zip"));
        return;
    };
    for i in 0..zip.len() {
        if saida.len() >= MAX_ARQUIVOS {
            return;
        }
        if !orc.membro() {
            saida.push(ruim(pacote.into(), "arquivos demais dentro dos pacotes"));
            return;
        }
        let Ok(mut f) = zip.by_index(i) else { continue };
        if !f.is_file() {
            continue;
        }
        let interno = f.name().rsplit('/').next().unwrap_or_default().to_string();
        let baixo = interno.to_lowercase();
        let nome = format!("{pacote} › {interno}");
        let limite = if baixo.ends_with(".xml") {
            MAX_TAMANHO
        } else if entrar_em_zip && baixo.ends_with(".zip") {
            MAX_ZIP_INTERNO
        } else {
            continue;
        };
        if f.size() > limite {
            saida.push(ruim(nome, "grande demais para ser uma nota"));
            continue;
        }
        let Some(b) = ler_ate(&mut f, limite) else {
            saida.push(ruim(nome, "não deu para descompactar"));
            continue;
        };
        if !orc.gastar(b.len() as u64) {
            saida.push(ruim(pacote.into(), "os pacotes descompactados passam de 512 MB; escolha menos arquivos"));
            return;
        }
        if baixo.ends_with(".xml") {
            saida.push(Xml { nome, texto: texto_do_xml(&b), erro: String::new() });
        } else {
            de_zip(&nome, std::io::Cursor::new(b), saida, false, orc);
        }
    }
}

fn de_rar(pacote: &str, caminho: &Path, saida: &mut Vec<Xml>, orc: &mut Orcamento) {
    let arquivo = match rars::ArchiveReader::read_path(caminho) {
        Ok(a) => a,
        Err(_) => {
            saida.push(ruim(pacote.into(), "o .rar está estragado, tem senha ou é de várias partes"));
            return;
        }
    };
    let membros: Vec<(usize, String, bool, u64)> = arquivo
        .members()
        .enumerate()
        .map(|(i, m)| (i, m.meta.name_lossy(), m.meta.is_directory, m.meta.unpacked_size))
        .collect();
    for (i, nome_interno, pasta, tamanho) in membros {
        if saida.len() >= MAX_ARQUIVOS {
            return;
        }
        if !orc.membro() {
            saida.push(ruim(pacote.into(), "arquivos demais dentro dos pacotes"));
            return;
        }
        if pasta {
            continue;
        }
        let interno = nome_interno.rsplit(['/', '\\']).next().unwrap_or_default().to_string();
        if !interno.to_lowercase().ends_with(".xml") {
            continue;
        }
        let nome = format!("{pacote} › {interno}");
        // O tamanho é conferido antes de descompactar.
        if tamanho > MAX_TAMANHO {
            saida.push(ruim(nome, "grande demais para ser uma nota"));
            continue;
        }
        if !orc.gastar(tamanho) {
            saida.push(ruim(pacote.into(), "os pacotes descompactados passam de 512 MB; escolha menos arquivos"));
            return;
        }
        let mut opcoes = rars::ArchiveReadOptions::new();
        opcoes.max_member_output_bytes = Some(MAX_TAMANHO);
        opcoes.max_total_output_bytes = Some(MAX_TOTAL);
        opcoes.rar50_dictionary_size_limit = Some(256 * 1024 * 1024);
        match arquivo.read_member_at_with_options(i, opcoes) {
            Ok(Some(b)) if b.len() as u64 <= MAX_TAMANHO => {
                saida.push(Xml { nome, texto: texto_do_xml(&b), erro: String::new() })
            }
            Ok(Some(_)) => saida.push(ruim(nome, "grande demais para ser uma nota")),
            Ok(None) => {}
            Err(_) => saida.push(ruim(nome, "não deu para descompactar (senha ou grande demais?)")),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn zip_com(arquivos: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (n, b) in arquivos {
            z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(b).unwrap();
        }
        z.finish().unwrap().into_inner()
    }

    #[test]
    fn limite_total_dos_pacotes() {
        let zip = zip_com(&[("a.xml", b"<a>0123456</a>"), ("b.xml", b"<b>0123456</b>"), ("c.xml", b"<c>0123456</c>")]);
        let mut saida = Vec::new();
        let mut orc = Orcamento { bytes: 30, membros: 100 };
        de_zip("p.zip", std::io::Cursor::new(zip.clone()), &mut saida, true, &mut orc);
        assert_eq!(saida.iter().filter(|x| x.erro.is_empty()).count(), 2);
        assert!(saida.last().unwrap().erro.contains("512 MB"));
        // Itens demais.
        let mut saida = Vec::new();
        let mut orc = Orcamento { bytes: 1000, membros: 1 };
        de_zip("p.zip", std::io::Cursor::new(zip), &mut saida, true, &mut orc);
        assert!(saida.iter().any(|x| x.erro.contains("demais")));
        // Ler além do declarado não passa do limite.
        assert!(ler_ate(&mut &b"0123456789"[..], 5).is_none());
        assert_eq!(ler_ate(&mut &b"01234"[..], 5).unwrap(), b"01234");
    }

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
