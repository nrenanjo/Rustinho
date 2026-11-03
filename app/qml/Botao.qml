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
                visible: botao.atalho !== "" && !Tema.celular && !Tema.estreito
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
            case "principal": return baixo ? Tema.principalBaixo : sobre ? Tema.principalSobre : Tema.tinta
            case "perigo": return baixo ? Tema.perigoBaixo : sobre ? Tema.perigoSobre : Tema.vermelho
            case "secundario": return baixo ? Tema.botaoBaixo : sobre ? Tema.botaoSobre : Tema.botao
            default: return baixo ? Tema.sombra(0.13) : sobre ? Tema.sombra(0.07) : "transparent"
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
