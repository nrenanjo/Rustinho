# Especificação Executável de Contratos da API (`api.rs`) — Rustinho 0.9.17

> Gerado pelo Reversa em 2026-11-02
> Extraído de `nucleo/src/api.rs` (4.550 linhas) e verificado com `nucleo/tests/contratos.rs` e `nucleo/tests/contratos_telas.rs`.

---

## 1. Envelope Geral de Requisição e Resposta

- **Assinatura Rust:** `pub fn chamar(&self, comando: &str, dados_json: &str) -> String`
- **Envelope de Sucesso:** `{"ok": true, ...campos_do_comando}`
- **Envelope de Erro:**
  - Erro geral / validação: `{"ok": false, "erro": "<mensagem>", "campo": "<opcional>"}`
  - Permissão / Supervisão necessária (`D-09`): `{"ok": false, "erro": "Só o gerente pode: <motivo>", "precisaGerente": true, "acaoGerente": "<escopo>", "motivoGerente": "<texto>"}`
  - Requisito de configuração ausente (`0.9.16`/`0.9.17`): `{"ok": false, "erro": "requisito_ausente", "requisito": {"id": "<id>", "tela": "<Tela>", ...}}`
  - Modo Seguro (`A3`/`RVI-03`): `{"ok": false, "erro": "O programa está em modo seguro (somente leitura)..."}`

---

## 2. Lista dos 52 Comandos com Idempotência Obrigatória (`NAO_REPETIR`)

Fonte: `nucleo/src/api.rs:64–110`. Todos exigem `"chaveOperacao"` não vazia no payload JSON:

| Domínio | Comandos em `NAO_REPETIR` |
|---|---|
| **Configuração e Loja** | `config_inicial`, `config_salvar`, `loja_salvar`, `conta_loja_salvar`, `conta_loja_ajustar`, `conta_loja_transferir`, `dre_metas_salvar`, `dre_classificar_salvar`, `fiscal_salvar`, `fiscal_regra_st_salvar`, `fiscal_certificado_importar`, `fiscal_certificado_remover` |
| **Usuários** | `usuario_salvar`, `usuario_senha` |
| **Peças, Estoque e Inventário** | `produto_salvar`, `estoque_ajustar`, `estoque_reconciliar`, `inventario_criar`, `inventario_fechar`, `importar_confirmar`, `importar_cadastros_confirmar`, `importar_csv_confirmar` |
| **Compras e Encomendas** | `compra_salvar`, `compra_confirmar`, `compra_estornar`, `encomenda_salvar`, `encomenda_situacao` |
| **Caixa, Vendas, Orçamentos e Trocas** | `caixa_abrir`, `caixa_movimento`, `caixa_fechar`, `movimento_lancar`, `orcamento_salvar`, `orcamento_converter`, `venda_concluir`, `venda_cancelar`, `devolucao_registrar` |
| **Crediário, Contas a Pagar e Cheques** | `receber_pagar`, `receber_pagar_cliente`, `pagar_lancar`, `pagar_pagar`, `pagar_lancar_recorrentes`, `cheque_depositar`, `cheque_compensar`, `cheque_devolver`, `cheque_repassar`, `cheque_resgatar` |
| **Pessoas e Oficina** | `pessoa_salvar`, `veiculo_salvar`, `ordem_salvar`, `ordem_situacao`, `ordem_converter_venda` |
| **Fiscal (Saída)** | `nota_emitir` |

---

## 3. Matriz de Supervisão do Gerente (`Supervisao` — `supervisao.rs` e `api.rs`)

| Escopo (`acao`) | Comandos que Aceitam / Exigem quando Operador é `caixa` | Consumo do Token |
|---|---|---|
| `preco_regra` | `produto_salvar` (alterar preço de venda ou violar margem mínima sobre custo) | Uso único (`usar`) |
| `estoque_ajuste` | `estoque_ajustar`, `inventario_fechar` | Uso único (`usar`) |
| `desconto` | `venda_concluir`, `orcamento_salvar` (desconto acima do limite do caixa ou venda no crediário acima do limite) | Uso único (`usar`) |
| `venda_cancelar` | `venda_cancelar` | Uso único (`usar`) |
| `devolucao` | `devolucao_registrar` | Uso único (`usar`) |
| `compra_estornar` | `compra_estornar` | Uso único (`usar`) |
| `config_loja` | `config_salvar`, `loja_salvar`, `fiscal_salvar`, `fiscal_regra_st_salvar`, `fiscal_certificado_importar`, `fiscal_certificado_remover` | Uso único (`usar`) |
| `config_loja_ver` | Consulta de `TelaConfiguracoes` (aba Loja / Impressão / Aparência pelo caixa) | Consulta (`conferir_sem_gastar`, válido por 120s) |
| `config_simultaneidade_ver` | Consulta da aba Simultaneidade (`Duplicatas`, `Histórico`, `Caixas`, `Backup`, `Fiscal`) e `backup_criar` pelo caixa | Consulta (`conferir_sem_gastar`, válido por 120s) |
| `config_gerente_ver` | Consulta da aba Gerente em `TelaConfiguracoes` pelo caixa | Consulta (`conferir_sem_gastar`, válido por 120s) |

