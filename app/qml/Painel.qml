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
    // Decisão 1a do proprietário (18/11/2026): no celular (Android), a
    // janelinha não tem a seta. O "voltar" do Android (botão e gesto) a fecha:
    // o Qt do Android trata a tecla Voltar como o Esc (CloseOnEscape). O
    // conteúdo sobe para o lugar da seta. O computador continua com ela.
    readonly property bool mostraSeta: comSeta && !Tema.celular
    modal: true
    focus: true
    anchors.centerIn: T.Overlay.overlay
    width: Math.min(560, Tema.largura - 24)
    // H06 (7.2): a janelinha nunca passa da janela (fica uma folga em cima e
    // embaixo). A que pode ficar mais alta que isso rola o miolo e deixa os
    // botões à vista (ex.: o Receber da venda, com crediário em 1366 × 768).
    readonly property real alturaMaxima: Math.max(160, (T.Overlay.overlay ? T.Overlay.overlay.height : Tema.altura)
                                                       - 2 * (Tema.estreito ? 8 : 16))
    height: Math.min(implicitHeight, alturaMaxima)
    padding: Tema.estreito ? 18 : 30
    // O conteúdo começa logo abaixo da área de toque da seta.
    topPadding: mostraSeta ? setaDoPainel.y + setaDoPainel.height : padding
    closePolicy: Popup.CloseOnEscape
    // A-1 da revisão (C8, caso 7): a janela sabe quais janelinhas estão abertas, na ordem, para fechar e reabrir a
    // cadeia inteira (a ficha da conta e a pergunta do motivo) quando a guia leva a outra tela.
    // N-1 da re-revisão: a janela é guardada ao abrir, para a janelinha sair do registro também quando é destruída
    // aberta (a tela dela saiu sem o `closed`: a seção trocada pela volta do 10b, ou a sessão encerrada).
    property var _janelaDoRegistro: null
    Connections {
        target: painel
        function onOpened() {
            const j = painel.ApplicationWindow.window
            if (j && typeof j.registrarJanelinha === "function") {
                painel._janelaDoRegistro = j
                j.registrarJanelinha(painel, true)
            }
        }
        function onClosed() {
            const j = painel._janelaDoRegistro || painel.ApplicationWindow.window
            if (j && typeof j.registrarJanelinha === "function")
                j.registrarJanelinha(painel, false)
        }
    }
    Component.onDestruction: {
        if (painel._janelaDoRegistro && typeof painel._janelaDoRegistro.registrarJanelinha === "function")
            painel._janelaDoRegistro.registrarJanelinha(painel, false)
    }
    // O pop-up herda as cores da janela, não da tela: diz a sua superfície.
    palette.window: Tema.superficie(painel.cor)
    T.Overlay.modal: Rectangle { color: Tema.veu }
    background: Rectangle {
        // Os testes das telas sabem por este nome que há uma janelinha aberta.
        objectName: "fundoDaJanelinha"
        color: painel.cor
        radius: Tema.raio + 3
        Seta {
            id: setaDoPainel
            visible: painel.mostraSeta
            // O vidro da seta um pouco à esquerda do conteúdo, como no print,
            // com folga das bordas da janelinha (0.9.18: a seta tem vidro).
            x: Math.max(8, painel.leftPadding - 12)
            y: Tema.estreito ? 8 : 12
            onClicked: painel.close()
        }
    }
}
