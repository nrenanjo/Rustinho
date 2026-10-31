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
    readonly property string nomeNaNavegacao: "Compras"
    property var itens: []
    property bool buscou: false
    // 0.9.17: arquivos que esperam a importação (faltava o CNPJ da loja). Ficam
    // aqui enquanto a pessoa resolve o requisito; nada é importado sozinho.
    property var pendentes: []
    property string avisoPendentes: ""
    // Notas de outro CNPJ: não entraram (nada foi gravado). Ficam à vista até
    // a pessoa descartar ou sair de Compras.
    property var naoImportadas: []

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
    // Um de cada vez, à parte, com a tela de espera (0.9.15).
    function importar(arquivos) {
        // Caminhos (os arquivos que esperavam o requisito já vêm assim).
        const lista = Array.from(arquivos).map(a => /^(file|content):/.test(a.toString()) ? Api.caminho(a.toString()) : a.toString())
        const novas = []
        const erros = []
        const conflitos = []
        let deOutra = 0
        pendentes = []
        avisoPendentes = ""
        function proximo(i) {
            if (i >= lista.length) {
                terminar()
                return
            }
            const arquivo = lista[i]
            // FALHA-04: uma chave para a importação deste arquivo.
            const chaveArquivo = Api.novaChave()
            Api.gravarDemorado("compra_importar", { caminho: arquivo },
                               lista.length > 1 ? "Importando " + (i + 1) + " de " + lista.length + "…" : "Importando o XML…", r => {
                if (r.ok) {
                    for (const c of r.dados.compras) {
                        novas.push(c)
                        if (c.avisoDestinatario.indexOf("não é") >= 0)
                            deOutra++
                    }
                    for (const e of r.dados.erros)
                        erros.push(e)
                    for (const c of r.dados.conflitos)
                        conflitos.push(c)
                } else if (tela.janela.guiar(r.campo, r.erro, ok => tela.voltouDaGuia(ok))) {
                    // Falta o CNPJ da loja: a navegação guiada leva a
                    // Configurações › Loja; este arquivo e os que faltam ficam
                    // esperando aqui.
                    tela.pendentes = lista.slice(i)
                    if (novas.length > 0 || erros.length > 0 || conflitos.length > 0)
                        terminar()
                    return
                } else {
                    erros.push(nomeArquivo(arquivo) + ": " + r.erro)
                }
                proximo(i + 1)
            }, chaveArquivo)
        }
        function terminar() {
            buscar()
            tela.guardarNaoImportadas(conflitos)
            if (erros.length > 0)
                janela.avisar(erros.slice(0, 6).join("\n") + (erros.length > 6 ? "\n… e mais " + (erros.length - 6) : ""), "erro")
            if (novas.length === 1)
                janela.abrir("TelaCompra", { compraId: novas[0].id })
            else if (novas.length > 1)
                janela.avisar(novas.length + " notas importadas. Abra cada uma para conferir e lançar. O Rustinho ainda não verifica a assinatura digital do XML: use só XML que veio do fornecedor ou da SEFAZ."
                              + (deOutra > 0 ? " Atenção: " + deOutra + (deOutra === 1 ? " nota não é" : " notas não são") + " da loja (outro CNPJ)." : ""),
                              deOutra > 0 ? "info" : "ok")
        }
        proximo(0)
    }

    // Volta da navegação guiada: os arquivos continuam esperando. Com o
    // requisito satisfeito, confere se eles continuam no lugar; a pessoa
    // manda importar (nada é importado sozinho).
    function voltouDaGuia(satisfeito) {
        if (!satisfeito || pendentes.length === 0)
            return
        const c = Api.chamar("arquivos_conferir", { arquivos: pendentes })
        if (c.ok && c.dados.faltam.length > 0) {
            pendentes = c.dados.existem
            avisoPendentes = "Não estão mais no lugar e ficaram de fora: "
                    + c.dados.faltam.map(a => nomeArquivo(a)).join(", ") + "."
        }
    }

    // Notas de outro CNPJ desta importação: entram na lista (sem repetir a
    // mesma) e a janela de conflito abre na primeira nova.
    // A mesma nota (o Qt copia os objetos entre a lista e a janela: compara
    // pelo que identifica a nota, não pelo objeto).
    function chaveDaNota(c) {
        return c.arquivo + "|" + c.numero + "|" + c.serie + "|" + c.emitenteDocumento
    }
    function guardarNaoImportadas(conflitos) {
        const ja = naoImportadas.map(chaveDaNota)
        const novas = conflitos.filter(c => ja.indexOf(chaveDaNota(c)) < 0)
        if (novas.length === 0)
            return
        naoImportadas = naoImportadas.concat(novas)
        conflitoCnpj.mostrar(novas[0])
    }
    function descartarNaoImportada(c) {
        naoImportadas = naoImportadas.filter(x => chaveDaNota(x) !== chaveDaNota(c))
    }

    function nomeArquivo(url) {
        const t = decodeURIComponent(url.toString())
        return t.substring(Math.max(t.lastIndexOf("/"), t.lastIndexOf("\\")) + 1)
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

    ConflitoCnpj {
        id: conflitoCnpj
        onDescartar: (c) => tela.descartarNaoImportada(c)
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        spacing: 14

        Cabecalho {
            sensivel: true
            titulo: "Compras"
            subtitulo: "Importe o XML da nota do fornecedor para atualizar o estoque"
            Layout.fillWidth: true
            onVoltar: tela.janela.voltar()
            Botao {
                text: "Importar XML"
                icone: "mais"
                atalho: "F2"
                onClicked: seletor.open()
            }
        }

        // Arquivos esperando o CNPJ da loja (navegação guiada).
        Faixa {
            visible: tela.pendentes.length > 0
            titulo: "Esperando para importar"
            corTitulo: Tema.apagado
            color: Tema.atencao
            Layout.fillWidth: true
            Nota {
                text: tela.pendentes.map(a => tela.nomeArquivo(a)).join(" · ")
                Layout.fillWidth: true
            }
            Nota {
                visible: tela.avisoPendentes !== ""
                text: tela.avisoPendentes
                color: Tema.vermelho
                Layout.fillWidth: true
            }
            Flow {
                spacing: 12
                Layout.fillWidth: true
                Botao {
                    text: "Importar agora"
                    icone: "mais"
                    onClicked: tela.importar(tela.pendentes)
                }
                Botao {
                    text: "Desistir"
                    tipo: "secundario"
                    onClicked: {
                        tela.pendentes = []
                        tela.avisoPendentes = ""
                    }
                }
            }
        }

        // Notas de outro CNPJ: não entraram; dá para ver de novo ou descartar.
        Faixa {
            visible: tela.naoImportadas.length > 0
            titulo: tela.naoImportadas.length === 1 ? "Não importada: nota de outro CNPJ"
                                                     : "Não importadas: " + tela.naoImportadas.length + " notas de outro CNPJ"
            corTitulo: Tema.vermelho
            color: Tema.rosa
            Layout.fillWidth: true
            Repeater {
                model: tela.naoImportadas
                delegate: RowLayout {
                    required property var modelData
                    spacing: 12
                    Layout.fillWidth: true
                    Nota {
                        text: modelData.arquivo + " — nota " + modelData.numero + " de " + modelData.emitenteNome
                        elide: Text.ElideMiddle
                        wrapMode: Text.NoWrap
                        Layout.fillWidth: true
                    }
                    Botao {
                        text: "Ver"
                        tipo: "secundario"
                        onClicked: conflitoCnpj.mostrar(modelData)
                    }
                    Botao {
                        text: "Descartar"
                        tipo: "discreto"
                        onClicked: tela.descartarNaoImportada(modelData)
                    }
                }
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
            visible: tela.itens.length > 0 && !Tema.estreito
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

            // Sem compras: o meio fica limpo (a orientação está embaixo do
            // título e o único botão de importar fica em cima). Numa busca
            // sem resultado, só o aviso curto.
            Nota {
                anchors.centerIn: parent
                width: Math.min(600, parent.width - 40)
                visible: tela.buscou && tela.itens.length === 0 && busca.texto !== ""
                text: "Nenhuma compra combina com “" + busca.texto + "”."
                font.pixelSize: 16
                horizontalAlignment: Text.AlignHCenter
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
        busca.focarCursor()
    }
}
