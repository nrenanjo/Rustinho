import QtQuick
import QtQuick.Layouts
import rustinho

// Mensagem de erro (some quando vazia).
Text {
    visible: text !== ""
    color: Tema.roxo ? Tema.vermelhoNo(palette.window) : Tema.vermelho
    // H10 (extensão): texto puro. Um dado com "<b>", "<img …>" ou "&amp;"
    // aparece como foi escrito, e não como formatação (o formato automático do
    // Qt lia como HTML). Quem precisa de formatação declara o seu e escapa os
    // dados (Tema.escapar).
    textFormat: Text.PlainText
    font.family: Tema.texto
    font.weight: Font.DemiBold
    font.pixelSize: 15
    wrapMode: Text.WordWrap
    Layout.fillWidth: true
}
