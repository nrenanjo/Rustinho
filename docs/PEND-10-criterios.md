# PEND-10 · Sincronização sem internet, por todas as interfaces locais (cabo primeiro, Wi-Fi como reserva)

**Status:** **revisão 3 (20/10/2026): implementado e testado em simulação; validação física (FV-1 a FV-13) pendente; Windows e Android têm lacunas registradas (§8.3).** Critérios e plano de testes foram definidos **antes** do código (revisão 1); a revisão 2 incorporou as 5 observações do dono do projeto (§0); a revisão 3 (§8) registra o que foi feito e os resultados. Versão 0.9.11; o protocolo (`PROTOCOLO`) não muda.
**Regra de ouro:** *o caixa é identificado pelo `dispositivo` (id), nunca pelo endereço.* Endereço é só um caminho para chegar nele. Trocar de cabo para Wi-Fi troca o caminho; o contador (`cursor`), a marca, a geração e o histórico continuam sendo do caixa.

## 0. Revisão 2 — as cinco observações do dono do projeto (20/10/2026)

| # | Observação | Onde ficou nos critérios |
|---|---|---|
| 1 | Confirmar que a conexão TCP usa a interface escolhida, com as diferenças entre Windows, Linux e Android | CA-T1 a CA-T5 (§3-T) e tabela por sistema (§3-T) |
| 2 | Classificar interfaces por metadados do sistema; o nome só como alternativa | CA-K1 a CA-K4 (§3-K) |
| 3 | Anúncios automáticos só nas interfaces apropriadas; configuração explícita para VPN, celular e virtuais | CA-A1 a CA-A4 (§3-A) |
| 4 | 10 s × < 1 s (CA-R4) e o cabo direto com link-local | CA-R4 (reescrito) e CA-L1 a CA-L3 (§3-L) |
| 5 | Dezenas de caixas indisponíveis: operações locais responsivas, tentativas limitadas | CA-B1 a CA-B6 (§3-B) |

Os testes novos (T-25 a T-38) são escritos **antes** das mudanças no núcleo, como os demais. Resultados **simulados** (neste computador, endereços 127.0.0.x, conectores falsos, relógio injetado) e **físicos** (equipamentos reais) são registrados em colunas separadas (§5) e nunca se misturam.

## 1. O que existe hoje (lido no código, `nucleo/src/sync.rs`)

| Ponto | Hoje | Consequência |
|---|---|---|
| Servidor TCP | `bind("0.0.0.0")` | Já atende em **todas** as interfaces, inclusive as que surgirem depois. Sem mudança. |
| Escuta UDP | `bind("0.0.0.0")` | Já recebe de todas. O IP de origem do pacote diz por qual interface o outro caixa chegou. |
| Anúncio UDP | um `send_to("255.255.255.255")` a cada 10 s | Um broadcast "limitado" sai por **uma** interface só (a escolhida pelo sistema). Os outros caixas ligados por outra interface não ouvem. **Lacuna 1.** |
| Endereço do par | `sync_par.endereco`, **uma** coluna, reescrita a cada anúncio | Dois caminhos para o mesmo caixa se atropelam; não há reserva. **Lacuna 2.** |
| Escolha do caminho | um único endereço por par; sem ordem, sem reserva, sem memória de falha | Sem prioridade de cabo, sem troca automática. **Lacuna 3.** |
| Contador | por `dispositivo` (certo), **mas** na 1ª volta busca por endereço; endereço novo de caixa conhecido pede `desde = 0` e reaplica um lote inteiro (idempotente, mas desperdício) | **Lacuna 4.** |
| Identidade | `dispositivo` dentro do pacote cifrado com a chave da loja; endereço trocado de dono é detectado (`dono != dispositivo`) | Base boa; vamos preservar. |
| Duplicação | `aplicar` reconhece a alteração pelo id; o cursor só avança depois de aplicar | Retomada já é segura em teoria; **falta teste de corte** (nenhum teste corta a conexão no meio). **Lacuna 5.** |
| Testes | todos em `127.0.0.1` | Não provam várias interfaces. **Lacuna 6.** |
| Depende de internet? | Não (sem DNS, sem servidor externo) | Manter e **provar** (CA-C6). |

## 2. Desenho mínimo (sem reestruturar)

