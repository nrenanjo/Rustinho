# Rede, firewall e VPN

Este texto é para quem instala o Rustinho numa loja. Ele explica o que precisa estar liberado para os caixas se acharem e
como ligar caixas que ficam em lugares diferentes.

## As portas
| Porta | Para quê | De onde |
|---|---|---|
| **TCP 47810** | A troca de dados entre os caixas da loja (e, na 0.9.19, com as lojas parceiras) | Dos outros caixas da loja e das lojas parceiras |
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

## Caixas ou lojas em lugares diferentes: VPN
O Rustinho não tem VPN própria. Ele usa a da loja:
- **VPN de camada 3** (WireGuard, OpenVPN no modo `tun`, Tailscale): funciona pelo endereço da VPN, digitado como acima.
  - Em Configurações › Simultaneidade, a tela mostra "VPN neste computador: wg0 (10.8.0.2)", que é o endereço que os outros
    digitam.
  - O Rustinho não anuncia nada pela VPN.
- **VPN de camada 2** (ZeroTier, OpenVPN no modo `tap`): é tratada como rede virtual, e só vale se for liberada em
  "Interfaces extras" (no modo completo).
- **Dados móveis:** só com VPN no aparelho (a operadora não deixa receber conexão).
- **Por nome numa VPN:** se a VPN dá nomes aos computadores (o Tailscale, por exemplo), digite o nome.

## Quando não conecta
- **"O nome … não foi encontrado na rede":** confira o nome, ou use o número.
- **"O nome … não respondeu a tempo":** o servidor de nomes da rede está lento; use o número.
- **Um lado acha o outro, mas o outro não acha este:** o firewall de um dos dois está bloqueando a TCP 47810.
