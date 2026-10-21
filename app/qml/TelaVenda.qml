import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Templates as T
import QtQuick.Layouts
import rustinho

// Uma venda ou orçamento: itens, pagamentos, imprimir, cancelar, vender.
FocusScope {
    id: tela
    property string parte: "itens"
    readonly property var janela: ApplicationWindow.window
    property string vendaId: ""
    property var v: null
    readonly property bool orcamento: !!v && v.tipo === "orcamento"

    function carregar() {
        const r = Api.chamar("venda_abrir", { id: vendaId })
        if (r.ok)
            v = r.dados
        else {
            janela.avisar(r.erro, "erro")
            janela.voltar()
        }
    }

    function imprimir() {
        const r = JSON.parse(Sistema.imprimir(orcamento ? "orcamento" : "venda", vendaId))
        if (!r.ok)
            janela.avisar(r.erro, "erro")
        else if (r.dados.impresso)
            janela.avisar("Enviado para a impressora.", "ok")
    }

    function cancelar() {
        motivo.texto = ""
        motivo.erro = ""
        caixaCancelar.open()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        anchors.bottomMargin: 0
        spacing: 12

        Cabecalho {
            textoVoltar: "Voltar"
            titulo: !tela.v ? "" : (tela.orcamento ? "Orçamento " : "Venda ") + tela.v.numero
                                   + " · " + (tela.v.cliente || (tela.orcamento ? "sem cliente" : "Consumidor"))
            subtitulo: !tela.v ? "" : [Qt.formatDateTime(new Date(tela.v.criadoEm), "dd/MM/yyyy 'às' HH:mm"), tela.v.caixa,
                                       "por " + tela.v.vendedor, tela.orcamento ? "válido até " + tela.v.validade : ""].filter(x => !!x).join(" · ")
            Layout.fillWidth: true
            onVoltar: tela.janela.voltar()
            Botao {
                visible: !!tela.v && ((tela.orcamento && tela.v.situacao === "aberto") || (!tela.orcamento && tela.v.situacao === "concluida"))
                text: tela.orcamento ? "Cancelar orçamento" : "Cancelar venda"
                tipo: "discreto"
                onClicked: tela.cancelar()
            }
            Botao {
                visible: !tela.orcamento && !!tela.v && tela.v.situacao === "concluida"
                text: "Devolver ou trocar"
                tipo: "secundario"
                onClicked: tela.janela.abrir("TelaDevolucao", { vendaId: tela.vendaId })
            }
            Botao {
                text: "Imprimir"
                tipo: "secundario"
                onClicked: tela.imprimir()
            }
            Botao {
                visible: tela.orcamento && !!tela.v && tela.v.situacao === "aberto"
                text: "Vender"
                icone: "check"
                onClicked: tela.janela.abrir("TelaVender", { orcamentoId: tela.vendaId })
            }
        }

        Rectangle {
            visible: !!tela.v && ["cancelada", "cancelado", "vendido"].includes(tela.v.situacao) || (!!tela.v && tela.v.vencido)
            Layout.fillWidth: true
            radius: Tema.raioCampo
            color: tela.v && tela.v.situacao === "vendido" ? Tema.verde : Tema.rosa
            implicitHeight: aviso.implicitHeight + 20
            Text {
                id: aviso
                x: 14
                y: 10
                width: parent.width - 28
                wrapMode: Text.WordWrap
                text: !tela.v ? "" : tela.v.situacao === "vendido" ? "Este orçamento já virou venda."
                      : tela.v.vencido ? "Este orçamento passou da validade. Ao vender, os preços de hoje serão conferidos."
                      : (tela.orcamento ? "Orçamento cancelado" : "Venda cancelada: as peças voltaram ao estoque")
                        + (tela.v.motivoCancelamento ? ". Motivo: " + tela.v.motivoCancelamento : ".")
                color: tela.v && tela.v.situacao === "vendido" ? Tema.verdeEscuro : Tema.vermelho
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 15
            }
        }

        // Celular: uma parte de cada vez, escolhida nas pílulas.
        Pilulas {
            visible: Tema.estreito && (true)
            modelo: [{ id: "itens", nome: "Peças" }, { id: "total", nome: "Total e pagamento" }]
            atual: tela.parte
            onEscolhido: id => tela.parte = id
        }

        RowLayout {
            spacing: 18
            Layout.fillWidth: true
            Layout.fillHeight: true

            ListView {
                visible: !Tema.estreito || tela.parte === "itens"
                id: lista
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 6
                model: tela.v ? tela.v.itens : []
                ScrollBar.vertical: ScrollBar {}
                delegate: Rectangle {
                    required property var modelData
                    required property int index
                    width: lista.width
                    height: Tema.estreito ? 62 : 56
                    radius: Tema.raio
                    color: (index + 1) % 2 === 0 ? Tema.azul : Tema.pergaminho
                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 16
                        anchors.rightMargin: 18
                        spacing: 14
                        Text {
                            text: modelData.codigo
                            color: Tema.tintaSuave
                            font.family: Tema.texto
                            font.weight: Font.DemiBold
                            font.pixelSize: 14
                            visible: !Tema.estreito
                            Layout.preferredWidth: 60
                        }
                        ColumnLayout {
                            spacing: 0
                            Layout.fillWidth: true
                            Text {
                                text: modelData.descricao
                                color: Tema.tinta
                                font.family: Tema.titulo
                                font.weight: Font.Bold
                                font.pixelSize: Tema.estreito ? 18 : 20
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                            Text {
                                visible: Tema.estreito
                                text: modelData.quantidade + " " + modelData.unidade + " × R$ " + modelData.preco
                                      + (modelData.desconto ? "  − R$ " + modelData.desconto : "")
                                color: Tema.tintaSuave
                                font.family: Tema.texto
                                font.pixelSize: 13
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                        }
                        Text {
                            visible: !Tema.estreito
                            text: modelData.quantidade + " " + modelData.unidade + " × R$ " + modelData.preco
                                  + (modelData.desconto ? "  − R$ " + modelData.desconto : "")
                            color: Tema.tintaSuave
                            font.family: Tema.texto
                            font.pixelSize: 15
                        }
                        Text {
                            text: "R$ " + modelData.total
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.Bold
                            font.pixelSize: 17
                            horizontalAlignment: Text.AlignRight
                            Layout.preferredWidth: Tema.estreito ? -1 : 110
                        }
                    }
                }
            }

            Faixa {
                visible: !Tema.estreito || tela.parte === "total"
                titulo: "Total"
                color: Tema.verde
                corTitulo: Tema.verdeEscuro
                espacamento: 8
                Layout.preferredWidth: Tema.estreito ? -1 : 340
                Layout.fillWidth: Tema.estreito
                Layout.alignment: Qt.AlignTop
                Repeater {
                    model: !tela.v ? [] : [["Subtotal", "R$ " + tela.v.subtotal],
                                          ["Desconto", tela.v.desconto !== "0,00" ? "− R$ " + tela.v.desconto : ""]]
                                          .concat(tela.v.pagamentos.map(p => [p.nome, "R$ " + p.valor]))
                                          .concat([["Troco", tela.v.troco !== "0,00" ? "R$ " + tela.v.troco : ""]])
                                          .filter(l => l[1] !== "")
                    delegate: RowLayout {
                        required property var modelData
                        Layout.fillWidth: true
                        Text { text: modelData[0]; color: Tema.tintaSuave; font.family: Tema.texto; font.pixelSize: 15; Layout.fillWidth: true }
                        Text { text: modelData[1]; color: Tema.tinta; font.family: Tema.texto; font.pixelSize: 15 }
                    }
                }
                Rotulo { text: "Total"; color: Tema.verdeEscuro; Layout.topMargin: 6 }
                Text {
                    text: tela.v ? "R$ " + tela.v.total : ""
                    color: Tema.tinta
                    font.family: Tema.texto
                    font.weight: Font.Bold
                    font.pixelSize: 40
                }
                Rotulo {
                    visible: !!tela.v && tela.v.parcelas.length > 0
                    text: "Crediário"
                    color: Tema.verdeEscuro
                    Layout.topMargin: 6
                }
                Repeater {
                    model: tela.v ? tela.v.parcelas : []
                    delegate: RowLayout {
                        required property var modelData
                        Layout.fillWidth: true
                        Text {
                            text: modelData.parcela + "  vence " + modelData.vencimento
                            color: modelData.situacao === "vencida" ? Tema.vermelho : Tema.tintaSuave
                            font.family: Tema.texto
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }
                        Text {
                            text: modelData.situacao === "paga" ? "paga" : modelData.situacao === "cancelada" ? "cancelada" : "R$ " + modelData.saldo
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.pixelSize: 14
                        }
                    }
                }
                Botao {
                    visible: !!tela.v && tela.v.parcelas.length > 0 && !!tela.v.clienteId
                    text: "Ver no Receber"
                    tipo: "discreto"
                    onClicked: tela.janela.abrir("TelaReceber", { clienteInicial: tela.v.clienteId })
                }
                Rotulo {
                    visible: !!tela.v && tela.v.cheques.length > 0
                    text: "Cheques"
                    color: Tema.verdeEscuro
                    Layout.topMargin: 6
                }
                Repeater {
                    model: tela.v ? tela.v.cheques : []
                    delegate: Text {
                        required property var modelData
                        text: modelData
                        color: Tema.tintaSuave
                        font.family: Tema.texto
                        font.pixelSize: 14
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                }
                Nota {
                    visible: !!tela.v && tela.v.observacoes !== ""
                    text: tela.v ? tela.v.observacoes : ""
                    font.pixelSize: 14
                    Layout.fillWidth: true
                }
            }
        }
    }

    Popup {
        id: caixaCancelar
        modal: true
        focus: true
        anchors.centerIn: T.Overlay.overlay
        width: Math.min(500, Tema.largura - 24)
        padding: Tema.estreito ? 18 : 30
        T.Overlay.modal: Rectangle { color: Tema.veu }
        background: Rectangle { color: Tema.pergaminho; radius: Tema.raio + 3 }
        contentItem: ColumnLayout {
            spacing: 14
            Titulo { text: tela.orcamento ? "Cancelar o orçamento?" : "Cancelar a venda?"; font.pixelSize: 30; Layout.fillWidth: true }
            Nota {
                text: tela.orcamento ? "O orçamento fica guardado como cancelado."
                                     : "As peças voltam ao estoque e a venda sai do total do caixa. Devolva o dinheiro ao cliente. Venda de um caixa já fechado: o fechamento dele não muda; o dinheiro sai da gaveta aberta (sem caixa aberto, do Caixa geral)."
                Layout.fillWidth: true
            }
            Campo {
                id: motivo
                rotulo: "Motivo"
                dica: "Ex.: cliente desistiu, peça errada"
                maximo: 120
                Layout.fillWidth: true
                onConfirmado: botaoCancelar.clicked()
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                Botao { text: "Voltar"; tipo: "discreto"; onClicked: caixaCancelar.close() }
                Botao {
                    id: botaoCancelar
                    text: tela.orcamento ? "Cancelar orçamento" : "Cancelar venda"
                    tipo: "perigo"
                    onClicked: {
                        tela.janela.chamarGerente("venda_cancelar", { id: tela.vendaId, motivo: motivo.texto }, r => {
                            if (r.ok) {
                                tela.v = r.dados
                                caixaCancelar.close()
                                tela.janela.avisar(tela.orcamento ? "Orçamento cancelado." : "Venda cancelada. As peças voltaram ao estoque.", "ok")
                            } else {
                                motivo.erro = r.erro
                            }
                        })
                    }
                }
            }
        }
        onOpened: motivo.focar()
    }

    StackView.onActivated: carregar()
}
