# Rustinho 0.9.18-dev.3 — validação de busca e importação XML

Data: 2026-11-09

## Implementação confirmada

- A busca aceita código interno, GTIN/código de barras, código original, código de fornecedor, equivalentes, descrição, marca, aplicação, local e demais campos indexados.
- Resultados informam o identificador que produziu a correspondência; o código interno é identificado como `Cód. interno`.
- A paginação usa deslocamento, contagem total e desempate estável por ID.
- Desktop e Android usam os mesmos espelhos de busca e apresentação de peças.
- O parser separa `cEAN`, `cEANTrib`, `cBarra` e `cBarraTrib`; a migração 0024 permite múltiplos GTINs sem substituir registros.
- `cProd` é gravado como código do fornecedor associado ao emitente; vínculos anteriores permanecem e conflitos viram alternativas/pendências.
- Pendências permanecem no alerta crítico até decisão explícita na tela de compra.

## Testes

- XML real em `/home/renan/Downloads/xml's.zip`: 558 entradas XML, 549 NF-e lidas, 5.193 itens, 2.212 cEAN, 2.212 cEANTrib e 2 itens com cBarra/cBarraTrib; teste reproduzível em `dev3-xmls-reais.log`.
- Busca serial: 4 testes aprovados, incluindo código interno, GTIN, OEM, fornecedor e paginação sem repetição.
- Importação/identificadores: teste de quatro identificadores, múltiplos fornecedores, reimportação idempotente, preservação de associação e ambiguidade aprovado.
- Suíte workspace serial: exit 0; núcleo 433 aprovados e 5 ignorados; integrações caixas 9, contratos 4, contratos de telas 8, estavel 57 aprovados e 4 ignorados, fluxo 5.
- A rodada paralela anterior permanece registrada com 413 aprovados, 17 falhas de rede/sincronização e 5 ignorados. Os 17 foram repetidos serialmente e passaram.

## Limitações

Validação visual completa de QML e Android físico continuam pendentes. Os pacotes Windows/Linux ainda precisam ser montados e testados pelos runners correspondentes.
