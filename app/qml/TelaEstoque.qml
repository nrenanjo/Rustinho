import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Estoque que orienta: o que comprar (e quanto), as encomendas de clientes,
// o que mais vende e o que está parado. Uma parte de cada vez, nas pílulas.
// Faz parte da pílula unida "Estoque / Fornecedores" (0.9.15).
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    property string secao: "comprar"

    function aoEsc() {
        if (conteudo.item && typeof conteudo.item.aoEsc === "function" && conteudo.item.aoEsc())
            return
        janela.voltar()
    }

    function imprimirLista() {
        const r = JSON.parse(Sistema.imprimir("lista_compra", ""))
        if (!r.ok)
            janela.avisar(r.erro, "erro")
        else if (r.dados.impresso)
            janela.avisar("Enviado para a impressora.", "ok")
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        spacing: 14

        Cabecalho {
            sensivel: true
            titulo: "Estoque / Fornecedores"
            subtitulo: "O que comprar, o que o cliente encomendou, o que vende e o que está parado"
            Layout.fillWidth: true
            onVoltar: tela.janela.voltar()
            Botao {
                visible: tela.secao === "comprar"
                text: "Imprimir lista"
                icone: "impressora"
                tipo: "secundario"
                onClicked: tela.imprimirLista()
            }
            Botao {
                visible: tela.secao === "encomendas"
                text: "Nova encomenda"
                icone: "mais"
                tipo: "secundario"
                onClicked: conteudo.item.nova()
            }
        }

        // Pílula unida "Estoque / Fornecedores": a linha de cima troca entre as duas.
        Pilulas {
            modelo: [{ id: "estoque", nome: "Estoque" }, { id: "fornecedores", nome: "Fornecedores" }]
            atual: "estoque"
            onEscolhido: id => { if (id === "fornecedores") tela.janela.substituir("TelaPessoas", { papel: "fornecedor" }) }
        }

        Pilulas {
            pequenas: true
            modelo: [
                { id: "comprar", nome: "O que comprar" },
                { id: "encomendas", nome: "Encomendas" },
                { id: "vendidos", nome: "Mais vendidos" },
                { id: "parados", nome: "Parados" }
            ]
            atual: tela.secao
            onEscolhido: id => tela.secao = id
        }

        Loader {
            id: conteudo
            Layout.fillWidth: true
            Layout.fillHeight: true
            source: ({
                comprar: "SecaoComprar.qml",
                encomendas: "SecaoEncomendas.qml",
                vendidos: "SecaoVendidos.qml",
                parados: "SecaoParados.qml"
            })[tela.secao]
            onLoaded: item.forceActiveFocus()
        }
    }

    StackView.onActivated: {
        if (conteudo.item && typeof conteudo.item.carregar === "function")
            conteudo.item.carregar()
    }
}
