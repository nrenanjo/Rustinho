# Auditoria da integração fiscal (etapa C8, 28/10/2026)

**Conclusão:** o Rustinho **não emite** nota fiscal (NF-e 55 nem NFC-e 65) e **não consulta** a SEFAZ. O adaptador fiscal (`rustinho-fiscal`) **não está neste repositório** e não está entre os repositórios acessíveis a esta sessão: não foi auditado. As tarefas 16 a 18 do briefing (fechar contrato, implementar emissão e consulta, homologar) **não foram feitas**: dependem do adaptador e das decisões DF-1 a DF-6. Nenhuma operação fiscal real foi feita (proibido pelo briefing). **Bloqueador da liberação comercial** (briefing §7 e §14).

## 1. O que existe no núcleo (lido nesta etapa)

| Parte | Arquivo | O que faz | O que **não** faz |
|---|---|---|---|
| Regras tributárias da loja | `nucleo/src/fiscal.rs` (728 linhas) | CSOSN, CST de IPI/PIS, exigibilidade do ISS, regras de ST por UF/CEST com vigência, conferência das peças sem dado fiscal, "usar automático" (com histórico, numa transação; **qualquer usuário** pode usar — C8-07) | não monta XML de saída, não calcula a nota inteira, não assina, não transmite |
| Leitura do XML de compra | `nucleo/src/nfe_xml.rs` (607) | lê NF-e de fornecedor (itens, duplicatas, chave e dígito, CNPJ alfanumérico) com limites de tamanho e profundidade | não lê `protNFe` como prova de autorização |
| Assinatura do XML de compra (PEND-09) | `nucleo/src/nfe_assinatura/` (mod, c14n, ancoras: 1.154 linhas + 593 de testes) | C14N 1.0 inclusiva, RSA SHA-1/SHA-256, cadeia até as raízes ICP-Brasil escolhidas, validade em `dhEmi` e `dhRecbto`, titular, recusa homologação, contra "wrapping" | **não prova que a SEFAZ autorizou** (o `protNFe` não é assinado: PEND-15); não consulta revogação (LCR) |
| Tela Fiscal | `app/qml/TelaFiscal.qml`, `app/qml/EmissaoFiscal.qml` | "Ver opções de integração" só avisa que está em planejamento | nenhum emissor ligado |

Busca no código por `sefaz`, `webservice`, `emitir`: só aparecem nos textos e na recusa de nota de homologação de **compra**. Não há chamada de rede fiscal nenhuma.

Os testes do núcleo fiscal (PEND-09, PEND-19) fazem parte da suíte completa desta etapa e passaram (log `logs/10-suite-final.log`). Isso comprova só a **entrada** (compra por XML), não a emissão.

## 2. O adaptador (`rustinho-fiscal`)

- Não encontrado no repositório; `list_repos` desta sessão não mostra nenhum repositório com "fiscal" (28/10/2026).
- Contrato de integração: **v0.1, sujeito a revisão** (documento à parte, entregue ao Sunda; não está neste repositório). Não foi possível conferir dados, estados, erros e responsabilidades contra código real.
- Regra do briefing mantida: não integrar uma versão do adaptador só porque compila; exemplos e respostas simuladas não contam como integração.

## 3. O que falta para a integração (cada item precisa de teste de contrato e de falha)

| Requisito (briefing §7) | Situação |
|---|---|
| Emissão NF-e 55 e NFC-e 65 | não existe |
| Assinatura A1/A3 sem guardar PIN nem chave em claro | não existe para saída; DF-2 (qual A3, leitor, middleware) e DF-4 (estação de assinatura) abertas; porta de credencial PEND-16 aberta |
| Consulta de autorização e situação | não existe (PEND-15) |
| Indisponibilidade da SEFAZ, contingência (SVC, offline NFC-e) | não existe |
| Numeração e séries por caixa sem conflito com o Sintec | DF-6 aberta |
| Rejeição, cancelamento, inutilização, recuperação | não existe |
| Guarda e integridade dos XML autorizados e protocolos | não existe para saída |
| Regras estaduais (SP), CSC da NFC-e | DF-5 aberta |
| Regime e CRT | DF-1 aberta (CSOSN 102 informado pelo dono; contador confirma) |
| Provedor (emissão própria em Rust, ACBrLib, ACBrMonitorPLUS) | DF-3 aberta; ACBrLib não tem Android |

## 4. O prazo do Sintec

`docs/AVALIACAO-FISCAL-esqueleto.md` registra o Sintec NFe 4 (v4.26.10-02, 32 bits) com **"versão válida até 15/12/2026"**, emitindo NF-e modelo 55 em produção, e o ACBrMonitorPLUS 1.4.0.207 (x86) lendo `ENT.TXT`. A data vem da documentação do projeto (tela do programa da loja); **não foi confirmada com o fornecedor nesta etapa**. A partir de hoje faltam **48 dias**. Não dá para implementar, homologar e substituir a emissão com segurança nesse prazo partindo do zero: o plano (documento B) recomenda **renovar o Sintec** como contingência e não usar o Rustinho para emitir até a homologação terminar.

## 5. Critério de conclusão (antes de dizer "fiscal pronto")
1. DF-1 a DF-6 decididas por escrito (dono + contador).
2. Contrato adaptador ↔ núcleo revisado (v1.0), com estados, erros e retomada.
3. Testes de contrato e de falha (SEFAZ fora, resposta truncada, rejeição, duplicidade, queda no meio, certificado vencido/ausente).
4. Homologação oficial da SEFAZ-SP para 55 e 65: autorização, rejeição, cancelamento, inutilização, contingência, consulta pela chave; XML e protocolo guardados e conferidos.
5. Teste com o certificado e o leitor físicos (A3) da loja.
6. Só então autorização expressa do dono para produção.
