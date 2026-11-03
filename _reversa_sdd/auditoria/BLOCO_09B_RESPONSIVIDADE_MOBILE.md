# BLOCO 9B — Telas Responsivas Compartilhadas Desktop/Android (`Tema.celular`, `Tema.estreito`, `Tema.fino`, `Tema.baixo`)

**Data da Auditoria:** 2026-11-02  
**Escopo:** Auditoria do comportamento móvel e responsivo (`Tema.celular`, `Tema.estreito`, `Tema.fino`, `Tema.baixo`) nas 14 telas compartilhadas de Estoque, Vendas, Caixa, Trocas e Orçamentos lidas no Bloco 9A2/9A2C (`TelaVender.qml`, `TelaEstoque.qml`, `SecaoEncomendas.qml`, `SecaoComprar.qml`, `TelaVenda.qml`, `SecaoVendidos.qml`, `TelaCaixa.qml`, `LinhaVenda.qml`, `LinhaCarrinho.qml`, `SecaoParados.qml`, `TelaTrocas.qml`, `TelaDevolucao.qml`, `AjusteEstoque.qml`, `TelaOrcamentos.qml`).  
**Total de código auditado neste bloco:** 4.769 linhas (14 arquivos QML compartilhados entre Desktop e Android).

---

## 1. Constatação Arquitetural: Base QML Unificada Desktop + Android

1. Os arquivos QML de vendas, estoque, caixa e trocas são **compartilhados entre Desktop (`app/qml/`) e Android (`android/rust/qml/`)**, alternando seu layout e comportamento em tempo de execução por meio das propriedades reativas do singleton `Tema`:
   - `Tema.celular` (verdadeiro no Android / tela sensível ao toque sem mouse);
   - `Tema.estreito` (`largura < 820`);
   - `Tema.fino` (`largura < 560`);
   - `Tema.baixo` (`altura < 620`).
2. Como consequência direta dessa arquitetura unificada, **todos os 14 achados funcionais confirmados nos Blocos 9A1, 9A2 e 9A2C (`B9A1-01`, `B9A1-02`, `B9A1-03`, `B9A1-06`, `B9A2-01` a `B9A2-04`, `B9A2C-01` a `B9A2C-06`) permanecem 100% presentes no Android**, somados aos defeitos geométricos e de navegação exclusivos do modo estreito/celular auditados abaixo.

---

## 2. Mapa de Adaptação Responsiva nos 14 Arquivos QML

| Nº | Arquivo (`0.9.17`) | Linhas | Flags `Tema.*` Utilizadas (Linhas Exatas) | Comportamento Adaptativo Previsto |
|---:|---|---:|---|---|
| 1 | `TelaVender.qml` | 1.401 | `Tema.estreito`, `Tema.fino`, `Tema.baixo` (`11, 403, 416, 441, 450, 478, 487, 490, 506, 586, 593, 621, 635, 641, 658, 660, 679–693, 830, 886, 1282`) | Ativa `camera: true` na busca (`487`), oculta a coluna lateral `"Cliente / Total"` (`679`), exibe barra inferior fixa com total e botão `"Receber"` (`829–873`) e usa `verLateral` para alternar entre carrinho e página `"Cliente e desconto"`. |
| 2 | `TelaEstoque.qml` | 93 | Nenhuma flag direta no contenedor (delega às subseções via `Loader` em `75–86`). | Mantém cabeçalho, pílulas `"Estoque / Fornecedores"` (`57–61`) e subpílulas (`63–73`). |
| 3 | `SecaoEncomendas.qml` | 482 | `Tema.estreito`, `Tema.celular` (`63, 129, 139, 141, 144, 167, 181, 187, 197–199, 243, 250, 257, 267`) | Usa `colunasEstreito: 2` nos cartões (`63`), move a barra `acoesEnc` para uma 2ª linha abaixo do texto (`y: 66`, `129, 199`) e torna visíveis sem hover os botões secundários quando `Tema.celular === true` (`243–267`). |
| 4 | `SecaoComprar.qml` | 364 | `colunasEstreito: 2` (`75`) e `Tema.celular` (`249`); **sem `Tema.estreito` em `cPeca` (`270–358`)** | Quebra os 4 cartões superiores em 2 colunas (`75`) e exibe sempre `"Já pedi"` nas encomendas (`249`), mantendo 4 colunas horizontais fixas em `cPeca`. |
| 5 | `TelaVenda.qml` | 330 | `Tema.estreito` (`105, 117, 129, 143, 154, 159, 170, 184, 191, 196, 197, 286`) | Divide `"Peças"` e `"Total e pagamento"` em abas via `Pilulas` (`104–109`) e empilha quantidade/preço sob a descrição do item (`158–167`). |
| 6 | `SecaoVendidos.qml` | 245 | `colunasEstreito: 2` (`40`) e `Tema.estreito` (`122–124, 132, 168, 189, 208`) | Oculta as colunas de quantidade/vendas (`168`) e o chip de estoque (`208`) em telas estreitas. |
| 7 | `TelaCaixa.qml` | 414 | `Tema.estreito` (`89, 102–104, 257, 300, 365`) | Divide `"Resumo"` e `"Vendas (N)"` em pílulas (`88–93`) quando o caixa está aberto e reduz `padding` dos modais `movimento` e `fechar` para `18` (`300, 365`). |
| 8 | `LinhaVenda.qml` | 103 | `Tema.estreito` (`23, 27–29, 37–38, 41, 61, 95, 97`) | Oculta a coluna separada de data/hora (`41`) e concatena a data/hora dentro do subtítulo (`61`). |
| 9 | `LinhaCarrinho.qml` | 211 | `Tema.estreito`, `Tema.fino` (`22, 41, 42, 46, 111–116, 135, 156, 176, 192`) | Empilha a linha de campos (`qtd`, `preco`, `desc`, subtotal) em 2 andares e reduz larguras dos campos quando `Tema.fino`. |
| 10 | `SecaoParados.qml` | 217 | `colunasEstreito: 2` (`50`) e `Tema.estreito` (`149, 160, 167`) | Oculta o `Chip` lateral de tempo parado (`160`) e transfere `linha.p.ha` para o subtítulo (`149`). |
| 11 | `TelaTrocas.qml` | 362 | `Tema.celular` (`199, 207, 293, 301`); **sem `Tema.estreito` em `listaVales` e `listaFeitas`** | Torna os botões `"Imprimir"`, `"Usar no Vender"` e `"Desfazer"` permanentemente visíveis sem hover quando `Tema.celular === true`. |
| 12 | `TelaDevolucao.qml` | 468 | `Tema.estreito` (`151, 164, 268–270`) | Divide a tela em duas abas via `Pilulas` (`"Peças"` e `"Como devolver"`, `150–155`), exibindo apenas uma por vez (`164, 268`). |
| 13 | `AjusteEstoque.qml` | 122 | `Tema.largura` (`21`) e `Tema.estreito` (`22`) | Ajusta `width: Math.min(520, Tema.largura - 24)` e reduz `padding` de `30` para `18`. |
| 14 | `TelaOrcamentos.qml` | 123 | Nenhuma flag direta no contenedor (delega a `LinhaVenda.qml` na linha `99`). | Lista orçamentos usando `LinhaVenda` responsiva, porém mantém `Pilulas` + `Campo { id: busca }` na mesma `Linha` (`56–77`). |

