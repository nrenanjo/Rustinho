//! Porta de entrada do núcleo: a interface manda um comando com dados em
//! JSON e recebe a resposta em JSON.
//!
//! Resposta de sucesso: `{"ok": true, "dados": ...}`
//! Resposta de erro:    `{"ok": false, "erro": "mensagem", "campo": "nome"}`

use std::path::Path;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{json, Value};

use crate::banco::Banco;
use crate::despacho::{ler, Acesso, Classe, Comando, Estorno, Id, Pedido};
use crate::erro::{Erro, Resultado};
use crate::{
    arquivos, auditoria, avisos, backup, busca, caixa, cheques, codigo, comissoes, compras, config, devolucoes,
    documento, dre, encomendas, etiquetas, faixas, financeiro, fiscal, importar, impressao, menu, numeros, oficina,
    pagar, painel, pessoas, produtos, receber, reposicao, requisitos, sync, texto, unidades, usuarios, vendas,
};

/// Versão do Rustinho. Mora na base (`rustinho_base::VERSAO`) para quem grava
/// a versão (registro, backup) não depender do serviço; continua aqui também.
pub use rustinho_base::VERSAO;

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
/// O nome da tela do gerente na auditoria da liberação.
/// C04-a: a senha do gerente digitada errada fica no histórico: quem pediu,
/// em que caixa e para quê. A senha digitada nunca é gravada; a espera depois
/// de muitos erros (D-09) continua a mesma.
fn senha_errada_na_auditoria(
    conn: &rusqlite::Connection,
    disp: &str,
    usuario_id: &str,
    nome: &str,
    para: &str,
) -> Resultado<()> {
    let caixa = config::nome_caixa(conn)?;
    auditoria::registrar(
        conn,
        disp,
        Some(usuario_id),
        "gerente.senha_errada",
        "usuario",
        usuario_id,
        &[format!("{nome} digitou a senha do gerente errada"), format!("Para: {para}"), format!("Caixa: {caixa}")],
    )
}

fn nome_da_tela(tela: &str) -> &'static str {
    match tela {
        "importar_cadastros" => "Trazer cadastros",
        _ => "tela do gerente",
    }
}

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

/// Qual ação do gerente cada comando precisa (a classe dele diz).
fn acao_do_comando(comando: &str) -> Option<&'static str> {
    classe(comando).and_then(Classe::acao)
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
    "xml_pendencia_resolver",
];

/// Quantas chaves de operação já feitas ficam guardadas (as mais antigas saem).
const CHAVES_GUARDADAS: usize = 2000;

/// Os comandos que o próprio serviço atende (no `match` de `executar_livre` e
/// no de `executar_comando`), com a classe de permissão de cada um (veja
/// `despacho::Classe`). Um comando sem classe não roda: o serviço o trata
/// como desconhecido. Um comando novo também entra na lista congelada de
/// `tests/contratos_comandos.rs`.
const DO_SERVICO: &[(&str, Classe)] = &[
    // Sem ninguém conectado (o `match` de `executar_livre`).
    ("estado", Classe::LeituraSemSessao),
    ("configurar_inicio", Classe::SemSessao),
    ("usuarios_entrada", Classe::LeituraSemSessao),
    ("entrar", Classe::SemSessao),
    ("sync_juntar", Classe::SemSessao),
    ("sync_desistir_juntar", Classe::SemSessao),
    ("sync_ler", Classe::LeituraSemSessao),
    ("aparencia_salvar", Classe::SemSessao),
    ("codigo_ler_foto", Classe::LeituraSemSessao),
    // F03 (dev.3.3, 2.3): o lugar do quadro da câmera é do núcleo.
    ("codigo_caminho_foto", Classe::LeituraSemSessao),
    ("backup_automatico", Classe::SemSessao),
    ("backup_ao_fechar", Classe::SemSessao),
    ("sair", Classe::SemSessao),
    ("numero_formatar", Classe::LeituraSemSessao),
    // ARQ-09/H13 (dev.3.3, 2.2): somas e o exemplo de atraso pelo núcleo,
    // para a tela não ler número do jeito dela.
    ("numero_somar", Classe::LeituraSemSessao),
    ("crediario_exemplo", Classe::LeituraSemSessao),
    ("preco_calcular", Classe::LeituraSemSessao),
    // Permissões, senha do gerente e usuários (pílula Gerente).
    ("permissao", Classe::Leitura),
    ("gerente_liberar", Classe::Caixa),
    ("gerente_encerrar", Classe::Caixa),
    ("gerente_ler", Classe::ConsultaDoGerente("gerente")),
    ("gerente_salvar", Classe::Gerente("gerente")),
    ("usuario_salvar", Classe::Gerente("usuarios")),
    ("senha_trocar", Classe::Caixa),
    // D7: a senha do gerente é pedida e conferida no próprio comando.
    ("compra_conferir_autorizacao", Classe::Caixa),
    // Configurações e impressão (juntam vários domínios).
    ("computador_salvar", Classe::Caixa),
    ("config_ler", Classe::Leitura),
    ("config_salvar", Classe::Gerente("configuracoes")),
    ("impressao", Classe::Leitura),
    // Troca de dados (vão para o domínio na etapa 5). FALHA-05: o que a Simultaneidade só lê tem consulta própria (a
    // senha dela não abre a pílula Gerente); gravar continua sendo "gerente". Pendências da troca de dados e
    // retirada de caixa: 0.9.15.
    ("sync_salvar", Classe::Gerente("configuracoes")),
    ("conflitos_listar", Classe::ConsultaDoGerente("simultaneidade")),
    ("conflito_resolver", Classe::Gerente("gerente")),
    ("referencia_aceitar", Classe::Gerente("gerente")),
    ("conflitos_manter_todos", Classe::Gerente("gerente")),
    ("caixas_conhecidos", Classe::Leitura),
    ("caixa_retirar", Classe::Gerente("gerente")),
    ("caixa_readmitir", Classe::Gerente("gerente")),
    ("pendencias_listar", Classe::ConsultaDoGerente("simultaneidade")),
    ("troca_historico", Classe::ConsultaDoGerente("simultaneidade")),
    ("auditoria_listar", Classe::ConsultaDoGerente("simultaneidade")),
    ("pendencia_arquivar", Classe::Gerente("gerente")),
    ("pendencias_reaplicar", Classe::Gerente("gerente")),
    // Códigos das peças, Gerente › Códigos (faixas: etapa 5).
    ("codigos_situacao", Classe::ConsultaDoGerente("simultaneidade")),
    ("codigos_assumir", Classe::Gerente("gerente")),
    ("codigos_definir_pendentes", Classe::Gerente("gerente")),
    ("codigos_confirmar_computador", Classe::Gerente("gerente")),
    ("codigos_registrar_caixa_novo", Classe::Gerente("gerente")),
    ("codigos_pessoa_dar_novo", Classe::Gerente("gerente")),
    ("codigos_dar_novo", Classe::Gerente("gerente")),
    // Backup (etapa 6).
    ("backup_ler", Classe::ConsultaDoGerente("backup")),
    ("backup_salvar", Classe::Gerente("backup")),
    ("backup_agora", Classe::Gerente("backup")),
    ("backup_conectar_drive", Classe::Gerente("backup")),
    ("backup_desligar_drive", Classe::Gerente("backup")),
    ("backup_nuvem_lista", Classe::ConsultaDoGerente("backup")),
    ("backup_conferir", Classe::ConsultaDoGerente("backup")),
    ("backup_restaurar", Classe::Gerente("backup")),
];

