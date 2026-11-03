import QtQuick
import rustinho

// Pílula pequena do sol e da lua: sol = fundo cinza de sempre,
// lua = fundo preto (AMOLED). A escolha fica guardada só neste aparelho.
Rectangle {
    id: pilula
    readonly property real lado: Tema.celular ? 36 : 30

    implicitWidth: lado * 2 + 6
    implicitHeight: lado + 6
    radius: height / 2
    color: Tema.vela(0.45)
    border.width: 1
    border.color: Tema.sombra(0.1)

    function escolher(escuro) {
        if (Tema.escuro === escuro)
            return
        Tema.escuro = escuro
        Api.chamar("aparencia_salvar", { escuro: escuro })
    }

    Row {
        anchors.centerIn: parent
        spacing: 0
        Repeater {
            model: [{ nome: "sol", escuro: false, dica: "Fundo cinza" },
                    { nome: "lua", escuro: true, dica: "Fundo preto" }]
            delegate: Rectangle {
                id: botao
                required property var modelData
                readonly property bool ativo: Tema.escuro === modelData.escuro
                width: pilula.lado
                height: pilula.lado
                radius: height / 2
                color: ativo ? Tema.tinta : sobre.hovered ? Tema.sombra(0.07) : "transparent"
                Behavior on color { ColorAnimation { duration: 120 } }
                Icone {
                    anchors.centerIn: parent
                    nome: botao.modelData.nome
                    tamanho: 17
                    espessura: 1.8
                    cor: botao.ativo ? Tema.claro : Tema.tintaSuave
                }
                HoverHandler { id: sobre; cursorShape: Qt.PointingHandCursor }
                TapHandler { onTapped: pilula.escolher(botao.modelData.escuro) }
                Accessible.role: Accessible.Button
                Accessible.name: modelData.dica
            }
        }
    }
}
