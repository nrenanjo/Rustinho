# Rustinho 0.9.17 — Handoff da auditoria Gemini 4 Argon

**Data de consolidação:** 02/11/2026  
**Finalidade:** entregar a um novo conjunto de agentes (VS Code / Gemini / Claude Code / Reversa) o histórico da auditoria anterior, para que seja refeito desde o Bloco 0 e validado independentemente.  
**Base de código de referência:** Rustinho 0.9.17.  
**Auditoria original:** realizada em blocos com Gemini 4 Argon, esforço Alto, conforme configuração mostrada nas capturas de tela do usuário.

---

## 0. Instrução central aos novos agentes

Este documento é **histórico e ponto de partida, não uma certificação de correção**. Não aceitem nenhum achado do Gemini anterior como verdade sem reabrir o código-fonte original e verificar a evidência.

A tarefa solicitada é **recomeçar no Bloco 0**, não continuar automaticamente no Bloco 10. O novo processo deve:

1. Inventariar novamente o ZIP/repositório recebido, conferir versão, hash, árvore e ferramentas disponíveis.
2. Repetir a auditoria documental desde o Bloco 0, registrando o que pode ou não ser acessado.
3. Fazer uma auditoria independente do código-fonte completo, módulo por módulo, sem pular arquivos por já constarem como auditados aqui.
4. Confrontar cada achado anterior com a fonte atual e classificá-lo como: **confirmado**, **não confirmado**, **refutado**, **corrigido no código atual**, **não aplicável**, ou **dependente de teste runtime**.
5. Manter trilha de evidência: caminho, símbolo/função/componente, linhas atuais, trecho literal, cenário, impacto e classificação.
6. Distinguir sempre: código lido; teste encontrado; teste escrito; teste executado; teste aprovado; validação de ambiente real.
7. **Não alterar o código durante a fase de auditoria.** Não aplicar correções, refatorações, formatação ou migrações sem autorização expressa do usuário.
8. Não declarar teste executado sem registrar comando, ambiente, saída e resultado. Para testes que não podem ser executados, explicar a limitação.
9. Não afirmar que um relatório foi salvo no repositório se ele só foi escrito na resposta do agente.
10. Não tratar documentação histórica ou requisito como implementação. Marcar claramente “documentado”, “confirmado no código” ou “não verificado”.

### Regra para conflitos entre relatórios e código

A versão/hash do código que estiver aberta no VS Code é a fonte primária para a auditoria atual. Os textos em `relatorios_originais/` são evidência do que o Gemini anterior declarou, não substituem a fonte. Se a numeração de linhas divergir, prevalecem as linhas conferidas no arquivo atual com ferramenta equivalente a `nl -ba`.

---

## 1. Escopo do que já foi feito

O trabalho histórico teve duas grandes partes:

- **Blocos 0 a 5D2:** auditoria documental, requisitos, arquitetura, histórico de mudanças, navegação, padrões visuais e documentação de uma crate fiscal independente.
- **Blocos 6 a 9B:** auditoria estática de partes do código Rust e QML do Rustinho 0.9.17.

O levantamento anterior contabilizou **45 arquivos-fonte únicos** nas etapas de código dos Blocos 6 a 9A2 Complementar. Os 12 arquivos examinados no Bloco 9B são os mesmos arquivos QML já examinados no desktop; não devem ser contados outra vez. O número 45 é um registro do trabalho anterior, não uma declaração de cobertura total do projeto.

**Não há evidência de que o projeto inteiro tenha sido auditado.** A lista total de arquivos e a porcentagem de cobertura devem ser refeitas pelo novo agente, a partir da árvore/hash que ele realmente receber.

---

## 2. Blocos 0 a 5D2 — auditoria documental anterior

### Bloco 0 — recepção, integridade e limites do material

- Foram recebidos o ZIP do acervo documental `rus-rus.zip`, o PDF `Log_Consolidado_Rustinho_0.9.17.pdf` e materiais auxiliares.
- O PDF tem 257 páginas. Na sessão original, o material acessível e efetivamente lido foi limitado às páginas **1–95**; as páginas **96–257** ficaram inacessíveis. A auditoria não deve apresentar o PDF inteiro como lido.
- O Bloco 0 estabeleceu a necessidade de diferenciar origem, hash, arquivo efetivamente recebido e alcance da leitura.
- Os agentes atuais devem refazer hashes e inventário, pois não se deve presumir que os arquivos recebidos agora são byte a byte idênticos aos da sessão anterior.

