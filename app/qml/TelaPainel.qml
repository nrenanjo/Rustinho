import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Painel: como a loja está indo. Resumo do dia, do mês e do ano, e quem
// compra, de quem a loja compra e como os clientes pagam.
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    property string secao: "resumo"
    property int dias: 30

    function aoEsc() {
        janela.voltar()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        spacing: 14

        Cabecalho {
            sensivel: true
            titulo: "Painel"
            subtitulo: "Como a loja está indo"
            Layout.fillWidth: true
            onVoltar: tela.janela.voltar()
        }

        Linha {
            spacing: 14
            Layout.fillWidth: true
            Pilulas {
                modelo: [
                    { id: "resumo", nome: "Resumo" },
                    { id: "clientes", nome: "Clientes" },
                    { id: "fornecedores", nome: "Fornecedores" },
                    { id: "pagamentos", nome: "Pagamentos" }
                ]
                atual: tela.secao
                onEscolhido: id => tela.secao = id
            }
            Item { Layout.fillWidth: true }
            Pilulas {
                visible: tela.secao !== "resumo"
                pequenas: true
                modelo: [{ id: "30", nome: "30 dias" }, { id: "90", nome: "90 dias" }, { id: "365", nome: "12 meses" }]
                atual: String(tela.dias)
                onEscolhido: id => tela.dias = parseInt(id)
            }
        }

        Loader {
            id: conteudo
            Layout.fillWidth: true
            Layout.fillHeight: true
            source: ({
                resumo: "SecaoResumo.qml",
                clientes: "SecaoClientes.qml",
                fornecedores: "SecaoFornecedores.qml",
                pagamentos: "SecaoPagamentos.qml"
            })[tela.secao]
            onLoaded: {
                if (item.hasOwnProperty("dias"))
                    item.dias = Qt.binding(() => tela.dias)
                item.forceActiveFocus()
            }
        }
    }

    StackView.onActivated: {
        if (conteudo.item && typeof conteudo.item.carregar === "function")
            conteudo.item.carregar()
    }
}
