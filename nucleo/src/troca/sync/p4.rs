//! Protocolo 4 no motor da troca (0.9.19-dev.2, T008; `roadmap.md` §5.6).
//!
//! - O "4" é o **fio** (DT-P4-25): a conexão TLS 1.3 com as chaves cruas de cada
//!   caixa (`protocolo4::canal`). As mensagens da troca dentro dele seguem o
//!   formato de hoje, mais o recurso [`RECURSO_CHAVES`] e o campo `registro` (as
//!   linhas assinadas de caixas aprovados e retirados, que só viajam por aqui).
//! - O contexto ([`contexto`], DT-P4-26) é montado por conexão a partir do banco
//!   (loja, raiz fixada, registro) e da identidade **deste processo** (aberta uma
//!   vez, `cofre::identidade_do_processo`). Sem ele, o caixa ainda não fala o P4.
//! - A raiz da loja ([`garantir_raiz`], DT-P4-10) é conferida na puxada, de minuto
//!   em minuto, nunca a cada conexão (grava `config`).

use super::*;
use crate::troca::protocolo4::{aprovados, canal, cofre, hex, Chave};

/// Recurso da troca: este caixa fala o protocolo 4 (o fio autenticado).
pub const RECURSO_CHAVES: &str = "chaves-v1";

/// Só nos testes: quantas conexões o servidor atendeu pelo fio do protocolo 4
/// (prova de que a troca passou mesmo pelo canal autenticado).
#[cfg(test)]
pub(super) static ATENDIDAS_PELO_P4: AtomicUsize = AtomicUsize::new(0);

/// Prazo do aperto de mão TLS de uma conexão da troca.
pub(super) const PRAZO_DO_APERTO: Duration = Duration::from_secs(5);

/// O modo deste processo (o Rustinho portátil ou a instalação).
pub(super) fn modo() -> cofre::Modo {
    if crate::portatil::ligado() {
        cofre::Modo::portatil_deste_computador()
    } else {
        cofre::Modo::Instalado
    }
}

/// O que este caixa precisa para falar o protocolo 4 numa conexão.
pub(super) struct ContextoP4 {
    pub(super) identidade: Arc<cofre::IdentidadeLocal>,
    pub(super) raiz: Chave,
    pub(super) loja: String,
}

impl ContextoP4 {
    /// A chave pública deste caixa.
    pub(super) fn eu(&self) -> Chave {
        self.identidade.identidade.publica()
    }

    /// As chaves autorizadas agora nesta loja, sem a deste caixa (com quem ele fala).
    pub(super) fn outras_autorizadas(&self, conn: &Connection) -> Resultado<std::collections::HashSet<Chave>> {
        let av = aprovados::avaliar(conn, &self.raiz, &self.loja)?;
        let mut outras = av.autorizadas().clone();
        outras.remove(&self.eu());
        Ok(outras)
    }

    /// A chave autorizada do caixa `dispositivo` (I3), se houver.
    pub(super) fn chave_de(&self, conn: &Connection, dispositivo: &str) -> Resultado<Option<Chave>> {
        aprovados::chave_do_dispositivo(conn, &self.raiz, &self.loja, dispositivo)
    }
}

/// O contexto P4 deste caixa, ou `None` quando ele ainda não fala o P4: sem loja,
/// sem raiz fixada, identidade indisponível agora (o motivo vai para o registro
/// técnico; nunca gera identidade nova sozinho) ou a própria chave não autorizada.
/// A raiz é conferida antes da identidade: um caixa sem raiz não paga o Argon2.
pub(super) fn contexto(conn: &Connection, caminho: &Path) -> Option<ContextoP4> {
    let loja = crate::loja::id(conn).ok().filter(|l| !l.is_empty())?;
    let raiz = cofre::raiz_atual(conn, &loja).ok().flatten()?;
    let pasta = caminho.parent()?;
    let identidade = match cofre::identidade_do_processo(pasta, &modo()) {
        Ok(i) => i,
        Err(e) => {
            crate::registro::anotar(&format!("troca de dados (protocolo 4): identidade indisponível agora: {e}"));
            return None;
        }
    };
    let ctx = ContextoP4 { identidade, raiz, loja };
    aprovados::autorizada(conn, &ctx.raiz, &ctx.loja, &ctx.eu()).ok().filter(|ok| *ok)?;
    Some(ctx)
}

