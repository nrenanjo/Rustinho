//! RC2-2 (revisão cruzada 2), a outra metade da D-I2 (integração 2): a marca
//! da 5.2 (`sync_inicio:produto_gtin`) acompanha o banco restaurado. O
//! `Pendente::aplicar` não apaga a marca do banco que entrou, então um backup
//! JÁ da 5.2 restaurado não registra os GTINs de novo na abertura seguinte.
//! Sem esta guarda, a mutação M5 da revisão (o `aplicar` volta a apagar a
//! marca) sobrevivia a todos os testes: os GTINs viravam alteração outra vez e
//! iam de novo para os outros caixas. É a sonda S-DI2b do revisor, trazida
//! como teste.
//!
//! Num arquivo de teste só dele: o backup tem uma vez só no programa inteiro
//! (`backup::Vez`), e dois testes de restaurar no mesmo processo, em paralelo,
//! se atrapalham ("Tem um backup em andamento").

use rustinho_nucleo::banco::Banco;
use rustinho_nucleo::{config, produtos, usuarios};

const GTIN_1: &str = "7891000000090";
const GTIN_2: &str = "7891000000113";

fn contar(b: &Banco, sql: &str) -> i64 {
    b.conn.query_row(sql, [], |l| l.get(0)).unwrap()
}

/// Um GTIN gravado como num banco de antes da 5.2: sem passar pela troca.
fn gtin_sem_troca(x: &Banco, produto: &str, gtin: &str) {
    x.conn.execute("UPDATE sync_estado SET aplicando = 1 WHERE id = 1", []).unwrap();
    x.conn
        .execute(
            "INSERT INTO produto_gtin (id, produto_id, gtin, tipo, origem, ordem, criado_em)
             VALUES (?1, ?2, ?3, 'comercial', 'cEAN', 5, '2026-11-01T00:00:00Z')",
            [rustinho_nucleo::tempo::novo_id(), produto.to_string(), gtin.to_string()],
        )
        .unwrap();
    x.conn.execute("UPDATE sync_estado SET aplicando = 0 WHERE id = 1", []).unwrap();
}

#[test]
fn rc2_2_backup_da_5_2_restaurado_nao_registra_os_gtins_de_novo() {
    let pasta = std::env::temp_dir().join(format!("rustinho-rc2-2-{}", rustinho_nucleo::tempo::novo_id()));
    std::fs::create_dir_all(&pasta).unwrap();
    let db = pasta.join("a.db");
    let senha = "balcao-azul-2026";
    let a = Banco::abrir(&db).unwrap();
    let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", senha, true).unwrap();
    usuarios::definir_gerente(&a.conn, &u.id, true).unwrap();
    config::gravar(&a.conn, "loja_nome", "Loja de Teste").unwrap();
    let mut f = produtos::ficha_nova(&a.conn).unwrap();
    f.nome = "Filtro de ar".into();
    f.custo = "40".into();
    let p = produtos::salvar(&a.conn, &a.dispositivo, &u.id, &f).unwrap();
    gtin_sem_troca(&a, &p, GTIN_1);
    gtin_sem_troca(&a, &p, GTIN_2);
    a.conn.execute("DELETE FROM config WHERE chave = 'sync_inicio:produto_gtin'", []).unwrap();
    drop(a);
    // Atualiza para a 5.2 (registra os 2 GTINs e põe a marca) e faz o backup.
    let mut a = Banco::abrir(&db).unwrap();
    let no_log = contar(&a, "SELECT COUNT(*) FROM alteracao WHERE tabela = 'produto_gtin'");
    assert_eq!(no_log, 2);
    let feito = rustinho_nucleo::backup::fazer(&db, false).unwrap();
    rustinho_nucleo::backup::restaurar(&mut a.conn, Some(&db), &feito.arquivo, false, senha).unwrap();
    let marca = contar(&a, "SELECT COUNT(*) FROM config WHERE chave = 'sync_inicio:produto_gtin'");
    assert_eq!(marca, 1, "a marca do backup da 5.2 sumiu na restauração");
    drop(a);
    let a = Banco::abrir(&db).unwrap();
    let depois = contar(&a, "SELECT COUNT(*) FROM alteracao WHERE tabela = 'produto_gtin'");
    assert_eq!(depois, no_log, "os GTINs de um backup da 5.2 foram registrados de novo");
    drop(a);
    let _ = std::fs::remove_dir_all(&pasta);
}
