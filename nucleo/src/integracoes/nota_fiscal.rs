//! A nota fiscal emitida pelo Rustinho (0.9.19, etapa 3, pacote E; FIS-P1 e FIS-P2): a máquina de estados, o painel
//! e o detalhe.
//!
//! Estados: `rascunho` → `numerada` (número reservado) → `enviada` → `autorizada` | `rejeitada` | `denegada` |
//! `desconhecida` → (`cancelada` | `inutilizada`, depois). A assinatura é do emissor (o ACBrMonitorPLUS assina com o
//! A3): o Rustinho não guarda nem vê a chave privada.
//!
//! Regras (FIS-P2 e o texto A §13 e §14):
//! - **cada passo é gravado antes de chamar o emissor** (`marcar_enviada` numa transação confirmada antes da chamada),
//!   e o resultado é gravado depois, em outra transação;
//! - uma transmissão **sem resposta conclusiva** (a conexão caiu, o tempo acabou, a resposta é de outra nota) fica
//!   `desconhecida`, separada de `rejeitada`, e **não se reenvia**: só a consulta tira a nota desse estado;
//! - a rejeitada pode ser corrigida e transmitida de novo, à mão (mesmo número);
//! - o número é por ambiente, modelo e série, e nunca se repete (índice único no banco);
//! - toda conversa com o emissor passa pela porta do FIS-LIGA (`fiscal_porta::com_adaptador`).
//!
//! Nesta etapa não há emissor de verdade (o adaptador ACBr é a T008, com os dados do proprietário): a tela só lê.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::erro::{Erro, Resultado};
use crate::fiscal_porta::{NotaParaEnviar, RespostaSefaz};
use crate::tempo;

/// O que uma resposta conclusiva da SEFAZ quer dizer (MOC 7.0: 100 autorizado; 150 autorizado fora de prazo; 110,
/// 301, 302 e 303 denegado; o resto é rejeição).
pub fn estado_da_resposta(cstat: &str) -> &'static str {
    match cstat.trim() {
        "100" | "150" => "autorizada",
        "110" | "301" | "302" | "303" => "denegada",
        _ => "rejeitada",
    }
}

fn evento(
    tx: &Connection,
    nota_id: &str,
    de: &str,
    para: &str,
    detalhe: &str,
    usuario_id: Option<&str>,
) -> Resultado<()> {
    tx.execute(
        "INSERT INTO nota_fiscal_evento (id, nota_id, quando, de, para, detalhe, usuario_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![tempo::novo_id(), nota_id, tempo::agora(), de, para, detalhe, usuario_id],
    )?;
    Ok(())
}

fn estado(tx: &Connection, id: &str) -> Resultado<String> {
    tx.query_row("SELECT estado FROM nota_fiscal WHERE id = ?1", [id], |l| l.get(0))
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Nota fiscal".into()))
}

fn mudar(tx: &Connection, id: &str, de: &str, para: &str, detalhe: &str, usuario_id: Option<&str>) -> Resultado<()> {
    tx.execute(
        "UPDATE nota_fiscal SET estado = ?2, atualizado_em = ?3 WHERE id = ?1",
        params![id, para, tempo::agora()],
    )?;
    evento(tx, id, de, para, detalhe, usuario_id)
}

/// Um rascunho de nota. `ambiente`: 1 produção, 2 homologação; `modelo`: 55 ou 65.
#[allow(clippy::too_many_arguments)]
pub fn criar_rascunho(
    tx: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    ambiente: i64,
    modelo: i64,
    serie: i64,
    venda_id: Option<&str>,
    valor: i64,
    destinatario_doc: &str,
    destinatario_nome: &str,
) -> Resultado<String> {
    if !matches!(ambiente, 1 | 2) {
        return Err(Erro::validacao("Ambiente fiscal inválido."));
    }
    if !matches!(modelo, 55 | 65) {
        return Err(Erro::validacao("Modelo de nota inválido (55 ou 65)."));
    }
    if !(0..=999).contains(&serie) {
        return Err(Erro::validacao("Série inválida (0 a 999)."));
    }
    let id = tempo::novo_id();
    let agora = tempo::agora();
    tx.execute(
        "INSERT INTO nota_fiscal (id, ambiente, modelo, serie, numero, estado, venda_id, destinatario_doc,
                                  destinatario_nome, valor, criado_em, atualizado_em, dispositivo)
         VALUES (?1, ?2, ?3, ?4, 0, 'rascunho', ?5, ?6, ?7, ?8, ?9, ?9, ?10)",
        params![id, ambiente, modelo, serie, venda_id, destinatario_doc, destinatario_nome, valor, agora, dispositivo],
    )?;
    evento(tx, &id, "", "rascunho", "Nota criada", Some(usuario_id))?;
    Ok(id)
}

