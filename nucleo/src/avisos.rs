//! Avisos do dia, para a faixa do Início: o que vence hoje e o que já
//! venceu. Cada aviso tem um id; a faixa some por aquele dia quando a
//! pessoa a arrasta para o lado.

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::erro::Resultado;
use crate::{backup, config, fiscal, numeros};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Aviso {
    /// "receber", "pagar" ou "cheques".
    pub id: String,
    /// Dia a que o aviso se refere ("2026-10-15"); escondido vale só nele.
    pub dia: String,
    /// "Hoje" ou "Vencido".
    pub titulo: String,
    /// "2 parcelas a receber · R$ 341,89".
    pub texto: String,
    /// Segunda parte, em vermelho: "vencido R$ 120,00 de 3 clientes".
    pub atraso: String,
    /// Função do menu que abre ao clicar.
    pub funcao: String,
}

/// Esconde o aviso neste computador até o dia seguinte.
pub fn esconder(conn: &Connection, id: &str) -> Resultado<()> {
    config::gravar(conn, &format!("aviso_escondido.{id}"), &fiscal::hoje(conn)?)
}

fn escondido(conn: &Connection, id: &str, hoje: &str) -> Resultado<bool> {
    Ok(config::ler(conn, &format!("aviso_escondido.{id}"))?.as_deref() == Some(hoje))
}

fn plural(n: i64, um: &str, varios: &str) -> String {
    format!("{n} {}", if n == 1 { um } else { varios })
}

/// Aviso de quem entrou com uma senha antiga fora da política (F-17). Não
/// bloqueia nada; some por um dia quando a pessoa o arrasta para o lado e
/// some de vez quando ela troca a senha. É por usuário: o computador pode
/// ter vários, e esconder o aviso de um não esconde o do outro.
pub fn da_senha(conn: &Connection, usuario_id: &str) -> Resultado<Option<Aviso>> {
    let Some(Some(problema)) = crate::usuarios::situacao_senha(conn, usuario_id)? else {
        return Ok(None);
    };
    let id = format!("senha.{usuario_id}");
    let hoje = fiscal::hoje(conn)?;
    if escondido(conn, &id, &hoje)? {
        return Ok(None);
    }
    let gerente = crate::usuarios::buscar(conn, usuario_id)?.gerente;
    Ok(Some(Aviso {
        id,
        dia: hoje,
        titulo: "Sua senha".into(),
        texto: format!("{} · troque quando puder", problema.resumo()),
        atraso: if gerente { "ela também protege os backups".into() } else { String::new() },
        funcao: "configuracoes".into(),
    }))
}

