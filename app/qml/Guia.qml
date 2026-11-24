import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Templates as T
import rustinho

// Navegação guiada (0.9.16; movimento, recorte S-3 e persistência NCG-2 na 0.9.19-dev.1):
// - Destaque: véu suave e luz difusa recortados à área rolável (S-3 / MG-11),
//   sem vazar sobre cabeçalho, pílulas ou barra superior. Só entra por esmaecimento
//   após a rolagem concluir (MG-3 / MG-14); o foco é aplicado só depois (MG-15).
// - Zero Timer de 60 ms (H25): acompanha por sinais (Connections) e FrameAnimation
//   ligado apenas enquanto há movimento ou transição ativa (0% CPU em repouso).
// - Destaque preso ao registro (MG-5): durante recarga de lista, congela o retângulo
//   e desliza suavemente até o novo item (ou esmaece para fora se o item sumiu).
// - Orientação persistente (NCG-2): usa o padrão visual fixo de Notificacao.qml
//   (P-NOTIF) e não some sozinha enquanto persistente === true.
Item {
    id: guia
    anchors.fill: parent
    z: 1400

    readonly property var janela: ApplicationWindow.window
    // Espaço que a orientação ocupa no alto (o Main desce as telas por ele).
    readonly property real alturaNoAlto: temAviso && notificacao.item ? notificacao.item.height + 2 * espaco : 0
    // Onde começa (abaixo da faixa do modo seguro, se houver).
    property real topo: 0
    readonly property real espaco: 12

    property Item alvo: null
    property Item areaRolagem: null
    // Retângulo do alvo nesta camada (sempre em sincronia com o quadro).
    property rect area: Qt.rect(0, 0, 0, 0)
    // Retângulo de recorte S-3 (área rolável ou área abaixo da orientação).
    property rect recorte: Qt.rect(0, 0, width, height)
    property real opacidadeDestaque: 0
    property bool aguardandoChegada: false
    property bool recarregandoLista: false
    property bool deslizandoRegistro: false
    property rect areaDe: Qt.rect(0, 0, 0, 0)
    property rect areaPara: Qt.rect(0, 0, 0, 0)
    property real deslizeInicioMs: 0
    property int _tokenDestaque: 0

    readonly property bool destacando: opacidadeDestaque > 0.005 && area.width > 0
    readonly property real folga: 8
    readonly property real alcanceLuz: 58

    property string estado: ""      // "" | "pendente" | "erro" | "satisfeito"
    property string titulo: ""
    property string texto: ""
    property bool persistente: false
    readonly property bool temAviso: estado !== ""

    readonly property bool precisaAcompanhar:
        deslizandoRegistro
        || (alvo !== null && !recarregandoLista && (
            (janela && (janela.movimentoAtivo || janela.descidaAnimando))
            || (areaRolagem && (areaRolagem.moving || areaRolagem.dragging || areaRolagem.flicking))
        ))

    Behavior on opacidadeDestaque {
        NumberAnimation {
            duration: Tema.duracaoEsmaecer(180)
            easing.type: Easing.OutCubic
        }
    }
    onOpacidadeDestaqueChanged: {
        if (opacidadeDestaque <= 0.005 && alvo === null && !recarregandoLista)
            area = Qt.rect(0, 0, 0, 0)
    }

    function _logMov(evento) {
        if (!janela || !janela.medirMovimento)
            return
        let alvoY = -1
        let alvoH = 0
        if (alvo && alvo.visible) {
            const p = alvo.mapToItem(guia, 0, 0)
            alvoY = Math.round(p.y * 10) / 10
            alvoH = Math.round(alvo.height * 10) / 10
        }
        console.info("MOV:" + JSON.stringify({
            guia: evento,
            destaqueY: Math.round(area.y * 10) / 10,
            destaqueH: Math.round(area.height * 10) / 10,
            alvoY: alvoY,
            alvoH: alvoH,
            opacidade: Math.round(opacidadeDestaque * 100) / 100,
            clipY: Math.round(recorte.y * 10) / 10,
            clipH: Math.round(recorte.height * 10) / 10
        }))
    }

    function destacar(item, rolagem, leve, aoChegar) {
        const token = ++_tokenDestaque
        deslizandoRegistro = false
        recarregandoLista = false
        veuLeve = leve === true
        alvo = item || null
        areaRolagem = rolagem || null
        if (!item) {
            semDestaque()
            return
        }
        const finalizarChegada = (interrompido) => {
            if (_tokenDestaque !== token || alvo !== item)
                return
            aguardandoChegada = false
            medir()
            opacidadeDestaque = 1
            _logMov(interrompido ? "chegada_interrompida" : "destaque_ativo")
            if (typeof aoChegar === "function" && !interrompido)
                aoChegar()
        }
        if (rolagem && janela && typeof janela.rolarAteItem === "function") {
            // MG-3 / A6: destaque só entra depois de a rolagem concluir.
            aguardandoChegada = true
            opacidadeDestaque = 0
            medirRecorte()
            _logMov("aguardando_rolagem")
            janela.rolarAteItem(rolagem, item, 24, finalizarChegada)
        } else {
            aguardandoChegada = false
            medir()
            opacidadeDestaque = 1
            _logMov("destaque_direto")
            if (typeof aoChegar === "function")
                Qt.callLater(() => { if (_tokenDestaque === token && alvo === item) aoChegar() })
        }
    }

    function semDestaque(imediato) {
        _tokenDestaque++
        deslizandoRegistro = false
        recarregandoLista = false
        aguardandoChegada = false
        veuLeve = false
        alvo = null
        areaRolagem = null
        opacidadeDestaque = 0
        if (imediato === true || Tema.movimentoReduzido)
            area = Qt.rect(0, 0, 0, 0)
    }

    // MG-5 / A7: recarga da lista mantém o destaque atual congelado sem sumir.
    function iniciarRecargaLista() {
        if (!alvo && area.width <= 0)
            return
        recarregandoLista = true
        deslizandoRegistro = false
    }

    // MG-5 / A7: após recarga, desliza suavemente até o novo item do mesmo registro (ou esmaece se sumiu).
    function atualizarItemDoRegistro(novoItem) {
        recarregandoLista = false
        if (!novoItem || !novoItem.visible || novoItem.width <= 0) {
            semDestaque()
            return
        }
        const antes = area
        alvo = novoItem
        const p = novoItem.mapToItem(guia, 0, 0)
        const novoRect = Qt.rect(p.x - folga, p.y - folga, novoItem.width + 2 * folga, novoItem.height + 2 * folga)
        medirRecorte()
        if (antes.width > 0 && !Tema.movimentoReduzido
                && (Math.abs(novoRect.y - antes.y) > 1 || Math.abs(novoRect.height - antes.height) > 1)) {
            areaDe = antes
            areaPara = novoRect
            deslizeInicioMs = Date.now()
            deslizandoRegistro = true
            opacidadeDestaque = 1
            _logMov("registro_deslizar")
        } else {
            area = novoRect
            opacidadeDestaque = 1
            _logMov("registro_atualizado")
        }
    }

    function medirRecorte() {
        if (areaRolagem && areaRolagem.visible && areaRolagem.width > 0 && areaRolagem.height > 0) {
            const rc = areaRolagem.mapToItem(guia, 0, 0)
            const rx = Math.max(0, rc.x)
            const ry = Math.max(0, rc.y)
            const rw = Math.max(0, Math.min(guia.width - rx, areaRolagem.width - Math.max(0, -rc.x)))
            const rh = Math.max(0, Math.min(guia.height - ry, areaRolagem.height - Math.max(0, -rc.y)))
            recorte = Qt.rect(rx, ry, rw, rh)
        } else {
            const topY = Math.max(0, guia.topo + guia.alturaNoAlto)
            recorte = Qt.rect(0, topY, guia.width, Math.max(0, guia.height - topY))
        }
    }

    function medir() {
        medirRecorte()
        if (recarregandoLista || deslizandoRegistro)
            return
        if (!alvo || !alvo.visible || alvo.width <= 0) {
            if (!aguardandoChegada)
                area = Qt.rect(0, 0, 0, 0)
            return
        }
        const p = alvo.mapToItem(guia, 0, 0)
        area = Qt.rect(p.x - folga, p.y - folga, alvo.width + 2 * folga, alvo.height + 2 * folga)
    }

    function passoAcompanhamento() {
        if (deslizandoRegistro) {
            const dur = Math.max(1, Tema.duracaoPosicao(180))
            const t = Math.min(1, (Date.now() - deslizeInicioMs) / dur)
            const e = 1 - Math.pow(1 - t, 3)
            area = Qt.rect(
                areaDe.x + (areaPara.x - areaDe.x) * e,
                areaDe.y + (areaPara.y - areaDe.y) * e,
                areaDe.width + (areaPara.width - areaDe.width) * e,
                areaDe.height + (areaPara.height - areaDe.height) * e
            )
            medirRecorte()
            if (t >= 1) {
                deslizandoRegistro = false
                medir()
            }
            return
        }
        medir()
        if (opacidadeDestaque > 0.01)
            _logMov("acompanhar")
    }

    onAreaChanged: luz.requestPaint()
    onWidthChanged: if (alvo) medir()
    onHeightChanged: if (alvo) medir()
    onTopoChanged: if (alvo) medir()
    onAlturaNoAltoChanged: if (alvo) Qt.callLater(medir)

    // H25: zero polling fixo; apenas conexões de eventos + FrameAnimation sob demanda.
    Connections {
        target: guia.alvo
        function onXChanged() { guia.medir() }
        function onYChanged() { guia.medir() }
        function onWidthChanged() { guia.medir() }
        function onHeightChanged() { guia.medir() }
        function onVisibleChanged() { guia.medir() }
    }
    Connections {
        target: guia.areaRolagem
        function onContentYChanged() { if (!guia.aguardandoChegada) guia.medir() }
        function onContentXChanged() { if (!guia.aguardandoChegada) guia.medir() }
        function onWidthChanged() { guia.medir() }
        function onHeightChanged() { guia.medir() }
        function onYChanged() { guia.medir() }
        function onXChanged() { guia.medir() }
    }
    FrameAnimation {
        running: guia.precisaAcompanhar
        onTriggered: guia.passoAcompanhamento()
    }

    // transitoria: some sozinha depois de dar tempo de ler ("satisfeito" sem
    // persistente sempre some; "pendente" transitoria some após voltar).
    // manterFixa (NCG-2): notificação persistente — NÃO some sozinha por tempo.
    function mostrar(novoEstado, novoTitulo, novoTexto, transitoria, manterFixa) {
        estado = novoEstado
        titulo = novoTitulo
        texto = novoTexto
        persistente = manterFixa === true
        tempo.stop()
        if (!persistente && (novoEstado === "satisfeito" || transitoria === true))
            tempo.restart()
    }
    function esconder() {
        tempo.stop()
        persistente = false
        estado = ""
    }
    Timer {
        id: tempo
        interval: 9000
        onTriggered: if (!guia.persistente) guia.esconder()
    }

    // ---- Destaque: véu suave e luz difusa recortados à área rolável (S-3) ----
    property bool veuLeve: false
    readonly property real forcaVeu: veuLeve ? 0.5 : 1
    readonly property color corVeu: Tema.roxo ? Qt.rgba(0.09, 0, 0.09, 0.5 * forcaVeu)
                                    : Tema.escuro ? Qt.rgba(0, 0, 0, 0.55 * forcaVeu)
                                    : Qt.rgba(0.16, 0.15, 0.13, 0.30 * forcaVeu)
    Item {
        id: camadaDestaque
        objectName: "camadaDestaqueGuia"
        x: guia.recorte.x
        y: guia.recorte.y
        width: guia.recorte.width
        height: guia.recorte.height
        clip: true
        opacity: guia.opacidadeDestaque
        visible: guia.destacando

        readonly property real lx: guia.area.x - x
        readonly property real ly: guia.area.y - y
        readonly property real lw: guia.area.width
        readonly property real lh: guia.area.height

        Rectangle {   // em cima
            x: 0; y: 0; width: camadaDestaque.width; height: Math.max(0, camadaDestaque.ly)
            color: guia.corVeu
        }
        Rectangle {   // embaixo
            x: 0; y: camadaDestaque.ly + camadaDestaque.lh
            width: camadaDestaque.width; height: Math.max(0, camadaDestaque.height - y)
            color: guia.corVeu
        }
        Rectangle {   // à esquerda
            x: 0; y: Math.max(0, camadaDestaque.ly)
            width: Math.max(0, camadaDestaque.lx)
            height: Math.max(0, Math.min(camadaDestaque.height, camadaDestaque.ly + camadaDestaque.lh) - y)
            color: guia.corVeu
        }
        Rectangle {   // à direita
            x: camadaDestaque.lx + camadaDestaque.lw
            y: Math.max(0, camadaDestaque.ly)
            width: Math.max(0, camadaDestaque.width - x)
            height: Math.max(0, Math.min(camadaDestaque.height, camadaDestaque.ly + camadaDestaque.lh) - y)
            color: guia.corVeu
        }
        Canvas {
            id: luz
            x: camadaDestaque.lx - guia.alcanceLuz
            y: camadaDestaque.ly - guia.alcanceLuz
            width: camadaDestaque.lw + 2 * guia.alcanceLuz
            height: camadaDestaque.lh + 2 * guia.alcanceLuz

            onPaint: {
                const ctx = getContext("2d")
                ctx.clearRect(0, 0, width, height)
                if (camadaDestaque.lw <= 0 || camadaDestaque.lh <= 0)
                    return

                const x = guia.alcanceLuz
                const y = guia.alcanceLuz
                const w = camadaDestaque.lw
                const h = camadaDestaque.lh
                const r = Math.min(Tema.raioCampo + guia.folga, Math.min(w, h) / 2)

                ctx.save()
                ctx.shadowColor = Tema.roxo ? "rgba(255, 214, 246, 0.92)"
                                : Tema.escuro ? "rgba(232, 194, 126, 0.62)" : "rgba(255, 248, 230, 0.96)"
                ctx.shadowBlur = 42
                ctx.shadowOffsetX = 0
                ctx.shadowOffsetY = 0
                ctx.fillStyle = Tema.roxo ? "rgba(255, 230, 250, 0.14)"
                              : Tema.escuro ? "rgba(232, 194, 126, 0.06)" : "rgba(255, 249, 237, 0.16)"
                ctx.beginPath()
                ctx.moveTo(x + r, y)
                ctx.lineTo(x + w - r, y)
                ctx.quadraticCurveTo(x + w, y, x + w, y + r)
                ctx.lineTo(x + w, y + h - r)
                ctx.quadraticCurveTo(x + w, y + h, x + w - r, y + h)
                ctx.lineTo(x + r, y + h)
                ctx.quadraticCurveTo(x, y + h, x, y + h - r)
                ctx.lineTo(x, y + r)
                ctx.quadraticCurveTo(x, y, x + r, y)
                ctx.closePath()
                ctx.fill()
                ctx.restore()
            }
        }
    }
    Connections {
        target: Tema
        function onTemaChanged() { luz.requestPaint() }
    }

    // ---- Orientação: a notificação do Início (padrão fixo P-NOTIF), no alto ----
    Item {
        parent: T.Overlay.overlay
        anchors.fill: parent
        z: 1000
        visible: guia.temAviso
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
                    minimizado: false,
                    persistente: guia.persistente
                })
                onEsconder: if (!guia.persistente) guia.esconder()
            }
        }
    }
}
