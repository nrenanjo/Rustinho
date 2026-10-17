import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Um item da nota de compra e o que fazer com ele.
Rectangle {
    id: linha
    property var item: ({})
    property bool editavel: true

    signal escolherPeca()
    signal cadastrarNova()
    signal naoLancar()
    signal fatorMudou(string valor)
    signal baixarPrecoMudou(bool baixar)
    signal abrirPeca(string produtoId)

    readonly property bool pendente: item.acao === ""
    readonly property bool ignorado: item.acao === "ignorar"
    readonly property var peca: item.peca || null

    radius: Tema.raio
    color: pendente ? Tema.rosa : ignorado ? Tema.vela(0.25) : (item.ordem % 2 === 0 ? Tema.azul : Tema.pergaminho)
    implicitHeight: coluna.implicitHeight + 32

    function focarFator() {
        fator.focar()
    }

    ColumnLayout {
        id: coluna
        x: 20
        y: 16
        width: parent.width - 40
        spacing: 10

        // O que veio na nota
        RowLayout {
            spacing: 14
            Layout.fillWidth: true
            Text {
                text: linha.item.ordem
                color: Tema.dourado
                font.family: Tema.titulo
                font.weight: Font.Bold
                font.pixelSize: Tema.estreito ? 24 : 30
                Layout.preferredWidth: Tema.estreito ? 20 : 30
                Layout.alignment: Qt.AlignTop
            }
            ColumnLayout {
                spacing: 1
                Layout.fillWidth: true
                Text {
                    text: linha.item.descricao
                    color: Tema.tinta
                    font.family: Tema.titulo
                    font.weight: Font.Bold
                    font.pixelSize: Tema.estreito ? 18 : 21
                    elide: Text.ElideRight
                    wrapMode: Tema.estreito ? Text.WordWrap : Text.NoWrap
                    maximumLineCount: 2
                    Layout.fillWidth: true
                }
                Nota {
                    text: ["Cód. " + linha.item.codigoFornecedor, linha.item.gtin ? "Barras " + linha.item.gtin : "",
                           linha.item.ncm ? "NCM " + linha.item.ncm : "", linha.item.cest ? "CEST " + linha.item.cest : "",
                           linha.item.monofasico ? "monofásico" : ""].filter(x => !!x).join("  ·  ")
                    font.pixelSize: 14
                    wrapMode: Text.NoWrap
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }
            ColumnLayout {
                spacing: 1
                Layout.alignment: Qt.AlignTop
                Text {
                    text: linha.item.quantidade + " " + linha.item.unidade + " × R$ " + Tema.valor(linha.item.valorUnitario)
                    color: Tema.tintaSuave
                    font.family: Tema.texto
                    font.pixelSize: 15
                    Layout.alignment: Qt.AlignRight
                }
                Text {
                    text: "R$ " + Tema.valor(linha.item.valorProdutos)
                    color: Tema.tinta
                    font.family: Tema.texto
                    font.weight: Font.Bold
                    font.pixelSize: 17
                    Layout.alignment: Qt.AlignRight
                }
            }
        }

        // A decisão
        Rectangle {
            Layout.fillWidth: true
            Layout.leftMargin: Tema.estreito ? 0 : 44
            radius: Tema.raioCampo
            color: Tema.vela(0.42)
            implicitHeight: decisao.implicitHeight + 20

            ColumnLayout {
                id: decisao
                x: 14
                y: 10
                width: parent.width - 28
                spacing: 8

                RowLayout {
                    spacing: 10
                    Layout.fillWidth: true
                    Icone {
                        nome: linha.pendente ? "ajuste" : linha.ignorado ? "fechar" : linha.item.acao === "nova" ? "mais" : "check"
                        cor: linha.pendente ? Tema.vermelho : linha.ignorado ? Tema.apagado : Tema.verdeEscuro
                        tamanho: 20
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        Text {
                            text: {
                                if (linha.pendente)
                                    return "Que peça é esta?"
                                if (linha.ignorado)
                                    return "Não entra no estoque"
                                if (linha.item.acao === "nova")
                                    return "Vai ser cadastrada como peça nova"
                                return linha.peca ? linha.peca.codigo + " · " + linha.peca.nome + (linha.peca.marca ? " (" + linha.peca.marca + ")" : "") : "Peça não encontrada"
                            }
                            color: linha.pendente ? Tema.vermelho : Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.DemiBold
                            font.pixelSize: 16
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                            TapHandler {
                                enabled: !!linha.peca
                                onTapped: linha.abrirPeca(linha.peca.id)
                            }
                            HoverHandler { cursorShape: linha.peca ? Qt.PointingHandCursor : Qt.ArrowCursor }
                        }
                        Nota {
                            visible: text !== ""
                            text: {
                                switch (linha.item.achadoPor) {
                                case "fornecedor": return "Ligada sozinha: mesmo código deste fornecedor na última compra"
                                case "barras": return "Ligada sozinha pelo código de barras"
                                case "codigo": return "Ligada sozinha pelo código original — confira"
                                }
                                if (linha.pendente)
                                    return "Não achei esta peça pelo código nem pelo código de barras."
                                if (linha.item.acao === "nova")
                                    return "Com o nome, NCM, CEST e código de barras da nota, e o preço pelo lucro padrão."
                                if (linha.ignorado)
                                    return "Use para material de uso da loja, brinde ou item que não é vendido."
                                return ""
                            }
                            font.pixelSize: 13
                            Layout.fillWidth: true
                        }
                    }
                    Botao {
                        visible: linha.editavel
                        text: linha.peca || linha.item.acao === "ligar" ? "Trocar peça" : "Escolher peça"
                        tipo: linha.pendente ? "principal" : "secundario"
                        implicitHeight: 36
                        onClicked: linha.escolherPeca()
                    }
                    Botao {
                        visible: linha.editavel && linha.item.acao !== "nova"
                        text: "Cadastrar nova"
                        tipo: linha.pendente ? "secundario" : "discreto"
                        implicitHeight: 36
                        onClicked: linha.cadastrarNova()
                    }
                    Botao {
                        visible: linha.editavel && !linha.ignorado
                        text: "Não lançar"
                        tipo: "discreto"
                        implicitHeight: 36
                        onClicked: linha.naoLancar()
                    }
                }

                // Quantidade e custo
                Linha {
                    visible: !linha.ignorado && !linha.pendente
                    spacing: 12
                    Layout.fillWidth: true
                    Layout.leftMargin: Tema.estreito ? 0 : 30
                    Text {
                        text: "Cada " + linha.item.unidade + " da nota tem"
                        color: Tema.tintaSuave
                        font.family: Tema.texto
                        font.pixelSize: 14
                    }
                    Campo {
                        id: fator
                        texto: linha.item.fator
                        numerico: true
                        maximo: 8
                        altura: 32
                        tamanhoFonte: 15
                        somenteLeitura: !linha.editavel
                        Layout.preferredWidth: 70
                        onConfirmado: linha.fatorMudou(texto)
                        onSaiu: if (texto !== linha.item.fator) linha.fatorMudou(texto)
                    }
                    Text {
                        text: (linha.item.fator === "1" ? "peça" : "peças") + "  →  entram " + linha.item.quantidadePecas
                              + " " + (linha.peca ? linha.peca.unidade : "") + " no estoque"
                        color: Tema.tintaSuave
                        font.family: Tema.texto
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                    ColumnLayout {
                        spacing: 0
                        Text {
                            text: "Custo R$ " + Tema.valor(linha.item.custoUnitario) + " cada"
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.Bold
                            font.pixelSize: 15
                            Layout.alignment: Qt.AlignRight
                        }
                        Text {
                            visible: linha.editavel && !!linha.peca && linha.peca.custoAtual !== ""
                            text: linha.peca ? "antes R$ " + Tema.valor(linha.peca.custoAtual) + (linha.peca.variacaoCusto && !Tema.oculto ? "  (" + linha.peca.variacaoCusto + "%)" : "") : ""
                            color: linha.peca && linha.peca.custoSubiu ? Tema.vermelho : Tema.tintaSuave
                            font.family: Tema.texto
                            font.pixelSize: 13
                            Layout.alignment: Qt.AlignRight
                        }
                    }
                    ColumnLayout {
                        visible: !!linha.peca
                        spacing: 0
                        Layout.leftMargin: 8
                        Text {
                            text: linha.peca ? (linha.editavel ? "Revenda R$ " + linha.peca.precoNovo : "Revenda hoje R$ " + linha.peca.precoAtual) : ""
                            color: Tema.verdeEscuro
                            font.family: Tema.texto
                            font.weight: Font.Bold
                            font.pixelSize: 15
                            Layout.alignment: Qt.AlignRight
                        }
                        Text {
                            visible: linha.editavel && !!linha.peca && linha.peca.precoAtual !== linha.peca.precoNovo
                            text: linha.peca ? "era R$ " + linha.peca.precoAtual : ""
                            color: Tema.tintaSuave
                            font.family: Tema.texto
                            font.pixelSize: 13
                            Layout.alignment: Qt.AlignRight
                        }
                    }
                }

                // O custo baixou: o preço baixa junto?
                Rectangle {
                    visible: linha.editavel && !!linha.peca && linha.peca.custoBaixou && linha.item.acao === "ligar"
                    Layout.fillWidth: true
                    Layout.leftMargin: Tema.estreito ? 0 : 30
                    radius: Tema.raioCampo
                    color: Tema.verde
                    implicitHeight: perguntaPreco.implicitHeight + 16
                    Linha {
                        id: perguntaPreco
                        x: 12
                        y: 8
                        width: parent.width - 24
                        spacing: 16
                        Text {
                            text: "O custo baixou. O preço de revenda:"
                            color: Tema.verdeEscuro
                            font.family: Tema.texto
                            font.weight: Font.DemiBold
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }
                        ButtonGroup { id: grupoPreco }
                        Marcar {
                            text: linha.peca ? "fica R$ " + linha.peca.precoAtual : ""
                            checked: !!linha.peca && !linha.peca.baixarPreco
                            font.pixelSize: 14
                            ButtonGroup.group: grupoPreco
                            onToggled: if (checked) linha.baixarPrecoMudou(false)
                        }
                        Marcar {
                            text: linha.peca ? "baixa para R$ " + linha.peca.precoSeBaixar : ""
                            checked: !!linha.peca && linha.peca.baixarPreco
                            font.pixelSize: 14
                            ButtonGroup.group: grupoPreco
                            onToggled: if (checked) linha.baixarPrecoMudou(true)
                        }
                    }
                }
            }
        }
    }
}
