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
use serde_json::Value;

use crate::erro::{Erro, Resultado};
use crate::{backup, config, fiscal, numeros};

pub const INFORMATIVA: &str = "informativa";
pub const ATENCAO: &str = "atencao";
pub const CRITICA: &str = "critica";

/// T-07 (C8): horas sem trocar dados com um caixa da loja até a notificação
/// de atenção. Um dia inteiro: o caixa desligado à noite ou num feriado não
/// avisa; o que ficou fora da rede por mais tempo, sim.
pub const HORAS_SEM_CONTATO: i64 = 24;

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
    /// C8 (9a, 9c, 10a; dev3.3): o registro exato, quando a notificação é sobre UM só
    /// ("conflito:<id>", "codigo:<tipo>|<código>", "caixa:<dispositivo>", "referencia:<tabela>|<chave>",
    /// "pendencia:<id>", "encomenda:<id>"). A tela o destaca; vazio = a lista (mais de um, ou sem registro).
    pub alvo: String,
    /// QT-1b: uma 2ª ação, que resolve o aviso pela pessoa ("Conferi"), pelo comando `aviso_resolver` (vazio = não há).
    pub resolver: String,
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
            alvo: String::new(),
            resolver: String::new(),
            minimizado: false,
        }
    }

    fn alvo(mut self, alvo: String) -> Aviso {
        self.alvo = alvo;
        self
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

/// 9a (C8): o conflito aberto, quando há um só (senão, a lista).
fn um_conflito(conn: &Connection) -> Resultado<String> {
    let abertos: Vec<i64> =
        crate::sync::conflitos(conn)?.into_iter().filter(|c| c.resolvido.is_empty()).map(|c| c.id).collect();
    Ok(match abertos.as_slice() {
        [id] => format!("conflito:{id}"),
        _ => String::new(),
    })
}

/// 9a (C8): o código repetido, quando é o único caso a ver em Duplicatas.
fn um_codigo(conn: &Connection) -> Resultado<String> {
    Ok(match crate::faixas::repetidos(conn)?.as_slice() {
        [r] => format!("codigo:{}|{}", r.tipo, r.codigo),
        _ => String::new(),
    })
}

/// 9c (C8): o registro incompleto (apagado ou comprovadamente inconsistente), quando há um só.
fn uma_referencia(conn: &Connection) -> Resultado<String> {
    let quebradas: Vec<_> = crate::sync::referencias(conn)?
        .into_iter()
        .filter(|r| r.estado == "apagado" || r.estado == "inconsistente")
        .collect();
    Ok(match quebradas.as_slice() {
        [r] => format!("referencia:{}|{}", r.tabela, r.chave),
        _ => String::new(),
    })
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
            "encomendas" | "encomendas_numero_repetido" => "Ver encomendas",
            "oficina" => "Ver ordens de serviço",
            // Caso 8 (C8, 0.9.18): o modo seguro leva ao Backup (restaurar).
            "backup" | "backup_nuvem" | "backup_sem_tranca" | "backup_chave_nao_confirmada" | "banco_defeito" => {
                "Ver backup"
            }
            // NOTIF-01 (0.9.18): o botão abre Simultaneidade › Duplicatas.
            "conflitos" | "codigos" | "codigos_bloqueio" => "Ver duplicatas",
            "referencias" | "referencias_atrasadas" | "outra_loja" => "Ver caixas",
            "xml_pendencias" => "Resolver pendências",
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
    minimizar_por(conn, atuais, id, minimizado, None)
}

/// A marca de "minimizado" de um aviso. N-05 da re-revisão: a do aviso de fechamento inesperado é POR PESSOA (como
/// o "visto"): o caixa minimizar não faz o gerente receber o aviso já minimizado. As outras são deste computador.
pub fn chave_minimizado(id: &str, usuario_id: Option<&str>) -> String {
    match usuario_id {
        Some(u) if id.starts_with(FECHAMENTO_INESPERADO) || id.starts_with(QUEDA_SEM_PENDENCIA) => {
            format!("aviso_minimizado.{id}.{u}")
        }
        _ => format!("aviso_minimizado.{id}"),
    }
}

/// `minimizar`, sabendo quem está usando (para a marca por pessoa do aviso de fechamento inesperado).
pub fn minimizar_por(
    conn: &Connection,
    atuais: &[Aviso],
    id: &str,
    minimizado: bool,
    usuario_id: Option<&str>,
) -> Resultado<()> {
    let Some(aviso) = atuais.iter().find(|a| a.id == id) else {
        return Err(Erro::validacao("Esta notificação já não está aparecendo."));
    };
    if minimizado && aviso.categoria == CRITICA {
        return Err(Erro::validacao("A notificação crítica fica aberta até o problema ser resolvido."));
    }
    for outra in atuais.iter().filter(|a| a.id != id && a.categoria != CRITICA) {
        config::gravar(conn, &chave_minimizado(&outra.id, usuario_id), "1")?;
    }
    config::gravar(conn, &chave_minimizado(id, usuario_id), if minimizado { "1" } else { "0" })
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

/// QT-1b: o aviso de fechamento inesperado, para quem entrou. Para as aberturas do programa que caíram (não
/// fecharam pelo caminho normal, ou tiveram erro interno) e que esta pessoa não conferiu, mostra o que FOI GRAVADO
/// e a tela não chegou a mostrar, o que NÃO foi concluído (nada gravado) e o que precisa ser CONFERIDO (operação de
/// várias transações). Nunca diz que um rascunho ou formulário foi salvo, e não repete nada: a pessoa confere e dá
/// "Conferi". O id leva o número da abertura mais nova (`fechamento_inesperado.<n>`): uma queda nova começa aberta;
/// minimizar pega (passa por `completar`, como os outros).
pub fn do_fechamento(conn: &Connection, usuario_id: &str, gerente: bool) -> Resultado<Option<Aviso>> {
    let atual = crate::operacoes::abertura_atual(conn)?;
    let Some((desde, ultima, linhas)) = crate::operacoes::fechamento(conn, usuario_id, gerente, atual)? else {
        return Ok(None);
    };
    // Os momentos são gravados em UTC; o aviso mostra a hora do computador, e o dia quando não é hoje (F-12).
    let quando = |q: &str| -> String {
        conn.query_row(
            "SELECT CASE WHEN date(?1, 'localtime') = date('now', 'localtime') THEN strftime('%H:%M', ?1, 'localtime')
                         ELSE strftime('%d/%m %H:%M', ?1, 'localtime') END",
            [q],
            |l| l.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
        .unwrap_or_default()
    };
    let dia_hora = |q: &str| -> String {
        conn.query_row("SELECT strftime('%d/%m às %H:%M', ?1, 'localtime')", [q], |l| l.get::<_, Option<String>>(0))
            .ok()
            .flatten()
            .unwrap_or_default()
    };
    let detalhes = linhas
        .iter()
        .map(|l| {
            let mut partes = vec![nome_da_operacao(&l.comando)];
            let r = l.resultado.clone().unwrap_or(Value::Null);
            let numero = r.get("numero").and_then(Value::as_i64).filter(|n| *n > 0);
            let total = r.get("total").and_then(Value::as_str).filter(|s| !s.is_empty());
            if let Some(n) = numero {
                partes.push(format!("nº {n}"));
            }
            if let Some(t) = total {
                partes.push(format!("R$ {t}"));
            }
            // F-12: quando a resposta não identifica a operação, o tipo e o valor do pedido.
            if numero.is_none() && total.is_none() && !l.resumo.is_empty() {
                partes.push(l.resumo.clone());
            }
            partes.push(quando(&l.quando));
            let (rotulo, fim) = match l.classe {
                "gravada" => ("Gravada", ""),
                "confira" => ("Confira", " · pode ter gravado só uma parte"),
                _ => ("Não concluída", " · nada foi gravado"),
            };
            Detalhe { rotulo: rotulo.to_string(), valor: format!("{}{fim}", partes.join(" · ")) }
        })
        .collect::<Vec<_>>();
    let id = format!("{FECHAMENTO_INESPERADO}.{ultima}");
    let mut a = Aviso::simples(
        &id,
        &fiscal::hoje(conn)?,
        "Fechamento inesperado".into(),
        "o Rustinho fechou sem o encerramento normal · confira antes de refazer".into(),
        String::new(),
        "caixa",
    )
    .categoria(ATENCAO);
    a.titulo_longo = "Atenção: o Rustinho fechou sem o encerramento normal".into();
    a.descricao = format!(
        "Desde {}, o Rustinho não fechou pelo caminho normal. Veja abaixo o que foi gravado sem a tela confirmar e o \
         que não foi gravado, e confira antes de refazer qualquer venda, recebimento ou pagamento. O que estava só na \
         tela (uma venda sendo montada, um formulário) não foi guardado.",
        dia_hora(&desde)
    );
    a.detalhes = detalhes;
    a.acao = "Ver vendas do caixa".into();
    a.resolver = "Conferi".into();
    // F-05 (revisão): pelo mesmo caminho dos outros (resumo, ação); começa ABERTO (uma queda nova precisa ser
    // vista) e respeita o minimizar da pessoa.
    let mut a = completar(conn, a)?;
    // N-05 da re-revisão: o minimizar desta pessoa (o de outra não vale para ela).
    a.minimizado = config::ler(conn, &chave_minimizado(&id, Some(usuario_id)))?.as_deref() == Some("1");
    Ok(Some(a))
}

/// O começo do id do aviso de fechamento inesperado (o resto é o número da abertura).
pub const FECHAMENTO_INESPERADO: &str = "fechamento_inesperado";

/// S2 (decisão de 15/11/2026; ON-03 da revisão do QT-1): depois de uma queda SEM nada pendente (o caso mais comum:
/// uma venda sendo montada), uma linha informativa, **só no computador** (no Android, onde a morte em segundo plano
/// é comum, nada). Só para a queda vista hoje; "Entendi" tira para esta pessoa (como o "Conferi"); minimizar é por
/// pessoa. Não diz que nada se perdeu: o que estava só na tela não foi guardado.
pub fn da_queda_sem_pendencia(conn: &Connection, usuario_id: &str) -> Resultado<Option<Aviso>> {
    if cfg!(target_os = "android") {
        return Ok(None);
    }
    let atual = crate::operacoes::abertura_atual(conn)?;
    let Some((ultima, vista)) = crate::operacoes::queda_sem_pendencia(conn, usuario_id, atual)? else {
        return Ok(None);
    };
    let hora = conn
        .query_row("SELECT strftime('%H:%M', ?1, 'localtime')", [&vista], |l| l.get::<_, Option<String>>(0))?
        .unwrap_or_default();
    let id = format!("{QUEDA_SEM_PENDENCIA}.{ultima}");
    let mut a = Aviso::simples(
        &id,
        &fiscal::hoje(conn)?,
        "Fechamento inesperado".into(),
        "nada ficou para conferir · o que estava só na tela não foi guardado".into(),
        String::new(),
        "caixa",
    );
    a.titulo_longo = "O Rustinho fechou sem o encerramento normal".into();
    a.descricao = format!(
        "O Rustinho fechou sem o encerramento normal e foi aberto de novo às {hora}. Nenhuma venda, recebimento ou \
         pagamento ficou para conferir. O que estava só na tela (uma venda sendo montada, um formulário) não foi \
         guardado."
    );
    a.acao = "Ver vendas do caixa".into();
    a.resolver = "Entendi".into();
    let mut a = completar(conn, a)?;
    a.minimizado = config::ler(conn, &chave_minimizado(&id, Some(usuario_id)))?.as_deref() == Some("1");
    Ok(Some(a))
}

/// O começo do id da linha informativa do S2 (o resto é o número da abertura).
pub const QUEDA_SEM_PENDENCIA: &str = "queda_sem_pendencia";

/// O nome de cada operação no aviso de fechamento inesperado. Todo comando de `NAO_REPETIR` (menos as
/// exceções do R0) tem o seu (teste `todo_comando_que_nao_se_repete_tem_nome_no_aviso`).
pub const NOMES_DAS_OPERACOES: &[(&str, &str)] = &[
    ("caixa_abrir", "Abertura do caixa"),
    ("caixa_fechar", "Fechamento do caixa"),
    ("caixa_movimentar", "Movimento do caixa"),
    ("caixa_readmitir", "Caixa readmitido na loja"),
    ("caixa_retirar", "Caixa retirado da loja"),
    ("categoria_salvar", "Categoria do financeiro"),
    ("cheque_depositar", "Depósito de cheque"),
    ("cheque_desfazer", "Cheque desfeito (depósito, repasse ou devolução)"),
    ("cheque_devolver", "Cheque devolvido"),
    ("cheque_repassar", "Cheque repassado"),
    ("comissao_fechar", "Fechamento de comissão"),
    ("comissao_reabrir", "Comissão reaberta"),
    ("compra_estornar", "Estorno de compra"),
    ("compra_importar", "Importação de compras"),
    ("compra_lancar", "Compra lançada"),
    ("conflito_compensar", "Compensação de conflito entre caixas"),
    ("conta_cancelar", "Dívida tirada do crediário"),
    ("conta_loja_salvar", "Conta da loja"),
    ("devolucao_desfazer", "Devolução desfeita"),
    ("devolucao_gravar", "Devolução"),
    ("encomenda_cancelar", "Encomenda cancelada"),
    ("encomenda_chegou", "Encomenda que chegou"),
    ("encomenda_criar", "Encomenda"),
    ("encomenda_entregar", "Encomenda entregue"),
    ("encomenda_pedir", "Encomenda pedida"),
    ("estoque_ajustar", "Ajuste de estoque"),
    ("importar_cadastros", "Importação de cadastros"),
    ("movimento_estornar", "Estorno de movimento da conta"),
    ("movimento_lancar", "Movimento da conta"),
    ("orcamento_salvar", "Orçamento"),
    ("os_cancelar", "Cancelamento de OS"),
    ("os_rascunho_excluir", "Rascunho de OS excluído"),
    ("os_rascunho_salvar", "Rascunho de OS"),
    ("os_salvar", "Ordem de serviço"),
    ("os_situacao", "Situação da OS"),
    ("pagamento_estornar", "Estorno de pagamento"),
    ("pagar_cancelar", "Conta a pagar tirada"),
    ("pagar_lancar", "Conta a pagar"),
    ("pagar_pagar", "Pagamento de conta"),
    ("pendencia_arquivar", "Pendência arquivada"),
    ("pessoa_salvar", "Cadastro de pessoa"),
    ("produto_salvar", "Cadastro de peça"),
    ("produto_variacao_criar", "Variação de peça"),
    ("receber_baixar", "Recebimento"),
    ("receber_lancar", "Crediário"),
    ("recebimento_estornar", "Estorno de recebimento"),
    ("transferir", "Transferência"),
    ("unidade_salvar", "Unidade"),
    ("usuario_salvar", "Cadastro de usuário"),
    ("venda_cancelar", "Cancelamento de venda"),
    ("venda_concluir", "Venda"),
    ("xml_pendencia_resolver", "Pendência de XML resolvida"),
];

/// O nome da operação no aviso (um comando sem nome, que não deveria existir, aparece como "Operação").
fn nome_da_operacao(comando: &str) -> String {
    NOMES_DAS_OPERACOES.iter().find(|(c, _)| *c == comando).map(|(_, n)| *n).unwrap_or("Operação").to_string()
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
        // 10a (C8): uma só → a linha dela destacada em Estoque › Encomendas.
        let alvo = if chegaram + a_pedir == 1 {
            conn.query_row(
                "SELECT id FROM encomenda WHERE (situacao = 'chegou' AND avisado_em IS NULL) OR situacao = 'aberta'",
                [],
                |l| l.get::<_, String>(0),
            )
            .map(|id| format!("encomenda:{id}"))
            .unwrap_or_default()
        } else {
            String::new()
        };
        avisos.push(
            Aviso::simples("encomendas", &hoje, "Encomendas".into(), partes.join(" · "), String::new(), "estoque")
                .alvo(alvo),
        );
    }

    // I1 (decisão do proprietário, 15/11; 0915-M): o mesmo número de encomenda em caixas diferentes. Cada caixa
    // numera as suas (o maior + 1), então dois caixas sem conexão dão o mesmo número; depois da troca de dados,
    // aparecem dois "nº X". Só as que ainda estão andando. Leva à encomenda do OUTRO caixa, destacada.
    let repetidas: Vec<(i64, String, String)> = {
        let mut s = conn.prepare(
            "SELECT e.numero, e.id, e.dispositivo FROM encomenda e
              WHERE e.situacao IN ('aberta', 'pedida', 'chegou')
                AND EXISTS (SELECT 1 FROM encomenda o WHERE o.numero = e.numero AND o.dispositivo <> e.dispositivo
                              AND o.situacao IN ('aberta', 'pedida', 'chegou'))
              ORDER BY e.numero, e.criado_em, e.id",
        )?;
        let lista = s.query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?.collect::<Result<Vec<_>, _>>()?;
        lista
    };
    if !repetidas.is_empty() {
        let mut numeros: Vec<i64> = repetidas.iter().map(|r| r.0).collect();
        numeros.dedup();
        let este = config::ler(conn, "dispositivo_id")?.unwrap_or_default();
        let alvo = repetidas.iter().find(|r| r.2 != este).map(|r| format!("encomenda:{}", r.1)).unwrap_or_default();
        let lista = numeros.iter().map(|n| format!("nº {n}")).collect::<Vec<_>>().join(", ");
        avisos.push(
            Aviso::simples(
                "encomendas_numero_repetido",
                &hoje,
                // Um título próprio: o quadrado minimizado não se confunde com o das encomendas pendentes.
                "Número de encomenda repetido".into(),
                format!("Mesmo número em caixas diferentes: {lista}. Confira de qual cliente é cada uma."),
                String::new(),
                "estoque",
            )
            .categoria(ATENCAO)
            .alvo(alvo),
        );
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
            .secao("duplicatas")
            .alvo(if conflitos == 1 { um_conflito(conn)? } else { String::new() }),
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
            .secao("duplicatas")
            .alvo(if codigos == 1 { um_codigo(conn)? } else { String::new() }),
        );
    }

    // 9e (C8, dev3.3): o fiscal da loja. Nunca sugere que o Rustinho emite nota.
    // UF ou CNPJ faltando, numa loja que já usa o fiscal (tem nota de compra pelo XML; as regras de imposto
    // vêm prontas em toda loja e não contam):
    // Auto — o botão leva direto ao campo (navegação guiada) e volta ao Início quando o núcleo confirma.
    let usa_fiscal: bool = conn.query_row("SELECT EXISTS (SELECT 1 FROM compra WHERE xml <> '')", [], |l| l.get(0))?;
    if usa_fiscal {
        let loja = crate::requisitos::conferir(conn, crate::requisitos::UF_E_CNPJ_DA_LOJA)?;
        if !loja.satisfeito {
            let mut a = Aviso::simples(
                "fiscal_loja",
                &hoje,
                "Fiscal".into(),
                "faltam dados da loja para a conferência fiscal".into(),
                String::new(),
                "configuracoes",
            )
            .secao("loja");
            a.descricao = format!(
                "{} O estado e o CNPJ servem para conferir as regras de imposto das notas de compra e para separar \
                 as notas da loja das de fornecedores. O Rustinho não emite nota fiscal.",
                loja.falta
            );
            a.acao = "Informar UF e CNPJ".into();
            avisos.push(a);
        }
    }
    // NF-e de compra cuja autorização na SEFAZ ainda não foi conferida (PEND-09):
    // com 1 nota só, leva direto à conferência dela (`alvo`); com 2+, abre Compras já filtrada.
    let aguardando = crate::compras::contar_aguardando_autorizacao(conn)?;
    if aguardando > 0 {
        let mut a = Aviso::simples(
            "fiscal_notas",
            &hoje,
            "Notas de compra".into(),
            format!(
                "{} aguardando a conferência da autorização",
                plural(aguardando, "nota de compra", "notas de compra")
            ),
            "o gerente confere a chave no portal da NF-e antes de lançar".into(),
            "compras",
        );
        a.acao = "Ver compras".into();
        if aguardando == 1 {
            if let Some(id) = crate::compras::unica_aguardando_autorizacao(conn)? {
                a.alvo = id;
            }
        }
        avisos.push(a);
    }

    // Pendências persistentes de identificação XML (Fase 4.D): ficam visíveis
    // na tela inicial até todas as associações serem salvas.
    // A13: uma falha ao contar não vira "zero pendências": o aviso continua,
    // dizendo que não deu para conferir, e o detalhe vai para o registro técnico.
    let xml_pendentes = crate::importar::contar_pendencias_ou_erro(conn);
    if let Err(e) = &xml_pendentes {
        crate::registro::anotar(&format!("contagem das pendências de identificação XML: {e}"));
    }
    // Decisão 3 do proprietário (25/11/2026): com o descarte possível (R6), as pendências deixam de ser para sempre;
    // o aviso é de atenção, que pode ser minimizado. A falha ao contar continua crítica (é defeito, não pendência).
    let (texto_xml, categoria_xml) = match xml_pendentes {
        Ok(0) => (None, ATENCAO),
        Ok(n) => {
            (Some(format!("{} aguardando associação com peças", plural(n, "item de XML", "itens de XML"))), ATENCAO)
        }
        Err(_) => (Some("não foi possível conferir as pendências agora".to_string()), CRITICA),
    };
    if let Some(texto) = texto_xml {
        let mut a = Aviso::simples(
            "xml_pendencias",
            &hoje,
            "Pendências de identificação XML".into(),
            texto,
            "associe a uma peça existente ou cadastre como peça nova".into(),
            "pecas",
        )
        .categoria(categoria_xml);
        a.acao = "Resolver pendências".into();
        avisos.push(a);
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
        let outro_rustinho =
            |v: &[&(&str, String)]| v.iter().any(|(_, m)| m.contains("outro Rustinho aberto neste computador"));
        let (pt, pu) = crate::portatil::portas();
        let ocupante = crate::sync::ocupante_em_conflito(std::path::Path::new(&banco), pt, pu);
        let detalhes_ocupante = |o: Option<&crate::sync::OcupanteDaPorta>| -> Vec<Detalhe> {
            let Some(o) = o else { return Vec::new() };
            let loja = if o.loja.trim().is_empty() { "Ainda sem nome" } else { o.loja.trim() };
            let caixa = if o.caixa.trim().is_empty() { "Caixa 1" } else { o.caixa.trim() };
            vec![
                Detalhe { rotulo: "Outro Rustinho (loja)".into(), valor: loja.into() },
                Detalhe { rotulo: "Caixa".into(), valor: caixa.into() },
                Detalhe { rotulo: "Pasta dos dados".into(), valor: o.pasta.clone() },
            ]
        };
        if !paradas.is_empty() {
            let (resumo, acao) = if outro_rustinho(&paradas) || ocupante.is_some() {
                let quem =
                    ocupante.as_ref().map(|o| format!(" ({})", crate::sync::resumo_ocupante(o))).unwrap_or_default();
                (
                    format!("a porta da troca de dados está ocupada por outro Rustinho neste computador{quem}"),
                    "feche o outro Rustinho e toque em \"Tentar de novo\"".to_string(),
                )
            } else if porta(&paradas) {
                (
                    "a porta da troca de dados está ocupada por outro programa".to_string(),
                    "feche o outro programa e toque em \"Tentar de novo\" (ou abra o Rustinho de novo); se continuar, chame o suporte".to_string(),
                )
            } else {
                (
                    "um erro interno parou a troca com os outros caixas".to_string(),
                    "toque em \"Tentar de novo\" ou feche e abra o Rustinho; se continuar, chame o suporte".to_string(),
                )
            };
            let mut a =
                Aviso::simples("troca_parada", &hoje, "A troca de dados parou".into(), resumo, acao, "configuracoes")
                    .categoria(CRITICA)
                    .secao("caixas");
            a.descricao = format!(
                "Parou: {}. As vendas e o resto deste caixa continuam normalmente neste computador, mas as alterações \
                 não vão nem vêm dos outros caixas enquanto a porta estiver ocupada. Quando liberar, toque em \
                 \"Tentar de novo\". Nada do que já foi recebido se perdeu.",
                lista(&paradas)
            );
            a.detalhes = detalhes_ocupante(ocupante.as_ref());
            a.resolver = "Tentar de novo".into();
            avisos.push(a);
        } else if !tentando.is_empty() {
            // Não está rodando agora: o Início não pode dizer que está tudo bem.
            let resumo_tentando = if let Some(ref o) = ocupante {
                format!(
                    "a porta está ocupada por outro Rustinho ({}) · tentando de novo",
                    crate::sync::resumo_ocupante(o)
                )
            } else {
                "o Rustinho está tentando de novo".into()
            };
            let mut a = Aviso::simples(
                "troca_tentando",
                &hoje,
                "A troca de dados não está funcionando".into(),
                resumo_tentando,
                String::new(),
                "configuracoes",
            )
            .categoria(ATENCAO)
            .secao("caixas");
            a.descricao = format!(
                "Falhou: {}. O Rustinho tenta de novo sozinho, algumas vezes (ou agora mesmo em \"Tentar de novo\"); \
                 até lá as alterações não vão nem vêm dos outros caixas. As vendas continuam normalmente.",
                lista(&tentando)
            );
            a.detalhes = detalhes_ocupante(ocupante.as_ref());
            a.resolver = "Tentar de novo".into();
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
            .secao("caixas")
            .alvo(uma_referencia(conn)?),
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
        avisos.push(aviso_de_compatibilidade(&hoje, &c).alvo(format!("caixa:{}", c.dispositivo)));
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
        a.alvo = format!("caixa:{}", c.dispositivo);
        avisos.push(a);
    }
    // T-07 (C8): caixa da loja que não troca dados com este há um dia ou
    // mais. Só com a troca de dados ligada; o retirado não conta. Some
    // sozinha na primeira troca que der certo.
    if crate::sync::ajustes(conn).map(|a| a.ligado).unwrap_or(false) {
        for c in caixas_sem_contato(conn, HORAS_SEM_CONTATO)? {
            let alvo = format!("caixa:{}", c.dispositivo);
            avisos.push(aviso_sem_contato(&hoje, &c).alvo(alvo));
        }
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
        if pendentes == 1 {
            a.alvo = conn
                .query_row(
                    "SELECT alteracao_id FROM sync_pendencia WHERE lado = 'recebida' AND estado = 'pendente'",
                    [],
                    |l| l.get::<_, String>(0),
                )
                .map(|id| format!("pendencia:{id}"))
                .unwrap_or_default();
        }
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
    // F02 (passo 6.7, DP-32): já houve backup e a loja ainda não tem a chave
    // (nenhum gerente entrou): as cópias saem sem tranca. O Drive recusa essas
    // cópias, mas a do computador fica aberta; o aviso diz como resolver.
    if !b.ultimo.is_empty() && backup::publica_da_loja(conn)?.is_none() {
        let mut a = Aviso::simples(
            "backup_sem_tranca",
            &hoje,
            "Backup".into(),
            "Os backups ainda saem sem tranca".into(),
            String::new(),
            "configuracoes",
        )
        .categoria(ATENCAO)
        .secao("backup");
        a.descricao = "Enquanto nenhum gerente entrou no Rustinho, a loja não tem a chave que tranca os backups: \
             quem tiver o arquivo abre os dados da loja. Entre uma vez como gerente para criar a chave; daí em \
             diante os backups saem trancados."
            .into();
        avisos.push(a);
    }
    // T099 (N1 da 3ª conferência): a chave dos backups ainda não foi confirmada por um gerente neste computador
    // (depois da atualização, de entrar numa loja, ou de a chave mudar): as cópias antigas não são apagadas, para
    // não perder a única que abre. O disco cresce até um gerente digitar a senha aqui.
    if !b.ultimo.is_empty() && !backup::chave_confirmada(conn)? {
        let mut a = Aviso::simples(
            "backup_chave_nao_confirmada",
            &hoje,
            "Backup".into(),
            "As cópias antigas do backup não estão sendo apagadas".into(),
            String::new(),
            "configuracoes",
        )
        .categoria(ATENCAO)
        .secao("backup");
        a.descricao = "Nenhum gerente confirmou ainda, neste computador, a chave que tranca os backups. Para não \
             perder a única cópia que abre, o Rustinho guarda todas as antigas. Basta um gerente digitar a senha \
             dele aqui (para entrar ou para liberar uma tela). Se o aviso continuar, a chave foi trocada em outro \
             caixa: entre aqui com um gerente que entrou lá depois da troca."
            .into();
        avisos.push(a);
    }
    // T010 (A4): enquanto os caixas antigos (protocolo 3) estiverem liberados, o
    // Início lembra todo dia até quando, para atualizá-los antes do prazo.
    if let Some(ate) = crate::sync::convivencia_p3_ate(conn)? {
        let data = crate::fiscal::data_br(&ate);
        let mut a = Aviso::simples(
            "p4.convivencia",
            &hoje,
            "Caixas antigos liberados".to_string(),
            format!("Até {data}"),
            String::new(),
            "configuracoes",
        );
        a.categoria = ATENCAO.into();
        a.descricao = format!(
            "Os caixas com versão antiga do Rustinho ainda trocam dados com este caixa até {data} (ele serve de \
             ponte para os outros). Atualize esses caixas antes disso: depois, só os caixas atualizados trocam dados."
        );
        avisos.push(a);
    }
    // T013 (DT-P4-41, DT-P4-10): este caixa da loja ainda não foi aprovado no
    // protocolo 4 e já há caixas atualizados na rede: sem a aprovação (uma vez,
    // pelo convite do gerente), ele não troca dados com eles.
    if crate::sync::precisa_de_aprovacao(conn)? {
        let mut a = Aviso::simples(
            "p4.aprovar_este_caixa",
            &hoje,
            "Este caixa precisa ser aprovado".to_string(),
            "Peça um convite ao gerente".to_string(),
            String::new(),
            "configuracoes",
        )
        .categoria(ATENCAO)
        .secao("caixas");
        a.descricao = "Os outros caixas da loja já estão atualizados e só trocam dados com caixas aprovados pelo \
                       gerente. Num caixa aprovado, o gerente gera um convite com o código deste caixa; depois, \
                       digite o convite em Configurações › Caixas da loja."
            .to_string();
        a.acao = "Ver caixas".into();
        avisos.push(a);
    }
    // C2R4-1: o registro de caixas da loja deixou aprovações de fora por falta de lugar
    // (um caixa aprovou caixas demais: pode ser um computador roubado). Retirar esse
    // caixa no primeiro caixa da loja devolve o lugar, e o aviso sai sozinho.
    if crate::sync::registro_de_caixas_cheio(conn)? {
        let mut a = Aviso::simples(
            "p4.registro_cheio",
            &hoje,
            "Registro de caixas cheio".to_string(),
            "Confira os caixas da loja".to_string(),
            String::new(),
            "configuracoes",
        )
        .categoria(ATENCAO)
        .secao("caixas");
        a.descricao = crate::troca::protocolo4::aprovados::TEXTO_REGISTRO_CHEIO.to_string();
        a.acao = "Ver caixas".into();
        avisos.push(a);
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

/// Um caixa da loja que já trocou dados com este e parou (T-07).
struct SemContato {
    dispositivo: String,
    nome: String,
    horas: i64,
    /// Última troca que deu certo, no horário daqui ("03/11/2026 10:32").
    ultima: String,
    visto: String,
    erro: String,
}

/// Os caixas (não retirados) cuja última troca que deu certo passou de
/// `horas`. O que nunca trocou dados não entra: ele aparece como "nunca"
/// em Configurações › Caixas da loja e nos avisos da troca.
fn caixas_sem_contato(conn: &Connection, horas: i64) -> Resultado<Vec<SemContato>> {
    let mut c = conn.prepare(
        "SELECT dispositivo, nome, segundos / 3600,
                COALESCE(strftime('%d/%m/%Y %H:%M', sincronizado_em, 'localtime'), ''),
                COALESCE(strftime('%d/%m/%Y %H:%M', visto_em, 'localtime'), ''),
                erro
           FROM (SELECT *, CAST(strftime('%s', 'now') AS INTEGER) - CAST(strftime('%s', sincronizado_em) AS INTEGER)
                           AS segundos
                   FROM sync_par WHERE COALESCE(sincronizado_em, '') <> '')
          WHERE segundos >= ?1 * 3600
          ORDER BY nome, dispositivo",
    )?;
    let todos: Vec<SemContato> = c
        .query_map([horas], |l| {
            Ok(SemContato {
                dispositivo: l.get(0)?,
                nome: l.get(1)?,
                horas: l.get(2)?,
                ultima: l.get(3)?,
                visto: l.get(4)?,
                erro: l.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    let mut lista = Vec::new();
    for s in todos {
        if !crate::sync::retirado(conn, &s.dispositivo)? {
            lista.push(s);
        }
    }
    Ok(lista)
}

/// A notificação de um caixa sem trocar dados (Atenção, T-07).
fn aviso_sem_contato(hoje: &str, c: &SemContato) -> Aviso {
    let nome = if c.nome.trim().is_empty() { "Um caixa da loja".to_string() } else { c.nome.clone() };
    let tempo = if c.horas < 48 { plural(c.horas, "hora", "horas") } else { plural(c.horas / 24, "dia", "dias") };
    let mut a = Aviso::simples(
        &format!("sem_contato.{}", c.dispositivo),
        hoje,
        "caixa sem contato".into(),
        format!("{nome} sem trocar dados há {tempo}"),
        String::new(),
        "configuracoes",
    )
    .categoria(ATENCAO)
    .secao("caixas");
    a.titulo_longo = format!("Atenção: {nome} sem trocar dados há {tempo}");
    a.resumo = format!("Atenção: {nome} sem trocar dados há {tempo}.");
    a.descricao = format!(
        "{nome} não troca dados com este caixa desde {}. Enquanto isso, o que um vende, cadastra ou muda não \
         chega ao outro: estoque, preços e contas podem ficar diferentes entre os dois. Confira se ele está \
         ligado, com o Rustinho aberto e na mesma rede. Se ele não for mais usado, retire-o da loja em \
         Configurações › Caixas da loja.",
        c.ultima
    );
    a.detalhes = vec![
        Detalhe { rotulo: "Caixa".into(), valor: nome },
        Detalhe { rotulo: "Última troca".into(), valor: format!("{} (há {tempo})", c.ultima) },
        Detalhe {
            rotulo: "Visto na rede".into(),
            valor: if c.visto.is_empty() { "não".into() } else { c.visto.clone() },
        },
    ];
    if !c.erro.trim().is_empty() {
        a.detalhes.push(Detalhe { rotulo: "Último aviso da troca".into(), valor: c.erro.chars().take(120).collect() });
    }
    a.acao = "Ver caixas".into();
    a
}

/// Motivo curto para a faixa, como está. RC3-3: não escapa: a faixa de
/// atenção e a crítica mostram a descrição em texto puro, a informativa
/// escapa no QML (`Tema.escapar`, H10) e o leitor de tela lê o resumo; com o
/// escape aqui, um "&" aparecia como "&amp;".
fn curto(t: &str) -> String {
    t.chars().take(90).collect()
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

// ------------------------------------------------------------- comandos das telas

pub use comandos::COMANDOS;

/// Os comandos das notificações do Início (etapa 3 da modularização: vieram do `match` do
/// `api.rs` sem mudar nome, entrada, resposta nem permissão).
mod comandos {
    use serde_json::{json, Value};

    use crate::despacho::{ler, Classe, Comando, Pedido};
    use crate::erro::Resultado;

    pub const COMANDOS: &[Comando] = &[
        Comando { nome: "avisos_do_dia", classe: Classe::Leitura, funcao: avisos_do_dia },
        Comando { nome: "aviso_minimizar", classe: Classe::Caixa, funcao: aviso_minimizar },
        Comando { nome: "aviso_esconder", classe: Classe::Caixa, funcao: aviso_esconder },
        // QT-1b: a 2ª ação de um aviso ("Conferi"), que o resolve para quem conferiu.
        Comando { nome: "aviso_resolver", classe: Classe::Caixa, funcao: aviso_resolver },
    ];

    fn avisos_do_dia(p: &mut Pedido, _dados: Value) -> Resultado<Value> {
        Ok(json!(avisos_de(p)?))
    }

    fn aviso_minimizar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: Value = ler(dados)?;
        let atuais = avisos_de(p)?;
        let tx = p.conn.transaction()?;
        crate::avisos::minimizar_por(
            &tx,
            &atuais,
            d["id"].as_str().unwrap_or_default(),
            d["minimizado"].as_bool().unwrap_or(true),
            Some(p.usuario_id),
        )?;
        tx.commit()?;
        Ok(json!(true))
    }

    fn aviso_resolver(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: Value = ler(dados)?;
        match d["id"].as_str().unwrap_or_default() {
            id if id.starts_with(crate::avisos::FECHAMENTO_INESPERADO)
                || id.starts_with(crate::avisos::QUEDA_SEM_PENDENCIA) =>
            {
                let tx = p.conn.transaction()?;
                let atual = crate::operacoes::abertura_atual(&tx)?;
                crate::operacoes::fechamento_visto(&tx, p.usuario_id, atual)?;
                tx.commit()?;
                Ok(json!(true))
            }
            "troca_parada" | "troca_tentando" => {
                if let Ok(banco) =
                    p.conn.query_row("SELECT file FROM pragma_database_list WHERE name = 'main'", [], |l| {
                        l.get::<_, String>(0)
                    })
                {
                    if !banco.is_empty() {
                        crate::supervisao::reiniciar(std::path::Path::new(&banco));
                    }
                }
                Ok(json!(true))
            }
            _ => Err(crate::erro::Erro::validacao("Esta notificação não tem o que conferir.")),
        }
    }

    fn aviso_esconder(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: Value = ler(dados)?;
        let id = d["id"].as_str().unwrap_or_default();
        // A categoria da notificação que esta pessoa está vendo decide.
        let atuais = avisos_de(p)?;
        // T-04 da 2ª re-revisão do QT-1: a linha do S2 arrastada para o lado vale como o "Entendi" desta pessoa
        // (a marca de "escondida" do computador não era lida por quem monta a linha, e ela voltava na entrada
        // seguinte). Ela só aparece quando as quedas não deixaram nada a conferir, então nada que peça atenção some.
        if id.starts_with(crate::avisos::QUEDA_SEM_PENDENCIA) {
            if !atuais.iter().any(|a| a.id == id) {
                return Err(crate::erro::Erro::validacao("Esta notificação já não está aparecendo."));
            }
            let tx = p.conn.transaction()?;
            let atual = crate::operacoes::abertura_atual(&tx)?;
            crate::operacoes::fechamento_visto(&tx, p.usuario_id, atual)?;
            tx.commit()?;
            return Ok(json!(true));
        }
        crate::avisos::esconder(p.conn, &atuais, id)?;
        Ok(json!(true))
    }

    /// As notificações do Início para quem está conectado (as mesmas que a
    /// tela mostra e que `aviso_esconder` confere).
    fn avisos_de(p: &mut Pedido) -> Resultado<Vec<crate::avisos::Aviso>> {
        let mut avisos = match crate::avisos::do_dia(p.conn) {
            Ok(a) => a,
            // RVI-03: banco antigo em modo seguro (estrutura sem atualizar):
            // pelo menos o aviso do modo seguro.
            Err(e) if p.modo_seguro => {
                crate::registro::anotar(&format!("avisos no modo seguro: {e}"));
                crate::avisos::so_do_modo_seguro(p.conn, p.defeito.unwrap_or_default())
            }
            Err(e) => return Err(e),
        };
        // D-03: os avisos de contas a pagar e cheques têm valores do
        // financeiro: só para quem pode ver o financeiro.
        if !p.pode_sem_senha("financeiro")? {
            avisos.retain(|a| a.funcao != "financeiro");
        }
        if let Some(a) = crate::avisos::da_senha(p.conn, p.usuario_id)? {
            avisos.push(a);
        }
        // QT-1b: o fechamento inesperado vem primeiro (é o que conferir antes de refazer qualquer coisa).
        if !p.modo_seguro {
            if let Some(a) = crate::avisos::do_fechamento(p.conn, p.usuario_id, p.gerente)? {
                avisos.insert(0, a);
            } else if let Some(a) = crate::avisos::da_queda_sem_pendencia(p.conn, p.usuario_id)? {
                // S2: só informa; fica depois das outras (não toma o lugar de uma de atenção).
                avisos.push(a);
            }
        }
        crate::avisos::uma_aberta(&mut avisos);
        Ok(avisos)
    }
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
        // RC3-3: o motivo vem como está (a faixa é texto puro).
        assert!(a.iter().any(|a| a.id == "backup" && a.atraso.contains("<x>")));
        config::gravar(&b.conn, "backup_nuvem", "1").unwrap();
        config::gravar(&b.conn, "backup_nuvem_desde", &crate::tempo::dias_atras(2)).unwrap();
        assert!(do_dia(&b.conn).unwrap().iter().any(|a| a.id == "backup_nuvem"));
    }

    /// RC3-3 (revisão cruzada 3; H10, extensão): o motivo do erro do backup
    /// e o da nuvem chegam à faixa sem entidades HTML. A faixa de atenção (e a
    /// crítica) mostra a `descricao` em texto puro (`Notificacao.qml`, desde a
    /// 0.9.17), e a informativa escapa no QML (`Tema.escapar`); o leitor de
    /// tela lê o `resumo`. Um caminho com "&" ou "<" aparecia como "&amp;" e
    /// "&lt;". Era a sonda `sonda_rev3_erro_do_backup_…` da revisão.
    #[test]
    fn rc3_3_erro_do_backup_chega_sem_entidades() {
        let b = Banco::em_memoria().unwrap();
        let erro = "Não deu para gravar na pasta /home/ze/Peças & Cia <loja>";
        config::gravar(&b.conn, "backup_erro", erro).unwrap();
        config::gravar(&b.conn, "backup_nuvem", "1").unwrap();
        config::gravar(&b.conn, "backup_nuvem_desde", &crate::tempo::dias_atras(9)).unwrap();
        config::gravar(&b.conn, "backup_nuvem_erro", "rclone: pasta \"Loja & Cia\" <sem acesso>").unwrap();
        let lista = do_dia(&b.conn).unwrap();
        let a = lista.iter().find(|a| a.id == "backup").expect("o aviso do backup");
        assert_eq!(a.categoria, ATENCAO);
        assert_eq!(a.atraso, erro);
        assert_eq!(a.descricao, format!("O último backup não deu certo · {erro}"));
        assert!(a.resumo.ends_with(erro), "o resumo (o leitor de tela) com entidades: {:?}", a.resumo);
        let n = lista.iter().find(|a| a.id == "backup_nuvem").expect("o aviso da nuvem");
        assert_eq!(n.atraso, "rclone: pasta \"Loja & Cia\" <sem acesso>");
        for x in [a, n] {
            for t in [&x.atraso, &x.descricao, &x.resumo] {
                assert!(!t.contains("&amp;") && !t.contains("&lt;") && !t.contains("&gt;"), "com entidades: {t:?}");
            }
        }
    }

    /// NOTIF-01 (0.9.18): o texto do botão diz o que ele faz. Os avisos de
    /// diferenças entre caixas e de códigos abrem Simultaneidade ›
    /// Duplicatas: o botão é "Ver duplicatas" (antes: "Resolver no Gerente").
    /// Nenhum botão fala em "Gerente" sem levar à parte do gerente.
    #[test]
    fn notif01_botao_diz_para_onde_leva() {
        let b = Banco::em_memoria().unwrap();
        for id in ["conflitos", "codigos", "codigos_bloqueio"] {
            let a = Aviso::simples(id, "2026-11-05", "Teste".into(), "texto".into(), String::new(), "configuracoes")
                .secao("duplicatas");
            let a = completar(&b.conn, a).unwrap();
            assert_eq!((a.secao.as_str(), a.acao.as_str()), ("duplicatas", "Ver duplicatas"), "{id}");
        }
        for id in [
            "receber",
            "pagar",
            "cheques",
            "encomendas",
            "oficina",
            "backup",
            "backup_nuvem",
            "referencias",
            "outra_loja",
            "senha.ana",
            "qualquer",
        ] {
            let a = Aviso::simples(id, "2026-11-05", "Teste".into(), "texto".into(), String::new(), "configuracoes");
            let a = completar(&b.conn, a).unwrap();
            assert!(!a.acao.to_lowercase().contains("gerente") || a.secao == "gerente", "{id}: {}", a.acao);
        }
    }

    /// F02 (passo 6.7, DP-32): com backup feito e sem a chave da loja (nenhum
    /// gerente entrou), o aviso do dia diz que os backups saem sem tranca e leva
    /// ao Backup; sem backup ainda, ou com a chave, não avisa.
    #[test]
    fn f02_backup_sem_tranca_avisa_ate_o_gerente_entrar() {
        let b = Banco::em_memoria().unwrap();
        let c = &b.conn;
        let aviso = |c: &Connection| do_dia(c).unwrap().into_iter().find(|a| a.id == "backup_sem_tranca");
        assert!(aviso(c).is_none(), "sem backup ainda: nada");
        config::gravar(c, "backup_ultimo", "2026-11-12T10:00:00Z").unwrap();
        let a = aviso(c).expect("backup feito sem a chave: avisa");
        assert_eq!(a.categoria, ATENCAO);
        assert_eq!((a.funcao.as_str(), a.secao.as_str(), a.acao.as_str()), ("configuracoes", "backup", "Ver backup"));
        assert!(a.descricao.contains("Entre uma vez como gerente"), "{}", a.descricao);
        config::gravar(c, "backup_publica", &"ab".repeat(32)).unwrap();
        assert!(aviso(c).is_none(), "com a chave da loja: some");
    }

    /// T-07 (C8): caixa da loja sem trocar dados há um dia ou mais vira
    /// atenção, com o nome e a última troca. Some com uma troca que dê certo,
    /// com a retirada do caixa e com a troca de dados desligada. Menos de um
    /// dia, ou nunca ter trocado, não avisa.
    #[test]
    fn t07_caixa_sem_contato_ha_um_dia() {
        let b = Banco::em_memoria().unwrap();
        let c = &b.conn;
        let sem_contato = |c: &Connection| -> Vec<String> {
            do_dia(c).unwrap().into_iter().map(|a| a.id).filter(|i| i.starts_with("sem_contato.")).collect()
        };
        c.execute(
            "INSERT INTO sync_par (dispositivo, nome, sincronizado_em, visto_em) VALUES
               ('caixa-2', 'Caixa 2', strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-30 hours'),
                                      strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-29 hours')),
               ('caixa-3', 'Caixa 3', strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-23 hours'), NULL),
               ('caixa-4', 'Caixa 4', NULL, NULL),
               ('caixa-5', 'Caixa 5', strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-5 days', '-1 hours'), NULL)",
            [],
        )
        .unwrap();
        assert!(sem_contato(c).is_empty(), "troca de dados desligada: nada");
        config::gravar(c, "sync_ligado", "1").unwrap();
        assert_eq!(sem_contato(c), ["sem_contato.caixa-2", "sem_contato.caixa-5"], "23 h e nunca: sem aviso");

        let v = do_dia(c).unwrap();
        let a = v.iter().find(|a| a.id == "sem_contato.caixa-2").unwrap();
        assert_eq!(a.categoria, ATENCAO);
        assert_eq!(a.titulo_longo, "Atenção: Caixa 2 sem trocar dados há 30 horas");
        assert_eq!((a.funcao.as_str(), a.secao.as_str(), a.acao.as_str()), ("configuracoes", "caixas", "Ver caixas"));
        assert!(a.descricao.contains("retire-o da loja"), "{}", a.descricao);
        assert!(a.detalhes.iter().any(|d| d.rotulo == "Última troca" && d.valor.ends_with("(há 30 horas)")));
        assert!(a.detalhes.iter().any(|d| d.rotulo == "Visto na rede" && d.valor != "não"));
        let a = v.iter().find(|a| a.id == "sem_contato.caixa-5").unwrap();
        assert_eq!(a.titulo_longo, "Atenção: Caixa 5 sem trocar dados há 5 dias");
        assert!(a.detalhes.iter().any(|d| d.rotulo == "Visto na rede" && d.valor == "não"));
        // Atenção: não se esconde (some quando o problema acaba).
        assert!(esconder(c, &v, "sem_contato.caixa-2").is_err());

        // Trocou dados agora: some sozinha.
        c.execute(
            "UPDATE sync_par SET sincronizado_em = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE dispositivo = 'caixa-2'",
            [],
        )
        .unwrap();
        assert_eq!(sem_contato(c), ["sem_contato.caixa-5"]);
        // Retirado da loja: não avisa mais.
        config::gravar(c, &format!("{}caixa-5", crate::sync::PREFIXO_RETIRADO), "2026-11-04T10:00:00Z").unwrap();
        assert!(sem_contato(c).is_empty());
    }
}
