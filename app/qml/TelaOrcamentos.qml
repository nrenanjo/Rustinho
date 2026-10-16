import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Orçamentos: lista, busca e atalho para fazer um novo (na tela Vender).
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    property var itens: []
    property bool buscou: false

    function buscar() {
        const r = Api.chamar("vendas_listar", { tipo: "orcamento", texto: busca.texto, limite: 300 })
        if (r.ok) {
            itens = r.dados
            lista.currentIndex = itens.length > 0 ? 0 : -1
        }
        buscou = true
    }

    function abrir(i) {
        if (i >= 0 && i < itens.length)
            janela.abrir("TelaVenda", { vendaId: itens[i].id })
    }

    function aoEsc() {
        if (busca.texto !== "") {
            busca.texto = ""
            buscar()
        } else {
            janela.voltar()
        }
    }

    Timer { id: espera; interval: 200; onTriggered: tela.buscar() }

    Shortcut {
        sequence: "F2"
        enabled: tela.StackView.status === StackView.Active
        onActivated: tela.janela.abrir("TelaVender")
    }

    Keys.onDownPressed: lista.currentIndex = Math.min(lista.currentIndex + 1, lista.count - 1)
    Keys.onUpPressed: lista.currentIndex = Math.max(lista.currentIndex - 1, 0)

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        spacing: 14

        Cabecalho {
            titulo: "Orçamentos"
            subtitulo: "Monte o orçamento na tela Vender e escolha “Orçamento”" + (Tema.celular ? "" : " (F6)")
            Layout.fillWidth: true
            onVoltar: tela.janela.voltar()
            Botao {
                text: "Novo orçamento"
                icone: "mais"
                atalho: "F2"
                onClicked: tela.janela.abrir("TelaVender")
            }
        }

        Campo {
            id: busca
            icone: "busca"
            altura: 54
            tamanhoFonte: 18
            dica: "Cliente, número ou peça"
            Layout.fillWidth: true
            onEditado: espera.restart()
            onConfirmado: tela.abrir(lista.currentIndex)
        }

        ListView {
            id: lista
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 6
            model: tela.itens
            currentIndex: -1
            highlightMoveDuration: 0
            keyNavigationEnabled: false
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            delegate: LinhaVenda {
                required property var modelData
                required property int index
                width: lista.width
                item: modelData
                indice: index
                atual: lista.currentIndex === index
                onAbrir: tela.abrir(index)
            }
            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(560, parent.width - 40)
                visible: tela.buscou && tela.itens.length === 0
                spacing: 12
                Titulo {
                    text: busca.texto === "" ? "Nenhum orçamento ainda" : "Nada encontrado"
                    font.pixelSize: 32
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Nota {
                    text: "Na tela Vender, ponha as peças no carrinho e escolha “Orçamento”. Ele sai impresso em A4 e depois vira venda com um clique."
                    font.pixelSize: 16
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
            }
        }
    }

    StackView.onActivated: {
        buscar()
        busca.focarCursor()
    }
}
