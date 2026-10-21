# Testes de estabilidade (antes da versão estável)

O Rustinho só será chamado de **estável** quando todos os testes abaixo
estiverem aprovados, os automáticos **e** os feitos à mão, no **Linux** e no
**Windows 10/11 64 bits**. Android e Bluetooth ficam em espera.

- Testes automáticos: `cargo test -p rustinho-nucleo --test estavel` (pasta
  `nucleo/tests/estavel/`). Rodam sozinhos, com dados de mentira, em pastas
  temporárias.
- Teste das telas: `python3 app/testes/telas_d09.py` (D-09) e
  `python3 app/testes/telas_d14_portatil.py` (D-14 e o Rustinho portátil),
  depois de `cargo build -p rustinho` (precisa de Xvfb, xdotool e ImageMagick).
- Testes à mão: `docs/ROTEIRO-linux-completo.md` (o mesmo roteiro serve no
  Windows, com o ZIP portátil `Rustinho-<versão>-Windows-portatil.zip`).
- Auditoria técnica da 0.9.14 (defeitos, riscos, melhorias, testes que
  faltam e critérios de maturidade): `docs/AUDITORIA-0.9.14.md`.
- Defeitos: cada um é anotado em `docs/DEFEITOS.md` **antes** da correção.
  Nenhuma função nova entra enquanto houver defeito aberto de prioridade alta.

## Prioridades

**Prioridade 1** — dinheiro, estoque e dados: se falhar, a loja perde dinheiro
ou informação. **Prioridade 2** — o que depende de fora (sistema antigo, rede).

