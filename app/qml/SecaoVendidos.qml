import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Mais vendidos, com a curva ABC: as peças "A" fazem 80% do faturamento e
// são as que nunca podem faltar.
FocusScope {
    id: secao
    readonly property var janela: ApplicationWindow.window
    property var dados: null
    property int dias: 90
    property string ordem: "valor"
    property bool buscou: false

    function carregar() {
        const r = Api.chamar("estoque_mais_vendidos", { dias: dias, ordem: ordem, texto: busca.texto })
        if (r.ok)
            dados = r.dados
        else
            janela.avisar(r.erro, "erro")
        buscou = true
    }
    function aoEsc() {
        if (busca.texto !== "") {
            busca.texto = ""
            carregar()
            return true
        }
        return false
    }

    Timer { id: espera; interval: 200; onTriggered: secao.carregar() }

    ColumnLayout {
        anchors.fill: parent
        spacing: 12

        Linha {
            colunasEstreito: 2
            spacing: 12
            Layout.fillWidth: true
            CartaoNumero { rotulo: "Vendido"; valor: secao.dados ? secao.dados.total : ""; cor: Tema.verde; corValor: Tema.verdeEscuro }
            CartaoNumero { rotulo: "Lucro bruto"; sensivel: true; valor: secao.dados ? secao.dados.lucro : "" }
            CartaoNumero {
                rotulo: "Peças vendidas"
                prefixo: ""
                valor: secao.dados ? String(secao.dados.pecas) : ""
                nota: "diferentes"
                cor: Tema.azul
            }
            CartaoNumero {
                rotulo: "Curva A"
                prefixo: ""
                valor: secao.dados ? String(secao.dados.curvaA) : ""
                nota: "fazem 80% do valor"
                cor: Tema.pergaminho
            }
        }

        Linha {
            spacing: 14
            Layout.fillWidth: true
            Campo {
                id: busca
                icone: "busca"
                altura: 46
                tamanhoFonte: 16
                dica: "Procurar peça na lista"
                Layout.fillWidth: true
                onEditado: espera.restart()
            }
            Pilulas {
                pequenas: true
                modelo: [{ id: "30", nome: "30 dias" }, { id: "90", nome: "90 dias" }, { id: "365", nome: "12 meses" }]
                atual: String(secao.dias)
                onEscolhido: id => {
                    secao.dias = parseInt(id)
                    secao.carregar()
                }
            }
            Pilulas {
                pequenas: true
                modelo: [{ id: "valor", nome: "Por valor" }, { id: "quantidade", nome: "Por quantidade" }]
                atual: secao.ordem
                onEscolhido: id => {
                    secao.ordem = id
                    secao.carregar()
                }
            }
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
                readonly property var v: modelData
                width: lista.width
                implicitHeight: 64
                radius: Tema.raio
                color: (index + 1) % 2 === 0 ? Tema.azul : Tema.pergaminho
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Tema.estreito ? 10 : 16
                    anchors.rightMargin: Tema.estreito ? 12 : 16
                    spacing: Tema.estreito ? 8 : 14
                    Text {
                        text: linha.v.posicao
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
                        Text {
                            text: linha.v.nome + (linha.v.marca ? " · " + linha.v.marca : "")
                            color: Tema.tinta
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: 20
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                            TapHandler { onTapped: secao.janela.abrir("TelaPeca", { produtoId: linha.v.produtoId }) }
                            HoverHandler { cursorShape: Qt.PointingHandCursor }
                        }
                        // barrinha do quanto vendeu (perto do 1º)
                        Item {
                            Layout.fillWidth: true
                            implicitHeight: 6
                            Rectangle {
                                height: 6
                                radius: 3
                                width: Math.max(6, parent.width * linha.v.parte)
                                color: Tema.verdeEscuro
                                opacity: 0.4
                            }
                        }
                    }
                    Chip {
                        texto: linha.v.curva
                        implicitWidth: 30
                        color: linha.v.curva === "A" ? Tema.verde : linha.v.curva === "B" ? Tema.papel : Qt.rgba(1, 1, 1, 0.5)
                        corTexto: linha.v.curva === "A" ? Tema.verdeEscuro : Tema.tintaSuave
                    }
                    ColumnLayout {
                        visible: !Tema.estreito
                        spacing: 0
                        Layout.preferredWidth: 130
                        Text {
                            text: linha.v.quantidade + " " + linha.v.unidade
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.DemiBold
                            font.pixelSize: 15
                            Layout.alignment: Qt.AlignRight
                        }
                        Text {
                            text: linha.v.vendas + (linha.v.vendas === 1 ? " venda" : " vendas")
                            color: Tema.tintaSuave
                            font.family: Tema.texto
                            font.pixelSize: 12
                            Layout.alignment: Qt.AlignRight
                        }
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.preferredWidth: Tema.estreito ? -1 : 140
                        Text {
                            text: "R$ " + linha.v.total
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.Bold
                            font.pixelSize: 17
                            Layout.alignment: Qt.AlignRight
                        }
                        Text {
                            text: "lucro R$ " + Tema.valor(linha.v.lucro) + (linha.v.margem && !Tema.oculto ? " (" + linha.v.margem + ")" : "")
                            color: Tema.tintaSuave
                            font.family: Tema.texto
                            font.pixelSize: 12
                            Layout.alignment: Qt.AlignRight
                        }
                    }
                    Item {
                        visible: !Tema.estreito
                        Layout.preferredWidth: 96
                        implicitHeight: 26
                        Chip {
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            texto: linha.v.estoque === "0" || linha.v.estoque.startsWith("-") ? "sem estoque" : "tem " + linha.v.estoque
                            color: linha.v.estoqueBaixo ? Tema.rosa : Qt.rgba(1, 1, 1, 0.5)
                            corTexto: linha.v.estoqueBaixo ? Tema.vermelho : Tema.tintaSuave
                        }
                    }
                }
            }
            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(parent.width - 40, 600)
                visible: secao.buscou && lista.count === 0
                spacing: 8
                Titulo {
                    text: busca.texto !== "" ? "Nada encontrado" : "Nenhuma venda no período"
                    font.pixelSize: 30
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
            }
        }
        Nota {
            text: "Curva ABC pelo valor vendido: A = as peças que somam 80% do faturamento, B = os 15% seguintes, C = o resto."
            font.pixelSize: 12
            Layout.fillWidth: true
        }
    }

    Component.onCompleted: {
        carregar()
        busca.focar()
    }
}
