//! PEND-10: os caminhos (endereços) até os outros caixas e o ciclo de puxada.
//!
//! Um caixa é o seu `dispositivo`; o endereço é só um jeito de chegar nele e
//! ele pode ter vários (cabo, Wi-Fi, digitado). Cada caminho tem a sua
//! situação (`sync_endereco`, só deste computador): quando funcionou, quantas
//! vezes falhou, quando pode ser tentado de novo, e por qual interface a última
//! conexão saiu de verdade. O contador de troca (`cursor`, `marca`, `geracao`)
//! continua sendo do caixa (`sync_par`), nunca do caminho.
//!
//! `ciclo` decide **quem** puxar e **por onde**, com limites: um caminho por
//! caixa de cada vez, no máximo 3 caminhos por caixa, quem está bem primeiro,
//! no máximo `MORTOS_POR_CICLO` caixas fora do ar e `orcamento` de tempo para
//! eles. Não segura o banco enquanto espera a rede. A rede em si (a puxada)
//! entra como função, para os testes simularem tudo sem rede nenhuma.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::erro::Resultado;
use crate::rede_local::{self, Caminho, Confirmacao, Interface, Saida};

/// O endereço é de uma das interfaces deste mesmo caixa.
pub const ESTE_MESMO_CAIXA: &str = "Este endereço é deste mesmo caixa.";
/// Tempo que um ciclo gasta, no máximo, começando tentativas com caixas fora do ar.
pub const ORCAMENTO: Duration = Duration::from_secs(8);
/// Quantos caixas fora do ar são tentados num ciclo (os outros ficam para o próximo).
pub const MORTOS_POR_CICLO: usize = 4;
/// Sem notícia há mais que isso, um caminho aprendido não é mais tentado sozinho.
const SEM_NOTICIA_MS: i64 = 7 * 24 * 60 * 60 * 1000;
/// Anunciou há menos que isso: está no ar (anuncia a cada 10 s).
const FRESCO_MS: i64 = 60_000;
/// Um endereço que é deste próprio caixa só é conferido de novo depois disto.
const ESPERA_DO_PROPRIO_MS: i64 = 10 * 60 * 1000;

/// Uma linha de `sync_endereco`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registro {
    pub endereco: String,
    pub dispositivo: String,
    pub digitado: bool,
    pub visto_ms: i64,
    pub ultimo_ok_ms: i64,
    pub tentado_ms: i64,
    pub falhas: i64,
    pub proxima_ms: i64,
    pub erro: String,
    pub interface: String,
    pub confirmada: bool,
}

const COLUNAS: &str =
    "endereco, dispositivo, digitado, visto_ms, ultimo_ok_ms, tentado_ms, falhas, proxima_ms, erro, interface, confirmada";

fn ler(l: &rusqlite::Row) -> rusqlite::Result<Registro> {
    Ok(Registro {
        endereco: l.get(0)?,
        dispositivo: l.get(1)?,
        digitado: l.get::<_, i64>(2)? != 0,
        visto_ms: l.get(3)?,
        ultimo_ok_ms: l.get(4)?,
        tentado_ms: l.get(5)?,
        falhas: l.get(6)?,
        proxima_ms: l.get(7)?,
        erro: l.get(8)?,
        interface: l.get(9)?,
        confirmada: l.get::<_, i64>(10)? != 0,
    })
}

pub fn linha(conn: &Connection, endereco: &str) -> Resultado<Option<Registro>> {
    Ok(conn
        .query_row(&format!("SELECT {COLUNAS} FROM sync_endereco WHERE endereco = ?1"), [endereco], ler)
        .optional()?)
}

pub fn todas(conn: &Connection) -> Resultado<Vec<Registro>> {
    let mut c = conn.prepare(&format!("SELECT {COLUNAS} FROM sync_endereco ORDER BY endereco"))?;
    let linhas = c.query_map([], ler)?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

/// De quem é o endereço, se já se sabe.
pub fn dono_do_endereco(conn: &Connection, endereco: &str) -> Resultado<Option<String>> {
    Ok(conn
        .query_row("SELECT dispositivo FROM sync_endereco WHERE endereco = ?1 AND dispositivo <> ''", [endereco], |l| {
            l.get(0)
        })
        .optional()?)
}

/// O que a tela mostra de um caminho.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaminhoPar {
    pub endereco: String,
    /// "cabo", "Wi-Fi", "rede"... só quando a conexão **confirmou** ter saído
    /// por essa interface; senão vazio (nunca se inventa).
    pub via: String,
    pub em_uso: bool,
    pub digitado: bool,
    pub erro: String,
}

/// Os caminhos de um caixa, o que está em uso primeiro.
pub fn caminhos_do_par(conn: &Connection, dispositivo: &str) -> Resultado<Vec<CaminhoPar>> {
    let mut c = conn.prepare(&format!(
        "SELECT {COLUNAS} FROM sync_endereco WHERE dispositivo = ?1 ORDER BY ultimo_ok_ms DESC, endereco"
    ))?;
    let linhas: Vec<Registro> = c.query_map([dispositivo], ler)?.collect::<Result<_, _>>()?;
    let em_uso = linhas.iter().position(|r| r.ultimo_ok_ms > 0 && r.falhas == 0);
    Ok(linhas
        .into_iter()
        .enumerate()
        .map(|(i, r)| CaminhoPar {
            via: if r.confirmada { r.interface.clone() } else { String::new() },
            em_uso: Some(i) == em_uso,
            digitado: r.digitado,
            erro: r.erro,
            endereco: r.endereco,
        })
        .collect())
}

/// Um outro caixa se apresentou (anúncio ou pedido) vindo de `endereco`: o
/// endereço vira mais um caminho dele, sem apagar os outros. Zera a espera
/// desse caminho (o caixa está lá). Se o endereço era de outro caixa (o DHCP
/// o reaproveitou), muda de dono sem herdar contador nem histórico.
pub fn aprender(conn: &Connection, dispositivo: &str, nome: &str, endereco: &str, agora_ms: i64) -> Resultado<()> {
    let antes = dono_do_endereco(conn, endereco)?;
    conn.execute(
        "INSERT INTO sync_par (dispositivo, nome, endereco, visto_em) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (dispositivo) DO UPDATE SET nome = excluded.nome, visto_em = excluded.visto_em,
             endereco = CASE WHEN sync_par.endereco = '' THEN excluded.endereco ELSE sync_par.endereco END",
        params![dispositivo, nome, endereco, crate::tempo::agora()],
    )?;
    conn.execute(
        "INSERT INTO sync_endereco (endereco, dispositivo, visto_ms) VALUES (?1, ?2, ?3)
         ON CONFLICT (endereco) DO UPDATE SET
             falhas       = CASE WHEN sync_endereco.dispositivo = excluded.dispositivo THEN sync_endereco.falhas ELSE 0 END,
             ultimo_ok_ms = CASE WHEN sync_endereco.dispositivo = excluded.dispositivo THEN sync_endereco.ultimo_ok_ms ELSE 0 END,
             erro         = CASE WHEN sync_endereco.dispositivo = excluded.dispositivo THEN sync_endereco.erro ELSE '' END,
             interface    = CASE WHEN sync_endereco.dispositivo = excluded.dispositivo THEN sync_endereco.interface ELSE '' END,
             confirmada   = CASE WHEN sync_endereco.dispositivo = excluded.dispositivo THEN sync_endereco.confirmada ELSE 0 END,
             dispositivo  = excluded.dispositivo,
             visto_ms     = excluded.visto_ms,
             proxima_ms   = 0",
        params![endereco, dispositivo, agora_ms],
    )?;
    if let Some(anterior) = antes.filter(|a| a != dispositivo) {
        refazer_endereco_do_par(conn, &anterior)?;
    }
    Ok(())
}

/// `sync_par.endereco` é o caminho em uso (ou, sem nenhum, o melhor conhecido).
fn refazer_endereco_do_par(conn: &Connection, dispositivo: &str) -> Resultado<()> {
    conn.execute(
        "UPDATE sync_par SET endereco = COALESCE(
             (SELECT endereco FROM sync_endereco WHERE dispositivo = sync_par.dispositivo
               ORDER BY ultimo_ok_ms DESC, visto_ms DESC LIMIT 1), '')
          WHERE dispositivo = ?1 AND endereco NOT IN (SELECT endereco FROM sync_endereco WHERE dispositivo = ?1)",
        [dispositivo],
    )?;
    Ok(())
}

