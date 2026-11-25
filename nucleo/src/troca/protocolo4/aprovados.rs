//! Registro assinado dos caixas aprovados e retirados — protocolo 4, etapa 2
//! (D1, E5).
//!
//! Duas tabelas (migração 0026): `caixa_aprovado` e `caixa_retirado`. Cada
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
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{de_hex, hex, verificar, Assinatura, Chave, Dominio, Identidade};
use crate::erro::{Erro, Resultado};

mod serde_hex_32 {
    use super::*;
    pub fn serialize<S: Serializer>(v: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex(v))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let t = String::deserialize(d)?;
        de_hex::<32>(&t).ok_or_else(|| serde::de::Error::custom("chave hex inválida (esperados 64 caracteres)"))
    }
}

mod serde_hex_64 {
    use super::*;
    pub fn serialize<S: Serializer>(v: &[u8; 64], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex(v))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 64], D::Error> {
        let t = String::deserialize(d)?;
        de_hex::<64>(&t).ok_or_else(|| serde::de::Error::custom("assinatura hex inválida (esperados 128 caracteres)"))
    }
}

/// Tamanho máximo do nome do caixa, do motivo da retirada e do `dispositivo` (I3).
const NOME_MAXIMO: usize = 80;
const MOTIVO_MAXIMO: usize = 300;
pub const DISPOSITIVO_MAXIMO: usize = 64;

/// Valida `dispositivo` (decisão I3): obrigatório no ingresso P4, forma canônica
/// (sem espaços nas pontas), até 64 caracteres ASCII seguros (`A-Z`, `a-z`, `0-9`, `-`, `_`).
pub fn validar_dispositivo(dispositivo: &str) -> Resultado<()> {
    if dispositivo.is_empty() {
        return Err(Erro::validacao("O pedido do caixa novo precisa informar o identificador deste caixa."));
    }
    if dispositivo != dispositivo.trim() || dispositivo.len() > DISPOSITIVO_MAXIMO {
        return Err(Erro::validacao("Identificador de dispositivo inválido no pedido do caixa."));
    }
    if !dispositivo.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(Erro::validacao("Identificador de dispositivo com caracteres inválidos no pedido do caixa."));
    }
    Ok(())
}

/// Valida forma canônica de `dispositivo` opcional (vazio aceito em linhas legadas 24–28).
fn dispositivo_canonico(dispositivo: &str) -> bool {
    dispositivo.is_empty()
        || (dispositivo == dispositivo.trim()
            && dispositivo.len() <= DISPOSITIVO_MAXIMO
            && dispositivo.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
}

/// O pedido de entrada de um caixa novo: as duas chaves públicas dele, o
/// identificador `dispositivo` (I3) e a prova de que ele tem as partes privadas
/// (assinatura Ed25519 no contexto [`Dominio::Pedido`]). O gerente confere o
/// [`Pedido::codigo`] na tela do caixa novo.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pedido {
    #[serde(with = "serde_hex_32")]
    pub chave: Chave,
    #[serde(with = "serde_hex_32")]
    pub chave_troca: Chave,
    /// UUID do dispositivo (`dispositivo_id`, decisão I3). Vazio em pedidos legados.
    #[serde(default)]
    pub dispositivo: String,
    #[serde(with = "serde_hex_64")]
    pub assinatura: Assinatura,
}

impl Pedido {
    fn corpo(chave: &Chave, chave_troca: &Chave, dispositivo: &str) -> Vec<u8> {
        let mut v = Vec::with_capacity(64 + dispositivo.len() + 10);
        v.extend_from_slice(chave);
        v.extend_from_slice(chave_troca);
        if !dispositivo.is_empty() {
            v.extend_from_slice(b"\x01disp\0");
            campo(&mut v, dispositivo);
        }
        v
    }

    /// O pedido deste caixa (sem `dispositivo` explícito).
    pub fn de(id: &Identidade) -> Pedido {
        Pedido::com_dispositivo(id, "")
    }

    /// O pedido deste caixa vinculando criptograficamente `dispositivo` (I3).
    pub fn com_dispositivo(id: &Identidade, dispositivo: &str) -> Pedido {
        let (chave, chave_troca) = (id.publica(), id.publica_troca());
        let disp = dispositivo.trim().to_string();
        let corpo = Pedido::corpo(&chave, &chave_troca, &disp);
        Pedido { chave, chave_troca, dispositivo: disp, assinatura: id.assinar(Dominio::Pedido, &corpo) }
    }

    /// A assinatura confere (quem pediu tem a chave privada Ed25519 e ligou a
    /// ela esta chave X25519 e o `dispositivo` em forma canônica)?
    pub fn confere(&self) -> bool {
        dispositivo_canonico(&self.dispositivo)
            && verificar(
                Dominio::Pedido,
                &self.chave,
                &Pedido::corpo(&self.chave, &self.chave_troca, &self.dispositivo),
                &self.assinatura,
            )
    }

    /// O código que o gerente confere.
    pub fn codigo(&self) -> String {
        super::codigo_de_pedido(&self.chave, &self.chave_troca)
    }
}

/// Uma linha de `caixa_aprovado`: `aprovador` aprovou `chave` na loja
/// `loja_id`, com o gerente `gerente` e `dispositivo` (I3), e assinou tudo.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Aprovacao {
    #[serde(with = "serde_hex_32")]
    pub chave: Chave,
    #[serde(with = "serde_hex_32")]
    pub chave_troca: Chave,
    pub loja_id: String,
    pub nome: String,
    #[serde(with = "serde_hex_32")]
    pub aprovador: Chave,
    pub gerente: String,
    pub aprovado_em: String,
    /// UUID do caixa (`dispositivo_id`, decisão I3). Incluído na assinatura quando preenchido.
    #[serde(default)]
    pub dispositivo: String,
    #[serde(with = "serde_hex_64")]
    pub assinatura: Assinatura,
}

/// Uma linha de `caixa_retirado`: `retirador` retirou `chave` da loja.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Retirada {
    #[serde(with = "serde_hex_32")]
    pub chave: Chave,
    pub loja_id: String,
    pub motivo: String,
    #[serde(with = "serde_hex_32")]
    pub retirador: Chave,
    pub gerente: String,
    pub retirado_em: String,
    #[serde(with = "serde_hex_64")]
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
        if !self.dispositivo.is_empty() {
            v.extend_from_slice(b"\x01disp\0");
            campo(&mut v, &self.dispositivo);
        }
        v
    }

    /// A assinatura do aprovador confere (e `dispositivo` está em forma canônica)?
    pub fn assinatura_confere(&self) -> bool {
        #[cfg(test)]
        conferencias_de_teste::contar();
        dispositivo_canonico(&self.dispositivo)
            && verificar(Dominio::Aprovacao, &self.aprovador, &self.corpo(), &self.assinatura)
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
        #[cfg(test)]
        conferencias_de_teste::contar();
        verificar(Dominio::Retirada, &self.retirador, &self.corpo(), &self.assinatura)
    }
}

