import QtQuick
import rustinho

// Rótulo pequeno em maiúsculas, como "POSITIVAS" no livro.
Text {
    color: Tema.roxo ? Tema.apagadoNo(palette.window) : Tema.apagado
    // H10 (extensão): texto puro. Um dado com "<b>", "<img …>" ou "&amp;"
    // aparece como foi escrito, e não como formatação (o formato automático do
    // Qt lia como HTML). Quem precisa de formatação declara o seu e escapa os
    // dados (Tema.escapar).
    textFormat: Text.PlainText
    font.family: Tema.texto
    font.pixelSize: 12
    font.weight: Font.Bold
    font.letterSpacing: 1.8
    font.capitalization: Font.AllUppercase
    elide: Text.ElideRight
}
