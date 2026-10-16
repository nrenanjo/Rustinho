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

        RowLayout {
            spacing: 12
            Layout.fillWidth: true

            Text {
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
                Text {
                    text: linha.item.descricao || ""
                    color: Tema.tinta
                    font.family: Tema.titulo
                    font.weight: Font.Bold
                    font.pixelSize: 20
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                    TapHandler { onTapped: linha.abrirPeca() }
                    HoverHandler { cursorShape: linha.item.servico ? Qt.ArrowCursor : Qt.PointingHandCursor }
                }
                RowLayout {
                    spacing: 8
                    Text {
                        text: [linha.item.codigo, linha.item.marca, linha.item.local ? "local " + linha.item.local : ""].filter(x => !!x).join("  ·  ")
                        color: Tema.tintaSuave
                        font.family: Tema.serifa
                        font.italic: true
                        font.pixelSize: 13
                        elide: Text.ElideRight
                        Layout.maximumWidth: 320
                    }
                    Rectangle {
                        radius: height / 2
                        color: linha.item.servico ? Tema.verde : linha.item.faltaEstoque ? Tema.rosa : Qt.rgba(1, 1, 1, 0.45)
                        implicitWidth: textoEstoque.implicitWidth + 14
                        implicitHeight: textoEstoque.implicitHeight + 4
                        Text {
                            id: textoEstoque
                            anchors.centerIn: parent
                            readonly property bool semNada: (linha.item.estoque || "0") === "0" || (linha.item.estoque || "").startsWith("-")
                            text: linha.item.servico ? "serviço (mão de obra)"
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
                        color: linha.item.encomendar ? Tema.verde : sobreEnc.hovered ? Tema.papel : Qt.rgba(1, 1, 1, 0.45)
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
                Layout.preferredWidth: 96
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
                Layout.preferredWidth: 118
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
                Layout.preferredWidth: 92
                onConfirmado: linha.mudou("desconto", texto)
                onSaiu: if (texto !== (linha.entrada.desconto || "")) linha.mudou("desconto", texto)
            }
            Text {
                text: "R$ " + (linha.item.total || "0,00")
                color: Tema.tinta
                font.family: Tema.texto
                font.weight: Font.Bold
                font.pixelSize: 18
                horizontalAlignment: Text.AlignRight
                Layout.preferredWidth: 116
            }
            Botao {
                tipo: "discreto"
                icone: "lixeira"
                implicitWidth: 36
                leftPadding: 0
                rightPadding: 0
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
            Layout.leftMargin: 38
        }
    }
}