- Novo arquivo `nucleo/src/rede_local.rs` (pequeno, sem tocar no protocolo): lista as interfaces IPv4 ativas (crate `if-addrs`, que usa `getifaddrs` no Linux/Android e `GetAdaptersAddresses` no Windows), classifica cada uma (cabo / Wi-Fi / virtual / celular / desconhecida) e calcula o broadcast dirigido de cada sub-rede. A lista entra por uma função trocável, para os testes simularem interfaces.
- Migração `0019` (a 0017 é do R-05 e a 0018 da F-17): tabela local `sync_endereco(dispositivo, endereco, origem, visto_em, ultimo_ok, falhas, proxima_tentativa)`, semeada com os endereços que já estão em `sync_par`. Não é sincronizada entre caixas. `sync_par.endereco` continua existindo (compatibilidade) e guarda o último endereço que funcionou.
- Anúncio: um envio **por interface** (socket preso ao IP da interface, destino = broadcast dirigido da sub-rede), mais o limitado de hoje. O corpo do pacote continua `{dispositivo, nome, porta}`; campos extras são opcionais e ignorados por versões antigas.
- Puxada: para cada par, lista de caminhos candidatos ordenada (§4); tenta em ordem dentro do mesmo ciclo; falha entra em espera crescente; o contador é sempre o do `dispositivo`.
- Mudanças em `sync.rs`: só a escolha do endereço no ciclo de puxada, o tratamento do anúncio (extraído para uma função testável), o contador na 1ª volta e o registro de erro por caminho.

**Fora do escopo (declarado):** IPv6; encaminhamento entre sub-redes; descoberta por mDNS; alterar o protocolo ou a criptografia; mexer em Argon2/senha da rede; qualquer envio ao GitHub.

## 3. Critérios de aceitação

Legenda: **A** = provado por teste automático neste computador; **F** = só se prova com equipamento real (§6). Nenhum critério é dado por cumprido sem o teste citado ter rodado e passado.

### D · Descoberta por todas as interfaces
| ID | Critério | Prova |
|---|---|---|
| CA-D1 | O anúncio sai por **cada** interface IPv4 ativa e não-loopback (broadcast dirigido da sub-rede dela), mais o broadcast limitado de hoje. Interface desativada, sem IPv4 ou "ponto a ponto" não recebe anúncio. A lista é relida a cada anúncio (10 s): cabo plugado ou IP novo aparecem sozinhos. | A: T-02, T-03, T-17 · F: FV-1, FV-2 |
| CA-D2 | Um anúncio recebido grava o **endereço de origem** como mais um caminho do caixa, sem apagar os outros. | A: T-04 |
| CA-D3 | Falha de envio em uma interface não impede as outras; o erro é registrado uma vez por mudança (sem encher o registro). | A: T-05 |
| CA-D4 | Endereços digitados em Configurações continuam valendo e são associados ao caixa na primeira conversa. | A: T-13 |
| CA-D5 | Se a lista de interfaces falhar, o comportamento volta ao de hoje (broadcast limitado), nunca a nada. | A: T-18 |
| CA-D6 | Link-local (169.254.x.x), usado em cabo direto entre dois computadores sem roteador, é aceito. | A: T-02 · F: FV-2 |

### I · Identidade
| ID | Critério | Prova |
|---|---|---|
| CA-I1 | O mesmo `dispositivo` visto por 2 ou mais endereços é **um** par, com vários caminhos (`pares()` devolve uma entrada só). | A: T-04 |
| CA-I2 | `cursor`, `marca` e `geracao` pertencem ao `dispositivo`. Puxar por um caminho novo de um caixa conhecido continua do contador dele e **não reaplica** o lote inicial. | A: T-13 |
| CA-I3 | Endereço que passou a ser de **outro** caixa (DHCP reaproveitou o IP) não herda contador nem histórico do anterior. | A: T-14 |
| CA-I4 | O próprio caixa visto por outra de suas interfaces (anúncio ou endereço digitado) nunca vira par e não conta como falha de caminho de outro caixa. | A: T-15 |
| CA-I5 | Anúncio de outra loja (chave diferente) é ignorado, venha de qual interface vier. Banco copiado continua bloqueado como hoje. | A: T-05 |
| CA-I6 | O pacote de anúncio antigo (`{dispositivo, nome, porta}`) continua aceito; campos novos são opcionais. `PROTOCOLO` não muda. | A: T-22 |

### R · Escolha e troca do caminho
| ID | Critério | Prova |
|---|---|---|
| CA-R1 | Ordem dos caminhos de um par: (1) cabo; (2) desconhecida/digitada; (3) Wi-Fi; (4) virtual e celular. Dentro da mesma classe, o que funcionou há menos tempo vai primeiro. A classe é a da **interface local que alcança** o endereço (mesma sub-rede). | A: T-01, T-06 |
| CA-R2 | Falha no caminho escolhido (não conectou, ou caiu antes de a resposta completa chegar) faz o **mesmo ciclo** tentar o próximo, até 3 caminhos por par. | A: T-07 |
| CA-R3 | Caminho que falhou entra em espera crescente (**8 s, 16 s, 30 s no máximo**) para não gastar 3 s de conexão a cada 4 s num cabo solto; ao fim da espera é tentado de novo e, se funcionar, **volta a ser o preferido** (cabo reassume em até 30 s + 1 ciclo). | A: T-08 |
| CA-R4 | **Prazos (resolve a divergência 10 s × < 1 s).** A lista de interfaces é lida de novo **a cada ciclo de puxada (4 s)**, não só a cada anúncio (10 s). Caminho cuja interface local sumiu (cabo desconectado) deixa de ser candidato **no ciclo seguinte, sem esperar o tempo-limite de 3 s da conexão**: o que é < 1 s é o *processamento* (o teste prova que o ciclo escolhe o outro caminho sem gastar o tempo-limite); o que pode demorar até 4 s é a *percepção* da mudança. Interface nova (cabo plugado) é percebida no máximo no ciclo seguinte (≤ 4 s) e dispara um **anúncio imediato** (sem esperar os 10 s). | A: T-09, T-26 · F: FV-3 |
| CA-R5 | Endereço alterado (DHCP): o novo é aprendido pelo anúncio; o velho some depois de falhar e de ficar 7 dias sem ser visto (digitados nunca somem sozinhos). O contador é preservado. | A: T-17 |
| CA-R6 | Um par é puxado por **um** caminho de cada vez (nunca cabo e Wi-Fi ao mesmo tempo no mesmo ciclo). | A: T-19 |
| CA-R7 | Configurações mostra, por par, o caminho em uso ("cabo", "Wi-Fi", "digitado") e, se houver, o último erro de cada caminho. | A: T-21 (campos do contrato) · F: FV-3 (aparência) |

