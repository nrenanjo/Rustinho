# PEND-09 · Validação criptográfica da NF-e (assinatura do emitente) e prova de autorização da SEFAZ

**Status:** **revisão 4, critérios finais** (20/10/2026, 14:4x), **sem código ainda**. A revisão 4 fecha as duas questões que você levantou: **instante de validade do certificado** (§3.5) e **uso do certificado: assinatura × TLS** (§3.6). Incorpora a revisão sobre biblioteca existente, âncoras por critério histórico e pacote oficial verificado (rev. 2) e as decisões **D7 e D8 aprovadas**. Versão 0.9.11; só commits locais; nada vai ao GitHub.
**Decisões D1 a D8: todas aprovadas por você em 20/10/2026** (D1–D6 às 13:37; D7 e D8 às 14:06), sem opção de forçar importação. **D7 é provisória** (vale até o PEND-15 existir e ser revisada nesse momento).
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
| Cadeia (biblioteca `rustls-webpki`, âncoras v5+v2 do pacote, data = `dhEmi`) | **528 aceitas**, 4 recusadas (fsist de teste); só v5: 527; âncoras v10/v11: **0 de 532**; data = hoje: só 190 aceitas, 342 recusadas | Biblioteca madura resolve a cadeia; a **data de referência é a de emissão**. |
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

`rustls-webpki` constrói e valida o caminho (assinatura de cada elo, `CA`/`pathLen`, validade na **data de referência** passada por nós, nomes, extensões críticas desconhecidas). **Lido no código da biblioteca (0.103.16):** ela **ignora o `KeyUsage`** (inclusive `keyCertSign` das ACs) e aplica a regra de `ExtendedKeyUsage` passada por nós a **todos** os certificados do caminho; por isso o Rustinho confere `KeyUsage` por conta própria e passa uma regra de EKU própria (§3.6). Medido com os dados reais (§2). Limites a tratar no teste de viabilidade inicial da implementação: (i) a biblioteca **não aceita SHA-1 em certificado** → para as raízes v1 e original, avaliar um **adaptador SHA-1 controlado** (algoritmo extra declarado só para essas raízes e só com `dhEmi` dentro da validade da raiz); se não for seguro, essas cadeias ficam **recusadas com motivo claro**; (ii) certificados de ACs antigos com DER fora do padrão (1 caso, Serpro) ficam fora; (iii) ECDSA P-521/Ed448/pós-quântico: sem suporte inicial.

### 3.5 Instante de validade do certificado (revisão 4, item 1)

**Regras oficiais:** o Manual de Orientação do Contribuinte (padrões técnicos, versão 6.0, item 3.2.5) manda a SEFAZ "verificar o prazo de validade do certificado utilizado" e conferir a revogação pela lista "disponível no momento da conferência da assinatura", isto é, **no recebimento**; a regra E02 (rejeição 291) confere "data início e data fim" da validade; a regra B09-10 (rejeição 703) recusa `dhEmi` mais de **5 minutos depois** do recebimento; a B09-20 (rejeição 228) recusa emissão com mais de **30 dias** (salvo contingência). Nenhuma regra oficial dá **tolerância de 48 h** antes do início da validade.

**Medição no acervo** (528 NF-e genuínas, emitidas de 04/03/2021 a 14/10/2026, todas `tpEmis = 1`; as 4 de teste fsist já excluídas):

| Alternativa (certificado e cadeia válidos em…) | Aceitas | Recusadas | Observação |
|---|---|---|---|
| A. agora (20/10/2026) | 190 | 338 | Recusaria notas legítimas antigas. **Descartada.** |
| B. `dhEmi` (estrito) | 528 | 0 | `dhEmi` está **dentro** da região assinada. |
| C. `dhEmi` com 48 h de tolerância | 528 | 0 | A tolerância **nunca foi usada**: a menor folga entre o início da validade e `dhEmi` é de **5 h 11 min**. **Sem fundamento: removida.** |
| D. `dhRecbto` (estrito) | 528 | 0 | Instante em que a SEFAZ conferiu, mas o campo **não é assinado** (qualquer um escreve). |
| **E. `dhEmi` E `dhRecbto` (estritos)** | **528** | **0** | **Escolhida.** |

