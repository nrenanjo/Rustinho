import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Peças e vendedores: rankings separados por quantidade, receita, lucro e
// margem (maior venda não é maior lucro) e o desempenho de cada vendedor.
FocusScope {
    id: secao
    readonly property var janela: ApplicationWindow.window
    property var filtro: ({ periodo: "mes", de: "", ate: "", vendedorId: "" })
    property var r: null

    onFiltroChanged: carregar()

    function carregar() {
        const x = Api.chamar("dre_indicadores", Object.assign({ busca: busca.texto }, filtro))
        if (x.ok)
            r = x.dados
        else
            janela.avisar(x.erro, "erro")
    }
    function linhas(lista, campo, dinheiro) {
        if (!lista || lista.length === 0)
            return []
        const num = t => parseFloat(String(t).replace(/\./g, "").replace(",", ".").replace("%", "")) || 0
        const maior = Math.max(...lista.map(p => num(p[campo])))
        return lista.map(p => ({
            nome: p.nome,
            detalhe: campo === "quantidade" ? "R$ " + p.receita : campo === "margem" ? "lucro R$ " + p.lucro : p.quantidade + " un.",
            valor: campo === "quantidade" ? p.quantidade + " un." : p[campo],
            parte: maior > 0 ? Math.max(0, num(p[campo])) / maior : 0,
            percentual: ""
        }))
    }

    Timer { id: espera; interval: 250; onTriggered: secao.carregar() }

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

            Linha {
                spacing: 12
                Layout.fillWidth: true
                Campo {
                    id: busca
                    rotulo: "Procurar peça"
                    dica: "nome ou marca"
                    icone: "busca"
                    Layout.fillWidth: true
                    onEditado: espera.restart()
                }
                Nota {
                    text: secao.r ? secao.r.periodo.rotulo : ""
                    font.pixelSize: 14
                    Layout.alignment: Qt.AlignBottom
                }
            }

            GridLayout {
                columns: Tema.fino ? 1 : 2
                columnSpacing: 14
                rowSpacing: 14
                Layout.fillWidth: true
                Repeater {
                    model: [
                        { titulo: "Mais vendidas (quantidade)", lista: "porQuantidade", campo: "quantidade", cor: Tema.azul, dinheiro: false },
                        { titulo: "Maior receita líquida", lista: "porReceita", campo: "receita", cor: Tema.pergaminho, dinheiro: true },
                        { titulo: "Maior lucro bruto", lista: "porLucro", campo: "lucro", cor: Tema.verde, dinheiro: true },
                        { titulo: "Maior margem", lista: "porMargem", campo: "margem", cor: Tema.azul, dinheiro: false }
                    ]
                    delegate: Faixa {
                        id: ranking
                        required property var modelData
                        titulo: modelData.titulo
                        color: modelData.cor
                        espacamento: 10
                        Layout.fillWidth: true
                        Layout.alignment: Qt.AlignTop
                        Repeater {
                            model: secao.r ? secao.linhas(secao.r[ranking.modelData.lista], ranking.modelData.campo) : []
                            delegate: LinhaRanking {
                                required property var modelData
                                required property int index
                                item: modelData
                                posicao: index + 1
                                dinheiro: ranking.modelData.dinheiro
                                sensivel: ranking.modelData.campo === "margem"
                            }
                        }
                        Nota {
                            visible: !!secao.r && secao.r[ranking.modelData.lista].length === 0
                            text: "Nada no período."
                            font.pixelSize: 14
                        }
                    }
                }
            }

            Faixa {
                titulo: "Vendedores"
                subtitulo: secao.r ? secao.r.criterioVendedor : ""
                color: Tema.pergaminho
                espacamento: 6
                Layout.fillWidth: true
                RowLayout {
                    spacing: 8
                    Layout.fillWidth: true
                    Repeater {
                        model: ["Vendedor", "Vendas", "Receita", "Ticket", "Descontos", "Lucro", "Margem", "Devolv.", "Cancel."]
                        delegate: Rotulo {
                            required property var modelData
                            required property int index
                            text: modelData
                            horizontalAlignment: index === 0 ? Text.AlignLeft : Text.AlignRight
                            Layout.fillWidth: index === 0
                            Layout.preferredWidth: index === 0 ? 140 : index === 5 ? 120 : 96
                        }
                    }
                }
                Repeater {
                    model: secao.r ? secao.r.vendedores : []
                    delegate: RowLayout {
                        id: v
                        required property var modelData
                        spacing: 8
                        Layout.fillWidth: true
                        // Sem depender do olho: o modelo não muda ao mostrar/esconder (o Qt 6.4
                        // cai se um Repeater dentro de um layout é refeito no meio da arrumação).
                        readonly property var colunas: [
                            String(modelData.vendas),
                            "R$ " + modelData.receita,
                            modelData.ticket ? "R$ " + modelData.ticket : "—",
                            "R$ " + modelData.descontos,
                            modelData.lucro ? "R$ " + modelData.lucro + (modelData.semCusto ? " *" : "") : "—",
                            modelData.margem ? modelData.margem : "—",
                            modelData.devolucoes > 0 ? modelData.devolucoes + " · R$ " + modelData.devolucoesValor : "—",
                            modelData.canceladas > 0 ? modelData.canceladas + " · R$ " + modelData.canceladasValor : "—"
                        ]
                        Text {
                            text: v.modelData.nome
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.DemiBold
                            font.pixelSize: 14
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                            Layout.preferredWidth: 140
                        }
                        Repeater {
                            model: v.colunas
                            delegate: Text {
                                required property var modelData
                                required property int index
                                text: index === 5 && Tema.oculto && modelData !== "—" ? "••" : Tema.sigiloTexto(modelData)
                                color: Tema.tinta
                                font.family: Tema.texto
                                font.pixelSize: 13
                                horizontalAlignment: Text.AlignRight
                                elide: Text.ElideLeft
                                Layout.preferredWidth: index === 4 ? 120 : 96
                            }
                        }
                    }
                }
                Nota {
                    visible: !!secao.r && secao.r.vendedores.length === 0
                    text: "Nenhuma venda no período."
                    font.pixelSize: 14
                }
                Nota {
                    visible: !!secao.r && secao.r.vendedores.some(v => v.semCusto)
                    text: "* lucro incompleto: há peça vendida sem custo no cadastro."
                    font.pixelSize: 13
                    Layout.fillWidth: true
                }
            }

            Nota {
                text: secao.r ? secao.r.nota : ""
                font.pixelSize: 13
                Layout.fillWidth: true
            }
        }
    }

    Component.onCompleted: carregar()
    Component.onDestruction: Api.chamar("gerente_encerrar")
}
