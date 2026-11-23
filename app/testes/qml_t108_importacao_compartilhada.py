#!/usr/bin/env python3
"""T108 (texto R do proprietário, 21/11/2026): a lógica das telas da importação compartilhada, sem o núcleo.

Teste de QML (qmltestrunner, sem janela). As funções e as expressões saem do próprio QML:
- de `app/qml/TelaCompras.qml`, o `importar`;
- de `app/qml/ImportarCadastros.qml`, o `lerArquivos`, o `cat` e o `texto`, o modelo da prévia, a visibilidade da
  linha, o texto do "já" e o `onToggled` do Marcar.
O núcleo (`Api`), a janela e o tema são simulados. O programa dos roteiros de tela não chega a uma prévia com nota de
verdade (PEND-09: ele só confia nas autoridades de verdade); por isso esta prova lê o QML.

Confere:
  N1  (2ª conferência): UMA mensagem só no fim da importação de Compras; com erro, ela é de erro e leva também as
      notas importadas e as que já estavam em Compras.
  A3  (FI-1 da opção nova): "Notas de compra em Compras" prende "Fornecedores", na 1ª leitura e ao marcar, com o porquê.
  A8  a linha das notas de compra aparece também só com as que não servem para Compras, e diz quantas.
  N5  o singular: "1 já estava em Compras".

    python3 app/testes/qml_t108_importacao_compartilhada.py [raiz do projeto]
"""

import os
import re
import shutil
import subprocess
import sys
import tempfile

RAIZ = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..", ".."))

API = """pragma Singleton
import QtQuick
QtObject {
    property var respostas: []
    property var gravacoes: []
    property var gravacoesDemoradas: []
    property bool repetidaAgora: false
    function caminho(a) { return a }
    function novaChave() { return "chave" }
    function gravar(comando, dados, chave) { gravacoes.push({ comando: comando, dados: dados, chave: chave }); return respostas.shift() }
    function gravarDemorado(comando, dados, texto, depois, chave) { gravacoesDemoradas.push({ comando: comando, dados: dados, texto: texto, chave: chave }); depois(respostas.shift()) }
    function chamarDemorado(comando, dados, texto, depois) { depois(respostas.shift()) }
    function chamar(comando, dados) { return respostas.shift() }
}
"""

TEMA = """pragma Singleton
import QtQuick
QtObject {
    property string verde: "verde"
    property string azul: "azul"
    property string papel: "papel"
}
"""

