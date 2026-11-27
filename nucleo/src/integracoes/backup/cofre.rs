//! E7-2 e E7-3 da 0.9.19, etapa 7 (DRV-B (a), decisão do proprietário de 15/11/2026): o cofre contínuo no Google Drive.
//!
//! Além da cópia inteira do dia, este computador manda, em lotes pequenos, as alterações que **ele mesmo** gravou
//! (`alteracao.dispositivo` = este), na ordem do registro da troca de dados, fechando o lote no fim de uma operação
//! comercial (`sync::alteracoes_desde`, PEND-35). O que veio de outros caixas não vai: cada caixa manda o que é dele.
//! **Limite (B2 da R1 da 008):** uma operação com mais de 2000 alterações (ou 16 MiB) é cortada em lotes seguidos (D54-2):
//! uma recuperação por lotes (a D-3, depois da Beta) precisa aplicar todos os lotes da operação juntos.
//!
//! - **Formato:** o JSON do lote, cifrado no mesmo envelope das cópias (`embrulhar`, a chave da loja). Arquivo
//!   `<de>-<ate>.rbl` (16 dígitos cada), imutável.
//! - **Fila:** o lote é gerado na pasta de backup deste computador (`lotes/<dispositivo>/`), só sai de lá depois de conferido no Drive,
//!   e é mandado em ordem: um lote que não confere para a fila (nenhum é pulado), e o envio seguinte recomeça dele.
//! - **D3:** só a conferência (o tamanho e o MD5 que o próprio Drive calcula, iguais aos daqui) faz `CONFERIDO_ATE`
//!   subir. Sem ela, nada conta.
//! - **Quando:** a cada volta do backup automático (o temporizador de 60 s da tela, dentro dos 30 a 60 s da P-06), só se há
//!   alteração nova e o Drive está ligado.
//! - **Ponto de partida:** na primeira volta, o fim do registro (`LOTE_INICIO`): o que veio antes está nas cópias do dia
//!   (a proteção do que veio antes é a cópia do dia conferida; a T005 decide o estado).

use super::*;

pub(super) const EXT_LOTE: &str = "rbl";
const MAGICA_LOTE: &str = "RUSTLOTE1";
/// O registro (`alteracao.seq`) até onde os lotes já foram gerados (o próximo começa depois).
pub(crate) const GERADO_ATE: &str = "backup_lote_gerado_ate";
/// Até onde os lotes deste computador estão conferidos no Drive (D3). Só sobe, e só com prova.
pub(crate) const CONFERIDO_ATE: &str = "backup_lote_conferido_ate";
pub(crate) const CONFERIDO_QUANDO: &str = "backup_lote_conferido_quando";
/// Onde o cofre começou neste computador (o fim do registro na primeira volta).
pub(crate) const LOTE_INICIO: &str = "backup_lote_inicio";
pub(crate) const LOTE_ERRO: &str = "backup_lote_erro";
/// B6 da R1 da 008: a última limpeza dos lotes no Drive (local).
const LIMPEZA_QUANDO: &str = "backup_lote_limpeza";
/// Alterações por consulta ao registro (o lote fecha no fim de uma operação; a que passa disso é cortada, B2).
const POR_VEZ: i64 = 2000;

fn ler_seq(conn: &Connection, chave: &str) -> Resultado<Option<i64>> {
    Ok(config::ler(conn, chave)?.and_then(|v| v.trim().parse().ok()))
}

/// Na primeira volta, o cofre começa no fim do registro.
fn partida(conn: &Connection) -> Resultado<i64> {
    if let Some(v) = ler_seq(conn, GERADO_ATE)? {
        return Ok(v);
    }
    let fim: i64 = conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM alteracao", [], |l| l.get(0))?;
    config::gravar(conn, LOTE_INICIO, &fim.to_string())?;
    config::gravar(conn, GERADO_ATE, &fim.to_string())?;
    Ok(fim)
}

/// A pasta dos lotes neste computador (dentro da pasta de backup).
/// M4 da R1 da 008: `lotes/<dispositivo>/`: numa pasta de backup compartilhada (dois caixas apontando para a mesma), os
/// lotes de um nunca vão para a pasta do Drive do outro.
pub(super) fn pasta_local_dos_lotes(conn: &Connection, caminho_db: &Path) -> Resultado<PathBuf> {
    let disp = config::ler(conn, "dispositivo_id")?
        .filter(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .ok_or_else(|| Erro::Interno("o caixa sem identidade não gera lotes".into()))?;
    Ok(ajustes(conn, Some(caminho_db))?.pasta.join("lotes").join(disp))
}

/// O nome do lote: `<de>-<ate>.rbl`, com 16 dígitos (a ordem alfabética é a numérica).
pub(super) fn nome_do_lote(de: i64, ate: i64) -> String {
    format!("{de:016}-{ate:016}.{EXT_LOTE}")
}

/// `(de, ate)` de um nome de lote.
pub(super) fn ler_nome_do_lote(nome: &str) -> Option<(i64, i64)> {
    let meio = nome.strip_suffix(&format!(".{EXT_LOTE}"))?;
    let (de, ate) = meio.split_once('-')?;
    if de.len() != 16 || ate.len() != 16 {
        return None;
    }
    Some((de.parse().ok()?, ate.parse().ok()?))
}

/// O conteúdo do próximo lote, a partir de `desde`: as alterações próprias até o fim da consulta (no fim de uma operação,
/// salvo a operação maior que a consulta: B2),
/// e até onde a consulta chegou (`ate`). `None` se o registro não tem nada depois de `desde`. Um trecho só com
/// alterações de outros caixas dá lote vazio (`alteracoes` = []) com `ate` andando.
pub(super) fn montar_lote(conn: &Connection, desde: i64) -> Resultado<Option<(Value, i64, bool)>> {
    let (lista, tem_mais) = crate::sync::alteracoes_desde(conn, desde, POR_VEZ)?;
    let Some(ultima) = lista.last() else { return Ok(None) };
    let ate = ultima.seq;
    let este = config::ler(conn, "dispositivo_id")?.unwrap_or_default();
    let proprias: Vec<&crate::sync::Alteracao> = lista.iter().filter(|a| a.dispositivo == este).collect();
    let lote = json!({
        "tipo": MAGICA_LOTE,
        "loja": config::ler(conn, "loja_id")?.unwrap_or_default(),
        "dispositivo": este,
        "de": desde + 1,
        "ate": ate,
        "alteracoes": proprias,
    });
    Ok(Some((lote, ate, tem_mais)))
}

/// Gera os lotes que faltam (cifrados) na pasta local. Devolve quantos arquivos novos.
pub(super) fn gerar_lotes(conn: &Connection, caminho_db: &Path) -> Resultado<usize> {
    let publica = publica_da_loja(conn)?.ok_or_else(|| {
        Erro::validacao(
            "Um gerente precisa entrar no Rustinho antes: sem a chave da loja nada vai para o Google Drive.",
        )
    })?;
    let acessos = acessos_da_loja(conn)?;
    let pasta = pasta_local_dos_lotes(conn, caminho_db)?;
    std::fs::create_dir_all(&pasta).map_err(|e| erro_pasta(&pasta, e))?;
    // A3 da R1 da 008 (regra F01): o lote aberto nunca toca a pasta de destino (pen drive, pasta sincronizada); ele
    // fica, por um instante, na pasta temporária da pasta privada, só do usuário. As sobras de uma queda saem aqui
    // (as da pasta privada, com mais de uma hora; e o aberto que uma versão anterior tenha deixado na pasta dos lotes).
    let privada = pasta_privada(Some(caminho_db));
    apagar_sobras(&privada);
    for e in std::fs::read_dir(&pasta).into_iter().flatten().flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if n.starts_with('.') && n.ends_with(".json") {
            let _ = std::fs::remove_file(e.path());
        }
    }
    let tmp = Temporaria::nova(&privada)?;
    let mut desde = partida(conn)?;
    let mut novos = 0;
    while let Some((lote, ate, tem_mais)) = montar_lote(conn, desde)? {
        if lote["alteracoes"].as_array().is_some_and(|a| !a.is_empty()) {
            let nome = nome_do_lote(desde + 1, ate);
            let claro = tmp.0.join(format!("{nome}.json"));
            let parcial = pasta.join(format!(".{nome}.parcial"));
            std::fs::write(&claro, serde_json::to_vec(&lote)?).map_err(|e| erro_pasta(&tmp.0, e))?;
            #[cfg(test)]
            if limites_de_teste::QUEDA_NO_LOTE.with(|c| c.get()) {
                std::mem::forget(tmp);
                return Err(Erro::validacao("queda simulada"));
            }
            let r = (|| -> Resultado<()> {
                let mut saida = std::fs::File::create(&parcial).map_err(|e| erro_pasta(&pasta, e))?;
                embrulhar(&claro, &mut saida, Some(&publica), &acessos)?;
                saida.sync_all().map_err(|e| erro_pasta(&pasta, e))?;
                // O lote só aparece com o nome final depois de inteiro no disco.
                std::fs::rename(&parcial, pasta.join(&nome)).map_err(|e| erro_pasta(&pasta, e))?;
                Ok(())
            })();
            let _ = std::fs::remove_file(&claro);
            if r.is_err() {
                let _ = std::fs::remove_file(&parcial);
            }
            r?;
            novos += 1;
        }
        config::gravar(conn, GERADO_ATE, &ate.to_string())?;
        desde = ate;
        if !tem_mais {
            break;
        }
    }
    Ok(novos)
}

