import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Notificação do Início (0.9.15). Duas apresentações:
// - expandida: título, o problema e a consequência, o quadro de detalhes
//   (quando há) e o botão que leva ao lugar de resolver;
// - minimizada: uma linha com o essencial.
// No canto superior direito, sempre, o triângulo de alerta (PNG
// transparente: preto no modo claro, branco no escuro).
//
// Um clique no corpo alterna entre expandida e minimizada. Arrastar para a
// direita minimiza. Arrastar para a esquerda: a informativa some até a
// meia-noite; a de atenção só minimiza; a crítica não sai. Nada disso
// resolve o problema: a notificação some quando a condição real acaba.
Item {
    id: raiz
    property var aviso: ({})
    signal abrir()
    signal minimizar(bool minimizado)
    signal esconder()

    readonly property string categoria: aviso.categoria || "informativa"
    readonly property bool minimizado: aviso.minimizado === true
    readonly property bool temAcao: (aviso.acao || "") !== ""
    property bool saindo: false

    implicitHeight: saindo ? 0 : cartao.height
    clip: saindo
    visible: !saindo || height > 0.5
    Behavior on implicitHeight { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }

    readonly property color corFundo: categoria === "critica" ? Tema.critico
                                      : categoria === "atencao" ? Tema.atencao : Tema.informativo
    readonly property color corBorda: categoria === "critica" ? Tema.bordaCritico
                                      : categoria === "atencao" ? Tema.bordaAtencao : Tema.bordaInformativo

    // Linha da minimizada. A informativa mostra o título em negrito e o
    // atraso em vermelho, como antes; as outras, o resumo.
    function linha() {
        if (categoria !== "informativa")
            return aviso.resumo || ""
        const partes = []
        if (aviso.texto)
            partes.push(aviso.texto)
        if (aviso.atraso)
            partes.push("<font color='" + Tema.vermelho + "'>" + aviso.atraso + "</font>")
        return "<b>" + (aviso.titulo || "") + "</b>" + (partes.length ? "  ·  " + partes.join("  ·  ") : "")
    }

    Rectangle {
        id: cartao
        width: raiz.width
        height: conteudo.implicitHeight + (raiz.minimizado ? 22 : 34)
        radius: Tema.raio
        color: raiz.corFundo
        border.width: 1
        border.color: raiz.corBorda
        opacity: 1 - Math.min(Math.abs(x) / (raiz.width * 0.6), 0.75)
        Behavior on height { NumberAnimation { duration: 140; easing.type: Easing.OutCubic } }

        ColumnLayout {
            id: conteudo
            x: Tema.estreito ? 16 : 22
            width: parent.width - x - (Tema.estreito ? 40 : 48)
            anchors.verticalCenter: parent.verticalCenter
            spacing: 6

            // ---- Minimizada: uma linha ----
            Text {
                visible: raiz.minimizado
                text: raiz.linha()
                textFormat: raiz.categoria === "informativa" ? Text.StyledText : Text.PlainText
                color: Tema.tinta
                font.family: Tema.texto
                font.weight: raiz.categoria === "informativa" ? Font.Normal : Font.DemiBold
                font.pixelSize: Tema.estreito ? 14 : 15
                wrapMode: Tema.estreito ? Text.WordWrap : Text.NoWrap
                elide: Tema.estreito ? Text.ElideNone : Text.ElideRight
                Layout.fillWidth: true
            }

            // ---- Expandida ----
            RowLayout {
                visible: !raiz.minimizado
                spacing: 16
                Layout.fillWidth: true
                ColumnLayout {
                    spacing: 3
                    Layout.fillWidth: true
                    Text {
                        text: raiz.aviso.tituloLongo || raiz.aviso.titulo || ""
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.weight: Font.Bold
                        font.pixelSize: Tema.estreito ? 16 : 18
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                    Text {
                        visible: text !== ""
                        text: raiz.categoria === "informativa" ? raiz.linha().replace(/^<b>[^<]*<\/b>(\s*·\s*)?/, "") : (raiz.aviso.descricao || "")
                        textFormat: raiz.categoria === "informativa" ? Text.StyledText : Text.PlainText
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.pixelSize: Tema.estreito ? 14 : 15
                        lineHeight: 1.1
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                }
                Botao {
                    id: acaoLarga
                    visible: raiz.temAcao && !Tema.estreito
                    text: raiz.aviso.acao || ""
                    tipo: "principal"
                    Layout.alignment: Qt.AlignVCenter
                    onClicked: raiz.abrir()
                }
            }

            // Quadro de detalhes (só na expandida e só quando há).
            Rectangle {
                visible: !raiz.minimizado && (raiz.aviso.detalhes || []).length > 0
                radius: 7
                color: Tema.vela(0.45)
                border.width: 1
                border.color: Tema.sombra(0.09)
                implicitHeight: grade.implicitHeight + 16
                Layout.fillWidth: true
                Layout.topMargin: 4
                GridLayout {
                    id: grade
                    x: 12
                    y: 8
                    width: parent.width - 24
                    columns: 2
                    columnSpacing: 18
                    rowSpacing: 3
                    Repeater {
                        model: (raiz.aviso.detalhes || []).length * 2
                        delegate: Text {
                            required property int index
                            readonly property var linha: raiz.aviso.detalhes[Math.floor(index / 2)]
                            text: index % 2 === 0 ? linha.rotulo + ":" : linha.valor
                            textFormat: Text.PlainText
                            color: index % 2 === 0 ? Tema.tintaSuave : Tema.tinta
                            font.family: Tema.texto
                            font.pixelSize: 14
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: index % 2 === 1
                        }
                    }
                }
            }

            Botao {
                id: acaoEstreita
                visible: !raiz.minimizado && raiz.temAcao && Tema.estreito
                text: raiz.aviso.acao || ""
                tipo: "principal"
                Layout.topMargin: 4
                onClicked: raiz.abrir()
            }
        }

        // Triângulo de alerta: a arte original, sem fundo; 10% menor que na
        // referência (20 px → 18 px de largura), na expandida e na minimizada.
        Image {
            source: Tema.escuro ? "qrc:/qt/qml/rustinho/icone/alerta-branco.png" : "qrc:/qt/qml/rustinho/icone/alerta-preto.png"
            width: 18
            height: Math.round(18 * 1222 / 1287)
            sourceSize.width: 36
            fillMode: Image.PreserveAspectFit
            smooth: true
            mipmap: true
            anchors.top: parent.top
            anchors.right: parent.right
            anchors.topMargin: raiz.minimizado ? Math.round((cartao.height - height) / 2) : 14
            anchors.rightMargin: 14
        }

        DragHandler {
            id: arrasto
            target: cartao
            yAxis.enabled: false
            onActiveChanged: {
                if (active)
                    return
                const limite = raiz.width * 0.18
                if (cartao.x > limite) {
                    // Para a direita: minimiza (não resolve nada).
                    volta.start()
                    if (!raiz.minimizado)
                        raiz.minimizar(true)
                } else if (cartao.x < -limite && raiz.categoria === "informativa") {
                    saida.start()
                } else if (cartao.x < -limite && raiz.categoria === "atencao") {
                    // Atenção não se esconde: só minimiza.
                    volta.start()
                    if (!raiz.minimizado)
                        raiz.minimizar(true)
                } else {
                    volta.start()
                }
            }
        }
        // Um clique no corpo alterna expandida/minimizada (o botão de ação
        // fica com o próprio clique).
        TapHandler {
            onTapped: (ponto) => {
                // O clique no botão de ação é só dele: não alterna.
                for (const b of [acaoLarga, acaoEstreita]) {
                    if (b.visible && b.contains(b.mapFromItem(cartao, ponto.position.x, ponto.position.y)))
                        return
                }
                raiz.minimizar(!raiz.minimizado)
            }
        }
        HoverHandler { cursorShape: arrasto.active ? Qt.ClosedHandCursor : Qt.PointingHandCursor }
    }

    NumberAnimation {
        id: volta
        target: cartao; property: "x"; to: 0
        duration: 160; easing.type: Easing.OutCubic
    }
    SequentialAnimation {
        id: saida
        NumberAnimation {
            target: cartao; property: "x"
            to: -raiz.width
            duration: 160; easing.type: Easing.InCubic
        }
        ScriptAction { script: { raiz.saindo = true; raiz.esconder() } }
    }
}
