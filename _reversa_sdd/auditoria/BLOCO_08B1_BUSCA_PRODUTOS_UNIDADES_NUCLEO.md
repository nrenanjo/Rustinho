# BLOCO 8B1 — Auditoria de Código: Busca, Produtos, Códigos, Faixas, GTIN, Unidades e Etiquetas (Núcleo)

**Data da auditoria:** 2026-11-02
**Escopo:** Revisão independente de `nucleo/src/busca.rs`, `nucleo/src/produtos.rs`, `nucleo/src/unidades.rs`, `nucleo/src/codigo.rs`, `nucleo/src/gtin.rs`, `nucleo/src/faixas.rs` e `nucleo/src/etiquetas.rs`, confrontando contra o relatório histórico #20 do Gemini 4 Argon (`RELATORIO_AUDITORIA_BLOCO_8B1_GEMINI_4_ARGON.md`) e os contratos documentais dos Blocos 1 a 5.
**Restrição de imutabilidade:** Nenhum arquivo do código-fonte ou documentação preexistente foi modificado.

---

## 1. Arquivos Lidos Integralmente no Workspace

| # | Arquivo | Linhas no Workspace | Linhas no Gemini 4 (Bloco 8B1) | Status de Leitura |
|---|---|---|---|---|
| 1 | `nucleo/src/busca.rs` | `1–383` (383 linhas) | 383 linhas | 100% lido |
| 2 | `nucleo/src/produtos.rs` | `1–1387` (1.387 linhas) | 1.387 linhas | 100% lido |
| 3 | `nucleo/src/unidades.rs` | `1–154` (154 linhas) | 154 linhas | 100% lido |
| 4 | `nucleo/src/codigo.rs` | `1–156` (156 linhas) | 156 linhas | 100% lido |
| 5 | `nucleo/src/gtin.rs` | `1–31` (31 linhas) | 31 linhas | 100% lido |
| 6 | `nucleo/src/faixas.rs` | `1–3560` (3.560 linhas) | 3.560 linhas | 100% lido |
| 7 | `nucleo/src/etiquetas.rs` | `1–363` (363 linhas) | 363 linhas | 100% lido |
| — | **Total do Bloco 8B1** | **6.034 linhas** | **6.034 linhas** | **100% lido** |

---

## 2. Revisão Independente dos Achados Históricos do Gemini 4 Argon (Bloco 8B1)

