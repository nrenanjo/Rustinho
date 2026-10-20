import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Cadastro de cliente ou fornecedor: uma tela só, em faixas.
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    property string pessoaId: ""
    property string papel: "cliente"
    property var ficha: null
    property var ufs: []
    property bool alterado: false
    property bool carregando: true
    readonly property bool nova: !ficha || ficha.id === ""
    readonly property bool juridica: tipoJ.checked

    ListModel { id: veiculos }

    function carregar() {
        carregando = true
        const c = Api.chamar("config_ler")
        if (c.ok)
            ufs = [""].concat(c.dados.ufs)
        const r = pessoaId ? Api.chamar("pessoa_abrir", { id: pessoaId }) : Api.chamar("pessoa_nova", { papel: papel })
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
        codigo.texto = f.codigo
        cliente.checked = f.cliente
        fornecedor.checked = f.fornecedor
        tipoJ.checked = f.tipo === "J"
        tipoF.checked = f.tipo !== "J"
        documento.texto = f.documento
        nome.texto = f.nome
        fantasia.texto = f.fantasia
        ie.texto = f.ie
        isento.checked = f.indicadorIe === 2
        telefone.texto = f.telefone
        celular.texto = f.celular
        email.texto = f.email
        cep.texto = f.cep
        logradouro.texto = f.logradouro
        numero.texto = f.numero
        complemento.texto = f.complemento
        bairro.texto = f.bairro
        municipio.texto = f.municipio
        uf.indice = Math.max(0, ufs.indexOf(f.uf))
        limite.texto = f.limiteCredito
        diaVencimento.texto = f.diaVencimento
        observacoes.texto = f.observacoes
        ativo.checked = f.ativo
        veiculos.clear()
        for (const v of f.veiculos)
            veiculos.append({ placa: v.placa, descricao: v.descricao })
        for (const c of todosCampos())
            c.erro = ""
        alterado = false
    }

    function todosCampos() {
        return [codigo, documento, nome, fantasia, ie, telefone, celular, email, cep, logradouro, numero,
                complemento, bairro, municipio, limite, diaVencimento, observacoes, uf]
    }

    function coletar() {
        const lista = []
        for (let i = 0; i < veiculos.count; i++) {
            const v = veiculos.get(i)
            lista.push({ placa: v.placa, descricao: v.descricao })
        }
        return {
            id: ficha ? ficha.id : "",
            codigo: codigo.texto,
            cliente: cliente.checked,
            fornecedor: fornecedor.checked,
            tipo: tipoJ.checked ? "J" : "F",
            nome: nome.texto,
            fantasia: tipoJ.checked ? fantasia.texto : "",
            documento: documento.texto,
            ie: tipoJ.checked ? ie.texto : "",
            indicadorIe: tipoJ.checked && isento.checked ? 2 : 9,
            telefone: telefone.texto,
            celular: celular.texto,
            email: email.texto,
            cep: cep.texto,
            logradouro: logradouro.texto,
            numero: numero.texto,
            complemento: complemento.texto,
            bairro: bairro.texto,
            municipio: municipio.texto,
            municipioIbge: ficha ? ficha.municipioIbge : "",
            uf: uf.indice > 0 ? ufs[uf.indice] : "",
            limiteCredito: limite.texto,
            diaVencimento: diaVencimento.texto,
            observacoes: observacoes.texto,
            ativo: ativo.checked,
            veiculos: lista
        }
    }

    function salvar() {
        const eraNova = nova
        const r = Api.chamar("pessoa_salvar", coletar())
        if (r.ok) {
            preencher(r.dados)
            pessoaId = r.dados.id
            janela.avisar(eraNova ? "Cadastro feito." : "Alterações salvas.", "ok")
        } else {
            mostrarErro(r.campo, r.erro)
        }
    }

    function mostrarErro(campo, mensagem) {
        const campos = {
            codigo: codigo, documento: documento, nome: nome, fantasia: fantasia, ie: ie, telefone: telefone,
            celular: celular, email: email, cep: cep, logradouro: logradouro, numero: numero,
            complemento: complemento, bairro: bairro, municipio: municipio, uf: uf, limiteCredito: limite, diaVencimento: diaVencimento,
            observacoes: observacoes
        }
        let alvo = campo ? campos[campo] : null
        const v = campo ? campo.match(/^veiculos\.(\d+)\.(\w+)$/) : null
        if (v) {
            const linha = repVeiculos.itemAt(parseInt(v[1]))
            if (linha)
                alvo = linha.campo(v[2])
        }
        if (alvo) {
            alvo.erro = mensagem
            if (typeof alvo.focar === "function")
                alvo.focar()
        }
        janela.avisar(mensagem, "erro")
    }

    function marcarAlterado() {
        if (!carregando)
            alterado = true
    }

    function aoEsc() {
        if (alterado)
            janela.confirmar("Descartar alterações?", "O que você mudou neste cadastro ainda não foi salvo.",
                             "Descartar", () => janela.voltar(), "perigo")
        else
            janela.voltar()
    }

    function dataHora(iso) {
        return iso ? Qt.formatDateTime(new Date(iso), "dd/MM/yyyy 'às' HH:mm") : ""
    }

    function adicionarVeiculo() {
        veiculos.append({ placa: "", descricao: "" })
        marcarAlterado()
        Qt.callLater(() => {
            const linha = repVeiculos.itemAt(veiculos.count - 1)
            if (linha)
                linha.focar()
        })
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
        spacing: 12

        Cabecalho {
            sensivel: true
            textoVoltar: tela.papel === "fornecedor" ? "Fornecedores" : "Clientes"
            titulo: tela.nova ? (tela.papel === "fornecedor" ? "Novo fornecedor" : "Novo cliente") : tela.ficha.nome
            subtitulo: {
                if (!tela.ficha)
                    return ""
                if (tela.nova)
                    return "Só o nome é obrigatório. Para emitir nota, preencha CPF/CNPJ e endereço."
                let s = "Código " + (tela.ficha.codigo || "a definir")
                if (tela.ficha.compras > 0)
                    s += " · " + tela.ficha.compras + (tela.ficha.compras === 1 ? " compra lançada" : " compras lançadas")
                if (tela.ficha.atualizadoEm)
                    s += " · alterado" + (tela.ficha.atualizadoPor ? " por " + tela.ficha.atualizadoPor : "")
                         + " em " + tela.dataHora(tela.ficha.atualizadoEm)
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

                ColumnLayout {
                    spacing: 14
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignTop

                    Faixa {
                        titulo: "Identificação"
                        color: Tema.pergaminho
                        Layout.fillWidth: true

                        Linha {
                            colunasEstreito: 2
                            spacing: 22
                            Layout.fillWidth: true
                            ButtonGroup { id: grupoTipo }
                            Marcar {
                                id: tipoF
                                text: "Pessoa física"
                                ButtonGroup.group: grupoTipo
                                onToggled: tela.marcarAlterado()
                            }
                            Marcar {
                                id: tipoJ
                                text: "Empresa (CNPJ)"
                                ButtonGroup.group: grupoTipo
                                onToggled: tela.marcarAlterado()
                            }
                            Item { Layout.fillWidth: true }
                            Marcar {
                                id: cliente
                                text: "Cliente"
                                onToggled: tela.marcarAlterado()
                            }
                            Marcar {
                                id: fornecedor
                                text: "Fornecedor"
                                onToggled: tela.marcarAlterado()
                            }
                        }
                        Linha {
                            spacing: 12
                            Layout.fillWidth: true
                            Campo {
                                id: documento
                                sensivel: true
                                rotulo: tela.juridica ? "CNPJ" : "CPF"
                                dica: tela.juridica ? "00.000.000/0000-00" : "000.000.000-00"
                                maximo: 20
                                maiusculas: true
                                Layout.preferredWidth: 220
                                onEditado: {
                                    tela.marcarAlterado()
                                    const limpo = texto.replace(/[^0-9A-Z]/g, "")
                                    if (limpo.length === 14)
                                        tipoJ.checked = true
                                    else if (limpo.length === 11 && /^[0-9]+$/.test(limpo))
                                        tipoF.checked = true
                                }
                            }
                            Campo {
                                id: nome
                                rotulo: tela.juridica ? "Razão social" : "Nome"
                                maximo: 100
                                tamanhoFonte: 18
                                altura: 40
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                            }
                            Campo {
                                id: codigo
                                rotulo: "Código"
                                // Vazio: o Rustinho dá o código ao salvar, da faixa deste caixa.
                                dica: !tela.ficha ? "" : tela.nova
                                      ? (tela.ficha.codigoPrevisto ? "Automático: " + tela.ficha.codigoPrevisto : "Automático")
                                      : "A definir"
                                maximo: 20
                                Layout.preferredWidth: 150
                                onEditado: tela.marcarAlterado()
                            }
                        }
                        // Código a definir, caixa bloqueado ou código que também é de outra pessoa.
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
                            visible: tela.juridica
                            spacing: 12
                            Layout.fillWidth: true
                            Campo {
                                id: fantasia
                                rotulo: "Nome fantasia"
                                maximo: 60
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                            }
                            Campo {
                                id: ie
                                rotulo: "Inscrição estadual"
                                maximo: 20
                                maiusculas: true
                                Layout.preferredWidth: 200
                                onEditado: {
                                    tela.marcarAlterado()
                                    if (texto !== "")
                                        isento.checked = false
                                }
                            }
                            Marcar {
                                id: isento
                                text: "Isento de IE"
                                Layout.alignment: Qt.AlignBottom
                                Layout.bottomMargin: 6
                                onToggled: {
                                    tela.marcarAlterado()
                                    if (checked)
                                        ie.texto = ""
                                }
                            }
                        }
                        Marcar {
                            id: ativo
                            visible: !tela.nova
                            text: "Ativo (aparece na busca)"
                            onToggled: tela.marcarAlterado()
                        }
                    }

                    Faixa {
                        titulo: "Endereço"
                        color: Tema.azul
                        Layout.fillWidth: true

                        Linha {
                            spacing: 12
                            Layout.fillWidth: true
                            Campo {
                                id: cep
                                rotulo: "CEP"
                                maximo: 9
                                Layout.preferredWidth: 130
                                onEditado: tela.marcarAlterado()
                            }
                            Campo {
                                id: logradouro
                                rotulo: "Rua"
                                maximo: 80
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                            }
                            Campo {
                                id: numero
                                rotulo: "Número"
                                maximo: 20
                                Layout.preferredWidth: 100
                                onEditado: tela.marcarAlterado()
                            }
                        }
                        Linha {
                            spacing: 12
                            Layout.fillWidth: true
                            Campo {
                                id: complemento
                                rotulo: "Complemento"
                                maximo: 60
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                            }
                            Campo {
                                id: bairro
                                rotulo: "Bairro"
                                maximo: 60
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                            }
                        }
                        Linha {
                            spacing: 12
                            Layout.fillWidth: true
                            Campo {
                                id: municipio
                                rotulo: "Cidade"
                                maximo: 60
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                            }
                            Selecao {
                                id: uf
                                rotulo: "UF"
                                modelo: tela.ufs
                                Layout.preferredWidth: 100
                                onEscolhido: tela.marcarAlterado()
                            }
                        }
                    }

                    Faixa {
                        visible: cliente.checked
                        titulo: "Carros"
                        subtitulo: "a busca acha o cliente pela placa"
                        color: Tema.pergaminho
                        Layout.fillWidth: true

                        Repeater {
                            id: repVeiculos
                            model: veiculos
                            delegate: LinhaVeiculo {
                                Layout.fillWidth: true
                                onMudou: (campo, valor) => {
                                    veiculos.setProperty(index, campo, valor)
                                    tela.marcarAlterado()
                                }
                                onRemover: {
                                    veiculos.remove(index)
                                    tela.marcarAlterado()
                                }
                            }
                        }
                        Botao {
                            text: "Adicionar carro"
                            icone: "mais"
                            tipo: "secundario"
                            onClicked: tela.adicionarVeiculo()
                        }
                    }
                }

                ColumnLayout {
                    spacing: 14
                    Layout.preferredWidth: Math.min(420, Math.max(340, corpo.width * 0.33))
                    Layout.maximumWidth: 420
                    Layout.alignment: Qt.AlignTop

                    Faixa {
                        titulo: "Contato"
                        color: Tema.verde
                        corTitulo: Tema.verdeEscuro
                        Layout.fillWidth: true

                        Linha {
                            spacing: 12
                            Layout.fillWidth: true
                            Campo {
                                id: celular
                                rotulo: "Celular / WhatsApp"
                                maximo: 20
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                            }
                            Campo {
                                id: telefone
                                rotulo: "Telefone"
                                maximo: 20
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                            }
                        }
                        Campo {
                            id: email
                            rotulo: "E-mail"
                            maximo: 80
                            Layout.fillWidth: true
                            onEditado: tela.marcarAlterado()
                        }
                    }

                    Faixa {
                        visible: cliente.checked
                        titulo: "Crediário"
                        color: Tema.azul
                        Layout.fillWidth: true
                        Linha {
                            spacing: 12
                            Layout.fillWidth: true
                            Campo {
                                id: limite
                                rotulo: "Limite de crédito"
                                prefixo: "R$"
                                numerico: true
                                maximo: 16
                                dica: "Sem limite"
                                Layout.fillWidth: true
                                onEditado: tela.marcarAlterado()
                            }
                            Campo {
                                id: diaVencimento
                                rotulo: "Dia do convênio"
                                numerico: true
                                maximo: 2
                                dica: "1 a 31"
                                Layout.preferredWidth: 150
                                onEditado: tela.marcarAlterado()
                            }
                        }
                        Nota {
                            text: "Passar do limite só gera aviso na venda. Com o dia do convênio, as parcelas vencem sempre nesse dia; sem ele, a cada 30 dias."
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }
                        Botao {
                            visible: !tela.nova
                            text: "Ver o que deve"
                            tipo: "secundario"
                            onClicked: tela.janela.abrir("TelaReceber", { clienteInicial: tela.pessoaId })
                        }
                    }

                    Faixa {
                        titulo: "Observações"
                        color: Tema.pergaminho
                        Layout.fillWidth: true
                        AreaTexto {
                            id: observacoes
                            dica: tela.papel === "fornecedor" ? "Ex.: vendedor João, entrega às terças" : "Ex.: mecânico, prefere peça original"
                            Layout.fillWidth: true
                            onEditado: tela.marcarAlterado()
                        }
                    }

                    Faixa {
                        visible: !tela.nova && tela.ficha && tela.ficha.historico.length > 0
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

    Component.onCompleted: carregar()
    StackView.onActivated: {
        if (nova)
            documento.focar()
        else
            rolagem.forceActiveFocus()
    }
}
