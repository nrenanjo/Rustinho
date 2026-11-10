# Amostras de estrutura (dev.3.3, passo 1.6)

Três bancos **de teste**, um de cada estrutura, usados por `nucleo/tests/migracoes_linhagem.rs`
(AUD33-N01, DP-50). Os dados são de mentira: loja "Loja Teste", usuária Ana, no máximo uma peça,
endereços só `127.0.0.1`.

| Arquivo | `user_version` | Linha | De onde veio |
|---|---|---|---|
| `dev32_estrutura24.db` | 24 | dev.3.1/dev.3.2 (esta linha) | banco deixado pelos testes automáticos da dev.3.2 |
| `dev3_estrutura24.db` | 24 | dev.3 paralela (`v0.9.18-dev.3`, `8cb29af`), antes da 0025 | banco deixado pelos testes automáticos da dev.3 |
| `dev3_estrutura25.db` | 25 | dev.3 paralela, com a 0025 dela | banco deixado pelos testes automáticos da dev.3 |

O número 24 tem duas estruturas diferentes: a 0024 desta linha (`0024_gtins_e_pendencias_xml.sql`)
e a 0024 da dev.3 (`0024_identificadores_produto.sql`). A dev.3 tem ainda uma 0025 própria
(`0025_cancelamento_idempotente.sql`). O Rustinho só guarda o número; por isso a conferência tem de
ser pelo conteúdo (`docs/MIGRACOES.md`).

## Cópias limpas

Os originais estão em `_reversa_sdd/auditoria_dev33/investigacao_dev3/amostras/`, com o SHA-256 em
`investigacao_dev3/SHA256SUMS.txt`, e não foram mexidos. Estas cópias diferem deles numa coisa só:
o valor de `config.computador_id`, que nos originais é a impressão digital do computador onde os
testes rodaram. Aqui ele vale `sha256("amostra-dev33-ficticia")`. A troca foi feita com
`secure_delete` e `VACUUM`, sem rodar nenhum gatilho, e a estrutura (`sqlite_master` e
`user_version`) ficou idêntica à dos originais. A integridade foi conferida (`integrity_check` = ok).

| Arquivo | SHA-256 do original | SHA-256 desta cópia |
|---|---|---|
| `dev32_estrutura24.db` | `8f1eee3d117c137a8363c31c9973206fdc94be5bb32d57827286567d9868d0b3` | `dd73352676adf18bd4c56113ff933b8a7064d98702b0fd1972bdcd280a00922c` |
| `dev3_estrutura24.db` | `9c3c6f3371764e242ddbe76ecc3857abc2790876c499a6d5d4c9dc596aece387` | `26a094e9e21adc25bc620078fe6979a8bdf207f31dc562643bc6cf7d3478a95f` |
| `dev3_estrutura25.db` | `c7bca8c005ad77bf2cc0b031bf1b45326a520cf548e9d66c3adceddcb6972123` | `563f70206aca6427f40e99e163fa726a950eb218a77fd27018e6ed4646db5953` |

O teste confere `SHA256SUMS` antes de usar as amostras e só abre **cópias** delas, numa pasta
temporária. Nunca troque estes arquivos: se precisar de outra amostra, acrescente uma nova e
registre a origem aqui.
