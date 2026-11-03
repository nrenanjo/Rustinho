# BLOCO 07A — Reauditoria Independente: Navegação Contextual Guiada e Requisitos (Parte 1)

> **Fase do Plano:** Fase 2 — Reauditoria Independente dos Blocos 6 a 9B + Componentes Correlatos
> **Status:** CONCLUÍDO ✅
> **Data:** 2026-11-02
> **Modo de Operação:** Somente Auditoria (Regra de Imutabilidade Absoluta ativa: `allowLegacyEdits: false`)

---

## 1. Mapa Estrutural Exato dos 6 Arquivos Auditados no Workspace (`0.9.17`)

> **Descoberta de Reconciliação de Versão:** A inspeção direta dos arquivos reais no disco comprova que `nucleo/src/requisitos.rs` (`91` linhas), `nucleo/src/testes_nav.rs` (`226` linhas, `7` testes) e `app/qml/Main.qml` (`764` linhas) implementam no workspace atual apenas a **Etapa 1** da Navegação Contextual Guiada (`loja.cnpj` e `loja.uf_cnpj`), ao passo que o snapshot analisado pelo Gemini 4 Argon no PDF histórico continha uma versão posterior desses dois arquivos (`244` linhas em `requisitos.rs` e `734` linhas / `20` testes em `testes_nav.rs`). Além disso, em nosso workspace, `ImportarCadastros.qml` (`359` linhas) já herda de `Painel` (e não `Popup`).