/// C2R4-3: quantas assinaturas de linhas do registro esta linha de execução conferiu
/// (só nos testes).
#[cfg(test)]
pub(crate) mod conferencias_de_teste {
    use std::cell::Cell;
    thread_local!(static CONTADOR: Cell<usize> = const { Cell::new(0) });
    pub(crate) fn contar() {
        CONTADOR.with(|c| c.set(c.get() + 1));
    }
    /// Zera e devolve a contagem até aqui.
    pub(crate) fn tirar() -> usize {
        CONTADOR.with(|c| c.replace(0))
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

    /// Todas as chaves autorizadas neste momento.
    pub fn autorizadas(&self) -> &HashSet<Chave> {
        &self.autorizados
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
        "SELECT chave, chave_troca, loja_id, nome, aprovador, gerente, aprovado_em, dispositivo, assinatura
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
                dispositivo: texto(l, 7)?,
                assinatura: de_hex(&texto(l, 8)?)?,
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

/// As aprovações gravadas desta loja com a assinatura conferida (cada uma, uma vez).
fn aprovacoes_validas(conn: &Connection, loja_id: &str) -> Resultado<Vec<Aprovacao>> {
    Ok(ler_aprovacoes(conn, loja_id)?.into_iter().filter(|a| a.loja_id == loja_id && a.assinatura_confere()).collect())
}

/// Confere o registro inteiro da loja `loja_id` a partir da `raiz`.
pub fn avaliar(conn: &Connection, raiz: &Chave, loja_id: &str) -> Resultado<Avaliacao> {
    if loja_id.is_empty() {
        return Ok(Avaliacao::default());
    }
    // 1. Só as aprovações com assinatura válida, desta loja.
    let validas = aprovacoes_validas(conn, loja_id)?;
    Ok(avaliar_com(raiz, loja_id, &validas, &ler_retiradas(conn, loja_id)?))
}

/// [`avaliar`] sobre aprovações já conferidas (`validas`: desta loja, assinatura certa).
fn avaliar_com(raiz: &Chave, loja_id: &str, validas: &[Aprovacao], retiradas: &[Retirada]) -> Avaliacao {
    // Aprovação de si mesmo só vale para a raiz.
    let mut raiz_valida = false;
    let mut arestas: HashMap<Chave, Vec<Chave>> = HashMap::new();
    for a in validas {
        if a.aprovador == a.chave {
            raiz_valida |= a.chave == *raiz;
            continue;
        }
        arestas.entry(a.aprovador).or_default().push(a.chave);
    }
    if !raiz_valida {
        return Avaliacao::default();
    }
    // 2. Aprovados alguma vez (sem olhar retiradas).
    let aprovados = alcancados(raiz, &arestas, &HashSet::new());
    // 3. Retiradas válidas: assinadas por quem já foi aprovado; a raiz não sai.
    let retirados: HashSet<Chave> = retiradas
        .iter()
        .filter(|r| {
            r.loja_id == loja_id && r.chave != *raiz && aprovados.contains(&r.retirador) && r.assinatura_confere()
        })
        .map(|r| r.chave)
        .collect();
    // 4. Autorizados: cadeia a partir da raiz sem passar por retirado.
    let autorizados = alcancados(raiz, &arestas, &retirados);
    Avaliacao { aprovados, retirados, autorizados }
}

/// C2R5-2 (R5 da 2C): o registro de uma loja conferido de uma vez (cada assinatura, uma
/// vez), para as consultas que olham todos os caixas (a lista de caixas, o Início): quem
/// está autorizado e quem cai junto se cada um sair, sem conferir tudo de novo por caixa.
pub struct RegistroConferido {
    raiz: Chave,
    validas: Vec<Aprovacao>,
    avaliacao: Avaliacao,
    /// As chaves retiradas pela própria raiz (o lugar delas volta: C2R4-1).
    pela_raiz: HashSet<Chave>,
}

impl RegistroConferido {
    pub fn carregar(conn: &Connection, raiz: &Chave, loja_id: &str) -> Resultado<RegistroConferido> {
        if loja_id.trim().is_empty() {
            return Ok(RegistroConferido {
                raiz: *raiz,
                validas: Vec::new(),
                avaliacao: Avaliacao::default(),
                pela_raiz: HashSet::new(),
            });
        }
        let validas = aprovacoes_validas(conn, loja_id)?;
        let retiradas = ler_retiradas(conn, loja_id)?;
        let avaliacao = avaliar_com(raiz, loja_id, &validas, &retiradas);
        let pela_raiz = retiradas
            .iter()
            .filter(|t| {
                t.loja_id == loja_id
                    && t.retirador == *raiz
                    && avaliacao.retirados.contains(&t.chave)
                    && t.assinatura_confere()
            })
            .map(|t| t.chave)
            .collect();
        Ok(RegistroConferido { raiz: *raiz, validas, avaliacao, pela_raiz })
    }

    /// C2R5-1: a raiz já retirou esta chave.
    pub fn retirada_pela_raiz(&self, chave: &Chave) -> bool {
        self.pela_raiz.contains(chave)
    }

    /// C2R4-1: o lugar ocupado no registro gravado.
    fn ocupacao(&self) -> Ocupacao {
        let mut arestas: HashMap<Chave, Vec<Chave>> = HashMap::new();
        for a in &self.validas {
            if a.aprovador != a.chave {
                arestas.entry(a.aprovador).or_default().push(a.chave);
            }
        }
        ocupacao(&self.raiz, self.validas.iter(), &arestas, &self.pela_raiz, &self.avaliacao.retirados)
    }

    /// C2R5-4: o registro está no teto (os caixas, menos a raiz, não aprovam caixas novos).
    pub fn cheio(&self) -> bool {
        self.ocupacao().no_teto >= limites_vigentes().teto
    }

    pub fn avaliacao(&self) -> &Avaliacao {
        &self.avaliacao
    }

    /// As aprovações desta loja com a assinatura certa.
    pub fn aprovacoes(&self) -> &[Aprovacao] {
        &self.validas
    }

    /// A situação de cada chave por `dispositivo` declarado nas aprovações válidas.
    pub fn situacoes_por_dispositivo(&self) -> HashMap<String, Vec<(Chave, Situacao)>> {
        let mut m: HashMap<String, Vec<(Chave, Situacao)>> = HashMap::new();
        for a in &self.validas {
            let disp = a.dispositivo.trim();
            if disp.is_empty() {
                continue;
            }
            let lista = m.entry(disp.to_string()).or_default();
            if !lista.iter().any(|(k, _)| *k == a.chave) {
                lista.push((a.chave, self.avaliacao.situacao(&a.chave)));
            }
        }
        m
    }

    /// A cadeia viva (aprovador autorizado → aprovado autorizado) e a aprovação mais
    /// recente de cada chave, feita por um aprovador autorizado (o nome que a tela mostra).
    fn cadeia_viva(&self) -> (HashMap<Chave, Vec<Chave>>, HashMap<Chave, &Aprovacao>) {
        let vivos = &self.avaliacao.autorizados;
        let mut arestas: HashMap<Chave, Vec<Chave>> = HashMap::new();
        let mut por_chave: HashMap<Chave, &Aprovacao> = HashMap::new();
        for a in &self.validas {
            if !vivos.contains(&a.aprovador) {
                continue;
            }
            if a.aprovador != a.chave && vivos.contains(&a.chave) {
                arestas.entry(a.aprovador).or_default().push(a.chave);
            }
            por_chave
                .entry(a.chave)
                .and_modify(|atual| {
                    if fica(&a.aprovado_em, &a.assinatura) > fica(&atual.aprovado_em, &atual.assinatura) {
                        *atual = a;
                    }
                })
                .or_insert(a);
        }
        (arestas, por_chave)
    }

    /// P4-4: as aprovações dos caixas hoje autorizados que ficariam `SemCadeia` se `chave`
    /// fosse retirada, por nome.
    pub fn dependentes_de(&self, chave: &Chave) -> Vec<Aprovacao> {
        if self.avaliacao.situacao(chave) != Situacao::Autorizada {
            return Vec::new();
        }
        let (arestas, por_chave) = self.cadeia_viva();
        let mut fora = self.avaliacao.retirados.clone();
        fora.insert(*chave);
        let depois = alcancados(&self.raiz, &arestas, &fora);
        let mut afetados: Vec<Aprovacao> = self
            .avaliacao
            .autorizados
            .iter()
            .filter(|k| **k != *chave && !depois.contains(*k))
            .filter_map(|k| por_chave.get(k).map(|a| (*a).clone()))
            .collect();
        afetados.sort_by(|a, b| a.nome.cmp(&b.nome));
        afetados
    }

    /// C2R5-2: para cada chave autorizada (menos a raiz), quem cai junto se ela sair (as
    /// que ela domina na cadeia viva): até `ate` aprovações, por nome, e quantas são ao
    /// todo. Uma conta só para todas (os dominadores), sem conferir nada de novo: numa
    /// cadeia comprida a lista inteira de cada uma cresceria com o quadrado.
    pub fn quem_cai_junto(&self, ate: usize) -> HashMap<Chave, (Vec<Aprovacao>, usize)> {
        let mut m = HashMap::new();
        if !self.avaliacao.autorizados.contains(&self.raiz) {
            return m;
        }
        let (arestas, por_chave) = self.cadeia_viva();
        let mut filhos: HashMap<Chave, Vec<Chave>> = HashMap::new();
        for (v, d) in dominadores(&self.raiz, &arestas) {
            if v != d {
                filhos.entry(d).or_default().push(v);
            }
        }
        let tamanho = tamanhos_das_subarvores(&self.raiz, &filhos);
        for k in &self.avaliacao.autorizados {
            if *k == self.raiz {
                continue;
            }
            let total = tamanho.get(k).copied().unwrap_or(1).saturating_sub(1);
            let mut lista: Vec<Aprovacao> = Vec::new();
            let mut pilha: Vec<Chave> = filhos.get(k).cloned().unwrap_or_default();
            while let Some(v) = pilha.pop() {
                if lista.len() >= ate {
                    break;
                }
                if let Some(a) = por_chave.get(&v) {
                    lista.push((*a).clone());
                }
                pilha.extend(filhos.get(&v).into_iter().flatten().copied());
            }
            lista.sort_by(|a, b| a.nome.cmp(&b.nome));
            m.insert(*k, (lista, total));
        }
        m
    }
}

/// C2R5-2: o dominador imediato de cada chave alcançada a partir da `raiz` (quem domina
/// `v` está em todo caminho da raiz até `v`; a raiz domina a si mesma). Algoritmo
/// iterativo de Cooper, Harvey e Kennedy, sobre a pós-ordem.
fn dominadores(raiz: &Chave, arestas: &HashMap<Chave, Vec<Chave>>) -> HashMap<Chave, Chave> {
    let mut pos: Vec<Chave> = Vec::new();
    let mut visto: HashSet<Chave> = HashSet::from([*raiz]);
    let mut pilha: Vec<(Chave, usize)> = vec![(*raiz, 0)];
    while let Some(topo) = pilha.last_mut() {
        let (n, i) = *topo;
        match arestas.get(&n).and_then(|f| f.get(i)) {
            Some(f) => {
                topo.1 += 1;
                if visto.insert(*f) {
                    pilha.push((*f, 0));
                }
            }
            None => {
                pos.push(n);
                pilha.pop();
            }
        }
    }
    let num: HashMap<Chave, usize> = pos.iter().enumerate().map(|(i, k)| (*k, i)).collect();
    let mut antecessores: Vec<Vec<usize>> = vec![Vec::new(); pos.len()];
    for (de, paras) in arestas {
        let Some(&d) = num.get(de) else { continue };
        for p in paras {
            if let Some(&q) = num.get(p) {
                antecessores[q].push(d);
            }
        }
    }
    let r = pos.len() - 1;
    let mut idom: Vec<Option<usize>> = vec![None; pos.len()];
    idom[r] = Some(r);
    let cruzar = |idom: &[Option<usize>], mut a: usize, mut b: usize| -> usize {
        while a != b {
            while a < b {
                a = idom[a].unwrap_or(r);
            }
            while b < a {
                b = idom[b].unwrap_or(r);
            }
        }
        a
    };
    let mut mudou = true;
    while mudou {
        mudou = false;
        for b in (0..r).rev() {
            let mut novo: Option<usize> = None;
            for &p in &antecessores[b] {
                if idom[p].is_some() {
                    novo = Some(match novo {
                        None => p,
                        Some(n) => cruzar(&idom, p, n),
                    });
                }
            }
            if novo.is_some() && idom[b] != novo {
                idom[b] = novo;
                mudou = true;
            }
        }
    }
    pos.iter().enumerate().filter_map(|(i, k)| idom[i].map(|d| (*k, pos[d]))).collect()
}

/// C2R5-2: o tamanho da subárvore de cada nó (ele mesmo incluído).
fn tamanhos_das_subarvores(raiz: &Chave, filhos: &HashMap<Chave, Vec<Chave>>) -> HashMap<Chave, usize> {
    let mut ordem = Vec::new();
    let mut pilha = vec![*raiz];
    while let Some(v) = pilha.pop() {
        ordem.push(v);
        pilha.extend(filhos.get(&v).into_iter().flatten().copied());
    }
    let mut t: HashMap<Chave, usize> = HashMap::new();
    for v in ordem.iter().rev() {
        let s = 1 + filhos.get(v).into_iter().flatten().map(|f| t.get(f).copied().unwrap_or(0)).sum::<usize>();
        t.insert(*v, s);
    }
    t
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
        "INSERT INTO caixa_aprovado (chave, aprovador, chave_troca, loja_id, nome, gerente, aprovado_em, dispositivo, assinatura)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT (chave, aprovador) DO UPDATE SET
            chave_troca = excluded.chave_troca, loja_id = excluded.loja_id, nome = excluded.nome,
            gerente = excluded.gerente, aprovado_em = excluded.aprovado_em,
            dispositivo = excluded.dispositivo, assinatura = excluded.assinatura",
        params![
            hex(&a.chave),
            hex(&a.aprovador),
            hex(&a.chave_troca),
            a.loja_id,
            a.nome,
            a.gerente,
            a.aprovado_em,
            a.dispositivo,
            hex(&a.assinatura)
        ],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn assinar_aprovacao(
    aprovador: &Identidade,
    chave: Chave,
    chave_troca: Chave,
    loja_id: &str,
    nome: &str,
    gerente: &str,
    dispositivo_alvo: &str,
) -> Aprovacao {
    let mut a = Aprovacao {
        chave,
        chave_troca,
        loja_id: loja_id.to_string(),
        nome: nome.to_string(),
        aprovador: aprovador.publica(),
        gerente: gerente.to_string(),
        aprovado_em: crate::tempo::agora(),
        dispositivo: dispositivo_alvo.trim().to_string(),
        assinatura: [0u8; 64],
    };
    a.assinatura = aprovador.assinar(Dominio::Aprovacao, &a.corpo());
    a
}

/// Loja nova: este caixa vira a **raiz** da cadeia (registro assinado por ele
/// mesmo, com o gerente que criou a loja e o `dispositivo_id` local — I3).
/// Uma loja só tem uma raiz: se já houver outra raiz registrada para esta loja, recusa.
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
    let disp = dispositivo(conn)?;
    let a = assinar_aprovacao(eu, eu.publica(), eu.publica_troca(), loja_id, &nome, gerente, &disp);
    em_bloco(conn, || {
        gravar_aprovacao(conn, &a)?;
        crate::auditoria::registrar(
            conn,
            &disp,
            Some(gerente),
            "aprovar_caixa_raiz",
            "caixa",
            &hex(&a.chave),
            &[format!("Caixa: {nome}"), format!("Código: {}", eu.codigo_de_pedido())],
        )
    })?;
    Ok(a)
}

/// A única chave pública de raiz válida registrada em `caixa_aprovado` para `loja_id`.
///
/// Falha fechada (A1): se existir mais de uma chave autoassinada distinta para a mesma
/// loja, devolve erro em vez de escolher uma arbitrariamente. Nunca substitui um pino
/// `p4_raiz` já fixado em `config` (veja `cofre::raiz_atual`).
pub fn raiz_registrada(conn: &Connection, loja_id: &str) -> Resultado<Option<Chave>> {
    if loja_id.trim().is_empty() {
        return Ok(None);
    }
    let mut raizes: std::collections::BTreeSet<Chave> = std::collections::BTreeSet::new();
    for a in ler_aprovacoes(conn, loja_id)? {
        if a.loja_id == loja_id && a.chave == a.aprovador && a.assinatura_confere() {
            raizes.insert(a.chave);
        }
    }
    match raizes.len() {
        0 => Ok(None),
        1 => Ok(raizes.into_iter().next()),
        n => Err(Erro::validacao(format!(
            "Encontradas {n} raízes distintas no registro de caixas desta loja; recusa segura."
        ))),
    }
}

/// Chave pública **autorizada** registrada para `dispositivo_alvo` (I3, A3).
///
/// Restringe estritamente a chaves autorizadas na cadeia da `raiz` (excluindo
/// retiradas, sem cadeia e autoaprovações de estranhos). Se mais de uma chave
/// autorizada distinta declarar o mesmo `dispositivo`, recusa por ambiguidade.
pub fn chave_do_dispositivo(
    conn: &Connection,
    raiz: &Chave,
    loja_id: &str,
    dispositivo_alvo: &str,
) -> Resultado<Option<Chave>> {
    let alvo = dispositivo_alvo.trim();
    if loja_id.trim().is_empty() || alvo.is_empty() {
        return Ok(None);
    }
    let av = avaliar(conn, raiz, loja_id)?;
    let mut chaves: std::collections::BTreeSet<Chave> = std::collections::BTreeSet::new();
    for a in ler_aprovacoes(conn, loja_id)? {
        if a.loja_id == loja_id
            && a.dispositivo.trim() == alvo
            && av.situacao(&a.chave) == Situacao::Autorizada
            && av.situacao(&a.aprovador) == Situacao::Autorizada
            && a.assinatura_confere()
        {
            chaves.insert(a.chave);
        }
    }
    match chaves.len() {
        0 => Ok(None),
        1 => Ok(chaves.into_iter().next()),
        n => Err(Erro::validacao(format!("{TEXTO_CAIXA_COM_VARIAS_IDENTIDADES} ({n} identidades aprovadas)"))),
    }
}

/// T012: a situação de cada chave por `dispositivo` declarado nas aprovações
/// válidas (assinatura conferida) desta loja, a partir da `raiz`. Para a tela
/// (quem tem chave, quem foi retirado pela chave); quem decide a troca continua
/// sendo [`chave_do_dispositivo`] e [`autorizada`].
pub fn situacoes_por_dispositivo(
    conn: &Connection,
    raiz: &Chave,
    loja_id: &str,
) -> Resultado<HashMap<String, Vec<(Chave, Situacao)>>> {
    Ok(RegistroConferido::carregar(conn, raiz, loja_id)?.situacoes_por_dispositivo())
}

/// P4-4: caixas hoje autorizados que ficariam `SemCadeia` se `chave` fosse retirada.
pub fn dependentes_de(conn: &Connection, raiz: &Chave, loja_id: &str, chave: &Chave) -> Resultado<Vec<Aprovacao>> {
    Ok(RegistroConferido::carregar(conn, raiz, loja_id)?.dependentes_de(chave))
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
    aprovar_com_limites(conn, aprovador, raiz, loja_id, pedido, nome, gerente, Limites::PADRAO)
}

/// [`aprovar`] com os limites do registro dados (C2R4-1; os testes usam limites
/// pequenos).
#[allow(clippy::too_many_arguments)]
pub(crate) fn aprovar_com_limites(
    conn: &Connection,
    aprovador: &Identidade,
    raiz: &Chave,
    loja_id: &str,
    pedido: &Pedido,
    nome: &str,
    gerente: &str,
    lim: Limites,
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
    // F-05 (I3): o mesmo caixa (`dispositivo`) não fica com duas chaves autorizadas ao
    // mesmo tempo (acontece quando ele refaz a identidade e pede entrada de novo, ou
    // num banco copiado). Recusa já aqui, com o caminho de saída: retirar antes o
    // registro antigo (a tela da retirada lista quem depende dele, P4-4).
    if !pedido.dispositivo.is_empty() {
        match chave_do_dispositivo(conn, raiz, loja_id, &pedido.dispositivo) {
            Ok(Some(atual)) if atual == pedido.chave => {}
            Ok(None) => {}
            Ok(Some(_)) => return Err(Erro::validacao(TEXTO_CAIXA_COM_OUTRA_IDENTIDADE)),
            // Conflito já existente (duas chaves autorizadas, vindas pela troca) ou erro
            // do banco: o motivo real sobe (N-06).
            Err(e) => return Err(e),
        }
    }
    // C2R4-1: a aprovação que os outros caixas não guardariam (registro cheio, ou este
    // caixa no limite de aprovados) é recusada já aqui, com o caminho de saída.
    if let Some(motivo) = falta_de_lugar(conn, &aprovador.publica(), raiz, loja_id, &pedido.chave, lim)? {
        return Err(Erro::validacao(motivo));
    }
    let a =
        assinar_aprovacao(aprovador, pedido.chave, pedido.chave_troca, loja_id, &nome, gerente, &pedido.dispositivo);
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

/// I3: o mesmo caixa ficou aprovado com mais de uma identidade (aprovações feitas ao
/// mesmo tempo em dois caixas da loja, que se encontraram pela troca).
pub const TEXTO_CAIXA_COM_VARIAS_IDENTIDADES: &str = "Este caixa está aprovado nesta loja com mais de uma identidade. \
     Retire em Configurações › Caixas da loja as que não valem mais antes de aprová-lo de novo.";

/// F-05: o caixa que pede entrada já está aprovado com outra identidade.
pub const TEXTO_CAIXA_COM_OUTRA_IDENTIDADE: &str =
    "Este caixa já está aprovado nesta loja com outra identidade (de antes). Para aprová-lo de novo, retire \
     primeiro o registro antigo dele em Configurações › Caixas da loja e depois gere um convite novo.";

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
        // C2R5-1 (R5 da 2C): a raiz retira também a chave que outro caixa já retirou (uma
        // vez): a retirada dela devolve o lugar que essa chave ocupa no registro.
        Situacao::Retirada if retirador.publica() == *raiz && retirada_gravada(conn, chave, raiz)?.is_none() => {}
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
        let forcada = assinar_aprovacao(&raiz, c2.publica(), c2.publica_troca(), LOJA, "Caixa 2", &u, "");
        gravar_aprovacao(&b.conn, &forcada).unwrap();
        assert_eq!(situacao(&b, &raiz, &c2.publica()), Situacao::Retirada);
        // Trocar o "identificador declarado" (nome, dispositivo) não muda nada:
        // a retirada é da chave.
        b.conn.execute("UPDATE config SET valor = 'outro-id' WHERE chave = 'dispositivo_id'", []).unwrap();
        assert_eq!(situacao(&b, &raiz, &c2.publica()), Situacao::Retirada);
    }

    /// I3 (0.9.19-dev.2): o `dispositivo` vai assinado no Pedido e na Aprovacao;
    /// adulterar a coluna `dispositivo` no banco derruba a assinatura; P4-4 lista dependentes.
    #[test]
    fn i3_dispositivo_assinado_e_dependentes_p4_4() {
        let (b, u, raiz) = loja();
        let r = raiz.publica();
        let (c2, c3) = (Identidade::gerar().unwrap(), Identidade::gerar().unwrap());
        let mut p2 = Pedido::com_dispositivo(&c2, "disp-caixa-2");
        assert!(p2.confere());
        p2.dispositivo = "disp-adulterado".into();
        assert!(!p2.confere(), "trocar dispositivo no pedido invalida assinatura");
        let p2_ok = Pedido::com_dispositivo(&c2, "disp-caixa-2");
        let a2 = aprovar(&b.conn, &raiz, &r, LOJA, &p2_ok, "Caixa 2", &u).unwrap();
        assert_eq!(a2.dispositivo, "disp-caixa-2");
        assert_eq!(chave_do_dispositivo(&b.conn, &r, LOJA, "disp-caixa-2").unwrap(), Some(c2.publica()));
        assert_eq!(raiz_registrada(&b.conn, LOJA).unwrap(), Some(r));

        // A3: estranho não autorizado junta aprovação autoassinada com o mesmo `dispositivo`
        // e data futura (2999) — `chave_do_dispositivo` continua devolvendo a vítima autorizada!
        let estranho = Identidade::gerar().unwrap();
        let mut falsa = assinar_aprovacao(
            &estranho,
            estranho.publica(),
            estranho.publica_troca(),
            LOJA,
            "Falso",
            &u,
            "disp-caixa-2",
        );
        falsa.aprovado_em = "2999-12-31T23:59:59Z".into();
        falsa.assinatura = estranho.assinar(Dominio::Aprovacao, &falsa.corpo());
        juntar(&b.conn, LOJA, &Linhas { aprovacoes: vec![falsa], retiradas: Vec::new() }).unwrap();
        assert_eq!(
            chave_do_dispositivo(&b.conn, &r, LOJA, "disp-caixa-2").unwrap(),
            Some(c2.publica()),
            "A3: aprovação de estranho nunca substitui a chave autorizada"
        );
        // A1: com duas raízes autoassinadas no banco, `raiz_registrada` falha fechada.
        assert!(raiz_registrada(&b.conn, LOJA).is_err(), "A1: >1 raiz distinta deve recusar");
        // Remove a linha falsa para seguir com o teste de dependentes e conflito de dispositivo.
        b.conn.execute("DELETE FROM caixa_aprovado WHERE chave = ?1", [hex(&estranho.publica())]).unwrap();

        let p3 = Pedido::com_dispositivo(&c3, "disp-caixa-3");
        aprovar(&b.conn, &c2, &r, LOJA, &p3, "Caixa 3", &u).unwrap();
        // F-05: `aprovar` recusa uma segunda chave para o mesmo caixa (`dispositivo`).
        let c4 = Identidade::gerar().unwrap();
        let p4 = Pedido::com_dispositivo(&c4, "disp-caixa-3");
        assert!(matches!(
            aprovar(&b.conn, &raiz, &r, LOJA, &p4, "Caixa 4", &u),
            Err(Erro::Validacao { ref mensagem, .. }) if mensagem == TEXTO_CAIXA_COM_OUTRA_IDENTIDADE
        ));
        // A3: o conflito ainda pode chegar pela troca (duas aprovações feitas ao mesmo
        // tempo em dois caixas autorizados): duas chaves autorizadas com o mesmo
        // `dispositivo` devolvem erro (falha fechada).
        let concorrente =
            assinar_aprovacao(&raiz, c4.publica(), c4.publica_troca(), LOJA, "Caixa 4", &u, "disp-caixa-3");
        juntar(&b.conn, LOJA, &Linhas { aprovacoes: vec![concorrente], retiradas: Vec::new() }).unwrap();
        assert!(
            chave_do_dispositivo(&b.conn, &r, LOJA, "disp-caixa-3").is_err(),
            "A3: >1 chave autorizada com o mesmo dispositivo deve falhar"
        );
        // N-06: com o conflito, `aprovar` mostra o motivo real (não "aprovado com outra identidade").
        let c5 = Identidade::gerar().unwrap();
        let recusa = aprovar(&b.conn, &raiz, &r, LOJA, &Pedido::com_dispositivo(&c5, "disp-caixa-3"), "Caixa 5", &u);
        assert!(
            matches!(&recusa, Err(Erro::Validacao { mensagem, .. }) if mensagem.starts_with(TEXTO_CAIXA_COM_VARIAS_IDENTIDADES)),
            "{recusa:?}"
        );
        b.conn.execute("DELETE FROM caixa_aprovado WHERE chave = ?1", [hex(&c4.publica())]).unwrap();

        // B9 / M4: dispositivo com espaço nas pontas ou grande demais falha em `confere` / `validar_dispositivo`.
        assert!(validar_dispositivo("").is_err());
        assert!(validar_dispositivo(" disp ").is_err());
        assert!(validar_dispositivo(&"a".repeat(DISPOSITIVO_MAXIMO + 1)).is_err());
        assert!(validar_dispositivo("disp/invalido").is_err());
        assert!(validar_dispositivo("disp-ok_123").is_ok());
        let mut p_nao_canon = p2_ok.clone();
        p_nao_canon.dispositivo = " disp-caixa-2 ".into();
        p_nao_canon.assinatura = c2.assinar(
            Dominio::Pedido,
            &Pedido::corpo(&p_nao_canon.chave, &p_nao_canon.chave_troca, &p_nao_canon.dispositivo),
        );
        assert!(!p_nao_canon.confere(), "B9: dispositivo não canônico é recusado");

        // M1-R2: estranho (não autorizado) assina linha mirando `c2` (autorizado) com
        // `dispositivo = "disp-caixa-3"` e `c3` com `nome = "Spoof"` em ano 2999.
        // Nem `chave_do_dispositivo` sofre DoS/conflito, nem `dependentes_de` exibe o nome falso!
        let mut dos_i3 =
            assinar_aprovacao(&estranho, c2.publica(), c2.publica_troca(), LOJA, "Spoof C2", &u, "disp-caixa-3");
        dos_i3.aprovado_em = "2999-12-31T23:59:59Z".into();
        dos_i3.assinatura = estranho.assinar(Dominio::Aprovacao, &dos_i3.corpo());
        let mut spoof_c3 =
            assinar_aprovacao(&estranho, c3.publica(), c3.publica_troca(), LOJA, "Spoof C3", &u, "disp-caixa-3");
        spoof_c3.aprovado_em = "2999-12-31T23:59:59Z".into();
        spoof_c3.assinatura = estranho.assinar(Dominio::Aprovacao, &spoof_c3.corpo());
        juntar(&b.conn, LOJA, &Linhas { aprovacoes: vec![dos_i3, spoof_c3], retiradas: Vec::new() }).unwrap();
        assert_eq!(
            chave_do_dispositivo(&b.conn, &r, LOJA, "disp-caixa-3").unwrap(),
            Some(c3.publica()),
            "M1-R2: linha de aprovador não autorizado não causa DoS/conflito em chave_do_dispositivo"
        );

        // P4-4: retirar Caixa 2 derrubaria Caixa 3 em cascata (com nome legítimo, não o spoof).
        let deps = dependentes_de(&b.conn, &r, LOJA, &c2.publica()).unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].chave, c3.publica());
        assert_eq!(deps[0].nome, "Caixa 3", "M1-R2: aprovador não autorizado não adultera metadados de dependentes");

        // Adulterar `dispositivo` no banco derruba a assinatura de Caixa 2 (I3).
        b.conn
            .execute("UPDATE caixa_aprovado SET dispositivo = 'clonado' WHERE chave = ?1", [hex(&c2.publica())])
            .unwrap();
        assert_eq!(situacao(&b, &raiz, &c2.publica()), Situacao::Desconhecida, "dispositivo adulterado não vale");
    }

