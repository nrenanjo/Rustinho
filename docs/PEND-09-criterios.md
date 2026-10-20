# PEND-09 · Validação criptográfica da NF-e (XMLDSig + cadeia ICP-Brasil)

**Status:** desenho e testes de aceitação **antes do código** (20/10/2026). Nenhuma linha de implementação foi escrita. Versão 0.9.11; só commits locais; nada vai ao GitHub.
**Regra de ouro:** *coerência entre campos não é prova de autenticidade.* Um XML só entra no estoque, no custo ou no financeiro se (1) o resumo do conteúdo assinado confere, (2) a assinatura RSA confere com a chave do certificado que veio no XML e (3) esse certificado leva a uma raiz da ICP-Brasil que o Rustinho já conhece. Qualquer falha = recusa, sem gravar nada.

## 0. O que preciso de você (só o item 1 bloqueia o código)

| # | Pedido | Por quê |
|---|---|---|
| 1 | **Baixar e me enviar, anexado, o pacote "Certificados das ACs da ICP-Brasil – Arquivo Único Compactado"** do ITI (gov.br/iti › Repositório › AC-Raiz). | O XML traz só o certificado da empresa emissora, **não** as ACs intermediárias. Sem elas a cadeia não fecha. Meu acesso à internet daqui bloqueia o servidor `acraiz.icpbrasil.gov.br`, então não consigo baixar. Baixar você mesmo, do site oficial, também é a origem de confiança correta. |
| 2 | Aprovar as decisões D1–D6 (§5). | Mudam o que o programa recusa. |
| 3 | **Trocar para Opus 5.5, esforço alto**, antes da implementação. | Previsto desde o início para esta pendência (criptografia). O desenho (esta etapa) foi feito em Sonnet 5.5. |

## 1. O que existe hoje (lido no código)

| Ponto | Hoje |
|---|---|
| `nfe_xml::conferir_coerencia` | Confere chave de 44 dígitos, dígito verificador, campos × chave, `Reference URI == #NFe<chave>`, protocolo e `DigestValue == digVal` (**dois textos copiados um do outro**). Não calcula nada. |
| `nfe_xml::LIMITE_DA_CONFERENCIA` | Aviso fixo na tela da compra e na recusa: "NÃO verifica a assinatura". |
| Teste `limite_conhecido_xml_adulterado_com_resumos_mantidos_ainda_passa` | Documenta o buraco. **Será invertido** (§4, A-20). |
| Pontos de entrada de XML | `compras::importar_xml` (estoque, custo, contas a pagar) e `importar::…` via `ler_cadastro` (pessoas e peças; aceita NFC-e). |
| `pagar::duplicatas_da_compra` | Relê o XML guardado; não valida (o que está guardado já foi aceito). |
| `tests/nfe_exemplo.xml` | Nota sintética; assinatura de mentira. Vários testes de compras, encomendas e importação dependem dela. |

## 2. O que medi nos seus 541 XMLs reais (protótipo em Python, só para decidir o desenho)

| Fato | Resultado | Consequência no desenho |
|---|---|---|
| Algoritmos | 539/539: c14n 1.0 inclusivo (`xml-c14n-20010315`), transformações `enveloped-signature` + c14n, **SHA-1** no resumo, **RSA-SHA1** na assinatura | Aceitar só essas URIs fixas (e o par SHA-256 equivalente, §3.3). |
| Resumo e assinatura recalculados | **539/539 conferem** (resumo do `infNFe` canonicalizado e RSA sobre o `SignedInfo` canonicalizado) | A solução é viável e compatível com todos os documentos válidos que o sistema já usa. |
| Certificado do emissor | 535 RSA-2048 (4 RSA-1024, de uma ferramenta de teste "SEM VALIDADE JURÍDICA"); 515 sem `CA:TRUE`; uso de chave com `clientAuth`; só 1 certificado dentro do XML | Cadeia tem de vir de um **repositório de ACs que o Rustinho carrega** (pedido 1). |
| Validade | **346 de 539 certificados já venceram hoje**; 532/532 NF-e estavam **válidos na data de emissão** (`dhEmi`) | A validade se confere **na data de emissão**, nunca "hoje". Senão as notas antigas seriam recusadas. |
| CNPJ do certificado × emitente | NF-e: CNPJ idêntico ou mesma raiz de 8 dígitos em 532/532 | Regra D3 (raiz do CNPJ igual) não recusa nenhum documento real. |
| Assinatura do protocolo | **`protNFe` não traz assinatura nos arquivos distribuídos**: `nProt`, `dhRecbto` e `digVal` podem ser escritos por qualquer um | Provar a assinatura **do emitente** não prova a **autorização da SEFAZ**. Limite declarado (§6-L1); só consulta online à SEFAZ resolve. |
| Tipos misturados | Há CT-e e eventos na pasta (o sistema já recusa), 2 sem assinatura, 4 de teste (fsist) | Devem ser recusados, cada um com mensagem própria. |

