//! Passo 7.2 (dev.3.3): travas que leem o texto dos QML, para o que o roteiro
//! de tela não alcança.
//!
//! - **H05**: o rótulo dos campos de formulário (`Campo`, `Selecao`,
//!   `AreaTexto`) segue a superfície embaixo dele (`Tema.apagadoNo`,
//!   `Tema.vermelhoNo`), como o `Rotulo` comum. Com a cor fixa (`Tema.apagado`,
//!   `Tema.vermelho`), um rótulo sobre o fundo roxo ou a faixa escura do roxo
//!   fica quase invisível. Nenhuma tela de hoje põe um `Selecao` ou `AreaTexto`
//!   com rótulo sobre uma superfície escura (a revisão visual não achou), então
//!   a trava é do componente, e o contraste é calculado das cores do `Tema.qml`.
//! - **H10**: um dado (nome, observação, código…) num texto do Qt nunca vira
//!   formatação. Os textos formatados de propósito escapam os dados
//!   (`Tema.escapar`); os outros declaram o texto puro (extensão da f7c: o
//!   formato automático do Qt, o `Text.AutoText`, lia como HTML o dado que
//!   parecia HTML).
//!
//! A igualdade das duas cópias (computador e Android) é do `copias_android.rs`.

use std::path::{Path, PathBuf};

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn ler(caminho: &str) -> String {
    std::fs::read_to_string(raiz().join(caminho)).unwrap_or_else(|e| panic!("{caminho}: {e}"))
}

/// O bloco `Rotulo { … }` de um componente (o primeiro, que é o do rótulo).
fn bloco_do_rotulo(qml: &str) -> &str {
    let inicio = qml.find("    Rotulo {").expect("o componente tem um Rotulo");
    let fim = inicio + qml[inicio..].find("\n    }").expect("fim do bloco do Rotulo");
    &qml[inicio..fim]
}

/// Cor "#RRGGBB" de uma propriedade do Tema.qml; `tema` escolhe o ramo
/// (`roxo ? A : escuro ? B : C`) ou a constante sem ramo.
fn cor(tema_qml: &str, nome: &str, ramo: usize) -> (u8, u8, u8) {
    let chave = format!("readonly property color {nome}: ");
    let linha =
        tema_qml.lines().find(|l| l.trim_start().starts_with(&chave)).unwrap_or_else(|| panic!("{nome} no Tema"));
    let cores: Vec<&str> = linha.split('"').filter(|p| p.starts_with('#') && p.len() == 7).collect();
    let h = cores.get(ramo).or(cores.first()).unwrap_or_else(|| panic!("{nome} sem cor"));
    let v = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap();
    (v(1), v(3), v(5))
}

