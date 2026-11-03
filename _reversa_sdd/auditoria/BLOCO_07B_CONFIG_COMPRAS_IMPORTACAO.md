# BLOCO 7B — Auditoria de Código: Configurações, Compras, Importação e Componentes

**Data da auditoria:** 2026-11-02
**Escopo:** Revisão independente de `nucleo/src/importar.rs`, `nucleo/src/compras.rs`, `app/qml/TelaConfiguracoes.qml`, `app/qml/TelaCompras.qml`, `app/qml/TelaCompra.qml`, `app/qml/LinhaCompra.qml`, `app/qml/LinhaItemCompra.qml`, `app/qml/TelaFiscal.qml`, `app/qml/PedirGerente.qml` e `app/qml/Campo.qml`, confrontando contra o relatório histórico #17 do Gemini 4 Argon (`RELATORIO_AUDITORIA_BLOCO_7B_GEMINI_4_ARGON.md`) e os contratos documentais dos Blocos 1 a 5.
**Restrição de imutabilidade:** Nenhum arquivo do código-fonte ou documentação preexistente foi modificado.

---

## 1. Arquivos Lidos Integralmente no Workspace

| # | Arquivo | Linhas no Workspace | Linhas no Gemini 4 (Bloco 7B) | Status de Leitura |
|---|---|---|---|---|
| 1 | `nucleo/src/importar.rs` | `1–760` (760 linhas) | 663 linhas | 100% lido |
| 2 | `nucleo/src/compras.rs` | `1–2082` (2.082 linhas) | 1.158 linhas | 100% lido |
| 3 | `app/qml/TelaConfiguracoes.qml` | `1–2034` (2.034 linhas) | 2.053 linhas | 100% lido |
| 4 | `app/qml/TelaCompras.qml` | `1–345` (345 linhas) | 345 linhas | 100% lido |
| 5 | `app/qml/TelaCompra.qml` | `1–699` (699 linhas) | 561 linhas | 100% lido |
| 6 | `app/qml/LinhaCompra.qml` | `1–117` (117 linhas) | 112 linhas | 100% lido |
| 7 | `app/qml/LinhaItemCompra.qml` | `1–306` (306 linhas) | 306 linhas | 100% lido |
| 8 | `app/qml/TelaFiscal.qml` | `1–370` (370 linhas) | 369 linhas | 100% lido |
| 9 | `app/qml/PedirGerente.qml` | `1–109` (109 linhas) | 109 linhas | 100% lido |
| 10 | `app/qml/Campo.qml` | `1–332` (332 linhas) | 332 linhas | 100% lido |
| — | **Total do Bloco 7B** | **7.154 linhas** | **6.008 linhas** | **100% lido** |

---

## 2. Reconciliação do Estado de Versão (Workspace vs. Snapshot do Gemini 4 Argon)

A comparação linha a linha entre o workspace atual (`/home/renan/Área de trabalho/Rustinh0`) e o snapshot que o Gemini 4 Argon recebeu no Bloco 7B revela uma bifurcação importante em duas frentes:

1. **Mais avançado no nosso workspace (`compras.rs`, `importar.rs`, `TelaCompra.qml`, `LinhaCompra.qml`):**
   - No Gemini 4 Argon, `compras.rs` tinha `1.158` linhas (ainda chamava `nfe_xml::ler` sem verificar assinatura digital ICP-Brasil) e `importar.rs` tinha `663` linhas.
   - No nosso workspace, `compras.rs` tem `2.082` linhas (+924 linhas), `importar.rs` tem `760` linhas (+97 linhas) e `TelaCompra.qml` tem `699` linhas (+138 linhas), pois já incorporam integralmente a implementação de **`PEND-09` (`D2`, `D5`, `D7`, `D8` de `docs/seguranca_xml_nfe.md`)** e **`PEND-19` (CNPJ alfanumérico)**:
     - `importar.rs:199` chama `nfe_xml::ler_cadastro_verificada(xml, crate::nfe_assinatura::Ancoras::padrao())`, exigindo XML assinado com certificado ICP-Brasil válido na data de emissão e proibindo efeitos financeiros ou de estoque (coberto pelo teste `a43_cadastro_por_xml_so_com_assinatura_e_sem_efeito_financeiro` em `importar.rs:717–759`).
     - `compras.rs:165` chama `nfe_xml::ler_verificada(xml, crate::nfe_assinatura::Ancoras::padrao())`, grava `xml_verificacao = 'verificada'`, `xml_prova` (JSON da prova criptográfica) e `autorizacao = 'aguardando'` (`compras.rs:193–220`), e bloqueia `compras::lancar` (`compras.rs:1154–1156`) até que um gerente execute `compras::registrar_conferencia` (`compras.rs:471–547`) com um dos 3 canais de autorização (`automatico_sefaz`, `chave_portal` com 44 dígitos ou `danfe_papel` com justificativa obrigatória).