### Bloco 1 — inventário documental

- Foi feita reconciliação do acervo: materiais originais, entradas internas do ZIP e documentos Markdown duplicados/relacionados.
- Os relatórios históricos fazem distinção entre **19 materiais originais**, **138 entradas internas** e um conjunto de Markdown com duplicidades (21 entradas / 18 conteúdos únicos em uma das reconciliações).
- O novo agente deve recalcular essas quantidades a partir dos arquivos recebidos, sem copiá-las automaticamente.

### Bloco 2 — padrão geral, busca, unidades e requisitos

Foram consolidados padrões de identidade visual, componentes QML, campo de busca, unidades de medida e requisitos gerais do produto. Entre os requisitos de unidades registrados pelo usuário:

1. Primeira linha: cinco unidades com maior quantidade de produtos ativos cadastrados, excluindo produtos inativos e serviços sem estoque; ordenar por quantidade decrescente e, em empate, sigla alfabética.
2. Segunda linha: cinco unidades com maior volume líquido de vendas nas últimas 24 horas.
3. A classificação das vendas deve considerar devoluções vinculadas à venda original; devolução de venda anterior à janela de 24 horas não deve reduzir vendas recentes.
4. No desktop, a substituição da segunda linha ocorre da direita para a esquerda, nas posições 10 até 6; as cinco mais vendidas permanecem.
5. Unidades restantes aparecem em ordem alfabética pela sigla. Não repetir siglas.

Essas regras são requisitos aprovados/documentados; a implementação foi posteriormente examinada nos Blocos 8B1 e 8B2.

### Bloco 3 — notificações e tela inicial (documental)

- Consolidou a distinção entre avisos persistentes e avisos rápidos, categorias de gravidade, identidade visual e comportamento de notificação.
- A auditoria posterior do código está no Bloco 6; requisitos visuais e implementação não devem ser confundidos.

### Bloco 4 — navegação contextual guiada

- Consolidou a matriz e os requisitos de navegação contextual, linkagem de telas, foco e abandono/retomada de guias.
- A verificação de código ocorreu depois nos Blocos 7A e 7B.

### Blocos 5A a 5D2 — crate fiscal independente e documentação técnica

- Foram analisados documentos de origem, arquitetura, contrato neutro, plano de integração, leiaute, provedores e segurança da crate `rustinho-fiscal v0.2.3`.
- O histórico descreve essa crate como **em desenvolvimento**, sem demonstrar integração de produção com o núcleo Rustinho 0.9.17, homologação SEFAZ ou operação fiscal produtiva.
- Foram identificados gates pendentes: compilação/testes Rust da crate, definição tributária/contábil, cadastro fiscal completo do emitente, regras por UF, decisão de provedor/arquitetura desktop e Android, testes reais com ACBrMonitorPLUS e SEFAZ, numeração/outbox e reconciliação de estados incertos.
- Há uma allowlist técnica documentada de quatro combinações CFOP/CST/CSOSN no material v0.2.3; ela foi classificada como **documentada, mas não verificada no código Rust** naquela etapa. A ausência de variante MEI e de outras combinações deve ser revalidada antes de qualquer conclusão.
- Os relatórios registraram simulação matemática independente em Python para rateio de descontos (5.659.572 combinações), mas isso **não executou a função Rust**.
- Os relatórios indicaram que `cargo fmt`, `cargo clippy`, `cargo test` e `cargo build --release` não foram executados no ambiente histórico da crate fiscal.
- Há incertezas normativas e temporais sobre Notas Técnicas, inutilização, regimes e regras estaduais. Revalidar em fontes oficiais atuais; nunca tratar citação em PDF histórico como prova de vigência.
- Limitação fundamental: o Apêndice H do PDF (páginas 96–257), que conteria outros artefatos da crate, não foi acessível na etapa documental anterior.

**Atenção:** os Blocos 5A–5D2 são em grande parte auditoria de documentação e arquitetura. Não equivalem a uma auditoria completa dos módulos fiscais atuais (`nucleo/src/fiscal.rs`, `nfe_xml.rs`, `nfe_assinatura/` e integrações).

---

## 3. Blocos 6 a 9B — auditoria estática anterior do código

### Bloco 6 — notificações e tela inicial

