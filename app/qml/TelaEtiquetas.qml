import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Etiquetas de preço com código de barras, em folha A4 de etiquetas
// adesivas. Dá para começar numa folha já usada (escolhendo a primeira
// etiqueta livre).
FocusScope {
    id: tela
    property string parte: "pecas"
    readonly property var janela: ApplicationWindow.window
    // Vindo da peça ou da compra.
    property string produtoId: ""
    property string compraId: ""
    // [{ etiqueta: {...}, quantidade: n }]
    property var itens: []
    property var opcoes: null
    property string modelo: "33"
    property int inicio: 1
    property bool comBarras: true
    property bool comLoja: false
    property bool mostrarAjuste: false

    readonly property var m: opcoes ? (opcoes.modelos.find(x => x.id === modelo) || opcoes.modelos[1]) : null
    readonly property int porFolha: m ? m.colunas * m.linhas : 33
    readonly property int total: itens.reduce((s, i) => s + (i.quantidade || 0), 0)
    readonly property int folhas: total > 0 ? Math.ceil((inicio - 1 + total) / porFolha) : 0

    function carregar() {
        const r = Api.chamar("etiquetas_opcoes")
        if (r.ok) {
            opcoes = r.dados
            modelo = r.dados.modelo
            comBarras = r.dados.barras
            comLoja = r.dados.loja
            ajusteX.texto = r.dados.ajusteX ? String(r.dados.ajusteX / 10).replace(".", ",") : ""
            ajusteY.texto = r.dados.ajusteY ? String(r.dados.ajusteY / 10).replace(".", ",") : ""
        }
        if (produtoId !== "") {
            adicionar(produtoId, 1)
            produtoId = ""
        }
        if (compraId !== "") {
            const c = Api.chamar("etiquetas_da_compra", { id: compraId })
            compraId = ""
            if (c.ok)
                itens = c.dados
        }
    }

    function adicionar(id, quantidade) {
        const lista = itens.slice()
        const i = lista.findIndex(x => x.etiqueta.produtoId === id)
        if (i >= 0) {
            lista[i] = { etiqueta: lista[i].etiqueta, quantidade: lista[i].quantidade + quantidade }
        } else {
            const r = Api.chamar("etiqueta_previa", { id: id })
            if (!r.ok) {
                janela.avisar(r.erro, "erro")
                return
            }
            lista.push({ etiqueta: r.dados, quantidade: quantidade })
        }
        itens = lista
    }

    function mudarQuantidade(indice, texto) {
        const lista = itens.slice()
        lista[indice] = { etiqueta: lista[indice].etiqueta, quantidade: Math.max(0, Math.min(999, parseInt(texto) || 0)) }
        itens = lista
    }

    function tirar(indice) {
        const lista = itens.slice()
        lista.splice(indice, 1)
        itens = lista
    }

    // Leitor de código de barras: soma 1; senão abre a busca.
    function confirmarBusca() {
        const t = busca.texto.trim()
        if (t === "") {
            escolher.abrirCom("", "")
            return
        }
        const r = Api.chamar("produto_por_codigo", { texto: t })
        if (r.ok && r.dados.id) {
            adicionar(r.dados.id, 1)
            busca.texto = ""
        } else {
            escolher.abrirCom("", t)
            busca.texto = ""
        }
    }

    function decimos(t) {
        const v = parseFloat(String(t || "").replace(",", "."))
        return isNaN(v) ? 0 : Math.round(v * 10)
    }

    function imprimir() {
        const pedido = {
            itens: itens.map(i => ({ produtoId: i.etiqueta.produtoId, quantidade: i.quantidade })),
            modelo: modelo, inicio: inicio, barras: comBarras, loja: comLoja,
            ajusteX: decimos(ajusteX.texto), ajusteY: decimos(ajusteY.texto)
        }
        const r = JSON.parse(Sistema.imprimir("etiquetas", JSON.stringify(pedido)))
        if (!r.ok)
            janela.avisar(r.erro, "erro")
        else if (r.dados.impresso)
            janela.avisar("Enviado para a impressora.", "ok")
    }

    function aoEsc() {
        janela.voltar()
    }
    function focarBusca() {
        busca.focar()
    }

    Shortcut {
        sequence: "F2"
        enabled: tela.StackView.status === StackView.Active
        onActivated: tela.imprimir()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        spacing: 14

        Cabecalho {
            titulo: "Etiquetas"
            subtitulo: "Preço e código de barras em folha A4 de etiquetas adesivas"
            textoVoltar: "Voltar"
            Layout.fillWidth: true
            onVoltar: tela.janela.voltar()
            Rotulo {
                visible: tela.total > 0
                text: tela.total + (tela.total === 1 ? " etiqueta" : " etiquetas") + " · " + tela.folhas + (tela.folhas === 1 ? " folha" : " folhas")
                Layout.alignment: Qt.AlignVCenter
                Layout.rightMargin: 6
            }
            Botao {
                text: "Imprimir"
                icone: "impressora"
                atalho: "F2"
                enabled: tela.total > 0
                onClicked: tela.imprimir()
            }
        }

        // Celular: peças e folha em pílulas (uma parte de cada vez).
        Pilulas {
            visible: Tema.estreito
            modelo: [{ id: "pecas", nome: "Peças (" + tela.itens.length + ")" }, { id: "folha", nome: "Folha e ajuste" }]
            atual: tela.parte
            onEscolhido: id => tela.parte = id
        }

        RowLayout {
            spacing: 18
            Layout.fillWidth: true
            Layout.fillHeight: true

            // ---------- Peças ----------
            ColumnLayout {
                visible: !Tema.estreito || tela.parte === "pecas"
                spacing: 10
                Layout.fillWidth: true
                Layout.fillHeight: true
                Campo {
                    id: busca
                    icone: "busca"
                    camera: true
                    altura: 50
                    tamanhoFonte: 17
                    dica: Tema.estreito ? "Código ou nome da peça" : "Passe o leitor na peça ou digite o nome e Enter"
                    Layout.fillWidth: true
                    onConfirmado: tela.confirmarBusca()
                }
                ListView {
                    id: lista
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 6
                    model: tela.itens
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar {}
                    delegate: Rectangle {
                        id: linha
                        required property var modelData
                        required property int index
                        readonly property var e: modelData.etiqueta
                        width: lista.width
                        implicitHeight: 62
                        radius: Tema.raio
                        color: (index + 1) % 2 === 0 ? Tema.azul : Tema.pergaminho
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: Tema.estreito ? 12 : 18
                            anchors.rightMargin: 10
                            spacing: Tema.estreito ? 8 : 12
                            ColumnLayout {
                                spacing: 0
                                Layout.fillWidth: true
                                Text {
                                    text: linha.e.nome
                                    color: Tema.tinta
                                    font.family: Tema.titulo
                                    font.weight: Font.Bold
                                    font.pixelSize: 19
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                                Text {
                                    text: ["cód. " + linha.e.codigo, linha.e.textoBarras !== linha.e.codigo ? "barras " + linha.e.textoBarras : ""].filter(x => !!x).join("  ·  ")
                                    color: Tema.tintaSuave
                                    font.family: Tema.serifa
                                    font.italic: true
                                    font.pixelSize: 13
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                            }
                            Chip {
                                texto: linha.e.semPreco ? "sem preço" : ""
                                color: Tema.rosa
                                corTexto: Tema.vermelho
                            }
                            Text {
                                text: linha.e.semPreco ? "" : "R$ " + linha.e.preco
                                color: Tema.tinta
                                font.family: Tema.texto
                                font.weight: Font.Bold
                                font.pixelSize: 16
                                Layout.preferredWidth: Tema.estreito ? -1 : 100
                                horizontalAlignment: Text.AlignRight
                            }
                            Campo {
                                numerico: true
                                maximo: 3
                                sufixo: "etiq."
                                texto: String(linha.modelData.quantidade)
                                Layout.preferredWidth: Tema.estreito ? 84 : 120
                                Layout.alignment: Qt.AlignVCenter
                                onEditado: tela.mudarQuantidade(linha.index, texto)
                            }
                            Botao {
                                tipo: "discreto"
                                icone: "lixeira"
                                text: ""
                                implicitHeight: 36
                                onClicked: tela.tirar(linha.index)
                            }
                        }
                    }
                    ColumnLayout {
                        anchors.centerIn: parent
                        width: Math.min(parent.width - 40, 560)
                        visible: lista.count === 0
                        spacing: 8
                        Titulo {
                            text: "Quais peças?"
                            font.pixelSize: 30
                            horizontalAlignment: Text.AlignHCenter
                            Layout.fillWidth: true
                        }
                        Nota {
                            text: (Tema.celular ? "Digite o código ou o nome da peça, ou use a câmera (cada leitura é mais uma etiqueta). " : "Passe o leitor no código de barras da peça (cada passada é mais uma etiqueta) ou digite o nome e Enter. ")
                                  + "Na compra lançada e no cadastro da peça também tem o botão Etiquetas."
                            horizontalAlignment: Text.AlignHCenter
                            Layout.fillWidth: true
                        }
                    }
                }
            }

            // ---------- Folha ----------
            Flickable {
                visible: !Tema.estreito || tela.parte === "folha"
                Layout.fillWidth: Tema.estreito
                Layout.preferredWidth: Tema.estreito ? -1 : 420
                Layout.fillHeight: true
                contentHeight: lado.implicitHeight
                clip: true
                boundsBehavior: Flickable.StopAtBounds

                ColumnLayout {
                    id: lado
                    width: parent.width
                    spacing: 14

                    Faixa {
                        titulo: "Folha de etiquetas"
                        color: Tema.pergaminho
                        Layout.fillWidth: true
                        Pilulas {
                            pequenas: true
                            modelo: tela.opcoes ? tela.opcoes.modelos.map(x => ({ id: x.id, nome: x.nome })) : []
                            atual: tela.modelo
                            onEscolhido: id => {
                                tela.modelo = id
                                tela.inicio = 1
                            }
                        }
                        Nota {
                            text: tela.m ? tela.m.detalhe : ""
                            font.pixelSize: 13
                            Layout.fillWidth: true
                        }
                    }

                    Faixa {
                        titulo: "Começar na etiqueta " + tela.inicio
                        color: Tema.azul
                        Layout.fillWidth: true
                        // A folha em miniatura.
                        Item {
                            Layout.fillWidth: true
                            implicitHeight: folha.height
                            Rectangle {
                                id: folha
                                readonly property real escala: Math.min(parent.width, 230) / 210
                                anchors.horizontalCenter: parent.horizontalCenter
                                width: 210 * escala
                                height: 297 * escala
                                radius: 4
                                color: Tema.papel
                                Repeater {
                                    model: tela.m ? tela.porFolha : 0
                                    delegate: Rectangle {
                                        required property int index
                                        readonly property int col: index % tela.m.colunas
                                        readonly property int lin: Math.floor(index / tela.m.colunas)
                                        readonly property bool usada: index + 1 < tela.inicio
                                        readonly property bool vai: index + 1 >= tela.inicio && index + 1 < tela.inicio + tela.total
                                        x: (tela.m.esquerda + col * (tela.m.largura + tela.m.espacoH)) * folha.escala
                                        y: (tela.m.topo + lin * (tela.m.altura + tela.m.espacoV)) * folha.escala
                                        width: tela.m.largura * folha.escala - 1
                                        height: tela.m.altura * folha.escala - 1
                                        radius: Math.min(width, height) * 0.18
                                        color: index + 1 === tela.inicio ? Tema.verdeEscuro
                                               : vai ? Qt.rgba(Tema.verdeEscuro.r, Tema.verdeEscuro.g, Tema.verdeEscuro.b, 0.35)
                                               : usada ? Tema.sombra(0.08) : Qt.rgba(Tema.dourado.r, Tema.dourado.g, Tema.dourado.b, 0.28)
                                        HoverHandler { cursorShape: Qt.PointingHandCursor }
                                        TapHandler { onTapped: tela.inicio = index + 1 }
                                    }
                                }
                            }
                        }
                        Nota {
                            text: "Folha já usada? Clique na primeira etiqueta livre."
                            font.pixelSize: 13
                            Layout.fillWidth: true
                        }
                    }

                    Faixa {
                        titulo: "Na etiqueta"
                        color: Tema.verde
                        corTitulo: Tema.verdeEscuro
                        Layout.fillWidth: true
                        Marcar { text: "Código de barras"; checked: tela.comBarras; onToggled: tela.comBarras = checked }
                        Marcar { text: "Nome da loja"; checked: tela.comLoja; onToggled: tela.comLoja = checked }
                        // Prévia da primeira etiqueta.
                        Rectangle {
                            visible: tela.itens.length > 0 && !!tela.m
                            id: previa
                            readonly property var e: tela.itens.length > 0 ? tela.itens[0].etiqueta : null
                            readonly property var mm: tela.m || { largura: 63.5, altura: 25.4 }
                            readonly property real escala: Math.min(320 / mm.largura, 140 / mm.altura)
                            Layout.alignment: Qt.AlignHCenter
                            implicitWidth: mm.largura * escala
                            implicitHeight: mm.altura * escala
                            radius: 8
                            color: "white"
                            ColumnLayout {
                                anchors.fill: parent
                                anchors.margins: 1.6 * previa.escala
                                spacing: 0
                                Text {
                                    visible: tela.comLoja
                                    text: tela.janela ? (tela.janela.estado.loja || "").toUpperCase() : ""
                                    color: "#5C534A"
                                    font.family: Tema.texto
                                    font.weight: Font.DemiBold
                                    font.pixelSize: Math.max(6, 2.0 * previa.escala)
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                                Text {
                                    text: previa.e ? previa.e.nome : ""
                                    color: "#2A2622"
                                    font.family: Tema.texto
                                    font.weight: Font.DemiBold
                                    font.pixelSize: Math.max(7, (previa.mm.altura < 23 ? 2.3 : 2.7) * previa.escala)
                                    wrapMode: Text.WordWrap
                                    maximumLineCount: previa.mm.altura < 23 && tela.comBarras ? 1 : 2
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                                Item { Layout.fillHeight: true }
                                RowLayout {
                                    Layout.fillWidth: true
                                    Text {
                                        text: previa.e && !previa.e.semPreco ? "R$ " + previa.e.preco : ""
                                        color: "#2A2622"
                                        font.family: Tema.texto
                                        font.weight: Font.Bold
                                        font.pixelSize: (previa.mm.altura < 23 ? 3.9 : previa.mm.altura > 30 ? 6 : 4.6) * previa.escala * (tela.comBarras ? 1 : 1.25)
                                        Layout.fillWidth: true
                                    }
                                    Text {
                                        text: previa.e ? "cód. " + previa.e.codigo : ""
                                        color: "#5C534A"
                                        font.family: Tema.texto
                                        font.pixelSize: Math.max(6, 2.0 * previa.escala)
                                        Layout.alignment: Qt.AlignBottom
                                    }
                                }
                                // Barras
                                Item {
                                    visible: tela.comBarras
                                    Layout.fillWidth: true
                                    implicitHeight: (previa.mm.altura < 23 ? 5 : previa.mm.altura > 30 ? 11 : 6.6) * previa.escala
                                    Row {
                                        readonly property real modulo: previa.e ? Math.min(0.33 * previa.escala, parent.width / (previa.e.barras.length + 12)) : 1
                                        anchors.horizontalCenter: parent.horizontalCenter
                                        height: parent.height
                                        Repeater {
                                            model: previa.e && tela.comBarras ? previa.e.barras.split("") : []
                                            delegate: Rectangle {
                                                required property var modelData
                                                width: parent.modulo
                                                height: parent.height
                                                color: modelData === "1" ? "#2A2622" : "transparent"
                                            }
                                        }
                                    }
                                }
                                Text {
                                    visible: tela.comBarras
                                    text: previa.e ? previa.e.textoBarras : ""
                                    color: Tema.tinta
                                    font.family: Tema.texto
                                    font.pixelSize: Math.max(6, 1.8 * previa.escala)
                                    Layout.alignment: Qt.AlignHCenter
                                }
                            }
                        }
                    }

                    Botao {
                        text: tela.mostrarAjuste ? "Esconder o ajuste fino" : "Saiu torto? Ajuste fino"
                        icone: tela.mostrarAjuste ? "cima" : "baixo"
                        tipo: "discreto"
                        Layout.leftMargin: -10
                        onClicked: tela.mostrarAjuste = !tela.mostrarAjuste
                    }
                    Faixa {
                        visible: tela.mostrarAjuste
                        titulo: "Ajuste fino da impressora"
                        color: Tema.pergaminho
                        Layout.fillWidth: true
                        RowLayout {
                            spacing: 12
                            Layout.fillWidth: true
                            Campo { id: ajusteX; rotulo: "Para a direita"; sufixo: "mm"; numerico: true; maximo: 5; dica: "0"; Layout.fillWidth: true }
                            Campo { id: ajusteY; rotulo: "Para baixo"; sufixo: "mm"; numerico: true; maximo: 5; dica: "0"; Layout.fillWidth: true }
                        }
                        Nota {
                            text: "Imprima numa folha comum, ponha por cima da folha de etiquetas contra a luz e veja quanto precisa mover. Use menos (−) para o outro lado."
                            font.pixelSize: 13
                            Layout.fillWidth: true
                        }
                    }
                }
            }
        }
    }

    EscolherPeca {
        id: escolher
        onEscolhida: id => tela.adicionar(id, 1)
    }

    StackView.onActivated: busca.focar()
    Component.onCompleted: carregar()
}