| Arquivo | Linhas Exatas | Estruturas, Funções, Componentes e Testes (`nl -ba`) | Papel Confirmado no Código |
|---|---|---|---|
| `nucleo/src/requisitos.rs` | `91` (`1–91`) | • Constantes `PREFIXO` (`14`), `CNPJ_DA_LOJA` (`16`), `CAMPO_CNPJ_DA_LOJA` (`17`), `UF_E_CNPJ_DA_LOJA` (`19`), `PARA_CADASTROS` (`22`), `PARA_COMPRAS` (`24`)<br>• `pub struct Situacao` (`28–33`), `pub fn conferir` (`36–55`), `pub fn exigir_cnpj_da_loja` (`60–64`), `fn documento_da_loja` (`70–79`)<br>• `pub struct Arquivos` (`82–85`), `pub fn arquivos` (`88–90`)<br>• Testes próprios no arquivo: `0` (ficam em `testes_nav.rs`). | Conferência no banco dos requisitos `loja.cnpj` e `loja.uf_cnpj`, helper de recusa `exigir_cnpj_da_loja` e verificação de existência de arquivos em disco (`arquivos`). |
| `nucleo/src/testes_nav.rs` | `226` (`1–226`) | • Helpers `chamar` (`17–22`), `ok` (`24–28`), `loja` (`31–42`), `salvar_cnpj` (`45–63`), `satisfeito` (`65–67`), `contar` (`122–124`)<br>• **7 funções `#[test]`** (`72–225`): `nav01_previa_sem_cnpj_diz_o_requisito`, `nav01_requisito_cnpj_so_com_o_salvamento_valido`, `nav02_cnpj_gravado_invalido_tambem_e_requisito`, `nav03_compras_sem_cnpj_e_requisito`, `nav04_nota_de_outro_cnpj_vem_como_conflito`, `nav05_requisito_do_fiscal_uf_e_cnpj`, `nav01_arquivos_conferir_separa_os_que_faltam`, `nav01_comandos_novos_sao_so_leitura` (**8 funções `#[test]`** no total!). | Testes automatizados da Etapa 1 da navegação guiada (`importar_previa`, `compra_importar`, `requisito_conferir`, `arquivos_conferir` e conflito de CNPJ em compras). |
| `app/qml/Main.qml` | `764` (`1–764`) | • `ApplicationWindow { id: janela }` (`8–764`), mapa `telas` (`48–66`), navegação `abrir` (`80–88`), `voltar` (`90–93`), `substituir` (`98–100`), `inicio` (`102–104`), `trocarPara` (`106–109`), `entrou` (`112–134`), `trocarUsuario` (`136–141`), `abrirFuncao` (`143–152`)<br>• Autorização de gerente: `podeGerente` (`168–171`), `comGerente` (`172–178`), `comGerenteVarias` (`181–188`), `chamarGerenteDemorado` (`193–210`), `chamarGerente` (`211–229`), `encerrarGerente` (`231–233`)<br>• Navegação guiada (`268–375`): `requisitos` (`268–281`), `guiaAtiva` (`282`), `guiar` (`286–304`), `guiaDestacar` (`306–309`), `guiaOrientar` (`311–315`), `guiaFalhou` (`317–320`), `concluirGuia` (`322–352`), `depoisDaTroca` (`354–366`), `abandonarGuia` (`369–375`)<br>• `faixaModoSeguro` (`380–397`), `StackView { id: pilha }` (`399–449`), `Shortcut` `"F11"` (`452–462`), backup automático/ao fechar (`466–510`), `"F12"` (`513–517`), `--textos-da-tela` (`522–547`), `Aviso` (`549`), `Guia` (`551–554`), `Dialogo` (`556`), overlay `espera` (`562–653`), `erroGeral` (`656–689`) | Janela raiz, pilha de navegação, orquestrador da navegação guiada (`guiar`/`concluirGuia`/`abandonarGuia`) e controle de sessão/permissões. |
| `app/qml/Guia.qml` | `158` (`1–158`) | • `Item { id: guia; z: 1400 }` (`18–158`), `alturaNoAlto` (`24`), `alvo` (`29`), `area` (`32`), `destacando` (`33`), `estado`/`titulo`/`texto` (`36–38`)<br>• `destacar` (`41–46`), `semDestaque` (`47–50`), `mostrarNaRolagem` (`52–59`), `medir` (`60–67`), `Timer` 60ms (`68–73`), `mostrar` (`77–84`), `esconder` (`85–88`), `Timer { id: tempo; interval: 9000 }` (`89–93`)<br>• 4 retângulos `corVeu` + contorno azul do campo (`96–126`)<br>• `Loader { id: notificacao }` em `T.Overlay.overlay` (`129–157`) instanciando `Notificacao` (`142–155`) | Camada visual da navegação guiada: véu escuro recortado ao redor do campo-alvo + banner superior reutilizando `Notificacao.qml`. |
| `app/qml/ImportarCadastros.qml` | `359` (`1–359`) | • `Painel { id: painel }` (`11–359`), `abrirCom` (`32–40`), `ler` (`42–47`), `lerArquivos` (`50–73` com chamada a `janela.guiar` na linha `70`), `voltouDaGuia` (`77–94`), `cadastrar` (`96–98`), `cadastrarAgora` (`99–108`), `depoisDeImportar` (`109–128`)<br>• `FileDialog` (`134–141`), `FolderDialog` (`142–147`), `DropArea` (`153–160`), `ColumnLayout { id: coluna }` (`162–357`) | Modal de prévia e importação de clientes, fornecedores e peças via XML/ZIP/RAR, integrado à navegação guiada para `loja.cnpj`. |
| `app/qml/ConflitoCnpj.qml` | `267` (`1–267`) | • `Painel { id: janelaConflito }` (`11–267`) com `height: Math.min(implicitHeight, Tema.altura - 24)` (`17`), `mostrar(c)` (`20–24`)<br>• `Flickable { id: rolagem }` (`29–239`) com comparativo lado a lado `"CNPJ da loja"` (`93–111`) vs. `"CNPJ no XML (destinatário)"` (`112–130`), dados da nota (`133–174`), `ListView { id: itens }` (`181–227`) e botões `"Conferir nota"`, `"Descartar nota"` e `"Cancelar"` (`241–265`) | Modal de segurança para divergência de CNPJ destinatário na importação de compras, impedindo alteração acidental do CNPJ da loja. |

---

## 2. Revisão Independente de Todos os Achados Anteriores do Bloco 7A

### 2.1. Quadro Consolidado de Veredito Independente

