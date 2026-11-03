# BLOCO 10 — Auditoria do Módulo Financeiro, Contas a Receber, Contas a Pagar, Cheques, Comissões e DRE (Núcleo + UI)

**Data da Auditoria:** 2026-11-02  
**Versão Auditada:** Rustinho 0.9.17  
**Escopo do Bloco:** Auditoria integral de código-fonte (leitura linha a linha) do motor financeiro completo no núcleo Rust (`receber.rs`, `financeiro.rs`, `pagar.rs`, `cheques.rs`, `comissoes.rs`, `dre.rs`, `dre/testes.rs`) e das 22 telas/seções/componentes QML correspondentes em `app/qml/`.  
**Restrição de Imutabilidade:** Cumprida integralmente (`allowLegacyEdits: false`). Nenhum arquivo do projeto legado foi modificado.

---

## 1. Inventário de Arquivos Lidos Integralmente neste Bloco

### 1.1. Núcleo Rust (`nucleo/src/` — 7 arquivos, 7.267 linhas, 17 funções `#[test]`)

| # | Arquivo | Linhas Lidas | Total de Linhas | `#[test]` Encontrados | Escopo Auditado |
|---|---|---:|---:|---:|---|
| 1 | [nucleo/src/receber.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/receber.rs) | 1–1269 | 1.269 | 2 | Crediário, limites, bloqueio por atraso, juros simples pró-rata die, descontos c/ alçada, rateio FIFO de recebimentos, cheques recebidos, recibo térmico, estorno e cancelamento de dívida manual. |
| 2 | [nucleo/src/financeiro.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/financeiro.rs) | 1–1264 | 1.264 | 5 | Contas financeiras (`caixa-geral` e bancos), saldo inicial, movimentos manuais (`entrada`, `saida`, `ajuste`), transferências pareadas (`par_id`), extrato c/ saldo progressivo, resumo executivo e fluxo de caixa (realizado + previsto + compras sem parcelas). |
| 3 | [nucleo/src/pagar.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/pagar.rs) | 1–924 | 924 | 0 *(testado em `financeiro.rs`)* | Categorias financeiras, lançamento de contas a pagar (1 a 60 parcelas, competência `YYYY-MM`, rateio de centavos na 1ª parcela), baixa parcial/total com juros/desconto, saída de conta bancária ou sangria de gaveta, estorno e cancelamento. |
| 4 | [nucleo/src/cheques.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/cheques.rs) | 1–557 | 557 | 1 | Ciclo completo de cheques (`em_maos` → `depositado`/`compensado`/`repassado`/`devolvido` → `pago`/`cancelado`), vínculo com `movimento_conta` e reabertura ou recriação de `conta_receber` ao devolver cheque. |
| 5 | [nucleo/src/comissoes.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/comissoes.rs) | 1–284 | 284 | 0 *(testado em `financeiro.rs`)* | Apuração de comissões por vendedor (`percentual_efetivo`: peça → categoria → vendedor → loja), abatimento de devoluções quando `comissao_descontar_devolucao = true`, fechamento com geração de `conta_pagar` na categoria `'Comissões'` e reabertura. |
| 6 | [nucleo/src/dre.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/dre.rs) | 1–2350 | 2.350 | 0 *(em submódulo)* | Classificação de categorias na DRE, apuração por competência vs. caixa, alíquotas efetivas do Simples Nacional (Anexos I e III com 6 faixas e RBT12), rateio proporcional de tributos sobre serviços, CMV (com queda para custo atual quando `custo == 0`), comissões devidas vs. lançadas, drill-down (`detalhe` com teto de 500 itens), rankings (`clientes`, `fornecedores`, `pagamentos`, `indicadores`) e metas mensais/semanais. |
| 7 | [nucleo/src/dre/testes.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/dre/testes.rs) | 1–619 | 619 | 9 | Suíte de testes unitários da DRE, Simples Nacional, indicadores, rankings, drill-down e metas. |

### 1.2. Camada de Interface QML (`app/qml/` — 22 arquivos, 6.227 linhas)

