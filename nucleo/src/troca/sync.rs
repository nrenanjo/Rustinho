//! Sincronização entre os caixas da loja (pela rede local / Wi-Fi).
//!
//! Como funciona:
//! - Toda alteração nas tabelas da loja vira uma linha em `alteracao`
//!   (gatilhos do SQLite criados ao abrir o banco).
//! - Cada caixa escuta na porta TCP 47810 e anuncia que existe pela porta
//!   UDP 47811 (assim um acha o outro sozinho). Também dá para digitar o
//!   endereço dos outros caixas.
//! - A cada poucos segundos, cada caixa pede aos outros as alterações que
//!   ainda não tem ("desde a número N") e aplica.
//! - Cada alteração guarda sobre qual versão foi feita ("base"), um contador
//!   do registro ("ordem": base + 1) e as versões anteriores que o caixa
//!   conhecia ("historia", até 8, só como atalho). Vale a de maior ordem (empate: maior id,
//!   o UUID v7). Uma alteração feita depois de outra sempre vale, mesmo com
//!   o relógio do caixa atrasado, e todos os caixas escolhem a mesma, em
//!   qualquer ordem de chegada (R-08).
//! - Duas alterações em que nenhuma está na história da outra foram feitas
//!   sem uma ver a outra: conflito. Se a história não basta para saber,
//!   também conta como conflito ("ordem incerta"). Nos dois casos as duas
//!   versões ficam guardadas e o gerente decide; a decisão vai para os
//!   outros caixas. Nada disso olha o relógio.
//! - Compactar o log só tira os dados das versões velhas (ficam id, base e
//!   ordem): a cadeia de versões continua inteira em todos os caixas. Uma
//!   alteração repetida (reenvio, backup restaurado) é reconhecida pelo id
//!   e não vira aviso de novo.
//! - O estoque é só soma de movimentos, então nunca há conflito nele.
//! - Tudo que passa pela rede vai criptografado (ChaCha20-Poly1305) com uma
//!   chave tirada da "senha da rede" da loja por Argon2id, com um "sal" de
//!   cada caixa que vai junto na mensagem (protocolo versão 2, desde a
//!   0.9.11). A senha precisa ser a mesma nos caixas: sem ela, nada é lido
//!   nem aceito.
//! - Registros repetidos no log de alterações (versões velhas de um mesmo
//!   registro) são apagados de tempos em tempos: só a mais nova importa.
//! - A diferença de relógio entre caixas aparece como aviso na tela dos
//!   caixas da loja (só para conferir; não decide nada).
//! - Protocolo 3 (desde o fim da 0.9.11): mensagens começam com "RST3".
//!   Caixa com outro protocolo é recusado antes de ler qualquer dado.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::thread;
use std::time::{Duration, Instant};

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::ChaCha20Poly1305;
use rusqlite::types::Value as SqlValor;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::caminhos::{self, CaminhoPar, Puxada};
use crate::erro::{Erro, Resultado};
use crate::rede_local::{self, Interface, Saida};
use crate::{banco, config, tempo};

mod ajustes;
mod aplicacao;
mod base;
mod caixas;
mod cifra;
mod cliente;
mod conflitos;
mod motor;
mod referencias;
mod registro;
mod servidor;
mod versoes;

