> **Frase-resumo:** "O Rustinho deve tornar simples o que é complexo por natureza, sem esconder a complexidade que precisa ser tratada corretamente."

> Documento de diretriz do projeto. Vale para todas as decisões futuras de código, interface, documentação e distribuição.

# DIRETRIZ CENTRAL DO RUSTINHO
## Simplicidade, consistência, linkagem e liberdade

### 1. Princípio fundamental: simples é melhor que complexo

O Rustinho deve buscar sempre a solução mais simples que seja tecnicamente correta, segura, confiável e fácil de manter.

Não adicionar camadas, dependências, configurações, telas, etapas ou abstrações sem uma necessidade concreta. Não complicar o sistema para resolver problemas que podem ser solucionados de maneira mais direta.

Simplicidade não significa eliminar recursos necessários, ignorar riscos ou esconder informações importantes. Significa resolver cada problema com o menor grau de complexidade possível, sem comprometer a integridade dos dados, a segurança, a confiabilidade ou a capacidade de evolução do projeto.

Toda complexidade introduzida deve ter uma justificativa técnica clara e um benefício que compense seu custo de implementação, manutenção e compreensão.

### 2. Núcleo estável e código compreensível

O núcleo Rust deve ser idiomático, modular, bem estruturado, testável e documentado. Suas responsabilidades devem estar claramente separadas, permitindo que novos recursos sejam implementados sem exigir mudanças desnecessárias em partes não relacionadas do sistema.

A interface deve continuar utilizando Qt 6 e QML, priorizando responsividade, consistência visual, simplicidade de uso e uma experiência limpa.

O código deve ser compreensível para outros desenvolvedores, com nomes claros, comentários que expliquem decisões relevantes e documentação técnica que reflita o funcionamento real do projeto.

Mudanças estruturais devem ser justificadas, testadas e realizadas de forma incremental. Não reescrever código funcional apenas por preferência estética ou para adotar uma tecnologia nova.

### 3. Linkagem: princípio universal de navegação contextual

A linkagem, conforme definida para o Rustinho, deve ser um princípio transversal a todo o sistema, aplicável a todas as pílulas, subpílulas, telas, formulários, notificações e funções, tanto para usuários comuns quanto para gerentes.

Quando uma ação, operação ou configuração depender de uma informação ausente, incorreta ou pendente, o sistema deve oferecer um caminho direto até o local exato em que o problema pode ser resolvido.

A linkagem deve conduzir o usuário à pílula, subpílula e campo correspondentes, sem obrigá-lo a procurar manualmente a configuração necessária ou descobrir sozinho onde realizar a correção.

O sistema deve preservar o contexto da operação original. Quando uma correção exigir sair temporariamente da tela atual, os dados já preenchidos, os arquivos selecionados e o progresso realizado devem ser preservados sempre que tecnicamente possível.

Após a correção e o salvamento, o sistema deve retomar o fluxo previsto para aquela ação, sem exigir que o usuário refaça etapas já concluídas.

Essa lógica não deve ser implementada apenas em casos isolados. Ela deve constituir um padrão de comportamento consistente em todo o Rustinho.

### 4. Linkagem e autorização de gerente

A linkagem deve funcionar mesmo quando a correção exigir privilégios superiores aos do usuário atual.

Se um usuário comum tentar executar uma operação permitida somente a um gerente, o sistema deve solicitar a autorização necessária no contexto da própria operação, preferencialmente por meio da senha do gerente.

A autorização não deve exigir uma troca desnecessária de usuário nem alterar silenciosamente a sessão ativa. O sistema deve identificar e registrar corretamente quem solicitou a operação e quem a autorizou, conforme os requisitos de auditoria.

Concedida a autorização, o gerente deve poder realizar a ação necessária sem que o usuário seja obrigado a navegar por caminhos adicionais ou reiniciar o procedimento original.

A autorização deve se limitar à operação autorizada. Não deve conceder privilégios permanentes ao usuário comum nem modificar seu papel no sistema.

### 5. Notificações integradas à navegação

As notificações centralizadas existem na tela inicial do Rustinho. Elas devem funcionar como pontos de entrada para as ações que precisam ser realizadas, não apenas como mensagens informativas.

Ao clicar em uma notificação, o sistema deve abrir diretamente a pílula e a subpílula correspondentes e, quando aplicável, posicionar o usuário no registro ou campo que exige atenção.

O fluxo deve funcionar da seguinte maneira:

1. A notificação aparece na tela inicial.
2. O usuário clica nela.
3. O sistema abre o local exato em que a ação necessária pode ser realizada.
4. Se houver restrição de privilégios, solicita a autorização do gerente sem alterar indevidamente a sessão ativa.
5. O usuário ou gerente realiza a correção e salva.
6. Após o salvamento bem-sucedido, o sistema retorna automaticamente à tela inicial, onde as notificações são centralizadas.

