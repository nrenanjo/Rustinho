import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Window
import rustinho

// Janela principal: controla qual tela aparece (pilha de telas).
ApplicationWindow {
    id: janela
    width: 1280
    height: 760
    minimumWidth: 340
    minimumHeight: 480
    visible: true
    onWidthChanged: Tema.largura = width
    onHeightChanged: Tema.altura = height
    // O Qt acrescenta " — Rustinho" sozinho (nome do programa).
    title: estado.loja || ""
    color: Tema.fundo
    font.family: Tema.texto
    font.pixelSize: 15

    // Cores fixas: o Rustinho fica igual mesmo com o tema escuro do KDE.
    palette.window: Tema.fundo
    palette.windowText: Tema.tinta
    palette.base: Tema.papel
    palette.alternateBase: Tema.pergaminho
    palette.text: Tema.tinta
    palette.button: "#E6DAC2"
    palette.buttonText: Tema.tinta
    palette.brightText: Tema.claro
    palette.highlight: Tema.azulEscuro
    palette.highlightedText: "white"
    palette.placeholderText: "#9A9086"
    palette.toolTipBase: Tema.tinta
    palette.toolTipText: Tema.claro
    palette.light: Tema.papel
    palette.midlight: Tema.pergaminho
    palette.mid: "#B9AE9F"
    palette.dark: "#8C8074"
    palette.shadow: Tema.tinta
    palette.link: Tema.azulEscuro

    property var estado: ({})
    property string erroGeral: ""

    // Telas que já funcionam nesta etapa.
    readonly property var telas: ({
        "vender": ["TelaVender", {}],
        "orcamentos": ["TelaOrcamentos", {}],
        "caixa": ["TelaCaixa", {}],
        "trocas": ["TelaTrocas", {}],
        "oficina": ["TelaOrdens", {}],
        "receber": ["TelaReceber", {}],
        "financeiro": ["TelaFinanceiro", {}],
        "pecas": ["TelaPecas", {}],
        "clientes": ["TelaPessoas", { papel: "cliente" }],
        "fornecedores": ["TelaPessoas", { papel: "fornecedor" }],
        "compras": ["TelaCompras", {}],
        "estoque": ["TelaEstoque", {}],
        "fiscal": ["TelaFiscal", {}],
        "painel": ["TelaPainel", {}],
        "usuarios": ["TelaUsuarios", {}],
        "configuracoes": ["TelaConfiguracoes", {}]
    })

    function atualizarEstado() {
        const r = Api.chamar("estado")
        if (r.ok) {
            estado = r.dados
            erroGeral = ""
        } else {
            erroGeral = r.erro
        }
        return r.ok
    }

    function abrir(tela, propriedades) {
        pilha.push(Qt.resolvedUrl(tela + ".qml"), propriedades || {})
    }

    function voltar() {
        if (pilha.depth > 1)
            pilha.pop()
    }

    function inicio() {
        pilha.pop(null)
    }

    function trocarPara(tela, propriedades) {
        pilha.clear(StackView.Immediate)
        pilha.push(Qt.resolvedUrl(tela + ".qml"), propriedades || {}, StackView.Immediate)
    }

    // Depois de entrar (ou de configurar pela primeira vez).
    function entrou() {
        Tema.oculto = true
        atualizarEstado()
        trocarPara("TelaInicio")
        // Para testes: rustinho --abrir TelaPecas
        const args = Qt.application.arguments
        const i = args.indexOf("--abrir")
        if (i >= 0 && i + 1 < args.length) {
            const j = args.indexOf("--props")
            abrir(args[i + 1], j >= 0 && j + 1 < args.length ? JSON.parse(args[j + 1]) : {})
        }
    }

    function trocarUsuario() {
        Tema.oculto = true
        Api.chamar("sair")
        atualizarEstado()
        trocarPara("TelaEntrar")
    }

    function abrirFuncao(item) {
        if (!item.disponivel) {
            avisar(item.titulo + " chega na etapa " + item.etapa + ".", "info")
            return
        }
        Api.chamar("menu_usar", { id: item.id })
        const t = telas[item.id]
        if (t)
            abrir(t[0], t[1])
    }

    function avisar(texto, tipo) {
        aviso.mostrar(texto, tipo)
    }

    // Celular: a câmera lê um código de barras e devolve o texto.
    function lerCodigo(depois) {
        if (leitorCodigo.item)
            leitorCodigo.item.ler(depois)
        else
            avisar("A câmera não abriu neste aparelho.", "erro")
    }
    Loader {
        id: leitorCodigo
        active: Tema.celular
        source: Tema.celular ? Qt.resolvedUrl("../qml_android/LeitorCamera.qml") : ""
    }

    function confirmar(titulo, mensagem, textoBotao, funcao, tipo) {
        dialogo.perguntar(titulo, mensagem, textoBotao, funcao, tipo)
    }

    StackView {
        id: pilha
        anchors.fill: parent
        focus: true

        pushEnter: Transition {
            NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 140 }
        }
        pushExit: Transition {
            NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 100 }
        }
        popEnter: Transition {
            NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 140 }
        }
        popExit: Transition {
            NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 100 }
        }
        replaceEnter: Transition {
            NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 140 }
        }
        replaceExit: Transition {
            NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 100 }
        }

        // Esc volta (cada tela pode decidir antes, com a função aoEsc).
        Keys.onEscapePressed: (evento) => {
            const tela = pilha.currentItem
            if (tela && typeof tela.aoEsc === "function")
                tela.aoEsc()
            else
                janela.voltar()
            evento.accepted = true
        }
        // Android: o "voltar" do celular faz o mesmo que o Esc. Na primeira
        // tela, deixa o Android fechar o app (o backup é feito antes).
        Keys.onBackPressed: (evento) => {
            const tela = pilha.currentItem
            if (tela && typeof tela.aoEsc === "function")
                tela.aoEsc()
            else if (pilha.depth > 1)
                janela.voltar()
            else {
                evento.accepted = false
                return
            }
            evento.accepted = true
        }
    }

    // F11: ir para a busca de peças de qualquer lugar.
    Shortcut {
        sequence: "F11"
        enabled: !!janela.estado.usuario
        onActivated: {
            const tela = pilha.currentItem
            if (tela && typeof tela.focarBusca === "function")
                tela.focarBusca()
            else
                janela.abrir("TelaPecas")
        }
    }

    // Backup do dia: confere de tempos em tempos (o trabalho roda em
    // segundo plano, sem travar o caixa).
    Timer {
        interval: 60 * 1000
        running: true
        onTriggered: {
            Api.chamar("backup_automatico")
            interval = 5 * 60 * 1000
            restart()
        }
    }

    // Ao fechar: faz o backup (se o último tem mais de meia hora) e fecha.
    property bool podeFechar: false
    onClosing: (fechar) => {
        if (podeFechar || estado.primeiraVez)
            return
        fechar.accepted = false
        fechando.visible = true
        esperaFechar.start()
    }
    Timer {
        id: esperaFechar
        interval: 150
        onTriggered: {
            Api.chamar("backup_ao_fechar")
            janela.podeFechar = true
            janela.close()
        }
    }
    Rectangle {
        id: fechando
        visible: false
        anchors.fill: parent
        color: Tema.fundo
        opacity: 0.96
        z: 100
        MouseArea { anchors.fill: parent }
        Text {
            anchors.centerIn: parent
            text: "Guardando o backup antes de fechar…"
            color: Tema.tinta
            font.family: Tema.titulo
            font.weight: Font.Bold
            font.pixelSize: 30
        }
    }

    // F12: mostrar ou esconder CPF/CNPJ, custo, lucro e saldos.
    Shortcut {
        sequence: "F12"
        enabled: !!janela.estado.usuario
        onActivated: Tema.oculto = !Tema.oculto
    }

    Aviso { id: aviso }

    Dialogo { id: dialogo }

    // Se o banco de dados não abrir, mostra o motivo.
    Rectangle {
        anchors.fill: parent
        visible: janela.erroGeral !== ""
        color: Tema.fundo
        z: 2000
        Column {
            anchors.centerIn: parent
            width: Math.min(640, parent.width - 80)
            spacing: 16
            Titulo { text: "Não foi possível abrir o Rustinho"; width: parent.width; wrapMode: Text.WordWrap }
            Text {
                text: janela.erroGeral
                width: parent.width
                wrapMode: Text.WordWrap
                color: Tema.tinta
                font.family: Tema.serifa
                font.pixelSize: 16
            }
            Botao { text: "Tentar de novo"; onClicked: if (janela.atualizarEstado()) janela.iniciar() }
        }
    }

    function iniciar() {
        if (estado.primeiraVez)
            trocarPara("TelaBoasVindas")
        else
            trocarPara("TelaEntrar")
    }

    Component.onCompleted: {
        if (atualizarEstado())
            iniciar()
        janela.showMaximized()
    }
}
