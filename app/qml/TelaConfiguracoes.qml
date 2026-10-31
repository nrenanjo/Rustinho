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
    onSecaoChanged: if (partesSimultaneidade.includes(secao)) ultimaSimultaneidade = secao
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
    // Como esta tela aparece no "voltar" de outra (navegação guiada).
    readonly property string nomeNaNavegacao: secao === "fiscal" ? "Fiscal" : "Configurações"
    // Se um gerente liberou esta tela para um caixa, acaba ao sair dela.
    Component.onDestruction: {
        Api.chamar("gerente_encerrar")
        if (guia)
            janela.abandonarGuia(guia)
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
    // Guia: abre a parte do requisito e destaca o primeiro campo que falta.
    // Os campos da loja só aparecem com a senha do gerente: sem ela, pede a
    // senha primeiro (a guia continua apontando para o botão da senha).
    function comecarGuia() {
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
        const c = campoChamado(guia.requisito.campos[0])
        if (!c)
            return
        secao = c[1]
        Qt.callLater(() => {
            janela.guiaDestacar(c[0], rolagem)
            // Estado e CNPJ juntos: o cursor vai para o que falta primeiro.
            const foco = c[0] === linhaDocumentos ? (ufLoja.indice > 0 ? cnpjLoja : ufLoja) : c[0]
            if (typeof foco.focarSempre === "function")
                foco.focarSempre()
            else if (foco.combo)
                foco.combo.forceActiveFocus()
        })
    }
    // Sair: com guia, volta à tela de origem sem marcar nada.
    function sair() {
        if (guia)
            janela.concluirGuia(false)
        else
            janela.voltar()
    }

    // Exemplo do crediário: parcela de R$ 100,00 com 10 dias de atraso.
    function numero(t) {
        const v = parseFloat(String(t || "").replace(/\./g, "").replace(",", "."))
        return isNaN(v) ? 0 : v
    }
    readonly property string exemploJuros: {
        const dias = 10
        const tol = parseInt(carencia.texto) || 0
        if (dias <= tol)
            return "Exemplo: parcela de R$ 100,00 com 10 dias de atraso fica em R$ 100,00 (está dentro da tolerância)."
        const extra = numero(multa.texto) + numero(jurosMes.texto) * dias / 30
        const total = 100 + extra
        return "Exemplo: parcela de R$ 100,00 com 10 dias de atraso fica em R$ "
               + total.toFixed(2).replace(".", ",") + (extra > 0 ? " (R$ " + extra.toFixed(2).replace(".", ",") + " de multa e juros)." : ".")
    }

    function carregar() {
        const r = Api.chamar("config_ler")
        if (!r.ok) {
            janela.avisar(r.erro, "erro")
            return
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
    function entrarSimultaneidade(destino) {
        janela.comGerenteVarias(["configuracoes", "backup", "simultaneidade"], "Simultaneidade", () => {
            const recarregar = !tela.completo || !tela.verSimultaneidade
            tela.completo = true
            tela.verBackup = true
            tela.verSimultaneidade = true
            if (recarregar)
                carregar()
            carregarHistorico()
            tela.secao = destino
        })
    }

    function carregarHistorico() {
        const h = Api.chamar("troca_historico")
        historicoTroca = h.ok ? h.dados : null
        const a = Api.chamar("auditoria_listar")
        auditoria = a.ok ? a.dados : []
    }

    function escolherSecao(id) {
        if (id === "simultaneidade") {
            entrarSimultaneidade(ultimaSimultaneidade)
            return
        }
        if (id === "backup" && !verBackup) {
            janela.comGerente("backup", () => {
                tela.verBackup = true
                tela.secao = id
            })
            return
        }
        if (id === "gerente" && !verGerente) {
            janela.comGerente("gerente", () => {
                tela.verGerente = true
                carregarGerente()
                tela.secao = id
            })
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
            janela.atualizarEstado()
            carregar()
            janela.avisar("Configurações salvas.", "ok")
        } else if (r.campo === "caixa") {
            secao = "loja"
            caixa.erro = r.erro
            caixa.focar()
        } else {
            janela.avisar(r.erro, "erro")
        }
    }

    function salvar() {
        if (secao === "gerente")
            return salvarGerente()
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
                if (typeof alvo.focar === "function")
                    alvo.focar()
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

    function carregarCaixas() {
        const c = Api.chamar("caixas_conhecidos")
        if (c.ok) {
            caixasConhecidos = c.dados.caixas
            prazoCompatibilidade = c.dados.prazoDias
        }
        if (verSimultaneidade) {
            const p = Api.chamar("pendencias_listar")
            pendencias = p.ok ? p.dados : []
        }
    }

    function retirarCaixa(c) {
        motivoReferencia.perguntar(
            "Retirar " + c.nome + " da loja?",
            "Use só quando este computador deixou de ser da loja. Ele para de trocar dados e não conta mais para a versão dos caixas; "
            + "se voltar à rede, não entra sozinho: o gerente precisa readmiti-lo. Fica no histórico com o seu nome e o motivo.",
            "Retirar da loja", (m, pronto) => janela.chamarGerente("caixa_retirar", { dispositivo: c.dispositivo, motivo: m }, r => {
                pronto(r)
                if (r.ok) {
                    carregarCaixas()
                    janela.avisar(c.nome + " foi retirado da loja.", "ok")
                }
            }),
            "Motivo (por que sai da loja?)", "perigo")
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
        carregarCaixas()
        const r = Api.chamar("sync_ler")
        if (r.ok) {
            sync = r.dados
            syncLigado.checked = sync.ligado
            syncEnderecos.texto = sync.enderecos
            syncInterfaces.texto = sync.interfaces
            syncSenha.texto = ""
        }
    }

    // Só a situação e a lista de referências (não mexe no que está sendo digitado).
    function recarregarReferencias() {
        const r = Api.chamar("sync_ler")
        if (r.ok)
            sync = r.dados
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

    Timer {
        interval: 3000
        repeat: true
        running: tela.StackView.status === StackView.Active && !!tela.sync && tela.sync.ligado
        onTriggered: {
            const r = Api.chamar("sync_ler")
            if (r.ok)
                tela.sync = r.dados
        }
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
        if (secao === "fiscal" && fiscal.item && fiscal.item.regrasAlteradas)
            janela.confirmar("Descartar as regras alteradas?", "As mudanças nas regras de ST ainda não foram salvas.",
                             "Descartar", () => tela.sair(), "perigo")
        else if (alterado)
            janela.confirmar("Descartar alterações?", "As configurações mudadas ainda não foram salvas.",
                             "Descartar", () => tela.sair(), "perigo")
        else
            tela.sair()
    }

    Shortcut {
        sequence: "F2"
        enabled: tela.StackView.status === StackView.Active
        onActivated: tela.salvar()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        anchors.bottomMargin: 0
        spacing: 14

        Cabecalho {
            sensivel: true
            titulo: "Configurações"
            // Aberta pela navegação guiada: o "voltar" diz para onde volta.
            textoVoltar: tela.guia ? tela.guia.origemNome : "Início"
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
                sobre: "Versão e onde ficam os dados"
            })[tela.secao]
            Layout.fillWidth: true
            onVoltar: tela.aoEsc()
            Rotulo {
                visible: tela.alterado
                text: "Não salvo"
                color: Tema.vermelho
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

        Pilulas {
            modelo: tela.completo ? [
                { id: "loja", nome: "Loja" },
                { id: "vendas", nome: "Vendas" },
                { id: "receber", nome: "Crediário" },
                { id: "impressao", nome: "Impressão" },
                { id: "simultaneidade", nome: "Simultaneidade" },
                { id: "gerente", nome: "Gerente" },
                { id: "sobre", nome: "Sobre" }
            ] : [
                { id: "loja", nome: "Este computador" },
                { id: "impressao", nome: "Impressão" },
                { id: "simultaneidade", nome: "Simultaneidade" },
                { id: "sobre", nome: "Sobre" }
            ]
            atual: tela.grupo
            onEscolhido: id => tela.escolherSecao(id)
        }

        // Subpílulas de Simultaneidade, numa faixa clara.
        Rectangle {
            visible: tela.grupo === "simultaneidade"
            color: Tema.sombra(0.06)
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

        // Fiscal: a conferência fiscal inteira (a mesma da tela Fiscal).
        Loader {
            id: fiscal
            visible: tela.secao === "fiscal"
            active: tela.secao === "fiscal"
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
            clip: true
            contentHeight: corpo.implicitHeight + Tema.margem
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}

            ColumnLayout {
                id: corpo
                width: Math.min(rolagem.width - 14, 960)
                y: 4
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
                            onEditado: tela.alterado = true
                        }
                        Campo {
                            id: caixa
                            rotulo: "Nome deste computador"
                            maximo: 30
                            Layout.preferredWidth: 220
                            onEditado: tela.alterado = true
                        }
                    }
                    Linha {
                        id: linhaDocumentos
                        visible: tela.completo
                        spacing: 12
                        Layout.fillWidth: true
                        Campo {
                            id: cnpjLoja
                            rotulo: "CNPJ da loja"
                            dica: "00.000.000/0000-00"
                            maximo: 20
                            maiusculas: true
                            sensivel: true
                            Layout.preferredWidth: 240
                            onEditado: tela.alterado = true
                        }
                        Selecao {
                            id: ufLoja
                            rotulo: "Estado"
                            Layout.preferredWidth: 100
                            onEscolhido: tela.alterado = true
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
                            dica: "Pelo menos 4 caracteres"
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
                        Rotulo { text: "Lucro sugerido"; Layout.preferredWidth: 150 }
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
                                alvo.focar()
                            }

                            Text {
                                property bool manterLargura: true
                                text: linhaTabela.numero
                                color: Tema.dourado
                                font.family: Tema.titulo
                                font.weight: Font.Bold
                                font.pixelSize: 26
                                Layout.preferredWidth: 30
                                Layout.alignment: Qt.AlignTop
                            }
                            Campo {
                                id: nomeTabela
                                texto: linhaTabela.modelData.nome
                                maximo: 30
                                Layout.fillWidth: true
                                onEditado: tela.alterado = true
                            }
                            // D-09: só quem vê os custos vê e muda o lucro sugerido
                            // (o campo fica, sem esconder: veja D-10).
                            Campo {
                                id: lucroTabela
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
                            dica: tela.sync && tela.sync.temSenha ? "•••••••• (deixe vazio para manter)" : "A mesma em todos os caixas"
                            senha: true
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
                    // O-3 / RVI-02: não mostra a troca como funcionando com uma
                    // tarefa parada ou esperando para tentar de novo (com o motivo).
                    Nota {
                        readonly property var falhas: tela.sync && tela.sync.tarefasComFalha ? tela.sync.tarefasComFalha : []
                        visible: falhas.length > 0
                        text: "A troca de dados não está funcionando neste caixa: "
                              + falhas.map(f => f.tarefa + " — " + f.motivo
                                                + (f.situacao === "parada" ? " (parou)" : " (tentando de novo)")).join("; ")
                              + (falhas.some(f => f.situacao === "parada")
                                 ? ". Feche e abra o Rustinho; se continuar, chame o suporte." : ".")
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
                    Botao {
                        text: "Salvar sincronização"
                        tipo: "secundario"
                        onClicked: tela.salvarSync()
                    }
                    Repeater {
                        model: tela.sync ? tela.sync.pares : []
                        delegate: RowLayout {
                            required property var modelData
                            spacing: 8
                            Layout.fillWidth: true
                            Rectangle {
                                Layout.preferredWidth: 10
                                Layout.preferredHeight: 10
                                Layout.alignment: Qt.AlignVCenter
                                radius: 5
                                color: modelData.erro ? Tema.vermelho : modelData.sincronizadoEm ? Tema.verdeEscuro : Tema.apagado
                            }
                            Text {
                                text: (modelData.nome || "Caixa") + " · " + (modelData.erro ? modelData.erro : "sincronizado " + tela.quandoFoi(modelData.sincronizadoEm) + (modelData.via ? " · via " + modelData.via : ""))
                                color: modelData.erro ? Tema.vermelho : Tema.tinta
                                font.family: Tema.texto
                                font.pixelSize: 14
                                wrapMode: Text.WordWrap
                                Layout.fillWidth: true
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
                        color: Tema.vermelho
                        font.family: Tema.texto
                        font.pixelSize: 13
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                    // Cada registro incompleto: o que é, o que falta e o que fazer.
                    Repeater {
                        model: tela.sync && tela.sync.referencias ? tela.sync.referencias.filter(r => r.estado !== "esperando" || r.atrasada).slice(0, 20) : []
                        delegate: ColumnLayout {
                            id: linhaRef
                            required property var modelData
                            spacing: 6
                            Layout.fillWidth: true
                            Text {
                                text: "<b>" + linhaRef.modelData.oQue + "</b> (desde " + linhaRef.modelData.desde + ") aponta para "
                                      + linhaRef.modelData.oQueFalta.toLowerCase() + " que "
                                      + (linhaRef.modelData.estado === "esperando" ? (linhaRef.modelData.atrasada ? "ainda não chegou (atrasada)" : "ainda não chegou")
                                         : linhaRef.modelData.estado === "apagado" ? "foi apagado"
                                         : linhaRef.modelData.estado === "aceito" ? "não existe (aceito pelo gerente " + linhaRef.modelData.aceitaPor
                                                                                  + " em " + linhaRef.modelData.aceitaEm + ": " + linhaRef.modelData.aceiteMotivo + ")"
                                         : "não existe (comprovado)")
                                      + ". " + linhaRef.modelData.acao
                                textFormat: Text.StyledText
                                color: linhaRef.modelData.estado === "esperando" || linhaRef.modelData.estado === "aceito" ? Tema.tintaSuave : Tema.vermelho
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
                                    "Aceitar", (m, pronto) => janela.chamarGerente("referencia_aceitar",
                                                             { tabela: linhaRef.modelData.tabela, chave: linhaRef.modelData.chave, motivo: m }, pronto),
                                    "Motivo (por que aceitar?)", "secundario")
                            }
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
                        model: tela.caixasConhecidos
                        delegate: RowLayout {
                            id: linhaCaixa
                            required property var modelData
                            spacing: 10
                            Layout.fillWidth: true
                            Rectangle {
                                Layout.preferredWidth: 10
                                Layout.preferredHeight: 10
                                Layout.alignment: Qt.AlignVCenter
                                radius: 5
                                color: linhaCaixa.modelData.situacao === "compativel" ? Tema.verdeEscuro
                                       : linhaCaixa.modelData.situacao === "retirado" ? Tema.apagado : Tema.vermelho
                            }
                            ColumnLayout {
                                spacing: 1
                                Layout.fillWidth: true
                                Text {
                                    text: linhaCaixa.modelData.nome
                                    color: Tema.tinta
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
                                    color: linhaCaixa.modelData.bloqueia ? Tema.vermelho : Tema.tintaSuave
                                    font.family: Tema.texto
                                    font.pixelSize: 13
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                            }
                            Botao {
                                visible: linhaCaixa.modelData.situacao !== "retirado"
                                text: "Retirar da loja"
                                tipo: "discreto"
                                onClicked: tela.retirarCaixa(linhaCaixa.modelData)
                            }
                            Botao {
                                visible: linhaCaixa.modelData.situacao === "retirado"
                                text: "Readmitir"
                                tipo: "secundario"
                                onClicked: tela.readmitirCaixa(linhaCaixa.modelData)
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
                        model: tela.verSimultaneidade ? tela.pendencias : []
                        delegate: RowLayout {
                            id: linhaPend
                            required property var modelData
                            spacing: 10
                            Layout.fillWidth: true
                            Text {
                                text: "<b>" + linhaPend.modelData.textoMotivo + "</b> · " + linhaPend.modelData.oQue + " · "
                                      + (linhaPend.modelData.lado === "nao_enviada" ? "não vai para " : "veio de ") + linhaPend.modelData.caixa
                                      + (linhaPend.modelData.detalhe ? " · " + linhaPend.modelData.detalhe : "")
                                      + (linhaPend.modelData.ultimoErro ? " · " + linhaPend.modelData.ultimoErro : "")
                                      + (linhaPend.modelData.estado === "arquivada_manual"
                                         ? " · arquivada por " + linhaPend.modelData.arquivadaPor + ": " + linhaPend.modelData.arquivadaMotivo : "")
                                textFormat: Text.StyledText
                                color: linhaPend.modelData.estado === "arquivada_manual" ? Tema.tintaSuave : Tema.tinta
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
                        text: "Cada gerente troca a própria senha em “Minha senha”. A senha de gerente precisa ter pelo menos 4 caracteres; como ela também protege os backups, quanto mais longa, melhor."
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
                        model: tela.conflitos
                        delegate: Rectangle {
                            required property var modelData
                            color: modelData.resolvido ? "transparent" : Tema.papel
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
                                    color: Tema.tinta
                                    font.family: Tema.texto
                                    font.weight: Font.DemiBold
                                    font.pixelSize: 15
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                                Text {
                                    text: modelData.texto
                                    color: Tema.tinta
                                    font.family: Tema.texto
                                    font.pixelSize: 14
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                                Repeater {
                                    model: modelData.campos
                                    delegate: Text {
                                        required property var modelData
                                        text: modelData.nome + ": ficou " + modelData.ficou + " · a outra versão tinha " + modelData.perdeu
                                        color: Tema.tintaSuave
                                        font.family: Tema.texto
                                        font.pixelSize: 14
                                        wrapMode: Text.WordWrap
                                        Layout.fillWidth: true
                                    }
                                }
                                Text {
                                    visible: !!modelData.resolvido
                                    text: "Resolvido: " + modelData.resolvido
                                    color: Tema.verdeEscuro
                                    font.family: Tema.texto
                                    font.pixelSize: 14
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
                        Botao {
                            text: "Cancelar"
                            tipo: "secundario"
                            onClicked: tela.confirmarCaixaNovo = false
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
                        delegate: RowLayout {
                            required property var modelData
                            spacing: 10
                            Layout.fillWidth: true
                            Text {
                                text: modelData.nome + " · código " + modelData.codigo + " (" + modelData.caixa + ")"
                                      + (modelData.conflito ? " · já é da peça “" + modelData.outraPeca + "”" : " · esperando a confirmação")
                                color: modelData.conflito ? Tema.vermelho : Tema.tinta
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
                    // Faixas de dois caixas que se cruzam (lojas juntadas, dois distribuidores).
                    Repeater {
                        model: tela.codigos ? tela.codigos.cruzadas : []
                        delegate: Text {
                            required property var modelData
                            text: "Os códigos de " + tela.milhar(modelData.inicio) + " a " + tela.milhar(modelData.fim) + " são de dois caixas ("
                                  + modelData.caixas.join(" e ") + "). Nenhum caixa dá código novo nesse pedaço; as peças que já têm código dele ficam como estão."
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
                        model: tela.codigos ? tela.codigos.repetidos.slice(0, 50) : []
                        delegate: Rectangle {
                            id: grupo
                            required property var modelData
                            color: Tema.papel
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
                                    color: Tema.tinta
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
                                            color: Tema.tinta
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
                        delegate: RowLayout {
                            required property var modelData
                            spacing: 10
                            Layout.fillWidth: true
                            Text {
                                text: modelData.nome + " · código " + modelData.codigo + " (" + modelData.caixa + ")"
                                      + (modelData.conflito ? " · já é de “" + modelData.outraPessoa + "”" : " · esperando a confirmação")
                                color: modelData.conflito ? Tema.vermelho : Tema.tinta
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
                            id: grupoPessoas
                            required property var modelData
                            color: Tema.papel
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
                                    color: Tema.tinta
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
                                            color: Tema.tinta
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
                        Botao {
                            text: "Cancelar"
                            tipo: "secundario"
                            onClicked: tela.confirmarAssumir = false
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
                        delegate: RowLayout {
                            id: linhaTroca
                            required property var modelData
                            spacing: 10
                            Layout.fillWidth: true
                            Rectangle {
                                Layout.preferredWidth: 10
                                Layout.preferredHeight: 10
                                Layout.alignment: Qt.AlignVCenter
                                radius: 5
                                color: linhaTroca.modelData.situacao === "retirado" ? Tema.apagado
                                       : linhaTroca.modelData.erro ? Tema.vermelho
                                       : linhaTroca.modelData.sincronizadoEm ? Tema.verdeEscuro : Tema.apagado
                            }
                            ColumnLayout {
                                spacing: 1
                                Layout.fillWidth: true
                                Text {
                                    text: linhaTroca.modelData.nome + (linhaTroca.modelData.situacao === "retirado" ? " (retirado da loja)" : "")
                                    color: Tema.tinta
                                    font.family: Tema.texto
                                    font.weight: Font.DemiBold
                                    font.pixelSize: 15
                                    Layout.fillWidth: true
                                }
                                Text {
                                    text: "Última troca: " + (linhaTroca.modelData.sincronizadoEm || "nunca")
                                          + " · visto na rede: " + (linhaTroca.modelData.vistoEm || "nunca")
                                          + (linhaTroca.modelData.erro ? " · último erro: " + linhaTroca.modelData.erro : "")
                                    color: linhaTroca.modelData.erro ? Tema.vermelho : Tema.tintaSuave
                                    font.family: Tema.texto
                                    font.pixelSize: 13
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
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
                        delegate: Text {
                            required property var modelData
                            text: "<b>" + modelData.oQue + "</b> · " + (modelData.lado === "nao_enviada" ? "não ia para " : "veio de ") + modelData.caixa
                                  + " · " + modelData.textoMotivo + " · desde " + modelData.criadoEm + " · "
                                  + (modelData.estado === "arquivada_manual"
                                     ? "arquivada em " + modelData.encerradaEm + " por " + (modelData.arquivadaPor || "gerente") + ": " + modelData.arquivadaMotivo
                                     : "resolvida em " + modelData.encerradaEm)
                                  + (modelData.tentativas > 0 ? " · " + modelData.tentativas + (modelData.tentativas === 1 ? " tentativa" : " tentativas") : "")
                            textFormat: Text.StyledText
                            color: modelData.estado === "arquivada_manual" ? Tema.tintaSuave : Tema.tinta
                            font.family: Tema.texto
                            font.pixelSize: 13
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                    }
                }

                Faixa {
                    visible: tela.secao === "historico"
                    titulo: "Quem fez o quê"
                    subtitulo: "auditoria da loja: os " + tela.auditoria.length + " registros mais recentes"
                    color: Tema.pergaminho
                    Layout.fillWidth: true
                    Campo {
                        id: filtroAuditoria
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
                            const f = filtroAuditoria.texto.trim().toLowerCase()
                            if (!f)
                                return tela.auditoria
                            return tela.auditoria.filter(r => (r.usuario + " " + r.caixa + " " + r.descricao + " " + r.detalhes.join(" ")).toLowerCase().indexOf(f) >= 0)
                        }
                        delegate: ColumnLayout {
                            required property var modelData
                            spacing: 1
                            Layout.fillWidth: true
                            Text {
                                text: "<b>" + modelData.quando + "</b> · " + (modelData.usuario || "Sistema")
                                      + (modelData.caixa ? " (" + modelData.caixa + ")" : "") + " " + modelData.descricao
                                textFormat: Text.StyledText
                                color: Tema.tinta
                                font.family: Tema.texto
                                font.pixelSize: 14
                                wrapMode: Text.WordWrap
                                Layout.fillWidth: true
                            }
                            Text {
                                visible: modelData.detalhes.length > 0
                                text: modelData.detalhes.join(" · ")
                                textFormat: Text.PlainText
                                color: Tema.tintaSuave
                                font.family: Tema.texto
                                font.pixelSize: 13
                                wrapMode: Text.WordWrap
                                Layout.fillWidth: true
                            }
                        }
                    }
                }

                // ---------------- Backup ----------------
                Loader {
                    active: tela.secao === "backup"
                    visible: active
                    Layout.fillWidth: true
                    sourceComponent: SecaoBackup { width: corpo.width }
                }

                // ---------------- Sobre ----------------
                Faixa {
                    visible: tela.secao === "sobre"
                    titulo: "Sobre"
                    color: Tema.pergaminho
                    Layout.fillWidth: true
                    Text {
                        text: "Rustinho " + (tela.config ? tela.config.versao : "") + " · Etapa " + (tela.janela && tela.janela.estado.etapa ? tela.janela.estado.etapa : "")
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
        completo = janela.podeGerente("configuracoes", true)
        verBackup = janela.podeGerente("backup", true)
        verGerente = janela.podeGerente("gerente", true)
        verSimultaneidade = janela.podeGerente("simultaneidade", true)
        carregar()
        // Aberta por uma notificação numa parte que só aparece com a senha do
        // gerente: pede a senha ali mesmo, sem trocar de usuário.
        const soComGerente = !["loja", "impressao", "sobre"].includes(secao)
        if (partesSimultaneidade.includes(secao)) {
            const destino = secao
            secao = "loja"
            entrarSimultaneidade(destino)
        } else if (soComGerente && !completo) {
            const destino = secao
            secao = "loja"
            janela.comGerente("configuracoes", () => {
                tela.completo = true
                carregar()
                tela.escolherSecao(destino)
            })
        } else if (secao === "backup" || secao === "gerente") {
            const destino = secao
            secao = "loja"
            escolherSecao(destino)
        }
        if (guia)
            comecarGuia()
    }
}
