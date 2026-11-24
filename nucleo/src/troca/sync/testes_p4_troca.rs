//! T008 (0.9.19-dev.2, `roadmap.md` §5.6, DT-P4-29): a troca de dados pelo fio do
//! protocolo 4 entre caixas aprovados de verdade (raiz, aprovação assinada e
//! registro juntado nos dois bancos).
//!
//! A entrada pelo convite (etapa 5) é da T009; aqui o caixa 2 entra na loja como
//! hoje (protocolo 3) e é aprovado pelo gerente no caixa 1, como a A4 prevê para os
//! caixas que já trocavam dados.

use super::testes_seguranca_caixas::{caixa_vazio_em_arquivo, loja_em_arquivo, motor, temp};
use super::*;
use crate::banco::Banco;
use crate::troca::protocolo4::{aprovados, cofre};

/// A identidade (deste processo) do caixa cujo banco está em `caminho`.
fn identidade(caminho: &Path) -> Arc<cofre::IdentidadeLocal> {
    cofre::identidade_do_processo(caminho.parent().unwrap(), &cofre::Modo::Instalado).unwrap()
}

/// O caixa 1 (distribuidor, gerente "Ana") vira a raiz; o caixa 2 entra na loja pelo
/// caminho de hoje e é aprovado no caixa 1; os dois juntam o registro. Devolve os
/// bancos, a loja e as identidades.
#[allow(clippy::type_complexity)]
fn loja_com_dois_caixas_aprovados(
    nome: &str,
) -> (PathBuf, PathBuf, Banco, Banco, String, Arc<cofre::IdentidadeLocal>, Arc<cofre::IdentidadeLocal>) {
    let (ca, cb) = (temp(&format!("{nome}-a")), temp(&format!("{nome}-b")));
    let (a, _ua) = loja_em_arquivo(&ca, "Caixa 1");
    p4::garantir_raiz(&a.conn, &ca).unwrap();
    let loja = crate::loja::id(&a.conn).unwrap();
    let id_a = identidade(&ca);
    assert_eq!(cofre::raiz_atual(&a.conn, &loja).unwrap(), Some(id_a.identidade.publica()), "o caixa 1 é a raiz");

    // O caixa 2 entra na loja como hoje (protocolo 3).
    let b = caixa_vazio_em_arquivo(&cb, "Caixa 2");
    {
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b).unwrap();
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
    }
    assert_eq!(crate::loja::id(&b.conn).unwrap(), loja, "o caixa 2 recebeu a loja");

    // O gerente aprova o caixa 2 no caixa 1; o caixa 2 junta o registro e fixa a raiz.
    let id_b = identidade(&cb);
    let disp_b = config::ler(&b.conn, "dispositivo_id").unwrap().unwrap();
    let pedido = aprovados::Pedido::com_dispositivo(&id_b.identidade, &disp_b);
    aprovados::aprovar(&a.conn, &id_a.identidade, &id_a.identidade.publica(), &loja, &pedido, "Caixa 2", "Ana")
        .unwrap();
    aprovados::juntar(&b.conn, &loja, &aprovados::linhas(&a.conn, &loja).unwrap()).unwrap();
    cofre::garantir_raiz_se_aplicavel(&b.conn, &id_b.identidade, false, &loja, "Caixa 2", "", false).unwrap();
    assert_eq!(cofre::raiz_atual(&b.conn, &loja).unwrap(), Some(id_a.identidade.publica()), "o caixa 2 fixou a raiz");
    (ca, cb, a, b, loja, id_a, id_b)
}

/// Uma peça nova no caixa `b` (com o usuário que a loja tem).
fn peca_nova(b: &Banco, nome: &str) -> String {
    let u: String = b.conn.query_row("SELECT id FROM usuario LIMIT 1", [], |l| l.get(0)).unwrap();
    let mut f = crate::produtos::ficha_nova(&b.conn).unwrap();
    f.nome = nome.into();
    f.custo = "10".into();
    crate::produtos::salvar(&b.conn, &b.dispositivo, &u, &f).unwrap()
}

fn tem_peca(b: &Banco, nome: &str) -> bool {
    b.conn.query_row("SELECT COUNT(*) FROM produto WHERE nome = ?1", [nome], |l| l.get::<_, i64>(0)).unwrap() > 0
}

/// Dois caixas aprovados trocam dados pelo fio autenticado, nos dois sentidos.
#[test]
fn p4_dois_caixas_aprovados_trocam_pelo_fio_autenticado() {
    thread::spawn(|| {
        let (ca, cb, a, b, _loja, _ia, _ib) = loja_com_dois_caixas_aprovados("p4-troca");
        peca_nova(&a, "Peça do caixa 1 pelo P4");
        peca_nova(&b, "Peça do caixa 2 pelo P4");
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        let antes = p4::ATENDIDAS_PELO_P4.load(Ordering::SeqCst);
        puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b).unwrap();
        puxar(&ca, &format!("127.0.0.1:{pt_b}"), pt_a).unwrap();
        let pelo_p4 = p4::ATENDIDAS_PELO_P4.load(Ordering::SeqCst) - antes;
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        assert!(tem_peca(&b, "Peça do caixa 1 pelo P4"), "a peça do caixa 1 chegou ao 2");
        assert!(tem_peca(&a, "Peça do caixa 2 pelo P4"), "a peça do caixa 2 chegou ao 1");
        assert!(pelo_p4 >= 2, "as duas puxadas passaram pelo fio do protocolo 4 ({pelo_p4})");
    })
    .join()
    .unwrap();
}

