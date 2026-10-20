# Manual do Rustinho

O que cada parte faz, versão por versão. Para baixar e instalar, veja o [README](README.md).

## O que já funciona

- **Primeira vez:** nome da loja, nome deste computador (ex.: Caixa 1) e o seu usuário.
- **Entrada:** cada pessoa clica no próprio nome e digita a senha. Depois de 5 senhas erradas, espera 30 segundos.
- **Início:** as funções mais usadas aparecem primeiro. O alfinete fixa uma função em "Mais usados". Funções das próximas etapas aparecem como "em breve".
- **Peças:** uma busca só, que acha a peça por código interno, código de barras, código original, equivalente, nome, marca, carro e ano (ex.: `pastilha gol 2015`, `vw gol 1.6`, `32206a`).
- **Cadastro de peça:** uma tela só, sem abas:
  - dados da peça;
  - preços: custo, % sobre o custo e revenda, que se calculam nos dois sentidos, mais 4 tabelas em "Outros preços";
  - estoque inicial, mínimo e local;
  - aplicações (carros e anos) e equivalências (outras marcas);
  - fiscal básico (NCM, CEST, origem, ANP);
  - histórico de quem mudou o quê.
- **Ajuste de estoque:** com a quantidade contada e o motivo, que fica registrado.
- **Usuários:** cadastro, ativar e desativar, troca de senha. Todos têm acesso a tudo.
- **Configurações:** loja, CNPJ e estado da loja, nome do computador, nomes e lucro sugerido das tabelas de preço, troca da própria senha.

### Novo na etapa 2

- **Clientes e fornecedores:** um cadastro só (uma empresa pode ser as duas coisas). CPF e CNPJ conferidos, inclusive o CNPJ novo com letras. Busca por nome, CPF/CNPJ, telefone, cidade ou **placa do carro**. Carros do cliente, limite de crédito e histórico de alterações.
- **Compras pelo XML da NF-e:**
  1. Em Compras, **Importar XML** (F2) ou arraste os arquivos para a janela. Dá para escolher vários de uma vez.
  2. O fornecedor é cadastrado sozinho, com endereço, se ainda não existir.
  3. Cada item tenta se ligar sozinho a uma peça: pelo código que o fornecedor usou da última vez, pelo código de barras ou pelo código original/equivalente. O que não for achado fica em rosa: **Escolher peça**, **Cadastrar nova** ou **Não lançar**.
  4. Caixa com várias peças: diga quantas peças vêm em cada unidade da nota.
  5. **Frete pago à parte** (transportadora) é dividido entre as peças pelo valor de cada uma.
  6. O custo de cada peça já inclui frete, seguro, IPI e ICMS-ST, menos o desconto (o Simples Nacional não aproveita esses créditos).
  7. **Lançar no estoque** (F2): entra no estoque, o custo é atualizado e, se marcado, o preço de revenda segue o % de lucro de cada peça. Tudo fica no histórico da peça.
  8. A mesma nota não entra duas vezes. Uma compra lançada pode ser **estornada** (as peças saem do estoque).
- **Conferência fiscal** (hoje, Fiscal):
  - Regras de substituição tributária por estado, começo do CEST e período, que o contador pode mudar.
  - Já vem com a regra de São Paulo: autopeças (CEST 01) e pneus (CEST 16) saíram da ST em **01/10/2026** (Portaria SRE 34/2026). Desde então, venda dentro do estado = **CSOSN 102 / CFOP 5102** (antes, 500 / 5405).
  - Lista as peças com CSOSN fixado que não combina com a regra (como as que vêm de outro sistema) e passa todas para **automático** com um clique.
  - Lista peças sem NCM e peças com NCM de autopeça sem CEST.
- **Na ficha da peça:** CSOSN (automático ou escolhido), o CFOP que a venda vai usar, aviso em rosa quando não combina com a regra, marcação de **PIS/Cofins monofásico** e os códigos que cada fornecedor usa para a peça.

**Importante:** o Rustinho sugere e avisa, mas quem decide a tributação é o contador. Confira com ele antes de usar em notas de verdade.

### Novo na etapa 3

- **Vender** (balcão):
  - Uma busca só: passe o **leitor de código de barras** ou digite código, nome, código original ou carro. Cada resultado mostra **estoque, local e preço**.
  - `3*` antes do código ou do nome põe 3 unidades (ex.: `3*7891234567895`, `2*vela`). Passar o mesmo código de novo soma 1.
  - No carrinho dá para mudar a quantidade, o preço e o desconto (em R$ ou %). O desconto máximo de cada peça é respeitado.
  - Vender mais do que o sistema diz que tem **não trava**: aparece "só 2 no estoque" em rosa, e dá para **encomendar** (veja 0.4.3).
  - Cliente opcional: busque por nome, CPF, telefone ou **placa**, ou só digite um nome.
  - **Receber** (F2): dinheiro, Pix, débito, crédito ou outro, e dá para dividir entre várias formas. Clicar no nome da forma põe nela o que falta. O troco aparece grande. Crediário a partir da etapa 4 (veja abaixo).
  - Depois de receber: **Imprimir comprovante** (A4). O comprovante não é documento fiscal e diz isso.
