//! Usuários: cadastro, senhas e entrada.
//!
//! Há dois papéis: gerente e caixa. O gerente faz tudo; o caixa não faz o
//! que a loja deixou só para o gerente (veja `api::ACOES_DO_GERENTE`), a não
//! ser que um gerente digite a senha na hora. As senhas são guardadas só como
//! "hash" Argon2 (não dá para descobrir a senha olhando o banco).

use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::Argon2;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::erro::{Erro, Resultado};
use crate::{tempo, texto};

const SENHA_MINIMA: usize = 4;

/// Argon2id com os parâmetros escritos aqui (os mesmos de sempre, o padrão
/// da biblioteca: 19 MiB de memória, 2 passadas, 1 linha). Escritos para
/// que uma atualização da biblioteca não mude a chave da rede nem a dos
/// backups. Num celular de entrada leva perto de 1 s; o resultado fica
/// guardado na memória enquanto o programa está aberto.
pub fn argon2() -> Argon2<'static> {
    Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(19 * 1024, 2, 1, None).unwrap_or_default(),
    )
}

/// Um usuário, sem a senha.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usuario {
    pub id: String,
    pub nome: String,
    pub ativo: bool,
    pub criado_em: String,
    pub ultimo_acesso: Option<String>,
    /// Comissão padrão em % ("" = sem comissão).
    pub comissao: String,
    pub gerente: bool,
}

fn de_linha(l: &rusqlite::Row) -> rusqlite::Result<Usuario> {
    Ok(Usuario {
        id: l.get(0)?,
        nome: l.get(1)?,
        ativo: l.get(2)?,
        criado_em: l.get(3)?,
        ultimo_acesso: l.get(4)?,
        comissao: l.get::<_, Option<i64>>(5)?.map(crate::numeros::escrever_percentual).unwrap_or_default(),
        gerente: l.get(6)?,
    })
}

const COLUNAS: &str = "id, nome, ativo, criado_em, ultimo_acesso, comissao, gerente";

/// Lista os usuários em ordem alfabética.
pub fn listar(conn: &Connection, apenas_ativos: bool) -> Resultado<Vec<Usuario>> {
    let sql = format!(
        "SELECT {COLUNAS} FROM usuario {} ORDER BY nome COLLATE NOCASE",
        if apenas_ativos { "WHERE ativo = 1" } else { "" }
    );
    let mut consulta = conn.prepare(&sql)?;
    let linhas = consulta.query_map([], de_linha)?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

/// Busca um usuário pelo id.
pub fn buscar(conn: &Connection, id: &str) -> Resultado<Usuario> {
    conn.query_row(&format!("SELECT {COLUNAS} FROM usuario WHERE id = ?1"), [id], de_linha)
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Usuário".into()))
}

/// Quantos usuários existem (0 = primeira vez que o Rustinho abre).
pub fn quantidade(conn: &Connection) -> Resultado<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM usuario", [], |l| l.get(0))?)
}

/// Cria um usuário.
pub fn criar(conn: &Connection, dispositivo: &str, nome: &str, senha: &str, ativo: bool) -> Resultado<Usuario> {
    let nome = validar_nome(conn, nome, None)?;
    validar_senha(senha)?;
    let id = tempo::novo_id();
    let agora = tempo::agora();
    conn.execute(
        "INSERT INTO usuario (id, nome, senha_hash, ativo, criado_em, atualizado_em, dispositivo)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6)",
        params![id, nome, gerar_hash(senha)?, ativo, agora, dispositivo],
    )?;
    buscar(conn, &id)
}

/// Altera nome, situação (ativo) e, se informada, a senha.
pub fn alterar(conn: &Connection, id: &str, nome: &str, ativo: bool, nova_senha: Option<&str>) -> Resultado<Usuario> {
    let atual = buscar(conn, id)?;
    let nome = validar_nome(conn, nome, Some(id))?;
    if atual.ativo && !ativo {
        let outros_ativos: i64 =
            conn.query_row("SELECT COUNT(*) FROM usuario WHERE ativo = 1 AND id <> ?1", [id], |l| l.get(0))?;
        if outros_ativos == 0 {
            return Err(Erro::campo("ativo", "Este é o único usuário ativo. Crie outro antes de desativar este."));
        }
        if atual.gerente && gerentes_ativos_alem(conn, id)? == 0 {
            return Err(Erro::campo(
                "ativo",
                "Este é o único gerente. Faça outro usuário gerente antes de desativar este.",
            ));
        }
    }
    let agora = tempo::agora();
    conn.execute(
        "UPDATE usuario SET nome = ?2, ativo = ?3, atualizado_em = ?4 WHERE id = ?1",
        params![id, nome, ativo, agora],
    )?;
    if let Some(senha) = nova_senha {
        validar_senha(senha)?;
        conn.execute("UPDATE usuario SET senha_hash = ?2 WHERE id = ?1", params![id, gerar_hash(senha)?])?;
    }
    buscar(conn, id)
}

