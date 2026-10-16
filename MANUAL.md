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
- **Senha do backup** (a mesma em todos os caixas): a cópia sai trancada (ChaCha20-Poly1305). **Anote a senha num papel e guarde fora da loja: sem ela o backup não abre.**
- **Guardar:** 30 dias (dá para mudar). De cada dia fica a cópia mais nova; as 3 mais novas nunca são apagadas.
- **Pasta:** a pasta `backups` ao lado dos dados (veja "Onde ficam os dados"; dá para trocar, ex.: um HD externo).
- **Google Drive** (grátis, 15 GB sobram muito):
  1. O programa que leva os arquivos (rclone) já vem no instalador do Windows e no AppImage. Compilando no Linux, instale o `rclone` pelo gerenciador de pacotes.
  2. Em Backup, crie a senha e clique em **Conectar ao Google Drive**. O navegador abre: entre na conta do Google e clique em **Permitir**. O Rustinho não vê a senha do Google e só enxerga os arquivos que ele mesmo mandar (pasta "Rustinho Backup").
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

- Notas fiscais: NF-e e NFS-e
- Boletos e conciliação bancária
- Garantias e kits de serviço (ex.: revisão com as peças certas)
