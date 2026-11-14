import QtQuick
import QtQuick.Controls.Basic
import rustinho

// Botão do Rustinho. Tipos: principal (tinta; no roxo, magenta), secundario
// (pergaminho; no roxo, rosa claro), discreto (sem fundo) e perigo (vermelho).
Button {
    id: botao
    property string tipo: "principal"
    property string atalho: ""
    property string icone: ""
    property real tamanhoIcone: 18
    // A altura do desenho do botão (o fundo). A15 (0.9.18): no celular, a
    // área de toque tem pelo menos 48 × 48 dp (Tema.toqueMinimo) e o desenho
    // fica do tamanho de sempre, no meio dela (as margens de dentro, insets).
    // No computador, a área de toque é o próprio desenho, como antes.
    property real altura: 42
    // A largura do desenho, quando quem usa fixa uma (-1 = a do conteúdo).
    property real largura: -1
    readonly property real larguraDesenho: largura >= 0 ? largura : implicitContentWidth + leftPadding + rightPadding

    implicitWidth: Math.max(larguraDesenho, Tema.toqueMinimo)
    implicitHeight: Math.max(altura, Tema.toqueMinimo)
    leftInset: (implicitWidth - larguraDesenho) / 2
    rightInset: leftInset
    topInset: (implicitHeight - altura) / 2
    bottomInset: topInset
    leftPadding: tipo === "discreto" ? 10 : 18
    rightPadding: tipo === "discreto" ? 12 : 18
    font.family: Tema.texto
    font.pixelSize: 15
    font.weight: Font.DemiBold
    focusPolicy: Qt.StrongFocus
    opacity: enabled ? 1 : 0.45

    readonly property bool escuro: tipo === "principal" || tipo === "perigo"
    // A superfície embaixo (0.9.18): no roxo, o discreto no fundo da tela tem
    // texto claro e, num painel, escuro. Nos outros temas não importa.
    readonly property color superficie: Tema.roxo ? palette.window : Tema.fundo
    readonly property color corTexto: tipo === "principal" ? Tema.sobrePrincipal
                                      : tipo === "perigo" ? Tema.claro
                                      : tipo === "secundario" ? Tema.tintaBotao
                                      : Tema.tintaNo(superficie)

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
                textFormat: Text.PlainText
                font: botao.font
                color: botao.corTexto
                anchors.verticalCenter: parent.verticalCenter
            }
            Text {
                visible: botao.atalho !== "" && !Tema.celular && !Tema.estreito
                text: botao.atalho
                textFormat: Text.PlainText
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
            case "principal": return baixo ? Tema.principalBaixo : sobre ? Tema.principalSobre : Tema.principal
            case "perigo": return baixo ? Tema.perigoBaixo : sobre ? Tema.perigoSobre : Tema.vermelho
            case "secundario": return baixo ? Tema.botaoBaixo : sobre ? Tema.botaoSobre : Tema.botao
            default: return baixo ? Tema.sombraNo(0.13, botao.superficie) : sobre ? Tema.sombraNo(0.07, botao.superficie) : "transparent"
            }
        }
        border.width: botao.visualFocus ? 2 : 0
        border.color: botao.tipo === "discreto" ? Tema.azulEscuroNo(botao.superficie) : Tema.azulEscuro
    }

    // Enter também aperta o botão (no Qt, só a barra de espaço apertava).
    Keys.onReturnPressed: botao.clicked()
    Keys.onEnterPressed: botao.clicked()

    HoverHandler { cursorShape: Qt.PointingHandCursor }
}
