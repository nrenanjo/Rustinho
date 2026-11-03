# BLOCO 02 A 04 — Requisitos Documentais: Padrão Geral, Unidades, Notificações e Navegação Contextual

> **Data:** 2026-11-02
> **Escopo:** Revisão independente dos relatórios originais dos Blocos 2, 3 e 4 do Gemini 4 Argon (`Texto colado(20261031-145308).txt`, `Texto colado(20261031-145538).txt` e `Texto colado(20261031-175213).txt`) e consolidação dos contratos documentais que regem a auditoria de código dos Blocos 6, 7A, 7B, 8A, 8B1 e 8B2.
> **Política de Imutabilidade:** 100% somente-leitura sobre os arquivos do projeto legado.

---

## 1. Consolidação e Revisão do Bloco 2 (Padrão Geral, Busca, Componentes QML e Algoritmo das 10 Unidades)

### 1.1 Identidade Visual e Padrão Geral dos Componentes QML
Os contratos documentais extraídos das especificações históricas e cruzados com `MANUAL.md` e `docs/DIRETRIZ-CENTRAL.md` estabelecem:

1. **Geometria e Ergonomia (`app/base/Tema.qml` e componentes QML):**
   - Altura de botões, campos e chips interativos: `34 px` (desktop) / `42 px` (celular).
   - Raio de canto arredondado (`radius`): `4 px` (campos/botões/pílulas) e `6 px` (cartões/modais).
   - Espaçamento interno das telas: `18 px` (desktop) / `10 px` (celular).
   - Alinhamento estrito de colunas numéricas à direita com largura fixa e fonte tabular/monoespaçada (`Tema.fonteNumero`).
   - Proibição de `console.log` de depuração em código de produção.
2. **Identidade Visual do Campo de Busca (`app/qml/Campo.qml`):**
   - **Lado esquerdo interno:** Ícone de prancheta com sinal de mais `+` (função de colar da área de transferência).
   - **Lado direito interno:** Ícone de dois retângulos sobrepostos (função de copiar para a área de transferência).
   - **Atalho `F11`:** Posicionado fora do campo de busca, imediatamente à direita.
   - **Proibição expressa:** Vedada a substituição desses ícones por lupa genérica ou ícone de código de barras.

### 1.2 Contrato Integral das Unidades de Medida em "Peças" (2 Linhas × 5 Posições + Linhas Inferiores)
O contrato funcional de unidades de medida exige a preservação integral das seguintes regras (que serão confrontadas diretamente com `nucleo/src/unidades.rs`, `app/qml/EscolherUnidade.qml` e `app/qml/TelaPecas.qml` nos Blocos 8B1 e 8B2):

1. **Contagem de produtos ativos no cadastro (1ª linha lógica — posições 1 a 5):**
   - Conta apenas produtos ativos (`ativo = 1`) que **não sejam serviços** (`servico = 0`).
   - Cada produto ativo conta exatamente `1` ponto para sua unidade de medida (independentemente do saldo físico em estoque).
   - Exibe as 5 unidades com maior número de produtos ativos cadastrados; empate resolvido por ordem alfabética crescente da sigla (`ASC`).
2. **2ª linha lógica inicial (sem vendas nas últimas 24h — posições 6 a 10):**
   - Quando não há vendas líquidas positivas na janela móvel de 24h, exibe as posições 6º a 10º do ranking de cadastro (sem siglas fictícias nem duplicadas).
3. **Janela móvel de 24 horas e critério de "vender mais":**
   - Referência temporal $T$ = instante exato da abertura/consulta (`(T - 24h, T]`), proibido usar dia civil (`00:00`–`23:59`).
   - Critério: **quantidade líquida de itens vendidos** nas últimas 24h (incluindo frações, ex.: `0,350 kg`, `1,5 m`), proibido usar valor financeiro (`R$`) ou número de cupons; desempate por sigla em ordem alfabética crescente.
