import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Cartão de uma função no início (Vender, Peças...), com a letra grande
// como a capitular do livro.
Rectangle {
    id: cartao
    property var item: ({})
    property bool grande: false
    property int indice: 0

    signal abrir()
    signal fixar()

    readonly property bool par: (indice + 1) % 2 === 0
    readonly property color corBase: par ? Tema.azul : Tema.pergaminho

    radius: Tema.raio
    opacity: item.disponivel ? 1 : 0.72
    color: sobre.hovered && item.disponivel ? Qt.darker(corBase, 1.045) : corBase
    implicitHeight: grande ? 156 : 88
    border.width: foco.activeFocus ? 2 : 0
    border.color: Tema.azulEscuro

    Text {
        id: letra
        x: cartao.grande ? 20 : 16
        y: cartao.grande ? 4 : 2
        text: cartao.item.letra || ""
        font.family: Tema.titulo
        font.italic: true
        font.weight: Font.DemiBold
        font.pixelSize: cartao.grande ? 92 : 54
        color: Tema.dourado
        opacity: cartao.item.disponivel ? 0.9 : 0.55
    }

    ColumnLayout {
        anchors.left: parent.left
        anchors.leftMargin: cartao.grande ? 90 : 66
        anchors.right: parent.right
        anchors.rightMargin: 16
        anchors.verticalCenter: parent.verticalCenter
        spacing: cartao.grande ? 4 : 1

        Titulo {
            text: cartao.item.titulo || ""
            font.pixelSize: cartao.grande ? 30 : 24
            font.weight: Font.Bold
            color: cartao.item.disponivel ? Tema.tinta : Tema.tintaSuave
            // Nome comprido (ex.: Conferência fiscal) diminui em vez de cortar.
            fontSizeMode: Text.HorizontalFit
            minimumPixelSize: cartao.grande ? 22 : 18
            Layout.fillWidth: true
        }
        Nota {
            text: cartao.item.descricao || ""
            font.pixelSize: cartao.grande ? 15 : 14
            maximumLineCount: cartao.grande ? 2 : 1
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
        Rotulo {
            visible: !cartao.item.disponivel
            text: "Em breve · etapa " + cartao.item.etapa
            font.pixelSize: 11
            Layout.topMargin: cartao.grande ? 4 : 1
        }
    }

    Botao {
        id: pino
        tipo: "discreto"
        icone: "pino"
        tamanhoIcone: 17
        implicitHeight: 32
        implicitWidth: 34
        leftPadding: 0
        rightPadding: 0
        anchors.top: parent.top
        anchors.right: parent.right
        anchors.margins: 6
        visible: sobre.hovered || cartao.item.fixo === true
        opacity: cartao.item.fixo ? 1 : 0.55
        ToolTip.visible: hovered
        ToolTip.delay: 500
        ToolTip.text: cartao.item.fixo ? "Soltar de Mais usados" : "Fixar em Mais usados"
        onClicked: cartao.fixar()
    }

    Item {
        id: foco
        anchors.fill: parent
        activeFocusOnTab: true
        Keys.onReturnPressed: cartao.abrir()
        Keys.onEnterPressed: cartao.abrir()
        Keys.onSpacePressed: cartao.abrir()
    }

    HoverHandler {
        id: sobre
        cursorShape: cartao.item.disponivel ? Qt.PointingHandCursor : Qt.ArrowCursor
    }
    TapHandler { onTapped: cartao.abrir() }
}
