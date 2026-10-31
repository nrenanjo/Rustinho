import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Nota de compra emitida para outro CNPJ (0.9.17). Ela não entra como compra
// (regra de sempre) e nada foi gravado: a janela mostra os dois documentos
// para comparar. "Conferir nota" mostra a nota, só para ler; "Descartar nota"
// tira a nota desta importação; "Cancelar" (ou Esc) só fecha: a nota fica em
// "Não importadas", em Compras. Nada aqui muda o cadastro da loja.
Painel {
    id: janelaConflito
    property var conflito: null
    property bool conferindo: false
    width: Math.min(720, Tema.largura - 24)
    // Nunca mais alta que a janela: o conteúdo rola e os botões ficam à vista.
    height: Math.min(implicitHeight, Tema.altura - 24)
    signal descartar(var conflito)

    function mostrar(c) {
        conflito = c
        conferindo = false
        open()
    }

    contentItem: ColumnLayout {
        spacing: 16

        Flickable {
            id: rolagem
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.preferredHeight: corpo.implicitHeight
            contentHeight: corpo.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}

            ColumnLayout {
                id: corpo
                width: rolagem.width
                spacing: 16

                RowLayout {
                    spacing: 16
                    Layout.fillWidth: true
                    Rectangle {
                        Layout.preferredWidth: 64
                        Layout.preferredHeight: 64
                        Layout.alignment: Qt.AlignTop
                        radius: Tema.raio
                        color: Tema.atencao
                        border.color: Tema.bordaAtencao
                        Image {
                            anchors.centerIn: parent
                            width: 36
                            height: 36
                            source: Tema.escuro ? "qrc:/qt/qml/rustinho/icone/alerta-branco.png" : "qrc:/qt/qml/rustinho/icone/alerta-preto.png"
                            fillMode: Image.PreserveAspectFit
                            smooth: true
                            mipmap: true
                        }
                    }
                    ColumnLayout {
                        spacing: 6
                        Layout.fillWidth: true
                        Titulo {
                            text: "CNPJ da nota é diferente"
                            font.pixelSize: Tema.estreito ? 26 : 32
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                        Nota {
                            text: "O destinatário desta nota não é a loja. Ela não entra como compra; confira antes de decidir."
                            Layout.fillWidth: true
                        }
                    }
                }

                Nota {
                    text: janelaConflito.conflito ? janelaConflito.conflito.arquivo : ""
                    font.pixelSize: 14
                    elide: Text.ElideMiddle
                    wrapMode: Text.NoWrap
                    Layout.fillWidth: true
                }

                GridLayout {
                    columns: Tema.estreito ? 1 : 2
                    columnSpacing: 14
                    rowSpacing: 14
                    Layout.fillWidth: true
                    Faixa {
                        titulo: "CNPJ da loja"
                        corTitulo: Tema.verdeEscuro
                        color: Tema.verde
                        Layout.fillWidth: true
                        Layout.preferredWidth: 1
                        Text {
                            text: Tema.sigilo(janelaConflito.conflito ? janelaConflito.conflito.lojaDocumento : "")
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.DemiBold
                            font.pixelSize: 20
                        }
                        Nota {
                            text: janelaConflito.conflito ? janelaConflito.conflito.lojaNome : ""
                            visible: text !== ""
                            Layout.fillWidth: true
                        }
                    }
                    Faixa {
                        titulo: "CNPJ no XML (destinatário)"
                        corTitulo: Tema.vermelho
                        color: Tema.rosa
                        Layout.fillWidth: true
                        Layout.preferredWidth: 1
                        Text {
                            text: Tema.sigilo(janelaConflito.conflito ? janelaConflito.conflito.destinatarioDocumento : "")
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.DemiBold
                            font.pixelSize: 20
                        }
                        Nota {
                            text: janelaConflito.conflito ? janelaConflito.conflito.destinatarioNome : ""
                            visible: text !== ""
                            Layout.fillWidth: true
                        }
                    }
                }

                Faixa {
                    titulo: "A nota"
                    Layout.fillWidth: true
                    GridLayout {
                        columns: 2
                        columnSpacing: 18
                        rowSpacing: 6
                        Layout.fillWidth: true
                        Repeater {
                            model: {
                                const c = janelaConflito.conflito
                                if (!c)
                                    return []
                                return [
                                    ["Emitente (fornecedor)", c.emitenteNome],
                                    ["CNPJ do emitente", Tema.sigilo(c.emitenteDocumento)],
                                    ["Número / série", c.numero + (c.serie ? " / " + c.serie : "")],
                                    ["Emissão", c.emissao],
                                    ["Valor total", "R$ " + Tema.valor(c.total)]
                                ]
                            }
                            delegate: RowLayout {
                                required property var modelData
                                Layout.columnSpan: 2
                                Layout.fillWidth: true
                                spacing: 18
                                Nota {
                                    text: modelData[0]
                                    Layout.preferredWidth: Tema.estreito ? 130 : 190
                                }
                                Text {
                                    text: modelData[1]
                                    color: Tema.tinta
                                    font.family: Tema.texto
                                    font.pixelSize: 15
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                            }
                        }
                    }
                }

                // Conferir: os itens da nota, só para ler.
                Faixa {
                    visible: janelaConflito.conferindo
                    titulo: janelaConflito.conflito ? "Itens da nota (" + janelaConflito.conflito.itens.length + ")" : ""
                    Layout.fillWidth: true
                    ListView {
                        id: itens
                        Layout.fillWidth: true
                        Layout.preferredHeight: Math.min(contentHeight, 240)
                        clip: true
                        spacing: 4
                        boundsBehavior: Flickable.StopAtBounds
                        ScrollBar.vertical: ScrollBar {}
                        model: janelaConflito.conflito ? janelaConflito.conflito.itens : []
                        delegate: RowLayout {
                            required property var modelData
                            width: itens.width
                            spacing: 12
                            Text {
                                text: modelData.codigo
                                color: Tema.tintaSuave
                                font.family: Tema.texto
                                font.pixelSize: 14
                                Layout.preferredWidth: 90
                                elide: Text.ElideRight
                            }
                            Text {
                                text: modelData.descricao
                                color: Tema.tinta
                                font.family: Tema.texto
                                font.pixelSize: 14
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                            }
                            Text {
                                text: modelData.quantidade + " " + modelData.unidade
                                color: Tema.tinta
                                font.family: Tema.texto
                                font.pixelSize: 14
                                horizontalAlignment: Text.AlignRight
                                Layout.preferredWidth: 90
                            }
                            Text {
                                text: "R$ " + Tema.valor(modelData.total)
                                color: Tema.tinta
                                font.family: Tema.texto
                                font.pixelSize: 14
                                horizontalAlignment: Text.AlignRight
                                Layout.preferredWidth: 100
                            }
                        }
                    }
                }

                Faixa {
                    color: Tema.atencao
                    Layout.fillWidth: true
                    Nota {
                        text: "A nota não foi importada e nada foi gravado. O CNPJ da loja não muda."
                        Layout.fillWidth: true
                    }
                }
            }
        }

        Flow {
            spacing: 12
            Layout.fillWidth: true
            layoutDirection: Qt.RightToLeft
            Botao {
                text: janelaConflito.conferindo ? "Esconder os itens" : "Conferir nota"
                icone: "documento"
                onClicked: janelaConflito.conferindo = !janelaConflito.conferindo
            }
            Botao {
                text: "Descartar nota"
                tipo: "perigo"
                onClicked: {
                    const c = janelaConflito.conflito
                    janelaConflito.close()
                    janelaConflito.descartar(c)
                }
            }
            Botao {
                text: "Cancelar"
                tipo: "secundario"
                atalho: "Esc"
                onClicked: janelaConflito.close()
            }
        }
    }
}
