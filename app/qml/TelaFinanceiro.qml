import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Financeiro: o que a loja deve (A pagar), onde está o dinheiro (Contas da
// loja), cheques, fluxo de caixa e comissões. Uma parte de cada vez,
// escolhida nas pílulas do topo.
FocusScope {
    id: tela
    // Como esta tela aparece na orientação da navegação guiada ("Depois o
    // Rustinho volta para …").
    readonly property string nomeNaNavegacao: "Financeiro"
    // Se um gerente liberou esta tela para um caixa, acaba ao sair dela.
    Component.onDestruction: Api.chamar("gerente_encerrar")
    readonly property var janela: ApplicationWindow.window
    property string secao: "pagar"
    property var resumo: null

    function carregarResumo() {
        const r = Api.chamar("financeiro_resumo")
        if (r.ok)
            resumo = r.dados
    }

    // O Esc e a seta fazem o mesmo (D-V3, decidida pelo dono em 07/11/2026:
    // os dois voltam direto). limparBuscaAntes só deixaria a seção limpar a
    // busca antes se a regra mudasse em Main.
    function aoEsc(daSeta) {
        if (janela.limparBuscaAntes(daSeta) && conteudo.item && typeof conteudo.item.aoEsc === "function" && conteudo.item.aoEsc())
            return
        janela.voltar()
    }

    // BUG-20261120-FN34: em janela baixa (340 x 480 ou celular deitado), a tela
    // inteira rola verticalmente para que os filtros, Categorias, busca e lista
    // nunca fiquem inacessíveis abaixo da borda. Em janelas normais, o layout
    // fixo permanece idêntico.
    readonly property bool telaRolavel: Tema.baixo
    readonly property bool telaMinima: Tema.baixo && Tema.largura < 420

    Flickable {
        id: rolagemDoFinanceiro
        objectName: "rolagemDoFinanceiro"
        anchors.fill: parent
        clip: true
        interactive: tela.telaRolavel
        contentWidth: width
        contentHeight: tela.telaRolavel ? Math.max(height, corpoFinanceiro.implicitHeight + 2 * Tema.margem) : height
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { visible: tela.telaRolavel }

        ColumnLayout {
            id: corpoFinanceiro
            x: Tema.margem
            y: Tema.margem - 12
            width: rolagemDoFinanceiro.width - 2 * Tema.margem - (tela.telaRolavel ? 10 : 0)
            height: tela.telaRolavel ? Math.max(rolagemDoFinanceiro.height - (Tema.margem - 12) - Tema.margem, implicitHeight)
                                     : rolagemDoFinanceiro.height - (Tema.margem - 12) - Tema.margem
            spacing: tela.telaMinima ? 8 : 14

            Cabecalho {
                sensivel: true
                titulo: "Financeiro"
                subtitulo: tela.telaMinima ? "" : "O que a loja deve, onde está o dinheiro e o que vem pela frente"
                Layout.fillWidth: true
                Botao {
                    visible: tela.secao === "pagar"
                    text: "Lançar conta"
                    icone: "mais"
                    tipo: "secundario"
                    onClicked: conteudo.item.lancar()
                }
                Botao {
                    visible: tela.secao === "contas"
                    text: "Nova conta"
                    icone: "mais"
                    tipo: "secundario"
                    onClicked: conteudo.item.novaConta()
                }
            }

            Pilulas {
                modelo: [
                    { id: "pagar", nome: "A pagar" },
                    { id: "contas", nome: "Contas da loja" },
                    { id: "cheques", nome: "Cheques" },
                    { id: "fluxo", nome: "Fluxo de caixa" },
                    { id: "comissoes", nome: "Comissões" }
                ]
                atual: tela.secao
                onEscolhido: id => tela.secao = id
            }

            Loader {
                id: conteudo
                Layout.fillWidth: true
                Layout.fillHeight: !tela.telaRolavel
                Layout.preferredHeight: tela.telaRolavel ? Math.max(360, item ? item.implicitHeight : 360) : -1
                source: ({
                    pagar: "SecaoPagar.qml",
                    contas: "SecaoContas.qml",
                    cheques: "SecaoCheques.qml",
                    fluxo: "SecaoFluxo.qml",
                    comissoes: "SecaoComissoes.qml"
                })[tela.secao]
                onLoaded: item.forceActiveFocus()
            }
        }
    }

    // 10b (C8, dev3.3): de Comissões, "Ver no A pagar" abre a ficha da conta
    // da comissão; ao fechar a ficha, volta a Comissões com o fechamento
    // destacado. Só navega: nada muda de valor por aqui.
    function abrirContaPagar(contaId, fechamentoId) {
        // M-4 da revisão: o período de Comissões volta igual.
        const antes = conteudo.item && typeof conteudo.item.estado === "function" ? conteudo.item.estado() : null
        secao = "pagar"
        Qt.callLater(() => {
            if (!conteudo.item || typeof conteudo.item.abrirConta !== "function")
                return
            conteudo.item.abrirConta(contaId, () => {
                tela.secao = "comissoes"
                Qt.callLater(() => {
                    if (conteudo.item && typeof conteudo.item.restaurar === "function")
                        conteudo.item.restaurar(antes)
                    if (conteudo.item && typeof conteudo.item.mostrarFechamento === "function")
                        conteudo.item.mostrarFechamento(fechamentoId)
                })
            })
        })
    }
    Connections {
        target: conteudo.item
        ignoreUnknownSignals: true
        function onVerConta(contaId, fechamentoId) { tela.abrirContaPagar(contaId, fechamentoId) }
    }

    StackView.onActivated: {
        if (janela && janela.retornandoDeF11)
            return
        carregarResumo()
        if (conteudo.item && typeof conteudo.item.carregar === "function")
            conteudo.item.carregar()
    }
}
