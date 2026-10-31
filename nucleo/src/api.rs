//! Porta de entrada do núcleo: a interface manda um comando com dados em
//! JSON e recebe a resposta em JSON.
//!
//! Resposta de sucesso: `{"ok": true, "dados": ...}`
//! Resposta de erro:    `{"ok": false, "erro": "mensagem", "campo": "nome"}`

use std::path::Path;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::banco::Banco;
use crate::erro::{Erro, Resultado};
use crate::{
    arquivos, auditoria, backup, busca, caixa, cheques, codigo, comissoes, compras, config, devolucoes, documento, dre,
    encomendas, estoque, etiquetas, faixas, financeiro, fiscal, importar, impressao, menu, numeros, oficina, pagar,
    painel, pessoas, produtos, receber, reposicao, requisitos, sync, texto, unidades, usuarios, vendas,
};

/// Versão do Rustinho.
pub const VERSAO: &str = env!("CARGO_PKG_VERSION");

const TENTATIVAS_MAXIMAS: u32 = 5;
/// Segundos de espera depois de muitas senhas erradas seguidas.
const ESPERA_APOS_ERROS: i64 = 30;
/// Quanto tempo vale a senha do gerente digitada no "aperto": a consulta de
/// uma tela do gerente renova a cada leitura; a de uma gravação acaba depois
/// de poucos minutos (ou antes, quando a gravação dá certo).
const LIBERACAO: Duration = Duration::from_secs(15 * 60);
const LIBERACAO_CURTA: Duration = Duration::from_secs(3 * 60);

/// O que pode ficar só para o gerente. Na pílula Gerente das Configurações,
/// cada uma tem uma marca de sim/não (de fábrica, todas ligadas).
pub const ACOES_DO_GERENTE: &[(&str, &str)] = &[
    ("cancelar_venda", "Cancelar venda"),
    ("estornar", "Estornar recebimentos, pagamentos, compras e trocas"),
    ("ajustar_estoque", "Ajustar o estoque"),
    ("backup", "Backup e restaurar"),
    ("configuracoes", "Configurações da loja"),
    ("usuarios", "Usuários"),
    ("financeiro", "Financeiro: contas a pagar, contas da loja, cheques e comissões"),
    ("resultados", "Resultados: lucro, custos, despesas, DRE gerencial e metas"),
];

/// Nunca ficam livres para todos (a pílula Gerente não tem marca para elas).
/// "precos": mudar as regras de formação de preço (D-09).
/// "simultaneidade" (FALHA-05): consultar Configurações › Simultaneidade
/// (Duplicatas, Histórico e Caixas da loja), separada da pílula Gerente.
const SEMPRE_DO_GERENTE: &[&str] = &["gerente", "precos", "simultaneidade"];

/// Nome da ação para as mensagens.
fn nome_da_acao(acao: &str) -> String {
    match acao {
        "gerente" => "a pílula Gerente".into(),
        "simultaneidade" => "a Simultaneidade (duplicatas, histórico e caixas da loja)".into(),
        "precos" => "mudar o preço de venda ou a regra de preço (% de lucro)".into(),
        _ => ACOES_DO_GERENTE
            .iter()
            .find(|(id, _)| *id == acao)
            .map(|(_, n)| n.to_lowercase())
            .unwrap_or_else(|| "isso".into()),
    }
}

/// A ação pedida para consultar existe? (A senha do gerente só vale para
/// uma ação certa; "precos" não tem consulta.)
fn acao_de_consulta(acao: &str) -> Resultado<&'static str> {
    ACOES_DO_GERENTE
        .iter()
        .map(|(id, _)| *id)
        .chain(["gerente", "simultaneidade"])
        .find(|a| *a == acao.trim())
        .ok_or_else(|| Erro::validacao("Diga para que é a senha do gerente."))
}

/// O comando pode pedir a senha do gerente? (os do gerente e salvar a peça,
/// que pede quando muda o preço.)
fn comando_do_gerente(comando: &str) -> bool {
    // produto_salvar e compra_lancar: só pedem o gerente quando mudam o preço
    // de venda de uma peça cadastrada (D-09; 0915-N na compra).
    acao_do_comando(comando).is_some() || comando == "produto_salvar" || comando == "compra_lancar"
}

/// Resumo (SHA-256) de um comando com os seus dados, com as chaves em ordem:
/// a senha do gerente para uma gravação fica presa a ele. Só o resumo fica
/// na memória (os dados podem ter senhas, como a de um usuário novo).
fn resumo_da_chamada(comando: &str, dados: &Value) -> [u8; 32] {
    fn escrever(v: &Value, saida: &mut String) {
        match v {
            Value::Object(m) => {
                let mut chaves: Vec<&String> = m.keys().collect();
                chaves.sort();
                saida.push('{');
                for (i, k) in chaves.into_iter().enumerate() {
                    if i > 0 {
                        saida.push(',');
                    }
                    saida.push_str(&Value::String(k.clone()).to_string());
                    saida.push(':');
                    escrever(&m[k], saida);
                }
                saida.push('}');
            }
            Value::Array(a) => {
                saida.push('[');
                for (i, x) in a.iter().enumerate() {
                    if i > 0 {
                        saida.push(',');
                    }
                    escrever(x, saida);
                }
                saida.push(']');
            }
            outro => saida.push_str(&outro.to_string()),
        }
    }
    use sha2::Digest;
    let mut texto = String::new();
    escrever(dados, &mut texto);
    let mut h = sha2::Sha256::new();
    h.update(comando.as_bytes());
    h.update([0u8]);
    h.update(texto.as_bytes());
    h.finalize().into()
}

/// Qual ação do gerente cada comando precisa.
fn acao_do_comando(comando: &str) -> Option<&'static str> {
    Some(match comando {
        "venda_cancelar" => "cancelar_venda",
        "compra_estornar"
        | "recebimento_estornar"
        | "pagamento_estornar"
        | "movimento_estornar"
        | "devolucao_desfazer"
        | "cheque_desfazer"
        | "conta_cancelar" => "estornar",
        "estoque_ajustar" => "ajustar_estoque",
        "backup_ler"
        | "backup_salvar"
        | "backup_agora"
        | "backup_conectar_drive"
        | "backup_desligar_drive"
        | "backup_nuvem_lista"
        | "backup_conferir"
        | "backup_restaurar" => "backup",
        "config_salvar" | "sync_salvar" | "fiscal_regras_salvar" | "importar_cadastros" => "configuracoes",
        "usuario_salvar" => "usuarios",
        "financeiro_resumo"
        | "pagar_painel"
        | "pagar_ficha"
        | "pagar_previa"
        | "pagar_lancar"
        | "pagar_alterar"
        | "pagar_cancelar"
        | "pagar_pagar"
        | "pagar_formas"
        | "conta_loja_salvar"
        | "conta_loja_extrato"
        | "movimento_lancar"
        | "transferir"
        | "conta_loja_conferir"
        | "categorias"
        | "categoria_salvar"
        | "fluxo_caixa"
        | "cheques_painel"
        | "cheque_depositar"
        | "cheque_repassar"
        | "cheque_devolver"
        | "comissoes"
        | "comissao_fechar"
        | "comissao_reabrir" => "financeiro",
        "dre_apurar"
        | "dre_detalhe"
        | "dre_despesas"
        | "dre_indicadores"
        | "dre_serie"
        | "dre_resumo"
        | "dre_classificacao"
        | "dre_classificacao_salvar"
        | "dre_metas"
        | "dre_meta_salvar" => "resultados",
        // FALHA-05: o que a Simultaneidade só lê tem consulta própria (a senha
        // dela não abre a pílula Gerente). Gravar continua sendo "gerente".
        "conflitos_listar" | "codigos_situacao" | "pendencias_listar" | "troca_historico" | "auditoria_listar" => {
            "simultaneidade"
        }
        // A própria pílula Gerente: sempre só o gerente.
        "gerente_ler"
        | "gerente_salvar"
        | "conflito_resolver"
        | "referencia_aceitar"
        | "conflitos_manter_todos"
        | "codigos_assumir"
        | "codigos_definir_pendentes"
        | "codigos_dar_novo"
        | "codigos_pessoa_dar_novo"
        | "codigos_confirmar_computador"
        | "codigos_registrar_caixa_novo"
        // Pendências da troca de dados e retirada de caixa (0.9.15).
        | "pendencia_arquivar"
        | "pendencias_reaplicar"
        | "caixa_retirar"
        | "caixa_readmitir" => "gerente",
        _ => return None,
    })
}

/// Comandos que criam ou mudam registros e aceitam a chave da operação
/// (`chaveOperacao`): a tela gera uma chave para cada operação (uma venda,
/// um recebimento) e manda junto. A mesma chave chegando de novo (duplo
/// clique, Enter apertado duas vezes, a tela mandando de novo) é recusada em
/// vez de gravar duas vezes.
/// A3: comandos que gravam fora de `NAO_REPETIR` (configurações, códigos,
/// conflitos, avisos, menu, troca de dados...). No modo seguro, recusados.
const GRAVAM_TAMBEM: &[&str] = &[
    "aparencia_salvar",
    "aviso_esconder",
    "aviso_minimizar",
    "backup_conectar_drive",
    "backup_desligar_drive",
    "backup_salvar",
    "codigos_assumir",
    "codigos_confirmar_computador",
    "codigos_dar_novo",
    "codigos_definir_pendentes",
    "codigos_pessoa_dar_novo",
    "codigos_registrar_caixa_novo",
    "compra_atualizar",
    "compra_conferir_autorizacao",
    "compra_descartar",
    "computador_salvar",
    "config_salvar",
    "configurar_inicio",
    "conflito_resolver",
    "conflitos_manter_todos",
    "conta_loja_conferir",
    "dre_classificacao_salvar",
    "dre_meta_salvar",
    "encomenda_whatsapp",
    "estoque_dias",
    "fiscal_automatico",
    "fiscal_regras_salvar",
    "gerente_salvar",
    "menu_fixar",
    "menu_usar",
    "pagar_alterar",
    "pendencias_reaplicar",
    "referencia_aceitar",
    "senha_trocar",
    "sync_desistir_juntar",
    "sync_juntar",
    "sync_salvar",
];

/// A3: no modo seguro só passam consultas, entrar/sair, a senha do gerente,
/// restaurar um backup e a cópia para o suporte.
fn bloqueado_no_modo_seguro(comando: &str) -> bool {
    match comando {
        "backup_restaurar" | "backup_agora" => false,
        c => NAO_REPETIR.contains(&c) || GRAVAM_TAMBEM.contains(&c),
    }
}

/// A3: o motivo, igual em toda gravação recusada.
pub const TEXTO_MODO_SEGURO: &str = "O banco de dados deste caixa está com defeito (a conferência de integridade \
     falhou) e o Rustinho está em modo seguro: dá para consultar, mas nada é gravado — venda, recebimento, \
     estoque, caixa, cadastros e troca de dados ficam parados. Não continue a operar neste computador: o \
     arquivo do banco fica como está; restaure um backup bom em Configurações › Backup ou chame o suporte.";

fn erro_modo_seguro() -> Erro {
    Erro::campo("modoSeguro", TEXTO_MODO_SEGURO)
}

pub const NAO_REPETIR: &[&str] = &[
    "venda_concluir",
    "venda_cancelar",
    "orcamento_salvar",
    "os_salvar",
    "os_rascunho_salvar",
    "os_rascunho_excluir",
    "os_situacao",
    "os_cancelar",
    "devolucao_gravar",
    "devolucao_desfazer",
    "caixa_abrir",
    "caixa_fechar",
    "caixa_movimentar",
    "receber_lancar",
    "receber_baixar",
    "recebimento_estornar",
    "encomenda_criar",
    "encomenda_pedir",
    "encomenda_chegou",
    "encomenda_entregar",
    "encomenda_cancelar",
    "compra_importar",
    "compra_lancar",
    "compra_estornar",
    "pessoa_salvar",
    "produto_salvar",
    "produto_variacao_criar",
    "unidade_salvar",
    "estoque_ajustar",
    "pagar_lancar",
    "pagar_pagar",
    "pagar_cancelar",
    "pagamento_estornar",
    "movimento_lancar",
    "movimento_estornar",
    "transferir",
    "conta_loja_salvar",
    "conta_cancelar",
    "categoria_salvar",
    "cheque_depositar",
    "cheque_repassar",
    "cheque_devolver",
    "cheque_desfazer",
    "comissao_fechar",
    "comissao_reabrir",
    "usuario_salvar",
    "backup_restaurar",
    "importar_cadastros",
    "caixa_retirar",
    "caixa_readmitir",
    "pendencia_arquivar",
];

/// Quantas chaves de operação já feitas ficam guardadas (as mais antigas saem).
const CHAVES_GUARDADAS: usize = 2000;

/// Comandos que funcionam sem ninguém conectado.
pub const SEM_SESSAO: &[&str] = &[
    "estado",
    "configurar_inicio",
    "usuarios_entrada",
    "entrar",
    "sync_juntar",
    "sync_desistir_juntar",
    "sync_ler",
    "aparencia_salvar",
    "codigo_ler_foto",
    "backup_automatico",
    "backup_ao_fechar",
    "sair",
    "numero_formatar",
    "preco_calcular",
];

/// Comandos que só leem (não gravam nada no banco). Depois de um erro
/// interno grave, só estes continuam (veja `Servico::chamar`).
pub const SO_LEITURA: &[&str] = &[
    "estado",
    "usuarios_entrada",
    "sync_ler",
    "codigo_ler_foto",
    "numero_formatar",
    "preco_calcular",
    "permissao",
    "gerente_ler",
    "conflitos_listar",
    "pendencias_listar",
    "troca_historico",
    "auditoria_listar",
    "caixas_conhecidos",
    "os_rascunho_pode",
    "codigos_situacao",
    "menu",
    "produtos_buscar",
    "produto_novo",
    "produto_abrir",
    "produto_por_codigo",
    "produto_sugestoes",
    "unidades_listar",
    "usuarios_listar",
    "pessoas_buscar",
    "pessoa_abrir",
    "pessoa_nova",
    "situacao_cliente",
    "compras_listar",
    "compra_abrir",
    "fiscal_conferir",
    "fiscal_tributacao",
    "venda_calcular",
    "venda_abrir",
    "vendas_listar",
    "orcamento_carregar",
    "formas_pagamento",
    "os_abrir",
    "os_calcular",
    "os_listar",
    "os_placa",
    "os_para_venda",
    "mecanicos",
    "devolucao_venda",
    "devolucao_calcular",
    "devolucoes_listar",
    "vales_abertos",
    "caixa_estado",
    "caixa_resumo",
    "avisos_do_dia",
    "receber_painel",
    "receber_cliente",
    "receber_calcular",
    "receber_previa",
    "painel_resumo",
    "dre_apurar",
    "dre_detalhe",
    "dre_despesas",
    "dre_indicadores",
    "dre_serie",
    "dre_resumo",
    "dre_classificacao",
    "dre_metas",
    "painel_clientes",
    "painel_fornecedores",
    "painel_pagamentos",
    "estoque_mais_vendidos",
    "estoque_parados",
    "estoque_sugestao",
    "etiqueta_previa",
    "etiquetas_da_compra",
    "etiquetas_opcoes",
    "encomendas_painel",
    "encomenda_para_venda",
    "financeiro_resumo",
    "pagar_painel",
    "pagar_ficha",
    "pagar_previa",
    "pagar_formas",
    "contas_loja",
    "conta_loja_extrato",
    "categorias",
    "fluxo_caixa",
    "cheques_painel",
    "cheques_previa",
    "comissoes",
    "config_ler",
    "backup_ler",
    "backup_nuvem_lista",
    "backup_conferir",
    "importar_previa",
    "impressao",
    // Navegação guiada: o requisito está satisfeito? os arquivos continuam lá?
    "requisito_conferir",
    "arquivos_conferir",
];

/// Comandos que gravam e que qualquer usuário conectado pode usar (os do
/// gerente estão em `acao_do_comando`). Um comando novo que não esteja em
/// nenhuma lista faz o teste `todo_comando_tem_permissao` falhar.
pub const DO_CAIXA: &[&str] = &[
    // R-04: grava os dias escolhidos na tela de reposição.
    "estoque_dias",
    "gerente_liberar",
    "gerente_encerrar",
    "computador_salvar",
    "menu_usar",
    "menu_fixar",
    "produto_salvar",
    "produto_variacao_criar",
    "unidade_salvar",
    "senha_trocar",
    "pessoa_salvar",
    "compra_importar",
    "compra_atualizar",
    "compra_lancar",
    "compra_descartar",
    // D7: a senha do gerente é pedida e conferida no próprio comando.
    "compra_conferir_autorizacao",
    "fiscal_automatico",
    "venda_concluir",
    "orcamento_salvar",
    "os_salvar",
    "os_rascunho_salvar",
    "os_rascunho_excluir",
    "os_situacao",
    "os_cancelar",
    "devolucao_gravar",
    "caixa_abrir",
    "caixa_fechar",
    "caixa_movimentar",
    "aviso_esconder",
    "aviso_minimizar",
    "receber_lancar",
    "receber_baixar",
    "encomenda_criar",
    "encomenda_pedir",
    "encomenda_chegou",
    "encomenda_entregar",
    "encomenda_cancelar",
    // RVI-01: marca o cliente como avisado quando a encomenda chegou.
    "encomenda_whatsapp",
];

/// Resposta a uma gravação depois de um erro interno grave (pânico).
const TEXTO_COMPROMETIDO: &str =
    "O Rustinho teve um erro interno. Feche e abra de novo para continuar; até lá nada é gravado.";

/// RVI-01: nos testes, toda consulta de `SO_LEITURA` roda com o banco só
/// para leitura, para a suíte acusar um comando de leitura que grave. No
/// programa, isso vale depois de um pânico (veja `Servico::chamar`).
const LEITURA_ESTRITA: bool = cfg!(any(test, feature = "ancoras-de-teste"));

/// O que podemos fazer depois de um erro interno grave: ler, sair e fazer o
/// backup ao fechar.
fn pode_depois_de_erro(comando: &str) -> bool {
    SO_LEITURA.contains(&comando) || matches!(comando, "sair" | "backup_ao_fechar" | "gerente_encerrar")
}

/// Classificação de um comando (para os testes de permissão).
pub fn classe_do_comando(comando: &str) -> Option<&'static str> {
    if let Some(a) = acao_do_comando(comando) {
        return Some(a);
    }
    if SO_LEITURA.contains(&comando) {
        return Some("leitura");
    }
    if DO_CAIXA.contains(&comando) || SEM_SESSAO.contains(&comando) {
        return Some("caixa");
    }
    None
}

/// Quem está usando o Rustinho agora.
#[derive(Debug, Clone)]
struct Sessao {
    usuario_id: String,
    nome: String,
    gerente: bool,
}

/// Senha do gerente digitada na hora, para um caixa fazer algo do gerente.
/// Sempre presa a quem estava usando. A senha mesma não fica em lugar nenhum.
enum Liberacao {
    /// Consulta de uma tela do gerente (Financeiro, Usuários, Resultados...):
    /// só as leituras daquela ação, até sair da tela. Não grava nada.
    Consulta { usuario_id: String, acao: &'static str, ate: Instant },
    /// Uma gravação: este comando com estes dados exatos. Acaba quando dá
    /// certo; se der erro, só repete a mesma coisa.
    Uma { usuario_id: String, comando: String, resumo: [u8; 32], ate: Instant },
}

impl Liberacao {
    fn ate(&self) -> Instant {
        match self {
            Liberacao::Consulta { ate, .. } | Liberacao::Uma { ate, .. } => *ate,
        }
    }
}

/// O serviço que a interface usa. Guarda o banco e a sessão.
pub struct Servico {
    banco: Banco,
    sessao: Option<Sessao>,
    /// Senhas do gerente digitadas no aperto (D-09: cada uma vale só para
    /// o que foi pedido; veja `Liberacao`).
    liberacoes: Vec<Liberacao>,
    /// O comando que está rodando e o resumo dos dados dele.
    chamada: Option<(String, [u8; 32])>,
    /// Sincronização com os outros caixas (só com banco em arquivo).
    _motor: Option<sync::Motor>,
    /// Houve um erro interno grave (pânico): não dá para provar que o que
    /// está na memória ficou certo, então nada mais é gravado até reabrir.
    comprometido: bool,
    /// Parte secreta da chave do backup, aberta com a senha de um gerente
    /// (só na memória, enquanto o programa está aberto). Serve para trancar
    /// a chave para um gerente novo ou com senha nova.
    segredo_backup: Option<[u8; 32]>,
    /// Trava do arquivo do banco (solta quando o serviço acaba).
    _trava: Option<std::fs::File>,
    /// Quanto esperar as tarefas da sincronização ao fechar.
    prazo_encerrar: Duration,
    /// Chaves das operações já gravadas (comando + chave + usuário), para
    /// recusar a mesma repetida (duplo clique).
    feitas: std::collections::VecDeque<[u8; 32]>,
}

/// Trava `<banco>.trava` só para este programa. O sistema solta sozinho se
/// o Rustinho fechar de qualquer jeito (até travando).
fn travar(caminho: &Path) -> Resultado<std::fs::File> {
    let mut nome = caminho.as_os_str().to_owned();
    nome.push(".trava");
    let f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(std::path::PathBuf::from(nome))
        .map_err(|e| Erro::validacao(format!("Não deu para usar a pasta dos dados: {e}")))?;
    resultado_da_trava(f.try_lock())?;
    Ok(f)
}

/// R-07: sem a trava, não abre. Seguir sem ela deixaria dois Rustinhos
/// gravando no mesmo banco (pasta de rede sem trava, falta de permissão...).
fn resultado_da_trava(r: Result<(), std::fs::TryLockError>) -> Resultado<()> {
    match r {
        Ok(()) => Ok(()),
        Err(std::fs::TryLockError::WouldBlock) => Err(Erro::validacao(
            "O Rustinho já está aberto neste computador (talvez em outra janela). Use o que já está aberto.",
        )),
        Err(std::fs::TryLockError::Error(e)) => {
            crate::registro::anotar(&format!("trava do banco: {e}"));
            Err(Erro::validacao(
                "Não deu para garantir que só um Rustinho use estes dados: a pasta não aceita trava \
                 (pasta de rede ou sem permissão). Guarde os dados numa pasta deste computador.",
            ))
        }
    }
}

/// Quanto esperar as tarefas da sincronização terminarem ao fechar (as
/// conexões têm prazo de no máximo uns 45 s).
const PRAZO_PARA_ENCERRAR: Duration = Duration::from_secs(45);

impl Drop for Servico {
    /// R-06: primeiro para a sincronização e espera as tarefas que usam o
    /// banco; só depois a trava é solta. Se alguma não terminar no prazo, a
    /// trava fica presa até o programa fechar: outro Rustinho neste banco é
    /// recusado em vez de gravar junto.
    fn drop(&mut self) {
        if let Some(mut m) = self._motor.take() {
            if !m.encerrar(self.prazo_encerrar) {
                crate::registro::anotar("sync: tarefas não terminaram no prazo; a trava fica até fechar");
                if let Some(t) = self._trava.take() {
                    std::mem::forget(t);
                }
            }
        }
    }
}

impl Servico {
    /// Abre o banco no arquivo indicado. Um Rustinho só por banco (C-12):
    /// um segundo, aberto sem querer, é recusado (restaurar um backup com
    /// outro aberto estragaria o que ele estivesse gravando).
    pub fn abrir(caminho: &Path) -> Resultado<Servico> {
        Self::abrir_com(caminho, true)
    }

    /// Só nos testes: abre como o programa, com ou sem a troca de dados.
    #[cfg(test)]
    pub(crate) fn abrir_para_teste(caminho: &Path, com_motor: bool) -> Resultado<Servico> {
        Self::abrir_com(caminho, com_motor)
    }

