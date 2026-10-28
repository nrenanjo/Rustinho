# Matriz de homologação (etapa C8, 28/10/2026)

Regra (briefing §8): teste não executado **não** está aprovado. "Não executado" = ninguém rodou; "Executado aqui" = rodado nesta etapa no ambiente de desenvolvimento descrito abaixo, que **não** é equipamento físico da loja.

**Ambiente desta etapa:** contêiner Linux na nuvem, Ubuntu 24.04.5 LTS, núcleo 6.18, x86-64, 2 processadores, 7 GB; Rust 1.97.0; Qt 6.4.2; telas em X virtual (Xvfb); sem rede física, sem periféricos, sem certificado, sem Windows, sem Android.

## Linux
| # | Cenário | Equipamento / SO / versão | Passos | Resultado |
|---|---|---|---|---|
| L1 | Compilação limpa do núcleo e do app | contêiner acima, 0.9.15+C8 | `cargo build`, `cargo clippy --workspace --all-targets`, `cargo fmt --check` | **Executado aqui: passou** (`logs/10-*`, `logs/11-build-app.log`) |
| L2 | Suíte completa | idem | `cargo test --workspace --no-fail-fast` | **Executado aqui: 476 passaram, 0 falharam, 9 ignorados** (`logs/10-suite-final.log`) |
| L3 | Telas (X virtual) | idem, Xvfb | `app/testes/telas_*.py` (5 roteiros) | **Executado aqui: 61 de 61** (`logs/12-telas-*.log`) — não substitui mouse/teclado reais |
| L4 | Pacote AppImage: montar | idem | montagem local | Montado nesta sessão (entregue antes); **instalação e uso num computador real: não executado** |
| L5 | Backup, restauração, modo seguro, banco antigo com defeito | idem | testes automáticos (A3, RVI-03, F-11, backup da zip 2 → zip 9) | **Executado aqui: passou** (`logs/03-*`, `logs/08-r07-compat-backup-antigo.log`) |
| L6 | Dois computadores reais (cabo, Wi-Fi, perda e volta) | — | — | **Não executado** |
| L7 | Leitor físico de códigos, impressora | — | — | **Não executado** |
| L8 | Pasta do modo portátil sem permissão de escrita (R-08) | — | — | **Não executado** |

## Windows
| # | Cenário | Resultado |
|---|---|---|
| W1 | Compilação real (workflow) e instalador | **Não executado** nesta etapa (sem push: proibido) |
| W2 | Abrir, restaurar, rede, permissões, interface | **Não executado** |
| W3 | Porta ocupada, troca de interface de rede, reconexão | **Não executado** (no Linux: `logs/02-rvi02-*`) |
| W4 | Impressora, leitor de códigos | **Não executado** |
| W5 | Desinstalar remove a pasta do usuário certo (instalador usa `{localappdata}` de quem instala como administrador — observação desta etapa) | **Não executado** |
| W6 | Compilar com a `zip` 9 (R-07) | **Não executado** |

## Android
| # | Cenário | Resultado |
|---|---|---|
| A1 | APK real (workflow) | **Não executado** nesta etapa; o `Cargo.lock` do Android foi atualizado (zip 9) e `cargo audit` passou, mas **não compilado** |
| A2 | 32 bits (Moto e6i, Android 10): tela cinza (PEND-18) | **Aberto**; diagnóstico pronto, não rodado no aparelho |
| A3 | 64 bits: interface, permissões, persistência, restauração | **Não executado** |
| A4 | Wi-Fi sem internet, troca de dados, bateria, pausa/retomada/fechar | **Não executado** |
| A5 | Registro de diagnóstico no armazenamento externo (observação desta etapa: visível a outros apps) | **Não executado** |

## Rede e vários caixas
| # | Cenário | Resultado |
|---|---|---|
| R1 | Dois caixas no mesmo computador, portas escolhidas pelo sistema | **Executado aqui: passou** (família de rede sob carga, `logs/05-familia-rede-sob-carga.log`, e suíte final) |
| R2 | Porta ocupada por outro programa (aviso, retomada) | **Executado aqui: passou** (`logs/02-rvi02-*`, tela `telas_rvi02.py`) |
| R3 | Dois ou mais caixas **físicos**: venda, compra, recebimento, pagamento ao mesmo tempo | **Não executado** |
| R4 | Queda de rede no meio de um lote | só automático (`teste_rede.rs`, lote atômico); **físico não executado** |
| R5 | Sem rede e troca depois; conflitos financeiros (R-03) | só automático (`testes_r03.rs`, detecção); **físico não executado** |
| R6 | Cópia, restauração, caixa novo (D1) | cópia/restauração automáticas passam; **D1 não implementado** (proposta) |

## Fiscal
| # | Cenário | Resultado |
|---|---|---|
| F1 | Certificado A3 e leitor físicos | **Não executado** (DF-2, DF-4 abertas) |
| F2 | Homologação oficial SEFAZ-SP (55 e 65) | **Não executado**: não há emissão nem adaptador neste repositório (`docs/AUDITORIA-FISCAL-C8.md`) |
| F3 | Rejeição, indisponibilidade, recuperação | **Não executado** |
| F4 | Conferência de XML de **compra** (assinatura, cadeia, titular) | **Executado aqui (automático): passou** na suíte; acervo real de 528 NF-e conferido em 20/10/2026 (PEND-09), não repetido nesta etapa |
| F5 | Protocolo e numeração de notas emitidas | **Não executado** (não há emissão) |

## Resumo
Executados aqui: L1, L2, L3, L5, R1, R2, F4 (e L4 só a montagem). Todo o resto: **não executado** — Windows, Android, rede física, periféricos e fiscal ficam para a homologação no equipamento da loja.