/// Uma aprovação nova feita no caixa 1 chega ao caixa 2 pelo canal autenticado (o
/// registro não viaja pelo protocolo 3).
#[test]
fn p4_registro_novo_chega_pelo_canal() {
    thread::spawn(|| {
        let (ca, cb, a, b, loja, ia, _ib) = loja_com_dois_caixas_aprovados("p4-registro");
        let c3 = crate::troca::protocolo4::Identidade::gerar().unwrap();
        let pedido = aprovados::Pedido::com_dispositivo(&c3, "disp-caixa-3-p4");
        aprovados::aprovar(&a.conn, &ia.identidade, &ia.identidade.publica(), &loja, &pedido, "Caixa 3", "Ana")
            .unwrap();
        let raiz = ia.identidade.publica();
        assert!(!aprovados::autorizada(&b.conn, &raiz, &loja, &c3.publica()).unwrap());
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b).unwrap();
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        assert!(
            aprovados::autorizada(&b.conn, &raiz, &loja, &c3.publica()).unwrap(),
            "o caixa 3 chegou ao registro do 2"
        );
        let n: i64 = b
            .conn
            .query_row("SELECT COUNT(*) FROM alteracao WHERE tabela IN ('caixa_aprovado', 'caixa_retirado')", [], |l| {
                l.get(0)
            })
            .unwrap();
        assert_eq!(n, 0, "o registro não entra no log da troca do protocolo 3");
    })
    .join()
    .unwrap();
}

/// Caixa retirado deixa de trocar na conexão seguinte, e não cai para o protocolo 3.
#[test]
fn p4_caixa_retirado_deixa_de_trocar_e_nao_cai_para_o_p3() {
    thread::spawn(|| {
        let (ca, cb, a, b, loja, ia, ib) = loja_com_dois_caixas_aprovados("p4-retirado");
        aprovados::retirar(
            &a.conn,
            &ia.identidade,
            &ia.identidade.publica(),
            &loja,
            &ib.identidade.publica(),
            "saiu da loja",
            "Ana",
        )
        .unwrap();
        peca_nova(&a, "Peça depois da retirada");
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        let r = puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b);
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        assert!(r.is_err(), "o caixa retirado não troca mais: {r:?}");
        assert!(!tem_peca(&b, "Peça depois da retirada"), "nada chegou depois da retirada (nem pelo protocolo 3)");
    })
    .join()
    .unwrap();
}

/// O `dispositivo` declarado tem de ser o da aprovação da chave do canal (I3).
#[test]
fn p4_dispositivo_declarado_diferente_da_chave_e_recusado() {
    thread::spawn(|| {
        let (ca, cb, a, b, _loja, _ia, _ib) = loja_com_dois_caixas_aprovados("p4-dispositivo");
        peca_nova(&a, "Peça que não pode ir");
        // O caixa 2 passa a declarar o identificador de outro caixa.
        config::gravar(&b.conn, "dispositivo_id", "outro-caixa-qualquer-0001").unwrap();
        let (mut ma, pt_a) = motor(&ca);
        let (mut mb, pt_b) = motor(&cb);
        let r = puxar(&cb, &format!("127.0.0.1:{pt_a}"), pt_b);
        assert!(ma.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        assert!(r.is_err(), "o pedido com outro dispositivo é recusado: {r:?}");
        assert!(!tem_peca(&b, "Peça que não pode ir"));
    })
    .join()
    .unwrap();
}

/// DT-P4-14/28 (anti-rebaixamento): para o caixa que o registro diz ter chave, a
/// troca é só pelo P4 — nem quando quem responde no endereço dele só fala o
/// protocolo 3 (o que alguém faria para se passar por ele com a senha da rede).
#[test]
fn p4_par_com_chave_nao_cai_para_o_p3() {
    thread::spawn(|| {
        let (ca, cb, a, b, _loja, _ia, _ib) = loja_com_dois_caixas_aprovados("p4-rebaixar");
        // Um terceiro caixa da mesma loja, que entrou pela senha da rede (só fala o P3).
        let cc = temp("p4-rebaixar-c");
        let c = caixa_vazio_em_arquivo(&cc, "Caixa 3");
        {
            let (mut ma, pt_a) = motor(&ca);
            let (mut mc, pt_c) = motor(&cc);
            puxar(&cc, &format!("127.0.0.1:{pt_a}"), pt_c).unwrap();
            assert!(ma.encerrar(Duration::from_secs(20)));
            assert!(mc.encerrar(Duration::from_secs(20)));
        }
        peca_nova(&c, "Peça de quem só fala o P3");
        let (mut mc, pt_c) = motor(&cc);
        let (mut mb, pt_b) = motor(&cb);
        // No caixa 2, o endereço do caixa 1 passa a ser o do caixa 3 (quem responde ali).
        let disp_a = config::ler(&a.conn, "dispositivo_id").unwrap().unwrap();
        b.conn
            .execute(
                "UPDATE sync_par SET endereco = ?2 WHERE dispositivo = ?1",
                params![disp_a, format!("127.0.0.1:{pt_c}")],
            )
            .unwrap();
        let r = puxar(&cb, &format!("127.0.0.1:{pt_c}"), pt_b);
        assert!(mc.encerrar(Duration::from_secs(20)));
        assert!(mb.encerrar(Duration::from_secs(20)));
        assert!(r.is_err(), "o caixa com chave não pode ser trocado por quem só fala o P3: {r:?}");
        assert!(!tem_peca(&b, "Peça de quem só fala o P3"), "nada chegou pelo protocolo 3");
    })
    .join()
    .unwrap();
}