### C · Continuidade e integridade
| ID | Critério | Prova |
|---|---|---|
| CA-C1 | Conexão cortada em **qualquer** ponto da resposta: nada parcial é aplicado, o contador não avança; a tentativa seguinte (outro caminho ou o mesmo) completa. O banco final fica igual ao do outro caixa, sem registro duplicado. | A: T-10 · F: FV-3 |
| CA-C2 | Corte depois de aplicar e **antes** de gravar o contador: repetir o lote não duplica nada e avança o contador. | A: T-11 |
| CA-C3 | Troca de caminho no meio de uma puxada de vários lotes (cabo cai entre o lote 1 e o 2): o lote 2 vem pelo outro caminho a partir do contador gravado, sem buracos e sem repetição. | A: T-12 · F: FV-3 |
| CA-C4 | Pedido de faixa de códigos refeito por outro caminho depois de resposta perdida não dá duas faixas diferentes. | A: T-20 |
| CA-C5 | Duas puxadas simultâneas do mesmo par (cenário forçado de erro) terminam iguais, sem duplicata. | A: T-19 |
| CA-C6 | Nada no caminho da sincronização depende de internet (sem DNS, sem servidor externo, sem hora de rede). | A: T-16 (roda com a rede externa indisponível) e revisão do código · F: FV-4 |
| CA-C7 | Todos os testes de rede atuais (`127.0.0.1`) continuam passando **sem alteração**. | A: T-21 |

### F · Falhas e desconexões
| ID | Critério | Prova |
|---|---|---|
| CA-F1 | Todos os caminhos caindo: o ciclo termina em tempo limitado (até 3 caminhos × 3 s por par), vendas e demais operações locais continuam, cada caminho guarda o seu erro, e quando um volta a sincronização retoma **sozinha**, sem reiniciar o programa. | A: T-16 · F: FV-4 |
| CA-F2 | Falha ao abrir a porta TCP/UDP (porta ocupada) deixa de ser silenciosa: vai para o registro. | A: T-23 |
| CA-F3 | Migração 0019 num banco 0.9.11 existente (user_version 18 → 19) preserva os pares e seus endereços. | A: T-24 |

### T · A conexão TCP usa a interface escolhida (observação 1)

Fato técnico que orienta o desenho: escolher o endereço do outro caixa **não** escolhe a placa de saída. Quem escolhe é a tabela de rotas do sistema. Se o cabo e o Wi-Fi estão **na mesma sub-rede** (o caso mais comum numa loja), o sistema decide sozinho (normalmente o cabo, pela menor métrica) e uma "troca para Wi-Fi" por endereço não muda nada. Por isso a conexão de saída passa a ser feita **a partir do IP local da interface escolhida** e o programa **confere o que realmente aconteceu**: depois de conectar, o endereço local do socket diz por qual interface a conexão saiu.

| ID | Critério | Prova |
|---|---|---|
| CA-T1 | A conexão de saída de um caminho prende o socket ao IP local da interface escolhida (`bind` antes de `connect`); o outro caixa vê essa origem. | A: T-27 |
| CA-T2 | Depois de conectar, o endereço local da conexão é comparado com o da interface escolhida; se diferir, o caminho é marcado "não confirmado" (continua válido, mas não conta como prova de que a interface foi usada) e isso vai para o registro uma vez. | A: T-28 |
| CA-T3 | Onde o sistema permite, o socket também é preso **à interface** (não só ao IP): Linux e Android com `SO_BINDTODEVICE` (se o sistema negar, cai para só o IP, sem erro); Windows com `IP_UNICAST_IF` (índice da interface). Em qualquer falha ao prender, o caminho é tentado como hoje (sem prender) em vez de não ser tentado. | A: T-29 (Linux) · F: FV-3, FV-5, FV-6 (Windows e Android) |
| CA-T4 | O caminho em uso exibido em Configurações é o **confirmado** pelo endereço local da conexão, não o planejado. | A: T-28, T-21 |
| CA-T5 | A resposta, o contador e a identidade do caixa não dependem de por qual interface a conexão saiu (o mesmo `dispositivo` por qualquer caminho). | A: T-13 |

