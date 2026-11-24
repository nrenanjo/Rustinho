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
    // QT-1b: a 2ª ação ("Conferi"), que resolve a notificação para quem conferiu.
    signal resolver()

    readonly property string categoria: aviso.categoria || "informativa"
    readonly property bool critica: categoria === "critica"
    readonly property bool persistente: aviso.persistente === true
    // A crítica (e a orientação persistente da navegação guiada, NCG-2) não minimizam.
    readonly property bool minimizado: aviso.minimizado === true && !critica && !persistente
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
    readonly property bool temResolver: (aviso.resolver || "") !== ""
    property bool saindo: false

    implicitWidth: minimizado ? lado : 0
    implicitHeight: saindo ? 0 : cartao.height
    clip: saindo
    visible: !saindo || height > 0.5
    Behavior on implicitHeight {
        enabled: !raiz.persistente
        NumberAnimation { duration: Tema.duracaoPosicao(160); easing.type: Easing.OutCubic }
    }

    // P-NOTIF (diretriz do proprietário, 18/11 e 20/11/2026): padrão visual FIXO
    // extraído de `fofznotifiz.png`, completamente independente dos três temas
    // (Claro, Escuro e Planeta Roxo), tanto expandido quanto minimizado.
    // O aviso rápido (`Aviso.qml`) permanece fora desta regra (verde/vermelho).
    readonly property color fixaFundoInicio: "#F6C2DC"
    readonly property color fixaFundoMeio: "#F8CDD7"
    readonly property color fixaFundoFim: "#FCDAC5"
    readonly property color fixaBorda: "#B43C8A"
    readonly property color fixaTitulo: "#1F1423"
    readonly property color fixaTexto: "#2C1E31"
    readonly property color fixaRotulo: "#5B3858"
    readonly property color fixaAtraso: "#9E1F46"
    readonly property color fixaQuadroFundo: Qt.rgba(1, 1, 1, 0.45)
    readonly property color fixaQuadroBorda: Qt.rgba(0.70, 0.24, 0.54, 0.24)
    readonly property color fixaBotaoPrincipal: "#D123AB"
    readonly property color fixaBotaoPrincipalSobre: "#C21E9E"
    readonly property color fixaBotaoPrincipalBaixo: "#A8198A"
    readonly property color fixaBotaoSecundario: "#FCE6F3"
    readonly property color fixaBotaoSecundarioSobre: "#F7D5EA"
    readonly property color fixaBotaoSecundarioBaixo: "#EFC2DF"
    readonly property color fixaBotaoSecundarioTexto: "#4A1142"

    // Botão próprio de notificação com cores fixas independentes do tema.
    component BotaoNotificacao: Button {
        id: btnNotif
        property bool principal: true
        implicitWidth: Math.max(implicitContentWidth + 32, Tema.toqueMinimo)
        implicitHeight: Math.max(40, Tema.toqueMinimo)
        topInset: (implicitHeight - 40) / 2
        bottomInset: topInset
        font.family: Tema.texto
        font.pixelSize: 15
        font.weight: Font.DemiBold
        focusPolicy: Qt.StrongFocus
        contentItem: Text {
            text: btnNotif.text
            textFormat: Text.PlainText
            font: btnNotif.font
            color: btnNotif.principal ? "#FFFFFF" : raiz.fixaBotaoSecundarioTexto
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
        background: Item {
            // Sombra discreta fixa sob o botão magenta (fofznotifiz.png).
            Rectangle {
                anchors.fill: fundoBtn
                anchors.topMargin: 2
                anchors.bottomMargin: -2
                radius: 8
                color: Qt.rgba(0.28, 0.04, 0.22, 0.18)
                visible: btnNotif.principal
            }
            Rectangle {
                id: fundoBtn
                anchors.fill: parent
                radius: 8
                color: btnNotif.principal
                       ? (btnNotif.down ? raiz.fixaBotaoPrincipalBaixo : btnNotif.hovered ? raiz.fixaBotaoPrincipalSobre : raiz.fixaBotaoPrincipal)
                       : (btnNotif.down ? raiz.fixaBotaoSecundarioBaixo : btnNotif.hovered ? raiz.fixaBotaoSecundarioSobre : raiz.fixaBotaoSecundario)
                border.width: btnNotif.visualFocus ? 2 : (btnNotif.principal ? 0 : 1)
                border.color: btnNotif.visualFocus ? "#6A0F5C" : raiz.fixaBorda
            }
        }
        Keys.onReturnPressed: btnNotif.clicked()
        Keys.onEnterPressed: btnNotif.clicked()
        HoverHandler { cursorShape: Qt.PointingHandCursor }
    }

    // Texto da informativa expandida: o que é e o atraso em vinho escuro fixo.
    function linha() {
        const partes = []
        // H10 (7.2): os dados vão escapados (o texto é formatado).
        if (aviso.texto)
            partes.push(Tema.escapar(aviso.texto))
        if (aviso.atraso)
            partes.push("<font color='#9E1F46'>" + Tema.escapar(aviso.atraso) + "</font>")
        return partes.join("  ·  ")
    }

    Rectangle {
        id: cartao
        objectName: "cartaoNotificacao"
        width: raiz.width
        height: raiz.minimizado ? raiz.lado : conteudo.implicitHeight + 34
        radius: 12
        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0.0; color: raiz.fixaFundoInicio }
            GradientStop { position: 0.52; color: raiz.fixaFundoMeio }
            GradientStop { position: 1.0; color: raiz.fixaFundoFim }
        }
        border.width: 1.5
        border.color: raiz.fixaBorda
        opacity: 1 - Math.min(Math.abs(x) / (raiz.width * 0.6), 0.75)
        Behavior on height {
            enabled: !raiz.persistente
            NumberAnimation { duration: Tema.duracaoPosicao(140); easing.type: Easing.OutCubic }
        }

        // ---- Minimizada: o ícone da função, no centro do quadrado ----
        IconeFuncao {
            visible: raiz.minimizado
            anchors.centerIn: parent
            funcao: raiz.funcao
            nome: raiz.icone
            claroFixo: true
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
                        textFormat: Text.PlainText
                        color: raiz.fixaTitulo
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
                        color: raiz.fixaTexto
                        font.family: Tema.texto
                        font.pixelSize: Tema.estreito ? 14 : 15
                        lineHeight: 1.1
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                }
                BotaoNotificacao {
                    id: acaoLarga
                    visible: raiz.temAcao && !Tema.estreito
                    text: raiz.aviso.acao || ""
                    principal: true
                    Layout.alignment: Qt.AlignVCenter
                    onClicked: raiz.abrir()
                }
                BotaoNotificacao {
                    id: resolverLarga
                    visible: raiz.temResolver && !Tema.estreito
                    text: raiz.aviso.resolver || ""
                    principal: false
                    Layout.alignment: Qt.AlignVCenter
                    onClicked: raiz.resolver()
                }
            }

            // Quadro de detalhes (só na expandida e só quando há).
            Rectangle {
                visible: (raiz.aviso.detalhes || []).length > 0
                radius: 7
                color: raiz.fixaQuadroFundo
                border.width: 1
                border.color: raiz.fixaQuadroBorda
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
                            color: index % 2 === 0 ? raiz.fixaRotulo : raiz.fixaTexto
                            font.family: Tema.texto
                            font.pixelSize: 14
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: index % 2 === 1
                        }
                    }
                }
            }

            RowLayout {
                visible: Tema.estreito && (raiz.temAcao || raiz.temResolver)
                spacing: 8
                Layout.topMargin: 4
                BotaoNotificacao {
                    id: acaoEstreita
                    visible: raiz.temAcao && Tema.estreito
                    text: raiz.aviso.acao || ""
                    principal: true
                    onClicked: raiz.abrir()
                }
                BotaoNotificacao {
                    id: resolverEstreita
                    visible: raiz.temResolver && Tema.estreito
                    text: raiz.aviso.resolver || ""
                    principal: false
                    onClicked: raiz.resolver()
                }
            }
        }

        // Triângulo de alerta: sempre escuro (sobre o fundo fixo rosa/pêssego),
        // independente do tema ativo (P-NOTIF).
        Image {
            source: "qrc:/qt/qml/rustinho/icone/alerta-preto.png"
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
            enabled: !raiz.persistente
            yAxis.enabled: false
            onActiveChanged: {
                if (active)
                    return
                if (raiz.persistente) {
                    volta.start()
                    return
                }
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
                // O clique num botão de ação é só dele: não alterna.
                for (const b of [acaoLarga, acaoEstreita, resolverLarga, resolverEstreita]) {
                    if (b.visible && b.contains(b.mapFromItem(cartao, ponto.position.x, ponto.position.y)))
                        return
                }
                // A crítica e a persistente ficam expandidas.
                if (!raiz.critica && !raiz.persistente)
                    raiz.minimizar(!raiz.minimizado)
            }
        }
        HoverHandler { cursorShape: raiz.persistente ? Qt.ArrowCursor : arrasto.active ? Qt.ClosedHandCursor : Qt.PointingHandCursor }
    }

    NumberAnimation {
        id: volta
        target: cartao; property: "x"; to: 0
        duration: Tema.duracaoPosicao(160); easing.type: Easing.OutCubic
    }
    SequentialAnimation {
        id: saida
        NumberAnimation {
            target: cartao; property: "x"
            to: -raiz.width
            duration: Tema.duracaoPosicao(160); easing.type: Easing.InCubic
        }
        ScriptAction { script: { raiz.saindo = true; raiz.esconder() } }
    }
}