Cadeia completa (biblioteca de cadeia, âncoras v5+v2): **528 de 528 em `dhEmi` e 528 de 528 em `dhRecbto`**. Nenhum certificado começa ou vence entre `dhEmi` e `dhRecbto` no acervo.

**Por que E:** `dhEmi` é a afirmação **assinada** do emitente; `dhRecbto` é quando a SEFAZ **de fato** conferiu a validade (MOC). Uma nota autorizada de verdade passou nas duas condições. Exigir as duas **só restringe**: o `dhRecbto`, por não ser assinado, nunca é usado para **ampliar** a aceitação. **Limite honesto:** quem tiver uma chave vazada de um certificado vencido pode escrever datas antigas nos dois campos; nenhuma data dentro do XML protege disso. Só a consulta à SEFAZ (PEND-15) ou a conferência do gerente (D7) cobrem esse caso.

**Coerência entre os dois instantes (B09) não é exigida:** no acervo há **1 nota do Paraná** (em 2 arquivos) com o recebimento carimbado pela SEFAZ com fuso `-02:00` errado (os relógios distam 14 s, mas a conta dá "recebida 1 h antes da emissão"). Exigir a regra B09 recusaria uma nota legítima por defeito do carimbo da SEFAZ; ela fica só registrada no resultado (aviso técnico, sem recusa).

### 3.6 Uso do certificado: assinatura do XML × autenticação TLS (revisão 4, item 2)

**Fontes oficiais:** MOC (padrões técnicos, "Padrão de Certificado Digital"): certificado ICP-Brasil **tipo A1 ou A3**, com o CNPJ do titular no `otherName` **2.16.76.1.3.3**; **para assinatura de mensagens:** o CNPJ de um estabelecimento do emitente e o **"uso da chave" previsto para assinatura digital**, respeitando a política do certificado; **para transmissão (TLS):** o CNPJ de quem transmite (pode ser outro) e a extensão **Extended Key Usage com "Autenticação Cliente"**. Políticas de Certificado tipo **A1** (OID 2.16.76.1.2.1.161) e **A3** (OID 2.16.76.1.2.3.152) da AC IDFEDERAL, versão 1.1 de março/2025, item 7.1.2: **`KeyUsage` crítica com `digitalSignature`, `nonRepudiation` e `keyEncipherment`**; **`ExtendedKeyUsage` obrigatória, não crítica, com pelo menos `clientAuth` (1.3.6.1.5.5.7.3.2) ou `emailProtection` (1.3.6.1.5.5.7.3.4)**; `certificatePolicies` com o OID da política; para pessoa jurídica, `otherName` 2.16.76.1.3.4, .2, **.3 (CNPJ)** e .7; para pessoa física, **2.16.76.1.3.1** (CPF nas posições 9 a 19). Regra F03 da NT 2026.001 (rejeição 213): **raiz do CNPJ do certificado = raiz do CNPJ do emitente**; F03A (rejeição 227): CPF igual.

**Medição no acervo (528 certificados de assinatura):** `KeyUsage` crítica com `digitalSignature + nonRepudiation + keyEncipherment` em **528**; `ExtendedKeyUsage` não crítica com `clientAuth + emailProtection` em **521** (+ `msSmartcardLogin` em 7); políticas **2.16.76.1.2.1.\*** (A1) em **479** e **2.16.76.1.2.3.\*** (A3) em **49**; `otherName` .2, .3, .4 e .7 em **528**; `BasicConstraints` `CA:FALSE` em 508 e ausente em 20; nenhuma extensão crítica além de `keyUsage` e `basicConstraints`. **ACs do pacote:** `keyCertSign` em todas as legíveis; 330 sem EKU; 7 com `serverAuth` (são as ACs de SSL, fora das âncoras de NF-e).

