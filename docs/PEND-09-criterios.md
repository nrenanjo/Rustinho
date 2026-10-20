# PEND-09 · Validação criptográfica da NF-e (assinatura do emitente) e prova de autorização da SEFAZ

**Status:** **revisão 2 do desenho** (20/10/2026, tarde), **sem código**. Incorpora a sua revisão: biblioteca existente antes de criptografia própria, âncoras de confiança por critério histórico, uso do pacote oficial verificado, e separação entre "o emitente assinou" e "a SEFAZ autorizou". Versão 0.9.11; só commits locais; nada vai ao GitHub.
**Decisões D1 a D6: aprovadas por você em 20/10/2026 (13:37), sem opção de forçar importação.** D7 e D8 (novas, §6) aguardam você.
**Regra de ouro:** *coerência entre campos não é prova de autenticidade, e assinatura válida não é prova de autorização.* Duas provas diferentes, tratadas separadamente (§4 e §5).

## 0. Estado e o que falta

| Item | Situação |
|---|---|
| Pacote oficial de ACs | **Obtido por você, SHA-512 conferido** (os dois arquivos). Relatório em `docs/PEND-09-pacote-acs.md`. |
| Viabilidade medida | Cadeia: 528 de 528 notas reais com cadeia completa validadas por biblioteca madura, na data de emissão (4 de teste recusadas). Assinatura e resumo: 539/539 conferem (protótipo). |
| Falta antes de codificar | Sua resposta a **D7 e D8** (§6) e a aprovação destes critérios finais. |
| Modelo/esforço da implementação | **Opus 5.5, esforço alto**, depois da sua aprovação. |

## 1. O que existe hoje (lido no código)

| Ponto | Hoje |
|---|---|
| `nfe_xml::conferir_coerencia` | Confere chave de 44 dígitos, dígito verificador, campos × chave, `Reference URI`, protocolo e `DigestValue == digVal` (**dois textos copiados um do outro**). Não calcula nada. |
| `nfe_xml::LIMITE_DA_CONFERENCIA` e teste `limite_conhecido_xml_adulterado…` | Aviso fixo na tela e teste que **documenta o buraco**. Ambos mudam (A-20). |
| `compras::importar_xml` | Cria **rascunho** de compra (`situacao = 'rascunho'`), **já grava o fornecedor** (`pessoas::garantir_fornecedor`) e os itens. **Estoque, custo e contas a pagar só mudam em `compras::lancar`.** |
| `importar.rs` (`ler_cadastro`) | Cadastra pessoas e peças a partir de XML (aceita NFC-e). |
| `pagar::duplicatas_da_compra` | Relê o XML guardado (não valida). |
| `tests/nfe_exemplo.xml` | Nota sintética com assinatura de mentira; vários testes dependem dela. |

Consequência para o desenho: a verificação tem de acontecer **antes** de `garantir_fornecedor` (senão o cadastro já é contaminado no rascunho) e a prova de autorização pode ser exigida **na porta única do estoque/financeiro, `lancar`**.

## 2. O que medi nos seus 541 XMLs (protótipo e testes descartáveis, fora do projeto)

| Fato | Resultado | Consequência |
|---|---|---|
| Algoritmos | 539/539: c14n 1.0 inclusivo, transformações `enveloped-signature` + c14n, **SHA-1**, **RSA-SHA1** | Aceitar só essas URIs fixas (+ par SHA-256). |
| Resumo e assinatura recalculados | **539/539 conferem** | Compatível com todos os documentos válidos já usados. |
| Cadeia (biblioteca `rustls-webpki`, âncoras v5+v2 do pacote, data = `dhEmi`) | **528 aceitas**, 4 recusadas (fsist de teste); só v5: 527; âncoras v10/v11: **0 de 532**; data = hoje: muitas recusadas | Biblioteca madura resolve a cadeia; a **data de referência é a de emissão**. |
| Validade | 346 de 539 certificados já venceram hoje; todas as 528 estavam válidas na emissão | Nunca conferir validade "hoje". |
| Titular × emitente | CNPJ idêntico ou mesma raiz de 8 dígitos em todas as NF-e | D3 não recusa nada real. |
| `protNFe` | **Sem assinatura** nos arquivos distribuídos | Assinatura do emitente ≠ autorização (§5). |

## 3. Biblioteca existente × solução própria (item 2 da sua revisão)

### 3.1 Comparação

