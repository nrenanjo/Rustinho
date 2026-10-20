# Módulo fiscal · Arquitetura de assinatura independente (A1, A3 de pessoa jurídica, Selo Eletrônico)

**Status:** proposta de arquitetura, plano de transição e testes de aceitação (20/10/2026). **Nada foi implementado. Nenhum mecanismo é declarado como suportado.** Versão 0.9.11; só commits locais.
**Princípio:** a lógica fiscal (montar documento, numerar, tributar, transmitir, contingência) **não sabe** como a assinatura é produzida. Ela pede "assine este resumo, com esta política" a uma porta; quem responde pode ser um arquivo A1, um token, uma estação na loja, um HSM ou um serviço de confiança. Trocar de mecanismo = trocar um adaptador e um perfil de configuração, não reescrever o módulo.

## 0. Ponto de partida (lido no repositório)

* O Rustinho **hoje não emite** NF-e, NFC-e, NFS-e nem MDF-e: o Manual mostra "em breve" e as notas seguem no sistema atual. Este é, portanto, o desenho da **emissão futura**; a verificação de XML recebido é o PEND-09 (já desenhado) e **reutiliza** as mesmas peças de política.
* Os caixas (computadores e celulares) já sincronizam entre si por rede local com um canal cifrado e autenticado pela chave da loja (PEND-10). Isso é a base para a "estação assinadora" (§3, topologia T2).

## 1. Regras vigentes: o que está confirmado e o que não está

Confiança: **P** = texto da norma lido (reprodução em site de legislação; conferir no DOU/ITI antes de decidir contrato); **C** = site comercial/blog (indício, **não** fonte oficial); **N** = **não verificado**.

| # | Fato | Confiança | Fonte |
|---|---|---|---|
| 1 | Resolução CG ICP-Brasil **211/2024** (assinada em 31/10/2024; publicada no DOU em novembro/2024; vigente desde a publicação): cria os certificados **SE-S** e **SE-H** (Selo Eletrônico em software e em hardware) e **AE-S/AE-H** (aplicações específicas); **extingue os tipos A1 e A2** de assinatura. | P | normasbrasil.com.br (Resolução 211/2024), Arts. 2, 3, 17 |
| 2 | **Selo Eletrônico** = garante **origem e integridade** de documento emitido por pessoa jurídica; **só para pessoa jurídica**; **vedado** usá-lo para manifestar vontade (isso segue com o e-CPF do representante); CNPJ no campo `serialNumber`; responsável pelo uso identificado por **biometria** em posto credenciado; SE-S = repositório em software protegido por senha/biometria; **SE-H = mídia criptográfica de hardware homologada pela ICP-Brasil ou certificada pelo INMETRO** (a norma lida não trata de HSM remoto/nuvem). Validade: SE-S 1 ano; SE-H 5 anos. | P | idem, Art. 4 e itens 1.1.7, 3.2.9.7, Tabelas 4 e 6 do DOC-ICP-04 |
| 3 | **A3 e A4 passam a ser só de pessoa física** (item 1.1.6 do perfil novo). | P | idem |
| 4 | **Cadeia V5:** A1, A2, A3 e A4 **podem ser emitidos e usados** nos perfis anteriores **até 2 de março de 2029**. **Cadeia V10:** emissão de A1 e A3 até **31/12/2026**, só para aplicações específicas. | P | idem, Arts. 13 e 14 |
| 5 | Consequência para o **e-CNPJ A3 que você já tem**: se ele é da cadeia V5, a norma permite seguir usando-o **até o fim do seu prazo de validade e, no máximo, até 02/03/2029**. A confirmar no **seu** certificado (cadeia, data de validade). | P + inferência | idem; a conferir no certificado |
| 6 | Prazos de mercado ("fim da emissão de A3 PJ de 3 anos em 28/02/2026", "eliminação de OIDs do SAN em 31/12/2028", "metas de adesão ao Selo 10/30/80/100%") | **C** | blogs de fornecedores (um deles com inconsistências internas) |
| 7 | Selo "pode ser usado em nuvem por Prestador de Serviço de Confiança (PSC)", "um Selo atende vários sistemas" | **C** | blogs de ACs/fornecedores |
| 8 | **PAA (Provedor de Assinatura e Autorização)**, Nota Técnica **2026.001**: modelo em que o **provedor** assina e transmite pelo contribuinte (assinatura híbrida: RSA avançada gerada pelo Fisco + assinatura ICP-Brasil **do provedor**); vínculo no portal da SVRS com gov.br prata/ouro; **não disponível para regime normal** (exceto produtor rural), pensado para MEI, Simples e produtor rural; datas de homologação/produção **adiadas** (v1.02, 09/06/2026) | **C** (resumos de artigos; não li a NT) | blogs |
| 9 | **O Selo Eletrônico é aceito pelos serviços da SEFAZ (assinatura do XML e/ou autenticação TLS do cliente)? Desde quando? Em quais UFs? Cadeia V12 aceita nos autorizadores?** | **N** | **Nenhuma fonte oficial encontrada.** Nem o Manual de Orientação do Contribuinte nem nota técnica específica foram localizados. **Não presumir.** |
| 10 | Mecanismos para **TLS mútuo** (certificado do cliente) com certificado de nuvem ou de Selo | **N** | A autenticação TLS exige operação com a chave privada em cada conexão; depende do provedor oferecer um módulo/agente local compatível. A validar. |
| 11 | Preços (token, cartão, leitora, Selo, PSC, HSM) | **N** | Não pesquisados em fonte confiável; **cotação com a AC** é requisito (§6). |