| # | Arquivo | Linhas Lidas | Total de Linhas | Escopo Auditado |
|---|---|---:|---:|---|
| 1 | [app/qml/TelaFinanceiro.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaFinanceiro.qml) | 1–89 | 89 | Container com 11 abas (`Resumo`, `Contas a pagar`, `Fluxo de caixa`, `Contas e bancos`, `Cheques`, `Comissões`, `Resultado (DRE)`, `Indicadores`, `Por cliente`, `Por fornecedor`, `Formas de pagamento`) via `StackLayout`. |
| 2 | [app/qml/TelaReceber.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaReceber.qml) | 1–928 | 928 | Tela principal de crediário: lista de devedores, ficha do cliente, seleção de parcelas, popup `pagar` (formas, cheques, troco, juros/desconto), popup `recibo`, popup `lancar` (dívida manual) e popup local `motivo`. |
| 3 | [app/qml/SecaoResumo.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoResumo.qml) | 1–341 | 341 | Visão executiva consolidada: saldos de caixa/bancos, a receber (30d/vencido), a pagar (30d/vencido), resultado do mês, alertas clicáveis e listas de vencidos. |
| 4 | [app/qml/SecaoPagar.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoPagar.qml) | 1–839 | 839 | Gestão de contas a pagar: filtros, busca, lançamento (1–60x c/ competência e fornecedor), baixa (conta ou gaveta aberta), ficha da conta, estorno/cancelamento e gestão de categorias. |
| 5 | [app/qml/SecaoFluxo.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoFluxo.qml) | 1–216 | 216 | Fluxo de caixa realizado + projetado (7 a 90 dias, hoje, mês, ano ou personalizado) com alerta de primeiro dia negativo. |
| 6 | [app/qml/SecaoContas.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoContas.qml) | 1–534 | 534 | Contas financeiras e bancos: cartões de saldo, extrato por período, lançamento manual (`entrada`/`saida`/`ajuste`), transferência entre contas, edição/criação de conta e estorno. |
| 7 | [app/qml/SecaoCheques.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoCheques.qml) | 1–358 | 358 | Carteira de cheques recebidos: filtros de situação, ações de depositar, compensar, devolver (com reabertura automática de crediário), receber em mãos, repassar e cancelar. |
| 8 | [app/qml/SecaoComissoes.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoComissoes.qml) | 1–268 | 268 | Apuração de comissões por período, detalhamento por venda/item, fechamento com geração de conta a pagar e reabertura. |
| 9 | [app/qml/SecaoDre.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoDre.qml) | 1–409 | 409 | Demonstração do Resultado do Exercício: regime de competência vs. caixa, comparação com período anterior e ano passado, avisos fiscais/custo/classificação e drill-down por linha. |
| 10 | [app/qml/SecaoIndicadores.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoIndicadores.qml) | 1–199 | 199 | Indicadores comerciais: tíquete médio, margem bruta, descontos, devoluções, tabela de desempenho por vendedor e rankings Top 10 de peças/categorias. |
| 11 | [app/qml/SecaoClientes.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoClientes.qml) | 1–175 | 175 | Ranking de faturamento por cliente no período, com drill-down de compras (`dre_detalhe` tipo `cliente`). |
| 12 | [app/qml/SecaoFornecedores.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoFornecedores.qml) | 1–169 | 169 | Ranking de compras/despesas por fornecedor (competência vs. caixa), com drill-down (`dre_detalhe` tipo `fornecedor`). |
| 13 | [app/qml/SecaoPagamentos.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoPagamentos.qml) | 1–83 | 83 | Distribuição do faturamento por forma de pagamento no período, com drill-down (`dre_detalhe` tipo `pagamento`). |
| 14 | [app/qml/PainelCheques.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/PainelCheques.qml) | 1–123 | 123 | Subpainel reutilizável de cheques (usado em `TelaVender` e `TelaReceber`) com atalhos `À vista`, `30`, `30/60`, `30/60/90`. |
| 15 | [app/qml/DetalheDre.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/DetalheDre.qml) | 1–114 | 114 | Modal de rastreabilidade (`dre_detalhe`) exibindo até 500 lançamentos que compõem uma linha da DRE ou ranking. |
| 16 | [app/qml/MetasDre.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/MetasDre.qml) | 1–178 | 178 | Modal de metas mensais/semanais da loja ou por vendedor (`vendas`, `lucro_bruto`, `margem`, `quantidade`). |
| 17 | [app/qml/ClassificarCategorias.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/ClassificarCategorias.qml) | 1–126 | 126 | Modal para classificar cada `categoria_financeira` em um grupo da DRE e confirmar revisão contábil (`dre_classificacao_confirmada`). |
| 18 | [app/qml/LinhaContaPagar.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/LinhaContaPagar.qml) | 1–120 | 120 | Delegate de conta a pagar na lista de `SecaoPagar.qml`. |
| 19 | [app/qml/LinhaDevedor.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/LinhaDevedor.qml) | 1–77 | 77 | Delegate de cliente devedor na coluna esquerda de `TelaReceber.qml`. |
| 20 | [app/qml/LinhaFluxo.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/LinhaFluxo.qml) | 1–106 | 106 | Delegate de dia no fluxo de caixa com barras proporcionais de entrada/saída e saldo acumulado. |
| 21 | [app/qml/LinhaParcela.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/LinhaParcela.qml) | 1–121 | 121 | Delegate de parcela de crediário na ficha do cliente em `TelaReceber.qml`. |
| 22 | [app/qml/LinhaRanking.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/LinhaRanking.qml) | 1–86 | 86 | Delegate de barra horizontal animada para rankings (clientes, fornecedores, pagamentos, peças, categorias e despesas da DRE). |