- **Orçamentos:** no Vender, **Salvar como orçamento** (F6), com validade e observações. Sai impresso em A4. Na lista de orçamentos, **Vender** põe tudo no carrinho de novo.
- **Caixa:** abertura com o troco da gaveta, **sangria** e **suprimento**, "dinheiro na gaveta" sempre à vista, vendas por forma de pagamento e **fechamento** com o dinheiro contado (mostra se sobrou ou faltou e imprime). Cada venda pode ser aberta, reimpressa ou **cancelada** (as peças voltam ao estoque).
- **Compras:** quando o custo de uma peça **baixa**, a compra pergunta se o preço de revenda fica ou baixa junto. Quando sobe, o preço segue o % de lucro.
- **Vários caixas juntos (Wi-Fi):**
  1. No caixa que já tem os dados: **Configurações → Caixas da loja** → marque "Trocar dados com os outros caixas" e crie uma **senha da rede da loja** (pelo menos 8 caracteres).
  2. Nos outros computadores, na primeira vez que o Rustinho abrir: **"Já uso o Rustinho em outro caixa da loja"** → nome do computador (Caixa 2, 3, 4) e a mesma senha. Em segundos chegam usuários, peças, clientes, estoque e vendas.
  3. Daí em diante, cada venda, compra ou cadastro aparece nos outros caixas em poucos segundos. Sem internet ou sem rede, cada caixa continua vendendo e acerta quando a rede volta.
  - Os caixas se acham sozinhos. Se a rede não deixar, digite o endereço do outro caixa (aparece em Configurações).
  - Tudo vai criptografado com a senha da rede. Portas usadas: TCP 47810 e UDP 47811 (libere no firewall, se houver).
  - Se o mesmo cadastro for mudado em dois caixas ao mesmo tempo, fica a mudança mais recente. O estoque nunca se perde, porque é a soma das entradas e saídas de todos os caixas.
- **Impressão A4:** o Rustinho cria um PDF e abre no visualizador de PDF, de onde se imprime com Ctrl+P. Em Configurações dá para mandar **direto para a impressora padrão**. Endereço e telefone da loja (em Configurações) saem no comprovante e no orçamento.

## Atalhos

| Tecla | O que faz |
|---|---|
| F11 | Buscar peça (de qualquer tela) |
| F2 | Nova peça (na lista) / Salvar (no cadastro, usuários e configurações) |
| Enter | Abrir a peça escolhida / confirmar |
| ↑ ↓ | Andar na lista de peças enquanto digita a busca |
| Esc | Voltar (pergunta antes se houver alteração não salva) |
| F2 em Compras | Importar XML |
| F2 numa compra | Lançar no estoque |
| Enter | Também aperta o botão que está em foco |
| F2 no Vender | Receber |
| F6 no Vender | Salvar como orçamento |
| Esc no Vender | Limpa a busca; de novo, pergunta se limpa a venda |
| F12 | Mostrar ou esconder CPF/CNPJ, custo, lucro e saldos |
| F2 nas Etiquetas | Imprimir |
| F2 em Ordens de serviço | Nova OS (dentro da OS: salvar) |

## Onde ficam os dados

| Sistema | Banco de dados | Backups |
|---|---|---|
| Windows | `%LOCALAPPDATA%\Rustinho\rustinho.db` | `%LOCALAPPDATA%\Rustinho\backups` |
| Linux | `~/.local/share/rustinho/rustinho.db` | `~/.local/share/rustinho/backups` |
| Android | dentro do app (só ele vê) | dentro do app |

Para começar do zero, feche o Rustinho e apague essa pasta (os backups vão junto: copie antes se quiser guardar). No Windows, o desinstalador oferece apagar os dados.

### Novo na versão 0.9.11: gerente, modo escuro e mais segurança

