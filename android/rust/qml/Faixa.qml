import QtQuick
import QtQuick.Layouts
import rustinho

// Faixa de cor com cantos arredondados, como as combinações do livro.
// O que for colocado dentro dela fica empilhado (ColumnLayout).
Rectangle {
    id: faixa
    property string titulo: ""
    property string subtitulo: ""
    property color corTitulo: Tema.apagado
    property int preenchimento: Tema.estreito ? 16 : 22
    property alias espacamento: coluna.spacing
    default property alias conteudo: coluna.data

    color: Tema.pergaminho
    radius: Tema.raio
    implicitHeight: coluna.implicitHeight + preenchimento * 2
    implicitWidth: coluna.implicitWidth + preenchimento * 2

    ColumnLayout {
        id: coluna
        x: faixa.preenchimento
        y: faixa.preenchimento
        width: faixa.width - faixa.preenchimento * 2
        spacing: 14

        RowLayout {
            visible: faixa.titulo !== ""
            spacing: 12
            Layout.fillWidth: true
            Rotulo {
                text: faixa.titulo
                color: faixa.corTitulo
            }
            Nota {
                visible: faixa.subtitulo !== ""
                text: faixa.subtitulo
                font.pixelSize: 14
                wrapMode: Tema.estreito ? Text.WordWrap : Text.NoWrap
                elide: Tema.estreito ? Text.ElideNone : Text.ElideRight
                Layout.fillWidth: true
            }
        }
    }
}