| Opção | Segurança | Manutenção | SHA-1 (NF-e) | Linux / Windows / Android 32 e 64 | Veredito |
|---|---|---|---|---|---|
| **`xml-sec` 0.1.23** (Rust puro; XMLDSig + C14N 1.0/1.1/exclusivo; `x509-parser` e RustCrypto) | O **próprio README** diz: "pré-lançamento, API instável, cobertura incompleta, **não usar ainda em produção**". Sem auditoria declarada. | 1 mantenedor; primeira versão em 03/2026; 24 versões em 7 meses; exige Rust ≥ 1.92 (temos 1.97) | **RSA-SHA1 e SHA-1 não aparecem na lista de algoritmos** (a confirmar no código) | docs.rs só compila Linux; sem declaração para Windows/Android | **Não adotar como dependência de produção agora.** Usar como **segundo avaliador nos testes** (item 3.3) e reavaliar quando houver versão estável. |
| **libxmlsec1 por FFI** (C; o que muitos sistemas de NF-e usam) | Maduro, mas com histórico próprio de vulnerabilidades; SHA-1 exige habilitar opção "legado" | libxml2 tem manutenção reduzida (verificar situação atual); libxmlsec1 é mantido | Sim, com opção | **Compilar libxml2 + libxmlsec1 + OpenSSL/NSS para Windows e Android é pesado e frágil**; superfície de ataque maior que a necessária (só precisamos de 1 perfil) | Não é descartado por ter C (a base já usa SQLite em C), mas o custo de build multiplataforma e a superfície de ataque superam o ganho para um único perfil fixo. |
| **Verificador estrito próprio** sobre primitivas maduras | **Não reimplementa RSA, SHA nem X.509**: usa `ring` (RSA/SHA-1 legado), `sha1`/`sha2`, `x509-parser`, `rustls-webpki` (cadeia). Próprio só: **(a) C14N do perfil da NF-e** e **(b) conferência estrita da estrutura**. | Código pequeno e fixo (um perfil), testável | `ring` tem `RSA_PKCS1_2048_8192_SHA1_FOR_LEGACY_USE_ONLY` | `ring` e `webpki` compilam para os três alvos (precisa conferir no CI; só Linux testado aqui) | **Recomendado**, com os controles abaixo. |

### 3.2 Por que escrever a C14N à mão é aceitável aqui (justificativa técnica)

1. **Perfil mínimo e fixo:** C14N 1.0 inclusiva, sem comentários, de **um elemento** (`infNFe`) e do `SignedInfo`. Não precisa de XPath, exclusiva, 1.1 nem subconjuntos arbitrários. Qualquer coisa fora desse perfil é **recusada**, não interpretada.
2. **Falha fechada:** um erro de canonicalização muda o resumo calculado e **recusa** uma nota boa; não faz aceitar uma nota ruim. O perigo real seria o contrário (duas leituras do mesmo XML), tratado em 3.3-c.
3. **Oráculo independente:** o libxml2 (via lxml) calcula a C14N dos 539 documentos reais e de casos sintéticos; o teste exige **bytes idênticos**.

### 3.3 Controles adicionais (obrigatórios)

* **a) Avaliador diferencial:** além do libxml2, comparar a C14N e o resultado de verificação com o `xml-sec` (como `dev-dependency`, **nunca** no programa final), se ele tiver os algoritmos; divergência = bloqueia a entrega.
* **b) Testes de propriedade/fuzz** sobre a C14N (documentos gerados com atributos fora de ordem, namespaces repetidos, referências de caractere, CRLF, vazio) e sobre o verificador (XML malformado, profundidade, tamanho).
* **c) Uma só leitura:** os dados da nota (`nfe_xml`) são extraídos **do mesmo nó** (`roxmltree`) que foi canonicalizado e verificado; o verificador devolve esse nó e a leitura **não busca de novo** por nome. Nada de dois analisadores com interpretações diferentes.
* **d) Fora do perfil, recusa:** atributos `xml:*` herdados, `DOCTYPE`, entidades, instruções de processamento dentro da região assinada, comentários, namespaces estranhos.
* **e) Sem criptografia própria:** nenhuma implementação de RSA, hash ou X.509 no projeto.

### 3.4 Cadeia por biblioteca (não própria)