**Arquivos:**
- `nucleo/src/avisos.rs`
- `nucleo/src/testes_avisos.rs`
- `app/qml/Notificacao.qml`
- `app/qml/IconeFuncao.qml`
- `app/qml/Aviso.qml`
- `app/qml/TelaInicio.qml`

**Achados reportados:**
- `NOTIF-01`: rótulo “Resolver no Gerente” conflita com o destino “Simultaneidade › Duplicatas”.
- `NOTIF-02`: `outra_loja` foi confirmado como já corrigido em 0.9.17, usando “Ver caixas”.
- `NOTIF-03`: linkagem de notificações de Simultaneidade e cancelamento da senha do gerente ficou para inspeção de `TelaConfiguracoes.qml`, feita no Bloco 7B.
- `B6-01`: `Aviso.qml` posiciona aviso rápido no rodapé, embora o requisito visual registrado fosse no topo.
- `B6-02`: fallback crítico de Modo Seguro não preenche título longo/resumo/ação em uma rota de erro.
- `B6-03`: aviso informativo escondido pode reaparecer após atualização local dos delegates antes do timer periódico.
- `B6-04`: entidades HTML escapadas podem ser exibidas literalmente em `Text.PlainText`.
- `B6-05`: campo de busca da Tela Inicial não reproduz a identidade visual aprovada.
- `B6-06`: rótulos genéricos “Abrir” e caminhos textuais incompletos em algumas notificações.
- `B6-07`: risco de marcação/injeção de texto em `Text.StyledText`, pendente de validação de origem e escape.
- `B6-08`: risco de diferença de data/fuso na contagem de dias sem backup.

**Testes:** o relatório identificou testes unitários e de integração escritos; a execução desses testes não foi comprovada naquela auditoria.

### Bloco 7A — navegação e requisitos, primeira parte

**Arquivos:**
- `nucleo/src/requisitos.rs`
- `nucleo/src/testes_nav.rs`
- `app/qml/Main.qml`
- `app/qml/Guia.qml`
- `app/qml/ImportarCadastros.qml`
- `app/qml/ConflitoCnpj.qml`

**Achados reportados:**
- `B7A-01`: F11 global pode abrir TelaPecas durante navegação guiada, mesmo quando a tela de destino não oferece o campo de busca esperado.
- `NAV-01`: `guiar()` pode empilhar outra instância de `TelaConfiguracoes` quando já existe uma instância ativa.
- `NAV-02`: rolagem/visibilidade do alvo pode ocultar destaque sem encerrar o aviso/estado da guia.
- `B7A-02`: troca/início de tela pode deixar estado de guia residual em determinadas sequências de ciclo de vida.
- `B7A-03`: falhas no pós-importação exibem erro sem retomar navegação guiada; limites de altura/rolagem da importação foram apontados.
- `B7A-04`: banner do Modo Seguro contém caminho incompleto e não oferece ação adequada em determinado caso.
- `B7A-05`: diretórios temporários criados por testes podem não ser removidos.
- Validação de CNPJ salvo pelo requisito foi identificada no núcleo.

**Testes:** oito funções de teste foram reportadas no arquivo de testes; execução runtime não comprovada.

### Bloco 7B — configurações, compras, importação e campos

**Arquivos:**
- `nucleo/src/importar.rs`
- `nucleo/src/compras.rs`
- `app/qml/TelaConfiguracoes.qml`
- `app/qml/TelaCompras.qml`
- `app/qml/TelaFiscal.qml`
- `app/qml/PedirGerente.qml`
- `app/qml/Campo.qml`

**Achados reportados:**
- `NOTIF-03`: cancelamento da senha do gerente ao entrar em Simultaneidade pode deixar o operador na seção “Este computador”, sem callback de desistência.
- `B7B-01`: após cancelar modal de senha, foco/orientação para preencher CNPJ pode não ser restaurado.
- `NAV-01`: componente de TelaFiscal pode acionar guia que empilha segunda TelaConfiguracoes; primeira instância pode ficar obsoleta.
- `NAV-02`: pills permitem mudar de seção durante uma guia, podendo fazer o destaque sumir enquanto o banner permanece.
- `B7B-02`: texto/disclaimer antigo diz que assinatura XML não é verificada, embora `compras::importar_xml` use leitura verificada ICP-Brasil (revalidar na versão atual).
- `B7B-03`: a exigência de CNPJ em `compras::conferir_nota` foi reclassificada como melhoria/coerência, pois os comandos de importação relevantes já exigem CNPJ no caminho auditado.
- `B7B-04`: risco de guia/destaque não atualizar corretamente quando outro campo de configuração falha.
- `B6-05`: campo visual em `Campo.qml` não segue a referência aprovada (ícones e F11).
- `abandonarGuia` chamado em `Component.onDestruction` foi apontado como causa potencial de erro `null`; requer reprodução runtime antes de afirmar causalidade.
- `let deOutra` foi apontado como código morto.