/// Reserva o próximo número da série (por ambiente e modelo). Só do rascunho.
pub fn reservar_numero(tx: &Connection, usuario_id: &str, id: &str) -> Resultado<i64> {
    let e = estado(tx, id)?;
    if e != "rascunho" {
        return Err(Erro::validacao("Só o rascunho recebe número."));
    }
    let numero: i64 = tx.query_row(
        "SELECT COALESCE(MAX(n.numero), 0) + 1 FROM nota_fiscal n, nota_fiscal x
          WHERE x.id = ?1 AND n.ambiente = x.ambiente AND n.modelo = x.modelo AND n.serie = x.serie",
        [id],
        |l| l.get(0),
    )?;
    tx.execute("UPDATE nota_fiscal SET numero = ?2 WHERE id = ?1", params![id, numero])?;
    mudar(tx, id, &e, "numerada", &format!("Número {numero} reservado"), Some(usuario_id))?;
    Ok(numero)
}

/// Os dados que o emissor recebe.
pub fn para_enviar(tx: &Connection, id: &str) -> Resultado<NotaParaEnviar> {
    tx.query_row(
        "SELECT id, ambiente, modelo, serie, numero, chave, valor, destinatario_doc, destinatario_nome
           FROM nota_fiscal WHERE id = ?1",
        [id],
        |l| {
            Ok(NotaParaEnviar {
                id: l.get(0)?,
                ambiente: l.get(1)?,
                modelo: l.get(2)?,
                serie: l.get(3)?,
                numero: l.get(4)?,
                chave: l.get(5)?,
                valor: l.get(6)?,
                destinatario_doc: l.get(7)?,
                destinatario_nome: l.get(8)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| Erro::NaoEncontrado("Nota fiscal".into()))
}

/// Grava "enviada" **antes** de chamar o emissor. Só a numerada ou a rejeitada (corrigida, à mão) se transmitem;
/// a enviada e a desconhecida não: consulte antes.
pub fn marcar_enviada(tx: &Connection, usuario_id: &str, id: &str) -> Resultado<()> {
    let e = estado(tx, id)?;
    match e.as_str() {
        "numerada" | "rejeitada" => mudar(tx, id, &e, "enviada", "Transmissão começou", Some(usuario_id)),
        "enviada" | "desconhecida" => Err(Erro::validacao(
            "O Rustinho não sabe se a SEFAZ recebeu esta nota. Não transmita de novo: consulte na SEFAZ primeiro.",
        )),
        _ => Err(Erro::validacao(format!("Uma nota {e} não se transmite."))),
    }
}

/// Grava o resultado de uma transmissão ou de uma consulta. `resposta`: `Ok` com a resposta da SEFAZ, ou `Err` com
/// o que aconteceu (sem resposta conclusiva). A resposta de outra nota (outra chave) também é desconhecida.
pub fn registrar_resposta(
    tx: &Connection,
    usuario_id: &str,
    id: &str,
    resposta: Result<&RespostaSefaz, &str>,
) -> Resultado<String> {
    let e = estado(tx, id)?;
    let chave_antes: String = tx.query_row("SELECT chave FROM nota_fiscal WHERE id = ?1", [id], |l| l.get(0))?;
    let para = match resposta {
        Err(o_que) => {
            mudar(tx, id, &e, "desconhecida", &format!("Sem resposta conclusiva: {o_que}"), Some(usuario_id))?;
            return Ok("desconhecida".into());
        }
        Ok(r)
            if r.cstat.trim().is_empty()
                || (!chave_antes.is_empty() && !r.chave.is_empty() && r.chave != chave_antes) =>
        {
            let o_que =
                if r.cstat.trim().is_empty() { "resposta sem cStat" } else { "resposta de outra nota (outra chave)" };
            mudar(tx, id, &e, "desconhecida", &format!("Sem resposta conclusiva: {o_que}"), Some(usuario_id))?;
            return Ok("desconhecida".into());
        }
        Ok(r) => r,
    };
    let novo = estado_da_resposta(&para.cstat);
    tx.execute(
        "UPDATE nota_fiscal SET cstat = ?2, motivo = ?3, protocolo = CASE WHEN ?4 <> '' THEN ?4 ELSE protocolo END,
                                chave = CASE WHEN chave = '' THEN ?5 ELSE chave END,
                                xml_nome = CASE WHEN ?6 <> '' THEN ?6 ELSE xml_nome END
          WHERE id = ?1",
        params![id, para.cstat.trim(), para.motivo, para.protocolo, para.chave, para.xml_nome],
    )?;
    mudar(tx, id, &e, novo, &format!("{} - {}", para.cstat.trim(), para.motivo), Some(usuario_id))?;
    Ok(novo.into())
}

// ------------------------------------------------------------------ painel (FIS-P1) e detalhe (FIS-P2)

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NotaResumo {
    pub id: String,
    pub modelo: i64,
    pub serie: i64,
    pub numero: i64,
    pub estado: String,
    pub destinatario_doc: String,
    pub destinatario_nome: String,
    pub valor: i64,
    pub criado_em: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Painel {
    /// 1 = produção, 2 = homologação: sempre à vista.
    pub ambiente: i64,
    pub autorizadas: i64,
    pub rejeitadas: i64,
    pub desconhecidas: i64,
    /// Rascunho, numerada ou enviada.
    pub pendentes: i64,
    pub notas: Vec<NotaResumo>,
    pub total: i64,
}

/// O ambiente fiscal deste computador (homologação de fábrica; a troca é o FIS-P3).
pub fn ambiente(conn: &Connection) -> Resultado<i64> {
    Ok(crate::config::ler(conn, "fiscal_ambiente")?
        .and_then(|v| v.parse().ok())
        .filter(|v| matches!(v, 1 | 2))
        .unwrap_or(2))
}

/// Os filtros do painel. `indicador`: "autorizadas", "rejeitadas", "desconhecidas", "pendentes" ou vazio (todas).
#[derive(Debug, Default, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Filtros {
    pub indicador: String,
    pub texto: String,
    /// "AAAA-MM-DD"
    pub de: String,
    pub ate: String,
    /// 0 = os dois; 55 ou 65.
    pub modelo: i64,
}

fn estados_do_indicador(indicador: &str) -> &'static [&'static str] {
    match indicador {
        "autorizadas" => &["autorizada"],
        "rejeitadas" => &["rejeitada", "denegada"],
        "desconhecidas" => &["desconhecida"],
        "pendentes" => &["rascunho", "numerada", "enviada"],
        _ => &[],
    }
}

pub fn painel(conn: &Connection, f: &Filtros) -> Resultado<Painel> {
    let amb = ambiente(conn)?;
    let conta = |estados: &[&str]| -> Resultado<i64> {
        let lista = estados.iter().map(|e| format!("'{e}'")).collect::<Vec<_>>().join(", ");
        Ok(conn.query_row(
            &format!("SELECT COUNT(*) FROM nota_fiscal WHERE ambiente = ?1 AND estado IN ({lista})"),
            [amb],
            |l| l.get(0),
        )?)
    };
    let estados = estados_do_indicador(f.indicador.trim());
    let filtro_estado = if estados.is_empty() {
        String::new()
    } else {
        format!(" AND estado IN ({})", estados.iter().map(|e| format!("'{e}'")).collect::<Vec<_>>().join(", "))
    };
    let texto = format!("%{}%", f.texto.trim());
    let de = if f.de.trim().is_empty() { "0000-00-00".to_string() } else { f.de.trim().to_string() };
    let ate = if f.ate.trim().is_empty() { "9999-12-31".to_string() } else { f.ate.trim().to_string() };
    let mut c = conn.prepare(&format!(
        "SELECT id, modelo, serie, numero, estado, destinatario_doc, destinatario_nome, valor, criado_em
           FROM nota_fiscal
          WHERE ambiente = ?1{filtro_estado}
            AND (?2 = 0 OR modelo = ?2)
            AND substr(criado_em, 1, 10) BETWEEN ?3 AND ?4
            AND (CAST(numero AS TEXT) LIKE ?5 OR destinatario_nome LIKE ?5 OR destinatario_doc LIKE ?5 OR chave LIKE ?5)
          ORDER BY criado_em DESC, numero DESC"
    ))?;
    let notas: Vec<NotaResumo> = c
        .query_map(params![amb, f.modelo, de, ate, texto], |l| {
            Ok(NotaResumo {
                id: l.get(0)?,
                modelo: l.get(1)?,
                serie: l.get(2)?,
                numero: l.get(3)?,
                estado: l.get(4)?,
                destinatario_doc: l.get(5)?,
                destinatario_nome: l.get(6)?,
                valor: l.get(7)?,
                criado_em: l.get(8)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(Painel {
        ambiente: amb,
        autorizadas: conta(estados_do_indicador("autorizadas"))?,
        rejeitadas: conta(estados_do_indicador("rejeitadas"))?,
        desconhecidas: conta(estados_do_indicador("desconhecidas"))?,
        pendentes: conta(estados_do_indicador("pendentes"))?,
        total: notas.len() as i64,
        notas,
    })
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Evento {
    pub quando: String,
    pub de: String,
    pub para: String,
    pub detalhe: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Detalhe {
    pub nota: NotaResumo,
    pub ambiente: i64,
    pub chave: String,
    pub protocolo: String,
    /// O código e a mensagem da SEFAZ, na íntegra (para copiar).
    pub cstat: String,
    pub motivo: String,
    pub xml_nome: String,
    /// Primeiro a explicação humana (FIS-P2).
    pub explicacao: String,
    /// Só a desconhecida (e a enviada que ficou sem volta) pedem consulta; nenhuma se reenvia sozinha.
    pub pede_consulta: bool,
    pub eventos: Vec<Evento>,
}

/// A explicação humana do estado (FIS-P2: primeiro ela; o código vem depois).
pub fn explicacao(estado: &str) -> &'static str {
    match estado {
        "rascunho" => "Esta nota ainda não tem número nem foi transmitida.",
        "numerada" => "O número desta nota está reservado. Ela ainda não foi transmitida.",
        "enviada" => "A transmissão começou e a resposta ainda não chegou.",
        "autorizada" => "A SEFAZ autorizou esta nota.",
        "rejeitada" => "A SEFAZ recusou esta nota. Veja o motivo abaixo, corrija e transmita de novo.",
        "denegada" => "A SEFAZ denegou esta nota (problema no cadastro do emitente ou do destinatário). O número não pode ser usado de novo.",
        "desconhecida" => "O Rustinho não sabe se a SEFAZ recebeu esta nota: a conexão caiu ou a resposta não veio. Não transmita de novo: consulte na SEFAZ primeiro.",
        "cancelada" => "Esta nota foi cancelada.",
        "inutilizada" => "Este número foi inutilizado.",
        _ => "",
    }
}

pub fn detalhe(conn: &Connection, id: &str) -> Resultado<Detalhe> {
    let (nota, ambiente, chave, protocolo, cstat, motivo, xml_nome) = conn
        .query_row(
            "SELECT id, modelo, serie, numero, estado, destinatario_doc, destinatario_nome, valor, criado_em,
                    ambiente, chave, protocolo, cstat, motivo, xml_nome
               FROM nota_fiscal WHERE id = ?1",
            [id],
            |l| {
                Ok((
                    NotaResumo {
                        id: l.get(0)?,
                        modelo: l.get(1)?,
                        serie: l.get(2)?,
                        numero: l.get(3)?,
                        estado: l.get(4)?,
                        destinatario_doc: l.get(5)?,
                        destinatario_nome: l.get(6)?,
                        valor: l.get(7)?,
                        criado_em: l.get(8)?,
                    },
                    l.get::<_, i64>(9)?,
                    l.get::<_, String>(10)?,
                    l.get::<_, String>(11)?,
                    l.get::<_, String>(12)?,
                    l.get::<_, String>(13)?,
                    l.get::<_, String>(14)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Nota fiscal".into()))?;
    let mut c = conn.prepare(
        "SELECT quando, de, para, detalhe FROM nota_fiscal_evento WHERE nota_id = ?1 ORDER BY quando, rowid",
    )?;
    let eventos = c
        .query_map([id], |l| Ok(Evento { quando: l.get(0)?, de: l.get(1)?, para: l.get(2)?, detalhe: l.get(3)? }))?
        .collect::<Result<_, _>>()?;
    let pede_consulta = matches!(nota.estado.as_str(), "desconhecida" | "enviada");
    Ok(Detalhe {
        explicacao: explicacao(&nota.estado).to_string(),
        nota,
        ambiente,
        chave,
        protocolo,
        cstat,
        motivo,
        xml_nome,
        pede_consulta,
        eventos,
    })
}

// ------------------------------------------------------------- comandos das telas e a transmissão

pub use comandos::COMANDOS;

/// Os atendentes: abrem as transações (regra C5). A transmissão e a consulta ficam aqui porque gravam e confirmam
/// antes de chamar o emissor e gravam de novo depois (duas transações em volta da chamada).
pub(crate) mod comandos {
    use rusqlite::Connection;
    use serde_json::{json, Value};

    use crate::despacho::{ler, Classe, Comando, Id, Pedido};
    use crate::erro::Resultado;
    use crate::fiscal_porta::{self, Adaptador};

    pub const COMANDOS: &[Comando] = &[
        Comando { nome: "nota_painel", classe: Classe::Leitura, funcao: nota_painel },
        Comando { nome: "nota_detalhe", classe: Classe::Leitura, funcao: nota_detalhe },
    ];

    fn nota_painel(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let f: super::Filtros = ler(dados)?;
        Ok(json!({
            "painel": super::painel(p.conn, &f)?,
            "liga": fiscal_porta::estado(p.conn, p.gerente)?,
        }))
    }

    fn nota_detalhe(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: Id = ler(dados)?;
        Ok(json!(super::detalhe(p.conn, &d.id)?))
    }

    /// Transmite: grava "enviada" e confirma; chama o emissor pela porta; grava o resultado. Sem resposta
    /// conclusiva, fica "desconhecida". Devolve o estado novo.
    // Sem emissor de verdade nesta etapa (o adaptador ACBr é a T008): por enquanto só os testes chamam.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn transmitir(
        conn: &mut Connection,
        usuario_id: &str,
        adaptador: &mut dyn Adaptador,
        id: &str,
    ) -> Resultado<String> {
        let nota = {
            let tx = conn.transaction()?;
            super::marcar_enviada(&tx, usuario_id, id)?;
            let n = super::para_enviar(&tx, id)?;
            tx.commit()?;
            n
        };
        let resposta = fiscal_porta::com_adaptador(conn, adaptador, |a, l| a.enviar(l, &nota));
        let tx = conn.transaction()?;
        let novo = match &resposta {
            Ok(r) => super::registrar_resposta(&tx, usuario_id, id, Ok(r))?,
            Err(e) => super::registrar_resposta(&tx, usuario_id, id, Err(&e.to_string()))?,
        };
        tx.commit()?;
        Ok(novo)
    }

    /// Consulta a nota enviada ou desconhecida na SEFAZ e grava o que vier.
    // Sem emissor de verdade nesta etapa (o adaptador ACBr é a T008): por enquanto só os testes chamam.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn consultar(
        conn: &mut Connection,
        usuario_id: &str,
        adaptador: &mut dyn Adaptador,
        id: &str,
    ) -> Resultado<String> {
        let nota = super::para_enviar(conn, id)?;
        let e = super::estado(conn, id)?;
        if !matches!(e.as_str(), "enviada" | "desconhecida") {
            return Err(crate::erro::Erro::validacao("Só a nota sem resposta conclusiva precisa de consulta."));
        }
        let resposta = fiscal_porta::com_adaptador(conn, adaptador, |a, l| a.consultar(l, &nota));
        let tx = conn.transaction()?;
        let novo = match &resposta {
            Ok(r) => super::registrar_resposta(&tx, usuario_id, id, Ok(r))?,
            Err(e) => super::registrar_resposta(&tx, usuario_id, id, Err(&e.to_string()))?,
        };
        tx.commit()?;
        Ok(novo)
    }
}

#[cfg(test)]
mod testes {
    use super::comandos::{consultar, transmitir};
    use super::*;
    use crate::banco::Banco;
    use crate::fiscal_porta::{Adaptador, Licenca};

    /// O emissor falso: devolve as respostas da fila, na ordem; `None` é "sem resposta" (a conexão caiu).
    struct Falso {
        respostas: Vec<Option<RespostaSefaz>>,
        enviadas: Vec<NotaParaEnviar>,
        consultas: u32,
    }

    impl Falso {
        fn com(respostas: Vec<Option<RespostaSefaz>>) -> Falso {
            Falso { respostas, enviadas: Vec::new(), consultas: 0 }
        }
        fn proxima(&mut self) -> Resultado<RespostaSefaz> {
            match self.respostas.remove(0) {
                Some(r) => Ok(r),
                None => Err(Erro::Interno("a conexão com o emissor caiu".into())),
            }
        }
    }

    impl Adaptador for Falso {
        fn status(&mut self, _l: &Licenca) -> Resultado<String> {
            Ok("OK".into())
        }
        fn enviar(&mut self, _l: &Licenca, nota: &NotaParaEnviar) -> Resultado<RespostaSefaz> {
            self.enviadas.push(nota.clone());
            self.proxima()
        }
        fn consultar(&mut self, _l: &Licenca, _nota: &NotaParaEnviar) -> Resultado<RespostaSefaz> {
            self.consultas += 1;
            self.proxima()
        }
    }

    fn sefaz(cstat: &str, motivo: &str) -> Option<RespostaSefaz> {
        Some(RespostaSefaz {
            chave: "35261111222333000181650010000000011000000019".into(),
            cstat: cstat.into(),
            motivo: motivo.into(),
            protocolo: if cstat == "100" { "135260000000001".into() } else { String::new() },
            xml_nome: "35261111222333000181650010000000011000000019-nfe.xml".into(),
        })
    }

    /// Um banco com o fiscal ligado e um gerente.
    fn banco() -> (Banco, String) {
        let b = Banco::em_memoria().unwrap();
        let u = crate::usuarios::criar(&b.conn, &b.dispositivo, "Ana", "senha-da-ana-1", true).unwrap().id;
        crate::usuarios::definir_gerente(&b.conn, &u, true).unwrap();
        crate::fiscal_porta::ligar(&b.conn, &b.dispositivo, &u, true, "senha-da-ana-1").unwrap();
        (b, u)
    }

    fn nova(b: &Banco, u: &str) -> String {
        let id = criar_rascunho(&b.conn, &b.dispositivo, u, 2, 65, 1, None, 15000, "", "").unwrap();
        reservar_numero(&b.conn, u, &id).unwrap();
        id
    }

    #[test]
    fn pacote_e_autorizada_e_rejeitada_e_denegada() {
        let (mut b, u) = banco();
        let mut falso = Falso::com(vec![
            sefaz("100", "Autorizado o uso da NF-e"),
            sefaz("225", "Rejeição: Falha no Schema XML do lote de NFe"),
        ]);
        let a = nova(&b, &u);
        assert_eq!(transmitir(&mut b.conn, &u, &mut falso, &a).unwrap(), "autorizada");
        let r = nova(&b, &u);
        assert_eq!(transmitir(&mut b.conn, &u, &mut falso, &r).unwrap(), "rejeitada");
        let d = detalhe(&b.conn, &r).unwrap();
        assert_eq!(
            (d.cstat.as_str(), d.motivo.as_str()),
            ("225", "Rejeição: Falha no Schema XML do lote de NFe"),
            "na íntegra"
        );
        assert!(d.explicacao.contains("recusou"));
        assert!(!d.pede_consulta);
        assert_eq!(estado_da_resposta("150"), "autorizada");
        assert_eq!(estado_da_resposta("302"), "denegada");
        // A rejeitada se transmite de novo, à mão, com o mesmo número.
        let mut falso2 = Falso::com(vec![sefaz("100", "Autorizado o uso da NF-e")]);
        assert_eq!(transmitir(&mut b.conn, &u, &mut falso2, &r).unwrap(), "autorizada");
        assert_eq!(falso2.enviadas[0].numero, 2);
    }

    /// FIS-P2: sem resposta conclusiva não é rejeição, e não se reenvia; só a consulta resolve.
    #[test]
    fn pacote_e_sem_resposta_fica_desconhecida_e_nao_reenvia() {
        let (mut b, u) = banco();
        let id = nova(&b, &u);
        let mut falso = Falso::com(vec![None]);
        assert_eq!(transmitir(&mut b.conn, &u, &mut falso, &id).unwrap(), "desconhecida");
        let d = detalhe(&b.conn, &id).unwrap();
        assert_eq!(d.nota.estado, "desconhecida");
        assert!(d.pede_consulta);
        assert!(d.explicacao.contains("Não transmita de novo"));
        // Transmitir de novo é recusado, e o emissor não é chamado.
        let mut outro = Falso::com(vec![sefaz("100", "Autorizado o uso da NF-e")]);
        assert!(transmitir(&mut b.conn, &u, &mut outro, &id).is_err());
        assert!(outro.enviadas.is_empty());
        // A consulta resolve.
        assert_eq!(consultar(&mut b.conn, &u, &mut outro, &id).unwrap(), "autorizada");
        assert_eq!(outro.consultas, 1);
    }

    /// A resposta de outra nota (outra chave) não vale: fica desconhecida.
    #[test]
    fn pacote_e_resposta_de_outra_nota_fica_desconhecida() {
        let (mut b, u) = banco();
        let id = nova(&b, &u);
        let mut falso = Falso::com(vec![None, sefaz("100", "Autorizado")]);
        transmitir(&mut b.conn, &u, &mut falso, &id).unwrap();
        b.conn.execute("UPDATE nota_fiscal SET chave = 'chave-desta' WHERE id = ?1", [&id]).unwrap();
        assert_eq!(consultar(&mut b.conn, &u, &mut falso, &id).unwrap(), "desconhecida");
    }

    /// "Enviada" é gravada antes de chamar o emissor: se o programa cair durante a chamada, a nota não volta a
    /// "numerada" (e não se transmite de novo às cegas).
    #[test]
    fn pacote_e_enviada_e_gravada_antes_da_chamada() {
        struct Espiao {
            visto: Option<String>,
            caminho: std::path::PathBuf,
        }
        impl Adaptador for Espiao {
            fn status(&mut self, _l: &Licenca) -> Resultado<String> {
                Ok("OK".into())
            }
            fn enviar(&mut self, _l: &Licenca, nota: &NotaParaEnviar) -> Resultado<RespostaSefaz> {
                // Outra conexão, como outro processo: o estado já tem de estar confirmado no disco.
                let c = Connection::open(&self.caminho).unwrap();
                self.visto = c.query_row("SELECT estado FROM nota_fiscal WHERE id = ?1", [&nota.id], |l| l.get(0)).ok();
                Err(Erro::Interno("caiu".into()))
            }
            fn consultar(&mut self, _l: &Licenca, _nota: &NotaParaEnviar) -> Resultado<RespostaSefaz> {
                unreachable!()
            }
        }
        let pasta = std::env::temp_dir().join(format!("rustinho-nota-{}", tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let caminho = pasta.join("rustinho.db");
        let mut b = Banco::abrir(&caminho).unwrap();
        let u = crate::usuarios::criar(&b.conn, &b.dispositivo, "Ana", "senha-da-ana-1", true).unwrap().id;
        crate::usuarios::definir_gerente(&b.conn, &u, true).unwrap();
        crate::fiscal_porta::ligar(&b.conn, &b.dispositivo, &u, true, "senha-da-ana-1").unwrap();
        let id = nova(&b, &u);
        let mut espiao = Espiao { visto: None, caminho: caminho.clone() };
        assert_eq!(transmitir(&mut b.conn, &u, &mut espiao, &id).unwrap(), "desconhecida");
        assert_eq!(espiao.visto.as_deref(), Some("enviada"));
        drop(b);
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// O número é por ambiente, modelo e série, e nunca se repete.
    #[test]
    fn pacote_e_numero_por_ambiente_modelo_e_serie() {
        let (b, u) = banco();
        let a = nova(&b, &u);
        let n = nova(&b, &u);
        assert_eq!(para_enviar(&b.conn, &a).unwrap().numero, 1);
        assert_eq!(para_enviar(&b.conn, &n).unwrap().numero, 2);
        let nfe = criar_rascunho(&b.conn, &b.dispositivo, &u, 2, 55, 1, None, 0, "", "").unwrap();
        assert_eq!(reservar_numero(&b.conn, &u, &nfe).unwrap(), 1, "outro modelo, outra numeração");
        let prod = criar_rascunho(&b.conn, &b.dispositivo, &u, 1, 65, 1, None, 0, "", "").unwrap();
        assert_eq!(reservar_numero(&b.conn, &u, &prod).unwrap(), 1, "outro ambiente, outra numeração");
        assert!(reservar_numero(&b.conn, &u, &a).is_err(), "só o rascunho recebe número");
        // O banco recusa o número repetido.
        assert!(b.conn.execute("UPDATE nota_fiscal SET numero = 1 WHERE id = ?1", [&n]).is_err());
    }

    /// Com o fiscal desligado, a transmissão não chama o emissor (a porta do FIS-LIGA) e a nota fica desconhecida
    /// (o "enviada" já estava gravado): o certo é consultar quando ligar.
    #[test]
    fn pacote_e_fiscal_desligado_nao_chama_o_emissor() {
        let (mut b, u) = banco();
        crate::fiscal_porta::desligar(&b.conn, &b.dispositivo, &u, true).unwrap();
        let id = nova(&b, &u);
        let mut falso = Falso::com(vec![sefaz("100", "Autorizado")]);
        assert_eq!(transmitir(&mut b.conn, &u, &mut falso, &id).unwrap(), "desconhecida");
        assert!(falso.enviadas.is_empty());
    }

    /// FIS-P1: os indicadores filtram, a pesquisa e o período também, e o ambiente vem junto.
    #[test]
    fn pacote_e_painel_filtra() {
        let (mut b, u) = banco();
        let mut falso = Falso::com(vec![sefaz("100", "Autorizado"), sefaz("225", "Rejeição"), None]);
        for _ in 0..3 {
            let id = nova(&b, &u);
            transmitir(&mut b.conn, &u, &mut falso, &id).unwrap();
        }
        nova(&b, &u);
        let p = painel(&b.conn, &Filtros::default()).unwrap();
        assert_eq!((p.autorizadas, p.rejeitadas, p.desconhecidas, p.pendentes, p.total), (1, 1, 1, 1, 4));
        assert_eq!(p.ambiente, 2, "homologação de fábrica");
        let so = painel(&b.conn, &Filtros { indicador: "desconhecidas".into(), ..Default::default() }).unwrap();
        assert_eq!(so.notas.len(), 1);
        assert_eq!(so.notas[0].estado, "desconhecida");
        let numero = painel(&b.conn, &Filtros { texto: "4".into(), ..Default::default() }).unwrap();
        assert_eq!(numero.notas.len(), 1);
        let nada =
            painel(&b.conn, &Filtros { de: "2000-01-01".into(), ate: "2000-01-02".into(), ..Default::default() })
                .unwrap();
        assert!(nada.notas.is_empty());
        let nfe = painel(&b.conn, &Filtros { modelo: 55, ..Default::default() }).unwrap();
        assert!(nfe.notas.is_empty());
    }
}
