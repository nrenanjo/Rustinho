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

/// Lança um movimento de estoque (positivo entra, negativo sai). Serviço não
/// tem estoque: para ele, não lança nada.
pub fn lancar(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: Option<&str>,
    produto_id: &str,
    quantidade: i64,
    tipo: &str,
    motivo: &str,
) -> Resultado<()> {
    conn.execute(
        "INSERT INTO estoque_movimento (id, produto_id, quantidade, tipo, motivo, usuario_id, dispositivo, criado_em)
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8
          WHERE NOT EXISTS (SELECT 1 FROM produto WHERE id = ?2 AND tipo = 'servico')",
        params![tempo::novo_id(), produto_id, quantidade, tipo, motivo, usuario_id, dispositivo, tempo::agora()],
    )?;
    Ok(())
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
