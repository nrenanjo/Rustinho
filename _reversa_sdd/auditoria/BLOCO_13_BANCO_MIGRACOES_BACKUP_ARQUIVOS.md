# BLOCO 13 — Auditoria de Banco de Dados, Migrações SQL (0001 a 0023), Backup, Caminhos e Arquivos

**Data:** 2026-11-02
**Escopo:** Fase 3 — Bloco 13 do Plano de Auditoria (`.reversa/plan.md`)
**Status:** CONCLUÍDO ✅ (Somente leitura — nenhum arquivo do projeto foi modificado)

---

## 1. Arquivos Lidos Integralmente neste Bloco

### 1.1. Módulos Rust e QML
| # | Arquivo | Linhas | Escopo Principal | Testes Embutidos (`#[test]`) |
|---|---|---|---|---|
| 1 | [banco.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/banco.rs) | 1–742 (742) | Abertura do banco SQLite, modo seguro (`preparar_seguro`, `query_only`), verificação `quick_check`/`integrity_check`, funções escalares SQLite (`normalizar`, `rustinho_id`, `rustinho_agora`, `rustinho_operacao`, `rustinho_historia`), motor de migração (`migrar`, `aplicar_migracao`, `migracao_sem_chaves`, `conferir_estrutura`), localização padrão do arquivo (`caminho_padrao`) | 8 funções `#[test]` (`linhas 468–742`) |
| 2 | [migracao.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracao.rs) | 1–652 (652) | Atualização segura da estrutura com cópia prévia (`VACUUM INTO`), máquina de estados persistida em `<banco>.migracao` (`Copiando`, `CopiaPronta`, `Migrando`, `Validando`, `Restaurando`, `Falhou`), conferência pós-migração de contagens, FKs e igualdade exata de `ordem_servico`/`ordem_servico_item` via `ATTACH DATABASE`, restauração automática em caso de falha | Testado via `testes_0915.rs`, `testes_rvi03.rs` e `t6_recuperacao.rs` |
| 3 | [backup.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/backup.rs) | 1–2191 (2.191) | Sistema de backup local e em nuvem (Google Drive via `rclone`): formatos 0/1/2/3 (`RUSTBKP1`), chave X25519 da loja + cadeados por gerente (Argon2id + ChaCha20-Poly1305 em blocos de 1 MB), proteção contra zip bomb (`limite_do_banco`), retenção (`para_apagar`), restauração com preservação de estado local (`Pendente`, `<banco>.restaurando`) | 12 funções `#[test]` (`linhas 1708–2191`) |
| 4 | [caminhos.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/caminhos.rs) | 1–1373 (1.373) | Gerenciamento de múltiplos caminhos de rede por par (`sync_endereco`), aprendizado por anúncio/conexão, caminhos digitados, expiração de 7 dias, ciclo de puxada (`ciclo`) com orçamento de tempo (`ORCAMENTO = 8s`) e rodízio de caixas fora do ar (`MORTOS_POR_CICLO = 4`) sem segurar lock do banco durante I/O de rede | 19 funções `#[test]` (`linhas 483–1373`) |
| 5 | [arquivos.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/arquivos.rs) | 1–565 (565) | Conversão de URLs `file://`, permissões Unix `0600`/`0700` (`so_do_usuario`), varredura limitada de pastas e descompactação de `.xml`, `.zip` (incluindo `.zip` dentro de `.zip`) e `.rar` (via `rars`) com orçamentos de bytes (`MAX_TOTAL`) e membros (`MAX_MEMBROS`) | 7 funções `#[test]` (`linhas 419–565`) |
| 6 | [arquivos_teste_rar.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/arquivos_teste_rar.rs) | 1–2 (2) | Massas de teste RAR5 normal e sólido codificadas em Base64 | Incluído por `arquivos.rs:564` |
| 7 | [SecaoBackup.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoBackup.qml) | 1–661 (661) | Interface de configuração, acompanhamento, conexão ao Google Drive e restauração de backups | N/A (QML) |

