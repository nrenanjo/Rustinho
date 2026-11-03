# BLOCO 9A2 e 9A2 COMPLEMENTAR — Telas Desktop de Estoque, Vendas, Caixa, Trocas e Orçamentos

**Data da Auditoria:** 2026-11-02  
**Escopo:** `app/qml/AjusteEstoque.qml` (122 linhas), `app/qml/LinhaVenda.qml` (103 linhas), `app/qml/LinhaCarrinho.qml` (211 linhas), `app/qml/TelaCaixa.qml` (414 linhas), `app/qml/TelaDevolucao.qml` (468 linhas), `app/qml/TelaEstoque.qml` (93 linhas), `app/qml/TelaVenda.qml` (330 linhas), `app/qml/SecaoComprar.qml` (364 linhas), `app/qml/SecaoEncomendas.qml` (482 linhas), `app/qml/SecaoParados.qml` (217 linhas), `app/qml/SecaoVendidos.qml` (245 linhas), `app/qml/TelaTrocas.qml` (362 linhas), `app/qml/TelaVender.qml` (1.401 linhas), `app/qml/TelaOrcamentos.qml` (123 linhas) + verificação cruzada em `app/qml/PerguntaMotivo.qml` (66 linhas)  
**Total de código lido integralmente neste bloco:** 4.835 linhas (14 arquivos do escopo principal + `PerguntaMotivo.qml` como referência complementar).

---

## 1. Mapa Estrutural dos 14 Componentes QML Auditados

