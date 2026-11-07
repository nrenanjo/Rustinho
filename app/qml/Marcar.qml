import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Caixa de marcar.
CheckBox {
    id: caixa
    font.family: Tema.texto
    font.pixelSize: 15
    spacing: 10
    // A15 (0.9.18): no celular, a área de toque tem pelo menos 48 × 48 dp
    // (Tema.toqueMinimo); o quadradinho e o texto ficam no meio da altura.
    // No computador é a conta de sempre do CheckBox.
    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding, Tema.toqueMinimo)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitContentHeight + topPadding + bottomPadding,
                             implicitIndicatorHeight + topPadding + bottomPadding, Tema.toqueMinimo)
    // No celular, o texto comprido quebra a linha em vez de passar da tela.
    Layout.fillWidth: Tema.fino
    // A superfície embaixo (0.9.18; veja Tema.superficie).
    readonly property color superficie: Tema.roxo ? palette.window : Tema.fundo

    indicator: Rectangle {
        implicitWidth: 22
        implicitHeight: 22
        x: caixa.leftPadding
        y: (caixa.height - height) / 2
        radius: 5
        color: caixa.checked ? Tema.principal : Tema.papel
        border.width: caixa.visualFocus ? 2 : 0
        border.color: Tema.azulEscuroNo(caixa.superficie)
        Icone {
            anchors.centerIn: parent
            nome: "check"
            cor: Tema.sobrePrincipal
            tamanho: 16
            espessura: 2.2
            visible: caixa.checked
        }
    }

    contentItem: Text {
        leftPadding: caixa.indicator.width + caixa.spacing
        text: caixa.text
        font: caixa.font
        color: Tema.tintaNo(caixa.superficie)
        verticalAlignment: Text.AlignVCenter
        wrapMode: Text.WordWrap
    }

    HoverHandler { cursorShape: Qt.PointingHandCursor }
}
