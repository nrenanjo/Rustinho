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
