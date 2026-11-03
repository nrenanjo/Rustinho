# Avaliação fiscal comparativa — esqueleto (a preencher)

Regras: só documento; não modificar o código principal; não implementar; nada vai para o GitHub sem autorização. Tudo que não for comprovado fica **"não verificado"**. Nenhuma opção é declarada pronta para produção sem teste real em homologação. Pendência: pedir acesso de leitura (add_repo) aos repositórios do GitHub.

## Projetos avaliados
1. fiscal-rs — https://github.com/JoaoHenriqueBarbosa/fiscal-rs (Rust)
2. ACBrLib — https://acbr.sourceforge.io/ACBrLib/BemVindo.html (Pascal; bibliotecas compartilhadas)
3. sped-nfe — https://github.com/nfephp-org/sped-nfe (PHP)
4. @brasil-fiscal/nfe — https://github.com/brasil-fiscal/nfe (TypeScript)

Referências complementares (só se sobrar tempo): FinOpenPOS, F2U ERP.

## Pontos já levantados (a confirmar)
- fiscal-core 0.7.1 (docs.rs): monta XML, calcula impostos, QR Code, contingência; a página não cita assinatura, certificado nem SEFAZ. Primeira versão em 03/2026. Licença não confirmada.
- dfe (crate Rust): MIT, parado desde 01/2025, usa OpenSSL, "não usar em produção".
- sped-nfe: LGPL-3, GPL-3 ou MIT (Packagist); v5.2.8 de 27/07/2026.
- ACBr: LGPL (site do projeto); ACBrLib = DLL/.so chamável de outras linguagens.
- Texto de outra IA diz que o fiscal-rs tem ciclo completo, 1.834 testes, 263 commits, 13 estrelas: **não verificado**.

## Tabela comparativa (preencher com evidência: arquivo/linha/link)

| Critério | fiscal-rs | ACBrLib | sped-nfe | @brasil-fiscal/nfe |
|---|---|---|---|---|
| Licença do código | | | | |
| Licença das dependências | | | | |
| Última versão / último commit | | | | |
| Testes (existem? rodam? em quais sistemas) | | | | |
| Documentação | | | | |
| NF-e modelo 55 | | | | |
| NFC-e modelo 65 (QR Code, CSC) | | | | |
| Regras tributárias por empresa / produto / operação | | | | |
| Simples Nacional (CSOSN 102 etc.) | | | | |
| Assinatura A1 | | | | |
| Certificado A3 (cartão/token) sem guardar PIN nem chave | | | | |
| Transmissão e autorização | | | | |
| Consulta de protocolo | | | | |
| Cancelamento | | | | |
| Inutilização | | | | |
| Contingência (offline NFC-e, SVC) | | | | |
| Esquemas vigentes (NT atuais) | | | | |
| IBS/CBS (reforma tributária) | | | | |
| CNPJ alfanumérico | | | | |
| Integração com Rust | | | | |
| Integração com SQLite / armazenamento de XML | | | | |
| Windows 10/11 | | | | |
| Linux | | | | |
| Empacotamento (dependências extras) | | | | |
| Riscos de licença para um programa MIT | | | | |
| Limitações encontradas | | | | |

Legenda para cada célula: **Sim (evidência)** / **Não** / **Não verificado**.

