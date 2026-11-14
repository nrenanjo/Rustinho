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
    // Busca da Início e barras de pesquisa gerais (0.9.18-dev.3.2): colar à
    // esquerda e copiar à direita, dentro do campo. Nas buscas com lupa, a lupa
    // permanece visível ao lado do botão colar.
    property bool colarCopiar: icone === "busca"
    readonly property real ladoBotaoInterno: Tema.celular ? 48 : 40
    // H01 (7.1): os controles da direita ficam lado a lado, cada um no seu
    // espaço, da borda para dentro: Copiar, a câmera (ou o botão de ação), o
    // atalho (F11) e o sufixo. Antes, todos se ancoravam no mesmo canto e o
    // Copiar ficava em cima do F11 (computador) e da câmera (celular).
    readonly property real larguraCopiar: colarCopiar ? ladoBotaoInterno + 8 : 0
    readonly property real larguraBotao: comBotao ? 44 : 0
    // 0.9.18: a superfície embaixo (o rótulo e o erro, fora do campo) e a
    // busca escura do Planeta Roxo (campo com a lupa).
    readonly property color superficie: Tema.roxo ? palette.window : Tema.fundo
    readonly property bool busca: icone === "busca" || colarCopiar
    readonly property bool buscaEscura: Tema.roxo && busca
    readonly property color corDoCampo: somenteLeitura ? Tema.campoFixo : buscaEscura ? Tema.buscaFundo : Tema.papel

    // ---- Os seis estados do campo (0.9.18, ordem visual §6) ----
    // normal, focado, preenchido, inválido, desabilitado e validado. Validado:
    // a tela diz (valido: true) ou o campo confere sozinho o que dá para
    // conferir sem o núcleo (conferir: "documento" = CPF ou CNPJ, "cpf",
    // "cnpj", "gtin" = código de barras), com as mesmas contas do núcleo
    // (base/src/documento.rs e gtin.rs, CNPJ alfanumérico incluído). Fica
    // verde só quando o número está completo e certo; o núcleo continua
    // conferindo ao salvar (o erro dele aparece como sempre).
    property bool valido: false
    property string conferir: ""
    readonly property bool validoAgora: valido || (conferir !== "" && confere(entrada.text))
    readonly property string estado: !enabled ? "desabilitado"
        : erro !== "" ? "invalido"
        : entrada.activeFocus ? "focado"
        : validoAgora ? "valido"
        : entrada.text !== "" ? "preenchido" : "normal"
    opacity: enabled ? 1 : Tema.opacidadeDesabilitado

    function _limpo(t) { return String(t).replace(/[^0-9A-Za-z]/g, "").toUpperCase() }
    function _todosIguais(s) { return s.split("").every(c => c === s[0]) }
    function cpfValido(t) {
        const s = _limpo(t)
        if (!/^[0-9]{11}$/.test(s) || _todosIguais(s))
            return false
        const d = s.split("").map(Number)
        const dv = n => {
            let soma = 0
            for (let i = 0; i < n; i++)
                soma += d[i] * (n + 1 - i)
            const resto = (soma * 10) % 11
            return resto === 10 ? 0 : resto
        }
        return dv(9) === d[9] && dv(10) === d[10]
    }
    function cnpjValido(t) {
        const s = _limpo(t)
        if (!/^[0-9A-Z]{12}[0-9]{2}$/.test(s) || _todosIguais(s))
            return false
        const v = c => c.charCodeAt(0) - 48
        const dv = n => {
            let soma = 0
            for (let i = 0; i < n; i++)
                soma += v(s[i]) * ((n - 1 - i) % 8 + 2)
            const resto = soma % 11
            return resto < 2 ? 0 : 11 - resto
        }
        return dv(12) === v(s[12]) && dv(13) === v(s[13])
    }
    function gtinValido(t) {
        const s = String(t).replace(/\s/g, "")
        if (!/^[0-9]+$/.test(s) || [8, 12, 13, 14].indexOf(s.length) < 0)
            return false
        let soma = 0
        for (let i = s.length - 2, k = 0; i >= 0; i--, k++)
            soma += Number(s[i]) * (k % 2 === 0 ? 3 : 1)
        return (10 - soma % 10) % 10 === Number(s[s.length - 1])
    }
    function confere(t) {
        switch (conferir) {
        case "documento": return cpfValido(t) || cnpjValido(t)
        case "cpf": return cpfValido(t)
        case "cnpj": return cnpjValido(t)
        case "gtin": return gtinValido(t)
        }
        return false
    }

    // Botão redondo dentro do campo (colar e copiar da busca da Início):
    // área de toque de 40 × 40 px (48 × 48 dp no celular).
    component BotaoInterno: Rectangle {
        id: botaoInterno
        property string nome: ""
        property string dica: ""
        property color fundo: Tema.papel
        property real lado: 40
        // Para leitores de tela e para os testes das telas (--textos-da-tela).
        readonly property string nomeAcessivel: dica
        signal acionado()
        width: lado
        height: lado
        radius: height / 2
        anchors.verticalCenter: parent.verticalCenter
        color: toque.pressed ? Tema.sombraNo(0.13, fundo) : toque.containsMouse ? Tema.sombraNo(0.07, fundo) : "transparent"
        Icone {
            anchors.centerIn: parent
            nome: botaoInterno.nome
            tamanho: 22
            cor: Tema.tintaSuaveNo(botaoInterno.fundo)
        }
        // MouseArea (e não TapHandler): fica com o toque só para si, sem o
        // campo de texto embaixo roubar o toque para abrir o teclado.
        MouseArea {
            id: toque
            anchors.fill: parent
            hoverEnabled: true
            preventStealing: true
            cursorShape: Qt.PointingHandCursor
            onClicked: botaoInterno.acionado()
        }
        ToolTip.visible: toque.containsMouse && !Tema.celular
        ToolTip.delay: 600
        ToolTip.text: dica
        Accessible.role: Accessible.Button
        Accessible.name: dica
    }

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

    // Os botões da busca da Início. Colar troca o texto da busca pelo que
    // foi copiado; copiar leva o texto da busca inteiro.
    function colar() {
        entrada.forceActiveFocus()
        entrada.selectAll()
        entrada.paste()
        campo.erro = ""
        campo.editado()
    }
    function copiar() {
        const janela = ApplicationWindow.window
        if (entrada.text === "") {
            if (janela)
                janela.avisar("A busca está vazia: nada para copiar.", "info")
            return
        }
        const de = entrada.selectionStart
        const ate = entrada.selectionEnd
        entrada.selectAll()
        entrada.copy()
        entrada.select(de, ate)
        if (janela)
            janela.avisar("Copiado: " + entrada.text, "ok")
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
        color: campo.erro !== "" ? Tema.vermelhoNo(campo.superficie) : Tema.apagadoNo(campo.superficie)
        Layout.fillWidth: true
        Layout.preferredWidth: 1
    }

    TextField {
        id: entrada
        // Para os testes das telas (--textos-da-tela): onde está a busca.
        objectName: (campo.colarCopiar && campo.icone !== "busca") ? "buscaDaInicio" : campo.busca ? "campoDeBusca" : ""
        Layout.fillWidth: true
        implicitWidth: 120
        // A15 (0.9.18): no celular, a área de toque do campo tem pelo menos
        // 48 dp de altura (Tema.toqueMinimo); o desenho do campo fica da
        // altura de sempre (campo.altura), no meio. No computador, igual.
        implicitHeight: Math.max(campo.altura, Tema.toqueMinimo)
        topInset: (implicitHeight - campo.altura) / 2
        bottomInset: topInset
        font.family: Tema.texto
        font.pixelSize: campo.tamanhoFonte
        color: Tema.tintaNo(campo.corDoCampo)
        placeholderTextColor: campo.buscaEscura ? Tema.roxoTintaSuaveClara : Tema.dica
        // O que fica dentro do campo (o atalho "F11") sabe a cor embaixo.
        palette.window: Tema.superficie(campo.corDoCampo)
        selectionColor: Tema.atual
        selectedTextColor: Tema.tintaAtual
        echoMode: campo.senha || campo.escondido ? TextInput.Password : TextInput.Normal
        passwordCharacter: "•"
        horizontalAlignment: campo.numerico ? TextInput.AlignRight : TextInput.AlignLeft
        verticalAlignment: TextInput.AlignVCenter
        readOnly: campo.somenteLeitura
        maximumLength: campo.maximo
        leftPadding: (campo.colarCopiar && campo.icone === "busca" ? campo.ladoBotaoInterno + 38
                      : campo.colarCopiar ? campo.ladoBotaoInterno + 10
                      : campo.icone !== "" ? 48 : 12)
                     + (campo.prefixo !== "" ? textoPrefixo.implicitWidth + 6 : 0)
        rightPadding: 12 + (campo.sufixo !== "" ? textoSufixo.implicitWidth + 6 : 0)
                      + (textoAtalho.visible ? textoAtalho.implicitWidth + 16 : 0)
                      + campo.larguraBotao
                      + campo.larguraCopiar
                      + (marcaValido.visible ? 26 : 0)
        selectByMouse: true
        // Celular: sem "texto previsto" (o teclado manda cada letra direto;
        // evita letras trocadas de lugar). Números quando o campo é de número.
        inputMethodHints: Qt.ImhNoPredictiveText
                          | (campo.numerico ? Qt.ImhPreferNumbers : Qt.ImhNone)
                          | (campo.maiusculas ? Qt.ImhPreferUppercase : Qt.ImhNone)

        background: Item {
            // O brilho em volta (foco magenta, erro vermelho), como na
            // referência aprovada do CNPJ: fora do campo, sem cobrir o texto.
            Rectangle {
                objectName: campo.objectName !== "" ? campo.objectName + "Brilho" : ""
                anchors.fill: parent
                anchors.margins: -4
                radius: Tema.raioCampo + 4
                color: "transparent"
                border.width: 4
                border.color: campo.estado === "invalido" ? Tema.erroBrilhoNo(campo.corDoCampo) : Tema.focoBrilho
                visible: campo.estado === "invalido" || campo.estado === "focado"
            }
            Rectangle {
                objectName: campo.objectName !== "" ? campo.objectName + "Caixa" : ""
                anchors.fill: parent
                radius: Tema.raioCampo
                color: campo.corDoCampo
                border.width: campo.estado === "normal" || campo.estado === "preenchido" || campo.estado === "desabilitado"
                              ? (campo.busca ? 2 : 1) : 2
                border.color: {
                    switch (campo.estado) {
                    case "invalido": return Tema.vermelhoNo(campo.corDoCampo)
                    case "focado": return campo.busca ? Tema.buscaBorda : Tema.foco
                    case "valido": return Tema.verdeEscuroNo(campo.corDoCampo)
                    }
                    if (campo.busca)
                        return Tema.buscaBorda
                    return campo.estado === "preenchido" ? Tema.bordaPreenchido : Tema.bordaCampo
                }
            }
        }
        // Validado: a marca verde no fim do campo (onde não tem outro botão).
        Icone {
            id: marcaValido
            objectName: campo.objectName !== "" ? campo.objectName + "Valido" : ""
            visible: campo.estado === "valido" && !campo.comBotao && campo.sufixo === "" && !textoAtalho.visible
                     && !campo.colarCopiar
            nome: "check"
            tamanho: 18
            cor: Tema.verdeEscuroNo(campo.corDoCampo)
            anchors.right: parent.right
            anchors.rightMargin: 12
            anchors.verticalCenter: parent.verticalCenter
        }

        Icone {
            visible: campo.icone !== ""
            nome: campo.icone
            tamanho: 22
            cor: campo.buscaEscura ? Tema.buscaBorda : Tema.apagado
            anchors.left: parent.left
            anchors.leftMargin: campo.colarCopiar ? campo.ladoBotaoInterno + 8 : 15
            anchors.verticalCenter: parent.verticalCenter
        }
        // Colar (à esquerda) e copiar (à direita), na busca da Início.
        BotaoInterno {
            visible: campo.colarCopiar
            nome: "colar"
            dica: "Colar na busca"
            fundo: campo.corDoCampo
            lado: campo.ladoBotaoInterno
            anchors.left: parent.left
            anchors.leftMargin: 6
            onAcionado: campo.colar()
        }
        BotaoInterno {
            visible: campo.colarCopiar
            nome: "copiar"
            dica: "Copiar a busca"
            fundo: campo.corDoCampo
            lado: campo.ladoBotaoInterno
            anchors.right: parent.right
            anchors.rightMargin: 6
            onAcionado: campo.copiar()
        }
        Rectangle {
            // Para os testes das telas acharem o botão (GEO:), quando o campo tem nome.
            objectName: campo.objectName !== "" ? campo.objectName + "Acao" : ""
            visible: campo.comBotao
            width: 40
            height: 40
            radius: 20
            color: toqueCamera.pressed ? Tema.verde : "transparent"
            anchors.right: parent.right
            anchors.rightMargin: 6 + campo.larguraCopiar
            anchors.verticalCenter: parent.verticalCenter
            Icone {
                anchors.centerIn: parent
                nome: campo.comCamera ? "camera" : campo.acao
                tamanho: 24
                cor: Tema.tintaNo(campo.corDoCampo)
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
            anchors.rightMargin: 16 + campo.larguraCopiar + campo.larguraBotao
            anchors.verticalCenter: parent.verticalCenter
        }
        Text {
            id: textoPrefixo
            text: campo.prefixo
            visible: text !== ""
            anchors.left: parent.left
            anchors.leftMargin: campo.colarCopiar ? campo.ladoBotaoInterno + 10 : campo.icone !== "" ? 48 : 12
            anchors.verticalCenter: parent.verticalCenter
            color: Tema.apagadoNo(campo.corDoCampo)
            font.family: Tema.texto
            font.pixelSize: campo.tamanhoFonte - 2
        }
        Text {
            id: textoSufixo
            text: campo.sufixo
            visible: text !== ""
            anchors.right: parent.right
            anchors.rightMargin: 12 + campo.larguraCopiar + campo.larguraBotao
                                 + (textoAtalho.visible ? textoAtalho.implicitWidth + 16 : 0)
            anchors.verticalCenter: parent.verticalCenter
            color: Tema.apagadoNo(campo.corDoCampo)
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
        color: Tema.vermelhoNo(campo.superficie)
        font.family: Tema.texto
        font.pixelSize: 14
        wrapMode: Text.WordWrap
        Layout.fillWidth: true
        Layout.preferredWidth: 1
    }

    Popup {
        id: lista
        // Logo abaixo do desenho do campo (no celular, a área de toque é maior).
        y: entrada.y + (entrada.height + campo.altura) / 2 + 4
        x: entrada.x
        width: Math.max(entrada.width, 220)
        padding: 4
        closePolicy: Popup.CloseOnPressOutsideParent
        background: Rectangle {
            color: Tema.papel
            radius: 8
            border.width: 1
            border.color: Tema.sombra(0.12)
        }
        contentItem: ListView {
            id: vista
            implicitHeight: contentHeight
            model: campo.filtradas
            currentIndex: -1
            boundsBehavior: Flickable.StopAtBounds
            delegate: ItemDelegate {
                id: sugestao
                required property var modelData
                required property int index
                width: vista.width
                // A15: no celular, cada sugestão com 48 dp de altura.
                height: Math.max(34, Tema.toqueMinimo)
                text: modelData
                font.family: Tema.texto
                font.pixelSize: 15
                highlighted: vista.currentIndex === index
                // H10 (extensão): a sugestão é um dado (uma marca, um local...).
                // O rótulo padrão do Qt lê como HTML o texto que parece HTML (e,
                // no Qt 6.12, um "<img …>" nele fecha o programa) e some com o
                // "&". Aqui é texto puro, com o desenho do padrão: as cores do
                // estilo, uma linha, reticências no fim.
                contentItem: Text {
                    text: sugestao.text
                    textFormat: Text.PlainText
                    font: sugestao.font
                    color: sugestao.highlighted ? sugestao.palette.highlightedText : sugestao.palette.text
                    elide: Text.ElideRight
                    verticalAlignment: Text.AlignVCenter
                }
                background: Rectangle {
                    radius: 6
                    color: parent.highlighted ? Tema.azul : (parent.hovered ? Tema.sombra(0.06) : "transparent")
                }
                onClicked: campo.escolher(modelData)
            }
        }
    }
}
