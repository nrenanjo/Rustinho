import QtQuick
import rustinho

// Balão pequeno e arredondado ao lado do nome: diz por qual código a peça
// foi achada ("Cód. barras", "Similar · SKF"). Some quando não tem texto.
Rectangle {
    id: balao
    property string texto: ""
    property bool escuro: false   // a linha está selecionada (fundo escuro)

    visible: texto !== ""
    radius: height / 2
    // Na linha escolhida: no roxo (magenta), um véu escuro deixa o texto
    // branco legível; nos outros, um véu claro.
    color: escuro ? (Tema.roxo ? Qt.rgba(0.2, 0, 0.2, 0.35) : Tema.claroSuave(0.16)) : Tema.papel
    implicitWidth: Math.min(rotulo.implicitWidth + 18, 220)
    implicitHeight: rotulo.implicitHeight + 6

    Text {
        id: rotulo
        anchors.centerIn: parent
        width: Math.min(implicitWidth, balao.width - 18)
        text: balao.texto
        color: balao.escuro ? Tema.claro : Tema.azulEscuro
        font.family: Tema.texto
        font.weight: Font.DemiBold
        font.pixelSize: 12
        elide: Text.ElideRight
    }
}