2. **Mais antigo no nosso workspace quanto à Matriz C8 (`TelaConfiguracoes.qml`, `compras::conferir_nota`):**
   - Conforme já identificado no Bloco 7A, o nosso workspace implementa apenas a **Etapa 1** da Matriz C8 (`requisito:loja.cnpj` e `requisito:loja.uf_cnpj` em `requisitos.rs:1–91` e `TelaConfiguracoes.qml:106–113`).
   - Em `compras.rs:80–91`, `conferir_nota` ainda lê `loja_cnpj` diretamente via `config::ler(conn, "loja_cnpj")` em vez de chamar `requisitos::exigir_cnpj_da_loja` (que só é chamado na camada de API em `api.rs` para `"compra_importar"`).

---

## 3. Revisão Independente dos Achados Históricos do Gemini 4 Argon (Bloco 7B)

| ID Histórico | Título / Descrição Original | Classificação Revisada | Natureza | Evidência Exata no Código-Fonte (`arquivo:linhas`) |
|---|---|---|---|---|
| **B7B-01** | **Guia contextual (`tela.guia`) não é retomada se o operador `Caixa` cancelar `PedirGerente` inicialmente e depois clicar em `botaoSenhaGerente ("Digitar senha do gerente")`:** Em `TelaConfiguracoes.qml`, quando um operador no modo `Caixa` chega via `janela.guiar(...)`, `comecarGuia()` (`linhas 66–79`) destaca `botaoSenhaGerente`, orienta `"Peça a senha do gerente para preencher e salvar este dado."` e abre `PedirGerente` via `janela.comGerente("configuracoes", ...)`. Se o operador cancelar o diálogo modal naquele instante e depois clicar manualmente no botão destacado `botaoSenhaGerente` (`linhas 551–558`), este chama `tela.liberar()` (`linhas 156–161`), que apenas faz `tela.completo = true; tela.carregar()` **sem** limpar o aviso com `janela.guiaOrientar("")` e **sem** chamar `mostrarCampoDaGuia()`. O campo-alvo da guia nunca recebe destaque nem foco, e o destaque permanece preso num botão que acabou de ficar invisível (`visible: !tela.completo`). | **CONFIRMADO** | **Defeito comprovado** | `app/qml/TelaConfiguracoes.qml:66–79` (`comecarGuia`), `156–161` (`liberar`: `function liberar() {janela.comGerente("configuracoes", () => { tela.completo = true; tela.carregar() })}`), `551–558` (`botaoSenhaGerente.onClicked: tela.liberar()`). |
| **B7B-02** (`NAV-03`) | **Mensagem enganosa sobre verificação de assinatura XML e variável morta `deOutra` em `TelaCompras.qml`:** (a) Ao importar XML em `TelaCompras.qml:91–93`, a interface exibe o toast fixo `"O Rustinho ainda não verifica a assinatura digital do XML: use só XML que veio do fornecedor ou da SEFAZ."` — o que contradiz frontalmente `nucleo/src/compras.rs:165` (`nfe_xml::ler_verificada`), `TelaCompras.qml:315` (`"XML sem assinatura digital ICP-Brasil ou modificado é recusado."`) e `TelaCompra.qml:254, 274` (`compra.assinaturaTexto`). (b) Em `TelaCompras.qml:47, 62–65, 90`, a variável `let deOutra = 0` incrementa quando `c.avisoDestinatario.indexOf("não é") >= 0`, mas no núcleo (`compras.rs:84–88, 174–185`) qualquer NF-e cujo destinatário divirja do CNPJ da loja é **recusada** antes de gerar rascunho e vai para `r.dados.conflitos` (`TelaCompras.qml:68`), tornando `deOutra` código morto inalcançável. | **CONFIRMADO** | **Defeito comprovado** (a: mensagem contraditória ao usuário) / **Melhoria** (b: código morto) | `app/qml/TelaCompras.qml:47` (`let deOutra = 0`), `62–65` (`if (c.avisoDestinatario.indexOf("não é") >= 0) deOutra++`), `90–93` (toast contraditório), `315` (texto da página que afirma o oposto); `nucleo/src/compras.rs:84–88, 165, 174–185`. |
| **B7B-03** (`NAV-04`) | **Divergência entre `compras::importar_xmls` (núcleo) e `compra_importar` (`api.rs`) quando `loja_cnpj` está vazio:** Em `nucleo/src/compras.rs:80–91`, `conferir_nota` lê `loja_cnpj` e executa `if !loja.is_empty() { if dest != loja { return Err(...) } } else if !dest.is_empty() { return Ok("Confira se esta nota é mesmo da loja — o CNPJ da loja ainda não foi preenchido nas configurações.".into()); }`. Ou seja, chamar `compras::importar_xmls` diretamente em Rust sem CNPJ cadastrado **permite** importar a NF-e com apenas um aviso textual, enquanto a rota `"compra_importar"` em `api.rs` bloqueia via `requisitos::exigir_cnpj_da_loja(conn)`. Se o requisito não é verificado dentro de `compras::importar_xmls`, o contrato `C8-02` depende exclusivamente do caller em `api.rs` lembrar de chamar `requisitos::exigir_cnpj_da_loja`. | **CONFIRMADO** | **Risco** (acoplamento de guarda fora da função de domínio + ramo residual em `conferir_nota`) | `nucleo/src/compras.rs:80–91` (`conferir_nota`), `161–186` (`importar_um`). |
| **B7B-04** | **Foco desviado sem atualizar o destaque da guia quando `config_salvar` falha em outro campo:** Em `app/qml/TelaConfiguracoes.qml:331–341`, quando `salvar()` recebe `!r.ok` com `r.campo` apontando para outro campo da seção `"loja"` diferente do campo da guia (ex.: guia em `lojaUf`/`lojaCnpj`, mas `loja` — nome da loja — está em branco e falha primeiro), `salvar()` executa `escolherSecao(mapa.secao); ... alvo.focar()` **sem** atualizar `janela.guiaDestacar(alvo, rolagem)`. O foco do teclado vai para `loja`, mas o anel dourado da guia continua destacando o campo original da guia. | **CONFIRMADO** | **Defeito comprovado** (dessincronia visual foco vs. destaque da guia) | `app/qml/TelaConfiguracoes.qml:323–347` (`salvar` -> tratamento de `!r.ok` com `r.campo`). |
| **NAV-01** | **Empilhamento duplicado de `TelaConfiguracoes` quando `TelaFiscal` embutida aciona `janela.guiar(...)`:** Em `TelaConfiguracoes.qml:617–628`, a seção `"fiscal"` carrega `TelaFiscal.qml` dentro de um `Loader` (`item.embutida = true`). Quando o usuário clica em `"Ir para Loja"` (`TelaFiscal.qml:220`), o handler chama `tela.janela.guiar("requisito:loja.uf_cnpj", "", ok => tela.carregar())`. Em `Main.qml:295–303`, `guiar()` grava `origem = pilha.currentItem` (que é a **primeira** instância de `TelaConfiguracoes` na seção `"fiscal"`) e chama `abrir("TelaConfiguracoes", { secao: "loja", guia: g })`, empilhando uma **segunda** instância de `TelaConfiguracoes` sobre a primeira! Além disso, quando a segunda instância conclui e faz `pilha.pop()`, a primeira instância de `TelaConfiguracoes` não possui `StackView.onActivated` (`TelaConfiguracoes.qml:2005–2033` só chama `carregar()` em `Component.onCompleted`), de modo que apenas o `TelaFiscal` embutido recarrega via callback `ok => tela.carregar()`, enquanto o `tela.config` da primeira instância de `TelaConfiguracoes` fica desatualizado em memória até que o usuário clique em salvar ou recarregue. | **CONFIRMADO** | **Defeito comprovado** (2 instâncias de `TelaConfiguracoes` na `StackView` + `tela.config` obsoleto na 1ª instância) | `app/qml/TelaConfiguracoes.qml:617–628, 2005–2033`; `app/qml/TelaFiscal.qml:214–223`; `app/qml/Main.qml:295–303`. |
| **NAV-02** | **Troca manual de seção (`escolherSecao`) ou subpílula durante guia contextual ativa não atualiza nem suspende o destaque da guia:** Em `TelaConfiguracoes.qml:186–207` (`escolherSecao`) e `608–612` (`subpilulas`), o usuário pode clicar em outra pílula de seção (ex.: `"aparencia"`, `"fiscal"`, `"impressao"`) enquanto `tela.guia` está ativa. A seção muda, ocultando o campo destacado pela guia, mas a faixa dourada de `Guia.qml` no topo da janela permanece visível apontando para um campo oculto, e quando o usuário volta à seção da guia o destaque não é reposicionado. | **CONFIRMADO** | **Defeito comprovado** (UX / estado inconsistente da guia) | `app/qml/TelaConfiguracoes.qml:186–207` (`escolherSecao`), `568–614` (pílulas e subpílulas sem guarda `if (tela.guia)`). |
| **NOTIF-03** | **Cancelamento de `PedirGerente` ao abrir `TelaConfiguracoes` a partir de notificação em `TelaInicio` deixa o operador `Caixa` preso em `TelaConfiguracoes` (`secao: "loja"`):** Em `TelaInicio.qml:161–186`, clicar numa notificação administrativa (`simultaneidade`, `backup`, `gerente`) abre `TelaConfiguracoes` passando `{ secao: ... }`. Em `TelaConfiguracoes.qml:2013–2030`, quando `!tela.completo` (modo `Caixa`), `Component.onCompleted` força `tela.secao = "loja"` (`linhas 2016, 2020, 2028`) e abre `PedirGerente` via `janela.comGerente("configuracoes", ...)`. Se o operador cancelar o modal `PedirGerente`, nenhum callback de cancelamento executa `janela.voltar()`, deixando o operador numa tela de configurações truncada na seção `"loja"` em vez de devolvê-lo à `TelaInicio`. | **CONFIRMADO** | **Defeito comprovado** | `app/qml/TelaConfiguracoes.qml:2013–2030`; `app/qml/Main.qml:444–450` (`comGerente` não aceita callback `aoCancelar`); `app/qml/PedirGerente.qml:90–94`. |
| **B7A-02** *(re-verificado em `TelaConfiguracoes.qml`)* | **`Component.onDestruction` em `TelaConfiguracoes.qml:48–52` referencia `janela.abandonarGuia(guia)` quando `ApplicationWindow.window` já é `null`:** Conforme provado no Bloco 7A, `readonly property var janela: ApplicationWindow.window` (`linha 13`) já vale `null` durante `Component.onDestruction` após remoção da `StackView`. Logo, `if (guia) janela.abandonarGuia(guia)` na linha `51` falha com `TypeError: Cannot call method 'abandonarGuia' of null` se `guia` ainda não tiver sido zerada. | **CONFIRMADO** | **Defeito comprovado** | `app/qml/TelaConfiguracoes.qml:13, 48–52`. |
| **B6-05** *(re-verificado em `TelaConfiguracoes.qml`)* | **Desalinhamento do nome de exibição de `TelaConfiguracoes` na seção `"fiscal"`:** Na linha `46` de `TelaConfiguracoes.qml`, `readonly property string nomeNaNavegacao: secao === "fiscal" ? "Fiscal" : "Configurações"`. Isso faz o botão de retorno da guia (`Main.qml:298`) exibir `"Voltar para Fiscal"` quando a origem era a aba Fiscal de Configurações, mas `TelaInicio.qml` e a pílula principal continuam abrindo `TelaConfiguracoes` com `secao: "fiscal"`, coexistindo com `TelaFiscal.qml` embutida (`linhas 617–628`). | **CONFIRMADO** | **Melhoria** | `app/qml/TelaConfiguracoes.qml:46, 617–628`. |

