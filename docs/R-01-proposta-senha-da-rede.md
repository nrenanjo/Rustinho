# R-01 (auditoria da 0.9.14) — proposta para a próxima etapa: senha da rede

**Situação:** aberto. Nada disto foi implementado na 0.9.15 (decisão do dono: esta rodada é de estabilização). Documento para a decisão e para a próxima etapa.

## O problema, exatamente

- A troca de dados entre caixas cifra cada mensagem com ChaCha20-Poly1305. A chave vem da **senha da rede** por Argon2id (`sync.rs`, `derivar`), com um sal de 16 bytes de cada caixa, que vai em claro no começo de cada mensagem.
- O anúncio de cada caixa sai a cada 10 s pela porta **UDP 47811**, para toda a rede local; a troca em si é pela **TCP 47810**.
- A política aprovada permite senha da rede com **4 caracteres quaisquer**.
- Quem estiver na rede da loja (Wi-Fi de clientes na mesma rede, por exemplo) pode **gravar um anúncio** e depois, no próprio computador, testar senhas: para cada tentativa, calcula o Argon2id com o sal do pacote e tenta abrir a mensagem (a etiqueta Poly1305 diz se acertou). Isso é um **ataque offline**: o Rustinho não vê as tentativas.
- **Limitar tentativas no programa (rate limiting) não resolve**: a espera depois de erros (`R-09` da 0.9.11, `SAIS_NOVOS_POR_MINUTO`) só protege contra quem testa senhas **contra um caixa**. Contra um pacote capturado, quem testa é o computador do atacante, sem limite.
- Com 4 caracteres, o espaço é pequeno: só com algarismos são 10 mil senhas; com letras minúsculas e algarismos, cerca de 1,7 milhão. O custo de cada tentativa é o do Argon2id com os parâmetros atuais; numa máquina com placa de vídeo ou vários núcleos isso cai para horas ou dias.
- Quem acerta a senha da rede pode receber todos os dados da loja (como um caixa novo) e, sem a D1, entrar como caixa da loja.

## Alternativas (da mais simples para a mais completa)

| # | O que muda | Compatível com o protocolo 3? | O que resolve | O que não resolve / risco que fica |
|---|---|---|---|---|
| 1 | **Mínimo maior só para a senha da rede** (ex.: 10 caracteres, ou frase de 3 palavras), com aviso para trocar a atual; a senha de entrada dos usuários continua 4 | Sim (é só a política ao salvar) | Aumenta o custo do ataque offline em ordens de grandeza | Depende de as lojas trocarem a senha; senhas fracas antigas ficam até trocar. Muda uma regra aprovada (precisa de decisão) |
| 2 | **Senha da rede gerada pelo Rustinho** (ex.: 6 palavras de uma lista, ~77 bits), mostrada para digitar nos outros caixas ou levada por QR code na junção | Sim | Torna o ataque offline inviável na prática | Muda o jeito de juntar caixas (digitar ou ler QR); precisa de tela nova |
| 3 | **Parâmetros do Argon2id mais caros** para a senha da rede (mais memória e passadas) | Não sozinho: os dois lados precisam derivar igual; exige versão/recurso novo ("kdf-v2") e período de convivência | Multiplica o custo de cada tentativa (talvez 10x a 50x) | Com 4 caracteres continua quebrável; pesa no celular de entrada (medir); não vale como solução única |
| 4 | **Anúncio sem nada que permita testar a senha**: o anúncio UDP passa a levar só um identificador aleatório (sem conteúdo cifrado com a chave da senha) | Não: muda o formato do anúncio; exige recurso novo e convivência com caixas antigos | Tira o pacote mais fácil de capturar (o que sai sozinho a cada 10 s) | A conexão TCP continua testável se o atacante conseguir provocá-la ou capturá-la; reduz, não elimina |
| 5 | **Troca de chaves autenticada por senha (PAKE, ex.: CPace ou OPAQUE)** no começo de cada conexão TCP, e anúncio sem conteúdo derivado da senha | Não: protocolo novo (versão 4) com convivência | Ataque offline deixa de existir: cada tentativa exige falar com um caixa de verdade, e aí o limite de tentativas vale | Trabalho maior; biblioteca auditada para PAKE em Rust precisa ser escolhida e testada |
| 6 | **Chaves por computador (a D1)**: cada caixa tem um par de chaves; o gerente da loja aprova o caixa novo; a senha da rede vira só convite | Não: é a arquitetura da D1 (anexo de decisões "chaves de dispositivo e assinatura") | Resolve também a autorização de caixa novo | É a etapa D1 inteira |

## Recomendação técnica para decidir

Curto prazo (sem mudar o protocolo): **1 ou 2**, com prioridade para 2, porque não depende da disciplina de cada loja. Médio prazo: **5 junto com 6 (D1)**, numa versão do protocolo com período de convivência. A alternativa 3 sozinha não é suficiente com 4 caracteres.

## Riscos residuais, mesmo depois

- Quem já tem a senha da rede (funcionário que saiu) continua podendo receber dados até a senha ser trocada em todos os caixas; com a D1, o gerente pode retirar o computador (já existe "retirar caixa", que hoje vale só entre caixas da loja).
- A rede local da loja continua sendo parte da segurança: separar o Wi-Fi de clientes da rede dos caixas reduz a exposição, com qualquer alternativa.

## Testes que a implementação terá de ter

- Ataque offline simulado: a partir de um anúncio gravado, um teste que tenta N senhas e mede o custo por tentativa (para registrar o número, não para "passar").
- Convivência: caixa com o protocolo novo e caixa antigo, nos dois sentidos, sem perder alteração.
- Troca da senha da rede com caixas desligados (o que acontece quando voltam).
- Celular de entrada: tempo da derivação com os parâmetros escolhidos.