    /// F-05: o caixa que refez a identidade (ou um banco copiado) pede entrada de
    /// novo com o mesmo `dispositivo`: a aprovação é recusada com o caminho de saída;
    /// depois de retirar o registro antigo, a aprovação da chave nova passa e o
    /// caixa volta a ter uma chave só.
    #[test]
    fn f05_mesmo_caixa_com_chave_nova_exige_retirar_a_antiga() {
        let (b, u, raiz) = loja();
        let r = raiz.publica();
        let antiga = Identidade::gerar().unwrap();
        aprovar(&b.conn, &raiz, &r, LOJA, &Pedido::com_dispositivo(&antiga, "disp-caixa-2"), "Caixa 2", &u).unwrap();
        let nova = Identidade::gerar().unwrap();
        let pedido_novo = Pedido::com_dispositivo(&nova, "disp-caixa-2");
        let recusa = aprovar(&b.conn, &raiz, &r, LOJA, &pedido_novo, "Caixa 2", &u);
        assert!(
            matches!(&recusa, Err(Erro::Validacao { mensagem, .. }) if mensagem == TEXTO_CAIXA_COM_OUTRA_IDENTIDADE),
            "{recusa:?}"
        );
        assert_eq!(situacao(&b, &raiz, &nova.publica()), Situacao::Desconhecida, "nada foi gravado");
        assert_eq!(chave_do_dispositivo(&b.conn, &r, LOJA, "disp-caixa-2").unwrap(), Some(antiga.publica()));
        // Reaprovar a MESMA chave (repetição) continua valendo.
        aprovar(&b.conn, &raiz, &r, LOJA, &Pedido::com_dispositivo(&antiga, "disp-caixa-2"), "Caixa 2", &u).unwrap();
        // O caminho de saída: retirar o registro antigo e aprovar a chave nova.
        retirar(&b.conn, &raiz, &r, LOJA, &antiga.publica(), "refez a identidade", &u).unwrap();
        aprovar(&b.conn, &raiz, &r, LOJA, &pedido_novo, "Caixa 2", &u).unwrap();
        assert_eq!(chave_do_dispositivo(&b.conn, &r, LOJA, "disp-caixa-2").unwrap(), Some(nova.publica()));
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
        let auto = assinar_aprovacao(&estranho, estranho.publica(), estranho.publica_troca(), LOJA, "Eu", &u, "");
        gravar_aprovacao(&b.conn, &auto).unwrap();
        assert_eq!(situacao(&b, &raiz, &estranho.publica()), Situacao::Desconhecida, "autoaprovação");
        // Aprovação assinada por quem não é da loja, também não.
        let outro = Identidade::gerar().unwrap();
        let a = assinar_aprovacao(&outro, estranho.publica(), estranho.publica_troca(), LOJA, "Eu", &u, "");
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

// ---------------------------------------------------------------------------
// Juntar linhas vindas de fora (preparação da 5.5: comum a todas as
// alternativas da A5; usada pela replicação das etapas 4 e 7 e pela
// restauração de backup da etapa 6). Ninguém chama ainda.
// ---------------------------------------------------------------------------

/// O registro de uma loja como está gravado (linha fora do formato fica de
/// fora), para mandar a outro caixa ou guardar antes de restaurar um backup.
/// Nada aqui diz quem está autorizado: isso é sempre [`avaliar`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Linhas {
    #[serde(default)]
    pub aprovacoes: Vec<Aprovacao>,
    #[serde(default)]
    pub retiradas: Vec<Retirada>,
}

/// As linhas do registro da loja `loja_id` gravadas neste banco.
pub fn linhas(conn: &Connection, loja_id: &str) -> Resultado<Linhas> {
    Ok(Linhas { aprovacoes: ler_aprovacoes(conn, loja_id)?, retiradas: ler_retiradas(conn, loja_id)? })
}

/// O que [`juntar`] fez com cada linha que chegou.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Juntadas {
    /// Não estava aqui: gravada.
    pub novas: usize,
    /// Já estava aqui igual, ou a daqui fica pelo desempate.
    pub ja_tinha: usize,
    /// Mesma chave primária com outro conteúdo, e a que chegou fica pelo desempate.
    pub trocadas: usize,
    /// Assinatura que não confere ou de outra loja: não gravada.
    pub recusadas: usize,
    /// C2R4-1: aprovações que ficaram de fora por falta de lugar (já contadas em
    /// `recusadas`).
    pub sem_lugar: usize,
}

