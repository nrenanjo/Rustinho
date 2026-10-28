# Proposta: D1 (aprovação de caixa novo) e R-01 (senha da rede) — protocolo 4

**Situação (28/10/2026):** proposta para decisão do dono. **Nada disto foi implementado.** O protocolo continua o 3. R-01 e D1 continuam **bloqueadores do uso comercial** (PEND-24, PEND-25).

Por que parar aqui: as duas exigem mudar o protocolo da troca de dados, e o briefing manda apresentar o desenho antes de qualquer mudança de protocolo, sem criar algoritmo próprio e sem declarar o R-01 resolvido com algo que só dificulte tentativas online.

## 1. O problema (com a evidência de hoje)

- **R-01:** a chave da troca vem da senha da rede (Argon2id, `sync.rs::derivar`), com o sal em claro em cada mensagem. O anúncio UDP (porta 47811) sai a cada 10 s cifrado com essa chave. Quem grava um anúncio na rede da loja testa senhas no próprio computador (ataque **offline**); o limite de tentativas do Rustinho (R-09) não alcança isso. Com o mínimo aprovado de 4 caracteres, o espaço é pequeno.
- **D1:** a senha da rede é a **única** credencial. Quem a tem e um banco vazio entra como caixa novo e recebe a loja inteira. A identidade da loja e a loja escolhida (FALHA-01) impedem misturar **duas lojas**, mas não impedem um caixa novo **desta** loja sem aprovação do gerente.
- Hoje não há credencial por computador: não dá para aprovar, revogar ou substituir um caixa de verdade.

## 2. Requisitos (do briefing e de `docs/SIMULTANEIDADE-requisitos.md`)

1. Cada computador identificado por criptografia (par de chaves próprio).
2. O gerente da loja existente autoriza a entrada de um caixa novo; a senha da rede não é autorização.
3. Caixa vazio não entra em loja nenhuma só por conhecer a senha.
4. Junção não cria gerente local; lojas independentes continuam isoladas; banco copiado/restaurado legítimo continua.
5. Revogação, recuperação e substituição de dispositivo.
6. Autorização registrada e auditável.
7. Aprovação conferida **antes** de aceitar qualquer dado do caixa novo.
8. Nada capturado na rede permite testar hipóteses de senha offline.
9. Só mecanismos padronizados e revisados; nada de criptografia própria.
10. Mudança de protocolo explícita, com convivência e recusa segura de caixas antigos.

## 3. Desenho proposto

### 3.1 Chave de cada computador
- Na primeira abertura, cada caixa gera um par **Ed25519** (assinatura) e um par **X25519** (troca de chaves) com `ring`/`x25519-dalek` (já usados no projeto).
- A parte privada fica **só neste computador**: no Linux, no serviço de senhas do sistema quando houver (D4); sem ele, num arquivo próprio protegido (0600) e cifrado com uma chave derivada de um segredo local — e o gerente é avisado (D4 exige não guardar chave em claro em silêncio). No Windows, DPAPI; no Android, Keystore. **A decidir** (seção 6).
- O banco copiado para outro computador **não** leva a chave privada → continua o bloqueio de cópia de hoje; "Registrar como caixa novo" passa a exigir aprovação (3.3).

### 3.2 Registro de caixas da loja (assinado, replicado)
- Tabela nova `caixa_aprovado` (replicada): chave pública Ed25519 e X25519, nome, quem aprovou (chave do caixa aprovador + usuário gerente), quando, e a **assinatura** do caixa aprovador sobre tudo isso.
- Raiz: o primeiro caixa da loja se registra sozinho (assinatura própria) e a identidade da loja (`loja_id`) passa a ser o resumo da chave dele. Lojas da 0.9.15 sem chaves: o caixa distribuidor das faixas vira a raiz na migração (é único por loja e replicado), e os outros caixas da loja que já trocavam dados são aprovados uma vez pelo gerente, com aviso no Início.
- Revogação: registro `caixa_retirado` assinado por um caixa aprovado com senha do gerente ("Retirar caixa", que já existe, passa a ser criptográfico).
- Cada caixa só aceita um registro novo se a cadeia de assinaturas chega à raiz e nenhum elo foi retirado.

### 3.3 Entrada de caixa novo (D1)
1. No caixa novo: "Entrar numa loja que já existe" mostra um **código de pedido** (resumo curto da chave pública dele, como "4F2K-9QXA").
2. Num caixa da loja, o gerente abre **Simultaneidade › Caixas › Aprovar caixa novo**, digita a **senha de gerente**, confere o código de pedido que aparece na tela do caixa novo e aprova.
3. O caixa da loja gera um **convite de uso único** (≥ 128 bits aleatórios, validade de 10 minutos), mostrado como **QR** (a tela já lê códigos com a câmera) e como texto de 26 caracteres.
4. O caixa novo lê o convite. A conexão de entrada usa o convite como segredo forte num canal **Noise** (padrão `XXpsk3`) ou TLS 1.3 com chave pré-compartilhada; como o convite tem ≥ 128 bits e vale uma vez só, gravar essa conexão não ajuda ninguém a adivinhar nada.
5. Pelo canal: o caixa da loja confere que a chave pública do caixa novo bate com o código de pedido que o gerente viu, grava o `caixa_aprovado` assinado (com o usuário gerente, na auditoria) e só então entrega os dados. Sem aprovação, nada é entregue (requisito 7).
6. Queda de conexão no meio: o convite continua valendo até expirar ou ser usado; repetir não aprova duas vezes (a aprovação é uma linha por chave pública).

