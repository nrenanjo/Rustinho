# BLOCO 06 — Reauditoria Independente: Notificações, Avisos e Tela Inicial

> **Fase do Plano:** Fase 2 — Reauditoria Independente dos Blocos 6 a 9B + Componentes Correlatos
> **Status:** CONCLUÍDO ✅
> **Data:** 2026-11-02
> **Modo de Operação:** Somente Auditoria (Regra de Imutabilidade Absoluta ativa: `allowLegacyEdits: false`)

---

## 1. Mapa Estrutural Exato dos 6 Arquivos Auditados no Workspace (`0.9.17`)

> **Descoberta de Reconciliação de Versão:** No workspace real do repositório (`Rustinho 0.9.17`), os arquivos `nucleo/src/avisos.rs`, `nucleo/src/testes_avisos.rs`, `app/qml/Notificacao.qml`, `app/qml/IconeFuncao.qml` e `app/qml/TelaInicio.qml` contêm a implementação da **revisão 0.9.16 de notificações em faixa e quadrado** (`1.003` linhas em `avisos.rs` e `252` linhas em `Notificacao.qml`), mais recente que o snapshot de `863` linhas / `209` linhas examinado pelo Gemini 4 Argon no PDF histórico.

| Arquivo | Linhas Exatas | Estruturas, Funções, Componentes e Testes (`nl -ba`) | Papel Confirmado no Código |
|---|---|---|---|
| `nucleo/src/avisos.rs` | `1.003` (`1–1003`) | • Constantes `INFORMATIVA` (`24`), `ATENCAO` (`25`), `CRITICA` (`26`)<br>• `pub struct Detalhe` (`30–33`), `pub struct Aviso` (`37–65`) e `impl Aviso` (`67–97`)<br>• `fn completar` (`102–142`), `pub fn uma_aberta` (`147–153`), `pub fn minimizar` (`160–171`), `pub fn esconder` (`176–186`), `fn escondida_hoje` (`191–193`), `fn plural` (`195–197`)<br>• `pub fn da_senha` (`203–223`), `pub fn do_dia` (`226–674`), `fn aviso_de_compatibilidade` (`678–744`), `fn curto` (`747–750`), `fn do_modo_seguro` (`753–774`), `pub fn so_do_modo_seguro` (`780–783`)<br>• `#[cfg(test)] mod testes` (`785–1003`): **7 funções `#[test]`** | Geração, categorização (`informativa`, `atencao`, `critica`), ordenação por criticidade, minimização persistente por computador (`aviso_minimizado.<id>`) e ocultação diária até meia-noite (`aviso_escondido.<id>`). |
| `nucleo/src/testes_avisos.rs` | `159` (`1–159`) | • Helpers `chamar` (`7–12`), `ok` (`14–18`), `aviso` (`20–22`), `loja` (`26–53`), `pasta` (`55–57`), `abertas` (`83–91`)<br>• **3 funções `#[test]`**: `n06_esconder_pela_api_respeita_a_categoria` (`61–80`), `n08_so_uma_nao_critica_aberta_mesmo_com_marcas_antigas` (`97–128`), `n07_minimizada_continua_depois_de_fechar_e_abrir` (`133–159`) | Testes de integração via `Servico::chamar` (`avisos_do_dia`, `aviso_minimizar`, `aviso_esconder`) com persistência em arquivo SQLite. |
| `app/qml/Notificacao.qml` | `252` (`1–252`) | • `Item { id: raiz }` (`19–252`), signals `abrir()` (`25`), `minimizar(bool minimizado)` (`26`), `esconder()` (`27`)<br>• Propriedades `categoria` (`29`), `critica` (`30`), `minimizado` (`32`), `lado` (`34`: `78` ou `92`), `temAcao` (`43`), `corFundo` (`52–53`), `corBorda` (`54–55`), `function linha()` (`58–65`)<br>• `Rectangle { id: cartao }` (`67–236`): `IconeFuncao` minimizado (`80–85`), `ColumnLayout { id: conteudo }` expandido (`88–176`) com `acaoLarga` (`123–130`), quadro `detalhes` (`134–166`) e `acaoEstreita` (`168–175`), `Image` triângulo de alerta (`180–192`), `DragHandler` (`194–220`), `TapHandler` (`223–234`)<br>• Animações `volta` (`238–242`) e `saida` (`243–251`) | Componente dual (faixa expandida ou quadrado minimizado `92×92`/`78×78` com ícone da função e triângulo de alerta). |
| `app/qml/IconeFuncao.qml` | `42` (`1–42`) | • `Rectangle { id: icone }` (`7–42`), mapa `cores` para 14 funções (`13–28`), fallback cinza (`29`), `Icone` interno (`35–41`) | Quadrado pastel (`48×48` ou `42×42` em `Tema.estreito`) com ícone vetorial da função para pílulas e notificações minimizadas. |
| `app/qml/Aviso.qml` | `47` (`1–47`) | • `Rectangle { id: aviso }` (`5–47`), posicionamento `y: parent.height - height - 30` (`10`), `Timer { id: tempo }` (`33–36`: `5500 ms` para `"erro"`, `3000 ms` demais), `TapHandler` (`38`), `function mostrar(texto, tipoAviso)` (`40–46`) | Toast rápido flutuante no rodapé da janela principal (`Main.qml`). |
| `app/qml/TelaInicio.qml` | `276` (`1–276`) | • `FocusScope { id: tela }` (`8–276`), propriedades `faixas` (`15`) e `quadrados` (`16`)<br>• Funções `carregar()` (`18–25`), `funcao(id)` (`27–29`), `esconderAviso(aviso)` (`31–37`), `minimizarAviso(aviso, minimizado)` (`42–52`), `abrirAviso(aviso)` (`55–78`), `fixar(item)` (`80–86`), `saudacao()` (`88–91`), `focarBusca()` (`93–95`), `buscar()` (`97–102`)<br>• Topo fixo com saudação, loja/caixa, `Aparencia`, `"Trocar usuário"` e `Campo { id: busca }` (`111–173`)<br>• `Flickable { id: rolagem }` (`176–254`) com `Repeater { model: tela.faixas }` (`199–211`), `Flow` + `Repeater { model: tela.quadrados }` (`212–229`) e `GridLayout` de `CartaoFuncao` (`234–252`)<br>• `Timer` de atualização a cada `20000 ms` (`266–275`) | Tela inicial do sistema: saudação, busca rápida de peças, área de notificações (faixas expandidas + fila de quadrados minimizados) e grade de funções. |

