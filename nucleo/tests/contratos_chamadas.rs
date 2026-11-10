//! Dev.3.3, passo 1.3 — contrato das chamadas das telas (C02, K04).
//!
//! - C02: o contrato antigo (`api.rs`) só conferia `Api.chamar("…")`. Aqui
//!   entram TODAS as formas com que as telas chamam o núcleo: `Api.chamar`,
//!   `Api.chamarDemorado`, `chamarGerente` e `chamarGerenteDemorado` (com
//!   qualquer prefixo: `janela.`, `tela.janela.`…). Cada nome tem de ser um
//!   comando que existe (`api::comandos()`). Trava: passa hoje.
//! - K04: as formas ERRADAS de chamar com a senha do gerente, que a tela
//!   publicada mostrou (H11, H12). Caracterização: falha em `4b253c6`
//!   apontando `SecaoBackup.qml:69`, `:81` e `TelaReceber.qml:865` (e as
//!   cópias do Android), até o passo 2.1.

use std::path::{Path, PathBuf};

const FORMAS: &[&str] = &["Api.chamar(\"", "Api.chamarDemorado(\"", "chamarGerente(\"", "chamarGerenteDemorado(\""];

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// Os QML das telas (computador e Android), com o caminho relativo à raiz.
fn arquivos_qml() -> Vec<(String, String)> {
    let r = raiz();
    let mut lista = Vec::new();
    for pasta in
        ["app/qml", "app/qml_android", "app/base", "android/rust/qml", "android/rust/qml_android", "android/rust/base"]
    {
        let Ok(itens) = std::fs::read_dir(r.join(pasta)) else { continue };
        let mut nomes: Vec<_> =
            itens.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "qml")).collect();
        nomes.sort();
        for p in nomes {
            let texto = std::fs::read_to_string(&p).unwrap_or_default();
            lista.push((format!("{pasta}/{}", p.file_name().unwrap().to_string_lossy()), texto));
        }
    }
    lista
}

fn linha_de(texto: &str, pos: usize) -> usize {
    texto[..pos].matches('\n').count() + 1
}

/// (forma, comando, linha) de cada chamada ao núcleo no texto de uma tela.
/// `chamarGerente("` não é contado dentro de `chamarGerenteDemorado("` nem em
/// `Api.chamar("` dentro de `Api.chamarDemorado("` (as formas são exatas).
fn chamadas(texto: &str) -> Vec<(&'static str, String, usize)> {
    let mut v = Vec::new();
    for forma in FORMAS {
        let mut inicio = 0;
        while let Some(i) = texto[inicio..].find(forma) {
            let pos = inicio + i;
            let antes = texto[..pos].chars().last();
            // `chamarGerente(` precisa começar a palavra (não `xchamarGerente(`).
            let comeca = forma.starts_with("Api.") || !antes.is_some_and(|c| c.is_alphanumeric() || c == '_');
            let resto = &texto[pos + forma.len()..];
            if comeca {
                let nome: String = resto.chars().take_while(|c| *c != '"').collect();
                v.push((*forma, nome, linha_de(texto, pos)));
            }
            inicio = pos + forma.len();
        }
    }
    v
}

/// O segundo argumento de uma chamada (o texto até a vírgula seguinte no
/// nível de parênteses e chaves da chamada).
fn segundo_argumento(resto: &str) -> String {
    // `resto` começa logo depois do nome do comando e da aspa de fechamento.
    let mut nivel = 0i32;
    let mut arg = String::new();
    let mut comecou = false;
    for c in resto.chars() {
        if !comecou {
            if c == ',' {
                comecou = true;
            } else if c == ')' {
                return String::new();
            }
            continue;
        }
        match c {
            '(' | '{' | '[' => nivel += 1,
            ')' | '}' | ']' if nivel == 0 => break,
            ')' | '}' | ']' => nivel -= 1,
            ',' if nivel == 0 => break,
            _ => {}
        }
        arg.push(c);
    }
    arg.trim().to_string()
}

#[test]
fn chamadas_extraidas_de_todas_as_formas() {
    let t = "Api.chamar(\"a_x\", {})\nAPI\njanela.chamarGerente(\"b_y\", d, f)\n\
             Api.chamarDemorado(\"c_z\", {}, \"t\", f)\n tela.janela.chamarGerenteDemorado(\"d_w\", {}, \"t\", f)";
    let nomes: Vec<String> = chamadas(t).into_iter().map(|(_, n, _)| n).collect();
    for n in ["a_x", "b_y", "c_z", "d_w"] {
        assert!(nomes.contains(&n.to_string()), "{n} não foi extraído: {nomes:?}");
    }
    assert_eq!(nomes.len(), 4, "nenhuma chamada contada duas vezes: {nomes:?}");
}

/// C02 — toda chamada das telas, em qualquer forma, usa um comando que existe.
#[test]
fn toda_chamada_das_telas_usa_comando_que_existe() {
    let comandos = rustinho_nucleo::api::comandos();
    let mut faltam = Vec::new();
    let mut total = 0;
    for (arq, texto) in arquivos_qml() {
        for (forma, nome, linha) in chamadas(&texto) {
            total += 1;
            if !comandos.contains(&nome.as_str()) {
                faltam.push(format!("{arq}:{linha} {forma}…{nome}"));
            }
        }
    }
    assert!(total > 200, "poucas chamadas achadas ({total}): o leitor dos QML quebrou?");
    assert!(faltam.is_empty(), "as telas chamam comandos que não existem: {faltam:#?}");
}

/// K04 — formas erradas de chamar com a senha do gerente:
/// 1. `chamarGerente`/`chamarGerenteDemorado` com uma FUNÇÃO no lugar dos
///    dados (o segundo argumento) — a resposta é descartada (H12);
/// 2. um diálogo de motivo que chama `acao(motivo)` com um argumento só e usa
///    o retorno na hora, numa tela em que as ações são `(m, pronto) => …`
///    (assíncronas, respondem pelo `pronto`) — o retorno é `undefined` (H11).
#[test]
#[ignore = "caracterização K04 (H11, H12): formas erradas em 4b253c6; passa no passo 2.1"]
fn chamadas_com_senha_do_gerente_na_forma_certa() {
    let mut erradas = Vec::new();
    for (arq, texto) in arquivos_qml() {
        for forma in ["chamarGerente(\"", "chamarGerenteDemorado(\""] {
            let mut inicio = 0;
            while let Some(i) = texto[inicio..].find(forma) {
                let pos = inicio + i;
                let depois_do_nome = &texto[pos + forma.len()..];
                let resto = &depois_do_nome[depois_do_nome.find('"').map(|x| x + 1).unwrap_or(0)..];
                let arg = segundo_argumento(resto);
                let e_funcao = arg.starts_with("function") || arg.contains("=>");
                if e_funcao {
                    erradas.push(format!("{arq}:{} função no lugar dos dados: {forma}…", linha_de(&texto, pos)));
                }
                inicio = pos + forma.len();
            }
        }
        if texto.contains("(m, pronto) =>") {
            let mut inicio = 0;
            while let Some(i) = texto[inicio..].find("= acao(") {
                let pos = inicio + i;
                let args = &texto[pos + "= acao(".len()..];
                let fim = args.find(')').unwrap_or(0);
                if !args[..fim].contains(',') {
                    erradas.push(format!(
                        "{arq}:{} diálogo de motivo usa o retorno de acao(…) sem o `pronto`",
                        linha_de(&texto, pos)
                    ));
                }
                inicio = pos + 1;
            }
        }
    }
    assert!(erradas.is_empty(), "chamadas com a senha do gerente na forma errada: {erradas:#?}");
}
