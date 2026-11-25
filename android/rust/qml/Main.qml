import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Window
import QtQuick.Layouts
import rustinho

// Janela principal: controla qual tela aparece (pilha de telas).
ApplicationWindow {
    // T-02 (QT-1b / 0.9.19-dev.1): a confirmação de entrega ao núcleo só acontece depois que
    // o quadro que já sincronizou o estado pós-gravação (`afterSynchronizing`) é efetivamente
    // exibido (`frameSwapped`). Um `frameSwapped` de um quadro sincronizado antes da gravação
    // não confirma prematuramente; se uma gravação chegou depois, pede o próximo quadro.
    property bool _entregaSincronizadaNoQuadro: false
    onAfterSynchronizing: {
        if (Api._entregaPendente)
            _entregaSincronizadaNoQuadro = true
    }
    onFrameSwapped: {
        if (_entregaSincronizadaNoQuadro) {
            _entregaSincronizadaNoQuadro = false
            Api.confirmarEntrega()
        } else if (Api._entregaPendente) {
            janela.update()
        }
    }
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
            // 0.9.19-dev.1 (P6 / MG-8 / MG-D1): preferência de movimento reduzido (aparelho + OS).
            Tema.modoMovimento = r.dados.movimentoReduzido || "sistema"
            Tema.movimentoReduzidoSistema = Sistema.movimento_reduzido_sistema()
                || (textosDaTela && Qt.application.arguments.indexOf("--movimento-reduzido-sistema") >= 0)
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
        encerrarValidacaoProgressiva()
        pilhaSuspensaoF11 = []
        origemDaFaixa = null
        movimento.cancelar(true)
        guia.semDestaque(true)
        guia.esconder()
        pilha.pop(null)
    }

    function trocarPara(tela, propriedades) {
        largarGuia()
        encerrarValidacaoProgressiva()
        pilhaSuspensaoF11 = []
        origemDaFaixa = null
        movimento.cancelar(true)
        // BUG-20261118-SJ7F (casos 7j e 7k): a guia que acabou de terminar
        // deixa a faixa por alguns segundos ("A operação ainda não foi
        // concluída… O que você já tinha feito continua lá"), e o destaque leve
        // de uma notificação dura 6 s. A pilha inteira vai ser trocada (sessão
        // encerrada, trocar de usuário, backup restaurado): a tela de origem
        // sai, e nada disso vai junto para a tela seguinte.
        guia.semDestaque(true)
        guia.esconder()
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
        guardarRascunhosDaPilha()
        Tema.oculto = true
        Api.chamar("sair")
        atualizarEstado()
        trocarPara("TelaEntrar")
    }

    // REC-1 (ES-5 7a): antes de encerrar a sessão por troca de usuário ou fechamento
    // da janela, garante que qualquer tela com edição pendente na pilha persista o seu
    // retrato mais recente em `trabalho`.
    function guardarRascunhosDaPilha() {
        for (let i = 0; i < pilha.depth; i++) {
            const item = pilha.get(i, StackView.DontLoad)
            if (item && typeof item.guardarRascunhoAgora === "function") {
                try {
                    item.guardarRascunhoAgora()
                } catch (e) {
                    console.warn("Rustinho: falha ao guardar rascunho da tela: " + e)
                }
            }
        }
    }

    function temRascunhoAbertoNaPilha() {
        for (let i = 0; i < pilha.depth; i++) {
            const item = pilha.get(i, StackView.DontLoad)
            if (!item)
                continue
            if (item.alterado === true && item.travada !== true)
                return true
            if (Array.isArray(item.carrinho) && item.carrinho.length > 0)
                return true
            if (typeof item.temRascunhoSignificativo === "function" && item.temRascunhoSignificativo())
                return true
            if (typeof item.rascunhoId === "string" && item.rascunhoId !== "" && item.rascunho === true)
                return true
        }
        return false
    }

    // 7b / G9 / B1: ao sair com Esc/seta de uma tela aberta sobre trabalho de usuário desativado,
    // permite voltar mantendo o trabalho guardado no Início (para conferência sem obrigar descarte).
    function confirmarSaidaTrabalhoDesativado(descricao, aoVoltar) {
        confirmar(
            "Manter trabalho guardado e voltar?",
            "O trabalho" + (descricao ? " (“" + descricao + "”)" : "")
            + " deixado pelo usuário desativado continua na lista do Início para concluir depois ou usar “Descartar como gerente”.",
            "Manter e voltar",
            () => {
                if (aoVoltar)
                    aoVoltar()
            },
            "principal"
        )
    }

    // 7b / G9: descarte explícito pelo gerente de trabalho deixado por usuário desativado
    // (exige motivo obrigatório >= 3 letras e senha do gerente, auditando `trabalho.gerente_descartar`).
    function descartarTrabalhoDesativado(trabalhoId, descricao, depois) {
        perguntaDescartarDesativado.aoTerminar = depois || null
        perguntaDescartarDesativado.perguntar(
            "Descartar trabalho de usuário desativado?",
            "Este trabalho" + (descricao ? " (“" + descricao + "”)" : "")
            + " foi deixado por um usuário desativado. Informe o motivo do descarte para registro no histórico.",
            "Descartar trabalho",
            (m, pronto) => janela.chamarGerente("trabalho_gerente_descartar", { id: trabalhoId, motivo: m }, pronto),
            "Motivo do descarte",
            "perigo"
        )
    }

    // REC-1 / REC-2 (7b): abre a tela correspondente ao trabalho guardado, sempre como
    // rascunho (nunca conclui operação comercial nem grava cadastro sozinho).
    function abrirTelaDoTrabalho(aberto, trabalhoDesativadoId) {
        if (!aberto || !aberto.resumo)
            return
        const res = aberto.resumo
        if (res.jaConcluida) {
            confirmar(
                "Operação já concluída",
                "Esta operação já tinha sido confirmada e gravada antes da interrupção"
                + (res.resumoConcluida ? " (" + res.resumoConcluida + ")" : "")
                + ". O Rustinho não repete a gravação. Deseja retirar este item da lista de trabalhos guardados?",
                "Retirar da lista",
                () => {
                    if (trabalhoDesativadoId)
                        chamarGerente("trabalho_gerente_concluir", { id: res.id, motivo: "Operação já constava como concluída" }, () => {
                            if (pilha.currentItem && typeof pilha.currentItem.carregar === "function")
                                pilha.currentItem.carregar()
                        })
                    else {
                        Api.chamar("trabalho_concluir", { id: res.id })
                        if (pilha.currentItem && typeof pilha.currentItem.carregar === "function")
                            pilha.currentItem.carregar()
                    }
                },
                "principal"
            )
            return
        }
        if (res.situacao === "sem_conserto" || res.formatoIncompativel) {
            avisar(
                res.formatoIncompativel
                ? "Este rascunho foi salvo num formato incompatível com esta versão do Rustinho e continua preservado."
                : "Não foi possível ler os dados deste rascunho (o estado gravado está ilegível).",
                "erro"
            )
            return
        }
        const mapaTelas = {
            peca: ["TelaPeca", { produtoId: res.alvoId || "" }],
            pessoa: ["TelaPessoa", { pessoaId: res.alvoId || "" }],
            venda: ["TelaVender", {}],
            receber: ["TelaReceber", { clienteInicial: res.alvoId || "" }],
            os: ["TelaOrdem", { osId: res.alvoId || "" }],
            compra: ["TelaCompra", { compraId: res.alvoId || "" }]
        }
        let destino = mapaTelas[res.tipo]
        if (!destino && res.tipo === "guia" && aberto.estado && typeof aberto.estado.tela === "string"
                && /^Tela[A-Za-z]{1,40}$/.test(aberto.estado.tela)) {
            const propsGuia = aberto.estado.props && typeof aberto.estado.props === "object" ? aberto.estado.props : {}
            destino = [aberto.estado.tela, propsGuia]
        }
        if (!destino) {
            avisar("Tipo de trabalho não suportado nesta tela: " + res.tipo, "erro")
            return
        }
        const props = Object.assign({}, destino[1], {
            rascunhoRecuperado: aberto,
            trabalhoDesativadoId: trabalhoDesativadoId || ""
        })
        abrir(destino[0], props)
        if (res.usouVersaoAnterior)
            avisar("A última gravação estava incompleta; o Rustinho restaurou a versão anterior íntegra do rascunho.", "info")
        if (res.conflitoBase && res.motivoConflito)
            avisar(res.motivoConflito, "erro")
    }

    function retomarTrabalho(item) {
        const r = Api.chamar("trabalho_abrir", { id: item.id })
        if (!r.ok) {
            avisar(r.erro, "erro")
            return
        }
        if (!r.dados) {
            avisar("Este rascunho já não está mais disponível.", "info")
            return
        }
        abrirTelaDoTrabalho(r.dados, "")
    }

    function retomarTrabalhoDesativado(item, motivo, depois) {
        chamarGerente("trabalho_gerente_abrir", { id: item.id, motivo: motivo || "Conferência pelo gerente" }, r => {
            if (depois)
                depois(r)
            if (!r.ok) {
                avisar(r.erro, "erro")
                return
            }
            abrirTelaDoTrabalho(r.dados, item.id)
        })
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
        // I6: a repetição devolveu "ok" sem gravar de novo (a operação já estava feita).
        if (tipo === "ok" && Api.repetidaAgora) {
            Api.repetidaAgora = false
            texto = "Já estava feito. " + texto
        }
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

    // `enterConfirma` (R8): opcional; sem ele, a pergunta de perigo não confirma com o Enter.
    function confirmar(titulo, mensagem, textoBotao, funcao, tipo, enterConfirma) {
        dialogo.perguntar(titulo, mensagem, textoBotao, funcao, tipo, enterConfirma)
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
        },
        // Caso 7 (dev3.3): operação com dinheiro de um registro
        // comprovadamente inconsistente. O id traz o registro exato
        // ("caixas.referencia|<tabela>|<chave>"); a linha dele fica destacada.
        // Só a pedido ("Ver o registro"); a decisão é do gerente (aceitar como
        // está ou restaurar o backup do caixa de origem). Na volta, a operação
        // NÃO é refeita: a janelinha dela reabre como estava.
        "caixas.referencia": {
            tela: "TelaConfiguracoes", propriedades: { secao: "caixas" }, campos: ["referencia"],
            nome: "a decisão do gerente sobre o registro inconsistente",
            titulo: "Registro inconsistente: o dinheiro dele está bloqueado",
            texto: "O registro destacado precisa de uma decisão do gerente: aceitar como está ou restaurar o backup do caixa de origem. Depois o Rustinho volta para %1 e você conclui a operação."
        }
    })
    property var guiaAtiva: null
    // BUG-20261118-FXVT: tela dona da faixa de retorno da guia (quando ela sai da pilha, a faixa sai junto).
    property var origemDaFaixa: null
    // NCG-1 / NCG-2 (0.9.19-dev.1): validação progressiva guiada no mesmo formulário.
    // { tela, obterPendencias, satisfeito, ultimoCampo }
    property var validacaoAtiva: null
    // NCG-3 (0.9.19-dev.1): pilha de suspensões da navegação livre por F11 (sem banner "Você veio de...").
    property var pilhaSuspensaoF11: []
    property bool retornandoDeF11: false

    // Motor único de movimento (P6 / MG-1..MG-8 / MG-D4).
    readonly property bool movimentoAtivo: movimento.ativo
    function rolarSuave(area, destinoY, opcoes) { movimento.rolarSuave(area, destinoY, opcoes) }
    function rolarAteItem(area, item, folga, aoTerminar, aoAtualizar) {
        movimento.rolarAteItem(area, item, folga, aoTerminar, aoAtualizar)
    }
    function rolarAteIndice(lista, indice, modo, aoTerminar) {
        movimento.rolarAteIndice(lista, indice, modo, aoTerminar)
    }
    function cancelarMovimento() { movimento.cancelar(false) }

    // Chamada pela tela de origem com a resposta recusada do núcleo. Devolve
    // true se a guia começou (ou já estava em andamento): a tela não precisa
    // fazer mais nada além de guardar o próprio estado.
    // textoSatisfeito (B-2 da revisão): a mensagem da volta quando não há operação a concluir (a guia veio de
    // uma notificação do Início).
    function guiar(campo, motivo, aoVoltar, textoSatisfeito) {
        if (typeof campo !== "string" || campo.indexOf("requisito:") !== 0)
            return false
        const id = campo.slice("requisito:".length)
        // Um requisito pode trazer o registro exato depois de "|" (caso 7):
        // o mapa é pela primeira parte; o núcleo confere pelo id inteiro.
        const partes = id.split("|")
        const req = requisitos[partes[0]]
        if (!req)
            return false
        if (guiaAtiva)
            return true
        encerrarValidacaoProgressiva()
        origemDaFaixa = null
        const origem = pilha.currentItem
        const mesmaTela = req.mesmaTela === true && origem
                          && typeof origem.iniciarGuiaNaMesmaTela === "function"
        const g = {
            id: id, requisito: req, origem: origem, motivo: motivo || "", aoVoltar: aoVoltar,
            textoSatisfeito: textoSatisfeito || "",
            alvo: partes.length > 2 ? { tabela: partes[1], chave: partes.slice(2).join("|") } : null,
            mesmaTela: !!mesmaTela,
            // Para a orientação ("Depois o Rustinho volta para Compras"): toda
            // tela tem nomeNaNavegacao.
            origemNome: (origem && origem.nomeNaNavegacao) ? origem.nomeNaNavegacao : "a tela de antes"
        }
        guiaAtiva = g
        // NCG-2: enquanto a pendência existir, a orientação permanece visível (persistente).
        guia.mostrar("pendente", req.titulo, req.texto.arg(g.origemNome), false, true)
        if (mesmaTela)
            origem.iniciarGuiaNaMesmaTela(g)
        else
            abrir(req.tela, Object.assign({}, req.propriedades, { guia: g }))
        return true
    }
    // A tela de destino mostra qual campo destacar (e a rolagem dele).
    // MG-15: `aoChegar` é chamado somente depois que a rolagem concluir e o destaque aparecer.
    // Retorna true quando a janela assumiu o callback `aoChegar`.
    function guiaDestacar(item, rolagem, aoChegar) {
        if (!guiaAtiva)
            return false
        guia.destacar(item, rolagem, false, aoChegar)
        return true
    }
    // MG-5: preserva o destaque preso ao registro durante recarga de lista.
    function guiaIniciarRecargaLista() {
        if (guiaAtiva || validacaoAtiva || guia.destacando)
            guia.iniciarRecargaLista()
    }
    function guiaAtualizarRegistro(novoItem) {
        if (guiaAtiva || validacaoAtiva || guia.recarregandoLista)
            guia.atualizarItemDoRegistro(novoItem)
    }
    // 9a, 9c e 10a (C8, dev3.3): a notificação levou ao registro exato (um
    // só). Só o destaque visual, sem guia e sem orientação; some sozinho em
    // alguns segundos ou quando a tela sai. Nada é escolhido nem resolvido por
    // aqui: a decisão continua com a pessoa (e, no que é dele, com o gerente).
    function destacarAlvo(item, rolagem, aoChegar) {
        if (guiaAtiva || validacaoAtiva || !item)
            return
        fimDoDestaque.restart()
        guia.destacar(item, rolagem, true, () => {
            if (!guiaAtiva && !validacaoAtiva)
                fimDoDestaque.restart()
            if (typeof aoChegar === "function")
                aoChegar()
        })   // o véu leve (I4)
    }
    Timer {
        id: fimDoDestaque
        interval: 6000
        onTriggered: if (!janela.guiaAtiva && !janela.validacaoAtiva) guia.semDestaque()
    }
    // O destino muda a orientação (vazio = a de sempre do requisito).
    function guiaOrientar(texto) {
        if (guiaAtiva)
            guia.mostrar("pendente", guiaAtiva.requisito.titulo,
                         texto || guiaAtiva.requisito.texto.arg(guiaAtiva.origemNome), false, true)
    }
    // O destino tentou salvar e não deu: continua lá, com o que foi digitado.
    function guiaFalhou(mensagem) {
        if (guiaAtiva)
            guia.mostrar("erro", "Não foi salvo", mensagem, false, true)
    }
    function finalizarRetornoGuia(g, satisfeito) {
        // BUG-20261118-SJ7F (lacuna 14, caso 7i) e BUG-20261118-FXVT:
        // a faixa pertence estritamente à tela de origem; se a sessão caiu ou a
        // tela de origem já saiu (ou deixou de ser a atual), nada vai para outra tela.
        if (janela.voltandoParaEntrada || !(g.origem && pilha.find(item => item === g.origem) !== null)
                || pilha.currentItem !== g.origem)
            return
        origemDaFaixa = g.origem
        if (satisfeito)
            guia.mostrar("satisfeito", g.textoSatisfeito || "Requisito satisfeito — conclua a operação.", "", false, false)
        else
            guia.mostrar("pendente", "A operação ainda não foi concluída",
                         "Falta " + g.requisito.nome + ". O que você já tinha feito continua lá.", true, false)
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
                guia.mostrar("erro", g.requisito.titulo, r.ok ? r.dados.falta : r.erro, false, true)
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
                // O3 (BUG-20261118-FXVT): reconfere a presença da origem na pilha antes de restaurar.
                if (janela.voltandoParaEntrada || !(g.origem && pilha.find(item => item === g.origem) !== null))
                    return
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

    // ---- NCG-1 e NCG-2 (0.9.19-dev.1): validação progressiva guiada no mesmo formulário ----
    // Conduz o usuário campo a campo pelas pendências detectadas ao tentar salvar;
    // a notificação é persistente (não some por tempo), avança sozinha quando o
    // campo é corrigido e muda para "Requisito satisfeito — conclua a operação"
    // sem nunca gravar automaticamente.
    function iniciarValidacaoProgressiva(telaOrigem, obterPendencias) {
        debounceValidacao.stop()
        if (guiaAtiva || !telaOrigem || typeof obterPendencias !== "function")
            return false
        const pendencias = obterPendencias()
        if (!Array.isArray(pendencias) || pendencias.length === 0) {
            encerrarValidacaoProgressiva(telaOrigem)
            return false
        }
        origemDaFaixa = null
        validacaoAtiva = {
            tela: telaOrigem,
            obterPendencias: obterPendencias,
            satisfeito: false,
            ultimoCampo: ""
        }
        _aplicarEtapaValidacao(pendencias)
        return true
    }

    function _aplicarEtapaValidacao(pendencias) {
        if (!validacaoAtiva || !Array.isArray(pendencias) || pendencias.length === 0)
            return
        const p = pendencias[0]
        if (typeof p.preparar === "function")
            p.preparar()
        if (p.item && p.item.erro !== undefined)
            p.item.erro = p.texto || ""
        const restantes = pendencias.length > 1
            ? " (1 de " + pendencias.length + " pendências — corrija e avance para a próxima)"
            : ""
        guia.mostrar("pendente", p.titulo || "Corrija o campo destacado", (p.texto || "") + restantes, false, true)
        if (validacaoAtiva.ultimoCampo !== p.chave) {
            validacaoAtiva.ultimoCampo = p.chave
            const alvoItem = p.item
            const areaItem = p.rolagem || null
            const focarFn = () => {
                if (!validacaoAtiva || validacaoAtiva.ultimoCampo !== p.chave)
                    return
                if (typeof p.focar === "function")
                    p.focar()
                else if (alvoItem && typeof alvoItem.focarSempre === "function")
                    alvoItem.focarSempre()
                else if (alvoItem && typeof alvoItem.focar === "function")
                    alvoItem.focar()
                else if (alvoItem && alvoItem.combo)
                    alvoItem.combo.forceActiveFocus()
            }
            Qt.callLater(() => {
                if (!validacaoAtiva || validacaoAtiva.ultimoCampo !== p.chave)
                    return
                guia.destacar(alvoItem, areaItem, false, focarFn)
            })
        }
    }

    // Debounce gentil: impede que a validação progressiva roube o foco na 1ª letra digitada
    // de um campo livre (ex.: nome ou preço); avança automaticamente assim que a pessoa termina
    // de digitar (~600 ms) ou imediatamente ao sair do campo / tentar salvar.
    Timer {
        id: debounceValidacao
        interval: 600
        onTriggered: janela._executarReavaliacaoValidacao()
    }

    function _executarReavaliacaoValidacao() {
        if (!validacaoAtiva)
            return
        const pendencias = validacaoAtiva.obterPendencias()
        if (Array.isArray(pendencias) && pendencias.length > 0) {
            validacaoAtiva.satisfeito = false
            _aplicarEtapaValidacao(pendencias)
            return
        }
        // Todos os requisitos foram satisfeitos: remove destaque, muda a notificação
        // persistente e aguarda a confirmação explícita do usuário em Salvar (NCG-1 / NCG-2).
        validacaoAtiva.satisfeito = true
        validacaoAtiva.ultimoCampo = ""
        guia.semDestaque()
        guia.mostrar("satisfeito", "Requisito satisfeito — conclua a operação",
                     "Todos os campos pendentes foram corrigidos. Clique em Salvar (F2) para gravar.", false, true)
    }

    function reavaliarValidacaoProgressiva(telaOrigem, imediato) {
        if (!validacaoAtiva || validacaoAtiva.tela !== telaOrigem)
            return
        const pendencias = validacaoAtiva.obterPendencias()
        const continuaNoMesmo = Array.isArray(pendencias) && pendencias.length > 0
                                && pendencias[0].chave === validacaoAtiva.ultimoCampo
        if (continuaNoMesmo) {
            debounceValidacao.stop()
            _aplicarEtapaValidacao(pendencias)
            return
        }
        if (imediato === true) {
            debounceValidacao.stop()
            _executarReavaliacaoValidacao()
        } else {
            debounceValidacao.restart()
        }
    }

    function encerrarValidacaoProgressiva(telaOrigem) {
        debounceValidacao.stop()
        if (!validacaoAtiva)
            return
        if (telaOrigem && validacaoAtiva.tela !== telaOrigem)
            return
        validacaoAtiva = null
        if (!guiaAtiva) {
            guia.semDestaque()
            guia.esconder()
        }
    }

    // ---- NCG-3 (0.9.19-dev.1): navegação livre com F11 e retorno exato sem banner ----
    function abrirBuscaLivreF11() {
        const tela = pilha.currentItem
        if (!tela)
            return
        if (typeof tela.guardarRascunhoAgora === "function") {
            try {
                tela.guardarRascunhoAgora()
            } catch (e) {
                console.warn("Rustinho: falha ao guardar rascunho antes do F11: " + e)
            }
        }
        const foco = janela.activeFocusItem
        const snap = {
            tela: tela,
            foco: foco,
            cursor: foco && foco.cursorPosition !== undefined ? foco.cursorPosition : -1,
            selStart: foco && foco.selectionStart !== undefined ? foco.selectionStart : -1,
            selEnd: foco && foco.selectionEnd !== undefined ? foco.selectionEnd : -1,
            guiaAtiva: guiaAtiva,
            validacaoAtiva: validacaoAtiva,
            origemDaFaixa: origemDaFaixa,
            avisoGuia: guia.temAviso ? {
                estado: guia.estado,
                titulo: guia.titulo,
                texto: guia.texto,
                persistente: guia.persistente
            } : null,
            alvoDestaque: guia.alvo,
            rolagemDestaque: guia.areaRolagem,
            veuLeve: guia.veuLeve
        }
        pilhaSuspensaoF11 = pilhaSuspensaoF11.concat([snap])
        movimento.cancelar(true)
        // Suspende visualmente a guia/validação sem cancelá-la nem mostrar banner de origem.
        guiaAtiva = null
        validacaoAtiva = null
        origemDaFaixa = null
        guia.semDestaque(true)
        guia.esconder()
        abrir("TelaPecas")
    }

    function restaurarBuscaLivreF11() {
        if (pilhaSuspensaoF11.length === 0)
            return
        const topo = pilhaSuspensaoF11[pilhaSuspensaoF11.length - 1]
        if (!topo || pilha.currentItem !== topo.tela)
            return
        pilhaSuspensaoF11 = pilhaSuspensaoF11.slice(0, pilhaSuspensaoF11.length - 1)
        retornandoDeF11 = true
        depoisDaTroca(() => {
            Qt.callLater(() => { janela.retornandoDeF11 = false })
            if (pilha.currentItem !== topo.tela)
                return
            guiaAtiva = topo.guiaAtiva
            validacaoAtiva = topo.validacaoAtiva
            origemDaFaixa = topo.origemDaFaixa
            if (topo.avisoGuia)
                guia.mostrar(topo.avisoGuia.estado, topo.avisoGuia.titulo, topo.avisoGuia.texto,
                             !topo.avisoGuia.persistente, topo.avisoGuia.persistente)
            if (topo.alvoDestaque && topo.alvoDestaque.visible)
                guia.destacar(topo.alvoDestaque, topo.rolagemDestaque, topo.veuLeve)
            if (topo.foco && topo.foco.visible) {
                try {
                    topo.foco.forceActiveFocus()
                    if (topo.selStart >= 0 && topo.selEnd >= 0 && topo.selStart !== topo.selEnd
                            && typeof topo.foco.select === "function") {
                        const lim = typeof topo.foco.text === "string" ? topo.foco.text.length : topo.selEnd
                        topo.foco.select(Math.min(topo.selStart, lim), Math.min(topo.selEnd, lim))
                    } else if (topo.cursor >= 0 && topo.foco.cursorPosition !== undefined) {
                        const lim = typeof topo.foco.text === "string" ? topo.foco.text.length : topo.cursor
                        topo.foco.cursorPosition = Math.min(topo.cursor, lim)
                    }
                } catch (e) {
                    // O controle focado antes pode ter sido recriado; a tela segue funcional.
                }
            }
        })
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
        // N-1 da re-revisão: as janelinhas das telas que saem não voltam (a pilha inteira vai ser trocada).
        janelinhasAbertas = []
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

    // Caso 7 da navegação contextual (dev3.3): a operação com dinheiro foi
    // recusada porque o registro ficou inconsistente. Decisão do gerente:
    // aqui só se oferece o caminho ("Ver o registro"); nada muda de tela sem a
    // pessoa escolher. A janelinha da operação fecha guardando o que foi
    // digitado e, na volta (resolvido ou não), reabre igual: a operação nunca
    // é repetida sozinha. Devolve true quando a recusa era essa.
    function ehReferenciaInconsistente(r) {
        return !!r && !r.ok && typeof r.campo === "string" && r.campo.indexOf("requisito:caixas.referencia|") === 0
    }
    function oferecerReferencia(r, janelinha, aoVoltar, comMotivo) {
        if (!ehReferenciaInconsistente(r))
            return false
        // B-5 da revisão: com uma guia já em andamento, não há segunda oferta; quem chamou mostra o erro.
        if (guiaAtiva)
            return false
        // B-3 da revisão: curto (o motivo completo já está na janelinha, em vermelho). N-6 da re-revisão: sem
        // janelinha (o campo da compra, lançar a compra, desfazer cheque, desfazer fechamento de comissão), a
        // primeira frase do motivo vem na oferta (qual registro e o que falta). S-1 da 2ª re-revisão: a janelinha
        // que não mostra o erro (a conferência da compra) pede o motivo na oferta com `comMotivo`.
        let motivo = ""
        if ((!janelinha || comMotivo) && typeof r.erro === "string" && r.erro !== "") {
            const fim = r.erro.indexOf(". ")
            motivo = (fim >= 0 ? r.erro.slice(0, fim + 1) : r.erro) + " "
        }
        dialogo.perguntar("Registro inconsistente",
                          motivo + "A operação não foi feita. O gerente decide em Simultaneidade › Caixas, com o registro destacado; depois você volta e conclui.",
                          "Ver o registro", () => {
            // A-1 da revisão: fecha a cadeia inteira de janelinhas abertas (a ficha e a pergunta), guardando o que
            // foi digitado, e reabre na mesma ordem na volta. O fechamento por causa da guia não dispara a ação de
            // fechar de nenhuma delas (fechandoParaGuia).
            // N-1 da re-revisão: só as janelinhas que ainda existem e estão abertas (o registro pode ter guardado
            // uma destruída), e o `fechandoParaGuia` volta a false aconteça o que acontecer.
            const cadeia = janelinhasAbertas.filter(janelinhaViva)
            if (janelinhaViva(janelinha) && cadeia.indexOf(janelinha) < 0)
                cadeia.push(janelinha)
            fechandoParaGuia = true
            try {
                for (let i = cadeia.length - 1; i >= 0; i--) {
                    try {
                        cadeia[i].close()
                    } catch (e) {
                        // Destruída entre a conferência e o fechamento: nada a fechar.
                    }
                }
            } finally {
                fechandoParaGuia = false
            }
            guiar(r.campo, r.erro, satisfeito => {
                for (const p of cadeia) {
                    try {
                        if (p && typeof p.open === "function")
                            p.open()
                    } catch (e) {
                        // A janelinha já não existe (a tela dela saiu): nada a reabrir.
                    }
                }
                if (aoVoltar)
                    aoVoltar(satisfeito)
            })
        }, "principal")
        return true
    }
    // As janelinhas (Painel) abertas, na ordem em que abriram (A-1 da revisão).
    property var janelinhasAbertas: []
    property bool fechandoParaGuia: false
    function registrarJanelinha(p, aberta) {
        // N-1 da re-revisão: a janelinha destruída não vale `null` no JavaScript do Qt; `janelinhaViva` a reconhece.
        const lista = janelinhasAbertas.filter(x => x !== p && janelinhaViva(x))
        if (aberta)
            lista.push(p)
        janelinhasAbertas = lista
    }
    // A janelinha existe (não foi destruída) e está aberta.
    function janelinhaViva(p) {
        try {
            return !!p && typeof p.close === "function" && typeof p.open === "function" && p.opened === true
        } catch (e) {
            return false
        }
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
        // A orientação da guia e o aviso rápido ficam no alto, cada um no seu espaço: as telas descem por eles.
        anchors.topMargin: guia.alturaNoAlto + aviso.alturaNoAlto
        Behavior on anchors.topMargin {
            NumberAnimation {
                id: animDescida
                duration: Tema.duracaoPosicao(140)
                easing.type: Easing.OutCubic
            }
        }
        focus: true

        pushEnter: Transition {
            NumberAnimation { property: "opacity"; from: 0; to: 1; duration: Tema.duracaoEsmaecer(140) }
        }
        pushExit: Transition {
            NumberAnimation { property: "opacity"; from: 1; to: 0; duration: Tema.duracaoEsmaecer(100) }
        }
        popEnter: Transition {
            NumberAnimation { property: "opacity"; from: 0; to: 1; duration: Tema.duracaoEsmaecer(140) }
        }
        popExit: Transition {
            NumberAnimation { property: "opacity"; from: 1; to: 0; duration: Tema.duracaoEsmaecer(100) }
        }
        replaceEnter: Transition {
            NumberAnimation { property: "opacity"; from: 0; to: 1; duration: Tema.duracaoEsmaecer(140) }
        }
        replaceExit: Transition {
            NumberAnimation { property: "opacity"; from: 1; to: 0; duration: Tema.duracaoEsmaecer(100) }
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

    // N6 (decisão do proprietário, 18/11/2026: "1a"). Quando a orientação da guia ou o aviso entram ou saem, as telas
    // descem ou voltam pela altura deles (um aviso de uma linha: 66 px, quase o passo das linhas de encomenda). Um
    // clique dado nesse instante caía em outra linha: o "Já pedi" marcava a encomenda vizinha (M-2 da revisão do VIS2).
    // Enquanto as telas se movem, e por um instante depois, o clique e o toque nelas são DESCARTADOS (não vão para
    // lugar nenhum). O teclado, a roda do mouse, o próprio aviso, a guia e as janelinhas continuam valendo.
    readonly property real descidaDasTelas: guia.alturaNoAlto + aviso.alturaNoAlto
    readonly property bool descidaAnimando: animDescida.running
    onDescidaDasTelasChanged: cliqueDescartado.restart()
    Timer {
        id: cliqueDescartado
        // A volta das telas (140 ms) e um instante depois (300 ms), o tempo de quem já ia clicar.
        interval: Tema.movimentoReduzido ? 300 : 440
    }
    // Decisão 2c do proprietário (18/11/2026, bug YVZ4), no escopo que ele
    // aprovou (ajuste depois do S1-R4 da revisão Sonnet): ignorar novos cliques
    // QUANDO A LISTA MUDA debaixo do ponteiro. O botão apertado guarda o ponto e
    // o registro da linha em que estava (registrarToqueNoBotao, no Botao). Logo
    // depois da operação gravada, se o registro que fica naquele ponto não é
    // mais o mesmo (a linha marcada saiu e a de baixo subiu para o lugar dela),
    // os botões (Botao) não aceitam o clique por 0,4 s. Gravação que não muda a
    // lista não bloqueia nada, e o bloqueio não pega a rolagem por gesto nem o
    // que não é botão.
    Timer {
        id: cliqueDescartadoNaLista
        interval: 400
    }
    readonly property bool descartandoCliqueNaLista: cliqueDescartadoNaLista.running
    // Para os testes (a premissa do tempo, linha TESTE_TOQUE_DEPOIS_DA_MUDANCA):
    // quantos ms depois de a mudança da lista ser vista chegou o último toque
    // num botão (-1: nenhum depois de uma mudança). Um 2º toque que o roteiro
    // mandou "logo depois" e que chega ao programa fora dos 0,4 s não prova
    // nada sobre o bloqueio: o resultado é inconclusivo por tempo.
    property real mudancaDaListaEm: 0
    property real toqueDepoisDaMudanca: -1
    // S2-R3 da conferência Sonnet: só com --textos-da-tela (os testes); o
    // programa de verdade não mede nada.
    function anotarToqueNoBotao() {
        if (textosDaTela && mudancaDaListaEm > 0)
            toqueDepoisDaMudanca = Date.now() - mudancaDaListaEm
    }
    // Para os testes: quantas vezes a lista mudou debaixo do ponteiro.
    property int listasQueMudaram: 0
    property var toqueNoBotao: null
    function registrarToqueNoBotao(botao) {
        let dentro = false
        for (let i = botao; i; i = i.parent)
            if (i === pilha) {
                dentro = true
                break
            }
        if (!dentro) {
            toqueNoBotao = null
            return
        }
        const p = botao.mapToItem(null, botao.width / 2, botao.height / 2)
        const item = itemDaLinha(botao)
        toqueNoBotao = { x: p.x, y: p.y, linha: linhaDe(botao), item: item, vista: vistaDe(item), quando: Date.now() }
    }
    // A ListView da linha (o item da linha fica no contentItem dela), ou null
    // (uma lista feita com Repeater).
    function vistaDe(item) {
        const v = item && item.parent ? item.parent.parent : null
        return v && typeof v.itemAt === "function" && typeof v.indexAt === "function" && v.contentItem === item.parent
            ? v : null
    }
    // O item da linha que está agora no ponto: numa ListView, pela própria
    // lista (itemAt, nas coordenadas do conteúdo); senão, descendo pelos itens.
    function linhaNoPonto(t) {
        if (t.vista) {
            const c = t.vista.contentItem
            if (!c)
                return null
            const p = c.mapFromItem(null, t.x, t.y)
            return t.vista.itemAt(p.x, p.y)
        }
        return itemDaLinha(itemNoPonto(t.x, t.y))
    }
    // O item da linha de lista (o 1º acima com um modelData que seja um
    // registro); null fora de lista.
    function itemDaLinha(item) {
        for (let i = item; i; i = i.parent)
            if (i.modelData && typeof i.modelData === "object")
                return i
        return null
    }
    // O registro da linha de lista em que o item está: o 1º acima dele com um
    // modelData que seja um registro (objeto), pela chave dele (id, chave ou
    // número; senão, o conteúdo). "" quando não está numa lista.
    function linhaDe(item) {
        for (let i = item; i; i = i.parent) {
            const m = i.modelData
            if (m && typeof m === "object") {
                if (m.id !== undefined)
                    return "id:" + m.id
                if (m.chave !== undefined)
                    return "chave:" + m.chave
                if (m.numero !== undefined)
                    return "numero:" + m.numero
                // A linha que embrulha o registro (ex.: { tipo, e } em O que comprar).
                for (const k in m)
                    if (m[k] && typeof m[k] === "object" && m[k].id !== undefined)
                        return k + ":" + m[k].id
                return JSON.stringify(m)
            }
        }
        return ""
    }
    // O item mais de dentro da pilha naquele ponto da janela.
    function itemNoPonto(x, y) {
        let item = pilha
        for (let k = 0; k < 200; k++) {
            const p = item.mapFromItem(null, x, y)
            const filho = item.childAt(p.x, p.y)
            if (!filho)
                return item
            item = filho
        }
        return item
    }
    Connections {
        target: Api
        function onGravou() {
            // T-02: garante um ciclo afterSynchronizing -> frameSwapped após toda gravação.
            janela.update()
            if (janela.toqueNoBotao && Date.now() - janela.toqueNoBotao.quando < 1500) {
                conferirLinhaDoToque.vezes = 0
                conferirLinhaDoToque.interval = janela.atrasoConferenciaTeste > 0 ? janela.atrasoConferenciaTeste : 25
                conferirLinhaDoToque.restart()
            }
        }
    }
    // Só para os testes (com --textos-da-tela): --atraso-conferencia-lista=N faz
    // a 1ª volta da conferência vir N ms depois da gravação, como numa máquina
    // lenta em que a janela fica ocupada e o 2º toque é atendido antes da
    // conferência (o caso K4 do telas_lista_que_muda). Sem a opção, 0: nada
    // muda.
    readonly property int atrasoConferenciaTeste: {
        if (!textosDaTela)
            return 0
        const a = Qt.application.arguments.find(x => x.startsWith("--atraso-conferencia-lista="))
        return a ? Math.max(0, parseInt(a.slice(a.indexOf("=") + 1)) || 0) : 0
    }
    // A lista se refaz depois da gravação (no próximo quadro; com o desenho
    // lento, pode levar mais). A conferência espera a linha naquele ponto ser
    // OUTRA instância (a lista já se refez) e só então compara o registro: o
    // mesmo, a lista não mudou (nada é bloqueado); outro, mudou (bloqueia).
    // Sem linha nenhuma no ponto, espera a lista se refazer; se continuar sem
    // linha até o fim (a linha sumiu e nada ficou no lugar), conta como mudou.
    // Se a mesma instância continua lá até o fim, a lista não se refez: não
    // mudou. null = ainda não dá para dizer.
    function mudouALinhaDoToque(t, fim) {
        // S2-R1 da conferência Sonnet: a lista já não existe (a tela saiu da
        // pilha antes de a conferência terminar). Não há o que conferir: não
        // mudou debaixo do ponteiro.
        if (t.vista && !t.vista.contentItem)
            return false
        const item = linhaNoPonto(t)
        if (item && item === t.item)
            return fim ? false : null
        if (item)
            return linhaDe(item) !== t.linha
        return fim ? true : null
    }
    function concluirConferenciaDoToque(mudou) {
        conferirLinhaDoToque.stop()
        toqueNoBotao = null
        if (mudou) {
            listasQueMudaram++
            mudancaDaListaEm = Date.now()
            cliqueDescartadoNaLista.restart()
        }
    }
    readonly property bool conferindoLinhaDoToque: conferirLinhaDoToque.running
    // Um novo toque num botão enquanto a conferência ainda espera (numa
    // máquina lenta, o 2º toque pode ser atendido antes da 1ª volta do relógio
    // e cair na linha que subiu; o K4 prova esse caso). O botão pergunta
    // AQUI, na hora do toque: a lista já trocada é posta em ordem
    // (forceLayout) e o registro naquele ponto é comparado. Mudou: o bloqueio
    // começa e o toque é descartado (true). Não mudou, ou a lista ainda não
    // se refez: o toque passa (false).
    function conferirToqueAgora() {
        if (!conferirLinhaDoToque.running)
            return false
        const t = toqueNoBotao
        if (!t || t.linha === "") {
            concluirConferenciaDoToque(false)
            return false
        }
        // S2-R1: um erro aqui não pode deixar os botões mortos. A conferência
        // termina como "não mudou" e o toque passa (falha aberta).
        try {
            if (t.vista && t.vista.contentItem && typeof t.vista.forceLayout === "function")
                t.vista.forceLayout()
            const mudou = mudouALinhaDoToque(t, false)
            if (mudou === null)
                return false
            concluirConferenciaDoToque(mudou)
            return mudou
        } catch (e) {
            console.warn("Rustinho: a conferência da lista falhou e foi encerrada: " + e)
            concluirConferenciaDoToque(false)
            return false
        }
    }
    Timer {
        id: conferirLinhaDoToque
        interval: 25
        repeat: true
        property int vezes: 0
        onTriggered: {
            interval = 25
            const t = janela.toqueNoBotao
            if (!t || t.linha === "") {
                janela.concluirConferenciaDoToque(false)
                return
            }
            vezes++
            // S2-R1: um erro não pode deixar a conferência rodando para sempre
            // (com ela, a área de cada botão fica ligada).
            try {
                const mudou = janela.mudouALinhaDoToque(t, vezes >= 24)
                if (mudou !== null)
                    janela.concluirConferenciaDoToque(mudou)
            } catch (e) {
                console.warn("Rustinho: a conferência da lista falhou e foi encerrada: " + e)
                janela.concluirConferenciaDoToque(false)
            }
        }
    }
    readonly property bool descartandoCliqueNoMovimento: cliqueDescartado.running
    readonly property bool descartandoClique: cliqueDescartado.running || cliqueDescartadoNaLista.running
    readonly property bool temJanelinhaAberta: janelinhasAbertas.length > 0
    // BUG-20261120-TJ8X: enquanto qualquer janelinha modal estiver aberta, intercepta
    // ponteiro/TapHandler sobre a pilha (sem desabilitar a pilha, pois as janelinhas são
    // filhas das telas e renderizam no Overlay).
    Item {
        objectName: "escudoDeJanelinha"
        anchors.fill: pilha
        z: 900
        visible: janela.temJanelinhaAberta
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.AllButtons
            hoverEnabled: true
        }
        TapHandler {
            acceptedButtons: Qt.AllButtons
            grabPermissions: PointerHandler.CanTakeOverFromAnything
        }
    }
    MouseArea {
        objectName: "cliqueDescartadoNoMovimento"
        anchors.fill: pilha
        // Só a N6 (as telas se movendo). A 2c fica nos botões (Botao).
        enabled: cliqueDescartado.running
        acceptedButtons: Qt.AllButtons
        // Segura o clique (o pressionar e o soltar) e não faz nada com ele.
    }

    // T047 (decisão do proprietário de 18/11, sobre a tela Painel no
    // emulador): quantas setas de voltar estão visíveis agora (telas e
    // janelinhas). No Android, o programa diz isso no registro do sistema
    // depois de cada tela pronta, para o teste do emulador conferir sem
    // depender da árvore de acessibilidade; nos testes do computador, vai na
    // leitura dos textos (TESTE_SETAS_VISIVEIS). Só a contagem, nada dos dados.
    function setasVisiveis() {
        let n = 0
        function andar(item) {
            if (!item || !item.visible || item.opacity === 0)
                return
            if (item.objectName === "setaDeRetorno")
                n++
            for (let i = 0; i < item.children.length; i++)
                andar(item.children[i])
        }
        let raiz = janela.contentItem
        while (raiz.parent)
            raiz = raiz.parent
        andar(raiz)
        return n
    }
    Timer {
        id: contarSetasNoRegistro
        interval: 1500
        // Pelo mesmo caminho da "tela pronta" (o qInfo do diagnóstico, que o
        // android/testar.sh já lê): "tela pronta: TelaPecas · setas visíveis: 0".
        // O nome da tela continua o primeiro (o tela_atual do teste só lê ele).
        onTriggered: if (pilha.currentItem && typeof diagnostico !== "undefined")
                         diagnostico.telaPronta(String(pilha.currentItem).split("(")[0].split("_")[0]
                                                + " · setas visíveis: " + janela.setasVisiveis())
    }

    // F11 (NCG-3, 0.9.19-dev.1): ir para a busca de peças de qualquer tela preservando
    // o estado exato da tela de origem (foco, cursor, guia e validação) para retorno
    // sem banner "Você veio de...". Em TelaPecas e TelaInicio, apenas foca a busca.
    Shortcut {
        sequence: "F11"
        enabled: !!janela.estado.usuario
                 && !espera.visible
                 && !dialogo.opened
                 && !pedirGerente.opened
                 && !perguntaDescartarDesativado.opened
                 && !janela.temJanelinhaAberta
        onActivated: {
            const tela = pilha.currentItem
            if (!tela)
                return
            if (tela.objectName === "telaPecas" || tela.objectName === "telaInicio") {
                if (typeof tela.focarBusca === "function")
                    tela.focarBusca()
                return
            }
            janela.abrirBuscaLivreF11()
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
    // REC-6 / ES-1: se houver operação crítica em andamento (venda, recebimento,
    // importação, backup ou restauração), pede confirmação explicando o que está em
    // andamento e quando dá para cancelar com segurança, sem jamais prender a janela
    // indefinidamente.
    property bool podeFechar: false
    property string comandoCriticoSimulado: ""
    property var avaliacaoFechamentoAtual: null

    function fecharComBackup() {
        guardarRascunhosDaPilha()
        fechando.visible = true
        esperaFechar.start()
    }

    function tentarFecharJanela(fechar) {
        if (podeFechar || estado.primeiraVez)
            return true
        if (fechar)
            fechar.accepted = false
        const cmdAtivo = Api.comandoAtivo() || comandoCriticoSimulado || (espera.visible ? "operacao_em_andamento" : "")
        const temRascunho = temRascunhoAbertoNaPilha()
        if (temRascunho)
            guardarRascunhosDaPilha()
        const r = Api.chamar("trabalho_fechamento_avaliar", {
            comandoAtivo: cmdAtivo,
            telaAtiva: pilha.currentItem && pilha.currentItem.nomeNaNavegacao ? pilha.currentItem.nomeNaNavegacao : "",
            temRascunhoAberto: temRascunho
        })
        if (r.ok && r.dados && r.dados.emAndamento) {
            avaliacaoFechamentoAtual = r.dados
            confirmar(
                r.dados.titulo || "Operação em andamento",
                r.dados.explicacao || "",
                r.dados.textoSair || "Sair mesmo assim",
                () => {
                    janela.comandoCriticoSimulado = ""
                    janela.fecharComBackup()
                },
                "perigo"
            )
            return false
        }
        fecharComBackup()
        return false
    }

    onClosing: (fechar) => {
        tentarFecharJanela(fechar)
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
    // P6 / T021 (0.9.19-dev.1): telemetria de movimento somente nos testes (--medir-movimento).
    readonly property bool medirMovimento: textosDaTela && Qt.application.arguments.indexOf("--medir-movimento") >= 0
    Shortcut {
        sequence: "Ctrl+Shift+F12"
        enabled: janela.textosDaTela
        context: Qt.ApplicationShortcut
        onActivated: janela.escreverTextos()
    }
    Shortcut {
        sequence: "Ctrl+Shift+F11"
        enabled: janela.textosDaTela
        context: Qt.ApplicationShortcut
        onActivated: janela.close()
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
        // N6 e YVZ4 (18/11/2026): os roteiros esperam o fim do instante em que
        // o clique é descartado antes de clicar. Só para os testes.
        linhas.push("0\t0\tTESTE_CLIQUE_DESCARTADO:" + (janela.descartandoClique ? 1 : 0))
        linhas.push("0\t0\tTESTE_LISTA_MUDOU:" + janela.listasQueMudaram)
        linhas.push("0\t0\tTESTE_TOQUE_DEPOIS_DA_MUDANCA:" + Math.round(janela.toqueDepoisDaMudanca))
        linhas.push("0\t0\tTESTE_AVISO_FECHADO:" + aviso.fechadoPor)
        linhas.push("0\t0\tTESTE_SETAS_VISIVEIS:" + janela.setasVisiveis())
        linhas.push("0\t0\tTESTE_MOVIMENTO_REDUZIDO:" + (Tema.movimentoReduzido ? 1 : 0))
        linhas.push("0\t0\tTESTE_MODO_MOVIMENTO:" + Tema.modoMovimento)
        linhas.push("0\t0\tTESTE_GUIA_PERSISTENTE:" + (guia.temAviso && guia.persistente ? 1 : 0))
        linhas.push("0\t0\tTESTE_F11_SUSPENSAS:" + janela.pilhaSuspensaoF11.length)
        linhas.push("0\t0\tTESTE_MOVIMENTO_ATIVO:" + (janela.movimentoAtivo || guia.aguardandoChegada ? 1 : 0))
        linhas.push("0\t0\tTESTE_DESTAQUE_ATIVO:" + (guia.destacando ? 1 : 0))
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

    Movimento { id: movimento }

    // O aviso rápido, no alto: abaixo da faixa do modo seguro e da orientação da guia.
    Aviso {
        id: aviso
        topo: faixaModoSeguro.height + guia.alturaNoAlto
    }

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
        Behavior on opacity { NumberAnimation { duration: Tema.duracaoEsmaecer(220); easing.type: Easing.InQuad } }
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
                        width: Tema.movimentoReduzido ? parent.width * 0.45 : parent.width * 0.32
                        x: Tema.movimentoReduzido ? (parent.width - width) / 2 : 0
                        height: parent.height
                        radius: 4
                        color: Tema.azulEscuro
                        NumberAnimation on x {
                            running: espera.visible && !Tema.movimentoReduzido
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

    PerguntaMotivo {
        id: perguntaDescartarDesativado
        property var aoTerminar: null
        onFeito: {
            const cb = aoTerminar
            aoTerminar = null
            if (cb)
                cb()
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
    function _pertenceATela(item, tela) {
        if (!item || !tela)
            return false
        for (let p = item; p; p = p.parent)
            if (p === tela)
                return true
        return false
    }
    Connections {
        target: pilha
        function onCurrentItemChanged() {
            // S2-R1: a conferência da lista é da tela que estava em cima; com
            // outra tela, ela termina (sem bloquear nada) e o toque guardado sai.
            if (conferirLinhaDoToque.running)
                janela.concluirConferenciaDoToque(false)
            janela.toqueNoBotao = null
            if (movimento.ativo && movimento.area && !janela._pertenceATela(movimento.area, pilha.currentItem))
                movimento.cancelar(true)
            // BUG-20261118-FXVT: a faixa de retorno pertence à tela de origem;
            // quando ela deixa de ser a tela atual (voltar/substituir/início), some.
            if (!janela.guiaAtiva && !janela.validacaoAtiva && guia.temAviso
                    && janela.origemDaFaixa && pilha.currentItem !== janela.origemDaFaixa) {
                guia.esconder()
                janela.origemDaFaixa = null
            }
            // Destaque solto (notificação sem guia/validação) não vaza para outra tela,
            // mas não apaga um destaque que a tela recém-aberta já iniciou para si mesma.
            if (!janela.guiaAtiva && !janela.validacaoAtiva && (guia.destacando || guia.aguardandoChegada)
                    && (!guia.alvo || !janela._pertenceATela(guia.alvo, pilha.currentItem))) {
                guia.semDestaque(true)
            }
            // Validação progressiva (NCG-1) encerra quando a tela dela sai da pilha.
            if (janela.validacaoAtiva && (!janela.validacaoAtiva.tela
                    || pilha.find(item => item === janela.validacaoAtiva.tela) === null)) {
                janela.encerrarValidacaoProgressiva()
            }
            // NCG-3: limpa suspensões de telas já removidas da pilha e restaura ao retornar à origem.
            if (janela.pilhaSuspensaoF11.length > 0) {
                const vivas = janela.pilhaSuspensaoF11.filter(s => s.tela && pilha.find(item => item === s.tela) !== null)
                if (vivas.length !== janela.pilhaSuspensaoF11.length)
                    janela.pilhaSuspensaoF11 = vivas
                if (vivas.length > 0 && pilha.currentItem === vivas[vivas.length - 1].tela)
                    janela.restaurarBuscaLivreF11()
            }
            if (pilha.currentItem && typeof diagnostico !== "undefined") {
                diagnostico.telaPronta(String(pilha.currentItem).split("(")[0])
                contarSetasNoRegistro.restart()
            }
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
