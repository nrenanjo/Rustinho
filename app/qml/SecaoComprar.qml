import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// O que comprar: as encomendas dos clientes (à parte) e a reposição do
// estoque, por prioridade, com a quantidade sugerida pelo ritmo de venda.
FocusScope {
    id: secao
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

    function carregar() {
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
        const r = Api.chamar("encomenda_pedir", { ids: ids })
        if (!r.ok) {
            janela.avisar(r.erro, "erro")
            return
        }
        janela.avisar(ids.length === 1 ? "Encomenda marcada como pedida." : ids.length + " encomendas marcadas como pedidas.", "ok")
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
            model: secao.linhas
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            delegate: Loader {
                required property var modelData
                width: lista.width
                sourceComponent: modelData.tipo === "titulo" ? cTitulo : modelData.tipo === "encomenda" ? cEncomenda : cPeca
                property var dado: modelData
            }

            ColumnLayout {
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
                    implicitHeight: 32
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
            implicitHeight: 62
            radius: Tema.raio
            color: Tema.verde
            HoverHandler { id: sobreE }
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 18
                anchors.rightMargin: 12
                spacing: 14
                ColumnLayout {
                    spacing: 0
                    Layout.fillWidth: true
                    Text {
                        text: le.e.descricao
                        color: Tema.tinta
                        font.family: Tema.titulo
                        font.weight: Font.Bold
                        font.pixelSize: 20
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    Text {
                        text: ["Nº " + le.e.numero + " · " + le.e.cliente, le.e.desde, le.e.codigo ? "cód. " + le.e.codigo : "",
                               le.e.fornecedor ? le.e.fornecedor + (le.e.codigoFornecedor ? " (" + le.e.codigoFornecedor + ")" : "") : ""]
                              .filter(x => !!x).join("  ·  ")
                        color: Tema.tintaSuave
                        font.family: Tema.serifa
                        font.italic: true
                        font.pixelSize: 13
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                }
                Botao {
                    visible: sobreE.hovered
                    text: "Já pedi"
                    icone: "check"
                    tipo: "discreto"
                    implicitHeight: 36
                    onClicked: secao.pedidas([le.e.id])
                }
                Text {
                    text: "Comprar " + le.e.quantidade + " " + le.e.unidade
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
            implicitHeight: 64
            radius: Tema.raio
            color: (i + 1) % 2 === 0 ? Tema.azul : Tema.pergaminho
            RowLayout {
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
                        color: Tema.tinta
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
                        color: Tema.tintaSuave
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
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.weight: Font.DemiBold
                        font.pixelSize: 14
                        Layout.alignment: Qt.AlignRight
                    }
                    Text {
                        text: lp.p.ritmo
                        color: Tema.tintaSuave
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
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.weight: Font.Bold
                        font.pixelSize: 17
                        Layout.alignment: Qt.AlignRight
                    }
                    Text {
                        text: lp.p.custoTotal !== "0,00" ? "≈ R$ " + Tema.valor(lp.p.custoTotal) : "sem custo"
                        color: Tema.tintaSuave
                        font.family: Tema.texto
                        font.pixelSize: 12
                        Layout.alignment: Qt.AlignRight
                    }
                }
            }
        }
    }

    Component.onCompleted: {
        carregar()
        busca.focar()
    }
}
