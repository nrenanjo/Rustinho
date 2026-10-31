//! Notificações do Início (0.9.15): cada uma tem uma categoria, que diz o
//! que a pessoa pode fazer com ela:
//!
//! - **informativa**: pode ser escondida até a meia-noite (ao voltar, mostra
//!   o estado de agora; o que perdeu a validade não volta);
//! - **atenção**: pode ser minimizada, nunca escondida nem descartada: fica
//!   até a condição acabar (vale depois de fechar e abrir o Rustinho);
//! - **crítica**: não se esconde nem se minimiza; fica expandida até ser
//!   resolvida.
//!
//! Informativa e atenção começam minimizadas (o quadrado com o ícone da
//! função, 0.9.16). Minimizar ou expandir é só apresentação (fica anotado
//! neste computador):
//! nunca resolve, apaga ou libera nada. Uma notificação some sozinha quando
//! a condição real que a gerou acaba. Cada condição tem um id: a mesma
//! condição atualiza a notificação, não cria outra.

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::erro::{Erro, Resultado};
use crate::{backup, config, fiscal, numeros};

pub const INFORMATIVA: &str = "informativa";
pub const ATENCAO: &str = "atencao";
pub const CRITICA: &str = "critica";

/// Uma linha do quadro de detalhes da notificação expandida.
#[derive(Debug, Clone, Serialize)]
pub struct Detalhe {
    pub rotulo: String,
    pub valor: String,
}

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
    /// "informativa", "atencao" ou "critica".
    pub categoria: String,
    /// Título da notificação expandida ("Atenção: caixa desatualizado").
    pub titulo_longo: String,
    /// A linha da notificação minimizada.
    pub resumo: String,
    /// Texto da notificação expandida (o problema e a consequência).
    pub descricao: String,
    /// Quadro de detalhes (vazio quando não precisa).
    pub detalhes: Vec<Detalhe>,
    /// Botão de ação ("Ver caixas") e onde ele leva (função + seção).
    pub acao: String,
    pub secao: String,
    /// Minimizada neste computador (só apresentação).
    pub minimizado: bool,
}

impl Aviso {
    /// Aviso simples (uma linha), como os de antes.
    fn simples(id: &str, dia: &str, titulo: String, texto: String, atraso: String, funcao: &str) -> Aviso {
        Aviso {
            id: id.into(),
            dia: dia.into(),
            titulo,
            texto,
            atraso,
            funcao: funcao.into(),
            categoria: INFORMATIVA.into(),
            titulo_longo: String::new(),
            resumo: String::new(),
            descricao: String::new(),
            detalhes: Vec::new(),
            acao: String::new(),
            secao: String::new(),
            minimizado: false,
        }
    }

    fn categoria(mut self, c: &str) -> Aviso {
        self.categoria = c.into();
        self
    }

    fn secao(mut self, s: &str) -> Aviso {
        self.secao = s.into();
        self
    }
}

/// Completa o que a tela precisa (título, resumo, estado minimizado).
/// Informativa e atenção começam minimizadas; a crítica fica sempre expandida
/// (nem uma marca antiga de "minimizada" muda isso).
fn completar(conn: &Connection, mut a: Aviso) -> Resultado<Aviso> {
    if a.titulo_longo.is_empty() {
        a.titulo_longo = match a.categoria.as_str() {
            ATENCAO => format!("Atenção: {}", a.titulo),
            CRITICA => format!("Crítico: {}", a.titulo),
            _ => a.titulo.clone(),
        };
    }
    if a.resumo.is_empty() {
        let partes: Vec<String> = [a.texto.clone(), a.atraso.clone()].into_iter().filter(|t| !t.is_empty()).collect();
        a.resumo = if partes.is_empty() {
            a.titulo_longo.clone()
        } else {
            format!("{} — {}", a.titulo_longo, partes.join(" · "))
        };
    }
    if a.descricao.is_empty() {
        a.descricao =
            [a.texto.clone(), a.atraso.clone()].into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" · ");
    }
    // Toda notificação leva ao lugar de resolver (linkagem): o clique no corpo
    // só alterna expandida/minimizada, então a ação fica num botão.
    if a.acao.is_empty() {
        a.acao = match a.id.as_str() {
            "receber" => "Ver crediário",
            "pagar" => "Ver contas a pagar",
            "cheques" => "Ver cheques",
            "encomendas" => "Ver encomendas",
            "oficina" => "Ver ordens de serviço",
            "backup" | "backup_nuvem" => "Ver backup",
            "conflitos" | "codigos" | "codigos_bloqueio" => "Resolver no Gerente",
            "referencias" | "referencias_atrasadas" | "outra_loja" => "Ver caixas",
            id if id.starts_with("senha.") => "Trocar senha",
            _ => "Abrir",
        }
        .to_string();
    }
    a.minimizado =
        a.categoria != CRITICA && config::ler(conn, &format!("aviso_minimizado.{}", a.id))?.as_deref() != Some("0");
    Ok(a)
}