| Arquivo (`0.9.17`) | Linhas | Funções, Propriedades e Subcomponentes Principais (Linhas Exatas) | Papel Confirmado na Interface |
|---|---:|---|---|
| `app/qml/AjusteEstoque.qml` | 122 | `Popup { id: ajuste }` (`8–27`), `abrirCom(id, atual, unidadePeca)` (`28–38`), `confirmar()` (`40–44`), `depoisDeAjustar(r)` (`45–56`), `Campo { id: quantidade }` (`66–76`), `Campo { id: motivo }` (`77–84`), motivos rápidos (`85–103`). | Modal de acerto de estoque por contagem física em `TelaPeca.qml`, exigindo senha do gerente via `chamarGerente("estoque_ajustar", ...)`. |
| `app/qml/LinhaVenda.qml` | 103 | `Rectangle { id: linha }` (`6–24`), `RowLayout` (`25–99`): número (`32–39`), data/hora (`40–47`), cliente + metadados (`48–71`), chip de situação (`72–89`) e total (`90–98`). | Linha reutilizada nas listas de vendas e orçamentos (`TelaCaixa.qml`, `TelaTrocas.qml`, `TelaOrcamentos.qml`). |
| `app/qml/LinhaCarrinho.qml` | 211 | `Rectangle { id: linha }` (`6–37`), `focarQuantidade()` (`29–31`), cabeçalho e aviso de falta/encomenda (`58–104`), `Campo { id: qtd }` (`125–146`), `Campo { id: preco }` (`148–166`), `Campo { id: desc }` (`168–186`), subtotal e botão remover (`188–206`). | Linha editável do carrinho em `TelaVender.qml`, com edição direta de quantidade, preço unitário, desconto individual e opção `"Tirar do estoque"` quando falta saldo. |
| `app/qml/TelaCaixa.qml` | 414 | `carregar()` (`20–35`), `imprimir(id)` (`37–41`), cabeçalho (`54–85`), faixas `"Abrir o caixa"` (`115–148`), `"Dinheiro na gaveta"` (`150–168`), `"Hoje neste caixa"` (`170–204`), `"Sangrias e suprimentos"` (`206–222`), `"Caixas anteriores"` (`224–251`), `ListView` de vendas (`256–286`), modais `movimento` (`291–356`) e `fechar` (`359–411`). | Gestão da sessão de caixa do computador local (`"caixa_estado"`, `"caixa_abrir"`, `"caixa_movimentar"`, `"caixa_fechar"`). |
| `app/qml/TelaDevolucao.qml` | 468 | `carregar()` (`27–37`), `entrada()` (`39–44`), `recalcular()` (`46–52`), `mudar()` (`54–60`), `tudo()` (`62–74`), `devolver(trocar)` (`76–95`), `ListView { id: lista }` com `Marcar { id: defeito }` e `Campo { id: quantidade }` (`169–258`), coluna `"Como devolver"` (`267–436`), `Painel { id: pronto }` (`441–465`). | Devolução parcial ou total de itens de uma venda (`"devolucao_venda"`, `"devolucao_calcular"`, `"devolucao_gravar"`) e encaminhamento para troca imediata em `TelaVender.qml`. |
| `app/qml/TelaEstoque.qml` | 93 | `aoEsc()` (`14–18`), `imprimirLista()` (`20–26`), `Pilulas` `"Estoque"` / `"Fornecedores"` (`57–61`), subpílulas `"comprar"`, `"encomendas"`, `"vendidos"`, `"parados"` (`63–73`) e `Loader { id: conteudo }` (`75–86`). | Contenedor das quatro seções gerenciais de estoque e reposição. |
| `app/qml/TelaVenda.qml` | 330 | `carregar()` (`16–24`), `imprimir()` (`26–32`), `cancelar()` (`34–38`), cabeçalho (`47–78`), banner de situação (`80–101`), `ListView` de itens (`116–188`), faixa `"Total"` com pagamentos, parcelas e cheques (`190–276`), `Popup { id: caixaCancelar }` (`280–327`). | Inspeção detalhada de venda ou orçamento (`"venda_abrir"`), cancelamento com senha de gerente (`"venda_cancelar"`), impressão e transição para devolução ou venda. |
| `app/qml/SecaoComprar.qml` | 364 | `carregar()` (`31–41`), `salvarDias()` (`42–49`), `pedidas(ids)` (`50–58`), 4 `CartaoNumero` (`78–105`), filtro + `Campo { id: dias }` (`111–138`), `ListView` polimórfica com `cTitulo`, `cEncomenda` e `cPeca` (`141–358`). | Sugestão de compras e reposição (`"estoque_sugestao"`, `"estoque_dias"`, `"encomenda_pedir"`). |
| `app/qml/SecaoEncomendas.qml` | 482 | `carregar()` (`15–22`), `nova()` (`23–25`), `acao()` (`27–37`), `avisar(e)` (`38–46`), 3 `CartaoNumero` (`66–87`), `ListView` com ações do ciclo de vida (`113–296`), `Painel { id: nova_ }` (`300–469`), `EscolherPeca` (`471–474`), `PerguntaMotivo` (`476`). | Gestão completa de encomendas de clientes (`"encomendas_painel"`, `"encomenda_criar"`, `"encomenda_pedir"`, `"encomenda_chegou"`, `"encomenda_entregar"`, `"encomenda_whatsapp"`, `"encomenda_cancelar"`). |
| `app/qml/SecaoParados.qml` | 217 | `carregar()` (`15–25`), `salvarDias()` (`26–33`), 3 `CartaoNumero` (`53–75`), filtro + `Campo { id: dias }` (`81–108`), `ListView` (`111–205`). | Relatório de capital imobilizado e peças sem giro (`"estoque_parados"`, `"estoque_dias"`). |
| `app/qml/SecaoVendidos.qml` | 245 | `carregar()` (`16–23`), 4 `CartaoNumero` (`43–67`), filtro + pílulas de período (`30`/`90`/`365`) e critério (`"valor"`/`"quantidade"`) (`70–100`), `ListView` com curva ABC (`102–233`). | Ranking de produtos mais vendidos e Curva ABC (`"estoque_mais_vendidos"`). |
| `app/qml/TelaTrocas.qml` | 362 | `carregar()` (`17–32`), `imprimir(tipo, id)` (`34–40`), pílulas `"devolver"`, `"vales"`, `"feitas"` (`78–100`), `listaVendas` (`105–140`), `listaVales` (`143–241`), `listaFeitas` com botão `"Desfazer"` (`244–346`), `PerguntaMotivo` (`349–355`). | Central de trocas, vales-troca abertos e desfazimento de devoluções (`"devolucao_desfazer"`). |
| `app/qml/TelaVender.qml` | 1.401 | `entrada()` (`53–70`), `recalcular()` (`72–82`), `adicionar()` (`96–112`), `confirmarBusca()` (`123–156`), `limparVenda()` (`207–226`), `abrirReceber()` (`286–307`), `concluir()` (`309–322`), `salvarOrcamento()` (`324–337`), `Popup { id: receber }` (`877–1206`), `Popup { id: concluida }` (`1209–1273`), `Popup { id: caixaOrcamento }` (`1276–1330`), `StackView.onActivated` (`1332–1400`). | Frente de caixa (PDV) completa: carrinho, busca com multiplicador `N*`, descontos, cliente, recebimento multimodo, orçamento e conversão de OS/Encomenda/Orçamento. |
| `app/qml/TelaOrcamentos.qml` | 123 | `carregar()` (`16–23`), cabeçalho com `"Novo no Vender"` (`39–51`), pílulas `"abertos"`, `"vencidos"`, `"todos"` + `Campo { id: busca }` (`56–77`), `ListView` com `LinhaVenda { orcamento: true }` (`80–115`). | Listagem e busca de orçamentos (`"orcamentos_listar"`) com abertura de `TelaVenda.qml`. |

