# Regras de Negócio Extraídas e Verificadas — Rustinho 0.9.17

> Gerado pelo Reversa em 2026-11-02
> Cada regra referencia sua implementação exata no código-fonte e seu status de conformidade verificado na auditoria.

---

## 1. Segurança, Sessão, Papéis e Idempotência

- **RN-SEC-01 (Papéis de Usuário):** O sistema possui dois papéis (`gerente` e `caixa`). Operadores `caixa` não visualizam custos, margens de lucro, DRE nem relatórios financeiros (`api.rs:640–690`, `produtos.rs`, `compras.rs`, `estoque.rs`).
- **RN-SEC-02 (Supervisão de Uso Único — D-09):** Ações restritas executadas por um `caixa` exigem token `gerente_liberar` (`supervisao.rs:1–188`), vinculado ao `usuario_id` do caixa, ao escopo exato (`acao`: `preco_regra`, `desconto`, `estoque_ajuste`, `compra_estornar`, `venda_cancelar`, `devolucao`, `config_loja`, `config_simultaneidade_ver`, `config_gerente_ver`), válido por 120s e consumido na primeira gravação (`api.rs:761–857`).
  - *Nota de auditoria (`B8A-01a/b/c`, `NEW-B8A-01`):* Na consulta de telas (`config_loja_ver`, `config_simultaneidade_ver`), o token é validado sem consumo (`conferir_sem_gastar`) para permitir o carregamento dos subpainéis; na escrita (`produto_salvar`), um mesmo comando pode combinar mudança de preço e ajuste de estoque com um único token `preco_regra`.
- **RN-SEC-03 (Idempotência Obrigatória — FALHA-04):** Os 52 comandos listados em `NAO_REPETIR` (`api.rs:64–110`) exigem `"chaveOperacao"` não vazia; a resposta JSON é persistida em `operacao_feita` dentro da mesma transação SQLite da mutação (`api.rs:277–360`).

---

## 2. Catálogo de Peças, Unidades e Busca

- **RN-PROD-01 (Código Interno de 6 Dígitos):** Quando não informado, o código interno é gerado sequencialmente em faixa de 6 dígitos (`000001`–`999999`) por computador (`faixas.rs`, `codigo.rs`).
- **RN-PROD-02 (Seletor de 10 Unidades em 2 Linhas):** `unidades::sugerir` (`unidades.rs:58–178`) seleciona até 5 unidades com mais produtos ativos (`top_produtos`) + até 5 unidades com maior volume líquido vendido nas últimas 24h (`top_vendas_24h`, descontando devoluções vinculadas da janela), preenche com `UNIDADES_PADRAO` até 10 itens sem repetição e ordena alfabeticamente.
  - *Desvio comprovado (`B8B1-01` / `B8B1-09`):* `top_vendas_24h` filtra `v.criado_em >= datetime('now', '-24 hours')` (UTC) enquanto `venda.criado_em` é gravado em horário local (`tempo::agora()`), deslocando a janela em ~21h no fuso BRT (`UTC-3`). Além disso, no desktop (`EscolherUnidade.qml:99–111`), escolher uma unidade do popup modal reordena os 6 chips visíveis em vez de fazer substituição posicional no 6º chip.
- **RN-PROD-03 (Trava de Preço Mínimo e Custo):** Nenhuma peça com custo cadastrado pode ter preço de venda alterado abaixo da margem mínima (`preco_minimo_margem`) por um `caixa` sem token `preco_regra` do gerente (`produtos.rs:416–470`).

---

## 3. Estoque, Compras e Importação de NF-e

- **RN-EST-01 (Kardex Auditável):** Toda alteração de saldo em `estoque_saldo` deve gerar um registro correspondente em `estoque_movimento` com `antes`, `depois`, `quantidade`, `custo_unitario` e `motivo`.
- **RN-COMP-01 (Validação Criptográfica de XML de Compra):** Na importação de XML de NF-e (`importar.rs`, `nfe_assinatura/mod.rs`), o XML só é aceito se possuir assinatura XMLDSig válida sob certificado ICP-Brasil vigente na data de emissão (`dhEmi`), cujo CNPJ na OID `2.16.76.1.3.3` coincida com o CNPJ do `<emit>` e cujo `<dest>` coincida com o `loja_cnpj` configurado.
- **RN-COMP-02 (Custo Médio Ponderado):** Ao confirmar uma compra (`compras.rs:316–331`), o novo `custo_medio` da peça é recalculado ponderando o saldo anterior positivo e a quantidade que deu entrada.
  - *Desvio comprovado (`B8B1-02`):* A entrada de compra não multiplica a quantidade da nota pelo `fator` de `produto_unidade` quando a unidade da nota é uma embalagem secundária (ex.: `CX` com 12 `UN`).

---

## 4. Vendas, Caixa, Trocas e Financeiro

- **RN-VEN-01 (Congelamento de Custo na Venda):** No momento de `vendas::concluir` (`vendas.rs:425–498`), cada `venda_item` congela `custo_unitario = produto.custo_medio` vigente, garantindo que o CMV histórico do DRE (`dre.rs`) não mude retroativamente com compras futuras.
- **RN-VEN-02 (Isolamento de Caixa Fechado — D-14):** Cancelar uma venda em dinheiro cujo `caixa_sessao` original já foi fechado mantém `caixa_sessao.esperado` e `contado` originais intactos, marca `venda.cancelado_apos_fechar = 1` e lança uma `sangria` (`origem_tipo = 'venda_cancelada'`) na sessão de caixa aberta no momento do cancelamento (`vendas.rs:765–845`).
- **RN-FIN-01 (Crediário e Limite de Crédito):** Vendas no crediário validam `limite_credito` do cliente somando parcelas abertas (`vendas.rs:342–390`); superação de limite ou cliente bloqueado exige liberação do gerente (`desconto`/`credito`).
- **RN-FIN-02 (DRE por Regime de Competência):** O DRE (`dre.rs`) apura Receita Bruta pelas vendas concluídas no período, deduz Devoluções (`devolucao` no período) e Impostos sobre Vendas, subtrai o CMV dos itens líquidos vendidos e as Despesas Operacionais (`conta_pagar` por competência/vencimento, excluindo compras de mercadoria para revenda e categorias `fornecedor_pecas` / `imposto_venda` / `investimento` / `distribuicao`).

---

## 5. Avisos do Início e Navegação Contextual Guiada

- **RN-NOTIF-01 (Três Gravidades de Avisos):** Avisos críticos (`outra_loja`, `codigos_outro_computador`, `sync_parou`, `backup_atrasado`, `modo_seguro`) aparecem sempre expandidos em faixa e não podem ser minimizados nem escondidos; avisos de atenção (`conflitos`, `estoque_negativo`, `caixa_antigo`) minimizam em quadrado mas não podem ser escondidos; avisos informativos (`receber`, `pagar`, `encomendas`, `orcamentos`) começam minimizados em quadrado, abrem no máximo uma faixa não-crítica por vez e podem ser escondidos até meia-noite local (`avisos.rs`, `Notificacao.qml`, `TelaInicio.qml`).
- **RN-NAV-01 (Navegação Contextual Guiada sem Auto-Execução):** Quando uma operação falha por `requisito_ausente` (`requisitos.rs`), a UI preserva o estado da tentativa na origem, navega para o campo faltante com orientação (`Guia.qml`), e após o preenchimento válido retorna à origem exibindo `"Requisito satisfeito — conclua a operação."` sem executar a gravação/importação automaticamente (`Main.qml`, `TelaPecas.qml`, `TelaCompras.qml`, `TelaFiscal.qml`).

