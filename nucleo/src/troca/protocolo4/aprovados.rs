//! Registro assinado dos caixas aprovados e retirados — protocolo 4, etapa 2
//! (D1, E5).
//!
//! Duas tabelas (migração 0025): `caixa_aprovado` e `caixa_retirado`. Cada
//! linha leva a assinatura Ed25519 de quem a fez, sobre todos os campos
//! (veja [`Aprovacao::corpo`] e [`Retirada::corpo`]); nenhuma linha vale só
//! por estar no banco.
//!
//! **Quem está autorizado** ([`autorizada`]): a confiança parte de uma
//! **raiz** fixada por quem chama (a chave do primeiro caixa da loja; onde ela
//! fica fixada é das etapas 5 e 7). Uma chave está autorizada quando:
//! 1. existe uma cadeia de aprovações com assinatura válida da raiz até ela
//!    (a raiz tem de ter o próprio registro, assinado por ela mesma, com a
//!    identidade da loja); e
//! 2. nenhuma chave dessa cadeia (nem ela mesma) foi retirada: **um elo
//!    retirado invalida** o que depende dele, até outro caixa aprovado
//!    aprovar de novo por outro caminho; e
//! 3. **a retirada vence a aprovação**: uma chave retirada continua fora mesmo
//!    que alguém a aprove de novo (o caixa precisa de identidade nova).
//!
//! Uma retirada conta quando a assinatura confere e quem a assinou já foi
//! aprovado de verdade (cadeia válida até a raiz, sem olhar retiradas): na
//! dúvida, o caixa fica fora. A raiz não pode ser retirada por ninguém nesta
//! etapa: isso pararia a loja inteira e depende da recuperação (D2).
//!
//! **Fora da troca de dados do protocolo 3, de propósito.** As tabelas não
//! entram em `sync::TABELAS`: um caixa da 0.9.17 não as conhece, e uma linha
//! aprovada que chegasse pelo protocolo 3 viria pelo mesmo canal que só exige
//! a senha da rede. A replicação delas é ponto de integração das etapas 4
//! (canal autenticado) e 7 (convivência): o registro só passa a valer entre
//! caixas quando viajar pelo canal autenticado. O teste
//! `registro_fica_fora_da_troca_do_protocolo_3` segura isso até lá.
//!
//! Quem chama [`aprovar`] e [`retirar`] confere antes a senha do gerente
//! (`usuarios::conferir_senha` + gerente): o fluxo de entrada com o gerente é
//! da etapa 5. Aqui o gerente vai assinado em cada registro e na auditoria.

use std::collections::{HashMap, HashSet, VecDeque};

use rusqlite::{params, Connection, OptionalExtension};

use super::{de_hex, hex, verificar, Assinatura, Chave, Dominio, Identidade};
use crate::erro::{Erro, Resultado};

/// Tamanho máximo do nome do caixa e do motivo da retirada (caracteres).
const NOME_MAXIMO: usize = 80;
const MOTIVO_MAXIMO: usize = 300;

/// O pedido de entrada de um caixa novo: as duas chaves públicas dele e a
/// prova de que ele tem as partes privadas (assinatura Ed25519 sobre as duas
/// chaves, contexto [`Dominio::Pedido`]). O gerente confere o
/// [`Pedido::codigo`] na tela do caixa novo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pedido {
    pub chave: Chave,
    pub chave_troca: Chave,
    pub assinatura: Assinatura,
}

impl Pedido {
    fn corpo(chave: &Chave, chave_troca: &Chave) -> Vec<u8> {
        let mut v = Vec::with_capacity(64);
        v.extend_from_slice(chave);
        v.extend_from_slice(chave_troca);
        v
    }

    /// O pedido deste caixa.
    pub fn de(id: &Identidade) -> Pedido {
        let (chave, chave_troca) = (id.publica(), id.publica_troca());
        Pedido { chave, chave_troca, assinatura: id.assinar(Dominio::Pedido, &Pedido::corpo(&chave, &chave_troca)) }
    }

    /// A assinatura confere (quem pediu tem a chave privada Ed25519 e ligou a
    /// ela esta chave X25519)?
    pub fn confere(&self) -> bool {
        verificar(Dominio::Pedido, &self.chave, &Pedido::corpo(&self.chave, &self.chave_troca), &self.assinatura)
    }

    /// O código que o gerente confere.
    pub fn codigo(&self) -> String {
        super::codigo_de_pedido(&self.chave, &self.chave_troca)
    }
}

/// Uma linha de `caixa_aprovado`: `aprovador` aprovou `chave` na loja
/// `loja_id`, com o gerente `gerente`, e assinou tudo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Aprovacao {
    pub chave: Chave,
    pub chave_troca: Chave,
    pub loja_id: String,
    pub nome: String,
    pub aprovador: Chave,
    pub gerente: String,
    pub aprovado_em: String,
    pub assinatura: Assinatura,
}

/// Uma linha de `caixa_retirado`: `retirador` retirou `chave` da loja.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Retirada {
    pub chave: Chave,
    pub loja_id: String,
    pub motivo: String,
    pub retirador: Chave,
    pub gerente: String,
    pub retirado_em: String,
    pub assinatura: Assinatura,
}