---

## 4. Verificação Antecipada de Achados Cruzados Presentes nos Arquivos do Bloco 7B (`B8A-03` e `PEND-17`)

Como `nucleo/src/compras.rs` e `app/qml/LinhaItemCompra.qml` fazem parte do Bloco 7B, auditamos diretamente no código-fonte dois pontos cruciais que se cruzam com o Bloco 8A e com `PENDENCIAS.md`:

### 4.1. Verificação de `B8A-03` em `nucleo/src/compras.rs:1265–1331` (`atualizar_custo`)
- **O que o Gemini 4 Argon afirmou no Bloco 8A (`B8A-03`):** Que `compras::lancar` recriaria preços de venda ignorando preços travados (`preco_travado`).
- **O que o código-fonte real de `nucleo/src/compras.rs:1265–1331` faz:**
  ```rust
  // nucleo/src/compras.rs:1286-1308
  if atualizar_precos {
      let mut st = tx.prepare(
          "SELECT tabela, preco, lucro FROM produto_preco WHERE produto_id = ?1 ORDER BY tabela",
      )?;
      let linhas: Vec<(i64, i64, Option<f64>)> = st
          .query_map(params![produto_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
          .collect::< rusqlite::Result<Vec<_>> >()?;
      drop(st);
      for (tabela, preco_atual, lucro) in linhas {
          let Some(lucro) = lucro else { continue };
          let novo = crate::moeda::aplicar_lucro(novo_custo, lucro);
          if novo < preco_atual && !baixar_preco {
              continue;
          }
          if novo != preco_atual {
              tx.execute(
                  "UPDATE produto_preco SET preco = ?1 WHERE produto_id = ?2 AND tabela = ?3",
                  params![novo, produto_id, tabela],
              )?;
              ...
  ```