/// Só uma não crítica aberta (em faixa) por vez. Uma base de outra versão
/// pode ter guardado várias como abertas: fica aberta a primeira, na ordem em
/// que aparecem. Só apresentação: nada é gravado aqui.
pub fn uma_aberta(avisos: &mut [Aviso]) {
    let mut aberta = false;
    for a in avisos.iter_mut().filter(|a| a.categoria != CRITICA && !a.minimizado) {
        a.minimizado = aberta;
        aberta = true;
    }
}

/// Minimiza ou expande neste computador uma das notificações que estão
/// aparecendo (`atuais`). É só apresentação. A crítica não se minimiza. As
/// outras não críticas ficam gravadas como minimizadas: abrir uma fecha a
/// outra, e uma marca antiga de "aberta" não reabre nada depois. Quem chama
/// passa uma transação, para gravar tudo ou nada.
pub fn minimizar(conn: &Connection, atuais: &[Aviso], id: &str, minimizado: bool) -> Resultado<()> {
    let Some(aviso) = atuais.iter().find(|a| a.id == id) else {
        return Err(Erro::validacao("Esta notificação já não está aparecendo."));
    };
    if minimizado && aviso.categoria == CRITICA {
        return Err(Erro::validacao("A notificação crítica fica aberta até o problema ser resolvido."));
    }
    for outra in atuais.iter().filter(|a| a.id != id && a.categoria != CRITICA) {
        config::gravar(conn, &format!("aviso_minimizado.{}", outra.id), "1")?;
    }
    config::gravar(conn, &format!("aviso_minimizado.{id}"), if minimizado { "1" } else { "0" })
}

/// Esconde neste computador, até a meia-noite, uma das notificações que
/// estão aparecendo (`atuais`). Quem decide é a categoria: só a informativa;
/// atenção e crítica ficam até a condição acabar.
pub fn esconder(conn: &Connection, atuais: &[Aviso], id: &str) -> Resultado<()> {
    let Some(aviso) = atuais.iter().find(|a| a.id == id) else {
        return Err(Erro::validacao("Esta notificação já não está aparecendo."));
    };
    if aviso.categoria != INFORMATIVA {
        return Err(Erro::validacao(
            "Este aviso não pode ser escondido: ele some quando o problema for resolvido. Você pode minimizá-lo.",
        ));
    }
    config::gravar(conn, &format!("aviso_escondido.{id}"), &fiscal::hoje(conn)?)
}

