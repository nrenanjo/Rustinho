import QtQuick
import rustinho

// Título com a fonte do livro (Cormorant Garamond).
Text {
    color: Tema.tinta
    font.family: Tema.titulo
    font.weight: Font.DemiBold
    font.pixelSize: 36
    elide: Text.ElideRight
    // No celular, título comprido quebra a linha em vez de cortar.
    wrapMode: Tema.estreito ? Text.WordWrap : Text.NoWrap
}