## 3. Desenho

### 3.1 Módulo novo `nucleo/src/fiscal/` (puro, sem rede, sem banco)

| Arquivo | Função |
|---|---|
| `c14n.rs` | Canonicalização C14N 1.0 inclusiva, sem comentários, de **um subconjunto** (um elemento no contexto do documento: namespaces herdados no elemento, atributos ordenados, escapes do padrão, `<a></a>` para vazio, `\r\n`→`\n`). Escrito à mão sobre `roxmltree` (já é dependência). Validado contra o libxml2 (lxml) em todo o acervo (§4-A-03). |
| `xmldsig.rs` | Estrutura **estrita** da assinatura da NF-e e verificação: resumo e RSA. Detalhes em §3.2 e §3.3. |
| `cadeia.rs` | Cadeia X.509 de `certificado do emissor` até uma **âncora** (raiz ICP-Brasil): assinatura de cada elo, nomes, `BasicConstraints`/`pathLen`, `KeyUsage` (`keyCertSign`), validade **na data de referência**, identificadores de chave. |
| `ancoras.rs` | Âncoras (raízes ICP-Brasil v1…v13) **embutidas no programa**, identificadas pelo SHA-256 do certificado; ACs intermediárias (do pacote do ITI) em uma lista de "dicas" que só valem se fecharem a cadeia até uma âncora. |
| `mod.rs` | `verificar(xml: &str, ancoras: &Ancoras) -> Result<Prova, Recusa>`; `Prova{ emitente, cnpj_cert, cadeia: Vec<String>, referencia, algoritmo }`. |

Bibliotecas (escolha e alternativas):

| Peça | Escolha | Por quê / alternativas descartadas |
|---|---|---|
| Ler X.509 | `x509-parser` 0.18 (MIT/Apache; sem o recurso `verify`) | Maduro e muito usado. `x509-cert` 0.3 também serve, mas o ecossistema ao redor é menor. |
| RSA PKCS#1 v1.5 (SHA-1/256/384/512) | `ring` 0.17 (`RSA_PKCS1_2048_8192_SHA1_FOR_LEGACY_USE_ONLY`, `…SHA256`, …) | Auditada, tempo constante. `rsa` está só em versão candidata (0.10-rc) e tem aviso conhecido de segurança em operações com chave privada (que não usamos). Efeito: RSA-1024 é recusado (é o caso dos 4 certificados de teste). |
| Resumo | `sha1` (RustCrypto) e `sha2` (já existe) | — |
| XMLDSig pronto (`xmlsec`, `signxml`…) | **Descartado** | Dependem de C (libxmlsec/OpenSSL) ou de Python; difícil de compilar para Windows/Android; superfície de ataque maior que um verificador estrito de um único formato. |
| Cadeia pronta (`rustls-webpki`) | **Descartado** | Pensado para TLS (rejeita SHA-1 em certificados, exige usos de chave de servidor); as ACs antigas da ICP-Brasil usam SHA-1. Uma validação de cadeia curta e própria é mais fácil de provar com testes. |

### 3.2 Estrutura aceita (anti "wrapping" e anti confusão de algoritmo)

O verificador **recusa** (não corrige, não tenta outra leitura) se qualquer item faltar:

1. Documento sem `DOCTYPE`/entidades; tamanho ≤ 5 MB, profundidade ≤ 64 (DoS).
2. Exatamente **uma** `Signature` (namespace `xmldsig#`), filha direta de `NFe`, depois de `infNFe`.
3. Exatamente **uma** `Reference`, com `URI="#<Id do infNFe>"`; **exatamente um** elemento do documento com esse `Id`, e ele é o `infNFe` filho direto de `NFe`.
4. Transformações exatamente `[enveloped-signature, c14n-20010315]`; `CanonicalizationMethod` = `c14n-20010315`; `DigestMethod` e `SignatureMethod` do mesmo par (**sha1/rsa-sha1** ou **sha256/rsa-sha256**).
5. `KeyInfo` com **um** `X509Certificate` (DER válido). `KeyValue`, `X509Data` extra ou certificados a mais: ignorados (nunca usados como chave).
6. **Quem lê os dados é o mesmo nó verificado**: `nfe_xml` passa a receber o `infNFe` já verificado (identidade do nó), nunca fazer nova busca por nome no documento (isso fecharia a porta para um segundo `infNFe` escondido).

