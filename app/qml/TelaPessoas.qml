import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Clientes ou fornecedores: busca única e lista.
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    // "cliente" ou "fornecedor"
    property string papel: "cliente"
    property var itens: []
    property int total: 0
    property bool buscou: false
    readonly property bool fornecedores: papel === "fornecedor"

    function buscar(manterSelecao) {
        const anterior = manterSelecao && lista.currentIndex >= 0 && lista.currentIndex < itens.length
                         ? itens[lista.currentIndex].id : ""
        const r = Api.chamar("pessoas_buscar", { texto: busca.texto, papel: papel, inativos: inativos.checked, limite: 200 })
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

    function abrirPessoa(indice) {
        if (indice >= 0 && indice < itens.length)
            janela.abrir("TelaPessoa", { pessoaId: itens[indice].id, papel: papel })
    }

    function nova() {
        janela.abrir("TelaPessoa", { pessoaId: "", papel: papel })
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
        onActivated: tela.nova()
    }

    Keys.onDownPressed: lista.currentIndex = Math.min(lista.currentIndex + 1, lista.count - 1)
    Keys.onUpPressed: lista.currentIndex = Math.max(lista.currentIndex - 1, 0)

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        spacing: 14

        Cabecalho {
            sensivel: true
            titulo: tela.fornecedores ? "Fornecedores" : "Clientes"
            subtitulo: {
                const n = tela.total
                const nome = tela.fornecedores ? (n === 1 ? "fornecedor" : "fornecedores") : (n === 1 ? "cliente" : "clientes")
                return n === 0 ? "Nenhum " + (tela.fornecedores ? "fornecedor" : "cliente") + " cadastrado" : n + " " + nome
            }
            Layout.fillWidth: true
            onVoltar: tela.janela.voltar()
            Botao {
                text: "Importar XML"
                tipo: "secundario"
                onClicked: importador.abrirCom()
            }
            Botao {
                text: tela.fornecedores ? "Novo fornecedor" : "Novo cliente"
                icone: "mais"
                atalho: "F2"
                onClicked: tela.nova()
            }
        }

        Linha {
            spacing: 18
            Layout.fillWidth: true
            Campo {
                id: busca
                icone: "busca"
                altura: 54
                tamanhoFonte: 18
                dica: tela.fornecedores ? "Nome, fantasia, CNPJ, telefone ou cidade"
                                        : "Nome, CPF/CNPJ, telefone, cidade ou placa do carro"
                Layout.fillWidth: true
                onEditado: espera.restart()
                onConfirmado: tela.abrirPessoa(lista.currentIndex)
            }
            Marcar {
                id: inativos
                text: "Mostrar inativos"
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
            Rotulo { text: "Código"; Layout.preferredWidth: 64 }
            Rotulo { text: busca.texto.trim() === "" ? "Nome · alterados por último" : "Nome"; Layout.fillWidth: true }
            Rotulo { text: "CPF / CNPJ"; Layout.preferredWidth: 190 }
            Rotulo { text: "Telefone"; horizontalAlignment: Text.AlignRight; Layout.preferredWidth: 150 }
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
            onCurrentIndexChanged: if (currentIndex >= 0) positionViewAtIndex(currentIndex, ListView.Contain)
            delegate: LinhaPessoa {
                required property var modelData
                required property int index
                width: lista.width
                item: modelData
                indice: index
                atual: lista.currentIndex === index
                onAbrir: tela.abrirPessoa(index)
            }

            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(560, parent.width - 40)
                visible: tela.buscou && tela.itens.length === 0
                spacing: 14
                Titulo {
                    text: tela.total === 0 ? (tela.fornecedores ? "Nenhum fornecedor ainda" : "Nenhum cliente ainda") : "Nada encontrado"
                    font.pixelSize: 32
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Nota {
                    text: tela.total === 0
                          ? (tela.fornecedores ? "Os fornecedores também são cadastrados sozinhos quando você importa o XML de uma compra."
                                               : "Cadastre quem compra a prazo, pede nota com CPF/CNPJ ou traz o carro para a oficina.")
                          : "Ninguém combina com “" + busca.texto + "”."
                    font.pixelSize: 16
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Botao {
                    visible: tela.total === 0
                    text: tela.fornecedores ? "Cadastrar fornecedor" : "Cadastrar o primeiro cliente"
                    icone: "mais"
                    Layout.alignment: Qt.AlignHCenter
                    onClicked: tela.nova()
                }
            }
        }
    }

    StackView.onActivated: {
        buscar(true)
        busca.entrada.forceActiveFocus()
    }

    ImportarCadastros {
        id: importador
        onImportou: tela.buscar()
    }
}