---

## 2. Revisão Independente de Todos os Achados Anteriores do Bloco 6 (`NOTIF-01` a `B6-08`)

### 2.1. Quadro Consolidado de Veredito Independente

| ID Histórico | Classificação Original (Gemini 4) | Veredito Independente no Código Atual (`0.9.17`) | Classificação Técnica Revisada | Linhas Exatas no Workspace |
|---|---|---|---|---|
| **NOTIF-01** | Defeito Comprovado | **CONFIRMADO** | **Melhoria / Inconsistência de Rótulo** | `nucleo/src/avisos.rs:132` vs. `365, 379–381, 386, 400, 404` |
| **NOTIF-02** | Corrigido na 0.9.17 | **CORRIGIDO NO CÓDIGO ATUAL** | **Corrigido (`"Ver caixas"` + `.secao("caixas")`)** | `nucleo/src/avisos.rs:133, 514` |
| **NOTIF-03** | Defeito Comprovado | **CONFIRMADO** | **Defeito Comprovado (Fluxo de Permissão/Navegação)** | `app/qml/TelaInicio.qml:24, 61–62, 273` + `app/qml/TelaConfiguracoes.qml` |
| **B6-01** | Defeito Comprovado (vs. doc) | **CONFIRMADO** | **Divergência Documental (Comportamento Intencional no Código)** | `app/qml/Aviso.qml:4, 10` |
| **B6-02** | Defeito Comprovado | **CONFIRMADO (Agravado na 0.9.16/0.9.17)** | **Defeito Comprovado** | `nucleo/src/avisos.rs:131, 753–783` + `app/qml/Notificacao.qml:43, 125, 170, 223–234` |
| **B6-03** | Defeito Comprovado | **CONFIRMADO (Agravado na 0.9.16/0.9.17)** | **Defeito Comprovado** | `app/qml/TelaInicio.qml:31–37, 49–51, 195, 213` |
| **B6-04** | Defeito Comprovado | **CONFIRMADO** | **Defeito Comprovado** | `nucleo/src/avisos.rs:118–120, 628, 650, 747–750` + `app/qml/Notificacao.qml:42, 113–114` |
| **B6-05** | Defeito Comprovado (vs. doc) | **CONFIRMADO** | **Divergência Documental (Design Implementado vs. Spec Externa)** | `app/qml/TelaInicio.qml:161–172` |
| **B6-06** | Melhoria Proposta | **PARCIALMENTE CONFIRMADO** | **Melhoria Proposta** | `nucleo/src/avisos.rs:135, 445–496, 753–774` |
| **B6-07** | Risco (`StyledText`) | **CONFIRMADO** | **Risco Preventivo Baixo** | `app/qml/Notificacao.qml:58–65, 113–114` |
| **B6-08** | Hipótese (`julianday('now')`) | **CONFIRMADO COMO HIPÓTESE** | **Hipótese de Baixo Impacto** | `nucleo/src/avisos.rs:617–623` |

