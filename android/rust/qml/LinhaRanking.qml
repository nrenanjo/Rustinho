import QtQuick
import QtQuick.Layouts
import rustinho

// Uma linha de ranking com barrinha: nome, detalhe, valor e percentual.
// item: { nome, detalhe, valor, parte (0 a 1), percentual }
// 0.9.18 (V-6): as linhas alternam (indice); no cinza e no AMOLED, um véu
// claro numa e nada na outra (ficam dentro de painéis); no roxo, as faixas.
Rectangle {
    id: linha
    property var item: ({})
    property bool dinheiro: true
    property color cor: Tema.azulEscuro
    property int posicao: 0
    property int indice: 0
    // Valor que não é dinheiro mas também some com o olho fechado (margem).
    property bool sensivel: false
    signal abrir()

    readonly property bool par: (indice + 1) % 2 === 0
    color: Tema.faixa(par, Tema.vela(0.4), "transparent")
    palette.window: Tema.superficie(color)
    radius: Tema.raioCampo
    implicitWidth: coluna.implicitWidth + 20
    implicitHeight: coluna.implicitHeight + 12
    Layout.fillWidth: true

    ColumnLayout {
        id: coluna
        x: 10
        y: 6
        width: linha.width - 20
        spacing: 4

        RowLayout {
            spacing: 10
            Layout.fillWidth: true
            Text {
                visible: linha.posicao > 0
                text: linha.posicao
                textFormat: Text.PlainText
                color: Tema.douradoDaFaixa(linha.par)
                font.family: Tema.titulo
                font.weight: Font.Bold
                font.pixelSize: 18
                Layout.preferredWidth: 18
            }
            Text {
                text: linha.item.nome || ""
                textFormat: Text.PlainText
                color: Tema.tintaDaFaixa(linha.par)
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 15
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            Text {
                text: Tema.sigiloTexto(linha.item.detalhe || "")
                textFormat: Text.PlainText
                color: Tema.apagadoDaFaixa(linha.par)
                font.family: Tema.serifa
                font.italic: true
                font.pixelSize: 12
            }
            Text {
                text: (linha.dinheiro ? "R$ " + Tema.valor(linha.item.valor)
                       : linha.sensivel && Tema.oculto ? "••" : linha.item.valor || "")
                textFormat: Text.PlainText
                color: Tema.tintaDaFaixa(linha.par)
                font.family: Tema.texto
                font.weight: Font.Bold
                font.pixelSize: 15
                horizontalAlignment: Text.AlignRight
                Layout.minimumWidth: 86
            }
        }
        RowLayout {
            spacing: 10
            Layout.fillWidth: true
            Layout.leftMargin: linha.posicao > 0 ? 28 : 0
            Rectangle {
                Layout.fillWidth: true
                implicitHeight: 8
                radius: 4
                color: Tema.roxo && linha.par ? Qt.rgba(1, 1, 1, 0.22) : Tema.vela(0.5)
                Rectangle {
                    height: parent.height
                    radius: 4
                    width: Math.max(linha.item.parte > 0 ? 8 : 0, parent.width * (linha.item.parte || 0))
                    color: Tema.roxo && linha.par ? Qt.lighter(linha.cor, 2.2) : linha.cor
                    opacity: 0.55
                    Behavior on width { NumberAnimation { duration: 260; easing.type: Easing.OutCubic } }
                }
            }
            Text {
                text: linha.item.percentual || ""
                textFormat: Text.PlainText
                color: Tema.tintaSuaveDaFaixa(linha.par)
                font.family: Tema.texto
                font.pixelSize: 12
                horizontalAlignment: Text.AlignRight
                Layout.preferredWidth: 34
            }
        }
    }
}
