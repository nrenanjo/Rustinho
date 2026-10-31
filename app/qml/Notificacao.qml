import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Notificação do Início (0.9.15; quadrado na 0.9.16). Duas apresentações:
// - expandida: faixa com o título, o problema e a consequência, o quadro de
//   detalhes (quando há) e o botão que leva ao lugar de resolver;
// - minimizada (só informativa e atenção): um quadrado com o ícone da função
//   (o mesmo da pílula) — a crítica fica sempre expandida.
// No canto superior direito, sempre, o triângulo de alerta (PNG
// transparente: preto no modo claro, branco no escuro).
//
// Um clique no corpo alterna entre expandida e minimizada (na crítica, não
// faz nada). Arrastar para a direita minimiza (não na crítica). Arrastar para
// a esquerda: a informativa some até a meia-noite; a de atenção só minimiza;
// a crítica não sai. Nada disso resolve o problema: a notificação some
// quando a condição real acaba.
Item {
    id: raiz
    property var aviso: ({})
    // A função da notificação (o ícone e a cor do quadrado).
    property string funcao: ""
    property string icone: ""
    signal abrir()
    signal minimizar(bool minimizado)
    signal esconder()

    readonly property string categoria: aviso.categoria || "informativa"
    readonly property bool critica: categoria === "critica"
    // A crítica não minimiza (nem com uma marca antiga de "minimizada").
    readonly property bool minimizado: aviso.minimizado === true && !critica
    // Lado do quadrado (minimizada).
    readonly property real lado: Tema.estreito ? 78 : 92
    // O quadrado não tem texto: nos textos da tela (testes) ele aparece pelo
    // título da notificação; a faixa já mostra o título escrito.
    readonly property string nomeAcessivel: minimizado ? (aviso.tituloLongo || aviso.titulo || "") : ""
    // Leitor de tela: sempre o título; na faixa, também o resumo. A crítica
    // não alterna com o clique: é um alerta, não um botão.
    Accessible.role: critica ? Accessible.AlertMessage : Accessible.Button
    Accessible.name: aviso.tituloLongo || aviso.titulo || ""
    Accessible.description: minimizado ? "" : (aviso.resumo || "")
    readonly property bool temAcao: (aviso.acao || "") !== ""
    property bool saindo: false

    implicitWidth: minimizado ? lado : 0
    implicitHeight: saindo ? 0 : cartao.height
    clip: saindo
    visible: !saindo || height > 0.5
    Behavior on implicitHeight { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }

    readonly property color corFundo: categoria === "critica" ? Tema.critico
                                      : categoria === "atencao" ? Tema.atencao : Tema.informativo
    readonly property color corBorda: categoria === "critica" ? Tema.bordaCritico
                                      : categoria === "atencao" ? Tema.bordaAtencao : Tema.bordaInformativo

    // Texto da informativa expandida: o que é e o atraso em vermelho.
    function linha() {
        const partes = []
        if (aviso.texto)
            partes.push(aviso.texto)
        if (aviso.atraso)
            partes.push("<font color='" + Tema.vermelho + "'>" + aviso.atraso + "</font>")
        return partes.join("  ·  ")
    }

    Rectangle {
        id: cartao
        width: raiz.width
        height: raiz.minimizado ? raiz.lado : conteudo.implicitHeight + 34
        radius: Tema.raio
        // O quadrado tem o fundo das pílulas; a faixa, o da categoria.
        color: raiz.minimizado ? Tema.cartao : raiz.corFundo
        border.width: 1
        border.color: raiz.minimizado ? Tema.bordaCartao : raiz.corBorda
        opacity: 1 - Math.min(Math.abs(x) / (raiz.width * 0.6), 0.75)
        Behavior on height { NumberAnimation { duration: 140; easing.type: Easing.OutCubic } }

        // ---- Minimizada: o ícone da função, no centro do quadrado ----
        IconeFuncao {
            visible: raiz.minimizado
            anchors.centerIn: parent
            funcao: raiz.funcao
            nome: raiz.icone
        }

        // ---- Expandida ----
        ColumnLayout {
            id: conteudo
            visible: !raiz.minimizado
            x: Tema.estreito ? 16 : 22
            width: parent.width - x - (Tema.estreito ? 40 : 48)
            anchors.verticalCenter: parent.verticalCenter
            spacing: 6

            RowLayout {
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
                        text: raiz.categoria === "informativa" ? raiz.linha() : (raiz.aviso.descricao || "")
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
                visible: (raiz.aviso.detalhes || []).length > 0
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
                visible: raiz.temAcao && Tema.estreito
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
            anchors.topMargin: raiz.minimizado ? 9 : 14
            anchors.rightMargin: raiz.minimizado ? 9 : 14
        }

        DragHandler {
            id: arrasto
            target: cartao
            yAxis.enabled: false
            onActiveChanged: {
                if (active)
                    return
                // No quadrado, o limite é pela largura da faixa de notificações
                // (não pelo próprio quadrado, pequeno demais para isso).
                const limite = Math.max(raiz.width, 400) * 0.18
                if (cartao.x > limite) {
                    // Para a direita: minimiza (não resolve nada); a crítica não.
                    volta.start()
                    if (!raiz.minimizado && !raiz.critica)
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
                // A crítica fica expandida.
                if (!raiz.critica)
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
