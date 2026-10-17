import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Uma parcela do crediário, com a caixa para escolher o que receber.
Rectangle {
    id: linha
    property var item: ({})
    property int indice: 0
    property bool marcada: false

    signal marcar(bool sim)
    signal cancelar()
    signal abrirVenda()

    readonly property bool fechada: item.situacao === "paga" || item.situacao === "cancelada"
    readonly property var cores: ({ vencida: Tema.rosa, hoje: Tema.amarelo, aberta: Tema.vela(0.55),
                                    paga: Tema.verde, cancelada: Tema.sombra(0.08) })
    readonly property var nomes: ({ vencida: "Vencida", hoje: "Vence hoje", aberta: "Em aberto", paga: "Paga", cancelada: "Cancelada" })

    radius: Tema.raioCampo
    color: (indice + 1) % 2 === 0 ? Tema.azul : Tema.pergaminho
    implicitHeight: Tema.estreito ? Math.max(58, conteudoParcela.implicitHeight + 16) : 58

    RowLayout {
        id: conteudoParcela
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 14
        spacing: 12
        opacity: linha.item.situacao === "cancelada" ? 0.55 : 1

        Marcar {
            visible: !linha.fechada
            checked: linha.marcada
            onToggled: linha.marcar(checked)
            Layout.preferredWidth: 30
        }
        Item { visible: linha.fechada; Layout.preferredWidth: 30 }

        ColumnLayout {
            spacing: 0
            Layout.fillWidth: true
            Text {
                text: linha.item.descricao + "  ·  " + linha.item.parcela
                color: Tema.tinta
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 15
                elide: Text.ElideRight
                wrapMode: Tema.estreito ? Text.WordWrap : Text.NoWrap
                maximumLineCount: 2
                Layout.fillWidth: true
                TapHandler {
                    enabled: !linha.item.manual
                    onTapped: linha.abrirVenda()
                }
                HoverHandler { cursorShape: linha.item.manual ? Qt.ArrowCursor : Qt.PointingHandCursor }
            }
            Text {
                text: [Tema.estreito ? (linha.nomes[linha.item.situacao] || "") : "", "vence " + linha.item.vencimento,
                       linha.item.pago !== "0,00" ? "pago R$ " + linha.item.pago + " de R$ " + linha.item.valor : "",
                       linha.item.diasAtraso > 0 ? linha.item.diasAtraso + " dias de atraso" : ""].filter(x => !!x).join("  ·  ")
                color: linha.item.situacao === "vencida" ? Tema.vermelho : Tema.tintaSuave
                font.family: Tema.texto
                font.pixelSize: 13
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }
        Rectangle {
            visible: !Tema.estreito
            radius: height / 2
            color: linha.cores[linha.item.situacao] || Tema.papel
            implicitWidth: chip.implicitWidth + 18
            implicitHeight: chip.implicitHeight + 6
            Text {
                id: chip
                anchors.centerIn: parent
                text: linha.nomes[linha.item.situacao] || ""
                color: linha.item.situacao === "vencida" ? Tema.vermelho : Tema.tinta
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 12
            }
        }
        ColumnLayout {
            spacing: 0
            Layout.preferredWidth: Tema.estreito ? 77 : 110
            Text {
                text: "R$ " + (linha.fechada ? linha.item.valor : linha.item.saldo)
                color: Tema.tinta
                font.family: Tema.texto
                font.weight: Font.Bold
                font.pixelSize: 16
                Layout.alignment: Qt.AlignRight
            }
            Text {
                visible: linha.item.juros !== ""
                text: "+ R$ " + linha.item.juros + " juros"
                color: Tema.vermelho
                font.family: Tema.texto
                font.pixelSize: 12
                Layout.alignment: Qt.AlignRight
            }
        }
        Item {
            visible: !Tema.estreito || (linha.item.manual && !linha.fechada && linha.item.pago === "0,00")
            Layout.preferredWidth: Tema.estreito ? 50 : 64
            implicitHeight: 32
            Botao {
                anchors.fill: parent
                visible: linha.item.manual && !linha.fechada && linha.item.pago === "0,00"
                text: "Tirar"
                tipo: "discreto"
                onClicked: linha.cancelar()
            }
        }
    }
}
