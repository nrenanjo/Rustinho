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
    readonly property bool dobrar: Tema.fino

    columns: dobrar ? colunasEstreito : 999
    columnSpacing: spacing
    rowSpacing: Math.max(10, spacing)

    property var _antes: new Map()

    function ajustar() {
        for (let i = 0; i < children.length; i++) {
            const c = children[i]
            if (!c || c.Layout === undefined)
                continue
            // Espaçador vazio (Item {}) some na coluna.
            const vazio = c.children.length === 0 && String(c).indexOf("QQuickItem") === 0
            if (dobrar) {
                if (!_antes.has(c))
                    _antes.set(c, { f: c.Layout.fillWidth, w: c.Layout.preferredWidth, m: c.Layout.maximumWidth, n: c.Layout.minimumWidth, v: c.visible })
                if (vazio) {
                    c.visible = false
                } else if (c.manterLargura !== true) {
                    c.Layout.fillWidth = true
                    c.Layout.preferredWidth = -1
                    c.Layout.maximumWidth = Number.POSITIVE_INFINITY
                    c.Layout.minimumWidth = 0
                }
            } else if (_antes.has(c)) {
                const a = _antes.get(c)
                c.Layout.fillWidth = a.f
                c.Layout.preferredWidth = a.w
                c.Layout.maximumWidth = a.m
                c.Layout.minimumWidth = a.n
                if (vazio)
                    c.visible = a.v
            }
        }
    }
    onDobrarChanged: ajustar()
    // Filhos criados depois (Repeater) também se ajustam.
    onChildrenChanged: if (dobrar) Qt.callLater(ajustar)
    Component.onCompleted: ajustar()
}