`rustls-webpki` constrói e valida o caminho (assinatura de cada elo, `CA`/`pathLen`, uso de chave, **data de referência** passada por nós, nomes). Medido com os dados reais (§2). Limites a tratar no teste de viabilidade inicial da implementação: (i) a biblioteca **não aceita SHA-1 em certificado** → para as raízes v1 e original, avaliar um **adaptador SHA-1 controlado** (algoritmo extra declarado só para essas raízes e só com `dhEmi` dentro da validade da raiz); se não for seguro, essas cadeias ficam **recusadas com motivo claro**; (ii) certificados de ACs antigos com DER fora do padrão (1 caso, Serpro) ficam fora; (iii) ECDSA P-521/Ed448/pós-quântico: sem suporte inicial.

## 4. Âncoras de confiança (item 3 e 4 da sua revisão)

*Detalhes e fontes em `docs/PEND-09-pacote-acs.md` §3.* Resumo da política:

| Âncora | Uso |
|---|---|
| **v5, v12** (vigentes, RSA-4096/SHA-512) | Permitidas. |
| **v2** (2010–2023, expirada) | **Histórica:** só para notas cuja `dhEmi` esteja dentro da validade da raiz. |
| **v1** (SHA-1) e **raiz original 2001** | v1: só com o adaptador SHA-1 controlado (§3.4-i) e a mesma regra histórica; raiz original: **não incluída**. |
| **v3, v8, v9** | **Revogadas** (ITI): **nunca**, mesmo estando v3 no pacote. |
| **v10** (SSL), **v11** (código) | **Nunca para NF-e** (finalidade). Teste: com só essas âncoras, 0 de 532 validam. |
| **v4, v6, v7, v13** | Reconhecidas, **sem suporte inicial** (algoritmo); a nota recusa com "algoritmo da cadeia não suportado". |

* Origem: **somente** o pacote do ITI com SHA-512 conferido por você; as âncoras ficam embutidas no programa **pelo SHA-256 do certificado inteiro** (nome não vale).
* **Certificado de dentro do XML nunca é âncora e nunca é AC intermediária.** Só o 1º `X509Certificate` é usado, como certificado da empresa emissora.
* Atualização futura do pacote (Gerente › Fiscal): só adiciona **ACs intermediárias** que fecham até âncora já embutida; **raiz nova só entra com nova versão do programa**.
* Revogação: sem internet não se consulta LCR; limite declarado (L2).

## 5. Assinatura do emitente × autorização da SEFAZ (item 5 da sua revisão)

### 5.1 O que cada prova diz

| Prova | O que demonstra | Não demonstra |
|---|---|---|
| **P1, assinatura do emitente** (esta pendência) | Este conteúdo foi assinado pela chave de um certificado ICP-Brasil da empresa emitente, e não mudou depois. | Que a SEFAZ o **autorizou** (uma nota pode ser assinada e nunca enviada, ou enviada e **rejeitada**, **cancelada** ou **denegada**). |
| **P2, autorização da SEFAZ** | A SEFAZ registrou a autorização de uso com um protocolo (`cStat = 100`) para aquela chave, e o resumo registrado é o do conteúdo. | — |

Nos arquivos distribuídos, o `protNFe` (`nProt`, `dhRecbto`, `digVal`) **não vem assinado**: qualquer pessoa pode escrever um protocolo falso. Por isso o programa **não aceita o `protNFe` do XML como prova**.

### 5.2 Como impedir o lançamento sem autorização comprovada

O estoque, o custo e o financeiro só mudam em `compras::lancar`. A regra passa a ser:

> **Nenhuma compra de origem XML pode ser lançada se `autorizacao` não for uma das situações comprovadas.** A porta única é `lancar`; a atualização de preços e o estorno dependem de `lancar`.

Novo campo da compra (migração 0020): `xml_verificacao` (`verificada` | `nao_verificada`) e `autorizacao`:

| `autorizacao` | Significado | Pode lançar? |
|---|---|---|
| `aguardando` | Assinatura do emitente válida, autorização ainda não comprovada (**estado normal logo após a importação**) | **Não** |
| `comprovada_sefaz` | Consulta à SEFAZ pela chave mostrou autorização vigente (cStat 100) e resumo igual (PEND-15, automática) | Sim |
| `comprovada_assinatura_sefaz` | O `protNFe` traz assinatura da SEFAZ e ela passa na mesma cadeia ICP-Brasil (raro; nenhum dos 539 tem) | Sim |
| `confirmada_gerente` | O **gerente** conferiu a autorização no portal oficial e registrou: chave, protocolo, data da conferência; **fica na auditoria**. É **atestado humano, não prova criptográfica**, e aparece rotulado assim | Sim (se D7-B) |
| `negada` | A SEFAZ informou cancelada/denegada/rejeitada | **Não**, sem exceção |
| compra **sem XML** (digitada) | Não há nota eletrônica | Sim (como hoje) |