**Regra de uso do Rustinho para verificar a assinatura do XML (perfil "NF-e, leiaute 4.00, A1/A3"):**

| # | Regra | Base | Acervo |
|---|---|---|---|
| U1 | `KeyUsage` **presente e crítica**, com **`digitalSignature`** e **`nonRepudiation`** | MOC ("uso da chave para assinatura") + PC A1/A3 item 7.1.2.7.f | 528/528 |
| U2 | `keyEncipherment` **não** é exigido para verificar assinatura (é uso de cifragem de chave); a ausência fica registrada como "fora do perfil", **sem** recusar | PC exige o bit, mas ele não participa da assinatura | 528/528 têm |
| U3 | `ExtendedKeyUsage` **presente**, com **`clientAuth` ou `emailProtection`**. Ela é conferida **como conformidade com o perfil A1/A3**, **não** como permissão de assinar: o perfil não tem um propósito de "assinatura de documento"; uma leitura literal da RFC 5280 recusaria os 528 certificados legítimos | PC A1/A3 item 7.1.2.7.f | 528/528 |
| U4 | `certificatePolicies` com OID sob **2.16.76.1.2.1** (A1) ou **2.16.76.1.2.3** (A3). Outros tipos (A2, A4, S, T, **Selo SE-S/SE-H**) são recusados com "tipo de certificado não aceito para NF-e por este perfil" até haver fonte oficial que os inclua | MOC ("tipo A1 ou A3") | 528/528 |
| U5 | `BasicConstraints` ausente ou `CA:FALSE` | perfil de usuário final | 528/528 |
| U6 | Titular: `otherName` 2.16.76.1.3.3 com a mesma **raiz de 8 dígitos** do CNPJ do emitente; emitente com CPF: `otherName` 2.16.76.1.3.1 com o mesmo CPF | MOC + F03/F03A | 528/528 (CNPJ) |
| U7 | Extensão crítica desconhecida → recusa | RFC 5280 (feito pela biblioteca) | 0 casos |
| U8 | ACs do caminho: `CA:TRUE`, **`keyCertSign`** (conferido pelo Rustinho, pois a biblioteca ignora), `pathLen`; EKU das ACs **não** restringe a assinatura | perfil de AC | 29/29 caminhos reais |

**Para TLS (futuro: PEND-15 e emissão), perfil separado:** EKU **com `clientAuth` obrigatório** (MOC); CNPJ de quem transmite, que pode não ser o do emitente. **Os dois perfis não se misturam no código:** a verificação de XML nunca usa a regra `clientAuth` (o protótipo usava, e isso está corrigido nestes critérios).

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
| `confirmada_gerente` | O **gerente** conferiu a autorização no portal oficial e registrou, **na mesma ação**: chave, protocolo (iguais aos do XML), data e hora, e a situação conferida = **Autorizada**; autenticação de gerente refeita; **auditoria**. É **atestado humano, não prova criptográfica**, e a tela diz isso (D7, provisória) | Sim |
| `negada` | O gerente registrou que a nota está **rejeitada, cancelada ou denegada** (ou, no futuro, a SEFAZ informou isso na consulta automática) | **Não, sem exceção**; nenhum atestado posterior a libera; o rascunho só pode ser descartado. A ação pede confirmação antes de gravar, para evitar clique errado |
| compra **sem XML** (digitada) | Não há nota eletrônica | Sim (como hoje) |

