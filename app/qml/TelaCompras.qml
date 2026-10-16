import QtCore
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts
import rustinho

// Compras: lista das notas de entrada e importação do XML.
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    property var itens: []
    property bool buscou: false

    function buscar() {
        const r = Api.chamar("compras_listar", { texto: busca.texto, limite: 300 })
        if (r.ok) {
            itens = r.dados
            lista.currentIndex = itens.length > 0 ? 0 : -1
        } else {
            janela.avisar(r.erro, "erro")
        }
        buscou = true
    }

    function abrirCompra(indice) {
        if (indice >= 0 && indice < itens.length)
            janela.abrir("TelaCompra", { compraId: itens[indice].id })
    }

    // Cada arquivo pode ser um XML, um .zip/.rar com várias notas ou uma pasta.
    function importar(arquivos) {
        const novas = []
        let deOutra = 0
        const erros = []
        for (const arquivo of arquivos) {
            const r = Api.chamar("compra_importar", { caminho: arquivo.toString() })
            if (r.ok) {
                for (const c of r.dados.compras) {
                    novas.push(c)
                    if (c.avisoDestinatario.indexOf("não é") >= 0)
                        deOutra++
                }
                for (const e of r.dados.erros)
                    erros.push(e)
            } else {
                erros.push(nomeArquivo(arquivo) + ": " + r.erro)
            }
        }
        buscar()
        if (erros.length > 0)
            janela.avisar(erros.slice(0, 6).join("\n") + (erros.length > 6 ? "\n… e mais " + (erros.length - 6) : ""), "erro")
        if (novas.length === 1)
            janela.abrir("TelaCompra", { compraId: novas[0].id })
        else if (novas.length > 1)
            janela.avisar(novas.length + " notas importadas. Abra cada uma para conferir e lançar."
                          + (deOutra > 0 ? " Atenção: " + deOutra + (deOutra === 1 ? " nota não é" : " notas não são") + " da loja (outro CNPJ)." : ""),
                          deOutra > 0 ? "info" : "ok")
    }

    function nomeArquivo(url) {
        const t = decodeURIComponent(url.toString())
        return t.substring(t.lastIndexOf("/") + 1)
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
        interval: 200
        onTriggered: tela.buscar()
    }

    Shortcut {
        sequence: "F2"
        enabled: tela.StackView.status === StackView.Active
        onActivated: seletor.open()
    }

    Keys.onDownPressed: lista.currentIndex = Math.min(lista.currentIndex + 1, lista.count - 1)
    Keys.onUpPressed: lista.currentIndex = Math.max(lista.currentIndex - 1, 0)

    FileDialog {
        id: seletor
        title: "Escolha o XML da nota de compra (ou um .zip / .rar)"
        fileMode: FileDialog.OpenFiles
        // Começa na pasta Downloads, onde costumam ficar os XML baixados do e-mail.
        currentFolder: StandardPaths.standardLocations(StandardPaths.DownloadLocation)[0]
        nameFilters: ["XML, ZIP ou RAR (*.xml *.XML *.zip *.ZIP *.rar *.RAR)", "Todos os arquivos (*)"]
        onAccepted: tela.importar(selectedFiles)
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        spacing: 14

        Cabecalho {
            sensivel: true
            titulo: "Compras"
            subtitulo: "Notas de entrada: importe o XML que o fornecedor manda por e-mail"
            Layout.fillWidth: true
            onVoltar: tela.janela.voltar()
            Botao {
                text: "Importar XML"
                icone: "mais"
                atalho: "F2"
                onClicked: seletor.open()
            }
        }

        Campo {
            id: busca
            icone: "busca"
            altura: 54
            tamanhoFonte: 18
            dica: "Fornecedor, número da nota ou peça"
            Layout.fillWidth: true
            onEditado: espera.restart()
            onConfirmado: tela.abrirCompra(lista.currentIndex)
        }

        RowLayout {
            visible: tela.itens.length > 0
            spacing: 18
            Layout.fillWidth: true
            Layout.leftMargin: 18
            Layout.rightMargin: 20
            Layout.topMargin: 4
            Rotulo { text: "Emissão"; Layout.preferredWidth: 96 }
            Rotulo { text: "Fornecedor"; Layout.fillWidth: true }
            Rotulo { text: "Situação"; horizontalAlignment: Text.AlignRight; Layout.preferredWidth: 170 }
            Rotulo { text: "Total da nota"; horizontalAlignment: Text.AlignRight; Layout.preferredWidth: 140 }
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
            delegate: LinhaCompra {
                required property var modelData
                required property int index
                width: lista.width
                item: modelData
                indice: index
                atual: lista.currentIndex === index
                onAbrir: tela.abrirCompra(index)
            }

            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(600, parent.width - 40)
                visible: tela.buscou && tela.itens.length === 0
                spacing: 14
                Titulo {
                    text: busca.texto === "" ? "Nenhuma compra ainda" : "Nada encontrado"
                    font.pixelSize: 32
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Nota {
                    text: busca.texto === ""
                          ? "Importe o XML da nota do fornecedor. O Rustinho cadastra o fornecedor, liga cada item à peça certa, divide o frete e põe tudo no estoque com o custo novo."
                          : "Nenhuma compra combina com “" + busca.texto + "”."
                    font.pixelSize: 16
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Botao {
                    visible: busca.texto === ""
                    text: "Importar o primeiro XML"
                    icone: "mais"
                    Layout.alignment: Qt.AlignHCenter
                    onClicked: seletor.open()
                }
            }
        }
    }

    // Arrastar arquivos XML para a janela também importa.
    DropArea {
        anchors.fill: parent
        keys: ["text/uri-list"]
        onDropped: (evento) => {
            if (evento.hasUrls)
                tela.importar(evento.urls)
        }
    }

    StackView.onActivated: {
        buscar()
        busca.entrada.forceActiveFocus()
    }
}