### 1.2. Migrações SQL (`nucleo/src/migracoes/0001_inicial.sql` a `0023_rascunho_e_pendencias.sql`)
| # | Arquivo SQL | Linhas | Principais Tabelas / Alterações de Esquema |
|---|---|---|---|
| 1 | [0001_inicial.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0001_inicial.sql) | 1–145 | `config`, `usuario`, `tabela_preco`, `produto`, `produto_preco`, `produto_aplicacao`, `produto_equivalente`, `estoque_movimento`, `menu_uso`, `auditoria` |
| 2 | [0002_pessoas_compras_fiscal.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0002_pessoas_compras_fiscal.sql) | 1–164 | `pessoa`, `pessoa_veiculo`, `produto.{csosn,monofasico}`, `produto_fornecedor`, `compra`, `compra_item`, `regra_st` |
| 3 | [0003_vendas_caixa_sync.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0003_vendas_caixa_sync.sql) | 1–158 | `compra_item.baixar_preco`, remoção de UNIQUE em códigos/chaves para multi-caixa, `caixa_sessao`, `caixa_movimento`, `venda`, `venda_item`, `venda_pagamento`, `alteracao`, `sync_estado`, `sync_par` |
| 4 | [0004_crediario.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0004_crediario.sql) | 1–73 | `pessoa.dia_vencimento`, `conta_receber`, `recebimento`, `recebimento_parcela`, view `conta_receber_saldo` |
| 5 | [0005_financeiro.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0005_financeiro.sql) | 1–183 | `conta_loja`, `categoria`, `conta_pagar`, `pagamento`, view `conta_pagar_saldo`, `movimento_conta`, `caixa_movimento.conta_loja_id`, `cheque`, `usuario.comissao`, `comissao_fechamento`, `compra.gerar_pagar` |
| 6 | [0006_encomendas.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0006_encomendas.sql) | 1–46 | `encomenda` e índices compostos de `venda_item`, `venda` e `estoque_movimento` |
| 7 | [0007_devolucoes.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0007_devolucoes.sql) | 1–77 | `devolucao`, `devolucao_item`, `vale`, `venda_pagamento.vale_id`, view `vale_saldo` |
| 8 | [0008_oficina.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0008_oficina.sql) | 1–56 | `ordem_servico`, `ordem_servico_item` |
| 9 | [0009_cadastro.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0009_cadastro.sql) | 1–65 | `unidade`, `produto.{tipo,oferta_preco,oferta_ate,preco_aberto,grade,variacao,fiscal}` |
| 10 | [0010_gerente.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0010_gerente.sql) | 1–26 | `usuario.gerente`, `entrada_falha`, `sync_conflito` |
| 11 | [0011_backup_conflitos.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0011_backup_conflitos.sql) | 1–34 | `usuario.backup_acesso`, colunas extras de `sync_conflito`, `alteracao.base` |
| 12 | [0012_historia_das_alteracoes.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0012_historia_das_alteracoes.sql) | 1–80 | `alteracao.{ordem,historia,compactada}`, `sync_conflito.{base_perdeu,historia_perdeu,certeza}`, `sync_resolucao`, `sync_referencia`, `sync_par.{marca,geracao}` |
| 13 | [0013_faixas_de_codigo.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0013_faixas_de_codigo.sql) | 1–54 | `faixa_codigo`, `produto_codigo_antigo`, índices numéricos de código e `codigos_legado` |
| 14 | [0014_codigo_confirmado.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0014_codigo_confirmado.sql) | 1–15 | `produto.codigo_validado` |
| 15 | [0015_codigo_das_pessoas.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0015_codigo_das_pessoas.sql) | 1–9 | Índices `pessoa_codigo_numero` e `pessoa_sem_codigo` |
| 16 | [0016_codigo_das_pessoas_confirmado.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0016_codigo_das_pessoas_confirmado.sql) | 1–10 | `pessoa.codigo_validado` |
| 17 | [0017_referencias_com_prova.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0017_referencias_com_prova.sql) | 1–24 | `sync_par.completo_em`, `sync_referencia.{motivo,aceita_por,aceita_em,aceite_motivo}` |
| 18 | [0018_situacao_da_senha.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0018_situacao_da_senha.sql) | 1–14 | `senha_situacao` |
| 19 | [0019_caminhos_da_rede.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0019_caminhos_da_rede.sql) | 1–25 | `sync_endereco` + migração dos endereços existentes de `sync_par` |
| 20 | [0020_verificacao_da_nfe.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0020_verificacao_da_nfe.sql) | 1–19 | `compra.{xml_verificacao,xml_prova,autorizacao,autorizacao_dados}` |
| 21 | [0021_dre_gerencial.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0021_dre_gerencial.sql) | 1–44 | `categoria.dre_grupo`, `conta_pagar.competencia`, `meta` |
| 22 | [0022_caixa_fechado.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0022_caixa_fechado.sql) | 1–21 | `venda.cancelado_apos_fechar`, `recebimento.estornado_apos_fechar`, `devolucao.desfeita_apos_fechar`, `caixa_movimento.{origem_tipo,origem_id}`, `movimento_conta.origem_tipo` |
| 23 | [0023_rascunho_e_pendencias.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0023_rascunho_e_pendencias.sql) | 1–124 | Recriação de `ordem_servico` com `'rascunho'` (`-- rustinho: sem_chaves_estrangeiras`), `alteracao.operacao_id`, `sync_pendencia`, `sync_par.{esquema,recursos,compativel_em}`, `menu_uso.fixado_em` |

