import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Em que linha da DRE entra cada categoria. A de fábrica é só uma sugestão:
// o gerente confere com o contador e confirma (fica na auditoria).
Painel {
    id: janelinha
    property var c: null
    property var escolhas: ({})
    property string erro: ""
    signal salvo()
    width: Math.min(760, Tema.largura - 24)
    height: Math.min(680, alturaMaxima)
    readonly property bool estreita: width < 600

    function abrir() {
        const r = Api.chamar("dre_classificacao")
        if (!r.ok) {
            Qt.callLater(() => ApplicationWindow.window.avisar(r.erro, "erro"))
            return
        }
        c = r.dados
        const e = {}
        for (const x of c.categorias)
            e[x.id] = x.grupo
        escolhas = e
        confirmar.checked = false
        erro = ""
        open()
    }
    function nomesGrupos() {
        return ["Sem classificação"].concat(c ? c.grupos.map(g => g.nome) : [])
    }
    function indiceDe(grupo) {
        if (!c || !grupo)
            return 0
        const i = c.grupos.findIndex(g => g.id === grupo)
        return i < 0 ? 0 : i + 1
    }
    function salvar() {
        const lista = Object.keys(escolhas).map(id => ({ id: id, grupo: escolhas[id] }))
        ApplicationWindow.window.chamarGerente("dre_classificacao_salvar", { categorias: lista, confirmar: confirmar.checked }, r => {
            if (!r.ok) {
                erro = r.erro
                return
            }
            close()
            salvo()
        })
    }

    contentItem: ColumnLayout {
        spacing: 12
        Titulo {
            text: "Classificar categorias"
            font.pixelSize: janelinha.estreita ? 24 : 30
            Layout.fillWidth: true
        }
        Flickable {
            id: rolagemClassificar
            objectName: "rolagemClassificar"
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: width
            contentHeight: mioloClassificar.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}

            ColumnLayout {
                id: mioloClassificar
                width: rolagemClassificar.width - (rolagemClassificar.contentHeight > rolagemClassificar.height ? 14 : 0)
                spacing: 10

                Nota {
                    text: (janelinha.c ? janelinha.c.nota + " " : "") + "Categoria sem classificação conta como despesa e deixa o resultado incompleto."
                    font.pixelSize: 14
                    Layout.fillWidth: true
                }
                Nota {
                    visible: !!janelinha.c && janelinha.c.confirmada
                    text: "A classificação já foi confirmada. Mudanças ficam registradas na auditoria."
                    color: Tema.verdeEscuro
                    font.pixelSize: 14
                    Layout.fillWidth: true
                }
                ColumnLayout {
                    spacing: 6
                    Layout.fillWidth: true
                    Repeater {
                        model: janelinha.c ? janelinha.c.categorias : []
                        delegate: Rectangle {
                            id: linha
                            required property var modelData
                            required property int index
                            Layout.fillWidth: true
                            // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                            readonly property bool par: (index + 1) % 2 === 0
                            color: Tema.faixa(linha.par, Tema.vela(0.4), "transparent")
                            palette.window: Tema.superficie(color)
                            radius: Tema.raioCampo
                            implicitHeight: conteudoDaFaixa.implicitHeight + 14
                            GridLayout {
                                id: conteudoDaFaixa
                                x: 10
                                y: 7
                                width: linha.width - 20
                                columns: janelinha.estreita ? 1 : 2
                                columnSpacing: 12
                                rowSpacing: 6
                                ColumnLayout {
                                    spacing: 0
                                    Layout.fillWidth: true
                                    Layout.minimumWidth: 0
                                    Text {
                                        text: linha.modelData.nome
                                        textFormat: Text.PlainText
                                        color: linha.modelData.ativo ? Tema.tintaDaFaixa(linha.par) : Tema.apagadoDaFaixa(linha.par)
                                        font.family: Tema.texto
                                        font.weight: Font.DemiBold
                                        font.pixelSize: 15
                                        wrapMode: janelinha.estreita ? Text.WordWrap : Text.NoWrap
                                        elide: janelinha.estreita ? Text.ElideNone : Text.ElideRight
                                        Layout.fillWidth: true
                                        Layout.minimumWidth: 0
                                    }
                                    Text {
                                        text: (linha.modelData.tipo === "receita" ? "entrada" : "saída") + (linha.modelData.ativo ? "" : " · desativada")
                                        textFormat: Text.PlainText
                                        color: Tema.apagadoDaFaixa(linha.par)
                                        font.family: Tema.serifa
                                        font.italic: true
                                        font.pixelSize: 12
                                        Layout.fillWidth: true
                                    }
                                }
                                Selecao {
                                    modelo: janelinha.nomesGrupos()
                                    indice: janelinha.indiceDe(janelinha.escolhas[linha.modelData.id])
                                    Layout.fillWidth: janelinha.estreita
                                    Layout.preferredWidth: janelinha.estreita ? -1 : Math.min(300, Math.max(200, Math.round(linha.width * 0.48)))
                                    onEscolhido: {
                                        const e = Object.assign({}, janelinha.escolhas)
                                        e[linha.modelData.id] = indice === 0 ? "" : janelinha.c.grupos[indice - 1].id
                                        janelinha.escolhas = e
                                    }
                                }
                            }
                        }
                    }
                }
                Marcar {
                    id: confirmar
                    visible: !!janelinha.c && !janelinha.c.confirmada
                    text: "Conferi com o contador: confirmo a classificação"
                    Layout.fillWidth: true
                }
                TextoErro { text: janelinha.erro; Layout.fillWidth: true }
            }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 10
            Item { Layout.fillWidth: true }
            Botao { text: "Salvar"; icone: "check"; onClicked: janelinha.salvar() }
        }
    }
}