| ID Histórico | Classificação Original (Gemini 4) | Veredito Independente no Código Atual (`0.9.17`) | Classificação Técnica Revisada | Linhas Exatas no Workspace |
|---|---|---|---|---|
| **B7A-01** | Defeito Comprovado | **CONFIRMADO** | **Defeito Comprovado** | `app/qml/Main.qml:452–462` |
| **B7A-02** | Risco de Estado Órfão | **CONFIRMADO** | **Defeito Comprovado / Risco de Estado Órfão** | `app/qml/Main.qml:102–109, 369–375` + `app/qml/TelaConfiguracoes.qml:49` |
| **B7A-03** | Risco / Melhoria | **CONFIRMADO** | **Defeito Comprovado (Falta de `guiar` em `depoisDeImportar` + falta de `Flickable`)** | `app/qml/ImportarCadastros.qml:11–16, 109–115, 149–166` |
| **B7A-04** | Melhoria Proposta | **CONFIRMADO** | **Melhoria Proposta** | `app/qml/Main.qml:380–397` |
| **B7A-05** | Melhoria Proposta | **CONFIRMADO** | **Melhoria Proposta** | `nucleo/src/testes_nav.rs:38–41, 72–217` |

---

### 2.2. Evidências Literais e Análise Detalhada

#### 1. `[B7A-01]` — Atalho global `F11` (`Main.qml:452–462`) permanece ativo durante guia e empilha `TelaPecas` por cima da tela de destino sem encerrar `guiaAtiva`
- **Veredito:** **CONFIRMADO** (Classificação: **Defeito Comprovado**).
- **Localização exata:** `app/qml/Main.qml:452–462`.
- **Trecho literal (`app/qml/Main.qml:452–462`):**
  ```qml
      // F11: ir para a busca de peças de qualquer lugar.
      Shortcut {
          sequence: "F11"
          enabled: !!janela.estado.usuario
          onActivated: {
              const tela = pilha.currentItem
              if (tela && typeof tela.focarBusca === "function")
                  tela.focarBusca()
              else
                  janela.abrir("TelaPecas")
          }
      }
  ```
- **Sustentação:** Enquanto o usuário está em `TelaConfiguracoes` resolvendo um requisito guiado (`janela.guiaAtiva !== null`), `TelaConfiguracoes` não define `focarBusca()`. Se o usuário pressionar `F11`, `janela.abrir("TelaPecas")` empilha `TelaPecas` sobre `TelaConfiguracoes` mantendo `guiaAtiva` ativa, a faixa da guia no topo (`Guia.qml`) e o `Timer` de medição (`Guia.qml:68–73`) apontando para o campo da tela soterrada.

#### 2. `[B7A-02]` — `inicio()` e `trocarPara()` em `Main.qml` não limpam `guiaAtiva` diretamente
- **Veredito:** **CONFIRMADO** (Classificação: **Defeito Comprovado / Risco de Estado Órfão**).
- **Localização exata:** `app/qml/Main.qml:102–109, 136–141, 369–375`.
- **Trecho literal (`app/qml/Main.qml:102–109`):**
  ```qml
      function inicio() {
          pilha.pop(null)
      }

      function trocarPara(tela, propriedades) {
          pilha.clear(StackView.Immediate)
          pilha.push(Qt.resolvedUrl(tela + ".qml"), propriedades || {}, StackView.Immediate)
      }
  ```
- **Sustentação:** `inicio()` e `trocarPara()` delegam implicitamente o encerramento da guia ao `Component.onDestruction` da tela de destino (`TelaConfiguracoes.qml:49`: `if (guia) janela.abandonarGuia(guia)`). Porém, conforme confirmado na inspeção conjunta com `TelaConfiguracoes.qml`, `janela` naquela tela é `readonly property var janela: ApplicationWindow.window` (`linha 11`), que passa a `null` quando o componente é removido da árvore visual do `StackView` antes de `onDestruction`, deixando `janela.guiaAtiva` presa em `Main.qml`!