**Caminhos de comprovação (separados, com o que cada um custa):**
1. **PEND-15 (automático):** consulta à SEFAZ pela chave dentro do programa (web service de consulta, que exige **certificado digital da empresa** e internet) → `comprovada_sefaz`. Exige: importar/guardar o certificado A1 da loja (segredo novo), cliente TLS com certificado, tratamento de ambiente/UF, limites de uso. É uma pendência própria e **não** é feita nesta etapa.
2. **Confirmação do gerente (D7, provisória):** diálogo "Registrar conferência no portal…" (só gerente): situação conferida (**Autorizada** / Rejeitada / Cancelada / Denegada), chave, protocolo, data e hora. Só "Autorizada" com chave e protocolo iguais aos do XML libera; as outras três gravam `negada`. Fica rotulada `confirmada_gerente`.
3. **Assinatura da SEFAZ no `protNFe`:** aceita se existir e validar.

**Efeito na tela** (sem poluir): o rascunho de compra mostra **uma** linha curta no cabeçalho da nota ("Assinatura do emitente conferida · autorização da SEFAZ ainda não comprovada"), e o botão **Lançar** fica desabilitado com o motivo ao tocar; para o gerente aparece **uma** ação "Registrar conferência no portal…" (diálogo com chave, protocolo, data). Operador não registra. Nada de aviso fixo permanente: o aviso `LIMITE_DA_CONFERENCIA` sai.

## 6. Decisões

| ID | Decisão | Estado |
|---|---|---|
| D1 a D6 | Sem forçar importação; cadastros por XML também verificados; titular do certificado; homologação recusada; compras antigas só marcadas; CPF quando aplicável | **Aprovadas** |
| **D7** | **Atestado do gerente libera o lançamento, provisoriamente.** Condições obrigatórias: conferência no **portal oficial**; **chave** (44 dígitos) e **protocolo** (`nProt`, 15 dígitos) digitados e **iguais aos do XML**; **data e hora da conferência**; **identificação do gerente** (autenticação de gerente refeita no ato); **auditoria**; a tela diz **"Conferência humana do gerente, não é prova automática da SEFAZ"**; nota **rejeitada, cancelada ou denegada nunca é liberada** por esse caminho; o XML e o protocolo dele, sozinhos, nunca comprovam autorização | **Aprovada (provisória)** |
| **D8** | **Cadastros por XML** (pessoas e peças): exigem **assinatura válida do emitente (P1)**; **não** exigem autorização (P2); **não podem alterar estoque, custos nem financeiro** (teste A-43) | **Aprovada** |

## 7. Desenho técnico (resumo)

