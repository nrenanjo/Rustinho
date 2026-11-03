import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Templates as T
import rustinho

// Navegação guiada (0.9.16): a parte que se vê.
// - Destaque: o resto da tela fica mais escuro e o campo que falta ganha um
//   contorno. É só desenho por cima: não muda o campo e não segura clique
//   nem teclado (dá para digitar e salvar normalmente).
// - Orientação: a mesma notificação do Início (Notificacao), no alto da
//   janela, com as margens do Início. Ela ocupa o próprio espaço (o Main
//   desce as telas pela altura dela, como faz com a faixa do modo seguro):
//   não cobre nada. Fica na camada das janelinhas para continuar legível com
//   uma delas aberta (a importação, a senha do gerente).
// Estados: "pendente" (atenção), "erro" (crítica) e "satisfeito" (informativa,
// some sozinha depois de dar tempo de ler). Quem decide quando mostrar é o
// Main (janela.guiar / concluirGuia).
Item {
    id: guia
    anchors.fill: parent
    z: 1400

    // Espaço que a orientação ocupa no alto (o Main desce as telas por ele).
    readonly property real alturaNoAlto: temAviso && notificacao.item ? notificacao.item.height + 2 * espaco : 0
    // Onde começa (abaixo da faixa do modo seguro, se houver).
    property real topo: 0
    readonly property real espaco: 12

    property Item alvo: null
    // Retângulo do alvo nesta camada (atualizado enquanto ele está à vista:
    // a tela pode rolar ou mudar de tamanho).
    property rect area: Qt.rect(0, 0, 0, 0)
    readonly property bool destacando: alvo !== null && area.width > 0
    readonly property real folga: 8

    property string estado: ""      // "" | "pendente" | "erro" | "satisfeito"
    property string titulo: ""
    property string texto: ""
    readonly property bool temAviso: estado !== ""

    function destacar(item, rolagem) {
        alvo = item
        if (item && rolagem)
            mostrarNaRolagem(item, rolagem)
        medir()
    }
    function semDestaque() {
        alvo = null
        area = Qt.rect(0, 0, 0, 0)
    }
    // Rola só se o campo não estiver todo à vista, e o deixa perto do topo.
    function mostrarNaRolagem(item, rolagem) {
        const p = item.mapToItem(rolagem.contentItem, 0, 0)
        const topoVisivel = rolagem.contentY
        if (p.y >= topoVisivel && p.y + item.height <= topoVisivel + rolagem.height)
            return
        const maximo = Math.max(0, rolagem.contentHeight - rolagem.height)
        rolagem.contentY = Math.max(0, Math.min(p.y - 24, maximo))
    }
    function medir() {
        if (!alvo || !alvo.visible || alvo.width <= 0) {
            area = Qt.rect(0, 0, 0, 0)
            return
        }
        const p = alvo.mapToItem(guia, 0, 0)
        area = Qt.rect(p.x - folga, p.y - folga, alvo.width + 2 * folga, alvo.height + 2 * folga)
    }
    Timer {
        interval: 60
        repeat: true
        running: guia.alvo !== null
        onTriggered: guia.medir()
    }

    // transitoria: some sozinha depois de dar tempo de ler ("satisfeito"
    // sempre some; "pendente" some quando a pessoa já voltou para agir).
    function mostrar(novoEstado, novoTitulo, novoTexto, transitoria) {
        estado = novoEstado
        titulo = novoTitulo
        texto = novoTexto
        tempo.stop()
        if (novoEstado === "satisfeito" || transitoria)
            tempo.restart()
    }
    function esconder() {
        tempo.stop()
        estado = ""
    }
    Timer {
        id: tempo
        interval: 9000
        onTriggered: guia.esconder()
    }

    // ---- Destaque (quatro faixas escuras em volta do campo) ----
    readonly property color corVeu: Tema.escuro ? Qt.rgba(0, 0, 0, 0.55) : Qt.rgba(0.16, 0.15, 0.13, 0.30)
    Rectangle {   // em cima
        visible: guia.destacando
        x: 0; y: 0; width: guia.width; height: Math.max(0, guia.area.y)
        color: guia.corVeu
    }
    Rectangle {   // embaixo
        visible: guia.destacando
        x: 0; y: guia.area.y + guia.area.height
        width: guia.width; height: Math.max(0, guia.height - y)
        color: guia.corVeu
    }
    Rectangle {   // à esquerda
        visible: guia.destacando
        x: 0; y: guia.area.y; width: Math.max(0, guia.area.x); height: guia.area.height
        color: guia.corVeu
    }
    Rectangle {   // à direita
        visible: guia.destacando
        x: guia.area.x + guia.area.width; y: guia.area.y
        width: Math.max(0, guia.width - x); height: guia.area.height
        color: guia.corVeu
    }
    Rectangle {   // contorno do campo
        visible: guia.destacando
        x: guia.area.x; y: guia.area.y; width: guia.area.width; height: guia.area.height
        radius: Tema.raioCampo + guia.folga
        color: "transparent"
        border.width: 2
        border.color: Tema.azulEscuro
    }

    // ---- Orientação: a notificação do Início, no alto ----
    Item {
        parent: T.Overlay.overlay
        anchors.fill: parent
        z: 1000
        visible: guia.temAviso
        // Criada de novo a cada vez que aparece (arrastada para fora, ela
        // não fica escondida para a próxima).
        Loader {
            id: notificacao
            active: guia.temAviso
            x: Tema.margem
            y: guia.topo + guia.espaco
            width: parent.width - 2 * Tema.margem
            sourceComponent: Notificacao {
                aviso: ({
                    id: "guia",
                    categoria: guia.estado === "erro" ? "critica" : guia.estado === "pendente" ? "atencao" : "informativa",
                    titulo: guia.titulo,
                    tituloLongo: guia.titulo,
                    descricao: guia.texto,
                    resumo: guia.texto !== "" ? guia.titulo + " · " + guia.texto : guia.titulo,
                    texto: guia.texto,
                    // A orientação é passageira: fica sempre aberta.
                    minimizado: false
                })
                onEsconder: guia.esconder()
            }
        }
    }
}
