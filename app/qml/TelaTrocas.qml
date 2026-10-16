import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Trocas e devoluções: achar a venda para devolver, os vales-troca em
// aberto e as devoluções já feitas (que dá para desfazer).
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    property string secao: "devolver"
    property var vendas: []
    property var vales: []
    property var feitas: []
    property bool buscou: false

    function carregar() {
        if (secao === "devolver") {
            const r = Api.chamar("vendas_listar", { tipo: "venda", texto: busca.texto, limite: 60 })
            if (r.ok)
                vendas = r.dados.filter(v => v.situacao === "concluida")
        } else if (secao === "vales") {
            const r = Api.chamar("vales_abertos", { id: "" })
            if (r.ok)
                vales = r.dados
        } else {
            const r = Api.chamar("devolucoes_listar", { texto: busca.texto })
            if (r.ok)
                feitas = r.dados
        }
        buscou = true
    }

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
            carregar()
        } else {
            janela.voltar()
        }
    }

    function focarBusca() {
        busca.focar()
    }

    onSecaoChanged: {
        buscou = false
        carregar()
    }

    Timer { id: espera; interval: 200; onTriggered: tela.carregar() }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        spacing: 14

        Cabecalho {
            titulo: "Trocas e devoluções"
            subtitulo: "Peça que o cliente devolve, troca e vale-troca"
            Layout.fillWidth: true
            onVoltar: tela.janela.voltar()
        }

        Linha {
            spacing: 14
            Layout.fillWidth: true
            Pilulas {
                modelo: [
                    { id: "devolver", nome: "Devolver" },
                    { id: "vales", nome: "Vales-troca" },
                    { id: "feitas", nome: "Já devolvidas" }
                ]
                atual: tela.secao
                onEscolhido: id => tela.secao = id
            }
            Campo {
                id: busca
                visible: tela.secao !== "vales"
                icone: "busca"
                altura: 46
                tamanhoFonte: 16
                dica: tela.secao === "devolver" ? "Número da venda (está no comprovante), cliente ou peça" : "Cliente, número da venda ou peça"
                Layout.fillWidth: true
                onEditado: espera.restart()
                onConfirmado: {
                    if (tela.secao === "devolver" && tela.vendas.length === 1)
                        tela.janela.abrir("TelaDevolucao", { vendaId: tela.vendas[0].id })
                }
            }
            Item { visible: tela.secao === "vales"; Layout.fillWidth: true }
        }

        // ---------- Devolver: escolher a venda ----------
        ListView {
            id: listaVendas
            visible: tela.secao === "devolver"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 6
            model: tela.vendas
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            delegate: LinhaVenda {
                required property var modelData
                required property int index
                width: listaVendas.width
                item: modelData
                indice: index
                onAbrir: tela.janela.abrir("TelaDevolucao", { vendaId: modelData.id })
            }
            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(parent.width - 40, 620)
                visible: tela.buscou && listaVendas.count === 0
                spacing: 8
                Titulo {
                    text: busca.texto !== "" ? "Nenhuma venda encontrada" : "Nenhuma venda ainda"
                    font.pixelSize: 30
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Nota {
                    text: "Procure pelo número que está no comprovante da venda, pelo nome do cliente ou pela peça."
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
            }
        }

        // ---------- Vales-troca em aberto ----------
        ListView {
            id: listaVales
            visible: tela.secao === "vales"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 6
            model: tela.vales
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                id: lv
                required property var modelData
                required property int index
                width: listaVales.width
                implicitHeight: 62
                radius: Tema.raio
                color: (index + 1) % 2 === 0 ? Tema.azul : Tema.verde
                HoverHandler { id: sobreVale }
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 18
                    anchors.rightMargin: 16
                    spacing: 14
                    Text {
                        text: "nº " + lv.modelData.numero
                        color: Tema.dourado
                        font.family: Tema.titulo
                        font.weight: Font.Bold
                        font.pixelSize: 22
                        Layout.preferredWidth: 70
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        Text {
                            text: lv.modelData.cliente || "Sem cliente"
                            color: Tema.tinta
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: 20
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        Text {
                            text: [lv.modelData.quando, lv.modelData.caixa, lv.modelData.vendaNumero ? "da venda " + lv.modelData.vendaNumero : "",
                                   lv.modelData.valor !== lv.modelData.saldo ? "era R$ " + lv.modelData.valor : ""].filter(x => !!x).join("  ·  ")
                            color: Tema.tintaSuave
                            font.family: Tema.serifa
                            font.italic: true
                            font.pixelSize: 13
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                    }
                    Botao {
                        visible: sobreVale.hovered
                        text: "Imprimir"
                        icone: "impressora"
                        tipo: "discreto"
                        implicitHeight: 36
                        onClicked: tela.imprimir("vale", lv.modelData.id)
                    }
                    Botao {
                        visible: sobreVale.hovered
                        text: "Usar no Vender"
                        tipo: "secundario"
                        implicitHeight: 36
                        onClicked: tela.janela.abrir("TelaVender", { valeId: lv.modelData.id })
                    }
                    Text {
                        text: "R$ " + lv.modelData.saldo
                        color: Tema.verdeEscuro
                        font.family: Tema.texto
                        font.weight: Font.Bold
                        font.pixelSize: 18
                        horizontalAlignment: Text.AlignRight
                        Layout.preferredWidth: 120
                    }
                }
            }
            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(parent.width - 40, 620)
                visible: tela.buscou && listaVales.count === 0
                spacing: 8
                Titulo {
                    text: "Nenhum vale-troca em aberto"
                    font.pixelSize: 30
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Nota {
                    text: "O vale-troca nasce na devolução. No Vender, ele aparece no Receber para pagar a compra nova."
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
            }
        }

        // ---------- Devoluções já feitas ----------
        ListView {
            id: listaFeitas
            visible: tela.secao === "feitas"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 6
            model: tela.feitas
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                id: lf
                required property var modelData
                required property int index
                width: listaFeitas.width
                implicitHeight: 66
                radius: Tema.raio
                color: lf.modelData.cancelada ? Qt.rgba(1, 1, 1, 0.3) : (index + 1) % 2 === 0 ? Tema.azul : Tema.pergaminho
                HoverHandler { id: sobreFeita }
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 18
                    anchors.rightMargin: 16
                    spacing: 14
                    opacity: lf.modelData.cancelada ? 0.6 : 1
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        Text {
                            text: "Devolução " + lf.modelData.numero + " · venda " + lf.modelData.vendaNumero + (lf.modelData.cliente ? " · " + lf.modelData.cliente : "")
                            color: Tema.tinta
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: 20
                            font.strikeout: lf.modelData.cancelada
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        Text {
                            text: [lf.modelData.quando, lf.modelData.itens, lf.modelData.motivo, lf.modelData.cancelada ? "desfeita" : ""].filter(x => !!x).join("  ·  ")
                            color: Tema.tintaSuave
                            font.family: Tema.serifa
                            font.italic: true
                            font.pixelSize: 13
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                    }
                    Botao {
                        visible: sobreFeita.hovered
                        text: "Imprimir"
                        icone: "impressora"
                        tipo: "discreto"
                        implicitHeight: 36
                        onClicked: tela.imprimir("devolucao", lf.modelData.id)
                    }
                    Botao {
                        visible: sobreFeita.hovered && !lf.modelData.cancelada
                        text: "Desfazer"
                        tipo: "discreto"
                        implicitHeight: 36
                        onClicked: pergunta.perguntar("Desfazer a devolução " + lf.modelData.numero + "?",
                                                      lf.modelData.valeUsado ? "O vale-troca desta devolução já foi usado: cancele antes a venda que usou o vale."
                                                                             : "As peças saem de novo do estoque" + (lf.modelData.como.indexOf("vale") >= 0 ? " e o vale-troca é cancelado." : "."),
                                                      "Desfazer",
                                                      (m) => Api.chamar("devolucao_desfazer", { id: lf.modelData.id, motivo: m }))
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.preferredWidth: 190
                        Text {
                            text: "R$ " + lf.modelData.valor
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.Bold
                            font.pixelSize: 17
                            Layout.alignment: Qt.AlignRight
                        }
                        Text {
                            text: lf.modelData.como
                            color: Tema.tintaSuave
                            font.family: Tema.texto
                            font.pixelSize: 12
                            elide: Text.ElideRight
                            Layout.maximumWidth: 190
                            Layout.alignment: Qt.AlignRight
                        }
                    }
                }
            }
            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(parent.width - 40, 620)
                visible: tela.buscou && listaFeitas.count === 0
                spacing: 8
                Titulo {
                    text: busca.texto !== "" ? "Nada encontrado" : "Nenhuma devolução ainda"
                    font.pixelSize: 30
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
            }
        }
    }

    PerguntaMotivo {
        id: pergunta
        onFeito: {
            tela.janela.avisar("Devolução desfeita.", "ok")
            tela.carregar()
        }
    }

    StackView.onActivated: {
        carregar()
        if (secao !== "vales")
            busca.focar()
    }
}
