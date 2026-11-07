import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Templates as T
import rustinho

// Janelinha por cima da tela (pop-up), com o fundo do Rustinho.
// V-4 (0.9.18): a seta de retorno no canto de cima, à esquerda (como em
// comoficará.png). A seta e o Esc fazem o mesmo: fecham a janelinha sem
// fazer nada. Quem precisa de mais ao fechar usa onClosed (ex.: a devolução
// feita volta para a tela de antes). Os botões de texto que só fechavam
// ("Cancelar", "Voltar", "Fechar") saíram; os que fazem algo ficam.
Popup {
    id: painel
    property color cor: Tema.pergaminho
    property bool comSeta: true
    modal: true
    focus: true
    anchors.centerIn: T.Overlay.overlay
    width: Math.min(560, Tema.largura - 24)
    padding: Tema.estreito ? 18 : 30
    // O conteúdo começa logo abaixo da área de toque da seta.
    topPadding: comSeta ? setaDoPainel.y + setaDoPainel.height : padding
    closePolicy: Popup.CloseOnEscape
    // O pop-up herda as cores da janela, não da tela: diz a sua superfície.
    palette.window: Tema.superficie(painel.cor)
    T.Overlay.modal: Rectangle { color: Tema.veu }
    background: Rectangle {
        color: painel.cor
        radius: Tema.raio + 3
        Seta {
            id: setaDoPainel
            visible: painel.comSeta
            // O vidro da seta um pouco à esquerda do conteúdo, como no print,
            // com folga das bordas da janelinha (0.9.18: a seta tem vidro).
            x: Math.max(8, painel.leftPadding - 12)
            y: Tema.estreito ? 8 : 12
            onClicked: painel.close()
        }
    }
}
