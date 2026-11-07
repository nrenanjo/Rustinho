//! Unidades de medida (UN, KG, MT...). A lista vem pronta e a loja pode
//! criar as suas. As fracionadas (KG, LT, MT...) vendem quebrado: 0,350 kg.
//!
//! Ordem no seletor (Seção 6 do "Consolidação do padrão Rustinho", 0.9.18):
//! - ranking do cadastro: quantos produtos ATIVOS que não são serviço usam a
//!   unidade (mais produtos primeiro; empate pela sigla). O saldo em estoque
//!   não entra;
//! - grupo de vendas: as até 5 unidades com mais quantidade líquida vendida no
//!   intervalo (T − 24 h, T], T = abertura da pílula Peças (a que mais vendeu
//!   primeiro; empate pela sigla). Cancelamento e devolução só abatem venda
//!   feita dentro da janela, nunca abaixo de zero;
//! - 1ª linha: as 5 primeiras do ranking que não estão no grupo de vendas;
//! - 2ª linha: as seguintes do ranking (5 − k, com k vendidas) e, na direita,
//!   as k vendidas (sem vendas, a 2ª linha é o 6º ao 10º do ranking);
//! - linhas de baixo: todas as outras, pela sigla.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

use crate::erro::{Erro, Resultado};
use crate::texto;

/// Quantas unidades cabem em cada uma das duas primeiras linhas (e no grupo
/// de vendas).
pub const POR_LINHA: usize = 5;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Unidade {
    pub sigla: String,
    pub descricao: String,
    pub fracionada: bool,
    /// Quantos produtos ativos (sem os serviços) usam: o ranking do cadastro.
    pub produtos: i64,
    /// Quantidade líquida vendida na janela de 24 horas, em milésimos.
    pub vendida: i64,
    /// Linha no seletor: 1, 2 ou 3 (as linhas de baixo, pela sigla).
    pub linha: u8,
}

/// As unidades na ordem do seletor, com a janela de vendas terminando agora.
pub fn listar(conn: &Connection) -> Resultado<Vec<Unidade>> {
    listar_em(conn, "")
}

/// As unidades na ordem do seletor, com a janela de vendas terminando em
/// `referencia` (RFC 3339: a abertura da pílula Peças). Vazia, inválida ou
/// no futuro: agora.
pub fn listar_em(conn: &Connection, referencia: &str) -> Resultado<Vec<Unidade>> {
    let (inicio, fim) = janela(referencia);
    let mut c = conn.prepare(
        "SELECT u.sigla, u.descricao, u.fracionada,
                (SELECT COUNT(*) FROM produto p WHERE p.unidade = u.sigla AND p.ativo = 1 AND p.tipo <> 'servico')
           FROM unidade u",
    )?;
    let mut lista: Vec<Unidade> = c
        .query_map([], |l| {
            Ok(Unidade {
                sigla: l.get(0)?,
                descricao: l.get(1)?,
                fracionada: l.get(2)?,
                produtos: l.get(3)?,
                vendida: 0,
                linha: 3,
            })
        })?
        .collect::<Result<_, _>>()?;
    let vendidas = vendidas_na_janela(conn, &inicio, &fim)?;
    for u in &mut lista {
        u.vendida = vendidas.get(&u.sigla).copied().unwrap_or(0);
    }
    Ok(ordenar(lista))
}

/// O intervalo (início, fim] da janela de 24 horas, no formato das datas
/// gravadas ("2026-10-13T17:30:05Z"), para comparar como texto.
fn janela(referencia: &str) -> (String, String) {
    let agora = OffsetDateTime::now_utc();
    let t = OffsetDateTime::parse(referencia.trim(), &Rfc3339)
        .map(|t| t.to_offset(UtcOffset::UTC))
        .ok()
        .filter(|t| *t <= agora)
        .unwrap_or(agora);
    let formatar = |t: OffsetDateTime| t.replace_nanosecond(0).unwrap_or(t).format(&Rfc3339).unwrap_or_default();
    (formatar(t - time::Duration::hours(24)), formatar(t))
}

