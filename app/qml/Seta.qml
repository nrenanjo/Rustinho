import QtQuick
import QtQuick.Controls.Basic
import rustinho

// A seta de retorno (0.9.18, V-2 e V-4): no canto de cima, à esquerda, das
// telas (Cabecalho) e das janelinhas (Painel, Dialogo). Faz o mesmo que o
// Esc (e o "voltar" do Android). Área de toque de 40 × 40 px no computador e
// 48 × 48 dp no celular (Tema.toqueSeta). O texto ao lado (o destino) é a
// decisão D-V1 (Tema.setaComTexto); a cor, a D-V2 (Tema.setaNo).
Button {
    id: seta
    // Para onde a seta leva ("Início", "Compras"...): fica ao lado da seta
    // (D-V1). Vazio nas janelinhas (lá a seta só fecha).
    property string destino: ""
    // O nome do botão para leitores de tela e para os testes das telas
    // (--textos-da-tela), que acham por ele o botão sem texto.
    readonly property string nomeAcessivel: destino !== "" ? destino : "Voltar"
    // A superfície embaixo: no roxo, clara no fundo da tela e escura num
    // painel claro. Nos outros temas não importa.
    readonly property color superficie: Tema.roxo ? palette.window : Tema.fundo
    readonly property color cor: Tema.setaNo(superficie)
    readonly property real tamanhoIcone: 18
    // Do canto da área de toque até o desenho da seta: quem põe a seta usa
    // -folga na margem para o desenho ficar alinhado com o conteúdo.
    readonly property real folga: (Tema.toqueSeta - tamanhoIcone) / 2

    // Os testes acham a seta (e onde ela está) por este nome.
    objectName: "setaDeRetorno"
    text: Tema.setaComTexto ? destino : ""
    implicitWidth: Math.max(Tema.toqueSeta, leftPadding + contentItem.implicitWidth + rightPadding)
    implicitHeight: Tema.toqueSeta
    leftPadding: folga
    rightPadding: text !== "" ? 12 : folga
    topPadding: 0
    bottomPadding: 0
    font.family: Tema.texto
    font.pixelSize: 14
    font.weight: Font.DemiBold
    focusPolicy: Qt.StrongFocus
    Accessible.name: destino !== "" ? "Voltar para " + destino : "Voltar"

    contentItem: Item {
        implicitWidth: linha.implicitWidth
        implicitHeight: linha.implicitHeight
        Row {
            id: linha
            anchors.verticalCenter: parent.verticalCenter
            spacing: 8
            Icone {
                nome: "voltar"
                cor: seta.cor
                tamanho: seta.tamanhoIcone
                anchors.verticalCenter: parent.verticalCenter
            }
            Text {
                visible: seta.text !== ""
                text: seta.text
                font: seta.font
                color: seta.cor
                anchors.verticalCenter: parent.verticalCenter
            }
        }
    }

    background: Rectangle {
        radius: 8
        color: seta.down ? Tema.sombraNo(0.13, seta.superficie)
                         : seta.hovered ? Tema.sombraNo(0.07, seta.superficie) : "transparent"
        border.width: seta.visualFocus ? 2 : 0
        border.color: Tema.azulEscuroNo(seta.superficie)
    }

    // Enter também aperta (no Qt, só a barra de espaço apertava).
    Keys.onReturnPressed: seta.clicked()
    Keys.onEnterPressed: seta.clicked()

    HoverHandler { cursorShape: Qt.PointingHandCursor }
}