/// As tabelas de comandos dos domínios (cada domínio declara a sua no
/// próprio módulo; veja `despacho`).
const TABELAS: &[&[Comando]] = &[
    avisos::COMANDOS,
    busca::COMANDOS,
    caixa::COMANDOS,
    cheques::COMANDOS,
    comissoes::COMANDOS,
    compras::COMANDOS,
    devolucoes::COMANDOS,
    dre::COMANDOS,
    encomendas::COMANDOS,
    etiquetas::COMANDOS,
    financeiro::COMANDOS,
    fiscal::COMANDOS,
    importar::COMANDOS,
    menu::COMANDOS,
    oficina::COMANDOS,
    pagar::COMANDOS,
    painel::COMANDOS,
    pessoas::COMANDOS,
    produtos::COMANDOS,
    receber::COMANDOS,
    reposicao::COMANDOS,
    requisitos::COMANDOS,
    unidades::COMANDOS,
    usuarios::COMANDOS,
    vendas::COMANDOS,
];

/// O comando de um domínio, se é de algum.
fn do_dominio(comando: &str) -> Option<&'static Comando> {
    TABELAS.iter().flat_map(|t| t.iter()).find(|c| c.nome == comando)
}

/// A classe de permissão de um comando.
pub fn classe(comando: &str) -> Option<Classe> {
    DO_SERVICO
        .iter()
        .find(|(nome, _)| *nome == comando)
        .map(|(_, c)| *c)
        .or_else(|| do_dominio(comando).map(|c| c.classe))
}

/// Todos os comandos que o núcleo atende (os do serviço e os dos domínios).
pub fn comandos() -> Vec<&'static str> {
    DO_SERVICO.iter().map(|(nome, _)| *nome).chain(TABELAS.iter().flat_map(|t| t.iter().map(|c| c.nome))).collect()
}

/// Comandos que só leem (não gravam nada no banco). Depois de um erro
/// interno grave, só estes continuam (veja `Servico::chamar`).
pub fn so_leitura(comando: &str) -> bool {
    classe(comando).is_some_and(Classe::leitura)
}

/// Comandos que funcionam sem ninguém conectado.
pub fn sem_sessao(comando: &str) -> bool {
    classe(comando).is_some_and(Classe::sem_sessao)
}

/// Resposta a uma gravação depois de um erro interno grave (pânico).
const TEXTO_COMPROMETIDO: &str =
    "O Rustinho teve um erro interno. Feche e abra de novo para continuar; até lá nada é gravado.";

/// RVI-01: nos testes, toda consulta (classe que só lê) roda com o banco só
/// para leitura, para a suíte acusar um comando de leitura que grave. No
/// programa, isso vale depois de um pânico (veja `Servico::chamar`).
const LEITURA_ESTRITA: bool = cfg!(any(test, feature = "ancoras-de-teste"));

/// O que podemos fazer depois de um erro interno grave: ler, sair e fazer o
/// backup ao fechar.
fn pode_depois_de_erro(comando: &str) -> bool {
    so_leitura(comando) || matches!(comando, "sair" | "backup_ao_fechar" | "gerente_encerrar")
}

