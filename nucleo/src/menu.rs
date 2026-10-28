//! Menu principal: as funções do Rustinho na tela inicial (0.9.15).
//!
//! Uma grade só, na ordem fixa de `FUNCOES` (sem "Mais usados" nem "Todas as
//! funções"). Fixar muda a ordem: a 1ª fixada fica na 1ª posição, a 2ª na
//! 2ª, até 4; depois de 4 fixadas, as outras não mostram mais o alfinete e
//! só as fixadas têm o de soltar. A ordem não depende de quantas vezes cada
//! função é aberta.
//!
//! "Vender / Receber" e "Estoque / Fornecedores" são uma pílula cada (com
//! as subpílulas dentro). Os ids antigos ("receber", "fornecedores") ainda
//! valem: viram a pílula unida. A conferência fiscal saiu da tela inicial e
//! está em Configurações › Fiscal.

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::erro::{Erro, Resultado};
use crate::tempo;

/// Quantas funções podem ser fixadas.
pub const FIXADAS_MAXIMAS: usize = 4;

/// Uma função do sistema.
pub struct Funcao {
    pub id: &'static str,
    pub titulo: &'static str,
    pub descricao: &'static str,
    /// Ícone da pílula (veja `Icone.qml`).
    pub icone: &'static str,
    /// Etapa do projeto em que a função fica pronta.
    pub etapa: u8,
}

/// Etapa atual: funções de etapas até esta já funcionam.
pub const ETAPA_ATUAL: u8 = 4;

/// Funções da etapa atual que ainda não ficaram prontas.
const AINDA_NAO: &[&str] = &[];

/// As 14 funções da tela inicial, na ordem fixa.
pub const FUNCOES: &[Funcao] = &[
    Funcao {
        id: "vender",
        titulo: "Vender / Receber",
        descricao: "Balcão, crediário e a receber",
        icone: "carrinho",
        etapa: 3,
    },
    Funcao {
        id: "pecas",
        titulo: "Peças",
        descricao: "Buscar, ver estoque e cadastrar",
        icone: "engrenagem",
        etapa: 1,
    },
    Funcao {
        id: "orcamentos",
        titulo: "Orçamentos",
        descricao: "Montar, imprimir e vender",
        icone: "documento",
        etapa: 3,
    },
    Funcao {
        id: "caixa",
        titulo: "Caixa",
        descricao: "Abertura, fechamento e sangria",
        icone: "registradora",
        etapa: 3,
    },
    Funcao {
        id: "trocas",
        titulo: "Trocas e devoluções",
        descricao: "Peça devolvida e vale-troca",
        icone: "trocas",
        etapa: 4,
    },
    Funcao { id: "notas", titulo: "Notas fiscais", descricao: "NF-e, NFC-e e NFS-e", icone: "nota", etapa: 5 },
    Funcao {
        id: "clientes",
        titulo: "Clientes",
        descricao: "Cadastro e histórico de compras",
        icone: "clientes",
        etapa: 2,
    },
    Funcao { id: "compras", titulo: "Compras", descricao: "Notas de entrada pelo XML", icone: "cesta", etapa: 2 },
    Funcao {
        id: "estoque",
        titulo: "Estoque / Fornecedores",
        descricao: "O que comprar, encomendas e fornecedores",
        icone: "caixa",
        etapa: 4,
    },
    Funcao {
        id: "financeiro",
        titulo: "Financeiro",
        descricao: "A pagar, cheques e fluxo de caixa",
        icone: "barras",
        etapa: 4,
    },
    Funcao {
        id: "oficina",
        titulo: "Ordens de serviço",
        descricao: "Oficina: carro, peças e mão de obra",
        icone: "chave",
        etapa: 4,
    },
    Funcao { id: "painel", titulo: "Painel", descricao: "Vendas do dia, do mês e do ano", icone: "pizza", etapa: 4 },
    Funcao { id: "usuarios", titulo: "Usuários", descricao: "Quem usa o Rustinho", icone: "pessoa", etapa: 1 },
    Funcao {
        id: "configuracoes",
        titulo: "Configurações",
        descricao: "Loja, fiscal, backup e caixas",
        icone: "engrenagem",
        etapa: 1,
    },
];

/// Ids de antes da 0.9.15 que viraram parte de outra pílula.
fn id_atual(item: &str) -> &str {
    match item {
        "receber" => "vender",
        "fornecedores" => "estoque",
        outro => outro,
    }
}

/// Uma função do jeito que a tela precisa.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemMenu {
    pub id: String,
    pub titulo: String,
    pub descricao: String,
    pub icone: String,
    /// Letra do cartão (como a capitular do livro).
    pub letra: String,
    pub disponivel: bool,
    pub etapa: u8,
    pub fixo: bool,
    /// Pode mostrar o alfinete de fixar (menos de 4 fixadas).
    pub pode_fixar: bool,
}

/// O menu de um usuário: a grade toda, já na ordem.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Menu {
    pub todos: Vec<ItemMenu>,
    pub fixadas: usize,
    pub fixadas_maximas: usize,
}

