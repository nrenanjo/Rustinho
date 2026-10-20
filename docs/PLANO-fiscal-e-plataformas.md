# Rustinho 0.9.12 · Plano técnico único: conclusão fiscal, PEND-13/14/17 e tela cinza do Android

**Status:** plano (20/10/2026), **sem código**. Base: commit `f50aa301552e49fd4963af5ce00aec029aa8cd68`. Substitui, na parte de ordem e critérios, o §"Plano de transição" de `docs/FISCAL-assinatura-arquitetura.md`, cuja arquitetura (porta `Assinante`, porta `CanalAutenticado`, perfil em dados, estação de assinatura) continua valendo.

**Regras que valem em todo o plano**
- Duas coisas diferentes, nunca misturadas:
  - **Compras (XML recebido):** provar que a nota **do fornecedor** foi autorizada e não foi cancelada (PEND-15). O Rustinho só **consulta**.
  - **Vendas (emissão própria):** o Rustinho **cria, assina e transmite** NF-e 55 e NFC-e 65, e responde por rejeições, cancelamentos e contingência.
  - Terminar o PEND-15 **não** conclui a parte fiscal.
- **Selo Eletrônico: não suportado** até haver fonte oficial e teste em homologação mostrando que a SEFAZ aceita. Não aparece na tela como opção.
- **A3:** o Rustinho nunca guarda o PIN nem a chave privada. A chave fica no cartão. Quando o leitor ou o sistema operacional pedir o PIN, o pedido é dele (diálogo do Windows ou do middleware); se o Rustinho tiver de repassar o PIN ao cartão (PKCS#11 no Linux), o PIN fica só na memória durante a operação e é apagado em seguida.
- **Tributos em dados, não no código:** regime, CRT, CSOSN/CST, CFOP, alíquotas, IBS/CBS e séries vêm da configuração de cada CNPJ. Nada de "Simples Nacional" fixo.
- **Nada em produção** sem: homologação completa passando, sua autorização expressa e conferência do contador.
- Publicação, releases, apagar artefatos e mexer em credenciais: só com sua autorização (como sempre).

---

## 1. Fatos oficiais que mudam o desenho (verificados em 20/10/2026)

| Fato | Efeito no plano | Fonte |
|---|---|---|
| **SP: CF-e SAT vedado desde 01/01/2026**; NFC-e é o documento do varejo | NFC-e é obrigatória para o balcão da FANTI; não existe plano B por SAT | Portaria SRE 79/2024 (via [Alterdata](https://ajuda.alterdata.com.br/interfacenetbase/netpaf/descontinuidade-do-cf-e-sat)) |
| **SP: contingência off-line da NFC-e** prevista (Portaria SRE 40/2024) | Venda continua sem internet; transmissão depois. O **prazo** de envio em SP ainda precisa ser lido na portaria | idem |
| **NFeDistribuicaoDFe (NT 2014.002 v1.40, jul/2026):** o destinatário recebe só o **resumo** até manifestar (Ciência da Operação); **cancelamento é distribuído mesmo sem manifestação**; por chave: **20 consultas/hora** e só a nota, **sem eventos**; lote por NSU (até 50); após "nenhum documento", esperar **1 hora** (senão rejeição 656 e bloqueio de 1 h); certificado **PJ pela raiz do CNPJ**; documentos dos **últimos 90 dias**; CNPJ passa a **alfanumérico** | É o caminho principal do PEND-15 (ver F3). Limites de consumo viram regra do agendador. | [NT 2014.002 v1.40](https://www.reformatributaria.com/wp-content/uploads/2026/07/NT2014.002_v1.40-WsNFeDistribuicaoDFe-CNPJ-alfa.pdf) |
| **IBS/CBS (NT 2025.002-RTC v1.51, jul/2026):** homologação exige o grupo desde 01/07/2026 **só para CRT 3**; produção **suspensa, "implementação futura"**, tanto para CRT 3 quanto para Simples/MEI | O leiaute deve **ter** o grupo IBS/CBS, ligado por configuração. Para Simples, hoje desligado. Reler a NT na hora de implementar. | [Projeto ACBr, resumo da v1.51](https://www.projetoacbr.com.br/forum/topic/93915-reforma-tribut%C3%A1ria-nt-2025002-v151-suspende-data-fixa-de-obrigatoriedade-do-ibscbs-na-nf-e-e-adia-exig%C3%AAncia-em-produ%C3%A7%C3%A3o/) |
| **QR Code da NFC-e versão 3** (NT 2025.001) | Gerar o QR v3; conferir o cronograma de obrigatoriedade na NT | [NT 2025.001 v1.02](https://inventti.com.br/wp-content/uploads/2025/09/NT2025.001_v1.02-NFCe_qrCode_v3.pdf) |
| **CNPJ alfanumérico** (leiautes de jul/2026) | Cadastros, validação de CNPJ, chave de acesso e o titular do certificado (PEND-09) precisam aceitar letras. **Novo item (F0)**. | NT 2014.002 v1.40 |
| **Certificados A1/A2/S1–S4 inutilizáveis a partir de 03/03/2029** (FAQ do ITI) | O A1 fica suportado só como transição; o caminho principal é o A3 | ITI (citado em `FISCAL-assinatura-arquitetura.md`) |

**Ainda não confirmado (vira teste em homologação, não suposição):**
- Se a consulta de situação (`NFeConsultaProtocolo4`) aceita o certificado do destinatário em todas as UFs dos fornecedores, e se devolve os eventos.
- O prazo de envio da contingência off-line em SP.
- O credenciamento da FANTI para emitir em homologação (é feito no posto fiscal eletrônico da SEFAZ-SP pelo responsável da empresa).

---

## 2. Decisões que dependem de você (ou do contador)

| ID | Pergunta | Bloqueia |
|---|---|---|
| **DF-1** | Regime e CRT da FANTI (você disse que os produtos saem com CSOSN 102; o contador confirma o CRT) | F4 e a emissão (F5/F6) |
| **DF-2** | Qual é o A3: cartão ou token; fabricante e modelo do cartão e do leitor; middleware instalado nos caixas (SafeSign, Watchdata, GD…); validade | F1 |
| **DF-3** | Emissão **própria em Rust** (mais trabalho, roda igual no Windows/Linux/Android) **ou** usar a **ACBrLib** (grátis, você já preferiu antes; madura, mas Windows/Linux, **sem Android**, e a FANTI dependeria de outra biblioteca) | F5 em diante |
| **DF-4** | Em qual caixa o cartão A3 fica (a estação de assinatura); o que fazer quando ele estiver desligado | F1, F6 |
| **DF-5** | CSC (código de segurança) da NFC-e de homologação e de produção (gerado no portal da SEFAZ-SP pelo responsável) | F6 |
| **DF-6** | Séries das notas por caixa (55 e 65), sem conflito com o SINTEC durante a transição | F5, F6 |

---

## 3. Ordem de execução e dependências

```
P0 tela cinza ─┐
P1 PEND-13 (Windows) ──┐
P2 PEND-14 (Android) ──┤
F0 CNPJ alfanumérico ──┤
                       ├─► F1 assinatura A3 + estação ─► F2 canal SEFAZ (mTLS) ─► F3 PEND-15 compras
F4 config. tributária ─┘                                         │
                                                                 ├─► F5 NF-e 55 (homologação)
                                                                 ├─► F6 NFC-e 65 (homologação)
                                                                 └─► F7 eventos, contingência, retomada
                                                                         └─► F8 validação completa ─► F9 piloto em produção
```

P0–P2 e F0 vêm primeiro porque não dependem de certificado e destravam o teste nos aparelhos reais. F4 pode correr em paralelo com F1/F2.

### P0 · Tela cinza do Android (32 bits)
**O que se sabe:**
- O APK de teste abriu no seu celular e ficou com a **tela toda cinza**.
- O teste automático de 32 bits de 17/10 **nunca rodou o app**: a instalação no emulador falhou com `INSTALL_FAILED_NO_MATCHING_ABIS` (o emulador não roda ARMv7), e as fotos daquele ramo são de **outro aplicativo** (YouTube).
- Portanto, o Android de 32 bits **nunca foi visto funcionando** em lugar nenhum.

**Hipóteses (nenhuma confirmada):**
- O desenho pela CPU (forçado em 32 bits) falha nesse aparelho.
- Uma tela QML não carrega (módulo que faltou no APK).
- Falha ao abrir o banco antes da primeira tela.

**Plano:**
- (a) Uma **tela de erro legível**: se a tela principal não carregar, o app mostra o motivo em vez do cinza.
- (b) Um **registro de diagnóstico** salvo numa pasta que você consiga abrir e me mandar, sem precisar de cabo nem `adb`.
- (c) Corrigir a causa.
- (d) Trocar o teste automático de 32 bits por um que **realmente rode ARMv7** e **reprove** quando a instalação falhar.

**Aceitação:**
- O app de 32 bits abre a primeira tela no seu celular.
- O teste automático reprova quando a instalação falha.
- O registro chega até mim.
- Precisa saber: modelo do celular e versão do Android.

### P1 · PEND-13 · Windows: escolher a interface de rede
**Escopo:** prender o envio e o anúncio da sincronização à placa escolhida (`IP_UNICAST_IF` / `IP_MULTICAST_IF`) e mostrar o tipo da interface (Wi-Fi, cabo, virtual) para esconder adaptadores virtuais (VPN, Hyper-V, VirtualBox).

**Testes:**
- Unitários com interfaces simuladas.
- **Compilação para Windows 64 bits** (alvo `x86_64-pc-windows-msvc` ou `-gnu`), de **todo** o programa, incluindo o verificador da NF-e (`ring`). Pelo GitHub, com sua autorização de envio; ou na sua máquina.

**Aceitação:**
- Compila; testes verdes.
- **Em equipamento real** (FV-1…FV-13): dois caixas Windows 10/11 sincronizam pela placa escolhida, mesmo com VPN ou adaptador virtual ligado.
- Windows 32 bits está **fora**: você decidiu só 10 e 11 de 64 bits.

### P2 · PEND-14 · Android: usar a rede Wi-Fi
**Escopo:** prender o socket à rede Wi-Fi pelo `ConnectivityManager` / `Network.bindSocket` (JNI), para o celular não mandar pelo 4G quando o Wi-Fi não tem internet.

**Testes:**
- Compilação **ARMv7 e ARM64**.
- Teste em emulador **ARM64** (que roda de verdade) com Wi-Fi sem internet + dados móveis.

**Aceitação:**
- Os dois APKs compilam; o de 64 bits passa no emulador.
- **Em equipamento real:** o celular sincroniza com um caixa pelo Wi-Fi da loja com os dados móveis ligados.

### PEND-17 · tela de compra no celular
Corrigir os botões sobre o texto na linha do item. **Aceitação:** fotos em largura de celular (≈360 px) e no seu aparelho mostram texto e botões sem sobreposição. Fica junto do P2, porque precisa do Android funcionando (P0).

### F0 · CNPJ alfanumérico
**Escopo:** validação e máscara do CNPJ, chave de acesso de 44 posições com letras, titular do certificado (PEND-09), importação do SINTEC.

**Aceitação:**
- Testes com CNPJ alfanumérico válido e inválido (dígito verificador da regra nova).
- O acervo real continua 528/528.

### F1 · Assinatura com e-CNPJ A3 (PEND-16, parte A3)
**Escopo:** implementar a porta `Assinante` com dois adaptadores:
- **Windows:** CNG/CryptoAPI (o Windows pede o PIN).
- **Linux:** PKCS#11, com a biblioteca do fabricante indicada na configuração (`cryptoki`).

E a **estação de assinatura**: um caixa com o cartão assina para os outros pelo canal cifrado da rede da loja, com fila e confirmação.
- **Android:** **não** lê o cartão; pede a assinatura à estação.
- **A1 (arquivo .pfx):** adaptador de transição, opcional, com a chave cifrada e restrita.

**Testes:**
- (a) Token de software (SoftHSM) no Linux, para PKCS#11, sem cartão.
- (b) Assinar o XML e verificar com o **verificador do PEND-09** e com o `xmlsec1`.
- (c) Fila da estação: cartão ausente, PIN errado, cartão retirado no meio, estação desligada. Nunca uma nota "meio assinada".

**Aceitação:**
- (a), (b) e (c) passam.
- **Em equipamento real:** o **seu A3** assina um XML de homologação num caixa Windows, e outro caixa obtém a assinatura pela estação.
- Nenhum arquivo, banco ou registro contém o PIN ou a chave (busca automática nos dados após o teste).

### F2 · Canal autenticado com a SEFAZ (mTLS)
**Escopo:** cliente SOAP 1.2 com TLS de cliente usando o mesmo `Assinante` (a chave do A3 assina o aperto de mão TLS, via `rustls` com chave externa). Tabela de endereços por UF e ambiente **em dados** (atualizável). Certificado do servidor conferido.

**Testes:** servidor falso local (TLS mútuo) para rejeições, tempo esgotado e respostas malformadas.

**Aceitação:**
- Status do serviço (`NFeStatusServico4`) respondido pela **homologação real** da SEFAZ-SP e do Ambiente Nacional, com o seu A3.

### F3 · PEND-15 · Autorização automática das compras por XML
**Escopo, em duas vias:**
- (a) **Distribuição de DF-e** (Ambiente Nacional) por NSU, para o CNPJ da loja: recebe as notas emitidas contra a FANTI e os **cancelamentos**, mesmo sem manifestação. Manifestação **Ciência da Operação** automática só quando for preciso o XML completo.
- (b) **Consulta da situação** da chave na SEFAZ do emitente, para uma nota importada que ainda não veio pela distribuição.

**Efeito na compra:**
- O estado novo `comprovada_sefaz` libera o lançamento.
- Cancelada, denegada ou "não encontrada" **bloqueia**.
- Falha de comunicação mantém `aguardando`, mostra uma pílula discreta "consulta pendente" e tenta de novo depois, **sem travar o caixa**.

**Limites oficiais no agendador:**
- No máximo 20 consultas por chave por hora.
- Espera de 1 hora após "nenhum documento".
- Nunca repete a consulta durante um bloqueio 656.

D7 (conferência humana) continua só como **exceção**, registrada como tal, quando a SEFAZ estiver fora por tempo longo. Deixa de ser a rotina.

**Testes:**
- Servidor falso com cada código de retorno: 100, 101, 110, 135/155 (cancelamento), 137, 138, 217, 656.
- Nota cancelada **depois** de lançada: aviso ao gerente e estorno guiado, nunca automático.

**Aceitação:**
- Na **homologação real**, uma NF-e de homologação emitida contra o CNPJ de teste passa para `comprovada_sefaz` sem intervenção.
- Uma cancelada fica bloqueada.
- 0 lançamentos sem comprovação (teste de atomicidade).

### F4 · Configuração tributária por empresa/CNPJ
**Escopo:** cada empresa (CNPJ) com regime, CRT, IE, CSC, séries e ambiente. Cada produto com NCM, CEST, origem, CSOSN/CST, PIS/COFINS, IPI e grupo IBS/CBS por **regra configurável**. Valores aproximados de tributos (Lei 12.741) por tabela importada.

**Aceitação:**
- Duas empresas fictícias (Simples CRT 1 e Normal CRT 3) emitem com regras diferentes sem mudar o código.
- Nenhum imposto ou regime fixo no código (busca automática nos fontes).
- O contador confere as regras da FANTI.

### F5 · Emissão própria de NF-e modelo 55 (homologação)
**Escopo:** montar o XML 4.00 a partir da venda e da configuração (F4); validar no esquema oficial (XSD) antes de assinar; assinar (F1); transmitir síncrono (F2); guardar o `nfeProc` autorizado; imprimir o DANFE em A4.

**Aceitação:**
- 100% das notas de teste passam no XSD.
- Autorizadas na **homologação da SEFAZ-SP** (cStat 100), com o seu A3.
- O DANFE confere com o XML.
- O XML autorizado passa no verificador do PEND-09.

### F6 · Emissão própria de NFC-e modelo 65 (homologação)
**Escopo:** venda do balcão gera a NFC-e; QR Code v3 com o CSC; DANFE NFC-e na impressora A4 que a loja usa (e bobina, se um dia houver); consumidor com ou sem CPF.

**Aceitação:**
- Autorizada na homologação da SEFAZ-SP.
- QR Code lido e aceito pela consulta pública de homologação.
- A venda não demora mais por causa da nota (medido).

### F7 · Rejeições, cancelamento, contingência e retomada
**Escopo:**
- Cada rejeição explicada em português com o que corrigir, permitindo reenviar.
- Cancelamento dentro do prazo, carta de correção (só NF-e) e inutilização de números.
- **NFC-e:** contingência **off-line** de SP (emite e imprime sem internet, transmite depois, dentro do prazo).
- **NF-e:** SVC-AN/SVC-RS e EPEC, conforme o MOC.
- **Retomada:** após queda de energia ou de rede, nenhuma nota fica com destino desconhecido. O Rustinho consulta o recibo ou a chave antes de reenviar e nunca gera o mesmo número duas vezes.

**Aceitação:**
- Testes de "puxar o cabo" em cada etapa (antes de enviar, enviado sem resposta, autorizado sem gravar).
- Na homologação real: cancelamento, carta de correção, inutilização e uma NFC-e em contingência transmitida depois.

### F8 · Validação completa em homologação e em equipamento real
Roteiro único com os 5 caixas Windows, o seu A3, o celular e a impressora: vender, emitir, cancelar, ficar sem internet, voltar, comprar por XML e lançar. Tudo registrado com fotos e XMLs **de homologação**.

### F9 · Piloto em produção
Só depois de F8 aprovado por você e conferido pelo contador, num caixa e por tempo limitado, com o SINTEC ao lado. **Fora deste plano até você autorizar.**

---

## 4. Critério de conclusão fiscal (o mesmo do seu pedido)

O módulo fiscal só será chamado de pronto quando **todos** forem verdade:
1. **Emissão e assinatura:** NF-e 55 e NFC-e 65 autorizadas na homologação real com o A3.
2. **Comunicação e confirmação automática:** confirmação automática das autorizações (vendas e compras) comprovada na homologação.
3. **Exceções:** rejeições, cancelamentos, contingência e falhas com tratamento seguro e testado (F7).
4. **Equipamento real:** fluxo completo em equipamento real (F8).
5. **Regras tributárias:** conferidas pelo contador (F4).

O PEND-09 (verificar XML recebido) e o PEND-15 (autorização das compras) **sozinhos não bastam**.

---

## 5. Compilação e testes por plataforma (para todas as fases)

| Plataforma | Compilação | Teste automático | Equipamento real |
|---|---|---|---|
| Linux x86-64 | Aqui | Aqui | Seu notebook |
| Windows 10/11 x64 | GitHub (com sua autorização) ou sua máquina | Testes do núcleo no Windows do GitHub | Caixas da loja |
| Android ARM64 | GitHub | Emulador ARM64 | Celular de 64 bits |
| Android ARMv7 | GitHub | **Não há emulador confiável**: só no aparelho | Seu celular de 32 bits |

**Limites deste ambiente:** daqui eu **não** consigo compilar Windows nem Android (Qt, NDK e os alvos Rust estão bloqueados pela rede). Também não consigo baixar os artefatos e registros do GitHub (os servidores de armazenamento do GitHub estão bloqueados). Cada compilação para Windows/Android precisa de: (a) sua autorização de envio de um ramo de teste, **ou** (b) você rodar um script na sua máquina, **ou** (c) liberar no claude.ai os domínios `productionresultssa16.blob.core.windows.net` e `results-receiver.actions.githubusercontent.com`.

---

## 6. Modelo e esforço por fase

| Fase | Modelo / esforço |
|---|---|
| P0, P1, P2, PEND-17, F0 | Sonnet 5.5, alto |
| F1, F2, F3 (criptografia, A3, mTLS, regras de consulta) | **Opus 5.5, alto** |
| F4 | Opus 5.5, alto (regras fiscais) |
| F5, F6 | Opus 5.5, alto |
| F7 | Opus 5.5, alto |
| F8 | Sonnet 5.5, médio (roteiro e registro), com você e os aparelhos |