/// A notificação foi escondida hoje? Só vale para a informativa: uma marca
/// de "escondida" numa atenção ou numa crítica (de outra versão, de um backup
/// restaurado) não esconde nada.
fn escondida_hoje(conn: &Connection, a: &Aviso, hoje: &str) -> Resultado<bool> {
    Ok(a.categoria == INFORMATIVA && config::ler(conn, &format!("aviso_escondido.{}", a.id))?.as_deref() == Some(hoje))
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
    let gerente = crate::usuarios::buscar(conn, usuario_id)?.gerente;
    let a = Aviso::simples(
        &id,
        &hoje,
        "Sua senha".into(),
        format!("{} · troque quando puder", problema.resumo()),
        if gerente { "ela também protege os backups".into() } else { String::new() },
        "configuracoes",
    )
    .secao("loja");
    if escondida_hoje(conn, &a, &hoje)? {
        return Ok(None);
    }
    Ok(Some(completar(conn, a)?))
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
    if hoje_qtd > 0 || venc_valor > 0 {
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
        avisos.push(Aviso::simples("receber", &hoje, titulo, texto, atraso, "vender"));
    }

    // Contas a pagar
    let (p_qtd, p_valor, p_venc, p_venc_qtd) = crate::pagar::do_dia(conn)?;
    if p_qtd > 0 || p_venc > 0 {
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
        avisos.push(Aviso::simples("pagar", &hoje, titulo, texto, atraso, "financeiro"));
    }

    // Cheques para depositar
    let (c_qtd, c_valor) = crate::cheques::do_dia(conn)?;
    if c_qtd > 0 {
        avisos.push(Aviso::simples(
            "cheques",
            &hoje,
            "Cheques".into(),
            format!(
                "{} para depositar · R$ {}",
                plural(c_qtd, "cheque bom", "cheques bons"),
                numeros::escrever_dinheiro(c_valor)
            ),
            String::new(),
            "financeiro",
        ));
    }

    // Encomendas que chegaram (falta avisar o cliente) e que falta pedir
    let (chegaram, a_pedir) = crate::encomendas::do_dia(conn)?;
    if chegaram > 0 || a_pedir > 0 {
        let mut partes = Vec::new();
        if chegaram > 0 {
            partes.push(format!("{} · avise o cliente", plural(chegaram, "chegou", "chegaram")));
        }
        if a_pedir > 0 {
            partes.push(format!("{} para pedir", plural(a_pedir, "encomenda", "encomendas")));
        }
        avisos.push(Aviso::simples(
            "encomendas",
            &hoje,
            "Encomendas".into(),
            partes.join(" · "),
            String::new(),
            "estoque",
        ));
    }

    // Oficina: carros prontos para entregar e OS que passaram da previsão
    let (prontas, atrasadas): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(situacao = 'pronta'), 0),
                COALESCE(SUM(situacao IN ('orcamento', 'aprovada') AND previsao IS NOT NULL AND previsao < ?1), 0)
           FROM ordem_servico",
        [&hoje],
        |l| Ok((l.get(0)?, l.get(1)?)),
    )?;
    if prontas > 0 || atrasadas > 0 {
        avisos.push(Aviso::simples(
            "oficina",
            &hoje,
            "Oficina".into(),
            if prontas > 0 {
                format!("{} para entregar", plural(prontas, "carro pronto", "carros prontos"))
            } else {
                String::new()
            },
            if atrasadas > 0 {
                format!("{} passou da previsão", plural(atrasadas, "OS", "OS"))
            } else {
                String::new()
            },
            "oficina",
        ));
    }

    // Troca de dados entre caixas: conflitos esperando o gerente.
    let conflitos = crate::sync::conflitos_abertos(conn)?;
    if conflitos > 0 {
        avisos.push(
            Aviso::simples(
                "conflitos",
                &hoje,
                "Caixas".into(),
                format!("{} entre caixas para o gerente ver", plural(conflitos, "diferença", "diferenças")),
                String::new(),
                "configuracoes",
            )
            .categoria(ATENCAO)
            .secao("duplicatas"),
        );
    }

    // Banco copiado de outro computador (ou computador não confirmado): o
    // caixa não dá códigos até o gerente resolver. Não dá para esconder.
    if let Some(motivo) = crate::faixas::bloqueio(conn)? {
        avisos.push(
            Aviso::simples(
                "codigos_bloqueio",
                &hoje,
                "Códigos".into(),
                "Este caixa está sem poder dar códigos de peças".into(),
                if motivo == crate::faixas::BLOQUEIO_COPIA {
                    "o banco veio de outro computador: o gerente registra em Configurações › Simultaneidade › Duplicatas".into()
                } else {
                    "o gerente confirma em Configurações › Simultaneidade › Duplicatas".into()
                },
                "configuracoes",
            )
            .categoria(CRITICA)
            .secao("duplicatas"),
        );
    }

    // Códigos das peças que levam a mais de uma peça (ou faixas de dois
    // caixas que se cruzam): o gerente resolve em Simultaneidade › Duplicatas.
    let codigos = crate::faixas::para_ver(conn)?;
    if codigos > 0 {
        avisos.push(
            Aviso::simples(
                "codigos",
                &hoje,
                "Códigos".into(),
                format!("{} em mais de uma peça", plural(codigos, "código", "códigos")),
                "o gerente resolve em Configurações › Simultaneidade › Duplicatas".into(),
                "configuracoes",
            )
            .categoria(ATENCAO)
            .secao("duplicatas"),
        );
    }

    // O-3: uma tarefa da troca de dados caiu (erro interno). Parada de vez:
    // crítico; voltou sozinha há pouco: atenção.
    let banco: Option<String> =
        conn.query_row("SELECT file FROM pragma_database_list WHERE name = 'main'", [], |l| l.get(0)).ok();
    if let Some(banco) = banco.filter(|b| !b.is_empty()) {
        // FALHA-06: a conferência de integridade ao abrir achou defeito.
        if let Some(defeito) = crate::banco::defeito(std::path::Path::new(&banco)) {
            avisos.push(do_modo_seguro(&hoje, &defeito));
        }
        // RVI-02: com o motivo (porta ocupada, erro interno). O servidor e o
        // anúncio só valem com a troca de dados ligada; a puxada também dá os
        // códigos das peças, então vale sempre.
        let ligado = crate::sync::ajustes(conn).map(|a| a.ligado).unwrap_or(false);
        let vale = |(n, _): &&(&'static str, String)| ligado || *n == "puxada";
        let situacao = crate::supervisao::situacao(std::path::Path::new(&banco));
        let paradas: Vec<&(&str, String)> = situacao.paradas.iter().filter(vale).collect();
        let tentando: Vec<&(&str, String)> = situacao.tentando.iter().filter(vale).collect();
        let voltaram: Vec<&(&str, String)> = situacao.voltaram.iter().filter(vale).collect();
        let lista = |v: &[&(&str, String)]| {
            v.iter()
                .map(|(n, motivo)| format!("{} ({motivo})", crate::supervisao::nome_para_tela(n)))
                .collect::<Vec<_>>()
                .join("; ")
        };
        let porta = |v: &[&(&str, String)]| v.iter().any(|(_, m)| m.contains("porta"));
        if !paradas.is_empty() {
            let (resumo, acao) = if porta(&paradas) {
                (
                    "a porta da troca de dados está ocupada por outro programa",
                    "feche o outro programa e abra o Rustinho de novo; se continuar, chame o suporte",
                )
            } else {
                (
                    "um erro interno parou a troca com os outros caixas",
                    "feche e abra o Rustinho; se continuar, chame o suporte",
                )
            };
            let mut a = Aviso::simples(
                "troca_parada",
                &hoje,
                "A troca de dados parou".into(),
                resumo.into(),
                acao.into(),
                "configuracoes",
            )
            .categoria(CRITICA)
            .secao("caixas");
            a.descricao = format!(
                "Parou: {}. As vendas e o resto deste caixa continuam normalmente, mas as alterações não vão \
                 nem vêm dos outros caixas até o Rustinho ser aberto de novo. Nada do que já foi recebido se perdeu.",
                lista(&paradas)
            );
            avisos.push(a);
        } else if !tentando.is_empty() {
            // Não está rodando agora: o Início não pode dizer que está tudo bem.
            let mut a = Aviso::simples(
                "troca_tentando",
                &hoje,
                "A troca de dados não está funcionando".into(),
                "o Rustinho está tentando de novo".into(),
                String::new(),
                "configuracoes",
            )
            .categoria(ATENCAO)
            .secao("caixas");
            a.descricao = format!(
                "Falhou: {}. O Rustinho tenta de novo sozinho, algumas vezes; até lá as alterações não vão nem vêm \
                 dos outros caixas. As vendas continuam normalmente.",
                lista(&tentando)
            );
            avisos.push(a);
        } else if !voltaram.is_empty() {
            let mut a = Aviso::simples(
                "troca_voltou",
                &hoje,
                "Troca de dados".into(),
                "teve um problema e voltou sozinha".into(),
                String::new(),
                "configuracoes",
            )
            .categoria(ATENCAO)
            .secao("caixas");
            a.descricao = format!(
                "Uma parte da troca de dados com os outros caixas parou e voltou sozinha: {}. Nada se perdeu. Se \
                 este aviso aparecer muitas vezes, chame o suporte.",
                lista(&voltaram)
            );
            avisos.push(a);
        }
    }

    // FALHA-01: um computador de outra loja (configurada separadamente) tentou
    // trocar dados com este, com a mesma senha da rede. Nada foi misturado.
    if let Some((nome, endereco, _)) = crate::loja::ultima_recusa(conn)? {
        let mut a = Aviso::simples(
            "outra_loja",
            &hoje,
            "Troca de dados recusada".into(),
            format!(
                "{} é de outra loja",
                if nome.trim().is_empty() { format!("O computador {endereco}") } else { nome.clone() }
            ),
            "nada foi misturado; confira a senha da rede".into(),
            "configuracoes",
        )
        .categoria(CRITICA)
        .secao("caixas");
        a.descricao = format!(
            "Um computador{} usa a mesma senha da rede, mas foi configurado como outra loja. Para não misturar \
             as duas lojas (gerentes, cadastros e códigos), a troca de dados com ele foi recusada. Se ele devia \
             ser um caixa desta loja, ele precisa entrar pela opção \"Entrar numa loja que já existe\", com o \
             banco vazio. Se é de outra loja, troque a senha da rede de uma das duas.",
            if endereco.is_empty() { String::new() } else { format!(" ({endereco})") }
        );
        avisos.push(a);
    }

    // R-05: registros que apontam para algo apagado (ou que não chegou em
    // 24 h). Não some sozinho: o gerente precisa olhar.
    let quebradas: i64 = config::ler(conn, "sync_quebradas")?.and_then(|t| t.parse().ok()).unwrap_or(0);
    let atrasadas: i64 = config::ler(conn, "sync_atrasadas")?.and_then(|t| t.parse().ok()).unwrap_or(0);
    if quebradas > 0 {
        avisos.push(
            Aviso::simples(
                "referencias",
                &hoje,
                "Caixas".into(),
                format!("{} entre caixas", plural(quebradas, "registro incompleto", "registros incompletos")),
                "apontam para algo apagado ou inconsistente (comprovado)".into(),
                "configuracoes",
            )
            .categoria(ATENCAO)
            .secao("caixas"),
        );
    } else if atrasadas > 0 {
        // Atraso não é erro: só pede para olhar se os outros caixas estão ligados.
        avisos.push(
            Aviso::simples(
                "referencias_atrasadas",
                &hoje,
                "Caixas".into(),
                format!(
                    "{} há mais de 24 horas",
                    plural(atrasadas, "registro espera dados de outro caixa", "registros esperam dados de outro caixa")
                ),
                "confira se os outros caixas estão ligados; nada está bloqueado".into(),
                "configuracoes",
            )
            .secao("caixas"),
        );
    }

    // 0.9.15: caixas sem compatibilidade confirmada (bloqueiam rascunhos de OS).
    for c in crate::sync::bloqueios_de_rascunho(conn)? {
        avisos.push(aviso_de_compatibilidade(&hoje, &c));
    }
    // Alterações que não vão para um caixa de versão antiga.
    for c in crate::sync::caixas_conhecidos(conn)?.iter().filter(|c| c.nao_enviadas > 0 && c.situacao != "retirado") {
        let mut a = Aviso::simples(
            &format!("nao_enviadas.{}", c.dispositivo),
            &hoje,
            format!("{} parou de receber alterações", c.nome),
            format!("{} esperando a atualização do caixa", plural(c.nao_enviadas, "alteração", "alterações")),
            String::new(),
            "configuracoes",
        )
        .categoria(ATENCAO)
        .secao("caixas");
        a.descricao = format!(
            "O {} está com uma versão antiga do Rustinho e não consegue gravar uma alteração recente (por exemplo, \
             um rascunho de OS). Para nada se perder, a troca de dados com ele parou nesse ponto. Atualize o \
             Rustinho nele: a troca continua sozinha.",
            c.nome
        );
        a.acao = "Ver caixas".into();
        avisos.push(a);
    }
    // Alterações recebidas guardadas sem aplicar (o gerente olha).
    let (pendentes, origens): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), COUNT(DISTINCT origem) FROM sync_pendencia WHERE lado = 'recebida' AND estado = 'pendente'",
        [],
        |l| Ok((l.get(0)?, l.get(1)?)),
    )?;
    if pendentes > 0 {
        let mut a = Aviso::simples(
            "pendencias",
            &hoje,
            "Pendências da troca de dados".into(),
            format!(
                "{} de {} guardada{} sem aplicar",
                plural(pendentes, "alteração", "alterações"),
                plural(origens.max(1), "caixa", "caixas"),
                if pendentes == 1 { "" } else { "s" }
            ),
            String::new(),
            "configuracoes",
        )
        .categoria(ATENCAO)
        .secao("caixas");
        a.descricao = "Alterações vindas de outros caixas que este não conseguiu aplicar. Elas estão guardadas \
             inteiras (nada se perdeu) e o Rustinho tenta de novo sozinho. O gerente vê o motivo de cada uma \
             em Configurações › Caixas da loja."
            .into();
        a.acao = "Ver pendências".into();
        avisos.push(a);
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
    if !b.erro.is_empty() || sem_backup >= backup::DIAS_ATE_AVISAR {
        let (texto, atraso) = if !b.erro.is_empty() {
            ("O último backup não deu certo".to_string(), curto(&b.erro))
        } else if b.ultimo.is_empty() {
            ("Ainda não teve nenhum backup".to_string(), String::new())
        } else {
            (format!("Sem backup há {sem_backup} dias"), String::new())
        };
        avisos.push(
            Aviso::simples("backup", &hoje, "Backup".into(), texto, atraso, "configuracoes")
                .categoria(ATENCAO)
                .secao("backup"),
        );
    }
    if config::ligado(conn, "backup_nuvem")? {
        let referencia = if b.nuvem_ultimo.is_empty() { &b.nuvem_desde } else { &b.nuvem_ultimo };
        let sem_nuvem = if referencia.is_empty() { 0 } else { dias_desde(referencia)? };
        if sem_nuvem >= backup::DIAS_ATE_AVISAR {
            avisos.push(
                Aviso::simples(
                    "backup_nuvem",
                    &hoje,
                    "Backup".into(),
                    format!("Nada chega ao Google Drive há {sem_nuvem} dias"),
                    curto(&b.nuvem_erro),
                    "configuracoes",
                )
                .categoria(ATENCAO)
                .secao("backup"),
            );
        }
    }
    // Crítica primeiro, depois atenção, depois informativa (ordem estável).
    let peso = |c: &str| match c {
        CRITICA => 0,
        ATENCAO => 1,
        _ => 2,
    };
    avisos.sort_by_key(|a| peso(&a.categoria));
    // A informativa escondida hoje fica de fora (volta à meia-noite, se a
    // condição ainda existir). A regra é a categoria, só aqui.
    let mut visiveis = Vec::with_capacity(avisos.len());
    for a in avisos {
        if !escondida_hoje(conn, &a, &hoje)? {
            visiveis.push(completar(conn, a)?);
        }
    }
    Ok(visiveis)
}