Módulo `nucleo/src/fiscal/` (puro, sem rede, sem banco): `c14n.rs` (perfil da NF-e), `xmldsig.rs` (estrutura estrita e verificação, RSA via `ring`), `cadeia.rs` (invólucro fino sobre `rustls-webpki`), `ancoras.rs` (âncoras por SHA-256 + ACs do pacote, com categoria e validade), `mod.rs` (`verificar(xml, &Ancoras) -> Result<Prova, Recusa>`); a **regra de titular é uma função plugável** (hoje: CNPJ/CPF nos OIDs do e-CNPJ/e-CPF; também CNPJ em `serialNumber`, como no Selo Eletrônico — Resolução 211/2024), para a transição de certificados não exigir reescrita, e `autorizacao.rs` (estados da §5.2, sem rede). Estrutura aceita (anti "wrapping"): uma `Signature` filha de `NFe` depois de `infNFe`; uma `Reference` para o `Id` do `infNFe`, **único** no documento; transformações exatamente `[enveloped-signature, c14n-20010315]`; pares sha1/rsa-sha1 ou sha256/rsa-sha256; 1 certificado de `KeyInfo`; sem `DOCTYPE`; ≤ 5 MB; profundidade ≤ 64. Ordem: estrutura → resumo → assinatura RSA → cadeia e certificado válidos em `dhEmi` **e** em `dhRecbto`, sem tolerância (§3.5) → uso do certificado U1–U8 (§3.6) → titular (raiz do CNPJ/CPF) → `tpAmb = 1` → só então `nfe_xml` lê os dados do **mesmo nó** e a coerência atual segue como verificação adicional. `importar_xml` verifica **antes** de qualquer escrita (inclusive `garantir_fornecedor`).

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
| A-15 | Certificado fora da validade em `dhEmi` (vencido antes; começa **1 s** depois de `dhEmi`, sem tolerância) e, separadamente, fora da validade em `dhRecbto` com `dhEmi` válido | Recusa nos três casos. |
| A-16 | Certificado vencido **hoje**, válido em `dhEmi` e `dhRecbto` (como 338 do acervo) | **Aceita.** |
| A-16b | AC intermediária ou raiz (v2) válida em `dhEmi` e vencida em `dhRecbto` | Recusa. |
| A-16c | Nota do acervo com fuso errado no `dhRecbto` (Paraná) | **Aceita**; aviso técnico registrado. |
| A-17 | AC com `CA:FALSE`, sem `keyCertSign`, `pathLen` estourado | Recusa. |
| A-18 | CNPJ de outra raiz; mesma raiz/outra filial; CPF | Recusa; aceita; confere CPF. |
| **A-49** | `KeyUsage` ausente; não crítica; sem `digitalSignature`; sem `nonRepudiation` | Recusa nos quatro. |
| **A-50** | `KeyUsage` sem `keyEncipherment` (resto conforme) | **Aceita**, com registro "fora do perfil". |
| **A-51** | EKU ausente; EKU só com `serverAuth`; EKU vazia | Recusa. EKU só `emailProtection`: **aceita**; só `clientAuth`: **aceita**; `clientAuth + emailProtection + msSmartcardLogin` (7 do acervo): **aceita**. |
| **A-52** | Política fora de A1/A3 (A4, Selo de teste, SSL) ou ausente | Recusa com motivo "tipo de certificado". |
| **A-53** | Certificado de usuário com `CA:TRUE` | Recusa. |
| **A-54** | AC do caminho sem `keyCertSign` | Recusa (prova que o Rustinho confere o que a biblioteca ignora). |
| **A-55** | Acervo real: as 528 passam em U1–U8; o verificador **não** chama a regra `clientAuth` de TLS (teste de código: o perfil de XML e o de TLS são tipos diferentes) | 528 aceitas; teste de tipo passa. |
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
| **A-43** | **D8:** importar cadastros por XML (assinatura válida, **sem** autorização): cria/atualiza pessoas e peças; **estoque, custo médio, movimentos, contas a pagar/receber e `alteracao` financeira idênticos antes e depois** (comparação por linha); XML adulterado = nada é cadastrado | Conforme |
| **A-44** | **D7, campos obrigatórios:** sem chave, sem protocolo, protocolo diferente do `nProt` do XML, chave diferente, data/hora ausente, gerente não autenticado ou usuário operador | Recusado; `autorizacao` continua `aguardando` |
| **A-45** | **D7, situação:** "Rejeitada", "Cancelada" ou "Denegada" → `negada`; **tentativa de atestar "Autorizada" depois de `negada`** | Recusada; `lancar` continua bloqueado |
| **A-46** | **D7, trilha:** cada atestado grava gerente, data/hora do registro, data/hora informada da conferência, chave, protocolo, situação e rótulo "confirmação humana" na auditoria; a tela da compra mostra o rótulo | Conforme |
| **A-47** | **Pacote de ACs como intermediárias, não como confiança:** todas as raízes do pacote presentes na lista de certificados, mas **só** as âncoras selecionadas por SHA-256 validam; v3 (revogada), v10 e v11 (finalidade), v4/v6/v7/v13 (sem suporte) **não** validam nenhuma nota; certificado do XML igual a uma raiz do pacote **não** a promove; hashes originais e evidências guardados | Conforme |
| **A-48** | Mudar um byte de qualquer certificado embutido (pacote ou âncora) faz o teste de integridade (SHA-256 do arquivo e de cada âncora) **falhar** | Falha, bloqueia a compilação do teste |
| A-31 | 528 notas em sequência | Tempo total e por nota registrados (meta < 50 ms/nota no final). |
| A-32 | Suítes completas, **desenvolvimento e programa final (release)**, clippy, formatação | 0 falhas. |
| A-33 | `cargo tree`; compilação das bibliotecas novas para Linux; **Windows e Android não testados aqui** | Registrado (L5). |
| A-41 | Avaliador diferencial (`xml-sec`, se suportar SHA-1): mesma decisão que o verificador | Sem divergência; se divergir, **bloqueia a entrega**. |

