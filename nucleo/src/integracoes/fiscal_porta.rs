//! FIS-LIGA (decisão do proprietário, 25/11/2026): o seletor manual do certificado A3 e da integração fiscal, e a
//! **porta única** por onde o Rustinho fala com o adaptador fiscal e com o certificado.
//!
//! - **Desligado de fábrica**, com o estado gravado. A chave `fiscal_ligado` não está em `CONFIG_DA_LOJA`: fica só
//!   neste computador (o A3 está ligado a um computador; ligar num caixa não liga nos outros).
//! - **Quem:** só um gerente vê e muda. **Ligar pede a senha de um gerente** digitada na hora (a senha já digitada no
//!   "aperto" não vale), com a mesma espera depois de muitas senhas erradas. **Qualquer gerente desliga.** As duas
//!   ações vão para a auditoria.
//! - **A porta única:** cada método do [`Adaptador`] recebe uma [`Licenca`], e só [`com_adaptador`] cria uma, e
//!   só com o fiscal ligado. Fora deste módulo não há como criar a licença (o campo é privado): o compilador garante
//!   que nenhum caminho fala com o adaptador sem passar pela porta. Um teste confere que a licença só é criada aqui.
//! - Desligado, o resto do Rustinho funciona igual (a importação do XML só lê arquivos e não usa o certificado).

use rusqlite::Connection;
use serde::Serialize;

use crate::erro::{Erro, Resultado};
use crate::{auditoria, config, usuarios};

/// A chave do seletor no `config` (só deste computador).
pub const CHAVE: &str = "fiscal_ligado";

/// A mesma regra do "aperto" (`api`): 5 senhas erradas seguidas, 30 segundos de espera.
const TENTATIVAS_MAXIMAS: u32 = 5;
const ESPERA_APOS_ERROS: i64 = 30;
/// O balde das senhas erradas do gerente (o mesmo do "aperto").
const BALDE_DO_GERENTE: &str = "gerente";

/// A mensagem de quando algo tenta usar o fiscal desligado.
pub const DESLIGADO: &str =
    "O uso do certificado A3 e a integração fiscal estão desligados neste computador. Um gerente liga em Configurações › Fiscal.";

/// O seletor está ligado neste computador?
pub fn ligado(conn: &Connection) -> Resultado<bool> {
    config::ligado(conn, CHAVE)
}

/// O que a tela mostra.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Estado {
    pub ligado: bool,
    /// Só um gerente vê e muda o seletor.
    pub pode_mudar: bool,
}

pub fn estado(conn: &Connection, gerente: bool) -> Resultado<Estado> {
    Ok(Estado { ligado: ligado(conn)?, pode_mudar: gerente })
}

/// Confere a senha de um gerente digitada agora, com a espera depois de 5 erros e o registro de cada erro (FIS-LIGA;
/// também a troca para a produção, B1 da R1). Grava o erro **fora** da transação de quem chama (o erro tem de ficar
/// registrado mesmo que o comando falhe): chame antes de abrir a transação. Devolve o nome do gerente.
pub fn conferir_senha_do_gerente(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    senha: &str,
    para_que: &str,
) -> Resultado<String> {
    if let Some((quantidade, passou)) = usuarios::falhas(conn, BALDE_DO_GERENTE)? {
        if quantidade >= TENTATIVAS_MAXIMAS && passou < ESPERA_APOS_ERROS {
            let faltam = ESPERA_APOS_ERROS - passou;
            return Err(Erro::campo(
                "senha",
                format!("Muitas senhas erradas. Espere {faltam} segundos e tente de novo."),
            ));
        }
    }
    if senha.is_empty() {
        return Err(Erro::campo("senha", format!("Digite a senha do gerente para {para_que}.")));
    }
    let Some(g) = usuarios::conferir_gerente(conn, senha)? else {
        usuarios::anotar_falha(conn, BALDE_DO_GERENTE, ESPERA_APOS_ERROS)?;
        auditoria::registrar(
            conn,
            dispositivo,
            Some(usuario_id),
            "gerente.senha_errada",
            "usuario",
            usuario_id,
            &[format!("A senha do gerente digitada para {para_que} estava errada")],
        )?;
        return Err(Erro::campo("senha", "Senha do gerente errada."));
    };
    usuarios::limpar_falhas(conn, BALDE_DO_GERENTE)?;
    Ok(g.nome)
}