    /// Só nos testes: a troca de dados está rodando?
    #[cfg(test)]
    pub(crate) fn tem_motor(&self) -> bool {
        self._motor.is_some()
    }

    fn abrir_com(caminho: &Path, com_motor: bool) -> Resultado<Servico> {
        if let Some(pasta) = caminho.parent() {
            std::fs::create_dir_all(pasta).ok();
            crate::arquivos::so_do_usuario(pasta, "pasta dos dados");
            crate::registro::iniciar(pasta);
        }
        let trava = travar(caminho)?;
        let mut s = Self::com_banco(Banco::abrir(caminho)?);
        // O banco e os arquivos que o SQLite cria ao lado: só do usuário.
        for (fim, o_que) in [("", "banco"), ("-wal", "banco (-wal)"), ("-shm", "banco (-shm)"), (".trava", "trava")] {
            let mut nome = caminho.as_os_str().to_owned();
            nome.push(fim);
            // -wal e -shm só existem com o banco aberto e em uso.
            if Path::new(&nome).exists() {
                crate::arquivos::so_do_usuario(Path::new(&nome), o_que);
            }
        }
        s._trava = Some(trava);
        // A3: banco com defeito (modo seguro): a troca de dados não liga.
        if com_motor && !s.banco.em_modo_seguro() {
            // O portátil usa outras portas (não conversa com um caixa instalado).
            let (tcp, udp) = crate::portatil::portas();
            s._motor = Some(sync::Motor::iniciar(caminho.to_path_buf(), tcp, udp));
        }
        Ok(s)
    }

    /// Banco em memória (testes).
    pub fn em_memoria() -> Resultado<Servico> {
        Ok(Self::com_banco(Banco::em_memoria()?))
    }

    /// Só nos testes: o banco por baixo do serviço.
    #[cfg(test)]
    pub(crate) fn banco_para_teste(&mut self) -> &mut Banco {
        &mut self.banco
    }

    fn com_banco(banco: Banco) -> Servico {
        Servico {
            banco,
            sessao: None,
            liberacoes: Vec::new(),
            chamada: None,
            _motor: None,
            comprometido: false,
            segredo_backup: None,
            _trava: None,
            prazo_encerrar: PRAZO_PARA_ENCERRAR,
            feitas: std::collections::VecDeque::new(),
        }
    }

