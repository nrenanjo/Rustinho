//! Histórico de quem fez o quê (auditoria).

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::erro::Resultado;
use crate::tempo;

/// Registra uma ação. `detalhes` são frases curtas, como
/// "Preço Revenda: 85,00 → 89,90".
pub fn registrar(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: Option<&str>,
    acao: &str,
    entidade: &str,
    entidade_id: &str,
    detalhes: &[String],
) -> Resultado<()> {
    conn.execute(
        "INSERT INTO auditoria (id, quando, usuario_id, dispositivo, acao, entidade, entidade_id, detalhes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            tempo::novo_id(),
            tempo::agora(),
            usuario_id,
            dispositivo,
            acao,
            entidade,
            entidade_id,
            serde_json::to_string(detalhes).unwrap_or_default(),
        ],
    )?;
    Ok(())
}

/// Um item do histórico, pronto para mostrar.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Registro {
    pub quando: String,
    pub usuario: String,
    pub acao: String,
    /// Frase curta: "cadastrou a peça", "alterou"...
    pub descricao: String,
    pub detalhes: Vec<String>,
}

/// Últimos registros de uma entidade (uma peça, um usuário...).
pub fn historico(conn: &Connection, entidade: &str, entidade_id: &str, limite: i64) -> Resultado<Vec<Registro>> {
    let mut consulta = conn.prepare_cached(
        "SELECT a.quando, COALESCE(u.nome, ''), a.acao, a.detalhes
           FROM auditoria a LEFT JOIN usuario u ON u.id = a.usuario_id
          WHERE a.entidade = ?1 AND a.entidade_id = ?2
          ORDER BY a.quando DESC, a.id DESC
          LIMIT ?3",
    )?;
    let linhas = consulta.query_map(params![entidade, entidade_id, limite], |l| {
        let acao: String = l.get(2)?;
        let detalhes: String = l.get(3)?;
        Ok(Registro {
            quando: l.get(0)?,
            usuario: l.get(1)?,
            descricao: descrever(&acao).to_string(),
            acao,
            detalhes: serde_json::from_str(&detalhes).unwrap_or_default(),
        })
    })?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

/// Um item da auditoria geral (Configurações › Simultaneidade › Histórico).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistroGeral {
    /// "27/10/2026 08:18" no horário daqui.
    pub quando: String,
    pub usuario: String,
    pub dispositivo: String,
    pub acao: String,
    pub descricao: String,
    pub detalhes: Vec<String>,
}

/// Os últimos registros da loja toda (de todos os caixas que já trocaram
/// dados com este), do mais novo para o mais velho.
pub fn recentes(conn: &Connection, limite: i64) -> Resultado<Vec<RegistroGeral>> {
    let mut consulta = conn.prepare_cached(
        "SELECT COALESCE(strftime('%d/%m/%Y %H:%M', a.quando, 'localtime'), a.quando),
                COALESCE(u.nome, ''), a.dispositivo, a.acao, a.detalhes
           FROM auditoria a LEFT JOIN usuario u ON u.id = a.usuario_id
          ORDER BY a.quando DESC, a.id DESC
          LIMIT ?1",
    )?;
    let linhas = consulta.query_map(params![limite], |l| {
        let acao: String = l.get(3)?;
        let detalhes: String = l.get(4)?;
        Ok(RegistroGeral {
            quando: l.get(0)?,
            usuario: l.get(1)?,
            dispositivo: l.get(2)?,
            descricao: descrever(&acao).to_string(),
            acao,
            detalhes: serde_json::from_str(&detalhes).unwrap_or_default(),
        })
    })?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

fn descrever(acao: &str) -> &'static str {
    match acao {
        "produto.criar" => "cadastrou a peça",
        "produto.alterar" => "alterou",
        "estoque.ajustar" => "ajustou o estoque",
        "usuario.criar" => "criou o usuário",
        "pessoa.criar" => "cadastrou",
        "pessoa.alterar" => "alterou",
        "compra.importar" => "importou o XML",
        "compra.entrada" => "deu entrada pela compra",
        "compra.lancar" => "lançou no estoque",
        "compra.estornar" => "estornou",
        "compra.descartar" => "descartou o rascunho da compra",
        "fiscal.regras" => "mudou as regras de ST",
        "usuario.alterar" => "alterou o usuário",
        "usuario.senha_fora_da_politica" => "entrou com senha fora da política",
        "receber.lancar" => "lançou dívida",
        "receber.receber" => "recebeu",
        "receber.estornar" => "estornou um pagamento",
        "receber.cancelar" => "tirou uma dívida",
        "venda.cancelar" => "cancelou a venda",
        "pagar.lancar" => "lançou conta a pagar",
        "pagar.alterar" => "corrigiu a conta",
        "pagar.cancelar" => "tirou a conta",
        "pagar.pagar" => "pagou",
        "pagar.estornar" => "estornou um pagamento",
        "financeiro.conta" => "salvou a conta da loja",
        "financeiro.movimento" => "lançou movimento",
        "financeiro.transferir" => "transferiu",
        "financeiro.conferir" => "conferiu o saldo",
        "financeiro.estornar" => "estornou movimento",
        "cheque.depositar" => "depositou o cheque",
        "cheque.repassar" => "repassou o cheque",
        "cheque.devolver" => "registrou cheque devolvido",
        "cheque.desfazer" => "voltou o cheque para a carteira",
        "comissao.fechar" => "fechou a comissão",
        "comissao.reabrir" => "desfez o fechamento da comissão",
        "encomenda.criar" => "anotou a encomenda",
        "encomenda.pedir" => "pediu a encomenda",
        "encomenda.chegou" => "marcou que a encomenda chegou",
        "encomenda.entregar" => "entregou a encomenda",
        "encomenda.cancelar" => "cancelou a encomenda",
        "encomenda.voltar" => "voltou a encomenda para falta pedir",
        "encomenda.chegou_compra" => "recebeu a encomenda pela compra",
        "encomenda.voltou_estorno" => "voltou a encomenda a esperar (compra estornada)",
        "estoque.dias" => "mudou os dias da sugestão de compra",
        "backup.restaurar" => "restaurou um backup",
        "caixa.abrir" => "abriu o caixa",
        "caixa.fechar" => "fechou o caixa",
        "caixa.caixa_fechado" => "lançou no Caixa geral (caixa fechado)",
        "caixa.retirar" => "retirou um caixa da loja",
        "caixa.readmitir" => "readmitiu um caixa na loja",
        "caixa.sangria" => "fez uma sangria",
        "caixa.suprimento" => "fez um suprimento",
        "categoria.criar" => "criou a categoria",
        "categoria.alterar" => "alterou a categoria",
        "codigos.assumir" => "passou a distribuir as faixas de códigos",
        "codigos.caixa_novo" => "registrou este banco como caixa novo",
        "codigos.computador" => "confirmou o computador deste caixa",
        "codigos.definir" => "deu código aos cadastros a definir",
        "compra.autorizacao" => "conferiu a nota no portal da NF-e",
        "conflito.resolver" => "resolveu uma diferença entre caixas",
        "conflito.manter_todos" => "deixou as diferenças entre caixas como estão",
        "dre.classificacao" => "classificou as categorias do resultado",
        "dre.meta" => "mudou uma meta do resultado",
        "gerente.liberar" => "digitou a senha do gerente",
        "gerente.senha_errada" => "errou a senha do gerente",
        "gerente.salvar" => "mudou quem é gerente ou o que só o gerente pode",
        "os.criar" => "abriu a OS",
        "os.situacao" => "mudou a etapa da OS",
        "os.entregar" => "entregou a OS",
        "os.cancelar" => "cancelou a OS",
        "os.rascunho" => "salvou um rascunho de OS",
        "os.rascunho_excluir" => "excluiu um rascunho de OS",
        "os.alterar" => "alterou a OS",
        "pendencia.arquivar" => "arquivou uma pendência da troca de dados",
        "pessoa.codigo" => "deu código novo ao cadastro",
        "precos.regras" => "mudou a regra de preço",
        "produto.codigo" => "deu código novo à peça",
        "produto.defeito" => "registrou peça com defeito",
        "produto.importar" => "importou cadastros",
        "produto.gtin_importado" => "gravou GTIN do XML na peça",
        "unidade.criar" => "cadastrou a unidade",
        "referencia.aceitar" => "aceitou um registro incompleto como está",
        "venda.devolucao" => "registrou troca ou devolução",
        "venda.devolucao_desfeita" => "desfez a troca ou devolução",
        "entrar" => "entrou",
        "sair" => "saiu",
        _ => "registrou",
    }
}