Diferenças por sistema (o que será feito e o que fica registrado como pendente):

| Sistema | Prender ao IP | Prender à interface | Interface "de verdade" | Situação |
|---|---|---|---|---|
| **Linux** | `bind(ip)` | `SO_BINDTODEVICE` (sem privilégio a partir do kernel 5.7; se negar, só o IP) | `local_addr()` + índice da interface | implementado e **testado aqui** (simulado) |
| **Windows** | `bind(ip)` | `IP_UNICAST_IF` com o índice da interface (opção própria do Windows para escolher a saída) | `local_addr()` | implementado; **compilado só se o alvo Windows existir aqui; nunca executado no Windows** → FV-6 |
| **Android** | `bind(ip)` | `SO_BINDTODEVICE` costuma ser negado ao aplicativo; o caminho certo é associar o socket à rede pelo `ConnectivityManager` (Java/JNI) | `local_addr()` | só IP + conferência. **Risco conhecido, não resolvido:** com Wi-Fi sem internet e dados móveis ligados, o Android pode mandar tráfego "não associado" pelos dados móveis e a rede local não responder. Registrado; teste físico FV-11; solução (JNI) fica como pendência, sem fingir que está pronta |

### K · Classificação das interfaces (observação 2)

| ID | Critério | Prova |
|---|---|---|
| CA-K1 | A classe da interface vem, nesta ordem, de: (1) **metadado do sistema** quando houver — Linux/Android: `/sys/class/net/<if>/wireless`, `phy80211`, `bridge`, `tun_flags`, `device` ausente (virtual), `type` (ARPHRD); Windows: tipo da interface (`IfType`: Ethernet 6, IEEE 802.11 71, túnel 131, WWAN 243/244); (2) **nome**, só se não houver metadado. | A: T-01, T-30 · F: FV-6, FV-9 |
| CA-K2 | **Desconhecida continua desconhecida**: o que não se consegue classificar nunca é tratado como cabo nem como Wi-Fi (fica entre os dois na ordem e é anunciado só se tiver IPv4 de rede privada — CA-A2). | A: T-30 |
| CA-K3 | A classificação mostrada ao usuário diz a origem ("cabo", "Wi-Fi", "não identificada"), nunca inventa. | A: T-21 |
| CA-K4 | O que **não** está implementado fica escrito: no Windows desta versão a classe vem do **nome** (o tipo do sistema exige chamar `GetAdaptersAddresses` diretamente, o que não pode ser executado nem testado aqui) e no Android do nome (o tipo certo vem do `ConnectivityManager`). Nomes em português do Windows ("Conexão de Rede Sem Fio", "Conexão Local") entram na lista de nomes; o resto fica "não identificada". | A: T-01 · F: FV-6 |

### A · Anúncios só nas interfaces apropriadas (observação 3)

| ID | Critério | Prova |
|---|---|---|
| CA-A1 | Por padrão, anunciam-se **cabo e Wi-Fi**. VPN, celular e virtuais (Docker, VirtualBox, `tun`, `vEthernet`…) **não** recebem anúncio. | A: T-31 |
| CA-A2 | Interface **não identificada** anuncia só se o IPv4 for de rede privada (10/8, 172.16/12, 192.168/16) ou link-local (169.254/16); IP público ou fora disso não. | A: T-31 |
| CA-A3 | Configuração explícita `sync_interfaces` (nomes, separados por vírgula) liberta as interfaces de VPN/celular/virtual **que o gerente nomear**; o que está lá é anunciado e seus caminhos são usados. Endereços **digitados** pelo gerente sempre são usados, em qualquer interface. Sem a configuração, caminhos aprendidos por anúncio de interface virtual/celular são ignorados. | A: T-32 |
| CA-A4 | Receber anúncio continua em todas as interfaces (só o **envio** é restrito). A tela mostra o campo só no modo completo das Configurações (sem poluir a tela simples) e a lista das interfaces que estão sendo ignoradas aparece só quando o gerente a pede. | A: T-32, T-21 · inspeção visual da tela |

### L · Cabo direto com link-local (observação 4)

| ID | Critério | Prova |
|---|---|---|
| CA-L1 | A exclusão de interfaces "ponto a ponto" vale **só** para as marcadas assim pelo sistema (PPP, túneis, VPN). Interface Ethernet ou Wi-Fi com `169.254.x.x/16` **não** é ponto a ponto e entra (broadcast dirigido `169.254.255.255`). | A: T-02, T-33 |
| CA-L2 | O fluxo completo em link-local (anúncio → caminho → conexão presa ao IP local) é provado com endereços simulados na parte lógica; a parte de rede real (dois PCs com cabo direto, sem DHCP) só em FV-2. | A: T-33 · F: FV-2 |
| CA-L3 | Link-local numa interface Wi-Fi sem DHCP (o sistema se deu um 169.254) é anunciado e testado como qualquer outra, sem tratamento especial e sem preferir. | A: T-33 |

