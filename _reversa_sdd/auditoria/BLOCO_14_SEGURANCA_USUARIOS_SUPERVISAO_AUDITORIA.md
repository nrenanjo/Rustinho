# BLOCO 14 — Segurança, Usuários, Senhas, Supervisão, Registro e Auditoria

**Fase Reversa:** 3 (`escavacao` — Auditoria Completa dos Módulos Não Cobertos Anteriormente)  
**Bloco do Plano:** Bloco 14  
**Status:** CONCLUÍDO ✅  

---

## 1. Escopo Auditado no Bloco 14

Este bloco auditou integralmente (100% das linhas lidas) os 9 arquivos do domínio de **Segurança, Usuários, Política de Senhas, Supervisão de Threads, Registro Técnico com Redação de Segredos e Trilha de Auditoria** listados no `.reversa/plan.md`:

| # | Arquivo | Camada | Linhas Lidas | Testes Internos (`#[test]`) |
|---|---------|--------|--------------|-----------------------------|
| 1 | [`nucleo/src/usuarios.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/usuarios.rs#L1-L473) | Núcleo Rust | 1–473 (100%) | 8 testes (`linhas 399–472`) |
| 2 | [`nucleo/src/senhas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/senhas.rs#L1-L193) | Núcleo Rust | 1–193 (100%) | 4 testes (`linhas 130–192`) |
| 3 | [`nucleo/src/supervisao.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/supervisao.rs#L1-L262) | Núcleo Rust | 1–262 (100%) | — (testado em `nucleo/tests/estabilidade_suporte.rs`) |
| 4 | [`nucleo/src/registro.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/registro.rs#L1-L99) | Núcleo Rust | 1–99 (100%) | 1 teste (`linhas 86–98`) |
| 5 | [`nucleo/src/auditoria.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/auditoria.rs#L1-L187) | Núcleo Rust | 1–187 (100%) | — |
| 6 | [`app/qml/TelaEntrar.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaEntrar.qml#L1-L152) | Interface QML | 1–152 (100%) | — |
| 7 | [`app/qml/TelaBoasVindas.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaBoasVindas.qml#L1-L143) | Interface QML | 1–143 (100%) | — |
| 8 | [`app/qml/TelaUsuarios.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaUsuarios.qml#L1-L279) | Interface QML | 1–279 (100%) | — |
| 9 | [`app/qml/PerguntaMotivo.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/PerguntaMotivo.qml#L1-L66) | Interface QML | 1–66 (100%) | — |

**Total auditado neste bloco:** 9 arquivos (`1.854` linhas de código-fonte). Também foram verificados os pontos de chamada correspondentes em [`nucleo/src/api.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/api.rs#L816-L1070), [`nucleo/src/api.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/api.rs#L1428-L1470) e [`nucleo/src/api.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/api.rs#L2685-L2912).

---

## 2. Revisão e Cruzamento com Achados Históricos e de Blocos Anteriores

- **`D-07` / `D-09` (Permissões de Gerente, Liberação por Chamada e Sessão Desativada):**
  - Confirmado que [`api::atualizar_sessao`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/api.rs#L2804-L2816) verifica `u.ativo` a cada comando conectado e encerra imediatamente a sessão caso o usuário tenha sido desativado local ou remotamente.
  - Confirmado em [`PerguntaMotivo.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/PerguntaMotivo.qml#L31-L45) que o componente oficial de motivo passa os dois argumentos esperados `(campo.texto, resposta => motivo.terminar(resposta))` e verifica `if (r !== undefined) terminar(r)`, comprovando por contraste o defeito **`[NEW-B10-02]`** em [`TelaReceber.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaReceber.qml#L857-L892), onde um `Popup { id: motivo }` local foi duplicado sem o segundo argumento de callback `pronto`.
- **`F-17` (Política de Senhas sem Bloqueio Retroativo):**
  - Confirmado em [`senhas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/senhas.rs#L1-L127) e [`usuarios::avaliar_no_login`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/usuarios.rs#L351-L396) que senhas legadas fora da política atual apenas gravam marcador em `config` (`senha_fraca:<id>`) e aviso na tela inicial, sem impedir login nem alterar cadeados de backup existentes.
- **`O-3` / `RVI-02` (Supervisão de Tarefas de Sincronização):**
  - Confirmado em [`supervisao.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/supervisao.rs#L96-L261) que pânicos em threads supervisionadas são capturados via `catch_unwind`, registrados com mensagem higienizada (`limpar_panico`), submetidos a backoff exponencial (`1s` a `30s`) e expostos em `situacao(banco)` após `MAX_FALHAS_SEGUIDAS` (5) ou durante novas tentativas (`tentando`).
- **`C-15` (Redação de Segredos no Log Técnico `rustinho.log`):**
  - Confirmado em [`registro.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/registro.rs#L48-L84) que `limpar_segredos` mascara chaves `senha`/`token`/`secret`/`authorization`/`cookie`/`chave`/`pin` e sequências de 11 ou 14 dígitos (CPF/CNPJ), além de rotações de arquivo acima de `512 KB` (`MAX_BYTES`).

---

## 3. Análise Técnica Detalhada por Arquivo

### 3.1. [`nucleo/src/usuarios.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/usuarios.rs#L1-L473)
- **Responsabilidade:** Cadastro, listagem, autenticação Argon2 (`Argon2::default()` com `OsRng`), controle de tentativas de login persistidas no banco (`tentativa_login`), definição de comissão (`comissao_cent`), papel de gerente (`gerente`) e avaliação de conformidade de senha (`senha_fraca:<id>`).
- **Constantes:** `MIN_SENHA = 4` (`linha 15`).
- **Pontos de atenção identificados:**
  - Em [`usuarios::alterar`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/usuarios.rs#L99-L125), o `UPDATE usuario SET nome = ?2, ativo = ?3, atualizado_em = ?4 WHERE id = ?1` (`linhas 116–119`) é executado **antes** de `validar_senha(senha)?` (`linha 121`) e sem transação interna, seguido de um segundo `UPDATE usuario SET senha_hash = ?2 WHERE id = ?1` (`linha 122`) que não atualiza `atualizado_em` (**`[NEW-B14-01]`** e **`[NEW-B14-02]`**).
  - [`usuarios::trocar_senha`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/usuarios.rs#L144-L160) não é utilizada pelo comando `"senha_trocar"` da API ([`api.rs:1445–1470`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/api.rs#L1445-L1470)), que chama `usuarios::alterar` diretamente (**`[NEW-B14-05]`**).
  - [`usuarios::registrar_acesso`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/usuarios.rs#L169-L172) faz `UPDATE usuario SET ultimo_acesso = ?2 WHERE id = ?1` com gatilhos de sync ativos (`aplicando = 0`), disparando `sync_usuario_u` a cada login (**`[NEW-B14-03]`**).

### 3.2. [`nucleo/src/senhas.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/senhas.rs#L1-L193)
- **Responsabilidade:** Política de senhas para usuários, gerentes e rede/backup.
- **Regras implementadas:**
  - Mínimo de 4 caracteres para caixa/atendente e 6 caracteres para gerente/rede/backup (`linhas 36–47`).
  - Rejeita todos os caracteres iguais (`todos_iguais`, ex.: `"1111"`, `"aaaaaa"` — `linhas 86–94`).
  - Rejeita sequências numéricas estritas crescentes ou decrescentes de passo 1 (`sequencia_numerica`, ex.: `"1234"`, `"4321"`, `"123456"` — `linhas 97–111`).
  - Rejeita lista de senhas banais (`COMUNS` — `linhas 6–20`), igualdade ao nome da loja ou ao nome/primeiro nome do usuário (`linhas 54–78`).

### 3.3. [`nucleo/src/supervisao.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/supervisao.rs#L1-L262)
- **Responsabilidade:** Wrapper de supervisão de threads em background (`supervisionar`) com captura de pânico (`catch_unwind`), higienização de mensagens de pânico (`limpar_panico` delegando a `registro::limpar_segredos`), retentativa até `MAX_FALHAS_SEGUIDAS = 5` com espera exponencial (`ESPERA_INICIAL_MS = 1_000` até `ESPERA_MAXIMA_MS = 30_000`), e registro global por caminho de banco (`PARADAS` e `TENTANDO`).

### 3.4. [`nucleo/src/registro.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/registro.rs#L1-L99)
- **Responsabilidade:** Gravação em `rustinho.log` com rotação em `rustinho.log.antigo` ao atingir `512 KB`, permissões restritas ao usuário do SO (`arquivos::so_do_usuario`) e redação de segredos (`limpar_segredos`).

### 3.5. [`nucleo/src/auditoria.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/auditoria.rs#L1-L187)
- **Responsabilidade:** Gravação imutável na tabela `auditoria` (`registrar`) e formatação legível para a aba Histórico (`listar`, `descrever`).
- **Ponto de atenção identificado:**
  - [`auditoria::descrever`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/auditoria.rs#L110-L186) não possui braços específicos no `match acao` para diversas ações efetivamente gravadas por `auditoria::registrar` no código (`"codigos.assumir"`, `"codigos.definir"`, `"codigos.computador"`, `"codigos.caixa_novo"`, `"conflito.manter_todos"`, `"usuario.senha_fora_da_politica"`, `"pessoa.codigo"`), caindo no fallback genérico `"registrou"` (**`[NEW-B14-04]`**).

### 3.6. [`app/qml/TelaEntrar.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaEntrar.qml#L1-L152), [`app/qml/TelaBoasVindas.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaBoasVindas.qml#L1-L143), [`app/qml/TelaUsuarios.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaUsuarios.qml#L1-L279) e [`app/qml/PerguntaMotivo.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/PerguntaMotivo.qml#L1-L66)
- **`TelaEntrar.qml`:** Lista usuários ativos via `usuarios_entrada`, exibe cartões de seleção e campo de senha com submissão ao pressionar Enter. Na linha 94, a expressão `color: cartao.marcado ? Tema.dourado : Tema.dourado` possui ambos os ramos do operador ternário idênticos (**`[NEW-B14-06]`**).
- **`TelaBoasVindas.qml`:** Fluxo inicial para criar loja nova (`configurar_inicio`) ou entrar em loja existente (`abrirJuntar`).
- **`TelaUsuarios.qml`:** Lista usuários (`usuarios_listar`), permite criar/editar usuário (`usuario_salvar` com suporte a `pedirGerente`) e exibe aviso de senha fora da política (`senhaFraca`).
- **`PerguntaMotivo.qml`:** Diálogo modal padrão para coleta de justificativa antes de operações sensíveis ou protegidas por gerente.

---

## 4. Novos Achados Identificados no Bloco 14

### `[NEW-B14-01]` — `usuarios::alterar` executa o `UPDATE` de nome/ativo antes de validar a nova senha e sem transação própria
- **Classificação:** `Defeito comprovado`
- **Severidade:** Baixa
- **Evidência:**
  - [`nucleo/src/usuarios.rs:116-123`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/usuarios.rs#L116-L123):
    ```rust
    conn.execute(
        "UPDATE usuario SET nome = ?2, ativo = ?3, atualizado_em = ?4 WHERE id = ?1",
        params![id, nome, ativo as i32, tempo::agora()],
    )?;
    if let Some(senha) = nova_senha {
        validar_senha(senha)?;
        conn.execute("UPDATE usuario SET senha_hash = ?2 WHERE id = ?1", params![id, gerar_hash(senha)?])?;
    }
    ```
- **Análise Técnica:**
  - `validar_senha(senha)?` (`linha 121`) só é chamada **após** o primeiro `conn.execute("UPDATE usuario SET nome = ?2, ativo = ?3, atualizado_em = ?4 WHERE id = ?1", ...)` (`linhas 116–119`).
  - Embora os dois chamadores atuais na API (`"usuario_salvar"` em `api.rs:2847–2909` e `"senha_trocar"` em `api.rs:1454–1468`) abram uma transação explícita (`let tx = self.banco.conn.transaction()?;`) e chamem `exigir_politica` antes, qualquer chamada direta a `usuarios::alterar` em uma conexão sem transação aberta deixará `nome`, `ativo` e `atualizado_em` gravados no banco caso `validar_senha` ou `gerar_hash` retorne erro. Além disso, o segundo `UPDATE` (`linha 122`) altera `senha_hash` sem atualizar `atualizado_em` caso os dois `UPDATE`s ocorram em segundos diferentes.

---

### `[NEW-B14-02]` — Edição ou troca de senha de usuário executa múltiplos `UPDATE usuario` separados na mesma operação, multiplicando registros na fila de sincronização (`alteracao`)
- **Classificação:** `Defeito comprovado`
- **Severidade:** Baixa
- **Evidência:**
  - [`nucleo/src/usuarios.rs:116-123`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/usuarios.rs#L116-L123) (2 `UPDATE usuario` em `alterar`: um para `nome`/`ativo` e outro para `senha_hash`).
  - [`nucleo/src/api.rs:2869-2895`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/api.rs#L2869-L2895) (`usuario_salvar` chama em sequência `usuarios::alterar`, `usuarios::definir_comissao`, `usuarios::definir_gerente` e `backup::senha_trocada` / `backup::deixou_de_ser_gerente`).
  - [`nucleo/src/api.rs:1454-1458`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/api.rs#L1454-L1458) (`"senha_trocar"` chama `usuarios::alterar(&tx, &u.id, &u.nome, u.ativo, Some(&d.nova))` seguido de `backup::senha_trocada`).
  - [`nucleo/migracoes/0016_sync.sql:70-88`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/migracoes/0016_sync.sql#L70-L88) (gatilho `sync_usuario_u` dispara `AFTER UPDATE ON usuario` e insere uma linha completa em `alteracao` para cada comando `UPDATE`).
- **Análise Técnica:**
  - No comando `"senha_trocar"`, embora `nome` e `ativo` não mudem, `usuarios::alterar` executa um `UPDATE` para `nome`/`ativo`, outro `UPDATE` para `senha_hash`, e `backup::senha_trocada` executa um terceiro `UPDATE` para `backup_acesso` (quando gerente). Isso gera **3 linhas distintas em `alteracao`** (cada uma com o JSON completo da tabela `usuario`) para uma única troca de senha.
  - Em `"usuario_salvar"`, salvar um usuário gerente com nova senha e comissão dispara até **5 `UPDATE usuario` consecutivos** (e 5 linhas em `alteracao`) dentro da mesma transação.

---

### `[NEW-B14-03]` — `usuarios::registrar_acesso` atualiza `usuario.ultimo_acesso` com gatilho de sincronização ativo, replicando o registro completo do usuário a cada login
- **Classificação:** `Risco`
- **Severidade:** Baixa
- **Evidência:**
  - [`nucleo/src/usuarios.rs:169-172`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/usuarios.rs#L169-L172):
    ```rust
    pub fn registrar_acesso(conn: &Connection, id: &str) -> Resultado<()> {
        conn.execute("UPDATE usuario SET ultimo_acesso = ?2 WHERE id = ?1", params![id, tempo::agora()])?;
        Ok(())
    }
    ```
  - [`nucleo/src/api.rs:2751`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/api.rs#L2751): chamado em todo login bem-sucedido (`entrar`).
- **Análise Técnica:**
  - Como `sync_controle.aplicando` está em `0` durante o login e o gatilho `sync_usuario_u` ([`0016_sync.sql:70-88`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/migracoes/0016_sync.sql#L70-L88)) não possui cláusula `OF ...` restritiva, todo login em qualquer caixa da loja gera um evento `alteracao` na tabela `usuario` (contendo `senha_hash`, `backup_acesso`, etc.) que é transmitido via rede para todos os demais caixas apenas para atualizar `ultimo_acesso`. Além disso, como `registrar_acesso` não atualiza `atualizado_em`, se um usuário apenas fizer login após uma alteração concorrente, o pacote de sync enviado carrega o `atualizado_em` antigo.

---

### `[NEW-B14-04]` — `auditoria::descrever` não mapeia 7 códigos de ação gravados pelo núcleo, exibindo texto genérico `"registrou"` no Histórico
- **Classificação:** `Risco`
- **Severidade:** Baixa
- **Evidência:**
  - [`nucleo/src/auditoria.rs:110-186`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/auditoria.rs#L110-L186):
    ```rust
    fn descrever(acao: &str, entidade: &str) -> String {
        match acao {
            ...
            _ => "registrou".into(),
        }
    }
    ```
  - Ações gravadas via `auditoria::registrar` que não constam no `match` de `descrever`:
    1. `"codigos.assumir"` ([`api.rs:1170`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/api.rs#L1170))
    2. `"codigos.definir"` ([`api.rs:1187`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/api.rs#L1187))
    3. `"codigos.computador"` ([`api.rs:1204`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/api.rs#L1204))
    4. `"codigos.caixa_novo"` ([`api.rs:1226`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/api.rs#L1226))
    5. `"conflito.manter_todos"` ([`api.rs:1257`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/api.rs#L1257))
    6. `"usuario.senha_fora_da_politica"` ([`api.rs:2760`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/api.rs#L2760))
    7. `"pessoa.codigo"` ([`pessoas.rs:429`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/pessoas.rs#L429))
- **Análise Técnica:**
  - Embora os detalhes (`detalhe`) continuem visíveis ao expandir/ler o item na aba Histórico, a coluna de resumo (`resumo`) exibe apenas `"registrou"` para essas 7 ações administrativas.

---

### `[NEW-B14-05]` — Função pública `usuarios::trocar_senha` é código morto em produção e diverge do fluxo real de `"senha_trocar"` em `api.rs`
- **Classificação:** `Melhoria`
- **Severidade:** Baixa
- **Evidência:**
  - [`nucleo/src/usuarios.rs:144-160`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/usuarios.rs#L144-L160) define `pub fn trocar_senha(conn: &Connection, id: &str, senha_atual: &str, senha_nova: &str) -> Resultado<()>`.
  - [`nucleo/src/api.rs:1445-1470`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/api.rs#L1445-L1470) implementa `"senha_trocar"` chamando `usuarios::conferir_senha`, `usuarios::exigir_politica` e `usuarios::alterar`, sem chamar `usuarios::trocar_senha`.
- **Análise Técnica:**
  - `usuarios::trocar_senha` só é chamada pelo teste unitário `trocar_senha_exige_atual` ([`usuarios.rs:442-466`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/usuarios.rs#L442-L466)) e aplica apenas `validar_senha` (tamanho mínimo 4), sem chamar `exigir_politica` nem atualizar `atualizado_em`.

---

### `[NEW-B14-06]` — Expressão ternária redundante em `TelaEntrar.qml` (`cartao.marcado ? Tema.dourado : Tema.dourado`)
- **Classificação:** `Melhoria`
- **Severidade:** Baixa
- **Evidência:**
  - [`app/qml/TelaEntrar.qml:94`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaEntrar.qml#L94):
    ```qml
    color: cartao.marcado ? Tema.dourado : Tema.dourado
    ```
- **Análise Técnica:**
  - Os dois ramos do operador ternário retornam exatamente `Tema.dourado`.

---

## 5. Registro Separado: Código Lido vs. Testes Encontrados vs. Testes Executados

### 5.1. Código Lido Integralmente (100%)
- `nucleo/src/usuarios.rs` (473 linhas)
- `nucleo/src/senhas.rs` (193 linhas)
- `nucleo/src/supervisao.rs` (262 linhas)
- `nucleo/src/registro.rs` (99 linhas)
- `nucleo/src/auditoria.rs` (187 linhas)
- `app/qml/TelaEntrar.qml` (152 linhas)
- `app/qml/TelaBoasVindas.qml` (143 linhas)
- `app/qml/TelaUsuarios.qml` (279 linhas)
- `app/qml/PerguntaMotivo.qml` (66 linhas)

### 5.2. Testes Encontrados e Lidos nos Módulos Auditados
- Em [`nucleo/src/usuarios.rs:399-472`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/usuarios.rs#L399-L472) (8 testes):
  - `criar_e_entrar`
  - `nao_desativa_ultimo_ativo`
  - `trocar_senha_exige_atual`
  - `senha_Antiga_fora_da_politica_entra_e_marca_aviso`
  - `avaliar_no_login_limpa_aviso_quando_senha_passa_a_valer`
  - `avaliar_no_login_respeita_papel_de_gerente`
  - `exigir_politica_recusa_senha_fraca_com_campo_indicado`
  - `tentativas_erradas_bloqueiam_e_limpam`
- Em [`nucleo/src/senhas.rs:130-192`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/senhas.rs#L130-L192) (4 testes):
  - `rejeita_curtas`
  - `rejeita_repetidas_e_sequencias`
  - `rejeita_comuns_loja_e_usuario`
  - `aceita_senhas_razoaveis`
- Em [`nucleo/src/registro.rs:86-98`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/registro.rs#L86-L98) (1 teste):
  - `limpa_senha_token_e_documento`

### 5.3. Testes Executados Neste Bloco
- **Nenhum teste foi executado neste bloco.** Conforme o `.reversa/plan.md`, a execução da suíte completa de testes está concentrada e isolada no **Bloco 18**.

