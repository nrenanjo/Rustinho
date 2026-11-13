//! O esquema do banco numa fonte só (dev.3.3, passo 3.2; ARQ-10, contrato C2
//! da `ARQUITETURA_ALVO_RUSTINHO.md`): cada tabela, a classe dela na troca de
//! dados entre os caixas e os módulos que gravam nela, **como está hoje**.
//!
//! Antes a lista de tabelas estava em três lugares: as migrações, a
//! `sync::TABELAS` (as que viajam) e a `ESCRITORES` do teste de dependências.
//! Agora as duas listas saem daqui, e um teste confere que toda tabela e
//! visão criada pelas migrações tem a sua linha (tabela sem classe falha).
//!
//! As tabelas de classe `Viaja` são as de antes, na mesma ordem (é a ordem em
//! que a troca as percorre), e no fim as que passaram a viajar no passo 5.2
//! (DP-15: o `produto_gtin`; veja [`PASSARAM_A_VIAJAR`]).

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
    t("produto_fornecedor", Viaja, &["codigos"]),
    // Códigos que uma peça já teve.
    t("produto_codigo_antigo", Viaja, &["codigos"]),
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
    // Os GTINs da peça (PEND-35, DP-15: viajam desde o passo 5.2).
    t("produto_gtin", Viaja, &["codigos"]),

    // Só deste caixa.
    t("alteracao", Local, &["loja", "sync"]),
    t("entrada_falha", Local, &["usuarios"]),
    t("menu_uso", Local, &["menu"]),
    t("senha_situacao", Local, &["usuarios"]),
    // Interna do SQLite (contadores do AUTOINCREMENT).
    t("sqlite_sequence", Local, &["sync"]),
    t("sync_conflito", Local, &["sync"]),
    t("sync_endereco", Local, &["backup", "caminhos"]),
    t("sync_estado", Local, &["backup", "loja", "sync"]),
    t("sync_par", Local, &["backup", "caminhos", "sync"]),
    t("sync_pendencia", Local, &["sync"]),
    t("sync_referencia", Local, &["sync"]),
    t("xml_pendencia_identificacao", Local, &["pendencias"]),

    // Visões (saldos calculados na hora).
    t("conta_pagar_saldo", Derivada, &[]),
    t("conta_receber_saldo", Derivada, &[]),
    t("vale_saldo", Derivada, &[]),
];

// ---------------------------------------------------------------------------
// Colunas que não seguem a classe da tabela (passo 5.2, DP-15; contrato C2).
// ---------------------------------------------------------------------------

/// Classe de uma coluna de tabela que viaja, quando ela não viaja junto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClasseDaColuna {
    /// Calculada em cada caixa a partir de outras colunas e de outras tabelas
    /// (ex.: o índice de busca da peça). Não viaja, não conta como conflito,
    /// e quem recebe uma alteração (ou o gerente resolve um conflito) a refaz
    /// pelo [`Derivado`] dito aqui.
    Derivada(Derivado),
    /// Só deste caixa (ex.: a hora do último acesso). Não viaja, não conta
    /// como conflito, e o que chegar de outro caixa é ignorado.
    Local,
}

/// Um valor derivado e quem o refaz. A troca de dados chama o dono (veja
/// `troca::sync`, `refazer_derivado`: um `match` sem `_`, então um derivado
/// novo não compila sem dizer quem o refaz).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Derivado {
    /// O índice de busca da peça (`produto.busca`, `produto.codigos`), que
    /// sai da peça inteira: `catalogo::codigos::refazer_indice`.
    IndiceDaPeca,
}

/// Uma coluna com classe própria.
#[derive(Debug)]
pub struct Coluna {
    pub tabela: &'static str,
    pub nome: &'static str,
    pub classe: ClasseDaColuna,
}

const fn c(tabela: &'static str, nome: &'static str, classe: ClasseDaColuna) -> Coluna {
    Coluna { tabela, nome, classe }
}