---

### 2.2. Evidências Literais e Análise Detalhada de Cada Achado

#### 1. `[NOTIF-01]` — Rótulo `"Resolver no Gerente"` para avisos que abrem a seção `"duplicatas"`
- **Veredito:** **CONFIRMADO** (Classificação: **Melhoria / Inconsistência de Rótulo**).
- **Localização exata:** `nucleo/src/avisos.rs`, função `completar` (linha `132`) vs. `do_dia` (linhas `365`, `379–381`, `386`, `400`, `404`).
- **Trecho literal (`nucleo/src/avisos.rs:131–136` e `395–405`):**
  ```rust
              "backup" | "backup_nuvem" => "Ver backup",
              "conflitos" | "codigos" | "codigos_bloqueio" => "Resolver no Gerente",
              "referencias" | "referencias_atrasadas" | "outra_loja" => "Ver caixas",
  ```
  ```rust
              Aviso::simples(
                  "codigos",
                  &hoje,
                  "Códigos".into(),
                  format!("{} em mais de uma peça", plural(codigos, "código", "códigos")),
                  "o gerente resolve em Configurações › Simultaneidade › Duplicatas".into(),
                  "configuracoes",
              )
              .categoria(ATENCAO)
              .secao("duplicatas"),
  ```
- **Análise:** O botão de ação exibe `"Resolver no Gerente"`, mas a própria descrição (`linhas 379, 381, 400`) orienta ir em `"Duplicatas"` e o clique navega para `secao: "duplicatas"`.

#### 2. `[NOTIF-02]` — Rótulo e seção de `"outra_loja"`
- **Veredito:** **CORRIGIDO NO CÓDIGO ATUAL**.
- **Localização exata:** `nucleo/src/avisos.rs:133` (`"referencias" | "referencias_atrasadas" | "outra_loja" => "Ver caixas"`) e `514` (`.secao("caixas")`).

