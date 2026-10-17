import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Comissões: quanto cada vendedor ganhou no período; "Fechar" lança no A pagar.
FocusScope {
    id: secao
    readonly property var janela: ApplicationWindow.window
    property var dados: null
    property string periodo: "mes"
    property string erro: ""

    function datas(p) {
        const h = new Date()
        const f = d => Qt.formatDate(d, "dd/MM/yyyy")
        if (p === "mes")
            return [f(new Date(h.getFullYear(), h.getMonth(), 1)), f(h)]
        if (p === "passado")
            return [f(new Date(h.getFullYear(), h.getMonth() - 1, 1)), f(new Date(h.getFullYear(), h.getMonth(), 0))]
        if (p === "quinzena") {
            const d = h.getDate() <= 15 ? 1 : 16
            return [f(new Date(h.getFullYear(), h.getMonth(), d)), f(h)]
        }
        return [de.texto, ate.texto]
    }
    function usar(p) {
        periodo = p
        const d = datas(p)
        de.texto = d[0]
        ate.texto = d[1]
        carregar()
    }
    function carregar() {
        const r = Api.chamar("comissoes", { de: de.texto, ate: ate.texto })
        erro = r.ok ? "" : r.erro
        if (r.ok)
            dados = r.dados
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 12

        Linha {
            spacing: 12
            Layout.fillWidth: true
            Pilulas {
                pequenas: true
                modelo: [{ id: "mes", nome: "Este mês" }, { id: "quinzena", nome: "Esta quinzena" }, { id: "passado", nome: "Mês passado" }]
                atual: secao.periodo
                onEscolhido: id => secao.usar(id)
            }
            Item { Layout.fillWidth: true }
            RowLayout {
                spacing: 8
                Campo { id: de; dica: "de"; maximo: 10; Layout.preferredWidth: Tema.estreito ? -1 : 140; Layout.fillWidth: Tema.estreito; onConfirmado: { secao.periodo = ""; secao.carregar() } }
                Text { text: "a"; color: Tema.tintaSuave; font.family: Tema.serifa; font.italic: true; font.pixelSize: 15 }
                Campo { id: ate; dica: "até"; maximo: 10; Layout.preferredWidth: Tema.estreito ? -1 : 140; Layout.fillWidth: Tema.estreito; onConfirmado: { secao.periodo = ""; secao.carregar() } }
                Botao { text: "Ver"; tipo: "secundario"; onClicked: { secao.periodo = ""; secao.carregar() } }
            }
        }
        TextoErro { text: secao.erro }

        ListView {
            id: lista
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 6
            model: secao.dados ? secao.dados.vendedores : []
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                id: linha
                required property var modelData
                required property int index
                width: lista.width
                implicitHeight: 64
                radius: Tema.raio
                color: (index + 1) % 2 === 0 ? Tema.azul : Tema.pergaminho
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Tema.estreito ? 14 : 20
                    anchors.rightMargin: 14
                    spacing: Tema.estreito ? 10 : 16
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        Text {
                            text: linha.modelData.nome
                            color: Tema.tinta
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: 22
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        Text {
                            text: linha.modelData.percentual ? Tema.pct(linha.modelData.percentual) + "% nas peças sem comissão própria"
                                                             : "sem % no cadastro (só as peças com comissão própria)"
                            color: Tema.tintaSuave
                            font.family: Tema.serifa
                            font.italic: true
                            font.pixelSize: 13
                        }
                    }
                    ColumnLayout {
                        visible: !Tema.estreito
                        spacing: 0
                        Layout.preferredWidth: 150
                        Text {
                            text: "R$ " + linha.modelData.vendido
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.DemiBold
                            font.pixelSize: 15
                            Layout.alignment: Qt.AlignRight
                        }
                        Text {
                            text: linha.modelData.vendas + (linha.modelData.vendas === 1 ? " venda" : " vendas")
                            color: Tema.tintaSuave
                            font.family: Tema.texto
                            font.pixelSize: 12
                            Layout.alignment: Qt.AlignRight
                        }
                    }
                    Text {
                        text: "R$ " + Tema.valor(linha.modelData.comissao)
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.weight: Font.Bold
                        font.pixelSize: 19
                        horizontalAlignment: Text.AlignRight
                        Layout.preferredWidth: Tema.estreito ? -1 : 130
                    }
                    Item {
                        Layout.preferredWidth: Tema.estreito ? 76 : 190
                        implicitHeight: 36
                        Chip {
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            visible: !Tema.estreito && texto !== ""
                            texto: linha.modelData.jaFechado ? "Fechada " + linha.modelData.jaFechado : ""
                            color: Tema.verde
                        }
                        Botao {
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            visible: !linha.modelData.jaFechado && linha.modelData.comissao !== "0,00"
                            text: "Fechar"
                            tipo: "secundario"
                            onClicked: fechamento.abrirCom(linha.modelData)
                        }
                    }
                }
            }
            footer: ColumnLayout {
                width: lista.width
                spacing: 5
                Nota {
                    text: "A comissão da peça (no cadastro da peça) vale mais que a do vendedor (em Usuários). O desconto da venda é dividido entre as peças; venda cancelada não conta."
                    font.pixelSize: 13
                    Layout.fillWidth: true
                    Layout.topMargin: 10
                }
                Rotulo {
                    text: "Fechamentos"
                    visible: !!secao.dados && secao.dados.fechamentos.length > 0
                    Layout.topMargin: 16
                }
                Repeater {
                    model: secao.dados ? secao.dados.fechamentos : []
                    delegate: Rectangle {
                        required property var modelData
                        Layout.fillWidth: true
                        radius: Tema.raioCampo
                        color: Tema.vela(0.45)
                        implicitHeight: 44
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 14
                            anchors.rightMargin: 8
                            spacing: 12
                            opacity: modelData.cancelado ? 0.5 : 1
                            Text {
                                text: [modelData.nome, modelData.periodo, modelData.vendas + " vendas · R$ " + modelData.vendido,
                                       "fechada em " + modelData.criadoEm, modelData.cancelado ? "DESFEITA" : modelData.pago ? "paga" : "no A pagar"].join("  ·  ")
                                color: Tema.tintaSuave
                                font.family: Tema.texto
                                font.pixelSize: 13
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                            Text {
                                text: "R$ " + Tema.valor(modelData.valor)
                                color: Tema.tinta
                                font.family: Tema.texto
                                font.weight: Font.Bold
                                font.pixelSize: 15
                                font.strikeout: modelData.cancelado
                            }
                            Botao {
                                visible: !modelData.cancelado && !modelData.pago
                                text: "Desfazer"
                                tipo: "discreto"
                                implicitHeight: 32
                                onClicked: janela.confirmar("Desfazer o fechamento?", "A conta da comissão sai do A pagar.", "Desfazer", () => {
                                    const r = Api.chamar("comissao_reabrir", { id: modelData.id })
                                    if (!r.ok)
                                        janela.avisar(r.erro, "erro")
                                    secao.carregar()
                                })
                            }
                        }
                    }
                }
            }
        }
    }

    // ---------- Fechar ----------
    Painel {
        id: fechamento
        property var vendedor: null
        property string erro: ""
        width: Math.min(520, Tema.largura - 24)
        function abrirCom(v) {
            vendedor = v
            erro = ""
            vencimento.texto = Qt.formatDate(new Date(), "dd/MM/yyyy")
            open()
            vencimento.focar()
        }
        function salvar() {
            const r = Api.chamar("comissao_fechar", { usuarioId: vendedor.usuarioId, de: de.texto, ate: ate.texto, vencimento: vencimento.texto })
            if (!r.ok) {
                erro = r.erro
                return
            }
            close()
            secao.dados = r.dados
            janela.avisar("Comissão lançada no A pagar.", "ok")
        }
        contentItem: ColumnLayout {
            spacing: 12
            Titulo { text: fechamento.vendedor ? "Fechar comissão de " + fechamento.vendedor.nome : ""; font.pixelSize: 30; wrapMode: Text.WordWrap; Layout.fillWidth: true }
            Nota {
                text: fechamento.vendedor ? de.texto + " a " + ate.texto + "  ·  " + fechamento.vendedor.vendas + " vendas  ·  comissão R$ " + Tema.valor(fechamento.vendedor.comissao) : ""
                font.pixelSize: 15
                Layout.fillWidth: true
            }
            Campo { id: vencimento; rotulo: "Pagar em"; maximo: 10; Layout.preferredWidth: 180; onConfirmado: fechamento.salvar() }
            TextoErro { text: fechamento.erro }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao { text: "Cancelar"; tipo: "discreto"; onClicked: fechamento.close() }
                Botao { text: "Fechar e lançar"; icone: "check"; onClicked: fechamento.salvar() }
            }
        }
    }

    Component.onCompleted: usar("mes")
}