/// Avisos de hoje.
pub fn do_dia(conn: &Connection) -> Resultado<Vec<Aviso>> {
    let hoje = fiscal::hoje(conn)?;
    let mut avisos = Vec::new();

    let (hoje_qtd, hoje_valor, venc_valor, venc_clientes): (i64, i64, i64, i64) = conn.query_row(
        "SELECT COUNT(CASE WHEN vencimento = ?1 THEN 1 END),
                COALESCE(SUM(CASE WHEN vencimento = ?1 THEN valor - pago END), 0),
                COALESCE(SUM(CASE WHEN vencimento < ?1 THEN valor - pago END), 0),
                COUNT(DISTINCT CASE WHEN vencimento < ?1 THEN cliente_id END)
           FROM conta_receber_saldo WHERE cancelado_em IS NULL AND valor > pago",
        params![hoje],
        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)),
    )?;
    if (hoje_qtd > 0 || venc_valor > 0) && !escondido(conn, "receber", &hoje)? {
        let atraso = if venc_valor > 0 {
            format!(
                "vencido R$ {} de {}",
                numeros::escrever_dinheiro(venc_valor),
                plural(venc_clientes, "cliente", "clientes")
            )
        } else {
            String::new()
        };
        let (titulo, texto) = if hoje_qtd > 0 {
            (
                "Hoje".to_string(),
                format!(
                    "{} a receber · R$ {}",
                    plural(hoje_qtd, "parcela", "parcelas"),
                    numeros::escrever_dinheiro(hoje_valor)
                ),
            )
        } else {
            ("A receber".to_string(), String::new())
        };
        avisos.push(Aviso { id: "receber".into(), dia: hoje.clone(), titulo, texto, atraso, funcao: "receber".into() });
    }

    // Contas a pagar
    let (p_qtd, p_valor, p_venc, p_venc_qtd) = crate::pagar::do_dia(conn)?;
    if (p_qtd > 0 || p_venc > 0) && !escondido(conn, "pagar", &hoje)? {
        let atraso = if p_venc > 0 {
            format!(
                "{} vencida{} R$ {}",
                plural(p_venc_qtd, "conta", "contas"),
                if p_venc_qtd == 1 { "" } else { "s" },
                numeros::escrever_dinheiro(p_venc)
            )
        } else {
            String::new()
        };
        let (titulo, texto) = if p_qtd > 0 {
            (
                "Hoje".to_string(),
                format!("{} a pagar · R$ {}", plural(p_qtd, "conta", "contas"), numeros::escrever_dinheiro(p_valor)),
            )
        } else {
            ("A pagar".to_string(), String::new())
        };
        avisos.push(Aviso {
            id: "pagar".into(),
            dia: hoje.clone(),
            titulo,
            texto,
            atraso,
            funcao: "financeiro".into(),
        });
    }

    // Cheques para depositar
    let (c_qtd, c_valor) = crate::cheques::do_dia(conn)?;
    if c_qtd > 0 && !escondido(conn, "cheques", &hoje)? {
        avisos.push(Aviso {
            id: "cheques".into(),
            dia: hoje.clone(),
            titulo: "Cheques".into(),
            texto: format!(
                "{} para depositar · R$ {}",
                plural(c_qtd, "cheque bom", "cheques bons"),
                numeros::escrever_dinheiro(c_valor)
            ),
            atraso: String::new(),
            funcao: "financeiro".into(),
        });
    }

    // Encomendas que chegaram (falta avisar o cliente) e que falta pedir
    let (chegaram, a_pedir) = crate::encomendas::do_dia(conn)?;
    if (chegaram > 0 || a_pedir > 0) && !escondido(conn, "encomendas", &hoje)? {
        let mut partes = Vec::new();
        if chegaram > 0 {
            partes.push(format!("{} · avise o cliente", plural(chegaram, "chegou", "chegaram")));
        }
        if a_pedir > 0 {
            partes.push(format!("{} para pedir", plural(a_pedir, "encomenda", "encomendas")));
        }
        avisos.push(Aviso {
            id: "encomendas".into(),
            dia: hoje.clone(),
            titulo: "Encomendas".into(),
            texto: partes.join(" · "),
            atraso: String::new(),
            funcao: "estoque".into(),
        });
    }

    // Oficina: carros prontos para entregar e OS que passaram da previsão
    let (prontas, atrasadas): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(situacao = 'pronta'), 0),
                COALESCE(SUM(situacao IN ('orcamento', 'aprovada') AND previsao IS NOT NULL AND previsao < ?1), 0)
           FROM ordem_servico",
        [&hoje],
        |l| Ok((l.get(0)?, l.get(1)?)),
    )?;
    if (prontas > 0 || atrasadas > 0) && !escondido(conn, "oficina", &hoje)? {
        avisos.push(Aviso {
            id: "oficina".into(),
            dia: hoje.clone(),
            titulo: "Oficina".into(),
            texto: if prontas > 0 {
                format!("{} para entregar", plural(prontas, "carro pronto", "carros prontos"))
            } else {
                String::new()
            },
            atraso: if atrasadas > 0 {
                format!("{} passou da previsão", plural(atrasadas, "OS", "OS"))
            } else {
                String::new()
            },
            funcao: "oficina".into(),
        });
    }

    // Troca de dados entre caixas: conflitos esperando o gerente.
    let conflitos = crate::sync::conflitos_abertos(conn)?;
    if conflitos > 0 && !escondido(conn, "conflitos", &hoje)? {
        avisos.push(Aviso {
            id: "conflitos".into(),
            dia: hoje.clone(),
            titulo: "Caixas".into(),
            texto: format!("{} entre caixas para o gerente ver", plural(conflitos, "diferença", "diferenças")),
            atraso: String::new(),
            funcao: "configuracoes".into(),
        });
    }

    // Banco copiado de outro computador (ou computador não confirmado): o
    // caixa não dá códigos até o gerente resolver. Não dá para esconder.
    if let Some(motivo) = crate::faixas::bloqueio(conn)? {
        avisos.push(Aviso {
            id: "codigos_bloqueio".into(),
            dia: hoje.clone(),
            titulo: "Códigos".into(),
            texto: "Este caixa está sem poder dar códigos de peças".into(),
            atraso: if motivo == crate::faixas::BLOQUEIO_COPIA {
                "o banco veio de outro computador: o gerente registra em Configurações › Gerente".into()
            } else {
                "o gerente confirma em Configurações › Gerente".into()
            },
            funcao: "configuracoes".into(),
        });
    }

    // Códigos das peças que levam a mais de uma peça (ou faixas de dois
    // caixas que se cruzam): o gerente resolve na pílula Gerente.
    let codigos = crate::faixas::para_ver(conn)?;
    if codigos > 0 && !escondido(conn, "codigos", &hoje)? {
        avisos.push(Aviso {
            id: "codigos".into(),
            dia: hoje.clone(),
            titulo: "Códigos".into(),
            texto: format!("{} em mais de uma peça", plural(codigos, "código", "códigos")),
            atraso: "o gerente resolve em Configurações › Gerente".into(),
            funcao: "configuracoes".into(),
        });
    }

    // R-05: registros que apontam para algo apagado (ou que não chegou em
    // 24 h). Não some sozinho: o gerente precisa olhar.
    let quebradas: i64 = config::ler(conn, "sync_quebradas")?.and_then(|t| t.parse().ok()).unwrap_or(0);
    let atrasadas: i64 = config::ler(conn, "sync_atrasadas")?.and_then(|t| t.parse().ok()).unwrap_or(0);
    if quebradas > 0 && !escondido(conn, "referencias", &hoje)? {
        avisos.push(Aviso {
            id: "referencias".into(),
            dia: hoje.clone(),
            titulo: "Caixas".into(),
            texto: format!("{} entre caixas", plural(quebradas, "registro incompleto", "registros incompletos")),
            atraso: "apontam para algo apagado ou inconsistente (comprovado)".into(),
            funcao: "configuracoes".into(),
        });
    } else if atrasadas > 0 && !escondido(conn, "referencias_atrasadas", &hoje)? {
        // Atraso não é erro: só pede para olhar se os outros caixas estão ligados.
        avisos.push(Aviso {
            id: "referencias_atrasadas".into(),
            dia: hoje.clone(),
            titulo: "Caixas".into(),
            texto: format!(
                "{} há mais de 24 horas",
                plural(atrasadas, "registro espera dados de outro caixa", "registros esperam dados de outro caixa")
            ),
            atraso: "confira se os outros caixas estão ligados; nada está bloqueado".into(),
            funcao: "configuracoes".into(),
        });
    }

    // Backup: falhou agora, ou está há 2 dias sem cópia (no computador ou no Drive).
    let b = backup::situacao(conn)?;
    let dias_desde = |quando: &str| -> Resultado<i64> {
        Ok(conn
            .query_row("SELECT CAST(julianday('now') - julianday(?1) AS INTEGER)", [quando], |l| {
                l.get::<_, Option<i64>>(0)
            })?
            .unwrap_or(0))
    };
    let referencia = if b.ultimo.is_empty() { &b.desde } else { &b.ultimo };
    let sem_backup = if referencia.is_empty() { 0 } else { dias_desde(referencia)? };
    if (!b.erro.is_empty() || sem_backup >= backup::DIAS_ATE_AVISAR) && !escondido(conn, "backup", &hoje)? {
        let (texto, atraso) = if !b.erro.is_empty() {
            ("O último backup não deu certo".to_string(), curto(&b.erro))
        } else if b.ultimo.is_empty() {
            ("Ainda não teve nenhum backup".to_string(), String::new())
        } else {
            (format!("Sem backup há {sem_backup} dias"), String::new())
        };
        avisos.push(Aviso {
            id: "backup".into(),
            dia: hoje.clone(),
            titulo: "Backup".into(),
            texto,
            atraso,
            funcao: "configuracoes".into(),
        });
    }
    if config::ligado(conn, "backup_nuvem")? {
        let referencia = if b.nuvem_ultimo.is_empty() { &b.nuvem_desde } else { &b.nuvem_ultimo };
        let sem_nuvem = if referencia.is_empty() { 0 } else { dias_desde(referencia)? };
        if sem_nuvem >= backup::DIAS_ATE_AVISAR && !escondido(conn, "backup_nuvem", &hoje)? {
            avisos.push(Aviso {
                id: "backup_nuvem".into(),
                dia: hoje.clone(),
                titulo: "Backup".into(),
                texto: format!("Nada chega ao Google Drive há {sem_nuvem} dias"),
                atraso: curto(&b.nuvem_erro),
                funcao: "configuracoes".into(),
            });
        }
    }
    Ok(avisos)
}

