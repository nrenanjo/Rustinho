import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Clientes no Painel: quem mais compra no período, quantas vezes, quando foi
// a última compra e se está devendo.
FocusScope {
    id: secao
    readonly property var janela: ApplicationWindow.window
    property int dias: 30
    property var dados: null

    function carregar() {
        const r = Api.chamar("painel_clientes", { dias: dias })
        if (r.ok)
            dados = r.dados
        else
            janela.avisar(r.erro, "erro")
    }
    onDiasChanged: carregar()

    ColumnLayout {
        anchors.fill: parent
        spacing: 12

        Linha {
            colunasEstreito: 2
            spacing: 12
            Layout.fillWidth: true
            CartaoNumero {
                rotulo: "Vendido com cliente"
                sensivel: true
                valor: secao.dados ? secao.dados.totalComCliente : ""
                nota: secao.dados && secao.dados.parteComCliente ? secao.dados.parteComCliente + " das vendas" : ""
                cor: Tema.verde
                corValor: Tema.verdeEscuro
            }
            CartaoNumero { rotulo: "Clientes que compraram"; prefixo: ""; valor: secao.dados ? String(secao.dados.clientes) : "" }
            CartaoNumero { rotulo: "Clientes novos"; prefixo: ""; valor: secao.dados ? String(secao.dados.novos) : ""; nota: "cadastrados"; cor: Tema.azul }
            CartaoNumero { rotulo: "Ticket médio"; sensivel: true; valor: secao.dados ? secao.dados.ticket : ""; cor: Tema.pergaminho }
        }

        ListView {
            id: lista
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 6
            model: secao.dados ? secao.dados.itens : []
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                id: linha
                required property var modelData
                required property int index
                readonly property var c: modelData
                width: lista.width
                implicitHeight: 64
                radius: Tema.raio
                color: (index + 1) % 2 === 0 ? Tema.azul : Tema.pergaminho
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 16
                    anchors.rightMargin: 16
                    spacing: 14
                    Text {
                        text: linha.c.posicao
                        color: Tema.dourado
                        font.family: Tema.titulo
                        font.weight: Font.Bold
                        font.pixelSize: 24
                        horizontalAlignment: Text.AlignRight
                        Layout.preferredWidth: 38
                    }
                    ColumnLayout {
                        spacing: 3
                        Layout.fillWidth: true
                        Text {
                            text: linha.c.nome
                            color: Tema.tinta
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: 20
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                            TapHandler { onTapped: secao.janela.abrir("TelaPessoa", { pessoaId: linha.c.clienteId, papel: "cliente" }) }
                            HoverHandler { cursorShape: Qt.PointingHandCursor }
                        }
                        Item {
                            Layout.fillWidth: true
                            implicitHeight: 6
                            Rectangle {
                                height: 6
                                radius: 3
                                width: Math.max(6, parent.width * linha.c.parte)
                                color: Tema.verdeEscuro
                                opacity: 0.4
                            }
                        }
                    }
                    Chip {
                        texto: linha.c.deve ? "deve R$ " + Tema.valor(linha.c.deve) : ""
                        color: linha.c.vencido ? Tema.rosa : Qt.rgba(1, 1, 1, 0.5)
                        corTexto: linha.c.vencido ? Tema.vermelho : Tema.tintaSuave
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.preferredWidth: 150
                        Text {
                            text: linha.c.compras + (linha.c.compras === 1 ? " compra" : " compras")
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.DemiBold
                            font.pixelSize: 15
                            Layout.alignment: Qt.AlignRight
                        }
                        Text {
                            text: "última " + linha.c.ultima
                            color: Tema.tintaSuave
                            font.family: Tema.texto
                            font.pixelSize: 12
                            Layout.alignment: Qt.AlignRight
                        }
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.preferredWidth: 140
                        Text {
                            text: "R$ " + Tema.valor(linha.c.total)
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.Bold
                            font.pixelSize: 17
                            Layout.alignment: Qt.AlignRight
                        }
                        Text {
                            text: "média R$ " + Tema.valor(linha.c.ticket)
                            color: Tema.tintaSuave
                            font.family: Tema.texto
                            font.pixelSize: 12
                            Layout.alignment: Qt.AlignRight
                        }
                    }
                }
            }
            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(parent.width - 40, 600)
                visible: !!secao.dados && lista.count === 0
                spacing: 8
                Titulo {
                    text: "Nenhuma venda com cliente no período"
                    font.pixelSize: 30
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Nota {
                    text: "Aparece aqui quem comprou com o nome escolhido na venda (cliente cadastrado)."
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
            }
        }
        Nota {
            text: "Só entram as vendas com cliente cadastrado. Clique no nome para abrir o cadastro."
            font.pixelSize: 12
            Layout.fillWidth: true
        }
    }

    Component.onCompleted: carregar()
}
