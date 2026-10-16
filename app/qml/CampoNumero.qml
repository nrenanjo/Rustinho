import QtQuick
import QtQuick.Layouts
import rustinho

// Campo curto de número (%, R$, quantidade ou só dígitos), que se arruma
// sozinho ao sair ("12,5" → "12,50" no dinheiro).
Campo {
    id: campoNumero
    // "dinheiro", "percentual", "quantidade" ou "" (não arruma).
    property string tipoNumero: ""
    numerico: true
    maximo: 24
    Layout.fillWidth: true
    onSaiu: {
        if (tipoNumero === "" || texto.trim() === "")
            return
        const r = Api.chamar("numero_formatar", { tipo: tipoNumero, texto: texto })
        if (r.ok && r.dados.texto !== texto)
            texto = r.dados.texto
    }
}