#### 3. `[B7A-03]` — `ImportarCadastros.qml`: ausência de `guiar()` em `depoisDeImportar` (item `3` da Matriz C8) e ausência de `Flickable` / limite de altura no `Painel`
- **Veredito:** **CONFIRMADO** (Classificação: **Defeito Comprovado**).
- **Localização exata:** `app/qml/ImportarCadastros.qml:11–16, 109–115, 149–166`.
- **Trecho literal (`app/qml/ImportarCadastros.qml:109–115` e `149–152`):**
  ```qml
      function depoisDeImportar(r) {
          Qt.callLater(() => {
              painel.trabalhando = false
              if (!r.ok) {
                  painel.erro = r.erro
                  return
              }
  ```
  ```qml
      contentItem: Item {
          implicitWidth: coluna.implicitWidth
          implicitHeight: coluna.implicitHeight
  ```
- **Sustentação:**
  1. Enquanto `lerArquivos()` (`linha 70`) invoca `if (painel.janela.guiar(r.campo, r.erro, ok => painel.voltouDaGuia(ok))) painel.close()`, `depoisDeImportar(r)` (`linhas 112–115`) apenas atribui `painel.erro = r.erro`. Se `"importar_cadastros"` recusar com `campo == "requisito:loja.cnpj"`, a navegação guiada não é acionada.
  2. Diferentemente de `ConflitoCnpj.qml` (`linha 17`: `height: Math.min(implicitHeight, Tema.altura - 24)` + `Flickable` na linha `29`), `ImportarCadastros.qml` não limita `height` a `Tema.altura - 24` e não possui `Flickable`, podendo cortar os botões `"Fechar"` e `"Cadastrar"` (`linhas 343–356`) em resoluções baixas quando os 4 blocos de prévia e avisos estão visíveis.

#### 4. `[B7A-04]` e `[B7A-05]` — Melhorias Confirmadas
- **`B7A-04` (`Main.qml:380–397`):** `faixaModoSeguro` não possui botão ou `TapHandler` para abrir `TelaConfiguracoes` na seção `"backup"`.
- **`B7A-05` (`nucleo/src/testes_nav.rs:38–41`):** Os diretórios temporários `rustinho-nav-*` criados em `std::env::temp_dir()` pela função `loja()` não são removidos ao final dos testes.

---

## 3. Novos Achados Identificados Independentemente no Bloco 7A (`0.9.17`)

### 1. `[NEW-B7A-01]` — Implementação incompleta da Matriz de Navegação Contextual C8 no código atual (apenas Etapa 1 presente em `requisitos.rs` e `Main.qml`)
- **Classificação:** **Defeito Comprovado (Lacuna de Implementação frente ao Contrato Documental do Bloco 4)**.
- **Localização exata:** `nucleo/src/requisitos.rs:13–55` e `app/qml/Main.qml:268–281`.
- **Evidência literal (`nucleo/src/requisitos.rs:36–54`):**
  ```rust
  pub fn conferir(conn: &Connection, id: &str) -> Resultado<Situacao> {
      let (satisfeito, falta) = match id {
          CNPJ_DA_LOJA => match documento_da_loja(conn)? { ... },
          UF_E_CNPJ_DA_LOJA => { ... }
          _ => return Err(Erro::validacao(format!("Requisito desconhecido: {id}."))),
      };
      Ok(Situacao { id: id.to_string(), satisfeito, falta })
  }
  ```
- **Sustentação:** Das 17 entradas do contrato documental de navegação contextual guiada (Bloco 4), o código atual do repositório implementa apenas os 2 requisitos da **Etapa 1** (`"loja.cnpj"` e `"loja.uf_cnpj"`, cobrindo as entradas `1`, `2` e `3b`). Os outros 6 requisitos (`"fiscal.regime_ambiente"`, `"fiscal.serie_nfce"`, `"fiscal.csc"`, `"fiscal.certificado"`, `"receber.conta_cartao"`, `"receber.taxa_cartao"`, entradas `4` a `8`, `9a` a `9e` e `10a` a `10b`) não existem em `requisitos.rs` nem no mapa `requisitos` de `Main.qml`.

