# Rede, firewall e VPN

Este texto é para quem instala o Rustinho numa loja. Ele explica o que precisa estar liberado para os caixas se acharem e
como ligar caixas que ficam em lugares diferentes.

## As portas
| Porta | Para quê | De onde |
|---|---|---|
| **TCP 47810** | A troca de dados entre os caixas da loja | Dos outros caixas da loja |
| **UDP 47811** | O anúncio: cada caixa avisa, a cada 10 s, que está na rede | Só da rede local da loja |
| TCP 47820 e UDP 47821 | As mesmas, no **Rustinho portátil** (o de testar) | — |

- **No firewall do computador** (o do Windows, por exemplo), libere a TCP 47810 e a UDP 47811 **para a rede da loja**.
- **Nunca abra a porta 47810 para a internet** (no roteador, o "redirecionamento de porta"). Ela é para a rede da loja ou
  para uma VPN.

## Caixas na mesma loja
Na mesma rede (o mesmo Wi-Fi ou o mesmo cabo), os caixas se acham sozinhos pelo anúncio.
- **Quando não se acham** (uma rede com o anúncio bloqueado, por exemplo), digite em Configurações › Simultaneidade ›
  "Endereço dos outros caixas":
  - o **número** de um caixa (`192.168.0.12`); ou
  - o **nome** dele na rede (`caixa2.loja.local`).
- **Com nome,** a tela mostra o número que o nome deu, e o Rustinho acompanha se ele mudar.

## Caixas em lugares diferentes: VPN
O Rustinho não tem VPN própria. Ele usa a da loja. Uma VPN é uma "rede particular" pela internet: os computadores se
enxergam como se estivessem na mesma loja.
- **O jeito simples, que vale para qualquer VPN:** digite, no outro caixa, o **endereço da VPN** deste computador (o
  número que a VPN deu a ele, por exemplo `10.8.0.2` ou `100.64.0.5`), como acima. O endereço digitado sempre vale.
- **WireGuard, Tailscale e OpenVPN comum** são VPNs que dão um endereço próprio ao computador (os técnicos chamam de "camada
  3"). Para elas, Configurações › Simultaneidade mostra "VPN neste computador: wg0 (10.8.0.2)": esse é o número que os outros
  digitam.
- **ZeroTier e OpenVPN no modo `tap`** imitam um cabo de rede (os técnicos chamam de "camada 2"). Também funcionam pelo
  endereço digitado. Para os caixas se acharem **sozinhos** por elas, sem digitar, libere a interface em "Interfaces
  extras" (no modo completo).
- **Se não souber qual é a sua VPN:** use o número que a VPN mostra para este computador.
- O Rustinho não anuncia nada pela VPN.
- **Dados móveis:** só com VPN no aparelho (a operadora não deixa receber conexão).
- **Por nome numa VPN:** se a VPN dá nomes aos computadores (o Tailscale, por exemplo), digite o nome.

## Quando não conecta
- **"… não foi encontrado na rede":** confira o nome, ou use o número.
- **"… não respondeu a tempo":** o servidor de nomes da rede está lento; use o número.
- **"… usando o último endereço bom":** o nome falhou agora, mas o Rustinho continua pelo número que funcionava.
- **"(este próprio computador)":** o nome digitado é deste computador, e não de outro caixa.
- **Nome com sublinhado (`caixa_2`):** não é aceito; use hífen (`caixa-2`) ou o número.
- **Um lado acha o outro, mas o outro não acha este:** o firewall de um dos dois está bloqueando a TCP 47810.
