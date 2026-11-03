# BLOCO 12 — Auditoria de Sincronização P2P, Rede Local, Computador e Modo Portátil

**Data:** 2026-11-02
**Escopo:** Fase 3 — Bloco 12 do Plano de Auditoria (`.reversa/plan.md`)
**Status:** CONCLUÍDO ✅ (Somente leitura — nenhum arquivo do projeto foi modificado)

---

## 1. Arquivos Lidos Integralmente neste Bloco

| # | Arquivo | Linhas | Escopo Principal | Testes Embutidos (`#[test]`) |
|---|---|---|---|---|
| 1 | [sync.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/sync.rs) | 1–6430 (6.430) | Motor completo de sincronização P2P entre caixas: gatilhos SQLite (`alteracao`), relógio lógico (`seq`), mesclagem campo-a-campo (`mesclar_campos`), regras de imutabilidade (`IMUTAVEIS`), proteção de estoque por movimento (`estoque_atual`), conciliação pós-lote (`verificar_depois`), integridade referencial (`TABELAS_COM_REFERENCIAS`, `sync_referencia`, `exigir_editavel`), conflitos (`sync_conflito`), faixas de código (`faixa_codigo`), retirada de caixa (`caixa_retirado`), criptografia ChaCha20-Poly1305 + Argon2id (`Chaves`, `cifrar`, `decifrar`), transporte TCP/UDP (`Motor`, `anunciar`, `ouvir_anuncios`, `atender`, `juntar`) | 63 funções `#[test]` (`linhas 3440–6430`) |
| 2 | [rede_local.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/rede_local.rs) | 1–1037 (1.037) | Descoberta de interfaces de rede IPv4 sem dependências externas (`getifaddrs` via libc no Linux/Android e `/proc/net/wireless` + `/sys/class/net`; `GetAdaptersAddresses` via `iphlpapi.dll` no Windows), classificação `Cabo` / `Wifi` / `Virtual`, cálculo de broadcast da sub-rede, geração de 254 alvos `/24` para varredura TCP quando UDP falha | 13 funções `#[test]` (`linhas 802–1037`) |
| 3 | [computador.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/computador.rs) | 1–118 (118) | Descoberta e higienização do nome da máquina (`COMPUTERNAME` no Windows, `/etc/hostname` e `HOSTNAME` no Linux) para sugerir nome de caixa ao entrar numa loja existente | 3 funções `#[test]` (`linhas 86–118`) |
| 4 | [portatil.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/portatil.rs) | 1–143 (143) | Modo portátil para testes: detecção via arquivo marcador `portatil.txt` ao lado do executável ou variável `RUSTINHO_PORTATIL=1`, isolamento de pasta (`dados-portatil/`), portas exclusivas (`47821` TCP / `47820` UDP) e separação de senha da rede (`PREFIXO_SENHA_REDE = "rustinho-portatil-teste\0"`) | 2 funções `#[test]` (`linhas 118–143`) |
| 5 | [teste_rede.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/teste_rede.rs) | 1–185 (185) | Simulador de testes em memória/disco temporário para malha de sincronização multi-caixa (`CaixaTeste`, `par_sincroniza`, `malha_converge`, `tres_caixas_offline_convergem`, `conflito_simultaneo_preserva_perdedor`) | 4 funções `#[test]` (`linhas 83–185`) |
| 6 | [TelaJuntar.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaJuntar.qml) | 1–169 (169) | Tela de ingresso de um novo caixa em uma loja já existente (`gerente.juntar_a_loja`), com preenchimento automático de `gerente.nome_deste_computador()` e opção de endereço IP manual | N/A (QML) |

**Total auditado no Bloco 12:** 6 arquivos | **8.082 linhas** lidas integralmente | **85 funções `#[test]`** inspecionadas no código-fonte.

---

## 2. Revisão de Achados e Requisitos Históricos Relacionados

Embora o Gemini 4 Argon não tenha dedicado um bloco exclusivo ao motor interno de `sync.rs`, `rede_local.rs`, `computador.rs` e `portatil.rs` (tendo inspecionado apenas a UI de `SecaoRede.qml` no Bloco 7B e os requisitos R5 no Bloco 2–4), revisamos no código-fonte todas as referências históricas correlatas:

