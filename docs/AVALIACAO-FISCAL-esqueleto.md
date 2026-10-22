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

## Testes reais necessários (feitos pelo Renan)
- Assinar e transmitir em homologação com o e-CNPJ A3 (cartão com leitor USB), sem o Rustinho guardar PIN nem chave privada.
- Conversa com o contador sobre regime, CSOSN, CClasTrib e CSC.