**Perguntas para você que mudam o desenho** (§8): regime tributário (afeta PAA); modelos a emitir (55, 65, NFS-e, MDF-e); cadeia e validade do seu e-CNPJ A3; AC e modelo do token; sistemas operacionais dos computadores; volume diário de documentos.

## 2. Os mecanismos **não** são equivalentes

| Mecanismo | Onde fica a chave | Pode copiar para cada aparelho? | Assina XML | TLS mútuo com SEFAZ | Situação normativa (acima) | Risco principal |
|---|---|---|---|---|---|---|
| **A1** (arquivo `.pfx`/PKCS#12) | Arquivo (+ senha) | **Sim, tecnicamente** (e é exatamente o risco) | Sim, no próprio programa | Sim | Extinto como tipo novo; cadeia V5 até 02/03/2029 (**N/P** conforme cadeia) | Chave exportável: roubo do arquivo = fraude fiscal |
| **A3 de PJ (token USB / cartão)** — o seu | **Dentro do dispositivo; não sai** | **Não** | Sim, via PKCS#11 ou CNG/minidriver, **no computador onde o token está** | Sim, com a chave do token na conexão (adaptador TLS próprio) | Permitido na cadeia V5 até 02/03/2029; sem emissão nova de PJ | Depende de **um** aparelho com o token e de PIN; drivers por fabricante |
| **SE-S** (Selo em software) | Repositório em software | Como o A1 | Sim | Sim (se aceito: **N**) | Vigente (P); aceitação fiscal **N** | Igual ao A1; validade curta (1 ano) |
| **SE-H local** (token/cartão/HSM) | Hardware | **Não** | Sim, como o A3 | Sim (se aceito: **N**) | Vigente (P); aceitação fiscal **N** | Mesmo modelo operacional do A3 |
| **Selo/A3 em nuvem (PSC)** | HSM do provedor | **Não** (chave nunca sai do provedor) | Via API do provedor | **N** (exige agente/módulo compatível) | Norma lida não trata (**N**) | Dependência de internet e do fornecedor; custo recorrente |
| **PAA** | Chaves do provedor/Fisco | Não se aplica | O provedor assina | Provedor transmite | NT 2026.001 (C); **fora do regime normal** | Não serve a todos; prazos adiados |

Regra do desenho: **nenhum adaptador é tratado como substituto automático de outro.** Cada um declara suas **capacidades** (assina XML? TLS mútuo? exige PIN por assinatura? funciona offline?) e a lógica fiscal só usa o que a capacidade declara.

## 3. Arquitetura proposta (portas e adaptadores)

```
 Regras fiscais (montar, numerar, tributar, eventos, contingência)   ← sem dependência criptográfica
              │  pede: "assinar este resumo com a política P"        pede: "abrir canal TLS com o SEFAZ"
              ▼                                                      ▼
   ┌───────── PORTA  Assinante ──────────┐           ┌──── PORTA  CanalAutenticado ────┐
   │ identidade() · capacidades()        │           │ conectar(url) -> fluxo TLS      │
   │ assinar(resumo, algoritmo, política)│           └──────────────┬──────────────────┘
   └──────┬──────────┬───────────┬───────┘                          │ (usa a MESMA credencial)
          │          │           │
   A1 arquivo   PKCS#11    Estação na rede   Nuvem (PSC)    CNG/Windows    (adaptadores)
   (PFX)        (token,    (outro aparelho   (API do        (minidriver)
                cartão,     da loja assina)   provedor)
                HSM)
```

**Camadas (crates/módulos separados, dependência só para baixo):**

1. `fiscal-regras`: documentos, XML, tributação, numeração/séries, contingência, eventos. **Não importa nada de criptografia**; recebe `&dyn Assinante` e `&dyn CanalAutenticado`.
2. `fiscal-politica`: **perfil de assinatura por versão do leiaute**, em dados (não em código): algoritmos aceitos (hoje RSA-SHA1/SHA-1 no leiaute 4.00; SHA-256 se o leiaute migrar), tipos de certificado aceitos, **regra de titular** (hoje: CNPJ no OID 2.16.76.1.3.3; **também** CNPJ em `serialNumber` do Selo), âncoras (as do PEND-09), janela de validade. Mudar a norma = nova linha no perfil.
3. `fiscal-assinatura`: a **porta** `Assinante` (sem segredo no tipo: só `assinar`) e os adaptadores. `Credencial` = identidade pública (cadeia, titular, validade, tipo `A1|A3|SE-S|SE-H|PSC`, capacidades), **sem** chave.
4. `fiscal-transporte`: cliente HTTPS/SOAP; a autenticação do cliente usa a **mesma** `Credencial` por um assinador de handshake (o `rustls` aceita chave de assinatura própria, o que permite usar token ou estação). Se a credencial **não** declara TLS mútuo (ex.: nuvem sem agente), o transporte usa outra rota (PAA, ou estação com token) em vez de falhar no meio da emissão.
5. `fiscal-verificação`: o PEND-09 (mesma política, mesmas âncoras); **a extração do titular passa a ser plugável**, para aceitar o CNPJ do Selo.

**Segurança transversal:** a chave privada **nunca** entra no núcleo, no banco, na sincronização, nos backups nem nos registros; o **PIN não é gravado em disco** (pedido na abertura da sessão, mantido só em memória, apagado ao encerrar); toda assinatura gera linha de **auditoria** (quem pediu, qual documento, resumo, credencial, hora, resultado); limite de assinaturas por minuto; o adaptador devolve erros **tipados** (sem token, PIN bloqueado, PIN errado com contagem de tentativas restantes, certificado vencido, mecanismo não suportado), e o PIN **nunca** é repetido automaticamente (bloquear o token por erro de PIN repetido é um risco real).

**Topologias (escolhidas por configuração, não por código):**

| Topologia | Descrição | Quando |
|---|---|---|
| **T1** | Um computador com o token faz tudo | Loja de um só caixa |
| **T2 (recomendada para A3/SE-H local)** | **Estação assinadora**: um computador da loja, com o token, **recebe pedidos** dos outros aparelhos (computadores e celulares) **pelo canal cifrado já existente da loja**, assina, transmite e devolve o resultado. A chave nunca vai para os outros aparelhos. | Vários caixas, token único |
| **T3** | A1 em cada caixa | **Não recomendada** (copia a chave; só enquanto o A1 for permitido e com arquivo criptografado) |
| **T4** | Serviço de confiança (PSC) | Se o provedor oferecer API/agente validados (**N**) |
| **T5** | PAA | Só se o seu regime permitir (**N**: depende do seu regime) |

**Pontos de projeto da T2:** a estação é um ponto único de falha e precisa de **política de contingência** definida pela norma (**N**: conferir no Manual do Contribuinte as regras de contingência para NF-e e NFC-e com a estação fora do ar); o pedido de assinatura leva o documento **e o resumo**, a estação **recalcula** o resumo (não assina cegamente um valor recebido); numeração e série pertencem a **uma** autoridade (hoje há o conceito de faixas por dispositivo), definida na etapa de numeração, não aqui; o celular **não** é anfitrião de token (adaptadores de token em Android dependem do fabricante e da conexão USB-OTG/NFC; **não prometer**).

## 4. Compatibilidade a validar (nada declarado)

| Item | Linux | Windows | Android 32/64 | Observação |
|---|---|---|---|---|
| A1 (PFX) | a validar | a validar | a validar | Mais simples; o mais perigoso para as chaves |
| A3 por PKCS#11 (`cryptoki`) | módulo do fabricante + `p11-kit`/OpenSC | módulo do fabricante | **não como anfitrião** | Assinatura de XML em RSA-SHA1: o token precisa suportar o mecanismo (algumas só fazem RSA PKCS#1 sobre `DigestInfo`, o que o adaptador compõe) |
| A3 por CNG/minidriver | — | a validar | — | Caminho comum no Windows quando o fabricante entrega minidriver |
| TLS mútuo com chave de token | a validar | a validar | — | Tokens antigos só fazem PKCS#1 v1.5; TLS 1.3 exige PSS; os serviços da SEFAZ usam TLS 1.2 (**a confirmar**) |
| Estação na rede (T2) | a validar | a validar | cliente apenas | Reusa o canal da loja |
| Nuvem/PSC | a validar | a validar | a validar | Depende do provedor (**N**) |

Validação **simulada** (SoftHSM2 + AC de teste) e **física** (o seu token A3) são registradas **separadas**.

## 5. Estratégia de transição (sem presumir que o Selo será aceito)

| Fase | Quando | O que fazer | Critério para avançar |
|---|---|---|---|
| **0. Inventário** | Antes de implementar | Anotar do seu e-CNPJ A3: AC emissora, **cadeia (V5/V10/V12)**, data de validade, modelo do token e driver; conferir se a AC já oferece Selo e quando. | Dados registrados. |
| **1. Emitir com o que existe** | Implementação inicial | Suportar o **A3 por PKCS#11/CNG na estação (T2)**; A1 só se for necessário e permitido. | Aceitação simulada + homologação SEFAZ com o seu token. |
| **2. Preparar o Selo sem depender dele** | Em paralelo | Perfil de assinatura e extração de titular já aceitam o Selo (CNPJ em `serialNumber`); adaptadores iguais (SE-H por token usa o mesmo PKCS#11). **Nenhuma ativação.** | Testes com certificado de Selo **de teste** (AC de homologação, se existir) ou fixture. |
| **3. Prova em homologação** | Quando a AC emitir o Selo | Obter **um** Selo; testar na **homologação** da SEFAZ **todos** os serviços usados: assinatura do XML de NF-e/NFC-e, eventos, consulta, distribuição, TLS do cliente. Registrar UF, data, resultado. | **Aprovação em todos os serviços**, não só na emissão. |
| **4. Operação paralela** | Depois da prova | Cadastrar **duas credenciais** com prioridade e validade; o Selo entra como reserva; piloto em produção com poucas notas. | Sem rejeição por certificado durante o piloto. |
| **5. Corte** | **Antes de 02/03/2029** (meta interna sugerida: 6 meses antes, **opinião, não norma**) | Trocar a prioridade; manter o A3 como reserva até vencer. | — |

Regras: (a) **não renovar nem comprar nada por pressa**; decisão de compra é sua, com cotação; (b) se o Selo **não** for aceito por algum serviço fiscal nas datas necessárias, as alternativas são PAA (se o regime permitir), serviço de terceiros ou emissor homologado; o desenho já as comporta como adaptadores/modos; (c) **a lógica fiscal não muda** em nenhuma fase.

## 6. Dependências e custos recorrentes (categorias; sem valores inventados)

| Item | Natureza | Observação |
|---|---|---|
| Certificado (renovação periódica) | Recorrente | A3/SE-H até 5 anos, SE-S 1 ano (P); **valor: cotar com a AC** |
| Token/cartão e leitora | Eventual (troca/perda) | Compatibilidade com o driver do sistema operacional |
| Selo Eletrônico (emissão) | Recorrente | Exige identificação biométrica do responsável (P); disponibilidade pela AC **a confirmar** |
| PSC/nuvem | Recorrente por assinatura/volume | Só se escolhido; contrato e API do provedor |
| HSM local | Investimento + manutenção | Só para volume alto; improvável aqui |
| PAA | Possível tarifa do provedor | Só se o regime permitir |
| SEFAZ | Sem tarifa de uso dos serviços (**a confirmar**) | — |
| Bibliotecas (software) | Sem custo de licença | `cryptoki` (PKCS#11), `rustls` com assinador próprio, leitura de PKCS#12, `ring`/`webpki` (já previstos no PEND-09), SoftHSM2 só em teste |
| Drivers de token | Em geral gratuitos do fabricante | Por sistema operacional |

## 7. Testes de aceitação (antes de implementar; simulados ≠ físicos)

| ID | Teste | Esperado |
|---|---|---|
| S-01 | **Independência:** o `fiscal-regras` compila **sem** nenhuma dependência de criptografia (verificação em `cargo tree`) e funciona com um assinador falso | Sim |
| S-02 | Trocar o adaptador (A1 de teste → PKCS#11 SoftHSM → estação na rede) **sem** alterar o módulo fiscal | Mesmo XML assinado verificável pelo PEND-09 |
| S-03 | XML assinado por cada adaptador passa em `fiscal::verificar` (resumo, RSA, cadeia da AC de teste, titular) | Passa |
| S-04 | Chave privada **não** aparece no banco, nos backups, na sincronização, nos registros nem na memória descarregada (busca por padrões na saída dos testes) | Não aparece |
| S-05 | PIN: nunca gravado; apagado ao encerrar; PIN errado **não** é repetido; tentativas restantes informadas; token bloqueado gera erro claro | Conforme |
| S-06 | Capacidades: credencial sem TLS mútuo não é usada no transporte; o transporte escolhe outra rota ou falha **antes** de consumir numeração | Conforme |
| S-07 | Estação (T2): pedido de outro aparelho; a estação **recalcula** o resumo; pedido adulterado ou repetido (replay) é recusado; estação fora do ar → o aparelho não perde o documento (fila) | Conforme |
| S-08 | Auditoria de cada assinatura; operador não configura credencial | Conforme |
| S-09 | Perfil de assinatura em dados: trocar RSA-SHA1 por SHA-256 no perfil muda o resultado **sem** mudar código fiscal | Conforme |
| S-10 | Titular: CNPJ por OID (e-CNPJ) **e** por `serialNumber` (Selo) aceitos nas fixtures; CNPJ de outra raiz recusado; Selo usado para "manifestar vontade" não se aplica (documento fiscal é origem/integridade) | Conforme |
| S-11 | Credenciais múltiplas: prioridade e validade; a vencida é pulada; nenhuma disponível → erro tipado, sem emissão parcial | Conforme |
| S-12 | Concorrência: dois pedidos simultâneos ao mesmo token não corrompem a sessão | Conforme |
| S-13 | Falha no meio da assinatura (token removido): nenhuma nota "meio emitida"; numeração tratada conforme a regra de inutilização (**N**: confirmar no manual) | Conforme |
| **F-01** | **Físico:** assinar com o **seu e-CNPJ A3** na homologação e verificar o XML no próprio PEND-09 | A fazer por você |
| **F-02** | **Físico:** TLS mútuo com o token contra a homologação | A fazer |
| **F-03** | **Físico:** estação em Windows e em Linux; computadores e celular como solicitantes | A fazer |
| **F-04** | **Físico (futuro):** Selo SE-H e/ou nuvem: todos os serviços na homologação | Só quando houver o Selo |

**Não declarar suporte:** A1, A3, SE-S, SE-H, nuvem e PAA só serão listados como suportados no Manual **depois** da implementação **e** da validação correspondente (simulada para a lógica; física para o mecanismo).

## 8. Perguntas e decisões

1. **Regime tributário** da FANTI (Simples, Lucro Presumido ou Real)? Define se o PAA é opção.
2. Quais documentos emitir primeiro (NF-e 55, NFC-e 65, NFS-e, MDF-e)? Cada um tem serviço e município/UF próprios.
3. Dados do seu e-CNPJ A3: AC, cadeia, validade, modelo do token (preciso para planejar o driver).
4. **T2 (estação assinadora) como arquitetura-base?** (recomendo)
5. Aceita adiar A1 por completo (só PKCS#11 + estação), já que o tipo está extinto e o risco de cópia da chave é o que você quer evitar? (recomendo)
6. Ordem: **PEND-09** (verificação de XML) → **PEND-13/14** (rede) → **esta arquitetura** (portas e estação) → PEND-15 (consulta à SEFAZ, que **precisa** da porta de credencial/TLS) → emissão. Ou prioridade diferente?

## 9. Etapas, modelo e esforço

| Etapa | O que | Modelo / esforço |
|---|---|---|
| E1 (esta) | Pesquisa de normas e desenho | Sonnet 5.5, alto |
| E2 | Inventário do seu A3 e respostas do §8 | Você |
| E3 | Portas, perfil em dados e adaptador PKCS#11 com SoftHSM2 (testes S-01…S-13 primeiro) | **Opus 5.5, alto** |
| E4 | Estação na rede, TLS por token, telas de credencial | Opus 5.5, alto |
| E5 | Validação física F-01…F-03 | Você; revisão das evidências em Sonnet 5.5, baixo |
| E6 | Selo/nuvem (quando houver aceitação confirmada) | Opus 5.5, alto |
