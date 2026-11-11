//! Rustinho: abre a janela principal.
// No Windows, sem a janela preta do console atrás.
#![cfg_attr(windows, windows_subsystem = "windows")]
//!
//! As regras e o banco de dados ficam no núcleo (pasta `nucleo`). Aqui só
//! se cria a aplicação Qt e se carrega a tela principal (qml/Main.qml).

mod ponte;

use std::sync::atomic::{AtomicBool, Ordering};

use cxx_qt_lib::{QByteArray, QGuiApplication, QQmlApplicationEngine, QString, QUrl};

/// J04 (dev.3.3): se a tela principal não carrega, o Rustinho ficava aberto e
/// invisível (nada na tela, o processo rodando). Agora mostra este aviso e
/// sai com erro quando ele é fechado (como o Android, que avisa e sai). Só
/// QtQuick: se o que falhou foi outra parte das telas, o aviso ainda abre.
const AVISO_SEM_TELA: &str = r##"
import QtQuick
import QtQuick.Window
Window {
    width: 560
    height: 220
    visible: true
    title: "Rustinho"
    color: "#f4eff7"
    onClosing: Qt.exit(1)
    Item {
        anchors.fill: parent
        focus: true
        Keys.onEscapePressed: Qt.exit(1)
        Keys.onReturnPressed: Qt.exit(1)
    }
    Column {
        anchors.centerIn: parent
        width: parent.width - 48
        spacing: 18
        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            font.pixelSize: 17
            color: "#2a1f33"
            text: "A tela principal do Rustinho não abriu. Feche e abra de novo. Se continuar, instale o Rustinho de novo ou chame o suporte."
        }
        Text {
            text: "Fechar"
            font.pixelSize: 17
            font.bold: true
            color: "#8a1a8f"
            MouseArea { anchors.fill: parent; onClicked: Qt.exit(1) }
        }
    }
}
"##;

/// A tela principal já falhou uma vez (então o que falhou agora foi o aviso).
static SEM_TELA: AtomicBool = AtomicBool::new(false);

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
        // J04: a tela principal não carregou → o aviso; nem o aviso → sai.
        engine
            .as_mut()
            .on_object_creation_failed(|engine, url| {
                eprintln!("Rustinho: a tela {url} não carregou (o motivo está nas mensagens acima).");
                if SEM_TELA.swap(true, Ordering::SeqCst) {
                    std::process::exit(1);
                }
                engine.load_data(&QByteArray::from(AVISO_SEM_TELA), &QUrl::from("rustinho:aviso-sem-tela.qml"));
            })
            .release();
        engine.load(&QUrl::from("qrc:/qt/qml/rustinho/qml/Main.qml"));
    }

    if let Some(app) = app.as_mut() {
        let codigo = app.exec();
        if codigo != 0 {
            std::process::exit(codigo);
        }
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