### B · Dezenas de caixas indisponíveis (observação 5)

Limites de projeto (escritos aqui para o teste conferir): **tempo-limite de conexão 3 s** (o de hoje); **no máximo 3 caminhos por par por ciclo**; **orçamento de 8 s de espera de conexão por ciclo** (somando os pares); **no máximo 4 pares "sem resposta" tentados por ciclo** (os que estão em espera não contam); ordem dos pares: quem respondeu mais recentemente primeiro, quem nunca respondeu por último, com rodízio entre os mortos para ninguém ficar sem vez.

| ID | Critério | Prova |
|---|---|---|
| CA-B1 | Com 40 pares indisponíveis e 1 saudável, o saudável sincroniza **no primeiro ciclo** (não espera os mortos). | A: T-34 |
| CA-B2 | O tempo de um ciclo com 40 pares mortos tem limite (orçamento + folga), **não** 40 × 3 s. | A: T-34 |
| CA-B3 | Operações locais (venda, cadastro, caixa) continuam responsivas durante o ciclo de pares mortos: uma gravação feita por outra conexão ao mesmo arquivo conclui em < 200 ms; o ciclo não segura o banco enquanto espera conexão. | A: T-35 |
| CA-B4 | Número de conexões/threads simultâneas fica limitado (não cresce com o número de pares) e a memória do registro não cresce (erro repetido é gravado uma vez por mudança). | A: T-36 |
| CA-B5 | Quando um dos mortos volta, ele é sincronizado sozinho em até 30 s (espera máxima) + 1 ciclo, sem reiniciar o programa, com o contador dele preservado e sem duplicar. | A: T-37 |
| CA-B6 | Pares sem notícia há mais de 7 dias deixam de ser tentados automaticamente (continuam listados, para o gerente apagar); digitados nunca somem sozinhos. | A: T-38 |

## 4. Plano de testes automáticos (o que vou escrever, antes do código)

Técnica para simular "várias interfaces" neste computador, sem inventar rede: o bloco `127.0.0.0/8` inteiro é local, então `127.0.0.2`, `127.0.0.3` etc. fazem o papel de interfaces diferentes (origens distintas no UDP e no TCP). Um **encaminhador TCP** de teste, posto num desses endereços, representa "o cabo": fechá-lo é puxar o cabo; cortá-lo depois de N bytes é a conexão caindo no meio. Uma lista de interfaces **simulada** (nome, IP, máscara, estado) alimenta a lógica; o código de produção chama a função real.