**Testes:** dois testes de importação e onze testes em `compras.rs` foram reportados como escritos; execução não comprovada.

### Bloco 8A — API central e permissões

**Arquivo:** `nucleo/src/api.rs`.

**Rastreabilidade corrigida:** arquivo confirmado com **4.550 linhas**, **32 atributos `#[test]`**, SHA-256 reportado `2271ad7ff76eab60a26a07b0856a8d0686b2485653ae53cb5ed772dd372ffd22`. Referências anteriores de linhas foram corrigidas com base em arquivo produzido por `nl -ba`; novas auditorias devem comparar hash do arquivo atual.

**Achados centrais reportados:**
- `B8A-01a/b/c`: token de liberação do gerente é associado a usuário/comando/hash, mas não à ação específica autorizada, e não é consumido após o comando. Em comandos que internamente verificam outra ação, o mesmo token pode satisfazer uma segunda verificação (exemplos reportados: `config_salvar`/`precos`, `sync_salvar`/`muda_rede`, `usuario_salvar`/`mexe_em_gerente`).
- A primeira ação que dispara a solicitação de senha pode ocultar outras alterações contidas no mesmo payload, prejudicando a clareza da autorização.
- `gerente_liberar` pode apresentar ação padrão incorreta em comando com múltiplas ações.
- O ponto deve ser revalidado integralmente e testado com casos negativos; é achado de autorização, com impacto potencial alto.

**Testes:** 32 testes foram encontrados/reportados como escritos, mas execução runtime não comprovada pela auditoria.

### Bloco 8B1 — busca, produtos e unidades (núcleo)

**Arquivos:** `nucleo/src/busca.rs`, `nucleo/src/produtos.rs`, `nucleo/src/unidades.rs`.

**Contagens corrigidas:** `busca.rs` 383 linhas; `produtos.rs` 1.387 linhas; `unidades.rs` 154 linhas.

**Achados reportados:**
- `B8B1-01`: códigos de fornecedor/códigos antigos não participam adequadamente da busca parcial/reordenada; comparação reportada é restrita a igualdade do compactado inteiro.
- `B8B1-02`: tokens alfanuméricos como `rg08`/`rg-08` não são decompostos como `rg 08`.
- `B8B1-03`: números de 1900 a 2100 podem ser interpretados como ano de veículo e filtrar produtos encontrados por código.
- `B8B1-04`: rótulo `achou` pode não reconhecer código curto ou exclusivamente alfabético.
- `B8B1-05`: contagem por unidade inclui produtos inativos e serviços.
- `B8B1-06`: ausência da regra de organização móvel de unidades foi inicialmente tratada como defeito, depois corretamente delimitada como ausente nos módulos do núcleo e pendente da UI; o Bloco 8B2 confirmou ausência na camada examinada.
- `B8B1-07`: `unidades::salvar` faz `ON CONFLICT (sigla) DO UPDATE`, podendo sobrescrever cadastro existente sem confirmação da UI; confirmado no Bloco 8B2.
- `B8B1-08`: salvar produto pode guardar código anterior sem a mesma confirmação exigida em `trocar_codigo`.
- `B8B1-09`: risco de frações em unidades indivisíveis foi depois confirmado ponta a ponta no Bloco 8B2.
- `B8B1-10`: melhoria: deleção/reinserção incondicional de tabelas filhas pode gerar churn de UUID e custo de sincronização.
- `B8B1-11`: risco de N+1 em buscas.
- `B8B1-12`: dependência de transação externa; métodos aceitam `&Connection` sem savepoint próprio.

**Testes:** quatro testes escritos foram reportados nos três módulos; execução não comprovada.

### Bloco 8B2 — interface de busca, unidades e estoque

**Arquivos:**
- `app/qml/EscolherUnidade.qml`
- `app/qml/TelaPecas.qml`
- `app/qml/TelaPeca.qml`
- `nucleo/src/estoque.rs` (reaparece no Bloco 9A1; contar apenas uma vez)

