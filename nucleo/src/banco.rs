//! Abertura do banco de dados SQLite e atualização da sua estrutura.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};

use crate::erro::{Erro, Resultado};
use crate::tempo;

/// Cada arquivo SQL é uma versão da estrutura do banco. Novas versões entram
/// no fim da lista; as antigas nunca mudam.
const MIGRACOES: &[&str] = &[
    include_str!("migracoes/0001_inicial.sql"),
    include_str!("migracoes/0002_pessoas_compras_fiscal.sql"),
    include_str!("migracoes/0003_vendas_caixa_sync.sql"),
    include_str!("migracoes/0004_crediario.sql"),
    include_str!("migracoes/0005_financeiro.sql"),
    include_str!("migracoes/0006_encomendas.sql"),
    include_str!("migracoes/0007_devolucoes.sql"),
    include_str!("migracoes/0008_oficina.sql"),
    include_str!("migracoes/0009_cadastro.sql"),
    include_str!("migracoes/0010_gerente.sql"),
    include_str!("migracoes/0011_backup_conflitos.sql"),
    include_str!("migracoes/0012_historia_das_alteracoes.sql"),
    include_str!("migracoes/0013_faixas_de_codigo.sql"),
    include_str!("migracoes/0014_codigo_confirmado.sql"),
    include_str!("migracoes/0015_codigo_das_pessoas.sql"),
    include_str!("migracoes/0016_codigo_das_pessoas_confirmado.sql"),
    include_str!("migracoes/0017_referencias_com_prova.sql"),
    include_str!("migracoes/0018_situacao_da_senha.sql"),
    include_str!("migracoes/0019_caminhos_da_rede.sql"),
    include_str!("migracoes/0020_verificacao_da_nfe.sql"),
];

/// Número da estrutura mais nova que este Rustinho conhece.
pub fn versao_da_estrutura() -> usize {
    MIGRACOES.len()
}

/// Conexão com o banco mais a identidade deste computador.
pub struct Banco {
    pub conn: Connection,
    /// Identificador único deste computador (caixa), criado na primeira vez.
    pub dispositivo: String,
    /// Onde o arquivo está (vazio para banco em memória, usado nos testes).
    pub caminho: Option<PathBuf>,
}

impl Banco {
    /// Abre (ou cria) o banco no arquivo indicado.
    pub fn abrir(caminho: &Path) -> Resultado<Banco> {
        if let Some(pasta) = caminho.parent() {
            std::fs::create_dir_all(pasta)
                .map_err(|e| Erro::Interno(format!("não foi possível criar a pasta {}: {e}", pasta.display())))?;
        }
        let conn = Connection::open(caminho)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::preparar(conn, Some(caminho.to_path_buf()), crate::computador::atual())
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
        registrar_funcoes(&conn)?;
        migrar(&mut conn)?;
        let dispositivo = garantir_dispositivo(&conn)?;
        crate::sync::criar_gatilhos(&conn)?;
        crate::sync::registrar_existentes(&conn)?;
        crate::backup::limpar_chave_antiga(&conn)?;
        // Códigos das peças: arquivo ao lado do banco, distribuidor e
        // peças com código a definir (veja `faixas`).
        crate::faixas::preparar(&mut conn, computador.as_deref())?;
        Ok(Banco { conn, dispositivo, caminho })
    }
}

/// Funções do Rustinho dentro do SQLite: `normalizar(texto)` (sem acento,
/// minúsculas), `rustinho_id()` (UUID v7) e `rustinho_agora()` (UTC).
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
    conn.create_scalar_function("rustinho_agora", 0, FunctionFlags::SQLITE_UTF8, |_| Ok(tempo::agora()))?;
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
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

/// Só nos testes: leva a estrutura até a versão `ate` (para simular um
/// banco de uma versão antiga do Rustinho).
#[cfg(test)]
pub(crate) fn migrar_ate(conn: &mut Connection, ate: usize) -> Resultado<()> {
    for (i, sql) in MIGRACOES.iter().enumerate().take(ate) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
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
pub fn caminho_padrao() -> PathBuf {
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
        assert_eq!(MIGRACOES.len(), 20);
    }
}
