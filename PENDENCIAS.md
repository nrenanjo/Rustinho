# Pendências conhecidas

Coisas que sabemos que faltam ou que ainda não foram resolvidas. Cada uma tem um número para ser citada. Quando uma for resolvida, ela sai desta lista e entra no manual.

## Códigos

- **PEND-01 · Código de clientes e fornecedores entre caixas sem rede — corrigido nesta rodada, com um limite (PEND-07).**
  Os códigos de números de clientes e fornecedores agora saem da faixa do caixa (a mesma das peças, com contagem própria), sem renumerar nada que existe. Veja o manual e os testes `j_*` em `faixas.rs`. O que resta está na PEND-07.

- **PEND-07 · Código de cliente/fornecedor com letras escolhido por dois caixas sem rede — corrigido nesta rodada.**
  Segue o princípio das peças (decisão do Mário): loja de um caixa só confirma na hora; com vários caixas o código espera a confirmação do caixa que distribui as faixas (`pessoa.codigo_validado`, migração 0016); sem conexão o cadastro é achado pelo nome e o código pendente não vale como único; duplicidade avisa o gerente, que dá um código novo; nada é renumerado em silêncio e os cadastros de antes valem como confirmados. Testes `k_*` e `j_migracao_*` em `faixas.rs`. Corrigido junto: alterar **só o código** de uma pessoa era descartado em silêncio. Limite que resta: o código antigo de uma pessoa não fica guardado como nas peças (não há etiqueta nem leitor para clientes).

- **PEND-08 · Cadastros feitos num banco copiado ficam com o identificador antigo — testado para peças e pessoas; segue manual de propósito.**
  Peças e pessoas criadas enquanto o caixa está bloqueado como cópia ficam com código a definir (os de letras, esperando) e com o identificador do caixa de antes. Depois de **Registrar como caixa novo**, o próprio caixa **não** os atribui sozinho: o identificador antigo é o mesmo do caixa original, então não dá para saber de quem é cada um. O gerente faz este caixa distribuir (**Este caixa passa a distribuir as faixas**) e usa **Dar código deste caixa a todos os cadastros a definir**; os códigos vêm de uma faixa que pula as de antes e os de letras são confirmados. Teste `l_pend08_pecas_e_pessoas_criadas_no_banco_copiado`.

- **PEND-02 · Código com letras digitado à mão em loja com vários caixas depende do caixa que dá as faixas.**
  Sem esse caixa por perto, o código fica "esperando" (a peça vende, mas sem etiqueta e sem leitor por esse código). É de propósito (não dá para garantir que seja único sem ele), mas, se o caixa que dá as faixas ficar muito tempo fora, o gerente precisa usar **Este caixa passa a distribuir as faixas** em outro caixa.

- **PEND-03 · Troca de placa-mãe, de sistema ou do aparelho muda a identidade do computador.**
  O banco passa a se comportar como cópia (não dá código, não troca dados) até o gerente usar **Registrar como caixa novo**. É o preço de não deixar uma cópia duplicar códigos em silêncio.

## Testes que ainda precisam ser feitos de verdade

- **PEND-04 · Teclado, leitor de código de barras e mouse.** As telas foram abertas sem janela (modo `offscreen`) e a lógica de qual peça entra, de a etiqueta sair e de o leitor achar tem teste automático, mas ninguém usou as telas com teclado, leitor e mouse reais.
- **PEND-05 · Windows, Android, celular físico e rede real entre computadores.** Os testes de rede usam o próprio computador (127.0.0.1). A identidade do computador no Windows e no Android vem do Qt (`QSysInfo::machineUniqueId`) e não foi conferida nesses sistemas.

## Entrega

- **PEND-06 · Assinatura do APK e artefatos antigos.** Antes de qualquer lançamento, confirmar a assinatura do APK e revisar os artefatos antigos que ainda possam trazer a chave pública conhecida. Os 4 APKs das versões 0.9.0 e 0.9.1 não devem ser apagados até se confirmar se algum celular tem dados reais.

## Fiscal

- **PEND-09 · R-01: a importação de XML não verifica a assinatura digital (bloqueador para confiar no estoque e no custo).**
  O Rustinho confere só a **coerência** entre chave, dígito, dados da nota, referência da assinatura, protocolo e `digVal == DigestValue` (textos copiados). Não calcula o resumo (SHA-1) do `infNFe` canonicalizado nem verifica a assinatura RSA / cadeia ICP-Brasil. Um XML com itens, quantidades ou valores mudados, mantendo a chave, o resumo e o protocolo, passa, entra no estoque e muda o custo. Hoje isso está **documentado** (manual, aviso fixo na tela da compra, mensagem de recusa e o teste `limite_conhecido_xml_adulterado_com_resumos_mantidos_ainda_passa`) e **não resolvido**.
  Para dar como resolvido: (1) canonicalização C14N do `infNFe` e conferência do `DigestValue`; (2) verificação da assinatura RSA com o certificado do XML; (3) cadeia até a ICP-Brasil (ou, no mínimo, validade e emissor); (4) testes com XMLs adulterados (item, quantidade, valor, chave, protocolo, certificado trocado, assinatura de outra nota) que devem ser recusados; (5) inverter o teste do limite conhecido para exigir a recusa. Enquanto isso, o uso em produção fiscal não deve ser tratado como seguro.

## Sincronização

- **PEND-10 · A sincronização deve funcionar sem internet, por todas as interfaces locais, priorizando cabo (Ethernet) e recorrendo ao Wi-Fi sem perder nem duplicar operações.**
  Pedido do usuário (20/10/2026). Hoje a troca de dados já funciona sem internet (rede local, `127.0.0.1` nos testes), mas: a descoberta por broadcast usa um só caminho, cada caixa guarda um único endereço "ip:porta", não há preferência cabo/Wi-Fi nem troca automática, e não há teste com mais de uma interface. Critérios de aceitação e plano de testes em `docs/PEND-10-criterios.md` (a criar antes do código). **Status: critérios em definição; sem código.**