/// A notificação de um caixa sem compatibilidade confirmada (Atenção). Os
/// números e nomes vêm do estado real registrado.
fn aviso_de_compatibilidade(hoje: &str, c: &crate::sync::CaixaConhecido) -> Aviso {
    let dias = c.dias_sem_confirmar;
    let (curto, descricao, situacao) = match c.situacao.as_str() {
        "vencido" => (
            format!("{} desatualizado há {} dias", c.nome, dias.unwrap_or(0)),
            format!(
                "O {} está há {} dias sem confirmar compatibilidade. Novos rascunhos de ordens de serviço estão \
                 bloqueados até a confirmação ou a retirada formal do caixa.",
                c.nome,
                dias.unwrap_or(0)
            ),
            "Sem confirmação de compatibilidade (passou o prazo)".to_string(),
        ),
        "antigo" => (
            format!("{} com versão antiga", c.nome),
            format!(
                "O {} está com uma versão do Rustinho de antes da 0.9.15. Novos rascunhos de ordens de serviço \
                 estão bloqueados até ele ser atualizado ou retirado formalmente da loja.",
                c.nome
            ),
            "Versão antiga do Rustinho".to_string(),
        ),
        _ => (
            format!("{} sem versão conhecida", c.nome),
            format!(
                "O {} aparece nos dados da loja, mas nunca confirmou a versão com este caixa. Novos rascunhos de \
                 ordens de serviço estão bloqueados até a confirmação ou a retirada formal do caixa.",
                c.nome
            ),
            "Versão desconhecida".to_string(),
        ),
    };
    let ultima = if c.compativel_em.is_empty() {
        "nunca".to_string()
    } else {
        let quando = c.compativel_em_texto.clone();
        match dias {
            Some(d) => format!("{quando} (há {d} dias)"),
            None => quando,
        }
    };
    let mut a = Aviso::simples(
        &format!("compatibilidade.{}", c.dispositivo),
        hoje,
        "caixa desatualizado".into(),
        String::new(),
        String::new(),
        "configuracoes",
    )
    .categoria(ATENCAO)
    .secao("caixas");
    a.titulo_longo = format!("Atenção: {curto}");
    a.resumo = format!("Atenção: {curto} — rascunhos de OS bloqueados.");
    a.descricao = descricao;
    a.detalhes = vec![
        Detalhe { rotulo: "Caixa".into(), valor: c.nome.clone() },
        Detalhe { rotulo: "Última confirmação".into(), valor: ultima },
        Detalhe { rotulo: "Situação".into(), valor: situacao },
        Detalhe { rotulo: "Impacto".into(), valor: "Bloqueio de novos rascunhos de ordens de serviço".into() },
        Detalhe {
            rotulo: "O que fazer".into(),
            valor: "Atualizar o Rustinho nesse caixa e deixar trocar dados, ou retirá-lo da loja.".into(),
        },
    ];
    a.acao = "Ver caixas".into();
    a
}

