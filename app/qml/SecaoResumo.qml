import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Resumo do Painel: hoje, o mês e o ano, vendas por dia, os últimos 12
// meses, o movimento por hora e, ao lado, como pagaram e o que mais vendeu.
FocusScope {
    id: secao
    readonly property var janela: ApplicationWindow.window
    property var r: null

    function carregar() {
        const x = Api.chamar("painel_resumo")
        if (x.ok)
            r = x.dados
        else
            janela.avisar(x.erro, "erro")
    }

    function abrir(funcao, propriedades, tela) {
        Api.chamar("menu_usar", { id: funcao })
        janela.abrir(tela, propriedades || {})
    }

    Flickable {
        id: rolagem
        anchors.fill: parent
        clip: true
        contentHeight: corpo.implicitHeight + 24
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}

        ColumnLayout {
            id: corpo
            width: rolagem.width - 14
            spacing: 14

            RowLayout {
                spacing: 12
                Layout.fillWidth: true
                CartaoNumero {
                    rotulo: "Hoje"
                    sensivel: true
                    valor: secao.r ? secao.r.hoje.total : ""
                    nota: secao.r ? secao.r.hoje.vendas + (secao.r.hoje.vendas === 1 ? " venda" : " vendas") : ""
                    cor: Tema.verde
                    corValor: Tema.verdeEscuro
                    implicitHeight: 96
                    notaEmbaixo: true
                }
                CartaoNumero {
                    rotulo: secao.r ? "Em " + secao.r.mesNome : "No mês"
                    sensivel: true
                    valor: secao.r ? secao.r.mes.total : ""
                    nota: secao.r ? (secao.r.comparacao || secao.r.mes.vendas + " vendas") : ""
                    corNota: secao.r && secao.r.comparacao ? (secao.r.comparacaoSubiu ? Tema.verdeEscuro : Tema.vermelho) : Tema.apagado
                    implicitHeight: 96
                    notaEmbaixo: true
                }
                CartaoNumero {
                    rotulo: secao.r ? "Em " + secao.r.anoNumero : "No ano"
                    sensivel: true
                    valor: secao.r ? secao.r.ano.total : ""
                    nota: secao.r ? secao.r.ano.vendas + " vendas" : ""
                    cor: Tema.azul
                    implicitHeight: 96
                    notaEmbaixo: true
                }
                CartaoNumero {
                    rotulo: "Lucro do mês"
                    sensivel: true
                    valor: secao.r ? secao.r.lucroMes : ""
                    nota: secao.r && secao.r.margemMes && !Tema.oculto ? secao.r.margemMes + " das vendas" : ""
                    cor: Tema.rosa
                    implicitHeight: 96
                    notaEmbaixo: true
                }
            }

            RowLayout {
                spacing: 14
                Layout.fillWidth: true

                ColumnLayout {
                    spacing: 14
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignTop

                    Faixa {
                        titulo: secao.r ? "Vendas por dia · " + secao.r.mesNome : "Vendas por dia"
                        subtitulo: secao.r && secao.r.mes.ticket ? "ticket médio R$ " + Tema.valor(secao.r.mes.ticket) : ""
                        color: Tema.pergaminho
                        Layout.fillWidth: true
                        GraficoBarras {
                            barras: secao.r ? secao.r.porDia : []
                            alturaBarras: 150
                        }
                    }

                    RowLayout {
                        spacing: 14
                        Layout.fillWidth: true
                        Faixa {
                            titulo: "Últimos 12 meses"
                            color: Tema.azul
                            Layout.fillWidth: true
                            Layout.preferredWidth: 3
                            GraficoBarras {
                                barras: secao.r ? secao.r.porMes : []
                                alturaBarras: 110
                            }
                        }
                        Faixa {
                            titulo: "Movimento por hora"
                            subtitulo: "30 dias"
                            color: Tema.verde
                            corTitulo: Tema.verdeEscuro
                            Layout.fillWidth: true
                            Layout.preferredWidth: 2
                            GraficoBarras {
                                barras: secao.r ? secao.r.porHora : []
                                dinheiro: false
                                unidade: " vendas"
                                cor: Tema.verdeEscuro
                                corDestaque: Tema.verdeEscuro
                                alturaBarras: 110
                                rotuloCada: 2
                            }
                        }
                    }

                    Nota {
                        visible: !!secao.r && secao.r.semCusto > 0 && !Tema.oculto
                        text: secao.r ? secao.r.semCusto + (secao.r.semCusto === 1 ? " peça vendida no mês está" : " peças vendidas no mês estão")
                                        + " sem custo no cadastro e contam inteiras como lucro." : ""
                        font.pixelSize: 13
                        Layout.fillWidth: true
                    }
                }

                ColumnLayout {
                    spacing: 14
                    Layout.preferredWidth: 400
                    Layout.maximumWidth: 400
                    Layout.alignment: Qt.AlignTop

                    // A receber e a pagar (clica para abrir).
                    Repeater {
                        model: secao.r ? [
                            { titulo: "A receber no crediário", valor: secao.r.aReceber, vencido: secao.r.receberVencido,
                              cor: Tema.verde, funcao: "receber", tela: "TelaReceber", props: {} },
                            { titulo: "A pagar em 7 dias", valor: secao.r.pagar7Dias, vencido: secao.r.pagarVencido,
                              cor: Tema.rosa, funcao: "financeiro", tela: "TelaFinanceiro", props: { secao: "pagar" } }
                        ] : []
                        delegate: Rectangle {
                            id: conta
                            required property var modelData
                            Layout.fillWidth: true
                            implicitHeight: 64
                            radius: Tema.raio
                            color: sobreConta.hovered ? Qt.lighter(modelData.cor, 1.04) : modelData.cor
                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 18
                                anchors.rightMargin: 14
                                spacing: 10
                                ColumnLayout {
                                    spacing: 1
                                    Layout.fillWidth: true
                                    Rotulo { text: conta.modelData.titulo }
                                    Text {
                                        visible: conta.modelData.vencido !== "0,00"
                                        text: "vencido R$ " + Tema.valor(conta.modelData.vencido)
                                        color: Tema.vermelho
                                        font.family: Tema.serifa
                                        font.italic: true
                                        font.pixelSize: 13
                                    }
                                }
                                Text {
                                    text: "R$ " + Tema.valor(conta.modelData.valor)
                                    color: Tema.tinta
                                    font.family: Tema.titulo
                                    font.weight: Font.Bold
                                    font.pixelSize: 24
                                }
                                Icone { nome: "direita"; tamanho: 18; cor: Tema.apagado }
                            }
                            HoverHandler { id: sobreConta; cursorShape: Qt.PointingHandCursor }
                            TapHandler { onTapped: secao.abrir(conta.modelData.funcao, conta.modelData.props, conta.modelData.tela) }
                        }
                    }

                    Faixa {
                        titulo: secao.r ? "Como pagaram · " + secao.r.mesNome : "Como pagaram"
                        color: Tema.pergaminho
                        espacamento: 12
                        Layout.fillWidth: true
                        Repeater {
                            model: secao.r ? secao.r.formas : []
                            delegate: LinhaRanking {
                                required property var modelData
                                item: modelData
                            }
                        }
                        Nota {
                            visible: !!secao.r && secao.r.formas.length === 0
                            text: "Nenhuma venda no mês ainda."
                            font.pixelSize: 14
                        }
                    }

                    Faixa {
                        titulo: "Mais vendidos no mês"
                        color: Tema.azul
                        espacamento: 12
                        Layout.fillWidth: true
                        Repeater {
                            model: secao.r ? secao.r.maisVendidos : []
                            delegate: LinhaRanking {
                                required property var modelData
                                required property int index
                                item: modelData
                                posicao: index + 1
                            }
                        }
                        Botao {
                            visible: !!secao.r && secao.r.maisVendidos.length > 0
                            text: "Ver todos no Estoque"
                            tipo: "discreto"
                            icone: "direita"
                            implicitHeight: 32
                            font.pixelSize: 14
                            Layout.leftMargin: -10
                            onClicked: secao.abrir("estoque", { secao: "vendidos" }, "TelaEstoque")
                        }
                        Nota {
                            visible: !!secao.r && secao.r.maisVendidos.length === 0
                            text: "Nenhuma peça vendida no mês ainda."
                            font.pixelSize: 14
                        }
                    }

                    Faixa {
                        visible: !!secao.r && secao.r.vendedores.length > 0
                        titulo: "Vendedores no mês"
                        color: Tema.verde
                        corTitulo: Tema.verdeEscuro
                        espacamento: 12
                        Layout.fillWidth: true
                        Repeater {
                            model: secao.r ? secao.r.vendedores : []
                            delegate: LinhaRanking {
                                required property var modelData
                                item: modelData
                                cor: Tema.verdeEscuro
                            }
                        }
                    }
                }
            }
        }
    }

    Component.onCompleted: carregar()
}
