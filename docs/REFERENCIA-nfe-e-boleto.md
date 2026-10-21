# Referência para a fase fiscal e financeira: NF-e de venda e boleto

O dono da loja mostrou uma NF-e de venda real e o boleto dela (21/10/2026).
**O documento real não fica no repositório nem em publicação nenhuma.** Aqui
fica só o que ele ensina sobre a estrutura, sem nomes, CNPJ, endereços, chave
de acesso, protocolo, nem dados do banco ou do boleto. Quando precisar de uma
fixture, ela é sintética (como `nucleo/tests/pki/gerar_acervo.py`).

Isto **não muda o escopo atual** (estabilidade e troca de dados). Serve de
roteiro para a fase fiscal (`docs/PLANO-fiscal-e-plataformas.md`) e para as
contas a receber por boleto.

## 1. NF-e de venda (emissão própria)

O que o DANFE mostra e o Rustinho vai ter de montar e testar:

| Parte | No exemplo | O que testar no Rustinho |
|---|---|---|
| Identificação | modelo 55, série 1, número de 9 dígitos, saída, natureza "VENDA" | numeração sem buraco e sem repetir por série, mesmo com dois caixas |
| Emitente | empresa do Simples Nacional (CSOSN) | dados vindos de Configurações › Loja |
| Destinatário | pessoa jurídica com inscrição estadual, mesmo município | cliente com CNPJ e IE; com CPF; consumidor sem cadastro (NFC-e) |
| Itens | 10 linhas: código, descrição, NCM, CSOSN 0102, CFOP 5102, unidade (PEC, UN, BD, PC), quantidade, valor unitário, total | cada campo vindo da peça e da venda; unidades de fora (BD = balde) mapeadas |
| Código repetido | **o mesmo código interno em duas peças diferentes** (duas correias de medidas diferentes) | peças não podem ser fundidas: corrigido na importação (D-13); na emissão, cada item sai com a descrição da sua peça |
| Totais | produtos = nota; frete 0; desconto 0; sem ICMS destacado; "valor aproximado dos tributos" com percentual | soma dos itens = total; desconto rateado; arredondamento por item |
| Transporte | "9 – sem frete"; quantidade de volumes | modalidade de frete e volumes na venda |
| Tributos | CSOSN 102, sem ICMS, PIS e COFINS no Simples | **configurável**: o exemplo vale para esta nota, não é regra para todas as peças da loja; cada peça e cada operação têm a sua (ST, monofásico, CFOP de outro estado, devolução) |
| Lei da transparência | valor e percentual aproximado dos tributos | percentual por NCM vindo de tabela atualizável, não fixo |

Para validar a estrutura fiscal completa (assinatura, protocolo, campos que o
DANFE não mostra), é preciso o **XML autorizado** da mesma nota. O PDF é só
referência visual. Pedir ao dono o XML, que fica fora do repositório (junto
do acervo real, só para conferência local).

## 2. Boleto (contas a receber)

O que o boleto mostra e o que isso pede do Rustinho:

| No boleto | No Rustinho |
|---|---|
| Valor = total da nota; vencimento em cerca de 20 dias | conta a receber criada na venda, ligada à venda, com o vencimento do boleto |
| Número do documento = número da nota | a conta guarda o número da nota e o "nosso número" do banco |
| Multa de 2% e mora a partir do dia seguinte ao vencimento; data limite de pagamento | multa e juros **configuráveis** (já existem no crediário), mais a data limite; os percentuais do exemplo são do banco e do contrato da loja, não uma regra fixa |
| Pix no boleto (QR) e linha digitável | só guardar e mostrar; o Rustinho não gera o boleto (é do banco) |

Regra que não pode quebrar: **emitir o boleto não é receber.** A conta fica
em aberto até a loja confirmar o pagamento (pelo extrato do banco ou pelo
aviso de pagamento). A baixa:

- é feita por quem tem permissão, com a data em que o dinheiro entrou;
- entra na conta da loja certa (banco), não na gaveta;
- pode ser parcial; multa e juros entram como receita financeira (como no crediário, T3.2);
- pode ser estornada (dinheiro devolvido ou baixa errada), voltando a conta para aberta;
- um boleto vencido e não pago aparece como atrasado, com multa e juros calculados, sem baixa automática.

Testes a escrever na fase financeira (como os de T3): venda com boleto → conta
em aberto, nada no caixa; baixa total na data do crédito; baixa parcial; baixa
depois do vencimento com multa e juros conferidos à mão; estorno da baixa;
boleto cancelado (venda cancelada antes de pagar); conferência de que a DRE
não conta o boleto como recebido antes da baixa.

## 3. O que já ficou valendo agora

- D-13 (código repetido com descrições diferentes) foi corrigido e testado
  com notas de mentira (T7.1 e T7.3).
- Nada mais mudou por causa deste exemplo.