/// Quantidade líquida vendida por unidade em (inicio, fim]: os itens das
/// vendas feitas na janela (a unidade gravada no item, na hora da venda),
/// menos o que foi devolvido deles; venda cancelada não conta (nem as
/// devoluções dela: nada é abatido duas vezes). Devolução de venda de fora
/// da janela não abate nada. Cada item fica em zero no mínimo.
fn vendidas_na_janela(conn: &Connection, inicio: &str, fim: &str) -> Resultado<HashMap<String, i64>> {
    let mut c = conn.prepare(
        "SELECT vi.unidade,
                MAX(0, vi.quantidade - COALESCE((
                    SELECT SUM(di.quantidade) FROM devolucao_item di JOIN devolucao d ON d.id = di.devolucao_id
                     WHERE di.venda_item_id = vi.id AND d.criado_em <= ?2
                       AND (d.cancelado_em IS NULL OR d.cancelado_em > ?2)), 0))
           FROM venda v JOIN venda_item vi ON vi.venda_id = v.id
          WHERE v.tipo = 'venda' AND v.criado_em > ?1 AND v.criado_em <= ?2
            AND NOT (v.situacao = 'cancelada' AND COALESCE(v.cancelado_em, '') <= ?2)",
    )?;
    let mut total: HashMap<String, i64> = HashMap::new();
    let linhas = c.query_map(params![inicio, fim], |l| Ok((l.get::<_, String>(0)?, l.get::<_, i64>(1)?)))?;
    for linha in linhas {
        let (unidade, quantidade) = linha?;
        *total.entry(unidade.trim().to_uppercase()).or_insert(0) += quantidade;
    }
    total.retain(|_, q| *q > 0);
    Ok(total)
}

/// Põe as unidades na ordem do seletor e marca a linha de cada uma (regra no
/// começo do arquivo). Só usa `sigla`, `produtos` e `vendida`.
pub fn ordenar(mut todas: Vec<Unidade>) -> Vec<Unidade> {
    // Ranking do cadastro: mais produtos primeiro; empate pela sigla.
    todas.sort_by(|a, b| b.produtos.cmp(&a.produtos).then_with(|| a.sigla.cmp(&b.sigla)));
    // Grupo de vendas: as que mais venderam (empate pela sigla), até 5.
    let mut vendidas: Vec<usize> = (0..todas.len()).filter(|&i| todas[i].vendida > 0).collect();
    vendidas
        .sort_by(|&a, &b| todas[b].vendida.cmp(&todas[a].vendida).then_with(|| todas[a].sigla.cmp(&todas[b].sigla)));
    vendidas.truncate(POR_LINHA);
    // O resto do ranking, sem as do grupo de vendas (cada sigla uma vez só).
    let restantes: Vec<usize> = (0..todas.len()).filter(|i| !vendidas.contains(i)).collect();
    let na_primeira = restantes.len().min(POR_LINHA);
    let na_segunda = (restantes.len() - na_primeira).min(POR_LINHA - vendidas.len());
    let mut ordem: Vec<(usize, u8)> = Vec::with_capacity(todas.len());
    ordem.extend(restantes[..na_primeira].iter().map(|&i| (i, 1)));
    ordem.extend(restantes[na_primeira..na_primeira + na_segunda].iter().map(|&i| (i, 2)));
    ordem.extend(vendidas.iter().map(|&i| (i, 2)));
    let mut de_baixo: Vec<usize> = restantes[na_primeira + na_segunda..].to_vec();
    de_baixo.sort_by(|&a, &b| todas[a].sigla.cmp(&todas[b].sigla));
    ordem.extend(de_baixo.into_iter().map(|i| (i, 3)));
    let mut vagas: Vec<Option<Unidade>> = todas.into_iter().map(Some).collect();
    ordem
        .into_iter()
        .filter_map(|(i, linha)| {
            vagas[i].take().map(|mut u| {
                u.linha = linha;
                u
            })
        })
        .collect()
}

/// A sigla como o Rustinho guarda: maiúsculas, sem espaço, até 6 letras.
pub fn conferir_sigla(sigla: &str) -> Resultado<String> {
    let s = texto::limpar(sigla).to_uppercase();
    if s.is_empty() {
        return Err(Erro::campo("unidade", "Escolha a unidade."));
    }
    if texto::tamanho(&s) > 6 || s.contains(' ') {
        return Err(Erro::campo("unidade", "A unidade tem no máximo 6 letras, sem espaço (ex.: UN, KG, MT)."));
    }
    Ok(s)
}

