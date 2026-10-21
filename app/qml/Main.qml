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
    palette.button: Tema.botao
    palette.buttonText: Tema.tinta
    palette.brightText: Tema.claro
    palette.highlight: Tema.azulEscuro
    palette.highlightedText: Tema.claro
    palette.placeholderText: Tema.dica
    palette.toolTipBase: Tema.tinta
    palette.toolTipText: Tema.claro
    palette.light: Tema.papel
    palette.midlight: Tema.pergaminho
    palette.mid: Tema.escuro ? "#4A4239" : "#B9AE9F"
    palette.dark: Tema.escuro ? "#6A6156" : "#8C8074"
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
            Tema.escuro = !!r.dados.escuro
            erroGeral = ""
        } else {
            erroGeral = r.erro
        }
        return r.ok
    }

    function abrir(tela, propriedades) {
        // Telas do gerente: pede a senha dele se quem está usando é caixa.
        const acao = ({ TelaFinanceiro: "financeiro", TelaUsuarios: "usuarios" })[tela]
        const empurrar = () => pilha.push(Qt.resolvedUrl(tela + ".qml"), propriedades || {})
        if (acao)
            comGerente(acao, empurrar)
        else
            empurrar()
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
        // (No Android, os do teste vêm em "argumentosExtras", de main.cpp.)
        const args = Qt.application.arguments.concat(typeof argumentosExtras !== "undefined" ? argumentosExtras : [])
        const i = args.indexOf("--abrir")
        // Só nomes de tela de verdade; dados quebrados são ignorados (C-13).
        if (i >= 0 && i + 1 < args.length && /^Tela[A-Za-z]{1,40}$/.test(args[i + 1])) {
            const j = args.indexOf("--props")
            let props = {}
            try {
                if (j >= 0 && j + 1 < args.length)
                    props = JSON.parse(args[j + 1])
            } catch (e) {
                props = {}
            }
            abrir(args[i + 1], props !== null && typeof props === "object" && !Array.isArray(props) ? props : {})
        }
        if (args.indexOf("--camera") >= 0)
            Qt.callLater(() => lerCodigo(c => avisar("Código lido: " + c, "ok")))
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

    // Funções que a loja deixou só para o gerente (D-09). Se quem está
    // usando é caixa:
    // - comGerente(acao, depois): abrir a tela da ação para CONSULTAR. A
    //   senha vale só para ler aquela tela, até sair dela;
    // - chamarGerente(comando, dados, depois): uma GRAVAÇÃO. Tenta; se o
    //   Rustinho pedir o gerente, pede a senha presa a este comando com estes
    //   dados e repete. depois(r) recebe a resposta final (também se desistir).
    readonly property var nomesDasAcoes: ({
        cancelar_venda: "Cancelar venda", estornar: "Estornar", ajustar_estoque: "Ajustar estoque",
        backup: "Backup", configuracoes: "Configurações da loja", usuarios: "Usuários",
        financeiro: "Financeiro", resultados: "Resultados (lucro e DRE)", gerente: "Gerente",
        precos: "Mudar o preço de venda"
    })
    function podeGerente(acao, consulta) {
        const r = Api.chamar("permissao", { acao: acao, consulta: !!consulta })
        return r.ok && r.dados.pode
    }
    function comGerente(acao, depois) {
        if (podeGerente(acao, true)) {
            depois()
            return
        }
        pedirGerente.consultar(acao, nomesDasAcoes[acao] || "", depois)
    }
    function chamarGerente(comando, dados, depois) {
        const r = Api.chamar(comando, dados)
        const pedeSenha = !r.ok && (r.campo === "gerente" || r.campo === "precos")
                          && !(estado.usuario && estado.usuario.gerente)
        if (!pedeSenha) {
            if (depois)
                depois(r)
            return
        }
        pedirGerente.paraChamada(comando, dados, r.erro, () => {
            const r2 = Api.chamar(comando, dados)
            if (depois)
                depois(r2)
        }, () => {
            if (depois)
                depois(r)
        })
    }
    // Ao sair de uma tela do gerente aberta com a senha dele.
    function encerrarGerente() {
        Api.chamar("gerente_encerrar")
    }
    PedirGerente { id: pedirGerente }

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

    // Para os testes das telas (app/testes): rustinho --textos-da-tela.
    // Ctrl+Shift+F12 escreve na saída cada texto que está na tela e onde
    // (sem senhas: os campos de senha dão os pontinhos). Sem a opção, nada.
    readonly property bool textosDaTela: Qt.application.arguments.indexOf("--textos-da-tela") >= 0
    Shortcut {
        sequence: "Ctrl+Shift+F12"
        enabled: janela.textosDaTela
        context: Qt.ApplicationShortcut
        onActivated: janela.escreverTextos()
    }
    function escreverTextos() {
        const linhas = []
        function andar(item) {
            if (!item || !item.visible || item.opacity === 0)
                return
            const t = item.displayText !== undefined ? item.displayText : item.text
            if (typeof t === "string" && t.trim() !== "") {
                const p = item.mapToItem(null, item.width / 2, item.height / 2)
                linhas.push(Math.round(p.x) + "\t" + Math.round(p.y) + "\t" + t.replace(/\s+/g, " ").trim())
            }
            for (let i = 0; i < item.children.length; i++)
                andar(item.children[i])
        }
        // A raiz da janela: as telas e, por cima, as janelinhas (senha do gerente...).
        andar(janela.contentItem.parent || janela.contentItem)
        console.info("TEXTOS-DA-TELA-INICIO\n" + linhas.join("\n") + "\nTEXTOS-DA-TELA-FIM")
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

    // Diagnóstico do Android (PEND-18): só existe no celular; no computador
    // "diagnostico" não existe e nada acontece.
    function marcarDiagnostico(texto) {
        if (typeof diagnostico !== "undefined")
            diagnostico.marcar(texto)
    }
    Connections {
        target: pilha
        function onCurrentItemChanged() {
            if (pilha.currentItem && typeof diagnostico !== "undefined")
                diagnostico.telaPronta(String(pilha.currentItem).split("(")[0])
        }
    }
    onErroGeralChanged: if (erroGeral !== "") marcarDiagnostico("erro ao abrir: " + erroGeral)

    // D-07: usuário desativado (aqui ou em outro caixa) ou sessão perdida:
    // volta para a entrada uma vez só, com o motivo.
    property bool voltandoParaEntrada: false
    Connections {
        target: Api
        function onSessaoEncerrada(motivo) {
            if (janela.voltandoParaEntrada)
                return
            const atual = pilha.currentItem ? String(pilha.currentItem) : ""
            if (atual.startsWith("TelaEntrar") || atual.startsWith("TelaBoasVindas"))
                return
            janela.voltandoParaEntrada = true
            Qt.callLater(() => {
                Tema.oculto = true
                janela.atualizarEstado()
                janela.trocarPara("TelaEntrar")
                janela.avisar(motivo, "erro")
                janela.voltandoParaEntrada = false
            })
        }
    }

    Component.onCompleted: {
        marcarDiagnostico("Main.qml: início")
        if (atualizarEstado())
            iniciar()
        marcarDiagnostico("Main.qml: estado " + (erroGeral === "" ? "lido" : "COM ERRO"))
        janela.showMaximized()
    }
}