| ID | O que testa | Critério |
|---|---|---|
| T-01 | Classificação por nome: `eth0`, `enp3s0`, `eno1`, `Ethernet`, `Ethernet 2` → cabo; `wlan0`, `wlp2s0`, `Wi-Fi`, `Conexão de Rede Wireless` → Wi-Fi; `docker0`, `veth…`, `tun0`, `vEthernet (…)`, `VirtualBox…` → virtual; `rmnet_data0` → celular; resto → desconhecida | R1 |
| T-02 | Alvos de anúncio: broadcast dirigido por interface (/24, /16, /30, link-local), exclui loopback, desativada, /32, IPv6; inclui o limitado uma vez | D1, D6 |
| T-03 | Envio real em sockets: 2 "interfaces" (127.0.0.2 e 127.0.0.3) → o receptor em `0.0.0.0` vê as duas origens | D1 |
| T-04 | Dois anúncios do mesmo `dispositivo` por origens diferentes → 1 par, 2 caminhos; um terceiro com outro dispositivo → 2 pares | D2, I1 |
| T-05 | Chave errada ignorada em qualquer origem; anúncio do próprio caixa ignorado; erro de envio em uma interface não impede a outra e é registrado uma vez | D3, I5 |
| T-06 | Ordem: cabo antes de Wi-Fi mesmo com o Wi-Fi mais recente; virtual por último; digitado no meio; empate → o que funcionou mais recentemente | R1 |
| T-07 | Cabo (encaminhador) fechado: o mesmo ciclo entrega os dados pelo Wi-Fi; resultado completo | R2 |
| T-08 | Espera crescente (relógio injetado): dentro da espera o cabo não é tentado; depois da espera é tentado e, funcionando, vira o preferido de novo | R3 |
| T-09 | Interface sai da lista: caminho some dos candidatos em < 1 s, sem tempo-limite de conexão | R4 |
| T-10 | Corte da resposta em vários pontos (0, 1, 10 bytes, meio, fim − 1): nada parcial aplicado, contador igual ao anterior; segunda tentativa por outro caminho completa; ids únicos = total; bancos iguais | C1 |
| T-11 | Lote aplicado mas contador não gravado (simulado): repetir → 0 novos, contador avança | C2 |
| T-12 | Lote pequeno (`LOTE` ajustável só em teste): lote 1 pelo cabo, cabo cai, lote 2 pelo Wi-Fi; sem buracos nem repetição; contador final = `seqMaxima` | C3 |
| T-13 | Caminho novo de caixa conhecido (também digitado): continua do contador, não reaplica o 1º lote; `sync_par` com 1 linha | I2, D4 |
| T-14 | Endereço passa a ser de outro caixa: não herda contador | I3 |
| T-15 | Endereço próprio por outra interface (digitado ou anunciado): não vira par, mensagem clara, não penaliza o caminho dos outros | I4 |
| T-16 | Todos os caminhos caídos: ciclo limitado no tempo; venda local funciona; um caminho volta → sincroniza sozinho; nenhuma chamada externa | F1, C6 |
| T-17 | IP do par muda de A para B (novo anúncio): sincroniza por B; A expira só quando falha e passa de 7 dias (relógio injetado); digitado não expira; contador preservado | R5, D1 |
| T-18 | Lista de interfaces falha → volta ao broadcast limitado | D5 |
| T-19 | Um caminho por par por ciclo; duas puxadas forçadas ao mesmo tempo terminam iguais, sem duplicata | R6, C5 |
| T-20 | Pedido de faixa repetido por outro caminho: a mesma faixa, nunca duas | C4 |
| T-21 | Regressão: suíte atual de rede (127.0.0.1) intacta; contrato de `pares()` (campos novos) conferido | C7, R7 |
| T-22 | Pacote antigo `{dispositivo,nome,porta}` aceito; pacote novo com campos extras lido por um leitor "antigo" (só os 3 campos); `PROTOCOLO` inalterado | I6 |
| T-23 | Porta ocupada → registro, sem pânico | F2 |
| T-24 | Migração 18 → 19 com pares existentes | F3 |
| T-25 | Ordem dos pares: quem respondeu há pouco primeiro, quem nunca respondeu por último, rodízio entre os mortos (ninguém fica sem vez) | B1 |
| T-26 | Relógio injetado: interface some da lista entre dois ciclos → o ciclo seguinte já não a tenta e entrega por outro caminho sem gastar o tempo-limite; interface nova dispara anúncio imediato | R4 |
| T-27 | Conexão presa ao IP local: o servidor de teste vê a origem `127.0.0.2` (e depois `127.0.0.3`) quando o caminho escolhido é essa "interface"; sem prender, vê `127.0.0.1` | T1 |
| T-28 | Conferência: endereço local ≠ interface planejada → "não confirmado", registro uma vez; confirmado → mostrado como em uso | T2, T4 |
| T-29 | Linux: `SO_BINDTODEVICE` em `lo` funciona ou é negado sem erro (o teste aceita as duas e registra qual); falha ao prender → tenta sem prender | T3 |
| T-30 | Classificação por metadado antes do nome: nome enganoso (`eth0` com `wireless`, `wlan0` sem `wireless`) → vale o metadado; sem metadado e nome desconhecido → "não identificada"; nomes do Windows em português | K1, K2, K4 |
| T-31 | Anúncio padrão: cabo e Wi-Fi sim; docker/VirtualBox/tun/vEthernet/celular não; não identificada só com IP privado ou link-local | A1, A2 |
| T-32 | Configuração explícita libera a interface nomeada; digitado sempre vale; caminho aprendido de virtual sem liberação é ignorado; receber continua em todas | A3, A4 |
| T-33 | Link-local: `169.254.x.y/16` em Ethernet e em Wi-Fi entra; PPP/VPN (ponto a ponto) não; broadcast dirigido `169.254.255.255`; fluxo lógico completo até a conexão presa | L1, L2, L3 |
| T-34 | 40 pares mortos (conector falso que "espera" 3 s no relógio injetado) + 1 saudável: o saudável sincroniza no 1º ciclo; o ciclo respeita o orçamento de 8 s e o máximo de 4 mortos | B1, B2 |
| T-35 | Durante esse ciclo, outra conexão ao mesmo arquivo grava uma venda em < 200 ms (ciclo em thread própria) | B3 |
| T-36 | Contagem de conexões/threads abertas ao longo de 30 ciclos nunca passa do limite; registro não cresce com erro repetido | B4 |
| T-37 | Um dos mortos "volta": sincroniza em ≤ 30 s simulados + 1 ciclo, com contador preservado e sem duplicar | B5 |
| T-38 | Par sem notícia há mais de 7 dias deixa de ser tentado (continua listado); digitado não | B6 |

**Mutações planejadas (defeitos plantados, como nas etapas anteriores):** anúncio só pela primeira interface; origem do anúncio sobrescreve em vez de somar; contador voltando a ser por endereço; sem fallback; sem espera crescente; cabo nunca reassume; resposta parcial aplicada; contador gravado antes de aplicar; endereço próprio vira par; ordem Wi-Fi antes do cabo. Cada uma tem de ser pega por pelo menos um teste acima.

## 5. O que os testes automáticos (simulados) **comprovam** e o que **não** comprovam