### 3.3 Verificação (ordem; para na primeira falha)

1. Estrutura (§3.2). 2. Resumo: C14N do `infNFe` → SHA-1 → Base64 → igual ao `DigestValue` (comparação em tempo constante). 3. Assinatura: C14N do `SignedInfo` → RSA com a chave pública **do certificado do XML**. 4. Certificado: RSA ≥ 2048; `BasicConstraints` não-CA ou ausente; (não obrigar `KeyUsage` além de `digitalSignature` quando presente). 5. Cadeia até âncora, **na data de referência** = `dhEmi` do próprio `infNFe` (está dentro da região assinada; o `dhRecbto` do protocolo não é assinado e **não** é usado). Tolerância: certificado emitido até 48 h depois de `dhEmi` é aceito (assinatura posterior à emissão). 6. **Titular:** a raiz de 8 dígitos do CNPJ do certificado (OID 2.16.76.1.3.3) = a do `emit/CNPJ`; emitente CPF: CPF do certificado (OID 2.16.76.1.3.1) igual ao `emit/CPF`. 7. Ambiente: `tpAmb = 1` (homologação é recusada). 8. Só então o `nfe_xml` extrai os dados e a coerência atual (chave, dígito, protocolo) continua valendo como **verificação adicional**, não como prova.

### 3.4 Onde se aplica e o que fica guardado

* `compras::importar_xml` e a importação de cadastros por XML (`importar.rs`) chamam `fiscal::verificar` **antes** de qualquer escrita. Falha → nenhuma linha gravada (compra, estoque, custo, contas a pagar, pessoa, peça, auditoria de entrada).
* Migração **0020**: `compra.xml_verificacao` (texto: `verificada` | `nao_verificada`) + `compra.xml_verificada_em`. Compras já existentes recebem `nao_verificada` (nada é apagado ou alterado; só é marcado).
* Tela: o aviso fixo `LIMITE_DA_CONFERENCIA` **sai**. Na compra aparece **uma** linha discreta só quando a nota é `nao_verificada` ("Importada antes da verificação da assinatura"). A recusa aparece no ato (mensagem em português simples, por motivo). Sem botão "lançar mesmo assim".
* Gerente › Fiscal (seção existente de códigos/ajustes; sem tela nova): "Autoridades certificadoras: pacote de DD/MM/AAAA, N certificados" e um botão **"Atualizar pacote de ACs…"** (escolhe o arquivo do ITI). O arquivo só vale se cada AC fechar a cadeia até uma raiz embutida; nunca adiciona raiz.
* O pacote entregue de fábrica vem embutido (`include_bytes!`) com a data; atualizar é para ACs novas depois da versão.

## 4. Testes de aceitação (escritos **antes** do código; resultado simulado ≠ resultado em equipamento real)

**Material:** (a) acervo real do dono (532 NF-e; **fora do repositório**, caminho por variável de ambiente, testes `#[ignore]`); (b) infraestrutura de teste **só para teste**: raiz, AC e emitente fictícios gerados uma vez com `openssl`, XML de exemplo (`nfe_exemplo.xml`) reassinado por ela, guardados em `nucleo/tests/pki/` (nomes "SEM VALIDADE – SÓ TESTE"); em teste, as âncoras são **injetadas** (`Ancoras` por parâmetro e, nos testes de alto nível, um substituto só em `#[cfg(test)]`); o programa final usa só as embutidas; (c) o libxml2 (lxml) como **oráculo** da C14N.