**Caminhos de comprovação (separados, com o que cada um custa):**
1. **PEND-15 (automático):** consulta à SEFAZ pela chave dentro do programa (web service de consulta, que exige **certificado digital da empresa** e internet) → `comprovada_sefaz`. Exige: importar/guardar o certificado A1 da loja (segredo novo), cliente TLS com certificado, tratamento de ambiente/UF, limites de uso. É uma pendência própria e **não** é feita nesta etapa.
2. **Confirmação do gerente (interina, desde já):** o gerente consulta a chave no portal da NF-e, confere situação "Autorizada" e registra. Rotulada como `confirmada_gerente`. Se você **não** aceitar atestado humano como prova (D7-A), até o PEND-15 **nenhuma nota é lançável**, e a importação por XML continua criando só o rascunho.
3. **Assinatura da SEFAZ no `protNFe`:** aceita se existir e validar.

**Efeito na tela** (sem poluir): o rascunho de compra mostra **uma** linha curta no cabeçalho da nota ("Assinatura do emitente conferida · autorização da SEFAZ ainda não comprovada"), e o botão **Lançar** fica desabilitado com o motivo ao tocar; para o gerente aparece **uma** ação "Registrar conferência no portal…" (diálogo com chave, protocolo, data). Operador não registra. Nada de aviso fixo permanente: o aviso `LIMITE_DA_CONFERENCIA` sai.

## 6. Decisões

| ID | Decisão | Estado |
|---|---|---|
| D1 a D6 | Sem forçar importação; cadastros por XML também verificados; titular do certificado; homologação recusada; compras antigas só marcadas; CPF quando aplicável | **Aprovadas** |
| **D7** | O atestado do gerente (`confirmada_gerente`) **libera** o lançamento enquanto o PEND-15 não existe? **A)** não: nada é lançável por XML até o PEND-15; **B)** sim, com rótulo claro e auditoria | **Recomendo B** para não parar a loja, **mas é decisão sua**: B é prova humana, não criptográfica |
| **D8** | Importação de **cadastros** por XML (pessoas/peças): exige só a **assinatura do emitente (P1)** e **não** a autorização (P2) | **Recomendo sim** (dado de cadastro; sem efeito em estoque/financeiro) |

## 7. Desenho técnico (resumo)

Módulo `nucleo/src/fiscal/` (puro, sem rede, sem banco): `c14n.rs` (perfil da NF-e), `xmldsig.rs` (estrutura estrita e verificação, RSA via `ring`), `cadeia.rs` (invólucro fino sobre `rustls-webpki`), `ancoras.rs` (âncoras por SHA-256 + ACs do pacote, com categoria e validade), `mod.rs` (`verificar(xml, &Ancoras) -> Result<Prova, Recusa>`); a **regra de titular é uma função plugável** (hoje: CNPJ/CPF nos OIDs do e-CNPJ/e-CPF; também CNPJ em `serialNumber`, como no Selo Eletrônico — Resolução 211/2024), para a transição de certificados não exigir reescrita, e `autorizacao.rs` (estados da §5.2, sem rede). Estrutura aceita (anti "wrapping"): uma `Signature` filha de `NFe` depois de `infNFe`; uma `Reference` para o `Id` do `infNFe`, **único** no documento; transformações exatamente `[enveloped-signature, c14n-20010315]`; pares sha1/rsa-sha1 ou sha256/rsa-sha256; 1 certificado de `KeyInfo`; sem `DOCTYPE`; ≤ 5 MB; profundidade ≤ 64. Ordem: estrutura → resumo → assinatura RSA → cadeia na data `dhEmi` (+48 h de tolerância no início da validade do certificado) → titular (raiz do CNPJ/CPF) → `tpAmb = 1` → só então `nfe_xml` lê os dados do **mesmo nó** e a coerência atual segue como verificação adicional. `importar_xml` verifica **antes** de qualquer escrita (inclusive `garantir_fornecedor`).

## 8. Testes de aceitação finais (escritos **antes** do código)

**Material:** (a) acervo real do dono (**fora do repositório**, `#[ignore]`, caminho por variável de ambiente) e o pacote de ACs verificado; (b) infraestrutura de teste fictícia (raiz, AC, emitente) gerada com `openssl` e guardada em `nucleo/tests/pki/` ("SEM VALIDADE – SÓ TESTE"); `nfe_exemplo.xml` reassinado por ela; âncoras de teste **injetadas** só em teste; (c) libxml2 e, se possível, `xml-sec` como avaliadores diferenciais. Resultado **simulado** ≠ **físico**.