/// Junta campos sem ambiguidade: cada texto com o tamanho na frente.
fn campo(v: &mut Vec<u8>, texto: &str) {
    v.extend_from_slice(&(texto.len() as u32).to_be_bytes());
    v.extend_from_slice(texto.as_bytes());
}

impl Aprovacao {
    /// Os bytes que o aprovador assina (todos os campos, na ordem).
    pub fn corpo(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(256);
        v.extend_from_slice(&self.chave);
        v.extend_from_slice(&self.chave_troca);
        campo(&mut v, &self.loja_id);
        campo(&mut v, &self.nome);
        v.extend_from_slice(&self.aprovador);
        campo(&mut v, &self.gerente);
        campo(&mut v, &self.aprovado_em);
        v
    }

    /// A assinatura do aprovador confere?
    pub fn assinatura_confere(&self) -> bool {
        verificar(Dominio::Aprovacao, &self.aprovador, &self.corpo(), &self.assinatura)
    }
}

impl Retirada {
    /// Os bytes que o retirador assina.
    pub fn corpo(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(256);
        v.extend_from_slice(&self.chave);
        campo(&mut v, &self.loja_id);
        campo(&mut v, &self.motivo);
        v.extend_from_slice(&self.retirador);
        campo(&mut v, &self.gerente);
        campo(&mut v, &self.retirado_em);
        v
    }

    /// A assinatura do retirador confere?
    pub fn assinatura_confere(&self) -> bool {
        verificar(Dominio::Retirada, &self.retirador, &self.corpo(), &self.assinatura)
    }
}

/// A situação de uma chave na loja.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Situacao {
    /// Cadeia válida até a raiz, sem elo retirado.
    Autorizada,
    /// Retirada da loja (por retirada assinada válida).
    Retirada,
    /// Já foi aprovada, mas a cadeia passa por um caixa retirado.
    SemCadeia,
    /// Nenhuma aprovação válida chega a ela (chave desconhecida, assinatura
    /// adulterada, outra loja, outra raiz...).
    Desconhecida,
}

/// O registro conferido de uma loja (a partir de uma raiz).
#[derive(Debug, Default)]
pub struct Avaliacao {
    /// Aprovados de verdade alguma vez (cadeia válida, sem olhar retiradas).
    aprovados: HashSet<Chave>,
    retirados: HashSet<Chave>,
    autorizados: HashSet<Chave>,
}

impl Avaliacao {
    pub fn situacao(&self, chave: &Chave) -> Situacao {
        if self.retirados.contains(chave) {
            Situacao::Retirada
        } else if self.autorizados.contains(chave) {
            Situacao::Autorizada
        } else if self.aprovados.contains(chave) {
            Situacao::SemCadeia
        } else {
            Situacao::Desconhecida
        }
    }

    /// Quantas chaves estão autorizadas agora.
    pub fn quantas_autorizadas(&self) -> usize {
        self.autorizados.len()
    }
}

/// Uma coluna de texto; `None` se o valor não é texto (linha estragada).
fn texto(l: &rusqlite::Row<'_>, i: usize) -> Option<String> {
    l.get::<_, String>(i).ok()
}

/// As aprovações gravadas desta loja. Linha fora do formato (chave ou
/// assinatura que não é hexadecimal do tamanho certo, coluna que não é
/// texto) não vale e não impede as outras.
fn ler_aprovacoes(conn: &Connection, loja_id: &str) -> Resultado<Vec<Aprovacao>> {
    let mut c = conn.prepare(
        "SELECT chave, chave_troca, loja_id, nome, aprovador, gerente, aprovado_em, assinatura
           FROM caixa_aprovado WHERE loja_id = ?1",
    )?;
    let mut linhas = c.query([loja_id])?;
    let mut lista = Vec::new();
    while let Some(l) = linhas.next()? {
        let ler = || -> Option<Aprovacao> {
            Some(Aprovacao {
                chave: de_hex(&texto(l, 0)?)?,
                chave_troca: de_hex(&texto(l, 1)?)?,
                loja_id: texto(l, 2)?,
                nome: texto(l, 3)?,
                aprovador: de_hex(&texto(l, 4)?)?,
                gerente: texto(l, 5)?,
                aprovado_em: texto(l, 6)?,
                assinatura: de_hex(&texto(l, 7)?)?,
            })
        };
        lista.extend(ler());
    }
    Ok(lista)
}

/// As retiradas gravadas desta loja (mesma regra das aprovações).
fn ler_retiradas(conn: &Connection, loja_id: &str) -> Resultado<Vec<Retirada>> {
    let mut c = conn.prepare(
        "SELECT chave, loja_id, motivo, retirador, gerente, retirado_em, assinatura
           FROM caixa_retirado WHERE loja_id = ?1",
    )?;
    let mut linhas = c.query([loja_id])?;
    let mut lista = Vec::new();
    while let Some(l) = linhas.next()? {
        let ler = || -> Option<Retirada> {
            Some(Retirada {
                chave: de_hex(&texto(l, 0)?)?,
                loja_id: texto(l, 1)?,
                motivo: texto(l, 2)?,
                retirador: de_hex(&texto(l, 3)?)?,
                gerente: texto(l, 4)?,
                retirado_em: texto(l, 5)?,
                assinatura: de_hex(&texto(l, 6)?)?,
            })
        };
        lista.extend(ler());
    }
    Ok(lista)
}

