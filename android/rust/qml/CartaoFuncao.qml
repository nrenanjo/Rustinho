import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Pílula de uma função na tela inicial (0.9.15): retângulo horizontal de
// cantos arredondados, ícone num quadrado de fundo pastel à esquerda e o
// nome à direita. O alfinete fica no canto: nas fixadas sempre (para
// soltar); nas outras só ao passar o mouse e enquanto houver vaga (até 4).
Rectangle {
    id: cartao
    property var item: ({})
    property int indice: 0
    // Colunas da grade (1 a 4): no Planeta Roxo os cartões ficam em xadrez,
    // claro e escuro por (linha + coluna) % 2, o primeiro escuro (V-7).
    property int colunas: 1
    readonly property int linhaNaGrade: Math.floor(indice / Math.max(1, colunas))
    readonly property int colunaNaGrade: indice % Math.max(1, colunas)
    readonly property bool escuraNoXadrez: (linhaNaGrade + colunaNaGrade) % 2 === 0
    // No cinza e no AMOLED, a cor de sempre (nada de xadrez).
    readonly property color base: Tema.roxo ? Tema.faixa(escuraNoXadrez) : Tema.cartao

    signal abrir()
    signal fixar()

    // As funções que têm ladrilho próprio (os ids do menu).
    readonly property var ladrilhos: ["vender", "pecas", "orcamentos", "caixa", "trocas", "notas", "clientes", "compras",
                                      "estoque", "financeiro", "oficina", "painel", "usuarios", "configuracoes"]

    readonly property bool mostraPino: cartao.item.fixo === true || (cartao.item.podeFixar !== false && sobre.hovered && !Tema.celular)

    radius: Tema.raio
    opacity: item.disponivel ? 1 : 0.78
    color: sobre.hovered && item.disponivel ? Qt.darker(base, Tema.escuro || Tema.escura(base) ? 0.85 : 1.03) : base
    palette.window: Tema.superficie(base)
    // Ajuste visual de 15/11/2026: sem o contorno; só o foco do teclado continua desenhado (2 px).
    border.width: foco.activeFocus ? 2 : 0
    border.color: Tema.azulEscuro
    implicitHeight: Tema.estreito ? 64 : 72

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Tema.estreito ? 10 : 12
        anchors.rightMargin: 34
        spacing: Tema.estreito ? 12 : 16

        // Ajuste visual de 15/11/2026 (referência purpp.png do dono): os 14 ladrilhos roxos em PNG
        // (icone/inicio-<id>.png), na proporção da referência (100 × 82). No computador, 40 de altura (49 de
        // largura): a pílula de 1280 px continua com "Estoque / Fornecedores" numa linha só (antes, o ícone ocupava
        // 48; com 49 sobram 1,5 px). No estreito e no celular, 42 (51 de largura), com espaço de sobra.
        // Função sem ladrilho próprio: o ícone desenhado de antes.
        Image {
            id: ladrilho
            readonly property bool tem: cartao.ladrilhos.indexOf(cartao.item.id) >= 0
            readonly property real altura: Tema.estreito ? 42 : 40
            visible: tem
            source: tem ? "qrc:/qt/qml/rustinho/icone/inicio-" + cartao.item.id + ".png" : ""
            fillMode: Image.PreserveAspectFit
            smooth: true
            mipmap: true
            Layout.preferredHeight: altura
            // Todos os ladrilhos têm 100 × 82 px: a mesma largura em todas as pílulas.
            Layout.preferredWidth: Math.round(altura * 100 / 82)
            Layout.alignment: Qt.AlignVCenter
        }
        IconeFuncao {
            visible: !ladrilho.tem
            funcao: cartao.item.id || ""
            nome: cartao.item.icone || "caixa"
            Layout.alignment: Qt.AlignVCenter
        }

        ColumnLayout {
            spacing: 0
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
            RowLayout {
                spacing: 8
                Layout.fillWidth: true
                Text {
                    text: cartao.item.titulo || ""
                    textFormat: Text.PlainText
                    color: cartao.item.disponivel ? Tema.tintaNo(cartao.base) : Tema.tintaSuaveNo(cartao.base)
                    font.family: Tema.texto
                    font.weight: Font.DemiBold
                    font.pixelSize: Tema.estreito ? 16 : 18
                    wrapMode: Text.WordWrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                // Selo numérico fora da pílula (diretriz do dono, C.1 reprovada;
                // passo 7.1): o aviso textual das pendências fica na Início.
            }
            Text {
                visible: !cartao.item.disponivel
                text: "em breve"
                color: Tema.apagadoNo(cartao.base)
                font.family: Tema.texto
                font.italic: true
                font.pixelSize: 13
            }
        }
    }

    Botao {
        id: pino
        tipo: "discreto"
        icone: "pino"
        tamanhoIcone: 15
        altura: 28
        largura: 30
        leftPadding: 0
        rightPadding: 0
        anchors.top: parent.top
        anchors.right: parent.right
        anchors.margins: 4
        visible: cartao.mostraPino
        opacity: cartao.item.fixo ? 1 : 0.55
        ToolTip.visible: hovered && !Tema.celular
        ToolTip.delay: 500
        ToolTip.text: cartao.item.fixo ? "Soltar" : "Fixar no começo da tela"
        onClicked: cartao.fixar()
    }

    Item {
        id: foco
        anchors.fill: parent
        activeFocusOnTab: true
        Keys.onReturnPressed: cartao.abrir()
        Keys.onEnterPressed: cartao.abrir()
        Keys.onSpacePressed: cartao.abrir()
    }

    HoverHandler {
        id: sobre
        cursorShape: cartao.item.disponivel ? Qt.PointingHandCursor : Qt.ArrowCursor
    }
    TapHandler {
        onTapped: cartao.abrir()
        // No celular: segurar o dedo fixa ou solta (se houver vaga).
        onLongPressed: if (cartao.item.fixo || cartao.item.podeFixar !== false) cartao.fixar()
    }
}
