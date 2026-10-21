import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts
import QtCore
import rustinho

// Backup nas Configurações: como está, senha, Google Drive, onde e quando,
// e restaurar. Tudo em faixas, sem abas.
ColumnLayout {
    id: secao
    spacing: 14
    readonly property var janela: ApplicationWindow.window
    property var b: null
    property bool estavaOcupado: false
    property string onde: "computador"      // lista: "computador" ou "nuvem"
    property var nuvem: []
    property bool buscandoNuvem: false
    property bool todas: false
    property var pedirSenhaDe: null         // cópia que pediu outra senha
    property bool restaurando: false
    property bool conectando: false

    function carregar() {
        const r = Api.chamar("backup_ler")
        if (!r.ok) {
            janela.avisar(r.erro, "erro")
            return
        }
        const antes = estavaOcupado
        b = r.dados
        estavaOcupado = b.ocupado
        if (antes && !b.ocupado && conectando) {
            conectando = false
            if (b.drive)
                janela.avisar("Google Drive conectado. O backup mais novo já foi para lá.", "ok")
            else
                janela.avisar("Não deu para conectar: " + (b.situacao.nuvemErro || "o navegador foi fechado antes de permitir."), "erro")
        } else if (antes && !b.ocupado) {
            if (b.situacao.erro)
                janela.avisar("O backup não deu certo: " + b.situacao.erro, "erro")
            else if (b.nuvem && b.situacao.nuvemErro)
                janela.avisar("Backup feito no computador, mas não foi para o Google Drive: " + b.situacao.nuvemErro, "erro")
            else
                janela.avisar("Backup feito e conferido.", "ok")
        }
        pasta.texto = b.pasta
        hora.texto = b.hora
        dias.texto = b.dias
    }

    function salvar() {
        pasta.erro = ""; hora.erro = ""; dias.erro = ""
        janela.chamarGerente("backup_salvar", { pasta: pasta.texto, hora: hora.texto, dias: dias.texto }, r => {
            if (r.ok) {
                b = r.dados
                janela.avisar("Ajustes do backup salvos.", "ok")
                return
            }
            const campos = { pasta: pasta, hora: hora, dias: dias }
            if (campos[r.campo])
                campos[r.campo].erro = r.erro
            else
                janela.avisar(r.erro, "erro")
        })
    }

    function agora() {
        janela.chamarGerente("backup_agora", r => {
            if (!r.ok) {
                janela.avisar(r.erro, "erro")
                return
            }
            estavaOcupado = true
            b = r.dados
            b.ocupado = true
        })
    }

    function conectar() {
        janela.chamarGerente("backup_conectar_drive", r => {
            if (!r.ok) {
                janela.avisar(r.erro, "erro")
                return
            }
            estavaOcupado = true
            conectando = true
            carregar()
        })
    }

    function desligar() {
        janela.confirmar("Parar de mandar para o Google Drive?",
                         "As cópias que já estão lá continuam lá. As novas ficam só neste computador.",
                         "Parar", () => janela.chamarGerente("backup_desligar_drive", {}, r => {
                             if (r.ok)
                                 b = r.dados
                         }), "perigo")
    }

    function buscarNuvem() {
        onde = "nuvem"
        buscandoNuvem = true
        Qt.callLater(() => {
            const r = Api.chamar("backup_nuvem_lista")
            buscandoNuvem = false
            if (r.ok)
                nuvem = r.dados
            else {
                nuvem = []
                janela.avisar(r.erro, "erro")
            }
        })
    }

    function caminhoDe(url) {
        return Api.caminho(url.toString())
    }

    // "caixa1" → "Caixa 1"
    function nomeCaixa(t) {
        const m = String(t).match(/^(.*?)(\d+)$/)
        const base = m ? m[1] + " " + m[2] : String(t)
        return base.charAt(0).toUpperCase() + base.slice(1)
    }

    function tamanho(bytes) {
        if (bytes < 1024 * 1024)
            return Math.max(1, Math.round(bytes / 1024)) + " KB"
        return (bytes / 1024 / 1024).toFixed(1).replace(".", ",") + " MB"
    }

    function quando(iso) {
        if (!iso)
            return ""
        const d = new Date(iso)
        const hoje = new Date()
        const ontem = new Date(); ontem.setDate(hoje.getDate() - 1)
        const hm = Qt.formatTime(d, "HH:mm")
        if (d.toDateString() === hoje.toDateString())
            return "hoje às " + hm
        if (d.toDateString() === ontem.toDateString())
            return "ontem às " + hm
        return Qt.formatDateTime(d, "dd/MM/yyyy") + " às " + hm
    }

    function perguntarRestaurar(c) {
        let msg = "Os dados do Rustinho vão voltar para como estavam em " + c.quando + ". "
                + "Antes, o Rustinho guarda uma cópia de agora (fica na lista como \"antes de restaurar\"), então dá para desfazer."
        if (b && b.sincronizacao)
            msg += "\n\nCom a sincronização ligada, este caixa recebe de novo dos outros tudo o que mudou depois dessa data."
        janela.confirmar("Restaurar o backup de " + c.quando + "?", msg, "Continuar", () => {
            // Restaurar sempre pede a senha de um gerente.
            secao.pedirSenhaDe = c
            senhaOutra.texto = ""
            senhaOutra.erro = ""
            senhaOutra.focar()
        }, "perigo")
    }

    function restaurar(c, senhaDigitada) {
        restaurando = true
        esperaRestaurar.copia = c
        esperaRestaurar.senhaUsada = senhaDigitada
        esperaRestaurar.start()
    }

    Timer {
        id: esperaRestaurar
        property var copia: null
        property string senhaUsada: ""
        interval: 120
        onTriggered: {
            const c = esperaRestaurar.copia
            janela.chamarGerente("backup_restaurar", { arquivo: c.nuvem ? c.nome : c.caminho, nuvem: !!c.nuvem,
                                                       senha: esperaRestaurar.senhaUsada }, r => {
                secao.restaurando = false
                if (r.ok) {
                    secao.pedirSenhaDe = null
                    janela.avisar("Backup de " + c.quando + " restaurado. Entre de novo.", "ok")
                    janela.atualizarEstado()
                    janela.trocarPara("TelaEntrar")
                    return
                }
                if (r.campo === "senha") {
                    secao.pedirSenhaDe = c
                    senhaOutra.erro = esperaRestaurar.senhaUsada !== "" ? r.erro : ""
                    senhaOutra.focar()
                    return
                }
                janela.avisar(r.erro, "erro")
                secao.carregar()
            })
        }
    }

    Timer {
        interval: 1500
        repeat: true
        running: !!secao.b && secao.b.ocupado && secao.visible
        onTriggered: secao.carregar()
    }

    FolderDialog {
        id: escolherPasta
        title: "Onde guardar os backups"
        onAccepted: pasta.texto = secao.caminhoDe(selectedFolder)
    }

    FileDialog {
        id: escolherArquivo
        title: "Escolha um backup do Rustinho (.rbk)"
        currentFolder: StandardPaths.standardLocations(StandardPaths.HomeLocation)[0]
        nameFilters: ["Backup do Rustinho (*.rbk)", "Todos os arquivos (*)"]
        onAccepted: {
            const caminho = secao.caminhoDe(selectedFile)
            const nome = caminho.substring(Math.max(caminho.lastIndexOf("/"), caminho.lastIndexOf("\\")) + 1)
            const m = nome.match(/^rustinho-(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})/)
            const q = m ? m[3] + "/" + m[2] + "/" + m[1] + " " + m[4] + ":" + m[5] : nome
            secao.perguntarRestaurar({ nome: nome, caminho: caminho, quando: q, nuvem: false })
        }
    }

    // ---------------- Como está ----------------
    Faixa {
        readonly property bool comErro: !!secao.b && secao.b.situacao.erro !== ""
        color: comErro ? Tema.rosa : Tema.verde
        Layout.fillWidth: true
        Linha {
            spacing: 18
            Layout.fillWidth: true
            ColumnLayout {
                spacing: 6
                Layout.fillWidth: true
                Text {
                    text: !secao.b ? "" : secao.b.situacao.ultimo
                          ? "Último backup " + secao.quando(secao.b.situacao.ultimo)
                          : "Ainda não teve nenhum backup"
                    color: Tema.tinta
                    font.family: Tema.titulo
                    font.weight: Font.Bold
                    font.pixelSize: Tema.estreito ? 24 : 28
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }
                Repeater {
                    model: !secao.b ? [] : [
                        { ok: secao.b.situacao.erro === "" && secao.b.situacao.ultimo !== "",
                          ruim: secao.b.situacao.erro !== "",
                          texto: secao.b.situacao.erro !== ""
                                 ? "No computador: " + secao.b.situacao.erro
                                 : secao.b.situacao.ultimo
                                   ? "No computador: conferido, " + secao.tamanho(secao.b.situacao.ultimoTamanho)
                                   : "No computador: o primeiro sai hoje, às " + secao.b.hora + " ou ao fechar o Rustinho" },
                        { ok: secao.b.drive && secao.b.situacao.nuvemErro === "" && secao.b.situacao.nuvemUltimo !== "",
                          ruim: secao.b.nuvem && secao.b.situacao.nuvemErro !== "",
                          texto: !secao.b.nuvem ? "No Google Drive: desligado"
                                 : secao.b.situacao.nuvemErro !== "" ? "No Google Drive: " + secao.b.situacao.nuvemErro
                                 : secao.b.situacao.nuvemUltimo ? "No Google Drive: enviado " + secao.quando(secao.b.situacao.nuvemUltimo)
                                 : "No Google Drive: esperando o primeiro envio" }
                    ]
                    delegate: RowLayout {
                        required property var modelData
                        spacing: 8
                        Layout.fillWidth: true
                        Rectangle {
                            width: 10; height: 10; radius: 5
                            color: modelData.ruim ? Tema.vermelho : modelData.ok ? Tema.verdeEscuro : Tema.apagado
                            Layout.alignment: Qt.AlignTop
                            Layout.topMargin: 6
                        }
                        Text {
                            text: modelData.texto
                            color: modelData.ruim ? Tema.vermelho : Tema.tinta
                            font.family: Tema.texto
                            font.pixelSize: 15
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                    }
                }
                Nota {
                    visible: !!secao.b && secao.b.ocupado
                    text: secao.b ? (secao.b.fazendo || "Trabalhando…") : ""
                    color: Tema.azulEscuro
                    Layout.fillWidth: true
                }
            }
            Botao {
                text: secao.b && secao.b.ocupado ? "Aguarde…" : "Fazer backup agora"
                icone: "check"
                enabled: !!secao.b && !secao.b.ocupado
                Layout.alignment: Qt.AlignVCenter
                onClicked: secao.agora()
            }
        }
    }

    // ---------------- Proteção ----------------
    Faixa {
        titulo: "Proteção"
        subtitulo: secao.b && secao.b.temSenha ? "trancado · abre com a senha de um gerente" : "ainda sem trancar"
        color: secao.b && !secao.b.temSenha ? Tema.rosa : Tema.pergaminho
        Layout.fillWidth: true
        Nota {
            text: secao.b && secao.b.temSenha
                  ? "Os backups saem trancados. Para restaurar (aqui ou num computador novo), é pedida a senha de um gerente: a de agora ou a de quando o backup foi feito."
                  : "Os backups ficam trancados assim que um gerente entrar no Rustinho. Até lá, ficam só neste computador."
            Layout.fillWidth: true
        }
        Text {
            text: "A proteção é a senha do gerente: com uma senha curta, quem copiar os dados descobre testando. Para proteger de verdade, o gerente deve usar 12 ou mais caracteres, de preferência uma frase. Sem nenhuma senha de gerente, o backup não abre — nem o Rustinho consegue."
            color: Tema.vermelho
            font.family: Tema.texto
            font.weight: Font.DemiBold
            font.pixelSize: 14
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }
    }

    // ---------------- Google Drive (celular: quem manda é o computador) ----------------
    Faixa {
        visible: Tema.celular
        titulo: "Google Drive"
        subtitulo: "fica com os computadores da loja"
        color: Tema.azul
        corTitulo: Tema.azulEscuro
        Layout.fillWidth: true
        Nota {
            text: "No celular, o backup fica guardado no próprio aparelho. Quem manda os backups para o Google Drive é um computador da loja: lá estão todos os dados, os do celular também."
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }
    }

    // ---------------- Google Drive ----------------
    Faixa {
        visible: !Tema.celular
        titulo: "Google Drive"
        subtitulo: !secao.b ? "" : secao.b.drive ? "conectado · pasta “Rustinho Backup”" : "cópia fora da loja, de graça"
        color: Tema.azul
        corTitulo: Tema.azulEscuro
        Layout.fillWidth: true

        // 1. Falta o rclone.
        ColumnLayout {
            visible: !!secao.b && !secao.b.rclone
            spacing: 10
            Layout.fillWidth: true
            Nota {
                text: Qt.platform.os === "windows"
                      ? "Falta o programa que leva os arquivos até o Drive. Ele vem com o instalador: instale o Rustinho de novo (os dados ficam)."
                      : "Falta um programa grátis que leva os arquivos até o Drive. Abra o terminal, cole a linha abaixo e digite a sua senha do computador:"
                Layout.fillWidth: true
            }
            RowLayout {
                visible: Qt.platform.os !== "windows"
                spacing: 10
                Rectangle {
                    color: Tema.papel
                    radius: Tema.raioCampo
                    implicitWidth: comando.implicitWidth + 28
                    implicitHeight: comando.implicitHeight + 16
                    TextEdit {
                        id: comando
                        anchors.centerIn: parent
                        text: "sudo pacman -S rclone"
                        readOnly: true
                        selectByMouse: true
                        color: Tema.tinta
                        font.family: "monospace"
                        font.pixelSize: 16
                    }
                }
                Botao {
                    text: "Copiar"
                    tipo: "secundario"
                    onClicked: {
                        comando.selectAll()
                        comando.copy()
                        comando.deselect()
                        secao.janela.avisar("Copiado.", "ok")
                    }
                }
                Botao {
                    text: "Já instalei"
                    tipo: "secundario"
                    onClicked: secao.carregar()
                }
            }
        }

        // 2. Instalado, falta conectar.
        ColumnLayout {
            visible: !!secao.b && secao.b.rclone && !secao.b.drive
            spacing: 10
            Layout.fillWidth: true
            Nota {
                text: "Clique em conectar: o navegador abre, você entra na sua conta do Google e clica em \"Permitir\". "
                      + "O Rustinho não vê a sua senha do Google e só enxerga os arquivos que ele mesmo mandar."
                Layout.fillWidth: true
            }
            RowLayout {
                spacing: 12
                Botao {
                    text: secao.b && secao.b.ocupado ? "Esperando o navegador…" : "Conectar ao Google Drive"
                    enabled: !!secao.b && secao.b.temSenha && !secao.b.ocupado
                    onClicked: secao.conectar()
                }
                Nota {
                    visible: !!secao.b && !secao.b.temSenha
                    text: "Um gerente precisa entrar no Rustinho antes (para os backups saírem trancados)."
                    color: Tema.vermelho
                    font.pixelSize: 14
                }
            }
        }

        // 3. Conectado.
        RowLayout {
            visible: !!secao.b && secao.b.drive
            spacing: 12
            Layout.fillWidth: true
            Nota {
                text: "Cada backup do dia também vai para o seu Drive, trancado (abre com a senha de um gerente). Lá ficam os mesmos " + (secao.b ? secao.b.dias : "") + " dias."
                Layout.fillWidth: true
            }
            Botao {
                text: "Parar de mandar"
                tipo: "secundario"
                onClicked: secao.desligar()
            }
        }
    }

    // ---------------- Onde e quando ----------------
    Faixa {
        titulo: "Onde e quando"
        subtitulo: Tema.celular ? "neste aparelho" : "neste computador"
        color: Tema.pergaminho
        Layout.fillWidth: true
        Linha {
            spacing: 12
            Layout.fillWidth: true
            Campo {
                id: pasta
                rotulo: "Pasta dos backups"
                maximo: 500
                Layout.fillWidth: true
            }
            Botao {
                text: "Escolher…"
                visible: !Tema.celular
                tipo: "secundario"
                Layout.alignment: Qt.AlignBottom
                onClicked: {
                    escolherPasta.currentFolder = Api.url(pasta.texto)
                    escolherPasta.open()
                }
            }
            Botao {
                text: "Abrir"
                visible: !Tema.celular
                tipo: "secundario"
                Layout.alignment: Qt.AlignBottom
                onClicked: Qt.openUrlExternally(Api.url(secao.b ? secao.b.pasta : ""))
            }
        }
        Linha {
            spacing: 12
            Layout.fillWidth: true
            Campo {
                id: hora
                rotulo: "Horário do backup"
                dica: "18:00"
                maximo: 5
                Layout.preferredWidth: 170
            }
            Campo {
                id: dias
                rotulo: "Guardar"
                sufixo: "dias"
                numerico: true
                maximo: 3
                Layout.preferredWidth: 140
            }
            Nota {
                text: "Também faz um backup ao fechar o Rustinho. Se o computador ficar desligado no horário, faz na próxima vez que abrir."
                font.pixelSize: 14
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignBottom
            }
            Botao {
                text: "Salvar"
                tipo: "secundario"
                icone: "check"
                Layout.alignment: Qt.AlignBottom
                onClicked: secao.salvar()
            }
        }
        Nota {
            visible: !!secao.b && pasta.texto !== secao.b.pastaPadrao
            text: "<a href='#'>Voltar para a pasta padrão</a>"
            textFormat: Text.StyledText
            linkColor: Tema.azulEscuro
            font.pixelSize: 14
            onLinkActivated: pasta.texto = secao.b.pastaPadrao
            MouseArea { anchors.fill: parent; acceptedButtons: Qt.NoButton; cursorShape: Qt.PointingHandCursor }
        }
    }

    // ---------------- Restaurar ----------------
    Faixa {
        titulo: "Restaurar"
        subtitulo: "voltar os dados para uma data"
        color: Tema.pergaminho
        Layout.fillWidth: true

        RowLayout {
            spacing: 12
            Layout.fillWidth: true
            Pilulas {
                modelo: [{ id: "computador", nome: "Neste computador" }, { id: "nuvem", nome: "No Google Drive" }]
                atual: secao.onde
                visible: !!secao.b && secao.b.drive
                onEscolhido: id => {
                    if (id === "nuvem")
                        secao.buscarNuvem()
                    else
                        secao.onde = "computador"
                }
            }
            Item { Layout.fillWidth: true }
            Botao {
                text: "Abrir outro arquivo…"
                tipo: "secundario"
                onClicked: escolherArquivo.open()
            }
        }

        // Senha diferente (backup antigo ou de outra loja).
        Rectangle {
            visible: secao.pedirSenhaDe !== null
            color: Tema.rosa
            radius: Tema.raio
            Layout.fillWidth: true
            implicitHeight: linhaSenha.implicitHeight + 28
            Linha {
                id: linhaSenha
                x: 16; y: 14
                width: parent.width - 32
                spacing: 12
                Campo {
                    id: senhaOutra
                    rotulo: "Senha de um gerente (backup de antes da 0.9.11: a senha do backup)"
                    senha: true
                    maximo: 256
                    Layout.fillWidth: true
                    onConfirmado: secao.restaurar(secao.pedirSenhaDe, senhaOutra.texto)
                }
                Botao {
                    text: "Restaurar"
                    tipo: "perigo"
                    Layout.alignment: Qt.AlignBottom
                    onClicked: secao.restaurar(secao.pedirSenhaDe, senhaOutra.texto)
                }
                Botao {
                    text: "Cancelar"
                    tipo: "secundario"
                    Layout.alignment: Qt.AlignBottom
                    onClicked: { secao.pedirSenhaDe = null; senhaOutra.texto = "" }
                }
            }
        }

        Nota {
            visible: secao.restaurando
            text: "Restaurando… pode levar alguns segundos."
            color: Tema.azulEscuro
            Layout.fillWidth: true
        }
        Nota {
            readonly property var lista: secao.onde === "nuvem" ? secao.nuvem : (secao.b ? secao.b.lista : [])
            visible: !secao.buscandoNuvem && lista.length === 0
            text: secao.onde === "nuvem" ? "Nenhum backup no Google Drive ainda." : "Nenhum backup nesta pasta ainda."
            Layout.fillWidth: true
        }
        Nota {
            visible: secao.buscandoNuvem
            text: "Buscando no Google Drive…"
            Layout.fillWidth: true
        }

        Repeater {
            model: {
                const lista = secao.onde === "nuvem" ? secao.nuvem : (secao.b ? secao.b.lista : [])
                return secao.todas ? lista : lista.slice(0, 6)
            }
            delegate: Rectangle {
                required property var modelData
                required property int index
                color: index % 2 === 0 ? Tema.papel : "transparent"
                radius: Tema.raioCampo
                Layout.fillWidth: true
                implicitHeight: 48
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 16
                    anchors.rightMargin: 8
                    spacing: 12
                    Text {
                        text: modelData.quando
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.weight: Font.DemiBold
                        font.pixelSize: 16
                        Layout.preferredWidth: 150
                    }
                    Chip {
                        texto: modelData.antes ? "antes de restaurar" : ""
                        color: Tema.rosa
                    }
                    Text {
                        text: secao.nomeCaixa(modelData.caixa) + " · " + secao.tamanho(modelData.tamanho) + (modelData.temSenha ? " · trancado" : " · sem trancar")
                        color: Tema.tintaSuave
                        font.family: Tema.texto
                        font.pixelSize: 15
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    Botao {
                        text: "Restaurar"
                        tipo: "secundario"
                        enabled: !secao.restaurando && !!secao.b && !secao.b.ocupado
                        onClicked: secao.perguntarRestaurar(modelData)
                    }
                }
            }
        }
        Nota {
            readonly property var lista: secao.onde === "nuvem" ? secao.nuvem : (secao.b ? secao.b.lista : [])
            visible: !secao.todas && lista.length > 6
            text: "<a href='#'>Mostrar todos (" + lista.length + ")</a>"
            textFormat: Text.StyledText
            linkColor: Tema.azulEscuro
            font.pixelSize: 14
            onLinkActivated: secao.todas = true
            MouseArea { anchors.fill: parent; acceptedButtons: Qt.NoButton; cursorShape: Qt.PointingHandCursor }
        }
    }

    Component.onCompleted: carregar()
}
