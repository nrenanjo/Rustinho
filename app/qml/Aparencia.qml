import QtQuick
import rustinho

// Pílula pequena dos temas (0.9.18): sol = fundo cinza de sempre,
// lua = fundo preto (AMOLED), planeta = Planeta Roxo. A escolha fica
// guardada só neste aparelho.
Rectangle {
    id: pilula
    readonly property real lado: Tema.celular ? 36 : 30

    implicitWidth: lado * 3 + 6
    implicitHeight: lado + 6
    radius: height / 2
    color: Tema.seletorFundo
    border.width: 1
    border.color: Tema.sombra(0.1)

    function escolher(tema) {
        if (Tema.tema === tema)
            return
        Tema.tema = tema
        Api.chamar("aparencia_salvar", { tema: tema })
    }

    Row {
        anchors.centerIn: parent
        spacing: 0
        Repeater {
            model: [{ nome: "sol", tema: "cinza", dica: "Fundo cinza" },
                    { nome: "lua", tema: "amoled", dica: "Fundo preto" },
                    { nome: "planeta", tema: "roxo", dica: "Planeta Roxo" }]
            delegate: Rectangle {
                id: botao
                required property var modelData
                readonly property bool ativo: Tema.tema === modelData.tema
                width: pilula.lado
                height: pilula.lado
                radius: height / 2
                color: ativo ? Tema.seletorAtivo : sobre.hovered ? Tema.sombraNo(0.07, pilula.color) : "transparent"
                Behavior on color { ColorAnimation { duration: 120 } }
                Icone {
                    visible: botao.modelData.nome !== "planeta"
                    anchors.centerIn: parent
                    nome: botao.modelData.nome
                    tamanho: 17
                    espessura: 1.8
                    cor: botao.ativo ? Tema.claro : Tema.tintaSuaveNo(pilula.color)
                }
                // O planeta aprovado (Rustinho.png reduzido), não um desenho.
                Image {
                    visible: botao.modelData.nome === "planeta"
                    anchors.centerIn: parent
                    width: pilula.lado - 6
                    height: width
                    source: "qrc:/qt/qml/rustinho/icone/planeta.png"
                    sourceSize.width: 96
                    sourceSize.height: 96
                    smooth: true
                    mipmap: true
                }
                HoverHandler { id: sobre; cursorShape: Qt.PointingHandCursor }
                TapHandler { onTapped: pilula.escolher(botao.modelData.tema) }
                Accessible.role: Accessible.Button
                Accessible.name: modelData.dica
                // Para os testes das telas (--textos-da-tela): o botão sem texto.
                readonly property string nomeAcessivel: modelData.dica
            }
        }
    }
}