- **Constatação:** No esquema real do Rustinho (`produto_preco`), **não existe coluna `preco_travado`**; um preço fixo (sem margem automática atrelada ao custo) é representado por `lucro IS NULL` (`Option<f64> == None`), enquanto um preço com margem percentual tem `lucro = Some(...)`. E a linha `1293` (`let Some(lucro) = lucro else { continue };`) **pula explicitamente** qualquer tabela de preço cujo `lucro` seja `None`! Além disso, o usuário controla o checkbox global `atualizarPrecos` por nota (`TelaCompra.qml:373–399`) e a decisão por item `baixarPreco` quando o custo cai (`LinhaItemCompra.qml:265–297`, `compras.rs:1295–1297`), tudo testado em `fluxo_completo_importar_ligar_lancar_desfazer` (`compras.rs:1932–1963`). Confirmaremos no Bloco 8A como `produtos.rs` e `api.rs` gravam `lucro` em `produto_preco`.

### 4.2. Verificação de `PEND-17` (Ocultação de Custo / Lucro com `Tema.oculto` — Atalho `F8` / Olho) em `LinhaItemCompra.qml` e `TelaCompra.qml`
- Em `app/qml/LinhaItemCompra.qml:222–298`:
  - Linha `225`: `text: "Custo R$ " + Tema.valor(linha.item.custoUnitario) + " cada"` — usa `Tema.valor(...)`, que mascara o número quando `Tema.oculto == true`.
  - Linha `234`: `text: linha.peca ? "antes R$ " + Tema.valor(linha.peca.custoAtual) + (linha.peca.variacaoCusto && !Tema.oculto ? "  (" + linha.peca.variacaoCusto + "%)" : "") : ""` — mascara `custoAtual` e esconde o percentual `variacaoCusto` quando `Tema.oculto == true`.
  - **PORÉM (Vazamento 1 — `NEW-B7B-01a`):** Na linha `235` de `LinhaItemCompra.qml`:
    ```qml
    color: linha.peca && linha.peca.custoSubiu ? Tema.vermelho : Tema.tintaSuave
    ```
    Mesmo com `Tema.oculto == true`, a cor vermelha denuncia que o custo subiu.
  - **PORÉM (Vazamento 2 — `NEW-B7B-01b`, crítico para `PEND-17`):** Nas linhas `264–298` de `LinhaItemCompra.qml`:
    ```qml
    Rectangle {
        visible: linha.editavel && !!linha.peca && linha.peca.custoBaixou && linha.item.acao === "ligar"
        ...
        Text {
            text: "O custo baixou. O preço de revenda:"
            ...
        }
        ...
        text: linha.peca ? "baixa para R$ " + linha.peca.precoSeBaixar : ""
    ```
    Quando `Tema.oculto == true`, esse quadro verde continua **totalmente visível**, revelando:
    1. Que o custo da peça baixou (`"O custo baixou"`); e
    2. O valor exato de `linha.peca.precoSeBaixar` (que permite deduzir a queda percentual do custo pela relação com `precoAtual`)!

