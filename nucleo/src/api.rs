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
    arquivos, auditoria, backup, busca, caixa, codigo, devolucoes, etiquetas, importar, cheques, comissoes, compras, config, documento, encomendas, estoque, financeiro, fiscal, impressao, menu,
    numeros, oficina, pagar, painel, pessoas, produtos, receber, reposicao, sync, texto, unidades, usuarios, vendas,
};

/// Versão do Rustinho.
pub const VERSAO: &str = env!("CARGO_PKG_VERSION");

const TENTATIVAS_MAXIMAS: u32 = 5;
/// Segundos de espera depois de muitas senhas erradas seguidas.
const ESPERA_APOS_ERROS: i64 = 30;
/// Quanto tempo vale a senha do gerente digitada no "aperto": numa tela do
/// gerente, renova a cada uso; numa ação só, acaba depois de poucos minutos.
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
];

/// Qual ação do gerente cada comando precisa.
fn acao_do_comando(comando: &str) -> Option<&'static str> {
    Some(match comando {
        "venda_cancelar" => "cancelar_venda",
        "compra_estornar" | "recebimento_estornar" | "pagamento_estornar" | "movimento_estornar" | "devolucao_desfazer"
        | "cheque_desfazer" | "conta_cancelar" => "estornar",
        "estoque_ajustar" => "ajustar_estoque",
        "backup_ler" | "backup_salvar" | "backup_agora" | "backup_conectar_drive" | "backup_desligar_drive"
        | "backup_nuvem_lista" | "backup_conferir" | "backup_restaurar" => "backup",
        "config_salvar" | "sync_salvar" | "fiscal_regras_salvar" | "importar_cadastros" => "configuracoes",
        "usuario_salvar" => "usuarios",
        "financeiro_resumo" | "pagar_painel" | "pagar_ficha" | "pagar_previa" | "pagar_lancar" | "pagar_alterar"
        | "pagar_cancelar" | "pagar_pagar" | "pagar_formas" | "conta_loja_salvar" | "conta_loja_extrato"
        | "movimento_lancar" | "transferir" | "conta_loja_conferir" | "categorias" | "categoria_salvar" | "fluxo_caixa"
        | "cheques_painel" | "cheque_depositar" | "cheque_repassar" | "cheque_devolver" | "comissoes"
        | "comissao_fechar" | "comissao_reabrir" => "financeiro",
        // A própria pílula Gerente: sempre só o gerente.
        "gerente_ler" | "gerente_salvar" => "gerente",
        _ => return None,
    })
}

/// Quem está usando o Rustinho agora.
#[derive(Debug, Clone)]
struct Sessao {
    usuario_id: String,
    nome: String,
    gerente: bool,
}

/// Senha do gerente digitada na hora, para um caixa fazer algo do gerente.
struct Liberacao {
    ate: Instant,
    /// Numa tela do gerente (Financeiro...) renova a cada uso; numa ação
    /// só (cancelar uma venda), vale poucos minutos e acaba.
    renova: bool,
}

/// O serviço que a interface usa. Guarda o banco e a sessão.
pub struct Servico {
    banco: Banco,
    sessao: Option<Sessao>,
    liberacao: Option<Liberacao>,
    /// Sincronização com os outros caixas (só com banco em arquivo).
    _motor: Option<sync::Motor>,
}

impl Servico {
    /// Abre o banco no arquivo indicado.
    pub fn abrir(caminho: &Path) -> Resultado<Servico> {
        let mut s = Self::com_banco(Banco::abrir(caminho)?);
        s._motor = Some(sync::Motor::iniciar(caminho.to_path_buf(), sync::PORTA_TCP, sync::PORTA_UDP));
        Ok(s)
    }

    /// Banco em memória (testes).
    pub fn em_memoria() -> Resultado<Servico> {
        Ok(Self::com_banco(Banco::em_memoria()?))
    }

    fn com_banco(banco: Banco) -> Servico {
        Servico {
            banco,
            sessao: None,
            liberacao: None,
            _motor: None,
        }
    }