---

## 2. Revisão Independente dos Achados Históricos (`B9A2-01` a `B9A2-05` e `B9A2C-01` a `B9A2C-07`)

### 2.1. `[B9A2-01]` Clicar no chip `"Veio com defeito"` antes de preencher a quantidade (ou antes de clicar em `"Devolver tudo"`) não marca `defeito = true` nos itens da devolução
- **Status da Revisão Independente:** **CONFIRMADO**
- **Classificação:** **Defeito comprovado**
- **Evidência no código-fonte (`app/qml/TelaDevolucao.qml:54–74, 385–400`):**
  ```qml
  function mudar(itemId, campo, valor) {
      const e = Object.assign({}, escolha)
      const atual = e[itemId] || { quantidade: "", defeito: false }
      atual[campo] = valor
      e[itemId] = atual
      escolha = e
      espera.restart()
  }
  function tudo() {
      if (!venda) return
      const e = {}
      for (const it of venda.itens) {
          if (it.pode !== "0")
              e[it.vendaItemId] = { quantidade: it.pode, defeito: (escolha[it.vendaItemId] || {}).defeito || false }
      }
      escolha = e
      recalcular()
  }
  ```
  E no `TapHandler` dos motivos rápidos (`linhas 388–399`):
  ```qml
  onTapped: {
      motivo.texto = modelData
      if (modelData === "Veio com defeito") {
          const e = Object.assign({}, tela.escolha)
          for (const id in e)
              e[id] = Object.assign({}, e[id], { defeito: true })
          tela.escolha = e
      }
  }
  ```
  - Se o operador clicar em `"Veio com defeito"` antes de digitar a quantidade (quando `tela.escolha` ainda é `{}`) ou clicar em `"Devolver tudo"` depois, os itens entram em `tela.escolha` com `defeito: false`, fazendo a peça defeituosa voltar ao estoque bom (`volta_estoque = 1`). Além disso, o `onTapped` da linha `388` não chama `tela.recalcular()`.

### 2.2. `[B9A2-02]` Cancelar o modal `PedirGerente` exibe o erro `"Só o gerente pode: ..."` como erro de validação em `quantidade.erro` (`AjusteEstoque.qml`) ou `motivo.erro` (`TelaVenda.qml`)
- **Status da Revisão Independente:** **CONFIRMADO**
- **Classificação:** **Defeito comprovado**
- **Evidência no código-fonte (`app/qml/AjusteEstoque.qml:45–56` e `app/qml/TelaVenda.qml:312–322`):**
  - Em `AjusteEstoque.qml:49–55`:
    ```qml
    if (r.campo === "motivo") {
        motivo.erro = r.erro
        motivo.focar()
    } else {
        quantidade.erro = r.erro
        quantidade.focar()
    }
    ```
  - Em `TelaVenda.qml:318–320`:
    ```qml
    } else {
        motivo.erro = r.erro
    }
    ```
  - Quando um operador `Caixa` cancela o modal `PedirGerente`, `Main.chamarGerente` devolve a resposta original recusada com `r.campo === "gerente"` e `r.erro === "Só o gerente pode: ..."`, pintando indevidamente o campo de quantidade ou motivo como inválido.

