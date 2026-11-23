//! Motor de recuperação automática do trabalho em andamento (`0.9.19-dev.1`, Pacote 3: `R1` e `R2`;
//! `REC-1` a `REC-6`, `DR-3` a `DR-7`, `ES-1`, `ES-5` `7a`/`7b` e `T-03`).
//!
//! ## Garantias arquiteturais (`G1`–`G9`)
//! - **Preservado no disco (`G1`):** toda gravação roda em transação `IMMEDIATE` com `PRAGMA synchronous = FULL`
//!   (fixado na abertura do banco). A tela só exibe `"Guardado às HH:MM"` depois que `trabalho_guardar` confirma.
//! - **Duas versões com resumo SHA-256:** ao atualizar um trabalho, a versão anterior íntegra fica em
//!   `estado_anterior_json` + `resumo_anterior_sha256`. Se a versão mais nova estiver corrompida, o motor recupera
//!   automaticamente a versão anterior e registra no diagnóstico; se ambas falharem, marca `sem_conserto` sem
//!   derrubar o programa nem apagar o registro.
//! - **Sanitização de segredos:** senhas, PIN, tokens, segredos e chaves privadas são removidos recursivamente do
//!   JSON antes de gravar no banco.
//! - **Retorno sempre como rascunho (`G3`, `G7`):** recuperar ou abrir um trabalho **nunca** conclui venda,
//!   recebimento, lançamento de compra, ordem de serviço ou cadastro sozinho. Se `chave_operacao` já constar em
//!   `operacao_feita` (`DR-7` / `REC-5`), o motor sinaliza `jaConcluida = true` com o resultado gravado, impedindo
//!   duplicação (`G2`).
//! - **Revalidação contra alterações concorrentes:** para registros já existentes (`alvo_id != ""`), confere o
//!   `atualizado_em` atual da tabela do domínio contra `base_atualizado_em` de quando a edição começou, sinalizando
//!   `conflitoBase = true` se outro caixa alterou ou removeu o registro nesse meio-tempo.
//! - **Isolamento e sigilo (`DR-3` + exceção auditável `7b`, `G8`, `G9`):**
//!   - Usuário **ativo**: só a própria pessoa lista, abre, conclui ou descarta seus trabalhos. O gerente vê apenas
//!     a contagem/existência (`outrosAtivos`), nunca o conteúdo, e qualquer tentativa de abrir trabalho de usuário
//!     ativo alheio é recusada.
//!   - Usuário **desativado** (`usuario.ativo = 0`): o gerente autenticado (com senha do gerente) pode abrir,
//!     concluir ou descartar o trabalho deixado pelo usuário desativado, gravando obrigatoriamente trilha completa
//!     em `auditoria` (`trabalho.gerente_abrir`, `trabalho.gerente_concluir`, `trabalho.gerente_descartar`) com
//!     autor original, gerente responsável, tipo/título, data/hora e motivo.
//! - **Prazo de 30 dias (`DR-5` / `REC-3`):** trabalhos sem atualização há mais de 30 dias aparecem classificados
//!   como `"abandonado"` na lista, mas **nunca** são apagados sozinhos.
//! - **Backup e conflito de origem (`DR-4` / `DR-6` / `REC-4`):** a tabela `trabalho` faz parte do arquivo SQLite
//!   principal e vai junto no backup. Quando um backup do caixa A é restaurado no caixa B (`dispositivo_origem != disp`),
//!   o trabalho fica preservado e sinalizado com `conflitoOrigem = true`, sem transferir nem descartar sozinho, e
//!   não pode ser aberto como se fosse do caixa B.
//! - **Relógio recuado (`T-03`):** se o relógio do computador voltou no tempo em relação a sessões/gravações já
//!   registradas, `relogio_recuado` é marcado e a classificação automática de 30 dias (`abandonado`) fica suspensa
//!   para não rebaixar trabalhos recentes por engano.
//! - **Fechamento durante operação crítica (`ES-1` / `REC-6`):** `avaliar_fechamento` informa o que está em
//!   andamento, distingue o que pode ser cancelado com segurança do que convém esperar terminar, e nunca prende o
//!   usuário indefinidamente.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::erro::{Erro, Resultado};
use crate::{auditoria, registro, tempo, texto};

/// Versão atual do formato de retrato de trabalho.
pub const FORMATO_ATUAL: i64 = 1;

/// Prazo em dias para classificar um trabalho parado como `"abandonado"` (`DR-5`: nunca apagar sozinho).
pub const DIAS_ABANDONADO: i64 = 30;

/// Tolerância (1 minuto) para considerar que o relógio do computador voltou para trás (`T-03`).
const TOLERANCIA_RELOGIO_MS: i64 = 60_000;

/// Tipos de trabalho aceitos pelo motor (`R1`: `peca`; `R2`: `pessoa`, `venda`, `receber`, `os`, `compra`, `guia`).
pub const TIPOS_VALIDOS: &[&str] = &["peca", "pessoa", "venda", "receber", "os", "compra", "guia"];

fn agora_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Calcula o SHA-256 em hexadecimal minúsculo.
pub fn sha256_hex(texto: &str) -> String {
    Sha256::digest(texto.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Remove recursivamente campos sensíveis (senhas, PIN, tokens, segredos, chaves privadas) antes de persistir.
pub fn sanitizar_estado(valor: &Value) -> Value {
    fn eh_chave_sensivel(chave: &str) -> bool {
        let k = chave.trim().to_ascii_lowercase();
        k.contains("senha")
            || k == "pin"
            || k.ends_with("_pin")
            || k.starts_with("pin_")
            || k.contains("segredo")
            || k.contains("token")
            || k == "csc"
            || k.contains("chaveprivada")
            || k.contains("chave_privada")
    }
    match valor {
        Value::Object(mapa) => {
            let mut limpo = serde_json::Map::new();
            for (k, v) in mapa {
                if !eh_chave_sensivel(k) {
                    limpo.insert(k.clone(), sanitizar_estado(v));
                }
            }
            Value::Object(limpo)
        }
        Value::Array(lista) => Value::Array(lista.iter().map(sanitizar_estado).collect()),
        outro => outro.clone(),
    }
}

/// Detecta se o relógio do computador recuou em relação às sessões ou trabalhos já gravados (`T-03`).
pub fn relogio_recuado_em(conn: &Connection, agora_epoch_ms: i64) -> Resultado<bool> {
    let max_sessao: i64 =
        conn.query_row("SELECT COALESCE(MAX(aberta_ms), 0) FROM sessao_local", [], |l| l.get(0)).unwrap_or(0);
    let max_trabalho: i64 =
        conn.query_row("SELECT COALESCE(MAX(atualizado_ms), 0) FROM trabalho", [], |l| l.get(0)).unwrap_or(0);
    let maior = max_sessao.max(max_trabalho);
    Ok(maior > 0 && agora_epoch_ms + TOLERANCIA_RELOGIO_MS < maior)
}

/// Registra a abertura de uma sessão local de usuário (`REC-1` / `T-03`).
pub fn registrar_entrada(conn: &Connection, dispositivo_id: &str, usuario_id: &str) -> Resultado<i64> {
    let ms = agora_ms();
    let recuado = relogio_recuado_em(conn, ms)?;
    let quando = tempo::agora();
    conn.execute(
        "INSERT INTO sessao_local (id, dispositivo_id, usuario_id, aberta_em, aberta_ms, limpa, relogio_recuado)
         VALUES ((SELECT COALESCE(MAX(id), 0) + 1 FROM sessao_local), ?1, ?2, ?3, ?4, 0, ?5)",
        params![dispositivo_id, usuario_id, quando, ms, i64::from(recuado)],
    )?;
    let id: i64 = conn.query_row("SELECT COALESCE(MAX(id), 0) FROM sessao_local", [], |l| l.get(0))?;
    sincronizar_classificacoes(conn, dispositivo_id, recuado)?;
    Ok(id)
}

/// Marca a última sessão aberta deste dispositivo como encerrada de forma limpa (`sair` ou fechar janela).
/// Os trabalhos em andamento continuam guardados para a pessoa retomar quando voltar (`ES-5` 7a).
pub fn encerrar_sessao(conn: &Connection, dispositivo_id: &str) -> Resultado<()> {
    let quando = tempo::agora();
    conn.execute(
        "UPDATE sessao_local SET encerrada_em = ?1, limpa = 1
          WHERE id = (SELECT MAX(id) FROM sessao_local WHERE dispositivo_id = ?2 AND limpa = 0)",
        params![quando, dispositivo_id],
    )?;
    Ok(())
}

/// Atualiza no banco as marcas persistentes de `conflito_origem` (`DR-6`) e `abandonado` (`DR-5`, 30 dias,
/// suspenso se `relogio_recuado`).
pub fn sincronizar_classificacoes(conn: &Connection, dispositivo_atual: &str, relogio_recuado: bool) -> Resultado<()> {
    conn.execute(
        "UPDATE trabalho SET conflito_origem = CASE WHEN dispositivo_origem <> ?1 THEN 1 ELSE 0 END
          WHERE conflito_origem <> CASE WHEN dispositivo_origem <> ?1 THEN 1 ELSE 0 END",
        params![dispositivo_atual],
    )?;
    if !relogio_recuado {
        let corte = tempo::dias_atras(DIAS_ABANDONADO);
        conn.execute(
            "UPDATE trabalho SET situacao = 'abandonado'
              WHERE situacao = 'aberto' AND atualizado_em < ?1",
            params![corte],
        )?;
    }
    Ok(())
}

/// Entrada para guardar um retrato de trabalho em andamento (`REC-1`).
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct GuardarEntrada {
    pub id: String,
    pub tipo: String,
    pub alvo_id: String,
    pub titulo: String,
    pub formato_versao: Option<i64>,
    pub estado: Value,
    pub percurso: Value,
    pub base_atualizado_em: String,
    pub chave_operacao: String,
}

/// Resposta confirmada ao guardar um trabalho (`REC-1`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrabalhoGuardado {
    pub id: String,
    pub tipo: String,
    pub alvo_id: String,
    pub titulo: String,
    pub guardado_em: String,
    pub guardado_hora: String,
    pub resumo_sha256: String,
    pub relogio_recuado: bool,
}

/// Nome amigável do tipo de trabalho para exibição na tela.
pub fn nome_do_tipo(tipo: &str) -> &'static str {
    match tipo {
        "peca" => "Ficha de peça",
        "pessoa" => "Cadastro de cliente ou fornecedor",
        "venda" => "Venda em andamento",
        "receber" => "Recebimento de crediário",
        "os" => "Ordem de serviço",
        "compra" => "Conferência de nota de compra",
        "guia" => "Percurso de navegação guiada",
        _ => "Trabalho guardado",
    }
}