| ID | Teste | Esperado |
|---|---|---|
| A-01 | Acervo real: as 532 NF-e, com o pacote do ITI | **532 aceitas** (compatibilidade). |
| A-02 | Acervo real sem o pacote de ACs | Recusa "autoridade certificadora desconhecida" (nunca aceita por exceção). |
| A-03 | C14N do `infNFe` e do `SignedInfo` de 539 documentos × libxml2 | **Bytes idênticos** nos 539. Mais casos sintéticos: atributos fora de ordem, namespaces repetidos, `&#xD;`, `]]>`, `>` e `"` em texto/atributo, vazio, espaços e CRLF, prefixos `ns:`. |
| A-04 | Documento de teste assinado pela PKI de teste | Aceita; `Prova` traz emitente, cadeia e data. |
| A-05 | Altera `qCom` de um item, **mantendo `DigestValue`, `digVal`, chave, protocolo** | Recusa "o conteúdo não é o que foi assinado". |
| A-06 | Altera `vUnCom`/`vProd`/`vNF` mantendo todos os resumos | Recusa. |
| A-07 | Altera `xProd` (descrição) mantendo todos os resumos | Recusa. |
| A-08 | Altera `cProd`, `NCM`, `CFOP`, CNPJ/IE do emitente, duplicata (`dup`), 1 byte qualquer dentro do `infNFe` | Recusa (um teste por campo). |
| A-09 | Altera o **resumo** para casar com o conteúdo adulterado (recalcula SHA-1) mas não a assinatura | Recusa "assinatura inválida". |
| A-10 | Troca `SignatureValue` por outro válido (de outra nota) | Recusa. |
| A-11 | Troca só o certificado dentro do XML por outro ICP válido (de outra empresa) | Recusa (assinatura não confere com essa chave). |
| A-12 | Reassina a nota adulterada com **certificado próprio** (autoassinado) | Recusa "cadeia não chega à ICP-Brasil". |
| A-13 | Reassina com certificado de uma AC **fictícia** que se diz "ICP-Brasil" (mesmo texto no nome) | Recusa (âncora é por SHA-256 do certificado, não por nome). |
| A-14 | Cadeia quebrada: AC intermediária ausente / assinatura da AC corrompida / AC de outra raiz | Recusa, um motivo por caso. |
| A-15 | Certificado fora da validade **na data de emissão** (vencido antes / emitido depois > 48 h) | Recusa. |
| A-16 | Certificado vencido **hoje**, mas válido na emissão (como 346 do acervo) | **Aceita.** |
| A-17 | AC intermediária usada com `CA:FALSE`, ou sem `keyCertSign`, ou `pathLen` estourado | Recusa. |
| A-18 | Certificado de titular com CNPJ de **outra raiz** | Recusa "certificado não é do emitente". Mesma raiz, filial diferente: aceita. Emitente CPF: confere o CPF. |
| A-19 | `tpAmb = 2` (homologação), mesmo assinado | Recusa. |
| A-20 | Teste `limite_conhecido_xml_adulterado…` **invertido** | O adulterado antes aceito agora é **recusado**; o aviso fixo some. |
| A-21 | Wrapping: dois `infNFe`, dois `Id` iguais, `Reference` apontando para outro elemento, `Signature` em outro lugar, duas `Signature`, `infNFe` não-filho de `NFe` | Recusa em todos (um teste por variação); os dados lidos são do nó verificado. |
| A-22 | Confusão de algoritmos: `rsa-sha1`↔`sha256` trocados, `hmac-sha1`, `none`, transformações extras (XPath, c14n exclusivo), c14n com comentários | Recusa. |
| A-23 | Malformados: vazio, truncado, não-XML, binário, UTF-8 inválido, BOM, `DOCTYPE` com entidade (bomba de expansão), 6 MB, profundidade 200, `X509Certificate` com Base64/DER inválido | Recusa limpa, sem pânico e em tempo curto (< 2 s). |
| A-24 | **Atomicidade:** para cada recusa acima, via `compras::importar_xml`: contagem de linhas em `compra`, `compra_item`, `estoque`/movimentos, custo médio, `conta_pagar`, `auditoria`, `alteracao` (log de sincronização) **antes = depois** | Nada muda. |
| A-25 | Importação por XML de cadastro (`importar.rs`), mesmos casos | Nada é cadastrado; a lista de recusas mostra o motivo. |
| A-26 | Notas válidas importadas depois da mudança continuam gerando: itens, custo, duplicatas, `xml_verificacao = verificada` | Igual ao comportamento atual. |
| A-27 | Migração 19 → 20 com compras existentes | Marcadas `nao_verificada`; nada apagado; `pagar::duplicatas_da_compra` continua lendo. |
| A-28 | Cópia da nota que já foi importada (chave igual) | Mantém a recusa por duplicidade atual (ordem: verificação, depois duplicidade). |
| A-29 | Pacote de ACs atualizado: com AC nova que fecha a cadeia | A nota que era recusada por "AC desconhecida" passa. |
| A-30 | Pacote adulterado: tenta adicionar **raiz** nova ou AC que não fecha | Recusado; pacote anterior continua valendo. |
| A-31 | Tempo: 532 notas verificadas em sequência | Total e por nota registrados (meta: < 50 ms por nota em modo final). |
| A-32 | Regressão: toda a suíte anterior (256 + integração) em desenvolvimento e programa final | 0 falhas. |
| A-33 | Dependências: `cargo tree` mostra as bibliotecas novas; `ring` compila para Linux (alvo disponível); Windows/Android **não testados aqui** | Registrado (lacuna L5). |

