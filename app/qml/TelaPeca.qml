import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Cadastro de uma peça: uma tela só, em faixas, sem abas.
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    property string produtoId: ""
    property var ficha: null
    property var precos: []
    property var sugestoes: ({ marcas: [], categorias: [], montadoras: [], modelos: [], locais: [], unidades: [], origens: [], csosns: [] })
    property var tributacao: null
    property bool alterado: false
    property bool carregando: true
    property bool mostrarOutros: false
    // D-09: custo e % de lucro só para quem vê os resultados (vem da ficha).
    readonly property bool verCustos: !!ficha && !!ficha.verCustos
    readonly property bool nova: !ficha || ficha.id === ""
    property string tipo: "produto"
    readonly property bool servico: tipo === "servico"
    readonly property bool temGrade: !!ficha && !!ficha.gradeItens && ficha.gradeItens.length > 0
    // Parte do fiscal à mostra (pílulas): icms, pis, ipi, combustivel, servico.
    property string blocoFiscal: "icms"

    ListModel { id: aplicacoes }
    ListModel { id: equivalentes }

    // ---------- Carregar e salvar ----------

    function carregar() {
        carregando = true
        const s = Api.chamar("produto_sugestoes")
        if (s.ok)
            sugestoes = s.dados
        const r = produtoId ? Api.chamar("produto_abrir", { id: produtoId }) : Api.chamar("produto_novo")
        if (!r.ok) {
            janela.avisar(r.erro, "erro")
            janela.voltar()
            return
        }
        preencher(r.dados)
        carregando = false
    }

    function preencher(f) {
        ficha = f
        nome.texto = f.nome
        codigo.texto = f.codigo
        codigoBarras.texto = f.codigoBarras
        codigoOriginal.texto = f.codigoOriginal
        marca.texto = f.marca
        unidade.texto = f.unidade
        categoria.texto = f.categoria
        tipo = f.tipo || "produto"
        blocoFiscal = tipo === "servico" ? "servico" : "icms"
        variacao.texto = f.variacao
        precoAberto.checked = f.precoAberto
        emOferta.checked = f.ofertaPreco !== ""
        ofertaPreco.texto = f.ofertaPreco
        ofertaAte.texto = f.ofertaAte
        preencherFiscal(f.fiscal || {})
        ativo.checked = f.ativo
        campoCusto.texto = f.custo
        precos = f.precos
        mostrarOutros = f.precos.some(p => p.tabela > 1 && p.preco !== "")
        estoqueInicial.texto = f.estoqueInicial
        estoqueMinimo.texto = f.estoqueMinimo
        local.texto = f.local
        ncm.texto = f.ncm
        cest.texto = f.cest
        origem.indice = f.origem
        csosn.indice = Math.max(0, (sugestoes.csosns || []).findIndex(c => c.valor === f.csosn))
        monofasico.checked = f.monofasico
        tributacao = f.tributacao
        anpCodigo.texto = f.anpCodigo
        anpDescricao.texto = f.anpDescricao
        peso.texto = f.peso
        descontoMaximo.texto = f.descontoMaximo
        comissao.texto = f.comissao
        observacoes.texto = f.observacoes
        aplicacoes.clear()
        for (const a of f.aplicacoes)
            aplicacoes.append({ montadora: a.montadora, modelo: a.modelo, motor: a.motor, anoInicio: a.anoInicio, anoFim: a.anoFim, observacao: a.observacao })
        equivalentes.clear()
        for (const q of f.equivalentes)
            equivalentes.append({ marca: q.marca, codigo: q.codigo })
        limparErros()
        alterado = false
    }

    function coletar() {
        const listaPrecos = []
        for (let i = 0; i < repPrecos.count; i++) {
            const linha = repPrecos.itemAt(i)
            listaPrecos.push({ tabela: linha.tabela, lucro: linha.lucroTexto, preco: linha.precoTexto })
        }
        const listaAplicacoes = []
        for (let i = 0; i < aplicacoes.count; i++) {
            const a = aplicacoes.get(i)
            listaAplicacoes.push({ montadora: a.montadora, modelo: a.modelo, motor: a.motor, anoInicio: a.anoInicio, anoFim: a.anoFim, observacao: a.observacao })
        }
        const listaEquivalentes = []
        for (let i = 0; i < equivalentes.count; i++) {
            const q = equivalentes.get(i)
            listaEquivalentes.push({ marca: q.marca, codigo: q.codigo })
        }
        return {
            id: ficha ? ficha.id : "",
            codigo: codigo.texto,
            nome: nome.texto,
            marca: marca.texto,
            unidade: unidade.texto,
            codigoBarras: codigoBarras.texto,
            codigoOriginal: codigoOriginal.texto,
            categoria: categoria.texto,
            ativo: ativo.checked,
            custo: campoCusto.texto,
            precos: listaPrecos,
            estoqueInicial: estoqueInicial.texto,
            estoqueMinimo: estoqueMinimo.texto,
            local: local.texto,
            ncm: ncm.texto,
            cest: cest.texto,
            origem: Math.max(0, origem.indice),
            csosn: tela.csosnEscolhido(),
            monofasico: monofasico.checked,
            anpCodigo: anpCodigo.texto,
            anpDescricao: anpDescricao.texto,
            tipo: tela.tipo,
            variacao: variacao.texto,
            precoAberto: precoAberto.checked,
            ofertaPreco: emOferta.checked ? ofertaPreco.texto : "",
            ofertaAte: emOferta.checked ? ofertaAte.texto : "",
            fiscal: coletarFiscal(),
            peso: peso.texto,
            descontoMaximo: descontoMaximo.texto,
            comissao: comissao.texto,
            observacoes: observacoes.texto,
            aplicacoes: listaAplicacoes,
            equivalentes: listaEquivalentes
        }
    }

    // Campos do fiscal da nota: [campo, chave, bloco, lista?]. "marcar" = caixa
    // de marcar; "uf" = lista dos estados; outra lista = CSTs.
    function camposFiscais() {
        return [
            [ipiCst, "ipiCst", "ipi", "cstIpi"], [ipiAliquota, "ipiAliquota", "ipi"], [ipiValor, "ipiValor", "ipi"],
            [ipiEnquadramento, "ipiEnquadramento", "ipi"], [ipiSelo, "ipiSelo", "ipi"], [ipiSeloQuantidade, "ipiSeloQuantidade", "ipi"],
            [pisCst, "pisCst", "pis", "cstPis"], [pisAliquota, "pisAliquota", "pis"], [pisValor, "pisValor", "pis"],
            [cofinsCst, "cofinsCst", "pis", "cstPis"], [cofinsAliquota, "cofinsAliquota", "pis"], [cofinsValor, "cofinsValor", "pis"],
            [naturezaReceita, "naturezaReceita", "pis"],
            [glp, "glp", "combustivel"], [gnNacional, "gnNacional", "combustivel"], [gnImportado, "gnImportado", "combustivel"],
            [valorPartida, "valorPartida", "combustivel"], [codif, "codif", "combustivel"], [temperatura, "temperatura", "combustivel"],
            [importado, "importado", "combustivel", "marcar"], [ufOrigem, "ufOrigem", "combustivel", "uf"],
            [percentualOrigem, "percentualOrigem", "combustivel"], [cide, "cide", "combustivel"], [biodiesel, "biodiesel", "combustivel"],
            [servicoCodigo, "servicoCodigo", "servico"], [cnae, "cnae", "servico"], [nbs, "nbs", "servico"],
            [codigoMunicipio, "codigoMunicipio", "servico"], [issAliquota, "issAliquota", "servico"],
            [issExigibilidade, "issExigibilidade", "servico", "exigibilidades"], [incentivoFiscal, "incentivoFiscal", "servico", "marcar"],
            [servicoDescricao, "servicoDescricao", "servico"]
        ]
    }
    function ufsComVazio() {
        return [""].concat(sugestoes.ufs || [])
    }
    function preencherFiscal(fi) {
        for (const [c, k, , lista] of camposFiscais()) {
            if (lista === "marcar")
                c.checked = !!fi[k]
            else if (lista === "uf")
                c.indice = Math.max(0, ufsComVazio().indexOf(fi[k] || ""))
            else if (lista)
                c.indice = Math.max(0, (sugestoes[lista] || []).findIndex(x => x.valor === (fi[k] || (lista === "exigibilidades" ? "1" : ""))))
            else
                c.texto = fi[k] || ""
        }
    }
    function coletarFiscal() {
        const fi = {}
        for (const [c, k, , lista] of camposFiscais()) {
            if (lista === "marcar")
                fi[k] = c.checked
            else if (lista === "uf")
                fi[k] = ufsComVazio()[Math.max(0, c.indice)] || ""
            else if (lista) {
                const l = sugestoes[lista] || []
                fi[k] = c.indice >= 0 && c.indice < l.length ? l[c.indice].valor : ""
            } else
                fi[k] = c.texto
        }
        return fi
    }

    // Mais um tamanho ou cor (copia este produto).
    function criarVariacao() {
        varAtual.erro = ""
        varNova.erro = ""
        const r = Api.chamar("produto_variacao_criar", { id: ficha.id, atual: varAtual.texto, nova: varNova.texto })
        if (!r.ok) {
            (r.campo === "atual" ? varAtual : varNova).erro = r.erro
            return
        }
        painelVariacao.close()
        janela.avisar("“" + r.dados.variacao + "” criado com código " + r.dados.codigo + ". Estoque começa em zero.", "ok")
        produtoId = ficha.id
        carregar()
    }
    function abrirOutro(id) {
        if (id === ficha.id)
            return
        if (alterado) {
            janela.avisar("Salve as alterações antes de trocar de tamanho ou cor.", "info")
            return
        }
        produtoId = id
        carregar()
    }

    function salvar() {
        limparErros()
        const eraNova = nova
        // D-09: mudar o preço de venda de uma peça cadastrada pede a senha
        // do gerente, para este salvar com estes dados (desconto na venda é
        // outra coisa).
        janela.chamarGerente("produto_salvar", coletar(), r => {
            if (r.ok) {
                const s = Api.chamar("produto_sugestoes")
                if (s.ok)
                    sugestoes = s.dados
                preencher(r.dados)
                produtoId = r.dados.id
                janela.avisar(eraNova ? (servico ? "Serviço cadastrado." : "Peça cadastrada.") : "Alterações salvas.", "ok")
            } else {
                mostrarErro(r.campo, r.erro)
            }
        })
    }

    function limparErros() {
        for (const c of [nome, codigo, codigoBarras, codigoOriginal, marca, unidade, categoria, campoCusto,
                         estoqueInicial, estoqueMinimo, local, ncm, cest, anpCodigo, anpDescricao,
                         peso, descontoMaximo, comissao, variacao, ofertaPreco, ofertaAte])
            c.erro = ""
        for (const [c, , , lista] of camposFiscais())
            if (lista !== "marcar")
                c.erro = ""
        observacoes.erro = ""
        origem.erro = ""
        csosn.erro = ""
        for (let i = 0; i < repPrecos.count; i++) {
            repPrecos.itemAt(i).campoLucro.erro = ""
            repPrecos.itemAt(i).campoPreco.erro = ""
        }
    }

    function mostrarErro(campo, mensagem) {
        const campos = {
            nome: nome, codigo: codigo, codigoBarras: codigoBarras, codigoOriginal: codigoOriginal,
            marca: marca, unidade: unidade, categoria: categoria, custo: campoCusto,
            estoqueInicial: estoqueInicial, estoqueMinimo: estoqueMinimo, local: local,
            ncm: ncm, cest: cest, anpCodigo: anpCodigo, anpDescricao: anpDescricao, origem: origem, csosn: csosn,
            peso: peso, descontoMaximo: descontoMaximo, comissao: comissao, observacoes: observacoes,
            variacao: variacao, ofertaPreco: ofertaPreco, ofertaAte: ofertaAte
        }
        let alvo = campo ? campos[campo] : null
        if (campo && campo.startsWith("fiscal.")) {
            const achado = camposFiscais().find(x => "fiscal." + x[1] === campo)
            if (achado) {
                blocoFiscal = achado[2]
                alvo = achado[0]
            }
        }
        if (campo === "anpCodigo" || campo === "anpDescricao")
            blocoFiscal = "combustivel"
        if (campo === "ncm" || campo === "cest" || campo === "origem" || campo === "csosn")
            blocoFiscal = "icms"
        if (!alvo && campo) {
            const p = campo.match(/^(preco|lucro)(\d)$/)
            const a = campo.match(/^aplicacoes\.(\d+)\.(\w+)$/)
            const e = campo.match(/^equivalentes\.(\d+)\.(\w+)$/)
            if (p) {
                const numero = parseInt(p[2])
                if (numero > 1)
                    mostrarOutros = true
                for (let i = 0; i < repPrecos.count; i++) {
                    const linha = repPrecos.itemAt(i)
                    if (linha.tabela === numero)
                        alvo = p[1] === "preco" ? linha.campoPreco : linha.campoLucro
                }
            } else if (a) {
                const linha = repAplicacoes.itemAt(parseInt(a[1]))
                if (linha)
                    alvo = linha.campo(a[2])
            } else if (e) {
                const linha = repEquivalentes.itemAt(parseInt(e[1]))
                if (linha)
                    alvo = linha.campo(e[2])
            }
        }
        if (alvo) {
            alvo.erro = mensagem
            rolarAte(alvo)
            if (typeof alvo.focar === "function")
                alvo.focar()
        }
        janela.avisar(mensagem, "erro")
    }

    function rolarAte(item) {
        const y = item.mapToItem(rolagem.contentItem, 0, 0).y
        if (y < rolagem.contentY + 20 || y + item.height > rolagem.contentY + rolagem.height - 20)
            rolagem.contentY = Math.max(0, Math.min(y - 120, rolagem.contentHeight - rolagem.height))
    }

    function csosnEscolhido() {
        const lista = sugestoes.csosns || []
        return csosn.indice >= 0 && csosn.indice < lista.length ? lista[csosn.indice].valor : ""
    }

    // Recalcula CSOSN/CFOP quando muda o CEST ou o CSOSN escolhido.
    function atualizarTributacao() {
        const r = Api.chamar("fiscal_tributacao", { cest: cest.texto, csosn: csosnEscolhido() })
        if (r.ok)
            tributacao = r.dados
    }

    Timer {
        id: esperaFiscal
        interval: 300
        onTriggered: tela.atualizarTributacao()
    }

    function marcarAlterado() {
        if (!carregando)
            alterado = true
    }

    function aoEsc() {
        if (alterado)
            janela.confirmar("Descartar alterações?", "O que você mudou nesta peça ainda não foi salvo.",
                             "Descartar", () => janela.voltar(), "perigo")
        else
            janela.voltar()
    }

    function recalcularPrecos() {
        const valor = campoCusto.texto
        for (let i = 0; i < repPrecos.count; i++)
            repPrecos.itemAt(i).recalcular("custo", valor)
    }

    function formatarCampo(campo, tipo) {
        const r = Api.chamar("numero_formatar", { tipo: tipo, texto: campo.texto })
        if (r.ok && r.dados.texto !== campo.texto)
            campo.texto = r.dados.texto
    }

    function dataHora(iso) {
        if (!iso)
            return ""
        return Qt.formatDateTime(new Date(iso), "dd/MM/yyyy 'às' HH:mm")
    }

    function adicionarAplicacao() {
        aplicacoes.append({ montadora: "", modelo: "", motor: "", anoInicio: "", anoFim: "", observacao: "" })
        marcarAlterado()
        Qt.callLater(() => {
            const linha = repAplicacoes.itemAt(aplicacoes.count - 1)
            if (linha) {
                linha.focar()
                rolarAte(linha)
            }
        })
    }

    function adicionarEquivalente() {
        equivalentes.append({ marca: "", codigo: "" })
        marcarAlterado()
        Qt.callLater(() => {
            const linha = repEquivalentes.itemAt(equivalentes.count - 1)
            if (linha) {
                linha.focar()
                rolarAte(linha)
            }
        })
    }

    Shortcut {
        sequence: "F2"
        enabled: tela.StackView.status === StackView.Active
        onActivated: tela.salvar()
    }

    // ---------- Tela ----------

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        anchors.bottomMargin: 0
        spacing: 12

        Cabecalho {
            sensivel: true
            textoVoltar: "Peças"
            titulo: tela.nova ? (tela.servico ? "Novo serviço" : "Nova peça") : tela.ficha.nome
            subtitulo: {
                if (!tela.ficha)
                    return ""
                if (tela.nova)
                    return "Preencha o nome e o preço. O resto pode ficar para depois."
                let s = "Código " + (tela.ficha.codigo || "a definir")
                if (tela.ficha.atualizadoEm) {
                    const nova = tela.ficha.atualizadoEm === tela.ficha.criadoEm
                    s += (nova ? " · cadastrada" : " · alterada")
                         + (tela.ficha.atualizadoPor ? " por " + tela.ficha.atualizadoPor : "")
                         + " em " + tela.dataHora(tela.ficha.atualizadoEm)
                }
                return s
            }
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
                visible: !tela.nova && !tela.alterado
                text: "Etiqueta"
                icone: "impressora"
                tipo: "secundario"
                onClicked: tela.janela.abrir("TelaEtiquetas", { produtoId: tela.ficha.id })
            }
            Botao {
                text: "Cancelar"
                tipo: "discreto"
                atalho: "Esc"
                onClicked: tela.aoEsc()
            }
            Botao {
                text: "Salvar"
                icone: "check"
                atalho: "F2"
                onClicked: tela.salvar()
            }
        }

        Flickable {
            id: rolagem
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentWidth: width
            contentHeight: corpo.implicitHeight + Tema.margem + 8
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}

            Linha {
                id: corpo
                width: rolagem.width - 14
                y: 6
                spacing: 18

                // ---------- Coluna da esquerda ----------
                ColumnLayout {
                    spacing: 14
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignTop

                    Faixa {
                        titulo: tela.servico ? "Serviço" : "Peça"
                        color: Tema.pergaminho
                        Layout.fillWidth: true

                        Pilulas {
                            pequenas: true
                            modelo: [{ id: "produto", nome: "Produto" }, { id: "servico", nome: "Serviço (sem estoque)" }]
                            atual: tela.tipo
                            onEscolhido: (id) => {
                                if (id === tela.tipo)
                                    return
                                tela.tipo = id
                                tela.blocoFiscal = id === "servico" ? "servico" : "icms"
                                tela.marcarAlterado()
                            }
                        }
                        Campo {
                            id: nome
                            rotulo: tela.servico ? "Nome do serviço" : "Nome da peça"
                            dica: tela.servico ? "Ex.: Troca de óleo, instalação" : "Ex.: Rolamento roda traseira"
                            maximo: 120
                            tamanhoFonte: 18
                            altura: 44
                            Layout.fillWidth: true
                            onEditado: tela.marcarAlterado()
                        }
                        Linha {
                            spacing: 12
                            Layout.fillWidth: true
                            Campo {
                                id: codigo
                                rotulo: "Código interno"
                                // Vazio: o Rustinho dá o código ao salvar, da faixa deste caixa.
                                dica: !tela.ficha ? "" : tela.nova
                                      ? (tela.ficha.codigoPrevisto ? "Automático: " + tela.ficha.codigoPrevisto : "Automático")
                                      : "A definir"
                                maximo: 60
                                Layout.preferredWidth: 130
                                onEditado: tela.marcarAlterado()
                            }
                            Campo {
                                id: codigoBarras
                                visible: !tela.servico
                                rotulo: "Código de barras"
                                dica: "Passe o leitor aqui"
                                maximo: 14
                                Layout.preferredWidth: 190
                                onEditado: tela.marcarAlterado()
                            }
                            Campo {
                                id: codigoOriginal
                                visible: !tela.servico
                                rotulo: "Código original / fabricante"
                                maximo: 60
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                            }
                        }
                        // Código a definir, ou código que também leva a outra peça.
                        Text {
                            visible: !!tela.ficha && !!tela.ficha.codigoAviso
                            text: tela.ficha ? tela.ficha.codigoAviso : ""
                            color: tela.ficha && tela.ficha.codigo && tela.ficha.codigoAviso ? Tema.vermelho : Tema.tintaSuave
                            font.family: Tema.texto
                            font.pixelSize: 13
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                        Linha {
                            spacing: 12
                            Layout.fillWidth: true
                            Campo {
                                id: marca
                                visible: !tela.servico
                                rotulo: "Marca"
                                maximo: 60
                                sugestoes: tela.sugestoes.marcas
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                            }
                            Campo {
                                id: unidade
                                rotulo: "Unidade"
                                maximo: 6
                                maiusculas: true
                                sugestoes: tela.sugestoes.unidades
                                acao: "baixo"
                                Layout.preferredWidth: 120
                                onEditado: tela.marcarAlterado()
                                onAcionado: escolherUnidade.abrirCom(unidade.texto)
                            }
                            Campo {
                                id: categoria
                                rotulo: "Categoria"
                                dica: "Ex.: Freios, Suspensão"
                                maximo: 60
                                sugestoes: tela.sugestoes.categorias
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                            }
                        }
                        // Tamanhos e cores: cada um é um produto, com estoque próprio.
                        Linha {
                            visible: tela.temGrade
                            spacing: 12
                            Layout.fillWidth: true
                            Campo {
                                id: variacao
                                rotulo: "Tamanho ou cor deste"
                                maximo: 30
                                Layout.preferredWidth: 170
                                onEditado: tela.marcarAlterado()
                            }
                            ColumnLayout {
                                spacing: 5
                                Layout.fillWidth: true
                                Layout.alignment: Qt.AlignTop
                                Rotulo { text: "Na grade (toque para abrir)" }
                                Flow {
                                    spacing: 6
                                    Layout.fillWidth: true
                                    Repeater {
                                        model: tela.temGrade ? tela.ficha.gradeItens : []
                                        delegate: Rectangle {
                                            id: itemGrade
                                            required property var modelData
                                            readonly property bool este: modelData.id === tela.ficha.id
                                            radius: height / 2
                                            height: Tema.celular ? 42 : 36
                                            width: textoGrade.implicitWidth + 28
                                            color: este ? Tema.tinta : sobreGrade.hovered ? Tema.sombra(0.07) : Tema.papel
                                            Text {
                                                id: textoGrade
                                                anchors.centerIn: parent
                                                text: "<b>" + (itemGrade.modelData.variacao || "—") + "</b>  " + itemGrade.modelData.estoque + " " + tela.ficha.unidade
                                                textFormat: Text.StyledText
                                                color: itemGrade.este ? Tema.claro : Tema.tinta
                                                font.family: Tema.texto
                                                font.pixelSize: 15
                                            }
                                            HoverHandler { id: sobreGrade; cursorShape: Qt.PointingHandCursor }
                                            TapHandler { onTapped: tela.abrirOutro(itemGrade.modelData.id) }
                                        }
                                    }
                                }
                            }
                        }
                        Botao {
                            visible: !tela.nova && !tela.servico
                            text: tela.temGrade ? "Mais um tamanho ou cor" : "Tem outros tamanhos ou cores?"
                            icone: "mais"
                            tipo: "discreto"
                            Layout.leftMargin: -10
                            onClicked: {
                                if (tela.alterado) {
                                    janela.avisar("Salve as alterações antes.", "info")
                                    return
                                }
                                varAtual.texto = ""
                                varNova.texto = ""
                                varAtual.erro = ""
                                varNova.erro = ""
                                painelVariacao.open()
                                if (tela.temGrade)
                                    varNova.focarSempre()
                                else
                                    varAtual.focarSempre()
                            }
                        }
                        Marcar {
                            id: ativo
                            visible: !tela.nova
                            text: "Peça ativa (aparece na busca e na venda)"
                            onToggled: tela.marcarAlterado()
                        }
                    }

                    Faixa {
                        visible: !tela.servico
                        titulo: "Aplicações"
                        subtitulo: "em quais carros esta peça serve"
                        color: Tema.azul
                        Layout.fillWidth: true

                        RowLayout {
                            visible: aplicacoes.count > 0 && !Tema.estreito
                            spacing: 10
                            Layout.fillWidth: true
                            Rotulo { text: "Montadora"; Layout.preferredWidth: 150 }
                            Rotulo { text: "Modelo"; Layout.fillWidth: true }
                            Rotulo { text: "Motor"; Layout.preferredWidth: 104 }
                            Rotulo { text: "Ano de"; Layout.preferredWidth: 74 }
                            Rotulo { text: "Até"; Layout.preferredWidth: 74 }
                            Item { Layout.preferredWidth: 40 }
                        }
                        Repeater {
                            id: repAplicacoes
                            model: aplicacoes
                            delegate: LinhaAplicacao {
                                Layout.fillWidth: true
                                sugestoesMontadora: tela.sugestoes.montadoras
                                sugestoesModelo: tela.sugestoes.modelos
                                onMudou: (campo, valor) => {
                                    aplicacoes.setProperty(index, campo, valor)
                                    tela.marcarAlterado()
                                }
                                onRemover: {
                                    aplicacoes.remove(index)
                                    tela.marcarAlterado()
                                }
                            }
                        }
                        Nota {
                            visible: aplicacoes.count === 0
                            text: "Nenhum carro ainda. A busca do balcão usa esta lista: quem digitar “pastilha gol 2015” acha a peça."
                            Layout.fillWidth: true
                        }
                        Botao {
                            text: "Adicionar carro"
                            icone: "mais"
                            tipo: "secundario"
                            onClicked: tela.adicionarAplicacao()
                        }
                    }

                    Faixa {
                        visible: !tela.servico
                        titulo: "Equivalências"
                        subtitulo: "a mesma peça em outras marcas"
                        color: Tema.pergaminho
                        Layout.fillWidth: true

                        Repeater {
                            id: repEquivalentes
                            model: equivalentes
                            delegate: LinhaEquivalente {
                                Layout.fillWidth: true
                                sugestoesMarca: tela.sugestoes.marcas
                                onMudou: (campo, valor) => {
                                    equivalentes.setProperty(index, campo, valor)
                                    tela.marcarAlterado()
                                }
                                onRemover: {
                                    equivalentes.remove(index)
                                    tela.marcarAlterado()
                                }
                            }
                        }
                        Nota {
                            visible: equivalentes.count === 0
                            text: "Ex.: SKF BR-32206 para um rolamento FAG 32206A. A busca acha a peça por qualquer um desses códigos."
                            Layout.fillWidth: true
                        }
                        Botao {
                            text: "Adicionar equivalente"
                            icone: "mais"
                            tipo: "secundario"
                            onClicked: tela.adicionarEquivalente()
                        }
                    }

                    Faixa {
                        titulo: "Mais detalhes"
                        color: Tema.azul
                        Layout.fillWidth: true

                        Linha {
                            spacing: 12
                            Layout.fillWidth: true
                            Campo {
                                id: peso
                                visible: !tela.servico
                                rotulo: "Peso"
                                sufixo: "kg"
                                numerico: true
                                maximo: 12
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                                onSaiu: tela.formatarCampo(peso, "quantidade")
                            }
                            Campo {
                                id: descontoMaximo
                                rotulo: "Desconto máximo"
                                sufixo: "%"
                                numerico: true
                                maximo: 8
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                                onSaiu: tela.formatarCampo(descontoMaximo, "percentual")
                            }
                            Campo {
                                id: comissao
                                rotulo: "Comissão do vendedor"
                                sufixo: "%"
                                numerico: true
                                maximo: 8
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                                onSaiu: tela.formatarCampo(comissao, "percentual")
                            }
                        }
                        AreaTexto {
                            id: observacoes
                            rotulo: "Observações"
                            dica: "Ex.: vem em par; conferir lado antes de vender"
                            Layout.fillWidth: true
                            onEditado: tela.marcarAlterado()
                        }
                    }
                }

                // ---------- Coluna da direita ----------
                ColumnLayout {
                    spacing: 14
                    Layout.preferredWidth: Math.min(420, Math.max(340, corpo.width * 0.33))
                    Layout.maximumWidth: 420
                    Layout.alignment: Qt.AlignTop

                    Faixa {
                        titulo: "Preços"
                        color: Tema.verde
                        corTitulo: Tema.verdeEscuro
                        Layout.fillWidth: true

                        Campo {
                            id: campoCusto
                            visible: tela.verCustos
                            sensivel: true
                            rotulo: "Custo"
                            prefixo: "R$"
                            numerico: true
                            maximo: 18
                            Layout.fillWidth: true
                            onEditado: {
                                tela.recalcularPrecos()
                                tela.marcarAlterado()
                            }
                            onSaiu: tela.formatarCampo(campoCusto, "dinheiro")
                        }
                        Repeater {
                            id: repPrecos
                            model: tela.precos
                            delegate: LinhaPreco {
                                required property var modelData
                                visible: tabela === 1 || tela.mostrarOutros
                                tabela: modelData.tabela
                                nome: modelData.nome
                                lucroInicial: modelData.lucro
                                precoInicial: modelData.preco
                                custo: campoCusto.texto
                                verLucro: tela.verCustos
                                destaque: tabela === 1
                                Layout.fillWidth: true
                                Layout.topMargin: tabela === 2 ? 4 : 0
                                onEditado: tela.marcarAlterado()
                            }
                        }
                        Nota {
                            visible: !tela.verCustos
                            text: "O custo e o % de lucro ficam com o gerente. Mudar o preço de venda de uma peça já cadastrada pede a senha do gerente."
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }
                        Botao {
                            text: tela.mostrarOutros ? "Esconder outros preços" : "Outros preços"
                            icone: tela.mostrarOutros ? "cima" : "baixo"
                            tipo: "discreto"
                            Layout.leftMargin: -10
                            onClicked: tela.mostrarOutros = !tela.mostrarOutros
                        }
                        Marcar {
                            id: emOferta
                            text: "Em oferta"
                            onToggled: {
                                tela.marcarAlterado()
                                if (checked)
                                    ofertaPreco.focarSempre()
                            }
                        }
                        Linha {
                            visible: emOferta.checked
                            spacing: 12
                            Layout.fillWidth: true
                            Campo {
                                id: ofertaPreco
                                sensivel: false
                                rotulo: "Preço da oferta"
                                prefixo: "R$"
                                numerico: true
                                maximo: 18
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                                onSaiu: tela.formatarCampo(ofertaPreco, "dinheiro")
                            }
                            Campo {
                                id: ofertaAte
                                rotulo: "Até o dia"
                                dica: "dd/mm/aaaa"
                                maximo: 10
                                numerico: true
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                            }
                        }
                        Nota {
                            visible: emOferta.checked
                            text: tela.ficha && tela.ficha.ofertaPreco !== "" && !tela.ficha.ofertaValendo && !tela.alterado
                                  ? "Esta oferta já venceu: a venda usa o preço normal."
                                  : "Na venda, entra no lugar do preço “" + (tela.precos.length > 0 ? tela.precos[0].nome : "") + "”. Sem data, vale até desmarcar."
                            color: tela.ficha && tela.ficha.ofertaPreco !== "" && !tela.ficha.ofertaValendo && !tela.alterado ? Tema.vermelho : Tema.tintaSuave
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }
                        Marcar {
                            id: precoAberto
                            text: "Preço digitado na hora da venda"
                            onToggled: tela.marcarAlterado()
                        }
                    }

                    Faixa {
                        visible: !tela.servico
                        titulo: "Estoque"
                        color: Tema.azul
                        Layout.fillWidth: true

                        RowLayout {
                            visible: !tela.nova
                            spacing: 14
                            Layout.fillWidth: true
                            ColumnLayout {
                                spacing: 0
                                Layout.fillWidth: true
                                Text {
                                    text: tela.ficha ? tela.ficha.estoque + " " + tela.ficha.unidade : ""
                                    color: tela.ficha && tela.ficha.estoqueValor <= 0 ? Tema.vermelho : Tema.tinta
                                    font.family: Tema.texto
                                    font.weight: Font.Bold
                                    font.pixelSize: 38
                                }
                                Nota {
                                    text: "na prateleira, pelo sistema"
                                    font.pixelSize: 14
                                }
                            }
                            Botao {
                                text: "Ajustar"
                                icone: "ajuste"
                                tipo: "secundario"
                                onClicked: {
                                    if (tela.alterado) {
                                        janela.avisar("Salve as alterações antes de ajustar o estoque.", "info")
                                        return
                                    }
                                    ajusteEstoque.abrirCom(tela.ficha.id, tela.ficha.estoque, tela.ficha.unidade)
                                }
                            }
                        }
                        Linha {
                            spacing: 12
                            Layout.fillWidth: true
                            Campo {
                                id: estoqueInicial
                                visible: tela.nova
                                rotulo: "Estoque inicial"
                                sufixo: unidade.texto
                                numerico: true
                                maximo: 14
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                                onSaiu: tela.formatarCampo(estoqueInicial, "quantidade")
                            }
                            Campo {
                                id: estoqueMinimo
                                rotulo: "Estoque mínimo"
                                sufixo: unidade.texto
                                numerico: true
                                maximo: 14
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                                onSaiu: tela.formatarCampo(estoqueMinimo, "quantidade")
                            }
                        }
                        Campo {
                            id: local
                            rotulo: "Local na prateleira"
                            dica: "Ex.: Corredor B, prateleira 3"
                            maximo: 40
                            sugestoes: tela.sugestoes.locais
                            Layout.fillWidth: true
                            onEditado: tela.marcarAlterado()
                        }
                        Rotulo {
                            visible: tela.ficha && tela.ficha.fornecedores && tela.ficha.fornecedores.length > 0
                            text: "Código nos fornecedores"
                            Layout.topMargin: 4
                        }
                        Repeater {
                            model: tela.ficha && tela.ficha.fornecedores ? tela.ficha.fornecedores : []
                            delegate: Nota {
                                required property var modelData
                                text: modelData.codigo + "  ·  " + modelData.fornecedor + (modelData.fator !== "1" ? "  ·  caixa com " + modelData.fator : "")
                                font.pixelSize: 14
                                elide: Text.ElideRight
                                wrapMode: Text.NoWrap
                                Layout.fillWidth: true
                            }
                        }
                    }

                    Faixa {
                        titulo: "Fiscal"
                        subtitulo: "para a nota"
                        color: Tema.pergaminho
                        Layout.fillWidth: true

                        Pilulas {
                            pequenas: true
                            modelo: tela.servico
                                    ? [{ id: "servico", nome: "ISS" }, { id: "pis", nome: "PIS/Cofins" }]
                                    : [{ id: "icms", nome: "ICMS" }, { id: "pis", nome: "PIS/Cofins" }, { id: "ipi", nome: "IPI" },
                                       { id: "combustivel", nome: "Combustível" }]
                            atual: tela.blocoFiscal
                            onEscolhido: (id) => tela.blocoFiscal = id
                        }

                        // ---------- ICMS ----------
                        ColumnLayout {
                            visible: tela.blocoFiscal === "icms"
                            spacing: 12
                            Layout.fillWidth: true
                            Linha {
                                spacing: 12
                                Layout.fillWidth: true
                                Campo {
                                    id: ncm
                                    rotulo: "NCM"
                                    dica: "8 números"
                                    maximo: 12
                                    Layout.fillWidth: true
                                    onEditado: tela.marcarAlterado()
                                }
                                Campo {
                                    id: cest
                                    rotulo: "CEST"
                                    dica: "7 números"
                                    maximo: 10
                                    Layout.fillWidth: true
                                    onEditado: {
                                        tela.marcarAlterado()
                                        esperaFiscal.restart()
                                    }
                                }
                            }
                            Selecao {
                                id: origem
                                rotulo: "Origem da mercadoria"
                                modelo: tela.sugestoes.origens
                                Layout.fillWidth: true
                                onEscolhido: tela.marcarAlterado()
                            }
                            Selecao {
                                id: csosn
                                rotulo: "CSOSN na venda"
                                modelo: (tela.sugestoes.csosns || []).map(c => c.texto)
                                Layout.fillWidth: true
                                onEscolhido: {
                                    tela.marcarAlterado()
                                    tela.atualizarTributacao()
                                }
                            }
                            Rectangle {
                                visible: !!tela.tributacao
                                radius: Tema.raioCampo
                                color: tela.tributacao && tela.tributacao.alerta !== "" ? Tema.rosa : Tema.vela(0.35)
                                implicitHeight: textoFiscal.implicitHeight + 22
                                Layout.fillWidth: true
                                ColumnLayout {
                                    id: textoFiscal
                                    x: 12
                                    y: 11
                                    width: parent.width - 24
                                    spacing: 4
                                    Text {
                                        text: tela.tributacao ? "Venda no estado: CSOSN " + tela.tributacao.csosn + " · CFOP " + tela.tributacao.cfop
                                                                + (tela.tributacao.automatico ? "  (automático)" : "") : ""
                                        color: Tema.tinta
                                        font.family: Tema.texto
                                        font.weight: Font.DemiBold
                                        font.pixelSize: 15
                                        wrapMode: Text.WordWrap
                                        Layout.fillWidth: true
                                    }
                                    Nota {
                                        text: tela.tributacao ? (tela.tributacao.alerta !== "" ? tela.tributacao.alerta : tela.tributacao.explicacao) : ""
                                        color: tela.tributacao && tela.tributacao.alerta !== "" ? Tema.vermelho : Tema.tintaSuave
                                        font.pixelSize: 14
                                        Layout.fillWidth: true
                                    }
                                }
                            }
                        }

                        // ---------- PIS e Cofins ----------
                        ColumnLayout {
                            visible: tela.blocoFiscal === "pis"
                            spacing: 12
                            Layout.fillWidth: true
                            Marcar {
                                id: monofasico
                                text: "PIS/Cofins monofásico (a receita sai separada no Simples)"
                                Layout.fillWidth: true
                                onToggled: tela.marcarAlterado()
                            }
                            Selecao {
                                id: pisCst
                                rotulo: "CST do PIS"
                                modelo: (tela.sugestoes.cstPis || []).map(c => c.texto)
                                Layout.fillWidth: true
                                onEscolhido: tela.marcarAlterado()
                            }
                            Linha {
                                spacing: 12
                                Layout.fillWidth: true
                                CampoNumero { id: pisAliquota; rotulo: "PIS"; sufixo: "%"; tipoNumero: "percentual"; onEditado: tela.marcarAlterado() }
                                CampoNumero { id: pisValor; rotulo: "PIS por unidade"; prefixo: "R$"; tipoNumero: "dinheiro"; onEditado: tela.marcarAlterado() }
                            }
                            Selecao {
                                id: cofinsCst
                                rotulo: "CST da Cofins"
                                modelo: (tela.sugestoes.cstPis || []).map(c => c.texto)
                                Layout.fillWidth: true
                                onEscolhido: tela.marcarAlterado()
                            }
                            Linha {
                                spacing: 12
                                Layout.fillWidth: true
                                CampoNumero { id: cofinsAliquota; rotulo: "Cofins"; sufixo: "%"; tipoNumero: "percentual"; onEditado: tela.marcarAlterado() }
                                CampoNumero { id: cofinsValor; rotulo: "Cofins por unidade"; prefixo: "R$"; tipoNumero: "dinheiro"; onEditado: tela.marcarAlterado() }
                            }
                            CampoNumero { id: naturezaReceita; rotulo: "Natureza da receita"; dica: "3 números"; tipoNumero: ""; onEditado: tela.marcarAlterado() }
                        }

                        // ---------- IPI ----------
                        ColumnLayout {
                            visible: tela.blocoFiscal === "ipi"
                            spacing: 12
                            Layout.fillWidth: true
                            Nota {
                                text: "Só para quem fabrica ou importa. Loja de revenda normalmente deixa “Sem IPI”."
                                font.pixelSize: 14
                                Layout.fillWidth: true
                            }
                            Selecao {
                                id: ipiCst
                                rotulo: "CST do IPI"
                                modelo: (tela.sugestoes.cstIpi || []).map(c => c.texto)
                                Layout.fillWidth: true
                                onEscolhido: tela.marcarAlterado()
                            }
                            Linha {
                                spacing: 12
                                Layout.fillWidth: true
                                CampoNumero { id: ipiAliquota; rotulo: "IPI"; sufixo: "%"; tipoNumero: "percentual"; onEditado: tela.marcarAlterado() }
                                CampoNumero { id: ipiValor; rotulo: "Ou valor fixo"; prefixo: "R$"; tipoNumero: "dinheiro"; onEditado: tela.marcarAlterado() }
                            }
                            Linha {
                                spacing: 12
                                Layout.fillWidth: true
                                CampoNumero { id: ipiEnquadramento; rotulo: "Enquadramento"; dica: "Ex.: 999"; tipoNumero: ""; onEditado: tela.marcarAlterado() }
                                CampoNumero { id: ipiSelo; rotulo: "Selo"; numerico: false; tipoNumero: ""; onEditado: tela.marcarAlterado() }
                                CampoNumero { id: ipiSeloQuantidade; rotulo: "Qtde. de selos"; tipoNumero: ""; onEditado: tela.marcarAlterado() }
                            }
                        }

                        // ---------- Combustível e lubrificante ----------
                        ColumnLayout {
                            visible: tela.blocoFiscal === "combustivel"
                            spacing: 12
                            Layout.fillWidth: true
                            Nota {
                                text: "Para óleo, lubrificante e combustível (a nota pede o código ANP)."
                                font.pixelSize: 14
                                Layout.fillWidth: true
                            }
                            Campo {
                                id: anpCodigo
                                rotulo: "Código ANP"
                                dica: "9 números"
                                maximo: 12
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                            }
                            Campo {
                                id: anpDescricao
                                rotulo: "Descrição conforme a ANP"
                                maximo: 95
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                            }
                            Linha {
                                spacing: 12
                                Layout.fillWidth: true
                                CampoNumero { id: glp; rotulo: "GLP"; sufixo: "%"; tipoNumero: "percentual"; onEditado: tela.marcarAlterado() }
                                CampoNumero { id: gnNacional; rotulo: "Gás natural"; sufixo: "%"; tipoNumero: "percentual"; onEditado: tela.marcarAlterado() }
                                CampoNumero { id: gnImportado; rotulo: "GN importado"; sufixo: "%"; tipoNumero: "percentual"; onEditado: tela.marcarAlterado() }
                            }
                            Linha {
                                spacing: 12
                                Layout.fillWidth: true
                                CampoNumero { id: valorPartida; rotulo: "Valor da partida"; prefixo: "R$"; tipoNumero: "dinheiro"; onEditado: tela.marcarAlterado() }
                                CampoNumero { id: codif; rotulo: "CODIF"; tipoNumero: ""; onEditado: tela.marcarAlterado() }
                            }
                            Linha {
                                spacing: 12
                                Layout.fillWidth: true
                                CampoNumero { id: temperatura; rotulo: "Qtde. temp. ambiente"; tipoNumero: "quantidade"; onEditado: tela.marcarAlterado() }
                                CampoNumero { id: biodiesel; rotulo: "Mistura de biodiesel"; sufixo: "%"; tipoNumero: "percentual"; onEditado: tela.marcarAlterado() }
                            }
                            Linha {
                                spacing: 12
                                Layout.fillWidth: true
                                CampoNumero { id: cide; rotulo: "CIDE por unidade"; prefixo: "R$"; tipoNumero: "dinheiro"; onEditado: tela.marcarAlterado() }
                                Item { Layout.fillWidth: true }
                            }
                            Rotulo { text: "Origem do combustível"; Layout.topMargin: 2 }
                            Marcar {
                                id: importado
                                text: "Importado"
                                onToggled: tela.marcarAlterado()
                            }
                            Linha {
                                spacing: 12
                                Layout.fillWidth: true
                                Selecao {
                                    id: ufOrigem
                                    rotulo: "Estado de origem"
                                    modelo: tela.ufsComVazio().map(u => u === "" ? "—" : u)
                                    Layout.fillWidth: true
                                    onEscolhido: tela.marcarAlterado()
                                }
                                CampoNumero { id: percentualOrigem; rotulo: "Percentual originário"; sufixo: "%"; tipoNumero: "percentual"; onEditado: tela.marcarAlterado() }
                            }
                        }

                        // ---------- Serviço (ISS) ----------
                        ColumnLayout {
                            visible: tela.blocoFiscal === "servico"
                            spacing: 12
                            Layout.fillWidth: true
                            Linha {
                                spacing: 12
                                Layout.fillWidth: true
                                CampoNumero { id: servicoCodigo; rotulo: "Código do serviço"; dica: "Ex.: 14.01"; numerico: false; tipoNumero: ""; onEditado: tela.marcarAlterado() }
                                CampoNumero { id: cnae; rotulo: "CNAE"; dica: "7 números"; tipoNumero: ""; onEditado: tela.marcarAlterado() }
                                CampoNumero { id: issAliquota; rotulo: "ISS"; sufixo: "%"; tipoNumero: "percentual"; onEditado: tela.marcarAlterado() }
                            }
                            Linha {
                                spacing: 12
                                Layout.fillWidth: true
                                CampoNumero { id: codigoMunicipio; rotulo: "Código no município"; numerico: false; tipoNumero: ""; onEditado: tela.marcarAlterado() }
                                CampoNumero { id: nbs; rotulo: "NBS"; dica: "9 números"; tipoNumero: ""; onEditado: tela.marcarAlterado() }
                            }
                            Selecao {
                                id: issExigibilidade
                                rotulo: "Exigibilidade do ISS"
                                modelo: (tela.sugestoes.exigibilidades || []).map(c => c.texto)
                                Layout.fillWidth: true
                                onEscolhido: tela.marcarAlterado()
                            }
                            Marcar {
                                id: incentivoFiscal
                                text: "Incentivo fiscal"
                                onToggled: tela.marcarAlterado()
                            }
                            AreaTexto {
                                id: servicoDescricao
                                rotulo: "Descrição do serviço (vai na nota)"
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                            }
                        }
                    }

                    Faixa {
                        visible: !tela.nova
                        titulo: "Histórico"
                        color: Tema.azul
                        espacamento: 10
                        Layout.fillWidth: true

                        Repeater {
                            model: tela.ficha ? tela.ficha.historico : []
                            delegate: ColumnLayout {
                                required property var modelData
                                spacing: 1
                                Layout.fillWidth: true
                                Text {
                                    text: tela.dataHora(modelData.quando) + " · " + (modelData.usuario || "alguém") + " " + modelData.descricao
                                    color: Tema.tinta
                                    font.family: Tema.texto
                                    font.weight: Font.DemiBold
                                    font.pixelSize: 14
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                                Repeater {
                                    model: modelData.detalhes
                                    delegate: Nota {
                                        required property string modelData
                                        text: modelData
                                        font.pixelSize: 14
                                        Layout.fillWidth: true
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    EscolherUnidade {
        id: escolherUnidade
        onEscolhida: (sigla) => {
            if (unidade.texto !== sigla) {
                unidade.texto = sigla
                tela.marcarAlterado()
            }
            unidade.erro = ""
            const s = Api.chamar("produto_sugestoes")
            if (s.ok)
                tela.sugestoes = s.dados
        }
    }

    Painel {
        id: painelVariacao
        contentItem: ColumnLayout {
            spacing: 14
            Titulo { text: "Tamanho ou cor"; font.pixelSize: 30 }
            Nota {
                text: "O novo copia tudo deste (preços, fiscal, carros), com código próprio e estoque zero. Cada tamanho ou cor tem o seu estoque."
                font.pixelSize: 15
                Layout.fillWidth: true
            }
            Campo {
                id: varAtual
                visible: !tela.temGrade
                rotulo: "Este produto é"
                dica: "Ex.: P, 38, Azul M"
                maximo: 30
                Layout.fillWidth: true
                onConfirmado: varNova.focarSempre()
            }
            Campo {
                id: varNova
                rotulo: "Novo tamanho ou cor"
                dica: "Ex.: M, 40, Azul G"
                maximo: 30
                Layout.fillWidth: true
                onConfirmado: tela.criarVariacao()
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao { text: "Voltar"; tipo: "discreto"; onClicked: painelVariacao.close() }
                Botao { text: "Criar"; icone: "mais"; onClicked: tela.criarVariacao() }
            }
        }
    }

    AjusteEstoque {
        id: ajusteEstoque
        onAjustado: (fichaNova) => {
            tela.preencher(fichaNova)
            janela.avisar("Estoque ajustado para " + fichaNova.estoque + " " + fichaNova.unidade + ".", "ok")
        }
    }

    Component.onCompleted: carregar()
    StackView.onActivated: {
        if (nova)
            nome.focar()
        else
            rolagem.forceActiveFocus()
    }
}
