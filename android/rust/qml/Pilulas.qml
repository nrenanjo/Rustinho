import QtQuick
import QtQuick.Layouts
import rustinho

// Escolha entre poucas opções, em pílulas arredondadas (sem abas nem linhas).
// modelo: [{ id: "abertas", nome: "Em aberto" }, ...]
// Quando não cabem numa linha (celular), as pílulas passam para a de baixo.
Item {
    id: pilulas
    property var modelo: []
    property string atual: ""
    property bool pequenas: false
    signal escolhido(string id)
    // A superfície embaixo (0.9.18): no roxo, no fundo da tela a escolhida é
    // rosa com texto escuro; numa faixa (subpílulas, painel), roxo-escura com
    // texto branco. Nos outros temas, sempre a cor da tinta.
    readonly property color superficie: Tema.roxo ? palette.window : Tema.fundo
    readonly property bool noFundo: Tema.escura(superficie)

    // Dentro de um Layout, ocupa a largura toda (para poder quebrar a linha).
    Layout.fillWidth: true
    // Largura de tudo numa linha só (fora de um Layout).
    implicitWidth: _soma
    implicitHeight: fluxo.implicitHeight
    readonly property real _soma: {
        let t = 0
        for (let i = 0; i < repetidor.count; i++) {
            const c = repetidor.itemAt(i)
            if (c)
                t += c.width + (i > 0 ? fluxo.spacing : 0)
        }
        return t
    }

    Flow {
        id: fluxo
        width: pilulas.width
        spacing: 6

        Repeater {
            id: repetidor
            model: pilulas.modelo
            // A15 (0.9.18): cada pílula numa célula que é a área de toque. No
            // celular, a célula tem pelo menos 48 × 48 dp (Tema.toqueMinimo) e
            // a pílula fica do tamanho de sempre, no meio (só o espaço entre
            // as linhas de pílulas cresce). No computador, célula = pílula.
            delegate: Item {
                id: celula
                required property var modelData
                readonly property bool ativa: pilulas.atual === modelData.id
                readonly property real alturaDesenho: pilulas.pequenas ? 32 : (Tema.celular ? 42 : 40)
                readonly property real larguraDesenho: rotulo.implicitWidth + (pilulas.pequenas ? 26 : 36)
                height: Math.max(alturaDesenho, Tema.toqueMinimo)
                width: Math.max(larguraDesenho, Tema.toqueMinimo)
                Rectangle {
                    id: pilula
                    anchors.centerIn: parent
                    width: celula.larguraDesenho
                    height: celula.alturaDesenho
                    radius: height / 2
                    color: celula.ativa ? (pilulas.noFundo ? Tema.pilulaAtiva : Tema.subpilulaAtiva)
                                        : sobre.hovered ? Tema.sombraNo(0.07, pilulas.superficie) : "transparent"
                    Behavior on color { ColorAnimation { duration: 120 } }
                    Text {
                        id: rotulo
                        anchors.centerIn: parent
                        text: celula.modelData.nome
                        color: celula.ativa ? (pilulas.noFundo ? Tema.sobrePilulaAtiva : Tema.sobreSubpilulaAtiva)
                                            : Tema.tintaNo(pilulas.superficie)
                        font.family: Tema.texto
                        font.weight: Font.DemiBold
                        font.pixelSize: pilulas.pequenas ? 14 : 16
                    }
                }
                HoverHandler { id: sobre; cursorShape: Qt.PointingHandCursor }
                TapHandler {
                    onTapped: pilulas.escolhido(celula.modelData.id)
                }
            }
        }
    }
}
