//! R0 da recuperação do trabalho (parecer da 0.9.19 §3; QT-1, preparado em 14/11/2026): a operação já
//! feita fica no banco, na mesma transação da própria operação.
//!
//! Antes, a lista das chaves já usadas ficava só na memória do serviço: um reinício (no Android, o Qt 6.8
//! reinicia o processo numa recriação da atividade) a apagava. Agora:
//! - a mesma chave com o mesmo pedido devolve o **resultado da primeira vez** (DR-7 (b));
//! - a mesma chave com outro pedido é **recusada** (o engano da tela aparece, em vez de ser engolido);
//! - sem linha (a operação não chegou ao `COMMIT`), a operação é feita normalmente.
//!
//! A linha entra na transação do atendente do comando (`despacho::marcar_feita`): se a transação é
//! desfeita, a marca também é. A unicidade da chave primária arbitra duas tentativas ao mesmo tempo.
//! Este módulo é o único que grava na `operacao_feita` (dono, veja `esquema`).

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::erro::Resultado;

/// Por quantos dias a chave de uma operação feita fica guardada (DR-5: o mesmo prazo do trabalho
/// guardado; a limpeza é na abertura do banco, nunca no meio de uma operação).
pub const RETENCAO_DIAS: i64 = 30;

/// Uma operação de `NAO_REPETIR` identificada pela chave que a tela mandou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operacao {
    /// SHA-256 de (comando, chave da operação, usuário): o mesmo cálculo de antes na memória.
    pub chave: [u8; 32],
    pub comando: String,
    pub usuario_id: String,
    /// SHA-256 dos dados do comando em forma canônica, sem a chave e sem senhas.
    pub pedido: [u8; 32],
    /// QT-1b: a abertura do programa em que a operação acontece (0 = nenhuma: testes, banco em memória).
    pub abertura: i64,
    /// QT-1b: o que identifica a operação no aviso quando a resposta não diz (tipo e valor: "Sangria · R$ 5,00").
    pub resumo: String,
    /// F-09 (revisão): o pedido tem um campo de senha preenchido. A senha não entra no resumo do pedido; por isso a
    /// mesma chave com um pedido assim nunca é tomada como "a mesma operação" (seria "ok" sem trocar a senha).
    pub tem_senha: bool,
}

/// O que já existe no banco para a chave.
#[derive(Debug, PartialEq)]
pub enum Anterior {
    /// Nenhuma operação confirmada com essa chave: fazer.
    Nenhuma,
    /// A mesma operação já foi feita: o resultado da primeira vez.
    Mesma(Value),
    /// A chave já foi usada para outro pedido: recusar.
    Outra,
}

/// A operação a partir do comando, da chave mandada pela tela, do usuário e dos dados.
pub fn identificar(comando: &str, chave_da_tela: &str, usuario_id: &str, dados: &Value) -> Operacao {
    let mut h = Sha256::new();
    h.update(comando.as_bytes());
    h.update([0u8]);
    h.update(chave_da_tela.as_bytes());
    h.update([0u8]);
    h.update(usuario_id.as_bytes());
    let chave: [u8; 32] = h.finalize().into();
    Operacao {
        chave,
        comando: comando.to_string(),
        usuario_id: usuario_id.to_string(),
        pedido: pedido(dados),
        abertura: 0,
        resumo: resumo(comando, dados),
        tem_senha: tem_senha(dados),
    }
}

/// O pedido tem algum campo de senha preenchido?
fn tem_senha(dados: &Value) -> bool {
    match dados {
        Value::Object(m) => m.iter().any(|(k, v)| {
            (k.to_lowercase().contains("senha") && v.as_str().is_some_and(|s| !s.is_empty())) || tem_senha(v)
        }),
        Value::Array(a) => a.iter().any(tem_senha),
        _ => false,
    }
}

