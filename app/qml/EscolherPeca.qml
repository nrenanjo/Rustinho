import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Templates as T
import QtQuick.Layouts
import rustinho

// Janela para escolher uma peça (usada na compra).
Popup {
    id: janela
    property string referencia: ""
    property var itens: []

    signal escolhida(string produtoId)

    modal: true
    focus: true
    anchors.centerIn: T.Overlay.overlay
    width: Math.min(1000, (T.Overlay.overlay ? T.Overlay.overlay.width : 1200) - 80)
    height: Math.min(640, (T.Overlay.overlay ? T.Overlay.overlay.height : 800) - 80)
    padding: 28
    closePolicy: Popup.CloseOnEscape

    T.Overlay.modal: Rectangle { color: Qt.rgba(0.16, 0.15, 0.13, 0.35) }
    background: Rectangle { color: Tema.fundo; radius: Tema.raio + 3 }

    function abrirCom(referencia, textoInicial) {
        janela.referencia = referencia
        busca.texto = textoInicial || ""
        janela.open()
        buscar()
    }

    function buscar() {
        const r = Api.chamar("produtos_buscar", { texto: busca.texto, inativas: false, limite: 100 })
        if (r.ok) {
            itens = r.dados.itens
            lista.currentIndex = itens.length > 0 ? 0 : -1
        }
    }

    function escolher(indice) {
        if (indice >= 0 && indice < itens.length) {
            janela.escolhida(itens[indice].id)
            janela.close()
        }
    }

    Timer {
        id: espera
        interval: 160
        onTriggered: janela.buscar()
    }

    contentItem: ColumnLayout {
        spacing: 12
        Keys.onDownPressed: lista.currentIndex = Math.min(lista.currentIndex + 1, lista.count - 1)
        Keys.onUpPressed: lista.currentIndex = Math.max(lista.currentIndex - 1, 0)

        Titulo {
            text: "Qual é a peça?"
            font.pixelSize: 34
            Layout.fillWidth: true
        }
        Nota {
            text: "Na nota: " + janela.referencia
            font.pixelSize: 15
            elide: Text.ElideRight
            wrapMode: Text.NoWrap
            Layout.fillWidth: true
        }
        Campo {
            id: busca
            icone: "busca"
            altura: 50
            tamanhoFonte: 17
            dica: "Qualquer código, nome, modelo, marca ou carro"
            Layout.fillWidth: true
            onEditado: espera.restart()
            onConfirmado: janela.escolher(lista.currentIndex)
        }
        ListView {
            id: lista
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 6
            model: janela.itens
            currentIndex: -1
            highlightMoveDuration: 0
            keyNavigationEnabled: false
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            onCurrentIndexChanged: if (currentIndex >= 0) positionViewAtIndex(currentIndex, ListView.Contain)
            delegate: LinhaPeca {
                required property var modelData
                required property int index
                width: lista.width
                item: modelData
                indice: index
                atual: lista.currentIndex === index
                onAbrir: janela.escolher(index)
            }
            Nota {
                anchors.centerIn: parent
                visible: janela.itens.length === 0
                text: "Nenhuma peça encontrada. Feche e escolha “Cadastrar nova”."
                font.pixelSize: 16
            }
        }
        Linha {
            Layout.fillWidth: true
            Nota {
                text: "↑ ↓ para escolher, Enter para confirmar"
                font.pixelSize: 14
                Layout.fillWidth: true
            }
            Botao {
                text: "Cancelar"
                tipo: "discreto"
                atalho: "Esc"
                onClicked: janela.close()
            }
            Botao {
                text: "Usar esta peça"
                icone: "check"
                enabled: lista.currentIndex >= 0
                onClicked: janela.escolher(lista.currentIndex)
            }
        }
    }

    onOpened: busca.focar()
}