- **Modo escuro:** no topo da tela inicial, a pílula pequena do **sol** (fundo cinza de sempre) e da **lua** (fundo preto, bom para tela AMOLED). Vale só para o aparelho onde foi escolhido. Impressão e PDF continuam em folha branca.
- **Gerente e caixa:** quem configurou a loja é o gerente; os outros usuários são caixa. Só o gerente cancela venda, estorna, ajusta estoque, faz backup e restaura, mexe nas configurações da loja, nos usuários e no Financeiro. Quando o caixa precisa de uma dessas, o gerente digita a senha dele na hora ("Senha do gerente"), sem trocar de usuário.
- **Pílula Gerente** (em Configurações, só para o gerente): quem é gerente e, com marcas de sim/não, o que fica só para o gerente. A loja sempre fica com pelo menos um gerente. O caixa vê nas Configurações só o que é do aparelho (nome do caixa, impressora e a própria senha).
- **Senhas erradas:** depois de 5 erros seguidos, o Rustinho espera 30 segundos, mesmo que fechem e abram o programa.
- **Android de 32 bits e celulares com vídeo PowerVR** (como o Moto e6i): as telas são desenhadas pela CPU, porque a placa de vídeo deles trocava as cores e sumia com as letras. A câmera do leitor mostra a imagem em preto e branco, o que basta para o código de barras.
- **Assinatura do APK:** o APK agora tem a chave só da loja. **Quem tem a 0.9.1 ou anterior no celular precisa desinstalar o Rustinho antes de instalar a 0.9.11** (uma vez só; os dados do celular se perdem, então faça o backup antes se precisar).
- **Troca de dados entre caixas:** a chave da rede é tirada da senha com Argon2id (mais difícil de adivinhar). O formato da troca mudou (protocolo 3): **atualize todos os caixas juntos**. Um caixa com versão diferente é recusado com aviso ("o outro caixa está com outra versão"), sem receber nem gravar nada pela metade; os dados dele continuam guardados e vão quando ele for atualizado. Um caixa com o relógio errado aparece com aviso em "Caixas da loja", mas **o relógio não decide mais nada**: uma alteração feita depois de ver a outra sempre vale, mesmo com a hora errada. O registro de alterações tira de tempos em tempos os dados das versões velhas (fica só o "esqueleto": quem veio antes de quem). Enquanto um backup é restaurado, a troca de dados espera.
- **Backup sem "senha do backup":** as cópias saem sempre trancadas e **para restaurar é pedida a senha de um gerente** (a de agora deste caixa, ou a que o gerente tinha quando a cópia foi feita, num computador novo). O backup automático continua sem ninguém digitar nada. Quem copiar o banco não abre os backups sem a senha de um gerente. **A proteção é do tamanho da senha do gerente:** com 4 números, ela se descobre testando; para proteger de verdade, use 8 ou mais letras e números. Backups de antes da 0.9.11 abrem com a senha do backup daquela época.
- **Diferenças entre caixas** (pílula Gerente): quando dois caixas mexem na mesma coisa sem um ver o do outro, uma fica valendo (a de mais alterações seguidas; empate, a de identificador maior, igual em todos os caixas) e a outra aparece na lista, **guardada**, com o que mudou e quais caixas. Se não der para saber qual veio depois, também aparece ("ordem incerta"). Nos cadastros (peças, clientes, tabelas), o gerente escolhe **Usar a do outro caixa** ou **Deixar como está**; a escolha vai para os outros caixas, que fecham a mesma diferença sozinhos. Com muitas na lista, **Deixar todas como estão** resolve de uma vez (as versões continuam guardadas na lista dos resolvidos). Reenvio e backup restaurado não repetem o aviso. Também aparece ali quando o mesmo orçamento virou venda em dois caixas sem rede (cancele a que sobrou) e quando a loja ficou sem gerente depois da troca de dados (o usuário mais antigo volta a ser gerente). Um aviso no Início chama o gerente.
- **Um Rustinho de cada vez:** abrir o Rustinho duas vezes no mesmo computador mostra um aviso (dois abertos ao mesmo tempo poderiam estragar uma restauração). Se a pasta dos dados não aceitar essa trava (pasta de rede, por exemplo), o Rustinho não abre e pede uma pasta do próprio computador. **Os caixas não devem dividir um banco numa pasta de rede**: cada caixa tem o seu banco, na pasta dele, e eles trocam dados pela rede da loja. Para mudar de pasta: (1) no caixa antigo, faça um backup e confira ("Conferir" no Backup); (2) instale/abra o Rustinho no computador, com a pasta local; (3) restaure o backup conferido. O Rustinho nunca move nem apaga o banco sozinho. Ao fechar, ele para a troca de dados e espera as tarefas terminarem antes de soltar o banco; se alguma não terminar no prazo, a trava fica até o programa fechar de vez (outro Rustinho no mesmo banco é recusado em vez de gravar junto).
- **Dados só do usuário:** no Linux e no Android, a pasta dos dados, o banco, o registro de falhas e os backups ficam só para o usuário do computador (outros usuários do mesmo computador não leem).
- **Registros incompletos entre caixas:** em "Caixas da loja", cada registro que aponta para algo que falta aparece numa lista, com o que é, desde quando e o que fazer: **espera** (ainda não chegou de outro caixa; resolve sozinho quando chegar), **apagado** (foi apagado em um caixa: abra o registro e corrija) ou **não bate** (não chegou em 24 horas, ou o registro de alterações diz que existe e a tabela não tem: confira os caixas e chame quem cuida do sistema). Os dois últimos aparecem em vermelho e num aviso do Início. Nada é apagado para esconder a diferença.
- **Mesmo orçamento vendido em dois caixas sem rede:** não tem como impedir enquanto os caixas estão desligados um do outro. Quando eles voltam a conversar, a pílula Gerente mostra as duas vendas; cancele a que sobrou.
- **Vendas, pagamentos e recebimentos não são reescritos:** cancelar e estornar marcam o registro (quem, quando e por quê) e o valor original continua; a troca de dados não resolve diferença sobrescrevendo dinheiro.
- **Backup restaurado num caixa (ou computador trocado, ou banco copiado à mão):** cada banco tem uma "geração" e cada caixa guarda, para cada outro, até onde já recebeu e qual foi a última alteração recebida. Se o outro voltou para trás, a conta não bate e os dois recomeçam do zero entre si: recebem tudo de novo, e o que já tinham é reconhecido pelo identificador e não é aplicado outra vez (vendas não duplicam, o que foi apagado não volta). O caixa restaurado recebe de volta tudo o que perdeu, inclusive o que ele mesmo tinha feito depois do backup, e os outros continuam recebendo dele as alterações novas.
- **Dois gerentes decidiram diferente sobre a mesma diferença** (sem rede): vale "Usar a do outro caixa", porque ela grava uma alteração nova feita sobre a que valia; a decisão de manter fica registrada. A lista mostra quem decidiu o quê e qual versão vale agora, igual em todos os caixas.
- **Servidor da troca de dados:** aparelho que conecta e não manda nada espera numa fila e cai em 2 segundos, sem ocupar vaga; cada endereço tem no máximo 2 conexões; o caixa de verdade continua trocando dados mesmo com muitas conexões assim na rede.
- **Tamanho do registro de alterações:** compactar tira os dados das versões velhas, mas guarda o "esqueleto" de cada versão (cerca de 200 bytes), para os caixas atrasados e os backups antigos se acertarem. No teste com 2.000 peças mexidas 10 vezes, uma peça mexida 3.000 vezes e 500 vendas, o banco ficou com 75 MB depois de compactar; um caixa novo recebeu tudo em 13 segundos. Ainda não há limite para o número de esqueletos (fica para quando houver um jeito seguro de apagar).
- **Código interno das peças sem repetir entre caixas:** cada caixa só dá códigos de uma **faixa de números que é só dele**. As faixas são dadas por um caixa só, o que configurou a loja (loja de antes desta versão: o caixa que nunca trocou dados com ninguém, ou, com vários caixas, o que gravou o nome da loja). O primeiro fica com 1 a 99.999 (ou até cobrir o maior código de números que já existe); cada outro caixa recebe a dele (100.000 a 199.999, 200.000 a 299.999...) na troca de dados, e só depois de receber dá códigos dela. Faltando menos de um décimo, ele pede a próxima. Detalhes:
  - Na peça nova, deixe o **código em branco**: o Rustinho dá o código ao salvar (a tela mostra o previsto). Caixa sem faixa (computador novo antes da primeira troca, ou faixa acabou sem rede): a peça fica com **código a definir** e recebe o código quando a faixa chegar; não sai etiqueta dela antes disso.
  - **Código escrito à mão** nunca pode ser o código atual ou antigo de outra peça. Só com números, tem de ser da faixa deste caixa (no caixa que dá as faixas, também um número que ainda não é de faixa nenhuma). **Com letras** (ex.: `A-123`) não há faixa que garanta que dois caixas sem rede não escolham o mesmo código, então ele só **vale depois de confirmado pelo caixa que dá as faixas**, que decide numa gravação só: no caixa que dá as faixas vale na hora (e numa loja de um caixa só, também); nos outros, a peça fica **esperando** até a peça chegar ao caixa que dá as faixas na troca de dados. Enquanto espera, a peça **vende e é achada pelo nome**, mas **a etiqueta não sai** e **o leitor não acha a peça por esse código** (a venda avisa e deixa escolher pela lista). Se dois caixas cadastraram o mesmo código sem rede, **só um confirma** (o da peça criada primeiro); o outro continua esperando, com o conflito na pílula **Gerente › Códigos das peças**, e o gerente resolve com **Dar código novo** (o código que nunca foi confirmado não fica guardado como antigo). Nenhum código é trocado sozinho, e os códigos que já existiam antes desta versão continuam valendo. Sem o caixa que dá as faixas por perto, o código com letras fica esperando (a peça segue vendendo).
  - **Backup restaurado, banco copiado ou levado para outro computador:** o caixa não usa mais a faixa de antes (não dá para saber até onde ela foi usada) e pede outra. Um arquivo ao lado do banco (`rustinho.db.codigos`) guarda até onde o caixa foi; não apague. As faixas nunca são liberadas de novo, nem quando um caixa fica muito tempo sem rede.
  - **Banco copiado inteiro para outro computador (o banco e o `rustinho.db.codigos` juntos, no mesmo caminho):** os arquivos são iguais aos do caixa de origem, então só a **identidade do computador** (a que o sistema dá a ele; o Rustinho guarda só uma impressão digital dela no banco) mostra que é uma cópia. Se a identidade for outra, ou o sistema não informar a identidade, o caixa **não dá código nenhum** (as peças novas ficam com código a definir), **não distribui faixa**, **não aceita código só de números** e, no caso de cópia, **não troca dados com os outros caixas**, até o gerente usar **Registrar como caixa novo** em **Gerente › Códigos das peças**. O registro dá um identificador novo ao caixa e uma geração de códigos nova; ele pede uma faixa nova ao caixa que distribui e **nunca usa a faixa do caixa de onde veio a cópia**. Bancos de versões antes desta conferência pedem uma vez que o gerente confirme **Este é o computador de sempre deste caixa**; restaurar um backup (ação do gerente) passa a ser deste computador. Voltar uma cópia para o computador de origem não tira o bloqueio sozinho. Troca de placa-mãe ou de sistema pode mudar a identidade: o caminho é o mesmo (**Registrar como caixa novo**).
  - **Códigos repetidos de antes** (ou de duas lojas juntadas) **não são trocados sozinhos.** Na pílula **Gerente › Códigos das peças** aparecem os códigos que levam a mais de uma peça (também código de barras repetido e o código antigo de uma igual ao de outra), com **Abrir** e **Dar código novo** (o código de antes fica guardado: a etiqueta velha ainda acha a peça). O leitor, a busca e o Enter da venda **mostram as peças e perguntam qual é**; a etiqueta dessas peças não sai até resolver. Um aviso no Início chama o gerente.
  - **Enter na venda:** só entra direto o código exato de uma peça só. Para um nome, o Enter escolhe a peça marcada na lista que já estava na tela; se a lista ainda não tinha aparecido para o que foi digitado, ela aparece e o próximo Enter escolhe.
  - **Código de clientes e fornecedores** (mesma faixa do caixa, contagem própria): deixe o código em branco e o Rustinho dá o próximo da faixa deste caixa (a tela mostra o previsto). Numa loja de um caixa só continua 1, 2, 3...; os códigos que já existem **não mudam** e o próximo vem depois do maior. Caixa sem faixa, ou bloqueado (cópia do banco): o cadastro fica com **código a definir** e recebe o código quando a faixa chegar, sem mudar mais nada. Código escrito à mão: só com números, tem de ser da faixa deste caixa (como nas peças); **com letras** só vale depois de **confirmado pelo caixa que dá as faixas**, como nas peças: numa loja de **um caixa só** vale na hora; com **vários caixas**, o cadastro fica *aguardando confirmação* (a lista marca o código com “(?)” e a ficha avisa) até o caixa que distribui receber o cadastro. Enquanto espera, o cadastro é achado pelo nome e funciona normalmente (vendas, débitos e notas usam o cadastro, não o código), mas o código **não vale como único**. Se dois caixas sem rede escolherem o mesmo código, o mais antigo fica com ele; o outro continua esperando e o gerente o vê em **Gerente › Códigos** (com o nome do cadastro que já tem o código) e usa **Dar código novo**. Nenhum código é trocado sozinho e os que já existiam antes desta versão valem como confirmados. Uma pessoa com código repetido continua podendo ser alterada, e mudar só o código de um cadastro agora é gravado (e passa pela mesma confirmação). Um cadastro feito num banco copiado, antes do registro como caixa novo, fica com o identificador de antes: o gerente usa **Dar código deste caixa a todos os cadastros a definir**.
  - **Importar os XMLs das notas do sistema antigo** deve ser feito **no caixa que dá as faixas**: lá os códigos de números da nota são mantidos; em outro caixa, os códigos só de números da nota são trocados por códigos da faixa dele (os com letras seguem a confirmação acima).
  - **Se o caixa que dá as faixas quebrar e não voltar**, o gerente usa **Este caixa passa a distribuir as faixas** em outro caixa. É seguro quando o caixa antigo realmente não volta: o novo pula 20 faixas (2 milhões de códigos) para não cruzar com alguma que o antigo tenha dado e ainda não tenha chegado; se o antigo voltar e distribuir ao mesmo tempo, as faixas cruzadas aparecem em vermelho no Gerente e nenhum caixa dá código no pedaço cruzado. Antes, troque dados com o máximo de caixas, para o novo conhecer as faixas já dadas. Os códigos que já existem não mudam.
  - A pílula também mostra a conferência dos códigos feita quando as faixas começaram (quantas peças, maior código, acima de 99.999, com letras, com zero na frente, repetidos).
