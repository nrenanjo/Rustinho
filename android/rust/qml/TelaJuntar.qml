import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Primeira vez num caixa novo: em vez de começar do zero, entra na loja e
// recebe tudo de outro caixa (usuários, peças, clientes, estoque...).
// 0.9.19 (protocolo 4, A6/D1): a entrada é pelo convite que o gerente gera num
// caixa da loja, conferindo o código deste caixa; a senha da rede não basta.
FocusScope {
    id: tela
    // Como esta tela aparece na orientação da navegação guiada ("Depois o
    // Rustinho volta para …").
    readonly property string nomeNaNavegacao: "Juntar este caixa"
    readonly property var janela: ApplicationWindow.window
    property var sync: null
    property bool procurando: false
    // O código deste caixa (o gerente confere ao gerar o convite). Vem do
    // núcleo pelo caminho demorado: abrir a identidade custa (DT-P4-30).
    property string codigo: ""
    property bool cofreSistema: true
    property string erroDoCodigo: ""

    // FALHA-01 (V01a): voltar desfaz a tentativa inteira (troca de dados
    // desligada, endereços da rede, o que já tinha chegado da loja).
    function desistir() {
        tela.procurando = false
        const r = Api.chamar("sync_desistir_juntar")
        if (!r.ok) {
            tela.janela.avisar(r.erro, "erro")
            return
        }
        tela.janela.atualizarEstado()
        tela.janela.trocarPara("TelaBoasVindas")
    }
    // A seta e o Esc fazem o mesmo: desistem e voltam às boas-vindas (o
    // botão "Voltar" saiu na correção definitiva do botão de retorno,
    // 07/11/2026). H24 (DP-28, diretriz §7: confirmar o irreversível): se já
    // chegou algum dado da loja, os dois perguntam antes (desistir apaga daqui
    // o que chegou); sem dado chegado, desistem direto, como antes. O estado é
    // lido na hora (pode ter chegado depois da última leitura); sem conseguir
    // ler, pergunta.
    function aoEsc(daSeta) {
        const r = Api.chamar("sync_ler")
        if (r.ok && !r.dados.chegouDado) {
            desistir()
            return
        }
        tela.janela.confirmar("Desistir de juntar este caixa?",
                              "Parte dos dados da loja já chegou a este " + Tema.aparelho
                              + ". Se desistir, o que chegou é apagado daqui; no outro caixa nada muda.",
                              "Desistir", () => tela.desistir(), "perigo")
    }

    // O código deste caixa (cria a identidade dele na primeira vez).
    function pedirCodigo() {
        tela.erroDoCodigo = ""
        Api.chamarDemorado("p4_identidade", {}, "Preparando o código deste " + Tema.aparelho + "…", r => {
            if (r.ok) {
                tela.codigo = r.dados.codigoPedido
                tela.cofreSistema = r.dados.cofreSistema
                convite.focarSempre()
            } else {
                tela.erroDoCodigo = r.erro
            }
        })
    }

    // Aberta de novo no meio de uma tentativa (o programa fechou): continua
    // esperando os dados da loja. Senão, prepara o código deste caixa.
    Component.onCompleted: {
        if (janela && janela.estado && janela.estado.juntando)
            procurando = true
        else
            Qt.callLater(tela.pedirCodigo)
    }

    function entrar() {
        convite.erro = ""
        endereco.erro = ""
        if (convite.texto.trim() === "") {
            convite.erro = "Digite o convite que aparece na tela do gerente."
            convite.focarSempre()
            return
        }
        Api.chamarDemorado("p4_juntar_com_convite", { convite: convite.texto, enderecos: endereco.texto },
                           "Entrando na loja…", r => {
            if (r.ok) {
                tela.sync = r.dados
                tela.procurando = true
                tela.janela.atualizarEstado()
            } else {
                const alvo = r.campo === "enderecos" ? endereco : convite
                alvo.erro = r.erro
                alvo.focarSempre()
            }
        })
    }

    Timer {
        interval: 2000
        repeat: true
        running: tela.procurando
        onTriggered: {
            const r = Api.chamar("sync_ler")
            if (!r.ok)
                return
            tela.sync = r.dados
            if (r.dados.usuarios > 0) {
                tela.procurando = false
                tela.janela.atualizarEstado()
                tela.janela.avisar("Pronto: os dados da loja chegaram.", "ok")
                tela.janela.trocarPara("TelaEntrar")
            }
        }
    }

    Flickable {
        anchors.fill: parent
        contentHeight: Math.max(height, coluna.implicitHeight + 120)
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}

        ColumnLayout {
            id: coluna
            width: Math.min(620, parent.width - 2 * Tema.margem)
            x: (parent.width - width) / 2
            // Abaixo da seta de retorno, mesmo numa tela baixa (no celular,
            // sem a seta, só a margem).
            // (S1-R5 da revisão Sonnet: pela regra, e não pela visibilidade efetiva da seta.)
            y: Math.max(Tema.margem + (Tema.celular ? 0 : Tema.toqueSeta + 8), (tela.height - implicitHeight) / 2 - 20)
            spacing: 18

            ColumnLayout {
                spacing: 0
                Layout.fillWidth: true
                Titulo {
                    text: "Juntar este caixa"
                    font.pixelSize: Tema.estreito ? 42 : 56
                    Layout.fillWidth: true
                }
                Nota {
                    text: "Este " + Tema.aparelho + " vai receber tudo da loja: usuários, peças, clientes e estoque. "
                          + "O gerente aprova a entrada num caixa da loja que já tem o Rustinho 0.9.19 ou mais novo. "
                          + "Os dois precisam estar na mesma rede (Wi-Fi ou cabo), com o Rustinho aberto."
                    font.pixelSize: 17
                    Layout.fillWidth: true
                }
            }

            Faixa {
                visible: !tela.procurando
                titulo: "1. Mostre este código ao gerente"
                color: Tema.pergaminho
                Layout.fillWidth: true
                Campo {
                    id: campoCodigo
                    objectName: "codigoDesteCaixa"
                    visible: tela.codigo !== ""
                    rotulo: "Código deste " + Tema.aparelho
                    texto: tela.codigo
                    somenteLeitura: true
                    tamanhoFonte: 26
                    altura: 52
                    Layout.fillWidth: true
                }
                Nota {
                    visible: tela.codigo !== ""
                    text: "No caixa da loja, o gerente abre Configurações › Caixas da loja › Aprovar caixa novo, "
                          + "digita este código e um nome para este " + Tema.aparelho + ". Lá aparece o convite."
                    Layout.fillWidth: true
                }
                Nota {
                    visible: tela.codigo !== "" && !tela.cofreSistema
                    text: "Este computador não tem o cofre de senhas do sistema. A identidade deste caixa fica "
                          + "protegida dentro da pasta do Rustinho: não copie essa pasta para outro computador."
                    font.pixelSize: 13
                    Layout.fillWidth: true
                }
                Nota {
                    visible: tela.erroDoCodigo !== ""
                    text: "Não deu para preparar o código deste " + Tema.aparelho + ": " + tela.erroDoCodigo
                    color: Tema.vermelho
                    Layout.fillWidth: true
                }
                Botao {
                    visible: tela.erroDoCodigo !== ""
                    text: "Tentar de novo"
                    tipo: "secundario"
                    onClicked: tela.pedirCodigo()
                }
            }

            Faixa {
                visible: !tela.procurando
                titulo: "2. Digite o convite"
                color: Tema.pergaminho
                Layout.fillWidth: true
                Campo {
                    id: convite
                    objectName: "conviteDaLoja"
                    rotulo: "Convite (aparece na tela do gerente; vale 10 minutos)"
                    dica: "Ex.: ABCDE-FGHJK-LMNPQ-RSTUV-WXYZ2-3"
                    podeColar: true
                    // No celular, o convite também vem pelo QR da tela do gerente.
                    camera: true
                    maximo: 400
                    Layout.fillWidth: true
                    onConfirmado: tela.entrar()
                }
                // O botão na linha do endereço: à vista mesmo numa tela baixa.
                Linha {
                    spacing: 12
                    Layout.fillWidth: true
                    Campo {
                        id: endereco
                        rotulo: "Endereço do caixa da loja (só se não achar sozinho)"
                        dica: "Aparece junto do convite. Ex.: 192.168.0.12"
                        maximo: 300
                        Layout.fillWidth: true
                        onConfirmado: tela.entrar()
                    }
                    Botao {
                        text: "Entrar na loja"
                        Layout.alignment: Qt.AlignBottom
                        onClicked: tela.entrar()
                    }
                }
            }

            Faixa {
                visible: tela.procurando
                titulo: "Recebendo os dados da loja"
                color: Tema.azul
                Layout.fillWidth: true
                Nota {
                    text: !tela.sync || tela.sync.pares.length === 0
                          ? "A entrada foi aprovada. Esperando o caixa da loja mandar os dados: deixe os dois com o Rustinho aberto. Endereço deste " + Tema.aparelho + ": " + (tela.sync ? tela.sync.meuEndereco : "")
                          : "Recebendo os dados…"
                    Layout.fillWidth: true
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
                        implicitHeight: conteudoDaFaixa.implicitHeight + 8
                        implicitWidth: conteudoDaFaixa.implicitWidth + 16
                        Text {
                            id: conteudoDaFaixa
                            x: 8
                            y: 4
                            width: faixaDaLista.width - 16
                            text: (modelData.nome || "Caixa") + " (" + modelData.endereco + ")" + (modelData.erro ? " · " + modelData.erro : "")
                            textFormat: Text.PlainText
                            color: modelData.erro ? Tema.vermelhoDaFaixa(faixaDaLista.par) : Tema.tintaDaFaixa(faixaDaLista.par)
                            font.family: Tema.texto
                            font.pixelSize: 15
                            wrapMode: Text.WordWrap
                        }
                    }
                }
            }
        }
    }

    // A seta de retorno no canto de cima, à esquerda, no mesmo lugar das
    // telas com Cabecalho (Tema.margem; Seta.descida no computador). Fica por
    // cima da rolagem, sempre à vista.
    Seta {
        id: seta
        // No celular (Android), sem a seta (pedido do proprietário,
        // 18/11/2026): o "voltar" do Android faz o mesmo.
        visible: !Tema.celular
        x: Tema.margem
        y: Tema.margem - 12 + seta.descida
        z: 1
        onClicked: tela.janela.retornar(true)
    }

    StackView.onActivated: if (!tela.procurando && tela.codigo !== "") convite.focarSempre()
}