TESTE = """import QtQuick
import QtTest
import falsos

TestCase {
    name: "T108"

    QtObject {
        id: janelaFalsa
        property var avisos: []
        property var abertas: []
        property var abertasProps: []
        property var confirmacoes: []
        property bool guiarRetorno: false
        function avisar(texto, tipo) { avisos.push({ texto: texto, tipo: tipo }) }
        function abrir(nome, propriedades) { abertas.push(nome); abertasProps.push({ nome: nome, props: propriedades || {} }) }
        function abrirFuncao(f) { abertas.push("funcao:" + f.id) }
        function guiar(campo, erro, depois) { return guiarRetorno }
        function confirmar(titulo, detalhe, acao, depois, tipo) {
            confirmacoes.push({ titulo: titulo, detalhe: detalhe, acao: acao, tipo: tipo })
        }
    }

    QtObject {
        id: telaInicioFalsa
        property var janela: janelaFalsa
        function funcao(id) { return { id: id, disponivel: true } }
%ABRIR_AVISO%
    }

    QtObject {
        id: linhaCompraFalsa
        property var item: ({})
        readonly property bool rascunho: item.situacao === "rascunho"
        readonly property bool bloqueada: rascunho && item.autorizacao === "negada"
        readonly property bool aguardandoConferencia: rascunho && item.podeLancar === false && !bloqueada
        function textoSituacao() { return %TEXTO_SITUACAO_COMPRA% }
    }

    QtObject {
        id: telaCompraFalsa
        property var janela: janelaFalsa
        property var compra: null
%DESCARTAR%
    }

    QtObject {
        id: linhaItem
        property var item: ({})
        property bool editavel: true
        property var peca: item.peca || null
        readonly property bool pendente: item.acao === ""
        readonly property bool ignorado: item.acao === "ignorar"
        function notaItem() { return %NOTA_ITEM% }
        function mostraCustoBaixou() { return %VIS_BAIXOU% }
    }

    QtObject {
        id: conflitoCnpj
        property var mostrados: []
        function mostrar(c) { mostrados.push(c) }
    }

    QtObject {
        id: listaNaoImportadas
        property real contentY: 0
        property real contentHeight: 300
        property real height: 150
        function forceLayout() {}
    }

    QtObject {
        id: travaDescartar
        property bool running: false
        function restart() { running = true }
    }

    QtObject {
        id: tela
        property var janela: janelaFalsa
        property var itens: []
        property var pendentes: []
        property string avisoPendentes: ""
        property var naoImportadas: []
        function buscar() {}
        function nomeArquivo(a) { return a }
%CONFERIR_COMPRA%
%IMPORTAR%
%CHAVE_NOTA%
%ARQUIVOS_NOTA%
%TEXTO_ARQUIVOS_NOTA%
%GUARDAR_NAO_IMPORTADAS%
%DESCARTAR_NAO_IMPORTADA%
    }

    QtObject {
        id: painel
        property var janela: janelaFalsa
        property var arquivos: ["pasta"]
        property string chaveTela: ""
        property var previa: null
        property string erro: ""
        property bool trabalhando: false
        property var escolhas: null
        property bool qClientes: false
        property bool qFornecedores: false
        property bool qPecasLoja: false
        property bool qPecasFornecedor: false
        property bool qCompras: false
        property int refeitas: 0
        property var pendenciaAberta: null
        property string chavePendencia: ""
        property string pecaDesativadaId: ""
        property bool corrigindoIdentificacao: false
        property bool liberada: true
        property bool fechou: false
        signal importou()
        function close() { fechou = true }
        function carregarPendencias() {}
        function refazerPrevia(x) { refeitas += 1 }
        function dadosDaPrevia(comOpcoes) { return {} }
        function exigirSenhaDaTela(ok, cancel) {}
        function cadastrarAgora() {}
%CAT%
%TEXTO%
%LER%
%DEPOIS_IMPORTAR%
%ABRIR_PENDENCIA%
%RESOLVER_NOVA%
        function linhas() { return %MODELO% }
        function alternar(linha, checked) {
%ALTERNAR%
        }
    }

    QtObject { id: campoCorrNome; property string texto: ""; property string erro: "" }
    QtObject { id: campoCorrCodigo; property string texto: ""; property string erro: "" }
    QtObject { id: campoCorrBarras; property string texto: ""; property string erro: "" }
    QtObject { id: campoCorrUnidade; property string texto: ""; property string erro: "" }

    function visivel(modelData) { return %VISIVEL% }
    function textoJa(linha) { return %TEXTOJA% }

    function previa(campos) {
        const p = { clientesNovos: 0, clientesJa: 0, fornecedoresNovos: 0, fornecedoresJa: 0, pecasLojaNovas: 0,
                    pecasFornecedorNovas: 0, comprasNovas: 0, comprasJa: 0, comprasNaoServe: 0, categorias: [],
                    exemplosClientes: [], exemplosFornecedores: [] }
        for (const k in campos)
            p[k] = campos[k]
        return p
    }
    function linha(chave) { return painel.linhas().find(l => l.chave === chave) }

    function init() {
        janelaFalsa.avisos = []
        janelaFalsa.abertas = []
        janelaFalsa.abertasProps = []
        janelaFalsa.confirmacoes = []
        janelaFalsa.guiarRetorno = false
        Api.respostas = []
        Api.gravacoes = []
        Api.gravacoesDemoradas = []
        tela.itens = []
        tela.naoImportadas = []
        conflitoCnpj.mostrados = []
        travaDescartar.running = false
        linhaCompraFalsa.item = ({})
        painel.escolhas = null
        painel.fechou = false
        painel.pendenciaAberta = null
        painel.corrigindoIdentificacao = false
        painel.erro = ""
        painel.qClientes = false
        painel.qFornecedores = false
        painel.qPecasLoja = false
        painel.qPecasFornecedor = false
        painel.qCompras = false
        telaCompraFalsa.compra = null
        linhaItem.item = ({})
        linhaItem.editavel = true
        campoCorrNome.erro = ""
        campoCorrCodigo.erro = ""
        campoCorrBarras.erro = ""
        campoCorrUnidade.erro = ""
    }

    function test_n1_com_erro_uma_mensagem_so_e_de_erro() {
        Api.respostas = [{ ok: true, dados: { compras: [{ id: "a" }, { id: "b" }], erros: ["x.xml: XML alterado"],
                                              conflitos: [], jaEmCompras: 1 } }]
        tela.importar(["pasta"])
        compare(janelaFalsa.avisos.length, 1, JSON.stringify(janelaFalsa.avisos))
        const a = janelaFalsa.avisos[0]
        compare(a.tipo, "erro")
        verify(a.texto.indexOf("x.xml: XML alterado") >= 0, a.texto)
        verify(a.texto.indexOf("2 notas importadas") >= 0, a.texto)
        verify(a.texto.indexOf("1 nota já estava em Compras") >= 0, a.texto)
    }

    function test_n1_uma_nota_e_as_que_ja_estavam() {
        Api.respostas = [{ ok: true, dados: { compras: [{ id: "a" }], erros: [], conflitos: [], jaEmCompras: 2 } }]
        tela.importar(["pasta"])
        compare(janelaFalsa.abertas, ["TelaCompra"])
        compare(janelaFalsa.avisos.length, 1, JSON.stringify(janelaFalsa.avisos))
        compare(janelaFalsa.avisos[0].tipo, "ok")
        verify(janelaFalsa.avisos[0].texto.indexOf("2 notas já estavam em Compras") >= 0, janelaFalsa.avisos[0].texto)
    }

    function test_n1_so_as_que_ja_estavam() {
        Api.respostas = [{ ok: true, dados: { compras: [], erros: [], conflitos: [], jaEmCompras: 3 } }]
        tela.importar(["pasta"])
        compare(janelaFalsa.avisos.length, 1, JSON.stringify(janelaFalsa.avisos))
        verify(janelaFalsa.avisos[0].texto.indexOf("3 notas já estavam em Compras") >= 0, janelaFalsa.avisos[0].texto)
    }

    function test_t111_aviso_pecas_cadastradas_e_pendencias_uma_mensagem_so() {
        Api.respostas = [{ ok: true, dados: { compras: [{ id: "a" }], erros: [], conflitos: [],
                                              pecasCadastradas: 3, pendenciasIdentificacao: 1 } }]
        tela.importar(["pasta"])
        compare(janelaFalsa.abertas, ["TelaCompra"])
        compare(janelaFalsa.avisos.length, 1, JSON.stringify(janelaFalsa.avisos))
        compare(janelaFalsa.avisos[0].tipo, "ok")
        verify(janelaFalsa.avisos[0].texto.indexOf("3 peças novas cadastradas automaticamente") >= 0, janelaFalsa.avisos[0].texto)
        verify(janelaFalsa.avisos[0].texto.indexOf("1 item com identificação duvidosa enviado para conferência") >= 0, janelaFalsa.avisos[0].texto)
    }

    function test_t111_singular_e_plural_com_erro() {
        Api.respostas = [{ ok: true, dados: { compras: [{ id: "a" }, { id: "b" }], erros: ["z.xml: falha"],
                                              conflitos: [], pecasCadastradas: 1, pendenciasIdentificacao: 2 } }]
        tela.importar(["pasta"])
        compare(janelaFalsa.avisos.length, 1, JSON.stringify(janelaFalsa.avisos))
        compare(janelaFalsa.avisos[0].tipo, "erro")
        verify(janelaFalsa.avisos[0].texto.indexOf("1 peça nova cadastrada automaticamente") >= 0, janelaFalsa.avisos[0].texto)
        verify(janelaFalsa.avisos[0].texto.indexOf("2 itens com identificação duvidosa enviados para conferência") >= 0, janelaFalsa.avisos[0].texto)
        verify(janelaFalsa.avisos[0].texto.indexOf("z.xml: falha") >= 0, janelaFalsa.avisos[0].texto)
    }

    function test_a3_primeira_leitura_prende_fornecedores() {
        Api.respostas = [{ ok: true, dados: previa({ comprasNovas: 2 }) }]
        painel.lerArquivos()
        verify(painel.qCompras, "as notas de compra marcadas")
        verify(painel.qFornecedores, "Fornecedores marcada junto (FI-1)")
        const f = linha("qFornecedores")
        verify(f.presa, "Fornecedores travada")
        verify(f.porque.indexOf("Notas de compra em Compras") >= 0, f.porque)
    }

    function test_a3_marcar_as_notas_marca_fornecedores() {
        painel.previa = previa({ comprasNovas: 1 })
        painel.alternar({ modelData: { chave: "qCompras" } }, true)
        verify(painel.qCompras)
        verify(painel.qFornecedores, "Fornecedores marcada junto (FI-1)")
        verify(painel.escolhas.qCompras, "a escolha guardada")
    }

    function test_a8_linha_so_com_as_que_nao_servem() {
        painel.previa = previa({ comprasNaoServe: 2 })
        const l = linha("qCompras")
        verify(visivel(l), "a linha aparece")
        verify(l.extra.indexOf("2 notas de fornecedor não servem para Compras") >= 0, l.extra)
    }

    function test_n5_singular_do_ja() {
        painel.previa = previa({ comprasJa: 1 })
        const t = textoJa({ modelData: linha("qCompras") })
        verify(t.indexOf("1 já estava em Compras") >= 0, t)
    }

    function test_t111_corrigir_identificacao_envia_campos_no_resolver() {
        painel.abrirPendencia({ id: "p-1", descricao: "CORREIA X", codigoFornecedor: "B-1", gtinComercial: "", unidade: "UN" })
        compare(campoCorrNome.texto, "CORREIA X")
        compare(campoCorrCodigo.texto, "B-1")
        campoCorrNome.texto = "CORREIA GATES"
        campoCorrCodigo.texto = "F1-200"
        campoCorrUnidade.texto = "PC"
        Api.respostas = [{ ok: true, dados: { produtoId: "prod-1", pendentes: 0 } }]
        painel.resolverComoNova(true)
        compare(Api.gravacoes.length, 1)
        compare(Api.gravacoes[0].comando, "xml_pendencia_resolver")
        compare(Api.gravacoes[0].dados.nome, "CORREIA GATES")
        compare(Api.gravacoes[0].dados.codigoFornecedor, "F1-200")
        compare(Api.gravacoes[0].dados.unidade, "PC")
        compare(painel.pendenciaAberta, null)
    }

    function test_t111_corrigir_identificacao_erro_no_campo_gtin() {
        painel.abrirPendencia({ id: "p-2", descricao: "FILTRO", codigoFornecedor: "B-2", gtinComercial: "", unidade: "UN" })
        campoCorrBarras.texto = "12345"
        Api.respostas = [{ ok: false, campo: "codigoBarras", erro: "GTIN inválido" }]
        painel.resolverComoNova(true)
        compare(campoCorrBarras.erro, "GTIN inválido")
        verify(painel.pendenciaAberta !== null, "mantém a pendência aberta para ajustar")
    }

    function test_t111_h2_selecao_multipla_envia_caminhos_em_uma_chamada() {
        Api.respostas = [{ ok: true, dados: { compras: [{ id: "1" }, { id: "2" }], erros: [], conflitos: [],
                                              pecasCadastradas: 2, pendenciasIdentificacao: 1 } }]
        tela.importar(["file:///tmp/a.xml", "file:///tmp/b.xml"])
        compare(Api.gravacoesDemoradas.length, 1, "H2: deve mandar todos os arquivos em uma única chamada")
        compare(Api.gravacoesDemoradas[0].comando, "compra_importar")
        compare(Api.gravacoesDemoradas[0].dados.caminhos.length, 2, JSON.stringify(Api.gravacoesDemoradas[0].dados))
        compare(janelaFalsa.avisos.length, 1)
        verify(janelaFalsa.avisos[0].texto.indexOf("2 notas importadas") >= 0, janelaFalsa.avisos[0].texto)
    }

    function test_t111_h3_corrigir_envia_flag_e_erro_nome() {
        painel.abrirPendencia({ id: "p-3", descricao: "CORREIA", codigoFornecedor: "B-1", gtinComercial: "", unidade: "UN" })
        campoCorrNome.texto = ""
        Api.respostas = [{ ok: false, campo: "nome", erro: "Informe o nome da peça." }]
        painel.resolverComoNova(true)
        compare(Api.gravacoes.length, 1)
        compare(Api.gravacoes[0].dados.corrigir, true, "H3: deve enviar corrigir: true")
        compare(campoCorrNome.erro, "Informe o nome da peça.", "H3: erro de nome deve ir para campoCorrNome")
        verify(painel.pendenciaAberta !== null)
    }

    function test_t111_h9_linha_item_editavel_motivo_e_baixou() {
        linhaItem.item = { acao: "nova", achadoPor: "importacao", unidade: "CX", peca: { custoBaixou: true } }
        linhaItem.editavel = false
        verify(linhaItem.notaItem().indexOf("confira antes de lançar") < 0,
               "H9b: nota já lançada não pode dizer confira antes de lançar: " + linhaItem.notaItem())
        linhaItem.editavel = true
        verify(linhaItem.mostraCustoBaixou(), "H9b: faixa custoBaixou deve valer também para acao nova")

        linhaItem.item = { acao: "", achadoPor: "", motivoPendencia: "Código do fornecedor muito curto e sem GTIN válido" }
        verify(linhaItem.notaItem().indexOf("muito curto") >= 0,
               "H9b: item pendente deve exibir motivoPendencia real: " + linhaItem.notaItem())
    }

    function test_t111_h9_descartar_avisa_provisorias_pelo_sinal_do_nucleo() {
        telaCompraFalsa.compra = {
            id: "c-2",
            temProvisoriasDescartaveis: true,
            itens: [{ achadoPor: "fornecedor" }]
        }
        telaCompraFalsa.descartar()
        compare(janelaFalsa.confirmacoes.length, 1)
        verify(janelaFalsa.confirmacoes[0].detalhe.indexOf("Peças provisórias") >= 0,
               "H9c: deve avisar sobre peças provisórias na 2ª nota: " + janelaFalsa.confirmacoes[0].detalhe)
    }

    function test_t111_h9_tela_compra_tem_atalho_conferir_pendencias() {
        verify(%TEM_ATALHO_PENDENCIAS%, "H9a: TelaCompra.qml deve oferecer atalho para conferir pendências")
    }

    function test_t111_fiscal_notas_navega_para_compras_filtrada_ou_nota_unica() {
        Api.respostas = [{ ok: true }, { ok: true }]
        telaInicioFalsa.abrirAviso({ id: "fiscal_notas", funcao: "compras", alvo: "" })
        compare(janelaFalsa.abertasProps.length, 1)
        compare(janelaFalsa.abertasProps[0].nome, "TelaCompras")
        compare(janelaFalsa.abertasProps[0].props.filtro, "aguardando_autorizacao")

        telaInicioFalsa.abrirAviso({ id: "fiscal_notas", funcao: "compras", alvo: "cp-unica" })
        compare(janelaFalsa.abertasProps.length, 2)
        compare(janelaFalsa.abertasProps[1].nome, "TelaCompra")
        compare(janelaFalsa.abertasProps[1].props.compraId, "cp-unica")
        compare(janelaFalsa.abertasProps[1].props.abrirConferenciaAoAbrir, true)
    }

    function test_t111_linha_compra_chip_honesto_e_acao_conferir() {
        linhaCompraFalsa.item = { situacao: "rascunho", pendentes: 0, autorizacao: "aguardando", podeLancar: false }
        compare(linhaCompraFalsa.textoSituacao(), "Aguardando conferência")
        verify(linhaCompraFalsa.aguardandoConferencia, "deve oferecer botão Conferir")

        linhaCompraFalsa.item = { situacao: "rascunho", pendentes: 0, autorizacao: "negada", podeLancar: false }
        compare(linhaCompraFalsa.textoSituacao(), "Nota bloqueada")
        verify(!linhaCompraFalsa.aguardandoConferencia, "nota bloqueada não oferece Conferir")

        linhaCompraFalsa.item = { situacao: "rascunho", pendentes: 0, autorizacao: "confirmada_gerente", podeLancar: true }
        compare(linhaCompraFalsa.textoSituacao(), "Pronta para lançar")

        linhaCompraFalsa.item = { situacao: "rascunho", pendentes: 3, autorizacao: "confirmada_gerente", podeLancar: true }
        compare(linhaCompraFalsa.textoSituacao(), "3 itens a decidir")

        tela.itens = [{ id: "cp-42" }]
        tela.conferirCompra(0)
        compare(janelaFalsa.abertasProps.length, 1)
        compare(janelaFalsa.abertasProps[0].nome, "TelaCompra")
        compare(janelaFalsa.abertasProps[0].props.compraId, "cp-42")
        compare(janelaFalsa.abertasProps[0].props.abrirConferenciaAoAbrir, true)
    }

    function test_t111_tela_compra_tem_conferencia_no_cartao_e_copiar_chave() {
        verify(%TEM_ATALHO_CONFERENCIA%, "TelaCompra.qml deve oferecer conferência no cartão Para lançar")
        verify(%TEM_COPIAR_CHAVE%, "TelaCompra.qml deve oferecer botão Copiar chave no painel de conferência")
    }

    function test_d_img_1_deduplica_nota_repetida_em_dois_arquivos() {
        tela.guardarNaoImportadas([
            { arquivo: "498752-nfe.xml", numero: "803058", serie: "1", emitenteDocumento: "11", destinatarioDocumento: "22", emitenteNome: "UNIVERSAL" },
            { arquivo: "498752-nfe (1).xml", numero: "803058", serie: "1", emitenteDocumento: "11", destinatarioDocumento: "22", emitenteNome: "UNIVERSAL" }
        ])
        compare(tela.naoImportadas.length, 1, "D-IMG-1: mesma nota em dois arquivos conta uma única vez")
        compare(tela.naoImportadas[0].arquivos.length, 2, "D-IMG-1: preserva os dois arquivos agrupados")
        verify(tela.textoArquivosDaNota(tela.naoImportadas[0]).indexOf("+1 arquivo") >= 0)
        compare(conflitoCnpj.mostrados.length, 1)
    }

    function test_t054_c2_r7_duplo_clique_descartar_nao_remove_segunda_nota() {
        tela.guardarNaoImportadas([
            { arquivo: "a.xml", numero: "101", serie: "1", emitenteDocumento: "11", destinatarioDocumento: "22", emitenteNome: "A" },
            { arquivo: "b.xml", numero: "102", serie: "1", emitenteDocumento: "11", destinatarioDocumento: "22", emitenteNome: "B" }
        ])
        compare(tela.naoImportadas.length, 2)
        tela.descartarNaoImportada(tela.naoImportadas[0], false)
        compare(tela.naoImportadas.length, 1)
        // Segundo clique imediato durante a janela de 400 ms é bloqueado.
        tela.descartarNaoImportada(tela.naoImportadas[0], false)
        compare(tela.naoImportadas.length, 1, "T054-C2-R7: clique duplo rápido não pode descartar a nota seguinte")
        // Descartar vindo da janelinha (confirmado lá) continua permitido.
        tela.descartarNaoImportada(tela.naoImportadas[0], true)
        compare(tela.naoImportadas.length, 0)
    }

    function test_t110_p2_p3_depois_de_importar_sem_espaco_e_sem_nada_novo() {
        painel.depoisDeImportar({ ok: true, dados: { clientes: 0, fornecedores: 0, pecas: 0, codigosLigados: 2, compras: 1 } })
        wait(20)
        compare(janelaFalsa.avisos.length, 1, JSON.stringify(janelaFalsa.avisos))
        const txt = janelaFalsa.avisos[0].texto
        verify(txt.indexOf("Nada novo") < 0, "N5.1: não pode dizer Nada novo quando há compras: " + txt)
        verify(!txt.startsWith(" "), "T110-P3: não pode começar com espaço: '" + txt + "'")
        verify(txt.indexOf("abra em Compras") >= 0, "T110-P3: singular correto para 1 nota: " + txt)
    }

    function test_t110_p4_aviso_erro_curto_sem_abra_cada_uma() {
        Api.respostas = [{ ok: true, dados: { compras: [{ id: "a" }, { id: "b" }], erros: ["e.xml: ruim"], conflitos: [] } }]
        tela.importar(["pasta"])
        compare(janelaFalsa.avisos.length, 1)
        compare(janelaFalsa.avisos[0].tipo, "erro")
        verify(janelaFalsa.avisos[0].texto.indexOf("Abra cada uma") < 0, "T110-P4: aviso de erro mais curto: " + janelaFalsa.avisos[0].texto)
    }
}
"""


