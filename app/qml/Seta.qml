import QtQuick
import QtQuick.Controls.Basic
import rustinho

// A seta de retorno (0.9.18, V-2 e V-4): no canto de cima, à esquerda, das
// telas (Cabecalho) e das janelinhas (Painel, Dialogo). Faz o mesmo que o
// Esc (e o "voltar" do Android). Área de toque de 40 × 40 px no computador e
// 48 × 48 dp no celular (Tema.toqueSeta); a cor é a D-V2 (Tema.setaNo).
// Correção definitiva do dono (07/11/2026, decide a D-V1): um botão só, a
// seta para a esquerda no vidro roxo, sem texto, legenda ou nome de destino
// ao lado. Ela substitui os botões "Voltar" e "Cancelar". Para onde volta
// quem decide é a tela (janela.retornar e aoEsc), pelo mesmo caminho do Esc.
Button {
    id: seta
    // O nome do botão só para leitores de tela e para os testes das telas
    // (--textos-da-tela), que acham por ele o botão sem texto. Não aparece.
    readonly property string nomeAcessivel: "Voltar"
    // A superfície embaixo: no roxo, clara no fundo da tela e escura num
    // painel claro. Nos outros temas não importa.
    readonly property color superficie: Tema.roxo ? palette.window : Tema.fundo
    readonly property color cor: Tema.setaNo(superficie)
    readonly property real tamanhoIcone: 18
    // Ordem visual §9 (0.9.18): a seta fica no canto de cima, à esquerda, a
    // cerca de 1 cm das bordas da janela. Com o vidro à mostra, a borda do
    // vidro fica alinhada com o conteúdo (Tema.margem, 32 px) e, no
    // computador, 10 px mais abaixo do que o topo do conteúdo: 32 × 30 px,
    // ≈ 0,85 × 0,8 cm numa tela de 96 dpi. No celular, as margens de sempre.
    readonly property real descida: Tema.estreito ? 0 : 10

    // Os testes acham a seta (e onde ela está) por este nome.
    objectName: "setaDeRetorno"
    implicitWidth: Tema.toqueSeta
    implicitHeight: Tema.toqueSeta
    padding: 0
    focusPolicy: Qt.StrongFocus
    Accessible.name: "Voltar"

    // Só o desenho da seta, no meio do vidro.
    contentItem: Item {
        Icone {
            anchors.centerIn: parent
            nome: "voltar"
            cor: seta.cor
            tamanho: seta.tamanhoIcone
        }
    }

    // Vidro roxo (ordem visual §9): um roxo translúcido redondo, com
    // contorno fino e um brilho na metade de cima; claro sobre o fundo roxo
    // ou escuro, roxo sobre um painel claro (Tema.vidroNo). Mouse em cima e
    // apertado deixam o vidro mais denso; o foco do teclado, o contorno de
    // destaque.
    background: Rectangle {
        radius: height / 2
        color: seta.down ? Tema.vidroBaixoNo(seta.superficie)
                         : seta.hovered ? Tema.vidroSobreNo(seta.superficie) : Tema.vidroNo(seta.superficie)
        border.width: seta.visualFocus ? 2 : 1
        border.color: seta.visualFocus ? Tema.azulEscuroNo(seta.superficie) : Tema.vidroBordaNo(seta.superficie)
        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: 1
            height: parent.height / 2
            radius: height
            gradient: Gradient {
                GradientStop { position: 0.0; color: Tema.vidroBrilhoNo(seta.superficie) }
                GradientStop { position: 1.0; color: "transparent" }
            }
        }
    }

    // Enter também aperta (no Qt, só a barra de espaço apertava).
    Keys.onReturnPressed: seta.clicked()
    Keys.onEnterPressed: seta.clicked()

    HoverHandler { cursorShape: Qt.PointingHandCursor }
}