fn hora_local_curta(conn: &Connection, quando_utc: &str) -> String {
    conn.query_row("SELECT COALESCE(strftime('%H:%M', ?1, 'localtime'), '')", [quando_utc], |l| l.get::<_, String>(0))
        .unwrap_or_default()
}

fn data_hora_local(conn: &Connection, quando_utc: &str) -> String {
    conn.query_row("SELECT COALESCE(strftime('%d/%m/%Y às %H:%M', ?1, 'localtime'), ?1)", [quando_utc], |l| {
        l.get::<_, String>(0)
    })
    .unwrap_or_else(|_| quando_utc.to_string())
}

/// Busca o `atualizado_em` atual do registro alvo no banco (quando `alvo_id` não é vazio).
/// Devolve `(existe, atualizado_em_atual)`.
pub fn estado_base_no_banco(conn: &Connection, tipo: &str, alvo_id: &str) -> Resultado<(bool, String)> {
    let id = alvo_id.trim();
    if id.is_empty() {
        return Ok((true, String::new()));
    }
    let sql = match tipo {
        "peca" => "SELECT COALESCE(atualizado_em, criado_em, '') FROM produto WHERE id = ?1",
        "pessoa" => "SELECT COALESCE(atualizado_em, criado_em, '') FROM pessoa WHERE id = ?1",
        "os" => "SELECT COALESCE(atualizado_em, criado_em, '') FROM ordem_servico WHERE id = ?1",
        "compra" => "SELECT COALESCE(conferida_em, criado_em, '') FROM compra WHERE id = ?1",
        "receber" => "SELECT COALESCE(atualizado_em, criado_em, '') FROM pessoa WHERE id = ?1",
        _ => return Ok((true, String::new())),
    };
    let achado: Option<String> = conn.query_row(sql, [id], |l| l.get(0)).optional()?;
    Ok(match achado {
        Some(ts) => (true, ts),
        None => (false, String::new()),
    })
}

/// Verifica se a `chave_operacao` associada ao trabalho já foi concluída em `operacao_feita` (`DR-7` / `REC-5`).
fn operacao_ja_concluida(
    conn: &Connection,
    tipo: &str,
    chave_da_tela: &str,
    usuario_id: &str,
) -> Resultado<Option<(String, Value)>> {
    let chave_limpa = chave_da_tela.trim();
    if chave_limpa.is_empty() {
        return Ok(None);
    }
    let comandos_possiveis: &[&str] = match tipo {
        "peca" => &["produto_salvar"],
        "pessoa" => &["pessoa_salvar"],
        "venda" => &["venda_concluir", "orcamento_salvar"],
        "receber" => &["receber_baixar", "receber_lancar"],
        "os" => &["os_salvar", "os_rascunho_salvar"],
        "compra" => &["compra_lancar", "compra_importar"],
        _ => &[],
    };
    for cmd in comandos_possiveis {
        let op = crate::operacoes::identificar(cmd, chave_limpa, usuario_id, &Value::Null);
        let linha: Option<(String, String)> = conn
            .query_row("SELECT resultado, resumo FROM operacao_feita WHERE chave = ?1", [&op.chave[..]], |l| {
                Ok((l.get(0)?, l.get(1)?))
            })
            .optional()?;
        if let Some((resultado_txt, resumo)) = linha {
            let val = serde_json::from_str(&resultado_txt).unwrap_or(Value::Null);
            return Ok(Some((resumo, val)));
        }
    }
    Ok(None)
}

/// Guarda (cria ou atualiza) o retrato de um trabalho em andamento (`REC-1`).
/// Roda dentro da transação aberta pelo comando (`mod comandos`).
pub fn guardar(
    conn: &Connection,
    dispositivo_id: &str,
    usuario_id: &str,
    d: GuardarEntrada,
) -> Resultado<TrabalhoGuardado> {
    let tipo = d.tipo.trim().to_ascii_lowercase();
    if !TIPOS_VALIDOS.contains(&tipo.as_str()) {
        return Err(Erro::validacao(format!("Tipo de trabalho desconhecido: {tipo}.")));
    }
    if !d.estado.is_object() {
        return Err(Erro::validacao("O retrato do trabalho precisa ser um objeto JSON válido."));
    }
    let alvo_id = d.alvo_id.trim().to_string();
    let formato_versao = d.formato_versao.unwrap_or(FORMATO_ATUAL);
    if formato_versao <= 0 || formato_versao > FORMATO_ATUAL {
        return Err(Erro::validacao(format!(
            "Versão de formato do trabalho ({formato_versao}) incompatível com esta versão do Rustinho ({FORMATO_ATUAL})."
        )));
    }
    let estado_limpo = sanitizar_estado(&d.estado);
    let estado_json = serde_json::to_string(&estado_limpo)?;
    let resumo_sha256 = sha256_hex(&estado_json);

    let percurso_limpo = if d.percurso.is_array() { sanitizar_estado(&d.percurso) } else { json!([]) };
    let percurso_json = serde_json::to_string(&percurso_limpo)?;

    let titulo_limpo = texto::limpar(&d.titulo);
    let titulo = if titulo_limpo.is_empty() {
        nome_do_tipo(&tipo).to_string()
    } else {
        titulo_limpo.chars().take(120).collect()
    };

    let base_atualizado_em = if !d.base_atualizado_em.trim().is_empty() {
        d.base_atualizado_em.trim().to_string()
    } else {
        estado_base_no_banco(conn, &tipo, &alvo_id)?.1
    };

    let ms = agora_ms();
    let recuado = relogio_recuado_em(conn, ms)?;
    let agora = tempo::agora();

    // Procura se já existe um trabalho para (usuario_id, dispositivo_id, tipo, alvo_id) ou pelo id informado.
    let existente: Option<(String, String, String, String)> = conn
        .query_row(
            "SELECT id, estado_json, resumo_sha256, base_atualizado_em FROM trabalho
              WHERE (usuario_id = ?1 AND dispositivo_origem = ?2 AND tipo = ?3 AND alvo_id = ?4)
                 OR (?5 <> '' AND id = ?5 AND usuario_id = ?1 AND dispositivo_origem = ?2)
              LIMIT 1",
            params![usuario_id, dispositivo_id, tipo, alvo_id, d.id.trim()],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)),
        )
        .optional()?;

    let id = match existente {
        Some((id_atual, est_ant, sha_ant, base_ant)) => {
            // Só promove a versão anterior para o slot de fallback se ela estiver íntegra (SHA-256 bate).
            let ant_integra = sha256_hex(&est_ant) == sha_ant;
            let base_final = if d.base_atualizado_em.trim().is_empty() && !base_ant.is_empty() {
                base_ant
            } else {
                base_atualizado_em
            };
            if ant_integra {
                conn.execute(
                    "UPDATE trabalho
                        SET titulo = ?1,
                            formato_versao = ?2,
                            estado_anterior_json = estado_json,
                            resumo_anterior_sha256 = resumo_sha256,
                            estado_json = ?3,
                            resumo_sha256 = ?4,
                            percurso_json = ?5,
                            base_atualizado_em = ?6,
                            chave_operacao = ?7,
                            situacao = 'aberto',
                            conflito_origem = 0,
                            atualizado_em = ?8,
                            atualizado_ms = ?9
                      WHERE id = ?10",
                    params![
                        titulo,
                        formato_versao,
                        estado_json,
                        resumo_sha256,
                        percurso_json,
                        base_final,
                        d.chave_operacao.trim(),
                        agora,
                        ms,
                        id_atual
                    ],
                )?;
            } else {
                conn.execute(
                    "UPDATE trabalho
                        SET titulo = ?1,
                            formato_versao = ?2,
                            estado_json = ?3,
                            resumo_sha256 = ?4,
                            percurso_json = ?5,
                            base_atualizado_em = ?6,
                            chave_operacao = ?7,
                            situacao = 'aberto',
                            conflito_origem = 0,
                            atualizado_em = ?8,
                            atualizado_ms = ?9
                      WHERE id = ?10",
                    params![
                        titulo,
                        formato_versao,
                        estado_json,
                        resumo_sha256,
                        percurso_json,
                        base_final,
                        d.chave_operacao.trim(),
                        agora,
                        ms,
                        id_atual
                    ],
                )?;
            }
            id_atual
        }
        None => {
            let novo_id = if d.id.trim().is_empty() { tempo::novo_id() } else { d.id.trim().to_string() };
            conn.execute(
                "INSERT INTO trabalho (
                    id, usuario_id, dispositivo_origem, tipo, alvo_id, titulo, formato_versao,
                    estado_json, resumo_sha256, estado_anterior_json, resumo_anterior_sha256,
                    percurso_json, base_atualizado_em, chave_operacao, situacao, conflito_origem,
                    criado_em, atualizado_em, atualizado_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, NULL, ?10, ?11, ?12, 'aberto', 0, ?13, ?13, ?14)",
                params![
                    novo_id,
                    usuario_id,
                    dispositivo_id,
                    tipo,
                    alvo_id,
                    titulo,
                    formato_versao,
                    estado_json,
                    resumo_sha256,
                    percurso_json,
                    base_atualizado_em,
                    d.chave_operacao.trim(),
                    agora,
                    ms
                ],
            )?;
            novo_id
        }
    };

    sincronizar_classificacoes(conn, dispositivo_id, recuado)?;
    let guardado_hora = hora_local_curta(conn, &agora);
    Ok(TrabalhoGuardado {
        id,
        tipo,
        alvo_id,
        titulo,
        guardado_em: agora,
        guardado_hora,
        resumo_sha256,
        relogio_recuado: recuado,
    })
}