**Achados reportados:**
- `B8B1-06` fechado: não foi encontrada a organização móvel em duas linhas, classificação por vendas líquidas em 24h, tratamento de retornos e ordenação alfabética nos arquivos examinados.
- `B8B1-07` confirmado na interface: permite salvar sigla já existente sem prevenção clara; risco de sobrescrita de descrição/fracionada.
- `B8B1-09` confirmado na pilha: interface aceita decimais para estoque inicial/mínimo; validação de produto e funções de lançamento/ajuste no núcleo não bloqueiam frações para unidade indivisível.
- `B8B1-10` permanece melhoria de churn nas tabelas filhas.
- N+1 e fronteiras transacionais permanecem riscos a confirmar em contexto mais amplo.

**Testes:** nenhum teste encontrado nos quatro arquivos examinados nesse bloco; execução runtime 0.

### Bloco 9A1 — núcleo de vendas, devoluções, caixa e estoque

**Arquivos:**
- `nucleo/src/caixa.rs`
- `nucleo/src/estoque.rs` (já auditado no Bloco 8B2)
- `nucleo/src/vendas.rs`
- `nucleo/src/devolucoes.rs`
- `nucleo/examples/estoque_teste.rs`
- `nucleo/tests/caixas.rs`
- `nucleo/tests/estavel/t2_estoque.rs`

**Contagens reportadas:** caixa 523; estoque 97; vendas 1.477; devoluções 1.017; exemplo 93; testes/caixas 310; t2_estoque 249 linhas.

**Achados reportados:**
- `B9A1-01`: cálculo de devolução não força quantidade inteira quando unidade é indivisível; pode retornar `0,5 UN` e somar fração ao estoque.
- `B9A1-02`: cancelamento de venda em dinheiro pode não conferir o dinheiro físico esperado após sangria e pode afetar sessão de outro caixa enquanto ainda aberta.
- `B9A1-03`: desfazer devolução em outro computador pode alterar o esperado da gaveta da sessão original em vez de compensar a gaveta atual.
- `B9A1-04`: exemplo de estoque pode falhar por ausência de `chaveOperacao` em chamadas `NAO_REPETIR`.
- `B9A1-05`: risco de falta de `sync::exigir_editavel` em cancelamento de venda/gravação de devolução em certos caminhos.
- `B9A1-06`: desconto global aplicado após desconto individual pode superar `desconto_maximo`; confirmado como problema da interface/fluxo no Bloco 9A2 Complementar.
- `B9A1-07`: hipótese pendente de investigar estorno direto de recebimento com `forma = 'devolucao'`; **não auditado ainda** porque o Bloco 10 foi recusado pelo Gemini.
- `B9A1-08`: lacunas na trilha de auditoria de estoque/devoluções/vendas/orçamentos.
- `B9A1-09`: encapsulamento transacional próprio classificado como melhoria, não defeito comprovado, pois os chamadores podem fornecer transação externa.
- A janela móvel de 24h precisa usar venda original e devolução vinculada ao item original; não usar movimento de estoque como substituto sem vínculo apropriado.

**Testes:** 22 testes foram reportados como escritos nos arquivos do bloco; runtime 0.

### Bloco 9A2 — telas desktop de estoque, venda, caixa e devolução

**Arquivos:**
- `app/qml/AjusteEstoque.qml`
- `app/qml/LinhaVenda.qml`
- `app/qml/TelaCaixa.qml`
- `app/qml/TelaDevolucao.qml`
- `app/qml/TelaEstoque.qml`
- `app/qml/TelaVenda.qml`

**Achados reportados:**
- `B9A1-01`: UI também aceita quantidade fracionária para unidade indivisível em devolução/ajuste.
- `B9A1-02`: tela de venda/caixa não expõe adequadamente sessão, dispositivo, estado da gaveta e consequência do cancelamento; saldo negativo pode não ter destaque visual.
- `B9A1-03`: desfazimento de devolução não pôde ser concluído sem `TelaTrocas.qml` (analisada no complemento).
- `B9A1-06`: campos de desconto globais e integração de recebimento não estavam todos nestes arquivos; complementação necessária.
- `B9A1-08`: histórico exibido pode depender de tabela de auditoria e omitir movimentos de estoque decorrentes de venda/cancelamento/devolução.
- `B9A2-01`: chip “Veio com defeito” pode não marcar item que ainda não estava em `tela.escolha`; a inicialização posterior usa `defeito: false`.
- `B9A2-02`: cancelamento da modal do gerente pode ser apresentado como erro do campo quantidade/motivo.
- `B9A2-03`: primeira conta de movimento é pré-selecionada; erros de conta podem aparecer no campo valor.
- `B9A2-04`: tela de venda não indica claramente estado de devolução parcial/total.
- `B9A2-05`: melhorias de chave de operação, destaque visual e recuperação de caixa antigo.