fn contraste(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    fn lin(c: u8) -> f64 {
        let c = c as f64 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    let l = |(r, g, b): (u8, u8, u8)| 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
    let (x, y) = (l(a), l(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// H05: os três componentes pintam o rótulo pela superfície, e nunca com a cor
/// fixa do tema (a do painel claro).
#[test]
fn h05_rotulo_dos_campos_segue_a_superficie() {
    let mut fora = Vec::new();
    for arquivo in ["app/qml/Campo.qml", "app/qml/Selecao.qml", "app/qml/AreaTexto.qml"] {
        let qml = ler(arquivo);
        let rotulo = bloco_do_rotulo(&qml);
        let linha_cor = rotulo.lines().find(|l| l.trim_start().starts_with("color:")).unwrap_or("");
        let segue = linha_cor.contains("Tema.apagadoNo(") && linha_cor.contains("Tema.vermelhoNo(");
        let fixa = linha_cor.contains("Tema.apagado ")
            || linha_cor.trim_end().ends_with("Tema.apagado")
            || linha_cor.contains("Tema.vermelho ")
            || linha_cor.contains("Tema.vermelho:");
        if !segue || fixa {
            fora.push(format!("{arquivo}: {}", linha_cor.trim()));
        }
    }
    assert!(fora.is_empty(), "rótulo de campo fora da regra de superfície (H05): {fora:#?}");
}

/// H05: a cor que o rótulo usa sobre as superfícies escuras do roxo (o fundo e
/// a faixa escura) tem contraste de texto (>= 4,5:1); a cor fixa de antes não tem.
#[test]
fn h05_contraste_do_rotulo_nas_superficies_escuras_do_roxo() {
    let tema = ler("app/base/Tema.qml");
    let fundo_roxo = cor(&tema, "fundo", 0);
    let faixa_escura = cor(&tema, "roxoFaixaEscura", 0);
    let apagado_claro = cor(&tema, "roxoApagadoClaro", 0);
    let vermelho_claro = cor(&tema, "roxoVermelhoClaro", 0);
    let apagado_fixo = cor(&tema, "apagado", 0);
    for (nome, sup) in [("fundo roxo", fundo_roxo), ("faixa escura", faixa_escura)] {
        for (qual, c) in [("apagado claro", apagado_claro), ("vermelho claro", vermelho_claro)] {
            let r = contraste(c, sup);
            assert!(r >= 4.5, "{qual} sobre {nome}: {r:.2}:1 (< 4,5)");
        }
        // O que a cor fixa dava (o defeito): fica registrado que não serve.
        assert!(contraste(apagado_fixo, sup) < 3.0, "a cor fixa sobre {nome} não era o problema?");
    }
}

/// Os textos formatados (`textFormat: Text.StyledText`) de um QML: a expressão
/// do `text:` logo antes de cada um (até a linha do `textFormat`).
fn textos_formatados(qml: &str) -> Vec<String> {
    let linhas: Vec<&str> = qml.lines().collect();
    let mut lista = Vec::new();
    for (i, l) in linhas.iter().enumerate() {
        if l.trim() != "textFormat: Text.StyledText" {
            continue;
        }
        let Some(inicio) = (0..i).rev().find(|&k| linhas[k].trim_start().starts_with("text:")) else { continue };
        lista.push(linhas[inicio..i].join("\n"));
    }
    lista
}

/// Tira da expressão os trechos já escapados: `Tema.escapar( … )`, com os
/// parênteses equilibrados.
fn sem_escapados(expr: &str) -> String {
    let mut saida = String::new();
    let mut resto = expr;
    while let Some(i) = resto.find("Tema.escapar(") {
        saida.push_str(&resto[..i]);
        let depois = &resto[i + "Tema.escapar(".len()..];
        let mut nivel = 1;
        let mut fim = depois.len();
        for (k, c) in depois.char_indices() {
            match c {
                '(' => nivel += 1,
                ')' => {
                    nivel -= 1;
                    if nivel == 0 {
                        fim = k + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        resto = &depois[fim..];
    }
    saida.push_str(resto);
    saida
}

/// H10: nos textos formatados das telas citadas na auditoria, todo dado que
/// vem do núcleo e é impresso (`modelData.…`, `aviso.…`, `tela.ficha.…`) passa
/// por `Tema.escapar(…)`. Fora dele, o dado só pode aparecer como condição
/// (`=== `, `!== `, `> `, `?`). O roteiro `telas_layout_72.py` (M10) mostra na
/// tela a variação "<i>P</i>" chegando escapada.
#[test]
fn h10_textos_formatados_escapam_os_dados() {
    let mut fora = Vec::new();
    let mut vistos = 0;
    for arquivo in ["app/qml/Notificacao.qml", "app/qml/TelaPeca.qml", "app/qml/TelaConfiguracoes.qml"] {
        let qml = ler(arquivo);
        // A Notificacao monta o texto numa função (linha()).
        let mut exprs = textos_formatados(&qml);
        if let Some(i) = qml.find("function linha()") {
            exprs.push(qml[i..i + qml[i..].find("\n    }").unwrap()].to_string());
        }
        for expr in exprs {
            let resto = sem_escapados(&expr);
            for marca in ["modelData.", "aviso.texto", "aviso.atraso", "tela.ficha."] {
                for (i, _) in resto.match_indices(marca) {
                    vistos += 1;
                    let cauda: String = resto[i + marca.len()..]
                        .trim_start_matches(|c: char| c.is_alphanumeric() || c == '_' || c == '.')
                        .trim_start()
                        .chars()
                        .take(3)
                        .collect();
                    let condicao = ["===", "!==", ">", "<", "?", "|"].iter().any(|p| cauda.starts_with(p))
                        || resto[..i].trim_end().ends_with("if (");
                    if !condicao {
                        let trecho: String = resto[i..].chars().take(40).collect();
                        fora.push(format!("{arquivo}: {trecho}"));
                    }
                }
            }
        }
    }
    assert!(vistos >= 10, "a leitura achou poucos dados nos textos formatados ({vistos})");
    assert!(fora.is_empty(), "dado impresso sem escapar num texto formatado (H10): {fora:#?}");
}

// ---------------------------------------------------------------------------
// H10, extensão (f7c; observação P-4 da revisão cruzada de 13/11): o formato
// automático do Qt.
//
// Um `Text` (ou `Label`) sem `textFormat` usa o `Text.AutoText`: se o texto
// "parece" HTML (`Qt::mightBeRichText`: a primeira marca da primeira linha é
// uma marca conhecida, como `<b>`, `<i>`, `<img …>` ou `<a …>`), ele é
// desenhado como HTML. Um nome de peça, de cliente ou de fornecedor, uma
// observação ou um código vindo da nota, de outro caixa ou digitado vira então
// negrito, imagem ou link, e o resto some ou muda. Os controles do Qt com o
// rótulo padrão (`Button`, `ItemDelegate`…: `IconLabel`) também usam o
// AutoText, e no Qt 6.12 um `<img src=…>` no texto deles fecha o programa
// (sonda da f7c). O roteiro `telas_texto_puro.py` mostra isso na tela.

/// O QML sem comentários (`//` e `/* */`); as strings e as quebras de linha ficam.
fn sem_comentarios(src: &str) -> String {
    let b = src.as_bytes();
    let mut saida = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'"' || c == b'\'' {
            let mut j = i + 1;
            while j < b.len() && b[j] != c && b[j] != b'\n' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            let fim = (j + 1).min(b.len());
            saida.extend_from_slice(&b[i..fim]);
            i = fim;
        } else if b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b[i..].starts_with(b"/*") {
            let fim = b[i + 2..].windows(2).position(|w| w == b"*/").map_or(b.len(), |p| i + 2 + p + 2);
            saida.extend(b[i..fim].iter().filter(|&&x| x == b'\n'));
            i = fim;
        } else {
            saida.push(c);
            i += 1;
        }
    }
    String::from_utf8(saida).expect("o QML sem comentários continua UTF-8")
}

/// O mesmo texto com o miolo das strings trocado por `S` (byte a byte, para as
/// posições continuarem as mesmas): chaves, `;` e `:` dentro de um texto não
/// contam como código.
fn mascarado(limpo: &str) -> Vec<u8> {
    let mut b = limpo.as_bytes().to_vec();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'"' || c == b'\'' {
            let mut j = i + 1;
            while j < b.len() && b[j] != c && b[j] != b'\n' {
                let passo = if b[j] == b'\\' { 2 } else { 1 };
                for k in j..(j + passo).min(b.len()) {
                    b[k] = b'S';
                }
                j += passo;
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    b
}

/// Um objeto do QML: o tipo, a linha e as propriedades diretas
/// (nome → expressão, do texto sem comentários).
struct Objeto {
    tipo: String,
    linha: usize,
    props: Vec<(String, String)>,
}

impl Objeto {
    fn prop(&self, nome: &str) -> Option<&str> {
        self.props.iter().find(|(n, _)| n == nome).map(|(_, e)| e.as_str())
    }
}

fn eh_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'.'
}

/// Os objetos (`Tipo { … }`, com o tipo começando por maiúscula) de um QML.
fn objetos(limpo: &str) -> Vec<Objeto> {
    let m = mascarado(limpo);
    let mut lista = Vec::new();
    for k in 0..m.len() {
        if m[k] != b'{' {
            continue;
        }
        let mut fim_id = k;
        while fim_id > 0 && m[fim_id - 1].is_ascii_whitespace() {
            fim_id -= 1;
        }
        let mut ini_id = fim_id;
        while ini_id > 0 && eh_ident(m[ini_id - 1]) {
            ini_id -= 1;
        }
        if ini_id == fim_id
            || (ini_id > 0 && !(m[ini_id - 1].is_ascii_whitespace() || b":;[{,".contains(&m[ini_id - 1])))
        {
            continue;
        }
        let tipo = &limpo[ini_id..fim_id];
        let ultimo = tipo.rsplit('.').next().unwrap_or("");
        if !ultimo.starts_with(|c: char| c.is_ascii_uppercase()) {
            continue;
        }
        let mut nivel = 0i32;
        let mut fim = m.len();
        for (j, &c) in m.iter().enumerate().skip(k) {
            match c {
                b'{' => nivel += 1,
                b'}' => {
                    nivel -= 1;
                    if nivel == 0 {
                        fim = j;
                        break;
                    }
                }
                _ => {}
            }
        }
        lista.push(Objeto {
            tipo: ultimo.to_string(),
            linha: limpo[..k].matches('\n').count() + 1,
            props: propriedades(limpo, &m, k + 1, fim),
        });
    }
    lista
}

/// As instruções diretas do corpo `[ini, fim)` de um objeto: separadas por `;`
/// ou por quebra de linha fora de parênteses, colchetes e chaves; a linha
/// continua quando termina num operador ou a seguinte começa com um (os
/// ternários quebrados em várias linhas).
fn propriedades(limpo: &str, m: &[u8], ini: usize, fim: usize) -> Vec<(String, String)> {
    const TERMINA: [&str; 13] = ["?", ":", "+", "-", "||", "&&", "(", ",", "=", "*", "/", "<", ">"];
    const COMECA: [&str; 9] = ["?", ":", "+", "||", "&&", ".", ")", "*", "/"];
    let mut partes = Vec::new();
    let mut nivel = 0i32;
    let mut inicio = ini;
    let mut i = ini;
    while i < fim {
        match m[i] {
            b'(' | b'[' | b'{' => nivel += 1,
            b')' | b']' | b'}' => nivel -= 1,
            b';' if nivel == 0 => {
                partes.push((inicio, i));
                inicio = i + 1;
            }
            b'\n' if nivel == 0 => {
                let atual = std::str::from_utf8(&m[inicio..i]).unwrap_or("").trim_end();
                let resto = std::str::from_utf8(&m[i + 1..fim]).unwrap_or("").trim_start();
                let continua = !atual.trim().is_empty()
                    && (TERMINA.iter().any(|t| atual.ends_with(t)) || COMECA.iter().any(|t| resto.starts_with(t)));
                if !continua {
                    partes.push((inicio, i));
                    inicio = i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    partes.push((inicio, fim));
    let mut props = Vec::new();
    for (a, b) in partes {
        let trecho_m = std::str::from_utf8(&m[a..b]).unwrap_or("");
        let Some(dois_pontos) = trecho_m.find(':') else { continue };
        // O nome: o último pedaço antes do ":" ("readonly property string x").
        let antes = trecho_m[..dois_pontos].trim();
        let nome = antes.rsplit(char::is_whitespace).next().unwrap_or("");
        let valido = !nome.is_empty()
            && nome.bytes().all(eh_ident)
            && (antes == nome
                || ["property ", "readonly ", "required ", "default "].iter().any(|p| antes.starts_with(p)));
        if !valido {
            continue;
        }
        let expr = limpo[a + dois_pontos + 1..b].trim().to_string();
        props.push((nome.to_string(), expr));
    }
    props
}

/// Tira os parênteses que envolvem a expressão inteira.
fn sem_parenteses_de_fora(e: &str) -> &str {
    let mut e = e.trim();
    while e.starts_with('(') && e.ends_with(')') {
        let mut nivel = 0;
        let mut fecha_no_fim = true;
        for (k, c) in e.char_indices() {
            match c {
                '(' => nivel += 1,
                ')' => {
                    nivel -= 1;
                    if nivel == 0 && k != e.len() - 1 {
                        fecha_no_fim = false;
                        break;
                    }
                }
                _ => {}
            }
        }
        if !fecha_no_fim {
            break;
        }
        e = e[1..e.len() - 1].trim();
    }
    e
}

/// A expressão (mascarada) só pode dar texto fixo do programa: uma string,
/// `qsTr("…")`, uma soma só de textos fixos ou um ternário cujos ramos são
/// todos assim. Qualquer outra coisa (um campo, uma função, uma soma com um
/// campo) é dado.
fn so_texto_fixo(e: &str) -> bool {
    let e = sem_parenteses_de_fora(e);
    let literal = |s: &str| {
        s.len() >= 2
            && (s.starts_with('"') || s.starts_with('\''))
            && s.ends_with(&s[..1])
            && s[1..s.len() - 1].bytes().all(|c| c == b'S')
    };
    if literal(e) || (e.starts_with("qsTr(") && e.ends_with(')') && literal(e[5..e.len() - 1].trim())) {
        return true;
    }
    // Soma de textos fixos ("a" + "b": um texto comprido quebrado em linhas).
    let mut partes = Vec::new();
    let (mut nivel, mut inicio) = (0, 0);
    for (k, c) in e.char_indices() {
        match c {
            '(' | '[' | '{' => nivel += 1,
            ')' | ']' | '}' => nivel -= 1,
            '+' if nivel == 0 => {
                partes.push(&e[inicio..k]);
                inicio = k + 1;
            }
            _ => {}
        }
    }
    partes.push(&e[inicio..]);
    if partes.len() > 1 && partes.iter().all(|p| so_texto_fixo(p)) {
        return true;
    }
    // Ternário: a condição pode ser qualquer coisa; os ramos, só texto fixo.
    let mut nivel = 0;
    let mut pergunta = None;
    for (k, c) in e.char_indices() {
        match c {
            '(' | '[' | '{' => nivel += 1,
            ')' | ']' | '}' => nivel -= 1,
            '?' if nivel == 0 => {
                pergunta = Some(k);
                break;
            }
            _ => {}
        }
    }
    let Some(q) = pergunta else { return false };
    let resto = &e[q + 1..];
    let (mut nivel, mut aninhados) = (0, 0);
    for (k, c) in resto.char_indices() {
        match c {
            '(' | '[' | '{' => nivel += 1,
            ')' | ']' | '}' => nivel -= 1,
            '?' if nivel == 0 => aninhados += 1,
            ':' if nivel == 0 => {
                if aninhados == 0 {
                    return so_texto_fixo(&resto[..k]) && so_texto_fixo(&resto[k + 1..]);
                }
                aninhados -= 1;
            }
            _ => {}
        }
    }
    false
}

/// Os QML do programa (computador; as cópias do Android são iguais, `copias_android.rs`).
fn todos_os_qml() -> Vec<String> {
    let mut lista = Vec::new();
    for pasta in ["app/qml", "app/base", "app/qml_android"] {
        let mut nomes: Vec<String> = std::fs::read_dir(raiz().join(pasta))
            .unwrap_or_else(|e| panic!("{pasta}: {e}"))
            .filter_map(|x| x.ok()?.file_name().into_string().ok())
            .filter(|n| n.ends_with(".qml"))
            .collect();
        nomes.sort();
        lista.extend(nomes.into_iter().map(|n| format!("{pasta}/{n}")));
    }
    lista
}

/// Os componentes cuja raiz é um `Text` (`Rotulo`, `Titulo`, `TextoErro`,
/// `Nota`…), com o formato que a raiz declara.
fn componentes_de_texto() -> Vec<(String, Option<String>)> {
    let mut lista = Vec::new();
    for arquivo in todos_os_qml() {
        let limpo = sem_comentarios(&ler(&arquivo));
        if let Some(raiz) = objetos(&limpo).into_iter().next() {
            if raiz.tipo == "Text" || raiz.tipo == "Label" {
                let nome = arquivo.rsplit('/').next().unwrap().trim_end_matches(".qml").to_string();
                lista.push((nome, raiz.prop("textFormat").map(str::to_string)));
            }
        }
    }
    lista
}

/// H10 (extensão): os componentes de texto do programa desenham o texto puro.
/// Uma mudança só em cada um cobre todos os usos (inclusive os do `Main.qml`,
/// que não muda sem pedido próprio: o título e o detalhe da espera).
#[test]
fn h10_componentes_de_texto_em_texto_puro() {
    let comps = componentes_de_texto();
    let nomes: Vec<&str> = comps.iter().map(|(n, _)| n.as_str()).collect();
    for esperado in ["Rotulo", "Titulo", "TextoErro", "Nota"] {
        assert!(nomes.contains(&esperado), "{esperado} deixou de ser um componente de texto? {nomes:?}");
    }
    let fora: Vec<String> = comps
        .iter()
        .filter(|(_, f)| f.as_deref() != Some("Text.PlainText"))
        .map(|(n, f)| format!("{n}.qml: textFormat {}", f.as_deref().unwrap_or("(não declarado: AutoText)")))
        .collect();
    assert!(fora.is_empty(), "componente de texto sem texto puro (H10, extensão): {fora:#?}");
}

/// Os textos formatados de propósito (`textFormat` diferente de texto puro):
/// arquivo e um pedaço da expressão do texto. Os dados deles passam por
/// `Tema.escapar()` (trava `h10_textos_formatados_escapam_os_dados`) ou não
/// têm dado (o link "Mostrar todos (n)" da seção de backup: texto fixo e uma
/// contagem). Um texto formatado só com texto fixo não precisa estar aqui.
const FORMATADOS_DE_PROPOSITO: &[(&str, &str)] = &[
    ("app/qml/Notificacao.qml", "raiz.linha()"),
    ("app/qml/TelaPeca.qml", "Tema.escapar(itemGrade.modelData.variacao"),
    ("app/qml/TelaConfiguracoes.qml", "Tema.escapar(linhaRef.modelData.oQue)"),
    ("app/qml/TelaConfiguracoes.qml", "Tema.escapar(linhaPend.modelData.textoMotivo)"),
    ("app/qml/TelaConfiguracoes.qml", "\"<b>\" + Tema.escapar(modelData.oQue)"),
    ("app/qml/TelaConfiguracoes.qml", "\"<b>\" + Tema.escapar(modelData.quando)"),
    ("app/qml/SecaoBackup.qml", "Mostrar todos (\" + lista.length"),
];

/// Exceções (lista fechada): arquivo, um pedaço da expressão e o porquê.
const EXCECOES: &[(&str, &str, &str)] = &[
    (
        "app/qml/Main.qml",
        "janela.erroGeral",
        "o Main.qml não muda sem pedido próprio (regra 8 do plano); é a mensagem de erro ao abrir o programa",
    ),
    (
        "app/qml/Campo.qml",
        "dica",
        "dica do botão Copiar/Colar do próprio Campo: só recebe texto fixo (\"Colar na busca\", \"Copiar a busca\")",
    ),
    ("app/qml/GraficoBarras.qml", "Tema.valor(col.modelData.valor)", "dica da barra: só número e unidade"),
];

/// Os controles do Qt que, sem `contentItem` próprio, desenham o texto com o
/// rótulo padrão do estilo (`IconLabel`/`CheckLabel`/`Text`, todos AutoText).
const CONTROLES_COM_ROTULO_PADRAO: &[&str] = &[
    "Button",
    "ItemDelegate",
    "CheckBox",
    "RadioButton",
    "Switch",
    "MenuItem",
    "MenuBarItem",
    "TabButton",
    "ToolButton",
    "RoundButton",
    "DelayButton",
    "CheckDelegate",
    "RadioDelegate",
    "SwitchDelegate",
    "SwipeDelegate",
    "ToolTip",
];

/// H10 (extensão): todo texto que mostra dado declara o formato. `Text` e
/// `Label` com dado têm `textFormat` (o texto puro, ou um dos formatados de
/// propósito); os componentes de texto já são puros; um controle do Qt com
/// dado tem o `contentItem` próprio (o rótulo padrão é AutoText); uma dica
/// (`ToolTip.text`) com dado só entre as exceções. Texto fixo do programa (uma
/// string, ou um ternário de strings) não precisa: não tem marcação.
#[test]
fn h10_textos_de_dado_declaram_o_formato() {
    let puros: Vec<String> = componentes_de_texto()
        .into_iter()
        .filter(|(_, f)| f.as_deref() == Some("Text.PlainText"))
        .map(|(n, _)| n)
        .collect();
    let todos_comps: Vec<String> = componentes_de_texto().into_iter().map(|(n, _)| n).collect();
    let mut fora = Vec::new();
    let mut vistos = 0;
    let mut excecoes_usadas = vec![false; EXCECOES.len()];
    let mut formatados_usados = vec![false; FORMATADOS_DE_PROPOSITO.len()];
    for arquivo in todos_os_qml() {
        let limpo = sem_comentarios(&ler(&arquivo));
        for obj in objetos(&limpo) {
            let texto_ou_comp = obj.tipo == "Text" || obj.tipo == "Label" || todos_comps.contains(&obj.tipo);
            let controle = CONTROLES_COM_ROTULO_PADRAO.contains(&obj.tipo.as_str());
            // A dica (ToolTip.text) em qualquer objeto.
            if let Some(dica) = obj.prop("ToolTip.text") {
                if !so_texto_fixo(&String::from_utf8(mascarado(dica)).unwrap()) {
                    vistos += 1;
                    match EXCECOES.iter().position(|(a, p, _)| *a == arquivo && dica.contains(p)) {
                        Some(i) => excecoes_usadas[i] = true,
                        None => {
                            fora.push(format!("{arquivo}:{}: {} › ToolTip.text com dado: {dica}", obj.linha, obj.tipo))
                        }
                    }
                }
            }
            if !texto_ou_comp && !controle {
                continue;
            }
            // O texto: o `text:` ou, sem ele, o atribuído por código (`id.text = …`).
            let expr = match obj.prop("text") {
                Some(e) => e.to_string(),
                None => match obj.prop("id") {
                    Some(id) if limpo.contains(&format!("{id}.text = ")) => format!("{id}.text = …"),
                    _ => continue,
                },
            };
            if so_texto_fixo(&String::from_utf8(mascarado(&expr)).unwrap()) {
                continue;
            }
            vistos += 1;
            let resumo: String = expr.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(90).collect();
            if let Some(i) = EXCECOES.iter().position(|(a, p, _)| *a == arquivo && expr.contains(p)) {
                excecoes_usadas[i] = true;
                continue;
            }
            if controle {
                if obj.prop("contentItem").is_none() {
                    fora.push(format!(
                        "{arquivo}:{}: {} com o rótulo padrão do Qt (AutoText; sem contentItem próprio): {resumo}",
                        obj.linha, obj.tipo
                    ));
                }
                continue;
            }
            match obj.prop("textFormat") {
                Some("Text.PlainText") => {}
                None if puros.contains(&obj.tipo) => {}
                None => fora.push(format!("{arquivo}:{}: {} sem textFormat: {resumo}", obj.linha, obj.tipo)),
                Some(f) => match FORMATADOS_DE_PROPOSITO.iter().position(|(a, p)| *a == arquivo && expr.contains(p)) {
                    Some(i) => formatados_usados[i] = true,
                    None => fora.push(format!(
                        "{arquivo}:{}: {} formatado ({f}) fora da lista dos formatados de propósito: {resumo}",
                        obj.linha, obj.tipo
                    )),
                },
            }
        }
    }
    assert!(vistos >= 300, "a leitura achou poucos textos com dado ({vistos}): a leitura dos QML quebrou?");
    let sobrando: Vec<_> = EXCECOES.iter().zip(&excecoes_usadas).filter(|(_, u)| !**u).map(|(e, _)| e).collect();
    assert!(sobrando.is_empty(), "exceção que não existe mais (tire da lista): {sobrando:#?}");
    let sobrando: Vec<_> =
        FORMATADOS_DE_PROPOSITO.iter().zip(&formatados_usados).filter(|(_, u)| !**u).map(|(e, _)| e).collect();
    assert!(sobrando.is_empty(), "formatado de propósito que não existe mais (tire da lista): {sobrando:#?}");
    assert!(fora.is_empty(), "{} texto(s) com dado no formato automático do Qt (H10, extensão): {fora:#?}", fora.len());
}

/// A leitura dos QML desta trava, nos casos que importam (para a trava não
/// passar por engano).
#[test]
fn h10_leitura_dos_qml_da_trava() {
    let qml = "Item {\n    Text {\n        text: a.b\n              ? \"x\"\n              : c\n        color: \"red\" // { não conta\n    }\n    Text { text: \"fixo\"; font.bold: true }\n    Rotulo {\n        text: cond ? \"A\" : (outra ? \"B\" : \"C\")\n    }\n    Text {\n        id: msg\n    }\n    function f() { msg.text = x }\n    ItemDelegate { text: modelData }\n}\n";
    let objs = objetos(&sem_comentarios(qml));
    let tipos: Vec<&str> = objs.iter().map(|o| o.tipo.as_str()).collect();
    assert_eq!(tipos, ["Item", "Text", "Text", "Rotulo", "Text", "ItemDelegate"]);
    assert_eq!(objs.iter().map(|o| o.linha).collect::<Vec<_>>(), [1, 2, 8, 9, 12, 16]);
    let fixo = |e: &str| so_texto_fixo(&String::from_utf8(mascarado(e)).unwrap());
    assert_eq!(objs[1].prop("text"), Some("a.b\n              ? \"x\"\n              : c"));
    assert!(!fixo(objs[1].prop("text").unwrap()), "um ramo com campo é dado");
    assert_eq!(objs[1].prop("color"), Some("\"red\""));
    assert!(fixo(objs[2].prop("text").unwrap()));
    assert!(fixo(objs[3].prop("text").unwrap()), "ternário de strings é texto fixo");
    assert_eq!(objs[4].prop("id"), Some("msg"));
    assert!(!fixo("\"R$ \" + x") && !fixo("x || \"—\"") && fixo("qsTr(\"Ok\")") && fixo("''"));
    assert!(fixo("\"Um texto \" + \"comprido\"") && fixo("c ? \"a\" + \"b\" : \"d\"") && !fixo("\"a\" + n + \"b\""));
}
