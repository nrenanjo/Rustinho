#!/usr/bin/env python3
"""Verificação estática das linhas alternadas (0.9.18, V-6 / critério A9).

Percorre todas as telas e componentes (`app/qml`) e acha cada lista feita por
`delegate:` (ListView e Repeater). Cada uma está classificada abaixo:

  - LISTA: uma linha por registro (peça, pessoa, venda, conta, parcela,
    cheque, histórico, caixa da rede...). Tem de usar `Tema.faixa(` no
    delegado (ou no componente da linha, como `LinhaPeca`), para alternar
    claro/escuro nos três temas e inverter o texto no Planeta Roxo.
  - FORA: não é lista de registros (menu suspenso, sugestões de busca,
    botões e pílulas de escolha, resumo chave-valor, prévia de cálculo antes
    de gravar, avisos, gráfico, prévia de impressão) ou é linha editável de
    formulário, que espera a decisão D-V5 do dono.

Falha se uma lista nova não estiver classificada, se uma classificada sumir
ou se uma LISTA não usar `Tema.faixa(`. Não abre janela nenhuma.

    python3 app/testes/listas_alternadas.py
"""

import os
import re
import sys

RAIZ = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
QML = os.path.join(RAIZ, "app", "qml")

# Linhas editáveis de formulário: esperam a decisão D-V5 (alternam ou não).
D_V5 = "linha editável de formulário: espera a decisão D-V5"
MENU = "menu suspenso / sugestões da busca (fora pela especificação V-6)"
ESCOLHA = "botões ou pílulas de escolha, não registros"
RESUMO = "resumo chave-valor ou totais, não lista de registros"
PREVIA = "prévia de cálculo antes de gravar, não registros"
AVISOS = "avisos e mensagens"

