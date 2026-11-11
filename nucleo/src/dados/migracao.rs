//! Atualização segura da estrutura do banco de um caixa que já tem dados.
//!
//! Quando um Rustinho novo abre um banco de uma versão anterior, antes de
//! mexer na estrutura:
//!
//! 1. tira uma cópia do banco (`VACUUM INTO`, que é uma foto consistente) e
//!    guarda o SHA-256 dela;
//! 2. confere a cópia (`quick_check`, `integrity_check`);
//! 3. roda as migrações (cada uma numa transação, veja `banco::migrar`);
//! 4. depois do COMMIT, abre o banco de novo numa conexão nova e confere:
//!    integridade, versão, as mesmas quantidades de registros e as mesmas
//!    referências que não batiam antes (nenhuma nova), e a estrutura que a
//!    migração devia criar;
//! 5. só então guarda a cópia em `pre-migracao/` (nunca é apagada: regra
//!    aprovada; não há retenção para as cópias da migração).
//!
//! FALHA-06 (auditoria da 0.9.15): uma nova tentativa depois de uma falha
//! reaproveita a cópia de antes que já existe quando o banco é exatamente o
//! mesmo (SHA-256 igual, sem nada pendente no `-wal`) e a cópia passa na
//! conferência; senão, faz uma cópia nova (a que não confere fica onde está).
//! Cada banco que falhou fica guardado com nome próprio.
//!
//! Cada passo fica anotado num arquivo ao lado do banco (`<banco>.migracao`,
//! gravado e levado ao disco antes de seguir). Se o Rustinho cair no meio
//! (falta de luz, fechado à força), ao abrir de novo ele lê esse arquivo e
//! termina antes de qualquer outra coisa (vendas, troca de dados):
//!
//! - caiu copiando: o banco nem foi tocado; a cópia pela metade é apagada e
//!   tudo recomeça;
//! - caiu migrando: se a versão do banco ainda é a antiga, o SQLite desfez a
//!   transação; recomeça da cópia já conferida. Se já é a nova, o COMMIT
//!   aconteceu: vai para a conferência;
//! - caiu conferindo: confere de novo;
//! - a conferência falhou: o banco migrado é guardado como
//!   `falha-migracao-<data>.db` (nunca apagado), a cópia volta para o lugar
//!   (conferida pelo SHA-256) e o Rustinho não tenta de novo sozinho: avisa e
//!   espera alguém pedir outra tentativa (`liberar_nova_tentativa`).
//!
//! Nada aqui junta dados das duas versões: ou fica o banco migrado e
//! conferido, ou volta a cópia inteira.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::erro::{Erro, Resultado};

static ANDAMENTO: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

fn dizer(texto: &str) {
    if let Ok(mut a) = ANDAMENTO.lock() {
        *a = texto.to_string();
    }
}

/// O que a atualização está fazendo agora ("" = nada), para a tela de espera.
pub fn andamento() -> String {
    ANDAMENTO.lock().map(|a| a.clone()).unwrap_or_default()
}

/// Em que passo a atualização está (anotado no arquivo ao lado do banco).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fase {
    Copiando,
    CopiaPronta,
    Migrando,
    Validando,
    Restaurando,
    Falhou,
}

/// O que fica anotado ao lado do banco durante a atualização.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Estado {
    pub fase: Fase,
    /// Versão da estrutura antes (a da cópia) e a que se quer.
    pub versao_antes: i64,
    pub versao_alvo: i64,
    /// Nome do arquivo da cópia (na mesma pasta do banco).
    pub copia: String,
    /// SHA-256 da cópia (hexadecimal), depois de pronta.
    #[serde(default)]
    pub sha256: String,
    /// Quantos registros cada tabela tinha na cópia (para conferir depois).
    #[serde(default)]
    pub contagens: Vec<(String, i64)>,
    /// Referências que não batiam na cópia, por (tabela, tabela que falta).
    #[serde(default)]
    pub referencias: Vec<(String, String, i64)>,
    /// Por que falhou (só na fase Falhou).
    #[serde(default)]
    pub motivo: String,
    /// Onde ficou o banco migrado que falhou na conferência.
    #[serde(default)]
    pub guardado_em: String,
    pub quando: String,
}