/// Desempate entre duas linhas válidas com a mesma chave primária (o mesmo
/// caixa aprovou ou retirou a mesma chave duas vezes): fica a de maior
/// (data declarada, assinatura). Não depende da ordem de chegada, então todos
/// os caixas guardam a mesma. Não muda quem está autorizado: a cadeia só olha
/// quem aprovou ou retirou quem.
fn fica(data: &str, assinatura: &Assinatura) -> (String, String) {
    (data.to_string(), hex(assinatura))
}

fn aprovacao_gravada(conn: &Connection, chave: &Chave, aprovador: &Chave) -> Resultado<Option<Aprovacao>> {
    let loja: Option<String> = conn
        .query_row(
            "SELECT loja_id FROM caixa_aprovado WHERE chave = ?1 AND aprovador = ?2",
            [hex(chave), hex(aprovador)],
            |l| l.get(0),
        )
        .optional()?;
    let Some(loja) = loja else { return Ok(None) };
    Ok(ler_aprovacoes(conn, &loja)?.into_iter().find(|a| a.chave == *chave && a.aprovador == *aprovador))
}

fn retirada_gravada(conn: &Connection, chave: &Chave, retirador: &Chave) -> Resultado<Option<Retirada>> {
    let loja: Option<String> = conn
        .query_row(
            "SELECT loja_id FROM caixa_retirado WHERE chave = ?1 AND retirador = ?2",
            [hex(chave), hex(retirador)],
            |l| l.get(0),
        )
        .optional()?;
    let Some(loja) = loja else { return Ok(None) };
    Ok(ler_retiradas(conn, &loja)?.into_iter().find(|r| r.chave == *chave && r.retirador == *retirador))
}

