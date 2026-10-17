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
    width: Math.min(560, Tema.largura - 24)
    padding: Tema.estreito ? 18 : 30
    closePolicy: Popup.CloseOnEscape
    T.Overlay.modal: Rectangle { color: Tema.veu }
    background: Rectangle { color: painel.cor; radius: Tema.raio + 3 }
}
