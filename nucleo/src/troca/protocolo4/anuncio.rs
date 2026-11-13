//! Anúncio assinado do protocolo 4 — etapa 3 (R-01).
//!
//! O anúncio do protocolo 3 sai pela rede a cada 10 s **cifrado com a chave
//! tirada da senha da rede** (Argon2id), com o sal em claro: quem grava um
//! anúncio testa senhas no próprio computador, sem limite (R-01). O anúncio
//! daqui não tem **nada** que venha da senha da rede: só
//! `{loja_id, chave pública, porta, momento, nonce}` em claro, assinados com a
//! chave Ed25519 do caixa. Um anúncio gravado não serve para testar senha
//! nenhuma (não há o que abrir), e ninguém anuncia um caixa sem a chave
//! privada dele.
//!
//! Contra repetição: o momento tem de estar a até [`JANELA_MS`] do relógio de
//! quem recebe, e cada (chave, nonce) só é aceito uma vez enquanto está na
//! janela ([`Repeticoes`]). Repetir um anúncio velho não faz nada; e o
//! anúncio só diz por onde achar o caixa: quem ele diz ser é provado de novo
//! no canal autenticado (etapa 4).
//!
//! **Não está ligado à rede.** O `Motor` continua mandando o anúncio do
//! protocolo 3: trocar exige o canal autenticado (decisão A5) e a convivência
//! com os caixas da 0.9.17 (decisão A4). Enquanto o anúncio antigo sair, o
//! R-01 continua aberto; estas funções ficam prontas para a etapa que ligar.
//!
//! Formato (versão 1, no máximo 256 bytes):
//! `RSA4` | versão (1) | momento em ms, com sinal (8) | nonce (16) |
//! chave Ed25519 (32) | porta (2) | tamanho da loja (1) | loja (1 a 128,
//! UTF-8) | assinatura Ed25519 (64) sobre tudo o que vem antes.

use std::collections::{HashMap, VecDeque};

use super::{verificar, Assinatura, Chave, Dominio, Identidade};
use crate::erro::{Erro, Resultado};

/// Começo de todo anúncio assinado (diferente do `RST3` do protocolo 3).
pub const MAGICA_ANUNCIO: &[u8; 4] = b"RSA4";
const VERSAO: u8 = 1;
/// Distância máxima entre o momento do anúncio e o relógio de quem recebe.
/// Folgada de propósito: relógios de caixas da mesma loja costumam diferir
/// alguns minutos (o aviso de relógio do protocolo 3 é de 2 min).
pub const JANELA_MS: i64 = 10 * 60 * 1000;
/// Tamanho máximo da identidade da loja dentro do anúncio.
pub const LOJA_MAXIMA: usize = 128;
/// Anúncios lembrados por caixa (um a cada 10 s dá 120 em 20 min).
const LEMBRADOS_POR_CAIXA: usize = 512;
/// Caixas lembrados ao mesmo tempo.
const CAIXAS_LEMBRADOS: usize = 1024;

/// Começo (4) + versão (1) + momento (8) + nonce (16) + chave (32) + porta
/// (2) + tamanho da loja (1).
const FIXO: usize = 4 + 1 + 8 + 16 + 32 + 2 + 1;

/// Um anúncio que passou na conferência.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anuncio {
    pub loja_id: String,
    pub chave: Chave,
    pub porta: u16,
    pub momento_ms: i64,
    pub nonce: [u8; 16],
}

/// Por que um anúncio foi recusado (para o registro; nada disso vira aviso
/// na tela sozinho).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recusa {
    /// Não é um anúncio assinado desta versão (ou está cortado/sobrando).
    Formato,
    /// O momento está longe demais do relógio deste caixa.
    ForaDaJanela,
    /// A assinatura não confere com a chave anunciada.
    Assinatura,
    /// É de outra loja.
    OutraLoja,
    /// A chave não está autorizada nesta loja.
    NaoAutorizada,
    /// Este mesmo anúncio já foi aceito (repetição).
    Repetido,
    /// Lembrança cheia: na dúvida, recusa.
    Excesso,
}

/// Monta o anúncio deste caixa. Não recebe a senha da rede nem nada tirado
/// dela: o resultado depende só da chave, da loja, da porta, do momento e de
/// um nonce sorteado.
pub fn montar(id: &Identidade, loja_id: &str, porta: u16, agora_ms: i64) -> Resultado<Vec<u8>> {
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|e| Erro::Interno(format!("sem números aleatórios: {e}")))?;
    montar_com(id, loja_id, porta, agora_ms, nonce)
}