/// As colunas de tabelas que viajam que NÃO viajam. Os gatilhos da troca
/// são gerados sem elas (nem no conteúdo da alteração, nem como motivo para
/// gravar uma alteração), quem recebe as ignora e as refaz, e elas não
/// contam como conflito.
///
/// **Como registrar uma coluna derivada nova** (ex.: as colunas normalizadas
/// da peça, passo 7.5):
/// 1. a migração cria a coluna, e o dono a grava (como hoje);
/// 2. uma linha aqui: `c("produto", "<coluna>", Derivada(Derivado::IndiceDaPeca))`,
///    se ela sai da peça inteira e o `catalogo::codigos::refazer_indice` passa
///    a gravá-la; ou um `Derivado` novo, com o dono que a refaz no `match` de
///    `troca::sync::refazer_derivado`;
/// 3. as tabelas que a alimentam em [`ALIMENTAM`] (as da peça já estão);
/// 4. os testes deste módulo conferem que a coluna existe, não é da chave,
///    tem quem a alimente e não vai nos gatilhos.
///
/// Mudar esta lista muda o que viaja: exige todos os caixas na mesma versão
/// (o recurso de troca `classes-v1`, veja `troca::sync`).
#[rustfmt::skip]
pub const COLUNAS: &[Coluna] = &[
    // C07: a hora da última entrada é de cada caixa (entrar não gera troca).
    c("usuario", "ultimo_acesso", ClasseDaColuna::Local),
    // D01, D02, D05: o índice de busca da peça sai da peça inteira (inclusive
    // dos GTINs, das aplicações e das equivalências) e é refeito em cada caixa.
    c("produto", "busca", ClasseDaColuna::Derivada(Derivado::IndiceDaPeca)),
    c("produto", "codigos", ClasseDaColuna::Derivada(Derivado::IndiceDaPeca)),
];

/// Tabelas que viajam cujas linhas alimentam um derivado: (tabela, coluna com
/// o id da linha dona do derivado, derivado). Ex.: uma aplicação da peça que
/// chega muda o índice da peça `produto_id`.
#[rustfmt::skip]
pub const ALIMENTAM: &[(&str, &str, Derivado)] = &[
    ("produto", "id", Derivado::IndiceDaPeca),
    ("produto_aplicacao", "produto_id", Derivado::IndiceDaPeca),
    ("produto_equivalente", "produto_id", Derivado::IndiceDaPeca),
    ("produto_gtin", "produto_id", Derivado::IndiceDaPeca),
];

/// A coluna viaja com a tabela?
pub fn coluna_viaja(tabela: &str, coluna: &str) -> bool {
    !COLUNAS.iter().any(|c| c.tabela == tabela && c.nome == coluna)
}

/// Algum caixa tem uma coluna com este nome que não viaja (para comparar
/// versões sem saber a tabela: o nome basta, veja `sync::diferencas`)?
pub fn nome_que_nao_viaja(coluna: &str) -> bool {
    COLUNAS.iter().any(|c| c.nome == coluna)
}

/// Se as linhas da tabela alimentam um derivado: (coluna com o id da linha
/// dona, derivado).
pub fn alimenta(tabela: &str) -> Option<(&'static str, Derivado)> {
    ALIMENTAM.iter().find(|a| a.0 == tabela).map(|a| (a.1, a.2))
}

/// Tabelas que passaram a viajar depois que a troca de dados já existia (no
/// fim de [`TABELAS`], entre as que viajam). Num banco de antes, as linhas que
/// elas já tinham nunca viraram alteração: ao abrir, viram uma vez só (veja
/// `sync::registrar_existentes`; a marca fica na configuração deste caixa).
pub const PASSARAM_A_VIAJAR: &[&str] = &["produto_gtin"];

/// Chave natural de uma tabela que viaja e cuja chave primária é um id
/// sorteado: o mesmo dado pode nascer em dois caixas com ids diferentes (o
/// mesmo GTIN na mesma peça, gravado pelas notas de cada caixa). Ao receber,
/// a linha com a mesma chave natural já gravada aqui fica; a que chegou entra
/// só no log (não trava o lote nem vira pendência). Hoje essas linhas só são
/// criadas e apagadas junto com a peça (nunca alteradas).
pub const CHAVES_NATURAIS: &[(&str, &[&str])] = &[("produto_gtin", &["produto_id", "gtin", "tipo", "origem"])];