### Bloco 9A2 Complementar — restante das telas de vendas e compras

**Arquivos:**
- `app/qml/SecaoComprar.qml`
- `app/qml/SecaoEncomendas.qml`
- `app/qml/SecaoParados.qml`
- `app/qml/SecaoVendidos.qml`
- `app/qml/TelaTrocas.qml`
- `app/qml/TelaVender.qml`

**Achados reportados:**
- `B9A1-06` confirmado como defeito: desconto global não é submetido ao limite individual por produto; `venda_concluir` é chamado sem autorização adicional do gerente nesse fluxo.
- `B9A2C-01`: botão Vale-troca pode preencher `0,00` se o campo dinheiro já estiver preenchido com o total e `calc.falta` for zero.
- `B9A2C-02`: custos de reposição/estimados podem ser mostrados a operador sem permissão de resultados em `SecaoComprar.qml`.
- `B9A2C-03`: certos erros de campo em modal Nova encomenda podem não ser exibidos.
- `B9A2C-04`: `abrirReceber()` não verifica `erroDesconto`; o total pode não ser atualizado ao digitar por ausência de `onEditado`. A conclusão de que uma venda inválida passa no backend precisa de teste/inspeção backend, não foi provada só pela UI.
- `B9A2C-05`: carregamento de orçamento usa `tabela: 1`, ignorando tabela original; requisito de suporte a cinco tabelas precisa ser reconciliado documentalmente.
- `B9A2C-06`: aba “Já devolvidas” omite caixa/vendaCaixa; botão Desfazer não é desabilitado com vale usado embora o backend bloqueie; texto sobre estoque não diferencia item defeituoso.
- Melhorias adicionais: validação de orçamento, identificação de unidade, conferência de callback de modal.

**Testes:** zero testes QML encontrados/execução runtime 0.

### Bloco 9B — telas responsivas compartilhadas desktop/Android

**Arquivos examinados:** os mesmos 12 QML já contados nos Blocos 9A2 e 9A2 Complementar:
- `TelaVender.qml`, `TelaEstoque.qml`, `SecaoEncomendas.qml`, `SecaoComprar.qml`, `TelaVenda.qml`, `SecaoVendidos.qml`, `TelaCaixa.qml`, `LinhaVenda.qml`, `SecaoParados.qml`, `TelaTrocas.qml`, `TelaDevolucao.qml`, `AjusteEstoque.qml`.

**Constatação arquitetural:** os relatórios afirmam que são os mesmos arquivos-fonte QML para desktop e Android, usando as propriedades responsivas do singleton `Tema` (`Tema.celular`, `Tema.estreito`, `Tema.fino`, `Tema.baixo` e outras). Portanto, não existe uma segunda cópia independente desses 12 arquivos para Android.

**Achados herdados que o relatório diz persistirem no Android:** os 14 casos foram marcados SIM no relatório original: `B9A1-01`, `B9A1-02`, `B9A1-03`, `B9A1-06`, `B9A2-01`, `B9A2-02`, `B9A2-03`, `B9A2-04`, `B9A2C-01`, `B9A2C-02`, `B9A2C-03`, `B9A2C-04`, `B9A2C-05` e `B9A2C-06`. A revisão independente atual deve confirmar cada um novamente no estado atual do código.

**Achados específicos de modo móvel reportados:**
- `B9B-01`: `limparVenda()` não reseta `verLateral = false`; venda nova pode iniciar presa na página Cliente/Total vazia.
- `B9B-02`: Popup de recebimento não tem rolagem/limite adequado de altura; linhas fixas podem estourar a largura em celular.
- `B9B-03`: `cPeca` em `SecaoComprar.qml` tem várias colunas de largura fixa, esmagando a coluna central em tela estreita.
- `B9B-04`: listas `listaVales` e `listaFeitas` podem colocar botões permanentes na mesma linha e esmagar texto em celular.
- `B9B-05`: erro e botões de ação ficam na aba “Como devolver”, mas a aba “Peças” é mostrada inicialmente; o erro pode ficar invisível e faltar caminho claro para avançar.
- `B9B-06`: modal Nova encomenda não tem Flickable/limite de altura; teclado e sugestões podem empurrar campos/botões para fora da tela.
- `B9B-07`: chamadas a `Campo.focar()` podem não abrir teclado/focar no Android com teclado virtual fechado.
- `B9B-08`: `Row` de ações de encomenda pode não quebrar linha, transbordando em estado com muitos botões.
- `B9B-09`: barras de filtros podem comprimir campo de busca em telas estreitas.