/// A pasta dos lotes deste computador no Drive (`<loja>/<dispositivo>/lotes`), ou `None` sem a identidade da loja.
fn pasta_no_drive(conn: &Connection) -> Resultado<Option<String>> {
    Ok(pasta_do_caixa(conn)?.map(|p| format!("{}/lotes", p.trim_end_matches("/diarias"))))
}

/// M3 da R1 da 008: os lotes em ordem (pelo começo, em número): a fila vai do mais antigo ao mais novo, e um que falha
/// segura os de depois. Nomes que não são de lote ficam de fora.
pub(super) fn em_ordem(nomes: Vec<String>) -> Vec<(i64, i64, String)> {
    let mut lotes: Vec<(i64, i64, String)> = nomes
        .into_iter()
        .filter_map(|nome| {
            let (de, ate) = ler_nome_do_lote(&nome)?;
            Some((de, ate, nome))
        })
        .collect();
    lotes.sort();
    lotes
}

/// Manda os lotes da pasta local para o Drive, em ordem, conferindo cada um (D3). Um que não confere para a fila.
/// Devolve quantos foram conferidos nesta volta.
pub(super) fn enviar_lotes(conn: &Connection, caminho_db: &Path) -> Resultado<usize> {
    let Some(remota) = pasta_no_drive(conn)? else { return Ok(0) };
    let pasta = pasta_local_dos_lotes(conn, caminho_db)?;
    let nomes: Vec<String> = std::fs::read_dir(&pasta)
        .map(|it| it.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect())
        .unwrap_or_default();
    let lotes = em_ordem(nomes);
    let mut conferidos = 0;
    for (_, ate, nome) in lotes {
        let local = pasta.join(&nome);
        let daqui = tamanho_e_md5(&local.display().to_string())?;
        let destino = format!("{remota}/{nome}");
        rclone(
            &["copyto", "--checksum", "--retries", "3", &local.display().to_string(), &destino],
            Duration::from_secs(600),
        )?;
        if daqui.1.is_empty() || tamanho_e_md5(&destino)? != daqui {
            return Err(Erro::validacao(
                "Um lote no Google Drive não confere com o deste computador: ele não conta como guardado. O Rustinho \
                 manda de novo na próxima volta.",
            ));
        }
        // B3 da R1 da 008: só sobe (um lote velho que sobrou na pasta não faz o conferido voltar).
        if ler_seq(conn, CONFERIDO_ATE)?.is_none_or(|c| ate > c) {
            config::gravar(conn, CONFERIDO_ATE, &ate.to_string())?;
        }
        config::gravar(conn, CONFERIDO_QUANDO, &tempo::agora())?;
        // Conferido no Drive: a cópia local do lote sai (o Drive é o cofre).
        let _ = std::fs::remove_file(&local);
        conferidos += 1;
    }
    Ok(conferidos)
}

/// E7-5 (P-09 (a)): os lotes ficam 7 dias no Drive.
pub(crate) const DIAS_DOS_LOTES: i64 = 7;

/// E7-5: apaga do Drive os lotes deste computador com mais de 7 dias E cobertos pela cópia do dia conferida (o `ate` do
/// lote até o `seq` lido na foto dela: o que eles têm já está nela). Sem cópia do dia conferida, nenhum sai; sem a chave confirmada por um gerente neste caixa,
/// nenhum sai (T097). A hora do lote é a que o Drive guarda (`lsf --format pt`, hora local), passada para UTC.
pub(super) fn limpar_lotes(conn: &Connection) -> Resultado<usize> {
    let Some(remota) = pasta_no_drive(conn)? else { return Ok(0) };
    if !chave_confirmada(conn)? {
        return Ok(0);
    }
    // M1: coberto pela cópia do dia conferida = o `ate` do lote dentro do que a foto tem (pelo `seq`).
    let Some(copia) = seq_da_copia_conferida(conn)? else { return Ok(0) };
    let saida = match rclone(
        &["lsf", "--files-only", "--format", "pt", "--separator", "|", &remota],
        Duration::from_secs(60),
    ) {
        Ok(s) => s,
        Err(e) if e.to_string().contains("directory not found") => return Ok(0),
        Err(e) => return Err(e),
    };
    let mut apagados = 0;
    for l in saida.lines() {
        let Some((nome, hora)) = l.split_once('|') else { continue };
        let Some((_, ate)) = ler_nome_do_lote(nome) else { continue };
        let velho: bool = conn
            .query_row(
                "SELECT julianday('now') - julianday(?1, 'utc') > ?2",
                params![hora.trim(), DIAS_DOS_LOTES],
                |r| r.get::<_, Option<bool>>(0),
            )?
            .unwrap_or(false);
        let sai = velho && ate <= copia;
        if sai && rclone(&["deletefile", &format!("{remota}/{nome}")], Duration::from_secs(60)).is_ok() {
            apagados += 1;
        }
    }
    Ok(apagados)
}

/// Uma volta do cofre: gera e manda. O erro fica anotado (a tela mostra), e não para o backup do dia.
pub(super) fn volta(conn: &Connection, caminho_db: &Path) -> Resultado<usize> {
    // B1 da R1 da 008: sem para onde mandar (o caixa sem a identidade da loja), nada é gerado (os lotes não se acumulam).
    if !ajustes(conn, Some(caminho_db))?.nuvem || pasta_no_drive(conn)?.is_none() {
        return Ok(0);
    }
    let r = gerar_lotes(conn, caminho_db).and_then(|_| enviar_lotes(conn, caminho_db));
    config::gravar(conn, LOTE_ERRO, &r.as_ref().err().map(|e| e.to_string()).unwrap_or_default())?;
    // A limpeza não desfaz nada do envio; um erro nela fica para a próxima volta. B6 da R1 da 008: no máximo uma vez
    // por hora (ela lista a pasta dos lotes no Drive).
    let hora_de_limpar: bool = match config::ler(conn, LIMPEZA_QUANDO)? {
        Some(q) if !q.is_empty() => {
            conn.query_row("SELECT ?1 <= strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-1 hour')", [&q], |l| l.get(0))?
        }
        _ => true,
    };
    if hora_de_limpar {
        let _ = limpar_lotes(conn);
        config::gravar(conn, LIMPEZA_QUANDO, &tempo::agora())?;
    }
    r
}

/// M5 da R1 da 008: a espera depois de falhas seguidas (`{"falhas": n, "ate": "<UTC>"}`; local).
pub(super) const ESPERA: &str = "backup_cofre_espera";

/// A espera (s) depois da n-ésima falha seguida: 1, 2, 4, 8 e, dali em diante, 15 min.
pub(super) fn espera_depois_de(falhas: i64) -> i64 {
    (60_i64 << (falhas.clamp(1, 5) - 1)).min(900)
}

