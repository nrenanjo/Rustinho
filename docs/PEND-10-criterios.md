# PEND-10 · Sincronização sem internet, por todas as interfaces locais (cabo primeiro, Wi-Fi como reserva)

**Status:** critérios e plano de testes definidos **antes** do código (20/10/2026). Versão 0.9.11; o protocolo (`PROTOCOLO`) não muda.
**Regra de ouro:** *o caixa é identificado pelo `dispositivo` (id), nunca pelo endereço.* Endereço é só um caminho para chegar nele. Trocar de cabo para Wi-Fi troca o caminho; o contador (`cursor`), a marca, a geração e o histórico continuam sendo do caixa.

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
- Migração `0017`: tabela local `sync_endereco(dispositivo, endereco, origem, visto_em, ultimo_ok, falhas, proxima_tentativa)`, semeada com os endereços que já estão em `sync_par`. Não é sincronizada entre caixas. `sync_par.endereco` continua existindo (compatibilidade) e guarda o último endereço que funcionou.
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
| CA-R3 | Caminho que falhou entra em espera (15 s, 30 s, 60 s no máximo) para não gastar 3 s de conexão a cada 4 s num cabo solto; ao fim da espera é tentado de novo e, se funcionar, **volta a ser o preferido** (cabo reassume). | A: T-08 |
| CA-R4 | Caminho cuja interface local sumiu (cabo desconectado) deixa de ser candidato **sem esperar o tempo-limite**. | A: T-09 |
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
| CA-F3 | Migração 0017 num banco 0.9.11 existente (user_version 16 → 17) preserva os pares e seus endereços. | A: T-24 |

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
| T-24 | Migração 16 → 17 com pares existentes | F3 |

**Mutações planejadas (defeitos plantados, como nas etapas anteriores):** anúncio só pela primeira interface; origem do anúncio sobrescreve em vez de somar; contador voltando a ser por endereço; sem fallback; sem espera crescente; cabo nunca reassume; resposta parcial aplicada; contador gravado antes de aplicar; endereço próprio vira par; ordem Wi-Fi antes do cabo. Cada uma tem de ser pega por pelo menos um teste acima.

## 5. O que os testes automáticos **comprovam** e o que **não** comprovam

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
| FV-3 | Sincronizando um lote grande, **arrancar o cabo**; depois recolocar | Troca para o Wi-Fi em até ~10 s; ao recolocar, volta ao cabo em até ~60 s; contagem de vendas/produtos igual nos dois lados; nenhum duplicado | Contagens antes/depois nos dois PCs |
| FV-4 | Desligar o roteador/modem (sem internet e sem DHCP) com IPs fixos | Sincroniza do mesmo jeito | Print |
| FV-5 | Celular Android só em Wi-Fi + PC com cabo e Wi-Fi | Celular sincroniza pelo Wi-Fi; PC mostra o celular | Print dos dois |
| FV-6 | Windows em português: primeira execução com o firewall (rede privada) e nomes das placas | Pergunta do firewall respondida; placas classificadas corretamente | Print do firewall + lista de nomes das placas |
| FV-7 | Ponto de acesso com "isolamento de clientes" ligado | Wi-Fi falha, cabo funciona; sem erro confuso | Print |
| FV-8 | Trocar o IP de um PC com o programa aberto (DHCP/estático) | O outro reencontra em até ~10–20 s, sem duplicar | Print |
| FV-9 | PCs com VirtualBox/Docker/VPN ativos | Não atrapalham; não são preferidos | Lista de placas + print |
| FV-10 | Suspender e retomar o notebook | Retoma sozinho | Horas |

## 7. Etapas, modelo e esforço (para gastar só o necessário)

| Etapa | O que | Modelo / esforço |
|---|---|---|
| E1 (esta) | Critérios e plano de testes | Sonnet 5.5, alto (leitura do código e decisões de desenho) |
| E2 | Escrever os testes T-01…T-24 **primeiro** (falhando) e o `rede_local.rs` mínimo | Sonnet 5.5, alto (concorrência e rede exigem cuidado) |
| E3 | Mudanças em `sync.rs`, migração 0017, tela de Configurações; fazer os testes passarem | Sonnet 5.5, alto |
| E4 | Mutações, suíte completa (dev e final), clippy, entrega | Sonnet 5.5, **médio** (rodar scripts e conferir) |
| E5 | Roteiro FV-1…FV-10 em equipamentos reais | Você, com os aparelhos; eu só reviso as evidências em **Sonnet 5.5, baixo** |
Opus não é necessário nesta pendência.
