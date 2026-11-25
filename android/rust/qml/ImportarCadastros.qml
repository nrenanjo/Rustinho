import QtCore
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts
import rustinho

// Trazer clientes, fornecedores e peças dos XMLs das notas: as da loja
// (exportadas do sistema antigo) e as dos fornecedores. Mostra antes o que vai
// ser feito, pelas mesmas regras do cadastro (T-1): o que entra, as peças que já
// existem e mudam (A03b) e o que vai para pendências (O-3).
// PEND-38/H02 (DP-52 (b)): quem não é gerente nem tem a ação liberada pela loja
// digita a senha do gerente ao abrir, antes de escolher arquivos; ela vale para
// esta tela até fechá-la. A aba de pendências continua sem senha.
Painel {
    id: painel
    property string chave: ""
    property string chavePendencia: ""
    readonly property var janela: ApplicationWindow.window
    width: Math.min(680, Tema.largura - 24)
    property string aba: "importar"
    property var pendencias: []
    property var pendenciaAberta: null
    property var arquivos: []
    property var previa: null
    property string erro: ""
    property bool trabalhando: false
    // Escolhas guardadas para quando os arquivos são lidos de novo.
    property var escolhas: null
    // Arquivos que sumiram enquanto a pessoa resolvia um requisito.
    property string avisoArquivos: ""
    // O que cadastrar (marcado na prévia).
    property bool qClientes: false
    property bool qFornecedores: false
    property bool qPecasLoja: false
    property bool qPecasFornecedor: false
    // T108 (texto R do proprietário, 21/11/2026): as notas de compra também vão para Compras, em rascunho.
    property bool qCompras: false
    // DP-52 (b): a chave desta abertura da tela e se a senha já valeu.
    property string chaveTela: ""
    property bool liberada: false
    // A navegação guiada fecha e reabre o painel: não encerra a liberação.
    property bool naGuia: false
    // A07: a peça desativada que o "Associar" recusou (para "Abrir a peça").
    property string pecaDesativadaId: ""
    // T111 (Regra 3): corrigir identificação do registro provisório sem redigitar o restante do XML.
    property bool corrigindoIdentificacao: false
    // R6 (D-AUD-5, decisão do proprietário de 25/11/2026): descartando a pendência aberta (pede o motivo).
    property bool descartando: false
    // A aba de importar, já com a senha (ou sem precisar dela).
    readonly property bool naImportacao: aba === "importar" && liberada
    // T058 (pedido do proprietário, 20/11/2026): com uma pendência aberta e
    // outras na lista, a seta e o Esc voltam à lista (no lugar do antigo
    // "Todas as pendências"); sem passo para voltar, fecham a janela.
    voltarUmPasso: function () {
        if (painel.aba === "pendencias" && painel.pendenciaAberta && painel.pendencias.length > 1) {
            painel.pendenciaAberta = null
            painel.erro = ""
            return true
        }
        return false
    }

    signal importou()


    // PEND-38/H02: antes de escolher arquivos, a senha do gerente (se precisar).
    function exigirSenhaDaTela(depois, desistiu) {
        if (liberada) {
            depois()
            return
        }
        const p = Api.chamar("permissao", { acao: "configuracoes" })
        if (p.ok && p.dados.pode) {
            liberada = true
            depois()
            return
        }
        chaveTela = Api.novaChave()
        pedirSenhaTela.paraTela("importar_cadastros", chaveTela, "Trazer cadastros", () => {
            painel.liberada = true
            depois()
        }, desistiu)
    }
    onClosed: {
        if (naGuia)
            return
        if (chaveTela !== "")
            Api.chamar("gerente_encerrar", { tela: "importar_cadastros", chave: chaveTela })
        chaveTela = ""
        liberada = false
    }
    function cat(codigo) {
        const c = previa ? (previa.categorias || []).find(x => x.codigo === codigo) : null
        return c ? c.quantidade : 0
    }
    // O que vai para a prévia e para o cadastro: os arquivos, a chave da tela e,
    // depois da primeira leitura, as opções marcadas.
    function dadosDaPrevia(comOpcoes) {
        const d = { arquivos: painel.arquivos, chaveTela: painel.chaveTela }
        if (comOpcoes) {
            d.clientes = painel.qClientes
            d.fornecedores = painel.qFornecedores
            d.pecasLoja = painel.qPecasLoja
            d.pecasFornecedor = painel.qPecasFornecedor
            d.compras = painel.qCompras
        }
        return d
    }
    // Refaz a prévia com as opções marcadas (o que vai para pendências e as
    // peças que mudam dependem delas).
    function refazerPrevia(manterErro) {
        const erroAntes = painel.erro
        trabalhando = true
        Api.chamarDemorado("importar_previa", dadosDaPrevia(true), "Conferindo…", r => {
            painel.trabalhando = false
            if (r.ok) {
                painel.previa = r.dados
                painel.erro = manterErro ? erroAntes : ""
            } else {
                painel.erro = r.erro
            }
        })
    }

    function carregarPendencias() {
        const r = Api.chamar("xml_pendencias_listar")
        if (r.ok && Array.isArray(r.dados)) {
            pendencias = r.dados
            if (pendenciaAberta) {
                const atualizada = pendencias.find(p => p.id === pendenciaAberta.id)
                pendenciaAberta = atualizada || null
            }
        }
    }

    function abrirCom() {
        chave = Api.novaChave()
        aba = "importar"
        pendenciaAberta = null
        arquivos = []
        previa = null
        erro = ""
        escolhas = null
        avisoArquivos = ""
        pecaDesativadaId = ""
        naGuia = false
        liberada = false
        chaveTela = ""
        carregarPendencias()
        open()
        // Antes de escolher arquivos: sem a senha, nada da importação aparece e,
        // desistindo, a tela fecha sem ler nada.
        Qt.callLater(() => painel.exigirSenhaDaTela(() => {}, () => painel.close()))
    }

    function abrirPendencias() {
        chave = Api.novaChave()
        aba = "pendencias"
        pendenciaAberta = null
        erro = ""
        carregarPendencias()
        if (pendencias.length === 1)
            abrirPendencia(pendencias[0])
        open()
    }

    function abrirPendencia(p) {
        pendenciaAberta = p
        chavePendencia = Api.novaChave()
        erro = ""
        pecaDesativadaId = ""
        corrigindoIdentificacao = false
        descartando = false
        motivoDescarte.texto = ""
        motivoDescarte.erro = ""
        campoCorrNome.texto = p ? (p.descricao || "") : ""
        campoCorrNome.erro = ""
        campoCorrCodigo.texto = p ? (p.codigoFornecedor || "") : ""
        campoCorrCodigo.erro = ""
        campoCorrBarras.texto = p ? (p.gtinComercial || "") : ""
        campoCorrBarras.erro = ""
        campoCorrUnidade.texto = p ? (p.unidade || "") : ""
        campoCorrUnidade.erro = ""
    }

    function resolverComoNova(comCorrecao) {
        if (!pendenciaAberta)
            return
        erro = ""
        campoCorrNome.erro = ""
        campoCorrCodigo.erro = ""
        campoCorrBarras.erro = ""
        const payload = {
            id: pendenciaAberta.id,
            acao: "nova",
            produtoId: ""
        }
        if (comCorrecao) {
            payload.corrigir = true
            payload.nome = campoCorrNome.texto
            payload.codigoFornecedor = campoCorrCodigo.texto
            payload.codigoBarras = campoCorrBarras.texto
            payload.unidade = campoCorrUnidade.texto
        }
        const r = Api.gravar("xml_pendencia_resolver", payload, chavePendencia)
        if (!r.ok) {
            erro = r.erro
            if (r.campo === "nome")
                campoCorrNome.erro = r.erro
            else if (r.campo === "codigoFornecedor")
                campoCorrCodigo.erro = r.erro
            else if (r.campo === "codigoBarras")
                campoCorrBarras.erro = r.erro
            return
        }
        corrigindoIdentificacao = false
        pendenciaAberta = null
        carregarPendencias()
        janela.avisar(comCorrecao ? "Identificação corrigida, peça cadastrada e associada ao fornecedor."
                                  : "Peça cadastrada e associada ao fornecedor.", "ok")
        painel.importou()
    }

    // R6: descarta a pendência aberta com o motivo; é do gerente (a caixa recebe o pedido da senha).
    function descartarPendencia() {
        if (!pendenciaAberta)
            return
        erro = ""
        motivoDescarte.erro = ""
        janela.chamarGerente("xml_pendencia_resolver",
                             { id: pendenciaAberta.id, acao: "descartar", motivo: motivoDescarte.texto }, r => {
            if (!r.ok) {
                if (r.campo === "motivo")
                    motivoDescarte.erro = r.erro
                else
                    erro = r.erro
                return
            }
            descartando = false
            pendenciaAberta = null
            carregarPendencias()
            janela.avisar("Pendência descartada. O motivo ficou no histórico.", "ok")
        }, chavePendencia)
    }

    function resolverLigando(produtoId) {
        if (!pendenciaAberta || !produtoId)
            return
        erro = ""
        const r = Api.gravar("xml_pendencia_resolver", {
            id: pendenciaAberta.id,
            acao: "ligar",
            produtoId: produtoId
        }, chavePendencia)
        if (!r.ok) {
            erro = r.erro
            // A07: a peça está desativada; a tela oferece abrir a ficha dela.
            pecaDesativadaId = r.campo === "pecaDesativada" ? produtoId : ""
            return
        }
        pecaDesativadaId = ""
        pendenciaAberta = null
        carregarPendencias()
        janela.avisar("Item do XML associado à peça.", "ok")
        painel.importou()
    }

    function ler(lista) {
        arquivos = lista.map(u => Api.caminho(u.toString()))
        escolhas = null
        avisoArquivos = ""
        lerArquivos()
    }
    // Lê (ou lê de novo) os arquivos já escolhidos: só mostra a prévia, não
    // cadastra nada.
    function lerArquivos() {
        erro = ""
        previa = null
        trabalhando = true
        // Deixa a tela mostrar "Lendo..." antes de começar.
        Api.chamarDemorado("importar_previa", dadosDaPrevia(false), "Lendo os arquivos…", r => {
            painel.trabalhando = false
            if (r.ok) {
                painel.previa = r.dados
                const e = painel.escolhas
                // FI-3 (R17): quem já está cadastrado (como fornecedor) passa a ser cliente.
                painel.qClientes = (r.dados.clientesNovos > 0 || r.dados.clientesViram > 0) && (!e || e.qClientes)
                painel.qFornecedores = (r.dados.fornecedoresNovos > 0 || r.dados.fornecedoresViram > 0)
                                       && (!e || e.qFornecedores)
                painel.qPecasLoja = r.dados.pecasLojaNovas > 0 && (!e || e.qPecasLoja)
                painel.qPecasFornecedor = r.dados.pecasFornecedorNovas > 0 && (!e || e.qPecasFornecedor)
                painel.qCompras = r.dados.comprasNovas > 0 && (!e || e.qCompras)
                // A3 da revisão da T108: "Notas de compra em Compras" também exige "Fornecedores" (cada compra é de um
                // fornecedor), como as peças do fornecedor (FI-1).
                if (painel.qCompras)
                    painel.qFornecedores = true
                // FI-1 (a): "Peças das notas de fornecedores" exige "Fornecedores"
                // (as peças ficam ligadas ao fornecedor, cadastrado com elas).
                if (painel.qPecasFornecedor)
                    painel.qFornecedores = true
                // A primeira leitura supõe todas as opções (P-5); com outras
                // marcadas, a prévia é refeita para mostrar o que elas fazem.
                if (!(painel.qClientes && painel.qFornecedores && painel.qPecasLoja && painel.qPecasFornecedor && painel.qCompras))
                    painel.refazerPrevia(false)
                return
            }
            painel.erro = r.erro
            // Falta um requisito (ex.: o CNPJ da loja): a navegação guiada
            // leva ao lugar de resolver. Os arquivos e as escolhas ficam aqui;
            // a janela volta igual quando a pessoa voltar (a senha da tela fica).
            if (painel.janela.guiar(r.campo, r.erro, ok => painel.voltouDaGuia(ok))) {
                painel.naGuia = true
                painel.close()
            }
        })
    }
    // Volta da navegação guiada: reabre como estava. Com o requisito
    // satisfeito, confere se os arquivos continuam no lugar e lê de novo
    // (a pessoa decide se cadastra).
    function voltouDaGuia(satisfeito) {
        naGuia = false
        open()
        if (!satisfeito)
            return
        const c = Api.chamar("arquivos_conferir", { arquivos: painel.arquivos })
        if (c.ok && c.dados.faltam.length > 0) {
            const nomes = c.dados.faltam.map(a => a.split(/[\\/]/).pop()).join(", ")
            painel.arquivos = c.dados.existem
            if (c.dados.existem.length === 0) {
                painel.erro = "Os arquivos escolhidos não estão mais no lugar (" + nomes + "). Escolha de novo."
                return
            }
            painel.lerArquivos()
            painel.avisoArquivos = "Não estão mais no lugar e ficaram de fora: " + nomes + ". Os outros continuam escolhidos."
            return
        }
        painel.lerArquivos()
    }

    function cadastrar() {
        cadastrarAgora()
    }
    function cadastrarAgora() {
        trabalhando = true
        erro = ""
        Qt.callLater(() => {
            // DP-52 (b): a senha dada ao abrir vale para o cadastro (chave da
            // tela); P-4: o carimbo da prévia que a pessoa viu.
            const d = painel.dadosDaPrevia(true)
            d.carimbo = painel.previa ? painel.previa.carimbo : ""
            Api.gravarDemorado("importar_cadastros", d, "Cadastrando…", r => painel.depoisDeImportar(r), painel.chave)
        })
    }
    function depoisDeImportar(r) {
        Qt.callLater(() => {
            painel.trabalhando = false
            if (!r.ok) {
                // A senha da tela venceu (muito tempo parada): pede de novo.
                if (r.campo === "gerente") {
                    painel.liberada = false
                    painel.exigirSenhaDaTela(() => painel.cadastrarAgora(), () => {})
                    return
                }
                painel.erro = r.erro
                // P-4: os arquivos ou os cadastros mudaram: a prévia é refeita.
                if (r.erro.indexOf("mudaram desde a prévia") >= 0)
                    painel.refazerPrevia(true)
                return
            }
            const d = r.dados
            const partes = []
            if (d.clientes > 0) partes.push(d.clientes + (d.clientes === 1 ? " cliente" : " clientes"))
            if (d.fornecedores > 0) partes.push(d.fornecedores + (d.fornecedores === 1 ? " fornecedor" : " fornecedores"))
            if (d.pecas > 0) partes.push(d.pecas + (d.pecas === 1 ? " peça" : " peças"))
            // N5 / T110-P3: só com notas para Compras, sem "Nada novo para cadastrar" e sem espaço inicial.
            let msg = partes.length > 0 ? "Cadastrados: " + partes.join(", ") + "." : d.compras > 0 ? "" : "Nada novo para cadastrar."
            if (d.codigosLigados > 0)
                msg = (msg ? msg + " " : "") + d.codigosLigados + (d.codigosLigados === 1 ? " código de fornecedor ligado às peças." : " códigos de fornecedor ligados às peças.")
            // T108: as notas de compra que foram para Compras.
            if (d.compras > 0)
                msg = (msg ? msg + " " : "") + (d.compras === 1
                    ? "1 nota de compra foi para Compras, em rascunho: abra em Compras para conferir e lançar."
                    : d.compras + " notas de compra foram para Compras, em rascunho: abra cada uma em Compras para conferir e lançar.")
            painel.carregarPendencias()
            // T-05 da 2ª re-revisão: a resposta é tratada depois (Qt.callLater), quando a marca da repetição (I6) já
            // caiu; aqui ela vem da própria resposta.
            if (r.repetida === true)
                Api.repetidaAgora = true
            painel.janela.avisar(msg, "ok")
            painel.close()
            painel.importou()
        })
    }

    function texto(n, um, varios) {
        return n + " " + (n === 1 ? um : varios)
    }

    FileDialog {
        id: seletorArquivos
        title: "Escolha os XMLs das notas (ou um .zip / .rar)"
        fileMode: FileDialog.OpenFiles
        currentFolder: StandardPaths.standardLocations(StandardPaths.DownloadLocation)[0]
        nameFilters: ["XML, ZIP ou RAR (*.xml *.XML *.zip *.ZIP *.rar *.RAR)", "Todos os arquivos (*)"]
        onAccepted: painel.ler(selectedFiles)
    }
    FolderDialog {
        id: seletorPasta
        title: "Escolha a pasta com os XMLs"
        currentFolder: StandardPaths.standardLocations(StandardPaths.DownloadLocation)[0]
        onAccepted: painel.ler([selectedFolder])
    }

    contentItem: Item {
        implicitWidth: coluna.implicitWidth
        // T060 (bug 21, BUG-20261121-K7QX): com muitas notas, os avisos da prévia passavam da altura da janelinha
        // (o Painel a limita à janela) e o Cadastrar ficava fora de alcance. Como no ConflitoCnpj, o miolo rola
        // e a linha do Cadastrar fica embaixo, sempre à vista.
        implicitHeight: externa.implicitHeight

        DropArea {
            anchors.fill: parent
            keys: ["text/uri-list"]
            onDropped: (evento) => {
                if (evento.hasUrls)
                    painel.ler(evento.urls)
            }
        }

        ColumnLayout {
            id: externa
            anchors.fill: parent
            spacing: 14

            Flickable {
                id: rolagemDoImportar
                // A geometria do miolo para os roteiros (GEO:rolagemDoImportar).
                objectName: "rolagemDoImportar"
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.preferredHeight: coluna.implicitHeight
                contentHeight: coluna.implicitHeight
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar {}

                ColumnLayout {
                    id: coluna
                    width: rolagemDoImportar.width
                    spacing: 14

                    Pilulas {
                        visible: painel.pendencias.length > 0 || painel.aba === "pendencias"
                        pequenas: true
                        modelo: [
                            { id: "importar", nome: "Trazer cadastros" },
                            { id: "pendencias", nome: "Pendências de identificação" + (painel.pendencias.length > 0 ? " (" + painel.pendencias.length + ")" : "") }
                        ]
                        atual: painel.aba
                        onEscolhido: (id) => {
                            painel.erro = ""
                            if (id === "pendencias") {
                                painel.aba = id
                                painel.carregarPendencias()
                                return
                            }
                            // A aba de importar pede a senha (se precisar) antes.
                            painel.exigirSenhaDaTela(() => painel.aba = id, () => {})
                        }
                    }

                    Titulo {
                        text: painel.aba === "pendencias" ? "Pendências de identificação XML" : "Trazer cadastros pelo XML"
                        font.pixelSize: 32
                        Layout.fillWidth: true
                    }
                    Nota {
                        visible: painel.naImportacao
                        text: "Use os XMLs das notas emitidas pela loja (exportados do sistema antigo) para trazer clientes e peças com o código e o preço da loja, "
                              + "e os das notas de fornecedores para trazer fornecedores e peças com o custo. Nas peças já cadastradas, só entram GTIN e código do fornecedor, e a prévia mostra quais. "
                              + "As notas dos fornecedores (as de compra) também podem ir para Compras, em rascunho: o estoque e o financeiro só mudam quando cada uma for lançada lá."
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }

                    Linha {
                        visible: painel.naImportacao
                        spacing: 10
                        Layout.fillWidth: true
                        Botao {
                            text: "Escolher arquivos"
                            icone: "mais"
                            tipo: painel.previa ? "secundario" : "principal"
                            enabled: !painel.trabalhando
                            onClicked: seletorArquivos.open()
                        }
                        Botao {
                            text: "Escolher uma pasta"
                            visible: !Tema.celular
                            tipo: "secundario"
                            enabled: !painel.trabalhando
                            onClicked: seletorPasta.open()
                        }
                        Item { Layout.fillWidth: true }
                    }
                    Nota {
                        visible: painel.naImportacao && !painel.previa && !painel.trabalhando && painel.erro === ""
                        text: "Dá para escolher vários .xml, um .zip ou .rar com os XMLs dentro ou uma pasta inteira (com subpastas). Também dá para arrastar para cá."
                        font.pixelSize: 13
                        Layout.fillWidth: true
                    }

                    Text {
                        visible: painel.naImportacao && painel.trabalhando
                        text: "Lendo as notas..."
                        color: Tema.tintaSuave
                        font.family: Tema.serifa
                        font.italic: true
                        font.pixelSize: 16
                    }
                    // Os arquivos já escolhidos (continuam escolhidos quando a leitura
                    // não deu: dá para tentar de novo sem escolher outra vez).
                    Linha {
                        visible: painel.naImportacao && painel.arquivos.length > 0 && !painel.previa && !painel.trabalhando
                        spacing: 10
                        Layout.fillWidth: true
                        Nota {
                            text: painel.texto(painel.arquivos.length, "arquivo escolhido", "arquivos escolhidos") + ": "
                                  + painel.arquivos.map(a => a.split(/[\\/]/).pop()).join(", ")
                            font.pixelSize: 13
                            elide: Text.ElideRight
                            maximumLineCount: 2
                            Layout.fillWidth: true
                        }
                        Botao {
                            text: "Tentar de novo"
                            tipo: "secundario"
                            onClicked: painel.lerArquivos()
                        }
                    }

                    // Prévia
                    ColumnLayout {
                        visible: painel.naImportacao && !!painel.previa && !painel.trabalhando
                        spacing: 8
                        Layout.fillWidth: true

                        Rotulo {
                            text: painel.previa ? painel.texto(painel.previa.notas, "nota lida", "notas lidas")
                                                  + "  ·  " + painel.previa.notasLoja + " da loja  ·  " + painel.previa.notasFornecedor + " de fornecedores"
                                                  + (painel.previa.ignorados > 0 ? "  ·  " + painel.texto(painel.previa.ignorados, "arquivo ignorado", "arquivos ignorados") : "") : ""
                            Layout.fillWidth: true
                        }

                        Repeater {
                            model: painel.previa ? [
                                { chave: "qClientes", titulo: "Clientes", novos: painel.previa.clientesNovos, ja: painel.previa.clientesJa,
                                  // M1 da revisão: `mudam` contado com todas as opções (a caixa não some ao desmarcar).
                                  mudam: painel.previa.clientesViram, exemplos: painel.previa.exemplosClientes, cor: Tema.verde,
                                  // FI-3 (R17): quem já está cadastrado (como fornecedor) passa a ser cliente.
                                  extra: painel.qClientes && painel.previa.clientesViram > 0
                                         ? painel.texto(painel.previa.clientesViram, "cadastro já existente passa", "cadastros já existentes passam")
                                           + " a ser cliente" : "" },
                                { chave: "qFornecedores", titulo: "Fornecedores", novos: painel.previa.fornecedoresNovos, ja: painel.previa.fornecedoresJa,
                                  mudam: painel.previa.fornecedoresViram, exemplos: painel.previa.exemplosFornecedores, cor: Tema.azul,
                                  // FI-2: quem já está cadastrado (como cliente) passa a ser fornecedor.
                                  extra: painel.qFornecedores && painel.previa.fornecedoresViram > 0
                                         ? painel.texto(painel.previa.fornecedoresViram, "cadastro já existente passa", "cadastros já existentes passam")
                                           + " a ser fornecedor" : "",
                                  // FI-1 (a): travada enquanto "Peças das notas de fornecedores" estiver marcada.
                                  presa: painel.qPecasFornecedor || painel.qCompras,
                                  porque: painel.qPecasFornecedor
                                          ? "Marcada junto com \"Peças das notas de fornecedores\": as peças ficam ligadas ao fornecedor, que é cadastrado com elas."
                                          : "Marcada junto com \"Notas de compra em Compras\": cada compra é de um fornecedor." },
                                { chave: "qPecasLoja", titulo: "Peças das notas da loja", novos: painel.previa.pecasLojaNovas, ja: -1,
                                  mudam: painel.cat("loja.ja_gtin_novo"), exemplos: [], cor: Tema.papel },
                                { chave: "qPecasFornecedor", titulo: "Peças das notas de fornecedores", novos: painel.previa.pecasFornecedorNovas, ja: -1,
                                  mudam: painel.cat("forn.ja_pelo_gtin_liga") + painel.cat("forn.ja_pelo_vinculo_gtin_novo"),
                                  exemplos: [], cor: Tema.papel },
                                // T108 (texto R): as notas de compra vão para Compras, em rascunho; as que já estão lá não
                                // entram de novo.
                                { chave: "qCompras", titulo: "Notas de compra em Compras", novos: painel.previa.comprasNovas,
                                  ja: painel.previa.comprasJa, jaUm: " já estava em Compras", jaTexto: " já estavam em Compras",
                                  novoUm: "nova", novoVarios: "novas",
                                  naoServe: painel.previa.comprasNaoServe,
                                  extra: [painel.previa.comprasNovas > 0
                                          ? "Entram em rascunho: o estoque e o A pagar só mudam quando cada uma for lançada em Compras. "
                                            + "Se estas notas já foram lançadas no sistema antigo, desmarque." : "",
                                          // A8 da revisão da T108: as que não servem para Compras não somem sem aviso.
                                          painel.previa.comprasNaoServe > 0
                                          ? painel.texto(painel.previa.comprasNaoServe, "nota de fornecedor não serve", "notas de fornecedor não servem")
                                            + " para Compras (sem a autorização da SEFAZ, para outro CNPJ, devolução, NFC-e ou outro motivo): só os cadastros." : ""]
                                         .filter(x => !!x).join(" "),
                                  exemplos: [], cor: Tema.papel }
                            ] : []
                            delegate: Rectangle {
                                id: linha
                                required property var modelData
                                visible: modelData.novos > 0 || modelData.ja > 0 || (modelData.mudam || 0) > 0 || (modelData.naoServe || 0) > 0
                                Layout.fillWidth: true
                                implicitHeight: coluna.implicitHeight + 20
                                radius: Tema.raio
                                color: modelData.novos > 0 ? modelData.cor : Tema.vela(0.35)
                                palette.window: Tema.superficie(color)
                                ColumnLayout {
                                    id: coluna
                                    x: 14
                                    y: 10
                                    width: parent.width - 28
                                    spacing: 2
                                    RowLayout {
                                        spacing: 6
                                        Layout.fillWidth: true
                                        Marcar {
                                            visible: linha.modelData.novos > 0 || (linha.modelData.mudam || 0) > 0 || !!linha.modelData.presa
                                            enabled: !linha.modelData.presa
                                            checked: painel[linha.modelData.chave]
                                            onToggled: {
                                                painel[linha.modelData.chave] = checked
                                                // FI-1 (a): as peças do fornecedor levam o fornecedor junto.
                                                if (linha.modelData.chave === "qPecasFornecedor" && checked)
                                                    painel.qFornecedores = true
                                                // A3 da revisão da T108: as notas de compra também.
                                                if (linha.modelData.chave === "qCompras" && checked)
                                                    painel.qFornecedores = true
                                                painel.escolhas = { qClientes: painel.qClientes, qFornecedores: painel.qFornecedores,
                                                                    qPecasLoja: painel.qPecasLoja, qPecasFornecedor: painel.qPecasFornecedor,
                                                                    qCompras: painel.qCompras }
                                                painel.refazerPrevia(false)
                                            }
                                            leftPadding: 0
                                        }
                                        Text {
                                            text: linha.modelData.titulo
                                            textFormat: Text.PlainText
                                            color: Tema.tinta
                                            font.family: Tema.titulo
                                            font.weight: Font.Bold
                                            font.pixelSize: 20
                                            Layout.fillWidth: true
                                            wrapMode: Text.WordWrap
                                        }
                                        Text {
                                            text: linha.modelData.novos > 0 ? painel.texto(linha.modelData.novos, linha.modelData.novoUm || "novo",
                                                                                           linha.modelData.novoVarios || "novos") : "nada novo"
                                            textFormat: Text.PlainText
                                            color: linha.modelData.novos > 0 ? Tema.verdeEscuro : Tema.apagado
                                            font.family: Tema.texto
                                            font.weight: Font.Bold
                                            font.pixelSize: 15
                                        }
                                    }
                                    Text {
                                        visible: text !== ""
                                        text: [linha.modelData.ja > 0 ? linha.modelData.ja + (linha.modelData.ja === 1 && linha.modelData.jaUm
                                                                                              ? linha.modelData.jaUm : (linha.modelData.jaTexto || " já cadastrados")) : "",
                                               linha.modelData.exemplos.length > 0 ? linha.modelData.exemplos.join(", ") + (linha.modelData.novos > linha.modelData.exemplos.length ? "..." : "") : ""]
                                              .filter(x => !!x).join("  ·  ")
                                        textFormat: Text.PlainText
                                        color: Tema.tintaSuave
                                        font.family: Tema.serifa
                                        font.italic: true
                                        font.pixelSize: 13
                                        elide: Text.ElideRight
                                        Layout.fillWidth: true
                                    }
                                    Text {
                                        visible: text !== ""
                                        text: [linha.modelData.extra || "", linha.modelData.presa ? linha.modelData.porque : ""]
                                              .filter(x => !!x).join("  ·  ")
                                        textFormat: Text.PlainText
                                        color: Tema.tinta
                                        font.family: Tema.texto
                                        font.pixelSize: 13
                                        wrapMode: Text.WordWrap
                                        Layout.fillWidth: true
                                    }
                                }
                            }
                        }

                        // A03b e O-3 (com as opções marcadas): o que muda no que já existe
                        // e o que fica para identificar depois.
                        Nota {
                            visible: !!painel.previa && painel.previa.pecasQueMudam > 0
                            text: painel.previa ? "Peças que já existem e mudam (GTIN ou código do fornecedor): " + painel.previa.pecasQueMudam
                                  + (painel.previa.exemplosPecasQueMudam.length > 0
                                     ? " — " + painel.previa.exemplosPecasQueMudam.join(", ")
                                       + (painel.previa.pecasQueMudam > painel.previa.exemplosPecasQueMudam.length ? "..." : "")
                                     : "") : ""
                            font.pixelSize: 13
                            Layout.fillWidth: true
                        }
                        Nota {
                            visible: !!painel.previa && painel.previa.vaoParaPendencias > 0
                            text: painel.previa ? "Vão para pendências: " + painel.previa.vaoParaPendencias + " — "
                                  + painel.previa.pendenciasPorMotivo.map(m => m.texto + ": " + m.quantidade).join("; ") : ""
                            font.pixelSize: 13
                            Layout.fillWidth: true
                        }
                        Nota {
                            visible: !!painel.previa && painel.previa.pecasLojaNovas > 0 && painel.previa.pecasFornecedorNovas > 0
                            text: "Peça sem código de barras pode ficar repetida (uma com o código da loja, outra do fornecedor). Se for migrar de outro sistema, prefira só as notas da loja."
                            font.pixelSize: 13
                            Layout.fillWidth: true
                        }
                        Repeater {
                            model: painel.previa ? painel.previa.avisos : []
                            delegate: Text {
                                required property var modelData
                                text: modelData
                                textFormat: Text.PlainText
                                color: Tema.apagado
                                font.family: Tema.texto
                                font.pixelSize: 12
                                // T060: o aviso inteiro, em mais linhas (cortado no meio, não dava para ler o motivo).
                                wrapMode: Text.Wrap
                                Layout.fillWidth: true
                            }
                        }
                    }

                    // Pendências de identificação XML (Fase 4.D)
                    ColumnLayout {
                        visible: painel.aba === "pendencias"
                        spacing: 10
                        Layout.fillWidth: true

                        Nota {
                            visible: painel.pendencias.length === 0
                            text: "Nenhuma pendência de identificação XML aberta no momento."
                            font.pixelSize: 15
                            Layout.fillWidth: true
                        }

                        // Lista de pendências abertas
                        ListView {
                            id: listaPendencias
                            visible: !painel.pendenciaAberta && painel.pendencias.length > 0
                            Layout.fillWidth: true
                            implicitHeight: Math.min(360, contentHeight)
                            clip: true
                            spacing: 6
                            model: painel.pendencias
                            boundsBehavior: Flickable.StopAtBounds; delegate: Rectangle {
                                id: cartaoPend
                                required property var modelData
                                required property int index
                                readonly property bool par: (index + 1) % 2 === 0
                                width: listaPendencias.width
                                implicitHeight: colPend.implicitHeight + 20
                                radius: Tema.raio
                                color: Tema.faixa(par)
                                palette.window: Tema.superficie(color)
                                RowLayout {
                                    id: colPend
                                    x: 14
                                    y: 10
                                    width: parent.width - 28
                                    spacing: 12
                                    ColumnLayout {
                                        spacing: 2
                                        Layout.fillWidth: true
                                        Text {
                                            text: cartaoPend.modelData.descricao || ""
                                            textFormat: Text.PlainText
                                            color: Tema.tintaDaFaixa(cartaoPend.par)
                                            font.family: Tema.texto
                                            font.weight: Font.Bold
                                            font.pixelSize: 16
                                            elide: Text.ElideRight
                                            Layout.fillWidth: true
                                        }
                                        Text {
                                            text: [
                                                "Cód. forn.: " + (cartaoPend.modelData.codigoFornecedor || "—"),
                                                cartaoPend.modelData.fornecedorNome || cartaoPend.modelData.fornecedorDocumento,
                                                cartaoPend.modelData.gtinComercial ? "GTIN: " + cartaoPend.modelData.gtinComercial + " (" + cartaoPend.modelData.origemComercial + ")" : "SEM GTIN"
                                            ].filter(x => !!x).join("  ·  ")
                                            textFormat: Text.PlainText
                                            color: Tema.tintaSuaveDaFaixa(cartaoPend.par)
                                            font.family: Tema.texto
                                            font.pixelSize: 13
                                            elide: Text.ElideRight
                                            Layout.fillWidth: true
                                        }
                                        Text {
                                            text: cartaoPend.modelData.motivoTexto || ""
                                            textFormat: Text.PlainText
                                            // H04 (7.2): o vermelho da faixa (claro na faixa escura do roxo).
                                            color: Tema.vermelhoDaFaixa(cartaoPend.par)
                                            font.family: Tema.texto
                                            font.pixelSize: 12
                                            wrapMode: Text.WordWrap
                                            Layout.fillWidth: true
                                        }
                                    }
                                    Botao {
                                        text: "Abrir pendência"
                                        tipo: "secundario"
                                        Layout.alignment: Qt.AlignVCenter
                                        onClicked: painel.abrirPendencia(cartaoPend.modelData)
                                    }
                                }
                            }
                        }

                        // Detalhe da pendência aberta com todos os dados preservados do XML
                        Rectangle {
                            visible: !!painel.pendenciaAberta
                            Layout.fillWidth: true
                            implicitHeight: detCol.implicitHeight + 28
                            radius: Tema.raio
                            color: Tema.pergaminho
                            palette.window: Tema.superficie(color)
                            ColumnLayout {
                                id: detCol
                                x: 16
                                y: 14
                                width: parent.width - 32
                                spacing: 8
                                Text {
                                    text: painel.pendenciaAberta ? painel.pendenciaAberta.descricao : ""
                                    textFormat: Text.PlainText
                                    color: Tema.tintaNo(Tema.pergaminho)
                                    font.family: Tema.titulo
                                    font.weight: Font.Bold
                                    font.pixelSize: 20
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                                Text {
                                    visible: !!painel.pendenciaAberta && !!painel.pendenciaAberta.motivoTexto
                                    text: painel.pendenciaAberta ? painel.pendenciaAberta.motivoTexto : ""
                                    textFormat: Text.PlainText
                                    color: Tema.vermelho
                                    font.family: Tema.texto
                                    font.weight: Font.DemiBold
                                    font.pixelSize: 13
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                                Text {
                                    text: !painel.pendenciaAberta ? "" : [
                                        "Fornecedor: " + (painel.pendenciaAberta.fornecedorNome || "—") + " · CNPJ " + (painel.pendenciaAberta.fornecedorDocumento || "—"),
                                        "NF-e: " + (painel.pendenciaAberta.numeroNfe || "—")
                                            + (painel.pendenciaAberta.emissaoNfe ? " · emissão " + painel.pendenciaAberta.emissaoNfe : "")
                                            + (painel.pendenciaAberta.chaveNfe ? " · chave " + painel.pendenciaAberta.chaveNfe : ""),
                                        "Cód. fornecedor (cProd): " + (painel.pendenciaAberta.codigoFornecedor || "—"),
                                        "GTIN comercial: " + (painel.pendenciaAberta.gtinComercial
                                            ? painel.pendenciaAberta.gtinComercial + " (" + painel.pendenciaAberta.origemComercial + ")"
                                            : "SEM GTIN")
                                            + "  ·  GTIN tributável: " + (painel.pendenciaAberta.gtinTributavel
                                            ? painel.pendenciaAberta.gtinTributavel + " (" + painel.pendenciaAberta.origemTributavel + ")"
                                            : "SEM GTIN"),
                                        "NCM: " + (painel.pendenciaAberta.ncm || "—")
                                            + (painel.pendenciaAberta.cest ? " · CEST: " + painel.pendenciaAberta.cest : "")
                                            + (painel.pendenciaAberta.cfop ? " · CFOP: " + painel.pendenciaAberta.cfop : ""),
                                        "Quantidade: " + (painel.pendenciaAberta.quantidade || "1") + " " + (painel.pendenciaAberta.unidade || "UN")
                                            + "  ·  Valor unitário: R$ " + (painel.pendenciaAberta.valorUnitario || "0,00")
                                    ].join("\n")
                                    textFormat: Text.PlainText
                                    color: Tema.tintaNo(Tema.pergaminho)
                                    font.family: Tema.texto
                                    font.pixelSize: 13
                                    lineHeight: 1.25
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }

                                Rotulo {
                                    visible: !!painel.pendenciaAberta && (painel.pendenciaAberta.candidatos || []).length > 0
                                    text: "Peças candidatas no cadastro"
                                    color: Tema.roxo ? Tema.apagadoNo(Tema.pergaminho) : Tema.apagado
                                }

                                ListView {
                                    id: listaCands
                                    visible: !!painel.pendenciaAberta && (painel.pendenciaAberta.candidatos || []).length > 0
                                    Layout.fillWidth: true
                                    implicitHeight: Math.min(180, contentHeight)
                                    clip: true
                                    spacing: 4
                                    model: painel.pendenciaAberta ? (painel.pendenciaAberta.candidatos || []) : []
                                    boundsBehavior: Flickable.StopAtBounds; delegate: Rectangle {
                                        id: candItem
                                        required property var modelData
                                        required property int index
                                        readonly property bool par: (index + 1) % 2 === 0
                                        width: listaCands.width
                                        implicitHeight: candLinha.implicitHeight + 14
                                        radius: Tema.raioCampo
                                        color: Tema.faixa(par)
                                        palette.window: Tema.superficie(color)
                                        RowLayout {
                                            id: candLinha
                                            x: 10
                                            y: 7
                                            width: parent.width - 20
                                            spacing: 10
                                            ColumnLayout {
                                                spacing: 1
                                                Layout.fillWidth: true
                                                Text {
                                                    text: candItem.modelData.nome || ""
                                                    textFormat: Text.PlainText
                                                    color: Tema.tintaDaFaixa(candItem.par)
                                                    font.family: Tema.texto
                                                    font.weight: Font.Bold
                                                    font.pixelSize: 14
                                                    elide: Text.ElideRight
                                                    Layout.fillWidth: true
                                                }
                                                Text {
                                                    text: [
                                                        "Interno: " + (candItem.modelData.codigo || "a definir"),
                                                        candItem.modelData.marca,
                                                        candItem.modelData.codigoBarras ? "GTIN: " + candItem.modelData.codigoBarras : ""
                                                    ].filter(x => !!x).join("  ·  ")
                                                    textFormat: Text.PlainText
                                                    color: Tema.tintaSuaveDaFaixa(candItem.par)
                                                    font.family: Tema.texto
                                                    font.pixelSize: 12
                                                    elide: Text.ElideRight
                                                    Layout.fillWidth: true
                                                }
                                            }
                                            Botao {
                                                text: "Associar a esta peça"
                                                tipo: "principal"
                                                onClicked: painel.resolverLigando(candItem.modelData.id)
                                            }
                                        }
                                    }
                                }

                                // T111 (Regra 3): corrigir apenas a identificação sem redigitar NCM/CEST/origem/custo.
                                ColumnLayout {
                                    visible: painel.corrigindoIdentificacao
                                    Layout.fillWidth: true
                                    Layout.topMargin: 4
                                    spacing: 8
                                    Nota {
                                        text: "Corrija apenas o que precisa. NCM, CEST, origem e custo do XML continuam preenchidos automaticamente."
                                        font.pixelSize: 13
                                        Layout.fillWidth: true
                                    }
                                    Campo {
                                        id: campoCorrNome
                                        rotulo: "Nome da peça"
                                        maximo: 120
                                        Layout.fillWidth: true
                                    }
                                    RowLayout {
                                        spacing: 10
                                        Layout.fillWidth: true
                                        Campo {
                                            id: campoCorrCodigo
                                            rotulo: "Código no fornecedor"
                                            maximo: 60
                                            Layout.fillWidth: true
                                        }
                                        Campo {
                                            id: campoCorrUnidade
                                            rotulo: "Unidade"
                                            maximo: 6
                                            maiusculas: true
                                            Layout.preferredWidth: 100
                                        }
                                    }
                                    Campo {
                                        id: campoCorrBarras
                                        rotulo: "Código de barras (GTIN, opcional)"
                                        maximo: 14
                                        dica: "Deixe vazio se o produto não tiver GTIN"
                                        Layout.fillWidth: true
                                    }
                                }

                                Flow {
                                    spacing: 10
                                    Layout.fillWidth: true
                                    Layout.topMargin: 4
                                    layoutDirection: Qt.RightToLeft
                                    // T058: o "Todas as pendências" saiu; a seta e o Esc
                                    // voltam à lista (voltarUmPasso, lá em cima).
                                    Botao {
                                        text: painel.corrigindoIdentificacao ? "Salvar e cadastrar peça" : "Cadastrar como peça nova"
                                        icone: painel.corrigindoIdentificacao ? "check" : "mais"
                                        tipo: "principal"
                                        onClicked: painel.resolverComoNova(painel.corrigindoIdentificacao)
                                    }
                                    Botao {
                                        text: "Escolher peça existente"
                                        tipo: "secundario"
                                        onClicked: escolherPecaParaPendencia.abrirCom(painel.pendenciaAberta ? painel.pendenciaAberta.descricao : "")
                                    }
                                    Botao {
                                        text: painel.corrigindoIdentificacao ? "Usar dados do XML" : "Corrigir identificação"
                                        tipo: "secundario"
                                        onClicked: {
                                            painel.corrigindoIdentificacao = !painel.corrigindoIdentificacao
                                            painel.erro = ""
                                            campoCorrNome.erro = ""
                                            campoCorrCodigo.erro = ""
                                            campoCorrBarras.erro = ""
                                        }
                                    }
                                    // R6: descartar, com o motivo (o mesmo botão desiste).
                                    Botao {
                                        objectName: "descartarPendencia"
                                        text: painel.descartando ? "Não descartar" : "Descartar"
                                        tipo: "discreto"
                                        onClicked: {
                                            painel.descartando = !painel.descartando
                                            painel.erro = ""
                                            motivoDescarte.erro = ""
                                            if (painel.descartando)
                                                motivoDescarte.focar()
                                        }
                                    }
                                }

                                // R6 (D-AUD-5): o motivo do descarte.
                                ColumnLayout {
                                    visible: painel.descartando && !!painel.pendenciaAberta
                                    spacing: 8
                                    Layout.fillWidth: true
                                    Nota {
                                        text: "A pendência sai da lista. O que ela dizia e o motivo ficam no histórico."
                                        Layout.fillWidth: true
                                    }
                                    Campo {
                                        id: motivoDescarte
                                        rotulo: "Motivo do descarte"
                                        maximo: 200
                                        dica: "Ex.: uso e consumo, brinde do fornecedor"
                                        Layout.fillWidth: true
                                        onConfirmado: painel.descartarPendencia()
                                    }
                                    Flow {
                                        spacing: 10
                                        Layout.fillWidth: true
                                        layoutDirection: Qt.RightToLeft
                                        Botao {
                                            objectName: "confirmarDescarte"
                                            text: "Descartar a pendência"
                                            tipo: "principal"
                                            onClicked: painel.descartarPendencia()
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // T077 (AUD-X-1): o erro, o aviso dos arquivos e o "Abrir a peça" ficam fora da rolagem, logo
            // acima do Cadastrar (como no Vender e no A pagar): quem rolou até o fim e apertou o Cadastrar vê
            // a recusa (ex.: a prévia vencida).
            TextoErro { text: painel.erro; Layout.fillWidth: true }
            // A07: a peça escolhida está desativada; dá para abrir a ficha e reativá-la.
            Botao {
                visible: painel.aba === "pendencias" && painel.pecaDesativadaId !== ""
                text: "Abrir a peça"
                tipo: "secundario"
                // Fecha o painel antes: a ficha abre por baixo dele e precisa ficar à vista
                // (é lá que a peça é reativada); a pendência continua aberta.
                onClicked: {
                    const id = painel.pecaDesativadaId
                    painel.pecaDesativadaId = ""
                    painel.close()
                    painel.janela.abrir("TelaPeca", { produtoId: id })
                }
            }
            Nota {
                visible: painel.naImportacao && painel.avisoArquivos !== ""
                text: painel.avisoArquivos
                color: Tema.vermelho
                font.pixelSize: 13
                Layout.fillWidth: true
            }
            RowLayout {
                visible: painel.naImportacao
                spacing: 10
                Layout.fillWidth: true
                Layout.topMargin: 6
                Item { Layout.fillWidth: true }
                Botao {
                    visible: !!painel.previa
                    text: "Cadastrar"
                    icone: "check"
                    enabled: !painel.trabalhando && (painel.qClientes || painel.qFornecedores || painel.qPecasLoja || painel.qPecasFornecedor
                                                     || painel.qCompras)
                    onClicked: painel.cadastrar()
                }
            }
        }

        // PEND-38/H02: o pedido da senha da tela, por cima do painel aberto.
        PedirGerente { id: pedirSenhaTela }

        EscolherPeca {
            id: escolherPecaParaPendencia
            onEscolhida: (p) => painel.resolverLigando(p.id)
        }
    }
}