# (arquivo, começo da expressão do model): (classe, o quê)
CLASSES = {
    ("AjusteEstoque.qml", '["Contagem"'): ("FORA", ESCOLHA),
    ("Aparencia.qml", '[{ nome: "sol"'): ("FORA", ESCOLHA + " (os três temas)"),
    ("Campo.qml", "campo.filtradas"): ("FORA", MENU),
    ("ClassificarCategorias.qml", "janelinha.c ? janelinha.c.categorias"): ("LISTA", "categorias da DRE"),
    ("ConflitoCnpj.qml", "{ const c"): ("FORA", RESUMO + " (dados da nota)"),
    ("ConflitoCnpj.qml", "janelaConflito.conflito ? janelaConflito.conflito.itens"): ("LISTA", "itens da nota em conflito"),
    ("DetalheDre.qml", "janelinha.d ? (janelinha.d.itens"): ("LISTA", "lançamentos de uma linha da DRE"),
    ("EmissaoFiscal.qml", '["NFC-e'): ("FORA", ESCOLHA),
    ("EscolherPeca.qml", "janela.itens"): ("LISTA", "peças (LinhaPeca)"),
    ("EscolherUnidade.qml", "escolher.lista"): ("FORA", ESCOLHA),
    ("Etapas.qml", "etapas.modelo"): ("FORA", ESCOLHA + " (etapas da ordem)"),
    ("GraficoBarras.qml", "grafico.barras"): ("FORA", "gráfico"),
    ("ImportarCadastros.qml", "painel.previa ? ["): ("FORA", PREVIA + " (contagem por tipo)"),
    ("ImportarCadastros.qml", "painel.previa ? painel.previa.avisos"): ("FORA", AVISOS),
    ("MetasDre.qml", "janelinha.metas"): ("LISTA", "metas"),
    ("Notificacao.qml", "(raiz.aviso.detalhes"): ("FORA", RESUMO + " (detalhes da notificação)"),
    ("PainelCheques.qml", "painel.calc ? painel.calc.cheques"): ("FORA", PREVIA + " (cheques da conta, em linha)"),
    ("Pilulas.qml", "pilulas.modelo"): ("FORA", ESCOLHA),
    ("SecaoBackup.qml", "!secao.b ? [] : ["): ("FORA", RESUMO + " (situação do backup)"),
    ("SecaoBackup.qml", "{ const lista"): ("LISTA", "backups guardados"),
    ("SecaoCheques.qml", "secao.painel ? secao.painel.cheques"): ("LISTA", "cheques"),
    ("SecaoClientes.qml", "secao.dados ? secao.dados.itens"): ("LISTA", "clientes (indicadores)"),
    ("SecaoComissoes.qml", "secao.dados ? secao.dados.vendedores"): ("LISTA", "comissões por vendedor"),
    ("SecaoComissoes.qml", "secao.dados ? secao.dados.fechamentos"): ("LISTA", "fechamentos de comissão"),
    ("SecaoComprar.qml", "secao.linhas"): ("LISTA", "peças a comprar (componente cPeca)"),
    ("SecaoContas.qml", "secao.contas"): ("LISTA", "contas (dinheiro e banco)"),
    ("SecaoContas.qml", "secao.extrato ? secao.extrato.linhas"): ("LISTA", "extrato da conta"),
    ("SecaoDre.qml", "secao.d ? secao.d.linhas"): ("LISTA", "linhas da DRE"),
    ("SecaoDre.qml", "secao.d ? secao.d.pendencias"): ("FORA", AVISOS + " (pendências da DRE)"),
    ("SecaoDre.qml", "secao.d ? secao.d.variacoes"): ("FORA", RESUMO + " (variação dos indicadores)"),
    ("SecaoDre.qml", "!secao.d ? [] : ["): ("FORA", RESUMO),
    ("SecaoDre.qml", "secao.d ? secao.d.avisos"): ("FORA", AVISOS),
    ("SecaoDre.qml", "secao.serie.slice().reverse()"): ("LISTA", "meses (série)"),
    ("SecaoDre.qml", "secao.desp ? secao.desp.categorias"): ("LISTA", "despesas por categoria (LinhaRanking)"),
    ("SecaoEncomendas.qml", "secao.painel ? secao.painel.encomendas"): ("LISTA", "encomendas"),
    ("SecaoEncomendas.qml", "nova_.achados"): ("FORA", MENU),
    ("SecaoFluxo.qml", "secao.fluxo ? secao.fluxo.realizado"): ("LISTA", "fluxo realizado (LinhaFluxo)"),
    ("SecaoFluxo.qml", "secao.fluxo ? secao.fluxo.previsto"): ("LISTA", "fluxo previsto (LinhaFluxo)"),
    ("SecaoFornecedores.qml", "secao.dados ? secao.dados.itens"): ("LISTA", "fornecedores (indicadores)"),
    ("SecaoIndicadores.qml", "["): ("FORA", "cartões dos rankings (cada um é um painel)"),
    ("SecaoIndicadores.qml", "secao.r ? secao.linhas("): ("LISTA", "rankings (LinhaRanking)"),
    ("SecaoIndicadores.qml", '["Vendedor"'): ("FORA", "cabeçalho das colunas"),
    ("SecaoIndicadores.qml", "secao.r ? secao.r.vendedores"): ("LISTA", "vendedores"),
    ("SecaoIndicadores.qml", "v.colunas"): ("FORA", "células de uma linha (a linha alterna)"),
    ("SecaoPagamentos.qml", "secao.dados ? secao.dados.itens"): ("LISTA", "formas de pagamento (LinhaRanking)"),
    ("SecaoPagar.qml", "secao.painel ? secao.painel.parcelas"): ("LISTA", "contas a pagar (LinhaContaPagar)"),
    ("SecaoPagar.qml", "lancamento.fornecedor ? [] : lancamento.achados"): ("FORA", MENU),
    ("SecaoPagar.qml", "lancamento.previa"): ("FORA", PREVIA),
    ("SecaoPagar.qml", "secao.opcoes.formas"): ("FORA", ESCOLHA),
    ("SecaoPagar.qml", "ficha.dados ? ficha.dados.pagamentos"): ("LISTA", "pagamentos de uma conta"),
    ("SecaoPagar.qml", "categorias.itens"): ("FORA", D_V5 + " (categorias)"),
    ("SecaoParados.qml", "secao.dados ? secao.dados.itens"): ("LISTA", "peças paradas"),
    ("SecaoResumo.qml", "secao.dre ? secao.dre.avisos"): ("FORA", AVISOS),
    ("SecaoResumo.qml", "secao.r ? ["): ("FORA", "cartões de resumo das contas"),
    ("SecaoResumo.qml", "secao.r ? secao.r.formas"): ("LISTA", "formas de pagamento (LinhaRanking)"),
    ("SecaoResumo.qml", "secao.r ? secao.r.maisVendidos"): ("LISTA", "mais vendidos (LinhaRanking)"),
    ("SecaoResumo.qml", "secao.r ? secao.r.vendedores"): ("LISTA", "vendedores (LinhaRanking)"),
    ("SecaoVendidos.qml", "secao.dados ? secao.dados.itens"): ("LISTA", "peças vendidas"),
    ("Selecao.qml", ""): ("FORA", MENU),
    ("TelaCaixa.qml", '!tela.r ? [] : [["Troco inicial"'): ("FORA", RESUMO),
    ("TelaCaixa.qml", "tela.r ? tela.r.movimentos"): ("LISTA", "sangrias e suprimentos"),
    ("TelaCaixa.qml", "tela.estado ? tela.estado.historico"): ("LISTA", "caixas anteriores"),
    ("TelaCaixa.qml", "tela.vendas"): ("LISTA", "vendas do caixa (LinhaVenda)"),
    ("TelaCompra.qml", ""): ("FORA", D_V5 + " (LinhaItemCompra)"),
    ("TelaCompra.qml", "tela.compra ? tela.compra.duplicatas"): ("LISTA", "duplicatas da nota"),
    ("TelaCompra.qml", "tela.compra ? ["): ("FORA", RESUMO),
    ("TelaCompras.qml", "tela.naoImportadas"): ("LISTA", "notas esperando"),
    ("TelaCompras.qml", "tela.itens"): ("LISTA", "compras (LinhaCompra)"),
    ("TelaConfiguracoes.qml", ""): ("FORA", D_V5 + " (tabelas de preço)"),
    ("TelaConfiguracoes.qml", "tela.sync ? tela.sync.pares"): ("LISTA", "caixas da rede"),
    ("TelaConfiguracoes.qml", "tela.sync && tela.sync.referencias"): ("LISTA", "registros incompletos"),
    ("TelaConfiguracoes.qml", "tela.caixasConhecidos"): ("LISTA", "caixas conhecidos"),
    ("TelaConfiguracoes.qml", "tela.verSimultaneidade ? tela.pendencias"): ("LISTA", "pendências da troca de dados"),
    ("TelaConfiguracoes.qml", "tela.gerente ? tela.gerente.usuarios"): ("FORA", ESCOLHA + " (caixas de marcar)"),
    ("TelaConfiguracoes.qml", "tela.gerente ? tela.gerente.acoes"): ("FORA", ESCOLHA + " (caixas de marcar)"),
    ("TelaConfiguracoes.qml", "tela.conflitos"): ("LISTA", "duplicatas entre caixas"),
    ("TelaConfiguracoes.qml", "modelData.campos"): ("FORA", "campos de um conflito (o cartão alterna)"),
    ("TelaConfiguracoes.qml", "tela.codigos ? tela.codigos.esperando"): ("LISTA", "códigos de peça esperando"),
    ("TelaConfiguracoes.qml", "tela.codigos ? tela.codigos.cruzadas"): ("FORA", AVISOS),
    ("TelaConfiguracoes.qml", "tela.codigos ? tela.codigos.repetidos"): ("LISTA", "códigos de peça repetidos"),
    ("TelaConfiguracoes.qml", "grupo.modelData.pecas"): ("FORA", "peças de um grupo (o grupo alterna)"),
    ("TelaConfiguracoes.qml", "tela.codigos ? tela.codigos.pessoasEsperando"): ("LISTA", "códigos de pessoa esperando"),
    ("TelaConfiguracoes.qml", "tela.codigos ? tela.codigos.pessoasRepetidas"): ("LISTA", "códigos de pessoa repetidos"),
    ("TelaConfiguracoes.qml", "grupoPessoas.modelData.pessoas"): ("FORA", "pessoas de um grupo (o grupo alterna)"),
    ("TelaConfiguracoes.qml", "tela.historicoTroca ? tela.historicoTroca.caixas"): ("LISTA", "histórico da troca por caixa"),
    ("TelaConfiguracoes.qml", "tela.historicoTroca ? tela.historicoTroca.encerradas"): ("LISTA", "pendências encerradas"),
    ("TelaConfiguracoes.qml", "{ const f = filtroAuditoria"): ("LISTA", "auditoria: quem fez o quê"),
    ("TelaDevolucao.qml", "tela.venda ? tela.venda.itens"): ("LISTA", "itens da venda"),
    ("TelaDevolucao.qml", "tela.calc && tela.calc.partes"): ("FORA", PREVIA),
    ("TelaDevolucao.qml", "tela.motivos"): ("FORA", ESCOLHA),
    ("TelaEntrar.qml", "tela.usuarios"): ("LISTA", "usuários na entrada"),
    ("TelaEtiquetas.qml", "tela.itens"): ("LISTA", "peças das etiquetas"),
    ("TelaEtiquetas.qml", "tela.bloqueadas"): ("LISTA", "peças que não saem agora (caso 4 da navegação)"),
    ("TelaEtiquetas.qml", "tela.m ? tela.porFolha"): ("FORA", "prévia de impressão (papel branco)"),
    ("TelaEtiquetas.qml", "previa.e && tela.comBarras"): ("FORA", "prévia de impressão (papel branco)"),
    ("TelaFiscal.qml", "listaPecas.tudo ? listaPecas.pecas"): ("LISTA", "peças da conferência fiscal"),
    ("TelaFiscal.qml", "regras"): ("FORA", D_V5 + " (LinhaRegraSt)"),
    ("TelaInicio.qml", "tela.faixas"): ("FORA", "notificações"),
    ("TelaInicio.qml", "tela.quadrados"): ("FORA", "notificações"),
    ("TelaInicio.qml", "tela.menu.todos"): ("FORA", "cartões da Início: xadrez próprio (V-7)"),
    ("TelaJuntar.qml", "tela.sync ? tela.sync.pares"): ("LISTA", "caixas achados na rede"),
    ("TelaOrcamentos.qml", "tela.itens"): ("LISTA", "orçamentos (LinhaVenda)"),
    ("TelaOrdem.qml", "tela.achados"): ("FORA", MENU),
    ("TelaOrdem.qml", '[["Hoje"'): ("FORA", ESCOLHA),
    ("TelaOrdem.qml", "tela.itens.length"): ("LISTA", "peças e serviços da ordem"),
    ("TelaOrdem.qml", "tela.servicosComuns"): ("FORA", ESCOLHA),
    ("TelaOrdens.qml", "tela.itens"): ("LISTA", "ordens de serviço"),
    ("TelaPeca.qml", "tela.temGrade"): ("FORA", ESCOLHA + " (grade)"),
    ("TelaPeca.qml", "aplicacoes"): ("FORA", D_V5 + " (LinhaAplicacao)"),
    ("TelaPeca.qml", "equivalentes"): ("FORA", D_V5 + " (LinhaEquivalente)"),
    ("TelaPeca.qml", "tela.precos"): ("FORA", D_V5 + " (LinhaPreco)"),
    ("TelaPeca.qml", "tela.ficha && tela.ficha.fornecedores"): ("LISTA", "códigos nos fornecedores"),
    ("TelaPeca.qml", "tela.ficha ? tela.ficha.historico"): ("LISTA", "histórico da peça"),
    ("TelaPeca.qml", "modelData.detalhes"): ("FORA", "detalhes de um item do histórico (a linha alterna)"),
    ("TelaPecas.qml", "tela.itens"): ("LISTA", "peças (LinhaPeca)"),
    ("TelaPessoa.qml", "veiculos"): ("FORA", D_V5 + " (LinhaVeiculo)"),
    ("TelaPessoa.qml", "tela.ficha ? tela.ficha.historico"): ("LISTA", "histórico da pessoa"),
    ("TelaPessoa.qml", "modelData.detalhes"): ("FORA", "detalhes de um item do histórico (a linha alterna)"),
    ("TelaPessoas.qml", "tela.itens"): ("LISTA", "pessoas (LinhaPessoa)"),
    ("TelaReceber.qml", "["): ("FORA", "cartões de resumo"),
    ("TelaReceber.qml", "tela.painel ? tela.painel.clientes"): ("LISTA", "quem deve (LinhaDevedor)"),
    ("TelaReceber.qml", "tela.conta ? tela.conta.parcelas"): ("LISTA", "parcelas (LinhaParcela)"),
    ("TelaReceber.qml", "tela.conta ? tela.conta.recebimentos"): ("LISTA", "recebimentos"),
    ("TelaReceber.qml", "tela.formas.filter("): ("FORA", ESCOLHA),
    ("TelaReceber.qml", "pagar.calc ? pagar.calc.abates"): ("FORA", PREVIA),
    ("TelaReceber.qml", "lancar.cliente ? [] : lancar.achados"): ("FORA", MENU),
    ("TelaReceber.qml", "lancar.previa"): ("FORA", PREVIA),
    ("TelaTrocas.qml", "tela.vendas"): ("LISTA", "vendas (LinhaVenda)"),
    ("TelaTrocas.qml", "tela.vales"): ("LISTA", "vales-troca"),
    ("TelaTrocas.qml", "tela.feitas"): ("LISTA", "trocas feitas"),
    ("TelaUsuarios.qml", "tela.usuarios"): ("LISTA", "usuários"),
    ("TelaVenda.qml", "tela.v ? tela.v.itens"): ("LISTA", "itens da venda"),
    ("TelaVenda.qml", '!tela.v ? [] : [["Subtotal"'): ("FORA", RESUMO),
    ("TelaVenda.qml", "tela.v ? tela.v.parcelas"): ("LISTA", "parcelas da venda"),
    ("TelaVenda.qml", "tela.v ? tela.v.cheques"): ("LISTA", "cheques da venda"),
    ("TelaVender.qml", "tela.calculo ? tela.calculo.itens"): ("LISTA", "carrinho (LinhaCarrinho)"),
    ("TelaVender.qml", "tela.resultados"): ("LISTA", "peças achadas na busca"),
    ("TelaVender.qml", "tela.clientesAchados"): ("FORA", MENU),
    ("TelaVender.qml", "tela.calculo ? ["): ("FORA", RESUMO),
    ("TelaVender.qml", "tela.formas"): ("FORA", D_V5 + " (formas de pagamento)"),
    ("TelaVender.qml", "painelCred.cred ? painelCred.cred.parcelas"): ("FORA", PREVIA + " (crediário em linha)"),
    ("TelaVender.qml", "painelCred.cred ? painelCred.cred.avisos"): ("FORA", AVISOS),
}

