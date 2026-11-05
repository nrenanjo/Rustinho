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
    // Linux (AppImage): janelas de arquivo e de pasta do sistema (T-16).
    if let Some(tema) =
        tema_do_portal(std::env::var_os("APPIMAGE").is_some(), std::env::var_os("QT_QPA_PLATFORMTHEME").as_deref())
    {
        std::env::set_var("QT_QPA_PLATFORMTHEME", tema);
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

/// T-16: no Linux, as janelas de arquivo e de pasta do sistema (KDE, GNOME,
/// XFCE...) vêm pelo portal (xdg-desktop-portal). O Rustinho é um
/// QGuiApplication: o tema do Plasma só dá a janela do KDE a programas
/// QApplication, e sem o tema do portal o Qt mostra a janela de arquivos dele.
///
/// Só no AppImage (`APPIMAGE` definido): ele traz o tema do portal e nenhum
/// tema de desktop, e o Qt volta sozinho à janela dele quando a sessão não
/// tem portal. Fora do AppImage (Qt do sistema, testes) nada muda: com o tema
/// do Plasma instalado e o portal fora do ar, o Qt pediria a janela de
/// widgets do Plasma, que não existe num QGuiApplication.
///
/// Quem já escolheu um tema em `QT_QPA_PLATFORMTHEME` (mesmo vazio, para
/// ficar com a janela do Qt) fica com a escolha. A exceção é "gtk2", que o
/// gancho do linuxdeploy põe no GNOME e no XFCE e que não existe no Qt 6.
fn tema_do_portal(appimage: bool, atual: Option<&std::ffi::OsStr>) -> Option<&'static str> {
    if !cfg!(target_os = "linux") || !appimage {
        return None;
    }
    match atual {
        None => Some("xdgdesktopportal"),
        Some(t) if t == "gtk2" => Some("xdgdesktopportal"),
        Some(_) => None,
    }
}

#[cfg(test)]
mod testes {
    use super::tema_do_portal;
    use std::ffi::OsStr;

    #[test]
    #[cfg(target_os = "linux")]
    fn t16_appimage_usa_o_portal_quando_ninguem_escolheu() {
        assert_eq!(tema_do_portal(true, None), Some("xdgdesktopportal"));
        // O linuxdeploy põe "gtk2" no GNOME e no XFCE; o Qt 6 não tem esse tema.
        assert_eq!(tema_do_portal(true, Some(OsStr::new("gtk2"))), Some("xdgdesktopportal"));
    }

    #[test]
    fn t16_escolha_da_pessoa_e_fora_do_appimage_nada_muda() {
        assert_eq!(tema_do_portal(false, None), None, "Qt do sistema e testes");
        assert_eq!(tema_do_portal(false, Some(OsStr::new("gtk2"))), None);
        assert_eq!(tema_do_portal(true, Some(OsStr::new(""))), None, "vazio = janela do Qt, de propósito");
        assert_eq!(tema_do_portal(true, Some(OsStr::new("kde"))), None);
        assert_eq!(tema_do_portal(true, Some(OsStr::new("xdgdesktopportal"))), None, "já está");
    }
}
