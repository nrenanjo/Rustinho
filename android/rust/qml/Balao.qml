import QtQuick
import rustinho

// Balão pequeno e arredondado ao lado do nome: diz por qual código a peça
// foi achada ("Cód. barras", "Similar · SKF"). Some quando não tem texto.
Rectangle {
    id: balao
    property string texto: ""
    property bool escuro: false   // a linha está selecionada (fundo escuro)
    property int larguraMaxima: 220

    visible: texto !== ""
    radius: height / 2
    // Na linha escolhida (magenta nos três temas): um véu escuro deixa o
    // texto branco legível.
    color: escuro ? Tema.chipAtual : Tema.papel
    implicitWidth: Math.min(rotulo.implicitWidth + 18, larguraMaxima)
    implicitHeight: rotulo.implicitHeight + 6

    Text {
        id: rotulo
        anchors.centerIn: parent
        width: Math.min(implicitWidth, balao.width - 18)
        text: balao.texto
        textFormat: Text.PlainText
        color: balao.escuro ? Tema.tintaAtual : Tema.azulEscuro
        font.family: Tema.texto
        font.weight: Font.DemiBold
        font.pixelSize: 12
        elide: Text.ElideRight
    }
}
