# BLOCO 9A1 — Núcleo de Vendas, Devoluções, Caixa, Encomendas e Reposição

**Fase do Plano:** Fase 2 — Auditoria de Código e Revalidação dos Achados (Bloco 9A1)  
**Data da Auditoria:** 2026-11-02  
**Status:** CONCLUÍDO ✅

---

## 1. Escopo Auditado (Leitura Integral)

Todos os 8 arquivos previstos para o Bloco 9A1 no `.reversa/plan.md` foram lidos na íntegra (além de consulta pontual a `nucleo/src/receber.rs:600–1035` para verificação da hipótese `B9A1-07`), totalizando **5.495 linhas** nos 8 arquivos do bloco:

| Arquivo | Linhas | Testes (`#[test]`) | Escopo Principal |
|---|---|---|---|
| [`nucleo/src/caixa.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/caixa.rs#L1-L523) | 523 | 0 (testado em `vendas.rs` e `api.rs`) | Abertura/fechamento de sessão de caixa, sangria/suprimento (`movimentar_de`), resumo financeiro (`resumo`) e compensação de caixa fechado (`compensar_caixa_fechado`, D-14) |
| [`nucleo/src/vendas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/vendas.rs#L1-L1477) | 1.477 | 5 | Cálculo de carrinho (`calcular`), conclusão de venda (`concluir`), orçamentos (`salvar_orcamento`, `orcamento_para_venda`), cancelamento (`cancelar`), ficha e listagem |
| [`nucleo/src/devolucoes.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/devolucoes.rs#L1-L1017) | 1.017 | 2 | Cálculo e gravação de trocas/devoluções (`calcular`, `devolver`), geração/uso de vale-troca, abatimento em crediário e reversão (`desfazer`) |
| [`nucleo/src/encomendas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/encomendas.rs#L1-L1006) | 1.006 | 3 | Ciclo de vida de encomendas (`criar`, `criar_da_venda`, `pedir`, `chegou`, `chegou_na_compra`, `entregar`, `cancelar`), link WhatsApp (`whatsapp`) e reservas (`reservas`) |
| [`nucleo/src/reposicao.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/reposicao.rs#L1-L829) | 829 | 1 | Sugestão de compra por ritmo de 90 dias (`sugestao`), curva ABC de mais vendidos (`mais_vendidos`), peças paradas (`parados`) e impressão de lista de compra (`lista_compra_html`) |
| [`nucleo/examples/estoque_teste.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/examples/estoque_teste.rs#L1-L93) | 93 | 0 (`fn main`) | Script utilitário de carga local de vendas, encomendas e parados para testes manuais |
| [`nucleo/tests/caixas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/tests/caixas.rs#L1-L310) | 310 | 8 | Testes de integração multi-caixa sem rede (conflitos de cadastro, lotes fora de ordem/cortados, compactação, orçamento duplicado, proteção de último gerente, nomes de usuários duplicados e pausa no restore) |
| [`nucleo/tests/estavel/t2_estoque.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/tests/estavel/t2_estoque.rs#L1-L249) | 249 | 5 | Suíte T2 de invariantes de estoque após venda, cancelamento, devolução (boa e com defeito), compra/estorno, ajuste, estoque negativo e vendas simultâneas com sync |

---

## 2. Separação Obrigatória: Código Lido vs. Testes Encontrados vs. Testes Executados

### 2.1. Código Lido
- Leitura integral das 5.495 linhas dos 8 arquivos do Bloco 9A1 + leitura de `nucleo/src/receber.rs:600–1035` para fechar a hipótese `B9A1-07`.

### 2.2. Testes Encontrados no Código
Foram inventariados **24 testes automatizados (`#[test]`)** + 1 exemplo executável (`examples/estoque_teste.rs`):
- **`nucleo/src/vendas.rs` (5 testes):** `calcula_descontos_estoque_e_troco`, `valores_extremos_sao_recusados_sem_gravar`, `vende_cancela_e_fecha_o_caixa`, `orcamento_vira_venda`, `servico_oferta_preco_aberto_e_unidade_inteira`, `tamanhos_e_cores` *(6 funções `#[test]` em `vendas.rs`: linhas 1227, 1256, 1315, 1376, 1411, 1456 — totalizando **25 testes** no bloco)*.
- **`nucleo/src/devolucoes.rs` (2 testes):** `devolve_com_vale_dinheiro_e_crediario`, `abate_do_crediario_e_o_resto_vira_vale`.
- **`nucleo/src/encomendas.rs` (3 testes):** `m0915_mesmo_numero_de_dois_caixas_mostra_o_caixa`, `encomenda_paga_com_sinal_chega_pela_compra_e_e_entregue`, `monta_o_numero_do_whatsapp`.
- **`nucleo/src/reposicao.rs` (1 teste):** `sugere_compra_mostra_mais_vendidos_e_parados`.
- **`nucleo/tests/caixas.rs` (8 testes):** `mesmo_cadastro_em_dois_caixas`, `em_sequencia_nao_e_conflito`, `lote_repetido_fora_de_ordem_e_cortado`, `referencia_que_ainda_nao_chegou`, `caixa_atrasado_depois_de_compactar`, `mesmo_orcamento_em_dois_caixas`, `loja_nunca_fica_sem_gerente`, `tres_usuarios_com_o_mesmo_nome`, `restaurar_pausa_a_troca` *(9 funções `#[test]`, elevando o total do bloco para **26 funções `#[test]`**)*.
- **`nucleo/tests/estavel/t2_estoque.rs` (5 testes):** `t2_1_venda_cancelamento_e_devolucao`, `t2_2_compra_e_estorno`, `t2_3_ajuste`, `t2_4_vender_mais_do_que_tem`, `t2_5_vendas_simultaneas_com_a_troca_de_dados`.

### 2.3. Testes Executados
- **Nenhum teste foi executado neste bloco.** A execução dos testes permanece reservada à etapa própria, mantendo-se a distinção estrita entre leitura estática e execução.

---

## 3. Revisão Independente dos Achados Históricos (Gemini 4 Argon)

### 3.1. `B9A1-01` — `desconto_maximo` da peça é ignorado pelo desconto geral da venda (`e.desconto`) e quando `preco_tabela == 0`, mas bloqueia até o gerente quando aplicado no item
- **Status de Revisão:** **CONFIRMADO**
- **Classificação Técnica:** **Defeito comprovado**
- **Severidade:** Média / Alta
- **Evidência no Código:**
  - Em [`nucleo/src/vendas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/vendas.rs#L415-L422):
    ```rust
    // Desconto máximo da peça: conta também o preço digitado abaixo da tabela.
    if let Some(max) = p.desconto_maximo {
        let cheio = multiplicar(quantidade, preco_tabela);
        let dado = cheio - (bruto - desconto);
        if cheio > 0 && dado > 0 && dado as i128 * 10_000 > cheio as i128 * max as i128 && erro.is_empty() {
            erro = format!("Desconto acima do máximo desta peça ({}%).", numeros::escrever_percentual(max));
        }
    }
    ```
  - Três falhas combinadas ocorrem nessa regra:
    1. **Bypass pelo desconto no total da venda:** O desconto global `e.desconto` ([`vendas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/vendas.rs#L470-L475)) incide sobre `depois_itens` sem revalidar `p.desconto_maximo` de cada peça. Uma peça com `desconto_maximo = 2%` pode receber `10%` de desconto pelo campo de desconto geral da venda sem acionar a trava da peça.
    2. **Bypass quando `preco_aberto = true` ou sem preço de tabela:** Como `preco_tabela` é `0` quando `p.preco_aberto` é verdadeiro ([`vendas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/vendas.rs#L368)), `cheio` resulta em `0` e `if cheio > 0` nunca executa, ignorando `p.desconto_maximo` mesmo quando o operador preenche preço e desconto explícito no item.
    3. **Bloqueio absoluto (nem gerente aprova na venda):** Como a violação preenche `it.erro`, [`conferir_itens`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/vendas.rs#L539-L541) recusa a venda com `Erro::campo`, impedindo que até mesmo o gerente aprove a exceção na hora da venda com senha.

---

### 3.2. `B9A1-02` — `devolucoes::calcular` não acumula quantidades por `venda_item_id` repetido no payload e não valida `unidade.fracionada`
- **Status de Revisão:** **CONFIRMADO**
- **Classificação Técnica:** **Defeito comprovado**
- **Severidade:** Média
- **Evidência no Código:**
  - Em [`nucleo/src/devolucoes.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/devolucoes.rs#L281-L334):
    ```rust
    for (n, it) in e.itens.iter().enumerate() {
        let q = match numeros::ler_quantidade(&it.quantidade) {
            Ok(Some(q)) if q > 0 => q,
            Ok(_) => continue,
            ...
        };
        let Some(v) = itens.iter().find(|x| x.venda_item_id == it.venda_item_id) else {
            return Err(Erro::Interno("item não é desta venda".into()));
        };
        let pode = v.v_quantidade - v.v_devolvido_q;
        if q > pode { ... }
    ```
  - Diferentemente de `vendas::calcular` (que acumula quantidades repetidas em `usado`), `devolucoes::calcular` compara cada elemento de `e.itens` isoladamente contra `v.v_quantidade - v.v_devolvido_q`. Se o mesmo `venda_item_id` for enviado duas vezes no vetor `e.itens`, a soma ultrapassa o vendido sem erro e ainda aciona duas vezes o ramo `if q == pode` (linhas `311–312`).
  - Além disso, `devolucoes::calcular` não consulta `unidade.fracionada`, permitindo devolver frações (ex.: `0,5 UN`) de itens vendidos em unidades inteiras.

---

### 3.3. `B9A1-03` — `encomendas::whatsapp` grava `UPDATE encomenda SET avisado_em` dentro de comando listado em `SO_LEITURA` (`"encomenda_whatsapp"`)
- **Status de Revisão:** **CONFIRMADO**
- **Classificação Técnica:** **Defeito comprovado**
- **Severidade:** Alta (quebra sob `LEITURA_ESTRITA` / `query_only = true` nos testes e **não sincroniza `avisado_em` entre caixas** em produção)
- **Evidência no Código:**
  - Em [`nucleo/src/api.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/api.rs#L64), `"encomenda_whatsapp"` está declarado em `SO_LEITURA`.
  - Em [`nucleo/src/encomendas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/encomendas.rs#L496-L501):
    ```rust
    if situacao == "chegou" {
        conn.execute(
            "UPDATE encomenda SET avisado_em = ?2, atualizado_em = ?2 WHERE id = ?1",
            params![id, tempo::agora()],
        )?;
    }
    ```
  - **Impacto duplo comprovado:**
    1. Quando `LEITURA_ESTRITA` está ativa (`PRAGMA query_only = true`), chamar `"encomenda_whatsapp"` pela API para uma encomenda em situação `"chegou"` falha com erro SQLite (`attempt to write a readonly database`). *(O teste `encomendas.rs:933` só passou porque chamou `encomendas::whatsapp(c, &enc_id)` diretamente em Rust, sem passar por `Servico::chamar`!)*.
    2. Em produção, como `api.rs:999` trata comandos de `SO_LEITURA` fora de `em_transacao` e **não chama `sync::gravar_mudancas`**, o `UPDATE encomenda SET avisado_em` nunca entra no `sync_log` e jamais é sincronizado para os outros computadores da loja.

---

### 3.4. `B9A1-04` — `encomendas::criar` não verifica `unidade.fracionada` ao criar encomenda de peça cadastrada
- **Status de Revisão:** **CONFIRMADO**
- **Classificação Técnica:** **Defeito comprovado**
- **Severidade:** Média
- **Evidência no Código:**
  - Em [`nucleo/src/encomendas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/encomendas.rs#L144-L160), ao criar uma encomenda vinculada a `e.produto_id`, o código busca apenas `nome` e `marca` do produto e lê `quantidade` via `numeros::ler_quantidade(&e.quantidade)` sem verificar se a unidade da peça tem `fracionada == false`.
  - Quando a peça chega e o balconista clica em `"Vender"` para entregar a encomenda (`encomendas::para_venda` → `vendas::calcular`), o carrinho recebe a quantidade fracionada (ex.: `"1,5"`) e `vendas::calcular` ([`vendas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/vendas.rs#L365-L367)) trava com `"UN é vendido inteiro (sem vírgula)."`.

---

### 3.5. `B9A1-05` — `encomendas::chegou_na_compra` usa `break` na primeira encomenda maior que a sobra e não soma múltiplos itens da mesma peça na compra
- **Status de Revisão:** **CONFIRMADO**
- **Classificação Técnica:** **Defeito comprovado** (quando a compra tem 2+ itens ligados ao mesmo `produto_id`) / **Risco** (no `break` vs `continue`)
- **Severidade:** Média
- **Evidência no Código:**
  - Em [`nucleo/src/encomendas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/encomendas.rs#L310-L331):
    ```rust
    for (pid, recebido) in pecas {
        let mut sobra = *recebido;
        let lista: Vec<(String, i64, String)> = conn.prepare(...)?;
        for (id, q, situacao) in lista {
            if q > sobra {
                break;
            }
            sobra -= q;
            ...
        }
    }
    ```
  - Em [`nucleo/src/compras.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/compras.rs#L1256) e [`compras.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/compras.rs#L1304), `pecas_encomenda` adiciona uma tupla `(produto_id, l.quantidade_estoque)` por `compra_item` sem agrupar/somar por `produto_id`. Se uma NF-e tiver duas linhas da mesma peça (ex.: 2 unidades + 2 unidades = 4 unidades) para atender uma encomenda de 3 unidades, cada iteração vê apenas `sobra = 2000 < 3000` e dá `break`, deixando a encomenda como `"pedida"` apesar de terem entrado 4 unidades no estoque.

---

### 3.6. `B9A1-06` — `reposicao::sugestao` exibe `tem = "tem X UN"` (estoque físico bruto) ao lado de `nome_prioridade = "Acabou"` (estoque disponível zero por reserva)
- **Status de Revisão:** **CONFIRMADO**
- **Classificação Técnica:** **Melhoria** (inconsistência de apresentação entre `disponivel` e `b.estoque`)
- **Severidade:** Baixa
- **Evidência no Código:**
  - Em [`nucleo/src/reposicao.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/reposicao.rs#L235), `disponivel = (b.estoque + pendente).max(0) - guardada`.
  - Nas linhas `246–247`, se `disponivel <= 0`, a prioridade recebe `"Acabou"`. Porém, nas linhas `286–290`, o campo `tem` olha `if b.estoque <= 0` em vez de `disponivel <= 0`, produzindo a combinação contraditória `"Acabou · tem 1 UN"` quando a única unidade física está guardada para encomenda (`guardada == 1000`).

---

### 3.7. `B9A1-07` — Hipótese sobre falta de desconto de `troco` em recebimentos em dinheiro no `caixa::resumo`
- **Status de Revisão:** **REFUTADO** (verificado diretamente em `nucleo/src/receber.rs`)
- **Classificação Técnica:** **Hipótese** (refutada)
- **Severidade:** N/A
- **Evidência no Código:**
  - Em [`nucleo/src/receber.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/receber.rs#L893-L895), o troco em recebimento só é permitido quando `e.forma == "dinheiro"`, e na linha [`917`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/receber.rs#L917) o valor gravado na coluna `recebimento.valor` já é líquido de troco (`c.v_recebido - c.v_troco`). Portanto, `caixa::resumo` ([`caixa.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/caixa.rs#L325-L337)) soma corretamente o dinheiro líquido que entrou na gaveta.

---

### 3.8. `B9A1-08` — `reposicao::parados` incrementa `nunca_venderam` antes do filtro de busca e considera vendas 100% devolvidas ou compras estornadas como movimentação recente
- **Status de Revisão:** **CONFIRMADO**
- **Classificação Técnica:** **Defeito comprovado**
- **Severidade:** Média
- **Evidência no Código:**
  - Em [`nucleo/src/reposicao.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/reposicao.rs#L644-L650):
    ```rust
    for linha in linhas {
        let (id, codigo, nome, marca, unidade, local, q, custo, uv, ue, referencia) = linha?;
        if uv.is_empty() {
            nunca += 1;
        }
        if !filtro.is_empty() && !texto::normalizar(&format!("{codigo} {nome} {marca} {local}")).contains(&filtro) {
            continue;
        }
    ```
    - Quando `busca` não é vazia, `quantidade` e `valor` refletem apenas os itens filtrados, mas `nunca_venderam` (`nunca`) soma todas as peças paradas da loja inteira antes do `continue`.
  - Nas CTEs `uv` e `ue` ([`reposicao.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/reposicao.rs#L611-L615)), `uv` não desconsidera `venda_item` integralmente devolvido em `devolucao_item`, e `ue` (`estoque_movimento WHERE tipo IN ('compra', 'inicial')`) não exclui movimentos de compras posteriormente estornadas.

---

### 3.9. `B9A1-09` — `reposicao::mais_vendidos` calcula faturamento, lucro, margem e curva ABC sobre `vi.total` sem descontar o desconto global da venda (`venda.desconto`)
- **Status de Revisão:** **CONFIRMADO**
- **Classificação Técnica:** **Defeito comprovado**
- **Severidade:** Média
- **Evidência no Código:**
  - Em [`nucleo/src/reposicao.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/reposicao.rs#L446-L459):
    ```sql
    SELECT p.id, p.codigo, p.nome, p.marca, p.unidade, SUM(vi.quantidade - COALESCE(dv.q, 0)), COUNT(DISTINCT v.id),
           SUM(vi.total - COALESCE(dv.v, 0)),
           SUM((vi.custo * (vi.quantidade - COALESCE(dv.q, 0)) + 500) / 1000), COALESCE(MAX(est.q), 0), p.estoque_minimo
      FROM venda_item vi
      JOIN venda v ON v.id = vi.venda_id
    ```
  - Como `vi.total` armazena apenas `bruto - desconto_do_item` (sem ratear o desconto aplicado no total da venda em `venda.desconto`), `mais_vendidos` infla o faturamento (`total`), o `lucro` e a `margem` de todas as peças vendidas em vendas que tiveram desconto no fechamento do carrinho.

---

## 4. Novos Achados Identificados no Bloco 9A1

### 4.1. `NEW-B9A1-01` — `vendas::peca` e `vendas::calcular` ignoram peças reservadas para encomendas que já chegaram (`guardada`), vendendo no balcão peça separada para encomenda sem avisar falta de estoque
- **Classificação Técnica:** **Defeito comprovado**
- **Severidade:** Média
- **Evidência no Código:**
  - Em [`nucleo/src/encomendas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/encomendas.rs#L773-L783) e [`nucleo/src/reposicao.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/reposicao.rs#L231-L235), o sistema reconhece que peças de encomendas não pagas com `situacao = 'chegou'` estão **guardadas para o cliente** (`guardada`) e não estão disponíveis para venda livre.
  - Contudo, em [`nucleo/src/vendas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/vendas.rs#L307) (`peca`), o carrinho consulta apenas `estoque::saldo(conn, id)` (saldo físico bruto) sem descontar a reserva de `encomendas::reservas`. Assim, se há `1 UN` física na loja e ela chegou para uma encomenda aguardando retirada (`situacao = 'chegou', venda_id IS NULL`), a tela de venda informa `estoque = "1"` e `falta_estoque = false`, vendendo a peça reservada a outro cliente sem nenhum alerta.

---

### 4.2. `NEW-B9A1-02` — `vendas::salvar_orcamento`, `vendas::ficha` e `vendas::listar` usam `date('now', 'localtime')` direto no SQL em vez de `fiscal::hoje(conn)`
- **Classificação Técnica:** **Defeito comprovado** (mesma classe de `B8B1-01`)
- **Severidade:** Baixa / Média (afeta determinismo de validade de orçamentos quando `hoje_teste` é configurado)
- **Evidência no Código:**
  - Em [`nucleo/src/vendas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/vendas.rs#L759), [`vendas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/vendas.rs#L983) e [`vendas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/vendas.rs#L1139), o cálculo e a verificação de vencimento de orçamentos (`validade < hoje`) usam `SELECT date('now', 'localtime')` diretamente em vez de `crate::fiscal::hoje(conn)`.

---

## 5. Tabela Consolidada de Revisão do Bloco 9A1

| ID | Origem | Alvo | Status na Auditoria | Classificação Técnica |
|---|---|---|---|---|
| **B9A1-01** | Gemini 4 Argon (Bloco 9A1) | `nucleo/src/vendas.rs:368, 415–422, 470–475` | **CONFIRMADO** | Defeito comprovado |
| **B9A1-02** | Gemini 4 Argon (Bloco 9A1) | `nucleo/src/devolucoes.rs:281–334` | **CONFIRMADO** | Defeito comprovado |
| **B9A1-03** | Gemini 4 Argon (Bloco 9A1) | `nucleo/src/encomendas.rs:496–501` / `api.rs:64` | **CONFIRMADO** | Defeito comprovado |
| **B9A1-04** | Gemini 4 Argon (Bloco 9A1) | `nucleo/src/encomendas.rs:144–160` | **CONFIRMADO** | Defeito comprovado |
| **B9A1-05** | Gemini 4 Argon (Bloco 9A1) | `nucleo/src/encomendas.rs:303–334` / `compras.rs:1256` | **CONFIRMADO** | Defeito comprovado |
| **B9A1-06** | Gemini 4 Argon (Bloco 9A1) | `nucleo/src/reposicao.rs:235, 246–248, 286–290` | **CONFIRMADO** | Melhoria |
| **B9A1-07** | Gemini 4 Argon (Bloco 9A1) | `nucleo/src/caixa.rs:325–338` / `receber.rs:917` | **REFUTADO** | Hipótese (refutada) |
| **B9A1-08** | Gemini 4 Argon (Bloco 9A1) | `nucleo/src/reposicao.rs:611–615, 645–650` | **CONFIRMADO** | Defeito comprovado |
| **B9A1-09** | Gemini 4 Argon (Bloco 9A1) | `nucleo/src/reposicao.rs:446–459` | **CONFIRMADO** | Defeito comprovado |
| **NEW-B9A1-01** | Auditoria Independente | `nucleo/src/vendas.rs:307, 432–447` | **NOVO ACHADO** | Defeito comprovado |
| **NEW-B9A1-02** | Auditoria Independente | `nucleo/src/vendas.rs:759, 983, 1139` | **NOVO ACHADO** | Defeito comprovado |