/// F-12 (revisão): um resumo curto do pedido para o aviso, quando a resposta da operação não identifica nada
/// (movimento do caixa, cheques, transferência…): o tipo e o valor, ou a descrição. Nunca senha.
pub fn resumo(comando: &str, dados: &Value) -> String {
    let texto = |k: &str| dados[k].as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let mut partes = Vec::new();
    if let Some(tipo) = texto("tipo").filter(|_| comando == "caixa_movimentar" || comando == "movimento_lancar") {
        let mut c = tipo.chars();
        partes.push(c.next().map(|p| p.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default());
    }
    if let Some(d) = texto("descricao").or_else(|| texto("motivo").filter(|_| comando != "caixa_movimentar")) {
        partes.push(d.chars().take(40).collect());
    }
    if let Some(v) = texto("valor") {
        let v = crate::numeros::ler_dinheiro(&v).ok().flatten().map(crate::numeros::escrever_dinheiro).unwrap_or(v);
        partes.push(format!("R$ {v}"));
    }
    partes.join(" · ")
}

/// SHA-256 dos dados em forma canônica (o `serde_json` deste projeto ordena as chaves dos objetos), sem as
/// chaves de identificação (`chaveOperacao`, `chaveTela`) e sem nenhum campo de senha (a senha do gerente
/// digitada para a operação não fica no banco, nem como resumo).
pub fn pedido(dados: &Value) -> [u8; 32] {
    fn limpar(v: &Value) -> Value {
        match v {
            Value::Object(m) => Value::Object(
                m.iter()
                    .filter(|(k, _)| {
                        let k = k.to_lowercase();
                        k != "chaveoperacao" && k != "chavetela" && !k.contains("senha")
                    })
                    .map(|(k, v)| (k.clone(), limpar(v)))
                    .collect(),
            ),
            Value::Array(a) => Value::Array(a.iter().map(limpar).collect()),
            outro => outro.clone(),
        }
    }
    Sha256::digest(limpar(dados).to_string().as_bytes()).into()
}

/// O que já existe para a operação.
pub fn consultar(conn: &Connection, op: &Operacao) -> Resultado<Anterior> {
    let linha: Option<(Vec<u8>, String)> = conn
        .query_row("SELECT pedido, resultado FROM operacao_feita WHERE chave = ?1", [&op.chave[..]], |l| {
            Ok((l.get(0)?, l.get(1)?))
        })
        .optional()?;
    Ok(match linha {
        None => Anterior::Nenhuma,
        Some((pedido, _)) if pedido != op.pedido => Anterior::Outra,
        Some((_, resultado)) => Anterior::Mesma(serde_json::from_str(&resultado)?),
    })
}

/// Grava a operação como feita. Chamar DENTRO da transação da operação, antes do `commit`: se ela for
/// desfeita, a marca também é. Uma segunda tentativa com a mesma chave bate na chave primária e a
/// transação inteira dela é desfeita.
pub fn marcar(tx: &Connection, op: &Operacao, resultado: &Value) -> Resultado<()> {
    tx.execute(
        "INSERT INTO operacao_feita (chave, comando, usuario_id, pedido, resultado, feita_em, feita_ms, abertura, resumo)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            &op.chave[..],
            op.comando,
            op.usuario_id,
            &op.pedido[..],
            resultado.to_string(),
            crate::tempo::agora(),
            agora_ms(),
            op.abertura,
            op.resumo
        ],
    )?;
    // QT-1b: a operação deixou de estar "em curso" NA MESMA transação em que foi gravada.
    tx.execute("DELETE FROM operacao_em_curso WHERE chave = ?1", [&op.chave[..]])?;
    Ok(())
}

// ------------------------------------------------------------- QT-1b: o aviso de fechamento inesperado

/// Comandos de várias transações (um pacote de notas grava nota por nota): se o programa caiu no meio, pode ter
/// gravado parte. No aviso, ficam como "confira", nunca como "não concluída". A lista é conferida contra o código
/// dos atendentes (teste `rev_f11_varias_transacoes_bate_com_o_codigo`): a importação de cadastros é UMA transação.
pub const VARIAS_TRANSACOES: &[&str] = &["compra_importar"];