4. **Substituição posicional progressiva da 2ª linha (da direita para a esquerda — `10 → 6`):**
   - Com 1 unidade vendida nas últimas 24h (`qtd_liquida > 0`), ela entra na posição **10** (extrema direita da 2ª linha).
   - Com $k$ unidades vendidas ($1 \le k \le 5$), elas ocupam as últimas $k$ posições da 2ª linha (posições $10 - k + 1$ até $10$), ordenadas entre si da **maior para a menor quantidade líquida vendida** (da esquerda para a direita dentro do bloco de vendas).
   - Limite máximo de 5 unidades no grupo de vendas da 2ª linha; unidades vendidas excedentes (6ª em diante) vão para as linhas inferiores.
5. **Unicidade absoluta de sigla e recomposição da 1ª linha:**
   - Nenhuma unidade pode aparecer duas vezes na tela. Se uma unidade que estaria na 1ª linha (top 5 cadastro) teve venda líquida $> 0$ nas últimas 24h e entrou no grupo de vendas da 2ª linha, ela cede seu lugar na 1ª linha para a próxima unidade do ranking de cadastro (6º lugar sobe para a 1ª linha).
6. **Cancelamentos, devoluções e estornos na janela de 24h:**
   - Vendas canceladas ou devolvidas cuja venda original ocorreu **dentro** da janela móvel de 24h abatem a quantidade vendida dessa janela (com vínculo à venda original para impedir abatimento duplo).
   - Devoluções de vendas ocorridas **há mais de 24h** não reduzem a contagem da janela atual e jamais geram saldo líquido negativo artificial.
7. **Bloqueio de siglas duplicadas no cadastro:**
   - Comparação `TRIM` + case-insensitive (`"kg"`, `"KG"`, `" KG "` são a mesma sigla), retornando erro claro em vermelho.
8. **Linhas inferiores e adaptação responsiva:**
   - Todas as demais unidades (fora das 10 primeiras posições) são exibidas nas linhas inferiores em **ordem alfabética crescente pela sigla**, sem ocultar nenhuma unidade cadastrada.
   - Layout de até **5 botões por linha no Desktop** e até **3 botões por linha no Smartphone/Android**.
9. **19 Critérios Mínimos de Teste Obrigatórios:** Catalogados no relatório histórico e serão verificados linha a linha contra os testes existentes em `nucleo/src/unidades.rs` no Bloco 8B1.

---

## 2. Consolidação e Revisão do Bloco 3 (Sistema de Notificações Persistentes, Guia e Avisos Rápidos)

### 2.1 Distinção Estrita entre os Três Mecanismos de Comunicação Visual
1. **Notificações Persistentes de Condição (`nucleo/src/avisos.rs`, `app/qml/Notificacao.qml`, `app/qml/TelaInicio.qml`):**
   - Governadas pelo estado real do banco/rede (`avisos_do_dia`, `da_senha`), atualizadas a cada 20 segundos na Tela Inicial.
   - Marcas locais (`aviso_escondido.<id>` e `aviso_minimizado.<id>`) ficam em `CONFIG_DA_LOJA` (não sincronizadas entre caixas).
   - Hierarquia na `TelaInicio.qml`: Campo de busca no topo $\rightarrow$ Faixas críticas sempre abertas em coluna $\rightarrow$ No máximo uma faixa não crítica aberta por vez $\rightarrow$ Fila (`Flow`) de quadrados minimizados $\rightarrow$ Grade das 14 funções principais.
2. **Orientação Passageira da Navegação Guiada (`app/qml/Guia.qml` reutilizando `app/qml/Notificacao.qml`):**
   - Vinculada exclusivamente à sessão ativa de navegação guiada (`guiaAtiva`), sempre em faixa aberta no topo da janela (relatada sobre `T.Overlay.overlay`, `z: 1000`), sem gravar marcas `aviso_*` no banco.
3. **Avisos Rápidos / Feedback Efêmero (`app/qml/Aviso.qml`):**
   - Mensagens curtas de feedback imediato de ação, que também devem respeitar o posicionamento superior/contextual aprovado (nunca sobrepostas no rodapé como na evidência histórica `erro-notificação_embaixo.png`).

