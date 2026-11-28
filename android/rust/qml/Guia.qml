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
    property real opacidadeAlvo: 0
    property real opacidadeInicio: 0
    property real opacidadeInicioMs: 0
    property bool animandoOpacidade: false
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

    function _animarOpacidadePara(destino, imediato) {
        opacidadeAlvo = destino
        if (imediato === true || Tema.duracaoEsmaecer(180) <= 0) {
            animandoOpacidade = false
            opacidadeDestaque = destino
            if (destino <= 0.005 && alvo === null && !recarregandoLista)
                area = Qt.rect(0, 0, 0, 0)
            return
        }
        opacidadeInicio = opacidadeDestaque
        opacidadeInicioMs = Date.now()
        animandoOpacidade = true
    }

    Timer {
        id: pulsoTransicaoGuia
        interval: 16
        repeat: true
        running: guia.animandoOpacidade || guia.deslizandoRegistro
        onTriggered: {
            if (guia.animandoOpacidade) {
                const dur = Math.max(1, Tema.duracaoEsmaecer(180))
                const t = Math.min(1, (Date.now() - guia.opacidadeInicioMs) / dur)
                const e = 1 - Math.pow(1 - t, 3)
                guia.opacidadeDestaque = guia.opacidadeInicio + (guia.opacidadeAlvo - guia.opacidadeInicio) * e
                if (t >= 1) {
                    guia.opacidadeDestaque = guia.opacidadeAlvo
                    guia.animandoOpacidade = false
                    if (guia.opacidadeDestaque <= 0.005 && guia.alvo === null && !guia.recarregandoLista)
                        guia.area = Qt.rect(0, 0, 0, 0)
                }
            }
            if (guia.deslizandoRegistro)
                guia.passoAcompanhamento()
        }
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
            _animarOpacidadePara(1, false)
            _logMov(interrompido ? "chegada_interrompida" : "destaque_ativo")
            if (typeof aoChegar === "function" && !interrompido)
                aoChegar()
        }
        let precisaRolar = false
        if (rolagem && rolagem.contentItem && rolagem.height > 0 && janela && typeof janela.rolarAteItem === "function") {
            const p = item.mapToItem(rolagem.contentItem, 0, 0)
            const topoV = rolagem.contentY
            const baseV = topoV + rolagem.height
            precisaRolar = !(p.y >= topoV + 16 && p.y + item.height <= baseV - 16)
        }
        if (precisaRolar) {
            // MG-3 / A6: destaque só entra depois de a rolagem concluir.
            aguardandoChegada = true
            _animarOpacidadePara(0, true)
            medirRecorte()
            _logMov("aguardando_rolagem")
            janela.rolarAteItem(rolagem, item, 24, finalizarChegada)
        } else {
            aguardandoChegada = false
            medir()
            _animarOpacidadePara(1, false)
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
        _animarOpacidadePara(0, imediato === true || Tema.movimentoReduzido)
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
            _animarOpacidadePara(1, false)
            _logMov("registro_deslizar")
        } else {
            area = novoRect
            _animarOpacidadePara(1, false)
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

    // ---- Destaque: véu na janela toda fora da janela visível do alvo + luz recortada à área rolável (S-3) ----
    // O recorte S-3 (MG-11 / A10 / s8) garante que nem a abertura do véu nem a luz difusa
    // invadam cabeçalho ou pílulas fixas quando a página rola, enquanto o véu continua
    // escurecendo de forma uniforme toda a área externa (incluindo margens laterais e topo).
    property bool veuLeve: false
    readonly property real forcaVeu: veuLeve ? 0.5 : 1
    readonly property color corVeu: Tema.roxo ? Qt.rgba(0.09, 0, 0.09, 0.5 * forcaVeu)
                                    : Tema.escuro ? Qt.rgba(0, 0, 0, 0.55 * forcaVeu)
                                    : Qt.rgba(0.16, 0.15, 0.13, 0.30 * forcaVeu)
    readonly property real vx1: Math.max(guia.area.x, guia.recorte.x)
    readonly property real vy1: Math.max(guia.area.y, guia.recorte.y)
    readonly property real vx2: Math.min(guia.area.x + guia.area.width, guia.recorte.x + guia.recorte.width)
    readonly property real vy2: Math.min(guia.area.y + guia.area.height, guia.recorte.y + guia.recorte.height)
    readonly property bool janelaVisivel: vx2 > vx1 && vy2 > vy1

    Item {
        id: camadaVeu
        anchors.fill: parent
        opacity: guia.opacidadeDestaque
        visible: guia.destacando

        Rectangle {   // quando o alvo rolou inteiramente para fora da área rolável, véu contínuo sem buraco nas pílulas
            visible: !guia.janelaVisivel
            anchors.fill: parent
            color: guia.corVeu
        }
        Rectangle {   // em cima
            visible: guia.janelaVisivel
            x: 0; y: 0; width: parent.width; height: Math.max(0, guia.vy1)
            color: guia.corVeu
        }
        Rectangle {   // embaixo
            visible: guia.janelaVisivel
            x: 0; y: guia.vy2
            width: parent.width; height: Math.max(0, parent.height - y)
            color: guia.corVeu
        }
        Rectangle {   // à esquerda
            visible: guia.janelaVisivel
            x: 0; y: guia.vy1
            width: Math.max(0, guia.vx1)
            height: Math.max(0, guia.vy2 - guia.vy1)
            color: guia.corVeu
        }
        Rectangle {   // à direita
            visible: guia.janelaVisivel
            x: guia.vx2; y: guia.vy1
            width: Math.max(0, parent.width - x)
            height: Math.max(0, guia.vy2 - guia.vy1)
            color: guia.corVeu
        }
    }

    Item {
        id: camadaDestaque
        objectName: "camadaDestaqueGuia"
        x: guia.recorte.x
        y: guia.recorte.y
        width: guia.recorte.width
        height: guia.recorte.height
        clip: true
        opacity: guia.opacidadeDestaque
        visible: guia.destacando && guia.janelaVisivel

        readonly property real lx: guia.area.x - x
        readonly property real ly: guia.area.y - y
        readonly property real lw: guia.area.width
        readonly property real lh: guia.area.height

        // T040: a luz é feita de anéis (Rectangle), e não de um Canvas. O Canvas com sombra de 42 px era repintado no
        // processador a cada mudança de lugar da área (a parada de ~1 s na rolagem guiada, T039); os anéis só mudam de
        // lugar, sem pintura nova. Cada anel tem a opacidade da sombra naquela distância (desfoque 42 ≈ 21 px de
        // espalhamento); o anel de dentro (a folga) leva a sombra e o tom claro, e o interior do campo fica limpo.
        Item {
            id: luz
            objectName: "luzGuia"
            x: camadaDestaque.lx
            y: camadaDestaque.ly
            width: camadaDestaque.lw
            height: camadaDestaque.lh
            visible: width > 0 && height > 0

            readonly property real raio: Math.min(Tema.raioCampo + guia.folga, Math.min(width, height) / 2)
            readonly property color corSombra: Tema.roxo ? Qt.rgba(1, 214 / 255, 246 / 255, 0.92)
                                               : Tema.escuro ? Qt.rgba(232 / 255, 194 / 255, 126 / 255, 0.62)
                                               : Qt.rgba(1, 248 / 255, 230 / 255, 0.96)
            readonly property color corTom: Tema.roxo ? Qt.rgba(1, 230 / 255, 250 / 255, 0.14)
                                            : Tema.escuro ? Qt.rgba(232 / 255, 194 / 255, 126 / 255, 0.06)
                                            : Qt.rgba(1, 249 / 255, 237 / 255, 0.16)
            readonly property real passo: 3
            readonly property int aneis: Math.ceil(guia.alcanceLuz / passo)

            // A força da sombra a uma distância d da borda (d < 0 é dentro): metade de erfc(d / (σ√2)), σ = 21.
            function forca(d) {
                const z = d / (21 * Math.SQRT2)
                const t = 1 / (1 + 0.3275911 * Math.abs(z))
                const e = t * (0.254829592 + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027
                          + t * 1.061405429)))) * Math.exp(-z * z)
                return 0.5 * (z >= 0 ? e : 2 - e)
            }
            function comAlfa(c, a) { return Qt.rgba(c.r, c.g, c.b, c.a * a) }

            Repeater {
                model: luz.aneis
                Rectangle {
                    required property int index
                    readonly property real fora: (index + 1) * luz.passo
                    x: -fora
                    y: -fora
                    width: luz.width + 2 * fora
                    height: luz.height + 2 * fora
                    radius: luz.raio + fora
                    color: "transparent"
                    border.width: luz.passo
                    border.color: luz.comAlfa(luz.corSombra, luz.forca(fora - luz.passo / 2))
                    antialiasing: false
                }
            }
            Rectangle {   // a folga: a sombra por baixo e o tom claro por cima
                anchors.fill: parent
                radius: luz.raio
                color: "transparent"
                border.width: Math.min(guia.folga, Math.min(luz.width, luz.height) / 2)
                border.color: luz.comAlfa(luz.corSombra, luz.forca(-guia.folga / 2))
                antialiasing: false
                Rectangle {
                    anchors.fill: parent
                    radius: parent.radius
                    color: "transparent"
                    border.width: parent.border.width
                    border.color: luz.corTom
                    antialiasing: false
                }
            }
        }
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
