//! Arquivos escolhidos na tela: caminhos (inclusive "file:///C:/..." no
//! Windows) e XMLs de notas soltos, em pastas, em .zip ou em .rar.

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::erro::{Erro, Resultado};

/// Limites para não travar com uma pasta enorme por engano. No celular
/// (2 GB de memória, às vezes 32 bits) os limites são menores (C-08): um
/// ano de notas de uma loja cabe folgado nos dois.
#[cfg(not(target_os = "android"))]
mod limites {
    pub const MAX_ARQUIVOS: usize = 30_000;
    pub const MAX_PACOTE: u64 = 1024 * 1024 * 1024;
    pub const MAX_ZIP_INTERNO: u64 = 100 * 1024 * 1024;
    pub const MAX_TOTAL: u64 = 512 * 1024 * 1024;
    pub const MAX_MEMBROS: usize = 100_000;
    pub const MAX_DICIONARIO_RAR: u64 = 256 * 1024 * 1024;
}
#[cfg(target_os = "android")]
mod limites {
    pub const MAX_ARQUIVOS: usize = 10_000;
    pub const MAX_PACOTE: u64 = 256 * 1024 * 1024;
    pub const MAX_ZIP_INTERNO: u64 = 32 * 1024 * 1024;
    pub const MAX_TOTAL: u64 = 128 * 1024 * 1024;
    pub const MAX_MEMBROS: usize = 20_000;
    pub const MAX_DICIONARIO_RAR: u64 = 64 * 1024 * 1024;
}
/// Quantos arquivos numa escolha só.
pub const MAX_ARQUIVOS: usize = limites::MAX_ARQUIVOS;
/// Tamanho máximo de um XML de nota.
pub const MAX_TAMANHO: u64 = 20 * 1024 * 1024;
/// Tamanho máximo de um .zip ou .rar.
const MAX_PACOTE: u64 = limites::MAX_PACOTE;
/// Tamanho máximo de um .zip dentro de outro (é lido na memória).
const MAX_ZIP_INTERNO: u64 = limites::MAX_ZIP_INTERNO;
/// Total descompactado numa escolha de arquivos (protege de "bombas" de zip).
const MAX_TOTAL: u64 = limites::MAX_TOTAL;
/// Itens olhados dentro dos pacotes, somando todos.
const MAX_MEMBROS: usize = limites::MAX_MEMBROS;

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
    // Só bytes (nunca fatia o texto): um "%" seguido de letras com acento ou
    // de pouca coisa fica como está, sem derrubar o programa.
    fn hexa(b: u8) -> Option<u8> {
        (b as char).to_digit(16).map(|v| v as u8)
    }
    let bytes = t.as_bytes();
    let mut saida = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let Some([a, b]) = bytes.get(i + 1..i + 3).and_then(|p| <[u8; 2]>::try_from(p).ok()) {
                if let (Some(a), Some(b)) = (hexa(a), hexa(b)) {
                    saida.push(a * 16 + b);
                    i += 3;
                    continue;
                }
            }
        }
        saida.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&saida).into_owned()
}

/// Deixa a pasta ou o arquivo só para o usuário do computador (Linux,
/// macOS, Android): o banco, os backups e o registro têm dados da loja e
/// dos clientes. No Windows, a pasta do usuário já é só dele.
///
/// Confere depois se ficou mesmo assim. Se o lugar não aceitar (pen drive
/// FAT, por exemplo) ou der erro, anota no registro de falhas só o que era
/// (`o_que`: "banco", "backup"...) e o tipo do erro, sem o caminho (que pode
/// ter o nome do usuário), e segue: nada é apagado nem escondido. Devolve se
/// ficou protegido.
pub fn so_do_usuario(caminho: &Path, o_que: &str) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let Ok(antes) = std::fs::metadata(caminho) else {
            crate::registro::anotar(&format!("permissões: {o_que} não existe para proteger"));
            return false;
        };
        let modo = if antes.is_dir() { 0o700 } else { 0o600 };
        if let Err(e) = std::fs::set_permissions(caminho, std::fs::Permissions::from_mode(modo)) {
            crate::registro::anotar(&format!("permissões: não deu para proteger {o_que} ({:?})", e.kind()));
            return false;
        }
        let ficou = std::fs::metadata(caminho).map(|m| m.permissions().mode() & 0o777).unwrap_or(0);
        if ficou != modo {
            crate::registro::anotar(&format!("permissões: {o_que} ficou com {ficou:o} (a pasta não aceita)"));
            return false;
        }
        true
    }
    #[cfg(not(unix))]
    {
        let _ = (caminho, o_que);
        true
    }
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

/// Junta os arquivos aceitos, sem guardar a pasta inteira na memória (R-03):
/// uma pasta só entra até o limite, e o total de itens olhados (pastas,
/// fotos, qualquer coisa) também tem teto. Devolve false se cortou. `fundo`
/// fica verdadeiro se alguma subpasta passou de 6 níveis e não foi olhada (T087).
fn juntar(caminho: &Path, lista: &mut Vec<PathBuf>, profundidade: u32, olhar: &mut usize, fundo: &mut bool) -> bool {
    if lista.len() >= MAX_ARQUIVOS || *olhar == 0 {
        return false;
    }
    *olhar -= 1;
    if caminho.is_dir() {
        if profundidade > 6 {
            *fundo = true;
            return true;
        }
        let Ok(dir) = std::fs::read_dir(caminho) else { return true };
        let mut filhos = Vec::new();
        let mut inteiro = true;
        for e in dir.filter_map(|e| e.ok()) {
            if *olhar == 0 || filhos.len() >= MAX_ARQUIVOS {
                inteiro = false;
                break;
            }
            let p = e.path();
            // Só guarda o que interessa: subpastas e XML/ZIP/RAR.
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) || aceito(&nome_curto(&p)) {
                filhos.push(p);
            } else {
                *olhar -= 1;
            }
        }
        filhos.sort();
        for f in filhos {
            inteiro &= juntar(&f, lista, profundidade + 1, olhar, fundo);
        }
        inteiro
    } else {
        if aceito(&nome_curto(caminho)) {
            lista.push(caminho.to_path_buf());
        }
        true
    }
}

