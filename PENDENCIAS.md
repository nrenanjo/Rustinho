# Pendências conhecidas

Coisas que sabemos que faltam ou que ainda não foram resolvidas. Cada uma tem um número para ser citada. Quando uma for resolvida, ela sai desta lista e entra no manual.

## Códigos

- **PEND-01 · Código de clientes e fornecedores pode repetir entre caixas sem rede.**
  Os códigos das **peças** saem de uma faixa de números de cada caixa (veja o manual). Os códigos de **clientes e fornecedores** ainda usam o método antigo: cada caixa dá "o maior código + 1" (`pessoas::proximo_codigo`). Dois caixas sem rede podem dar o mesmo código a pessoas diferentes. Nada é apagado ou trocado quando isso acontece; as pessoas continuam separadas (a chave é o identificador interno, não o código), mas dois cadastros ficam com o mesmo número. Não foi corrigido porque estava fora do pedido da correção dos códigos das peças. A correção pede o mesmo desenho (faixa por caixa, sem renumerar o que existe, repetidos para o gerente) e deve ser feita à parte.

- **PEND-02 · Código com letras digitado à mão em loja com vários caixas depende do caixa que dá as faixas.**
  Sem esse caixa por perto, o código fica "esperando" (a peça vende, mas sem etiqueta e sem leitor por esse código). É de propósito (não dá para garantir que seja único sem ele), mas, se o caixa que dá as faixas ficar muito tempo fora, o gerente precisa usar **Este caixa passa a distribuir as faixas** em outro caixa.

- **PEND-03 · Troca de placa-mãe, de sistema ou do aparelho muda a identidade do computador.**
  O banco passa a se comportar como cópia (não dá código, não troca dados) até o gerente usar **Registrar como caixa novo**. É o preço de não deixar uma cópia duplicar códigos em silêncio.

## Testes que ainda precisam ser feitos de verdade

- **PEND-04 · Teclado, leitor de código de barras e mouse.** As telas foram abertas sem janela (modo `offscreen`) e a lógica de qual peça entra, de a etiqueta sair e de o leitor achar tem teste automático, mas ninguém usou as telas com teclado, leitor e mouse reais.
- **PEND-05 · Windows, Android, celular físico e rede real entre computadores.** Os testes de rede usam o próprio computador (127.0.0.1). A identidade do computador no Windows e no Android vem do Qt (`QSysInfo::machineUniqueId`) e não foi conferida nesses sistemas.

## Entrega

- **PEND-06 · Assinatura do APK e artefatos antigos.** Antes de qualquer lançamento, confirmar a assinatura do APK e revisar os artefatos antigos que ainda possam trazer a chave pública conhecida. Os 4 APKs das versões 0.9.0 e 0.9.1 não devem ser apagados até se confirmar se algum celular tem dados reais.
