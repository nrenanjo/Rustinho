import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Templates as T
import QtQuick.Layouts
import rustinho

// Receber: quem deve no crediário, as parcelas de cada um e o pagamento.
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    // Cliente a abrir direto (vindo de outra tela).
    property string clienteInicial: ""

    property var painel: null
    property var conta: null          // conta do cliente escolhido
    property string clienteId: ""
    property var marcadas: []         // ids das parcelas escolhidas
    property bool todas: false        // mostrar pagas e canceladas
    property var formas: []
    property bool buscou: false

    function carregarPainel() {
        const r = Api.chamar("receber_painel", { texto: busca.texto })
        if (r.ok) {
            painel = r.dados
            const i = painel.clientes.findIndex(c => c.clienteId === clienteId)
            lista.currentIndex = i
        } else {
            janela.avisar(r.erro, "erro")
        }
        buscou = true
    }

    function abrirCliente(id) {
        clienteId = id
        marcadas = []
        carregarConta()
        const i = painel ? painel.clientes.findIndex(c => c.clienteId === id) : -1
        lista.currentIndex = i
    }

    function carregarConta() {
        if (!clienteId) {
            conta = null
            return
        }
        const r = Api.chamar("receber_cliente", { id: clienteId, todas: todas })
        if (r.ok) {
            conta = r.dados
            // Tira da escolha o que não está mais em aberto.
            marcadas = marcadas.filter(id => conta.parcelas.some(p => p.id === id && p.situacao !== "paga" && p.situacao !== "cancelada"))
        } else {
            janela.avisar(r.erro, "erro")
        }
    }

    function atualizarTudo() {
        carregarPainel()
        carregarConta()
    }

    function marcar(id, sim) {
        const m = marcadas.filter(x => x !== id)
        if (sim)
            m.push(id)
        marcadas = m
    }

    readonly property var abertas: conta ? conta.parcelas.filter(p => p.situacao !== "paga" && p.situacao !== "cancelada") : []
    readonly property bool todasMarcadas: abertas.length > 0 && abertas.every(p => marcadas.includes(p.id))

    function imprimir(tipo, id) {
        const r = JSON.parse(Sistema.imprimir(tipo, id))
        if (!r.ok)
            janela.avisar(r.erro, "erro")
        else if (r.dados.impresso)
            janela.avisar("Enviado para a impressora.", "ok")
    }

    function aoEsc() {
        if (busca.texto !== "") {
            busca.texto = ""
            carregarPainel()
        } else {
            janela.voltar()
        }
    }

    Timer { id: espera; interval: 200; onTriggered: tela.carregarPainel() }

    Keys.onDownPressed: if (painel && lista.currentIndex < painel.clientes.length - 1) abrirCliente(painel.clientes[lista.currentIndex + 1].clienteId)
    Keys.onUpPressed: if (painel && lista.currentIndex > 0) abrirCliente(painel.clientes[lista.currentIndex - 1].clienteId)

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        spacing: 14

        Cabecalho {
            sensivel: true
            titulo: "Receber"
            subtitulo: "Crediário e convênio: quem deve, quanto e quando vence"
            Layout.fillWidth: true
            onVoltar: tela.janela.voltar()
            Botao {
                text: "Lançar dívida"
                icone: "mais"
                tipo: "secundario"
                onClicked: lancar.abrirCom()
            }
        }

        // Números do crediário
        RowLayout {
            spacing: 12
            Layout.fillWidth: true
            Repeater {
                model: [
                    ["Total a receber", tela.painel ? tela.painel.total : "", Tema.pergaminho, Tema.tinta],
                    ["Vencido", tela.painel ? tela.painel.vencido : "", Tema.rosa,
                     tela.painel && tela.painel.vencido !== "0,00" ? Tema.vermelho : Tema.tinta],
                    ["Vence hoje", tela.painel ? tela.painel.venceHoje : "", Tema.azul, Tema.tinta],
                    ["Próximos 7 dias", tela.painel ? tela.painel.vence7Dias : "", Tema.verde, Tema.verdeEscuro]
                ]
                delegate: Rectangle {
                    required property var modelData
                    radius: Tema.raio
                    color: modelData[2]
                    implicitHeight: 72
                    Layout.fillWidth: true
                    ColumnLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 18
                        anchors.topMargin: 10
                        anchors.bottomMargin: 10
                        spacing: 0
                        Rotulo { text: modelData[0] }
                        Text {
                            text: "R$ " + modelData[1]
                            color: modelData[3]
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: 28
                        }
                    }
                }
            }
        }

        RowLayout {
            spacing: 18
            Layout.fillWidth: true
            Layout.fillHeight: true

            // Quem deve
            ColumnLayout {
                spacing: 10
                Layout.preferredWidth: 400
                Layout.maximumWidth: 460
                Layout.fillHeight: true
                Campo {
                    id: busca
                    icone: "busca"
                    altura: 48
                    tamanhoFonte: 17
                    dica: "Procurar cliente"
                    Layout.fillWidth: true
                    onEditado: espera.restart()
                    onConfirmado: if (tela.painel && tela.painel.clientes.length > 0) tela.abrirCliente(tela.painel.clientes[Math.max(0, lista.currentIndex)].clienteId)
                }
                ListView {
                    id: lista
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 6
                    model: tela.painel ? tela.painel.clientes : []
                    currentIndex: -1
                    highlightMoveDuration: 0
                    keyNavigationEnabled: false
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar {}
                    delegate: LinhaDevedor {
                        required property var modelData
                        required property int index
                        width: lista.width
                        item: modelData
                        indice: index
                        atual: modelData.clienteId === tela.clienteId
                        onAbrir: tela.abrirCliente(modelData.clienteId)
                    }
                    ColumnLayout {
                        anchors.centerIn: parent
                        width: parent.width - 40
                        visible: tela.buscou && lista.count === 0
                        spacing: 8
                        Titulo {
                            text: busca.texto === "" ? "Ninguém devendo" : "Nada encontrado"
                            font.pixelSize: 28
                            horizontalAlignment: Text.AlignHCenter
                            Layout.fillWidth: true
                        }
                        Nota {
                            text: busca.texto === "" ? "Venda no crediário (na tela Vender, forma “Crediário”) ou use “Lançar dívida” para quem já devia antes."
                                                     : "Só aparecem clientes com parcela em aberto."
                            horizontalAlignment: Text.AlignHCenter
                            Layout.fillWidth: true
                        }
                    }
                }
            }

            // A conta do cliente
            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                radius: Tema.raio
                color: Qt.rgba(1, 1, 1, 0.28)

                ColumnLayout {
                    anchors.centerIn: parent
                    width: parent.width - 80
                    visible: !tela.conta
                    spacing: 8
                    Titulo {
                        text: "Escolha um cliente"
                        font.pixelSize: 30
                        horizontalAlignment: Text.AlignHCenter
                        Layout.fillWidth: true
                    }
                    Nota {
                        text: "Clique num nome à esquerda para ver as parcelas e receber."
                        horizontalAlignment: Text.AlignHCenter
                        Layout.fillWidth: true
                    }
                }

                ColumnLayout {
                    visible: !!tela.conta
                    anchors.fill: parent
                    anchors.margins: 20
                    spacing: 12

                    RowLayout {
                        spacing: 12
                        Layout.fillWidth: true
                        ColumnLayout {
                            spacing: 0
                            Layout.fillWidth: true
                            Titulo {
                                text: tela.conta ? tela.conta.nome : ""
                                font.pixelSize: 32
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                            Nota {
                                text: !tela.conta ? "" : [tela.conta.telefone, Tema.sigilo(tela.conta.documento),
                                      tela.conta.situacao.limite ? "limite R$ " + tela.conta.situacao.limite + " (livre R$ " + tela.conta.situacao.disponivel + ")" : "sem limite de crédito",
                                      tela.conta.situacao.diaVencimento ? "convênio: vence dia " + tela.conta.situacao.diaVencimento : ""].filter(x => !!x).join("  ·  ")
                                font.pixelSize: 14
                                Layout.fillWidth: true
                            }
                        }
                        ColumnLayout {
                            spacing: 0
                            Rotulo { text: "Deve"; Layout.alignment: Qt.AlignRight }
                            Text {
                                text: tela.conta ? "R$ " + tela.conta.situacao.emAberto : ""
                                color: tela.conta && tela.conta.situacao.vencido !== "0,00" ? Tema.vermelho : Tema.tinta
                                font.family: Tema.titulo
                                font.weight: Font.Bold
                                font.pixelSize: 30
                                Layout.alignment: Qt.AlignRight
                            }
                        }
                    }

                    RowLayout {
                        spacing: 10
                        Layout.fillWidth: true
                        Marcar {
                            text: "Todas em aberto"
                            visible: tela.abertas.length > 0
                            checked: tela.todasMarcadas
                            onToggled: tela.marcadas = checked ? tela.abertas.map(p => p.id) : []
                        }
                        Item { Layout.fillWidth: true }
                        Marcar {
                            text: "Mostrar pagas"
                            checked: tela.todas
                            onToggled: {
                                tela.todas = checked
                                tela.carregarConta()
                            }
                        }
                        Botao {
                            text: "Extrato"
                            tipo: "discreto"
                            onClicked: tela.imprimir("extrato", tela.clienteId)
                        }
                        Botao {
                            text: "Lançar dívida"
                            tipo: "discreto"
                            onClicked: lancar.abrirCom(tela.conta)
                        }
                    }

                    ListView {
                        id: listaParcelas
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true
                        spacing: 5
                        model: tela.conta ? tela.conta.parcelas : []
                        boundsBehavior: Flickable.StopAtBounds
                        ScrollBar.vertical: ScrollBar {}
                        delegate: LinhaParcela {
                            required property var modelData
                            required property int index
                            width: listaParcelas.width
                            item: modelData
                            indice: index
                            marcada: tela.marcadas.includes(modelData.id)
                            onMarcar: sim => tela.marcar(modelData.id, sim)
                            onAbrirVenda: tela.janela.abrir("TelaVenda", { vendaId: modelData.vendaId })
                            onCancelar: motivo.perguntar("Tirar esta dívida?", modelData.descricao + " · R$ " + modelData.valor,
                                                         "Tirar", id => Api.chamar("conta_cancelar", { id: modelData.id, motivo: id }))
                        }
                        Nota {
                            anchors.centerIn: parent
                            visible: listaParcelas.count === 0
                            text: "Nada em aberto."
                            font.pixelSize: 16
                        }

                        // Recebimentos (embaixo das parcelas)
                        footer: ColumnLayout {
                            width: listaParcelas.width
                            spacing: 5
                            visible: !!tela.conta && tela.conta.recebimentos.length > 0
                            Rotulo { text: "Pagamentos"; Layout.topMargin: 16 }
                            Repeater {
                                model: tela.conta ? tela.conta.recebimentos : []
                                delegate: Rectangle {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    radius: Tema.raioCampo
                                    color: Qt.rgba(1, 1, 1, 0.45)
                                    implicitHeight: 46
                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.leftMargin: 14
                                        anchors.rightMargin: 10
                                        spacing: 12
                                        opacity: modelData.estornado ? 0.55 : 1
                                        Text {
                                            text: modelData.quando + "  ·  " + modelData.forma + "  ·  " + modelData.caixa + " nº " + modelData.numero
                                                  + (modelData.estornado ? "  ·  ESTORNADO: " + modelData.motivoEstorno : "")
                                            color: Tema.tintaSuave
                                            font.family: Tema.texto
                                            font.pixelSize: 13
                                            elide: Text.ElideRight
                                            Layout.fillWidth: true
                                        }
                                        Text {
                                            text: "R$ " + modelData.valor
                                            color: Tema.tinta
                                            font.family: Tema.texto
                                            font.weight: Font.Bold
                                            font.pixelSize: 15
                                            font.strikeout: modelData.estornado
                                        }
                                        Botao {
                                            text: "Recibo"
                                            tipo: "discreto"
                                            implicitHeight: 32
                                            onClicked: tela.imprimir("recibo", modelData.id)
                                        }
                                        Botao {
                                            visible: !modelData.estornado
                                            text: "Estornar"
                                            tipo: "discreto"
                                            implicitHeight: 32
                                            onClicked: motivo.perguntar("Estornar o pagamento?",
                                                "R$ " + modelData.valor + " de " + modelData.quando + ". As parcelas voltam a ficar em aberto.",
                                                "Estornar", m => Api.chamar("recebimento_estornar", { id: modelData.id, motivo: m }))
                                        }
                                    }
                                }
                            }
                        }
                    }

                    RowLayout {
                        spacing: 12
                        Layout.fillWidth: true
                        Nota {
                            text: tela.marcadas.length === 0 ? "Marque as parcelas que o cliente vai pagar (ou receba tudo)."
                                  : tela.marcadas.length + (tela.marcadas.length === 1 ? " parcela escolhida" : " parcelas escolhidas")
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }
                        Botao {
                            text: tela.marcadas.length > 0 ? "Receber escolhidas" : "Receber tudo"
                            icone: "check"
                            enabled: tela.abertas.length > 0
                            onClicked: pagar.abrirCom()
                        }
                    }
                }
            }
        }
    }

    // ---------- Receber ----------
    Popup {
        id: pagar
        property var calc: null
        property string forma: "dinheiro"
        property string erro: ""
        modal: true
        focus: true
        anchors.centerIn: T.Overlay.overlay
        width: Math.min(660, Tema.largura - 24)
        padding: Tema.estreito ? 18 : 30
        closePolicy: Popup.CloseOnEscape
        T.Overlay.modal: Rectangle { color: Qt.rgba(0.16, 0.15, 0.13, 0.35) }
        background: Rectangle { color: Tema.pergaminho; radius: Tema.raio + 3 }

        function entrada() {
            return { clienteId: tela.clienteId, contas: tela.marcadas, valor: valor.texto, juros: juros.texto,
                     desconto: desconto.texto, forma: forma, observacoes: "", cheques: chequesReceber.dados() }
        }
        function abrirCom() {
            erro = ""
            forma = "dinheiro"
            chequesReceber.limpar()
            valor.texto = ""
            juros.texto = ""
            desconto.texto = ""
            atualizar()
            if (calc) {
                juros.texto = calc.juros === "0,00" ? "" : calc.juros
                valor.texto = calc.total
            }
            atualizar()
            open()
            valor.focar()
            valor.entrada.selectAll()
        }
        function atualizar() {
            const r = Api.chamar("receber_calcular", entrada())
            if (r.ok)
                calc = r.dados
        }
        function concluir() {
            const r = Api.chamar("receber_baixar", entrada())
            if (!r.ok) {
                erro = r.erro
                return
            }
            close()
            feito.recibo = r.dados
            feito.open()
            tela.atualizarTudo()
        }

        contentItem: ColumnLayout {
            spacing: 12
            Titulo {
                text: pagar.calc ? "Receber de " + (tela.conta ? tela.conta.nome : "") : ""
                font.pixelSize: 30
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            Nota {
                text: pagar.calc ? "Deve R$ " + pagar.calc.saldo + (pagar.calc.jurosSugerido !== "0,00" ? " · juros e multa até hoje R$ " + pagar.calc.jurosSugerido : "") : ""
                font.pixelSize: 15
                Layout.fillWidth: true
            }
            RowLayout {
                spacing: 12
                Layout.fillWidth: true
                Campo {
                    id: juros
                    rotulo: "Juros e multa"
                    prefixo: "R$"
                    numerico: true
                    maximo: 12
                    Layout.fillWidth: true
                    onEditado: pagar.atualizar()
                }
                Campo {
                    id: desconto
                    rotulo: "Desconto"
                    prefixo: "R$"
                    numerico: true
                    maximo: 12
                    Layout.fillWidth: true
                    onEditado: pagar.atualizar()
                }
            }
            Campo {
                id: valor
                rotulo: "Valor pago" + (pagar.calc ? " (total R$ " + pagar.calc.total + ")" : "")
                prefixo: "R$"
                numerico: true
                maximo: 14
                altura: 52
                tamanhoFonte: 22
                Layout.fillWidth: true
                onEditado: pagar.atualizar()
                onConfirmado: pagar.concluir()
            }
            Flow {
                spacing: 8
                Layout.fillWidth: true
                Repeater {
                    model: tela.formas.filter(f => f.forma !== "crediario" && f.disponivel)
                    delegate: Botao {
                        required property var modelData
                        text: modelData.nome
                        tipo: pagar.forma === modelData.forma ? "principal" : "secundario"
                        implicitHeight: 36
                        onClicked: {
                            pagar.forma = modelData.forma
                            pagar.erro = ""
                            pagar.atualizar()
                        }
                    }
                }
            }
            PainelCheques {
                id: chequesReceber
                calc: pagar.calc && pagar.forma === "cheque" ? pagar.calc.cheques : null
                Layout.fillWidth: true
                onMudou: pagar.atualizar()
            }
            // Para onde vai o dinheiro
            Repeater {
                model: pagar.calc ? pagar.calc.abates : []
                delegate: Text {
                    required property var modelData
                    text: modelData.descricao + " · " + modelData.parcela + " (vence " + modelData.vencimento + "): R$ " + modelData.valor
                          + (modelData.resta === "0,00" ? "  → quitada" : "  → ainda falta R$ " + modelData.resta)
                    color: Tema.tintaSuave
                    font.family: Tema.texto
                    font.pixelSize: 13
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }
            Text {
                readonly property bool sobra: !!pagar.calc && pagar.calc.troco !== "0,00"
                readonly property bool resta: !!pagar.calc && pagar.calc.continuaDevendo !== "0,00"
                text: !pagar.calc ? "" : pagar.calc.erro ? pagar.calc.erro
                      : sobra ? "Troco R$ " + pagar.calc.troco
                      : resta ? "Continua devendo R$ " + pagar.calc.continuaDevendo
                      : tela.marcadas.length > 0 ? "Quita as parcelas escolhidas" : "Quita tudo"
                color: !pagar.calc || pagar.calc.erro ? Tema.vermelho : resta ? Tema.dourado : Tema.verdeEscuro
                font.family: Tema.texto
                font.weight: Font.Bold
                font.pixelSize: pagar.calc && pagar.calc.erro ? 16 : 26
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
            Text {
                visible: pagar.erro !== ""
                text: pagar.erro
                color: Tema.vermelho
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 15
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao { text: "Voltar"; tipo: "discreto"; atalho: "Esc"; onClicked: pagar.close() }
                Botao { text: "Receber"; icone: "check"; atalho: "Enter"; onClicked: pagar.concluir() }
            }
        }
        onClosed: busca.focar()
    }

    // ---------- Recebido ----------
    Popup {
        id: feito
        property var recibo: null
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
                text: feito.recibo ? "Recebido R$ " + feito.recibo.valor : ""
                font.pixelSize: 34
                Layout.fillWidth: true
            }
            Text {
                visible: !!feito.recibo && feito.recibo.troco !== "0,00"
                text: feito.recibo ? "Troco R$ " + feito.recibo.troco : ""
                color: Tema.verdeEscuro
                font.family: Tema.texto
                font.weight: Font.Bold
                font.pixelSize: 40
            }
            Nota {
                text: feito.recibo ? feito.recibo.cliente + (feito.recibo.emAberto === "0,00" ? " não deve mais nada." : " ainda deve R$ " + feito.recibo.emAberto + ".") : ""
                font.pixelSize: 16
                Layout.fillWidth: true
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                Layout.topMargin: 8
                spacing: 10
                Botao {
                    text: "Imprimir recibo"
                    tipo: "secundario"
                    onClicked: tela.imprimir("recibo", feito.recibo.id)
                }
                Botao {
                    id: botaoOk
                    text: "Pronto"
                    atalho: "Enter"
                    onClicked: feito.close()
                }
            }
        }
        onOpened: botaoOk.forceActiveFocus()
        onClosed: busca.focar()
    }

    // ---------- Lançar dívida ----------
    Popup {
        id: lancar
        property string cliente: ""
        property string clienteNome: ""
        property var achados: []
        property var previa: []
        property string erro: ""
        modal: true
        focus: true
        anchors.centerIn: T.Overlay.overlay
        width: Math.min(580, Tema.largura - 24)
        padding: Tema.estreito ? 18 : 30
        closePolicy: Popup.CloseOnEscape
        T.Overlay.modal: Rectangle { color: Qt.rgba(0.16, 0.15, 0.13, 0.35) }
        background: Rectangle { color: Tema.pergaminho; radius: Tema.raio + 3 }

        function abrirCom(conta) {
            erro = ""
            cliente = conta ? conta.clienteId : ""
            clienteNome = conta ? conta.nome : ""
            campoCliente.texto = clienteNome
            achados = []
            descricao.texto = ""
            valorDivida.texto = ""
            parcelas.texto = "1"
            vencimento.texto = ""
            previa = []
            open()
            if (cliente)
                descricao.focar()
            else
                campoCliente.focar()
        }
        function entrada() {
            return { clienteId: cliente, descricao: descricao.texto, valor: valorDivida.texto,
                     parcelas: parseInt(parcelas.texto) || 1, primeiroVencimento: vencimento.texto }
        }
        function atualizar() {
            const r = Api.chamar("receber_previa", entrada())
            previa = r.ok ? r.dados : []
            erro = r.ok ? "" : r.erro
        }
        function buscarCliente() {
            cliente = ""
            const t = campoCliente.texto.trim()
            if (t.length < 2) {
                achados = []
                return
            }
            const r = Api.chamar("pessoas_buscar", { texto: t, papel: "cliente", inativos: false, limite: 6 })
            achados = r.ok ? r.dados.itens : []
        }
        function salvar() {
            const r = Api.chamar("receber_lancar", entrada())
            if (!r.ok) {
                erro = r.erro
                return
            }
            close()
            janela.avisar("Dívida lançada.", "ok")
            tela.carregarPainel()
            tela.abrirCliente(cliente)
        }

        Timer { id: esperaCliente; interval: 200; onTriggered: lancar.buscarCliente() }

        contentItem: ColumnLayout {
            spacing: 12
            Titulo { text: "Lançar dívida"; font.pixelSize: 32; Layout.fillWidth: true }
            Nota {
                text: "Para quem já devia antes do Rustinho (caderninho, sistema antigo). Venda nova no crediário se faz na tela Vender."
                font.pixelSize: 14
                Layout.fillWidth: true
            }
            Campo {
                id: campoCliente
                rotulo: "Cliente"
                dica: "Nome, CPF ou placa"
                Layout.fillWidth: true
                onEditado: esperaCliente.restart()
                onConfirmado: if (lancar.achados.length > 0) linhasCliente.itemAt(0).escolher()
            }
            Repeater {
                id: linhasCliente
                model: lancar.cliente ? [] : lancar.achados
                delegate: Rectangle {
                    required property var modelData
                    function escolher() {
                        lancar.cliente = modelData.id
                        lancar.clienteNome = modelData.nome
                        campoCliente.texto = modelData.nome
                        lancar.achados = []
                        descricao.focar()
                    }
                    Layout.fillWidth: true
                    radius: Tema.raioCampo
                    color: sobreCliente.hovered ? Tema.azul : Qt.rgba(1, 1, 1, 0.5)
                    implicitHeight: 36
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        x: 12
                        text: modelData.nome + (modelData.documento ? "  ·  " + Tema.sigilo(modelData.documento) : "")
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.pixelSize: 15
                    }
                    HoverHandler { id: sobreCliente; cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: parent.escolher() }
                }
            }
            Campo {
                id: descricao
                rotulo: "Do que é"
                dica: "Ex.: caderno de setembro"
                maximo: 80
                Layout.fillWidth: true
                onConfirmado: valorDivida.focar()
            }
            RowLayout {
                spacing: 12
                Layout.fillWidth: true
                Campo {
                    id: valorDivida
                    rotulo: "Valor"
                    prefixo: "R$"
                    numerico: true
                    maximo: 14
                    Layout.fillWidth: true
                    onEditado: lancar.atualizar()
                }
                Campo {
                    id: parcelas
                    rotulo: "Parcelas"
                    sufixo: "×"
                    numerico: true
                    maximo: 2
                    Layout.preferredWidth: 110
                    onEditado: lancar.atualizar()
                }
                Campo {
                    id: vencimento
                    rotulo: "1º vencimento"
                    dica: "dd/mm/aaaa"
                    maximo: 10
                    Layout.preferredWidth: 160
                    onEditado: lancar.atualizar()
                    onConfirmado: lancar.salvar()
                }
            }
            Flow {
                spacing: 14
                Layout.fillWidth: true
                Repeater {
                    model: lancar.previa
                    delegate: Text {
                        required property var modelData
                        text: modelData.numero + "ª  " + modelData.vencimento + "  R$ " + modelData.valor
                        color: Tema.tintaSuave
                        font.family: Tema.texto
                        font.pixelSize: 14
                    }
                }
            }
            Text {
                visible: lancar.erro !== ""
                text: lancar.erro
                color: Tema.vermelho
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 15
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao { text: "Cancelar"; tipo: "discreto"; onClicked: lancar.close() }
                Botao { text: "Lançar"; icone: "check"; enabled: lancar.cliente !== ""; onClicked: lancar.salvar() }
            }
        }
        onClosed: busca.focar()
    }

    // ---------- Pergunta com motivo (estornar, tirar dívida) ----------
    Popup {
        id: motivo
        property string titulo: ""
        property string mensagem: ""
        property string textoBotao: ""
        property var acao: null
        property string erro: ""
        modal: true
        focus: true
        anchors.centerIn: T.Overlay.overlay
        width: Math.min(500, Tema.largura - 24)
        padding: Tema.estreito ? 18 : 30
        closePolicy: Popup.CloseOnEscape
        T.Overlay.modal: Rectangle { color: Qt.rgba(0.16, 0.15, 0.13, 0.35) }
        background: Rectangle { color: Tema.pergaminho; radius: Tema.raio + 3 }

        function perguntar(titulo, mensagem, textoBotao, acao) {
            motivo.titulo = titulo
            motivo.mensagem = mensagem
            motivo.textoBotao = textoBotao
            motivo.acao = acao
            motivo.erro = ""
            campoMotivo.texto = ""
            open()
            campoMotivo.focar()
        }
        function confirmar() {
            const r = acao(campoMotivo.texto)
            if (!r.ok) {
                erro = r.erro
                return
            }
            close()
            tela.atualizarTudo()
        }

        contentItem: ColumnLayout {
            spacing: 14
            Titulo { text: motivo.titulo; font.pixelSize: 30; wrapMode: Text.WordWrap; Layout.fillWidth: true }
            Nota { text: motivo.mensagem; font.pixelSize: 15; Layout.fillWidth: true }
            Campo {
                id: campoMotivo
                rotulo: "Motivo"
                erro: motivo.erro
                maximo: 200
                Layout.fillWidth: true
                onConfirmado: motivo.confirmar()
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao { text: "Voltar"; tipo: "discreto"; onClicked: motivo.close() }
                Botao { text: motivo.textoBotao; tipo: "perigo"; onClicked: motivo.confirmar() }
            }
        }
        onClosed: busca.focar()
    }

    StackView.onActivated: {
        const f = Api.chamar("formas_pagamento")
        if (f.ok)
            formas = f.dados
        carregarPainel()
        if (clienteInicial !== "") {
            abrirCliente(clienteInicial)
            clienteInicial = ""
        } else {
            carregarConta()
        }
        busca.focar()
    }
}