---

## 3. Revisão Independente dos 14 Casos Herdados no Android e dos Achados `B9B-01` a `B9B-09`

### 3.1. Confirmação dos 14 Casos Herdados (`B9A1-01`, `B9A1-02`, `B9A1-03`, `B9A1-06`, `B9A2-01` a `B9A2-04`, `B9A2C-01` a `B9A2C-06`)
- **Status da Revisão Independente:** **CONFIRMADO (14/14 casos presentes no Android)**
- Todos os 14 casos herdados operam sobre o mesmo código QML e sobre os mesmos comandos de `api.rs`, com dois agravamentos operacionais importantes quando o dispositivo Android atua em rede local como terminal adicional:
  1. **`B9A1-02` e `B9A1-03` no celular:** como um aparelho Android na loja frequentemente consulta vendas (`TelaVenda.qml`) ou devoluções (`TelaTrocas.qml`, onde o botão `"Desfazer"` fica sempre visível por `Tema.celular` na linha `301`) realizadas em um computador de balcão cuja sessão de caixa ainda está aberta, acionar `"venda_cancelar"` ou `"devolucao_desfazer"` pelo celular altera silenciosamente o `esperado_dinheiro` da gaveta do computador de balcão;
  2. **`B9A1-06` no celular:** na subpágina `"Cliente e desconto"` (`verLateral == true`, `TelaVender.qml:679, 784–794`), o operador no Android pode aplicar desconto global acima de `p.desconto_maximo` e concluir a venda sem senha de gerente.

### 3.2. Revisão Individual dos Achados Exclusivos Mobile (`B9B-01` a `B9B-09`)

