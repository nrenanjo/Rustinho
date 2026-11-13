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
