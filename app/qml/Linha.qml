import QtQuick
import QtQuick.Layouts
import rustinho

// Linha de campos e botões lado a lado. Na tela estreita (celular em pé),
// cada coisa vai para uma linha própria, ocupando a largura toda.
// Use no lugar de RowLayout nos formulários.
GridLayout {
    id: linha
    property real spacing: 12
    // Quantas colunas na tela estreita (1 = um embaixo do outro).
    property int colunasEstreito: 1
    readonly property bool dobrar: Tema.estreito

    columns: dobrar ? colunasEstreito : 999
    columnSpacing: spacing
    rowSpacing: Math.max(10, spacing)

    property var _antes: ({})

    function ajustar() {
        for (let i = 0; i < children.length; i++) {
            const c = children[i]
            if (!c || c.Layout === undefined)
                continue
            // Espaçador vazio (Item {}) some na coluna.
            const vazio = c.children.length === 0 && String(c).indexOf("QQuickItem") === 0
            if (dobrar) {
                if (_antes[i] === undefined)
                    _antes[i] = { f: c.Layout.fillWidth, w: c.Layout.preferredWidth, m: c.Layout.maximumWidth, v: c.visible }
                if (vazio) {
                    c.visible = false
                } else if (c.manterLargura !== true) {
                    c.Layout.fillWidth = true
                    c.Layout.preferredWidth = -1
                    c.Layout.maximumWidth = Number.POSITIVE_INFINITY
                }
            } else if (_antes[i] !== undefined) {
                const a = _antes[i]
                c.Layout.fillWidth = a.f
                c.Layout.preferredWidth = a.w
                c.Layout.maximumWidth = a.m
                if (vazio)
                    c.visible = a.v
            }
        }
    }
    onDobrarChanged: ajustar()
    Component.onCompleted: ajustar()
}
