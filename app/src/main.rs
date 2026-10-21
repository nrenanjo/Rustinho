//! Rustinho: abre a janela principal.
// No Windows, sem a janela preta do console atrás.
#![cfg_attr(windows, windows_subsystem = "windows")]
//!
//! As regras e o banco de dados ficam no núcleo (pasta `nucleo`). Aqui só
//! se cria a aplicação Qt e se carrega a tela principal (qml/Main.qml).

mod ponte;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

fn main() {
    // PEND-09: um programa montado com a raiz de TESTE aceitaria notas
    // assinadas por ela. Isso nunca pode chegar à loja.
    if rustinho_nucleo::nfe_assinatura::ANCORAS_DE_TESTE_COMPILADAS {
        eprintln!("Rustinho montado com as âncoras de TESTE da NF-e: este programa não pode ser usado.");
        std::process::exit(2);
    }
    // Rustinho portátil: nada fora da própria pasta. O Qt guardaria a cópia
    // compilada das telas na pasta de sempre do usuário (no Windows,
    // %LOCALAPPDATA%\rustinho, a mesma do Rustinho instalado).
    if rustinho_nucleo::portatil::ligado() {
        std::env::set_var("QML_DISABLE_DISK_CACHE", "1");
    }
    let mut app = QGuiApplication::new();
    if let Some(mut app) = app.as_mut() {
        app.as_mut().set_application_name(&QString::from("rustinho"));
        app.as_mut().set_application_display_name(&QString::from("Rustinho"));
        app.as_mut().set_organization_name(&QString::from("rustinho"));
    }
    ponte::qobject::rustinho_preparar_janela();

    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine) = engine.as_mut() {
        // Necessário no Qt 6.4 (nas versões novas já é o padrão).
        engine.as_mut().add_import_path(&QString::from("qrc:/qt/qml"));
        engine.load(&QUrl::from("qrc:/qt/qml/rustinho/qml/Main.qml"));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
