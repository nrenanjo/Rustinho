//! Abertura do banco de dados SQLite e atualização da sua estrutura.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{Connection, OpenFlags, OptionalExtension};

use crate::erro::{Erro, Resultado};
use crate::tempo;

/// Cada arquivo SQL é uma versão da estrutura do banco. Novas versões entram
/// no fim da lista; as antigas nunca mudam.
const MIGRACOES: &[&str] = &[
    include_str!("../migracoes/0001_inicial.sql"),
    include_str!("../migracoes/0002_pessoas_compras_fiscal.sql"),
    include_str!("../migracoes/0003_vendas_caixa_sync.sql"),
    include_str!("../migracoes/0004_crediario.sql"),
    include_str!("../migracoes/0005_financeiro.sql"),
    include_str!("../migracoes/0006_encomendas.sql"),
    include_str!("../migracoes/0007_devolucoes.sql"),
    include_str!("../migracoes/0008_oficina.sql"),
    include_str!("../migracoes/0009_cadastro.sql"),
    include_str!("../migracoes/0010_gerente.sql"),
    include_str!("../migracoes/0011_backup_conflitos.sql"),
    include_str!("../migracoes/0012_historia_das_alteracoes.sql"),
    include_str!("../migracoes/0013_faixas_de_codigo.sql"),
    include_str!("../migracoes/0014_codigo_confirmado.sql"),
    include_str!("../migracoes/0015_codigo_das_pessoas.sql"),
    include_str!("../migracoes/0016_codigo_das_pessoas_confirmado.sql"),
    include_str!("../migracoes/0017_referencias_com_prova.sql"),
    include_str!("../migracoes/0018_situacao_da_senha.sql"),
    include_str!("../migracoes/0019_caminhos_da_rede.sql"),
    include_str!("../migracoes/0020_verificacao_da_nfe.sql"),
    include_str!("../migracoes/0021_dre_gerencial.sql"),
    include_str!("../migracoes/0022_caixa_fechado.sql"),
    include_str!("../migracoes/0023_rascunho_e_pendencias.sql"),
    include_str!("../migracoes/0024_gtins_e_pendencias_xml.sql"),
    include_str!("../migracoes/0025_caixas_aprovados.sql"),
    include_str!("../migracoes/0026_indices_e_busca_normalizada.sql"),
];

/// Primeira linha da migração que recria tabelas: ela roda com as chaves
/// estrangeiras desligadas (veja `aplicar_migracao`).
const SEM_CHAVES_ESTRANGEIRAS: &str = "-- rustinho: sem_chaves_estrangeiras";

/// Número da estrutura mais nova que este Rustinho conhece.
pub fn versao_da_estrutura() -> usize {
    MIGRACOES.len()
}

// ---------------------------------------------------------------------------
// N01 (dev.3.3, DP-50): a linhagem da estrutura, conferida pelo conteúdo.
// ---------------------------------------------------------------------------

/// A forma de uma estrutura, pelo conteúdo: (tipo, nome) → o que importa de
/// cada objeto. Tabela: as colunas (nome, tipo, não nulo, posição na chave).
/// Índice: a tabela, se é único e as colunas. Visão: a tabela. NÃO o texto do
/// CREATE, que versões diferentes do SQLite reescrevem: um banco legítimo,
/// atualizado ao longo do tempo por Rustinhos antigos, não pode ser recusado
/// por isso. Sem gatilhos: nenhuma migração cria gatilho (o programa os cria
/// ao abrir, `sync::criar_gatilhos`).
type Forma = BTreeMap<(String, String), String>;

