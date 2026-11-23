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
    property bool descartando: false
    width: Math.min(720, Tema.largura - 24)
    // Nunca mais alta que a janela: o conteúdo rola e os botões ficam à vista.
    height: Math.min(implicitHeight, Tema.altura - 24)
    padding: Tema.estreito ? 16 : 20
    bottomPadding: Tema.estreito ? 14 : 18
    signal descartar(var conflito)

    function mostrar(c) {
        conflito = c
        conferindo = false
        descartando = false
        open()
    }
    onClosed: descartando = false

    contentItem: ColumnLayout {
        spacing: 12

        Flickable {
            id: rolagem
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.preferredHeight: corpo.implicitHeight
            contentHeight: corpo.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {
                policy: rolagem.contentHeight > rolagem.height + 1 ? ScrollBar.AlwaysOn : ScrollBar.AsNeeded
            }

            ColumnLayout {
                id: corpo
                width: rolagem.width
                spacing: 8

                RowLayout {
                    spacing: 14
                    Layout.fillWidth: true
                    Rectangle {
                        Layout.preferredWidth: 52
                        Layout.preferredHeight: 52
                        Layout.alignment: Qt.AlignTop
                        radius: Tema.raio
                        color: Tema.atencao
                        border.color: Tema.bordaAtencao
                        Image {
                            anchors.centerIn: parent
                            width: 30
                            height: 30
                            source: Tema.escuro ? "qrc:/qt/qml/rustinho/icone/alerta-branco.png" : "qrc:/qt/qml/rustinho/icone/alerta-preto.png"
                            fillMode: Image.PreserveAspectFit
                            smooth: true
                            mipmap: true
                        }
                    }
                    ColumnLayout {
                        spacing: 4
                        Layout.fillWidth: true
                        Titulo {
                            text: "CNPJ da nota é diferente"
                            font.pixelSize: Tema.estreito ? 24 : 28
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
                    text: janelaConflito.conflito
                          ? (janelaConflito.conflito.arquivos && janelaConflito.conflito.arquivos.length > 1
                             ? janelaConflito.conflito.arquivos.join(" · ")
                             : janelaConflito.conflito.arquivo)
                          : ""
                    font.pixelSize: 14
                    elide: Text.ElideMiddle
                    wrapMode: Text.NoWrap
                    Layout.fillWidth: true
                }

                // Ponto 9 do doc 12: os dois cartões de CNPJ com mesmo topo e mesma altura.
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
                        Layout.fillHeight: true
                        Layout.alignment: Qt.AlignTop
                        Layout.preferredWidth: 1
                        Text {
                            text: Tema.sigilo(janelaConflito.conflito ? janelaConflito.conflito.lojaDocumento : "")
                            textFormat: Text.PlainText
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
                        Item { Layout.fillHeight: true }
                    }
                    Faixa {
                        titulo: "CNPJ no XML (destinatário)"
                        corTitulo: Tema.vermelho
                        color: Tema.rosa
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        Layout.alignment: Qt.AlignTop
                        Layout.preferredWidth: 1
                        Text {
                            text: Tema.sigilo(janelaConflito.conflito ? janelaConflito.conflito.destinatarioDocumento : "")
                            textFormat: Text.PlainText
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
                        Item { Layout.fillHeight: true }
                    }
                }
                // D-IMG-3: avisa como revelar os CNPJs sem mudar a regra de sigilo ("Mostrar valores").
                Nota {
                    visible: Tema.oculto
                    text: "Os CNPJs estão escondidos. Use “Mostrar valores” no canto superior para comparar."
                    font.pixelSize: 13
                    Layout.fillWidth: true
                }

                Faixa {
                    titulo: "A nota"
                    Layout.fillWidth: true
                    GridLayout {
                        columns: 2
                        columnSpacing: 18
                        rowSpacing: 3
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
                                    textFormat: Text.PlainText
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
                        delegate: Rectangle {
                            id: faixaDaLista
                            required property var modelData
                            width: itens.width
                            required property int index
                            // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                            readonly property bool par: (index + 1) % 2 === 0
                            color: Tema.faixa(faixaDaLista.par, Tema.vela(0.4), "transparent")
                            palette.window: Tema.superficie(color)
                            radius: Tema.raioCampo
                            implicitHeight: conteudoDaFaixa.implicitHeight + 8
                            implicitWidth: conteudoDaFaixa.implicitWidth + 16
                            RowLayout {
                                id: conteudoDaFaixa
                                x: 8
                                y: 4
                                width: faixaDaLista.width - 16
                                spacing: 12
                                Text {
                                    text: modelData.codigo
                                    textFormat: Text.PlainText
                                    color: Tema.tintaSuaveDaFaixa(faixaDaLista.par)
                                    font.family: Tema.texto
                                    font.pixelSize: 14
                                    Layout.preferredWidth: 90
                                    elide: Text.ElideRight
                                }
                                Text {
                                    text: modelData.descricao
                                    textFormat: Text.PlainText
                                    color: Tema.tintaDaFaixa(faixaDaLista.par)
                                    font.family: Tema.texto
                                    font.pixelSize: 14
                                    Layout.fillWidth: true
                                    elide: Text.ElideRight
                                }
                                Text {
                                    text: modelData.quantidade + " " + modelData.unidade
                                    textFormat: Text.PlainText
                                    color: Tema.tintaDaFaixa(faixaDaLista.par)
                                    font.family: Tema.texto
                                    font.pixelSize: 14
                                    horizontalAlignment: Text.AlignRight
                                    Layout.preferredWidth: 90
                                }
                                Text {
                                    text: "R$ " + Tema.valor(modelData.total)
                                    textFormat: Text.PlainText
                                    color: Tema.tintaDaFaixa(faixaDaLista.par)
                                    font.family: Tema.texto
                                    font.pixelSize: 14
                                    horizontalAlignment: Text.AlignRight
                                    Layout.preferredWidth: 100
                                }
                            }
                        }
                    }
                }

                // Aviso compacto (sem título de Faixa) para caber inteiro em 1280x760 sem cortar nem rolar.
                Rectangle {
                    color: Tema.atencao
                    palette.window: Tema.superficie(color)
                    radius: Tema.raio
                    Layout.fillWidth: true
                    implicitHeight: avisoNadaGravado.implicitHeight + 16
                    Nota {
                        id: avisoNadaGravado
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.leftMargin: 14
                        anchors.rightMargin: 14
                        text: "A nota não foi importada e nada foi gravado. O CNPJ da loja não muda."
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
                enabled: !janelaConflito.descartando
                onClicked: {
                    if (janelaConflito.descartando || !janelaConflito.conflito)
                        return
                    janelaConflito.descartando = true
                    const c = janelaConflito.conflito
                    janelaConflito.close()
                    janelaConflito.descartar(c)
                }
            }
        }
    }
}