    /// Executa um comando. Nunca entra em pânico: qualquer problema vira uma
    /// resposta de erro em JSON.
    pub fn chamar(&mut self, comando: &str, dados: &str) -> String {
        let resultado = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let dados: Value = if dados.trim().is_empty() {
                json!({})
            } else {
                serde_json::from_str(dados)?
            };
            self.executar(comando, dados)
        }));
        let resposta = match resultado {
            Ok(Ok(valor)) => json!({ "ok": true, "dados": valor }),
            Ok(Err(erro)) => json!({ "ok": false, "erro": erro.to_string(), "campo": erro.nome_do_campo() }),
            Err(_) => json!({ "ok": false, "erro": "Erro interno inesperado. Tente de novo.", "campo": null }),
        };
        resposta.to_string()
    }

    fn sessao(&self) -> Resultado<&Sessao> {
        self.sessao.as_ref().ok_or(Erro::SemSessao)
    }

    fn executar(&mut self, comando: &str, dados: Value) -> Resultado<Value> {
        match comando {
            // Sem precisar estar conectado.
            "estado" => self.estado(),
            "configurar_inicio" => self.configurar_inicio(ler(dados)?),
            "usuarios_entrada" => {
                let lista = usuarios::listar(&self.banco.conn, true)?;
                Ok(json!(lista
                    .into_iter()
                    .map(|u| json!({ "id": u.id, "nome": u.nome }))
                    .collect::<Vec<_>>()))
            }
            "entrar" => self.entrar(ler(dados)?),
            "sync_juntar" => {
                if usuarios::quantidade(&self.banco.conn)? > 0 {
                    return Err(Erro::validacao("Este caixa já tem usuários. Use Configurações para ligar a sincronização."));
                }
                let d: Juntar = ler(dados)?;
                let caixa = texto::limpar(&d.caixa);
                config::validar_nome_caixa(&caixa)?;
                sync::salvar_ajustes(&self.banco.conn, true, &d.senha, &d.enderecos)?;
                config::gravar(&self.banco.conn, "caixa_nome", &caixa)?;
                self.sync_ler()
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
                self.liberacao = None;
                if let Some(s) = self.sessao.take() {
                    auditoria::registrar(
                        &self.banco.conn,
                        &self.banco.dispositivo,
                        Some(&s.usuario_id),
                        "sair",
                        "usuario",
                        &s.usuario_id,
                        &[],
                    )?;
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

    /// Pode fazer esta ação do gerente? (gerente, ação liberada para todos na
    /// loja, ou senha do gerente digitada há pouco.)
    fn pode(&mut self, acao: &str, sessao: &Sessao) -> Resultado<bool> {
        if sessao.gerente {
            return Ok(true);
        }
        if acao != "gerente" && acoes_livres(&self.banco.conn)?.iter().any(|a| a == acao) {
            return Ok(true);
        }
        if let Some(l) = self.liberacao.as_mut() {
            if Instant::now() < l.ate {
                if l.renova {
                    l.ate = Instant::now() + LIBERACAO;
                }
                return Ok(true);
            }
            self.liberacao = None;
        }
        Ok(false)
    }

    fn exigir(&mut self, acao: &str, sessao: &Sessao) -> Resultado<()> {
        if self.pode(acao, sessao)? {
            return Ok(());
        }
        let nome = ACOES_DO_GERENTE
            .iter()
            .find(|(id, _)| *id == acao)
            .map(|(_, n)| n.to_lowercase())
            .unwrap_or_else(|| "isso".into());
        Err(Erro::campo("gerente", format!("Só o gerente pode: {nome}. Peça para o gerente digitar a senha.")))
    }

    fn executar_conectado(&mut self, comando: &str, dados: Value, sessao: &Sessao) -> Resultado<Value> {
        let disp = self.banco.dispositivo.clone();
        if let Some(acao) = acao_do_comando(comando) {
            self.exigir(acao, sessao)?;
        }
        match comando {
            // ---- Gerente ----
            "permissao" => {
                let d: Acao = ler(dados)?;
                Ok(json!({ "pode": self.pode(&d.acao, sessao)?, "gerente": sessao.gerente }))
            }
            "gerente_liberar" => {
                let d: SenhaGerente = ler(dados)?;
                self.esperar_se_errou("gerente")?;
                match usuarios::conferir_gerente(&self.banco.conn, &d.senha)? {
                    Some(g) => {
                        usuarios::limpar_falhas(&self.banco.conn, "gerente")?;
                        self.liberacao = Some(Liberacao {
                            ate: Instant::now() + if d.curta { LIBERACAO_CURTA } else { LIBERACAO },
                            renova: !d.curta,
                        });
                        auditoria::registrar(
                            &self.banco.conn,
                            &disp,
                            Some(&sessao.usuario_id),
                            "gerente.liberar",
                            "usuario",
                            &g.id,
                            &[format!("{} liberou para {}", g.nome, sessao.nome)],
                        )?;
                        Ok(json!({ "nome": g.nome }))
                    }
                    None => {
                        usuarios::anotar_falha(&self.banco.conn, "gerente", ESPERA_APOS_ERROS)?;
                        Err(Erro::campo("senha", "Senha do gerente errada."))
                    }
                }
            }
            "gerente_encerrar" => {
                self.liberacao = None;
                Ok(Value::Null)
            }
            "gerente_ler" => self.gerente_ler(),
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
                        let u = usuarios::buscar(&tx, id)?;
                        detalhes.push(format!("{}: {}", u.nome, if gerente { "gerente" } else { "caixa" }));
                    }
                }
                config::gravar(&tx, "gerente_livres", &livres.join(","))?;
                if !detalhes.is_empty() {
                    auditoria::registrar(&tx, &disp, Some(&sessao.usuario_id), "gerente.salvar", "config", "gerente", &detalhes)?;
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
                config::gravar(&self.banco.conn, "caixa_nome", &caixa)?;
                config::gravar(&self.banco.conn, "imprimir_direto", if d.imprimir_direto { "1" } else { "0" })?;
                self.config_ler()
            }

            "menu" => Ok(json!(menu::menu(&self.banco.conn, &sessao.usuario_id)?)),
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
            "produto_novo" => Ok(json!(produtos::ficha_nova(&self.banco.conn)?)),
            "produto_abrir" => {
                let d: Id = ler(dados)?;
                Ok(json!(produtos::ficha(&self.banco.conn, &d.id)?))
            }
            "produto_salvar" => {
                let ficha: produtos::ProdutoFicha = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let id = produtos::salvar(&tx, &disp, &sessao.usuario_id, &ficha)?;
                tx.commit()?;
                Ok(json!(produtos::ficha(&self.banco.conn, &id)?))
            }
            "produto_sugestoes" => {
                let mut v = json!(produtos::sugestoes(&self.banco.conn)?);
                let lista = |l: &[(&str, &str)]| json!(l.iter().map(|(c, t)| json!({ "valor": c, "texto": t })).collect::<Vec<_>>());
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
                Ok(json!(produtos::ficha(&self.banco.conn, &id)?))
            }
            "estoque_ajustar" => {
                let d: AjusteEstoque = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                produtos::ficha(&tx, &d.produto_id)?; // confere se a peça existe
                estoque::ajustar(&tx, &disp, &sessao.usuario_id, &d.produto_id, &d.quantidade, &d.motivo)?;
                tx.commit()?;
                Ok(json!(produtos::ficha(&self.banco.conn, &d.produto_id)?))
            }

            "usuarios_listar" => {
                let lista = usuarios::listar(&self.banco.conn, false)?;
                Ok(json!(lista
                    .into_iter()
                    .map(|u| {
                        let voce = u.id == sessao.usuario_id;
                        let mut v = json!(u);
                        v["voce"] = json!(voce);
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
                usuarios::alterar(&self.banco.conn, &u.id, &u.nome, u.ativo, Some(&d.nova))
                    .map_err(renomear_campo("senha", "nova"))?;
                auditoria::registrar(
                    &self.banco.conn,
                    &disp,
                    Some(&sessao.usuario_id),
                    "usuario.alterar",
                    "usuario",
                    &u.id,
                    &["Senha trocada".into()],
                )?;
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
                let lista = arquivos::xmls(std::slice::from_ref(&d.caminho))?;
                let pacote = lista.len() > 1 || lista.iter().any(|x| x.nome.contains(" › "));
                let mut fichas = Vec::new();
                let mut erros = Vec::new();
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
                        Err(e) if pacote && ["não é de uma NF-e", "resumo ou um evento", "CT-e", "não é um XML válido"].iter().any(|t| e.to_string().contains(t)) => {}
                        Err(e) => erros.push(format!("{}: {e}", x.nome)),
                    }
                }
                if fichas.is_empty() && erros.is_empty() {
                    return Err(Erro::validacao("Não achei nenhuma NF-e nesse arquivo."));
                }
                Ok(json!({ "compras": fichas, "erros": erros }))
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
                let tx = self.banco.conn.transaction()?;
                compras::atualizar(&tx, &d)?;
                let r = compras::lancar(&tx, &disp, &sessao.usuario_id, &d.id)?;
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
            "compra_descartar" => {
                let d: Id = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                compras::excluir_rascunho(&tx, &d.id)?;
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
                Ok(json!({ "alteradas": n, "conferencia": fiscal::conferir(&self.banco.conn, &fiscal::hoje(&self.banco.conn)?)? }))
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
                Ok(json!(fiscal::tributacao(&self.banco.conn, &uf, &d.cest, &d.csosn, &fiscal::hoje(&self.banco.conn)?)?))
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
                // Leitor de código de barras: só o código exato.
                let d: BuscaProdutos = ler(dados)?;
                let lista = busca::buscar(&self.banco.conn, &d.texto, false, 2)?;
                Ok(match lista.first() {
                    Some(p) if p.codigo_exato => json!({ "id": p.id }),
                    _ => json!({ "id": null }),
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
                caixa::movimentar_com_conta(&tx, &disp, &sessao.usuario_id, &d.tipo, &d.valor, &d.motivo, Some(&d.conta))?;
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
            "avisos_do_dia" => Ok(json!(crate::avisos::do_dia(&self.banco.conn)?)),
            "aviso_esconder" => {
                let d: Value = ler(dados)?;
                crate::avisos::esconder(&self.banco.conn, d["id"].as_str().unwrap_or_default())?;
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
            "importar_cadastros" => {
                let d: importar::Entrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                let r = importar::importar(&tx, &disp, &sessao.usuario_id, &d)?;
                tx.commit()?;
                Ok(json!(r))
            }

            // Painel: como a loja está indo
            "painel_resumo" => Ok(json!(painel::resumo(&self.banco.conn)?)),
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
                Ok(json!(reposicao::salvar_dias(&self.banco.conn, &disp, &sessao.usuario_id, &d.qual, &d.dias)?))
            }
            "estoque_mais_vendidos" => {
                let d: MaisVendidos = ler(dados)?;
                Ok(json!(reposicao::mais_vendidos(&self.banco.conn, d.dias, &d.ordem, &d.texto)?))
            }
            "estoque_parados" => {
                let d: Filtro = ler(dados)?;
                Ok(json!(reposicao::parados(&self.banco.conn, &d.texto)?))
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
                let d: Id = ler(dados)?;
                Ok(json!({ "url": encomendas::whatsapp(&self.banco.conn, &d.id)? }))
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
                Ok(json!(financeiro::contas(&self.banco.conn, d.quais == "todas")?))
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
                        (impressao::orcamento(conn, &d.id)?, format!("orcamento-{}-{}", v.numero, texto::compactar(&v.caixa)))
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
                    "lista_compra" => (impressao::lista_compra(conn)?, format!("lista-de-compra-{}", fiscal::hoje(conn)?)),
                    "os" => {
                        let n: (i64, String) =
                            conn.query_row("SELECT numero, caixa_nome FROM ordem_servico WHERE id = ?1", [&d.id], |l| Ok((l.get(0)?, l.get(1)?)))?;
                        (impressao::ordem_servico(conn, &d.id)?, format!("os-{}-{}", n.0, texto::compactar(&n.1)))
                    }
                    "devolucao" => {
                        let n: (i64, String) = conn.query_row("SELECT numero, caixa_nome FROM devolucao WHERE id = ?1", [&d.id], |l| {
                            Ok((l.get(0)?, l.get(1)?))
                        })?;
                        (impressao::devolucao(conn, &d.id)?, format!("devolucao-{}-{}", n.0, texto::compactar(&n.1)))
                    }
                    "vale" => {
                        let n: (i64, String) =
                            conn.query_row("SELECT numero, caixa_nome FROM vale WHERE id = ?1", [&d.id], |l| Ok((l.get(0)?, l.get(1)?)))?;
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
                let senha = if d.senha.is_empty() { sync::ajustes(&self.banco.conn)?.senha } else { d.senha };
                sync::salvar_ajustes(&self.banco.conn, d.ligado, &senha, &d.enderecos)?;
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
                let senha = if d.senha.is_empty() { backup::ajustes(&self.banco.conn, None)?.senha } else { d.senha };
                Ok(json!(backup::conferir(std::path::Path::new(&d.arquivo), &senha)?))
            }
            "backup_restaurar" => {
                let d: Restaurar = ler(dados)?;
                let caminho = self.banco.caminho.clone();
                let info = backup::restaurar(&mut self.banco.conn, caminho.as_deref(), &d.arquivo, d.nuvem, &d.senha)?;
                auditoria::registrar(&self.banco.conn, &disp, Some(&sessao.usuario_id), "backup.restaurar", "config", &info.quando, &[])?;
                // Os usuários podem ser outros agora: entra de novo.
                self.sessao = None;
                Ok(json!(info))
            }

            "config_ler" => self.config_ler(),
            "config_salvar" => {
                let d: config::ConfigEntrada = ler(dados)?;
                let tx = self.banco.conn.transaction()?;
                config::salvar(&tx, &d)?;
                receber::salvar_taxas(&tx, &d.crediario_multa, &d.crediario_juros, &d.crediario_carencia)?;
                tx.commit()?;
                self.config_ler()
            }

            _ => Err(Erro::Interno(format!("comando desconhecido: {comando}"))),
        }
    }

    fn caminho_banco(&self) -> Resultado<std::path::PathBuf> {
        self.banco
            .caminho
            .clone()
            .ok_or_else(|| Erro::validacao("Backup só funciona com o banco num arquivo."))
    }

    fn sync_ler(&self) -> Resultado<Value> {
        let conn = &self.banco.conn;
        let a = sync::ajustes(conn)?;
        Ok(json!({
            "ligado": a.ligado,
            "temSenha": !a.senha.is_empty(),
            "enderecos": a.enderecos.join(", "),
            "caixa": a.nome,
            "pares": sync::pares(conn)?,
            "meuEndereco": endereco_local(),
            "porta": sync::PORTA_TCP,
            "usuarios": usuarios::quantidade(conn)?,
        }))
    }

    fn estado(&self) -> Resultado<Value> {
        let conn = &self.banco.conn;
        Ok(json!({
            "primeiraVez": usuarios::quantidade(conn)? == 0,
            "loja": config::nome_loja(conn)?,
            "caixa": config::nome_caixa(conn)?,
            "usuario": self.sessao.as_ref().map(|s| json!({ "id": s.usuario_id, "nome": s.nome, "gerente": s.gerente })),
            "versao": VERSAO,
            "etapa": menu::ETAPA_ATUAL,
            "validadeOrcamento": config::validade_orcamento(conn)?,
            "escuro": config::ler(conn, "tela_escura")?.as_deref() == Some("1"),
        }))
    }

    fn configurar_inicio(&mut self, d: Inicio) -> Resultado<Value> {
        if usuarios::quantidade(&self.banco.conn)? > 0 {
            return Err(Erro::validacao("O Rustinho já foi configurado neste computador."));
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
        let u = usuarios::criar(&tx, &disp, &d.nome, &d.senha, true)?;
        // Quem configura a loja é o gerente.
        usuarios::definir_gerente(&tx, &u.id, true)?;
        auditoria::registrar(&tx, &disp, Some(&u.id), "usuario.criar", "usuario", &u.id, &[])?;
        usuarios::registrar_acesso(&tx, &u.id)?;
        tx.commit()?;
        self.liberacao = None;
        self.sessao = Some(Sessao {
            usuario_id: u.id,
            nome: u.nome,
            gerente: true,
        });
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
                usuarios::limpar_falhas(&self.banco.conn, &d.id)?;
                usuarios::registrar_acesso(&self.banco.conn, &u.id)?;
                auditoria::registrar(
                    &self.banco.conn,
                    &self.banco.dispositivo,
                    Some(&u.id),
                    "entrar",
                    "usuario",
                    &u.id,
                    &[],
                )?;
                self.liberacao = None;
                self.sessao = Some(Sessao {
                    usuario_id: u.id,
                    nome: u.nome,
                    gerente: u.gerente,
                });
                self.estado()
            }
            Err(e) => {
                if e.nome_do_campo() == Some("senha") {
                    usuarios::anotar_falha(&self.banco.conn, &d.id, ESPERA_APOS_ERROS)?;
                }
                Err(e)
            }
        }
    }

    /// Nome e papel podem ter mudado (por este ou por outro caixa).
    fn atualizar_sessao(&mut self) -> Resultado<()> {
        if let Some(s) = self.sessao.as_mut() {
            let u = usuarios::buscar(&self.banco.conn, &s.usuario_id)?;
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
        if mexe_em_gerente && !sessao.gerente && self.liberacao.as_ref().map(|l| Instant::now() >= l.ate).unwrap_or(true) {
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
            let u = usuarios::criar(&tx, &disp, &d.nome, &d.senha, d.ativo)?;
            usuarios::definir_comissao(&tx, &u.id, &d.comissao)?;
            if d.gerente == Some(true) {
                usuarios::definir_gerente(&tx, &u.id, true)?;
            }
            auditoria::registrar(&tx, &disp, Some(&sessao.usuario_id), "usuario.criar", "usuario", &u.id, &[])?;
            usuarios::buscar(&tx, &u.id)?
        } else {
            let antes = usuarios::buscar(&tx, &d.id)?;
            let senha = if tem_senha { Some(d.senha.as_str()) } else { None };
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
            if !detalhes.is_empty() {
                auditoria::registrar(&tx, &disp, Some(&sessao.usuario_id), "usuario.alterar", "usuario", &u.id, &detalhes)?;
            }
            u
        };
        tx.commit()?;
        self.atualizar_sessao()?;
        Ok(json!(usuarios::buscar(&self.banco.conn, &u.id)?))
    }

    fn config_ler(&self) -> Resultado<Value> {
        let conn = &self.banco.conn;
        let taxas = receber::taxas_texto(conn)?;
        Ok(json!({
            "loja": config::nome_loja(conn)?,
            "caixa": config::nome_caixa(conn)?,
            "tabelas": config::tabelas_preco(conn)?,
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
}

#[derive(Deserialize)]
struct SenhaGerente {
    senha: String,
    #[serde(default)]
    curta: bool,
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
fn ler<T: DeserializeOwned>(dados: Value) -> Resultado<T> {
    Ok(serde_json::from_value(dados)?)
}

/// Troca o nome do campo de um erro (ex.: "senha" → "nova").
fn renomear_campo(de: &'static str, para: &'static str) -> impl Fn(Erro) -> Erro {
    move |e| match e {
        Erro::Validacao { campo: Some(c), mensagem } if c == de => Erro::Validacao {
            campo: Some(para.to_string()),
            mensagem,
        },
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
