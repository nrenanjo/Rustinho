//! Menu principal: as funções do Rustinho, com as mais usadas primeiro.
//!
//! A lista "Todas as funções" fica sempre na mesma ordem (para ninguém se
//! perder). Em cima dela, "Mais usados" mostra as fixadas e as que cada
//! usuário mais abre.

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::erro::{Erro, Resultado};
use crate::tempo;

/// Quantos itens aparecem em "Mais usados".
const MAIS_USADOS: usize = 4;

/// Uma função do sistema.
pub struct Funcao {
    pub id: &'static str,
    pub titulo: &'static str,
    pub descricao: &'static str,
    /// Etapa do projeto em que a função fica pronta.
    pub etapa: u8,
}

/// Etapa atual: funções de etapas até esta já funcionam.
pub const ETAPA_ATUAL: u8 = 4;

/// Funções da etapa atual que ainda não ficaram prontas.
const AINDA_NAO: &[&str] = &[];

/// Todas as funções, na ordem fixa do menu.
pub const FUNCOES: &[Funcao] = &[
    Funcao { id: "vender", titulo: "Vender", descricao: "Balcão com leitor de barras", etapa: 3 },
    Funcao { id: "pecas", titulo: "Peças", descricao: "Buscar, ver estoque e cadastrar", etapa: 1 },
    Funcao { id: "orcamentos", titulo: "Orçamentos", descricao: "Montar, imprimir e vender", etapa: 3 },
    Funcao { id: "caixa", titulo: "Caixa", descricao: "Abertura, fechamento e sangria", etapa: 3 },
    Funcao { id: "trocas", titulo: "Trocas e devoluções", descricao: "Peça devolvida e vale-troca", etapa: 4 },
    Funcao { id: "receber", titulo: "Receber", descricao: "Crediário, convênio e a receber", etapa: 4 },
    Funcao { id: "notas", titulo: "Notas fiscais", descricao: "NF-e, NFC-e e NFS-e", etapa: 5 },
    Funcao { id: "clientes", titulo: "Clientes", descricao: "Cadastro e histórico de compras", etapa: 2 },
    Funcao { id: "compras", titulo: "Compras", descricao: "Notas de entrada pelo XML", etapa: 2 },
    Funcao { id: "estoque", titulo: "Estoque", descricao: "O que comprar, encomendas e parados", etapa: 4 },
    Funcao { id: "fornecedores", titulo: "Fornecedores", descricao: "Distribuidoras e fábricas", etapa: 2 },
    Funcao { id: "fiscal", titulo: "Fiscal", descricao: "Notas fiscais, CSOSN, ST e NCM", etapa: 2 },
    Funcao { id: "financeiro", titulo: "Financeiro", descricao: "A pagar, cheques e fluxo de caixa", etapa: 4 },
    Funcao { id: "oficina", titulo: "Ordem de serviço", descricao: "Oficina: carro, peças e mão de obra", etapa: 4 },
    Funcao { id: "painel", titulo: "Painel", descricao: "Vendas do dia, do mês e do ano", etapa: 4 },
    Funcao { id: "usuarios", titulo: "Usuários", descricao: "Quem usa o Rustinho", etapa: 1 },
    Funcao { id: "configuracoes", titulo: "Configurações", descricao: "Loja, impressão, backup e caixas", etapa: 1 },
];

/// Uma função do jeito que a tela precisa.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemMenu {
    pub id: String,
    pub titulo: String,
    pub descricao: String,
    /// Letra grande do cartão (como a capitular do livro).
    pub letra: String,
    pub disponivel: bool,
    pub etapa: u8,
    pub fixo: bool,
    pub usos: i64,
}

/// O menu de um usuário.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Menu {
    pub mais_usados: Vec<ItemMenu>,
    pub todos: Vec<ItemMenu>,
}

/// Monta o menu do usuário, com as mais usadas primeiro.
pub fn menu(conn: &Connection, usuario_id: &str) -> Resultado<Menu> {
    let mut consulta = conn.prepare_cached("SELECT item, usos, fixo, ultimo_uso FROM menu_uso WHERE usuario_id = ?1")?;
    let uso: Vec<(String, i64, bool, Option<String>)> = consulta
        .query_map([usuario_id], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)))?
        .collect::<Result<_, _>>()?;

    let todos: Vec<ItemMenu> = FUNCOES
        .iter()
        .map(|f| {
            let u = uso.iter().find(|(item, ..)| item == f.id);
            ItemMenu {
                id: f.id.to_string(),
                titulo: f.titulo.to_string(),
                descricao: f.descricao.to_string(),
                letra: f.titulo.chars().next().map(|c| c.to_string()).unwrap_or_default(),
                disponivel: f.etapa <= ETAPA_ATUAL && !AINDA_NAO.contains(&f.id),
                etapa: f.etapa,
                fixo: u.map(|x| x.2).unwrap_or(false),
                usos: u.map(|x| x.1).unwrap_or(0),
            }
        })
        .collect();

    let ultimo_uso = |id: &str| -> String {
        uso.iter()
            .find(|(item, ..)| item == id)
            .and_then(|x| x.3.clone())
            .unwrap_or_default()
    };

    // 1º as fixadas (na ordem do menu); 2º as mais abertas; 3º as primeiras do menu.
    let mut mais_usados: Vec<ItemMenu> = todos.iter().filter(|i| i.fixo).cloned().collect();
    let mut usadas: Vec<&ItemMenu> = todos.iter().filter(|i| !i.fixo && i.usos > 0).collect();
    usadas.sort_by(|a, b| b.usos.cmp(&a.usos).then_with(|| ultimo_uso(&b.id).cmp(&ultimo_uso(&a.id))));
    for item in usadas.into_iter().chain(todos.iter()) {
        if mais_usados.len() >= MAIS_USADOS {
            break;
        }
        if !mais_usados.iter().any(|m| m.id == item.id) {
            mais_usados.push(item.clone());
        }
    }

    Ok(Menu { mais_usados, todos })
}

