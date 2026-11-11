//! O esquema do banco numa fonte só (dev.3.3, passo 3.2; ARQ-10, contrato C2
//! da `ARQUITETURA_ALVO_RUSTINHO.md`): cada tabela, a classe dela na troca de
//! dados entre os caixas e os módulos que gravam nela, **como está hoje**.
//!
//! Antes a lista de tabelas estava em três lugares: as migrações, a
//! `sync::TABELAS` (as que viajam) e a `ESCRITORES` do teste de dependências.
//! Agora as duas listas saem daqui, e um teste confere que toda tabela e
//! visão criada pelas migrações tem a sua linha (tabela sem classe falha).
//!
//! Nada muda no que viaja: as tabelas de classe `Viaja` são as mesmas e estão
//! na mesma ordem de antes (é a ordem em que a troca as percorre).

/// Como a tabela participa da troca de dados entre os caixas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classe {
    /// Vai e vem entre os caixas (`sync::TABELAS`).
    Viaja,
    /// Fica só neste caixa.
    Local,
    /// Montada a partir de outras tabelas (as visões): não viaja e ninguém grava nela.
    Derivada,
}

/// Uma tabela (ou visão) do banco.
#[derive(Debug)]
pub struct Tabela {
    pub nome: &'static str,
    pub classe: Classe,
    /// Os módulos que gravam nela hoje no código de produção (INSERT,
    /// REPLACE, UPDATE ou DELETE), em ordem alfabética. O teste de
    /// dependências confere esta lista com o código: módulo novo gravando
    /// falha, e quem deixou de gravar tem de sair daqui.
    pub escritores: &'static [&'static str],
}

impl Tabela {
    /// O dono: o único módulo que grava nela. `None` enquanto houver mais de
    /// um (ARQ-01: as de produto na 3.3, as da troca na 5.1, o resto na 8.2).
    pub fn dono(&self) -> Option<&'static str> {
        match self.escritores {
            [m] => Some(m),
            _ => None,
        }
    }
}