/// Milissegundos do relógio: só a ordem e a hora no aviso. Quem decide o que entra no aviso é o número da abertura.
fn agora_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// A operação vai começar: fica registrada como "em curso" (transação própria, antes do comando). Sai na
/// mesma transação que a grava (`marcar`) ou, se o comando falhar de forma limpa, por `desistir`.
pub fn iniciar(conn: &Connection, op: &Operacao) -> Resultado<()> {
    conn.execute(
        "INSERT OR REPLACE INTO operacao_em_curso (chave, comando, usuario_id, iniciada_em, iniciada_ms, abertura, resumo)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![&op.chave[..], op.comando, op.usuario_id, crate::tempo::agora(), agora_ms(), op.abertura, op.resumo],
    )?;
    Ok(())
}

/// O comando devolveu um erro (a transação dele foi desfeita): não fica "em curso". Na importação de compras, as
/// notas gravadas antes do erro ficam (cada uma na sua transação); a tela mostrou o erro e a lista.
pub fn desistir(conn: &Connection, op: &Operacao) -> Resultado<()> {
    conn.execute("DELETE FROM operacao_em_curso WHERE chave = ?1", [&op.chave[..]])?;
    Ok(())
}

/// Abertura do programa: ganha o próximo número (1, 2, 3…). As aberturas anteriores que não fecharam pelo caminho
/// normal (ou que tiveram erro interno) continuam guardadas até cada pessoa conferir: duas quedas seguidas não
/// apagam a primeira (F-01), e o relógio não decide nada (F-03). Devolve o número desta abertura.
pub fn abrir_sessao(conn: &Connection) -> Resultado<i64> {
    conn.execute(
        "INSERT INTO programa_abertura (numero, aberta_em, aberta_ms)
         VALUES ((SELECT COALESCE(MAX(numero), 0) + 1 FROM programa_abertura), ?1, ?2)",
        params![crate::tempo::agora(), agora_ms()],
    )?;
    Ok(conn.query_row("SELECT MAX(numero) FROM programa_abertura", [], |l| l.get(0))?)
}

/// O número da abertura que está correndo (a maior; 0 se nenhuma).
pub fn abertura_atual(conn: &Connection) -> Resultado<i64> {
    Ok(conn.query_row("SELECT COALESCE(MAX(numero), 0) FROM programa_abertura", [], |l| l.get(0))?)
}

/// Encerramento limpo (o fechar da janela, o fim do serviço). Uma abertura com erro interno não vira limpa.
pub fn fechar_sessao(conn: &Connection, abertura: i64) -> Resultado<()> {
    conn.execute("UPDATE programa_abertura SET limpa = 1 WHERE numero = ?1 AND com_erro = 0", [abertura])?;
    Ok(())
}

/// F-04 (revisão): houve um erro interno (pânico) nesta abertura. A transação foi desfeita; a operação fica "em
/// curso" (não concluída) e a abertura seguinte avisa, mesmo que esta feche pela janela.
pub fn marcar_erro(conn: &Connection, abertura: i64) -> Resultado<()> {
    conn.execute("UPDATE programa_abertura SET com_erro = 1 WHERE numero = ?1", [abertura])?;
    Ok(())
}

/// F-02 (revisão): a tela recebeu a resposta destas operações (houve outra chamada depois delas: as chamadas são
/// uma de cada vez). O que a tela já mostrou não é motivo de aviso depois de uma queda.
pub fn entregar(conn: &Connection, chaves: &[[u8; 32]]) -> Resultado<()> {
    let mut s = conn.prepare_cached("UPDATE operacao_feita SET entregue = 1 WHERE chave = ?1 AND entregue = 0")?;
    for c in chaves {
        s.execute([&c[..]])?;
    }
    Ok(())
}

/// Uma linha do aviso.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Linha {
    /// "gravada", "nao_concluida" ou "confira".
    pub classe: &'static str,
    pub comando: String,
    pub quando: String,
    /// Só nas gravadas: a resposta da primeira vez (número, total…).
    pub resultado: Option<Value>,
    /// O tipo e o valor do pedido (quando a resposta não identifica a operação).
    pub resumo: String,
}

