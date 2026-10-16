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
    readonly property bool nova: !ficha || ficha.id === ""

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
        lubrificante.checked = f.anpCodigo !== "" || f.anpDescricao !== ""
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
            anpCodigo: lubrificante.checked ? anpCodigo.texto : "",
            anpDescricao: lubrificante.checked ? anpDescricao.texto : "",
            peso: peso.texto,
            descontoMaximo: descontoMaximo.texto,
            comissao: comissao.texto,
            observacoes: observacoes.texto,
            aplicacoes: listaAplicacoes,
            equivalentes: listaEquivalentes
        }
    }

    function salvar() {
        limparErros()
        const eraNova = nova
        const r = Api.chamar("produto_salvar", coletar())
        if (r.ok) {
            const s = Api.chamar("produto_sugestoes")
            if (s.ok)
                sugestoes = s.dados
            preencher(r.dados)
            produtoId = r.dados.id
            janela.avisar(eraNova ? "Peça cadastrada." : "Alterações salvas.", "ok")
        } else {
            mostrarErro(r.campo, r.erro)
        }
    }

    function limparErros() {
        for (const c of [nome, codigo, codigoBarras, codigoOriginal, marca, unidade, categoria, campoCusto,
                         estoqueInicial, estoqueMinimo, local, ncm, cest, anpCodigo, anpDescricao,
                         peso, descontoMaximo, comissao])
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
            peso: peso, descontoMaximo: descontoMaximo, comissao: comissao, observacoes: observacoes
        }
        let alvo = campo ? campos[campo] : null
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
            titulo: tela.nova ? "Nova peça" : tela.ficha.nome
            subtitulo: {
                if (!tela.ficha)
                    return ""
                if (tela.nova)
                    return "Preencha o nome e o preço. O resto pode ficar para depois."
                let s = "Código " + tela.ficha.codigo
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

            RowLayout {
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
                        titulo: "Peça"
                        color: Tema.pergaminho
                        Layout.fillWidth: true

                        Campo {
                            id: nome
                            rotulo: "Nome da peça"
                            dica: "Ex.: Rolamento roda traseira"
                            maximo: 120
                            tamanhoFonte: 18
                            altura: 44
                            Layout.fillWidth: true
                            onEditado: tela.marcarAlterado()
                        }
                        RowLayout {
                            spacing: 12
                            Layout.fillWidth: true
                            Campo {
                                id: codigo
                                rotulo: "Código interno"
                                maximo: 60
                                Layout.preferredWidth: 130
                                onEditado: tela.marcarAlterado()
                            }
                            Campo {
                                id: codigoBarras
                                rotulo: "Código de barras"
                                dica: "Passe o leitor aqui"
                                maximo: 14
                                Layout.preferredWidth: 190
                                onEditado: tela.marcarAlterado()
                            }
                            Campo {
                                id: codigoOriginal
                                rotulo: "Código original / fabricante"
                                maximo: 60
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                            }
                        }
                        RowLayout {
                            spacing: 12
                            Layout.fillWidth: true
                            Campo {
                                id: marca
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
                                Layout.preferredWidth: 104
                                onEditado: tela.marcarAlterado()
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
                        Marcar {
                            id: ativo
                            visible: !tela.nova
                            text: "Peça ativa (aparece na busca e na venda)"
                            onToggled: tela.marcarAlterado()
                        }
                    }

                    Faixa {
                        titulo: "Aplicações"
                        subtitulo: "em quais carros esta peça serve"
                        color: Tema.azul
                        Layout.fillWidth: true

                        RowLayout {
                            visible: aplicacoes.count > 0
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

                        RowLayout {
                            spacing: 12
                            Layout.fillWidth: true
                            Campo {
                                id: peso
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
                                destaque: tabela === 1
                                Layout.fillWidth: true
                                Layout.topMargin: tabela === 2 ? 4 : 0
                                onEditado: tela.marcarAlterado()
                            }
                        }
                        Botao {
                            text: tela.mostrarOutros ? "Esconder outros preços" : "Outros preços"
                            icone: tela.mostrarOutros ? "cima" : "baixo"
                            tipo: "discreto"
                            Layout.leftMargin: -10
                            onClicked: tela.mostrarOutros = !tela.mostrarOutros
                        }
                    }

                    Faixa {
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
                        RowLayout {
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
                        color: Tema.pergaminho
                        Layout.fillWidth: true

                        RowLayout {
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
                        Marcar {
                            id: lubrificante
                            text: "Óleo, lubrificante ou combustível (pede código ANP)"
                            Layout.fillWidth: true
                            onToggled: tela.marcarAlterado()
                        }
                        Campo {
                            id: anpCodigo
                            visible: lubrificante.checked
                            rotulo: "Código ANP"
                            dica: "9 números"
                            maximo: 12
                            Layout.fillWidth: true
                            onEditado: tela.marcarAlterado()
                        }
                        Campo {
                            id: anpDescricao
                            visible: lubrificante.checked
                            rotulo: "Descrição conforme a ANP"
                            maximo: 95
                            Layout.fillWidth: true
                            onEditado: tela.marcarAlterado()
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
                            color: tela.tributacao && tela.tributacao.alerta !== "" ? Tema.rosa : Qt.rgba(1, 1, 1, 0.35)
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
                        Marcar {
                            id: monofasico
                            text: "PIS/Cofins monofásico (a receita sai separada no Simples)"
                            Layout.fillWidth: true
                            onToggled: tela.marcarAlterado()
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