#### 3. `[NOTIF-03]` — Operador `Caixa` que clica em notificação de seção restrita de Configurações e cancela `PedirGerente`
- **Veredito:** **CONFIRMADO** (Classificação: **Defeito Comprovado**).
- **Localização exata:** `app/qml/TelaInicio.qml:24, 61–62, 273` (em conjunto com `app/qml/TelaConfiguracoes.qml`, que será detalhada no Bloco 7B).
- **Análise:** Como `"configuracoes"` tem `disponivel: true` para todos os perfis (para permitir acesso à seção `"loja"` / troca da própria senha), `avisos = a.dados.filter(x => funcao(x.funcao))` (`linha 24`) exibe ao operador `Caixa` todas as notificações de `"caixas"`, `"duplicatas"` e `"backup"`. Ao clicar no botão de ação (`linha 61–62`: `else if (aviso.secao) janela.abrir("TelaConfiguracoes", { secao: aviso.secao })`), `TelaConfiguracoes` é empilhada e solicita a senha do gerente; se o operador cancelar o modal `PedirGerente`, ele permanece dentro de `TelaConfiguracoes` na aba `"loja"` em vez de retornar à `TelaInicio`.

#### 4. `[B6-01]` — Posicionamento vertical de `app/qml/Aviso.qml`
- **Veredito:** **CONFIRMADO** (Classificação: **Divergência Documental — Comportamento Intencional no Código**).
- **Localização exata:** `app/qml/Aviso.qml:4, 10`:
  ```qml
  // Mensagem rápida que aparece embaixo e some sozinha.
  Rectangle {
      id: aviso
      property string tipo: "info"

      anchors.horizontalCenter: parent.horizontalCenter
      y: parent.height - height - 30
  ```
- **Análise:** O componente implementa intencionalmente um toast inferior (`30 px` acima da borda inferior), divergindo apenas do requisito externo do Bloco 3 que pedia `24 px` do topo.

#### 5. `[B6-02]` — `so_do_modo_seguro` retorna `Aviso` sem `completar`, deixando `acao`, `titulo_longo` e `resumo` vazios e suprimindo o botão de ação em `Notificacao.qml`
- **Veredito:** **CONFIRMADO E AGRAVADO NA 0.9.16/0.9.17** (Classificação: **Defeito Comprovado**).
- **Localização exata:** `nucleo/src/avisos.rs:124–138, 753–783` e `app/qml/Notificacao.qml:43, 125, 170, 223–234`.
- **Trecho literal (`nucleo/src/avisos.rs:753–783`):**
  ```rust
  fn do_modo_seguro(hoje: &str, defeito: &str) -> Aviso {
      let mut a = Aviso::simples(
          "banco_defeito",
          hoje,
          "Modo seguro: o banco de dados tem defeito".into(),
          "nada é gravado neste caixa".into(),
          "restaure um backup bom ou chame o suporte".into(),
          "configuracoes",
      )
      .categoria(CRITICA)
      .secao("backup");
      a.descricao = format!( ... );
      a
  }

  pub fn so_do_modo_seguro(conn: &Connection, defeito: &str) -> Vec<Aviso> {
      let hoje = fiscal::hoje(conn).unwrap_or_else(|_| crate::tempo::agora().chars().take(10).collect());
      vec![do_modo_seguro(&hoje, defeito)]
  }
  ```
- **Sustentação técnica:**
  1. `Aviso::simples` inicializa `titulo_longo`, `resumo` e `acao` com `String::new()` (`linhas 78–82`).
  2. Quando `so_do_modo_seguro` é chamado em fallback (`api.rs`, comando `"avisos_do_dia"`), ele não chama `completar(conn, a)` nem preenche `a.acao = "Ver backup".into()`.
  3. Na versão atual de `Notificacao.qml` (`0.9.16/0.9.17`), **clicar no corpo da faixa não abre mais o destino** (o `TapHandler` nas linhas `223–234` apenas alterna `minimizado` para não-críticas e ignora `raiz.critica`!). A única forma de navegar para `Configurações › Backup` a partir da notificação é clicando no botão `acaoLarga` (`linha 125`) ou `acaoEstreita` (`linha 170`), ambos controlados por `visible: raiz.temAcao` (`(aviso.acao || "") !== ""`, linha `43`). Com `acao == ""`, o botão desaparece e a notificação crítica de modo seguro fica **completamente sem ação clicável**!
  4. Adicionalmente, mesmo no fluxo normal (`do_dia`), `"banco_defeito"` não está listado na linha `131` de `completar` (`"backup" | "backup_nuvem" => "Ver backup"`), caindo em `_ => "Abrir"` (`linha 135`).

