import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Fornecedores no Painel: de quem a loja mais compra (notas lançadas).
FocusScope {
    id: secao
    readonly property var janela: ApplicationWindow.window
    property int dias: 30
    property var dados: null

    function carregar() {
        const r = Api.chamar("painel_fornecedores", { dias: dias })
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
            CartaoNumero { rotulo: "Comprado"; sensivel: true; valor: secao.dados ? secao.dados.total : ""; nota: "notas lançadas"; cor: Tema.rosa }
            CartaoNumero { rotulo: "Notas"; prefixo: ""; valor: secao.dados ? String(secao.dados.notas) : "" }
            CartaoNumero { rotulo: "Fornecedores"; prefixo: ""; valor: secao.dados ? String(secao.dados.fornecedores) : ""; cor: Tema.azul }
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
                readonly property var f: modelData
                width: lista.width
                implicitHeight: 64
                radius: Tema.raio
                color: (index + 1) % 2 === 0 ? Tema.azul : Tema.pergaminho
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Tema.estreito ? 10 : 16
                    anchors.rightMargin: 16
                    spacing: Tema.estreito ? 8 : 14
                    Text {
                        text: linha.f.posicao
                        color: Tema.dourado
                        font.family: Tema.titulo
                        font.weight: Font.Bold
                        font.pixelSize: 24
                        horizontalAlignment: Text.AlignRight
                        Layout.preferredWidth: Tema.estreito ? 24 : 38
                    }
                    ColumnLayout {
                        spacing: 3
                        Layout.fillWidth: true
                        RowLayout {
                            spacing: 10
                            Layout.fillWidth: true
                            Text {
                                text: linha.f.nome
                                color: Tema.tinta
                                font.family: Tema.titulo
                                font.weight: Font.Bold
                                font.pixelSize: 20
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                                Layout.maximumWidth: implicitWidth
                                TapHandler { onTapped: secao.janela.abrir("TelaPessoa", { pessoaId: linha.f.fornecedorId, papel: "fornecedor" }) }
                                HoverHandler { cursorShape: Qt.PointingHandCursor }
                            }
                            Text {
                                text: linha.f.cidade
                                color: Tema.tintaSuave
                                font.family: Tema.serifa
                                font.italic: true
                                font.pixelSize: 13
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                            }
                        }
                        Item {
                            Layout.fillWidth: true
                            implicitHeight: 6
                            Rectangle {
                                height: 6
                                radius: 3
                                width: Math.max(6, parent.width * linha.f.parte)
                                color: Tema.azulEscuro
                                opacity: 0.4
                            }
                        }
                    }
                    ColumnLayout {
                        visible: !Tema.estreito
                        spacing: 0
                        Layout.preferredWidth: 170
                        Text {
                            text: linha.f.notas + (linha.f.notas === 1 ? " nota" : " notas") + " · " + linha.f.pecas + (linha.f.pecas === 1 ? " peça" : " peças")
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.DemiBold
                            font.pixelSize: 15
                            Layout.alignment: Qt.AlignRight
                        }
                        Text {
                            text: "última " + linha.f.ultima
                            color: Tema.tintaSuave
                            font.family: Tema.texto
                            font.pixelSize: 12
                            Layout.alignment: Qt.AlignRight
                        }
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.preferredWidth: Tema.estreito ? -1 : 150
                        Text {
                            text: "R$ " + Tema.valor(linha.f.total)
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.Bold
                            font.pixelSize: 17
                            Layout.alignment: Qt.AlignRight
                        }
                        Text {
                            text: linha.f.percentual ? linha.f.percentual + " das compras" : ""
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
                    text: "Nenhuma compra lançada no período"
                    font.pixelSize: 30
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Nota {
                    text: "As notas de compra (XML) lançadas no estoque aparecem aqui, por fornecedor."
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
            }
        }
    }

    Component.onCompleted: carregar()
}
