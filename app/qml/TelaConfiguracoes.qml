import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Configurações, uma parte de cada vez nas pílulas: a loja, vendas,
// crediário, impressão, backup, os caixas da loja e "sobre".
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    property var config: null
    property bool alterado: false
    property string secao: "loja"
    readonly property bool temSalvar: ["loja", "vendas", "receber", "impressao", "gerente"].includes(secao)
    // Gerente (ou senha do gerente digitada agora): vê as configurações da
    // loja toda. O caixa vê só as deste computador.
    property bool completo: false
    property bool verBackup: false
    property bool verGerente: false
    property var gerente: null       // pílula Gerente: { acoes, usuarios }
    // Se um gerente liberou esta tela para um caixa, acaba ao sair dela.
    Component.onDestruction: Api.chamar("gerente_encerrar")

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
        alterado = false
    }

    function liberar() {
        janela.comGerente("configuracoes", () => {
            tela.completo = true
            tela.verBackup = true
            tela.verGerente = true
            carregar()
        }, true)
    }

    function carregarGerente() {
        const r = Api.chamar("gerente_ler")
        gerente = r.ok ? r.dados : null
    }

    function salvarGerente() {
        const acoes = {}
        for (const a of gerente.acoes)
            acoes[a.id] = a.soGerente
        const gerentes = {}
        for (const u of gerente.usuarios)
            gerentes[u.id] = u.gerente
        const r = Api.chamar("gerente_salvar", { acoes: acoes, gerentes: gerentes })
        if (r.ok) {
            gerente = r.dados
            alterado = false
            janela.atualizarEstado()
            janela.avisar("Salvo.", "ok")
        } else {
            janela.avisar(r.erro, "erro")
            carregarGerente()
        }
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
        const r = Api.chamar("config_salvar", { loja: loja.texto, caixa: caixa.texto, tabelas: lista,
                                                cnpj: cnpjLoja.texto, uf: ufLoja.indice > 0 ? ufLoja.modelo[ufLoja.indice] : "",
                                                endereco: enderecoLoja.texto, telefone: telefoneLoja.texto,
                                                imprimirDireto: imprimirDireto.checked,
                                                crediarioMulta: multa.texto, crediarioJuros: jurosMes.texto,
                                                crediarioCarencia: carencia.texto, orcamentoValidade: validadeOrc.texto,
                                                rodapeVenda: rodapeVenda.texto, rodapeOrcamento: rodapeOrcamento.texto, rodapeOs: rodapeOs.texto,
                                                imprimirLocal: imprimirLocal.checked })
        if (r.ok) {
            janela.atualizarEstado()
            carregar()
            janela.avisar("Configurações salvas.", "ok")
            return
        }
        const campos = { loja: [loja, "loja"], caixa: [caixa, "loja"], cnpj: [cnpjLoja, "loja"], uf: [ufLoja, "loja"],
                         crediarioMulta: [multa, "receber"], crediarioJuros: [jurosMes, "receber"], crediarioCarencia: [carencia, "receber"],
                         orcamentoValidade: [validadeOrc, "vendas"], rodapeVenda: [rodapeVenda, "impressao"],
                         rodapeOrcamento: [rodapeOrcamento, "impressao"], rodapeOs: [rodapeOs, "impressao"] }
        if (campos[r.campo]) {
            const [alvo, onde] = campos[r.campo]
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
    }

    property var sync: null

    function carregarSync() {
        const r = Api.chamar("sync_ler")
        if (r.ok) {
            sync = r.dados
            syncLigado.checked = sync.ligado
            syncEnderecos.texto = sync.enderecos
            syncSenha.texto = ""
        }
    }

    function salvarSync() {
        syncSenha.erro = ""
        syncEnderecos.erro = ""
        const r = Api.chamar("sync_salvar", { ligado: syncLigado.checked, senha: syncSenha.texto, enderecos: syncEnderecos.texto })
        if (r.ok) {
            sync = r.dados
            syncSenha.texto = ""
            janela.avisar(syncLigado.checked ? "Sincronização ligada. Os outros caixas aparecem em alguns segundos." : "Sincronização desligada.", "ok")
        } else if (r.campo === "enderecos") {
            syncEnderecos.erro = r.erro
        } else {
            syncSenha.erro = r.erro
        }
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
        if (alterado)
            janela.confirmar("Descartar alterações?", "As configurações mudadas ainda não foram salvas.",
                             "Descartar", () => janela.voltar(), "perigo")
        else
            janela.voltar()
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
            subtitulo: ({
                loja: tela.completo ? "A loja, este computador e a sua senha" : "Este computador e a sua senha",
                gerente: "Quem é gerente e o que só o gerente pode fazer",
                vendas: "Tabelas de preço e orçamentos",
                receber: "Multa e juros de quem paga atrasado",
                impressao: "Comprovante, orçamento e impressora",
                backup: "Cópias dos dados no computador e no Google Drive",
                caixas: "Os computadores da loja trocando dados pelo Wi-Fi",
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
                { id: "backup", nome: "Backup" },
                { id: "caixas", nome: "Caixas da loja" },
                { id: "gerente", nome: "Gerente" },
                { id: "sobre", nome: "Sobre" }
            ].filter(p => (p.id !== "backup" || tela.verBackup) && (p.id !== "gerente" || tela.verGerente)) : [
                { id: "loja", nome: "Este computador" },
                { id: "impressao", nome: "Impressão" },
                { id: "sobre", nome: "Sobre" }
            ]
            atual: tela.secao
            onEscolhido: id => tela.secao = id
        }

        Flickable {
            id: rolagem
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
                            maximo: 128
                            Layout.fillWidth: true
                            onConfirmado: nova.focar()
                        }
                        Campo {
                            id: nova
                            rotulo: "Nova senha"
                            senha: true
                            maximo: 128
                            Layout.fillWidth: true
                            onConfirmado: nova2.focar()
                        }
                        Campo {
                            id: nova2
                            rotulo: "Repita a nova senha"
                            senha: true
                            maximo: 128
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
                            Campo {
                                id: lucroTabela
                                texto: linhaTabela.modelData.lucroPadrao
                                sufixo: "%"
                                numerico: true
                                sensivel: true
                                maximo: 10
                                dica: "—"
                                Layout.preferredWidth: 150
                                onEditado: tela.alterado = true
                            }
                        }
                    }
                    Nota {
                        text: "O lucro sugerido já vem preenchido quando se cadastra uma peça nova. As tabelas 2 a 5 aparecem em “Outros preços” no cadastro da peça."
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
                    subtitulo: "sincronização pelo Wi-Fi"
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
                            maximo: 128
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
                    Nota {
                        text: tela.sync ? "Endereço deste caixa: " + (tela.sync.meuEndereco || "sem rede") : ""
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
                                width: 10
                                height: 10
                                radius: 5
                                color: modelData.erro ? Tema.vermelho : modelData.sincronizadoEm ? Tema.verdeEscuro : Tema.apagado
                            }
                            Text {
                                text: (modelData.nome || "Caixa") + " · " + (modelData.erro ? modelData.erro : "sincronizado " + tela.quandoFoi(modelData.sincronizadoEm))
                                color: modelData.erro ? Tema.vermelho : Tema.tinta
                                font.family: Tema.texto
                                font.pixelSize: 14
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                        }
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
                        text: "Cada gerente troca a própria senha em “Minha senha”. A senha precisa ter pelo menos 4 caracteres."
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

    Component.onCompleted: {
        completo = janela.podeGerente("configuracoes")
        verBackup = completo && janela.podeGerente("backup")
        verGerente = completo && janela.podeGerente("gerente")
        carregar()
    }
}
