import QtQuick
import rustinho

// Mensagem rápida que aparece embaixo e some sozinha.
Rectangle {
    id: aviso
    property string tipo: "info"

    anchors.horizontalCenter: parent.horizontalCenter
    y: parent.height - height - 30
    z: 1000
    radius: 10
    color: tipo === "erro" ? Tema.vermelho : tipo === "ok" ? Tema.verdeEscuro : Tema.tinta
    width: Math.min(mensagem.implicitWidth + 48, parent.width - 80)
    height: mensagem.implicitHeight + 26
    opacity: 0
    visible: opacity > 0

    Text {
        id: mensagem
        anchors.centerIn: parent
        width: Math.min(implicitWidth, aviso.parent.width - 128)
        color: Tema.claro
        font.family: Tema.texto
        font.pixelSize: 15
        font.weight: Font.DemiBold
        wrapMode: Text.WordWrap
        horizontalAlignment: Text.AlignHCenter
    }

    Behavior on opacity { NumberAnimation { duration: 160 } }

    Timer {
        id: tempo
        onTriggered: aviso.opacity = 0
    }

    TapHandler { onTapped: aviso.opacity = 0 }

    function mostrar(texto, tipoAviso) {
        mensagem.text = texto
        aviso.tipo = tipoAviso || "info"
        aviso.opacity = 1
        tempo.interval = aviso.tipo === "erro" ? 5500 : 3000
        tempo.restart()
    }
}
