//! Contrato dos comandos das telas (etapa 3 da modularização): a lista dos
//! comandos que o núcleo atende, com a permissão de cada um, CONGELADA como
//! estava no `api.rs` do commit `5b580c9` (antes do despacho por domínio).
//!
//! Para cada comando ficam congelados:
//! - a classe (`api::classe_do_comando`): a ação do gerente que ele exige
//!   (`"financeiro"`, `"resultados"`, `"gerente"`...), `"leitura"` ou `"caixa"`;
//! - as marcas: `leitura` (só lê: `SO_LEITURA`, banco só para leitura nos
//!   testes e depois de um pânico), `sem_sessao` (funciona sem ninguém
//!   conectado: `SEM_SESSAO`), `do_caixa` (grava e qualquer conectado usa:
//!   `DO_CAIXA`) e `nao_repetir` (exige a chave da operação: `NAO_REPETIR`).
//!   Desde o passo 2 da etapa 3, `SO_LEITURA`, `SEM_SESSAO` e `DO_CAIXA` não
//!   são mais listas: são as classes `despacho::Classe` das tabelas
//!   (leitura, sem sessão e `Classe::Caixa`).
//!
//! Mudar qualquer um destes é mudar a regra de permissão: o teste falha. Um
//! comando novo (ou um que sumiu) também faz o teste falhar até a lista ser
//! acertada — de propósito, como `SETAS_EM_CICLO` em `dependencias.rs`.
//!
//! Como a lista foi gerada (reprodutível): no commit `5b580c9` com este
//! arquivo como no commit `1f99993` (passo 1) e `CONGELADA` vazia,
//! `RUSTINHO_COMANDOS_MOSTRAR=1 cargo test -p rustinho-nucleo --test contratos_comandos -- --nocapture`
//! imprime a lista no formato de `CONGELADA` (comandos em ordem alfabética).

use std::collections::{BTreeMap, BTreeSet};

use rustinho_nucleo::api;
use rustinho_nucleo::despacho::Classe;