/// Grava o "ligado" e o registro, juntos (C-M11 da R1: o atendente passa a transação; ou os dois, ou nenhum).
/// `liberou`: o nome do gerente cuja senha foi conferida por [`conferir_senha_do_gerente`].
pub fn gravar_ligado(
    tx: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    gerente: bool,
    liberou: &str,
) -> Resultado<()> {
    if !gerente {
        return Err(Erro::validacao("Só um gerente liga o uso do certificado A3 e a integração fiscal."));
    }
    if !ligado(tx)? {
        config::gravar(tx, CHAVE, "1")?;
        auditoria::registrar(
            tx,
            dispositivo,
            Some(usuario_id),
            "fiscal.ligar",
            "config",
            CHAVE,
            &[format!("{liberou} ligou o uso do certificado A3 e a integração fiscal neste computador")],
        )?;
    }
    Ok(())
}

/// Liga o uso do A3 e a integração fiscal neste computador. `gerente`: quem está conectado é gerente. `senha`: a
/// senha de um gerente, digitada agora. Devolve o nome do gerente que liberou. (O comando faz o mesmo, com a gravação
/// numa transação.)
pub fn ligar(conn: &Connection, dispositivo: &str, usuario_id: &str, gerente: bool, senha: &str) -> Resultado<String> {
    if !gerente {
        return Err(Erro::validacao("Só um gerente liga o uso do certificado A3 e a integração fiscal."));
    }
    let nome = conferir_senha_do_gerente(conn, dispositivo, usuario_id, senha, "ligar o fiscal")?;
    gravar_ligado(conn, dispositivo, usuario_id, gerente, &nome)?;
    Ok(nome)
}

