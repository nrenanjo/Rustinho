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

    onOpened: confirmar.forceActiveFocus()

    function perguntar(titulo, mensagem, textoBotao, funcao, tipo) {
        dialogo.titulo = titulo
        dialogo.mensagem = mensagem
        dialogo.textoConfirmar = textoBotao
        dialogo.aoConfirmar = funcao
        dialogo.tipoConfirmar = tipo || "principal"
        dialogo.open()
    }
}