/// (comando, classe, marcas) — tirado do `api.rs` em `5b580c9`.
#[rustfmt::skip]
const CONGELADA: &[(&str, &str, &[&str])] = &[
    ("aparencia_salvar", "caixa", &["sem_sessao"]),
    ("arquivos_conferir", "leitura", &["leitura"]),
    ("auditoria_listar", "simultaneidade", &["leitura"]),
    ("aviso_esconder", "caixa", &["do_caixa"]),
    ("aviso_minimizar", "caixa", &["do_caixa"]),
    ("avisos_do_dia", "leitura", &["leitura"]),
    ("backup_agora", "backup", &[]),
    ("backup_ao_fechar", "caixa", &["sem_sessao"]),
    ("backup_automatico", "caixa", &["sem_sessao"]),
    ("backup_conectar_drive", "backup", &[]),
    ("backup_conferir", "backup", &["leitura"]),
    ("backup_desligar_drive", "backup", &[]),
    ("backup_ler", "backup", &["leitura"]),
    ("backup_nuvem_lista", "backup", &["leitura"]),
    ("backup_restaurar", "backup", &["nao_repetir"]),
    ("backup_salvar", "backup", &[]),
    ("caixa_abrir", "caixa", &["do_caixa", "nao_repetir"]),
    ("caixa_estado", "leitura", &["leitura"]),
    ("caixa_fechar", "caixa", &["do_caixa", "nao_repetir"]),
    ("caixa_movimentar", "caixa", &["do_caixa", "nao_repetir"]),
    ("caixa_readmitir", "gerente", &["nao_repetir"]),
    ("caixa_resumo", "leitura", &["leitura"]),
    ("caixa_retirar", "gerente", &["nao_repetir"]),
    ("caixas_conhecidos", "leitura", &["leitura"]),
    ("categoria_salvar", "financeiro", &["nao_repetir"]),
    ("categorias", "financeiro", &["leitura"]),
    ("cheque_depositar", "financeiro", &["nao_repetir"]),
    ("cheque_desfazer", "estornar", &["nao_repetir"]),
    ("cheque_devolver", "financeiro", &["nao_repetir"]),
    ("cheque_repassar", "financeiro", &["nao_repetir"]),
    ("cheques_painel", "financeiro", &["leitura"]),
    ("cheques_previa", "leitura", &["leitura"]),
    ("codigo_ler_foto", "leitura", &["leitura", "sem_sessao"]),
    ("codigos_assumir", "gerente", &[]),
    ("codigos_confirmar_computador", "gerente", &[]),
    ("codigos_dar_novo", "gerente", &[]),
    ("codigos_definir_pendentes", "gerente", &[]),
    ("codigos_pessoa_dar_novo", "gerente", &[]),
    ("codigos_registrar_caixa_novo", "gerente", &[]),
    ("codigos_situacao", "simultaneidade", &["leitura"]),
    ("comissao_fechar", "financeiro", &["nao_repetir"]),
    ("comissao_reabrir", "financeiro", &["nao_repetir"]),
    ("comissoes", "financeiro", &["leitura"]),
    ("compra_abrir", "leitura", &["leitura"]),
    ("compra_atualizar", "caixa", &["do_caixa"]),
    ("compra_conferir_autorizacao", "caixa", &["do_caixa"]),
    ("compra_descartar", "caixa", &["do_caixa"]),
    ("compra_estornar", "estornar", &["nao_repetir"]),
    ("compra_importar", "caixa", &["do_caixa", "nao_repetir"]),
    ("compra_lancar", "caixa", &["do_caixa", "nao_repetir"]),
    ("compras_listar", "leitura", &["leitura"]),
    ("computador_salvar", "caixa", &["do_caixa"]),
    ("config_ler", "leitura", &["leitura"]),
    ("config_salvar", "configuracoes", &[]),
    ("configurar_inicio", "caixa", &["sem_sessao"]),
    ("conflito_resolver", "gerente", &[]),
    ("conflitos_listar", "simultaneidade", &["leitura"]),
    ("conflitos_manter_todos", "gerente", &[]),
    ("conta_cancelar", "estornar", &["nao_repetir"]),
    ("conta_loja_conferir", "financeiro", &[]),
    ("conta_loja_extrato", "financeiro", &["leitura"]),
    ("conta_loja_salvar", "financeiro", &["nao_repetir"]),
    ("contas_loja", "leitura", &["leitura"]),
    ("devolucao_calcular", "leitura", &["leitura"]),
    ("devolucao_desfazer", "estornar", &["nao_repetir"]),
    ("devolucao_gravar", "caixa", &["do_caixa", "nao_repetir"]),
    ("devolucao_venda", "leitura", &["leitura"]),
    ("devolucoes_listar", "leitura", &["leitura"]),
    ("dre_apurar", "resultados", &["leitura"]),
    ("dre_classificacao", "resultados", &["leitura"]),
    ("dre_classificacao_salvar", "resultados", &[]),
    ("dre_despesas", "resultados", &["leitura"]),
    ("dre_detalhe", "resultados", &["leitura"]),
    ("dre_indicadores", "resultados", &["leitura"]),
    ("dre_meta_salvar", "resultados", &[]),
    ("dre_metas", "resultados", &["leitura"]),
    ("dre_resumo", "resultados", &["leitura"]),
    ("dre_serie", "resultados", &["leitura"]),
    ("encomenda_cancelar", "caixa", &["do_caixa", "nao_repetir"]),
    ("encomenda_chegou", "caixa", &["do_caixa", "nao_repetir"]),
    ("encomenda_criar", "caixa", &["do_caixa", "nao_repetir"]),
    ("encomenda_entregar", "caixa", &["do_caixa", "nao_repetir"]),
    ("encomenda_para_venda", "leitura", &["leitura"]),
    ("encomenda_pedir", "caixa", &["do_caixa", "nao_repetir"]),
    ("encomenda_whatsapp", "caixa", &["do_caixa"]),
    ("encomendas_painel", "leitura", &["leitura"]),
    ("entrar", "caixa", &["sem_sessao"]),
    ("estado", "leitura", &["leitura", "sem_sessao"]),
    ("estoque_ajustar", "ajustar_estoque", &["nao_repetir"]),
    ("estoque_dias", "caixa", &["do_caixa"]),
    ("estoque_mais_vendidos", "leitura", &["leitura"]),
    ("estoque_parados", "leitura", &["leitura"]),
    ("estoque_sugestao", "leitura", &["leitura"]),
    ("etiqueta_previa", "leitura", &["leitura"]),
    ("etiquetas_da_compra", "leitura", &["leitura"]),
    ("etiquetas_opcoes", "leitura", &["leitura"]),
    ("financeiro_resumo", "financeiro", &["leitura"]),
    ("fiscal_automatico", "caixa", &["do_caixa"]),
    ("fiscal_conferir", "leitura", &["leitura"]),
    ("fiscal_regras_salvar", "configuracoes", &[]),
    ("fiscal_tributacao", "leitura", &["leitura"]),
    ("fluxo_caixa", "financeiro", &["leitura"]),
    ("formas_pagamento", "leitura", &["leitura"]),
    ("gerente_encerrar", "caixa", &["do_caixa"]),
    ("gerente_ler", "gerente", &["leitura"]),
    ("gerente_liberar", "caixa", &["do_caixa"]),
    ("gerente_salvar", "gerente", &[]),
    ("importar_cadastros", "configuracoes", &["nao_repetir"]),
    ("importar_previa", "leitura", &["leitura"]),
    ("impressao", "leitura", &["leitura"]),
    ("mecanicos", "leitura", &["leitura"]),
    ("menu", "leitura", &["leitura"]),
    ("menu_fixar", "caixa", &["do_caixa"]),
    ("menu_usar", "caixa", &["do_caixa"]),
    ("movimento_estornar", "estornar", &["nao_repetir"]),
    ("movimento_lancar", "financeiro", &["nao_repetir"]),
    ("numero_formatar", "leitura", &["leitura", "sem_sessao"]),
    ("orcamento_carregar", "leitura", &["leitura"]),
    ("orcamento_salvar", "caixa", &["do_caixa", "nao_repetir"]),
    ("os_abrir", "leitura", &["leitura"]),
    ("os_calcular", "leitura", &["leitura"]),
    ("os_cancelar", "caixa", &["do_caixa", "nao_repetir"]),
    ("os_listar", "leitura", &["leitura"]),
    ("os_para_venda", "leitura", &["leitura"]),
    ("os_placa", "leitura", &["leitura"]),
    ("os_rascunho_excluir", "caixa", &["do_caixa", "nao_repetir"]),
    ("os_rascunho_pode", "leitura", &["leitura"]),
    ("os_rascunho_salvar", "caixa", &["do_caixa", "nao_repetir"]),
    ("os_salvar", "caixa", &["do_caixa", "nao_repetir"]),
    ("os_situacao", "caixa", &["do_caixa", "nao_repetir"]),
    ("pagamento_estornar", "estornar", &["nao_repetir"]),
    ("pagar_alterar", "financeiro", &[]),
    ("pagar_cancelar", "financeiro", &["nao_repetir"]),
    ("pagar_ficha", "financeiro", &["leitura"]),
    ("pagar_formas", "financeiro", &["leitura"]),
    ("pagar_lancar", "financeiro", &["nao_repetir"]),
    ("pagar_pagar", "financeiro", &["nao_repetir"]),
    ("pagar_painel", "financeiro", &["leitura"]),
    ("pagar_previa", "financeiro", &["leitura"]),
    ("painel_clientes", "leitura", &["leitura"]),
    ("painel_fornecedores", "leitura", &["leitura"]),
    ("painel_pagamentos", "leitura", &["leitura"]),
    ("painel_resumo", "leitura", &["leitura"]),
    ("pendencia_arquivar", "gerente", &["nao_repetir"]),
    ("pendencias_listar", "simultaneidade", &["leitura"]),
    ("pendencias_reaplicar", "gerente", &[]),
    ("permissao", "leitura", &["leitura"]),
    ("pessoa_abrir", "leitura", &["leitura"]),
    ("pessoa_nova", "leitura", &["leitura"]),
    ("pessoa_salvar", "caixa", &["do_caixa", "nao_repetir"]),
    ("pessoas_buscar", "leitura", &["leitura"]),
    ("preco_calcular", "leitura", &["leitura", "sem_sessao"]),
    ("produto_abrir", "leitura", &["leitura"]),
    ("produto_novo", "leitura", &["leitura"]),
    ("produto_por_codigo", "leitura", &["leitura"]),
    ("produto_salvar", "caixa", &["do_caixa", "nao_repetir"]),
    ("produto_sugestoes", "leitura", &["leitura"]),
    ("produto_variacao_criar", "caixa", &["do_caixa", "nao_repetir"]),
    ("produtos_buscar", "leitura", &["leitura"]),
    ("receber_baixar", "caixa", &["do_caixa", "nao_repetir"]),
    ("receber_calcular", "leitura", &["leitura"]),
    ("receber_cliente", "leitura", &["leitura"]),
    ("receber_lancar", "caixa", &["do_caixa", "nao_repetir"]),
    ("receber_painel", "leitura", &["leitura"]),
    ("receber_previa", "leitura", &["leitura"]),
    ("recebimento_estornar", "estornar", &["nao_repetir"]),
    ("referencia_aceitar", "gerente", &[]),
    ("requisito_conferir", "leitura", &["leitura"]),
    ("sair", "caixa", &["sem_sessao"]),
    ("senha_trocar", "caixa", &["do_caixa"]),
    ("situacao_cliente", "leitura", &["leitura"]),
    ("sync_desistir_juntar", "caixa", &["sem_sessao"]),
    ("sync_juntar", "caixa", &["sem_sessao"]),
    ("sync_ler", "leitura", &["leitura", "sem_sessao"]),
    ("sync_salvar", "configuracoes", &[]),
    ("transferir", "financeiro", &["nao_repetir"]),
    ("troca_historico", "simultaneidade", &["leitura"]),
    ("unidade_salvar", "caixa", &["do_caixa", "nao_repetir"]),
    ("unidades_listar", "leitura", &["leitura"]),
    ("usuario_salvar", "usuarios", &["nao_repetir"]),
    ("usuarios_entrada", "leitura", &["leitura", "sem_sessao"]),
    ("usuarios_listar", "leitura", &["leitura"]),
    ("vales_abertos", "leitura", &["leitura"]),
    ("venda_abrir", "leitura", &["leitura"]),
    ("venda_calcular", "leitura", &["leitura"]),
    ("venda_cancelar", "cancelar_venda", &["nao_repetir"]),
    ("venda_concluir", "caixa", &["do_caixa", "nao_repetir"]),
    ("vendas_listar", "leitura", &["leitura"]),
];