**Mutações planejadas** (defeitos plantados, todos pegos; inclui, para a revisão 4: voltar a tolerância de 48 h; conferir só `dhEmi`; conferir só `dhRecbto`; usar a data de hoje; não conferir `KeyUsage`/`nonRepudiation`; aceitar EKU ausente; exigir `clientAuth` (recusaria `emailProtection`); aceitar política fora de A1/A3; não conferir `keyCertSign` das ACs; e, para D7/D8: atestado sem conferir o protocolo do XML; `negada` liberável por atestado; operador atestando; cadastro por XML mexendo em estoque/custo; qualquer raiz do pacote tratada como âncora): ignorar o resumo; comparar só um prefixo; não conferir a assinatura; usar chave de outro ponto do XML; aceitar sem cadeia; âncora por nome; data de hoje em vez de `dhEmi`; âncora v3/v10/v11 aceita; certificado do XML tratado como âncora; não conferir `CA`/`pathLen`; não conferir CNPJ; aceitar `tpAmb=2`; ler os dados de um segundo `infNFe`; aceitar `Reference` para outro `Id`; transformações extras; **gravar a compra/fornecedor antes de verificar**; **`lancar` sem consultar `autorizacao`**; **`protNFe` do XML tratado como prova**; **`negada` liberada**; RSA-1024; ordem dos atributos na C14N; pacote com raiz nova; `nao_verificada` virar `verificada` na migração.

**Verificação de fechamento (executada depois da implementação):** suítes completas em **desenvolvimento e release**, mutações, testes de atomicidade (A-24, A-25, A-43) e registro explícito do que **não** foi testado em **Windows e Android**.

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

## 11. Resultados da implementação (20/10/2026) — comprovado no Linux; Windows e Android não testados

Commits locais (nada no GitHub): `f976c4f` testes e PKI de teste **antes** do verificador; `e07b0a4` verificador; `f9fbb6c` integração (importação, cadastro, lançamento, D7, migração 0020, tela, trava); `2a23d1a` teste e documentos.

### 11.1 Desvios do plano (registrados, não escondidos)

| Item | O que mudou | Por quê |
|---|---|---|
| D8 / cadastro | `verificar_para_cadastro` aceita também a `NFe` **sem** `nfeProc`/protocolo (a assinatura é exigida igual; a validade é conferida em `dhEmi` e, se houver protocolo, também em `dhRecbto`) | D8: cadastro não exige autorização da SEFAZ. A compra continua exigindo `nfeProc` com protocolo e `dhRecbto`. |
| A-38 (`comprovada_assinatura_sefaz`) | **Não implementado.** Uma `Signature` dentro do `protNFe` é **ignorada**: não conta como prova e a compra fica `aguardando` | Nenhuma das 532 notas reais tem; implementar sem caso real aumentaria a superfície. Pendência. |
| A-41 (avaliador `xml-sec`) | **Não executado.** | Os oráculos usados foram o libxml2 (16 casos sintéticos) e **as 532 assinaturas reais** (o resumo do próprio emitente): ambos independentes da implementação. |
| §3.4-i (adaptador SHA-1 para as raízes v1/original) | **Não adotado**: v1 e a original não são âncoras | Nenhuma nota do acervo depende delas. Notas dessa época são recusadas com "não chega a uma raiz reconhecida". |
| A-29/A-30 (atualizar o pacote de ACs pela tela) | **Não implementado**: o pacote vai embutido e conferido pelo SHA-512 a cada abertura | Pendência; nenhuma raiz pode entrar por fora do programa. |
| Achado durante a implementação | O leitor de XML (`roxmltree`) é recursivo e não limita a profundidade: um XML com milhares de níveis **derrubava o programa** (estouro de pilha, não interceptável) — risco que já existia na importação. **Corrigido**: a profundidade é contada antes de ler, também no leitor de antes (`nfe_xml::ler`). | Teste `profundidade_e_contada_antes_de_ler_sem_estourar_a_pilha` (100 mil níveis) e mutação V24. |
| Painel "Para lançar" | Passa a dizer "Falta a conferência da autorização" / "Não pode ser lançada" em vez de "Tudo pronto" | Achado na inspeção visual: a tela se contradizia. |
| Trava do aplicativo | O aplicativo (computador e Android) **se recusa a abrir** se o recurso `ancoras-de-teste` (raiz fictícia) tiver sido compilado | Só os testes ligam esse recurso. |