/// O arquivo que anota a atualização em andamento.
pub fn arquivo_de_estado(caminho_db: &Path) -> PathBuf {
    let mut nome = caminho_db.as_os_str().to_owned();
    nome.push(".migracao");
    PathBuf::from(nome)
}

fn pasta_de(caminho_db: &Path) -> PathBuf {
    caminho_db.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
}

fn erro_io(o_que: &str, e: std::io::Error) -> Erro {
    Erro::Interno(format!("{o_que}: {e}"))
}

/// Grava o estado e leva ao disco (arquivo temporário + renomear + fsync da
/// pasta): ou fica o estado novo inteiro, ou o anterior.
fn gravar_estado(caminho_db: &Path, estado: &Estado) -> Resultado<()> {
    let destino = arquivo_de_estado(caminho_db);
    let mut tmp = destino.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let texto = serde_json::to_vec_pretty(estado)?;
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp).map_err(|e| erro_io("gravar o estado da atualização", e))?;
        f.write_all(&texto).map_err(|e| erro_io("gravar o estado da atualização", e))?;
        f.sync_all().map_err(|e| erro_io("gravar o estado da atualização", e))?;
    }
    std::fs::rename(&tmp, &destino).map_err(|e| erro_io("gravar o estado da atualização", e))?;
    sincronizar_pasta(&pasta_de(caminho_db));
    Ok(())
}

/// Leva ao disco a lista de arquivos da pasta (renomear/apagar). No Windows
/// não há como abrir uma pasta para isso; o NTFS registra no próprio diário.
fn sincronizar_pasta(pasta: &Path) {
    #[cfg(unix)]
    if let Ok(d) = std::fs::File::open(pasta) {
        let _ = d.sync_all();
    }
    #[cfg(not(unix))]
    let _ = pasta;
}

pub fn ler_estado(caminho_db: &Path) -> Resultado<Option<Estado>> {
    let arquivo = arquivo_de_estado(caminho_db);
    match std::fs::read(&arquivo) {
        Ok(b) => Ok(Some(serde_json::from_slice(&b).map_err(|e| {
            Erro::Interno(format!("o arquivo {} está estragado ({e}); não mexi no banco", arquivo.display()))
        })?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(erro_io("ler o estado da atualização", e)),
    }
}

fn apagar_estado(caminho_db: &Path) {
    let _ = std::fs::remove_file(arquivo_de_estado(caminho_db));
    sincronizar_pasta(&pasta_de(caminho_db));
}

fn sha256_do_arquivo(caminho: &Path) -> Resultado<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(caminho).map_err(|e| erro_io("ler a cópia do banco", e))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf).map_err(|e| erro_io("ler a cópia do banco", e))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Só nos testes: cai (como numa falta de luz) ou falha neste ponto.
fn ponto(nome: &str) {
    #[cfg(any(test, feature = "ancoras-de-teste"))]
    if std::env::var("RUSTINHO_TESTE_MIGRACAO_CAIR").is_ok_and(|v| v == nome) {
        std::process::abort();
    }
    let _ = nome;
}

fn falhar_aqui(nome: &str) -> bool {
    #[cfg(any(test, feature = "ancoras-de-teste"))]
    if std::env::var("RUSTINHO_TESTE_MIGRACAO_FALHAR").is_ok_and(|v| v == nome) {
        return true;
    }
    let _ = nome;
    false
}

fn versao(conn: &Connection) -> Resultado<i64> {
    Ok(conn.query_row("PRAGMA user_version", [], |l| l.get(0))?)
}

/// Quantos registros cada tabela tem (as tabelas do Rustinho).
pub(crate) fn contagens(conn: &Connection) -> Resultado<Vec<(String, i64)>> {
    let tabelas: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?
        .query_map([], |l| l.get(0))?
        .collect::<Result<_, _>>()?;
    let mut lista = Vec::new();
    for t in tabelas {
        let n: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM \"{}\"", t.replace('"', "")), [], |l| l.get(0))?;
        lista.push((t, n));
    }
    Ok(lista)
}

