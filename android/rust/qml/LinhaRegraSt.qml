import QtQuick
import QtQuick.Layouts
import rustinho

// O formulário de uma regra de ST (FIS-P5 e FIS-P6, 0.9.19, etapa 3): estado,
// começo do CEST, período, se tem ST, a descrição e a fonte. Fica no painel de
// edição da tela Fiscal (a consulta é a lista, separada). Numa regra que já
// valeu, o estado, o CEST, o início e o "com ST" ficam travados: para mudar,
// encerra-se e cria-se outra (o núcleo confere de novo).
ColumnLayout {
    id: linha
    property var ufs: []
    property bool travada: false
    spacing: 10

    function campo(nome) {
        return { uf: campoUf, cestInicio: campoCest, vigenteDe: campoDe, vigenteAte: campoAte,
                 descricao: campoDescricao, fonte: campoFonte, dispositivo: campoDispositivo }[nome] || null
    }

    function preencher(r) {
        campoUf.indice = Math.max(0, linha.ufs.indexOf(r.uf || ""))
        campoCest.texto = r.cestInicio || ""
        campoDe.texto = r.vigenteDe || ""
        campoAte.texto = r.vigenteAte || ""
        campoSt.checked = !!r.comSt
        campoDescricao.texto = r.descricao || ""
        campoFonte.texto = r.fonte || ""
        campoDispositivo.texto = r.dispositivo || ""
        for (const c of [campoCest, campoDe, campoAte, campoDescricao, campoFonte, campoDispositivo])
            c.erro = ""
    }

    function valores() {
        return { uf: linha.ufs[campoUf.indice] || "", cestInicio: campoCest.texto, vigenteDe: campoDe.texto,
                 vigenteAte: campoAte.texto, comSt: campoSt.checked, descricao: campoDescricao.texto,
                 fonte: campoFonte.texto, dispositivo: campoDispositivo.texto }
    }

    function focar() {
        if (linha.travada)
            campoAte.focar()
        else
            campoCest.focar()
    }

    RowLayout {
        spacing: 10
        Layout.fillWidth: true
        Selecao {
            id: campoUf
            rotulo: "Estado"
            modelo: linha.ufs
            enabled: !linha.travada
            Layout.preferredWidth: Tema.estreito ? 80 : 110
        }
        Campo {
            id: campoCest
            rotulo: "Começo do CEST"
            dica: "01"
            maximo: 9
            enabled: !linha.travada
            Layout.fillWidth: true
        }
    }
    Marcar {
        id: campoSt
        text: "Com substituição tributária (ST)"
        enabled: !linha.travada
    }
    RowLayout {
        spacing: 10
        Layout.fillWidth: true
        Campo {
            id: campoDe
            rotulo: "Vale a partir de"
            dica: "dd/mm/aaaa"
            maximo: 10
            enabled: !linha.travada
            Layout.fillWidth: true
        }
        Campo {
            id: campoAte
            rotulo: "Até (vazio: sem fim)"
            dica: "dd/mm/aaaa"
            maximo: 10
            Layout.fillWidth: true
        }
    }
    Campo {
        id: campoDescricao
        rotulo: "Descrição"
        dica: "ex.: Autopeças fora da ST"
        maximo: 120
        Layout.fillWidth: true
    }
    Campo {
        id: campoFonte
        rotulo: "Fonte (o ato e onde foi lido)"
        dica: "ex.: Portaria SRE 34/2026, legislacao.fazenda.sp.gov.br"
        maximo: 200
        Layout.fillWidth: true
    }
    Campo {
        id: campoDispositivo
        rotulo: "Artigo, anexo ou item"
        maximo: 200
        Layout.fillWidth: true
    }
}
