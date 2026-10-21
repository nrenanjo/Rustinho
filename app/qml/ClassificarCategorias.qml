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
    height: Math.min(680, Tema.altura - 40)

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
        const r = Api.chamar("dre_classificacao_salvar", { categorias: lista, confirmar: confirmar.checked })
        if (!r.ok) {
            erro = r.erro
            return
        }
        close()
        salvo()
    }

    contentItem: ColumnLayout {
        spacing: 12
        Titulo { text: "Classificar categorias"; font.pixelSize: 30; Layout.fillWidth: true }
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
        ListView {
            id: lista
            clip: true
            spacing: 6
            model: janelinha.c ? janelinha.c.categorias : []
            Layout.fillWidth: true
            Layout.fillHeight: true
            ScrollBar.vertical: ScrollBar {}
            delegate: RowLayout {
                id: linha
                required property var modelData
                width: lista.width - 14
                spacing: 12
                ColumnLayout {
                    spacing: 0
                    Layout.fillWidth: true
                    Text {
                        text: linha.modelData.nome
                        color: linha.modelData.ativo ? Tema.tinta : Tema.apagado
                        font.family: Tema.texto
                        font.weight: Font.DemiBold
                        font.pixelSize: 15
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    Text {
                        text: (linha.modelData.tipo === "receita" ? "entrada" : "saída") + (linha.modelData.ativo ? "" : " · desativada")
                        color: Tema.apagado
                        font.family: Tema.serifa
                        font.italic: true
                        font.pixelSize: 12
                    }
                }
                Selecao {
                    modelo: janelinha.nomesGrupos()
                    indice: janelinha.indiceDe(janelinha.escolhas[linha.modelData.id])
                    Layout.preferredWidth: 300
                    onEscolhido: {
                        const e = Object.assign({}, janelinha.escolhas)
                        e[linha.modelData.id] = indice === 0 ? "" : janelinha.c.grupos[indice - 1].id
                        janelinha.escolhas = e
                    }
                }
            }
        }
        Marcar {
            id: confirmar
            visible: !!janelinha.c && !janelinha.c.confirmada
            text: "Conferi com o contador: confirmo a classificação"
        }
        TextoErro { text: janelinha.erro }
        RowLayout {
            Layout.alignment: Qt.AlignRight
            spacing: 10
            Botao { text: "Cancelar"; tipo: "discreto"; onClicked: janelinha.close() }
            Botao { text: "Salvar"; icone: "check"; onClicked: janelinha.salvar() }
        }
    }
}