**Total auditado no Bloco 13:** 30 arquivos (6 Rust + 1 QML + 23 SQL) | **7.552 linhas** lidas integralmente | **46 funções `#[test]`** inspecionadas no código-fonte.

---

## 2. Revisão de Achados e Requisitos Históricos Relacionados

| ID / Referência Histórica | Descrição Original | Veredito no Bloco 13 | Evidência no Código-Fonte Atual (0.9.17) |
|---|---|---|---|
| **RVI-03 / FALHA-06 / A3** (Modo seguro ao abrir banco corrompido ou ao falhar cópia pré-migração) | Conferir integridade antes de qualquer escrita (`journal_mode`, migração, gatilhos, backup); abrir em `query_only = true` se houver defeito; preservar cópias `falha-migracao-*.db` com sufixo único. | **CONFIRMADO** | Implementado em [banco.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/banco.rs#L73-L97) e [migracao.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracao.rs#L331-L434). |
| **R-03 / F-11** (Proteção contra pastas gigantes, URLs malformadas e bombas de descompactação em ZIP/RAR e backups) | Limitar arquivos varridos, orçamento total de bytes/membros e tamanho real descomprimido em backups e importações XML. | **CONFIRMADO PARCIALMENTE** | Implementado em [backup.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/backup.rs#L1030-L1042) e [arquivos.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/arquivos.rs#L43-L122). Contudo, em `arquivos::de_rar` (`arquivos.rs:396–415`), o orçamento `orc.gastar(tamanho)` desconta o tamanho declarado no cabeçalho RAR em vez do tamanho real descomprimido, e `max_total_output_bytes` é reiniciado a cada membro (ver **`NEW-B13-03`**). |
| **PEND-10** (Múltiplos caminhos de rede por caixa e ciclo de puxada sem bloquear o SQLite) | Tabela `sync_endereco`, preferência por cabo, fallback para Wi-Fi, backoff de 8/16/30s, orçamento de 8s e máximo de 4 caixas mortos por ciclo sem segurar lock do banco. | **CONFIRMADO** | Implementado em [caminhos.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/caminhos.rs#L400-L481) e validado pelo teste `t35_operacoes_locais_continuam_durante_o_ciclo` (`caminhos.rs:868–908`). |

---

## 3. Novos Achados Descobertos no Bloco 13

### 3.1. Defeitos Comprovados

#### `[NEW-B13-01]` `backup::Pendente::ler` e `Pendente::aplicar` filtram apenas `sync::CONFIG_DA_LOJA` e esquecem `sync::PREFIXO_RETIRADO` (`"caixa_retirado:"`), corrompendo o estado de caixas retirados ao restaurar backups
- **Classificação:** **Defeito comprovado**
- **Severidade:** **Média**
- **Localização:**
  - [backup.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/backup.rs#L1585-L1596) (`Pendente::ler`)
  - [backup.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/backup.rs#L1626-L1636) (`Pendente::aplicar`)
  - Comparar com [sync.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/sync.rs#L308-L318) (`filtro_config` e `chave_da_loja`)
- **Evidência literal no código:**
  Em [sync.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/sync.rs#L316-L318), uma configuração pertence à loja (e não ao computador local) quando está em `CONFIG_DA_LOJA` **ou** começa com `PREFIXO_RETIRADO` (`"caixa_retirado:"`):
  ```rust
  pub fn chave_da_loja(chave: &str) -> bool {
      CONFIG_DA_LOJA.contains(&chave) || chave.starts_with(PREFIXO_RETIRADO)
  }
  ```
  Porém, em [backup.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/backup.rs#L1586-L1596) e [backup.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/backup.rs#L1628-L1636), `Pendente` usa apenas `sync::CONFIG_DA_LOJA`:
  ```rust
  fn ler(conn: &Connection) -> Resultado<Pendente> {
      let da_loja: Vec<&str> = sync::CONFIG_DA_LOJA.to_vec();
      ...
      if !da_loja.contains(&k.as_str()) {
          locais.push((k, v));
      }
  ```
  e ao aplicar após `conn.restore`:
  ```rust
  tx.execute("UPDATE sync_estado SET aplicando = 1 WHERE id = 1", [])?;
  let lista_loja = da_loja.iter().map(|c| format!("'{c}'")).collect::<Vec<_>>().join(", ");
  tx.execute(&format!("DELETE FROM config WHERE chave NOT IN ({lista_loja})"), [])?;
  tx.execute("UPDATE sync_estado SET aplicando = 0 WHERE id = 1", [])?;
  for (k, v) in &self.locais {
      config::gravar(&tx, k, v)?;
  }
  ```
- **Impacto Comprovado:**
  1. **Restauração em computador novo apaga todos os registros `caixa_retirado:*` do backup:** Como `DELETE FROM config WHERE chave NOT IN ({lista_loja})` apaga tudo que não está em `CONFIG_DA_LOJA` e o computador novo não tinha `caixa_retirado:*` em `self.locais`, todas as marcações de caixas retirados em `config` trazidas pelo backup são perdidas.
  2. **Restauração no mesmo computador impede desfazer a retirada de um caixa e re-propaga a retirada na rede:** Se um gerente retirar um caixa por engano e restaurar o backup anterior, `Pendente::ler` terá capturado `caixa_retirado:<id>` em `self.locais` e `Pendente::aplicar` o regravará com os gatilhos ativos (`aplicando = 0`), gerando uma nova `alteracao` que mantém o caixa retirado.

---

#### `[NEW-B13-02]` Confirmação estrutural em `0005_financeiro.sql` do defeito `NEW-B12-03`: `financeiro::fluxo` (`financeiro.rs:724–736`) referencia tabela (`conta_pagar_pagamento`) e coluna (`pago_em`) que nunca existiram em nenhuma das 23 migrações SQL
- **Classificação:** **Defeito comprovado** (evidência de esquema SQL completando `NEW-B12-03`)
- **Severidade:** **Alta**
- **Localização:**
  - [0005_financeiro.sql](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/migracoes/0005_financeiro.sql#L77-L97) (`CREATE TABLE pagamento`)
  - [financeiro.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/financeiro.rs#L724-L736) (`financeiro::fluxo`)
- **Evidência literal em `0005_financeiro.sql:77–86`:**
  ```sql
  -- Um pagamento de uma parcela (inteira ou em parte).
  -- valor = quanto abateu da parcela; saiu valor + juros - desconto.
  CREATE TABLE pagamento (
      id                 TEXT PRIMARY KEY,
      conta_pagar_id     TEXT NOT NULL REFERENCES conta_pagar (id),
      -- AAAA-MM-DD em que o dinheiro sai (cheque: o "bom para")
      data               TEXT NOT NULL,
      valor              INTEGER NOT NULL,
      juros              INTEGER NOT NULL DEFAULT 0,
      desconto           INTEGER NOT NULL DEFAULT 0,
  ```
- **Análise Completa:**
  A leitura integral das 23 migrações (`0001` a `0023`) comprova que:
  1. A tabela chama-se `pagamento` (jamais existiu `conta_pagar_pagamento`).
  2. A coluna da data do pagamento chama-se `data` (jamais existiu `pago_em`).
  3. O desembolso efetivo é `valor + juros - desconto` em centavos inteiros (`INTEGER`), enquanto `financeiro::fluxo` escreveu `COALESCE(SUM(g.valor), 0.0)` lendo `f64` sem somar `juros` nem subtrair `desconto`.

---

### 3.2. Riscos Identificados

#### `[NEW-B13-03]` `arquivos::de_rar` desconta do orçamento apenas o `unpacked_size` declarado no cabeçalho e reinicia `max_total_output_bytes` a cada membro
- **Classificação:** **Risco**
- **Severidade:** **Baixa**
- **Localização:** [arquivos.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/arquivos.rs#L396-L416) (`de_rar`)
- **Evidência e Análise:**
  Em `de_rar`, `orc.gastar(tamanho)` (linha 400) utiliza `m.meta.unpacked_size` antes de descompactar, sem descontar a diferença caso `b.len() as u64 > tamanho`. Além disso, `opcoes.max_total_output_bytes = Some(MAX_TOTAL)` (linha 406) é instanciado dentro do laço por membro, permitindo que um arquivo `.rar` sólido ou com `unpacked_size` subdeclarado descompacte até `MAX_TAMANHO` (20 MB) por membro sem esgotar `orc.bytes`.

#### `[NEW-B13-04]` `SecaoBackup.qml` chama `Api.chamar("backup_nuvem_lista")` de forma síncrona na thread de UI
- **Classificação:** **Risco**
- **Severidade:** **Baixa**
- **Localização:** [SecaoBackup.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/SecaoBackup.qml#L101-L114) (`buscarNuvem`) e [backup.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/backup.rs#L1389-L1403) (`nomes_na_nuvem`)
- **Evidência e Análise:**
  Ao clicar na pílula `"No Google Drive"` na seção Restaurar, `buscarNuvem()` executa `Api.chamar("backup_nuvem_lista")` na thread principal da interface gráfica. Como `nomes_na_nuvem()` invoca o processo externo `rclone lsf` com timeout de até **60 segundos** (`Duration::from_secs(60)`), uma conexão lenta ou sem internet congela a janela do aplicativo durante a espera (diferentemente de `backup_restaurar`, que usa `chamarGerenteDemorado` em thread separada).

---

## 4. Separação Rigorosa: Código Lido vs. Testes Encontrados vs. Testes Executados

1. **Código-fonte e SQL lidos integralmente:**
   - `nucleo/src/banco.rs` (742 linhas)
   - `nucleo/src/migracao.rs` (652 linhas)
   - `nucleo/src/backup.rs` (2.191 linhas)
   - `nucleo/src/caminhos.rs` (1.373 linhas)
   - `nucleo/src/arquivos.rs` (565 linhas)
   - `nucleo/src/arquivos_teste_rar.rs` (2 linhas)
   - `app/qml/SecaoBackup.qml` (661 linhas)
   - `nucleo/src/migracoes/0001_inicial.sql` a `0023_rascunho_e_pendencias.sql` (23 arquivos, 1.366 linhas)
2. **Testes encontrados no código-fonte (46 testes unitários lidos):**
   - `banco.rs`: 8 testes (`cria_estrutura_e_dispositivo` até `d14_migracao_22_nao_muda_o_que_existia`).
   - `backup.rs`: 12 testes (`nomes` até `nuvem_pelo_rclone`).
   - `caminhos.rs`: 19 testes (`t04_dois_enderecos_do_mesmo_caixa_sao_um_par` até `t16_todos_os_caminhos_caidos_e_volta_sozinho`).
   - `arquivos.rs`: 7 testes (`limite_total_dos_pacotes` até `limite_para_xml_solto_e_pasta`).
3. **Testes executados neste bloco:**
   - **Nenhum (`0`).** Conforme o plano de auditoria, a execução efetiva da suíte de testes ocorrerá separadamente no **Bloco 18**.