#[cfg(test)]
mod testes {
    use super::descrever;

    /// As ações que o código grava a partir de uma variável (não dá para ler do texto).
    const PELA_VARIAVEL: &[&str] = &["caixa.sangria", "caixa.suprimento"];

    /// O 4º argumento de cada `auditoria::registrar(` no código do núcleo: os
    /// textos "dominio.acao" que aparecem nele.
    fn acoes_gravadas() -> Vec<(String, String)> {
        fn ler(dir: &std::path::Path, saida: &mut Vec<(String, String)>) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    ler(&p, saida);
                    continue;
                }
                if p.extension().and_then(|x| x.to_str()) != Some("rs") {
                    continue;
                }
                let t = std::fs::read_to_string(&p).unwrap();
                for (i, _) in t.match_indices("auditoria::registrar(") {
                    let resto = &t[i + "auditoria::registrar(".len()..];
                    let (mut nivel, mut virgulas, mut inicio) = (0i32, 0, 0);
                    let mut arg = "";
                    for (j, c) in resto.char_indices() {
                        match c {
                            '(' | '[' | '{' => nivel += 1,
                            ')' | ']' | '}' if nivel == 0 => break,
                            ')' | ']' | '}' => nivel -= 1,
                            ',' if nivel == 0 => {
                                virgulas += 1;
                                if virgulas == 3 {
                                    inicio = j + 1;
                                } else if virgulas == 4 {
                                    arg = &resto[inicio..j];
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    for parte in arg.split('"').skip(1).step_by(2) {
                        if parte.contains('.') && parte.chars().all(|c| c.is_ascii_lowercase() || c == '.' || c == '_')
                        {
                            saida.push((parte.to_string(), p.display().to_string()));
                        }
                    }
                }
            }
        }
        let mut v = Vec::new();
        ler(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut v);
        v
    }

    /// Toda ação gravada tem uma frase no Histórico (sem cair no "registrou").
    #[test]
    fn toda_acao_gravada_tem_descricao_no_historico() {
        let acoes = acoes_gravadas();
        assert!(acoes.len() > 60, "a leitura do código achou poucas ações: {}", acoes.len());
        let mut faltam: Vec<String> = acoes
            .iter()
            .map(|(a, _)| a.as_str())
            .chain(PELA_VARIAVEL.iter().copied())
            .filter(|a| descrever(a) == descrever("?"))
            .map(|a| a.to_string())
            .collect();
        faltam.sort();
        faltam.dedup();
        assert!(faltam.is_empty(), "ações sem frase no Histórico: {faltam:?}");
    }
}