### 2. `[NEW-B7A-02]` — Banner de orientação em `Guia.qml` instancia `Notificacao` com cursor de clique e arrasto ativos, mas sem tratar `onMinimizar`
- **Classificação:** **Melhoria / Polimento de UI**.
- **Localização exata:** `app/qml/Guia.qml:142–155` vs. `app/qml/Notificacao.qml:194–235`.
- **Análise:** `Guia.qml` instancia `Notificacao` com `categoria: "atencao"` quando `guia.estado === "pendente"`. Em `Notificacao.qml`, itens `"atencao"` exibem `Qt.PointingHandCursor` e aceitam clique/arrasto para emitir `minimizar(true)`, mas `Guia.qml` só conecta `onEsconder: guia.esconder()`, fazendo com que o banner pareça clicável sem reagir ao clique.

---

## 4. Registro Separado de Código Lido, Testes Encontrados e Testes Executados (Bloco 7A)

### 4.1. Código-Fonte Lido Integralmente
- `nucleo/src/requisitos.rs` (`1–91` — 100%)
- `nucleo/src/testes_nav.rs` (`1–226` — 100%)
- `app/qml/Main.qml` (`1–764` — 100%)
- `app/qml/Guia.qml` (`1–158` — 100%)
- `app/qml/ImportarCadastros.qml` (`1–359` — 100%)
- `app/qml/ConflitoCnpj.qml` (`1–267` — 100%)

### 4.2. Testes Automatizados Encontrados no Código (`8` funções `#[test]` em `nucleo/src/testes_nav.rs`)

| Nº | Arquivo | Nome Exato da Função `#[test]` | Linhas Exatas | Escopo Auditado Estaticamente |
|---|---|---|---|---|
| 1 | `nucleo/src/testes_nav.rs` | `nav01_previa_sem_cnpj_diz_o_requisito` | `71–82` | `"importar_previa"` sem CNPJ retorna `campo: "requisito:loja.cnpj"` e passa após salvar CNPJ válido. |
| 2 | `nucleo/src/testes_nav.rs` | `nav01_requisito_cnpj_so_com_o_salvamento_valido` | `87–101` | `"requisito_conferir"` para `"loja.cnpj"` recusa CNPJ inválido ou vazio e recusa ID desconhecido. |
| 3 | `nucleo/src/testes_nav.rs` | `nav02_cnpj_gravado_invalido_tambem_e_requisito` | `107–120` | Regra `RNI-02`: CNPJ inválido pré-gravado no banco também aciona `campo: "requisito:loja.cnpj"`. |
| 4 | `nucleo/src/testes_nav.rs` | `nav03_compras_sem_cnpj_e_requisito` | `130–152` | `"compra_importar"` sem CNPJ ou com CNPJ inválido no banco retorna `campo: "requisito:loja.cnpj"` sem gravar nada. |
| 5 | `nucleo/src/testes_nav.rs` | `nav04_nota_de_outro_cnpj_vem_como_conflito` | `157–177` | Nota emitida para outro CNPJ retorna em `conflitos` sem gravar compra/fornecedor e sem alterar `loja_cnpj`. |
| 6 | `nucleo/src/testes_nav.rs` | `nav05_requisito_do_fiscal_uf_e_cnpj` | `181–205` | `"requisito_conferir"` para `"loja.uf_cnpj"` exige UF válida em `documento::UFS` e CNPJ válido. |
| 7 | `nucleo/src/testes_nav.rs` | `nav01_arquivos_conferir_separa_os_que_faltam` | `209–217` | `"arquivos_conferir"` particiona caminhos em `existem` e `faltam`. |
| 8 | `nucleo/src/testes_nav.rs` | `nav01_comandos_novos_sao_so_leitura` | `221–226` | Verifica que `"requisito_conferir"` e `"arquivos_conferir"` pertencem a `api::SO_LEITURA`. |

### 4.3. Testes Executados Neste Bloco
- **0 testes executados em runtime neste bloco.**

