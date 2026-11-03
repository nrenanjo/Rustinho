import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Templates as T
import QtQuick.Layouts
import rustinho

// Devolução de uma venda: quais peças voltam (e se vieram com defeito) e
// como o valor volta ao cliente. Com vale-troca dá para trocar na hora.
FocusScope {
    id: tela
    property string parte: "pecas"
    readonly property var janela: ApplicationWindow.window
    property string vendaId: ""
    property var venda: null
    property var calc: null
    // venda_item_id → { quantidade, defeito }
    property var escolha: ({})
    property string destino: "vale"
    property string resto: "vale"
    property string erro: ""
    property var feita: null
    // FALHA-04: chave desta devolução (a mesma se repetir; nova ao carregar de novo).
    property string chaveDevolucao: Api.novaChave()

    readonly property var motivos: ["Peça errada", "Não serviu no carro", "Veio com defeito", "Desistiu da compra", "Comprou a mais"]

    function carregar() {
        chaveDevolucao = Api.novaChave()
        const r = Api.chamar("devolucao_venda", { id: vendaId })
        if (!r.ok) {
            janela.avisar(r.erro, "erro")
            janela.voltar()
            return
        }
        venda = r.dados
        recalcular()
    }

    function entrada() {
        const itens = []
        for (const id in escolha)
            itens.push({ vendaItemId: id, quantidade: escolha[id].quantidade, defeito: escolha[id].defeito })
        return { vendaId: vendaId, itens: itens, destino: destino, resto: resto, motivo: motivo.texto }
    }

    function recalcular() {
        const r = Api.chamar("devolucao_calcular", entrada())
        if (r.ok)
            calc = r.dados
        else
            janela.avisar(r.erro, "erro")
    }

    function mudar(id, campo, valor) {
        const e = Object.assign({}, escolha)
        e[id] = Object.assign({ quantidade: "", defeito: false }, e[id] || {})
        e[id][campo] = valor
        escolha = e
        espera.restart()
    }

    function tudo() {
        const e = {}
        for (const it of venda.itens)
            if (it.pode !== "0")
                e[it.vendaItemId] = { quantidade: it.pode, defeito: (escolha[it.vendaItemId] || {}).defeito || false }
        escolha = e
        for (let i = 0; i < lista.count; i++) {
            const l = lista.itemAtIndex(i)
            if (l)
                l.atualizar()
        }
        recalcular()
    }

    function devolver(trocar) {
        erro = ""
        const r = Api.gravar("devolucao_gravar", entrada(), chaveDevolucao)
        if (!r.ok) {
            erro = r.erro
            if (r.campo === "motivo")
                motivo.focar()
            return
        }
        feita = r.dados
        feita.trocar = trocar
        if (trocar && feita.valeId) {
            janela.avisar("Devolução feita. O vale-troca já está no Vender.", "ok")
            const vale = feita.valeId
            janela.voltar()
            janela.abrir("TelaVender", { valeId: vale })
            return
        }
        pronto.open()
    }

    function imprimir() {
        const r = JSON.parse(Sistema.imprimir("devolucao", feita.id))
        if (!r.ok)
            janela.avisar(r.erro, "erro")
        else if (r.dados.impresso)
            janela.avisar("Enviado para a impressora.", "ok")
    }

    function aoEsc() {
        janela.voltar()
    }

    Timer { id: espera; interval: 150; onTriggered: tela.recalcular() }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        anchors.bottomMargin: Tema.margem - 8
        spacing: 12

        Cabecalho {
            textoVoltar: "Voltar"
            titulo: tela.venda ? "Devolução da venda " + tela.venda.numero : "Devolução"
            subtitulo: !tela.venda ? "" : [tela.venda.quando, tela.venda.caixa, tela.venda.cliente || "Consumidor", "total R$ " + tela.venda.total].join(" · ")
            Layout.fillWidth: true
            onVoltar: tela.janela.voltar()
            Botao {
                visible: !!tela.venda && tela.venda.situacao === "concluida"
                text: "Devolver tudo"
                tipo: "secundario"
                onClicked: tela.tudo()
            }
        }

        Rectangle {
            visible: !!tela.venda && tela.venda.situacao !== "concluida"
            Layout.fillWidth: true
            radius: Tema.raioCampo
            color: Tema.rosa
            implicitHeight: 44
            Text {
                anchors.verticalCenter: parent.verticalCenter
                x: 14
                text: "Esta venda foi cancelada: as peças já voltaram ao estoque."
                color: Tema.vermelho
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 15
            }
        }

        // Celular: uma parte de cada vez, escolhida nas pílulas.
        Pilulas {
            visible: Tema.estreito && (true)
            modelo: [{ id: "pecas", nome: "Peças" }, { id: "como", nome: "Como devolver" }]
            atual: tela.parte
            onEscolhido: id => tela.parte = id
        }

        RowLayout {
            spacing: 18
            Layout.fillWidth: true
            Layout.fillHeight: true

            // ---------- Peças da venda ----------
            ColumnLayout {
                visible: !Tema.estreito || tela.parte === "pecas"
                spacing: 8
                Layout.fillWidth: true
                Layout.fillHeight: true
                Rotulo { text: "Quais peças voltam? Digite a quantidade." }
                ListView {
                    id: lista
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 6
                    model: tela.venda ? tela.venda.itens : []
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar {}
                    delegate: Rectangle {
                        id: linha
                        required property var modelData
                        required property int index
                        readonly property var it: modelData
                        readonly property bool podeVoltar: it.pode !== "0"
                        readonly property bool escolhida: {
                            const e = tela.escolha[it.vendaItemId]
                            return !!e && e.quantidade !== "" && e.quantidade !== "0"
                        }
                        function atualizar() {
                            const e = tela.escolha[it.vendaItemId]
                            quantidade.texto = e ? e.quantidade : ""
                            defeito.checked = e ? e.defeito : false
                        }
                        width: lista.width
                        implicitHeight: 72
                        radius: Tema.raio
                        color: escolhida ? Tema.verde : (index + 1) % 2 === 0 ? Tema.azul : Tema.pergaminho
                        opacity: podeVoltar ? 1 : 0.55
                        Behavior on color { ColorAnimation { duration: 120 } }
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 18
                            anchors.rightMargin: 14
                            spacing: 14
                            ColumnLayout {
                                spacing: 1
                                Layout.fillWidth: true
                                Text {
                                    text: linha.it.descricao
                                    color: Tema.tinta
                                    font.family: Tema.titulo
                                    font.weight: Font.Bold
                                    font.pixelSize: 20
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                                Text {
                                    text: ["cód. " + linha.it.codigo, "vendeu " + linha.it.vendido + " " + linha.it.unidade,
                                           linha.it.devolvido ? "já voltou " + linha.it.devolvido : "",
                                           "R$ " + linha.it.valorUnitario + " cada"].filter(x => !!x).join("  ·  ")
                                    color: Tema.tintaSuave
                                    font.family: Tema.serifa
                                    font.italic: true
                                    font.pixelSize: 13
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                            }
                            Marcar {
                                id: defeito
                                visible: linha.escolhida
                                text: "Com defeito"
                                onToggled: tela.mudar(linha.it.vendaItemId, "defeito", checked)
                            }
                            Text {
                                visible: !linha.podeVoltar
                                text: "já voltou tudo"
                                color: Tema.apagado
                                font.family: Tema.serifa
                                font.italic: true
                                font.pixelSize: 14
                            }
                            Campo {
                                id: quantidade
                                visible: linha.podeVoltar
                                numerico: true
                                maximo: 10
                                dica: "0"
                                sufixo: linha.it.unidade
                                altura: 44
                                tamanhoFonte: 18
                                Layout.preferredWidth: 130
                                Layout.alignment: Qt.AlignVCenter
                                onEditado: tela.mudar(linha.it.vendaItemId, "quantidade", texto)
                            }
                        }
                        Component.onCompleted: atualizar()
                    }
                }
                Nota {
                    text: "“Com defeito”: a peça não volta para a prateleira (fica anotado no histórico dela, para a garantia com o fornecedor)."
                    font.pixelSize: 12
                    Layout.fillWidth: true
                }
            }

            // ---------- Valor e como devolver ----------
            Flickable {
                visible: !Tema.estreito || tela.parte === "como"
                Layout.preferredWidth: Tema.estreito ? -1 : 430
                Layout.fillWidth: Tema.estreito
                Layout.fillHeight: true
                contentHeight: lado.implicitHeight
                clip: true
                boundsBehavior: Flickable.StopAtBounds

                ColumnLayout {
                    id: lado
                    width: parent.width
                    spacing: 14

                    Faixa {
                        titulo: "Volta para o cliente"
                        color: Tema.pergaminho
                        Layout.fillWidth: true
                        Text {
                            text: "R$ " + (tela.calc ? tela.calc.valor : "0,00")
                            color: Tema.tinta
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: 44
                        }
                        Nota {
                            text: "O que o cliente pagou pelas peças, já com o desconto da venda."
                            font.pixelSize: 13
                            Layout.fillWidth: true
                        }
                    }

                    Faixa {
                        titulo: "Como devolver"
                        color: Tema.azul
                        Layout.fillWidth: true
                        Flow {
                            spacing: 6
                            Layout.fillWidth: true
                            Pilulas {
                                pequenas: true
                                modelo: [{ id: "vale", nome: "Vale-troca" }, { id: "dinheiro", nome: "Dinheiro" }, { id: "pix", nome: "Pix ou cartão" }]
                                        .concat(tela.venda && tela.venda.crediarioAberto !== "0,00" ? [{ id: "crediario", nome: "Abater do crediário" }] : [])
                                atual: tela.destino
                                onEscolhido: id => {
                                    tela.destino = id
                                    tela.recalcular()
                                }
                            }
                        }
                        Nota {
                            text: ({
                                vale: "Vira crédito para outra compra (é assim a troca). Pode usar de uma vez ou aos poucos.",
                                dinheiro: "Sai da gaveta do caixa aberto.",
                                pix: "Devolvido por Pix ou estornado na maquininha (não mexe na gaveta).",
                                crediario: tela.venda ? "O cliente ainda deve R$ " + tela.venda.crediarioAberto + " desta venda: o valor abate as parcelas." : ""
                            })[tela.destino] || ""
                            font.pixelSize: 13
                            Layout.fillWidth: true
                        }
                        ColumnLayout {
                            visible: !!tela.calc && tela.calc.precisaResto
                            spacing: 6
                            Layout.fillWidth: true
                            Rotulo { text: "E o que o cliente já tinha pago?" }
                            Pilulas {
                                pequenas: true
                                modelo: [{ id: "vale", nome: "Vale-troca" }, { id: "dinheiro", nome: "Dinheiro" }, { id: "pix", nome: "Pix ou cartão" }]
                                atual: tela.resto
                                onEscolhido: id => {
                                    tela.resto = id
                                    tela.recalcular()
                                }
                            }
                        }
                        Repeater {
                            model: tela.calc && tela.calc.partes.length > 1 ? tela.calc.partes : []
                            delegate: RowLayout {
                                required property var modelData
                                Layout.fillWidth: true
                                Text { text: modelData.nome; color: Tema.tinta; font.family: Tema.texto; font.pixelSize: 15; Layout.fillWidth: true }
                                Text { text: "R$ " + modelData.valor; color: Tema.tinta; font.family: Tema.texto; font.weight: Font.Bold; font.pixelSize: 15 }
                            }
                        }
                    }

                    Faixa {
                        titulo: "Motivo"
                        color: Tema.verde
                        corTitulo: Tema.verdeEscuro
                        Layout.fillWidth: true
                        Campo {
                            id: motivo
                            dica: "Ex.: peça errada, não serviu no carro"
                            maximo: 200
                            sugestoes: tela.motivos
                            Layout.fillWidth: true
                        }
                        Flow {
                            spacing: 6
                            Layout.fillWidth: true
                            Repeater {
                                model: tela.motivos
                                delegate: Rectangle {
                                    required property var modelData
                                    radius: height / 2
                                    height: 28
                                    width: rotuloMotivo.implicitWidth + 22
                                    color: motivo.texto === modelData ? Tema.tinta : sobreMotivo.hovered ? Tema.vela(0.75) : Tema.vela(0.45)
                                    Text {
                                        id: rotuloMotivo
                                        anchors.centerIn: parent
                                        text: modelData
                                        color: motivo.texto === modelData ? Tema.claro : Tema.tintaSuave
                                        font.family: Tema.texto
                                        font.pixelSize: 13
                                    }
                                    HoverHandler { id: sobreMotivo; cursorShape: Qt.PointingHandCursor }
                                    TapHandler {
                                        onTapped: {
                                            motivo.texto = modelData
                                            if (modelData === "Veio com defeito") {
                                                const e = Object.assign({}, tela.escolha)
                                                for (const id in e)
                                                    e[id] = Object.assign({}, e[id], { defeito: true })
                                                tela.escolha = e
                                                for (let i = 0; i < lista.count; i++) {
                                                    const l = lista.itemAtIndex(i)
                                                    if (l)
                                                        l.atualizar()
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    TextoErro {
                        text: tela.erro !== "" ? tela.erro : (tela.calc && tela.calc.erro ? tela.calc.erro : "")
                        Layout.fillWidth: true
                    }

                    Linha {
                        spacing: 10
                        Layout.fillWidth: true
                        Botao {
                            visible: tela.destino === "vale" || (tela.calc && tela.calc.precisaResto && tela.resto === "vale")
                            text: "Trocar agora"
                            tipo: "secundario"
                            enabled: !!tela.calc && tela.calc.itens > 0
                            onClicked: tela.devolver(true)
                        }
                        Item { Layout.fillWidth: true }
                        Botao {
                            text: "Devolver"
                            icone: "check"
                            enabled: !!tela.calc && tela.calc.itens > 0
                            onClicked: tela.devolver(false)
                        }
                    }
                    Nota {
                        visible: tela.destino === "vale"
                        text: "“Trocar agora” devolve e já abre o Vender com o vale-troca, para o cliente levar outra peça."
                        font.pixelSize: 12
                        Layout.fillWidth: true
                    }
                }
            }
        }
    }

    // ---------- Devolução feita ----------
    Painel {
        id: pronto
        cor: Tema.verde
        width: Math.min(520, Tema.largura - 24)
        contentItem: ColumnLayout {
            spacing: 12
            Titulo { text: "Devolução feita"; font.pixelSize: 34; Layout.fillWidth: true }
            Nota {
                text: !tela.feita ? "" : tela.feita.valeId ? "Ficou um vale-troca para o cliente. Imprima e entregue o comprovante."
                                                           : "As peças já voltaram ao estoque (menos as com defeito)."
                font.pixelSize: 16
                Layout.fillWidth: true
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao { text: "Fechar"; tipo: "discreto"; onClicked: { pronto.close(); tela.janela.voltar() } }
                Botao {
                    text: "Imprimir comprovante"
                    icone: "impressora"
                    onClicked: { tela.imprimir(); pronto.close(); tela.janela.voltar() }
                }
            }
        }
    }

    Component.onCompleted: carregar()
}