    /// Executa um comando. Um problema vira uma resposta de erro em JSON.
    ///
    /// Defeito no programa (pânico): a transação que estava aberta é
    /// desfeita pelo SQLite, a resposta é um erro genérico e o serviço fica
    /// "comprometido": daí em diante só consultas, até fechar e abrir o
    /// Rustinho. Assim uma venda que talvez tenha sido gravada não é
    /// repetida, e nada é gravado a partir de um estado que pode estar
    /// errado. (Isto vale porque o perfil de compilação usa `panic = unwind`,
    /// o padrão; com `abort` o programa fecharia na hora.)
    pub fn chamar(&mut self, comando: &str, dados: &str) -> String {
        if self.comprometido && !pode_depois_de_erro(comando) {
            return json!({ "ok": false, "erro": TEXTO_COMPROMETIDO, "campo": null }).to_string();
        }
        // RVI-01: depois de um pânico, uma consulta roda com o banco só para
        // leitura (`query_only`): se algum comando da lista de leituras
        // gravasse, o próprio SQLite recusaria. Nos testes vale sempre, para
        // que a suíte inteira acuse um comando de leitura que grava.
        let travar_leitura =
            SO_LEITURA.contains(&comando) && (self.comprometido || LEITURA_ESTRITA) && !self.banco.em_modo_seguro();
        if travar_leitura {
            if let Err(e) = self.banco.conn.pragma_update(None, "query_only", true) {
                crate::registro::anotar(&format!("{comando}: não foi possível travar a leitura: {e}"));
                if self.comprometido {
                    return json!({ "ok": false, "erro": TEXTO_COMPROMETIDO, "campo": null }).to_string();
                }
            }
        }
        let resultado = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let dados: Value = if dados.trim().is_empty() { json!({}) } else { serde_json::from_str(dados)? };
            self.executar(comando, dados)
        }));
        if travar_leitura {
            if let Err(e) = self.banco.conn.pragma_update(None, "query_only", false) {
                crate::registro::anotar(&format!("{comando}: não foi possível destravar a leitura: {e}"));
            }
        }
        let resultado = match resultado {
            Ok(Err(e)) if travar_leitura && crate::banco::erro_de_somente_leitura(&e) => {
                crate::registro::anotar(&format!("{comando}: comando de leitura tentou gravar (recusado)"));
                Ok(Err(if self.comprometido {
                    Erro::validacao(TEXTO_COMPROMETIDO)
                } else {
                    Erro::Interno(format!("\"{comando}\" está em SO_LEITURA mas tentou gravar no banco"))
                }))
            }
            outro => outro,
        };
        let resposta = match resultado {
            Ok(Ok(valor)) => json!({ "ok": true, "dados": valor }),
            Ok(Err(erro)) => {
                // Só o comando e o erro técnico; os dados digitados não.
                if matches!(erro, Erro::Interno(_) | Erro::Banco(_)) {
                    crate::registro::anotar(&format!("{comando}: {erro}"));
                }
                json!({ "ok": false, "erro": erro.to_string(), "campo": erro.nome_do_campo() })
            }
            Err(_) => {
                self.comprometido = true;
                self.liberacoes.clear();
                // Se algo ficou no meio de uma transação, desfaz.
                if !self.banco.conn.is_autocommit() {
                    let _ = self.banco.conn.execute_batch("ROLLBACK");
                }
                crate::registro::anotar(&format!(
                    "{comando}: pânico (veja a linha anterior); só consultas até reabrir"
                ));
                json!({
                    "ok": false,
                    "erro": "Erro interno inesperado. Confira se a operação foi gravada antes de repetir. Feche e abra o Rustinho para continuar.",
                    "campo": null
                })
            }
        };
        resposta.to_string()
    }

    fn sessao(&self) -> Resultado<&Sessao> {
        self.sessao.as_ref().ok_or(Erro::SemSessao)
    }

    /// A3: em modo seguro (banco com defeito) as gravações são recusadas
    /// com o motivo antes de começar; e qualquer gravação que escape (o banco
    /// está só para leitura) volta com o mesmo motivo.
    fn executar(&mut self, comando: &str, dados: Value) -> Resultado<Value> {
        if !self.banco.em_modo_seguro() {
            return self.executar_livre(comando, dados);
        }
        if bloqueado_no_modo_seguro(comando) {
            return Err(erro_modo_seguro());
        }
        self.executar_livre(comando, dados).map_err(|e| {
            if crate::banco::erro_de_somente_leitura(&e) {
                erro_modo_seguro()
            } else {
                e
            }
        })
    }

    /// No modo seguro, entrar, sair e a senha do gerente funcionam, mas nada
    /// do que eles gravariam (acesso, tentativas, histórico) é gravado.
    fn gravar_se_der(&self, r: Resultado<()>) -> Resultado<()> {
        match r {
            Err(e) if self.banco.em_modo_seguro() && crate::banco::erro_de_somente_leitura(&e) => Ok(()),
            outro => outro,
        }
    }

    fn executar_livre(&mut self, comando: &str, dados: Value) -> Resultado<Value> {
        match comando {
            // Sem precisar estar conectado.
            "estado" => self.estado(),
            "configurar_inicio" => self.configurar_inicio(ler(dados)?),
            "usuarios_entrada" => {
                let lista = usuarios::listar(&self.banco.conn, true)?;
                Ok(json!(lista.into_iter().map(|u| json!({ "id": u.id, "nome": u.nome })).collect::<Vec<_>>()))
            }
            "entrar" => self.entrar(ler(dados)?),
            "sync_juntar" => {
                if usuarios::quantidade(&self.banco.conn)? > 0 {
                    return Err(Erro::validacao(
                        "Este caixa já tem usuários. Use Configurações para ligar a sincronização.",
                    ));
                }
                let d: Juntar = ler(dados)?;
                let caixa = texto::limpar(&d.caixa);
                config::validar_nome_caixa(&caixa)?;
                sync::salvar_ajustes(&self.banco.conn, true, &d.senha, &d.enderecos)?;
                config::gravar(&self.banco.conn, "caixa_nome", &caixa)?;
                // FALHA-01: a tentativa fica marcada até chegar um usuário
                // da loja ou até desistir (veja `loja::desistir_da_juncao`).
                crate::loja::comecar_juncao(&self.banco.conn)?;
                self.sync_ler()
            }
            // FALHA-01 (V01a): desistir de entrar numa loja. Sai tudo o que a
            // tentativa deixou; a rede de uma loja configurada não é tocada.
            "sync_desistir_juntar" => {
                let n = crate::loja::desistir_da_juncao(&mut self.banco.conn)?;
                Ok(json!({ "saiu": n }))
            }
            "sync_ler" => self.sync_ler(),
            // Modo escuro: vale só para este aparelho (não vai para os outros caixas).
            "aparencia_salvar" => {
                let d: Aparencia = ler(dados)?;
                config::gravar(&self.banco.conn, "tela_escura", if d.escuro { "1" } else { "0" })?;
                Ok(json!({ "escuro": d.escuro }))
            }
            // Câmera do celular como leitor de código de barras.
            "codigo_ler_foto" => {
                let d: Arquivo = ler(dados)?;
                Ok(json!({ "codigo": codigo::ler_foto(&arquivos::caminho_local(&d.caminho))?.unwrap_or_default() }))
            }
            // Backup automático e ao fechar: rodam mesmo sem ninguém conectado.
            "backup_automatico" => {
                if let Some(c) = &self.banco.caminho {
                    if usuarios::quantidade(&self.banco.conn)? > 0 {
                        backup::automatico(c.clone());
                    }
                }
                Ok(Value::Null)
            }
            "backup_ao_fechar" => match &self.banco.caminho {
                Some(c) if usuarios::quantidade(&self.banco.conn)? > 0 => Ok(json!({ "feito": backup::ao_fechar(c)? })),
                _ => Ok(json!({ "feito": false })),
            },
            "sair" => {
                self.liberacoes.clear();
                if let Some(s) = self.sessao.take() {
                    self.gravar_se_der(auditoria::registrar(
                        &self.banco.conn,
                        &self.banco.dispositivo,
                        Some(&s.usuario_id),
                        "sair",
                        "usuario",
                        &s.usuario_id,
                        &[],
                    ))?;
                }
                Ok(Value::Null)
            }
            "numero_formatar" => {
                let d: Formatar = ler(dados)?;
                Ok(json!({ "texto": formatar(&d.tipo, &d.texto) }))
            }
            "preco_calcular" => {
                let d: PrecoCalculo = ler(dados)?;
                let (lucro, preco) = produtos::calcular_preco(&d.custo, &d.lucro, &d.preco, &d.de);
                Ok(json!({ "lucro": lucro, "preco": preco }))
            }
            // Daqui para baixo, só com alguém conectado.
            _ => {
                self.sessao()?;
                // O papel pode ter mudado em outro caixa.
                self.atualizar_sessao()?;
                let sessao = self.sessao()?.clone();
                self.executar_conectado(comando, dados, &sessao)
            }
        }
    }

    /// Pode fazer esta ação do gerente sem senha? (é gerente, ou a loja
    /// liberou a ação para todos na pílula Gerente.)
    fn pode_sem_senha(&self, acao: &str, sessao: &Sessao) -> Resultado<bool> {
        Ok(sessao.gerente
            || (!SEMPRE_DO_GERENTE.contains(&acao) && acoes_livres(&self.banco.conn)?.iter().any(|a| a == acao)))
    }

    /// Pode ler o que é desta ação? Sem senha, ou com a senha do gerente
    /// digitada para consultar a tela dela (renova a cada leitura).
    fn pode_ler(&mut self, acao: &str, sessao: &Sessao) -> Resultado<bool> {
        if self.pode_sem_senha(acao, sessao)? {
            return Ok(true);
        }
        Ok(self.consulta_aberta(acao, sessao))
    }

    fn consulta_aberta(&mut self, acao: &str, sessao: &Sessao) -> bool {
        let agora = Instant::now();
        self.liberacoes.retain(|l| agora < l.ate());
        for l in &mut self.liberacoes {
            if let Liberacao::Consulta { usuario_id, acao: a, ate } = l {
                if *a == acao && *usuario_id == sessao.usuario_id {
                    *ate = agora + LIBERACAO;
                    return true;
                }
            }
        }
        false
    }

    /// A senha do gerente foi digitada para esta chamada (o mesmo comando,
    /// com os mesmos dados, pelo mesmo usuário)?
    fn liberada_esta_chamada(&mut self, sessao: &Sessao) -> bool {
        let agora = Instant::now();
        self.liberacoes.retain(|l| agora < l.ate());
        let Some((comando, resumo)) = &self.chamada else { return false };
        self.liberacoes.iter().any(|l| {
            matches!(l, Liberacao::Uma { usuario_id, comando: c, resumo: r, .. }
                if usuario_id == &sessao.usuario_id && c == comando && r == resumo)
        })
    }

    /// Esta chamada pode fazer esta ação do gerente? Uma leitura vale com a
    /// consulta da tela; qualquer outra coisa precisa da senha para ela.
    fn exigir(&mut self, acao: &str, sessao: &Sessao) -> Resultado<()> {
        if self.pode_sem_senha(acao, sessao)? || self.liberada_esta_chamada(sessao) {
            return Ok(());
        }
        let leitura = self
            .chamada
            .as_ref()
            .map(|(c, _)| SO_LEITURA.contains(&c.as_str()) && acao_do_comando(c) == Some(acao))
            .unwrap_or(false);
        if leitura && self.consulta_aberta(acao, sessao) {
            return Ok(());
        }
        // "precos" tem campo próprio: a tela sabe que é o preço da peça que
        // pede a senha.
        Err(Erro::campo(
            if acao == "precos" { "precos" } else { "gerente" },
            format!("Só o gerente pode: {}. Peça para o gerente digitar a senha.", nome_da_acao(acao)),
        ))
    }

    /// Roda o comando. Se deu certo, a senha do gerente digitada para ele
    /// (mesmos dados) acaba: não fica aberta para outra coisa (D-09).
    fn executar_conectado(&mut self, comando: &str, dados: Value, sessao: &Sessao) -> Resultado<Value> {
        let resumo = resumo_da_chamada(comando, &dados);
        // Duplo clique: a mesma operação (a mesma chave) chegando de novo.
        let repeticao = match dados["chaveOperacao"].as_str().map(str::trim) {
            Some(k) if !k.is_empty() && NAO_REPETIR.contains(&comando) => {
                use sha2::Digest;
                let mut h = sha2::Sha256::new();
                h.update(comando.as_bytes());
                h.update([0u8]);
                h.update(k.as_bytes());
                h.update([0u8]);
                h.update(sessao.usuario_id.as_bytes());
                let chave: [u8; 32] = h.finalize().into();
                Some(chave)
            }
            _ => None,
        };
        // FALHA-04: todas as telas mandam a chave nestes comandos; sem ela
        // não dá para saber se é a mesma operação de novo, então nada é gravado.
        if repeticao.is_none() && NAO_REPETIR.contains(&comando) {
            return Err(Erro::campo(
                "chave",
                "Operação sem identificação (chave da operação): nada foi gravado. Feche e abra a tela e tente de novo.",
            ));
        }
        if let Some(chave) = repeticao {
            if self.feitas.contains(&chave) {
                return Err(Erro::campo(
                    "repetida",
                    "Esta operação já foi feita (clique repetido): nada foi gravado de novo.",
                ));
            }
        }
        self.chamada = Some((comando.to_string(), resumo));
        let r = self.executar_comando(comando, dados, sessao);
        self.chamada = None;
        if let (Some(chave), Ok(_)) = (repeticao, &r) {
            self.feitas.push_back(chave);
            while self.feitas.len() > CHAVES_GUARDADAS {
                self.feitas.pop_front();
            }
        }
        if r.is_ok() {
            self.liberacoes.retain(|l| {
                !matches!(l, Liberacao::Uma { usuario_id, comando: c, resumo: d, .. }
                    if usuario_id == &sessao.usuario_id && c == comando && *d == resumo)
            });
        }
        r
    }

    fn executar_comando(&mut self, comando: &str, dados: Value, sessao: &Sessao) -> Resultado<Value> {
        let disp = self.banco.dispositivo.clone();
        if let Some(acao) = acao_do_comando(comando) {
            self.exigir(acao, sessao)?;
        }
        match comando {
            // ---- Gerente ----
            // Pode sem senha? Com "consulta", vale também a senha digitada
            // para consultar a tela desta ação.
            "permissao" => {
                let d: Acao = ler(dados)?;
                let pode =
                    if d.consulta { self.pode_ler(&d.acao, sessao)? } else { self.pode_sem_senha(&d.acao, sessao)? };
                Ok(json!({ "pode": pode, "gerente": sessao.gerente }))
            }
            "gerente_liberar" => {
                let d: SenhaGerente = ler(dados)?;
                // Para uma gravação (o comando e os dados exatos) ou para
                // consultar a tela de uma ação.
                let pedido = if !d.comando.trim().is_empty() {
                    if !comando_do_gerente(&d.comando) {
                        return Err(Erro::validacao("Diga para que é a senha do gerente."));
                    }
                    let dados = d.dados.clone().unwrap_or_else(|| json!({}));
                    Err((d.comando.clone(), resumo_da_chamada(&d.comando, &dados)))
                } else if !d.acoes.is_empty() {
                    // Simultaneidade (0.9.15): uma senha abre a consulta das
                    // partes que ficam juntas ali (só leitura; cada gravação
                    // continua pedindo a senha para ela).
                    let mut lista: Vec<&'static str> = Vec::new();
                    for a in &d.acoes {
                        let a = acao_de_consulta(a)?;
                        if !lista.contains(&a) {
                            lista.push(a);
                        }
                    }
                    Ok(lista)
                } else {
                    Ok(vec![acao_de_consulta(&d.acao)?])
                };
                self.esperar_se_errou("gerente")?;
                match usuarios::conferir_gerente(&self.banco.conn, &d.senha)? {
                    Some(g) => {
                        self.gravar_se_der(usuarios::limpar_falhas(&self.banco.conn, "gerente"))?;
                        self.segredo_backup = self.segredo_conferido(&g.id, &d.senha)?;
                        let usuario_id = sessao.usuario_id.clone();
                        let o_que = match &pedido {
                            Ok(acoes) => {
                                for &acao in acoes.iter() {
                                    self.liberacoes.retain(|l| {
                                        !matches!(l, Liberacao::Consulta { acao: a, usuario_id: u, .. }
                                            if *a == acao && *u == usuario_id)
                                    });
                                    self.liberacoes.push(Liberacao::Consulta {
                                        usuario_id: usuario_id.clone(),
                                        acao,
                                        ate: Instant::now() + LIBERACAO,
                                    });
                                }
                                let nomes: Vec<String> = acoes.iter().map(|a| nome_da_acao(a)).collect();
                                format!("consultar {} (só leitura, nesta tela)", nomes.join("; "))
                            }
                            Err((comando, resumo)) => {
                                self.liberacoes.retain(|l| {
                                    !matches!(l, Liberacao::Uma { comando: c, usuario_id: u, .. }
                                        if c == comando && *u == usuario_id)
                                });
                                self.liberacoes.push(Liberacao::Uma {
                                    usuario_id,
                                    comando: comando.clone(),
                                    resumo: *resumo,
                                    ate: Instant::now() + LIBERACAO_CURTA,
                                });
                                format!(
                                    "{} (uma vez, com estes dados)",
                                    nome_da_acao(acao_do_comando(comando).unwrap_or("precos"))
                                )
                            }
                        };
                        self.gravar_se_der(auditoria::registrar(
                            &self.banco.conn,
                            &disp,
                            Some(&sessao.usuario_id),
                            "gerente.liberar",
                            "usuario",
                            &g.id,
                            &[format!("{} liberou para {}: {o_que}", g.nome, sessao.nome)],
                        ))?;
                        Ok(json!({ "nome": g.nome }))
                    }
                    None => {
                        self.gravar_se_der(usuarios::anotar_falha(&self.banco.conn, "gerente", ESPERA_APOS_ERROS))?;
                        Err(Erro::campo("senha", "Senha do gerente errada."))
                    }
                }
            }
            "gerente_encerrar" => {
                self.liberacoes.clear();
                Ok(Value::Null)
            }
            "gerente_ler" => self.gerente_ler(),
            // Conflitos entre caixas (pílula Gerente).
            "conflitos_listar" => Ok(json!(sync::conflitos(&self.banco.conn)?)),
            "conflito_resolver" => {
                let d: Resolver = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                sync::resolver_conflito(&tx, d.id, d.usar, &sessao.usuario_id)?;
                auditoria::registrar(
                    &tx,
                    &disp,
                    Some(&sessao.usuario_id),
                    "conflito.resolver",
                    "sync_conflito",
                    &d.id.to_string(),
                    &[if d.usar { "Voltou a versão que tinha perdido".into() } else { "Ficou como está".into() }],
                )?;
                tx.commit()?;
                Ok(json!(sync::conflitos(&self.banco.conn)?))
            }
            // R-05: o gerente aceita como está um registro comprovadamente inconsistente.
            "referencia_aceitar" => {
                let d: AceitarReferencia = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                sync::aceitar_inconsistencia(&tx, &sessao.usuario_id, &d.tabela, &d.chave, &d.motivo)?;
                auditoria::registrar(
                    &tx,
                    &disp,
                    Some(&sessao.usuario_id),
                    "referencia.aceitar",
                    &d.tabela,
                    &d.chave,
                    &[format!("Aceito como está. Motivo: {}", crate::texto::limpar(&d.motivo))],
                )?;
                tx.commit()?;
                Ok(json!(sync::referencias(&self.banco.conn)?))
            }
            // Códigos das peças: faixas, repetidos e peças com código a definir.
            "codigos_situacao" => faixas::situacao(&self.banco.conn),
            "codigos_assumir" => {
                let tx = self.banco.conn.transaction()?;
                faixas::assumir(&tx)?;
                auditoria::registrar(
                    &tx,
                    &disp,
                    Some(&sessao.usuario_id),
                    "codigos.assumir",
                    "config",
                    "codigos_distribuidor",
                    &["Este caixa passou a distribuir as faixas de códigos".into()],
                )?;
                tx.commit()?;
                faixas::manter(&mut self.banco.conn)?;
                faixas::situacao(&self.banco.conn)
            }
            "codigos_definir_pendentes" => {
                let tx = self.banco.conn.transaction()?;
                let n = faixas::definir_todos_pendentes(&tx)?;
                let p = faixas::definir_todas_pessoas_pendentes(&tx)?;
                auditoria::registrar(
                    &tx,
                    &disp,
                    Some(&sessao.usuario_id),
                    "codigos.definir",
                    "produto",
                    "",
                    &[format!(
                        "{n} peças e {p} clientes ou fornecedores com código a definir receberam código deste caixa"
                    )],
                )?;
                tx.commit()?;
                faixas::situacao(&self.banco.conn)
            }
            "codigos_confirmar_computador" => {
                let tx = self.banco.conn.transaction()?;
                faixas::confirmar_computador(&tx, crate::computador::atual().as_deref())?;
                auditoria::registrar(
                    &tx,
                    &disp,
                    Some(&sessao.usuario_id),
                    "codigos.computador",
                    "config",
                    "computador_id",
                    &["O gerente confirmou que este é o computador de sempre deste caixa".into()],
                )?;
                tx.commit()?;
                faixas::manter(&mut self.banco.conn)?;
                faixas::situacao(&self.banco.conn)
            }
            "codigos_registrar_caixa_novo" => {
                #[derive(Deserialize, Default)]
                #[serde(default)]
                struct Nome {
                    nome: String,
                }
                let d: Nome = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let novo = faixas::registrar_caixa_novo(&tx, crate::computador::atual().as_deref(), Some(&d.nome))?;
                auditoria::registrar(
                    &tx,
                    &novo,
                    Some(&sessao.usuario_id),
                    "codigos.caixa_novo",
                    "config",
                    "dispositivo_id",
                    &["Este banco foi registrado como caixa novo (identificador novo, faixa de códigos nova)".into()],
                )?;
                tx.commit()?;
                self.banco.dispositivo = novo;
                faixas::manter(&mut self.banco.conn)?;
                faixas::situacao(&self.banco.conn)
            }
            "codigos_pessoa_dar_novo" => {
                let d: Id = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                pessoas::trocar_codigo(&tx, &disp, &sessao.usuario_id, &d.id)?;
                tx.commit()?;
                faixas::situacao(&self.banco.conn)
            }
            "codigos_dar_novo" => {
                let d: Id = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                produtos::trocar_codigo(&tx, &disp, &sessao.usuario_id, &d.id)?;
                tx.commit()?;
                faixas::situacao(&self.banco.conn)
            }
            "conflitos_manter_todos" => {
                let tx = self.banco.conn.transaction()?;
                let n = sync::manter_todos(&tx, &sessao.usuario_id)?;
                auditoria::registrar(
                    &tx,
                    &disp,
                    Some(&sessao.usuario_id),
                    "conflito.manter_todos",
                    "sync_conflito",
                    "",
                    &[format!("{n} diferenças ficaram como estão")],
                )?;
                tx.commit()?;
                Ok(json!(sync::conflitos(&self.banco.conn)?))
            }
            // Só nos testes: defeito de propósito em pontos diferentes (C-15).
            #[cfg(test)]
            "__teste_panico" => {
                let d: Value = dados;
                let tx = self.banco.conn.transaction()?;
                tx.execute(
                    "INSERT INTO categoria (id, nome, tipo, atualizado_em) VALUES ('t', 'Teste', 'despesa', 'x')",
                    [],
                )?;
                if d["onde"] == "antes_do_commit" {
                    panic!("defeito de teste com senha=segredo123 e cpf 529.982.247-25");
                }
                tx.commit()?;
                if d["onde"] == "depois_do_commit" {
                    panic!("defeito de teste depois de gravar");
                }
                Ok(Value::Null)
            }
            "gerente_salvar" => {
                let d: GerenteEntrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let mut detalhes = Vec::new();
                let mut livres = Vec::new();
                for (id, nome) in ACOES_DO_GERENTE {
                    let so_gerente = d.acoes.get(*id).and_then(|v| v.as_bool()).unwrap_or(true);
                    if !so_gerente {
                        livres.push(*id);
                    }
                    let antes = !acoes_livres(&tx)?.iter().any(|a| a == id);
                    if antes != so_gerente {
                        detalhes.push(format!("{nome}: {}", if so_gerente { "só o gerente" } else { "todos" }));
                    }
                }
                for (id, gerente) in d.gerentes.iter().filter_map(|(k, v)| v.as_bool().map(|b| (k, b))) {
                    if usuarios::definir_gerente(&tx, id, gerente)? {
                        if !gerente {
                            backup::deixou_de_ser_gerente(&tx, id)?;
                        }
                        let u = usuarios::buscar(&tx, id)?;
                        detalhes.push(format!("{}: {}", u.nome, if gerente { "gerente" } else { "caixa" }));
                    }
                }
                config::gravar(&tx, "gerente_livres", &livres.join(","))?;
                if !detalhes.is_empty() {
                    auditoria::registrar(
                        &tx,
                        &disp,
                        Some(&sessao.usuario_id),
                        "gerente.salvar",
                        "config",
                        "gerente",
                        &detalhes,
                    )?;
                }
                tx.commit()?;
                self.atualizar_sessao()?;
                self.gerente_ler()
            }
            // Configurações deste computador, para qualquer um.
            "computador_salvar" => {
                let d: Computador = ler(dados)?;
                let caixa = texto::limpar(&d.caixa);
                config::validar_nome_caixa(&caixa)?;
                // M-04: tudo ou nada.
                let tx = self.banco.conn.transaction()?;
                config::gravar(&tx, "caixa_nome", &caixa)?;
                config::gravar(&tx, "imprimir_direto", if d.imprimir_direto { "1" } else { "0" })?;
                tx.commit()?;
                let ve = self.ve_custos(sessao)?;
                self.config_ler(ve)
            }

            "menu" => match menu::menu(&self.banco.conn, &sessao.usuario_id) {
                Ok(m) => Ok(json!(m)),
                // RVI-03: banco antigo em modo seguro: o menu sem as fixadas
                // (o Início precisa dele para mostrar o aviso do modo seguro).
                Err(e) if self.banco.em_modo_seguro() => {
                    crate::registro::anotar(&format!("menu no modo seguro: {e}"));
                    Ok(json!(menu::sem_fixadas()))
                }
                Err(e) => Err(e),
            },
            "menu_usar" => {
                let d: ItemMenu = ler(dados)?;
                menu::registrar_uso(&self.banco.conn, &sessao.usuario_id, &d.id)?;
                Ok(Value::Null)
            }
            "menu_fixar" => {
                let d: ItemMenu = ler(dados)?;
                menu::fixar(&self.banco.conn, &sessao.usuario_id, &d.id, d.fixo)?;
                Ok(json!(menu::menu(&self.banco.conn, &sessao.usuario_id)?))
            }

            "produtos_buscar" => {
                let d: BuscaProdutos = ler(dados)?;
                let lista = busca::buscar(&self.banco.conn, &d.texto, d.inativas, d.limite.unwrap_or(200))?;
                let total = busca::total(&self.banco.conn, d.inativas)?;
                Ok(json!({ "itens": lista, "total": total }))
            }
            "produto_novo" => {
                let f = produtos::ficha_nova(&self.banco.conn)?;
                self.peca_para_tela(f, sessao)
            }
            "produto_abrir" => {
                let d: Id = ler(dados)?;
                let f = produtos::ficha(&self.banco.conn, &d.id)?;
                self.peca_para_tela(f, sessao)
            }
            "produto_salvar" => {
                let mut ficha: produtos::ProdutoFicha = ler(dados)?;
                // D-09: quem não vê custo e lucro não mexe neles (o custo
                // gravado fica; o % segue o preço).
                if !self.ve_custos(sessao)? {
                    produtos::tirar_custos_da_entrada(&self.banco.conn, &mut ficha)?;
                }
                // Mudar o preço de venda (com custo ou sem) ou o % de lucro
                // de uma peça cadastrada: só com o gerente, para esta
                // gravação com estes dados (D-09).
                if produtos::muda_preco(&self.banco.conn, &ficha)? {
                    self.exigir("precos", sessao)?;
                }
                let tx = self.banco.conn.transaction()?;
                let id = produtos::salvar(&tx, &disp, &sessao.usuario_id, &ficha)?;
                tx.commit()?;
                let f = produtos::ficha(&self.banco.conn, &id)?;
                self.peca_para_tela(f, sessao)
            }
            "produto_sugestoes" => {
                let mut v = json!(produtos::sugestoes(&self.banco.conn)?);
                let lista = |l: &[(&str, &str)]| {
                    json!(l.iter().map(|(c, t)| json!({ "valor": c, "texto": t })).collect::<Vec<_>>())
                };
                v["csosns"] = lista(fiscal::CSOSNS);
                v["cstIpi"] = lista(fiscal::CST_IPI);
                v["cstPis"] = lista(fiscal::CST_PIS);
                v["exigibilidades"] = lista(fiscal::EXIGIBILIDADE_ISS);
                v["ufs"] = json!(crate::documento::UFS);
                Ok(v)
            }
            "unidades_listar" => Ok(json!(unidades::listar(&self.banco.conn)?)),
            "unidade_salvar" => {
                let d: UnidadeEntrada = ler(dados)?;
                let sigla = unidades::salvar(&self.banco.conn, &d.sigla, &d.descricao, d.fracionada)?;
                Ok(json!({ "sigla": sigla, "lista": unidades::listar(&self.banco.conn)? }))
            }
            "produto_variacao_criar" => {
                let d: VariacaoEntrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let id = produtos::criar_variacao(&tx, &disp, &sessao.usuario_id, &d.id, &d.atual, &d.nova)?;
                tx.commit()?;
                let f = produtos::ficha(&self.banco.conn, &id)?;
                self.peca_para_tela(f, sessao)
            }
            "estoque_ajustar" => {
                let d: AjusteEstoque = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                produtos::ficha(&tx, &d.produto_id)?; // confere se a peça existe
                estoque::ajustar(&tx, &disp, &sessao.usuario_id, &d.produto_id, &d.quantidade, &d.motivo)?;
                tx.commit()?;
                let f = produtos::ficha(&self.banco.conn, &d.produto_id)?;
                self.peca_para_tela(f, sessao)
            }

            "usuarios_listar" => {
                let lista = usuarios::listar(&self.banco.conn, false)?;
                Ok(json!(lista
                    .into_iter()
                    .map(|u| {
                        let voce = u.id == sessao.usuario_id;
                        let mut v = json!(u);
                        v["voce"] = json!(voce);
                        // Só o gerente vê quem está com senha fora da política.
                        let fraca = sessao.gerente
                            && matches!(usuarios::situacao_senha(&self.banco.conn, &u.id), Ok(Some(Some(_))));
                        v["senhaFraca"] = json!(fraca);
                        v
                    })
                    .collect::<Vec<_>>()))
            }
            "usuario_salvar" => self.usuario_salvar(ler(dados)?, sessao),
            "senha_trocar" => {
                let d: TrocaSenha = ler(dados)?;
                usuarios::conferir_senha(&self.banco.conn, &sessao.usuario_id, &d.atual)
                    .map_err(|_| Erro::campo("atual", "A senha atual não confere."))?;
                if d.nova != d.nova2 {
                    return Err(Erro::campo("nova2", "As duas senhas novas não são iguais."));
                }
                let u = usuarios::buscar(&self.banco.conn, &sessao.usuario_id)?;
                usuarios::exigir_politica(&self.banco.conn, &d.nova, &u.nome, u.gerente, "nova")?;
                let tx = self.banco.conn.transaction()?;
                usuarios::alterar(&tx, &u.id, &u.nome, u.ativo, Some(&d.nova))
                    .map_err(renomear_campo("senha", "nova"))?;
                backup::senha_trocada(&tx, &u.id, &d.nova, self.segredo_backup)?;
                // M-04: o histórico vai na mesma transação (sem ele, nada muda).
                auditoria::registrar(
                    &tx,
                    &disp,
                    Some(&sessao.usuario_id),
                    "usuario.alterar",
                    "usuario",
                    &u.id,
                    &["Senha trocada".into()],
                )?;
                tx.commit()?;
                Ok(Value::Null)
            }

            // Clientes e fornecedores
            "pessoas_buscar" => {
                let d: BuscaPessoas = ler(dados)?;
                let itens = pessoas::buscar(&self.banco.conn, &d.texto, &d.papel, d.inativos, d.limite.unwrap_or(200))?;
                let total = pessoas::total(&self.banco.conn, &d.papel)?;
                Ok(json!({ "itens": itens, "total": total }))
            }
            "pessoa_nova" => {
                let d: Papel = ler(dados)?;
                Ok(json!(pessoas::ficha_nova(&self.banco.conn, &d.papel)?))
            }
            "pessoa_abrir" => {
                let d: Id = ler(dados)?;
                Ok(json!(pessoas::ficha(&self.banco.conn, &d.id)?))
            }
            "pessoa_salvar" => {
                let ficha: pessoas::PessoaFicha = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let id = pessoas::salvar(&tx, &disp, &sessao.usuario_id, &ficha)?;
                tx.commit()?;
                Ok(json!(pessoas::ficha(&self.banco.conn, &id)?))
            }

            // Compras
            "compras_listar" => {
                let d: BuscaProdutos = ler(dados)?;
                Ok(json!(compras::listar(&self.banco.conn, &d.texto, d.limite.unwrap_or(300))?))
            }
            // Um arquivo escolhido: um XML, um .zip ou .rar com várias notas,
            // ou uma pasta. Cada nota vira uma compra; o que falhar vem em "erros".
            "compra_importar" => {
                let d: Arquivo = ler(dados)?;
                if d.caminho.trim().is_empty() {
                    return Err(Erro::validacao("Escolha o arquivo XML da nota."));
                }
                // 0.9.17: sem o CNPJ da loja (ou com um inválido) não importa:
                // a tela leva a Configurações › Loja e volta com os arquivos.
                crate::requisitos::exigir_cnpj_da_loja(&self.banco.conn, crate::requisitos::PARA_COMPRAS)?;
                let lista = arquivos::xmls(std::slice::from_ref(&d.caminho))?;
                let pacote = lista.len() > 1 || lista.iter().any(|x| x.nome.contains(" › "));
                let mut fichas = Vec::new();
                let mut erros = Vec::new();
                // Notas emitidas para outro CNPJ: não entram; a tela mostra a
                // janela de conflito para conferir ou descartar.
                let mut conflitos = Vec::new();
                for x in lista {
                    if !x.erro.is_empty() {
                        erros.push(format!("{}: {}", x.nome, x.erro));
                        continue;
                    }
                    let tx = self.banco.conn.transaction()?;
                    match compras::importar_xml(&tx, &disp, &sessao.usuario_id, &x.texto) {
                        Ok(id) => {
                            tx.commit()?;
                            fichas.push(compras::ficha(&self.banco.conn, &id)?);
                        }
                        // Dentro de um pacote, eventos (cancelamento, carta de
                        // correção) e outros XMLs que não são nota são pulados.
                        Err(e)
                            if pacote
                                && ["não é de uma NF-e", "resumo ou um evento", "CT-e", "não é um XML válido"]
                                    .iter()
                                    .any(|t| e.to_string().contains(t)) => {}
                        Err(e) if e.nome_do_campo() == Some(compras::CAMPO_OUTRO_CNPJ) => {
                            conflitos.push(compras::conflito_cnpj(&tx, &x.nome, &x.texto)?);
                        }
                        Err(e) => erros.push(format!("{}: {e}", x.nome)),
                    }
                }
                if fichas.is_empty() && erros.is_empty() && conflitos.is_empty() {
                    return Err(Erro::validacao("Não achei nenhuma NF-e nesse arquivo."));
                }
                Ok(json!({ "compras": fichas, "erros": erros, "conflitos": conflitos }))
            }
            "compra_abrir" => {
                let d: Id = ler(dados)?;
                Ok(json!(compras::ficha(&self.banco.conn, &d.id)?))
            }
            "compra_atualizar" => {
                let d: compras::CompraEntrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                compras::atualizar(&tx, &d)?;
                tx.commit()?;
                Ok(json!(compras::ficha(&self.banco.conn, &d.id)?))
            }
            "compra_lancar" => {
                let d: compras::CompraEntrada = ler(dados)?;
                // 0915-N / D-09: a compra que muda o preço de venda de uma
                // peça já cadastrada ("os preços seguem o % de lucro") é do
                // gerente, como no cadastro. Confere antes (sem efeito) e
                // decide depois de saber se algum preço mudou: sem a senha,
                // nada é lançado (a transação volta inteira).
                let sem_precos = self.exigir("precos", sessao).err();
                let tx = self.banco.conn.transaction()?;
                compras::atualizar(&tx, &d)?;
                let r = compras::lancar(&tx, &disp, &sessao.usuario_id, &d.id)?;
                if r.precos_atualizados > 0 {
                    if let Some(e) = sem_precos {
                        return Err(e);
                    }
                }
                tx.commit()?;
                Ok(json!({ "resumo": r, "compra": compras::ficha(&self.banco.conn, &d.id)? }))
            }
            "compra_estornar" => {
                let d: Estorno = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                compras::estornar(&tx, &disp, &sessao.usuario_id, &d.id, &d.motivo)?;
                tx.commit()?;
                Ok(json!(compras::ficha(&self.banco.conn, &d.id)?))
            }
            "compra_conferir_autorizacao" => {
                let d: ConferenciaDoGerente = ler(dados)?;
                self.esperar_se_errou("gerente")?;
                let g = match usuarios::conferir_gerente(&self.banco.conn, &d.senha)? {
                    Some(g) => {
                        usuarios::limpar_falhas(&self.banco.conn, "gerente")?;
                        g
                    }
                    None => {
                        usuarios::anotar_falha(&self.banco.conn, "gerente", ESPERA_APOS_ERROS)?;
                        return Err(Erro::campo("senha", "Senha do gerente errada."));
                    }
                };
                let tx = self.banco.conn.transaction()?;
                compras::registrar_conferencia(&tx, &disp, &g.id, &d.conferencia)?;
                tx.commit()?;
                Ok(json!(compras::ficha(&self.banco.conn, &d.conferencia.id)?))
            }
            "compra_descartar" => {
                let d: Id = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                compras::excluir_rascunho(&tx, &disp, &sessao.usuario_id, &d.id)?;
                tx.commit()?;
                Ok(Value::Null)
            }

            // Fiscal
            "fiscal_conferir" => Ok(json!(fiscal::conferir(&self.banco.conn, &fiscal::hoje(&self.banco.conn)?)?)),
            "fiscal_automatico" => {
                let d: Ids = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let n = fiscal::usar_automatico(&tx, &disp, &sessao.usuario_id, &d.ids)?;
                tx.commit()?;
                Ok(
                    json!({ "alteradas": n, "conferencia": fiscal::conferir(&self.banco.conn, &fiscal::hoje(&self.banco.conn)?)? }),
                )
            }
            "fiscal_regras_salvar" => {
                let d: Regras = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                fiscal::salvar_regras(&tx, &disp, &sessao.usuario_id, &d.regras)?;
                tx.commit()?;
                Ok(json!(fiscal::conferir(&self.banco.conn, &fiscal::hoje(&self.banco.conn)?)?))
            }
            "fiscal_tributacao" => {
                let d: Tributacao = ler(dados)?;
                let uf = fiscal::uf_loja(&self.banco.conn)?;
                Ok(json!(fiscal::tributacao(
                    &self.banco.conn,
                    &uf,
                    &d.cest,
                    &d.csosn,
                    &fiscal::hoje(&self.banco.conn)?
                )?))
            }

            // Vendas e orçamentos
            "venda_calcular" => {
                let d: vendas::VendaEntrada = ler(dados)?;
                Ok(json!(vendas::calcular(&self.banco.conn, &d)?))
            }
            "venda_concluir" => {
                let d: vendas::VendaEntrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let id = vendas::concluir(&tx, &disp, &sessao.usuario_id, &d)?;
                tx.commit()?;
                Ok(json!(vendas::ficha(&self.banco.conn, &id)?))
            }
            "orcamento_salvar" => {
                let d: vendas::VendaEntrada = ler(dados)?;
                let caixa_nome = config::nome_caixa(&self.banco.conn)?;
                let tx = self.banco.conn.transaction()?;
                let id = vendas::salvar_orcamento(&tx, &disp, &sessao.usuario_id, &caixa_nome, &d)?;
                tx.commit()?;
                Ok(json!(vendas::ficha(&self.banco.conn, &id)?))
            }
            "orcamento_carregar" => {
                let d: Id = ler(dados)?;
                Ok(json!(vendas::orcamento_para_venda(&self.banco.conn, &d.id)?))
            }
            "venda_abrir" => {
                let d: Id = ler(dados)?;
                Ok(json!(vendas::ficha(&self.banco.conn, &d.id)?))
            }
            "vendas_listar" => {
                let d: ListaVendas = ler(dados)?;
                Ok(json!(vendas::listar(&self.banco.conn, &d.tipo, &d.texto, &d.sessao, d.limite.unwrap_or(300))?))
            }
            "venda_cancelar" => {
                let d: Estorno = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                vendas::cancelar(&tx, &disp, &sessao.usuario_id, &d.id, &d.motivo)?;
                tx.commit()?;
                Ok(json!(vendas::ficha(&self.banco.conn, &d.id)?))
            }
            "produto_por_codigo" => {
                // Leitor de código de barras: só o código exato, e só se for
                // de uma peça. Duas ou mais com o mesmo código: a tela mostra
                // as peças e a pessoa escolhe (nunca entra uma sozinha).
                let d: BuscaProdutos = ler(dados)?;
                let lista = busca::pelo_codigo_exato(&self.banco.conn, &d.texto)?;
                // Peças com este código que ainda esperam a confirmação do
                // caixa que distribui as faixas: o leitor não as acha.
                let esperando = busca::esperando_pelo_codigo(&self.banco.conn, &d.texto)?;
                Ok(match lista.as_slice() {
                    [p] => json!({ "id": p.id, "repetidos": 1, "esperando": esperando }),
                    [] => json!({ "id": null, "repetidos": 0, "esperando": esperando }),
                    varias => {
                        json!({ "id": null, "repetidos": varias.len(), "opcoes": varias, "esperando": esperando })
                    }
                })
            }
            // O vale-troca aparece à parte no Receber (só quando há vale).
            "formas_pagamento" => Ok(json!(caixa::FORMAS
                .iter()
                .filter(|(f, ..)| *f != "vale")
                .map(|(f, n, ok)| json!({ "forma": f, "nome": n, "disponivel": ok }))
                .collect::<Vec<_>>())),

            // Oficina: ordens de serviço
            "os_listar" => {
                let d: Filtro = ler(dados)?;
                Ok(json!({
                    "itens": oficina::listar(&self.banco.conn, &d.quais, &d.texto)?,
                    "contagem": oficina::contagem(&self.banco.conn)?,
                }))
            }
            "os_abrir" => {
                let d: Id = ler(dados)?;
                Ok(json!(oficina::ficha(&self.banco.conn, &d.id)?))
            }
            "os_calcular" => {
                let d: oficina::Entrada = ler(dados)?;
                Ok(json!(oficina::calcular(&self.banco.conn, &d)?))
            }
            "os_salvar" => {
                let d: oficina::Entrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let id = oficina::salvar(&tx, &disp, &sessao.usuario_id, &d)?;
                tx.commit()?;
                Ok(json!(oficina::ficha(&self.banco.conn, &id)?))
            }
            "os_rascunho_salvar" => {
                let d: oficina::Entrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let id = oficina::salvar_rascunho(&tx, &disp, &sessao.usuario_id, &d)?;
                tx.commit()?;
                Ok(json!(oficina::ficha(&self.banco.conn, &id)?))
            }
            "os_rascunho_excluir" => {
                let d: Id = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                oficina::excluir_rascunho(&tx, &disp, &sessao.usuario_id, &d.id)?;
                tx.commit()?;
                Ok(Value::Null)
            }
            "os_rascunho_pode" => {
                let caixas = sync::bloqueios_de_rascunho(&self.banco.conn)?;
                Ok(json!({ "pode": caixas.is_empty(), "caixas": caixas }))
            }
            "caixas_conhecidos" => Ok(json!({
                "caixas": sync::caixas_conhecidos(&self.banco.conn)?,
                "prazoDias": sync::PRAZO_COMPATIBILIDADE_DIAS,
            })),
            "caixa_retirar" => {
                let d: CaixaDaLoja = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                sync::retirar_caixa(&tx, &d.dispositivo, &d.motivo, &sessao.nome)?;
                auditoria::registrar(
                    &tx,
                    &disp,
                    Some(&sessao.usuario_id),
                    "caixa.retirar",
                    "sync_par",
                    &d.dispositivo,
                    &[format!("Caixa retirado da loja · {}", texto::limpar(&d.motivo))],
                )?;
                tx.commit()?;
                Ok(json!(sync::caixas_conhecidos(&self.banco.conn)?))
            }
            "caixa_readmitir" => {
                let d: CaixaDaLoja = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                sync::readmitir_caixa(&tx, &d.dispositivo)?;
                auditoria::registrar(
                    &tx,
                    &disp,
                    Some(&sessao.usuario_id),
                    "caixa.readmitir",
                    "sync_par",
                    &d.dispositivo,
                    &["Caixa readmitido na loja".into()],
                )?;
                tx.commit()?;
                Ok(json!(sync::caixas_conhecidos(&self.banco.conn)?))
            }
            "pendencias_listar" => Ok(json!(sync::pendencias_da_troca(&self.banco.conn)?)),
            // Simultaneidade › Histórico (0.9.15): só leitura.
            "troca_historico" => Ok(json!(sync::historico_da_troca(&self.banco.conn, 200)?)),
            "auditoria_listar" => {
                let eu = config::ler(&self.banco.conn, "dispositivo_id")?.unwrap_or_default();
                let mut nomes: std::collections::HashMap<String, String> =
                    sync::caixas_conhecidos(&self.banco.conn)?.into_iter().map(|c| (c.dispositivo, c.nome)).collect();
                nomes.insert(eu, config::nome_caixa(&self.banco.conn)?);
                let lista: Vec<Value> = auditoria::recentes(&self.banco.conn, 300)?
                    .into_iter()
                    .map(|r| {
                        let caixa = nomes.get(&r.dispositivo).cloned().unwrap_or_default();
                        let mut v = json!(r);
                        v["caixa"] = json!(caixa);
                        v
                    })
                    .collect();
                Ok(json!(lista))
            }
            "pendencia_arquivar" => {
                let d: Estorno = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                sync::arquivar_pendencia(&tx, &d.id, &d.motivo, &sessao.usuario_id)?;
                auditoria::registrar(
                    &tx,
                    &disp,
                    Some(&sessao.usuario_id),
                    "pendencia.arquivar",
                    "sync_pendencia",
                    &d.id,
                    &[format!("Pendência da troca de dados arquivada · {}", texto::limpar(&d.motivo))],
                )?;
                tx.commit()?;
                Ok(json!(sync::pendencias_da_troca(&self.banco.conn)?))
            }
            "pendencias_reaplicar" => {
                let n = sync::reaplicar_pendencias(&mut self.banco.conn)?;
                Ok(json!({ "aplicadas": n, "pendencias": sync::pendencias_da_troca(&self.banco.conn)? }))
            }
            "os_situacao" => {
                let d: Situacao = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                oficina::mudar(&tx, &disp, &sessao.usuario_id, &d.id, &d.situacao)?;
                tx.commit()?;
                Ok(json!(oficina::ficha(&self.banco.conn, &d.id)?))
            }
            "os_cancelar" => {
                let d: Estorno = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                oficina::cancelar(&tx, &disp, &sessao.usuario_id, &d.id, &d.motivo)?;
                tx.commit()?;
                Ok(Value::Null)
            }
            "os_para_venda" => {
                let d: Id = ler(dados)?;
                Ok(json!(oficina::para_venda(&self.banco.conn, &d.id)?))
            }
            "os_placa" => {
                let d: Filtro = ler(dados)?;
                Ok(json!(oficina::por_placa(&self.banco.conn, &d.texto)?))
            }
            "mecanicos" => Ok(json!(self
                .banco
                .conn
                .prepare("SELECT id, nome FROM usuario WHERE ativo = 1 ORDER BY nome COLLATE NOCASE")?
                .query_map([], |l| Ok(json!({ "id": l.get::<_, String>(0)?, "nome": l.get::<_, String>(1)? })))?
                .collect::<Result<Vec<_>, _>>()?)),

            // Trocas e devoluções
            "devolucao_venda" => {
                let d: Id = ler(dados)?;
                Ok(json!(devolucoes::da_venda(&self.banco.conn, &d.id)?))
            }
            "devolucao_calcular" => {
                let d: devolucoes::Entrada = ler(dados)?;
                Ok(json!(devolucoes::calcular(&self.banco.conn, &d)?))
            }
            "devolucao_gravar" => {
                let d: devolucoes::Entrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let id = devolucoes::devolver(&tx, &disp, &sessao.usuario_id, &d)?;
                tx.commit()?;
                let conn = &self.banco.conn;
                let vale: Option<String> =
                    conn.query_row("SELECT vale_id FROM devolucao WHERE id = ?1", [&id], |l| l.get(0))?;
                Ok(json!({ "id": id, "valeId": vale, "venda": devolucoes::da_venda(conn, &d.venda_id)? }))
            }
            "devolucao_desfazer" => {
                let d: Estorno = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                devolucoes::desfazer(&tx, &disp, &sessao.usuario_id, &d.id, &d.motivo)?;
                tx.commit()?;
                Ok(Value::Null)
            }
            "devolucoes_listar" => {
                let d: Filtro = ler(dados)?;
                Ok(json!(devolucoes::listar(&self.banco.conn, &d.texto)?))
            }
            "vales_abertos" => {
                let d: Id = ler(dados)?;
                Ok(json!(devolucoes::vales_abertos(&self.banco.conn, &d.id)?))
            }

            // Caixa
            "caixa_estado" => {
                let s = caixa::sessao_aberta(&self.banco.conn, &disp)?;
                let resumo = match &s {
                    Some(s) => Some(caixa::resumo(&self.banco.conn, &s.id)?),
                    None => None,
                };
                Ok(json!({
                    "aberto": s.is_some(),
                    "resumo": resumo,
                    "caixa": config::nome_caixa(&self.banco.conn)?,
                    "historico": caixa::historico(&self.banco.conn, &disp, 30)?,
                }))
            }
            "caixa_abrir" => {
                let d: Valor = ler(dados)?;
                let nome = config::nome_caixa(&self.banco.conn)?;
                let tx = self.banco.conn.transaction()?;
                caixa::abrir(&tx, &disp, &sessao.usuario_id, &nome, &d.valor)?;
                tx.commit()?;
                Ok(Value::Null)
            }
            "caixa_movimentar" => {
                let d: Valor = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                caixa::movimentar_com_conta(
                    &tx,
                    &disp,
                    &sessao.usuario_id,
                    &d.tipo,
                    &d.valor,
                    &d.motivo,
                    Some(&d.conta),
                )?;
                tx.commit()?;
                Ok(Value::Null)
            }
            "caixa_fechar" => {
                let d: Valor = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let id = caixa::fechar(&tx, &disp, &sessao.usuario_id, &d.valor, &d.motivo)?;
                tx.commit()?;
                Ok(json!(caixa::resumo(&self.banco.conn, &id)?))
            }
            "caixa_resumo" => {
                let d: Id = ler(dados)?;
                Ok(json!(caixa::resumo(&self.banco.conn, &d.id)?))
            }

            // Crediário e contas a receber
            "avisos_do_dia" => Ok(json!(self.avisos_de(sessao)?)),
            "aviso_minimizar" => {
                let d: Value = ler(dados)?;
                let atuais = self.avisos_de(sessao)?;
                let tx = self.banco.conn.transaction()?;
                crate::avisos::minimizar(
                    &tx,
                    &atuais,
                    d["id"].as_str().unwrap_or_default(),
                    d["minimizado"].as_bool().unwrap_or(true),
                )?;
                tx.commit()?;
                Ok(json!(true))
            }
            "aviso_esconder" => {
                let d: Value = ler(dados)?;
                // A categoria da notificação que esta pessoa está vendo decide.
                let atuais = self.avisos_de(sessao)?;
                crate::avisos::esconder(&self.banco.conn, &atuais, d["id"].as_str().unwrap_or_default())?;
                Ok(json!(true))
            }
            "receber_painel" => {
                let d: BuscaProdutos = ler(dados)?;
                Ok(json!(receber::painel(&self.banco.conn, &d.texto)?))
            }
            "receber_cliente" => {
                let d: ContaCliente = ler(dados)?;
                Ok(json!(receber::cliente(&self.banco.conn, &d.id, d.todas)?))
            }
            "receber_calcular" => {
                let d: receber::ReceberEntrada = ler(dados)?;
                Ok(json!(receber::calcular(&self.banco.conn, &d)?))
            }
            "receber_baixar" => {
                let d: receber::ReceberEntrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let id = receber::receber(&tx, &disp, &sessao.usuario_id, &d)?;
                tx.commit()?;
                Ok(json!(receber::recibo(&self.banco.conn, &id)?))
            }
            "recebimento_estornar" => {
                let d: Estorno = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                receber::estornar(&tx, &disp, &sessao.usuario_id, &d.id, &d.motivo)?;
                tx.commit()?;
                Ok(Value::Null)
            }
            "receber_previa" => {
                let d: receber::LancamentoEntrada = ler(dados)?;
                let valor = numeros::ler_dinheiro(&d.valor).map_err(|m| Erro::campo("valor", m))?.unwrap_or(0);
                Ok(json!(receber::previa(&self.banco.conn, &d.cliente_id, valor, d.parcelas, &d.primeiro_vencimento)?))
            }
            "receber_lancar" => {
                let d: receber::LancamentoEntrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                receber::lancar(&tx, &disp, &sessao.usuario_id, &d)?;
                tx.commit()?;
                Ok(Value::Null)
            }
            "conta_cancelar" => {
                let d: Estorno = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                receber::cancelar_conta(&tx, &disp, &sessao.usuario_id, &d.id, &d.motivo)?;
                tx.commit()?;
                Ok(Value::Null)
            }
            "situacao_cliente" => {
                let d: Id = ler(dados)?;
                Ok(json!(receber::situacao(&self.banco.conn, &d.id)?))
            }

            // Financeiro: contas a pagar
            // Estoque: o que comprar, encomendas, mais vendidos e parados
            // Cadastros pelo XML das notas (da loja e de fornecedores)
            "importar_previa" => {
                let d: importar::Entrada = ler(dados)?;
                Ok(json!(importar::previa(&self.banco.conn, &d.arquivos)?))
            }
            // Navegação guiada (piloto 0.9.16): só leitura.
            "requisito_conferir" => {
                let d: Requisito = ler(dados)?;
                Ok(json!(requisitos::conferir(&self.banco.conn, &d.id)?))
            }
            "arquivos_conferir" => {
                let d: importar::Entrada = ler(dados)?;
                Ok(json!(requisitos::arquivos(&d.arquivos)))
            }
            "importar_cadastros" => {
                let d: importar::Entrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let r = importar::importar(&tx, &disp, &sessao.usuario_id, &d)?;
                tx.commit()?;
                Ok(json!(r))
            }

            // Painel: como a loja está indo
            "painel_resumo" => {
                // Lucro e custos só para quem pode ver os resultados (gerente,
                // ou ação liberada para todos na pílula Gerente).
                // Na tela do Painel vale a consulta dos resultados; o financeiro
                // é de outra tela.
                let pode = self.pode_ler("resultados", sessao)?;
                let financeiro = self.pode_sem_senha("financeiro", sessao)?;
                let mut v = json!(painel::resumo(&self.banco.conn)?);
                if !pode {
                    v["lucroMes"] = json!("");
                    v["margemMes"] = json!("");
                    v["semCusto"] = json!(0);
                }
                // D-04: o que a loja tem a pagar é do financeiro.
                if !financeiro {
                    v["pagar7Dias"] = json!("");
                    v["pagarVencido"] = json!("");
                }
                v["resultados"] = json!(pode);
                v["financeiro"] = json!(financeiro);
                Ok(v)
            }

            // DRE gerencial (docs/DRE-gerencial.md)
            "dre_apurar" => {
                let d: dre::Filtro = ler(dados)?;
                Ok(json!(dre::apurar_dre(&self.banco.conn, &d)?))
            }
            "dre_detalhe" => {
                let d: DreLinha = ler(dados)?;
                Ok(json!(dre::detalhe(&self.banco.conn, &d.filtro, &d.linha)?))
            }
            "dre_despesas" => {
                let d: DreLinha = ler(dados)?;
                Ok(json!(dre::despesas(&self.banco.conn, &d.filtro, &d.categoria)?))
            }
            "dre_indicadores" => {
                let d: DreLinha = ler(dados)?;
                Ok(json!(dre::indicadores(&self.banco.conn, &d.filtro, &d.busca, 10)?))
            }
            "dre_serie" => Ok(json!(dre::serie(&self.banco.conn)?)),
            "dre_resumo" => Ok(json!(dre::resumo_painel(&self.banco.conn)?)),
            "dre_classificacao" => Ok(json!(dre::classificacao(&self.banco.conn)?)),
            "dre_classificacao_salvar" => {
                let d: dre::ClassificacaoEntrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                dre::salvar_classificacao(&tx, &disp, &sessao.usuario_id, &d)?;
                tx.commit()?;
                Ok(json!(dre::classificacao(&self.banco.conn)?))
            }
            "dre_metas" => Ok(json!(dre::metas(&self.banco.conn)?)),
            "dre_meta_salvar" => {
                let d: dre::MetaEntrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                dre::salvar_meta(&tx, &disp, &sessao.usuario_id, &d)?;
                tx.commit()?;
                Ok(json!(dre::metas(&self.banco.conn)?))
            }
            "painel_clientes" => {
                let d: Dias90 = ler(dados)?;
                Ok(json!(painel::clientes(&self.banco.conn, d.dias.max(1))?))
            }
            "painel_fornecedores" => {
                let d: Dias90 = ler(dados)?;
                Ok(json!(painel::fornecedores(&self.banco.conn, d.dias.max(1))?))
            }
            "painel_pagamentos" => {
                let d: Dias90 = ler(dados)?;
                Ok(json!(painel::pagamentos(&self.banco.conn, d.dias.max(1))?))
            }

            "estoque_sugestao" => {
                let d: Filtro = ler(dados)?;
                Ok(json!(reposicao::sugestao(&self.banco.conn, &d.texto)?))
            }
            "estoque_dias" => {
                let d: Dias = ler(dados)?;
                // C8: os dias e o histórico, numa transação só.
                let tx = self.banco.conn.transaction()?;
                let n = reposicao::salvar_dias(&tx, &disp, &sessao.usuario_id, &d.qual, &d.dias)?;
                tx.commit()?;
                Ok(json!(n))
            }
            "estoque_mais_vendidos" => {
                let d: MaisVendidos = ler(dados)?;
                let pode = self.pode_sem_senha("resultados", sessao)?;
                let mut v = json!(reposicao::mais_vendidos(&self.banco.conn, d.dias, &d.ordem, &d.texto)?);
                // D-01: lucro e margem só para quem pode ver os resultados.
                if !pode {
                    v["lucro"] = json!("");
                    if let Some(itens) = v["itens"].as_array_mut() {
                        for it in itens {
                            it["lucro"] = json!("");
                            it["margem"] = json!("");
                        }
                    }
                }
                v["resultados"] = json!(pode);
                Ok(v)
            }
            "estoque_parados" => {
                let d: Filtro = ler(dados)?;
                let pode = self.pode_sem_senha("resultados", sessao)?;
                let mut v = json!(reposicao::parados(&self.banco.conn, &d.texto)?);
                // D-09: o valor parado é pelo custo (estoque × custo de cada peça).
                if !pode {
                    v["valor"] = json!("");
                    if let Some(itens) = v["itens"].as_array_mut() {
                        for it in itens {
                            it["valor"] = json!("");
                        }
                    }
                }
                v["resultados"] = json!(pode);
                Ok(v)
            }
            "encomendas_painel" => {
                let d: Filtro = ler(dados)?;
                Ok(json!(encomendas::painel(&self.banco.conn, &d.texto, &d.quais)?))
            }
            "encomenda_criar" => {
                let d: encomendas::EncomendaEntrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let id = encomendas::criar(&tx, &disp, &sessao.usuario_id, &d)?;
                tx.commit()?;
                Ok(json!(id))
            }
            "encomenda_pedir" => {
                let d: Desfazer = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                for id in d.ids.iter().chain(std::iter::once(&d.id)).filter(|i| !i.is_empty()) {
                    encomendas::pedir(&tx, &disp, &sessao.usuario_id, id, d.desfazer)?;
                }
                tx.commit()?;
                Ok(Value::Null)
            }
            "encomenda_chegou" => {
                let d: Id = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                encomendas::chegou(&tx, &disp, &sessao.usuario_id, &d.id)?;
                tx.commit()?;
                Ok(Value::Null)
            }
            "encomenda_entregar" => {
                let d: Id = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                encomendas::entregar(&tx, &disp, &sessao.usuario_id, &d.id)?;
                tx.commit()?;
                Ok(Value::Null)
            }
            "encomenda_cancelar" => {
                let d: Estorno = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                encomendas::cancelar(&tx, &disp, &sessao.usuario_id, &d.id, &d.motivo)?;
                tx.commit()?;
                Ok(Value::Null)
            }
            "encomenda_whatsapp" => {
                // RVI-01: grava (avisado_em) quando a encomenda chegou.
                let d: Id = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let url = encomendas::whatsapp(&tx, &d.id)?;
                tx.commit()?;
                Ok(json!({ "url": url }))
            }
            "encomenda_para_venda" => {
                let d: Id = ler(dados)?;
                Ok(json!(encomendas::para_venda(&self.banco.conn, &d.id)?))
            }

            "financeiro_resumo" => Ok(json!(financeiro::resumo(&self.banco.conn)?)),
            "pagar_painel" => {
                let d: Filtro = ler(dados)?;
                Ok(json!(pagar::painel(&self.banco.conn, &d.texto, &d.quais)?))
            }
            "pagar_ficha" => {
                let d: Id = ler(dados)?;
                Ok(json!(pagar::ficha(&self.banco.conn, &d.id)?))
            }
            "pagar_previa" => {
                let d: pagar::LancamentoEntrada = ler(dados)?;
                Ok(json!(pagar::previa_texto(&self.banco.conn, &d)?))
            }
            "pagar_lancar" => {
                let d: pagar::LancamentoEntrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let ids = pagar::lancar(&tx, &disp, &sessao.usuario_id, &d)?;
                tx.commit()?;
                Ok(json!(ids))
            }
            "pagar_alterar" => {
                let d: pagar::AlteracaoEntrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                pagar::alterar(&tx, &disp, &sessao.usuario_id, &d)?;
                tx.commit()?;
                Ok(json!(pagar::ficha(&self.banco.conn, &d.id)?))
            }
            "pagar_cancelar" => {
                let d: Estorno = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                pagar::cancelar(&tx, &disp, &sessao.usuario_id, &d.id, &d.motivo)?;
                tx.commit()?;
                Ok(Value::Null)
            }
            "pagar_pagar" => {
                let d: pagar::PagamentoEntrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                pagar::pagar(&tx, &disp, &sessao.usuario_id, &d)?;
                tx.commit()?;
                Ok(json!(pagar::ficha(&self.banco.conn, &d.conta_id)?))
            }
            "pagamento_estornar" => {
                let d: Estorno = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                pagar::estornar(&tx, &disp, &sessao.usuario_id, &d.id, &d.motivo)?;
                tx.commit()?;
                Ok(Value::Null)
            }
            "pagar_formas" => {
                let gaveta = caixa::sessao_aberta(&self.banco.conn, &disp)?;
                Ok(json!({
                    "formas": pagar::FORMAS.iter().map(|(f, n)| json!({ "forma": f, "nome": n })).collect::<Vec<_>>(),
                    "contas": financeiro::contas(&self.banco.conn, false)?,
                    "gaveta": gaveta.map(|s| s.caixa_nome),
                    "categorias": financeiro::categorias(&self.banco.conn, false)?,
                }))
            }

            // Financeiro: contas da loja e categorias
            "contas_loja" => {
                let d: Filtro = ler(dados)?;
                let mut v = json!(financeiro::contas(&self.banco.conn, d.quais == "todas")?);
                // D-02: o caixa usa só o nome (para escolher a conta); o saldo é do financeiro.
                if !self.pode_ler("financeiro", sessao)? {
                    if let Some(lista) = v.as_array_mut() {
                        for c in lista.iter_mut().filter_map(|c| c.as_object_mut()) {
                            c.remove("saldo");
                            c.remove("saldoInicial");
                        }
                    }
                }
                Ok(v)
            }
            "conta_loja_salvar" => {
                let d: financeiro::ContaEntrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let id = financeiro::salvar_conta(&tx, &disp, &sessao.usuario_id, &d)?;
                tx.commit()?;
                Ok(json!(id))
            }
            "conta_loja_extrato" => {
                let d: Id = ler(dados)?;
                Ok(json!(financeiro::extrato(&self.banco.conn, &d.id, 500)?))
            }
            "movimento_lancar" => {
                let d: financeiro::MovimentoEntrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                financeiro::lancar_movimento(&tx, &disp, &sessao.usuario_id, &d)?;
                tx.commit()?;
                Ok(Value::Null)
            }
            "movimento_estornar" => {
                let d: Estorno = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                financeiro::estornar_movimento(&tx, &disp, &sessao.usuario_id, &d.id, &d.motivo)?;
                tx.commit()?;
                Ok(Value::Null)
            }
            "transferir" => {
                let d: financeiro::TransferenciaEntrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                financeiro::transferir(&tx, &disp, &sessao.usuario_id, &d)?;
                tx.commit()?;
                Ok(Value::Null)
            }
            "conta_loja_conferir" => {
                let d: Conferencia = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let diferenca = financeiro::conferir(&tx, &disp, &sessao.usuario_id, &d.conta_id, &d.saldo)?;
                tx.commit()?;
                Ok(json!(numeros::escrever_dinheiro(diferenca)))
            }
            "categorias" => {
                let d: Filtro = ler(dados)?;
                Ok(json!(financeiro::categorias(&self.banco.conn, d.quais == "todas")?))
            }
            "categoria_salvar" => {
                let d: financeiro::CategoriaEntrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let id = financeiro::salvar_categoria(&tx, &d)?;
                tx.commit()?;
                Ok(json!(id))
            }
            "fluxo_caixa" => {
                let d: Periodo = ler(dados)?;
                Ok(json!(financeiro::fluxo(&self.banco.conn, &d.de, &d.ate, d.dias.unwrap_or(30))?))
            }

            // Cheques
            "cheques_painel" => {
                let d: Filtro = ler(dados)?;
                Ok(json!(cheques::painel(&self.banco.conn, &d.texto, &d.quais)?))
            }
            "cheques_previa" => {
                let d: ChequesPrevia = ler(dados)?;
                let valor = numeros::ler_dinheiro(&d.valor).map_err(|m| Erro::campo("valor", m))?.unwrap_or(0);
                Ok(json!(cheques::previa(&self.banco.conn, valor, &d.cheques)?))
            }
            "cheque_depositar" => {
                let d: ChequeAcao = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                cheques::depositar(&tx, &disp, &sessao.usuario_id, &d.id, &d.conta)?;
                tx.commit()?;
                Ok(Value::Null)
            }
            "cheque_repassar" => {
                let d: ChequeAcao = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                cheques::repassar(&tx, &disp, &sessao.usuario_id, &d.id, &d.motivo)?;
                tx.commit()?;
                Ok(Value::Null)
            }
            "cheque_devolver" => {
                let d: ChequeAcao = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                cheques::devolver(&tx, &disp, &sessao.usuario_id, &d.id, &d.motivo, d.divida)?;
                tx.commit()?;
                Ok(Value::Null)
            }
            "cheque_desfazer" => {
                let d: ChequeAcao = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                cheques::desfazer(&tx, &disp, &sessao.usuario_id, &d.id)?;
                tx.commit()?;
                Ok(Value::Null)
            }

            // Comissões
            "comissoes" => {
                let d: Periodo = ler(dados)?;
                Ok(json!(comissoes::relatorio(&self.banco.conn, &d.de, &d.ate)?))
            }
            "comissao_fechar" => {
                let d: Periodo = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                comissoes::fechar(&tx, &disp, &sessao.usuario_id, &d.usuario_id, &d.de, &d.ate, &d.vencimento)?;
                tx.commit()?;
                Ok(json!(comissoes::relatorio(&self.banco.conn, &d.de, &d.ate)?))
            }
            "comissao_reabrir" => {
                let d: Id = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                comissoes::reabrir(&tx, &disp, &sessao.usuario_id, &d.id)?;
                tx.commit()?;
                Ok(Value::Null)
            }

            // Impressão (o PDF é feito na janela)
            "etiquetas_opcoes" => Ok(etiquetas::opcoes(&self.banco.conn)?),
            "etiqueta_previa" => {
                let d: Id = ler(dados)?;
                Ok(json!(etiquetas::etiqueta(&self.banco.conn, &d.id)?))
            }
            "etiquetas_da_compra" => {
                let d: Id = ler(dados)?;
                let conn = &self.banco.conn;
                let mut c = conn.prepare(
                    "SELECT produto_id, SUM((quantidade * fator + 500000) / 1000000) FROM compra_item
                      WHERE compra_id = ?1 AND produto_id IS NOT NULL AND acao <> 'ignorar' GROUP BY produto_id ORDER BY MIN(ordem)",
                )?;
                let itens: Vec<(String, i64)> =
                    c.query_map([&d.id], |l| Ok((l.get(0)?, l.get(1)?)))?.collect::<Result<_, _>>()?;
                let mut lista = Vec::new();
                for (pid, q) in itens {
                    let e = etiquetas::etiqueta(conn, &pid)?;
                    lista.push(json!({ "etiqueta": e, "quantidade": q.max(1) }));
                }
                Ok(json!(lista))
            }

            "impressao" => {
                let d: Impressao = ler(dados)?;
                let pode_resultados = d.tipo == "dre" && self.pode_ler("resultados", sessao)?;
                let conn = &self.banco.conn;
                let pasta = match &self.banco.caminho {
                    Some(c) => c.parent().map(|p| p.join("impressoes")).unwrap_or_default(),
                    None => std::env::temp_dir().join("rustinho-impressoes"),
                };
                if d.tipo == "etiquetas" {
                    let pedido: etiquetas::Pedido = serde_json::from_str(&d.id)?;
                    let folhas = etiquetas::montar(conn, &pedido)?;
                    return Ok(json!({
                        "etiquetas": folhas,
                        "caminho": pasta.join(format!("etiquetas-{}.pdf", tempo_curto())).display().to_string(),
                        "direto": config::ler(conn, "imprimir_direto")?.as_deref() == Some("1"),
                    }));
                }
                let (html, nome) = match d.tipo.as_str() {
                    "venda" => {
                        let v = vendas::ficha(conn, &d.id)?;
                        (impressao::venda(conn, &d.id)?, format!("venda-{}-{}", v.numero, texto::compactar(&v.caixa)))
                    }
                    "orcamento" => {
                        let v = vendas::ficha(conn, &d.id)?;
                        (
                            impressao::orcamento(conn, &d.id)?,
                            format!("orcamento-{}-{}", v.numero, texto::compactar(&v.caixa)),
                        )
                    }
                    "caixa" => (impressao::caixa(conn, &d.id)?, format!("caixa-{}", &d.id[..8.min(d.id.len())])),
                    "recibo" => {
                        let r = receber::recibo(conn, &d.id)?;
                        (impressao::recibo(conn, &d.id)?, format!("recibo-{}-{}", r.numero, texto::compactar(&r.caixa)))
                    }
                    "extrato" => (
                        impressao::extrato(conn, &d.id)?,
                        format!("extrato-{}", texto::compactar(&pessoas::ficha(conn, &d.id)?.nome)),
                    ),
                    "dre" => {
                        if !pode_resultados {
                            return Err(Erro::campo(
                                "gerente",
                                "Só o gerente pode: resultados. Peça para o gerente digitar a senha.",
                            ));
                        }
                        let f: dre::Filtro =
                            if d.id.trim().is_empty() { dre::Filtro::default() } else { serde_json::from_str(&d.id)? };
                        (impressao::dre(conn, &f)?, format!("dre-gerencial-{}", fiscal::hoje(conn)?))
                    }
                    "lista_compra" => {
                        (impressao::lista_compra(conn)?, format!("lista-de-compra-{}", fiscal::hoje(conn)?))
                    }
                    "os" => {
                        let n: (i64, String) = conn.query_row(
                            "SELECT numero, caixa_nome FROM ordem_servico WHERE id = ?1",
                            [&d.id],
                            |l| Ok((l.get(0)?, l.get(1)?)),
                        )?;
                        (impressao::ordem_servico(conn, &d.id)?, format!("os-{}-{}", n.0, texto::compactar(&n.1)))
                    }
                    "devolucao" => {
                        let n: (i64, String) =
                            conn.query_row("SELECT numero, caixa_nome FROM devolucao WHERE id = ?1", [&d.id], |l| {
                                Ok((l.get(0)?, l.get(1)?))
                            })?;
                        (impressao::devolucao(conn, &d.id)?, format!("devolucao-{}-{}", n.0, texto::compactar(&n.1)))
                    }
                    "vale" => {
                        let n: (i64, String) =
                            conn.query_row("SELECT numero, caixa_nome FROM vale WHERE id = ?1", [&d.id], |l| {
                                Ok((l.get(0)?, l.get(1)?))
                            })?;
                        (impressao::vale(conn, &d.id)?, format!("vale-troca-{}-{}", n.0, texto::compactar(&n.1)))
                    }
                    _ => return Err(Erro::Interno("impresso desconhecido".into())),
                };
                Ok(json!({
                    "html": html,
                    "caminho": pasta.join(format!("{nome}.pdf")).display().to_string(),
                    "direto": config::ler(conn, "imprimir_direto")?.as_deref() == Some("1"),
                }))
            }

            // Sincronização
            "sync_salvar" => {
                let d: AjustesSync = ler(dados)?;
                // FALHA-01 / A2: ligar a troca de dados ou trocar a senha da
                // rede é sempre do gerente, com a rede ligada ou desligada (o
                // primeiro caixa liga sozinho, com o gerente dele). Só os
                // endereços continuam em Configurações. Campo vazio (ou só
                // espaços) = manter a senha gravada.
                let atual = sync::ajustes(&self.banco.conn)?;
                let entrada = d.senha.trim();
                let senha_nova = !entrada.is_empty() && entrada != atual.senha;
                let muda_rede = (d.ligado && !atual.ligado) || senha_nova;
                if muda_rede && !sessao.gerente && !self.liberada_esta_chamada(sessao) {
                    return Err(Erro::campo(
                        "gerente",
                        "Só o gerente pode ligar a troca de dados ou trocar a senha da rede. Peça para o gerente digitar a senha.",
                    ));
                }
                let senha = if entrada.is_empty() { atual.senha.clone() } else { entrada.to_string() };
                // 0915-K: tudo numa transação (endereço inválido não deixa as
                // interfaces gravadas pela metade).
                let tx = self.banco.conn.transaction()?;
                if senha_nova {
                    crate::loja::esquecer_recusa(&tx)?;
                }
                if let Some(nomes) = &d.interfaces {
                    sync::salvar_interfaces(&tx, nomes)?;
                }
                sync::salvar_ajustes(&tx, d.ligado, &senha, &d.enderecos)?;
                tx.commit()?;
                self.sync_ler()
            }

            // Backup
            "backup_ler" => backup::resumo(&self.banco.conn, self.banco.caminho.as_deref(), true),
            "backup_salvar" => {
                let d: backup::AjustesEntrada = ler(dados)?;
                backup::salvar_ajustes(&self.banco.conn, self.banco.caminho.as_deref(), &d)?;
                backup::resumo(&self.banco.conn, self.banco.caminho.as_deref(), true)
            }
            "backup_agora" => {
                backup::agora(self.caminho_banco()?)?;
                backup::resumo(&self.banco.conn, self.banco.caminho.as_deref(), false)
            }
            "backup_conectar_drive" => {
                backup::conectar_drive(self.caminho_banco()?)?;
                Ok(Value::Null)
            }
            "backup_desligar_drive" => {
                backup::desligar_drive(&self.banco.conn)?;
                backup::resumo(&self.banco.conn, self.banco.caminho.as_deref(), true)
            }
            "backup_nuvem_lista" => Ok(json!(backup::listar_nuvem()?)),
            "backup_conferir" => {
                let d: Restaurar = ler(dados)?;
                Ok(json!(backup::conferir(&self.banco.conn, std::path::Path::new(&d.arquivo), &d.senha)?))
            }
            "backup_restaurar" => {
                let d: Restaurar = ler(dados)?;
                let caminho = self.banco.caminho.clone();
                // A3: no modo seguro, restaurar é a saída. Só para isso o banco
                // deixa de ser só leitura (a cópia de antes sai como cópia para
                // o suporte); depois volta a ser, até fechar e abrir de novo,
                // quando o banco restaurado é conferido.
                let seguro = self.banco.em_modo_seguro();
                if seguro {
                    self.banco.conn.pragma_update(None, "query_only", false)?;
                }
                let r = backup::restaurar(&mut self.banco.conn, caminho.as_deref(), &d.arquivo, d.nuvem, &d.senha)
                    .and_then(|info| {
                        auditoria::registrar(
                            &self.banco.conn,
                            &disp,
                            Some(&sessao.usuario_id),
                            "backup.restaurar",
                            "config",
                            &info.quando,
                            &[],
                        )?;
                        Ok(info)
                    });
                let conferido = seguro && r.is_ok() && crate::banco::conferir_integridade(&self.banco.conn).is_ok();
                if seguro {
                    self.banco.conn.pragma_update(None, "query_only", true)?;
                }
                let info = r?;
                // Os usuários podem ser outros agora: entra de novo.
                self.sessao = None;
                let mut v = json!(info);
                if seguro {
                    v["reabrir"] = json!(true);
                    v["conferido"] = json!(conferido);
                }
                Ok(v)
            }

            "config_ler" => {
                let ve = self.ve_custos(sessao)?;
                self.config_ler(ve)
            }
            "config_salvar" => {
                let mut d: config::ConfigEntrada = ler(dados)?;
                // D-09: o lucro sugerido das tabelas é regra de preço.
                let ve = self.ve_custos(sessao)?;
                if !ve {
                    config::manter_lucro(&self.banco.conn, &mut d)?;
                }
                let regras = config::mudancas_no_lucro(&self.banco.conn, &d)?;
                if !regras.is_empty() {
                    self.exigir("precos", sessao)?;
                }
                let tx = self.banco.conn.transaction()?;
                config::salvar(&tx, &d)?;
                receber::salvar_taxas(&tx, &d.crediario_multa, &d.crediario_juros, &d.crediario_carencia)?;
                if !regras.is_empty() {
                    auditoria::registrar(
                        &tx,
                        &disp,
                        Some(&sessao.usuario_id),
                        "precos.regras",
                        "config",
                        "tabela_preco",
                        &regras,
                    )?;
                }
                tx.commit()?;
                self.config_ler(ve)
            }

            _ => Err(Erro::Interno(format!("comando desconhecido: {comando}"))),
        }
    }

    fn caminho_banco(&self) -> Resultado<std::path::PathBuf> {
        self.banco.caminho.clone().ok_or_else(|| Erro::validacao("Backup só funciona com o banco num arquivo."))
    }

    fn sync_ler(&self) -> Resultado<Value> {
        let conn = &self.banco.conn;
        let a = sync::ajustes(conn)?;
        // O-3 / RVI-02: tarefas da troca de dados que não estão rodando.
        let situacao = self.banco.caminho.as_deref().map(crate::supervisao::situacao).unwrap_or_default();
        Ok(json!({
            "ligado": a.ligado,
            "temSenha": !a.senha.is_empty(),
            "senhaFraca": sync::senha_fora_da_politica(conn)?,
            "enderecos": a.enderecos.join(", "),
            "interfaces": a.interfaces.join(", "),
            "interfacesIgnoradas": sync::interfaces_ignoradas(&a.interfaces),
            "caixa": a.nome,
            "pares": sync::pares(conn)?,
            "meuEndereco": endereco_local(),
            "porta": crate::portatil::portas().0,
            "usuarios": usuarios::quantidade(conn)?,
            "pendencias": config::ler(conn, "sync_pendencias")?.and_then(|t| t.parse::<i64>().ok()).unwrap_or(0),
            "quebradas": config::ler(conn, "sync_quebradas")?.and_then(|t| t.parse::<i64>().ok()).unwrap_or(0),
            "atrasadas": config::ler(conn, "sync_atrasadas")?.and_then(|t| t.parse::<i64>().ok()).unwrap_or(0),
            "referencias": sync::referencias(conn)?,
            // O-3: tarefas da troca de dados que caíram (a tela não diz que
            // está tudo bem com o motor parado).
            "tarefasParadas": situacao.paradas.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
            // RVI-02: o que não está rodando agora, com o motivo.
            "tarefasComFalha": situacao
                .paradas
                .iter()
                .map(|t| (t, "parada"))
                .chain(situacao.tentando.iter().map(|t| (t, "tentando")))
                .map(|((n, motivo), qual)| json!({
                    "tarefa": crate::supervisao::nome_para_tela(n),
                    "situacao": qual,
                    "motivo": motivo,
                }))
                .collect::<Vec<_>>(),
        }))
    }

    fn estado(&self) -> Resultado<Value> {
        let conn = &self.banco.conn;
        Ok(json!({
            "primeiraVez": usuarios::quantidade(conn)? == 0,
            // FALHA-01: no meio de uma entrada numa loja (a tela volta para ela).
            "juntando": crate::loja::juntando(conn)?,
            "loja": config::nome_loja(conn)?,
            "caixa": config::nome_caixa(conn)?,
            "usuario": self.sessao.as_ref().map(|s| json!({ "id": s.usuario_id, "nome": s.nome, "gerente": s.gerente })),
            "versao": VERSAO,
            "etapa": menu::ETAPA_ATUAL,
            "validadeOrcamento": config::validade_orcamento(conn)?,
            "escuro": config::ler(conn, "tela_escura")?.as_deref() == Some("1"),
            // A3: banco com defeito de integridade: modo seguro (o defeito; null = normal).
            "modoSeguro": self.banco.defeito,
        }))
    }

    fn configurar_inicio(&mut self, d: Inicio) -> Resultado<Value> {
        if usuarios::quantidade(&self.banco.conn)? > 0 {
            return Err(Erro::validacao("O Rustinho já foi configurado neste computador."));
        }
        // FALHA-01 (V01a): uma loja nova não nasce em cima de uma entrada pela
        // metade numa loja que já existe (troca ligada, senha da rede, dados
        // de outro caixa). Primeiro desiste dela.
        if crate::loja::juntando(&self.banco.conn)? {
            return Err(Erro::campo(
                "juntar",
                "Este computador começou a entrar numa loja que já existe. Para criar uma loja nova, volte e toque \
                 em \"Voltar\" na tela de entrar na loja (isso desfaz a tentativa).",
            ));
        }
        let loja = texto::limpar(&d.loja);
        let caixa = texto::limpar(&d.caixa);
        if texto::tamanho(&loja) > 80 {
            return Err(Erro::campo("loja", "O nome da loja pode ter no máximo 80 letras."));
        }
        config::validar_nome_caixa(&caixa)?;
        if d.senha != d.senha2 {
            return Err(Erro::campo("senha2", "As duas senhas não são iguais."));
        }
        let disp = self.banco.dispositivo.clone();
        let tx = self.banco.conn.transaction()?;
        config::gravar(&tx, "loja_nome", &loja)?;
        config::gravar(&tx, "caixa_nome", &caixa)?;
        // Quem configura a loja é gerente; a regra é a mesma para todos (pelo menos 4 caracteres).
        usuarios::exigir_politica(&tx, &d.senha, &d.nome, true, "senha")?;
        let u = usuarios::criar(&tx, &disp, &d.nome, &d.senha, true)?;
        // Quem configura a loja é o gerente.
        usuarios::definir_gerente(&tx, &u.id, true)?;
        // Loja nova: este caixa distribui as faixas de códigos das peças e
        // fica com a primeira (1 a 99.999).
        faixas::comecar_loja(&tx)?;
        // Identidade da loja (FALHA-01): vai para os caixas que entrarem nela.
        crate::loja::criar(&tx)?;
        self.segredo_backup = backup::gerente_conferido(&tx, &u.id, &d.senha, self.segredo_backup)?;
        auditoria::registrar(&tx, &disp, Some(&u.id), "usuario.criar", "usuario", &u.id, &[])?;
        usuarios::registrar_acesso(&tx, &u.id)?;
        tx.commit()?;
        self.liberacoes.clear();
        self.sessao = Some(Sessao { usuario_id: u.id, nome: u.nome, gerente: true });
        self.estado()
    }

    /// Depois de muitas senhas erradas seguidas, espera um pouco (fica
    /// guardado no banco: fechar e abrir o programa não zera).
    fn esperar_se_errou(&self, id: &str) -> Resultado<()> {
        if let Some((quantidade, passou)) = usuarios::falhas(&self.banco.conn, id)? {
            if quantidade >= TENTATIVAS_MAXIMAS && passou < ESPERA_APOS_ERROS {
                let faltam = ESPERA_APOS_ERROS - passou;
                return Err(Erro::campo(
                    "senha",
                    format!("Muitas senhas erradas. Espere {faltam} segundos e tente de novo."),
                ));
            }
        }
        Ok(())
    }

    fn entrar(&mut self, d: Entrada) -> Resultado<Value> {
        self.esperar_se_errou(&d.id)?;
        match usuarios::conferir_senha(&self.banco.conn, &d.id, &d.senha) {
            Ok(u) => {
                self.gravar_se_der(usuarios::limpar_falhas(&self.banco.conn, &d.id))?;
                self.gravar_se_der(usuarios::registrar_acesso(&self.banco.conn, &u.id))?;
                // F-17: senha antiga fora da política só gera aviso e marcador;
                // nunca impede a entrada nem muda nada na senha ou nos backups.
                if let Ok(a) = usuarios::avaliar_no_login(&self.banco.conn, &u, &d.senha) {
                    if let (true, Some(p)) = (a.nova, a.problema) {
                        let _ = auditoria::registrar(
                            &self.banco.conn,
                            &self.banco.dispositivo,
                            Some(&u.id),
                            "usuario.senha_fora_da_politica",
                            "usuario",
                            &u.id,
                            &[format!("Senha fora da política: {}", p.resumo())],
                        );
                    }
                }
                if u.gerente {
                    self.segredo_backup = self.segredo_conferido(&u.id, &d.senha)?;
                }
                self.gravar_se_der(auditoria::registrar(
                    &self.banco.conn,
                    &self.banco.dispositivo,
                    Some(&u.id),
                    "entrar",
                    "usuario",
                    &u.id,
                    &[],
                ))?;
                self.liberacoes.clear();
                self.sessao = Some(Sessao { usuario_id: u.id, nome: u.nome, gerente: u.gerente });
                self.estado()
            }
            Err(e) => {
                if e.nome_do_campo() == Some("senha") {
                    self.gravar_se_der(usuarios::anotar_falha(&self.banco.conn, &d.id, ESPERA_APOS_ERROS))?;
                }
                Err(e)
            }
        }
    }

    /// O segredo dos backups com a senha de um gerente. No modo seguro, sem
    /// gravar nada: se precisaria gravar (cadeado novo), fica o que já havia.
    fn segredo_conferido(&self, usuario_id: &str, senha: &str) -> Resultado<Option<[u8; 32]>> {
        match backup::gerente_conferido(&self.banco.conn, usuario_id, senha, self.segredo_backup) {
            Err(e) if self.banco.em_modo_seguro() && crate::banco::erro_de_somente_leitura(&e) => {
                Ok(self.segredo_backup)
            }
            outro => outro,
        }
    }

    /// Nome e papel podem ter mudado (por este ou por outro caixa).
    fn atualizar_sessao(&mut self) -> Resultado<()> {
        if let Some(s) = self.sessao.as_mut() {
            let u = usuarios::buscar(&self.banco.conn, &s.usuario_id)?;
            // D-07: desativado (aqui ou em outro caixa): a sessão acaba na hora.
            if !u.ativo {
                self.sessao = None;
                self.liberacoes.clear();
                return Err(Erro::campo("sessao", "Seu usuário foi desativado. Fale com o gerente."));
            }
            s.nome = u.nome;
            s.gerente = u.gerente;
        }
        Ok(())
    }

    fn gerente_ler(&self) -> Resultado<Value> {
        let conn = &self.banco.conn;
        let livres = acoes_livres(conn)?;
        Ok(json!({
            "acoes": ACOES_DO_GERENTE
                .iter()
                .map(|(id, nome)| json!({ "id": id, "nome": nome, "soGerente": !livres.iter().any(|a| a == id) }))
                .collect::<Vec<_>>(),
            "usuarios": usuarios::listar(conn, true)?
                .into_iter()
                .map(|u| json!({ "id": u.id, "nome": u.nome, "gerente": u.gerente }))
                .collect::<Vec<_>>(),
        }))
    }

    fn usuario_salvar(&mut self, d: UsuarioEntrada, sessao: &Sessao) -> Resultado<Value> {
        let disp = self.banco.dispositivo.clone();
        // Mexer na conta de um gerente (ou criar um) é sempre do gerente,
        // mesmo com "Usuários" liberado para todos.
        let mexe_em_gerente = d.gerente == Some(true)
            || (!d.id.is_empty() && d.id != sessao.usuario_id && usuarios::buscar(&self.banco.conn, &d.id)?.gerente);
        if mexe_em_gerente && !sessao.gerente && !self.liberada_esta_chamada(sessao) {
            return Err(Erro::campo("gerente", "Só o gerente pode mexer na conta de um gerente."));
        }
        let tem_senha = !d.senha.is_empty() || !d.senha2.is_empty();
        if tem_senha && d.senha != d.senha2 {
            return Err(Erro::campo("senha2", "As duas senhas não são iguais."));
        }
        let tx = self.banco.conn.transaction()?;
        let u = if d.id.is_empty() {
            if !tem_senha {
                return Err(Erro::campo("senha", "Crie uma senha para o novo usuário."));
            }
            usuarios::exigir_politica(&tx, &d.senha, &d.nome, d.gerente == Some(true), "senha")?;
            let u = usuarios::criar(&tx, &disp, &d.nome, &d.senha, d.ativo)?;
            usuarios::definir_comissao(&tx, &u.id, &d.comissao)?;
            if d.gerente == Some(true) {
                usuarios::definir_gerente(&tx, &u.id, true)?;
                backup::senha_trocada(&tx, &u.id, &d.senha, self.segredo_backup)?;
            }
            auditoria::registrar(&tx, &disp, Some(&sessao.usuario_id), "usuario.criar", "usuario", &u.id, &[])?;
            usuarios::buscar(&tx, &u.id)?
        } else {
            let antes = usuarios::buscar(&tx, &d.id)?;
            let senha = if tem_senha { Some(d.senha.as_str()) } else { None };
            if tem_senha {
                // A regra é a do papel que o usuário terá depois de salvar.
                let sera_gerente = d.gerente.unwrap_or(antes.gerente);
                usuarios::exigir_politica(&tx, &d.senha, &d.nome, sera_gerente, "senha")?;
            }
            let u = usuarios::alterar(&tx, &d.id, &d.nome, d.ativo, senha)?;
            let mut detalhes = Vec::new();
            if antes.nome != u.nome {
                detalhes.push(format!("Nome: {} → {}", antes.nome, u.nome));
            }
            if antes.ativo != u.ativo {
                detalhes.push(if u.ativo { "Reativado".into() } else { "Desativado".into() });
            }
            if tem_senha {
                detalhes.push("Senha trocada".into());
            }
            if let Some((a, b)) = usuarios::definir_comissao(&tx, &u.id, &d.comissao)? {
                detalhes.push(format!("Comissão: {a} → {b}"));
            }
            if let Some(g) = d.gerente {
                if usuarios::definir_gerente(&tx, &u.id, g)? {
                    detalhes.push(if g { "Agora é gerente".into() } else { "Agora é caixa".into() });
                }
            }
            let u = usuarios::buscar(&tx, &u.id)?;
            // Chave do backup: senha nova de gerente passa a abrir; quem deixou
            // de ser gerente (ou foi desativado) não abre mais.
            if !u.gerente || !u.ativo {
                backup::deixou_de_ser_gerente(&tx, &u.id)?;
            } else if tem_senha {
                backup::senha_trocada(&tx, &u.id, &d.senha, self.segredo_backup)?;
            }
            if !detalhes.is_empty() {
                auditoria::registrar(
                    &tx,
                    &disp,
                    Some(&sessao.usuario_id),
                    "usuario.alterar",
                    "usuario",
                    &u.id,
                    &detalhes,
                )?;
            }
            u
        };
        tx.commit()?;
        self.atualizar_sessao()?;
        Ok(json!(usuarios::buscar(&self.banco.conn, &u.id)?))
    }

    /// Pode ver custo e % de lucro das peças (D-09): quem vê os resultados.
    /// Na consulta de uma tela de resultados com a senha do gerente, o
    /// cadastro não abre (é outra tela).
    fn ve_custos(&mut self, sessao: &Sessao) -> Resultado<bool> {
        self.pode_sem_senha("resultados", sessao)
    }

    /// Ficha da peça para a tela: sem custo, % de lucro e histórico do
    /// custo para quem não vê os resultados (D-09).
    fn peca_para_tela(&mut self, mut f: produtos::ProdutoFicha, sessao: &Sessao) -> Resultado<Value> {
        f.ver_custos = self.ve_custos(sessao)?;
        if !f.ver_custos {
            produtos::esconder_custos(&mut f);
        }
        Ok(json!(f))
    }

    /// As notificações do Início para quem está conectado (as mesmas que a
    /// tela mostra e que `aviso_esconder` confere).
    fn avisos_de(&self, sessao: &Sessao) -> Resultado<Vec<crate::avisos::Aviso>> {
        let mut avisos = match crate::avisos::do_dia(&self.banco.conn) {
            Ok(a) => a,
            // RVI-03: banco antigo em modo seguro (estrutura sem atualizar):
            // pelo menos o aviso do modo seguro.
            Err(e) if self.banco.em_modo_seguro() => {
                crate::registro::anotar(&format!("avisos no modo seguro: {e}"));
                crate::avisos::so_do_modo_seguro(&self.banco.conn, self.banco.defeito.as_deref().unwrap_or_default())
            }
            Err(e) => return Err(e),
        };
        // D-03: os avisos de contas a pagar e cheques têm valores do
        // financeiro: só para quem pode ver o financeiro.
        if !self.pode_sem_senha("financeiro", sessao)? {
            avisos.retain(|a| a.funcao != "financeiro");
        }
        if let Some(a) = crate::avisos::da_senha(&self.banco.conn, &sessao.usuario_id)? {
            avisos.push(a);
        }
        crate::avisos::uma_aberta(&mut avisos);
        Ok(avisos)
    }

    fn config_ler(&self, ve_custos: bool) -> Resultado<Value> {
        let conn = &self.banco.conn;
        let taxas = receber::taxas_texto(conn)?;
        let mut tabelas = config::tabelas_preco(conn)?;
        if !ve_custos {
            for t in &mut tabelas {
                t.lucro_padrao = String::new();
                t.lucro_padrao_valor = None;
            }
        }
        Ok(json!({
            "loja": config::nome_loja(conn)?,
            "caixa": config::nome_caixa(conn)?,
            "tabelas": tabelas,
            "verLucro": ve_custos,
            "uf": config::ler(conn, "loja_uf")?.unwrap_or_default(),
            "cnpj": documento::escrever(&config::ler(conn, "loja_cnpj")?.unwrap_or_default()),
            "ufs": documento::UFS,
            "endereco": config::ler(conn, "loja_endereco")?.unwrap_or_default(),
            "telefone": config::ler(conn, "loja_telefone")?.unwrap_or_default(),
            "imprimirDireto": config::ler(conn, "imprimir_direto")?.as_deref() == Some("1"),
            "crediarioMulta": taxas.0,
            "crediarioJuros": taxas.1,
            "crediarioCarencia": taxas.2,
            "orcamentoValidade": config::validade_orcamento(conn)?.to_string(),
            "rodapeVenda": config::ler(conn, "rodape_venda")?.unwrap_or_default(),
            "rodapeOrcamento": config::ler(conn, "rodape_orcamento")?.unwrap_or_default(),
            "rodapeOs": config::ler(conn, "rodape_os")?.unwrap_or_default(),
            "imprimirLocal": config::ligado(conn, "imprimir_local")?,
            "dispositivo": self.banco.dispositivo,
            "dados": self.banco.caminho.as_ref().map(|c| c.display().to_string()).unwrap_or_default(),
            "versao": VERSAO,
            "pecas": busca::total(conn, true)?,
        }))
    }
}

