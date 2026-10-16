import QtQuick
import QtQuick.Controls.Basic
import rustinho

// Botão do Rustinho. Tipos: principal (tinta), secundario (pergaminho),
// discreto (sem fundo) e perigo (vermelho).
Button {
    id: botao
    property string tipo: "principal"
    property string atalho: ""
    property string icone: ""
    property real tamanhoIcone: 18

    implicitHeight: 42
    leftPadding: tipo === "discreto" ? 10 : 18
    rightPadding: tipo === "discreto" ? 12 : 18
    font.family: Tema.texto
    font.pixelSize: 15
    font.weight: Font.DemiBold
    focusPolicy: Qt.StrongFocus
    opacity: enabled ? 1 : 0.45

    readonly property bool escuro: tipo === "principal" || tipo === "perigo"
    readonly property color corTexto: escuro ? Tema.claro : Tema.tinta

    contentItem: Item {
        implicitWidth: linha.implicitWidth
        implicitHeight: linha.implicitHeight
        Row {
            id: linha
            anchors.centerIn: parent
            spacing: 8
            Icone {
                visible: botao.icone !== ""
                nome: botao.icone
                cor: botao.corTexto
                tamanho: botao.tamanhoIcone
                anchors.verticalCenter: parent.verticalCenter
            }
            Text {
                visible: botao.text !== ""
                text: botao.text
                font: botao.font
                color: botao.corTexto
                anchors.verticalCenter: parent.verticalCenter
            }
            Text {
                visible: botao.atalho !== ""
                text: botao.atalho
                font.family: Tema.texto
                font.pixelSize: 12
                font.weight: Font.Bold
                color: botao.corTexto
                opacity: 0.6
                anchors.verticalCenter: parent.verticalCenter
            }
        }
    }

    background: Rectangle {
        radius: 8
        color: {
            const baixo = botao.down
            const sobre = botao.hovered
            switch (botao.tipo) {
            case "principal": return baixo ? "#1A1714" : sobre ? "#3B352F" : Tema.tinta
            case "perigo": return baixo ? "#7E3930" : sobre ? "#A34D42" : Tema.vermelho
            case "secundario": return baixo ? "#D3C5A8" : sobre ? "#DDD0B5" : "#E6DAC2"
            default: return baixo ? Qt.rgba(0.16, 0.15, 0.13, 0.13) : sobre ? Qt.rgba(0.16, 0.15, 0.13, 0.07) : "transparent"
            }
        }
        border.width: botao.visualFocus ? 2 : 0
        border.color: Tema.azulEscuro
    }

    // Enter também aperta o botão (no Qt, só a barra de espaço apertava).
    Keys.onReturnPressed: botao.clicked()
    Keys.onEnterPressed: botao.clicked()

    HoverHandler { cursorShape: Qt.PointingHandCursor }
}