### 2.2 Regras das Três Categorias de Notificações Persistentes
- **`CRITICA`:** Sempre expandida em faixa; não pode ser minimizada nem escondida (gestos/cliques de fechar são bloqueados na UI e recusados no núcleo por `avisos::minimizar` / `escondida_hoje`); papel acessível `Accessible.AlertMessage`.
- **`ATENCAO`:** Começa minimizada em quadrado pastel com `IconeFuncao` central e triângulo PNG do tema no canto superior direito; clique alterna quadrado $\leftrightarrow$ faixa (fechando outra não crítica aberta); arrasto para a direita minimiza para quadrado; arrasto para a esquerda **não esconde** (volta e permanece visível); papel acessível `Accessible.Button`.
- **`INFORMATIVA`:** Começa minimizada em quadrado; clique alterna quadrado $\leftrightarrow$ faixa; arrasto para a direita minimiza; arrasto para a esquerda esconde até a virada do dia (`aviso_escondido.<id>` com a data local `hoje`), reaparecendo no dia seguinte se a condição persistir.

### 2.3 Achados Históricos Catalogados no Bloco 3 para Inspeção no Código (Bloco 6 e Bloco 7B)
| ID Histórico | Descrição no Relatório Gemini 4 Argon | Onde Será Verificado no Código |
|---|---|---|
| **`NOTIF-01`** | Em `nucleo/src/avisos.rs` (`completar`), notificações `"conflitos"`, `"codigos"` e `"codigos_bloqueio"` exibem o botão `"Resolver no Gerente"`, embora a seção de destino seja `Simultaneidade > Duplicatas` (`secao: "duplicatas"`). | **Bloco 6** (`nucleo/src/avisos.rs`) |
| **`NOTIF-02`** | Divergência documental vs. código: relatórios históricos de 0.9.15/0.9.16 apontavam que `outra_loja` recebia o botão genérico `"Abrir"` em vez de `"Ver caixas"`, mas análise posterior indicou que 0.9.17 já teria `"Ver caixas"`. | **Bloco 6** (`nucleo/src/avisos.rs`) |
| **`NOTIF-03`** | Operador com perfil Caixa que clica em notificação de Simultaneidade na `TelaInicio.qml` e cancela a senha no modal `PedirGerente` fica preso na aba `Loja` de `TelaConfiguracoes.qml` em vez de voltar à `TelaInicio.qml`. | **Bloco 6 / Bloco 7B** (`app/qml/TelaConfiguracoes.qml`, `app/qml/TelaInicio.qml`) |

---

## 3. Consolidação e Revisão do Bloco 4 (Navegação Contextual Guiada e Matriz C8)

### 3.1 Arquitetura da Navegação Contextual Guiada (`Main.qml`, `Guia.qml`, `requisitos.rs`)
1. **Mecanismo único sobre `StackView` (`app/qml/Main.qml`):**
   - `janela.guiar(requisito, detalhe, aoVoltar)` mantém a tela de origem viva na pilha (preservando arquivos escolhidos, filtros, checkboxes e rascunhos em memória) e empilha a tela de destino.
   - O cabeçalho da tela de destino exibe `"← <Nome da Tela de Origem>"` (ex.: `"← Peças"`).
2. **Recorte visual não bloqueante (`app/qml/Guia.qml`):**
   - Escurece o entorno do campo-alvo com 4 retângulos e desenha borda de destaque sem interceptar cliques ou digitação.
3. **Validação exclusiva pelo núcleo (`nucleo/src/requisitos.rs`):**
   - A UI não decide sozinha que um requisito foi cumprido; consulta `requisito_conferir` (`SO_LEITURA`) no núcleo e, quando aplicável, `arquivos_conferir` (`SO_LEITURA`) antes de retomar a prévia na origem.
   - **Proibição de execução automática no retorno:** Ao retornar à tela de origem com o requisito satisfeito, o sistema atualiza a prévia/estado, mas **nunca** executa cadastros, importações, transferências ou lançamentos financeiros automaticamente sem clique explícito do operador.
