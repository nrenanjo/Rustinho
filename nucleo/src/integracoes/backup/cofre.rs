//! E7-2 e E7-3 da 0.9.19, etapa 7 (DRV-B (a), decisão do proprietário de 15/11/2026): o cofre contínuo no Google Drive.
//!
//! Além da cópia inteira do dia, este computador manda, em lotes pequenos, as alterações que **ele mesmo** gravou
//! (`alteracao.dispositivo` = este), na ordem do registro da troca de dados e sem cortar uma operação comercial
//! (`sync::alteracoes_desde`, PEND-35). O que veio de outros caixas não vai: cada caixa manda o que é dele.
//!
//! - **Formato:** o JSON do lote, cifrado no mesmo envelope das cópias (`embrulhar`, a chave da loja). Arquivo
//!   `<de>-<ate>.rbl` (16 dígitos cada), imutável.
//! - **Fila:** o lote é gerado na pasta de backup deste computador (`lotes/`), só sai de lá depois de conferido no Drive,
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
/// Alterações por consulta ao registro (o lote fecha no fim de uma operação).
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
pub(super) fn pasta_local_dos_lotes(conn: &Connection, caminho_db: &Path) -> Resultado<PathBuf> {
    Ok(ajustes(conn, Some(caminho_db))?.pasta.join("lotes"))
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

/// O conteúdo do próximo lote, a partir de `desde`: as alterações próprias até o fim da consulta (sem cortar operação),
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
    let mut desde = partida(conn)?;
    let mut novos = 0;
    while let Some((lote, ate, tem_mais)) = montar_lote(conn, desde)? {
        if lote["alteracoes"].as_array().is_some_and(|a| !a.is_empty()) {
            let nome = nome_do_lote(desde + 1, ate);
            let claro = pasta.join(format!(".{nome}.json"));
            let parcial = pasta.join(format!(".{nome}.parcial"));
            std::fs::write(&claro, serde_json::to_vec(&lote)?).map_err(|e| erro_pasta(&pasta, e))?;
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

/// Manda os lotes da pasta local para o Drive, em ordem, conferindo cada um (D3). Um que não confere para a fila.
/// Devolve quantos foram conferidos nesta volta.
pub(super) fn enviar_lotes(conn: &Connection, caminho_db: &Path) -> Resultado<usize> {
    let Some(remota) = pasta_no_drive(conn)? else { return Ok(0) };
    let pasta = pasta_local_dos_lotes(conn, caminho_db)?;
    let mut lotes: Vec<(i64, i64, String)> = std::fs::read_dir(&pasta)
        .map(|it| {
            it.filter_map(|e| e.ok())
                .filter_map(|e| {
                    let nome = e.file_name().to_string_lossy().to_string();
                    let (de, ate) = ler_nome_do_lote(&nome)?;
                    Some((de, ate, nome))
                })
                .collect()
        })
        .unwrap_or_default();
    lotes.sort();
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
        config::gravar(conn, CONFERIDO_ATE, &ate.to_string())?;
        config::gravar(conn, CONFERIDO_QUANDO, &tempo::agora())?;
        // Conferido no Drive: a cópia local do lote sai (o Drive é o cofre).
        let _ = std::fs::remove_file(&local);
        conferidos += 1;
    }
    Ok(conferidos)
}

/// Uma volta do cofre: gera e manda. O erro fica anotado (a tela mostra), e não para o backup do dia.
pub(super) fn volta(conn: &Connection, caminho_db: &Path) -> Resultado<usize> {
    if !ajustes(conn, Some(caminho_db))?.nuvem {
        return Ok(0);
    }
    let r = gerar_lotes(conn, caminho_db).and_then(|_| enviar_lotes(conn, caminho_db));
    config::gravar(conn, LOTE_ERRO, &r.as_ref().err().map(|e| e.to_string()).unwrap_or_default())?;
    r
}

/// E7-4 (P-10 (a), decisão do proprietário de 26/11/2026; testes 16 a 18 de `docs/SIMULTANEIDADE-requisitos.md`): o estado
/// da proteção deste computador. `nivel`:
/// - "sem_cofre": o Drive está desligado (nada é declarado protegido);
/// - "em_dia": nenhuma alteração deste computador sem prova;
/// - "atencao": há alteração deste computador sem prova (pendente);
/// - "critico": a mais antiga sem prova tem mais de 24 h.
///
/// Uma alteração deste computador só está protegida com prova gravada: o lote dela conferido no Drive (`CONFERIDO_ATE`),
/// ou ela é anterior à HORA DA CÓPIA do dia que foi conferida no Drive (pelo nome da cópia, e não pela hora da
/// conferência, que viria depois e cobriria alterações que a cópia não tem). As de outros caixas são de cada um.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Protecao {
    pub nivel: String,
    pub pendentes: i64,
    pub desde: Option<String>,
}

/// A hora (UTC, RFC 3339) em que foi feita a cópia do dia conferida no Drive, pelo nome dela (hora local).
fn hora_da_copia_conferida(conn: &Connection) -> Resultado<Option<String>> {
    let Some(texto) = config::ler(conn, CONFERIDA_NO_DRIVE)? else { return Ok(None) };
    let Ok(v) = serde_json::from_str::<Value>(&texto) else { return Ok(None) };
    let Some(b) = v["nome"].as_str().and_then(ler_nome) else { return Ok(None) };
    let local = format!(
        "{}-{}-{} {}:{}:{}",
        &b.dia[..4],
        &b.dia[4..6],
        &b.dia[6..8],
        &b.hora[..2],
        &b.hora[2..4],
        &b.hora[4..6]
    );
    Ok(conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', ?1, 'utc')", [&local], |l| l.get(0))?)
}

pub fn protecao(conn: &Connection) -> Resultado<Protecao> {
    if config::ler(conn, "backup_nuvem")?.as_deref() != Some("1") {
        return Ok(Protecao { nivel: "sem_cofre".into(), pendentes: 0, desde: None });
    }
    let este = config::ler(conn, "dispositivo_id")?.unwrap_or_default();
    let conferido = ler_seq(conn, CONFERIDO_ATE)?.unwrap_or(-1);
    let copia = hora_da_copia_conferida(conn)?;
    let (pendentes, desde): (i64, Option<String>) = conn.query_row(
        "SELECT COUNT(*), MIN(quando) FROM alteracao
          WHERE dispositivo = ?1 AND seq > ?2 AND (?3 IS NULL OR julianday(quando) > julianday(?3))",
        params![este, conferido, copia],
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
    /// pendente e vira crítica depois de 24 h; o lote conferido protege; a cópia do dia conferida protege só o que é
    /// anterior à hora DELA.
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
        // O lote conferido até o fim do registro: em dia.
        let fim: i64 = b.conn.query_row("SELECT MAX(seq) FROM alteracao", [], |l| l.get(0)).unwrap();
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
        // A cópia do dia conferida protege o que é anterior à hora dela (pelo nome), e não o que veio depois.
        let agora_local: String =
            b.conn.query_row("SELECT strftime('%Y%m%d-%H%M%S', 'now', 'localtime')", [], |l| l.get(0)).unwrap();
        let nome = format!("rustinho-{agora_local}-abc.{EXTENSAO}");
        config::gravar(&b.conn, CONFERIDA_NO_DRIVE, &json!({ "nome": nome }).to_string()).unwrap();
        assert_eq!(protecao(&b.conn).unwrap().nivel, "em_dia", "a de 25 h atrás está na cópia de agora");
        b.conn
            .execute(
                "UPDATE alteracao SET quando = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '+1 hour') WHERE seq > ?1",
                [fim],
            )
            .unwrap();
        assert_eq!(protecao(&b.conn).unwrap().nivel, "atencao", "a posterior à cópia continua pendente");
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

    #[test]
    fn nome_do_lote_ida_e_volta() {
        assert_eq!(ler_nome_do_lote(&nome_do_lote(5, 1234)), Some((5, 1234)));
        assert_eq!(ler_nome_do_lote("5-1234.rbl"), None);
        assert!(nome_do_lote(9, 10) < nome_do_lote(10, 11), "a ordem alfabética é a numérica");
    }
}