/// Motivo curto e seguro para a faixa (que aceita um pouco de HTML).
fn curto(t: &str) -> String {
    let t: String = t.chars().take(90).collect();
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::receber::{self, LancamentoEntrada};
    use crate::{pessoas, usuarios};

    #[test]
    fn avisa_o_que_vence_hoje_e_o_vencido() {
        let b = Banco::em_memoria().unwrap();
        let c = &b.conn;
        let d = &b.dispositivo;
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        assert!(do_dia(c).unwrap().is_empty(), "sem nada, sem faixa");

        let mut cli = pessoas::ficha_nova(c, "cliente").unwrap();
        cli.nome = "Oficina do Zé".into();
        let ze = pessoas::salvar(c, d, &u, &cli).unwrap();
        let hoje = fiscal::hoje(c).unwrap();
        let (a, m, dd) = (&hoje[0..4], &hoje[5..7], &hoje[8..10]);
        receber::lancar(
            c,
            d,
            &u,
            &LancamentoEntrada {
                cliente_id: ze.clone(),
                descricao: "Caderno".into(),
                valor: "341,89".into(),
                parcelas: 1,
                primeiro_vencimento: format!("{dd}/{m}/{a}"),
            },
        )
        .unwrap();
        let v = do_dia(c).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].titulo, "Hoje");
        assert_eq!(v[0].texto, "1 parcela a receber · R$ 341,89");
        assert_eq!(v[0].atraso, "");

        c.execute("UPDATE conta_receber SET vencimento = date(vencimento, '-3 days')", []).unwrap();
        let v = do_dia(c).unwrap();
        assert_eq!(v[0].titulo, "A receber");
        assert_eq!(v[0].atraso, "vencido R$ 341,89 de 1 cliente");

        esconder(c, "receber").unwrap();
        assert!(do_dia(c).unwrap().is_empty(), "escondido hoje");
        config::gravar(c, "aviso_escondido.receber", "2026-01-01").unwrap();
        assert_eq!(do_dia(c).unwrap().len(), 1, "volta no outro dia");
    }

    #[test]
    fn avisa_backup() {
        let b = Banco::em_memoria().unwrap();
        assert!(!do_dia(&b.conn).unwrap().iter().any(|a| a.id == "backup"));
        config::gravar(&b.conn, "backup_desde", &crate::tempo::dias_atras(3)).unwrap();
        let a = do_dia(&b.conn).unwrap();
        assert!(a.iter().any(|a| a.id == "backup" && a.texto.contains("nenhum")));
        config::gravar(&b.conn, "backup_ultimo", &crate::tempo::agora()).unwrap();
        assert!(!do_dia(&b.conn).unwrap().iter().any(|a| a.id == "backup"));
        config::gravar(&b.conn, "backup_erro", "Não deu para gravar na pasta <x>").unwrap();
        let a = do_dia(&b.conn).unwrap();
        assert!(a.iter().any(|a| a.id == "backup" && a.atraso.contains("&lt;x&gt;")));
        config::gravar(&b.conn, "backup_nuvem", "1").unwrap();
        config::gravar(&b.conn, "backup_nuvem_desde", &crate::tempo::dias_atras(2)).unwrap();
        assert!(do_dia(&b.conn).unwrap().iter().any(|a| a.id == "backup_nuvem"));
    }
}