| ID Histórico | Status da Revisão | Classificação | Localização Exata (`0.9.17`) | Evidência e Análise Técnica Confirmada |
|---|---|---|---|---|
| **B9B-01** | **CONFIRMADO** | **Defeito comprovado** | `app/qml/TelaVender.qml:11, 207–226, 478, 679, 830` | `limparVenda()` (`linhas 207–226`) não reseta `verLateral = false`. No celular (`Tema.estreito === true`), se o operador entra em `"Cliente e desconto ›"` (`verLateral = true`, `linha 862`) e dali conclui a venda (`319`), salva orçamento (`334`) ou limpa o carrinho via Voltar/Esc (`242`), a próxima venda inicia presa na tela lateral vazia (`R$ 0,00`) com os botões desabilitados e a busca oculta. |
| **B9B-02** | **CONFIRMADO** | **Defeito comprovado** | `app/qml/TelaVender.qml:877–1206` | `Popup { id: receber }` não possui `Flickable` nem limite de altura (`height: Math.min(implicitHeight, Tema.altura - 24)`), cortando formas de pagamento, `painelCred`, `painelCheque` e o botão `"Concluir venda"` em telas de celular. Além disso, a linha de Vale-troca (`1013–1038`) exige `220 px` (`Botao`) + `Selecao` + `150 px` (`Campo`) = `394 px + Selecao`, estourando a largura útil de smartphones (`300–340 px`). |
| **B9B-03** | **CONFIRMADO** | **Defeito comprovado** | `app/qml/SecaoComprar.qml:270–358` (`cPeca`) | Em `cPeca`, três colunas laterais têm larguras fixas (`132 px` na linha `284`, `150 px` na linha `320` e `150 px` na linha `339`), totalizando `508 px` com margens (`18 + 16 + 3*14 + 432 = 508 px`). Em smartphones em retrato (`340–430 px`), a coluna central com `Layout.fillWidth: true` (`linhas 293–317`, onde fica o nome e código da peça) é esmagada para largura zero/negativa. |
| **B9B-04** | **CONFIRMADO** | **Defeito comprovado** | `app/qml/TelaTrocas.qml:153–223, 254–333` | Em `listaVales` (`199, 207`) e `listaFeitas` (`293, 301`), `Tema.celular === true` torna permanentemente visíveis os botões `"Imprimir"`, `"Usar no Vender"` e `"Desfazer"` dentro do único `RowLayout` da linha (junto com as colunas fixas de `70 + 120 px` ou `190 px`), sem quebrar para uma 2ª linha, esmagando os textos de cliente e itens. |
| **B9B-05** | **CONFIRMADO** | **Defeito comprovado** | `app/qml/TelaDevolucao.qml:150–168, 267–269, 406–428` | Quando `Tema.estreito === true`, a tela inicia na aba `tela.parte === "pecas"` (`linha 11`), ocultando o `Flickable` da aba `"como"` (`linha 268`). O único `TextoErro` (`linha 407`) e os botões `"Trocar agora"`/`"Devolver"` (`linhas 411–428`) ficam dentro da aba `"como"`, de modo que erros de quantidade digitados na aba `"Peças"` ficam invisíveis na aba ativa e não há barra inferior para avançar para `"Como devolver"`. |
| **B9B-06** | **CONFIRMADO** | **Defeito comprovado** | `app/qml/SecaoEncomendas.qml:300–469` (`Painel { id: nova_ }`) | O modal `nova_` usa um `ColumnLayout` direto sem `Flickable` e sem limite de altura. Ao digitar o nome do cliente no celular com o teclado virtual aberto e exibir até 5 cartões de sugestões (`achados`, `linhas 391–412`), os campos inferiores e o botão `"Anotar encomenda"` são empurrados para fora da tela sem rolagem. |
| **B9B-07** | **CONFIRMADO** | **Risco** (Usabilidade mobile) | `app/qml/Campo.qml:50–56` + chamadas `.focar()` em modais | `Campo.focar()` só foca o campo se `!Tema.celular || Qt.inputMethod.visible`. Assim, ao abrir modais por toque em botão no Android (com teclado fechado), chamadas a `.focar()` em `AjusteEstoque.qml:121`, `TelaCaixa.qml:33, 314, 375`, `TelaVenda.qml:326`, `TelaVender.qml:918, 948, 961, 1291` e `SecaoEncomendas.qml:324, 341, 349` não focam o campo nem abrem o teclado virtual. |
| **B9B-08** | **CONFIRMADO** | **Risco** (Usabilidade mobile) | `app/qml/SecaoEncomendas.qml:193–275` (`Row { id: acoesEnc }`) | Em `Tema.celular === true`, todos os botões secundários da encomenda ficam visíveis simultaneamente dentro de um `Row` (sem quebra de linha como `Flow`), podendo ultrapassar a largura da tela em aparelhos estreitos. |
| **B9B-09** | **CONFIRMADO** | **Risco** (Usabilidade mobile) | `SecaoVendidos.qml:70–100`, `TelaTrocas.qml:75–102`, `SecaoComprar.qml:108–139`, `SecaoParados.qml:78–109` (+ `TelaOrcamentos.qml:56–77`) | Barras de filtros horizontais combinam `Pilulas` e `Campo { id: busca }` na mesma `Linha` sem `colunasEstreito: 1`, comprimindo o campo de busca em telas estreitas (observado também em `TelaOrcamentos.qml:56–77`). |

---

## 4. Registro Separado de Código Lido, Testes Encontrados e Testes Executados (Bloco 9B)

| Métrica | Valor | Detalhamento |
|---|---:|---|
| **Arquivos QML auditados quanto à responsividade mobile** | **14** | Mesmos 14 arquivos QML compartilhados do Bloco 9A2/9A2C (`4.769 linhas`), inspecionados sob as condições `Tema.celular`, `Tema.estreito`, `Tema.fino` e `Tema.baixo`. |
| **Testes automatizados mobile embutidos nos arquivos QML** | **0** | Não há testes automatizados embutidos nos arquivos QML nem execução histórica dessas 14 telas sob `Tema.celular = true` / `Tema.estreito = true`. |
| **Testes executados neste bloco** | **0** | Conforme o plano, a execução da suíte de testes está isolada no Bloco 18. |