## Perguntas a responder em cada projeto
1. A licença do repositório e a de cada dependência permitem uso num programa MIT? O que cada uma exige (aviso, código-fonte, ligação dinâmica)?
2. Há assinatura e transmissão reais, ou só montagem de XML?
3. Como o A3 é usado (driver do sistema, PKCS#11, repositório de certificados)? Quem pede o PIN?
4. Quais esquemas e notas técnicas estão incluídos, e em que data?
5. As regras tributárias vêm de configuração (por empresa, produto, operação) ou estão fixas no código?
6. Como os testes comprovam o funcionamento? Alguma coisa foi testada em homologação?
7. O que o Rustinho ainda teria que construir por conta própria (configuração fiscal por empresa, vínculo com a venda, guarda dos XMLs, fluxo)?

## Interface do motor fiscal (simples)
Só um conjunto pequeno de operações atrás de uma interface do Rustinho: montar nota, assinar, transmitir, consultar, cancelar, inutilizar. Sem infraestrutura complexa antes de escolher a biblioteca.

## Licenças: obrigações registradas
(preencher por projeto e dependência; revisão jurídica antes de distribuir)

## Recomendação
(preencher no fim, com evidências e limitações; indicar o que depende de teste real com o cartão A3 e o leitor da loja)

## Pergunta em aberto (22/10/2026)
- A loja já usa o ACBr no computador? Na tela do Windows apareceu "O programa ACBrMonitor já está em execução", o que indica que o ACBrMonitor roda em segundo plano para o Sintec.
- Confirmar: (a) ícone do ACBrMonitor na bandeja do Windows; (b) arquivos de configuração do ACBr na pasta do Sintec; (c) com o Rogério ou o contador, como a emissão é feita hoje.
- Se confirmado: a loja já tem ACBr, certificados e configurações prontos, o que favorece a opção ACBrLib; e indica que o A3 já é lido nesse computador (a verificar).

## Confirmação do ACBr na loja (22/10/2026)
- Bandeja do Windows: ACBrMonitorPLUS 1.4.0.207 (x86), "Monitorando Arq.: ENT.TXT". O Sintec usa o ACBr por troca de arquivos (ENT.TXT para entrada; saída provavelmente SAI.TXT — a confirmar).
- Consequências: a ACBrLib (DLL) seria forma diferente de integração; a versão x86 sugere 32 bits no Sintec — decidir 32 ou 64 bits se a ACBrLib for escolhida.
- A confirmar com Rogério ou contador: versão do ACBr, configuração do certificado (A3), pasta dos arquivos ENT.TXT e SAI.TXT.

## Sintec na loja (tela "Sobre", 22/10/2026; dados de clientes não foram copiados)
- Sintec NFe 4 v4.26.10-02, executável x32 (32 bits), Windows 11 64 bits.
- Versão válida até 15/12/2026: prazo da assinatura atual; a migração precisa estar pronta e testada antes disso, ou a assinatura precisa ser renovada.
- A loja emite NF-e modelo 55 em produção (com venda a prazo). O fiscal é obrigatório, não opcional.
- Consequência técnica: se a ACBrLib for escolhida, o build de 32 bits pode ser necessário se for carregada no mesmo processo do Sintec; o Rustinho é 64 bits, então a integração por arquivo (como o Sintec faz) costuma ser mais simples. A verificar.

## Evidências do ambiente da loja e ACBrMonitor × ACBrLib (22/10/2026)
Nenhum dado fiscal real (CNPJ, clientes, documentos, senhas) foi copiado para o repositório.

### Fatos observados (por capturas de tela do Renan)
1. Bandeja do Windows: **ACBrMonitorPLUS 1.4.0.207 – x86**, dica "Monitorando Arq.: ENT.TXT".
2. Ao tentar abrir outra cópia: "O programa ACBrMonitor já está em execução" (só uma instância por vez).
3. Sintec NFe 4 v4.26.10-02, **x32 (32 bits)**, Windows 11 de 64 bits.
4. Tela "Controle de Notas Fiscais – Ambiente de Produção", modelo **55 – NF-e**, com botão "Emitir NF-e F6".
5. "Versão válida até **15/12/2026**".

### Hipóteses (não verificadas)
- H1. O Sintec usa esse ACBrMonitor para emitir. Só há coincidência de computador e de arquivo monitorado; **não foi comprovado** que o Sintec escreve no ENT.TXT dele.
- H2. A resposta volta por arquivo de saída (SAI.TXT ou arquivo de resposta). **Não verificado**: as páginas consultadas do fórum do ACBr só mostram o ENT.TXT em formato INI (por exemplo `NFE.CriarNFe`); não há descrição do arquivo de resposta.
- H3. O A3 funciona porque o ACBrMonitor está ativo. **Não presumir.** Estar em execução não prova que o certificado assina neste computador.
- Como confirmar H1/H2: **não emitir nota só para investigar.** Só com um teste de homologação seguro, combinado antes (quem faz, em qual computador, com qual certificado, sem afetar a numeração de produção). Observar arquivos e horários dá indício, mas não prova qual programa os gerou. Perguntar ao Rogério ou ao contador como a emissão é feita hoje é a forma sem risco.

### Comparação das duas alternativas
| Critério | Integração por arquivos (ACBrMonitor) | ACBrLib (biblioteca) |
|---|---|---|
| Como o Rustinho fala | Escreve comandos em arquivo INI e lê a resposta | Chama funções da DLL/.so |
| Já presente na loja | Sim (observado), mas só em 32 bits | Não observado |
| Arquitetura | Processo separado de 32 bits: independe de o Rustinho ser 64 bits | Precisa do build igual ao do processo (64 bits para o Rustinho); há builds de 32 e 64 bits (site do ACBr) |
| Conflito de arquivos | Alto risco: o Sintec e o Rustinho disputariam o mesmo ENT.TXT se usassem a mesma pasta ou o mesmo monitor | Sem arquivo compartilhado; instância própria por processo |
| Concorrência (5 caixas) | Um monitor por computador consumindo um arquivo; duas notas seguidas podem se sobrescrever sem fila própria. **A confirmar**: como o monitor trata um novo arquivo enquanto processa o anterior | Chamadas dentro do processo; o Rustinho controla a fila |
| Identificação da resposta | Depende de o arquivo de resposta trazer o identificador do pedido. **Não verificado** | Retorno direto da chamada, associado ao pedido |
| Linux | O ACBrMonitor PLUS tem versão para Linux (**não verificado** nesta pesquisa) | Há .so para Linux (site do ACBr) |
| Manutenção | Atualizar o monitor separado em cada computador; mistura com o Sintec durante a transição | Biblioteca distribuída junto com o Rustinho; atualização com as versões dele |
| Licença | LGPL | LGPL (ligação dinâmica em programa MIT; revisão jurídica antes de distribuir) |
| Certificado A3 | Mesmo problema da outra opção | Mesmo problema da outra opção |

Resumo provisório: a ACBrLib parece mais adequada ao Rustinho (sem disputa de arquivos, retorno direto, controle da fila). A integração por arquivos só se justifica se a ACBrLib não conseguir usar o A3 da loja. Isto é uma inclinação, **não uma decisão**.

### Certificado A3, leitor, driver, Windows e Linux
- O que se sabe (fórum do ACBr, tópico de 2017 sobre guardar a senha do A3): o A3 costuma exigir o PIN por segurança do token ou do leitor; a tela do próprio token pede a senha; guardar o PIN no sistema é um risco. O Rustinho **não guardará PIN nem chave privada**.
- Não verificado: qual biblioteca criptográfica a ACBrLib usa para o A3 em cada sistema, se aceita PKCS#11 no Linux, se o driver do cartão da loja é compatível, se o leitor USB funciona com o Rustinho num computador específico.
- Teste real obrigatório (Renan, com o cartão e o leitor, em homologação): assinar e transmitir uma nota de teste no computador onde o cartão fica ligado; anotar o modelo do leitor, o fabricante do cartão e do driver.
- Windows 10/11: o certificado costuma ser lido pelo repositório do Windows (a verificar). Linux: requer driver PKCS#11 do fabricante (a verificar).

### Riscos a registrar
- Mexer na pasta do ACBrMonitor da loja pode atrapalhar o Sintec em produção; qualquer teste com ele deve ser feito com cópia ou em horário combinado.
- Substituir o Sintec exige tratar a transição: mesmo certificado, mesma numeração de série de notas (não repetir número), e nenhum cancelamento ou inutilização duplicado.

## Decisão provisória (22/10/2026)
- A ACBrLib é a **primeira candidata**, não a escolha definitiva. Depende dos testes de A3, Windows e Linux.
- O A3 continua sendo o principal ponto de incerteza; documentação não substitui o teste com o cartão e o leitor reais.
- Prioridade: terminar antes os testes de vendas, recebimentos, parcelas, troco e permissões já combinados. O código principal do Rustinho não muda para o fiscal agora.
- O protótipo fiscal, se vier, fica separado do Rustinho principal até validar a solução.
- Ainda **incompleto**: a tabela comparativa dos 4 projetos (fiscal-rs, ACBrLib, sped-nfe, @brasil-fiscal/nfe) está vazia e depende da leitura dos repositórios (etapa do Opus).

## Pendência: tela de unidades de medida — especificação alinhada (22/10/2026)
Não implementar agora; nenhum código muda nesta etapa. Vem depois dos testes de vendas, caixa e permissões.

**Linhas** (desktop: 5 unidades por linha; celular: sugestão de 3 por linha — mesma lógica, número diferente por tela):
1. **Primeira linha:** unidades ordenadas pelo **número de produtos cadastrados** que as usam (maior → menor, mais à esquerda). Empate: sigla A→Z.
2. **Segunda linha:** unidades ordenadas pelas **vendas das últimas 24 horas**, em itens (maior → menor, mais à esquerda). Empate: sigla A→Z. Só as 5 que mais vendem ficam; a que vendeu menos e não cabe vai para as linhas de baixo.
3. **Demais linhas:** as restantes em ordem alfabética pela sigla (ex.: BD, CX, KG, LT, UN).

**Janela das vendas:** as 24 horas anteriores ao momento em que a pílula "Peças" é aberta (retrato no instante da abertura; o relógio anda, a tela é calculada ao abrir).

**Contagem das vendas (itens vendidos):**
- Soma dos itens vendidos de produtos de cada unidade.
- Devolução subtrai a quantidade devolvida (e o estoque volta a aumentar). Cancelamento de venda subtrai os itens daquela venda (10 vendidos, 1 cancelado → 9). Só se descontam devoluções e cancelamentos de vendas feitas dentro da janela, para a conta nunca ficar abaixo de zero.
- Estorno de recebimento **não** entra (não mexe em itens vendidos).
- Resultado zero = "sem venda".
- Ficam fora da contagem: produtos inativos (peça desativada, ainda cadastrada) e serviços (unidade SV). Só conta o que está cadastrado no sistema.

**Segunda linha sem vendas (ou com poucas):**
- Enquanto não há vendas, a segunda linha é a continuação da primeira **pelo número de produtos cadastrados**: a 1ª linha tem as 5 unidades com mais produtos (1º ao 5º lugar) e a 2ª linha tem as do 6º ao 10º lugar, em ordem decrescente, sem repetir nenhuma unidade. Ex.: 1ª linha a,b,c,d,e; 2ª linha f,g,h,i,j (da mais usada à menos usada).
- Conforme as unidades vendem, elas entram na segunda linha pela direita, em ordem de volume (maior à esquerda do grupo vendido), substituindo f..j (6º ao 10º por produtos) da direita para a esquerda; as unidades que saem da segunda linha voltam para as linhas de baixo (ordem alfabética pela sigla): 1..9,a → 1..8,a,b → 1..7,a,b,c → ... até 1ª linha 1..5 e 2ª linha a,b,c,d,e (todas com vendas).

**Duplicidade:** uma unidade aparece só uma vez na tela. Se ela está nas duas listas (por cadastro e por vendas), **prevalece a linha das vendas**: vai para a segunda linha e a primeira pega a próxima por número de produtos.
- Tentativa de criar uma sigla repetida: mensagem de aviso no padrão da interface do Rustinho.

## REGRA DEFINITIVA — unidades de medida (23/10/2026, opção A confirmada pelo Renan)
Prevalece sobre qualquer redação anterior. Não há fila por ordem de chegada.

**Primeira linha:** as 5 unidades com mais produtos **ativos** cadastrados (serviços sem estoque e inativos fora). Empate: sigla A→Z.

**Segunda linha:** as 5 unidades com maior **quantidade líquida de itens vendidos** nas últimas 24 horas, em ordem decrescente de quantidade; empate: sigla A→Z.
- Sem vendas: começa com as unidades do 6º ao 10º lugar do cadastro.
- Preenchimento da direita para a esquerda conforme vendem:
  - 1 unidade vendida: ocupa a posição da extrema direita (substitui o 10º).
  - 2 unidades: ocupam as 2 posições da direita, da maior para a menor quantidade.
  - 3 unidades: as 3 da direita, mesmo critério. E assim por diante até 5.
- A ordem é sempre pela **quantidade**, não pela ordem de chegada.
- Com mais de 5 unidades vendendo: ficam as 5 mais vendidas; as demais vão para as linhas inferiores (alfabético pela sigla).

**Prevalência de vendas (sem duplicar):** se uma unidade está entre as 5 mais cadastradas e também entre as 5 mais vendidas, ela aparece na **segunda** linha, e a primeira recebe a próxima unidade elegível do ranking de cadastro. Se está só entre as mais cadastradas (não entre as mais vendidas), permanece na primeira.
- Nenhuma sigla aparece duas vezes.

**Janela e recálculo:** 24 horas anteriores ao instante em que a pílula "Peças" é aberta; recalcula a cada abertura.

**Vendas:** quantidade líquida (vendido − devolvido − cancelado, só dentro da janela). Estorno financeiro não conta. Zero = sem venda.

**Testes obrigatórios (opção A):**
- Unidade que vendeu mais ultrapassa outra que começou a vender antes.
- Com mais de 5 vendendo, sai a de menor venda, mesmo que tenha entrado antes.
- Empate na quantidade → ordem alfabética.
- Unidade da primeira linha que vende vai para a segunda e a primeira é reposta pelo cadastro, sem duplicar.
- Nenhuma fila por ordem de chegada.

**Pendente:** implementação só após "começar" do Renan, na etapa do Opus (terça). Pesquisa fiscal: mesma etapa, com fontes oficiais comparadas às enviadas; SAT, MFE e outros modelos estaduais diferenciados, sem descartar outras UFs.

## Princípio central (23/10/2026, Renan)
- O Rustinho deve exigir o **mínimo de esforço** para executar uma operação corretamente. O que puder ser preenchido, relacionado, validado ou calculado com segurança fica a cargo do sistema.
- Mostrar primeiro o essencial; detalhes só quando necessários; sem perguntas repetitivas.
- Simplificar nunca compromete segurança, integridade dos dados ou conformidade fiscal. Validação, registro, permissões e rastreabilidade funcionam nos bastidores e interrompem o usuário só quando há motivo real.
- O mesmo padrão vale para tela inicial, pílulas, abas, mensagens e diálogos, no desktop e no Android.
- O Sintec é referência de funções e cenários, não modelo visual nem operacional.

## Evidências da emissão no Sintec (telas de 23/10/2026; sem dados de clientes)
- NF-e modelo 55, série 1, em Ambiente de Produção, com status **APROVADA** e protocolo de uso preenchido (emissão normal).
- Campos automáticos vistos: CFOP 5102 (natureza VENDA), tipo de operação interna, presença do consumidor "0 – Não se aplica", indicador de IE "Contribuinte", forma de pagamento "A PRAZO (30 DIAS)" com vencimento gerado.
- Abas da nota: dados da nota; transporte; dados adicionais (vencimentos, pedido, OS, marketplace); impostos/observação (valores aproximados IBPT federal, estadual e municipal; campos EC 87/2015, FCP, DIFAL, Suframa, ISSQN, poder público, PIS/COFINS diferenciado); dados de autorização.
- Implicação: a loja já emite NF-e autorizada no ambiente de produção pelo Sintec. Qualquer migração precisa manter a numeração, a série e os protocolos sem sobreposição.
- Nenhum dado de cliente, CNPJ, endereço ou protocolo real foi copiado para o repositório.

## Critérios testáveis de praticidade (23/10/2026, versão ajustada pelo Renan)
1. **Automático com transparência:** o que o sistema preencheu é identificável, sobretudo em fiscal e financeiro. O valor pode ser corrigido quando a legislação e a operação permitirem; se não puder, a tela explica o motivo. Sem confirmação para cada cálculo: mostrar o resultado, permitir conferir os detalhes e pedir intervenção só quando necessário.
2. **Desfazer em vez de perguntar:** para ações reversíveis (remover item antes de concluir), oferecer desfazer. Ações com efeito legal, financeiro ou irreversível têm proteção apropriada. Se o usuário já clicou explicitamente em "Emitir nota", não acrescentar outra pergunta por hábito; confirmação adicional só com risco ou ambiguidade concreta, ou quando exigida pelo fluxo fiscal.
3. **Resposta imediata:** a interface reage ao clique, mostra o estado da operação e impede acionamento duplicado enquanto a primeira execução está em andamento. A proteção existe na interface e no núcleo (bloquear o botão não basta). Não travar a tela inteira durante o processamento. Informar se cancelar é seguro; se não for, o estado permanece claro.
4. **Medir a praticidade:** começar pelas tarefas mais frequentes da loja. Para cada uma registrar número de ações, tempo até concluir, erros e interrupções. Só então definir metas realistas. Não fixar números arbitrários antes de medir. Etapa extra só se for exigência fiscal ou decisão comercial indispensável; pergunta repetitiva não justifica.
5. **Preservar contexto:** em erro, configuração pendente ou necessidade de outra pílula, preservar os dados preenchidos, levar ao lugar certo e permitir retomar sem recomeçar.
6. **Toda melhoria de praticidade é testada** para verificar que não prejudicou integridade dos dados, segurança ou conformidade fiscal.

## PRIORIDADE DE SEGURANÇA — papel de gerente na entrada em loja existente (23/10/2026)
Achado: na VM, o primeiro usuário criado localmente (Operador-virt) virou gerente automaticamente. A sincronização com o computador principal o tirou do papel, como esperado. O problema está na janela entre criar o banco local e entrar na loja: enquanto isso, o gerente local pode executar ações de gerente.

Regra definitiva (acordada entre Renan e Mário):
1. **Instalação independente:** o primeiro usuário vira gerente automaticamente, para o Rustinho funcionar sozinho.
2. **Entrada em loja existente:** ao escolher juntar o computador a uma loja que já existe, a **primeira ação** é remover o privilégio de gerente criado localmente.
3. **Depois:** sincronizar o banco e as permissões da loja. Operador-virt passa a operador comum; Renan mantém o gerente conforme o cadastro autorizado do computador principal.
4. **Novos gerentes:** só são autorizados por um gerente existente.
5. **Falha na sincronização:** permitir tentar de novo **sem devolver** o privilégio de gerente local.

Pontos a definir antes de implementar:
- Como a escolha "juntar a uma loja existente" aparece na primeira configuração (tela e texto).
- O que acontece com ações de gerente feitas localmente antes de juntar: conferir se ficam na auditoria e se entram na sincronização (risco de a loja herdar ações não autorizadas).
- Teste de regressão obrigatório: um computador novo com um usuário não pode executar ações de gerente antes de juntar-se à loja, e não pode manter o privilégio depois de juntar-se.

## Testes reais necessários (feitos pelo Renan)
- Assinar e transmitir em homologação com o e-CNPJ A3 (cartão com leitor USB), sem o Rustinho guardar PIN nem chave privada.
- Conversa com o contador sobre regime, CSOSN, CClasTrib e CSC.