/// Quem a raiz alcança pelas aprovações válidas, sem passar por `fora`.
fn alcancados(raiz: &Chave, arestas: &HashMap<Chave, Vec<Chave>>, fora: &HashSet<Chave>) -> HashSet<Chave> {
    let mut vistos = HashSet::new();
    if fora.contains(raiz) {
        return vistos;
    }
    let mut fila = VecDeque::from([*raiz]);
    vistos.insert(*raiz);
    while let Some(de) = fila.pop_front() {
        for para in arestas.get(&de).into_iter().flatten() {
            if !fora.contains(para) && vistos.insert(*para) {
                fila.push_back(*para);
            }
        }
    }
    vistos
}

/// Confere o registro inteiro da loja `loja_id` a partir da `raiz`.
pub fn avaliar(conn: &Connection, raiz: &Chave, loja_id: &str) -> Resultado<Avaliacao> {
    if loja_id.is_empty() {
        return Ok(Avaliacao::default());
    }
    // 1. Só as aprovações com assinatura válida, desta loja. Aprovação de si
    //    mesmo só vale para a raiz.
    let mut raiz_valida = false;
    let mut arestas: HashMap<Chave, Vec<Chave>> = HashMap::new();
    for a in ler_aprovacoes(conn, loja_id)? {
        if a.loja_id != loja_id || !a.assinatura_confere() {
            continue;
        }
        if a.aprovador == a.chave {
            raiz_valida |= a.chave == *raiz;
            continue;
        }
        arestas.entry(a.aprovador).or_default().push(a.chave);
    }
    if !raiz_valida {
        return Ok(Avaliacao::default());
    }
    // 2. Aprovados alguma vez (sem olhar retiradas).
    let aprovados = alcancados(raiz, &arestas, &HashSet::new());
    // 3. Retiradas válidas: assinadas por quem já foi aprovado; a raiz não sai.
    let retirados: HashSet<Chave> = ler_retiradas(conn, loja_id)?
        .into_iter()
        .filter(|r| {
            r.loja_id == loja_id && r.chave != *raiz && aprovados.contains(&r.retirador) && r.assinatura_confere()
        })
        .map(|r| r.chave)
        .collect();
    // 4. Autorizados: cadeia a partir da raiz sem passar por retirado.
    let autorizados = alcancados(raiz, &arestas, &retirados);
    Ok(Avaliacao { aprovados, retirados, autorizados })
}

/// **Esta chave está autorizada nesta loja agora?** (cadeia válida até a
/// `raiz`, nenhum elo retirado, ela mesma não retirada).
pub fn autorizada(conn: &Connection, raiz: &Chave, loja_id: &str, chave: &Chave) -> Resultado<bool> {
    Ok(avaliar(conn, raiz, loja_id)?.situacao(chave) == Situacao::Autorizada)
}

fn limpar(texto: &str, maximo: usize, nome_do_campo: &str, vazio: &str) -> Resultado<String> {
    let t = crate::texto::limpar(texto);
    if t.is_empty() {
        return Err(Erro::campo(nome_do_campo, vazio));
    }
    if t.chars().count() > maximo {
        return Err(Erro::campo(nome_do_campo, format!("Use no máximo {maximo} caracteres.")));
    }
    Ok(t)
}

fn exigir_gerente(gerente: &str) -> Resultado<()> {
    if gerente.trim().is_empty() {
        return Err(Erro::validacao("A aprovação de caixas é do gerente."));
    }
    Ok(())
}

fn dispositivo(conn: &Connection) -> Resultado<String> {
    Ok(crate::config::ler(conn, "dispositivo_id")?.unwrap_or_default())
}

/// Grava tudo de `f` ou nada: um ponto de salvamento serve tanto dentro da
/// transação de quem chama (como a `api` faz) quanto sozinho.
fn em_bloco<T>(conn: &Connection, f: impl FnOnce() -> Resultado<T>) -> Resultado<T> {
    conn.execute_batch("SAVEPOINT caixas_aprovados")?;
    match f() {
        Ok(v) => {
            conn.execute_batch("RELEASE caixas_aprovados")?;
            Ok(v)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK TO caixas_aprovados; RELEASE caixas_aprovados");
            Err(e)
        }
    }
}

fn gravar_aprovacao(conn: &Connection, a: &Aprovacao) -> Resultado<()> {
    conn.execute(
        "INSERT INTO caixa_aprovado (chave, aprovador, chave_troca, loja_id, nome, gerente, aprovado_em, assinatura)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT (chave, aprovador) DO UPDATE SET
            chave_troca = excluded.chave_troca, loja_id = excluded.loja_id, nome = excluded.nome,
            gerente = excluded.gerente, aprovado_em = excluded.aprovado_em, assinatura = excluded.assinatura",
        params![
            hex(&a.chave),
            hex(&a.aprovador),
            hex(&a.chave_troca),
            a.loja_id,
            a.nome,
            a.gerente,
            a.aprovado_em,
            hex(&a.assinatura)
        ],
    )?;
    Ok(())
}

