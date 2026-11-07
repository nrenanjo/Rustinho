import QtQuick
import rustinho

// Pílula pequena dos temas (0.9.18): sol = tema claro, lua = tema escuro
// (AMOLED), planeta = Planeta Roxo; os três na identidade roxa. A escolha
// fica guardada só neste aparelho.
Rectangle {
    id: pilula
    // O círculo de cada tema (o desenho).
    readonly property real lado: Tema.celular ? 36 : 30
    // A15 (0.9.18): a célula de cada tema é a área de toque. No celular tem
    // 48 × 48 dp (Tema.toqueMinimo): os círculos continuam com 36 e ficam
    // mais afastados, e a pílula fica mais comprida (antes, 36 dp de toque).
    // No computador, célula = círculo (como antes).
    readonly property real celula: Math.max(lado, Tema.toqueMinimo)

    implicitWidth: celula * 3 + 6
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
            model: [{ nome: "sol", tema: "cinza", dica: "Tema claro" },
                    { nome: "lua", tema: "amoled", dica: "Tema escuro" },
                    { nome: "planeta", tema: "roxo", dica: "Planeta Roxo" }]
            delegate: Item {
                id: celula
                required property var modelData
                readonly property bool ativo: Tema.tema === modelData.tema
                width: pilula.celula
                height: pilula.celula
                Rectangle {
                    id: botao
                    anchors.centerIn: parent
                    width: pilula.lado
                    height: pilula.lado
                    radius: height / 2
                    color: celula.ativo ? Tema.seletorAtivo : sobre.hovered ? Tema.sombraNo(0.07, pilula.color) : "transparent"
                    Behavior on color { ColorAnimation { duration: 120 } }
                    Icone {
                        visible: celula.modelData.nome !== "planeta"
                        anchors.centerIn: parent
                        nome: celula.modelData.nome
                        tamanho: 17
                        espessura: 1.8
                        cor: celula.ativo ? Tema.claro : Tema.tintaSuaveNo(pilula.color)
                    }
                    // O planeta aprovado (Rustinho.png reduzido), não um desenho.
                    Image {
                        visible: celula.modelData.nome === "planeta"
                        anchors.centerIn: parent
                        width: pilula.lado - 6
                        height: width
                        source: "qrc:/qt/qml/rustinho/icone/planeta.png"
                        sourceSize.width: 96
                        sourceSize.height: 96
                        smooth: true
                        mipmap: true
                    }
                }
                HoverHandler { id: sobre; cursorShape: Qt.PointingHandCursor }
                TapHandler { onTapped: pilula.escolher(celula.modelData.tema) }
                Accessible.role: Accessible.Button
                Accessible.name: modelData.dica
                // Para os testes das telas (--textos-da-tela): o botão sem texto.
                readonly property string nomeAcessivel: modelData.dica
            }
        }
    }
}