**Mutações planejadas** (defeitos plantados, cada um tem de ser pego): ignorar o resumo; comparar só os 20 primeiros caracteres; aceitar sem conferir a assinatura; usar a chave de outro lugar do XML; aceitar certificado sem cadeia; aceitar qualquer âncora pelo nome; usar a data de hoje em vez de `dhEmi`; não conferir `CA:TRUE` na AC; não conferir `pathLen`; não conferir a raiz do CNPJ; aceitar `tpAmb=2`; ler os dados de um segundo `infNFe`; aceitar `Reference` para outro `Id`; aceitar transformações extras; gravar a compra antes de verificar; manter o aviso fixo; aceitar RSA-1024; trocar a ordem de c14n (atributos); aceitar pacote com raiz nova; `nao_verificada` virar `verificada` na migração.

## 5. Decisões que peço (com a recomendação)

| ID | Decisão | Recomendação |
|---|---|---|
| D1 | Sem botão "lançar mesmo assim" para nota recusada (a compra continua podendo ser **digitada** à mão) | Sim, sem exceção. |
| D2 | Importação de **cadastros** por XML também exige assinatura válida | Sim (preço e descrição de peça adulterados contaminam o catálogo). |
| D3 | Titular: raiz do CNPJ do certificado = raiz do CNPJ do emitente | Sim (vale 532/532 no seu acervo). |
| D4 | Homologação (`tpAmb=2`) recusada | Sim. |
| D5 | Compras antigas ficam `nao_verificada`, sem bloquear nada, com uma linha discreta na compra | Sim. |
| D6 | Cadastro de **CPF** (produtor rural) entra na verificação de titular; se o certificado não trouxer CPF, recusa | Sim. |

## 6. Limites que continuam depois da implementação (não escondidos)

* **L1 — Autorização da SEFAZ não é provada.** O `protNFe` (`nProt`, `dhRecbto`, `digVal`) **não é assinado** nos arquivos distribuídos. Provamos "o emitente assinou este conteúdo", **não** "a SEFAZ autorizou". A prova só vem de **consultar a SEFAZ pela chave** (exige internet e, nos serviços oficiais, certificado do usuário) — fora desta pendência; fica registrada como PEND-15 caso queira.
* **L2 — Revogação.** Sem internet, não se consulta a lista de certificados revogados (LCR). Um certificado revogado **depois** da emissão passa. Atualizar LCR por pacote seria uma etapa futura.
* **L3 — SHA-1.** O padrão da NF-e 4.00 obriga SHA-1 no resumo e RSA-SHA1 na assinatura; colisão escolhida de prefixo é possível em teoria e cara. Não há como evitar sem mudança do padrão (aceitamos também SHA-256 se a SEFAZ migrar).
* **L4 — Âncoras.** A confiança nasce do pacote do ITI baixado pelo dono e das raízes embutidas. Se uma raiz for comprometida/revogada, é preciso **atualizar o programa**.
* **L5 — Plataformas.** Só Linux foi compilado e testado aqui; `ring` para Windows e Android (32 e 64 bits) precisa ser conferido (validação física/CI futura).
* **L6 — Notas sem emitente verificável** (CPF sem OID no certificado, certificados de procuração) serão recusadas; pode haver casos reais raros a tratar quando aparecerem.
* **L7 — O que fica fora:** CT-e, eventos e NFC-e de venda (só leitura de cadastro pela NFC-e passa pelo mesmo verificador); DANFE em PDF não é assinado e continua sem validade como prova.

## 7. Etapas, modelo e esforço

| Etapa | O que | Modelo / esforço |
|---|---|---|
| E1 (esta) | Pesquisa, medição no acervo, desenho e testes de aceitação | Sonnet 5.5, alto |
| E2 | Seu aval às decisões D1–D6 e envio do pacote de ACs (pedidos 1 e 2) | Você |
| E3 | Testes A-xx primeiro (falhando); PKI de teste; C14N; xmldsig; cadeia; migração 0020; telas | **Opus 5.5, alto** |
| E4 | Mutações, suíte completa (desenvolvimento e final), clippy, inspeção visual, entrega | Sonnet 5.5, médio |
| E5 | Validação em Windows/Android (L5) | Você, com os aparelhos |