/// [`montar`] com o nonce dado (os testes conferem que o anúncio é o mesmo
/// com qualquer senha da rede).
pub(crate) fn montar_com(
    id: &Identidade,
    loja_id: &str,
    porta: u16,
    momento_ms: i64,
    nonce: [u8; 16],
) -> Resultado<Vec<u8>> {
    if loja_id.is_empty() || loja_id.len() > LOJA_MAXIMA {
        return Err(Erro::Interno(format!(
            "identidade da loja com tamanho inválido para o anúncio ({})",
            loja_id.len()
        )));
    }
    if porta == 0 {
        return Err(Erro::Interno("porta 0 no anúncio".into()));
    }
    let mut v = Vec::with_capacity(FIXO + loja_id.len() + 64);
    v.extend_from_slice(MAGICA_ANUNCIO);
    v.push(VERSAO);
    v.extend_from_slice(&momento_ms.to_be_bytes());
    v.extend_from_slice(&nonce);
    v.extend_from_slice(&id.publica());
    v.extend_from_slice(&porta.to_be_bytes());
    v.push(loja_id.len() as u8);
    v.extend_from_slice(loja_id.as_bytes());
    let assinatura = id.assinar(Dominio::Anuncio, &v);
    v.extend_from_slice(&assinatura);
    Ok(v)
}

/// Lê e confere um anúncio: formato, janela de tempo e assinatura. Não olha
/// loja, autorização nem repetição (veja [`verificar_anuncio`]).
pub fn abrir(bytes: &[u8], agora_ms: i64) -> Result<Anuncio, Recusa> {
    if bytes.len() < FIXO + 1 + 64 || bytes[..4] != MAGICA_ANUNCIO[..] || bytes[4] != VERSAO {
        return Err(Recusa::Formato);
    }
    let tamanho = usize::from(bytes[FIXO - 1]);
    if tamanho == 0 || tamanho > LOJA_MAXIMA || bytes.len() != FIXO + tamanho + 64 {
        return Err(Recusa::Formato);
    }
    let momento_ms = i64::from_be_bytes(bytes[5..13].try_into().map_err(|_| Recusa::Formato)?);
    let nonce: [u8; 16] = bytes[13..29].try_into().map_err(|_| Recusa::Formato)?;
    let chave: Chave = bytes[29..61].try_into().map_err(|_| Recusa::Formato)?;
    let porta = u16::from_be_bytes([bytes[61], bytes[62]]);
    let loja_id = std::str::from_utf8(&bytes[FIXO..FIXO + tamanho]).map_err(|_| Recusa::Formato)?;
    if porta == 0 {
        return Err(Recusa::Formato);
    }
    // A janela antes da assinatura: a conta barata primeiro.
    if agora_ms.abs_diff(momento_ms) > JANELA_MS as u64 {
        return Err(Recusa::ForaDaJanela);
    }
    let (assinado, assinatura) = bytes.split_at(FIXO + tamanho);
    let assinatura: Assinatura = assinatura.try_into().map_err(|_| Recusa::Formato)?;
    if !verificar(Dominio::Anuncio, &chave, assinado, &assinatura) {
        return Err(Recusa::Assinatura);
    }
    Ok(Anuncio { loja_id: loja_id.to_string(), chave, porta, momento_ms, nonce })
}

/// Os anúncios já aceitos, para recusar repetição enquanto estão na janela.
#[derive(Debug, Default)]
pub struct Repeticoes {
    vistos: HashMap<Chave, VecDeque<(i64, [u8; 16])>>,
}

impl Repeticoes {
    pub fn nova() -> Repeticoes {
        Repeticoes::default()
    }

    /// `Ok` na primeira vez que este anúncio aparece (e passa a lembrá-lo);
    /// `Err(Repetido)` se já foi aceito; `Err(Excesso)` se não há onde lembrar.
    /// `agora_ms` tem de ser o mesmo relógio usado em [`abrir`].
    pub fn primeira_vez(&mut self, a: &Anuncio, agora_ms: i64) -> Result<(), Recusa> {
        // Esquece o que já saiu da janela (não seria aceito de novo mesmo).
        let vencido = |m: i64| agora_ms.saturating_sub(m) > JANELA_MS;
        self.vistos.retain(|_, fila| {
            fila.retain(|(m, _)| !vencido(*m));
            !fila.is_empty()
        });
        if !self.vistos.contains_key(&a.chave) && self.vistos.len() >= CAIXAS_LEMBRADOS {
            return Err(Recusa::Excesso);
        }
        let fila = self.vistos.entry(a.chave).or_default();
        if fila.iter().any(|(m, n)| *m == a.momento_ms && *n == a.nonce) {
            return Err(Recusa::Repetido);
        }
        if fila.len() >= LEMBRADOS_POR_CAIXA {
            return Err(Recusa::Excesso);
        }
        fila.push_back((a.momento_ms, a.nonce));
        Ok(())
    }
}

