//! R13 (DP-42), revisão R8-N5: travas que leem o texto dos QML da peça juntada, para o que o roteiro de tela não
//! alcança. Na ficha de uma peça juntada em outra o estoque é 0 e a contagem vale para a peça que ficou: o botão
//! "Ajustar" não aparece ali (antes: o diálogo dizia "o sistema mostra 0" e a ficha continuava 0 depois do ajuste),
//! e a nota da junção diz onde contar. A igualdade das duas cópias (computador e Android) é do `copias_android.rs`.

use std::path::{Path, PathBuf};

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn ler(caminho: &str) -> String {
    std::fs::read_to_string(raiz().join(caminho)).unwrap_or_else(|e| panic!("{caminho}: {e}"))
}

/// O bloco `Botao { … }` que abre o ajuste de estoque.
fn bloco_do_ajustar(qml: &str) -> &str {
    let chamada = qml.find("ajusteEstoque.abrirCom(").expect("a ficha abre o ajuste de estoque");
    let inicio = qml[..chamada].rfind("Botao {").expect("o ajuste é aberto por um Botao");
    &qml[inicio..chamada]
}

#[test]
fn r8_n5_ajustar_nao_aparece_na_ficha_da_peca_juntada() {
    let qml = ler("app/qml/TelaPeca.qml");
    let bloco = bloco_do_ajustar(&qml);
    let visivel = bloco
        .lines()
        .find(|l| l.trim_start().starts_with("visible:"))
        .unwrap_or_else(|| panic!("o Ajustar não tem visible:\n{bloco}"));
    // R9-B3: a linha inteira (a inversão `!!tela.ficha.juntadaCom` contém o mesmo trecho e sumiria com o Ajustar das
    // peças comuns).
    assert_eq!(visivel.trim(), "visible: !!tela.ficha && !tela.ficha.juntadaCom", "o Ajustar some só na peça juntada");
}

#[test]
fn r8_n5_nota_da_juntada_diz_onde_contar() {
    let qml = ler("app/qml/TelaPeca.qml");
    let inicio = qml.find("objectName: \"notaJuntada\"").expect("a nota da junção");
    let nota = &qml[inicio..inicio + qml[inicio..].find("font.pixelSize").unwrap()];
    assert!(nota.contains("conte na peça que ficou"), "{nota}");
}
