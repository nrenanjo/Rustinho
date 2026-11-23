import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Metas mensais (padrão) ou semanais, da loja ou de um vendedor: realizado,
// meta, diferença e percentual atingido.
Painel {
    id: janelinha
    property var metas: []
    property var vendedores: []
    property string erro: ""
    signal mudou()
    width: Math.min(760, Tema.largura - 24)
    height: Math.min(680, alturaMaxima)
    readonly property bool estreita: width < 640

    readonly property var indicadores: [
        { id: "vendas", nome: "Vendas líquidas (R$)" },
        { id: "lucro_bruto", nome: "Lucro bruto (R$)" },
        { id: "margem", nome: "Margem bruta (%)" },
        { id: "quantidade", nome: "Quantidade de vendas" }
    ]

    function carregar() {
        const r = Api.chamar("dre_metas")
        metas = r.ok ? r.dados : []
        if (!r.ok)
            erro = r.erro
    }
    function abrir() {
        const u = Api.chamar("usuarios_listar")
        vendedores = u.ok ? u.dados.filter(x => x.ativo) : []
        erro = ""
        valor.texto = ""
        carregar()
        open()
    }
    // depois(): só quando gravou (a senha do gerente pode ser pedida antes, D-09).
    function salvar(periodo, indicador, vendedorId, v, depois) {
        ApplicationWindow.window.chamarGerente("dre_meta_salvar", { periodo: periodo, indicador: indicador, vendedorId: vendedorId, valor: v }, r => {
            if (!r.ok) {
                erro = r.erro
                return
            }
            erro = ""
            metas = r.dados
            mudou()
            if (depois)
                depois()
        })
    }

    contentItem: ColumnLayout {
        spacing: 12
        Titulo {
            text: "Metas"
            font.pixelSize: janelinha.estreita ? 24 : 30
            Layout.fillWidth: true
        }
        Flickable {
            id: rolagemDasMetas
            objectName: "rolagemDasMetas"
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: width
            contentHeight: mioloMetas.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}

            ColumnLayout {
                id: mioloMetas
                width: rolagemDasMetas.width - (rolagemDasMetas.contentHeight > rolagemDasMetas.height ? 14 : 0)
                spacing: 12

                Nota {
                    text: "Mensal: do dia 1 até hoje. Semanal: de segunda até hoje. Vendas líquidas = vendas concluídas e não canceladas, já com desconto."
                    font.pixelSize: 14
                    Layout.fillWidth: true
                }
                ColumnLayout {
                    id: listaMetas
                    visible: janelinha.metas.length > 0
                    spacing: 8
                    Layout.fillWidth: true
                    Repeater {
                        model: janelinha.metas
                        delegate: Rectangle {
                            id: meta
                            required property var modelData
                            required property int index
                            Layout.fillWidth: true
                            implicitHeight: col.implicitHeight + 20
                            radius: Tema.raio
                            // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                            readonly property bool par: (index + 1) % 2 === 0 && !modelData.atingida
                            color: modelData.atingida ? Tema.verde : Tema.faixa(meta.par)
                            palette.window: Tema.superficie(color)
                            ColumnLayout {
                                id: col
                                anchors.fill: parent
                                anchors.margins: 10
                                anchors.leftMargin: 14
                                spacing: 4
                                RowLayout {
                                    spacing: 8
                                    Layout.fillWidth: true
                                    Text {
                                        text: meta.modelData.indicadorNome + " · " + meta.modelData.vendedor
                                        textFormat: Text.PlainText
                                        color: Tema.tintaDaFaixa(meta.par)
                                        font.family: Tema.texto
                                        font.weight: Font.DemiBold
                                        font.pixelSize: 15
                                        wrapMode: Text.WordWrap
                                        Layout.minimumWidth: 0
                                        Layout.fillWidth: true
                                    }
                                    Text {
                                        text: meta.modelData.percentual || "—"
                                        textFormat: Text.PlainText
                                        color: meta.modelData.atingida ? Tema.verdeEscuroDaFaixa(meta.par) : Tema.tintaDaFaixa(meta.par)
                                        font.family: Tema.titulo
                                        font.weight: Font.Bold
                                        font.pixelSize: janelinha.estreita ? 17 : 20
                                    }
                                    Botao {
                                        text: "Tirar"
                                        tipo: "discreto"
                                        altura: 30
                                        font.pixelSize: 13
                                        onClicked: janelinha.salvar(meta.modelData.periodo, meta.modelData.indicador, meta.modelData.vendedorId, "")
                                    }
                                }
                                Rectangle {
                                    Layout.fillWidth: true
                                    implicitHeight: 6
                                    radius: 3
                                    color: Tema.vela(0.35)
                                    Rectangle {
                                        width: parent.width * meta.modelData.parte
                                        height: parent.height
                                        radius: 3
                                        color: meta.modelData.atingida ? Tema.verdeEscuro : Tema.azulEscuro
                                    }
                                }
                                Text {
                                    text: meta.modelData.periodoRotulo + "  ·  realizado " + (Tema.oculto && meta.modelData.unidade === "R$" ? "•••" : meta.modelData.realizado)
                                          + " de " + (Tema.oculto && meta.modelData.unidade === "R$" ? "•••" : meta.modelData.meta)
                                          + (meta.modelData.diferenca && !Tema.oculto ? "  ·  diferença " + meta.modelData.diferenca : "")
                                    textFormat: Text.PlainText
                                    color: Tema.tintaSuaveDaFaixa(meta.par)
                                    font.family: Tema.serifa
                                    font.italic: true
                                    font.pixelSize: 13
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                            }
                        }
                    }
                }
                Nota {
                    visible: janelinha.metas.length === 0
                    text: "Nenhuma meta ainda."
                    font.pixelSize: 14
                    Layout.fillWidth: true
                }
                Rotulo {
                    text: "Nova meta (ou trocar o valor de uma que já existe)"
                    wrapMode: Text.WordWrap
                    elide: Text.ElideNone
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                }
                GridLayout {
                    columns: janelinha.estreita ? 1 : 4
                    columnSpacing: 10
                    rowSpacing: 10
                    Layout.fillWidth: true
                    Selecao {
                        id: periodo
                        rotulo: "Período"
                        modelo: ["Mensal", "Semanal"]
                        Layout.fillWidth: janelinha.estreita
                        Layout.preferredWidth: janelinha.estreita ? -1 : 130
                    }
                    Selecao {
                        id: indicador
                        rotulo: "Indicador"
                        modelo: janelinha.indicadores.map(x => x.nome)
                        Layout.fillWidth: true
                    }
                    Selecao {
                        id: quem
                        rotulo: "De quem"
                        modelo: ["Loja"].concat(janelinha.vendedores.map(v => v.nome))
                        Layout.fillWidth: janelinha.estreita
                        Layout.preferredWidth: janelinha.estreita ? -1 : 155
                    }
                    Campo {
                        id: valor
                        rotulo: "Meta"
                        numerico: true
                        maximo: 14
                        Layout.fillWidth: janelinha.estreita
                        Layout.preferredWidth: janelinha.estreita ? -1 : 120
                        onConfirmado: salvarBotao.clicked()
                    }
                }
                TextoErro { text: janelinha.erro; Layout.fillWidth: true }
            }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 10
            Item { Layout.fillWidth: true }
            Botao {
                id: salvarBotao
                text: "Salvar meta"
                icone: "check"
                onClicked: {
                    const vend = quem.indice > 0 ? janelinha.vendedores[quem.indice - 1].id : ""
                    janelinha.salvar(periodo.indice === 1 ? "semana" : "mes", janelinha.indicadores[indicador.indice].id, vend, valor.texto,
                                     () => valor.texto = "")
                }
            }
        }
    }
}