fn gravar_retirada(conn: &Connection, r: &Retirada) -> Resultado<()> {
    conn.execute(
        "INSERT INTO caixa_retirado (chave, retirador, loja_id, motivo, gerente, retirado_em, assinatura)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT (chave, retirador) DO UPDATE SET
            loja_id = excluded.loja_id, motivo = excluded.motivo, gerente = excluded.gerente,
            retirado_em = excluded.retirado_em, assinatura = excluded.assinatura",
        params![hex(&r.chave), hex(&r.retirador), r.loja_id, r.motivo, r.gerente, r.retirado_em, hex(&r.assinatura)],
    )?;
    Ok(())
}

/// C2R2-3 e C2R4-1: quantas aprovações o registro de uma loja guarda, sem contar as do
/// primeiro caixa (a raiz) nem as de quem a raiz retirou. Uma loja de verdade tem
/// dezenas; o teto só barra o enchimento. O registro viaja inteiro em cada resposta do
/// protocolo 4, então o pior caso precisa ficar pequeno (C2R4-1: de 10.000 linhas no
/// total para 2.000 aprovações que contam).
pub const TETO_DO_REGISTRO: usize = 2_000;

/// C2R4-1: quantos caixas que continuam na loja (não retirados) um mesmo caixa aprova.
pub const LIMITE_POR_APROVADOR: usize = 100;

/// C2R4-1: quantas aprovações feitas por caixas que já estão fora (o "apoio") uma
/// retirada leva junto para valer aqui como vale onde foi feita. Na loja de verdade,
/// uma ou duas.
pub const APOIO_MAXIMO: usize = 4;

/// Os limites da junção e da aprovação (os testes usam limites pequenos).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limites {
    /// Aprovações que contam no registro ([`TETO_DO_REGISTRO`]).
    pub teto: usize,
    /// Caixas não retirados por aprovador ([`LIMITE_POR_APROVADOR`]).
    pub por_aprovador: usize,
    /// Assinaturas de aprovações que chegam conferidas numa mensagem (o registro
    /// inteiro de um caixa honesto cabe com folga).
    pub conferencias: usize,
}

impl Limites {
    pub const PADRAO: Limites =
        Limites { teto: TETO_DO_REGISTRO, por_aprovador: LIMITE_POR_APROVADOR, conferencias: 8 * TETO_DO_REGISTRO };
}

/// C2R4-1: o lugar ocupado no registro.
struct Ocupacao {
    /// Aprovações que contam no teto: não são da raiz nem de quem a raiz retirou
    /// (direta ou indiretamente).
    no_teto: usize,
    /// Por aprovador, quantos caixas aprovados por ele não foram retirados.
    por_aprovador: HashMap<Chave, usize>,
}

fn ocupacao<'a>(
    raiz: &Chave,
    validas: impl Iterator<Item = &'a Aprovacao>,
    arestas: &HashMap<Chave, Vec<Chave>>,
    pela_raiz: &HashSet<Chave>,
    retirados: &HashSet<Chave>,
) -> Ocupacao {
    let contam = alcancados(raiz, arestas, pela_raiz);
    let mut o = Ocupacao { no_teto: 0, por_aprovador: HashMap::new() };
    for a in validas {
        if a.aprovador == a.chave {
            continue;
        }
        if a.aprovador != *raiz && contam.contains(&a.aprovador) {
            o.no_teto += 1;
        }
        if !retirados.contains(&a.chave) {
            *o.por_aprovador.entry(a.aprovador).or_default() += 1;
        }
    }
    o
}

/// C2R4-1: as retiradas válidas gravadas (assinatura confere, retirador em `cadeia`,
/// nunca a raiz).
fn retiradas_validas<'a>(
    gravadas: &'a [Retirada],
    raiz: &Chave,
    loja_id: &str,
    cadeia: &HashSet<Chave>,
) -> Vec<&'a Retirada> {
    gravadas
        .iter()
        .filter(|t| t.loja_id == loja_id && t.chave != *raiz && cadeia.contains(&t.retirador) && t.assinatura_confere())
        .collect()
}

/// C2R4-1: por que a aprovação nova não cabe (texto para a tela), ou `None` se cabe.
fn falta_de_lugar(
    conn: &Connection,
    aprovador: &Chave,
    raiz: &Chave,
    loja_id: &str,
    chave: &Chave,
    lim: Limites,
) -> Resultado<Option<String>> {
    let reg = RegistroConferido::carregar(conn, raiz, loja_id)?;
    // Aprovar de novo a mesma chave pelo mesmo caixa troca a linha: não ocupa lugar.
    if reg.validas.iter().any(|a| a.chave == *chave && a.aprovador == *aprovador) {
        return Ok(None);
    }
    let (o, retirados) = (reg.ocupacao(), &reg.avaliacao.retirados);
    if *aprovador != *raiz && o.no_teto >= lim.teto {
        return Ok(Some(TEXTO_REGISTRO_CHEIO.to_string()));
    }
    if !retirados.contains(chave) && o.por_aprovador.get(aprovador).copied().unwrap_or(0) >= lim.por_aprovador {
        return Ok(Some(format!(
            "Este caixa já aprovou {} caixas que continuam na loja. Retire em Configurações › Caixas da loja os que \
             não são mais usados, ou gere o convite em outro caixa da loja.",
            lim.por_aprovador
        )));
    }
    Ok(None)
}

/// C2R5-4 (R5 da 2C): o registro desta loja está no teto (pelo estado gravado, não por um
/// evento): os caixas, menos a raiz, não aprovam caixas novos até o lugar voltar. O
/// Início avisa enquanto for verdade.
pub fn registro_cheio(conn: &Connection, raiz: &Chave, loja_id: &str) -> Resultado<bool> {
    if loja_id.trim().is_empty() {
        return Ok(false);
    }
    Ok(RegistroConferido::carregar(conn, raiz, loja_id)?.cheio())
}

/// Os limites em vigor: os de [`Limites::PADRAO`]; nos testes, os que o teste escolher
/// para esta linha de execução ([`para_testes::usar_limites`]).
fn limites_vigentes() -> Limites {
    #[cfg(test)]
    if let Some(l) = para_testes::LIMITES.with(|c| c.get()) {
        return l;
    }
    Limites::PADRAO
}

/// Só nos testes: limites pequenos para o aviso do Início.
#[cfg(test)]
pub(crate) mod para_testes {
    use super::Limites;
    use std::cell::Cell;
    thread_local!(pub(super) static LIMITES: Cell<Option<Limites>> = const { Cell::new(None) });
    pub(crate) fn usar_limites(l: Limites) {
        LIMITES.with(|c| c.set(Some(l)));
    }
}

