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
    height: Math.min(680, Tema.altura - 40)

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
        Titulo { text: "Metas"; font.pixelSize: 30; Layout.fillWidth: true }
        Nota {
            text: "Mensal: do dia 1 até hoje. Semanal: de segunda até hoje. Vendas líquidas = vendas concluídas e não canceladas, já com desconto."
            font.pixelSize: 14
            Layout.fillWidth: true
        }
        ListView {
            id: lista
            clip: true
            spacing: 8
            model: janelinha.metas
            Layout.fillWidth: true
            Layout.fillHeight: true
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                id: meta
                required property var modelData
                width: lista.width - 14
                implicitHeight: col.implicitHeight + 20
                radius: Tema.raio
                color: modelData.atingida ? Tema.verde : Tema.pergaminho
                ColumnLayout {
                    id: col
                    anchors.fill: parent
                    anchors.margins: 10
                    anchors.leftMargin: 16
                    spacing: 4
                    RowLayout {
                        spacing: 10
                        Layout.fillWidth: true
                        Text {
                            text: meta.modelData.indicadorNome + " · " + meta.modelData.vendedor
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.DemiBold
                            font.pixelSize: 15
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        Text {
                            text: meta.modelData.percentual || "—"
                            color: meta.modelData.atingida ? Tema.verdeEscuro : Tema.tinta
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: 20
                        }
                        Botao {
                            text: "Tirar"
                            tipo: "discreto"
                            implicitHeight: 30
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
                        color: Tema.tintaSuave
                        font.family: Tema.serifa
                        font.italic: true
                        font.pixelSize: 13
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                }
            }
        }
        Nota {
            visible: janelinha.metas.length === 0
            text: "Nenhuma meta ainda."
            font.pixelSize: 14
        }
        Rotulo { text: "Nova meta (ou trocar o valor de uma que já existe)" }
        Linha {
            spacing: 10
            Layout.fillWidth: true
            Selecao { id: periodo; rotulo: "Período"; modelo: ["Mensal", "Semanal"]; Layout.preferredWidth: 140 }
            Selecao { id: indicador; rotulo: "Indicador"; modelo: janelinha.indicadores.map(x => x.nome); Layout.fillWidth: true }
            Selecao {
                id: quem
                rotulo: "De quem"
                modelo: ["Loja"].concat(janelinha.vendedores.map(v => v.nome))
                Layout.preferredWidth: 170
            }
            Campo {
                id: valor
                rotulo: "Meta"
                numerico: true
                maximo: 14
                Layout.preferredWidth: 130
                onConfirmado: salvarBotao.clicked()
            }
        }
        TextoErro { text: janelinha.erro }
        RowLayout {
            Layout.alignment: Qt.AlignRight
            spacing: 10
            Botao { text: "Fechar"; tipo: "discreto"; onClicked: janelinha.close() }
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
