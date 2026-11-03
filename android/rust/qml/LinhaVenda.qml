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
    readonly property color corBase: par ? Tema.azul : Tema.pergaminho
    readonly property color corTexto: atual ? Tema.claro : Tema.tinta
    readonly property color corSuave: atual ? Tema.claroSuave(0.78) : Tema.tintaSuave
    readonly property bool orcamento: item.tipo === "orcamento"
    readonly property bool apagada: ["cancelada", "cancelado"].includes(item.situacao)

    radius: Tema.raio
    color: atual ? Tema.tinta : sobre.hovered ? Qt.darker(corBase, 1.04) : corBase
    implicitHeight: Tema.estreito ? 66 : 62

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Tema.estreito ? 12 : 18
        anchors.rightMargin: Tema.estreito ? 12 : 20
        spacing: Tema.estreito ? 10 : 16
        opacity: linha.apagada ? 0.55 : 1

        Text {
            text: linha.item.numero
            color: linha.atual ? Tema.claro : Tema.dourado
            font.family: Tema.titulo
            font.weight: Font.Bold
            font.pixelSize: Tema.estreito ? 22 : 26
            Layout.preferredWidth: Tema.estreito ? -1 : 54
        }
        Text {
            visible: !Tema.estreito
            text: Qt.formatDateTime(new Date(linha.item.criadoEm), "dd/MM HH:mm")
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
                color: linha.corTexto
                font.family: Tema.titulo
                font.weight: Font.Bold
                font.pixelSize: 20
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            Text {
                text: [Tema.estreito ? Qt.formatDateTime(new Date(linha.item.criadoEm), "dd/MM HH:mm") : "",
                       linha.item.itens + (linha.item.itens === 1 ? " item" : " itens"), linha.item.caixa, linha.item.vendedor,
                       linha.orcamento ? "válido até " + linha.item.validade : linha.item.formas].filter(x => !!x).join("  ·  ")
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
            color: linha.atual ? Tema.vela(0.12) : linha.apagada ? Tema.sombra(0.08)
                   : linha.item.situacao === "vendido" ? Tema.verde : linha.item.vencido ? Tema.rosa : Tema.vela(0.5)
            implicitWidth: chip.implicitWidth + 18
            implicitHeight: chip.implicitHeight + 6
            Text {
                id: chip
                anchors.centerIn: parent
                text: linha.apagada ? (linha.orcamento ? "Cancelado" : "Cancelada")
                      : linha.item.situacao === "vendido" ? "Vendido" : linha.item.vencido ? "Vencido" : "Em aberto"
                color: linha.atual ? Tema.claro : linha.item.vencido ? Tema.vermelho : Tema.tintaSuave
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 12
            }
        }
        Text {
            text: "R$ " + linha.item.total
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