4. **Três modos de decisão:**
   - `Auto` (redirecionamento automático apenas para requisito obrigatório ausente com solução única e inequívoca: Casos 1, 2, 3 e subcasos inequívocos de 9);
   - `Opções` (escolha explícita do usuário quando há mais de um caminho: Casos 4, 5, 6, 8);
   - `Decisão` (exclusivo do gerente na própria tela, sem redirecionamento automático: Casos 2b, 7).

### 3.2 Achados Históricos Catalogados no Bloco 4 para Inspeção no Código (Blocos 7A, 7B e 10)
| ID Histórico | Descrição no Relatório Gemini 4 Argon | Onde Será Verificado no Código |
|---|---|---|
| **`NAV-01`** | Guiar a partir de `TelaFiscal.qml` (embutida via `Loader` dentro de `TelaConfiguracoes.qml`) empilha uma segunda instância de `TelaConfiguracoes.qml` sobre a primeira e, no retorno, não recarrega `tela.config` da 1ª instância. | **Bloco 7A / Bloco 7B** (`app/qml/Main.qml`, `app/qml/TelaConfiguracoes.qml`, `app/qml/TelaFiscal.qml`) |
| **`NAV-02`** | Trocar manualmente de pílula em `TelaConfiguracoes.qml` durante uma guia ativa oculta o recorte escuro (`alvo.visible == false`), mas mantém a faixa de orientação presa no topo sobre abas que não contêm o campo-alvo. | **Bloco 7A / Bloco 7B** (`app/qml/Guia.qml`, `app/qml/TelaConfiguracoes.qml`) |
| **`NAV-03`** | Variável `let deOutra = 0` em `app/qml/TelaCompras.qml` tornou-se código morto após o Caso 2b separar notas de outro CNPJ em `r.dados.conflitos` (`ConflitoCnpj.qml`). | **Bloco 7B** (`app/qml/TelaCompras.qml`) |
| **`NAV-04`** | Confirmar se `importar::cnpj_loja` (`nucleo/src/importar.rs`) e `compras.rs` validam `config::documento_da_loja_valido(&cnpj)` além de `!cnpj.is_empty()`. | **Bloco 7B** (`nucleo/src/importar.rs`, `nucleo/src/compras.rs`) |
| **Casos 4, 5, 6, 7, 8 da Matriz C8** | Relatados no histórico como pendentes ou parciais na v0.9.17 (Etiquetas com código ambíguo, Transferência sem conta destino, Rascunho de OS com caixa incompatível, Operação financeira bloqueada por referência, botão na `faixaModoSeguro`). | **Blocos 7A, 8B2, 10, 12, 15** |
| **Caso 10c (`receber.rs`)** | Conta a receber com pagamento parcial em várias contas: classificado como hipótese de bug financeiro no núcleo (`nucleo/src/receber.rs`), não caso de navegação. | **Bloco 10** (`nucleo/src/receber.rs`) |

---

## 4. Contabilização Separada: Leitura vs. Testes Encontrados vs. Testes Executados

| Categoria | Quantidade | Detalhamento |
|---|---:|---|
| **(a) Relatórios históricos lidos integralmente neste bloco** | **3 relatórios originais** | `Texto colado(20261031-145308).txt` (Bloco 2), `Texto colado(20261031-145538).txt` (Ajustes Bloco 2 + Bloco 3), `Texto colado(20261031-175213).txt` (Ajustes Bloco 3 + Bloco 4). |
| **(b) Testes encontrados/especificados nos contratos** | **19 critérios de unidades** + **5 cenários (A–E) / 32 checks de navegação guiada** | Catalogados documentalmente para verificação direta contra `nucleo/src/unidades.rs`, `nucleo/src/testes_avisos.rs`, `nucleo/src/testes_nav.rs`, `app/testes/telas_notificacoes.py` e `app/testes/telas_navegacao_guiada.py` nos Blocos 6, 7A, 8B1 e 18. |
| **(c) Testes executados neste bloco** | **0** | Etapa estritamente documental; nenhuma execução de código realizada neste bloco. |

