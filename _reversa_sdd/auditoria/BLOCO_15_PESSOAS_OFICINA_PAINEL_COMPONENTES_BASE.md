# BLOCO 15 — Pessoas, Oficina, Painel, Loja, Configuração e Utilitários Base (Núcleo + UI)

**Fase Reversa:** 3 (`escavacao` — Auditoria Completa dos Módulos Não Cobertos Anteriormente)  
**Bloco do Plano:** Bloco 15  
**Status:** CONCLUÍDO ✅  

---

## 1. Escopo Auditado no Bloco 15

Este bloco auditou integralmente (100% das linhas lidas) todos os **14 módulos restantes de produção do Núcleo Rust** (`nucleo/src/`) e todos os **34 arquivos QML restantes** (`app/qml/` e `app/base/`), completando **100% de cobertura de leitura de todos os módulos de produção em `nucleo/src/` e de todas as 100 telas/componentes em `app/qml/` + 2 singletons em `app/base/`**:

### 1.1. Módulos do Núcleo (`nucleo/src/`)

| # | Arquivo | Linhas Lidas | Testes Internos (`#[test]`) |
|---|---------|--------------|-----------------------------|
| 1 | [`nucleo/src/pessoas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/pessoas.rs#L1-L1062) | 1–1062 (100%) | 4 testes (`linhas 953–1062`) |
| 2 | [`nucleo/src/oficina.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/oficina.rs#L1-L1139) | 1–1139 (100%) | 1 teste (`linhas 1034–1139`) |
| 3 | [`nucleo/src/painel.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/painel.rs#L1-L749) | 1–749 (100%) | 1 teste (`linhas 688–749`) |
| 4 | [`nucleo/src/loja.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/loja.rs#L1-L445) | 1–445 (100%) | — (testado em `testes_loja.rs`) |
| 5 | [`nucleo/src/config.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/config.rs#L1-L247) | 1–247 (100%) | — |
| 6 | [`nucleo/src/documento.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/documento.rs#L1-L193) | 1–193 (100%) | 4 testes (`linhas 152–193`) |
| 7 | [`nucleo/src/numeros.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/numeros.rs#L1-L376) | 1–376 (100%) | 9 testes (`linhas 235–376`) |
| 8 | [`nucleo/src/texto.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/texto.rs#L1-L84) | 1–84 (100%) | 3 testes (`linhas 59–84`) |
| 9 | [`nucleo/src/tempo.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/tempo.rs#L1-L75) | 1–75 (100%) | 2 testes (`linhas 57–75`) |
| 10 | [`nucleo/src/erro.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/erro.rs#L1-L69) | 1–69 (100%) | — |
| 11 | [`nucleo/src/menu.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/menu.rs#L1-L371) | 1–371 (100%) | 4 testes (`linhas 264–371`) |
| 12 | [`nucleo/src/lib.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/lib.rs#L1-L86) | 1–86 (100%) | — |
| 13 | [`nucleo/src/codigo.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/codigo.rs#L1-L156) | 1–156 (100%) | 3 testes (`linhas 97–156`) |
| 14 | [`nucleo/src/etiquetas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/etiquetas.rs#L1-L363) | 1–363 (100%) | 3 testes (`linhas 313–363`) |

### 1.2. Telas, Singletons e Componentes Base QML (`app/qml/` e `app/base/`)

| # | Arquivo | Linhas Lidas | Papel |
|---|---------|--------------|-------|
| 1 | [`app/qml/TelaPessoas.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaPessoas.qml#L1-L212) | 1–212 (100%) | Lista/busca de clientes e fornecedores |
| 2 | [`app/qml/TelaPessoa.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaPessoa.qml#L1-L615) | 1–615 (100%) | Ficha cadastral de pessoa física/jurídica e veículos |
| 3 | [`app/qml/LinhaPessoa.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/LinhaPessoa.qml#L1-L101) | 1–101 (100%) | Delegate de linha da lista de pessoas |
| 4 | [`app/qml/LinhaVeiculo.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/LinhaVeiculo.qml#L1-L54) | 1–54 (100%) | Linha de veículo na ficha de cliente |
| 5 | [`app/qml/TelaOrdens.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaOrdens.qml#L1-L251) | 1–251 (100%) | Lista e filtro de Ordens de Serviço da oficina |
| 6 | [`app/qml/TelaOrdem.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaOrdem.qml#L1-L968) | 1–968 (100%) | Formulário completo de OS, rascunho, etapas e entrega |
| 7 | [`app/qml/TelaPainel.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaPainel.qml#L1-L159) | 1–159 (100%) | Container das 6 abas do Painel gerencial |
| 8 | [`app/qml/Painel.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/Painel.qml#L1-L18) | 1–18 (100%) | Popup modal estilizado base |
| 9 | [`app/base/Api.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/base/Api.qml#L1-L120) | 1–120 (100%) | Singleton de comunicação JSON com `Sistema` e `chaveOperacao` |
| 10 | [`app/base/Tema.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/base/Tema.qml#L1-L131) | 1–131 (100%) | Singleton de cores, fontes, responsividade e modo sigilo |
| 11 | [`app/qml/Aparencia.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/Aparencia.qml#L1-L53) | 1–53 (100%) | Seletor sol/lua de modo claro/escuro |
| 12 | [`app/qml/AreaTexto.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/AreaTexto.qml#L1-L67) | 1–67 (100%) | Campo de texto multilinha |
| 13 | [`app/qml/Balao.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/Balao.qml#L1-L28) | 1–28 (100%) | Badge indicando motivo de match na busca |
| 14 | [`app/qml/Botao.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/Botao.qml#L1-L81) | 1–81 (100%) | Botão padrão (`principal`, `secundario`, `discreto`, `perigo`) |
| 15 | [`app/qml/Cabecalho.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/Cabecalho.qml#L1-L80) | 1–80 (100%) | Cabeçalho de tela com botão Voltar, Olho e ações |
| 16 | [`app/qml/CampoNumero.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/CampoNumero.qml#L1-L21) | 1–21 (100%) | Campo numérico com autoformatação via `numero_formatar` |
| 17 | [`app/qml/CartaoFuncao.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/CartaoFuncao.qml#L1-L103) | 1–103 (100%) | Cartão/pílula da tela inicial com pino de fixação |
| 18 | [`app/qml/CartaoNumero.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/CartaoNumero.qml#L1-L71) | 1–71 (100%) | Cartão de KPI numérico com suporte a `sensivel` |
| 19 | [`app/qml/Chip.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/Chip.qml#L1-L23) | 1–23 (100%) | Etiqueta arredondada de status |
| 20 | [`app/qml/Dialogo.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/Dialogo.qml#L1-L77) | 1–77 (100%) | Modal de confirmação |
| 21 | [`app/qml/Etapas.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/Etapas.qml#L1-L67) | 1–67 (100%) | Stepper horizontal em pílulas |
| 22 | [`app/qml/Faixa.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/Faixa.qml#L1-L46) | 1–46 (100%) | Container de seção com cantos arredondados |
| 23 | [`app/qml/GraficoBarras.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/GraficoBarras.qml#L1-L77) | 1–77 (100%) | Gráfico de barras verticais em pílulas |
| 24 | [`app/qml/Icone.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/Icone.qml#L1-L73) | 1–73 (100%) | Ícones vetoriais SVG em grade 24x24 |
| 25 | [`app/qml/IconeFuncao.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/IconeFuncao.qml#L1-L42) | 1–42 (100%) | Quadrado pastel com ícone da função |
| 26 | [`app/qml/Linha.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/Linha.qml#L1-L54) | 1–54 (100%) | `GridLayout` responsivo que dobra em `Tema.fino` |
| 27 | [`app/qml/Marcar.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/Marcar.qml#L1-L44) | 1–44 (100%) | `CheckBox` customizado |
| 28 | [`app/qml/Nota.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/Nota.qml#L1-L12) | 1–12 (100%) | Texto auxiliar em itálico com serifa |
| 29 | [`app/qml/Olho.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/Olho.qml#L1-L41) | 1–41 (100%) | Botão de alternância de `Tema.oculto` |
| 30 | [`app/qml/Pilulas.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/Pilulas.qml#L1-L63) | 1–63 (100%) | Seletor de abas em pílulas com quebra de linha (`Flow`) |
| 31 | [`app/qml/Rotulo.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/Rotulo.qml#L1-L13) | 1–13 (100%) | Rótulo em caixa-alta com espaçamento entre letras |
| 32 | [`app/qml/Selecao.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/Selecao.qml#L1-L119) | 1–119 (100%) | `ComboBox` estilizado com contraste corrigido (0.9.15) |
| 33 | [`app/qml/TextoErro.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TextoErro.qml#L1-L14) | 1–14 (100%) | Mensagem de erro que se oculta quando vazia |
| 34 | [`app/qml/Titulo.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/Titulo.qml#L1-L13) | 1–13 (100%) | Título em Cormorant Garamond com quebra no celular |

**Total auditado neste bloco:** 48 arquivos (`5.415` linhas de Rust + `3.912` linhas de QML = **`9.327` linhas de código-fonte**).

---

## 2. Verificação de Requisitos Históricos nos Módulos Auditados

- **`FALHA-01` / `V01a` (Identidade da Loja e Desistência Transacional da Junção):**
  - Confirmado em [`nucleo/src/loja.rs:115-180`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/loja.rs#L115-L180) (`decidir`) e [`loja.rs:337-434`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/loja.rs#L337-L434) (`desistir_da_juncao`) que:
    - Um caixa sem usuários fixa a primeira loja de quem recebe (`juntar_alvo`) e recusa pacotes de outras lojas;
    - Um caixa sem usuários nunca tem suas alterações aplicadas por caixas existentes (`ele_novo -> NadaAReceber`);
    - `desistir_da_juncao` restaura registros de fábrica a partir de `Banco::em_memoria()`, recusa desistência se houver registros de origem incerta (`incertos > 0`) e faz rollback integral se ocorrer falha antes do `commit`.
- **`FALHA-02` e Rascunho de OS (`0.9.15`):**
  - Confirmado em [`nucleo/src/oficina.rs:306-374`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/oficina.rs#L306-L374) e [`oficina.rs:456-597`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/oficina.rs#L456-L597) que a numeração de OS é por dispositivo (`WHERE dispositivo = ?1`), rascunhos são gravados com `numero = 0` e `situacao = 'rascunho'`, bloqueados se houver caixa antigo sem suporte (`sync::bloqueios_de_rascunho`), e convertidos in-place ao salvar.
- **`PEND-19` (CNPJ Alfanumérico — Receita Federal Julho/2026):**
  - Confirmado em [`nucleo/src/documento.rs:44-82`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/documento.rs#L44-L82) e [`nucleo/src/pessoas.rs:133-159`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/pessoas.rs#L133-L159) que os 12 primeiros caracteres aceitam letras maiúsculas (`A..Z` com peso `ASCII - 48 = 17..42`) e os 2 dígitos verificadores permanecem numéricos.
- **`FALHA-04` / `C-14` (`Api.qml`):**
  - Confirmado em [`app/base/Api.qml:43-90`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/base/Api.qml#L43-L90) que `Api.gravar` injeta `chaveOperacao` e `interpretar` valida estritamente `typeof r.ok === "boolean"`.

---

## 3. Novos Achados Identificados no Bloco 15

### `[NEW-B15-01]` — `painel::vendedores_periodo` ordena por vendas brutas (`SUM(v.total)`), mas exibe e calcula proporções sobre vendas líquidas de devoluções, podendo inverter o ranking e distorcer as barras
- **Classificação:** `Defeito comprovado`
- **Severidade:** Média
- **Evidência:**
  - [`nucleo/src/painel.rs:427-452`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/painel.rs#L427-L452):
    ```rust
    fn vendedores_periodo(conn: &Connection, de: &str, ate: &str) -> Resultado<Vec<Linha>> {
        let mut c = conn.prepare(
            "SELECT COALESCE(u.nome, 'Sem vendedor'), COUNT(*),
                    SUM(v.total - COALESCE((SELECT SUM(d.valor) FROM devolucao d WHERE d.venda_id = v.id AND d.cancelado_em IS NULL), 0))
               FROM venda v LEFT JOIN usuario u ON u.id = v.vendedor_id
              WHERE v.tipo = 'venda' AND v.situacao = 'concluida' AND v.criado_em >= ?3
                AND date(v.criado_em, 'localtime') BETWEEN ?1 AND ?2
              GROUP BY v.vendedor_id
              ORDER BY SUM(v.total) DESC",
        )?;
        let linhas: Vec<(String, i64, i64)> = c
            .query_map(params![de, ate, utc_desde(de)], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?
            .collect::<Result<_, _>>()?;
        let total = numeros::soma(linhas.iter().map(|l| l.2))?;
        let maior = linhas.first().map(|l| l.2).unwrap_or(0);
    ```
- **Análise Técnica:**
  1. A coluna 2 (`l.2`) seleciona o valor **líquido** de devoluções (`SUM(v.total - COALESCE((SELECT SUM(d.valor) FROM devolucao ...), 0))`), porém o `ORDER BY` na linha 435 ordena pelo valor **bruto** (`ORDER BY SUM(v.total) DESC`).
  2. Na linha 441, `maior` assume que `linhas.first()` possui o maior `l.2` (`linhas.first().map(|l| l.2)` em vez de `linhas.iter().map(|l| l.2).max()`).
  3. Se o 1º colocado em vendas brutas teve devoluções que deixaram seu líquido menor que o 2º colocado:
     - O ranking `"Vendedores no mês"` em `SecaoResumo.qml` exibe um vendedor com valor líquido menor acima de outro com valor líquido maior;
     - `maior` recebe o líquido menor do 1º item, fazendo com que `proporcao(t, maior)` sature em `1.0` para todos os vendedores com líquido superior a ele;
     - Vendedores com 100% das vendas devolvidas (`t == 0`) continuam aparecendo na lista com `"R$ 0,00"` e `"0%"` (não há `HAVING` nem `.filter(|l| l.2 > 0)`).

---

### `[NEW-B15-02]` — `painel::pagamentos` ignora `"credito_loja"` (vale-troca) nos totalizadores `"Entram na hora"` (`na_hora`) e `"A prazo / cartão"` (`depois`), divergindo do `"Total no período"`
- **Classificação:** `Defeito comprovado`
- **Severidade:** Baixa
- **Evidência:**
  - [`nucleo/src/painel.rs:656-686`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/painel.rs#L656-L686):
    ```rust
    pub fn pagamentos(conn: &Connection, dias: i64) -> Resultado<Pagamentos> {
        let (de, ate) = periodo(conn, dias)?;
        let (itens, total, vendas) = formas_periodo(conn, &de, &ate)?;
        let mut na_hora = 0i64;
        let mut depois = 0i64;
        // Recalcula pelas formas (já sem o troco).
        for f in caixa::FORMAS {
            ...
            match f.0 {
                "dinheiro" | "pix" | "debito" => na_hora += v,
                "credito" | "cheque" | "crediario" => depois += v,
                _ => {}
            }
        }
    ```
- **Análise Técnica:**
  - `caixa::FORMAS` ([`nucleo/src/caixa.rs:14-22`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/caixa.rs#L14-L22)) contém 7 formas: `"dinheiro"`, `"pix"`, `"debito"`, `"credito"`, `"crediario"`, `"cheque"` e `"credito_loja"` (`"Crédito na loja"`).
  - `formas_periodo` (`painel.rs:353–390`) inclui `"credito_loja"` na lista `itens` e na soma `total`.
  - Contudo, em `painel::pagamentos`, `"credito_loja"` cai no braço `_ => {}`. Quando existem vendas pagas parcial ou totalmente com vale-troca (`"credito_loja"`), a soma dos cartões `na_hora + depois` na aba `Painel › Pagamentos` (`SecaoPagamentos.qml`) fica menor que o cartão `total`.

---

### `[NEW-B15-03]` — `TelaOrdem.qml` permite imprimir um rascunho de OS sem salvá-lo quando reaberto da lista sem edições (`alterado === false`), imprimindo `"ORDEM DE SERVIÇO Nº 0"`
- **Classificação:** `Defeito comprovado`
- **Severidade:** Baixa
- **Evidência:**
  - [`app/qml/TelaOrdem.qml:211-219`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaOrdem.qml#L211-L219):
    ```qml
    function imprimir() {
        if ((alterado || nova) && !salvar())
            return
        const r = JSON.parse(Sistema.imprimir("os", osId))
        if (!r.ok)
            janela.avisar(r.erro, "erro")
        else if (r.dados.impresso)
            janela.avisar("Enviado para a impressora.", "ok")
    }
    ```
  - [`app/qml/TelaOrdem.qml:25-28`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaOrdem.qml#L25-L28):
    ```qml
    readonly property bool nova: osId === ""
    readonly property string situacao: ficha ? ficha.situacao : "orcamento"
    readonly property bool rascunho: situacao === "rascunho"
    ```
- **Análise Técnica:**
  - Quando o usuário abre um rascunho salvo anteriormente na lista (`TelaOrdens.qml`), `osId` já está preenchido (`nova === false`), `rascunho === true` e `alterado === false`.
  - Se o usuário clicar em `"Imprimir"` sem modificar nenhum campo, a condição `(alterado || nova)` é `false` (faltou `|| rascunho`). Com isso, `Sistema.imprimir("os", osId)` é chamado diretamente sobre o rascunho (`numero = 0`, podendo não ter cliente nem veículo), gerando um comprovante impresso com `"ORDEM DE SERVIÇO Nº 0"` e situação `"Rascunho"`.

---

### `[NEW-B15-04]` — `pessoas::gravar` executa dois comandos SQL na tabela `pessoa` por operação e `pessoas::garantir_pela_nota` atualiza papéis sem atualizar `atualizado_em`
- **Classificação:** `Defeito comprovado`
- **Severidade:** Baixa
- **Evidência:**
  - [`nucleo/src/pessoas.rs:453-535`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/pessoas.rs#L453-L535):
    - `gravar` executa `INSERT INTO pessoa` (`linhas 454–492`) ou `UPDATE pessoa` (`linhas 494–532`) sem a coluna `dia_vencimento` e, em seguida, executa `conn.execute("UPDATE pessoa SET dia_vencimento = ?2 WHERE id = ?1", params![id, d.dia_vencimento])?;` (`linha 535`).
  - [`nucleo/src/pessoas.rs:834-857`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/pessoas.rs#L834-L857):
    ```rust
    if cliente && !eh_cliente {
        conn.execute("UPDATE pessoa SET cliente = 1 WHERE id = ?1", [&id])?;
        ...
    }
    if !cliente && !eh_fornecedor {
        conn.execute("UPDATE pessoa SET fornecedor = 1 WHERE id = ?1", [&id])?;
        ...
    }
    ```
- **Análise Técnica:**
  1. Em `gravar`, o `UPDATE pessoa SET dia_vencimento = ?2` extra na linha 535 faz com que todo cadastro ou edição de cliente/fornecedor dispare o gatilho de sincronização da tabela `pessoa` **duas vezes** na mesma transação (`sync_pessoa_i` + `sync_pessoa_u`, ou `sync_pessoa_u` duas vezes), duplicando os registros de `pessoa` na tabela `alteracao`.
  2. Em `garantir_pela_nota`, quando um cadastro existente passa a acumular o segundo papel (`cliente = 1` ou `fornecedor = 1`) durante a importação de XML, o `UPDATE pessoa` não atualiza `atualizado_em = tempo::agora()`. O pacote de sincronização gerado em `alteracao` carrega o `atualizado_em` antigo, podendo perder a resolução LWW (`TEM_ATUALIZADO_EM`) em outros caixas.

---

### `[NEW-B15-05]` — `oficina::salvar` ao converter rascunho em orçamento executa dois `UPDATE ordem_servico` consecutivos (o primeiro sem `atualizado_em`) e mantém `criado_em` da criação do rascunho
- **Classificação:** `Defeito comprovado`
- **Severidade:** Baixa
- **Evidência:**
  - [`nucleo/src/oficina.rs:353-396`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/oficina.rs#L353-L396):
    ```rust
    if situacao == "rascunho" {
        let numero: i64 = conn.query_row(
            "SELECT COALESCE(MAX(numero), 0) + 1 FROM ordem_servico WHERE dispositivo = ?1",
            [dispositivo],
            |l| l.get(0),
        )?;
        conn.execute(
            "UPDATE ordem_servico SET situacao = 'orcamento', numero = ?2, dispositivo = ?3, caixa_nome = ?4,
                    criado_por = COALESCE(criado_por, ?5)
              WHERE id = ?1 AND situacao = 'rascunho'",
            params![id, numero, dispositivo, config::nome_caixa(conn)?, usuario_id],
        )?;
        ...
    }
    conn.execute(
        "UPDATE ordem_servico SET cliente_id = ?2, cliente_nome = ?3, telefone = ?4, placa = ?5, veiculo = ?6, km = ?7,
                defeito = ?8, diagnostico = ?9, observacoes = ?10, mecanico_id = ?11, previsao = ?12, desconto = ?13,
                atualizado_em = ?14
          WHERE id = ?1",
        ...
    )?;
    ```
- **Análise Técnica:**
  - Converter um rascunho em orçamento dispara duas vezes o gatilho `sync_ordem_servico_u` na mesma transação: o primeiro `UPDATE` grava um snapshot intermediário em `alteracao` com `situacao = 'orcamento'`, mas ainda com `cliente_nome`/`placa` antigos do rascunho e `atualizado_em` antigo; o segundo `UPDATE` grava o snapshot final.

---

### `[NEW-B15-06]` — `painel::pecas_periodo` não filtra itens de serviço (`vi.servico = 0`) no ranking `"Peças mais vendidas (mês)"` do Resumo do Painel
- **Classificação:** `Risco`
- **Severidade:** Baixa
- **Evidência:**
  - [`nucleo/src/painel.rs:392-407`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/painel.rs#L392-L407):
    ```sql
    SELECT COALESCE(p.nome, vi.descricao), COALESCE(p.marca, ''), COALESCE(p.unidade, vi.unidade),
           SUM(vi.quantidade - COALESCE(dv.q, 0)), SUM(vi.total - COALESCE(dv.v, 0))
      FROM venda_item vi JOIN venda v ON v.id = vi.venda_id
      ...
     WHERE v.tipo = 'venda' AND v.situacao = 'concluida' AND v.criado_em >= ?3
       AND date(v.criado_em, 'localtime') BETWEEN ?1 AND ?2
     GROUP BY COALESCE(vi.produto_id, vi.descricao)
    ```
- **Análise Técnica:**
  - Diferentemente de `dre::indicadores` ([`nucleo/src/dre.rs:581`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/dre.rs#L581), que filtra `AND vi.servico = 0`), `painel::pecas_periodo` não filtra `vi.servico = 0`. Serviços de mão de obra faturados na entrega de OS aparecem listados dentro do quadro `"Peças mais vendidas (mês)"` em `SecaoResumo.qml`.

---

## 4. Registro Separado: Código Lido vs. Testes Encontrados vs. Testes Executados

### 4.1. Código Lido Integralmente (100%)
- **14 módulos Rust (`5.415` linhas):** `pessoas.rs`, `oficina.rs`, `painel.rs`, `loja.rs`, `config.rs`, `documento.rs`, `numeros.rs`, `texto.rs`, `tempo.rs`, `erro.rs`, `menu.rs`, `lib.rs`, `codigo.rs`, `etiquetas.rs`.
- **34 arquivos QML (`3.912` linhas):** `TelaPessoas.qml`, `TelaPessoa.qml`, `LinhaPessoa.qml`, `LinhaVeiculo.qml`, `TelaOrdens.qml`, `TelaOrdem.qml`, `TelaPainel.qml`, `Painel.qml`, `Api.qml`, `Tema.qml`, `Aparencia.qml`, `AreaTexto.qml`, `Balao.qml`, `Botao.qml`, `Cabecalho.qml`, `CampoNumero.qml`, `CartaoFuncao.qml`, `CartaoNumero.qml`, `Chip.qml`, `Dialogo.qml`, `Etapas.qml`, `Faixa.qml`, `GraficoBarras.qml`, `Icone.qml`, `IconeFuncao.qml`, `Linha.qml`, `Marcar.qml`, `Nota.qml`, `Olho.qml`, `Pilulas.qml`, `Rotulo.qml`, `Selecao.qml`, `TextoErro.qml`, `Titulo.qml`.

### 4.2. Testes Encontrados e Lidos nos Módulos Auditados (34 testes `#[test]`)
- `pessoas.rs` (4 testes): `cadastra_busca_e_altera`, `confere_campos`, `pend19_cnpj_alfanumerico_no_cadastro`, `fornecedor_da_nota`.
- `oficina.rs` (1 teste): `os_do_orcamento_a_entrega`.
- `painel.rs` (1 teste): `resume_o_dia_o_mes_e_as_formas`.
- `documento.rs` (4 testes): `confere_cpf`, `confere_cnpj_numerico_e_alfanumerico`, `escreve_documentos`, `confere_placas`.
- `numeros.rs` (9 testes): `milhar_de_tres_em_tres`, `soma_nos_limites`, `fronteiras_do_numero`, `le_dinheiro`, `escreve_dinheiro`, `percentuais`, `quantidades`, `preco_e_lucro`, `anos`.
- `texto.rs` (3 testes): `normaliza_acentos_e_pontuacao`, `compacta_codigos`, `limpa_espacos`.
- `tempo.rs` (2 testes): `formato_da_hora`, `ids_unicos_e_ordenados`.
- `menu.rs` (4 testes): `ordem_fixa_sem_mais_usados_e_uso_nao_muda_a_ordem`, `fixar_poe_na_frente_na_ordem_em_que_foi_fixada`, `fixadas_de_antes_viram_a_pilula_unida_e_fiscal_sai`, `letra_e_disponibilidade`.
- `codigo.rs` (3 testes): `le_ean_e_code128`, `foto_em_pgm_e_ppm`, `sem_codigo_e_lixo`.
- `etiquetas.rs` (3 testes): `quantidade_enorme_nao_estoura`, `simbolos_do_code128_tem_11_modulos`, `codigos_iguais_aos_de_referencia`.

### 4.3. Testes Executados Neste Bloco
- **Nenhum teste foi executado neste bloco.** A execução da suíte completa de testes está isolada no **Bloco 18**.