### 2.3. `[B9A2-03]` `movimento.abrirCom` em `TelaCaixa.qml` pré-seleciona a 1ª conta da loja (`contaMov.indice = 0`) em toda Sangria/Suprimento e atribui erros de `x.campo === "conta"` a `valorMov.erro`
- **Status da Revisão Independente:** **CONFIRMADO**
- **Classificação:** **Defeito comprovado**
- **Evidência no código-fonte (`app/qml/TelaCaixa.qml:310–312, 326–331, 347–351`):**
  ```qml
  valorMov.erro = ""
  motivoMov.erro = ""
  contaMov.indice = 0
  ```
  ```qml
  modelo: movimento.contas.map(c => c.nome + " (R$ " + c.saldo + ")").concat(["Nenhuma conta (saiu da loja)"])
  ```
  ```qml
  if (x.campo === "motivo") {
      motivoMov.erro = x.erro
  } else {
      valorMov.erro = x.erro
  }
  ```
  - Como `"Nenhuma conta (saiu da loja)"` fica no final do `concat`, `contaMov.indice = 0` seleciona silenciosamente a 1ª conta financeira da loja em toda sangria/suprimento; além disso, qualquer erro com `x.campo === "conta"` cai no `else` e é atribuído a `valorMov.erro`.

### 2.4. `[B9A2-04]` `TelaVenda.qml` não indica se a venda concluída já sofreu devoluções parciais ou totais
- **Status da Revisão Independente:** **CONFIRMADO**
- **Classificação:** **Risco** (Omissão visual de histórico de devolução na ficha da venda)
- **Evidência no código-fonte (`app/qml/TelaVenda.qml:1–330` e `nucleo/src/vendas.rs:880–970`):**
  - Ao abrir uma venda concluída que já teve itens devolvidos, `TelaVenda.qml` exibe apenas a quantidade e o total originais da venda, sem sinalizar que houve devolução parcial ou total (informação só visível ao clicar em `"Devolver ou trocar"` para abrir `TelaDevolucao.qml:218`).

### 2.5. `[B9A2-05]` Melhorias em `TelaVenda.qml`, `TelaCaixa.qml` e `TelaDevolucao.qml`
- **Status da Revisão Independente:** **CONFIRMADO**
- **Classificação:** **Melhoria**
- **Evidência no código-fonte:**
  1. `TelaVenda.qml:34–38, 313`: `chamarGerente("venda_cancelar", ...)` não passa chave pré-gerada como 4º argumento, gerando nova chave a cada clique em `"Confirmar cancelamento"`;
  2. `TelaCaixa.qml:158, 225`: `esperado` negativo em `"Dinheiro na gaveta"` não recebe destaque em `Tema.vermelho`, e a faixa `"Caixas anteriores"` (`visible: !tela.aberto && ...`) fica oculta enquanto o caixa atual está aberto;
  3. `TelaDevolucao.qml:151` e `TelaVenda.qml:105`: expressão `visible: Tema.estreito && (true)` contém `&& (true)` redundante.

