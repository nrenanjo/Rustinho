import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Uma compra: conferir os itens da nota e lançar no estoque.
FocusScope {
    id: tela
    property string parte: "itens"
    readonly property var janela: ApplicationWindow.window
    property string compraId: ""
    property var compra: null
    // FALHA-04: chave do lançamento desta compra (nova ao carregar de novo).
    property string chaveLancar: Api.novaChave()
    property int itemEscolhendo: -1
    readonly property bool rascunho: !!compra && compra.situacao === "rascunho"

    function carregar() {
        chaveLancar = Api.novaChave()
        const r = Api.chamar("compra_abrir", { id: compraId })
        if (r.ok)
            compra = r.dados
        else {
            janela.avisar(r.erro, "erro")
            janela.voltar()
        }
    }

    // Monta o que a tela decidiu, com uma mudança opcional num item.
    function entrada(indice, mudanca) {
        const itens = compra.itens.map((it, i) => {
            const base = { id: it.id, acao: it.acao, produtoId: it.peca ? it.peca.id : "", fator: it.fator,
                           baixarPreco: it.peca ? it.peca.baixarPreco : false }
            return i === indice ? Object.assign(base, mudanca) : base
        })
        return {
            id: compra.id,
            freteExtra: freteExtra.texto,
            atualizarPrecos: atualizar.checked,
            observacoes: observacoes.texto,
            gerarPagar: gerarPagar.checked,
            itens: itens
        }
    }

    // Grava o rascunho (cada escolha já fica salva).
    function gravar(indice, mudanca) {
        const y = lista.contentY
        const r = Api.chamar("compra_atualizar", entrada(indice, mudanca))
        if (r.ok) {
            compra = r.dados
            Qt.callLater(() => lista.contentY = Math.min(y, Math.max(0, lista.contentHeight - lista.height)))
        } else
            mostrarErro(r)
        return r.ok
    }

    function mostrarErro(r) {
        if (r.campo === "freteExtra")
            freteExtra.erro = r.erro
        janela.avisar(r.erro, "erro")
    }

    function lancar() {
        if (!rascunho)
            return
        // PEND-09 / D7: sem a autorização comprovada, não lança.
        if (!compra.podeLancar) {
            janela.avisar(compra.motivoBloqueio, "erro")
            return
        }
        if (compra.pendentes > 0) {
            janela.avisar("Decida o que fazer com " + (compra.pendentes === 1 ? "o item marcado" : "os " + compra.pendentes + " itens marcados") + " em rosa.", "erro")
            const i = compra.itens.findIndex(it => it.acao === "")
            if (i >= 0)
                lista.positionViewAtIndex(i, ListView.Contain)
            return
        }
        const partes = []
        const entram = compra.itens.length - compra.ignorados
        partes.push(entram + (entram === 1 ? " item entra" : " itens entram") + " no estoque")
        if (compra.novas > 0)
            partes.push(compra.novas + (compra.novas === 1 ? " peça nova será cadastrada" : " peças novas serão cadastradas"))
        partes.push(atualizar.checked ? "os preços de revenda seguem o % de lucro de cada peça" : "os preços de revenda não mudam")
        janela.confirmar("Lançar a NF " + compra.numero + "?", partes.join("; ") + ".", "Lançar no estoque", () => {
            // 0915-N / D-09: se a compra muda o preço de venda de uma peça
            // cadastrada, pede a senha do gerente (com estes dados).
            janela.chamarGerente("compra_lancar", entrada(-1, {}), r => {
                if (r.ok) {
                    compra = r.dados.compra
                    const s = r.dados.resumo
                    janela.avisar("Compra lançada: " + s.itensEstoque + " itens no estoque"
                                  + (s.pecasNovas > 0 ? ", " + s.pecasNovas + " peças novas" : "")
                                  + (s.precosAtualizados > 0 ? ", " + s.precosAtualizados + " preços atualizados" : "")
                                  + (s.parcelasPagar > 0 ? ", " + s.parcelasPagar + (s.parcelasPagar === 1 ? " parcela" : " parcelas") + " no A pagar" : "") + ".", "ok")
                } else {
                    mostrarErro(r)
                }
            }, chaveLancar)
        })
    }

    function abrirConferencia() {
        conferencia.situacao = "autorizada"
        protocoloConferido.texto = ""
        protocoloConferido.erro = ""
        quandoConferido.texto = Qt.formatDateTime(new Date(), "dd/MM/yyyy HH:mm")
        quandoConferido.erro = ""
        senhaConferencia.texto = ""
        senhaConferencia.erro = ""
        conferencia.open()
        protocoloConferido.focar()
    }

    function registrarConferencia() {
        const r = Api.chamar("compra_conferir_autorizacao", {
            id: compra.id, situacao: conferencia.situacao, protocolo: protocoloConferido.texto,
            conferidoEm: quandoConferido.texto, senha: senhaConferencia.texto
        })
        if (r.ok) {
            compra = r.dados
            conferencia.close()
            janela.avisar(conferencia.situacao === "autorizada" ? "Conferência registrada. A compra já pode ser lançada."
                                                                : "Conferência registrada: esta nota não pode ser lançada.", "ok")
            return
        }
        if (r.campo === "protocolo") {
            protocoloConferido.erro = r.erro
            protocoloConferido.focar()
        } else if (r.campo === "conferidoEm") {
            quandoConferido.erro = r.erro
            quandoConferido.focar()
        } else if (r.campo === "senha") {
            senhaConferencia.erro = r.erro
            senhaConferencia.focar()
            senhaConferencia.entrada.selectAll()
        } else {
            janela.avisar(r.erro, "erro")
        }
    }

    function estornar() {
        motivoEstorno.texto = ""
        motivoEstorno.erro = ""
        caixaEstorno.open()
    }

    function descartar() {
        janela.confirmar("Descartar esta importação?", "Nada foi para o estoque ainda. Você pode importar o XML de novo depois.",
                         "Descartar", () => {
            const r = Api.chamar("compra_descartar", { id: compra.id })
            if (r.ok) {
                janela.avisar("Importação descartada.", "ok")
                janela.voltar()
            } else {
                janela.avisar(r.erro, "erro")
            }
        }, "perigo")
    }

    function primeiraPalavra(texto) {
        const p = (texto || "").split(" ").filter(x => x.length > 2)
        return p.length > 0 ? p[0].toLowerCase() : ""
    }

    Shortcut {
        sequence: "F2"
        enabled: tela.StackView.status === StackView.Active && tela.rascunho
        onActivated: tela.lancar()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        anchors.bottomMargin: 0
        spacing: 12

        Cabecalho {
            sensivel: true
            textoVoltar: "Compras"
            titulo: tela.compra ? "NF " + tela.compra.numero + " · " + tela.compra.fornecedor : ""
            subtitulo: {
                if (!tela.compra)
                    return ""
                const c = tela.compra
                let s = "Emitida em " + c.emissao + " · " + c.itens.length + (c.itens.length === 1 ? " item" : " itens") + " · R$ " + Tema.valor(c.total)
                if (c.situacao === "lancada")
                    s += " · lançada" + (c.lancadaPor ? " por " + c.lancadaPor : "") + " em " + Qt.formatDateTime(new Date(c.lancadaEm), "dd/MM/yyyy 'às' HH:mm")
                else if (c.situacao === "estornada")
                    s += " · estornada"
                return s
            }
            Layout.fillWidth: true
            onVoltar: tela.janela.voltar()
            Botao {
                visible: tela.rascunho
                text: "Descartar"
                tipo: "discreto"
                onClicked: tela.descartar()
            }
            Botao {
                visible: !!tela.compra && tela.compra.situacao === "lancada"
                text: "Estornar"
                tipo: "discreto"
                onClicked: tela.estornar()
            }
            Botao {
                visible: !!tela.compra && tela.compra.situacao === "lancada"
                text: "Etiquetas"
                icone: "impressora"
                tipo: "secundario"
                onClicked: tela.janela.abrir("TelaEtiquetas", { compraId: tela.compraId })
            }
            Botao {
                visible: tela.rascunho
                text: "Lançar no estoque"
                icone: "check"
                atalho: "F2"
                onClicked: tela.lancar()
            }
        }

        // Celular: uma parte de cada vez, escolhida nas pílulas.
        Pilulas {
            visible: Tema.estreito && (true)
            modelo: [{ id: "itens", nome: "Itens" }, { id: "nota", nome: "Nota e custos" }]
            atual: tela.parte
            onEscolhido: id => tela.parte = id
        }

        RowLayout {
            spacing: 18
            Layout.fillWidth: true
            Layout.fillHeight: true

            // Itens
            ListView {
                visible: !Tema.estreito || tela.parte === "itens"
                id: lista
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 8
                model: tela.compra ? tela.compra.itens : []
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar {}
                header: Item {
                    width: lista.width
                    height: (avisoTopo.visible ? avisoTopo.height + 10 : 0) + (avisoConferencia.visible ? avisoConferencia.height + 10 : 0)
                    // PEND-09: assinatura do emitente e autorização da SEFAZ (uma faixa só).
                    Rectangle {
                        id: avisoConferencia
                        visible: !!tela.compra && tela.compra.assinaturaTexto !== ""
                        y: avisoTopo.visible ? avisoTopo.height + 10 : 0
                        width: parent.width
                        height: faixaNota.implicitHeight + 20
                        radius: Tema.raioCampo
                        color: Tema.pergaminho
                        readonly property bool aguardando: !!tela.compra && tela.compra.autorizacao === "aguardando"
                        readonly property bool negada: !!tela.compra && tela.compra.autorizacao === "negada"
                        GridLayout {
                            id: faixaNota
                            x: 14
                            y: 10
                            width: parent.width - 28
                            columns: Tema.estreito ? 1 : 2
                            columnSpacing: 12
                            rowSpacing: 8
                            ColumnLayout {
                                spacing: 2
                                Layout.fillWidth: true
                                Text {
                                    text: tela.compra ? tela.compra.assinaturaTexto : ""
                                    color: Tema.tintaSuave
                                    font.family: Tema.texto
                                    font.pixelSize: 14
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                                Text {
                                    visible: text !== ""
                                    text: tela.compra ? tela.compra.autorizacaoTexto : ""
                                    color: avisoConferencia.negada ? Tema.vermelho : Tema.tinta
                                    font.family: Tema.texto
                                    font.weight: avisoConferencia.aguardando || avisoConferencia.negada ? Font.DemiBold : Font.Normal
                                    font.pixelSize: 14
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                            }
                            Botao {
                                visible: tela.rascunho && avisoConferencia.aguardando
                                text: "Registrar conferência…"
                                tipo: "secundario"
                                onClicked: tela.abrirConferencia()
                            }
                        }
                    }
                    Rectangle {
                        id: avisoTopo
                        visible: !!tela.compra && (tela.compra.avisoDestinatario !== "" || tela.compra.situacao === "estornada")
                        width: parent.width
                        height: textoAviso.implicitHeight + 20
                        radius: Tema.raioCampo
                        readonly property bool soLembrete: !!tela.compra && tela.compra.situacao !== "estornada"
                                                           && tela.compra.avisoDestinatario.indexOf("Preencha") === 0
                        color: soLembrete ? Tema.pergaminho : Tema.rosa
                        Text {
                            id: textoAviso
                            x: 14
                            y: 10
                            width: parent.width - 28
                            text: tela.compra ? (tela.compra.situacao === "estornada"
                                                 ? "Esta compra foi estornada: as peças saíram do estoque. O custo das peças não voltou ao valor de antes."
                                                 : tela.compra.avisoDestinatario) : ""
                            color: avisoTopo.soLembrete ? Tema.tintaSuave : Tema.vermelho
                            font.family: Tema.texto
                            font.weight: Font.DemiBold
                            font.pixelSize: 15
                            wrapMode: Text.WordWrap
                        }
                    }
                }
                delegate: LinhaItemCompra {
                    required property var modelData
                    required property int index
                    width: lista.width
                    item: modelData
                    editavel: tela.rascunho
                    onEscolherPeca: {
                        tela.itemEscolhendo = index
                        escolher.abrirCom(modelData.descricao + "  ·  cód. " + modelData.codigoFornecedor, tela.primeiraPalavra(modelData.descricao))
                    }
                    onCadastrarNova: tela.gravar(index, { acao: "nova", produtoId: "" })
                    onNaoLancar: tela.gravar(index, { acao: "ignorar", produtoId: "" })
                    onFatorMudou: (valor) => tela.gravar(index, { fator: valor })
                    onBaixarPrecoMudou: (baixar) => tela.gravar(index, { baixarPreco: baixar })
                    onAbrirPeca: (produtoId) => tela.janela.abrir("TelaPeca", { produtoId: produtoId })
                }
            }

            // Resumo
            Flickable {
                visible: !Tema.estreito || tela.parte === "nota"
                Layout.preferredWidth: Tema.estreito ? -1 : 380
                Layout.fillWidth: Tema.estreito
                Layout.fillHeight: true
                clip: true
                contentHeight: lado.implicitHeight + 20
                boundsBehavior: Flickable.StopAtBounds

                ColumnLayout {
                    id: lado
                    width: parent.width
                    spacing: 14

                    Faixa {
                        id: paraLancar
                        visible: tela.rascunho
                        // PEND-09: sem a autorização comprovada também falta algo.
                        readonly property bool falta: !!tela.compra && (tela.compra.pendentes > 0 || !tela.compra.podeLancar)
                        titulo: "Para lançar"
                        color: falta ? Tema.rosa : Tema.verde
                        corTitulo: falta ? Tema.vermelho : Tema.verdeEscuro
                        Layout.fillWidth: true
                        Text {
                            text: !tela.compra ? "" : tela.compra.pendentes > 0
                                  ? (tela.compra.pendentes === 1 ? "Falta decidir 1 item" : "Faltam decidir " + tela.compra.pendentes + " itens")
                                  : !tela.compra.podeLancar
                                    ? (tela.compra.autorizacao === "negada" ? "Não pode ser lançada" : "Falta a conferência da autorização")
                                    : "Tudo pronto"
                            color: Tema.tinta
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: 26
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                        }
                        Nota {
                            text: !tela.compra ? "" : [
                                      (tela.compra.itens.length - tela.compra.pendentes - tela.compra.novas - tela.compra.ignorados) + " ligados a peças",
                                      tela.compra.novas + " novas", tela.compra.ignorados + " fora do estoque"].join(" · ")
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }
                    }

                    Faixa {
                        titulo: "Custos"
                        color: Tema.pergaminho
                        Layout.fillWidth: true
                        Campo {
                            id: freteExtra
                            rotulo: "Frete pago à parte"
                            prefixo: "R$"
                            numerico: true
                            maximo: 14
                            dica: "Ex.: conhecimento da transportadora"
                            somenteLeitura: !tela.rascunho
                            Layout.fillWidth: true
                            onConfirmado: tela.gravar(-1, {})
                            onSaiu: if (tela.rascunho && texto !== (tela.compra ? tela.compra.freteExtra : "")) tela.gravar(-1, {})
                        }
                        Nota {
                            text: "É dividido entre as peças pelo valor de cada uma, e entra no custo."
                            font.pixelSize: 13
                            Layout.fillWidth: true
                        }
                        Marcar {
                            id: atualizar
                            enabled: tela.rascunho
                            text: "Atualizar o preço de revenda pelo % de lucro de cada peça"
                            Layout.fillWidth: true
                            onToggled: tela.gravar(-1, {})
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            Layout.topMargin: 4
                            Rotulo { text: "Custo das peças"; Layout.fillWidth: true }
                            Text {
                                text: tela.compra ? "R$ " + Tema.valor(tela.compra.custoTotal) : ""
                                color: Tema.tinta
                                font.family: Tema.texto
                                font.weight: Font.Bold
                                font.pixelSize: 18
                            }
                        }
                    }

                    Faixa {
                        titulo: "Pagamento"
                        color: Tema.verde
                        espacamento: 6
                        Layout.fillWidth: true
                        Repeater {
                            model: tela.compra ? tela.compra.duplicatas : []
                            delegate: Text {
                                required property var modelData
                                text: modelData
                                color: Tema.tinta
                                font.family: Tema.texto
                                font.pixelSize: 14
                                Layout.fillWidth: true
                            }
                        }
                        Nota {
                            visible: !!tela.compra && tela.compra.duplicatas.length === 0
                            text: "A nota não traz parcelas. Se for a prazo, lance no Financeiro (A pagar)."
                            font.pixelSize: 13
                            Layout.fillWidth: true
                        }
                        Marcar {
                            id: gerarPagar
                            visible: !!tela.compra && tela.compra.duplicatas.length > 0 && tela.rascunho
                            text: "Lançar as parcelas no A pagar"
                            Layout.fillWidth: true
                            onToggled: tela.gravar(-1, {})
                        }
                        RowLayout {
                            visible: !!tela.compra && tela.compra.parcelasPagar > 0
                            Layout.fillWidth: true
                            Nota {
                                text: tela.compra ? tela.compra.parcelasPagar + (tela.compra.parcelasPagar === 1 ? " parcela" : " parcelas") + " no A pagar" : ""
                                font.pixelSize: 14
                                Layout.fillWidth: true
                            }
                            Botao {
                                text: "Ver"
                                tipo: "discreto"
                                onClicked: tela.janela.abrir("TelaFinanceiro", {})
                            }
                        }
                    }

                    Faixa {
                        titulo: "A nota"
                        color: Tema.azul
                        espacamento: 6
                        Layout.fillWidth: true
                        Repeater {
                            model: tela.compra ? [
                                ["Fornecedor", tela.compra.fornecedor, ""],
                                ["CNPJ", tela.compra.fornecedorDocumento, "doc"],
                                ["Cidade", tela.compra.fornecedorCidade, ""],
                                ["Natureza", tela.compra.natureza, ""],
                                ["Produtos", tela.compra.totalProdutos, "R$"],
                                ["Frete", tela.compra.frete, "R$"],
                                ["IPI", tela.compra.ipi, "R$"],
                                ["ICMS-ST", tela.compra.icmsSt, "R$"],
                                ["Desconto", tela.compra.desconto, "R$"],
                                ["Total", tela.compra.total, "R$"]
                            ].filter(l => l[1] !== "" && l[1] !== "0,00")
                             .map(l => [l[0], l[2] === "R$" ? "R$ " + Tema.valor(l[1]) : l[2] === "doc" ? Tema.sigilo(l[1]) : l[1]]) : []
                            delegate: RowLayout {
                                required property var modelData
                                spacing: 10
                                Layout.fillWidth: true
                                Rotulo { text: modelData[0]; Layout.preferredWidth: 100 }
                                Text {
                                    text: modelData[1]
                                    color: Tema.tinta
                                    font.family: Tema.texto
                                    font.pixelSize: 14
                                    font.weight: modelData[0] === "Total" ? Font.Bold : Font.Normal
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                            }
                        }
                        Text {
                            visible: !!tela.compra && tela.compra.chave !== ""
                            text: tela.compra ? tela.compra.chave.replace(/(\d{4})/g, "$1 ").trim() : ""
                            color: Tema.tintaSuave
                            font.family: Tema.texto
                            font.pixelSize: 12
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                            Layout.topMargin: 4
                        }
                    }

                    Faixa {
                        titulo: "Observações"
                        color: Tema.pergaminho
                        Layout.fillWidth: true
                        AreaTexto {
                            id: observacoes
                            dica: "Ex.: faltou 1 jogo de pastilha, fornecedor vai mandar"
                            Layout.fillWidth: true
                        }
                        Botao {
                            visible: tela.rascunho
                            text: "Guardar observação"
                            tipo: "secundario"
                            onClicked: if (tela.gravar(-1, {})) tela.janela.avisar("Observação guardada.", "ok")
                        }
                    }
                }
            }
        }
    }

    EscolherPeca {
        id: escolher
        onEscolhida: (produtoId) => tela.gravar(tela.itemEscolhendo, { acao: "ligar", produtoId: produtoId })
    }

    // D7 (provisória, até o PEND-15): conferência humana do gerente no portal.
    Painel {
        id: conferencia
        property string situacao: "autorizada"
        readonly property bool bloqueia: situacao !== "autorizada"
        width: Math.min(520, Tema.largura - 24)
        contentItem: ColumnLayout {
            spacing: 14
            Titulo { text: "Conferência no portal da NF-e"; font.pixelSize: 28; wrapMode: Text.WordWrap; Layout.fillWidth: true }
            Nota {
                text: "Consulte esta chave no portal da NF-e e registre o que ele mostra. É uma conferência humana do gerente, não uma prova automática da SEFAZ."
                font.pixelSize: 15
                Layout.fillWidth: true
            }
            ColumnLayout {
                spacing: 4
                Layout.fillWidth: true
                Rotulo { text: "Chave de acesso"; color: Tema.apagado; Layout.fillWidth: true }
                // Quebra a linha no celular; dá para selecionar e copiar.
                TextEdit {
                    text: tela.compra ? tela.compra.chave.replace(/(\d{4})(?=\d)/g, "$1 ") : ""
                    readOnly: true
                    selectByMouse: true
                    color: Tema.tinta
                    font.family: Tema.texto
                    font.pixelSize: 16
                    wrapMode: TextEdit.Wrap
                    Layout.fillWidth: true
                }
            }
            Pilulas {
                modelo: [{ id: "autorizada", nome: "Autorizada" }, { id: "rejeitada", nome: "Rejeitada" },
                         { id: "cancelada", nome: "Cancelada" }, { id: "denegada", nome: "Denegada" }]
                atual: conferencia.situacao
                pequenas: true
                onEscolhido: id => conferencia.situacao = id
                Layout.fillWidth: true
            }
            Campo {
                id: protocoloConferido
                visible: !conferencia.bloqueia
                rotulo: "Protocolo de autorização"
                numerico: true
                maximo: 20
                Layout.fillWidth: true
                onConfirmado: quandoConferido.focar()
            }
            Nota {
                visible: conferencia.bloqueia
                text: "A nota ficará bloqueada: não poderá entrar no estoque nem no financeiro."
                color: Tema.vermelho
                font.pixelSize: 15
                Layout.fillWidth: true
            }
            Campo {
                id: quandoConferido
                rotulo: "Data e hora da conferência"
                dica: "dd/mm/aaaa hh:mm"
                maximo: 16
                Layout.fillWidth: true
                onConfirmado: senhaConferencia.focar()
            }
            Campo {
                id: senhaConferencia
                rotulo: "Senha do gerente"
                senha: true
                maximo: 256
                Layout.fillWidth: true
                onConfirmado: tela.registrarConferencia()
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao { text: "Voltar"; tipo: "discreto"; onClicked: conferencia.close() }
                Botao {
                    text: conferencia.bloqueia ? "Registrar e bloquear" : "Registrar"
                    icone: conferencia.bloqueia ? "" : "check"
                    tipo: conferencia.bloqueia ? "perigo" : "principal"
                    onClicked: tela.registrarConferencia()
                }
            }
        }
    }

    Popup {
        id: caixaEstorno
        modal: true
        focus: true
        anchors.centerIn: parent
        width: Math.min(480, Tema.largura - 24)
        padding: Tema.estreito ? 18 : 30
        background: Rectangle { color: Tema.pergaminho; radius: Tema.raio + 3 }
        contentItem: ColumnLayout {
            spacing: 14
            Titulo { text: "Estornar a compra?"; font.pixelSize: 30; Layout.fillWidth: true }
            Nota {
                text: "As peças desta nota saem do estoque. O custo e os preços não voltam ao que eram."
                Layout.fillWidth: true
            }
            Campo {
                id: motivoEstorno
                rotulo: "Motivo"
                dica: "Ex.: nota devolvida ao fornecedor"
                maximo: 120
                Layout.fillWidth: true
                onConfirmado: botaoEstornar.clicked()
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                Botao { text: "Cancelar"; tipo: "discreto"; onClicked: caixaEstorno.close() }
                Botao {
                    id: botaoEstornar
                    text: "Estornar"
                    tipo: "perigo"
                    onClicked: {
                        tela.janela.chamarGerente("compra_estornar", { id: tela.compra.id, motivo: motivoEstorno.texto }, r => {
                            if (r.ok) {
                                tela.compra = r.dados
                                caixaEstorno.close()
                                tela.janela.avisar("Compra estornada.", "ok")
                            } else {
                                motivoEstorno.erro = r.erro
                            }
                        })
                    }
                }
            }
        }
        onOpened: motivoEstorno.focar()
    }

    onCompraChanged: {
        if (!compra)
            return
        if (compra.situacao !== "rascunho")
            Qt.callLater(() => lista.positionViewAtBeginning())
        if (!freteExtra.entrada.activeFocus)
            freteExtra.texto = compra.freteExtra
        atualizar.checked = compra.atualizarPrecos
        gerarPagar.checked = compra.gerarPagar
        if (observacoes.texto === "")
            observacoes.texto = compra.observacoes
    }

    Component.onCompleted: carregar()
    StackView.onActivated: {
        if (compra)
            carregar()
        lista.forceActiveFocus()
    }
}
