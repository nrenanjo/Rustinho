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
    property string filtro: ""
    property int aguardandoAutorizacao: 0
    property int negadasAutorizacao: 0
    // 0.9.17: arquivos que esperam a importação (faltava o CNPJ da loja). Ficam
    // aqui enquanto a pessoa resolve o requisito; nada é importado sozinho.
    property var pendentes: []
    property string avisoPendentes: ""
    // Notas de outro CNPJ: não entraram (nada foi gravado). Ficam à vista até
    // a pessoa descartar ou sair de Compras.
    property var naoImportadas: []

    function atualizarContagem() {
        const c = Api.chamar("compras_contagem", {})
        if (c.ok) {
            aguardandoAutorizacao = c.dados.aguardandoAutorizacao || 0
            negadasAutorizacao = c.dados.negadas || 0
        }
    }

    function buscar() {
        const r = Api.chamar("compras_listar", { texto: busca.texto, filtro: tela.filtro, limite: 5000 })
        if (r.ok) {
            itens = r.dados
            lista.currentIndex = itens.length > 0 ? 0 : -1
        } else {
            janela.avisar(r.erro, "erro")
        }
        atualizarContagem()
        buscou = true
    }

    function abrirCompra(indice) {
        if (indice >= 0 && indice < itens.length)
            janela.abrir("TelaCompra", { compraId: itens[indice].id })
    }

    function conferirCompra(indice) {
        if (indice >= 0 && indice < itens.length)
            janela.abrir("TelaCompra", { compraId: itens[indice].id, abrirConferenciaAoAbrir: true })
    }

    // Cada arquivo pode ser um XML, um .zip/.rar com várias notas ou uma pasta.
    // T111 (Achado 2): manda o lote inteiro numa chamada só para o núcleo
    // analisar ambiguidades entre todas as notas e tratar seleção múltipla como pacote.
    function importar(arquivos) {
        // Caminhos (os arquivos que esperavam o requisito já vêm assim).
        const lista = Array.from(arquivos).map(a => /^(file|content):/.test(a.toString()) ? Api.caminho(a.toString()) : a.toString())
        if (lista.length === 0)
            return
        const novas = []
        const erros = []
        const conflitos = []
        // T108 / T111: notas já em Compras, peças novas cadastradas e itens duvidosos para conferência.
        let jaEmCompras = 0
        let pecasCadastradas = 0
        let pendenciasIdentificacao = 0
        pendentes = []
        avisoPendentes = ""
        function terminar() {
            buscar()
            tela.guardarNaoImportadas(conflitos)
            // N1 da 2ª conferência da T108: UMA mensagem só (o aviso mostra uma de cada vez e a de depois troca a de
            // antes). Com erro, ela é de erro e leva também as notas importadas e as que já estavam em Compras.
            const partes = []
            if (novas.length > 1)
                partes.push(erros.length > 0
                            ? novas.length + " notas importadas."
                            : novas.length + " notas importadas. Abra cada uma para conferir e lançar.")
            if (pecasCadastradas > 0)
                partes.push(pecasCadastradas === 1 ? "1 peça nova cadastrada automaticamente."
                                                   : pecasCadastradas + " peças novas cadastradas automaticamente.")
            if (pendenciasIdentificacao > 0)
                partes.push(pendenciasIdentificacao === 1
                            ? "1 item com identificação duvidosa enviado para conferência em Peças."
                            : pendenciasIdentificacao + " itens com identificação duvidosa enviados para conferência em Peças.")
            if (jaEmCompras > 0)
                partes.push(jaEmCompras === 1 ? "1 nota já estava em Compras e não entrou de novo."
                                              : jaEmCompras + " notas já estavam em Compras e não entraram de novo.")
            if (erros.length > 0)
                janela.avisar((partes.length > 0 ? partes.join(" ") + "\n" : "")
                              + erros.slice(0, 6).join("\n") + (erros.length > 6 ? "\n… e mais " + (erros.length - 6) : ""), "erro")
            else if (partes.length > 0)
                janela.avisar(partes.join(" "), "ok")
            if (novas.length === 1)
                janela.abrir("TelaCompra", { compraId: novas[0].id })
        }
        // FALHA-04: uma chave para a importação deste lote.
        const chaveLote = Api.novaChave()
        Api.gravarDemorado("compra_importar", { caminhos: lista },
                           lista.length > 1 ? "Importando " + lista.length + " arquivos…" : "Importando o XML…", r => {
            if (r.ok) {
                // Nota de outro CNPJ não entra: vem em "conflitos" (caso 2b).
                for (const c of r.dados.compras)
                    novas.push(c)
                for (const e of r.dados.erros)
                    erros.push(e)
                for (const c of r.dados.conflitos)
                    conflitos.push(c)
                jaEmCompras += r.dados.jaEmCompras || 0
                pecasCadastradas += r.dados.pecasCadastradas || 0
                pendenciasIdentificacao += r.dados.pendenciasIdentificacao || 0
                terminar()
            } else if (tela.janela.guiar(r.campo, r.erro, ok => tela.voltouDaGuia(ok))) {
                // Falta o CNPJ da loja: a navegação guiada leva a
                // Configurações › Loja; os arquivos ficam esperando aqui.
                tela.pendentes = lista
            } else {
                erros.push(lista.length === 1 ? nomeArquivo(lista[0]) + ": " + r.erro : r.erro)
                terminar()
            }
        }, chaveLote)
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

    // D-IMG-1 (21/11/2026): a mesma nota em dois arquivos diferentes conta e aparece
    // uma única vez, agrupando os nomes dos arquivos em `arquivos`.
    function chaveDaNota(c) {
        if (c.chave && String(c.chave).trim() !== "")
            return "chave|" + String(c.chave).trim()
        return (c.numero || "") + "|" + (c.serie || "") + "|" + (c.emitenteDocumento || "") + "|" + (c.destinatarioDocumento || "")
    }
    function arquivosDaNota(c) {
        if (c.arquivos && c.arquivos.length > 0)
            return c.arquivos
        return c.arquivo ? [c.arquivo] : []
    }
    function textoArquivosDaNota(c) {
        const lista = arquivosDaNota(c)
        if (lista.length <= 1)
            return lista[0] || ""
        return lista[0] + "  (+" + (lista.length - 1) + (lista.length === 2 ? " arquivo)" : " arquivos)")
    }
    function guardarNaoImportadas(conflitos) {
        const mapa = {}
        const atual = naoImportadas.map(x => {
            const copia = Object.assign({}, x, { arquivos: arquivosDaNota(x).slice() })
            mapa[chaveDaNota(copia)] = copia
            return copia
        })
        let primeiraNova = null
        for (const c of conflitos) {
            const k = chaveDaNota(c)
            if (mapa[k]) {
                for (const a of arquivosDaNota(c)) {
                    if (mapa[k].arquivos.indexOf(a) < 0)
                        mapa[k].arquivos.push(a)
                }
                mapa[k].arquivo = mapa[k].arquivos.join(", ")
            } else {
                const nova = Object.assign({}, c, { arquivos: arquivosDaNota(c).slice() })
                mapa[k] = nova
                atual.push(nova)
                if (!primeiraNova)
                    primeiraNova = nova
            }
        }
        naoImportadas = atual
        if (primeiraNova)
            conflitoCnpj.mostrar(primeiraNova)
    }
    // T054-C2-R7: protege contra clique duplo rápido em "Descartar" (evita descartar a nota que subiu).
    Timer {
        id: travaDescartar
        interval: 400
    }
    function descartarNaoImportada(c, daJanelinha) {
        if (!c || travaDescartar.running)
            return
        travaDescartar.restart()
        // T054: a lista não volta para o começo depois de descartar uma nota.
        const onde = listaNaoImportadas.contentY
        naoImportadas = naoImportadas.filter(x => chaveDaNota(x) !== chaveDaNota(c))
        Qt.callLater(() => {
            // T054-R3 da revisão: refaz a lista antes de ler a altura dela (preservação silenciosa de posição).
            listaNaoImportadas.forceLayout()
            listaNaoImportadas.contentY = Math.max(0, Math.min(onde, Math.max(0, listaNaoImportadas.contentHeight - listaNaoImportadas.height)))
        })
    }

    function nomeArquivo(url) {
        const t = decodeURIComponent(url.toString())
        return t.substring(Math.max(t.lastIndexOf("/"), t.lastIndexOf("\\")) + 1)
    }

    function focarBusca() {
        busca.focar()
    }

    // O Esc e a seta fazem o mesmo (D-V3, decidida pelo dono em 07/11/2026:
    // com a busca preenchida, os dois voltam direto). limparBuscaAntes só
    // limparia a busca antes se a regra mudasse em Main.
    function aoEsc(daSeta) {
        if (busca.texto !== "" && janela.limparBuscaAntes(daSeta)) {
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
        // D-IMG-6: informa explicitamente que dá para selecionar vários arquivos.
        title: "Escolha os XMLs das notas de compra (ou arquivos .zip / .rar)"
        fileMode: FileDialog.OpenFiles
        // Começa na pasta Downloads, onde costumam ficar os XML baixados do e-mail.
        currentFolder: StandardPaths.standardLocations(StandardPaths.DownloadLocation)[0]
        nameFilters: ["XML, ZIP ou RAR (*.xml *.XML *.zip *.ZIP *.rar *.RAR)", "Todos os arquivos (*)"]
        onAccepted: tela.importar(selectedFiles)
    }

    // T107 (texto Q do proprietário, 21/11/2026): uma pasta inteira, com as subpastas, como em Peças › Trazer
    // cadastros pelo XML. A pasta vai à mesma importação dos arquivos (o núcleo lê os XMLs, .zip e .rar de dentro).
    FolderDialog {
        id: seletorPasta
        title: "Escolha a pasta com os XMLs das notas de compra"
        currentFolder: StandardPaths.standardLocations(StandardPaths.DownloadLocation)[0]
        onAccepted: tela.importar([selectedFolder])
    }

    ConflitoCnpj {
        id: conflitoCnpj
        onDescartar: (c) => tela.descartarNaoImportada(c, true)
        // T054 (pedido do proprietário, 20/11/2026): com a janelinha aberta, o
        // "Mostrar valores" do canto da tela continua à vista e clicável, no
        // mesmo lugar (é o mesmo controle, que passa para cima do véu).
        onOpened: cabecalhoDeCompras.levarOlho(conflitoCnpj)
        onClosed: cabecalhoDeCompras.devolverOlho()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        spacing: 14

        Cabecalho {
            id: cabecalhoDeCompras
            sensivel: true
            titulo: "Compras"
            // D-IMG-6: informa que é possível escolher vários arquivos ou arrastá-los para a tela.
            // Em janela muito estreita (< 440 px), versão compacta preserva a busca visível em 340 × 480.
            subtitulo: Tema.largura < 440
                       ? "Importe ou arraste XMLs de notas"
                       : "Importe os XMLs das notas dos fornecedores (escolha vários arquivos ou arraste para cá)"
            Layout.fillWidth: true
            Botao {
                // Na janela bem estreita, o rótulo curto deixa os dois botões numa linha só (a busca continua à vista
                // na janela mínima; t054, C8-340x480-1).
                text: Tema.largura < 440 ? "Pasta" : "Escolher uma pasta"
                visible: !Tema.celular
                tipo: "secundario"
                onClicked: seletorPasta.open()
            }
            Botao {
                text: "Importar XML"
                icone: "mais"
                atalho: "F2"
                onClicked: seletor.open()
            }
        }

        // Arquivos esperando o CNPJ da loja (navegação guiada).
        Faixa {
            // T042: por cima da linha da barra de pesquisa logo abaixo: o halo da barra passa atrás, não por cima.
            z: 1
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
            id: notasDeOutroCnpj
            // T042: por cima da linha da barra de pesquisa logo abaixo: o halo da barra passa atrás, não por cima.
            z: 1
            visible: tela.naoImportadas.length > 0
            titulo: tela.naoImportadas.length === 1 ? "Não importada: nota de outro CNPJ"
                                                     : "Não importadas: " + tela.naoImportadas.length + " notas de outro CNPJ"
            corTitulo: Tema.vermelho
            color: Tema.rosa
            Layout.fillWidth: true
            // D-IMG-5: orientação curta sob o título de "Não importadas".
            Nota {
                text: "Nada foi importado. Veja cada nota; descartar só tira da lista."
                font.pixelSize: Tema.baixo ? 12 : 14
                Layout.fillWidth: true
            }
            // T054 (pedido do proprietário, 20/11/2026): com centenas de notas, a
            // lista rola por conta própria numa altura limitada, e a busca e as
            // compras continuam à vista. Antes, um Repeater desenhava todas as
            // notas e empurrava o resto da tela para fora. Sem paginação nem
            // "carregar mais" (simplicidade funcional): a lista inteira rola.
            ListView {
                id: listaNaoImportadas
                // Os testes leem onde a lista fica (T054-R6 da revisão).
                objectName: "listaNaoImportadas"
                Layout.fillWidth: true
                // Até 40% da tela (em janela baixa como 340 × 480, mínimo 110 px para a busca continuar à vista).
                Layout.preferredHeight: Math.min(contentHeight, Math.max(Tema.baixo ? 110 : 150, tela.height * 0.4 - 90))
                clip: true
                spacing: notasDeOutroCnpj.espacamento
                boundsBehavior: Flickable.StopAtBounds
                model: tela.naoImportadas
                ScrollBar.vertical: ScrollBar { id: barraNaoImportadas }
                delegate: Rectangle {
                    id: faixaDaLista
                    required property var modelData
                    // A barra de rolagem fica ao lado, sem cobrir o Descartar.
                    width: listaNaoImportadas.width
                           - (listaNaoImportadas.contentHeight > listaNaoImportadas.height ? barraNaoImportadas.width + 4 : 0)
                    required property int index
                    // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                    readonly property bool par: (index + 1) % 2 === 0
                    color: Tema.faixa(faixaDaLista.par, Tema.vela(0.4), "transparent")
                    palette.window: Tema.superficie(color)
                    radius: Tema.raioCampo
                    implicitHeight: conteudoDaFaixa.implicitHeight + 10
                    implicitWidth: conteudoDaFaixa.implicitWidth + 16
                    RowLayout {
                        id: conteudoDaFaixa
                        x: 8
                        y: 5
                        width: faixaDaLista.width - 16
                        spacing: 12
                        // D-IMG-4: fornecedor e nota em destaque (como na lista de compras) e arquivo embaixo.
                        ColumnLayout {
                            spacing: 1
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            Text {
                                text: (modelData.emitenteNome || "Fornecedor") + " — nota " + (modelData.numero || "—")
                                textFormat: Text.PlainText
                                color: Tema.tintaDaFaixa(faixaDaLista.par)
                                font.family: Tema.texto
                                font.weight: Font.DemiBold
                                font.pixelSize: 15
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                            Nota {
                                text: tela.textoArquivosDaNota(modelData)
                                font.pixelSize: 12
                                elide: Text.ElideMiddle
                                wrapMode: Text.NoWrap
                                Layout.fillWidth: true
                            }
                        }
                        Botao {
                            text: "Ver"
                            tipo: "secundario"
                            onClicked: conflitoCnpj.mostrar(modelData)
                        }
                        Botao {
                            text: "Descartar"
                            tipo: "discreto"
                            onClicked: tela.descartarNaoImportada(modelData, false)
                        }
                    }
                }
            }
        }

        Pilulas {
            visible: tela.naoImportadas.length === 0
                     && (tela.aguardandoAutorizacao > 0 || tela.negadasAutorizacao > 0 || tela.filtro !== "")
            modelo: {
                const m = [
                    { id: "", nome: "Todas" },
                    { id: "aguardando_autorizacao", nome: "Aguardando conferência" + (tela.aguardandoAutorizacao > 0 ? " · " + tela.aguardandoAutorizacao : "") }
                ]
                if (tela.negadasAutorizacao > 0 || tela.filtro === "negada")
                    m.push({ id: "negada", nome: "Bloqueadas" + (tela.negadasAutorizacao > 0 ? " · " + tela.negadasAutorizacao : "") })
                m.push({ id: "rascunho", nome: "Rascunhos" })
                m.push({ id: "lancada", nome: "Lançadas" })
                return m
            }
            atual: tela.filtro
            onEscolhido: id => {
                if (tela.filtro !== id) {
                    tela.filtro = id
                    tela.buscar()
                }
            }
        }

        Faixa {
            id: faixaConferencia
            z: 1
            visible: tela.naoImportadas.length === 0 && tela.filtro === "aguardando_autorizacao"
            titulo: tela.aguardandoAutorizacao === 0
                    ? "Conferência da autorização"
                    : tela.aguardandoAutorizacao === 1
                      ? "1 nota aguardando conferência da autorização"
                      : tela.aguardandoAutorizacao + " notas aguardando conferência da autorização"
            corTitulo: Tema.apagado
            color: Tema.atencao
            Layout.fillWidth: true
            GridLayout {
                columns: Tema.estreito ? 1 : 2
                columnSpacing: 12
                rowSpacing: 8
                Layout.fillWidth: true
                Nota {
                    text: tela.aguardandoAutorizacao > 0
                          ? "Clique em Conferir na nota para copiar a chave, consultar no portal da NF-e e registrar o protocolo antes de lançar."
                          : "Todas as notas de compra já tiveram a autorização conferida."
                    Layout.fillWidth: true
                }
                Botao {
                    text: "Mostrar todas"
                    tipo: "secundario"
                    onClicked: {
                        tela.filtro = ""
                        tela.buscar()
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
            onConfirmado: {
                // T068 (AUD-L-2, o mesmo defeito do bug 22): o Enter dado dentro da espera (o leitor
                // de código de barras manda o código e o Enter em poucos ms) agia sobre a lista de
                // antes. Agora a busca do texto atual sai na hora, nada abre e a pessoa escolhe na
                // lista; o texto fica selecionado para a próxima leitura trocar, e não juntar.
                if (espera.running) {
                    espera.stop()
                    tela.buscar()
                    busca.focar()
                    return
                }
                tela.abrirCompra(lista.currentIndex)
            }
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
            Rotulo { text: "Situação"; horizontalAlignment: Text.AlignRight; Layout.preferredWidth: 190 }
            Rotulo { text: "Total da nota"; horizontalAlignment: Text.AlignRight; Layout.preferredWidth: 130 }
            Item { visible: tela.aguardandoAutorizacao > 0; Layout.preferredWidth: 108 }
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
            onCurrentIndexChanged: if (currentIndex >= 0) janela.rolarAteIndice(lista, currentIndex, ListView.Contain)
            delegate: LinhaCompra {
                required property var modelData
                required property int index
                width: lista.width
                item: modelData
                indice: index
                atual: lista.currentIndex === index
                temAcaoConferir: tela.aguardandoAutorizacao > 0
                onAbrir: tela.abrirCompra(index)
                onConferir: tela.conferirCompra(index)
            }

            // Sem compras: o meio fica limpo (a orientação está embaixo do
            // título e o único botão de importar fica em cima). Numa busca ou
            // filtro sem resultado, só o aviso curto (em aguardando_autorizacao
            // sem busca, a faixa acima já informa que não resta nenhuma nota).
            Nota {
                anchors.centerIn: parent
                width: Math.min(600, parent.width - 40)
                visible: tela.buscou && tela.itens.length === 0
                         && (busca.texto !== "" || (tela.filtro !== "" && tela.filtro !== "aguardando_autorizacao"))
                text: busca.texto !== ""
                      ? "Nenhuma compra combina com “" + busca.texto + "”."
                      : tela.filtro === "negada"
                        ? "Nenhuma nota bloqueada."
                        : tela.filtro === "rascunho"
                          ? "Nenhuma compra em rascunho."
                          : "Nenhuma compra lançada."
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
        if (janela && janela.retornandoDeF11)
            return
        buscar()
        busca.focarCursor()
    }
}