fn forma(conn: &Connection) -> Resultado<Forma> {
    let objetos: Vec<(String, String, String)> = conn
        .prepare("SELECT type, name, tbl_name FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' AND type <> 'trigger'")?
        .query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let mut f = Forma::new();
    for (tipo, nome, tabela) in objetos {
        let descricao = match tipo.as_str() {
            "table" => conn
                .prepare("SELECT name, upper(type), \"notnull\", pk FROM pragma_table_info(?1) ORDER BY cid")?
                .query_map([&nome], |l| {
                    Ok(format!(
                        "{}|{}|{}|{}",
                        l.get::<_, String>(0)?,
                        l.get::<_, String>(1)?,
                        l.get::<_, i64>(2)?,
                        l.get::<_, i64>(3)?
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?
                .join(";"),
            "index" => {
                let unico: i64 = conn
                    .query_row("SELECT \"unique\" FROM pragma_index_list(?1) WHERE name = ?2", [&tabela, &nome], |l| {
                        l.get(0)
                    })
                    .optional()?
                    .unwrap_or(0);
                let colunas: Vec<String> = conn
                    .prepare("SELECT coalesce(name, '(expressão)') FROM pragma_index_info(?1) ORDER BY seqno")?
                    .query_map([&nome], |l| l.get(0))?
                    .collect::<Result<_, _>>()?;
                format!("{tabela}|{unico}|{}", colunas.join(","))
            }
            _ => tabela,
        };
        f.insert((tipo, nome), descricao);
    }
    Ok(f)
}

/// A forma que as migrações 0001..`n` deixam, aplicadas num banco na
/// memória como num banco de verdade (uma vez por número enquanto o programa
/// está aberto).
fn forma_esperada(n: usize) -> Resultado<Forma> {
    static GUARDADAS: Mutex<BTreeMap<usize, Forma>> = Mutex::new(BTreeMap::new());
    if let Some(f) = GUARDADAS.lock().unwrap_or_else(|e| e.into_inner()).get(&n) {
        return Ok(f.clone());
    }
    let mut c = Connection::open_in_memory()?;
    c.pragma_update(None, "foreign_keys", true)?;
    registrar_funcoes(&c)?;
    for (i, sql) in MIGRACOES.iter().enumerate().take(n) {
        aplicar_migracao(&mut c, i, sql)?;
    }
    let f = forma(&c)?;
    GUARDADAS.lock().unwrap_or_else(|e| e.into_inner()).insert(n, f.clone());
    Ok(f)
}

fn diferencas_de_forma(esperada: &Forma, real: &Forma) -> Vec<String> {
    let mut d = Vec::new();
    for (k, v) in esperada {
        match real.get(k) {
            None => d.push(format!("falta {} {}", k.0, k.1)),
            Some(r) if r != v => d.push(format!("{} {} diferente", k.0, k.1)),
            _ => {}
        }
    }
    for k in real.keys().filter(|k| !esperada.contains_key(*k)) {
        d.push(format!("sobra {} {}", k.0, k.1));
    }
    d
}

/// O que a tela mostra quando o banco não é desta linha de versões.
pub const TEXTO_OUTRA_LINHA: &str = "Este banco de dados não é de uma versão do Rustinho que este programa \
     reconhece (parece de outra linha de versões, como a 0.9.18-dev.3, ou foi mudado fora do Rustinho). Nada foi \
     alterado nele. Use o Rustinho que criou este banco, ou restaure um backup, e chame o suporte.";

/// N01 (DP-50): antes de qualquer gravação (da cópia, da migração e até do
/// diário WAL), confere a estrutura do arquivo pelo conteúdo, só lendo. O
/// número (`user_version`) sozinho não basta: o 24 tem duas estruturas
/// publicadas (esta linha e a dev.3). Banco da linha dev.3 ou de estrutura
/// desconhecida é recusado sem nada gravado; o detalhe vai para o registro.
/// Número maior que o desta versão: se o banco tem tudo o que esta versão
/// conhece, igual, é de um Rustinho mais novo desta linha ("versão mais
/// nova"); senão, é de outra linha.
pub(crate) fn conferir_linhagem(caminho: &Path) -> Resultado<()> {
    if !caminho.exists() {
        return Ok(());
    }
    let (v, real) = {
        let conn =
            Connection::open_with_flags(caminho, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
        registrar_funcoes(&conn)?;
        let v: i64 = conn.query_row("PRAGMA user_version", [], |l| l.get(0))?;
        (v, forma(&conn)?)
    };
    if v == 0 && real.is_empty() {
        return Ok(()); // arquivo novo, ainda sem estrutura
    }
    let n = MIGRACOES.len();
    let (esperada, numero) = match usize::try_from(v) {
        Ok(k) if (1..=n).contains(&k) => (forma_esperada(k)?, k),
        Ok(k) if k > n => {
            let e = forma_esperada(n)?;
            if e.iter().all(|(chave, f)| real.get(chave) == Some(f)) {
                return Err(Erro::Interno(format!(
                    "este banco de dados é de uma versão mais nova do Rustinho (estrutura {v})"
                )));
            }
            (e, n)
        }
        _ => (Forma::new(), 0),
    };
    if real == esperada {
        return Ok(());
    }
    let d = diferencas_de_forma(&esperada, &real);
    crate::registro::anotar(&format!(
        "banco recusado ao abrir, sem nada gravado (N01): {} tem o número {v} e uma estrutura que não é a {numero} \
         desta linha ({} diferenças: {})",
        caminho.display(),
        d.len(),
        d.iter().take(8).cloned().collect::<Vec<_>>().join("; ")
    ));
    Err(Erro::validacao(TEXTO_OUTRA_LINHA))
}

/// Conexão com o banco mais a identidade deste computador.
pub struct Banco {
    pub conn: Connection,
    /// Identificador único deste computador (caixa), criado na primeira vez.
    pub dispositivo: String,
    /// Onde o arquivo está (vazio para banco em memória, usado nos testes).
    pub caminho: Option<PathBuf>,
    /// A3: o defeito de integridade achado ao abrir. Com defeito o banco abre
    /// em **modo seguro**: só leitura (`query_only`), sem migração, gatilhos,
    /// faixas nem troca de dados; nada é gravado até restaurar um backup bom
    /// e abrir de novo. None = conferido e íntegro (ou banco em memória).
    pub defeito: Option<String>,
}

impl Banco {
    /// Abre (ou cria) o banco no arquivo indicado.
    pub fn abrir(caminho: &Path) -> Resultado<Banco> {
        if let Some(pasta) = caminho.parent() {
            std::fs::create_dir_all(pasta)
                .map_err(|e| Erro::Interno(format!("não foi possível criar a pasta {}: {e}", pasta.display())))?;
        }
        // N01 (dev.3.3): a estrutura é conferida pelo conteúdo antes de tudo,
        // só lendo: banco de outra linha ou desconhecido é recusado sem gravar.
        conferir_linhagem(caminho)?;
        // Banco de uma versão anterior: cópia conferida antes de mexer na
        // estrutura, e volta da cópia se algo der errado (veja `migracao`).
        // RVI-03: um banco de versão anterior é conferido antes da cópia e da
        // atualização; com defeito, a atualização não roda e ele abre em modo
        // seguro na estrutura antiga, sem nada mexido.
        let defeito_antes = crate::migracao::atualizar_com_copia(caminho)?;
        let conn = Connection::open(caminho)?;
        // FALHA-06 / A3: o banco é conferido antes de qualquer gravação (até
        // antes de mudar o diário). Com defeito, modo seguro.
        if let Some(defeito) = defeito_antes.or_else(|| conferir_ao_abrir(&conn, caminho)) {
            return Self::preparar_seguro(conn, caminho.to_path_buf(), defeito);
        }
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::preparar(conn, Some(caminho.to_path_buf()), crate::computador::atual())
    }

    /// A3: banco com defeito de integridade. Só leitura: nada da preparação
    /// de sempre (estrutura, gatilhos, registro inicial, faixas, identidade),
    /// que grava. O arquivo fica como está, para restaurar ou para o suporte.
    fn preparar_seguro(conn: Connection, caminho: PathBuf, defeito: String) -> Resultado<Banco> {
        conn.pragma_update(None, "query_only", true)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        // Transação comum (a de reservar a gravação não serve sem gravar).
        let mut conn = conn;
        conn.set_transaction_behavior(rusqlite::TransactionBehavior::Deferred);
        registrar_funcoes(&conn)?;
        let dispositivo = crate::config::ler(&conn, "dispositivo_id").ok().flatten().unwrap_or_default();
        crate::registro::anotar("banco aberto em modo seguro (defeito de integridade): nada é gravado até restaurar");
        Ok(Banco { conn, dispositivo, caminho: Some(caminho), defeito: Some(defeito) })
    }

    /// A3: em modo seguro (banco com defeito)?
    pub fn em_modo_seguro(&self) -> bool {
        self.defeito.is_some()
    }

    /// Só nos testes: abre o banco como se este fosse o computador de
    /// identidade `computador` (None = o sistema não informa a identidade).
    #[cfg(test)]
    pub(crate) fn abrir_em(caminho: &Path, computador: Option<&str>) -> Resultado<Banco> {
        let conn = Connection::open(caminho)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::preparar(conn, Some(caminho.to_path_buf()), computador.map(crate::computador::impressao))
    }

    /// Banco temporário em memória (para testes).
    pub fn em_memoria() -> Resultado<Banco> {
        // Um banco em memória não vai para outro computador: a identidade
        // é sempre a mesma (assim os testes não dependem de onde rodam).
        Self::preparar(Connection::open_in_memory()?, None, Some(crate::computador::impressao("em-memoria")))
    }

    fn preparar(mut conn: Connection, caminho: Option<PathBuf>, computador: Option<String>) -> Resultado<Banco> {
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        // D-05: a transação já começa reservando a gravação. Assim, se outra
        // conexão (a troca de dados, o backup) estiver gravando, esta espera
        // a vez em vez de falhar no meio com "database is locked".
        conn.set_transaction_behavior(rusqlite::TransactionBehavior::Immediate);
        registrar_funcoes(&conn)?;
        migrar(&mut conn)?;
        // D-08: restauração que caiu no meio termina antes de tudo (a
        // identidade deste computador volta antes de ser lida).
        if let Some(c) = &caminho {
            crate::backup::terminar_restauracao_pela_metade(&mut conn, c)?;
        }
        let dispositivo = garantir_dispositivo(&conn)?;
        crate::sync::criar_gatilhos(&conn)?;
        crate::sync::registrar_existentes(&conn)?;
        crate::backup::limpar_chave_antiga(&conn)?;
        // Códigos das peças: arquivo ao lado do banco, distribuidor e
        // peças com código a definir (veja `faixas`).
        crate::faixas::preparar(&mut conn, computador.as_deref())?;
        // FALHA-01: banco de antes da identidade da loja (veja `loja`). Não
        // impede o banco de abrir: a troca de dados é que confere.
        if let Err(e) = crate::loja::garantir_ao_abrir(&conn) {
            crate::registro::anotar(&format!("loja: identidade: {e}"));
        }
        Ok(Banco { conn, dispositivo, caminho, defeito: None })
    }
}

/// FALHA-06: defeitos de integridade achados ao abrir, por banco (só nesta
/// execução do programa; ao abrir de novo, confere de novo).
fn defeitos() -> &'static std::sync::Mutex<std::collections::HashMap<PathBuf, String>> {
    static M: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<PathBuf, String>>> =
        std::sync::OnceLock::new();
    M.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// `PRAGMA quick_check` ao abrir. Devolve o defeito (None = íntegro). A
/// conferência que nem consegue rodar também conta como defeito (integridade
/// não comprovada). Com defeito (decisão A3 do dono, 0.9.15): modo seguro —
/// o banco abre só para consulta e para restaurar um backup; o registro anota,
/// o Início mostra o aviso crítico, a troca de dados e os backups automáticos
/// não rodam e uma cópia feita dele é só para o suporte (veja `backup`).
fn conferir_ao_abrir(conn: &Connection, caminho: &Path) -> Option<String> {
    conferir_com(conn, caminho, "quick_check")
}

/// A conferência (`quick_check` ou `integrity_check`, a completa), anotando o
/// defeito deste banco (veja `defeito`).
fn conferir_com(conn: &Connection, caminho: &Path, pragma: &str) -> Option<String> {
    let r: String = conn
        .query_row(&format!("PRAGMA {pragma}"), [], |l| l.get(0))
        .unwrap_or_else(|e| format!("não deu para conferir: {e}"));
    let mut m = defeitos().lock().unwrap_or_else(|e| e.into_inner());
    if r == "ok" {
        m.remove(caminho);
        None
    } else {
        let resumo: String = r.lines().find(|l| !l.starts_with("***")).unwrap_or(&r).chars().take(200).collect();
        crate::registro::anotar(&format!("banco com defeito de integridade ({pragma}): {resumo}"));
        m.insert(caminho.to_path_buf(), resumo.clone());
        Some(resumo)
    }
}

/// RVI-03: confere o arquivo do banco antes de atualizar a estrutura (nada é
/// gravado nele). `completa` = `integrity_check` (depois de a cópia falhar).
/// Devolve o defeito (None = íntegro).
pub(crate) fn conferir_arquivo(caminho: &Path, completa: bool) -> Option<String> {
    match Connection::open(caminho) {
        Ok(conn) => conferir_com(&conn, caminho, if completa { "integrity_check" } else { "quick_check" }),
        Err(e) => {
            let resumo = format!("não deu para conferir: {e}");
            crate::registro::anotar(&format!("banco com defeito de integridade: {resumo}"));
            defeitos().lock().unwrap_or_else(|e| e.into_inner()).insert(caminho.to_path_buf(), resumo.clone());
            Some(resumo)
        }
    }
}

/// O defeito de integridade achado ao abrir este banco (None = íntegro ou
/// não conferido).
pub fn defeito(caminho: &Path) -> Option<String> {
    defeitos().lock().unwrap_or_else(|e| e.into_inner()).get(caminho).cloned()
}

/// Uma restauração de backup trocou este banco e não terminou (a marca ao
/// lado dele, D-08): até ela terminar ao abrir, a troca de dados não usa o
/// banco e nada é gravado nele (0.9.18).
pub fn restauracao_pela_metade(caminho: &Path) -> bool {
    crate::backup::marca_da_restauracao(caminho).exists()
}

/// O motivo, para a tela e para a troca de dados.
pub const RESTAURACAO_PELA_METADE: &str = "O backup foi copiado, mas a restauração não terminou. Feche e abra o \
     Rustinho: ela termina ao abrir. Até lá nada é gravado e a troca de dados fica parada.";

/// A3: a gravação foi recusada porque o banco está só para leitura (modo seguro)?
pub fn erro_de_somente_leitura(e: &Erro) -> bool {
    matches!(e, Erro::Banco(rusqlite::Error::SqliteFailure(f, _)) if f.code == rusqlite::ErrorCode::ReadOnly)
}

/// Confere a integridade de uma conexão (`quick_check`). Erro com o resumo.
pub fn conferir_integridade(conn: &Connection) -> Resultado<()> {
    let r: String = conn.query_row("PRAGMA quick_check", [], |l| l.get(0))?;
    if r == "ok" {
        Ok(())
    } else {
        Err(Erro::Interno(r.lines().next().unwrap_or_default().chars().take(200).collect()))
    }
}

/// Funções do Rustinho dentro do SQLite: `normalizar(texto)` (sem acento,
/// minúsculas), `rustinho_id()` (UUID v7), `rustinho_agora()` (UTC) e
/// `rustinho_conexao()` (o número desta conexão).
/// Precisam ser registradas em toda conexão (os gatilhos da sincronização
/// usam as duas últimas).
pub fn registrar_funcoes(conn: &Connection) -> Resultado<()> {
    use rusqlite::functions::FunctionFlags;
    let deterministica = FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC;
    conn.create_scalar_function("normalizar", 1, deterministica, |ctx| {
        let t: Option<String> = ctx.get(0)?;
        Ok(t.map(|t| crate::texto::normalizar(&t)).unwrap_or_default())
    })?;
    conn.create_scalar_function("rustinho_id", 0, FunctionFlags::SQLITE_UTF8, |_| Ok(tempo::novo_id()))?;
    // PEND-39 (7.5): um número desta conexão, nunca repetido enquanto o
    // programa roda. A busca guarda a contagem de peças por conexão e a refaz
    // só quando o banco muda (`busca::total_guardado`).
    static PROXIMA_CONEXAO: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
    let conexao = PROXIMA_CONEXAO.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    conn.create_scalar_function("rustinho_conexao", 0, deterministica, move |_| Ok(conexao))?;
    conn.create_scalar_function("rustinho_agora", 0, FunctionFlags::SQLITE_UTF8, |_| Ok(tempo::agora()))?;
    // Operação comercial (0.9.15): todas as alterações gravadas na mesma
    // transação recebem o mesmo id (uma venda com itens, pagamentos e
    // movimentos). O id nasce no primeiro uso e acaba no COMMIT ou ROLLBACK;
    // uma gravação fora de transação é uma operação sozinha.
    let operacao: std::sync::Arc<std::sync::Mutex<Option<String>>> = Default::default();
    let atual = operacao.clone();
    conn.create_scalar_function("rustinho_operacao", 0, FunctionFlags::SQLITE_UTF8, move |_| {
        let mut o = atual.lock().unwrap_or_else(|e| e.into_inner());
        Ok(o.get_or_insert_with(tempo::novo_id).clone())
    })?;
    let ao_confirmar = operacao.clone();
    conn.commit_hook(Some(move || {
        *ao_confirmar.lock().unwrap_or_else(|e| e.into_inner()) = None;
        false
    }))?;
    let ao_desfazer = operacao;
    conn.rollback_hook(Some(move || {
        *ao_desfazer.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }))?;
    // História de uma alteração nova: a base e o que ela já conhecia (R-08).
    conn.create_scalar_function("rustinho_historia", 3, deterministica, |ctx| {
        let base: Option<String> = ctx.get(0)?;
        let historia: Option<String> = ctx.get(1)?;
        let base_da_base: Option<String> = ctx.get(2)?;
        Ok(crate::sync::historia_nova(base.as_deref(), historia.as_deref(), base_da_base.as_deref()))
    })?;
    Ok(())
}

/// Aplica as versões da estrutura que ainda não foram aplicadas.
pub(crate) fn migrar(conn: &mut Connection) -> Resultado<()> {
    let versao: usize = conn.query_row("PRAGMA user_version", [], |l| l.get::<_, i64>(0))? as usize;
    if versao > MIGRACOES.len() {
        return Err(Erro::Interno(format!(
            "este banco de dados é de uma versão mais nova do Rustinho (estrutura {versao})"
        )));
    }
    for (i, sql) in MIGRACOES.iter().enumerate().skip(versao) {
        aplicar_migracao(conn, i, sql)?;
    }
    Ok(())
}

/// Roda uma migração numa transação. A que recria tabelas (primeira linha
/// `SEM_CHAVES_ESTRANGEIRAS`) roda com `foreign_keys = OFF` e
/// `legacy_alter_table = ON`: com as chaves ligadas, o DROP TABLE da tabela
/// velha faria um DELETE implícito e o ON DELETE CASCADE apagaria os filhos.
/// `foreign_keys` só muda fora de transação, então é desligado antes e
/// religado depois, em qualquer caminho (deu certo, deu erro ou pânico).
/// Antes do COMMIT confere que nenhuma tabela perdeu ou ganhou registros e
/// que nenhuma referência nova ficou sem par.
fn aplicar_migracao(conn: &mut Connection, i: usize, sql: &str) -> Resultado<()> {
    if !sql.starts_with(SEM_CHAVES_ESTRANGEIRAS) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
        return Ok(());
    }
    let chaves: i64 = conn.query_row("PRAGMA foreign_keys", [], |l| l.get(0))?;
    let legado: i64 = conn.query_row("PRAGMA legacy_alter_table", [], |l| l.get(0))?;
    conn.pragma_update(None, "foreign_keys", false)?;
    conn.pragma_update(None, "legacy_alter_table", true)?;
    let desligado: i64 = conn.query_row("PRAGMA foreign_keys", [], |l| l.get(0))?;
    let r = if desligado != 0 {
        Ok(Err(Erro::Interno("não deu para desligar as chaves estrangeiras para a migração".into())))
    } else {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| migracao_sem_chaves(conn, i, sql)))
    };
    // Volta como estava, aconteça o que acontecer.
    if !conn.is_autocommit() {
        let _ = conn.execute_batch("ROLLBACK");
    }
    let voltou = conn.pragma_update(None, "legacy_alter_table", legado != 0).and_then(|_| {
        conn.pragma_update(None, "foreign_keys", chaves != 0)?;
        conn.query_row("PRAGMA foreign_keys", [], |l| l.get::<_, i64>(0))
    });
    let r = match r {
        Ok(r) => r,
        Err(panico) => std::panic::resume_unwind(panico),
    };
    match voltou {
        Ok(v) if v == chaves => r,
        Ok(_) | Err(_) => {
            Err(Erro::Interno("as chaves estrangeiras não voltaram a ficar como estavam depois da migração".into()))
        }
    }
}

fn migracao_sem_chaves(conn: &mut Connection, i: usize, sql: &str) -> Resultado<()> {
    let tx = conn.transaction()?;
    let contagens_antes = crate::migracao::contagens(&tx)?;
    let referencias_antes = crate::migracao::referencias(&tx)?;
    tx.execute_batch(sql)?;
    let depois: std::collections::HashMap<String, i64> = crate::migracao::contagens(&tx)?.into_iter().collect();
    for (tabela, n) in &contagens_antes {
        // Tabela recriada com outro nome (a "_nova") não conta; a de nome
        // igual tem de ter os mesmos registros.
        if let Some(m) = depois.get(tabela) {
            if m != n {
                return Err(Erro::Interno(format!(
                    "migração {}: a tabela {tabela} tinha {n} registros e ficaria com {m}; nada foi mudado",
                    i + 1
                )));
            }
        } else if *n > 0 {
            return Err(Erro::Interno(format!(
                "migração {}: a tabela {tabela} ({n} registros) sumiria; nada foi mudado",
                i + 1
            )));
        }
    }
    let referencias_depois = crate::migracao::referencias(&tx)?;
    if referencias_depois != referencias_antes {
        return Err(Erro::Interno(format!(
            "migração {}: mudariam as referências que não batem ({referencias_antes:?} → {referencias_depois:?}); nada foi mudado",
            i + 1
        )));
    }
    tx.pragma_update(None, "user_version", (i + 1) as i64)?;
    #[cfg(any(test, feature = "ancoras-de-teste"))]
    if std::env::var("RUSTINHO_TESTE_MIGRACAO_CAIR").is_ok_and(|v| v == "dentro_da_transacao") {
        std::process::abort();
    }
    tx.commit()?;
    Ok(())
}

/// Confere a estrutura que as migrações recentes deviam deixar (pela
/// definição real no banco, não só por contagens). Usada na conferência
/// depois de atualizar um banco antigo.
pub(crate) fn conferir_estrutura(conn: &Connection) -> Resultado<()> {
    let v: i64 = conn.query_row("PRAGMA user_version", [], |l| l.get(0))?;
    if v < 23 {
        return Ok(());
    }
    let sql: String =
        conn.query_row("SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'ordem_servico'", [], |l| {
            l.get(0)
        })?;
    if !sql.contains("'rascunho'") {
        return Err(Erro::Interno("ordem_servico não aceita a situação rascunho".into()));
    }
    let ligacoes: Vec<(String, String, String, String)> = conn
        .prepare("SELECT \"table\", \"from\", \"to\", on_delete FROM pragma_foreign_key_list('ordem_servico_item')")?
        .query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)))?
        .collect::<Result<_, _>>()?;
    if !ligacoes.iter().any(|(t, de, para, ao_apagar)| {
        t == "ordem_servico" && de == "ordem_id" && para == "id" && ao_apagar == "CASCADE"
    }) {
        return Err(Erro::Interno(format!("ordem_servico_item perdeu a ligação com ordem_servico: {ligacoes:?}")));
    }
    for indice in ["ordem_servico_situacao", "ordem_servico_placa", "ordem_servico_venda", "ordem_servico_item_ordem"] {
        let tem: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'index' AND name = ?1",
            [indice],
            |l| l.get(0),
        )?;
        if !tem {
            return Err(Erro::Interno(format!("o índice {indice} sumiu")));
        }
    }
    let tem_nova: bool =
        conn.query_row("SELECT COUNT(*) > 0 FROM sqlite_master WHERE name = 'ordem_servico_nova'", [], |l| l.get(0))?;
    if tem_nova {
        return Err(Erro::Interno("sobrou a tabela ordem_servico_nova".into()));
    }
    let operacao: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('alteracao') WHERE name = 'operacao_id'",
        [],
        |l| l.get(0),
    )?;
    let pendencia: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'sync_pendencia'",
        [],
        |l| l.get(0),
    )?;
    if !operacao || !pendencia {
        return Err(Erro::Interno("faltam operacao_id ou sync_pendencia".into()));
    }
    if v >= 24 {
        for tabela in ["produto_gtin", "xml_pendencia_identificacao"] {
            let tem: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [tabela],
                |l| l.get(0),
            )?;
            if !tem {
                return Err(Erro::Interno(format!("falta a tabela {tabela}")));
            }
        }
    }
    // 0025 (protocolo 4, etapa 2): o registro de caixas aprovados e retirados.
    if v >= 25 {
        for tabela in ["caixa_aprovado", "caixa_retirado"] {
            let tem: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [tabela],
                |l| l.get(0),
            )?;
            if !tem {
                return Err(Erro::Interno(format!("falta a tabela {tabela}")));
            }
        }
    }
    // 0026 (passo 7.5): os índices de desempenho e o nome sem acento gravado.
    if v >= 26 {
        let tem_coluna: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('produto') WHERE name = 'nome_normalizado'",
            [],
            |l| l.get(0),
        )?;
        if !tem_coluna {
            return Err(Erro::Interno("falta a coluna produto.nome_normalizado".into()));
        }
        for indice in ["estoque_movimento_produto_tipo_data", "produto_alteradas_por_ultimo"] {
            let tem: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'index' AND name = ?1",
                [indice],
                |l| l.get(0),
            )?;
            if !tem {
                return Err(Erro::Interno(format!("falta o índice {indice}")));
            }
        }
    }
    Ok(())
}