/// Troca a comissão padrão do vendedor. Devolve (antes, depois) se mudou.
pub fn definir_comissao(conn: &Connection, id: &str, texto_pct: &str) -> Resultado<Option<(String, String)>> {
    let novo = crate::numeros::ler_percentual(texto_pct).map_err(|m| Erro::campo("comissao", m))?;
    if let Some(p) = novo {
        if !(0..=10000).contains(&p) {
            return Err(Erro::campo("comissao", "Use de 0 a 100%."));
        }
    }
    let antes: Option<i64> = conn.query_row("SELECT comissao FROM usuario WHERE id = ?1", [id], |l| l.get(0))?;
    if antes == novo {
        return Ok(None);
    }
    conn.execute(
        "UPDATE usuario SET comissao = ?2, atualizado_em = ?3 WHERE id = ?1",
        params![id, novo, tempo::agora()],
    )?;
    let t = |v: Option<i64>| {
        v.map(|p| format!("{}%", crate::numeros::escrever_percentual(p))).unwrap_or_else(|| "—".into())
    };
    Ok(Some((t(antes), t(novo))))
}

/// Confere a senha de um usuário. Devolve o usuário se estiver certa.
pub fn conferir_senha(conn: &Connection, id: &str, senha: &str) -> Resultado<Usuario> {
    let hash: Option<String> =
        conn.query_row("SELECT senha_hash FROM usuario WHERE id = ?1", [id], |l| l.get(0)).optional()?;
    let hash = hash.ok_or_else(|| Erro::NaoEncontrado("Usuário".into()))?;
    let usuario = buscar(conn, id)?;
    if !usuario.ativo {
        return Err(Erro::campo("senha", "Este usuário está desativado."));
    }
    let analisado = PasswordHash::new(&hash).map_err(|e| Erro::Interno(format!("senha guardada inválida: {e}")))?;
    if argon2().verify_password(senha.as_bytes(), &analisado).is_err() {
        return Err(Erro::campo("senha", "Senha errada."));
    }
    Ok(usuario)
}

/// Anota a hora do último acesso.
pub fn registrar_acesso(conn: &Connection, id: &str) -> Resultado<()> {
    conn.execute("UPDATE usuario SET ultimo_acesso = ?2 WHERE id = ?1", params![id, tempo::agora()])?;
    Ok(())
}

/// Quantos gerentes ativos existem além deste usuário.
fn gerentes_ativos_alem(conn: &Connection, id: &str) -> Resultado<i64> {
    Ok(conn
        .query_row("SELECT COUNT(*) FROM usuario WHERE ativo = 1 AND gerente = 1 AND id <> ?1", [id], |l| l.get(0))?)
}

/// Faz (ou deixa de fazer) o usuário gerente. Sempre sobra pelo menos um.
/// Devolve true se mudou.
pub fn definir_gerente(conn: &Connection, id: &str, gerente: bool) -> Resultado<bool> {
    let atual = buscar(conn, id)?;
    if atual.gerente == gerente {
        return Ok(false);
    }
    if !gerente && gerentes_ativos_alem(conn, id)? == 0 {
        return Err(Erro::campo("gerente", "A loja precisa de pelo menos um gerente."));
    }
    conn.execute(
        "UPDATE usuario SET gerente = ?2, atualizado_em = ?3 WHERE id = ?1",
        params![id, gerente, tempo::agora()],
    )?;
    Ok(true)
}

/// Senha de algum gerente ativo (o "aperto": o gerente digita a senha na
/// hora, sem trocar de usuário). Devolve o gerente que confere.
pub fn conferir_gerente(conn: &Connection, senha: &str) -> Resultado<Option<Usuario>> {
    let mut c = conn.prepare("SELECT id, senha_hash FROM usuario WHERE ativo = 1 AND gerente = 1")?;
    let gerentes: Vec<(String, String)> = c.query_map([], |l| Ok((l.get(0)?, l.get(1)?)))?.collect::<Result<_, _>>()?;
    for (id, hash) in gerentes {
        if let Ok(analisado) = PasswordHash::new(&hash) {
            if argon2().verify_password(senha.as_bytes(), &analisado).is_ok() {
                return Ok(Some(buscar(conn, &id)?));
            }
        }
    }
    Ok(None)
}

