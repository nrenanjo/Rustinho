import QtQuick
import QtQuick.Layouts
import rustinho

// Uma conta a pagar (parcela) na lista do Financeiro.
Rectangle {
    id: linha
    property var item: ({})
    property int indice: 0

    signal abrir()
    signal pagar()

    readonly property bool fechada: item.situacao === "paga" || item.situacao === "cancelada"
    readonly property var cores: ({ vencida: Tema.rosa, hoje: Tema.amarelo, aberta: Tema.papel,
                                    paga: Tema.verde, cancelada: Tema.sombra(0.08) })
    readonly property var nomes: ({ vencida: "Vencida", hoje: "Vence hoje", aberta: "Em aberto", paga: "Paga", cancelada: "Tirada" })

    radius: Tema.raio
    color: sobre.hovered ? Qt.darker(corBase, 1.04) : corBase
    readonly property color corBase: (indice + 1) % 2 === 0 ? Tema.azul : Tema.pergaminho
    implicitHeight: 64

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Tema.estreito ? 12 : 18
        anchors.rightMargin: 14
        spacing: 14
        opacity: linha.item.situacao === "cancelada" ? 0.55 : 1

        ColumnLayout {
            visible: !Tema.estreito
            spacing: 0
            Layout.preferredWidth: 96
            Text {
                text: linha.item.vencimento || ""
                color: linha.item.situacao === "vencida" ? Tema.vermelho : Tema.tinta
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 15
            }
            Text {
                text: linha.item.diasAtraso > 0 ? linha.item.diasAtraso + (linha.item.diasAtraso === 1 ? " dia" : " dias") + " atrasada" : (linha.item.parcela !== "1/1" ? "parcela " + linha.item.parcela : "")
                visible: text !== ""
                color: linha.item.diasAtraso > 0 ? Tema.vermelho : Tema.tintaSuave
                font.family: Tema.texto
                font.pixelSize: 12
            }
        }

        ColumnLayout {
            spacing: 0
            Layout.fillWidth: true
            Text {
                text: linha.item.descricao || ""
                color: Tema.tinta
                font.family: Tema.titulo
                font.weight: Font.Bold
                font.pixelSize: 20
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            Text {
                text: [Tema.estreito ? "vence " + (linha.item.vencimento || "") + (linha.item.diasAtraso > 0 ? " (" + linha.item.diasAtraso + (linha.item.diasAtraso === 1 ? " dia" : " dias") + " atrasada)" : "") : "",
                       linha.item.fornecedor && linha.item.descricao.indexOf(linha.item.fornecedor) < 0 ? linha.item.fornecedor : "",
                       linha.item.categoria, linha.item.documento].filter(x => !!x).join("  ·  ")
                visible: text !== ""
                color: Tema.estreito && linha.item.situacao === "vencida" ? Tema.vermelho : Tema.tintaSuave
                font.family: Tema.serifa
                font.italic: true
                font.pixelSize: 13
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }

        Chip {
            visible: !Tema.estreito && texto !== ""
            texto: linha.nomes[linha.item.situacao] || ""
            color: linha.cores[linha.item.situacao] || Tema.papel
            corTexto: linha.item.situacao === "vencida" ? Tema.vermelho : Tema.tinta
        }

        ColumnLayout {
            spacing: 0
            Layout.preferredWidth: Tema.estreito ? -1 : 120
            Text {
                text: "R$ " + (linha.fechada ? linha.item.valor : linha.item.saldo)
                color: Tema.tinta
                font.family: Tema.texto
                font.weight: Font.Bold
                font.pixelSize: 17
                Layout.alignment: Qt.AlignRight
            }
            Text {
                visible: !linha.fechada && linha.item.pago !== "0,00"
                text: "de R$ " + linha.item.valor
                color: Tema.tintaSuave
                font.family: Tema.texto
                font.pixelSize: 12
                Layout.alignment: Qt.AlignRight
            }
        }

        Item {
            Layout.preferredWidth: Tema.estreito ? 64 : 84
            implicitHeight: 36
            Botao {
                anchors.fill: parent
                visible: !linha.fechada
                text: "Pagar"
                tipo: "secundario"
                onClicked: linha.pagar()
            }
        }
    }

    HoverHandler { id: sobre; cursorShape: Qt.PointingHandCursor }
    TapHandler { onTapped: linha.abrir() }
}
