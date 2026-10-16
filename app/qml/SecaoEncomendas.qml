import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Encomendas de clientes: falta pedir → pedida → chegou (avisar no
// WhatsApp) → entregue. Cada linha mostra o próximo passo.
FocusScope {
    id: secao
    readonly property var janela: ApplicationWindow.window
    property var painel: null
    property string quais: "abertas"
    property bool buscou: false

    function carregar() {
        const r = Api.chamar("encomendas_painel", { texto: busca.texto, quais: quais })
        if (r.ok)
            painel = r.dados
        else
            janela.avisar(r.erro, "erro")
        buscou = true
    }
    function nova() {
        nova_.abrir()
    }
    function acao(comando, dados, mensagem) {
        const r = Api.chamar(comando, dados)
        if (!r.ok) {
            janela.avisar(r.erro, "erro")
            return false
        }
        if (mensagem)
            janela.avisar(mensagem, "ok")
        carregar()
        return true
    }
    function avisar(e) {
        const r = Api.chamar("encomenda_whatsapp", { id: e.id })
        if (!r.ok) {
            janela.avisar(r.erro, "erro")
            return
        }
        Qt.openUrlExternally(r.dados.url)
        carregar()
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

        RowLayout {
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

        RowLayout {
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

        ListView {
            id: lista
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
                width: lista.width
                implicitHeight: 66
                radius: Tema.raio
                color: e.situacao === "chegou" ? Tema.verde : (index + 1) % 2 === 0 ? Tema.azul : Tema.pergaminho
                HoverHandler { id: sobre }
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 18
                    anchors.rightMargin: 12
                    spacing: 14
                    opacity: linha.aberta ? 1 : 0.7
                    Item {
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
                            color: Tema.tinta
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: 20
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        Text {
                            text: ["Nº " + linha.e.numero + " · " + linha.e.cliente, linha.e.telefone, linha.e.desde,
                                   linha.e.avisado ? "avisado " + linha.e.avisadoEm : "",
                                   linha.e.situacao === "cancelada" ? linha.e.motivo : linha.e.observacoes]
                                  .filter(x => !!x).join("  ·  ")
                            color: Tema.tintaSuave
                            font.family: Tema.serifa
                            font.italic: true
                            font.pixelSize: 13
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                    }
                    Chip {
                        visible: linha.aberta
                        texto: linha.e.nomePagamento
                        color: linha.e.pagamento === "pago" ? Tema.verde : linha.e.pagamento === "falta" ? Tema.rosa : Tema.papel
                        corTexto: linha.e.pagamento === "pago" ? Tema.verdeEscuro : linha.e.pagamento === "falta" ? Tema.vermelho : Tema.tintaSuave
                    }
                    Row {
                        spacing: 4
                        Layout.preferredWidth: 330
                        layoutDirection: Qt.RightToLeft
                        // Próximo passo (sempre à vista)
                        Botao {
                            visible: linha.e.situacao === "aberta"
                            text: "Já pedi"
                            tipo: "secundario"
                            implicitHeight: 36
                            onClicked: secao.acao("encomenda_pedir", { id: linha.e.id }, "")
                        }
                        Botao {
                            visible: linha.e.situacao === "pedida"
                            text: "Chegou"
                            tipo: "secundario"
                            implicitHeight: 36
                            onClicked: secao.janela.confirmar("A peça chegou?",
                                "Quando a nota da compra é lançada pelo XML, o Rustinho marca sozinho. Marque aqui se chegou sem nota.",
                                "Chegou", () => secao.acao("encomenda_chegou", { id: linha.e.id }, ""))
                        }
                        Botao {
                            visible: linha.e.situacao === "chegou" && linha.e.podeVender
                            text: "Vender"
                            tipo: "secundario"
                            implicitHeight: 36
                            onClicked: secao.janela.abrir("TelaVender", { encomendaId: linha.e.id })
                        }
                        Botao {
                            visible: linha.e.situacao === "chegou" && linha.e.podeEntregar
                            text: "Entregar"
                            tipo: "secundario"
                            implicitHeight: 36
                            onClicked: secao.janela.confirmar("Entregar ao cliente?",
                                linha.e.pagamento === "falta" ? linha.e.nomePagamento + " no Receber. Receba antes ou depois, pelo Receber." : "A encomenda nº " + linha.e.numero + " sai da lista.",
                                "Entregar", () => secao.acao("encomenda_entregar", { id: linha.e.id }, "Encomenda entregue."))
                        }
                        Botao {
                            visible: linha.e.situacao === "chegou" && linha.e.temWhatsapp
                            text: linha.e.avisado ? "Avisar de novo" : "Avisar"
                            icone: "mensagem"
                            tipo: linha.e.avisado ? "discreto" : "principal"
                            implicitHeight: 36
                            onClicked: secao.avisar(linha.e)
                        }
                        // O resto, só com o mouse em cima
                        Botao {
                            visible: linha.aberta && linha.e.pagamento === "falta" && sobre.hovered
                            text: "Receber"
                            tipo: "discreto"
                            implicitHeight: 36
                            onClicked: secao.janela.abrir("TelaReceber", { clienteInicial: linha.e.clienteId })
                        }
                        Botao {
                            visible: linha.e.situacao === "pedida" && sobre.hovered
                            text: "Não pedi"
                            tipo: "discreto"
                            implicitHeight: 36
                            onClicked: secao.acao("encomenda_pedir", { id: linha.e.id, desfazer: true }, "")
                        }
                        Botao {
                            visible: linha.aberta && linha.e.situacao !== "chegou" && linha.e.temWhatsapp && sobre.hovered
                            icone: "mensagem"
                            tipo: "discreto"
                            implicitHeight: 36
                            implicitWidth: 40
                            leftPadding: 0
                            rightPadding: 0
                            onClicked: secao.avisar(linha.e)
                        }
                        Botao {
                            visible: linha.aberta && sobre.hovered
                            text: "Cancelar"
                            tipo: "discreto"
                            implicitHeight: 36
                            onClicked: motivo.perguntar("Cancelar a encomenda nº " + linha.e.numero + "?",
                                                        linha.e.descricao + " · " + linha.e.cliente, "Cancelar encomenda",
                                                        m => Api.chamar("encomenda_cancelar", { id: linha.e.id, motivo: m }))
                        }
                    }
                }
            }
            ColumnLayout {
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
        width: 600
        property string clienteId: ""
        property var achados: []
        property string produtoId: ""
        property string erro: ""
        property string campoErro: ""

        function abrir() {
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
            const r = Api.chamar("encomenda_criar", {
                clienteId: clienteId,
                clienteNome: clienteId ? "" : nCliente.texto,
                telefone: nTelefone.texto,
                produtoId: produtoId,
                descricao: produtoId ? "" : nPeca.texto,
                quantidade: nQtd.texto,
                observacoes: nObs.texto
            })
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
                    color: sobreCli.hovered ? Tema.azul : Qt.rgba(1, 1, 1, 0.45)
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        x: 12
                        width: parent.width - 24
                        text: modelData.nome + (modelData.telefone ? "  ·  " + modelData.telefone : "")
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
            RowLayout {
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
            RowLayout {
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
                Botao { text: "Cancelar"; tipo: "discreto"; onClicked: nova_.close() }
                Botao { text: "Anotar encomenda"; icone: "check"; onClicked: nova_.salvar() }
            }
        }
    }

    EscolherPeca {
        id: escolher
        onEscolhida: id => nova_.escolherPeca(id)
    }

    PerguntaMotivo { id: motivo; onFeito: secao.carregar() }

    Component.onCompleted: {
        carregar()
        busca.focar()
    }
}