/// Depois de juntar os dados dos caixas, dois usuários podem ter ficado com o
/// mesmo nome (criados ao mesmo tempo em caixas diferentes). O mais novo
/// ganha um número no fim: "Maria (2)". Todos os caixas escolhem o mesmo
/// usuário e o mesmo nome, então o resultado é igual em todos.
pub fn resolver_nomes_repetidos(conn: &Connection) -> Resultado<usize> {
    let mut c = conn.prepare("SELECT id, nome FROM usuario ORDER BY criado_em, id")?;
    let todos: Vec<(String, String)> = c.query_map([], |l| Ok((l.get(0)?, l.get(1)?)))?.collect::<Result<_, _>>()?;
    let mut usados: Vec<String> = Vec::new();
    let mut mudados = 0;
    for (id, nome) in todos {
        let mut final_ = nome.clone();
        let mut n = 2;
        while usados.contains(&texto::normalizar(&final_)) {
            let base: String = nome.chars().take(34).collect();
            final_ = format!("{} ({n})", base.trim_end());
            n += 1;
        }
        usados.push(texto::normalizar(&final_));
        if final_ != nome {
            conn.execute(
                "UPDATE usuario SET nome = ?2, atualizado_em = ?3 WHERE id = ?1",
                params![id, final_, tempo::agora()],
            )?;
            mudados += 1;
        }
    }
    Ok(mudados)
}

fn segundos_agora() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Quantas senhas erradas seguidas e há quantos segundos foi a última.
pub fn falhas(conn: &Connection, id: &str) -> Resultado<Option<(u32, i64)>> {
    let r: Option<(i64, i64)> = conn
        .query_row("SELECT quantidade, ultima FROM entrada_falha WHERE usuario_id = ?1", [id], |l| {
            Ok((l.get(0)?, l.get(1)?))
        })
        .optional()?;
    Ok(r.map(|(q, u)| (q.clamp(0, u32::MAX as i64) as u32, (segundos_agora() - u).max(0))))
}

/// Anota mais uma senha errada. Depois de `espera` segundos, recomeça do 1.
pub fn anotar_falha(conn: &Connection, id: &str, espera: i64) -> Resultado<()> {
    let agora = segundos_agora();
    conn.execute(
        "INSERT INTO entrada_falha (usuario_id, quantidade, ultima) VALUES (?1, 1, ?2)
         ON CONFLICT(usuario_id) DO UPDATE SET
             quantidade = CASE WHEN ?2 - ultima >= ?3 OR ?2 < ultima THEN 1 ELSE quantidade + 1 END,
             ultima = ?2",
        params![id, agora, espera],
    )?;
    Ok(())
}

pub fn limpar_falhas(conn: &Connection, id: &str) -> Resultado<()> {
    conn.execute("DELETE FROM entrada_falha WHERE usuario_id = ?1", [id])?;
    Ok(())
}

fn validar_nome(conn: &Connection, nome: &str, id_atual: Option<&str>) -> Resultado<String> {
    let nome = texto::limpar(nome);
    if texto::tamanho(&nome) < 2 {
        return Err(Erro::campo("nome", "Digite o nome (pelo menos 2 letras)."));
    }
    if texto::tamanho(&nome) > 40 {
        return Err(Erro::campo("nome", "Use no máximo 40 letras no nome."));
    }
    let normalizado = texto::normalizar(&nome);
    for outro in listar(conn, false)? {
        if Some(outro.id.as_str()) != id_atual && texto::normalizar(&outro.nome) == normalizado {
            return Err(Erro::campo("nome", "Já existe um usuário com esse nome."));
        }
    }
    Ok(nome)
}

fn validar_senha(senha: &str) -> Resultado<()> {
    if senha.chars().count() < SENHA_MINIMA {
        return Err(Erro::campo("senha", format!("A senha precisa ter pelo menos {SENHA_MINIMA} caracteres.")));
    }
    if senha.len() > 128 {
        return Err(Erro::campo("senha", "Senha comprida demais."));
    }
    Ok(())
}

fn gerar_hash(senha: &str) -> Resultado<String> {
    argon2()
        .hash_password(senha.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| Erro::Interno(format!("não foi possível proteger a senha: {e}")))
}

#[cfg(test)]
mod testes {
    use super::*;

    /// F-02: os parâmetros escritos dão a mesma chave de antes (a rede e os
    /// backups de versões anteriores continuam abrindo).
    #[test]
    fn argon2_igual_ao_de_antes() {
        let (mut a, mut b) = ([0u8; 32], [0u8; 32]);
        argon2().hash_password_into(b"senha-da-loja", b"sal-de-16-bytes!", &mut a).unwrap();
        Argon2::default().hash_password_into(b"senha-da-loja", b"sal-de-16-bytes!", &mut b).unwrap();
        assert_eq!(a, b);
        let p = argon2().params().clone();
        assert_eq!((p.m_cost(), p.t_cost(), p.p_cost()), (19 * 1024, 2, 1));
    }
    use crate::banco::Banco;

