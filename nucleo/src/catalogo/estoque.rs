//! Estoque: o saldo de cada peça é a soma dos seus movimentos.
//!
//! Nunca se "troca o número" do estoque. Cada entrada ou saída vira um
//! movimento (estoque inicial, ajuste, venda, compra...). Assim dá para saber
//! sempre de onde veio cada unidade, e os caixas poderão juntar seus
//! movimentos sem perder nada.

use rusqlite::{params, Connection, OptionalExtension};

use crate::erro::{Erro, Resultado};
use crate::{auditoria, numeros, tempo, texto};

/// Saldo atual da peça, em milésimos.
pub fn saldo(conn: &Connection, produto_id: &str) -> Resultado<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(quantidade), 0) FROM estoque_movimento WHERE produto_id = ?1",
        [produto_id],
        |l| l.get(0),
    )?)
}

/// R13 (DP-42): a raiz de uma peça: segue as junções (`produto_juncao`, peça juntada em outra) até a peça que não
/// foi juntada. No máximo 8 passos; num ciclo (duas peças juntadas uma na outra em caixas sem rede), a raiz é o
/// menor id do ciclo (a Duplicatas mostra o ciclo para o gerente reativar uma). Só lê.
pub fn raiz(conn: &Connection, produto_id: &str) -> Resultado<String> {
    let mut vistos = vec![produto_id.to_string()];
    for _ in 0..8 {
        let atual = vistos.last().cloned().unwrap_or_default();
        let para: Option<String> = conn
            .query_row("SELECT para_id FROM produto_juncao WHERE produto_id = ?1", [&atual], |l| l.get(0))
            .optional()?;
        match para {
            None => return Ok(atual),
            Some(p) => {
                if let Some(i) = vistos.iter().position(|v| *v == p) {
                    return Ok(vistos[i..].iter().min().cloned().unwrap_or(p));
                }
                vistos.push(p);
            }
        }
    }
    Ok(vistos.last().cloned().unwrap_or_default())
}

/// Lança um movimento de estoque (positivo entra, negativo sai). Serviço não
/// tem estoque: para ele, não lança nada. R13 (F3): lança na raiz da peça (a devolução, o cancelamento e o estorno
/// de compra de uma peça juntada entram na peça que ficou).
pub fn lancar(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: Option<&str>,
    produto_id: &str,
    quantidade: i64,
    tipo: &str,
    motivo: &str,
) -> Resultado<()> {
    let produto_id = raiz(conn, produto_id)?;
    let produto_id = produto_id.as_str();
    conn.execute(
        "INSERT INTO estoque_movimento (id, produto_id, quantidade, tipo, motivo, usuario_id, dispositivo, criado_em)
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8
          WHERE NOT EXISTS (SELECT 1 FROM produto WHERE id = ?2 AND tipo = 'servico')",
        params![tempo::novo_id(), produto_id, quantidade, tipo, motivo, usuario_id, dispositivo, tempo::agora()],
    )?;
    Ok(())
}