/// Classificação de um comando (para os testes de permissão): a ação do
/// gerente, "leitura" ou "caixa".
pub fn classe_do_comando(comando: &str) -> Option<&'static str> {
    classe(comando).map(Classe::nome)
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
    /// DP-52 (b): a senha dada ao ABRIR uma tela que só o gerente pode usar
    /// ("Trazer cadastros"). Vale só para os comandos dessa tela
    /// (`TELAS_DO_GERENTE`), só para quem estava usando e só com a chave que a
    /// tela sorteou; acaba quando a tela encerra, depois de `LIBERACAO` sem
    /// uso, ou ao sair. O `gerente_encerrar` sem dados não a apaga.
    Tela { usuario_id: String, tela: &'static str, chave: String, ate: Instant },
}

/// DP-52 (b): as telas que pedem a senha do gerente ao abrir e os comandos
/// que essa senha libera (T-2 (a): a prévia também).
const TELAS_DO_GERENTE: &[(&str, &[&str])] = &[("importar_cadastros", &["importar_cadastros", "importar_previa"])];

fn tela_do_gerente(tela: &str) -> Option<(&'static str, &'static [&'static str])> {
    TELAS_DO_GERENTE.iter().find(|(t, _)| *t == tela.trim()).map(|(t, c)| (*t, *c))
}

impl Liberacao {
    fn ate(&self) -> Instant {
        match self {
            Liberacao::Consulta { ate, .. } | Liberacao::Uma { ate, .. } | Liberacao::Tela { ate, .. } => *ate,
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
    chamada: Option<(String, [u8; 32], String)>,
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
    resultado_da_trava(tentar_travar(&f))?;
    Ok(f)
}

/// A trava em si, sem esperar. Em todo Unix é o `flock` do sistema, o mesmo
/// que a biblioteca do Rust usa no Linux: a da montagem do Android (Rust 1.97)
/// não tem `try_lock` no Android e respondia "não suportado", e por isso o
/// app do celular nunca abria o banco (0.9.18). No Windows, a da biblioteca.
#[cfg(unix)]
fn tentar_travar(f: &std::fs::File) -> Result<(), std::fs::TryLockError> {
    use std::os::fd::AsRawFd;
    // SAFETY: o descritor é de `f`, que fica aberto durante toda a chamada;
    // `flock` só recebe o número e não mexe em memória do Rust.
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        return Ok(());
    }
    let e = std::io::Error::last_os_error();
    if e.kind() == std::io::ErrorKind::WouldBlock {
        Err(std::fs::TryLockError::WouldBlock)
    } else {
        Err(std::fs::TryLockError::Error(e))
    }
}

#[cfg(not(unix))]
fn tentar_travar(f: &std::fs::File) -> Result<(), std::fs::TryLockError> {
    f.try_lock()
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

/// A resposta de um comando para a tela: `{ok: true, dados}` ou `{ok: false,
/// erro, campo}`.
fn envelope(comando: &str, resultado: Resultado<Value>) -> Value {
    match resultado {
        Ok(valor) => json!({ "ok": true, "dados": valor }),
        Err(erro) => {
            // Só o comando e o erro técnico; os dados digitados não.
            if matches!(erro, Erro::Interno(_) | Erro::Banco(_)) {
                crate::registro::anotar(&format!("{comando}: {erro}"));
            }
            // G02 (dev.3.3): o erro do banco vai curto para a tela; o
            // detalhe ficou no registro (acima).
            json!({ "ok": false, "erro": erro.para_a_tela(), "campo": erro.nome_do_campo() })
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
            so_leitura(comando) && (self.comprometido || LEITURA_ESTRITA) && !self.banco.em_modo_seguro();
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
            Ok(r) => envelope(comando, r),
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
        // Só com alguém conectado: o que a classe do comando não diz que
        // funciona sem sessão (inclusive um comando desconhecido).
        if !sem_sessao(comando) {
            self.sessao()?;
            // O papel pode ter mudado em outro caixa.
            self.atualizar_sessao()?;
            let sessao = self.sessao()?.clone();
            return self.executar_conectado(comando, dados, &sessao);
        }
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
            // Tema da tela (cinza, AMOLED ou Planeta Roxo): vale só para este
            // aparelho (não vai para os outros caixas nem vem do backup).
            // Aceita também `{ escuro }`, das telas de antes da 0.9.18.
            "aparencia_salvar" => {
                let d: Aparencia = ler(dados)?;
                let tema = d.tema()?;
                config::gravar(&self.banco.conn, "tela_tema", tema)?;
                // A chave antiga continua certa para quem volta a uma versão
                // anterior (lá o roxo vira o cinza).
                config::gravar(&self.banco.conn, "tela_escura", if tema == "amoled" { "1" } else { "0" })?;
                Ok(json!({ "tema": tema, "escuro": tema == "amoled" }))
            }
            // Câmera do celular como leitor de código de barras.
            // F03 (dev.3.3, 2.3): o quadro fica num lugar só, o do núcleo;
            // outro caminho mandado pela tela é recusado e nada é apagado.
            "codigo_caminho_foto" => Ok(json!({ "caminho": self.foto_da_camera().display().to_string() })),
            "codigo_ler_foto" => {
                let d: FotoDaCamera = ler(dados)?;
                let foto = self.foto_da_camera();
                if !d.caminho.is_empty() && Path::new(&arquivos::caminho_local(&d.caminho)) != foto {
                    return Err(Erro::validacao("A foto da câmera fica num lugar só, e não é esse."));
                }
                Ok(json!({ "codigo": codigo::ler_foto(&foto)?.unwrap_or_default() }))
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
            "numero_somar" => {
                let d: Somar = ler(dados)?;
                Ok(json!({ "texto": somar_textos(&d.tipo, &d.somar, &d.subtrair)? }))
            }
            "crediario_exemplo" => {
                let d: ExemploAtraso = ler(dados)?;
                let valor = numeros::ler_dinheiro(&d.valor).map_err(|m| Erro::campo("valor", m))?.unwrap_or(0);
                let (extra, total, dentro) = receber::exemplo_atraso(&d.multa, &d.juros, &d.carencia, valor, d.dias)?;
                Ok(json!({
                    "valor": numeros::escrever_dinheiro(valor),
                    "extra": numeros::escrever_dinheiro(extra),
                    "total": numeros::escrever_dinheiro(total),
                    "dentroDaTolerancia": dentro,
                }))
            }
            "preco_calcular" => {
                let d: PrecoCalculo = ler(dados)?;
                let (lucro, preco) = produtos::calcular_preco(&d.custo, &d.lucro, &d.preco, &d.de);
                Ok(json!({ "lucro": lucro, "preco": preco }))
            }
            _ => Err(Erro::Interno(format!("comando desconhecido: {comando}"))),
        }
    }

    /// F03: o arquivo do quadro da câmera, na pasta privada (a do banco).
    fn foto_da_camera(&self) -> std::path::PathBuf {
        let privada = self.banco.caminho.as_deref().and_then(Path::parent).map(Path::to_path_buf);
        codigo::caminho_da_foto(&privada.unwrap_or_else(std::env::temp_dir))
    }

    /// Pode fazer esta ação do gerente sem senha? (é gerente, ou a loja
    /// liberou a ação para todos na pílula Gerente.)
    fn pode_sem_senha(&self, acao: &str, sessao: &Sessao) -> Resultado<bool> {
        pode_sem_senha(&self.banco.conn, acao, sessao)
    }

    /// As permissões desta chamada (as senhas do gerente digitadas e o
    /// comando em curso) e o banco, para as perguntas de permissão.
    fn permissoes<'a>(&'a mut self, sessao: &'a Sessao) -> (Permissoes<'a>, &'a rusqlite::Connection) {
        (Permissoes { liberacoes: &mut self.liberacoes, chamada: self.chamada.as_ref(), sessao }, &self.banco.conn)
    }

    /// Veja `Permissoes::pode_ler`.
    fn pode_ler(&mut self, acao: &str, sessao: &Sessao) -> Resultado<bool> {
        let (mut p, conn) = self.permissoes(sessao);
        p.pode_ler(conn, acao)
    }

    /// Veja `Permissoes::liberada_esta_chamada`.
    fn liberada_esta_chamada(&mut self, sessao: &Sessao) -> bool {
        self.permissoes(sessao).0.liberada_esta_chamada()
    }

    /// Veja `Permissoes::exigir`.
    fn exigir(&mut self, acao: &str, sessao: &Sessao) -> Resultado<()> {
        let (mut p, conn) = self.permissoes(sessao);
        p.exigir(conn, acao)
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
        let chave_tela = dados["chaveTela"].as_str().unwrap_or("").trim().to_string();
        self.chamada = Some((comando.to_string(), resumo, chave_tela));
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
        // A permissão vem da classe do comando; sem classe, não roda.
        match classe(comando) {
            Some(c) => {
                if let Some(acao) = c.acao() {
                    self.exigir(acao, sessao)?;
                }
            }
            // Só nos testes: o defeito de propósito (C-15) não tem classe.
            None if cfg!(test) && comando == "__teste_panico" => {}
            None => return Err(Erro::Interno(format!("comando desconhecido: {comando}"))),
        }
        // Os comandos dos domínios: a função da tabela do domínio.
        if let Some(c) = do_dominio(comando) {
            let modo_seguro = self.banco.em_modo_seguro();
            let mut acesso = Permissoes { liberacoes: &mut self.liberacoes, chamada: self.chamada.as_ref(), sessao };
            let mut pedido = Pedido::novo(
                &mut self.banco.conn,
                &disp,
                &sessao.usuario_id,
                sessao.gerente,
                modo_seguro,
                self.banco.defeito.as_deref(),
                &mut acesso,
            );
            return (c.funcao)(&mut pedido, dados);
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
                // DP-52 (b): a tela do gerente, com a chave que ela sorteou.
                let tela = if d.tela.trim().is_empty() {
                    None
                } else {
                    let Some((tela, _)) = tela_do_gerente(&d.tela) else {
                        return Err(Erro::validacao("Diga para que é a senha do gerente."));
                    };
                    if d.chave.trim().chars().count() < 16 {
                        return Err(Erro::validacao("A tela não mandou a chave dela. Feche e abra a tela de novo."));
                    }
                    Some((tela, d.chave.trim().to_string()))
                };
                let pedido = if tela.is_some() {
                    Ok(Vec::new())
                } else if !d.comando.trim().is_empty() {
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
                            Ok(_) if tela.is_some() => {
                                let (tela, chave) = tela.clone().unwrap_or_default();
                                self.liberacoes.retain(|l| {
                                    !matches!(l, Liberacao::Tela { tela: t, usuario_id: u, .. }
                                        if *t == tela && *u == usuario_id)
                                });
                                self.liberacoes.push(Liberacao::Tela {
                                    usuario_id: usuario_id.clone(),
                                    tela,
                                    chave,
                                    ate: Instant::now() + LIBERACAO,
                                });
                                format!("{} (nesta tela, até fechar)", nome_da_tela(tela))
                            }
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
                        let para = match (&tela, &pedido) {
                            (Some((t, _)), _) => format!("{} (tela {t})", nome_da_tela(t)),
                            (None, Err((comando, _))) => format!(
                                "{} (comando {comando})",
                                nome_da_acao(acao_do_comando(comando).unwrap_or("precos"))
                            ),
                            (None, Ok(acoes)) => format!(
                                "consultar {} ({})",
                                acoes.iter().map(|a| nome_da_acao(a)).collect::<Vec<_>>().join("; "),
                                acoes.join(", ")
                            ),
                        };
                        self.gravar_se_der(senha_errada_na_auditoria(
                            &self.banco.conn,
                            &disp,
                            &sessao.usuario_id,
                            &sessao.nome,
                            &para,
                        ))?;
                        Err(Erro::campo("senha", "Senha do gerente errada."))
                    }
                }
            }
            "gerente_encerrar" => {
                // DP-52 (b): com a tela e a chave, encerra só a liberação dessa
                // tela; sem dados, as consultas e as gravações (como antes), e a
                // da tela continua até ela fechar (a navegação guiada fecha e
                // reabre painéis sem fechar a tela).
                let tela = dados["tela"].as_str().unwrap_or("").trim().to_string();
                let chave = dados["chave"].as_str().unwrap_or("").trim().to_string();
                let usuario_id = sessao.usuario_id.clone();
                if tela.is_empty() {
                    self.liberacoes.retain(|l| matches!(l, Liberacao::Tela { .. }));
                } else {
                    self.liberacoes.retain(|l| {
                        !matches!(l, Liberacao::Tela { tela: t, chave: c, usuario_id: u, .. }
                            if *t == tela && *c == chave && *u == usuario_id)
                    });
                }
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
                        senha_errada_na_auditoria(
                            &self.banco.conn,
                            &disp,
                            &sessao.usuario_id,
                            &sessao.nome,
                            "conferir a autorização da nota de compra (comando compra_conferir_autorizacao)",
                        )?;
                        return Err(Erro::campo("senha", "Senha do gerente errada."));
                    }
                };
                let tx = self.banco.conn.transaction()?;
                compras::registrar_conferencia(&tx, &disp, &g.id, &d.conferencia)?;
                tx.commit()?;
                Ok(json!(compras::ficha(&self.banco.conn, &d.conferencia.id)?))
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
                        (impressao::dre(conn, &dre::html(conn, &f)?)?, format!("dre-gerencial-{}", fiscal::hoje(conn)?))
                    }
                    "lista_compra" => (
                        impressao::lista_compra(conn, &reposicao::lista_compra_html(conn)?)?,
                        format!("lista-de-compra-{}", fiscal::hoje(conn)?),
                    ),
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
                // M-04: o registro na auditoria vai na transação que termina
                // a restauração (também se ela terminar ao abrir).
                let r = backup::restaurar_por(
                    &mut self.banco.conn,
                    caminho.as_deref(),
                    &d.arquivo,
                    d.nuvem,
                    &d.senha,
                    Some(&sessao.usuario_id),
                );
                let conferido = seguro && r.is_ok() && crate::banco::conferir_integridade(&self.banco.conn).is_ok();
                if seguro {
                    self.banco.conn.pragma_update(None, "query_only", true)?;
                }
                // O banco foi trocado mas a restauração não terminou: até
                // fechar e abrir (quando ela termina), nada é gravado.
                if r.is_err() && caminho.as_deref().is_some_and(crate::banco::restauracao_pela_metade) {
                    self.comprometido = true;
                    self.sessao = None;
                    self.liberacoes.clear();
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
        let tema = tema_da_tela(conn)?;
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
            "tema": tema,
            "escuro": tema == "amoled",
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
    /// Ou (DP-52 (b)): usar esta tela do gerente, com a chave que ela sorteou.
    #[serde(default)]
    tela: String,
    #[serde(default)]
    chave: String,
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

/// Pode fazer esta ação do gerente sem senha? (é gerente, ou a loja
/// liberou a ação para todos na pílula Gerente.)
fn pode_sem_senha(conn: &rusqlite::Connection, acao: &str, sessao: &Sessao) -> Resultado<bool> {
    Ok(sessao.gerente || (!SEMPRE_DO_GERENTE.contains(&acao) && acoes_livres(conn)?.iter().any(|a| a == acao)))
}

/// As perguntas de permissão de uma chamada: as senhas do gerente digitadas
/// no aperto (`Liberacao`), o comando em curso e quem está conectado. É a
/// regra de sempre; os comandos dos domínios a usam pelo `despacho::Acesso`.
struct Permissoes<'a> {
    liberacoes: &'a mut Vec<Liberacao>,
    chamada: Option<&'a (String, [u8; 32], String)>,
    sessao: &'a Sessao,
}

impl Permissoes<'_> {
    fn consulta_aberta(&mut self, acao: &str) -> bool {
        let agora = Instant::now();
        self.liberacoes.retain(|l| agora < l.ate());
        for l in self.liberacoes.iter_mut() {
            if let Liberacao::Consulta { usuario_id, acao: a, ate } = l {
                if *a == acao && *usuario_id == self.sessao.usuario_id {
                    *ate = agora + LIBERACAO;
                    return true;
                }
            }
        }
        false
    }

    /// DP-52 (b): a senha foi dada ao abrir a tela deste comando, por quem
    /// está usando, com a mesma chave? Renova o prazo a cada uso.
    fn liberada_pela_tela(&mut self) -> bool {
        let agora = Instant::now();
        self.liberacoes.retain(|l| agora < l.ate());
        let Some((comando, _, chave)) = self.chamada else { return false };
        if chave.is_empty() {
            return false;
        }
        for l in self.liberacoes.iter_mut() {
            if let Liberacao::Tela { usuario_id, tela, chave: c, ate } = l {
                let cobre = tela_do_gerente(tela).is_some_and(|(_, comandos)| comandos.contains(&comando.as_str()));
                if cobre && *usuario_id == self.sessao.usuario_id && c == chave {
                    *ate = agora + LIBERACAO;
                    return true;
                }
            }
        }
        false
    }

    /// A senha do gerente foi digitada para esta chamada (o mesmo comando,
    /// com os mesmos dados, pelo mesmo usuário)?
    fn liberada_esta_chamada(&mut self) -> bool {
        let agora = Instant::now();
        self.liberacoes.retain(|l| agora < l.ate());
        let Some((comando, resumo, _)) = self.chamada else { return false };
        self.liberacoes.iter().any(|l| {
            matches!(l, Liberacao::Uma { usuario_id, comando: c, resumo: r, .. }
                if usuario_id == &self.sessao.usuario_id && c == comando && r == resumo)
        })
    }
}

impl Acesso for Permissoes<'_> {
    fn pode_sem_senha(&mut self, conn: &rusqlite::Connection, acao: &str) -> Resultado<bool> {
        pode_sem_senha(conn, acao, self.sessao)
    }

    /// Pode ler o que é desta ação? Sem senha, ou com a senha do gerente
    /// digitada para consultar a tela dela (renova a cada leitura).
    fn pode_ler(&mut self, conn: &rusqlite::Connection, acao: &str) -> Resultado<bool> {
        if pode_sem_senha(conn, acao, self.sessao)? {
            return Ok(true);
        }
        Ok(self.consulta_aberta(acao))
    }

    /// Esta chamada pode fazer esta ação do gerente? Uma leitura vale com a
    /// consulta da tela; qualquer outra coisa precisa da senha para ela.
    fn exigir(&mut self, conn: &rusqlite::Connection, acao: &str) -> Resultado<()> {
        if pode_sem_senha(conn, acao, self.sessao)? || self.liberada_esta_chamada() {
            return Ok(());
        }
        if self.liberada_pela_tela() {
            return Ok(());
        }
        let leitura = self.chamada.map(|(c, _, _)| so_leitura(c) && acao_do_comando(c) == Some(acao)).unwrap_or(false);
        if leitura && self.consulta_aberta(acao) {
            return Ok(());
        }
        Err(erro_so_do_gerente(acao))
    }
}