*Todo resultado deste documento é de um dos dois tipos e é registrado na coluna certa: **simulado** (neste computador: 127.0.0.x, conectores falsos, relógio injetado) ou **físico** (equipamentos reais, FV-n). Um teste simulado verde nunca é apresentado como prova física.*

| Comprovam (neste computador, endereços 127.0.0.x) | Não comprovam |
|---|---|
| Lógica de descoberta, soma de caminhos e identidade única por `dispositivo` | Que o roteador/switch real entrega o broadcast (isolamento de cliente do Wi-Fi, IGMP, firewall) |
| Ordem cabo → Wi-Fi, fallback, espera e retorno ao cabo (com falhas simuladas) | O comportamento do sistema operacional ao perder o cabo (tempo para a interface cair, rotas antigas) |
| Retomada após corte de conexão em vários pontos; ausência de duplicata e de buraco | Perda de pacotes, latência alta, Wi-Fi fraco, roaming entre pontos de acesso |
| Migração e compatibilidade do pacote de anúncio | Classificação de nomes de interface em Windows em português e em Android reais (testada só com os nomes que conheço) |
| Que o caminho de sincronização não usa internet | Permissões do Android (acesso a multicast/broadcast), perfil de rede e firewall do Windows |

## 6. Validação que depende de dois ou mais equipamentos reais (nada feito)

| ID | Roteiro | Resultado esperado | Evidência a anexar |
|---|---|---|---|
| FV-1 | 2 PCs no mesmo switch por cabo **e** no mesmo Wi-Fi; sincronização ligada nos dois | Cada um aparece uma vez em Configurações, caminho "cabo" | Foto/print da tela + trecho do registro |
| FV-2 | Cabo direto PC–PC, sem roteador nem DHCP (link-local) | Descobrem-se e sincronizam | Print + horas de início e fim |
| FV-3 | Sincronizando um lote grande, **arrancar o cabo**; depois recolocar | Troca para o Wi-Fi em até ~10 s; ao recolocar, volta ao cabo em até ~35 s; contagem de vendas/produtos igual nos dois lados; nenhum duplicado | Contagens antes/depois nos dois PCs |
| FV-4 | Desligar o roteador/modem (sem internet e sem DHCP) com IPs fixos | Sincroniza do mesmo jeito | Print |
| FV-5 | Celular Android só em Wi-Fi + PC com cabo e Wi-Fi | Celular sincroniza pelo Wi-Fi; PC mostra o celular | Print dos dois |
| FV-6 | Windows em português: primeira execução com o firewall (rede privada) e nomes das placas | Pergunta do firewall respondida; placas classificadas corretamente | Print do firewall + lista de nomes das placas |
| FV-7 | Ponto de acesso com "isolamento de clientes" ligado | Wi-Fi falha, cabo funciona; sem erro confuso | Print |
| FV-8 | Trocar o IP de um PC com o programa aberto (DHCP/estático) | O outro reencontra em até ~10–20 s, sem duplicar | Print |
| FV-9 | PCs com VirtualBox/Docker/VPN ativos | Não atrapalham; não são preferidos | Lista de placas + print |
| FV-10 | Suspender e retomar o notebook | Retoma sozinho | Horas |
| FV-11 | Android com dados móveis **ligados** e Wi-Fi da loja **sem internet**; PC na mesma rede | O celular sincroniza com o PC (ou falha de modo claro, registrando o risco do §3-T) | Print do celular e do PC; modelo e versão do Android |
| FV-12 | Cabo e Wi-Fi **na mesma sub-rede**; com a conexão presa ao Wi-Fi, conferir pelo lado do outro PC (captura simples ou registro) de qual placa chegou | Chega pela placa escolhida; se o sistema ignorar a escolha, o programa mostra "não confirmado" | Registro dos dois lados |
| FV-13 | Conferir que VPN/Docker/VirtualBox ativos **não** recebem anúncio e que o campo de interfaces liberadas funciona | Nenhum anúncio nelas; liberar uma delas faz anunciar | Captura ou registro |

## 7. Etapas, modelo e esforço (para gastar só o necessário)

| Etapa | O que | Modelo / esforço |
|---|---|---|
| E1 (esta) | Critérios e plano de testes | Sonnet 5.5, alto (leitura do código e decisões de desenho) |
| E2 | Escrever os testes T-01…T-24 **primeiro** (falhando) e o `rede_local.rs` mínimo | Sonnet 5.5, alto (concorrência e rede exigem cuidado) |
| E3 | Mudanças em `sync.rs`, migração 0019, tela de Configurações (inspecionada visualmente); fazer os testes passarem | Sonnet 5.5, alto |
| E4 | Mutações, suíte completa (dev e final), clippy, entrega | Sonnet 5.5, **médio** (rodar scripts e conferir) |
| E5 | Roteiro FV-1…FV-10 em equipamentos reais | Você, com os aparelhos; eu só reviso as evidências em **Sonnet 5.5, baixo** |
Opus não é necessário nesta pendência.

## 8. Revisão 3 — implementação e resultados (commits 266f570, 28ddc52, 7dc5a73; nada enviado ao remoto)