/// Windows-1252 nos bytes 0x80 a 0x9F (fora isso, é o Latin-1). Os cinco
/// sem letra ficam como no Latin-1.
const CP1252: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž', '\u{8F}', '\u{90}', '‘', '’',
    '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9D}', 'ž', 'Ÿ',
];

/// XMLs antigos às vezes não vêm em UTF-8: ISO-8859-1 (Latin-1) ou, feitos no
/// Windows, Windows-1252 (o Latin-1 com aspas curvas, travessão e € nos
/// bytes 0x80 a 0x9F). F06 (dev.3.3): lidos como Windows-1252, e a
/// declaração passa a dizer UTF-8 com qualquer aspa e qualquer nome (antes,
/// só `encoding="ISO-8859-1"` com aspas duplas era trocado).
pub fn texto_do_xml(bytes: &[u8]) -> String {
    // BUG-20261128-XDCL (diretriz do dono, 28/11): o Rustinho lê a nota apesar do que vier em volta dela.
    // - UTF-16 (o "Unicode" do Bloco de Notas do Windows), com ou sem a marca;
    // - UTF-8 válido;
    // - UTF-8 com lixo inválido em volta (M-2 da revisão): a codificação é decidida pela parte da nota, depois do corte,
    //   e não pelo arquivo inteiro (um byte estragado no lixo não pode estragar os acentos da nota);
    // - senão, ISO-8859-1 / Windows-1252.
    // Depois, `so_a_nota` tira o que estiver em volta da nota (só quando o arquivo não abre como está).
    let preparar = |t: String| declarar_utf8(so_a_nota(declarar_utf8(t)));
    if let Some(t) = utf16(bytes) {
        return preparar(t);
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(t) => preparar(t.to_string()),
        Err(_) => {
            let cortado = preparar(String::from_utf8_lossy(bytes).into_owned());
            if !cortado.contains('\u{FFFD}') && cortado.starts_with('<') {
                cortado
            } else {
                preparar(
                    bytes
                        .iter()
                        .map(|&b| if (0x80..0xA0).contains(&b) { CP1252[(b - 0x80) as usize] } else { b as char })
                        .collect(),
                )
            }
        }
    }
}

/// O texto em UTF-16, com a marca (FF FE ou FE FF) ou, sem ela, reconhecido pelos bytes nulos dos dois primeiros
/// caracteres (3C 00 3F 00 ou 00 3C 00 3F). `None`: não é UTF-16. Um byte sobrando no fim é ignorado; um substituto
/// solto vira U+FFFD.
fn utf16(bytes: &[u8]) -> Option<String> {
    let (corpo, le) = match bytes {
        [0xFF, 0xFE, r @ ..] => (r, true),
        [0xFE, 0xFF, r @ ..] => (r, false),
        [a, 0, b, 0, ..] if *a != 0 && *b != 0 => (bytes, true),
        [0, a, 0, b, ..] if *a != 0 && *b != 0 => (bytes, false),
        _ => return None,
    };
    let unidades =
        corpo.as_chunks::<2>().0.iter().map(|&c| if le { u16::from_le_bytes(c) } else { u16::from_be_bytes(c) });
    Some(char::decode_utf16(unidades).map(|r| r.unwrap_or('\u{FFFD}')).collect())
}

/// Os elementos que começam um documento fiscal que o Rustinho lê (a nota, a nota processada e o que o importador
/// sabe explicar: CT-e, resumo e evento).
const RAIZES_FISCAIS: [&str; 9] =
    ["nfeProc", "NFe", "procNFe", "cteProc", "CTe", "procEventoNFe", "resNFe", "resEvento", "evento"];

/// Quantos lugares de começo o corte tenta, no máximo (cada tentativa lê o texto uma vez).
const TENTATIVAS: usize = 16;

/// O texto já abre como XML? (Sem DTD e com limite de nós, como o verificador da assinatura.)
fn abre(texto: &str) -> bool {
    ler_xml(texto).is_some()
}

/// Lê o texto como XML, com as mesmas proteções do verificador da assinatura: sem DTD, com limite de nós e, **antes**,
/// a profundidade (o leitor é recursivo, e um arquivo hostil profundo demais derrubaria o programa: achado pelo teste
/// do arquivo hostil).
fn ler_xml(texto: &str) -> Option<roxmltree::Document<'_>> {
    use crate::nfe_assinatura::{profundidade_excessiva, PROFUNDIDADE_MAXIMA};
    if profundidade_excessiva(texto, PROFUNDIDADE_MAXIMA) {
        return None;
    }
    let opcoes = roxmltree::ParsingOptions { allow_dtd: false, nodes_limit: 1_000_000, ..Default::default() };
    roxmltree::Document::parse_with_options(texto, opcoes).ok()
}

