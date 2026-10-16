//! Usuários: cadastro, senhas e entrada.
//!
//! Todos os usuários têm acesso a todas as funções. As senhas são guardadas
//! só como "hash" Argon2 (não dá para descobrir a senha olhando o banco).

use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::Argon2;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::erro::{Erro, Resultado};
use crate::{tempo, texto};

const SENHA_MINIMA: usize = 4;

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
}

fn de_linha(l: &rusqlite::Row) -> rusqlite::Result<Usuario> {
    Ok(Usuario {
        id: l.get(0)?,
        nome: l.get(1)?,
        ativo: l.get(2)?,
        criado_em: l.get(3)?,
        ultimo_acesso: l.get(4)?,
        comissao: l.get::<_, Option<i64>>(5)?.map(crate::numeros::escrever_percentual).unwrap_or_default(),
    })
}

const COLUNAS: &str = "id, nome, ativo, criado_em, ultimo_acesso, comissao";

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
        let outros_ativos: i64 = conn.query_row(
            "SELECT COUNT(*) FROM usuario WHERE ativo = 1 AND id <> ?1",
            [id],
            |l| l.get(0),
        )?;
        if outros_ativos == 0 {
            return Err(Erro::campo(
                "ativo",
                "Este é o único usuário ativo. Crie outro antes de desativar este.",
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
        conn.execute(
            "UPDATE usuario SET senha_hash = ?2 WHERE id = ?1",
            params![id, gerar_hash(senha)?],
        )?;
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
    conn.execute("UPDATE usuario SET comissao = ?2, atualizado_em = ?3 WHERE id = ?1", params![id, novo, tempo::agora()])?;
    let t = |v: Option<i64>| v.map(|p| format!("{}%", crate::numeros::escrever_percentual(p))).unwrap_or_else(|| "—".into());
    Ok(Some((t(antes), t(novo))))
}

/// Confere a senha de um usuário. Devolve o usuário se estiver certa.
pub fn conferir_senha(conn: &Connection, id: &str, senha: &str) -> Resultado<Usuario> {
    let hash: Option<String> = conn
        .query_row("SELECT senha_hash FROM usuario WHERE id = ?1", [id], |l| l.get(0))
        .optional()?;
    let hash = hash.ok_or_else(|| Erro::NaoEncontrado("Usuário".into()))?;
    let usuario = buscar(conn, id)?;
    if !usuario.ativo {
        return Err(Erro::campo("senha", "Este usuário está desativado."));
    }
    let analisado = PasswordHash::new(&hash).map_err(|e| Erro::Interno(format!("senha guardada inválida: {e}")))?;
    if Argon2::default().verify_password(senha.as_bytes(), &analisado).is_err() {
        return Err(Erro::campo("senha", "Senha errada."));
    }
    Ok(usuario)
}

/// Anota a hora do último acesso.
pub fn registrar_acesso(conn: &Connection, id: &str) -> Resultado<()> {
    conn.execute(
        "UPDATE usuario SET ultimo_acesso = ?2 WHERE id = ?1",
        params![id, tempo::agora()],
    )?;
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
        return Err(Erro::campo(
            "senha",
            format!("A senha precisa ter pelo menos {SENHA_MINIMA} caracteres."),
        ));
    }
    if senha.len() > 128 {
        return Err(Erro::campo("senha", "Senha comprida demais."));
    }
    Ok(())
}

fn gerar_hash(senha: &str) -> Resultado<String> {
    Argon2::default()
        .hash_password(senha.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| Erro::Interno(format!("não foi possível proteger a senha: {e}")))
}

#[cfg(test)]
mod testes {
    use super::*;
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
    fn senha_curta() {
        let b = Banco::em_memoria().unwrap();
        assert!(criar(&b.conn, &b.dispositivo, "Ana", "12", true).is_err());
    }
}
