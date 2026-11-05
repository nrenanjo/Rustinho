import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Topo das telas: a seta de retorno ("← Início"), título grande e, à
// direita, os botões.
ColumnLayout {
    id: cabecalho
    property string titulo: ""
    property string subtitulo: ""
    // Para onde a seta volta (o texto ao lado dela, D-V1).
    property string textoVoltar: "Início"
    property bool mostrarVoltar: true
    // Tela com CPF/CNPJ, custo, lucro ou saldos: mostra o olho no canto.
    property bool sensivel: false
    default property alias acoes: linhaAcoes.data

    spacing: 2

    RowLayout {
        spacing: 10
        Layout.fillWidth: true
        visible: cabecalho.mostrarVoltar || cabecalho.sensivel
        // V-2/V-3 (0.9.18): a seta faz o mesmo que o Esc, pelo mesmo caminho
        // (janela.retornar): a tela decide com aoEsc, se tiver; senão, volta.
        // Nenhuma tela trata a seta à parte.
        Seta {
            id: seta
            visible: cabecalho.mostrarVoltar
            destino: cabecalho.textoVoltar
            Layout.leftMargin: -seta.folga
            onClicked: ApplicationWindow.window.retornar(true)
        }
        Item { Layout.fillWidth: true; implicitHeight: Tema.toqueSeta }
        Olho { visible: cabecalho.sensivel }
    }

    // Título e botões lado a lado; na tela estreita, os botões vão para
    // baixo do título e quebram linha.
    GridLayout {
        columns: Tema.fino ? 1 : 2
        columnSpacing: 16
        rowSpacing: 10
        Layout.fillWidth: true
        ColumnLayout {
            spacing: 0
            Layout.fillWidth: true
            Titulo {
                text: cabecalho.titulo
                font.pixelSize: Tema.baixo ? 30 : Tema.estreito ? 34 : 44
                Layout.fillWidth: true
            }
            Nota {
                text: cabecalho.subtitulo
                visible: text !== ""
                font.pixelSize: Tema.estreito ? 15 : 16
                Layout.fillWidth: true
            }
        }
        Flow {
            id: linhaAcoes
            spacing: 10
            visible: children.length > 0
            Layout.alignment: Tema.fino ? Qt.AlignLeft : Qt.AlignBottom | Qt.AlignRight
            Layout.fillWidth: Tema.fino
            Layout.preferredWidth: Tema.fino ? -1 : implicitLargura
            // Flow não calcula a própria largura: soma a dos botões.
            readonly property real implicitLargura: {
                let t = 0
                let n = 0
                for (let i = 0; i < children.length; i++)
                    if (children[i].visible) {
                        t += children[i].width
                        n++
                    }
                return t + Math.max(0, n - 1) * spacing
            }
        }
    }
}