/// R13 (DP-42): passa o saldo da peça juntada (D) para a raiz (K) com duas linhas diretas (sem o desvio do
/// `lancar`, que mandaria a saída de D para K). O id é pelo conteúdo: o resumo dos ids dos movimentos de D que entram
/// no saldo, e K; `criado_em` é o do último deles e `usuario_id` fica vazio (quem fez está no registro). Assim dois
/// caixas com a mesma visão gravam as mesmas linhas (sem conflito na troca); visões diferentes, ou D juntada em peças
/// diferentes (a disputa), gravam pares diferentes, cada um equilibrado (o total da loja não muda; na disputa D fica
/// negativa até o gerente escolher, e [`desfazer_passagem`] devolve o par da peça que perdeu). Saldo 0 não grava nada.
/// Devolve a quantidade passada.
pub fn passar_por_juncao(conn: &Connection, dispositivo: &str, d: &str, k: &str) -> Resultado<i64> {
    use sha2::{Digest, Sha256};
    let mut movimentos: Vec<(String, i64, String)> = conn
        .prepare("SELECT id, quantidade, criado_em FROM estoque_movimento WHERE produto_id = ?1")?
        .query_map([d], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let saldo: i64 = movimentos.iter().map(|m| m.1).sum();
    if saldo == 0 {
        return Ok(0);
    }
    movimentos.sort();
    let ids: Vec<&str> = movimentos.iter().map(|m| m.0.as_str()).collect();
    let resumo: String = Sha256::digest(ids.join(",").as_bytes()).iter().take(16).map(|b| format!("{b:02x}")).collect();
    let em = movimentos.iter().map(|m| m.2.clone()).max().unwrap_or_else(tempo::agora);
    for (sufixo, peca, quantidade) in [("saida", d, -saldo), ("entrada", k, saldo)] {
        conn.execute(
            "INSERT OR IGNORE INTO estoque_movimento (id, produto_id, quantidade, tipo, motivo, usuario_id, dispositivo, criado_em)
             VALUES (?1, ?2, ?3, 'juncao', 'Peças juntadas', NULL, ?4, ?5)",
            params![format!("juntar:{resumo}:{sufixo}:{k}"), peca, quantidade, dispositivo, em],
        )?;
    }
    Ok(saldo)
}

/// R13 (definição final 4): o gerente escolheu outra peça para D; as passagens de D para `perdeu` voltam para D (um
/// par de volta para cada entrada, com id pelo id dela: idempotente e igual em qualquer caixa). Devolve quantas.
pub fn desfazer_passagem(conn: &Connection, dispositivo: &str, d: &str, perdeu: &str) -> Resultado<usize> {
    let sufixo = format!(":entrada:{perdeu}");
    let entradas: Vec<(String, i64, String)> = conn
        .prepare(
            "SELECT id, quantidade, criado_em FROM estoque_movimento
              WHERE produto_id = ?1 AND tipo = 'juncao' AND substr(id, 1, 7) = 'juntar:'
                AND substr(id, -length(?2)) = ?2",
        )?
        .query_map(params![perdeu, sufixo], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?
        .collect::<Result<_, _>>()?;
    for (id, quantidade, em) in &entradas {
        for (lado, peca, q) in [("saida", perdeu, -quantidade), ("entrada", d, *quantidade)] {
            conn.execute(
                "INSERT OR IGNORE INTO estoque_movimento (id, produto_id, quantidade, tipo, motivo, usuario_id, dispositivo, criado_em)
                 VALUES (?1, ?2, ?3, 'juncao', 'Junção escolhida em outra peça', NULL, ?4, ?5)",
                params![format!("{id}:volta:{lado}"), peca, q, dispositivo, em],
            )?;
        }
    }
    Ok(entradas.len())
}

/// A mensagem de quantidade quebrada numa unidade que não vende quebrado.
pub fn inteira(unidade: &str) -> String {
    format!("{unidade} é contado inteiro (sem vírgula).")
}

/// Acerta o estoque para a quantidade contada agora. Lança só a diferença.
/// Devolve o novo saldo.
pub fn ajustar(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    produto_id: &str,
    contado: &str,
    motivo: &str,
) -> Resultado<i64> {
    let contado = numeros::ler_quantidade(contado)
        .map_err(|m| Erro::campo("quantidade", m))?
        .ok_or_else(|| Erro::campo("quantidade", "Digite a quantidade contada."))?;
    if contado < 0 {
        return Err(Erro::campo("quantidade", "A quantidade não pode ser negativa."));
    }
    let motivo = texto::limpar(motivo);
    if motivo.is_empty() {
        return Err(Erro::campo("motivo", "Diga o motivo do ajuste."));
    }
    if texto::tamanho(&motivo) > 120 {
        return Err(Erro::campo("motivo", "Use no máximo 120 letras."));
    }
    let (servico, unidade, fracionada): (bool, String, bool) = conn
        .query_row(
            "SELECT p.tipo = 'servico', p.unidade, COALESCE((SELECT fracionada FROM unidade WHERE sigla = p.unidade), 0)
               FROM produto p WHERE p.id = ?1",
            [produto_id],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Peça".into()))?;
    if servico {
        return Err(Erro::validacao("Serviço não tem estoque."));
    }
    // NEW-B8B2-01 (0.9.18): unidade inteira (UN, PC, PAR...) não fica com
    // estoque quebrado, como já não vende quebrado.
    if contado % 1000 != 0 && !fracionada {
        return Err(Erro::campo("quantidade", inteira(&unidade)));
    }
    let atual = saldo(conn, produto_id)?;
    let diferenca = contado - atual;
    if diferenca == 0 {
        return Err(Erro::campo("quantidade", "O estoque já está com essa quantidade."));
    }
    lancar(conn, dispositivo, Some(usuario_id), produto_id, diferenca, "ajuste", &motivo)?;
    let sinal = if diferenca > 0 { "+" } else { "" };
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "estoque.ajustar",
        "produto",
        produto_id,
        &[
            format!(
                "Estoque: {} → {} ({sinal}{})",
                numeros::escrever_quantidade(atual),
                numeros::escrever_quantidade(contado),
                numeros::escrever_quantidade(diferenca)
            ),
            format!("Motivo: {motivo}"),
        ],
    )?;
    Ok(contado)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::{produtos, usuarios};

    fn campo_do_erro(e: Erro) -> (String, String) {
        match e {
            Erro::Validacao { campo, mensagem } => (campo.unwrap_or_default(), mensagem),
            outro => panic!("{outro:?}"),
        }
    }

    /// NEW-B8B2-01: unidade inteira não aceita estoque quebrado (no ajuste,
    /// no estoque inicial e no mínimo); a fracionada aceita.
    #[test]
    fn unidade_inteira_sem_estoque_quebrado() {
        let b = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap().id;
        let mut f = produtos::ficha_nova(&b.conn).unwrap();
        f.nome = "Parafuso".into();
        f.custo = "1,00".into();
        f.unidade = "UN".into();
        f.estoque_inicial = "2,5".into();
        let (campo, msg) = campo_do_erro(produtos::salvar(&b.conn, &b.dispositivo, &u, &f).unwrap_err());
        assert_eq!((campo.as_str(), msg.as_str()), ("estoqueInicial", "UN é contado inteiro (sem vírgula)."));
        f.estoque_inicial = "2".into();
        f.estoque_minimo = "0,5".into();
        let (campo, _) = campo_do_erro(produtos::salvar(&b.conn, &b.dispositivo, &u, &f).unwrap_err());
        assert_eq!(campo, "estoqueMinimo");
        f.estoque_minimo = "1".into();
        let parafuso = produtos::salvar(&b.conn, &b.dispositivo, &u, &f).unwrap();
        let (campo, msg) =
            campo_do_erro(ajustar(&b.conn, &b.dispositivo, &u, &parafuso, "2,5", "contagem").unwrap_err());
        assert_eq!((campo.as_str(), msg.as_str()), ("quantidade", "UN é contado inteiro (sem vírgula)."));
        assert_eq!(saldo(&b.conn, &parafuso).unwrap(), 2_000, "nada lançado");
        assert_eq!(ajustar(&b.conn, &b.dispositivo, &u, &parafuso, "3", "contagem").unwrap(), 3_000);

        let mut g = produtos::ficha_nova(&b.conn).unwrap();
        g.nome = "Cabo".into();
        g.custo = "1,00".into();
        g.unidade = "MT".into();
        g.estoque_inicial = "2,5".into();
        g.estoque_minimo = "0,5".into();
        let cabo = produtos::salvar(&b.conn, &b.dispositivo, &u, &g).unwrap();
        assert_eq!(ajustar(&b.conn, &b.dispositivo, &u, &cabo, "1,25", "contagem").unwrap(), 1_250);
    }
}