---

## 5. Novos Achados Identificados no Bloco 7B (`NEW-B7B-01` a `NEW-B7B-05`)

| ID | Classificação | Arquivo e Linhas | Descrição Técnica e Evidência |
|---|---|---|---|
| **NEW-B7B-01** | **Defeito comprovado** (`PEND-17` parcial) | `app/qml/LinhaItemCompra.qml:235, 264–298` | **Vazamento de variação de custo em `LinhaItemCompra.qml` quando `Tema.oculto` está ativo (`F8`):** Embora a linha `234` oculte `variacaoCusto` quando `Tema.oculto == true`, a linha `235` mantém a cor `Tema.vermelho` quando `custoSubiu`, e o bloco das linhas `264–298` (`"O custo baixou. O preço de revenda: fica R$ ... / baixa para R$ ..."`) permanece visível mesmo com `Tema.oculto == true`, expondo que o custo baixou e a proporção da queda via `precoSeBaixar`. |
| **NEW-B7B-02** | **Defeito comprovado** (perda de alterações não salvas) | `app/qml/TelaConfiguracoes.qml:66–84, 156–161` | **Perda de edições em andamento quando operador `Caixa` clica em `"Digitar senha do gerente"` ou quando a guia chama `carregar()`:** Se um operador no modo `Caixa` editar qualquer campo liberado da seção `"loja"` ou `"aparencia"` (`sujo == true`) e em seguida clicar em `botaoSenhaGerente` (`liberar()` nas linhas `156–161`), `liberar()` chama `tela.carregar()`, que sobrescreve todos os campos com os valores do banco e zera `tela.sujo = false` (`linha 285`), descartando silenciosamente o que o usuário havia digitado antes de pedir a senha do gerente. |
| **NEW-B7B-03** | **Risco** (consistência de estoque / custo médio ao desfazer compra) | `nucleo/src/compras.rs:1349–1449` (`desfazer`) | **Restauração direta de `custo_antes` e `precos_antes` em `compras::desfazer` sem verificar compras ou vendas posteriores:** Quando `compras::desfazer` (`linhas 1349–1449`) reverte uma compra lançada, ela subtrai a quantidade comprada (truncando em zero via `.max(0)` na linha `1402`: `let novo_estoque = (estoque_atual - aplicar).max(0);`) e restaura `custo = ?1` para `custo_antes` (`linha 1409`) e `produto_preco` para `precos_antes` (`linhas 1413–1428`). Se uma **segunda** compra do mesmo produto foi lançada depois desta e a primeira compra é desfeita fora de ordem cronológica (não há bloqueio que exija desfazer apenas a última compra do produto), o custo e os preços de revenda do produto voltam para os valores anteriores à **primeira** compra, sobrescrevendo o custo médio e os preços calculados pela segunda compra. Além disso, se houve venda de parte do lote antes de desfazer, `.max(0)` evita estoque negativo no banco mas faz com que a soma algébrica de `estoque_movimento` divirja de `produtos.estoque_cent`. |
| **NEW-B7B-04** | **Risco** (UX / truncamento silencioso em listagem de compras) | `nucleo/src/compras.rs:347–362` (`listar`) | **Limite fixo `LIMIT 200` sem paginação nem aviso em `compras::listar`:** A consulta de `compras::listar` (`linhas 347–362`) aplica `LIMIT 200` fixo ordenado por `id DESC`, sem parâmetro de filtro/busca na tela `TelaCompras.qml` e sem indicar ao usuário quando existem mais de 200 notas importadas/lançadas no histórico. |
| **NEW-B7B-05** | **Melhoria** | `app/qml/PedirGerente.qml:13–21, 90–94` vs. `app/qml/Main.qml:444–450` | **Ausência de sinal/callback `aoCancelar` em `PedirGerente.qml` e `Main.comGerente`:** `PedirGerente.qml` só emite `signal autorizado()` (`linha 14`); quando o usuário clica em `"Cancelar"` (`linhas 90–94`) ou fecha o popup (`Escape`), `fechar()` apenas limpa a senha e fecha o modal sem emitir um sinal `cancelado()`. Isso impede que chamadores como `TelaConfiguracoes.qml` (`NOTIF-03`, `B7B-01`) reajam ao cancelamento (por exemplo, voltando à `TelaInicio` ou exibindo estado de espera claro na guia). |