/// Só nos testes: leva a estrutura até a versão `ate` (para simular um
/// banco de uma versão antiga do Rustinho).
#[cfg(test)]
pub(crate) fn migrar_ate(conn: &mut Connection, ate: usize) -> Resultado<()> {
    for (i, sql) in MIGRACOES.iter().enumerate().take(ate) {
        aplicar_migracao(conn, i, sql)?;
    }
    Ok(())
}

/// Lê (ou cria na primeira vez) o identificador deste computador.
fn garantir_dispositivo(conn: &Connection) -> Resultado<String> {
    let atual: Option<String> =
        conn.query_row("SELECT valor FROM config WHERE chave = 'dispositivo_id'", [], |l| l.get(0)).optional()?;
    if let Some(id) = atual {
        return Ok(id);
    }
    let id = tempo::novo_id();
    conn.execute("INSERT INTO config (chave, valor) VALUES ('dispositivo_id', ?1)", [&id])?;
    Ok(id)
}

/// Onde o banco fica por padrão:
/// - Linux: `~/.local/share/rustinho/rustinho.db`
/// - Windows: `%LOCALAPPDATA%\Rustinho\rustinho.db`
///
/// A variável de ambiente `RUSTINHO_DADOS` troca a pasta (útil para testes;
/// o app de Android usa para apontar a pasta do aplicativo).
///
/// No Rustinho portátil (veja `portatil`), sempre `dados/` ao lado do
/// executável: nem a pasta de sempre nem `RUSTINHO_DADOS` valem.
pub fn caminho_padrao() -> PathBuf {
    if let Some(raiz) = crate::portatil::raiz() {
        return crate::portatil::caminho_do_banco(raiz);
    }
    let pasta = if let Some(p) = std::env::var_os("RUSTINHO_DADOS").filter(|p| !p.is_empty()) {
        PathBuf::from(p)
    } else if let Some(p) = std::env::var_os("LOCALAPPDATA").filter(|p| cfg!(windows) && !p.is_empty()) {
        PathBuf::from(p).join("Rustinho")
    } else if let Some(p) = std::env::var_os("XDG_DATA_HOME").filter(|p| !p.is_empty()) {
        PathBuf::from(p).join("rustinho")
    } else if let Some(casa) = std::env::var_os("HOME") {
        PathBuf::from(casa).join(".local/share/rustinho")
    } else {
        PathBuf::from("dados")
    };
    pasta.join("rustinho.db")
}