pub use ajustes::*;
pub use aplicacao::*;
pub use base::*;
pub use caixas::*;
use cifra::*;
pub use cliente::*;
pub use conflitos::*;
pub use motor::*;
pub use referencias::*;
pub use registro::*;
pub use servidor::*;
pub use versoes::*;

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;

    /// D-16: configuração só deste computador que chega pela rede é
    /// ignorada; as da loja continuam chegando.
    #[test]
    fn d16_configuracao_deste_computador_nao_vem_da_rede() {
        let mut b = Banco::em_memoria().unwrap();
        let identidade = config::ler(&b.conn, "dispositivo_id").unwrap();
        config::gravar(&b.conn, "sync_senha", "segredo-daqui").unwrap();
        let alteracao = |chave: &str, valor: &str| Alteracao {
            seq: 1,
            id: crate::tempo::novo_id(),
            tabela: "config".into(),
            chave: json!([chave]).to_string(),
            operacao: "gravar".into(),
            dados: Some(json!({ "chave": chave, "valor": valor }).to_string()),
            dispositivo: "outro".into(),
            quando: crate::tempo::agora(),
            base: None,
            ordem: 1,
            historia: None,
            compactada: false,
            operacao_id: None,
            incompativel: None,
        };
        let lista = [
            alteracao("dispositivo_id", "invasor"),
            alteracao("sync_senha", "outra"),
            alteracao("caixa_nome", "Caixa do invasor"),
            alteracao("loja_nome", "Loja Nova"),
        ];
        aplicar(&mut b.conn, &lista).unwrap();
        assert_eq!(config::ler(&b.conn, "dispositivo_id").unwrap(), identidade);
        assert_eq!(config::ler(&b.conn, "sync_senha").unwrap().as_deref(), Some("segredo-daqui"));
        assert_ne!(config::ler(&b.conn, "caixa_nome").unwrap().as_deref(), Some("Caixa do invasor"));
        assert_eq!(config::nome_loja(&b.conn).unwrap(), "Loja Nova", "as da loja continuam chegando");
        // Chave estranha (não é uma lista com um texto): ignorada.
        let mut ruim = alteracao("loja_nome", "x");
        ruim.chave = "[1, 2]".into();
        aplicar(&mut b.conn, &[ruim]).unwrap();
        assert_eq!(config::nome_loja(&b.conn).unwrap(), "Loja Nova");
    }
    use crate::{caixa, estoque, produtos, usuarios, vendas};

    fn temp(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("rustinho-sync-{nome}-{}", tempo::novo_id()));
        std::fs::create_dir_all(&p).unwrap();
        p.join("rustinho.db")
    }

    /// Pedido de faixa de códigos das peças pela rede: os caixas novos puxam
    /// do distribuidor, pedem a faixa (dois ao mesmo tempo) e já dão códigos
    /// dela; as faixas são diferentes.
    #[test]
    fn faixa_de_codigos_pela_rede() {
        thread::spawn(|| {
            let ca = temp("faixa-a");
            let a = Banco::abrir(&ca).unwrap();
            let u = crate::faixas::para_testes::configurar(&a, "Caixa A", 0);
            salvar_ajustes(&a.conn, true, "senha-da-loja", "").unwrap();
            let mut ma = Motor::iniciar(ca.clone(), 0, 0);
            let (pt_a, _) = ma.portas(Duration::from_secs(10)).expect("as portas do motor abriram");
            let para_a = format!("127.0.0.1:{pt_a}");
            let mut tarefas = Vec::new();
            for (i, nome) in ["Caixa B", "Caixa C"].into_iter().enumerate() {
                let (para_a, u) = (para_a.clone(), u.clone());
                tarefas.push(thread::spawn(move || {
                    let c = temp(&format!("faixa-{i}"));
                    let b = Banco::abrir(&c).unwrap();
                    config::gravar(&b.conn, "caixa_nome", nome).unwrap();
                    salvar_ajustes(&b.conn, true, "senha-da-loja", "").unwrap();
                    puxar(&c, &para_a, 48220 + i as u16).unwrap();
                    let p = crate::faixas::para_testes::peca(&b, &u, "Peça nova");
                    (crate::faixas::para_testes::minhas(&b), crate::faixas::para_testes::codigo(&b, &p))
                }));
            }
            let mut r: Vec<(Vec<(i64, i64)>, String)> = tarefas.into_iter().map(|t| t.join().unwrap()).collect();
            r.sort();
            assert_eq!(
                r,
                vec![
                    (vec![(100_000, 199_999)], "100000".to_string()),
                    (vec![(200_000, 299_999)], "200000".to_string())
                ]
            );
            assert!(crate::faixas::cruzadas(&a.conn).unwrap().is_empty());
            assert!(ma.encerrar(Duration::from_secs(20)));
        })
        .join()
        .unwrap();
    }

    /// Banco copiado de outro computador (identidade diferente da guardada):
    /// não responde a quem pede dados nem puxa de ninguém até ser registrado
    /// como caixa novo; depois do registro, troca de novo com os outros.
    #[test]
    fn banco_copiado_nao_troca_dados_pela_rede_ate_ser_registrado() {
        thread::spawn(|| {
            let (ca, cb) = (temp("copia-rede-a"), temp("copia-rede-b"));
            let a = Banco::abrir(&ca).unwrap();
            let u = crate::faixas::para_testes::configurar(&a, "Caixa A", 0);
            salvar_ajustes(&a.conn, true, "senha-da-loja", "").unwrap();
            crate::faixas::para_testes::peca(&a, &u, "Filtro");
            let b = Banco::abrir(&cb).unwrap();
            config::gravar(&b.conn, "caixa_nome", "Caixa B").unwrap();
            salvar_ajustes(&b.conn, true, "senha-da-loja", "").unwrap();
            // A vira "cópia": o banco diz que nasceu em outro computador.
            config::gravar(&a.conn, "computador_id", &crate::computador::impressao("outro-pc")).unwrap();
            crate::faixas::conferir_computador(&a.conn, crate::computador::atual().as_deref()).unwrap();
            assert!(crate::faixas::copia_detectada(&a.conn).unwrap());
            let mut ma = Motor::iniciar(ca.clone(), 0, 0);
            let (pt_a, _) = ma.portas(Duration::from_secs(10)).expect("as portas do motor abriram");
            let para_a = format!("127.0.0.1:{pt_a}");
            // Quem pede a ela não recebe nada (C8: recusado por ela, com o
            // servidor no ar — não "Connection refused").
            let r = puxar(&cb, &para_a, 48302);
            assert!(
                matches!(&r, Err(e) if !e.to_string().contains("Connection refused") && !e.to_string().contains("os error 111")),
                "{r:?}"
            );
            let produtos_b: i64 = b.conn.query_row("SELECT COUNT(*) FROM produto", [], |l| l.get(0)).unwrap();
            assert_eq!(produtos_b, 0);
            // Ela também não puxa de ninguém.
            let r = puxar(&ca, "127.0.0.1:48399", pt_a);
            assert!(matches!(r, Err(Erro::Validacao { .. })), "{r:?}");
            // Registrada como caixa nova, volta a trocar.
            crate::faixas::registrar_caixa_novo(&a.conn, crate::computador::atual().as_deref(), Some("Caixa A2"))
                .unwrap();
            assert!(!crate::faixas::copia_detectada(&a.conn).unwrap());
            puxar(&cb, &para_a, 48302).unwrap();
            let produtos_b: i64 = b.conn.query_row("SELECT COUNT(*) FROM produto", [], |l| l.get(0)).unwrap();
            assert_eq!(produtos_b, 1);
            assert!(ma.encerrar(Duration::from_secs(20)));
        })
        .join()
        .unwrap();
    }

    /// Item 1 da revisão: o caixa A volta para trás (backup restaurado no
    /// mesmo computador, computador novo no mesmo endereço, ou banco copiado
    /// à mão) e faz uma alteração nova. O caixa B, que guardava um contador
    /// alto de A, tem de receber a alteração nova; A recupera o que perdeu;
    /// nada é aplicado duas vezes (vendas, itens, estoque), o apagado não
    /// volta, e depois os dois continuam trocando normalmente.
    #[test]
    fn recuo_do_contador_e_ressincronizacao() {
        // Cada cenário numa tarefa própria: o limite de contas caras dos
        // testes é por tarefa, e cada cenário tem caixas novos.
        for (n, cenario) in ["mesmo computador", "computador novo", "copia manual"].into_iter().enumerate() {
            thread::spawn(move || recuo_um_cenario(n, cenario)).join().expect(cenario);
        }
    }

    fn recuo_um_cenario(n: usize, cenario: &str) {
        {
            let ca = temp(&format!("recuo-a-{n}"));
            let cb = temp(&format!("recuo-b-{n}"));
            let a = Banco::abrir(&ca).unwrap();
            let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "1234", true).unwrap().id;
            usuarios::definir_gerente(&a.conn, &u, true).unwrap();
            config::gravar(&a.conn, "caixa_nome", "Caixa A").unwrap();
            salvar_ajustes(&a.conn, true, "senha-da-loja", "").unwrap();
            let b = Banco::abrir(&cb).unwrap();
            config::gravar(&b.conn, "caixa_nome", "Caixa B").unwrap();
            salvar_ajustes(&b.conn, true, "senha-da-loja", "").unwrap();
            // Portas escolhidas pelo sistema (C8); depois A abre de novo na
            // mesma porta (os outros caixas a conhecem: "mesmo endereço").
            let mut ma = Motor::iniciar(ca.clone(), 0, 0);
            let _mb = Motor::iniciar(cb.clone(), 0, 0);
            let (pt_a, pu_a) = ma.portas(Duration::from_secs(10)).expect("as portas do motor abriram");
            let (pt_b, _) = _mb.portas(Duration::from_secs(10)).expect("as portas do motor abriram");
            let (para_a, para_b) = (format!("127.0.0.1:{pt_a}"), format!("127.0.0.1:{pt_b}"));
            let trocar_rede = || {
                for _ in 0..3 {
                    puxar(&cb, &para_a, pt_b).unwrap();
                    puxar(&ca, &para_b, pt_a).unwrap();
                }
            };
            // A tem uma peça com aplicação, faz uma venda; B recebe tudo.
            let mut f = produtos::ficha_nova(&a.conn).unwrap();
            f.nome = "Correia".into();
            f.custo = "10".into();
            f.estoque_inicial = "10".into();
            f.aplicacoes =
                vec![produtos::AplicacaoFicha { montadora: "Fiat".into(), modelo: "Uno".into(), ..Default::default() }];
            let p = produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
            caixa::abrir(&a.conn, &a.dispositivo, &u, "Caixa A", "0").unwrap();
            let venda = |x: &Banco| {
                let e = vendas::VendaEntrada {
                    itens: vec![vendas::ItemEntrada {
                        produto_id: p.clone(),
                        quantidade: "1".into(),
                        ..Default::default()
                    }],
                    tabela: 1,
                    pagamentos: vec![vendas::PagamentoEntrada {
                        forma: "dinheiro".into(),
                        valor: "100".into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                };
                vendas::concluir(&x.conn, &x.dispositivo, &u, &e).unwrap();
            };
            venda(&a);
            trocar_rede();
            // Cópia do banco de A (o "backup" ou a cópia manual).
            let copia = ca.with_file_name("copia.db");
            a.conn.backup(rusqlite::MAIN_DB, &copia, None).unwrap();
            let seq_antes_do_backup: i64 =
                a.conn.query_row("SELECT MAX(seq) FROM alteracao", [], |l| l.get(0)).unwrap();
            // Depois da cópia: A apaga a aplicação, vende de novo, mexe muito
            // (o contador de B para A sobe bem acima do da cópia).
            let mut f = produtos::ficha(&a.conn, &p).unwrap();
            f.aplicacoes.clear();
            produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
            venda(&a);
            for i in 0..30 {
                let mut f = produtos::ficha(&a.conn, &p).unwrap();
                f.observacoes = format!("obs {i}");
                produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
            }
            trocar_rede();
            let cursor_b: i64 = b
                .conn
                .query_row("SELECT cursor FROM sync_par WHERE dispositivo = ?1", [&a.dispositivo], |l| l.get(0))
                .unwrap();
            assert!(cursor_b > seq_antes_do_backup, "{cenario}: B guardou um contador alto de A");
            // A volta para trás.
            assert!(ma.encerrar(Duration::from_secs(20)));
            let seq_antes: i64 = a.conn.query_row("SELECT MAX(seq) FROM alteracao", [], |l| l.get(0)).unwrap();
            drop(a);
            for fim in ["", "-wal", "-shm"] {
                let mut x = ca.as_os_str().to_owned();
                x.push(fim);
                let _ = std::fs::remove_file(std::path::PathBuf::from(x));
            }
            match cenario {
                "mesmo computador" => {
                    std::fs::copy(&copia, &ca).unwrap();
                    let a = Banco::abrir(&ca).unwrap();
                    depois_de_restaurar(&a.conn, seq_antes).unwrap();
                }
                "copia manual" => {
                    std::fs::copy(&copia, &ca).unwrap();
                }
                _ => {
                    // Computador novo no mesmo endereço: banco vazio, entra na rede.
                    let a = Banco::abrir(&ca).unwrap();
                    config::gravar(&a.conn, "caixa_nome", "Caixa A").unwrap();
                    salvar_ajustes(&a.conn, true, "senha-da-loja", "").unwrap();
                }
            }
            let mut a = Banco::abrir(&ca).unwrap();
            let _ma = Motor::iniciar(ca.clone(), pt_a, pu_a);
            assert_eq!(
                _ma.portas(Duration::from_secs(10)).expect("as portas do motor abriram"),
                (pt_a, pu_a),
                "A abriu de novo no mesmo endereço"
            );
            if cenario == "computador novo" {
                // FALHA-01: o computador novo entra na loja pelo caminho da
                // junção (banco vazio, recebe tudo primeiro). Antes ele criava
                // um usuário próprio e só depois trocava dados, que é o que a
                // proteção contra a fusão de lojas independentes recusa.
                puxar(&ca, &para_b, pt_a).unwrap();
            }
            // A, antes de qualquer troca de volta, faz uma alteração nova.
            let mut g = produtos::ficha_nova(&a.conn).unwrap();
            g.nome = format!("Feita depois de voltar ({cenario})");
            g.custo = "5".into();
            // Sem faixa de códigos ainda: a peça fica com código a definir
            // (antes das faixas, o computador novo dava o código 1, que já
            // era de outra peça na loja).
            let nova = produtos::salvar(&a.conn, &a.dispositivo, &u, &g).unwrap_or_else(|_| {
                // Computador novo ainda não tem o usuário: cria um.
                let u2 = usuarios::criar(&a.conn, &a.dispositivo, "Bia", "1234", true).unwrap().id;
                produtos::salvar(&a.conn, &a.dispositivo, &u2, &g).unwrap()
            });
            // B pede a A com o contador antigo: tem de receber a alteração nova.
            puxar(&cb, &para_a, pt_b).unwrap();
            assert!(produtos::ficha(&b.conn, &nova).is_ok(), "{cenario}: B recebeu a alteração nova de A");
            trocar_rede();
            let contar = |x: &Banco, sql: &str| -> i64 { x.conn.query_row(sql, [], |l| l.get(0)).unwrap() };
            for (quem, x) in [("A", &a), ("B", &b)] {
                assert_eq!(
                    contar(x, "SELECT COUNT(*) FROM venda WHERE tipo = 'venda'"),
                    2,
                    "{cenario} {quem}: duas vendas, sem repetir"
                );
                assert_eq!(contar(x, "SELECT COUNT(*) FROM venda_item"), 2, "{cenario} {quem}");
                assert_eq!(estoque::saldo(&x.conn, &p).unwrap(), 8000, "{cenario} {quem}: estoque certo");
                assert!(
                    produtos::ficha(&x.conn, &p).unwrap().aplicacoes.is_empty(),
                    "{cenario} {quem}: o apagado não volta"
                );
                assert_eq!(produtos::ficha(&x.conn, &p).unwrap().observacoes, "obs 29", "{cenario} {quem}");
                assert!(produtos::ficha(&x.conn, &nova).is_ok(), "{cenario} {quem}");
                assert_eq!(pendencias(&x.conn).unwrap(), 0, "{cenario} {quem}: nada apontando para o que falta");
                assert_eq!(
                    conflitos_abertos(&x.conn).unwrap(),
                    0,
                    "{cenario} {quem}: sem aviso falso: {:?}",
                    x.conn
                        .prepare("SELECT tabela, chave, ficou, perdeu, certeza, dados_perdeu FROM sync_conflito")
                        .unwrap()
                        .query_map([], |l| Ok(format!(
                            "{:?}",
                            (
                                l.get::<_, String>(0)?,
                                l.get::<_, String>(1)?,
                                l.get::<_, String>(4)?,
                                l.get::<_, Option<String>>(5)?
                            )
                        )))
                        .unwrap()
                        .collect::<Vec<_>>()
                );
            }
            // Depois, os dois continuam trocando alterações novas.
            let mut h = produtos::ficha_nova(&b.conn).unwrap();
            h.nome = "Nova de B".into();
            h.custo = "1".into();
            let de_b = produtos::salvar(&b.conn, &b.dispositivo, &u, &h).unwrap();
            let mut f = produtos::ficha(&a.conn, &nova).unwrap();
            f.nome = "Nova de A, mexida".into();
            let quem_salva = usuarios::listar(&a.conn, false).unwrap()[0].id.clone();
            produtos::salvar(&a.conn, &a.dispositivo, &quem_salva, &f).unwrap();
            trocar_rede();
            assert!(produtos::ficha(&a.conn, &de_b).is_ok(), "{cenario}: A recebe de B");
            assert_eq!(produtos::ficha(&b.conn, &nova).unwrap().nome, "Nova de A, mexida", "{cenario}: B recebe de A");
            let _ = &mut a;
        }
    }

    #[test]
    fn dois_caixas_pela_rede() {
        // Caixa 1: já tinha dados antes da sincronização.
        let c1 = temp("c1");
        let b1 = Banco::abrir(&c1).unwrap();
        let u = usuarios::criar(&b1.conn, &b1.dispositivo, "Maria", "1234", true).unwrap();
        let mut f = produtos::ficha_nova(&b1.conn).unwrap();
        f.nome = "Pastilha".into();
        f.custo = "40".into();
        f.estoque_inicial = "5".into();
        let p = produtos::salvar(&b1.conn, &b1.dispositivo, &u.id, &f).unwrap();
        config::gravar(&b1.conn, "loja_nome", "Loja Modelo").unwrap();
        config::gravar(&b1.conn, "caixa_nome", "Caixa 1").unwrap();
        salvar_ajustes(&b1.conn, true, "senha-da-loja", "").unwrap();
        drop(b1);
        let b1 = Banco::abrir(&c1).unwrap(); // reabrir não duplica o registro inicial
        let n1: i64 = b1.conn.query_row("SELECT COUNT(*) FROM alteracao", [], |l| l.get(0)).unwrap();
        drop(b1);
        let b1 = Banco::abrir(&c1).unwrap();
        let n1b: i64 = b1.conn.query_row("SELECT COUNT(*) FROM alteracao", [], |l| l.get(0)).unwrap();
        assert_eq!(n1, n1b);

        // Caixa 2: novo, entra na rede da loja.
        let c2 = temp("c2");
        let b2 = Banco::abrir(&c2).unwrap();
        config::gravar(&b2.conn, "caixa_nome", "Caixa 2").unwrap();
        salvar_ajustes(&b2.conn, true, "senha-da-loja", "").unwrap();

        // Portas escolhidas pelo sistema na hora de abrir (C8): as fixas de
        // antes (47910/47920) ficam na faixa de portas de saída do sistema e
        // podem estar ocupadas por uma conexão qualquer.
        let _m1 = Motor::iniciar(c1.clone(), 0, 0);
        let _m2 = Motor::iniciar(c2.clone(), 0, 0);
        let (pt1, _) = _m1.portas(Duration::from_secs(10)).expect("o motor do caixa 1 abriu as portas");
        let (pt2, _) = _m2.portas(Duration::from_secs(10)).expect("o motor do caixa 2 abriu as portas");

        // 2 puxa de 1
        let (_, novas) = puxar(&c2, &format!("127.0.0.1:{pt1}"), pt2).unwrap();
        assert!(novas > 0);
        assert_eq!(usuarios::quantidade(&b2.conn).unwrap(), 1, "o usuário chegou");
        assert_eq!(config::nome_loja(&b2.conn).unwrap(), "Loja Modelo");
        assert_eq!(config::nome_caixa(&b2.conn).unwrap(), "Caixa 2", "o nome do caixa não é copiado");
        assert_eq!(estoque::saldo(&b2.conn, &p).unwrap(), 5000);
        assert_eq!(produtos::ficha(&b2.conn, &p).unwrap().precos[0].preco, "80,00");

        // venda no caixa 2 (saída de estoque) e preço mudado no caixa 1
        estoque::lancar(&b2.conn, &b2.dispositivo, Some(&u.id), &p, -2000, "venda", "Venda 1 · Caixa 2").unwrap();
        let mut f1 = produtos::ficha(&b1.conn, &p).unwrap();
        f1.precos[0].preco = "85,00".into();
        f1.precos[0].lucro = String::new();
        produtos::salvar(&b1.conn, &b1.dispositivo, &u.id, &f1).unwrap();

        puxar(&c1, &format!("127.0.0.1:{pt2}"), pt1).unwrap();
        puxar(&c2, &format!("127.0.0.1:{pt1}"), pt2).unwrap();
        assert_eq!(estoque::saldo(&b1.conn, &p).unwrap(), 3000);
        assert_eq!(estoque::saldo(&b2.conn, &p).unwrap(), 3000);
        assert_eq!(produtos::ficha(&b2.conn, &p).unwrap().precos[0].preco, "85,00");

        // puxar de novo não traz nada novo (nem fica em eco)
        assert_eq!(puxar(&c2, &format!("127.0.0.1:{pt1}"), pt2).unwrap().1, 0);
        assert_eq!(puxar(&c1, &format!("127.0.0.1:{pt2}"), pt1).unwrap().1, 0);

        // alteração mais nova vence; a mais velha não desfaz
        let mut f2 = produtos::ficha(&b2.conn, &p).unwrap();
        f2.nome = "Pastilha dianteira".into();
        produtos::salvar(&b2.conn, &b2.dispositivo, &u.id, &f2).unwrap();
        thread::sleep(Duration::from_millis(5));
        let mut f1 = produtos::ficha(&b1.conn, &p).unwrap();
        f1.nome = "Pastilha de freio dianteira".into();
        produtos::salvar(&b1.conn, &b1.dispositivo, &u.id, &f1).unwrap();
        puxar(&c1, &format!("127.0.0.1:{pt2}"), pt1).unwrap();
        puxar(&c2, &format!("127.0.0.1:{pt1}"), pt2).unwrap();
        assert_eq!(produtos::ficha(&b1.conn, &p).unwrap().nome, "Pastilha de freio dianteira");
        assert_eq!(produtos::ficha(&b2.conn, &p).unwrap().nome, "Pastilha de freio dianteira");

        // senha errada: recusa
        let c3 = temp("c3");
        let b3 = Banco::abrir(&c3).unwrap();
        salvar_ajustes(&b3.conn, true, "outra-senha-qualquer", "").unwrap();
        let e = puxar(&c3, &format!("127.0.0.1:{pt1}"), 1).unwrap_err().to_string();
        // C8: "Connection refused" quer dizer que o servidor nem estava no ar:
        // aí o teste não provaria a recusa da senha.
        assert!(
            !e.contains("Connection refused") && !e.contains("os error 111"),
            "o servidor do caixa 1 tem de estar no ar para este teste valer: {e}"
        );
        assert!(e.contains("Falha na rede") || e.contains("senha"), "{e}");
        assert_eq!(usuarios::quantidade(&b3.conn).unwrap(), 0);

        // o caixa 1 conheceu o caixa 2 quando foi procurado por ele
        assert!(pares(&b1.conn).unwrap().iter().any(|x| x.nome == "Caixa 2"));
    }

    /// R-09: rajada de pacotes com sais inventados, de um endereço e de
    /// muitos: um endereço só faz no máximo `SAIS_NOVOS_POR_ORIGEM` contas
    /// caras por minuto e todos juntos no máximo `SAIS_NOVOS_POR_MINUTO`; o
    /// controle não cresce sem fim; um caixa novo de verdade, de outro
    /// endereço, ainda entra; os caixas conhecidos continuam sem gastar nada;
    /// pacote repetido não gasta de novo depois de aceito.
    #[test]
    fn rajada_de_sais_inventados() {
        use std::net::{IpAddr, Ipv4Addr};
        let ip = |n: u32| Some(IpAddr::V4(Ipv4Addr::from(0x0A00_0000 + n)));
        let b = Banco::em_memoria().unwrap();
        let k1 = Chaves::de(&b.conn, "senha-da-loja").unwrap();
        let falso = |i: u32| {
            let mut p = MAGICA.to_vec();
            let mut sal = [0u8; 16];
            sal[..4].copy_from_slice(&i.to_be_bytes());
            sal[4] = 0xAB;
            p.extend(sal);
            p.extend([0u8; 12]);
            p.extend([1u8; 40]);
            p
        };
        // Um caixa de verdade conversa antes da rajada (o sal dele fica guardado).
        let sal_bom = [7u8; 16];
        let k2 = Chaves {
            senha: "senha-da-loja".into(),
            sal: sal_bom,
            propria: derivar("senha-da-loja", &sal_bom).unwrap(),
        };
        assert_eq!(decifrar(&k1, &cifrar(&k2, b"ola").unwrap(), ip(1)).unwrap(), b"ola");
        // Rajada de um endereço só: só 3 contas caras.
        let inicio = Instant::now();
        let recusados =
            (0..300).filter(|i| decifrar(&k1, &falso(*i), ip(66)).unwrap_err().contains("Muitos pedidos")).count();
        assert_eq!(recusados, 300 - SAIS_NOVOS_POR_ORIGEM as usize);
        // Um caixa novo de verdade, de outro endereço, ainda entra.
        let sal_novo = [8u8; 16];
        let k3 = Chaves {
            senha: "senha-da-loja".into(),
            sal: sal_novo,
            propria: derivar("senha-da-loja", &sal_novo).unwrap(),
        };
        assert_eq!(decifrar(&k1, &cifrar(&k3, b"cheguei").unwrap(), ip(2)).unwrap(), b"cheguei");
        // Rajada de muitos endereços inventados: o total de contas para no limite de todos.
        let recusados = (0..2000u32)
            .filter(|i| decifrar(&k1, &falso(10_000 + i), ip(1000 + i)).unwrap_err().contains("Muitos pedidos"))
            .count();
        let feitas = SAIS_NOVOS_POR_MINUTO as usize - 1 - SAIS_NOVOS_POR_ORIGEM as usize - 1;
        assert_eq!(recusados, 2000 - feitas);
        assert!(inicio.elapsed() < Duration::from_secs(20), "{:?}", inicio.elapsed());
        // Os conhecidos e o próprio caixa continuam (sem gastar o limite).
        assert_eq!(decifrar(&k1, &cifrar(&k2, b"de novo").unwrap(), ip(1)).unwrap(), b"de novo");
        assert_eq!(decifrar(&k1, &cifrar(&k3, b"eu de novo").unwrap(), ip(2)).unwrap(), b"eu de novo");
        assert_eq!(decifrar(&k1, &cifrar(&k1, b"eu").unwrap(), None).unwrap(), b"eu");
        // O controle não cresce sem fim e volta depois de um minuto.
        let mut p = Porteiro::novo();
        for i in 0..10_000u32 {
            p.permitir(Instant::now(), ip(i));
        }
        assert!(p.por_origem.len() <= ORIGENS_POR_MINUTO);
        assert!(!p.permitir(Instant::now(), ip(99_999)));
        assert!(p.permitir(Instant::now() + Duration::from_secs(61), ip(99_999)));
        assert_eq!(p.por_origem.len(), 1);
    }

    /// Conexão TCP saindo de um endereço escolhido (127.0.0.x), para simular
    /// aparelhos diferentes na rede.
    #[cfg(unix)]
    fn conectar_de(origem: [u8; 4], porta: u16) -> std::io::Result<TcpStream> {
        use std::os::fd::FromRawFd;
        // SAFETY: chamadas de socket do sistema com estruturas zeradas e do
        // tamanho certo; o descritor vira um TcpStream (dono) ou é fechado.
        unsafe {
            let fd = libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0);
            if fd < 0 {
                return Err(std::io::Error::last_os_error());
            }
            let tam = std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;
            let mut de: libc::sockaddr_in = std::mem::zeroed();
            de.sin_family = libc::AF_INET as libc::sa_family_t;
            de.sin_addr.s_addr = u32::from_ne_bytes(origem);
            if libc::bind(fd, &de as *const libc::sockaddr_in as *const libc::sockaddr, tam) != 0 {
                let e = std::io::Error::last_os_error();
                libc::close(fd);
                return Err(e);
            }
            let mut para: libc::sockaddr_in = std::mem::zeroed();
            para.sin_family = libc::AF_INET as libc::sa_family_t;
            para.sin_port = porta.to_be();
            para.sin_addr.s_addr = u32::from_ne_bytes([127, 0, 0, 1]);
            if libc::connect(fd, &para as *const libc::sockaddr_in as *const libc::sockaddr, tam) != 0 {
                let e = std::io::Error::last_os_error();
                libc::close(fd);
                return Err(e);
            }
            Ok(TcpStream::from_raw_fd(fd))
        }
    }

    /// A conexão foi fechada pelo servidor (lê fim de arquivo logo)?
    #[cfg(unix)]
    fn fechada(f: &mut TcpStream) -> bool {
        f.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
        let mut b = [0u8; 1];
        match f.read(&mut b) {
            Ok(0) => true,
            Ok(_) => false,
            Err(e) => !matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut),
        }
    }

    /// Item 5 da revisão: conexões que não mandam nada. Um endereço não
    /// ocupa mais que `VAGAS_POR_ORIGEM` vagas; quem fica mudo cai em
    /// `PRAZO_DO_PEDIDO`; com tudo ocupado por conexões mudas de vários
    /// endereços (e outras chegando sem parar), o caixa de verdade ainda
    /// consegue trocar dados; o servidor nunca passa de `CONEXOES_MAXIMAS`.
    #[cfg(unix)]
    #[test]
    fn conexoes_mudas_nao_tomam_o_servidor() {
        let ca = temp("mudas-a");
        let a = Banco::abrir(&ca).unwrap();
        let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "1234", true).unwrap().id;
        let mut f = produtos::ficha_nova(&a.conn).unwrap();
        f.nome = "Peça do servidor".into();
        f.custo = "1".into();
        let p = produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
        salvar_ajustes(&a.conn, true, "senha-da-loja", "").unwrap();
        drop(a);
        let cb = temp("mudas-b");
        let b = Banco::abrir(&cb).unwrap();
        salvar_ajustes(&b.conn, true, "senha-da-loja", "").unwrap();
        let _m = Motor::iniciar(ca.clone(), 0, 0);
        let (pt, _) = _m.portas(Duration::from_secs(10)).expect("as portas do motor abriram");
        // Um endereço só abre 10 conexões mudas: só 2 ficam abertas.
        let mut de_um: Vec<TcpStream> = (0..10).filter_map(|_| conectar_de([127, 0, 0, 66], pt).ok()).collect();
        thread::sleep(Duration::from_millis(200));
        let abertas = de_um.iter_mut().map(|c| !fechada(c)).filter(|aberta| *aberta).count();
        assert_eq!(abertas, VAGAS_POR_ORIGEM, "um endereço só ocupa {VAGAS_POR_ORIGEM} vagas");
        // Quem fica mudo cai no prazo.
        thread::sleep(PRAZO_DO_PEDIDO + Duration::from_millis(600));
        assert!(de_um.iter_mut().all(fechada), "as mudas caíram no prazo");
        // Muitos endereços mudos ocupam todas as vagas e continuam chegando.
        let parar = Arc::new(AtomicBool::new(false));
        let parar_ataque = parar.clone();
        let ataque = thread::spawn(move || {
            let mut abertas: Vec<TcpStream> = Vec::new();
            let mut n = 0u32;
            while !parar_ataque.load(Ordering::SeqCst) {
                let ip = [127, 0, 1 + (n % 20) as u8, 10 + (n / 20 % 200) as u8];
                if let Ok(c) = conectar_de(ip, pt) {
                    abertas.push(c);
                }
                n += 1;
                if abertas.len() > 200 {
                    abertas.drain(..100);
                }
                thread::sleep(Duration::from_millis(15));
            }
            n
        });
        thread::sleep(Duration::from_millis(800));
        // O caixa de verdade troca dados mesmo assim (várias vezes).
        let t = Instant::now();
        for _ in 0..3 {
            puxar(&cb, &format!("127.0.0.1:{pt}"), 1).unwrap();
        }
        let tempo = t.elapsed();
        parar.store(true, Ordering::SeqCst);
        let tentativas = ataque.join().unwrap();
        assert!(produtos::ficha(&b.conn, &p).is_ok(), "o caixa de verdade recebeu os dados");
        assert!(tempo < Duration::from_secs(15), "{tempo:?}");
        assert!(tentativas > 50, "o ataque mandou {tentativas} conexões");
        println!("conexões mudas durante o teste: {tentativas}; 3 trocas de dados do caixa de verdade em {tempo:?}");
    }

    /// R-06: fechar no meio de uma puxada travada (o outro caixa aceita a
    /// conexão e não responde nada) ou com erro de rede (ninguém escutando):
    /// encerra logo, sem esperar os 30 s de prazo da rede.
    #[test]
    fn encerrar_com_rede_lenta_ou_com_erro() {
        // "Caixa" que aceita a conexão e fica mudo. O-5 (auditoria da 0.9.15):
        // a espera pela conexão tem prazo (antes, `incoming().take(1)` podia
        // esperar para sempre se o motor nunca chegasse a ele), e o teste
        // espera a conexão acontecer de fato, em vez de um tempo fixo.
        let mudo = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        mudo.set_nonblocking(true).unwrap();
        let porta_muda = mudo.local_addr().unwrap().port();
        let conectou = Arc::new(AtomicBool::new(false));
        let terminou = Arc::new(AtomicBool::new(false));
        let segura = {
            let (conectou, terminou) = (conectou.clone(), terminou.clone());
            thread::spawn(move || {
                let fim = Instant::now() + Duration::from_secs(60);
                let mut conexoes = Vec::new();
                while conexoes.is_empty() && Instant::now() < fim {
                    match mudo.accept() {
                        Ok((f, _)) => conexoes.push(f),
                        Err(_) => thread::sleep(Duration::from_millis(20)),
                    }
                }
                conectou.store(!conexoes.is_empty(), Ordering::SeqCst);
                // Fica mudo até o teste acabar (no máximo 30 s).
                let fim = Instant::now() + Duration::from_secs(30);
                while !terminou.load(Ordering::SeqCst) && Instant::now() < fim {
                    thread::sleep(Duration::from_millis(20));
                }
                drop(conexoes);
            })
        };
        // Porta sem ninguém (erro de rede).
        let porta_fechada = TcpListener::bind(("127.0.0.1", 0)).unwrap().local_addr().unwrap().port();
        let c = temp("rede-lenta");
        let b = Banco::abrir(&c).unwrap();
        salvar_ajustes(&b.conn, true, "senha-da-loja", &format!("127.0.0.1:{porta_fechada}, 127.0.0.1:{porta_muda}"))
            .unwrap();
        drop(b);
        let mut m = Motor::iniciar(c.clone(), 0, 0);
        // A puxada roda a cada 4 s: espera ela ficar presa no caixa mudo (com prazo).
        let fim = Instant::now() + Duration::from_secs(60);
        while !conectou.load(Ordering::SeqCst) && Instant::now() < fim {
            thread::sleep(Duration::from_millis(50));
        }
        let chegou = conectou.load(Ordering::SeqCst);
        thread::sleep(Duration::from_millis(300));
        let t = Instant::now();
        let parou = m.encerrar(Duration::from_secs(20));
        let levou = t.elapsed();
        terminou.store(true, Ordering::SeqCst);
        segura.join().unwrap();
        assert!(chegou, "o motor não chegou ao caixa mudo em 60 s (o teste não provou nada)");
        assert!(parou, "o motor não parou no prazo");
        assert!(levou < Duration::from_secs(3), "parou logo: {levou:?}");
    }

    /// R-06: ao encerrar, o motor espera a conexão que está sendo atendida
    /// (mesmo lenta) antes de devolver; com prazo curto demais, avisa que
    /// ainda tem tarefa rodando (e o serviço não solta a trava).
    #[test]
    fn encerrar_espera_as_tarefas() {
        let c = temp("encerrar");
        let b = Banco::abrir(&c).unwrap();
        salvar_ajustes(&b.conn, true, "senha-da-loja", "").unwrap();
        drop(b);
        // Sem nenhuma conexão: encerra logo. (Portas escolhidas pelo sistema, C8.)
        let mut m = Motor::iniciar(c.clone(), 0, 0);
        m.portas(Duration::from_secs(10)).expect("as portas do motor abriram");
        let t = Instant::now();
        assert!(m.encerrar(Duration::from_secs(10)));
        assert!(t.elapsed() < Duration::from_secs(3), "{:?}", t.elapsed());
        drop(m);
        thread::sleep(Duration::from_millis(300));
        // Com um caixa lento conectado (não manda nada): o atendimento para
        // logo ao pedir para encerrar (não espera os 5 s de prazo dele), e só
        // então o motor devolve. Nenhuma tarefa fica para trás.
        let mut m = Motor::iniciar(c.clone(), 0, 0);
        let (pt, _) = m.portas(Duration::from_secs(10)).expect("as portas do motor abriram");
        let lento = TcpStream::connect(("127.0.0.1", pt)).unwrap();
        thread::sleep(Duration::from_millis(500));
        let t = Instant::now();
        assert!(m.encerrar(Duration::from_secs(20)));
        assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
        assert!(m.tarefas.is_empty());
        // Com uma tarefa que não termina: prazo curto devolve "não deu",
        // e a tarefa continua guardada (quem chamou segura a trava do banco).
        // O anúncio do próprio motor também pode passar de 300 ms (espera
        // pacote até 1 s); quando as do motor terminam, só a travada fica.
        let mut m = Motor::iniciar(c.clone(), 0, 0);
        let (solta, espera) = std::sync::mpsc::channel::<()>();
        let travada = thread::spawn(move || {
            let _ = espera.recv();
        });
        let id_travada = travada.thread().id();
        m.esperar_tambem(travada);
        assert!(!m.encerrar(Duration::from_millis(300)));
        assert!(m.tarefas.iter().any(|t| t.thread().id() == id_travada));
        let fim = Instant::now() + Duration::from_secs(10);
        while m.tarefas.iter().filter(|t| !t.is_finished()).count() > 1 && Instant::now() < fim {
            thread::sleep(Duration::from_millis(50));
        }
        assert!(!m.encerrar(Duration::from_millis(300)));
        let vivas: Vec<_> = m.tarefas.iter().map(|t| t.thread().name().map(String::from)).collect();
        assert_eq!(m.tarefas.len(), 1, "tarefas que ficaram: {vivas:?}");
        assert_eq!(m.tarefas[0].thread().id(), id_travada);
        solta.send(()).unwrap();
        assert!(m.encerrar(Duration::from_secs(5)));
        drop(lento);
    }

    /// R-05: as três situações de referência quebrada, cada uma com o
    /// registro, o que falta, o estado e a ação; a que espera resolve
    /// sozinha quando o registro chega (fora de ordem, repetido); a espera
    /// longa vira inconsistente. Nada é apagado.
    #[test]
    fn referencias_esperando_apagadas_e_inconsistentes() {
        // Caixa A cria uma peça com aplicação; B recebe a aplicação antes da peça.
        let a = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "1234", true).unwrap();
        let mut f = produtos::ficha_nova(&a.conn).unwrap();
        f.nome = "Correia".into();
        f.custo = "10".into();
        f.aplicacoes =
            vec![produtos::AplicacaoFicha { montadora: "Fiat".into(), modelo: "Uno".into(), ..Default::default() }];
        let pid = produtos::salvar(&a.conn, &a.dispositivo, &u.id, &f).unwrap();
        let (tudo, _) = alteracoes_desde(&a.conn, 0, 100_000).unwrap();
        let sem_a_peca: Vec<_> = tudo.iter().filter(|x| x.tabela != "produto").cloned().collect();
        let so_a_peca: Vec<_> = tudo.iter().filter(|x| x.tabela == "produto").cloned().collect();
        let mut b = Banco::em_memoria().unwrap();
        assert_eq!(registrar_referencias(&b.conn).unwrap(), Referencias::default());
        aplicar(&mut b.conn, &sem_a_peca).unwrap();
        aplicar(&mut b.conn, &sem_a_peca).unwrap(); // repetido
        let r = registrar_referencias(&b.conn).unwrap();
        assert!(r.esperando >= 1 && r.quebradas == 0, "{r:?}");
        let lista = referencias(&b.conn).unwrap();
        let ap = lista.iter().find(|x| x.tabela == "produto_aplicacao").expect("a aplicação aparece");
        assert_eq!((ap.estado.as_str(), ap.pai.as_str()), ("esperando", "produto"));
        assert!(ap.falta.contains(&pid) && ap.acao.contains("resolve sozinho"));
        // Pai apagado (o log diz) e inconsistente (o log diz que existe, a tabela não tem).
        let sem_gatilho = |sql: &str| {
            b.conn.execute("UPDATE sync_estado SET aplicando = 1 WHERE id = 1", []).unwrap();
            b.conn.pragma_update(None, "foreign_keys", false).unwrap();
            b.conn.execute(sql, []).unwrap();
            b.conn.pragma_update(None, "foreign_keys", true).unwrap();
            b.conn.execute("UPDATE sync_estado SET aplicando = 0 WHERE id = 1", []).unwrap();
        };
        sem_gatilho("INSERT INTO produto_aplicacao (id, produto_id) VALUES ('ap2', 'peca-apagada')");
        sem_gatilho("INSERT INTO produto_aplicacao (id, produto_id) VALUES ('ap3', 'peca-fantasma')");
        for (id, peca, op) in [
            ("01a00000-0000-7000-8000-000000000001", "peca-apagada", "apagar"),
            ("01a00000-0000-7000-8000-000000000002", "peca-fantasma", "gravar"),
        ] {
            b.conn
                .execute(
                    "INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando, ordem)
                     VALUES (?1, 'produto', json_array(?2), ?3, NULL, 'outro', '2026-01-01', 2)",
                    params![id, peca, op],
                )
                .unwrap();
        }
        registrar_referencias(&b.conn).unwrap();
        let estado = |b: &Banco, chave: &str| {
            referencias(&b.conn).unwrap().into_iter().find(|x| x.chave.contains(chave)).map(|x| x.estado)
        };
        assert_eq!(estado(&b, "ap2").as_deref(), Some("apagado"));
        assert_eq!(estado(&b, "ap3").as_deref(), Some("inconsistente"));
        assert!(referencias(&b.conn)
            .unwrap()
            .iter()
            .find(|x| x.chave.contains("ap2"))
            .unwrap()
            .acao
            .contains("Nada foi apagado"));
        // Esperando há muito tempo, sem prova nenhuma: continua ESPERANDO (R-05
        // revisado). O tempo sozinho não torna nada inconsistente; só ganha o
        // marcador "atrasada".
        sem_gatilho("INSERT INTO produto_aplicacao (id, produto_id) VALUES ('ap4', 'peca-que-nunca-vem')");
        registrar_referencias(&b.conn).unwrap();
        b.conn
            .execute("UPDATE sync_referencia SET desde = '2020-01-01T00:00:00Z' WHERE chave LIKE '%ap4%'", [])
            .unwrap();
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado(&b, "ap4").as_deref(), Some("esperando"));
        assert!(referencias(&b.conn).unwrap().iter().find(|x| x.chave.contains("ap4")).unwrap().atrasada);
        assert_eq!(config::ler(&b.conn, "sync_quebradas").unwrap().as_deref(), Some("2"));
        assert_eq!(config::ler(&b.conn, "sync_atrasadas").unwrap().as_deref(), Some("1"));
        // A peça chega depois (e chega de novo): a espera some sozinha.
        aplicar(&mut b.conn, &so_a_peca).unwrap();
        aplicar(&mut b.conn, &so_a_peca).unwrap();
        let r = registrar_referencias(&b.conn).unwrap();
        assert_eq!(r, Referencias { esperando: 1, quebradas: 2, atrasadas: 1 });
        assert!(referencias(&b.conn).unwrap().iter().all(|x| !x.falta.contains(&pid)), "resolvida sozinha");
        assert_eq!(produtos::ficha(&b.conn, &pid).unwrap().aplicacoes.len(), 1);
        // Nada foi apagado para esconder.
        let n: i64 = b.conn.query_row("SELECT COUNT(*) FROM produto_aplicacao", [], |l| l.get(0)).unwrap();
        assert_eq!(n, 4);
        let total: i64 = b.conn.query_row("SELECT COUNT(*) FROM sync_referencia", [], |l| l.get(0)).unwrap();
        assert_eq!(total, 3, "uma linha por referência, sem repetir");
        // A faixa do Início avisa.
        assert!(crate::avisos::do_dia(&b.conn).unwrap().iter().any(|a| a.id == "referencias"));
    }

    /// Item 3 da revisão: a espera de uma referência guarda a data do
    /// começo enquanto for o mesmo registro faltando; recomeça quando passa
    /// a faltar outro; some quando o registro que falta chega.
    #[test]
    fn espera_recomeca_quando_o_que_falta_muda() {
        // Outro caixa tem as peças X e Y; este ainda não recebeu nenhuma.
        let a = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "1234", true).unwrap();
        let mut ids = Vec::new();
        for n in ["Peça X", "Peça Y"] {
            let mut f = produtos::ficha_nova(&a.conn).unwrap();
            f.nome = n.into();
            f.custo = "1".into();
            ids.push(produtos::salvar(&a.conn, &a.dispositivo, &u.id, &f).unwrap());
        }
        let (x, y) = (ids[0].clone(), ids[1].clone());
        let (tudo, _) = alteracoes_desde(&a.conn, 0, 100_000).unwrap();
        let da_y: Vec<_> = tudo
            .iter()
            .filter(|v| v.chave.contains(&y) || v.dados.as_deref().is_some_and(|d| d.contains(&y)))
            .cloned()
            .collect();
        let mut b = Banco::em_memoria().unwrap();
        let sem_gatilho = |b: &Banco, sql: &str| {
            b.conn.execute("UPDATE sync_estado SET aplicando = 1 WHERE id = 1", []).unwrap();
            b.conn.pragma_update(None, "foreign_keys", false).unwrap();
            b.conn.execute(sql, []).unwrap();
            b.conn.pragma_update(None, "foreign_keys", true).unwrap();
            b.conn.execute("UPDATE sync_estado SET aplicando = 0 WHERE id = 1", []).unwrap();
        };
        let ler = |b: &Banco| -> (String, String, String) {
            b.conn
                .query_row("SELECT falta, estado, desde FROM sync_referencia WHERE chave LIKE '%ap1%'", [], |l| {
                    Ok((l.get(0)?, l.get(1)?, l.get(2)?))
                })
                .unwrap()
        };
        sem_gatilho(&b, &format!("INSERT INTO produto_aplicacao (id, produto_id) VALUES ('ap1', '{x}')"));
        registrar_referencias(&b.conn).unwrap();
        // Esperando há quase 24 horas.
        b.conn
            .execute("UPDATE sync_referencia SET desde = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-23 hours')", [])
            .unwrap();
        let (_, _, quase) = ler(&b);
        // Conferir de novo (outra troca de dados): a data do começo não muda.
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(ler(&b), (format!(r#"["{x}"]"#), "esperando".into(), quase.clone()));
        // Passa a apontar para a outra peça, que também não chegou: conta do zero.
        sem_gatilho(&b, &format!("UPDATE produto_aplicacao SET produto_id = '{y}' WHERE id = 'ap1'"));
        registrar_referencias(&b.conn).unwrap();
        let (falta, estado, desde) = ler(&b);
        assert_eq!((falta, estado.as_str()), (format!(r#"["{y}"]"#), "esperando"));
        assert!(desde > quase, "a nova referência começou a contar agora: {desde} > {quase}");
        // Uma hora depois ainda é "esperando" (a anterior já passaria de 24 h).
        b.conn
            .execute("UPDATE sync_referencia SET desde = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-1 hours')", [])
            .unwrap();
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(ler(&b).1, "esperando");
        // A peça Y chega pela troca de dados: a pendência some.
        aplicar(&mut b.conn, &da_y).unwrap();
        registrar_referencias(&b.conn).unwrap();
        let n: i64 = b.conn.query_row("SELECT COUNT(*) FROM sync_referencia", [], |l| l.get(0)).unwrap();
        assert_eq!(n, 0, "{:?}", referencias(&b.conn).unwrap());
    }

    // ------------------------------------------------------------------
    // R-05 (revisado): atraso não é inconsistência; só a prova bloqueia.
    // ------------------------------------------------------------------

    /// Grava direto, sem gatilho nem chave estrangeira (simula o que a troca
    /// de dados deixa para trás).
    fn sem_gatilho(b: &Banco, sql: &str) {
        b.conn.execute("UPDATE sync_estado SET aplicando = 1 WHERE id = 1", []).unwrap();
        b.conn.pragma_update(None, "foreign_keys", false).unwrap();
        b.conn.execute(sql, []).unwrap();
        b.conn.pragma_update(None, "foreign_keys", true).unwrap();
        b.conn.execute("UPDATE sync_estado SET aplicando = 0 WHERE id = 1", []).unwrap();
    }

    fn estado_de(b: &Banco, id: &str) -> (String, String) {
        b.conn
            .query_row("SELECT estado, motivo FROM sync_referencia WHERE chave = json_array(?1)", [id], |l| {
                Ok((l.get(0)?, l.get(1)?))
            })
            .unwrap()
    }

    fn banco_com_aplicacao_sem_peca(id: &str) -> Banco {
        let b = Banco::em_memoria().unwrap();
        sem_gatilho(
            &b,
            &format!("INSERT INTO produto_aplicacao (id, produto_id) VALUES ('{id}', 'peca-ausente-{id}')"),
        );
        registrar_referencias(&b.conn).unwrap();
        b
    }

    fn envelhecer(b: &Banco, id: &str) {
        b.conn
            .execute("UPDATE sync_referencia SET desde = '2020-01-01T00:00:00Z' WHERE chave = json_array(?1)", [id])
            .unwrap();
    }

    /// R5-T12: chave composta. Se QUALQUER elemento da chave de um registro
    /// inconsistente é o que a operação está mexendo, ela é recusada (e só ela).
    #[test]
    fn r5_t12_chave_composta_bloqueia_por_qualquer_elemento() {
        let b = Banco::em_memoria().unwrap();
        b.conn
            .execute(
                "INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo)
                 VALUES ('conta_receber', '[\"c0\",\"c1\"]', 'pessoa', '[\"x\"]', 'inconsistente', '2020-01-01T00:00:00Z', 'log')",
                [],
            )
            .unwrap();
        assert!(exigir_editavel(&b.conn, "conta_receber", "c0").is_err(), "primeiro elemento");
        assert!(exigir_editavel(&b.conn, "conta_receber", "c1").is_err(), "segundo elemento");
        assert!(exigir_editavel(&b.conn, "conta_receber", "c9").is_ok(), "outro registro");
        assert!(exigir_editavel(&b.conn, "recebimento", "c1").is_ok(), "outra tabela");
    }

    fn par_com_troca(b: &Banco, dispositivo: &str, completo_em: Option<&str>) {
        b.conn
            .execute(
                "INSERT OR REPLACE INTO sync_par (dispositivo, nome, endereco, completo_em) VALUES (?1, ?1, '', ?2)",
                params![dispositivo, completo_em],
            )
            .unwrap();
    }

    /// R5-T1 · R5-1, R5-2, R5-4: 30 dias esperando e nenhum caixa conhecido: continua
    /// esperando (atrasada), não bloqueia nada.
    #[test]
    fn r5_t1_tempo_sozinho_nunca_e_prova() {
        let b = banco_com_aplicacao_sem_peca("ap1");
        envelhecer(&b, "ap1");
        let r = registrar_referencias(&b.conn).unwrap();
        assert_eq!(r, Referencias { esperando: 1, quebradas: 0, atrasadas: 1 });
        assert_eq!(estado_de(&b, "ap1").0, "esperando");
        let l = referencias(&b.conn).unwrap();
        assert!(l[0].atrasada && !l[0].pode_aceitar && l[0].acao.contains("não bloqueia"), "{:?}", l[0]);
        exigir_editavel(&b.conn, "produto_aplicacao", "ap1").unwrap();
        // Recente: esperando, mas ainda não atrasada.
        let n = banco_com_aplicacao_sem_peca("ap9");
        assert!(!referencias(&n.conn).unwrap()[0].atrasada);
        assert_eq!(config::ler(&n.conn, "sync_atrasadas").unwrap().as_deref(), Some("0"));
    }

    /// R5-T2 · R5-3, R5-7: o log diz que existe e a tabela não tem: inconsistente (log).
    #[test]
    fn r5_t2_prova_do_log_bloqueia() {
        let b = banco_com_aplicacao_sem_peca("ap3");
        b.conn
            .execute(
                "INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando, ordem)
                 VALUES ('01a00000-0000-7000-8000-0000000000aa', 'produto', json_array('peca-ausente-ap3'), 'gravar', NULL, 'outro', '2026-01-01', 2)",
                [],
            )
            .unwrap();
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado_de(&b, "ap3"), ("inconsistente".into(), "log".into()));
        let e = exigir_editavel(&b.conn, "produto_aplicacao", "ap3").unwrap_err().to_string();
        assert!(e.contains("inconsistente") && e.contains("gerente"), "{e}");
        assert!(referencias(&b.conn).unwrap()[0].pode_aceitar);
        // Outro registro da mesma tabela passa.
        exigir_editavel(&b.conn, "produto_aplicacao", "ap-outro").unwrap();
    }

    /// R5-T3 · R5-4, R5-5: a prova dos pares exige TODOS os caixas conhecidos com
    /// troca completa DEPOIS de a referência aparecer; um par novo desfaz a prova.
    #[test]
    fn r5_t3_prova_dos_pares_exige_todos() {
        let b = banco_com_aplicacao_sem_peca("ap1");
        let depois = "2999-01-01T00:00:00Z";
        // Nenhum par conhecido: sem prova.
        assert_eq!(estado_de(&b, "ap1").0, "esperando");
        // Dois pares, nenhum com troca completa: sem prova.
        par_com_troca(&b, "p1", None);
        par_com_troca(&b, "p2", None);
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado_de(&b, "ap1").0, "esperando");
        // Só um completou depois: sem prova.
        par_com_troca(&b, "p1", Some(depois));
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado_de(&b, "ap1").0, "esperando");
        // O outro completou, mas ANTES de a referência aparecer: sem prova.
        par_com_troca(&b, "p2", Some("2000-01-01T00:00:00Z"));
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado_de(&b, "ap1").0, "esperando");
        exigir_editavel(&b.conn, "produto_aplicacao", "ap1").unwrap();
        // Os dois completaram depois: comprovado pelos pares.
        par_com_troca(&b, "p2", Some(depois));
        let r = registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado_de(&b, "ap1"), ("inconsistente".into(), "pares".into()));
        assert_eq!(r.quebradas, 1);
        let e = exigir_editavel(&b.conn, "produto_aplicacao", "ap1").unwrap_err().to_string();
        assert!(e.contains("todos os outros caixas"), "{e}");
        // Aparece um caixa novo, que ainda não trocou nada: a prova se desfaz
        // e o registro volta a funcionar, sem ninguém aceitar nada.
        par_com_troca(&b, "p3", None);
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado_de(&b, "ap1"), ("esperando".into(), "".into()));
        exigir_editavel(&b.conn, "produto_aplicacao", "ap1").unwrap();
    }

    /// R5-T4/T5 · R5-6, R5-7: o pai chega e o bloqueio some sozinho; apagado não bloqueia.
    #[test]
    fn r5_t4_pai_que_chega_desbloqueia_e_apagado_nao_bloqueia() {
        // Caixa A cria a peça com uma aplicação; B recebe a aplicação antes da peça.
        let a = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "1234", true).unwrap();
        let mut f = produtos::ficha_nova(&a.conn).unwrap();
        f.nome = "Correia".into();
        f.custo = "10".into();
        f.aplicacoes =
            vec![produtos::AplicacaoFicha { montadora: "Fiat".into(), modelo: "Uno".into(), ..Default::default() }];
        produtos::salvar(&a.conn, &a.dispositivo, &u.id, &f).unwrap();
        let (tudo, _) = alteracoes_desde(&a.conn, 0, 100_000).unwrap();
        let sem_a_peca: Vec<_> = tudo.iter().filter(|x| x.tabela != "produto").cloned().collect();
        let so_a_peca: Vec<_> = tudo.iter().filter(|x| x.tabela == "produto").cloned().collect();
        let mut b = Banco::em_memoria().unwrap();
        aplicar(&mut b.conn, &sem_a_peca).unwrap();
        registrar_referencias(&b.conn).unwrap();
        let ap: String = b.conn.query_row("SELECT id FROM produto_aplicacao", [], |l| l.get(0)).unwrap();
        par_com_troca(&b, "p1", Some("2999-01-01T00:00:00Z"));
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado_de(&b, &ap).0, "inconsistente");
        exigir_editavel(&b.conn, "produto_aplicacao", &ap).unwrap_err();
        // O pai chega: a linha e o bloqueio somem sozinhos.
        aplicar(&mut b.conn, &so_a_peca).unwrap();
        registrar_referencias(&b.conn).unwrap();
        let n: i64 = b.conn.query_row("SELECT COUNT(*) FROM sync_referencia", [], |l| l.get(0)).unwrap();
        assert_eq!(n, 0, "{:?}", referencias(&b.conn).unwrap());
        exigir_editavel(&b.conn, "produto_aplicacao", &ap).unwrap();
        // Apagado: o log diz "apagar"; a correção (editar) tem de passar.
        sem_gatilho(&b, "INSERT INTO produto_aplicacao (id, produto_id) VALUES ('ap2', 'peca-apagada')");
        b.conn
            .execute(
                "INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando, ordem)
                 VALUES ('01a00000-0000-7000-8000-0000000000bb', 'produto', json_array('peca-apagada'), 'apagar', NULL, 'outro', '2026-01-01', 2)",
                [],
            )
            .unwrap();
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado_de(&b, "ap2").0, "apagado");
        exigir_editavel(&b.conn, "produto_aplicacao", "ap2").unwrap();
    }

    /// R5-T8 · R5-9: aceite do gerente (motivo obrigatório, só inconsistente, mesma falta).
    #[test]
    fn r5_t8_aceite_do_gerente() {
        let b = banco_com_aplicacao_sem_peca("ap1");
        let chave = r#"["ap1"]"#;
        // Esperando não pode ser aceito (não há prova de nada).
        let e = aceitar_inconsistencia(&b.conn, "u1", "produto_aplicacao", chave, "motivo bom").unwrap_err();
        assert!(e.to_string().contains("não está inconsistente"), "{e}");
        par_com_troca(&b, "p1", Some("2999-01-01T00:00:00Z"));
        registrar_referencias(&b.conn).unwrap();
        exigir_editavel(&b.conn, "produto_aplicacao", "ap1").unwrap_err();
        // Sem motivo: recusa.
        for m in ["", "  ", "abc"] {
            let e = aceitar_inconsistencia(&b.conn, "u1", "produto_aplicacao", chave, m).unwrap_err();
            assert_eq!(e.nome_do_campo(), Some("motivo"), "{m:?}");
        }
        exigir_editavel(&b.conn, "produto_aplicacao", "ap1").unwrap_err();
        // Com motivo: libera, guarda quem/quando/por quê.
        usuarios::criar(&b.conn, &b.dispositivo, "Ana", "1234", true).unwrap();
        let ana: String = b.conn.query_row("SELECT id FROM usuario WHERE nome = 'Ana'", [], |l| l.get(0)).unwrap();
        assert_eq!(
            aceitar_inconsistencia(&b.conn, &ana, "produto_aplicacao", chave, "Caixa antigo foi perdido").unwrap(),
            1
        );
        exigir_editavel(&b.conn, "produto_aplicacao", "ap1").unwrap();
        let l = referencias(&b.conn).unwrap();
        assert_eq!(l[0].estado, "aceito");
        assert_eq!((l[0].aceita_por.as_str(), l[0].aceite_motivo.as_str()), ("Ana", "Caixa antigo foi perdido"));
        assert!(!l[0].pode_aceitar && l[0].acao.contains("aceitou"));
        // Conferir de novo não desfaz o aceite (mesma falta).
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado_de(&b, "ap1").0, "aceito");
        exigir_editavel(&b.conn, "produto_aplicacao", "ap1").unwrap();
        // Passa a faltar OUTRA coisa: o aceite não vale, é avaliado de novo.
        sem_gatilho(&b, "UPDATE produto_aplicacao SET produto_id = 'outra-peca' WHERE id = 'ap1'");
        registrar_referencias(&b.conn).unwrap();
        assert_ne!(estado_de(&b, "ap1").0, "aceito");
        let aceita: Option<String> = b
            .conn
            .query_row("SELECT aceita_por FROM sync_referencia WHERE chave = json_array('ap1')", [], |l| l.get(0))
            .unwrap();
        assert!(aceita.is_none());
    }

    /// R5-T10 · R5-11: a troca completa grava `completo_em`; o recomeço do zero apaga.
    #[test]
    fn r5_t10_troca_completa_marca_o_par() {
        let c1 = temp("r5-c1");
        let b1 = Banco::abrir(&c1).unwrap();
        let u = usuarios::criar(&b1.conn, &b1.dispositivo, "Maria", "1234", true).unwrap();
        let mut f = produtos::ficha_nova(&b1.conn).unwrap();
        f.nome = "Pastilha".into();
        f.custo = "40".into();
        produtos::salvar(&b1.conn, &b1.dispositivo, &u.id, &f).unwrap();
        salvar_ajustes(&b1.conn, true, "senha-da-loja", "").unwrap();
        let c2 = temp("r5-c2");
        let b2 = Banco::abrir(&c2).unwrap();
        salvar_ajustes(&b2.conn, true, "senha-da-loja", "").unwrap();
        let _m1 = Motor::iniciar(c1.clone(), 0, 0);
        let (pt1, _) = _m1.portas(Duration::from_secs(10)).expect("as portas do motor abriram");
        let completo = |b: &Banco| -> Option<String> {
            b.conn
                .query_row("SELECT completo_em FROM sync_par WHERE dispositivo = ?1", [&b1.dispositivo], |l| l.get(0))
                .unwrap()
        };
        puxar(&c2, &format!("127.0.0.1:{pt1}"), 48310).unwrap();
        assert!(completo(&b2).is_some(), "depois de receber tudo, a troca está completa");
        // Restaurar um backup recomeça do zero: a prova some até nova troca completa.
        depois_de_restaurar(&b2.conn, 0).unwrap();
        assert!(completo(&b2).is_none());
        puxar(&c2, &format!("127.0.0.1:{pt1}"), 48310).unwrap();
        assert!(completo(&b2).is_some());
    }

    // ------------------------------------------------------------------
    // R-08: conflitos sem depender do relógio.
    // ------------------------------------------------------------------

    /// Leva de `de` para `para` tudo o que `de` tem (em lotes, como pela rede).
    fn levar(de: &Banco, para: &mut Banco) -> usize {
        let (mut desde, mut total) = (0, 0);
        loop {
            let (lista, mais) = alteracoes_desde(&de.conn, desde, 40).unwrap();
            if let Some(u) = lista.last() {
                desde = u.seq;
            }
            total += aplicar(&mut para.conn, &lista).unwrap();
            if !mais {
                return total;
            }
        }
    }

    fn trocar(a: &mut Banco, b: &mut Banco) {
        for _ in 0..6 {
            if levar(a, b) + levar(b, a) == 0 {
                return;
            }
        }
        panic!("os caixas não pararam de trocar");
    }

    /// Faz `f` com o relógio deste caixa adiantado/atrasado `ms`.
    fn com_relogio<T>(ms: i64, f: impl FnOnce() -> T) -> T {
        tempo::RELOGIO_DE_TESTE.with(|c| c.set(ms));
        let r = f();
        tempo::RELOGIO_DE_TESTE.with(|c| c.set(0));
        r
    }

    const MIN: i64 = 60_000;
    const HORA: i64 = 60 * MIN;
    const DIA: i64 = 24 * HORA;

    /// Loja com dois caixas e uma peça igual nos dois.
    fn dois_caixas() -> (Banco, Banco, String, String) {
        let mut a = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "1234", true).unwrap().id;
        config::gravar(&a.conn, "caixa_nome", "Caixa 1").unwrap();
        let mut f = produtos::ficha_nova(&a.conn).unwrap();
        f.nome = "Pastilha".into();
        f.custo = "40".into();
        let p = produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
        let mut b = Banco::em_memoria().unwrap();
        config::gravar(&b.conn, "caixa_nome", "Caixa 2").unwrap();
        trocar(&mut a, &mut b);
        (a, b, u, p)
    }

    fn renomear(b: &Banco, u: &str, p: &str, nome: &str) {
        let mut f = produtos::ficha(&b.conn, p).unwrap();
        f.nome = nome.into();
        produtos::salvar(&b.conn, &b.dispositivo, u, &f).unwrap();
    }

    fn nome(b: &Banco, p: &str) -> String {
        produtos::ficha(&b.conn, p).unwrap().nome
    }

    /// (versão que perdeu, nome guardado nela) dos conflitos abertos.
    fn abertos(b: &Banco) -> Vec<(String, String, String)> {
        let mut c = b
            .conn
            .prepare(
                "SELECT perdeu, COALESCE(json_extract(dados_perdeu, '$.nome'), tabela), certeza FROM sync_conflito
                  WHERE resolvido_em IS NULL AND tipo = 'registro' ORDER BY perdeu",
            )
            .unwrap();
        let l = c.query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?))).unwrap();
        l.collect::<Result<_, _>>().unwrap()
    }

    /// Relógios diferentes em minutos, 25 horas e vários dias (para frente e
    /// para trás): duas alterações feitas sem uma ver a outra são sempre
    /// conflito, nos dois caixas; a que perdeu fica guardada; os dois ficam
    /// iguais; o aviso do Início aparece; a lista diz quais caixas.
    #[test]
    fn conflito_com_qualquer_diferenca_de_relogio() {
        for atraso in [10 * MIN, -10 * MIN, 25 * HORA, -25 * HORA, 5 * DIA, -5 * DIA] {
            let (mut a, mut b, u, p) = dois_caixas();
            renomear(&a, &u, &p, "Pastilha dianteira");
            com_relogio(atraso, || renomear(&b, &u, &p, "Pastilha traseira"));
            trocar(&mut a, &mut b);
            assert_eq!(nome(&a, &p), nome(&b, &p), "atraso {atraso}: os dois iguais");
            for x in [&a, &b] {
                let k = abertos(x);
                assert_eq!(k.len(), 1, "atraso {atraso}: conflito anotado em cada caixa: {k:?}");
                let perdeu =
                    if nome(x, &p) == "Pastilha dianteira" { "Pastilha traseira" } else { "Pastilha dianteira" };
                assert_eq!(k[0].1, perdeu, "atraso {atraso}: a versão que perdeu está guardada");
                assert_eq!(k[0].2, "concorrente");
                let lista = conflitos(&x.conn).unwrap();
                assert!(lista[0].texto.contains("está guardada"), "{}", lista[0].texto);
                assert!(!lista[0].caixa.is_empty() && !lista[0].caixa_ficou.is_empty());
                assert!(crate::avisos::do_dia(&x.conn).unwrap().iter().any(|a| a.id == "conflitos"));
            }
            assert_eq!(abertos(&a)[0].0, abertos(&b)[0].0, "o mesmo conflito nos dois");
        }
    }

    /// Uma alteração feita DEPOIS de ver a outra vale, mesmo com o relógio
    /// do caixa dias atrasado (antes, perdia para a hora mais nova). Não é
    /// conflito.
    #[test]
    fn feita_depois_vale_mesmo_com_relogio_atrasado() {
        for atraso in [-10 * MIN, -25 * HORA, -5 * DIA] {
            let (mut a, mut b, u, p) = dois_caixas();
            renomear(&a, &u, &p, "Pastilha 1");
            trocar(&mut a, &mut b);
            com_relogio(atraso, || renomear(&b, &u, &p, "Pastilha 2"));
            trocar(&mut a, &mut b);
            assert_eq!((nome(&a, &p), nome(&b, &p)), ("Pastilha 2".into(), "Pastilha 2".into()), "atraso {atraso}");
            assert!(abertos(&a).is_empty() && abertos(&b).is_empty(), "atraso {atraso}");
        }
    }

    /// Três caixas, cada um mexe sem ver os outros, e as alterações chegam
    /// num quarto caixa em todas as ordens possíveis (com repetição): o
    /// resultado e os conflitos são sempre os mesmos.
    #[test]
    fn qualquer_ordem_de_chegada_termina_igual() {
        let (mut a, mut b, u, p) = dois_caixas();
        let mut c = Banco::em_memoria().unwrap();
        trocar(&mut a, &mut c);
        let todas = |x: &Banco, desde: i64| alteracoes_desde(&x.conn, desde, 100_000).unwrap().0;
        let ultimo = |x: &Banco| todas(x, 0).last().map(|v| v.seq).unwrap_or(0);
        let base = todas(&a, 0);
        let (sa, sb, sc) = (ultimo(&a), ultimo(&b), ultimo(&c));
        renomear(&a, &u, &p, "Versão A");
        com_relogio(-25 * HORA, || renomear(&b, &u, &p, "Versão B"));
        com_relogio(3 * DIA, || {
            renomear(&c, &u, &p, "Versão C1");
            renomear(&c, &u, &p, "Versão C2");
        });
        let (la, lb, lc) = (todas(&a, sa), todas(&b, sb), todas(&c, sc));
        let pacotes = [la, lb.clone(), lc, lb];
        let ordens: [[usize; 4]; 6] =
            [[0, 1, 2, 3], [2, 1, 0, 3], [1, 2, 3, 0], [3, 0, 2, 1], [2, 3, 1, 0], [1, 0, 3, 2]];
        let mut resultados = Vec::new();
        for o in ordens {
            let mut d = Banco::em_memoria().unwrap();
            aplicar(&mut d.conn, &base).unwrap();
            for i in o {
                aplicar(&mut d.conn, &pacotes[i]).unwrap();
            }
            resultados.push((nome(&d, &p), abertos(&d)));
        }
        for r in &resultados {
            assert_eq!(r, &resultados[0], "ordens diferentes, mesmo resultado");
        }
        // C mexeu duas vezes (ordem maior): vale C2. A e B ficam guardadas;
        // C1 não é conflito (C2 foi feita em cima dela).
        assert_eq!(resultados[0].0, "Versão C2");
        let guardadas: Vec<&str> = resultados[0].1.iter().map(|k| k.1.as_str()).collect();
        assert_eq!(guardadas.len(), 2, "{guardadas:?}");
        assert!(guardadas.contains(&"Versão A") && guardadas.contains(&"Versão B"));
        // E os três caixas de verdade, trocando entre si, ficam iguais.
        trocar(&mut a, &mut b);
        trocar(&mut b, &mut c);
        trocar(&mut a, &mut c);
        for x in [&a, &b, &c] {
            assert_eq!(nome(x, &p), "Versão C2");
            assert_eq!(abertos(x).len(), 2);
        }
    }

    /// Reenvio: o mesmo lote chega de novo (inteiro, ao contrário, depois de
    /// compactar o log): nada é aplicado de novo e o aviso não repete.
    #[test]
    fn reenvio_nao_repete() {
        let (mut a, mut b, u, p) = dois_caixas();
        renomear(&a, &u, &p, "A");
        renomear(&b, &u, &p, "B");
        let (lista, _) = alteracoes_desde(&a.conn, 0, 100_000).unwrap();
        trocar(&mut a, &mut b);
        assert_eq!(abertos(&b).len(), 1);
        let invertida: Vec<_> = lista.iter().rev().cloned().collect();
        assert_eq!(aplicar(&mut b.conn, &lista).unwrap(), 0);
        assert_eq!(aplicar(&mut b.conn, &invertida).unwrap(), 0);
        // Depois de resolver e compactar (a versão que perdeu sai do log), o
        // reenvio continua reconhecido pelo id.
        let id = conflitos(&b.conn).unwrap()[0].id;
        resolver_conflito(&b.conn, id, false, &u).unwrap();
        compactar(&b.conn).unwrap();
        assert_eq!(aplicar(&mut b.conn, &lista).unwrap(), 0);
        assert_eq!(conflitos_abertos(&b.conn).unwrap(), 0);
        let total: i64 = b.conn.query_row("SELECT COUNT(*) FROM sync_conflito", [], |l| l.get(0)).unwrap();
        assert_eq!(total, 1, "um só registro do conflito");
    }

    /// A decisão do gerente vai para os outros caixas: lá o mesmo conflito
    /// fecha sozinho e a versão que perdeu não volta a valer. Um caixa que
    /// recebe a versão que perdeu depois da decisão já a vê resolvida.
    #[test]
    fn decisao_do_gerente_vai_para_os_outros() {
        let (mut a, mut b, u, p) = dois_caixas();
        renomear(&a, &u, &p, "Da A");
        com_relogio(2 * DIA, || renomear(&b, &u, &p, "Da B"));
        trocar(&mut a, &mut b);
        let vale = nome(&a, &p);
        let k = conflitos(&a.conn).unwrap();
        resolver_conflito(&a.conn, k[0].id, false, &u).unwrap();
        trocar(&mut a, &mut b);
        assert_eq!(conflitos_abertos(&b.conn).unwrap(), 0, "fechou no outro caixa");
        assert!(conflitos(&b.conn).unwrap()[0].resolvido.contains("ficou como está"));
        // Um terceiro caixa chega depois: recebe tudo, inclusive a versão que
        // perdeu, e já vê o conflito resolvido.
        let mut c = Banco::em_memoria().unwrap();
        trocar(&mut b, &mut c);
        assert_eq!(nome(&c, &p), vale);
        assert_eq!(conflitos_abertos(&c.conn).unwrap(), 0);
        // Agora o gerente usa a que perdeu, num conflito novo.
        renomear(&a, &u, &p, "Nova da A");
        renomear(&c, &u, &p, "Nova da C");
        trocar(&mut a, &mut c);
        trocar(&mut a, &mut b);
        let k = conflitos(&b.conn).unwrap().into_iter().find(|k| k.resolvido.is_empty()).expect("conflito novo");
        let guardada = k.campos.iter().find(|c| c["campo"] == "nome").unwrap()["perdeu"].as_str().unwrap().to_string();
        resolver_conflito(&b.conn, k.id, true, &u).unwrap();
        trocar(&mut a, &mut b);
        trocar(&mut b, &mut c);
        trocar(&mut a, &mut c);
        for x in [&a, &b, &c] {
            assert_eq!(nome(x, &p), guardada, "a escolhida vale em todos");
            assert_eq!(conflitos_abertos(&x.conn).unwrap(), 0, "e nenhum caixa pergunta de novo");
        }
    }

    /// Item 2 da revisão: dois gerentes decidem diferente sobre o mesmo
    /// conflito, com os caixas sem rede (A: manter; B: usar a que perdeu, com
    /// o relógio de B dias atrasado). As decisões se cruzam nas duas ordens
    /// e um terceiro caixa recebe tudo depois. Todos terminam com os mesmos
    /// dados (vale "usar"), a mesma resolução escrita (que diz que houve
    /// decisões diferentes e qual valeu), as duas decisões guardadas, o
    /// mesmo evento da divergência e a versão que perdeu ainda guardada.
    #[test]
    fn dois_gerentes_decidem_diferente() {
        for ordem_a_primeiro in [true, false] {
            let (mut a, mut b, u, p) = dois_caixas();
            let bia = usuarios::criar(&a.conn, &a.dispositivo, "Bia", "5678", true).unwrap().id;
            let mut c = Banco::em_memoria().unwrap();
            trocar(&mut a, &mut b);
            trocar(&mut a, &mut c);
            renomear(&a, &u, &p, "Nome da A");
            renomear(&b, &u, &p, "Nome da B");
            trocar(&mut a, &mut b);
            let vale = nome(&a, &p);
            let perdeu = if vale == "Nome da A" { "Nome da B" } else { "Nome da A" };
            // Sem rede: cada gerente decide no seu caixa.
            let ka = conflitos(&a.conn).unwrap()[0].id;
            let kb = conflitos(&b.conn).unwrap()[0].id;
            resolver_conflito(&a.conn, ka, false, &u).unwrap();
            com_relogio(-5 * DIA, || resolver_conflito(&b.conn, kb, true, &bia).unwrap());
            assert_eq!(nome(&b, &p), perdeu);
            // As decisões se cruzam (nas duas ordens) e chegam a um terceiro.
            if ordem_a_primeiro {
                levar(&a, &mut b);
                levar(&b, &mut a);
            } else {
                levar(&b, &mut a);
                levar(&a, &mut b);
            }
            trocar(&mut a, &mut b);
            trocar(&mut b, &mut c);
            trocar(&mut a, &mut c);
            let mut textos = Vec::new();
            for (quem, x) in [("A", &a), ("B", &b), ("C", &c)] {
                assert_eq!(nome(x, &p), perdeu, "{quem}: vale usar a que perdeu");
                assert_eq!(conflitos_abertos(&x.conn).unwrap(), 0, "{quem}");
                let k = conflitos(&x.conn).unwrap();
                let r = k.iter().find(|k| k.tipo == "registro").expect("o conflito continua na lista");
                assert!(r.resolvido.contains("decisões diferentes"), "{quem}: {}", r.resolvido);
                assert!(
                    r.resolvido.contains("Ana manteve") && r.resolvido.contains("Bia usou"),
                    "{quem}: {}",
                    r.resolvido
                );
                assert!(r.resolvido.contains("Vale agora: a versão que tinha perdido"), "{quem}: {}", r.resolvido);
                textos.push(r.resolvido.clone());
                let decisoes: Vec<(String, String)> = x
                    .conn
                    .prepare("SELECT id, escolha FROM sync_resolucao ORDER BY id")
                    .unwrap()
                    .query_map([], |l| Ok((l.get(0)?, l.get(1)?)))
                    .unwrap()
                    .collect::<Result<_, _>>()
                    .unwrap();
                assert_eq!(decisoes.len(), 3, "{quem}: as duas decisões e o evento da divergência: {decisoes:?}");
                assert_eq!(decisoes.iter().filter(|d| d.1 == "aplicada").count(), 1, "{quem}");
                // A versão que estava valendo (a de "manter") continua no log de
                // alterações, com os dados, nos três caixas.
                let guardada: i64 = x
                    .conn
                    .query_row(
                        "SELECT COUNT(*) FROM alteracao WHERE tabela = 'produto' AND json_extract(dados, '$.nome') = ?1",
                        [&vale],
                        |l| l.get(0),
                    )
                    .unwrap();
                assert!(guardada >= 1, "{quem}: a versão de quem manteve não some");
            }
            assert!(textos.windows(2).all(|w| w[0] == w[1]), "a mesma resolução nos três: {textos:?}");
            let evento = |x: &Banco| -> String {
                x.conn.query_row("SELECT id FROM sync_resolucao WHERE escolha = 'aplicada'", [], |l| l.get(0)).unwrap()
            };
            assert!(evento(&a) == evento(&b) && evento(&b) == evento(&c), "o mesmo evento nos três");
        }
    }

    /// Backup restaurado num caixa, com o outro caixa tendo mexido muitas
    /// vezes no mesmo registro e compactado o log: as versões velhas do
    /// restaurado não viram aviso (a cadeia de versões mostra que vieram
    /// antes), ele fica igual ao outro e recebe de volta o que perdeu,
    /// inclusive o que ele mesmo tinha feito depois do backup.
    #[test]
    fn backup_restaurado() {
        let pasta = std::env::temp_dir().join(format!("rustinho-r08-{}", tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        for mudancas in [5, HISTORIA_MAXIMA + 8, 3 * HISTORIA_MAXIMA] {
            let (mut a, mut b, u, p) = dois_caixas();
            let arquivo = pasta.join(format!("a-{mudancas}.db"));
            a.conn.backup(rusqlite::MAIN_DB, &arquivo, None).unwrap();
            // Depois do backup: A cria uma peça (e manda para B); B mexe muitas vezes.
            let mut f = produtos::ficha_nova(&a.conn).unwrap();
            f.nome = "Feita pela A depois do backup".into();
            f.custo = "1".into();
            let nova = produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
            trocar(&mut a, &mut b);
            for i in 0..mudancas {
                renomear(&b, &u, &p, &format!("B {i}"));
            }
            compactar(&b.conn).unwrap();
            // A volta do backup e troca de novo.
            let mut a = Banco::abrir(&arquivo).unwrap();
            assert_eq!(nome(&a, &p), "Pastilha");
            assert!(produtos::ficha(&a.conn, &nova).is_err(), "a peça nova se perdeu no backup");
            trocar(&mut a, &mut b);
            let ultima = format!("B {}", mudancas - 1);
            assert_eq!((nome(&a, &p), nome(&b, &p)), (ultima.clone(), ultima.clone()), "{mudancas}");
            assert!(
                abertos(&a).is_empty() && abertos(&b).is_empty(),
                "{mudancas}: {:?} {:?}",
                abertos(&a),
                abertos(&b)
            );
            assert_eq!(nome(&a, &nova), "Feita pela A depois do backup", "voltou dos outros caixas");
        }
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// Quando o caminho de versões está incompleto (versão de antes do
    /// protocolo 3 com base que este caixa não tem), não dá para provar a
    /// ordem: conflito "incerto", com as duas versões guardadas, sem olhar
    /// o relógio. "Deixar todas como estão" fecha de uma vez e a decisão vai
    /// para o outro caixa.
    #[test]
    fn ordem_incerta_vira_conflito() {
        let (mut a, mut b, u, p) = dois_caixas();
        let (chave, dados): (String, String) = a
            .conn
            .query_row("SELECT chave, dados FROM alteracao WHERE tabela = 'produto' AND compactada = 0", [], |l| {
                Ok((l.get(0)?, l.get(1)?))
            })
            .unwrap();
        let estranha = Alteracao {
            seq: 1,
            id: tempo::novo_id(),
            tabela: "produto".into(),
            chave,
            operacao: "gravar".into(),
            dados: Some(dados.replace("Pastilha", "Pastilha de um caixa antigo")),
            dispositivo: "caixa-antigo".into(),
            quando: tempo::agora(),
            base: Some("01000000-0000-7000-8000-000000000000".into()),
            ordem: 0,
            historia: None,
            compactada: false,
            operacao_id: None,
            incompativel: None,
        };
        aplicar(&mut a.conn, std::slice::from_ref(&estranha)).unwrap();
        let k = abertos(&a);
        assert_eq!(k.len(), 1, "{k:?}");
        assert_eq!(k[0].2, "incerta");
        let lista = conflitos(&a.conn).unwrap();
        assert!(lista[0].texto.contains("Não deu para saber"), "{}", lista[0].texto);
        // As duas versões guardadas: a que vale está na peça, a outra no conflito.
        assert_eq!(nome(&a, &p), "Pastilha", "ordem 0 perde para a versão de ordem 1");
        assert_eq!(k[0].1, "Pastilha de um caixa antigo");
        trocar(&mut a, &mut b);
        assert_eq!(abertos(&b).len(), 1, "o outro caixa também avisa");
        assert_eq!(manter_todos(&a.conn, &u).unwrap(), 1);
        trocar(&mut a, &mut b);
        assert_eq!(conflitos_abertos(&a.conn).unwrap(), 0);
        assert_eq!(conflitos_abertos(&b.conn).unwrap(), 0, "a decisão chegou ao outro caixa");
    }

    /// Item 4 da revisão: volume. Um caixa com muitas alterações (peças
    /// criadas e mexidas, uma peça mexida milhares de vezes, vendas), o log
    /// compactado, um caixa novo recebendo tudo e um caixa que ficou muito
    /// tempo sem rede (e mexeu na peça mais mexida, sem ver as outras)
    /// voltando. Mede tamanho e tempos; confere que tudo fica igual e que o
    /// conflito é achado. Roda à parte (demora):
    /// `cargo test -p rustinho-nucleo --release -- --ignored volume --nocapture`
    #[test]
    #[ignore]
    fn volume_do_historico() {
        let (pecas, edicoes, quente, vendas_n) = (2000usize, 10usize, 3000usize, 500usize);
        let pasta = std::env::temp_dir().join(format!("rustinho-volume-{}", tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let abrir = |n: &str| Banco::abrir(&pasta.join(n)).unwrap();
        let tamanho = |b: &Banco, n: &str| -> f64 {
            b.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
            std::fs::metadata(pasta.join(n)).unwrap().len() as f64 / 1_048_576.0
        };
        let linhas = |b: &Banco| -> (i64, i64) {
            b.conn
                .query_row("SELECT COUNT(*), COALESCE(SUM(compactada), 0) FROM alteracao", [], |l| {
                    Ok((l.get(0)?, l.get(1)?))
                })
                .unwrap()
        };
        let mut a = abrir("a.db");
        let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "1234", true).unwrap().id;
        usuarios::definir_gerente(&a.conn, &u, true).unwrap();
        let mut f = produtos::ficha_nova(&a.conn).unwrap();
        f.nome = "Peça mais mexida".into();
        f.custo = "10".into();
        f.estoque_inicial = "100000".into();
        let quente_id = produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
        // B recebe o começo e fica sem rede.
        let mut b = abrir("b.db");
        levar(&a, &mut b);
        let t = Instant::now();
        let mut ids = Vec::new();
        a.conn.execute_batch("BEGIN").unwrap();
        for i in 0..pecas {
            let mut f = produtos::ficha_nova(&a.conn).unwrap();
            f.nome = format!("Peça {i}");
            f.custo = "10".into();
            f.estoque_inicial = "50".into();
            ids.push(produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap());
        }
        for e in 0..edicoes {
            for id in &ids {
                let mut f = produtos::ficha(&a.conn, id).unwrap();
                f.observacoes = format!("edição {e}");
                produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
            }
        }
        for e in 0..quente {
            let mut f = produtos::ficha(&a.conn, &quente_id).unwrap();
            f.observacoes = format!("quente {e}");
            produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
        }
        a.conn.execute_batch("COMMIT").unwrap();
        caixa::abrir(&a.conn, &a.dispositivo, &u, "Caixa A", "0").unwrap();
        for i in 0..vendas_n {
            let e = vendas::VendaEntrada {
                itens: vec![vendas::ItemEntrada {
                    produto_id: ids[i % ids.len()].clone(),
                    quantidade: "1".into(),
                    ..Default::default()
                }],
                tabela: 1,
                pagamentos: vec![vendas::PagamentoEntrada {
                    forma: "dinheiro".into(),
                    valor: "100".into(),
                    ..Default::default()
                }],
                ..Default::default()
            };
            vendas::concluir(&a.conn, &a.dispositivo, &u, &e).unwrap();
        }
        let gerar = t.elapsed();
        let (antes_linhas, _) = linhas(&a);
        let antes_mb = tamanho(&a, "a.db");
        let t = Instant::now();
        let compactadas = compactar(&a.conn).unwrap();
        let tempo_compactar = t.elapsed();
        a.conn.execute_batch("VACUUM").unwrap();
        let (depois_linhas, sem_dados) = linhas(&a);
        let depois_mb = tamanho(&a, "a.db");
        // B, sem rede, mexe na peça mais mexida (sem ver as 3000 de A).
        let mut f = produtos::ficha(&b.conn, &quente_id).unwrap();
        f.nome = "Mexida pelo caixa sem rede".into();
        produtos::salvar(&b.conn, &b.dispositivo, &u, &f).unwrap();
        // Caixa novo recebe tudo.
        let mut c = abrir("c.db");
        let t = Instant::now();
        let recebidas = levar(&a, &mut c);
        let tempo_novo = t.elapsed();
        let c_mb = tamanho(&c, "c.db");
        // B volta (recebe só o que falta, a partir do seu contador; aqui, tudo
        // pelo id: o que ele já tem é pulado) e acha o conflito.
        let t = Instant::now();
        levar(&a, &mut b);
        let tempo_atrasado = t.elapsed();
        let t = Instant::now();
        levar(&b, &mut a);
        let tempo_volta = t.elapsed();
        let conflitos_a = conflitos_abertos(&a.conn).unwrap();
        // Tudo igual.
        for x in [&b, &c] {
            let n: i64 = x.conn.query_row("SELECT COUNT(*) FROM produto", [], |l| l.get(0)).unwrap();
            assert_eq!(n as usize, pecas + 1);
            let v: i64 = x.conn.query_row("SELECT COUNT(*) FROM venda WHERE tipo = 'venda'", [], |l| l.get(0)).unwrap();
            assert_eq!(v as usize, vendas_n);
            assert_eq!(produtos::ficha(&x.conn, &ids[0]).unwrap().observacoes, format!("edição {}", edicoes - 1));
        }
        assert_eq!(produtos::ficha(&c.conn, &quente_id).unwrap().observacoes, format!("quente {}", quente - 1));
        assert!(conflitos_abertos(&b.conn).unwrap() >= 1, "o caixa que voltou acha o conflito");
        assert!(conflitos_a >= 1, "e o principal também");
        println!("\n=== Volume do histórico ===");
        println!("peças: {pecas}, edições por peça: {edicoes}, peça mais mexida: {quente} vezes, vendas: {vendas_n}");
        println!("gerar tudo no caixa A: {:.1} s", gerar.as_secs_f64());
        println!("log antes de compactar: {antes_linhas} linhas, banco {antes_mb:.1} MB");
        println!("compactar: {compactadas} versões ficaram sem dados em {:.2} s", tempo_compactar.as_secs_f64());
        println!("log depois (VACUUM): {depois_linhas} linhas ({sem_dados} só esqueleto), banco {depois_mb:.1} MB");
        let (esqueleto, com_dados): (f64, f64) = a
            .conn
            .query_row(
                "SELECT AVG(CASE WHEN compactada = 1 THEN length(id) + length(tabela) + length(chave) + length(operacao)
                                 + length(dispositivo) + length(quando) + COALESCE(length(base), 0) + 8 END),
                        AVG(CASE WHEN compactada = 0 THEN length(id) + length(tabela) + length(chave) + length(operacao)
                                 + length(dispositivo) + length(quando) + COALESCE(length(base), 0) + 8
                                 + COALESCE(length(dados), 0) + COALESCE(length(historia), 0) END)
                   FROM alteracao",
                [],
                |l| Ok((l.get::<_, Option<f64>>(0)?.unwrap_or(0.0), l.get::<_, Option<f64>>(1)?.unwrap_or(0.0))),
            )
            .unwrap();
        println!("conteúdo médio: versão só esqueleto {esqueleto:.0} bytes; versão com dados {com_dados:.0} bytes (sem contar índices)");
        println!(
            "caixa novo recebe tudo: {recebidas} alterações em {:.1} s, banco {c_mb:.1} MB",
            tempo_novo.as_secs_f64()
        );
        println!(
            "caixa que ficou sem rede volta: {:.1} s (e manda a dele: {:.2} s)",
            tempo_atrasado.as_secs_f64(),
            tempo_volta.as_secs_f64()
        );
        println!("conflitos achados: B {}, A {conflitos_a}", conflitos_abertos(&b.conn).unwrap());
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// História: cresce até o limite e marca quando não chega ao começo.
    #[test]
    fn historia_tem_tamanho_limitado() {
        assert_eq!(historia_nova(None, None, None), "[]");
        assert_eq!(historia_nova(Some("a"), Some("[]"), None), r#"["a"]"#);
        // Base de antes do protocolo 3 (sem história, mas com base): incompleta.
        assert_eq!(historia_nova(Some("b"), None, Some("a")), r#"["b","a","+"]"#);
        let mut h = "[]".to_string();
        let mut base: Option<String> = None;
        for i in 0..100 {
            let id = format!("v{i}");
            h = historia_nova(Some(&id), Some(&h), base.as_deref());
            base = Some(id);
        }
        let lista: Vec<String> = serde_json::from_str(&h).unwrap();
        assert_eq!(lista.len(), HISTORIA_MAXIMA + 1);
        assert_eq!(lista.last().unwrap(), INCOMPLETA);
        assert_eq!(lista[0], "v99");
    }

    #[test]
    fn mensagens_do_protocolo_2() {
        let b = Banco::em_memoria().unwrap();
        let k1 = Chaves::de(&b.conn, "senha-da-loja").unwrap();
        // Outro caixa: mesmo senha, outro sal.
        let k2 = Chaves {
            senha: "senha-da-loja".into(),
            sal: [9u8; 16],
            propria: derivar("senha-da-loja", &[9u8; 16]).unwrap(),
        };
        assert_ne!(k1.propria, k2.propria);
        let m = cifrar(&k2, b"ola").unwrap();
        assert_eq!(decifrar(&k1, &m, None).unwrap(), b"ola");
        // Senha errada.
        let k3 = Chaves {
            senha: "outra-senha".into(),
            sal: [9u8; 16],
            propria: derivar("outra-senha", &[9u8; 16]).unwrap(),
        };
        assert!(decifrar(&k3, &m, None).unwrap_err().contains("senha"));
        // Versão antiga (sem "RST3"), inclusive a do protocolo 2: recusada
        // antes de ler qualquer dado.
        assert!(decifrar(&k1, &[0u8; 80], None).unwrap_err().contains("versão"));
        let mut antiga = cifrar(&k2, b"ola").unwrap();
        antiga[..4].copy_from_slice(b"RST2");
        assert!(decifrar(&k1, &antiga, None).unwrap_err().contains("versão"));
        // O sal deste caixa fica guardado.
        assert_eq!(Chaves::de(&b.conn, "senha-da-loja").unwrap().sal, k1.sal);
    }

    #[test]
    fn compactar_e_conflito() {
        let mut b = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        config::gravar(&b.conn, "sync_inicio", "x").unwrap();
        for nome in ["Maria A", "Maria B", "Maria C"] {
            usuarios::alterar(&b.conn, &u.id, nome, true, None).unwrap();
        }
        let antes: i64 =
            b.conn.query_row("SELECT COUNT(*) FROM alteracao WHERE tabela = 'usuario'", [], |l| l.get(0)).unwrap();
        assert!(antes >= 4);
        assert!(compactar(&b.conn).unwrap() >= 3);
        // Fica uma versão com dados; as velhas ficam sem dados (só a cadeia).
        let (com_dados, sem_dados): (i64, i64) = b
            .conn
            .query_row(
                "SELECT SUM(compactada = 0), SUM(compactada = 1 AND dados IS NULL) FROM alteracao WHERE tabela = 'usuario'",
                [],
                |l| Ok((l.get(0)?, l.get(1)?)),
            )
            .unwrap();
        assert_eq!(com_dados, 1);
        assert_eq!(sem_dados, antes - 1);
        // Uma alteração de outro caixa, feita sem ver as nossas (sem base),
        // chega depois: não vale (ordem menor) e fica anotada, guardada.
        let (chave_u, dados): (String, String) = b
            .conn
            .query_row("SELECT chave, dados FROM alteracao WHERE tabela = 'usuario' AND compactada = 0", [], |l| {
                Ok((l.get(0)?, l.get(1)?))
            })
            .unwrap();
        let velha = Alteracao {
            seq: 1,
            id: "00000000-0000-7000-8000-000000000001".into(),
            tabela: "usuario".into(),
            chave: chave_u,
            operacao: "gravar".into(),
            dados: Some(dados.replace("Maria C", "Maria velha")),
            dispositivo: "outro".into(),
            quando: tempo::agora(),
            base: None,
            ordem: 1,
            historia: Some("[]".into()),
            compactada: false,
            operacao_id: None,
            incompativel: None,
        };
        aplicar(&mut b.conn, std::slice::from_ref(&velha)).unwrap();
        assert_eq!(usuarios::buscar(&b.conn, &u.id).unwrap().nome, "Maria C");
        let conflitos: i64 = b.conn.query_row("SELECT COUNT(*) FROM sync_conflito", [], |l| l.get(0)).unwrap();
        assert_eq!(conflitos, 1);
        // A mesma de novo: reconhecida pelo id, não repete.
        aplicar(&mut b.conn, &[velha]).unwrap();
        let conflitos: i64 = b.conn.query_row("SELECT COUNT(*) FROM sync_conflito", [], |l| l.get(0)).unwrap();
        assert_eq!(conflitos, 1);
        // Compactar não apaga a versão que perdeu enquanto o conflito está aberto.
        compactar(&b.conn).unwrap();
        let guardada: i64 = b
            .conn
            .query_row("SELECT COUNT(*) FROM alteracao WHERE id = '00000000-0000-7000-8000-000000000001'", [], |l| {
                l.get(0)
            })
            .unwrap();
        assert_eq!(guardada, 1);
        assert_eq!(pendencias(&b.conn).unwrap(), 0);
    }

    /// C-01: a vaga volta mesmo se a tarefa der pânico, e nunca passa do teto.
    #[test]
    fn vaga_volta_depois_de_panico() {
        let contador = Arc::new(AtomicUsize::new(0));
        let vagas: Vec<Vaga> = (0..CONEXOES_MAXIMAS).filter_map(|_| Vaga::pegar_de(&contador)).collect();
        assert_eq!(vagas.len(), CONEXOES_MAXIMAS);
        assert!(Vaga::pegar_de(&contador).is_none(), "não passa do teto");
        let mut vagas = vagas.into_iter();
        let uma = vagas.next().unwrap();
        let r = thread::spawn(move || {
            let _v = uma;
            panic!("defeito de teste");
        })
        .join();
        assert!(r.is_err());
        let outra = Vaga::pegar_de(&contador).expect("a vaga voltou depois do pânico");
        drop(outra);
        drop(vagas);
        assert_eq!(contador.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn senha_curta_recusada() {
        let b = Banco::em_memoria().unwrap();
        assert!(salvar_ajustes(&b.conn, true, "123", "").is_err());
        assert!(salvar_ajustes(&b.conn, true, "bola-de-gude", "192.168.0.300").is_err());
        assert!(salvar_ajustes(&b.conn, true, "", "").is_err(), "vazia");
        salvar_ajustes(&b.conn, true, "1234", "192.168.0.12").unwrap(); // só o tamanho conta (0.9.13)
        salvar_ajustes(&b.conn, true, "bola-de-gude", "192.168.0.12, 192.168.0.13").unwrap();
        assert_eq!(ajustes(&b.conn).unwrap().enderecos.len(), 2);
    }

    // ------------------------------------------------------------------
    // PEND-10: anúncio, portas e descoberta por várias origens.
    // ------------------------------------------------------------------

    /// T-22 · CA-I6: o pacote de anúncio continua `{dispositivo, nome, porta}`
    /// e o leitor aceita campos a mais (versões futuras); `PROTOCOLO` não muda.
    #[test]
    fn t22_pacote_de_anuncio_compativel() {
        let novo = anuncio("d1", "Caixa 1", 47810);
        let mut chaves: Vec<&str> = novo.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        chaves.sort();
        assert_eq!(chaves, ["dispositivo", "nome", "porta"], "o pacote enviado é o de antes");
        assert_eq!(ler_anuncio(&novo), Some(("d1".into(), "Caixa 1".into(), 47810)));
        let antigo = json!({ "dispositivo": "d2", "nome": "Caixa 2", "porta": 47900 });
        assert_eq!(ler_anuncio(&antigo), Some(("d2".into(), "Caixa 2".into(), 47900)));
        let futuro = json!({ "dispositivo": "d3", "nome": "Caixa 3", "porta": 47810, "interfaces": ["cabo"], "x": 1 });
        assert_eq!(ler_anuncio(&futuro), Some(("d3".into(), "Caixa 3".into(), 47810)));
        assert_eq!(ler_anuncio(&json!({ "dispositivo": "d4" })), Some(("d4".into(), String::new(), PORTA_TCP)));
        assert_eq!(ler_anuncio(&json!({ "nome": "sem id" })), None);
        assert_eq!(PROTOCOLO, 3, "o protocolo não mudou");
    }

    /// T-23 · CA-F2: porta ocupada deixa de ser silenciosa: o motor guarda o
    /// problema (e o registro recebe), sem pânico, e encerra normalmente.
    #[test]
    fn t23_porta_ocupada_nao_e_silenciosa() {
        let tcp = TcpListener::bind("0.0.0.0:0").unwrap();
        let udp = UdpSocket::bind("0.0.0.0:0").unwrap();
        let (pt, pu) = (tcp.local_addr().unwrap().port(), udp.local_addr().unwrap().port());
        let c = temp("ocupada");
        let b = Banco::abrir(&c).unwrap();
        salvar_ajustes(&b.conn, true, "balcao-azul-2026", "").unwrap();
        let mut m = Motor::iniciar(c, pt, pu);
        thread::sleep(Duration::from_millis(600));
        let problemas = m.problemas();
        assert_eq!(problemas.len(), 2, "{problemas:?}");
        assert!(problemas.iter().any(|p| p.contains("TCP") && p.contains(&pt.to_string())), "{problemas:?}");
        assert!(problemas.iter().any(|p| p.contains("UDP") && p.contains(&pu.to_string())), "{problemas:?}");
        assert!(m.encerrar(Duration::from_secs(10)));
        assert_eq!(
            texto_da_porta("TCP", 47810, "Address already in use"),
            texto_da_porta("TCP", 47810, "Address already in use")
        );
        assert!(texto_da_porta("TCP", 47810, "x").contains("47810"));
    }

    /// T-05 (rede) · CA-D2, CA-I1, CA-I5: o mesmo caixa anunciando de dois
    /// endereços (cabo e Wi-Fi) vira um par com dois caminhos; anúncio com a
    /// senha errada é ignorado em qualquer origem; o próprio caixa por outra
    /// interface não vira par.
    #[test]
    fn t05_descoberta_por_varias_origens() {
        let c = temp("descoberta");
        let b = Banco::abrir(&c).unwrap();
        salvar_ajustes(&b.conn, true, "balcao-azul-2026", "").unwrap();
        let mut m = Motor::iniciar(c.clone(), 0, 0);
        let (_, pu) = m.portas(Duration::from_secs(10)).expect("as portas do motor abriram");
        let sal = [9u8; 16];
        let boa = Chaves { senha: "balcao-azul-2026".into(), sal, propria: derivar("balcao-azul-2026", &sal).unwrap() };
        let errada =
            Chaves { senha: "outra-senha-2026".into(), sal, propria: derivar("outra-senha-2026", &sal).unwrap() };
        let enviar_de = |ip: [u8; 4], k: &Chaves, d: &str| {
            let s = UdpSocket::bind((std::net::Ipv4Addr::from(ip), 0)).unwrap();
            let pacote = cifrar(k, anuncio(d, "Caixa X", 47810).to_string().as_bytes()).unwrap();
            s.send_to(&pacote, ("127.0.0.1", pu)).unwrap();
        };
        enviar_de([127, 0, 0, 2], &boa, "dx"); // cabo
        enviar_de([127, 0, 0, 3], &boa, "dx"); // Wi-Fi
        enviar_de([127, 0, 0, 4], &errada, "dy"); // outra loja
        enviar_de([127, 0, 0, 5], &errada, "dx"); // outra loja dizendo ser o dx
        enviar_de([127, 0, 0, 6], &boa, &b.dispositivo); // ele mesmo, por outra interface
        thread::sleep(Duration::from_millis(3500));
        let pares = pares(&b.conn).unwrap();
        assert_eq!(pares.len(), 1, "{pares:?}");
        assert_eq!(pares[0].dispositivo, "dx");
        let mut enderecos: Vec<&str> = pares[0].caminhos.iter().map(|x| x.endereco.as_str()).collect();
        enderecos.sort();
        assert_eq!(enderecos, ["127.0.0.2:47810", "127.0.0.3:47810"], "dois caminhos do mesmo caixa");
        assert!(m.encerrar(Duration::from_secs(10)));
    }

    /// CA-A3, CA-I4 (na entrada do anúncio): o próprio caixa e o que vem de
    /// interface virtual sem liberação não viram caminho; liberada, vira.
    #[test]
    fn anuncio_do_proprio_caixa_ou_de_interface_virtual_e_ignorado() {
        let a = Ajustes { dispositivo: "eu".into(), ..Ajustes::default() };
        let lista = vec![
            Interface::de_teste("eno1", [192, 168, 0, 5], 24, rede_local::Tipo::Cabo),
            Interface::de_teste("tailscale0", [100, 100, 1, 1], 24, rede_local::Tipo::Virtual),
        ];
        assert!(aceitar_anuncio("outro", &a, Some(&lista), "192.168.0.9:47810"));
        assert!(!aceitar_anuncio("eu", &a, Some(&lista), "192.168.0.9:47810"), "ele mesmo");
        assert!(!aceitar_anuncio("outro", &a, Some(&lista), "100.100.1.9:47810"), "virtual sem liberação");
        let liberada = Ajustes { interfaces: vec!["tailscale0".into()], ..a };
        assert!(aceitar_anuncio("outro", &liberada, Some(&lista), "100.100.1.9:47810"));
        assert!(aceitar_anuncio("outro", &liberada, None, "100.100.1.9:47810"), "sem lista: como antes");
    }
}