# Classificações que valem para mais de um delegado do mesmo arquivo, de propósito.
REPETIDAS = set()

# Os componentes de linha (Linha*.qml): os de lista alternam; os de formulário esperam a D-V5.
LINHAS = {
    "LinhaCarrinho.qml": "LISTA", "LinhaCompra.qml": "LISTA", "LinhaContaPagar.qml": "LISTA",
    "LinhaDevedor.qml": "LISTA", "LinhaFluxo.qml": "LISTA", "LinhaParcela.qml": "LISTA",
    "LinhaPeca.qml": "LISTA", "LinhaPessoa.qml": "LISTA", "LinhaRanking.qml": "LISTA", "LinhaVenda.qml": "LISTA",
    "LinhaAplicacao.qml": "FORA", "LinhaEquivalente.qml": "FORA", "LinhaItemCompra.qml": "FORA",
    "LinhaPreco.qml": "FORA", "LinhaRegraSt.qml": "FORA", "LinhaVeiculo.qml": "FORA",
}


def delegados():
    """[(arquivo, tipo, model, linha, bloco)] de cada `delegate:`."""
    achados = []
    for arq in sorted(os.listdir(QML)):
        if not arq.endswith(".qml"):
            continue
        linhas = open(os.path.join(QML, arq), encoding="utf-8").read().split("\n")
        for i, l in enumerate(linhas):
            m = re.match(r"^\s*delegate:\s*(\w+)", l)
            if not m:
                continue
            modelo = ""
            for j in range(i - 1, max(-1, i - 16), -1):
                mm = re.search(r"\bmodel:\s*(.*)", linhas[j])
                if mm:
                    modelo = mm.group(1).strip()
                    # model em várias linhas ("{" ou "["): junta a linha seguinte,
                    # para duas listas assim no mesmo arquivo não se confundirem.
                    if modelo in ("{", "["):
                        modelo += " " + linhas[j + 1].strip()
                    break
            prof, bloco = 0, []
            for k in range(i, len(linhas)):
                s = linhas[k].split("//")[0]
                bloco.append(linhas[k])
                prof += s.count("{") - s.count("}")
                if prof <= 0 and k > i:
                    break
            achados.append((arq, m.group(1), modelo, i + 1, "\n".join(bloco)))
    return achados