/// BUG-20261128-XDCL (diretriz do dono, 28/11: o Rustinho lê o XML "independente de cabeçalho" e contorna os erros
/// irrelevantes, como o sistema antigo da loja).
/// 1. Tira espaços, bytes nulos e marcas de BOM (repetidas ou já viradas texto, `ï»¿`) do começo e do fim.
/// 2. **Se o que sobra já abre como XML, é ele** (um arquivo certo nunca é cortado; B-1 da revisão).
/// 3. Senão, procura o documento: os começos de elementos fiscais conhecido (`RAIZES_FISCAIS`, com ou sem prefixo), na
///    ordem, e por último o primeiro elemento; comentários, CDATA, instruções e declarações são pulados na procura.
///    Para cada começo, corta antes dele (se o que vem antes não for só uma declaração bem formada e comentários) e
///    depois do último fechamento dele; o primeiro corte que abre é o documento.
/// 4. **Mais de uma nota no arquivo** (mais de um `infNFe` com atributos: toda nota tem exatamente um, com `Id`; o do
///    CT-e e do MDF-e, só com a chave, e um nome solto no lixo não contam): não se corta nada, para nunca ficar com uma nota só sem avisar (B-2 da revisão).
/// 5. Nada abriu: volta o texto como veio (e a tela explica em português; B-7 da revisão: nunca um texto vazio sem
///    motivo).
///
/// Nada disso muda a nota: o que está fora do elemento raiz não faz parte da parte assinada (a assinatura cobre o
/// `infNFe`). O tempo é linear no tamanho do arquivo (no máximo `TENTATIVAS` leituras; M-1 da revisão).
fn so_a_nota(texto: String) -> String {
    let sujo = |c: char| c.is_whitespace() || c == '\0' || c == '\u{feff}';
    let mut inicio = texto.trim_start_matches(sujo);
    while let Some(r) = inicio.strip_prefix("ï»¿") {
        inicio = r.trim_start_matches(sujo);
    }
    let inicio = inicio.trim_end_matches(sujo);
    let aparado = |t: &str| if t.len() == texto.len() { None } else { Some(t.to_string()) };
    if abre(inicio) {
        return extrair_a_nota(inicio).or_else(|| aparado(inicio)).unwrap_or(texto);
    }
    let achados = achar(inicio);
    if achados.notas > 1 {
        return texto;
    }
    let candidatos: Vec<(usize, &str)> =
        achados.fiscais.iter().copied().chain(achados.primeiro).take(TENTATIVAS).collect();
    for (pos, nome) in candidatos {
        let mut fim = inicio.len();
        let fecho = format!("</{nome}");
        if let Some(k) = inicio.rfind(&fecho).filter(|&k| k > pos) {
            let depois = &inicio[k + fecho.len()..];
            let espacos = depois.len() - depois.trim_start().len();
            if depois.trim_start().starts_with('>') {
                fim = k + fecho.len() + espacos + 1;
            }
        } else if let Some(f) = inicio[pos..].find('>') {
            // Sem fechamento: a raiz que se fecha sozinha (`<NFe/>`) termina no `/>` dela.
            if inicio[pos..pos + f].ends_with('/') {
                fim = pos + f + 1;
            }
        }
        let comeco = if prologo_limpo(&inicio[..pos]) { 0 } else { pos };
        let corte = &inicio[comeco..fim];
        if abre(corte) {
            return extrair_a_nota(corte).unwrap_or_else(|| corte.to_string());
        }
    }
    texto
}

/// O texto abre, mas a raiz não é um documento fiscal e há **uma** nota dentro (uma página com a nota, um lote
/// `enviNFe` com uma nota só): fica só ela. As declarações de espaço de nomes que ela herdava do embrulho são repetidas
/// no começo dela, para a leitura e a conferência da assinatura serem as mesmas. Mais de uma nota, ou nenhuma: `None`.
fn extrair_a_nota(texto: &str) -> Option<String> {
    let doc = ler_xml(texto)?;
    let fiscal = |n: &roxmltree::Node| n.is_element() && RAIZES_FISCAIS.contains(&n.tag_name().name());
    let raiz = doc.root_element();
    if fiscal(&raiz) {
        return None;
    }
    if raiz.descendants().filter(|n| n.is_element() && n.tag_name().name() == "infNFe").count() > 1 {
        return None;
    }
    let nota = raiz.descendants().find(fiscal)?;
    let trecho = &texto[nota.range()];
    // O nome do elemento como está escrito (com o prefixo) e o fim dele no trecho.
    let fim_nome = trecho[1..].find(|c: char| c.is_whitespace() || matches!(c, '>' | '/'))? + 1;
    // B-1 da R4: a abertura é medida uma vez, e uma abertura gigante ou com espaços de nomes demais (só num arquivo
    // hostil: uma nota tem 2 ou 3) não é extraída.
    let abertura = &trecho[..trecho.find('>')?];
    if abertura.len() > 64 * 1024 || nota.namespaces().len() > 32 {
        return None;
    }
    // B-6 da R4 e B-2 da R5: os espaços de nomes que a nota usa, pela árvore (os nomes dos elementos e dos atributos),
    // e não pelo texto (um "a:" num texto livre não conta). B-1 da R6: pelo par (prefixo como está escrito, endereço),
    // e não só pelo endereço: um `xmlns:ns2` do embrulho com o mesmo endereço do `xmlns` da nota não é usado por ela.
    let usados: std::collections::HashSet<(Option<&str>, &str)> = nota
        .descendants()
        .filter(|n| n.is_element())
        .flat_map(|n| {
            let elemento = n.tag_name().namespace().map(|uri| (prefixo_escrito(texto, n.range().start + 1), uri));
            let atributos = n
                .attributes()
                .filter_map(|a| a.namespace().map(|uri| (prefixo_escrito(texto, a.range_qname().start), uri)));
            elemento.into_iter().chain(atributos)
        })
        .collect();
    let mut acrescimo = String::new();
    for ns in nota.namespaces().filter(|ns| ns.name() != Some("xml")) {
        let atributo = match ns.name() {
            Some(prefixo) => format!("xmlns:{prefixo}="),
            None => "xmlns=".to_string(),
        };
        // Só os que a nota usa: um prefixo só do embrulho entraria na canonicalização e derrubaria a assinatura de uma
        // nota boa.
        let usado = usados.contains(&(ns.name(), ns.uri()));
        if usado && !abertura.contains(&atributo) {
            acrescimo.push_str(&format!(" {atributo}\"{}\"", escapar_atributo(ns.uri())));
        }
    }
    let extraido = format!("{}{}{}", &trecho[..fim_nome], acrescimo, &trecho[fim_nome..]);
    abre(&extraido).then_some(extraido)
}