fn assinar_aprovacao(
    aprovador: &Identidade,
    chave: Chave,
    chave_troca: Chave,
    loja_id: &str,
    nome: &str,
    gerente: &str,
) -> Aprovacao {
    let mut a = Aprovacao {
        chave,
        chave_troca,
        loja_id: loja_id.to_string(),
        nome: nome.to_string(),
        aprovador: aprovador.publica(),
        gerente: gerente.to_string(),
        aprovado_em: crate::tempo::agora(),
        assinatura: [0u8; 64],
    };
    a.assinatura = aprovador.assinar(Dominio::Aprovacao, &a.corpo());
    a
}

/// Loja nova: este caixa vira a **raiz** da cadeia (registro assinado por ele
/// mesmo, com o gerente que criou a loja). Uma loja só tem uma raiz: se já
/// houver outra raiz registrada para esta loja, recusa.
pub fn registrar_raiz(
    conn: &Connection,
    eu: &Identidade,
    loja_id: &str,
    nome: &str,
    gerente: &str,
) -> Resultado<Aprovacao> {
    exigir_gerente(gerente)?;
    if loja_id.trim().is_empty() {
        return Err(Erro::validacao("A loja ainda não tem identidade."));
    }
    let nome = limpar(nome, NOME_MAXIMO, "nome", "Dê um nome a este caixa.")?;
    let outra: Option<String> = conn
        .query_row(
            "SELECT chave FROM caixa_aprovado WHERE loja_id = ?1 AND chave = aprovador AND chave <> ?2 LIMIT 1",
            params![loja_id, hex(&eu.publica())],
            |l| l.get(0),
        )
        .optional()?;
    if outra.is_some() {
        return Err(Erro::validacao("Esta loja já tem um caixa raiz."));
    }
    let a = assinar_aprovacao(eu, eu.publica(), eu.publica_troca(), loja_id, &nome, gerente);
    em_bloco(conn, || {
        gravar_aprovacao(conn, &a)?;
        crate::auditoria::registrar(
            conn,
            &dispositivo(conn)?,
            Some(gerente),
            "aprovar_caixa_raiz",
            "caixa",
            &hex(&a.chave),
            &[format!("Caixa: {nome}"), format!("Código: {}", eu.codigo_de_pedido())],
        )
    })?;
    Ok(a)
}

/// O caixa `aprovador` (autorizado nesta loja), com o gerente `gerente`,
/// aprova o caixa novo do `pedido`. Recusa se: o pedido não prova a posse da
/// chave; o aprovador não está autorizado; a chave do pedido foi retirada (a
/// retirada vence: o caixa precisa de identidade nova).
pub fn aprovar(
    conn: &Connection,
    aprovador: &Identidade,
    raiz: &Chave,
    loja_id: &str,
    pedido: &Pedido,
    nome: &str,
    gerente: &str,
) -> Resultado<Aprovacao> {
    exigir_gerente(gerente)?;
    let nome = limpar(nome, NOME_MAXIMO, "nome", "Dê um nome ao caixa novo.")?;
    if !pedido.confere() {
        return Err(Erro::validacao("O pedido deste caixa não confere. Peça o código de novo no caixa novo."));
    }
    if pedido.chave == aprovador.publica() {
        return Err(Erro::validacao("Este caixa não pode aprovar a si mesmo."));
    }
    let avaliacao = avaliar(conn, raiz, loja_id)?;
    if avaliacao.situacao(&aprovador.publica()) != Situacao::Autorizada {
        return Err(Erro::validacao("Este caixa não está autorizado a aprovar caixas nesta loja."));
    }
    if avaliacao.situacao(&pedido.chave) == Situacao::Retirada {
        return Err(Erro::validacao(
            "Este caixa foi retirado da loja e não volta com a mesma identidade. Gere uma identidade nova nele.",
        ));
    }
    let a = assinar_aprovacao(aprovador, pedido.chave, pedido.chave_troca, loja_id, &nome, gerente);
    em_bloco(conn, || {
        gravar_aprovacao(conn, &a)?;
        crate::auditoria::registrar(
            conn,
            &dispositivo(conn)?,
            Some(gerente),
            "aprovar_caixa",
            "caixa",
            &hex(&a.chave),
            &[
                format!("Caixa: {nome}"),
                format!("Código: {}", pedido.codigo()),
                format!("Aprovado pelo caixa: {}", aprovador.codigo_de_pedido()),
            ],
        )
    })?;
    Ok(a)
}

