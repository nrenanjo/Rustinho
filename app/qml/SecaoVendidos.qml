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

    // A-1 da revisão do VIS2 (16/11/2026): no celular a busca fica dentro da lista (topoNaLista), e a lista, quando
    // troca o modelo, dá o foco à 1ª linha: a busca perdia o foco a cada recarga (200 ms depois da tecla), e só 1 ou 2
    // letras entravam. Se a busca tinha o foco, ele e o cursor voltam depois da troca (sem selecionar o texto).
    function carregar() {
        const comFoco = busca.entrada.activeFocus
        const cursor = busca.entrada.cursorPosition
        carregarDados()
        if (comFoco)
            Qt.callLater(() => {
                if (busca.entrada.activeFocus)
                    return
                busca.entrada.forceActiveFocus()
                busca.entrada.cursorPosition = Math.min(cursor, busca.entrada.text.length)
            })
    }
    function carregarDados() {
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

        ColumnLayout {
            id: topo
            spacing: 12
            Layout.fillWidth: true

            Linha {
                // T042: por cima da linha da barra de pesquisa logo abaixo: o halo da barra passa atrás, não por cima.
                z: 1
                colunasEstreito: 2
                spacing: 12
                Layout.fillWidth: true
                // OLHO-1 e M4 da revisão: o valor vendido também some com o olho fechado ("mais vendidos" está na
                // decisão); a quantidade e a curva ABC ficam.
                CartaoNumero { rotulo: "Vendido"; sensivel: true; valor: secao.dados ? secao.dados.total : ""; cor: Tema.verde; corValor: Tema.verdeEscuro }
                // D-01: o lucro só para quem pode ver os resultados. O cartão fica
                // sempre (trocar "visible" dentro da linha embaralha no Qt 6.4, D-10).
                CartaoNumero {
                    readonly property bool pode: !!secao.dados && secao.dados.resultados
                    rotulo: "Lucro bruto"
                    sensivel: pode
                    prefixo: pode ? "R$ " : ""
                    valor: !secao.dados ? "" : pode ? secao.dados.lucro : "—"
                    nota: secao.dados && !pode ? "só o gerente" : ""
                }
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
        }

        ListView {
            id: lista
            // A geometria da lista para os roteiros (GEO:listaDoEstoque; R-5 da re-revisão 3).
            objectName: "listaDoEstoque"
            // T042: no celular, o topo (cartões e barra de pesquisa) rola dentro desta lista, que recorta o que passa da
            // borda (o halo da barra era cortado). A lista começa Tema.buscaAlcance antes e acaba depois, com essa margem
            // por dentro: o conteúdo fica no mesmo lugar e com a mesma largura (larguraUtil). No computador, nada muda.
            readonly property real folgaDoHalo: Tema.celular ? Tema.buscaAlcance : 0
            readonly property real larguraUtil: width - 2 * folgaDoHalo
            Layout.leftMargin: -folgaDoHalo
            Layout.rightMargin: -folgaDoHalo
            leftMargin: folgaDoHalo
            rightMargin: folgaDoHalo
            // R11: se a folga muda com a seção aberta (girar o tablet, mudar o tamanho da janela), o conteúdo volta ao lugar.
            onFolgaDoHaloChanged: contentX = -folgaDoHalo
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
                readonly property var v: modelData
                width: lista.larguraUtil
                implicitHeight: 64
                radius: Tema.raio
                // A15 (0.9.18): a linha inteira (64 dp) abre a peça; antes, só o
                // nome (25 dp de altura para o dedo).
                TapHandler { onTapped: secao.janela.abrir("TelaPeca", { produtoId: linha.v.produtoId }) }
                // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                readonly property bool par: (index + 1) % 2 === 0
                color: Tema.faixa(linha.par)
                palette.window: Tema.superficie(color)
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Tema.estreito ? 10 : 16
                    anchors.rightMargin: Tema.estreito ? 12 : 16
                    spacing: Tema.estreito ? 8 : 14
                    Text {
                        text: linha.v.posicao
                        textFormat: Text.PlainText
                        color: Tema.douradoDaFaixa(linha.par)
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
                            textFormat: Text.PlainText
                            color: Tema.tintaDaFaixa(linha.par)
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: 20
                            elide: Text.ElideRight
                            Layout.fillWidth: true
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
                        color: linha.v.curva === "A" ? Tema.verde : linha.v.curva === "B" ? Tema.papel : Tema.vela(0.5)
                        corTexto: linha.v.curva === "A" ? Tema.verdeEscuro : Tema.tintaSuave
                    }
                    ColumnLayout {
                        visible: !Tema.estreito
                        spacing: 0
                        Layout.preferredWidth: 130
                        Text {
                            text: linha.v.quantidade + " " + linha.v.unidade
                            textFormat: Text.PlainText
                            color: Tema.tintaDaFaixa(linha.par)
                            font.family: Tema.texto
                            font.weight: Font.DemiBold
                            font.pixelSize: 15
                            Layout.alignment: Qt.AlignRight
                        }
                        Text {
                            text: linha.v.vendas + (linha.v.vendas === 1 ? " venda" : " vendas")
                            textFormat: Text.PlainText
                            color: Tema.tintaSuaveDaFaixa(linha.par)
                            font.family: Tema.texto
                            font.pixelSize: 12
                            Layout.alignment: Qt.AlignRight
                        }
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.preferredWidth: Tema.estreito ? -1 : 140
                        Text {
                            text: "R$ " + Tema.valor(linha.v.total)
                            textFormat: Text.PlainText
                            color: Tema.tintaDaFaixa(linha.par)
                            font.family: Tema.texto
                            font.weight: Font.Bold
                            font.pixelSize: 17
                            Layout.alignment: Qt.AlignRight
                        }
                        Text {
                            visible: linha.v.lucro !== ""
                            text: "lucro R$ " + Tema.valor(linha.v.lucro) + (linha.v.margem && !Tema.oculto ? " (" + linha.v.margem + ")" : "")
                            textFormat: Text.PlainText
                            color: Tema.tintaSuaveDaFaixa(linha.par)
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
                            color: linha.v.estoqueBaixo ? Tema.rosa : Tema.vela(0.5)
                            corTexto: linha.v.estoqueBaixo ? Tema.vermelho : Tema.tintaSuave
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
                    text: busca.texto !== "" ? "Nada encontrado" : "Nenhuma venda no período"
                    font.pixelSize: 30
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
            }
        }
        Nota {
            id: rodape
            text: "Curva ABC pelo valor vendido: A = as peças que somam 80% do faturamento, B = os 15% seguintes, C = o resto."
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
        Item { width: lista.larguraUtil; height: topo.implicitHeight + 12 }
    }
    // A nota de rodapé também vai para dentro da lista (no fim), no celular: fora dela, tomava altura e ficava
    // por cima das linhas cortadas na borda.
    Component {
        id: espacoDoRodape
        Item { width: lista.larguraUtil; height: rodape.implicitHeight + 12 }
    }
    function topoNaLista() {
        rodape.parent = lista.contentItem
        rodape.x = 0
        rodape.width = Qt.binding(() => lista.larguraUtil)
        rodape.y = Qt.binding(() => lista.footerItem ? lista.footerItem.y + 12 : 0)
        topo.parent = lista.contentItem
        topo.x = 0
        topo.width = Qt.binding(() => lista.larguraUtil)
        // B-1 da revisão do VIS2: dentro da lista, o topo seguia com a altura que o arranjo de fora lhe deu no começo,
        // e não com a natural; os blocos se espalhavam e as pílulas passavam por cima da 1ª linha.
        topo.height = Qt.binding(() => topo.implicitHeight)
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
