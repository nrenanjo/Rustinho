//! Compila a parte em C++/Qt e junta as telas (QML) e as fontes no programa.

use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

/// Lista os arquivos de uma pasta com a extensão pedida, em ordem.
fn arquivos(pasta: &str, extensoes: &[&str]) -> Vec<String> {
    println!("cargo::rerun-if-changed={pasta}");
    let mut lista: Vec<String> = std::fs::read_dir(pasta)
        .unwrap_or_else(|e| panic!("não achei a pasta {pasta}: {e}"))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|x| x.to_str())
                .map(|x| extensoes.contains(&x))
                .unwrap_or(false)
        })
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    lista.sort();
    lista
}

fn main() {
    // Telas e componentes (pasta qml) + Tema e Api, que são únicos (pasta base).
    let mut qml: Vec<QmlFile> = arquivos("qml", &["qml"]).into_iter().map(QmlFile::from).collect();
    qml.push(QmlFile::from("base/Tema.qml").singleton(true));
    qml.push(QmlFile::from("base/Api.qml").singleton(true));

    let mut construtor = CxxQtBuilder::new_qml_module(QmlModule::new("rustinho").qml_files(qml))
        .qrc_resources(arquivos("fontes", &["ttf", "woff2"]))
        .qrc_resources(arquivos("icone", &["png"]))
        .files(["src/ponte.rs"])
        // Impressão em PDF (QTextDocument + QPdfWriter, do QtGui).
        .cpp_file("cpp/pdf.cpp")
        .cpp_file("cpp/janela.cpp");
    // Montagem do Android (android/rust): os cabeçalhos "rustinho/cpp/..."
    // ficam numa pasta à parte, porque lá o pacote tem outro nome.
    if std::path::Path::new("include").is_dir() {
        construtor = construtor.include_dir("include");
    }
    // No Windows, imprime direto na impressora padrão (QPrinter).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        construtor = construtor.qt_module("PrintSupport");
    }
    construtor.build();

    // Ícone do programa no Windows (o .exe e a barra de tarefas).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut r = winresource::WindowsResource::new();
        r.set_icon("../instalador/rustinho.ico");
        r.set("FileDescription", "Rustinho");
        r.set("ProductName", "Rustinho");
        if let Err(e) = r.compile() {
            println!("cargo::warning=sem ícone no .exe: {e}");
        }
    }
}
