import QtQuick
import QtQuick.Controls.Basic
import rustinho

// Botão do Rustinho. Tipos: principal (tinta; no roxo, magenta), secundario
// (pergaminho; no roxo, rosa claro), discreto (sem fundo) e perigo (vermelho).
Button {
    id: botao
    property string tipo: "principal"
    property string atalho: ""
    property string icone: ""
    property real tamanhoIcone: 18
    // A altura do desenho do botão (o fundo). A15 (0.9.18): no celular, a
    // área de toque tem pelo menos 48 × 48 dp (Tema.toqueMinimo) e o desenho
    // fica do tamanho de sempre, no meio dela (as margens de dentro, insets).
    // No computador, a área de toque é o próprio desenho, como antes.
    property real altura: 42
    // A largura do desenho, quando quem usa fixa uma (-1 = a do conteúdo).
    property real largura: -1
    readonly property real larguraDesenho: largura >= 0 ? largura : implicitContentWidth + leftPadding + rightPadding

    implicitWidth: Math.max(larguraDesenho, Tema.toqueMinimo)
    implicitHeight: Math.max(altura, Tema.toqueMinimo)
    leftInset: (implicitWidth - larguraDesenho) / 2
    rightInset: leftInset
    topInset: (implicitHeight - altura) / 2
    bottomInset: topInset
    leftPadding: tipo === "discreto" ? 10 : 18
    rightPadding: tipo === "discreto" ? 12 : 18
    font.family: Tema.texto
    font.pixelSize: 15
    font.weight: Font.DemiBold
    focusPolicy: Qt.StrongFocus
    opacity: enabled ? 1 : 0.45

    readonly property bool escuro: tipo === "principal" || tipo === "perigo"
    // A superfície embaixo (0.9.18): no roxo, o discreto no fundo da tela tem
    // texto claro e, num painel, escuro. Nos outros temas não importa.
    readonly property color superficie: Tema.roxo ? palette.window : Tema.fundo
    readonly property color corTexto: tipo === "principal" ? Tema.sobrePrincipal
                                      : tipo === "perigo" ? Tema.claro
                                      : tipo === "secundario" ? Tema.tintaBotao
                                      : Tema.tintaNo(superficie)

    contentItem: Item {
        implicitWidth: linha.implicitWidth
        implicitHeight: linha.implicitHeight
        Row {
            id: linha
            anchors.centerIn: parent
            spacing: 8
            Icone {
                visible: botao.icone !== ""
                nome: botao.icone
                cor: botao.corTexto
                tamanho: botao.tamanhoIcone
                anchors.verticalCenter: parent.verticalCenter
            }
            Text {
                visible: botao.text !== ""
                text: botao.text
                textFormat: Text.PlainText
                font: botao.font
                color: botao.corTexto
                anchors.verticalCenter: parent.verticalCenter
            }
            Text {
                visible: botao.atalho !== "" && !Tema.celular && !Tema.estreito
                text: botao.atalho
                textFormat: Text.PlainText
                font.family: Tema.texto
                font.pixelSize: 12
                font.weight: Font.Bold
                color: botao.corTexto
                opacity: 0.6
                anchors.verticalCenter: parent.verticalCenter
            }
        }
    }

    background: Rectangle {
        radius: 8
        color: {
            const baixo = botao.down
            const sobre = botao.hovered
            switch (botao.tipo) {
            case "principal": return baixo ? Tema.principalBaixo : sobre ? Tema.principalSobre : Tema.principal
            case "perigo": return baixo ? Tema.perigoBaixo : sobre ? Tema.perigoSobre : Tema.vermelho
            case "secundario": return baixo ? Tema.botaoBaixo : sobre ? Tema.botaoSobre : Tema.botao
            default: return baixo ? Tema.sombraNo(0.13, botao.superficie) : sobre ? Tema.sombraNo(0.07, botao.superficie) : "transparent"
            }
        }
        border.width: botao.visualFocus ? 2 : 0
        border.color: botao.tipo === "discreto" ? Tema.azulEscuroNo(botao.superficie) : Tema.azulEscuro
    }

    // Enter também aperta o botão (no Qt, só a barra de espaço apertava).
    Keys.onReturnPressed: botao.clicked()
    Keys.onEnterPressed: botao.clicked()

    HoverHandler { cursorShape: Qt.PointingHandCursor }

    // Decisão 2c do proprietário (18/11/2026, bug YVZ4): o botão apertado diz à
    // janela onde estava e em que linha de lista (Main, registrarToqueNoBotao).
    // Se a lista mudar debaixo do ponteiro depois da operação gravada, os botões
    // não aceitam o clique por 0,4 s (descartandoCliqueNaLista): um 2º clique
    // no mesmo ponto não cai na linha que subiu. Só os botões: a rolagem por
    // gesto e o resto continuam.
    onPressed: {
        const j = ApplicationWindow.window
        if (j && typeof j.anotarToqueNoBotao === "function")
            j.anotarToqueNoBotao()
        if (j && typeof j.registrarToqueNoBotao === "function")
            j.registrarToqueNoBotao(botao)
    }
    // Ligada no bloqueio e também enquanto a janela ainda confere se a lista
    // mudou (numa máquina lenta, o toque pode chegar antes da conferência).
    MouseArea {
        anchors.fill: parent
        z: 1000
        readonly property var janela: ApplicationWindow.window
        enabled: !!janela && (janela.descartandoCliqueNaLista === true || janela.conferindoLinhaDoToque === true)
        acceptedButtons: Qt.AllButtons
        // No bloqueio, segura o clique (o pressionar e o soltar) e não faz nada
        // com ele. Com a conferência esperando, confere na hora: se a lista
        // mudou, segura; senão, o toque passa para o botão (accepted = false).
        onPressed: (mouse) => {
            // S2-R1 da conferência Sonnet: começa deixando passar; só segura
            // com o bloqueio ou com a lista mudada. Um erro não engole o toque.
            mouse.accepted = false
            try {
                mouse.accepted = janela.descartandoCliqueNaLista === true
                    || (typeof janela.conferirToqueAgora === "function" && janela.conferirToqueAgora() === true)
            } catch (e) {
                mouse.accepted = false
            }
            // Segurado: o toque também conta para a premissa dos testes (o que
            // passa é anotado no onPressed do botão).
            if (mouse.accepted && typeof janela.anotarToqueNoBotao === "function")
                janela.anotarToqueNoBotao()
        }
    }
}