#[derive(Deserialize)]
struct Requisito {
    #[serde(default)]
    id: String,
}

#[derive(Deserialize)]
struct Acao {
    acao: String,
    #[serde(default)]
    consulta: bool,
}

#[derive(Deserialize)]
struct AceitarReferencia {
    tabela: String,
    chave: String,
    motivo: String,
}

#[derive(Deserialize)]
struct Resolver {
    id: i64,
    #[serde(default)]
    usar: bool,
}

#[derive(Deserialize)]
struct ConferenciaDoGerente {
    #[serde(flatten)]
    conferencia: compras::Conferencia,
    senha: String,
}

#[derive(Deserialize)]
struct SenhaGerente {
    senha: String,
    /// Consultar a tela desta ação (só leitura).
    #[serde(default)]
    acao: String,
    /// Ou de várias ações de uma vez (Simultaneidade), também só leitura.
    #[serde(default)]
    acoes: Vec<String>,
    /// Ou: fazer esta gravação, com estes dados exatos (uma vez).
    #[serde(default)]
    comando: String,
    #[serde(default)]
    dados: Option<Value>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct GerenteEntrada {
    /// {"cancelar_venda": true, ...}: true = só o gerente.
    acoes: serde_json::Map<String, Value>,
    /// {"<id do usuário>": true}: true = gerente.
    gerentes: serde_json::Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Computador {
    caixa: String,
    imprimir_direto: bool,
}

/// Ações que a loja liberou para todos (as outras são só do gerente).
fn acoes_livres(conn: &rusqlite::Connection) -> Resultado<Vec<String>> {
    Ok(config::ler(conn, "gerente_livres")?
        .unwrap_or_default()
        .split(',')
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty())
        .collect())
}

#[derive(Deserialize)]
struct Aparencia {
    escuro: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Restaurar {
    arquivo: String,
    #[serde(default)]
    nuvem: bool,
    #[serde(default)]
    senha: String,
}

/// Data e hora para nome de arquivo ("20261015-153012").
fn tempo_curto() -> String {
    crate::tempo::agora().chars().filter(|c| c.is_ascii_digit()).take(14).collect::<String>()
}

/// Reescreve um número digitado no formato padrão ("85" → "85,00").
/// Se não for um número válido, devolve o texto como veio.
fn formatar(tipo: &str, texto: &str) -> String {
    let resultado = match tipo {
        "dinheiro" => numeros::ler_dinheiro(texto).map(|v| v.map(numeros::escrever_dinheiro)),
        "percentual" => numeros::ler_percentual(texto).map(|v| v.map(numeros::escrever_percentual)),
        "quantidade" => numeros::ler_quantidade(texto).map(|v| v.map(numeros::escrever_quantidade)),
        _ => Ok(None),
    };
    match resultado {
        Ok(Some(t)) => t,
        _ => texto.to_string(),
    }
}

/// Converte os dados JSON no tipo esperado pelo comando.
/// Uma linha (ou categoria, ou busca) da DRE, com o período.
#[derive(Deserialize, Default)]
#[serde(default)]
struct DreLinha {
    #[serde(flatten)]
    filtro: dre::Filtro,
    linha: String,
    categoria: String,
    busca: String,
}

fn ler<T: DeserializeOwned>(dados: Value) -> Resultado<T> {
    Ok(serde_json::from_value(dados)?)
}

/// Troca o nome do campo de um erro (ex.: "senha" → "nova").
fn renomear_campo(de: &'static str, para: &'static str) -> impl Fn(Erro) -> Erro {
    move |e| match e {
        Erro::Validacao { campo: Some(c), mensagem } if c == de => {
            Erro::Validacao { campo: Some(para.to_string()), mensagem }
        }
        outro => outro,
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Inicio {
    loja: String,
    caixa: String,
    nome: String,
    senha: String,
    senha2: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Entrada {
    id: String,
    senha: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Id {
    id: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ItemMenu {
    id: String,
    fixo: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct BuscaProdutos {
    texto: String,
    inativas: bool,
    limite: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct UnidadeEntrada {
    sigla: String,
    descricao: String,
    fracionada: bool,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct VariacaoEntrada {
    id: String,
    atual: String,
    nova: String,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AjusteEstoque {
    produto_id: String,
    quantidade: String,
    motivo: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Formatar {
    tipo: String,
    texto: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PrecoCalculo {
    custo: String,
    lucro: String,
    preco: String,
    de: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TrocaSenha {
    atual: String,
    nova: String,
    nova2: String,
}

#[derive(Deserialize)]
#[serde(default)]
struct UsuarioEntrada {
    id: String,
    nome: String,
    ativo: bool,
    senha: String,
    senha2: String,
    comissao: String,
    /// None = não mexe no papel.
    gerente: Option<bool>,
}

impl Default for UsuarioEntrada {
    fn default() -> Self {
        UsuarioEntrada {
            id: String::new(),
            nome: String::new(),
            ativo: true,
            senha: String::new(),
            senha2: String::new(),
            comissao: String::new(),
            gerente: None,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Juntar {
    caixa: String,
    senha: String,
    enderecos: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AjustesSync {
    ligado: bool,
    senha: String,
    enderecos: String,
    /// Interfaces extras (VPN, virtuais, dados móveis). `None` = a tela não
    /// mostrou o campo (modo simples): o que está gravado fica como está.
    interfaces: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ListaVendas {
    tipo: String,
    texto: String,
    sessao: String,
    limite: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Valor {
    tipo: String,
    valor: String,
    motivo: String,
    conta: String,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Filtro {
    texto: String,
    quais: String,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Periodo {
    de: String,
    ate: String,
    dias: Option<i64>,
    usuario_id: String,
    vencimento: String,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ChequeAcao {
    id: String,
    conta: String,
    motivo: String,
    divida: bool,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Conferencia {
    conta_id: String,
    saldo: String,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ChequesPrevia {
    valor: String,
    #[serde(flatten)]
    cheques: crate::cheques::ChequesEntrada,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Impressao {
    tipo: String,
    id: String,
}

/// Endereço deste computador na rede local (para digitar nos outros caixas).
fn endereco_local() -> String {
    // Não envia nada: só pergunta ao sistema por qual endereço sairia.
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| s.connect("192.168.0.1:9").map(|_| s))
        .and_then(|s| s.local_addr())
        .map(|a| a.ip().to_string())
        .unwrap_or_default()
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct BuscaPessoas {
    texto: String,
    papel: String,
    inativos: bool,
    limite: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Papel {
    papel: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Arquivo {
    caminho: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Dias {
    qual: String,
    dias: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Situacao {
    id: String,
    situacao: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Dias90 {
    dias: i64,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct MaisVendidos {
    dias: i64,
    ordem: String,
    texto: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Desfazer {
    id: String,
    ids: Vec<String>,
    desfazer: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct CaixaDaLoja {
    dispositivo: String,
    motivo: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Estorno {
    id: String,
    motivo: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ContaCliente {
    id: String,
    todas: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Ids {
    ids: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Regras {
    regras: Vec<fiscal::RegraSt>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Tributacao {
    cest: String,
    csosn: String,
}

/// Resposta de erro no mesmo formato do `Servico::chamar`.
pub fn resposta_de_erro(mensagem: &str) -> String {
    json!({ "ok": false, "erro": mensagem, "campo": null }).to_string()
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::senhas::Problema;

    fn conectado() -> Servico {
        let mut s = Servico::em_memoria().unwrap();
        let r: Value = serde_json::from_str(&s.chamar(
            "configurar_inicio",
            r#"{"loja":"Loja Teste","caixa":"Caixa 1","nome":"Ana","senha":"balcao-azul-2026","senha2":"balcao-azul-2026"}"#,
        ))
        .unwrap();
        assert_eq!(r["ok"], true, "{r}");
        s
    }

    /// f) Pela api, do jeito que a tela usa: com duas peças com o mesmo
    /// código, o leitor não escolhe nenhuma (mostra as duas), a etiqueta é
    /// recusada e o gerente vê a repetição.
    #[test]
    fn leitor_e_etiqueta_com_codigo_repetido() {
        use crate::faixas::para_testes::*;
        let mut s = conectado();
        let u = s.sessao.as_ref().unwrap().usuario_id.clone();
        let p = peca(&s.banco, &u, "Filtro de óleo");
        let chamar = |s: &mut Servico, c: &str, d: Value| -> Value {
            serde_json::from_str(&crate::apoio_chave::chamar(NAO_REPETIR, c, &d.to_string(), |x| s.chamar(c, x)))
                .unwrap()
        };
        let r = chamar(&mut s, "produto_por_codigo", json!({ "texto": "1" }));
        assert_eq!(r["dados"]["id"], json!(p), "código de uma peça só: entra direto");
        let (b, ub) = loja("Outra loja");
        peca(&b, &ub, "Pastilha de freio");
        receber(&mut s.banco, &b);
        let r = chamar(&mut s, "produto_por_codigo", json!({ "texto": "1" }));
        assert_eq!(r["ok"], true);
        assert!(r["dados"]["id"].is_null(), "{r}");
        assert_eq!(r["dados"]["repetidos"], 2);
        assert_eq!(r["dados"]["opcoes"].as_array().unwrap().len(), 2);
        let e = chamar(&mut s, "etiqueta_previa", json!({ "id": p }));
        assert!(e["dados"]["bloqueio"].as_str().unwrap().contains("também é da peça"), "{e}");
        let pedido = json!({ "itens": [{ "produtoId": p, "quantidade": 1 }], "modelo": "", "inicio": 1,
                             "barras": true, "loja": false, "ajusteX": 0, "ajusteY": 0 });
        let i = chamar(&mut s, "impressao", json!({ "tipo": "etiquetas", "id": pedido.to_string() }));
        assert_eq!(i["ok"], false, "{i}");
        let sit = chamar(&mut s, "codigos_situacao", json!({}));
        assert_eq!(sit["ok"], true, "{sit}");
        assert_eq!(sit["dados"]["repetidos"].as_array().unwrap().len(), 1);
        assert_eq!(sit["dados"]["cruzadas"].as_array().unwrap().len(), 1);
    }

    /// g) Pela api, do jeito que a tela usa: o banco é uma cópia (a
    /// identidade do computador não bate), o caixa não dá código e só o
    /// gerente o registra como caixa novo, que ganha identificador novo.
    #[test]
    fn copia_do_banco_so_o_gerente_registra_como_caixa_novo() {
        let mut s = conectado();
        let antigo = s.banco.dispositivo.clone();
        // Os dois comandos são só do gerente.
        for c in ["codigos_confirmar_computador", "codigos_registrar_caixa_novo"] {
            assert_eq!(acao_do_comando(c), Some("gerente"), "{c}");
        }
        // A peça criada antes da cópia tem código.
        let ficha = chamar(&mut s, "produto_novo", "{}");
        assert_eq!(ficha["dados"]["codigoPrevisto"], "1");
        // O banco vira cópia.
        config::gravar(&s.banco.conn, "computador_id", &crate::computador::impressao("outro-pc")).unwrap();
        faixas::conferir_computador(&s.banco.conn, crate::computador::atual().as_deref()).unwrap();
        let sit = chamar(&mut s, "codigos_situacao", "{}");
        assert_eq!(sit["dados"]["bloqueio"]["motivo"], "copia");
        assert_eq!(sit["dados"]["bloqueio"]["podeRegistrar"], true);
        let ficha = chamar(&mut s, "produto_novo", "{}");
        assert_eq!(ficha["dados"]["codigoPrevisto"], "");
        assert!(ficha["dados"]["codigoAviso"].as_str().unwrap().contains("outro computador"), "{ficha}");
        // Confirmar "o computador de sempre" não vale para uma cópia.
        let r = chamar(&mut s, "codigos_confirmar_computador", "{}");
        assert_eq!(r["ok"], false, "{r}");
        // O aviso do Início.
        let avisos = crate::avisos::do_dia(&s.banco.conn).unwrap();
        assert!(avisos.iter().any(|a| a.id == "codigos_bloqueio"));
        // Registrar como caixa novo.
        let r = chamar(&mut s, "codigos_registrar_caixa_novo", r#"{"nome":"Caixa 7"}"#);
        assert_eq!(r["ok"], true, "{r}");
        assert!(r["dados"]["bloqueio"].is_null());
        assert_ne!(s.banco.dispositivo, antigo);
        assert_eq!(config::ler(&s.banco.conn, "dispositivo_id").unwrap().unwrap(), s.banco.dispositivo);
        assert_eq!(config::nome_caixa(&s.banco.conn).unwrap(), "Caixa 7");
        // Sem bloqueio, não há o que registrar.
        let r = chamar(&mut s, "codigos_registrar_caixa_novo", "{}");
        assert_eq!(r["ok"], false, "{r}");
        // O que o caixa novo faz já sai com o identificador novo.
        let r = chamar(&mut s, "estado", "{}");
        assert_eq!(r["ok"], true);
    }

    /// g) Banco de antes da conferência do computador: o gerente confirma que
    /// este é o computador de sempre e o caixa volta a dar códigos.
    #[test]
    fn banco_sem_registro_do_computador_o_gerente_confirma() {
        let mut s = conectado();
        s.banco.conn.execute("DELETE FROM config WHERE chave = 'computador_id'", []).unwrap();
        faixas::conferir_computador(&s.banco.conn, crate::computador::atual().as_deref()).unwrap();
        let sit = chamar(&mut s, "codigos_situacao", "{}");
        assert_eq!(sit["dados"]["bloqueio"]["motivo"], "sem_registro");
        assert_eq!(sit["dados"]["bloqueio"]["podeConfirmar"], true);
        let r = chamar(&mut s, "codigos_confirmar_computador", "{}");
        assert_eq!(r["ok"], true, "{r}");
        assert!(r["dados"]["bloqueio"].is_null());
        let ficha = chamar(&mut s, "produto_novo", "{}");
        assert_ne!(ficha["dados"]["codigoPrevisto"], "");
    }

    /// PEND-01 pela API: a ficha nova de cliente mostra o código previsto; o
    /// código sai da faixa ao salvar; o gerente resolve um código repetido.
    #[test]
    fn codigo_de_cliente_sai_da_faixa_e_o_gerente_resolve_repetido() {
        let mut s = conectado();
        assert_eq!(acao_do_comando("codigos_pessoa_dar_novo"), Some("gerente"));
        let ficha = chamar(&mut s, "pessoa_nova", r#"{"papel":"cliente"}"#);
        assert_eq!(ficha["dados"]["codigo"], "");
        assert_eq!(ficha["dados"]["codigoPrevisto"], "1");
        let mut f = ficha["dados"].clone();
        f["nome"] = json!("Cliente Um");
        let r = chamar(&mut s, "pessoa_salvar", &f.to_string());
        assert_eq!(r["ok"], true, "{r}");
        let id1 = r["dados"]["id"].as_str().unwrap_or_default().to_string();
        let aberta = chamar(&mut s, "pessoa_abrir", &json!({ "id": id1 }).to_string());
        assert_eq!(aberta["dados"]["codigo"], "1");
        // Outro cadastro com o mesmo código (chegou de outra loja): só o gerente vê.
        let mut g = chamar(&mut s, "pessoa_nova", r#"{"papel":"fornecedor"}"#)["dados"].clone();
        g["nome"] = json!("Fornecedor Dois");
        let r = chamar(&mut s, "pessoa_salvar", &g.to_string());
        let id2 = r["dados"]["id"].as_str().unwrap_or_default().to_string();
        s.banco.conn.execute("UPDATE pessoa SET codigo = '1' WHERE id = ?1", [&id2]).unwrap();
        let sit = chamar(&mut s, "codigos_situacao", "{}");
        assert_eq!(sit["dados"]["pessoasRepetidas"][0]["codigo"], "1");
        assert_eq!(sit["dados"]["pessoasRepetidas"][0]["pessoas"].as_array().unwrap().len(), 2);
        let r = chamar(&mut s, "codigos_pessoa_dar_novo", &json!({ "id": id2 }).to_string());
        assert_eq!(r["ok"], true, "{r}");
        assert!(r["dados"]["pessoasRepetidas"].as_array().unwrap().is_empty());
        let aberta = chamar(&mut s, "pessoa_abrir", &json!({ "id": id2 }).to_string());
        assert_eq!(aberta["dados"]["codigo"], "3");
    }

    fn chamar(s: &mut Servico, c: &str, d: &str) -> Value {
        serde_json::from_str(&crate::apoio_chave::chamar(NAO_REPETIR, c, d, |x| s.chamar(c, x))).unwrap()
    }

    fn categorias_teste(s: &Servico) -> i64 {
        s.banco.conn.query_row("SELECT COUNT(*) FROM categoria WHERE id = 't'", [], |l| l.get(0)).unwrap()
    }

    /// C-15: pânico antes do commit desfaz a gravação e bloqueia novas
    /// gravações; consultas continuam.
    #[test]
    fn panico_antes_do_commit_desfaz_e_bloqueia() {
        let mut s = conectado();
        let r = chamar(&mut s, "__teste_panico", r#"{"onde":"antes_do_commit"}"#);
        assert_eq!(r["ok"], false);
        assert!(r["erro"].as_str().unwrap().contains("Erro interno"));
        // Nada de dado técnico na resposta.
        assert!(!r.to_string().contains("segredo123"));
        assert_eq!(categorias_teste(&s), 0, "a transação foi desfeita");
        assert!(s.banco.conn.is_autocommit());
        // Consulta continua; gravação não.
        assert_eq!(chamar(&mut s, "estado", "")["ok"], true);
        assert_eq!(chamar(&mut s, "produtos_buscar", r#"{"texto":""}"#)["ok"], true);
        let r = chamar(&mut s, "produto_salvar", r#"{"nome":"X"}"#);
        assert_eq!(r["ok"], false);
        assert!(r["erro"].as_str().unwrap().contains("Feche e abra"));
        let r = chamar(&mut s, "__teste_panico", r#"{"onde":"nenhum"}"#);
        assert_eq!(r["ok"], false);
        assert_eq!(categorias_teste(&s), 0);
    }

    /// C-15: pânico depois do commit: o que foi gravado fica (uma vez só) e
    /// a repetição é bloqueada (não duplica).
    #[test]
    fn panico_depois_do_commit_nao_repete() {
        let mut s = conectado();
        let r = chamar(&mut s, "__teste_panico", r#"{"onde":"depois_do_commit"}"#);
        assert_eq!(r["ok"], false);
        assert!(r["erro"].as_str().unwrap().contains("Confira se a operação foi gravada"));
        assert_eq!(categorias_teste(&s), 1);
        assert_eq!(chamar(&mut s, "__teste_panico", r#"{"onde":"depois_do_commit"}"#)["ok"], false);
        assert_eq!(categorias_teste(&s), 1, "não gravou de novo");
        // Reabrir (um serviço novo) volta ao normal.
        let mut novo = conectado();
        assert_eq!(chamar(&mut novo, "__teste_panico", r#"{"onde":"nenhum"}"#)["ok"], true);
    }

    /// R-04: nenhum comando "só leitura" grava no banco, com entradas de
    /// todo tipo (vazias, ids de verdade, ids que não existem). Depois de um
    /// pânico, quem grava (como `estoque_dias`) é recusado.
    #[test]
    fn so_leitura_nao_grava() {
        let mut s = conectado();
        let ok = |s: &mut Servico, c: &str, d: Value| -> Value {
            let r = chamar(s, c, &d.to_string());
            assert_eq!(r["ok"], true, "{c}: {r}");
            r["dados"].clone()
        };
        // Uma loja com um pouco de tudo.
        let mut f = ok(&mut s, "produto_novo", json!({}));
        f["nome"] = json!("Pastilha");
        f["custo"] = json!("40");
        f["estoqueInicial"] = json!("5");
        let pid = ok(&mut s, "produto_salvar", f)["id"].as_str().unwrap().to_string();
        let mut p = ok(&mut s, "pessoa_nova", json!({}));
        p["nome"] = json!("Cliente Teste");
        let cid = ok(&mut s, "pessoa_salvar", p)["id"].as_str().unwrap_or_default().to_string();
        ok(&mut s, "caixa_abrir", json!({ "valor": "50" }));
        let itens = json!([{ "produtoId": pid, "quantidade": "1" }]);
        ok(
            &mut s,
            "venda_concluir",
            json!({ "itens": itens, "tabela": 1, "pagamentos": [{ "forma": "dinheiro", "valor": "100" }] }),
        );
        // RVI-01: uma encomenda que chegou (o aviso dela grava).
        let eid = encomenda_que_chegou(&mut s);
        let entradas = [
            json!({}),
            json!({ "id": eid }),
            json!({ "id": pid, "produtoId": pid, "pessoaId": cid, "clienteId": cid, "texto": "", "codigo": "1",
                    "dias": "30", "qual": "compra", "ordem": "valor", "itens": itens, "tabela": 1,
                    "quantidade": "1", "valor": "1", "acao": "backup", "placa": "ABC1D23" }),
            json!({ "id": cid, "texto": "Pas", "dias": "90", "qual": "parado", "pagina": 1 }),
            json!({ "id": "nao-existe", "dias": "x", "qual": "?", "itens": "não é lista" }),
        ];
        for c in SO_LEITURA {
            for d in &entradas {
                let antes = s.banco.conn.total_changes();
                let r = s.chamar(c, &d.to_string());
                let depois = s.banco.conn.total_changes();
                assert_eq!(antes, depois, "\"{c}\" é só leitura mas gravou no banco (entrada {d})");
                // RVI-01: nos testes a leitura roda travada; uma tentativa de
                // gravar volta como erro (e não muda o banco), então o erro
                // também acusa.
                assert!(!r.contains("tentou gravar"), "\"{c}\" é só leitura mas tentou gravar (entrada {d}): {r}");
            }
        }
        // Depois de um pânico: estoque_dias e as outras gravações recusadas,
        // e o banco fica igual.
        assert_eq!(chamar(&mut s, "__teste_panico", r#"{"onde":"antes_do_commit"}"#)["ok"], false);
        assert!(s.comprometido);
        let antes = s.banco.conn.total_changes();
        for c in DO_CAIXA.iter().filter(|c| !matches!(**c, "gerente_encerrar")) {
            let r = chamar(&mut s, c, r#"{"qual":"compra","dias":"45"}"#);
            assert_eq!(r["ok"], false, "{c} gravou depois do pânico");
        }
        assert_eq!(antes, s.banco.conn.total_changes());
    }

    /// Encomenda de um cliente sem cadastro, já na situação "chegou".
    fn encomenda_que_chegou(s: &mut Servico) -> String {
        let r = chamar(
            s,
            "encomenda_criar",
            r#"{"clienteNome":"Maria Souza","telefone":"(15) 99808-0000","descricao":"Pastilha dianteira","quantidade":"1"}"#,
        );
        assert_eq!(r["ok"], true, "{r}");
        let id = r["dados"].as_str().unwrap().to_string();
        let r = chamar(s, "encomenda_chegou", &json!({ "id": id }).to_string());
        assert_eq!(r["ok"], true, "{r}");
        id
    }

    fn aviso_da_encomenda(s: &Servico, id: &str) -> (Option<String>, String) {
        s.banco
            .conn
            .query_row("SELECT avisado_em, atualizado_em FROM encomenda WHERE id = ?1", [id], |l| {
                Ok((l.get(0)?, l.get(1)?))
            })
            .unwrap()
    }

    /// RVI-01: o aviso pelo WhatsApp de uma encomenda que chegou marca o
    /// cliente como avisado (grava). Depois de um pânico (serviço
    /// comprometido, só consultas), ele é recusado e nada muda no banco; as
    /// consultas de verdade continuam.
    #[test]
    fn rvi01_whatsapp_depois_do_panico_nao_grava() {
        let mut s = conectado();
        let id = encomenda_que_chegou(&mut s);
        let antes = aviso_da_encomenda(&s, &id);
        assert_eq!(antes.0, None, "ainda não foi avisado");
        assert_eq!(chamar(&mut s, "__teste_panico", r#"{"onde":"antes_do_commit"}"#)["ok"], false);
        assert!(s.comprometido);
        let mudancas = s.banco.conn.total_changes();
        let r = chamar(&mut s, "encomenda_whatsapp", &json!({ "id": id }).to_string());
        assert_eq!(r["ok"], false, "gravou depois do pânico: {r}");
        assert!(r["erro"].as_str().unwrap().contains("Feche e abra"), "{r}");
        assert_eq!(aviso_da_encomenda(&s, &id), antes, "avisado_em/atualizado_em mudaram");
        assert_eq!(s.banco.conn.total_changes(), mudancas);
        // Consultas que só leem continuam (a política do C-15).
        for (c, d) in [
            ("estado", json!({})),
            ("encomendas_painel", json!({})),
            ("encomenda_para_venda", json!({ "id": id })),
            ("produtos_buscar", json!({ "texto": "" })),
        ] {
            let r = chamar(&mut s, c, &d.to_string());
            assert_eq!(r["ok"], true, "{c}: {r}");
        }
        assert_eq!(s.banco.conn.total_changes(), mudancas);
    }

    /// RVI-01: sem pânico, o aviso continua funcionando e marca o cliente
    /// como avisado (a regra da tela de encomendas não mudou).
    #[test]
    fn rvi01_whatsapp_marca_avisado_sem_panico() {
        let mut s = conectado();
        let id = encomenda_que_chegou(&mut s);
        let r = chamar(&mut s, "encomenda_whatsapp", &json!({ "id": id }).to_string());
        assert_eq!(r["ok"], true, "{r}");
        assert!(r["dados"]["url"].as_str().unwrap().starts_with("https://wa.me/5515998080000?text="), "{r}");
        assert!(aviso_da_encomenda(&s, &id).0.is_some(), "marcou como avisado");
    }

    /// RVI-01: as listas de comandos não se contradizem. Quem grava (está na
    /// lista do modo seguro, `NAO_REPETIR` + `GRAVAM_TAMBEM`, ou em
    /// `DO_CAIXA`) não pode estar em `SO_LEITURA`, que é o que continua
    /// depois de um pânico e o que a consulta do gerente libera.
    #[test]
    fn rvi01_quem_grava_nao_e_so_leitura() {
        let mut contradicoes = Vec::new();
        for c in SO_LEITURA {
            if NAO_REPETIR.contains(c) || GRAVAM_TAMBEM.contains(c) || DO_CAIXA.contains(c) {
                contradicoes.push(*c);
            }
            if bloqueado_no_modo_seguro(c) {
                contradicoes.push(*c);
            }
        }
        contradicoes.sort();
        contradicoes.dedup();
        assert!(contradicoes.is_empty(), "comandos que gravam e estão em SO_LEITURA: {contradicoes:?}");
    }

    /// Uma falha de gravação de propósito (gatilho temporário, só nesta
    /// conexão) quando `quando` for verdade para a linha nova da `tabela`.
    fn falhar_ao_gravar(s: &Servico, tabela: &str, quando: &str) {
        s.banco
            .conn
            .execute_batch(&format!(
                "CREATE TEMP TRIGGER falha_de_teste BEFORE INSERT ON main.{tabela} WHEN {quando}
                 BEGIN SELECT RAISE(ABORT, 'falha de teste'); END;"
            ))
            .unwrap();
    }

    /// M-04 (auditoria da 0.9.14, O-8): trocar a senha e o histórico disso
    /// são uma coisa só — se o histórico não grava, a senha não muda.
    #[test]
    fn m04_senha_trocar_e_o_historico_juntos() {
        let mut s = conectado();
        falhar_ao_gravar(&s, "auditoria", "NEW.acao = 'usuario.alterar'");
        let r = chamar(
            &mut s,
            "senha_trocar",
            r#"{"atual":"balcao-azul-2026","nova":"outra-senha-2026","nova2":"outra-senha-2026"}"#,
        );
        assert_eq!(r["ok"], false, "{r}");
        let ana: String = s.banco.conn.query_row("SELECT id FROM usuario", [], |l| l.get(0)).unwrap();
        assert!(usuarios::conferir_senha(&s.banco.conn, &ana, "balcao-azul-2026").is_ok(), "a senha antiga continua");
        assert!(usuarios::conferir_senha(&s.banco.conn, &ana, "outra-senha-2026").is_err(), "a nova não entrou");
    }

    /// M-04: o nome do caixa e "imprimir direto" são salvos juntos (tudo ou nada).
    #[test]
    fn m04_computador_salvar_tudo_ou_nada() {
        let mut s = conectado();
        falhar_ao_gravar(&s, "config", "NEW.chave = 'imprimir_direto'");
        let r = chamar(&mut s, "computador_salvar", r#"{"caixa":"Caixa Novo","imprimirDireto":true}"#);
        assert_eq!(r["ok"], false, "{r}");
        assert_eq!(config::nome_caixa(&s.banco.conn).unwrap(), "Caixa 1", "o nome do caixa não mudou pela metade");
    }

    /// C8: os dias da reposição e o histórico disso são uma coisa só.
    #[test]
    fn c8_estoque_dias_e_o_historico_juntos() {
        let mut s = conectado();
        let antes = crate::reposicao::dias_compra(&s.banco.conn).unwrap();
        let novo = if antes == 45 { 60 } else { 45 };
        falhar_ao_gravar(&s, "auditoria", "1");
        let r = chamar(&mut s, "estoque_dias", &json!({ "qual": "compra", "dias": novo.to_string() }).to_string());
        assert_eq!(r["ok"], false, "{r}");
        assert_eq!(crate::reposicao::dias_compra(&s.banco.conn).unwrap(), antes, "os dias não mudaram sem o histórico");
    }

    /// C-12: um segundo Rustinho no mesmo banco é recusado; ao fechar o
    /// primeiro, abre.
    #[test]
    fn um_rustinho_por_banco() {
        let pasta = std::env::temp_dir().join(format!("rustinho-trava-{}", crate::tempo::novo_id()));
        let caminho = pasta.join("rustinho.db");
        let t1 = travar_para_teste(&caminho);
        let e = travar(&caminho).unwrap_err().to_string();
        assert!(e.contains("já está aberto"), "{e}");
        drop(t1);
        assert!(travar(&caminho).is_ok());
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// R-07: qualquer falha da trava recusa abrir (não segue sem ela).
    #[test]
    fn trava_que_falha_recusa() {
        assert!(resultado_da_trava(Ok(())).is_ok());
        let e = resultado_da_trava(Err(std::fs::TryLockError::WouldBlock)).unwrap_err().to_string();
        assert!(e.contains("já está aberto"));
        for tipo in [std::io::ErrorKind::Unsupported, std::io::ErrorKind::PermissionDenied, std::io::ErrorKind::Other] {
            let e = resultado_da_trava(Err(std::fs::TryLockError::Error(std::io::Error::from(tipo))))
                .unwrap_err()
                .to_string();
            assert!(e.contains("não aceita trava"), "{e}");
        }
    }

    /// Sunda: a pasta dos dados, o banco e o registro ficam só para o
    /// usuário do computador.
    #[cfg(unix)]
    #[test]
    fn dados_so_do_usuario() {
        use std::os::unix::fs::PermissionsExt;
        let pasta = std::env::temp_dir().join(format!("rustinho-perm-{}", crate::tempo::novo_id()));
        let caminho = pasta.join("rustinho.db");
        let modo = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        {
            let mut s = Servico::abrir(&caminho).unwrap();
            let _ = s.chamar("estado", "");
            crate::registro::anotar("teste de permissão");
            assert_eq!(modo(&pasta), 0o700);
            assert_eq!(modo(&caminho), 0o600);
            assert_eq!(modo(&pasta.join("registro")), 0o700);
        }
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// R-06/R-07: fechar com uma tarefa da sincronização travada não solta a
    /// trava (outro Rustinho no mesmo banco é recusado, em vez de gravar
    /// junto); fechar normal solta na hora e abre de novo.
    #[test]
    fn fechar_com_tarefa_travada_segura_a_trava() {
        let pasta = std::env::temp_dir().join(format!("rustinho-fechar-{}", crate::tempo::novo_id()));
        let caminho = pasta.join("rustinho.db");
        // Normal: fecha e abre de novo.
        drop(Servico::abrir(&caminho).unwrap());
        let s = Servico::abrir(&caminho).unwrap();
        drop(s);
        assert!(travar(&caminho).is_ok(), "fechou direito, a trava saiu");
        // Com uma tarefa que não termina no prazo.
        let mut s = Servico::abrir(&caminho).unwrap();
        s.prazo_encerrar = Duration::from_millis(200);
        let (solta, espera) = std::sync::mpsc::channel::<()>();
        let presa = std::thread::spawn(move || {
            let _ = espera.recv();
        });
        s._motor.as_mut().unwrap().esperar_tambem(presa);
        let t = Instant::now();
        drop(s);
        assert!(t.elapsed() < Duration::from_secs(5), "não fica preso para sempre: {:?}", t.elapsed());
        let e = travar(&caminho).unwrap_err().to_string();
        assert!(e.contains("já está aberto"), "a trava continua: {e}");
        assert!(Servico::abrir(&caminho).is_err(), "outro Rustinho não abre o mesmo banco");
        solta.send(()).unwrap();
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// R-07: pasta que não dá para usar (no lugar dela há um arquivo): erro
    /// claro, sem pânico; numa pasta boa abre normalmente.
    #[test]
    fn pasta_ruim_da_erro_e_a_boa_abre() {
        let pasta = std::env::temp_dir().join(format!("rustinho-pasta-ruim-{}", crate::tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        std::fs::write(pasta.join("sou-um-arquivo"), b"x").unwrap();
        let ruim = pasta.join("sou-um-arquivo").join("rustinho.db");
        let r = std::panic::catch_unwind(|| Servico::abrir(&ruim).err().map(|e| e.to_string()));
        let e = r.expect("sem pânico").expect("dá erro");
        assert!(e.contains("Não deu para usar a pasta dos dados"), "{e}");
        assert!(std::fs::metadata(pasta.join("sou-um-arquivo")).unwrap().is_file(), "nada foi mexido");
        let boa = pasta.join("boa").join("rustinho.db");
        drop(Servico::abrir(&boa).unwrap());
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// Permissões: confere o resultado e diz quando não deu.
    #[cfg(unix)]
    #[test]
    fn permissoes_conferidas() {
        use std::os::unix::fs::PermissionsExt;
        let pasta = std::env::temp_dir().join(format!("rustinho-perm2-{}", crate::tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let arquivo = pasta.join("a.db");
        std::fs::write(&arquivo, b"x").unwrap();
        std::fs::set_permissions(&arquivo, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(crate::arquivos::so_do_usuario(&arquivo, "banco"));
        assert_eq!(std::fs::metadata(&arquivo).unwrap().permissions().mode() & 0o777, 0o600);
        assert!(!crate::arquivos::so_do_usuario(&pasta.join("nao-existe"), "backup"));
        assert!(arquivo.exists(), "nada foi apagado");
        let _ = std::fs::remove_dir_all(&pasta);
    }

    fn travar_para_teste(caminho: &Path) -> std::fs::File {
        std::fs::create_dir_all(caminho.parent().unwrap()).unwrap();
        travar(caminho).unwrap()
    }

    /// C-07: todo comando que a API atende tem uma classe de permissão, e
    /// todo comando que as telas chamam existe na API.
    #[test]
    fn todo_comando_tem_permissao() {
        let fonte = include_str!("api.rs");
        let fim = fonte.find("#[cfg(test)]\nmod testes").unwrap();
        let mut comandos = Vec::new();
        for linha in fonte[..fim].lines() {
            let t = linha.trim_start();
            if !(linha.starts_with("            \"") || linha.starts_with("        | \"")) || !t.contains('"') {
                continue;
            }
            if !(t.ends_with("=> {") || t.contains("\" =>") || t.starts_with("| \"")) {
                continue;
            }
            for parte in t.split('|') {
                let p = parte.trim();
                if let Some(nome) = p.strip_prefix('"').and_then(|r| r.split('"').next()) {
                    if nome.chars().all(|c| c.is_ascii_lowercase() || c == '_') && !nome.starts_with("__") {
                        comandos.push(nome.to_string());
                    }
                }
            }
        }
        comandos.sort();
        comandos.dedup();
        assert!(comandos.len() > 130, "{}", comandos.len());
        let sem: Vec<&String> = comandos.iter().filter(|c| classe_do_comando(c).is_none()).collect();
        assert!(sem.is_empty(), "comandos sem permissão definida: {sem:?}");
        // Um comando não pode ser "só leitura" e "grava" ao mesmo tempo.
        for c in SO_LEITURA {
            assert!(!DO_CAIXA.contains(c), "{c} está em duas listas");
            assert!(comandos.iter().any(|x| x == c), "{c} não existe mais");
        }
        for c in DO_CAIXA {
            assert!(comandos.iter().any(|x| x == c), "{c} não existe mais");
        }
        // As telas só chamam comandos que existem.
        let pasta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../app");
        let mut faltam = Vec::new();
        for sub in ["qml", "qml_android", "base"] {
            let Ok(lista) = std::fs::read_dir(pasta.join(sub)) else { continue };
            for arq in lista.flatten() {
                let texto = std::fs::read_to_string(arq.path()).unwrap_or_default();
                for pedaco in texto.split("Api.chamar(\"").skip(1) {
                    let nome = pedaco.split('"').next().unwrap_or_default();
                    if !comandos.iter().any(|c| c == nome) {
                        faltam.push(format!("{}: {nome}", arq.file_name().to_string_lossy()));
                    }
                }
            }
        }
        assert!(faltam.is_empty(), "as telas chamam comandos que não existem: {faltam:?}");
    }

    // ------------------------------------------------------------------
    // R-05 (revisado): o bloqueio só vale para o dinheiro de um registro
    // comprovadamente inconsistente.
    // ------------------------------------------------------------------
    use rusqlite::{params, Connection};

    fn plantar(conn: &Connection, tabela: &str, id: &str, estado: &str) {
        conn.execute("DELETE FROM sync_referencia", []).unwrap();
        conn.execute(
            "INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo)
             VALUES (?1, json_array(?2), 'pessoa', '[\"sumiu\"]', ?3, '2026-01-01T00:00:00Z', 'log')",
            params![tabela, id, estado],
        )
        .unwrap();
    }

    type Operacao = Box<dyn Fn(&Connection) -> Resultado<()>>;

    /// R5-T6 · R5-7, R5-8: cada operação de dinheiro das famílias do documento recusa
    /// o registro inconsistente e deixa passar o esperando, o apagado, o aceito e
    /// qualquer outro registro.
    #[test]
    fn r5_t6_operacoes_de_dinheiro_recusam_so_o_inconsistente() {
        use crate::{cheques, compras, devolucoes, financeiro, pagar, receber};
        let b = Banco::em_memoria().unwrap();
        let c = &b.conn;
        let operacoes: Vec<(&str, &str, &str, Operacao)> = vec![
            (
                "conta_receber",
                "id1",
                "receber::cancelar_conta",
                Box::new(|c| receber::cancelar_conta(c, "d", "u", "id1", "motivo")),
            ),
            ("recebimento", "id1", "receber::estornar", Box::new(|c| receber::estornar(c, "d", "u", "id1", "motivo"))),
            ("cheque", "id1", "cheques::depositar", Box::new(|c| cheques::depositar(c, "d", "u", "id1", "conta"))),
            ("cheque", "id1", "cheques::repassar", Box::new(|c| cheques::repassar(c, "d", "u", "id1", "fornecedor"))),
            (
                "cheque",
                "id1",
                "cheques::devolver",
                Box::new(|c| cheques::devolver(c, "d", "u", "id1", "motivo", false)),
            ),
            ("cheque", "id1", "cheques::desfazer", Box::new(|c| cheques::desfazer(c, "d", "u", "id1"))),
            ("conta_pagar", "id1", "pagar::cancelar", Box::new(|c| pagar::cancelar(c, "d", "u", "id1", "motivo"))),
            (
                "conta_pagar",
                "id1",
                "pagar::alterar",
                Box::new(|c| {
                    pagar::alterar(c, "d", "u", &pagar::AlteracaoEntrada { id: "id1".into(), ..Default::default() })
                }),
            ),
            (
                "conta_pagar",
                "id1",
                "pagar::pagar",
                Box::new(|c| {
                    pagar::pagar(c, "d", "u", &pagar::PagamentoEntrada { conta_id: "id1".into(), ..Default::default() })
                        .map(|_| ())
                }),
            ),
            ("pagamento", "id1", "pagar::estornar", Box::new(|c| pagar::estornar(c, "d", "u", "id1", "motivo"))),
            ("compra", "id1", "compras::lancar", Box::new(|c| compras::lancar(c, "d", "u", "id1").map(|_| ()))),
            ("compra", "id1", "compras::estornar", Box::new(|c| compras::estornar(c, "d", "u", "id1", "motivo"))),
            (
                "compra",
                "id1",
                "compras::atualizar",
                Box::new(|c| compras::atualizar(c, &compras::CompraEntrada { id: "id1".into(), ..Default::default() })),
            ),
            (
                "devolucao",
                "id1",
                "devolucoes::desfazer",
                Box::new(|c| devolucoes::desfazer(c, "d", "u", "id1", "motivo")),
            ),
            (
                "movimento_conta",
                "id1",
                "financeiro::estornar_movimento",
                Box::new(|c| financeiro::estornar_movimento(c, "d", "u", "id1", "motivo")),
            ),
        ];
        for (tabela, id, nome, op) in &operacoes {
            plantar(c, tabela, id, "inconsistente");
            let e = op(c).expect_err(nome).to_string();
            assert!(e.contains("inconsistente"), "{nome}: devia recusar por inconsistência: {e}");
            for livre in ["esperando", "apagado", "aceito"] {
                plantar(c, tabela, id, livre);
                let r = op(c);
                assert!(
                    !r.err().is_some_and(|e| e.to_string().contains("inconsistente")),
                    "{nome} ({livre}) não podia recusar"
                );
            }
            // Inconsistente em OUTRO registro da mesma tabela: esta operação passa.
            plantar(c, tabela, "outro-id", "inconsistente");
            assert!(
                !op(c).err().is_some_and(|e| e.to_string().contains("inconsistente")),
                "{nome}: outro registro não podia bloquear"
            );
        }
        // Os filhos (itens) também seguram a operação do pai.
        c.execute("DELETE FROM sync_referencia", []).unwrap();
    }

    /// R5-T7 · R5-8: pelo fluxo real da tela, uma parcela inconsistente não segura as outras,
    /// nem a venda, nem o fechamento do caixa; ver e imprimir continuam.
    #[test]
    fn r5_t7_o_resto_da_loja_continua() {
        let mut s = conectado();
        let cliente = {
            let mut f = chamar(&mut s, "pessoa_nova", r#"{"papel":"cliente"}"#)["dados"].clone();
            f["nome"] = json!("Oficina do Zé");
            chamar(&mut s, "pessoa_salvar", &f.to_string())["dados"]["id"].as_str().unwrap().to_string()
        };
        let r = chamar(
            &mut s,
            "receber_lancar",
            &json!({ "clienteId": cliente, "descricao": "caderno", "valor": "200", "parcelas": 2, "primeiroVencimento": "" }).to_string(),
        );
        assert_eq!(r["ok"], true, "{r}");
        let conta = chamar(&mut s, "receber_cliente", &json!({ "id": cliente, "todas": false }).to_string());
        let ids: Vec<String> = conta["dados"]["parcelas"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["id"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(ids.len(), 2);
        assert_eq!(chamar(&mut s, "caixa_abrir", r#"{"valor":"50"}"#)["ok"], true);
        plantar(&s.banco.conn, "conta_receber", &ids[0], "inconsistente");
        let baixar = |s: &mut Servico, id: &str| {
            chamar(
                s,
                "receber_baixar",
                &json!({ "clienteId": cliente, "contas": [id], "valor": "100", "forma": "dinheiro" }).to_string(),
            )
        };
        let r = baixar(&mut s, &ids[0]);
        assert_eq!(r["ok"], false, "{r}");
        assert!(r["erro"].as_str().unwrap().contains("inconsistente"), "{r}");
        // A outra parcela do mesmo cliente é recebida normalmente.
        let r = baixar(&mut s, &ids[1]);
        assert_eq!(r["ok"], true, "{r}");
        // Ver continua; fechar o caixa não é bloqueado por isso.
        assert_eq!(chamar(&mut s, "receber_cliente", &json!({ "id": cliente, "todas": true }).to_string())["ok"], true);
        let r = chamar(&mut s, "caixa_fechar", r#"{"contado":"150","observacoes":""}"#);
        assert!(!r.to_string().contains("inconsistente"), "{r}");
    }

    /// R5-9 pela API: aceitar é comando de gerente, exige motivo e grava a auditoria.
    #[test]
    fn r5_api_aceitar_exige_gerente_motivo_e_audita() {
        let mut s = conectado();
        assert_eq!(acao_do_comando("referencia_aceitar"), Some("gerente"));
        plantar(&s.banco.conn, "conta_receber", "c1", "inconsistente");
        let dados = |m: &str| json!({ "tabela": "conta_receber", "chave": "[\"c1\"]", "motivo": m }).to_string();
        let r = chamar(&mut s, "referencia_aceitar", &dados(""));
        assert_eq!(r["ok"], false, "{r}");
        let r = chamar(&mut s, "referencia_aceitar", &dados("Caixa de origem foi perdido"));
        assert_eq!(r["ok"], true, "{r}");
        assert_eq!(r["dados"][0]["estado"], "aceito");
        let n: i64 = s
            .banco
            .conn
            .query_row("SELECT COUNT(*) FROM auditoria WHERE acao = 'referencia.aceitar' AND detalhes LIKE '%Caixa de origem%'", [], |l| l.get(0))
            .unwrap();
        assert_eq!(n, 1);
        crate::sync::exigir_editavel(&s.banco.conn, "conta_receber", "c1").unwrap();
    }

    /// R5-T9 · R5-10: varredura. Toda operação de dinheiro listada chama o bloqueio; se alguém
    /// tirar a chamada (ou criar uma operação nova sem ela e listá-la aqui), o teste falha.
    #[test]
    fn r5_t9_varredura_toda_operacao_de_dinheiro_chama_o_bloqueio() {
        let fontes: [(&str, &str, &[&str]); 6] = [
            ("receber", include_str!("receber.rs"), &["receber", "estornar", "cancelar_conta"]),
            ("cheques", include_str!("cheques.rs"), &["depositar", "repassar", "devolver", "desfazer"]),
            ("pagar", include_str!("pagar.rs"), &["pagar", "estornar", "alterar", "cancelar"]),
            ("compras", include_str!("compras.rs"), &["lancar", "atualizar", "estornar"]),
            ("devolucoes", include_str!("devolucoes.rs"), &["desfazer"]),
            ("financeiro", include_str!("financeiro.rs"), &["estornar_movimento"]),
        ];
        let corpo = |codigo: &str, nome: &str| -> String {
            let ini = codigo.find(&format!("pub fn {nome}(")).unwrap_or_else(|| panic!("não achei {nome}"));
            let resto = &codigo[ini + 1..];
            let fim = ["\npub fn ", "\nfn ", "\n#[cfg(test)]"]
                .iter()
                .filter_map(|m| resto.find(m))
                .min()
                .unwrap_or(resto.len());
            resto[..fim].to_string()
        };
        for (arq, codigo, nomes) in fontes {
            for nome in nomes {
                let c = corpo(codigo, nome);
                assert!(
                    c.contains("exigir_editavel") || c.contains("exigir_filhos_editaveis"),
                    "{arq}::{nome} mexe em dinheiro e não chama o bloqueio de registro inconsistente (R-05)"
                );
            }
        }
        // Controle: uma função de leitura não precisa (e a varredura sabe diferenciar).
        assert!(!corpo(include_str!("receber.rs"), "previa").contains("exigir_editavel"));
    }

    // ------------------------------------------------------------------
    // F-17: política das senhas na borda (tela) e aviso para senhas antigas.
    // ------------------------------------------------------------------

    fn f17_novo_servico() -> Servico {
        Servico::em_memoria().unwrap()
    }

    fn f17_inicio(s: &mut Servico, senha: &str) -> Value {
        chamar(
            s,
            "configurar_inicio",
            &json!({ "loja": "Fanti Auto Peças", "caixa": "Caixa 1", "nome": "Ana", "senha": senha, "senha2": senha })
                .to_string(),
        )
    }

    /// Senha antiga curta (3 caracteres; o "#" não aparece em hash Argon2).
    const F17_ANTIGA: &str = "a#1";

    /// Usuário "de antes da política": criado pelo núcleo com senha curta.
    fn f17_legado(s: &Servico, nome: &str, gerente: bool) -> String {
        let u = usuarios::criar(&s.banco.conn, &s.banco.dispositivo, nome, "1234", true).unwrap();
        // Hash gravado direto, como num banco de antes do piso de 4 caracteres.
        let hash = usuarios::argon2_hash_para_teste(F17_ANTIGA);
        s.banco.conn.execute("UPDATE usuario SET senha_hash = ?2 WHERE id = ?1", params![u.id, hash]).unwrap();
        if gerente {
            usuarios::definir_gerente(&s.banco.conn, &u.id, true).unwrap();
        }
        u.id
    }

    fn f17_entrar(s: &mut Servico, id: &str, senha: &str) -> Value {
        chamar(s, "entrar", &json!({ "id": id, "senha": senha }).to_string())
    }

    fn f17_auditorias(s: &Servico, usuario: &str) -> i64 {
        s.banco
            .conn
            .query_row(
                "SELECT COUNT(*) FROM auditoria WHERE acao = 'usuario.senha_fora_da_politica' AND usuario_id = ?1",
                [usuario],
                |l| l.get(0),
            )
            .unwrap()
    }

    /// F17-A1: a primeira configuração aplica a regra do gerente (4),
    /// sem criar nada quando recusa.
    #[test]
    fn f17_a1_primeira_configuracao() {
        let mut s = f17_novo_servico();
        for (senha, parte) in [("123", "pelo menos 4"), ("caf", "pelo menos 4")] {
            let r = f17_inicio(&mut s, senha);
            assert_eq!(r["ok"], false, "{senha}: {r}");
            assert_eq!(r["campo"], "senha", "{senha}: {r}");
            assert!(r["erro"].as_str().unwrap().contains(parte), "{senha}: {r}");
            assert_eq!(usuarios::quantidade(&s.banco.conn).unwrap(), 0, "recusada: nada foi criado");
            assert!(config::ler(&s.banco.conn, "loja_nome").unwrap().is_none(), "nem a loja");
        }
        let r = f17_inicio(&mut s, "1234");
        assert_eq!(r["ok"], true, "{r}");
    }

    /// F17-A2: usuários — mínimo de 4 caracteres para todos (regra da 0.9.13), Unicode e espaços valem.
    #[test]
    fn f17_a2_usuario_salvar() {
        let mut s = conectado();
        let novo = |s: &mut Servico, nome: &str, senha: &str, gerente: Option<bool>| {
            chamar(
                s,
                "usuario_salvar",
                &json!({ "nome": nome, "senha": senha, "senha2": senha, "ativo": true, "gerente": gerente })
                    .to_string(),
            )
        };
        for (senha, ok) in [("cur", false), ("pão de queijo", true)] {
            let r = novo(&mut s, "Bia", senha, None);
            assert_eq!(r["ok"], ok, "{senha}: {r}");
            if !ok {
                assert_eq!(r["campo"], "senha");
            }
        }
        // Já criada com a boa; as seguintes testam o gerente.
        let r = novo(&mut s, "Cida", "abc", Some(true));
        assert_eq!(r["ok"], false, "gerente também precisa de 4: {r}");
        assert!(r["erro"].as_str().unwrap().contains("backups"), "{r}");
        let r = novo(&mut s, "Cida", "doze-caracteres-ok", Some(true));
        assert_eq!(r["ok"], true, "{r}");
        assert_eq!(r["dados"]["gerente"], true);
        // Promover junto com uma senha curta demais não vale; promover sem trocar a senha não é bloqueado.
        let bia = usuarios::listar(&s.banco.conn, false).unwrap().into_iter().find(|u| u.nome == "Bia").unwrap();
        let r = chamar(
            &mut s,
            "usuario_salvar",
            &json!({ "id": bia.id, "nome": "Bia", "ativo": true, "senha": "abc", "senha2": "abc", "gerente": true })
                .to_string(),
        );
        assert_eq!(r["ok"], false, "{r}");
        let r = chamar(
            &mut s,
            "usuario_salvar",
            &json!({ "id": bia.id, "nome": "Bia", "ativo": true, "senha": "", "senha2": "", "gerente": true })
                .to_string(),
        );
        assert_eq!(r["ok"], true, "sem trocar a senha nada é forçado: {r}");
        // Operador comum: 4 basta na troca.
        let r = chamar(
            &mut s,
            "usuario_salvar",
            &json!({ "id": bia.id, "nome": "Bia", "ativo": true, "senha": "mesa-verde", "senha2": "mesa-verde", "gerente": false })
                .to_string(),
        );
        assert_eq!(r["ok"], true, "{r}");
    }

    /// F17-A3: troca da própria senha: a regra é a do papel; se recusar, a senha antiga continua valendo.
    #[test]
    fn f17_a3_senha_trocar() {
        let mut s = conectado(); // Ana, gerente, senha "balcao-azul-2026"
        let trocar = |s: &mut Servico, nova: &str| {
            chamar(s, "senha_trocar", &json!({ "atual": "balcao-azul-2026", "nova": nova, "nova2": nova }).to_string())
        };
        let r = trocar(&mut s, "caf");
        assert_eq!(r["ok"], false, "{r}");
        assert_eq!(r["campo"], "nova");
        assert!(r["erro"].as_str().unwrap().contains("4"), "{r}");
        let id = s.sessao.as_ref().unwrap().usuario_id.clone();
        assert!(usuarios::conferir_senha(&s.banco.conn, &id, "balcao-azul-2026").is_ok(), "a antiga continua valendo");
        assert_eq!(trocar(&mut s, "x".repeat(129).as_str())["ok"], false, "129 caracteres");
        assert_eq!(trocar(&mut s, "chuva de manhã na oficina")["ok"], true);
        // Outro usuário: a mesma regra (o que importa é o tamanho).
        let bia = usuarios::criar(&s.banco.conn, &s.banco.dispositivo, "Bia", "balcao-bia-1", true).unwrap();
        assert_eq!(f17_entrar(&mut s, &bia.id, "balcao-bia-1")["ok"], true);
        let r = chamar(
            &mut s,
            "senha_trocar",
            &json!({ "atual": "balcao-bia-1", "nova": "cafe-pao", "nova2": "cafe-pao" }).to_string(),
        );
        assert_eq!(r["ok"], true, "{r}");
    }

    /// F17-A4: quem tem senha antiga entra normalmente; vira aviso e marcador, não bloqueio.
    #[test]
    fn f17_a4_senha_antiga_entra_com_aviso() {
        let mut s = conectado();
        let bia = f17_legado(&s, "Bia", false);
        let cida = f17_legado(&s, "Cida", true);
        assert_eq!(usuarios::situacao_senha(&s.banco.conn, &bia).unwrap(), None, "ninguém foi avaliado ainda");
        let hash_antes: String =
            s.banco.conn.query_row("SELECT senha_hash FROM usuario WHERE id = ?1", [&bia], |l| l.get(0)).unwrap();

        let r = f17_entrar(&mut s, &bia, F17_ANTIGA);
        assert_eq!(r["ok"], true, "senha antiga entra: {r}");
        assert_eq!(usuarios::situacao_senha(&s.banco.conn, &bia).unwrap(), Some(Some(Problema::Curta)));
        let avisos = chamar(&mut s, "avisos_do_dia", "{}");
        let a = avisos["dados"].as_array().unwrap().iter().find(|a| a["id"] == format!("senha.{bia}")).cloned();
        let a = a.expect("aviso da senha");
        assert!(a["texto"].as_str().unwrap().contains("é curta"), "{a}");
        assert_eq!(a["atraso"], "", "operador: sem alarme em vermelho");
        assert_eq!(a["funcao"], "configuracoes");

        // Segunda entrada: o aviso continua, a auditoria não repete.
        f17_entrar(&mut s, &bia, F17_ANTIGA);
        assert_eq!(f17_auditorias(&s, &bia), 1);

        // Gerente: aviso mais firme (backups) e entra do mesmo jeito.
        assert_eq!(f17_entrar(&mut s, &cida, F17_ANTIGA)["ok"], true);
        let avisos = chamar(&mut s, "avisos_do_dia", "{}");
        let a =
            avisos["dados"].as_array().unwrap().iter().find(|a| a["id"] == format!("senha.{cida}")).cloned().unwrap();
        assert!(a["atraso"].as_str().unwrap().contains("backups"), "{a}");
        assert!(
            avisos["dados"].as_array().unwrap().iter().all(|a| a["id"] != format!("senha.{bia}")),
            "aviso é de quem entrou"
        );
        assert_eq!(f17_auditorias(&s, &cida), 1);

        // Nada foi forçado nem tocado: o hash é o mesmo e o marcador não é sincronizado nem guarda a senha.
        let hash_depois: String =
            s.banco.conn.query_row("SELECT senha_hash FROM usuario WHERE id = ?1", [&bia], |l| l.get(0)).unwrap();
        assert_eq!(hash_antes, hash_depois);
        let n: i64 = s
            .banco
            .conn
            .query_row("SELECT COUNT(*) FROM alteracao WHERE tabela = 'senha_situacao'", [], |l| l.get(0))
            .unwrap();
        assert_eq!(n, 0, "marcador só deste computador");
        let guardou: i64 = s
            .banco
            .conn
            .query_row(
                "SELECT COUNT(*) FROM auditoria WHERE detalhes LIKE '%a#1%'
                 OR (SELECT COUNT(*) FROM senha_situacao WHERE hash LIKE '%a#1%' OR problema LIKE '%a#1%') > 0",
                [],
                |l| l.get(0),
            )
            .unwrap();
        assert_eq!(guardou, 0, "a senha em claro não vai para lugar nenhum");
    }

    /// F17-A5: esconder o aviso vale para quem escondeu; o painel do gerente mostra quem está com
    /// senha antiga, e o operador não vê isso; trocar a senha apaga o aviso e a marca.
    #[test]
    fn f17_a5_esconder_painel_e_troca() {
        let mut s = conectado();
        let ana = s.sessao.as_ref().unwrap().usuario_id.clone();
        let bia = f17_legado(&s, "Bia", false);
        let cida = f17_legado(&s, "Cida", false);
        f17_entrar(&mut s, &bia, F17_ANTIGA);
        f17_entrar(&mut s, &cida, F17_ANTIGA);
        // Cida esconde o aviso dela; o de Bia continua para a Bia.
        let r = chamar(&mut s, "aviso_esconder", &json!({ "id": format!("senha.{cida}") }).to_string());
        assert_eq!(r["ok"], true);
        let avisos = chamar(&mut s, "avisos_do_dia", "{}");
        assert!(avisos["dados"].as_array().unwrap().iter().all(|a| a["id"] != format!("senha.{cida}")));
        f17_entrar(&mut s, &bia, F17_ANTIGA);
        let avisos = chamar(&mut s, "avisos_do_dia", "{}");
        assert!(avisos["dados"].as_array().unwrap().iter().any(|a| a["id"] == format!("senha.{bia}")));

        // Operador (Bia) não vê a marca de ninguém.
        let lista = chamar(&mut s, "usuarios_listar", "{}");
        assert!(lista["dados"].as_array().unwrap().iter().all(|u| u["senhaFraca"] == false), "{lista}");
        // O gerente vê quem está fora da política (Ana está dentro).
        usuarios::definir_gerente(&s.banco.conn, &cida, true).unwrap(); // papel mudou: marcador deixa de valer
        let hash_ana: String =
            s.banco.conn.query_row("SELECT senha_hash FROM usuario WHERE id = ?1", [&ana], |l| l.get(0)).unwrap();
        assert!(!hash_ana.is_empty());
        assert_eq!(f17_entrar(&mut s, &ana, "balcao-azul-2026")["ok"], true);
        let lista = chamar(&mut s, "usuarios_listar", "{}");
        let fraca = |nome: &str| {
            lista["dados"].as_array().unwrap().iter().find(|u| u["nome"] == nome).unwrap()["senhaFraca"].clone()
        };
        assert_eq!(fraca("Bia"), true, "{lista}");
        assert_eq!(fraca("Ana"), false);
        assert_eq!(fraca("Cida"), false, "o papel mudou: a avaliação antiga não vale mais (volta a não avaliada)");

        // Bia troca a senha: aviso e marca somem.
        f17_entrar(&mut s, &bia, F17_ANTIGA);
        let r = chamar(
            &mut s,
            "senha_trocar",
            &json!({ "atual": F17_ANTIGA, "nova": "mesa-verde-9", "nova2": "mesa-verde-9" }).to_string(),
        );
        assert_eq!(r["ok"], true, "{r}");
        let avisos = chamar(&mut s, "avisos_do_dia", "{}");
        assert!(avisos["dados"].as_array().unwrap().iter().all(|a| a["id"] != format!("senha.{bia}")));
        assert_eq!(usuarios::situacao_senha(&s.banco.conn, &bia).unwrap(), None);
        f17_entrar(&mut s, &ana, "balcao-azul-2026");
        let lista = chamar(&mut s, "usuarios_listar", "{}");
        assert_eq!(
            lista["dados"].as_array().unwrap().iter().find(|u| u["nome"] == "Bia").unwrap()["senhaFraca"],
            false
        );
    }

    /// F17-A6: se outro caixa trocar a senha (o hash muda), a avaliação antiga deixa de valer.
    #[test]
    fn f17_a6_hash_trocado_por_outro_caixa() {
        let mut s = conectado();
        let bia = f17_legado(&s, "Bia", false);
        f17_entrar(&mut s, &bia, F17_ANTIGA);
        assert_eq!(usuarios::situacao_senha(&s.banco.conn, &bia).unwrap(), Some(Some(Problema::Curta)));
        let novo = usuarios::argon2_hash_para_teste("mesa-verde-9");
        s.banco.conn.execute("UPDATE usuario SET senha_hash = ?2 WHERE id = ?1", params![bia, novo]).unwrap();
        assert_eq!(usuarios::situacao_senha(&s.banco.conn, &bia).unwrap(), None);
        let avisos = chamar(&mut s, "avisos_do_dia", "{}");
        assert!(avisos["dados"].as_array().unwrap().iter().all(|a| a["id"] != format!("senha.{bia}")));
        // Entrando com a nova (boa): dentro da política, sem aviso e sem auditoria.
        f17_entrar(&mut s, &bia, "mesa-verde-9");
        assert_eq!(usuarios::situacao_senha(&s.banco.conn, &bia).unwrap(), Some(None));
        assert_eq!(f17_auditorias(&s, &bia), 1, "só a da entrada antiga");
    }

    /// F17-A7: tamanhos grandes e Unicode funcionam de ponta a ponta (criar, sair e entrar).
    #[test]
    fn f17_a7_senha_longa_e_unicode() {
        let mut s = f17_novo_servico();
        let longa: String =
            (0..).map(|i| format!("palavra{i}-")).flat_map(|w| w.chars().collect::<Vec<_>>()).take(128).collect();
        assert_eq!(longa.chars().count(), 128);
        let r = f17_inicio(&mut s, &format!("{longa}z"));
        assert_eq!(r["ok"], false, "129 caracteres");
        let r = f17_inicio(&mut s, &longa);
        assert_eq!(r["ok"], true, "128 caracteres: {r}");
        let ana = s.sessao.as_ref().unwrap().usuario_id.clone();
        assert_eq!(f17_entrar(&mut s, &ana, &longa)["ok"], true, "entra com a senha de 128");
        let enorme = "a".repeat(100_000);
        let r = f17_entrar(&mut s, &ana, &enorme);
        assert_eq!(r["ok"], false, "texto enorme é recusado sem gastar o Argon2");
        // Unicode: acento composto, frase com espaços, outro alfabeto.
        let b = chamar(
            &mut s,
            "usuario_salvar",
            &json!({ "nome": "Bia", "senha": "pão de queijo quente", "senha2": "pão de queijo quente", "ativo": true })
                .to_string(),
        );
        assert_eq!(b["ok"], true, "{b}");
        let bia = b["dados"]["id"].as_str().unwrap().to_string();
        assert_eq!(f17_entrar(&mut s, &bia, "pão de queijo quente")["ok"], true);
        assert_eq!(f17_entrar(&mut s, &ana, &longa)["ok"], true);
        let c = chamar(
            &mut s,
            "usuario_salvar",
            &json!({ "nome": "Dai", "senha": "日本語のパスワードです", "senha2": "日本語のパスワードです", "ativo": true })
                .to_string(),
        );
        assert_eq!(c["ok"], true, "{c}");
    }

    /// F17-A8: senha da rede da loja: nova segue a política; a que já estava gravada não é forçada
    /// e aparece só como informação.
    #[test]
    fn f17_a8_senha_da_rede() {
        let mut s = conectado();
        let salvar = |s: &mut Servico, senha: &str| {
            chamar(s, "sync_salvar", &json!({ "ligado": true, "senha": senha, "enderecos": "" }).to_string())
        };
        for curta in ["abc", "12", "   abc  "] {
            let r = salvar(&mut s, curta);
            assert_eq!(r["ok"], false, "{curta}: {r}");
            assert_eq!(r["campo"], "senha");
        }
        // Só o tamanho conta (0.9.13): senhas simples valem.
        for simples in ["1234", "12345678", "senha123", "caixa1234ab"] {
            assert_eq!(salvar(&mut s, simples)["ok"], true, "{simples}");
        }
        // Senha antiga gravada (de antes da política): salvar só os endereços não força troca.
        config::gravar(&s.banco.conn, "sync_senha", "ab1").unwrap();
        let r = chamar(
            &mut s,
            "sync_salvar",
            &json!({ "ligado": true, "senha": "", "enderecos": "192.168.0.12" }).to_string(),
        );
        assert_eq!(r["ok"], true, "não força a troca: {r}");
        assert_eq!(r["dados"]["senhaFraca"], true);
        let r = salvar(&mut s, "bola-de-gude-azul");
        assert_eq!(r["ok"], true, "{r}");
        assert_eq!(r["dados"]["senhaFraca"], false);
        assert_eq!(chamar(&mut s, "sync_ler", "{}")["dados"]["senhaFraca"], false);
    }
}