O sistema deve atualizar o estado da notificação de acordo com o resultado real da operação. Uma notificação não pode ser considerada resolvida apenas porque foi aberta ou porque o usuário tentou salvar uma alteração.

Se o salvamento falhar, a falha deve ser informada e os dados não salvos devem ser preservados sempre que possível. A notificação deve continuar pendente enquanto a condição que a originou não estiver efetivamente resolvida.

Esse comportamento deve ser consistente para notificações informativas, de atenção e críticas, respeitando a política específica de cada categoria.

### 6. Notificações proporcionais à gravidade

As notificações devem ser curtas, claras e orientadas à ação. Sua gravidade deve determinar o comportamento, não apenas a cor utilizada.

- **Informativas:** podem ser ocultadas ou adiadas conforme as regras definidas para a categoria.
- **De atenção:** podem ser minimizadas, mas permanecem pendentes até que a condição correspondente seja resolvida.
- **Críticas:** permanecem visíveis até a resolução efetiva, sem possibilidade de descarte indevido.

Erros e condições importantes devem ser descritos em linguagem compreensível, evitando mensagens técnicas que obriguem o usuário a interpretar logs ou procurar ajuda para entender o próximo passo.

### 7. Consistência e simplicidade de uso

As mesmas regras de navegação, autorização, salvamento, notificações e apresentação devem funcionar de maneira coerente em todo o sistema.

O Rustinho deve evitar perguntas repetidas, confirmações desnecessárias, telas intermediárias sem propósito e obrigações de memorização que possam ser resolvidas pela própria interface.

Ações reversíveis devem privilegiar mecanismos de desfazer quando seguros e viáveis. Confirmações devem ser reservadas principalmente para operações irreversíveis ou com consequências relevantes.

A interface deve responder imediatamente às ações do usuário, apresentar o andamento das operações demoradas e impedir submissões duplicadas enquanto uma operação estiver sendo processada.

A simplicidade visual não deve esconder informações necessárias, truncar textos importantes ou obrigar o usuário a adivinhar o significado de controles.

### 8. Segurança e confiabilidade acima de atalhos

A busca pela simplicidade não pode justificar perda de dados, permissões incorretas, corrupção do banco, conflitos silenciosos ou comportamentos imprevisíveis.

O núcleo, o SQLite, a sincronização, a recuperação de dados e as integrações externas devem possuir responsabilidades claras, mecanismos de validação e testes reproduzíveis.

Quando houver uma falha, o sistema deve informar o que aconteceu, preservar o que for possível e orientar o usuário para a ação seguinte. Não deve afirmar que uma operação foi concluída sem confirmação de seu resultado.

Integrações externas, como soluções fiscais, devem permanecer isoladas por interfaces bem definidas sempre que isso reduzir o acoplamento e facilitar sua substituição. Não introduzir complexidade adicional sem benefício técnico demonstrável.

### 9. Liberdade do desenvolvedor e independência do projeto

O Rustinho deve ser mantido como um projeto independente, de código aberto sob a licença MIT, com distribuição própria e documentação clara de suas dependências externas.

A licença deve permitir que outros desenvolvedores utilizem, modifiquem, redistribuam e incorporem o código a projetos próprios, inclusive comerciais e de código fechado, respeitadas as condições da MIT.

A liberdade de quem aproveita o projeto é parte de sua filosofia. As licenças de componentes externos devem ser respeitadas separadamente, sem atribuir ao Rustinho direitos que ele não possui sobre esses componentes.

A documentação deve distinguir fatos verificados, decisões arquiteturais, limitações conhecidas e questões ainda pendentes de confirmação.

### 10. Regra para todas as decisões futuras

Ao propor uma funcionalidade, alteração arquitetural, dependência ou mudança de interface, avaliar:

- O problema concreto que precisa ser resolvido.
- A solução mais simples que o resolve corretamente.
- Os riscos para segurança, integridade, desempenho e manutenção.
- A consistência com os padrões já adotados pelo sistema.
- A possibilidade de testar e documentar o comportamento.
- O custo de complexidade introduzido e sua justificativa.

Se duas soluções atenderem aos mesmos requisitos de maneira igualmente segura e confiável, deve-se preferir a mais simples, clara, enxuta e fácil de manter.

**Objetivo final:** construir um sistema comercial sólido e confiável, com uma interface limpa, navegação contextual integrada, um núcleo Rust bem estruturado e uma arquitetura que permita evoluir por muitos anos sem acumular complexidade desnecessária.