### 2.6. `[B9A2C-01]` Botão `"Vale-troca"` (`completarVale()`) no modal `receber` de `TelaVender.qml` lê `calc.falta` (`"0,00"`) e preenche `campoVale.texto = "0,00"`
- **Status da Revisão Independente:** **CONFIRMADO**
- **Classificação:** **Defeito comprovado**
- **Evidência no código-fonte (`app/qml/TelaVender.qml:914–916, 941–962`):**
  ```qml
  } else if (repFormas.count > 0) {
      repFormas.itemAt(0).campo.texto = total
  }
  ```
  ```qml
  function completarVale() {
      if (tela.vales.length === 0) return
      const v = tela.vales[Math.max(0, escolhaVale.indice)]
      const saldo = tela.centavos(v ? v.saldo : "0")
      campoVale.texto = ""
      atualizar()
      const falta = calc ? tela.centavos(calc.falta) : 0
      campoVale.texto = tela.reais(Math.min(saldo, falta))
      atualizar()
      campoVale.focar()
  }
  ```
  - Como `abrirCom(total)` já preenche `repFormas.itemAt(0)` (`"Dinheiro"`) com `total`, `calc.falta` é `"0,00"`. Diferentemente de `completar(linha)` (`linhas 951–962`), que zera todas as formas quando `calc.falta === "0,00"`, `completarVale()` limpa apenas `campoVale.texto`, encontra `falta === 0` e preenche `campoVale.texto = "0,00"`.

### 2.7. `[B9A2C-02]` `"estoque_sugestao"` (`api.rs`) e `SecaoComprar.qml` expõem `custoTotal` para operador `Caixa` sem verificar a permissão `"resultados"` (D-09)
- **Status da Revisão Independente:** **CONFIRMADO**
- **Classificação:** **Defeito comprovado**
- **Evidência no código-fonte (`nucleo/src/api.rs:2099–2102` e `app/qml/SecaoComprar.qml:99–105, 349`):**
  - Em `api.rs:2099–2102`:
    ```rust
    "estoque_sugestao" => {
        let d: Filtro = ler(dados)?;
        Ok(json!(reposicao::sugestao(&self.banco.conn, &d.texto)?))
    }
    ```
  - Enquanto `"estoque_mais_vendidos"` (`api.rs:2113–2125`) e `"estoque_parados"` (`api.rs:2130–2141`) verificam `self.pode_sem_senha("resultados", sessao)?` e limpam `lucro`/`margem`/`valor` quando `!pode`, `"estoque_sugestao"` retorna `custoTotal` sem filtro e `SecaoComprar.qml` exibe o cartão `"Custo estimado"` (`linhas 99–105`) e `"≈ R$ " + Tema.valor(lp.p.custoTotal)` (`linha 349`) para qualquer operador `Caixa`.

### 2.8. `[B9A2C-03]` `TextoErro` no modal `nova_` de `SecaoEncomendas.qml` só exibe erro quando `nova_.campoErro === ""`, ocultando erros com outros nomes de campo
- **Status da Revisão Independente:** **CONFIRMADO**
- **Classificação:** **Defeito comprovado**
- **Evidência no código-fonte (`app/qml/SecaoEncomendas.qml:351–369, 387, 418, 429, 449, 461`):**
  ```qml
  TextoErro {
      text: nova_.campoErro === "" ? nova_.erro : ""
      Layout.fillWidth: true
  }
  ```
  - Os únicos `Campo`s que leem `nova_.campoErro` são `"cliente"` (`387`), `"telefone"` (`418`), `"peca"` (`429`) e `"quantidade"` (`449`). Se `"encomenda_criar"` retornar erro com `campo: "descricao"`, `"observacoes"`, `"chave"`, `"repetida"` ou `"modoSeguro"`, nem os `Campo`s nem o `TextoErro` exibem a mensagem.

### 2.9. `[B9A2C-04]` `abrirReceber()` em `TelaVender.qml` não verifica `calculo.erroDesconto` antes de abrir o modal `receber`, e `descontoTotal` não recalcula em `onEditado`
- **Status da Revisão Independente:** **CONFIRMADO**
- **Classificação:** **Defeito comprovado**
- **Evidência no código-fonte (`app/qml/TelaVender.qml:286–307, 784–794`):**
  - `abrirReceber()` verifica `calculo.itens.some(i => i.erro)` (`linha 297`) e `calculo.erroEncomenda` (`linha 301`), mas omite `if (calculo && calculo.erroDesconto)`, abrindo o modal `receber` mesmo quando o desconto global está inválido. Além disso, `Campo { id: descontoTotal }` (`linhas 784–794`) só chama `tela.recalcular()` em `onConfirmado` e `onSaiu` (sem `onEditado`).

