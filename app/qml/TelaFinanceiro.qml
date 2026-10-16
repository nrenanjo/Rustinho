import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Financeiro: o que a loja deve (A pagar), onde está o dinheiro (Contas da
// loja), cheques, fluxo de caixa e comissões. Uma parte de cada vez,
// escolhida nas pílulas do topo.
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    property string secao: "pagar"
    property var resumo: null

    function carregarResumo() {
        const r = Api.chamar("financeiro_resumo")
        if (r.ok)
            resumo = r.dados
    }

    function aoEsc() {
        if (conteudo.item && typeof conteudo.item.aoEsc === "function" && conteudo.item.aoEsc())
            return
        janela.voltar()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        spacing: 14

        Cabecalho {
            sensivel: true
            titulo: "Financeiro"
            subtitulo: "O que a loja deve, onde está o dinheiro e o que vem pela frente"
            Layout.fillWidth: true
            onVoltar: tela.janela.voltar()
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
            Layout.fillHeight: true
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

    StackView.onActivated: {
        carregarResumo()
        if (conteudo.item && typeof conteudo.item.carregar === "function")
            conteudo.item.carregar()
    }
}
