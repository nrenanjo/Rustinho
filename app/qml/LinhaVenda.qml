import QtQuick
import QtQuick.Layouts
import rustinho

// Uma venda ou orçamento na lista.
Rectangle {
    id: linha
    property var item: ({})
    property int indice: 0
    property bool atual: false

    signal abrir()

    readonly property bool par: (indice + 1) % 2 === 0
    // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
    readonly property color corBase: Tema.faixa(par)
    readonly property color corTexto: atual ? Tema.tintaAtual : Tema.tintaDaFaixa(par)
    readonly property color corSuave: atual ? Tema.claroSuave(0.78) : Tema.tintaSuaveDaFaixa(par)
    readonly property bool orcamento: item.tipo === "orcamento"
    readonly property bool apagada: ["cancelada", "cancelado"].includes(item.situacao)

    radius: Tema.raio
    color: atual ? Tema.atual : sobre.hovered ? Qt.darker(corBase, 1.04) : corBase
    palette.window: Tema.superficie(color)
    implicitHeight: Tema.estreito ? 66 : 62

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Tema.estreito ? 12 : 18
        anchors.rightMargin: Tema.estreito ? 12 : 20
        spacing: Tema.estreito ? 10 : 16
        opacity: linha.apagada ? 0.55 : 1

        Text {
            text: linha.item.numero
            textFormat: Text.PlainText
            color: linha.atual ? Tema.tintaAtual : Tema.douradoDaFaixa(linha.par)
            font.family: Tema.titulo
            font.weight: Font.Bold
            font.pixelSize: Tema.estreito ? 22 : 26
            Layout.preferredWidth: Tema.estreito ? -1 : 54
        }
        Text {
            visible: !Tema.estreito
            text: Qt.formatDateTime(new Date(linha.item.criadoEm), "dd/MM HH:mm")
            textFormat: Text.PlainText
            color: linha.corSuave
            font.family: Tema.texto
            font.pixelSize: 14
            Layout.preferredWidth: 90
        }
        ColumnLayout {
            spacing: 0
            Layout.fillWidth: true
            Text {
                text: linha.item.cliente || (linha.orcamento ? "Sem cliente" : "Consumidor")
                textFormat: Text.PlainText
                color: linha.corTexto
                font.family: Tema.fonteLinha
                font.weight: Font.Bold
                font.pixelSize: 20
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            Text {
                text: [Tema.estreito ? Qt.formatDateTime(new Date(linha.item.criadoEm), "dd/MM HH:mm") : "",
                       linha.item.itens + (linha.item.itens === 1 ? " item" : " itens"), linha.item.caixa, linha.item.vendedor,
                       linha.orcamento ? "válido até " + linha.item.validade : linha.item.formas].filter(x => !!x).join("  ·  ")
                textFormat: Text.PlainText
                color: linha.corSuave
                font.family: Tema.serifa
                font.italic: true
                font.pixelSize: 13
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }
        Rectangle {
            visible: linha.apagada || linha.orcamento
            radius: height / 2
            // No roxo, o selo neutro é claro nas duas faixas (texto escuro).
            color: linha.atual ? Tema.chipAtual : linha.apagada ? (Tema.roxo ? Tema.papel : Tema.sombra(0.08))
                   : linha.item.situacao === "vendido" ? Tema.verde : linha.item.vencido ? Tema.rosa : (Tema.roxo ? Tema.papel : Tema.vela(0.5))
            implicitWidth: chip.implicitWidth + 18
            implicitHeight: chip.implicitHeight + 6
            Text {
                id: chip
                anchors.centerIn: parent
                text: linha.apagada ? (linha.orcamento ? "Cancelado" : "Cancelada")
                      : linha.item.situacao === "vendido" ? "Vendido" : linha.item.vencido ? "Vencido" : "Em aberto"
                color: linha.atual ? Tema.tintaAtual : linha.item.vencido ? Tema.vermelho : Tema.tintaSuave
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 12
            }
        }
        Text {
            text: "R$ " + linha.item.total
            textFormat: Text.PlainText
            color: linha.corTexto
            font.family: Tema.texto
            font.weight: Font.Bold
            font.pixelSize: Tema.estreito ? 16 : 18
            horizontalAlignment: Text.AlignRight
            Layout.preferredWidth: Tema.estreito ? -1 : 120
        }
    }

    HoverHandler { id: sobre; cursorShape: Qt.PointingHandCursor }
    TapHandler { onTapped: linha.abrir() }
}