- **Erro grave:** se o Rustinho tiver um erro interno no meio de uma operação, nada mais é gravado até fechar e abrir de novo (as consultas continuam). Confira se a última operação foi gravada antes de repetir.
- **Compras pelo XML:** só entra NF-e autorizada pela SEFAZ, de venda do fornecedor para a loja (nada de nota de devolução, complementar ou de outro CNPJ, quando o CNPJ da loja está preenchido). O Rustinho também confere se a chave de acesso, o dígito, os dados da nota, a assinatura e o protocolo da SEFAZ **combinam entre si** (coerência): um XML montado à mão ou com o protocolo de outra nota é recusado. **Isso não prova que o XML é autêntico.** O Rustinho 0.9.11 ainda **não calcula o resumo do conteúdo da nota nem verifica a assinatura digital**; um XML em que só os itens, as quantidades ou os valores foram mudados, mantendo a chave, o resumo e o protocolo, **passa** na conferência e entraria no estoque e no custo. Por isso: use só XML que veio direto do fornecedor ou do portal da SEFAZ, e confira os itens e os valores na tela da compra antes de lançar (a tela mostra esse aviso em toda compra). A verificação criptográfica da assinatura é obrigatória antes de confiar nesses XMLs e fica para o módulo fiscal (veja PENDENCIAS.md, R-01). Os arquivos escolhidos (XMLs soltos, .zip e .rar) têm limite total de 512 MB no computador e 128 MB no celular; uma pasta com arquivos demais para cedo, com aviso.
- **Venda e contas:** não vende duas vezes o mesmo orçamento, confere se o cliente ainda existe e recusa quantidades, preços e totais absurdos (acima de R$ 99 bilhões por campo), sem gravar nada pela metade. "1.2.3,45" e "," sozinha não são mais aceitos como número.
- **Registro de falhas:** erros internos ficam em `registro/rustinho.log`, na pasta dos dados, sem nada do que foi digitado (nem a mensagem do erro, que poderia ter um dado): só o lugar no programa.