/// Cria uma unidade da loja. Sigla que já existe é recusada (sem espaços
/// nas pontas e sem diferença de maiúsculas: "kg", "KG" e " KG " são a mesma),
/// em vez de trocar o nome e o "vende quebrado" da que já está lá (Seção 6.8).
/// Nomes iguais com siglas diferentes podem.
pub fn salvar(conn: &Connection, sigla: &str, descricao: &str, fracionada: bool) -> Resultado<String> {
    let sigla = conferir_sigla(sigla).map_err(|e| match e {
        Erro::Validacao { mensagem, .. } => Erro::campo("sigla", mensagem),
        outro => outro,
    })?;
    let descricao = texto::limpar(descricao);
    if texto::tamanho(&descricao) > 40 {
        return Err(Erro::campo("descricao", "Use no máximo 40 letras."));
    }
    let existentes: Vec<String> =
        conn.prepare("SELECT sigla FROM unidade")?.query_map([], |l| l.get(0))?.collect::<Result<_, _>>()?;
    if existentes.iter().any(|s| s.trim().to_uppercase() == sigla) {
        return Err(Erro::campo("sigla", format!("Já existe uma unidade com a sigla '{sigla}'. Escolha outra sigla.")));
    }
    let descricao = if descricao.is_empty() { sigla.clone() } else { descricao };
    conn.execute(
        "INSERT INTO unidade (sigla, descricao, fracionada) VALUES (?1, ?2, ?3)",
        params![sigla, descricao, fracionada],
    )?;
    Ok(sigla)
}

/// Garante que a unidade existe na lista (uma digitada ou vinda de outro
/// sistema entra como da loja, inteira).
pub fn garantir(conn: &Connection, sigla: &str) -> Resultado<()> {
    conn.execute("INSERT OR IGNORE INTO unidade (sigla, descricao) VALUES (?1, ?1)", [sigla])?;
    Ok(())
}

/// A unidade vende quebrado?
pub fn fracionada(conn: &Connection, sigla: &str) -> Resultado<bool> {
    Ok(conn
        .query_row("SELECT fracionada FROM unidade WHERE sigla = ?1", [sigla], |l| l.get(0))
        .optional()?
        .unwrap_or(false))
}

/// Junta os nomes repetidos que vêm de outros sistemas (PCS, PEC → PC).
pub fn normalizar(sigla: &str) -> String {
    let s = sigla.trim().to_uppercase();
    let igual = match s.as_str() {
        "UND" | "UNID" | "UNI" | "UNIDADE" | "UNIT" | "U" => "UN",
        "PCS" | "PEC" | "PECA" | "PEÇA" | "PÇ" | "PÇS" => "PC",
        "KT" => "KIT",
        "JOGO" => "JG",
        "PARES" => "PAR",
        "CONJ" => "CJ",
        "CXA" | "CAIXA" => "CX",
        "PACOTE" | "PAC" => "PCT",
        "KGL" | "KILO" | "QUILO" | "KGS" => "KG",
        "GR" | "GRS" => "G",
        "L" | "LITRO" | "LTS" | "LTR" => "LT",
        "M" | "MET" | "METRO" | "MTS" => "MT",
        "M²" => "M2",
        "M³" => "M3",
        "ROLO" => "RL",
        "GALAO" | "GALÃO" => "GL",
        _ => "",
    };
    if igual.is_empty() {
        s
    } else {
        igual.to_string()
    }
}

/// Unidade vinda de uma nota ou de outro sistema: junta as repetidas e, se
/// não servir (longa, com espaço), usa UN.
pub fn de_fora(sigla: &str) -> String {
    let s = normalizar(sigla);
    if s.is_empty() || texto::tamanho(&s) > 6 || s.contains(' ') {
        "UN".into()
    } else {
        s
    }
}

// ------------------------------------------------------------- comandos das telas

pub use comandos::COMANDOS;

/// Os comandos das unidades de medida (etapa 3 da modularização: vieram do `match` do
/// `api.rs` sem mudar nome, entrada, resposta nem permissão).
mod comandos {
    use serde::Deserialize;
    use serde_json::{json, Value};

    use crate::despacho::{ler, Classe, Comando, Pedido};
    use crate::erro::Resultado;
    use crate::unidades;