**Testes:** zero testes QML embutidos e nenhum teste em emulador/aparelho Android reportado. O relatório reconhece ausência de testes runtime mobile.

---

## 4. Estado dos testes — regra de leitura obrigatória

Os números abaixo são registros do que os relatórios dizem estar escrito no código, não de testes executados:

- Bloco 6: testes de avisos unitários e de integração reportados como escritos.
- Bloco 7A: 8 testes reportados como escritos.
- Bloco 7B: 2 testes de importação e 11 de compras reportados como escritos.
- Bloco 8A: 32 testes em `api.rs` reportados como escritos.
- Bloco 8B1: 4 testes reportados como escritos.
- Bloco 8B2: nenhum teste nos arquivos do bloco.
- Bloco 9A1: 22 testes reportados como escritos.
- Blocos 9A2, 9A2 Complementar e 9B: nenhum teste QML no escopo; nenhum teste Android em runtime reportado.

**Não converter essa lista em “testes passaram”.** O agente novo deve executar a suíte atual se houver toolchain e ambiente disponíveis, coletar saída completa e explicar quais testes foram filtrados, ignorados ou bloqueados.

---

## 5. Cobertura: o que não pode ser presumido auditado

O Bloco 10 foi planejado para `receber.rs`, `financeiro.rs`, `pagar.rs`, `cheques.rs`, `comissoes.rs`, `dre.rs` e `dre/testes.rs`. O Gemini respondeu com uma recusa genérica; portanto, **o Bloco 10 não foi executado e não está concluído**. Em particular, `nucleo/src/receber.rs` permanece pendente, inclusive o achado hipotético `B9A1-07`.

Também não há evidência de auditoria de código completa, neste histórico, dos seguintes grupos (lista ilustrativa a reconciliar com a árvore real):

- Financeiro: `receber.rs`, `financeiro.rs`, `pagar.rs`, `cheques.rs`, `comissoes.rs`, `dre.rs`.
- Fiscal/assinatura/XML: `fiscal.rs`, `nfe_xml.rs`, `nfe_assinatura/` e respectivos testes.
- Sincronização/rede: `sync.rs`, `rede_local.rs`, `computador.rs`, `portatil.rs`, `teste_rede.rs`.
- Banco/backup/migração/arquivos: `banco.rs`, `backup.rs`, `migracao.rs`, `arquivos.rs`, `caminhos.rs`.
- Segurança/usuários/senhas/supervisão: `usuarios.rs`, `senhas.rs`, `supervisao.rs`, `registro.rs`, `auditoria.rs`.
- Outros domínios e testes: `encomendas.rs`, `reposicao.rs`, `pessoas.rs`, `oficina.rs`, `etiquetas.rs`, `gtin.rs`, `codigo.rs`, `documento.rs`, `numeros.rs`, `texto.rs`, `faixas.rs`, `painel.rs`, `loja.rs`, `config.rs`, `erro.rs`, `impressao.rs` e módulos de teste.

Essa lista não é um inventário definitivo. Refaça o inventário de **todos** os arquivos, incluindo Rust, QML, SQL, C++, Python, scripts, manifests, recursos, testes e build/CI. Evite limitar o escopo a `nucleo/src` ou apenas Rust/QML.

---

## 6. Método de trabalho sugerido para o novo ciclo

### Fase A — recomeço e revalidação pelo Gemini 4 Argon

1. Reabrir o projeto original 0.9.17 e identificar se é fonte sem alterações ou uma versão modificada.
2. Refazer o Bloco 0: integridade, hashes, inventário, ambiente e fontes disponíveis.
3. Refazer os blocos documentais e requisitos com acesso real aos arquivos disponíveis; marcar claramente qualquer arquivo ausente/inacessível.
4. Auditar todo o código, sem considerar os 45 arquivos anteriores como “dispensados”.
5. Ao chegar a cada arquivo previamente auditado, usar a seção 3 e os relatórios originais como checklist, mas executar nova inspeção independente.
6. Corrigir as linhas de referência para a versão atual e produzir uma matriz de rastreabilidade única.
7. Terminar com inventário de cobertura por arquivo e por linhas, separando produção, testes e documentação.