/// Quando a queda de uma abertura foi vista: quando começou a abertura seguinte (N-04 da re-revisão). Sem a
/// seguinte, quando ela mesma começou.
const VISTA_EM: &str = "COALESCE((SELECT b.aberta_em FROM programa_abertura b WHERE b.numero > a.numero
                                  ORDER BY b.numero LIMIT 1), a.aberta_em)";

/// As aberturas que caíram (não fecharam direito ou tiveram erro interno), anteriores a `atual`, que esta pessoa
/// ainda não conferiu, vistas nos últimos `RETENCAO_DIAS` dias: (número, quando abriu). N-04 da re-revisão: o prazo
/// conta de quando a queda foi vista, e não de quando a abertura começou (o programa aberto por mais de um mês, ou
/// um relógio atrasado na hora de abrir, não podem esconder uma queda de hoje).
fn caidas(conn: &Connection, atual: i64, usuario_id: &str) -> Resultado<Vec<(i64, String)>> {
    let mut s = conn.prepare(&format!(
        "SELECT numero, aberta_em FROM programa_abertura a
          WHERE numero < ?1 AND (limpa = 0 OR com_erro = 1) AND {VISTA_EM} >= ?3
            AND NOT EXISTS (SELECT 1 FROM fechamento_visto v WHERE v.usuario_id = ?2 AND v.abertura = a.numero)
          ORDER BY numero"
    ))?;
    let lista = s
        .query_map(params![atual, usuario_id, crate::tempo::dias_atras(RETENCAO_DIAS)], |l| Ok((l.get(0)?, l.get(1)?)))?
        .collect::<Result<Vec<(i64, String)>, _>>()?;
    Ok(lista)
}

/// O que ficou sem confirmação nas aberturas que caíram, para esta pessoa (o gerente vê de todos): as operações
/// gravadas que a tela não chegou a mostrar (F-02), as que começaram e não terminaram, e as de várias transações
/// ("confira"). `None` quando não há nada a mostrar. Devolve também quando abriu a abertura mais antiga e o número
/// da mais nova (o id do aviso).
pub fn fechamento(
    conn: &Connection,
    usuario_id: &str,
    gerente: bool,
    atual: i64,
) -> Resultado<Option<(String, i64, Vec<Linha>)>> {
    let lista = caidas(conn, atual, usuario_id)?;
    if lista.is_empty() {
        return Ok(None);
    }
    let numeros = lista.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>().join(",");
    let quem = if gerente { "" } else { usuario_id };
    let mut linhas = Vec::new();
    // As aberturas que têm linha para esta pessoa (N-03 da re-revisão): só elas decidem o "Desde" e o id.
    let mut com_linha = std::collections::BTreeSet::new();
    // TODAS as gravadas que a tela não mostrou, na ordem (N-02 da re-revisão: cortar justamente o que o aviso deve
    // mostrar escondia vendas sem confirmação, e o "Conferi" as dava por vistas). São poucas por abertura.
    let mut s = conn.prepare(&format!(
        "SELECT comando, feita_em, resultado, resumo, abertura FROM operacao_feita
          WHERE abertura IN ({numeros}) AND entregue = 0 AND (?1 = '' OR usuario_id = ?1)
          ORDER BY abertura, feita_ms"
    ))?;
    for l in s.query_map(params![quem], |l| {
        Ok((
            l.get::<_, String>(0)?,
            l.get::<_, String>(1)?,
            l.get::<_, String>(2)?,
            l.get::<_, String>(3)?,
            l.get::<_, i64>(4)?,
        ))
    })? {
        let (comando, quando, resultado, resumo, abertura) = l?;
        com_linha.insert(abertura);
        linhas.push(Linha {
            classe: "gravada",
            comando,
            quando,
            resultado: serde_json::from_str(&resultado).ok(),
            resumo,
        });
    }
    let mut s = conn.prepare(&format!(
        "SELECT comando, iniciada_em, resumo, abertura FROM operacao_em_curso
          WHERE abertura IN ({numeros}) AND (?1 = '' OR usuario_id = ?1) ORDER BY abertura, iniciada_ms"
    ))?;
    for l in s.query_map(params![quem], |l| {
        Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?, l.get::<_, String>(2)?, l.get::<_, i64>(3)?))
    })? {
        let (comando, quando, resumo, abertura) = l?;
        com_linha.insert(abertura);
        let classe = if VARIAS_TRANSACOES.contains(&comando.as_str()) { "confira" } else { "nao_concluida" };
        linhas.push(Linha { classe, comando, quando, resultado: None, resumo });
    }
    // N-03: a queda sem nada a mostrar (no Android, toda morte em segundo plano) não muda o "Desde" nem o id (o
    // minimizar da pessoa continua valendo).
    let (Some(&primeira), Some(&ultima)) = (com_linha.first(), com_linha.last()) else {
        return Ok(None);
    };
    let desde = lista.iter().find(|(n, _)| *n == primeira).map(|(_, q)| q.clone()).unwrap_or_default();
    Ok(Some((desde, ultima, linhas)))
}