/// Resumo de um item de trabalho na lista `"Continuar de onde parou?"`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumoTrabalho {
    pub id: String,
    pub usuario_id: String,
    pub usuario_nome: String,
    pub usuario_ativo: bool,
    pub dispositivo_origem: String,
    pub tipo: String,
    pub tipo_nome: String,
    pub alvo_id: String,
    pub titulo: String,
    pub formato_versao: i64,
    pub formato_incompativel: bool,
    pub situacao: String,
    pub conflito_origem: bool,
    pub conflito_base: bool,
    pub motivo_conflito: String,
    pub usou_versao_anterior: bool,
    pub ja_concluida: bool,
    pub resumo_concluida: String,
    pub criado_em: String,
    pub atualizado_em: String,
    pub atualizado_texto: String,
}

/// Contagem de trabalhos de outros usuários ativos (`DR-3`: o gerente vê só que existem, sem abrir).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumoOutrosAtivos {
    pub usuarios_com_trabalho: i64,
    pub total_trabalhos: i64,
}

/// Painel completo retornado por `trabalho_listar`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PainelTrabalhos {
    /// Trabalhos do próprio usuário neste caixa (`dispositivo_origem == disp`).
    pub meus: Vec<ResumoTrabalho>,
    /// Trabalhos que vieram num backup restaurado de outro caixa (`DR-6`: preservados e sinalizados).
    pub de_outro_caixa: Vec<ResumoTrabalho>,
    /// Só para gerente (`7b`): trabalhos deixados por usuários **desativados** (`usuario.ativo = 0`).
    pub de_usuarios_desativados: Vec<ResumoTrabalho>,
    /// Só para gerente (`DR-3`): existência de trabalhos de outros usuários **ativos** (sem mostrar conteúdo).
    pub outros_ativos: ResumoOutrosAtivos,
    /// A sessão anterior deste computador foi interrompida sem fechamento limpo?
    pub houve_interrupcao: bool,
    /// O relógio do computador recuou em relação às gravações anteriores (`T-03`)?
    pub relogio_recuado: bool,
}

/// Detalhe completo de um trabalho aberto para retomada (`trabalho_abrir` ou `trabalho_gerente_abrir`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrabalhoAberto {
    pub resumo: ResumoTrabalho,
    /// Retrato completo dos campos da tela (`Value::Null` se `sem_conserto`, `formato_incompativel` ou `ja_concluida`).
    pub estado: Value,
    /// Percurso da navegação contextual associado.
    pub percurso: Value,
    /// Sempre `true`: nenhuma recuperação conclui operação sozinha (`G3`, `G7`).
    pub rascunho: bool,
    /// Se `ja_concluida == true`, o resultado que `operacao_feita` guardou na primeira vez (`DR-7`).
    pub resultado_concluida: Value,
}

struct LinhaBruta {
    id: String,
    usuario_id: String,
    usuario_nome: String,
    usuario_ativo: bool,
    dispositivo_origem: String,
    tipo: String,
    alvo_id: String,
    titulo: String,
    formato_versao: i64,
    estado_json: String,
    resumo_sha256: String,
    estado_anterior_json: Option<String>,
    resumo_anterior_sha256: Option<String>,
    percurso_json: String,
    base_atualizado_em: String,
    chave_operacao: String,
    situacao: String,
    conflito_origem: bool,
    criado_em: String,
    atualizado_em: String,
}

/// Avalia a integridade (2 versões + SHA-256), compatibilidade de formato, prazo de 30 dias (`DR-5`),
/// conflito de origem (`DR-6`), conflito com o banco (`base_atualizado_em`) e conclusão prévia (`DR-7`).
fn avaliar_linha(
    conn: &Connection,
    dispositivo_atual: &str,
    relogio_recuado: bool,
    l: &LinhaBruta,
) -> Resultado<(ResumoTrabalho, Value, Value)> {
    let conflito_origem = l.conflito_origem || l.dispositivo_origem != dispositivo_atual;
    let formato_incompativel = l.formato_versao <= 0 || l.formato_versao > FORMATO_ATUAL;

    let mut usou_versao_anterior = false;
    let mut sem_conserto = l.situacao == "sem_conserto";
    let mut estado_val = Value::Null;

    if !formato_incompativel {
        if sha256_hex(&l.estado_json) == l.resumo_sha256 {
            if let Ok(v) = serde_json::from_str::<Value>(&l.estado_json) {
                estado_val = sanitizar_estado(&v);
                sem_conserto = false;
            } else {
                sem_conserto = true;
            }
        } else {
            sem_conserto = true;
        }

        if sem_conserto {
            if let (Some(ant_json), Some(ant_sha)) = (&l.estado_anterior_json, &l.resumo_anterior_sha256) {
                if sha256_hex(ant_json) == *ant_sha {
                    if let Ok(v) = serde_json::from_str::<Value>(ant_json) {
                        estado_val = sanitizar_estado(&v);
                        sem_conserto = false;
                        usou_versao_anterior = true;
                        registro::anotar(&format!(
                            "recuperação do trabalho {}: versão mais nova corrompida; restaurada a versão anterior íntegra",
                            l.id
                        ));
                    }
                }
            }
            if sem_conserto {
                registro::anotar(&format!(
                    "recuperação do trabalho {}: estado corrompido e sem versão anterior válida (marcado sem_conserto)",
                    l.id
                ));
            }
        }
    }

    let corte_30d = tempo::dias_atras(DIAS_ABANDONADO);
    let situacao = if sem_conserto {
        "sem_conserto".to_string()
    } else if l.situacao == "abandonado" || (!relogio_recuado && l.atualizado_em < corte_30d) {
        "abandonado".to_string()
    } else {
        "aberto".to_string()
    };

    let (existe_base, atual_no_banco) = estado_base_no_banco(conn, &l.tipo, &l.alvo_id)?;
    let (conflito_base, motivo_conflito) = if conflito_origem {
        (
            false,
            format!(
                "Este rascunho foi começado em outro caixa ({}) e veio na restauração do backup. Ele fica preservado aqui para conferência, sem mudar de caixa sozinho.",
                l.dispositivo_origem
            ),
        )
    } else if !l.alvo_id.trim().is_empty() && !existe_base {
        (
            true,
            "O cadastro original que estava sendo editado não existe mais no banco de dados. Confira antes de salvar para não recriar por engano."
                .to_string(),
        )
    } else if !l.alvo_id.trim().is_empty()
        && !l.base_atualizado_em.trim().is_empty()
        && !atual_no_banco.is_empty()
        && atual_no_banco != l.base_atualizado_em
    {
        (
            true,
            format!(
                "Este cadastro foi alterado depois que o rascunho começou (última alteração em {}). Confira as diferenças na tela antes de confirmar: nada foi gravado por cima.",
                data_hora_local(conn, &atual_no_banco)
            ),
        )
    } else {
        (false, String::new())
    };

    let (ja_concluida, resumo_concluida, resultado_concluida) =
        match operacao_ja_concluida(conn, &l.tipo, &l.chave_operacao, &l.usuario_id)? {
            Some((res, val)) => (true, res, val),
            None => (false, String::new(), Value::Null),
        };

    let percurso_val = serde_json::from_str::<Value>(&l.percurso_json)
        .ok()
        .filter(Value::is_array)
        .map(|v| sanitizar_estado(&v))
        .unwrap_or_else(|| json!([]));

    let resumo = ResumoTrabalho {
        id: l.id.clone(),
        usuario_id: l.usuario_id.clone(),
        usuario_nome: l.usuario_nome.clone(),
        usuario_ativo: l.usuario_ativo,
        dispositivo_origem: l.dispositivo_origem.clone(),
        tipo: l.tipo.clone(),
        tipo_nome: nome_do_tipo(&l.tipo).to_string(),
        alvo_id: l.alvo_id.clone(),
        titulo: l.titulo.clone(),
        formato_versao: l.formato_versao,
        formato_incompativel,
        situacao,
        conflito_origem,
        conflito_base,
        motivo_conflito,
        usou_versao_anterior,
        ja_concluida,
        resumo_concluida,
        criado_em: l.criado_em.clone(),
        atualizado_em: l.atualizado_em.clone(),
        atualizado_texto: data_hora_local(conn, &l.atualizado_em),
    };
    let _ = resultado_concluida;
    Ok((resumo, estado_val, percurso_val))
}