/// C2R4-1: o registro de caixas da loja não tem mais lugar para aprovações (texto para a
/// tela e para o aviso do Início).
pub const TEXTO_REGISTRO_CHEIO: &str = "O registro de caixas da loja está cheio: um caixa aprovou caixas demais \
     (pode ser um computador roubado ou com defeito). No primeiro caixa da loja, em Configurações › Caixas da loja, \
     retire o caixa que não é mais da loja (se ele já aparece como retirado, use \"Liberar o lugar\"): o lugar volta \
     sozinho. Enquanto isso, o convite gerado no primeiro caixa continua funcionando.";

/// Guarda só o que entra na cadeia da `raiz` (C2R2-3) e nunca deixa o teto barrar uma
/// retirada (C2R4-1). Veja [`juntar_na_cadeia_com_limites`].
pub fn juntar_na_cadeia(conn: &Connection, raiz: &Chave, loja_id: &str, de_fora: &Linhas) -> Resultado<Juntadas> {
    juntar_na_cadeia_com_limites(conn, raiz, loja_id, de_fora, Limites::PADRAO)
}

/// Como [`juntar`], mas só guarda a linha de quem está na cadeia da `raiz`, sem deixar o
/// registro crescer sem fim. Por mensagem (a ordem das linhas não importa; o que fica de
/// fora volta na próxima puxada e é visto de novo):
///
/// 0. **C2R4-3:** a linha igual à gravada já foi conferida quando entrou: fica como está,
///    sem conferir de novo (mensagem só com linhas iguais não custa nada).
/// 1. **A cadeia** (aprovados alguma vez, sem olhar retiradas — a regra de [`avaliar`]):
///    só se confere a aprovação cujo aprovador já está na cadeia; o lixo de fora nem é
///    conferido (C2R2-3).
/// 2. **As retiradas** de chaves conhecidas, por quem está na cadeia, entram **sem
///    teto** quando a chave retirada está viva aqui (C2R4-1: o caixa que encheu o
///    registro continua podendo ser retirado). Uma por chave. Quem retirou e só está na
///    cadeia por aprovações de caixas que já estão fora leva junto essas aprovações (o
///    apoio, até [`APOIO_MAXIMO`]), para a retirada valer aqui como vale lá.
/// 3. **As aprovações** de quem está vivo entram se couberem: no máximo
///    [`LIMITE_POR_APROVADOR`] caixas não retirados por aprovador e
///    [`TETO_DO_REGISTRO`] aprovações que contam (as da raiz não contam; as de quem a
///    raiz retirou deixam de contar: retirar pela raiz o caixa que encheu o registro
///    devolve o lugar). Primeiro as mais perto da raiz. A aprovação feita por quem já
///    está fora não vale para nada e só entra como apoio.
///
/// Enquanto o registro está no teto, o Início avisa ([`registro_cheio`], pelo estado).
pub(crate) fn juntar_na_cadeia_com_limites(
    conn: &Connection,
    raiz: &Chave,
    loja_id: &str,
    de_fora: &Linhas,
    lim: Limites,
) -> Resultado<Juntadas> {
    let gravadas_a = ler_aprovacoes(conn, loja_id)?;
    let gravadas_r = ler_retiradas(conn, loja_id)?;
    let mut total = Juntadas::default();

    // 0. C2R4-3: igual à gravada, já tinha. A mesma chave primária com outro conteúdo (o
    //    desempate de `juntar`) não faz o registro crescer e vai direto para `juntar`.
    let pk_a: HashMap<(Chave, Chave), &Aprovacao> = gravadas_a.iter().map(|a| ((a.chave, a.aprovador), a)).collect();
    let pk_r: HashMap<(Chave, Chave), &Retirada> = gravadas_r.iter().map(|t| ((t.chave, t.retirador), t)).collect();
    let (mut novas_a, mut outra_versao_a): (Vec<&Aprovacao>, Vec<Aprovacao>) = (Vec::new(), Vec::new());
    for a in &de_fora.aprovacoes {
        if a.loja_id != loja_id {
            total.recusadas += 1;
            continue;
        }
        match pk_a.get(&(a.chave, a.aprovador)) {
            Some(g) if *g == a => total.ja_tinha += 1,
            Some(_) => outra_versao_a.push(a.clone()),
            None => novas_a.push(a),
        }
    }
    let (mut novas_r, mut outra_versao_r): (Vec<&Retirada>, Vec<Retirada>) = (Vec::new(), Vec::new());
    for t in &de_fora.retiradas {
        if t.loja_id != loja_id {
            total.recusadas += 1;
            continue;
        }
        match pk_r.get(&(t.chave, t.retirador)) {
            Some(g) if *g == t => total.ja_tinha += 1,
            Some(_) => outra_versao_r.push(t.clone()),
            None => novas_r.push(t),
        }
    }
    if novas_a.is_empty() && novas_r.is_empty() && outra_versao_a.is_empty() && outra_versao_r.is_empty() {
        return Ok(total);
    }

    // O grafo das gravadas: aprovador → aprovado (cada assinatura conferida uma vez).
    let validas_a: Vec<&Aprovacao> =
        gravadas_a.iter().filter(|a| a.loja_id == loja_id && a.assinatura_confere()).collect();
    let mut arestas: HashMap<Chave, Vec<Chave>> = HashMap::new();
    for a in &validas_a {
        if a.aprovador != a.chave {
            arestas.entry(a.aprovador).or_default().push(a.chave);
        }
    }

    // 1. A cadeia com as aprovações que chegam (primeiro as gravadas de cada chave).
    let mut chegando: HashMap<Chave, Vec<&Aprovacao>> = HashMap::new();
    for a in novas_a {
        chegando.entry(a.aprovador).or_default().push(a);
    }
    for lista in chegando.values_mut() {
        lista.sort_by(|x, y| (x.aprovado_em.as_str(), x.chave).cmp(&(y.aprovado_em.as_str(), y.chave)));
    }
    let mut conferidas: HashMap<Chave, Vec<&Aprovacao>> = HashMap::new();
    let mut cadeia: HashSet<Chave> = HashSet::from([*raiz]);
    let mut fila = VecDeque::from([*raiz]);
    let mut orcamento = lim.conferencias;
    while let Some(k) = fila.pop_front() {
        for p in arestas.get(&k).into_iter().flatten() {
            if cadeia.insert(*p) {
                fila.push_back(*p);
            }
        }
        for a in chegando.remove(&k).unwrap_or_default() {
            if orcamento == 0 {
                // C2R5-4: orçamento de conferências, não falta de lugar.
                total.recusadas += 1;
                continue;
            }
            orcamento -= 1;
            // Aprovar a si mesmo só vale para a raiz.
            if (a.aprovador == a.chave && a.chave != *raiz) || !a.assinatura_confere() {
                total.recusadas += 1;
                continue;
            }
            if a.chave != a.aprovador && cadeia.insert(a.chave) {
                fila.push_back(a.chave);
            }
            conferidas.entry(k).or_default().push(a);
        }
    }
    // Aprovador fora da cadeia: lixo, nem conferido.
    total.recusadas += chegando.values().map(Vec::len).sum::<usize>();

    // 2a. As retiradas que chegam e podem valer: chave conhecida, retirador na cadeia, nunca
    //     a raiz; uma por chave retirada. C2R5-1: a da raiz não disputa a vaga com as outras
    //     (ela devolve o lugar mesmo quando a chave já foi retirada por outro). C2R5-5: as
    //     assinaturas conferidas aqui e nas outras versões têm orçamento próprio, e a linha
    //     repetida na mesma mensagem nem é conferida.
    let mut conhecidas: HashSet<Chave> = validas_a.iter().map(|a| a.chave).collect();
    conhecidas.extend(conferidas.values().flatten().map(|a| a.chave));
    novas_r.sort_by(|x, y| {
        (x.retirado_em.as_str(), x.retirador, x.chave).cmp(&(y.retirado_em.as_str(), y.retirador, y.chave))
    });
    let mut orcamento_r = lim.conferencias;
    let mut conferir = |assinatura_confere: &dyn Fn() -> bool, total: &mut Juntadas| -> bool {
        if orcamento_r == 0 {
            total.recusadas += 1;
            return false;
        }
        orcamento_r -= 1;
        let ok = assinatura_confere();
        if !ok {
            total.recusadas += 1;
        }
        ok
    };
    let mut candidatas: Vec<&Retirada> = Vec::new();
    let mut alvos: HashSet<(Chave, bool)> = HashSet::new();
    let mut vistas: HashSet<(Chave, Chave, Assinatura)> = HashSet::new();
    for t in novas_r {
        let da_raiz = t.retirador == *raiz;
        let pode = t.chave != *raiz
            && vistas.insert((t.chave, t.retirador, t.assinatura))
            && !alvos.contains(&(t.chave, da_raiz))
            && conhecidas.contains(&t.chave)
            && cadeia.contains(&t.retirador);
        if !pode {
            total.recusadas += 1;
            continue;
        }
        if conferir(&|| t.assinatura_confere(), &mut total) {
            alvos.insert((t.chave, da_raiz));
            candidatas.push(t);
        }
    }
    // As outras versões das linhas gravadas (o desempate de `juntar`) também gastam o
    // orçamento; a assinatura errada fica de fora já aqui.
    let mut vistas_a: HashSet<(Chave, Chave, Assinatura)> = HashSet::new();
    outra_versao_a.retain(|a| {
        vistas_a.insert((a.chave, a.aprovador, a.assinatura)) && conferir(&|| a.assinatura_confere(), &mut total)
    });
    outra_versao_r.retain(|t| {
        vistas.insert((t.chave, t.retirador, t.assinatura)) && conferir(&|| t.assinatura_confere(), &mut total)
    });

    // 3. As aprovações de quem está vivo (sem as retiradas gravadas nem as candidatas),
    //    das mais perto da raiz para as mais longe, enquanto couberem.
    let validas_r = retiradas_validas(&gravadas_r, raiz, loja_id, &cadeia);
    let retirados_gravados: HashSet<Chave> = validas_r.iter().map(|t| t.chave).collect();
    let mut fora = retirados_gravados.clone();
    fora.extend(candidatas.iter().map(|t| t.chave));
    let pela_raiz: HashSet<Chave> =
        validas_r.iter().chain(candidatas.iter()).filter(|t| t.retirador == *raiz).map(|t| t.chave).collect();
    let mut ocup = ocupacao(raiz, validas_a.iter().copied(), &arestas, &pela_raiz, &fora);
    let mut aceitas: Vec<Aprovacao> = Vec::new();
    let mut aceitas_pk: HashSet<(Chave, Chave)> = HashSet::new();
    let mut arestas_ok = arestas.clone();
    let mut vivos: HashSet<Chave> = HashSet::from([*raiz]);
    let mut fila = VecDeque::from([*raiz]);
    while let Some(k) = fila.pop_front() {
        for p in arestas.get(&k).into_iter().flatten() {
            if !fora.contains(p) && vivos.insert(*p) {
                fila.push_back(*p);
            }
        }
        for a in conferidas.remove(&k).unwrap_or_default() {
            let pk = (a.chave, a.aprovador);
            if !aceitas_pk.contains(&pk) {
                let no_teto = a.aprovador != *raiz;
                let no_aprovador = a.chave != a.aprovador && !fora.contains(&a.chave);
                let cheio = (no_teto && ocup.no_teto >= lim.teto)
                    || (no_aprovador && ocup.por_aprovador.get(&k).copied().unwrap_or(0) >= lim.por_aprovador);
                if cheio {
                    total.recusadas += 1;
                    total.sem_lugar += 1;
                    continue;
                }
                ocup.no_teto += usize::from(no_teto);
                *ocup.por_aprovador.entry(k).or_default() += usize::from(no_aprovador);
                aceitas_pk.insert(pk);
            }
            if a.chave != a.aprovador {
                arestas_ok.entry(k).or_default().push(a.chave);
                if !fora.contains(&a.chave) && vivos.insert(a.chave) {
                    fila.push_back(a.chave);
                }
            }
            aceitas.push(a.clone());
        }
    }
    // O que sobrou é de quem está fora (ou de quem só ficaria vivo por uma aprovação que
    // não coube): só pode entrar como apoio de uma retirada.
    let mut sobras: HashMap<Chave, Vec<&Aprovacao>> = HashMap::new();
    for (k, lista) in conferidas {
        sobras.entry(k).or_default().extend(lista);
    }
    let quantas_sobras: usize = sobras.values().map(Vec::len).sum();

    // 2b. As retiradas: só da chave viva aqui (gravadas e aceitas, com as retiradas
    //     gravadas), menos a da raiz, que vale sempre (C2R5-1); sem teto.
    let vivos_antes = alcancados(raiz, &arestas_ok, &retirados_gravados);
    let (mut aceitas_r, nao_vivas): (Vec<&Retirada>, Vec<&Retirada>) =
        candidatas.into_iter().partition(|t| t.retirador == *raiz || vivos_antes.contains(&t.chave));
    total.recusadas += nao_vivas.len();
    let na_cadeia = alcancados(raiz, &arestas_ok, &HashSet::new());
    let mut apoio: Vec<&Aprovacao> = Vec::new();
    for rodada in 1.. {
        let mut fora_final = retirados_gravados.clone();
        fora_final.extend(aceitas_r.iter().map(|t| t.chave));
        let vivos_final = alcancados(raiz, &arestas_ok, &fora_final);
        let pai = caminhos_de_apoio(&na_cadeia, &vivos_final, &sobras);
        let (mut ficam, mut linhas, mut sem_apoio) = (Vec::new(), Vec::new(), 0);
        for t in aceitas_r {
            match subir_pelo_apoio(&t.retirador, &na_cadeia, &pai) {
                Some(l) => {
                    linhas.extend(l);
                    ficam.push(t);
                }
                None => sem_apoio += 1,
            }
        }
        total.recusadas += sem_apoio;
        if sem_apoio == 0 {
            aceitas_r = ficam;
            apoio = linhas;
            break;
        }
        // Sem apoio, a retirada fica para a próxima puxada; tirá-la muda quem fica vivo,
        // então o apoio das outras é visto de novo. Depois de poucas rodadas, ficam só as
        // que não precisam de apoio.
        if rodada >= 4 {
            let (sem, com): (Vec<&Retirada>, Vec<&Retirada>) =
                ficam.into_iter().partition(|t| na_cadeia.contains(&t.retirador));
            total.recusadas += com.len();
            aceitas_r = sem;
            break;
        }
        aceitas_r = ficam;
    }
    let mut aprovacoes = aceitas;
    let mut ja_vai: HashSet<(Chave, Chave)> = aceitas_pk;
    let mut apoio_aceito = 0;
    for a in apoio {
        if ja_vai.insert((a.chave, a.aprovador)) {
            aprovacoes.push(a.clone());
            apoio_aceito += 1;
        }
    }
    // As de quem está fora que não viraram apoio ficam de fora (voltam na próxima puxada).
    total.recusadas += quantas_sobras - apoio_aceito;
    aprovacoes.extend(outra_versao_a);
    let mut retiradas: Vec<Retirada> = aceitas_r.into_iter().cloned().collect();
    retiradas.extend(outra_versao_r);
    let j = juntar(conn, loja_id, &Linhas { aprovacoes, retiradas })?;
    total.novas += j.novas;
    total.trocadas += j.trocadas;
    total.ja_tinha += j.ja_tinha;
    total.recusadas += j.recusadas;
    Ok(total)
}