    pub const COMANDOS: &[Comando] = &[
        Comando { nome: "unidades_listar", classe: Classe::Leitura, funcao: unidades_listar },
        Comando { nome: "unidade_salvar", classe: Classe::Caixa, funcao: unidade_salvar },
    ];

    /// `referencia` (opcional): a abertura da pílula Peças, fim da janela de
    /// 24 horas das vendas. Sem ela, agora.
    fn unidades_listar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: ListarEntrada = ler(dados)?;
        Ok(json!(unidades::listar_em(p.conn, &d.referencia)?))
    }

    #[derive(Deserialize, Default)]
    #[serde(default, rename_all = "camelCase")]
    struct ListarEntrada {
        referencia: String,
    }

    fn unidade_salvar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: UnidadeEntrada = ler(dados)?;
        let sigla = unidades::salvar(p.conn, &d.sigla, &d.descricao, d.fracionada)?;
        Ok(json!({ "sigla": sigla, "lista": unidades::listar(p.conn)? }))
    }

    #[derive(Deserialize, Default)]
    #[serde(default, rename_all = "camelCase")]
    struct UnidadeEntrada {
        sigla: String,
        descricao: String,
        fracionada: bool,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;

    #[test]
    fn lista_pronta_e_da_loja() {
        let b = Banco::em_memoria().unwrap();
        let l = listar(&b.conn).unwrap();
        assert!(l.iter().any(|u| u.sigla == "KG" && u.fracionada));
        assert!(l.iter().any(|u| u.sigla == "UN" && !u.fracionada));
        assert!(!l.iter().any(|u| u.sigla == "L" || u.sigla == "KGL"));
        assert_eq!(salvar(&b.conn, " qd ", "Quadrado de 10x10", false).unwrap(), "QD");
        assert!(listar(&b.conn).unwrap().iter().any(|u| u.sigla == "QD" && u.descricao == "Quadrado de 10x10"));
        assert!(salvar(&b.conn, "MUITO LONGA", "", false).is_err());
        garantir(&b.conn, "610").unwrap();
        assert!(!fracionada(&b.conn, "610").unwrap());
        assert!(fracionada(&b.conn, "MT").unwrap());
    }

    // ---------------------------------------------------------------- ordem do seletor
    //
    // Os 19 critérios de teste da Seção 6.11 (numerados como lá). O 19 (5 por
    // linha no computador, 3 no celular) é da tela (`EscolherUnidade.qml`).

    /// Unidades de mentira: (sigla, produtos ativos, vendida em milésimos).
    fn un(lista: &[(&str, i64, i64)]) -> Vec<Unidade> {
        lista
            .iter()
            .map(|&(s, p, v)| Unidade {
                sigla: s.into(),
                descricao: s.into(),
                fracionada: false,
                produtos: p,
                vendida: v,
                linha: 0,
            })
            .collect()
    }

    /// As siglas de cada linha: (1ª, 2ª, de baixo).
    fn linhas(l: &[Unidade]) -> (Vec<String>, Vec<String>, Vec<String>) {
        let de = |n: u8| l.iter().filter(|u| u.linha == n).map(|u| u.sigla.clone()).collect::<Vec<_>>();
        (de(1), de(2), de(3))
    }

    /// A ordem devolvida é 1ª linha, 2ª linha e as de baixo, nessa sequência,
    /// e cada sigla aparece uma vez só (critério 14).
    fn conferir_forma(l: &[Unidade], total: usize) {
        assert_eq!(l.len(), total, "nenhuma unidade some");
        let mut siglas: Vec<&str> = l.iter().map(|u| u.sigla.as_str()).collect();
        siglas.sort();
        siglas.dedup();
        assert_eq!(siglas.len(), total, "sigla repetida: {l:?}");
        assert!(l.windows(2).all(|p| p[0].linha <= p[1].linha), "linhas fora de ordem: {l:?}");
        assert!(l.iter().filter(|u| u.linha == 1).count() <= POR_LINHA);
        assert!(l.iter().filter(|u| u.linha == 2).count() <= POR_LINHA);
    }

    /// Doze unidades com contagens distintas: UN 50, PC 40, CX 30, KG 20,
    /// LT 15 (top 5), F 9, G 8, H 7, I 6, J 5 (6º–10º), e A 1, B 0.
    fn doze(vendas: &[(&str, i64)]) -> Vec<Unidade> {
        let base = [
            ("UN", 50),
            ("PC", 40),
            ("CX", 30),
            ("KG", 20),
            ("LT", 15),
            ("F", 9),
            ("G", 8),
            ("H", 7),
            ("I", 6),
            ("J", 5),
            ("A", 1),
            ("B", 0),
        ];
        let lista: Vec<(&str, i64, i64)> =
            base.iter().map(|&(s, p)| (s, p, vendas.iter().find(|v| v.0 == s).map(|v| v.1).unwrap_or(0))).collect();
        un(&lista)
    }

    #[test]
    fn criterios_6_13_e_14_sem_vendas() {
        let l = ordenar(doze(&[]));
        conferir_forma(&l, 12);
        let (l1, l2, baixo) = linhas(&l);
        assert_eq!(l1, ["UN", "PC", "CX", "KG", "LT"], "1ª linha pelo ranking, da esquerda para a direita");
        assert_eq!(l2, ["F", "G", "H", "I", "J"], "6: 2ª linha = 6º ao 10º");
        assert_eq!(baixo, ["A", "B"], "13: de baixo pela sigla");
    }

    #[test]
    fn criterios_7_8_e_9_entrada_pela_direita() {
        // 7: a primeira vendida entra no lugar do 10º (J desce).
        let (l1, l2, baixo) = linhas(&ordenar(doze(&[("A", 1_000)])));
        assert_eq!(l1, ["UN", "PC", "CX", "KG", "LT"]);
        assert_eq!(l2, ["F", "G", "H", "I", "A"]);
        assert_eq!(baixo, ["B", "J"]);
        // 8 e 9: a segunda amplia para a esquerda; a que mais vendeu fica à
        // esquerda das vendidas (B 100, A 1 → [F, G, H, B, A]).
        let l = ordenar(doze(&[("A", 1_000), ("B", 100_000)]));
        conferir_forma(&l, 12);
        let (_, l2, baixo) = linhas(&l);
        assert_eq!(l2, ["F", "G", "H", "B", "A"]);
        assert_eq!(baixo, ["I", "J"], "as deslocadas descem, pela sigla");
        // Três vendidas ocupam as três últimas.
        let (_, l2, _) = linhas(&ordenar(doze(&[("A", 1_000), ("B", 100_000), ("J", 500)])));
        assert_eq!(l2, ["F", "G", "B", "A", "J"]);
    }

    #[test]
    fn criterio_10_empate_de_vendas_pela_sigla() {
        let (_, l2, _) = linhas(&ordenar(doze(&[("B", 2_000), ("A", 2_000)])));
        assert_eq!(l2, ["F", "G", "H", "A", "B"]);
    }

    #[test]
    fn criterio_11_vendida_da_primeira_linha_cede_o_lugar() {
        // CX (3º do cadastro) vendeu: vai para o grupo de vendas e F sobe
        // para a 1ª linha; a 2ª fica com G, H, I, J e CX na direita.
        let l = ordenar(doze(&[("CX", 3_000)]));
        conferir_forma(&l, 12);
        let (l1, l2, baixo) = linhas(&l);
        assert_eq!(l1, ["UN", "PC", "KG", "LT", "F"]);
        assert_eq!(l2, ["G", "H", "I", "J", "CX"]);
        assert_eq!(baixo, ["A", "B"]);
    }

    #[test]
    fn criterio_12_no_maximo_cinco_vendidas() {
        let vendas = [("A", 9_000), ("B", 8_000), ("J", 7_000), ("I", 6_000), ("H", 5_000), ("G", 4_000), ("UN", 1)];
        let l = ordenar(doze(&vendas));
        conferir_forma(&l, 12);
        let (l1, l2, baixo) = linhas(&l);
        assert_eq!(l1, ["UN", "PC", "CX", "KG", "LT"], "UN vendeu pouco: fica fora do grupo e na 1ª linha");
        assert_eq!(l2, ["A", "B", "J", "I", "H"], "só as 5 mais vendidas, da maior para a menor");
        assert_eq!(baixo, ["F", "G"], "a 6ª vendida (G) e a deslocada (F) descem, pela sigla");
    }

    #[test]
    fn criterio_2_desempate_do_cadastro_pela_sigla_e_poucas_unidades() {
        let l = ordenar(un(&[
            ("ZZ", 3, 0),
            ("MT", 3, 0),
            ("AB", 3, 0),
            ("KG", 5, 0),
            ("UN", 0, 0),
            ("CX", 0, 0),
            ("BD", 0, 0),
        ]));
        conferir_forma(&l, 7);
        let (l1, l2, baixo) = linhas(&l);
        assert_eq!(l1, ["KG", "AB", "MT", "ZZ", "BD"]);
        assert_eq!(l2, ["CX", "UN"], "menos de 10 unidades: só as que existem, sem inventar nenhuma");
        assert!(baixo.is_empty());
    }

    // ------------------------------------------------- critérios do banco (contagem e janela)

    const T: &str = "2026-11-05T12:00:00Z";

    /// Banco só com as unidades dadas (sem a lista pronta).
    fn banco_com(unidades: &[&str]) -> Banco {
        let b = Banco::em_memoria().unwrap();
        b.conn.execute("DELETE FROM unidade", []).unwrap();
        for s in unidades {
            b.conn.execute("INSERT INTO unidade (sigla, descricao) VALUES (?1, ?1)", [s]).unwrap();
        }
        b
    }

    fn produto(b: &Banco, id: &str, unidade: &str, ativo: bool, tipo: &str) {
        b.conn
            .execute(
                "INSERT INTO produto (id, codigo, nome, unidade, ativo, tipo, criado_em, atualizado_em, dispositivo)
                 VALUES (?1, ?1, ?1, ?2, ?3, ?4, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'teste')",
                params![id, unidade, ativo, tipo],
            )
            .unwrap();
    }

    /// Uma venda concluída em `quando`, com itens (id do item, unidade, milésimos).
    fn venda(b: &Banco, id: &str, quando: &str, itens: &[(&str, &str, i64)]) {
        b.conn
            .execute(
                "INSERT INTO venda (id, tipo, numero, dispositivo, caixa_nome, situacao, subtotal, total, criado_em)
                 VALUES (?1, 'venda', 1, 'teste', 'Caixa', 'concluida', 0, 0, ?2)",
                params![id, quando],
            )
            .unwrap();
        for (n, (item, unidade, q)) in itens.iter().enumerate() {
            b.conn
                .execute(
                    "INSERT INTO venda_item (id, venda_id, ordem, descricao, unidade, quantidade, preco, total)
                     VALUES (?1, ?2, ?3, 'x', ?4, ?5, 0, 0)",
                    params![item, id, n as i64, unidade, q],
                )
                .unwrap();
        }
    }

    fn cancelar(b: &Banco, venda: &str, quando: &str) {
        b.conn
            .execute("UPDATE venda SET situacao = 'cancelada', cancelado_em = ?2 WHERE id = ?1", params![venda, quando])
            .unwrap();
    }

    /// Uma devolução em `quando` de itens de `venda`: (id do item da venda, milésimos).
    fn devolver(b: &Banco, id: &str, venda: &str, quando: &str, itens: &[(&str, i64)]) {
        b.conn
            .execute(
                "INSERT INTO devolucao (id, numero, dispositivo, caixa_nome, venda_id, valor, criado_em)
                 VALUES (?1, 1, 'teste', 'Caixa', ?2, 0, ?3)",
                params![id, venda, quando],
            )
            .unwrap();
        for (n, (item, q)) in itens.iter().enumerate() {
            b.conn
                .execute(
                    "INSERT INTO devolucao_item (id, devolucao_id, venda_item_id, descricao, quantidade, valor_item, valor)
                     VALUES (?1, ?2, ?3, 'x', ?4, 0, 0)",
                    params![format!("{id}-{n}"), id, item, q],
                )
                .unwrap();
        }
    }

    fn vendida(l: &[Unidade], sigla: &str) -> i64 {
        l.iter().find(|u| u.sigla == sigla).map(|u| u.vendida).unwrap()
    }

    #[test]
    fn criterios_1_2_3_4_e_5_ranking_do_cadastro() {
        let b = banco_com(&["UN", "PC", "CX", "KG", "LT", "MT"]);
        // PC: 3 ativos; UN: 2 ativos + 1 inativo + 1 serviço; CX e KG: 2 cada
        // (empate: CX antes); LT: 1; MT: 4 serviços e 2 inativos.
        for (id, unidade, ativo, tipo) in [
            ("p1", "PC", true, "produto"),
            ("p2", "PC", true, "produto"),
            ("p3", "PC", true, "produto"),
            ("u1", "UN", true, "produto"),
            ("u2", "UN", true, "produto"),
            ("u3", "UN", false, "produto"),
            ("u4", "UN", true, "servico"),
            ("k1", "KG", true, "produto"),
            ("k2", "KG", true, "produto"),
            ("c1", "CX", true, "produto"),
            ("c2", "CX", true, "produto"),
            ("l1", "LT", true, "produto"),
            ("m1", "MT", true, "servico"),
            ("m2", "MT", true, "servico"),
            ("m3", "MT", true, "servico"),
            ("m4", "MT", true, "servico"),
            ("m5", "MT", false, "produto"),
            ("m6", "MT", false, "produto"),
        ] {
            produto(&b, id, unidade, ativo, tipo);
        }
        // 5: saldo em estoque não muda o ranking (LT com muito estoque).
        crate::estoque::lancar(&b.conn, "teste", None, "l1", 9_999_000, "ajuste", "teste").unwrap();
        let l = listar_em(&b.conn, T).unwrap();
        conferir_forma(&l, 6);
        let ordem: Vec<(&str, i64)> = l.iter().map(|u| (u.sigla.as_str(), u.produtos)).collect();
        assert_eq!(ordem, [("PC", 3), ("CX", 2), ("KG", 2), ("UN", 2), ("LT", 1), ("MT", 0)]);
        let (l1, l2, _) = linhas(&l);
        assert_eq!(l1, ["PC", "CX", "KG", "UN", "LT"]);
        assert_eq!(l2, ["MT"]);
    }

    #[test]
    fn criterios_15_16_17_e_18_janela_de_24_horas() {
        let b = banco_com(&["UN", "PC", "CX", "KG", "LT", "F", "G", "H", "I", "J", "A", "B", "C"]);
        // Fora da janela (T − 24 h exato não entra; T entra).
        venda(&b, "v0", "2026-11-04T12:00:00Z", &[("v0a", "A", 50_000)]);
        venda(&b, "v1", "2026-11-04T12:00:01Z", &[("v1a", "A", 1_000), ("v1b", "KG", 350)]);
        venda(&b, "v2", T, &[("v2a", "B", 2_000)]);
        // Depois de T (não conta para esta abertura).
        venda(&b, "v3", "2026-11-05T12:00:01Z", &[("v3a", "C", 9_000)]);
        let l = listar_em(&b.conn, T).unwrap();
        conferir_forma(&l, 13);
        assert_eq!((vendida(&l, "A"), vendida(&l, "KG"), vendida(&l, "B"), vendida(&l, "C")), (1_000, 350, 2_000, 0));
        let (_, l2, _) = linhas(&l);
        assert_eq!(&l2[2..], ["B", "A", "KG"], "fração conta (0,350 kg)");

        // 15: cancelamento de venda da janela abate tudo; devolução abate o
        // devolvido; o cancelamento depois de uma devolução não abate duas vezes.
        venda(&b, "v4", "2026-11-05T08:00:00Z", &[("v4a", "C", 3_000)]);
        devolver(&b, "d4", "v4", "2026-11-05T09:00:00Z", &[("v4a", 1_000)]);
        let l = listar_em(&b.conn, T).unwrap();
        assert_eq!(vendida(&l, "C"), 2_000);
        cancelar(&b, "v4", "2026-11-05T10:00:00Z");
        assert_eq!(vendida(&listar_em(&b.conn, T).unwrap(), "C"), 0, "cancelada: some, sem abater de novo a devolução");
        // A devolução desfeita deixa de abater.
        venda(&b, "v5", "2026-11-05T08:30:00Z", &[("v5a", "J", 4_000)]);
        devolver(&b, "d5", "v5", "2026-11-05T09:30:00Z", &[("v5a", 4_000)]);
        assert_eq!(vendida(&listar_em(&b.conn, T).unwrap(), "J"), 0, "líquida zero: deixa de contar como vendida");
        b.conn.execute("UPDATE devolucao SET cancelado_em = '2026-11-05T10:30:00Z' WHERE id = 'd5'", []).unwrap();
        assert_eq!(vendida(&listar_em(&b.conn, T).unwrap(), "J"), 4_000);

        // 16 e 17: devolução de venda de antes da janela não abate nada e não
        // deixa nada negativo.
        devolver(&b, "d0", "v0", "2026-11-05T11:00:00Z", &[("v0a", 50_000)]);
        let l = listar_em(&b.conn, T).unwrap();
        assert_eq!(vendida(&l, "A"), 1_000);
        assert!(l.iter().all(|u| u.vendida >= 0));
        // Devolução maior que o vendido (dado estranho): o item fica em zero.
        devolver(&b, "d1", "v1", "2026-11-05T11:10:00Z", &[("v1a", 5_000)]);
        let l = listar_em(&b.conn, T).unwrap();
        assert_eq!(vendida(&l, "A"), 0);
        assert!(l.iter().all(|u| u.vendida >= 0));
        // ...e esse item no zero não come a venda de outro item da mesma
        // unidade (sem negativo artificial: 0 + 2 = 2, não −4 + 2).
        venda(&b, "v8", "2026-11-05T11:15:00Z", &[("v8a", "A", 2_000)]);
        assert_eq!(vendida(&listar_em(&b.conn, T).unwrap(), "A"), 2_000);

        // A abertura em T vê o banco como estava em T: devolução e
        // cancelamento feitos depois de T só contam numa abertura seguinte.
        venda(&b, "v6", "2026-11-05T11:20:00Z", &[("v6a", "F", 6_000)]);
        devolver(&b, "d6", "v6", "2026-11-05T12:30:00Z", &[("v6a", 6_000)]);
        venda(&b, "v7", "2026-11-05T11:30:00Z", &[("v7a", "G", 7_000)]);
        cancelar(&b, "v7", "2026-11-05T12:40:00Z");
        let l = listar_em(&b.conn, T).unwrap();
        assert_eq!((vendida(&l, "F"), vendida(&l, "G")), (6_000, 7_000));

        // 18: outra abertura (T + 2 h) recalcula a janela: v1 saiu, v3 entrou,
        // e a devolução e o cancelamento de depois de T passam a valer.
        let depois = listar_em(&b.conn, "2026-11-05T14:00:00Z").unwrap();
        assert_eq!((vendida(&depois, "KG"), vendida(&depois, "C")), (0, 9_000));
        assert_eq!((vendida(&depois, "F"), vendida(&depois, "G")), (0, 0));
        // Referência estranha ou no futuro: vale agora (nada disso é de agora).
        for r in ["", "ontem", "2999-01-01T00:00:00Z"] {
            assert!(listar_em(&b.conn, r).unwrap().iter().all(|u| u.vendida == 0), "{r}");
        }
        // Orçamento não é venda.
        b.conn.execute("UPDATE venda SET tipo = 'orcamento' WHERE id = 'v2'", []).unwrap();
        assert_eq!(vendida(&listar_em(&b.conn, T).unwrap(), "B"), 0);
    }

    #[test]
    fn sigla_repetida_e_recusada_sem_mudar_a_que_existe() {
        let b = Banco::em_memoria().unwrap();
        for tentativa in ["kg", "KG", " KG ", " kg"] {
            let e = salvar(&b.conn, tentativa, "Quilo de outra coisa", false).unwrap_err();
            match e {
                Erro::Validacao { campo, mensagem } => {
                    assert_eq!(campo.as_deref(), Some("sigla"));
                    assert_eq!(mensagem, "Já existe uma unidade com a sigla 'KG'. Escolha outra sigla.");
                }
                outro => panic!("{outro:?}"),
            }
        }
        let kg = listar(&b.conn).unwrap().into_iter().find(|u| u.sigla == "KG").unwrap();
        assert!(kg.fracionada, "a KG continua vendendo quebrado");
        assert_ne!(kg.descricao, "Quilo de outra coisa");
        // Nome igual com sigla diferente pode.
        assert_eq!(salvar(&b.conn, "kgx", &kg.descricao, true).unwrap(), "KGX");
    }

    #[test]
    fn junta_repetidas() {
        for (de, para) in [
            ("PCS", "PC"),
            ("pec", "PC"),
            ("KGL", "KG"),
            ("L", "LT"),
            ("MET", "MT"),
            ("M", "MT"),
            ("KT", "KIT"),
            ("QD", "QD"),
        ] {
            assert_eq!(normalizar(de), para);
        }
    }
}