/// Desliga neste computador. Qualquer gerente desliga, sem senha.
pub fn desligar(conn: &Connection, dispositivo: &str, usuario_id: &str, gerente: bool) -> Resultado<()> {
    if !gerente {
        return Err(Erro::validacao("Só um gerente desliga o uso do certificado A3 e a integração fiscal."));
    }
    if ligado(conn)? {
        config::gravar(conn, CHAVE, "0")?;
        let nome = usuarios::buscar(conn, usuario_id).map(|u| u.nome).unwrap_or_default();
        auditoria::registrar(
            conn,
            dispositivo,
            Some(usuario_id),
            "fiscal.desligar",
            "config",
            CHAVE,
            &[format!("{nome} desligou o uso do certificado A3 e a integração fiscal neste computador")],
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------- a porta única

/// A prova de que a chamada passou pela porta com o fiscal ligado. Só [`com_adaptador`] cria uma: o campo é
/// privado, então nenhum outro módulo consegue.
pub struct Licenca {
    _so_a_porta_cria: (),
}

/// O que o adaptador recebe para transmitir ou consultar uma nota (pacote E).
#[derive(Debug, Clone, PartialEq)]
pub struct NotaParaEnviar {
    pub id: String,
    /// 1 = produção, 2 = homologação.
    pub ambiente: i64,
    /// 55 = NF-e, 65 = NFC-e.
    pub modelo: i64,
    pub serie: i64,
    pub numero: i64,
    /// A chave de acesso, quando já se sabe (vazia antes da primeira resposta).
    pub chave: String,
    pub valor: i64,
    pub destinatario_doc: String,
    pub destinatario_nome: String,
}

/// Uma resposta conclusiva da SEFAZ, pelo adaptador: o cStat e o motivo na íntegra.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RespostaSefaz {
    pub chave: String,
    pub cstat: String,
    pub motivo: String,
    pub protocolo: String,
    pub xml_nome: String,
}

/// O adaptador fiscal (o ACBrMonitorPLUS por TCP/IP local, DF-3, ou o falso dos testes). Todo método recebe a
/// licença: sem passar pela porta, não há como chamar. Um erro (`Err`) quer dizer **sem resposta conclusiva** (a
/// conexão caiu, o tempo acabou): a nota fica como "desconhecida", e o certo é consultar antes de reenviar.
pub trait Adaptador {
    /// O estado do serviço (a primeira pergunta de qualquer conversa com o adaptador).
    fn status(&mut self, licenca: &Licenca) -> Resultado<String>;
    /// Transmite a nota e devolve a resposta da SEFAZ.
    fn enviar(&mut self, licenca: &Licenca, nota: &NotaParaEnviar) -> Resultado<RespostaSefaz>;
    /// Consulta a situação da nota na SEFAZ (pela chave, ou pelo número quando a chave não chegou).
    fn consultar(&mut self, licenca: &Licenca, nota: &NotaParaEnviar) -> Resultado<RespostaSefaz>;
}

/// A única forma de falar com o adaptador e com o certificado. Desligado, recusa sem chamar nada.
pub fn com_adaptador<T>(
    conn: &Connection,
    adaptador: &mut dyn Adaptador,
    uso: impl FnOnce(&mut dyn Adaptador, &Licenca) -> Resultado<T>,
) -> Resultado<T> {
    if !ligado(conn)? {
        return Err(Erro::validacao(DESLIGADO));
    }
    let licenca = Licenca { _so_a_porta_cria: () };
    uso(adaptador, &licenca)
}

// ------------------------------------------------------------- comandos das telas

pub use comandos::COMANDOS;

mod comandos {
    use serde::Deserialize;
    use serde_json::{json, Value};

    use crate::despacho::{ler, Classe, Comando, Pedido};
    use crate::erro::Resultado;

    pub const COMANDOS: &[Comando] = &[
        Comando { nome: "fiscal_liga_estado", classe: Classe::Leitura, funcao: fiscal_liga_estado },
        Comando { nome: "fiscal_ligar", classe: Classe::Caixa, funcao: fiscal_ligar },
        Comando { nome: "fiscal_desligar", classe: Classe::Caixa, funcao: fiscal_desligar },
    ];

    fn fiscal_liga_estado(p: &mut Pedido, _dados: Value) -> Resultado<Value> {
        Ok(json!(super::estado(p.conn, p.gerente)?))
    }

    /// C-M11 da R1: a senha é conferida antes (o erro fica registrado mesmo que o resto falhe); o "ligado" e o
    /// registro vão numa transação só.
    fn fiscal_ligar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: Senha = ler(dados)?;
        if !p.gerente {
            return Err(crate::erro::Erro::validacao(
                "Só um gerente liga o uso do certificado A3 e a integração fiscal.",
            ));
        }
        let nome = super::conferir_senha_do_gerente(p.conn, p.disp, p.usuario_id, &d.senha, "ligar o fiscal")?;
        let tx = p.conn.transaction()?;
        super::gravar_ligado(&tx, p.disp, p.usuario_id, p.gerente, &nome)?;
        tx.commit()?;
        Ok(json!({ "nome": nome, "estado": super::estado(p.conn, p.gerente)? }))
    }

    fn fiscal_desligar(p: &mut Pedido, _dados: Value) -> Resultado<Value> {
        let tx = p.conn.transaction()?;
        super::desligar(&tx, p.disp, p.usuario_id, p.gerente)?;
        tx.commit()?;
        Ok(json!(super::estado(p.conn, p.gerente)?))
    }

    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Senha {
        senha: String,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;

    /// O adaptador falso conta as chamadas: com o fiscal desligado, tem de ficar em zero.
    struct Falso {
        chamadas: u32,
    }

    impl Adaptador for Falso {
        fn status(&mut self, _licenca: &Licenca) -> Resultado<String> {
            self.chamadas += 1;
            Ok("OK: 107 - Serviço em Operação".into())
        }
        fn enviar(&mut self, _licenca: &Licenca, _nota: &NotaParaEnviar) -> Resultado<RespostaSefaz> {
            self.chamadas += 1;
            Ok(RespostaSefaz::default())
        }
        fn consultar(&mut self, _licenca: &Licenca, _nota: &NotaParaEnviar) -> Resultado<RespostaSefaz> {
            self.chamadas += 1;
            Ok(RespostaSefaz::default())
        }
    }

    fn banco_com_gerentes() -> (Banco, String, String, String) {
        let b = Banco::em_memoria().unwrap();
        let ana = usuarios::criar(&b.conn, &b.dispositivo, "Ana", "senha-da-ana-1", true).unwrap().id;
        let bia = usuarios::criar(&b.conn, &b.dispositivo, "Bia", "senha-da-bia-2", true).unwrap().id;
        let caio = usuarios::criar(&b.conn, &b.dispositivo, "Caio", "senha-do-caio-3", true).unwrap().id;
        usuarios::definir_gerente(&b.conn, &ana, true).unwrap();
        usuarios::definir_gerente(&b.conn, &bia, true).unwrap();
        (b, ana, bia, caio)
    }

    fn acoes(b: &Banco, acao: &str) -> i64 {
        b.conn.query_row("SELECT COUNT(*) FROM auditoria WHERE acao = ?1", [acao], |l| l.get(0)).unwrap()
    }

    #[test]
    fn fis_liga_nasce_desligado_e_nao_viaja_para_os_outros_caixas() {
        let b = Banco::em_memoria().unwrap();
        assert!(!ligado(&b.conn).unwrap());
        assert!(!crate::sync::chave_da_loja(CHAVE), "o seletor é só deste computador");
    }

    #[test]
    fn fis_liga_desligado_a_porta_nao_chama_o_adaptador() {
        let b = Banco::em_memoria().unwrap();
        let mut falso = Falso { chamadas: 0 };
        let r = com_adaptador(&b.conn, &mut falso, |a, l| a.status(l));
        assert!(r.is_err());
        assert_eq!(falso.chamadas, 0);
    }

    #[test]
    fn fis_liga_ligar_pede_gerente_e_a_senha_certa() {
        let (b, ana, _bia, caio) = banco_com_gerentes();
        // quem não é gerente não liga, nem com a senha certa de um gerente
        assert!(ligar(&b.conn, &b.dispositivo, &caio, false, "senha-da-ana-1").is_err());
        // senha vazia ou errada não liga, e a errada fica no histórico
        assert!(ligar(&b.conn, &b.dispositivo, &ana, true, "").is_err());
        let errada = ligar(&b.conn, &b.dispositivo, &ana, true, "senha-do-caio-3").unwrap_err();
        assert_eq!(errada.nome_do_campo(), Some("senha"));
        assert_eq!(acoes(&b, "gerente.senha_errada"), 1);
        assert!(!ligado(&b.conn).unwrap());
        // com a senha de um gerente, liga
        assert_eq!(ligar(&b.conn, &b.dispositivo, &ana, true, "senha-da-ana-1").unwrap(), "Ana");
        assert!(ligado(&b.conn).unwrap());
        assert_eq!(acoes(&b, "fiscal.ligar"), 1);
        // ligado, a porta chama
        let mut falso = Falso { chamadas: 0 };
        assert!(com_adaptador(&b.conn, &mut falso, |a, l| a.status(l)).is_ok());
        assert_eq!(falso.chamadas, 1);
    }

    #[test]
    fn fis_liga_qualquer_gerente_desliga_e_quem_nao_e_gerente_nao() {
        let (b, ana, bia, caio) = banco_com_gerentes();
        ligar(&b.conn, &b.dispositivo, &ana, true, "senha-da-ana-1").unwrap();
        assert!(desligar(&b.conn, &b.dispositivo, &caio, false).is_err());
        assert!(ligado(&b.conn).unwrap());
        desligar(&b.conn, &b.dispositivo, &bia, true).unwrap();
        assert!(!ligado(&b.conn).unwrap());
        assert_eq!(acoes(&b, "fiscal.desligar"), 1);
        let mut falso = Falso { chamadas: 0 };
        assert!(com_adaptador(&b.conn, &mut falso, |a, l| a.status(l)).is_err());
        assert_eq!(falso.chamadas, 0);
    }

    #[test]
    fn fis_liga_muitas_senhas_erradas_fazem_esperar() {
        let (b, ana, _bia, _caio) = banco_com_gerentes();
        for _ in 0..TENTATIVAS_MAXIMAS {
            assert!(ligar(&b.conn, &b.dispositivo, &ana, true, "errada").is_err());
        }
        // a próxima, mesmo certa, espera
        let e = ligar(&b.conn, &b.dispositivo, &ana, true, "senha-da-ana-1").unwrap_err();
        assert!(e.to_string().contains("Espere"), "{e}");
        assert!(!ligado(&b.conn).unwrap());
    }

    #[test]
    fn fis_liga_o_estado_continua_depois_de_fechar_e_abrir() {
        let pasta = std::env::temp_dir().join(format!("rustinho-fis-liga-{}", crate::tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let caminho = pasta.join("rustinho.db");
        {
            let b = Banco::abrir(&caminho).unwrap();
            let ana = usuarios::criar(&b.conn, &b.dispositivo, "Ana", "senha-da-ana-1", true).unwrap().id;
            usuarios::definir_gerente(&b.conn, &ana, true).unwrap();
            ligar(&b.conn, &b.dispositivo, &ana, true, "senha-da-ana-1").unwrap();
        }
        let b = Banco::abrir(&caminho).unwrap();
        assert!(ligado(&b.conn).unwrap());
        drop(b);
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// A licença só nasce na porta: em todo o núcleo, só um lugar monta a licença (o padrão é montado aqui em tempo
    /// de execução, para o próprio teste não contar).
    #[test]
    fn fis_liga_a_licenca_so_e_criada_na_porta() {
        let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let alvo = format!("{{ {}: () }}", ["_so", "a", "porta", "cria"].join("_"));
        let mut achados = Vec::new();
        let mut pilha = vec![raiz];
        while let Some(p) = pilha.pop() {
            for e in std::fs::read_dir(&p).unwrap() {
                let e = e.unwrap().path();
                if e.is_dir() {
                    pilha.push(e);
                } else if e.extension().is_some_and(|x| x == "rs") {
                    let texto = std::fs::read_to_string(&e).unwrap();
                    let n = texto.matches(&alvo).count();
                    if n > 0 {
                        achados.push((e.file_name().unwrap().to_string_lossy().to_string(), n));
                    }
                }
            }
        }
        assert_eq!(achados, vec![("fiscal_porta.rs".to_string(), 1)], "{achados:?}");
    }

    /// C-M11 da R1: ligar e desligar gravam o seletor e o registro juntos. Se o registro falha, o seletor não muda (e o
    /// gerente recebe o erro); a senha errada continua registrada.
    #[test]
    fn r1_cm11_ligar_e_desligar_sao_atomicos_com_o_registro() {
        use serde_json::{json, Value};
        let mut s = crate::Servico::em_memoria().unwrap();
        let chamar = |s: &mut crate::Servico, cmd: &str, d: Value| -> Value {
            serde_json::from_str(&s.chamar(cmd, &d.to_string())).unwrap()
        };
        let r = chamar(
            &mut s,
            "configurar_inicio",
            json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "12345678", "senha2": "12345678" }),
        );
        assert_eq!(r["ok"], true, "{r}");
        s.banco_para_teste()
            .conn
            .execute_batch(
                "CREATE TEMP TRIGGER t_cm11 BEFORE INSERT ON auditoria WHEN NEW.acao LIKE 'fiscal.%'
                 BEGIN SELECT RAISE(ABORT, 'o registro falhou'); END;",
            )
            .unwrap();
        let r = chamar(&mut s, "fiscal_ligar", json!({ "senha": "12345678" }));
        assert_eq!(r["ok"], false, "{r}");
        assert!(!ligado(&s.banco_para_teste().conn).unwrap(), "o registro falhou: o seletor não pode ter ligado");
        // A senha errada fica registrada mesmo assim (o registro dela não é 'fiscal.%').
        let r = chamar(&mut s, "fiscal_ligar", json!({ "senha": "errada" }));
        assert_eq!(r["ok"], false, "{r}");
        let erradas: i64 = s
            .banco_para_teste()
            .conn
            .query_row("SELECT COUNT(*) FROM auditoria WHERE acao = 'gerente.senha_errada'", [], |l| l.get(0))
            .unwrap();
        assert_eq!(erradas, 1);
        // Desligar: com o registro falhando, continua ligado.
        s.banco_para_teste().conn.execute_batch("DROP TRIGGER t_cm11;").unwrap();
        assert_eq!(chamar(&mut s, "fiscal_ligar", json!({ "senha": "12345678" }))["ok"], true);
        s.banco_para_teste()
            .conn
            .execute_batch(
                "CREATE TEMP TRIGGER t_cm11 BEFORE INSERT ON auditoria WHEN NEW.acao LIKE 'fiscal.%'
                 BEGIN SELECT RAISE(ABORT, 'o registro falhou'); END;",
            )
            .unwrap();
        assert_eq!(chamar(&mut s, "fiscal_desligar", json!({}))["ok"], false);
        assert!(ligado(&s.banco_para_teste().conn).unwrap(), "o registro falhou: o seletor não pode ter desligado");
    }
}