/// Referências que não batem, agrupadas por (tabela, tabela que falta).
pub(crate) fn referencias(conn: &Connection) -> Resultado<Vec<(String, String, i64)>> {
    Ok(conn
        .prepare("SELECT \"table\", parent, COUNT(*) FROM pragma_foreign_key_check GROUP BY 1, 2 ORDER BY 1, 2")?
        .query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?
        .collect::<Result<_, _>>()?)
}

fn conferir_integridade(conn: &Connection, o_que: &str) -> Resultado<()> {
    for pragma in ["quick_check", "integrity_check"] {
        let r: String = conn.query_row(&format!("PRAGMA {pragma}"), [], |l| l.get(0))?;
        if r != "ok" {
            return Err(Erro::Interno(format!("{o_que}: {pragma} = {r}")));
        }
    }
    Ok(())
}

/// Confere o banco já migrado (numa conexão nova) contra o que a cópia tinha.
fn conferir_migrado(caminho_db: &Path, estado: &Estado) -> Resultado<()> {
    if falhar_aqui("conferencia") {
        return Err(Erro::Interno("falha de teste na conferência".into()));
    }
    let conn = Connection::open_with_flags(caminho_db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    crate::banco::registrar_funcoes(&conn)?;
    conferir_integridade(&conn, "banco atualizado")?;
    let v = versao(&conn)?;
    if v != estado.versao_alvo {
        return Err(Erro::Interno(format!("versão do banco atualizado é {v}, devia ser {}", estado.versao_alvo)));
    }
    // Um passo só (a 0.9.14 abrindo a 0.9.15): a migração não grava nem
    // apaga registros, então toda tabela tem de ter a mesma quantidade e as
    // referências que não batiam têm de ser as mesmas. De versões mais
    // antigas, as migrações do meio podem acrescentar registros (padrões em
    // config...): aí a conferência fica nas OS e nos itens (a tabela recriada)
    // e cada migração que recria tabela já conferiu tudo antes do COMMIT.
    let um_passo = estado.versao_alvo == estado.versao_antes + 1;
    let agora: std::collections::HashMap<String, i64> = contagens(&conn)?.into_iter().collect();
    for (tabela, n) in
        estado.contagens.iter().filter(|(t, _)| um_passo || t == "ordem_servico" || t == "ordem_servico_item")
    {
        match agora.get(tabela) {
            Some(m) if m == n => {}
            Some(m) => return Err(Erro::Interno(format!("a tabela {tabela} tinha {n} registros e ficou com {m}"))),
            // Nenhuma migração apaga tabela com dados da loja.
            None if *n == 0 => {}
            None => return Err(Erro::Interno(format!("a tabela {tabela} ({n} registros) sumiu"))),
        }
    }
    let refs = referencias(&conn)?;
    if um_passo && refs != estado.referencias {
        return Err(Erro::Interno(format!(
            "as referências que não batem mudaram: antes {:?}, depois {:?}",
            estado.referencias, refs
        )));
    }
    // O conteúdo das OS e dos itens é exatamente o da cópia.
    conn.execute("ATTACH DATABASE ?1 AS antes", [pasta_de(caminho_db).join(&estado.copia).display().to_string()])?;
    for tabela in ["ordem_servico", "ordem_servico_item"] {
        let existe_antes: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM antes.sqlite_master WHERE type = 'table' AND name = ?1",
            [tabela],
            |l| l.get(0),
        )?;
        if !existe_antes {
            continue;
        }
        let colunas: Vec<String> = conn
            .prepare(&format!("SELECT name FROM pragma_table_info('{tabela}', 'antes')"))?
            .query_map([], |l| l.get(0))?
            .collect::<Result<_, _>>()?;
        let lista = colunas.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
        let diferentes: i64 = conn.query_row(
            &format!(
                "SELECT (SELECT COUNT(*) FROM (SELECT {lista} FROM antes.\"{tabela}\" EXCEPT SELECT {lista} FROM main.\"{tabela}\"))
                      + (SELECT COUNT(*) FROM (SELECT {lista} FROM main.\"{tabela}\" EXCEPT SELECT {lista} FROM antes.\"{tabela}\"))"
            ),
            [],
            |l| l.get(0),
        )?;
        if diferentes != 0 {
            return Err(Erro::Interno(format!("{diferentes} registros de {tabela} diferem da cópia")));
        }
    }
    conn.execute("DETACH DATABASE antes", [])?;
    crate::banco::conferir_estrutura(&conn)?;
    Ok(())
}

/// Leva o banco à estrutura nova com cópia, conferência e volta em caso de
/// falha. Roda antes de o banco ser aberto para uso. Banco novo (sem
/// estrutura) ou já atualizado: não faz nada.
///
/// RVI-03: devolve `Some(defeito)` quando o banco de antes não passa na
/// conferência de integridade: aí nada foi feito (nem cópia, nem migração, nem
/// estado anotado) e o banco abre em modo seguro (A3) na estrutura antiga.
pub fn atualizar_com_copia(caminho_db: &Path) -> Resultado<Option<String>> {
    let r = atualizar_com_copia_dentro(caminho_db);
    dizer("");
    r
}

fn atualizar_com_copia_dentro(caminho_db: &Path) -> Resultado<Option<String>> {
    let alvo = crate::banco::versao_da_estrutura() as i64;
    if let Some(estado) = ler_estado(caminho_db)? {
        return retomar(caminho_db, estado, alvo);
    }
    if !caminho_db.exists() {
        return Ok(None);
    }
    let atual = {
        let conn = Connection::open(caminho_db)?;
        versao(&conn)?
    };
    if atual == 0 || atual >= alvo {
        return Ok(None);
    }
    // RVI-03: confere o banco antes de copiar e de atualizar. Com defeito, não
    // mexe em nada (A3: modo seguro na estrutura antiga; a saída é restaurar
    // um backup bom).
    dizer("Conferindo o banco antes de atualizar…");
    if let Some(defeito) = crate::banco::conferir_arquivo(caminho_db, false) {
        crate::registro::anotar(&format!(
            "atualização da estrutura (de {atual} para {alvo}) não feita: o banco tem defeito ({defeito}); modo seguro"
        ));
        return Ok(Some(defeito));
    }
    comecar(caminho_db, atual, alvo)
}

fn comecar(caminho_db: &Path, atual: i64, alvo: i64) -> Resultado<Option<String>> {
    let quando = crate::tempo::agora();
    let carimbo: String = quando.chars().filter(|c| c.is_ascii_digit()).take(14).collect();
    let nome_db =
        caminho_db.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "rustinho.db".into());
    let sufixo: String = crate::tempo::novo_id().chars().rev().take(6).collect();
    let copia = format!("{nome_db}.antes-v{atual}-{carimbo}-{sufixo}.db");
    let mut estado = Estado {
        fase: Fase::Copiando,
        versao_antes: atual,
        versao_alvo: alvo,
        copia: copia.clone(),
        sha256: String::new(),
        contagens: Vec::new(),
        referencias: Vec::new(),
        motivo: String::new(),
        guardado_em: String::new(),
        quando,
    };
    // FALHA-06: o mesmo banco de uma tentativa que falhou: a cópia de antes
    // dele já existe e confere. Nada é copiado de novo.
    if let Some((existente, sha)) = copia_igual(caminho_db, &nome_db, atual)? {
        let c = Connection::open_with_flags(pasta_de(caminho_db).join(&existente), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        estado.copia = existente.clone();
        estado.contagens = contagens(&c)?;
        estado.referencias = referencias(&c)?;
        estado.sha256 = sha;
        estado.fase = Fase::CopiaPronta;
        drop(c);
        gravar_estado(caminho_db, &estado)?;
        crate::registro::anotar(&format!(
            "atualização da estrutura: o banco é o mesmo da tentativa anterior; a cópia {existente} foi reaproveitada"
        ));
        return migrar_e_conferir(caminho_db, estado).map(|()| None);
    }
    gravar_estado(caminho_db, &estado)?;
    dizer("Guardando uma cópia do banco antes de atualizar…");
    let caminho_copia = pasta_de(caminho_db).join(&copia);
    let copiou = if falhar_aqui("copia") {
        Err(Erro::Interno("falha de teste na cópia".into()))
    } else {
        Connection::open(caminho_db)
            .and_then(|conn| conn.execute("VACUUM INTO ?1", [caminho_copia.display().to_string()]))
            .map(|_| ())
            .map_err(Erro::from)
    };
    if let Err(e) = copiou {
        return falhou_a_copia(caminho_db, &caminho_copia, &e.to_string());
    }
    if let Ok(f) = std::fs::File::open(&caminho_copia) {
        let _ = f.sync_all();
    }
    ponto("copiando");
    dizer("Conferindo a cópia do banco…");
    let conferida = Connection::open_with_flags(&caminho_copia, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(Erro::from)
        .and_then(|c| {
            conferir_integridade(&c, "cópia de antes da atualização")?;
            Ok((contagens(&c)?, referencias(&c)?))
        });
    match conferida {
        Ok((c, r)) => {
            estado.contagens = c;
            estado.referencias = r;
        }
        Err(e) => return falhou_a_copia(caminho_db, &caminho_copia, &e.to_string()),
    }
    estado.sha256 = sha256_do_arquivo(&caminho_copia)?;
    estado.fase = Fase::CopiaPronta;
    gravar_estado(caminho_db, &estado)?;
    migrar_e_conferir(caminho_db, estado).map(|()| None)
}

/// RVI-03: a cópia de antes da atualização não saiu (ou não confere). O banco
/// não foi mexido. A cópia que não deu certo sai (é desta tentativa, não é uma
/// cópia de recuperação) e nada fica anotado como "copiando". Se a
/// conferência completa do banco acusar defeito, é modo seguro; senão, erro
/// claro (disco cheio, por exemplo) e a próxima abertura tenta de novo.
fn falhou_a_copia(caminho_db: &Path, copia: &Path, motivo: &str) -> Resultado<Option<String>> {
    crate::registro::anotar(&format!("atualização da estrutura: a cópia de antes não saiu ({motivo})"));
    let _ = std::fs::remove_file(copia);
    apagar_estado(caminho_db);
    if let Some(defeito) = crate::banco::conferir_arquivo(caminho_db, true) {
        return Ok(Some(defeito));
    }
    Err(Erro::validacao(format!(
        "Não foi possível guardar a cópia de segurança do banco antes de atualizá-lo para esta versão ({motivo}). \
         O banco não foi mexido. Confira se há espaço no disco e abra o Rustinho de novo; se continuar, chame o \
         suporte."
    )))
}

/// Uma cópia de antes desta versão (`<banco>.antes-v<atual>-*.db`, na pasta
/// do banco) com exatamente o conteúdo do banco de agora, e que passa na
/// conferência. Só quando o `-wal` do banco está vazio (o arquivo do banco é
/// então todo o conteúdo dele). Devolve (nome, SHA-256).
fn copia_igual(caminho_db: &Path, nome_db: &str, atual: i64) -> Resultado<Option<(String, String)>> {
    let mut wal = caminho_db.as_os_str().to_owned();
    wal.push("-wal");
    if std::fs::metadata(PathBuf::from(wal)).map(|m| m.len() > 0).unwrap_or(false) {
        return Ok(None);
    }
    let prefixo = format!("{nome_db}.antes-v{atual}-");
    let mut candidatas: Vec<String> = std::fs::read_dir(pasta_de(caminho_db))
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.starts_with(&prefixo) && n.ends_with(".db"))
                .collect()
        })
        .unwrap_or_default();
    if candidatas.is_empty() {
        return Ok(None);
    }
    candidatas.sort();
    let sha_banco = sha256_do_arquivo(caminho_db)?;
    for nome in candidatas.into_iter().rev() {
        let caminho = pasta_de(caminho_db).join(&nome);
        if sha256_do_arquivo(&caminho)? != sha_banco {
            continue;
        }
        let confere = Connection::open_with_flags(&caminho, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(Erro::from)
            .and_then(|c| conferir_integridade(&c, "cópia de antes da atualização"));
        if confere.is_ok() {
            return Ok(Some((nome, sha_banco)));
        }
    }
    Ok(None)
}

fn migrar_e_conferir(caminho_db: &Path, mut estado: Estado) -> Resultado<()> {
    estado.fase = Fase::Migrando;
    gravar_estado(caminho_db, &estado)?;
    ponto("antes_de_migrar");
    dizer("Atualizando a estrutura do banco…");
    let migrou = (|| -> Resultado<()> {
        let mut conn = Connection::open(caminho_db)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        crate::banco::registrar_funcoes(&conn)?;
        crate::banco::migrar(&mut conn)?;
        // O que ficou no -wal vai para o banco antes de conferir pela cópia.
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(())).ok();
        Ok(())
    })();
    if let Err(e) = migrou {
        // A transação da migração não chegou ao COMMIT (o SQLite desfez): o
        // banco continua o de antes. Mesmo assim, por garantia, a cópia volta.
        crate::registro::anotar(&format!("migração da estrutura: {e}"));
        return restaurar(caminho_db, estado, &format!("a migração deu erro: {e}"));
    }
    ponto("apos_commit");
    estado.fase = Fase::Validando;
    gravar_estado(caminho_db, &estado)?;
    validar_e_concluir(caminho_db, estado)
}

