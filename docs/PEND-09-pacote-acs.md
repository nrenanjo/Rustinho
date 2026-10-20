# PEND-09 · Relatório de obtenção e exame do pacote oficial de ACs da ICP-Brasil

**Data:** 20/10/2026. **Nada de código foi alterado; nada foi enviado ao remoto.** Os arquivos ficam guardados localmente, fora do repositório (pasta de evidências da sessão), até você aprovar o desenho.

## 1. Origem e verificação

| Item | Resultado |
|---|---|
| Origem oficial | Repositório do ITI (página "Certificados das ACs da ICP-Brasil – Arquivo Único Compactado", atualizada em 26/08/2026). Arquivos em `https://acraiz.icpbrasil.gov.br/credenciadas/CertificadosAC-ICP-Brasil/`. |
| Quem baixou | **Você**, no navegador (aba anônima), em 20/10/2026. Meu ambiente não alcança esse servidor (proxy responde `403`); portanto **eu não baixei nada**. A URL das capturas de tela tem o sufixo `?utm_source=chatgpt.com`, que é só rastreio do link; o domínio é o oficial. |
| `ACcompactado.zip` ("Cadeia Vigente") | 351.321 bytes. SHA-512 calculado por mim = SHA-512 publicado no `hashsha512.txt` → **confere**. |
| `ACcompactadox.zip` ("Cadeia Completa", inclui expiradas) | 624.621 bytes. SHA-512 calculado = SHA-512 publicado no `hashsha512x.txt` → **confere**. |
| Ressalva honesta | Os valores publicados foram lidos **das suas capturas de tela** dos dois arquivos `.txt` (eu não consegui abrir os `.txt`). Como os 128 caracteres hexadecimais batem integralmente com os que calculei, a coincidência por engano é impraticável. Para um registro à prova de dúvida, guarde os dois `.txt` originais junto. |
| Integridade dos zips | `testzip` sem erro; 0 nomes de arquivo suspeitos (sem `..` nem caminho absoluto); só extensão `.crt`. |

SHA-512 (copiados dos arquivos conferidos):
* `ACcompactado.zip`  `4585a99955607525e475cf22138302fe8ddce6ca8f0926cd1f01809f007d4bcddff5c1070369d29b1de99c0d2eb058ce0deb856bae0189469ba37e09a59d7985`
* `ACcompactadox.zip` `48a9ed4bd1e2a81bf31e00b981501854b979cdee4eb402c8da658184ab6bd55f2522863ff1e7a97c18159dea73f21ae080f299613e2ef4413a548483b50b5b56`

## 2. Conteúdo

