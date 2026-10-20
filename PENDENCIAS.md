# Pendências conhecidas

Coisas que sabemos que faltam ou que ainda não foram resolvidas. Cada uma tem um número para ser citada. Quando uma for resolvida, ela sai desta lista e entra no manual.

## Códigos

- **PEND-01 · Código de clientes e fornecedores entre caixas sem rede — corrigido nesta rodada, com um limite (PEND-07).**
  Os códigos de números de clientes e fornecedores agora saem da faixa do caixa (a mesma das peças, com contagem própria), sem renumerar nada que existe. Veja o manual e os testes `j_*` em `faixas.rs`. O que resta está na PEND-07.

- **PEND-07 · Código de cliente/fornecedor com letras escolhido por dois caixas sem rede pode repetir.**
  Diferente das peças, o código com letras de uma pessoa **não** espera a confirmação do caixa que distribui as faixas: vale na hora. Se dois caixas sem rede escolherem o mesmo, as duas pessoas ficam com ele; nada é trocado, as vendas e os débitos usam o identificador interno, e o código repetido aparece para o gerente em **Gerente › Códigos** (com **Dar código novo**). Foi uma escolha para não repetir toda a mecânica de confirmação onde o código não é lido por leitor nem impresso em etiqueta. Se o Mário quiser a mesma confirmação das peças, é uma coluna `codigo_validado` em `pessoa` e a mesma varredura do distribuidor.

- **PEND-08 · Cadastros feitos num banco copiado ficam com o identificador antigo.**
  Pessoas (e peças) criadas enquanto o caixa está bloqueado como cópia ficam com código a definir e com o identificador do caixa de antes; depois de **Registrar como caixa novo**, o próprio caixa não as atribui sozinho (a manutenção só atribui as do identificador atual). O gerente usa **Dar código deste caixa a todos os cadastros a definir** (ou o caixa de origem as atribui quando recebe). Não repete código; só não é automático.

- **PEND-02 · Código com letras digitado à mão em loja com vários caixas depende do caixa que dá as faixas.**
  Sem esse caixa por perto, o código fica "esperando" (a peça vende, mas sem etiqueta e sem leitor por esse código). É de propósito (não dá para garantir que seja único sem ele), mas, se o caixa que dá as faixas ficar muito tempo fora, o gerente precisa usar **Este caixa passa a distribuir as faixas** em outro caixa.

- **PEND-03 · Troca de placa-mãe, de sistema ou do aparelho muda a identidade do computador.**
  O banco passa a se comportar como cópia (não dá código, não troca dados) até o gerente usar **Registrar como caixa novo**. É o preço de não deixar uma cópia duplicar códigos em silêncio.

## Testes que ainda precisam ser feitos de verdade

- **PEND-04 · Teclado, leitor de código de barras e mouse.** As telas foram abertas sem janela (modo `offscreen`) e a lógica de qual peça entra, de a etiqueta sair e de o leitor achar tem teste automático, mas ninguém usou as telas com teclado, leitor e mouse reais.
- **PEND-05 · Windows, Android, celular físico e rede real entre computadores.** Os testes de rede usam o próprio computador (127.0.0.1). A identidade do computador no Windows e no Android vem do Qt (`QSysInfo::machineUniqueId`) e não foi conferida nesses sistemas.

## Entrega

- **PEND-06 · Assinatura do APK e artefatos antigos.** Antes de qualquer lançamento, confirmar a assinatura do APK e revisar os artefatos antigos que ainda possam trazer a chave pública conhecida. Os 4 APKs das versões 0.9.0 e 0.9.1 não devem ser apagados até se confirmar se algum celular tem dados reais.