### 11.2 Testes de aceitação → testes → resultado (simulado, PKI de teste)

| Critério | Teste(s) | Resultado |
|---|---|---|
| A-03 C14N | `a03_c14n_bate_com_o_libxml2_nos_casos_sinteticos` (16 casos) e `acervo_a03_…` | ok; **532/532** resumos reais reproduzidos |
| A-04 | `a04_nota_assinada_pela_pki_de_teste_e_aceita`, `a04_par_sha256_…` | ok |
| A-05 a A-08 | `a05_a08_qualquer_mudanca_no_conteudo_assinado_e_recusada` (qCom, vUnCom, vProd, vNF, xProd, cProd, NCM, CFOP, IE, vDup, xNome e 1 espaço) | ok |
| A-09 a A-14 | `a09_…`, `a10_…`, `a11_…`, `a12_…`, `a13_…`, `a14_cadeia_quebrada` | ok |
| A-15, A-16, A-16b, A-16c | `a15_…`, `a16_…`, `a16b_…`, `a16c_fuso_errado_…`, `protocolo_sem_data_de_recebimento_e_recusado` | ok |
| A-17, A-49 a A-55 | `a49_…`, `a50_…`, `a51_eku_e_conferido_como_perfil_nao_como_tls`, `a52_…`, `a53_…`, `a54_…`, `rsa_1024_e_recusado` | ok |
| A-18, A-19 | `a18_titular_do_certificado` (CNPJ, filial, PrintableString, CPF), `a19_homologacao_e_recusada` | ok |
| A-20 | `a20_xml_adulterado_com_resumos_mantidos_e_recusado_sem_gravar_nada` (o antigo "limite conhecido", invertido) | ok |
| A-21, A-22, A-23 | `a21_…` (14 variações), `a21_dados_lidos_sao_do_no_verificado`, `a22_…` (7), `a23_…` (8, cada um < 2 s), profundidade | ok |
| A-24, A-25, A-43 | `a24_atomicidade_de_toda_recusa_da_assinatura` (25 notas + 4 malformadas; compra, itens, **pessoa**, peça, estoque, custo, A pagar, auditoria, `alteracao`), `a43_cadastro_por_xml_…` | ok |
| A-26, A-27, A-28 | `a26_…`, `a27_migracao_20_…`, duplicidade em `importa_liga_lanca_e_estorna` | ok |
| A-34 a A-36, A-44 a A-46 | `a34_a35_…`, `a36_a46_conferencia_do_gerente`, `d5_compra_anterior_nao_e_bloqueada`, contrato de tela `compra_por_xml` (senha errada, D7 pela API) | ok |
| A-47, A-48 | `a47_so_as_ancoras_escolhidas_valem_…`, `a47_certificado_do_xml_nunca_vira_ancora`, `a48_pacote_embutido_confere_com_o_sha512_publicado_pelo_iti` | ok |
| A-01, A-02, A-31 (acervo real, fora do repositório) | `acervo_a01_a55_notas_reais_com_as_ancoras_da_icp_brasil` | **528/528** aceitas (479 A1, 49 A3; 3 avisos: 2 fusos do Paraná + 1 cadeia histórica v2); 4 de teste recusadas (RSA-1024); 9 não-NF-e recusadas; ~1,9 ms por arquivo (programa final) |