/// A conferência completa, na ordem segura: formato, janela, assinatura,
/// loja, autorização (`autorizada`, por exemplo [`super::aprovados::Avaliacao`])
/// e, só então, repetição — assim quem não é da loja nem ocupa a lembrança.
pub fn verificar_anuncio(
    bytes: &[u8],
    loja_esperada: &str,
    agora_ms: i64,
    repeticoes: &mut Repeticoes,
    autorizada: impl Fn(&Chave) -> bool,
) -> Result<Anuncio, Recusa> {
    let a = abrir(bytes, agora_ms)?;
    if a.loja_id != loja_esperada {
        return Err(Recusa::OutraLoja);
    }
    if !autorizada(&a.chave) {
        return Err(Recusa::NaoAutorizada);
    }
    repeticoes.primeira_vez(&a, agora_ms)?;
    Ok(a)
}

#[cfg(test)]
mod testes {
    use super::super::aprovados::{self, Pedido, Situacao};
    use super::*;

    const AGORA: i64 = 1_793_000_000_000;
    const LOJA: &str = "0199aaaa-bbbb-7ccc-8ddd-eeeeeeeeeeee";

    #[test]
    fn anuncio_assinado_ida_e_volta() {
        let id = Identidade::gerar().unwrap();
        let bytes = montar(&id, LOJA, 47810, AGORA).unwrap();
        assert!(bytes.len() <= 256);
        assert_eq!(&bytes[..4], MAGICA_ANUNCIO);
        let a = abrir(&bytes, AGORA + 5_000).unwrap();
        assert_eq!((a.loja_id.as_str(), a.chave, a.porta, a.momento_ms), (LOJA, id.publica(), 47810, AGORA));
        // Dois anúncios do mesmo caixa no mesmo instante: nonces diferentes.
        let outro = abrir(&montar(&id, LOJA, 47810, AGORA).unwrap(), AGORA).unwrap();
        assert_ne!(a.nonce, outro.nonce);
        // Entradas inválidas para montar.
        assert!(montar(&id, "", 47810, AGORA).is_err());
        assert!(montar(&id, &"x".repeat(LOJA_MAXIMA + 1), 47810, AGORA).is_err());
        assert!(montar(&id, LOJA, 0, AGORA).is_err());
        assert!(abrir(&montar(&id, &"x".repeat(LOJA_MAXIMA), 1, AGORA).unwrap(), AGORA).is_ok());
    }

    /// Ninguém anuncia um caixa sem a chave privada dele, e nenhum byte do
    /// anúncio pode ser mudado no caminho.
    #[test]
    fn anuncio_falsificado_ou_adulterado_e_recusado() {
        let caixa = Identidade::gerar().unwrap();
        let intruso = Identidade::gerar().unwrap();
        let bom = montar(&caixa, LOJA, 47810, AGORA).unwrap();
        // Cada byte mudado: recusado (formato, janela ou assinatura).
        for i in 0..bom.len() {
            let mut b = bom.clone();
            b[i] ^= 0x20;
            assert!(abrir(&b, AGORA).is_err(), "byte {i} mudado foi aceito");
        }
        // O intruso põe a chave do caixa no anúncio dele: a assinatura é dele.
        let mut forjado = montar(&intruso, LOJA, 47999, AGORA).unwrap();
        forjado[29..61].copy_from_slice(&caixa.publica());
        assert_eq!(abrir(&forjado, AGORA), Err(Recusa::Assinatura));
        // Trocar a porta do anúncio do caixa (desviar para outro lugar): recusado.
        let mut desviado = bom.clone();
        desviado[61..63].copy_from_slice(&47999u16.to_be_bytes());
        assert_eq!(abrir(&desviado, AGORA), Err(Recusa::Assinatura));
        // Cortado, sobrando, vazio, ou do protocolo 3: formato.
        assert_eq!(abrir(&bom[..bom.len() - 1], AGORA), Err(Recusa::Formato));
        assert_eq!(abrir(&[bom.as_slice(), &[0]].concat(), AGORA), Err(Recusa::Formato));
        assert_eq!(abrir(&[], AGORA), Err(Recusa::Formato));
        let mut p3 = bom.clone();
        p3[..4].copy_from_slice(b"RST3");
        assert_eq!(abrir(&p3, AGORA), Err(Recusa::Formato));
        // Uma assinatura de outro contexto (uma aprovação) não vale como anúncio.
        let mut cruzado = bom[..bom.len() - 64].to_vec();
        let s = caixa.assinar(Dominio::Aprovacao, &cruzado);
        cruzado.extend_from_slice(&s);
        assert_eq!(abrir(&cruzado, AGORA), Err(Recusa::Assinatura));
    }