def runner():
    """O qmltestrunner do Qt 6 (o /usr/bin/qmltestrunner pode ser o do Qt 5)."""
    candidatos = []
    if os.environ.get("QT_ROOT_DIR"):
        candidatos.append(os.path.join(os.environ["QT_ROOT_DIR"], "bin", "qmltestrunner"))
    candidatos += ["/usr/lib/qt6/bin/qmltestrunner", shutil.which("qmltestrunner6") or "", shutil.which("qmltestrunner") or ""]
    for c in candidatos:
        if c and os.path.exists(c):
            return c
    sys.exit("falta o qmltestrunner do Qt 6")


def achar(padrao, texto, nome, grupo=0):
    """O trecho do QML; sem ele, o teste reprova (a tela não tem o que se confere)."""
    m = re.search(padrao, texto, re.S | re.M)
    return m.group(grupo) if m else f"/* FALTA: {nome} */ undefined"


def main():
    inicio = open(os.path.join(RAIZ, "app", "qml", "TelaInicio.qml"), encoding="utf-8").read()
    compras = open(os.path.join(RAIZ, "app", "qml", "TelaCompras.qml"), encoding="utf-8").read()
    linha_compra = open(os.path.join(RAIZ, "app", "qml", "LinhaCompra.qml"), encoding="utf-8").read()
    compra = open(os.path.join(RAIZ, "app", "qml", "TelaCompra.qml"), encoding="utf-8").read()
    linha_item = open(os.path.join(RAIZ, "app", "qml", "LinhaItemCompra.qml"), encoding="utf-8").read()
    cad = open(os.path.join(RAIZ, "app", "qml", "ImportarCadastros.qml"), encoding="utf-8").read()
    nota_bloco = achar(r"Nota \{\n\s*visible: text !== \"\"\n\s*text: \{\n(.*?)\n\s*\}\n\s*font\.pixelSize: 13", linha_item, "nota do item", 1)
    situacao_bloco = achar(r"readonly property string textoSituacao: \{\n(.*?)\n    \}\n", linha_compra, "textoSituacao", 1)
    trocas = {
        "%ABRIR_AVISO%": achar(r"^    function abrirAviso\(aviso\) \{\n.*?^    \}\n", inicio, "abrirAviso"),
        "%TEXTO_SITUACAO_COMPRA%": f"(function() {{\n{situacao_bloco}\n}})()",
        "%CONFERIR_COMPRA%": achar(r"^    function conferirCompra\(indice\) \{\n.*?^    \}\n", compras, "conferirCompra"),
        "%IMPORTAR%": achar(r"^    function importar\(arquivos\) \{\n.*?^    \}\n", compras, "importar"),
        "%CHAVE_NOTA%": achar(r"^    function chaveDaNota\(c\) \{\n.*?^    \}\n", compras, "chaveDaNota"),
        "%ARQUIVOS_NOTA%": achar(r"^    function arquivosDaNota\(c\) \{\n.*?^    \}\n", compras, "arquivosDaNota"),
        "%TEXTO_ARQUIVOS_NOTA%": achar(r"^    function textoArquivosDaNota\(c\) \{\n.*?^    \}\n", compras, "textoArquivosDaNota"),
        "%GUARDAR_NAO_IMPORTADAS%": achar(r"^    function guardarNaoImportadas\(conflitos\) \{\n.*?^    \}\n", compras, "guardarNaoImportadas"),
        "%DESCARTAR_NAO_IMPORTADA%": achar(r"^    function descartarNaoImportada\(c, daJanelinha\) \{\n.*?^    \}\n", compras, "descartarNaoImportada"),
        "%DESCARTAR%": achar(r"^    function descartar\(\) \{\n.*?^    \}\n", compra, "descartar"),
        "%NOTA_ITEM%": f"(function() {{\n{nota_bloco.replace('linha.', 'linhaItem.')}\n}})()",
        "%VIS_BAIXOU%": achar(r"// O custo baixou: o preço baixa junto\?\n\s*Rectangle \{\n\s*visible: ([^\n]+)", linha_item, "visivel custoBaixou", 1).replace("linha.", "linhaItem."),
        "%TEM_ATALHO_PENDENCIAS%": "true" if ("abrirPendencias()" in compra and "Conferir pendências" in compra) else "false",
        "%TEM_ATALHO_CONFERENCIA%": "true" if ("Conferir autorização…" in compra and "abrirConferenciaAoAbrir" in compra) else "false",
        "%TEM_COPIAR_CHAVE%": "true" if ("Copiar chave" in compra and "chaveSemEspacos.copy()" in compra) else "false",
        "%CAT%": achar(r"^    function cat\(codigo\) \{\n.*?^    \}\n", cad, "cat"),
        "%TEXTO%": achar(r"^    function texto\(n, um, varios\) \{\n.*?^    \}\n", cad, "texto"),
        "%LER%": achar(r"^    function lerArquivos\(\) \{\n.*?^    \}\n", cad, "lerArquivos"),
        "%DEPOIS_IMPORTAR%": achar(r"^    function depoisDeImportar\(r\) \{\n.*?^    \}\n", cad, "depoisDeImportar"),
        "%ABRIR_PENDENCIA%": achar(r"^    function abrirPendencia\(p\) \{\n.*?^    \}\n", cad, "abrirPendencia"),
        "%RESOLVER_NOVA%": achar(r"^    function resolverComoNova\(comCorrecao\) \{\n.*?^    \}\n", cad, "resolverComoNova"),
        "%MODELO%": achar(r"model: (painel\.previa \? \[.*?\] : \[\])\n", cad, "o modelo da prévia", 1),
        "%VISIVEL%": achar(r"delegate: Rectangle \{\n\s*id: linha\n.*?\n\s*visible: ([^\n]*)\n", cad, "visible", 1),
        "%TEXTOJA%": achar(r"text: (\[linha\.modelData\.ja > 0 \?.*?\.join\(\"  ·  \"\))", cad, "o texto do já", 1),
        "%ALTERNAR%": achar(r"onToggled: \{\n(.*?)\n\s*\}\n\s*leftPadding", cad, "onToggled", 1),
    }
    teste = TESTE
    for k, v in trocas.items():
        teste = teste.replace(k, v)
    pasta = tempfile.mkdtemp(prefix="rustinho-qml-t108-")
    try:
        mod = os.path.join(pasta, "falsos")
        os.makedirs(mod)
        with open(os.path.join(mod, "Api.qml"), "w", encoding="utf-8") as f:
            f.write(API)
        with open(os.path.join(mod, "Tema.qml"), "w", encoding="utf-8") as f:
            f.write(TEMA)
        with open(os.path.join(mod, "qmldir"), "w", encoding="utf-8") as f:
            f.write("module falsos\nsingleton Api 1.0 Api.qml\nsingleton Tema 1.0 Tema.qml\n")
        with open(os.path.join(pasta, "tst_t108.qml"), "w", encoding="utf-8") as f:
            f.write(teste)
        r = subprocess.run([runner(), "-import", pasta, "-input", os.path.join(pasta, "tst_t108.qml")],
                           env=dict(os.environ, QT_QPA_PLATFORM="offscreen"), capture_output=True, text=True,
                           errors="replace")
        print(r.stdout + r.stderr)
        sys.exit(r.returncode)
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


if __name__ == "__main__":
    main()
