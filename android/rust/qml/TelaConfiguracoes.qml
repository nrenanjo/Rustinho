import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Configurações, uma parte de cada vez nas pílulas: a loja, vendas,
// crediário, impressão, Simultaneidade, Gerente e "sobre". Simultaneidade
// (0.9.15) junta o que é dos caixas trabalhando juntos: Duplicatas,
// Histórico, Caixas, Backup e Fiscal (que saiu da tela inicial); uma senha do
// gerente ao entrar abre a consulta de todas (gravar pede a senha de novo).
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    property var config: null
    property bool alterado: false
    property string secao: "loja"
    readonly property var partesSimultaneidade: ["duplicatas", "historico", "caixas", "backup", "fiscal"]
    readonly property string grupo: partesSimultaneidade.includes(secao) ? "simultaneidade" : secao
    property string ultimaSimultaneidade: "duplicatas"
    onSecaoChanged: {
        if (partesSimultaneidade.includes(secao))
            ultimaSimultaneidade = secao
        // 9b: o filtro do registro vale só para a ida ao Histórico que o pediu.
        if (secao !== "historico") {
            filtroRegistro = null
            avisoHistorico = ""
            historicoVeioDeDuplicatas = false
        }
        // NCG-1: encerra eventual validação progressiva da aba que saiu de vista.
        if (janela && janela.validacaoAtiva) {
            if (secao !== "fiscal" && fiscal.item && janela.validacaoAtiva.tela === fiscal.item)
                janela.encerrarValidacaoProgressiva(fiscal.item)
            else if (secao !== "loja" && janela.validacaoAtiva.tela === tela)
                janela.encerrarValidacaoProgressiva(tela)
        }
    }
    // 9b (C8, dev3.3): "Ver no histórico" de um conflito em Duplicatas.
    // Auto só quando a ligação é inequívoca (o registro tem um id só e há
    // eventos dele); senão, todo o histórico, dizendo por quê. Só consulta.
    property var filtroRegistro: null
    property string avisoHistorico: ""
    function verHistoricoDe(c) {
        const temEventos = !!c.registroId && auditoria.some(r => r.entidade === c.tabela && r.entidadeId === c.registroId)
        secao = "historico"
        // M-5 da revisão: o filtro de texto que já estava digitado fica (vale de novo em "Ver tudo"); o Esc e a
        // seta voltam às Duplicatas, de onde a pessoa veio.
        historicoVeioDeDuplicatas = true
        if (temEventos) {
            filtroRegistro = { entidade: c.tabela, id: c.registroId, nome: c.oQue }
        } else {
            filtroRegistro = null
            // N-9 da re-revisão: o filtro de texto digitado antes continua valendo (M-5); o aviso diz isso, em vez
            // de "todo o histórico" com uma lista filtrada. S-2 da 2ª re-revisão: aqui fica só o começo; o fim
            // ("com o filtro …" ou "todo o histórico") acompanha o campo do filtro, na Nota do Histórico.
            avisoHistorico = "Não há evento só deste registro (" + c.oQue + ") entre os " + auditoria.length
                             + " mais recentes: mostrando "
        }
    }
    property bool historicoVeioDeDuplicatas: false
    // Simultaneidade › Histórico (0.9.15).
    property var historicoTroca: null
    property var auditoria: []
    readonly property bool temSalvar: ["loja", "vendas", "receber", "impressao", "gerente"].includes(secao)
    // Gerente (ou senha do gerente digitada agora): vê as configurações da
    // loja toda. O caixa vê só as deste computador.
    property bool completo: false
    property bool verBackup: false
    property bool verGerente: false
    // FALHA-05: consulta da Simultaneidade (Duplicatas, Histórico, Caixas),
    // separada da pílula Gerente.
    property bool verSimultaneidade: false
    property var gerente: null       // pílula Gerente: { acoes, usuarios }
    property var conflitos: []       // pílula Gerente: diferenças entre caixas
    property var codigos: null       // pílula Gerente: faixas e códigos repetidos
    property bool confirmarAssumir: false
    property bool confirmarCaixaNovo: false
    // Caixas da loja (0.9.15): a versão de cada um e as pendências da troca.
    property var caixasConhecidos: []
    property int prazoCompatibilidade: 7
    property var pendencias: []
    // Navegação guiada (0.9.16): aberta por janela.guiar para resolver um
    // requisito de outra tela. Sem guia, tudo funciona como sempre.
    property var guia: null
    // 9a e 9c (C8, dev3.3): aberta por uma notificação sobre UM registro: ele
    // fica destacado ("conflito:<id>", "codigo:<tipo>|<código>",
    // "caixa:<dispositivo>", "referencia:<tabela>|<chave>", "pendencia:<id>").
    // Só mostra: a decisão continua do gerente, na própria linha.
    property string alvo: ""
    // Fiscal → Loja reutiliza esta instância; guarda a seção para restaurá-la
    // quando o requisito for salvo ou a pessoa desistir.
    property string secaoAntesDaGuia: ""
    // Como esta tela aparece na orientação da navegação guiada ("Depois o
    // Rustinho volta para …").
    readonly property string nomeNaNavegacao: secao === "fiscal" ? "Fiscal" : "Configurações"
    // Se um gerente liberou esta tela para um caixa, acaba ao sair dela. A
    // guia que não terminou aqui também acaba (a tela saiu da pilha por outro
    // caminho: sessão encerrada, backup restaurado). Quando a pilha destrói a
    // tela, ela já saiu da janela e "janela" vale null; por isso a janela fica
    // guardada desde a criação (Component.onCompleted).
    property var janelaAoSair: null
    Component.onDestruction: {
        // OS-2 da 2ª re-revisão (C8): o relógio da guia não toca mais depois que a tela saiu.
        esperaDaGuia.stop()
        // A-2 da revisão (C8): encerra só as consultas que esta tela pede (e as gravações liberadas). Aberta por
        // cima do Financeiro (caso 7, caso 8), não derruba a consulta do gerente da tela de baixo.
        Api.chamar("gerente_encerrar", { acoes: ["configuracoes", "backup", "gerente", "simultaneidade"] })
        if (guia)
            janelaAoSair.abandonarGuia(guia)
    }

    // Os campos da loja e a parte onde ficam (usado ao mostrar o erro de um
    // campo depois de salvar e na navegação guiada).
    function campoChamado(nome) {
        return ({ loja: [loja, "loja"], caixa: [caixa, "loja"], cnpj: [cnpjLoja, "loja"], uf: [ufLoja, "loja"],
                  uf_cnpj: [linhaDocumentos, "loja"],
                  crediarioMulta: [multa, "receber"], crediarioJuros: [jurosMes, "receber"], crediarioCarencia: [carencia, "receber"],
                  orcamentoValidade: [validadeOrc, "vendas"], rodapeVenda: [rodapeVenda, "impressao"],
                  rodapeOrcamento: [rodapeOrcamento, "impressao"], rodapeOs: [rodapeOs, "impressao"] })[nome] || null
    }
    // O que a guia destaca: um campo, ou (caso 6, 0.9.18) a linha do primeiro
    // caixa que bloqueia o rascunho de OS, em Simultaneidade › Caixas, ou
    // (caso 7, dev3.3) a linha do registro inconsistente que a operação pediu.
    function alvoDaGuia(nome) {
        if (nome === "referencia") {
            const alvo = guia ? guia.alvo : null
            for (let i = 0; alvo && i < listaReferencias.count; i++) {
                const linha = listaReferencias.itemAt(i)
                if (linha && linha.modelData.tabela === alvo.tabela && linha.modelData.chave === alvo.chave)
                    return [linha, "caixas"]
            }
            return null
        }
        if (nome === "caixa_bloqueando") {
            for (let i = 0; i < versaoDosCaixas.count; i++) {
                const linha = versaoDosCaixas.itemAt(i)
                if (linha && linha.modelData.bloqueia)
                    return [linha, "caixas"]
            }
            return null
        }
        return campoChamado(nome)
    }
    function linhaDoAlvo() {
        const sep = alvo.indexOf(":")
        if (sep < 0)
            return null
        const tipo = alvo.slice(0, sep), valor = alvo.slice(sep + 1)
        const procurarCodigo = codigos => {
            for (let i = 0; i < listaRepetidos.count; i++) {
                const l = listaRepetidos.itemAt(i)
                if (l && codigos.some(c => c !== "" && c === l.modelData.codigo))
                    return l
            }
            return null
        }
        const procurar = (rep, chave) => {
            for (let i = 0; rep && i < rep.count; i++) {
                const l = rep.itemAt(i)
                if (l && chave(l.modelData) === valor)
                    return l
            }
            return null
        }
        const busca = ({
            conflito: () => procurar(listaConflitos, m => String(m.id)),
            // "tipo|código" (a notificação) ou "|código|barras" (a etiqueta bloqueada, B-1): o primeiro que bater.
            codigo: () => valor.startsWith("|")
                          ? procurarCodigo(valor.slice(1).split("|"))
                          : procurar(listaRepetidos, m => m.tipo + "|" + m.codigo),
            caixa: () => procurar(versaoDosCaixas, m => m.dispositivo),
            referencia: () => procurar(listaReferencias, m => m.tabela + "|" + m.chave),
            pendencia: () => procurar(listaPendencias, m => m.alteracaoId)
        })[tipo]
        return busca ? busca() : null
    }
    // M-2 da revisão (C8): a parte acabou de trocar e ainda está montando; destacar na hora deixava a linha fora
    // da área visível (a rolagem media posições velhas). Espera a linha existir com tamanho e a altura da rolagem
    // parar de mudar; aí rola até ela e destaca.
    function mostrarAlvo() {
        if (!alvo || guia)
            return
        esperaDoAlvo.tentativas = 0
        esperaDoAlvo.alturaAntes = -1
        esperaDoAlvo.restart()
    }
    Timer {
        id: esperaDoAlvo
        interval: 35
        repeat: true
        property int tentativas: 0
        property real alturaAntes: -1
        onTriggered: {
            const linha = tela.linhaDoAlvo()
            const assentou = rolagem.contentHeight > 0 && rolagem.contentHeight === alturaAntes
            alturaAntes = rolagem.contentHeight
            if ((linha && linha.height > 0 && assentou) || ++tentativas >= 25) {
                stop()
                tela.alvo = ""
                if (linha && janela)
                    janela.destacarAlvo(linha, rolagem)
            }
        }
    }
    // Guia: abre a parte do requisito e destaca o primeiro campo que falta.
    // Os campos da loja só aparecem com a senha do gerente: sem ela, pede a
    // senha primeiro (a guia continua apontando para o botão da senha). As
    // partes da Simultaneidade pedem a senha delas ao abrir (onCompleted).
    function comecarGuia() {
        if (partesSimultaneidade.includes(guia.requisito.propriedades.secao)) {
            Qt.callLater(mostrarCampoDaGuia)
            return
        }
        if (!completo) {
            janela.guiaDestacar(botaoSenhaGerente, null)
            janela.guiaOrientar("Esta parte das configurações pede a senha do gerente.")
            janela.comGerente("configuracoes", () => {
                tela.completo = true
                carregar()
                janela.guiaOrientar("")
                Qt.callLater(tela.mostrarCampoDaGuia)
            })
            return
        }
        Qt.callLater(mostrarCampoDaGuia)
    }
    function mostrarCampoDaGuia() {
        if (!guia)
            return
        const c = alvoDaGuia(guia.requisito.campos[0])
        if (!c)
            return
        secao = c[1]
        // N-2 da re-revisão (C8): a mesma espera do M-2. A parte acabou de trocar e ainda está montando; destacar
        // no próximo quadro deixava o registro do caso 7 fora da área visível (y = 813 numa janela de 760).
        esperaDaGuia.tentativas = 0
        esperaDaGuia.alturaAntes = -1
        esperaDaGuia.restart()
    }
    Timer {
        id: esperaDaGuia
        interval: 35
        repeat: true
        property int tentativas: 0
        property real alturaAntes: -1
        onTriggered: {
            if (!tela.guia) {
                stop()
                return
            }
            const c = tela.alvoDaGuia(tela.guia.requisito.campos[0])
            const assentou = rolagem.contentHeight > 0 && rolagem.contentHeight === alturaAntes
            alturaAntes = rolagem.contentHeight
            if ((c && c[0] && c[0].height > 0 && assentou) || ++tentativas >= 25) {
                stop()
                if (c && c[0])
                    tela.destacarCampoDaGuia(c[0])
            }
        }
    }
    function destacarCampoDaGuia(campo) {
        // OS-2: a tela pode estar saindo (a janela vale null quando a pilha a tira) ou a guia já ter acabado.
        if (!janela || !tela.guia)
            return
        // MG-15: o foco entra somente depois que a rolagem guiada concluir (ou direto em janela simulada de teste).
        const aplicarFoco = () => {
            if (!janela || !tela.guia)
                return
            // Estado e CNPJ juntos: o cursor vai para o que falta primeiro.
            const foco = campo === linhaDocumentos ? (ufLoja.indice > 0 ? cnpjLoja : ufLoja) : campo
            if (typeof foco.focarSempre === "function")
                foco.focarSempre()
            else if (foco.combo)
                foco.combo.forceActiveFocus()
        }
        if (janela.guiaDestacar(campo, rolagem, aplicarFoco) !== true)
            aplicarFoco()
    }
    function iniciarGuiaNaMesmaTela(g) {
        secaoAntesDaGuia = secao
        guia = g
        secao = g.requisito.propriedades.secao
        comecarGuia()
    }
    function restaurarGuiaNaMesmaTela(requisitoConfirmado) {
        const retorno = secaoAntesDaGuia
        if (retorno !== "")
            secao = retorno
        if (retorno === "fiscal" && fiscal.item) {
            let situacao = requisitoConfirmado
            if (!situacao) {
                const requisito = Api.chamar("requisito_conferir", { id: "loja.uf_cnpj" })
                situacao = requisito.ok ? requisito.dados : null
                if (!requisito.ok)
                    janela.avisar(requisito.erro, "erro")
            }
            // Na desistência, reflete o estado atual do núcleo sem concluir a guia.
            fiscal.item.dadosDaLoja = situacao
            const conferencia = Api.chamar("fiscal_conferir")
            if (conferencia.ok)
                // Não chamar preencher(): regras pode conter alterações de ST pendentes.
                fiscal.item.dados = conferencia.dados
            else
                janela.avisar(conferencia.erro, "erro")
        }
        secaoAntesDaGuia = ""
        guia = null
    }
    // Sair: com guia, volta à tela de origem sem marcar nada.
    function sair() {
        // OS-2 da 2ª re-revisão (C8): desistir para o relógio da guia; antes, um toque dele durante a saída chamava o
        // destaque com a janela já nula (TypeError no registro; o s2 do telas_c8_rerevisao2 reprovou na integ8).
        esperaDaGuia.stop()
        if (guia)
            janela.concluirGuia(false)
        else
            janela.voltar()
    }
    function descartarAlteracoesESair() {
        if (guia && secaoAntesDaGuia === "fiscal" && !carregar())
            return
        sair()
    }

    // Exemplo do crediário: parcela de R$ 100,00 com 10 dias de atraso.
    // ARQ-09 (dev.3.3, 2.2): o exemplo é a conta da cobrança, feita pelo
    // núcleo com as taxas como estão digitadas ("2.5" = 2,5%; a tela lia 25%
    // e refazia a conta do jeito dela). Taxa fora do permitido: sem exemplo
    // (o erro aparece no campo ao salvar).
    readonly property string exemploJuros: {
        const r = Api.chamar("crediario_exemplo", { multa: multa.texto, juros: jurosMes.texto, carencia: carencia.texto,
                                                    valor: "100", dias: 10 })
        if (!r.ok)
            return ""
        const e = r.dados
        if (e.dentroDaTolerancia)
            return "Exemplo: parcela de R$ " + e.valor + " com 10 dias de atraso fica em R$ " + e.total + " (está dentro da tolerância)."
        return "Exemplo: parcela de R$ " + e.valor + " com 10 dias de atraso fica em R$ " + e.total
               + (e.extra !== "0,00" ? " (R$ " + e.extra + " de multa e juros)." : ".")
    }

    function carregar() {
        const r = Api.chamar("config_ler")
        if (!r.ok) {
            janela.avisar(r.erro, "erro")
            return false
        }
        config = r.dados
        loja.texto = config.loja
        caixa.texto = config.caixa
        cnpjLoja.texto = config.cnpj
        enderecoLoja.texto = config.endereco
        telefoneLoja.texto = config.telefone
        imprimirDireto.checked = config.imprimirDireto
        multa.texto = config.crediarioMulta
        jurosMes.texto = config.crediarioJuros
        carencia.texto = config.crediarioCarencia
        validadeOrc.texto = config.orcamentoValidade
        rodapeVenda.texto = config.rodapeVenda
        rodapeOrcamento.texto = config.rodapeOrcamento
        rodapeOs.texto = config.rodapeOs
        imprimirLocal.checked = config.imprimirLocal
        carregarSync()
        ufLoja.modelo = [""].concat(config.ufs)
        ufLoja.indice = Math.max(0, ufLoja.modelo.indexOf(config.uf))
        tabelas.model = config.tabelas
        if (verGerente)
            carregarGerente()
        if (verSimultaneidade)
            carregarDuplicatas()
        alterado = false
        return true
    }

    // D-09: a senha do gerente abre as Configurações só para consultar;
    // Backup e a pílula Gerente pedem a senha deles; cada gravação pede a
    // senha para ela (veja janela.chamarGerente).
    function liberar() {
        janela.comGerente("configuracoes", () => {
            tela.completo = true
            carregar()
        })
    }
    // Simultaneidade: uma senha do gerente abre a consulta das partes que
    // ficam ali (configurações da loja, backup e a própria Simultaneidade:
    // duplicatas, histórico e caixas). A pílula Gerente pede a senha dela
    // (FALHA-05, D-09).
    // depois (opcional): roda com a parte já aberta (a guia destaca ali).
    function entrarSimultaneidade(destino, desistiu, depois) {
        janela.comGerenteVarias(["configuracoes", "backup", "simultaneidade"], "Simultaneidade", () => {
            const recarregar = !tela.completo || !tela.verSimultaneidade
            tela.completo = true
            tela.verBackup = true
            tela.verSimultaneidade = true
            if (recarregar)
                carregar()
            carregarHistorico()
            tela.secao = destino
            if (depois)
                depois()
        }, desistiu)
    }

    // Caso 8 (0.9.18): a faixa do modo seguro pede o Backup com esta tela
    // aberta. Troca só a parte (pede a senha do gerente, se faltar); desistir
    // da senha deixa a pessoa onde estava, nesta tela.
    function mostrarBackup() {
        if (secao !== "backup")
            entrarSimultaneidade("backup")
    }

    // NOTIF-03 (0.9.18): a tela foi aberta direto numa parte do gerente (por
    // uma notificação, por exemplo) e a pessoa desistiu da senha. Ela não
    // pediu a Loja: volta para onde estava. Só se esta tela ainda é a de cima.
    function sairSemGerente() {
        if (tela.StackView.view && tela.StackView.view.currentItem === tela)
            janela.voltar()
    }

    function carregarHistorico() {
        const h = Api.chamar("troca_historico")
        historicoTroca = h.ok ? h.dados : null
        const a = Api.chamar("auditoria_listar")
        auditoria = a.ok ? a.dados : []
    }

    function escolherSecao(id, desistiu) {
        if (id === "simultaneidade") {
            entrarSimultaneidade(ultimaSimultaneidade, desistiu)
            return
        }
        if (id === "backup" && !verBackup) {
            janela.comGerente("backup", () => {
                tela.verBackup = true
                tela.secao = id
            }, desistiu)
            return
        }
        if (id === "gerente" && !verGerente) {
            janela.comGerente("gerente", () => {
                tela.verGerente = true
                carregarGerente()
                tela.secao = id
            }, desistiu)
            return
        }
        secao = id
    }

    function carregarGerente() {
        const r = Api.chamar("gerente_ler")
        gerente = r.ok ? r.dados : null
    }

    // Simultaneidade › Duplicatas: diferenças entre caixas e códigos repetidos.
    function carregarDuplicatas() {
        const k = Api.chamar("conflitos_listar")
        conflitos = k.ok ? k.dados : []
        const c = Api.chamar("codigos_situacao")
        codigos = c.ok ? c.dados : null
        confirmarAssumir = false
        confirmarCaixaNovo = false
    }

    function milhar(n) {
        return Number(n).toLocaleString(Qt.locale("pt_BR"), "f", 0)
    }

    // Códigos das peças (pílula Gerente).
    function comandoCodigos(comando, dados, mensagem) {
        janela.chamarGerente(comando, dados || {}, r => {
            if (r.ok) {
                codigos = r.dados
                janela.avisar(mensagem, "ok")
            } else {
                janela.avisar(r.erro, "erro")
            }
            confirmarAssumir = false
            confirmarCaixaNovo = false
        })
    }
    readonly property int codigosParaVer: codigos ? codigos.repetidos.length + codigos.cruzadas.length + (codigos.bloqueio ? 1 : 0) : 0

    // Conflito entre caixas: usar a versão que perdeu, ou deixar como está.
    function resolverConflito(c, usar) {
        janela.chamarGerente("conflito_resolver", { id: c.id, usar: usar }, r => {
            if (r.ok) {
                conflitos = r.dados
                janela.avisar(usar ? "Voltou a versão que tinha perdido. Ela vai para os outros caixas." : "Ficou como está.", "ok")
            } else {
                janela.avisar(r.erro, "erro")
            }
        })
    }

    // R15 (R-03, decisão do proprietário de 25/11/2026): a compensação pronta de uma conta paga a mais. A pergunta
    // mostra o que vai ser feito; só o gerente confirma (o núcleo confere).
    function compensarConflito(c) {
        // B1 e B2 da revisão R3: num computador só (o outro mostra "Corrigido" depois da troca), e o núcleo confere
        // que a operação é a mesma que o gerente viu.
        janela.confirmar("Fazer a compensação?", c.compensacao + ". O estorno fica no histórico. Faça num computador só: "
                         + "depois da troca de dados, os outros mostram esta diferença como corrigida.", "Fazer a compensação",
                         () => janela.chamarGerente("conflito_compensar", { id: c.id, operacao: c.compensacaoOperacao }, r => {
                             if (r.ok) {
                                 conflitos = r.dados
                                 janela.avisar("Compensação feita. O estorno está no histórico.", "ok")
                             } else {
                                 janela.avisar(r.erro, "erro")
                             }
                         }))
    }

    function salvarGerente() {
        const acoes = {}
        for (const a of gerente.acoes)
            acoes[a.id] = a.soGerente
        const gerentes = {}
        for (const u of gerente.usuarios)
            gerentes[u.id] = u.gerente
        janela.chamarGerente("gerente_salvar", { acoes: acoes, gerentes: gerentes }, r => {
            if (r.ok) {
                gerente = r.dados
                alterado = false
                janela.atualizarEstado()
                janela.avisar("Salvo.", "ok")
            } else {
                janela.avisar(r.erro, "erro")
                carregarGerente()
            }
        })
    }

    // Caixa: só o nome deste computador e a impressora.
    function salvarComputador() {
        caixa.erro = ""
        const r = Api.chamar("computador_salvar", { caixa: caixa.texto, imprimirDireto: imprimirDireto.checked })
        if (r.ok) {
            janela.encerrarValidacaoProgressiva(tela)
            janela.atualizarEstado()
            carregar()
            janela.avisar("Configurações salvas.", "ok")
        } else if (r.campo === "caixa") {
            secao = "loja"
            caixa.erro = r.erro
            janela.rolarAteItem(rolagem, caixa, 24, () => caixa.focar())
        } else {
            janela.avisar(r.erro, "erro")
        }
    }

    // P6 / MG-D1 (0.9.19-dev.1): preferência local de movimento reduzido deste computador/aparelho.
    function escolherMovimento(modo) {
        if (Tema.modoMovimento === modo)
            return
        const r = Api.chamar("aparencia_salvar", { movimentoReduzido: modo })
        if (r.ok && r.dados) {
            Tema.modoMovimento = r.dados.movimentoReduzido || modo
            janela.avisar(Tema.movimentoReduzido
                          ? "Movimento reduzido ativado neste " + Tema.aparelho + "."
                          : "Movimento normal ativado neste " + Tema.aparelho + ".", "ok")
        } else {
            janela.avisar(r.erro || "Não foi possível salvar a preferência de movimento.", "erro")
        }
    }

    function marcarAlterado() {
        alterado = true
        if (janela)
            janela.reavaliarValidacaoProgressiva(tela)
    }

    // NCG-1 / NCG-2: validação progressiva local nas Configurações (quando não guiada por outra tela).
    function pendenciasDeValidacao() {
        const lista = []
        if (caixa.texto.trim() === "") {
            lista.push({
                chave: "caixa",
                item: caixa,
                rolagem: rolagem,
                preparar: () => { secao = "loja" },
                titulo: "Falta o nome deste computador",
                texto: "Informe o nome deste computador."
            })
        } else {
            caixa.erro = ""
        }
        if (completo) {
            const cnpjLimpo = cnpjLoja.texto.replace(/[^0-9A-Za-z]/g, "").toUpperCase()
            if (cnpjLimpo !== "") {
                const cnpjOk = cnpjLimpo.length === 14 ? cnpjLoja.cnpjValido(cnpjLimpo)
                             : cnpjLimpo.length === 11 ? cnpjLoja.cpfValido(cnpjLimpo) : false
                if (!cnpjOk) {
                    lista.push({
                        chave: "cnpj",
                        item: cnpjLoja,
                        rolagem: rolagem,
                        preparar: () => { secao = "loja" },
                        titulo: "CNPJ da loja inválido",
                        texto: "CNPJ inválido. Confira os números e as letras."
                    })
                } else {
                    cnpjLoja.erro = ""
                }
            } else {
                cnpjLoja.erro = ""
            }
        }
        return lista
    }

    function salvar() {
        if (secao === "gerente")
            return salvarGerente()
        if (!tela.guia && janela.iniciarValidacaoProgressiva(tela, tela.pendenciasDeValidacao))
            return
        if (!completo)
            return salvarComputador()
        const lista = []
        for (let i = 0; i < tabelas.count; i++) {
            const t = tabelas.itemAt(i)
            t.limparErros()
            lista.push({ numero: t.numero, nome: t.nomeTexto, lucroPadrao: t.lucroTexto })
        }
        loja.erro = ""
        caixa.erro = ""
        cnpjLoja.erro = ""
        ufLoja.erro = ""
        multa.erro = ""
        jurosMes.erro = ""
        carencia.erro = ""
        validadeOrc.erro = ""
        rodapeVenda.erro = ""
        rodapeOrcamento.erro = ""
        rodapeOs.erro = ""
        janela.chamarGerente("config_salvar", { loja: loja.texto, caixa: caixa.texto, tabelas: lista,
                                                cnpj: cnpjLoja.texto, uf: ufLoja.indice > 0 ? ufLoja.modelo[ufLoja.indice] : "",
                                                endereco: enderecoLoja.texto, telefone: telefoneLoja.texto,
                                                imprimirDireto: imprimirDireto.checked,
                                                crediarioMulta: multa.texto, crediarioJuros: jurosMes.texto,
                                                crediarioCarencia: carencia.texto, orcamentoValidade: validadeOrc.texto,
                                                rodapeVenda: rodapeVenda.texto, rodapeOrcamento: rodapeOrcamento.texto, rodapeOs: rodapeOs.texto,
                                                imprimirLocal: imprimirLocal.checked }, r => {
            if (r.ok) {
                janela.encerrarValidacaoProgressiva(tela)
                janela.atualizarEstado()
                carregar()
                janela.avisar("Configurações salvas.", "ok")
                // Guia: volta à origem só se o núcleo confirmar o requisito.
                if (tela.guia)
                    janela.concluirGuia(true)
                return
            }
            if (tela.guia)
                janela.guiaFalhou(r.erro)
            const campo = campoChamado(r.campo)
            if (campo) {
                const [alvo, onde] = campo
                secao = onde
                alvo.erro = r.erro
                janela.rolarAteItem(rolagem, alvo, 24, () => {
                    if (typeof alvo.focar === "function")
                        alvo.focar()
                })
                return
            }
            const m = r.campo ? r.campo.match(/^tabela(\d)(Nome|Lucro)$/) : null
            if (m) {
                secao = "vendas"
                for (let i = 0; i < tabelas.count; i++) {
                    const t = tabelas.itemAt(i)
                    if (t.numero === parseInt(m[1]))
                        t.mostrarErro(m[2], r.erro)
                }
            } else {
                janela.avisar(r.erro, "erro")
            }
        })
    }

    property var sync: null

    // esperarAssentar: só na recarga da lista depois de uma ação (recarregarReferencias). Na
    // chegada da guia (carregarSync/carregarCaixas), a tela já espera a parte assentar antes
    // de destacar (esperaDaGuia); esperar de novo aqui congelava o destaque da chegada.
    function _atualizarDestaqueDeLista(esperarAssentar) {
        if (!janela || !tela.guia)
            return
        const nome = tela.guia.requisito.campos[0]
        if (nome !== "referencia" && nome !== "caixa_bloqueando")
            return
        if (esperarAssentar === true) {
            // A mesma espera da chegada da guia (N-2 da re-revisão C8): a lista recarregada e o
            // resto da seção (a faixa da segurança dos caixas, 0.9.19) ainda estão montando;
            // a conta de "ficou fora da área visível" só vale com a altura do conteúdo assentada.
            esperaDaLista.tentativas = 0
            esperaDaLista.alturaAntes = -1
            esperaDaLista.nome = nome
            esperaDaLista.restart()
        } else {
            Qt.callLater(() => tela._conferirRegistroDaGuia(nome))
        }
    }
    function _conferirRegistroDaGuia(nome) {
        if (!janela || !tela.guia)
            return
        const c = tela.alvoDaGuia(nome)
        if (c && c[0]) {
            janela.guiaAtualizarRegistro(c[0])
            // N-7 + MG-5: se o registro recarregado ficou fora da área visível, rola até ele.
            if (rolagem && rolagem.contentItem && rolagem.height > 0) {
                const p = c[0].mapToItem(rolagem.contentItem, 0, 0)
                const topoV = rolagem.contentY
                const baseV = topoV + rolagem.height
                if (p.y < topoV + 8 || p.y + c[0].height > baseV - 8)
                    janela.rolarAteItem(rolagem, c[0], 24)
            }
        } else {
            janela.guiaAtualizarRegistro(null)
        }
    }
    Timer {
        id: esperaDaLista
        interval: 35
        repeat: true
        property int tentativas: 0
        property real alturaAntes: -1
        property string nome: ""
        onTriggered: {
            if (!janela || !tela.guia) {
                stop()
                return
            }
            const assentou = rolagem.contentHeight > 0 && rolagem.contentHeight === alturaAntes
            alturaAntes = rolagem.contentHeight
            if (!assentou && ++tentativas < 25)
                return
            stop()
            tela._conferirRegistroDaGuia(nome)
        }
    }

    // T013 (DT-P4-39): o convite que o gerente acabou de gerar (só nesta tela),
    // a lista dos convites ativos (pedida com a senha do gerente) e o código
    // deste caixa (pedido pelo caminho demorado: abre a identidade).
    property var conviteGerado: null
    property var convitesAtivos: []
    property bool convitesVistos: false
    property string codigoDesteCaixa: ""

    function gerarConvite() {
        codigoNovo.erro = ""
        nomeNovo.erro = ""
        janela.chamarGerenteDemorado("p4_gerar_convite", { codigoPedido: codigoNovo.texto, nome: nomeNovo.texto },
                                     "Gerando o convite…", r => {
            if (r.ok) {
                tela.conviteGerado = r.dados
                codigoNovo.texto = ""
                nomeNovo.texto = ""
                tela.recarregarReferencias()
            } else if (r.campo === "nome") {
                nomeNovo.erro = r.erro
            } else if (r.campo === "codigoPedido") {
                codigoNovo.erro = r.erro
            } else {
                janela.avisar(r.erro, "erro")
            }
        })
    }

    function verConvites() {
        janela.chamarGerente("p4_convites", {}, r => {
            if (r.ok) {
                tela.convitesAtivos = r.dados
                tela.convitesVistos = true
            } else {
                janela.avisar(r.erro, "erro")
            }
        })
    }

    function cancelarConvite(id) {
        janela.chamarGerente("p4_cancelar_convite", { id: id }, r => {
            if (!r.ok) {
                janela.avisar(r.erro, "erro")
                return
            }
            tela.convitesAtivos = r.dados
            if (tela.conviteGerado && tela.conviteGerado.id === id)
                tela.conviteGerado = null
            tela.recarregarReferencias()
            janela.avisar("Convite anulado: ele não serve mais.", "ok")
        })
    }

    // DT-P4-42 (N-04): a identidade deste computador não abre (o segredo dela não veio
    // do cofre de senhas): a tela oferece uma nova, com a senha do gerente.
    property string identidadeIlegivel: ""

    function mostrarCodigoDesteCaixa() {
        Api.chamarDemorado("p4_identidade", {}, "Preparando o código deste caixa…", r => {
            if (r.ok) {
                tela.codigoDesteCaixa = r.dados.codigoPedido
                tela.identidadeIlegivel = ""
            } else if (r.campo === "identidade") {
                tela.identidadeIlegivel = r.erro
            } else {
                janela.avisar(r.erro, "erro")
            }
        })
    }

    function gerarIdentidadeNova(retiradoPorEngano) {
        const p4 = tela.sync && tela.sync.p4 ? tela.sync.p4 : {}
        const retirado = !!p4.retirado || retiradoPorEngano === true
        // C2R2-1: o caixa retirado volta assim; C2R2-2: na raiz, a confirmação reforçada.
        // C2R3-3: quando é o gerente quem diz (o caixa não tem como saber), as duas consequências.
        const texto = p4.retirado
            ? "Este caixa foi retirado da loja pela chave dele. Com uma identidade nova, ele pode ser aprovado de novo: "
              + "depois, peça ao gerente um convite com o código novo. A identidade antiga fica guardada nesta pasta."
            : retiradoPorEngano === true
              ? "Use só se este caixa foi retirado da loja (os outros caixas recusam a conexão dele). Com uma identidade nova, "
                + "ele pode ser aprovado de novo com um convite. Atenção: se ele ainda estiver aprovado, a identidade nova faz ele "
                + "parar de trocar dados até a aprovação nova, e a identidade antiga precisa ser retirada num caixa aprovado. "
                + "A identidade antiga fica guardada nesta pasta."
            : p4.raiz
              ? "Este é o primeiro caixa da loja: foi ele que aprovou os outros. Com a identidade nova, ele precisa ser "
                + "aprovado por outro caixa aprovado da loja (com um convite). Se a loja só tem este caixa, chame o suporte "
                + "antes: a identidade antiga fica guardada nesta pasta e pode ser recuperada."
              : "Use só se este computador não consegue mais abrir a identidade dele (por exemplo, depois de "
                + "reinstalar o sistema). Se for só o cofre de senhas fora do ar agora, espere e tente de novo. "
                + "A identidade antiga fica guardada nesta pasta, sem ser apagada. Depois, num caixa aprovado, o gerente "
                + "retira este caixa da loja e o aprova de novo com um convite."
        janela.confirmar("Gerar uma identidade nova para este caixa?", texto, "Gerar identidade nova", () => {
            janela.chamarGerenteDemorado("p4_gerar_identidade_nova", { confirmarRaiz: !!p4.raiz, confirmarRetirado: retirado }, "Gerando a identidade nova…", r => {
                if (r.ok) {
                    tela.codigoDesteCaixa = r.dados.codigoPedido
                    tela.identidadeIlegivel = ""
                    janela.avisar("Identidade nova pronta. Peça ao gerente a aprovação deste caixa com o código novo.", "ok")
                } else {
                    janela.avisar(r.erro, "erro")
                }
            })
        }, "perigo")
    }

    function usarConvite() {
        conviteRecebido.erro = ""
        enderecoDaLoja.erro = ""
        janela.chamarGerenteDemorado("p4_aprovar_este_caixa", { convite: conviteRecebido.texto, enderecos: enderecoDaLoja.texto },
                                     "Pedindo a aprovação deste caixa…", r => {
            if (r.ok) {
                tela.sync = r.dados
                conviteRecebido.texto = ""
                janela.avisar("Pronto: este caixa foi aprovado na loja.", "ok")
            } else if (r.campo === "enderecos") {
                enderecoDaLoja.erro = r.erro
            } else {
                conviteRecebido.erro = r.erro
            }
        })
    }

    function liberarCaixasAntigos(dias) {
        janela.chamarGerente("p4_convivencia_p3", { dias: dias }, r => {
            if (!r.ok) {
                if (r.campo === "dias")
                    diasAntigos.erro = r.erro
                else
                    janela.avisar(r.erro, "erro")
                return
            }
            diasAntigos.erro = ""
            tela.sync = r.dados
            janela.avisar(dias === null ? "Caixas antigos recusados a partir de agora."
                                        : "Caixas antigos liberados neste caixa até " + r.dados.p4.convivenciaAteTexto + ".", "ok")
        })
    }

    function carregarCaixas() {
        const acompanhar = !!janela && !!tela.guia && tela.guia.requisito.campos[0] === "caixa_bloqueando"
        if (acompanhar)
            janela.guiaIniciarRecargaLista()
        const c = Api.chamar("caixas_conhecidos")
        if (c.ok) {
            caixasConhecidos = c.dados.caixas
            prazoCompatibilidade = c.dados.prazoDias
        }
        if (verSimultaneidade) {
            const p = Api.chamar("pendencias_listar")
            pendencias = p.ok ? p.dados : []
        }
        if (acompanhar)
            _atualizarDestaqueDeLista()
    }

    function retirarCaixa(c) {
        // T013 (DT-P4-40): o caixa com chave é retirado de verdade (pela chave, P4); a lista dos que
        // caem junto (P4-4) aparece antes de confirmar. O caixa sem chave (versão antiga) segue como antes.
        const comChave = !!c.temChave
        const juntos = c.dependentes && c.dependentes.length > 0
                       ? " Também deixam de ser aprovados, porque foram aprovados por ele: " + c.dependentes.join(", ")
                         + ". Depois, aprove-os de novo com um convite."
                       : ""
        motivoReferencia.perguntar(
            "Retirar " + c.nome + " da loja?",
            comChave
            ? "Use só quando este computador deixou de ser da loja. Ele deixa de trocar dados com os caixas atualizados, "
              + "mesmo com a senha da rede, e não conta mais para a versão dos caixas. Para voltar, só com uma aprovação nova do gerente."
              + " (Se algum caixa estiver com os caixas antigos liberados, troque também a senha da rede: por ele, quem tem a senha ainda troca dados.)"
              + juntos + " Fica no histórico com o seu nome e o motivo."
            // T076 (OPUS-D-3): a retirada não tira a senha da rede do computador; a tela não promete o contrário.
            : "Use só quando este computador deixou de ser da loja. Ele para de trocar dados e não conta mais para a versão dos caixas. "
              + "Atenção: quem souber a senha da rede consegue entrar de novo, com outro computador ou com este mesmo. Se ele saiu da loja "
              + "(vendido, roubado ou de alguém que saiu), troque a senha da rede em todos os caixas. "
              + "Fica no histórico com o seu nome e o motivo.",
            "Retirar da loja", (m, pronto) => janela.chamarGerenteDemorado("caixa_retirar", { dispositivo: c.dispositivo, motivo: m },
                                                                         "Retirando o caixa…", r => {
                pronto(r)
                if (r.ok) {
                    carregarCaixas()
                    janela.avisar(comChave ? c.nome + " foi retirado da loja: não troca mais dados com nenhum caixa."
                                           : c.nome + " foi retirado da loja. Se ele saiu da loja, troque a senha da rede em todos os caixas.", "ok")
                    // Caso 6: a guia volta à OS só se o núcleo confirmar que
                    // nenhum caixa bloqueia mais; senão destaca o próximo.
                    if (tela.guia && !janela.concluirGuia(true))
                        Qt.callLater(tela.mostrarCampoDaGuia)
                }
            }),
            "Motivo (por que sai da loja?)", "perigo")
    }

    // C2R2-5: o caixa com duas identidades aprovadas (aprovações ao mesmo tempo em
    // caixas diferentes): o gerente retira a que não vale mais; o caixa continua.
    function retirarIdentidade(c, ident) {
        motivoReferencia.perguntar(
            "Retirar a identidade " + ident.codigo + " de " + c.nome + "?",
            "Use para a identidade que não vale mais (a que não aparece no próprio caixa, em Configurações › Caixas da loja). "
            + "Ela sai da loja para sempre; o caixa continua com a outra."
            // C2R3-8: quem cai junto com esta identidade.
            + (ident.dependentes && ident.dependentes.length > 0
               ? " Também deixam de ser aprovados, porque foram aprovados por ela: " + ident.dependentes.join(", ")
                 + ". Depois, aprove-os de novo com um convite."
               : "")
            + " Fica no histórico com o seu nome e o motivo.",
            "Retirar a identidade", (m, pronto) => janela.chamarGerenteDemorado("caixa_retirar",
                { dispositivo: c.dispositivo, motivo: m, chave: ident.chave }, "Retirando a identidade…", r => {
                pronto(r)
                if (r.ok) {
                    carregarCaixas()
                    janela.avisar("A identidade " + ident.codigo + " saiu da loja.", "ok")
                }
            }),
            "Motivo (por que esta identidade sai?)", "perigo")
    }

    // C2R5-1: no primeiro caixa, com o registro de caixas cheio, o caixa já retirado por outro
    // caixa é retirado também aqui: o lugar que ele ocupa no registro volta.
    function liberarLugar(c) {
        const chaves = c.liberarLugar || []
        if (chaves.length === 0)
            return
        motivoReferencia.perguntar(
            "Liberar o lugar de " + c.nome + "?",
            "Este caixa já foi retirado da loja por outro caixa, mas o lugar que ele ocupa no registro de caixas só volta "
            + "quando o primeiro caixa da loja (este) também o retira. Depois disso, os outros caixas voltam a aprovar "
            + "caixas novos. Fica no histórico com o seu nome e o motivo.",
            "Liberar o lugar", (m, pronto) => janela.chamarGerenteDemorado("caixa_retirar",
                { dispositivo: c.dispositivo, motivo: m, chave: chaves[0] }, "Liberando o lugar…", r => {
                pronto(r)
                if (r.ok) {
                    carregarCaixas()
                    janela.avisar("O lugar de " + c.nome + " no registro de caixas voltou.", "ok")
                }
            }),
            "Motivo (por que ele saiu da loja?)", "perigo")
    }

    function readmitirCaixa(c) {
        janela.confirmar("Readmitir " + c.nome + "?", "Ele volta a trocar dados com os caixas da loja.", "Readmitir", () => {
            janela.chamarGerente("caixa_readmitir", { dispositivo: c.dispositivo }, r => {
                if (r.ok) {
                    carregarCaixas()
                    janela.avisar(c.nome + " foi readmitido.", "ok")
                } else {
                    janela.avisar(r.erro, "erro")
                }
            })
        })
    }

    function arquivarPendencia(p) {
        motivoReferencia.perguntar(
            "Arquivar esta pendência?",
            "Ela não é aplicada nem apagada: a cópia inteira continua guardada e fica no histórico com o seu nome e o motivo.",
            "Arquivar", (m, pronto) => janela.chamarGerente("pendencia_arquivar", { id: p.alteracaoId, motivo: m }, r => {
                pronto(r)
                if (r.ok)
                    pendencias = r.dados
            }),
            "Motivo (por que não tem como aplicar?)", "secundario")
    }

    function reaplicarPendencias() {
        janela.chamarGerente("pendencias_reaplicar", {}, r => {
            if (!r.ok) {
                janela.avisar(r.erro, "erro")
                return
            }
            pendencias = r.dados.pendencias
            janela.avisar(r.dados.aplicadas > 0 ? r.dados.aplicadas + " pendência(s) aplicada(s)." : "Nenhuma pendência pôde ser aplicada ainda.",
                          r.dados.aplicadas > 0 ? "ok" : "info")
        })
    }

    function carregarSync() {
        const acompanhar = !!janela && !!tela.guia && tela.guia.requisito.campos[0] === "referencia"
        if (acompanhar)
            janela.guiaIniciarRecargaLista()
        carregarCaixas()
        const r = Api.chamar("sync_ler")
        if (r.ok) {
            sync = r.dados
            syncLigado.checked = sync.ligado
            syncEnderecos.texto = sync.enderecos
            syncInterfaces.texto = sync.interfaces
            syncSenha.texto = ""
        }
        if (acompanhar)
            _atualizarDestaqueDeLista()
    }

    // Só a situação e a lista de referências (não mexe no que está sendo digitado).
    // MG-5: preserva o destaque preso ao registro durante recarga da lista.
    function recarregarReferencias() {
        const acompanhar = !!janela && !!tela.guia && tela.guia.requisito.campos[0] === "referencia"
        if (acompanhar)
            janela.guiaIniciarRecargaLista()
        const r = Api.chamar("sync_ler")
        if (r.ok)
            sync = r.dados
        if (acompanhar)
            _atualizarDestaqueDeLista(true)
    }

    function salvarSync() {
        syncSenha.erro = ""
        syncEnderecos.erro = ""
        syncInterfaces.erro = ""
        // O campo das interfaces só existe no modo completo; no simples não se manda nada.
        const dados = { ligado: syncLigado.checked, senha: syncSenha.texto, enderecos: syncEnderecos.texto }
        if (completo)
            dados.interfaces = syncInterfaces.texto
        janela.chamarGerente("sync_salvar", dados, r => {
            if (r.ok) {
                sync = r.dados
                syncSenha.texto = ""
                if (syncLigado.checked && sync.tarefasComFalha && sync.tarefasComFalha.length > 0)
                    janela.avisar("A troca de dados não pôde começar: " + sync.tarefasComFalha[0].motivo, "erro")
                else
                    janela.avisar(syncLigado.checked ? "Sincronização ligada. Os outros caixas aparecem em alguns segundos." : "Sincronização desligada.", "ok")
            } else if (r.campo === "enderecos") {
                syncEnderecos.erro = r.erro
            } else if (r.campo === "interfaces") {
                syncInterfaces.erro = r.erro
            } else {
                syncSenha.erro = r.erro
            }
        })
    }

    function tentarSyncNovamente() {
        const r = Api.chamar("sync_tentar_novamente")
        if (r.ok) {
            sync = r.dados
            if (sync.tarefasComFalha && sync.tarefasComFalha.length > 0)
                janela.avisar("A troca de dados ainda não está funcionando: " + sync.tarefasComFalha[0].motivo, "erro")
            else if (sync.ligado)
                janela.avisar("Troca de dados funcionando novamente.", "ok")
        } else {
            janela.avisar(r.erro, "erro")
        }
    }

    Timer {
        interval: 3000
        repeat: true
        running: tela.StackView.status === StackView.Active && !!tela.sync && tela.sync.ligado
        onTriggered: tela.recarregarReferencias()
    }

    function quandoFoi(iso) {
        if (!iso)
            return "ainda não"
        const s = Math.round((new Date() - new Date(iso)) / 1000)
        return s < 60 ? "há " + Math.max(s, 1) + " s" : s < 3600 ? "há " + Math.round(s / 60) + " min" : Qt.formatDateTime(new Date(iso), "dd/MM HH:mm")
    }

    function trocarSenha() {
        atual.erro = ""
        nova.erro = ""
        nova2.erro = ""
        const r = Api.chamar("senha_trocar", { atual: atual.texto, nova: nova.texto, nova2: nova2.texto })
        if (r.ok) {
            atual.texto = ""
            nova.texto = ""
            nova2.texto = ""
            janela.avisar("Senha trocada.", "ok")
            return
        }
        const campos = { atual: atual, nova: nova, nova2: nova2 }
        const alvo = campos[r.campo] || nova
        alvo.erro = r.erro
        alvo.focar()
    }

    function aoEsc() {
        // A seta e o Esc desistem primeiro do que está pela metade dentro
        // desta tela, as confirmações que tinham um botão "Cancelar" (saíram
        // na correção definitiva do botão de retorno, 07/11/2026).
        if (secao === "duplicatas" && confirmarCaixaNovo) {
            confirmarCaixaNovo = false
            return
        }
        if (secao === "duplicatas" && confirmarAssumir) {
            confirmarAssumir = false
            return
        }
        // M-5 da revisão (9b): o Histórico aberto por "Ver no histórico" volta às Duplicatas (a origem).
        if (secao === "historico" && historicoVeioDeDuplicatas) {
            secao = "duplicatas"
            return
        }
        if (secao === "backup" && parteBackup.item && parteBackup.item.aoEsc())
            return
        if (secao === "fiscal" && fiscal.item && fiscal.item.regrasAlteradas)
            janela.confirmar("Descartar as regras alteradas?", "As mudanças nas regras de ST ainda não foram salvas.",
                             "Descartar", () => tela.sair(), "perigo")
        else if (alterado)
            janela.confirmar("Descartar alterações?", "As configurações mudadas ainda não foram salvas.",
                             "Descartar", () => tela.descartarAlteracoesESair(), "perigo")
        else
            tela.sair()
    }

    Shortcut {
        sequence: "F2"
        enabled: tela.StackView.status === StackView.Active
        onActivated: tela.salvar()
    }

    // I8: no celular, as pílulas rolam junto com o conteúdo (menos na parte Fiscal, que tem a rolagem dela).
    readonly property bool topoNaRolagem: Tema.celular && tela.secao !== "fiscal"

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        anchors.bottomMargin: 0
        spacing: 14

        Cabecalho {
            sensivel: true
            titulo: "Configurações"
            subtitulo: ({
                loja: tela.completo ? "A loja, este computador e a sua senha" : "Este computador e a sua senha",
                gerente: "Quem é gerente e o que só o gerente pode fazer",
                vendas: "Tabelas de preço e orçamentos",
                receber: "Multa e juros de quem paga atrasado",
                impressao: "Comprovante, orçamento e impressora",
                duplicatas: "Diferenças entre caixas e códigos repetidos",
                historico: "A troca de dados entre os caixas e quem fez o quê",
                backup: "Cópias dos dados no computador e no Google Drive",
                caixas: "Os computadores da loja trocando dados pela rede",
                fiscal: "Emissão fiscal, CSOSN, regras de ST, NCM e PIS/Cofins das peças",
                aparencia: "Tema visual e preferência de movimento deste " + Tema.aparelho,
                sobre: "Versão e onde ficam os dados"
            })[tela.secao]
            Layout.fillWidth: true
            Rotulo {
                visible: tela.alterado
                text: "Não salvo"
                color: Tema.vermelhoNo(palette.window)
                Layout.alignment: Qt.AlignVCenter
                Layout.rightMargin: 6
            }
            Botao {
                id: botaoSenhaGerente
                visible: !tela.completo
                text: "Senha do gerente"
                tipo: "discreto"
                icone: "pessoa"
                onClicked: tela.liberar()
            }
            Botao {
                visible: tela.temSalvar || tela.alterado
                text: "Salvar"
                icone: "check"
                atalho: "F2"
                onClicked: tela.salvar()
            }
        }

        // I8 (VIS-C8-02, 15/11/2026): no celular, as pílulas (as partes e as subpartes) tomavam a altura e, em
        // modo seguro (com a faixa vermelha em cima), o Backup ficava numa tira de uns 85 px. Lá, elas rolam junto
        // com o conteúdo (ficam no alto da rolagem); a seta e o título continuam fixos. Na parte Fiscal (que tem a
        // rolagem dela) e no computador, ficam fixas aqui, como antes.
        Item {
            id: lugarDoTopo
            visible: !tela.topoNaRolagem
            Layout.fillWidth: true
            implicitHeight: topoConfig.implicitHeight
            ColumnLayout {
                id: topoConfig
                parent: tela.topoNaRolagem ? rolagem.contentItem : lugarDoTopo
                // Na rolagem, na mesma coluna do corpo (T042: a rolagem começa
                // Tema.buscaAlcance antes, à esquerda).
                x: tela.topoNaRolagem ? corpo.x : 0
                width: tela.topoNaRolagem ? corpo.width : lugarDoTopo.width
                y: tela.topoNaRolagem ? 4 : 0
                spacing: 14

                Pilulas {
                    modelo: tela.completo ? [
                        { id: "loja", nome: "Loja" },
                        { id: "vendas", nome: "Vendas" },
                        { id: "receber", nome: "Crediário" },
                        { id: "impressao", nome: "Impressão" },
                        { id: "simultaneidade", nome: "Simultaneidade" },
                        { id: "gerente", nome: "Gerente" },
                        { id: "aparencia", nome: "Aparência" },
                        { id: "sobre", nome: "Sobre" }
                    ] : [
                        { id: "loja", nome: "Este computador" },
                        { id: "impressao", nome: "Impressão" },
                        { id: "simultaneidade", nome: "Simultaneidade" },
                        { id: "aparencia", nome: "Aparência" },
                        { id: "sobre", nome: "Sobre" }
                    ]
                    atual: tela.grupo
                    onEscolhido: id => tela.escolherSecao(id)
                }

                // Subpílulas de Simultaneidade, numa faixa clara (no roxo, magenta).
                Rectangle {
                    visible: tela.grupo === "simultaneidade"
                    color: Tema.faixaSubpilulas
                    palette.window: Tema.superficie(color)
                    radius: 14
                    Layout.preferredWidth: Math.min(subpilulas.implicitWidth + 8, parent.width)
                    implicitHeight: subpilulas.implicitHeight + 8
                    Layout.topMargin: -6
                    Pilulas {
                        id: subpilulas
                        x: 4
                        y: 4
                        width: parent.width - 8
                        modelo: [
                            { id: "duplicatas", nome: "Duplicatas" },
                            { id: "historico", nome: "Histórico" },
                            { id: "caixas", nome: "Caixas" },
                            { id: "backup", nome: "Backup" },
                            { id: "fiscal", nome: "Fiscal" }
                        ]
                        atual: tela.secao
                        onEscolhido: id => {
                            if (id === "historico")
                                tela.carregarHistorico()
                            tela.secao = id
                        }
                    }
                }
            }
        }

        // Fiscal: a conferência fiscal inteira (a mesma da tela Fiscal).
        Loader {
            id: fiscal
            visible: tela.secao === "fiscal"
            // Mantém a tela e as regras editadas em memória durante Fiscal → Loja.
            active: tela.secao === "fiscal" || tela.secaoAntesDaGuia === "fiscal"
            Layout.fillWidth: true
            Layout.fillHeight: true
            source: "TelaFiscal.qml"
            onLoaded: {
                item.embutida = true
                item.carregar()
            }
        }

        Flickable {
            id: rolagem
            visible: tela.secao !== "fiscal"
            Layout.fillWidth: true
            Layout.fillHeight: true
            // T042: a rolagem recorta o que passa da borda, e o halo da barra de
            // pesquisa (o filtro da auditoria, no Histórico) passava da borda da
            // esquerda. A rolagem começa antes, na margem vazia da tela; o corpo
            // (e o topo, quando rola junto) fica no mesmo lugar de antes.
            Layout.leftMargin: -Tema.buscaAlcance
            clip: true
            contentHeight: corpo.y - 4 + corpo.implicitHeight + Tema.margem
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}

            ColumnLayout {
                id: corpo
                x: Tema.buscaAlcance
                width: Math.min(rolagem.width - Tema.buscaAlcance - 14, 960)
                y: tela.topoNaRolagem ? topoConfig.y + topoConfig.height + 14 : 4
                spacing: 14

                // ---------------- Loja ----------------
                Faixa {
                    visible: tela.secao === "loja"
                    titulo: tela.completo ? "A loja e este computador" : "Este computador"
                    color: Tema.pergaminho
                    Layout.fillWidth: true
                    Linha {
                        spacing: 12
                        Layout.fillWidth: true
                        Campo {
                            id: loja
                            visible: tela.completo
                            rotulo: "Nome da loja"
                            maximo: 80
                            Layout.fillWidth: true
                            onEditado: tela.marcarAlterado()
                        }
                        Campo {
                            id: caixa
                            rotulo: "Nome deste computador"
                            maximo: 30
                            Layout.preferredWidth: 220
                            onEditado: tela.marcarAlterado()
                        }
                    }
                    Linha {
                        id: linhaDocumentos
                        visible: tela.completo
                        spacing: 12
                        Layout.fillWidth: true
                        Campo {
                            id: cnpjLoja
                            objectName: "campoCnpjLoja"
                            // Verde quando o CNPJ (ou CPF) está completo e certo (0.9.18).
                            conferir: "documento"
                            rotulo: "CNPJ da loja"
                            dica: "00.000.000/0000-00"
                            maximo: 20
                            maiusculas: true
                            sensivel: true
                            Layout.preferredWidth: 270
                            onEditado: tela.marcarAlterado()
                        }
                        Selecao {
                            id: ufLoja
                            rotulo: "Estado"
                            Layout.preferredWidth: 100
                            onEscolhido: tela.marcarAlterado()
                        }
                        Nota {
                            text: "O estado decide as regras de ST. O CNPJ confere se a nota de compra é mesmo para a loja."
                            font.pixelSize: 14
                            Layout.fillWidth: true
                            Layout.alignment: Qt.AlignBottom
                        }
                    }
                    Linha {
                        visible: tela.completo
                        spacing: 12
                        Layout.fillWidth: true
                        Campo {
                            id: enderecoLoja
                            rotulo: "Endereço (sai no comprovante e no orçamento)"
                            dica: "Rua, número, bairro, cidade"
                            maximo: 150
                            Layout.fillWidth: true
                            onEditado: tela.alterado = true
                        }
                        Campo {
                            id: telefoneLoja
                            rotulo: "Telefone / WhatsApp"
                            maximo: 40
                            Layout.preferredWidth: 220
                            onEditado: tela.alterado = true
                        }
                    }
                }

                Faixa {
                    visible: tela.secao === "loja"
                    titulo: "Minha senha"
                    color: Tema.azul
                    Layout.fillWidth: true
                    Linha {
                        spacing: 12
                        Layout.fillWidth: true
                        Campo {
                            id: atual
                            rotulo: "Senha atual"
                            senha: true
                            maximo: 256
                            Layout.fillWidth: true
                            onConfirmado: nova.focar()
                        }
                        Campo {
                            id: nova
                            rotulo: "Nova senha"
                            dica: (janela && janela.estado && janela.estado.usuario && janela.estado.usuario.gerente)
                                  ? "Pelo menos 8 caracteres"
                                  : "Pelo menos 4 caracteres"
                            senha: true
                            maximo: 256
                            Layout.fillWidth: true
                            onConfirmado: nova2.focar()
                        }
                        Campo {
                            id: nova2
                            rotulo: "Repita a nova senha"
                            senha: true
                            maximo: 256
                            Layout.fillWidth: true
                            onConfirmado: tela.trocarSenha()
                        }
                    }
                    Botao {
                        text: "Trocar minha senha"
                        tipo: "secundario"
                        onClicked: tela.trocarSenha()
                    }
                }

                // ---------------- Vendas ----------------
                Faixa {
                    visible: tela.secao === "vendas"
                    titulo: "Tabelas de preço"
                    subtitulo: "a Revenda é o preço principal"
                    color: Tema.verde
                    corTitulo: Tema.verdeEscuro
                    Layout.fillWidth: true

                    RowLayout {
                        visible: !Tema.estreito
                        spacing: 12
                        Layout.fillWidth: true
                        Rotulo { text: "Nº"; Layout.preferredWidth: 30 }
                        Rotulo { text: "Nome da tabela"; Layout.fillWidth: true }
                        // OLHO-1: o lucro sugerido some com o título com o olho fechado.
                        Rotulo { visible: !Tema.oculto; text: "Lucro sugerido"; Layout.preferredWidth: 150 }
                    }
                    Repeater {
                        id: tabelas
                        delegate: Linha {
                            id: linhaTabela
                            required property var modelData
                            readonly property int numero: modelData.numero
                            readonly property string nomeTexto: nomeTabela.texto
                            readonly property string lucroTexto: lucroTabela.texto
                            colunasEstreito: 3
                            spacing: 12
                            Layout.fillWidth: true

                            function limparErros() {
                                nomeTabela.erro = ""
                                lucroTabela.erro = ""
                            }
                            function mostrarErro(qual, mensagem) {
                                const alvo = qual === "Nome" ? nomeTabela : lucroTabela
                                alvo.erro = mensagem
                                janela.rolarAteItem(rolagem, alvo, 24, () => alvo.focar())
                            }

                            Text {
                                property bool manterLargura: true
                                text: linhaTabela.numero
                                textFormat: Text.PlainText
                                color: Tema.dourado
                                font.family: Tema.titulo
                                font.weight: Font.Bold
                                font.pixelSize: 26
                                Layout.preferredWidth: 30
                                Layout.alignment: Qt.AlignTop
                            }
                            Campo {
                                id: nomeTabela
                                objectName: "campoNomeTabela"
                                texto: linhaTabela.modelData.nome
                                maximo: 30
                                Layout.fillWidth: true
                                onEditado: tela.alterado = true
                            }
                            // D-09: só quem vê os custos vê e muda o lucro sugerido. OLHO-1 (25/11/2026): com o
                            // olho fechado, o campo some com o título (o D-10 falava do Qt 6.4; a CI e os pacotes usam
                            // o 6.8.3, onde trocar "visible" dentro da linha não embaralha; B5 da revisão).
                            Campo {
                                id: lucroTabela
                                objectName: "campoLucroTabela"
                                visible: !Tema.oculto
                                texto: linhaTabela.modelData.lucroPadrao
                                enabled: !!tela.config && tela.config.verLucro
                                sufixo: "%"
                                numerico: true
                                sensivel: true
                                maximo: 10
                                dica: !!tela.config && tela.config.verLucro ? "—" : "só o gerente"
                                Layout.preferredWidth: 150
                                onEditado: tela.alterado = true
                            }
                        }
                    }
                    Nota {
                        text: "O lucro sugerido já vem preenchido quando se cadastra uma peça nova. As tabelas 2 a 5 aparecem em “Outros preços” no cadastro da peça. Mudar o lucro sugerido pede a senha do gerente."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                }

                Faixa {
                    visible: tela.secao === "vendas"
                    titulo: "Orçamento"
                    color: Tema.pergaminho
                    Layout.fillWidth: true
                    Linha {
                        spacing: 16
                        Layout.fillWidth: true
                        Campo {
                            id: validadeOrc
                            rotulo: "Vale por"
                            sufixo: "dias"
                            numerico: true
                            maximo: 3
                            dica: "7"
                            Layout.preferredWidth: 150
                            onEditado: tela.alterado = true
                        }
                        Nota {
                            text: "Já vem preenchido ao salvar um orçamento no Vender (dá para mudar na hora)."
                            font.pixelSize: 14
                            Layout.fillWidth: true
                            Layout.alignment: Qt.AlignBottom
                            Layout.bottomMargin: 10
                        }
                    }
                }

                // ---------------- Crediário ----------------
                Faixa {
                    visible: tela.secao === "receber"
                    titulo: "Atraso no crediário"
                    subtitulo: "sugeridos na hora de receber; quem recebe pode mudar ou dar desconto"
                    color: Tema.azul
                    Layout.fillWidth: true
                    Linha {
                        spacing: 12
                        Layout.fillWidth: true
                        Campo {
                            id: multa
                            rotulo: "Multa (uma vez)"
                            sufixo: "%"
                            numerico: true
                            maximo: 6
                            dica: "0"
                            Layout.preferredWidth: 200
                            onEditado: tela.alterado = true
                        }
                        Campo {
                            id: jurosMes
                            rotulo: "Juros ao mês"
                            sufixo: "%"
                            numerico: true
                            maximo: 6
                            dica: "0"
                            Layout.preferredWidth: 200
                            onEditado: tela.alterado = true
                        }
                        Campo {
                            id: carencia
                            rotulo: "Tolerância"
                            sufixo: "dias"
                            numerico: true
                            maximo: 2
                            dica: "0"
                            Layout.preferredWidth: 200
                            onEditado: tela.alterado = true
                        }
                        Item { Layout.fillWidth: true }
                    }
                    Nota {
                        text: "Os juros são proporcionais aos dias de atraso. Dentro da tolerância não se cobra nada; passou dela, conta desde o vencimento."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                    Rectangle {
                        visible: tela.exemploJuros !== ""
                        Layout.fillWidth: true
                        implicitHeight: exemplo.implicitHeight + 20
                        radius: Tema.raioCampo
                        color: Tema.vela(0.45)
                        Text {
                            id: exemplo
                            anchors.verticalCenter: parent.verticalCenter
                            x: 14
                            width: parent.width - 28
                            text: tela.exemploJuros
                            textFormat: Text.PlainText
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.pixelSize: 15
                            wrapMode: Text.WordWrap
                        }
                    }
                }

                // ---------------- Impressão ----------------
                Faixa {
                    visible: tela.secao === "impressao"
                    titulo: "Impressora"
                    color: Tema.pergaminho
                    Layout.fillWidth: true
                    Marcar {
                        id: imprimirDireto
                        text: "Imprimir direto na impressora padrão (sem abrir o PDF antes)"
                        onToggled: tela.alterado = true
                    }
                    Marcar {
                        id: imprimirLocal
                        visible: tela.completo
                        text: "Mostrar o local da peça (prateleira) no comprovante de venda"
                        onToggled: tela.alterado = true
                    }
                    Nota {
                        text: "Tudo sai em folha A4. Sem “imprimir direto”, o PDF abre no visualizador e se imprime com Ctrl+P."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                }

                Faixa {
                    visible: tela.secao === "impressao" && tela.completo
                    titulo: "Recado no pé da folha"
                    subtitulo: "opcional"
                    color: Tema.verde
                    corTitulo: Tema.verdeEscuro
                    Layout.fillWidth: true
                    AreaTexto {
                        id: rodapeVenda
                        rotulo: "Comprovante de venda"
                        dica: "Ex.: Troca só com este comprovante, em até 7 dias. Peça elétrica não tem troca."
                        Layout.fillWidth: true
                        onEditado: tela.alterado = true
                    }
                    AreaTexto {
                        id: rodapeOrcamento
                        rotulo: "Orçamento"
                        dica: "Ex.: Pagamento à vista no Pix com desconto. Consulte prazo de entrega."
                        Layout.fillWidth: true
                        onEditado: tela.alterado = true
                    }
                    AreaTexto {
                        id: rodapeOs
                        rotulo: "Ordem de serviço"
                        dica: "Ex.: Garantia de 90 dias para a mão de obra. Peças trocadas ficam à disposição por 7 dias."
                        Layout.fillWidth: true
                        onEditado: tela.alterado = true
                    }
                }

                // ---------------- Caixas da loja ----------------
                Faixa {
                    visible: tela.secao === "caixas"
                    titulo: "Caixas da loja"
                    subtitulo: "sincronização pela rede da loja"
                    color: Tema.verde
                    corTitulo: Tema.verdeEscuro
                    Layout.fillWidth: true
                    Marcar {
                        id: syncLigado
                        text: "Trocar dados com os outros caixas"
                        Layout.fillWidth: true
                    }
                    Linha {
                        spacing: 12
                        Layout.fillWidth: true
                        Campo {
                            id: syncSenha
                            rotulo: "Senha da rede da loja"
                            dica: tela.sync && tela.sync.temSenha ? "•••••••• (deixe vazio para manter)"
                                  : tela.sync && tela.sync.p4 && tela.sync.p4.aprovado ? "Não precisa neste caixa (só para os caixas antigos)"
                                  : "A mesma em todos os caixas"
                            senha: true
                            podeColar: true
                            maximo: 256
                            Layout.fillWidth: true
                        }
                        Campo {
                            id: syncEnderecos
                            rotulo: "Endereço dos outros caixas"
                            dica: "Só se não se acharem sozinhos (ex.: 192.168.0.12)"
                            maximo: 300
                            Layout.fillWidth: true
                        }
                    }
                    // Só no modo completo: quem usa VPN ou rede virtual entre os caixas
                    // diz quais interfaces também valem; o resto da loja nem vê o campo.
                    Campo {
                        id: syncInterfaces
                        visible: tela.completo
                        rotulo: "Interfaces extras (VPN, rede virtual, dados móveis)"
                        dica: tela.sync && tela.sync.interfacesIgnoradas.length > 0
                              ? "Hoje não anunciam: " + tela.sync.interfacesIgnoradas.join(", ")
                              : "Só se os caixas se falam por uma delas (ex.: tailscale0)"
                        maximo: 300
                        Layout.fillWidth: true
                    }
                    Nota {
                        text: tela.sync ? "Endereço deste caixa: " + (tela.sync.meuEndereco || "sem rede") : ""
                        font.pixelSize: 13
                        Layout.fillWidth: true
                    }
                    // O-3 / RVI-02 / RD-B: não mostra a troca como funcionando com uma
                    // tarefa parada ou esperando para tentar de novo (com o motivo, loja e caixa do outro Rustinho e botão "Tentar de novo").
                    Nota {
                        readonly property var falhas: tela.sync && tela.sync.tarefasComFalha ? tela.sync.tarefasComFalha : []
                        visible: falhas.length > 0
                        text: "A troca de dados não está funcionando neste caixa: "
                              + falhas.map(f => f.tarefa + " — " + f.motivo
                                                + (f.situacao === "parada" ? " (parou)" : " (tentando de novo)")).join("; ")
                              + (falhas.some(f => f.situacao === "parada")
                                 ? ". Quando a porta liberar, toque em “Tentar de novo” (ou feche e abra o Rustinho; se continuar, chame o suporte)." : ".")
                        color: Tema.vermelho
                        font.pixelSize: 13
                        Layout.fillWidth: true
                    }
                    Nota {
                        visible: !!tela.sync && tela.sync.senhaFraca
                        text: "A senha da rede tem menos de 4 caracteres. Quando puder, troque-a em todos os caixas."
                        font.pixelSize: 13
                        Layout.fillWidth: true
                    }
                    Row {
                        spacing: 12
                        Botao {
                            text: "Salvar sincronização"
                            tipo: "secundario"
                            onClicked: tela.salvarSync()
                        }
                        Botao {
                            text: "Tentar de novo"
                            tipo: "secundario"
                            visible: (tela.sync && tela.sync.tarefasComFalha ? tela.sync.tarefasComFalha.length : 0) > 0
                            onClicked: tela.tentarSyncNovamente()
                        }
                    }
                    Repeater {
                        model: tela.sync ? tela.sync.pares : []
                        delegate: Rectangle {
                            id: faixaDaLista
                            required property var modelData
                            Layout.fillWidth: true
                            required property int index
                            // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                            readonly property bool par: (index + 1) % 2 === 0
                            color: Tema.faixa(faixaDaLista.par, Tema.vela(0.4), "transparent")
                            palette.window: Tema.superficie(color)
                            radius: Tema.raioCampo
                            implicitHeight: conteudoDaFaixa.implicitHeight + 10
                            implicitWidth: conteudoDaFaixa.implicitWidth + 20
                            RowLayout {
                                id: conteudoDaFaixa
                                x: 10
                                y: 5
                                width: faixaDaLista.width - 20
                                spacing: 8
                                Rectangle {
                                    Layout.preferredWidth: 10
                                    Layout.preferredHeight: 10
                                    Layout.alignment: Qt.AlignVCenter
                                    radius: 5
                                    color: modelData.erro ? Tema.vermelhoDaFaixa(faixaDaLista.par) : modelData.sincronizadoEm ? Tema.verdeEscuroDaFaixa(faixaDaLista.par) : Tema.apagadoDaFaixa(faixaDaLista.par)
                                }
                                Text {
                                    text: (modelData.nome || "Caixa") + " · " + (modelData.erro ? modelData.erro : "sincronizado " + tela.quandoFoi(modelData.sincronizadoEm) + (modelData.via ? " · via " + modelData.via : ""))
                                    textFormat: Text.PlainText
                                    color: modelData.erro ? Tema.vermelhoDaFaixa(faixaDaLista.par) : Tema.tintaDaFaixa(faixaDaLista.par)
                                    font.family: Tema.texto
                                    font.pixelSize: 14
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                            }
                        }
                    }
                    // Esperar dados de outro caixa é normal: uma linha discreta (e o atraso só se existir).
                    Nota {
                        visible: !!tela.sync && tela.sync.pendencias > 0
                        text: tela.sync ? tela.sync.pendencias + (tela.sync.pendencias === 1 ? " registro espera" : " registros esperam")
                                          + " dados de outro caixa e isso se resolve sozinho."
                                          + (tela.sync.atrasadas > 0 ? " " + tela.sync.atrasadas + " há mais de 24 horas: confira se os outros caixas estão ligados, na mesma rede e com a mesma versão." : "") : ""
                        font.pixelSize: 13
                        Layout.fillWidth: true
                    }
                    // Só o que tem problema comprovado aparece em vermelho.
                    Text {
                        visible: !!tela.sync && tela.sync.quebradas > 0
                        text: tela.sync ? tela.sync.quebradas + (tela.sync.quebradas === 1 ? " registro com problema." : " registros com problema.")
                                          + " Só o dinheiro desses registros fica bloqueado até o gerente resolver; o resto da loja funciona normalmente." : ""
                        textFormat: Text.PlainText
                        color: Tema.vermelho
                        font.family: Tema.texto
                        font.pixelSize: 13
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                    // Cada registro incompleto: o que é, o que falta e o que fazer.
                    Repeater {
                        id: listaReferencias
                        // Caso 7: o registro que a guia procura vem primeiro (mesmo depois dos 20).
                        model: {
                            const todas = tela.sync && tela.sync.referencias ? tela.sync.referencias.filter(r => r.estado !== "esperando" || r.atrasada) : []
                            const alvo = tela.guia ? tela.guia.alvo : null
                            const daGuia = alvo ? todas.filter(r => r.tabela === alvo.tabela && r.chave === alvo.chave) : []
                            return daGuia.concat(todas.filter(r => daGuia.indexOf(r) < 0)).slice(0, 20)
                        }
                        delegate: Rectangle {
                            id: linhaRef
                            required property var modelData
                            Layout.fillWidth: true
                            required property int index
                            // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                            readonly property bool par: (index + 1) % 2 === 0
                            color: Tema.faixa(linhaRef.par, Tema.vela(0.4), "transparent")
                            palette.window: Tema.superficie(color)
                            radius: Tema.raioCampo
                            implicitHeight: conteudoDaFaixa2.implicitHeight + 10
                            implicitWidth: conteudoDaFaixa2.implicitWidth + 20
                            ColumnLayout {
                                id: conteudoDaFaixa2
                                x: 10
                                y: 5
                                width: linhaRef.width - 20
                                spacing: 6
                                Text {
                                    // H10 (7.2): os dados vão escapados (o texto é formatado).
                                    text: "<b>" + Tema.escapar(linhaRef.modelData.oQue) + "</b> (desde " + Tema.escapar(linhaRef.modelData.desde) + ") aponta para "
                                          + Tema.escapar(linhaRef.modelData.oQueFalta.toLowerCase()) + " que "
                                          + (linhaRef.modelData.estado === "esperando" ? (linhaRef.modelData.atrasada ? "ainda não chegou (atrasada)" : "ainda não chegou")
                                             : linhaRef.modelData.estado === "apagado" ? "foi apagado"
                                             : linhaRef.modelData.estado === "aceito" ? "não existe (aceito pelo gerente " + Tema.escapar(linhaRef.modelData.aceitaPor)
                                                                                      + " em " + Tema.escapar(linhaRef.modelData.aceitaEm) + ": " + Tema.escapar(linhaRef.modelData.aceiteMotivo) + ")"
                                             : "não existe (comprovado)")
                                          + ". " + Tema.escapar(linhaRef.modelData.acao)
                                    textFormat: Text.StyledText
                                    color: linhaRef.modelData.estado === "esperando" || linhaRef.modelData.estado === "aceito" ? Tema.tintaSuaveDaFaixa(linhaRef.par) : Tema.vermelhoDaFaixa(linhaRef.par)
                                    font.family: Tema.texto
                                    font.pixelSize: 13
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                                Botao {
                                    visible: linhaRef.modelData.podeAceitar
                                    text: "Aceitar como está"
                                    tipo: "secundario"
                                    onClicked: motivoReferencia.perguntar(
                                        "Aceitar como está?",
                                        "O que falta não vai aparecer por isso: só libera o dinheiro deste registro. Fica no histórico com o seu nome e o motivo.",
                                        "Aceitar", (m, pronto) => {
                                            // O registro desta linha, guardado antes: a linha é recriada ao recarregar.
                                            const tabelaAceita = linhaRef.modelData.tabela, chaveAceita = linhaRef.modelData.chave
                                            janela.chamarGerente("referencia_aceitar",
                                                                 { tabela: tabelaAceita, chave: chaveAceita, motivo: m }, r => {
                                            // Caso 7: a guia volta à operação só se o núcleo confirmar
                                            // que o registro dela deixou de bloquear; a operação não é refeita.
                                            // Decide ANTES do pronto(r): ele recarrega a lista e esta linha
                                            // (o contexto desta função) é destruída.
                                            const t = tela, j = janela
                                            // B-2 da revisão: só volta se o aceito foi o registro da guia (outro
                                            // registro aceito não é "continua inconsistente").
                                            const daGuia = !!t.guia && !!t.guia.alvo && t.guia.alvo.tabela === tabelaAceita
                                                           && t.guia.alvo.chave === chaveAceita
                                            const comGuia = r.ok && daGuia
                                            pronto(r)
                                            if (comGuia && !j.concluirGuia(true))
                                                Qt.callLater(t.mostrarCampoDaGuia)
                                            // N-7 + MG-5: quando outro registro é aceito com a guia aberta,
                                            // `motivoReferencia.onFeito` -> `recarregarReferencias()` congela e
                                            // desliza suavemente o destaque até a nova posição do registro da guia.
                                        })
                                        },
                                        "Motivo (por que aceitar?)", "secundario")
                                }
                            }
                        }
                    }
                }

                // ---------------- Caixas da loja: segurança (protocolo 4, 0.9.19) ----------------
                // T013 (DT-P4-39): a situação deste caixa, a aprovação de caixas novos pelo convite
                // (com a senha do gerente) e os caixas antigos. O que abre a identidade deste
                // computador vai pelo caminho demorado (DT-P4-30).
                Faixa {
                    id: faixaSeguranca
                    readonly property var p4: tela.sync && tela.sync.p4 ? tela.sync.p4 : null
                    visible: tela.secao === "caixas" && !!p4
                    titulo: "Segurança dos caixas"
                    subtitulo: p4 && p4.aprovado ? "só os caixas aprovados pelo gerente trocam dados" : "este caixa ainda não foi aprovado"
                    color: Tema.pergaminho
                    Layout.fillWidth: true
                    Nota {
                        text: !faixaSeguranca.p4 ? ""
                              : faixaSeguranca.p4.retirado
                                ? "Este caixa foi retirado da loja pela chave dele: ele não troca dados com os outros caixas. "
                                  + "Para voltar, gere aqui uma identidade nova (com a senha do gerente) e peça ao gerente um "
                                  + "convite com o código novo."
                              : faixaSeguranca.p4.aprovado
                                ? (faixaSeguranca.p4.raiz ? "Este caixa é aprovado na loja (foi o primeiro caixa dela)." : "Este caixa é aprovado na loja.")
                                  + " Para um caixa novo entrar, gere aqui um convite com o código que aparece na tela dele."
                                : "Para trocar dados com os caixas atualizados, este caixa precisa ser aprovado pelo gerente, uma vez só: "
                                  + "num caixa já aprovado, o gerente gera um convite com o código deste caixa; depois, digite o convite aqui."
                        Layout.fillWidth: true
                    }

                    // Caixa aprovado: aprovar caixa novo.
                    Linha {
                        visible: !!faixaSeguranca.p4 && faixaSeguranca.p4.aprovado
                        spacing: 12
                        Layout.fillWidth: true
                        Campo {
                            id: codigoNovo
                            objectName: "codigoDoCaixaNovo"
                            rotulo: "Código do caixa novo"
                            dica: "Aparece na tela dele. Ex.: 4F2K-9QXA-7MPC"
                            podeColar: true
                            maiusculas: true
                            maximo: 20
                            Layout.fillWidth: true
                            onConfirmado: nomeNovo.focar()
                        }
                        Campo {
                            id: nomeNovo
                            objectName: "nomeDoCaixaNovo"
                            rotulo: "Nome do caixa novo"
                            dica: "Ex.: Caixa 2"
                            maximo: 30
                            Layout.fillWidth: true
                            onConfirmado: tela.gerarConvite()
                        }
                    }
                    Row {
                        visible: !!faixaSeguranca.p4 && faixaSeguranca.p4.aprovado
                        spacing: 12
                        Botao {
                            text: "Gerar convite"
                            onClicked: tela.gerarConvite()
                        }
                        // C2R2-1: o caixa retirado não fica sabendo (os outros recusam a
                        // conexão dele: a troca mostra a recusa); o gerente gera a identidade nova.
                        Botao {
                            // A raiz não é retirada (P4-3).
                            visible: !!faixaSeguranca.p4 && !faixaSeguranca.p4.raiz
                            text: "Este caixa foi retirado da loja?"
                            tipo: "discreto"
                            onClicked: tela.gerarIdentidadeNova(true)
                        }
                        Botao {
                            visible: !!faixaSeguranca.p4 && faixaSeguranca.p4.convitesAtivos > 0
                            text: "Convites ativos (" + (faixaSeguranca.p4 ? faixaSeguranca.p4.convitesAtivos : 0) + ")"
                            tipo: "secundario"
                            onClicked: tela.verConvites()
                        }
                    }
                    // O convite que acabou de ser gerado: o texto (com Copiar), o QR e a validade.
                    Rectangle {
                        visible: !!tela.conviteGerado
                        Layout.fillWidth: true
                        color: Tema.vela(0.5)
                        palette.window: Tema.superficie(color)
                        radius: Tema.raioCampo
                        implicitHeight: conteudoConvite.implicitHeight + 20
                        RowLayout {
                            id: conteudoConvite
                            x: 10
                            y: 10
                            width: parent.width - 20
                            spacing: 16
                            ColumnLayout {
                                spacing: 8
                                Layout.fillWidth: true
                                Campo {
                                    objectName: "conviteGerado"
                                    rotulo: "Convite para " + (tela.conviteGerado ? tela.conviteGerado.nome : "")
                                    texto: tela.conviteGerado ? tela.conviteGerado.convite : ""
                                    somenteLeitura: true
                                    tamanhoFonte: 20
                                    altura: 46
                                    Layout.fillWidth: true
                                }
                                Nota {
                                    text: tela.conviteGerado
                                          ? "Vale até " + tela.conviteGerado.expiraEmTexto + " (10 minutos) e uma vez só. No caixa novo: "
                                            + "Juntar este caixa › digite o convite (ou leia o QR com o celular)."
                                            + (tela.conviteGerado.endereco ? " Se ele não achar este caixa sozinho, digite lá o endereço "
                                                                             + tela.conviteGerado.endereco.split(":")[0] + "." : "")
                                          : ""
                                    font.pixelSize: 13
                                    Layout.fillWidth: true
                                }
                                Botao {
                                    text: "Anular este convite"
                                    tipo: "secundario"
                                    onClicked: tela.cancelarConvite(tela.conviteGerado.id)
                                }
                            }
                            Image {
                                // C2R2-8: o nome só quando a imagem carregou (o plugin SVG do Qt existe).
                                objectName: status === Image.Ready ? "qrDoConvite" : "qrDoConviteSemImagem"
                                source: tela.conviteGerado ? tela.conviteGerado.qr : ""
                                sourceSize.width: 180
                                sourceSize.height: 180
                                Layout.preferredWidth: 180
                                Layout.preferredHeight: 180
                                Layout.alignment: Qt.AlignTop
                                fillMode: Image.PreserveAspectFit
                                smooth: false
                            }
                        }
                    }
                    // Os convites ativos (vistos com a senha do gerente), com Cancelar.
                    Repeater {
                        model: tela.convitesVistos ? tela.convitesAtivos : []
                        delegate: Rectangle {
                            id: linhaConvite
                            required property var modelData
                            required property int index
                            Layout.fillWidth: true
                            readonly property bool par: (index + 1) % 2 === 0
                            color: Tema.faixa(linhaConvite.par, Tema.vela(0.4), "transparent")
                            palette.window: Tema.superficie(color)
                            radius: Tema.raioCampo
                            implicitHeight: conteudoLinhaConvite.implicitHeight + 10
                            RowLayout {
                                id: conteudoLinhaConvite
                                x: 10
                                y: 5
                                width: linhaConvite.width - 20
                                spacing: 10
                                Text {
                                    text: linhaConvite.modelData.nome + " · código " + linhaConvite.modelData.codigoPedido
                                          + " · vale até " + linhaConvite.modelData.expiraEmTexto + " · gerado por " + linhaConvite.modelData.gerente
                                    textFormat: Text.PlainText
                                    color: Tema.tintaDaFaixa(linhaConvite.par)
                                    font.family: Tema.texto
                                    font.pixelSize: 14
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                                Botao {
                                    text: "Anular"
                                    tipo: "discreto"
                                    onClicked: tela.cancelarConvite(linhaConvite.modelData.id)
                                }
                            }
                        }
                    }

                    // Caixa ainda não aprovado: o código dele e o convite que o gerente gerou.
                    Linha {
                        visible: !!faixaSeguranca.p4 && !faixaSeguranca.p4.aprovado
                        spacing: 12
                        Layout.fillWidth: true
                        Campo {
                            objectName: "codigoDesteCaixa"
                            visible: tela.codigoDesteCaixa !== ""
                            rotulo: "Código deste caixa (mostre ao gerente)"
                            texto: tela.codigoDesteCaixa
                            somenteLeitura: true
                            tamanhoFonte: 20
                            altura: 46
                            Layout.fillWidth: true
                        }
                        Botao {
                            visible: tela.codigoDesteCaixa === "" && !(faixaSeguranca.p4 && faixaSeguranca.p4.retirado)
                            text: "Mostrar o código deste caixa"
                            tipo: "secundario"
                            onClicked: tela.mostrarCodigoDesteCaixa()
                        }
                    }
                    Nota {
                        visible: tela.identidadeIlegivel !== ""
                        text: "A identidade deste computador não abriu: " + tela.identidadeIlegivel
                              + " Se o cofre de senhas voltar, tente de novo; se não voltar, gere uma identidade nova."
                        color: Tema.vermelho
                        Layout.fillWidth: true
                    }
                    Botao {
                        visible: tela.identidadeIlegivel !== "" || (!!faixaSeguranca.p4 && faixaSeguranca.p4.retirado)
                        text: "Gerar identidade nova"
                        tipo: "secundario"
                        onClicked: tela.gerarIdentidadeNova()
                    }
                    Linha {
                        visible: !!faixaSeguranca.p4 && !faixaSeguranca.p4.aprovado
                        spacing: 12
                        Layout.fillWidth: true
                        Campo {
                            id: conviteRecebido
                            objectName: "conviteRecebido"
                            rotulo: "Convite (aparece na tela do gerente)"
                            dica: "Ex.: ABCDE-FGHJK-LMNPQ-RSTUV-WXYZ2-3"
                            podeColar: true
                            camera: true
                            maximo: 400
                            Layout.fillWidth: true
                            onConfirmado: tela.usarConvite()
                        }
                        Campo {
                            id: enderecoDaLoja
                            rotulo: "Endereço do caixa do gerente (só se não achar sozinho)"
                            dica: "Ex.: 192.168.0.12"
                            maximo: 300
                            Layout.fillWidth: true
                            onConfirmado: tela.usarConvite()
                        }
                    }
                    Botao {
                        visible: !!faixaSeguranca.p4 && !faixaSeguranca.p4.aprovado
                        text: "Usar o convite"
                        onClicked: tela.usarConvite()
                    }

                    // Os caixas antigos (antes da 0.9.19): recusados, ou liberados por alguns dias (A4).
                    Nota {
                        visible: !!faixaSeguranca.p4 && faixaSeguranca.p4.temRaiz
                        text: !faixaSeguranca.p4 ? ""
                              : faixaSeguranca.p4.convivenciaAte
                                ? "Caixas antigos (antes da versão 0.9.19) liberados neste caixa até " + faixaSeguranca.p4.convivenciaAteTexto
                                  + ": eles trocam dados com este caixa, e os dados chegam aos outros caixas por ele. Depois, só os caixas atualizados."
                                : "Caixas antigos (antes da versão 0.9.19) não trocam dados com este caixa. Se precisar de um tempo para "
                                  + "atualizá-los, libere-os aqui por alguns dias (no máximo " + faixaSeguranca.p4.convivenciaMaximaDias
                                  + "): este caixa serve de ponte entre eles e os caixas atualizados."
                        Layout.fillWidth: true
                    }
                    Row {
                        visible: !!faixaSeguranca.p4 && faixaSeguranca.p4.temRaiz
                        spacing: 12
                        Campo {
                            id: diasAntigos
                            objectName: "diasCaixasAntigos"
                            visible: !faixaSeguranca.p4 || !faixaSeguranca.p4.convivenciaAte
                            rotulo: "Por quantos dias"
                            texto: "7"
                            numerico: true
                            maximo: 2
                            width: 160
                        }
                        Botao {
                            visible: !faixaSeguranca.p4 || !faixaSeguranca.p4.convivenciaAte
                            anchors.bottom: diasAntigos.bottom
                            text: "Liberar os caixas antigos"
                            tipo: "secundario"
                            onClicked: tela.liberarCaixasAntigos(parseInt(diasAntigos.texto, 10) || 0)
                        }
                        Botao {
                            visible: !!faixaSeguranca.p4 && !!faixaSeguranca.p4.convivenciaAte
                            text: "Recusar os caixas antigos agora"
                            tipo: "secundario"
                            onClicked: tela.liberarCaixasAntigos(null)
                        }
                    }
                }

                // ---------------- Caixas da loja: versão de cada um (0.9.15) ----------------
                Faixa {
                    visible: tela.secao === "caixas" && tela.caixasConhecidos.length > 0
                    titulo: "Versão dos caixas"
                    subtitulo: "rascunho de OS novo só quando todos confirmam a versão nos últimos " + tela.prazoCompatibilidade + " dias"
                    color: Tema.pergaminho
                    Layout.fillWidth: true
                    Repeater {
                        id: versaoDosCaixas
                        model: tela.caixasConhecidos
                        delegate: Rectangle {
                            id: linhaCaixa
                            required property var modelData
                            Layout.fillWidth: true
                            required property int index
                            // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                            readonly property bool par: (index + 1) % 2 === 0
                            color: Tema.faixa(linhaCaixa.par, Tema.vela(0.4), "transparent")
                            palette.window: Tema.superficie(color)
                            radius: Tema.raioCampo
                            implicitHeight: conteudoDaFaixa3.implicitHeight + 10
                            implicitWidth: conteudoDaFaixa3.implicitWidth + 20
                            RowLayout {
                                id: conteudoDaFaixa3
                                x: 10
                                y: 5
                                width: linhaCaixa.width - 20
                                spacing: 10
                                Rectangle {
                                    Layout.preferredWidth: 10
                                    Layout.preferredHeight: 10
                                    Layout.alignment: Qt.AlignVCenter
                                    radius: 5
                                    color: linhaCaixa.modelData.situacao === "compativel" ? Tema.verdeEscuroDaFaixa(linhaCaixa.par)
                                           : linhaCaixa.modelData.situacao === "retirado" ? Tema.apagadoDaFaixa(linhaCaixa.par) : Tema.vermelhoDaFaixa(linhaCaixa.par)
                                }
                                ColumnLayout {
                                    spacing: 1
                                    Layout.fillWidth: true
                                    Text {
                                        text: linhaCaixa.modelData.nome
                                        textFormat: Text.PlainText
                                        color: Tema.tintaDaFaixa(linhaCaixa.par)
                                        font.family: Tema.texto
                                        font.weight: Font.DemiBold
                                        font.pixelSize: 15
                                        Layout.fillWidth: true
                                    }
                                    Text {
                                        text: linhaCaixa.modelData.texto
                                              + (linhaCaixa.modelData.compativelEmTexto ? " Última confirmação: " + linhaCaixa.modelData.compativelEmTexto + "." : "")
                                              + (linhaCaixa.modelData.naoEnviadas > 0 ? " A troca de dados com ele parou: " + linhaCaixa.modelData.naoEnviadas
                                                 + " alteração(ões) esperam a atualização dele." : "")
                                              + (linhaCaixa.modelData.situacao === "retirado" ? " Motivo: " + linhaCaixa.modelData.retiradoMotivo
                                                 + (linhaCaixa.modelData.retiradoPor ? " (" + linhaCaixa.modelData.retiradoPor + ")" : "") + "." : "")
                                              + (linhaCaixa.modelData.retiradoPelaChave ? " Retirado pela chave: para voltar, só com uma aprovação nova do gerente." : "")
                                              // C2R3-4: aprovado de novo (chave nova), ainda com o bloqueio antigo.
                                              + (linhaCaixa.modelData.situacao === "retirado" && linhaCaixa.modelData.temChave
                                                 ? " Aprovado de novo pelo convite, mas ainda com o bloqueio antigo: toque em Readmitir para ele voltar a trocar dados." : "")
                                              + (linhaCaixa.modelData.raiz ? " É o primeiro caixa da loja: ele aprova os outros e não pode ser retirado." : "")
                                        textFormat: Text.PlainText
                                        color: linhaCaixa.modelData.bloqueia ? Tema.vermelhoDaFaixa(linhaCaixa.par) : Tema.tintaSuaveDaFaixa(linhaCaixa.par)
                                        font.family: Tema.texto
                                        font.pixelSize: 13
                                        wrapMode: Text.WordWrap
                                        Layout.fillWidth: true
                                    }
                                    // C2R2-5: duas identidades aprovadas (o registro ambíguo não troca dados).
                                    Text {
                                        visible: (linhaCaixa.modelData.identidades || []).length > 1
                                        text: "Este caixa tem duas identidades aprovadas (aprovações feitas ao mesmo tempo em caixas "
                                              + "diferentes) e não troca dados até sobrar uma. Confira no próprio caixa o código dele "
                                              + "(Configurações › Caixas da loja) e retire a outra."
                                        textFormat: Text.PlainText
                                        color: Tema.vermelhoDaFaixa(linhaCaixa.par)
                                        font.family: Tema.texto
                                        font.pixelSize: 13
                                        wrapMode: Text.WordWrap
                                        Layout.fillWidth: true
                                    }
                                    Flow {
                                        visible: (linhaCaixa.modelData.identidades || []).length > 1
                                        spacing: 8
                                        Layout.fillWidth: true
                                        Repeater {
                                            model: linhaCaixa.modelData.identidades || []
                                            delegate: Botao {
                                                required property var modelData
                                                text: "Retirar a identidade " + modelData.codigo
                                                tipo: "discreto"
                                                onClicked: tela.retirarIdentidade(linhaCaixa.modelData, modelData)
                                            }
                                        }
                                    }
                                }
                                Botao {
                                    visible: linhaCaixa.modelData.situacao !== "retirado" && !linhaCaixa.modelData.raiz
                                    text: "Retirar da loja"
                                    tipo: "discreto"
                                    onClicked: tela.retirarCaixa(linhaCaixa.modelData)
                                }
                                Botao {
                                    // C2R5-1: só no primeiro caixa, com o registro de caixas cheio.
                                    visible: (linhaCaixa.modelData.liberarLugar || []).length > 0
                                    text: "Liberar o lugar"
                                    tipo: "secundario"
                                    onClicked: tela.liberarLugar(linhaCaixa.modelData)
                                }
                                Botao {
                                    visible: linhaCaixa.modelData.situacao === "retirado" && !linhaCaixa.modelData.retiradoPelaChave
                                    text: "Readmitir"
                                    tipo: "secundario"
                                    onClicked: tela.readmitirCaixa(linhaCaixa.modelData)
                                }
                            }
                        }
                    }
                }

                // ---------------- Caixas da loja: pendências da troca (0.9.15) ----------------
                Faixa {
                    visible: tela.secao === "caixas"
                    titulo: "Pendências da troca de dados"
                    subtitulo: "alterações guardadas inteiras, sem aplicar"
                    color: Tema.azul
                    corTitulo: Tema.azulEscuro
                    Layout.fillWidth: true
                    Nota {
                        visible: !tela.verSimultaneidade
                        text: "Só o gerente vê as pendências."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                    Botao {
                        visible: !tela.verSimultaneidade
                        text: "Ver pendências"
                        tipo: "secundario"
                        onClicked: janela.comGerente("simultaneidade", () => {
                            tela.verSimultaneidade = true
                            tela.carregarCaixas()
                        })
                    }
                    Nota {
                        visible: tela.verSimultaneidade && tela.pendencias.length === 0
                        text: "Nenhuma pendência: tudo o que chegou dos outros caixas foi aplicado."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                    Repeater {
                        id: listaPendencias
                        model: tela.verSimultaneidade ? tela.pendencias : []
                        delegate: Rectangle {
                            id: linhaPend
                            required property var modelData
                            Layout.fillWidth: true
                            required property int index
                            // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                            readonly property bool par: (index + 1) % 2 === 0
                            color: Tema.faixa(linhaPend.par, Tema.vela(0.4), "transparent")
                            palette.window: Tema.superficie(color)
                            radius: Tema.raioCampo
                            implicitHeight: conteudoDaFaixa4.implicitHeight + 10
                            implicitWidth: conteudoDaFaixa4.implicitWidth + 20
                            RowLayout {
                                id: conteudoDaFaixa4
                                x: 10
                                y: 5
                                width: linhaPend.width - 20
                                spacing: 10
                                Text {
                                    // H10 (7.2): os dados vão escapados (o texto é formatado).
                                    text: "<b>" + Tema.escapar(linhaPend.modelData.textoMotivo) + "</b> · " + Tema.escapar(linhaPend.modelData.oQue) + " · "
                                          + (linhaPend.modelData.lado === "nao_enviada" ? "não vai para " : "veio de ") + Tema.escapar(linhaPend.modelData.caixa)
                                          + (linhaPend.modelData.detalhe ? " · " + Tema.escapar(linhaPend.modelData.detalhe) : "")
                                          + (linhaPend.modelData.ultimoErro ? " · " + Tema.escapar(linhaPend.modelData.ultimoErro) : "")
                                          + (linhaPend.modelData.estado === "arquivada_manual"
                                             ? " · arquivada por " + Tema.escapar(linhaPend.modelData.arquivadaPor) + ": " + Tema.escapar(linhaPend.modelData.arquivadaMotivo) : "")
                                    textFormat: Text.StyledText
                                    color: linhaPend.modelData.estado === "arquivada_manual" ? Tema.tintaSuaveDaFaixa(linhaPend.par) : Tema.tintaDaFaixa(linhaPend.par)
                                    font.family: Tema.texto
                                    font.pixelSize: 13
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                                Botao {
                                    visible: linhaPend.modelData.podeArquivar
                                    text: "Arquivar"
                                    tipo: "discreto"
                                    onClicked: tela.arquivarPendencia(linhaPend.modelData)
                                }
                            }
                        }
                    }
                    Botao {
                        visible: tela.verSimultaneidade && tela.pendencias.some(p => p.estado === "pendente" && p.lado === "recebida")
                        text: "Tentar aplicar de novo"
                        tipo: "secundario"
                        onClicked: tela.reaplicarPendencias()
                    }
                }

                // ---------------- Gerente ----------------
                Faixa {
                    visible: tela.secao === "gerente"
                    titulo: "Quem é gerente"
                    subtitulo: "os outros usuários são caixa"
                    color: Tema.pergaminho
                    Layout.fillWidth: true
                    Repeater {
                        model: tela.gerente ? tela.gerente.usuarios : []
                        delegate: Marcar {
                            required property var modelData
                            required property int index
                            text: modelData.nome
                            checked: modelData.gerente
                            onToggled: {
                                tela.gerente.usuarios[index].gerente = checked
                                tela.alterado = true
                            }
                        }
                    }
                    Nota {
                        text: "A loja precisa de pelo menos um gerente. Quando o caixa precisar de algo do gerente, o gerente digita a senha na hora, sem trocar de usuário."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                }
                Faixa {
                    visible: tela.secao === "gerente"
                    titulo: "Só o gerente pode"
                    subtitulo: "desmarque o que o caixa também pode fazer"
                    color: Tema.azul
                    Layout.fillWidth: true
                    Repeater {
                        model: tela.gerente ? tela.gerente.acoes : []
                        delegate: Marcar {
                            required property var modelData
                            required property int index
                            text: modelData.nome
                            checked: modelData.soGerente
                            onToggled: {
                                tela.gerente.acoes[index].soGerente = checked
                                tela.alterado = true
                            }
                        }
                    }
                }
                Faixa {
                    visible: tela.secao === "gerente"
                    titulo: "Senha do gerente"
                    color: Tema.verde
                    corTitulo: Tema.verdeEscuro
                    Layout.fillWidth: true
                    Nota {
                        text: "Cada gerente troca a própria senha em “Minha senha”. A senha de gerente precisa ter pelo menos 8 caracteres; como ela também protege os backups, quanto mais longa, melhor."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                    Botao {
                        text: "Trocar minha senha"
                        tipo: "secundario"
                        onClicked: {
                            tela.secao = "loja"
                            atual.focar()
                        }
                    }
                }

                Faixa {
                    visible: tela.secao === "duplicatas"
                    titulo: "Diferenças entre caixas"
                    subtitulo: tela.conflitos.filter(c => !c.resolvido).length > 0
                               ? tela.conflitos.filter(c => !c.resolvido).length + " para ver"
                               : "nenhuma para ver"
                    color: tela.conflitos.some(c => !c.resolvido) ? Tema.rosa : Tema.pergaminho
                    Layout.fillWidth: true
                    Nota {
                        text: "Quando dois caixas mexem na mesma coisa sem ver o do outro (ou não dá para saber qual veio depois), "
                              + "uma fica valendo e a outra aparece aqui, guardada. Escolha a que vale; a escolha vai para os outros caixas."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                    Botao {
                        visible: tela.conflitos.filter(c => !c.resolvido).length >= 3
                        text: "Deixar todas como estão"
                        tipo: "secundario"
                        onClicked: janela.chamarGerente("conflitos_manter_todos", {}, r => {
                            if (r.ok) {
                                tela.conflitos = r.dados
                                janela.avisar("Ficaram como estão. As versões que perderam continuam guardadas na lista.", "ok")
                            } else {
                                janela.avisar(r.erro, "erro")
                            }
                        })
                    }
                    Repeater {
                        id: listaConflitos
                        model: tela.conflitos
                        delegate: Rectangle {
                            id: faixaDaLista5
                            required property int index
                            required property var modelData
                            // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                            readonly property bool par: (index + 1) % 2 === 0
                            color: Tema.faixa(faixaDaLista5.par, modelData.resolvido ? "transparent" : Tema.papel, modelData.resolvido ? Tema.vela(0.25) : Tema.vela(0.8))
                            palette.window: Tema.superficie(color)
                            radius: Tema.raioCampo
                            Layout.fillWidth: true
                            implicitHeight: colunaConflito.implicitHeight + 24
                            ColumnLayout {
                                id: colunaConflito
                                x: 14; y: 12
                                width: parent.width - 28
                                spacing: 6
                                Text {
                                    text: modelData.oQue + " · " + modelData.quando + (modelData.caixa ? " · " + modelData.caixa : "")
                                    textFormat: Text.PlainText
                                    color: Tema.tintaDaFaixa(faixaDaLista5.par)
                                    font.family: Tema.texto
                                    font.weight: Font.DemiBold
                                    font.pixelSize: 15
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                                Text {
                                    text: modelData.texto
                                    textFormat: Text.PlainText
                                    color: Tema.tintaDaFaixa(faixaDaLista5.par)
                                    font.family: Tema.texto
                                    font.pixelSize: 14
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                                // 9b (C8): o que aconteceu com este registro (só consulta).
                                Botao {
                                    text: "Ver no histórico"
                                    tipo: "discreto"
                                    altura: 32
                                    onClicked: tela.verHistoricoDe(modelData)
                                }
                                Repeater {
                                    model: modelData.campos
                                    delegate: Text {
                                        required property var modelData
                                        text: modelData.nome + ": ficou " + modelData.ficou + " · a outra versão tinha " + modelData.perdeu
                                        textFormat: Text.PlainText
                                        color: Tema.tintaSuaveDaFaixa(faixaDaLista5.par)
                                        font.family: Tema.texto
                                        font.pixelSize: 14
                                        wrapMode: Text.WordWrap
                                        Layout.fillWidth: true
                                    }
                                }
                                Text {
                                    visible: !!modelData.resolvido
                                    text: "Resolvido: " + modelData.resolvido
                                    textFormat: Text.PlainText
                                    color: Tema.verdeEscuroDaFaixa(faixaDaLista5.par)
                                    font.family: Tema.texto
                                    font.pixelSize: 14
                                    Layout.fillWidth: true
                                }
                                Text {
                                    // R15: a compensação pronta, à vista antes do clique.
                                    visible: !modelData.resolvido && !!modelData.compensacao
                                    text: "Compensação pronta: " + modelData.compensacao + "."
                                    textFormat: Text.PlainText
                                    color: Tema.tintaDaFaixa(faixaDaLista5.par)
                                    font.family: Tema.texto
                                    font.pixelSize: 14
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                                Linha {
                                    visible: !modelData.resolvido
                                    spacing: 12
                                    Layout.fillWidth: true
                                    Botao {
                                        visible: modelData.podeUsar
                                        text: "Usar a do outro caixa"
                                        tipo: "secundario"
                                        onClicked: tela.resolverConflito(modelData, true)
                                    }
                                    // R15: a compensação pronta (a conta paga a mais), quando há uma exata.
                                    Botao {
                                        visible: !!modelData.compensacao
                                        text: "Fazer a compensação"
                                        tipo: "principal"
                                        onClicked: tela.compensarConflito(modelData)
                                    }
                                    Botao {
                                        text: modelData.podeUsar ? "Deixar como está" : "Entendi"
                                        tipo: "secundario"
                                        onClicked: tela.resolverConflito(modelData, false)
                                    }
                                }
                            }
                        }
                    }
                }

                Faixa {
                    visible: tela.secao === "duplicatas" && !!tela.codigos
                    titulo: "Códigos das peças"
                    subtitulo: tela.codigosParaVer > 0 ? tela.codigosParaVer + " para ver" : "nenhum repetido"
                    color: tela.codigosParaVer > 0 ? Tema.rosa : Tema.pergaminho
                    Layout.fillWidth: true
                    Nota {
                        text: "Cada caixa dá códigos só da faixa de números dele, e as faixas são dadas por um caixa só"
                              + (tela.codigos && tela.codigos.distribuidor ? " (" + tela.codigos.distribuidor.nome + ")" : "")
                              + ". Assim dois caixas nunca dão o mesmo código. Nenhum código que já existe é trocado sozinho."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                    // Banco copiado de outro computador (ou computador não confirmado).
                    Text {
                        visible: !!tela.codigos && !!tela.codigos.bloqueio
                        text: tela.codigos && tela.codigos.bloqueio ? tela.codigos.bloqueio.texto : ""
                        textFormat: Text.PlainText
                        color: Tema.vermelho
                        font.family: Tema.texto
                        font.weight: Font.DemiBold
                        font.pixelSize: 15
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                    Botao {
                        visible: !!tela.codigos && !!tela.codigos.bloqueio && tela.codigos.bloqueio.podeConfirmar
                        text: "Este é o computador de sempre deste caixa"
                        onClicked: tela.comandoCodigos("codigos_confirmar_computador", {},
                                                       "Computador confirmado. Este caixa volta a dar códigos.")
                    }
                    Botao {
                        visible: !!tela.codigos && !!tela.codigos.bloqueio && tela.codigos.bloqueio.podeRegistrar && !tela.confirmarCaixaNovo
                        text: "Registrar como caixa novo"
                        tipo: "secundario"
                        onClicked: tela.confirmarCaixaNovo = true
                    }
                    Nota {
                        visible: tela.confirmarCaixaNovo
                        text: "O caixa de onde veio esta cópia continua com as faixas dele. Este vira um caixa novo da loja: ganha identificador novo "
                              + "e pede uma faixa de códigos nova ao caixa que distribui (na próxima troca de dados). Até lá, as peças novas ficam com código a definir. "
                              + "Se o caixa de antes não existe mais, depois use “Este caixa passa a distribuir as faixas”."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                    Campo {
                        id: nomeCaixaNovo
                        visible: tela.confirmarCaixaNovo
                        rotulo: "Nome deste caixa (diferente do caixa de onde veio a cópia)"
                        maximo: 30
                        Layout.preferredWidth: 320
                    }
                    Linha {
                        visible: tela.confirmarCaixaNovo
                        spacing: 12
                        Botao {
                            text: "Registrar"
                            onClicked: tela.comandoCodigos("codigos_registrar_caixa_novo", { nome: nomeCaixaNovo.texto },
                                                           "Este banco foi registrado como caixa novo.")
                        }
                    }
                    Text {
                        text: {
                            const c = tela.codigos
                            if (!c)
                                return ""
                            const livres = c.minhas.filter(f => f.restantes > 0)
                            if (livres.length === 0)
                                return c.distribuidor
                                       ? "Este caixa ainda não tem faixa (ou a dele acabou): as peças novas ficam com código a definir até a troca de dados com " + c.distribuidor.nome + "."
                                       : "Ainda não há caixa distribuindo as faixas: as peças novas ficam com código a definir."
                            return "Este caixa: " + livres.map(f => tela.milhar(f.inicio) + " a " + tela.milhar(f.fim)).join(", ")
                                   + " · próximo código " + tela.milhar(livres[0].proximo) + " · restam " + tela.milhar(c.restantes)
                        }
                        textFormat: Text.PlainText
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.pixelSize: 14
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                    Text {
                        visible: !!tela.codigos && tela.codigos.pendentes > 0
                        text: tela.codigos ? tela.codigos.pendentes + (tela.codigos.pendentes === 1 ? " peça está" : " peças estão")
                                             + " com código a definir (" + tela.codigos.pendentesDeste + " deste caixa). Cada caixa dá o código das dele quando recebe a faixa." : ""
                        textFormat: Text.PlainText
                        color: Tema.tintaSuave
                        font.family: Tema.texto
                        font.pixelSize: 14
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                    Botao {
                        visible: !!tela.codigos && (tela.codigos.pendentes > tela.codigos.pendentesDeste || tela.codigos.pessoasPendentes > 0) && tela.codigos.restantes > 0
                        text: "Dar código deste caixa a todos os cadastros a definir"
                        tipo: "secundario"
                        onClicked: tela.comandoCodigos("codigos_definir_pendentes", {}, "Os cadastros a definir receberam código deste caixa.")
                    }
                    // Códigos com letras escritos à mão: só valem depois de confirmados
                    // pelo caixa que distribui as faixas.
                    Nota {
                        visible: !!tela.codigos && tela.codigos.esperando.length > 0
                        text: "Códigos com letras esperando a confirmação do caixa que distribui as faixas. Enquanto esperam, a peça vende e é achada pelo nome, "
                              + "mas a etiqueta não sai e o leitor não acha a peça por esse código. Se outra peça já tem o código, só uma fica com ele: "
                              + "dê um código novo à que ficou esperando."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                    Repeater {
                        model: tela.codigos ? tela.codigos.esperando : []
                        delegate: Rectangle {
                            id: faixaDaLista6
                            required property var modelData
                            Layout.fillWidth: true
                            required property int index
                            // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                            readonly property bool par: (index + 1) % 2 === 0
                            color: Tema.faixa(faixaDaLista6.par, Tema.vela(0.4), "transparent")
                            palette.window: Tema.superficie(color)
                            radius: Tema.raioCampo
                            implicitHeight: conteudoDaFaixa6.implicitHeight + 10
                            implicitWidth: conteudoDaFaixa6.implicitWidth + 20
                            RowLayout {
                                id: conteudoDaFaixa6
                                x: 10
                                y: 5
                                width: faixaDaLista6.width - 20
                                spacing: 10
                                Text {
                                    text: modelData.nome + " · código " + modelData.codigo + " (" + modelData.caixa + ")"
                                          + (modelData.conflito ? " · já é da peça “" + modelData.outraPeca + "”" : " · esperando a confirmação")
                                    textFormat: Text.PlainText
                                    color: modelData.conflito ? Tema.vermelhoDaFaixa(faixaDaLista6.par) : Tema.tintaDaFaixa(faixaDaLista6.par)
                                    font.family: Tema.texto
                                    font.pixelSize: 14
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                                Botao {
                                    text: "Abrir"
                                    tipo: "discreto"
                                    onClicked: tela.janela.abrir("TelaPeca", { produtoId: modelData.id })
                                }
                                Botao {
                                    visible: modelData.conflito
                                    text: "Dar código novo"
                                    tipo: "secundario"
                                    onClicked: tela.comandoCodigos("codigos_dar_novo", { id: modelData.id },
                                                                   "“" + modelData.nome + "” ganhou um código novo. Imprima a etiqueta nova.")
                                }
                            }
                        }
                    }
                    // Faixas de dois caixas que se cruzam (lojas juntadas, dois distribuidores).
                    Repeater {
                        model: tela.codigos ? tela.codigos.cruzadas : []
                        delegate: Text {
                            required property var modelData
                            text: "Os códigos de " + tela.milhar(modelData.inicio) + " a " + tela.milhar(modelData.fim) + " são de dois caixas ("
                                  + modelData.caixas.join(" e ") + "). Nenhum caixa dá código novo nesse pedaço; as peças que já têm código dele ficam como estão."
                            textFormat: Text.PlainText
                            color: Tema.vermelho
                            font.family: Tema.texto
                            font.pixelSize: 14
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                    }
                    Nota {
                        visible: !!tela.codigos && tela.codigos.repetidos.length > 0
                        text: "Códigos que levam a mais de uma peça. O leitor e a busca perguntam qual é, e a etiqueta dessas peças não sai. "
                              + "Para resolver, dê um código novo a uma delas: o código de antes fica guardado (a etiqueta velha ainda acha a peça e o leitor pergunta qual é). "
                              + "Troque também a etiqueta na prateleira."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                    Repeater {
                        id: listaRepetidos
                        model: tela.codigos ? tela.codigos.repetidos.slice(0, 50) : []
                        delegate: Rectangle {
                            required property int index
                            id: grupo
                            required property var modelData
                            // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                            readonly property bool par: (index + 1) % 2 === 0
                            color: Tema.faixa(grupo.par, Tema.papel, Tema.vela(0.6))
                            palette.window: Tema.superficie(color)
                            radius: Tema.raioCampo
                            Layout.fillWidth: true
                            implicitHeight: colunaGrupo.implicitHeight + 20
                            ColumnLayout {
                                id: colunaGrupo
                                x: 14; y: 10
                                width: parent.width - 28
                                spacing: 4
                                Text {
                                    text: (grupo.modelData.tipo === "barras" ? "Código de barras " : "Código ") + grupo.modelData.codigo
                                    textFormat: Text.PlainText
                                    color: Tema.tintaDaFaixa(grupo.par)
                                    font.family: Tema.texto
                                    font.weight: Font.DemiBold
                                    font.pixelSize: 15
                                    Layout.fillWidth: true
                                }
                                Repeater {
                                    model: grupo.modelData.pecas
                                    delegate: RowLayout {
                                        required property var modelData
                                        spacing: 10
                                        Layout.fillWidth: true
                                        Text {
                                            text: modelData.nome + (modelData.antigo ? " (era o código dela; agora é " + modelData.codigo + ")" : "")
                                            textFormat: Text.PlainText
                                            color: Tema.tintaDaFaixa(grupo.par)
                                            font.family: Tema.texto
                                            font.pixelSize: 14
                                            wrapMode: Text.WordWrap
                                            Layout.fillWidth: true
                                        }
                                        Botao {
                                            text: "Abrir"
                                            tipo: "discreto"
                                            onClicked: tela.janela.abrir("TelaPeca", { produtoId: modelData.id })
                                        }
                                        Botao {
                                            visible: grupo.modelData.tipo === "interno" && !modelData.antigo
                                            text: "Dar código novo"
                                            tipo: "secundario"
                                            onClicked: tela.comandoCodigos("codigos_dar_novo", { id: modelData.id },
                                                                           "“" + modelData.nome + "” ganhou um código novo. Imprima a etiqueta nova.")
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // Clientes e fornecedores: o código sai da faixa do caixa, como o das peças.
                    Text {
                        visible: !!tela.codigos && tela.codigos.pessoasPendentes > 0
                        text: tela.codigos ? tela.codigos.pessoasPendentes + (tela.codigos.pessoasPendentes === 1 ? " cliente ou fornecedor está" : " clientes ou fornecedores estão")
                                             + " com código a definir. Cada caixa dá o código dos dele quando recebe a faixa." : ""
                        textFormat: Text.PlainText
                        color: Tema.tintaSuave
                        font.family: Tema.texto
                        font.pixelSize: 14
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                    // Clientes e fornecedores com código com letras: o mesmo princípio das peças.
                    Nota {
                        visible: !!tela.codigos && tela.codigos.pessoasEsperando.length > 0
                        text: "Clientes e fornecedores com código com letras esperando a confirmação do caixa que distribui as faixas. Enquanto esperam, "
                              + "o cadastro é achado pelo nome e funciona normalmente, mas o código ainda não vale como único. Se outro cadastro já tem o código, "
                              + "só um fica com ele: dê um código novo ao que ficou esperando. Nada é trocado sozinho."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                    Repeater {
                        model: tela.codigos ? tela.codigos.pessoasEsperando : []
                        delegate: Rectangle {
                            id: faixaDaLista7
                            required property var modelData
                            Layout.fillWidth: true
                            required property int index
                            // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                            readonly property bool par: (index + 1) % 2 === 0
                            color: Tema.faixa(faixaDaLista7.par, Tema.vela(0.4), "transparent")
                            palette.window: Tema.superficie(color)
                            radius: Tema.raioCampo
                            implicitHeight: conteudoDaFaixa7.implicitHeight + 10
                            implicitWidth: conteudoDaFaixa7.implicitWidth + 20
                            RowLayout {
                                id: conteudoDaFaixa7
                                x: 10
                                y: 5
                                width: faixaDaLista7.width - 20
                                spacing: 10
                                Text {
                                    text: modelData.nome + " · código " + modelData.codigo + " (" + modelData.caixa + ")"
                                          + (modelData.conflito ? " · já é de “" + modelData.outraPessoa + "”" : " · esperando a confirmação")
                                    textFormat: Text.PlainText
                                    color: modelData.conflito ? Tema.vermelhoDaFaixa(faixaDaLista7.par) : Tema.tintaDaFaixa(faixaDaLista7.par)
                                    font.family: Tema.texto
                                    font.pixelSize: 14
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                                Botao {
                                    text: "Abrir"
                                    tipo: "discreto"
                                    onClicked: tela.janela.abrir("TelaPessoa", { pessoaId: modelData.id, papel: "cliente" })
                                }
                                Botao {
                                    visible: modelData.conflito
                                    text: "Dar código novo"
                                    tipo: "secundario"
                                    onClicked: tela.comandoCodigos("codigos_pessoa_dar_novo", { id: modelData.id },
                                                                   "“" + modelData.nome + "” ganhou um código novo.")
                                }
                            }
                        }
                    }
                    Nota {
                        visible: !!tela.codigos && tela.codigos.pessoasRepetidas.length > 0
                        text: "Códigos de clientes ou fornecedores que levam a mais de uma pessoa. Nada foi trocado: as vendas, os débitos e as notas "
                              + "usam o cadastro, não o código. Para resolver, dê um código novo a uma delas."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                    Repeater {
                        model: tela.codigos ? tela.codigos.pessoasRepetidas.slice(0, 50) : []
                        delegate: Rectangle {
                            required property int index
                            id: grupoPessoas
                            required property var modelData
                            // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                            readonly property bool par: (index + 1) % 2 === 0
                            color: Tema.faixa(grupoPessoas.par, Tema.papel, Tema.vela(0.6))
                            palette.window: Tema.superficie(color)
                            radius: Tema.raioCampo
                            Layout.fillWidth: true
                            implicitHeight: colunaPessoas.implicitHeight + 20
                            ColumnLayout {
                                id: colunaPessoas
                                x: 14; y: 10
                                width: parent.width - 28
                                spacing: 4
                                Text {
                                    text: "Código " + grupoPessoas.modelData.codigo
                                    textFormat: Text.PlainText
                                    color: Tema.tintaDaFaixa(grupoPessoas.par)
                                    font.family: Tema.texto
                                    font.weight: Font.DemiBold
                                    font.pixelSize: 15
                                    Layout.fillWidth: true
                                }
                                Repeater {
                                    model: grupoPessoas.modelData.pessoas
                                    delegate: RowLayout {
                                        required property var modelData
                                        spacing: 10
                                        Layout.fillWidth: true
                                        Text {
                                            text: modelData.nome + " · " + modelData.papel + " (" + modelData.caixa + ")"
                                            textFormat: Text.PlainText
                                            color: Tema.tintaDaFaixa(grupoPessoas.par)
                                            font.family: Tema.texto
                                            font.pixelSize: 14
                                            wrapMode: Text.WordWrap
                                            Layout.fillWidth: true
                                        }
                                        Botao {
                                            text: "Abrir"
                                            tipo: "discreto"
                                            onClicked: tela.janela.abrir("TelaPessoa", { pessoaId: modelData.id, papel: modelData.papel === "fornecedor" ? "fornecedor" : "cliente" })
                                        }
                                        Botao {
                                            text: "Dar código novo"
                                            tipo: "secundario"
                                            onClicked: tela.comandoCodigos("codigos_pessoa_dar_novo", { id: modelData.id },
                                                                           "“" + modelData.nome + "” ganhou um código novo.")
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Text {
                        visible: !!tela.codigos && !!tela.codigos.inventario
                        text: {
                            const i = tela.codigos ? tela.codigos.inventario : null
                            if (!i)
                                return ""
                            return "Quando as faixas começaram: " + tela.milhar(i.pecas) + " peças; maior código de números " + tela.milhar(i.maior)
                                   + "; " + i.acimaDoLimite + " acima de 99.999; " + i.comLetras + " com letras; "
                                   + i.foraDoPadrao + " com zero na frente ou muito longos; " + i.repetidos + " códigos repetidos ("
                                   + i.pecasRepetidas + " peças). Nada foi renumerado."
                        }
                        textFormat: Text.PlainText
                        color: Tema.tintaSuave
                        font.family: Tema.texto
                        font.pixelSize: 13
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                    Botao {
                        visible: !!tela.codigos && !(tela.codigos.distribuidor && tela.codigos.distribuidor.esteCaixa) && !tela.confirmarAssumir
                        text: "Este caixa passa a distribuir as faixas"
                        tipo: "secundario"
                        onClicked: tela.confirmarAssumir = true
                    }
                    Nota {
                        visible: tela.confirmarAssumir
                        text: "Só faça isso se o caixa que distribui as faixas não vai voltar (computador quebrado, trocado). "
                              + "Este caixa pula 20 faixas por segurança. Se o outro voltar, ele para de distribuir quando receber esta mudança."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                    Linha {
                        visible: tela.confirmarAssumir
                        spacing: 12
                        Botao {
                            text: "Confirmar"
                            onClicked: tela.comandoCodigos("codigos_assumir", {}, "Este caixa passou a distribuir as faixas de códigos.")
                        }
                    }
                }

                // ---------------- Simultaneidade › Histórico (0.9.15) ----------------
                Faixa {
                    visible: tela.secao === "historico"
                    titulo: "Troca de dados"
                    subtitulo: "como foi a última troca com cada caixa"
                    color: Tema.verde
                    corTitulo: Tema.verdeEscuro
                    Layout.fillWidth: true
                    Nota {
                        visible: !!tela.historicoTroca && tela.historicoTroca.caixas.length === 0
                        text: "Este computador ainda não trocou dados com nenhum outro caixa."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                    Repeater {
                        model: tela.historicoTroca ? tela.historicoTroca.caixas : []
                        delegate: Rectangle {
                            id: linhaTroca
                            required property var modelData
                            Layout.fillWidth: true
                            required property int index
                            // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                            readonly property bool par: (index + 1) % 2 === 0
                            color: Tema.faixa(linhaTroca.par, Tema.vela(0.4), "transparent")
                            palette.window: Tema.superficie(color)
                            radius: Tema.raioCampo
                            implicitHeight: conteudoDaFaixa8.implicitHeight + 10
                            implicitWidth: conteudoDaFaixa8.implicitWidth + 20
                            RowLayout {
                                id: conteudoDaFaixa8
                                x: 10
                                y: 5
                                width: linhaTroca.width - 20
                                spacing: 10
                                Rectangle {
                                    Layout.preferredWidth: 10
                                    Layout.preferredHeight: 10
                                    Layout.alignment: Qt.AlignVCenter
                                    radius: 5
                                    color: linhaTroca.modelData.situacao === "retirado" ? Tema.apagadoDaFaixa(linhaTroca.par)
                                           : linhaTroca.modelData.erro ? Tema.vermelhoDaFaixa(linhaTroca.par)
                                           : linhaTroca.modelData.sincronizadoEm ? Tema.verdeEscuroDaFaixa(linhaTroca.par) : Tema.apagadoDaFaixa(linhaTroca.par)
                                }
                                ColumnLayout {
                                    spacing: 1
                                    Layout.fillWidth: true
                                    Text {
                                        text: linhaTroca.modelData.nome + (linhaTroca.modelData.situacao === "retirado" ? " (retirado da loja)" : "")
                                        textFormat: Text.PlainText
                                        color: Tema.tintaDaFaixa(linhaTroca.par)
                                        font.family: Tema.texto
                                        font.weight: Font.DemiBold
                                        font.pixelSize: 15
                                        Layout.fillWidth: true
                                    }
                                    Text {
                                        text: "Última troca: " + (linhaTroca.modelData.sincronizadoEm || "nunca")
                                              + " · visto na rede: " + (linhaTroca.modelData.vistoEm || "nunca")
                                              + (linhaTroca.modelData.erro ? " · último erro: " + linhaTroca.modelData.erro : "")
                                        textFormat: Text.PlainText
                                        color: linhaTroca.modelData.erro ? Tema.vermelhoDaFaixa(linhaTroca.par) : Tema.tintaSuaveDaFaixa(linhaTroca.par)
                                        font.family: Tema.texto
                                        font.pixelSize: 13
                                        wrapMode: Text.WordWrap
                                        Layout.fillWidth: true
                                    }
                                }
                            }
                        }
                    }
                    Rotulo {
                        text: "Pendências encerradas"
                        Layout.topMargin: 6
                    }
                    Nota {
                        visible: !tela.historicoTroca || tela.historicoTroca.encerradas.length === 0
                        text: "Nenhuma pendência da troca de dados foi encerrada ainda. As que estão abertas ficam em Caixas."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                    Repeater {
                        model: tela.historicoTroca ? tela.historicoTroca.encerradas : []
                        delegate: Rectangle {
                            id: faixaDaLista9
                            required property var modelData
                            Layout.fillWidth: true
                            required property int index
                            // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                            readonly property bool par: (index + 1) % 2 === 0
                            color: Tema.faixa(faixaDaLista9.par, Tema.vela(0.4), "transparent")
                            palette.window: Tema.superficie(color)
                            radius: Tema.raioCampo
                            implicitHeight: conteudoDaFaixa9.implicitHeight + 10
                            implicitWidth: conteudoDaFaixa9.implicitWidth + 20
                            Text {
                                id: conteudoDaFaixa9
                                x: 10
                                y: 5
                                width: faixaDaLista9.width - 20
                                // H10 (7.2): os dados vão escapados (o texto é formatado).
                                text: "<b>" + Tema.escapar(modelData.oQue) + "</b> · " + (modelData.lado === "nao_enviada" ? "não ia para " : "veio de ") + Tema.escapar(modelData.caixa)
                                      + " · " + Tema.escapar(modelData.textoMotivo) + " · desde " + Tema.escapar(modelData.criadoEm) + " · "
                                      + (modelData.estado === "arquivada_manual"
                                         ? "arquivada em " + Tema.escapar(modelData.encerradaEm) + " por " + Tema.escapar(modelData.arquivadaPor || "gerente") + ": " + Tema.escapar(modelData.arquivadaMotivo)
                                         : "resolvida em " + Tema.escapar(modelData.encerradaEm))
                                      + (modelData.tentativas > 0 ? " · " + Tema.escapar(modelData.tentativas) + (modelData.tentativas === 1 ? " tentativa" : " tentativas") : "")
                                textFormat: Text.StyledText
                                color: modelData.estado === "arquivada_manual" ? Tema.tintaSuaveDaFaixa(faixaDaLista9.par) : Tema.tintaDaFaixa(faixaDaLista9.par)
                                font.family: Tema.texto
                                font.pixelSize: 13
                                wrapMode: Text.WordWrap
                            }
                        }
                    }
                }

                Faixa {
                    visible: tela.secao === "historico"
                    titulo: "Quem fez o quê"
                    subtitulo: "auditoria da loja: os " + tela.auditoria.length + " registros mais recentes"
                    color: Tema.pergaminho
                    Layout.fillWidth: true
                    // 9b (C8): veio de um conflito em Duplicatas.
                    Nota {
                        // T042: por cima da linha da barra de pesquisa logo abaixo: o halo da barra passa atrás, não por cima.
                        z: 1
                        visible: !!tela.filtroRegistro || tela.avisoHistorico !== ""
                        text: tela.filtroRegistro ? "Só o que aconteceu com este registro (" + tela.filtroRegistro.nome + "), entre os "
                                                    + tela.auditoria.length + " eventos mais recentes."
                                                  : tela.avisoHistorico + (filtroAuditoria.texto.trim() !== ""
                                                        ? "o histórico com o filtro “" + filtroAuditoria.texto.trim() + "”."
                                                        : "todo o histórico.")
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                    // Voltar às Duplicatas é a pílula delas (sem botão de texto que só volta:
                    // decisão definitiva do dono sobre a seta de retorno, 07/11/2026).
                    Botao {
                        visible: !!tela.filtroRegistro
                        text: "Ver tudo"
                        tipo: "discreto"
                        altura: 32
                        onClicked: tela.filtroRegistro = null
                    }
                    Campo {
                        id: filtroAuditoria
                        visible: !tela.filtroRegistro
                        dica: "Filtrar por usuário, caixa ou o que foi feito"
                        icone: "busca"
                        Layout.fillWidth: true
                    }
                    Nota {
                        visible: tela.auditoria.length === 0
                        text: "Nada registrado ainda."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                    Repeater {
                        model: {
                            const g = tela.filtroRegistro
                            if (g)
                                return tela.auditoria.filter(r => r.entidade === g.entidade && r.entidadeId === g.id)
                            const f = filtroAuditoria.texto.trim().toLowerCase()
                            if (!f)
                                return tela.auditoria
                            return tela.auditoria.filter(r => (r.usuario + " " + r.caixa + " " + r.descricao + " " + r.detalhes.join(" ")).toLowerCase().indexOf(f) >= 0)
                        }
                        delegate: Rectangle {
                            id: faixaDaLista10
                            required property var modelData
                            Layout.fillWidth: true
                            required property int index
                            // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                            readonly property bool par: (index + 1) % 2 === 0
                            color: Tema.faixa(faixaDaLista10.par, Tema.vela(0.4), "transparent")
                            palette.window: Tema.superficie(color)
                            radius: Tema.raioCampo
                            implicitHeight: conteudoDaFaixa10.implicitHeight + 10
                            implicitWidth: conteudoDaFaixa10.implicitWidth + 20
                            ColumnLayout {
                                id: conteudoDaFaixa10
                                x: 10
                                y: 5
                                width: faixaDaLista10.width - 20
                                spacing: 1
                                Text {
                                    // H10 (7.2): os dados vão escapados (o texto é formatado).
                                    text: "<b>" + Tema.escapar(modelData.quando) + "</b> · " + Tema.escapar(modelData.usuario || "Sistema")
                                          + (modelData.caixa ? " (" + Tema.escapar(modelData.caixa) + ")" : "") + " " + Tema.escapar(modelData.descricao)
                                    textFormat: Text.StyledText
                                    color: Tema.tintaDaFaixa(faixaDaLista10.par)
                                    font.family: Tema.texto
                                    font.pixelSize: 14
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                                Text {
                                    visible: modelData.detalhes.length > 0
                                    text: modelData.detalhes.join(" · ")
                                    textFormat: Text.PlainText
                                    color: Tema.tintaSuaveDaFaixa(faixaDaLista10.par)
                                    font.family: Tema.texto
                                    font.pixelSize: 13
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                            }
                        }
                    }
                }

                // ---------------- Backup ----------------
                Loader {
                    id: parteBackup
                    // Só com a consulta do Backup liberada: aberta direto aqui
                    // (pela faixa do modo seguro ou por uma notificação), a
                    // parte não carrega antes da senha (antes, o caixa via um
                    // "Só o gerente pode" por trás do pedido da senha).
                    active: tela.secao === "backup" && tela.verBackup
                    visible: active
                    Layout.fillWidth: true
                    sourceComponent: SecaoBackup { width: corpo.width }
                }

                // ---------------- Aparência (P6 / MG-D1) ----------------
                Faixa {
                    visible: tela.secao === "aparencia"
                    titulo: "Aparência deste " + Tema.aparelho
                    color: Tema.pergaminho
                    Layout.fillWidth: true
                    Rotulo { text: "Tema" }
                    RowLayout {
                        spacing: 14
                        Layout.fillWidth: true
                        Aparencia {}
                        Nota {
                            text: "Escolha Claro, Escuro (AMOLED) ou Planeta Roxo. Vale só para este " + Tema.aparelho + "."
                            Layout.fillWidth: true
                        }
                    }
                    Rotulo { text: "Movimento reduzido"; Layout.topMargin: 10 }
                    Nota {
                        text: "Controla a rolagem guiada e as transições de tela neste " + Tema.aparelho
                              + ". Em movimento reduzido, a tela vai direto ao campo e usa apenas esmaecimento leve."
                        Layout.fillWidth: true
                    }
                    Pilulas {
                        id: pilulasMovimento
                        objectName: "pilulasMovimentoReduzido"
                        pequenas: true
                        modelo: [
                            { id: "sistema", nome: "Seguir o " + Tema.aparelho },
                            { id: "ligado", nome: "Reduzido" },
                            { id: "desligado", nome: "Normal" }
                        ]
                        atual: Tema.modoMovimento
                        onEscolhido: id => tela.escolherMovimento(id)
                    }
                    Nota {
                        text: Tema.movimentoReduzido
                              ? "Ativo agora: movimento reduzido (sem rolagem animada)."
                              : "Ativo agora: movimento normal (rolagem suave)."
                        Layout.fillWidth: true
                    }
                }

                // ---------------- Sobre ----------------
                Faixa {
                    visible: tela.secao === "sobre"
                    titulo: "Sobre"
                    color: Tema.pergaminho
                    Layout.fillWidth: true
                    Text {
                        text: "Rustinho " + (tela.config ? tela.config.versao : "") + " · Etapa " + (tela.janela && tela.janela.estado.etapa ? tela.janela.estado.etapa : "")
                        textFormat: Text.PlainText
                        color: Tema.tinta
                        font.family: Tema.titulo
                        font.weight: Font.Bold
                        font.pixelSize: 24
                    }
                    Nota {
                        text: (tela.config ? tela.config.pecas : 0) + " peças no banco de dados."
                        Layout.fillWidth: true
                    }
                    Rotulo { text: "Pasta dos dados"; Layout.topMargin: 4 }
                    TextEdit {
                        text: tela.config ? tela.config.dados : ""
                        readOnly: true
                        selectByMouse: true
                        wrapMode: TextEdit.WrapAnywhere
                        color: Tema.tintaSuave
                        font.family: Tema.texto
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                    Botao {
                        text: "Abrir a pasta"
                        tipo: "secundario"
                        visible: !!tela.config && tela.config.dados !== ""
                        onClicked: {
                            const caminho = tela.config.dados
                            Qt.openUrlExternally(Api.url(Api.pastaDe(caminho)))
                        }
                    }
                    Rotulo { text: "Código deste computador"; Layout.topMargin: 4 }
                    TextEdit {
                        text: tela.config ? tela.config.dispositivo : ""
                        readOnly: true
                        selectByMouse: true
                        color: Tema.tintaSuave
                        font.family: Tema.texto
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                }
            }
        }
    }

    PerguntaMotivo { id: motivoReferencia; onFeito: tela.recarregarReferencias() }

    Component.onCompleted: {
        janelaAoSair = janela
        completo = janela.podeGerente("configuracoes", true)
        verBackup = janela.podeGerente("backup", true)
        verGerente = janela.podeGerente("gerente", true)
        verSimultaneidade = janela.podeGerente("simultaneidade", true)
        carregar()
        // Aberta por uma notificação numa parte que só aparece com a senha do
        // gerente: pede a senha ali mesmo, sem trocar de usuário.
        // Desistir da senha aqui volta para a tela de antes (NOTIF-03).
        const soComGerente = !["loja", "impressao", "aparencia", "sobre"].includes(secao)
        if (partesSimultaneidade.includes(secao)) {
            const destino = secao
            secao = "loja"
            // Com guia (caso 6): desistir da senha acaba a guia e volta à
            // origem sem marcar nada; com a senha, a guia destaca na parte.
            if (guia && !janela.podeGerente("simultaneidade", true))
                janela.guiaOrientar("Esta parte das configurações pede a senha do gerente.")
            entrarSimultaneidade(destino, guia ? () => janela.concluirGuia(false) : sairSemGerente,
                                 guia ? () => {
                                     janela.guiaOrientar("")
                                     tela.comecarGuia()
                                 } : () => Qt.callLater(tela.mostrarAlvo))
            return
        } else if (soComGerente && !completo) {
            const destino = secao
            secao = "loja"
            janela.comGerente("configuracoes", () => {
                tela.completo = true
                carregar()
                tela.escolherSecao(destino, sairSemGerente)
            }, sairSemGerente)
        } else if (secao === "backup" || secao === "gerente") {
            const destino = secao
            secao = "loja"
            escolherSecao(destino, sairSemGerente)
        }
        if (guia)
            comecarGuia()
    }
}
