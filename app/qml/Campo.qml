import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Campo de texto com rótulo em cima e mensagem de erro embaixo.
// Se `sugestoes` tiver itens, mostra uma lista enquanto a pessoa digita.
ColumnLayout {
    id: campo
    property string rotulo: ""
    property alias texto: entrada.text
    property alias dica: entrada.placeholderText
    property alias entrada: entrada
    property string erro: ""
    property bool numerico: false
    property bool senha: false
    // Dado sensível (custo, lucro, CPF): fica em "•••" enquanto o olho estiver
    // fechado. Ao clicar no campo para mudar, aparece.
    property bool sensivel: false
    readonly property bool escondido: sensivel && Tema.oculto && !entrada.activeFocus
    property bool somenteLeitura: false
    property bool maiusculas: false
    property string prefixo: ""
    property string sufixo: ""
    property int maximo: 200
    property int tamanhoFonte: 16
    property real altura: 40
    property string icone: ""
    property string atalho: ""
    // Celular: botão de câmera no fim do campo, para ler código de barras.
    property bool camera: false
    readonly property bool comCamera: camera && Tema.celular
    // Botão com ícone no fim do campo (ex.: "baixo" para abrir uma lista).
    property string acao: ""
    readonly property bool comBotao: comCamera || acao !== ""
    property var sugestoes: []
    property var filtradas: []

    signal editado()
    signal confirmado()
    signal saiu()
    signal acionado()

    spacing: 5
    Layout.alignment: Qt.AlignTop
    // Por padrão não estica; quem usar decide (Layout.fillWidth: true).
    Layout.fillWidth: false

    // No celular, trocar de tela não abre o teclado sozinho: o cursor só
    // pula de campo se o teclado já estiver aberto (a pessoa está digitando).
    function podeFocar() {
        return !Tema.celular || Qt.inputMethod.visible
    }
    function focar() {
        if (podeFocar())
            focarSempre()
    }
    function focarSempre() {
        entrada.forceActiveFocus()
        entrada.selectAll()
    }
    function focarCursor() {
        if (podeFocar())
            entrada.forceActiveFocus()
    }

    function semAcento(s) {
        const de = "áàâãäéèêëíìîïóòôõöúùûüçñÁÀÂÃÄÉÈÊËÍÌÎÏÓÒÔÕÖÚÙÛÜÇÑ"
        const para = "aaaaaeeeeiiiiooooouuuucnAAAAAEEEEIIIIOOOOOUUUUCN"
        let r = ""
        for (let i = 0; i < s.length; i++) {
            const k = de.indexOf(s[i])
            r += k >= 0 ? para[k] : s[i]
        }
        return r.toLowerCase()
    }

    function filtrar() {
        const t = semAcento(entrada.text.trim())
        if (t === "" || !campo.sugestoes || campo.sugestoes.length === 0) {
            lista.close()
            return
        }
        let inicio = [], meio = []
        for (let i = 0; i < campo.sugestoes.length; i++) {
            const s = campo.sugestoes[i]
            const n = semAcento(s)
            if (n === t)
                continue
            if (n.startsWith(t))
                inicio.push(s)
            else if (n.indexOf(t) >= 0)
                meio.push(s)
            if (inicio.length >= 8)
                break
        }
        campo.filtradas = inicio.concat(meio).slice(0, 8)
        if (campo.filtradas.length > 0) {
            vista.currentIndex = -1
            lista.open()
        } else {
            lista.close()
        }
    }

    function escolher(valor) {
        entrada.text = valor
        lista.close()
        campo.erro = ""
        campo.editado()
    }

    Rotulo {
        text: campo.rotulo
        visible: text !== ""
        color: campo.erro !== "" ? Tema.vermelho : Tema.apagado
        Layout.fillWidth: true
        Layout.preferredWidth: 1
    }

    TextField {
        id: entrada
        Layout.fillWidth: true
        implicitWidth: 120
        implicitHeight: campo.altura
        font.family: Tema.texto
        font.pixelSize: campo.tamanhoFonte
        color: Tema.tinta
        placeholderTextColor: "#9A9086"
        selectionColor: Tema.azulEscuro
        selectedTextColor: "white"
        echoMode: campo.senha || campo.escondido ? TextInput.Password : TextInput.Normal
        passwordCharacter: "•"
        horizontalAlignment: campo.numerico ? TextInput.AlignRight : TextInput.AlignLeft
        verticalAlignment: TextInput.AlignVCenter
        readOnly: campo.somenteLeitura
        maximumLength: campo.maximo
        leftPadding: (campo.icone !== "" ? 48 : 12) + (campo.prefixo !== "" ? textoPrefixo.implicitWidth + 6 : 0)
        rightPadding: 12 + (campo.sufixo !== "" ? textoSufixo.implicitWidth + 6 : 0)
                      + (textoAtalho.visible ? textoAtalho.implicitWidth + 16 : 0)
                      + (campo.comBotao ? 44 : 0)
        selectByMouse: true
        // Celular: sem "texto previsto" (o teclado manda cada letra direto;
        // evita letras trocadas de lugar). Números quando o campo é de número.
        inputMethodHints: Qt.ImhNoPredictiveText
                          | (campo.numerico ? Qt.ImhPreferNumbers : Qt.ImhNone)
                          | (campo.maiusculas ? Qt.ImhPreferUppercase : Qt.ImhNone)

        background: Rectangle {
            radius: Tema.raioCampo
            color: campo.somenteLeitura ? Qt.rgba(1, 1, 1, 0.28) : Tema.papel
            border.width: campo.erro !== "" || entrada.activeFocus ? 2 : 0
            border.color: campo.erro !== "" ? Tema.vermelho : Tema.azulEscuro
        }

        Icone {
            visible: campo.icone !== ""
            nome: campo.icone
            tamanho: 22
            cor: Tema.apagado
            anchors.left: parent.left
            anchors.leftMargin: 15
            anchors.verticalCenter: parent.verticalCenter
        }
        Rectangle {
            visible: campo.comBotao
            width: 40
            height: 40
            radius: 20
            color: toqueCamera.pressed ? Tema.verde : "transparent"
            anchors.right: parent.right
            anchors.rightMargin: 6
            anchors.verticalCenter: parent.verticalCenter
            Icone {
                anchors.centerIn: parent
                nome: campo.comCamera ? "camera" : campo.acao
                tamanho: 24
                cor: Tema.tinta
            }
            // MouseArea (e não TapHandler): fica com o toque só para si, sem o
            // campo de texto embaixo roubar o toque para abrir o teclado.
            MouseArea {
                id: toqueCamera
                anchors.fill: parent
                anchors.margins: -6
                preventStealing: true
                cursorShape: Qt.PointingHandCursor
                onClicked: {
                    if (!campo.comCamera) {
                        campo.acionado()
                        return
                    }
                    ApplicationWindow.window.lerCodigo(codigo => {
                        entrada.text = codigo
                        campo.editado()
                        campo.confirmado()
                    })
                }
            }
        }
        Rotulo {
            id: textoAtalho
            visible: campo.atalho !== "" && !Tema.celular && !Tema.estreito
            text: campo.atalho
            anchors.right: parent.right
            anchors.rightMargin: 16
            anchors.verticalCenter: parent.verticalCenter
        }
        Text {
            id: textoPrefixo
            text: campo.prefixo
            visible: text !== ""
            anchors.left: parent.left
            anchors.leftMargin: campo.icone !== "" ? 48 : 12
            anchors.verticalCenter: parent.verticalCenter
            color: Tema.apagado
            font.family: Tema.texto
            font.pixelSize: campo.tamanhoFonte - 2
        }
        Text {
            id: textoSufixo
            text: campo.sufixo
            visible: text !== ""
            anchors.right: parent.right
            anchors.rightMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            color: Tema.apagado
            font.family: Tema.texto
            font.pixelSize: campo.tamanhoFonte - 2
        }

        onTextEdited: {
            if (campo.maiusculas && text !== text.toUpperCase()) {
                const posicao = cursorPosition
                text = text.toUpperCase()
                cursorPosition = posicao
            }
            campo.erro = ""
            campo.editado()
            if (campo.sugestoes && campo.sugestoes.length > 0)
                campo.filtrar()
        }
        onAccepted: {
            if (lista.visible && vista.currentIndex >= 0)
                campo.escolher(campo.filtradas[vista.currentIndex])
            else
                campo.confirmado()
        }
        onActiveFocusChanged: {
            if (activeFocus) {
                // Entrou pelo Tab: seleciona tudo, para digitar por cima.
                if (focusReason === Qt.TabFocusReason || focusReason === Qt.BacktabFocusReason)
                    selectAll()
            } else {
                lista.close()
                campo.saiu()
            }
        }
        Keys.onDownPressed: (evento) => {
            if (lista.visible) {
                vista.currentIndex = Math.min(vista.currentIndex + 1, vista.count - 1)
                evento.accepted = true
            } else {
                evento.accepted = false
            }
        }
        Keys.onUpPressed: (evento) => {
            if (lista.visible) {
                vista.currentIndex = Math.max(vista.currentIndex - 1, -1)
                evento.accepted = true
            } else {
                evento.accepted = false
            }
        }
        Keys.onEscapePressed: (evento) => {
            if (lista.visible) {
                lista.close()
                evento.accepted = true
            } else {
                evento.accepted = false
            }
        }
    }

    Text {
        text: campo.erro
        visible: text !== ""
        color: Tema.vermelho
        font.family: Tema.texto
        font.pixelSize: 14
        wrapMode: Text.WordWrap
        Layout.fillWidth: true
        Layout.preferredWidth: 1
    }

    Popup {
        id: lista
        y: entrada.y + entrada.height + 4
        x: entrada.x
        width: Math.max(entrada.width, 220)
        padding: 4
        closePolicy: Popup.CloseOnPressOutsideParent
        background: Rectangle {
            color: Tema.papel
            radius: 8
            border.width: 1
            border.color: Qt.rgba(0.16, 0.15, 0.13, 0.12)
        }
        contentItem: ListView {
            id: vista
            implicitHeight: contentHeight
            model: campo.filtradas
            currentIndex: -1
            boundsBehavior: Flickable.StopAtBounds
            delegate: ItemDelegate {
                required property var modelData
                required property int index
                width: vista.width
                height: 34
                text: modelData
                font.family: Tema.texto
                font.pixelSize: 15
                highlighted: vista.currentIndex === index
                background: Rectangle {
                    radius: 6
                    color: parent.highlighted ? Tema.azul : (parent.hovered ? Qt.rgba(0.16, 0.15, 0.13, 0.06) : "transparent")
                }
                onClicked: campo.escolher(modelData)
            }
        }
    }
}