fn carregar_linhas(
    conn: &Connection,
    filtro_sql: &str,
    params_sql: &[&dyn rusqlite::ToSql],
) -> Resultado<Vec<LinhaBruta>> {
    let sql = format!(
        "SELECT t.id, t.usuario_id, COALESCE(u.nome, 'Usuário'), COALESCE(u.ativo, 0),
                t.dispositivo_origem, t.tipo, t.alvo_id, t.titulo, t.formato_versao,
                t.estado_json, t.resumo_sha256, t.estado_anterior_json, t.resumo_anterior_sha256,
                t.percurso_json, t.base_atualizado_em, t.chave_operacao, t.situacao,
                t.conflito_origem, t.criado_em, t.atualizado_em
           FROM trabalho t
           LEFT JOIN usuario u ON u.id = t.usuario_id
          WHERE {filtro_sql}
          ORDER BY t.atualizado_ms DESC, t.atualizado_em DESC, t.id DESC"
    );
    let mut st = conn.prepare(&sql)?;
    let linhas = st
        .query_map(params_sql, |l| {
            Ok(LinhaBruta {
                id: l.get(0)?,
                usuario_id: l.get(1)?,
                usuario_nome: l.get(2)?,
                usuario_ativo: l.get::<_, i64>(3)? != 0,
                dispositivo_origem: l.get(4)?,
                tipo: l.get(5)?,
                alvo_id: l.get(6)?,
                titulo: l.get(7)?,
                formato_versao: l.get(8)?,
                estado_json: l.get(9)?,
                resumo_sha256: l.get(10)?,
                estado_anterior_json: l.get(11)?,
                resumo_anterior_sha256: l.get(12)?,
                percurso_json: l.get(13)?,
                base_atualizado_em: l.get(14)?,
                chave_operacao: l.get(15)?,
                situacao: l.get(16)?,
                conflito_origem: l.get::<_, i64>(17)? != 0,
                criado_em: l.get(18)?,
                atualizado_em: l.get(19)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(linhas)
}

/// Lista os trabalhos disponíveis para quem está conectado (`REC-1` a `REC-5`).
/// Puramente leitura (`SELECT`), seguro também em conexões `query_only`.
pub fn listar(
    conn: &Connection,
    dispositivo_atual: &str,
    usuario_id: &str,
    gerente: bool,
) -> Resultado<PainelTrabalhos> {
    let recuado = relogio_recuado_em(conn, agora_ms())?;

    let proprias = carregar_linhas(conn, "t.usuario_id = ?1", &[&usuario_id])?;
    let mut meus = Vec::new();
    let mut de_outro_caixa = Vec::new();
    for l in &proprias {
        let (resumo, _, _) = avaliar_linha(conn, dispositivo_atual, recuado, l)?;
        if resumo.conflito_origem {
            de_outro_caixa.push(resumo);
        } else {
            meus.push(resumo);
        }
    }

    let mut de_usuarios_desativados = Vec::new();
    let mut outros_ativos = ResumoOutrosAtivos { usuarios_com_trabalho: 0, total_trabalhos: 0 };

    if gerente {
        // Outros trabalhos de outro caixa que vieram em backup (de qualquer usuário, quando quem olha é gerente).
        let outros_caixas = carregar_linhas(
            conn,
            "t.usuario_id <> ?1 AND (t.conflito_origem <> 0 OR t.dispositivo_origem <> ?2)",
            &[&usuario_id, &dispositivo_atual],
        )?;
        for l in &outros_caixas {
            let (resumo, _, _) = avaliar_linha(conn, dispositivo_atual, recuado, l)?;
            de_outro_caixa.push(resumo);
        }

        // 7b: trabalhos locais deixados por usuários DESATIVADOS (`u.ativo = 0`).
        let desativados = carregar_linhas(
            conn,
            "t.usuario_id <> ?1 AND t.dispositivo_origem = ?2 AND COALESCE(u.ativo, 0) = 0",
            &[&usuario_id, &dispositivo_atual],
        )?;
        for l in &desativados {
            let (resumo, _, _) = avaliar_linha(conn, dispositivo_atual, recuado, l)?;
            de_usuarios_desativados.push(resumo);
        }

        // DR-3: para usuários ATIVOS diferentes do gerente atual, só a contagem/existência, sem abrir nem listar.
        let (usuarios_cnt, total_cnt): (i64, i64) = conn.query_row(
            "SELECT COUNT(DISTINCT t.usuario_id), COUNT(*)
               FROM trabalho t
               JOIN usuario u ON u.id = t.usuario_id
              WHERE t.usuario_id <> ?1 AND t.dispositivo_origem = ?2 AND u.ativo = 1",
            params![usuario_id, dispositivo_atual],
            |l| Ok((l.get(0)?, l.get(1)?)),
        )?;
        outros_ativos = ResumoOutrosAtivos { usuarios_com_trabalho: usuarios_cnt, total_trabalhos: total_cnt };
    }

    // Houve interrupção na sessão anterior deste dispositivo?
    let abertura_atual = crate::operacoes::abertura_atual(conn).unwrap_or(0);
    let sessao_interrompida: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sessao_local s
              WHERE s.dispositivo_id = ?1 AND s.limpa = 0
                AND s.id < COALESCE((SELECT MAX(id) FROM sessao_local WHERE dispositivo_id = ?1), 0)",
            params![dispositivo_atual],
            |l| l.get(0),
        )
        .unwrap_or(false);
    let programa_caiu: bool = abertura_atual > 1
        && conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM programa_abertura WHERE numero < ?1 AND (limpa = 0 OR com_erro = 1)",
                params![abertura_atual],
                |l| l.get(0),
            )
            .unwrap_or(false);

    Ok(PainelTrabalhos {
        meus,
        de_outro_caixa,
        de_usuarios_desativados,
        outros_ativos,
        houve_interrupcao: sessao_interrompida || programa_caiu,
        relogio_recuado: recuado,
    })
}

/// Abre um trabalho do próprio usuário neste caixa para retomada (`REC-1`, `DR-3`, `DR-6`).
/// Puramente leitura (`SELECT`).
pub fn abrir_meu(
    conn: &Connection,
    dispositivo_atual: &str,
    usuario_id: &str,
    id: &str,
    tipo: &str,
    alvo_id: &str,
) -> Resultado<Option<TrabalhoAberto>> {
    let linhas = if !id.trim().is_empty() {
        carregar_linhas(conn, "t.id = ?1", &[&id.trim()])?
    } else {
        let t = tipo.trim();
        let a = alvo_id.trim();
        carregar_linhas(
            conn,
            "t.usuario_id = ?1 AND t.dispositivo_origem = ?2 AND t.tipo = ?3 AND t.alvo_id = ?4",
            &[&usuario_id, &dispositivo_atual, &t, &a],
        )?
    };
    let Some(l) = linhas.first() else {
        return Ok(None);
    };
    if l.usuario_id != usuario_id {
        return Err(Erro::validacao(
            "Este trabalho pertence a outro usuário. Pelo sigilo do trabalho em andamento, só quem começou pode abrir.",
        ));
    }
    let recuado = relogio_recuado_em(conn, agora_ms())?;
    let (resumo, estado_val, percurso_val) = avaliar_linha(conn, dispositivo_atual, recuado, l)?;
    if resumo.conflito_origem {
        return Err(Erro::validacao(
            "Este trabalho foi começado em outro caixa e veio de um backup restaurado. Na versão atual o trabalho não muda de caixa automaticamente.",
        ));
    }
    let resultado_concluida =
        operacao_ja_concluida(conn, &l.tipo, &l.chave_operacao, &l.usuario_id)?.map(|(_, v)| v).unwrap_or(Value::Null);
    let estado = if resumo.situacao == "sem_conserto" || resumo.formato_incompativel || resumo.ja_concluida {
        Value::Null
    } else {
        estado_val
    };
    Ok(Some(TrabalhoAberto { resumo, estado, percurso: percurso_val, rascunho: true, resultado_concluida }))
}

/// Remove o trabalho do próprio usuário quando a operação dele é concluída normalmente (`REC-1`).
pub fn concluir_meu(
    conn: &Connection,
    dispositivo_atual: &str,
    usuario_id: &str,
    id: &str,
    tipo: &str,
    alvo_id: &str,
) -> Resultado<bool> {
    let apagados = if !id.trim().is_empty() {
        let dono: Option<(String, String)> = conn
            .query_row("SELECT usuario_id, dispositivo_origem FROM trabalho WHERE id = ?1", [id.trim()], |l| {
                Ok((l.get(0)?, l.get(1)?))
            })
            .optional()?;
        if let Some((u, disp)) = dono {
            if u != usuario_id {
                return Err(Erro::validacao("Só o próprio usuário pode encerrar este trabalho."));
            }
            if disp != dispositivo_atual {
                return Err(Erro::validacao(
                    "Este trabalho veio de outro caixa no backup e não pode ser encerrado como se fosse deste caixa.",
                ));
            }
        }
        conn.execute(
            "DELETE FROM trabalho WHERE id = ?1 AND usuario_id = ?2 AND dispositivo_origem = ?3",
            params![id.trim(), usuario_id, dispositivo_atual],
        )?
    } else {
        conn.execute(
            "DELETE FROM trabalho WHERE usuario_id = ?1 AND dispositivo_origem = ?2 AND tipo = ?3 AND alvo_id = ?4",
            params![usuario_id, dispositivo_atual, tipo.trim(), alvo_id.trim()],
        )?
    };
    Ok(apagados > 0)
}

/// Descarta o trabalho do próprio usuário, exigindo confirmação na tela e gravando no histórico (`REC-1`, `G9`).
pub fn descartar_meu(
    conn: &Connection,
    dispositivo_atual: &str,
    usuario_id: &str,
    id: &str,
    motivo: &str,
) -> Resultado<()> {
    let id_limpo = id.trim();
    if id_limpo.is_empty() {
        return Err(Erro::validacao("Informe qual trabalho deve ser descartado."));
    }
    let linhas = carregar_linhas(conn, "t.id = ?1", &[&id_limpo])?;
    let Some(l) = linhas.first() else {
        return Err(Erro::validacao("Este trabalho já não está mais na lista."));
    };
    if l.usuario_id != usuario_id {
        return Err(Erro::validacao(
            "Este trabalho pertence a outro usuário. Pelo sigilo do trabalho em andamento, só quem começou pode descartar.",
        ));
    }
    if l.conflito_origem || l.dispositivo_origem != dispositivo_atual {
        return Err(Erro::validacao(
            "Este rascunho foi começado em outro caixa e veio na restauração do backup: ele fica preservado sem descarte automático.",
        ));
    }
    conn.execute("DELETE FROM trabalho WHERE id = ?1", [id_limpo])?;
    let mut detalhes = vec![format!("{}: {}", nome_do_tipo(&l.tipo), l.titulo)];
    let m = texto::limpar(motivo);
    if !m.is_empty() {
        detalhes.push(format!("Motivo: {m}"));
    }
    auditoria::registrar(
        conn,
        dispositivo_atual,
        Some(usuario_id),
        "trabalho.descartar",
        "trabalho",
        id_limpo,
        &detalhes,
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Exceção restrita e auditável 7b (ES-5b / REC-2): trabalho de usuário desativado
// ---------------------------------------------------------------------------

fn validar_acesso_gerente_7b(
    conn: &Connection,
    dispositivo_atual: &str,
    gerente_id: &str,
    id: &str,
) -> Resultado<(LinhaBruta, String)> {
    let id_limpo = id.trim();
    if id_limpo.is_empty() {
        return Err(Erro::validacao("Informe qual trabalho do usuário desativado deseja acessar."));
    }
    let linhas = carregar_linhas(conn, "t.id = ?1", &[&id_limpo])?;
    let Some(l) = linhas.into_iter().next() else {
        return Err(Erro::validacao("Este trabalho já não está mais guardado neste caixa."));
    };
    if l.conflito_origem || l.dispositivo_origem != dispositivo_atual {
        return Err(Erro::validacao(
            "Este trabalho foi começado em outro caixa e veio de um backup restaurado (`DR-6`).",
        ));
    }
    // Regra DR-3 + 7b: se o usuário está ATIVO (e não é o próprio gerente), acesso é estritamente proibido!
    if l.usuario_ativo && l.usuario_id != gerente_id {
        return Err(Erro::validacao(
            "Este trabalho pertence a um usuário ativo. Pela regra de sigilo (DR-3), o gerente só pode abrir trabalhos de usuários que foram desativados.",
        ));
    }
    let (nome_gerente, eh_gerente_ativo): (String, bool) = conn
        .query_row("SELECT nome, ativo = 1 AND gerente = 1 FROM usuario WHERE id = ?1", [gerente_id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?
        .unwrap_or_else(|| ("Gerente".to_string(), false));
    if !eh_gerente_ativo {
        return Err(Erro::campo("gerente", "Só um gerente ativo pode acessar o trabalho de um usuário desativado."));
    }
    Ok((l, nome_gerente))
}

/// `7b` (`REC-2`): o gerente abre o trabalho de um usuário **desativado**, gravando trilha auditável completa.
pub fn gerente_abrir_desativado(
    conn: &Connection,
    dispositivo_atual: &str,
    gerente_id: &str,
    id: &str,
    motivo: &str,
) -> Resultado<TrabalhoAberto> {
    let (l, nome_gerente) = validar_acesso_gerente_7b(conn, dispositivo_atual, gerente_id, id)?;
    let motivo_limpo = texto::limpar(motivo);
    let motivo_final = if motivo_limpo.is_empty() {
        "Conferência de trabalho deixado por usuário desativado".to_string()
    } else {
        motivo_limpo
    };
    let recuado = relogio_recuado_em(conn, agora_ms())?;
    let (resumo, estado_val, percurso_val) = avaliar_linha(conn, dispositivo_atual, recuado, &l)?;
    let detalhes = vec![
        format!("Autor original: {} ({})", l.usuario_nome, l.usuario_id),
        format!("Gerente responsável: {nome_gerente} ({gerente_id})"),
        format!("Trabalho: {} · {}", nome_do_tipo(&l.tipo), l.titulo),
        format!("Ação: abriu para conferência · Motivo: {motivo_final}"),
    ];
    auditoria::registrar(
        conn,
        dispositivo_atual,
        Some(gerente_id),
        "trabalho.gerente_abrir",
        "trabalho",
        &l.id,
        &detalhes,
    )?;
    let resultado_concluida =
        operacao_ja_concluida(conn, &l.tipo, &l.chave_operacao, &l.usuario_id)?.map(|(_, v)| v).unwrap_or(Value::Null);
    let estado = if resumo.situacao == "sem_conserto" || resumo.formato_incompativel || resumo.ja_concluida {
        Value::Null
    } else {
        estado_val
    };
    Ok(TrabalhoAberto { resumo, estado, percurso: percurso_val, rascunho: true, resultado_concluida })
}

/// `7b` (`REC-2`): o gerente registra a conclusão do trabalho deixado por um usuário **desativado**
/// (após finalizar a operação na tela), removendo o rascunho e gravando trilha auditável completa.
pub fn gerente_concluir_desativado(
    conn: &Connection,
    dispositivo_atual: &str,
    gerente_id: &str,
    id: &str,
    motivo: &str,
) -> Resultado<()> {
    let (l, nome_gerente) = validar_acesso_gerente_7b(conn, dispositivo_atual, gerente_id, id)?;
    let motivo_limpo = texto::limpar(motivo);
    let motivo_final = if motivo_limpo.is_empty() {
        "Conclusão de trabalho deixado por usuário desativado".to_string()
    } else {
        motivo_limpo
    };
    conn.execute("DELETE FROM trabalho WHERE id = ?1", [&l.id])?;
    let detalhes = vec![
        format!("Autor original: {} ({})", l.usuario_nome, l.usuario_id),
        format!("Gerente responsável: {nome_gerente} ({gerente_id})"),
        format!("Trabalho: {} · {}", nome_do_tipo(&l.tipo), l.titulo),
        format!("Ação: concluiu o trabalho · Motivo: {motivo_final}"),
    ];
    auditoria::registrar(
        conn,
        dispositivo_atual,
        Some(gerente_id),
        "trabalho.gerente_concluir",
        "trabalho",
        &l.id,
        &detalhes,
    )?;
    Ok(())
}

/// `7b` (`REC-2`): o gerente descarta o trabalho deixado por um usuário **desativado**, com motivo obrigatório
/// e trilha auditável completa.
pub fn gerente_descartar_desativado(
    conn: &Connection,
    dispositivo_atual: &str,
    gerente_id: &str,
    id: &str,
    motivo: &str,
) -> Resultado<()> {
    let (l, nome_gerente) = validar_acesso_gerente_7b(conn, dispositivo_atual, gerente_id, id)?;
    let motivo_limpo = texto::limpar(motivo);
    if motivo_limpo.chars().count() < 3 {
        return Err(Erro::campo(
            "motivo",
            "Informe o motivo do descarte (pelo menos 3 letras) para ficar no histórico.",
        ));
    }
    conn.execute("DELETE FROM trabalho WHERE id = ?1", [&l.id])?;
    let detalhes = vec![
        format!("Autor original: {} ({})", l.usuario_nome, l.usuario_id),
        format!("Gerente responsável: {nome_gerente} ({gerente_id})"),
        format!("Trabalho: {} · {}", nome_do_tipo(&l.tipo), l.titulo),
        format!("Ação: descartou o trabalho · Motivo: {motivo_limpo}"),
    ];
    auditoria::registrar(
        conn,
        dispositivo_atual,
        Some(gerente_id),
        "trabalho.gerente_descartar",
        "trabalho",
        &l.id,
        &detalhes,
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// REC-6 / ES-1: Fechar a janela durante uma operação crítica
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct AvaliarFechamentoEntrada {
    pub comando_ativo: String,
    pub tela_ativa: String,
    pub tem_rascunho_aberto: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AvaliacaoFechamento {
    pub em_andamento: bool,
    pub categoria: String,
    pub pode_cancelar_com_seguranca: bool,
    pub titulo: String,
    pub explicacao: String,
    pub texto_continuar: String,
    pub texto_sair: String,
}

/// Avalia se há uma operação crítica em andamento ao pedir para fechar a janela (`ES-1` / `REC-6`).
/// - Explica claramente o que está em andamento;
/// - Quando dá para cancelar com segurança (backup comum, importação em lote entre itens, rascunho), oferece cancelar;
/// - Quando a operação é uma transação única ou troca de banco (`venda_concluir`, `receber_baixar`, `backup_restaurar`),
///   explica que precisa terminar para não deixar dúvida, mas **nunca prende a janela para sempre** (`texto_sair`
///   sempre presente após confirmação).
pub fn avaliar_fechamento(conn: &Connection, d: &AvaliarFechamentoEntrada) -> Resultado<AvaliacaoFechamento> {
    let mut cmd = d.comando_ativo.trim().to_string();
    let mut resumo_em_curso = String::new();
    if cmd.is_empty() {
        if let Some((c, r)) = conn
            .query_row("SELECT comando, resumo FROM operacao_em_curso ORDER BY iniciada_ms DESC LIMIT 1", [], |l| {
                Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?))
            })
            .optional()?
        {
            cmd = c;
            resumo_em_curso = r;
        }
    }

    if !cmd.is_empty() {
        let complemento =
            if resumo_em_curso.trim().is_empty() { String::new() } else { format!(" ({})", resumo_em_curso.trim()) };
        let av = match cmd.as_str() {
            "venda_concluir" | "receber_baixar" | "receber_lancar" | "devolucao_gravar" | "caixa_fechar"
            | "caixa_movimentar" | "pagar_pagar" | "compra_lancar" | "os_salvar" => AvaliacaoFechamento {
                em_andamento: true,
                categoria: "transacao_critica".into(),
                pode_cancelar_com_seguranca: false,
                titulo: "Uma gravação importante está terminando".into(),
                explicacao: format!(
                    "O Rustinho está gravando a operação{complemento} neste instante. Ela precisa terminar para que o dinheiro e o estoque fiquem certos. Se a gravação travou e você precisar sair mesmo assim, o banco desfaz o que não terminou e o seu rascunho mais recente continua guardado."
                ),
                texto_continuar: "Esperar terminar".into(),
                texto_sair: "Sair mesmo assim".into(),
            },
            "compra_importar" | "importar_cadastros" => AvaliacaoFechamento {
                em_andamento: true,
                categoria: "importacao".into(),
                pode_cancelar_com_seguranca: true,
                titulo: "Uma importação de arquivos está em andamento".into(),
                explicacao: format!(
                    "O Rustinho está importando os arquivos escolhidos{complemento}. Dá para cancelar e fechar com segurança: as notas já confirmadas ficam guardadas e as restantes podem ser importadas depois, sem duplicar nada."
                ),
                texto_continuar: "Continuar importando".into(),
                texto_sair: "Cancelar importação e sair".into(),
            },
            "backup_restaurar" => AvaliacaoFechamento {
                em_andamento: true,
                categoria: "restauracao_backup".into(),
                pode_cancelar_com_seguranca: false,
                titulo: "A restauração do backup está em andamento".into(),
                explicacao: "O Rustinho está trocando o banco de dados pela cópia do backup. Essa etapa precisa terminar; se o programa for fechado agora, a restauração termina sozinha na próxima vez que o Rustinho abrir (nada fica misturado)."
                    .into(),
                texto_continuar: "Esperar a restauração".into(),
                texto_sair: "Sair mesmo assim".into(),
            },
            "backup_agora" | "backup_ao_fechar" | "backup_automatico" => AvaliacaoFechamento {
                em_andamento: true,
                categoria: "backup".into(),
                pode_cancelar_com_seguranca: true,
                titulo: "Um backup está sendo gravado".into(),
                explicacao: "O Rustinho está gerando uma cópia de segurança dos dados da loja. Você pode esperar alguns segundos até terminar ou cancelar a cópia agora (os dados da loja e os backups anteriores continuam intactos)."
                    .into(),
                texto_continuar: "Esperar o backup".into(),
                texto_sair: "Cancelar backup e sair".into(),
            },
            outro => AvaliacaoFechamento {
                em_andamento: true,
                categoria: "operacao".into(),
                pode_cancelar_com_seguranca: true,
                titulo: "Há uma operação em andamento".into(),
                explicacao: format!(
                    "O comando “{outro}”{complemento} ainda está rodando. Você pode esperar terminar ou fechar o Rustinho agora (o trabalho já guardado continua preservado)."
                ),
                texto_continuar: "Esperar terminar".into(),
                texto_sair: "Fechar agora".into(),
            },
        };
        return Ok(av);
    }

    if d.tem_rascunho_aberto {
        let onde = if d.tela_ativa.trim().is_empty() { "na tela atual" } else { d.tela_ativa.trim() };
        return Ok(AvaliacaoFechamento {
            em_andamento: true,
            categoria: "rascunho_guardado".into(),
            pode_cancelar_com_seguranca: true,
            titulo: "Fechar o Rustinho com trabalho em andamento?".into(),
            explicacao: format!(
                "O preenchimento em {onde} já está guardado neste caixa. Ao entrar de novo, o Rustinho oferece “Continuar de onde parou?” como rascunho, sem concluir nada sozinho."
            ),
            texto_continuar: "Continuar trabalhando".into(),
            texto_sair: "Guardar e sair".into(),
        });
    }

    Ok(AvaliacaoFechamento {
        em_andamento: false,
        categoria: String::new(),
        pode_cancelar_com_seguranca: true,
        titulo: String::new(),
        explicacao: String::new(),
        texto_continuar: String::new(),
        texto_sair: "Sair".into(),
    })
}

// ------------------------------------------------------------- comandos das telas

pub use comandos::COMANDOS;

mod comandos {
    use serde::Deserialize;
    use serde_json::{json, Value};

    use crate::despacho::{ler, Classe, Comando, Pedido};
    use crate::erro::Resultado;
    use crate::recuperacao::{self, GuardarEntrada};

    pub const COMANDOS: &[Comando] = &[
        Comando { nome: "trabalho_abrir", classe: Classe::Leitura, funcao: trabalho_abrir },
        Comando { nome: "trabalho_concluir", classe: Classe::Caixa, funcao: trabalho_concluir },
        Comando { nome: "trabalho_descartar", classe: Classe::Caixa, funcao: trabalho_descartar },
        Comando { nome: "trabalho_gerente_abrir", classe: Classe::Gerente("usuarios"), funcao: trabalho_gerente_abrir },
        Comando {
            nome: "trabalho_gerente_concluir",
            classe: Classe::Gerente("usuarios"),
            funcao: trabalho_gerente_concluir,
        },
        Comando {
            nome: "trabalho_gerente_descartar",
            classe: Classe::Gerente("usuarios"),
            funcao: trabalho_gerente_descartar,
        },
        Comando { nome: "trabalho_guardar", classe: Classe::Caixa, funcao: trabalho_guardar },
        Comando { nome: "trabalho_listar", classe: Classe::Leitura, funcao: trabalho_listar },
    ];

    fn trabalho_guardar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: GuardarEntrada = ler(dados)?;
        let tx = p.conn.transaction()?;
        let r = recuperacao::guardar(&tx, p.disp, p.usuario_id, d)?;
        tx.commit()?;
        Ok(json!(r))
    }

    fn trabalho_listar(p: &mut Pedido, _dados: Value) -> Resultado<Value> {
        Ok(json!(recuperacao::listar(p.conn, p.disp, p.usuario_id, p.gerente)?))
    }

    fn trabalho_abrir(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: AbrirEntrada = ler(dados)?;
        Ok(json!(recuperacao::abrir_meu(p.conn, p.disp, p.usuario_id, &d.id, &d.tipo, &d.alvo_id)?))
    }

    fn trabalho_concluir(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: AbrirEntrada = ler(dados)?;
        let tx = p.conn.transaction()?;
        let removido = recuperacao::concluir_meu(&tx, p.disp, p.usuario_id, &d.id, &d.tipo, &d.alvo_id)?;
        tx.commit()?;
        Ok(json!({ "removido": removido }))
    }

    fn trabalho_descartar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: AcaoTrabalhoEntrada = ler(dados)?;
        let tx = p.conn.transaction()?;
        recuperacao::descartar_meu(&tx, p.disp, p.usuario_id, &d.id, &d.motivo)?;
        tx.commit()?;
        Ok(json!({ "descartado": true }))
    }

    fn trabalho_gerente_abrir(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: AcaoTrabalhoEntrada = ler(dados)?;
        let tx = p.conn.transaction()?;
        let aberto = recuperacao::gerente_abrir_desativado(&tx, p.disp, p.usuario_id, &d.id, &d.motivo)?;
        tx.commit()?;
        Ok(json!(aberto))
    }

    fn trabalho_gerente_concluir(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: AcaoTrabalhoEntrada = ler(dados)?;
        let tx = p.conn.transaction()?;
        recuperacao::gerente_concluir_desativado(&tx, p.disp, p.usuario_id, &d.id, &d.motivo)?;
        tx.commit()?;
        Ok(json!({ "concluido": true }))
    }

    fn trabalho_gerente_descartar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: AcaoTrabalhoEntrada = ler(dados)?;
        let tx = p.conn.transaction()?;
        recuperacao::gerente_descartar_desativado(&tx, p.disp, p.usuario_id, &d.id, &d.motivo)?;
        tx.commit()?;
        Ok(json!({ "descartado": true }))
    }

    #[derive(Deserialize, Default)]
    #[serde(default, rename_all = "camelCase")]
    struct AbrirEntrada {
        id: String,
        tipo: String,
        alvo_id: String,
    }

    #[derive(Deserialize, Default)]
    #[serde(default, rename_all = "camelCase")]
    struct AcaoTrabalhoEntrada {
        id: String,
        motivo: String,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::usuarios;

    #[test]
    fn rec1_duas_versoes_sha256_sanitizacao_e_fallback_de_corrupcao() {
        let b = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&b.conn, &b.dispositivo, "Ana", "1234", true).unwrap();

        // 1ª versão: com campos sensíveis que devem ser sanitizados antes de ir para o disco.
        let g1 = guardar(
            &b.conn,
            &b.dispositivo,
            &u.id,
            GuardarEntrada {
                tipo: "peca".into(),
                alvo_id: String::new(),
                titulo: "Filtro de óleo 5W30".into(),
                estado: json!({
                    "nome": "Filtro de óleo 5W30",
                    "custo": "25,00",
                    "senhaGerente": "segredo-que-nunca-entra",
                    "pin": "9999",
                    "aninhado": { "token": "abc", "marca": "Tecfil" }
                }),
                percurso: json!([{ "tela": "TelaPecas", "senha": "123" }]),
                ..Default::default()
            },
        )
        .unwrap();

        let gravado_cru: String =
            b.conn.query_row("SELECT estado_json FROM trabalho WHERE id = ?1", [&g1.id], |l| l.get(0)).unwrap();
        assert!(!gravado_cru.contains("segredo-que-nunca-entra"), "senha vazou no banco: {gravado_cru}");
        assert!(!gravado_cru.contains("9999"), "pin vazou no banco: {gravado_cru}");
        assert!(!gravado_cru.contains("abc"), "token vazou no banco: {gravado_cru}");
        assert!(gravado_cru.contains("Tecfil"));

        // 2ª versão: move a 1ª para estado_anterior_json.
        let g2 = guardar(
            &b.conn,
            &b.dispositivo,
            &u.id,
            GuardarEntrada {
                id: g1.id.clone(),
                tipo: "peca".into(),
                alvo_id: String::new(),
                titulo: "Filtro de óleo 5W30 Turbo".into(),
                estado: json!({
                    "nome": "Filtro de óleo 5W30 Turbo",
                    "custo": "29,90"
                }),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(g1.id, g2.id, "atualiza o mesmo trabalho");

        // Corrompe estado_json da 2ª versão: abrir_meu deve recuperar automaticamente a 1ª versão!
        b.conn
            .execute("UPDATE trabalho SET estado_json = '{\"nome\":\"corrompido\"}' WHERE id = ?1", [&g1.id])
            .unwrap();
        let rec = abrir_meu(&b.conn, &b.dispositivo, &u.id, &g1.id, "", "").unwrap().unwrap();
        assert!(rec.resumo.usou_versao_anterior, "deve acusar que usou a versão anterior íntegra");
        assert_eq!(rec.resumo.situacao, "aberto");
        assert_eq!(rec.estado["nome"], "Filtro de óleo 5W30");
        assert_eq!(rec.estado["custo"], "25,00");
        assert!(rec.rascunho);

        // Corrompe também a versão anterior: vira sem_conserto sem pânico e sem apagar a linha.
        b.conn.execute("UPDATE trabalho SET estado_anterior_json = 'estrago' WHERE id = ?1", [&g1.id]).unwrap();
        let rec2 = abrir_meu(&b.conn, &b.dispositivo, &u.id, &g1.id, "", "").unwrap().unwrap();
        assert_eq!(rec2.resumo.situacao, "sem_conserto");
        assert_eq!(rec2.estado, Value::Null);
    }

    #[test]
    fn rec2_dr3_isolamento_entre_ativos_e_excecao_auditavel_7b_para_desativado() {
        let b = Banco::em_memoria().unwrap();
        let gerente = usuarios::criar(&b.conn, &b.dispositivo, "Gerente Ana", "1234", true).unwrap();
        usuarios::definir_gerente(&b.conn, &gerente.id, true).unwrap();
        let caixa_ativo = usuarios::criar(&b.conn, &b.dispositivo, "Carlos", "1234", true).unwrap();
        let caixa_saiu = usuarios::criar(&b.conn, &b.dispositivo, "Bia", "1234", true).unwrap();

        let t_carlos = guardar(
            &b.conn,
            &b.dispositivo,
            &caixa_ativo.id,
            GuardarEntrada {
                tipo: "venda".into(),
                titulo: "Venda de Carlos".into(),
                estado: json!({ "carrinho": [{ "produtoId": "p1", "quantidade": "2" }] }),
                ..Default::default()
            },
        )
        .unwrap();
        let t_bia = guardar(
            &b.conn,
            &b.dispositivo,
            &caixa_saiu.id,
            GuardarEntrada {
                tipo: "os".into(),
                titulo: "OS Gol placa ABC-1234".into(),
                estado: json!({ "placa": "ABC-1234", "defeito": "Freio" }),
                ..Default::default()
            },
        )
        .unwrap();

        // Enquanto ambos estão ativos:
        // 1) Carlos só vê o dele.
        let p_carlos = listar(&b.conn, &b.dispositivo, &caixa_ativo.id, false).unwrap();
        assert_eq!(p_carlos.meus.len(), 1);
        assert_eq!(p_carlos.meus[0].id, t_carlos.id);
        assert!(p_carlos.de_usuarios_desativados.is_empty());

        // 2) Gerente Ana vê apenas a contagem de 2 usuários ativos com 2 trabalhos (DR-3), e NÃO pode abrir nenhum!
        let p_ana = listar(&b.conn, &b.dispositivo, &gerente.id, true).unwrap();
        assert!(p_ana.meus.is_empty());
        assert!(p_ana.de_usuarios_desativados.is_empty());
        assert_eq!(p_ana.outros_ativos.usuarios_com_trabalho, 2);
        assert_eq!(p_ana.outros_ativos.total_trabalhos, 2);
        assert!(
            gerente_abrir_desativado(&b.conn, &b.dispositivo, &gerente.id, &t_carlos.id, "tentativa").is_err(),
            "DR-3: gerente não pode abrir trabalho de usuário ativo"
        );
        assert!(
            gerente_descartar_desativado(&b.conn, &b.dispositivo, &gerente.id, &t_carlos.id, "motivo valido").is_err(),
            "DR-3: gerente não pode descartar trabalho de usuário ativo"
        );

        // 3) Agora Bia é desativada (7b):
        usuarios::alterar(&b.conn, &caixa_saiu.id, "Bia", false, None).unwrap();
        let p_ana2 = listar(&b.conn, &b.dispositivo, &gerente.id, true).unwrap();
        assert_eq!(p_ana2.outros_ativos.usuarios_com_trabalho, 1);
        assert_eq!(p_ana2.de_usuarios_desativados.len(), 1);
        assert_eq!(p_ana2.de_usuarios_desativados[0].id, t_bia.id);

        // Gerente abre o trabalho de Bia (usuária desativada) e grava auditoria completa:
        let aberto =
            gerente_abrir_desativado(&b.conn, &b.dispositivo, &gerente.id, &t_bia.id, "Terminar OS da Bia").unwrap();
        assert_eq!(aberto.estado["placa"], "ABC-1234");
        assert!(aberto.rascunho);

        // Gerente descarta com motivo:
        gerente_descartar_desativado(&b.conn, &b.dispositivo, &gerente.id, &t_bia.id, "Cliente desistiu do orçamento")
            .unwrap();
        let hist = auditoria::historico(&b.conn, "trabalho", &t_bia.id, 10).unwrap();
        assert_eq!(hist.len(), 2, "abertura + descarte auditados: {hist:?}");
        assert_eq!(hist[0].acao, "trabalho.gerente_descartar");
        assert!(hist[0].detalhes.iter().any(|d| d.contains("Bia") && d.contains("Autor original")));
        assert!(hist[0].detalhes.iter().any(|d| d.contains("Gerente Ana") && d.contains("Gerente responsável")));
        assert!(hist[0].detalhes.iter().any(|d| d.contains("Cliente desistiu do orçamento")));
        assert_eq!(hist[1].acao, "trabalho.gerente_abrir");
    }

    #[test]
    fn rec3_dr5_trinta_dias_vira_abandonado_sem_apagar_e_t03_relogio_recuado_protege() {
        let b = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&b.conn, &b.dispositivo, "Ana", "1234", true).unwrap();
        let g = guardar(
            &b.conn,
            &b.dispositivo,
            &u.id,
            GuardarEntrada {
                tipo: "pessoa".into(),
                titulo: "Cliente Oficina Zé".into(),
                estado: json!({ "nome": "Oficina Zé" }),
                ..Default::default()
            },
        )
        .unwrap();

        // Retrocede atualizado_em do trabalho para 35 dias atrás:
        let antigo = tempo::dias_atras(35);
        b.conn.execute("UPDATE trabalho SET atualizado_em = ?1 WHERE id = ?2", params![antigo, g.id]).unwrap();

        // Caso T-03: se o relógio recuou (há uma sessão no futuro), NÃO marca como abandonado por engano!
        let futuro_ms = agora_ms() + 3_600_000;
        b.conn
            .execute(
                "INSERT INTO sessao_local (id, dispositivo_id, usuario_id, aberta_em, aberta_ms, limpa)
                 VALUES (1, ?1, ?2, ?3, ?4, 1)",
                params![b.dispositivo, u.id, tempo::agora(), futuro_ms],
            )
            .unwrap();
        let p_recuado = listar(&b.conn, &b.dispositivo, &u.id, false).unwrap();
        assert!(p_recuado.relogio_recuado);
        assert_eq!(p_recuado.meus[0].situacao, "aberto", "T-03: relógio recuado não rebaixa para abandonado");

        // Com relógio normal: aparece como "abandonado" e continua no banco (nunca apagado sozinho).
        b.conn.execute("DELETE FROM sessao_local", []).unwrap();
        sincronizar_classificacoes(&b.conn, &b.dispositivo, false).unwrap();
        let p_normal = listar(&b.conn, &b.dispositivo, &u.id, false).unwrap();
        assert!(!p_normal.relogio_recuado);
        assert_eq!(p_normal.meus.len(), 1);
        assert_eq!(p_normal.meus[0].situacao, "abandonado");
        let ainda_abre = abrir_meu(&b.conn, &b.dispositivo, &u.id, &g.id, "", "").unwrap().unwrap();
        assert_eq!(ainda_abre.estado["nome"], "Oficina Zé");
    }

    #[test]
    fn rec4_dr6_backup_de_outro_caixa_preserva_e_sinaliza_conflito_origem() {
        let b = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&b.conn, &b.dispositivo, "Ana", "1234", true).unwrap();
        let g = guardar(
            &b.conn,
            "caixa-origem-A",
            &u.id,
            GuardarEntrada {
                tipo: "venda".into(),
                titulo: "Venda começada no Caixa A".into(),
                estado: json!({ "carrinho": [{ "produtoId": "p1", "quantidade": "1" }] }),
                ..Default::default()
            },
        )
        .unwrap();

        // Agora o banco é aberto no Caixa B (`b.dispositivo` != "caixa-origem-A"):
        sincronizar_classificacoes(&b.conn, &b.dispositivo, false).unwrap();
        let p = listar(&b.conn, &b.dispositivo, &u.id, true).unwrap();
        assert!(p.meus.is_empty(), "não entra como trabalho local do Caixa B");
        assert_eq!(p.de_outro_caixa.len(), 1, "fica preservado e sinalizado em de_outro_caixa");
        assert!(p.de_outro_caixa[0].conflito_origem);
        assert_eq!(p.de_outro_caixa[0].dispositivo_origem, "caixa-origem-A");
        assert!(
            abrir_meu(&b.conn, &b.dispositivo, &u.id, &g.id, "", "").is_err(),
            "DR-6: não abre no Caixa B trabalho começado no Caixa A"
        );
    }

    #[test]
    fn rec6_es1_avaliar_fechamento_explica_operacao_e_nao_prende_usuario() {
        let b = Banco::em_memoria().unwrap();
        let v = avaliar_fechamento(
            &b.conn,
            &AvaliarFechamentoEntrada { comando_ativo: "venda_concluir".into(), ..Default::default() },
        )
        .unwrap();
        assert!(v.em_andamento);
        assert!(!v.pode_cancelar_com_seguranca);
        assert!(!v.texto_sair.is_empty(), "ES-1: nunca prende a janela sem opção de saída");

        let imp = avaliar_fechamento(
            &b.conn,
            &AvaliarFechamentoEntrada { comando_ativo: "compra_importar".into(), ..Default::default() },
        )
        .unwrap();
        assert!(imp.em_andamento);
        assert!(imp.pode_cancelar_com_seguranca);
        assert!(imp.explicacao.contains("cancelar"));

        let bkp = avaliar_fechamento(
            &b.conn,
            &AvaliarFechamentoEntrada { comando_ativo: "backup_agora".into(), ..Default::default() },
        )
        .unwrap();
        assert!(bkp.em_andamento);
        assert!(bkp.pode_cancelar_com_seguranca);
    }

    #[test]
    fn rec1_revalida_conflito_base_quando_cadastro_muda_no_banco_e_rec5_integra_operacao_feita() {
        let mut s = crate::Servico::em_memoria().unwrap();
        let chamar = |s: &mut crate::Servico, cmd: &str, d: Value| -> Value {
            let txt = crate::apoio_chave::chamar(crate::api::NAO_REPETIR, cmd, &d.to_string(), |x| s.chamar(cmd, x));
            serde_json::from_str(&txt).unwrap()
        };
        assert_eq!(
            chamar(
                &mut s,
                "configurar_inicio",
                json!({ "loja": "Loja R1", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" })
            )["ok"],
            true
        );

        // Cria uma peça inicial para editar:
        let mut f = chamar(&mut s, "produto_novo", json!({}))["dados"].clone();
        f["nome"] = json!("Pastilha Dianteira");
        f["custo"] = json!("40,00");
        let peca = chamar(&mut s, "produto_salvar", f)["dados"].clone();
        let pid = peca["id"].as_str().unwrap().to_string();
        let base_ts = peca["atualizadoEm"].as_str().unwrap().to_string();

        // Guarda um rascunho de edição dessa peça com a chave "chave-peca-r5":
        let g = chamar(
            &mut s,
            "trabalho_guardar",
            json!({
                "tipo": "peca",
                "alvoId": pid,
                "titulo": "Pastilha Dianteira Editada",
                "estado": { "ficha": { "id": pid, "nome": "Pastilha Dianteira Editada" } },
                "baseAtualizadoEm": base_ts,
                "chaveOperacao": "chave-peca-r5"
            }),
        );
        assert_eq!(g["ok"], true, "{g}");
        let tid = g["dados"]["id"].as_str().unwrap().to_string();

        // Simula alteração concorrente (de outro caixa) mudando atualizado_em da peça no banco:
        s.banco_para_teste()
            .conn
            .execute("UPDATE produto SET atualizado_em = '2099-01-01T12:00:00Z' WHERE id = ?1", [&pid])
            .unwrap();

        let aberto = chamar(&mut s, "trabalho_abrir", json!({ "id": tid }))["dados"].clone();
        assert_eq!(aberto["rascunho"], true, "G7: volta sempre como rascunho");
        assert_eq!(aberto["resumo"]["conflitoBase"], true, "deve detectar que a peça mudou no banco");
        assert!(aberto["resumo"]["motivoConflito"]
            .as_str()
            .unwrap()
            .contains("alterado depois que o rascunho começou"));

        // REC-5 / DR-7: se a operação com a mesma chaveOperacao ("chave-peca-r5") já foi concluída em operacao_feita:
        let mut f2 = peca;
        f2["nome"] = json!("Pastilha Dianteira Editada");
        f2["chaveOperacao"] = json!("chave-peca-r5");
        let salvo: Value = serde_json::from_str(&s.chamar("produto_salvar", &f2.to_string())).unwrap();
        assert_eq!(salvo["ok"], true, "{salvo}");

        let aberto_depois = chamar(&mut s, "trabalho_abrir", json!({ "id": tid }))["dados"].clone();
        assert_eq!(aberto_depois["resumo"]["jaConcluida"], true, "REC-5: reconhece que a chave já foi concluída");
        assert_eq!(aberto_depois["estado"], Value::Null, "não devolve formulário para não induzir reenvio");
        assert_eq!(aberto_depois["resultadoConcluida"]["nome"], "Pastilha Dianteira Editada");
    }

    #[test]
    fn rec4_backup_real_leva_rascunho_e_restaurar_em_outro_caixa_sinaliza_conflito() {
        let _vez = crate::backup::testes::um_por_vez();
        let pasta = std::env::temp_dir().join(format!("rustinho-rec4-bkp-{}-{}", std::process::id(), tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let db = pasta.join("rustinho.db");

        let disp_a: String;
        let tid: String;
        {
            let mut s = crate::Servico::abrir_para_teste(&db, false).unwrap();
            let chamar = |s: &mut crate::Servico, cmd: &str, d: Value| -> Value {
                let txt =
                    crate::apoio_chave::chamar(crate::api::NAO_REPETIR, cmd, &d.to_string(), |x| s.chamar(cmd, x));
                serde_json::from_str(&txt).unwrap()
            };
            chamar(
                &mut s,
                "configurar_inicio",
                json!({ "loja": "Loja Backup R1", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
            );
            disp_a = s.banco_para_teste().dispositivo.clone();
            let g = chamar(
                &mut s,
                "trabalho_guardar",
                json!({
                    "tipo": "venda",
                    "titulo": "Carrinho no Caixa 1",
                    "estado": { "carrinho": [{ "produtoId": "p1", "quantidade": "3" }] }
                }),
            );
            assert_eq!(g["ok"], true, "{g}");
            tid = g["dados"]["id"].as_str().unwrap().to_string();
        }

        // Faz o backup consistente do Caixa 1 (DR-4: leva `trabalho` junto):
        let feito = crate::backup::fazer(&db, false).expect("backup com trabalho em andamento");

        // Restaura esse backup num segundo caixa (Caixa 2, com dispositivo_id diferente):
        let pasta_b = pasta.join("caixa2");
        std::fs::create_dir_all(&pasta_b).unwrap();
        let db_b = pasta_b.join("rustinho.db");
        {
            let mut b2 = Banco::abrir(&db_b).unwrap();
            let u = usuarios::criar(&b2.conn, &b2.dispositivo, "Ana", "1234", true).unwrap();
            usuarios::definir_gerente(&b2.conn, &u.id, true).unwrap();
            let disp_b = b2.dispositivo.clone();
            assert_ne!(disp_a, disp_b);
            crate::backup::restaurar(&mut b2.conn, Some(&db_b), &feito.arquivo, false, "1234").unwrap();
        }
        {
            let b2 = Banco::abrir(&db_b).unwrap();
            let ana_id: String = b2.conn.query_row("SELECT id FROM usuario LIMIT 1", [], |l| l.get(0)).unwrap();
            let painel = listar(&b2.conn, &b2.dispositivo, &ana_id, true).unwrap();
            assert!(painel.meus.is_empty(), "DR-6: não abre como trabalho local do Caixa 2");
            assert_eq!(painel.de_outro_caixa.len(), 1, "DR-4 + DR-6: preservado e sinalizado em de_outro_caixa");
            assert_eq!(painel.de_outro_caixa[0].id, tid);
            assert!(painel.de_outro_caixa[0].conflito_origem);
            assert_eq!(painel.de_outro_caixa[0].dispositivo_origem, disp_a);
        }

        let _ = std::fs::remove_dir_all(&pasta);
    }
}