/// O prefixo de um nome como está escrito no texto, a partir do começo do nome (`q` em `q:NFe`; nenhum em `NFe`).
fn prefixo_escrito(texto: &str, inicio: usize) -> Option<&str> {
    let resto = &texto[inicio..];
    let nome = &resto[..resto.find(|c: char| c.is_whitespace() || matches!(c, '>' | '/' | '=')).unwrap_or(resto.len())];
    nome.split_once(':').map(|(p, _)| p)
}

/// O valor de um atributo escrito de volta no XML (B-6 da revisão): `&`, `<`, `"` e os espaços especiais com escape.
fn escapar_atributo(v: &str) -> String {
    let mut s = String::with_capacity(v.len());
    for c in v.chars() {
        match c {
            '&' => s.push_str("&amp;"),
            '<' => s.push_str("&lt;"),
            '"' => s.push_str("&quot;"),
            '\t' => s.push_str("&#9;"),
            '\n' => s.push_str("&#10;"),
            '\r' => s.push_str("&#13;"),
            _ => s.push(c),
        }
    }
    s
}

/// O que a procura achou, com a memória limitada (B-1 da revisão): o primeiro elemento, até `TENTATIVAS` começos de
/// elementos fiscais, e quantas notas (`infNFe`), contadas até 2.
struct Achados<'a> {
    primeiro: Option<(usize, &'a str)>,
    fiscais: Vec<(usize, &'a str)>,
    notas: usize,
}

/// Quantas notas (`infNFe` com atributos) o texto tem, contadas até 2 (para dizer "mais de uma nota").
pub(crate) fn notas_no_texto(t: &str) -> usize {
    achar(t).notas
}

/// Os começos de elemento, em ordem, pulando comentários, CDATA, instruções de processamento e declarações (`<!…>`).
/// Linear (M-1): cada nome termina em espaço, `>`, `/` ou no próximo `<`, e a procura pula o nome inteiro. Para assim que
/// já tem o que precisa (o primeiro, `TENTATIVAS` fiscais e duas notas).
fn achar(t: &str) -> Achados<'_> {
    let mut a = Achados { primeiro: None, fiscais: Vec::new(), notas: 0 };
    let mut i = 0;
    while let Some(d) = t[i..].find('<') {
        let p = i + d;
        let resto = &t[p + 1..];
        // B-3 da R4: o fim é procurado depois do abridor (`<!-->` não fecha o comentário).
        let pular =
            |abre: usize, fim: &str| resto[abre..].find(fim).map(|f| p + 1 + abre + f + fim.len()).unwrap_or(t.len());
        i = if resto.starts_with("!--") {
            pular(3, "-->")
        } else if resto.starts_with("![CDATA[") {
            pular(8, "]]>")
        } else if resto.starts_with('?') {
            pular(1, "?>")
        } else if resto.starts_with('!') {
            pular(1, ">")
        } else if resto.starts_with(|c: char| c.is_alphabetic() || c == '_') {
            let n = resto.find(|c: char| c.is_whitespace() || matches!(c, '>' | '/' | '<')).unwrap_or(resto.len());
            let nome = &resto[..n];
            let local = nome.rsplit(':').next().unwrap_or(nome);
            a.primeiro.get_or_insert((p, nome));
            // M-1 da R4: só conta como nota o `infNFe` com atributos (o da NF-e sempre tem `Id` e `versao`); o do
            // CT-e e do MDF-e, que só cita a chave das notas transportadas, não tem.
            if local == "infNFe" && resto[n..].starts_with(char::is_whitespace) {
                a.notas = (a.notas + 1).min(2);
            }
            if RAIZES_FISCAIS.contains(&local) && a.fiscais.len() < TENTATIVAS {
                a.fiscais.push((p, nome));
            }
            if a.notas >= 2 && a.fiscais.len() >= TENTATIVAS {
                break;
            }
            p + 1 + n
        } else {
            p + 1
        };
        if i >= t.len() {
            break;
        }
    }
    a
}

/// O que vem antes do documento é só uma declaração XML bem formada, espaços e comentários (o que um arquivo certo tem).
fn prologo_limpo(antes: &str) -> bool {
    let mut r = antes.trim();
    if let Some(x) = r.strip_prefix("<?xml") {
        let Some(f) = x.find("?>") else { return false };
        if x[..f].contains(['<', '>']) {
            return false;
        }
        r = x[f + 2..].trim_start();
    }
    while let Some(x) = r.strip_prefix("<!--") {
        let Some(f) = x.find("-->") else { return false };
        r = x[f + 3..].trim_start();
    }
    r.is_empty()
}

/// O texto já está em UTF-8: a declaração `<?xml … encoding="…"?>` passa a
/// dizer isso (o leitor de XML recusa outra codificação declarada).
fn declarar_utf8(texto: String) -> String {
    if !texto.starts_with("<?xml") {
        return texto;
    }
    let Some(fim) = texto.find("?>") else { return texto };
    let Some(i) = texto[..fim].find("encoding") else { return texto };
    let b = texto.as_bytes();
    let mut j = i + "encoding".len();
    while j < fim && b[j].is_ascii_whitespace() {
        j += 1;
    }
    if j >= fim || b[j] != b'=' {
        return texto;
    }
    j += 1;
    while j < fim && b[j].is_ascii_whitespace() {
        j += 1;
    }
    if j >= fim || (b[j] != b'"' && b[j] != b'\'') {
        return texto;
    }
    let inicio = j + 1;
    let Some(k) = texto[inicio..fim].find(b[j] as char) else { return texto };
    if texto[inicio..inicio + k].eq_ignore_ascii_case("utf-8") {
        return texto;
    }
    format!("{}UTF-8{}", &texto[..inicio], &texto[inicio + k..])
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
    xmls_com(entradas, Orcamento::novo(), MAX_MEMBROS)
}

