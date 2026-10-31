import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Pílula de uma função na tela inicial (0.9.15): retângulo horizontal de
// cantos arredondados, ícone num quadrado de fundo pastel à esquerda e o
// nome à direita. O alfinete fica no canto: nas fixadas sempre (para
// soltar); nas outras só ao passar o mouse e enquanto houver vaga (até 4).
Rectangle {
    id: cartao
    property var item: ({})
    property int indice: 0

    signal abrir()
    signal fixar()

    readonly property bool mostraPino: cartao.item.fixo === true || (cartao.item.podeFixar !== false && sobre.hovered && !Tema.celular)

    radius: Tema.raio
    opacity: item.disponivel ? 1 : 0.78
    color: sobre.hovered && item.disponivel ? Qt.darker(Tema.cartao, Tema.escuro ? 0.85 : 1.03) : Tema.cartao
    border.width: foco.activeFocus ? 2 : 1
    border.color: foco.activeFocus ? Tema.azulEscuro : Tema.bordaCartao
    implicitHeight: Tema.estreito ? 64 : 72

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Tema.estreito ? 10 : 12
        anchors.rightMargin: 34
        spacing: Tema.estreito ? 12 : 16

        IconeFuncao {
            funcao: cartao.item.id || ""
            nome: cartao.item.icone || "caixa"
            Layout.alignment: Qt.AlignVCenter
        }

        ColumnLayout {
            spacing: 0
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
            Text {
                text: cartao.item.titulo || ""
                color: cartao.item.disponivel ? Tema.tinta : Tema.tintaSuave
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: Tema.estreito ? 16 : 18
                wrapMode: Text.WordWrap
                maximumLineCount: 2
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            Text {
                visible: !cartao.item.disponivel
                text: "em breve"
                color: Tema.apagado
                font.family: Tema.serifa
                font.italic: true
                font.pixelSize: 13
            }
        }
    }

    Botao {
        id: pino
        tipo: "discreto"
        icone: "pino"
        tamanhoIcone: 15
        implicitHeight: 28
        implicitWidth: 30
        leftPadding: 0
        rightPadding: 0
        anchors.top: parent.top
        anchors.right: parent.right
        anchors.margins: 4
        visible: cartao.mostraPino
        opacity: cartao.item.fixo ? 1 : 0.55
        ToolTip.visible: hovered && !Tema.celular
        ToolTip.delay: 500
        ToolTip.text: cartao.item.fixo ? "Soltar" : "Fixar no começo da tela"
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
    TapHandler {
        onTapped: cartao.abrir()
        // No celular: segurar o dedo fixa ou solta (se houver vaga).
        onLongPressed: if (cartao.item.fixo || cartao.item.podeFixar !== false) cartao.fixar()
    }
}
