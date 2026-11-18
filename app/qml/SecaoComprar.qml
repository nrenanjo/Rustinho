import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// O que comprar: as encomendas dos clientes (à parte) e a reposição do
// estoque, por prioridade, com a quantidade sugerida pelo ritmo de venda.
FocusScope {
    id: secao
    // H16 (7.2, técnica DP-26): a linha tocada fica escolhida e mostra as ações
    // que antes só apareciam com o mouse em cima (no toque não há "em cima").
    property string linhaEscolhida: ""
    readonly property var janela: ApplicationWindow.window
    property var dados: null
    property bool buscou: false
    property string erroDias: ""

    // Uma lista só: títulos, encomendas e peças.
    readonly property var linhas: {
        if (!dados)
            return []
        const l = []
        if (dados.encomendas.length > 0) {
            l.push({ tipo: "titulo", texto: "Encomendas de clientes", acao: true })
            dados.encomendas.forEach(e => l.push({ tipo: "encomenda", e: e }))
        }
        if (dados.itens.length > 0) {
            l.push({ tipo: "titulo", texto: "Para o estoque", acao: false })
            dados.itens.forEach((p, i) => l.push({ tipo: "peca", p: p, i: i }))
        }
        return l
    }

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
    // B-4 da revisão do VIS2: no celular, depois de uma ação numa linha a lista voltava ao começo (o topo ocupa a área
    // toda) e as linhas sumiam da vista. Nas recargas por ação, a lista volta à mesma LINHA (e não ao mesmo pixel: as
    // linhas têm alturas diferentes e são criadas sob demanda, e o mesmo contentY caía sete linhas adiante).
    function recarregarNoLugar() {
        const i = Tema.celular ? lista.indexAt(lista.width / 2, lista.contentY + 1) : -1
        // R-2 da re-revisão 3 do Fable: com a lista pouco rolada, o alto da área ainda é o topo da seção (o cabeçalho
        // da lista: a busca e as pílulas), e ali não há linha (i < 0). Nesse caso a lista volta ao mesmo deslocamento
        // em pixels, que vale porque o topo não muda de altura com a ação.
        const deslocamento = Tema.celular && i < 0 ? lista.contentY - lista.originY : -1
        carregar()
        if (i >= 0)
            Qt.callLater(() => lista.positionViewAtIndex(Math.min(i, lista.count - 1), ListView.Beginning))
        else if (deslocamento > 0)
            Qt.callLater(() => {
                lista.contentY = lista.originY + Math.min(deslocamento, Math.max(0, lista.contentHeight - lista.height))
            })
    }
    function carregarDados() {
        const r = Api.chamar("estoque_sugestao", { texto: busca.texto })
        if (r.ok) {
            dados = r.dados
            if (!dias.entrada.activeFocus)
                dias.texto = String(r.dados.diasCompra)
        } else {
            janela.avisar(r.erro, "erro")
        }
        buscou = true
    }
    function salvarDias() {
        if (!dados || dias.texto === String(dados.diasCompra))
            return
        const r = Api.chamar("estoque_dias", { qual: "compra", dias: dias.texto })
        erroDias = r.ok ? "" : r.erro
        if (r.ok)
            carregar()
    }
    function pedidas(ids) {
        const r = Api.gravar("encomenda_pedir", { ids: ids })
        if (!r.ok) {
            janela.avisar(r.erro, "erro")
            return
        }
        janela.avisar(ids.length === 1 ? "Encomenda marcada como pedida." : ids.length + " encomendas marcadas como pedidas.", "ok")
        recarregarNoLugar()
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
                CartaoNumero {
                    rotulo: "Acabou"
                    prefixo: ""
                    valor: secao.dados ? String(secao.dados.acabou) : ""
                    nota: "peças que vendem"
                    cor: Tema.rosa
                    corValor: secao.dados && secao.dados.acabou > 0 ? Tema.vermelho : Tema.tinta
                }
                CartaoNumero {
                    rotulo: "Acabando"
                    prefixo: ""
                    valor: secao.dados ? String(secao.dados.acabando) : ""
                    nota: secao.dados ? "antes de " + secao.dados.diasCompra + " dias" : ""
                }
                CartaoNumero {
                    rotulo: "Encomendas a pedir"
                    prefixo: ""
                    valor: secao.dados ? String(secao.dados.encomendas.length) : ""
                    cor: Tema.verde
                    corValor: Tema.verdeEscuro
                }
                CartaoNumero {
                    rotulo: "Custo estimado"
                    sensivel: true
                    valor: secao.dados ? secao.dados.custoTotal : ""
                    nota: "sem as encomendas"
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
                    dica: "Filtrar por peça, código ou fornecedor"
                    Layout.fillWidth: true
                    onEditado: espera.restart()
                }
                Text {
                    text: "Comprar para"
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
            header: Tema.celular ? espacoDoTopo : null
            footer: Tema.celular ? espacoDoRodape : null
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 6
            model: secao.linhas
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            delegate: Loader {
                required property var modelData
                width: lista.larguraUtil
                sourceComponent: modelData.tipo === "titulo" ? cTitulo : modelData.tipo === "encomenda" ? cEncomenda : cPeca
                property var dado: modelData
            }

            ColumnLayout {
                // O estado vazio. I8: no celular, vai para o fim do topo (rola junto com ele; fixo na lista,
                // ficava abaixo da borda, cortado e sem como rolar até ele).
                id: vazio
                anchors.centerIn: parent
                width: Math.min(parent.width - 40, 620)
                visible: secao.buscou && lista.count === 0
                spacing: 8
                Titulo {
                    text: busca.texto !== "" ? "Nada encontrado" : "Nada para comprar agora"
                    font.pixelSize: 30
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Nota {
                    visible: busca.texto === ""
                    text: "O Rustinho olha o que cada peça vendeu nos últimos 90 dias e avisa aqui o que vai faltar. "
                          + "Peça com estoque mínimo no cadastro também aparece quando fica abaixo dele."
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
            }
        }
        Nota {
            id: rodape
            text: secao.dados ? "A quantidade sugerida é o ritmo de venda dos últimos " + secao.dados.janela + " dias para "
                                + secao.dados.diasCompra + " dias, menos o que já tem. Venda de encomenda não entra na conta." : ""
            font.pixelSize: 12
            Layout.fillWidth: true
        }
    }

    // ---------- Linhas ----------

    Component {
        id: cTitulo
        Item {
            readonly property var d: parent ? parent.dado : null
            implicitHeight: 38
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 4
                anchors.rightMargin: 4
                Rotulo { text: d ? d.texto : ""; Layout.alignment: Qt.AlignBottom; Layout.bottomMargin: 4 }
                Item { Layout.fillWidth: true }
                Botao {
                    visible: !!d && d.acao && secao.dados && secao.dados.encomendas.length > 1
                    text: "Marcar todas como pedidas"
                    tipo: "discreto"
                    altura: 32
                    font.pixelSize: 14
                    onClicked: secao.pedidas(secao.dados.encomendas.map(e => e.id))
                }
            }
        }
    }

    Component {
        id: cEncomenda
        Rectangle {
            id: le
            readonly property var e: parent ? parent.dado.e : ({})
            implicitHeight: Tema.celular ? 98 : 62
            radius: Tema.raio
            color: Tema.verde
            palette.window: Tema.superficie(color)
            HoverHandler { id: sobreE }
            TapHandler { onTapped: secao.linhaEscolhida = le.e.id }
            // M-1 da revisão do VIS2: no celular, a linha em três andares, como Encomendas e Parados já fazem. As colunas
            // de largura fixa do computador tomavam a largura toda do celular, e o nome ficava com largura zero.
            ColumnLayout {
                visible: Tema.celular
                anchors.fill: parent
                anchors.leftMargin: 14
                anchors.rightMargin: 12
                anchors.topMargin: 6
                anchors.bottomMargin: 6
                spacing: 0
                Text {
                    text: le.e.descricao
                    textFormat: Text.PlainText
                    color: Tema.tinta
                    font.family: Tema.titulo
                    font.weight: Font.Bold
                    font.pixelSize: 18
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                Text {
                    text: ["Nº " + le.e.numero + (le.e.caixa ? " (" + le.e.caixa + ")" : "") + " · " + le.e.cliente, le.e.desde, le.e.codigo ? "cód. " + le.e.codigo : "",
                           le.e.fornecedor ? le.e.fornecedor + (le.e.codigoFornecedor ? " (" + le.e.codigoFornecedor + ")" : "") : ""]
                          .filter(x => !!x).join("  ·  ")
                    textFormat: Text.PlainText
                    color: Tema.tintaSuave
                    font.family: Tema.serifa
                    font.italic: true
                    font.pixelSize: 13
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                RowLayout {
                    spacing: 10
                    Layout.fillWidth: true
                    Text {
                        text: "Comprar " + le.e.quantidade + " " + le.e.unidade
                        textFormat: Text.PlainText
                        color: Tema.verdeEscuro
                        font.family: Tema.texto
                        font.weight: Font.Bold
                        font.pixelSize: 16
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    Botao {
                        text: "Já pedi"
                        icone: "check"
                        tipo: "discreto"
                        altura: 36
                        onClicked: secao.pedidas([le.e.id])
                    }
                }
            }
            RowLayout {
                visible: !Tema.celular
                anchors.fill: parent
                anchors.leftMargin: 18
                anchors.rightMargin: 12
                spacing: 14
                ColumnLayout {
                    spacing: 0
                    Layout.fillWidth: true
                    Text {
                        text: le.e.descricao
                        textFormat: Text.PlainText
                        color: Tema.tinta
                        font.family: Tema.titulo
                        font.weight: Font.Bold
                        font.pixelSize: 20
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    Text {
                        text: ["Nº " + le.e.numero + (le.e.caixa ? " (" + le.e.caixa + ")" : "") + " · " + le.e.cliente, le.e.desde, le.e.codigo ? "cód. " + le.e.codigo : "",
                               le.e.fornecedor ? le.e.fornecedor + (le.e.codigoFornecedor ? " (" + le.e.codigoFornecedor + ")" : "") : ""]
                              .filter(x => !!x).join("  ·  ")
                        textFormat: Text.PlainText
                        color: Tema.tintaSuave
                        font.family: Tema.serifa
                        font.italic: true
                        font.pixelSize: 13
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                }
                Botao {
                    visible: (sobreE.hovered || Tema.celular || secao.linhaEscolhida === le.e.id)
                    text: "Já pedi"
                    icone: "check"
                    tipo: "discreto"
                    altura: 36
                    onClicked: secao.pedidas([le.e.id])
                }
                Text {
                    text: "Comprar " + le.e.quantidade + " " + le.e.unidade
                    textFormat: Text.PlainText
                    color: Tema.verdeEscuro
                    font.family: Tema.texto
                    font.weight: Font.Bold
                    font.pixelSize: 17
                    horizontalAlignment: Text.AlignRight
                    Layout.preferredWidth: 170
                }
            }
        }
    }

    Component {
        id: cPeca
        Rectangle {
            id: lp
            readonly property var p: parent ? parent.dado.p : ({})
            readonly property int i: parent ? parent.dado.i : 0
            implicitHeight: Tema.celular ? 100 : 64
            radius: Tema.raio
            // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
            readonly property bool par: (i + 1) % 2 === 0
            color: Tema.faixa(lp.par)
            palette.window: Tema.superficie(color)
            // M-1 da revisão do VIS2: no celular, a linha em três andares, como Encomendas e Parados já fazem. As colunas
            // de largura fixa do computador tomavam a largura toda do celular, e o nome ficava com largura zero.
            ColumnLayout {
                visible: Tema.celular
                anchors.fill: parent
                anchors.leftMargin: 14
                anchors.rightMargin: 12
                anchors.topMargin: 6
                anchors.bottomMargin: 6
                spacing: 0
                Text {
                    text: lp.p.nome + (lp.p.marca ? " · " + lp.p.marca : "")
                    textFormat: Text.PlainText
                    color: Tema.tintaDaFaixa(lp.par)
                    font.family: Tema.titulo
                    font.weight: Font.Bold
                    font.pixelSize: 18
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                    TapHandler { onTapped: secao.janela.abrir("TelaPeca", { produtoId: lp.p.produtoId }) }
                }
                Text {
                    text: ["cód. " + lp.p.codigo, lp.p.fornecedor ? lp.p.fornecedor + (lp.p.codigoFornecedor ? " (" + lp.p.codigoFornecedor + ")" : "") : "",
                           lp.p.observacao].filter(x => !!x).join("  ·  ")
                    textFormat: Text.PlainText
                    color: Tema.tintaSuaveDaFaixa(lp.par)
                    font.family: Tema.serifa
                    font.italic: true
                    font.pixelSize: 13
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                RowLayout {
                    spacing: 8
                    Layout.fillWidth: true
                    Chip {
                        texto: lp.p.nomePrioridade
                        color: lp.p.prioridade === "acabou" ? Tema.rosa : Tema.papel
                        corTexto: lp.p.prioridade === "minimo" ? Tema.tintaSuave : Tema.vermelho
                    }
                    Text {
                        text: [lp.p.tem, lp.p.ritmo, lp.p.custoTotal !== "0,00" ? "≈ R$ " + Tema.valor(lp.p.custoTotal) : ""]
                              .filter(x => !!x).join(" · ")
                        textFormat: Text.PlainText
                        color: Tema.tintaSuaveDaFaixa(lp.par)
                        font.family: Tema.texto
                        font.pixelSize: 12
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    Text {
                        text: "Comprar " + lp.p.sugerido + " " + lp.p.unidade
                        textFormat: Text.PlainText
                        color: Tema.tintaDaFaixa(lp.par)
                        font.family: Tema.texto
                        font.weight: Font.Bold
                        font.pixelSize: 15
                    }
                }
            }
            RowLayout {
                visible: !Tema.celular
                anchors.fill: parent
                anchors.leftMargin: 18
                anchors.rightMargin: 16
                spacing: 14
                Item {
                    Layout.preferredWidth: 132
                    implicitHeight: 26
                    Chip {
                        anchors.verticalCenter: parent.verticalCenter
                        texto: lp.p.nomePrioridade
                        color: lp.p.prioridade === "acabou" ? Tema.rosa : Tema.papel
                        corTexto: lp.p.prioridade === "minimo" ? Tema.tintaSuave : Tema.vermelho
                    }
                }
                ColumnLayout {
                    spacing: 0
                    Layout.fillWidth: true
                    Text {
                        text: lp.p.nome + (lp.p.marca ? " · " + lp.p.marca : "")
                        textFormat: Text.PlainText
                        color: Tema.tintaDaFaixa(lp.par)
                        font.family: Tema.titulo
                        font.weight: Font.Bold
                        font.pixelSize: 20
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                        TapHandler { onTapped: secao.janela.abrir("TelaPeca", { produtoId: lp.p.produtoId }) }
                        HoverHandler { cursorShape: Qt.PointingHandCursor }
                    }
                    Text {
                        text: ["cód. " + lp.p.codigo, lp.p.fornecedor ? lp.p.fornecedor + (lp.p.codigoFornecedor ? " (" + lp.p.codigoFornecedor + ")" : "") : "",
                               lp.p.observacao].filter(x => !!x).join("  ·  ")
                        textFormat: Text.PlainText
                        color: Tema.tintaSuaveDaFaixa(lp.par)
                        font.family: Tema.serifa
                        font.italic: true
                        font.pixelSize: 13
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                }
                ColumnLayout {
                    spacing: 0
                    Layout.preferredWidth: 150
                    Text {
                        text: lp.p.tem
                        textFormat: Text.PlainText
                        color: Tema.tintaDaFaixa(lp.par)
                        font.family: Tema.texto
                        font.weight: Font.DemiBold
                        font.pixelSize: 14
                        Layout.alignment: Qt.AlignRight
                    }
                    Text {
                        text: lp.p.ritmo
                        textFormat: Text.PlainText
                        color: Tema.tintaSuaveDaFaixa(lp.par)
                        font.family: Tema.texto
                        font.pixelSize: 12
                        Layout.alignment: Qt.AlignRight
                    }
                }
                ColumnLayout {
                    spacing: 0
                    Layout.preferredWidth: 150
                    Text {
                        text: "Comprar " + lp.p.sugerido + " " + lp.p.unidade
                        textFormat: Text.PlainText
                        color: Tema.tintaDaFaixa(lp.par)
                        font.family: Tema.texto
                        font.weight: Font.Bold
                        font.pixelSize: 17
                        Layout.alignment: Qt.AlignRight
                    }
                    Text {
                        text: lp.p.custoTotal !== "0,00" ? "≈ R$ " + Tema.valor(lp.p.custoTotal) : "sem custo"
                        textFormat: Text.PlainText
                        color: Tema.tintaSuaveDaFaixa(lp.par)
                        font.family: Tema.texto
                        font.pixelSize: 12
                        Layout.alignment: Qt.AlignRight
                    }
                }
            }
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