### Novo na versão 0.9.1: cadastro para qualquer loja

- **Câmera da venda:** o botão de câmera no fim da busca do Vender (e de Peças) voltou a abrir a câmera no celular.
- **Unidade de medida:** o botão ao lado da unidade abre a lista pronta (UN, PC, JG, KIT, PAR, CX, PCT, KG, G, LT, ML, MT, CM, M2, M3...), sem repetidas. Dá para criar a sua ali mesmo (ex.: QD, Quadrado de 10x10). Ao importar XML, as repetidas se juntam sozinhas (PCS e PEC viram PC; KGL vira KG; L vira LT; M e MET viram MT).
- **Venda por KG (ou litro, metro):** as unidades marcadas como "quebrado" aceitam vírgula na venda (`0,350*arame`). As outras vendem só inteiro: o Vender avisa se alguém digitar 1,5 de uma peça.
- **Serviço:** no cadastro, a pílula **Serviço (sem estoque)**: mão de obra, instalação, frete... Entra na venda pela busca, como uma peça, mas não mexe no estoque nem aparece na reposição.
- **Oferta:** em Preços, **Em oferta** com o preço e até quando (sem data, vale até desmarcar). Na venda, entra no lugar do preço da primeira tabela; vencida, volta o preço normal. A busca mostra "oferta" ao lado do preço.
- **Preço na hora da venda:** para o que não tem preço fixo; o Vender pede o preço.
- **Tamanhos e cores:** em produto já salvo, **Tem outros tamanhos ou cores?** cria o próximo (P, M, G; 38, 40; Azul, Preto) copiando tudo, com código próprio e estoque zero. Na ficha, as pílulas da grade mostram o estoque de cada um; toque para abrir.
- **Fiscal para a nota, em pílulas:** ICMS (como antes), **PIS/Cofins** (as tabelas de CST completas, alíquota e natureza da receita), **IPI** (CST, alíquota ou valor fixo, enquadramento e selo) e **Combustível** (código ANP, GLP, gás natural, partida, CODIF, biodiesel, CIDE e origem). No serviço, **ISS** (código do serviço, CNAE, NBS, alíquota, exigibilidade e descrição).
- **Fiscal (no Início):** mostra as notas que o Rustinho vai emitir (NF-e, NFC-e, NFS-e e MDF-e) como "em breve". Por enquanto, as notas continuam no sistema de hoje.

### Novo na versão 0.9.0: Rustinho no celular (Android)

- **App de Android** (Android 9 ou mais novo, celulares de 64 bits): faz tudo o que o computador faz, com as mesmas cores e pílulas. As telas viram uma coluna só, que se rola como um feed; onde o computador mostra duas partes lado a lado (ex.: Caixa, Compra, Ordem de serviço), o celular mostra pílulas para trocar de parte.
- **Deitado também:** é só virar o celular. Com a tela deitada, os campos ficam lado a lado.
- **Câmera como leitor:** no Vender e em Peças, o botão de câmera no fim da busca lê o código de barras da embalagem ou da etiqueta (EAN, Code 128, Code 39 e QR).
- **Voltar do celular** faz o mesmo que o Esc: volta uma tela. Na primeira tela, fecha o app (o backup é feito antes).
- **Junto com os caixas:** na primeira vez, escolha "Já uso o Rustinho na loja: juntar" e use a senha da rede da loja; o celular precisa estar no mesmo Wi-Fi.
- **Backup:** fica no próprio aparelho. O Google Drive fica com os computadores da loja (lá estão todos os dados, os do celular também).
- **XML, zip e rar:** escolha o arquivo pelo seletor do Android (Downloads, WhatsApp, Drive...). No celular não tem "escolher uma pasta".
- **Telas menores no computador:** a janela do Rustinho também se ajeita quando fica estreita.

### Novo na versão 0.8.0: Windows, AppImage, zip e rar

- **Windows 10 e 11 (64 bits):** instalador com avançar, avançar, concluir. Atalho no menu Iniciar e na área de trabalho, e **Desinstalar o Rustinho** no menu Iniciar. O desinstalador pergunta, em destaque, se apaga também os dados da loja (vem desmarcado).
  - O instalador já libera o Rustinho no firewall do Windows, para os caixas se acharem pelo Wi-Fi.
  - Impressão direta na impressora padrão (Configurações → Impressão).
  - O programa do backup no Google Drive (rclone) vem junto.