| ID / Referência Histórica | Descrição Original | Veredito no Bloco 12 | Evidência no Código-Fonte Atual (0.9.17) |
|---|---|---|---|
| **R5-01 a R5-19** (Requisitos de Sincronização e Rede do `PLANO_REQUISITOS.md`, revistos no Bloco 2–4) | Regras de mesclagem campo-a-campo, proteção de tabelas financeiras/fiscais imutáveis, integridade referencial pós-lote, detecção de relógio adiantado, faixas de código por caixa, retirada de caixa, interfaces virtuais e ingresso em loja existente (`juntar`). | **CONFIRMADO NO CÓDIGO (COM 2 DEFEITOS NOVOS EM CASOS DE BORDA)** | Implementados e cobertos pelos testes `r5_t01` a `r5_t17` em [sync.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/sync.rs#L4868-L6076). Contudo, a auditoria linha-a-linha revelou dois defeitos comprovados não detectados anteriormente: **`NEW-B12-01`** (derivação assimétrica de chave em `sync::decifrar` no modo portátil) e **`NEW-B12-02`** (`atender` responde a pedidos `"faixa"` antes de checar `retirado(&conn, &quem)`). |
| **B7B / `SecaoRede.qml` e `TelaJuntar.qml`** | Fluxo de juntar a uma loja existente limpando dados iniciais sem vendas e preservando o ID do dispositivo local. | **CONFIRMADO** | [sync.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/sync.rs#L3298-L3438) (`juntar` e `preparar_para_juntar`) bloqueiam a operação se já houver `venda`, `estoque_movimento`, `caixa_sessao`, `orcamento`, `ordem_servico`, `compra` ou `conta_pagar`, limpam as tabelas sincronizáveis dentro de `aplicando = 1` e restauram caso os pacotes iniciais falhem. |

---

## 3. Novos Achados Descobertos no Bloco 12

### 3.1. Defeitos Comprovados

#### `[NEW-B12-01]` Derivação assimétrica da chave criptográfica em `sync::decifrar` quebra a comunicação entre caixas no Modo Portátil e permite que o Modo Portátil decifre pacotes do Modo Instalado
- **Classificação:** **Defeito comprovado**
- **Severidade:** **Alta**
- **Localização:**
  - [sync.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/sync.rs#L2696-L2734) (`derivar` e `Chaves::de`)
  - [sync.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/sync.rs#L2837-L2866) (`decifrar`)
  - [portatil.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/portatil.rs#L74-L89) (`senha_da_rede`)
- **Evidência literal no código:**
  Em [sync.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/sync.rs#L2696-L2733), `derivar` aplica `crate::portatil::senha_da_rede(senha)` (que prefixa `"rustinho-portatil-teste\0"` quando `portatil::ligado()` é `true`), mas `Chaves::de` guarda em `Chaves.senha` a string `senha` original **sem o prefixo**:
  ```rust
  fn derivar(senha: &str, sal: &[u8; 16]) -> Resultado<[u8; 32]> {
      // O portátil tira a chave de outro jeito: nunca a mesma de um caixa
      // instalado, mesmo com a mesma senha (veja `portatil`).
      let senha = crate::portatil::senha_da_rede(senha);
      let senha = senha.as_ref();
      let mut k = [0u8; 32];
      crate::usuarios::argon2()
          .hash_password_into(senha.as_bytes(), sal, &mut k)
          .map_err(|_| Erro::Texto("Não foi possível preparar a chave da rede.".into()))?;
      Ok(k)
  }
  ...
  fn de(senha: &str) -> Resultado<Self> {
      ...
      let propria = derivar(senha, &sal)?;
      let s = crate::portatil::senha_da_rede(senha);
      let identificador: [u8; 32] = Sha256::digest(s.as_ref().as_bytes()).into();
      Ok(Self { senha: senha.to_string(), sal, propria, identificador, outros: HashMap::new() })
  }
  ```
  Porém, em [sync.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/sync.rs#L2843-L2860), quando `decifrar` recebe um pacote de **outro** caixa (`sal != chaves.sal`), ele **não chama `derivar(&chaves.senha, &sal)` nem `portatil::senha_da_rede(&chaves.senha)`** — ele passa `chaves.senha.as_bytes()` (sem prefixo!) diretamente para `Sha256::digest` e para `argon2().hash_password_into`:
  ```rust
  if sal == chaves.sal {
      if let Some(c) = abrir(&chaves.propria) {
          return Ok(c);
      }
  }
  let id: [u8; 32] = Sha256::digest(chaves.senha.as_bytes()).into();
  let senha_errada = "A senha da rede não confere com a deste caixa.";
  let corrompida = "Mensagem corrompida ou ilegível na rede.";
  let erro = if id == id_msg { corrompida } else { senha_errada };
  if let Some(k) = chaves.outros.get(&sal) {
      return abrir(k).ok_or(erro);
  }
  let mut k = [0u8; 32];
  crate::usuarios::argon2()
      .hash_password_into(chaves.senha.as_bytes(), &sal, &mut k)
      .map_err(|_| "chave inválida")?;
  let claro = abrir(&k).ok_or(senha_errada)?;
  ```
- **Impacto Comprovado:**
  1. **Quebra total da sincronização entre dois caixas em Modo Portátil:** Cada caixa gera seu próprio `sal` aleatório de 16 bytes (`sal_A != sal_B`). O Caixa Portátil A cifra seus pacotes usando `chaves.propria = Argon2("rustinho-portatil-teste\0" + senha, sal_A)`. Quando o Caixa Portátil B recebe o pacote, como `sal_A != sal_B`, ele deriva `k = Argon2(senha, sal_A)` (sem o prefixo `"rustinho-portatil-teste\0"`), falha ao abrir a cifra ChaCha20-Poly1305 e rejeita o pacote com `"A senha da rede não confere com a deste caixa."`.
  2. **Quebra da barreira criptográfica prometida em `portatil.rs:7–11, 80–89`:** Caso um pacote cifrado por um caixa **instalado** (`Argon2(senha, sal_instalado)`) chegue à porta de um caixa **portátil** configurado com a mesma senha humana, o caixa portátil calculará `Argon2(senha, sal_instalado)` e **decifrará o pacote do caixa instalado**.
  3. **Por que os testes atuais não pegaram:** Nos testes unitários de `sync.rs`, `portatil::ligado()` é `false` (pois não há `portatil.txt` ao lado do binário de teste nem `RUSTINHO_PORTATIL=1`), ocasião em que `senha_da_rede(senha)` retorna `Cow::Borrowed(senha)` sem prefixo.

---

#### `[NEW-B12-02]` Servidor TCP `sync::atender` processa pedidos de alocação de faixa de código (`"faixa"`) antes de verificar se o caixa solicitante foi retirado da loja (`retirado(&conn, &quem)`)
- **Classificação:** **Defeito comprovado**
- **Severidade:** **Média**
- **Localização:** [sync.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/sync.rs#L3161-L3190) (`atender`) e [sync.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/sync.rs#L1388-L1431) (`responder_faixa`)
- **Evidência literal no código:**
  ```rust
  let mut conn = abrir_conexao(caminho)?;
  if pedido["tipo"].as_str() == Some("faixa") {
      let resp = responder_faixa(&mut conn, &pedido)?;
      return escrever_mensagem(&mut fluxo, &mut chaves, &serde_json::to_vec(&resp).unwrap_or_default());
  }
  if pedido["tipo"].as_str() != Some("puxar") {
      return Err(Erro::Texto("Pedido desconhecido.".into()));
  }
  let quem = pedido["dispositivo"].as_str().unwrap_or("").to_string();
  ...
  if retirado(&conn, &quem)? {
      let resp = serde_json::json!({
          "tipo": "recusado",
          "motivo": "Este caixa foi retirado da loja pelo gerente.",
      });
      return escrever_mensagem(&mut fluxo, &mut chaves, &serde_json::to_vec(&resp).unwrap_or_default());
  }
  ```
- **Análise e Impacto:**
  Quando um caixa é retirado da loja via `sync::retirar_caixa` (`sync.rs:1463–1497`), suas faixas antigas em `faixa_codigo` são marcadas com `encerrada_em` e seu `dispositivo` é gravado em `caixa_retirado`. Porém, como `atender` trata `pedido["tipo"] == Some("faixa")` nas linhas `3168–3171` **antes** de verificar `retirado(&conn, &quem)?` na linha `3182` (e `responder_faixa` também não consulta `retirado`), um caixa retirado da loja que ainda possua a senha da rede consegue pedir e alocar novas faixas de códigos de produtos (`faixa_codigo`, de 1.000 em 1.000) no distribuidor.

---

#### `[NEW-B12-03]` Cruzamento Bloco 10 ↔ Bloco 12: `sync::TABELAS` confirma que a tabela de baixas de contas a pagar é `pagamento`, provando que `financeiro::fluxo` (`financeiro.rs:724–736`) consulta uma tabela inexistente (`conta_pagar_pagamento`) e ignora 100% das saídas pagas no Fluxo de Caixa
- **Classificação:** **Defeito comprovado** (descoberto no cruzamento de `sync.rs:230, 2120` com `financeiro.rs:724–736`)
- **Severidade:** **Alta**
- **Localização:**
  - [sync.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/sync.rs#L230) (`TABELAS` inclui `"pagamento"`)
  - [sync.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/sync.rs#L2120) (`"SELECT id FROM pagamento WHERE conta_pagar_id = ?1 AND estornado_em IS NULL"`)
  - [financeiro.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/financeiro.rs#L724-L736) (`financeiro::fluxo`)
- **Evidência literal em `financeiro.rs:724–736`:**
  ```rust
  // Pagamentos de conta a pagar realizados no período
  if let Ok(mut st) = conn.prepare(
      "SELECT substr(g.pago_em, 1, 10) AS d, COALESCE(SUM(g.valor), 0.0)
       FROM conta_pagar_pagamento g
       JOIN conta_pagar p ON p.id = g.conta_pagar_id
       WHERE substr(g.pago_em, 1, 10) >= ?1 AND substr(g.pago_em, 1, 10) <= ?2
         AND substr(g.pago_em, 1, 10) <= date('now', 'localtime')
         AND g.estornado_em IS NULL AND p.cancelado_em IS NULL
       GROUP BY d",
  ) {
  ```
- **Análise e Impacto:**
  A tabela SQL real de baixas do contas a pagar no Rustinho chama-se **`pagamento`** (como comprovado em `pagar.rs` e em `sync.rs:230, 2120`). Não existe tabela `conta_pagar_pagamento` no banco SQLite! Como `financeiro::fluxo` envolveu `conn.prepare(...)` em `if let Ok(mut st) = ...`, o erro `SQLite error: no such table: conta_pagar_pagamento` é silenciosamente descartado em todas as chamadas de `financeiro::fluxo`.
  Consequentemente, a aba **Fluxo de Caixa (`SecaoFluxo.qml`) jamais inclui os pagamentos realizados de contas a pagar (`pagamento`) em `saidas_realizadas`**, exibindo apenas compras à vista pagas sem duplicata e movimentos avulsos de saída!

---

### 3.2. Riscos Identificados

#### `[NEW-B12-04]` `sync::verificar_depois` promove o usuário mais antigo a gerente fora de `aplicando = 1`, gerando alterações e alertas de conflito duplicados em sincronizações concorrentes
- **Classificação:** **Risco**
- **Severidade:** **Média**
- **Localização:** [sync.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/sync.rs#L2022-L2055) (`verificar_depois`)
- **Evidência e Análise:**
  Quando um lote recebido deixa o banco com `usuarios > 0 && gerentes == 0`, `verificar_depois` executa `UPDATE usuario SET gerente = 1, atualizado_em = ?2 WHERE id = ?1` com os gatilhos de sincronização **ativos** (`aplicando = 0`). Se 3 caixas receberem o lote que rebaixou o último gerente, cada um dos 3 caixas executará esse `UPDATE` localmente com seu próprio `tempo::agora()` e `dispositivo`, gerando 3 linhas de `alteracao` concorrentes para o mesmo `usuario`. Embora `atualizado_em` e `dispositivo` estejam em `SEM_CONFLITO` (`sync.rs:413`), evitando um conflito secundário do tipo `'registro'`, o tráfego de alterações e os registros locais em `sync_conflito` (`tipo = 'gerente'`) se multiplicam a cada nó da malha.

#### `[NEW-B12-05]` Heurística `rede_local::parece_virtual` classifica qualquer interface iniciada por `"br"` como `Tipo::Virtual`
- **Classificação:** **Risco**
- **Severidade:** **Baixa**
- **Localização:** [rede_local.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/rede_local.rs#L297-L329) (`parece_virtual`)
- **Evidência e Análise:**
  `prefixo.starts_with("br")` captura tanto bridges de containers (`br-xxxx`) quanto bridges Linux de rede local (`br0`) ou interfaces BSD (`bridge0`). Caso o computador do caixa use uma bridge `br0` para acessar a LAN física (comum em servidores Linux que hospedam VMs), o Rustinho a classificará como `Virtual (VPN ou container)` e ignorará anúncios UDP vindos dela até que o gerente a marque manualmente na lista de interfaces liberadas em `SecaoRede.qml`.

---

### 3.3. Melhorias Identificadas

#### `[NEW-B12-06]` Responsividade dos botões lado a lado em `TelaJuntar.qml` para telas `< 350px`
- **Classificação:** **Melhoria**
- **Severidade:** **Baixa**
- **Localização:** [TelaJuntar.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaJuntar.qml#L140-L157)
- **Análise:**
  O `RowLayout` final contém os botões `"Voltar"` e `"Buscar e trazer dados"` na mesma linha horizontal sem empilhamento adaptativo (`GridLayout` ou `Flow`), o que pode comprimir o rótulo `"Buscar e trazer dados"` em telas móveis muito estreitas (`< 350px`).

---

## 4. Separação Rigorosa: Código Lido vs. Testes Encontrados vs. Testes Executados

1. **Código-fonte lido integralmente:**
   - `nucleo/src/sync.rs` (6.430 linhas)
   - `nucleo/src/rede_local.rs` (1.037 linhas)
   - `nucleo/src/computador.rs` (118 linhas)
   - `nucleo/src/portatil.rs` (143 linhas)
   - `nucleo/src/teste_rede.rs` (185 linhas)
   - `app/qml/TelaJuntar.qml` (169 linhas)
2. **Testes encontrados no código-fonte (85 testes unitários/integração lidos):**
   - `sync.rs`: 63 testes (`gatilho_grava_insercao_atualizacao_e_exclusao` até `anuncio_do_proprio_caixa_ou_de_interface_virtual_e_ignorado`, incluindo toda a suíte `r5_t01` a `r5_t17`).
   - `rede_local.rs`: 13 testes (`classifica_nomes_conhecidos` até `varredura_ignora_sub_rede_grande_demais`).
   - `computador.rs`: 3 testes (`limpa_nomes_comuns_de_maquina`, `rejeita_nomes_genericos_ou_curtos`, `corta_nomes_compridos_sem_quebrar_utf8`).
   - `portatil.rs`: 2 testes (`Portatil_separa_caminhos_portas_e_chave_da_rede`, `Portatil_detecta_arquivo_marcador_ao_lado`).
   - `teste_rede.rs`: 4 testes (`par_sincroniza`, `malha_converge`, `tres_caixas_offline_convergem`, `conflito_simultaneo_preserva_perdedor`).
   - **Lacuna de teste identificada:** Não há nenhum teste exercitando `sync::cifrar` + `sync::decifrar` entre dois nós com `sal` diferentes quando `portatil::ligado() == true` (o que teria capturado imediatamente o defeito `NEW-B12-01`), nem teste de `atender` recebendo `pedido["tipo"] == "faixa"` de um caixa em `caixa_retirado` (`NEW-B12-02`).
3. **Testes executados neste bloco:**
   - **Nenhum (`0`).** Conforme o plano de auditoria, a execução efetiva da suíte de testes ocorrerá separadamente no **Bloco 18**.

