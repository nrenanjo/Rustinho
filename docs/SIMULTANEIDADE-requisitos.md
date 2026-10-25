# Simultaneidade, reconexão e central de notificações — requisitos aprovados (24/10/2026)

Status: requisitos aprovados pelo dono. Nenhum código implementado. Implementação só após o comando "começar" e com Opus 5.5 em esforço alto.

## Decisões aprovadas
1. **Reconexão:** intervalos de 5 s, dobrando a cada falha, até o limite de 5 min. A contagem zera quando a rede muda.
2. **Rede local e Internet separadas:** queda da Internet não interrompe a sincronização entre caixas pela rede local. Quando a Internet voltar, o Google Drive é retomado automaticamente.
3. **Operação pendente:** deixa de ser pendente só após confirmação durável de outra réplica. Reconexão e retomada não podem gerar duplicatas nem perder alterações.
4. **Nome do caixa:** usar o nome configurado (ex.: "Caixa 1"). Não usar "PC principal". Na tela do próprio dispositivo pode aparecer discretamente "Este computador" como identificação contextual.
5. **Categorias de notificação** (por tipo e gravidade, não por cor):
   - **Informativo:** pode ser ocultado até amanhã (ex.: "A pagar").
   - **Atenção:** pode ser minimizado, mas continua pendente na central, com indicador visível.
   - **Crítico:** permanece destacado até a resolução, sem descarte manual (ex.: falha de integridade, problema de segurança, caixa sem autorização para gerar códigos).
6. **Ação direta:** cada notificação leva à função responsável. Quando necessário, pede a senha do gerente sem trocar o usuário ativo, e retorna à tela de origem preservando campos e trabalho em andamento.

## Lista de tarefas
- [ ] Desenho técnico da reconexão com backoff e detecção separada de rede local e Internet.
- [ ] Modelo de pendências com estado "pendente de replicação" e confirmação durável.
- [ ] Classificação de notificações por tipo e gravidade, com as três categorias acima.
- [ ] Central de notificações na tela inicial, reaproveitando as faixas atuais.
- [ ] Navegação direta da notificação para a função responsável, com retorno à tela de origem.
- [ ] Renomear a identificação do caixa conforme a regra acima.
- [ ] Testes de regressão e de aceitação abaixo.

## Testes de aceitação
1. Venda sem rede: registrada e aparece como pendente na tela inicial.
2. Restabelecer a rede: reconexão e replicação automáticas.
3. Queda só da Internet: a rede local continua sincronizando entre caixas; o Drive retoma ao voltar.
4. Queda só da rede local: a Internet e o Drive continuam; a sincronização local retoma ao voltar.
5. Várias tentativas de reconexão: nenhuma operação duplicada.
6. Notificações de todas as áreas aparecem na tela inicial e abrem o destino correto.
7. Informativo: pode ser ocultado até amanhã.
8. Atenção: minimizado continua pendente, com indicador visível.
9. Crítico: não pode ser descartado até resolvido.
10. Resolução preserva o trabalho em andamento e não exige troca de usuário.
11. Nome do caixa conforme a regra; nenhuma tela usa "PC principal".

## Decisões que fecham a especificação (24/10/2026)

### D1. Entrada de novo dispositivo
- Aprovado: um gerente autorizado confirma a entrada com a própria senha.
- A autorização é autenticada e registrada na auditoria.
- Só depois o novo dispositivo recebe as credenciais criptográficas.
- A entrada não exige computador principal.

### D2. Recuperação das chaves
- Código de recuperação gerado na criação da loja, impresso e guardado fora da loja.
- Permite recuperar as chaves sem depender de um computador ou de um único gerente.
- Chaves nunca em texto aberto.
- Perda do código é tratada explicitamente: o sistema informa a limitação, sem prometer recuperação impossível.

### D3. Réplicas
- O Google Drive conta como réplica independente.
- Só vale após o Rustinho confirmar que a operação criptografada foi armazenada e registrar essa confirmação. Notificação de alteração não basta.
- Drive fora do ar com dois caixas: a venda continua permitida e fica pendente até o outro caixa confirmar.
- Nenhuma réplica disponível: registro local com estado explícito de proteção pendente.

### D4. Chaves no Linux
- Usar o serviço de senhas do sistema (Secret Service, GNOME Keyring ou KWallet) quando disponível.
- Sem serviço: avisar o gerente e oferecer alternativa, como armazenamento criptografado protegido por senha.
- Nunca guardar chave privada desprotegida silenciosamente.
- Operação local continua quando possível, com limitações de sincronização explícitas até as chaves serem usadas com segurança.

### Diretriz
Nenhum computador principal obrigatório; nenhuma concessão indevida de privilégios; nenhum dado declarado protegido sem confirmação; nenhuma perda silenciosa de operações.

### Tarefas adicionais
- [ ] Fluxo de entrada de dispositivo com senha de gerente e auditoria (D1).
- [ ] Geração, impressão e validação do código de recuperação na criação da loja (D2).
- [ ] Registro de confirmação de armazenamento no Drive antes de marcar a operação como protegida (D3).
- [ ] Integração com o serviço de senhas do Linux e alternativa protegida por senha (D4).

### Testes de aceitação adicionais
12. Entrada de dispositivo sem senha de gerente válida é recusada e não recebe credenciais.
13. Entrada autorizada aparece na auditoria.
14. Recuperação de chaves com o código funciona sem o computador original.
15. Perda do código: o sistema informa a limitação e não declara recuperação.
16. Upload no Drive sem confirmação verificada não marca a operação como protegida.
17. Drive indisponível com dois caixas: venda permitida, pendente até replicação confirmada.
18. Sem nenhuma réplica: estado "proteção pendente" visível.
19. Linux sem serviço de senhas: aviso ao gerente e nenhuma chave privada em texto aberto.