| ID | Teste | Esperado |
|---|---|---|
| A-01 | Acervo real + pacote verificado | **528 aceitas**; as 4 de teste fsist recusadas; as 2 sem assinatura recusadas. |
| A-02 | Acervo sem o pacote de ACs | Recusa "autoridade certificadora desconhecida"; nunca aceita por exceção. |
| A-03 | C14N de `infNFe` e `SignedInfo` × libxml2 nos 539 + sintéticos (atributos, namespaces, `&#xD;`, `]]>`, `>`/`"`, vazio, CRLF, prefixos) | **Bytes idênticos.** |
| A-04 | Documento da PKI de teste | Aceita; `Prova` traz emitente, cadeia e data. |
| A-05 a A-08 | Alterar `qCom`; `vUnCom`/`vProd`/`vNF`; `xProd` (descrição); `cProd`/NCM/CFOP/CNPJ/IE/`dup`/1 byte — **mantendo `DigestValue`, `digVal`, chave, protocolo** | Recusa em todos, um teste por campo. |
| A-09 | Resumo recalculado para a nota adulterada, assinatura original | Recusa (assinatura). |
| A-10 | `SignatureValue` de outra nota | Recusa. |
| A-11 | Certificado ICP de outra empresa colocado no XML | Recusa. |
| A-12 | Nota adulterada reassinada com certificado próprio autoassinado | Recusa (cadeia). |
| A-13 | AC fictícia com nome "ICP-Brasil" | Recusa (âncora é por SHA-256). |
| A-14 | AC intermediária ausente / assinatura da AC corrompida / AC de outra raiz | Recusa, um motivo por caso. |
| A-15 | Certificado fora da validade **na data de emissão** | Recusa. |
| A-16 | Certificado vencido **hoje**, válido na emissão | **Aceita.** |
| A-17 | AC com `CA:FALSE`, sem `keyCertSign`, `pathLen` estourado | Recusa. |
| A-18 | CNPJ de outra raiz; mesma raiz/outra filial; CPF | Recusa; aceita; confere CPF. |
| A-19 | `tpAmb = 2` | Recusa. |
| A-20 | Teste do limite conhecido **invertido**; aviso fixo removido | O adulterado antes aceito é **recusado**. |
| A-21 | Wrapping: dois `infNFe`/`Id`, `Reference` para outro `Id`, `Signature` fora do lugar, duas `Signature` | Recusa em todos; dados lidos do nó verificado. |
| A-22 | Confusão de algoritmos e transformações extras | Recusa. |
| A-23 | Malformados: vazio, truncado, binário, UTF-8 inválido, `DOCTYPE` com entidade, 6 MB, profundidade 200, DER/Base64 inválido | Recusa limpa, sem pânico, < 2 s. |
| A-24 | **Atomicidade**, por caso de recusa: antes = depois em `compra`, `compra_item`, `pessoa` (fornecedor!), estoque/movimentos, custo, `conta_pagar`, auditoria, `alteracao` | Nada muda. |
| A-25 | Importação por XML de cadastro, mesmos casos | Nada é cadastrado. |
| A-26 | Notas válidas | Rascunho gerado como hoje; `xml_verificacao = verificada`. |
| A-27 | Migração 19 → 20 | Compras antigas `nao_verificada`; nada apagado; `duplicatas_da_compra` funciona. |
| A-28 | Nota já importada | Mantém a recusa por duplicidade (verificação primeiro). |
| A-29 / A-30 | Pacote de ACs atualizado: AC nova que fecha / tentativa de adicionar **raiz** | Passa / recusado; pacote anterior continua. |
| **A-34** | **`lancar` com `autorizacao = aguardando`** | **Recusado; estoque, custo e financeiro inalterados**; mesmo chamando direto a API. |
| **A-35** | `lancar` com `negada` | Recusado sempre, também com atestado. |
| **A-36** | `confirmada_gerente` (D7-B): só gerente; exige chave/protocolo/data; fica na auditoria; rótulo visível; atestado de operador recusado | Conforme. |
| **A-37** | `protNFe` **falso** no XML (protocolo e `digVal` copiados) | **Não** conta como prova: continua `aguardando`. |
| **A-38** | `protNFe` assinado pela SEFAZ (fixture da PKI de teste) | `comprovada_assinatura_sefaz` só se a cadeia passar. |
| **A-39** | Compra digitada (sem XML) | Lança como hoje. |
| **A-40** | Estorno/lançamentos antigos | Intactos. |
| **A-42** | **Titular pelo Selo Eletrônico:** certificado de teste com o CNPJ no `serialNumber` do titular (sem o OID do e-CNPJ) | Aceito quando a raiz do CNPJ bate; recusado quando não bate (extração do titular é **plugável**, ver `docs/FISCAL-assinatura-arquitetura.md`) |
| A-31 | 528 notas em sequência | Tempo total e por nota registrados (meta < 50 ms/nota no final). |
| A-32 | Suítes completas, **desenvolvimento e programa final (release)**, clippy, formatação | 0 falhas. |
| A-33 | `cargo tree`; compilação das bibliotecas novas para Linux; **Windows e Android não testados aqui** | Registrado (L5). |
| A-41 | Avaliador diferencial (`xml-sec`, se suportar SHA-1): mesma decisão que o verificador | Sem divergência; se divergir, **bloqueia a entrega**. |

