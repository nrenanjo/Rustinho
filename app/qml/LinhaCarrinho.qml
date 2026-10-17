import QtQuick
import QtQuick.Layouts
import rustinho

// Um item do carrinho: peça, quantidade, preço, desconto e total.
Rectangle {
    id: linha
    property var item: ({})
    property var entrada: ({})
    property int indice: 0
    property bool atual: false

    signal mudou(string campo, string valor)
    signal remover()
    signal abrirPeca()
    signal selecionar()
    signal encomendar()

    readonly property bool par: (indice + 1) % 2 === 0
    radius: Tema.raio
    color: item.erro ? Tema.rosa : atual ? Qt.darker(par ? Tema.azul : Tema.pergaminho, 1.05) : (par ? Tema.azul : Tema.pergaminho)
    border.width: atual ? 2 : 0
    border.color: Tema.azulEscuro
    implicitHeight: coluna.implicitHeight + 18

    function focarQuantidade() {
        quantidade.focar()
    }

    ColumnLayout {
        id: coluna
        x: 16
        y: 9
        width: parent.width - 28
        spacing: 2

        // Computador: tudo numa linha. Celular: peça e total em cima;
        // quantidade, preço, desconto e lixeira embaixo.
        GridLayout {
            readonly property bool e: Tema.estreito
            columns: e ? 4 : 7
            columnSpacing: e ? 8 : 12
            rowSpacing: 6
            Layout.fillWidth: true

            Text {
                visible: !parent.e
                Layout.row: 0
                Layout.column: 0
                text: linha.indice + 1
                color: Tema.dourado
                font.family: Tema.titulo
                font.weight: Font.Bold
                font.pixelSize: 24
                Layout.preferredWidth: 26
            }
            ColumnLayout {
                spacing: 0
                Layout.fillWidth: true
                Layout.row: 0
                Layout.column: parent.e ? 0 : 1
                Layout.columnSpan: parent.e ? 3 : 1
                Text {
                    text: linha.item.descricao || ""
                    color: Tema.tinta
                    font.family: Tema.titulo
                    font.weight: Font.Bold
                    font.pixelSize: 20
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                    TapHandler { onTapped: linha.abrirPeca() }
                    HoverHandler { cursorShape: linha.item.produtoId ? Qt.PointingHandCursor : Qt.ArrowCursor }
                }
                Flow {
                    spacing: 8
                    Layout.fillWidth: true
                    Text {
                        width: Math.min(implicitWidth, Tema.estreito ? parent.width : 320)
                        text: [linha.item.codigo, linha.item.marca, linha.item.local ? "local " + linha.item.local : ""].filter(x => !!x).join("  ·  ")
                        color: Tema.tintaSuave
                        font.family: Tema.serifa
                        font.italic: true
                        font.pixelSize: 13
                        elide: Text.ElideRight
                    }
                    Rectangle {
                        radius: height / 2
                        color: linha.item.servico ? Tema.verde : linha.item.faltaEstoque ? Tema.rosa : Tema.vela(0.45)
                        implicitWidth: textoEstoque.implicitWidth + 14
                        implicitHeight: textoEstoque.implicitHeight + 4
                        Text {
                            id: textoEstoque
                            anchors.centerIn: parent
                            readonly property bool semNada: (linha.item.estoque || "0") === "0" || (linha.item.estoque || "").startsWith("-")
                            text: linha.item.servico ? (linha.item.produtoId ? "serviço" : "serviço (mão de obra)")
                                  : linha.item.faltaEstoque && semNada ? "sem estoque"
                                  : (linha.item.faltaEstoque ? "só " : "") + (linha.item.estoque || "0") + " " + (linha.item.unidade || "") + " no estoque"
                            color: linha.item.servico ? Tema.verdeEscuro : linha.item.faltaEstoque ? Tema.vermelho : Tema.tintaSuave
                            font.family: Tema.texto
                            font.weight: Font.DemiBold
                            font.pixelSize: 12
                        }
                    }
                    // Falta no estoque: o cliente pode encomendar.
                    Rectangle {
                        visible: !!linha.item.faltaEstoque
                        radius: height / 2
                        color: linha.item.encomendar ? Tema.verde : sobreEnc.hovered ? Tema.papel : Tema.vela(0.45)
                        implicitWidth: linhaEnc.implicitWidth + 16
                        implicitHeight: textoEstoque.implicitHeight + 4
                        Row {
                            id: linhaEnc
                            anchors.centerIn: parent
                            spacing: 4
                            Icone {
                                visible: !!linha.item.encomendar
                                nome: "check"
                                tamanho: 13
                                espessura: 2.2
                                cor: Tema.verdeEscuro
                                anchors.verticalCenter: parent.verticalCenter
                            }
                            Text {
                                text: linha.item.encomendar ? "Encomendar " + linha.item.falta + " " + (linha.item.unidade || "") : "Encomendar?"
                                color: linha.item.encomendar ? Tema.verdeEscuro : Tema.tintaSuave
                                font.family: Tema.texto
                                font.weight: Font.DemiBold
                                font.pixelSize: 12
                            }
                        }
                        HoverHandler { id: sobreEnc; cursorShape: Qt.PointingHandCursor }
                        TapHandler { onTapped: linha.encomendar() }
                    }
                }
            }
            Campo {
                id: quantidade
                texto: linha.entrada.quantidade || ""
                numerico: true
                maximo: 10
                altura: 36
                sufixo: linha.item.unidade || ""
                Layout.preferredWidth: parent.e ? 78 : 96
                Layout.row: parent.e ? 1 : 0
                Layout.column: parent.e ? 0 : 2
                onConfirmado: linha.mudou("quantidade", texto)
                onSaiu: if (texto !== (linha.entrada.quantidade || "")) linha.mudou("quantidade", texto)
            }
            Campo {
                id: preco
                texto: linha.entrada.preco !== "" && linha.entrada.preco !== undefined ? linha.entrada.preco : (linha.item.preco || "")
                prefixo: "R$"
                numerico: true
                maximo: 14
                altura: 36
                Layout.preferredWidth: parent.e ? 104 : 118
                Layout.row: parent.e ? 1 : 0
                Layout.column: parent.e ? 1 : 3
                onConfirmado: linha.mudou("preco", texto)
                onSaiu: if (texto !== (linha.item.preco || "")) linha.mudou("preco", texto)
            }
            Campo {
                id: desconto
                texto: linha.entrada.desconto || ""
                dica: "R$ ou %"
                numerico: true
                maximo: 10
                altura: 36
                Layout.preferredWidth: parent.e ? -1 : 92
                Layout.fillWidth: parent.e
                Layout.row: parent.e ? 1 : 0
                Layout.column: parent.e ? 2 : 4
                onConfirmado: linha.mudou("desconto", texto)
                onSaiu: if (texto !== (linha.entrada.desconto || "")) linha.mudou("desconto", texto)
            }
            Text {
                text: "R$ " + (linha.item.total || "0,00")
                color: Tema.tinta
                font.family: Tema.texto
                font.weight: Font.Bold
                font.pixelSize: parent.e ? 16 : 18
                horizontalAlignment: Text.AlignRight
                Layout.preferredWidth: parent.e ? -1 : 116
                Layout.alignment: parent.e ? Qt.AlignRight | Qt.AlignTop : Qt.AlignVCenter
                Layout.row: 0
                Layout.column: parent.e ? 3 : 5
            }
            Botao {
                tipo: "discreto"
                icone: "lixeira"
                implicitWidth: 36
                leftPadding: 0
                rightPadding: 0
                Layout.row: parent.e ? 1 : 0
                Layout.column: parent.e ? 3 : 6
                onClicked: linha.remover()
            }
        }
        Text {
            visible: !!linha.item.erro
            text: linha.item.erro || ""
            color: Tema.vermelho
            font.family: Tema.texto
            font.weight: Font.DemiBold
            font.pixelSize: 13
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
            Layout.leftMargin: Tema.estreito ? 0 : 38
        }
    }
}