#### 6. `[B6-03]` — `esconderAviso` em `TelaInicio.qml` não remove o item de `tela.avisos`, mantendo contêineres visíveis e ressuscitando o aviso ao clicar em outro quadrado
- **Veredito:** **CONFIRMADO E APROFUNDADO** (Classificação: **Defeito Comprovado**).
- **Localização exata:** `app/qml/TelaInicio.qml:15–16, 31–37, 42–52, 195, 213`.
- **Trecho literal (`app/qml/TelaInicio.qml:31–52`):**
  ```qml
      function esconderAviso(aviso) {
          const r = Api.chamar("aviso_esconder", { id: aviso.id })
          if (!r.ok) {
              janela.avisar(r.erro, "info")
              carregar()
          }
      }
      ...
      function minimizarAviso(aviso, minimizado) {
          const r = Api.chamar("aviso_minimizar", { id: aviso.id, minimizado: minimizado })
          if (!r.ok) {
              janela.avisar(r.erro, "info")
              carregar()
              return
          }
          avisos = avisos.map(a => a.id === aviso.id ? Object.assign({}, a, { minimizado: minimizado })
                                   : a.categoria !== "critica" && !a.minimizado
                                     ? Object.assign({}, a, { minimizado: true }) : a)
      }
  ```
- **Sustentação técnica (2 efeitos comprovados):**
  1. Quando `r.ok === true` em `esconderAviso`, o item escondido continua dentro de `tela.avisos`. Se era a única notificação (ou o único quadrado em `tela.quadrados`), `tela.avisos.length > 0` (`linha 195`) e `tela.quadrados.length > 0` (`linha 213`) continuam `true`, preservando o espaço vazio (`spacing: 8`, `Layout.bottomMargin: 6`) acima das pílulas.
  2. **Efeito colateral crítico descoberto na revisão do código 0.9.16/0.9.17:** Se o usuário esconder uma notificação informativa (cujo delegate ficou oculto apenas localmente via `raiz.saindo = true` em `Notificacao.qml:250`) e, antes do `Timer` de 20s (`linha 267`), clicar em qualquer outro quadrado para expandi-lo ou minimizá-lo, `minimizarAviso` (`linhas 49–51`) reatribui `avisos = avisos.map(...)` a partir do array `avisos` que ainda contém o item escondido! Essa reatribuição reconstrói `tela.faixas` e `tela.quadrados`, recriando os delegates do `Repeater` com `saindo: false` e **fazendo a notificação escondida reaparecer na tela**!

#### 7. `[B6-04]` — `curto()` escapa HTML (`&amp;`, `&lt;`, `&gt;`), mas `Notificacao.qml` renderiza avisos `ATENCAO` com `Text.PlainText`
- **Veredito:** **CONFIRMADO** (Classificação: **Defeito Comprovado**).
- **Localização exata:** `nucleo/src/avisos.rs:118–120, 628, 650, 747–750` e `app/qml/Notificacao.qml:42, 113–114`.
- **Trechos literais:**
  - `nucleo/src/avisos.rs:747–750`:
    ```rust
    /// Motivo curto e seguro para a faixa (que aceita um pouco de HTML).
    fn curto(t: &str) -> String {
        let t: String = t.chars().take(90).collect();
        t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    }
    ```
  - `app/qml/Notificacao.qml:113–114`:
    ```qml
    text: raiz.categoria === "informativa" ? raiz.linha() : (raiz.aviso.descricao || "")
    textFormat: raiz.categoria === "informativa" ? Text.StyledText : Text.PlainText
    ```