### 2.10. `[B9A2C-05]` `TelaVender.qml` fixa `tabela: 1` e ignora a tabela de preços de orçamentos carregados
- **Status da Revisão Independente:** **CONFIRMADO**
- **Classificação:** **Defeito comprovado**
- **Evidência no código-fonte (`app/qml/TelaVender.qml:56, 1352–1364`):**
  - Na linha `56`, `entrada(pagamentos)` envia sempre `tabela: 1`, não existindo seletor para as tabelas 2 a 5 na tela de venda nem preservação da tabela ao carregar um orçamento (`linhas 1352–1364`).

### 2.11. `[B9A2C-06]` Aba `"Já devolvidas"` em `TelaTrocas.qml` omite o nome do caixa, mantém `"Desfazer"` ativo quando `valeUsado == true` e afirma que peças com defeito saem do estoque
- **Status da Revisão Independente:** **CONFIRMADO**
- **Classificação:** **Defeito comprovado**
- **Evidência no código-fonte (`app/qml/TelaTrocas.qml:272–291, 300–310`):**
  - As linhas `273–285` omitem `lf.modelData.caixa` e `lf.modelData.vendaCaixa`;
  - A linha `301` (`visible: (sobreFeita.hovered || Tema.celular) && !lf.modelData.cancelada`) mantém o botão `"Desfazer"` visível e clicável mesmo quando `lf.modelData.valeUsado === true`, abrindo `PerguntaMotivo` e `PedirGerente` para uma operação que inevitavelmente falhará no núcleo;
  - A linha `307` informa `"As peças saem de novo do estoque"` mesmo quando todos os itens devolvidos eram defeituosos (`volta_estoque == 0`).

### 2.12. `[B9A2C-07]` Melhorias em `TelaVender.qml`, `SecaoVendidos.qml` e verificação de `PerguntaMotivo.qml`
- **Status da Revisão Independente:** **CONFIRMADO (itens 1 e 2) / ESCLARECIDO (item 3)**
- **Classificação:** **Melhoria**
- **Evidência no código-fonte:**
  1. `TelaVender.qml:1287` (`caixaOrcamento.abrirCom()`): não valida `calculo.itens.some(i => i.erro)` nem `calculo.erroDesconto` antes de abrir o modal de salvar orçamento;
  2. `SecaoVendidos.qml:214`: o chip de estoque mostra `"Est. " + linha.v.estoque` sem concatenar `+ " " + linha.v.unidade` (diferentemente de `SecaoParados.qml:178`);
  3. Verificação em `app/qml/PerguntaMotivo.qml:33–37`: `confirmar()` executa `const r = acao(campo.texto, resposta => motivo.terminar(resposta)); if (r !== undefined) terminar(r)`. Portanto, `PerguntaMotivo.qml` suporta legitimamente tanto callbacks síncronos (`m => Api.gravar(...)` em `SecaoEncomendas.qml:273`) quanto assíncronos (`(m, pronto) => chamarGerente(..., pronto)` em `TelaTrocas.qml:309`).

---

## 3. Novos Achados Identificados na Auditoria Independente do Bloco 9A2 / 9A2C