- **Linux:** um arquivo AppImage, que abre sem instalar (com o rclone junto).
- **Zip e rar:** onde o Rustinho aceita XML (Compras e Importar XML de Peças, Clientes e Fornecedores), aceita também `.zip` e `.rar` com os XMLs dentro, e pastas. Um zip com várias notas vira várias compras de uma vez; cancelamentos e cartas de correção que vêm junto são pulados.

### Novo na versão 0.7.0: Backup

- **Configurações → Backup.** Uma cópia por dia, no horário escolhido (18:00 se ninguém mudar), e outra ao fechar o Rustinho. Se o computador ficar desligado no horário, a cópia sai na próxima vez que abrir. Nada trava o caixa: a cópia roda em segundo plano.
- Cada cópia é **conferida**: o Rustinho abre o arquivo e verifica o banco. Cópia que não abre é apagada e vira aviso.
- **Senha do backup** (a mesma em todos os caixas): a cópia sai trancada (ChaCha20-Poly1305). **Anote a senha num papel e guarde fora da loja: sem ela o backup não abre.** (Desde a 0.9.11 não tem mais senha do backup: restaurar pede a senha de um gerente.)
- **Guardar:** 30 dias (dá para mudar). De cada dia fica a cópia mais nova; as 3 mais novas nunca são apagadas.
- **Pasta:** a pasta `backups` ao lado dos dados (veja "Onde ficam os dados"; dá para trocar, ex.: um HD externo).
- **Google Drive** (grátis, 15 GB sobram muito):
  1. O programa que leva os arquivos (rclone) já vem no instalador do Windows e no AppImage. Compilando no Linux, instale o `rclone` pelo gerenciador de pacotes.
  2. Em Backup, clique em **Conectar ao Google Drive** (até a 0.9.1, antes era preciso criar a senha do backup). O navegador abre: entre na conta do Google e clique em **Permitir**. O Rustinho não vê a senha do Google e só enxerga os arquivos que ele mesmo mandar (pasta "Rustinho Backup").
  3. Pronto: cada cópia do dia vai também para o Drive. Faça isso em um caixa só (o que fica mais tempo ligado).
- **Restaurar:** escolha a cópia na lista (deste computador, do Google Drive ou "Abrir outro arquivo…" de um pen drive). Antes, o Rustinho guarda uma cópia de agora ("antes de restaurar"), então dá para desfazer. O nome do caixa, a sincronização e os ajustes do backup deste computador ficam como estão. Com a sincronização ligada, os outros caixas mandam de volta o que mudou depois da data da cópia.
- **Avisos no Início:** quando o último backup falha, quando passam 2 dias sem backup ou 2 dias sem nada chegar ao Google Drive.
- **Compras:** se o XML não é da loja (outro CNPJ), a nota mostra o aviso e deixa lançar mesmo assim. Sem o CNPJ da loja nas Configurações, lembra de preencher.

### Novo na versão 0.6.0: Ordem de serviço (oficina)

- **Ordem de serviço** (no Início), com o caminho em pílulas: **Orçamento › Em serviço › Pronta › Entregue**.
  1. **Nova OS** (F2): comece pela **placa**. Se o carro já veio antes (ou está no cadastro de um cliente), o Rustinho preenche o carro, o cliente e o celular.
  2. Anote **o que o cliente falou** e, depois, o **diagnóstico** do mecânico.
  3. **+ Peça** procura no estoque (com o preço de revenda, que dá para mudar). **+ Serviço** é a mão de obra: digite o que vai ser feito e o valor. Os serviços mais comuns aparecem em botões (troca de óleo, pastilhas, alinhamento...).
  4. **Mecânico**, **previsão** (Hoje, Amanhã, Em 2 dias, Em 1 semana) e observações.
  5. **Cliente aprovou** → **Ficou pronta** → **Avisar no WhatsApp** (mensagem pronta) → **Entregar e receber**: abre o Vender com as peças e a mão de obra, para receber em qualquer forma (dinheiro, Pix, cartão, crediário, cheque ou vale-troca).
  - As peças **só saem do estoque na entrega**. Se a venda for cancelada, a OS volta para "Pronta" e as peças voltam.
  - A OS impressa (A4) tem o carro, o km, o que o cliente relatou, peças, serviços e a linha para o cliente assinar ("Autorizo..." no orçamento, "Recebi o carro..." na entrega). Recado no pé da OS em Configurações → Impressão (ex.: garantia da mão de obra).
  - Carro novo de cliente cadastrado entra sozinho no cadastro dele. Na OS aparece quando o carro já veio antes.
  - No Início, a faixa **Oficina** avisa dos carros prontos e das OS que passaram da previsão.
- **Serviço na venda:** a venda pode ter mão de obra junto com as peças (vem da OS). O serviço não mexe no estoque.

### Novo na versão 0.5.0

- **Olho (dados escondidos):** nas telas com CPF/CNPJ, custo, lucro, comissões e saldos aparece no canto de cima, à direita, a pílula **Mostrar valores**. O Rustinho sempre abre com tudo escondido (••••). Clicou numa tela, vale para todas até clicar de novo (ou **F12**). Ao trocar de usuário, esconde de novo. O preço de venda continua sempre à vista. Para mudar um campo escondido, é só clicar nele.
- **Painel** (no Início, no lugar de Relatórios), em pílulas:
  - **Resumo:** vendido hoje, no mês (comparado com o mês passado até o mesmo dia) e no ano, lucro do mês, vendas por dia, últimos 12 meses, movimento por hora, como os clientes pagaram, mais vendidos, vendedores e o que há a receber e a pagar. Passe o mouse numa barra para ver o valor.
  - **Clientes:** quem mais compra (30 dias, 90 dias ou 12 meses), quantas compras, a última, a média e se está devendo.
  - **Fornecedores:** de quem a loja mais compra (notas lançadas).
  - **Pagamentos:** o que entra na hora (dinheiro, Pix, débito) e o que entra depois (crédito, cheque, crediário).