/// O caixa `retirador` (autorizado nesta loja), com o gerente, retira a
/// `chave` da loja. Vale por máquina: a chave fica fora para sempre, mesmo
/// que alguém a aprove de novo. Não retira a si mesmo nem a raiz.
pub fn retirar(
    conn: &Connection,
    retirador: &Identidade,
    raiz: &Chave,
    loja_id: &str,
    chave: &Chave,
    motivo: &str,
    gerente: &str,
) -> Resultado<Retirada> {
    exigir_gerente(gerente)?;
    let motivo = limpar(motivo, MOTIVO_MAXIMO, "motivo", "Diga por que o caixa está saindo da loja.")?;
    if *chave == retirador.publica() {
        return Err(Erro::validacao("Este computador não pode retirar a si mesmo da loja."));
    }
    if chave == raiz {
        return Err(Erro::validacao(
            "O primeiro caixa da loja (a raiz) não pode ser retirado: isso pararia a loja inteira.",
        ));
    }
    let avaliacao = avaliar(conn, raiz, loja_id)?;
    if avaliacao.situacao(&retirador.publica()) != Situacao::Autorizada {
        return Err(Erro::validacao("Este caixa não está autorizado a retirar caixas nesta loja."));
    }
    match avaliacao.situacao(chave) {
        Situacao::Retirada => return Err(Erro::validacao("Este caixa já foi retirado da loja.")),
        Situacao::Desconhecida => return Err(Erro::NaoEncontrado("Caixa".into())),
        Situacao::Autorizada | Situacao::SemCadeia => {}
    }
    let mut r = Retirada {
        chave: *chave,
        loja_id: loja_id.to_string(),
        motivo: motivo.clone(),
        retirador: retirador.publica(),
        gerente: gerente.to_string(),
        retirado_em: crate::tempo::agora(),
        assinatura: [0u8; 64],
    };
    r.assinatura = retirador.assinar(Dominio::Retirada, &r.corpo());
    em_bloco(conn, || {
        conn.execute(
            "INSERT INTO caixa_retirado (chave, retirador, loja_id, motivo, gerente, retirado_em, assinatura)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT (chave, retirador) DO UPDATE SET
            loja_id = excluded.loja_id, motivo = excluded.motivo, gerente = excluded.gerente,
            retirado_em = excluded.retirado_em, assinatura = excluded.assinatura",
            params![
                hex(&r.chave),
                hex(&r.retirador),
                r.loja_id,
                r.motivo,
                r.gerente,
                r.retirado_em,
                hex(&r.assinatura)
            ],
        )?;
        crate::auditoria::registrar(
            conn,
            &dispositivo(conn)?,
            Some(gerente),
            "retirar_caixa_por_chave",
            "caixa",
            &hex(chave),
            &[format!("Motivo: {motivo}"), format!("Retirado pelo caixa: {}", retirador.codigo_de_pedido())],
        )
    })?;
    Ok(r)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;

    const LOJA: &str = "loja-teste-p4";

    /// Loja com gerente "Ana" e este caixa (`raiz`) como raiz da cadeia.
    fn loja() -> (Banco, String, Identidade) {
        let (b, u) = crate::faixas::para_testes::loja("Caixa 1");
        let raiz = Identidade::gerar().unwrap();
        registrar_raiz(&b.conn, &raiz, LOJA, "Caixa 1", &u).unwrap();
        (b, u, raiz)
    }

    fn situacao(b: &Banco, raiz: &Identidade, chave: &Chave) -> Situacao {
        avaliar(&b.conn, &raiz.publica(), LOJA).unwrap().situacao(chave)
    }

    fn contar(b: &Banco, sql: &str) -> i64 {
        b.conn.query_row(sql, [], |l| l.get(0)).unwrap()
    }

    #[test]
    fn cadeia_valida_ate_a_raiz() {
        let (b, u, raiz) = loja();
        let (c2, c3) = (Identidade::gerar().unwrap(), Identidade::gerar().unwrap());
        let r = raiz.publica();
        assert!(autorizada(&b.conn, &r, LOJA, &r).unwrap(), "a raiz está autorizada");
        // Raiz aprova o Caixa 2; o Caixa 2 aprova o Caixa 3.
        let a2 = aprovar(&b.conn, &raiz, &r, LOJA, &Pedido::de(&c2), "Caixa 2", &u).unwrap();
        assert!(a2.assinatura_confere());
        assert_eq!(a2.aprovador, r);
        aprovar(&b.conn, &c2, &r, LOJA, &Pedido::de(&c3), "Caixa 3", &u).unwrap();
        for c in [&raiz, &c2, &c3] {
            assert!(autorizada(&b.conn, &r, LOJA, &c.publica()).unwrap(), "{c:?}");
        }
        assert_eq!(avaliar(&b.conn, &r, LOJA).unwrap().quantas_autorizadas(), 3);
        // A cadeia só vale a partir da raiz certa e na loja certa.
        assert!(!autorizada(&b.conn, &c2.publica(), LOJA, &c3.publica()).unwrap(), "c2 não é raiz");
        assert!(!autorizada(&b.conn, &r, "outra-loja", &c3.publica()).unwrap(), "outra loja");
        assert!(!autorizada(&b.conn, &r, "", &r).unwrap());
        // Uma segunda raiz para a mesma loja é recusada.
        let intrusa = Identidade::gerar().unwrap();
        assert!(registrar_raiz(&b.conn, &intrusa, LOJA, "Raiz falsa", &u).is_err());
        // Registrar a raiz de novo (mesma chave) não estraga nada.
        registrar_raiz(&b.conn, &raiz, LOJA, "Caixa 1", &u).unwrap();
        assert!(autorizada(&b.conn, &r, LOJA, &c3.publica()).unwrap());
    }

    #[test]
    fn elo_retirado_invalida_e_a_retirada_vence_a_aprovacao() {
        let (b, u, raiz) = loja();
        let r = raiz.publica();
        let (c2, c3) = (Identidade::gerar().unwrap(), Identidade::gerar().unwrap());
        aprovar(&b.conn, &raiz, &r, LOJA, &Pedido::de(&c2), "Caixa 2", &u).unwrap();
        aprovar(&b.conn, &c2, &r, LOJA, &Pedido::de(&c3), "Caixa 3", &u).unwrap();
        // A raiz retira o Caixa 2: ele sai, e o Caixa 3 (aprovado por ele)
        // fica sem cadeia.
        let ret = retirar(&b.conn, &raiz, &r, LOJA, &c2.publica(), "computador roubado", &u).unwrap();
        assert!(ret.assinatura_confere());
        assert_eq!(situacao(&b, &raiz, &c2.publica()), Situacao::Retirada);
        assert_eq!(situacao(&b, &raiz, &c3.publica()), Situacao::SemCadeia);
        assert!(!autorizada(&b.conn, &r, LOJA, &c3.publica()).unwrap(), "elo retirado invalida");
        // O retirado não aprova nem retira mais ninguém.
        let c4 = Identidade::gerar().unwrap();
        assert!(aprovar(&b.conn, &c2, &r, LOJA, &Pedido::de(&c4), "Caixa 4", &u).is_err());
        assert!(retirar(&b.conn, &c2, &r, LOJA, &c3.publica(), "x", &u).is_err());
        // Nem o Caixa 3, que ficou sem cadeia.
        assert!(aprovar(&b.conn, &c3, &r, LOJA, &Pedido::de(&c4), "Caixa 4", &u).is_err());
        // A raiz aprova o Caixa 3 de novo, direto: outra cadeia, ele volta.
        aprovar(&b.conn, &raiz, &r, LOJA, &Pedido::de(&c3), "Caixa 3", &u).unwrap();
        assert!(autorizada(&b.conn, &r, LOJA, &c3.publica()).unwrap());
        // A retirada vence: aprovar o Caixa 2 de novo é recusado...
        assert!(aprovar(&b.conn, &raiz, &r, LOJA, &Pedido::de(&c2), "Caixa 2", &u).is_err());
        // ...e mesmo uma aprovação assinada gravada direto no banco não o traz.
        let forcada = assinar_aprovacao(&raiz, c2.publica(), c2.publica_troca(), LOJA, "Caixa 2", &u);
        gravar_aprovacao(&b.conn, &forcada).unwrap();
        assert_eq!(situacao(&b, &raiz, &c2.publica()), Situacao::Retirada);
        // Trocar o "identificador declarado" (nome, dispositivo) não muda nada:
        // a retirada é da chave.
        b.conn.execute("UPDATE config SET valor = 'outro-id' WHERE chave = 'dispositivo_id'", []).unwrap();
        assert_eq!(situacao(&b, &raiz, &c2.publica()), Situacao::Retirada);
    }

    #[test]
    fn assinatura_adulterada_e_recusada() {
        let (b, u, raiz) = loja();
        let r = raiz.publica();
        let (c2, c3) = (Identidade::gerar().unwrap(), Identidade::gerar().unwrap());
        aprovar(&b.conn, &raiz, &r, LOJA, &Pedido::de(&c2), "Caixa 2", &u).unwrap();
        aprovar(&b.conn, &raiz, &r, LOJA, &Pedido::de(&c3), "Caixa 3", &u).unwrap();
        let k2 = hex(&c2.publica());
        let k3 = hex(&c3.publica());
        // Mexer em qualquer campo assinado derruba a aprovação.
        for (campo, valor) in [
            ("nome", "'Caixa 2 (mexido)'"),
            ("gerente", "'outro-gerente'"),
            ("aprovado_em", "'2020-01-01T00:00:00Z'"),
            ("chave_troca", &format!("'{}'", hex(&c3.publica_troca()))),
        ] {
            b.conn.execute_batch("SAVEPOINT mexer").unwrap();
            b.conn.execute(&format!("UPDATE caixa_aprovado SET {campo} = {valor} WHERE chave = ?1"), [&k2]).unwrap();
            assert_eq!(situacao(&b, &raiz, &c2.publica()), Situacao::Desconhecida, "{campo} mexido");
            assert_eq!(situacao(&b, &raiz, &c3.publica()), Situacao::Autorizada, "os outros continuam");
            b.conn.execute_batch("ROLLBACK TO mexer; RELEASE mexer").unwrap();
        }
        assert_eq!(situacao(&b, &raiz, &c2.publica()), Situacao::Autorizada);
        // Assinatura trocada (a de outra linha) ou com um bit mudado.
        let s3: String =
            b.conn.query_row("SELECT assinatura FROM caixa_aprovado WHERE chave = ?1", [&k3], |l| l.get(0)).unwrap();
        b.conn.execute("UPDATE caixa_aprovado SET assinatura = ?2 WHERE chave = ?1", [&k2, &s3]).unwrap();
        assert_eq!(situacao(&b, &raiz, &c2.publica()), Situacao::Desconhecida, "assinatura de outra linha");
        let mut s = de_hex::<64>(&s3).unwrap();
        s[10] ^= 0x04;
        b.conn.execute("UPDATE caixa_aprovado SET assinatura = ?2 WHERE chave = ?1", [&k3, &hex(&s)]).unwrap();
        assert_eq!(situacao(&b, &raiz, &c3.publica()), Situacao::Desconhecida, "assinatura com um bit mudado");
        // Lixo no lugar da assinatura não derruba a conferência das outras.
        b.conn.execute("UPDATE caixa_aprovado SET assinatura = 'zz' WHERE chave = ?1", [&k3]).unwrap();
        assert!(autorizada(&b.conn, &r, LOJA, &r).unwrap());
        // A raiz adulterada derruba a loja inteira (falha fechada).
        b.conn.execute("UPDATE caixa_aprovado SET nome = 'Raiz mexida' WHERE chave = aprovador", []).unwrap();
        assert!(!autorizada(&b.conn, &r, LOJA, &r).unwrap(), "raiz adulterada: nada vale");
    }

    #[test]
    fn retirada_forjada_ou_adulterada_nao_conta() {
        let (b, u, raiz) = loja();
        let r = raiz.publica();
        let c2 = Identidade::gerar().unwrap();
        aprovar(&b.conn, &raiz, &r, LOJA, &Pedido::de(&c2), "Caixa 2", &u).unwrap();
        // Um intruso (nunca aprovado) assina uma retirada do Caixa 2.
        let intruso = Identidade::gerar().unwrap();
        let mut falsa = Retirada {
            chave: c2.publica(),
            loja_id: LOJA.into(),
            motivo: "derrubar".into(),
            retirador: intruso.publica(),
            gerente: u.clone(),
            retirado_em: crate::tempo::agora(),
            assinatura: [0u8; 64],
        };
        falsa.assinatura = intruso.assinar(Dominio::Retirada, &falsa.corpo());
        assert!(falsa.assinatura_confere());
        let inserir = |r: &Retirada| {
            b.conn
                .execute(
                    "INSERT OR REPLACE INTO caixa_retirado (chave, retirador, loja_id, motivo, gerente, retirado_em, assinatura)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![hex(&r.chave), hex(&r.retirador), r.loja_id, r.motivo, r.gerente, r.retirado_em, hex(&r.assinatura)],
                )
                .unwrap()
        };
        inserir(&falsa);
        assert_eq!(situacao(&b, &raiz, &c2.publica()), Situacao::Autorizada, "retirada de quem não é da loja");
        // Retirada da raiz assinada por um caixa aprovado: não conta.
        let mut da_raiz = falsa.clone();
        da_raiz.chave = r;
        da_raiz.retirador = c2.publica();
        da_raiz.assinatura = c2.assinar(Dominio::Retirada, &da_raiz.corpo());
        inserir(&da_raiz);
        assert!(autorizada(&b.conn, &r, LOJA, &r).unwrap(), "a raiz não sai");
        // Retirada de verdade, depois adulterada (o motivo): deixa de contar.
        retirar(&b.conn, &raiz, &r, LOJA, &c2.publica(), "saiu da loja", &u).unwrap();
        assert_eq!(situacao(&b, &raiz, &c2.publica()), Situacao::Retirada);
        b.conn.execute("UPDATE caixa_retirado SET motivo = 'outro' WHERE retirador = ?1", [hex(&r)]).unwrap();
        assert_eq!(situacao(&b, &raiz, &c2.publica()), Situacao::Autorizada, "retirada adulterada não vale");
    }

    #[test]
    fn chave_desconhecida_e_recusada() {
        let (b, u, raiz) = loja();
        let r = raiz.publica();
        let estranho = Identidade::gerar().unwrap();
        assert_eq!(situacao(&b, &raiz, &estranho.publica()), Situacao::Desconhecida);
        assert!(!autorizada(&b.conn, &r, LOJA, &estranho.publica()).unwrap());
        // Aprovar a si mesmo (registro "de raiz" de quem não é a raiz) não vale.
        let auto = assinar_aprovacao(&estranho, estranho.publica(), estranho.publica_troca(), LOJA, "Eu", &u);
        gravar_aprovacao(&b.conn, &auto).unwrap();
        assert_eq!(situacao(&b, &raiz, &estranho.publica()), Situacao::Desconhecida, "autoaprovação");
        // Aprovação assinada por quem não é da loja, também não.
        let outro = Identidade::gerar().unwrap();
        let a = assinar_aprovacao(&outro, estranho.publica(), estranho.publica_troca(), LOJA, "Eu", &u);
        gravar_aprovacao(&b.conn, &a).unwrap();
        assert_eq!(situacao(&b, &raiz, &estranho.publica()), Situacao::Desconhecida, "aprovador de fora");
        // Quem não é da loja não aprova nem retira.
        let c2 = Identidade::gerar().unwrap();
        assert!(aprovar(&b.conn, &estranho, &r, LOJA, &Pedido::de(&c2), "Caixa 2", &u).is_err());
        assert!(retirar(&b.conn, &estranho, &r, LOJA, &r, "x", &u).is_err());
        // Sem raiz registrada, nada vale (nem a própria raiz).
        let (b2, _) = crate::faixas::para_testes::loja("Caixa 1");
        assert!(!autorizada(&b2.conn, &r, LOJA, &r).unwrap());
    }

    #[test]
    fn pedido_exige_prova_de_posse_da_chave() {
        let (b, u, raiz) = loja();
        let r = raiz.publica();
        let c2 = Identidade::gerar().unwrap();
        let bom = Pedido::de(&c2);
        assert!(bom.confere());
        assert_eq!(bom.codigo(), c2.codigo_de_pedido());
        // Alguém no caminho troca a chave do canal (X25519) do pedido: a prova
        // não confere e o código muda.
        let mut trocado = bom.clone();
        trocado.chave_troca = Identidade::gerar().unwrap().publica_troca();
        assert!(!trocado.confere());
        assert_ne!(trocado.codigo(), bom.codigo());
        assert!(aprovar(&b.conn, &raiz, &r, LOJA, &trocado, "Caixa 2", &u).is_err());
        // A chave de outro caixa com a prova de um terceiro: também não.
        let mut alheio = Pedido::de(&Identidade::gerar().unwrap());
        alheio.chave = c2.publica();
        assert!(!alheio.confere());
        assert!(aprovar(&b.conn, &raiz, &r, LOJA, &alheio, "Caixa 2", &u).is_err());
        assert_eq!(situacao(&b, &raiz, &c2.publica()), Situacao::Desconhecida);
        // Regras do aprovar/retirar: gerente, nome e motivo, e ninguém mexe em
        // si mesmo nem na raiz.
        assert!(aprovar(&b.conn, &raiz, &r, LOJA, &bom, "Caixa 2", " ").is_err(), "sem gerente");
        assert!(aprovar(&b.conn, &raiz, &r, LOJA, &bom, "  ", &u).is_err(), "sem nome");
        assert!(aprovar(&b.conn, &raiz, &r, LOJA, &Pedido::de(&raiz), "Eu", &u).is_err(), "a si mesmo");
        aprovar(&b.conn, &raiz, &r, LOJA, &bom, "Caixa 2", &u).unwrap();
        assert!(retirar(&b.conn, &raiz, &r, LOJA, &c2.publica(), " ", &u).is_err(), "sem motivo");
        assert!(retirar(&b.conn, &raiz, &r, LOJA, &r, "x", &u).is_err(), "a si mesmo");
        assert!(retirar(&b.conn, &c2, &r, LOJA, &r, "x", &u).is_err(), "a raiz");
        let estranho = Identidade::gerar().unwrap().publica();
        assert!(matches!(retirar(&b.conn, &raiz, &r, LOJA, &estranho, "x", &u), Err(Erro::NaoEncontrado(_))));
        retirar(&b.conn, &raiz, &r, LOJA, &c2.publica(), "saiu", &u).unwrap();
        assert!(retirar(&b.conn, &raiz, &r, LOJA, &c2.publica(), "de novo", &u).is_err(), "já retirado");
    }

    #[test]
    fn aprovacao_e_retirada_vao_para_a_auditoria_com_o_gerente() {
        let (b, u, raiz) = loja();
        let r = raiz.publica();
        let c2 = Identidade::gerar().unwrap();
        aprovar(&b.conn, &raiz, &r, LOJA, &Pedido::de(&c2), "Caixa 2", &u).unwrap();
        retirar(&b.conn, &raiz, &r, LOJA, &c2.publica(), "saiu da loja", &u).unwrap();
        let linhas: Vec<(String, Option<String>, String, String)> = b
            .conn
            .prepare(
                "SELECT acao, usuario_id, entidade_id, detalhes FROM auditoria
                  WHERE acao IN ('aprovar_caixa_raiz', 'aprovar_caixa', 'retirar_caixa_por_chave') ORDER BY quando, id",
            )
            .unwrap()
            .query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let acoes: Vec<&str> = linhas.iter().map(|l| l.0.as_str()).collect();
        assert_eq!(acoes, ["aprovar_caixa_raiz", "aprovar_caixa", "retirar_caixa_por_chave"]);
        assert!(linhas.iter().all(|l| l.1.as_deref() == Some(u.as_str())), "o gerente em cada uma");
        assert_eq!(linhas[1].2, hex(&c2.publica()));
        assert!(linhas[1].3.contains(&c2.codigo_de_pedido()), "o código conferido vai junto");
        assert!(linhas[2].3.contains("saiu da loja"));
    }

    /// Ponto de integração das etapas 4 e 7: o registro NÃO viaja pelo
    /// protocolo 3 (caixas da 0.9.17 não o conhecem, e a troca de hoje só
    /// exige a senha da rede). Quando o canal autenticado entrar, este teste
    /// muda junto, de propósito.
    #[test]
    fn registro_fica_fora_da_troca_do_protocolo_3() {
        for t in ["caixa_aprovado", "caixa_retirado"] {
            assert!(!crate::sync::TABELAS.contains(&t), "{t} não pode entrar na troca do protocolo 3");
        }
        let (b, u, raiz) = loja();
        let antes = contar(&b, "SELECT COUNT(*) FROM alteracao WHERE tabela IN ('caixa_aprovado', 'caixa_retirado')");
        let c2 = Identidade::gerar().unwrap();
        aprovar(&b.conn, &raiz, &raiz.publica(), LOJA, &Pedido::de(&c2), "Caixa 2", &u).unwrap();
        retirar(&b.conn, &raiz, &raiz.publica(), LOJA, &c2.publica(), "saiu", &u).unwrap();
        let depois = contar(&b, "SELECT COUNT(*) FROM alteracao WHERE tabela IN ('caixa_aprovado', 'caixa_retirado')");
        assert_eq!((antes, depois), (0, 0), "nada do registro entra no log da troca");
        let gatilhos = contar(
            &b,
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND tbl_name IN ('caixa_aprovado', 'caixa_retirado')",
        );
        assert_eq!(gatilhos, 0);
    }
}
