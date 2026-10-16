import QtQuick
import QtQuick.Layouts
import rustinho

// Aviso do dia no Início: faixa arredondada, sem botão de fechar.
// Clicar abre a tela; arrastar para a esquerda ou para a direita esconde
// o aviso até o dia seguinte.
Item {
    id: raiz
    property var aviso: ({})
    signal abrir()
    signal esconder()

    readonly property bool temAtraso: (aviso.atraso || "") !== ""
    property bool saindo: false

    implicitHeight: saindo ? 0 : faixa.height
    clip: saindo
    visible: !saindo || height > 0.5
    Behavior on implicitHeight { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }

    Rectangle {
        id: faixa
        width: raiz.width
        height: Tema.estreito ? Math.max(58, empilhado.implicitHeight + 24) : 58
        radius: Tema.raio
        color: raiz.temAtraso ? Tema.rosa : Tema.verde
        opacity: 1 - Math.min(Math.abs(x) / (raiz.width * 0.6), 0.85)

        readonly property string detalhe: {
            const partes = []
            if (raiz.aviso.texto)
                partes.push(raiz.aviso.texto)
            if (raiz.aviso.atraso)
                partes.push("<font color='" + Tema.vermelho + "'>" + raiz.aviso.atraso + "</font>")
            return partes.join("  ·  ")
        }

        // Computador: tudo numa linha.
        RowLayout {
            visible: !Tema.estreito
            anchors.fill: parent
            anchors.leftMargin: 22
            anchors.rightMargin: 22
            spacing: 14

            Text {
                text: raiz.aviso.titulo || ""
                color: Tema.tinta
                font.family: Tema.titulo
                font.weight: Font.Bold
                font.pixelSize: 24
            }
            Text {
                textFormat: Text.StyledText
                text: faixa.detalhe
                color: Tema.tinta
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 17
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            Text {
                text: "arraste para o lado para esconder até amanhã"
                visible: sobre.hovered && !arrasto.active
                color: Tema.apagado
                font.family: Tema.serifa
                font.italic: true
                font.pixelSize: 13
            }
        }

        // Celular: título em cima, o resto embaixo, quebrando a linha.
        ColumnLayout {
            id: empilhado
            visible: Tema.estreito
            x: 18
            width: parent.width - 36
            anchors.verticalCenter: parent.verticalCenter
            spacing: 2
            Text {
                text: raiz.aviso.titulo || ""
                color: Tema.tinta
                font.family: Tema.titulo
                font.weight: Font.Bold
                font.pixelSize: 22
            }
            Text {
                textFormat: Text.StyledText
                text: faixa.detalhe
                color: Tema.tinta
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 15
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
        }

        DragHandler {
            id: arrasto
            target: faixa
            yAxis.enabled: false
            onActiveChanged: {
                if (active)
                    return
                if (Math.abs(faixa.x) > raiz.width * 0.2)
                    saida.start()
                else
                    volta.start()
            }
        }
        TapHandler { onTapped: raiz.abrir() }
        HoverHandler { id: sobre; cursorShape: arrasto.active ? Qt.ClosedHandCursor : Qt.PointingHandCursor }
    }

    NumberAnimation {
        id: volta
        target: faixa; property: "x"; to: 0
        duration: 160; easing.type: Easing.OutCubic
    }
    SequentialAnimation {
        id: saida
        NumberAnimation {
            target: faixa; property: "x"
            to: faixa.x > 0 ? raiz.width : -raiz.width
            duration: 160; easing.type: Easing.InCubic
        }
        ScriptAction { script: { raiz.saindo = true; raiz.esconder() } }
    }
}
