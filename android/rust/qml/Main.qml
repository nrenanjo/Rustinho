import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Window
import QtQuick.Layouts
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
    palette.mid: Tema.roxo ? "#C49BC0" : Tema.escuro ? "#4A3F4E" : "#C9B8CF"
    palette.dark: Tema.roxo ? "#E2BEDD" : Tema.escuro ? "#6E5F74" : "#8E7A95"
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
        // A conferência fiscal fica em Configurações › Fiscal (0.9.15).
        "fiscal": ["TelaConfiguracoes", { secao: "fiscal" }],
        "painel": ["TelaPainel", {}],
        "usuarios": ["TelaUsuarios", {}],
        "configuracoes": ["TelaConfiguracoes", {}]
    })

    function atualizarEstado() {
        const r = Api.chamar("estado")
        if (r.ok) {
            estado = r.dados
            // 0.9.18: "tema" (cinza, amoled, roxo); um núcleo de antes só manda "escuro".
            Tema.tema = r.dados.tema || (r.dados.escuro ? "amoled" : "cinza")
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

    // V-3 (0.9.18): a seta de retorno (Cabecalho) e o Esc (e o "voltar" do
    // Android) vêm por aqui, pelo mesmo caminho: a tela de cima decide com
    // aoEsc(daSeta), se tiver (pergunta antes de descartar, limpa a busca...);
    // senão, volta. daSeta = true quando veio da seta.
    function retornar(daSeta) {
        const tela = pilha.currentItem
        if (tela && typeof tela.aoEsc === "function")
            tela.aoEsc(daSeta === true)
        else
            voltar()
    }
    // D-V3, decidida pelo dono na correção definitiva do botão de retorno
    // (07/11/2026): o Esc faz exatamente o mesmo que a seta. Com a busca
    // preenchida, o Esc também volta direto (antes ele limpava a busca
    // primeiro). true = o Esc limpa a busca antes de voltar. As telas
    // perguntam por esta função; a regra fica só aqui.
    readonly property bool escLimpaBuscaAntes: false
    function limparBuscaAntes(daSeta) {
        return daSeta !== true && escLimpaBuscaAntes
    }

    // Troca a tela de cima por outra (subpílulas das pílulas unidas:
    // Vender / Receber e Estoque / Fornecedores). A seta continua voltando
    // para onde ia antes.
    function substituir(tela, propriedades) {
        pilha.replace(Qt.resolvedUrl(tela + ".qml"), propriedades || {}, StackView.Immediate)
    }

    function inicio() {
        largarGuia()
        pilha.pop(null)
    }

    function trocarPara(tela, propriedades) {
        largarGuia()
        fecharJanelinhas()
        pilha.clear(StackView.Immediate)
        pilha.push(Qt.resolvedUrl(tela + ".qml"), propriedades || {}, StackView.Immediate)
    }

    // Depois de entrar (ou de configurar pela primeira vez).
    function entrou() {
        Tema.oculto = true
        atualizarEstado()
        trocarPara("TelaInicio")
        // Para testes: rustinho --textos-da-tela --abrir TelaPecas.
        // (No Android, os do teste vêm em "argumentosExtras", de main.cpp.)
        const extras = typeof argumentosExtras !== "undefined" ? argumentosExtras : []
        const args = Qt.application.arguments.concat(extras)
        // A abertura direta é só para os testes. No computador, vale junto do
        // modo de captura (--textos-da-tela); no Android, pelo "extra" do
        // android/testar.sh, que main.cpp já filtra (só "--abrir Tela..." e
        // "--camera"). A entrada comum sempre permanece em Início.
        const i = (args.indexOf("--textos-da-tela") >= 0 || extras.indexOf("--abrir") >= 0)
                  ? args.indexOf("--abrir") : -1
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
        simultaneidade: "Simultaneidade",
        precos: "Mudar o preço de venda"
    })
    function podeGerente(acao, consulta) {
        const r = Api.chamar("permissao", { acao: acao, consulta: !!consulta })
        return r.ok && r.dados.pode
    }
    // desistiu (opcional): chamada se a pessoa fechar o pedido da senha sem
    // liberar (Voltar ou Esc). NOTIF-03 (0.9.18).
    function comGerente(acao, depois, desistiu) {
        if (podeGerente(acao, true)) {
            depois()
            return
        }
        pedirGerente.consultar(acao, nomesDasAcoes[acao] || "", depois)
        pedirGerente.desistiu = desistiu || null
    }
    // Várias consultas de uma vez (Configurações › Simultaneidade): pede a
    // senha só se faltar alguma.
    function comGerenteVarias(acoes, oQue, depois, desistiu) {
        const faltam = acoes.filter(a => !podeGerente(a, true))
        if (faltam.length === 0) {
            depois()
            return
        }
        pedirGerente.consultarVarias(faltam, oQue, depois)
        pedirGerente.desistiu = desistiu || null
    }
    // O mesmo, para uma função demorada (roda à parte, com a tela de espera).
    // `chave`: a chave da operação da tela (FALHA-04); a mesma vai na
    // tentativa e na repetição com a senha do gerente (a senha fica presa a
    // estes dados exatos, com a chave).
    function chamarGerenteDemorado(comando, dados, texto, depois, chave) {
        if (formaErradaGerente(comando, dados, depois))
            return
        dados = Api.comChave(dados, chave)
        Api.chamarDemorado(comando, dados, texto, r => {
            const pedeSenha = !r.ok && (r.campo === "gerente" || r.campo === "precos")
                              && !(estado.usuario && estado.usuario.gerente)
            if (!pedeSenha) {
                if (depois)
                    depois(r)
                return
            }
            pedirGerente.paraChamada(comando, dados, r.erro, () => {
                Api.chamarDemorado(comando, dados, texto, r2 => { if (depois) depois(r2) })
            }, () => {
                if (depois)
                    depois(r)
            })
        })
    }
    // H12 (dev.3.3, 2.1): a forma das chamadas com a senha do gerente é
    // (comando, dados, depois, chave). Uma função no lugar dos dados, ou um
    // "depois" que não é função, é erro do programa: antes o comando rodava e
    // a resposta se perdia em silêncio (o backup manual não mostrava nada).
    // Agora o comando não roda, e o erro vai para o registro e para a tela.
    function formaErradaGerente(comando, dados, depois) {
        if (typeof dados !== "function" && (depois === undefined || depois === null || typeof depois === "function"))
            return false
        console.error("Rustinho: chamada com a senha do gerente na forma errada: " + comando
                      + " (os dados vêm antes da função de resposta)")
        avisar("Erro interno do programa (" + comando + "): nada foi feito. Avise o suporte.", "erro")
        return true
    }
    function chamarGerente(comando, dados, depois, chave) {
        if (formaErradaGerente(comando, dados, depois))
            return
        dados = Api.comChave(dados, chave)
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

    // ---------------------------------------------------------------------
    // Navegação guiada (0.9.16). Quando uma operação não pode seguir porque
    // falta um cadastro ou uma configuração, o núcleo recusa com
    // campo = "requisito:<id>". guiar() leva ao lugar de resolver, pela mesma
    // pilha de telas; a tela de origem continua viva embaixo, com tudo o que
    // estava nela. A tela de destino avisa quando salvou (concluirGuia(true));
    // a volta só acontece quando o núcleo confirma que o requisito foi
    // satisfeito. Desistir (concluirGuia(false)) volta sem marcar nada.
    // Uma guia de cada vez; a operação original nunca é repetida sozinha.
    //
    // Cada requisito diz onde se resolve (a tela, a parte dela e os campos).
    readonly property var requisitos: ({
        "loja.cnpj": {
            tela: "TelaConfiguracoes", propriedades: { secao: "loja" }, campos: ["cnpj"],
            nome: "o CNPJ da loja",
            titulo: "Falta o CNPJ da loja",
            texto: "Preencha o CNPJ e salve (F2). Depois o Rustinho volta para %1 e você continua de onde parou."
        },
        "loja.uf_cnpj": {
            tela: "TelaConfiguracoes", propriedades: { secao: "loja" }, campos: ["uf_cnpj"],
            mesmaTela: true,
            nome: "o estado (UF) e o CNPJ da loja",
            titulo: "Para continuar, informe a UF e o CNPJ da loja",
            texto: "Escolha o estado, preencha o CNPJ e salve (F2). Depois o Rustinho volta para %1 e você continua de onde parou."
        },
        // Caso 6 (0.9.18): rascunho de OS novo bloqueado por um caixa sem
        // versão confirmada. Só a pedido (botão "Ver caixas" na saída da OS);
        // o gerente decide (atualizar o caixa ou retirá-lo da loja). O
        // rascunho não é salvo sozinho na volta.
        "caixas.rascunho": {
            tela: "TelaConfiguracoes", propriedades: { secao: "caixas" }, campos: ["caixa_bloqueando"],
            nome: "a confirmação de versão de todos os caixas",
            titulo: "Rascunho de OS bloqueado por um caixa desatualizado",
            texto: "Atualize o Rustinho no caixa destacado e deixe trocar dados, ou retire-o da loja (decisão do gerente). Quando nenhum caixa bloquear, o Rustinho volta para %1 e você salva o rascunho."
        }
    })
    property var guiaAtiva: null
    // Chamada pela tela de origem com a resposta recusada do núcleo. Devolve
    // true se a guia começou (ou já estava em andamento): a tela não precisa
    // fazer mais nada além de guardar o próprio estado.
    function guiar(campo, motivo, aoVoltar) {
        if (typeof campo !== "string" || campo.indexOf("requisito:") !== 0)
            return false
        const id = campo.slice("requisito:".length)
        const req = requisitos[id]
        if (!req)
            return false
        if (guiaAtiva)
            return true
        const origem = pilha.currentItem
        const mesmaTela = req.mesmaTela === true && origem
                          && typeof origem.iniciarGuiaNaMesmaTela === "function"
        const g = {
            id: id, requisito: req, origem: origem, motivo: motivo || "", aoVoltar: aoVoltar,
            mesmaTela: !!mesmaTela,
            // Para a orientação ("Depois o Rustinho volta para Compras"): toda
            // tela tem nomeNaNavegacao.
            origemNome: (origem && origem.nomeNaNavegacao) ? origem.nomeNaNavegacao : "a tela de antes"
        }
        guiaAtiva = g
        guia.mostrar("pendente", req.titulo, req.texto.arg(g.origemNome))
        if (mesmaTela)
            origem.iniciarGuiaNaMesmaTela(g)
        else
            abrir(req.tela, Object.assign({}, req.propriedades, { guia: g }))
        return true
    }
    // A tela de destino mostra qual campo destacar (e a rolagem dele).
    function guiaDestacar(item, rolagem) {
        if (guiaAtiva)
            guia.destacar(item, rolagem)
    }
    // O destino muda a orientação (vazio = a de sempre do requisito).
    function guiaOrientar(texto) {
        if (guiaAtiva)
            guia.mostrar("pendente", guiaAtiva.requisito.titulo,
                         texto || guiaAtiva.requisito.texto.arg(guiaAtiva.origemNome))
    }
    // O destino tentou salvar e não deu: continua lá, com o que foi digitado.
    function guiaFalhou(mensagem) {
        if (guiaAtiva)
            guia.mostrar("erro", "Não foi salvo", mensagem)
    }
    function finalizarRetornoGuia(g, satisfeito) {
        if (satisfeito)
            guia.mostrar("satisfeito", "Requisito satisfeito — conclua a operação.", "")
        else
            guia.mostrar("pendente", "A operação ainda não foi concluída",
                         "Falta " + g.requisito.nome + ". O que você já tinha feito continua lá.", true)
        // No Fiscal embutido, o retorno na mesma tela atualiza os dados sem
        // recarregar a lista de regras de ST que pode ter edições pendentes.
        if (!g.mesmaTela && g.aoVoltar)
            g.aoVoltar(satisfeito)
    }
    // O destino salvou (satisfeito = true) ou a pessoa desistiu (false).
    function concluirGuia(satisfeito) {
        const g = guiaAtiva
        if (!g)
            return false
        let requisitoConfirmado = null
        if (satisfeito) {
            const r = Api.chamar("requisito_conferir", { id: g.id })
            if (!r.ok || !r.dados.satisfeito) {
                guia.mostrar("erro", g.requisito.titulo, r.ok ? r.dados.falta : r.erro)
                return false
            }
            requisitoConfirmado = r.dados
        }
        guiaAtiva = null
        guia.semDestaque()
        guia.esconder()
        const naPilha = g.origem && pilha.find(item => item === g.origem) !== null
        if (!naPilha)
            return true
        if (g.mesmaTela) {
            depoisDaTroca(() => {
                g.origem.restaurarGuiaNaMesmaTela(requisitoConfirmado)
                finalizarRetornoGuia(g, satisfeito)
            })
            return true
        }
        pilha.pop(g.origem)
        // Só depois que a tela de origem voltou de vez (ela se atualiza ao
        // ficar ativa; a retomada vem em seguida, sem disputar com isso).
        depoisDaTroca(() => finalizarRetornoGuia(g, satisfeito))
        return true
    }
    // Roda f quando a pilha terminar a troca de telas em andamento.
    function depoisDaTroca(f) {
        if (!pilha.busy) {
            Qt.callLater(f)
            return
        }
        const quandoParar = () => {
            if (pilha.busy)
                return
            pilha.busyChanged.disconnect(quandoParar)
            Qt.callLater(f)
        }
        pilha.busyChanged.connect(quandoParar)
    }
    // As telas da guia vão sair da pilha (Início, tela de entrada, sessão
    // encerrada, backup restaurado): a guia acaba agora, antes da troca. Sem
    // isso, a faixa, o véu e o destaque apareciam por cima da tela seguinte
    // até a tela de destino ser destruída (0.9.18). O mesmo abandono que o
    // destino faz ao sair (que então não encontra mais nada a fazer).
    function largarGuia() {
        if (guiaAtiva)
            abandonarGuia(guiaAtiva)
    }
    // A pilha inteira vai ser trocada (sessão encerrada, trocar de usuário,
    // backup restaurado): a pergunta e o pedido da senha do gerente que
    // estiverem abertos eram da tela (e da sessão) que sai. Fecham sem fazer
    // nada — sem confirmar e sem o "desistiu" de uma tela que já não existe.
    // Antes, ficavam abertos por cima da tela de entrada (0.9.18, frente estab).
    function fecharJanelinhas() {
        dialogo.aoConfirmar = null
        dialogo.close()
        pedirGerente.desistiu = null
        pedirGerente.depois = null
        pedirGerente.close()
    }
    // A tela de destino saiu por outro caminho (Início, trocar de usuário):
    // a guia acaba sem voltar e sem marcar nada.
    function abandonarGuia(g) {
        if (!guiaAtiva || guiaAtiva !== g)
            return
        guiaAtiva = null
        guia.semDestaque()
        guia.esconder()
    }

    // Caso 8 da navegação contextual (0.9.18): a faixa do modo seguro leva a
    // Configurações › Simultaneidade › Backup. Só navega: escolher o backup,
    // confirmar e digitar a senha do gerente continuam lá, como sempre; nada
    // é restaurado nem apagado por aqui. Com Configurações já aberta (sem
    // guia em andamento), só troca a parte dela, sem empilhar outra.
    function irParaBackup() {
        const atual = pilha.currentItem
        if (atual && typeof atual.mostrarBackup === "function" && !atual.guia) {
            atual.mostrarBackup()
            return
        }
        abrir("TelaConfiguracoes", { secao: "backup" })
    }

    // A3 (0.9.15): banco com defeito de integridade — modo seguro. A faixa
    // fica sempre à vista, em todas as telas; o núcleo recusa toda gravação.
    Rectangle {
        id: faixaModoSeguro
        visible: !!janela.estado.modoSeguro
        anchors { left: parent.left; right: parent.right; top: parent.top }
        height: visible ? conteudoModoSeguro.implicitHeight + 16 : 0
        color: Tema.critico
        border.color: Tema.bordaCritico
        z: 1500
        GridLayout {
            id: conteudoModoSeguro
            anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter; margins: 12 }
            // No celular, o botão fica embaixo do texto.
            columns: Tema.estreito ? 1 : 2
            columnSpacing: 14
            rowSpacing: 8
            Text {
                id: textoModoSeguro
                text: "Modo seguro: o banco de dados deste caixa tem defeito e nada é gravado (vendas, recebimentos, estoque, caixa, troca de dados). Não continue a operar: restaure um backup bom em Configurações › Backup ou chame o suporte."
                wrapMode: Text.WordWrap
                color: Tema.vermelho
                font.family: Tema.serifa
                font.pixelSize: 15
                font.bold: true
                Layout.fillWidth: true
            }
            // Só com alguém dentro (na tela de entrada não há o que abrir).
            Botao {
                visible: !!janela.estado.usuario
                text: "Restaurar um backup"
                tipo: "secundario"
                // Não tira o teclado da tela (o Esc continua voltando).
                focusPolicy: Qt.NoFocus
                Layout.alignment: Qt.AlignVCenter
                onClicked: janela.irParaBackup()
            }
        }
    }

    StackView {
        id: pilha
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom; top: faixaModoSeguro.bottom }
        // A orientação da navegação guiada fica no alto, no espaço dela.
        anchors.topMargin: guia.alturaNoAlto
        Behavior on anchors.topMargin { NumberAnimation { duration: 140; easing.type: Easing.OutCubic } }
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

        // Esc volta, pelo mesmo caminho da seta (cada tela pode decidir
        // antes, com a função aoEsc).
        Keys.onEscapePressed: (evento) => {
            janela.retornar(false)
            evento.accepted = true
        }
        // Android: o "voltar" do celular faz o mesmo que o Esc. Na primeira
        // tela, deixa o Android fechar o app (o backup é feito antes).
        Keys.onBackPressed: (evento) => {
            const tela = pilha.currentItem
            if ((tela && typeof tela.aoEsc === "function") || pilha.depth > 1)
                janela.retornar(false)
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
    // A15 (0.9.18), também só com a opção dos testes: --celular desenha as
    // telas como no celular (Tema.celular, numa tela virtual) e --medir-toque
    // acrescenta a área de toque de cada controle (linhas TOQUE:) e o
    // retângulo de cada texto (linhas RET:).
    readonly property bool medirToque: textosDaTela && Qt.application.arguments.indexOf("--medir-toque") >= 0
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
            // Sem texto (o quadrado de uma notificação, a seta sozinha de uma
            // janelinha): o nome acessível.
            let t = item.displayText !== undefined ? item.displayText
                  : item.text !== undefined ? item.text : item.nomeAcessivel
            if ((typeof t !== "string" || t.trim() === "") && typeof item.nomeAcessivel === "string")
                t = item.nomeAcessivel
            if (typeof t === "string" && t.trim() !== "") {
                const p = item.mapToItem(null, item.width / 2, item.height / 2)
                linhas.push(Math.round(p.x) + "\t" + Math.round(p.y) + "\t" + t.replace(/\s+/g, " ").trim())
            }
            // Itens com nome (a seta de retorno, a busca da Início): onde estão
            // e o tamanho, para os testes conferirem o lugar (0.9.18).
            if (item.objectName !== "") {
                const q = item.mapToItem(null, 0, 0)
                linhas.push(Math.round(q.x + item.width / 2) + "\t" + Math.round(q.y + item.height / 2) + "\tGEO:"
                            + item.objectName + ":" + [q.x, q.y, item.width, item.height].map(Math.round).join(","))
            }
            if (janela.medirToque)
                medir(item, linhas)
            for (let i = 0; i < item.children.length; i++)
                andar(item.children[i])
        }
        // A raiz da janela: as telas e, por cima, as janelinhas (senha do
        // gerente, painéis, janela de arquivos). Sobe até o topo: no Qt 6.8 é o
        // pai do contentItem; em versões mais novas (6.12) há um nível a mais,
        // e sem isto as janelinhas ficavam fora da lista.
        let raiz = janela.contentItem
        while (raiz.parent)
            raiz = raiz.parent
        andar(raiz)
        // TESTE_PILHA é consumido pelo teste visual da Etapa 1 (caso F) para
        // confirmar que Fiscal → Loja reutiliza Configurações sem empilhar outra tela.
        linhas.push("0\t0\tTESTE_PILHA:" + pilha.depth)
        console.info("TEXTOS-DA-TELA-INICIO\n" + linhas.join("\n") + "\nTEXTOS-DA-TELA-FIM")
    }
    // A15 (--medir-toque): onde o dedo aciona cada controle. Botões, caixas
    // de marcar e listas (AbstractButton, ComboBox), campos de texto,
    // MouseArea e TapHandler (com a margem dele). Uma linha por controle:
    // TOQUE:tipo|componente|classe|nome|x,y,largura,altura|ativo, em pixels
    // lógicos da janela (no celular, dp). E RET: o retângulo de cada texto
    // (x,y,largura,altura,largura e altura do desenho,largura natural), para
    // conferir que nada fica por cima dele nem o espreme.
    function medir(item, linhas) {
        function classe(o) {
            return String(o).split("(")[0].replace(/(_QMLTYPE_\d+|_QML_\d+)+$/, "")
        }
        function componente(o) {
            for (let p = o; p; p = p.parent) {
                const c = classe(p)
                if (!c.startsWith("QQuick") && !c.startsWith("QObject"))
                    return c
            }
            return classe(o)
        }
        function nome(o, fundo) {
            let t = typeof o.text === "string" ? o.text : ""
            if (t.trim() === "" && typeof o.nomeAcessivel === "string")
                t = o.nomeAcessivel
            if (t.trim() === "" && typeof o.displayText === "string")
                t = o.displayText
            if (t.trim() === "" && fundo > 0)
                for (let i = 0; i < o.children.length && t.trim() === ""; i++)
                    if (o.children[i].visible)
                        t = nome(o.children[i], fundo - 1)
            return t.replace(/[\s|]+/g, " ").trim()
        }
        function linha(tipo, o, dono, margem) {
            if (o.width <= 0 || o.height <= 0)
                return
            const q = o.mapToItem(null, -margem, -margem)
            const w = o.width + 2 * margem, h = o.height + 2 * margem
            linhas.push(Math.round(q.x + w / 2) + "\t" + Math.round(q.y + h / 2) + "\tTOQUE:" + tipo + "|"
                        + componente(dono) + "|" + classe(dono) + "|" + (nome(dono, 4) || "-") + "|"
                        + [q.x, q.y, w, h].map(v => Math.round(v * 10) / 10).join(",") + "|" + (o.enabled ? 1 : 0))
        }
        let tipo = ""
        if (item.autoRepeat !== undefined && item.checkable !== undefined)
            tipo = "botao"
        else if (item.currentIndex !== undefined && item.popup !== undefined && item.displayText !== undefined)
            tipo = "lista"
        else if (item.pressAndHoldInterval !== undefined && item.acceptedButtons !== undefined)
            tipo = item.enabled ? "mousearea" : ""
        else if (item.cursorPosition !== undefined && item.readOnly === false)
            tipo = "campo"
        if (tipo !== "")
            linha(tipo, item, tipo === "mousearea" && item.parent ? item.parent : item, 0)
        const rs = item.resources
        for (let k = 0; rs && k < rs.length; k++) {
            const h = rs[k]
            if (h && h.gesturePolicy !== undefined && h.tapCount !== undefined && h.enabled)
                linha("tap", item, item, h.margin || 0)
        }
        if (item.contentWidth !== undefined && item.horizontalAlignment !== undefined && item.font !== undefined
                && item.cursorPosition === undefined && typeof item.text === "string" && item.text.trim() !== "") {
            const q = item.mapToItem(null, 0, 0)
            linhas.push(Math.round(q.x + item.width / 2) + "\t" + Math.round(q.y + item.height / 2) + "\tRET:"
                        + [q.x, q.y, item.width, item.height, item.contentWidth, item.contentHeight, item.implicitWidth].map(v => Math.round(v * 10) / 10).join(",")
                        + "|" + item.horizontalAlignment + "|" + componente(item) + "|" + item.text.replace(/[\s|]+/g, " ").trim())
        }
    }

    Aviso { id: aviso }

    Guia {
        id: guia
        topo: faixaModoSeguro.height
    }

    Dialogo { id: dialogo }

    // Espera de uma função demorada (0.9.15): cobre a janela na hora (nada
    // recebe clique, nem o segundo de um duplo clique) e mostra o que está
    // sendo feito com a barra andando. A operação roda à parte: a janela não
    // trava.
    Connections {
        target: Api
        function onEsperaComecou(texto) {
            espera.texto = texto
            espera.visible = true
        }
        function onEsperaTerminou() {
            espera.visible = false
        }
    }
    Rectangle {
        id: espera
        property string texto: ""
        property string detalhe: ""
        visible: false
        anchors.fill: parent
        z: 1800
        color: Tema.veu
        // Aparece aos poucos: uma operação rápida nem chega a piscar.
        opacity: visible ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 220; easing.type: Easing.InQuad } }
        // Segura o teclado enquanto espera e, no fim, devolve o foco a quem
        // tinha (uma janelinha aberta continua respondendo ao Esc).
        property Item focoAntes: null
        onVisibleChanged: {
            if (visible) {
                focoAntes = janela.activeFocusItem
                forceActiveFocus()
            } else if (focoAntes) {
                const anterior = focoAntes
                focoAntes = null
                if (anterior.visible)
                    anterior.forceActiveFocus()
            }
        }
        focus: visible
        Keys.onPressed: (evento) => evento.accepted = true
        MouseArea { anchors.fill: parent; hoverEnabled: true; acceptedButtons: Qt.AllButtons; onWheel: (r) => r.accepted = true }
        Timer {
            interval: 300
            repeat: true
            running: espera.visible
            onTriggered: espera.detalhe = Sistema.fazendo()
        }
        Rectangle {
            anchors.centerIn: parent
            width: Math.min(460, parent.width - 40)
            height: colunaEspera.implicitHeight + 48
            radius: Tema.raio + 3
            color: Tema.pergaminho
            palette.window: Tema.superficie(color)
            ColumnLayout {
                id: colunaEspera
                x: 24
                y: 24
                width: parent.width - 48
                spacing: 12
                Titulo {
                    text: espera.texto
                    font.pixelSize: 26
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
                Nota {
                    text: espera.detalhe !== "" ? espera.detalhe : "Não feche o Rustinho: isto pode levar alguns minutos."
                    font.pixelSize: 15
                    Layout.fillWidth: true
                }
                // Barra andando (o tempo que falta não é conhecido).
                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: 8
                    radius: 4
                    color: Tema.sombra(0.1)
                    clip: true
                    Rectangle {
                        id: faixaEspera
                        width: parent.width * 0.32
                        height: parent.height
                        radius: 4
                        color: Tema.azulEscuro
                        NumberAnimation on x {
                            running: espera.visible
                            loops: Animation.Infinite
                            from: -faixaEspera.width
                            to: faixaEspera.parent.width
                            duration: 1300
                        }
                    }
                }
            }
        }
    }

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
            // A atualização da estrutura falhou e o banco de antes voltou: só
            // tenta de novo quando alguém pede (0.9.15).
            Botao {
                visible: janela.erroGeral.indexOf("Tentar a atualização de novo") >= 0
                text: "Tentar a atualização de novo"
                tipo: "secundario"
                onClicked: {
                    if (Sistema.tentar_atualizacao_de_novo()) {
                        janela.erroGeral = ""
                        janela.abrirBanco()
                    }
                }
            }
        }
    }

    function iniciar() {
        // FALHA-01: no meio de uma entrada numa loja, volta para ela (para
        // continuar ou desistir); a loja nova só depois de desistir.
        if (estado.primeiraVez)
            trocarPara(estado.juntando ? "TelaJuntar" : "TelaBoasVindas")
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
    // No Android também vai para o registro do sistema (logcat): o teste do
    // emulador (android/testar.sh) reprova quando o Rustinho não abre.
    onErroGeralChanged: {
        if (erroGeral === "")
            return
        marcarDiagnostico("erro ao abrir: " + erroGeral)
        if (typeof diagnostico !== "undefined")
            console.warn("Rustinho: erro ao abrir: " + erroGeral)
    }

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
            // A guia acaba já: a tela que recebeu a recusa não mostra mais
            // "Não foi salvo" na faixa, que iria junto para a entrada.
            janela.largarGuia()
            Qt.callLater(() => {
                Tema.oculto = true
                janela.atualizarEstado()
                janela.trocarPara("TelaEntrar")
                janela.avisar(motivo, "erro")
                janela.voltandoParaEntrada = false
            })
        }
    }

    // Abre o banco à parte (0.9.15): um banco antigo é atualizado com cópia
    // e conferência, com a tela de espera, sem travar a janela.
    function abrirBanco() {
        espera.texto = "Abrindo o Rustinho…"
        espera.visible = true
        Sistema.abrir_em_segundo_plano()
    }
    Connections {
        target: Sistema
        function onAberto(erro) {
            espera.visible = false
            if (erro !== "") {
                janela.erroGeral = erro
                marcarDiagnostico("Main.qml: estado COM ERRO")
                return
            }
            if (janela.atualizarEstado())
                janela.iniciar()
            marcarDiagnostico("Main.qml: estado " + (janela.erroGeral === "" ? "lido" : "COM ERRO"))
        }
    }

    Component.onCompleted: {
        if (textosDaTela && Qt.application.arguments.indexOf("--celular") >= 0)
            Tema.simularCelular = true
        marcarDiagnostico("Main.qml: início")
        janela.showMaximized()
        abrirBanco()
    }
}
