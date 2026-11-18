import QtQuick
import QtQuick.Layouts
import rustinho

// Faixa de cor com cantos arredondados, como as combinações do livro.
// O que for colocado dentro dela fica empilhado (ColumnLayout).
Rectangle {
    id: faixa
    property string titulo: ""
    property string subtitulo: ""
    property color corTitulo: Tema.roxo ? Tema.apagadoNo(color) : Tema.apagado
    property int preenchimento: Tema.estreito ? 16 : 22
    property alias espacamento: coluna.spacing
    default property alias conteudo: coluna.data

    // VIS-C8-01 (dev3.3): no estreito, o título e o subtítulo só dividem a
    // linha quando cabem inteiros nela; senão o subtítulo vai para baixo do
    // título, e cada um fica com a largura toda. Antes, os dois disputavam a
    // linha: o título em maiúsculas espaçadas ficava com meia largura, e uma
    // palavra mais larga que essa metade era desenhada por cima do subtítulo
    // ("DIFERENÇAS ENTRE CAIXAS" × "1 para ver" no celular). No computador,
    // igual a antes: uma linha, o subtítulo cortado com "…" se não couber.
    readonly property bool cabeNaLinha: !Tema.estreito || subtitulo === ""
                                        || medidaTitulo.advanceWidth + cabecalho.columnSpacing
                                           + medidaSubtitulo.advanceWidth + 2 <= coluna.width

    color: Tema.pergaminho
    // O texto de dentro sabe a cor embaixo dele (no roxo, a faixa é clara).
    palette.window: Tema.superficie(color)
    radius: Tema.raio
    implicitHeight: coluna.implicitHeight + preenchimento * 2
    implicitWidth: coluna.implicitWidth + preenchimento * 2

    // A largura natural (numa linha só) de cada texto, com a fonte dele.
    TextMetrics { id: medidaTitulo; font: rotuloTitulo.font; text: faixa.titulo }
    TextMetrics { id: medidaSubtitulo; font: notaSubtitulo.font; text: faixa.subtitulo }

    ColumnLayout {
        id: coluna
        x: faixa.preenchimento
        y: faixa.preenchimento
        width: faixa.width - faixa.preenchimento * 2
        spacing: 14

        GridLayout {
            id: cabecalho
            // T042: por cima da barra de pesquisa logo abaixo: o halo dela passa atrás, não por cima.
            z: 1
            visible: faixa.titulo !== ""
            columns: faixa.cabeNaLinha ? 2 : 1
            columnSpacing: 12
            rowSpacing: 4
            Layout.fillWidth: true
            Rotulo {
                id: rotuloTitulo
                text: faixa.titulo
                color: faixa.corTitulo
                // H21 (7.2): no celular, o título comprido quebra a linha em
                // vez de alargar a faixa além da tela (no computador, igual a
                // antes). VIS-C8-01: quebra também dentro de uma palavra que,
                // sozinha, não cabe na largura (Text.Wrap), em vez de passar
                // dela por cima de outro texto.
                wrapMode: Tema.estreito ? Text.Wrap : Text.NoWrap
                // Na mesma linha, a largura natural; sozinho, a largura toda.
                Layout.fillWidth: Tema.estreito && !faixa.cabeNaLinha
                Layout.maximumWidth: coluna.width
            }
            Nota {
                id: notaSubtitulo
                visible: faixa.subtitulo !== ""
                text: faixa.subtitulo
                font.pixelSize: 14
                wrapMode: Tema.estreito ? Text.WordWrap : Text.NoWrap
                elide: Tema.estreito ? Text.ElideNone : Text.ElideRight
                Layout.fillWidth: true
            }
        }
    }
}