/// O erro de uma ação do gerente sem a senha. "precos" tem campo próprio: a
/// tela sabe que é o preço da peça que pede a senha.
fn erro_so_do_gerente(acao: &str) -> Erro {
    Erro::campo(
        if acao == "precos" { "precos" } else { "gerente" },
        format!("Só o gerente pode: {}. Peça para o gerente digitar a senha.", nome_da_acao(acao)),
    )
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

/// Os temas da tela (0.9.18): cinza claro, preto AMOLED e Planeta Roxo.
const TEMAS: [&str; 3] = ["cinza", "amoled", "roxo"];

/// O tema deste aparelho: `tela_tema`; num banco de antes da 0.9.18, o
/// `tela_escura` ("1" = AMOLED; "0" ou nada = cinza).
fn tema_da_tela(conn: &rusqlite::Connection) -> Resultado<&'static str> {
    if let Some(t) = config::ler(conn, "tela_tema")? {
        if let Some(tema) = TEMAS.iter().find(|x| **x == t) {
            return Ok(tema);
        }
    }
    Ok(if config::ler(conn, "tela_escura")?.as_deref() == Some("1") { "amoled" } else { "cinza" })
}

#[derive(Deserialize)]
struct Aparencia {
    #[serde(default)]
    tema: Option<String>,
    /// Das telas de antes da 0.9.18: lua (true) ou sol (false).
    #[serde(default)]
    escuro: Option<bool>,
}