- **Trocas e devoluções** (no Início, ou **Devolver ou trocar** na venda):
  1. Ache a venda pelo número do comprovante, pelo cliente ou pela peça.
  2. Digite quantas peças voltam. **Com defeito**: a peça não volta para a prateleira (fica anotado no histórico dela, para a garantia).
  3. Escolha como devolver: **vale-troca**, **dinheiro** (sai da gaveta), **Pix ou cartão** ou **abater do crediário** (se o cliente ainda deve aquela venda).
  4. **Trocar agora** devolve e já abre o Vender com o vale-troca. No **Receber**, o vale aparece em cima e já vem preenchido. Vale maior que a compra: o que sobra continua no vale.
  - O valor de cada peça é o que o cliente pagou, já com o desconto da venda. Comprovante de devolução e vale-troca saem em A4.
  - **Já devolvidas:** reimprimir ou **desfazer** (com motivo). **Vales-troca:** os vales com saldo.
  - A devolução sai do Painel, das comissões, dos mais vendidos e do ritmo de venda. Dinheiro devolvido aparece no fechamento do caixa. Venda com devolução só se cancela depois de desfazer a devolução.
- **Etiquetas de preço** (botão **Etiquetas** em Peças, no cadastro da peça e na compra lançada):
  - Folhas A4 de etiquetas adesivas: **65**, **33** ou **21 por folha** (tamanhos das Pimaco A4351, A4356 e A4360).
  - Passe o leitor na peça para somar uma etiqueta, ou digite o nome e Enter. Na compra, já vem cada peça com a quantidade que chegou.
  - **Folha já usada:** clique na primeira etiqueta livre no desenho da folha.
  - Nome, preço, código e **código de barras** (o EAN da peça; sem EAN, o código interno). Conferido com leitor: o caixa lê as duas.
  - **Saiu torto?** Ajuste fino em milímetros (fica guardado).
- **Trazer cadastros pelo XML** (botão **Importar XML** em Peças, Clientes e Fornecedores):
  - **XMLs das notas da loja** (exportados do sistema antigo): trazem os **clientes** e as **peças com o código e o último preço da loja**.
  - **XMLs de fornecedores:** trazem os **fornecedores** e as peças com o custo (o código do fornecedor fica ligado à peça).
  - Aceita vários .xml, um **.zip** ou uma **pasta inteira**. Antes de cadastrar, mostra quantos são novos. Nada que já existe é mudado. Precisa do **CNPJ da loja** em Configurações.
- **Configurações** em pílulas: **Loja**, **Vendas**, **Crediário**, **Impressão**, **Caixas da loja** e **Sobre**.
  - **Crediário:** multa, juros ao mês e **tolerância** (dias de atraso sem cobrar nada), com um exemplo calculado na hora.
  - **Vendas:** validade padrão do orçamento.
  - **Impressão:** recado no pé do comprovante e do orçamento (ex.: regras de troca) e o local da peça (prateleira) no comprovante.
- **Importante com vários caixas:** atualize todos os computadores juntos (a estrutura do banco mudou na 0.5.0 e na 0.6.0; a 0.7.0 não mexe na estrutura).

### Novo na versão 0.4.3: Estoque e encomendas

- **Estoque** (no Início), em quatro partes nas pílulas do topo:
  - **O que comprar:** as **encomendas de clientes** que falta pedir (em verde, à parte) e a **reposição do estoque** por prioridade: *Acabou*, *Acaba em N dias*, *Abaixo do mínimo*. Para cada peça: quanto tem, o ritmo de venda (≈ por mês), **quanto comprar**, o custo estimado e o último fornecedor (com o código dele). **Comprar para N dias** muda conforme a demanda (o Rustinho guarda). **Imprimir lista** sai em A4, separada por fornecedor. **Já pedi** marca a encomenda como pedida.
  - **Encomendas:** *Falta pedir → Pedida → Chegou → Entregue*. Cada linha mostra se está **paga**, se **falta** parte (sinal) ou se **paga na entrega**, e o próximo passo: **Já pedi**, **Chegou**, **Avisar** (abre o WhatsApp com a mensagem pronta), **Vender** ou **Entregar**. **Nova encomenda** para quem paga quando a peça chegar (peça cadastrada ou só a descrição).
  - **Mais vendidos:** 30 dias, 90 dias ou 12 meses, por valor ou por quantidade, com lucro e **curva ABC** (A = as peças que fazem 80% do faturamento).
  - **Parados:** peças com estoque que não vendem nem entram há mais de **180 dias** (dá para mudar), com o **dinheiro parado** pelo custo.
- **Encomendar na venda:** peça sem estoque no carrinho mostra **Encomendar?**. Marcado, a venda pede o cliente cadastrado. No Receber, **Resto na entrega** deixa o que falta do sinal no crediário.
- **Chegada pela compra:** ao lançar a nota (XML), as encomendas daquela peça passam sozinhas para **Chegou**, as mais antigas primeiro. A faixa **Encomendas** do Início lembra de avisar o cliente e do que falta pedir.
- A sugestão olha as vendas dos **últimos 90 dias**. Venda de encomenda não entra no ritmo, e a peça que chegou para um cliente fica guardada para ele.
- Tudo fica registrado com data e hora (até o segundo) e quem fez.

### Novo na etapa 4 (parte B): Financeiro