/// Conta mais um uso da função.
pub fn registrar_uso(conn: &Connection, usuario_id: &str, item: &str) -> Resultado<()> {
    garantir_funcao(item)?;
    conn.execute(
        "INSERT INTO menu_uso (usuario_id, item, usos, ultimo_uso) VALUES (?1, ?2, 1, ?3)
         ON CONFLICT (usuario_id, item) DO UPDATE SET usos = usos + 1, ultimo_uso = excluded.ultimo_uso",
        params![usuario_id, item, tempo::agora()],
    )?;
    Ok(())
}

/// Fixa (ou solta) uma função em "Mais usados".
pub fn fixar(conn: &Connection, usuario_id: &str, item: &str, fixo: bool) -> Resultado<()> {
    garantir_funcao(item)?;
    if fixo {
        let fixadas: i64 = conn.query_row(
            "SELECT COUNT(*) FROM menu_uso WHERE usuario_id = ?1 AND fixo = 1 AND item <> ?2",
            params![usuario_id, item],
            |l| l.get(0),
        )?;
        if fixadas as usize >= MAIS_USADOS {
            return Err(Erro::validacao(format!(
                "Já há {MAIS_USADOS} funções fixadas. Solte uma antes de fixar outra."
            )));
        }
    }
    conn.execute(
        "INSERT INTO menu_uso (usuario_id, item, usos, fixo) VALUES (?1, ?2, 0, ?3)
         ON CONFLICT (usuario_id, item) DO UPDATE SET fixo = excluded.fixo",
        params![usuario_id, item, fixo],
    )?;
    Ok(())
}

fn garantir_funcao(item: &str) -> Resultado<()> {
    if FUNCOES.iter().any(|f| f.id == item) {
        Ok(())
    } else {
        Err(Erro::Interno(format!("função desconhecida: {item}")))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::usuarios;

    fn ids(itens: &[ItemMenu]) -> Vec<&str> {
        itens.iter().map(|i| i.id.as_str()).collect()
    }

    #[test]
    fn mais_usados_sobem_e_lista_completa_fica_fixa() {
        let b = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();

        let m = menu(&b.conn, &u.id).unwrap();
        assert_eq!(ids(&m.mais_usados), ["vender", "pecas", "orcamentos", "caixa"]);
        assert_eq!(m.todos.len(), FUNCOES.len());

        for _ in 0..3 {
            registrar_uso(&b.conn, &u.id, "usuarios").unwrap();
        }
        registrar_uso(&b.conn, &u.id, "pecas").unwrap();
        let m = menu(&b.conn, &u.id).unwrap();
        assert_eq!(ids(&m.mais_usados), ["usuarios", "pecas", "vender", "orcamentos"]);
        assert_eq!(ids(&m.todos)[0], "vender");

        fixar(&b.conn, &u.id, "configuracoes", true).unwrap();
        let m = menu(&b.conn, &u.id).unwrap();
        assert_eq!(ids(&m.mais_usados), ["configuracoes", "usuarios", "pecas", "vender"]);
        assert!(m.todos.iter().find(|i| i.id == "configuracoes").unwrap().fixo);
    }

    #[test]
    fn letra_e_disponibilidade() {
        let b = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        let m = menu(&b.conn, &u.id).unwrap();
        let orc = m.todos.iter().find(|i| i.id == "orcamentos").unwrap();
        assert_eq!(orc.letra, "O");
        assert!(orc.disponivel);
        let receber = m.todos.iter().find(|i| i.id == "receber").unwrap();
        assert!(receber.disponivel, "crediário é da etapa 4");
        let notas = m.todos.iter().find(|i| i.id == "notas").unwrap();
        assert!(!notas.disponivel, "notas fiscais são da etapa 5");
        assert!(m.todos.iter().find(|i| i.id == "oficina").unwrap().disponivel);
        assert!(m.todos.iter().find(|i| i.id == "financeiro").unwrap().disponivel);
        assert!(m.todos.iter().find(|i| i.id == "pecas").unwrap().disponivel);
        assert!(registrar_uso(&b.conn, &u.id, "inexistente").is_err());
    }
}
