import QtCore
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts
import rustinho

// Configurações › Fiscal (0.9.15; FIS-CAT e FIS-P4 na 0.9.19): a emissão de notas pelo
// Rustinho, como está de verdade hoje. A emissão e a integração com um
// emissor fiscal ainda não existem: nada aqui finge estado, documento ou
// pendência. Quando existirem, estes blocos mostram os dados reais.
ColumnLayout {
    id: emissao
    readonly property var janela: ApplicationWindow.window
    // Vêm da tela Fiscal: o seletor do FIS-LIGA, a UF e o CNPJ da loja e a
    // conferência das peças.
    property var liga: null
    property var dadosDaLoja: null
    property var dados: null
    readonly property bool ligado: !!liga && liga.ligado
    readonly property int semNcm: dados ? dados.semNcm.length : 0
    readonly property bool lojaPronta: !!dadosDaLoja && dadosDaLoja.satisfeito
    // T010b: o que foi achado nas notas da loja (null = ainda não procurou).
    property var achado: null

    // A pasta escolhida (para ler de novo com o emitente que for a loja).
    property string pastaLida: ""

    function descobrir(pasta, cnpj) {
        pastaLida = pasta
        const r = Api.chamar("fiscal_descobrir", { entradas: [pasta], cnpj: cnpj || "" })
        if (!r.ok) {
            emissao.janela.avisar(r.erro, "erro")
            return
        }
        achado = r.dados
        painelDescoberta.open()
    }

    function usar(campo, valor, nome, ambiente, ultimoNumero) {
        emissao.janela.chamarGerente("fiscal_descoberta_usar", { campo: campo, valor: String(valor),
                                                                 ambiente: ambiente || 0, ultimoNumero: ultimoNumero || 0 }, r => {
            if (r.ok)
                emissao.janela.avisar("Gravado: " + nome + ".", "ok")
            else
                emissao.janela.avisar(r.erro, "erro")
        })
    }

    function nomeDoCrt(c) {
        return ({ "1": "1 · Simples Nacional", "2": "2 · Simples Nacional, excesso de sublimite", "3": "3 · Regime Normal",
                  "4": "4 · MEI" })[c] || c
    }

    // C-1 da R4: o CNPJ alfanumérico mantém as letras (a loja guarda o CNPJ só com números e letras maiúsculas).
    function confere(achadoValor, daLoja) {
        if (!daLoja)
            return "a loja ainda não informou (informe em Configurações › Loja)"
        return achadoValor === daLoja ? "confere com o da loja" : "diferente do informado em Loja: confira lá"
    }
    readonly property var passos: [
        { titulo: "1. Emissor fiscal", situacao: "ainda não disponível", pronto: false,
          explicacao: "A ligação com o emissor (o programa ACBrMonitorPLUS, que assina e envia as notas à SEFAZ neste computador) ainda não está pronta nesta versão." },
        { titulo: "2. Certificado A3", situacao: ligado ? "liberado neste computador" : "desligado neste computador", pronto: ligado,
          explicacao: ligado ? "Um gerente liberou o uso do certificado A3 aqui." : "Um gerente liga no quadro \"Certificado A3 e integração fiscal\", nesta mesma tela." },
        { titulo: "3. Testar a conexão", situacao: "depois dos passos 1 e 2", pronto: false,
          explicacao: "O Rustinho confere a ligação com a SEFAZ em homologação, sem nota de verdade." },
        { titulo: "4. Condições para emitir", situacao: lojaPronta && semNcm === 0 ? "cadastro em dia" : "falta acertar", pronto: false,
          explicacao: (lojaPronta ? "UF e CNPJ da loja informados. " : "Falta informar a UF e o CNPJ da loja. ")
                      + (semNcm === 0 ? "Nenhuma peça sem NCM. " : semNcm + (semNcm === 1 ? " peça sem NCM. " : " peças sem NCM. "))
                      + "O regime da loja (Simples Nacional ou outro) fica com o contador." }
    ]
    spacing: 12

    Faixa {
        titulo: "Emissão fiscal"
        color: Tema.verde
        corTitulo: Tema.apagado
        Layout.fillWidth: true
        Nota {
            text: "Configure a integração fiscal para emitir documentos nesta loja."
            Layout.fillWidth: true
            Layout.topMargin: -8
        }
        Flow {
            spacing: 14
            Layout.fillWidth: true
            Repeater {
                model: ["NFC-e · modelo 65", "NF-e · modelo 55"]
                delegate: Rectangle {
                    required property string modelData
                    radius: Tema.raioCampo
                    color: Tema.papel
                    implicitWidth: linhaModelo.implicitWidth + 60
                    implicitHeight: 46
                    Row {
                        id: linhaModelo
                        anchors.centerIn: parent
                        spacing: 8
                        Text {
                            text: parent.parent.modelData
                            textFormat: Text.PlainText
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.Bold
                            font.pixelSize: 16
                            anchors.verticalCenter: parent.verticalCenter
                        }
                        Text {
                            text: "em breve"
                            color: Tema.apagado
                            font.family: Tema.titulo
                            font.italic: true
                            font.pixelSize: 14
                            anchors.verticalCenter: parent.verticalCenter
                        }
                    }
                }
            }
        }
        Nota {
            text: "Por enquanto, as notas continuam no sistema de hoje. O cadastro das peças já guarda o que a nota vai pedir (NCM, CEST, CSOSN, PIS/Cofins, IPI, ANP e ISS)."
            font.pixelSize: 14
            Layout.fillWidth: true
        }
    }

    // FIS-P4 (0.9.19, etapa 3; texto A §15): o que falta para emitir, numa
    // sequência só (emissor → certificado → testar → condições), cada passo
    // com a situação de verdade. Sem botão que não leva a lugar nenhum.
    Faixa {
        titulo: "Para emitir notas pelo Rustinho"
        color: Tema.azul
        Layout.fillWidth: true
        espacamento: 10
        Repeater {
            model: emissao.passos
            delegate: RowLayout {
                id: passo
                required property var modelData
                required property int index
                spacing: 14
                Layout.fillWidth: true
                Rectangle {
                    implicitWidth: 34
                    implicitHeight: 34
                    radius: 17
                    color: passo.modelData.pronto ? Tema.verde : Tema.papel
                    Layout.alignment: Qt.AlignTop
                    Text {
                        anchors.centerIn: parent
                        text: passo.modelData.pronto ? "✓" : String(passo.index + 1)
                        textFormat: Text.PlainText
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.weight: Font.Bold
                        font.pixelSize: 16
                    }
                }
                ColumnLayout {
                    spacing: 2
                    Layout.fillWidth: true
                    Text {
                        text: passo.modelData.titulo + " · " + passo.modelData.situacao
                        textFormat: Text.PlainText
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.weight: Font.DemiBold
                        font.pixelSize: 16
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                    Nota {
                        text: passo.modelData.explicacao
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                }
            }
        }
        // Bateria de 4ef0919 (geometria G4): o rótulo cabe no celular (o antigo, de 47 letras, alargava o quadro e os
        // passos passavam da borda da tela). B-6 da R9 (U1): a frase ao lado diz o que o botão faz, o que o rótulo curto
        // deixou de dizer.
        Nota {
            // B-7 da R10: diz quais dados.
            // B-7 da R11: o que se pode usar (o CNPJ só é conferido com o da loja).
            text: "O Rustinho pode ler os arquivos XML das notas que a loja já emitiu pelo sistema de hoje, conferir o CNPJ "
                  + "e trazer daí o regime, a série, o último número e o identificador do CSC. Nada é gravado sem você confirmar."
            font.pixelSize: 14
            Layout.fillWidth: true
        }
        Botao {
            text: "Ler as notas da loja"
            icone: "documento"
            tipo: "secundario"
            onClicked: seletorDescoberta.open()

        }
    }

    // As pendências (sem resposta, contingência) ficam no painel das notas,
    // logo abaixo (FIS-P1): um lugar só, sem dois cartões dizendo coisas diferentes.


    // T010b: a pasta com os XML que o sistema de hoje guarda (o seletor nativo, diretriz do dono).
    FolderDialog {
        id: seletorDescoberta
        title: "Escolha a pasta com os XML das notas que a loja já emitiu"
        currentFolder: StandardPaths.standardLocations(StandardPaths.DocumentsLocation)[0]
        onAccepted: emissao.descobrir(Api.caminho(selectedFolder.toString()), "")
    }

    // T010b: o que foi achado, item por item; nada se grava sem o "Usar este".
    Painel {
        id: painelDescoberta
        width: Math.min(680, Tema.largura - 24)
        readonly property var d: emissao.achado ? emissao.achado.descoberta : null
        readonly property var loja: emissao.achado ? emissao.achado.loja : null
        // Soma das autorizadas dos emitentes da pergunta (o resumo, B-2 da R10).
        readonly property int somaAutorizadas: !d ? 0 : d.candidatos.reduce((t, c) => t + c.notas, 0)
        contentItem: Flickable {
            implicitHeight: Math.min(colunaDescoberta.implicitHeight, painelDescoberta.alturaMaxima - 40)
            contentHeight: colunaDescoberta.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            ColumnLayout {
                id: colunaDescoberta
                width: parent.width
                spacing: 12
                Titulo { text: "O que achei nas notas da loja"; font.pixelSize: 26; wrapMode: Text.WordWrap; Layout.fillWidth: true }
                Nota {
                    text: !painelDescoberta.d ? ""
                          // B-6 da R9: com ou sem a pergunta, o resumo termina igual (antes, pela precedência do "?:", o
                          // ramo dos candidatos ficava sem o ponto e sem "Nada é gravado sem você confirmar").
                          : (painelDescoberta.d.candidatos.length > 0
                             ? (painelDescoberta.somaAutorizadas === 0
                                ? "Só notas denegadas, de " + painelDescoberta.d.candidatos.length + " emitentes"
                                // B-2 da R10: o singular também com a pergunta.
                                : (painelDescoberta.somaAutorizadas === 1 ? "1 nota autorizada lida, de "
                                   : painelDescoberta.somaAutorizadas + " notas autorizadas lidas, de ")
                                  + painelDescoberta.d.candidatos.length + " emitentes")
                             : (painelDescoberta.d.lidas === 1 ? "1 nota autorizada lida" : painelDescoberta.d.lidas + " notas autorizadas lidas"))
                          // B-6 da R11: o motivo da nota de CPF também.
                          + (painelDescoberta.d.ignorados.length === 1 ? " · 1 arquivo deixado de lado (não autorizado, de outro emitente, de CPF ou sem leitura)"
                             : painelDescoberta.d.ignorados.length ? " · " + painelDescoberta.d.ignorados.length + " arquivos deixados de lado (não autorizados, de outro emitente, de CPF ou sem leitura)" : "")
                          + ". Nada é gravado sem você confirmar."
                    font.pixelSize: 14
                    Layout.fillWidth: true
                }
                Nota {
                    visible: !!painelDescoberta.d && painelDescoberta.d.lidas === 0 && painelDescoberta.d.candidatos.length === 0
                    text: "Não achei nota autorizada da loja nesta pasta."
                    Layout.fillWidth: true
                }
                // C-M8 da R1: com notas de mais de um emitente (por exemplo, notas de compra), o Rustinho pergunta.
                Rotulo {
                    visible: !!painelDescoberta.d && painelDescoberta.d.candidatos.length > 0
                    text: "Qual destes CNPJ é o da loja?"
                }
                Nota {
                    visible: !!painelDescoberta.d && painelDescoberta.d.candidatos.length > 0
                    text: "A pasta tem notas de mais de um emitente (pode ter notas de compra). Escolha o da loja; os outros ficam de lado."
                    Layout.fillWidth: true
                }
                Repeater {
                    model: painelDescoberta.d ? painelDescoberta.d.candidatos : []
                    delegate: RowLayout {
                        id: linhaCandidato
                        required property var modelData
                        spacing: 10
                        Layout.fillWidth: true
                        Text {
                            // N-3 da R8: o emitente só com denegadas não aparece como "0 notas".
                            text: linhaCandidato.modelData.cnpj + " · "
                                  + (linhaCandidato.modelData.notas === 0
                                     // B-7 da R10: o mesmo formato no singular e no plural.
                                     ? (linhaCandidato.modelData.denegadas === 1 ? "só 1 nota denegada"
                                        : "só " + linhaCandidato.modelData.denegadas + " notas denegadas")
                                     : linhaCandidato.modelData.notas + (linhaCandidato.modelData.notas === 1 ? " nota" : " notas"))
                            textFormat: Text.PlainText
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.pixelSize: 15
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                        Botao {
                            text: "É o da loja"
                            tipo: "discreto"
                            onClicked: emissao.descobrir(emissao.pastaLida, linhaCandidato.modelData.cnpj)
                        }
                    }
                }
                Nota {
                    visible: !!painelDescoberta.d && painelDescoberta.d.qrVersao3 && !painelDescoberta.d.identificadorCsc
                    text: "As NFC-e da loja usam o QR Code versão 3, que não usa CSC: não é preciso pedir o CSC ao contador."
                    Layout.fillWidth: true
                }
                Repeater {
                    model: !painelDescoberta.d ? [] : [
                        { campo: "", nome: "CNPJ", a: painelDescoberta.d.cnpj, nota: painelDescoberta.d.cnpj ? emissao.confere(painelDescoberta.d.cnpj.valor, painelDescoberta.loja.cnpj.replace(/[^0-9A-Za-z]/g, "").toUpperCase()) : "" },
                        { campo: "", nome: "UF", a: painelDescoberta.d.uf, nota: painelDescoberta.d.uf ? emissao.confere(painelDescoberta.d.uf.valor, painelDescoberta.loja.uf) : "" },
                        { campo: "", nome: "Inscrição estadual", a: painelDescoberta.d.ie, nota: "" },
                        { campo: "crt", nome: "Regime (CRT)", a: painelDescoberta.d.crt, nota: "confirme com o contador" },
                        { campo: "identificadorCsc", nome: "Identificador do CSC", a: painelDescoberta.d.identificadorCsc,
                          nota: "o CSC (o código secreto) não aparece nas notas: ele fica com o contador ou no portal da SEFAZ" }
                    ].filter(x => !!x.a)
                    delegate: ColumnLayout {
                        id: itemAchado
                        required property var modelData
                        spacing: 4
                        Layout.fillWidth: true
                        Text {
                            text: itemAchado.modelData.nome + ": " + (itemAchado.modelData.campo === "crt" ? emissao.nomeDoCrt(itemAchado.modelData.a.valor) : itemAchado.modelData.a.valor)
                                  + (itemAchado.modelData.a.outros.length ? " (em notas antigas: " + itemAchado.modelData.a.outros.join(", ") + ")" : "")
                            textFormat: Text.PlainText
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.DemiBold
                            font.pixelSize: 15
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                        Nota {
                            // K10 da R2: o número da nota (posições 26 a 34 da chave), e não o fim da chave.
                            // Cód-10 da R3: sem número legível na chave, não mostra "nº NaN".
                            readonly property var numeroDaNota: parseInt(String(itemAchado.modelData.a.chaveDaNota).substr(25, 9), 10)
                            text: (isNaN(numeroDaNota) ? "De uma das notas lidas" : "Da nota nº " + numeroDaNota)
                                  + (itemAchado.modelData.nota ? " · " + itemAchado.modelData.nota : "")
                            font.pixelSize: 13
                            Layout.fillWidth: true
                        }
                        Botao {
                            visible: itemAchado.modelData.campo !== ""
                            text: "Usar este"
                            tipo: "discreto"
                            onClicked: emissao.usar(itemAchado.modelData.campo, itemAchado.modelData.a.valor, itemAchado.modelData.nome)
                        }
                    }
                }
                Rotulo { visible: !!painelDescoberta.d && painelDescoberta.d.series.length > 0; text: "Séries usadas" }
                Repeater {
                    model: painelDescoberta.d ? painelDescoberta.d.series : []
                    delegate: RowLayout {
                        id: linhaSerie
                        required property var modelData
                        spacing: 10
                        Layout.fillWidth: true
                        Text {
                            text: (linhaSerie.modelData.modelo === 65 ? "NFC-e" : "NF-e") + " série " + linhaSerie.modelData.serie
                                  + (linhaSerie.modelData.ambiente === 1 ? " (produção)" : " (homologação)")
                                  // B-4 da R7: a série só com notas denegadas diz isso (e não "0 notas").
                                  + ": último número " + linhaSerie.modelData.ultimoNumero + " · "
                                  + (linhaSerie.modelData.notas === 0 ? "só notas denegadas (o número delas foi gasto)"
                                     : linhaSerie.modelData.notas + (linhaSerie.modelData.notas === 1 ? " nota" : " notas"))
                                  // K5 da R2: o último número é o das notas desta pasta.
                                  + (linhaSerie.modelData.ambiente === 1 ? ". Usando esta série, a próxima nota do Rustinho sai depois da "
                                                                           + linhaSerie.modelData.ultimoNumero
                                                                           + " (o maior número nas notas desta pasta: confira que ela tem as notas mais recentes)." : "")
                            textFormat: Text.PlainText
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.pixelSize: 14
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                        // C-M4 da R1: usar a série grava também a última nota dela, e a próxima do Rustinho sai depois.
                        Botao {
                            visible: linhaSerie.modelData.ambiente === 1
                            text: "Usar esta série"
                            tipo: "discreto"
                            onClicked: emissao.usar(linhaSerie.modelData.modelo === 65 ? "serieNfce" : "serieNfe", linhaSerie.modelData.serie,
                                                    "Série da " + (linhaSerie.modelData.modelo === 65 ? "NFC-e" : "NF-e"),
                                                    linhaSerie.modelData.ambiente, linhaSerie.modelData.ultimoNumero)
                        }
                    }
                }
            }
        }
    }
}
