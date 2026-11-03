# Análise de Código, Padrões e Qualidade — Rustinho 0.9.17

> Gerado pelo Reversa (Arqueólogo / Detetive) em 2026-11-02
> Baseado na leitura integral de 100% dos arquivos de código-fonte (`nucleo/`, `app/`, `android/`, `instalador/`, `identidade/`, `.github/`).

---

## 1. Padrões Arquiteturais Identificados

### 1.1. Arquitetura em Camadas com Núcleo Único (`rustinho-nucleo`)
- **Núcleo de Domínio (`nucleo/src/`):** Concentra 100% das regras de negócio, persistência SQLite (`rusqlite` 0.40 bundled), criptografia (`argon2`, `chacha20poly1305`, `x25519-dalek`, `ring`, `rustls-webpki`), parser/validador fiscal NF-e e sincronização LAN.
- **Despachante JSON Único (`nucleo/src/api.rs`):** A struct `Sessao` expõe `chamar(comando: &str, dados_json: &str) -> String`. Todas as chamadas da interface (Desktop e Android) passam por esse ponto único de entrada, garantindo aplicação uniforme de:
  - Autenticação de sessão (`exigir_usuario`);
  - Controle de acesso por papel (`gerente` vs. `caixa`) e tokens de uso único (`gerente_liberar` / `Supervisao`);
  - Idempotência (`NAO_REPETIR` + `operacao_feita` + `chaveOperacao`);
  - Bloqueio de escrita em Modo Seguro (`somente_leitura` quando `PRAGMA quick_check` falha);
  - Registro de auditoria imutável (`auditoria::registrar`).
- **Ponte FFI Fina (`app/src/ponte.rs` e `android/rust/src/ponte.rs`):** Usa `cxx-qt` 0.10 para expor `PonteRust` (`QObject`) ao QML com `chamar`, `chamarAsync` (fila assíncrona em thread separada com sinal `respostaPronta(id, resposta)`), `escolherPasta`, `salvarPdf` e `prepararImpressao`.
- **Espelhamento Desktop ↔ Android:** As pastas `qml/`, `base/`, `cpp/`, `fontes/`, `icone/`, `qml_android/`, `src/ponte.rs` e `build.rs` são 100% idênticas entre `app/` e `android/rust/` (comprovado via `diff -rq` no Bloco 16).

### 1.2. Convenções de Domínio e Tipos
- **Valores Monetários em Centavos (`i64`):** Todos os preços, custos, descontos, saldos de caixa, parcelas de crediário e linhas do DRE são armazenados e calculados como inteiros `i64` em centavos (`R$ 1,00 = 100`), eliminando erros de ponto flutuante em somas contábeis.
- **Quantidades Inteira e Fracionária:** Quantidades inteiras (`i64`) coexistem com quantidades em milésimos (`quantidade_mili: i64`, onde `1.000 = 1 unidade`) para suportar unidades decimais (`KG`, `MT`, `LT`) mantendo aritmética exata de ponto fixo (`numeros::total_item_mili`).
- **Identificadores Ordenáveis (`UUIDv7`):** Entidades sincronizáveis usam `UUIDv7` textual (`banco::novo_id`), permitindo ordenação cronológica lexicográfica e geração offline sem colisão entre caixas.
- **Sincronização Event-Sourced por Gatilhos SQLite (`0016_sincronizacao.sql` e `sync.rs`):** Gatilhos `AFTER INSERT/UPDATE/DELETE` nas 31 tabelas sincronizáveis invocam a função SQLite customizada `rustinho_historia(tabela, chave, acao)` para gravar eventos na tabela `historia` apenas quando `sync.aplicando = 0`.

---

## 2. Pontos Fortes de Engenharia
1. **Proteção de Integridade e Modo Seguro (`banco.rs`, `migracao.rs`, `testes_modo_seguro.rs`, `testes_rvi03.rs`):** Abertura do banco verifica `PRAGMA quick_check` antes de qualquer migração; se houver corrupção física, recusa migrar e abre com `SQLITE_OPEN_READ_ONLY`, preservando os arquivos originais intocados.
2. **Validação Criptográfica ICP-Brasil Offline (`nfe_assinatura/`):** Implementação própria de C14N 1.0 inclusivo (`c14n.rs`), verificação RSA-SHA1/SHA256 (`ring`), cadeia X.509 (`rustls-webpki`) com âncoras da ICP-Brasil verificadas por SHA-512 em tempo de compilação e extração do CNPJ na OID `2.16.76.1.3.3` da `SubjectAltName`.
3. **Trava de Compilação para Âncoras de Teste (`nfe_assinatura/mod.rs:31`):** `#[cfg(all(feature = "ancoras-de-teste", not(test), not(debug_assertions)))] compile_error!(...)` impede que binários de release sejam compilados com certificados de teste.
4. **Overflow Checks em Release (`Cargo.toml`):** `overflow-checks = true` ativo no perfil `[profile.release]` tanto no desktop quanto no Android.

---

## 3. Fragilidades Estruturais e Padrões de Defeito Encontrados na Auditoria

### 3.1. Divergências entre Camadas de Validação (Regras de Negócio vs. SQL vs. UI)
- **Conversão de Unidades em Compras e Estoque (`B8B1-02`, `B9A1-05`):** A tabela `produto_unidade` armazena `fator`, mas `compras::confirmar`, `compras::confirmar_Direto` e `reposicao::sugestao_compra` somam/avaliam a quantidade bruta da nota sem multiplicar pelo fator da unidade quando a unidade comprada difere da unidade base da peça.
- **Consistência de Saldos e Preços (`B8B1-03`, `NEW-B8B2-01`):** `produtos::salvar` grava apenas `preco_1` (`tabela = 1`) e ignora `preco2`/`preco3`; `estoque::reconciliar` exige permissão de gerente mas usa `INSERT OR IGNORE` ao criar a linha faltante em `estoque_saldo` quando já há movimentos no kardex.
- **Orçamento Convertido vs. Cancelamento (`B9A1-02`, `NEW-B9A1-02`):** `vendas::cancelar` não reverte `orcamento.situacao = 'convertido'` e usa o operador SQLite `IS` em vez de `=` na busca de estorno no kardex.

### 3.2. Fragilidades em Sincronização Multi-Caixa (CRDT / LWW)
- **Referências Órfãs e Ressurreição (`NEW-B12-01`, `NEW-B12-02`):** `sync::desfazer_item_remoto` apaga `venda` sem limpar registros derivados (`parcela`, `caixa_movimento`, `estoque_movimento`); `sync::aplicar_evento` aplica LWW por linha sem reexecutar a máquina de estados de estoque/caixa.
- **Conflito de União em `config` (`NEW-B12-03`):** `unir_json` em `sync.rs` faz união aditiva de arrays/objetos em `config`, impedindo que a exclusão de uma categoria/meta DRE ou regra ST se propague se outro caixa editar a mesma chave.

### 3.3. Responsividade Mobile e Sobreposição de Textos (`B9B-*`, `NEW-B10-03`, `NEW-B15-03`)
- Tabelas densas de desktop (`SecaoDre.qml`, `SecaoFluxo.qml`, `TelaOrdens.qml`, `TelaVenda.qml`, `TelaCaixa.qml`, `SecaoVendidos.qml`, `SecaoParados.qml`, `SecaoComprar.qml`, `SecaoEncomendas.qml`) mantêm 5 a 8 colunas horizontais sem `Flickable` horizontal nem layout empilhado quando `Tema.celular` / `Tema.estreito` é verdadeiro (360dp).

