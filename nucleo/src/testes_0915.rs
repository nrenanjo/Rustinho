//! Testes da 0.9.15: migração 0023 com recuperação (inclusive com o processo
//! morto no meio), rascunho de OS, compatibilidade entre caixas de versões
//! diferentes, pendências da troca de dados, retirada de caixa, notificações
//! e a chave da operação contra o duplo clique.
//!
//! Os testes que derrubam o processo abrem o próprio programa de testes de
//! novo como "filho" (com a variável `RUSTINHO_T0915_PASTA`): o filho morre
//! (`abort`, como falta de luz) no ponto pedido e o pai confere o banco ao
//! abrir de novo. Sem a variável, o teste "filho" não faz nada.

use std::path::{Path, PathBuf};
use std::process::Command;

use rusqlite::{params, Connection};
use serde_json::{json, Value};

use crate::banco::{self, Banco};
use crate::sync::{self, Alteracao, AvancoCursor};
use crate::{avisos, config, migracao, oficina, usuarios};

const PASTA_FILHO: &str = "RUSTINHO_T0915_PASTA";

fn pasta_nova(nome: &str) -> PathBuf {
    let p =
        std::env::temp_dir().join(format!("rustinho-0915-{nome}-{}-{}", std::process::id(), crate::tempo::novo_id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// Banco na estrutura 22 (a da 0.9.14), com OS, itens e uma referência que
/// já não batia (item de uma OS que não existe: precisa continuar igual).
fn banco_v22(pasta: &Path) -> PathBuf {
    let db = pasta.join("rustinho.db");
    let mut c = Connection::open(&db).unwrap();
    c.pragma_update(None, "journal_mode", "WAL").unwrap();
    banco::registrar_funcoes(&c).unwrap();
    banco::migrar_ate(&mut c, 22).unwrap();
    let v: i64 = c.query_row("PRAGMA user_version", [], |l| l.get(0)).unwrap();
    assert_eq!(v, 22);
    c.execute("INSERT INTO config (chave, valor) VALUES ('dispositivo_id', 'disp-v22')", []).unwrap();
    for n in 1..=30 {
        c.execute(
            "INSERT INTO ordem_servico (id, numero, dispositivo, caixa_nome, situacao, cliente_nome, placa, criado_em, atualizado_em)
             VALUES (?1, ?2, 'disp-v22', 'Caixa 1', ?3, 'Cliente ' || ?2, 'ABC1D2' || (?2 % 10), '2026-10-01', '2026-10-01')",
            params![format!("os-{n}"), n, ["orcamento", "aprovada", "pronta", "entregue", "cancelada"][n as usize % 5]],
        )
        .unwrap();
        for i in 0..3 {
            c.execute(
                "INSERT INTO ordem_servico_item (id, ordem_id, ordem, tipo, descricao, quantidade, preco)
                 VALUES (?1, ?2, ?3, 'servico', 'Serviço ' || ?3, 1000, ?4)",
                params![format!("it-{n}-{i}"), format!("os-{n}"), i, 1000 * n + i],
            )
            .unwrap();
        }
    }
    c.pragma_update(None, "foreign_keys", false).unwrap();
    c.execute(
        "INSERT INTO ordem_servico_item (id, ordem_id, ordem, tipo, descricao, quantidade, preco)
         VALUES ('it-orfao', 'os-que-nao-existe', 0, 'servico', 'Órfão', 1000, 1)",
        [],
    )
    .unwrap();
    db
}

fn contar(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |l| l.get(0)).unwrap()
}

/// Confere o banco migrado: versão, itens, ligação com CASCADE, gatilhos e a
/// referência órfã de antes (igual, nem nova nem sumida).
fn conferir_migrado(db: &Path) {
    let b = Banco::abrir(db).unwrap();
    let c = &b.conn;
    assert_eq!(contar(c, "PRAGMA user_version"), banco::versao_da_estrutura() as i64);
    assert_eq!(c.query_row("PRAGMA integrity_check", [], |l| l.get::<_, String>(0)).unwrap(), "ok");
    assert_eq!(contar(c, "SELECT COUNT(*) FROM ordem_servico"), 30);
    assert_eq!(contar(c, "SELECT COUNT(*) FROM ordem_servico_item"), 91, "90 itens + o órfão de antes");
    assert_eq!(
        contar(c, "SELECT COALESCE(SUM(preco), 0) FROM ordem_servico_item WHERE id <> 'it-orfao'"),
        (1..=30).map(|n| 3 * 1000 * n + 3).sum::<i64>()
    );
    assert_eq!(contar(c, "SELECT COUNT(*) FROM pragma_foreign_key_check"), 1, "só a referência órfã de antes");
    banco::conferir_estrutura(c).unwrap();
    for sufixo in ["i", "u", "d"] {
        assert_eq!(
            contar(c, &format!("SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name = 'sync_ordem_servico_{sufixo}'")),
            1,
            "gatilho {sufixo} da OS recriado"
        );
    }
    // O gatilho recriado grava a alteração com a operação.
    let antes = contar(c, "SELECT COUNT(*) FROM alteracao WHERE tabela = 'ordem_servico'");
    c.execute("UPDATE ordem_servico SET km = '1' WHERE id = 'os-1'", []).unwrap();
    assert_eq!(contar(c, "SELECT COUNT(*) FROM alteracao WHERE tabela = 'ordem_servico'"), antes + 1);
    assert_eq!(contar(c, "SELECT COUNT(*) FROM alteracao WHERE tabela = 'ordem_servico' AND operacao_id IS NULL AND seq = (SELECT MAX(seq) FROM alteracao)"), 0);
    // O CASCADE continua valendo para exclusões futuras.
    c.execute("DELETE FROM ordem_servico WHERE id = 'os-2'", []).unwrap();
    assert_eq!(contar(c, "SELECT COUNT(*) FROM ordem_servico_item WHERE ordem_id = 'os-2'"), 0);
    // E aceita rascunho.
    c.execute("UPDATE ordem_servico SET situacao = 'rascunho' WHERE id = 'os-3'", []).unwrap();
}

fn copias_em(pasta: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(pasta)
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

#[test]
fn migracao_0023_preserva_tudo_e_guarda_a_copia() {
    let pasta = pasta_nova("migracao");
    let db = banco_v22(&pasta);
    conferir_migrado(&db);
    assert!(!migracao::arquivo_de_estado(&db).exists(), "estado apagado no fim");
    let guardadas = copias_em(&pasta.join("pre-migracao"));
    assert_eq!(guardadas.len(), 1, "a cópia de antes fica em pre-migracao/: {guardadas:?}");
    // A cópia é a 22 com os dados de antes.
    let copia = Connection::open(pasta.join("pre-migracao").join(&guardadas[0])).unwrap();
    assert_eq!(contar(&copia, "PRAGMA user_version"), 22);
    assert_eq!(contar(&copia, "SELECT COUNT(*) FROM ordem_servico_item"), 91);
    let _ = std::fs::remove_dir_all(&pasta);
}

#[test]
fn chaves_estrangeiras_voltam_ligadas_depois_da_migracao() {
    let mut c = Connection::open_in_memory().unwrap();
    banco::registrar_funcoes(&c).unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    banco::migrar(&mut c).unwrap();
    assert_eq!(contar(&c, "PRAGMA foreign_keys"), 1);
    assert_eq!(contar(&c, "PRAGMA legacy_alter_table"), 0);
    banco::conferir_estrutura(&c).unwrap();
}

/// O filho: abre o banco com o ponto de queda pedido (morre lá dentro). O
/// erro da abertura, se houver, fica em `erro-do-filho.txt` (para o pai ler).
#[test]
fn filho_migracao() {
    let Some(pasta) = std::env::var_os(PASTA_FILHO).map(PathBuf::from) else { return };
    if let Err(e) = Banco::abrir(&pasta.join("rustinho.db")) {
        let _ = std::fs::write(pasta.join("erro-do-filho.txt"), e.to_string());
    }
}

fn rodar_filho(pasta: &Path, variavel: &str, valor: &str) -> std::process::ExitStatus {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "testes_0915::filho_migracao", "--nocapture", "--test-threads=1"])
        .env(PASTA_FILHO, pasta)
        .env(variavel, valor)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
}

/// Queda de verdade (processo morto) em cada ponto da atualização: ao abrir
/// de novo, termina sem perder nada, na versão nova, com a cópia guardada.
#[test]
fn queda_no_meio_da_migracao_em_cada_ponto() {
    for ponto in ["copiando", "antes_de_migrar", "dentro_da_transacao", "apos_commit", "validando"] {
        let pasta = pasta_nova(&format!("queda-{ponto}"));
        let db = banco_v22(&pasta);
        let s = rodar_filho(&pasta, "RUSTINHO_TESTE_MIGRACAO_CAIR", ponto);
        assert!(!s.success(), "{ponto}: o filho devia ter caído");
        assert!(migracao::arquivo_de_estado(&db).exists(), "{ponto}: o estado ficou anotado");
        let estado = migracao::ler_estado(&db).unwrap().unwrap();
        if ponto == "apos_commit" || ponto == "validando" {
            // Depois do COMMIT: o banco já está na versão nova, sem a conferência.
            let c = Connection::open(&db).unwrap();
            assert_eq!(contar(&c, "PRAGMA user_version"), banco::versao_da_estrutura() as i64, "{ponto}");
            assert!(pasta.join(&estado.copia).exists(), "{ponto}: a cópia de recuperação continua lá");
        }
        if ponto == "dentro_da_transacao" {
            let c = Connection::open(&db).unwrap();
            assert_eq!(contar(&c, "PRAGMA user_version"), 22, "a transação não confirmada foi desfeita");
        }
        // Abre de novo (sem queda): termina e confere.
        conferir_migrado(&db);
        assert!(!migracao::arquivo_de_estado(&db).exists(), "{ponto}: terminou");
        assert_eq!(copias_em(&pasta.join("pre-migracao")).len(), 1, "{ponto}: uma cópia guardada");
        let _ = std::fs::remove_dir_all(&pasta);
    }
}

/// A conferência depois do COMMIT falha: a cópia volta (pelo SHA-256), o
/// banco migrado fica guardado e o Rustinho não tenta de novo sozinho.
#[test]
fn falha_na_conferencia_depois_do_commit_poe_a_copia_de_volta() {
    let pasta = pasta_nova("falha");
    let db = banco_v22(&pasta);
    let s = rodar_filho(&pasta, "RUSTINHO_TESTE_MIGRACAO_FALHAR", "conferencia");
    assert!(s.success(), "o filho termina (com o erro), sem cair");
    let estado = migracao::ler_estado(&db).unwrap().unwrap();
    assert_eq!(estado.fase, migracao::Fase::Falhou);
    {
        let c = Connection::open(&db).unwrap();
        assert_eq!(contar(&c, "PRAGMA user_version"), 22, "o banco de antes voltou");
        assert_eq!(contar(&c, "SELECT COUNT(*) FROM ordem_servico_item"), 91);
    }
    assert!(pasta.join(&estado.guardado_em).exists(), "o banco que falhou fica guardado");
    // Sem pedido de nova tentativa: continua recusando, sem mexer.
    let e = Banco::abrir(&db).err().expect("não abre");
    assert!(e.to_string().contains("posto de volta"), "{e}");
    assert!(migracao::liberar_nova_tentativa(&db).unwrap());
    conferir_migrado(&db);
    let _ = std::fs::remove_dir_all(&pasta);
}

/// RVI-03: a cópia de antes da atualização não sai, com o banco íntegro
/// (disco cheio, por exemplo; aqui, falha de teste): o banco não é mexido, a
/// mensagem diz o que houve, nada fica pela metade (nem cópia, nem estado de
/// atualização) e a próxima abertura atualiza normalmente.
#[test]
fn rvi03_copia_de_antes_falha_sem_mexer_no_banco() {
    use crate::testes_modo_seguro::sha;
    let pasta = pasta_nova("copia-falha");
    let db = banco_v22(&pasta);
    let antes = sha(&db);
    let sem_diario = |v: Vec<String>| -> Vec<String> {
        v.into_iter().filter(|n| !n.ends_with("-wal") && !n.ends_with("-shm") && n != "erro-do-filho.txt").collect()
    };
    let lista = sem_diario(copias_em(&pasta));
    let s = rodar_filho(&pasta, "RUSTINHO_TESTE_MIGRACAO_FALHAR", "copia");
    assert!(s.success(), "o filho termina (com o erro), sem cair");
    let erro = std::fs::read_to_string(pasta.join("erro-do-filho.txt")).expect("a abertura deu erro");
    assert!(erro.contains("cópia de segurança") && erro.contains("O banco não foi mexido"), "{erro}");
    assert_eq!(sha(&db), antes, "o banco não mudou");
    assert!(!migracao::arquivo_de_estado(&db).exists(), "nada ficou anotado pela metade");
    assert_eq!(sem_diario(copias_em(&pasta)), lista, "nenhuma cópia pela metade ficou");
    {
        let c = Connection::open(&db).unwrap();
        assert_eq!(contar(&c, "PRAGMA user_version"), 22, "não migrou");
    }
    // Sem a falha: atualiza e confere, como sempre.
    conferir_migrado(&db);
    assert_eq!(copias_em(&pasta.join("pre-migracao")).len(), 1);
    let _ = std::fs::remove_dir_all(&pasta);
}

// ---------------------------------------------------------------------------
// Rascunho de OS
// ---------------------------------------------------------------------------

fn entrada_rascunho() -> oficina::Entrada {
    oficina::Entrada {
        cliente_nome: "Carlos".into(),
        placa: "ABC1".into(),
        defeito: "Barulho".into(),
        itens: vec![oficina::ItemEntradaOs {
            tipo: "servico".into(),
            descricao: "Revisão".into(),
            preco: "50".into(),
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[test]
fn rascunho_salva_reabre_e_vira_orcamento_sem_duplicar() {
    let b = Banco::em_memoria().unwrap();
    let (c, d) = (&b.conn, b.dispositivo.as_str());
    let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
    // Rascunho sem cliente nem carro também vale.
    let vazio = oficina::Entrada::default();
    let r0 = oficina::salvar_rascunho(c, d, &u, &vazio).unwrap();
    oficina::excluir_rascunho(c, d, &u, &r0).unwrap();
    assert_eq!(contar(c, "SELECT COUNT(*) FROM ordem_servico"), 0);

    let id = oficina::salvar_rascunho(c, d, &u, &entrada_rascunho()).unwrap();
    let f = oficina::ficha(c, &id).unwrap();
    assert_eq!((f.situacao.as_str(), f.numero, f.nome_situacao.as_str()), ("rascunho", 0, "Rascunho"));
    assert_eq!(f.calculo.itens.len(), 1);
    assert_eq!(f.placa, "ABC1", "o que foi digitado fica, mesmo sem a placa completa");
    let abertas = oficina::listar(c, "abertas", "").unwrap();
    assert!(abertas.iter().any(|o| o.id == id && o.nome_situacao == "Rascunho"), "aparece em Abertas");
    assert!(oficina::listar(c, "todas", "").unwrap().iter().any(|o| o.id == id), "e em Todas");
    assert!(oficina::listar(c, "prontas", "").unwrap().is_empty());
    // Continuar o mesmo rascunho.
    let mut e = entrada_rascunho();
    e.id = id.clone();
    e.km = "85000".into();
    e.itens.push(oficina::ItemEntradaOs {
        tipo: "servico".into(),
        descricao: "Alinhamento".into(),
        preco: "30".into(),
        ..Default::default()
    });
    assert_eq!(oficina::salvar_rascunho(c, d, &u, &e).unwrap(), id);
    assert_eq!(oficina::ficha(c, &id).unwrap().calculo.itens.len(), 2);
    // Rascunho não muda de situação, não cancela, não vira venda.
    assert!(oficina::mudar(c, d, &u, &id, "aprovada").is_err());
    assert!(oficina::cancelar(c, d, &u, &id, "x").is_err());
    assert!(oficina::para_venda(c, &id).is_err());
    // Salvar de verdade: exige placa válida; vira orçamento com número, o mesmo registro.
    assert!(oficina::salvar(c, d, &u, &e).is_err(), "placa incompleta não passa ao salvar de verdade");
    e.placa = "ABC1D23".into();
    let id2 = oficina::salvar(c, d, &u, &e).unwrap();
    assert_eq!(id2, id);
    let f = oficina::ficha(c, &id).unwrap();
    assert_eq!((f.situacao.as_str(), f.numero), ("orcamento", 1));
    assert_eq!(contar(c, "SELECT COUNT(*) FROM ordem_servico"), 1, "sem duplicar");
    assert!(oficina::salvar_rascunho(c, d, &u, &e).is_err(), "OS salva não volta a rascunho");
    assert!(oficina::excluir_rascunho(c, d, &u, &id).is_err(), "OS salva não se exclui");
}

#[test]
fn chave_da_operacao_recusa_o_clique_repetido() {
    let mut s = crate::Servico::em_memoria().unwrap();
    let ok = |s: &mut crate::Servico, cmd: &str, d: Value| -> Value {
        let r: Value = serde_json::from_str(&s.chamar(cmd, &d.to_string())).unwrap();
        assert_eq!(r["ok"], true, "{cmd}: {r}");
        r["dados"].clone()
    };
    ok(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
    );
    let mut e = serde_json::to_value(json!({
        "clienteNome": "Carlos", "placa": "ABC1D23",
        "itens": [{ "tipo": "servico", "descricao": "Revisão", "preco": "50" }],
        "chaveOperacao": "op-1"
    }))
    .unwrap();
    ok(&mut s, "os_salvar", e.clone());
    let r: Value = serde_json::from_str(&s.chamar("os_salvar", &e.to_string())).unwrap();
    assert_eq!(r["ok"], false);
    assert_eq!(r["campo"], "repetida", "{r}");
    e["chaveOperacao"] = json!("op-2");
    ok(&mut s, "os_salvar", e.clone());
    let n: Value = ok(&mut s, "os_listar", json!({ "quais": "todas" }));
    assert_eq!(n["itens"].as_array().unwrap().len(), 2, "a repetida não gravou");
    // Sem chave (FALHA-04): recusado, nada gravado (todas as telas mandam a chave).
    e.as_object_mut().unwrap().remove("chaveOperacao");
    let r: Value = serde_json::from_str(&s.chamar("os_salvar", &e.to_string())).unwrap();
    assert_eq!(r["campo"], "chave", "{r}");
    let n: Value = ok(&mut s, "os_listar", json!({ "quais": "todas" }));
    assert_eq!(n["itens"].as_array().unwrap().len(), 2, "sem chave não gravou");
}

// ---------------------------------------------------------------------------
// Compatibilidade, pendências e retirada
// ---------------------------------------------------------------------------

fn com_par(b: &Banco, dispositivo: &str, nome: &str) {
    b.conn
        .execute(
            "INSERT INTO sync_par (dispositivo, nome, endereco) VALUES (?1, ?2, '10.0.0.2:47810')",
            params![dispositivo, nome],
        )
        .unwrap();
}

fn confirmar(b: &Banco, dispositivo: &str, dias_atras: i64) {
    sync::registrar_versao(&b.conn, dispositivo, &json!({ "esquema": 23, "recursos": ["pendencias-v1"] })).unwrap();
    b.conn
        .execute(
            "UPDATE sync_par SET compativel_em = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?2) WHERE dispositivo = ?1",
            params![dispositivo, format!("-{dias_atras} days")],
        )
        .unwrap();
}

#[test]
fn prazo_de_sete_dias_e_caixa_antigo_bloqueiam_rascunho_novo() {
    let b = Banco::em_memoria().unwrap();
    let (c, d) = (&b.conn, b.dispositivo.as_str());
    let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
    assert!(sync::bloqueios_de_rascunho(c).unwrap().is_empty(), "sozinho, pode");
    com_par(&b, "caixa-2", "Caixa 2");
    // Nunca confirmou: bloqueia.
    assert_eq!(sync::bloqueios_de_rascunho(c).unwrap().len(), 1);
    assert!(oficina::salvar_rascunho(c, d, &u, &entrada_rascunho()).is_err());
    confirmar(&b, "caixa-2", 2);
    let id = oficina::salvar_rascunho(c, d, &u, &entrada_rascunho()).unwrap();
    // Passou do prazo: bloqueia rascunho NOVO, o existente continua editável.
    confirmar(&b, "caixa-2", 8);
    let bloqueio = sync::bloqueios_de_rascunho(c).unwrap();
    assert_eq!(bloqueio.len(), 1);
    assert_eq!(bloqueio[0].situacao, "vencido");
    assert_eq!(bloqueio[0].dias_sem_confirmar, Some(8));
    assert!(oficina::salvar_rascunho(c, d, &u, &entrada_rascunho()).is_err());
    let mut e = entrada_rascunho();
    e.id = id.clone();
    assert_eq!(oficina::salvar_rascunho(c, d, &u, &e).unwrap(), id);
    // A notificação é de Atenção, com os dados reais, e não se esconde.
    let lista = avisos::do_dia(c).unwrap();
    let a = lista.iter().find(|a| a.id == "compatibilidade.caixa-2").expect("notificação");
    assert_eq!(a.categoria, avisos::ATENCAO);
    assert!(a.descricao.contains("Caixa 2") && a.descricao.contains("8 dias"), "{}", a.descricao);
    assert_eq!(a.acao, "Ver caixas");
    assert_eq!(a.secao, "caixas");
    // 0.9.16: informativa e atenção começam minimizadas (o quadrado).
    assert!(a.minimizado, "atenção começa minimizada");
    assert!(avisos::esconder(c, &lista, &a.id).is_err(), "atenção não se esconde");
    avisos::minimizar(c, &lista, &a.id, false).unwrap();
    assert!(!avisos::do_dia(c).unwrap().iter().find(|x| x.id == a.id).unwrap().minimizado, "expandir persiste");
    avisos::minimizar(c, &lista, &a.id, true).unwrap();
    assert!(avisos::do_dia(c).unwrap().iter().find(|x| x.id == a.id).unwrap().minimizado, "minimizar persiste");
    assert!(!sync::bloqueios_de_rascunho(c).unwrap().is_empty(), "minimizar não libera nada");
    // Caixa que volta numa versão antiga perde a confirmação na hora.
    confirmar(&b, "caixa-2", 0);
    assert!(sync::bloqueios_de_rascunho(c).unwrap().is_empty());
    sync::registrar_versao(c, "caixa-2", &json!({})).unwrap();
    assert_eq!(sync::bloqueios_de_rascunho(c).unwrap()[0].situacao, "antigo");
}

#[test]
fn caixa_que_so_aparece_no_log_bloqueia_ate_ser_retirado() {
    let b = Banco::em_memoria().unwrap();
    let c = &b.conn;
    let u = usuarios::criar(c, &b.dispositivo, "Maria", "1234", true).unwrap();
    c.execute(
        "INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando) VALUES ('x1', 'pessoa', '[\"p\"]', 'apagar', NULL, 'caixa-velho', '2026-01-01')",
        [],
    )
    .unwrap();
    let b0 = sync::bloqueios_de_rascunho(c).unwrap();
    assert_eq!((b0.len(), b0[0].situacao.as_str()), (1, "desconhecido"));
    assert!(sync::retirar_caixa(c, "caixa-velho", "", &u.nome).is_err(), "motivo obrigatório");
    assert!(sync::retirar_caixa(c, &b.dispositivo, "x", &u.nome).is_err(), "não retira a si mesmo");
    // Com alteração recebida dele ainda pendente: não conclui.
    c.execute(
        "INSERT INTO sync_pendencia (lado, par, alteracao_id, origem, motivo, criado_em, atualizado_em)
         VALUES ('recebida', '', 'p1', 'caixa-velho', 'invalida', 'agora', 'agora')",
        [],
    )
    .unwrap();
    assert!(sync::retirar_caixa(c, "caixa-velho", "Computador vendido", &u.nome).is_err());
    sync::arquivar_pendencia(c, "p1", "Dados estragados, conferido", &u.id).unwrap();
    sync::retirar_caixa(c, "caixa-velho", "Computador vendido", &u.nome).unwrap();
    assert!(sync::retirado(c, "caixa-velho").unwrap());
    assert!(sync::bloqueios_de_rascunho(c).unwrap().is_empty(), "retirado não conta");
    // A retirada vai para os outros caixas (configuração da loja no log).
    assert_eq!(
        contar(c, "SELECT COUNT(*) FROM alteracao WHERE tabela = 'config' AND chave = json_array('caixa_retirado:caixa-velho')"),
        1
    );
    assert!(sync::retirar_caixa(c, "caixa-velho", "de novo", &u.nome).is_err());
    sync::readmitir_caixa(c, "caixa-velho").unwrap();
    assert!(!sync::retirado(c, "caixa-velho").unwrap());
    assert_eq!(sync::bloqueios_de_rascunho(c).unwrap().len(), 1, "readmitido volta a contar");
}

/// O log de um caixa atualizado com um rascunho de OS no meio.
fn log_com_rascunho() -> (Banco, Vec<Alteracao>) {
    let a = Banco::em_memoria().unwrap();
    let (c, d) = (&a.conn, a.dispositivo.as_str());
    let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
    config::gravar(c, "loja_nome", "Loja").unwrap();
    oficina::salvar_rascunho(c, d, &u, &entrada_rascunho()).unwrap();
    config::gravar(c, "loja_telefone", "1199").unwrap();
    let (lista, _) = sync::alteracoes_desde(c, 0, 10_000).unwrap();
    (a, lista)
}

#[test]
fn caixa_antigo_para_no_rascunho_e_nada_se_perde() {
    let (a, lista) = log_com_rascunho();
    let c = &a.conn;
    let i_rascunho = lista
        .iter()
        .position(|x| x.tabela == "ordem_servico" && x.dados.as_deref().unwrap_or("").contains("rascunho"))
        .unwrap();
    // Pedido de um caixa 0.9.14 (sem esquema nem recursos).
    let (enviada, tem_mais) =
        sync::preparar_para(c, "caixa-antigo", &json!({ "desde": 0 }), lista.clone(), false).unwrap();
    assert_eq!(enviada.len(), i_rascunho, "para antes do rascunho");
    assert!(!tem_mais);
    assert!(enviada.iter().all(|x| sync::incompatibilidade(22, x).is_none()));
    let nao: (String, String) = c
        .query_row(
            "SELECT alteracao_id, estado FROM sync_pendencia WHERE lado = 'nao_enviada' AND par = 'caixa-antigo'",
            [],
            |l| Ok((l.get(0)?, l.get(1)?)),
        )
        .unwrap();
    assert_eq!(nao, (lista[i_rascunho].id.clone(), "pendente".into()), "o motivo fica anotado com o id");
    // Repetir o pedido não duplica a anotação.
    sync::preparar_para(c, "caixa-antigo", &json!({ "desde": 0 }), lista.clone(), false).unwrap();
    assert_eq!(contar(c, "SELECT COUNT(*) FROM sync_pendencia WHERE lado = 'nao_enviada'"), 1);
    // O caixa é atualizado: confirma a versão e recebe tudo, a partir dali.
    com_par(&a, "caixa-antigo", "Caixa 2");
    sync::registrar_versao(c, "caixa-antigo", &json!({ "esquema": 23, "recursos": ["pendencias-v1"] })).unwrap();
    assert_eq!(contar(c, "SELECT COUNT(*) FROM sync_pendencia WHERE lado = 'nao_enviada' AND estado = 'pendente'"), 0);
    let (enviada, _) = sync::preparar_para(
        c,
        "caixa-antigo",
        &json!({ "esquema": 23, "recursos": ["pendencias-v1"] }),
        lista[i_rascunho..].to_vec(),
        false,
    )
    .unwrap();
    assert_eq!(enviada.len(), lista.len() - i_rascunho, "o rascunho e o resto vão, com o mesmo id");
    assert_eq!(enviada[0].id, lista[i_rascunho].id);
    assert!(enviada.iter().all(|x| x.incompativel.is_none()));
    // Caixa novo que informa estrutura 22 com pendências: vai marcado.
    let (marcada, _) =
        sync::preparar_para(c, "x", &json!({ "esquema": 22, "recursos": ["pendencias-v1"] }), lista.clone(), false)
            .unwrap();
    assert_eq!(marcada.len(), lista.len());
    assert!(marcada[i_rascunho].incompativel.is_some());
}

/// Matriz de compatibilidade conferida contra a estrutura 22 de verdade
/// (as migrações reais): o que ela diz que não cabe é o que o banco recusa.
#[test]
fn matriz_bate_com_a_estrutura_22_real() {
    let mut v22 = Connection::open_in_memory().unwrap();
    banco::registrar_funcoes(&v22).unwrap();
    banco::migrar_ate(&mut v22, 22).unwrap();
    for t in sync::TABELAS {
        assert!(
            contar(&v22, &format!("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '{t}'")) == 1,
            "{t} existe na 22"
        );
    }
    let (_, lista) = log_com_rascunho();
    for a in lista.iter().filter(|a| a.tabela == "ordem_servico") {
        let dados: serde_json::Map<String, Value> = serde_json::from_str(a.dados.as_deref().unwrap()).unwrap();
        let (cols, vals): (Vec<String>, Vec<rusqlite::types::Value>) = dados
            .iter()
            .filter(|(k, _)| {
                contar(&v22, &format!("SELECT COUNT(*) FROM pragma_table_info('ordem_servico') WHERE name = '{k}'"))
                    == 1
            })
            .map(|(k, v)| {
                (
                    format!("\"{k}\""),
                    match v {
                        Value::Null => rusqlite::types::Value::Null,
                        Value::Number(n) => rusqlite::types::Value::Integer(n.as_i64().unwrap()),
                        Value::String(s) => rusqlite::types::Value::Text(s.clone()),
                        o => rusqlite::types::Value::Text(o.to_string()),
                    },
                )
            })
            .unzip();
        let marcas = (1..=cols.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(", ");
        let r = v22.execute(
            &format!("INSERT OR REPLACE INTO ordem_servico ({}) VALUES ({marcas})", cols.join(", ")),
            rusqlite::params_from_iter(vals),
        );
        assert_eq!(
            r.is_err(),
            sync::incompatibilidade(22, a).is_some(),
            "matriz x banco 22 para {:?}",
            dados.get("situacao")
        );
        // Um orçamento normal cabe na 22.
        let mut normal = a.clone();
        normal.dados = Some(a.dados.as_deref().unwrap().replace("\"rascunho\"", "\"orcamento\""));
        assert!(sync::incompatibilidade(22, &normal).is_none());
    }
}

fn alteracao(id: &str, seq: i64, tabela: &str, chave: Value, dados: Option<Value>, op: Option<&str>) -> Alteracao {
    Alteracao {
        seq,
        id: id.into(),
        tabela: tabela.into(),
        chave: chave.to_string(),
        operacao: if dados.is_some() { "gravar".into() } else { "apagar".into() },
        dados: dados.map(|d| d.to_string()),
        dispositivo: "caixa-2".into(),
        quando: "2026-10-27T10:00:00.000Z".into(),
        base: None,
        ordem: 1,
        historia: None,
        compactada: false,
        operacao_id: op.map(str::to_string),
        incompativel: None,
    }
}

fn pessoa(id: &str, nome: &str) -> Value {
    json!({ "id": id, "codigo": format!("C-{id}"), "cliente": 1, "dispositivo": "caixa-2", "nome": nome, "criado_em": "2026-10-27", "atualizado_em": "2026-10-27" })
}

fn os(id: &str, situacao: &str) -> Value {
    json!({ "id": id, "numero": 1, "dispositivo": "caixa-2", "caixa_nome": "Caixa 2", "situacao": situacao,
            "criado_em": "2026-10-27", "atualizado_em": "2026-10-27" })
}

fn cursor_de(b: &Banco, disp: &str) -> i64 {
    b.conn.query_row("SELECT cursor FROM sync_par WHERE dispositivo = ?1", [disp], |l| l.get(0)).unwrap()
}

/// Marca d'água: tudo até o contador está gravado (no log) ou guardado.
fn conferir_marca_dagua(b: &Banco, enviadas: &[Alteracao], cursor: i64) {
    for a in enviadas.iter().filter(|a| a.seq <= cursor) {
        if a.tabela == "config" {
            continue;
        }
        let no_log = contar(&b.conn, &format!("SELECT COUNT(*) FROM alteracao WHERE id = '{}'", a.id));
        let guardada = contar(&b.conn, &format!("SELECT COUNT(*) FROM sync_pendencia WHERE lado = 'recebida' AND alteracao_id = '{}' AND estado <> 'resolvida'", a.id));
        assert_eq!(no_log + guardada, 1, "a alteração {} (seq {}) sumiu ou duplicou", a.id, a.seq);
    }
}

fn aplicar_lote(b: &mut Banco, lote: &[Alteracao], de: i64) -> sync::Aplicacao {
    let para = lote.iter().map(|a| a.seq).max().unwrap_or(de);
    let marca = lote.iter().max_by_key(|a| a.seq).map(|a| a.id.clone()).unwrap_or_default();
    sync::aplicar_de(
        &mut b.conn,
        lote,
        "caixa-2",
        Some(AvancoCursor { dispositivo: "caixa-2", de, para, marca: &marca, geracao: "g" }),
    )
    .unwrap()
}

#[test]
fn erro_de_check_desfaz_o_lote_guarda_tudo_e_a_troca_continua() {
    let mut b = Banco::em_memoria().unwrap();
    com_par(&b, "caixa-2", "Caixa 2");
    // Lote 1: uma pessoa boa e uma OS com situação que o banco recusa (CHECK).
    let lote1 = vec![
        alteracao("a1", 1, "pessoa", json!(["p1"]), Some(pessoa("p1", "Zé")), Some("op-a")),
        alteracao("a2", 2, "ordem_servico", json!(["o1"]), Some(os("o1", "situacao_que_nao_existe")), Some("op-b")),
    ];
    let r = aplicar_lote(&mut b, &lote1, 0);
    assert!(r.suspenso && r.avancou);
    assert_eq!(r.pendentes, 2);
    assert_eq!(
        contar(&b.conn, "SELECT COUNT(*) FROM pessoa WHERE id = 'p1'"),
        0,
        "nada do lote entrou (sem aplicação parcial)"
    );
    assert_eq!(
        contar(&b.conn, "SELECT COUNT(*) FROM alteracao WHERE id IN ('a1', 'a2')"),
        0,
        "nada marcado como aplicado"
    );
    let motivo: String =
        b.conn.query_row("SELECT motivo FROM sync_pendencia WHERE alteracao_id = 'a2'", [], |l| l.get(0)).unwrap();
    assert_eq!(motivo, "erro_aplicacao");
    let erro: String =
        b.conn.query_row("SELECT erro FROM sync_par WHERE dispositivo = 'caixa-2'", [], |l| l.get(0)).unwrap();
    assert!(erro.contains("não pôde ser aplicada"), "erro registrado para o gerente: {erro}");
    assert_eq!(cursor_de(&b, "caixa-2"), 2);
    conferir_marca_dagua(&b, &lote1, 2);
    // Lote 2: independente entra; o que depende do suspenso fica guardado.
    let lote2 = vec![
        alteracao("a3", 3, "pessoa", json!(["p2"]), Some(pessoa("p2", "Ana")), Some("op-c")),
        alteracao("a4", 4, "pessoa", json!(["p1"]), Some(pessoa("p1", "Zé da Silva")), Some("op-d")),
        alteracao(
            "a5",
            5,
            "pessoa_veiculo",
            json!(["v1"]),
            Some(json!({ "id": "v1", "pessoa_id": "p1", "ordem": 0, "placa": "ABC1D23", "descricao": "Gol" })),
            Some("op-e"),
        ),
        alteracao("a6", 6, "pessoa", json!(["p3"]), Some(pessoa("p3", "Bia")), Some("op-b")),
    ];
    let r = aplicar_lote(&mut b, &lote2, 2);
    assert!(!r.suspenso);
    assert_eq!(contar(&b.conn, "SELECT COUNT(*) FROM pessoa WHERE id = 'p2'"), 1, "independente entrou");
    for (id, motivo) in [("a4", "mesmo registro"), ("a5", "pai pendente"), ("a6", "mesma operação")] {
        assert_eq!(
            contar(&b.conn, &format!("SELECT COUNT(*) FROM sync_pendencia WHERE alteracao_id = '{id}' AND motivo = 'dependencia_suspensa'")),
            1,
            "{id}: {motivo}"
        );
    }
    assert_eq!(cursor_de(&b, "caixa-2"), 6);
    let tudo: Vec<Alteracao> = lote1.iter().chain(lote2.iter()).cloned().collect();
    conferir_marca_dagua(&b, &tudo, 6);
    // Reenvio do mesmo lote (reinício antes de confirmar): nada duplica.
    let r = aplicar_lote(&mut b, &lote2, 6);
    assert_eq!((r.novas, r.pendentes), (0, 0));
    conferir_marca_dagua(&b, &tudo, 6);
    // Tentar de novo da cópia guardada: o lote com erro continua recusado e nada muda.
    assert_eq!(sync::reaplicar_pendencias(&mut b.conn).unwrap(), 0);
    assert_eq!(contar(&b.conn, "SELECT COUNT(*) FROM pessoa WHERE id IN ('p1', 'p3')"), 0);
    assert!(contar(&b.conn, "SELECT MIN(tentativas) FROM sync_pendencia WHERE estado = 'pendente'") >= 1);
    // O gerente arquiva a que não tem conserto: as outras entram na próxima tentativa.
    let u = usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
    assert!(sync::arquivar_pendencia(&b.conn, "a2", "", &u.id).is_err(), "motivo obrigatório");
    sync::arquivar_pendencia(&b.conn, "a2", "OS estragada no outro caixa", &u.id).unwrap();
    let aplicadas = sync::reaplicar_pendencias(&mut b.conn).unwrap();
    assert_eq!(aplicadas, 4, "a1, a4, a5 e a6");
    let nome: String = b.conn.query_row("SELECT nome FROM pessoa WHERE id = 'p1'", [], |l| l.get(0)).unwrap();
    assert_eq!(nome, "Zé da Silva", "a versão mais nova vale");
    assert_eq!(contar(&b.conn, "SELECT COUNT(*) FROM pessoa_veiculo WHERE id = 'v1'"), 1);
    let (arq, payload): (String, Option<String>) = b
        .conn
        .query_row("SELECT estado, dados FROM sync_pendencia WHERE alteracao_id = 'a2'", [], |l| {
            Ok((l.get(0)?, l.get(1)?))
        })
        .unwrap();
    assert_eq!(arq, "arquivada_manual", "arquivada não vira aplicada");
    assert!(payload.unwrap().contains("situacao_que_nao_existe"), "a cópia inteira continua guardada");
    assert_eq!(contar(&b.conn, "SELECT COUNT(*) FROM alteracao WHERE id = 'a2'"), 0);
    conferir_marca_dagua(&b, &tudo, 6);
    // Simultaneidade › Histórico: as encerradas aparecem (aplicadas depois e
    // a arquivada, com quem arquivou e o motivo); a pendente não.
    let h = sync::historico_da_troca(&b.conn, 200).unwrap();
    assert_eq!(h.encerradas.len(), 5, "{h:?}");
    let a2 = h.encerradas.iter().find(|p| p.alteracao_id == "a2").unwrap();
    assert_eq!((a2.estado.as_str(), a2.arquivada_por.as_str()), ("arquivada_manual", "Maria"));
    assert_eq!(a2.arquivada_motivo, "OS estragada no outro caixa");
    assert_eq!(a2.caixa, "Caixa 2");
    assert!(h.encerradas.iter().filter(|p| p.alteracao_id != "a2").all(|p| p.estado == "resolvida"));
    assert!(h.caixas.iter().any(|c| c.nome == "Caixa 2" && c.erro.contains("não pôde ser aplicada")), "{:?}", h.caixas);
}

#[test]
fn dados_estragados_e_tabela_desconhecida_viram_pendencia_e_d16_continua() {
    let mut b = Banco::em_memoria().unwrap();
    com_par(&b, "caixa-2", "Caixa 2");
    let mut nao_objeto = alteracao("b1", 1, "pessoa", json!(["p9"]), None, None);
    nao_objeto.operacao = "gravar".into();
    nao_objeto.dados = Some("[1, 2]".into());
    let sem_chave = alteracao("b2", 2, "pessoa", json!(["p8"]), Some(json!({ "nome": "sem id" })), None);
    let desconhecida = alteracao("b3", 3, "tabela_do_futuro", json!(["x"]), Some(json!({ "id": "x" })), None);
    let mut marcada = alteracao("b4", 4, "pessoa", json!(["p7"]), Some(pessoa("p7", "Futuro")), None);
    marcada.incompativel = Some("versão nova demais".into());
    let d16 =
        alteracao("b5", 5, "config", json!(["sync_senha"]), Some(json!({ "chave": "sync_senha", "valor": "x" })), None);
    let lote = vec![nao_objeto, sem_chave, desconhecida, marcada, d16];
    let r = aplicar_lote(&mut b, &lote, 0);
    assert_eq!(r.pendentes, 4);
    for (id, motivo) in
        [("b1", "invalida"), ("b2", "invalida"), ("b3", "incompativel_esquema"), ("b4", "incompativel_esquema")]
    {
        assert_eq!(
            contar(
                &b.conn,
                &format!("SELECT COUNT(*) FROM sync_pendencia WHERE alteracao_id = '{id}' AND motivo = '{motivo}'")
            ),
            1,
            "{id}"
        );
    }
    assert_eq!(
        contar(&b.conn, "SELECT COUNT(*) FROM sync_pendencia WHERE alteracao_id = 'b5'"),
        0,
        "D-16: descarte de propósito"
    );
    assert_eq!(config::ler(&b.conn, "sync_senha").unwrap(), None);
    assert_eq!(cursor_de(&b, "caixa-2"), 5);
    conferir_marca_dagua(&b, &lote, 5);
    let u = usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
    assert!(sync::arquivar_pendencia(&b.conn, "b3", "não dá", &u.id).is_err(), "incompatibilidade não se arquiva");
    let lista = sync::pendencias_da_troca(&b.conn).unwrap();
    assert_eq!(lista.len(), 4);
    assert!(!lista.iter().find(|p| p.alteracao_id == "b4").unwrap().pode_arquivar);
    assert!(lista.iter().find(|p| p.alteracao_id == "b1").unwrap().pode_arquivar);
    // Notificação de Atenção para o gerente.
    assert!(avisos::do_dia(&b.conn).unwrap().iter().any(|a| a.id == "pendencias" && a.categoria == avisos::ATENCAO));
}

#[test]
fn operacao_comercial_e_a_transacao() {
    let mut b = Banco::em_memoria().unwrap();
    let u = usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
    let tx = b.conn.transaction().unwrap();
    oficina::salvar_rascunho(&tx, "d", &u.id, &entrada_rascunho()).unwrap();
    tx.commit().unwrap();
    let ops: Vec<Option<String>> = b
        .conn
        .prepare("SELECT DISTINCT operacao_id FROM alteracao WHERE tabela IN ('ordem_servico', 'ordem_servico_item', 'auditoria') AND seq > (SELECT MIN(seq) FROM alteracao WHERE tabela = 'ordem_servico') - 1")
        .unwrap()
        .query_map([], |l| l.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(ops.len(), 1, "OS, itens e auditoria da mesma transação: a mesma operação ({ops:?})");
    assert!(ops[0].is_some());
    // Gravações fora de transação: uma operação cada.
    config::gravar(&b.conn, "loja_nome", "A").unwrap();
    config::gravar(&b.conn, "loja_nome", "B").unwrap();
    let n = contar(&b.conn, "SELECT COUNT(DISTINCT operacao_id) FROM alteracao WHERE tabela = 'config'");
    assert!(n >= 2);
}

#[test]
fn caixa_retirado_nao_troca_dados() {
    let b = Banco::em_memoria().unwrap();
    com_par(&b, "caixa-2", "Caixa 2");
    sync::retirar_caixa(&b.conn, "caixa-2", "Saiu da loja", "Maria").unwrap();
    let lista = sync::caixas_conhecidos(&b.conn).unwrap();
    let c2 = lista.iter().find(|c| c.dispositivo == "caixa-2").unwrap();
    assert_eq!(c2.situacao, "retirado");
    assert_eq!(c2.retirado_motivo, "Saiu da loja");
    assert!(!c2.bloqueia);
}

/// Regra dos cinco pontos do gerente local (verificação do que existe; nada
/// mudou na 0.9.15): entrar numa loja que já existe ("sync_juntar") não cria
/// usuário nem gerente neste computador e só é aceito sem usuários; a
/// instalação independente ("configurar_inicio") faz do primeiro usuário o
/// gerente; depois de configurado, não dá para "juntar" de novo por ali.
#[test]
fn juntar_a_loja_existente_nao_cria_gerente_local() {
    let mut s = crate::Servico::em_memoria().unwrap();
    let chamar = |s: &mut crate::Servico, cmd: &str, d: Value| -> Value {
        serde_json::from_str(&s.chamar(cmd, &d.to_string())).unwrap()
    };
    let r = chamar(&mut s, "sync_juntar", json!({ "caixa": "Caixa 2", "senha": "rede-da-loja", "enderecos": "" }));
    assert_eq!(r["ok"], true, "{r}");
    let r = chamar(&mut s, "usuarios_entrada", json!({}));
    assert_eq!(r["dados"].as_array().map(|a| a.len()), Some(0), "nenhum usuário (nem gerente) local: {r}");
    // Sem ninguém conectado, nada do gerente responde.
    let r = chamar(&mut s, "gerente_ler", json!({}));
    assert_eq!(r["ok"], false, "{r}");

    let mut s = crate::Servico::em_memoria().unwrap();
    let r = chamar(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
    );
    assert_eq!(r["ok"], true, "{r}");
    let e = chamar(&mut s, "estado", json!({}));
    assert_eq!(e["dados"]["usuario"]["gerente"], true, "instalação independente: o primeiro é gerente: {e}");
    let r = chamar(&mut s, "sync_juntar", json!({ "caixa": "Caixa 1", "senha": "rede-da-loja", "enderecos": "" }));
    assert_eq!(r["ok"], false, "já configurado não junta por ali: {r}");
}

// ---------------------------------------------------------------------------
// FALHA-03 (auditoria da 0.9.15): referência esperando com o pai guardado
// numa pendência.
// ---------------------------------------------------------------------------

fn estado_da_referencia(b: &Banco, tabela: &str) -> Option<(String, String)> {
    use rusqlite::OptionalExtension;
    b.conn
        .query_row("SELECT estado, motivo FROM sync_referencia WHERE tabela = ?1", [tabela], |l| {
            Ok((l.get(0)?, l.get(1)?))
        })
        .optional()
        .unwrap()
}

/// V03, na ordem que falhava: o filho chega antes (de outro caixa) e fica
/// esperando; o lote do pai é recusado e guardado em `sync_pendencia`; os
/// caixas fazem a troca completa. O filho continua esperando (não vira
/// inconsistente) e o dinheiro dele não é bloqueado, porque o pai está
/// guardado aqui. Resolvida a pendência, a referência some.
#[test]
fn v03_pai_guardado_em_pendencia_nao_vira_inconsistente() {
    let mut b = Banco::em_memoria().unwrap();
    com_par(&b, "caixa-2", "Caixa 2");
    com_par(&b, "caixa-3", "Caixa 3");
    let filho = vec![alteracao(
        "c1",
        1,
        "pessoa_veiculo",
        json!(["v1"]),
        Some(json!({ "id": "v1", "pessoa_id": "p1", "ordem": 0, "placa": "ABC1D23", "descricao": "Gol" })),
        Some("op-x"),
    )];
    sync::aplicar_de(
        &mut b.conn,
        &filho,
        "caixa-3",
        Some(AvancoCursor { dispositivo: "caixa-3", de: 0, para: 1, marca: "c1", geracao: "g" }),
    )
    .unwrap();
    sync::registrar_referencias(&b.conn).unwrap();
    assert_eq!(estado_da_referencia(&b, "pessoa_veiculo").unwrap().0, "esperando");
    // O lote do pai é recusado (a OS dele tem uma situação que não existe) e guardado.
    let lote = vec![
        alteracao("a1", 1, "pessoa", json!(["p1"]), Some(pessoa("p1", "Zé")), Some("op-a")),
        alteracao("a2", 2, "ordem_servico", json!(["o1"]), Some(os("o1", "situacao_que_nao_existe")), Some("op-b")),
    ];
    let r = aplicar_lote(&mut b, &lote, 0);
    assert!(r.suspenso, "o lote do pai foi guardado inteiro");
    assert_eq!(
        contar(
            &b.conn,
            "SELECT COUNT(*) FROM sync_pendencia WHERE tabela = 'pessoa' AND chave LIKE '%p1%' AND estado = 'pendente'"
        ),
        1
    );
    // Os dois caixas conhecidos entregaram tudo depois de a referência aparecer.
    b.conn.execute("UPDATE sync_par SET completo_em = '2999-01-01T00:00:00.000Z'", []).unwrap();
    sync::registrar_referencias(&b.conn).unwrap();
    assert_eq!(
        estado_da_referencia(&b, "pessoa_veiculo").unwrap().0,
        "esperando",
        "o pai está guardado aqui: ainda não é inconsistência"
    );
    assert!(sync::exigir_editavel(&b.conn, "pessoa_veiculo", "v1").is_ok(), "o dinheiro do filho não é bloqueado");
    // Banco que já tinha marcado errado (antes da correção) volta a esperar.
    b.conn.execute("UPDATE sync_referencia SET estado = 'inconsistente', motivo = 'pares'", []).unwrap();
    sync::registrar_referencias(&b.conn).unwrap();
    assert_eq!(estado_da_referencia(&b, "pessoa_veiculo").unwrap().0, "esperando");
    // O gerente arquiva a OS estragada; o pai entra; a referência some.
    let u = usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
    sync::arquivar_pendencia(&b.conn, "a2", "OS estragada", &u.id).unwrap();
    sync::reaplicar_pendencias(&mut b.conn).unwrap();
    sync::registrar_referencias(&b.conn).unwrap();
    assert_eq!(estado_da_referencia(&b, "pessoa_veiculo"), None, "o pai chegou");
}

/// Controle da FALHA-03: com o pai ausente de todo lugar (nenhuma pendência
/// com ele), a prova dos pares continua valendo e o dinheiro é bloqueado.
#[test]
fn v03_pai_ausente_de_todo_lugar_continua_inconsistente() {
    let mut b = Banco::em_memoria().unwrap();
    com_par(&b, "caixa-3", "Caixa 3");
    let filho = vec![alteracao(
        "c1",
        1,
        "pessoa_veiculo",
        json!(["v1"]),
        Some(json!({ "id": "v1", "pessoa_id": "p9", "ordem": 0, "placa": "ABC1D23", "descricao": "Gol" })),
        Some("op-x"),
    )];
    sync::aplicar_de(
        &mut b.conn,
        &filho,
        "caixa-3",
        Some(AvancoCursor { dispositivo: "caixa-3", de: 0, para: 1, marca: "c1", geracao: "g" }),
    )
    .unwrap();
    sync::registrar_referencias(&b.conn).unwrap();
    // Uma pendência de OUTRO registro não segura esta referência.
    let lote = vec![alteracao(
        "a2",
        2,
        "ordem_servico",
        json!(["o1"]),
        Some(os("o1", "situacao_que_nao_existe")),
        Some("op-b"),
    )];
    let r = aplicar_lote(&mut b, &lote, 0);
    assert!(r.suspenso);
    b.conn.execute("UPDATE sync_par SET completo_em = '2999-01-01T00:00:00.000Z'", []).unwrap();
    sync::registrar_referencias(&b.conn).unwrap();
    assert_eq!(estado_da_referencia(&b, "pessoa_veiculo").unwrap(), ("inconsistente".to_string(), "pares".to_string()));
    assert!(sync::exigir_editavel(&b.conn, "pessoa_veiculo", "v1").is_err(), "inconsistência comprovada bloqueia");
}

// ---------------------------------------------------------------------------
// FALHA-06 (auditoria da 0.9.15): cópias da migração e integridade.
// ---------------------------------------------------------------------------

fn copias_antes(pasta: &Path) -> Vec<String> {
    copias_em(pasta).into_iter().filter(|n| n.starts_with("rustinho.db.antes-v")).collect()
}

fn falhas_guardadas(pasta: &Path) -> Vec<String> {
    copias_em(pasta).into_iter().filter(|n| n.starts_with("falha-migracao-") && n.ends_with(".db")).collect()
}

fn sha(p: &Path) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(std::fs::read(p).unwrap()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Duas falhas seguidas da migração sobre o mesmo banco: a cópia de antes,
/// com o mesmo conteúdo, é reaproveitada (não fica uma por tentativa); cada
/// banco que falhou fica guardado com nome próprio (mesmo no mesmo segundo);
/// nenhuma cópia é apagada; e a tentativa seguinte, sem falha, termina.
#[test]
fn f06_duas_falhas_seguidas_nao_duplicam_a_copia() {
    let pasta = pasta_nova("f06-duas");
    let db = banco_v22(&pasta);
    let mut guardados = Vec::new();
    for _ in 1..=2 {
        assert!(rodar_filho(&pasta, "RUSTINHO_TESTE_MIGRACAO_FALHAR", "conferencia").success());
        let e = migracao::ler_estado(&db).unwrap().unwrap();
        assert_eq!(e.fase, migracao::Fase::Falhou);
        guardados.push(e.guardado_em.clone());
        assert!(migracao::liberar_nova_tentativa(&db).unwrap());
    }
    let antes = copias_antes(&pasta);
    assert_eq!(antes.len(), 1, "uma cópia de antes só, reaproveitada: {antes:?}");
    assert_ne!(guardados[0], guardados[1], "cada banco que falhou com nome próprio");
    let falhas = falhas_guardadas(&pasta);
    assert_eq!(falhas.len(), 2, "os dois bancos que falharam ficam: {falhas:?}");
    for f in &falhas {
        let c = Connection::open_with_flags(pasta.join(f), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        assert_eq!(c.query_row("PRAGMA quick_check", [], |l| l.get::<_, String>(0)).unwrap(), "ok");
    }
    // A tentativa sem falha termina, com a mesma cópia guardada em pre-migracao/.
    conferir_migrado(&db);
    assert_eq!(copias_em(&pasta.join("pre-migracao")), antes, "a cópia reaproveitada foi a guardada");
    assert_eq!(falhas_guardadas(&pasta).len(), 2, "nada apagado");
    let _ = std::fs::remove_dir_all(&pasta);
}

/// A cópia de antes que existe mas não confere (estragada ou pela metade)
/// não é reaproveitada: sai uma cópia nova, e a estragada fica onde está.
#[test]
fn f06_copia_estragada_nao_e_reaproveitada() {
    let pasta = pasta_nova("f06-estragada");
    let db = banco_v22(&pasta);
    assert!(rodar_filho(&pasta, "RUSTINHO_TESTE_MIGRACAO_FALHAR", "conferencia").success());
    assert!(migracao::liberar_nova_tentativa(&db).unwrap());
    let antes = copias_antes(&pasta);
    assert_eq!(antes.len(), 1);
    // Corta a cópia pela metade.
    let estragada = pasta.join(&antes[0]);
    let tamanho = std::fs::metadata(&estragada).unwrap().len();
    std::fs::OpenOptions::new().write(true).open(&estragada).unwrap().set_len(tamanho / 2).unwrap();
    let sha_estragada = sha(&estragada);
    assert!(rodar_filho(&pasta, "RUSTINHO_TESTE_MIGRACAO_FALHAR", "conferencia").success());
    let depois = copias_antes(&pasta);
    assert_eq!(depois.len(), 2, "cópia nova; a estragada continua lá: {depois:?}");
    assert_eq!(sha(&estragada), sha_estragada, "a estragada não foi mexida");
    let e = migracao::ler_estado(&db).unwrap().unwrap();
    assert_ne!(e.copia, antes[0], "a usada é a nova");
    // O banco voltou da cópia boa.
    let c = Connection::open(&db).unwrap();
    assert_eq!(contar(&c, "SELECT COUNT(*) FROM ordem_servico_item"), 91);
    let _ = std::fs::remove_dir_all(&pasta);
}

/// Banco sem migração: a abertura confere a integridade. Íntegro, nada de
/// aviso; com defeito, abre em modo seguro (decisão A3: só consulta; veja
/// `testes_modo_seguro`) com aviso crítico no Início, e o registro anota.
#[test]
fn f06_abertura_confere_a_integridade() {
    let pasta = pasta_nova("f06-abrir");
    let db = pasta.join("rustinho.db");
    {
        let b = Banco::abrir(&db).unwrap();
        b.conn.execute_batch("PRAGMA journal_mode = DELETE").unwrap();
        for i in 0..3000 {
            b.conn
                .execute(
                    "INSERT INTO auditoria (id, quando, dispositivo, usuario_id, acao, entidade, entidade_id, detalhes)
                     VALUES (?1, '2026-10-01', 'd', NULL, 'teste', 'teste', ?1, ?2)",
                    params![format!("a{i}"), "x".repeat(200)],
                )
                .unwrap();
        }
    }
    {
        let b = Banco::abrir(&db).unwrap();
        assert!(avisos::do_dia(&b.conn).unwrap().iter().all(|a| a.id != "banco_defeito"), "íntegro: sem aviso");
        b.conn.execute_batch("PRAGMA journal_mode = DELETE").unwrap();
    }
    // Estraga o cabeçalho de uma página do fim do arquivo (da auditoria).
    let pagina: u64 = {
        let c = Connection::open(&db).unwrap();
        c.query_row("PRAGMA page_size", [], |l| l.get::<_, i64>(0)).unwrap() as u64
    };
    let tamanho = std::fs::metadata(&db).unwrap().len();
    {
        use std::io::{Seek, SeekFrom, Write};
        let mut f = std::fs::OpenOptions::new().write(true).open(&db).unwrap();
        f.seek(SeekFrom::Start(tamanho - 3 * pagina)).unwrap();
        f.write_all(&[0xFFu8; 16]).unwrap();
    }
    let b = Banco::abrir(&db).expect("abre mesmo com defeito (em modo seguro)");
    assert!(b.em_modo_seguro());
    let av = avisos::do_dia(&b.conn).unwrap();
    let a = av.iter().find(|a| a.id == "banco_defeito").expect("aviso do defeito");
    assert_eq!(a.categoria, avisos::CRITICA);
    drop(b);

    // Backup desse banco: sai como erro (nunca como "íntegro"); se a cópia
    // sair, fica, e os backups antigos não são apagados pela retenção.
    let backups = pasta.join("backups");
    let velhos = backups_velhos(&backups);
    let e = crate::backup::fazer(&db, false).expect_err("backup de banco com defeito sai como erro");
    assert!(e.to_string().contains("defeito de integridade"), "{e}");
    for v in &velhos {
        assert!(backups.join(v).exists(), "o backup antigo {v} ficou");
    }
    assert!(copias_em(&backups).len() >= velhos.len(), "nada apagado");
    let _ = std::fs::remove_dir_all(&pasta);
}

/// Backups antigos de mentira (vários dias de 2025), que a retenção apagaria.
fn backups_velhos(pasta: &Path) -> Vec<String> {
    std::fs::create_dir_all(pasta).unwrap();
    let mut v = Vec::new();
    for d in 1..=6 {
        let n = format!("rustinho-2025010{d}-120000-caixa.rbk");
        std::fs::write(pasta.join(&n), b"backup antigo").unwrap();
        v.push(n);
    }
    v
}

/// Controle do teste acima: num banco íntegro, a retenção de sempre apaga os
/// antigos (é ela que o defeito suspende).
#[test]
fn f06_backup_de_banco_integro_segue_a_retencao() {
    let pasta = pasta_nova("f06-backup-ok");
    let db = pasta.join("rustinho.db");
    drop(Banco::abrir(&db).unwrap());
    let backups = pasta.join("backups");
    let velhos = backups_velhos(&backups);
    crate::backup::fazer(&db, false).unwrap();
    assert!(velhos.iter().any(|v| !backups.join(v).exists()), "a retenção normal apagou algum antigo");
    let _ = std::fs::remove_dir_all(&pasta);
}

// ---------------------------------------------------------------------------
// FALHA-02 (auditoria da 0.9.15): OS nº 1 de dois caixas.
// ---------------------------------------------------------------------------

/// Dois caixas abrem cada um a OS nº 1 (numeração por caixa, desde a 0.9.x;
/// sem índice único nem renumeração). A lista, a busca, a ficha e a venda da
/// OS trazem o caixa, e as telas mostram o caixa junto do número.
#[test]
fn f02_os_numero_1_de_dois_caixas_mostra_o_caixa() {
    let mut a = Banco::em_memoria().unwrap();
    let b = Banco::em_memoria().unwrap();
    let ua = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "1234", true).unwrap().id;
    let ub = usuarios::criar(&b.conn, &b.dispositivo, "Bob", "1234", true).unwrap().id;
    config::gravar(&a.conn, "caixa_nome", "Caixa 1").unwrap();
    config::gravar(&b.conn, "caixa_nome", "Caixa 2").unwrap();
    let mut e = entrada_rascunho();
    e.placa = "ABC1D23".into();
    let os_a = oficina::salvar(&a.conn, &a.dispositivo, &ua, &e).unwrap();
    let mut e2 = entrada_rascunho();
    e2.placa = "XYZ9K88".into();
    e2.cliente_nome = "Outro".into();
    let os_b = oficina::salvar(&b.conn, &b.dispositivo, &ub, &e2).unwrap();
    let (lista, _) = sync::alteracoes_desde(&b.conn, 0, 100_000).unwrap();
    sync::aplicar(&mut a.conn, &lista).unwrap();
    let busca = oficina::listar(&a.conn, "todas", "1").unwrap();
    let mut caixas: Vec<(i64, String)> = busca.iter().map(|o| (o.numero, o.caixa.clone())).collect();
    caixas.sort();
    assert_eq!(
        caixas,
        vec![(1, "Caixa 1".to_string()), (1, "Caixa 2".to_string())],
        "a busca traz o caixa de cada uma"
    );
    assert_eq!(oficina::ficha(&a.conn, &os_b).unwrap().caixa, "Caixa 2");
    assert_eq!(oficina::para_venda(&a.conn, &os_a).unwrap().caixa, "Caixa 1", "a venda da OS sabe o caixa");
    // As telas mostram o caixa junto do número.
    let qml = |nome: &str| {
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../app/qml").join(nome)).unwrap()
    };
    let ordens = qml("TelaOrdens.qml");
    assert!(ordens.contains("linha.o.caixa"), "a lista (e a busca) mostram o caixa");
    let ordem = qml("TelaOrdem.qml");
    let titulo = ordem.split("titulo: tela.nova").nth(1).unwrap().split("subtitulo").next().unwrap();
    assert!(titulo.contains("tela.ficha.caixa"), "a ficha mostra o caixa no título: {titulo}");
    assert!(ordem.contains("h.caixa"), "o histórico do carro mostra o caixa de cada OS");
    let vender = qml("TelaVender.qml");
    assert!(vender.contains("osCaixa"), "a venda da OS mostra o caixa");
}