/// Motivo curto e seguro para a faixa (que aceita um pouco de HTML).
fn curto(t: &str) -> String {
    let t: String = t.chars().take(90).collect();
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// A3: o aviso do modo seguro (banco com defeito), crítico, com o que fazer.
fn do_modo_seguro(hoje: &str, defeito: &str) -> Aviso {
    let mut a = Aviso::simples(
        "banco_defeito",
        hoje,
        "Modo seguro: o banco de dados tem defeito".into(),
        "nada é gravado neste caixa".into(),
        "restaure um backup bom ou chame o suporte".into(),
        "configuracoes",
    )
    .categoria(CRITICA)
    .secao("backup");
    a.descricao = format!(
        "Ao abrir, o Rustinho conferiu o banco de dados deste caixa e achou um defeito ({defeito}). Para não \
         piorar o estrago, ele está em modo seguro: dá para consultar, mas nada é gravado (vendas, \
         recebimentos, estoque, caixa, cadastros) e a troca de dados com os outros caixas está parada. Não \
         continue a operar neste computador. O arquivo do banco fica como está e os backups bons não são \
         apagados. Restaure um backup bom em Configurações › Backup (depois feche e abra o Rustinho) ou \
         chame o suporte. Uma cópia feita agora é só para o suporte: pode estar incompleta e não serve como \
         backup."
    );
    a
}

/// RVI-03: no modo seguro, um banco de versão anterior (que não foi
/// atualizado, porque tem defeito) pode não ter tudo o que os outros avisos
/// leem. Se eles não puderem ser montados, o Início mostra pelo menos o aviso
/// do modo seguro.
pub fn so_do_modo_seguro(conn: &Connection, defeito: &str) -> Vec<Aviso> {
    let hoje = fiscal::hoje(conn).unwrap_or_else(|_| crate::tempo::agora().chars().take(10).collect());
    vec![do_modo_seguro(&hoje, defeito)]
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

        esconder(c, &do_dia(c).unwrap(), "receber").unwrap();
        assert!(do_dia(c).unwrap().is_empty(), "escondido hoje");
        config::gravar(c, "aviso_escondido.receber", "2026-01-01").unwrap();
        assert_eq!(do_dia(c).unwrap().len(), 1, "volta no outro dia");
    }

    /// Uma loja com uma notificação de cada categoria: informativa (parcela
    /// vencida), atenção (backup atrasado) e crítica (outra loja recusada).
    fn loja_com_as_tres(b: &Banco) {
        let c = &b.conn;
        let d = &b.dispositivo;
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        let mut cli = pessoas::ficha_nova(c, "cliente").unwrap();
        cli.nome = "Oficina do Zé".into();
        let ze = pessoas::salvar(c, d, &u, &cli).unwrap();
        receber::lancar(
            c,
            d,
            &u,
            &LancamentoEntrada {
                cliente_id: ze,
                descricao: "Caderno".into(),
                valor: "50".into(),
                parcelas: 1,
                primeiro_vencimento: String::new(),
            },
        )
        .unwrap();
        c.execute("UPDATE conta_receber SET vencimento = date('now', '-3 days')", []).unwrap();
        config::gravar(c, "backup_desde", &crate::tempo::dias_atras(3)).unwrap();
        config::gravar(
            c,
            crate::loja::OUTRA_LOJA,
            &serde_json::json!({ "nome": "Caixa da outra loja", "endereco": "192.168.0.9", "quando": crate::tempo::agora() })
                .to_string(),
        )
        .unwrap();
    }

    fn categoria_de<'a>(lista: &'a [Aviso], id: &str) -> Option<&'a str> {
        lista.iter().find(|a| a.id == id).map(|a| a.categoria.as_str())
    }

    /// N-01: só a informativa pode ser escondida, e quem decide é a
    /// categoria. Atenção e crítica recusam e continuam na lista.
    #[test]
    fn n01_esconder_segue_a_categoria() {
        let b = Banco::em_memoria().unwrap();
        loja_com_as_tres(&b);
        let c = &b.conn;
        let antes = do_dia(c).unwrap();
        assert_eq!(categoria_de(&antes, "receber"), Some(INFORMATIVA), "{antes:?}");
        assert_eq!(categoria_de(&antes, "backup"), Some(ATENCAO));
        assert_eq!(categoria_de(&antes, "outra_loja"), Some(CRITICA));
        for a in &antes {
            let r = esconder(c, &antes, &a.id);
            assert_eq!(r.is_ok(), a.categoria == INFORMATIVA, "{} ({}): {r:?}", a.id, a.categoria);
        }
        let depois = do_dia(c).unwrap();
        assert_eq!(categoria_de(&depois, "receber"), None, "a informativa saiu até a meia-noite");
        assert_eq!(categoria_de(&depois, "backup"), Some(ATENCAO), "atenção continua");
        assert_eq!(categoria_de(&depois, "outra_loja"), Some(CRITICA), "crítica continua");
    }

    /// N-02: nada esconde uma atenção ou uma crítica, nem uma marca de
    /// "escondida" que esteja no banco (de outra versão, de um backup
    /// restaurado): a regra é a categoria, num lugar só.
    #[test]
    fn n02_marca_de_escondida_nao_some_com_atencao_nem_critica() {
        let b = Banco::em_memoria().unwrap();
        loja_com_as_tres(&b);
        let c = &b.conn;
        let hoje = fiscal::hoje(c).unwrap();
        for id in ["backup", "outra_loja", "troca_voltou"] {
            config::gravar(c, &format!("aviso_escondido.{id}"), &hoje).unwrap();
        }
        let v = do_dia(c).unwrap();
        assert_eq!(categoria_de(&v, "backup"), Some(ATENCAO), "{v:?}");
        assert_eq!(categoria_de(&v, "outra_loja"), Some(CRITICA), "{v:?}");
    }

    /// N-03: a informativa escondida volta no outro dia (meia-noite) só se a
    /// condição ainda existir.
    #[test]
    fn n03_escondida_volta_no_outro_dia_so_se_ainda_vale() {
        let b = Banco::em_memoria().unwrap();
        loja_com_as_tres(&b);
        let c = &b.conn;
        esconder(c, &do_dia(c).unwrap(), "receber").unwrap();
        assert_eq!(categoria_de(&do_dia(c).unwrap(), "receber"), None, "hoje, escondida");
        // Ontem pelo relógio da loja (a mesma data de `fiscal::hoje`), não em
        // UTC: das 21h à meia-noite de Brasília, o "ontem" UTC já é hoje.
        let ontem: String = c.query_row("SELECT date('now', 'localtime', '-1 day')", [], |l| l.get(0)).unwrap();
        config::gravar(c, "aviso_escondido.receber", &ontem).unwrap();
        assert_eq!(categoria_de(&do_dia(c).unwrap(), "receber"), Some(INFORMATIVA), "outro dia: volta");
        // Resolvida (a parcela foi cancelada): não volta mais.
        c.execute("UPDATE conta_receber SET cancelado_em = datetime('now')", []).unwrap();
        config::gravar(c, "aviso_escondido.receber", &ontem).unwrap();
        assert_eq!(categoria_de(&do_dia(c).unwrap(), "receber"), None, "sem a condição, não volta");
    }

    /// N-04: minimizar é só apresentação: a notificação continua, nada de
    /// fora das marcas deste computador muda e esconder não acontece.
    /// Informativa e atenção começam minimizadas (o quadrado); a crítica não
    /// se minimiza — nem pelo comando, nem por uma marca antiga no banco.
    #[test]
    fn n04_minimizar_e_so_apresentacao() {
        let b = Banco::em_memoria().unwrap();
        loja_com_as_tres(&b);
        let c = &b.conn;
        let contas: i64 =
            c.query_row("SELECT COUNT(*) FROM conta_receber WHERE cancelado_em IS NULL", [], |l| l.get(0)).unwrap();
        let minimizada = |id: &str| do_dia(c).unwrap().iter().find(|a| a.id == id).map(|a| a.minimizado);
        assert_eq!(minimizada("receber"), Some(true), "informativa começa minimizada");
        assert_eq!(minimizada("backup"), Some(true), "atenção começa minimizada");
        assert_eq!(minimizada("outra_loja"), Some(false), "crítica sempre expandida");
        for id in ["receber", "backup"] {
            minimizar(c, &do_dia(c).unwrap(), id, false).unwrap();
            assert_eq!(minimizada(id), Some(false), "{id} expandida");
            minimizar(c, &do_dia(c).unwrap(), id, true).unwrap();
            assert_eq!(minimizada(id), Some(true), "{id} minimizada de novo");
        }
        assert!(minimizar(c, &do_dia(c).unwrap(), "outra_loja", true).is_err(), "crítica não minimiza");
        config::gravar(c, "aviso_minimizado.outra_loja", "1").unwrap();
        assert_eq!(minimizada("outra_loja"), Some(false), "marca antiga não minimiza a crítica");
        let depois: i64 =
            c.query_row("SELECT COUNT(*) FROM conta_receber WHERE cancelado_em IS NULL", [], |l| l.get(0)).unwrap();
        assert_eq!(contas, depois, "minimizar não mexe na condição");
        let marcas: i64 =
            c.query_row("SELECT COUNT(*) FROM config WHERE chave LIKE 'aviso_escondido.%'", [], |l| l.get(0)).unwrap();
        assert_eq!(marcas, 0, "minimizar não esconde");
    }

    /// N-05: uma notificação por condição (o id é da condição: atualizar muda
    /// a mesma, não cria outra).
    #[test]
    fn n05_uma_notificacao_por_condicao() {
        let b = Banco::em_memoria().unwrap();
        loja_com_as_tres(&b);
        let c = &b.conn;
        let ids = |v: &[Aviso]| v.iter().map(|a| a.id.clone()).collect::<Vec<_>>();
        let primeira = ids(&do_dia(c).unwrap());
        let mut unicos = primeira.clone();
        unicos.sort();
        unicos.dedup();
        assert_eq!(primeira.len(), unicos.len(), "{primeira:?}");
        // A condição piora (mais uma parcela vencida): a mesma notificação muda.
        c.execute(
            "INSERT INTO conta_receber (id, cliente_id, descricao, valor, vencimento, parcela, parcelas, dispositivo, criado_em)
             SELECT 'outra', cliente_id, descricao, valor, vencimento, 1, 1, dispositivo, criado_em FROM conta_receber LIMIT 1",
            [],
        )
        .unwrap();
        let segunda = do_dia(c).unwrap();
        assert_eq!(ids(&segunda), primeira, "mesmas notificações");
        assert!(segunda.iter().find(|a| a.id == "receber").unwrap().atraso.contains("R$ 100,00"));
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
