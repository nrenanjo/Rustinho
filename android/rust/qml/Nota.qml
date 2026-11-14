import QtQuick
import rustinho

// Texto de apoio em itálico com serifa, como as frases-chave do livro.
Text {
    color: Tema.roxo ? Tema.tintaSuaveNo(palette.window) : Tema.tintaSuave
    // H10 (extensão): texto puro. Um dado com "<b>", "<img …>" ou "&amp;"
    // aparece como foi escrito, e não como formatação (o formato automático do
    // Qt lia como HTML). Quem precisa de formatação declara o seu e escapa os
    // dados (Tema.escapar).
    textFormat: Text.PlainText
    font.family: Tema.serifa
    font.italic: true
    font.pixelSize: 15
    wrapMode: Text.WordWrap
    lineHeight: 1.12
}