### 3.1. `[NEW-B9A2-01]` Carregar um orçamento em `TelaVender.qml` (`StackView.onActivated`) não chama `limparVenda()`, mantendo `encomendaCarregada`, `osCarregada` e `valeTroca` de uma operação anterior presos no carrinho
- **Classificação:** **Defeito comprovado**
- **Localização:** `app/qml/TelaVender.qml:59–63, 1352–1393`
- **Evidência literal (`app/qml/TelaVender.qml:1352–1370`):**
  ```qml
  if (orcamentoId !== "") {
      const id = orcamentoId
      orcamentoId = ""
      const r = Api.chamar("venda_abrir", { id })
      if (r.ok && r.dados.situacao === "orcamento") {
          orcamentoCarregado = r.dados.id
          chaveVenda = Api.novaChave()
          clienteId = r.dados.clienteId || ""
          clienteNome = r.dados.cliente || ""
          campoCliente.texto = clienteNome
          descontoTotal.texto = ""
          carrinho = r.dados.itens.map(i => ({ ... }))
          itemAtual = carrinho.length - 1
          recalcular()
      }
  } else if (encomendaId !== "") {
      const id = encomendaId
      encomendaId = ""
      const r = Api.chamar("encomenda_para_venda", { id })
      if (r.ok) {
          limparVenda()
          encomendaCarregada = r.dados.encomendaId
  ```
- **Análise técnica:** Enquanto os ramos `encomendaId !== ""` (`linha 1368`) e `osId !== ""` (`linha 1383`) chamam `limparVenda()` antes de popular o carrinho, o ramo `orcamentoId !== ""` (`linhas 1352–1364`) **não chama `limparVenda()`**. Se o operador havia iniciado no PDV a entrega de uma encomenda (`encomendaCarregada`), o faturamento de uma OS (`osCarregada`) ou o uso de um vale-troca (`valeTroca`), e em seguida abre um orçamento em `TelaOrcamentos.qml` e clica em `"Vender"`, `TelaVender.qml` carrega os itens do orçamento mantendo `encomendaCarregada`, `osCarregada` e `valeTroca` preenchidos e enviando `orcamentoId`, `encomendaId` e `osId` juntos em `entrada(pagamentos)` (`linhas 60–62`).

### 3.2. `[NEW-B9A2-02]` `adicionar()` em `TelaVender.qml` trata separadores de milhar de forma assimétrica ao somar quantidades repetidas no carrinho
- **Classificação:** **Risco** (Inconsistência numérica em quantidades $\ge 1.000$ ou com casas decimais no multiplicador `N*`)
- **Localização:** `app/qml/TelaVender.qml:96–104`
- **Evidência literal:**
  ```qml
  const atual = Number(String(lista[i].quantidade || "0").replace(".", "").replace(",", "."))
  const mais = Number(String(quantidade || "1").replace(",", "."))
  const soma = (isNaN(atual) ? 0 : atual) + (isNaN(mais) ? 1 : mais)
  lista[i].quantidade = String(soma).replace(".", ",")
  ```
- **Análise técnica:** `atual` remove apenas o primeiro `"."` (`.replace(".", "")`), `mais` não trata ponto de milhar, e `soma` em ponto flutuante binário (`0.1 + 0.2 = 0.30000000000000004`) sem `Math.round(soma * 1000) / 1000` pode gerar mais de 3 casas decimais em `lista[i].quantidade`, que depois falha em `numeros::quantidade` (`"Use no máximo 3 casas decimais."`).

---

## 4. Registro Separado de Código Lido, Testes Encontrados e Testes Executados (Bloco 9A2 / 9A2C)

| Métrica | Valor | Detalhamento |
|---|---:|---|
| **Arquivos QML lidos integralmente neste bloco** | **14 (+1)** | `AjusteEstoque.qml` (122), `LinhaVenda.qml` (103), `LinhaCarrinho.qml` (211), `TelaCaixa.qml` (414), `TelaDevolucao.qml` (468), `TelaEstoque.qml` (93), `TelaVenda.qml` (330), `SecaoComprar.qml` (364), `SecaoEncomendas.qml` (482), `SecaoParados.qml` (217), `SecaoVendidos.qml` (245), `TelaTrocas.qml` (362), `TelaVender.qml` (1.401), `TelaOrcamentos.qml` (123) + `PerguntaMotivo.qml` (66). Total: **4.835 linhas**. |
| **Testes automatizados embutidos nos arquivos QML** | **0** | Arquivos QML não contêm testes embutidos (os testes Python de UI em `app/testes/` serão auditados no Bloco 18). |
| **Testes executados neste bloco** | **0** | Conforme o plano, a execução da suíte de testes está isolada no Bloco 18. |