**Mutações planejadas** (defeitos plantados, todos pegos): ignorar o resumo; comparar só um prefixo; não conferir a assinatura; usar chave de outro ponto do XML; aceitar sem cadeia; âncora por nome; data de hoje em vez de `dhEmi`; âncora v3/v10/v11 aceita; certificado do XML tratado como âncora; não conferir `CA`/`pathLen`; não conferir CNPJ; aceitar `tpAmb=2`; ler os dados de um segundo `infNFe`; aceitar `Reference` para outro `Id`; transformações extras; **gravar a compra/fornecedor antes de verificar**; **`lancar` sem consultar `autorizacao`**; **`protNFe` do XML tratado como prova**; **`negada` liberada**; RSA-1024; ordem dos atributos na C14N; pacote com raiz nova; `nao_verificada` virar `verificada` na migração.

## 9. Limites que continuam depois da implementação (não escondidos)

* **L1 (maior): sem o PEND-15 a autorização só tem atestado humano (D7-B) ou fica bloqueada (D7-A).** O módulo fiscal **não é "pronto para produção"** até P2 ser automática ou você aceitar formalmente o atestado.
* **L2 — Revogação:** sem internet não se consulta a LCR. Certificado revogado depois da emissão passa.
* **L3 — SHA-1:** o padrão da NF-e 4.00 o impõe; colisão de prefixo escolhido é cara; sem solução fora do padrão.
* **L4 — Âncoras:** raiz comprometida/revogada exige atualizar o programa; cadeias v4, v6, v7, v13 (ECDSA/Ed448/pós-quântico) e raiz original não suportadas.
* **L5 — Plataformas:** só Linux compilado e testado aqui; `ring`/`webpki` em Windows e Android (32 e 64 bits) a conferir.
* **L6 — Emitente sem identificação no certificado** (certificado de procuração, CPF sem OID) é recusado.
* **L7 — Fora do escopo:** CT-e e eventos; DANFE em PDF (não assinado).
* **L8 — Cadeias históricas SHA-1** (v1/original): dependem do adaptador controlado; se não for adotado, notas dessa época são recusadas com motivo claro.
* **L9 — A consulta de autorização exige certificado da empresa e internet** (premissa a confirmar na documentação técnica oficial no início do PEND-15; a página pública do portal usa verificação "não sou robô" e não é automatizável).

## 10. Etapas, modelo e esforço

| Etapa | O que | Modelo / esforço |
|---|---|---|
| E1 | Desenho, medições, pacote verificado (feito) | Sonnet 5.5 |
| E2 | **Sua aprovação** do desenho, D7 e D8 | Você |
| E3a | Teste de viabilidade inicial: `xml-sec` como avaliador, adaptador SHA-1, `ring` e `webpki` nos 3 alvos de compilação disponíveis | Opus 5.5, alto |
| E3b | Testes A-xx primeiro; PKI de teste; `c14n`; `xmldsig`; cadeia; autorização; migração 0020; telas | **Opus 5.5, alto** |
| E4 | Mutações, suítes completas (desenvolvimento **e** release), clippy, inspeção visual, entrega | Sonnet 5.5, médio |
| E5 | PEND-15 (consulta à SEFAZ com certificado) — só se você mandar | Opus 5.5, alto |
| E6 | Windows e Android (L5) | Você, com os aparelhos |
