//! QT-1 / A1 (14/11/2026): a atividade do Android declara as mudanças de
//! configuração que o app trata sem ser recriado.
//!
//! No POCO C85 (etapa 2 da validação física), ligar "Texto em negrito" com o
//! Rustinho aberto fez o Android recriar a atividade, e o Qt 6.8
//! (`QtActivityBase.onCreate` → `restartApplication`) reiniciou o processo:
//! o que estava só na memória (a venda em andamento, a OS não salva) se
//! perdeu. Declarar a mudança no `android:configChanges` faz o Android só
//! avisar a atividade, sem recriá-la. Este teste falha se alguma das
//! mudanças sair do manifesto.

use std::path::Path;

/// As mudanças que o app trata sem recriar a atividade. As 14 primeiras são
/// as do modelo do Qt; as 4 últimas entraram com a A1 (antes, cada uma delas
/// recriava a atividade e o Qt reiniciava o app).
const TRATADAS: &[&str] = &[
    "orientation",
    "uiMode",
    "screenLayout",
    "screenSize",
    "smallestScreenSize",
    "layoutDirection",
    "locale",
    "fontScale",
    "keyboard",
    "keyboardHidden",
    "navigation",
    "mcc",
    "mnc",
    "density",
    // A1: estas recriavam a atividade (e o Qt reiniciava o processo).
    "fontWeightAdjustment",
    "colorMode",
    "grammaticalGender",
    "touchscreen",
];

#[test]
fn a_atividade_declara_as_mudancas_que_o_app_trata_sem_ser_recriado() {
    let caminho = Path::new(env!("CARGO_MANIFEST_DIR")).join("../android/pacote/AndroidManifest.xml");
    let xml = std::fs::read_to_string(&caminho).unwrap_or_else(|e| panic!("{}: {e}", caminho.display()));
    let atividade = xml
        .split("<activity")
        .skip(1)
        .find(|a| a.contains("org.qtproject.qt.android.bindings.QtActivity"))
        .expect("a QtActivity no manifesto");
    let valor = atividade
        .split("android:configChanges=\"")
        .nth(1)
        .and_then(|r| r.split('"').next())
        .expect("android:configChanges na QtActivity");
    let declaradas: Vec<&str> = valor.split('|').map(str::trim).collect();
    let faltam: Vec<&&str> = TRATADAS.iter().filter(|m| !declaradas.contains(m)).collect();
    assert!(
        faltam.is_empty(),
        "faltam no android:configChanges da QtActivity: {faltam:?}. Sem elas, a mudança recria a atividade e o Qt reinicia o app (perde o trabalho que está só na memória)."
    );
}