#[cfg(test)]
mod testes {
    use super::*;

    fn arquivo_n01(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("rustinho-n01-{nome}-{}", tempo::novo_id()));
        std::fs::create_dir_all(&p).unwrap();
        p.join("rustinho.db")
    }

    /// N01: nenhuma estrutura desta linha (0001 a 00NN, como as versões
    /// publicadas deixaram) é recusada pela conferência; e ela é rápida.
    #[test]
    fn n01_todas_as_estruturas_desta_linha_passam() {
        let inicio = std::time::Instant::now();
        for k in 1..=MIGRACOES.len() {
            let db = arquivo_n01(&format!("e{k}"));
            {
                let mut c = Connection::open(&db).unwrap();
                c.pragma_update(None, "foreign_keys", true).unwrap();
                registrar_funcoes(&c).unwrap();
                for (i, sql) in MIGRACOES.iter().enumerate().take(k) {
                    aplicar_migracao(&mut c, i, sql).unwrap();
                }
            }
            conferir_linhagem(&db).unwrap_or_else(|e| panic!("a estrutura {k} desta linha foi recusada: {e}"));
        }
        let novo = arquivo_n01("novo");
        conferir_linhagem(&novo).unwrap();
        drop(Connection::open(&novo).unwrap());
        conferir_linhagem(&novo).expect("arquivo vazio (banco novo) passa");
        eprintln!("N01: {} estruturas conferidas em {:?}", MIGRACOES.len(), inicio.elapsed());
    }

    /// N01: número maior que o desta versão, com tudo o que ela conhece igual,
    /// é de um Rustinho mais novo desta linha (a mensagem continua essa).
    #[test]
    fn n01_banco_de_um_rustinho_mais_novo_desta_linha() {
        let db = arquivo_n01("mais-novo");
        {
            let mut c = Connection::open(&db).unwrap();
            registrar_funcoes(&c).unwrap();
            for (i, sql) in MIGRACOES.iter().enumerate() {
                aplicar_migracao(&mut c, i, sql).unwrap();
            }
            c.execute_batch("CREATE TABLE tabela_de_uma_versao_futura (id TEXT PRIMARY KEY)").unwrap();
            c.pragma_update(None, "user_version", (MIGRACOES.len() + 1) as i64).unwrap();
        }
        let e = conferir_linhagem(&db).unwrap_err().to_string();
        assert!(e.contains("mais nova"), "{e}");
    }

    #[test]
    fn cria_estrutura_e_dispositivo() {
        let banco = Banco::em_memoria().unwrap();
        let versao: i64 = banco.conn.query_row("PRAGMA user_version", [], |l| l.get(0)).unwrap();
        assert_eq!(versao as usize, MIGRACOES.len());
        assert_eq!(banco.dispositivo.len(), 36);
        let tabelas: i64 = banco.conn.query_row("SELECT COUNT(*) FROM tabela_preco", [], |l| l.get(0)).unwrap();
        assert_eq!(tabelas, 5);
    }

    /// Banco da 0.9.1 (estrutura 9, com a senha do backup em claro e o log
    /// sem "base") abre na 0.9.11 sem perder nada: o primeiro usuário vira
    /// gerente, a senha do backup sai do banco, a troca de dados continua e
    /// reabrir não muda nada.
    #[test]
    fn banco_da_091_abre_sem_perder_nada() {
        let pasta = std::env::temp_dir().join(format!("rustinho-migracao-{}", tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let caminho = pasta.join("rustinho.db");
        {
            let mut c = Connection::open(&caminho).unwrap();
            registrar_funcoes(&c).unwrap();
            for (i, sql) in MIGRACOES.iter().take(9).enumerate() {
                let tx = c.transaction().unwrap();
                tx.execute_batch(sql).unwrap();
                tx.pragma_update(None, "user_version", (i + 1) as i64).unwrap();
                tx.commit().unwrap();
            }
            let hash = crate::usuarios::argon2_hash_para_teste("1234");
            c.execute_batch(&format!(
                "INSERT INTO config (chave, valor) VALUES ('dispositivo_id', 'caixa-antigo'), ('loja_nome', 'Loja Antiga'),
                        ('backup_senha', 'segredo-antigo'), ('sync_inicio', 'x');
                 INSERT INTO usuario (id, nome, senha_hash, ativo, criado_em, atualizado_em, dispositivo)
                 VALUES ('u1', 'Dona', '{hash}', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'caixa-antigo'),
                        ('u2', 'Caixa', '{hash}', 1, '2026-02-01T00:00:00Z', '2026-02-01T00:00:00Z', 'caixa-antigo');
                 INSERT INTO produto (id, codigo, nome, custo, criado_em, atualizado_em, dispositivo)
                 VALUES ('p1', '1', 'Pastilha', 4000, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'caixa-antigo');
                 INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando)
                 VALUES ('01990000-0000-7000-8000-000000000001', 'produto', '[\"p1\"]', 'gravar', '{{}}', 'caixa-antigo', 'x');"
            ))
            .unwrap();
        }
        let b = Banco::abrir(&caminho).unwrap();
        let versao: i64 = b.conn.query_row("PRAGMA user_version", [], |l| l.get(0)).unwrap();
        assert_eq!(versao as usize, MIGRACOES.len());
        assert_eq!(b.dispositivo, "caixa-antigo");
        assert_eq!(crate::config::nome_loja(&b.conn).unwrap(), "Loja Antiga");
        assert!(crate::usuarios::buscar(&b.conn, "u1").unwrap().gerente, "o primeiro vira gerente");
        assert!(!crate::usuarios::buscar(&b.conn, "u2").unwrap().gerente);
        assert!(crate::usuarios::conferir_senha(&b.conn, "u1", "1234").is_ok(), "a senha continua valendo");
        assert!(crate::config::ler(&b.conn, "backup_senha").unwrap().is_none(), "a senha do backup saiu do banco");
        assert_eq!(crate::produtos::ficha(&b.conn, "p1").unwrap().nome, "Pastilha");
        let antigas: i64 =
            b.conn.query_row("SELECT COUNT(*) FROM alteracao WHERE base IS NULL", [], |l| l.get(0)).unwrap();
        assert!(antigas >= 1, "o log antigo continua (sem base)");
        // Mexer depois anota a base.
        b.conn.execute("UPDATE produto SET nome = 'Pastilha nova' WHERE id = 'p1'", []).unwrap();
        let base: Option<String> = b
            .conn
            .query_row(
                "SELECT base FROM alteracao WHERE tabela = 'produto' ORDER BY ordem DESC, id DESC LIMIT 1",
                [],
                |l| l.get(0),
            )
            .unwrap();
        assert!(base.is_some());
        assert_eq!(crate::sync::pendencias(&b.conn).unwrap(), 0);
        drop(b);
        // Reabrir: nada muda.
        let b = Banco::abrir(&caminho).unwrap();
        let usuarios: i64 = b.conn.query_row("SELECT COUNT(*) FROM usuario", [], |l| l.get(0)).unwrap();
        assert_eq!(usuarios, 2);
        drop(b);
        std::fs::remove_dir_all(pasta).ok();
    }

    #[test]
    fn reabrir_mantem_dispositivo() {
        let pasta = std::env::temp_dir().join(format!("rustinho-teste-{}", tempo::novo_id()));
        let caminho = pasta.join("rustinho.db");
        let id = Banco::abrir(&caminho).unwrap().dispositivo;
        let de_novo = Banco::abrir(&caminho).unwrap().dispositivo;
        assert_eq!(id, de_novo);
        std::fs::remove_dir_all(pasta).ok();
    }

    /// R5-T11 · R5-1: a migração 16 → 17 preserva os pares e as referências
    /// que já existiam (inclusive as que a regra antiga chamou de
    /// inconsistentes por tempo); a próxima conferência as reavalia só por prova.
    #[test]
    fn r5_t11_migracao_17_preserva_pares_e_referencias() {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", true).unwrap();
        crate::banco::registrar_funcoes(&c).unwrap();
        migrar_ate(&mut c, 16).unwrap();
        c.execute(
            "INSERT INTO sync_par (dispositivo, nome, endereco, cursor) VALUES ('d1', 'Caixa 2', '10.0.0.2:47810', 7)",
            [],
        )
        .unwrap();
        c.execute(
            "INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde)
             VALUES ('produto_aplicacao', '[\"ap1\"]', 'produto', '[\"x\"]', 'inconsistente', '2020-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        migrar(&mut c).unwrap();
        let (cursor, completo): (i64, Option<String>) = c
            .query_row("SELECT cursor, completo_em FROM sync_par WHERE dispositivo = 'd1'", [], |l| {
                Ok((l.get(0)?, l.get(1)?))
            })
            .unwrap();
        assert_eq!((cursor, completo), (7, None));
        let (estado, motivo): (String, String) =
            c.query_row("SELECT estado, motivo FROM sync_referencia", [], |l| Ok((l.get(0)?, l.get(1)?))).unwrap();
        assert_eq!(
            (estado.as_str(), motivo.as_str()),
            ("esperando", ""),
            "sem prova, deixa de bloquear já na migração"
        );
        let versao: i64 = c.query_row("PRAGMA user_version", [], |l| l.get(0)).unwrap();
        assert_eq!(versao as usize, MIGRACOES.len());
    }

    /// T-24 · CA-F3: a migração 18 → 19 preserva os pares e os endereços deles
    /// (que viram o primeiro caminho de cada um).
    #[test]
    fn t24_migracao_19_preserva_pares_e_enderecos() {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", true).unwrap();
        crate::banco::registrar_funcoes(&c).unwrap();
        migrar_ate(&mut c, 18).unwrap();
        c.execute(
            "INSERT INTO sync_par (dispositivo, nome, endereco, cursor, marca) VALUES ('d1', 'Caixa 2', '10.0.0.2:47810', 7, 'm1')",
            [],
        )
        .unwrap();
        c.execute("INSERT INTO sync_par (dispositivo, nome, endereco, cursor) VALUES ('d2', 'Caixa 3', '', 3)", [])
            .unwrap();
        migrar(&mut c).unwrap();
        let (cursor, marca): (i64, String) = c
            .query_row("SELECT cursor, marca FROM sync_par WHERE dispositivo = 'd1'", [], |l| {
                Ok((l.get(0)?, l.get(1)?))
            })
            .unwrap();
        assert_eq!((cursor, marca.as_str()), (7, "m1"), "contador e marca intactos");
        let linhas: Vec<(String, String, i64)> = c
            .prepare("SELECT endereco, dispositivo, digitado FROM sync_endereco")
            .unwrap()
            .query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            linhas,
            vec![("10.0.0.2:47810".to_string(), "d1".to_string(), 0)],
            "par sem endereço não vira caminho"
        );
        let versao: i64 = c.query_row("PRAGMA user_version", [], |l| l.get(0)).unwrap();
        assert_eq!(versao as usize, MIGRACOES.len());
        assert!(MIGRACOES.len() >= 19);
    }
    /// A-27 (PEND-09): a migração 19 → 20 só marca as compras de antes
    /// (`nao_verificada` / `anterior`, D5): nada é apagado nem bloqueado.
    #[test]
    fn a27_migracao_20_marca_as_compras_antigas_sem_bloquear() {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", true).unwrap();
        crate::banco::registrar_funcoes(&c).unwrap();
        migrar_ate(&mut c, 19).unwrap();
        for (id, situacao) in [("c1", "rascunho"), ("c2", "lancada")] {
            c.execute(
                "INSERT INTO compra (id, chave, numero, situacao, xml, criado_em, dispositivo)
                 VALUES (?1, ?1, '1', ?2, '<nfeProc/>', '2026-01-01T00:00:00Z', 'd')",
                [id, situacao],
            )
            .unwrap();
        }
        migrar(&mut c).unwrap();
        let linhas: Vec<(String, String, String, String)> = c
            .prepare("SELECT id, situacao, xml_verificacao, autorizacao FROM compra ORDER BY id")
            .unwrap()
            .query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            linhas,
            vec![
                ("c1".into(), "rascunho".into(), "nao_verificada".into(), "anterior".into()),
                ("c2".into(), "lancada".into(), "nao_verificada".into(), "anterior".into()),
            ]
        );
        assert!(MIGRACOES.len() >= 20);
    }

    /// DRE gerencial: a migração 20 → 21 sugere a classificação das
    /// categorias de fábrica, deixa as criadas pela loja sem classificação e
    /// não inventa competência para as contas que já existiam.
    #[test]
    fn migracao_21_dre_nao_inventa_nada() {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", true).unwrap();
        crate::banco::registrar_funcoes(&c).unwrap();
        migrar_ate(&mut c, 20).unwrap();
        c.execute(
            "INSERT INTO categoria (id, nome, tipo, ordem, atualizado_em) VALUES ('cat-loja', 'Brindes', 'despesa', 99, 'x')",
            [],
        )
        .unwrap();
        c.execute(
            "INSERT INTO conta_pagar (id, categoria_id, descricao, valor, vencimento, criado_em, dispositivo)
             VALUES ('p1', 'cat-aluguel', 'Aluguel', 1000, '2026-10-05', '2026-10-01T00:00:00Z', 'd')",
            [],
        )
        .unwrap();
        migrar(&mut c).unwrap();
        let g = |id: &str| -> Option<String> {
            c.query_row("SELECT dre_grupo FROM categoria WHERE id = ?1", [id], |l| l.get(0)).unwrap()
        };
        assert_eq!(g("cat-aluguel").as_deref(), Some("fixa"));
        assert_eq!(g("cat-fornecedores").as_deref(), Some("estoque"));
        assert_eq!(g("cat-loja"), None, "categoria da loja: sem classificação");
        let comp: Option<String> =
            c.query_row("SELECT competencia FROM conta_pagar WHERE id = 'p1'", [], |l| l.get(0)).unwrap();
        assert_eq!(comp, None);
        let metas: i64 = c.query_row("SELECT COUNT(*) FROM meta", [], |l| l.get(0)).unwrap();
        assert_eq!(metas, 0);
        assert!(MIGRACOES.len() >= 21);
    }

    /// D-14: a migração 21 → 22 só acrescenta colunas (com valor de fábrica):
    /// vendas, recebimentos, devoluções e movimentos que já existiam ficam
    /// iguais e continuam contando no caixa deles como antes.
    #[test]
    fn d14_migracao_22_nao_muda_o_que_existia() {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", true).unwrap();
        crate::banco::registrar_funcoes(&c).unwrap();
        migrar_ate(&mut c, 21).unwrap();
        c.execute_batch(
            "INSERT INTO caixa_sessao (id, dispositivo, caixa_nome, aberto_em, valor_inicial, fechado_em, contado, esperado)
             VALUES ('s1', 'd', 'Caixa 1', '2026-10-01T10:00:00Z', 5000, '2026-10-01T20:00:00Z', 15000, 15000);
             INSERT INTO caixa_movimento (id, sessao_id, tipo, valor, motivo, criado_em)
             VALUES ('m1', 's1', 'sangria', 1000, 'banco', '2026-10-01T12:00:00Z');",
        )
        .unwrap();
        migrar(&mut c).unwrap();
        let (tipo, id): (String, Option<String>) = c
            .query_row("SELECT origem_tipo, origem_id FROM caixa_movimento WHERE id = 'm1'", [], |l| {
                Ok((l.get(0)?, l.get(1)?))
            })
            .unwrap();
        assert_eq!((tipo.as_str(), id), ("", None));
        let esperado: i64 = c.query_row("SELECT esperado FROM caixa_sessao WHERE id = 's1'", [], |l| l.get(0)).unwrap();
        assert_eq!(esperado, 15000);
        for (t, col) in [
            ("venda", "cancelado_apos_fechar"),
            ("recebimento", "estornado_apos_fechar"),
            ("devolucao", "desfeita_apos_fechar"),
        ] {
            let n: i64 = c
                .query_row(&format!("SELECT COUNT(*) FROM pragma_table_info('{t}') WHERE name = '{col}'"), [], |l| {
                    l.get(0)
                })
                .unwrap();
            assert_eq!(n, 1, "{t}.{col}");
        }
        // A 0.9.15 acrescentou a 0023 e a 0.9.18 acrescentou a 0024; a
        // dev.3.3, a 0025 (registro de caixas do protocolo 4, etapa 2) e a 0026
        // (passo 7.5: índices e busca). Números provisórios (MIG-1 (a)).
        assert_eq!(MIGRACOES.len(), 26);
    }

    /// Protocolo 4, etapa 2: a migração 24 → 25 só cria o registro de caixas
    /// aprovados e retirados, vazio; nenhuma tabela que existia ganha ou perde
    /// registros. E um banco na estrutura 24 (dev.3.1 a dev.3.3) abre pelo
    /// caminho de sempre (cópia, conferência, atualização) sem cair no modo
    /// seguro. (Na frente-p4, bbeac46, era a migração 23 → 24.)
    #[test]
    fn p4_migracao_25_cria_o_registro_de_caixas_sem_mexer_no_resto() {
        let semear = |c: &Connection| {
            c.execute_batch(
                "INSERT INTO config (chave, valor) VALUES ('dispositivo_id', 'caixa-dev32'),
                        ('loja_nome', 'Loja dev.3.2'), ('loja_id', 'loja-dev32');
                 INSERT INTO sync_par (dispositivo, nome, endereco, cursor)
                 VALUES ('d2', 'Caixa 2', '10.0.0.2:47810', 5);",
            )
            .unwrap();
        };
        let novas = ["caixa_aprovado", "caixa_retirado"];
        // A migração sozinha (em memória).
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", true).unwrap();
        registrar_funcoes(&c).unwrap();
        migrar_ate(&mut c, 24).unwrap();
        semear(&c);
        let antes = crate::migracao::contagens(&c).unwrap();
        assert!(antes.iter().all(|(t, _)| !novas.contains(&t.as_str())), "as tabelas ainda não existem na 24");
        migrar(&mut c).unwrap();
        let depois: std::collections::HashMap<String, i64> =
            crate::migracao::contagens(&c).unwrap().into_iter().collect();
        for (t, n) in &antes {
            assert_eq!(depois.get(t), Some(n), "{t}");
        }
        for t in novas {
            assert_eq!(depois.get(t), Some(&0), "{t} criada vazia");
        }
        let colunas = |t: &str| -> Vec<String> {
            c.prepare(&format!("SELECT name FROM pragma_table_info('{t}') ORDER BY cid"))
                .unwrap()
                .query_map([], |l| l.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert_eq!(
            colunas("caixa_aprovado"),
            ["chave", "aprovador", "chave_troca", "loja_id", "nome", "gerente", "aprovado_em", "assinatura"]
        );
        assert_eq!(
            colunas("caixa_retirado"),
            ["chave", "retirador", "loja_id", "motivo", "gerente", "retirado_em", "assinatura"]
        );
        conferir_estrutura(&c).unwrap();
        assert!(MIGRACOES.len() >= 25);
        // Um arquivo na estrutura 24 de verdade, aberto pelo caminho de sempre.
        let pasta = std::env::temp_dir().join(format!("rustinho-migracao-25-{}", tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let caminho = pasta.join("rustinho.db");
        {
            let mut c = Connection::open(&caminho).unwrap();
            registrar_funcoes(&c).unwrap();
            migrar_ate(&mut c, 24).unwrap();
            semear(&c);
        }
        let b = Banco::abrir(&caminho).unwrap();
        assert!(!b.em_modo_seguro());
        let versao: i64 = b.conn.query_row("PRAGMA user_version", [], |l| l.get(0)).unwrap();
        assert_eq!(versao as usize, MIGRACOES.len());
        assert_eq!(b.dispositivo, "caixa-dev32");
        assert_eq!(crate::loja::id(&b.conn).unwrap(), "loja-dev32");
        let cursor: i64 =
            b.conn.query_row("SELECT cursor FROM sync_par WHERE dispositivo = 'd2'", [], |l| l.get(0)).unwrap();
        assert_eq!(cursor, 5);
        for t in novas {
            let n: i64 = b.conn.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |l| l.get(0)).unwrap();
            assert_eq!(n, 0, "{t}");
        }
        drop(b);
        std::fs::remove_dir_all(pasta).ok();
    }

    /// Protocolo 4, etapa 2 (checklist das migrações, item 6: restaurar um
    /// backup antigo): um backup feito na estrutura 24 (dev.3.1 a dev.3.3)
    /// restaura num Rustinho com a 0025. O banco do backup volta inteiro, é
    /// atualizado na hora (o registro de caixas nasce vazio) e abre de novo
    /// sem cair no modo seguro.
    #[test]
    fn p4_backup_da_estrutura_24_restaura_e_atualiza_para_a_25() {
        let _vez = crate::backup::testes::um_por_vez();
        let pasta = std::env::temp_dir().join(format!("rustinho-p4-backup24-{}", tempo::novo_id()));
        let antigo = pasta.join("antigo").join("rustinho.db");
        let atual = pasta.join("atual").join("rustinho.db");
        std::fs::create_dir_all(antigo.parent().unwrap()).unwrap();
        {
            let mut c = Connection::open(&antigo).unwrap();
            registrar_funcoes(&c).unwrap();
            migrar_ate(&mut c, 24).unwrap();
            c.execute_batch(
                "INSERT INTO config (chave, valor) VALUES ('dispositivo_id', 'caixa-do-backup'),
                        ('loja_nome', 'Loja do backup 24'), ('loja_id', 'loja-24');
                 INSERT INTO sync_par (dispositivo, nome, endereco, cursor)
                 VALUES ('d2', 'Caixa 2', '10.0.0.2:47810', 7);",
            )
            .unwrap();
        }
        let feito = crate::backup::fazer(&antigo, false).unwrap();
        let mut b = Banco::abrir(&atual).unwrap();
        let info = crate::backup::restaurar(&mut b.conn, Some(&atual), &feito.arquivo, false, "").unwrap();
        assert_eq!(info.estrutura, 24, "o backup é da estrutura 24");
        let versao: i64 = b.conn.query_row("PRAGMA user_version", [], |l| l.get(0)).unwrap();
        assert_eq!(versao as usize, MIGRACOES.len(), "atualizado na restauração");
        conferir_estrutura(&b.conn).unwrap();
        assert_eq!(crate::config::nome_loja(&b.conn).unwrap(), "Loja do backup 24");
        for t in ["caixa_aprovado", "caixa_retirado"] {
            let n: i64 = b.conn.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |l| l.get(0)).unwrap();
            assert_eq!(n, 0, "{t} nasce vazia");
        }
        drop(b);
        let b = Banco::abrir(&atual).unwrap();
        assert!(!b.em_modo_seguro(), "{:?}", b.defeito);
        assert_eq!(crate::config::nome_loja(&b.conn).unwrap(), "Loja do backup 24");
        drop(b);
        std::fs::remove_dir_all(pasta).ok();
    }

    // ------------------------------------------------------------------
    // Passo 7.5: a migração 0026 (índices e busca normalizada). Na preparação
    // ela era a 0025; na integração a 0025 é a do protocolo 4, e os testes
    // valem a partir da 25 (a estrutura de antes) e da 24 (a das lojas hoje).
    // ------------------------------------------------------------------

    /// Uma loja numa estrutura anterior à 0026 (`estrutura`: a 25, de antes
    /// desta migração, ou a 24, a das lojas de hoje, dev.3.1 a dev.3.3): loja,
    /// caixa, a gerente Ana (senha 1234) com a chave do backup, a troca de dados
    /// ligada (os gatilhos e o log), e peças com acento e maiúsculas, com estoque.
    fn loja_antiga(pasta: &Path, estrutura: usize) -> PathBuf {
        let db = pasta.join("rustinho.db");
        let mut c = Connection::open(&db).unwrap();
        c.pragma_update(None, "journal_mode", "WAL").unwrap();
        c.pragma_update(None, "foreign_keys", true).unwrap();
        registrar_funcoes(&c).unwrap();
        migrar_ate(&mut c, estrutura).unwrap();
        crate::config::gravar(&c, "dispositivo_id", "caixa-v24").unwrap();
        crate::config::gravar(&c, "loja_nome", "Loja v24").unwrap();
        crate::config::gravar(&c, "caixa_nome", "Caixa 1").unwrap();
        crate::sync::criar_gatilhos(&c).unwrap();
        crate::config::gravar(&c, "sync_inicio", "2026-11-01T00:00:00Z").unwrap();
        let u = crate::usuarios::criar(&c, "caixa-v24", "Ana", "1234", true).unwrap();
        crate::usuarios::definir_gerente(&c, &u.id, true).unwrap();
        crate::backup::gerente_conferido(&c, &u.id, "1234", None).unwrap();
        for (i, nome) in ["Óleo 5W30 sintético", "ÁGUA destilada", "pastilha FRAS-LE", "Correia 8PK 1515", "Ácido"]
            .iter()
            .enumerate()
        {
            c.execute(
                "INSERT INTO produto (id, codigo, nome, busca, codigos, custo, criado_em, atualizado_em, dispositivo)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1000, '2026-10-01T10:00:00Z', '2026-10-01T10:00:00Z', 'caixa-v24')",
                rusqlite::params![
                    format!("p{i}"),
                    format!("{}", 100 + i),
                    nome,
                    crate::texto::normalizar(nome),
                    format!(" {} ", 100 + i)
                ],
            )
            .unwrap();
            c.execute(
                "INSERT INTO estoque_movimento (id, produto_id, quantidade, tipo, dispositivo, criado_em)
                 VALUES (?1, ?2, 5000, 'inicial', 'caixa-v24', '2026-10-01T10:00:00Z')",
                [format!("m{i}"), format!("p{i}")],
            )
            .unwrap();
        }
        c.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(())).unwrap();
        db
    }

    /// As tabelas da loja que a 0026 não pode mudar, com as colunas que elas
    /// têm antes dela: tabela → linhas (em ordem de rowid, como texto).
    const TABELAS_DA_LOJA: &[&str] =
        &["produto", "produto_preco", "estoque_movimento", "produto_gtin", "usuario", "pessoa", "alteracao"];

    fn conteudo(c: &Connection, colunas: &BTreeMap<String, Vec<String>>) -> BTreeMap<String, Vec<String>> {
        colunas
            .iter()
            .map(|(t, cols)| {
                let lista = cols.iter().map(|x| format!("quote(\"{x}\")")).collect::<Vec<_>>().join(" || '|' || ");
                let linhas: Vec<String> = c
                    .prepare(&format!("SELECT {lista} FROM \"{t}\" ORDER BY rowid"))
                    .unwrap()
                    .query_map([], |l| l.get(0))
                    .unwrap()
                    .collect::<Result<_, _>>()
                    .unwrap();
                (t.clone(), linhas)
            })
            .collect()
    }

    fn colunas_de(c: &Connection) -> BTreeMap<String, Vec<String>> {
        TABELAS_DA_LOJA
            .iter()
            .map(|t| {
                let cols: Vec<String> = c
                    .prepare(&format!("SELECT name FROM pragma_table_info('{t}') ORDER BY cid"))
                    .unwrap()
                    .query_map([], |l| l.get(0))
                    .unwrap()
                    .collect::<Result<_, _>>()
                    .unwrap();
                (t.to_string(), cols)
            })
            .collect()
    }

    fn pasta_75(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("rustinho-m26-{nome}-{}", tempo::novo_id()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    /// Passo 7.5: a migração 0026 não muda nada do que existia. Um banco na
    /// estrutura 25 (e um na 24, a das lojas de hoje, que passa pela 0025 do
    /// protocolo 4 e pela 0026) abre pelo caminho de sempre (cópia conferida e
    /// atualização), sem modo seguro; as tabelas da loja ficam com os mesmos
    /// registros e os mesmos valores nas colunas que já existiam; e o log da
    /// troca de dados não ganha nada (o que a 0026 grava não é alteração da
    /// loja e não vai para os outros caixas).
    #[test]
    fn m26_nao_muda_o_que_existia() {
        for estrutura in [25, 24] {
            let pasta = pasta_75(&format!("igual{estrutura}"));
            let db = loja_antiga(&pasta, estrutura);
            let (colunas, antes) = {
                let c = Connection::open(&db).unwrap();
                let colunas = colunas_de(&c);
                let antes = conteudo(&c, &colunas);
                (colunas, antes)
            };
            assert!(antes["alteracao"].len() > 5, "a loja de teste tem log da troca");
            let b = Banco::abrir(&db).unwrap();
            assert!(!b.em_modo_seguro(), "{:?}", b.defeito);
            let v: i64 = b.conn.query_row("PRAGMA user_version", [], |l| l.get(0)).unwrap();
            assert_eq!(v, 26, "da estrutura {estrutura}");
            conferir_estrutura(&b.conn).unwrap();
            let depois = conteudo(&b.conn, &colunas);
            for t in TABELAS_DA_LOJA {
                assert_eq!(antes[*t], depois[*t], "a tabela {t} mudou com a 0026 (da estrutura {estrutura})");
            }
            nome_normalizado_preenchido(&b.conn);
            drop(b);
            let _ = std::fs::remove_dir_all(&pasta);
        }
    }

    /// B04: a 0026 preenche o nome sem acento de todas as peças que já
    /// existiam com o mesmo `normalizar` do programa.
    fn nome_normalizado_preenchido(c: &Connection) {
        let pecas: Vec<(String, String)> = c
            .prepare("SELECT nome, nome_normalizado FROM produto ORDER BY rowid")
            .unwrap()
            .query_map([], |l| Ok((l.get(0)?, l.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(pecas.len(), 5);
        for (nome, normalizado) in pecas {
            assert_eq!(normalizado, crate::texto::normalizar(&nome), "{nome}");
        }
    }

    /// Repete enquanto outro teste estiver fazendo backup (a vez é uma só no
    /// programa inteiro, como na tela).
    fn na_vez<T>(mut f: impl FnMut() -> Resultado<T>) -> T {
        for _ in 0..1200 {
            match f() {
                Err(e) if e.to_string().contains("em andamento") => {
                    std::thread::sleep(std::time::Duration::from_millis(50))
                }
                r => return r.unwrap(),
            }
        }
        panic!("o backup ficou ocupado por um minuto")
    }

    /// Passo 7.5: um backup feito numa estrutura anterior à 0026 (a 25 e a 24)
    /// restaura num Rustinho com a 0026: o banco do backup volta inteiro e é
    /// atualizado para a 26 pelo caminho da restauração, sem gravar nada no
    /// log da troca de dados.
    #[test]
    fn m26_backup_de_estrutura_anterior_restaura_e_atualiza() {
        let _vez = crate::backup::testes::um_por_vez();
        for estrutura in [25, 24] {
            let pasta = pasta_75(&format!("backup{estrutura}"));
            let db = loja_antiga(&pasta, estrutura);
            let (colunas, antes) = {
                let c = Connection::open(&db).unwrap();
                let colunas = colunas_de(&c);
                (colunas.clone(), conteudo(&c, &colunas))
            };
            let feito = na_vez(|| crate::backup::fazer(&db, false));
            let mut b = Banco::abrir(&db).unwrap();
            // Depois do backup e da atualização, a loja muda.
            b.conn.execute("UPDATE produto SET nome = 'Outro nome' WHERE id = 'p0'", []).unwrap();
            b.conn.execute("DELETE FROM estoque_movimento WHERE id = 'm1'", []).unwrap();
            let info = na_vez(|| crate::backup::restaurar(&mut b.conn, Some(&db), &feito.arquivo, false, "1234"));
            assert_eq!(info.estrutura as usize, estrutura, "o backup é da estrutura {estrutura}");
            let v: i64 = b.conn.query_row("PRAGMA user_version", [], |l| l.get(0)).unwrap();
            assert_eq!(v, 26);
            conferir_estrutura(&b.conn).unwrap();
            let depois = conteudo(&b.conn, &colunas);
            for t in ["produto", "produto_preco", "estoque_movimento", "produto_gtin", "pessoa"] {
                assert_eq!(antes[t], depois[t], "a tabela {t} não voltou igual ao backup da estrutura {estrutura}");
            }
            let do_log = |c: &[String]| {
                c.iter().filter(|l| l.contains("'produto'") || l.contains("'estoque_movimento'")).count()
            };
            assert_eq!(do_log(&antes["alteracao"]), do_log(&depois["alteracao"]), "o log da troca ganhou registros");
            nome_normalizado_preenchido(&b.conn);
            drop(b);
            let _ = std::fs::remove_dir_all(&pasta);
        }
    }
}