/// Esta pessoa conferiu o aviso: as aberturas que caíram e que ela via somem PARA ELA. Nada é apagado (F-10: o
/// "Conferi" do caixa não tira do gerente a operação interrompida do caixa); a limpeza é por prazo.
pub fn fechamento_visto(conn: &Connection, usuario_id: &str, atual: i64) -> Resultado<()> {
    for (numero, _) in caidas(conn, atual, usuario_id)? {
        conn.execute(
            "INSERT OR IGNORE INTO fechamento_visto (usuario_id, abertura) VALUES (?1, ?2)",
            params![usuario_id, numero],
        )?;
    }
    Ok(())
}

/// Depois de restaurar um backup (na transação que termina a restauração, também a que termina ao abrir depois de
/// uma queda): as aberturas que vieram na cópia (e as operações em curso da hora do backup) não valem para este
/// banco: todas viram encerradas e as em curso saem. Sem isso, virariam um aviso falso de fechamento inesperado.
/// As operações gravadas que vieram no backup ficam (o R0 delas continua valendo). Quem restaurou abre uma
/// abertura nova em seguida (`abrir_sessao`).
pub fn depois_de_restaurar(conn: &Connection) -> Resultado<()> {
    conn.execute("DELETE FROM operacao_em_curso", [])?;
    conn.execute("UPDATE programa_abertura SET limpa = 1, com_erro = 0", [])?;
    Ok(())
}

/// A operação já está marcada?
pub fn marcada(conn: &Connection, op: &Operacao) -> Resultado<bool> {
    Ok(conn
        .query_row("SELECT 1 FROM operacao_feita WHERE chave = ?1", [&op.chave[..]], |_| Ok(()))
        .optional()?
        .is_some())
}

/// Tira as chaves com mais de `dias` dias, e o que do aviso ficou velho (O-04 da revisão: as operações em curso,
/// as aberturas e as conferências). Só na abertura do banco.
pub fn limpar_antigas(conn: &Connection, dias: i64) -> Resultado<usize> {
    let limite = crate::tempo::dias_atras(dias);
    // N-04 da re-revisão: a abertura vence `dias` depois de a queda ser vista (a seguinte começou), e não depois de
    // ela mesma começar; a atual nunca.
    conn.execute(
        &format!(
            "DELETE FROM programa_abertura AS a WHERE a.numero < (SELECT MAX(numero) FROM programa_abertura)
               AND {VISTA_EM} < ?1"
        ),
        [&limite],
    )?;
    // As chaves velhas saem, menos as gravadas sem entrega de uma abertura ainda no prazo (são linhas do aviso).
    let n = conn.execute(
        "DELETE FROM operacao_feita WHERE feita_em < ?1
           AND (entregue = 1 OR abertura NOT IN (SELECT numero FROM programa_abertura))",
        [&limite],
    )?;
    conn.execute(
        "DELETE FROM operacao_em_curso WHERE iniciada_em < ?1 AND abertura NOT IN (SELECT numero FROM programa_abertura)",
        [&limite],
    )?;
    conn.execute("DELETE FROM fechamento_visto WHERE abertura NOT IN (SELECT numero FROM programa_abertura)", [])?;
    Ok(n)
}