---

## 2. Revisão de Hipóteses Históricas que Dependiam do Bloco 10

### 2.1. Resolução de `B9A1-07` (Hipótese Histórica do Bloco 9A1 → Promovida a **DEFEITO COMPROVADO**)

| ID Histórico | Descrição Original | Veredito no Bloco 10 |
|---|---|---|
| **B9A1-07** | Hipótese de que um registro `recebimento` sintético gerado por `devolucoes::devolver` com `forma = 'devolucao'` e `valor = 0` poderia ser estornado manualmente em `receber::estornar`, reabrindo as parcelas do crediário sem desfazer a devolução. | **CONFIRMADO — PROMOVIDO A DEFEITO COMPROVADO (`NEW-B10-01`)** |

**Evidência Completa Cruzada (`devolucoes.rs` × `receber.rs` × `TelaReceber.qml`):**
1. Quando o cliente devolve peças de uma venda no crediário com `destino == "crediario"`, [`devolucoes::devolver`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/devolucoes.rs#L453-L485) grava um registro na tabela `recebimento` com `forma = 'devolucao'` e `valor = 0` e vincula o abatimento em `recebimento_parcela`.
2. Em [`receber::ficha`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/receber.rs#L371-L407), todos os registros de `recebimento` do cliente são retornados em `recebimentos`, traduzindo `forma = "devolucao"` para `forma_nome = "Abate por devolução"` (linha `212`).
3. Em [`TelaReceber.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaReceber.qml#L368-L415), a lista `"Pagamentos"` renderiza o botão `"Estornar"` para **qualquer** item com `visible: !modelData.estornado` (linha `406`), **sem filtrar `forma === "devolucao"`**.
4. Em [`receber::estornar`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/receber.rs#L961-L1015), a função busca `(cliente_id, numero, valor, forma, sessao_id, estornado_em)` (linhas `967–975`), verifica apenas `if estornado_em.is_some()` (linha `976`) e **não bloqueia `forma == "devolucao"`**.
5. **Consequência:** Ao estornar esse registro pela tela de Receber (ou via API `recebimento_estornar`), `recebimento.estornado_em` é preenchido, fazendo a view `conta_receber_saldo` somar de volta a dívida ao cliente, enquanto a devolução permanece ativa e as peças devolvidas continuam no estoque. Além disso, se alguém posteriormente acionar `devolucao_desfazer` ([`devolucoes.rs:820`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/devolucoes.rs#L820)), o núcleo sobrescreverá `estornado_em` novamente sem perceber que o abatimento já havia sido estornado avulsamente.

---

## 3. Novos Achados Descobertos na Auditoria do Bloco 10

### 3.1. Defeitos Comprovados

#### `[NEW-B10-01]` (ex-Hipótese `B9A1-07`) `receber::estornar` e `TelaReceber.qml` permitem estornar avulsamente um `"Abate por devolução"` (`forma == "devolucao"`), reabrindo a dívida do cliente sem reverter a devolução
- **Classificação:** **Defeito comprovado** (Integridade Financeira / Crediário × Devoluções)
- **Severidade:** Alta
- **Arquivos e Linhas:**
  - [`nucleo/src/receber.rs:205-215, 371-407, 961-1015`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/receber.rs#L961-L1015)
  - [`app/qml/TelaReceber.qml:378-415`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaReceber.qml#L378-L415)
- **Evidência:**
  ```rust
  // nucleo/src/receber.rs:967-979
  let (cliente_id, numero, valor, forma, sessao_id, estornado_em): (
      String, i64, i64, String, Option<String>, Option<String>,
  ) = conn.query_row(
      "SELECT cliente_id, numero, valor, forma, sessao_id, estornado_em FROM recebimento WHERE id = ?1",
      [id],
      |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
  ).map_err(|_| "Pagamento não encontrado.".to_string())?;
  if estornado_em.is_some() {
      return Err("Este pagamento já foi estornado.".into());
  }
  // Ausência de bloqueio para `if forma == "devolucao"`!
  ```
- **Impacto:** Duplicidade patrimonial a favor da loja (peça devolvida permanece no estoque da loja e a parcela de crediário volta a ficar em aberto contra o cliente).

---

#### `[NEW-B10-02]` Popup local `motivo` em `TelaReceber.qml` é incompatível com a assinatura assíncrona de `janela.chamarGerente`, lançando `TypeError: Cannot read property 'ok' of undefined` ao cancelar dívida manual ou estornar recebimento
- **Classificação:** **Defeito comprovado** (Quebra Funcional na UI / Fluxo de Supervisão)
- **Severidade:** Alta
- **Arquivo e Linhas:** [`app/qml/TelaReceber.qml:352-353, 410-412, 857-892`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaReceber.qml#L857-L892)
- **Evidência:**
  Diferentemente de `SecaoPagar.qml`, `SecaoContas.qml`, `SecaoCheques.qml` e `SecaoComissoes.qml` — que utilizam o componente reutilizável [`PerguntaMotivo.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/PerguntaMotivo.qml#L21-L36) preparado para callbacks `(motivo, pronto)` —, `TelaReceber.qml` define um `Popup { id: motivo }` próprio nas linhas `857–892`:
  ```qml
  // app/qml/TelaReceber.qml:352-353
  onCancelar: motivo.perguntar("Tirar esta dívida?", modelData.descricao + " · R$ " + modelData.saldo, "Tirar",
                               (m, pronto) => tela.janela.chamarGerente("conta_cancelar", { id: modelData.id, motivo: m }, pronto))
  // app/qml/TelaReceber.qml:410-412
  onClicked: motivo.perguntar("Estornar o pagamento?",
                              "O valor volta a ficar em aberto para o cliente.", "Estornar",
                              (m, pronto) => tela.janela.chamarGerente("recebimento_estornar", { id: modelData.id, motivo: m }, pronto))
  ```
  Porém, `confirmar()` em `TelaReceber.qml:883-891` invoca `acao` passando **apenas 1 argumento** (`campoMotivo.texto`), deixando `pronto` como `undefined`, e tenta ler `r.ok` do retorno síncrono de `chamarGerente` (que sempre retorna `undefined`):
  ```qml
  // app/qml/TelaReceber.qml:883-891
  function confirmar() {
      const r = acao(campoMotivo.texto)
      if (!r.ok) {
          erro = r.erro
          return
      }
      close()
      tela.atualizarTudo()
  }
  ```
- **Impacto:**
  1. **Se o usuário logado já é gerente:** `chamarGerente` executa a chamada na API, mas como `depois` (`pronto`) é `undefined`, não há callback; `chamarGerente` retorna `undefined`, e `if (!r.ok)` falha com `TypeError: Cannot read property 'ok' of undefined`. O popup `motivo` **não fecha**, erros de validação (ex.: motivo em branco) **não são exibidos**, e `tela.atualizarTudo()` **não é executado**!
  2. **Se o usuário logado não é gerente (ex.: caixa/vendedor):** `chamarGerente` abre o modal `PedirGerente` sobre o popup `motivo` (que continua aberto porque não houve `close()`), passa `depois = undefined`, e lança a mesma exceção `TypeError` imediatamente antes mesmo de o gerente digitar a senha; após o gerente autorizar, a tela `TelaReceber` também não é atualizada e o popup `motivo` permanece travado aberto na frente do usuário.

---

#### `[NEW-B10-03]` `pagar::estornar` falha na conciliação de gaveta quando o estorno ocorre a partir de outro computador/celular e grava movimento `'ajuste'` passível de estorno avulso
- **Classificação:** **Defeito comprovado** (Integridade de Caixa / Multi-dispositivo)
- **Severidade:** Alta
- **Arquivos e Linhas:**
  - [`nucleo/src/pagar.rs:867-895`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/pagar.rs#L867-L895)
  - [`nucleo/src/financeiro.rs:415-425, 551`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/financeiro.rs#L415-L425)
- **Evidência:**
  ```rust
  // nucleo/src/pagar.rs:867-895
  if let Some(sessao_id) = sessao {
      let aberta = caixa::sessao_aberta(conn, dispositivo)?.filter(|s| s.id == sessao_id);
      if aberta.is_some() {
          caixa::movimentar_com_conta(
              conn,
              dispositivo,
              usuario_id,
              "suprimento",
              & crate::moeda::formatar(total),
              &format!("Estorno: {descricao}"),
              Some("caixa-geral"),
          )?;
      } else {
          // A sessão já fechou: devolve direto no Caixa geral.
          conn.execute(
              "INSERT INTO movimento_conta (id, conta_id, data, tipo, valor, descricao, origem_tipo, origem_id, usuario_id, criado_em)
               VALUES (?1, 'caixa-geral', ?2, 'ajuste', ?3, ?4, 'pagamento_estorno', ?5, ?6, ?7)",
              params![...],
          )
  ```
- **Dois problemas combinados:**
  1. **Falso diagnóstico de "sessão fechada" em multi-dispositivo:** Em vez de consultar no banco se a `caixa_sessao` de ID `sessao_id` ainda está com `fechado_em IS NULL` (ou usar `caixa::sessao_fechada(conn, Some(&sessao_id))`), o código chama `caixa::sessao_aberta(conn, dispositivo)?.filter(|s| s.id == sessao_id)`. Se o pagamento da conta foi tirado da gaveta do **Computador A** (cuja sessão `sessao_id` **continua aberta**) e o gerente estorna o pagamento pelo **Computador B** ou pelo **celular Android**, `aberta` será `None`! O código assume erroneamente que a sessão já fechou e credita `'caixa-geral'`, deixando a gaveta aberta do Computador A com a sangria ativa (diferença física de caixa no fechamento).
  2. **Uso de `tipo = 'ajuste'` em vez de `'caixa_fechado'`:** Enquanto `caixa::compensar_caixa_fechado` grava `tipo = 'caixa_fechado'` — que é bloqueado contra estorno manual em [`financeiro::estornar_movimento`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/financeiro.rs#L419) (`if tipo == "inicial" || tipo == "caixa_fechado"`) e marcado como `estornavel: false` em [`financeiro::extrato`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/financeiro.rs#L551) —, `pagar::estornar` grava `tipo = 'ajuste'`. Assim, na aba `"Contas e bancos"` (`SecaoContas.qml`), esse lançamento automático de estorno aparece com o botão `"Estornar"` habilitado, permitindo que o usuário estorne o ajuste isoladamente e desbalanceie o `'caixa-geral'`.

---

#### `[NEW-B10-04]` `receber::estornar` não verifica se a sessão aberta da gaveta pertence ao dispositivo atual nem se há saldo físico em dinheiro antes de abater da gaveta
- **Classificação:** **Defeito comprovado** (Mesma classe arquitetural de `B9A1-02` e `B9A1-03`)
- **Severidade:** Média / Alta
- **Arquivo e Linhas:** [`nucleo/src/receber.rs:990-1012`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/receber.rs#L990-L1012)
- **Evidência:**
  ```rust
  let fechado = caixa::sessao_fechada(conn, sessao_id.as_deref())?;
  let conta_caixa = match forma.as_str() {
      "dinheiro" if fechado => Some("caixa-geral".to_string()),
      "pix" | "debito" | "credito" => ...
      _ => None,
  };
  ```
- **Impacto:**
  1. Se o recebimento em dinheiro ocorreu na gaveta aberta do Computador A (`fechado == false`) e o gerente estorna o recebimento a partir do Computador B (ou após uma sangria que esvaziou a gaveta do Computador A), `recebimento.estornado_em` passa a `NOT NULL` (deixando de somar em `resumo_sessao` do Computador A, podendo levar o esperado em dinheiro da gaveta a valor negativo) e nenhuma compensação entra no `'caixa-geral'`.
  2. Se o recebimento original foi em dinheiro **sem caixa aberto** (`sessao_id IS NULL`, ex.: registrado pelo celular ou com caixa fechado, onde `receber::receber` na linha `647` debitou direto em `'caixa-geral'`), `caixa::sessao_fechada(conn, None)` retorna `Ok(false)`! Logo, quando esse recebimento sem `sessao_id` é estornado em `receber::estornar`, `fechado` é **`false`**, `conta_caixa` vira **`None`**, e **nenhum movimento de saída é gravado em `'caixa-geral'`**, inflando permanentemente o saldo de `'caixa-geral'`!
  - Verifique a linha `647` de `receber.rs` vs linha `991`:
    - Em `receber::receber` (`receber.rs:647`): `"dinheiro" if sessao_id.is_none() => Some("caixa-geral".to_string())` — o dinheiro **entrou** em `'caixa-geral'`!
    - Em `receber::estornar` (`receber.rs:991-993`): `let fechado = caixa::sessao_fechada(conn, sessao_id.as_deref())?;` → para `sessao_id == None`, `caixa::sessao_fechada` retorna `Ok(false)`! E na linha `993`: `"dinheiro" if fechado => Some("caixa-geral".to_string())` avalia como `None`!
    - Portanto, **100% dos recebimentos em dinheiro feitos sem sessão de caixa aberta (`sessao_id IS NULL`) creditam `'caixa-geral'` no recebimento, mas NUNCA debitam `'caixa-geral'` quando estornados!** Isso é um defeito matemático determinístico.

---

#### `[NEW-B10-05]` `comissoes::reabrir` não verifica bloqueio de sincronização (`sync::exigir_editavel`) nem registra evento próprio na tabela `auditoria`
- **Classificação:** **Defeito comprovado** (Inconsistência de Trilha de Auditoria e Sincronização)
- **Severidade:** Baixa / Média
- **Arquivo e Linhas:** [`nucleo/src/comissoes.rs:258-284`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/comissoes.rs#L258-L284)
- **Evidência:**
  Enquanto `comissoes::fechar` grava `auditoria::registrar(conn, usuario_id, &nome, "comissao.fechar", ...)` (linhas `239–247`), `comissoes::reabrir` atualiza `comissao_fechamento` diretamente sem chamar `crate::sync::exigir_editavel(conn, "comissao_fechamento", id)?` e sem registrar `"comissao.reabrir"` em `auditoria`:
  ```rust
  // nucleo/src/comissoes.rs:277-281
  let agora = banco::agora();
  conn.execute(
      "UPDATE comissao_fechamento SET cancelado_em = ?2, atualizado_em = ?2 WHERE id = ?1",
      params![id, agora],
  )
  .map_err(|e| format!("Não deu para reabrir a comissão: {e}"))?;
  ```

---

#### `[NEW-B10-06]` `financeiro::fluxo` usa `date('now', 'localtime')` do SQLite em vez de `hoje` (`fiscal::hoje(conn)`) na consulta de pagamentos realizados
- **Classificação:** **Defeito comprovado** (Inconsistência de Fuso Horário / Data Fiscal)
- **Severidade:** Baixa
- **Arquivo e Linhas:** [`nucleo/src/financeiro.rs:724-734`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/financeiro.rs#L724-L734)
- **Evidência:**
  Todas as demais consultas de `financeiro::fluxo` usam o parâmetro `?3` (`&hoje`, obtido via `crate::fiscal::hoje(conn)` na linha `679`), mas a consulta de `conta_pagar_pagamento` na linha `728` usa `date('now', 'localtime')`:
  ```rust
  "SELECT g.data, g.valor + g.juros - g.desconto, p.descricao
   FROM conta_pagar_pagamento g
   JOIN conta_pagar p ON p.id = g.conta_id
   WHERE g.estornado_em IS NULL AND g.data >= ?1 AND g.data <= ?2 AND g.data <= date('now', 'localtime')"
  ```
- **Impacto:** Se o fuso configurado na loja (`fiscal::hoje(conn)`) divergir do `localtime` do sistema operacional (ou próximo à meia-noite entre fusos diferentes no Brasil, ex.: AC/AM/MT/MS vs Brasília), pagamentos do próprio dia podem sumir ou duplicar entre realizado e previsto.

---

### 3.2. Riscos e Limitações Arquiteturais Identificados

#### `[NEW-B10-07]` `dre::indicadores` — Devolução de peça ocorrida em período posterior ao da venda só é abatida no ranking de produtos se a peça tiver ao menos uma venda no período consultado
- **Classificação:** **Risco** (Consistência de Indicadores Comerciais entre Períodos)
- **Severidade:** Baixa / Média
- **Arquivo e Linhas:** [`nucleo/src/dre.rs:1707-1734`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/dre.rs#L1707-L1734)
- **Evidência:**
  ```rust
  // nucleo/src/dre.rs:1725-1732
  if let Some(pid) = prod_id {
      if let Some(e) = produtos.get_mut(&pid) {
          e.q -= q;
          e.receita -= valor;
          e.custo -= custo;
          e.lucro -= valor - custo;
      }
  }
  ```
- **Impacto:** Como `produtos` só é populado no laço de vendas do período (`linhas 1662–1691`), se uma peça foi vendida no mês passado e devolvida neste mês:
  - Se a peça **não** teve nenhuma venda neste mês, `produtos.get_mut(&pid)` é `None` e a devolução é ignorada no ranking de peças.
  - Se a peça teve 1 venda neste mês, a devolução do mês passado é subtraída daquela venda deste mês (podendo gerar `q = 0` ou `receita <= 0`, que depois é filtrada por `filter(|p| p.receita > 0 && p.q > 0.0001)` na linha `1741`).

---

#### `[NEW-B10-08]` Popups e Painéis extensos do Financeiro/Receber sem `Flickable` interno ou teto de altura estouram telas pequenas/celulares
- **Classificação:** **Risco** (Usabilidade Mobile / Telas Baixas)
- **Severidade:** Média
- **Arquivos e Linhas:**
  - [`app/qml/TelaReceber.qml:442-614`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaReceber.qml#L442-L614) (`Popup { id: pagar }`)
  - [`app/qml/TelaReceber.qml:671-854`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaReceber.qml#L671-L854) (`Popup { id: lancar }`)
  - [`app/qml/SecaoPagar.qml:156-347`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoPagar.qml#L156-L347) (`Painel { id: lancamento }`)
  - [`app/qml/SecaoPagar.qml:350-518`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoPagar.qml#L350-L518) (`Painel { id: pagamento }`)
  - [`app/qml/SecaoIndicadores.qml:116-175`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoIndicadores.qml#L116-L175) (Tabela de 9 colunas na faixa `"Vendedores"`)
- **Evidência:**
  1. Em `TelaReceber.qml`, `pagar` e `lancar` são `Popup` sem `height: Math.min(implicitHeight, Tema.altura - 24)` e sem `Flickable`. Quando `PainelCheques` (em `pagar`) ou a lista `previa` de parcelas (em `lancar`) se expande, a altura ultrapassa `Tema.altura` em telas de celular ou notebooks `1366×768`.
  2. Em `SecaoIndicadores.qml:116-175`, a tabela `"Vendedores"` usa um `RowLayout` fixo de 9 colunas (`140 + 7 × 96 + 120 = 932 px` de largura preferencial) sem `Flickable` horizontal nem adaptação para `Tema.estreito`, esmagando os números em telas mobile.

---

### 3.3. Melhorias Identificadas

#### `[NEW-B10-09]` Modo de sigilo (`Tema.oculto`, ícone do olho em `SecaoResumo`) não oculta valores monetários em `SecaoPagar`, `LinhaContaPagar`, `SecaoContas`, `SecaoCheques` e `SecaoComissoes`
- **Classificação:** **Melhoria** (Consistência Visual / Privacidade de Tela)
- **Severidade:** Baixa
- **Arquivos e Linhas:**
  - [`app/qml/LinhaContaPagar.qml:88, 97`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/LinhaContaPagar.qml#L88-L97)
  - [`app/qml/SecaoPagar.qml:53-56, 566-569, 646, 675`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoPagar.qml#L53-L56)
  - [`app/qml/SecaoContas.qml:43, 81, 200`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoContas.qml#L43)
  - [`app/qml/SecaoCheques.qml:44-47, 134`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoCheques.qml#L44-L47)
  - [`app/qml/SecaoComissoes.qml:58-61, 149-151, 223`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoComissoes.qml#L58-L61)
- **Evidência:** Enquanto `SecaoResumo`, `SecaoFluxo`, `SecaoDre`, `SecaoIndicadores`, `SecaoClientes`, `SecaoFornecedores`, `SecaoPagamentos`, `DetalheDre`, `MetasDre`, `LinhaDevedor`, `LinhaParcela` e `LinhaRanking` usam `Tema.valor(...)` ou `Tema.sigiloTexto(...)` para mascarar cifras quando o usuário fecha o "olho" (`Tema.oculto = true`), essas 5 telas do mesmo módulo `TelaFinanceiro` interpolam `"R$ " + ...` diretamente sem passar por `Tema.valor(...)`.

---

## 4. Registro Separado de Testes Encontrados no Código vs. Testes Executados

### 4.1. Testes Unitários Lidos no Código-Fonte (17 funções `#[test]`)

| Arquivo | Linha | Nome do Teste | O que verifica |
|---|---:|---|---|
| [nucleo/src/receber.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/receber.rs#L1122-L1200) | 1122 | `crediario_fluxo_completo` | Criação de crediário na venda, juros após vencimento, pagamento parcial + quitação, estorno e reabertura. |
| [nucleo/src/receber.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/receber.rs#L1203-L1268) | 1203 | `limite_e_bloqueio_por_atraso` | Bloqueio de venda no crediário por atraso e por estouro de limite de crédito, e liberação com autorização de gerente. |
| [nucleo/src/financeiro.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/financeiro.rs#L949-L1027) | 949 | `lancar_parcelar_pagar_e_estornar_conta_a_pagar` | Parcelamento em 3x (rateio `100,00` → `33,34 + 33,33 + 33,33`), pagamento parcial, quitação c/ juros e estorno. |
| [nucleo/src/financeiro.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/financeiro.rs#L1030-L1079) | 1030 | `compra_a_prazo_gera_contas_a_pagar_e_cancelar_tira` | Integração compras → `conta_pagar` e bloqueio de cancelamento quando já há parcela paga. |
| [nucleo/src/financeiro.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/financeiro.rs#L1082-L1150) | 1082 | `contas_transferencia_e_fluxo_de_caixa` | Saldo inicial, transferência entre contas (`par_id`), pagamento de conta e projeção de fluxo de caixa. |
| [nucleo/src/financeiro.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/financeiro.rs#L1153-L1217) | 1153 | `cheque_entra_compensa_devolve_e_reabre_divida` | Recebimento em 2 cheques, depósito, compensação do 1º e devolução do 2º com recriação de `conta_receber`. |
| [nucleo/src/financeiro.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/financeiro.rs#L1220-L1263) | 1220 | `comissao_calcula_fecha_gera_conta_a_pagar_e_reabre` | Apuração de comissão de 5%, fechamento gerando `conta_pagar` e reabertura cancelando a conta. |
| [nucleo/src/cheques.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/cheques.rs#L533-L556) | 533 | `soma_meses_respeita_fim_do_mes` | Soma de meses em anos bissextos/não-bissextos (`31/01/2026 + 1 mês = 28/02/2026`, `2024 = 29/02/2024`). |
| [nucleo/src/dre/testes.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/dre/testes.rs#L35-L85) | 35 | `aliquota_simples_faixas_do_anexo_i` | Cálculo da alíquota efetiva do Simples Nacional Anexo I na 1ª e 2ª faixas. |
| [nucleo/src/dre/testes.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/dre/testes.rs#L88-L214) | 88 | `dre_calcula_competencia_e_caixa_com_venda_devolucao_e_despesa` | Apuração completa da DRE nos regimes de competência e de caixa com venda, devolução, CMV e despesa operacional. |
| [nucleo/src/dre/testes.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/dre/testes.rs#L217-L263) | 217 | `dre_usa_custo_atual_quando_venda_antiga_estava_zerada` | Queda para `produto.custo` atual quando `venda_item.custo == 0` e emissão de aviso na DRE. |
| [nucleo/src/dre/testes.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/dre/testes.rs#L266-L343) | 266 | `dre_separa_servico_com_anexo_iii_e_rateia_imposto_manual` | Separação de mercadorias (Anexo I) e mão de obra de OS (Anexo III) na DRE. |
| [nucleo/src/dre/testes.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/dre/testes.rs#L346-L384) | 346 | `dre_nao_duplica_comissao_quando_fechamento_vira_conta_a_pagar` | Prevenção de dupla contagem entre comissão apurada nas vendas e comissão já fechada em `conta_pagar`. |
| [nucleo/src/dre/testes.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/dre/testes.rs#L387-L445) | 387 | `classificacao_salva_grupos_e_confirma_em_config` | Mudança de grupo DRE de categorias e gravação de `dre_classificacao_confirmada`. |
| [nucleo/src/dre/testes.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/dre/testes.rs#L448-L511) | 448 | `rankings_clientes_fornecedores_pagamentos_e_indicadores` | Rankings de clientes, fornecedores, formas de pagamento e indicadores comerciais. |
| [nucleo/src/dre/testes.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/dre/testes.rs#L514-L561) | 514 | `detalhe_lista_lancamentos_da_linha` | Drill-down (`dre_detalhe`) de linhas da DRE (`receita_mercadorias`, `cmv`, `despesa_administrativa`, etc.). |
| [nucleo/src/dre/testes.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/dre/testes.rs#L564-L618) | 564 | `metas_salvam_calculam_e_removem` | Cadastro, cálculo de percentual atingido e remoção de metas da DRE. |

### 4.2. Testes Executados neste Bloco
- **Nenhum teste foi executado neste bloco.** Conforme o plano de auditoria (`.reversa/plan.md`), a execução da suíte de testes está isolada e programada para o **Bloco 18**.

---

## 5. Resumo Consolidado do Bloco 10

| Categoria | Quantidade | Identificadores |
|---|---:|---|
| Hipóteses Históricas Promovidas a Defeito Comprovado | 1 | `B9A1-07` → `NEW-B10-01` |
| Novos Defeitos Comprovados | 6 | `NEW-B10-01`, `NEW-B10-02`, `NEW-B10-03`, `NEW-B10-04`, `NEW-B10-05`, `NEW-B10-06` |
| Novos Riscos Identificados | 2 | `NEW-B10-07`, `NEW-B10-08` |
| Novas Melhorias Identificadas | 1 | `NEW-B10-09` |
| Arquivos Auditados Integralmente | 29 | 7 Rust (`7.267` linhas) + 22 QML (`6.227` linhas) = `13.494` linhas |