/// C2R4-1: os caminhos de apoio, a partir da cadeia guardada, só por aprovações de quem
/// fica fora (`vivos` não aprova por apoio: o apoio nunca deixa ninguém vivo), até
/// [`APOIO_MAXIMO`] linhas: para cada chave alcançada, a linha por onde chegou.
fn caminhos_de_apoio<'a>(
    na_cadeia: &HashSet<Chave>,
    vivos: &HashSet<Chave>,
    sobras: &HashMap<Chave, Vec<&'a Aprovacao>>,
) -> HashMap<Chave, (&'a Aprovacao, usize)> {
    let mut pai: HashMap<Chave, (&'a Aprovacao, usize)> = HashMap::new();
    let mut fontes: Vec<Chave> = sobras.keys().filter(|u| na_cadeia.contains(*u)).copied().collect();
    fontes.sort();
    let mut fila: VecDeque<(Chave, usize)> = fontes.into_iter().map(|u| (u, 0)).collect();
    while let Some((u, d)) = fila.pop_front() {
        if vivos.contains(&u) || d >= APOIO_MAXIMO {
            continue;
        }
        for a in sobras.get(&u).into_iter().flatten() {
            if a.chave == a.aprovador || na_cadeia.contains(&a.chave) || pai.contains_key(&a.chave) {
                continue;
            }
            pai.insert(a.chave, (*a, d + 1));
            fila.push_back((a.chave, d + 1));
        }
    }
    pai
}

/// C2R4-1: as linhas de apoio de `retirador` até a cadeia guardada (`None`: sem caminho).
fn subir_pelo_apoio<'a>(
    retirador: &Chave,
    na_cadeia: &HashSet<Chave>,
    pai: &HashMap<Chave, (&'a Aprovacao, usize)>,
) -> Option<Vec<&'a Aprovacao>> {
    let mut x = *retirador;
    let mut linhas = Vec::new();
    while !na_cadeia.contains(&x) {
        let (a, _) = pai.get(&x)?;
        linhas.push(*a);
        x = a.aprovador;
    }
    Some(linhas)
}

/// Junta ao registro da loja `loja_id` as linhas que vieram de fora (outro
/// caixa, um backup restaurado): **acrescenta, nunca tira**. Uma linha só é
/// gravada se a assinatura de quem a fez confere e se é desta loja; quem está
/// autorizado continua decidido por [`avaliar`] na hora da consulta (uma linha
/// bem assinada por quem não é da loja é gravada e não vale nada, como uma
/// gravada aqui). Uma linha daqui estragada (fora do formato) dá lugar à que
/// chegou. Tudo ou nada (ponto de salvamento).
pub fn juntar(conn: &Connection, loja_id: &str, de_fora: &Linhas) -> Resultado<Juntadas> {
    em_bloco(conn, || {
        // C2R3-2: as linhas gravadas desta loja, lidas uma vez (antes, uma leitura da
        // tabela inteira para cada linha que chegava).
        let mut gravadas_a: HashMap<(Chave, Chave), Aprovacao> =
            ler_aprovacoes(conn, loja_id)?.into_iter().map(|a| ((a.chave, a.aprovador), a)).collect();
        let mut gravadas_r: HashMap<(Chave, Chave), Retirada> =
            ler_retiradas(conn, loja_id)?.into_iter().map(|t| ((t.chave, t.retirador), t)).collect();
        let mut r = Juntadas::default();
        for a in &de_fora.aprovacoes {
            if a.loja_id != loja_id || !a.assinatura_confere() {
                r.recusadas += 1;
                continue;
            }
            let atual = match gravadas_a.get(&(a.chave, a.aprovador)) {
                Some(x) => Some(x.clone()),
                // Linha de outra loja (ou estragada) com a mesma chave primária: a de antes.
                None => aprovacao_gravada(conn, &a.chave, &a.aprovador)?,
            };
            match atual {
                Some(x) if x == *a => r.ja_tinha += 1,
                Some(x) if fica(&x.aprovado_em, &x.assinatura) >= fica(&a.aprovado_em, &a.assinatura) => {
                    r.ja_tinha += 1
                }
                Some(_) => {
                    gravar_aprovacao(conn, a)?;
                    gravadas_a.insert((a.chave, a.aprovador), a.clone());
                    r.trocadas += 1;
                }
                None => {
                    gravar_aprovacao(conn, a)?;
                    gravadas_a.insert((a.chave, a.aprovador), a.clone());
                    r.novas += 1;
                }
            }
        }
        for t in &de_fora.retiradas {
            if t.loja_id != loja_id || !t.assinatura_confere() {
                r.recusadas += 1;
                continue;
            }
            let atual = match gravadas_r.get(&(t.chave, t.retirador)) {
                Some(x) => Some(x.clone()),
                None => retirada_gravada(conn, &t.chave, &t.retirador)?,
            };
            match atual {
                Some(x) if x == *t => r.ja_tinha += 1,
                Some(x) if fica(&x.retirado_em, &x.assinatura) >= fica(&t.retirado_em, &t.assinatura) => {
                    r.ja_tinha += 1
                }
                Some(_) => {
                    gravar_retirada(conn, t)?;
                    gravadas_r.insert((t.chave, t.retirador), t.clone());
                    r.trocadas += 1;
                }
                None => {
                    gravar_retirada(conn, t)?;
                    gravadas_r.insert((t.chave, t.retirador), t.clone());
                    r.novas += 1;
                }
            }
        }
        Ok(r)
    })
}