- **Sustentação:** `"backup"` (`linha 636`) e `"backup_nuvem"` (`linha 653`) têm categoria `ATENCAO`. O erro do backup passa por `curto()` (`linhas 628, 650`), entra em `a.atraso` e é copiado para `a.resumo` e `a.descricao` em `completar()` (`linhas 110–120`). Como `Notificacao.qml:114` usa `Text.PlainText` para `categoria !== "informativa"`, caracteres como `<`, `>` ou `&` no erro de backup aparecem literalmente como `&lt;`, `&gt;` e `&amp;` na interface.

#### 8. `[B6-05]` — `Campo { id: busca }` em `TelaInicio.qml`
- **Veredito:** **CONFIRMADO** (Classificação: **Divergência Documental**).
- **Localização exata:** `app/qml/TelaInicio.qml:161–172` (`icone: "busca"`, `atalho: "F11"`).

#### 9. `[B6-06]` — Rótulo genérico `"Abrir"` em `"troca_parada"`, `"troca_tentando"`, `"troca_voltou"` e `"banco_defeito"`
- **Veredito:** **PARCIALMENTE CONFIRMADO** (Classificação: **Melhoria Proposta**).
- **Localização exata:** `nucleo/src/avisos.rs:124–138, 445–496, 753–774`.
- **Análise:** Na arquitetura atual (`0.9.16/0.9.17`), `resumo` é formatado uniformemente por `completar()` (`"{titulo_longo} — {partes}"`), não cabendo mais o prefixo antigo `"Simultaneidade ›"`. Contudo, confirma-se que `"troca_parada"`, `"troca_tentando"` e `"troca_voltou"` (todos com `.secao("caixas")`) e `"banco_defeito"` (`.secao("backup")`) caem no braço `_ => "Abrir"` (`linha 135`), enquanto os demais avisos da mesma seção usam `"Ver caixas"` ou `"Ver backup"`.

#### 10. `[B6-07]` — `Notificacao.qml` usa `Text.StyledText` em `linha()` sem escapar `aviso.texto` / `aviso.atraso`
- **Veredito:** **CONFIRMADO** (Classificação: **Risco Preventivo Baixo**).
- **Localização exata:** `app/qml/Notificacao.qml:58–65, 113–114`. Atualmente nenhuma notificação `INFORMATIVA` interpola texto livre de usuário em `texto` ou `atraso`, mas a concatenação direta de `<font color='...'>` exige cuidado caso novos avisos informativos sejam criados.

#### 11. `[B6-08]` — `dias_desde` em `avisos.rs` usa `julianday('now')` (UTC)
- **Veredito:** **CONFIRMADO COMO HIPÓTESE DE BAIXO IMPACTO** (`nucleo/src/avisos.rs:617–623`). Como `backup_ultimo` e `backup_desde` são gravados com `tempo::agora()` / `tempo::dias_atras()` (ambos UTC), a diferença `julianday('now') - julianday(?1)` compara instantes na mesma referência UTC.

---

## 3. Novos Achados Identificados Independentemente no Bloco 6 (`0.9.17`)

### 1. `[NEW-B6-01]` — Notificação `"receber"` usa `funcao: "vender"`, exibindo ícone/cor de Vender no quadrado minimizado e contabilizando uso de `"vender"` ao abrir `"TelaReceber"`
- **Classificação:** **Risco de Usabilidade / Coerência Visual**.
- **Localização exata:** `nucleo/src/avisos.rs:261` (`Aviso::simples("receber", &hoje, titulo, texto, atraso, "vender")`) e `app/qml/TelaInicio.qml:24, 63–65, 204–205, 221–222`.
- **Evidência literal (`app/qml/TelaInicio.qml:63–65`):**
  ```qml
          } else if (aviso.id === "receber") {
              Api.chamar("menu_usar", { id: f.id })
              janela.abrir("TelaReceber", {})
  ```
