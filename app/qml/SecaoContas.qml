import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Contas da loja: o Caixa geral (cofre) e os bancos, com o extrato de cada um.
FocusScope {
    id: secao
    readonly property var janela: ApplicationWindow.window
    property var contas: []
    property string contaId: "caixa-geral"
    property var extrato: null
    property var categorias: []
    property bool inativas: false

    readonly property var atual: contas.find(c => c.id === contaId) || null

    function carregar() {
        const r = Api.chamar("contas_loja", { quais: inativas ? "todas" : "" })
        if (r.ok)
            contas = r.dados
        if (!contas.some(c => c.id === contaId) && contas.length > 0)
            contaId = contas[0].id
        const g = Api.chamar("categorias", {})
        if (g.ok)
            categorias = g.dados
        carregarExtrato()
    }
    function carregarExtrato() {
        const r = Api.chamar("conta_loja_extrato", { id: contaId })
        extrato = r.ok ? r.dados : null
    }
    function abrir(id) {
        contaId = id
        carregarExtrato()
    }
    function novaConta() {
        cadastro.abrirCom(null)
    }
    function hoje() {
        return Qt.formatDate(new Date(), "dd/MM/yyyy")
    }

    RowLayout {
        anchors.fill: parent
        spacing: 18

        // As contas
        ColumnLayout {
            spacing: 8
            Layout.preferredWidth: 360
            Layout.maximumWidth: 400
            Layout.fillHeight: true
            ListView {
                id: lista
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 8
                model: secao.contas
                boundsBehavior: Flickable.StopAtBounds
                delegate: Rectangle {
                    id: cartao
                    required property var modelData
                    required property int index
                    readonly property bool escolhida: modelData.id === secao.contaId
                    readonly property color base: (index + 1) % 2 === 0 ? Tema.azul : Tema.pergaminho
                    width: lista.width
                    implicitHeight: 82
                    radius: Tema.raio
                    color: escolhida ? Tema.tinta : sobre.hovered ? Qt.darker(base, 1.04) : base
                    opacity: modelData.ativo ? 1 : 0.6
                    ColumnLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 20
                        anchors.rightMargin: 18
                        anchors.topMargin: 12
                        anchors.bottomMargin: 12
                        spacing: 2
                        RowLayout {
                            spacing: 8
                            Layout.fillWidth: true
                            Text {
                                text: cartao.modelData.nome
                                color: cartao.escolhida ? Tema.claro : Tema.tinta
                                font.family: Tema.titulo
                                font.weight: Font.Bold
                                font.pixelSize: 22
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                            Text {
                                text: cartao.modelData.ativo ? (cartao.modelData.tipo === "banco" ? "banco" : "dinheiro") : "desativada"
                                color: cartao.escolhida ? Qt.rgba(0.95, 0.93, 0.88, 0.7) : Tema.apagado
                                font.family: Tema.serifa
                                font.italic: true
                                font.pixelSize: 13
                            }
                        }
                        Text {
                            text: "R$ " + Tema.valor(cartao.modelData.saldo)
                            color: cartao.escolhida ? (cartao.modelData.saldo.startsWith("-") ? "#F0B8AE" : Tema.claro)
                                                    : (cartao.modelData.saldo.startsWith("-") ? Tema.vermelho : Tema.tinta)
                            font.family: Tema.texto
                            font.weight: Font.Bold
                            font.pixelSize: 20
                        }
                    }
                    HoverHandler { id: sobre; cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: secao.abrir(cartao.modelData.id) }
                }
                footer: ColumnLayout {
                    width: lista.width
                    spacing: 6
                    Nota {
                        text: "O Caixa geral é o dinheiro guardado na loja (cofre). A gaveta de cada caixa fica no Caixa; a sangria pode vir para cá."
                        font.pixelSize: 13
                        Layout.fillWidth: true
                        Layout.topMargin: 10
                    }
                    Marcar {
                        text: "Mostrar desativadas"
                        checked: secao.inativas
                        onToggled: {
                            secao.inativas = checked
                            secao.carregar()
                        }
                    }
                }
            }
        }

        // O extrato
        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: Tema.raio
            color: Qt.rgba(1, 1, 1, 0.28)

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 20
                spacing: 12

                Linha {
                    spacing: 12
                    Layout.fillWidth: true
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        Titulo {
                            text: secao.atual ? secao.atual.nome : ""
                            font.pixelSize: 32
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        Nota {
                            text: secao.atual ? "Saldo inicial R$ " + Tema.valor(secao.atual.saldoInicial) : ""
                            font.pixelSize: 14
                        }
                    }
                    ColumnLayout {
                        spacing: 0
                        Rotulo { text: "Saldo hoje"; Layout.alignment: Qt.AlignRight }
                        Text {
                            text: secao.atual ? "R$ " + Tema.valor(secao.atual.saldo) : ""
                            color: secao.atual && secao.atual.saldo.startsWith("-") ? Tema.vermelho : Tema.tinta
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: 30
                            Layout.alignment: Qt.AlignRight
                        }
                    }
                }

                Flow {
                    spacing: 8
                    Layout.fillWidth: true
                    visible: !!secao.atual && secao.atual.ativo
                    Botao { text: "Entrada"; icone: "mais"; tipo: "secundario"; onClicked: movimento.abrirCom("entrada") }
                    Botao { text: "Saída"; tipo: "secundario"; onClicked: movimento.abrirCom("saida") }
                    Botao { text: "Transferir"; tipo: "secundario"; onClicked: transferencia.abrirCom() }
                    Botao { text: "Conferir saldo"; tipo: "secundario"; onClicked: conferencia.abrirCom() }
                    Botao { text: "Editar"; tipo: "discreto"; icone: "lapis"; onClicked: cadastro.abrirCom(secao.atual) }
                }

                ListView {
                    id: listaExtrato
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 4
                    model: secao.extrato ? secao.extrato.linhas : []
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar {}
                    delegate: Rectangle {
                        id: mov
                        required property var modelData
                        width: listaExtrato.width - 10
                        implicitHeight: 52
                        radius: Tema.raioCampo
                        color: modelData.futuro ? Qt.rgba(1, 1, 1, 0.25) : Qt.rgba(1, 1, 1, 0.5)
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 14
                            anchors.rightMargin: 8
                            spacing: 12
                            opacity: mov.modelData.estornado ? 0.5 : 1
                            Text {
                                text: mov.modelData.data
                                color: Tema.tintaSuave
                                font.family: Tema.texto
                                font.weight: Font.DemiBold
                                font.pixelSize: 14
                                Layout.preferredWidth: 86
                            }
                            ColumnLayout {
                                spacing: 0
                                Layout.fillWidth: true
                                Text {
                                    text: mov.modelData.descricao
                                    color: Tema.tinta
                                    font.family: Tema.texto
                                    font.weight: Font.DemiBold
                                    font.pixelSize: 15
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                                Text {
                                    text: [mov.modelData.tipo, mov.modelData.categoria, mov.modelData.futuro ? "ainda não saiu" : "",
                                           mov.modelData.estornado ? "ESTORNADO: " + mov.modelData.motivoEstorno : ""].filter(x => !!x).join("  ·  ")
                                    color: Tema.tintaSuave
                                    font.family: Tema.serifa
                                    font.italic: true
                                    font.pixelSize: 12
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                            }
                            Text {
                                text: (mov.modelData.entrada ? "+ " : "− ") + "R$ " + mov.modelData.valor
                                color: mov.modelData.entrada ? Tema.verdeEscuro : Tema.vermelho
                                font.family: Tema.texto
                                font.weight: Font.Bold
                                font.pixelSize: 15
                                font.strikeout: mov.modelData.estornado
                                horizontalAlignment: Text.AlignRight
                                Layout.preferredWidth: 130
                            }
                            Text {
                                text: mov.modelData.estornado ? "" : "R$ " + Tema.valor(mov.modelData.saldo)
                                color: Tema.tintaSuave
                                font.family: Tema.texto
                                font.pixelSize: 13
                                horizontalAlignment: Text.AlignRight
                                Layout.preferredWidth: 110
                            }
                            Item {
                                Layout.preferredWidth: 86
                                implicitHeight: 32
                                Botao {
                                    anchors.fill: parent
                                    visible: mov.modelData.estornavel
                                    text: "Estornar"
                                    tipo: "discreto"
                                    onClicked: motivo.perguntar("Estornar o movimento?", mov.modelData.descricao + " · R$ " + mov.modelData.valor,
                                                                "Estornar", m => Api.chamar("movimento_estornar", { id: mov.modelData.id, motivo: m }))
                                }
                            }
                        }
                    }
                    Nota {
                        anchors.centerIn: parent
                        visible: listaExtrato.count === 0
                        text: "Nenhum movimento ainda."
                        font.pixelSize: 16
                    }
                }
            }
        }
    }

    // ---------- Cadastro da conta ----------
    Painel {
        id: cadastro
        property string id_: ""
        property string erro: ""
        width: Math.min(520, Tema.largura - 24)
        function abrirCom(c) {
            id_ = c ? c.id : ""
            erro = ""
            cNome.texto = c ? c.nome : ""
            cTipo.indice = c && c.tipo === "caixa" ? 1 : 0
            cInicial.texto = c ? c.saldoInicial : ""
            cAtiva.checked = c ? c.ativo : true
            open()
            cNome.focar()
        }
        function salvar() {
            const r = Api.chamar("conta_loja_salvar", { id: id_, nome: cNome.texto, tipo: cTipo.indice === 1 ? "caixa" : "banco",
                                                        saldoInicial: cInicial.texto, ativo: cAtiva.checked })
            if (!r.ok) {
                erro = r.erro
                return
            }
            close()
            secao.contaId = r.dados
            secao.carregar()
        }
        contentItem: ColumnLayout {
            spacing: 12
            Titulo { text: cadastro.id_ ? "Editar conta" : "Nova conta da loja"; font.pixelSize: 30; Layout.fillWidth: true }
            Campo { id: cNome; rotulo: "Nome"; dica: "Ex.: Sicoob, Banco do Brasil, Cofre"; maximo: 40; Layout.fillWidth: true }
            Linha {
                spacing: 12
                Layout.fillWidth: true
                Selecao { id: cTipo; rotulo: "Tipo"; modelo: ["Banco", "Dinheiro (cofre)"]; Layout.fillWidth: true }
                Campo {
                    id: cInicial
                    rotulo: "Saldo no começo"
                    prefixo: "R$"
                    numerico: true
                    maximo: 14
                    Layout.preferredWidth: 200
                    onConfirmado: cadastro.salvar()
                }
            }
            Nota {
                text: "Saldo no começo: o que tinha na conta quando começou a usar o Rustinho."
                font.pixelSize: 13
                Layout.fillWidth: true
            }
            Marcar { id: cAtiva; text: "Conta em uso"; visible: cadastro.id_ !== "" }
            TextoErro { text: cadastro.erro }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao { text: "Cancelar"; tipo: "discreto"; onClicked: cadastro.close() }
                Botao { text: "Salvar"; icone: "check"; onClicked: cadastro.salvar() }
            }
        }
    }

    // ---------- Entrada ou saída ----------
    Painel {
        id: movimento
        property string tipo: "entrada"
        property string erro: ""
        readonly property var cats: secao.categorias.filter(c => c.tipo === (tipo === "entrada" ? "receita" : "despesa"))
        width: Math.min(560, Tema.largura - 24)
        function abrirCom(t) {
            tipo = t
            erro = ""
            mValor.texto = ""
            mData.texto = secao.hoje()
            mDescricao.texto = ""
            mCategoria.indice = 0
            open()
            mDescricao.focar()
        }
        function salvar() {
            const r = Api.chamar("movimento_lancar", {
                contaId: secao.contaId, tipo: tipo, valor: mValor.texto, data: mData.texto, descricao: mDescricao.texto,
                categoriaId: mCategoria.indice > 0 ? cats[mCategoria.indice - 1].id : ""
            })
            if (!r.ok) {
                erro = r.erro
                return
            }
            close()
            secao.carregar()
        }
        contentItem: ColumnLayout {
            spacing: 12
            Titulo {
                text: (movimento.tipo === "entrada" ? "Entrada em " : "Saída de ") + (secao.atual ? secao.atual.nome : "")
                font.pixelSize: 30
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            Nota {
                text: movimento.tipo === "entrada" ? "Dinheiro que entrou fora das vendas: rendimento, dinheiro do dono, devolução..."
                                                   : "Saída sem conta lançada: tarifa do banco, retirada do dono... (contas e boletos: use o A pagar)"
                font.pixelSize: 14
                Layout.fillWidth: true
            }
            Campo { id: mDescricao; rotulo: "Do que é"; maximo: 80; Layout.fillWidth: true; onConfirmado: mValor.focar() }
            RowLayout {
                spacing: 12
                Layout.fillWidth: true
                Campo { id: mValor; rotulo: "Valor"; prefixo: "R$"; numerico: true; maximo: 14; Layout.fillWidth: true; onConfirmado: movimento.salvar() }
                Campo { id: mData; rotulo: "Data"; maximo: 10; Layout.preferredWidth: 150 }
            }
            Selecao {
                id: mCategoria
                rotulo: "Categoria"
                modelo: ["Sem categoria"].concat(movimento.cats.map(c => c.nome))
                Layout.fillWidth: true
            }
            TextoErro { text: movimento.erro }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao { text: "Cancelar"; tipo: "discreto"; onClicked: movimento.close() }
                Botao { text: "Lançar"; icone: "check"; onClicked: movimento.salvar() }
            }
        }
    }

    // ---------- Transferir ----------
    Painel {
        id: transferencia
        property string erro: ""
        readonly property var outras: secao.contas.filter(c => c.ativo && c.id !== secao.contaId)
        width: Math.min(560, Tema.largura - 24)
        function abrirCom() {
            erro = ""
            tValor.texto = ""
            tData.texto = secao.hoje()
            tDescricao.texto = ""
            tPara.indice = 0
            open()
            tValor.focar()
        }
        function salvar() {
            const para = outras[tPara.indice]
            const r = Api.chamar("transferir", { de: secao.contaId, para: para ? para.id : "", valor: tValor.texto,
                                                 data: tData.texto, descricao: tDescricao.texto })
            if (!r.ok) {
                erro = r.erro
                return
            }
            close()
            secao.carregar()
        }
        contentItem: ColumnLayout {
            spacing: 12
            Titulo { text: "Transferir de " + (secao.atual ? secao.atual.nome : ""); font.pixelSize: 30; elide: Text.ElideRight; Layout.fillWidth: true }
            Nota {
                text: transferencia.outras.length === 0 ? "Cadastre outra conta antes (“Nova conta”)." : "Ex.: depósito do dinheiro do cofre no banco."
                font.pixelSize: 14
                Layout.fillWidth: true
            }
            Selecao { id: tPara; rotulo: "Para"; modelo: transferencia.outras.map(c => c.nome); Layout.fillWidth: true }
            RowLayout {
                spacing: 12
                Layout.fillWidth: true
                Campo { id: tValor; rotulo: "Valor"; prefixo: "R$"; numerico: true; maximo: 14; Layout.fillWidth: true; onConfirmado: transferencia.salvar() }
                Campo { id: tData; rotulo: "Data"; maximo: 10; Layout.preferredWidth: 150 }
            }
            Campo { id: tDescricao; rotulo: "Observação (opcional)"; maximo: 60; Layout.fillWidth: true }
            TextoErro { text: transferencia.erro }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao { text: "Cancelar"; tipo: "discreto"; onClicked: transferencia.close() }
                Botao { text: "Transferir"; icone: "check"; enabled: transferencia.outras.length > 0; onClicked: transferencia.salvar() }
            }
        }
    }

    // ---------- Conferir saldo ----------
    Painel {
        id: conferencia
        property string erro: ""
        width: Math.min(520, Tema.largura - 24)
        function abrirCom() {
            erro = ""
            real.texto = ""
            open()
            real.focar()
        }
        function salvar() {
            const r = Api.chamar("conta_loja_conferir", { contaId: secao.contaId, saldo: real.texto })
            if (!r.ok) {
                erro = r.erro
                return
            }
            close()
            janela.avisar(r.dados === "0,00" ? "Saldo certinho." : "Ajuste de R$ " + r.dados + " lançado.", "ok")
            secao.carregar()
        }
        contentItem: ColumnLayout {
            spacing: 12
            Titulo { text: "Conferir saldo"; font.pixelSize: 30; Layout.fillWidth: true }
            Nota {
                text: "Pelo Rustinho, " + (secao.atual ? secao.atual.nome + " tem R$ " + Tema.valor(secao.atual.saldo) : "") +
                      ". Digite o saldo de verdade (do extrato do banco ou contado no cofre); a diferença vira um ajuste."
                font.pixelSize: 14
                Layout.fillWidth: true
            }
            Campo { id: real; rotulo: "Saldo de verdade"; prefixo: "R$"; numerico: true; maximo: 14; altura: 52; tamanhoFonte: 22; Layout.fillWidth: true; onConfirmado: conferencia.salvar() }
            TextoErro { text: conferencia.erro }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao { text: "Cancelar"; tipo: "discreto"; onClicked: conferencia.close() }
                Botao { text: "Conferir"; icone: "check"; onClicked: conferencia.salvar() }
            }
        }
    }

    PerguntaMotivo { id: motivo; onFeito: secao.carregar() }

    Component.onCompleted: carregar()
}