#[cfg(test)]
mod testes_juntar {
    use super::*;
    use crate::banco::Banco;

    const LOJA: &str = "loja-teste-p4";

    /// Duas cópias do registro: a "daqui" com raiz, Caixa 2, Caixa 3 (aprovado
    /// pelo 2) e o 3 retirado; a "de lá" vazia, só com a mesma loja.
    fn duas_lojas() -> (Banco, Banco, String, Identidade, [Identidade; 2]) {
        let (aqui, u) = crate::faixas::para_testes::loja("Caixa 1");
        let (la, _) = crate::faixas::para_testes::loja("Caixa 9");
        let raiz = Identidade::gerar().unwrap();
        let (c2, c3) = (Identidade::gerar().unwrap(), Identidade::gerar().unwrap());
        let r = raiz.publica();
        registrar_raiz(&aqui.conn, &raiz, LOJA, "Caixa 1", &u).unwrap();
        aprovar(&aqui.conn, &raiz, &r, LOJA, &Pedido::de(&c2), "Caixa 2", &u).unwrap();
        aprovar(&aqui.conn, &c2, &r, LOJA, &Pedido::de(&c3), "Caixa 3", &u).unwrap();
        retirar(&aqui.conn, &raiz, &r, LOJA, &c3.publica(), "saiu", &u).unwrap();
        (aqui, la, u, raiz, [c2, c3])
    }

    fn quadro(b: &Banco, raiz: &Identidade, chaves: &[Chave]) -> Vec<Situacao> {
        let a = avaliar(&b.conn, &raiz.publica(), LOJA).unwrap();
        chaves.iter().map(|c| a.situacao(c)).collect()
    }

    #[test]
    fn juntar_traz_o_registro_inteiro_e_repetir_nao_muda_nada() {
        let (aqui, la, _u, raiz, [c2, c3]) = duas_lojas();
        let chaves = [raiz.publica(), c2.publica(), c3.publica()];
        let todas = linhas(&aqui.conn, LOJA).unwrap();
        assert_eq!((todas.aprovacoes.len(), todas.retiradas.len()), (3, 1));
        let j = juntar(&la.conn, LOJA, &todas).unwrap();
        assert_eq!(j, Juntadas { novas: 4, ..Juntadas::default() });
        assert_eq!(quadro(&la, &raiz, &chaves), quadro(&aqui, &raiz, &chaves));
        assert_eq!(
            quadro(&la, &raiz, &chaves),
            [Situacao::Autorizada, Situacao::Autorizada, Situacao::Retirada],
            "a retirada veio junto e vence"
        );
        let de_novo = juntar(&la.conn, LOJA, &todas).unwrap();
        assert_eq!(de_novo, Juntadas { ja_tinha: 4, ..Juntadas::default() });
        assert_eq!(linhas(&la.conn, LOJA).unwrap().aprovacoes.len(), 3);
    }

    /// Juntar nunca tira: um registro velho (backup de antes da retirada)
    /// juntado por cima do atual não traz o retirado de volta.
    #[test]
    fn juntar_nunca_tira_e_o_registro_velho_nao_ressuscita_o_retirado() {
        let (aqui, la, u, raiz, [c2, c3]) = duas_lojas();
        let r = raiz.publica();
        // "Backup de antes": o registro de lá sem a retirada.
        let velho = Linhas { retiradas: Vec::new(), ..linhas(&aqui.conn, LOJA).unwrap() };
        // Lá também retiram o Caixa 2 (depois do backup).
        juntar(&la.conn, LOJA, &linhas(&aqui.conn, LOJA).unwrap()).unwrap();
        retirar(&la.conn, &raiz, &r, LOJA, &c2.publica(), "roubado", &u).unwrap();
        let antes = linhas(&la.conn, LOJA).unwrap();
        let j = juntar(&la.conn, LOJA, &velho).unwrap();
        assert_eq!((j.novas, j.trocadas, j.recusadas), (0, 0, 0));
        assert_eq!(linhas(&la.conn, LOJA).unwrap(), antes, "nada saiu nem mudou");
        assert_eq!(quadro(&la, &raiz, &[c2.publica(), c3.publica()]), [Situacao::Retirada, Situacao::Retirada]);
    }

    #[test]
    fn linha_adulterada_ou_de_outra_loja_nao_entra_e_nao_barra_as_outras() {
        let (aqui, la, _u, raiz, [c2, c3]) = duas_lojas();
        let mut l = linhas(&aqui.conn, LOJA).unwrap();
        let i = l.aprovacoes.iter().position(|a| a.chave == c2.publica()).unwrap();
        l.aprovacoes[i].nome = "Caixa 2 do intruso".into();
        let mut outra = l.aprovacoes[0].clone();
        outra.loja_id = "outra-loja".into();
        l.aprovacoes.push(outra);
        l.retiradas[0].motivo = "mexido".into();
        let j = juntar(&la.conn, LOJA, &l).unwrap();
        assert_eq!((j.novas, j.recusadas), (2, 3), "{j:?}");
        let a = avaliar(&la.conn, &raiz.publica(), LOJA).unwrap();
        assert_eq!(a.situacao(&raiz.publica()), Situacao::Autorizada);
        assert_eq!(a.situacao(&c2.publica()), Situacao::Desconhecida, "a aprovação mexida não entrou");
        assert_eq!(a.situacao(&c3.publica()), Situacao::Desconhecida, "sem o elo do Caixa 2, a raiz não o alcança");
        let n: i64 =
            la.conn.query_row("SELECT COUNT(*) FROM caixa_aprovado WHERE loja_id <> ?1", [LOJA], |x| x.get(0)).unwrap();
        assert_eq!(n, 0);
    }

    /// Versões válidas da mesma linha (o mesmo caixa aprovou a mesma chave
    /// mais de uma vez): os caixas ficam com a mesma, em qualquer ordem de
    /// chegada. **Nenhum horário vem do relógio de verdade:** a aprovação do
    /// Caixa 2 que `duas_lojas` gravou com `tempo::agora()` fica de fora, e as
    /// três versões têm horário fixo. (Antes de 13/11 a de base vinha do relógio
    /// e, depois das 11:00 UTC daquele dia, ficava mais nova que a "v2" fixa: o
    /// teste falhava conforme a hora em que rodava.)
    #[test]
    fn mesma_linha_em_duas_versoes_fica_a_mesma_em_qualquer_ordem() {
        let (aqui, la, u, raiz, [c2, _]) = duas_lojas();
        let r = raiz.publica();
        let (outro, _) = crate::faixas::para_testes::loja("Caixa 8");
        let mut base = linhas(&aqui.conn, LOJA).unwrap();
        base.aprovacoes.retain(|a| !(a.chave == c2.publica() && a.aprovador == r));
        juntar(&la.conn, LOJA, &base).unwrap();
        juntar(&outro.conn, LOJA, &base).unwrap();
        let versao = |nome: &str, quando: &str| {
            let mut a = assinar_aprovacao(&raiz, c2.publica(), c2.publica_troca(), LOJA, nome, &u, "");
            a.aprovado_em = quando.into();
            a.assinatura = raiz.assinar(Dominio::Aprovacao, &a.corpo());
            a
        };
        let v0 = versao("Caixa 2", "2026-01-01T09:00:00Z");
        let v1 = versao("Caixa 2 (balcão)", "2026-01-01T10:00:00Z");
        let v2 = versao("Caixa 2 (fundos)", "2026-01-01T11:00:00Z");
        let so = |a: &Aprovacao| Linhas { aprovacoes: vec![a.clone()], retiradas: Vec::new() };
        // Lá chegam na ordem do tempo; no outro, a mais nova primeiro.
        for v in [&v0, &v1, &v2] {
            juntar(&la.conn, LOJA, &so(v)).unwrap();
        }
        let mut ja_tinha = 0;
        for v in [&v2, &v0, &v1] {
            ja_tinha += juntar(&outro.conn, LOJA, &so(v)).unwrap().ja_tinha;
        }
        assert_eq!(ja_tinha, 2, "a de 11:00 chegou primeiro e fica; as mais velhas não a trocam");
        let ordenadas = |b: &Banco| {
            let mut l = linhas(&b.conn, LOJA).unwrap();
            l.aprovacoes.sort_by_key(|a| (a.chave, a.aprovador));
            l.retiradas.sort_by_key(|t| (t.chave, t.retirador));
            l
        };
        assert_eq!(ordenadas(&la), ordenadas(&outro), "os dois caixas ficaram com o mesmo registro");
        let a = ordenadas(&la).aprovacoes.into_iter().find(|a| a.chave == c2.publica() && a.aprovador == r).unwrap();
        assert_eq!((a.nome.as_str(), a.aprovado_em.as_str()), ("Caixa 2 (fundos)", "2026-01-01T11:00:00Z"));
        assert!(autorizada(&la.conn, &r, LOJA, &c2.publica()).unwrap());
    }

    /// Uma linha daqui estragada (fora do formato) dá lugar à válida que chega.
    #[test]
    fn linha_estragada_daqui_da_lugar_a_valida() {
        let (aqui, la, _u, raiz, [c2, _]) = duas_lojas();
        let base = linhas(&aqui.conn, LOJA).unwrap();
        juntar(&la.conn, LOJA, &base).unwrap();
        la.conn.execute("UPDATE caixa_aprovado SET assinatura = 'zz' WHERE chave = ?1", [hex(&c2.publica())]).unwrap();
        assert_eq!(quadro(&la, &raiz, &[c2.publica()]), [Situacao::Desconhecida]);
        let j = juntar(&la.conn, LOJA, &base).unwrap();
        assert_eq!(j.novas, 1, "{j:?}");
        assert_eq!(quadro(&la, &raiz, &[c2.publica()]), [Situacao::Autorizada]);
    }
}
