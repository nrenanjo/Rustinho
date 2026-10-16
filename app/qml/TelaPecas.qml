import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Peças: busca única (código, nome, marca, carro...) e lista de resultados.
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    property string buscaInicial: ""
    property var itens: []
    property int total: 0
    property bool buscou: false

    function buscar(manterSelecao) {
        const anterior = manterSelecao && lista.currentIndex >= 0 && lista.currentIndex < itens.length
                         ? itens[lista.currentIndex].id : ""
        const r = Api.chamar("produtos_buscar", { texto: busca.texto, inativas: inativas.checked, limite: 200 })
        if (r.ok) {
            itens = r.dados.itens
            total = r.dados.total
            let indice = itens.length > 0 ? 0 : -1
            if (anterior !== "") {
                const achou = itens.findIndex(p => p.id === anterior)
                if (achou >= 0)
                    indice = achou
            }
            lista.currentIndex = indice
            if (indice <= 0)
                lista.positionViewAtBeginning()
        } else {
            janela.avisar(r.erro, "erro")
        }
        buscou = true
    }

    function abrirPeca(indice) {
        if (indice >= 0 && indice < itens.length)
            janela.abrir("TelaPeca", { produtoId: itens[indice].id })
    }

    function novaPeca() {
        janela.abrir("TelaPeca", { produtoId: "" })
    }

    function focarBusca() {
        busca.focar()
    }

    function aoEsc() {
        if (busca.texto !== "") {
            busca.texto = ""
            buscar()
        } else {
            janela.voltar()
        }
    }

    Timer {
        id: espera
        interval: 160
        onTriggered: tela.buscar()
    }

    Shortcut {
        sequence: "F2"
        enabled: tela.StackView.status === StackView.Active
        onActivated: tela.novaPeca()
    }

    // Setas e Page Up/Down mexem na lista enquanto se digita na busca.
    Keys.onDownPressed: lista.currentIndex = Math.min(lista.currentIndex + 1, lista.count - 1)
    Keys.onUpPressed: lista.currentIndex = Math.max(lista.currentIndex - 1, 0)
    Keys.onPressed: (evento) => {
        if (evento.key === Qt.Key_PageDown) {
            lista.currentIndex = Math.min(lista.currentIndex + 8, lista.count - 1)
            evento.accepted = true
        } else if (evento.key === Qt.Key_PageUp) {
            lista.currentIndex = Math.max(lista.currentIndex - 8, 0)
            evento.accepted = true
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        spacing: 14

        Cabecalho {
            titulo: "Peças"
            subtitulo: tela.total === 0 ? "Nenhuma peça cadastrada" : tela.total === 1 ? "1 peça cadastrada" : tela.total + " peças cadastradas"
            Layout.fillWidth: true
            onVoltar: tela.janela.voltar()
            Botao {
                text: "Etiquetas"
                icone: "impressora"
                tipo: "discreto"
                onClicked: tela.janela.abrir("TelaEtiquetas", {})
            }
            Botao {
                text: "Importar XML"
                tipo: "secundario"
                onClicked: importador.abrirCom()
            }
            Botao {
                text: "Nova peça"
                icone: "mais"
                atalho: "F2"
                onClicked: tela.novaPeca()
            }
        }

        Linha {
            spacing: 18
            Layout.fillWidth: true
            Campo {
                id: busca
                icone: "busca"
                atalho: "F11"
                altura: 54
                tamanhoFonte: 18
                dica: Tema.estreito ? "Código, nome ou carro" : "Qualquer código, nome, modelo, marca ou carro (ex.: 8PK1515, pastilha gol 2015)"
                Layout.fillWidth: true
                onEditado: espera.restart()
                onConfirmado: tela.abrirPeca(lista.currentIndex)
            }
            Marcar {
                id: inativas
                text: "Mostrar inativas"
                onToggled: tela.buscar()
            }
        }

        RowLayout {
            visible: tela.itens.length > 0 && !Tema.estreito
            spacing: 18
            Layout.fillWidth: true
            Layout.leftMargin: 18
            Layout.rightMargin: 20
            Layout.topMargin: 4
            Rotulo { text: "Código"; Layout.preferredWidth: 84 }
            Rotulo { text: busca.texto.trim() === "" ? "Peça · alteradas por último" : "Peça"; Layout.fillWidth: true }
            Rotulo { text: "Local"; Layout.preferredWidth: 90 }
            Rotulo { text: "Estoque"; horizontalAlignment: Text.AlignRight; Layout.preferredWidth: 104 }
            Rotulo { text: "Revenda"; horizontalAlignment: Text.AlignRight; Layout.preferredWidth: 128 }
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
            highlightFollowsCurrentItem: true
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
                onAbrir: tela.abrirPeca(index)
            }

            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(560, parent.width - 40)
                visible: tela.buscou && tela.itens.length === 0
                spacing: 14
                Titulo {
                    text: tela.total === 0 ? "Nenhuma peça cadastrada ainda" : "Nada encontrado"
                    font.pixelSize: 32
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Nota {
                    text: tela.total === 0
                          ? "Cadastre as peças com os carros em que servem: assim o balcão acha tudo por um campo só."
                          : "Nenhuma peça combina com “" + busca.texto + "”. Tente só uma parte do código ou do nome."
                    font.pixelSize: 16
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Botao {
                    visible: tela.total === 0
                    text: "Cadastrar a primeira peça"
                    icone: "mais"
                    Layout.alignment: Qt.AlignHCenter
                    onClicked: tela.novaPeca()
                }
            }
        }
    }

    StackView.onActivated: {
        buscar(true)
        busca.entrada.forceActiveFocus()
    }
    Component.onCompleted: {
        busca.texto = buscaInicial
    }

    ImportarCadastros {
        id: importador
        onImportou: tela.buscar()
    }
}