    #[test]
    fn criar_entrar_e_trocar_senha() {
        let b = Banco::em_memoria().unwrap();
        let u = criar(&b.conn, &b.dispositivo, "  Maria   Souza ", "1234", true).unwrap();
        assert_eq!(u.nome, "Maria Souza");
        assert!(conferir_senha(&b.conn, &u.id, "1234").is_ok());
        let errada = conferir_senha(&b.conn, &u.id, "9999").unwrap_err();
        assert_eq!(errada.nome_do_campo(), Some("senha"));

        alterar(&b.conn, &u.id, "Maria", true, Some("abcd")).unwrap();
        assert!(conferir_senha(&b.conn, &u.id, "1234").is_err());
        assert!(conferir_senha(&b.conn, &u.id, "abcd").is_ok());
    }

    #[test]
    fn nomes_unicos_sem_ligar_para_acento() {
        let b = Banco::em_memoria().unwrap();
        criar(&b.conn, &b.dispositivo, "José", "1234", true).unwrap();
        let e = criar(&b.conn, &b.dispositivo, "jose", "1234", true).unwrap_err();
        assert_eq!(e.nome_do_campo(), Some("nome"));
    }

    #[test]
    fn nao_desativa_o_ultimo_ativo() {
        let b = Banco::em_memoria().unwrap();
        let u = criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        assert!(alterar(&b.conn, &u.id, "Maria", false, None).is_err());
        let outro = criar(&b.conn, &b.dispositivo, "Luigi", "1234", true).unwrap();
        assert!(alterar(&b.conn, &u.id, "Maria", false, None).is_ok());
        assert!(conferir_senha(&b.conn, &u.id, "1234").is_err());
        assert!(conferir_senha(&b.conn, &outro.id, "1234").is_ok());
    }

    #[test]
    fn primeiro_usuario_e_gerente_e_sempre_sobra_um() {
        let b = Banco::em_memoria().unwrap();
        let g = criar(&b.conn, &b.dispositivo, "Renan", "1234", true).unwrap();
        definir_gerente(&b.conn, &g.id, true).unwrap();
        let c = criar(&b.conn, &b.dispositivo, "Luigi", "5678", true).unwrap();
        assert!(!c.gerente);
        assert!(definir_gerente(&b.conn, &g.id, false).is_err());
        assert!(alterar(&b.conn, &g.id, "Renan", false, None).is_err());
        assert_eq!(conferir_gerente(&b.conn, "1234").unwrap().unwrap().id, g.id);
        assert!(conferir_gerente(&b.conn, "5678").unwrap().is_none());
        definir_gerente(&b.conn, &c.id, true).unwrap();
        assert!(definir_gerente(&b.conn, &g.id, false).unwrap());
    }

    #[test]
    fn nomes_repetidos_depois_de_juntar() {
        let b = Banco::em_memoria().unwrap();
        let a = criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        // Como se tivesse chegado de outro caixa, sem passar pela conferência.
        b.conn
            .execute(
                "INSERT INTO usuario (id, nome, senha_hash, ativo, criado_em, atualizado_em, dispositivo)
                 SELECT 'zzz', 'MARIA', senha_hash, 1, '2999-01-01', '2999-01-01', 'outro' FROM usuario WHERE id = ?1",
                [&a.id],
            )
            .unwrap();
        assert_eq!(resolver_nomes_repetidos(&b.conn).unwrap(), 1);
        assert_eq!(buscar(&b.conn, &a.id).unwrap().nome, "Maria");
        assert_eq!(buscar(&b.conn, "zzz").unwrap().nome, "MARIA (2)");
        assert_eq!(resolver_nomes_repetidos(&b.conn).unwrap(), 0);
    }

    #[test]
    fn senhas_erradas_ficam_guardadas() {
        let b = Banco::em_memoria().unwrap();
        let u = criar(&b.conn, &b.dispositivo, "Ana", "1234", true).unwrap();
        for _ in 0..3 {
            anotar_falha(&b.conn, &u.id, 30).unwrap();
        }
        assert_eq!(falhas(&b.conn, &u.id).unwrap().unwrap().0, 3);
        limpar_falhas(&b.conn, &u.id).unwrap();
        assert!(falhas(&b.conn, &u.id).unwrap().is_none());
    }

    #[test]
    fn senha_curta() {
        let b = Banco::em_memoria().unwrap();
        assert!(criar(&b.conn, &b.dispositivo, "Ana", "12", true).is_err());
    }
}