### Fase B — correções

Somente após a conclusão e aprovação humana da auditoria, transformar os achados confirmados em plano de correção. Cada correção deve ser pequena, rastreável, possuir teste de regressão e não misturar mudança de comportamento com refatoração desnecessária.

### Fase C — segunda auditoria independente no VS Code / Claude Code / Reversa

Depois de as correções serem aplicadas e congeladas em um commit/hash identificado:

1. Claude Opus 5.5, esforço **Extra**, fará uma segunda auditoria independente do código já corrigido.
2. Não fornecer apenas o relatório do Gemini: fornecer também o código atual e os testes/saídas de execução.
3. Pedir revisão de cada correção e busca de defeitos novos/regressões, não apenas validação das correções propostas.
4. Registrar cada discordância entre Gemini e Opus, com evidência de código e teste.
5. Não permitir que nenhum agente aplique alterações durante essa segunda auditoria sem autorização do usuário.

---

## 7. Configuração observada nas capturas de tela (02/11/2026)

- VS Code está aberto, mas a captura mostra **“No Folder Opened”**: nenhum projeto Rustinho foi aberto naquele momento.
- Há uma aba/painel **Claude Code** visível no VS Code. Isso confirma a presença visual da extensão/painel, mas não comprova autenticação, acesso à conta nem configuração correta do modelo.
- No campo inferior do Claude Code na captura do VS Code aparece **“Fable 5.1 Low”**. Em outra captura do seletor de modelo aparece **“Opus 5.5”** com esforço **“Extra”** selecionado. Como as capturas não provam que sejam o mesmo seletor/contexto, a escolha efetiva dentro da extensão Claude Code precisa ser conferida no próprio VS Code.
- Na captura do Gemini aparece **“4 Argon”** selecionado e nível de raciocínio **“Alto”**.
- As capturas não mostram configuração concluída de Gemini Code Assist ou Reversa no VS Code. Não presumir que estejam instalados ou autenticados.

---

## 8. Regras de comunicação com o usuário

O usuário não é programador e solicitou orientação em português, simples, curta e sequencial. Portanto:

- apresentar **uma etapa por vez**, com poucos passos;
- preferir listas numeradas e comandos prontos para copiar;
- explicar onde clicar e o que deve aparecer na tela;
- não pedir que o usuário edite JSON/configurações manualmente sem necessidade;
- não presumir instalação, login, pasta extraída, permissões, assinatura, plano ou acesso a modelos;
- quando um detalhe essencial faltar, fazer apenas uma pergunta objetiva;
- aguardar o resultado/captura do passo antes de apresentar o próximo;
- nunca afirmar que agente, extensão, Reversa, conta, API ou projeto está configurado sem evidência visual ou teste correspondente.

---

## 9. Anexos originais

A pasta `relatorios_originais/` contém os textos das respostas de auditoria e retificações que foram colados pelo usuário ao longo dos Blocos 0 a 9B. Eles foram copiados sem reescrita para preservar a evidência histórica e permitir que Gemini/Claude comparem a síntese com os relatórios recebidos.

**Observação:** alguns arquivos são retificações de relatórios anteriores (principalmente correção de contagem/linhas do `api.rs`, `busca.rs`, `produtos.rs`, `unidades.rs` e arquivos QML). A retificação mais recente deve prevalecer quando houver divergência, mas o agente deve sempre verificar no código atual.

---

## 10. Ordem imediata de execução

1. Não instalar extensões ainda.
2. Primeiro abrir no VS Code uma **pasta extraída** do projeto Rustinho 0.9.17 (não apenas o arquivo ZIP).
3. Confirmar que o Explorer mostra a árvore do projeto e que a raiz contém os arquivos esperados (`Cargo.toml`, pastas `nucleo`, `app`, `android` etc., conforme a árvore real).
4. Só então configurar/verificar Gemini, Claude Code e Reversa, um de cada vez.
5. Antes de rodar qualquer agente, confirmar que o workspace aponta para a cópia de trabalho correta e que o Git/backup permite recuperar o estado original.

**Fim do handoff.**