### 3.4 Troca de dados entre caixas aprovados (R-01)
- Cada conexão TCP passa a ser autenticada **pelas chaves dos dois caixas** (Noise `KK` ou `IK` com as chaves X25519 do registro, ou TLS 1.3 com certificados próprios fixados pelas chaves do registro). A senha da rede deixa de proteger a troca.
- O anúncio UDP passa a levar só o identificador do caixa, a identidade da loja e a porta, **assinados** (Ed25519) — nada cifrado com chave derivada de senha. Um anúncio gravado não serve para testar senha nenhuma (requisito 8).
- A senha da rede deixa de existir na versão 4 (ou vira só um rótulo para achar os caixas certos). Com isso o mínimo de 4 caracteres deixa de pesar no R-01 — o que resolve é tirar a senha do caminho criptográfico, não aumentar o mínimo.

### 3.5 Bibliotecas (todas padronizadas; nenhuma criptografia própria)
| Para | Opção | Situação conhecida (verificar de novo antes de implementar) |
|---|---|---|
| Assinatura | Ed25519 do `ring` (já no projeto) | Biblioteca muito usada (derivada da BoringSSL) |
| Troca de chaves | X25519 (`x25519-dalek`, já no projeto) | Já usada nas chaves do backup |
| Canal autenticado | **Noise** (`snow` 0.10) — padrões `XXpsk3` (entrada) e `KK` (troca) | O protocolo Noise é analisado formalmente; o README do `snow` diz que a biblioteca **não teve auditoria formal** |
| Canal autenticado (alternativa) | **TLS 1.3** (`rustls`) com certificados próprios fixados | `rustls` teve auditoria independente; precisa de um certificado por caixa (gerado localmente) |
| Se o dono preferir código curto em vez de QR | **OPAQUE** (RFC 9807, julho de 2025; `opaque-ke`, auditada pela NCC Group em 2021, versão auditada 0.5, correções na 1.2; a atual é a 4.x) | Ataque offline a um código curto fica impossível; mais peças para manter |

Recomendação: **TLS 1.3 com `rustls`** para a troca entre caixas (biblioteca auditada, já existe `rustls-webpki` no projeto) e **convite forte por QR** para a entrada (dispensa PAKE). Se a tela de QR não servir para todos os computadores (sem câmera), o convite em texto de 26 caracteres cobre.

### 3.6 Protocolo 4 e convivência
- `PROTOCOLO = 4`, recurso `chaves-v1`. Um caixa da versão 4 conversa com outro da versão 4 só pelo canal autenticado.
- Convivência: durante a passagem, um caixa 4 aceita caixa 3 **da mesma loja** (identidade e histórico, como hoje) só se o gerente ligar "permitir caixas antigos até DD/MM" (aviso no Início todos os dias). Caixa 3 de fora: recusado como hoje.
- Fim da convivência: o gerente desliga, ou acaba o prazo; aí caixa 3 é recusado com o motivo "atualize este caixa".
- Testes de interoperabilidade (3↔4 nos dois sentidos, sem perder alteração) e de recusa segura (3 depois do prazo; 4 sem aprovação; anúncio forjado; convite repetido; convite expirado; chave trocada no meio).

## 4. Testes de aceitação (antes de declarar D1 e R-01 resolvidos)
1. Caixa vazio com a senha antiga da rede: não recebe nada (D1).
2. Entrada aprovada: auditoria com o gerente, o caixa aprovador e o caixa novo.
3. Convite repetido, expirado, ou de outra loja: recusado; nada entregue.
4. Imitação: caixa que anuncia a identidade de outro sem a chave privada: recusado.
5. Queda no meio da aprovação: retoma sem duplicar; nenhum dado parcial.
6. Lojas independentes: continuam sem trocar.
7. Banco copiado: bloqueado até aprovação; banco restaurado no mesmo computador: continua (a chave privada é do computador).
8. Revogação: caixa retirado não troca mais; os dados dele que já chegaram continuam.
9. Anúncio gravado + dicionário de senhas: o teste registra que não há o que testar (nenhum campo derivado de senha).
10. Convivência 3↔4 e fim da convivência.
11. Celular de entrada: tempo da conexão autenticada (medir).

## 5. O que muda para a loja
- Entrar com caixa novo: o gerente aprova no caixa da loja (código na tela + QR), em vez de só digitar a senha da rede.
- A senha da rede deixa de ser digitada nos caixas novos.
- Retirar caixa passa a valer de verdade (o retirado não troca mais nada).

## 6. Decisões do dono necessárias
1. Aprovar o desenho (chaves por computador, aprovação pelo gerente, protocolo 4).
2. Canal: TLS 1.3 (`rustls`, recomendado) ou Noise (`snow`, sem auditoria formal).
3. Entrada: convite forte por QR/texto (recomendado) ou código curto com OPAQUE.
4. Onde guardar a chave privada em cada sistema (D4) e o que fazer quando não houver serviço de senhas.
5. Prazo e regra da convivência com caixas 0.9.15.
6. Recuperação (D2): código de recuperação impresso na criação da loja.
