import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Templates as T
import rustinho

// Janelinha por cima da tela (pop-up), com o fundo do Rustinho.
Popup {
    id: painel
    property color cor: Tema.pergaminho
    modal: true
    focus: true
    anchors.centerIn: T.Overlay.overlay
    width: 560
    padding: 30
    closePolicy: Popup.CloseOnEscape
    T.Overlay.modal: Rectangle { color: Qt.rgba(0.16, 0.15, 0.13, 0.35) }
    background: Rectangle { color: painel.cor; radius: Tema.raio + 3 }
}