---

## 6. Registro Separado de Testes (Código Lido vs. Testes Encontrados vs. Testes Executados)

### 6.1. Testes Unitários Lidos no Código-Fonte (13 testes no Bloco 7B)

#### Em `nucleo/src/importar.rs` (`linhas 631–760` — 2 funções `#[test]`):
1. `csv_clientes_com_cabecalho_ptbr_e_ponto_e_virgula` (`linhas 644–681`): testa detecção de colunas (`nome`, `documento`, `telefone`, `cidade`, `uf`), validação de CPF/CNPJ e gravação em `clientes` via `confirmar`.
2. `a43_cadastro_por_xml_so_com_assinatura_e_sem_efeito_financeiro` (`linhas 717–759`): verifica os 4 invariantes da regra `D8` (`PEND-09`): (1) XML sem assinatura ICP-Brasil é recusado por `ler_xmls_nfe_com_ancoras`; (2) XML adulterado após assinatura é recusado; (3) XML assinado com âncora confiável extrai fornecedor, transportadora e destinatário; (4) após `confirmar`, `contas_pagar`, `compras` e `produtos.estoque_cent` permanecem intactos (`0`).

#### Em `nucleo/src/compras.rs` (`linhas 1848–2081` — 11 funções `#[test]`):
1. `fluxo_completo_importar_ligar_lancar_desfazer` (`linhas 1932–1963`): testa importação de NF-e assinada, vínculo automático por código do fornecedor, bloqueio de `lancar` antes de `registrar_conferencia`, lançamento após conferência com recálculo de custo médio (`(10*2000 + 10*3000)/20 = 2500`) e preço de venda (`5000`), geração de duplicatas em `contas_pagar` e reversão completa via `desfazer`.
2. `impede_duplicidade_por_chave` (`linhas 1966–1977`): garante que reimportar a mesma NF-e retorna no relatório `"Já importada na compra #1 (rascunho)."` sem duplicar registro.
3. `a1_recusa_xml_sem_assinatura` (`linhas 1981–1992`): garante que XML com `<Signature>` removida é recusado com erro contendo `"assinatura"` e não cria rascunho.
4. `a2_recusa_xml_adulterado_apos_assinatura` (`linhas 1995–2005`): garante que alterar `<vNF>` após assinar invalida o digest criptográfico e recusa a importação.
5. `a3_recusa_certificado_fora_da_cadeia_icp_brasil` (`linhas 2008–2023`): garante que XML assinado por certificado válido não pertencente às âncoras ICP-Brasil configuradas é recusado com erro de cadeia.
6. `a4_bloqueia_lancar_sem_conferencia_e_exige_prova_valida` (`linhas 2026–2056`): valida os requisitos de `registrar_conferencia` (`chave_portal` exige 44 dígitos idênticos à chave da NF-e; `danfe_papel` exige justificativa com ao menos 5 caracteres).
7. `a42_fornecedor_novo_nasce_pendente_e_aprovado_no_lancar` (`linhas 2059–2081`): comprova que o fornecedor criado na importação do XML nasce com `cadastro_status = 'pendente_conferencia'` e passa a `'aprovado'` somente após `registrar_conferencia` + `lancar`.

*(Nota: além das 7 funções `#[test]` acima, o módulo `mod testes` de `compras.rs:1848–1930` define 4 funções auxiliares de fixture criptográfica: `banco()`, `xml_exemplo_sem_assinatura()`, `assinar_exemplo()` e `importar_exemplo_assinado()`, totalizando 7 funções `#[test]` em `compras.rs` + 2 funções `#[test]` em `importar.rs` = **9 funções `#[test]`** lidas neste bloco.)*

### 6.2. Testes Executados
- **Nenhum teste foi executado em runtime nesta etapa**, mantendo separação estrita entre leitura estática e execução, conforme diretriz do plano.