/// A chave natural da tabela, se ela tem (veja [`CHAVES_NATURAIS`]).
pub fn chave_natural(tabela: &str) -> Option<&'static [&'static str]> {
    CHAVES_NATURAIS.iter().find(|c| c.0 == tabela).map(|c| c.1)
}

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

    /// As de antes, na mesma ordem; no fim, as que passaram a viajar (5.2).
    #[test]
    fn o_que_viaja_e_o_de_antes_na_mesma_ordem() {
        let esperado: Vec<&str> = VIAJAVAM.iter().chain(PASSARAM_A_VIAJAR).copied().collect();
        assert_eq!(QUE_VIAJAM.to_vec(), esperado);
        assert_eq!(crate::sync::TABELAS.to_vec(), esperado);
    }

    /// A chave natural declarada é um índice único da tabela (é ele que faz
    /// a linha repetida ser reconhecida ao receber).
    #[test]
    fn chave_natural_e_um_indice_unico() {
        let b = crate::banco::Banco::em_memoria().unwrap();
        for (tabela, cols) in CHAVES_NATURAIS {
            assert!(QUE_VIAJAM.contains(tabela), "{tabela}");
            let indices: Vec<(String, bool)> = b
                .conn
                .prepare(&format!("PRAGMA index_list(\"{tabela}\")"))
                .unwrap()
                .query_map([], |l| Ok((l.get(1)?, l.get(2)?)))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            let achou = indices.iter().filter(|i| i.1).any(|(nome, _)| {
                let colunas: Vec<String> = b
                    .conn
                    .prepare(&format!("PRAGMA index_info(\"{nome}\")"))
                    .unwrap()
                    .query_map([], |l| l.get(2))
                    .unwrap()
                    .map(Result::unwrap)
                    .collect();
                colunas == *cols
            });
            assert!(achou, "{tabela}: nenhum índice único com {cols:?}");
        }
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

    /// Passo 5.2 (contrato C2): toda coluna com classe própria existe numa
    /// tabela que viaja, não é da chave primária, e a derivada tem quem a
    /// alimente.
    #[test]
    fn colunas_com_classe_existem_e_tem_quem_as_refaca() {
        let b = crate::banco::Banco::em_memoria().unwrap();
        let mut vistas = BTreeSet::new();
        for col in COLUNAS {
            assert!(vistas.insert((col.tabela, col.nome)), "{}.{} aparece duas vezes", col.tabela, col.nome);
            assert!(QUE_VIAJAM.contains(&col.tabela), "{}: só tabela que viaja tem coluna com classe", col.tabela);
            let (todas, chave) = crate::sync::colunas_da_tabela(&b.conn, col.tabela).unwrap();
            assert!(todas.iter().any(|x| x == col.nome), "{}.{} não existe", col.tabela, col.nome);
            assert!(!chave.iter().any(|x| x == col.nome), "{}.{} é da chave: tem de viajar", col.tabela, col.nome);
            if let ClasseDaColuna::Derivada(d) = col.classe {
                assert!(ALIMENTAM.iter().any(|a| a.2 == d), "{}.{}: ninguém alimenta {d:?}", col.tabela, col.nome);
            }
        }
        for (tabela, coluna, _) in ALIMENTAM {
            assert!(QUE_VIAJAM.contains(tabela), "{tabela} alimenta um derivado mas não viaja");
            let (todas, _) = crate::sync::colunas_da_tabela(&b.conn, tabela).unwrap();
            assert!(todas.iter().any(|x| x == coluna), "{tabela}.{coluna} não existe");
        }
    }

    /// Passo 5.2 (ARQ-10, contrato C2: "coluna derivada no gatilho falha"):
    /// os gatilhos da troca são gerados do esquema; nenhuma coluna que não
    /// viaja vai no conteúdo da alteração.
    #[test]
    fn nenhuma_coluna_que_nao_viaja_vai_nos_gatilhos() {
        let b = crate::banco::Banco::em_memoria().unwrap();
        let gatilhos: Vec<(String, String)> = b
            .conn
            .prepare(
                "SELECT tbl_name, sql FROM sqlite_master WHERE type = 'trigger' AND name LIKE 'sync\\_%' ESCAPE '\\'",
            )
            .unwrap()
            .query_map([], |l| Ok((l.get(0)?, l.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        for t in QUE_VIAJAM {
            assert_eq!(gatilhos.iter().filter(|g| g.0 == *t).count(), 3, "{t}: os três gatilhos da troca");
        }
        for col in COLUNAS {
            for (_, sql) in gatilhos.iter().filter(|g| g.0 == col.tabela) {
                for prefixo in ["NEW", "OLD"] {
                    let no_conteudo = format!("'{}', {prefixo}.\"{}\"", col.nome, col.nome);
                    assert!(!sql.contains(&no_conteudo), "{}.{} vai no gatilho: {sql}", col.tabela, col.nome);
                }
            }
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
