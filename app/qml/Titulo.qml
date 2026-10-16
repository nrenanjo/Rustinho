import QtQuick
import rustinho

// Título com a fonte do livro (Cormorant Garamond).
Text {
    color: Tema.tinta
    font.family: Tema.titulo
    font.weight: Font.DemiBold
    font.pixelSize: 36
    elide: Text.ElideRight
}