/// Os comandos que a API atende hoje: as tabelas de comandos (a do serviço e
/// as dos domínios). No passo 1 da etapa 3 (commit que criou este teste, sobre
/// `5b580c9`) eles eram lidos do texto do `api.rs`, pela regra de
/// `todo_comando_tem_permissao`, e a comparação passou do mesmo jeito.
fn comandos_atuais() -> BTreeSet<String> {
    api::comandos().into_iter().map(String::from).collect()
}

/// A permissão de um comando como o núcleo a aplica hoje (a classe da tabela).
fn permissao_atual(c: &str) -> (String, Vec<&'static str>) {
    let classe = api::classe_do_comando(c).unwrap_or("SEM CLASSE").to_string();
    let mut marcas = Vec::new();
    if api::so_leitura(c) {
        marcas.push("leitura");
    }
    if api::sem_sessao(c) {
        marcas.push("sem_sessao");
    }
    if api::classe(c) == Some(Classe::Caixa) {
        marcas.push("do_caixa");
    }
    if api::NAO_REPETIR.contains(&c) {
        marcas.push("nao_repetir");
    }
    (classe, marcas)
}

#[test]
fn comandos_e_permissoes_iguais_aos_congelados() {
    let atuais: BTreeMap<String, (String, Vec<&str>)> =
        comandos_atuais().into_iter().map(|c| (c.clone(), permissao_atual(&c))).collect();
    if std::env::var_os("RUSTINHO_COMANDOS_MOSTRAR").is_some() {
        println!("== {} comandos", atuais.len());
        for (c, (classe, marcas)) in &atuais {
            let m: Vec<String> = marcas.iter().map(|x| format!("\"{x}\"")).collect();
            println!("    (\"{c}\", \"{classe}\", &[{}]),", m.join(", "));
        }
    }
    let mut vistos = BTreeSet::new();
    let mut anterior = "";
    for (c, ..) in CONGELADA {
        assert!(vistos.insert(*c), "comando repetido na lista congelada: {c}");
        assert!(anterior < *c, "a lista congelada fica em ordem alfabética: {anterior} antes de {c}");
        anterior = c;
    }
    let mut erros = Vec::new();
    for (c, classe, marcas) in CONGELADA {
        match atuais.get(*c) {
            None => erros.push(format!("{c}: o núcleo não atende mais este comando")),
            Some((k, m)) if k != classe || m.as_slice() != *marcas => {
                erros.push(format!("{c}: era {classe} {marcas:?}, agora é {k} {m:?}"))
            }
            Some(_) => {}
        }
    }
    for c in atuais.keys().filter(|c| !vistos.contains(c.as_str())) {
        erros.push(format!("{c}: comando novo, fora da lista congelada"));
    }
    assert!(erros.is_empty(), "{} diferença(s):\n{}", erros.len(), erros.join("\n"));
    assert!(CONGELADA.len() > 180, "a lista congelada tem só {}", CONGELADA.len());
}
