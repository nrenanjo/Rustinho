import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// A pagar: contas da loja (duplicatas das compras, aluguel, energia...).
FocusScope {
    id: secao
    readonly property var janela: ApplicationWindow.window
    property var painel: null
    property string quais: "abertas"
    property var opcoes: ({ formas: [], contas: [], gaveta: null, categorias: [] })
    property bool buscou: false

    function carregar() {
        const o = Api.chamar("pagar_formas")
        if (o.ok)
            opcoes = o.dados
        const r = Api.chamar("pagar_painel", { texto: busca.texto, quais: quais })
        if (r.ok)
            painel = r.dados
        else
            janela.avisar(r.erro, "erro")
        buscou = true
    }

    function lancar() {
        lancamento.abrirCom()
    }

    function aoEsc() {
        if (busca.texto !== "") {
            busca.texto = ""
            carregar()
            return true
        }
        return false
    }

    function nomesCategorias() {
        return ["Sem categoria"].concat(opcoes.categorias.filter(c => c.tipo === "despesa").map(c => c.nome))
    }
    function idCategoria(indice) {
        const lista = opcoes.categorias.filter(c => c.tipo === "despesa")
        return indice > 0 ? lista[indice - 1].id : ""
    }
    function indiceCategoria(id) {
        const lista = opcoes.categorias.filter(c => c.tipo === "despesa")
        return lista.findIndex(c => c.id === id) + 1
    }
    function hoje() {
        return Qt.formatDate(new Date(), "dd/MM/yyyy")
    }

    Timer { id: espera; interval: 200; onTriggered: secao.carregar() }

    ColumnLayout {
        anchors.fill: parent
        spacing: 12

        Linha {
            colunasEstreito: 2
            spacing: 12
            Layout.fillWidth: true
            CartaoNumero {
                rotulo: "Total a pagar"
                valor: secao.painel ? secao.painel.total : ""
            }
            CartaoNumero {
                rotulo: "Vencido"
                valor: secao.painel ? secao.painel.vencido : ""
                cor: Tema.rosa
                corValor: secao.painel && secao.painel.vencido !== "0,00" ? Tema.vermelho : Tema.tinta
            }
            CartaoNumero {
                rotulo: "Vence hoje"
                valor: secao.painel ? secao.painel.venceHoje : ""
                cor: Tema.azul
            }
            CartaoNumero {
                rotulo: "Próximos 7 dias"
                valor: secao.painel ? secao.painel.vence7Dias : ""
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
                dica: "Procurar: descrição, fornecedor, nota ou categoria"
                Layout.fillWidth: true
                onEditado: espera.restart()
            }
            Pilulas {
                pequenas: true
                modelo: [{ id: "abertas", nome: "Em aberto" }, { id: "pagas", nome: "Pagas" }, { id: "todas", nome: "Todas" }]
                atual: secao.quais
                onEscolhido: id => {
                    secao.quais = id
                    secao.carregar()
                }
            }
            Botao {
                text: "Categorias"
                tipo: "discreto"
                onClicked: categorias.abrirCom()
            }
        }

        ListView {
            id: lista
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 6
            model: secao.painel ? secao.painel.parcelas : []
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            delegate: LinhaContaPagar {
                required property var modelData
                required property int index
                width: lista.width
                item: modelData
                indice: index
                onAbrir: ficha.abrirCom(modelData.id)
                onPagar: pagamento.abrirCom(modelData)
            }
            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(parent.width - 40, 620)
                visible: secao.buscou && lista.count === 0
                spacing: 8
                Titulo {
                    text: busca.texto !== "" ? "Nada encontrado" : secao.quais === "abertas" ? "Nenhuma conta em aberto" : "Nada por aqui"
                    font.pixelSize: 30
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Nota {
                    visible: secao.quais === "abertas" && busca.texto === ""
                    text: "As duplicatas das notas de compra entram sozinhas ao lançar a compra. Aluguel, energia e o resto: “Lançar conta”."
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
            }
        }
    }

    // ---------- Lançar conta ----------
    Painel {
        id: lancamento
        property string fornecedor: ""
        property var achados: []
        property var previa: []
        property string erro: ""
        width: Math.min(620, Tema.largura - 24)

        function abrirCom() {
            erro = ""
            fornecedor = ""
            campoFornecedor.texto = ""
            achados = []
            lDescricao.texto = ""
            lCategoria.indice = 0
            lDocumento.texto = ""
            lValor.texto = ""
            lParcelas.texto = "1"
            lVencimento.texto = ""
            lCompetencia.texto = ""
            previa = []
            open()
            lDescricao.focar()
        }
        function entrada() {
            return { fornecedorId: fornecedor, categoriaId: secao.idCategoria(lCategoria.indice), descricao: lDescricao.texto,
                     documento: lDocumento.texto, valor: lValor.texto, parcelas: parseInt(lParcelas.texto) || 1,
                     primeiroVencimento: lVencimento.texto, competencia: lCompetencia.texto }
        }
        function atualizar() {
            const r = Api.chamar("pagar_previa", entrada())
            previa = r.ok ? r.dados : []
            erro = r.ok ? "" : r.erro
        }
        function buscarFornecedor() {
            fornecedor = ""
            const t = campoFornecedor.texto.trim()
            if (t.length < 2) {
                achados = []
                return
            }
            const r = Api.chamar("pessoas_buscar", { texto: t, papel: "fornecedor", inativos: false, limite: 6 })
            achados = r.ok ? r.dados.itens : []
        }
        function salvar() {
            const r = Api.chamar("pagar_lancar", entrada())
            if (!r.ok) {
                erro = r.erro
                return
            }
            close()
            janela.avisar(r.dados.length > 1 ? r.dados.length + " parcelas lançadas." : "Conta lançada.", "ok")
            secao.carregar()
        }

        Timer { id: esperaFornecedor; interval: 200; onTriggered: lancamento.buscarFornecedor() }

        contentItem: ColumnLayout {
            spacing: 12
            Titulo { text: "Lançar conta a pagar"; font.pixelSize: 32; Layout.fillWidth: true }
            Campo {
                id: lDescricao
                rotulo: "Do que é"
                dica: "Ex.: aluguel de outubro, energia, boleto da distribuidora"
                maximo: 80
                Layout.fillWidth: true
                onConfirmado: lValor.focar()
            }
            Linha {
                spacing: 12
                Layout.fillWidth: true
                Selecao {
                    id: lCategoria
                    rotulo: "Categoria"
                    modelo: secao.nomesCategorias()
                    Layout.fillWidth: true
                }
                Campo {
                    id: lDocumento
                    rotulo: "Nº da nota ou boleto"
                    dica: "opcional"
                    maximo: 40
                    Layout.preferredWidth: 200
                }
            }
            Campo {
                id: campoFornecedor
                rotulo: "Fornecedor (opcional)"
                dica: "Nome ou CNPJ"
                Layout.fillWidth: true
                onEditado: esperaFornecedor.restart()
                onConfirmado: if (lancamento.achados.length > 0) linhasFornecedor.itemAt(0).escolher()
            }
            Repeater {
                id: linhasFornecedor
                model: lancamento.fornecedor ? [] : lancamento.achados
                delegate: Rectangle {
                    required property var modelData
                    function escolher() {
                        lancamento.fornecedor = modelData.id
                        campoFornecedor.texto = modelData.nome
                        lancamento.achados = []
                        lValor.focar()
                    }
                    Layout.fillWidth: true
                    radius: Tema.raioCampo
                    color: sobreF.hovered ? Tema.azul : Tema.vela(0.5)
                    implicitHeight: 36
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        x: 12
                        text: modelData.nome + (modelData.documento ? "  ·  " + Tema.sigilo(modelData.documento) : "")
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.pixelSize: 15
                    }
                    HoverHandler { id: sobreF; cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: parent.escolher() }
                }
            }
            Linha {
                spacing: 12
                Layout.fillWidth: true
                Campo {
                    id: lValor
                    rotulo: "Valor total"
                    prefixo: "R$"
                    numerico: true
                    maximo: 14
                    Layout.fillWidth: true
                    onEditado: lancamento.atualizar()
                }
                Campo {
                    id: lParcelas
                    rotulo: "Parcelas"
                    sufixo: "×"
                    numerico: true
                    maximo: 2
                    Layout.preferredWidth: 110
                    onEditado: lancamento.atualizar()
                }
                Campo {
                    id: lVencimento
                    rotulo: "Vencimento"
                    dica: "Vazio: hoje"
                    maximo: 10
                    Layout.preferredWidth: 160
                    onEditado: lancamento.atualizar()
                    onConfirmado: lCompetencia.focar()
                }
                Campo {
                    id: lCompetencia
                    rotulo: "Mês de referência"
                    dica: "mm/aaaa"
                    maximo: 7
                    Layout.preferredWidth: 150
                    onConfirmado: lancamento.salvar()
                }
            }
            Nota {
                text: "Mês de referência: o mês a que a conta se refere (o aluguel de outubro pago em novembro é de 10/aaaa). Vazio: o mês do vencimento. Serve para o resultado (DRE) do Painel."
                font.pixelSize: 13
                Layout.fillWidth: true
            }
            Flow {
                spacing: 14
                visible: lancamento.previa.length > 1
                Layout.fillWidth: true
                Repeater {
                    model: lancamento.previa
                    delegate: Text {
                        required property var modelData
                        text: modelData.numero + "ª  " + modelData.vencimento + "  R$ " + modelData.valor
                        color: Tema.tintaSuave
                        font.family: Tema.texto
                        font.pixelSize: 14
                    }
                }
            }
            TextoErro { text: lancamento.erro }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao { text: "Cancelar"; tipo: "discreto"; onClicked: lancamento.close() }
                Botao { text: "Lançar"; icone: "check"; onClicked: lancamento.salvar() }
            }
        }
        onClosed: busca.focar()
    }

    // ---------- Pagar ----------
    Painel {
        id: pagamento
        property var conta: null
        property string forma: "boleto"
        property string erro: ""
        // ids na mesma ordem da Selecao "De onde sai"
        property var origens: []
        width: Math.min(700, Tema.largura - 24)

        function abrirCom(c) {
            conta = c
            erro = ""
            const lista = []
            const nomes = []
            if (secao.opcoes.gaveta) {
                lista.push("gaveta")
                nomes.push("Gaveta do " + secao.opcoes.gaveta + " (dinheiro)")
            }
            for (const ct of secao.opcoes.contas) {
                lista.push(ct.id)
                nomes.push(ct.nome + "  ·  saldo R$ " + ct.saldo)
            }
            origens = lista
            pOrigem.modelo = nomes
            escolherForma("boleto")
            pValor.texto = c.saldo
            pJuros.texto = ""
            pDesconto.texto = ""
            pData.texto = secao.hoje()
            pCheque.texto = ""
            open()
            pValor.focar()
        }
        function escolherForma(f) {
            forma = f
            erro = ""
            // dinheiro: gaveta (se aberta) ou Caixa geral; o resto: o primeiro banco
            let alvo = -1
            if (f === "dinheiro")
                alvo = origens.indexOf("gaveta") >= 0 ? origens.indexOf("gaveta") : origens.indexOf("caixa-geral")
            else {
                const banco = secao.opcoes.contas.find(c => c.tipo === "banco")
                alvo = banco ? origens.indexOf(banco.id) : origens.indexOf("caixa-geral")
            }
            if (alvo >= 0)
                pOrigem.indice = alvo
            if (f === "cheque")
                pCheque.focar()
        }
        readonly property string total: {
            const n = s => parseFloat((s || "0").replace(/\./g, "").replace(",", ".")) || 0
            const t = n(pValor.texto) + n(pJuros.texto) - n(pDesconto.texto)
            return t.toLocaleString(Qt.locale("pt_BR"), "f", 2)
        }
        function concluir() {
            const r = Api.chamar("pagar_pagar", {
                contaId: conta.id, valor: pValor.texto, juros: pJuros.texto, desconto: pDesconto.texto, data: pData.texto,
                forma: forma, origem: origens[pOrigem.indice] || "", chequeNumero: pCheque.texto
            })
            if (!r.ok) {
                erro = r.erro
                return
            }
            close()
            janela.avisar("Pago R$ " + total + ".", "ok")
            secao.carregar()
            ficha.recarregar()
        }

        contentItem: ColumnLayout {
            spacing: 12
            Titulo {
                text: pagamento.conta ? "Pagar " + pagamento.conta.descricao : ""
                font.pixelSize: 30
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            Nota {
                text: pagamento.conta ? "Falta R$ " + pagamento.conta.saldo + "  ·  vence " + pagamento.conta.vencimento
                                        + (pagamento.conta.diasAtraso > 0 ? " (" + pagamento.conta.diasAtraso + " dias de atraso)" : "") : ""
                font.pixelSize: 15
                Layout.fillWidth: true
            }
            Flow {
                spacing: 8
                Layout.fillWidth: true
                Repeater {
                    model: secao.opcoes.formas
                    delegate: Botao {
                        required property var modelData
                        text: modelData.nome
                        tipo: pagamento.forma === modelData.forma ? "principal" : "secundario"
                        implicitHeight: 36
                        onClicked: pagamento.escolherForma(modelData.forma)
                    }
                }
            }
            Linha {
                spacing: 12
                Layout.fillWidth: true
                Campo {
                    id: pValor
                    rotulo: "Valor (da conta)"
                    prefixo: "R$"
                    numerico: true
                    maximo: 14
                    Layout.fillWidth: true
                    onConfirmado: pagamento.concluir()
                }
                Campo {
                    id: pJuros
                    rotulo: "Juros e multa"
                    prefixo: "R$"
                    numerico: true
                    maximo: 12
                    Layout.preferredWidth: 150
                }
                Campo {
                    id: pDesconto
                    rotulo: "Desconto"
                    prefixo: "R$"
                    numerico: true
                    maximo: 12
                    Layout.preferredWidth: 150
                }
            }
            Linha {
                spacing: 12
                Layout.fillWidth: true
                Selecao {
                    id: pOrigem
                    rotulo: "De onde sai o dinheiro"
                    Layout.fillWidth: true
                }
                Campo {
                    id: pData
                    rotulo: pagamento.forma === "cheque" ? "Bom para" : "Data"
                    maximo: 10
                    Layout.preferredWidth: 150
                }
                Campo {
                    id: pCheque
                    visible: pagamento.forma === "cheque"
                    rotulo: "Nº do cheque"
                    maximo: 20
                    Layout.preferredWidth: 130
                }
            }
            Text {
                text: "Sai R$ " + pagamento.total
                color: Tema.tinta
                font.family: Tema.texto
                font.weight: Font.Bold
                font.pixelSize: 26
                Layout.fillWidth: true
            }
            TextoErro { text: pagamento.erro }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao { text: "Voltar"; tipo: "discreto"; atalho: "Esc"; onClicked: pagamento.close() }
                Botao { text: "Pagar"; icone: "check"; atalho: "Enter"; onClicked: pagamento.concluir() }
            }
        }
        onClosed: busca.focar()
    }

    // ---------- Ficha da conta ----------
    Painel {
        id: ficha
        property string contaId: ""
        property var dados: null
        width: Math.min(660, Tema.largura - 24)

        function abrirCom(id) {
            contaId = id
            recarregar()
            open()
        }
        function recarregar() {
            if (!contaId)
                return
            const r = Api.chamar("pagar_ficha", { id: contaId })
            if (r.ok)
                dados = r.dados
        }
        readonly property var p: dados ? dados.parcela : null
        readonly property bool aberta: !!p && p.situacao !== "paga" && p.situacao !== "cancelada"

        contentItem: ColumnLayout {
            spacing: 10
            Titulo {
                text: ficha.p ? ficha.p.descricao : ""
                font.pixelSize: 30
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
            Nota {
                text: !ficha.p ? "" : [ficha.p.fornecedor, ficha.p.categoria, ficha.p.documento ? "doc. " + ficha.p.documento : "",
                                       ficha.p.parcela !== "1/1" ? "parcela " + ficha.p.parcela : "",
                                       ficha.p.competencia ? "referente a " + ficha.p.competencia : ""].filter(x => !!x).join("  ·  ")
                visible: text !== ""
                font.pixelSize: 15
                Layout.fillWidth: true
            }
            RowLayout {
                spacing: 12
                Layout.fillWidth: true
                CartaoNumero { rotulo: "Vencimento"; prefixo: ""; valor: ficha.p ? ficha.p.vencimento : ""; cor: Tema.papel }
                CartaoNumero { rotulo: "Valor"; valor: ficha.p ? ficha.p.valor : ""; cor: Tema.papel }
                CartaoNumero {
                    rotulo: ficha.aberta ? "Falta" : "Pago"
                    valor: ficha.p ? (ficha.aberta ? ficha.p.saldo : ficha.p.pago) : ""
                    cor: ficha.aberta ? Tema.rosa : Tema.verde
                    corValor: ficha.aberta ? Tema.vermelho : Tema.verdeEscuro
                }
            }
            Nota {
                visible: !!ficha.p && ficha.p.observacoes !== ""
                text: ficha.p ? ficha.p.observacoes : ""
                font.pixelSize: 14
                Layout.fillWidth: true
            }
            Nota {
                visible: !!ficha.dados && ficha.dados.motivoCancelamento !== ""
                text: ficha.dados ? "Tirada: " + ficha.dados.motivoCancelamento : ""
                color: Tema.vermelho
                font.pixelSize: 14
                Layout.fillWidth: true
            }
            Rotulo { text: "Pagamentos"; visible: !!ficha.dados && ficha.dados.pagamentos.length > 0; Layout.topMargin: 6 }
            Repeater {
                model: ficha.dados ? ficha.dados.pagamentos : []
                delegate: Rectangle {
                    required property var modelData
                    Layout.fillWidth: true
                    radius: Tema.raioCampo
                    color: Tema.vela(0.5)
                    implicitHeight: 46
                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 14
                        anchors.rightMargin: 8
                        spacing: 12
                        opacity: modelData.estornado ? 0.55 : 1
                        Text {
                            text: [modelData.data, modelData.forma + (modelData.chequeNumero ? " nº " + modelData.chequeNumero : ""),
                                   modelData.origem, modelData.usuario,
                                   modelData.juros !== "0,00" ? "juros R$ " + modelData.juros : "",
                                   modelData.desconto !== "0,00" ? "desconto R$ " + modelData.desconto : "",
                                   modelData.estornado ? "ESTORNADO: " + modelData.motivoEstorno : ""].filter(x => !!x).join("  ·  ")
                            color: Tema.tintaSuave
                            font.family: Tema.texto
                            font.pixelSize: 13
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        Text {
                            text: "R$ " + modelData.total
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.Bold
                            font.pixelSize: 15
                            font.strikeout: modelData.estornado
                        }
                        Botao {
                            visible: !modelData.estornado
                            text: "Estornar"
                            tipo: "discreto"
                            implicitHeight: 32
                            // D-09: a senha do Financeiro não vale para estornar.
                            onClicked: secao.janela.comGerente("estornar", () => motivo.perguntar("Estornar o pagamento?",
                                "R$ " + modelData.total + " de " + modelData.data + ". A conta volta a ficar em aberto.",
                                "Estornar", m => Api.chamar("pagamento_estornar", { id: modelData.id, motivo: m })))
                        }
                    }
                }
            }
            Linha {
                Layout.topMargin: 8
                spacing: 10
                Layout.fillWidth: true
                Botao {
                    visible: ficha.aberta && !!ficha.dados && ficha.dados.pagamentos.every(x => x.estornado)
                    text: "Tirar"
                    tipo: "discreto"
                    onClicked: motivo.perguntar("Tirar esta conta?", ficha.p.descricao + " · R$ " + ficha.p.valor,
                                                "Tirar", m => Api.chamar("pagar_cancelar", { id: ficha.contaId, motivo: m }))
                }
                Botao {
                    visible: ficha.aberta
                    text: "Corrigir"
                    tipo: "discreto"
                    icone: "lapis"
                    onClicked: correcao.abrirCom(ficha.p)
                }
                Botao {
                    visible: !!ficha.p && ficha.p.compraId !== ""
                    text: "Ver a compra"
                    tipo: "discreto"
                    onClicked: {
                        ficha.close()
                        janela.abrir("TelaCompra", { compraId: ficha.p.compraId })
                    }
                }
                Item { Layout.fillWidth: true }
                Botao { text: "Fechar"; tipo: "discreto"; onClicked: ficha.close() }
                Botao {
                    visible: ficha.aberta
                    text: "Pagar"
                    icone: "check"
                    onClicked: pagamento.abrirCom(ficha.p)
                }
            }
        }
    }

    // ---------- Corrigir ----------
    Painel {
        id: correcao
        property string contaId: ""
        property string erro: ""
        width: Math.min(600, Tema.largura - 24)
        function abrirCom(p) {
            contaId = p.id
            erro = ""
            cDescricao.texto = p.descricao
            cCategoria.indice = secao.indiceCategoria(p.categoriaId)
            cDocumento.texto = p.documento
            cValor.texto = p.valor
            cVencimento.texto = p.vencimento
            cCompetencia.texto = p.competencia || ""
            cObs.texto = p.observacoes
            open()
            cVencimento.focar()
        }
        function salvar() {
            const r = Api.chamar("pagar_alterar", { id: contaId, descricao: cDescricao.texto, categoriaId: secao.idCategoria(cCategoria.indice),
                                                    documento: cDocumento.texto, valor: cValor.texto, vencimento: cVencimento.texto,
                                                    competencia: cCompetencia.texto, observacoes: cObs.texto })
            if (!r.ok) {
                erro = r.erro
                return
            }
            close()
            ficha.dados = r.dados
            secao.carregar()
        }
        contentItem: ColumnLayout {
            spacing: 12
            Titulo { text: "Corrigir a conta"; font.pixelSize: 30; Layout.fillWidth: true }
            Campo { id: cDescricao; rotulo: "Do que é"; maximo: 80; Layout.fillWidth: true }
            RowLayout {
                spacing: 12
                Layout.fillWidth: true
                Selecao { id: cCategoria; rotulo: "Categoria"; modelo: secao.nomesCategorias(); Layout.fillWidth: true }
                Campo { id: cDocumento; rotulo: "Nº da nota ou boleto"; maximo: 40; Layout.preferredWidth: 200 }
            }
            RowLayout {
                spacing: 12
                Layout.fillWidth: true
                Campo { id: cValor; rotulo: "Valor"; prefixo: "R$"; numerico: true; maximo: 14; Layout.fillWidth: true }
                Campo { id: cVencimento; rotulo: "Vencimento"; maximo: 10; Layout.preferredWidth: 160; onConfirmado: correcao.salvar() }
                Campo { id: cCompetencia; rotulo: "Mês de referência"; dica: "mm/aaaa"; maximo: 7; Layout.preferredWidth: 150; onConfirmado: correcao.salvar() }
            }
            Campo { id: cObs; rotulo: "Observações"; maximo: 300; Layout.fillWidth: true }
            TextoErro { text: correcao.erro }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao { text: "Cancelar"; tipo: "discreto"; onClicked: correcao.close() }
                Botao { text: "Salvar"; icone: "check"; onClicked: correcao.salvar() }
            }
        }
    }

    // ---------- Categorias ----------
    Painel {
        id: categorias
        property var itens: []
        property string erro: ""
        width: Math.min(560, Tema.largura - 24)
        function abrirCom() {
            erro = ""
            nova.texto = ""
            recarregar()
            open()
            nova.focar()
        }
        function recarregar() {
            const r = Api.chamar("categorias", { quais: "todas" })
            if (r.ok)
                itens = r.dados.filter(c => c.tipo === "despesa")
        }
        function salvar(c, nome, ativo) {
            const r = Api.chamar("categoria_salvar", { id: c ? c.id : "", nome: nome, tipo: "despesa", ativo: ativo })
            erro = r.ok ? "" : r.erro
            if (r.ok) {
                recarregar()
                secao.carregar()
            }
            return r.ok
        }
        contentItem: ColumnLayout {
            spacing: 10
            Titulo { text: "Categorias"; font.pixelSize: 30; Layout.fillWidth: true }
            Nota {
                text: "Para saber para onde vai o dinheiro. Mude o nome à vontade; desmarque as que não usa."
                font.pixelSize: 14
                Layout.fillWidth: true
            }
            ListView {
                id: listaCat
                Layout.fillWidth: true
                Layout.preferredHeight: Math.min(contentHeight, 380)
                clip: true
                spacing: 4
                model: categorias.itens
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar {}
                delegate: Linha {
                    required property var modelData
                    width: listaCat.width - 12
                    spacing: 10
                    Marcar {
                        checked: modelData.ativo
                        onToggled: categorias.salvar(modelData, nomeCat.texto, checked)
                    }
                    Campo {
                        id: nomeCat
                        texto: modelData.nome
                        maximo: 40
                        altura: 36
                        Layout.fillWidth: true
                        onSaiu: if (texto !== modelData.nome) categorias.salvar(modelData, texto, modelData.ativo)
                        onConfirmado: if (texto !== modelData.nome) categorias.salvar(modelData, texto, modelData.ativo)
                    }
                }
            }
            Linha {
                spacing: 10
                Layout.fillWidth: true
                Campo {
                    id: nova
                    dica: "Nova categoria"
                    maximo: 40
                    Layout.fillWidth: true
                    onConfirmado: if (categorias.salvar(null, texto, true)) texto = ""
                }
                Botao {
                    text: "Criar"
                    icone: "mais"
                    tipo: "secundario"
                    onClicked: if (categorias.salvar(null, nova.texto, true)) nova.texto = ""
                }
            }
            TextoErro { text: categorias.erro }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                Botao { text: "Pronto"; onClicked: categorias.close() }
            }
        }
        onClosed: busca.focar()
    }

    PerguntaMotivo {
        id: motivo
        onFeito: {
            secao.carregar()
            ficha.recarregar()
        }
    }

    Component.onCompleted: {
        carregar()
        busca.focar()
    }
}
