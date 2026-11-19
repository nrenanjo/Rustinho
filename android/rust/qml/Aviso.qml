import QtQuick
import rustinho

// Mensagem rápida que aparece NO ALTO e some sozinha (regra do topo para as notificações globais: auditoria de
// 31/10, bloco 3, e diretriz de 15/11). Fica abaixo da faixa do modo seguro e da orientação da guia e, como a
// orientação, reserva o próprio espaço: enquanto aparece, a janela desce as telas por `alturaNoAlto`, então ela
// não cobre o cabeçalho, os campos nem os botões. As cores, a fonte, os tempos e o toque para fechar são os de antes.
Rectangle {
    id: aviso
    property string tipo: "info"
    // Onde começa: abaixo da faixa do modo seguro e da orientação da guia (Main.qml).
    property real topo: 0
    readonly property real espaco: 10
    // Aparecendo agora. O espaço reservado depende disto, e não da animação da opacidade (não oscila).
    property bool ativo: false
    readonly property real alturaNoAlto: ativo ? height + 2 * espaco : 0

    anchors.horizontalCenter: parent.horizontalCenter
    y: topo + espaco
    // Acima do véu do destaque da guia (Guia.qml, z 1400): o véu escurecia o aviso, e a regra do topo diz que nada
    // o cobre (15/11/2026). A orientação da guia fica na camada das janelinhas, acima dos dois.
    z: 1500
    radius: 10
    color: tipo === "erro" ? Tema.vermelho : tipo === "ok" ? Tema.verdeEscuro : Tema.tinta
    width: Math.min(mensagem.implicitWidth + 48, parent.width - 80)
    height: mensagem.implicitHeight + 26
    opacity: ativo ? 1 : 0
    visible: opacity > 0

    Text {
        id: mensagem
        textFormat: Text.PlainText
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

    // Só para os testes (INT9-R7, 19/11/2026): como o último aviso fechou, "toque" ou "tempo", dito na leitura dos
    // textos da tela (TESTE_AVISO_FECHADO, Main.escreverTextos). Numa máquina lenta, o teste do toque não consegue
    // separar pelo tempo o aviso fechado pelo toque do que fechou sozinho; o programa diz. O comportamento não muda.
    property string fechadoPor: ""

    Timer {
        id: tempo
        onTriggered: {
            if (!aviso.ativo)
                return
            aviso.fechadoPor = "tempo"
            aviso.ativo = false
        }
    }

    TapHandler {
        onTapped: {
            if (!aviso.ativo)
                return
            aviso.fechadoPor = "toque"
            aviso.ativo = false
        }
    }

    function mostrar(texto, tipoAviso) {
        aviso.fechadoPor = ""
        mensagem.text = texto
        aviso.tipo = tipoAviso || "info"
        aviso.ativo = true
        tempo.interval = aviso.tipo === "erro" ? 5500 : 3000
        tempo.restart()
    }
}