*(Nota: no Relatório #20 do Gemini 4 Argon, os achados `B8B1-01` a `B8B1-12` foram numerados cobrindo simultaneamente os módulos Rust de `8B1` e as telas QML de `8B2`. Abaixo revisamos os achados pertencentes aos 7 módulos Rust de `8B1` e referenciamos os de interface que serão detalhados no Bloco 8B2.)*

| ID Histórico | Título / Descrição Original | Classificação Revisada | Natureza | Evidência Exata no Código-Fonte (`arquivo:linhas`) |
|---|---|---|---|---|
| **B8B1-01** | **Uso de `date('now', 'localtime')` do SQLite em `SQL_PRECO_1` e `SQL_EM_OFERTA` vs. `fiscal::hoje(conn)` em `produtos::ficha`:** Em `nucleo/src/produtos.rs:1060–1068`, as constantes SQL `SQL_PRECO_1` e `SQL_EM_OFERTA` (usadas por `busca::buscar` em `busca.rs:98–99`) avaliam a validade da oferta comparando `p.oferta_ate >= date('now', 'localtime')` direto no SQLite, enquanto `produtos::ficha` (`produtos.rs:951, 1042`) e `vendas::calcular` usam `fiscal::hoje(conn)`. | **CONFIRMADO** | **Risco** (baixo; divergência caso `fiscal::hoje` difira de `date('now', 'localtime')`) | `nucleo/src/produtos.rs:951, 1042, 1060–1068`; `nucleo/src/busca.rs:98–99`. |
| **B8B1-02** | **Cálculo incondicional de `lucro` a partir do preço em `produtos::validar` (`linha 308`) impede preço fixo (`lucro = NULL`) quando `custo > 0` e recalcula margem sem intenção:** Em `nucleo/src/produtos.rs:294–308`, quando `preco` é informado (`Some(valor)`), o match na linha `297` ignora `lucro` (`(Some(valor), _) => Some(valor)`) e a linha `308` grava `precos.push((tabela, valor, numeros::lucro_por_preco(custo, valor)))`. Consequentemente: (1) sempre que `custo > 0` e `valor >= custo`, `numeros::lucro_por_preco` retorna `Some(...)`, mesmo que o usuário tenha deixado o campo `% de lucro` em branco para travar o preço; (2) quando o custo da peça muda em `TelaPeca`, `manter_lucros` (`linhas 798–807`) retorna imediatamente (`if antes.custo != dados.custo { return; }`) e todas as tabelas passam a ter `lucro` recalculado pelo preço antigo; (3) por causa disso, em `compras::atualizar_custo` (`compras.rs:1293`), nenhuma tabela de peça com `custo > 0` e `preco >= custo` tem `lucro == None` (exceto se o preço for menor que o custo). | **CONFIRMADO** | **Defeito comprovado** (impossibilidade de manter tabela de preço sem `% de lucro` atrelado quando `custo > 0` e `preco >= custo`) | `nucleo/src/produtos.rs:289–317` (`validar`), `798–807` (`manter_lucros`), `824–839` (`tirar_custos_da_entrada`). |
| **B8B1-03** | **Limiar arbitrário `compacto.len() >= 3` e exibição redundante de `"Cód. interno"` no balão `achou` em `busca::buscar`:** Em `nucleo/src/busca.rs:193, 202–204, 215–248`: (1) `parece_codigo` exige `compacto.len() >= 3 && compacto.chars().any(|c| c.is_ascii_digit())`, de modo que códigos de 1 ou 2 dígitos ou códigos puramente alfabéticos nunca preenchem `item.achou`; (2) quando `compacto.len() >= 3` bate com `item.codigo` (ex.: código interno `"100"` ou `"100000"`), `achou_por` (`linha 217`) retorna `"Cód. interno"`, exibindo um balão redundante ao lado do nome da peça (cujo código interno já aparece na linha), ao contrário dos códigos `"1"` a `"99"` (como no teste da linha `381`, que espera `achou == ""`). | **CONFIRMADO** | **Defeito comprovado** (inconsistência visual entre códigos $< 3$ e $\ge 3$ dígitos e comentário do módulo `busca.rs:39–41`) | `nucleo/src/busca.rs:39–41, 193, 202–204, 215–248, 379–381`. |
| **B8B1-04** | **Filtro fino de ano de aplicação (`serve_no_ano`) aplicado em Rust após `LIMIT` do SQL em `busca::buscar`:** Em `nucleo/src/busca.rs:121–129, 150, 198–200`, quando a busca contém um ano de 4 dígitos (`1900..=2100`), o SQL filtra peças que tenham **qualquer** aplicação naquele ano com `LIMIT (limite * 5).min(5000)` (`linha 150`), e só depois o loop em Rust (`linha 198`) verifica se o ano pertence ao mesmo veículo das palavras buscadas (`serve_no_ano`). Se houver mais de `limite * 5` peças com aplicações naquele ano para outros carros antes das peças do carro buscado, resultados válidos podem ser cortados pelo `LIMIT` antes do filtro fino. | **CONFIRMADO** | **Risco** (baixo em lojas típicas; relevante em catálogos muito grandes) | `nucleo/src/busca.rs:121–129, 150, 198–200, 250–262`. |
| **B8B1-05** | **Divergência entre `unidades::listar` (`nucleo/src/unidades.rs:20–31`) e o Contrato Documental das 10 Unidades em 2 Linhas (Bloco 2):** O contrato documental do Bloco 2 especifica até 10 pílulas em 2 linhas de 5: 1ª linha com top 5 unidades por quantidade de **produtos ativos** (`p.ativo = 1`), e 2ª linha com top 5 unidades por **vendas líquidas nas últimas 24h móveis** (descontando devoluções vinculadas, sem repetir a 1ª linha), ambas ordenadas alfabeticamente. Já `unidades::listar` (`unidades.rs:20–31`) e `produtos::sugestoes` (`produtos.rs:1219–1221`) executam apenas `SELECT u.sigla, u.descricao, u.fracionada, (SELECT COUNT(*) FROM produto p WHERE p.unidade = u.sigla) AS n FROM unidade u ORDER BY n DESC, u.sigla` — contando produtos inativos e serviços, sem consulta de vendas líquidas nas últimas 24h e sem ordenação alfabética intra-linha. | **CONFIRMADO** | **Defeito comprovado** (descumprimento do contrato normativo do Bloco 2 no núcleo) | `nucleo/src/unidades.rs:20–31`; `nucleo/src/produtos.rs:1219–1221`. |
| **B8B1-06** a **B8B1-07**, **B8B1-09** a **B8B1-10** | Achados referentes a `EscolherUnidade.qml`, `TelaPeca.qml`, `TelaPecas.qml` e `estoque.rs`. | *(Ver Bloco 8B2)* | — | Auditados em detalhe no relatório `BLOCO_08B2_INTERFACE_PECAS_UNIDADES_ESTOQUE.md`. |
| **B8B1-08** | **Parser PNM (`codigo::ler_pnm`) assume exatamente 1 byte de espaço após `maximo` (`pos += 1`), deslocando os pixels se o cabeçalho usar `\r\n`:** Em `nucleo/src/codigo.rs:40`, após ler os 4 campos do cabeçalho P5/P6, o código faz `pos += 1; // um espaço depois do valor máximo`. Se o gerador da imagem PNM emitir quebra de linha Windows (`\r\n`) após `255`, `pos += 1` consome apenas `\r` e inclui `\n` (`0x0A`) como primeiro byte de pixel em `corpo` (`linha 53`), deslocando toda a imagem em 1 byte (e em PPM `P6` trocando os canais RGB para BRG). | **CONFIRMADO** | **Risco** (deslocamento de 1 byte com cabeçalho PNM em CRLF) | `nucleo/src/codigo.rs:16–63` (especialmente `linha 40`). |
| **B8B1-11** | **`etiquetas::montar` grava 5 chaves na tabela `config` dentro do comando `"impressao"`, que está classificado em `SO_LEITURA` (`api.rs:446`):** Em `nucleo/src/etiquetas.rs:182–187`, `montar()` executa 5 chamadas `config::gravar(conn, "etiqueta_...", ...)`. Como `"impressao"` está em `SO_LEITURA` (`api.rs:446`): (1) viola a invariante `R-04`/`RVI-01` de que comandos de `SO_LEITURA` não gravam no banco; (2) quando `LEITURA_ESTRITA == true` (`cfg(test)` ou `feature = "ancoras-de-teste"`, `api.rs:505, 725–754`) ou após pânico (`self.comprometido`), qualquer chamada a `"impressao"` com `tipo: "etiquetas"` **falha sempre** na linha `183` com `Erro::Interno("\"impressao\" está em SO_LEITURA mas tentou gravar no banco")` (inclusive mascarando a causa da falha no teste `leitor_e_etiqueta_com_codigo_repetido` em `api.rs:3483–3484`!); (3) em modo seguro (`em_modo_seguro()`), impede imprimir etiquetas porque tenta gravar em `config`; (4) se uma peça tiver bloqueio de etiqueta (`etiquetas.rs:196–198`), as 5 gravações em `config` nas linhas `183–187` já ocorreram fora de transação antes do erro! | **CONFIRMADO** | **Defeito comprovado** (violação de `SO_LEITURA` / `query_only` e gravação fora de transação antes da validação de bloqueio) | `nucleo/src/etiquetas.rs:182–198`; `nucleo/src/api.rs:446, 505, 725–754, 2416–2424, 3481–3484`. |
| **B8B1-12** | **Fragilidade na remoção de sufixo e atualização de `grade` fora de `salvar` em `produtos::criar_variacao`:** Em `nucleo/src/produtos.rs:1085–1134`: (1) quando a peça-base já tinha `variacao` mas o usuário havia editado `base.nome` (ou `base.nome` atingiu 120 caracteres na linha `1104` cortando parte do sufixo), `base.nome.strip_suffix(&sufixo)` (`linha 1123`) não encontra o sufixo e concatena a nova variação ao final do nome antigo; (2) na linha `1131`, `salvar` insere a nova variação com `grade = ""` e só na linha `1132` executa `UPDATE produto SET grade = ?1 WHERE id = ?2`. | **CONFIRMADO** | **Risco** (concatenação duplicada de sufixo de variação quando o nome foi editado ou truncado em 120 chars) | `nucleo/src/produtos.rs:1085–1134`. |

---

## 3. Novos Achados Identificados no Bloco 8B1 (`NEW-B8B1-01` a `NEW-B8B1-03`)

| ID | Classificação | Arquivo e Linhas | Descrição Técnica e Evidência |
|---|---|---|---|
| **NEW-B8B1-01** | **Risco** (integridade de quantidades fracionadas ao editar unidade existente) | `nucleo/src/unidades.rs:46–62` | **`unidades::salvar` permite desmarcar `fracionada` (`true` $\to$ `false`) em unidade já utilizada por peças com estoque fracionado:** A instrução `INSERT INTO unidade ... ON CONFLICT (sigla) DO UPDATE SET descricao = excluded.descricao, fracionada = excluded.fracionada` (`linhas 57–59`) não verifica se já existem produtos usando `sigla` com saldo fracionário (`estoque_movimento`) e não registra a alteração na tabela `auditoria`. |
| **NEW-B8B1-02** | **Defeito comprovado** (falso positivo em teste de `api.rs` por causa de `B8B1-11`) | `nucleo/src/etiquetas.rs:183–198` c/c `nucleo/src/api.rs:3481–3484` | **O teste `leitor_e_etiqueta_com_codigo_repetido` (`api.rs:3481–3484`) passa pelo motivo errado ao testar bloqueio de etiqueta via `"impressao"`:** O teste envia `"impressao"` com `tipo: "etiquetas"` para uma peça com código repetido e verifica apenas `assert_eq!(i["ok"], false)`. Na verdade, como `"impressao"` roda com `PRAGMA query_only = true` nos testes (`LEITURA_ESTRITA`), a chamada aborta na linha `183` de `etiquetas.rs` (`config::gravar`) com `Erro::Interno("\"impressao\" está em SO_LEITURA mas tentou gravar no banco")`, **nunca chegando** à verificação `if !e.bloqueio.is_empty()` na linha `196` de `etiquetas.rs`! |
| **NEW-B8B1-03** | **Melhoria** | `nucleo/src/etiquetas.rs:142–148` | **`etiquetas::etiqueta` só usa codificação EAN/UPC quando `barras.len() == 13`:** Enquanto `gtin::gtin_valido` (`gtin.rs:5–14`) e `produtos::validar` (`produtos.rs:264`) aceitam códigos de barras GTIN-8, GTIN-12 (UPC), GTIN-13 e GTIN-14, `etiquetas::etiqueta` (`linha 142`) só gera barras padrão EAN quando `barras.len() == 13`, caindo em `code128(&barras)` para GTIN-8, GTIN-12 e GTIN-14. (O Code 128 continua legível pelo leitor, mas difere do símbolo EAN-8/UPC impresso na embalagem.) |

---

## 4. Registro Separado de Testes (Código Lido vs. Testes Encontrados vs. Testes Executados)

### 4.1. Testes Unitários Lidos no Código-Fonte (45 funções `#[test]` no Bloco 8B1)

1. **`nucleo/src/busca.rs` (`linhas 345–383` — 1 função `#[test]`):**
   - `acha_pelo_codigo_do_fornecedor` (`352–382`).
2. **`nucleo/src/produtos.rs` (`linhas 1371–1387` — 1 função `#[test]`):**
   - `calcula_preco_e_lucro` (`1376–1386`).
3. **`nucleo/src/unidades.rs` (`linhas 119–154` — 2 funções `#[test]`):**
   - `lista_pronta_e_da_loja` (`125–137`), `junta_repetidas` (`140–153`).
4. **`nucleo/src/codigo.rs` (`linhas 97–156` — 3 funções `#[test]`):**
   - `le_ean_e_code128` (`118–123`), `foto_em_pgm_e_ppm` (`126–148`), `sem_codigo_e_lixo` (`151–155`).
5. **`nucleo/src/gtin.rs` (`linhas 16–31` — 1 função `#[test]`):**
   - `confere_digito` (`21–30`).
6. **`nucleo/src/etiquetas.rs` (`linhas 313–363` — 3 funções `#[test]`):**
   - `quantidade_enorme_nao_estoura` (`319–326`), `simbolos_do_code128_tem_11_modulos` (`329–334`), `codigos_iguais_aos_de_referencia` (`337–362`).
7. **`nucleo/src/faixas.rs` (`linhas 1734–3560` — 34 funções `#[test]` em 2 módulos de teste):**
   - Em `mod testes` (`16` funções `#[test]`):
     1. `a_dois_caixas_sem_rede_criam_pecas` (`1759–1814`)
     2. `b_computador_novo_entra_numa_loja_com_milhares_de_pecas` (`1818–1855`)
     3. `c_caixa_sem_rede_esgota_a_faixa` (`1861–1900`)
     4. `e_dois_caixas_com_codigos_que_ja_coincidiam` (`1907–1950`)
     5. `f_busca_leitor_etiqueta_e_venda_com_colisao` (`1956–2040`)
     6. `migracao_com_codigos_repetidos_e_acima_de_99999_sem_renumerar` (`2046–2142`)
     7. `reserva_concorrente` (`2148–2198`)
     8. `faixa_ja_usada_nao_volta` (`2204–2257`)
     9. `g_banco_copiado_com_o_arquivo_dos_codigos_para_outro_computador_nao_da_codigo` (`2298–2372`)
     10. `g_identidade_divergente_ou_nao_confirmada_bloqueia_e_so_o_registro_libera` (`2379–2426`)
     11. `g_caixa_registrado_como_novo_recebe_faixa_nova_e_nao_repete_codigos_do_original` (`2432–2516`)
     12. `h_loja_de_um_caixa_so_confirma_na_hora_e_o_segundo_caixa_recebe_os_codigos_validos` (`2553–2570`)
     13. `h_codigo_com_letras_espera_a_confirmacao_do_distribuidor` (`2577–2633`)
     14. `h_dois_caixas_offline_com_o_mesmo_codigo_so_um_confirma` (`2641–2709`)
     15. `h_confirmacao_e_atomica_no_distribuidor` (`2716–2740`)
     16. `i_distribuidor_perdido_outro_caixa_assume_sem_repetir` (`2748–2802`)
   - Em `mod testes_pessoas` (`18` funções `#[test]`):
     17. `j_dois_caixas_offline_nao_repetem_codigo_de_pessoa` (`2866–2906`)
     18. `j_codigos_de_antes_continuam_e_o_proximo_vem_depois` (`2912–2945`)
     19. `j_caixa_sem_faixa_deixa_codigo_a_definir_e_recebe_depois` (`2950–2987`)
     20. `j_codigo_digitado_e_mesmo_codigo_com_letras_em_dois_caixas` (`2993–3043`)
     21. `j_banco_copiado_nao_da_codigo_de_pessoa_ate_ser_registrado` (`3049–3085`)
     22. `j_restaurar_backup_nao_reaproveita_codigo_de_pessoa` (`3090–3100`)
     23. `j_faixa_esgotada_por_pessoas_pede_outra` (`3105–3119`)
     24. `j_caixa_comum_com_faixa_acabando_por_pessoas_pede_outra` (`3125–3144`)
     25. `j_gerente_define_pessoas_a_definir_de_outro_caixa` (`3149–3160`)
     26. `j_arquivo_ao_lado_guarda_a_contagem_das_pessoas` (`3165–3193`)
     27. `j_migracao_nao_renumera_pessoas_e_faixa_cobre_o_maior_codigo` (`3199–3251`)
     28. `k_loja_de_um_caixa_so_confirma_pessoa_na_hora` (`3260–3272`)
     29. `k_pessoa_com_letras_espera_a_confirmacao_do_distribuidor` (`3278–3328`)
     30. `k_mesmo_codigo_em_varios_caixas_so_um_confirma_e_o_gerente_resolve` (`3334–3387`)
     31. `k_confirmacao_de_pessoas_e_atomica_e_na_ordem_de_criacao` (`3397–3420`)
     32. `k_alterar_so_o_codigo_da_pessoa_grava_e_confirma_de_novo` (`3426–3451`)
     33. `k_banco_copiado_deixa_pessoa_com_letras_esperando` (`3457–3481`)
     34. `l_pend08_pecas_e_pessoas_criadas_no_banco_copiado` (`3495–3547`)

### 4.2. Testes Executados
- **Nenhum teste foi executado em runtime nesta etapa**, mantendo separação estrita entre leitura estática e execução, conforme diretriz do plano.