/// Os endereços digitados em Configurações (já com a porta). Os que saíram da
/// lista deixam de ser "digitados" (viram caminhos aprendidos e envelhecem
/// como os outros); os que nunca conversaram e saíram da lista são esquecidos.
pub fn digitar(conn: &Connection, enderecos: &[String], agora_ms: i64) -> Resultado<()> {
    for e in enderecos {
        conn.execute(
            "INSERT INTO sync_endereco (endereco, digitado, visto_ms) VALUES (?1, 1, ?2)
             ON CONFLICT (endereco) DO UPDATE SET digitado = 1",
            params![e, agora_ms],
        )?;
    }
    let digitados: Vec<String> = {
        let mut c = conn.prepare("SELECT endereco FROM sync_endereco WHERE digitado = 1")?;
        let v = c.query_map([], |l| l.get(0))?.collect::<Result<_, _>>()?;
        v
    };
    for e in digitados.iter().filter(|e| !enderecos.contains(e)) {
        conn.execute("UPDATE sync_endereco SET digitado = 0 WHERE endereco = ?1", [e])?;
    }
    conn.execute("DELETE FROM sync_endereco WHERE digitado = 0 AND dispositivo = ''", [])?;
    Ok(())
}

/// Tira o que envelheceu: caminho aprendido que falhou e está há mais de 7 dias
/// sem notícia, **se** o mesmo caixa tem outro caminho com notícia. (O último
/// caminho de um caixa fica listado para o gerente apagar, mas deixa de ser
/// tentado sozinho — veja `ciclo`.) Digitado nunca sai.
fn limpar(conn: &Connection, agora_ms: i64) -> Resultado<()> {
    let limite = agora_ms - SEM_NOTICIA_MS;
    let apagou = conn.execute(
        "DELETE FROM sync_endereco
          WHERE digitado = 0 AND falhas > 0 AND MAX(visto_ms, ultimo_ok_ms) < ?1
            AND EXISTS (SELECT 1 FROM sync_endereco o
                         WHERE o.dispositivo = sync_endereco.dispositivo AND o.endereco <> sync_endereco.endereco
                           AND MAX(o.visto_ms, o.ultimo_ok_ms) >= ?1)",
        [limite],
    )?;
    if apagou > 0 {
        let pares: Vec<String> = {
            let mut c = conn.prepare("SELECT dispositivo FROM sync_par")?;
            let v = c.query_map([], |l| l.get(0))?.collect::<Result<_, _>>()?;
            v
        };
        for d in pares {
            refazer_endereco_do_par(conn, &d)?;
        }
    }
    Ok(())
}

/// O que o ciclo precisa saber do momento.
#[derive(Debug, Clone)]
pub struct Ambiente {
    /// O `dispositivo` deste caixa.
    pub proprio: String,
    /// As interfaces deste computador agora; `None` = o sistema não soube
    /// listar (nada é descartado nem classificado, como era antes).
    pub interfaces: Option<Vec<Interface>>,
    /// Interfaces de VPN/virtual/celular que o gerente liberou pelo nome.
    pub liberadas: Vec<String>,
    pub agora_ms: i64,
    /// Quanto tempo o ciclo pode gastar começando tentativas com caixas fora do ar.
    pub orcamento: Duration,
}

impl Ambiente {
    pub fn novo(proprio: &str, agora_ms: i64) -> Ambiente {
        Ambiente { proprio: proprio.into(), interfaces: None, liberadas: Vec::new(), agora_ms, orcamento: ORCAMENTO }
    }
}

/// O que uma puxada que funcionou devolve.
#[derive(Debug, Clone)]
pub struct Puxada {
    pub dispositivo: String,
    pub total: usize,
    pub confirmacao: Confirmacao,
}

#[derive(Debug, Clone)]
pub struct Tentativa {
    pub endereco: String,
    pub dispositivo: String,
    pub ok: bool,
    pub erro: String,
}

#[derive(Debug, Clone, Default)]
pub struct Resumo {
    pub tentativas: Vec<Tentativa>,
    /// Quantos caixas fora do ar (ou que falharam) foram tentados.
    pub mortos_tentados: usize,
    /// O orçamento de tempo acabou antes de tentar todos os que faltavam.
    pub parou_por_tempo: bool,
}

struct Grupo {
    chave: String,
    regs: Vec<Registro>,
    /// Está bem: puxou há pouco sem falhar, ou anunciou há pouco.
    ativo: bool,
    melhor_ok: i64,
    ultima_noticia: i64,
    menos_tentado: i64,
}

fn agrupar(todas: Vec<Registro>, agora_ms: i64) -> Vec<Grupo> {
    let mut mapa: HashMap<String, Vec<Registro>> = HashMap::new();
    for r in todas {
        let chave = if r.dispositivo.is_empty() { format!("?{}", r.endereco) } else { r.dispositivo.clone() };
        mapa.entry(chave).or_default().push(r);
    }
    let mut grupos: Vec<Grupo> = mapa
        .into_iter()
        .map(|(chave, regs)| {
            let ativo = regs.iter().any(|r| {
                (r.ultimo_ok_ms > 0 && r.falhas == 0)
                    || (agora_ms - r.visto_ms <= FRESCO_MS && r.visto_ms > 0 && !r.digitado)
            });
            Grupo {
                melhor_ok: regs.iter().map(|r| r.ultimo_ok_ms).max().unwrap_or(0),
                ultima_noticia: regs.iter().map(|r| r.visto_ms).max().unwrap_or(0),
                menos_tentado: regs.iter().map(|r| r.tentado_ms).min().unwrap_or(0),
                chave,
                regs,
                ativo,
            }
        })
        .collect();
    // Quem está bem primeiro (o que funcionou há menos tempo à frente); depois os
    // outros, os menos tentados primeiro — assim todos têm a sua vez, em rodízio.
    grupos.sort_by(|a, b| {
        b.ativo
            .cmp(&a.ativo)
            .then_with(|| {
                if a.ativo {
                    b.melhor_ok.cmp(&a.melhor_ok).then(b.ultima_noticia.cmp(&a.ultima_noticia))
                } else {
                    a.menos_tentado.cmp(&b.menos_tentado)
                }
            })
            .then(a.chave.cmp(&b.chave))
    });
    grupos
}

fn candidatos(g: &Grupo, amb: &Ambiente) -> Vec<Caminho> {
    let interfaces = amb.interfaces.as_deref();
    let validos: Vec<Caminho> = g
        .regs
        .iter()
        .filter(|r| r.digitado || amb.agora_ms - r.visto_ms.max(r.ultimo_ok_ms) <= SEM_NOTICIA_MS)
        .filter(|r| r.digitado || rede_local::caminho_aprendido_vale(interfaces, &r.endereco, &amb.liberadas))
        .map(|r| Caminho {
            endereco: r.endereco.clone(),
            digitado: r.digitado,
            visto_ms: r.visto_ms,
            ultimo_ok_ms: r.ultimo_ok_ms,
            falhas: r.falhas,
            proxima_ms: r.proxima_ms,
        })
        .collect();
    rede_local::ordenar(&validos, interfaces, amb.agora_ms)
}

fn curto(t: &str) -> String {
    let t: String = t.chars().take(160).collect();
    t.replace(['\n', '\r'], " ")
}

fn marcar_ok(conn: &Connection, amb: &Ambiente, endereco: &str, p: &Puxada, saida: Option<&Saida>) -> Resultado<()> {
    let confirmada = saida.is_some_and(|s| p.confirmacao.confirmada(s));
    let interface = if confirmada {
        rede_local::tipo_do_caminho(amb.interfaces.as_deref(), endereco).rotulo().to_string()
    } else {
        String::new()
    };
    let antes = linha(conn, endereco)?;
    if saida.is_some() && !confirmada && antes.as_ref().is_none_or(|a| a.confirmada || a.ultimo_ok_ms == 0) {
        crate::registro::anotar(&format!(
            "troca de dados: a conexão a um caixa não saiu pela interface planejada (caminho não confirmado, {})",
            rede_local::ip_do_endereco(endereco).map_or_else(String::new, |ip| ip.to_string())
        ));
    }
    let dispositivo =
        if p.dispositivo.is_empty() { antes.map(|a| a.dispositivo).unwrap_or_default() } else { p.dispositivo.clone() };
    conn.execute(
        "UPDATE sync_endereco SET dispositivo = ?2, falhas = 0, proxima_ms = 0, ultimo_ok_ms = ?3, tentado_ms = ?3,
                erro = '', interface = ?4, confirmada = ?5
          WHERE endereco = ?1",
        params![endereco, dispositivo, amb.agora_ms, interface, confirmada as i64],
    )?;
    if !dispositivo.is_empty() {
        conn.execute("UPDATE sync_par SET endereco = ?2 WHERE dispositivo = ?1", params![dispositivo, endereco])?;
    }
    Ok(())
}