/// As fixadas do usuário, na ordem em que foram fixadas (ids atuais, sem
/// repetir; as de antes da 0.9.15 sem data vêm primeiro, na ordem do menu).
fn fixadas(conn: &Connection, usuario_id: &str) -> Resultado<Vec<String>> {
    let mut c = conn.prepare_cached("SELECT item, fixado_em FROM menu_uso WHERE usuario_id = ?1 AND fixo = 1")?;
    let mut lista: Vec<(String, Option<String>)> = c
        .query_map([usuario_id], |l| Ok((l.get::<_, String>(0)?, l.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|(i, q)| (id_atual(&i).to_string(), q))
        .filter(|(i, _)| FUNCOES.iter().any(|f| f.id == i))
        .collect();
    let posicao = |id: &str| FUNCOES.iter().position(|f| f.id == id).unwrap_or(usize::MAX);
    lista.sort_by(|a, b| match (&a.1, &b.1) {
        (None, None) => posicao(&a.0).cmp(&posicao(&b.0)),
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (Some(x), Some(y)) => x.cmp(y).then_with(|| posicao(&a.0).cmp(&posicao(&b.0))),
    });
    let mut ids: Vec<String> = Vec::new();
    for (i, _) in lista {
        if !ids.contains(&i) {
            ids.push(i);
        }
    }
    ids.truncate(FIXADAS_MAXIMAS);
    Ok(ids)
}

/// Monta o menu do usuário: as fixadas primeiro (na ordem em que foram
/// fixadas) e depois as outras, na ordem fixa.
pub fn menu(conn: &Connection, usuario_id: &str) -> Resultado<Menu> {
    Ok(montar(fixadas(conn, usuario_id)?))
}

/// RVI-03: o menu sem as fixadas (no modo seguro, um banco de versão anterior
/// pode não ter onde elas ficam).
pub fn sem_fixadas() -> Menu {
    montar(Vec::new())
}

fn montar(fixas: Vec<String>) -> Menu {
    let pode_fixar = fixas.len() < FIXADAS_MAXIMAS;
    let item = |f: &Funcao| ItemMenu {
        id: f.id.to_string(),
        titulo: f.titulo.to_string(),
        descricao: f.descricao.to_string(),
        icone: f.icone.to_string(),
        letra: f.titulo.chars().next().map(|c| c.to_string()).unwrap_or_default(),
        disponivel: f.etapa <= ETAPA_ATUAL && !AINDA_NAO.contains(&f.id),
        etapa: f.etapa,
        fixo: fixas.iter().any(|x| x == f.id),
        pode_fixar,
    };
    let mut todos: Vec<ItemMenu> =
        fixas.iter().filter_map(|id| FUNCOES.iter().find(|f| f.id == id)).map(item).collect();
    todos.extend(FUNCOES.iter().filter(|f| !fixas.iter().any(|x| x == f.id)).map(item));
    Menu { todos, fixadas: fixas.len(), fixadas_maximas: FIXADAS_MAXIMAS }
}

/// Conta mais um uso da função (só um registro; não muda a ordem).
pub fn registrar_uso(conn: &Connection, usuario_id: &str, item: &str) -> Resultado<()> {
    let item = garantir_funcao(item)?;
    conn.execute(
        "INSERT INTO menu_uso (usuario_id, item, usos, ultimo_uso) VALUES (?1, ?2, 1, ?3)
         ON CONFLICT (usuario_id, item) DO UPDATE SET usos = usos + 1, ultimo_uso = excluded.ultimo_uso",
        params![usuario_id, item, tempo::agora()],
    )?;
    Ok(())
}

/// Fixa (ou solta) uma função: fixar a põe depois das já fixadas.
pub fn fixar(conn: &Connection, usuario_id: &str, item: &str, fixo: bool) -> Resultado<()> {
    let item = garantir_funcao(item)?;
    let atuais = fixadas(conn, usuario_id)?;
    if fixo {
        if atuais.iter().any(|x| x == item) {
            return Ok(());
        }
        if atuais.len() >= FIXADAS_MAXIMAS {
            return Err(Erro::validacao(format!(
                "Já há {FIXADAS_MAXIMAS} funções fixadas. Solte uma antes de fixar outra."
            )));
        }
    }
    // A pílula unida: o id antigo fixado também sai (ou não volta).
    for antigo in ["receber", "fornecedores"] {
        if id_atual(antigo) == item {
            conn.execute(
                "UPDATE menu_uso SET fixo = 0 WHERE usuario_id = ?1 AND item = ?2",
                params![usuario_id, antigo],
            )?;
        }
    }
    conn.execute(
        "INSERT INTO menu_uso (usuario_id, item, usos, fixo, fixado_em) VALUES (?1, ?2, 0, ?3, ?4)
         ON CONFLICT (usuario_id, item) DO UPDATE SET fixo = excluded.fixo, fixado_em = excluded.fixado_em",
        // UUID v7: cresce com o tempo, mesmo duas fixadas no mesmo segundo.
        params![usuario_id, item, fixo, if fixo { Some(tempo::novo_id()) } else { None }],
    )?;
    Ok(())
}

fn garantir_funcao(item: &str) -> Resultado<&'static str> {
    let atual = id_atual(item);
    FUNCOES
        .iter()
        .find(|f| f.id == atual)
        .map(|f| f.id)
        .ok_or_else(|| Erro::Interno(format!("função desconhecida: {item}")))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::usuarios;

    fn ids(itens: &[ItemMenu]) -> Vec<&str> {
        itens.iter().map(|i| i.id.as_str()).collect()
    }

    /// A ordem padrão aprovada para a 0.9.15 (14 pílulas).
    const ORDEM: [&str; 14] = [
        "vender",
        "pecas",
        "orcamentos",
        "caixa",
        "trocas",
        "notas",
        "clientes",
        "compras",
        "estoque",
        "financeiro",
        "oficina",
        "painel",
        "usuarios",
        "configuracoes",
    ];

    #[test]
    fn ordem_fixa_sem_mais_usados_e_uso_nao_muda_a_ordem() {
        let b = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        let m = menu(&b.conn, &u.id).unwrap();
        assert_eq!(ids(&m.todos), ORDEM);
        assert_eq!(m.fixadas, 0);
        assert!(m.todos.iter().all(|i| !i.fixo && i.pode_fixar), "nenhuma fixada de fábrica");
        let titulos: Vec<&str> = m.todos.iter().map(|i| i.titulo.as_str()).collect();
        assert_eq!(titulos[0], "Vender / Receber");
        assert_eq!(titulos[8], "Estoque / Fornecedores");
        assert_eq!(titulos[10], "Ordens de serviço");
        assert!(!titulos.contains(&"Fiscal"), "a conferência fiscal foi para Configurações");
        for _ in 0..5 {
            registrar_uso(&b.conn, &u.id, "usuarios").unwrap();
        }
        registrar_uso(&b.conn, &u.id, "receber").unwrap();
        assert_eq!(ids(&menu(&b.conn, &u.id).unwrap().todos), ORDEM, "usar muito não muda a ordem");
    }

    #[test]
    fn fixar_poe_na_frente_na_ordem_em_que_foi_fixada() {
        let b = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        fixar(&b.conn, &u.id, "configuracoes", true).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
        fixar(&b.conn, &u.id, "painel", true).unwrap();
        let m = menu(&b.conn, &u.id).unwrap();
        assert_eq!(&ids(&m.todos)[..3], ["configuracoes", "painel", "vender"]);
        assert_eq!(m.todos.len(), 14, "fixar não repete a pílula");
        std::thread::sleep(std::time::Duration::from_millis(3));
        fixar(&b.conn, &u.id, "fornecedores", true).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
        fixar(&b.conn, &u.id, "pecas", true).unwrap();
        let m = menu(&b.conn, &u.id).unwrap();
        assert_eq!(&ids(&m.todos)[..4], ["configuracoes", "painel", "estoque", "pecas"]);
        assert!(m.todos.iter().all(|i| !i.pode_fixar), "com 4 fixadas, o alfinete de fixar some");
        assert!(fixar(&b.conn, &u.id, "caixa", true).is_err(), "a 5ª é recusada");
        fixar(&b.conn, &u.id, "painel", false).unwrap();
        let m = menu(&b.conn, &u.id).unwrap();
        assert_eq!(&ids(&m.todos)[..3], ["configuracoes", "estoque", "pecas"]);
        assert_eq!(ids(&m.todos)[3], "vender", "a solta volta para a ordem fixa");
        assert!(m.todos.iter().all(|i| i.pode_fixar));
    }

    #[test]
    fn fixadas_de_antes_viram_a_pilula_unida_e_fiscal_sai() {
        let b = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        for item in ["receber", "fiscal", "fornecedores", "vender"] {
            b.conn
                .execute(
                    "INSERT INTO menu_uso (usuario_id, item, usos, fixo) VALUES (?1, ?2, 3, 1)",
                    params![u.id, item],
                )
                .unwrap();
        }
        let m = menu(&b.conn, &u.id).unwrap();
        assert_eq!(&ids(&m.todos)[..2], ["vender", "estoque"], "receber+vender viram uma; fiscal sai");
        assert_eq!(m.fixadas, 2);
        assert_eq!(m.todos.len(), 14);
    }

    #[test]
    fn letra_e_disponibilidade() {
        let b = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        let m = menu(&b.conn, &u.id).unwrap();
        let orc = m.todos.iter().find(|i| i.id == "orcamentos").unwrap();
        assert_eq!(orc.letra, "O");
        assert!(orc.disponivel);
        let notas = m.todos.iter().find(|i| i.id == "notas").unwrap();
        assert!(!notas.disponivel, "notas fiscais são da etapa 5 (em breve)");
        assert!(m.todos.iter().find(|i| i.id == "oficina").unwrap().disponivel);
        assert!(m.todos.iter().find(|i| i.id == "financeiro").unwrap().disponivel);
        assert!(m.todos.iter().find(|i| i.id == "pecas").unwrap().disponivel);
        assert!(registrar_uso(&b.conn, &u.id, "inexistente").is_err());
        assert!(registrar_uso(&b.conn, &u.id, "fornecedores").is_ok(), "id antigo continua valendo");
    }
}
