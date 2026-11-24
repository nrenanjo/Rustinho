import QtQuick
import QtQuick.Controls.Basic
import rustinho

// Motor único de movimento e rolagem suave (0.9.19-dev.1, P6: MG-1..MG-8, MG-D2, MG-D4).
// - Rolagem guiada e automática usa 250–450 ms (InOutCubic; OutCubic se já em movimento).
// - Relógio de passos sob demanda (~60 Hz, desligado em repouso = 0% CPU), independente de
//   eventos externos ou vsync de renderização por software (Xvfb/llvmpipe).
// - Grande distância (> 3 alturas da área): esmaece (95 ms) → salta para perto →
//   entra suavemente (95 ms) com rolagem curta final (260 ms, OutCubic).
// - Interrupção manual (arrasto, flick ou roda do mouse / tecla): para no mesmo
//   quadro (<= 1 quadro), sem puxar de volta, restaurando opacidade = 1.
// - Movimento reduzido (`Tema.movimentoReduzido`): posiciona direto em 0 ms.
Item {
    id: mov
    width: 0
    height: 0

    readonly property var janela: ApplicationWindow.window
    readonly property bool ativo: estado !== ""

    property string estado: ""     // "" | "normal" | "fadeOut" | "fadeIn" | "listaFadeOut" | "listaFadeIn"
    property var area: null
    property real inicioY: 0
    property real destinoY: 0
    property real saltoY: 0
    property real ultimoYEscrito: -1
    property real ultimaAlturaArea: -1
    property real ultimoTickMs: 0
    property real decorridoMs: 0
    property real faseDecorridoMs: 0
    property bool primeiroQuadro: true
    property real duracaoMs: 300
    property bool jaEmMovimento: false
    property var aoTerminar: null
    property var aoAtualizar: null
    property var listaAlvo: null
    property int indiceAlvo: -1
    property int modoLista: ListView.Contain
    property int _geracao: 0

    function _limites(a) {
        const minY = a.originY !== undefined ? a.originY : 0
        const maxY = minY + Math.max(0, a.contentHeight - a.height)
        return [minY, maxY]
    }
    function _limitar(v, minY, maxY) {
        return Math.max(minY, Math.min(v, maxY))
    }
    function _log(fase, y, extra) {
        if (!janela || !janela.medirMovimento)
            return
        const dados = {
            t: Math.round(decorridoMs),
            fase: fase,
            y: Math.round(y * 10) / 10,
            destino: Math.round(destinoY * 10) / 10,
            opacidadeArea: area ? Math.round(area.opacity * 100) / 100 : 1,
            reduzido: Tema.movimentoReduzido
        }
        if (extra) {
            for (const k in extra)
                dados[k] = extra[k]
        }
        console.info("MOV:" + JSON.stringify(dados))
    }
    function _limpar() {
        estado = ""
        area = null
        listaAlvo = null
        indiceAlvo = -1
        ultimoYEscrito = -1
        ultimaAlturaArea = -1
        ultimoTickMs = 0
        decorridoMs = 0
        faseDecorridoMs = 0
        primeiroQuadro = true
        aoTerminar = null
        aoAtualizar = null
    }
    function _concluir(interrompido) {
        const cb = aoTerminar
        const a = area
        if (a)
            a.opacity = 1
        _log(interrompido ? "interrompido" : "fim", a ? a.contentY : destinoY, { interrompido: !!interrompido })
        _limpar()
        if (cb)
            cb(!!interrompido)
    }

    function cancelar(silencioso) {
        _geracao++
        if (!ativo)
            return
        if (silencioso) {
            if (area)
                area.opacity = 1
            _limpar()
        } else {
            _concluir(true)
        }
    }

    function interromperPorUsuario() {
        if (!ativo)
            return
        _geracao++
        _concluir(true)
    }

    function rolarSuave(alvoArea, novoDestinoY, opcoes) {
        const opts = opcoes || {}
        const cbFim = typeof opts.aoTerminar === "function" ? opts.aoTerminar : null
        const cbPasso = typeof opts.aoAtualizar === "function" ? opts.aoAtualizar : null
        const mesmaArea = ativo && area === alvoArea
        cancelar(true)
        if (!alvoArea || alvoArea.height <= 0) {
            if (cbFim)
                Qt.callLater(() => cbFim(false))
            return
        }
        const [minY, maxY] = _limites(alvoArea)
        const destino = _limitar(novoDestinoY, minY, maxY)
        const inicio = alvoArea.contentY
        const dist = Math.abs(destino - inicio)
        const gen = ++_geracao

        ultimoTickMs = Date.now()
        decorridoMs = 0
        faseDecorridoMs = 0
        primeiroQuadro = true
        destinoY = destino
        if (Tema.movimentoReduzido || dist < 1) {
            area = alvoArea
            alvoArea.contentY = destino
            alvoArea.opacity = 1
            _log(Tema.movimentoReduzido ? "reduzido" : "no_lugar", destino, { distancia: Math.round(dist) })
            _limpar()
            if (cbPasso)
                cbPasso()
            if (cbFim)
                Qt.callLater(() => { if (_geracao === gen) cbFim(false) })
            return
        }

        area = alvoArea
        inicioY = inicio
        ultimoYEscrito = inicio
        ultimaAlturaArea = alvoArea.height
        jaEmMovimento = mesmaArea
        aoTerminar = cbFim
        aoAtualizar = cbPasso

        const alturaRef = Math.max(200, alvoArea.height)
        if (dist > 3 * alturaRef) {
            // MG-7 / MG-D2 / A3: grande distância -> esmaece, salta para perto e desliza suavemente.
            const passoFinal = Math.min(240, alturaRef * 0.45)
            saltoY = _limitar(destino > inicio ? destino - passoFinal : destino + passoFinal, minY, maxY)
            duracaoMs = 260
            estado = "fadeOut"
            _log("inicio_longa", inicio, { distancia: Math.round(dist), salto: Math.round(saltoY) })
        } else {
            // MG-1 / A2: 250..450 ms proporcional à distância.
            duracaoMs = Math.round(250 + 200 * Math.min(1, dist / (2 * alturaRef)))
            estado = "normal"
            _log("inicio", inicio, { distancia: Math.round(dist), duracao: duracaoMs })
        }
    }

    function rolarAteItem(alvoArea, item, folga, cbFim, cbPasso) {
        if (!alvoArea || !item || !alvoArea.contentItem) {
            if (typeof cbFim === "function")
                Qt.callLater(() => cbFim(false))
            return
        }
        const p = item.mapToItem(alvoArea.contentItem, 0, 0)
        const margem = folga !== undefined && folga !== null ? folga : 24
        const topoVisivel = alvoArea.contentY
        const baseVisivel = topoVisivel + alvoArea.height
        const folgaVisivel = Math.min(16, margem)
        if (p.y >= topoVisivel + folgaVisivel && p.y + item.height <= baseVisivel - folgaVisivel) {
            const gen = ++_geracao
            decorridoMs = 0
            destinoY = topoVisivel
            area = alvoArea
            _log("ja_visivel", topoVisivel)
            _limpar()
            if (typeof cbFim === "function")
                Qt.callLater(() => { if (_geracao === gen) cbFim(false) })
            return
        }
        const dest = item.height > alvoArea.height - 48 ? p.y - 16 : p.y - margem
        rolarSuave(alvoArea, dest, { aoTerminar: cbFim, aoAtualizar: cbPasso })
    }

    function rolarAteIndice(lista, indice, modo, cbFim) {
        const m = modo !== undefined ? modo : ListView.Contain
        if (!lista || indice < 0 || indice >= lista.count) {
            if (typeof cbFim === "function")
                Qt.callLater(() => cbFim(false))
            return
        }
        const item = typeof lista.itemAtIndex === "function" ? lista.itemAtIndex(indice) : null
        if (item) {
            rolarAteItem(lista, item, 24, cbFim)
            return
        }
        cancelar(true)
        const gen = ++_geracao
        ultimoTickMs = Date.now()
        decorridoMs = 0
        faseDecorridoMs = 0
        primeiroQuadro = true
        if (Tema.movimentoReduzido) {
            area = lista
            lista.positionViewAtIndex(indice, m)
            destinoY = lista.contentY
            _log("lista_reduzido", lista.contentY, { indice: indice })
            _limpar()
            if (typeof cbFim === "function")
                Qt.callLater(() => { if (_geracao === gen) cbFim(false) })
            return
        }
        area = lista
        listaAlvo = lista
        indiceAlvo = indice
        modoLista = m
        ultimoYEscrito = lista.contentY
        ultimaAlturaArea = lista.height
        aoTerminar = typeof cbFim === "function" ? cbFim : null
        estado = "listaFadeOut"
        _log("lista_inicio", lista.contentY, { indice: indice })
    }

    function _passo() {
        const a = mov.area
        if (!a) {
            mov._limpar()
            return
        }
        const mesmaAltura = mov.ultimaAlturaArea < 0 || Math.abs(a.height - mov.ultimaAlturaArea) < 1
        // MG-6 / A8: detecta qualquer rolagem manual (roda do mouse, barra, PageUp/Down) no mesmo quadro.
        if (a.dragging || a.flicking
                || (mesmaAltura && mov.ultimoYEscrito >= 0 && Math.abs(a.contentY - mov.ultimoYEscrito) > 2.5)) {
            mov.interromperPorUsuario()
            return
        }
        mov.ultimaAlturaArea = a.height
        const agora = Date.now()
        const dtReal = mov.ultimoTickMs > 0 ? agora - mov.ultimoTickMs : 16
        mov.ultimoTickMs = agora
        // MG-9: acompanha o tempo real de relógio para nunca virar câmera lenta em hardware
        // mais modesto ou renderização por software (Xvfb/llvmpipe). No 1º quadro dá um passo
        // inicial de 16 ms; nos seguintes avança pelo tempo real decorrido. Apenas o fadeOut
        // inicial de grande distância limita o passo a 75 ms para garantir ao menos 1 quadro
        // esmaecido antes do salto (A3).
        const dt = mov.primeiroQuadro ? 16 : Math.max(12, dtReal)
        const dtFase = (mov.estado === "fadeOut" || mov.estado === "listaFadeOut")
                       ? (mov.primeiroQuadro ? 16 : Math.min(75, Math.max(12, dtReal)))
                       : dt
        mov.primeiroQuadro = false
        mov.decorridoMs += dt
        mov.faseDecorridoMs += dtFase
        const [minY, maxY] = mov._limites(a)

        if (mov.estado === "normal") {
            const t = Math.min(1, mov.decorridoMs / Math.max(1, mov.duracaoMs))
            const e = mov.jaEmMovimento
                      ? (1 - Math.pow(1 - t, 3))
                      : (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2)
            const y = mov._limitar(mov.inicioY + (mov.destinoY - mov.inicioY) * e, minY, maxY)
            a.contentY = y
            mov.ultimoYEscrito = y
            mov._log("rolar", y)
            if (mov.aoAtualizar)
                mov.aoAtualizar()
            if (t >= 1)
                mov._concluir(false)
        } else if (mov.estado === "fadeOut") {
            const t = Math.min(1, mov.faseDecorridoMs / 95)
            a.opacity = 1 - 0.85 * (1 - Math.pow(1 - t, 3))
            mov._log("fadeOut", a.contentY)
            if (t >= 1) {
                const s = mov._limitar(mov.saltoY, minY, maxY)
                a.contentY = s
                mov.ultimoYEscrito = s
                mov.inicioY = s
                mov.faseDecorridoMs = 0
                mov.estado = "fadeIn"
                mov._log("salto", s)
            }
        } else if (mov.estado === "fadeIn") {
            const tOp = Math.min(1, mov.faseDecorridoMs / 95)
            a.opacity = 0.15 + 0.85 * (1 - Math.pow(1 - tOp, 3))
            const tY = Math.min(1, mov.faseDecorridoMs / Math.max(1, mov.duracaoMs))
            const e = 1 - Math.pow(1 - tY, 3)
            const y = mov._limitar(mov.inicioY + (mov.destinoY - mov.inicioY) * e, minY, maxY)
            a.contentY = y
            mov.ultimoYEscrito = y
            mov._log("fadeIn", y)
            if (mov.aoAtualizar)
                mov.aoAtualizar()
            if (tY >= 1) {
                a.opacity = 1
                mov._concluir(false)
            }
        } else if (mov.estado === "listaFadeOut") {
            const t = Math.min(1, mov.faseDecorridoMs / 95)
            a.opacity = 1 - 0.85 * (1 - Math.pow(1 - t, 3))
            mov._log("listaFadeOut", a.contentY)
            if (t >= 1) {
                if (mov.listaAlvo && mov.indiceAlvo >= 0 && mov.indiceAlvo < mov.listaAlvo.count)
                    mov.listaAlvo.positionViewAtIndex(mov.indiceAlvo, mov.modoLista)
                mov.ultimoYEscrito = a.contentY
                mov.destinoY = a.contentY
                mov.faseDecorridoMs = 0
                mov.estado = "listaFadeIn"
                mov._log("listaSalto", a.contentY)
            }
        } else if (mov.estado === "listaFadeIn") {
            const tOp = Math.min(1, mov.faseDecorridoMs / 95)
            a.opacity = 0.15 + 0.85 * (1 - Math.pow(1 - tOp, 3))
            mov._log("listaFadeIn", a.contentY)
            if (tOp >= 1) {
                a.opacity = 1
                mov._concluir(false)
            }
        }
    }

    Connections {
        target: mov.area
        function onDraggingChanged() {
            if (mov.area && mov.area.dragging)
                mov.interromperPorUsuario()
        }
        function onFlickingChanged() {
            if (mov.area && mov.area.flicking)
                mov.interromperPorUsuario()
        }
    }

    // Pulso ativo estritamente durante o movimento (250–450 ms); desligado em repouso (0% CPU).
    Timer {
        id: pulsoMovimento
        interval: 16
        repeat: true
        running: mov.ativo
        onTriggered: mov._passo()
    }
}