fn xmls_com(entradas: &[String], mut orc: Orcamento, mut olhar: usize) -> Resultado<Vec<Xml>> {
    let mut lista = Vec::new();
    let mut inteiro = true;
    let mut fundo = false;
    for e in entradas {
        let p = caminho_local(e);
        if !p.exists() {
            return Err(Erro::validacao(format!("Não achei {}.", p.display())));
        }
        if p.is_file() && !aceito(&nome_curto(&p)) {
            return Err(Erro::validacao(format!("{} não é XML, ZIP nem RAR.", nome_curto(&p))));
        }
        inteiro &= juntar(&p, &mut lista, 0, &mut olhar, &mut fundo);
    }
    let mut saida = Vec::new();
    // T087 (OPUS-C-10): a subpasta funda demais não some em silêncio.
    if fundo {
        saida.push(ruim(
            "pasta".into(),
            "há subpastas com mais de 6 níveis, e os arquivos delas não foram olhados. Escolha a pasta de dentro",
        ));
    }
    if !inteiro {
        saida.push(ruim(
            "pasta".into(),
            &format!("arquivos demais; só os primeiros {} foram olhados. Escolha menos arquivos", lista.len()),
        ));
    }
    for p in lista {
        let nome = nome_curto(&p);
        let Ok(meta) = std::fs::metadata(&p) else { continue };
        match extensao(&p).as_str() {
            "xml" => {
                if meta.len() > MAX_TAMANHO {
                    saida.push(ruim(nome, "grande demais para ser uma nota"));
                    continue;
                }
                // R-03: XML solto também conta no limite total.
                if !orc.gastar(meta.len()) {
                    saida.push(ruim(nome, &passou_do_total()));
                    break;
                }
                // Lê no máximo o limite (o arquivo pode crescer depois de olhado).
                let lido = std::fs::File::open(&p).map(|mut f| ler_ate(&mut f, MAX_TAMANHO));
                match lido {
                    Ok(Some(b)) => saida.push(Xml { nome, texto: texto_do_xml(&b), erro: String::new() }),
                    Ok(None) => saida.push(ruim(nome, "grande demais para ser uma nota")),
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

fn passou_do_total() -> String {
    format!("os arquivos escolhidos passam de {} MB; escolha menos arquivos", MAX_TOTAL / (1024 * 1024))
}

fn de_zip<R: Read + std::io::Seek>(
    pacote: &str,
    leitor: R,
    saida: &mut Vec<Xml>,
    entrar_em_zip: bool,
    orc: &mut Orcamento,
) {
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
        // zip 9: o nome pode não ser lido (fica de fora, como uma entrada estragada).
        let Ok(nome_no_zip) = f.name() else { continue };
        let interno = nome_no_zip.rsplit('/').next().unwrap_or_default().to_string();
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
            saida.push(ruim(pacote.into(), &passou_do_total()));
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
            saida.push(ruim(pacote.into(), &passou_do_total()));
            return;
        }
        let mut opcoes = rars::ArchiveReadOptions::new();
        opcoes.max_member_output_bytes = Some(MAX_TAMANHO);
        opcoes.max_total_output_bytes = Some(MAX_TOTAL);
        opcoes.rar50_dictionary_size_limit = Some(limites::MAX_DICIONARIO_RAR as _);
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

    /// T087 (OPUS-C-10 da auditoria da 0.9.18-dev.3.3.3): os XMLs de subpastas com mais de 6 níveis eram
    /// deixados de fora sem aviso nenhum (nem o de corte). Agora vem um aviso dizendo que parte não foi olhada.
    #[test]
    fn subpastas_fundas_demais_avisam() {
        let raiz = std::env::temp_dir().join(format!("rustinho-arquivos-t087-{}", crate::tempo::novo_id()));
        let fundo = raiz.join("a/b/c/d/e/f/g/h");
        std::fs::create_dir_all(&fundo).unwrap();
        std::fs::write(fundo.join("nota.xml"), "<nfeProc/>").unwrap();
        let lista = xmls(&[raiz.display().to_string()]).unwrap();
        assert!(
            lista.iter().any(|x| !x.erro.is_empty() && x.erro.contains("não foram olhados")),
            "a nota da subpasta funda sumiu sem aviso: {:?}",
            lista.iter().map(|x| (&x.nome, &x.erro)).collect::<Vec<_>>()
        );
        let _ = std::fs::remove_dir_all(&raiz);
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

    /// F06 (dev.3.3, 2.4): Latin-1 com aspas simples na declaração e
    /// Windows-1252 (aspas curvas, travessão, € nos bytes 0x80 a 0x9F) viram
    /// o texto certo, com a declaração dizendo UTF-8, e o XML abre.
    #[test]
    fn f06_latin1_com_aspas_simples_e_windows_1252() {
        let t = texto_do_xml(b"<?xml version='1.0' encoding='ISO-8859-1'?><x>S\xC3O</x>");
        assert!(t.contains("encoding='UTF-8'") && t.contains("SÃO"), "{t}");
        assert!(roxmltree::Document::parse(&t).is_ok(), "não abriu: {t}");
        let t = texto_do_xml(b"<?xml version=\"1.0\" encoding=\"windows-1252\"?><x>\x93Pe\xE7a\x94 \x96 \x80 10</x>");
        assert!(t.contains("encoding=\"UTF-8\""), "{t}");
        assert!(t.contains("\u{201c}Peça\u{201d} \u{2013} € 10"), "{t}");
        assert!(roxmltree::Document::parse(&t).is_ok(), "não abriu: {t}");
        // Só ASCII (é UTF-8 também), mas declarado ISO-8859-1: abre.
        let t = texto_do_xml(b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><x>SAO</x>");
        assert!(roxmltree::Document::parse(&t).is_ok(), "não abriu: {t}");
        // O que já era UTF-8 fica igual.
        let utf8 = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><x>São “ok”</x>";
        assert_eq!(texto_do_xml(utf8.as_bytes()), utf8);
    }

    /// BUG-20261128-XDCL (diretriz do dono, 28/11): o Rustinho lê o XML da nota independente do que vier em volta dela.
    /// Um fornecedor (sistema emissor `verProc 1.4.1`) grava `??xml version="1.0"?>` no lugar de `<?xml …?>`; outros
    /// arquivos trazem texto solto, marcas de BOM repetidas ou já viradas texto, bytes nulos, lixo depois do fim, ou vêm
    /// em UTF-16. Fica só a nota; o que está fora dela não é assinado.
    #[test]
    fn xdcl_le_a_nota_apesar_do_que_vem_em_volta() {
        // A declaração estragada, de vários jeitos.
        assert_eq!(texto_do_xml(b"??xml version=\"1.0\"?><NFe><x/></NFe>"), "<NFe><x/></NFe>");
        assert_eq!(texto_do_xml(b"?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<NFe/>"), "<NFe/>");
        assert_eq!(texto_do_xml(b"?????xml version=\"1.0\"?><NFe/>"), "<NFe/>");
        assert_eq!(texto_do_xml(b"??XML VERSION=\"1.0\"??<NFe/>"), "<NFe/>");
        assert_eq!(texto_do_xml(b"?<?xml version=\"1.0\"?><NFe/>"), "<NFe/>");
        // Texto solto, cabeçalho de outro formato, comentário estragado, BOM repetido ou virado texto, bytes nulos.
        assert_eq!(
            texto_do_xml(b"lixo qualquer\r\n<nfeProc versao=\"4.00\"><NFe/></nfeProc>"),
            "<nfeProc versao=\"4.00\"><NFe/></nfeProc>"
        );
        assert_eq!(
            texto_do_xml(b"<html><body><pre><nfeProc><NFe/></nfeProc></pre></body></html>"),
            "<nfeProc><NFe/></nfeProc>"
        );
        assert_eq!(texto_do_xml(&[[0xEF, 0xBB, 0xBF, 0xEF, 0xBB, 0xBF].as_slice(), b"<NFe/>"].concat()), "<NFe/>");
        assert_eq!(texto_do_xml("ï»¿<?xml version=\"1.0\"?><NFe/>".as_bytes()), "<?xml version=\"1.0\"?><NFe/>");
        assert_eq!(texto_do_xml(b"\0\0\0<NFe/>\0\0"), "<NFe/>");
        // Lixo depois do fim da nota.
        assert_eq!(
            texto_do_xml(b"<nfeProc><NFe/></nfeProc>\r\nfim do arquivo <assinatura/>"),
            "<nfeProc><NFe/></nfeProc>"
        );
        // Com prefixo de espaço de nomes.
        assert_eq!(
            texto_do_xml(b"??xml?><ns2:nfeProc xmlns:ns2=\"x\"><ns2:NFe/></ns2:nfeProc>xx"),
            "<ns2:nfeProc xmlns:ns2=\"x\"><ns2:NFe/></ns2:nfeProc>"
        );
        // Latin-1 com a declaração estragada: a codificação ainda é lida certa.
        let t = texto_do_xml(b"??xml version=\"1.0\" encoding=\"ISO-8859-1\"?><x>S\xC3O</x>");
        assert_eq!(t, "<x>SÃO</x>");
        assert!(roxmltree::Document::parse(&t).is_ok());
        // UTF-16 (o "Unicode" do Bloco de Notas), com e sem a marca, nas duas ordens.
        let u = "<?xml version=\"1.0\" encoding=\"UTF-16\"?><NFe>Peça</NFe>";
        let le: Vec<u8> = u.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
        let be: Vec<u8> = u.encode_utf16().flat_map(|c| c.to_be_bytes()).collect();
        let esperado = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><NFe>Peça</NFe>";
        assert_eq!(texto_do_xml(&[[0xFF, 0xFE].as_slice(), &le].concat()), esperado);
        assert_eq!(texto_do_xml(&[[0xFE, 0xFF].as_slice(), &be].concat()), esperado);
        assert_eq!(texto_do_xml(&le), esperado);
        assert_eq!(texto_do_xml(&be), esperado);
        // O que já estava certo fica igual, inclusive a declaração e os comentários antes da nota.
        for certo in [
            "<?xml version=\"1.0\"?><NFe/>",
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- gerado por x -->\n<nfeProc><NFe/></nfeProc>",
            "<NFe><x>a &lt; b</x></NFe>",
        ] {
            assert_eq!(texto_do_xml(certo.as_bytes()), certo);
        }
        // M-2 da revisão: um byte estragado no lixo não estraga os acentos da nota (UTF-8).
        assert_eq!(
            texto_do_xml(b"\x80\x81 lixo\r\n<nfeProc><x>Pe\xC3\xA7a</x></nfeProc>\xFF"),
            "<nfeProc><x>Peça</x></nfeProc>"
        );
        // B-1: o que já abre como XML não é cortado, mesmo com nomes fiscais em comentários; o lixo com nome fiscal
        // antes da nota não engana.
        for certo in
            ["<!-- gerado por <NFe> -->\n<nfeProc><NFe/></nfeProc>", "<nfeProc><NFe/></nfeProc><!-- </nfeProc> -->"]
        {
            assert_eq!(texto_do_xml(certo.as_bytes()), certo);
        }
        assert_eq!(texto_do_xml(b"Segue a <NFe> em anexo\n<NFe xmlns=\"x\"><a/></NFe>"), "<NFe xmlns=\"x\"><a/></NFe>");
        // Um embrulho que abre como XML com UMA nota dentro (um lote enviNFe de uma nota): fica a nota, com o espaço de
        // nomes que ela herdava. Com duas notas, nada é cortado (B-2).
        assert_eq!(
            texto_do_xml(b"<enviNFe xmlns=\"http://www.portalfiscal.inf.br/nfe\" versao=\"4.00\"><idLote>1</idLote><NFe><infNFe Id=\"a\"/></NFe></enviNFe>"),
            "<NFe xmlns=\"http://www.portalfiscal.inf.br/nfe\"><infNFe Id=\"a\"/></NFe>"
        );
        let duas = "<enviNFe><NFe><infNFe Id=\"a\"/></NFe><NFe><infNFe Id=\"b\"/></NFe></enviNFe>";
        assert_eq!(texto_do_xml(duas.as_bytes()), duas);
        let duas_com_lixo = format!("lixo {duas}");
        assert_eq!(texto_do_xml(duas_com_lixo.as_bytes()), duas_com_lixo);
        // O evento também passa pelo mesmo caminho.
        assert_eq!(
            texto_do_xml(b"lixo<procEventoNFe><evento/></procEventoNFe>"),
            "<procEventoNFe><evento/></procEventoNFe>"
        );
        // Um DOCTYPE com entidade não vira uma nota que abre (o leitor continua recusando entidades).
        assert!(!abre(&texto_do_xml(b"<!DOCTYPE x [<!ENTITY b \"bomba\">]><nfeProc>&b;</nfeProc>")));
        // Lixo com emoji e acentos dos dois lados; UTF-16 de tamanho ímpar e com substituto solto: sem pânico.
        assert_eq!(texto_do_xml("😀 lixo ção <NFe/> 😀 fim".as_bytes()), "<NFe/>");
        let mut impar: Vec<u8> = "<NFe/>".encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
        impar.push(0x41);
        assert_eq!(texto_do_xml(&impar), "<NFe/>");
        let solto = [0xFF, 0xFE, b'<', 0, b'a', 0, 0x00, 0xD8, b'/', 0, b'>', 0];
        assert!(texto_do_xml(&solto).contains('\u{FFFD}'));
        // Sem nenhum elemento, não há o que salvar: fica como está (e a tela explica em português).
        for nada in ["", "sem sinal", "????"] {
            assert_eq!(texto_do_xml(nada.as_bytes()), nada);
        }
    }

    /// M-1 e B-3 da revisão: arquivos hostis (milhões de `<` sem fechamento, milhares de começos de nota, comentários
    /// abertos, bytes inválidos) não travam a leitura: o tempo é linear. Cada um roda numa linha à parte com prazo, para
    /// o teste reprovar em vez de travar a bateria se o tempo voltar a crescer.
    #[test]
    fn xdcl_arquivo_hostil_nao_trava() {
        let casos: Vec<Vec<u8>> = vec![
            "<a".repeat(1_000_000).into_bytes(),
            "<NFe>".repeat(200_000).into_bytes(),
            "<!--".repeat(500_000).into_bytes(),
            [b"\x80\x81".as_slice(), &b"<infNFe x=\"\xC3\">".repeat(100_000)].concat(),
        ];
        for (i, caso) in casos.into_iter().enumerate() {
            let (tx, rx) = std::sync::mpsc::channel();
            let entrada = caso.clone();
            std::thread::spawn(move || {
                let _ = tx.send(texto_do_xml(&caso));
            });
            let saida = rx
                .recv_timeout(std::time::Duration::from_secs(30))
                .unwrap_or_else(|_| panic!("o caso hostil {i} não terminou em 30 s"));
            // B-5 da R4: sem nota nenhuma, o texto volta como veio (os três primeiros são UTF-8 válido).
            if i < 3 {
                assert_eq!(saida.as_bytes(), entrada.as_slice(), "caso hostil {i}");
            }
        }
    }

    /// B-5 da R4: a leitura protegida aceita a profundidade de uma nota (até 64) e recusa a de um arquivo hostil (65 ou
    /// mais), sem derrubar o programa.
    #[test]
    fn xdcl_profundidade_no_limite() {
        let aninhado = |n: usize| format!("{}{}", "<a>".repeat(n), "</a>".repeat(n));
        assert!(abre(&aninhado(64)));
        assert!(!abre(&aninhado(65)));
        assert!(!abre(&aninhado(200_000)));
    }

    /// B-1 da R5: a extração não leva uma nota com espaços de nomes demais (só num arquivo hostil), e leva a de uma nota
    /// normal (com 3).
    #[test]
    fn xdcl_extracao_limita_os_espacos_de_nomes() {
        let muitos: String = (0..40).map(|i| format!(" xmlns:p{i}=\"u{i}\"")).collect();
        let hostil = format!("<pagina{muitos}><NFe><infNFe Id=\"a\"/></NFe></pagina>");
        assert!(texto_do_xml(hostil.as_bytes()).starts_with("<pagina"), "não extrai");
        let normal = "<pagina xmlns=\"n\" xmlns:a=\"x\" xmlns:b=\"y\"><NFe a:k=\"1\"><infNFe Id=\"a\"/></NFe></pagina>";
        assert_eq!(
            texto_do_xml(normal.as_bytes()),
            "<NFe xmlns=\"n\" xmlns:a=\"x\" a:k=\"1\"><infNFe Id=\"a\"/></NFe>"
        );
        // B-2 da R5: um "b:" no texto livre não faz levar o prefixo b.
        let texto_livre = "<pagina xmlns:b=\"y\"><NFe><infNFe Id=\"a\"><x>Entrega b: rua</x></infNFe></NFe></pagina>";
        assert_eq!(texto_do_xml(texto_livre.as_bytes()), "<NFe><infNFe Id=\"a\"><x>Entrega b: rua</x></infNFe></NFe>");
        // B-3 da R6: o atributo com prefixo depois de um tab (e não de um espaço) também faz levar o prefixo.
        let com_tab = "<pagina xmlns=\"n\" xmlns:a=\"x\"><NFe\ta:k=\"1\"><infNFe Id=\"a\"/></NFe></pagina>";
        assert_eq!(
            texto_do_xml(com_tab.as_bytes()),
            "<NFe xmlns=\"n\" xmlns:a=\"x\"\ta:k=\"1\"><infNFe Id=\"a\"/></NFe>"
        );
    }

    /// B-1 da R6: um prefixo do embrulho que a nota não usa não entra nela, nem quando tem o mesmo endereço de um espaço
    /// de nomes que ela usa (o `xmlns:ns2` de um lote `enviNFe`, o `xmlns:ds` de uma página com a nota assinada): ele
    /// entraria na canonicalização e a nota boa pareceria "alterada depois da assinatura". Vale o par (prefixo escrito,
    /// endereço), e não só o endereço.
    #[test]
    fn xdcl_extracao_leva_o_par_prefixo_e_endereco() {
        let t = texto_do_xml(b"<p xmlns=\"n\" xmlns:q=\"n\"><NFe><infNFe Id=\"a\"/></NFe></p>");
        assert_eq!(t, "<NFe xmlns=\"n\"><infNFe Id=\"a\"/></NFe>");
        let nfe = "http://www.portalfiscal.inf.br/nfe";
        let lote = format!(
            "<ns2:enviNFe xmlns:ns2=\"{nfe}\"><ns2:idLote>1</ns2:idLote><NFe xmlns=\"{nfe}\"><infNFe Id=\"a\"/></NFe>\
             </ns2:enviNFe>"
        );
        assert_eq!(texto_do_xml(lote.as_bytes()), format!("<NFe xmlns=\"{nfe}\"><infNFe Id=\"a\"/></NFe>"));
        let ds = "http://www.w3.org/2000/09/xmldsig#";
        let pagina =
            format!("<p xmlns:ds=\"{ds}\"><NFe xmlns=\"n\"><infNFe Id=\"a\"/><Signature xmlns=\"{ds}\"/></NFe></p>");
        assert_eq!(
            texto_do_xml(pagina.as_bytes()),
            format!("<NFe xmlns=\"n\"><infNFe Id=\"a\"/><Signature xmlns=\"{ds}\"/></NFe>")
        );
        // O prefixo que a nota usa no nome dela continua sendo levado.
        let t = texto_do_xml(b"<p xmlns:q=\"n\" xmlns:r=\"n\"><q:NFe><q:infNFe Id=\"a\"/></q:NFe></p>");
        assert_eq!(t, "<q:NFe xmlns:q=\"n\"><q:infNFe Id=\"a\"/></q:NFe>");
    }

    /// B-4 e B-6 da R4: na extração de uma nota de um embrulho, o endereço do espaço de nomes é escrito com escape
    /// (`&amp;` continua `&amp;`), e um prefixo que só o embrulho declara não entra na nota.
    #[test]
    fn xdcl_extracao_escapa_e_so_leva_o_que_a_nota_usa() {
        let t = texto_do_xml(b"<pagina xmlns=\"http://a/?x=1&amp;y=2\"><NFe><infNFe Id=\"a\"/></NFe></pagina>");
        assert_eq!(t, "<NFe xmlns=\"http://a/?x=1&amp;y=2\"><infNFe Id=\"a\"/></NFe>");
        let t = texto_do_xml(
            b"<s:Envelope xmlns:s=\"http://soap\" xmlns=\"http://nfe\"><s:Body><NFe><infNFe Id=\"a\"/></NFe></s:Body></s:Envelope>",
        );
        assert_eq!(t, "<NFe xmlns=\"http://nfe\"><infNFe Id=\"a\"/></NFe>");
    }

    /// B-7 da revisão: um arquivo só com espaços ou bytes nulos não vira um texto vazio (que quer dizer "não deu para
    /// ler" sem o motivo).
    #[test]
    fn xdcl_arquivo_so_com_espacos_fica_como_veio() {
        assert_eq!(texto_do_xml(b"   \r\n\0\0 "), "   \r\n\0\0 ");
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
        std::fs::write(
            pasta.join("sub/pacote.zip"),
            fazer_zip(&[("a.xml", b"<a/>"), ("leia.txt", b"x"), ("outro.zip", &dentro)]),
        )
        .unwrap();
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

    /// R-03 e item 5 da auditoria: endereço malformado não derruba.
    #[test]
    fn endereco_malformado() {
        for t in ["%", "%2", "%G1", "%é", "a%ã", "%%", "x%2€", "file:///%E9%"] {
            let r = std::panic::catch_unwind(|| caminho_local(t));
            assert!(r.is_ok(), "{t}");
        }
        assert_eq!(caminho_local("a%2Fb%20c"), PathBuf::from("a/b c"));
        assert_eq!(caminho_local("%C3%A9"), PathBuf::from("é"));
    }

    /// R-03: XMLs soltos contam no limite total, e pasta enorme para cedo.
    #[test]
    fn limite_para_xml_solto_e_pasta() {
        let pasta = std::env::temp_dir().join(format!("rustinho-limite-{}", crate::tempo::novo_id()));
        std::fs::create_dir_all(pasta.join("sub")).unwrap();
        for i in 0..10 {
            std::fs::write(pasta.join(format!("n{i:02}.xml")), b"<nota>0123456789</nota>").unwrap();
            std::fs::write(pasta.join(format!("foto{i}.jpg")), b"x").unwrap();
        }
        let todos = || vec![pasta.display().to_string()];
        // Sem aperto: os 10.
        let l = xmls_com(&todos(), Orcamento { bytes: 10_000, membros: 100 }, 1000).unwrap();
        assert_eq!(l.iter().filter(|x| x.erro.is_empty()).count(), 10);
        // Limite total de bytes: 23 bytes cada; cabem 4 em 100.
        let l = xmls_com(&todos(), Orcamento { bytes: 100, membros: 100 }, 1000).unwrap();
        assert_eq!(l.iter().filter(|x| x.erro.is_empty()).count(), 4);
        assert!(l.last().unwrap().erro.contains("escolha menos"), "{l:?}");
        // Itens olhados: para cedo e avisa, sem ler tudo.
        let l = xmls_com(&todos(), Orcamento { bytes: 10_000, membros: 100 }, 8).unwrap();
        assert!(l[0].erro.contains("arquivos demais"), "{l:?}");
        assert!(l.iter().filter(|x| x.erro.is_empty()).count() < 10);
        let _ = std::fs::remove_dir_all(&pasta);
    }

    include!("arquivos_teste_rar.rs");
}
