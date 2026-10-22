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

## Pendência: tela de unidades de medida (22/10/2026)
Critério objetivo, sem lista fixa de unidades populares:
1. Para cada unidade, contar quantos produtos cadastrados a usam.
2. Unidades com contagem maior aparecem primeiro, em ordem decrescente.
3. Em caso de empate, ordenar pela sigla.
4. Unidades sem uso ficam depois, em ordem alfabética pela sigla.
- Não implementar agora: a alteração fica como pendência; nenhum código muda nesta etapa. A tela de cadastro de produtos vem depois dos testes de vendas, caixa e permissões.

## Testes reais necessários (feitos pelo Renan)
- Assinar e transmitir em homologação com o e-CNPJ A3 (cartão com leitor USB), sem o Rustinho guardar PIN nem chave privada.
- Conversa com o contador sobre regime, CSOSN, CClasTrib e CSC.