/// A volta do temporizador: pula enquanto a espera de uma falha não passou; depois de uma falha, espera mais; um
/// acerto zera a espera.
pub(super) fn volta_com_espera(conn: &Connection, caminho_db: &Path) -> Resultado<usize> {
    let antes: Value = config::ler(conn, ESPERA)?.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or(Value::Null);
    let falhas = antes["falhas"].as_i64().unwrap_or(0);
    if let Some(ate) = antes["ate"].as_str() {
        let ainda: bool = conn.query_row("SELECT ?1 > strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [ate], |l| l.get(0))?;
        if ainda {
            return Ok(0);
        }
    }
    let r = volta(conn, caminho_db);
    if r.is_ok() {
        if falhas > 0 {
            config::gravar(conn, ESPERA, "")?;
        }
    } else {
        let n = falhas + 1;
        let ate: String = conn.query_row(
            "SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now', ?1)",
            [format!("+{} seconds", espera_depois_de(n))],
            |l| l.get(0),
        )?;
        config::gravar(conn, ESPERA, &json!({ "falhas": n, "ate": ate }).to_string())?;
    }
    r
}

/// E7-4 (P-10 (a), decisão do proprietário de 26/11/2026; testes 16 a 18 de `docs/SIMULTANEIDADE-requisitos.md`): o estado
/// da proteção deste computador. `nivel`:
/// - "sem_cofre": o Drive está desligado (nada é declarado protegido);
/// - "em_dia": nenhuma alteração deste computador sem prova;
/// - "atencao": há alteração deste computador sem prova (pendente);
/// - "critico": a mais antiga sem prova tem mais de 24 h.
///
/// Uma alteração deste computador só está protegida com prova gravada (A1 e M1 da R1 da 008):
/// - ou ela está num lote conferido no Drive, isto é, entre o `LOTE_INICIO` (exclusive) e o `CONFERIDO_ATE`;
/// - ou ela está na cópia do dia conferida no Drive, isto é, o `seq` dela é até o `MAX(seq)` deste computador lido NA
///   FOTO dessa cópia (e não pela hora, que pode vir depois da foto).
/// O que é anterior ao início dos lotes só conta com a cópia do dia. As de outros caixas são de cada um.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Protecao {
    pub nivel: String,
    pub pendentes: i64,
    pub desde: Option<String>,
}

/// M1 da R1 da 008: o `MAX(seq)` deste computador lido na foto de cada cópia do dia, pelo nome local dela (mapa local,
/// as 30 mais novas).
const SEQ_DAS_COPIAS: &str = "backup_seq_das_copias";

pub(super) fn anotar_seq_da_copia(conn: &Connection, nome: &str, seq: i64) -> Resultado<()> {
    let mut mapa: serde_json::Map<String, Value> =
        config::ler(conn, SEQ_DAS_COPIAS)?.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    mapa.insert(nome.to_string(), json!(seq));
    while mapa.len() > 30 {
        let mais_velha = mapa.keys().min().cloned();
        match mais_velha {
            Some(k) => {
                mapa.remove(&k);
            }
            None => break,
        }
    }
    config::gravar(conn, SEQ_DAS_COPIAS, &Value::Object(mapa).to_string())
}

pub(super) fn seq_da_copia(conn: &Connection, nome: &str) -> Resultado<Option<i64>> {
    Ok(config::ler(conn, SEQ_DAS_COPIAS)?
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v[nome].as_i64()))
}

/// O `seq` (deste computador) que a cópia do dia conferida no Drive tem. Sem ele (uma conferência de antes desta versão),
/// a cópia não cobre nada: nunca protegido sem prova.
fn seq_da_copia_conferida(conn: &Connection) -> Resultado<Option<i64>> {
    Ok(config::ler(conn, CONFERIDA_NO_DRIVE)?
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v["seq"].as_i64()))
}

