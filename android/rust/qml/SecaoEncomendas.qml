import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Encomendas de clientes: falta pedir → pedida → chegou (avisar no
// WhatsApp) → entregue. Cada linha mostra o próximo passo.
FocusScope {
    id: secao
    // H16 (7.2, técnica DP-26): a linha tocada fica escolhida e mostra as ações
    // que antes só apareciam com o mouse em cima (no toque não há "em cima").
    property string linhaEscolhida: ""
    readonly property var janela: ApplicationWindow.window
    property var painel: null
    property string quais: "abertas"
    property bool buscou: false
    // 10a (C8, dev3.3): a encomenda que a notificação pediu ("encomenda:<id>"):
    // a linha fica escolhida (com as ações à vista) e destacada. Só mostra.
    property string alvo: ""

    // A-1 da revisão do VIS2 (16/11/2026): no celular a busca fica dentro da lista (topoNaLista), e a lista, quando
    // troca o modelo, dá o foco à 1ª linha: a busca perdia o foco a cada recarga (200 ms depois da tecla), e só 1 ou 2
    // letras entravam. Se a busca tinha o foco, ele e o cursor voltam depois da troca (sem selecionar o texto).
    function carregar() {
        const comFoco = busca.entrada.activeFocus
        const cursor = busca.entrada.cursorPosition
        carregarDados()
        if (comFoco)
            Qt.callLater(() => {
                if (busca.entrada.activeFocus)
                    return
                busca.entrada.forceActiveFocus()
                busca.entrada.cursorPosition = Math.min(cursor, busca.entrada.text.length)
            })
    }
    // B-4 da revisão do VIS2: no celular, depois de uma ação numa linha a lista voltava ao começo (o topo ocupa a área
    // toda) e as linhas sumiam da vista. Nas recargas por ação, a lista volta à mesma LINHA (e não ao mesmo pixel: as
    // linhas têm alturas diferentes e são criadas sob demanda, e o mesmo contentY caía sete linhas adiante).
    function recarregarNoLugar() {
        const i = Tema.celular ? lista.indexAt(lista.width / 2, lista.contentY + 1) : -1
        // R-2 da re-revisão 3 do Fable: com a lista pouco rolada, o alto da área ainda é o topo da seção (o cabeçalho
        // da lista: a busca e as pílulas), e ali não há linha (i < 0). Nesse caso a lista volta ao mesmo deslocamento
        // em pixels, que vale porque o topo não muda de altura com a ação.
        const deslocamento = Tema.celular && i < 0 ? lista.contentY - lista.originY : -1
        carregar()
        if (i >= 0)
            Qt.callLater(() => lista.positionViewAtIndex(Math.min(i, lista.count - 1), ListView.Beginning))
        else if (deslocamento > 0)
            Qt.callLater(() => {
                lista.contentY = lista.originY + Math.min(deslocamento, Math.max(0, lista.contentHeight - lista.height))
            })
    }
    function carregarDados() {
        const r = Api.chamar("encomendas_painel", { texto: busca.texto, quais: quais })
        if (r.ok)
            painel = r.dados
        else
            janela.avisar(r.erro, "erro")
        buscou = true
        if (alvo.indexOf("encomenda:") === 0)
            Qt.callLater(mostrarAlvo)
    }
    function mostrarAlvo() {
        const id = alvo.slice("encomenda:".length)
        alvo = ""
        const i = painel ? painel.encomendas.findIndex(e => e.id === id) : -1
        if (i < 0)
            return
        lista.positionViewAtIndex(i, ListView.Contain)
        linhaEscolhida = id
        Qt.callLater(() => janela.destacarAlvo(lista.itemAtIndex(i), null))
    }
    function nova() {
        nova_.abrir()
    }
    // Ações sobre uma encomenda que já existe (FALHA-04: com chave).
    function acao(comando, dados, mensagem) {
        const r = Api.gravar(comando, dados)
        if (!r.ok) {
            janela.avisar(r.erro, "erro")
            return false
        }
        if (mensagem)
            janela.avisar(mensagem, "ok")
        recarregarNoLugar()
        return true
    }
    function avisar(e) {
        const r = Api.chamar("encomenda_whatsapp", { id: e.id })
        if (!r.ok) {
            janela.avisar(r.erro, "erro")
            return
        }
        Qt.openUrlExternally(r.dados.url)
        recarregarNoLugar()
    }
    function aoEsc() {
        if (busca.texto !== "") {
            busca.texto = ""
            carregar()
            return true
        }
        return false
    }

    Timer { id: espera; interval: 200; onTriggered: secao.carregar() }

    ColumnLayout {
        anchors.fill: parent
        spacing: 12

        ColumnLayout {
            id: topo
            spacing: 12
            Layout.fillWidth: true

            Linha {
                // T042: por cima da linha da barra de pesquisa logo abaixo: o halo da barra passa atrás, não por cima.
                z: 1
                colunasEstreito: 2
                spacing: 12
                Layout.fillWidth: true
                CartaoNumero {
                    rotulo: "Falta pedir"
                    prefixo: ""
                    valor: secao.painel ? String(secao.painel.aPedir) : ""
                    cor: Tema.rosa
                    corValor: secao.painel && secao.painel.aPedir > 0 ? Tema.vermelho : Tema.tinta
                }
                CartaoNumero {
                    rotulo: "Pedidas"
                    prefixo: ""
                    valor: secao.painel ? String(secao.painel.pedidas) : ""
                    nota: "esperando chegar"
                    cor: Tema.azul
                }
                CartaoNumero {
                    rotulo: "Chegaram"
                    prefixo: ""
                    valor: secao.painel ? String(secao.painel.chegaram) : ""
                    nota: secao.painel && secao.painel.semAviso > 0 ? secao.painel.semAviso + " sem avisar" : ""
                    cor: Tema.verde
                    corValor: Tema.verdeEscuro
                }
            }

            Linha {
                spacing: 14
                Layout.fillWidth: true
                Campo {
                    id: busca
                    icone: "busca"
                    altura: 46
                    tamanhoFonte: 16
                    dica: "Procurar: cliente, peça, código ou número"
                    Layout.fillWidth: true
                    onEditado: espera.restart()
                }
                Pilulas {
                    pequenas: true
                    modelo: [{ id: "abertas", nome: "Em andamento" }, { id: "todas", nome: "Todas" }]
                    atual: secao.quais
                    onEscolhido: id => {
                        secao.quais = id
                        secao.carregar()
                    }
                }
            }
        }

        ListView {
            id: lista
            // A geometria da lista para os roteiros (GEO:listaDoEstoque; R-5 da re-revisão 3).
            objectName: "listaDoEstoque"
            // T042: no celular, o topo (cartões e barra de pesquisa) rola dentro desta lista, que recorta o que passa da
            // borda (o halo da barra era cortado). A lista começa Tema.buscaAlcance antes e acaba depois, com essa margem
            // por dentro: o conteúdo fica no mesmo lugar e com a mesma largura (larguraUtil). No computador, nada muda.
            readonly property real folgaDoHalo: Tema.celular ? Tema.buscaAlcance : 0
            readonly property real larguraUtil: width - 2 * folgaDoHalo
            Layout.leftMargin: -folgaDoHalo
            Layout.rightMargin: -folgaDoHalo
            leftMargin: folgaDoHalo
            rightMargin: folgaDoHalo
            // R11: se a folga muda com a seção aberta (girar o tablet, mudar o tamanho da janela), o conteúdo volta ao lugar.
            onFolgaDoHaloChanged: contentX = -folgaDoHalo
            header: Tema.celular ? espacoDoTopo : null
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 6
            model: secao.painel ? secao.painel.encomendas : []
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                id: linha
                required property var modelData
                required property int index
                readonly property var e: modelData
                readonly property bool aberta: e.situacao !== "entregue" && e.situacao !== "cancelada"
                width: lista.larguraUtil
                implicitHeight: Tema.estreito ? 66 + (acoesEnc.width > 0 ? acoesEnc.height + 10 : 0) : 66
                radius: Tema.raio
                // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                readonly property bool par: (index + 1) % 2 === 0 && modelData.situacao !== "chegou"
                color: e.situacao === "chegou" ? Tema.verde : Tema.faixa(linha.par)
                palette.window: Tema.superficie(color)
                HoverHandler { id: sobre }
                TapHandler { onTapped: secao.linhaEscolhida = linha.e.id }
                RowLayout {
                    id: conteudoEnc
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    height: 66
                    anchors.leftMargin: Tema.estreito ? 14 : 18
                    anchors.rightMargin: 12
                    spacing: Tema.estreito ? 10 : 14
                    opacity: linha.aberta ? 1 : 0.7
                    Item {
                        visible: !Tema.estreito
                        Layout.preferredWidth: 100
                        implicitHeight: 26
                        Chip {
                            anchors.verticalCenter: parent.verticalCenter
                            texto: linha.e.nomeSituacao
                            color: linha.e.situacao === "aberta" ? Tema.rosa : linha.e.situacao === "chegou" ? Tema.papel : Tema.papel
                            corTexto: linha.e.situacao === "aberta" ? Tema.vermelho : linha.e.situacao === "chegou" ? Tema.verdeEscuro : Tema.tintaSuave
                        }
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        Text {
                            text: (linha.e.quantidade !== "1" ? linha.e.quantidade + " × " : "") + linha.e.descricao
                            textFormat: Text.PlainText
                            color: Tema.tintaDaFaixa(linha.par)
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: 20
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        Text {
                            text: [Tema.estreito ? linha.e.nomeSituacao : "", Tema.estreito && linha.aberta ? linha.e.nomePagamento : "",
                                   "Nº " + linha.e.numero + (linha.e.caixa ? " (" + linha.e.caixa + ")" : "") + " · " + linha.e.cliente, linha.e.telefone, linha.e.desde,
                                   linha.e.avisado ? "avisado " + linha.e.avisadoEm : "",
                                   linha.e.situacao === "cancelada" ? linha.e.motivo : linha.e.observacoes]
                                  .filter(x => !!x).join("  ·  ")
                            textFormat: Text.PlainText
                            color: Tema.tintaSuaveDaFaixa(linha.par)
                            font.family: Tema.serifa
                            font.italic: true
                            font.pixelSize: 13
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                    }
                    Chip {
                        visible: linha.aberta && !Tema.estreito
                        texto: linha.e.nomePagamento
                        color: linha.e.pagamento === "pago" ? Tema.verde : linha.e.pagamento === "falta" ? Tema.rosa : Tema.papel
                        corTexto: linha.e.pagamento === "pago" ? Tema.verdeEscuro : linha.e.pagamento === "falta" ? Tema.vermelho : Tema.tintaSuave
                    }
                    Item {
                        visible: !Tema.estreito
                        // S-4 da 2ª re-revisão: com a linha escolhida (o 10a e o I1 chegam assim), os botões passam
                        // de 330 px e cobriam a etiqueta do pagamento; o espaço é o da largura real deles.
                        Layout.preferredWidth: Math.max(330, acoesEnc.width)
                        implicitHeight: 36
                    }
                }
                // Botões: no computador, à direita; no celular, embaixo.
                Row {
                    spacing: 4
                    id: acoesEnc
                    opacity: conteudoEnc.opacity
                    layoutDirection: Tema.estreito ? Qt.LeftToRight : Qt.RightToLeft
                    x: Tema.estreito ? 14 : linha.width - 12 - width
                    y: Tema.estreito ? 66 : (66 - height) / 2
                    // Próximo passo (sempre à vista)
                    Botao {
                        visible: linha.e.situacao === "aberta"
                        text: "Já pedi"
                        tipo: "secundario"
                        altura: 36
                        onClicked: secao.acao("encomenda_pedir", { id: linha.e.id }, "")
                    }
                    Botao {
                        visible: linha.e.situacao === "pedida"
                        text: "Chegou"
                        tipo: "secundario"
                        altura: 36
                        onClicked: secao.janela.confirmar("A peça chegou?",
                            "Quando a nota da compra é lançada pelo XML, o Rustinho marca sozinho. Marque aqui se chegou sem nota.",
                            "Chegou", () => secao.acao("encomenda_chegou", { id: linha.e.id }, ""))
                    }
                    Botao {
                        visible: linha.e.situacao === "chegou" && linha.e.podeVender
                        text: "Vender"
                        tipo: "secundario"
                        altura: 36
                        onClicked: secao.janela.abrir("TelaVender", { encomendaId: linha.e.id })
                    }
                    Botao {
                        visible: linha.e.situacao === "chegou" && linha.e.podeEntregar
                        text: "Entregar"
                        tipo: "secundario"
                        altura: 36
                        onClicked: secao.janela.confirmar("Entregar ao cliente?",
                            linha.e.pagamento === "falta" ? linha.e.nomePagamento + " no Receber. Receba antes ou depois, pelo Receber." : "A encomenda nº " + linha.e.numero + (linha.e.caixa ? " (" + linha.e.caixa + ")" : "") + " sai da lista.",
                            "Entregar", () => secao.acao("encomenda_entregar", { id: linha.e.id }, "Encomenda entregue."))
                    }
                    Botao {
                        visible: linha.e.situacao === "chegou" && linha.e.temWhatsapp
                        text: linha.e.avisado ? "Avisar de novo" : "Avisar"
                        icone: "mensagem"
                        tipo: linha.e.avisado ? "discreto" : "principal"
                        altura: 36
                        onClicked: secao.avisar(linha.e)
                    }
                    // O resto, com o mouse em cima ou com a linha escolhida (H16)
                    Botao {
                        visible: linha.aberta && linha.e.pagamento === "falta" && (sobre.hovered || Tema.celular || secao.linhaEscolhida === linha.e.id)
                        text: "Receber"
                        tipo: "discreto"
                        altura: 36
                        onClicked: secao.janela.abrir("TelaReceber", { clienteInicial: linha.e.clienteId })
                    }
                    Botao {
                        visible: linha.e.situacao === "pedida" && (sobre.hovered || Tema.celular || secao.linhaEscolhida === linha.e.id)
                        text: "Não pedi"
                        tipo: "discreto"
                        altura: 36
                        onClicked: secao.acao("encomenda_pedir", { id: linha.e.id, desfazer: true }, "")
                    }
                    Botao {
                        visible: linha.aberta && linha.e.situacao !== "chegou" && linha.e.temWhatsapp && (sobre.hovered || Tema.celular || secao.linhaEscolhida === linha.e.id)
                        icone: "mensagem"
                        tipo: "discreto"
                        altura: 36
                        largura: 40
                        leftPadding: 0
                        rightPadding: 0
                        onClicked: secao.avisar(linha.e)
                    }
                    Botao {
                        visible: linha.aberta && (sobre.hovered || Tema.celular || secao.linhaEscolhida === linha.e.id)
                        text: "Cancelar encomenda"
                        tipo: "discreto"
                        altura: 36
                        onClicked: motivo.perguntar("Cancelar a encomenda nº " + linha.e.numero + (linha.e.caixa ? " (" + linha.e.caixa + ")" : "") + "?",
                                                    linha.e.descricao + " · " + linha.e.cliente, "Cancelar encomenda",
                                                    m => Api.gravar("encomenda_cancelar", { id: linha.e.id, motivo: m }))
                    }
                }
            }
            ColumnLayout {
                // O estado vazio. I8: no celular, vai para o fim do topo (rola junto com ele; fixo na lista,
                // ficava abaixo da borda, cortado e sem como rolar até ele).
                id: vazio
                anchors.centerIn: parent
                width: Math.min(parent.width - 40, 640)
                visible: secao.buscou && lista.count === 0
                spacing: 8
                Titulo {
                    text: busca.texto !== "" ? "Nada encontrado" : "Nenhuma encomenda em andamento"
                    font.pixelSize: 30
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Nota {
                    visible: busca.texto === ""
                    text: "Cliente pagou (tudo ou um sinal)? Em Vender, marque “Encomendar” na peça sem estoque. "
                          + "Vai pagar quando chegar? Use “Nova encomenda”, aqui em cima."
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
            }
        }
    }

    // ---------- Nova encomenda (paga quando chegar) ----------
    Painel {
        id: nova_
        width: Math.min(600, Tema.largura - 24)
        property string clienteId: ""
        property var achados: []
        property string produtoId: ""
        // FALHA-04: chave desta encomenda (nova a cada vez que a janela abre).
        property string chave: ""
        property string erro: ""
        property string campoErro: ""

        function abrir() {
            chave = Api.novaChave()
            clienteId = ""
            achados = []
            produtoId = ""
            erro = ""
            campoErro = ""
            nCliente.texto = ""
            nTelefone.texto = ""
            nPeca.texto = ""
            nQtd.texto = "1"
            nObs.texto = ""
            open()
            nCliente.focar()
        }
        function buscarCliente() {
            clienteId = ""
            const t = nCliente.texto.trim()
            if (t.length < 2) {
                achados = []
                return
            }
            const r = Api.chamar("pessoas_buscar", { texto: t, papel: "cliente", inativos: false, limite: 5 })
            achados = r.ok ? r.dados.itens : []
        }
        function escolherCliente(c) {
            clienteId = c.id
            nCliente.texto = c.nome
            nTelefone.texto = c.telefone || ""
            achados = []
            nPeca.focar()
        }
        function escolherPeca(id) {
            const r = Api.chamar("produto_abrir", { id: id })
            if (!r.ok)
                return
            nPeca.texto = r.dados.nome + (r.dados.marca ? " · " + r.dados.marca : "")
            produtoId = id
            nQtd.focar()
        }
        function salvar() {
            const r = Api.gravar("encomenda_criar", {
                clienteId: clienteId,
                clienteNome: clienteId ? "" : nCliente.texto,
                telefone: nTelefone.texto,
                produtoId: produtoId,
                descricao: produtoId ? "" : nPeca.texto,
                quantidade: nQtd.texto,
                observacoes: nObs.texto
            }, chave)
            if (!r.ok) {
                erro = r.erro
                campoErro = r.campo || ""
                return
            }
            close()
            secao.janela.avisar("Encomenda anotada. Falta pedir ao fornecedor.", "ok")
            secao.carregar()
        }

        Timer { id: esperaCli; interval: 200; onTriggered: nova_.buscarCliente() }

        contentItem: ColumnLayout {
            spacing: 12
            Titulo { text: "Nova encomenda"; font.pixelSize: 32; Layout.fillWidth: true }
            Nota {
                text: "Para o cliente que paga quando a peça chegar. Se ele vai pagar agora (tudo ou um sinal), "
                      + "faça a venda e marque “Encomendar” na peça."
                font.pixelSize: 14
                Layout.fillWidth: true
            }
            Campo {
                id: nCliente
                rotulo: "Cliente"
                icone: "pessoa"
                dica: "Nome, CPF, telefone ou placa"
                erro: nova_.campoErro === "cliente" ? nova_.erro : ""
                Layout.fillWidth: true
                onEditado: esperaCli.restart()
            }
            Repeater {
                model: nova_.achados
                delegate: Rectangle {
                    required property var modelData
                    Layout.fillWidth: true
                    implicitHeight: 38
                    radius: Tema.raioCampo
                    color: sobreCli.hovered ? Tema.azul : Tema.vela(0.45)
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        x: 12
                        width: parent.width - 24
                        text: modelData.nome + (modelData.telefone ? "  ·  " + modelData.telefone : "")
                        textFormat: Text.PlainText
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.pixelSize: 14
                        elide: Text.ElideRight
                    }
                    HoverHandler { id: sobreCli; cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: nova_.escolherCliente(modelData) }
                }
            }
            Campo {
                id: nTelefone
                rotulo: "Celular (para o WhatsApp)"
                dica: "(15) 91234-0000"
                maximo: 20
                erro: nova_.campoErro === "telefone" ? nova_.erro : ""
                Layout.fillWidth: true
            }
            Linha {
                spacing: 10
                Layout.fillWidth: true
                Campo {
                    id: nPeca
                    rotulo: nova_.produtoId ? "Peça (cadastrada)" : "Peça"
                    dica: "Procure a cadastrada ou descreva (ex.: farol Gol G5)"
                    maximo: 160
                    erro: nova_.campoErro === "peca" ? nova_.erro : ""
                    Layout.fillWidth: true
                    onEditado: nova_.produtoId = ""
                }
                Botao {
                    text: "Procurar"
                    icone: "busca"
                    tipo: "secundario"
                    Layout.alignment: Qt.AlignBottom
                    onClicked: escolher.abrirCom("", nPeca.texto)
                }
            }
            Linha {
                spacing: 10
                Layout.fillWidth: true
                Campo {
                    id: nQtd
                    rotulo: "Quantidade"
                    numerico: true
                    maximo: 8
                    erro: nova_.campoErro === "quantidade" ? nova_.erro : ""
                    Layout.preferredWidth: 140
                }
                Campo {
                    id: nObs
                    rotulo: "Observação"
                    dica: "Ex.: cliente vem buscar sábado"
                    maximo: 300
                    Layout.fillWidth: true
                    onConfirmado: nova_.salvar()
                }
            }
            TextoErro { text: nova_.campoErro === "" ? nova_.erro : "" }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao { text: "Anotar encomenda"; icone: "check"; onClicked: nova_.salvar() }
            }
        }
    }

    EscolherPeca {
        id: escolher
        onEscolhida: id => nova_.escolherPeca(id)
    }

    PerguntaMotivo { id: motivo; onFeito: secao.recarregarNoLugar() }

    // I8 (VIS-C8-02, 15/11/2026): no celular, o cabeçalho fixo da tela tomava quase toda a altura e a lista ficava
    // com poucos dp (as linhas e o estado vazio escondidos). Lá, o topo desta seção (cartões, busca e filtros) rola
    // junto com a lista: fica dentro dela, no lugar do cabeçalho (`espacoDoTopo`). No computador, nada muda. As
    // linhas continuam sob demanda (o ListView só cria as que aparecem), mesmo com milhares de peças.
    Component {
        id: espacoDoTopo
        Item { width: lista.larguraUtil; height: topo.implicitHeight + 12 }
    }
    function topoNaLista() {
        topo.parent = lista.contentItem
        topo.x = 0
        topo.width = Qt.binding(() => lista.larguraUtil)
        // B-1 da revisão do VIS2: dentro da lista, o topo seguia com a altura que o arranjo de fora lhe deu no começo,
        // e não com a natural; os blocos se espalhavam e as pílulas passavam por cima da 1ª linha.
        topo.height = Qt.binding(() => topo.implicitHeight)
        topo.y = Qt.binding(() => lista.headerItem ? lista.headerItem.y : 0)
        vazio.anchors.centerIn = undefined
        vazio.parent = topo
        vazio.Layout.fillWidth = true
        vazio.Layout.topMargin = 20
    }

    Component.onCompleted: {
        if (Tema.celular)
            topoNaLista()
        carregar()
        busca.focar()
    }
}