    #[test]
    fn anuncio_velho_ou_repetido_e_recusado() {
        let caixa = Identidade::gerar().unwrap();
        let bytes = montar(&caixa, LOJA, 47810, AGORA).unwrap();
        // Fora da janela, nos dois sentidos (relógio adiantado ou atrasado).
        assert_eq!(abrir(&bytes, AGORA + JANELA_MS + 1), Err(Recusa::ForaDaJanela));
        assert_eq!(abrir(&bytes, AGORA - JANELA_MS - 1), Err(Recusa::ForaDaJanela));
        assert!(abrir(&bytes, AGORA + JANELA_MS).is_ok(), "no limite ainda vale");
        assert!(abrir(&bytes, AGORA - JANELA_MS).is_ok());
        // Momentos extremos não estouram a conta.
        let extremo = montar_com(&caixa, LOJA, 47810, i64::MIN, [7; 16]).unwrap();
        assert_eq!(abrir(&extremo, i64::MAX), Err(Recusa::ForaDaJanela));
        // Dentro da janela: a primeira vez passa, a repetição não.
        let mut r = Repeticoes::nova();
        let todos = |_: &Chave| true;
        assert!(verificar_anuncio(&bytes, LOJA, AGORA, &mut r, todos).is_ok());
        assert_eq!(verificar_anuncio(&bytes, LOJA, AGORA + 1_000, &mut r, todos), Err(Recusa::Repetido));
        // O anúncio seguinte do mesmo caixa passa.
        let seguinte = montar(&caixa, LOJA, 47810, AGORA + 10_000).unwrap();
        assert!(verificar_anuncio(&seguinte, LOJA, AGORA + 10_000, &mut r, todos).is_ok());
        // Passada a janela, a lembrança é esquecida e a repetição já cai no tempo.
        assert_eq!(verificar_anuncio(&bytes, LOJA, AGORA + JANELA_MS + 1, &mut r, todos), Err(Recusa::ForaDaJanela));
        // Lembrança cheia de um caixa: recusa, não esquece calado.
        let mut cheia = Repeticoes::nova();
        for i in 0..LEMBRADOS_POR_CAIXA as i64 {
            let a = Anuncio {
                loja_id: LOJA.into(),
                chave: caixa.publica(),
                porta: 1,
                momento_ms: AGORA + i,
                nonce: [1; 16],
            };
            cheia.primeira_vez(&a, AGORA).unwrap();
        }
        let mais =
            Anuncio { loja_id: LOJA.into(), chave: caixa.publica(), porta: 1, momento_ms: AGORA, nonce: [2; 16] };
        assert_eq!(cheia.primeira_vez(&mais, AGORA), Err(Recusa::Excesso));
    }

    /// A conferência completa com o registro da etapa 2: só caixa autorizado
    /// da mesma loja; retirado e desconhecido ficam fora, e quem é recusado
    /// não ocupa a lembrança de repetição.
    #[test]
    fn anuncio_so_de_caixa_autorizado_da_loja() {
        let (b, u) = crate::faixas::para_testes::loja("Caixa 1");
        let raiz = Identidade::gerar().unwrap();
        let r = raiz.publica();
        aprovados::registrar_raiz(&b.conn, &raiz, LOJA, "Caixa 1", &u).unwrap();
        let c2 = Identidade::gerar().unwrap();
        aprovados::aprovar(&b.conn, &raiz, &r, LOJA, &Pedido::de(&c2), "Caixa 2", &u).unwrap();
        let estranho = Identidade::gerar().unwrap();
        let mut rep = Repeticoes::nova();
        let conferir = |bytes: &[u8], rep: &mut Repeticoes| {
            let av = aprovados::avaliar(&b.conn, &r, LOJA).unwrap();
            verificar_anuncio(bytes, LOJA, AGORA, rep, |k| av.situacao(k) == Situacao::Autorizada)
        };
        assert_eq!(conferir(&montar(&c2, LOJA, 47810, AGORA).unwrap(), &mut rep).unwrap().chave, c2.publica());
        assert_eq!(conferir(&montar(&estranho, LOJA, 47810, AGORA).unwrap(), &mut rep), Err(Recusa::NaoAutorizada));
        assert_eq!(conferir(&montar(&c2, "outra-loja", 47810, AGORA).unwrap(), &mut rep), Err(Recusa::OutraLoja));
        aprovados::retirar(&b.conn, &raiz, &r, LOJA, &c2.publica(), "saiu", &u).unwrap();
        assert_eq!(conferir(&montar(&c2, LOJA, 47810, AGORA).unwrap(), &mut rep), Err(Recusa::NaoAutorizada));
        assert_eq!(rep.vistos.len(), 1, "só o anúncio aceito ficou lembrado");
    }
}
