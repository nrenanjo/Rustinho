import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Templates as T
import QtQuick.Layouts
import rustinho

// Vender: leitor de código de barras ou busca, carrinho, cliente e receber.
FocusScope {
    id: tela
    // Celular: mostra a parte do cliente e total no lugar do carrinho.
    property bool verLateral: false
    readonly property var janela: ApplicationWindow.window
    // Orçamento a carregar ao abrir (vindo da tela de orçamentos).
    property string orcamentoId: ""
    // Encomenda a vender na entrega (vinda de Estoque › Encomendas).
    property string encomendaId: ""
    property string encomendaCarregada: ""
    // Ordem de serviço a entregar (vinda da Oficina).
    property string osId: ""
    property string osCarregada: ""
    property int osNumero: 0
    // Vale-troca que veio da troca (Trocas e devoluções → Trocar agora).
    property string valeId: ""
    property var valeTroca: null
    // Vales com saldo (aparecem no Receber).
    property var vales: []
    property int encomendaNumero: 0
    property int encomendasNaVenda: 0

    property var carrinho: []        // [{produtoId, quantidade, preco, desconto}]
    property var calculo: null
    property var resultados: []
    property var caixaEstado: null
    property string clienteId: ""
    property string clienteNome: ""
    property var clientesAchados: []
    property string orcamentoCarregado: ""
    property int itemAtual: -1
    property var formas: []
    property var ultimaVenda: null
    readonly property bool caixaAberto: !!caixaEstado && caixaEstado.aberto

    // ---------- Dados ----------

    function entrada(pagamentos) {
        return {
            itens: carrinho,
            tabela: 1,
            clienteId: clienteId,
            clienteNome: clienteId ? "" : clienteNome,
            desconto: descontoTotal.texto,
            pagamentos: pagamentos || [],
            observacoes: "",
            orcamentoId: orcamentoCarregado,
            validadeDias: 0,
            parcelas: parseInt(campoParcelas.texto) || 1,
            primeiroVencimento: campoVencimento.texto,
            cheques: painelCheque.dados(),
            encomendaId: encomendaCarregada,
            osId: osCarregada
        }
    }

    function recalcular() {
        if (carrinho.length === 0) {
            calculo = null
            return
        }
        const r = Api.chamar("venda_calcular", entrada())
        if (r.ok)
            calculo = r.dados
        else
            janela.avisar(r.erro, "erro")
    }

    function carregarCaixa() {
        const r = Api.chamar("caixa_estado")
        if (r.ok)
            caixaEstado = r.dados
    }

    function separarQuantidade(texto) {
        // "3*7891234567895" ou "3 * filtro" = 3 unidades
        const m = texto.match(/^\s*(\d+(?:[.,]\d+)?)\s*\*\s*(.+)$/)
        return m ? { quantidade: m[1], texto: m[2].trim() } : { quantidade: "1", texto: texto.trim() }
    }

    function adicionar(produtoId, quantidade) {
        const lista = carrinho.slice()
        const i = lista.findIndex(x => x.produtoId === produtoId && x.preco === "" && x.desconto === "")
        if (i >= 0) {
            const atual = parseFloat((lista[i].quantidade || "0").replace(".", "").replace(",", "."))
            const mais = parseFloat(quantidade.replace(",", "."))
            const soma = (atual + mais)
            lista[i] = Object.assign({}, lista[i], { quantidade: String(Number.isInteger(soma) ? soma : soma.toFixed(3)).replace(".", ",") })
            itemAtual = i
        } else {
            lista.push({ produtoId: produtoId, quantidade: quantidade, preco: "", desconto: "" })
            itemAtual = lista.length - 1
        }
        carrinho = lista
        recalcular()
        Qt.callLater(() => listaCarrinho.positionViewAtIndex(itemAtual, ListView.Contain))
    }

    // Enter na busca: código exato (leitor) entra direto; senão, o resultado escolhido.
    function confirmarBusca() {
        const p = separarQuantidade(busca.texto)
        if (p.texto === "")
            return
        const r = Api.chamar("produto_por_codigo", { texto: p.texto })
        if (r.ok && r.dados.id) {
            adicionar(r.dados.id, p.quantidade)
            limparBusca()
            return
        }
        espera.stop()
        buscar()
        if (resultados.length > 0 && listaResultados.currentIndex >= 0) {
            adicionar(resultados[listaResultados.currentIndex].id, p.quantidade)
            limparBusca()
        } else {
            janela.avisar("Nenhuma peça com “" + p.texto + "”.", "erro")
        }
    }

    function buscar() {
        const p = separarQuantidade(busca.texto)
        if (p.texto.length < 2) {
            resultados = []
            return
        }
        const r = Api.chamar("produtos_buscar", { texto: p.texto, inativas: false, limite: 30 })
        if (r.ok) {
            resultados = r.dados.itens
            listaResultados.currentIndex = resultados.length > 0 ? 0 : -1
        }
    }

    function limparBusca() {
        busca.texto = ""
        resultados = []
        busca.focar()
    }

    function mudarItem(i, campo, valor) {
        const lista = carrinho.slice()
        lista[i] = Object.assign({}, lista[i])
        lista[i][campo] = valor
        carrinho = lista
        recalcular()
    }

    function alternarEncomenda(i) {
        const lista = carrinho.slice()
        lista[i] = Object.assign({}, lista[i], { encomendar: !lista[i].encomendar })
        carrinho = lista
        recalcular()
        if (lista[i].encomendar && !clienteId)
            janela.avisar("Escolha o cliente, para avisar quando a peça chegar.", "info")
    }

    function removerItem(i) {
        const lista = carrinho.slice()
        lista.splice(i, 1)
        carrinho = lista
        itemAtual = Math.min(itemAtual, lista.length - 1)
        recalcular()
        busca.focar()
    }

    function limparVenda() {
        carrinho = []
        calculo = null
        clienteId = ""
        clienteNome = ""
        campoCliente.texto = ""
        descontoTotal.texto = ""
        orcamentoCarregado = ""
        encomendaCarregada = ""
        encomendaNumero = 0
        osCarregada = ""
        osNumero = 0
        valeTroca = null
        itemAtual = -1
        limparBusca()
    }

    // "1.234,56" → centavos, e de volta (sem ponto de milhar).
    function centavos(t) {
        const v = parseFloat(String(t || "").replace(/\./g, "").replace(",", "."))
        return isNaN(v) ? 0 : Math.round(v * 100)
    }
    function reais(c) {
        return (c / 100).toFixed(2).replace(".", ",")
    }

    function aoEsc() {
        if (busca.texto !== "") {
            limparBusca()
        } else if (carrinho.length > 0) {
            janela.confirmar("Limpar esta venda?", "Os itens do carrinho serão tirados. Nada foi gravado ainda.",
                             "Limpar", () => tela.limparVenda(), "perigo")
        } else {
            janela.voltar()
        }
    }

    function focarBusca() {
        busca.focar()
    }

    // ---------- Cliente ----------

    function buscarCliente() {
        const t = campoCliente.texto.trim()
        clienteId = ""
        clienteNome = t
        if (t.length < 2) {
            clientesAchados = []
            return
        }
        const r = Api.chamar("pessoas_buscar", { texto: t, papel: "cliente", inativos: false, limite: 6 })
        clientesAchados = r.ok ? r.dados.itens : []
    }

    function escolherCliente(c) {
        clienteId = c.id
        clienteNome = c.nome
        campoCliente.texto = c.nome
        clientesAchados = []
        recalcular()

    }

    // ---------- Receber ----------

    function abrirReceber() {
        if (!caixaAberto) {
            janela.avisar("Abra o caixa antes de vender.", "erro")
            return
        }
        if (carrinho.length === 0) {
            janela.avisar("Passe ou busque pelo menos uma peça.", "erro")
            busca.focar()
            return
        }
        recalcular()
        if (calculo && calculo.itens.some(i => i.erro)) {
            janela.avisar("Corrija o item marcado em rosa.", "erro")
            return
        }
        if (calculo && calculo.erroEncomenda) {
            janela.avisar(calculo.erroEncomenda, "erro")
            campoCliente.focar()
            return
        }
        receber.abrirCom(calculo.total)
    }

    function concluir(pagamentos) {
        const encomendas = calculo ? calculo.encomendas : 0
        const r = Api.chamar("venda_concluir", entrada(pagamentos))
        if (!r.ok) {
            receber.erro = r.erro
            return
        }
        receber.close()
        encomendasNaVenda = encomendas
        ultimaVenda = r.dados
        limparVenda()
        carregarCaixa()
        concluida.open()
    }

    function salvarOrcamento(dias, observacoes) {
        const e = entrada()
        e.validadeDias = dias
        e.observacoes = observacoes
        const r = Api.chamar("orcamento_salvar", e)
        if (!r.ok) {
            janela.avisar(r.erro, "erro")
            return
        }
        caixaOrcamento.close()
        limparVenda()
        janela.avisar("Orçamento " + r.dados.numero + " salvo.", "ok")
        tela.imprimir("orcamento", r.dados.id)
    }

    function imprimir(tipo, id) {
        const r = JSON.parse(Sistema.imprimir(tipo, id))
        if (!r.ok)
            janela.avisar(r.erro, "erro")
        else if (r.dados.impresso)
            janela.avisar("Enviado para a impressora.", "ok")
    }

    Timer {
        id: espera
        interval: 140
        onTriggered: tela.buscar()
    }
    Timer {
        id: esperaCliente
        interval: 200
        onTriggered: tela.buscarCliente()
    }

    Shortcut {
        sequence: "F2"
        enabled: tela.StackView.status === StackView.Active && !receber.opened && !concluida.opened
        onActivated: tela.abrirReceber()
    }
    Shortcut {
        sequence: "F6"
        enabled: tela.StackView.status === StackView.Active && tela.carrinho.length > 0
        onActivated: caixaOrcamento.abrirCom()
    }

    // Setas mexem nos resultados enquanto se digita na busca.
    Keys.onDownPressed: if (resultados.length > 0) listaResultados.currentIndex = Math.min(listaResultados.currentIndex + 1, listaResultados.count - 1)
    Keys.onUpPressed: if (resultados.length > 0) listaResultados.currentIndex = Math.max(listaResultados.currentIndex - 1, 0)

    // ---------- Tela ----------

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem - 4
        anchors.topMargin: Tema.margem - 16
        spacing: 12

        RowLayout {
            spacing: 14
            Layout.fillWidth: true
            ColumnLayout {
                spacing: 0
                Layout.fillWidth: true
                Botao {
                    tipo: "discreto"
                    icone: "voltar"
                    text: "Início"
                    implicitHeight: 32
                    font.pixelSize: 14
                    Layout.leftMargin: -10
                    onClicked: tela.aoEsc()
                }
                RowLayout {
                    spacing: 16
                    Titulo {
                        text: tela.osCarregada ? "Entregar OS nº " + tela.osNumero
                              : tela.encomendaCarregada ? "Vender encomenda nº " + tela.encomendaNumero
                              : tela.orcamentoCarregado ? "Vender orçamento" : "Vender"
                        font.pixelSize: Tema.estreito ? 32 : 42
                    }
                    Rectangle {
                        visible: !!tela.valeTroca
                        radius: height / 2
                        color: Tema.verde
                        implicitHeight: 30
                        implicitWidth: textoVale.implicitWidth + 24
                        Layout.alignment: Qt.AlignVCenter
                        Text {
                            id: textoVale
                            anchors.centerIn: parent
                            text: tela.valeTroca ? "Vale-troca nº " + tela.valeTroca.numero + " · R$ " + tela.valeTroca.saldo + " (vai no Receber)" : ""
                            color: Tema.verdeEscuro
                            font.family: Tema.texto
                            font.weight: Font.DemiBold
                            font.pixelSize: 14
                        }
                    }
                    Nota {
                        text: !tela.caixaEstado ? "" : tela.caixaAberto
                              ? tela.caixaEstado.caixa + " · aberto por " + tela.caixaEstado.resumo.sessao.abertoPor
                                + " às " + Qt.formatTime(new Date(tela.caixaEstado.resumo.sessao.abertoEm), "HH:mm")
                              : tela.caixaEstado.caixa + " · caixa fechado"
                        visible: !Tema.estreito
                        font.pixelSize: 16
                        Layout.alignment: Qt.AlignBaseline
                    }
                }
            }
            Olho { Layout.alignment: Qt.AlignBottom; Layout.bottomMargin: 4 }
            Botao {
                text: "Caixa"
                tipo: "discreto"
                icone: "caixa"
                Layout.alignment: Qt.AlignBottom
                onClicked: tela.janela.abrir("TelaCaixa")
            }
        }

        RowLayout {
            spacing: 18
            Layout.fillWidth: true
            Layout.fillHeight: true

            // ---------- Busca e carrinho ----------
            ColumnLayout {
                visible: !Tema.estreito || !tela.verLateral
                spacing: 10
                Layout.fillWidth: true
                Layout.fillHeight: true

                Campo {
                    id: busca
                    icone: "busca"
                    atalho: "F11"
                    camera: true
                    altura: 56
                    tamanhoFonte: 20
                    dica: Tema.estreito ? "Código, nome ou carro" : "Passe o leitor ou digite qualquer código, nome, modelo ou carro (3* antes = 3 unidades)"
                    Layout.fillWidth: true
                    onEditado: espera.restart()
                    onConfirmado: tela.confirmarBusca()
                }

                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    // Carrinho
                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 6

                        RowLayout {
                            visible: tela.carrinho.length > 0 && !Tema.estreito
                            spacing: 12
                            Layout.fillWidth: true
                            Layout.leftMargin: 16
                            Layout.rightMargin: 12
                            Item { Layout.preferredWidth: 26 }
                            Rotulo { text: "Peça"; Layout.fillWidth: true }
                            Rotulo { text: "Qtd."; Layout.preferredWidth: 96 }
                            Rotulo { text: "Preço"; Layout.preferredWidth: 118 }
                            Rotulo { text: "Desconto"; Layout.preferredWidth: 92 }
                            Rotulo { text: "Total"; horizontalAlignment: Text.AlignRight; Layout.preferredWidth: 116 }
                            Item { Layout.preferredWidth: 36 }
                        }

                        ListView {
                            id: listaCarrinho
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            clip: true
                            spacing: 6
                            model: tela.calculo ? tela.calculo.itens : []
                            boundsBehavior: Flickable.StopAtBounds
                            ScrollBar.vertical: ScrollBar {}
                            delegate: LinhaCarrinho {
                                required property var modelData
                                required property int index
                                width: listaCarrinho.width
                                item: modelData
                                entrada: tela.carrinho[index] || ({})
                                indice: index
                                atual: tela.itemAtual === index
                                onMudou: (campo, valor) => tela.mudarItem(index, campo, valor)
                                onRemover: tela.removerItem(index)
                                onSelecionar: tela.itemAtual = index
                                onAbrirPeca: if (!modelData.servico) tela.janela.abrir("TelaPeca", { produtoId: modelData.produtoId })
                                onEncomendar: tela.alternarEncomenda(index)
                            }

                            ColumnLayout {
                                anchors.centerIn: parent
                                width: Math.min(520, parent.width - 40)
                                visible: tela.carrinho.length === 0
                                spacing: 10
                                Icone {
                                    nome: "caixa"
                                    tamanho: 56
                                    cor: Tema.dourado
                                    Layout.alignment: Qt.AlignHCenter
                                }
                                Titulo {
                                    text: tela.caixaAberto ? "Passe o leitor ou busque a peça" : "O caixa está fechado"
                                    font.pixelSize: 30
                                    horizontalAlignment: Text.AlignHCenter
                                    Layout.fillWidth: true
                                }
                                Nota {
                                    text: tela.caixaAberto
                                          ? "A busca mostra o estoque e o preço de cada peça. Enter põe no carrinho; F2 recebe."
                                          : "Abra o caixa com o troco que está na gaveta para começar a vender. Dá para montar um orçamento mesmo com o caixa fechado."
                                    font.pixelSize: 16
                                    horizontalAlignment: Text.AlignHCenter
                                    Layout.fillWidth: true
                                }
                                Linha {
                                    visible: !!tela.caixaEstado && !tela.caixaAberto
                                    spacing: 10
                                    Layout.alignment: Qt.AlignHCenter
                                    Layout.topMargin: 6
                                    Campo {
                                        id: trocoInicial
                                        rotulo: "Troco na gaveta"
                                        prefixo: "R$"
                                        numerico: true
                                        dica: "0,00"
                                        Layout.preferredWidth: 180
                                        onConfirmado: botaoAbrir.clicked()
                                    }
                                    Botao {
                                        id: botaoAbrir
                                        text: "Abrir caixa"
                                        Layout.alignment: Qt.AlignBottom
                                        onClicked: {
                                            const r = Api.chamar("caixa_abrir", { valor: trocoInicial.texto })
                                            if (r.ok) {
                                                tela.carregarCaixa()
                                                tela.janela.avisar("Caixa aberto.", "ok")
                                                busca.focar()
                                            } else {
                                                trocoInicial.erro = r.erro
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Resultados da busca (por cima do carrinho)
                    Rectangle {
                        visible: tela.resultados.length > 0 && busca.texto.trim() !== ""
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        height: Math.min(listaResultados.contentHeight + 12, parent.height)
                        radius: Tema.raio
                        color: Tema.papel
                        border.width: 1
                        border.color: Qt.rgba(0.16, 0.15, 0.13, 0.15)
                        z: 10

                        ListView {
                            id: listaResultados
                            anchors.fill: parent
                            anchors.margins: 6
                            clip: true
                            spacing: 2
                            model: tela.resultados
                            currentIndex: -1
                            highlightMoveDuration: 0
                            boundsBehavior: Flickable.StopAtBounds
                            ScrollBar.vertical: ScrollBar {}
                            onCurrentIndexChanged: if (currentIndex >= 0) positionViewAtIndex(currentIndex, ListView.Contain)
                            delegate: Rectangle {
                                required property var modelData
                                required property int index
                                readonly property bool atual: listaResultados.currentIndex === index
                                width: listaResultados.width
                                height: 54
                                radius: Tema.raioCampo
                                color: atual ? Tema.tinta : sobre.hovered ? Tema.azul : "transparent"
                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 12
                                    anchors.rightMargin: 12
                                    spacing: Tema.estreito ? 8 : 14
                                    Text {
                                        text: modelData.codigo
                                        color: atual ? Tema.claro : Tema.tintaSuave
                                        font.family: Tema.texto
                                        font.weight: Font.DemiBold
                                        font.pixelSize: 14
                                        visible: !Tema.estreito
                                        Layout.preferredWidth: 64
                                    }
                                    ColumnLayout {
                                        spacing: 0
                                        Layout.fillWidth: true
                                        Item {
                                            Layout.fillWidth: true
                                            implicitHeight: nomeVenda.implicitHeight
                                            Text {
                                                id: nomeVenda
                                                width: Math.min(implicitWidth, parent.width - (achouVenda.visible ? achouVenda.implicitWidth + 8 : 0))
                                                text: modelData.nome
                                                color: atual ? Tema.claro : Tema.tinta
                                                font.family: Tema.titulo
                                                font.weight: Font.Bold
                                                font.pixelSize: 19
                                                elide: Text.ElideRight
                                            }
                                            Balao {
                                                id: achouVenda
                                                x: nomeVenda.width + 8
                                                anchors.verticalCenter: nomeVenda.verticalCenter
                                                texto: modelData.achou || ""
                                                escuro: atual
                                            }
                                        }
                                        Text {
                                            text: [Tema.estreito ? modelData.codigo : "", Tema.estreito ? modelData.local : "", modelData.marca, modelData.codigoOriginal, modelData.aplicacoes].filter(x => !!x).join("  ·  ")
                                            color: atual ? Qt.rgba(0.95, 0.93, 0.88, 0.75) : Tema.tintaSuave
                                            font.family: Tema.serifa
                                            font.italic: true
                                            font.pixelSize: 13
                                            elide: Text.ElideRight
                                            Layout.fillWidth: true
                                        }
                                    }
                                    Text {
                                        text: modelData.local
                                        color: atual ? Tema.claro : Tema.tintaSuave
                                        font.family: Tema.texto
                                        font.pixelSize: 14
                                        visible: !Tema.estreito
                                        Layout.preferredWidth: 60
                                    }
                                    Rectangle {
                                        radius: height / 2
                                        color: modelData.estoqueBaixo && !atual ? Tema.rosa : "transparent"
                                        implicitWidth: Math.max(Tema.estreito ? 0 : 80, txtEst.implicitWidth + 16)
                                        implicitHeight: txtEst.implicitHeight + 6
                                        Text {
                                            id: txtEst
                                            anchors.centerIn: parent
                                            text: modelData.estoque + " " + modelData.unidade
                                            color: modelData.estoqueBaixo ? (atual ? "#F2B8AE" : Tema.vermelho) : (atual ? Tema.claro : Tema.tinta)
                                            font.family: Tema.texto
                                            font.weight: Font.Bold
                                            font.pixelSize: 15
                                        }
                                    }
                                    Text {
                                        text: modelData.preco ? "R$ " + modelData.preco : "—"
                                        color: atual ? Tema.claro : Tema.tinta
                                        font.family: Tema.texto
                                        font.weight: Font.Bold
                                        font.pixelSize: Tema.estreito ? 16 : 18
                                        horizontalAlignment: Text.AlignRight
                                        Layout.preferredWidth: Tema.estreito ? -1 : 110
                                    }
                                }
                                HoverHandler { id: sobre; cursorShape: Qt.PointingHandCursor }
                                TapHandler {
                                    onTapped: {
                                        tela.adicionar(modelData.id, tela.separarQuantidade(busca.texto).quantidade)
                                        tela.limparBusca()
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ---------- Cliente e total ----------
            // No celular, fica numa "página" à parte (botão no rodapé).
            ColumnLayout {
                visible: !Tema.estreito || tela.verLateral
                spacing: 12
                Layout.preferredWidth: Tema.estreito ? -1 : 340
                Layout.maximumWidth: Tema.estreito ? Number.POSITIVE_INFINITY : 340
                Layout.fillWidth: Tema.estreito
                Layout.fillHeight: true

                Botao {
                    visible: Tema.estreito
                    tipo: "discreto"
                    icone: "voltar"
                    text: "Voltar ao carrinho"
                    Layout.leftMargin: -10
                    onClicked: tela.verLateral = false
                }

                Faixa {
                    titulo: "Cliente"
                    subtitulo: tela.clienteId ? "cadastrado" : "opcional"
                    color: Tema.pergaminho
                    Layout.fillWidth: true
                    Campo {
                        id: campoCliente
                        dica: "Nome, CPF, telefone ou placa"
                        icone: "pessoa"
                        Layout.fillWidth: true
                        onEditado: esperaCliente.restart()
                    }
                    Repeater {
                        model: tela.clientesAchados
                        delegate: Rectangle {
                            required property var modelData
                            Layout.fillWidth: true
                            implicitHeight: 40
                            radius: Tema.raioCampo
                            color: sobreCliente.hovered ? Tema.azul : Qt.rgba(1, 1, 1, 0.4)
                            ColumnLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 10
                                spacing: 0
                                Text {
                                    text: modelData.nome
                                    color: Tema.tinta
                                    font.family: Tema.texto
                                    font.weight: Font.DemiBold
                                    font.pixelSize: 14
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                                Text {
                                    text: [Tema.sigilo(modelData.documento), modelData.telefone, modelData.veiculos].filter(x => !!x).join(" · ")
                                    color: Tema.tintaSuave
                                    font.family: Tema.texto
                                    font.pixelSize: 12
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                            }
                            HoverHandler { id: sobreCliente; cursorShape: Qt.PointingHandCursor }
                            TapHandler { onTapped: tela.escolherCliente(modelData) }
                        }
                    }
                    Text {
                        visible: !!tela.calculo && tela.calculo.erroEncomenda !== ""
                        text: "Para encomendar, escolha o cliente cadastrado."
                        color: Tema.vermelho
                        font.family: Tema.texto
                        font.weight: Font.DemiBold
                        font.pixelSize: 13
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                }

                Faixa {
                    titulo: "Total"
                    color: Tema.verde
                    corTitulo: Tema.verdeEscuro
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    Repeater {
                        model: tela.calculo ? [
                            [tela.calculo.quantidadeItens === "1" ? "1 unidade" : tela.calculo.quantidadeItens + " unidades", ""],
                            ["Subtotal", "R$ " + tela.calculo.subtotal],
                            ["Desconto nos itens", tela.calculo.descontoItens !== "0,00" ? "− R$ " + tela.calculo.descontoItens : ""]
                        ].filter(l => l[0] && (l[1] !== "" || l[0].includes("unidade"))) : []
                        delegate: RowLayout {
                            required property var modelData
                            Layout.fillWidth: true
                            Text {
                                text: modelData[0]
                                color: Tema.tintaSuave
                                font.family: Tema.texto
                                font.pixelSize: 15
                                Layout.fillWidth: true
                            }
                            Text {
                                text: modelData[1]
                                color: Tema.tinta
                                font.family: Tema.texto
                                font.pixelSize: 15
                            }
                        }
                    }
                    Campo {
                        id: descontoTotal
                        rotulo: "Desconto na venda"
                        dica: "R$ ou %"
                        numerico: true
                        maximo: 10
                        erro: tela.calculo ? tela.calculo.erroDesconto : ""
                        Layout.fillWidth: true
                        onConfirmado: tela.recalcular()
                        onSaiu: tela.recalcular()
                    }
                    Item { Layout.fillHeight: true }
                    Rotulo { text: "Total a pagar"; color: Tema.verdeEscuro }
                    Text {
                        text: "R$ " + (tela.calculo ? tela.calculo.total : "0,00")
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.weight: Font.Bold
                        font.pixelSize: 46
                        fontSizeMode: Text.HorizontalFit
                        minimumPixelSize: 24
                        Layout.fillWidth: true
                    }
                    Botao {
                        text: "Receber"
                        icone: "check"
                        atalho: "F2"
                        implicitHeight: 52
                        font.pixelSize: 18
                        enabled: tela.carrinho.length > 0 && tela.caixaAberto
                        Layout.fillWidth: true
                        onClicked: tela.abrirReceber()
                    }
                    Botao {
                        text: "Salvar como orçamento"
                        tipo: "secundario"
                        atalho: "F6"
                        enabled: tela.carrinho.length > 0
                        Layout.fillWidth: true
                        onClicked: caixaOrcamento.abrirCom()
                    }
                }
            }
        }
        // Celular: total sempre à vista, com o botão de receber.
        Rectangle {
            visible: Tema.estreito && !tela.verLateral
            color: Tema.verde
            radius: Tema.raio
            Layout.fillWidth: true
            implicitHeight: rodapeLinha.implicitHeight + 20
            RowLayout {
                id: rodapeLinha
                x: 12
                width: parent.width - 24
                anchors.verticalCenter: parent.verticalCenter
                spacing: 10
                ColumnLayout {
                    spacing: 0
                    Layout.fillWidth: true
                    Text {
                        text: "R$ " + (tela.calculo ? tela.calculo.total : "0,00")
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.weight: Font.Bold
                        font.pixelSize: 26
                        fontSizeMode: Text.HorizontalFit
                        minimumPixelSize: 16
                        Layout.fillWidth: true
                    }
                    Text {
                        text: (tela.clienteId || campoCliente.texto ? campoCliente.texto : "Cliente e desconto") + "  ›"
                        color: Tema.verdeEscuro
                        font.family: Tema.texto
                        font.weight: Font.DemiBold
                        font.pixelSize: 14
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                        TapHandler { onTapped: tela.verLateral = true }
                    }
                }
                Botao {
                    text: "Receber"
                    icone: "check"
                    implicitHeight: 48
                    enabled: tela.carrinho.length > 0 && tela.caixaAberto
                    onClicked: tela.abrirReceber()
                }
            }
        }
    }

    // ---------- Receber ----------
    Popup {
        id: receber
        property string total: "0,00"
        property string erro: ""
        property var calc: null
        modal: true
        focus: true
        anchors.centerIn: T.Overlay.overlay
        width: Math.min(660, Tema.largura - 24)
        padding: Tema.estreito ? 18 : 30
        closePolicy: Popup.CloseOnEscape
        T.Overlay.modal: Rectangle { color: Qt.rgba(0.16, 0.15, 0.13, 0.35) }
        background: Rectangle { color: Tema.pergaminho; radius: Tema.raio + 3 }

        function abrirCom(total) {
            receber.total = total
            receber.erro = ""
            for (let i = 0; i < repFormas.count; i++)
                repFormas.itemAt(i).campo.texto = ""
            campoParcelas.texto = "1"
            campoVencimento.texto = ""
            painelCheque.limpar()
            const rv = Api.chamar("vales_abertos", { id: tela.clienteId })
            tela.vales = rv.ok ? rv.dados : []
            campoVale.texto = ""
            let iv = tela.valeTroca ? tela.vales.findIndex(v => v.id === tela.valeTroca.id) : -1
            if (iv < 0 && tela.clienteId)
                iv = tela.vales.findIndex(v => v.clienteId === tela.clienteId)
            escolhaVale.indice = Math.max(0, iv)
            receber.open()
            const dinheiro = repFormas.itemAt(0)
            if (iv >= 0 && tela.valeTroca) {
                // Veio da troca: o vale já entra; o resto fica no dinheiro.
                const usar = Math.min(tela.centavos(tela.vales[iv].saldo), tela.centavos(total))
                campoVale.texto = tela.reais(usar)
                const resto = tela.centavos(total) - usar
                dinheiro.campo.texto = resto > 0 ? tela.reais(resto) : ""
            } else {
                dinheiro.campo.texto = total
            }
            atualizar()
            dinheiro.campo.focar()
        }

        function pagamentos() {
            const lista = []
            for (let i = 0; i < repFormas.count; i++) {
                const l = repFormas.itemAt(i)
                if (l.campo.texto.trim() !== "")
                    lista.push({ forma: l.forma, valor: l.campo.texto })
            }
            if (tela.vales.length > 0 && campoVale.texto.trim() !== "")
                lista.push({ forma: "vale", valor: campoVale.texto, valeId: tela.vales[escolhaVale.indice].id })
            return lista
        }

        function atualizar() {
            const r = Api.chamar("venda_calcular", tela.entrada(pagamentos()))
            if (r.ok)
                calc = r.dados
        }

        // Clicar no nome da forma põe nela o que falta. Se nada falta (o
        // valor veio todo em dinheiro), leva o total para esta forma.
        function completarVale() {
            campoVale.texto = ""
            atualizar()
            const saldo = tela.centavos(tela.vales[escolhaVale.indice].saldo)
            const falta = calc ? tela.centavos(calc.falta) : 0
            campoVale.texto = tela.reais(Math.min(saldo, falta))
            atualizar()
            campoVale.focar()
        }

        function completar(linha) {
            linha.campo.texto = ""
            atualizar()
            if (calc && calc.falta === "0,00") {
                for (let i = 0; i < repFormas.count; i++)
                    repFormas.itemAt(i).campo.texto = ""
                atualizar()
            }
            linha.campo.texto = calc ? calc.falta : ""
            atualizar()
            linha.campo.focar()
        }

        contentItem: ColumnLayout {
            spacing: 12
            Titulo {
                text: "Receber R$ " + receber.total
                font.pixelSize: 34
                Layout.fillWidth: true
            }
            Nota {
                text: "Clique no nome da forma para pôr nela o que falta. Dá para dividir entre várias."
                font.pixelSize: 14
                Layout.fillWidth: true
            }
            Rectangle {
                visible: !!tela.calculo && tela.calculo.encomendas > 0
                Layout.fillWidth: true
                radius: Tema.raioCampo
                color: Tema.verde
                implicitHeight: linhaSinal.implicitHeight + 16
                RowLayout {
                    id: linhaSinal
                    x: 14
                    y: 8
                    width: parent.width - 24
                    spacing: 10
                    Nota {
                        text: "Tem encomenda. Recebeu um sinal? Digite o sinal na forma usada e deixe o resto para a entrega (fica no Receber)."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                    Botao {
                        text: "Resto na entrega"
                        tipo: "secundario"
                        implicitHeight: 36
                        onClicked: {
                            for (let i = 0; i < repFormas.count; i++) {
                                const l = repFormas.itemAt(i)
                                if (l.forma === "crediario") {
                                    campoParcelas.texto = "1"
                                    l.campo.texto = ""
                                    receber.atualizar()
                                    l.campo.texto = receber.calc ? receber.calc.falta : ""
                                    receber.atualizar()
                                }
                            }
                        }
                    }
                }
            }
            // Vale-troca (só quando há vale com saldo)
            Linha {
                visible: tela.vales.length > 0
                spacing: 12
                Layout.fillWidth: true
                Botao {
                    text: "Vale-troca"
                    tipo: "secundario"
                    Layout.preferredWidth: 220
                    onClicked: receber.completarVale()
                }
                Selecao {
                    id: escolhaVale
                    modelo: tela.vales.map(v => v.rotulo)
                    Layout.fillWidth: true
                    onEscolhido: receber.atualizar()
                }
                Campo {
                    id: campoVale
                    prefixo: "R$"
                    numerico: true
                    maximo: 14
                    Layout.preferredWidth: 150
                    onEditado: receber.atualizar()
                    onConfirmado: botaoConcluir.clicked()
                }
            }
            Repeater {
                id: repFormas
                model: tela.formas
                delegate: Linha {
                    required property var modelData
                    readonly property string forma: modelData.forma
                    property alias campo: valor
                    spacing: 12
                    Layout.fillWidth: true
                    Botao {
                        text: modelData.nome + (modelData.disponivel ? "" : " · em breve")
                        tipo: "secundario"
                        enabled: modelData.disponivel
                        Layout.preferredWidth: 220
                        onClicked: receber.completar(parent)
                    }
                    Campo {
                        id: valor
                        prefixo: "R$"
                        numerico: true
                        maximo: 14
                        somenteLeitura: !modelData.disponivel
                        Layout.fillWidth: true
                        onEditado: receber.atualizar()
                        onConfirmado: botaoConcluir.clicked()
                    }
                }
            }
            // Crediário: parcelas e vencimentos
            Rectangle {
                id: painelCred
                readonly property var cred: receber.calc ? receber.calc.crediario : null
                visible: !!cred && cred.valor !== "0,00"
                Layout.fillWidth: true
                radius: Tema.raioCampo
                color: Tema.azul
                implicitHeight: colunaCred.implicitHeight + 24
                ColumnLayout {
                    id: colunaCred
                    x: 14
                    y: 12
                    width: parent.width - 28
                    spacing: 8
                    RowLayout {
                        spacing: 10
                        Layout.fillWidth: true
                        Rotulo { text: "Crediário"; Layout.fillWidth: true }
                        Pilulas {
                            pequenas: true
                            modelo: [{ id: "1", nome: "30" }, { id: "2", nome: "30/60" }, { id: "3", nome: "30/60/90" }]
                            atual: campoVencimento.texto === "" ? campoParcelas.texto : ""
                            onEscolhido: id => {
                                campoParcelas.texto = id
                                campoVencimento.texto = ""
                                receber.atualizar()
                            }
                        }
                    }
                    Linha {
                        spacing: 12
                        Layout.fillWidth: true
                        Campo {
                            id: campoParcelas
                            rotulo: "Parcelas"
                            sufixo: "×"
                            numerico: true
                            maximo: 2
                            Layout.preferredWidth: 110
                            onEditado: receber.atualizar()
                        }
                        Campo {
                            id: campoVencimento
                            rotulo: "1º vencimento"
                            dica: "Vazio: daqui a 30 dias (ou o dia do convênio)"
                            maximo: 10
                            Layout.fillWidth: true
                            onEditado: receber.atualizar()
                        }
                    }
                    Flow {
                        spacing: 14
                        Layout.fillWidth: true
                        Repeater {
                            model: painelCred.cred ? painelCred.cred.parcelas : []
                            delegate: Text {
                                required property var modelData
                                text: modelData.numero + "ª  " + modelData.vencimento + "  R$ " + modelData.valor
                                color: Tema.tinta
                                font.family: Tema.texto
                                font.pixelSize: 14
                            }
                        }
                    }
                    Repeater {
                        model: painelCred.cred ? painelCred.cred.avisos : []
                        delegate: Text {
                            required property var modelData
                            text: "⚠ " + modelData
                            color: Tema.vermelho
                            font.family: Tema.texto
                            font.weight: Font.DemiBold
                            font.pixelSize: 14
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                    }
                    Text {
                        visible: text !== ""
                        text: painelCred.cred ? painelCred.cred.erro : ""
                        color: Tema.vermelho
                        font.family: Tema.texto
                        font.weight: Font.DemiBold
                        font.pixelSize: 14
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                }
            }
            PainelCheques {
                id: painelCheque
                calc: receber.calc ? receber.calc.cheques : null
                Layout.fillWidth: true
                onMudou: receber.atualizar()
            }
            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 6
                Text {
                    readonly property bool falta: !!receber.calc && receber.calc.falta !== "0,00"
                    text: !receber.calc ? "" : falta ? "Falta R$ " + receber.calc.falta
                          : receber.calc.troco !== "0,00" ? "Troco R$ " + receber.calc.troco : "Valor certo"
                    color: falta ? Tema.vermelho : Tema.verdeEscuro
                    font.family: Tema.texto
                    font.weight: Font.Bold
                    font.pixelSize: 30
                    Layout.fillWidth: true
                }
            }
            Text {
                visible: receber.erro !== ""
                text: receber.erro
                color: Tema.vermelho
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 15
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
            Linha {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao {
                    text: "Voltar"
                    tipo: "discreto"
                    atalho: "Esc"
                    onClicked: receber.close()
                }
                Botao {
                    id: botaoConcluir
                    text: "Concluir venda"
                    icone: "check"
                    atalho: "Enter"
                    onClicked: tela.concluir(receber.pagamentos())
                }
            }
        }
        onClosed: busca.focar()
    }

    // ---------- Venda concluída ----------
    Popup {
        id: concluida
        modal: true
        focus: true
        anchors.centerIn: T.Overlay.overlay
        width: Math.min(520, Tema.largura - 24)
        padding: 32
        closePolicy: Popup.CloseOnEscape
        T.Overlay.modal: Rectangle { color: Qt.rgba(0.16, 0.15, 0.13, 0.35) }
        background: Rectangle { color: Tema.verde; radius: Tema.raio + 3 }
        contentItem: ColumnLayout {
            spacing: 12
            Titulo {
                text: tela.ultimaVenda ? "Venda " + tela.ultimaVenda.numero + " concluída" : ""
                font.pixelSize: 34
                Layout.fillWidth: true
            }
            Text {
                visible: !!tela.ultimaVenda && tela.ultimaVenda.troco !== "0,00"
                text: tela.ultimaVenda ? "Troco R$ " + tela.ultimaVenda.troco : ""
                color: Tema.verdeEscuro
                font.family: Tema.texto
                font.weight: Font.Bold
                font.pixelSize: 44
            }
            Nota {
                text: tela.ultimaVenda ? "Total R$ " + tela.ultimaVenda.total + " · " + tela.ultimaVenda.pagamentos.map(p => p.nome).join(" + ") : ""
                font.pixelSize: 16
                Layout.fillWidth: true
            }
            Nota {
                visible: tela.encomendasNaVenda > 0
                text: tela.encomendasNaVenda === 1 ? "Encomenda anotada. Ela aparece em Estoque › Encomendas para pedir ao fornecedor."
                                                   : tela.encomendasNaVenda + " encomendas anotadas. Elas aparecem em Estoque › Encomendas."
                font.pixelSize: 16
                Layout.fillWidth: true
            }
            Nota {
                visible: !!tela.ultimaVenda && tela.ultimaVenda.parcelas.length > 0
                text: !tela.ultimaVenda || tela.ultimaVenda.parcelas.length === 0 ? ""
                      : "Crediário em " + tela.ultimaVenda.parcelas.length + "× · 1ª vence " + tela.ultimaVenda.parcelas[0].vencimento
                        + ". Imprima o comprovante para o cliente assinar."
                font.pixelSize: 16
                Layout.fillWidth: true
            }
            Linha {
                Layout.alignment: Qt.AlignRight
                Layout.topMargin: 8
                spacing: 10
                Botao {
                    text: "Imprimir comprovante"
                    tipo: "secundario"
                    onClicked: tela.imprimir("venda", tela.ultimaVenda.id)
                }
                Botao {
                    id: botaoNova
                    text: "Nova venda"
                    atalho: "Enter"
                    onClicked: concluida.close()
                }
            }
        }
        onOpened: botaoNova.forceActiveFocus()
        onClosed: busca.focar()
    }

    // ---------- Salvar orçamento ----------
    Popup {
        id: caixaOrcamento
        modal: true
        focus: true
        anchors.centerIn: T.Overlay.overlay
        width: Math.min(520, Tema.largura - 24)
        padding: Tema.estreito ? 18 : 30
        closePolicy: Popup.CloseOnEscape
        T.Overlay.modal: Rectangle { color: Qt.rgba(0.16, 0.15, 0.13, 0.35) }
        background: Rectangle { color: Tema.pergaminho; radius: Tema.raio + 3 }

        function abrirCom() {
            validade.texto = String(tela.janela.estado.validadeOrcamento || 7)
            obsOrcamento.texto = ""
            caixaOrcamento.open()
            validade.focar()
        }

        contentItem: ColumnLayout {
            spacing: 14
            Titulo { text: "Salvar como orçamento"; font.pixelSize: 32; Layout.fillWidth: true }
            Nota {
                text: (tela.clienteNome ? "Para " + tela.clienteNome + ". " : "Sem cliente (dá para escolher na faixa Cliente). ")
                      + "O orçamento não mexe no estoque; depois ele vira venda com um clique."
                Layout.fillWidth: true
            }
            Linha {
                spacing: 10
                Campo {
                    id: validade
                    rotulo: "Válido por"
                    sufixo: "dias"
                    numerico: true
                    maximo: 3
                    Layout.preferredWidth: 140
                }
            }
            AreaTexto {
                id: obsOrcamento
                rotulo: "Observações"
                dica: "Ex.: prazo de entrega, peça sob encomenda"
                Layout.fillWidth: true
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                Botao { text: "Cancelar"; tipo: "discreto"; onClicked: caixaOrcamento.close() }
                Botao {
                    text: "Salvar e imprimir"
                    icone: "check"
                    onClicked: tela.salvarOrcamento(parseInt(validade.texto) || 0, obsOrcamento.texto)
                }
            }
        }
        onClosed: busca.focar()
    }

    StackView.onActivated: {
        carregarCaixa()
        const f = Api.chamar("formas_pagamento")
        if (f.ok)
            formas = f.dados
        if (encomendaId !== "") {
            const r = Api.chamar("encomenda_para_venda", { id: encomendaId })
            encomendaId = ""
            if (r.ok) {
                carrinho = r.dados.itens
                clienteId = r.dados.clienteId
                clienteNome = r.dados.clienteNome
                encomendaCarregada = r.dados.encomendaId
                encomendaNumero = r.dados.numero
                orcamentoCarregado = ""
            } else {
                janela.avisar(r.erro, "erro")
            }
        }
        if (orcamentoId !== "") {
            const r = Api.chamar("orcamento_carregar", { id: orcamentoId })
            orcamentoId = ""
            if (r.ok) {
                carrinho = r.dados.itens
                clienteId = r.dados.clienteId
                clienteNome = r.dados.clienteNome
                descontoTotal.texto = r.dados.desconto
                orcamentoCarregado = r.dados.orcamentoId
            } else {
                janela.avisar(r.erro, "erro")
            }
        }
        if (osId !== "") {
            const r = Api.chamar("os_para_venda", { id: osId })
            osId = ""
            if (r.ok) {
                carrinho = r.dados.itens
                clienteId = r.dados.clienteId
                clienteNome = r.dados.clienteNome
                descontoTotal.texto = r.dados.desconto
                osCarregada = r.dados.osId
                osNumero = r.dados.numero
                orcamentoCarregado = ""
                encomendaCarregada = ""
            } else {
                janela.avisar(r.erro, "erro")
            }
        }
        if (valeId !== "") {
            const rv = Api.chamar("vales_abertos", { id: "" })
            const achou = rv.ok ? rv.dados.find(v => v.id === valeId) : null
            valeId = ""
            if (achou) {
                valeTroca = achou
                if (achou.clienteId && carrinho.length === 0) {
                    clienteId = achou.clienteId
                    clienteNome = achou.cliente
                }
            }
        }
        recalcular()
        if (clienteId && calculo)
            campoCliente.texto = calculo.cliente
        else if (clienteNome)
            campoCliente.texto = clienteNome
        busca.focar()
    }
}