const fn t(nome: &'static str, classe: Classe, escritores: &'static [&'static str]) -> Tabela {
    Tabela { nome, classe, escritores }
}

use Classe::{Derivada, Local, Viaja};

/// Todas as tabelas e visões que as migrações criam. As que viajam vêm
/// primeiro, na ordem em que a troca as percorre.
#[rustfmt::skip]
pub const TABELAS: &[Tabela] = &[
    t("config", Viaja, &["backup", "banco", "config", "faixas", "loja", "sync"]),
    t("usuario", Viaja, &["backup", "sync", "usuarios"]),
    t("tabela_preco", Viaja, &["config"]),
    t("regra_st", Viaja, &["fiscal"]),
    t("unidade", Viaja, &["unidades"]),
    t("produto", Viaja, &["codigos", "compras", "faixas", "fiscal", "produtos"]),
    t("produto_preco", Viaja, &["compras", "produtos"]),
    t("produto_aplicacao", Viaja, &["produtos"]),
    t("produto_equivalente", Viaja, &["produtos"]),
    t("produto_fornecedor", Viaja, &["compras", "importar"]),
    // Códigos que uma peça já teve.
    t("produto_codigo_antigo", Viaja, &["importar", "produtos"]),
    // Faixas de códigos das peças de cada caixa (veja `faixas`).
    t("faixa_codigo", Viaja, &["faixas"]),
    t("estoque_movimento", Viaja, &["estoque"]),
    t("pessoa", Viaja, &["faixas", "pessoas"]),
    t("pessoa_veiculo", Viaja, &["oficina", "pessoas"]),
    t("compra", Viaja, &["compras"]),
    t("compra_item", Viaja, &["compras"]),
    t("caixa_sessao", Viaja, &["caixa"]),
    t("caixa_movimento", Viaja, &["caixa"]),
    t("venda", Viaja, &["vendas"]),
    t("venda_item", Viaja, &["vendas"]),
    t("venda_pagamento", Viaja, &["vendas"]),
    t("conta_receber", Viaja, &["receber"]),
    t("recebimento", Viaja, &["devolucoes", "receber"]),
    t("recebimento_parcela", Viaja, &["devolucoes", "receber"]),
    t("conta_loja", Viaja, &["financeiro"]),
    t("categoria", Viaja, &["dre", "financeiro"]),
    t("conta_pagar", Viaja, &["pagar"]),
    t("pagamento", Viaja, &["pagar"]),
    t("movimento_conta", Viaja, &["caixa", "cheques", "financeiro", "pagar"]),
    t("cheque", Viaja, &["cheques"]),
    t("comissao_fechamento", Viaja, &["comissoes"]),
    // Metas da DRE gerencial (veja `dre`).
    t("meta", Viaja, &["dre"]),
    t("encomenda", Viaja, &["encomendas"]),
    t("devolucao", Viaja, &["devolucoes"]),
    t("devolucao_item", Viaja, &["devolucoes"]),
    t("vale", Viaja, &["devolucoes"]),
    t("ordem_servico", Viaja, &["oficina"]),
    t("ordem_servico_item", Viaja, &["oficina"]),
    t("auditoria", Viaja, &["auditoria"]),
    // Decisões do gerente sobre conflitos (vão para todos os caixas).
    t("sync_resolucao", Viaja, &["sync"]),

    // Só deste caixa.
    t("alteracao", Local, &["loja", "sync"]),
    t("entrada_falha", Local, &["usuarios"]),
    t("menu_uso", Local, &["menu"]),
    t("produto_gtin", Local, &["codigos"]),
    t("senha_situacao", Local, &["usuarios"]),
    // Interna do SQLite (contadores do AUTOINCREMENT).
    t("sqlite_sequence", Local, &["sync"]),
    t("sync_conflito", Local, &["sync"]),
    t("sync_endereco", Local, &["backup", "caminhos"]),
    t("sync_estado", Local, &["backup", "loja", "sync"]),
    t("sync_par", Local, &["backup", "caminhos", "sync"]),
    t("sync_pendencia", Local, &["sync"]),
    t("sync_referencia", Local, &["sync"]),
    t("xml_pendencia_identificacao", Local, &["importar"]),

    // Visões (saldos calculados na hora).
    t("conta_pagar_saldo", Derivada, &[]),
    t("conta_receber_saldo", Derivada, &[]),
    t("vale_saldo", Derivada, &[]),
];

/// Tabelas temporárias (TEMP): criadas na hora, só na conexão que as usa.
#[rustfmt::skip]
pub const TEMPORARIAS: &[Tabela] = &[
    // Referências da troca de dados (veja `sync`).
    t("ref_atual", Local, &["sync"]),
];

/// Módulos que gravam numa tabela de nome montado na hora (a troca aplica o
/// que chegou de outro caixa em qualquer tabela que viaja).
pub const GRAVAM_QUALQUER_TABELA: &[&str] = &["loja"];

/// As tabelas que viajam, na ordem de `TABELAS` (é a `sync::TABELAS`).
pub const QUE_VIAJAM: [&str; contar(Viaja)] = nomes(Viaja);

const fn contar(c: Classe) -> usize {
    let mut n = 0;
    let mut i = 0;
    while i < TABELAS.len() {
        if TABELAS[i].classe as u8 == c as u8 {
            n += 1;
        }
        i += 1;
    }
    n
}

const fn nomes<const N: usize>(c: Classe) -> [&'static str; N] {
    let mut r = [""; N];
    let mut i = 0;
    let mut j = 0;
    while i < TABELAS.len() {
        if TABELAS[i].classe as u8 == c as u8 {
            r[j] = TABELAS[i].nome;
            j += 1;
        }
        i += 1;
    }
    r
}

#[cfg(test)]
mod testes {
    use std::collections::BTreeSet;

    use super::*;

    /// A `sync::TABELAS` de antes do esquema (até a dev.3.3, passo 3.1), na
    /// ordem de antes. O que viaja só muda com decisão (fase 5) e mudando esta
    /// lista junto.
    const VIAJAVAM: &[&str] = &[
        "config",
        "usuario",
        "tabela_preco",
        "regra_st",
        "unidade",
        "produto",
        "produto_preco",
        "produto_aplicacao",
        "produto_equivalente",
        "produto_fornecedor",
        "produto_codigo_antigo",
        "faixa_codigo",
        "estoque_movimento",
        "pessoa",
        "pessoa_veiculo",
        "compra",
        "compra_item",
        "caixa_sessao",
        "caixa_movimento",
        "venda",
        "venda_item",
        "venda_pagamento",
        "conta_receber",
        "recebimento",
        "recebimento_parcela",
        "conta_loja",
        "categoria",
        "conta_pagar",
        "pagamento",
        "movimento_conta",
        "cheque",
        "comissao_fechamento",
        "meta",
        "encomenda",
        "devolucao",
        "devolucao_item",
        "vale",
        "ordem_servico",
        "ordem_servico_item",
        "auditoria",
        "sync_resolucao",
    ];

    #[test]
    fn o_que_viaja_e_o_de_antes_na_mesma_ordem() {
        assert_eq!(QUE_VIAJAM, VIAJAVAM);
        assert_eq!(crate::sync::TABELAS, VIAJAVAM);
    }

    /// Toda tabela e visão de um banco novo (todas as migrações) tem linha
    /// aqui, e tudo daqui existe no banco.
    #[test]
    fn toda_tabela_do_banco_tem_classe() {
        let b = crate::banco::Banco::em_memoria().unwrap();
        let mut st = b.conn.prepare("SELECT name FROM sqlite_master WHERE type IN ('table', 'view')").unwrap();
        let no_banco: BTreeSet<String> = st.query_map([], |l| l.get(0)).unwrap().map(Result::unwrap).collect();
        let aqui: BTreeSet<String> = TABELAS.iter().map(|t| t.nome.to_string()).collect();
        let sem_classe: Vec<_> = no_banco.difference(&aqui).collect();
        assert!(sem_classe.is_empty(), "tabela sem classe no dados::esquema: {sem_classe:?}");
        let sobrando: Vec<_> = aqui.difference(&no_banco).collect();
        assert!(sobrando.is_empty(), "no dados::esquema, mas nenhuma migração cria: {sobrando:?}");
    }

    #[test]
    fn cada_tabela_uma_vez_escritores_em_ordem_e_visao_sem_escritor() {
        let mut vistas = BTreeSet::new();
        for t in TABELAS.iter().chain(TEMPORARIAS) {
            assert!(vistas.insert(t.nome), "{} aparece duas vezes", t.nome);
            assert!(t.escritores.windows(2).all(|w| w[0] < w[1]), "{}: escritores fora de ordem", t.nome);
            assert_eq!(t.classe == Derivada, t.escritores.is_empty(), "{}: só a visão não tem escritor", t.nome);
        }
    }

    #[test]
    fn o_dono_e_o_escritor_unico() {
        let de = |n: &str| TABELAS.iter().find(|t| t.nome == n).unwrap();
        assert_eq!(de("venda").dono(), Some("vendas"));
        assert_eq!(de("config").dono(), None, "config ainda tem 6 escritores (ARQ-01)");
        assert_eq!(de("vale_saldo").dono(), None);
    }
}