impl Aparencia {
    fn tema(&self) -> Resultado<&'static str> {
        match (&self.tema, self.escuro) {
            (Some(t), _) => TEMAS
                .iter()
                .find(|x| **x == t.as_str())
                .copied()
                .ok_or_else(|| Erro::validacao("Tema desconhecido: escolha cinza, AMOLED ou Planeta Roxo.")),
            (None, Some(escuro)) => Ok(if escuro { "amoled" } else { "cinza" }),
            (None, None) => Err(Erro::validacao("Escolha um tema.")),
        }
    }
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

/// Soma pela regra do núcleo (H13, ARQ-09): cada texto lido como dinheiro,
/// percentual ou quantidade; vazio vale 0; texto inválido é recusado com a
/// mensagem da leitura (nunca vira 0 em silêncio).
fn somar_textos(tipo: &str, somar: &[String], subtrair: &[String]) -> Resultado<String> {
    let ler = |texto: &str| match tipo {
        "dinheiro" => numeros::ler_dinheiro(texto),
        "percentual" => numeros::ler_percentual(texto),
        _ => numeros::ler_quantidade(texto),
    };
    if !matches!(tipo, "dinheiro" | "percentual" | "quantidade") {
        return Err(Erro::Interno(format!("numero_somar: tipo desconhecido: {tipo}")));
    }
    let mut valores = Vec::with_capacity(somar.len() + subtrair.len());
    for (lista, sinal) in [(somar, 1), (subtrair, -1)] {
        for texto in lista {
            let v = ler(texto).map_err(Erro::validacao)?.unwrap_or(0);
            valores.push(v.checked_mul(sinal).ok_or_else(numeros::grande_demais)?);
        }
    }
    let total = numeros::soma(valores)?;
    Ok(match tipo {
        "dinheiro" => numeros::escrever_dinheiro(total),
        "percentual" => numeros::escrever_percentual(total),
        _ => numeros::escrever_quantidade(total),
    })
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
struct Formatar {
    tipo: String,
    texto: String,
}

/// `codigo_ler_foto`: o caminho é opcional (só vale o do próprio leitor).
#[derive(Deserialize, Default)]
#[serde(default)]
struct FotoDaCamera {
    caminho: String,
}

/// `numero_somar`: soma `somar` e tira `subtrair`, lendo cada texto pela
/// regra do núcleo (vazio = 0).
#[derive(Deserialize, Default)]
#[serde(default)]
struct Somar {
    tipo: String,
    somar: Vec<String>,
    subtrair: Vec<String>,
}

/// `crediario_exemplo`: as taxas como estão digitadas (ainda não gravadas).
#[derive(Deserialize)]
struct ExemploAtraso {
    multa: String,
    juros: String,
    carencia: String,
    valor: String,
    dias: i64,
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
struct CaixaDaLoja {
    dispositivo: String,
    motivo: String,
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
        // Caso 4 (C8): diz a parte certa de resolver.
        assert!(e["dados"]["bloqueio"].as_str().unwrap().contains("Simultaneidade › Duplicatas"), "{e}");
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
        for c in comandos().into_iter().filter(|c| so_leitura(c)) {
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
        for c in comandos().into_iter().filter(|c| classe(c) == Some(Classe::Caixa) && *c != "gerente_encerrar") {
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
    /// lista do modo seguro, `NAO_REPETIR` + `GRAVAM_TAMBEM`) não pode ter
    /// classe que só lê, que é o que continua depois de um pânico e o que a
    /// consulta do gerente libera. (Ler e gravar sem ser do gerente —
    /// `Classe::Leitura` e `Classe::Caixa` — são classes diferentes: um
    /// comando não tem as duas.)
    #[test]
    fn rvi01_quem_grava_nao_e_so_leitura() {
        let mut contradicoes = Vec::new();
        for c in comandos().into_iter().filter(|c| so_leitura(c)) {
            if NAO_REPETIR.contains(&c) || GRAVAM_TAMBEM.contains(&c) {
                contradicoes.push(c);
            }
            if bloqueado_no_modo_seguro(c) {
                contradicoes.push(c);
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

    /// A trava do sistema (a mesma do Android): pega, recusa um segundo
    /// arquivo aberto no mesmo lugar sem esperar e solta ao fechar. No Android
    /// a da biblioteca do Rust respondia "não suportado" e o app nunca abria.
    #[test]
    fn trava_do_sistema_pega_recusa_e_solta() {
        let pasta = std::env::temp_dir().join(format!("rustinho-flock-{}", crate::tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let caminho = pasta.join("rustinho.db.trava");
        let abrir = || std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&caminho).unwrap();
        let primeiro = abrir();
        assert!(tentar_travar(&primeiro).is_ok(), "a trava do sistema funciona aqui");
        let segundo = abrir();
        assert!(
            matches!(tentar_travar(&segundo), Err(std::fs::TryLockError::WouldBlock)),
            "o segundo é recusado na hora (sem esperar)"
        );
        drop(primeiro);
        assert!(tentar_travar(&segundo).is_ok(), "ao fechar o primeiro, a trava solta");
        drop(segundo);
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

    /// C-07: todo comando que a API atende tem uma classe de permissão (a
    /// tabela do serviço e as dos domínios), cada nome numa tabela só; o
    /// serviço atende cada comando da tabela dele; e todo comando que as telas
    /// chamam existe na API.
    #[test]
    fn todo_comando_tem_permissao() {
        let comandos = comandos();
        assert!(comandos.len() > 130, "{}", comandos.len());
        let mut vistos = std::collections::BTreeSet::new();
        for c in &comandos {
            assert!(vistos.insert(*c), "{c} está em duas tabelas");
            assert!(classe_do_comando(c).is_some(), "{c} sem classe");
        }
        // Os braços dos dois `match` do serviço (`executar_livre` e
        // `executar_comando`) são os comandos da tabela dele: sem classe um
        // braço não rodaria; sem braço, o comando seria desconhecido.
        let fonte = include_str!("api.rs");
        let fim = fonte.find("#[cfg(test)]\nmod testes").unwrap();
        let mut bracos = std::collections::BTreeSet::new();
        for linha in fonte[..fim].lines() {
            let Some(resto) = linha.strip_prefix("            \"") else { continue };
            let (nome, depois) = resto.split_once('"').unwrap_or_default();
            if depois.trim_start().starts_with("=>") && !nome.starts_with("__") {
                bracos.insert(nome);
            }
        }
        let da_tabela: std::collections::BTreeSet<&str> = DO_SERVICO.iter().map(|(n, _)| *n).collect();
        let sem_classe: Vec<_> = bracos.difference(&da_tabela).collect();
        let sem_braco: Vec<_> = da_tabela.difference(&bracos).collect();
        assert!(
            sem_classe.is_empty() && sem_braco.is_empty(),
            "braços do serviço sem classe em DO_SERVICO: {sem_classe:?}; comandos de DO_SERVICO sem braço: {sem_braco:?}"
        );
        // As telas só chamam comandos que existem.
        let pasta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../app");
        let mut faltam = Vec::new();
        for sub in ["qml", "qml_android", "base"] {
            let Ok(lista) = std::fs::read_dir(pasta.join(sub)) else { continue };
            for arq in lista.flatten() {
                let texto = std::fs::read_to_string(arq.path()).unwrap_or_default();
                for pedaco in texto.split("Api.chamar(\"").skip(1) {
                    let nome = pedaco.split('"').next().unwrap_or_default();
                    if !comandos.contains(&nome) {
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
        // O campo do valor contado é "valor" (o da tela, `TelaCaixa.qml`): com
        // outro nome, o fechamento era recusado por falta do valor e o teste
        // não provava nada sobre o bloqueio (achado da revisão da fase 6).
        let r = chamar(&mut s, "caixa_fechar", r#"{"valor":"150","motivo":""}"#);
        assert_eq!(r["ok"], true, "o fechamento do caixa não pode ser bloqueado pela parcela inconsistente: {r}");
        assert_eq!(r["dados"]["diferenca"], "0,00", "{r}");
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

    /// R5-T9 · R5-10 (gerado da marca `dinheiro`, dev.3.3 passo 3.5; E04):
    /// cada comando de `despacho::COMANDOS_DE_DINHEIRO` existe, o atendente
    /// dele chama a função de domínio marcada, e a função chama o bloqueio de registro
    /// inconsistente, menos as lacunas conhecidas, que ficam aqui até a 6.3
    /// (quando uma passar a chamar, tem de sair da lista). As 16 operações que
    /// a varredura conferia à mão até a 3.5 continuam marcadas: tirar a marca
    /// de uma delas faz o teste falhar.
    #[test]
    fn r5_t9_varredura_toda_operacao_de_dinheiro_chama_o_bloqueio() {
        use crate::despacho::COMANDOS_DE_DINHEIRO;
        const LACUNAS: &[&str] = &[
            "vendas::cancelar",
            "devolucoes::devolver",
            "caixa::movimentar_com_conta",
            "caixa::fechar",
            "comissoes::fechar",
            "comissoes::reabrir",
        ];
        const CONFERIDAS_ATE_A_3_5: &[&str] = &[
            "receber::receber",
            "receber::estornar",
            "receber::cancelar_conta",
            "cheques::depositar",
            "cheques::repassar",
            "cheques::devolver",
            "cheques::desfazer",
            "pagar::pagar",
            "pagar::estornar",
            "pagar::alterar",
            "pagar::cancelar",
            "compras::lancar",
            "compras::atualizar",
            "compras::estornar",
            "devolucoes::desfazer",
            "financeiro::estornar_movimento",
        ];
        let fonte = |modulo: &str| -> &'static str {
            match modulo {
                "receber" => include_str!("../operacao/receber.rs"),
                "cheques" => include_str!("../operacao/cheques.rs"),
                "pagar" => include_str!("../operacao/pagar.rs"),
                "compras" => include_str!("../compras/mod.rs"),
                "devolucoes" => include_str!("../operacao/devolucoes.rs"),
                "financeiro" => include_str!("../operacao/financeiro.rs"),
                "vendas" => include_str!("../operacao/vendas.rs"),
                "caixa" => include_str!("../operacao/caixa.rs"),
                "comissoes" => include_str!("../operacao/comissoes.rs"),
                _ => panic!("módulo marcado sem fonte no teste R-05: {modulo} (acrescente aqui)"),
            }
        };
        // O corpo de uma função: da assinatura até a próxima função (ou o fim do bloco de testes).
        let corpo = |codigo: &str, assinatura: &str| -> String {
            let ini = codigo.find(assinatura).unwrap_or_else(|| panic!("não achei {assinatura}"));
            let resto = &codigo[ini + assinatura.len()..];
            let mut fim = resto.len();
            let mut pos = 0;
            for linha in resto.split_inclusive('\n') {
                let l = linha.trim_start();
                if pos > 0 && (l.starts_with("fn ") || l.starts_with("pub fn ") || l.starts_with("#[cfg(test)]")) {
                    fim = pos;
                    break;
                }
                pos += linha.len();
            }
            resto[..fim].to_string()
        };
        for (comando, funcao) in COMANDOS_DE_DINHEIRO {
            assert!(
                super::do_dominio(comando).is_some(),
                "{comando} tem a marca dinheiro e não é comando de nenhum domínio"
            );
            let (modulo, nome) = funcao.split_once("::").unwrap();
            let codigo = fonte(modulo);
            assert!(
                corpo(codigo, &format!("fn {comando}(")).contains(&format!("{funcao}(")),
                "o atendente de {comando} não chama {funcao}"
            );
            let c = corpo(codigo, &format!("pub fn {nome}("));
            let chama = c.contains("exigir_editavel") || c.contains("exigir_filhos_editaveis");
            if LACUNAS.contains(funcao) {
                assert!(!chama, "{funcao} já chama o bloqueio: tire-a das LACUNAS (E04)");
            } else {
                assert!(
                    chama,
                    "{comando} ({funcao}) mexe em dinheiro e não chama o bloqueio de registro inconsistente (R-05)"
                );
            }
        }
        for f in CONFERIDAS_ATE_A_3_5.iter().chain(LACUNAS) {
            assert!(
                COMANDOS_DE_DINHEIRO.iter().any(|(_, x)| x == f),
                "{f} perdeu a marca dinheiro em despacho::COMANDOS_DE_DINHEIRO"
            );
        }
        // Controle: uma função de leitura não precisa (e a varredura sabe diferenciar).
        assert!(!corpo(include_str!("../operacao/receber.rs"), "pub fn previa(").contains("exigir_editavel"));
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