def classe(arq, modelo):
    candidatos = [(k, v) for k, v in CLASSES.items() if k[0] == arq and modelo.startswith(k[1])]
    # o começo mais comprido ganha (ex.: "{" × "tela.codigos ? ...")
    candidatos.sort(key=lambda kv: len(kv[0][1]), reverse=True)
    return candidatos[0] if candidatos else (None, None)


def main():
    erros = []
    usados = set()
    quantos = {}
    listas, fora = [], []
    for arq, tipo, modelo, linha, bloco in delegados():
        chave, c = classe(arq, modelo)
        if c is None:
            erros.append(f"{arq}:{linha} ({tipo}, model: {modelo[:60]}) não está classificada")
            continue
        usados.add(chave)
        quantos[chave] = quantos.get(chave, 0) + 1
        if quantos[chave] > 1 and chave not in REPETIDAS:
            erros.append(f"{arq}:{linha}: a classificação {chave} já valeu para outro delegado do arquivo (deixe-a mais exata)")
        if c[0] == "FORA":
            fora.append((arq, linha, c[1]))
            continue
        # A lista alterna: no delegado, no componente da linha ou (Loader) no arquivo.
        texto = bloco
        if os.path.exists(os.path.join(QML, tipo + ".qml")):
            texto += open(os.path.join(QML, tipo + ".qml"), encoding="utf-8").read()
        if tipo == "Loader":
            texto += open(os.path.join(QML, arq), encoding="utf-8").read()
        ok = "Tema.faixa(" in texto
        listas.append((arq, linha, c[1], ok))
        if not ok:
            erros.append(f"{arq}:{linha} ({c[1]}) é lista de registros e não usa Tema.faixa(")
    for chave in CLASSES:
        if chave not in usados:
            erros.append(f"classificada mas não achada: {chave[0]} model: {chave[1]}")
    linhas_ok = []
    for arq, c in sorted(LINHAS.items()):
        texto = open(os.path.join(QML, arq), encoding="utf-8").read()
        usa = "Tema.faixa(" in texto
        linhas_ok.append((arq, c, usa))
        if c == "LISTA" and not usa:
            erros.append(f"{arq}: componente de lista sem Tema.faixa(")
        if c == "FORA" and usa:
            erros.append(f"{arq}: linha de formulário (D-V5) mudou sem a decisão")

    print("LISTAS DE REGISTROS (alternam com Tema.faixa):")
    for arq, linha, oque, ok in listas:
        print(f"  {'✔' if ok else '✘'} {arq}:{linha}  {oque}")
    print("COMPONENTES Linha*.qml:")
    for arq, c, usa in linhas_ok:
        print(f"  {'✔' if (c == 'LISTA') == usa else '✘'} {arq}  {'alterna' if usa else 'fora (D-V5)'}")
    print("FORA (não são listas de registros, ou esperam a D-V5):")
    for arq, linha, motivo in fora:
        print(f"  · {arq}:{linha}  {motivo}")
    n_ok = sum(1 for x in listas if x[3])
    print(f"\n{len(listas)} listas de registros, {n_ok} com Tema.faixa; "
          f"{sum(1 for x in linhas_ok if x[1] == 'LISTA' and x[2])} de {sum(1 for x in linhas_ok if x[1] == 'LISTA')} componentes Linha* de lista; "
          f"{len(fora)} delegados fora; {len(erros)} erro(s).")
    for e in erros:
        print("✘", e)
    sys.exit(1 if erros else 0)


if __name__ == "__main__":
    main()
