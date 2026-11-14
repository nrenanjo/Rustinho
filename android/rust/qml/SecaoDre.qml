import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Resultado do Painel: a DRE gerencial (docs/DRE-gerencial.md). Linhas com
// subtotais, comparação, pendências, os últimos 12 meses e as despesas por
// categoria. Cada total abre os lançamentos que o compõem.
FocusScope {
    id: secao
    readonly property var janela: ApplicationWindow.window
    property var filtro: ({ periodo: "mes", de: "", ate: "", vendedorId: "" })
    property var d: null
    property var serie: []
    property var desp: null

    onFiltroChanged: carregar()

    function carregar() {
        const x = Api.chamar("dre_apurar", filtro)
        if (!x.ok) {
            janela.avisar(x.erro, "erro")
            return
        }
        d = x.dados
        const s = Api.chamar("dre_serie")
        serie = s.ok ? s.dados : []
        const e = Api.chamar("dre_despesas", Object.assign({ categoria: "" }, filtro))
        desp = e.ok ? e.dados : null
    }
    function abrirLinha(id) {
        detalhe.abrir("dre_detalhe", Object.assign({ linha: id }, filtro))
    }
    function abrirCategoria(nome, total) {
        const r = Api.chamar("dre_despesas", Object.assign({ categoria: nome }, filtro))
        if (!r.ok) {
            janela.avisar(r.erro, "erro")
            return
        }
        detalhe.d = { nome: nome, total: total, quantidade: r.dados.lancamentos.length, itens: r.dados.lancamentos, cortado: false }
        detalhe.erro = ""
        detalhe.open()
    }
    // Com o olho fechado, os valores dentro dos textos também somem.
    function sigilo(t) {
        return Tema.oculto ? String(t).replace(/R\$ ?-?[\d.]+,\d\d/g, "R$ •••") : t
    }
    function imprimir() {
        const x = JSON.parse(Sistema.imprimir("dre", JSON.stringify(filtro)))
        if (!x.ok)
            janela.avisar(x.erro, "erro")
        else if (x.dados.impresso)
            janela.avisar("Enviado para a impressora.", "ok")
    }
    readonly property color corEstado: !d ? Tema.pergaminho : d.estado === "completo" ? Tema.verde
                                       : d.estado === "incompleto" ? Tema.amarelo : Tema.pergaminho
    readonly property var barrasSerie: {
        const maior = Math.max(1, ...serie.map(m => m.vVendas))
        return serie.map((m, i) => ({
            rotulo: m.rotulo,
            valor: m.vendas || "0,00",
            parte: m.vVendas > 0 ? m.vVendas / maior : 0,
            destaque: i === serie.length - 1,
            futuro: m.estado === "antes",
            dica: m.estado === "antes" ? "antes do primeiro registro"
                  : m.estado === "sem_dados" ? "sem dados (não é zero comprovado)"
                  : "lucro bruto R$ " + m.lucroBruto + " · despesas R$ " + m.despesas
                    + " · resultado R$ " + m.resultado + (m.estado === "incompleto" ? " (incompleto)" : "")
                    + (m.parcial ? " · mês em andamento" : "")
        }))
    }

    DetalheDre { id: detalhe }
    ClassificarCategorias { id: classificar; onSalvo: secao.carregar() }
    MetasDre { id: metas }

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

            // Estado: completo, incompleto ou sem dados.
            Rectangle {
                Layout.fillWidth: true
                implicitHeight: estado.implicitHeight + 28
                radius: Tema.raio
                color: secao.corEstado
                palette.window: Tema.superficie(color)
                Linha {
                    id: estado
                    anchors.fill: parent
                    anchors.margins: 14
                    anchors.leftMargin: 20
                    spacing: 12
                    ColumnLayout {
                        spacing: 2
                        Layout.fillWidth: true
                        Rotulo { text: secao.d ? secao.d.periodo.rotulo + (secao.d.vendedor ? " · " + secao.d.vendedor : "") : "" }
                        Text {
                            text: secao.d ? secao.d.estadoTexto : ""
                            textFormat: Text.PlainText
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.DemiBold
                            font.pixelSize: 15
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                        Nota {
                            text: secao.d ? secao.d.base + ". Não substitui a contabilidade." : ""
                            font.pixelSize: 13
                            Layout.fillWidth: true
                        }
                    }
                    Botao { text: "Classificar categorias"; tipo: "secundario"; icone: "ajuste"; onClicked: classificar.abrir() }
                    Botao { text: "Metas"; tipo: "secundario"; onClicked: metas.abrir() }
                    Botao { text: "Imprimir"; tipo: "secundario"; icone: "impressora"; onClicked: secao.imprimir() }
                }
            }

            Linha {
                spacing: 14
                Layout.fillWidth: true

                // A DRE.
                Faixa {
                    titulo: "DRE gerencial"
                    subtitulo: "toque numa linha para ver de onde vem"
                    color: Tema.pergaminho
                    espacamento: 2
                    Layout.fillWidth: true
                    Layout.preferredWidth: 3
                    Layout.alignment: Qt.AlignTop
                    Repeater {
                        model: secao.d ? secao.d.linhas : []
                        delegate: Rectangle {
                            id: linhaDre
                            required property var modelData
                            required property int index
                            Layout.fillWidth: true
                            // A15 (0.9.18): no celular, cada linha com 48 dp (Tema.toqueMinimo).
                            implicitHeight: Math.max(modelData.subtotal ? 40 : 32, Tema.toqueMinimo)
                            radius: Tema.raioCampo
                            // Faixas alternadas (V-6): no cinza e no AMOLED, o subtotal
                            // continua com o véu dele e as outras linhas alternam com um
                            // véu leve; no roxo, faixa clara e escura (o subtotal em negrito).
                            readonly property bool par: (index + 1) % 2 === 0
                            readonly property color base: modelData.subtotal && !Tema.roxo ? Tema.vela(0.28)
                                                          : Tema.faixa(linhaDre.par, "transparent", Tema.vela(0.16))
                            color: sobre.hovered && modelData.detalhavel ? (Tema.roxo ? Qt.darker(base, 1.06) : Tema.vela(0.5)) : base
                            palette.window: Tema.superficie(color)
                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: linhaDre.modelData.subtotal ? 10 : 22
                                anchors.rightMargin: 10
                                spacing: 10
                                Text {
                                    text: linhaDre.modelData.nome
                                    textFormat: Text.PlainText
                                    color: Tema.tintaDaFaixa(linhaDre.par)
                                    font.family: linhaDre.modelData.subtotal ? Tema.titulo : Tema.texto
                                    font.weight: linhaDre.modelData.subtotal ? Font.Bold : Font.Normal
                                    font.pixelSize: linhaDre.modelData.subtotal ? 18 : 14
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                                Text {
                                    text: Tema.oculto ? "" : linhaDre.modelData.percentual
                                    textFormat: Text.PlainText
                                    color: Tema.apagadoDaFaixa(linhaDre.par)
                                    font.family: Tema.texto
                                    font.pixelSize: 12
                                    horizontalAlignment: Text.AlignRight
                                    Layout.preferredWidth: 56
                                }
                                Text {
                                    text: "R$ " + Tema.valor(linhaDre.modelData.valor)
                                    textFormat: Text.PlainText
                                    color: linhaDre.modelData.subtotal && linhaDre.modelData.v < 0 && !Tema.oculto ? Tema.vermelhoDaFaixa(linhaDre.par) : Tema.tintaDaFaixa(linhaDre.par)
                                    font.family: Tema.texto
                                    font.weight: linhaDre.modelData.subtotal ? Font.Bold : Font.Normal
                                    font.pixelSize: linhaDre.modelData.subtotal ? 16 : 14
                                    horizontalAlignment: Text.AlignRight
                                    Layout.minimumWidth: 120
                                }
                                Icone {
                                    nome: "direita"
                                    tamanho: 14
                                    cor: Tema.apagadoDaFaixa(linhaDre.par)
                                    opacity: linhaDre.modelData.detalhavel ? 1 : 0
                                }
                            }
                            HoverHandler { id: sobre; cursorShape: linhaDre.modelData.detalhavel ? Qt.PointingHandCursor : Qt.ArrowCursor }
                            TapHandler { enabled: linhaDre.modelData.detalhavel; onTapped: secao.abrirLinha(linhaDre.modelData.id) }
                        }
                    }
                    Nota {
                        visible: !!secao.d && secao.d.margemBruta !== ""
                        text: secao.d ? "Margem bruta " + (Tema.oculto ? "••" : secao.d.margemBruta)
                                        + (secao.d.margemContribuicao ? "  ·  margem de contribuição " + (Tema.oculto ? "••" : secao.d.margemContribuicao) : "") : ""
                        font.pixelSize: 14
                        Layout.topMargin: 8
                        Layout.fillWidth: true
                    }
                }

                ColumnLayout {
                    spacing: 14
                    Layout.fillWidth: true
                    Layout.preferredWidth: 2
                    Layout.alignment: Qt.AlignTop

                    Faixa {
                        visible: !!secao.d && secao.d.pendencias.length > 0
                        titulo: "Pendências"
                        subtitulo: "deixam o resultado incompleto"
                        color: Tema.amarelo
                        corTitulo: Tema.tinta
                        espacamento: 8
                        Layout.fillWidth: true
                        Repeater {
                            model: secao.d ? secao.d.pendencias : []
                            delegate: Text {
                                id: pend
                                required property var modelData
                                readonly property bool abre: modelData.codigo === "sem_custo" || modelData.codigo === "sem_classificacao"
                                text: "• " + secao.sigilo(modelData.texto) + (abre ? "  (toque para ver)" : "")
                                textFormat: Text.PlainText
                                color: Tema.tinta
                                font.family: Tema.texto
                                font.pixelSize: 14
                                wrapMode: Text.WordWrap
                                Layout.fillWidth: true
                                HoverHandler { enabled: pend.abre; cursorShape: Qt.PointingHandCursor }
                                TapHandler { enabled: pend.abre; onTapped: secao.abrirLinha(pend.modelData.codigo) }
                            }
                        }
                    }

                    Faixa {
                        titulo: "Comparação"
                        subtitulo: secao.d ? "com " + secao.d.comparacao.rotulo : ""
                        color: Tema.azul
                        espacamento: 6
                        Layout.fillWidth: true
                        Repeater {
                            model: secao.d ? secao.d.variacoes : []
                            delegate: RowLayout {
                                id: varia
                                required property var modelData
                                readonly property bool dinheiro: modelData.nome !== "Quantidade de vendas" && modelData.nome !== "Margem bruta"
                                spacing: 8
                                Layout.fillWidth: true
                                Text {
                                    text: varia.modelData.nome
                                    textFormat: Text.PlainText
                                    color: Tema.tinta
                                    font.family: Tema.texto
                                    font.pixelSize: 14
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                                Text {
                                    text: Tema.oculto && varia.dinheiro ? "•••" : (varia.dinheiro ? "R$ " : "") + varia.modelData.atual
                                    textFormat: Text.PlainText
                                    color: Tema.tinta
                                    font.family: Tema.texto
                                    font.weight: Font.Bold
                                    font.pixelSize: 14
                                    horizontalAlignment: Text.AlignRight
                                    Layout.minimumWidth: 90
                                }
                                Text {
                                    text: Tema.oculto && varia.dinheiro ? "•••" : "antes " + (varia.dinheiro && varia.modelData.anterior ? "R$ " : "") + (varia.modelData.anterior || "—")
                                    textFormat: Text.PlainText
                                    color: Tema.apagado
                                    font.family: Tema.texto
                                    font.pixelSize: 12
                                    horizontalAlignment: Text.AlignRight
                                    Layout.minimumWidth: 90
                                }
                                Text {
                                    text: Tema.oculto && varia.dinheiro ? "" : varia.modelData.variacao
                                    textFormat: Text.PlainText
                                    // H17 (7.3): verde quando a mudança é boa (despesa que sobe é piora).
                                    color: varia.modelData.melhorou ? Tema.verdeEscuro : Tema.vermelho
                                    font.family: Tema.texto
                                    font.weight: Font.DemiBold
                                    font.pixelSize: 13
                                    horizontalAlignment: Text.AlignRight
                                    Layout.preferredWidth: 78
                                }
                            }
                        }
                        Nota {
                            visible: !!secao.d && !secao.d.resultadoComparavel
                            text: "Despesas e resultado não se comparam aqui: os períodos não têm as mesmas contas mensais."
                            font.pixelSize: 13
                            Layout.fillWidth: true
                        }
                    }

                    Faixa {
                        titulo: "Vendas no período"
                        color: Tema.verde
                        corTitulo: Tema.verdeEscuro
                        espacamento: 4
                        Layout.fillWidth: true
                        Repeater {
                            model: !secao.d ? [] : [
                                ["Vendas líquidas", "R$ " + secao.d.comercial.vendasLiquidas],
                                ["Quantidade de vendas", String(secao.d.comercial.quantidade)],
                                ["Ticket médio", secao.d.comercial.ticket ? "R$ " + secao.d.comercial.ticket : "—"],
                                ["Peças por venda", secao.d.comercial.itensPorVenda || "—"],
                                ["Descontos concedidos", "R$ " + secao.d.comercial.descontos],
                                ["Devoluções", secao.d.comercial.devolucoes + " · R$ " + secao.d.comercial.devolucoesValor],
                                ["Vendas canceladas", secao.d.comercial.canceladas + " · R$ " + secao.d.comercial.canceladasValor]
                            ]
                            delegate: RowLayout {
                                required property var modelData
                                spacing: 8
                                Layout.fillWidth: true
                                Text { text: modelData[0]; textFormat: Text.PlainText; color: Tema.tintaSuave; font.family: Tema.texto; font.pixelSize: 14; Layout.fillWidth: true }
                                Text { text: Tema.sigiloTexto(modelData[1]); textFormat: Text.PlainText; color: Tema.tinta; font.family: Tema.texto; font.weight: Font.Bold; font.pixelSize: 14 }
                            }
                        }
                    }

                    Faixa {
                        visible: !!secao.d && secao.d.avisos.length > 0
                        titulo: "Como foi calculado"
                        color: Tema.pergaminho
                        espacamento: 6
                        Layout.fillWidth: true
                        Repeater {
                            model: secao.d ? secao.d.avisos : []
                            delegate: Nota {
                                required property var modelData
                                text: secao.sigilo(modelData)
                                font.pixelSize: 13
                                Layout.fillWidth: true
                            }
                        }
                    }
                }
            }

            Linha {
                spacing: 14
                Layout.fillWidth: true

                Faixa {
                    titulo: "Últimos 12 meses"
                    subtitulo: "vendas líquidas; passe o mouse para o resultado"
                    color: Tema.azul
                    espacamento: 8
                    Layout.fillWidth: true
                    Layout.preferredWidth: 3
                    Layout.alignment: Qt.AlignTop
                    GraficoBarras {
                        barras: secao.barrasSerie
                        alturaBarras: 110
                    }
                    Repeater {
                        model: secao.serie.slice().reverse()
                        delegate: Rectangle {
                            id: mes
                            required property var modelData
                            Layout.fillWidth: true
                            visible: modelData.estado !== "antes"
                            required property int index
                            // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                            readonly property bool par: (index + 1) % 2 === 0
                            color: Tema.faixa(mes.par, Tema.vela(0.4), "transparent")
                            palette.window: Tema.superficie(color)
                            radius: Tema.raioCampo
                            implicitHeight: conteudoDaFaixa.implicitHeight + 6
                            implicitWidth: conteudoDaFaixa.implicitWidth + 16
                            RowLayout {
                                id: conteudoDaFaixa
                                x: 8
                                y: 3
                                width: mes.width - 16
                                spacing: 8
                                Text { text: mes.modelData.rotulo; textFormat: Text.PlainText; color: Tema.apagadoDaFaixa(mes.par); font.family: Tema.texto; font.pixelSize: 13; Layout.preferredWidth: 56 }
                                Text {
                                    text: mes.modelData.estado === "sem_dados" ? "sem dados" : "vendas R$ " + Tema.valor(mes.modelData.vendas)
                                    textFormat: Text.PlainText
                                    color: Tema.tintaDaFaixa(mes.par); font.family: Tema.texto; font.pixelSize: 13; Layout.fillWidth: true
                                }
                                Text {
                                    visible: mes.modelData.estado !== "sem_dados"
                                    text: "resultado R$ " + Tema.valor(mes.modelData.resultado)
                                    textFormat: Text.PlainText
                                    color: mes.modelData.vResultado < 0 && !Tema.oculto ? Tema.vermelhoDaFaixa(mes.par) : Tema.tintaDaFaixa(mes.par)
                                    font.family: Tema.texto; font.weight: Font.DemiBold; font.pixelSize: 13
                                }
                                Chip {
                                    visible: mes.modelData.estado === "incompleto" || mes.modelData.parcial
                                    texto: mes.modelData.parcial ? "em andamento" : "incompleto"
                                }
                            }
                        }
                    }
                }

                Faixa {
                    titulo: "Despesas e tributos por categoria"
                    subtitulo: secao.desp ? "total R$ " + Tema.valor(secao.desp.total) : ""
                    color: Tema.rosa
                    espacamento: 12
                    Layout.fillWidth: true
                    Layout.preferredWidth: 2
                    Layout.alignment: Qt.AlignTop
                    Repeater {
                        model: secao.desp ? secao.desp.categorias : []
                        delegate: LinhaRanking {
                            required property var modelData
                            required property int index
                            indice: index
                            item: ({ nome: modelData.categoria, detalhe: modelData.grupo, valor: modelData.valor, parte: modelData.parte, percentual: "" })
                            cor: Tema.vermelho
                            HoverHandler { cursorShape: Qt.PointingHandCursor }
                            TapHandler { onTapped: secao.abrirCategoria(modelData.categoria, modelData.valor) }
                        }
                    }
                    Nota {
                        visible: !!secao.desp && secao.desp.categorias.length === 0
                        text: "Nenhuma despesa registrada no período."
                        font.pixelSize: 14
                    }
                }
            }
        }
    }

    Component.onCompleted: carregar()
    Component.onDestruction: Api.chamar("gerente_encerrar")
}
