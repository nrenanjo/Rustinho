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
    // O conteúdo começa logo abaixo da área de toque da seta, com folga visual (doc 12, ponto 10).
    topPadding: mostraSeta ? setaDoPainel.y + setaDoPainel.height + 8 : padding
    // T058 (pedido do proprietário, 20/11/2026): a janelinha com passos (ex.:
    // a pendência aberta dentro da lista de pendências) dá a função
    // voltarUmPasso, que volta um passo e devolve true; quando não há passo
    // para voltar, devolve false e a seta e o Esc fecham, como sempre. Sem a
    // função, nada muda.
    property var voltarUmPasso: null
    function retornar() {
        if (typeof painel.voltarUmPasso === "function" && painel.voltarUmPasso())
            return
        painel.close()
    }
    closePolicy: typeof voltarUmPasso === "function" ? Popup.NoAutoClose : Popup.CloseOnEscape
    // T054-R1 da revisão: o Esc desta janelinha só vale quando ela é a de cima
    // de todas (a última do registro da janela). Com outra por cima (a senha
    // do gerente, "Qual é a peça?"), o Esc é só da de cima, sem depender da
    // ordem em que o Qt entrega a tecla.
    readonly property bool _eDeCima: {
        const j = painel._janelaDoRegistro
        const lista = j ? j.janelinhasAbertas : null
        return !lista || lista.length === 0 || lista[lista.length - 1] === painel
    }
    Shortcut {
        // O "voltar" do Android chega como Back (com CloseOnEscape, o Qt o
        // tratava como o Esc).
        sequences: ["Esc", "Back"]
        enabled: painel.opened && typeof painel.voltarUmPasso === "function" && painel._eDeCima
        onActivated: painel.retornar()
    }
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
    T.Overlay.modal: Rectangle {
        color: Tema.veu
        // BUG-20261120-TJ8X: bloqueia tanto MouseArea quanto TapHandler da tela atrás do véu modal.
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.AllButtons
            hoverEnabled: true
        }
        TapHandler {
            acceptedButtons: Qt.AllButtons
            grabPermissions: PointerHandler.CanTakeOverFromAnything
        }
    }
    background: Rectangle {
        // Os testes das telas sabem por este nome que há uma janelinha aberta.
        objectName: "fundoDaJanelinha"
        color: painel.cor
        radius: Tema.raio + 3
        // BUG-20261120-TJ8X: absorve toques no fundo da janelinha para não vazarem a TapHandlers abaixo.
        // T035 da 005 (M-1 da R13): com a política padrão, este TapHandler só pegava posse PASSIVA, e o toque numa linha
        // tocável DENTRO da janelinha (ex.: um cliente na lista da nova encomenda) chegava também ao TapHandler da linha
        // da tela de trás (escolhia a encomenda atrás). Com ReleaseWithinBounds ele toma a posse exclusiva do toque que
        // chega ao fundo, e a entrega não desce para a tela de trás.
        TapHandler {
            acceptedButtons: Qt.AllButtons
            gesturePolicy: TapHandler.ReleaseWithinBounds
            grabPermissions: PointerHandler.CanTakeOverFromAnything
        }
        // T033 da 005 (M-1 da R12): o TapHandler acima não ACEITA o clique (a entrega continua para baixo). Um clique que
        // nenhum controle da janelinha aceita (num botão desligado, por exemplo o "Trocar" com impedimento) atravessava
        // a janelinha modal e chegava ao TapHandler de uma linha da tela de trás: abria o detalhe de uma nota por baixo,
        // e o Esc seguinte fechava esse detalhe, e não a janelinha. O MouseArea aceita o clique, e ele para aqui.
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.AllButtons
        }
        Seta {
            id: setaDoPainel
            visible: painel.mostraSeta
            // O vidro da seta um pouco à esquerda do conteúdo, como no print,
            // com folga das bordas da janelinha (0.9.18: a seta tem vidro).
            x: Math.max(8, painel.leftPadding - 12)
            y: Tema.estreito ? 8 : 12
            onClicked: painel.retornar()
        }
    }
}