pub fn protecao(conn: &Connection) -> Resultado<Protecao> {
    if config::ler(conn, "backup_nuvem")?.as_deref() != Some("1") {
        return Ok(Protecao { nivel: "sem_cofre".into(), pendentes: 0, desde: None });
    }
    let este = config::ler(conn, "dispositivo_id")?.unwrap_or_default();
    // A1 da R1 da 008: os lotes provam só o que está DEPOIS do início do cofre e até o conferido; o que é anterior ao
    // início só tem a prova da cópia do dia conferida (pelo `seq` da foto, M1). Antes, o primeiro lote conferido
    // "cobria" todo o histórico anterior, que não estava em arquivo nenhum.
    let inicio = ler_seq(conn, LOTE_INICIO)?;
    let conferido = ler_seq(conn, CONFERIDO_ATE)?;
    let (lote_de, lote_ate) = match (inicio, conferido) {
        (Some(i), Some(c)) if c > i => (i, c),
        _ => (0, 0),
    };
    let copia = seq_da_copia_conferida(conn)?.unwrap_or(-1);
    let (pendentes, desde): (i64, Option<String>) = conn.query_row(
        "SELECT COUNT(*), MIN(quando) FROM alteracao
          WHERE dispositivo = ?1 AND NOT (seq > ?2 AND seq <= ?3) AND seq > ?4",
        params![este, lote_de, lote_ate, copia],
        |l| Ok((l.get(0)?, l.get(1)?)),
    )?;
    let nivel = if pendentes == 0 {
        "em_dia"
    } else {
        let horas: f64 = conn
            .query_row("SELECT (julianday('now') - julianday(?1)) * 24", [desde.as_deref().unwrap_or("")], |l| {
                l.get::<_, Option<f64>>(0)
            })?
            .unwrap_or(0.0);
        if horas > 24.0 {
            "critico"
        } else {
            "atencao"
        }
    };
    Ok(Protecao { nivel: nivel.into(), pendentes, desde })
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::dados::banco::Banco;

    fn loja_de_teste(nome: &str) -> (Banco, PathBuf, PathBuf) {
        let pasta = super::super::testes::pasta_teste(nome);
        let nuvem = pasta.join("drive");
        std::fs::create_dir_all(&nuvem).unwrap();
        let caminho = pasta.join("rustinho.db");
        let b = Banco::abrir(&caminho).unwrap();
        config::gravar(&b.conn, "loja_id", "0193cccc-cccc-7ccc-cccc-cccccccccccc").unwrap();
        config::gravar(&b.conn, "backup_nuvem", "1").unwrap();
        let ana = crate::usuarios::criar(&b.conn, &b.dispositivo, "Ana", "1111", true).unwrap();
        crate::usuarios::definir_gerente(&b.conn, &ana.id, true).unwrap();
        gerente_conferido(&b.conn, &ana.id, "1111", None).unwrap();
        (b, caminho, nuvem)
    }

    /// Uma alteração gravada por outro caixa (direto no registro, como se tivesse chegado pela troca).
    fn alteracao_de_outro(conn: &Connection, id: &str) {
        conn.execute(
            "INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando, ordem)
             VALUES (?1, 'config', '\"x\"', 'gravar', '{}', 'outro-caixa', '2026-11-26T00:00:00Z', 1)",
            [id],
        )
        .unwrap();
    }

    /// E7-2: o lote leva só as alterações deste computador, começa no fim do registro da primeira volta e anda.
    #[test]
    fn e7_2_lote_leva_so_as_alteracoes_deste_computador() {
        let _trava = super::super::testes::UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        let (b, caminho, _nuvem) = loja_de_teste("cofre-proprias");
        // Antes do cofre: o que já existe fica para a cópia do dia.
        let inicio = partida(&b.conn).unwrap();
        crate::usuarios::criar(&b.conn, &b.dispositivo, "Bia", "2222", false).unwrap();
        alteracao_de_outro(&b.conn, "alt-outro-1");
        let (lote, ate, _) = montar_lote(&b.conn, inicio).unwrap().unwrap();
        let alts = lote["alteracoes"].as_array().unwrap();
        assert!(!alts.is_empty(), "a Bia (deste computador) vai no lote");
        assert!(alts.iter().all(|a| a["dispositivo"] == json!(b.dispositivo)), "só as deste computador: {alts:?}");
        assert!(ate > inicio);
        // Gerado e cifrado: o arquivo existe com o nome do intervalo e não é o JSON aberto.
        assert_eq!(gerar_lotes(&b.conn, &caminho).unwrap(), 1);
        let pasta = pasta_local_dos_lotes(&b.conn, &caminho).unwrap();
        let nomes: Vec<String> =
            std::fs::read_dir(&pasta).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
        assert_eq!(nomes.len(), 1, "{nomes:?}");
        let bytes = std::fs::read(pasta.join(&nomes[0])).unwrap();
        assert_eq!(&bytes[..8], MAGICA, "no envelope das cópias");
        assert!(!String::from_utf8_lossy(&bytes).contains("Bia"), "cifrado: o nome não aparece");
        // Nada novo: nenhum lote.
        assert_eq!(gerar_lotes(&b.conn, &caminho).unwrap(), 0);
    }

    /// A3 da R1 da 008 (regra F01): o lote aberto nunca é gravado na pasta de destino. Numa queda logo depois de
    /// gravar o aberto, ele fica só na pasta temporária da pasta privada (só do usuário) e sai na volta seguinte; o
    /// aberto que uma versão anterior tenha deixado na pasta dos lotes também sai.
    #[test]
    fn e7_2_lote_aberto_nunca_na_pasta_de_destino() {
        let _trava = super::super::testes::UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        let (b, caminho, _nuvem) = loja_de_teste("cofre-aberto-fora");
        let destino = caminho.parent().unwrap().join("destino");
        std::fs::create_dir_all(&destino).unwrap();
        config::gravar(&b.conn, "backup_pasta", &destino.display().to_string()).unwrap();
        let privada = caminho.parent().unwrap().to_path_buf();
        partida(&b.conn).unwrap();
        crate::usuarios::criar(&b.conn, &b.dispositivo, "Bia", "2222", false).unwrap();
        let pasta = pasta_local_dos_lotes(&b.conn, &caminho).unwrap();
        assert!(pasta.starts_with(&destino), "a pasta dos lotes fica no destino: {}", pasta.display());
        std::fs::create_dir_all(&pasta).unwrap();
        let velho = pasta.join(".0000000000000001-0000000000000002.rbl.json");
        std::fs::write(&velho, "{\"nome\":\"Velho\"}").unwrap();
        // Queda logo depois de gravar o aberto.
        super::super::limites_de_teste::QUEDA_NO_LOTE.with(|c| c.set(true));
        let r = gerar_lotes(&b.conn, &caminho);
        super::super::limites_de_teste::QUEDA_NO_LOTE.with(|c| c.set(false));
        assert!(r.is_err());
        assert!(!velho.exists(), "o aberto deixado por uma versão anterior saiu");
        let mut abertos_no_destino = vec![];
        let mut pilha = vec![destino.clone()];
        while let Some(d) = pilha.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                if e.path().is_dir() {
                    pilha.push(e.path());
                } else if String::from_utf8_lossy(&std::fs::read(e.path()).unwrap()).contains("Bia") {
                    abertos_no_destino.push(e.path());
                }
            }
        }
        assert!(abertos_no_destino.is_empty(), "nada aberto no destino: {abertos_no_destino:?}");
        let sobras: Vec<PathBuf> = std::fs::read_dir(&privada)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(".rustinho-tmp-"))
            .map(|e| e.path())
            .collect();
        assert_eq!(sobras.len(), 1, "a sobra da queda fica na pasta privada");
        let aberto = std::fs::read_dir(&sobras[0]).unwrap().flatten().next().unwrap().path();
        assert!(String::from_utf8_lossy(&std::fs::read(&aberto).unwrap()).contains("Bia"), "é o lote aberto");
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&sobras[0]).unwrap().permissions().mode() & 0o077, 0, "só do usuário");
        // Na volta seguinte (a sobra já velha), ela sai e o lote é gerado cifrado.
        super::super::limites_de_teste::IDADE_DAS_SOBRAS.with(|c| c.set(Some(0)));
        let r = gerar_lotes(&b.conn, &caminho);
        super::super::limites_de_teste::IDADE_DAS_SOBRAS.with(|c| c.set(None));
        assert_eq!(r.unwrap(), 1);
        assert!(!sobras[0].exists(), "a sobra saiu");
        assert!(
            std::fs::read_dir(&privada)
                .unwrap()
                .flatten()
                .all(|e| !e.file_name().to_string_lossy().starts_with(".rustinho-tmp-")),
            "nenhuma temporária ficou"
        );
        let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
    }

    /// Espera a vez do cofre ficar livre (uma volta deixada correndo por um teste termina antes do próximo).
    fn esperar_o_cofre_parar() {
        assert!(super::super::VezDoCofre::esperar(Duration::from_secs(30)).is_some(), "o cofre não parou");
    }

    fn rclone_falso(pasta: &Path, nome: &str, corpo: &str) -> PathBuf {
        let p = pasta.join(nome);
        std::fs::write(&p, format!("#!/bin/sh\n{corpo}\n")).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    /// M5 da R1 da 008: o cofre tem a vez própria; com o backup do dia ocupado (uma cópia grande indo para o Drive),
    /// a volta do cofre acontece do mesmo jeito.
    #[test]
    fn e7_2_m5_o_cofre_nao_espera_o_backup_do_dia() {
        let _trava = super::super::testes::UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        if !rclone_instalado() {
            eprintln!("rclone não instalado: teste do cofre pulado");
            return;
        }
        let (b, caminho, nuvem) = loja_de_teste("cofre-vez-propria");
        std::env::set_var("RUSTINHO_REMOTO", nuvem.display().to_string());
        partida(&b.conn).unwrap();
        crate::usuarios::criar(&b.conn, &b.dispositivo, "Bia", "2222", false).unwrap();
        let ocupado = super::super::Vez::pegar("Mandando a cópia do dia…").expect("o backup do dia estava livre");
        super::super::cofre_automatico(caminho.clone());
        let inicio = std::time::Instant::now();
        while ler_seq(&b.conn, CONFERIDO_ATE).unwrap().is_none() && inicio.elapsed() < Duration::from_secs(30) {
            std::thread::sleep(Duration::from_millis(100));
        }
        let conferido = ler_seq(&b.conn, CONFERIDO_ATE).unwrap();
        drop(ocupado);
        esperar_o_cofre_parar();
        std::env::remove_var("RUSTINHO_REMOTO");
        assert!(conferido.is_some(), "o cofre andou com o backup do dia ocupado");
        let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
    }

    /// M5 da R1 da 008: depois de uma falha, o temporizador espera mais a cada vez (1, 2, 4, 8, 15 min), sem chamar o
    /// Drive no meio; um acerto zera a espera.
    #[test]
    fn e7_2_m5_espera_crescente_depois_de_falhas() {
        let _trava = super::super::testes::UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!((1..=7).map(espera_depois_de).collect::<Vec<_>>(), vec![60, 120, 240, 480, 900, 900, 900]);
        let (b, caminho, nuvem) = loja_de_teste("cofre-espera");
        std::env::set_var("RUSTINHO_REMOTO", nuvem.display().to_string());
        let pasta = caminho.parent().unwrap().to_path_buf();
        let registro = pasta.join("chamadas.txt");
        let falso = rclone_falso(&pasta, "rclone-fora.sh", &format!("echo \"$1\" >> '{}'\nexit 1", registro.display()));
        std::env::set_var("RUSTINHO_RCLONE", falso.display().to_string());
        partida(&b.conn).unwrap();
        crate::usuarios::criar(&b.conn, &b.dispositivo, "Bia", "2222", false).unwrap();
        let chamadas = || std::fs::read_to_string(&registro).unwrap_or_default().lines().count();
        let espera = || -> Value { serde_json::from_str(&config::ler(&b.conn, ESPERA).unwrap().unwrap()).unwrap() };
        let segundos_ate = |e: &Value| -> i64 {
            b.conn
                .query_row(
                    "SELECT CAST(strftime('%s', ?1) AS INTEGER) - CAST(strftime('%s', 'now') AS INTEGER)",
                    [e["ate"].as_str().unwrap()],
                    |l| l.get(0),
                )
                .unwrap()
        };
        assert!(volta_com_espera(&b.conn, &caminho).is_err());
        let n1 = chamadas();
        assert!(n1 > 0, "chamou o Drive");
        let e1 = espera();
        assert_eq!(e1["falhas"], json!(1));
        assert!((55..=60).contains(&segundos_ate(&e1)), "1 min: {e1}");
        // Dentro da espera: nada de Drive.
        assert_eq!(volta_com_espera(&b.conn, &caminho).unwrap(), 0);
        assert_eq!(chamadas(), n1, "na espera não chama o Drive");
        // Passada a espera, tenta de novo; falha de novo: 2 min.
        config::gravar(&b.conn, ESPERA, &json!({ "falhas": 1, "ate": "2000-01-01T00:00:00Z" }).to_string()).unwrap();
        assert!(volta_com_espera(&b.conn, &caminho).is_err());
        assert!(chamadas() > n1);
        let e2 = espera();
        assert_eq!(e2["falhas"], json!(2));
        assert!((115..=120).contains(&segundos_ate(&e2)), "2 min: {e2}");
        std::env::remove_var("RUSTINHO_RCLONE");
        // O Drive volta: o acerto zera a espera.
        if rclone_instalado() {
            config::gravar(&b.conn, ESPERA, &json!({ "falhas": 2, "ate": "2000-01-01T00:00:00Z" }).to_string())
                .unwrap();
            assert_eq!(volta_com_espera(&b.conn, &caminho).unwrap(), 1);
            assert_eq!(config::ler(&b.conn, ESPERA).unwrap().unwrap_or_default(), "", "o acerto zera a espera");
        }
        std::env::remove_var("RUSTINHO_REMOTO");
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// M5 da R1 da 008: a restauração não troca o banco com o cofre no meio de uma volta (ele grava no banco): espera
    /// a vez dele e, se não vier, diz para tentar de novo.
    #[test]
    fn e7_2_m5_restauracao_espera_o_cofre() {
        let _trava = super::super::testes::UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        let (mut b, caminho, _nuvem) = loja_de_teste("cofre-restauracao");
        let cofre = super::super::VezDoCofre::pegar().expect("o cofre estava livre");
        super::super::limites_de_teste::COFRE_NA_RESTAURACAO.with(|c| c.set(Some(1)));
        let com_o_cofre =
            super::super::restaurar_por(&mut b.conn, Some(&caminho), "nao-existe.rbk", false, "1111", None);
        drop(cofre);
        let sem_o_cofre =
            super::super::restaurar_por(&mut b.conn, Some(&caminho), "nao-existe.rbk", false, "1111", None);
        super::super::limites_de_teste::COFRE_NA_RESTAURACAO.with(|c| c.set(None));
        let e = com_o_cofre.unwrap_err().to_string();
        assert!(e.contains("últimas vendas para o Google Drive"), "{e}");
        let e = sem_o_cofre.unwrap_err().to_string();
        assert!(!e.contains("últimas vendas"), "sem o cofre, a restauração segue (e falha pelo arquivo): {e}");
        let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
    }

    /// B8 e M5 da R1 da 008: ao fechar, o cofre manda o que falta para o Drive; e não segura o fechar mais que o
    /// limite (20 s; aqui 1 s, com um Drive que demora 5 s).
    #[test]
    fn e7_2_b8_ao_fechar_o_cofre_manda_o_que_falta() {
        let _trava = super::super::testes::UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        if !rclone_instalado() {
            eprintln!("rclone não instalado: teste do cofre pulado");
            return;
        }
        let (b, caminho, nuvem) = loja_de_teste("cofre-ao-fechar");
        std::env::set_var("RUSTINHO_REMOTO", nuvem.display().to_string());
        partida(&b.conn).unwrap();
        crate::usuarios::criar(&b.conn, &b.dispositivo, "Bia", "2222", false).unwrap();
        super::super::ao_fechar(&caminho).unwrap();
        let conferido = ler_seq(&b.conn, CONFERIDO_ATE).unwrap();
        assert!(conferido.is_some(), "ao fechar, o lote foi e conferiu");
        // Um Drive lento não segura o fechar além do limite.
        crate::usuarios::criar(&b.conn, &b.dispositivo, "Caio", "3333", false).unwrap();
        let lento = rclone_falso(caminho.parent().unwrap(), "rclone-lento.sh", "sleep 5\nexec rclone \"$@\"");
        std::env::set_var("RUSTINHO_RCLONE", lento.display().to_string());
        super::super::limites_de_teste::COFRE_AO_FECHAR.with(|c| c.set(Some(1)));
        let inicio = std::time::Instant::now();
        super::super::cofre_ao_fechar(&caminho);
        let levou = inicio.elapsed();
        super::super::limites_de_teste::COFRE_AO_FECHAR.with(|c| c.set(None));
        esperar_o_cofre_parar();
        std::env::remove_var("RUSTINHO_RCLONE");
        std::env::remove_var("RUSTINHO_REMOTO");
        assert!(levou < Duration::from_secs(3), "o fechar esperou {levou:?}");
        let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
    }

    /// A4 da R1 da 008: a tela chama o cofre a cada minuto, sempre (o temporizador do backup do dia passa a 5 min
    /// depois do primeiro disparo; o do cofre é outro e não muda de intervalo). Vale para as duas cópias da tela.
    #[test]
    fn e7_2_a4_o_temporizador_do_cofre_e_de_um_minuto() {
        for tela in ["../app/qml/Main.qml", "../android/rust/qml/Main.qml"] {
            let qml = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(tela)).unwrap();
            let chamada = qml.find("Api.chamar(\"backup_cofre\")").unwrap_or_else(|| panic!("{tela}: sem o cofre"));
            let inicio = qml[..chamada].rfind("Timer {").unwrap();
            let fim = chamada + qml[chamada..].find('}').unwrap();
            let bloco = &qml[inicio..fim];
            for deve in ["interval: 60 * 1000", "running: true", "repeat: true"] {
                assert!(bloco.contains(deve), "{tela}: o temporizador do cofre sem \"{deve}\":\n{bloco}");
            }
            assert!(!bloco.contains("interval ="), "{tela}: o intervalo do cofre não muda:\n{bloco}");
            assert_eq!(qml.matches("Api.chamar(\"backup_cofre\")").count(), 1, "{tela}");
        }
    }

    /// Espera o trabalho do backup em segundo plano terminar.
    fn esperar_o_backup_parar() {
        let inicio = std::time::Instant::now();
        while super::super::ocupado() && inicio.elapsed() < Duration::from_secs(60) {
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(!super::super::ocupado(), "o backup não terminou");
    }

    /// A2 da R1 da 008 (12.1-c (a)): com a credencial vencida, "Conectar" de novo chama o `config reconnect` (o
    /// navegador abre), e a conexão volta; a tela oferece "Reconectar". Controle: um erro de rede não reconecta.
    #[test]
    fn e7_6_a2_credencial_vencida_reconecta() {
        let _trava = super::super::testes::UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        if !rclone_instalado() {
            eprintln!("rclone não instalado: teste da reconexão pulado");
            return;
        }
        let (b, caminho, nuvem) = loja_de_teste("cofre-reconectar");
        std::env::set_var("RUSTINHO_REMOTO", nuvem.display().to_string());
        let pasta = caminho.parent().unwrap().to_path_buf();
        let registro = pasta.join("chamadas.txt");
        let marca = pasta.join("reconectado");
        let corpo = |erro: &str| {
            format!(
                "echo \"$@\" >> '{r}'\ncase \"$1\" in\n mkdir) [ -f '{m}' ] && exec rclone \"$@\"; echo '{erro}' >&2; exit 1;;\n \
                 config) if [ \"$2\" = reconnect ]; then touch '{m}'; exit 0; fi;;\nesac\nexec rclone \"$@\"",
                r = registro.display(),
                m = marca.display()
            )
        };
        let chamou_reconnect = || {
            std::fs::read_to_string(&registro).unwrap_or_default().lines().any(|l| l.starts_with("config reconnect"))
        };
        // Controle: sem internet, nada de reconectar (o erro fica, e a tela não oferece "Reconectar").
        let rede =
            rclone_falso(&pasta, "rclone-sem-rede.sh", &corpo("dial tcp: lookup www.googleapis.com: no such host"));
        std::env::set_var("RUSTINHO_RCLONE", rede.display().to_string());
        super::super::conectar_drive(caminho.clone()).unwrap();
        esperar_o_backup_parar();
        assert!(!chamou_reconnect(), "erro de rede não reconecta");
        let erro = config::ler(&b.conn, "backup_nuvem_erro").unwrap().unwrap_or_default();
        assert!(erro.contains("Sem internet"), "{erro}");
        assert_eq!(super::super::resumo(&b.conn, Some(&caminho), true).unwrap()["reconectar"], json!(false));
        // A credencial vencida: a tela oferece "Reconectar".
        let vencida = rclone_falso(
            &pasta,
            "rclone-vencido.sh",
            &corpo("Failed to create file system: couldn't fetch token: token expired and there's no refresh token"),
        );
        std::env::set_var("RUSTINHO_RCLONE", vencida.display().to_string());
        config::gravar(&b.conn, LOTE_ERRO, "").unwrap();
        super::super::conectar_drive(caminho.clone()).unwrap();
        esperar_o_backup_parar();
        // (o falso só deixa o mkdir passar depois do reconnect: esta volta reconecta e conecta)
        assert!(chamou_reconnect(), "a credencial vencida chama o config reconnect");
        let linha = std::fs::read_to_string(&registro).unwrap();
        assert!(linha.contains("config reconnect rustinho-drive: --auto-confirm"), "{linha}");
        assert_eq!(config::ler(&b.conn, "backup_nuvem_erro").unwrap().unwrap_or_default(), "", "conectou de novo");
        // A tela: com o erro da credencial anotado (no cofre), "Reconectar" aparece; sem ele, não.
        std::fs::remove_file(&marca).unwrap();
        config::gravar(&b.conn, LOTE_ERRO, &explicar_rclone("token expired")).unwrap();
        assert_eq!(super::super::resumo(&b.conn, Some(&caminho), true).unwrap()["reconectar"], json!(true));
        config::gravar(&b.conn, LOTE_ERRO, "").unwrap();
        assert_eq!(super::super::resumo(&b.conn, Some(&caminho), true).unwrap()["reconectar"], json!(false));
        std::env::remove_var("RUSTINHO_RCLONE");
        std::env::remove_var("RUSTINHO_REMOTO");
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// M4 da R1 da 008: dois caixas com a mesma pasta de backup: cada um manda só os lotes dele, para a pasta dele no
    /// Drive.
    #[test]
    fn e7_2_m4_pasta_de_backup_compartilhada_nao_mistura_lotes() {
        let _trava = super::super::testes::UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        if !rclone_instalado() {
            eprintln!("rclone não instalado: teste do cofre pulado");
            return;
        }
        let (b1, c1, nuvem) = loja_de_teste("cofre-compartilhada-1");
        let (b2, c2, _) = loja_de_teste("cofre-compartilhada-2");
        assert_ne!(b1.dispositivo, b2.dispositivo);
        let comum = c1.parent().unwrap().join("pasta-comum");
        std::fs::create_dir_all(&comum).unwrap();
        for b in [&b1, &b2] {
            config::gravar(&b.conn, "backup_pasta", &comum.display().to_string()).unwrap();
            partida(&b.conn).unwrap();
            crate::usuarios::criar(&b.conn, &b.dispositivo, "Bia", "2222", false).unwrap();
        }
        // O segundo caixa gera o lote dele e ainda não mandou (sem internet).
        assert_eq!(gerar_lotes(&b2.conn, &c2).unwrap(), 1);
        let do_segundo = pasta_local_dos_lotes(&b2.conn, &c2).unwrap();
        assert!(do_segundo.ends_with(&b2.dispositivo), "{}", do_segundo.display());
        // O primeiro faz a volta dele: manda só o seu.
        std::env::set_var("RUSTINHO_REMOTO", nuvem.display().to_string());
        assert_eq!(volta(&b1.conn, &c1).unwrap(), 1);
        std::env::remove_var("RUSTINHO_REMOTO");
        let no_drive = nuvem.join("0193cccc-cccc-7ccc-cccc-cccccccccccc").join(&b1.dispositivo).join("lotes");
        assert_eq!(std::fs::read_dir(&no_drive).unwrap().count(), 1, "só o lote do primeiro na pasta dele");
        assert_eq!(std::fs::read_dir(&do_segundo).unwrap().count(), 1, "o lote do segundo continua esperando por ele");
        let _ = std::fs::remove_dir_all(c1.parent().unwrap());
        let _ = std::fs::remove_dir_all(c2.parent().unwrap());
    }

    /// M3 da R1 da 008: a fila dos lotes vai em ordem, mesmo que a pasta os liste fora dela; o que não é lote fica fora.
    #[test]
    fn e7_3_m3_lotes_em_ordem() {
        let nomes = vec![
            nome_do_lote(21, 30),
            "lixo.txt".to_string(),
            nome_do_lote(1, 10),
            format!(".{}.parcial", nome_do_lote(31, 40)),
            nome_do_lote(11, 20),
        ];
        let ordem: Vec<(i64, i64)> = em_ordem(nomes).into_iter().map(|(de, ate, _)| (de, ate)).collect();
        assert_eq!(ordem, vec![(1, 10), (11, 20), (21, 30)]);
    }

    /// B1 da R1 da 008: o caixa sem a identidade da loja não tem para onde mandar: nenhum lote é gerado.
    #[test]
    fn e7_2_b1_sem_identidade_da_loja_nao_gera_lotes() {
        let _trava = super::super::testes::UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        let (b, caminho, _nuvem) = loja_de_teste("cofre-sem-loja");
        b.conn.execute("DELETE FROM config WHERE chave = 'loja_id'", []).unwrap();
        partida(&b.conn).unwrap();
        crate::usuarios::criar(&b.conn, &b.dispositivo, "Bia", "2222", false).unwrap();
        assert_eq!(volta(&b.conn, &caminho).unwrap(), 0);
        let pasta = pasta_local_dos_lotes(&b.conn, &caminho).unwrap();
        assert_eq!(std::fs::read_dir(&pasta).map(|d| d.count()).unwrap_or(0), 0, "nenhum lote acumulado");
        let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
    }

    /// B3 da R1 da 008: um lote velho que sobrou na pasta e vai de novo não faz o `CONFERIDO_ATE` voltar.
    #[test]
    fn e7_3_b3_conferido_so_sobe() {
        let _trava = super::super::testes::UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        if !rclone_instalado() {
            eprintln!("rclone não instalado: teste do cofre pulado");
            return;
        }
        let (b, caminho, nuvem) = loja_de_teste("cofre-so-sobe");
        std::env::set_var("RUSTINHO_REMOTO", nuvem.display().to_string());
        partida(&b.conn).unwrap();
        crate::usuarios::criar(&b.conn, &b.dispositivo, "Bia", "2222", false).unwrap();
        assert_eq!(gerar_lotes(&b.conn, &caminho).unwrap(), 1);
        let pasta = pasta_local_dos_lotes(&b.conn, &caminho).unwrap();
        let velho = std::fs::read_dir(&pasta).unwrap().next().unwrap().unwrap().path();
        let guardado = caminho.parent().unwrap().join("lote-velho");
        std::fs::copy(&velho, &guardado).unwrap();
        assert_eq!(enviar_lotes(&b.conn, &caminho).unwrap(), 1);
        crate::usuarios::criar(&b.conn, &b.dispositivo, "Caio", "3333", false).unwrap();
        assert_eq!(volta(&b.conn, &caminho).unwrap(), 1);
        let depois = ler_seq(&b.conn, CONFERIDO_ATE).unwrap().unwrap();
        // O velho volta para a pasta (sobrou de uma queda) e vai de novo.
        std::fs::copy(&guardado, &velho).unwrap();
        assert_eq!(enviar_lotes(&b.conn, &caminho).unwrap(), 1);
        assert_eq!(ler_seq(&b.conn, CONFERIDO_ATE).unwrap(), Some(depois), "o conferido não voltou");
        std::env::remove_var("RUSTINHO_REMOTO");
        let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
    }

    /// B6 da R1 da 008: a limpeza dos lotes (que lista a pasta deles no Drive) roda no máximo uma vez por hora, e não a
    /// cada volta de um minuto.
    #[test]
    fn e7_5_b6_limpeza_uma_vez_por_hora() {
        let _trava = super::super::testes::UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        if !rclone_instalado() {
            eprintln!("rclone não instalado: teste do cofre pulado");
            return;
        }
        let (b, caminho, nuvem) = loja_de_teste("cofre-limpeza-hora");
        std::env::set_var("RUSTINHO_REMOTO", nuvem.display().to_string());
        let pasta = caminho.parent().unwrap().to_path_buf();
        let registro = pasta.join("chamadas.txt");
        let falso = rclone_falso(
            &pasta,
            "rclone-conta.sh",
            &format!("echo \"$@\" >> '{}'\nexec rclone \"$@\"", registro.display()),
        );
        std::env::set_var("RUSTINHO_RCLONE", falso.display().to_string());
        // A limpeza só lista com a cópia do dia conferida (sem ela, nada sai e nada é listado).
        config::gravar(&b.conn, CONFERIDA_NO_DRIVE, &json!({ "nome": "x", "seq": 1 }).to_string()).unwrap();
        let limpezas = || {
            std::fs::read_to_string(&registro).unwrap_or_default().lines().filter(|l| l.contains("--format pt")).count()
        };
        partida(&b.conn).unwrap();
        for i in 0..3 {
            crate::usuarios::criar(&b.conn, &b.dispositivo, &format!("Pessoa {i}"), "2222", false).unwrap();
            volta(&b.conn, &caminho).unwrap();
        }
        assert_eq!(limpezas(), 1, "três voltas, uma limpeza");
        // Passada uma hora, limpa de novo.
        config::gravar(&b.conn, LIMPEZA_QUANDO, "2000-01-01T00:00:00Z").unwrap();
        volta(&b.conn, &caminho).unwrap();
        assert_eq!(limpezas(), 2);
        std::env::remove_var("RUSTINHO_RCLONE");
        std::env::remove_var("RUSTINHO_REMOTO");
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// E7-3 (D3): só a conferência faz o cofre andar; um lote mandado e conferido sai da pasta local, e um envio cortado
    /// (o rclone que corta o arquivo) não conta.
    #[test]
    fn e7_3_so_o_lote_conferido_conta() {
        let _trava = super::super::testes::UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        if !rclone_instalado() {
            eprintln!("rclone não instalado: teste do cofre pulado");
            return;
        }
        let (b, caminho, nuvem) = loja_de_teste("cofre-conferido");
        std::env::set_var("RUSTINHO_REMOTO", nuvem.display().to_string());
        partida(&b.conn).unwrap();
        crate::usuarios::criar(&b.conn, &b.dispositivo, "Bia", "2222", false).unwrap();
        assert_eq!(volta(&b.conn, &caminho).unwrap(), 1);
        let ate = ler_seq(&b.conn, CONFERIDO_ATE).unwrap().unwrap();
        assert_eq!(Some(ate), ler_seq(&b.conn, GERADO_ATE).unwrap(), "conferido até o gerado");
        let pasta = pasta_local_dos_lotes(&b.conn, &caminho).unwrap();
        assert_eq!(std::fs::read_dir(&pasta).unwrap().count(), 0, "conferido, sai da pasta local");
        let no_drive = nuvem.join("0193cccc-cccc-7ccc-cccc-cccccccccccc").join(&b.dispositivo).join("lotes");
        assert_eq!(std::fs::read_dir(&no_drive).unwrap().count(), 1, "no Drive, na pasta dos lotes deste caixa");
        // Um envio que chega cortado não conta: o cofre não anda e o lote fica na pasta local.
        crate::usuarios::criar(&b.conn, &b.dispositivo, "Caio", "3333", false).unwrap();
        let cortador = nuvem.parent().unwrap().join("rclone-cortado.sh");
        std::fs::write(
            &cortador,
            "#!/bin/sh\nif [ \"$1\" = copyto ]; then rclone \"$@\" || exit $?; for a; do d=$a; done; \
             truncate -s 100 \"$d\"; exit 0; fi\nexec rclone \"$@\"\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&cortador, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("RUSTINHO_RCLONE", cortador.display().to_string());
        assert!(volta(&b.conn, &caminho).is_err());
        std::env::remove_var("RUSTINHO_RCLONE");
        assert_eq!(ler_seq(&b.conn, CONFERIDO_ATE).unwrap(), Some(ate), "o cortado não fez o cofre andar");
        assert_eq!(std::fs::read_dir(&pasta).unwrap().count(), 1, "o lote fica para a próxima volta");
        assert!(!config::ler(&b.conn, LOTE_ERRO).unwrap().unwrap_or_default().is_empty(), "o erro fica anotado");
        // Na volta seguinte, com o rclone certo, o mesmo lote vai e confere.
        assert_eq!(volta(&b.conn, &caminho).unwrap(), 1);
        assert!(ler_seq(&b.conn, CONFERIDO_ATE).unwrap().unwrap() > ate);
        std::env::remove_var("RUSTINHO_REMOTO");
        let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
    }

    /// Testes 16, 17 e 18 (E7-4): nada é "em dia" sem prova; o envio sem conferência não protege; a alteração fica
    /// pendente e vira crítica depois de 24 h; o lote conferido protege; a cópia do dia conferida protege só o que a
    /// foto DELA tem (o `seq`).
    #[test]
    fn e7_4_protecao_so_com_prova() {
        let _trava = super::super::testes::UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        let (b, _caminho, _nuvem) = loja_de_teste("cofre-protecao");
        config::gravar(&b.conn, "backup_nuvem", "0").unwrap();
        assert_eq!(protecao(&b.conn).unwrap().nivel, "sem_cofre", "Drive desligado: não se declara protegido");
        config::gravar(&b.conn, "backup_nuvem", "1").unwrap();
        // 18: sem nenhuma prova, o que este computador gravou está pendente.
        let p = protecao(&b.conn).unwrap();
        assert_eq!(p.nivel, "atencao", "{p:?}");
        assert!(p.pendentes > 0 && p.desde.is_some());
        // 16: um lote mandado e não conferido não muda nada (só CONFERIDO_ATE conta).
        config::gravar(&b.conn, GERADO_ATE, "999999").unwrap();
        assert_eq!(protecao(&b.conn).unwrap().nivel, "atencao");
        // Os lotes conferidos desde o começo do registro (o cofre começou no zero): em dia.
        let fim: i64 = b.conn.query_row("SELECT MAX(seq) FROM alteracao", [], |l| l.get(0)).unwrap();
        config::gravar(&b.conn, LOTE_INICIO, "0").unwrap();
        config::gravar(&b.conn, CONFERIDO_ATE, &fim.to_string()).unwrap();
        assert_eq!(protecao(&b.conn).unwrap(), Protecao { nivel: "em_dia".into(), pendentes: 0, desde: None });
        // 17: uma venda nova (aqui, um usuário) sem lote conferido fica pendente; com mais de 24 h, crítica.
        crate::usuarios::criar(&b.conn, &b.dispositivo, "Bia", "2222", false).unwrap();
        assert_eq!(protecao(&b.conn).unwrap().nivel, "atencao");
        b.conn
            .execute(
                "UPDATE alteracao SET quando = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-25 hours') WHERE seq > ?1",
                [fim],
            )
            .unwrap();
        assert_eq!(protecao(&b.conn).unwrap().nivel, "critico");
        // M1: a cópia do dia conferida cobre o que a FOTO tem (o `seq` dela), e não o que veio depois.
        let fim2: i64 = b.conn.query_row("SELECT MAX(seq) FROM alteracao", [], |l| l.get(0)).unwrap();
        config::gravar(&b.conn, CONFERIDA_NO_DRIVE, &json!({ "nome": "x", "seq": fim2 }).to_string()).unwrap();
        assert_eq!(protecao(&b.conn).unwrap().nivel, "em_dia", "tudo até o seq da foto está na cópia");
        crate::usuarios::criar(&b.conn, &b.dispositivo, "Caio", "3333", false).unwrap();
        assert_eq!(protecao(&b.conn).unwrap().nivel, "atencao", "o posterior à foto continua pendente");
        // Uma conferência sem o `seq` (de antes desta versão) não cobre nada.
        config::gravar(&b.conn, CONFERIDA_NO_DRIVE, &json!({ "nome": "x" }).to_string()).unwrap();
        config::gravar(&b.conn, CONFERIDO_ATE, "0").unwrap();
        assert_eq!(protecao(&b.conn).unwrap().nivel, "critico");
    }

    /// A1 da R1 da 008 (a sonda r1_buraco_da_primeira_volta do revisor): o cofre começa no fim do registro; o primeiro lote
    /// conferido prova só o que veio DEPOIS do início. O histórico anterior, sem a cópia do dia conferida, continua
    /// pendente (antes: "em dia" com nada no Drive).
    /// M1 da R1 da 008: a cópia do dia guarda o `MAX(seq)` deste computador lido NA FOTO dela; o que entra depois não
    /// fica coberto por ela. (O caso "entre a foto e o nome" não é encenável aqui: a prova dele é a leitura na foto.)
    #[test]
    fn e7_4_m1_a_copia_guarda_o_seq_da_foto() {
        let _trava = super::super::testes::UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        let (b, caminho, _nuvem) = loja_de_teste("cofre-seq-da-foto");
        crate::usuarios::criar(&b.conn, &b.dispositivo, "Bia", "2222", false).unwrap();
        let antes: i64 = b
            .conn
            .query_row("SELECT MAX(seq) FROM alteracao WHERE dispositivo = ?1", [&b.dispositivo], |l| l.get(0))
            .unwrap();
        let feito = super::super::fazer(&caminho, false).unwrap();
        let nome = Path::new(&feito.arquivo).file_name().unwrap().to_string_lossy().to_string();
        assert_eq!(seq_da_copia(&b.conn, &nome).unwrap(), Some(antes), "o seq da foto, gravado com a cópia");
        crate::usuarios::criar(&b.conn, &b.dispositivo, "Caio", "3333", false).unwrap();
        assert_eq!(seq_da_copia(&b.conn, &nome).unwrap(), Some(antes), "o que veio depois não entra na cópia");
        let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
    }

    #[test]
    fn e7_4_a1_primeira_volta_nao_cobre_o_historico() {
        let _trava = super::super::testes::UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        let (b, _caminho, _nuvem) = loja_de_teste("cofre-primeira-volta");
        let antes = protecao(&b.conn).unwrap();
        assert_eq!(antes.nivel, "atencao");
        let inicio = partida(&b.conn).unwrap();
        crate::usuarios::criar(&b.conn, &b.dispositivo, "Bia", "2222", false).unwrap();
        let fim: i64 = b.conn.query_row("SELECT MAX(seq) FROM alteracao", [], |l| l.get(0)).unwrap();
        assert!(fim > inicio);
        // O primeiro lote, conferido até o fim: o histórico anterior continua sem prova.
        config::gravar(&b.conn, CONFERIDO_ATE, &fim.to_string()).unwrap();
        let p = protecao(&b.conn).unwrap();
        assert_eq!(p.nivel, "atencao", "o histórico anterior ao início não está em arquivo nenhum: {p:?}");
        assert_eq!(p.pendentes, antes.pendentes, "pendente é exatamente o histórico anterior");
        // Com a cópia do dia conferida cobrindo o histórico (o seq da foto até o início): em dia.
        config::gravar(&b.conn, CONFERIDA_NO_DRIVE, &json!({ "nome": "x", "seq": inicio }).to_string()).unwrap();
        assert_eq!(protecao(&b.conn).unwrap().nivel, "em_dia");
    }

    /// E7-4 no Início (resposta do proprietário de 26/11, perto da meia-noite): no minuto normal do envio, nenhum aviso;
    /// pendente há 5 min ou mais, Atenção; há mais de 24 h, Crítico.
    #[test]
    fn e7_4_aviso_no_inicio_so_depois_de_5_minutos() {
        let _trava = super::super::testes::UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        let (b, _caminho, _nuvem) = loja_de_teste("cofre-aviso");
        let aviso = |b: &Banco| {
            crate::avisos::do_dia(&b.conn).unwrap().into_iter().find(|a| a.id == "backup_protecao").map(|a| a.categoria)
        };
        let fim: i64 = b.conn.query_row("SELECT MAX(seq) FROM alteracao", [], |l| l.get(0)).unwrap();
        config::gravar(&b.conn, LOTE_INICIO, "0").unwrap();
        config::gravar(&b.conn, CONFERIDO_ATE, &fim.to_string()).unwrap();
        assert_eq!(aviso(&b), None, "em dia: sem aviso");
        crate::usuarios::criar(&b.conn, &b.dispositivo, "Bia", "2222", false).unwrap();
        assert_eq!(protecao(&b.conn).unwrap().nivel, "atencao");
        assert_eq!(aviso(&b), None, "pendente há segundos (o minuto normal do envio): sem aviso no Início");
        let mudar = |b: &Banco, quanto: &str| {
            b.conn
                .execute(
                    "UPDATE alteracao SET quando = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', ?2) WHERE seq > ?1",
                    params![fim, quanto],
                )
                .unwrap();
        };
        mudar(&b, "-6 minutes");
        assert_eq!(aviso(&b).as_deref(), Some("atencao"), "pendente há 6 min: Atenção");
        mudar(&b, "-25 hours");
        assert_eq!(aviso(&b).as_deref(), Some("critica"), "pendente há mais de 24 h: Crítico");
    }

    /// E7-5: no Drive, sai o lote de mais de 7 dias que é anterior à cópia do dia conferida; fica o de 2 dias, e fica o de
    /// 10 dias se a cópia conferida é mais velha que ele.
    #[test]
    fn e7_5_lotes_ficam_7_dias_e_so_saem_cobertos_pela_copia() {
        let _trava = super::super::testes::UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        if !rclone_instalado() {
            eprintln!("rclone não instalado: teste do cofre pulado");
            return;
        }
        let (b, caminho, nuvem) = loja_de_teste("cofre-limpeza");
        std::env::set_var("RUSTINHO_REMOTO", nuvem.display().to_string());
        let remota = pasta_no_drive(&b.conn).unwrap().unwrap();
        std::fs::create_dir_all(&remota).unwrap();
        let lote = |nome: &str, dias: u64| {
            let f = std::fs::File::create(Path::new(&remota).join(nome)).unwrap();
            f.set_modified(std::time::SystemTime::now() - Duration::from_secs(dias * 86400)).unwrap();
        };
        let velho = nome_do_lote(1, 10);
        let novo = nome_do_lote(11, 20);
        // M1: a cópia conferida cobre até o `seq` da foto dela.
        let copia_ate = |seq: i64| {
            config::gravar(&b.conn, CONFERIDA_NO_DRIVE, &json!({ "nome": "x", "seq": seq }).to_string()).unwrap();
        };
        lote(&velho, 10);
        lote(&novo, 2);
        // A cópia conferida tem só até o 5: o lote 1-10 tem coisa que ela não tem, e fica.
        copia_ate(5);
        assert_eq!(limpar_lotes(&b.conn).unwrap(), 0);
        // M3 (T097): sem a chave confirmada por um gerente neste caixa, nenhum lote sai.
        let publica = config::ler(&b.conn, "backup_publica").unwrap().unwrap();
        let outra: [u8; 32] = aleatorio().unwrap();
        config::gravar(&b.conn, "backup_publica", &hex(&publica_de(&outra))).unwrap();
        copia_ate(30);
        assert_eq!(limpar_lotes(&b.conn).unwrap(), 0, "chave não confirmada: nada sai");
        config::gravar(&b.conn, "backup_publica", &publica).unwrap();
        // A cópia tem até o 30: o de 10 dias está nela e sai; o de 2 dias fica (7 dias).
        assert_eq!(limpar_lotes(&b.conn).unwrap(), 1);
        let ficaram: Vec<String> =
            std::fs::read_dir(&remota).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
        assert_eq!(ficaram, vec![novo.clone()]);
        std::env::remove_var("RUSTINHO_REMOTO");
        let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
    }

    #[test]
    fn nome_do_lote_ida_e_volta() {
        assert_eq!(ler_nome_do_lote(&nome_do_lote(5, 1234)), Some((5, 1234)));
        assert_eq!(ler_nome_do_lote("5-1234.rbl"), None);
        assert!(nome_do_lote(9, 10) < nome_do_lote(10, 11), "a ordem alfabética é a numérica");
    }
}
