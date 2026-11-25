import QtQuick
import QtQuick.Layouts
import rustinho

// Pergunta de confirmação (ex.: "Descartar alterações?"). Como em
// comoficará.png (V-4, 0.9.18): a seta no canto (a do Painel) desiste, igual
// ao Esc; o botão faz a ação. Não há mais o "Cancelar" que só fechava.
Painel {
    id: dialogo
    property string titulo: ""
    property string mensagem: ""
    property string textoConfirmar: "Confirmar"
    property string tipoConfirmar: "principal"
    property var aoConfirmar: null
    // R8 (D-AUD-8, decisão do proprietário de 25/11/2026): nas perguntas de perigo (e onde a tela pedir), o botão
    // de confirmar não abre com o foco, então o Enter (às vezes o do leitor de código de barras) não confirma. O Tab
    // chega até ele, e o clique continua valendo.
    property bool enterConfirma: true

    width: Math.min(480, Tema.largura - 24)

    contentItem: ColumnLayout {
        spacing: 16
        Titulo {
            text: dialogo.titulo
            font.pixelSize: 30
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }
        Text {
            text: dialogo.mensagem
            textFormat: Text.PlainText
            visible: text !== ""
            color: Tema.tinta
            font.family: Tema.serifa
            font.pixelSize: 16
            wrapMode: Text.WordWrap
            lineHeight: 1.15
            Layout.fillWidth: true
        }
        RowLayout {
            spacing: 10
            Layout.alignment: Qt.AlignRight
            Layout.topMargin: 6
            Botao {
                id: confirmar
                text: dialogo.textoConfirmar
                tipo: dialogo.tipoConfirmar
                onClicked: {
                    const funcao = dialogo.aoConfirmar
                    dialogo.close()
                    if (funcao)
                        funcao()
                }
            }
        }
    }

    onOpened: {
        if (dialogo.enterConfirma)
            confirmar.forceActiveFocus()
        else
            dialogo.contentItem.forceActiveFocus()
    }

    function perguntar(titulo, mensagem, textoBotao, funcao, tipo, enterConfirma) {
        dialogo.titulo = titulo
        dialogo.mensagem = mensagem
        dialogo.textoConfirmar = textoBotao
        dialogo.aoConfirmar = funcao
        dialogo.tipoConfirmar = tipo || "principal"
        dialogo.enterConfirma = enterConfirma !== undefined ? !!enterConfirma : dialogo.tipoConfirmar !== "perigo"
        dialogo.open()
    }
}
