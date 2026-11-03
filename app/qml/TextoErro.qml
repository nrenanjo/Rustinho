import QtQuick
import QtQuick.Layouts
import rustinho

// Mensagem de erro (some quando vazia).
Text {
    visible: text !== ""
    color: Tema.vermelho
    font.family: Tema.texto
    font.weight: Font.DemiBold
    font.pixelSize: 15
    wrapMode: Text.WordWrap
    Layout.fillWidth: true
}
