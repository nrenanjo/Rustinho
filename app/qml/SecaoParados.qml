import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Parados: peças com estoque que não vendem há muito tempo (dinheiro
// parado na prateleira). As mais caras primeiro.
FocusScope {
    id: secao
    readonly property var janela: ApplicationWindow.window
    property var dados: null
    property bool buscou: false
    property string erroDias: ""

    function carregar() {
        const r = Api.chamar("estoque_parados", { texto: busca.texto })
        if (r.ok) {
            dados = r.dados
            if (!dias.entrada.activeFocus)
                dias.texto = String(r.dados.dias)
        } else {
            janela.avisar(r.erro, "erro")
        }
        buscou = true
    }
    function salvarDias() {
        if (!dados || dias.texto === String(dados.dias))
            return
        const r = Api.chamar("estoque_dias", { qual: "parado", dias: dias.texto })
        erroDias = r.ok ? "" : r.erro
        if (r.ok)
            carregar()
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
            CartaoNumero {
                rotulo: "Peças paradas"
                prefixo: ""
                valor: secao.dados ? String(secao.dados.quantidade) : ""
                nota: secao.dados ? "mais de " + secao.dados.dias + " dias" : ""
            }
            CartaoNumero {
                rotulo: "Dinheiro parado"
                sensivel: true
                valor: secao.dados ? secao.dados.valor : ""
                nota: "pelo custo"
                cor: Tema.rosa
            }
            CartaoNumero {
                rotulo: "Nunca venderam"
                prefixo: ""
                valor: secao.dados ? String(secao.dados.nuncaVenderam) : ""
                cor: Tema.azul
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
                dica: "Procurar: peça, código, marca ou local"
                Layout.fillWidth: true
                onEditado: espera.restart()
            }
            Text {
                text: "Parada depois de"
                color: Tema.tintaSuave
                font.family: Tema.serifa
                font.italic: true
                font.pixelSize: 15
            }
            Campo {
                id: dias
                numerico: true
                maximo: 3
                sufixo: "dias"
                altura: 46
                tamanhoFonte: 16
                erro: secao.erroDias
                Layout.preferredWidth: 120
                onConfirmado: secao.salvarDias()
                onSaiu: secao.salvarDias()
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
                readonly property var p: modelData
                width: lista.width
                implicitHeight: 62
                radius: Tema.raio
                color: (index + 1) % 2 === 0 ? Tema.azul : Tema.pergaminho
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 18
                    anchors.rightMargin: 16
                    spacing: 14
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        Text {
                            text: linha.p.nome + (linha.p.marca ? " · " + linha.p.marca : "")
                            color: Tema.tinta
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: 20
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                            TapHandler { onTapped: secao.janela.abrir("TelaPeca", { produtoId: linha.p.produtoId }) }
                            HoverHandler { cursorShape: Qt.PointingHandCursor }
                        }
                        Text {
                            text: [Tema.estreito ? linha.p.ha : "", "cód. " + linha.p.codigo, linha.p.local ? "local " + linha.p.local : "", linha.p.ultimaVenda, linha.p.ultimaCompra]
                                  .filter(x => !!x).join("  ·  ")
                            color: Tema.tintaSuave
                            font.family: Tema.serifa
                            font.italic: true
                            font.pixelSize: 13
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                    }
                    Chip {
                        visible: !Tema.estreito
                        texto: linha.p.ha
                        color: Tema.vela(0.5)
                        corTexto: Tema.tintaSuave
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.preferredWidth: Tema.estreito ? -1 : 130
                        Text {
                            text: "R$ " + Tema.valor(linha.p.valor)
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.Bold
                            font.pixelSize: 17
                            Layout.alignment: Qt.AlignRight
                        }
                        Text {
                            text: "tem " + linha.p.estoque + " " + linha.p.unidade
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
                visible: secao.buscou && lista.count === 0
                spacing: 8
                Titulo {
                    text: busca.texto !== "" ? "Nada encontrado" : "Nenhuma peça parada"
                    font.pixelSize: 30
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Nota {
                    visible: busca.texto === ""
                    text: "Aparece aqui a peça com estoque que não vende (nem entra) há mais tempo do que os dias escolhidos."
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
            }
        }
        Nota {
            text: "Ideias para peça parada: promoção no balcão, trocar com o fornecedor ou oferecer às oficinas."
            font.pixelSize: 12
            Layout.fillWidth: true
        }
    }

    Component.onCompleted: {
        carregar()
        busca.focar()
    }
}