- **Financeiro** (no Início), em cinco partes escolhidas nas pílulas do topo:
  - **A pagar:** as contas da loja. As **duplicatas da nota de compra** entram sozinhas ao lançar a compra (dá para desmarcar na tela da compra). Aluguel, energia e o resto: **Lançar conta**, em uma ou mais parcelas. **Pagar** inteiro ou em parte, com juros ou desconto, saindo de uma conta da loja ou da **gaveta do caixa** (vira sangria). Pagamento em **cheque** da loja sai no "bom para". **Corrigir**, **Tirar** e **Estornar** pedem motivo. **Categorias** simples (aluguel, energia, impostos...), que dá para renomear ou criar.
  - **Contas da loja:** o **Caixa geral** (cofre) e os bancos, com saldo e extrato. **Entrada**, **Saída**, **Transferir** (ex.: do cofre para o banco) e **Conferir saldo** (o Rustinho lança a diferença como ajuste).
  - **Cheques:** os cheques recebidos, por "bom para". **Depositar** numa conta, **Repassar** (para um fornecedor) ou **Devolvido** (sai da conta e pode virar dívida do cliente no Receber). Os botões menores aparecem ao passar o mouse.
  - **Fluxo de caixa:** o que entrou e saiu, dia a dia, e os **próximos 30 dias** (crediário e cheques a receber, contas a pagar) começando do saldo das contas.
  - **Comissões:** quanto cada vendedor ganhou no período. Vale a comissão da peça (cadastro da peça) ou a do vendedor (**Usuários → Comissão**). **Fechar** lança a comissão no A pagar.
- **Cheque na venda e no Receber:** forma **Cheque**, com número do 1º cheque, banco e "bom para". Botões **À vista, 30, 30/60, 30/60/90**: os cheques seguintes vêm um mês depois do outro, com o número em sequência.
- **Crediário:** botões **30, 30/60, 30/60/90** para as parcelas.
- **Caixa:** na sangria, escolha para onde vai o dinheiro (Caixa geral, banco ou nenhuma conta).
- **Início:** faixas também para **contas a pagar** de hoje e vencidas e para **cheques bons para depositar**.

### Novo na versão 0.4.1: busca e avisos

- **Busca com um campo só:** digite qualquer código (interno, de barras, original, de similar ou o código que o fornecedor usa), o nome (*óleo*) ou o modelo (*8PK1515*, junto ou separado). Quando acha por um código, aparece um **balão** ao lado do nome dizendo qual: *Cód. barras*, *Cód. original*, *Similar · Fram*, *Fornecedor · Dayco*. Achando pelo nome, não tem balão.
- **Avisos no Início:** faixa arredondada com o que vence hoje e o que está vencido no crediário. Clique para abrir o Receber; **arraste para o lado** para esconder até o dia seguinte.

### Novo na etapa 4 (parte A): crediário

- **Vender no crediário:** no Receber da venda, a forma **Crediário** (só com cliente cadastrado). Dá para dar uma entrada em dinheiro ou Pix e pôr o resto no crediário. Escolha em quantas **parcelas** (até 24) e, se quiser, o **1º vencimento**. Sem data: vence em 30 dias, ou no **dia do convênio** do cliente.
  - Avisa (sem impedir) se o cliente **passou do limite** ou tem parcela **vencida**.
  - O comprovante sai com as parcelas e uma linha para o cliente **assinar**.
- **Receber** (no Início):
  - Em cima: total a receber, **vencido**, vence hoje e próximos 7 dias.
  - À esquerda: quem deve, com os atrasados primeiro. À direita: as parcelas do cliente.
  - Marque as parcelas (ou receba tudo). **Juros e multa** já vêm calculados e dá para mudar ou dar desconto. Pode pagar **parte**: abate da parcela mais antiga para a mais nova. Troco só em dinheiro.
  - **Recibo** em A4, **extrato** do cliente e **estorno** de pagamento (com motivo).
  - **Lançar dívida:** para quem já devia antes (caderninho ou outro sistema), em parcelas.
  - O que entra no crediário aparece no **Caixa** ("Recebido do crediário"), e o dinheiro conta na gaveta.
- **Cliente:** limite de crédito, **dia do convênio** (as parcelas vencem sempre nesse dia) e o botão **Ver o que deve**.
- **Configurações → Crediário:** % de **multa** por atraso e % de **juros ao mês** (padrão 0).
- Venda com parcela já paga não se cancela: estorne o pagamento antes.

## Organização do código

- `nucleo/`: regras da loja e banco de dados (SQLite), sem interface. Conferir com `cargo test -p rustinho-nucleo`.
  - `pessoas.rs` clientes e fornecedores, `documento.rs` CPF/CNPJ/placa, `compras.rs` e `nfe_xml.rs` compras pelo XML, `fiscal.rs` regras de ST e CSOSN.
  - `oficina.rs` ordens de serviço, `painel.rs` o Painel, `devolucoes.rs` trocas, devoluções e vale-troca, `etiquetas.rs` etiquetas e códigos de barras (EAN-13 e Code 128), `importar.rs` cadastros pelo XML.
  - `vendas.rs` vendas e orçamentos, `caixa.rs` abertura e fechamento, `impressao.rs` comprovantes em A4, `receber.rs` crediário e contas a receber, `pagar.rs` contas a pagar, `financeiro.rs` contas da loja e fluxo de caixa, `cheques.rs`, `comissoes.rs`, `avisos.rs` faixas do Início, `encomendas.rs` encomendas de clientes, `reposicao.rs` sugestão de compra, mais vendidos e parados, `sync.rs` sincronização entre os caixas.
- `app/cpp/pdf.cpp`: transforma o comprovante em PDF A4 (Qt).
  - `migracoes/` guarda a estrutura do banco. Ao abrir, o Rustinho atualiza o banco da etapa 1 sozinho, sem perder nada.
- `app/`: a janela. `src/` liga o Qt ao núcleo, `qml/` tem as telas e `base/Tema.qml` tem as cores, as fontes e as medidas.
- `app/fontes/`: as fontes do livro (Cormorant Garamond, Source Sans 3 e Source Serif 4), com as licenças OFL em `LICENCAS/`. A Cormorant tem os acentos ajustados e números alinhados, como no livro (`preparar_fontes.py`).

## Próximas etapas

O que ainda falta ou não foi resolvido está em [PENDENCIAS.md](PENDENCIAS.md).

- Notas fiscais: NF-e e NFS-e
- Boletos e conciliação bancária
- Garantias e kits de serviço (ex.: revisão com as peças certas)