| | Vigente | Completa |
|---|---|---|
| Arquivos `.crt` | 180 | 340 (339 certificados lidos; 334 distintos por SHA-256; 4 pares duplicados, um deles com nome `…_retirar_repositorio`) |
| Raízes autoassinadas | 6 (v5, v6, v7, v10, v11, v12) | 12 (as 6 acima + raiz original 2001, v1, v2, v3, v4, v13) |
| Assinaturas | 175 RSA-4096/SHA-512, 4 Ed448, 1 pós-quântico (OID 1.3.6.1.4.1.44588.2.1) | 266 RSA-4096/SHA-512, 63 RSA-2048/**SHA-1** (cadeia antiga), 5 ECDSA-SHA512, 4 Ed448, 1 pós-quântico |
| Não lido | — | `Autoridade_Certificadora_do_Serpro_Final_v1.crt`: DER fora do padrão (campo `critical` com o valor padrão codificado), cadeia antiga do Serpro; **não afeta nenhuma nota do seu acervo** |

ACs sob cada raiz (certificados RSA verificáveis aqui): v5 = 138 (vigente) / 142 (completa); v12 = 23; v2 = 80; v1 = 41; raiz original = 20; v10 = 9–10; v6 = 3; v4 = 1; v13 = 1; v11 = 1.

## 3. Raízes: status segundo o ITI e uso previsto

*Fonte do status: página "Repositório AC-Raiz" do ITI (lida em 20/10/2026). Datas e algoritmos: medidos nos próprios certificados.*

| Raiz | Validade | Algoritmo | Status no ITI | Proposta para o Rustinho |
|---|---|---|---|---|
| Raiz original (2001) | 2001–2011 | RSA-2048 SHA-1 | expirada | **Não incluída** (sem uso demonstrado; precisaria de SHA-1 em certificado). Reavaliar só com nota real que a exija. |
| v1 | 2008–2021 | RSA-2048 SHA-1 | expirada | **Histórica, condicional:** só com o adaptador SHA-1 controlado (§5 do desenho) e só se a data de emissão estiver dentro da validade da raiz. Fora disso: recusa com motivo claro. |
| v2 | 2010–2023 | RSA-4096 SHA-512 | expirada | **Histórica:** vale **apenas** para notas emitidas dentro da validade (1 nota do seu acervo, de 2022, depende dela). |
| **v3** | 2010–2023 | ECDSA P-521 | **REVOGADA em 26/02/2014** | **Nunca.** Está no pacote completo; fica **fora** da lista de âncoras. |
| v4 | 2015–2035 | ECDSA | vigente | Reconhecida; **sem suporte inicial** (algoritmo). Nenhuma nota do acervo precisa. |
| **v5** | 2016–2029 | RSA-4096 SHA-512 | vigente | **Âncora principal** (527 de 528 notas reais). |
| v6 | 2018–2038 | Ed448 | vigente | Reconhecida; **sem suporte inicial** (algoritmo). |
| v7 | 2018–2038 | pós-quântico | vigente | Reconhecida; **sem suporte inicial**. |
| **v8, v9** | — | — | **REVOGADAS em 18/12/2018** | **Nunca** (não estão nem no pacote). |
| v10 | 2019–2032 | RSA-4096 SHA-512 | vigente (**SSL**) | **Nunca para NF-e** (finalidade: SSL). Teste feito: com só v10/v11 como âncoras, **0 de 532** notas validam. |
| v11 | 2019–2032 | RSA-4096 SHA-512 | vigente (**assinatura de código**) | **Nunca para NF-e** (finalidade). |
| **v12** | 2024–2037 | RSA-4096 SHA-512 | vigente | **Âncora** (RSA; 23 ACs; nenhuma nota do acervo ainda). |
| v13 | 2025–2045 | ECDSA | vigente | Reconhecida; **sem suporte inicial** (algoritmo). |

Critério único de âncora: (1) raiz da ICP-Brasil do pacote verificado; (2) não revogada; (3) finalidade compatível com assinatura de documento (exclui v10 SSL e v11 código); (4) algoritmo que o verificador suporta; (5) validade conferida **na data de emissão** da nota (histórica só vale para o passado); (6) identificada pelo **SHA-256 do certificado inteiro**, não pelo nome. Certificado encontrado dentro do XML **nunca** vira âncora nem AC intermediária.

## 4. O que as notas do seu acervo exigem

Das 532 NF-e do acervo, **528 têm cadeia completa até uma raiz do pacote** e 4 não (as 4 são de teste "www.fsist.com.br (SEM VALIDADE JURÍDICA)", autoassinadas, RSA-1024). Todas as 528 estavam válidas na data de emissão.

* **19 ACs emissoras** (o que assina o certificado da empresa) + **8 ACs de 1º nível** + **2 raízes** (v5 e v2) = **29 certificados** bastam para o acervo.
* **19 caminhos distintos**, sempre `certificado da empresa → AC emissora → AC de 1º nível → raiz`. Os mais comuns: AC Certisign RFB G5 (197 notas), AC SAFEWEB RFB v5 (101), AC SyngularID Múltipla (67), AC LINK RFB v2 (45), AC VALID RFB v5 (26) — todos via "AC Secretaria da Receita Federal do Brasil v4" ou AC do próprio fornecedor, sob a raiz v5.
* **Pacote vigente sozinho:** fecha 527 de 528; a nota que falta é a que usa a raiz v2 (expirada, só no pacote completo). **Por isso o pacote da Cadeia Completa é necessário** para validar notas históricas.
* As 19 ACs emissoras são todas RSA/SHA-512, `CA:TRUE` e `keyCertSign`; só uma tem `pathLen` (0: AC VALID BRASIL v5). As duas raízes usadas (v5 e v2) são RSA-4096/SHA-512. Os dados completos (validade, algoritmo, SHA-256 de cada AC) vão na pasta de evidências (`acs_usadas.json`).

## 5. Teste de viabilidade com biblioteca de validação de cadeia (descartável, fora do projeto)

Usei `rustls-webpki` 0.103 (com `ring`), âncoras = só as raízes permitidas, ACs do pacote como intermediárias, **data de referência = `dhEmi`** de cada nota, uso `clientAuth`:

| Variante | Aceitas | Recusadas |
|---|---|---|
| A. âncoras v5 + v2, data de emissão | **528** | 4 (as de teste fsist: emissor desconhecido) |
| B. só v5 | 527 | 5 (a da raiz v2 também é recusada) |
| C. v5 + v2, mas data = **hoje** | muitas recusadas (certificados vencidos) | prova que a data de referência importa |
| D. âncoras v10 + v11 (SSL/código) | **0** | 532 |

Conclusão: a validação de cadeia pode ser feita por uma biblioteca madura, sem escrever lógica própria de cadeia. Limitação já vista: a biblioteca **não aceita assinatura SHA-1 em certificado** (cadeias das raízes v1 e original), tratada no desenho.

## 6. Pendências deste item

* Guardar os dois `.txt` de hash originais (recomendado).
* Dizer se prefere que o pacote seja embutido no programa como estão os dois zips (verificados) ou só os certificados escolhidos (29 + âncoras) — recomendação: **embutir o pacote completo verificado**, e só as âncoras da lista da §3 valem como âncora.
* A atualização do pacote dentro do programa (Gerente › Fiscal) continua como no desenho: nenhuma raiz nova entra por ali.