### 11.3 Mutações (defeitos plantados): **35 de 35 pegas**

V1 resumo ignorado · V2 só o começo do resumo (pego depois do teste `comparacao_do_resumo_confere_todos_os_bytes`) · V3 RSA não conferida · V4 âncora pelo nome · V5 data de hoje · V6 tolerância de 48 h · V7 só `dhEmi` · V8 só `dhRecbto` · V9–V11 KeyUsage (digitalSignature, nonRepudiation, crítica) · V12 EKU fora do perfil · V13 regra de TLS (`clientAuth`) na assinatura · V14 política fora de A1/A3 · V15 AC sem keyCertSign · V16 AC como emitente · V17 raiz do CNPJ · V18 homologação · V19 segundo `infNFe` · V20 Id repetido · V21 transformações extras · V22 RSA-1024 · V23 C14N sem ordenar atributos · V24 sem contagem de profundidade (teste aborta) · V25 certificado do XML como âncora · V26 raiz histórica vencida · C1 fornecedor gravado antes · C2 lançar sem autorização · C3 negada liberada · C4 protocolo do XML como prova · C5 operador atesta · C6 protocolo não conferido · C7 negada atestada de novo · C8 migração marca como verificada · C9 conferência no futuro. Arquivos restaurados idênticos (SHA-256).

### 11.4 Inspeção visual real (aplicativo montado **sem** as âncoras de teste, Linux/Xvfb)

Tela larga: compra aguardando (faixa discreta + "Registrar conferência…"), lançar bloqueado (mensagem), diálogo (chave, situação, protocolo, data e hora, senha), protocolo errado (erro no campo), conferida (rótulo "conferência humana… não é prova automática"), itens decididos sem conferência ("Falta a conferência da autorização"), cancelada ("Registrar e bloquear", faixa vermelha, "Não pode ser lançada"). Tela estreita (412 px): faixa com o botão abaixo; diálogo com a chave quebrando a linha (corrigido) e rótulo curto (corrigido). Defeito antigo achado: PEND-17.

### 11.5 O que foi comprovado e o que não foi

| | Linux (este ambiente) | Windows | Android 32/64 |
|---|---|---|---|
| Compilação (núcleo e aplicativo) | **sim** (desenvolvimento e programa final) | **não testado** | **não testado** (`ring` tem código C/assembly por plataforma) |
| Testes automáticos | **sim** (§11.6) | não | não |
| Acervo real | **sim** (528/528) | não | não |
| Tela | **sim** (larga e estreita) | não | não (toque e teclado virtual não vistos) |
| Desempenho | ~1,9 ms/nota | não medido | não medido (celular de 2 GB) |

Continua **não provado em lugar nenhum**: autorização da SEFAZ (PEND-15), revogação dos certificados (LCR), Selo Eletrônico. **O módulo fiscal não está pronto para produção.**

### 11.6 Verificação completa (commit `2a23d1a`, Linux)

| Etapa | Resultado |
|---|---|
| `cargo fmt --check` | ok |
| `cargo clippy --workspace --all-targets -D warnings` | ok |
| Testes, desenvolvimento | núcleo **297 ok + 3 ignorados** (volume e 2 do acervo real, rodados à parte); integração 9 / 4 / 7 / 5; **0 falhas** |
| Testes, programa final (release, `overflow-checks`) | mesmos números, **0 falhas** |
| Acervo real (release, fora do repositório) | 528/528 aceitas; 532/532 resumos; 1,58 ms por arquivo |
| Volume | ok |
| Montagem do aplicativo (release) | ok; a árvore de recursos do programa final **não** contém `ancoras-de-teste` |
| Mutações | 35/35 pegas |