| # | Prioridade | Assunto | Automático (o que prova) | À mão | Situação |
|---|---|---|---|---|---|
| T1 | 1 | Backup e restauração em outra pasta | T1.1: loja com um pouco de tudo → backup → computador novo → restaura → **todas as 37 tabelas do negócio iguais**, telas de dinheiro e estoque iguais, configurações da loja iguais, backup e origem intactos, e o restaurado continua vendendo. T1.2: a cópia "de antes" desfaz uma restauração errada. T1.3: backup estragado ou de outro programa é recusado sem mudar nada. | Roteiro, parte B | automático ✔ · à mão pendente |
| T2 | 1 | Estoque | T2.1 venda, cancelamento (uma vez só), devolução boa e com defeito, devolução a mais recusada. T2.2 compra por XML entra; estorno tira (uma vez só). T2.3 ajuste (contagem) e só o gerente ajusta. T2.4 vender mais do que tem: passa, fica negativo, avisa antes. T2.5 **ao mesmo tempo**: este caixa vende 30 enquanto a troca de dados grava 30 do outro → 60 baixas, nenhuma perdida nem repetida. | Roteiro, parte C | automático ✔ (achou D-05, corrigido) |
| T3 | 1 | Crediário | T3.1 parciais, parcial que pega duas parcelas, quitação com troco, gaveta certa. T3.2 atraso de 40 dias: multa 2% + juros 1%/mês = R$ 3,33 (conta à mão) e os juros vão para a DRE como receita financeira. T3.3 estorno: dívida e gaveta voltam, só o gerente, uma vez só. T3.4 devolução abate a dívida; venda com recebimento não é cancelada. T3.5 crediário só com cliente; caixa fechado não recebe. | Roteiro, parte D | automático ✔ |
| T4 | 1 | DRE | T4.1 cinco vendas com desconto, peça sem custo, cancelamento, devolução boa e com defeito, DAS e aluguel: **cada linha confere com a conta à mão** (no arquivo do teste); pagar não duplica despesa; despesa do mês passado lançada hoje não muda este mês; custo cadastrado depois não muda venda feita; desfazer devolução volta receita e custo. T4.2 o caixa não vê a DRE por nenhuma porta. | Roteiro, parte E (conferir com o contador) | automático ✔ · conferência do dono e do contador pendente (PEND-20) |
| T5 | 1 | Permissões | T5.1 **cada uma** das funções do gerente (mais de 50), chamada direto por um caixa, é recusada e nada muda. T5.2 sem entrar, nada funciona. T5.3 o que o caixa lê não traz lucro nem dados do financeiro. T5.4 o gerente continua vendo tudo. T5.5 senha do gerente no aperto vale para uma ação; 5 erradas fazem esperar; fica na auditoria. T5.6 gerente rebaixado ou usuário desativado em outro caixa perde o acesso na hora. | Roteiro, parte F | automático ✔ (achou D-01 a D-04 e D-07, corrigidos) |
| T6 | 1 | Fechamento inesperado | O próprio teste abre um Rustinho "filho" que vende, recebe e devolve sem parar e **mata o processo de uma vez** (como falta de luz), duas vezes seguidas: o banco abre inteiro, sem venda pela metade, estoque e crediário batendo, tudo na fila da troca de dados, caixa ainda aberto. T6.2 queda no meio do backup: dados intactos, cópias prontas abrem, sobras limpas. T6.3 queda no meio da restauração: ao abrir, termina sozinha. | Roteiro, parte G | automático ✔ (achou D-08, corrigido) |
| T7 | 2 | Importação do SINTEC | Acervo fictício (15 arquivos: compras, vendas num .zip, nota repetida, devoluções, nota adulterada, evento, arquivo estragado, **mesmo código em peças diferentes**) com a conta à mão: 11 notas, 3 ignorados, 2 clientes, 2 fornecedores, 11 peças, 1 código ligado; código repetido não funde peças (D-13); preço da nota mais nova; custo e preço certos; **nada de estoque nem dinheiro**; importar de novo não repete; **a pasta de origem fica igual** (conteúdo e data de cada arquivo). Só o gerente importa. T7.3: compra por XML com o mesmo código do fornecedor em duas peças não liga sozinha. | Roteiro, parte H (com uma **cópia** da exportação real) | automático ✔ (achou D-13, corrigido) · com o acervo real pendente |
| D-09 | 1 | O caixa e os custos | `d09_custos` (9): o caixa não recebe custo, % nem histórico do custo (ficha, peça nova, busca, Configurações, estoque parado); o que ele manda não muda custo nem % (nem por chamada direta); continua cadastrando, criando tamanhos e lançando compras; na compra vê o custo; peça nova da compra sai com o preço pelo lucro sugerido; **qualquer** mudança do preço de venda de peça cadastrada (tabela, oferta, preço na hora, com custo ou sem), do % ou do lucro sugerido pede o gerente; desconto na venda não pede. Senha do gerente: presa ao usuário, ao comando e aos dados (outro motivo, outra venda, um campo a mais, outro comando: recusado; chaves em outra ordem: aceito); uma vez e acaba; com erro, só repete a mesma coisa; consulta de tela só lê aquela tela (não grava, não abre outra tela, nem Backup nem pílula Gerente), acaba ao sair e não passa para outro usuário; a senha não fica no banco, no registro nem em arquivo nenhum. `app/testes/telas_d09.py`: as telas de verdade como caixa e como gerente; o caixa muda um preço (com e sem custo), a tela pede a senha, salva uma vez, pede de novo; desistir não grava; nas Configurações, Backup e Gerente pedem a senha deles. Cada teste foi conferido desligando a regra de propósito. | Roteiro, parte F (F5 a F10) | automático ✔ |
| D-14 | 1 | Caixa já fechado não muda | `a1_auditoria::d14_*` (10): cancelar venda (dinheiro com troco, Pix, sem caixa aberto, gaveta sem dinheiro bastante, venda do caixa aberto), estornar recebimento e desfazer devolução de um caixa fechado: o fechamento antigo fica **idêntico** (esperado, contado, diferença, vendas, canceladas, por forma, recebido, devolvido), o dinheiro fica na gaveta aberta (sangria/suprimento ligado à operação) ou no Caixa geral, uma vez só (repetir é recusado), auditoria; o fluxo de caixa não conta duas vezes; depois de reabrir, igual. `d14_migracao_22_nao_muda_o_que_existia`. Tela de verdade: a gerente cancela pela tela a venda de um caixa fechado; o banco e a tela do caixa conferem. Cada parte desligada de propósito: os testes falharam. | Roteiro, parte K | automático ✔ |
| D-15 | 1 | % de lucro abaixo do custo na compra | `compras::testes::d15_lucro_abaixo_do_custo` (5 custos, igual à conta do cadastro); falha sem a correção. | — | automático ✔ |
| D-16 | 1 | Configuração deste computador não vem da rede | `sync::testes::d16_configuracao_deste_computador_nao_vem_da_rede`; falha sem a correção. | — | automático ✔ |
| P | 1 | Rustinho portátil isolado | `tests/portatil.rs`: o teste copia o próprio executável para uma pasta com a marca e roda a cópia (outro processo) com as pastas de sempre e `RUSTINHO_DADOS` apontando para uma "instalação de verdade" de mentira: o banco fica em `dados/`; troca de dados nas portas 47820/47821 e com outra chave; backup só dentro da pasta (pasta de fora recusada, e uma de fora já gravada não vale); Google Drive com configuração própria; **nenhum arquivo da instalação muda nem aparece**. Tela de verdade (Linux): abre a loja nova, banco em `dados/`, nada do Rustinho fora da pasta. No Windows (robô do GitHub): uma cópia do portátil aberta sem tela cria o banco em `dados\` e nada em `%LOCALAPPDATA%\Rustinho` nem em `RUSTINHO_DADOS`. Desligado de propósito: os testes falharam. | Roteiro, parte L | automático ✔ · **Windows à mão pendente** |
| T8 | 2 | Troca de dados entre computadores | Pela rede de verdade (portas do mesmo computador): B puxa 40 vendas de A, **o processo de B é morto no meio**, B volta e termina com as 40, sem nada em dobro; puxar de novo não traz nada; os dois vendem e trocam até ficarem iguais. Senha da rede errada não troca nada. | FV-1 a FV-13 (abaixo) | automático ✔ · **validação física pendente** |

Observação do T8: depois de uma queda, o outro caixa pode levar até cerca de
1 minuto para aceitar de novo (proteção contra quem tenta adivinhar a senha da
rede). A troca de dados tenta de novo sozinha.

## O que ainda falta para "estável"

1. Rodar o roteiro à mão no Linux (`docs/ROTEIRO-linux-completo.md`).
2. Rodar o mesmo roteiro no Windows 10/11 com o ZIP portátil da 0.9.14 (o
   ZIP já é montado pelo robô do GitHub; os testes do núcleo também rodam lá,
   veja abaixo).
3. Validação física da troca de dados (FV-1 a FV-13) com dois computadores na loja.
4. Conferência da DRE pelo dono e pelo contador (PEND-20).
5. Importação de uma **cópia** da exportação real do SINTEC, conferindo os totais.
6. ~~Decisão do dono sobre a D-09~~: decidida e corrigida (testes `d09_custos` e `app/testes/telas_d09.py`).
7. As fotos das telas (Linux, X virtual) acharam D-10, D-11 e D-12; o roteiro à mão pode achar mais.
8. Decidir os riscos R-01 (senha da rede curta), R-02 (cancelar venda de um
   caixa aberto em outro computador) e R-03 (parcela paga a mais por dois
   caixas sem rede) de `docs/AUDITORIA-0.9.14.md`.

## FV-1 a FV-13 em português simples

São os testes da troca de dados que só dá para fazer com **dois computadores
de verdade** na loja. Nenhum foi feito ainda.

| Teste | O que fazer | O que deve acontecer |
|---|---|---|
| FV-1 | Dois computadores ligados no mesmo roteador por cabo e também no Wi-Fi. | Cada um aparece uma vez só para o outro, pelo cabo. |
| FV-2 | Ligar os dois computadores direto um no outro, por um cabo, sem roteador. | Eles se acham e trocam dados. |
| FV-3 | No meio de uma troca grande, tirar o cabo; depois pôr de volta. | Passa para o Wi-Fi em uns 10 segundos e volta para o cabo em uns 35; no fim, as mesmas vendas e peças nos dois, sem nada repetido. |
| FV-4 | Desligar o modem (sem internet). | A troca continua funcionando. |
| FV-5 | Celular só no Wi-Fi e computador no cabo e no Wi-Fi. | O celular troca pelo Wi-Fi. (Android em espera.) |
| FV-6 | Primeira vez no Windows em português. | O Windows pergunta sobre o firewall; respondendo "rede privada", funciona; as placas de rede aparecem com o nome certo. |
| FV-7 | Wi-Fi com "isolamento de clientes" ligado (comum em roteador de visitante). | Pelo Wi-Fi não troca, pelo cabo troca, e a mensagem é clara. |
| FV-8 | Mudar o endereço (IP) de um computador com o Rustinho aberto. | O outro acha de novo em 10 a 20 segundos, sem duplicar. |
| FV-9 | Computador com VirtualBox, Docker ou VPN ligados. | Não atrapalha e não é usado no lugar da placa da loja. |
| FV-10 | Suspender o notebook e voltar. | A troca volta sozinha. |
| FV-11 | Celular com dados móveis ligados e Wi-Fi da loja sem internet. | Troca com o computador, ou avisa claramente que não deu. (Android em espera.) |
| FV-12 | Cabo e Wi-Fi na mesma rede, com a troca presa ao Wi-Fi. | Os dados chegam pela placa escolhida (o outro computador confirma). |
| FV-13 | VPN, Docker ou VirtualBox ligados. | O Rustinho não anuncia por elas; se você liberar uma delas, passa a anunciar. |
