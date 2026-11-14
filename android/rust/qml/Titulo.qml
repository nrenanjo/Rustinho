import QtQuick
import rustinho

// Título com a fonte do livro (Cormorant Garamond).
Text {
    // 0.9.18: no roxo, claro sobre o fundo e escuro nos painéis (a superfície
    // vem de `palette.window`; veja Tema.superficie).
    color: Tema.roxo ? Tema.tintaNo(palette.window) : Tema.tinta
    // H10 (extensão): texto puro. Um dado com "<b>", "<img …>" ou "&amp;"
    // aparece como foi escrito, e não como formatação (o formato automático do
    // Qt lia como HTML). Quem precisa de formatação declara o seu e escapa os
    // dados (Tema.escapar).
    textFormat: Text.PlainText
    font.family: Tema.titulo
    font.weight: Font.DemiBold
    font.pixelSize: 36
    elide: Text.ElideRight
    // No celular, título comprido quebra a linha em vez de cortar.
    wrapMode: Tema.estreito ? Text.WordWrap : Text.NoWrap
}
