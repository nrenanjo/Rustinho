import QtQuick
import rustinho

// Texto de apoio em itálico com serifa, como as frases-chave do livro.
Text {
    color: Tema.tintaSuave
    font.family: Tema.serifa
    font.italic: true
    font.pixelSize: 15
    wrapMode: Text.WordWrap
    lineHeight: 1.12
}