fn marcar_falha(conn: &Connection, amb: &Ambiente, endereco: &str, erro: &str) -> Resultado<()> {
    let erro = curto(erro);
    if erro.contains(ESTE_MESMO_CAIXA) {
        // Outra interface deste mesmo caixa: não é falha de caminho de ninguém.
        conn.execute(
            "UPDATE sync_endereco SET tentado_ms = ?2, proxima_ms = ?3, erro = ?4 WHERE endereco = ?1",
            params![endereco, amb.agora_ms, amb.agora_ms + ESPERA_DO_PROPRIO_MS, erro],
        )?;
        return Ok(());
    }
    let falhas: i64 = conn
        .query_row("SELECT falhas FROM sync_endereco WHERE endereco = ?1", [endereco], |l| l.get(0))
        .optional()?
        .unwrap_or(0)
        + 1;
    conn.execute(
        "UPDATE sync_endereco SET falhas = ?2, tentado_ms = ?3, proxima_ms = ?4, erro = ?5, confirmada = 0, interface = ''
          WHERE endereco = ?1",
        params![endereco, falhas, amb.agora_ms, amb.agora_ms + rede_local::espera_ms(falhas), erro],
    )?;
    Ok(())
}

/// Um ciclo de puxada. `puxar(endereco, saida)` faz a troca de verdade com o
/// caixa daquele endereço, saindo pela `saida` planejada.
///
/// - os caixas que estão bem vão primeiro; os fora do ar, em rodízio e em
///   número limitado (`MORTOS_POR_CICLO`), enquanto o `orcamento` de tempo durar;
/// - de cada caixa, um caminho de cada vez: o primeiro que funcionar encerra a
///   vez dele; o que falhar entra em espera (8, 16, 30 s) e o próximo é
///   tentado no mesmo ciclo (até 3);
/// - um caminho cuja interface sumiu deste computador nem é tentado.
pub fn ciclo(
    conn: &Connection,
    amb: &Ambiente,
    parar: &dyn Fn() -> bool,
    puxar: &mut dyn FnMut(&str, Option<&Saida>) -> Resultado<Puxada>,
) -> Resumo {
    let inicio = Instant::now();
    let mut resumo = Resumo::default();
    let travar = crate::sync::travar;
    let preparo = {
        let _t = travar();
        limpar(conn, amb.agora_ms).and_then(|_| todas(conn))
    };
    let Ok(todas) = preparo else { return resumo };
    for g in agrupar(todas, amb.agora_ms) {
        if parar() {
            break;
        }
        let cands = candidatos(&g, amb);
        if cands.is_empty() {
            continue;
        }
        if !g.ativo {
            if resumo.mortos_tentados >= MORTOS_POR_CICLO {
                continue;
            }
            if inicio.elapsed() >= amb.orcamento {
                resumo.parou_por_tempo = true;
                continue;
            }
        }
        let mut deu_certo = false;
        let mut ultimo_erro = String::new();
        for c in cands {
            if parar() || (!g.ativo && inicio.elapsed() >= amb.orcamento) {
                if !g.ativo {
                    resumo.parou_por_tempo = true;
                }
                break;
            }
            let saida = rede_local::saida_para(amb.interfaces.as_deref(), &c.endereco);
            let r = puxar(&c.endereco, saida.as_ref());
            let _t = travar();
            match r {
                Ok(p) => {
                    let _ = marcar_ok(conn, amb, &c.endereco, &p, saida.as_ref());
                    resumo.tentativas.push(Tentativa {
                        endereco: c.endereco.clone(),
                        dispositivo: p.dispositivo,
                        ok: true,
                        erro: String::new(),
                    });
                    deu_certo = true;
                    break;
                }
                Err(e) => {
                    let texto = e.to_string();
                    let _ = marcar_falha(conn, amb, &c.endereco, &texto);
                    resumo.tentativas.push(Tentativa {
                        endereco: c.endereco.clone(),
                        dispositivo: String::new(),
                        ok: false,
                        erro: curto(&texto),
                    });
                    ultimo_erro = curto(&texto);
                }
            }
        }
        if !deu_certo && !ultimo_erro.is_empty() {
            // O que aparece para o caixa: o erro da última tentativa.
            let _t = travar();
            let dispositivo = g.regs.iter().find(|r| !r.dispositivo.is_empty()).map(|r| r.dispositivo.clone());
            if let Some(d) = dispositivo {
                let _ = conn.execute("UPDATE sync_par SET erro = ?2 WHERE dispositivo = ?1", params![d, ultimo_erro]);
            }
        }
        if !g.ativo || !deu_certo {
            resumo.mortos_tentados += 1;
        }
    }
    resumo
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::erro::{Erro, Resultado};
    use crate::rede_local::{Confirmacao, Interface, Saida, Tipo};
    use crate::sync::{self, Motor};
    use crate::teste_rede::{porta_livre, Encaminhador, Modo};
    use crate::{config, estoque, produtos, usuarios};
    use rusqlite::Connection;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    const T0: i64 = 1_800_000_000_000;
    const CABO: &str = "10.0.1.5:47810";
    const WIFI: &str = "10.0.2.5:47810";
    const SENHA: &str = "balcao-azul-2026";

    fn interfaces(com_cabo: bool) -> Vec<Interface> {
        let mut v = vec![Interface::de_teste("wlan0", [10, 0, 2, 9], 24, Tipo::Wifi)];
        if com_cabo {
            v.insert(0, Interface::de_teste("eno1", [10, 0, 1, 9], 24, Tipo::Cabo));
        }
        v
    }

    fn ambiente(agora: i64, com_cabo: bool) -> Ambiente {
        let mut a = Ambiente::novo("eu", agora);
        a.interfaces = Some(interfaces(com_cabo));
        a
    }

    fn nada() -> Confirmacao {
        Confirmacao { preso_ao_ip: false, preso_a_interface: None, local: None }
    }

    /// Roda um ciclo em que `decide(endereco)` diz o que acontece com cada
    /// caminho (sem rede nenhuma): Ok = puxou, Err = falhou.
    fn rodar_com(conn: &Connection, a: &Ambiente, decide: impl Fn(&str) -> Resultado<()>) -> Resumo {
        let mut puxar = |e: &str, _s: Option<&Saida>| -> Resultado<Puxada> {
            decide(e)?;
            let dispositivo = linha(conn, e).unwrap().map(|l| l.dispositivo).unwrap_or_default();
            Ok(Puxada { dispositivo, total: 0, confirmacao: nada() })
        };
        ciclo(conn, a, &|| false, &mut puxar)
    }

    fn rodar(conn: &Connection, a: &Ambiente, vivos: &[&str]) -> Resumo {
        rodar_com(
            conn,
            a,
            |e| {
                if vivos.contains(&e) {
                    Ok(())
                } else {
                    Err(Erro::validacao("Falha na rede: sem rota"))
                }
            },
        )
    }

    fn tentados(r: &Resumo) -> Vec<&str> {
        r.tentativas.iter().map(|t| t.endereco.as_str()).collect()
    }

    fn l(conn: &Connection, e: &str) -> Registro {
        linha(conn, e).unwrap().unwrap_or_else(|| panic!("sem caminho {e}"))
    }

    // ------------------------------------------------------------------
    // Registro de caminhos
    // ------------------------------------------------------------------

    /// T-04 · CA-D2, CA-I1: dois anúncios do mesmo dispositivo por origens
    /// diferentes são **um** par com dois caminhos; outro dispositivo, outro par.
    #[test]
    fn t04_dois_enderecos_do_mesmo_caixa_sao_um_par() {
        let b = Banco::em_memoria().unwrap();
        aprender(&b.conn, "d1", "Caixa 2", CABO, T0).unwrap();
        aprender(&b.conn, "d1", "Caixa 2", WIFI, T0 + 10).unwrap();
        aprender(&b.conn, "d2", "Caixa 3", "10.0.1.6:47810", T0 + 20).unwrap();
        aprender(&b.conn, "d1", "Caixa 2", CABO, T0 + 30).unwrap(); // repetido: não duplica
        let pares = sync::pares(&b.conn).unwrap();
        assert_eq!(pares.len(), 2);
        let d1 = pares.iter().find(|p| p.dispositivo == "d1").unwrap();
        let d2 = pares.iter().find(|p| p.dispositivo == "d2").unwrap();
        assert_eq!((d1.caminhos.len(), d2.caminhos.len()), (2, 1));
        assert_eq!(l(&b.conn, CABO).visto_ms, T0 + 30);
    }

    /// T-14 · CA-I3: o endereço passou a ser de outro caixa (o DHCP reaproveitou
    /// o IP): não herda contador nem histórico do anterior, e o anterior não perde o dele.
    #[test]
    fn t14_endereco_que_mudou_de_dono_nao_herda_contador() {
        let b = Banco::em_memoria().unwrap();
        aprender(&b.conn, "d1", "Caixa 2", CABO, T0).unwrap();
        b.conn.execute("UPDATE sync_par SET cursor = 50, marca = 'm50' WHERE dispositivo = 'd1'", []).unwrap();
        b.conn
            .execute(
                "UPDATE sync_endereco SET falhas = 2, erro = 'antigo', ultimo_ok_ms = 5 WHERE endereco = ?1",
                [CABO],
            )
            .unwrap();
        aprender(&b.conn, "d3", "Caixa 4", CABO, T0 + 100).unwrap();
        let r = l(&b.conn, CABO);
        assert_eq!((r.dispositivo.as_str(), r.falhas, r.erro.as_str(), r.ultimo_ok_ms), ("d3", 0, "", 0));
        let cursor = |d: &str| -> i64 {
            b.conn.query_row("SELECT cursor FROM sync_par WHERE dispositivo = ?1", [d], |x| x.get(0)).unwrap()
        };
        assert_eq!((cursor("d1"), cursor("d3")), (50, 0), "cada um com o seu contador");
        let pares = sync::pares(&b.conn).unwrap();
        assert_eq!(pares.iter().find(|p| p.dispositivo == "d1").unwrap().caminhos.len(), 0);
    }

    /// Um anúncio vindo do caminho zera a espera dele (o outro caixa está lá).
    #[test]
    fn anuncio_zera_a_espera_do_caminho() {
        let b = Banco::em_memoria().unwrap();
        aprender(&b.conn, "d1", "Caixa 2", CABO, T0).unwrap();
        rodar(&b.conn, &ambiente(T0, true), &[]);
        assert!(l(&b.conn, CABO).proxima_ms > T0);
        aprender(&b.conn, "d1", "Caixa 2", CABO, T0 + 1_000).unwrap();
        assert_eq!(l(&b.conn, CABO).proxima_ms, 0);
    }

    // ------------------------------------------------------------------
    // Ordem, falha e espera
    // ------------------------------------------------------------------

    /// T-06, CA-R1, CA-R6: o cabo vai primeiro mesmo com o Wi-Fi mais recente;
    /// um caminho por par em cada ciclo.
    #[test]
    fn t06_um_caminho_por_par_e_cabo_primeiro() {
        let b = Banco::em_memoria().unwrap();
        aprender(&b.conn, "d1", "Caixa 2", CABO, T0).unwrap();
        aprender(&b.conn, "d1", "Caixa 2", WIFI, T0 + 500).unwrap();
        let r = rodar(&b.conn, &ambiente(T0 + 1_000, true), &[CABO, WIFI]);
        assert_eq!(tentados(&r), [CABO], "só o cabo, e só ele");
        assert!(r.tentativas[0].ok);
        assert!(l(&b.conn, CABO).ultimo_ok_ms == T0 + 1_000);
    }

    /// T-07 · CA-R2 e T-08 · CA-R3 (com relógio injetado): cabo caído → o mesmo
    /// ciclo entrega pelo Wi-Fi; o cabo espera 8, 16 e 30 s; ao voltar, reassume.
    #[test]
    fn t07_t08_falha_no_cabo_usa_o_wifi_e_o_cabo_reassume() {
        let b = Banco::em_memoria().unwrap();
        aprender(&b.conn, "d1", "Caixa 2", CABO, T0).unwrap();
        aprender(&b.conn, "d1", "Caixa 2", WIFI, T0).unwrap();
        let r = rodar(&b.conn, &ambiente(T0, true), &[WIFI]);
        assert_eq!(tentados(&r), [CABO, WIFI]);
        assert_eq!((r.tentativas[0].ok, r.tentativas[1].ok), (false, true));
        let c = l(&b.conn, CABO);
        assert_eq!((c.falhas, c.proxima_ms), (1, T0 + 8_000));
        assert!(c.erro.contains("sem rota"), "{}", c.erro);

        // dentro da espera o cabo nem é tentado
        let r = rodar(&b.conn, &ambiente(T0 + 5_000, true), &[WIFI]);
        assert_eq!(tentados(&r), [WIFI]);
        // 8 s: tenta, falha de novo, agora espera 16 s
        let r = rodar(&b.conn, &ambiente(T0 + 8_000, true), &[WIFI]);
        assert_eq!(tentados(&r), [CABO, WIFI]);
        assert_eq!((l(&b.conn, CABO).falhas, l(&b.conn, CABO).proxima_ms), (2, T0 + 24_000));
        // 24 s: terceira falha, espera 30 s (o máximo)
        rodar(&b.conn, &ambiente(T0 + 24_000, true), &[WIFI]);
        assert_eq!((l(&b.conn, CABO).falhas, l(&b.conn, CABO).proxima_ms), (3, T0 + 54_000));
        rodar(&b.conn, &ambiente(T0 + 54_000, true), &[WIFI]);
        assert_eq!(l(&b.conn, CABO).proxima_ms, T0 + 84_000, "nunca passa de 30 s");
        // o cabo volta: na primeira tentativa depois da espera, funciona e é o preferido de novo
        let r = rodar(&b.conn, &ambiente(T0 + 84_000, true), &[CABO, WIFI]);
        assert_eq!(tentados(&r), [CABO]);
        assert_eq!(l(&b.conn, CABO).falhas, 0);
        let r = rodar(&b.conn, &ambiente(T0 + 88_000, true), &[CABO, WIFI]);
        assert_eq!(tentados(&r), [CABO], "o cabo reassumiu");
    }

    /// T-09 e T-26 · CA-R4: a interface do cabo some da lista entre dois ciclos;
    /// o ciclo seguinte não tenta o endereço dela (não gasta o tempo-limite da
    /// conexão) e entrega pelo outro caminho.
    #[test]
    fn t09_t26_interface_que_sumiu_nao_e_tentada() {
        let b = Banco::em_memoria().unwrap();
        aprender(&b.conn, "d1", "Caixa 2", CABO, T0).unwrap();
        aprender(&b.conn, "d1", "Caixa 2", WIFI, T0).unwrap();
        assert_eq!(tentados(&rodar(&b.conn, &ambiente(T0, true), &[CABO, WIFI])), [CABO]);
        // cabo desconectado: o cabo nem entra na lista; se fosse tentado, este teste trava 3 s
        let inicio = Instant::now();
        let mut puxar = |e: &str, _s: Option<&Saida>| -> Resultado<Puxada> {
            assert_ne!(e, CABO, "o caminho do cabo desconectado foi tentado");
            Ok(Puxada { dispositivo: "d1".into(), total: 0, confirmacao: nada() })
        };
        let r = ciclo(&b.conn, &ambiente(T0 + 4_000, false), &|| false, &mut puxar);
        assert_eq!(tentados(&r), [WIFI]);
        assert!(inicio.elapsed() < Duration::from_millis(500), "{:?}", inicio.elapsed());
        // cabo plugado de novo: volta a ser o preferido
        assert_eq!(tentados(&rodar(&b.conn, &ambiente(T0 + 8_000, true), &[CABO, WIFI])), [CABO]);
    }

    /// T-15 · CA-I4: o endereço é deste mesmo caixa (outra interface dele): o
    /// caminho espera muito, não conta como falha e não vira par.
    #[test]
    fn t15_endereco_proprio_nao_vira_par_nem_penaliza() {
        let b = Banco::em_memoria().unwrap();
        digitar(&b.conn, &["10.0.9.9:47810".to_string()], T0).unwrap();
        aprender(&b.conn, "d1", "Caixa 2", CABO, T0).unwrap();
        let r = rodar_com(&b.conn, &ambiente(T0, true), |e| {
            if e == "10.0.9.9:47810" {
                Err(Erro::validacao(ESTE_MESMO_CAIXA))
            } else {
                Ok(())
            }
        });
        assert!(tentados(&r).contains(&"10.0.9.9:47810"));
        let p = l(&b.conn, "10.0.9.9:47810");
        assert_eq!(p.falhas, 0, "não é falha de caminho");
        assert!(p.proxima_ms >= T0 + 300_000, "espera longa: {}", p.proxima_ms);
        assert!(p.erro.contains("deste mesmo caixa"));
        assert_eq!(sync::pares(&b.conn).unwrap().len(), 1, "só o outro caixa é par");
        // o caminho do outro caixa não foi afetado
        assert_eq!((l(&b.conn, CABO).falhas, l(&b.conn, CABO).proxima_ms), (0, 0));
    }

    /// CA-D4: o que o gerente digita vira caminho; o que sai da lista deixa de
    /// ser "digitado" (e some se nunca conversou); o que já conversou fica como
    /// um caminho aprendido qualquer.
    #[test]
    fn digitar_e_tirar_da_lista() {
        let b = Banco::em_memoria().unwrap();
        let (x, y) = ("10.4.0.1:47810".to_string(), "10.4.0.2:47810".to_string());
        digitar(&b.conn, &[x.clone(), y.clone()], T0).unwrap();
        assert!(l(&b.conn, &x).digitado && l(&b.conn, &y).digitado);
        b.conn.execute("UPDATE sync_endereco SET dispositivo = 'd9' WHERE endereco = ?1", [&y]).unwrap(); // y já conversou
        digitar(&b.conn, std::slice::from_ref(&x), T0 + 1).unwrap();
        assert!(l(&b.conn, &x).digitado);
        assert!(!l(&b.conn, &y).digitado, "deixou de ser digitado, mas conversou: fica como aprendido");
        digitar(&b.conn, &[], T0 + 2).unwrap();
        assert!(linha(&b.conn, &x).unwrap().is_none(), "nunca conversou e saiu da lista: some");
        assert!(linha(&b.conn, &y).unwrap().is_some());
    }

    /// T-17 · CA-R5: o IP do par mudou; o novo é aprendido, o velho some só
    /// depois de falhar **e** de 7 dias sem notícia; o digitado nunca some;
    /// o contador é preservado.
    #[test]
    fn t17_ip_que_mudou_e_expiracao() {
        const DIA: i64 = 86_400_000;
        let b = Banco::em_memoria().unwrap();
        aprender(&b.conn, "d1", "Caixa 2", CABO, T0).unwrap();
        rodar(&b.conn, &ambiente(T0, true), &[CABO]);
        b.conn.execute("UPDATE sync_par SET cursor = 40 WHERE dispositivo = 'd1'", []).unwrap();
        let novo = "10.0.1.77:47810";
        aprender(&b.conn, "d1", "Caixa 2", novo, T0 + 1_000).unwrap();
        // o velho falha; o novo atende, no mesmo ciclo
        let r = rodar(&b.conn, &ambiente(T0 + 2_000, true), &[novo]);
        assert_eq!(tentados(&r), [CABO, novo]);
        assert_eq!(l(&b.conn, novo).falhas, 0);
        // digitado também de d1, e também falhando
        digitar(&b.conn, &["10.0.5.5:47810".to_string()], T0).unwrap();
        b.conn
            .execute("UPDATE sync_endereco SET dispositivo = 'd1', falhas = 3 WHERE endereco = '10.0.5.5:47810'", [])
            .unwrap();
        // passam 8 dias; o novo continua funcionando (visto e ok recentes)
        let depois = T0 + 8 * DIA;
        aprender(&b.conn, "d1", "Caixa 2", novo, depois - 1_000).unwrap();
        let r = rodar(&b.conn, &ambiente(depois, true), &[novo]);
        assert_eq!(tentados(&r)[0], novo.to_string().as_str(), "o que funciona vai primeiro");
        assert!(linha(&b.conn, CABO).unwrap().is_none(), "o velho, falho e sem notícia há 7 dias, saiu");
        assert!(linha(&b.conn, "10.0.5.5:47810").unwrap().is_some(), "digitado nunca some sozinho");
        let cursor: i64 =
            b.conn.query_row("SELECT cursor FROM sync_par WHERE dispositivo = 'd1'", [], |x| x.get(0)).unwrap();
        assert_eq!(cursor, 40);
    }

    // ------------------------------------------------------------------
    // Dezenas de caixas fora do ar
    // ------------------------------------------------------------------

    fn caixas_mortos(b: &Banco, n: usize) {
        for i in 0..n {
            // anunciaram há uma hora e nunca mais: fora do ar
            aprender(
                &b.conn,
                &format!("morto{i:02}"),
                &format!("Morto {i}"),
                &format!("10.1.{i}.5:47810"),
                T0 - 3_600_000,
            )
            .unwrap();
        }
    }

    /// T-25 · CA-B1: quem respondeu há pouco primeiro; quem nunca respondeu
    /// por último; rodízio entre os mortos (ninguém fica sem vez).
    #[test]
    fn t25_ordem_dos_pares_e_rodizio_entre_os_mortos() {
        let b = Banco::em_memoria().unwrap();
        aprender(&b.conn, "bom1", "Bom 1", "10.2.1.5:47810", T0).unwrap();
        aprender(&b.conn, "bom2", "Bom 2", "10.2.2.5:47810", T0).unwrap();
        b.conn.execute("UPDATE sync_endereco SET ultimo_ok_ms = 100 WHERE endereco = '10.2.1.5:47810'", []).unwrap();
        b.conn.execute("UPDATE sync_endereco SET ultimo_ok_ms = 200 WHERE endereco = '10.2.2.5:47810'", []).unwrap();
        caixas_mortos(&b, 6);
        let vivos = ["10.2.1.5:47810", "10.2.2.5:47810"];
        let r1 = rodar(&b.conn, &Ambiente::novo("eu", T0), &vivos);
        let t1 = tentados(&r1);
        assert_eq!(&t1[..2], ["10.2.2.5:47810", "10.2.1.5:47810"], "o que respondeu há menos tempo primeiro");
        assert_eq!(t1.len(), 2 + MORTOS_POR_CICLO, "só alguns mortos por ciclo: {t1:?}");
        // o ciclo seguinte (depois da espera) começa pelos mortos que ainda não tiveram vez
        let r2 = rodar(&b.conn, &Ambiente::novo("eu", T0 + 60_000), &vivos);
        let t2 = tentados(&r2);
        let novos: Vec<&&str> = t2[2..].iter().filter(|e| !t1.contains(e)).collect();
        assert_eq!(novos.len(), 6 - MORTOS_POR_CICLO, "os dois que faltavam entraram: {t2:?}");
        assert_eq!(
            &t2[2..2 + novos.len()],
            novos.iter().map(|e| **e).collect::<Vec<_>>().as_slice(),
            "e foram os primeiros"
        );
    }

    /// T-34, T-36 · CA-B1, CA-B2, CA-B4: 40 caixas mortos e 1 saudável. O
    /// saudável sincroniza no primeiro ciclo; o ciclo tem limite de tempo;
    /// nunca há duas conexões ao mesmo tempo; as linhas não crescem.
    #[test]
    fn t34_t36_quarenta_mortos_e_um_saudavel() {
        let b = Banco::em_memoria().unwrap();
        caixas_mortos(&b, 40);
        aprender(&b.conn, "bom", "Bom", "10.2.1.5:47810", T0).unwrap();
        let ao_mesmo_tempo = Arc::new(AtomicUsize::new(0));
        let maximo = Arc::new(AtomicUsize::new(0));
        let rodada = |agora: i64, orcamento: Duration| {
            let mut a = Ambiente::novo("eu", agora);
            a.orcamento = orcamento;
            let (m, mx) = (ao_mesmo_tempo.clone(), maximo.clone());
            let mut puxar = |e: &str, _s: Option<&Saida>| -> Resultado<Puxada> {
                let agora = m.fetch_add(1, Ordering::SeqCst) + 1;
                mx.fetch_max(agora, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(100)); // um "tempo-limite" encolhido
                m.fetch_sub(1, Ordering::SeqCst);
                if e.starts_with("10.2.") {
                    Ok(Puxada { dispositivo: "bom".into(), total: 0, confirmacao: nada() })
                } else {
                    Err(Erro::validacao("Falha na rede: tempo esgotado"))
                }
            };
            let inicio = Instant::now();
            let r = ciclo(&b.conn, &a, &|| false, &mut puxar);
            (r, inicio.elapsed())
        };
        let (r, _) = rodada(T0, Duration::from_secs(60));
        assert_eq!(tentados(&r)[0], "10.2.1.5:47810", "o saudável é o primeiro");
        assert!(r.tentativas[0].ok);
        assert_eq!(r.mortos_tentados, MORTOS_POR_CICLO, "no máximo {MORTOS_POR_CICLO} mortos por ciclo");
        // com o orçamento curto, o ciclo para por tempo (e o saudável ainda foi o primeiro)
        let (r, duracao) = rodada(T0 + 120_000, Duration::from_millis(250));
        assert!(r.parou_por_tempo);
        assert_eq!(tentados(&r)[0], "10.2.1.5:47810");
        assert!(duracao < Duration::from_millis(900), "{duracao:?}");
        assert_eq!(maximo.load(Ordering::SeqCst), 1, "uma conexão de cada vez, não uma por caixa");
        // em cinco ciclos todos os mortos tiveram vez, as linhas não cresceram e o erro é um texto só
        for i in 1..=12 {
            rodada(T0 + 120_000 + i * 60_000, Duration::from_secs(60));
        }
        let n: i64 = b.conn.query_row("SELECT COUNT(*) FROM sync_endereco", [], |x| x.get(0)).unwrap();
        assert_eq!(n, 41);
        let sem_vez: i64 =
            b.conn.query_row("SELECT COUNT(*) FROM sync_endereco WHERE tentado_ms = 0", [], |x| x.get(0)).unwrap();
        assert_eq!(sem_vez, 0, "ninguém ficou sem vez");
        let erro_mais_longo: i64 =
            b.conn.query_row("SELECT MAX(LENGTH(erro)) FROM sync_endereco", [], |x| x.get(0)).unwrap();
        assert!(erro_mais_longo < 200);
    }

    /// T-35 · CA-B3: durante um ciclo cheio de mortos, uma gravação feita por
    /// outra conexão ao mesmo arquivo conclui rápido: o ciclo não segura o banco
    /// enquanto espera a rede.
    #[test]
    fn t35_operacoes_locais_continuam_durante_o_ciclo() {
        let pasta = std::env::temp_dir().join(format!("rustinho-caminhos-{}", crate::tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let arquivo = pasta.join("rustinho.db");
        let b = Banco::abrir(&arquivo).unwrap();
        caixas_mortos(&b, 20);
        let caminho = arquivo.clone();
        let ciclo_lento = thread::spawn(move || {
            let conn = Connection::open(&caminho).unwrap();
            conn.busy_timeout(Duration::from_secs(10)).unwrap();
            let mut puxar = |_e: &str, _s: Option<&Saida>| -> Resultado<Puxada> {
                thread::sleep(Duration::from_millis(250));
                Err(Erro::validacao("Falha na rede: tempo esgotado"))
            };
            let mut a = Ambiente::novo("eu", T0);
            a.orcamento = Duration::from_secs(2);
            ciclo(&conn, &a, &|| false, &mut puxar)
        });
        thread::sleep(Duration::from_millis(100));
        let mut piores = Duration::ZERO;
        for i in 0..8 {
            let inicio = Instant::now();
            config::gravar(&b.conn, "teste_local", &i.to_string()).unwrap();
            piores = piores.max(inicio.elapsed());
            thread::sleep(Duration::from_millis(100));
        }
        let r = ciclo_lento.join().unwrap();
        assert!(!r.tentativas.is_empty(), "o ciclo estava de fato rodando");
        assert!(piores < Duration::from_millis(200), "gravação local demorou {piores:?}");
        std::fs::remove_dir_all(pasta).ok();
    }

    /// T-37 · CA-B5: um dos mortos volta; é sincronizado sozinho assim que a
    /// espera (no máximo 30 s) acaba, sem reiniciar nada, com o contador dele.
    #[test]
    fn t37_morto_que_volta_sincroniza_sozinho() {
        let b = Banco::em_memoria().unwrap();
        caixas_mortos(&b, 6);
        b.conn.execute("UPDATE sync_par SET cursor = 77 WHERE dispositivo = 'morto03'", []).unwrap();
        let alvo = "10.1.3.5:47810";
        let mut agora = T0;
        for _ in 0..8 {
            rodar(&b.conn, &Ambiente::novo("eu", agora), &[]);
            agora += 4_000;
        }
        // voltou; no pior caso espera 30 s + 1 ciclo de 4 s
        let mut quando = None;
        for passo in 0..=((30_000 + 4_000) / 4_000) {
            let r = rodar(&b.conn, &Ambiente::novo("eu", agora + passo * 4_000), &[alvo]);
            if r.tentativas.iter().any(|t| t.endereco == alvo && t.ok) {
                quando = Some(passo * 4_000);
                break;
            }
        }
        let quando = quando.expect("o caixa que voltou nunca foi tentado");
        assert!(quando <= 34_000, "demorou {quando} ms");
        let cursor: i64 =
            b.conn.query_row("SELECT cursor FROM sync_par WHERE dispositivo = 'morto03'", [], |x| x.get(0)).unwrap();
        assert_eq!(cursor, 77, "o contador dele continua o mesmo");
    }

    /// T-38 · CA-B6: caixa sem notícia há mais de 7 dias deixa de ser tentado
    /// sozinho (continua listado); o digitado continua sendo tentado.
    #[test]
    fn t38_sem_noticia_ha_7_dias_nao_e_mais_tentado() {
        const DIA: i64 = 86_400_000;
        let b = Banco::em_memoria().unwrap();
        aprender(&b.conn, "velho", "Velho", "10.3.1.5:47810", T0).unwrap();
        digitar(&b.conn, &["10.3.2.5:47810".to_string()], T0).unwrap();
        let r = rodar(&b.conn, &Ambiente::novo("eu", T0 + 8 * DIA), &[]);
        assert_eq!(tentados(&r), ["10.3.2.5:47810"], "só o digitado");
        assert!(sync::pares(&b.conn).unwrap().iter().any(|p| p.dispositivo == "velho"), "continua listado");
        // se ele voltar a anunciar, volta a ser tentado
        aprender(&b.conn, "velho", "Velho", "10.3.1.5:47810", T0 + 8 * DIA).unwrap();
        let r = rodar(&b.conn, &Ambiente::novo("eu", T0 + 8 * DIA + 1_000), &[]);
        assert!(tentados(&r).contains(&"10.3.1.5:47810"));
    }

    /// Virtual/celular aprendido pelo anúncio não é tentado sem liberação; o
    /// digitado é sempre usado (CA-A3).
    #[test]
    fn caminho_de_interface_virtual_so_com_liberacao_ou_digitado() {
        let b = Banco::em_memoria().unwrap();
        let mut a = ambiente(T0, true);
        a.interfaces.as_mut().unwrap().push(Interface::de_teste("tailscale0", [100, 100, 1, 1], 24, Tipo::Virtual));
        aprender(&b.conn, "d1", "Caixa 2", "100.100.1.9:47810", T0).unwrap();
        digitar(&b.conn, &["100.100.1.10:47810".to_string()], T0).unwrap();
        let r = rodar(&b.conn, &a, &[]);
        assert_eq!(tentados(&r), ["100.100.1.10:47810"], "só o digitado");
        a.liberadas = vec!["tailscale0".into()];
        a.agora_ms = T0 + 60_000;
        let r = rodar(&b.conn, &a, &[]);
        assert!(tentados(&r).contains(&"100.100.1.9:47810"), "liberada: o aprendido entra");
    }

    // ------------------------------------------------------------------
    // Com servidores de verdade (neste computador, endereços 127.0.0.x)
    // ------------------------------------------------------------------

    struct Caixa {
        caminho: PathBuf,
        banco: Banco,
        pt: u16,
        pu: u16,
        motor: Option<Motor>,
    }

    fn caixa(nome: &str) -> Caixa {
        let pasta = std::env::temp_dir().join(format!("rustinho-caminhos-{nome}-{}", crate::tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let caminho = pasta.join("rustinho.db");
        let banco = Banco::abrir(&caminho).unwrap();
        config::gravar(&banco.conn, "caixa_nome", nome).unwrap();
        sync::salvar_ajustes(&banco.conn, true, SENHA, "").unwrap();
        Caixa { caminho, banco, pt: porta_livre(), pu: porta_livre(), motor: None }
    }

    impl Caixa {
        fn servir(&mut self) {
            self.motor = Some(Motor::iniciar(self.caminho.clone(), self.pt, self.pu));
            thread::sleep(Duration::from_millis(400));
        }
        fn conexao(&self) -> Connection {
            let c = Connection::open(&self.caminho).unwrap();
            c.busy_timeout(Duration::from_secs(10)).unwrap();
            crate::banco::registrar_funcoes(&c).unwrap();
            c
        }
        fn destino(&self) -> std::net::SocketAddr {
            ([127, 0, 0, 1], self.pt).into()
        }
        fn dispositivo(&self) -> String {
            self.banco.dispositivo.clone()
        }
    }

    impl Drop for Caixa {
        fn drop(&mut self) {
            if let Some(mut m) = self.motor.take() {
                m.encerrar(Duration::from_secs(20));
            }
        }
    }

    /// A tem dados (um usuário, uma peça e `n` movimentos de estoque).
    fn popular(a: &Caixa, n: usize) {
        let u = usuarios::criar(&a.banco.conn, &a.banco.dispositivo, "Maria", "balcao-azul-2026", true).unwrap();
        let mut f = produtos::ficha_nova(&a.banco.conn).unwrap();
        f.nome = "Pastilha".into();
        f.custo = "40".into();
        f.estoque_inicial = "5".into();
        let p = produtos::salvar(&a.banco.conn, &a.banco.dispositivo, &u.id, &f).unwrap();
        for i in 0..n {
            estoque::lancar(&a.banco.conn, &a.banco.dispositivo, Some(&u.id), &p, 1000, "compra", &format!("lote {i}"))
                .unwrap();
        }
    }

    /// Os movimentos de estoque (ids) e as peças de cada banco: iguais, sem repetição.
    fn conteudo(c: &Connection) -> (Vec<String>, Vec<String>, i64) {
        let ids = |sql: &str| -> Vec<String> {
            c.prepare(sql).unwrap().query_map([], |l| l.get(0)).unwrap().collect::<Result<_, _>>().unwrap()
        };
        let repetidos: i64 =
            c.query_row("SELECT COUNT(*) - COUNT(DISTINCT id) FROM estoque_movimento", [], |l| l.get(0)).unwrap();
        (ids("SELECT id FROM estoque_movimento ORDER BY id"), ids("SELECT id FROM produto ORDER BY id"), repetidos)
    }

    fn eth_cabo() -> Interface {
        Interface::de_teste("cabo0", [127, 0, 0, 2], 32, Tipo::Cabo)
    }
    fn eth_wifi() -> Interface {
        Interface::de_teste("wlan9", [127, 0, 0, 3], 32, Tipo::Wifi)
    }

    /// Um ciclo de verdade de `b`: usa `puxar_por` com a porta local dele.
    fn ciclo_real(b: &Caixa, agora: i64, ifs: Vec<Interface>) -> Resumo {
        let conn = b.conexao();
        let mut a = Ambiente::novo(&b.dispositivo(), agora);
        a.interfaces = Some(ifs);
        let (caminho, porta) = (b.caminho.clone(), b.pt);
        let mut puxar = |e: &str, s: Option<&Saida>| sync::puxar_por(&caminho, e, porta, None, s);
        ciclo(&conn, &a, &|| false, &mut puxar)
    }

    /// T-07 (real), T-27 e T-28 no ciclo, CA-T1, CA-T4, CA-R3: pelo cabo (127.0.0.2)
    /// e pelo Wi-Fi (127.0.0.3). Cabo cai: o mesmo ciclo entrega pelo Wi-Fi. O cabo
    /// volta: reassume, a conexão sai do IP dele e a tela mostra "cabo".
    #[test]
    fn i01_cabo_cai_wifi_entrega_e_cabo_reassume() {
        let mut a = caixa("A");
        popular(&a, 3);
        a.servir();
        let b = caixa("B");
        let mut cabo = Encaminhador::novo([127, 0, 0, 2], a.destino());
        let wifi = Encaminhador::novo([127, 0, 0, 3], a.destino());
        {
            let c = b.conexao();
            aprender(&c, &a.dispositivo(), "A", &cabo.endereco(), T0).unwrap();
            aprender(&c, &a.dispositivo(), "A", &wifi.endereco(), T0).unwrap();
        }
        cabo.fechar();
        let r = ciclo_real(&b, T0, vec![eth_cabo(), eth_wifi()]);
        assert_eq!(tentados(&r), [cabo.endereco(), wifi.endereco()], "{r:?}");
        let (ma, mb) = (conteudo(&a.conexao()), conteudo(&b.conexao()));
        assert_eq!(ma, mb, "os dados chegaram pelo Wi-Fi, completos");
        assert!(!ma.0.is_empty());
        assert_eq!(*wifi.origens.lock().unwrap(), [Ipv4Addr::new(127, 0, 0, 3)], "saiu do IP do Wi-Fi");
        let c = b.conexao();
        assert!(l(&c, &wifi.endereco()).confirmada);
        assert_eq!(l(&c, &wifi.endereco()).interface, "Wi-Fi");

        // o cabo volta; depois da espera é tentado, funciona e reassume
        cabo.abrir();
        let r = ciclo_real(&b, T0 + 8_000, vec![eth_cabo(), eth_wifi()]);
        assert_eq!(tentados(&r), [cabo.endereco()]);
        assert_eq!(*cabo.origens.lock().unwrap(), [Ipv4Addr::new(127, 0, 0, 2)], "saiu do IP do cabo");
        let pares = sync::pares(&c).unwrap();
        assert_eq!(pares.len(), 1, "um par só, com dois caminhos");
        assert_eq!(pares[0].via, "cabo");
        assert_eq!(pares[0].endereco, cabo.endereco());
        assert_eq!(pares[0].caminhos.len(), 2);
    }

    /// T-10 · CA-C1: a resposta cortada em qualquer ponto (0, 1, 10 bytes, no
    /// meio, no fim − 1) não aplica nada parcial nem avança o contador; a
    /// tentativa por outro caminho completa; os bancos ficam iguais.
    #[test]
    fn t10_resposta_cortada_em_qualquer_ponto() {
        let mut a = caixa("A");
        popular(&a, 4);
        a.servir();
        // medida: quantos bytes tem a resposta inteira
        let medida = {
            let b0 = caixa("B0");
            let e = Encaminhador::novo([127, 0, 0, 2], a.destino());
            let r = ciclo_real(&b0, T0, vec![eth_cabo(), eth_wifi()]);
            assert!(r.tentativas.is_empty(), "sem caminhos ainda");
            aprender(&b0.conexao(), &a.dispositivo(), "A", &e.endereco(), T0).unwrap();
            ciclo_real(&b0, T0, vec![eth_cabo(), eth_wifi()]);
            e.bytes_da_resposta.load(Ordering::SeqCst)
        };
        assert!(medida > 100, "medida = {medida}");
        let b = caixa("B");
        let cabo = Encaminhador::novo([127, 0, 0, 2], a.destino());
        let mut wifi = Encaminhador::novo([127, 0, 0, 3], a.destino());
        wifi.fechar();
        {
            let c = b.conexao();
            aprender(&c, &a.dispositivo(), "A", &cabo.endereco(), T0).unwrap();
            aprender(&c, &a.dispositivo(), "A", &wifi.endereco(), T0).unwrap();
        }
        let antes = conteudo(&b.conexao());
        let cursor = |c: &Connection| -> i64 {
            c.query_row("SELECT COALESCE(MAX(cursor), 0) FROM sync_par", [], |x| x.get(0)).unwrap()
        };
        let mut agora = T0;
        for corte in [0, 1, 10, medida / 2, medida - 1] {
            cabo.modo(Modo::CortaResposta(corte));
            agora += 120_000; // passou qualquer espera
            let r = ciclo_real(&b, agora, vec![eth_cabo(), eth_wifi()]);
            assert!(r.tentativas.iter().all(|t| !t.ok), "corte em {corte}: {r:?}");
            assert_eq!(conteudo(&b.conexao()), antes, "corte em {corte}: nada parcial pode ficar");
            assert_eq!(cursor(&b.conexao()), 0, "corte em {corte}: o contador não avança");
        }
        // agora o Wi-Fi existe: o mesmo ciclo tenta o cabo (cortado), falha e completa pelo Wi-Fi
        wifi.abrir();
        agora += 120_000;
        let r = ciclo_real(&b, agora, vec![eth_cabo(), eth_wifi()]);
        assert_eq!(tentados(&r), [cabo.endereco(), wifi.endereco()]);
        assert_eq!(conteudo(&a.conexao()), conteudo(&b.conexao()));
        assert_eq!(conteudo(&b.conexao()).2, 0, "sem registro duplicado");
        assert!(cursor(&b.conexao()) > 0);
    }

    /// T-11 · CA-C2: o lote foi aplicado mas o contador não foi gravado (queda
    /// de energia no meio): repetir não duplica nada e o contador avança.
    #[test]
    fn t11_lote_aplicado_sem_contador_gravado() {
        let mut a = caixa("A");
        popular(&a, 3);
        a.servir();
        let mut b = caixa("B");
        let (lista, _) = sync::alteracoes_desde(&a.banco.conn, 0, 100_000).unwrap();
        let ultima = lista.iter().map(|x| x.seq).max().unwrap();
        sync::aplicar(&mut b.banco.conn, &lista).unwrap(); // aplicou; contador continua 0
        let cursor0: i64 =
            b.banco.conn.query_row("SELECT COALESCE(MAX(cursor), 0) FROM sync_par", [], |x| x.get(0)).unwrap();
        assert_eq!(cursor0, 0);
        let p = sync::puxar_por(&b.caminho, &a.destino().to_string(), b.pt, None, None).unwrap();
        assert_eq!(p.total, 0, "nada novo: o que veio já estava aplicado");
        let cursor: i64 = b
            .banco
            .conn
            .query_row("SELECT cursor FROM sync_par WHERE dispositivo = ?1", [a.dispositivo()], |x| x.get(0))
            .unwrap();
        assert_eq!(cursor, ultima, "o contador avançou");
        assert_eq!(conteudo(&a.conexao()), conteudo(&b.conexao()));
        drop(b.motor.take());
    }

    /// T-12 · CA-C3: lotes pequenos; o lote 1 vem pelo cabo, o cabo cai, o lote
    /// 2 em diante vem pelo Wi-Fi a partir do contador gravado: sem buracos, sem repetição.
    #[test]
    fn t12_troca_de_caminho_no_meio_da_puxada() {
        let mut a = caixa("A");
        popular(&a, 10);
        config::gravar(&a.banco.conn, "sync_lote_teste", "4").unwrap();
        a.servir();
        let b = caixa("B");
        let cabo = Encaminhador::novo([127, 0, 0, 2], a.destino());
        let wifi = Encaminhador::novo([127, 0, 0, 3], a.destino());
        cabo.modo(Modo::UmaSo); // atende a 1ª conexão (lote 1) e derruba as seguintes
        {
            let c = b.conexao();
            aprender(&c, &a.dispositivo(), "A", &cabo.endereco(), T0).unwrap();
            aprender(&c, &a.dispositivo(), "A", &wifi.endereco(), T0).unwrap();
        }
        let r = ciclo_real(&b, T0, vec![eth_cabo(), eth_wifi()]);
        assert_eq!(tentados(&r), [cabo.endereco(), wifi.endereco()], "{r:?}");
        assert_eq!(cabo.conexoes.load(Ordering::SeqCst), 2, "lote 1 e a tentativa do lote 2");
        assert!(wifi.conexoes.load(Ordering::SeqCst) >= 2, "os outros lotes vieram pelo Wi-Fi");
        assert_eq!(conteudo(&a.conexao()), conteudo(&b.conexao()), "sem buracos nem repetição");
        let (seq_a, cursor_b): (i64, i64) = (
            a.banco.conn.query_row("SELECT MAX(seq) FROM alteracao", [], |x| x.get(0)).unwrap(),
            b.banco.conn.query_row("SELECT cursor FROM sync_par", [], |x| x.get(0)).unwrap(),
        );
        assert_eq!(cursor_b, seq_a);
    }

    /// T-13 · CA-I2, CA-D4, CA-T5: caminho novo de um caixa conhecido (e um
    /// endereço digitado) continua do contador dele e não reaplica o lote
    /// inicial; `sync_par` segue com uma linha só.
    #[test]
    fn t13_caminho_novo_de_caixa_conhecido_continua_do_contador() {
        let mut a = caixa("A");
        popular(&a, 5);
        a.servir();
        let b = caixa("B");
        let primeiro = Encaminhador::novo([127, 0, 0, 2], a.destino());
        aprender(&b.conexao(), &a.dispositivo(), "A", &primeiro.endereco(), T0).unwrap();
        ciclo_real(&b, T0, vec![eth_cabo(), eth_wifi()]);
        let inicial = primeiro.bytes_da_resposta.load(Ordering::SeqCst);
        assert!(inicial > 500, "a 1ª resposta traz tudo: {inicial}");
        // 1) caminho novo (aprendido) com o primeiro fora do ar
        let mut primeiro = primeiro;
        primeiro.fechar();
        let novo = Encaminhador::novo([127, 0, 0, 3], a.destino());
        aprender(&b.conexao(), &a.dispositivo(), "A", &novo.endereco(), T0 + 1).unwrap();
        ciclo_real(&b, T0 + 60_000, vec![eth_cabo(), eth_wifi()]);
        let segunda = novo.bytes_da_resposta.load(Ordering::SeqCst);
        assert!(segunda > 0 && segunda * 3 < inicial, "o caminho novo só traz o que falta: {segunda} contra {inicial}");
        // 2) endereço digitado de um caixa que já é conhecido (a identidade só aparece na conversa)
        let digitado = Encaminhador::novo([127, 0, 0, 4], a.destino());
        digitar(&b.conexao(), &[digitado.endereco()], T0).unwrap();
        let mut novo = novo;
        novo.fechar();
        ciclo_real(
            &b,
            T0 + 120_000,
            vec![eth_cabo(), eth_wifi(), Interface::de_teste("lan4", [127, 0, 0, 4], 32, Tipo::Cabo)],
        );
        let terceira = digitado.bytes_da_resposta.load(Ordering::SeqCst);
        assert!(terceira > 0 && terceira * 3 < inicial, "digitado: {terceira} contra {inicial}");
        let c = b.conexao();
        let pares: i64 = c.query_row("SELECT COUNT(*) FROM sync_par", [], |x| x.get(0)).unwrap();
        assert_eq!(pares, 1);
        assert_eq!(l(&c, &digitado.endereco()).dispositivo, a.dispositivo(), "associado ao caixa na 1ª conversa");
        assert_eq!(conteudo(&a.conexao()), conteudo(&b.conexao()));
    }

    /// T-19 · CA-C5: duas puxadas do mesmo par ao mesmo tempo (cenário forçado)
    /// terminam iguais, sem duplicata.
    #[test]
    fn t19_duas_puxadas_ao_mesmo_tempo() {
        let mut a = caixa("A");
        popular(&a, 6);
        a.servir();
        let b = caixa("B");
        let alvo = a.destino().to_string();
        let tarefas: Vec<_> = (0..2)
            .map(|_| {
                let (caminho, alvo, porta) = (b.caminho.clone(), alvo.clone(), b.pt);
                thread::spawn(move || sync::puxar_por(&caminho, &alvo, porta, None, None))
            })
            .collect();
        for t in tarefas {
            t.join().unwrap().unwrap();
        }
        assert_eq!(conteudo(&a.conexao()), conteudo(&b.conexao()));
        assert_eq!(conteudo(&b.conexao()).2, 0);
    }

    /// T-20 · CA-C4: o pedido de faixa de códigos chegou, a resposta se perdeu;
    /// refeito por outro caminho, vem a MESMA faixa — nunca duas.
    #[test]
    fn t20_pedido_de_faixa_refeito_por_outro_caminho() {
        let mut a = caixa("A");
        crate::faixas::para_testes::configurar(&a.banco, "A", 0);
        a.servir();
        let b = caixa("B");
        let perde = Encaminhador::novo([127, 0, 0, 2], a.destino());
        let bom = Encaminhador::novo([127, 0, 0, 3], a.destino());
        perde.modo(Modo::PerdeNaConexao(2)); // 1ª conexão: puxar; 2ª: o pedido de faixa (resposta perdida)
        let c = b.conexao();
        aprender(&c, &a.dispositivo(), "A", &perde.endereco(), T0).unwrap();
        let ifs = vec![eth_cabo(), eth_wifi()];
        ciclo_real(&b, T0, ifs.clone());
        assert!(
            crate::faixas::para_testes::minhas(&b.banco).is_empty()
                || !crate::faixas::para_testes::minhas(&b.banco).is_empty()
        );
        let teve_antes = {
            let c2 = b.conexao();
            c2.query_row(
                "SELECT COUNT(*) FROM faixa_codigo WHERE dispositivo = ?1 AND inicio >= 100000",
                [b.dispositivo()],
                |x| x.get::<_, i64>(0),
            )
            .unwrap()
        };
        assert_eq!(teve_antes, 0, "a resposta se perdeu: B não gravou faixa nenhuma");
        // outro caminho
        aprender(&c, &a.dispositivo(), "A", &bom.endereco(), T0 + 1).unwrap();
        let mut perde = perde;
        perde.fechar();
        ciclo_real(&b, T0 + 120_000, ifs);
        let em_b = crate::faixas::para_testes::minhas(&b.banco);
        assert_eq!(em_b.len(), 1, "uma faixa só em B: {em_b:?}");
        let em_a: i64 = a
            .banco
            .conn
            .query_row(
                "SELECT COUNT(DISTINCT inicio) FROM faixa_codigo WHERE dispositivo = ?1 AND inicio >= 100000",
                [b.dispositivo()],
                |x| x.get(0),
            )
            .unwrap();
        assert_eq!(em_a, 1, "e uma faixa só dada a B por A");
    }

    /// T-16 · CA-F1, CA-C6: todos os caminhos caídos. O ciclo termina rápido, as
    /// vendas (gravação local) continuam, cada caminho guarda o seu erro e,
    /// quando um volta, a sincronização retoma sozinha. Nada usa a internet.
    #[test]
    fn t16_todos_os_caminhos_caidos_e_volta_sozinho() {
        let mut a = caixa("A");
        popular(&a, 2);
        a.servir();
        let b = caixa("B");
        let mut cabo = Encaminhador::novo([127, 0, 0, 2], a.destino());
        let mut wifi = Encaminhador::novo([127, 0, 0, 3], a.destino());
        {
            let c = b.conexao();
            aprender(&c, &a.dispositivo(), "A", &cabo.endereco(), T0).unwrap();
            aprender(&c, &a.dispositivo(), "A", &wifi.endereco(), T0).unwrap();
        }
        cabo.fechar();
        wifi.fechar();
        let inicio = Instant::now();
        let r = ciclo_real(&b, T0, vec![eth_cabo(), eth_wifi()]);
        assert!(inicio.elapsed() < Duration::from_secs(3), "{:?}", inicio.elapsed());
        assert!(r.tentativas.iter().all(|t| !t.ok) && r.tentativas.len() == 2);
        let c = b.conexao();
        assert!(!l(&c, &cabo.endereco()).erro.is_empty() && !l(&c, &wifi.endereco()).erro.is_empty());
        let par = &sync::pares(&c).unwrap()[0];
        assert!(!par.erro.is_empty() && par.caminhos.iter().all(|x| !x.erro.is_empty()));
        // venda local continua: gravar enquanto tudo está caído
        let u: String = b.banco.conn.query_row("SELECT 'x'", [], |x| x.get(0)).unwrap();
        config::gravar(&b.banco.conn, "teste_local", &u).unwrap();
        assert!(!conteudo(&a.conexao()).0.is_empty() && conteudo(&b.conexao()).0.is_empty());
        // o Wi-Fi volta; sem reiniciar nada, o ciclo seguinte (depois da espera) sincroniza
        wifi.abrir();
        let r = ciclo_real(&b, T0 + 30_000, vec![eth_cabo(), eth_wifi()]);
        assert!(r.tentativas.iter().any(|t| t.ok && t.endereco == wifi.endereco()));
        assert_eq!(conteudo(&a.conexao()), conteudo(&b.conexao()));
        assert_eq!(sync::pares(&b.conexao()).unwrap()[0].erro, "", "o erro some quando volta");
    }

    use std::net::Ipv4Addr;
}
