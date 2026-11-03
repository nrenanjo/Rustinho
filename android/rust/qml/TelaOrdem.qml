import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Templates as T
import QtQuick.Layouts
import rustinho

// Uma ordem de serviço: o carro e o cliente, o que ele reclamou, as peças
// e a mão de obra. Orçamento → Em serviço → Pronta → Entregue (vira venda).
FocusScope {
    id: tela
    property string parte: "carro"
    readonly property var janela: ApplicationWindow.window
    property string osId: ""
    property var ficha: null
    // [{ tipo, produtoId, descricao, quantidade, preco, desconto }]
    property var itens: []
    property var calc: null
    property string clienteId: ""
    property var achados: []
    property var mecanicos: []
    property bool alterado: false
    property string erro: ""
    property string campoErro: ""

    readonly property bool nova: osId === ""
    readonly property string situacao: ficha ? ficha.situacao : "orcamento"
    // Rascunho (0.9.15): guardado ao sair, sem número; "Salvar" o torna orçamento.
    readonly property bool rascunho: situacao === "rascunho"
    // Chave da operação: a mesma gravação repetida (duplo clique) é recusada
    // pelo núcleo. Muda a cada gravação que deu certo.
    property string chave: Api.novaChave()
    readonly property bool travada: situacao === "entregue" || situacao === "cancelada"
    readonly property var servicosComuns: ["Mão de obra", "Troca de óleo e filtro", "Troca de pastilhas", "Alinhamento", "Balanceamento",
                                           "Revisão", "Diagnóstico", "Troca de correia dentada", "Troca de amortecedores", "Limpeza de bicos"]

    function carregar() {
        const m = Api.chamar("mecanicos")
        mecanicos = m.ok ? m.dados : []
        if (nova) {
            ficha = null
            itens = []
            recalcular()
            if (tela.janela && tela.janela.estado.usuario) {
                const i = mecanicos.findIndex(x => x.id === tela.janela.estado.usuario.id)
                mecanico.indice = i + 1
            }
            placa.focar()
            return
        }
        const r = Api.chamar("os_abrir", { id: osId })
        if (!r.ok) {
            janela.avisar(r.erro, "erro")
            janela.voltar()
            return
        }
        preencher(r.dados)
    }

    function preencher(f) {
        ficha = f
        osId = f.id
        clienteId = f.clienteId
        cliente.texto = f.cliente
        telefone.texto = f.telefone
        placa.texto = f.placa
        veiculo.texto = f.veiculo
        km.texto = f.km
        defeito.texto = f.defeito
        diagnostico.texto = f.diagnostico
        observacoes.texto = f.observacoes
        previsao.texto = f.previsao
        desconto.texto = f.descontoTexto
        mecanico.indice = Math.max(0, mecanicos.findIndex(x => x.id === f.mecanicoId) + 1)
        itens = f.calculo.itens.map(i => ({ tipo: i.tipo, produtoId: i.produtoId, descricao: i.descricao,
                                            quantidade: i.quantidade, preco: i.preco, desconto: i.desconto }))
        calc = f.calculo
        alterado = false
    }

    function entrada() {
        return {
            id: osId, clienteId: clienteId, clienteNome: clienteId ? "" : cliente.texto, telefone: telefone.texto,
            placa: placa.texto, veiculo: veiculo.texto, km: km.texto, defeito: defeito.texto, diagnostico: diagnostico.texto,
            observacoes: observacoes.texto, previsao: previsao.texto, desconto: desconto.texto,
            mecanicoId: mecanico.indice > 0 ? mecanicos[mecanico.indice - 1].id : "",
            itens: itens
        }
    }

    function recalcular() {
        const r = Api.chamar("os_calcular", entrada())
        if (r.ok)
            calc = r.dados
    }

    function mudou() {
        alterado = true
        espera.restart()
    }

    function mudarItem(i, campo, valor) {
        const lista = itens.slice()
        lista[i] = Object.assign({}, lista[i])
        lista[i][campo] = valor
        itens = lista
        mudou()
    }

    function addPeca(id) {
        itens = itens.concat([{ tipo: "peca", produtoId: id, descricao: "", quantidade: "1", preco: "", desconto: "" }])
        mudou()
        recalcular()
        // O preço vem da tabela: grava no item para poder mudar.
        const k = itens.length - 1
        if (calc && calc.itens[k])
            mudarItem(k, "preco", calc.itens[k].preco)
    }

    function addServico(descricao) {
        itens = itens.concat([{ tipo: "servico", produtoId: "", descricao: descricao || "", quantidade: "1", preco: "", desconto: "" }])
        mudou()
        Qt.callLater(() => {
            const l = listaItens.itemAtIndex(itens.length - 1)
            if (l) {
                // Com o nome pronto (botão de serviço comum), vai direto ao valor.
                if (descricao)
                    l.focarPreco()
                else
                    l.focarDescricao()
            }
        })
    }

    function tirar(i) {
        const lista = itens.slice()
        lista.splice(i, 1)
        itens = lista
        mudou()
        recalcular()
    }

    function salvar() {
        erro = ""
        campoErro = ""
        const era = nova || rascunho
        const r = Api.gravar("os_salvar", entrada(), chave)
        if (!r.ok) {
            erro = r.erro
            campoErro = r.campo || ""
            if (r.campo === "placa")
                placa.focar()
            else if (r.campo === "cliente")
                cliente.focar()
            return false
        }
        chave = Api.novaChave()
        preencher(r.dados)
        janela.avisar(era ? "OS " + r.dados.numero + (r.dados.caixa ? " (" + r.dados.caixa + ")" : "") + " aberta." : "OS salva.", "ok")
        return true
    }

    // Guarda os campos preenchidos e os itens para continuar depois (a OS
    // aparece em Abertas como "Rascunho"). Não exige cliente nem carro.
    function salvarRascunho() {
        erro = ""
        campoErro = ""
        const r = Api.gravar("os_rascunho_salvar", entrada(), chave)
        if (!r.ok) {
            if (r.campo === "previsao")
                previsao.focar()
            janela.avisar(r.erro, "erro")
            return false
        }
        chave = Api.novaChave()
        preencher(r.dados)
        janela.avisar("Rascunho guardado. Ele fica em Ordens de serviço › Abertas para continuar depois.", "ok")
        return true
    }

    function excluirRascunho() {
        janela.confirmar("Excluir este rascunho?", "O que foi preenchido e os itens serão apagados.", "Excluir rascunho", () => {
            const r = Api.gravar("os_rascunho_excluir", { id: osId })
            if (!r.ok) {
                janela.avisar(r.erro, "erro")
                return
            }
            alterado = false
            janela.avisar("Rascunho excluído.", "ok")
            janela.voltar()
        }, "perigo")
    }

    function passar(nova_) {
        if (alterado && !salvar())
            return
        const r = Api.gravar("os_situacao", { id: osId, situacao: nova_ })
        if (!r.ok) {
            janela.avisar(r.erro, "erro")
            return
        }
        preencher(r.dados)
        janela.avisar(nova_ === "aprovada" ? "Aprovada: o carro está em serviço." : nova_ === "pronta" ? "Pronta! Avise o cliente." : "Voltou.", "ok")
    }

    function entregar() {
        if ((alterado || nova) && !salvar())
            return
        janela.abrir("TelaVender", { osId: osId })
    }

    function imprimir() {
        if ((alterado || nova) && !salvar())
            return
        const r = JSON.parse(Sistema.imprimir("os", osId))
        if (!r.ok)
            janela.avisar(r.erro, "erro")
        else if (r.dados.impresso)
            janela.avisar("Enviado para a impressora.", "ok")
    }

    function whatsapp() {
        const fone = (telefone.texto || "").replace(/\D/g, "")
        if (fone.length < 10)
            return
        const carro = [veiculo.texto, placa.texto].filter(x => !!x).join(" ")
        const msg = "Olá, " + (cliente.texto.split(" ")[0] || "") + "! Aqui é da " + (tela.janela.estado.loja || "loja")
                    + ". O seu carro" + (carro ? " (" + carro + ")" : "") + " está pronto."
                    + (calc ? " Total: R$ " + calc.total + "." : "") + " Pode vir buscar quando quiser."
        Qt.openUrlExternally("https://wa.me/55" + fone + "?text=" + encodeURIComponent(msg))
    }

    function buscarCliente() {
        const t = cliente.texto.trim()
        if (t.length < 2 || clienteId) {
            achados = []
            return
        }
        const r = Api.chamar("pessoas_buscar", { texto: t, papel: "cliente", inativos: false, limite: 5 })
        achados = r.ok ? r.dados.itens : []
    }

    function escolherCliente(c) {
        clienteId = c.id
        cliente.texto = c.nome
        if (!telefone.texto)
            telefone.texto = c.telefone || ""
        achados = []
        mudou()
        defeito.focar()
    }

    // Placa conhecida: traz o carro e o cliente.
    function conferirPlaca() {
        if (placa.texto.replace(/[^0-9A-Za-z]/g, "").length < 7 || cliente.texto !== "")
            return
        const r = Api.chamar("os_placa", { texto: placa.texto })
        if (r.ok && r.dados) {
            clienteId = r.dados.clienteId
            cliente.texto = r.dados.cliente
            telefone.texto = r.dados.telefone
            if (!veiculo.texto)
                veiculo.texto = r.dados.veiculo
            placa.texto = r.dados.placa
            mudou()
            janela.avisar("Carro conhecido: " + r.dados.cliente + ".", "info")
        }
    }

    // Ao sair da Nova OS (ou de um rascunho) com algo preenchido: três
    // opções. "Salvar rascunho" guarda tudo; "Cancelar" volta ao formulário
    // sem perder nada; "Sair sem salvar" descarta o que não foi salvo.
    function aoEsc() {
        if (saida.opened)
            return
        if (alterado && (nova || rascunho))
            saida.open()
        else if (alterado && !travada)
            janela.confirmar("Sair sem salvar?", "As mudanças nesta OS ainda não foram salvas.", "Sair", () => janela.voltar(), "perigo")
        else
            janela.voltar()
    }

    Timer { id: espera; interval: 300; onTriggered: tela.recalcular() }
    Timer { id: esperaCli; interval: 200; onTriggered: tela.buscarCliente() }

    Shortcut {
        sequence: "F2"
        enabled: tela.StackView.status === StackView.Active && !tela.travada
        onActivated: tela.salvar()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        anchors.bottomMargin: Tema.margem - 10
        spacing: 12

        Cabecalho {
            textoVoltar: "Ordens de serviço"
            titulo: tela.nova ? "Nova OS" : tela.rascunho ? "Rascunho de OS" + (tela.ficha.placa ? " · " + tela.ficha.placa : "")
                    : "OS " + tela.ficha.numero + (tela.ficha.caixa ? " · " + tela.ficha.caixa : "") + (tela.ficha.placa ? " · " + tela.ficha.placa : "")
            subtitulo: !tela.ficha ? "Comece pela placa: se o carro já veio antes, o Rustinho preenche o resto."
                       : [tela.ficha.veiculo, tela.ficha.cliente, "aberta " + tela.ficha.criadoEm + (tela.ficha.criadoPor ? " por " + tela.ficha.criadoPor : ""),
                          tela.ficha.vendaNumero ? "venda " + tela.ficha.vendaNumero : ""].filter(x => !!x).join(" · ")
            Layout.fillWidth: true
            onVoltar: tela.aoEsc()
            Rotulo {
                visible: tela.alterado
                text: "Não salvo"
                color: Tema.vermelho
                Layout.alignment: Qt.AlignVCenter
                Layout.rightMargin: 6
            }
            Botao {
                visible: tela.rascunho
                text: "Excluir rascunho"
                tipo: "discreto"
                onClicked: tela.excluirRascunho()
            }
            Botao {
                visible: !tela.nova && !tela.travada && !tela.rascunho
                text: "Cancelar OS"
                tipo: "discreto"
                onClicked: pergunta.perguntar("Cancelar a OS " + tela.ficha.numero + (tela.ficha.caixa ? " (" + tela.ficha.caixa + ")" : "") + "?", "Nada sai do estoque. A OS fica na lista como cancelada.",
                                              "Cancelar OS", (m) => Api.gravar("os_cancelar", { id: tela.osId, motivo: m }))
            }
            Botao {
                text: "Imprimir"
                icone: "impressora"
                tipo: "secundario"
                onClicked: tela.imprimir()
            }
            Botao {
                visible: !tela.travada
                text: "Salvar"
                icone: "check"
                atalho: "F2"
                tipo: tela.situacao === "pronta" ? "secundario" : "principal"
                onClicked: tela.salvar()
            }
        }

        // Caminho da OS e o próximo passo
        Rectangle {
            Layout.fillWidth: true
            radius: Tema.raio
            color: tela.situacao === "pronta" ? Tema.verde : tela.situacao === "cancelada" ? Tema.rosa : Tema.pergaminho
            implicitHeight: Tema.estreito ? caminhoOs.implicitHeight + 20 : 58
            Linha {
                id: caminhoOs
                x: 16
                width: parent.width - 26
                anchors.verticalCenter: parent.verticalCenter
                spacing: 12
                Text {
                    visible: tela.rascunho
                    text: "Rascunho: salve para virar orçamento com número"
                    color: Tema.tinta
                    font.family: Tema.texto
                    font.weight: Font.DemiBold
                    font.pixelSize: 16
                }
                Etapas {
                    visible: tela.situacao !== "cancelada" && !tela.rascunho
                    modelo: [{ id: "orcamento", nome: "Orçamento" }, { id: "aprovada", nome: "Em serviço" },
                             { id: "pronta", nome: "Pronta" }, { id: "entregue", nome: "Entregue" }]
                    atual: tela.situacao
                }
                Text {
                    visible: tela.situacao === "cancelada"
                    text: "Cancelada" + (tela.ficha && tela.ficha.motivoCancelamento ? ": " + tela.ficha.motivoCancelamento : "")
                    color: Tema.vermelho
                    font.family: Tema.texto
                    font.weight: Font.Bold
                    font.pixelSize: 16
                }
                Item { Layout.fillWidth: true }
                Text {
                    visible: !!tela.ficha && tela.ficha.atrasada
                    text: "passou da previsão"
                    color: Tema.vermelho
                    font.family: Tema.serifa
                    font.italic: true
                    font.pixelSize: 14
                }
                Botao {
                    visible: tela.situacao === "aprovada" || tela.situacao === "pronta"
                    text: "Voltar etapa"
                    tipo: "discreto"
                    implicitHeight: 38
                    onClicked: tela.passar(tela.situacao === "pronta" ? "aprovada" : "orcamento")
                }
                Botao {
                    visible: tela.situacao === "pronta" && telefone.texto.replace(/\D/g, "").length >= 10
                    text: "Avisar no WhatsApp"
                    icone: "mensagem"
                    tipo: "secundario"
                    implicitHeight: 38
                    onClicked: tela.whatsapp()
                }
                Botao {
                    visible: !tela.travada
                    // Rascunho: o mesmo que a Nova OS (salva com número e aprova).
                    text: tela.situacao === "orcamento" || tela.rascunho ? "Cliente aprovou" : tela.situacao === "aprovada" ? "Ficou pronta" : "Entregar e receber"
                    icone: tela.situacao === "pronta" ? "check" : "direita"
                    implicitHeight: 40
                    enabled: tela.situacao === "pronta" || (!tela.nova && !tela.rascunho) || tela.itens.length > 0
                    onClicked: {
                        if (tela.situacao === "pronta")
                            tela.entregar()
                        else if (tela.nova || tela.rascunho) {
                            if (tela.salvar())
                                tela.passar("aprovada")
                        } else
                            tela.passar(tela.situacao === "orcamento" ? "aprovada" : "pronta")
                    }
                }
            }
        }

        TextoErro { text: tela.erro; Layout.fillWidth: true }

        // Celular: uma parte de cada vez, escolhida nas pílulas.
        Pilulas {
            visible: Tema.estreito && (true)
            modelo: [{ id: "carro", nome: "Carro e cliente" }, { id: "pecas", nome: "Peças e serviços" }]
            atual: tela.parte
            onEscolhido: id => tela.parte = id
        }

        RowLayout {
            spacing: 18
            Layout.fillWidth: true
            Layout.fillHeight: true

            // ---------- Carro, cliente e o problema ----------
            Flickable {
                visible: !Tema.estreito || tela.parte === "carro"
                Layout.preferredWidth: Tema.estreito ? -1 : 420
                Layout.fillWidth: Tema.estreito
                Layout.fillHeight: true
                contentHeight: lado.implicitHeight
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                interactive: contentHeight > height
                ColumnLayout {
                    id: lado
                    width: parent.width
                    spacing: 14
                    enabled: !tela.travada

                    Faixa {
                        titulo: "Carro e cliente"
                        color: Tema.pergaminho
                        Layout.fillWidth: true
                        Linha {
                            spacing: 10
                            Layout.fillWidth: true
                            Campo {
                                id: placa
                                rotulo: "Placa"
                                dica: "ABC1D23"
                                maiusculas: true
                                maximo: 8
                                erro: tela.campoErro === "placa" ? " " : ""
                                Layout.preferredWidth: 130
                                onEditado: tela.mudou()
                                onSaiu: tela.conferirPlaca()
                                onConfirmado: { tela.conferirPlaca(); veiculo.focar() }
                            }
                            Campo {
                                id: veiculo
                                rotulo: "Carro"
                                dica: "Gol G5 1.6 2012"
                                maximo: 60
                                Layout.fillWidth: true
                                onEditado: tela.mudou()
                                onConfirmado: km.focar()
                            }
                            Campo {
                                id: km
                                rotulo: "Km"
                                numerico: true
                                maximo: 9
                                Layout.preferredWidth: 100
                                onEditado: tela.mudou()
                                onConfirmado: cliente.focar()
                            }
                        }
                        Campo {
                            id: cliente
                            rotulo: tela.clienteId ? "Cliente (cadastrado)" : "Cliente"
                            icone: "pessoa"
                            dica: "Nome, CPF, telefone ou placa"
                            maximo: 100
                            erro: tela.campoErro === "cliente" ? " " : ""
                            Layout.fillWidth: true
                            onEditado: {
                                tela.clienteId = ""
                                tela.mudou()
                                esperaCli.restart()
                            }
                            onConfirmado: tela.achados.length > 0 ? tela.escolherCliente(tela.achados[0]) : telefone.focar()
                        }
                        Repeater {
                            model: tela.achados
                            delegate: Rectangle {
                                required property var modelData
                                Layout.fillWidth: true
                                implicitHeight: 36
                                radius: Tema.raioCampo
                                color: sobreCli.hovered ? Tema.azul : Tema.vela(0.45)
                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    x: 12
                                    width: parent.width - 24
                                    text: modelData.nome + (modelData.telefone ? "  ·  " + modelData.telefone : "") + (modelData.veiculos ? "  ·  " + modelData.veiculos : "")
                                    color: Tema.tinta
                                    font.family: Tema.texto
                                    font.pixelSize: 14
                                    elide: Text.ElideRight
                                }
                                HoverHandler { id: sobreCli; cursorShape: Qt.PointingHandCursor }
                                TapHandler { onTapped: tela.escolherCliente(modelData) }
                            }
                        }
                        Campo {
                            id: telefone
                            rotulo: "Celular (para avisar)"
                            dica: "(15) 91234-0000"
                            maximo: 20
                            Layout.fillWidth: true
                            onEditado: tela.mudou()
                            onConfirmado: defeito.focar()
                        }
                    }

                    Faixa {
                        titulo: "O que o cliente falou"
                        color: Tema.azul
                        Layout.fillWidth: true
                        AreaTexto {
                            id: defeito
                            dica: "Ex.: barulho ao frear, luz da injeção acesa"
                            Layout.fillWidth: true
                            onEditado: tela.mudou()
                        }
                        Rotulo { text: "Diagnóstico (o que o mecânico achou)" }
                        AreaTexto {
                            id: diagnostico
                            dica: "Ex.: pastilhas no fim e disco riscado"
                            Layout.fillWidth: true
                            onEditado: tela.mudou()
                        }
                    }

                    Faixa {
                        titulo: "Serviço"
                        color: Tema.verde
                        corTitulo: Tema.verdeEscuro
                        Layout.fillWidth: true
                        Linha {
                            spacing: 10
                            Layout.fillWidth: true
                            Selecao {
                                id: mecanico
                                rotulo: "Mecânico"
                                modelo: ["—"].concat(tela.mecanicos.map(m => m.nome))
                                Layout.fillWidth: true
                                onEscolhido: tela.mudou()
                            }
                            Campo {
                                id: previsao
                                rotulo: "Previsão"
                                dica: "dd/mm/aaaa"
                                maximo: 10
                                Layout.preferredWidth: 150
                                onEditado: tela.mudou()
                            }
                        }
                        Flow {
                            spacing: 6
                            Layout.fillWidth: true
                            Repeater {
                                model: [["Hoje", 0], ["Amanhã", 1], ["Em 2 dias", 2], ["Em 1 semana", 7]]
                                delegate: Rectangle {
                                    required property var modelData
                                    radius: height / 2
                                    height: 28
                                    width: rotuloPrev.implicitWidth + 22
                                    color: sobrePrev.hovered ? Tema.vela(0.75) : Tema.vela(0.45)
                                    Text {
                                        id: rotuloPrev
                                        anchors.centerIn: parent
                                        text: modelData[0]
                                        color: Tema.tintaSuave
                                        font.family: Tema.texto
                                        font.pixelSize: 13
                                    }
                                    HoverHandler { id: sobrePrev; cursorShape: Qt.PointingHandCursor }
                                    TapHandler {
                                        onTapped: {
                                            const d = new Date()
                                            d.setDate(d.getDate() + modelData[1])
                                            previsao.texto = Qt.formatDate(d, "dd/MM/yyyy")
                                            tela.mudou()
                                        }
                                    }
                                }
                            }
                        }
                        Rotulo { text: "Observações (saem na OS impressa)" }
                        AreaTexto {
                            id: observacoes
                            dica: "Ex.: cliente pediu para guardar as peças velhas"
                            Layout.fillWidth: true
                            onEditado: tela.mudou()
                        }
                    }
                }
            }

            // ---------- Peças e serviços ----------
            ColumnLayout {
                visible: !Tema.estreito || tela.parte === "pecas"
                spacing: 10
                Layout.fillWidth: true
                Layout.fillHeight: true

                Linha {
                    spacing: 10
                    Layout.fillWidth: true
                    Rotulo { text: "Peças e mão de obra"; Layout.fillWidth: true }
                    Botao {
                        visible: !tela.travada
                        text: "Peça"
                        icone: "mais"
                        tipo: "secundario"
                        implicitHeight: 38
                        onClicked: escolher.abrirCom("", "")
                    }
                    Botao {
                        visible: !tela.travada
                        text: "Serviço"
                        icone: "mais"
                        tipo: "secundario"
                        implicitHeight: 38
                        onClicked: tela.addServico("")
                    }
                }

                ListView {
                    id: listaItens
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 6
                    model: tela.itens.length
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar {}
                    delegate: Rectangle {
                        id: li
                        required property int index
                        readonly property var e: tela.itens[index] || ({})
                        readonly property var c: tela.calc && tela.calc.itens[index] ? tela.calc.itens[index] : ({})
                        readonly property bool servico: e.tipo === "servico"
                        function focarDescricao() {
                            descricaoServico.focar()
                        }
                        function focarPreco() {
                            precoItem.focar()
                        }
                        width: listaItens.width
                        implicitHeight: 74
                        radius: Tema.raio
                        color: c.erro ? Tema.rosa : servico ? Tema.verde : (index + 1) % 2 === 0 ? Tema.azul : Tema.pergaminho
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 14
                            anchors.rightMargin: 8
                            spacing: 10
                            enabled: !tela.travada
                            Chip {
                                texto: li.servico ? "serviço" : "peça"
                                color: Tema.vela(0.55)
                                corTexto: li.servico ? Tema.verdeEscuro : Tema.tintaSuave
                                Layout.alignment: Qt.AlignVCenter
                            }
                            ColumnLayout {
                                visible: !li.servico
                                spacing: 0
                                Layout.fillWidth: true
                                Text {
                                    text: li.c.descricao || ""
                                    color: Tema.tinta
                                    font.family: Tema.titulo
                                    font.weight: Font.Bold
                                    font.pixelSize: 19
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                                Text {
                                    text: ["cód. " + (li.c.codigo || ""), li.c.estoque !== undefined && li.c.estoque !== "" ? "tem " + li.c.estoque + " " + (li.c.unidade || "") : ""]
                                          .filter(x => !!x).join("  ·  ")
                                    color: li.c.falta ? Tema.vermelho : Tema.tintaSuave
                                    font.family: Tema.serifa
                                    font.italic: true
                                    font.pixelSize: 13
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                            }
                            Campo {
                                id: descricaoServico
                                visible: li.servico
                                texto: li.e.descricao || ""
                                dica: "O que vai ser feito (ex.: troca de pastilhas)"
                                sugestoes: tela.servicosComuns
                                maximo: 120
                                altura: 42
                                Layout.fillWidth: true
                                Layout.alignment: Qt.AlignVCenter
                                onEditado: tela.mudarItem(li.index, "descricao", texto)
                                onConfirmado: precoItem.focar()
                            }
                            Campo {
                                id: qtdItem
                                texto: li.e.quantidade || ""
                                numerico: true
                                maximo: 8
                                sufixo: li.servico ? "×" : (li.c.unidade || "")
                                altura: 42
                                Layout.preferredWidth: 96
                                Layout.alignment: Qt.AlignVCenter
                                onEditado: tela.mudarItem(li.index, "quantidade", texto)
                            }
                            Campo {
                                id: precoItem
                                texto: li.e.preco || ""
                                prefixo: "R$"
                                numerico: true
                                maximo: 12
                                dica: li.servico ? "valor" : ""
                                altura: 42
                                Layout.preferredWidth: 124
                                Layout.alignment: Qt.AlignVCenter
                                onEditado: tela.mudarItem(li.index, "preco", texto)
                            }
                            Campo {
                                texto: li.e.desconto || ""
                                dica: "desconto"
                                numerico: true
                                maximo: 10
                                altura: 42
                                Layout.preferredWidth: 104
                                Layout.alignment: Qt.AlignVCenter
                                onEditado: tela.mudarItem(li.index, "desconto", texto)
                            }
                            Text {
                                text: li.c.total ? "R$ " + li.c.total : ""
                                color: Tema.tinta
                                font.family: Tema.texto
                                font.weight: Font.Bold
                                font.pixelSize: 16
                                horizontalAlignment: Text.AlignRight
                                Layout.preferredWidth: 104
                            }
                            Botao {
                                visible: !tela.travada
                                tipo: "discreto"
                                icone: "lixeira"
                                text: ""
                                implicitHeight: 36
                                onClicked: tela.tirar(li.index)
                            }
                        }
                    }
                    ColumnLayout {
                        anchors.centerIn: parent
                        width: Math.min(parent.width - 40, 520)
                        visible: listaItens.count === 0
                        spacing: 8
                        Titulo {
                            text: "Peças e mão de obra"
                            font.pixelSize: 28
                            horizontalAlignment: Text.AlignHCenter
                            Layout.fillWidth: true
                        }
                        Nota {
                            text: "“+ Peça” procura no estoque; “+ Serviço” é a mão de obra (digite o que vai ser feito e o valor). "
                                  + "As peças só saem do estoque quando o carro for entregue."
                            horizontalAlignment: Text.AlignHCenter
                            Layout.fillWidth: true
                        }
                        Flow {
                            visible: !tela.travada
                            spacing: 6
                            Layout.alignment: Qt.AlignHCenter
                            Layout.preferredWidth: 480
                            Repeater {
                                model: tela.servicosComuns.slice(0, 6)
                                delegate: Rectangle {
                                    required property var modelData
                                    radius: height / 2
                                    height: 30
                                    width: rotuloServ.implicitWidth + 24
                                    color: sobreServ.hovered ? Tema.vela(0.75) : Tema.vela(0.45)
                                    Text {
                                        id: rotuloServ
                                        anchors.centerIn: parent
                                        text: "+ " + modelData
                                        color: Tema.tintaSuave
                                        font.family: Tema.texto
                                        font.pixelSize: 13
                                    }
                                    HoverHandler { id: sobreServ; cursorShape: Qt.PointingHandCursor }
                                    TapHandler { onTapped: tela.addServico(modelData) }
                                }
                            }
                        }
                    }
                }

                // Totais
                Rectangle {
                    Layout.fillWidth: true
                    radius: Tema.raio
                    color: Tema.pergaminho
                    implicitHeight: 76
                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 18
                        anchors.rightMargin: 18
                        spacing: 22
                        ColumnLayout {
                            spacing: 0
                            Rotulo { text: "Peças" }
                            Text { text: "R$ " + (tela.calc ? tela.calc.pecas : "0,00"); color: Tema.tinta; font.family: Tema.texto; font.weight: Font.DemiBold; font.pixelSize: 17 }
                        }
                        ColumnLayout {
                            spacing: 0
                            Rotulo { text: "Mão de obra" }
                            Text { text: "R$ " + (tela.calc ? tela.calc.servicos : "0,00"); color: Tema.tinta; font.family: Tema.texto; font.weight: Font.DemiBold; font.pixelSize: 17 }
                        }
                        Campo {
                            id: desconto
                            rotulo: "Desconto"
                            dica: "R$ ou %"
                            numerico: true
                            maximo: 10
                            somenteLeitura: tela.travada
                            Layout.preferredWidth: 130
                            onEditado: tela.mudou()
                        }
                        Item { Layout.fillWidth: true }
                        ColumnLayout {
                            spacing: 0
                            Rotulo { text: "Total"; Layout.alignment: Qt.AlignRight }
                            Text {
                                text: "R$ " + (tela.calc ? tela.calc.total : "0,00")
                                color: Tema.tinta
                                font.family: Tema.titulo
                                font.weight: Font.Bold
                                font.pixelSize: 32
                            }
                        }
                    }
                }
                TextoErro { text: tela.calc && tela.calc.erro && tela.alterado ? tela.calc.erro : ""; Layout.fillWidth: true }

                // Outras vezes que o carro veio
                Text {
                    visible: !!tela.ficha && tela.ficha.historico.length > 0
                    text: tela.ficha ? "Este carro já veio: " + tela.ficha.historico.slice(0, 4).map(h => "OS " + h.numero + " (" + (h.caixa ? h.caixa + ", " : "") + h.quando + ")").join(", ") : ""
                    color: Tema.tintaSuave
                    font.family: Tema.serifa
                    font.italic: true
                    font.pixelSize: 13
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }
        }
    }

    EscolherPeca {
        id: escolher
        onEscolhida: id => tela.addPeca(id)
    }

    // Janela de saída da Nova OS: exatamente três opções.
    Popup {
        id: saida
        modal: true
        focus: true
        anchors.centerIn: T.Overlay.overlay
        width: Math.min(560, Tema.largura - 24)
        padding: Tema.estreito ? 18 : 28
        closePolicy: Popup.CloseOnEscape
        T.Overlay.modal: Rectangle { color: Tema.veu }
        background: Rectangle { color: Tema.pergaminho; radius: Tema.raio + 3 }
        contentItem: ColumnLayout {
            spacing: 14
            Titulo {
                text: "Sair da OS?"
                font.pixelSize: 30
                Layout.fillWidth: true
            }
            Text {
                text: "Os campos preenchidos e os itens ainda não foram salvos. Salve como rascunho para continuar depois."
                color: Tema.tinta
                font.family: Tema.serifa
                font.pixelSize: 16
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
            Flow {
                spacing: 10
                Layout.fillWidth: true
                Layout.topMargin: 6
                layoutDirection: Qt.RightToLeft
                Botao {
                    text: "Salvar rascunho"
                    icone: "check"
                    tipo: "principal"
                    onClicked: {
                        if (tela.salvarRascunho()) {
                            saida.close()
                            tela.janela.voltar()
                        }
                    }
                }
                Botao {
                    text: "Cancelar"
                    tipo: "discreto"
                    onClicked: saida.close()
                }
                Botao {
                    text: "Sair sem salvar"
                    tipo: "perigo"
                    onClicked: {
                        saida.close()
                        tela.alterado = false
                        tela.janela.voltar()
                    }
                }
            }
        }
    }

    PerguntaMotivo {
        id: pergunta
        onFeito: {
            tela.janela.avisar("OS cancelada.", "ok")
            tela.carregar()
        }
    }

    StackView.onActivated: {
        if (!tela.nova && !tela.alterado) {
            const r = Api.chamar("os_abrir", { id: osId })
            if (r.ok)
                preencher(r.dados)
        }
    }
    Component.onCompleted: carregar()
}
