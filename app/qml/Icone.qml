import QtQuick
import QtQuick.Shapes
import rustinho

// Ícones simples, desenhados com linhas numa grade de 24 x 24.
Item {
    id: icone
    property string nome: "busca"
    property color cor: Tema.tinta
    property real tamanho: 20
    property real espessura: 1.9

    implicitWidth: tamanho
    implicitHeight: tamanho

    readonly property var desenhos: ({
        "busca": "M 17 10.5 A 6.5 6.5 0 1 1 4 10.5 A 6.5 6.5 0 1 1 17 10.5 M 15.4 15.4 L 20 20",
        "voltar": "M 19 12 L 5 12 M 11 6 L 5 12 L 11 18",
        "mais": "M 12 5 L 12 19 M 5 12 L 19 12",
        "fechar": "M 6.5 6.5 L 17.5 17.5 M 17.5 6.5 L 6.5 17.5",
        "lixeira": "M 4.5 7 L 19.5 7 M 9.5 7 L 9.5 4.5 L 14.5 4.5 L 14.5 7 M 6.5 7 L 7.5 19.5 L 16.5 19.5 L 17.5 7 M 10.5 10.5 L 10.5 16 M 13.5 10.5 L 13.5 16",
        "check": "M 5 12.5 L 10 17.5 L 19 7",
        "baixo": "M 6 9.5 L 12 15.5 L 18 9.5",
        "cima": "M 6 14.5 L 12 8.5 L 18 14.5",
        "direita": "M 9.5 6 L 15.5 12 L 9.5 18",
        "pino": "M 12 15.5 L 12 21 M 7.5 3.5 L 16.5 3.5 M 9 3.5 L 9 9 L 6 13 L 6 15.5 L 18 15.5 L 18 13 L 15 9 L 15 3.5",
        "sair": "M 13.5 5 L 13.5 4 L 4.5 4 L 4.5 20 L 13.5 20 L 13.5 19 M 9.5 12 L 20.5 12 M 16.5 8 L 20.5 12 L 16.5 16",
        "caixa": "M 4 8 L 12 4 L 20 8 L 20 16 L 12 20 L 4 16 Z M 4 8 L 12 12 L 20 8 M 12 12 L 12 20",
        "pessoa": "M 16 8 A 4 4 0 1 1 8 8 A 4 4 0 1 1 16 8 M 4.5 20 C 5.5 15 18.5 15 19.5 20",
        "lapis": "M 4.5 19.5 L 5 16 L 16 5 L 19 8 L 8 19 Z M 14 7 L 17 10",
        "ajuste": "M 4 8 L 20 8 M 4 16 L 20 16 M 9 5.5 L 9 10.5 M 15 13.5 L 15 18.5",
        "relogio": "M 20 12 A 8 8 0 1 1 4 12 A 8 8 0 1 1 20 12 M 12 7.5 L 12 12 L 15 14",
        "mensagem": "M 4.5 5.5 L 19.5 5.5 L 19.5 16 L 11 16 L 7 19.5 L 7 16 L 4.5 16 Z",
        "olho": "M 2.5 12 C 6 6 18 6 21.5 12 C 18 18 6 18 2.5 12 Z M 15 12 A 3 3 0 1 1 9 12 A 3 3 0 1 1 15 12",
        "olhoFechado": "M 3 10 C 7 15.5 17 15.5 21 10 M 6.4 13.4 L 4.8 15.6 M 12 15 L 12 17.8 M 17.6 13.4 L 19.2 15.6",
        "impressora": "M 7.5 9 L 7.5 4.5 L 16.5 4.5 L 16.5 9 M 7.5 16.5 L 4.5 16.5 L 4.5 9 L 19.5 9 L 19.5 16.5 L 16.5 16.5 M 7.5 13 L 16.5 13 L 16.5 20 L 7.5 20 Z"
    })

    Shape {
        anchors.fill: parent
        layer.enabled: true
        layer.samples: 4
        layer.smooth: true

        ShapePath {
            scale: Qt.size(icone.tamanho / 24, icone.tamanho / 24)
            strokeColor: icone.cor
            strokeWidth: icone.espessura
            fillColor: "transparent"
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin
            PathSvg { path: icone.desenhos[icone.nome] || "" }
        }
    }
}