### 8.1 O que foi implementado

| Parte | Onde | Resumo |
|---|---|---|
| Interfaces e classificação | `nucleo/src/rede_local.rs` | Lista as interfaces, classifica por **metadado do sistema** (Linux/Android: `sysfs`) e só então pelo nome; o que não se identifica fica "desconhecida" (nunca adivinhado). Alvos de anúncio por interface (broadcast dirigido; link-local 169.254.x.y incluído). |
| Caminhos por caixa | `nucleo/src/caminhos.rs`, migração `0019_caminhos_da_rede.sql` (tabela local `sync_endereco`) | O caixa continua sendo o `dispositivo`; o endereço é só um caminho. Ordem: cabo, desconhecida, Wi-Fi, virtual, celular (digitado no meio). Um caminho com sucesso por caixa por ciclo; espera crescente 8/16/30 s; cabo reassume quando volta; endereço que mudou de dono não herda contador; endereço próprio não vira caixa nem conta falha. |
| Ciclo limitado | `caminhos::ciclo` | Orçamento de 8 s e no máximo 4 caixas "mortos" por ciclo; ativos primeiro; mortos em rodízio. Operações locais não esperam a rede. |
| Conexão presa à interface | `sync.rs` (`puxar_por`, socket2) | `bind` ao IP da interface planejada; `SO_BINDTODEVICE` (Linux/Android, melhor esforço); conferência por `local_addr()` → "em uso" só quando confirmado; senão "não confirmado" (registrado uma vez). |
| Anúncio | `sync.rs` | Um pacote por interface apropriada (cabo, Wi-Fi); VPN/virtual/celular só com configuração explícita (`sync_interfaces`); interfaces relistadas a cada 1 s, mudança dispara anúncio imediato (CA-R4: < 1 s em vez de até 10 s). Pacote e `PROTOCOLO = 3` inalterados. |
| Identidade de endereço novo | `sync.rs`/`caminhos.rs` | Sonda de identidade (cursor máximo, resposta "recomeçar" já existente) quando o endereço é desconhecido e já há pares: caminho novo de caixa conhecido continua do contador. |
| Backup | `backup.rs` | `sync_endereco` é preservado ao restaurar (pertence a este computador). |
| Tela | `TelaConfiguracoes.qml`, `TelaJuntar.qml` | "Caixas da loja": "via cabo / Wi-Fi / rede" só quando confirmado; campo de interfaces liberadas só em modo completo. Corrigido defeito antigo: as bolinhas de situação das linhas dos caixas nunca eram desenhadas. |

### 8.2 Resultados — **simulados** (este computador; 127.0.0.x, conectores falsos, relógio injetado)

| Verificação | Resultado |
|---|---|
| T-01 a T-38 (tabela §4) | Todos escritos antes do código e verdes. T-21 = suíte anterior de rede (127.0.0.1) sem alteração. Testes novos de PEND-10: 39 (mais o de integração `i01`: cabo cai, Wi-Fi entrega, cabo reassume). |
| Suíte completa, desenvolvimento | núcleo 256 ok + 1 ignorado; testes de integração 9 / 4 / 7 / 5 ok; 0 falhas |
| Suíte completa, programa final (release, `overflow-checks`) | mesmos números, 0 falhas |
| Teste de volume (ignorado por padrão) | ok |
| `cargo fmt --check`, `clippy -D warnings` | limpos |
| Mutações (P1–P31, defeitos plantados) | **31/31 pegas** (P11 e P31 foram inicialmente "não pega"; corrigido o filtro do teste e acrescentado `confirmacao_compara_o_ip_local_com_o_planejado`) |
| Inspeção visual (Linux, janela real sob xvfb) | Configurações › Caixas da loja como gerente e como operador (capturas em `visual11/`) |

### 8.3 Limites e lacunas (não escondidos)

1. **Windows:** prender a conexão a uma interface (`IP_UNICAST_IF`) e obter o tipo da placa pela API do sistema **não foram implementados** (só há alvo Linux aqui). No Windows vale `bind` ao IP + conferência por `local_addr()` + classificação por nome. → PEND-13.
2. **Android:** a conexão precisa de `Network.bindSocket` (ConnectivityManager/JNI) para escolher Wi-Fi quando há dados móveis ligados e o Wi-Fi não tem internet; hoje só `bind` ao IP. Risco de FV-11. → PEND-14.
3. Celular em modo USB (tethering) aparece como cabo (limite dos metadados).
4. A filtragem de interfaces virtuais no `Motor` só é testada via `aceitar_anuncio`; o `listar()` real não pode ser simulado.
5. O teste de "porta ocupada" confere `Motor::problemas()`, não o arquivo de registro global.
6. Rótulo de interface desconhecida fica neutro ("rede").
7. Layout estreito (Android) e Windows: inspeção visual impossível neste ambiente.
8. **Nenhum teste físico (FV-1 a FV-13) foi feito.** Nenhum resultado desta revisão deve ser lido como prova física.
