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

        ColumnLayout {
            id: topo
            spacing: 12
            Layout.fillWidth: true

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
                // D-09: o dinheiro parado é pelo custo: só para quem vê os
                // resultados. O cartão fica (sem esconder: veja D-10).
                CartaoNumero {
                    readonly property bool pode: !!secao.dados && secao.dados.resultados
                    rotulo: "Dinheiro parado"
                    sensivel: pode
                    prefixo: pode ? "R$ " : ""
                    valor: secao.dados ? (pode ? secao.dados.valor : "—") : ""
                    nota: secao.dados && !pode ? "só o gerente" : "pelo custo"
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
                    color: Tema.tintaSuaveNo(palette.window)
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
        }

        ListView {
            id: lista
            header: Tema.celular ? espacoDoTopo : null
            footer: Tema.celular ? espacoDoRodape : null
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
                // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                readonly property bool par: (index + 1) % 2 === 0
                color: Tema.faixa(linha.par)
                palette.window: Tema.superficie(color)
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
                            textFormat: Text.PlainText
                            color: Tema.tintaDaFaixa(linha.par)
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
                            textFormat: Text.PlainText
                            color: Tema.tintaSuaveDaFaixa(linha.par)
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
                            visible: linha.p.valor !== ""
                            text: "R$ " + Tema.valor(linha.p.valor)
                            textFormat: Text.PlainText
                            color: Tema.tintaDaFaixa(linha.par)
                            font.family: Tema.texto
                            font.weight: Font.Bold
                            font.pixelSize: 17
                            Layout.alignment: Qt.AlignRight
                        }
                        Text {
                            text: "tem " + linha.p.estoque + " " + linha.p.unidade
                            textFormat: Text.PlainText
                            color: Tema.tintaSuaveDaFaixa(linha.par)
                            font.family: Tema.texto
                            font.pixelSize: 12
                            Layout.alignment: Qt.AlignRight
                        }
                    }
                }
            }
            ColumnLayout {
                // O estado vazio. I8: no celular, vai para o fim do topo (rola junto com ele; fixo na lista,
                // ficava abaixo da borda, cortado e sem como rolar até ele).
                id: vazio
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
            id: rodape
            text: "Ideias para peça parada: promoção no balcão, trocar com o fornecedor ou oferecer às oficinas."
            font.pixelSize: 12
            Layout.fillWidth: true
        }
    }

    // I8 (VIS-C8-02, 15/11/2026): no celular, o cabeçalho fixo da tela tomava quase toda a altura e a lista ficava
    // com poucos dp (as linhas e o estado vazio escondidos). Lá, o topo desta seção (cartões, busca e filtros) rola
    // junto com a lista: fica dentro dela, no lugar do cabeçalho (`espacoDoTopo`). No computador, nada muda. As
    // linhas continuam sob demanda (o ListView só cria as que aparecem), mesmo com milhares de peças.
    Component {
        id: espacoDoTopo
        Item { width: lista.width; height: topo.implicitHeight + 12 }
    }
    // A nota de rodapé também vai para dentro da lista (no fim), no celular: fora dela, tomava altura e ficava
    // por cima das linhas cortadas na borda.
    Component {
        id: espacoDoRodape
        Item { width: lista.width; height: rodape.implicitHeight + 12 }
    }
    function topoNaLista() {
        rodape.parent = lista.contentItem
        rodape.x = 0
        rodape.width = Qt.binding(() => lista.width)
        rodape.y = Qt.binding(() => lista.footerItem ? lista.footerItem.y + 12 : 0)
        topo.parent = lista.contentItem
        topo.x = 0
        topo.width = Qt.binding(() => lista.width)
        topo.y = Qt.binding(() => lista.headerItem ? lista.headerItem.y : 0)
        vazio.anchors.centerIn = undefined
        vazio.parent = topo
        vazio.Layout.fillWidth = true
        vazio.Layout.topMargin = 20
    }

    Component.onCompleted: {
        if (Tema.celular)
            topoNaLista()
        carregar()
        busca.focar()
    }
}
