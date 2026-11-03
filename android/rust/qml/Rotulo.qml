import QtQuick
import rustinho

// Rótulo pequeno em maiúsculas, como "POSITIVAS" no livro.
Text {
    color: Tema.apagado
    font.family: Tema.texto
    font.pixelSize: 12
    font.weight: Font.Bold
    font.letterSpacing: 1.8
    font.capitalization: Font.AllUppercase
    elide: Text.ElideRight
}
