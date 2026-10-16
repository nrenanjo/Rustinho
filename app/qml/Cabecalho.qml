import QtQuick
import QtQuick.Layouts
import rustinho

// Topo das telas: "← Voltar", título grande e, à direita, os botões.
ColumnLayout {
    id: cabecalho
    property string titulo: ""
    property string subtitulo: ""
    property string textoVoltar: "Início"
    property bool mostrarVoltar: true
    // Tela com CPF/CNPJ, custo, lucro ou saldos: mostra o olho no canto.
    property bool sensivel: false
    default property alias acoes: linhaAcoes.data

    signal voltar()

    spacing: 2

    RowLayout {
        spacing: 10
        Layout.fillWidth: true
        visible: cabecalho.mostrarVoltar || cabecalho.sensivel
        Botao {
            visible: cabecalho.mostrarVoltar
            tipo: "discreto"
            icone: "voltar"
            text: cabecalho.textoVoltar
            implicitHeight: 34
            font.pixelSize: 14
            Layout.leftMargin: -10
            onClicked: cabecalho.voltar()
        }
        Item { Layout.fillWidth: true; implicitHeight: 34 }
        Olho { visible: cabecalho.sensivel }
    }

    RowLayout {
        spacing: 16
        Layout.fillWidth: true
        ColumnLayout {
            spacing: 0
            Layout.fillWidth: true
            Titulo {
                text: cabecalho.titulo
                font.pixelSize: 44
                Layout.fillWidth: true
            }
            Nota {
                text: cabecalho.subtitulo
                visible: text !== ""
                font.pixelSize: 16
                Layout.fillWidth: true
            }
        }
        RowLayout {
            id: linhaAcoes
            spacing: 10
            Layout.alignment: Qt.AlignBottom
        }
    }
}