fn validar_e_concluir(caminho_db: &Path, estado: Estado) -> Resultado<()> {
    ponto("validando");
    dizer("Conferindo o banco atualizado…");
    if let Err(e) = conferir_migrado(caminho_db, &estado) {
        crate::registro::anotar(&format!("conferência da atualização: {e}"));
        return restaurar(caminho_db, estado, &e.to_string());
    }
    // Deu certo: a cópia vai para pre-migracao/ (nunca apagada).
    let pasta = pasta_de(caminho_db);
    let destino = pasta.join("pre-migracao");
    let origem = pasta.join(&estado.copia);
    if std::fs::create_dir_all(&destino).is_ok() {
        let _ = std::fs::rename(&origem, destino.join(&estado.copia));
    }
    sincronizar_pasta(&pasta);
    apagar_estado(caminho_db);
    crate::registro::anotar(&format!(
        "estrutura do banco atualizada de {} para {} (cópia de antes em pre-migracao/{})",
        estado.versao_antes, estado.versao_alvo, estado.copia
    ));
    Ok(())
}

/// Põe a cópia de volta no lugar do banco. O banco que estava lá fica
/// guardado (nunca apagado).
fn restaurar(caminho_db: &Path, mut estado: Estado, motivo: &str) -> Resultado<()> {
    estado.fase = Fase::Restaurando;
    estado.motivo = motivo.to_string();
    dizer("A atualização não deu certo: pondo o banco de antes de volta…");
    let pasta = pasta_de(caminho_db);
    if estado.guardado_em.is_empty() {
        // Nome próprio (FALHA-06): duas falhas no mesmo segundo não disputam
        // o mesmo arquivo (antes, a segunda não era guardada).
        let carimbo: String = crate::tempo::agora().chars().filter(|c| c.is_ascii_digit()).take(14).collect();
        let sufixo: String = crate::tempo::novo_id().chars().rev().take(6).collect();
        estado.guardado_em = format!("falha-migracao-{carimbo}-{sufixo}.db");
    }
    gravar_estado(caminho_db, &estado)?;
    let copia = pasta.join(&estado.copia);
    if estado.sha256.is_empty() || sha256_do_arquivo(&copia)? != estado.sha256 {
        return Err(Erro::Interno(format!(
            "a cópia de antes da atualização ({}) não confere; o banco não foi trocado. Chame o suporte.",
            copia.display()
        )));
    }
    let guardado = pasta.join(&estado.guardado_em);
    if caminho_db.exists() && !guardado.exists() {
        std::fs::rename(caminho_db, &guardado).map_err(|e| erro_io("guardar o banco que falhou", e))?;
    }
    for fim in ["-wal", "-shm"] {
        let mut de = caminho_db.as_os_str().to_owned();
        de.push(fim);
        let de = PathBuf::from(de);
        if de.exists() {
            let mut para = guardado.as_os_str().to_owned();
            para.push(fim);
            let _ = std::fs::rename(&de, PathBuf::from(para));
        }
    }
    let mut tmp = caminho_db.as_os_str().to_owned();
    tmp.push(".voltando");
    let tmp = PathBuf::from(tmp);
    std::fs::copy(&copia, &tmp).map_err(|e| erro_io("pôr a cópia de volta", e))?;
    if let Ok(f) = std::fs::File::open(&tmp) {
        let _ = f.sync_all();
    }
    if sha256_do_arquivo(&tmp)? != estado.sha256 {
        let _ = std::fs::remove_file(&tmp);
        return Err(Erro::Interno("a cópia posta de volta não confere; tente de novo".into()));
    }
    std::fs::rename(&tmp, caminho_db).map_err(|e| erro_io("pôr a cópia de volta", e))?;
    sincronizar_pasta(&pasta);
    {
        let conn = Connection::open_with_flags(caminho_db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let v = versao(&conn)?;
        if v != estado.versao_antes {
            return Err(Erro::Interno(format!(
                "o banco posto de volta tem a versão {v}, devia ser {}",
                estado.versao_antes
            )));
        }
    }
    estado.fase = Fase::Falhou;
    gravar_estado(caminho_db, &estado)?;
    Err(erro_de_falha(&estado))
}

fn erro_de_falha(estado: &Estado) -> Erro {
    Erro::validacao(format!(
        "A atualização do banco para esta versão do Rustinho não deu certo e o banco de antes foi posto de volta, \
         sem perder nada. O banco que falhou ficou guardado em {}. Motivo: {}. \
         Para tentar de novo, use \"Tentar a atualização de novo\"; se falhar outra vez, chame o suporte.",
        estado.guardado_em, estado.motivo
    ))
}

/// Termina uma atualização que ficou pela metade.
fn retomar(caminho_db: &Path, estado: Estado, alvo: i64) -> Resultado<Option<String>> {
    crate::registro::anotar(&format!("atualização da estrutura pela metade (fase {:?}): retomando", estado.fase));
    let pasta = pasta_de(caminho_db);
    match estado.fase {
        Fase::Falhou => Err(erro_de_falha(&estado)),
        Fase::Copiando => {
            // O banco não foi tocado: apaga a cópia pela metade e recomeça
            // (RVI-03: conferindo o banco antes, como na primeira vez).
            let _ = std::fs::remove_file(pasta.join(&estado.copia));
            apagar_estado(caminho_db);
            atualizar_com_copia_dentro(caminho_db)
        }
        Fase::CopiaPronta | Fase::Migrando => {
            let v = {
                let conn = Connection::open(caminho_db)?;
                versao(&conn)?
            };
            if v == estado.versao_antes {
                // A migração não foi confirmada (o SQLite desfez a transação):
                // o banco é o de antes. Confere a cópia e roda de novo.
                if sha256_do_arquivo(&pasta.join(&estado.copia))? != estado.sha256 {
                    return restaurar(caminho_db, estado, "a cópia não conferia ao retomar").map(|()| None);
                }
                migrar_e_conferir(caminho_db, estado).map(|()| None)
            } else if v == estado.versao_alvo && v == alvo {
                let mut e = estado;
                e.fase = Fase::Validando;
                gravar_estado(caminho_db, &e)?;
                validar_e_concluir(caminho_db, e).map(|()| None)
            } else {
                restaurar(caminho_db, estado, &format!("versão inesperada ({v}) ao retomar")).map(|()| None)
            }
        }
        Fase::Validando => validar_e_concluir(caminho_db, estado).map(|()| None),
        Fase::Restaurando => {
            let motivo = estado.motivo.clone();
            restaurar(caminho_db, estado, &motivo).map(|()| None)
        }
    }
}

/// Alguém pediu para tentar a atualização de novo depois de uma falha: o
/// banco atual é o de antes (a cópia que voltou); a anotação sai e a próxima
/// abertura recomeça do zero, com cópia nova.
pub fn liberar_nova_tentativa(caminho_db: &Path) -> Resultado<bool> {
    match ler_estado(caminho_db)? {
        Some(e) if e.fase == Fase::Falhou => {
            crate::registro::anotar("pedido para tentar a atualização da estrutura de novo");
            apagar_estado(caminho_db);
            Ok(true)
        }
        _ => Ok(false),
    }
}