- **Análise:** Para permitir que operadores `Caixa` vejam o aviso de parcelas a receber mesmo quando `"financeiro"` está restrito, `avisos.rs:261` associa `"receber"` à função `"vender"`. Com a introdução dos quadrados minimizados na `0.9.16` (`TelaInicio.qml:221–222`), o quadrado minimizado de `"receber"` renderiza o ícone de **sacola/carrinho de vendas (`"vender"`)** em vez de um ícone de crediário/clientes, e clicar em `"Ver crediário"` incrementa o contador de uso de `"vender"` (`menu_usar`) embora abra `TelaReceber`.

---

## 4. Registro Separado de Código Lido, Testes Encontrados e Testes Executados (Bloco 6)

### 4.1. Código-Fonte Lido Integralmente
- `nucleo/src/avisos.rs` (`1–1003` — 100%)
- `nucleo/src/testes_avisos.rs` (`1–159` — 100%)
- `app/qml/Notificacao.qml` (`1–252` — 100%)
- `app/qml/IconeFuncao.qml` (`1–42` — 100%)
- `app/qml/Aviso.qml` (`1–47` — 100%)
- `app/qml/TelaInicio.qml` (`1–276` — 100%)

### 4.2. Testes Automatizados Encontrados no Código (`10` funções `#[test]`)

| Nº | Arquivo | Nome Exato da Função `#[test]` | Linhas Exatas | Escopo Auditado Estaticamente |
|---|---|---|---|---|
| 1 | `nucleo/src/avisos.rs` | `avisa_o_que_vence_hoje_e_o_vencido` | `792–833` | Aviso `"receber"` vencendo hoje e vencido, `esconder` hoje e retorno no outro dia. |
| 2 | `nucleo/src/avisos.rs` | `n01_esconder_segue_a_categoria` | `874–891` | Apenas `INFORMATIVA` aceita `esconder`; `ATENCAO` e `CRITICA` recusam e permanecem. |
| 3 | `nucleo/src/avisos.rs` | `n02_marca_de_escondida_nao_some_com_atencao_nem_critica` | `896–908` | Marca antiga `aviso_escondido.<id>` no banco é ignorada para `ATENCAO` e `CRITICA`. |
| 4 | `nucleo/src/avisos.rs` | `n03_escondida_volta_no_outro_dia_so_se_ainda_vale` | `912–928` | Aviso informativo escondido volta após meia-noite apenas se a condição persistir. |
| 5 | `nucleo/src/avisos.rs` | `n04_minimizar_e_so_apresentacao` | `934–960` | `INFORMATIVA` e `ATENCAO` iniciam minimizadas (`true`); `CRITICA` nunca minimiza; `minimizar` não altera dados de negócio nem esconde. |
| 6 | `nucleo/src/avisos.rs` | `n05_uma_notificacao_por_condicao` | `964–985` | Unicidade de `id` por condição e atualização in-place quando o valor vencido aumenta. |
| 7 | `nucleo/src/avisos.rs` | `avisa_backup` | `987–1002` | Avisos `"backup"` (sem backup, com erro escapado por `curto`) e `"backup_nuvem"`. |
| 8 | `nucleo/src/testes_avisos.rs` | `n06_esconder_pela_api_respeita_a_categoria` | `61–80` | Comando `"aviso_esconder"` pela API recusa `"backup"`, `"outra_loja"` e `"nao_existe"`, e esconde `"receber"`. |
| 9 | `nucleo/src/testes_avisos.rs` | `n08_so_uma_nao_critica_aberta_mesmo_com_marcas_antigas` | `97–128` | Regra `RNI-01`: apenas uma não-crítica fica aberta (em faixa) por vez, mesmo com marcas antigas `"0"` no banco. |
| 10 | `nucleo/src/testes_avisos.rs` | `n07_minimizada_continua_depois_de_fechar_e_abrir` | `133–159` | Estado minimizado/expandido sobrevive ao fechar e reabrir `Servico::abrir`, e o aviso some quando o backup é feito. |

### 4.3. Testes Executados Neste Bloco
- **0 testes executados em runtime neste bloco** (a execução real da suíte `cargo test` com registro de comando, ambiente e saída está concentrada no **Bloco 18**, ou pode ser verificada pontualmente quando necessário).

