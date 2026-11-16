import QtQuick
import QtQuick.Layouts
import rustinho

// Cartão com um número grande (totais do topo das telas).
Rectangle {
    id: cartao
    property string rotulo: ""
    property string valor: ""
    property string prefixo: "R$ "
    property color cor: Tema.pergaminho
    property color corValor: Tema.tinta
    property string nota: ""
    property color corNota: Tema.apagado
    // A nota vai embaixo do número (para notas compridas).
    property bool notaEmbaixo: false
    // No celular (cartões estreitos), a nota sempre vai embaixo.
    readonly property bool _embaixo: notaEmbaixo || Tema.estreito
    // Valor escondido enquanto o olho estiver fechado.
    property bool sensivel: false

    radius: Tema.raio
    color: cor
    palette.window: Tema.superficie(color)
    // VIS33-08 (7.2): cresce se o rótulo quebrar em duas linhas no celular.
    // I8 (VIS-C8-02, 15/11/2026): quem quer o cartão mais alto dá a altura mínima (e não fixa o `implicitHeight`):
    // fixar impedia o cartão de crescer, e a nota passava da borda de baixo quando o rótulo quebrava.
    // B-2 da revisão do VIS2: no computador, em qualquer largura de janela, quem dá a altura (o Resumo: 96 e 86) a tem
    // FIXA, como antes do I8 (o conteúdo pede 90, e a 2ª fila do Resumo crescia 4 px); no celular, ela é só o mínimo,
    // para o cartão crescer quando o rótulo quebra (o I8).
    property real alturaDada: 0
    property real alturaMinima: alturaDada > 0 ? alturaDada : (Tema.estreito && nota !== "" && !notaEmbaixo ? 88 : 72)
    implicitHeight: !Tema.celular && alturaDada > 0 ? alturaDada : Math.max(alturaMinima, conteudoCartao.implicitHeight + 20)
    Layout.fillWidth: true

    ColumnLayout {
        id: conteudoCartao
        anchors.fill: parent
        anchors.leftMargin: Tema.estreito ? 14 : 18
        anchors.rightMargin: Tema.estreito ? 10 : 14
        anchors.topMargin: 10
        anchors.bottomMargin: 10
        spacing: 0
        RowLayout {
            spacing: 8
            Layout.fillWidth: true
            // VIS33-08 (7.2): o rótulo nunca alarga o cartão: ocupa o espaço que
            // sobra e, no celular, quebra em até duas linhas, com reticências.
            Rotulo {
                text: cartao.rotulo
                wrapMode: Tema.estreito ? Text.WordWrap : Text.NoWrap
                maximumLineCount: Tema.estreito ? 2 : 1
                Layout.fillWidth: true
                Layout.preferredWidth: 1
            }
            Text {
                visible: !cartao._embaixo
                text: cartao.nota
                textFormat: Text.PlainText
                color: cartao.corNota
                font.family: Tema.serifa
                font.italic: true
                font.pixelSize: 12
                elide: Text.ElideRight
                Layout.maximumWidth: cartao.width * 0.6
            }
        }
        Text {
            text: cartao.prefixo + (cartao.sensivel ? Tema.valor(cartao.valor) : cartao.valor)
            textFormat: Text.PlainText
            color: cartao.corValor
            font.family: Tema.titulo
            font.weight: Font.Bold
            font.pixelSize: Tema.estreito ? 24 : 28
            fontSizeMode: Text.HorizontalFit
            minimumPixelSize: 16
            Layout.fillWidth: true
        }
        Text {
            visible: cartao._embaixo && cartao.nota !== ""
            text: cartao.nota
            textFormat: Text.PlainText
            color: cartao.corNota
            font.family: Tema.serifa
            font.italic: true
            font.pixelSize: 13
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
    }
}
