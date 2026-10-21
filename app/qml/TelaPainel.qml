import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Painel: como a loja está indo. Resumo do dia, do mês e do ano, e quem
// compra, de quem a loja compra e como os clientes pagam.
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    property string secao: "resumo"
    property int dias: 30
    // Período da DRE e dos indicadores (o mês atual é o padrão).
    property var filtro: ({ periodo: "mes", de: "", ate: "", vendedorId: "" })
    property var vendedores: []
    readonly property bool comPeriodo: secao === "dre" || secao === "pecas"

    function escolherSecao(id) {
        if (id === "dre" || id === "pecas") {
            // Lucro, custos e despesas: só o gerente (ou ação liberada).
            janela.comGerente("resultados", () => {
                if (vendedores.length === 0) {
                    const u = Api.chamar("usuarios_listar")
                    vendedores = u.ok ? u.dados : []
                }
                secao = id
            }, true)
            return
        }
        secao = id
    }
    function mudarFiltro(mudancas) {
        filtro = Object.assign({}, filtro, mudancas)
    }

    function aoEsc() {
        janela.voltar()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        spacing: 14

        Cabecalho {
            sensivel: true
            titulo: "Painel"
            subtitulo: "Como a loja está indo"
            Layout.fillWidth: true
            onVoltar: tela.janela.voltar()
        }

        Linha {
            spacing: 14
            Layout.fillWidth: true
            Pilulas {
                modelo: [
                    { id: "resumo", nome: "Resumo" },
                    { id: "dre", nome: "Resultado (DRE)" },
                    { id: "pecas", nome: "Peças e vendedores" },
                    { id: "clientes", nome: "Clientes" },
                    { id: "fornecedores", nome: "Fornecedores" },
                    { id: "pagamentos", nome: "Pagamentos" }
                ]
                atual: tela.secao
                onEscolhido: id => tela.escolherSecao(id)
            }
            Item { Layout.fillWidth: true }
            Pilulas {
                visible: tela.secao !== "resumo" && !tela.comPeriodo
                pequenas: true
                modelo: [{ id: "30", nome: "30 dias" }, { id: "90", nome: "90 dias" }, { id: "365", nome: "12 meses" }]
                atual: String(tela.dias)
                onEscolhido: id => tela.dias = parseInt(id)
            }
        }

        // Período e vendedor da DRE e dos indicadores.
        Linha {
            visible: tela.comPeriodo
            spacing: 12
            Layout.fillWidth: true
            Pilulas {
                pequenas: true
                modelo: [
                    { id: "hoje", nome: "Hoje" },
                    { id: "7dias", nome: "7 dias" },
                    { id: "mes", nome: "Mês atual" },
                    { id: "mes_anterior", nome: "Mês anterior" },
                    { id: "ano", nome: "Ano" },
                    { id: "personalizado", nome: "Escolher datas" }
                ]
                atual: tela.filtro.periodo
                onEscolhido: id => {
                    if (id === "personalizado") {
                        escolhaDatas.visible = true
                        Qt.callLater(de.focar)
                        if (de.texto && ate.texto)
                            tela.mudarFiltro({ periodo: id, de: de.texto, ate: ate.texto })
                        return
                    }
                    escolhaDatas.visible = false
                    tela.mudarFiltro({ periodo: id })
                }
            }
            Item { Layout.fillWidth: true }
            Selecao {
                id: vendedor
                rotulo: ""
                modelo: ["Todos os vendedores"].concat(tela.vendedores.map(v => v.nome))
                Layout.preferredWidth: 220
                onEscolhido: tela.mudarFiltro({ vendedorId: indice > 0 ? tela.vendedores[indice - 1].id : "" })
            }
        }
        Linha {
            id: escolhaDatas
            visible: false
            spacing: 12
            Layout.fillWidth: true
            Campo { id: de; rotulo: "De"; dica: "dd/mm/aaaa"; maximo: 10; Layout.preferredWidth: 160; onConfirmado: ate.focar() }
            Campo { id: ate; rotulo: "Até"; dica: "dd/mm/aaaa"; maximo: 10; Layout.preferredWidth: 160; onConfirmado: verDatas.clicked() }
            Botao {
                id: verDatas
                text: "Ver"
                icone: "check"
                Layout.alignment: Qt.AlignBottom
                onClicked: tela.mudarFiltro({ periodo: "personalizado", de: de.texto, ate: ate.texto })
            }
            Item { Layout.fillWidth: true }
        }

        Loader {
            id: conteudo
            Layout.fillWidth: true
            Layout.fillHeight: true
            source: ({
                resumo: "SecaoResumo.qml",
                dre: "SecaoDre.qml",
                pecas: "SecaoIndicadores.qml",
                clientes: "SecaoClientes.qml",
                fornecedores: "SecaoFornecedores.qml",
                pagamentos: "SecaoPagamentos.qml"
            })[tela.secao]
            onLoaded: {
                if (item.hasOwnProperty("dias"))
                    item.dias = Qt.binding(() => tela.dias)
                if (item.hasOwnProperty("filtro"))
                    item.filtro = Qt.binding(() => tela.filtro)
                item.forceActiveFocus()
            }
        }
    }

    StackView.onActivated: {
        if (conteudo.item && typeof conteudo.item.carregar === "function")
            conteudo.item.carregar()
    }
}