/// DT-P4-10 (A4): o caixa que distribui as faixas de uma loja com usuários vira a
/// raiz do registro (uma vez; o pino `p4_raiz` nunca é trocado), com o gerente
/// ativo mais antigo no campo informativo. Grava a chave pública deste caixa em
/// `config` (local). Sem loja, sem usuários ou com a raiz já fixada e a chave já
/// anotada, não faz nada (nem abre a identidade).
pub(super) fn garantir_raiz(conn: &Connection, caminho: &Path) -> Resultado<()> {
    let loja = crate::loja::id(conn)?;
    if loja.is_empty() || crate::usuarios::quantidade(conn)? == 0 {
        return Ok(());
    }
    let anotada = config::ler(conn, "p4_chave_publica")?.is_some_and(|c| !c.trim().is_empty());
    if anotada && cofre::raiz_atual(conn, &loja)?.is_some() {
        return Ok(());
    }
    let Some(pasta) = caminho.parent() else { return Ok(()) };
    let id = cofre::identidade_do_processo(pasta, &modo())?;
    let gerente = crate::usuarios::listar(conn, true)?
        .into_iter()
        .filter(|u| u.gerente)
        .min_by(|a, b| a.criado_em.cmp(&b.criado_em))
        .map(|u| u.nome)
        .unwrap_or_default();
    let distribuidor = crate::faixas::sou_distribuidor(conn)?;
    cofre::garantir_raiz_se_aplicavel(
        conn,
        &id.identidade,
        id.cofre_sistema,
        &loja,
        &config::nome_caixa(conn)?,
        &gerente,
        distribuidor,
    )?;
    Ok(())
}

/// O registro de caixas desta loja, em JSON, para ir junto numa mensagem P4.
pub(super) fn registro_em_json(conn: &Connection, loja: &str) -> Resultado<Value> {
    Ok(serde_json::to_value(aprovados::linhas(conn, loja)?)?)
}

/// Junta o registro que veio numa mensagem P4 (acrescenta, nunca tira; cada linha
/// se confere sozinha). Mensagem sem registro não muda nada; registro estragado é
/// recusado inteiro.
pub(super) fn juntar_registro(conn: &Connection, loja: &str, mensagem: &Value) -> Resultado<()> {
    let Some(r) = mensagem.get("registro") else { return Ok(()) };
    let linhas: aprovados::Linhas = serde_json::from_value(r.clone())
        .map_err(|_| Erro::validacao("O registro de caixas que veio do outro caixa está estragado."))?;
    aprovados::juntar(conn, loja, &linhas)?;
    Ok(())
}

/// O fio do protocolo 4: o canal TLS autenticado. Toda mensagem que sai leva o
/// registro deste caixa e o recurso [`RECURSO_CHAVES`].
pub(super) struct FioP4<C> {
    canal: canal::CanalAutenticado<C>,
    registro: Value,
}

impl<C> FioP4<C> {
    pub(super) fn novo(canal: canal::CanalAutenticado<C>, registro: Value) -> FioP4<C> {
        FioP4 { canal, registro }
    }

    /// Troca o registro que vai nas próximas mensagens (depois de juntar o do outro).
    pub(super) fn trocar_registro(&mut self, registro: Value) {
        self.registro = registro;
    }
}

impl<C, Data> Fio for FioP4<C>
where
    Data: rustls::SideData,
    C: std::ops::DerefMut<Target = rustls::ConnectionCommon<Data>>,
{
    fn chave_do_par(&self) -> Option<Chave> {
        Some(self.canal.chave_do_par())
    }

    fn enviar(&mut self, v: &Value) -> Resultado<()> {
        let mut v = v.clone();
        if let Some(obj) = v.as_object_mut() {
            obj.insert("registro".into(), self.registro.clone());
            if let Some(Value::Array(rec)) = obj.get_mut("recursos") {
                if !rec.iter().any(|r| r.as_str() == Some(RECURSO_CHAVES)) {
                    rec.push(Value::String(RECURSO_CHAVES.into()));
                }
            }
        }
        self.canal.enviar_json(&v, Duration::from_secs(30), None)
    }

    fn receber(&mut self, limite: usize, prazo: Duration, parar: Option<&AtomicBool>) -> Resultado<Value> {
        self.canal.receber_json(limite, prazo, parar)
    }

    fn origem(&self) -> Option<SocketAddr> {
        self.canal.peer_addr().ok()
    }
}

/// Texto para a tela: o caixa do outro lado não é o que diz ser (I3).
pub(super) const TEXTO_CAIXA_NAO_CONFERE: &str =
    "O outro computador disse ser um caixa desta loja que não confere com a identidade dele. A troca foi recusada.";

/// Anota a recusa de um caixa cuja identidade não confere com o `dispositivo`
/// declarado (DT-P4-14).
pub(super) fn anotar_recusa_de_identidade(dispositivo: &str, chave: &Chave) {
    crate::registro::anotar(&format!(
        "troca de dados (protocolo 4): recusado o caixa que declarou o dispositivo {dispositivo:?} com a chave {}",
        hex(chave)
    ));
}
