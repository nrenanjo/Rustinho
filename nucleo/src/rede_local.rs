//! PEND-10: a rede local do ponto de vista deste computador.
//!
//! A troca de dados entre caixas funciona sem internet, só pela rede local.
//! Este módulo cuida de três coisas pequenas e **sem tocar no protocolo**:
//!
//! 1. saber quais interfaces de rede (cabo, Wi-Fi, ...) estão ativas e de que
//!    tipo são;
//! 2. anunciar este caixa por **cada** interface (um broadcast só sai por uma);
//! 3. ordenar os caminhos até um outro caixa: cabo primeiro, Wi-Fi como reserva,
//!    com espera crescente para um caminho que falhou.
//!
//! Um caminho é só um endereço "ip:porta". Quem é o caixa continua sendo o seu
//! identificador (`dispositivo`), nunca o endereço. Tudo aqui que depende do
//! sistema (listar as interfaces) entra como dado, para os testes simularem
//! várias interfaces com os endereços 127.0.0.x.

use std::net::{Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
use std::path::Path;
use std::time::Duration;

/// Tipo de uma interface ou de um caminho. A ordem é a **prioridade**: o
/// menor vem primeiro (cabo, depois o que não se sabe, depois Wi-Fi, e por
/// último o virtual e o celular).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tipo {
    Cabo,
    Desconhecida,
    Wifi,
    Virtual,
    Celular,
}

impl Tipo {
    /// Como aparece para a pessoa (Configurações › Troca de dados).
    pub fn rotulo(self) -> &'static str {
        match self {
            Tipo::Cabo => "cabo",
            Tipo::Desconhecida => "rede",
            Tipo::Wifi => "Wi-Fi",
            Tipo::Virtual => "rede virtual",
            Tipo::Celular => "dados móveis",
        }
    }
}

/// Uma interface IPv4 deste computador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    pub nome: String,
    pub ip: Ipv4Addr,
    pub mascara: Ipv4Addr,
    /// Ligada e com cabo/sinal. Desativada não recebe anúncio nem serve de caminho.
    pub ativa: bool,
    pub tipo: Tipo,
}

impl Interface {
    #[cfg(test)]
    pub fn de_teste(nome: &str, ip: [u8; 4], prefixo: u8, tipo: Tipo) -> Interface {
        Interface {
            nome: nome.into(),
            ip: Ipv4Addr::from(ip),
            mascara: Ipv4Addr::from(u32::MAX.checked_shl(32 - prefixo as u32).unwrap_or(0)),
            ativa: true,
            tipo,
        }
    }
}

/// Classifica uma interface pelo nome que o sistema dá a ela (Linux, Android
/// e Windows, inclusive em português). Quando o nome não diz nada, é
/// `Desconhecida` (nem a primeira nem a última da fila). Em Linux e Android
/// `listar` ainda confere o que o sistema sabe (`/sys/class/net`).
pub fn classificar(nome: &str) -> Tipo {
    let n = nome.to_lowercase();
    let tem = |trechos: &[&str]| trechos.iter().any(|t| n.contains(t));
    let comeca = |prefixos: &[&str]| prefixos.iter().any(|p| n.starts_with(p));
    // Virtual primeiro: "vEthernet" e "VirtualBox" têm "ethernet"/"box" no nome.
    if tem(&[
        "docker",
        "virtual",
        "vmware",
        "vmnet",
        "vboxnet",
        "hyper-v",
        "vethernet",
        "tailscale",
        "zerotier",
        "loopback",
        "pseudo",
        "bluetooth",
        "tunnel",
        "vpn",
    ]) || comeca(&["veth", "br-", "virbr", "tun", "tap", "wg", "utun", "dummy", "lxc", "cni", "flannel", "podman"])
    {
        return Tipo::Virtual;
    }
    if comeca(&["rmnet", "ccmni", "wwan", "ppp", "usb0"]) || tem(&["celular", "cellular", "mobile", "móvel", "movel"])
    {
        return Tipo::Celular;
    }
    if comeca(&["wlan", "wlp", "wlx", "wifi", "wl0", "wl1", "swlan", "p2p"])
        || tem(&["wi-fi", "wifi", "wireless", "sem fio", "wlan", "802.11"])
    {
        return Tipo::Wifi;
    }
    let cabo_linux = n.starts_with("eth")
        || (n.starts_with("en")
            && n[2..]
                .chars()
                .next()
                .is_some_and(|c| c == 'p' || c == 'o' || c == 's' || c == 'x' || c.is_ascii_digit()));
    if cabo_linux || tem(&["ethernet", "conexão local", "conexao local", "local area connection", "lan"]) {
        return Tipo::Cabo;
    }
    Tipo::Desconhecida
}

/// As interfaces IPv4 deste computador (sem a de loopback). Erro se o sistema
/// não souber listar: quem chama volta ao comportamento antigo.
pub fn listar() -> std::io::Result<Vec<Interface>> {
    let mut lista = Vec::new();
    for i in if_addrs::get_if_addrs()? {
        if !aceita_na_lista(i.is_loopback(), i.is_p2p()) {
            continue;
        }
        let if_addrs::IfAddr::V4(v4) = &i.addr else { continue };
        let ativa = matches!(
            i.oper_status,
            if_addrs::IfOperStatus::Up | if_addrs::IfOperStatus::Unknown | if_addrs::IfOperStatus::Dormant
        ) && !v4.ip.is_unspecified();
        lista.push(Interface { tipo: tipo_no_sistema(&i.name), nome: i.name, ip: v4.ip, mascara: v4.netmask, ativa });
    }
    Ok(lista)
}

/// Fica na lista de interfaces? Fora só a de loopback e a que o **sistema**
/// marca como ponto a ponto (PPP, VPN). Endereço 169.254.x.x (cabo direto
/// sem DHCP) não é ponto a ponto e entra, no cabo e no Wi-Fi.
pub fn aceita_na_lista(loopback: bool, ponto_a_ponto: bool) -> bool {
    !loopback && !ponto_a_ponto
}

/// O tipo da interface: primeiro o que o sistema sabe dela, só depois o nome.
/// Sem nenhuma das duas informações, `Desconhecida` (e continua assim).
fn tipo_no_sistema(nome: &str) -> Tipo {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        tipo_com(Path::new("/sys/class/net"), nome)
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        classificar(nome)
    }
}

/// Metadados (se houver em `raiz`, no formato de `/sys/class/net`), senão o nome.
pub fn tipo_com(raiz: &Path, nome: &str) -> Tipo {
    tipo_por_metadados(raiz, nome).unwrap_or_else(|| classificar(nome))
}

/// O que o sistema diz da interface, sem olhar o nome. `None` quando ele não
/// diz nada útil (ou não deixa ler: o Android restringe a pasta).
///
/// - sem fio (`wireless`, `phy80211` ou `DEVTYPE=wlan`) → Wi-Fi;
/// - modem móvel (`DEVTYPE=wwan`) → dados móveis;
/// - ponte, VLAN, túnel, `veth`, ou dentro de `/virtual/` → virtual;
/// - Ethernet (`type` 1) ligada a um aparelho físico (`device`) → cabo.
///
/// Limite conhecido: um celular ligado por USB como placa de rede aparece como
/// Ethernet física e é tratado como cabo; só o nome o desmascararia.
pub fn tipo_por_metadados(raiz: &Path, nome: &str) -> Option<Tipo> {
    if nome.is_empty() || nome.contains(['/', '\\']) || nome.contains("..") {
        return None;
    }
    let base = raiz.join(nome);
    if !base.exists() {
        return None;
    }
    let uevent = std::fs::read_to_string(base.join("uevent")).unwrap_or_default();
    let devtype = uevent.lines().find_map(|l| l.strip_prefix("DEVTYPE=")).map(str::trim);
    if base.join("wireless").exists() || base.join("phy80211").exists() || devtype == Some("wlan") {
        return Some(Tipo::Wifi);
    }
    if devtype == Some("wwan") {
        return Some(Tipo::Celular);
    }
    if matches!(devtype, Some("bridge" | "vlan" | "veth" | "vxlan" | "macvlan" | "wireguard" | "gre" | "ipip" | "sit"))
        || std::fs::read_link(&base).is_ok_and(|d| d.to_string_lossy().contains("/virtual/"))
    {
        return Some(Tipo::Virtual);
    }
    let tipo_arp: Option<u32> = std::fs::read_to_string(base.join("type")).ok().and_then(|t| t.trim().parse().ok());
    match tipo_arp {
        Some(65534) => Some(Tipo::Virtual), // sem tipo de hardware: túnel (tun, wireguard)
        Some(1) if base.join("device").exists() => Some(Tipo::Cabo),
        _ => None,
    }
}

/// Esta interface anuncia o caixa sozinha? Cabo e Wi-Fi, sim. Virtual (VPN,
/// Docker, VMs) e dados móveis, só se a pessoa os liberou pelo nome em
/// `sync_interfaces`. A não identificada anuncia apenas com IP de rede privada
/// ou 169.254.x.x (cabo direto), nunca com um IP público.
pub fn anunciavel(i: &Interface, liberadas: &[String]) -> bool {
    if !i.ativa {
        return false;
    }
    if liberada(&i.nome, liberadas) {
        return true;
    }
    match i.tipo {
        Tipo::Cabo | Tipo::Wifi => true,
        Tipo::Desconhecida => i.ip.is_private() || i.ip.is_link_local(),
        Tipo::Virtual | Tipo::Celular => false,
    }
}

fn liberada(nome: &str, liberadas: &[String]) -> bool {
    liberadas.iter().any(|l| l.trim().eq_ignore_ascii_case(nome))
}

/// Vale um caminho **aprendido** (pelo anúncio) até este endereço? Não vale se
/// a interface que o alcança é virtual ou móvel e a pessoa não a liberou. Fora
/// das sub-redes conhecidas, ou sem lista de interfaces, vale (como antes).
pub fn caminho_aprendido_vale(interfaces: Option<&[Interface]>, endereco: &str, liberadas: &[String]) -> bool {
    let (Some(lista), Some(ip)) = (interfaces, ip_do_endereco(endereco)) else {
        return true;
    };
    if ip.is_loopback() {
        return true;
    }
    match alcanca(lista, ip) {
        Some(i) => !matches!(i.tipo, Tipo::Virtual | Tipo::Celular) || liberada(&i.nome, liberadas),
        None => true,
    }
}

/// Por onde sair para falar com um caminho: o IP (e o nome) da interface que o
/// alcança. `None` deixa o sistema decidir (destino fora das sub-redes
/// conhecidas, sem lista). O loopback real nunca está na lista de interfaces,
/// então nunca é preso; só os testes põem 127.0.0.x nela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saida {
    pub ip: Ipv4Addr,
    pub nome: Option<String>,
}

pub fn saida_para(interfaces: Option<&[Interface]>, endereco: &str) -> Option<Saida> {
    let ip = ip_do_endereco(endereco)?;
    alcanca(interfaces?, ip).map(|i| Saida { ip: i.ip, nome: Some(i.nome.clone()) })
}

/// O que se conseguiu de fato ao conectar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirmacao {
    /// O socket foi preso ao IP da interface antes de conectar.
    pub preso_ao_ip: bool,
    /// `None`: não se tentou. `Some(true/false)`: o sistema aceitou/negou
    /// prender também ao dispositivo (`SO_BINDTODEVICE`, só Linux e Android).
    pub preso_a_interface: Option<bool>,
    /// O IP local da conexão estabelecida, conforme o sistema.
    pub local: Option<Ipv4Addr>,
}

impl Confirmacao {
    /// Saiu mesmo pela interface planejada?
    pub fn confirmada(&self, saida: &Saida) -> bool {
        self.preso_ao_ip && self.local == Some(saida.ip)
    }
}

/// Conecta a `alvo` saindo, quando possível, pela interface `saida`. Se não
/// der para prender (IP que este computador já não tem, permissão negada,
/// sistema sem suporte), conecta do jeito de sempre e diz isso na
/// `Confirmacao`: o caixa continua alcançável, só "não confirmado".
pub fn conectar(
    alvo: SocketAddr,
    saida: Option<&Saida>,
    limite: Duration,
) -> std::io::Result<(TcpStream, Confirmacao)> {
    use socket2::{Domain, Socket, Type};
    if let (Some(saida), SocketAddr::V4(_)) = (saida, alvo) {
        let preso = (|| -> std::io::Result<(TcpStream, Confirmacao)> {
            let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
            #[allow(unused_mut)]
            let mut a_interface = None;
            #[cfg(any(target_os = "linux", target_os = "android"))]
            if let Some(nome) = &saida.nome {
                a_interface = Some(socket.bind_device(Some(nome.as_bytes())).is_ok());
            }
            socket.bind(&SocketAddr::from((saida.ip, 0)).into())?;
            socket.connect_timeout(&alvo.into(), limite)?;
            let fluxo: TcpStream = socket.into();
            let local = fluxo.local_addr().ok().and_then(|a| match a.ip() {
                std::net::IpAddr::V4(v4) => Some(v4),
                _ => None,
            });
            Ok((fluxo, Confirmacao { preso_ao_ip: true, preso_a_interface: a_interface, local }))
        })();
        match preso {
            Ok(ok) => return Ok(ok),
            // Não conseguiu prender: tenta sem prender. Falha de rede de verdade
            // (recusada, tempo esgotado) é a mesma nos dois jeitos: a segunda
            // tentativa não repete o tempo-limite inteiro, só quem não prendeu.
            Err(e) if !prender_falhou(&e) => return Err(e),
            Err(_) => {}
        }
    }
    let fluxo = TcpStream::connect_timeout(&alvo, limite)?;
    let local = fluxo.local_addr().ok().and_then(|a| match a.ip() {
        std::net::IpAddr::V4(v4) => Some(v4),
        _ => None,
    });
    Ok((fluxo, Confirmacao { preso_ao_ip: false, preso_a_interface: None, local }))
}

/// O erro é de não poder prender o socket (e não de a rede estar fora)?
fn prender_falhou(e: &std::io::Error) -> bool {
    use std::io::ErrorKind::*;
    matches!(e.kind(), AddrNotAvailable | PermissionDenied | InvalidInput | Unsupported | AddrInUse)
}

/// O endereço de broadcast da sub-rede (ex.: 192.168.0.12/24 → 192.168.0.255).
/// Sem ele (máscara inválida, /31 e /32) a interface não recebe anúncio.
pub fn broadcast_dirigido(ip: Ipv4Addr, mascara: Ipv4Addr) -> Option<Ipv4Addr> {
    let m = u32::from(mascara);
    // máscara contínua (1s seguidos e depois 0s)
    if m.leading_ones() + m.trailing_zeros() != 32 || m == 0 {
        return None;
    }
    if m.count_ones() > 30 {
        return None;
    }
    Some(Ipv4Addr::from(u32::from(ip) | !m))
}

/// A interface ativa deste computador que alcança `destino` diretamente
/// (mesma sub-rede). Se mais de uma serve, a de máscara mais específica.
pub fn alcanca(interfaces: &[Interface], destino: Ipv4Addr) -> Option<&Interface> {
    interfaces
        .iter()
        .filter(|i| i.ativa)
        .filter(|i| {
            let m = u32::from(i.mascara);
            m != 0 && (u32::from(i.ip) & m) == (u32::from(destino) & m)
        })
        .min_by_key(|i| (std::cmp::Reverse(u32::from(i.mascara).count_ones()), i.tipo))
}

/// Para onde e de onde mandar o anúncio: (IP local para onde o socket é
/// preso, destino do broadcast). Um por interface apropriada (`anunciavel`)
/// mais o broadcast limitado de sempre (sem IP fixo).
pub fn alvos_de_anuncio(interfaces: &[Interface], liberadas: &[String]) -> Vec<(Ipv4Addr, Ipv4Addr)> {
    let mut alvos: Vec<(Ipv4Addr, Ipv4Addr)> = Vec::new();
    for i in interfaces.iter().filter(|i| anunciavel(i, liberadas)) {
        if let Some(b) = broadcast_dirigido(i.ip, i.mascara) {
            if !alvos.contains(&(i.ip, b)) {
                alvos.push((i.ip, b));
            }
        }
    }
    alvos.push((Ipv4Addr::UNSPECIFIED, Ipv4Addr::BROADCAST));
    alvos
}

/// Manda o `pacote` por cada interface. Um erro numa interface não impede as
/// outras: volta a lista (IP local, texto do erro) do que falhou.
pub fn anunciar(interfaces: &[Interface], liberadas: &[String], pacote: &[u8], porta: u16) -> Vec<(Ipv4Addr, String)> {
    let mut erros = Vec::new();
    for (de, para) in alvos_de_anuncio(interfaces, liberadas) {
        let r = UdpSocket::bind((de, 0)).and_then(|s| {
            s.set_broadcast(true)?;
            s.send_to(pacote, (para, porta)).map(|_| ())
        });
        if let Err(e) = r {
            erros.push((de, e.to_string()));
        }
    }
    erros
}

/// A lista de interfaces mudou desde a última vez (interface nova, que sumiu,
/// sem sinal ou com outro IP)? A ordem não conta. Mudou → anuncia na hora, sem
/// esperar os 10 s.
pub fn interfaces_mudaram(antes: &[Interface], agora: &[Interface]) -> bool {
    let assinatura = |l: &[Interface]| -> Vec<(String, Ipv4Addr, Ipv4Addr, bool)> {
        let mut v: Vec<_> = l.iter().map(|i| (i.nome.clone(), i.ip, i.mascara, i.ativa)).collect();
        v.sort();
        v
    };
    assinatura(antes) != assinatura(agora)
}

/// `Some(lista)` se o sistema soube listar; `None` se falhou (quem chama volta
/// ao comportamento antigo: broadcast limitado e nenhum caminho descartado).
pub fn lista_ou_nada(r: std::io::Result<Vec<Interface>>) -> Option<Vec<Interface>> {
    r.ok()
}

/// Dos erros de envio de agora, os que **não** estavam na rodada anterior (o
/// mesmo erro, repetido a cada 10 s, entra no registro uma vez só). `anteriores`
/// passa a ser o que deu erro agora.
pub fn erros_novos(
    anteriores: &mut Vec<(Ipv4Addr, String)>,
    atuais: Vec<(Ipv4Addr, String)>,
) -> Vec<(Ipv4Addr, String)> {
    let novos = atuais.iter().filter(|e| !anteriores.contains(e)).cloned().collect();
    *anteriores = atuais;
    novos
}

/// Espera (ms) antes de tentar de novo um caminho que acabou de falhar
/// `falhas` vezes seguidas: 8 s, 16 s e depois 30 s. Um anúncio vindo do
/// caminho zera a espera (o outro caixa está lá).
pub fn espera_ms(falhas: i64) -> i64 {
    match falhas {
        i64::MIN..=0 => 0,
        1 => 8_000,
        2 => 16_000,
        _ => 30_000,
    }
}

/// Quantos caminhos de um mesmo caixa são tentados num mesmo ciclo.
pub const CAMINHOS_POR_CICLO: usize = 3;

/// O que se sabe de um caminho (uma linha de `sync_endereco`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caminho {
    pub endereco: String,
    /// Digitado em Configurações: nunca some sozinho e vale mesmo fora das
    /// sub-redes que este computador conhece (ex.: atrás de um roteador).
    pub digitado: bool,
    pub visto_ms: i64,
    pub ultimo_ok_ms: i64,
    pub falhas: i64,
    pub proxima_ms: i64,
}

/// Só o IP de "ip:porta" (IPv4).
pub fn ip_do_endereco(endereco: &str) -> Option<Ipv4Addr> {
    endereco.parse::<std::net::SocketAddr>().ok().and_then(|s| match s.ip() {
        std::net::IpAddr::V4(v4) => Some(v4),
        _ => None,
    })
}

/// O tipo do caminho: o da interface deste computador que o alcança.
pub fn tipo_do_caminho(interfaces: Option<&[Interface]>, endereco: &str) -> Tipo {
    match (interfaces, ip_do_endereco(endereco)) {
        (Some(lista), Some(ip)) => alcanca(lista, ip).map_or(Tipo::Desconhecida, |i| i.tipo),
        _ => Tipo::Desconhecida,
    }
}

/// Os caminhos a tentar neste ciclo, na ordem: cabo, depois o que não se
/// sabe, depois Wi-Fi, depois virtual e celular; na mesma classe, o que
/// funcionou há menos tempo. Ficam de fora:
/// - o que ainda está na espera de uma falha;
/// - o endereço aprendido pelo anúncio cuja interface sumiu deste computador
///   (cabo desconectado): não se espera o tempo-limite da conexão.
///
/// `interfaces = None` (o sistema não soube listar): nada é descartado nem
/// classificado, como era antes.
pub fn ordenar(caminhos: &[Caminho], interfaces: Option<&[Interface]>, agora_ms: i64) -> Vec<Caminho> {
    let mut v: Vec<(Tipo, &Caminho)> = caminhos
        .iter()
        .filter(|c| c.proxima_ms <= agora_ms)
        .filter(|c| match (interfaces, ip_do_endereco(&c.endereco)) {
            (Some(lista), Some(ip)) if !c.digitado && !ip.is_loopback() => alcanca(lista, ip).is_some(),
            _ => true,
        })
        .map(|c| (tipo_do_caminho(interfaces, &c.endereco), c))
        .collect();
    v.sort_by(|(ta, a), (tb, b)| {
        ta.cmp(tb)
            .then(b.ultimo_ok_ms.cmp(&a.ultimo_ok_ms))
            .then(b.visto_ms.cmp(&a.visto_ms))
            .then(a.endereco.cmp(&b.endereco))
    });
    v.into_iter().take(CAMINHOS_POR_CICLO).map(|(_, c)| c.clone()).collect()
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::time::Duration;

    fn c(endereco: &str, ok: i64, visto: i64) -> Caminho {
        Caminho {
            endereco: endereco.into(),
            digitado: false,
            visto_ms: visto,
            ultimo_ok_ms: ok,
            falhas: 0,
            proxima_ms: 0,
        }
    }

    /// T-01 · CA-R1: classificação pelo nome da interface.
    #[test]
    fn t01_classifica_pelo_nome() {
        for n in [
            "eth0",
            "eth1",
            "enp3s0",
            "eno1",
            "ens33",
            "enx00e04c680001",
            "Ethernet",
            "Ethernet 2",
            "Conexão local",
            "Local Area Connection",
        ] {
            assert_eq!(classificar(n), Tipo::Cabo, "{n}");
        }
        for n in [
            "wlan0",
            "wlp2s0",
            "wlx0013ef",
            "Wi-Fi",
            "Wi-Fi 2",
            "Conexão de Rede Wireless",
            "Wireless Network Connection",
            "Rede sem fio",
        ] {
            assert_eq!(classificar(n), Tipo::Wifi, "{n}");
        }
        for n in [
            "docker0",
            "veth12ab",
            "br-3f2a",
            "virbr0",
            "tun0",
            "tap0",
            "vEthernet (Default Switch)",
            "VirtualBox Host-Only Network",
            "VMware Network Adapter VMnet8",
            "tailscale0",
            "wg0",
            "dummy0",
        ] {
            assert_eq!(classificar(n), Tipo::Virtual, "{n}");
        }
        for n in ["rmnet_data0", "ccmni0", "wwan0", "ppp0", "Conexão Celular"] {
            assert_eq!(classificar(n), Tipo::Celular, "{n}");
        }
        for n in ["xyz0", "", "lo"] {
            assert_eq!(classificar(n), Tipo::Desconhecida, "{n}");
        }
        // a ordem de prioridade é a da fila: cabo, o que não se sabe, Wi-Fi, virtual, celular
        assert!(Tipo::Cabo < Tipo::Desconhecida && Tipo::Desconhecida < Tipo::Wifi);
        assert!(Tipo::Wifi < Tipo::Virtual && Tipo::Virtual < Tipo::Celular);
    }

    /// O broadcast dirigido e as sub-redes.
    #[test]
    fn broadcast_e_sub_rede() {
        let b = |ip: [u8; 4], p: u8| {
            let i = Interface::de_teste("x", ip, p, Tipo::Cabo);
            broadcast_dirigido(i.ip, i.mascara)
        };
        assert_eq!(b([192, 168, 0, 12], 24), Some(Ipv4Addr::new(192, 168, 0, 255)));
        assert_eq!(b([10, 1, 2, 3], 16), Some(Ipv4Addr::new(10, 1, 255, 255)));
        assert_eq!(b([172, 16, 5, 9], 30), Some(Ipv4Addr::new(172, 16, 5, 11)));
        assert_eq!(b([169, 254, 7, 8], 16), Some(Ipv4Addr::new(169, 254, 255, 255)));
        assert_eq!(b([192, 168, 0, 12], 31), None, "/31 não tem broadcast");
        assert_eq!(b([192, 168, 0, 12], 32), None, "/32 não tem broadcast");
        assert_eq!(
            broadcast_dirigido(Ipv4Addr::new(10, 0, 0, 1), Ipv4Addr::new(255, 0, 255, 0)),
            None,
            "máscara torta"
        );
        assert_eq!(broadcast_dirigido(Ipv4Addr::new(10, 0, 0, 1), Ipv4Addr::UNSPECIFIED), None);
    }

    /// T-02 · CA-D1, CA-D6: um alvo por interface ativa; sem desativada, sem
    /// /32; link-local entra; o broadcast limitado uma vez só no fim.
    #[test]
    fn t02_alvos_de_anuncio() {
        let mut desligada = Interface::de_teste("enp4s0", [192, 168, 9, 2], 24, Tipo::Cabo);
        desligada.ativa = false;
        let lista = vec![
            Interface::de_teste("eno1", [192, 168, 0, 12], 24, Tipo::Cabo),
            Interface::de_teste("wlan0", [10, 0, 5, 30], 16, Tipo::Wifi),
            Interface::de_teste("eth9", [169, 254, 7, 8], 16, Tipo::Cabo),
            Interface::de_teste("ponto", [10, 9, 9, 9], 32, Tipo::Virtual),
            desligada,
            Interface::de_teste("eno1:1", [192, 168, 0, 12], 24, Tipo::Cabo), // alias repetido
        ];
        assert_eq!(
            alvos_de_anuncio(&lista, &[]),
            vec![
                (Ipv4Addr::new(192, 168, 0, 12), Ipv4Addr::new(192, 168, 0, 255)),
                (Ipv4Addr::new(10, 0, 5, 30), Ipv4Addr::new(10, 0, 255, 255)),
                (Ipv4Addr::new(169, 254, 7, 8), Ipv4Addr::new(169, 254, 255, 255)),
                (Ipv4Addr::UNSPECIFIED, Ipv4Addr::BROADCAST),
            ]
        );
        // sem nenhuma interface (lista vazia ou falha): continua o de antes
        assert_eq!(alvos_de_anuncio(&[], &[]), vec![(Ipv4Addr::UNSPECIFIED, Ipv4Addr::BROADCAST)]);
    }

    /// T-03 · CA-D1, CA-D3: envio de verdade por sockets. Duas "interfaces"
    /// (127.0.0.2 e 127.0.0.3, ambas na sub-rede 127.0.0.0/8) mandam o
    /// anúncio; quem escuta em 0.0.0.0 enxerga as duas origens. Uma interface
    /// com IP que este computador não tem falha sem impedir a outra.
    #[cfg(unix)]
    #[test]
    fn t03_anuncio_sai_por_cada_interface() {
        let ouvinte = UdpSocket::bind(("0.0.0.0", 0)).unwrap();
        ouvinte.set_read_timeout(Some(Duration::from_millis(300))).unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        let lista = vec![
            Interface::de_teste("sim-cabo", [127, 0, 0, 2], 8, Tipo::Cabo),
            Interface::de_teste("sim-wifi", [127, 0, 0, 3], 8, Tipo::Wifi),
            Interface::de_teste("fantasma", [203, 0, 113, 77], 24, Tipo::Cabo), // IP que não é deste computador
        ];
        let erros = anunciar(&lista, &[], b"ola", porta);
        let mut origens = Vec::new();
        let mut buf = [0u8; 16];
        while let Ok((n, de)) = ouvinte.recv_from(&mut buf) {
            assert_eq!(&buf[..n], b"ola");
            origens.push(de.ip().to_string());
        }
        origens.sort();
        origens.dedup();
        assert!(origens.contains(&"127.0.0.2".to_string()), "faltou a origem do cabo: {origens:?}");
        assert!(origens.contains(&"127.0.0.3".to_string()), "faltou a origem do Wi-Fi: {origens:?}");
        assert!(
            erros.iter().any(|(ip, _)| *ip == Ipv4Addr::new(203, 0, 113, 77)),
            "a interface fantasma devia falhar: {erros:?}"
        );
        assert!(!erros.iter().any(|(ip, _)| ip.octets()[0] == 127), "as boas não podem falhar: {erros:?}");
    }

    /// Qual interface alcança um endereço (a mais específica) e o tipo do caminho.
    #[test]
    fn caminho_pela_interface_que_alcanca() {
        let lista = vec![
            Interface::de_teste("eno1", [192, 168, 0, 12], 24, Tipo::Cabo),
            Interface::de_teste("wlan0", [192, 168, 1, 40], 24, Tipo::Wifi),
            Interface::de_teste("geral", [192, 168, 0, 99], 16, Tipo::Virtual),
        ];
        assert_eq!(alcanca(&lista, Ipv4Addr::new(192, 168, 0, 50)).unwrap().nome, "eno1", "/24 vence /16");
        assert_eq!(alcanca(&lista, Ipv4Addr::new(192, 168, 1, 7)).unwrap().nome, "wlan0");
        assert_eq!(alcanca(&lista, Ipv4Addr::new(192, 168, 77, 7)).unwrap().nome, "geral");
        assert!(alcanca(&lista, Ipv4Addr::new(10, 1, 1, 1)).is_none());
        assert_eq!(tipo_do_caminho(Some(&lista), "192.168.1.7:47810"), Tipo::Wifi);
        assert_eq!(tipo_do_caminho(Some(&lista), "10.1.1.1:47810"), Tipo::Desconhecida);
        assert_eq!(tipo_do_caminho(None, "192.168.1.7:47810"), Tipo::Desconhecida);
        assert_eq!(tipo_do_caminho(Some(&lista), "isto não é endereço"), Tipo::Desconhecida);
    }

    /// T-06 · CA-R1: ordem dos caminhos.
    #[test]
    fn t06_cabo_antes_do_wifi() {
        let lista = vec![
            Interface::de_teste("eno1", [192, 168, 0, 12], 24, Tipo::Cabo),
            Interface::de_teste("wlan0", [192, 168, 1, 40], 24, Tipo::Wifi),
            Interface::de_teste("docker0", [172, 17, 0, 1], 16, Tipo::Virtual),
            Interface::de_teste("rmnet0", [100, 64, 0, 5], 16, Tipo::Celular),
        ];
        // o Wi-Fi funcionou mais recentemente, mesmo assim o cabo vai primeiro
        let caminhos = vec![
            c("192.168.1.7:47810", 9_000, 9_000),                         // Wi-Fi
            c("192.168.0.7:47810", 1_000, 1_000),                         // cabo
            c("172.17.0.7:47810", 8_000, 8_000),                          // virtual
            c("100.64.0.9:47810", 7_000, 7_000),                          // celular
            Caminho { digitado: true, ..c("10.50.0.7:47810", 500, 500) }, // digitado, fora das sub-redes (roteador)
        ];
        let ordem: Vec<String> = ordenar(&caminhos, Some(&lista), 10_000).into_iter().map(|c| c.endereco).collect();
        // só 3 por ciclo: cabo, digitado (desconhecida), Wi-Fi
        assert_eq!(ordem, ["192.168.0.7:47810", "10.50.0.7:47810", "192.168.1.7:47810"]);

        // na mesma classe, o que funcionou há menos tempo vai primeiro
        let dois_cabos =
            vec![c("192.168.0.7:47810", 1_000, 1), Caminho { digitado: true, ..c("192.168.0.8:47810", 5_000, 1) }];
        let ordem: Vec<String> = ordenar(&dois_cabos, Some(&lista), 10_000).into_iter().map(|c| c.endereco).collect();
        assert_eq!(ordem, ["192.168.0.8:47810", "192.168.0.7:47810"]);
    }

    /// T-09 · CA-R4: a interface sumiu (cabo desconectado) e o caminho que ela
    /// servia deixa de ser candidato na hora; o digitado e o de loopback ficam.
    #[test]
    fn t09_interface_que_sumiu_tira_o_caminho() {
        let caminhos = vec![
            c("192.168.0.7:47810", 1_000, 1_000),
            c("192.168.1.7:47810", 500, 500),
            Caminho { digitado: true, ..c("192.168.0.8:47810", 0, 0) },
            c("127.0.0.1:47810", 0, 0),
        ];
        let com_cabo = vec![
            Interface::de_teste("eno1", [192, 168, 0, 12], 24, Tipo::Cabo),
            Interface::de_teste("wlan0", [192, 168, 1, 40], 24, Tipo::Wifi),
        ];
        assert_eq!(ordenar(&caminhos, Some(&com_cabo), 2_000).len(), 3 /* limite por ciclo */);
        let mut sem_cabo = com_cabo.clone();
        sem_cabo[0].ativa = false;
        let ordem: Vec<String> = ordenar(&caminhos, Some(&sem_cabo), 2_000).into_iter().map(|c| c.endereco).collect();
        assert!(!ordem.contains(&"192.168.0.7:47810".to_string()), "caminho do cabo solto continuou: {ordem:?}");
        assert!(ordem.contains(&"192.168.1.7:47810".to_string()), "o Wi-Fi tem de continuar: {ordem:?}");
        assert!(ordem.contains(&"192.168.0.8:47810".to_string()), "o digitado nunca sai por isso: {ordem:?}");
        // lista vazia (todas as interfaces caíram): só digitado e loopback
        let ordem: Vec<String> = ordenar(&caminhos, Some(&[]), 2_000).into_iter().map(|c| c.endereco).collect();
        assert_eq!(ordem.len(), 2);
        // o sistema não soube listar: nada é descartado (como antes)
        assert_eq!(ordenar(&caminhos, None, 2_000).len(), 3);
    }

    /// T-08 (parte pura) · CA-R3: a espera cresce e o caminho em espera fica de fora.
    #[test]
    fn t08_espera_crescente() {
        assert_eq!(
            [espera_ms(0), espera_ms(1), espera_ms(2), espera_ms(3), espera_ms(40)],
            [0, 8_000, 16_000, 30_000, 30_000]
        );
        let mut cabo = c("192.168.0.7:47810", 1_000, 1_000);
        cabo.falhas = 1;
        cabo.proxima_ms = 20_000 + espera_ms(1);
        let wifi = c("192.168.1.7:47810", 500, 500);
        let lista = vec![
            Interface::de_teste("eno1", [192, 168, 0, 12], 24, Tipo::Cabo),
            Interface::de_teste("wlan0", [192, 168, 1, 40], 24, Tipo::Wifi),
        ];
        let todos = vec![cabo.clone(), wifi.clone()];
        let nomes =
            |agora| -> Vec<String> { ordenar(&todos, Some(&lista), agora).into_iter().map(|c| c.endereco).collect() };
        assert_eq!(nomes(20_000), ["192.168.1.7:47810"], "dentro da espera só o Wi-Fi");
        assert_eq!(nomes(27_999), ["192.168.1.7:47810"]);
        assert_eq!(
            nomes(28_000),
            ["192.168.0.7:47810", "192.168.1.7:47810"],
            "passada a espera o cabo volta a ser o primeiro"
        );
    }

    // ------------------------------------------------------------------
    // Revisão 2 da PEND-10: conexão presa à interface, metadados do sistema,
    // anúncio só nas interfaces apropriadas, link-local.
    // ------------------------------------------------------------------

    use std::net::{SocketAddr, TcpListener};

    /// Servidor de teste: aceita `n` conexões e devolve de que IP cada uma veio.
    fn servidor_que_anota(n: usize) -> (SocketAddr, std::thread::JoinHandle<Vec<Ipv4Addr>>) {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let alvo = ouvinte.local_addr().unwrap();
        let t = std::thread::spawn(move || {
            let mut origens = Vec::new();
            for _ in 0..n {
                let (_fluxo, de) = ouvinte.accept().unwrap();
                if let std::net::IpAddr::V4(v4) = de.ip() {
                    origens.push(v4);
                }
            }
            origens
        });
        (alvo, t)
    }

    /// T-27 · CA-T1: a conexão sai do IP da interface escolhida (o outro caixa
    /// enxerga essa origem); sem escolher, sai como sempre.
    #[cfg(unix)]
    #[test]
    fn t27_conexao_presa_ao_ip_da_interface() {
        let (alvo, servidor) = servidor_que_anota(3);
        let cabo = Saida { ip: Ipv4Addr::new(127, 0, 0, 2), nome: None };
        let wifi = Saida { ip: Ipv4Addr::new(127, 0, 0, 3), nome: None };
        let (_a, ca) = conectar(alvo, Some(&cabo), Duration::from_secs(3)).unwrap();
        let (_b, cb) = conectar(alvo, Some(&wifi), Duration::from_secs(3)).unwrap();
        let (_c, cc) = conectar(alvo, None, Duration::from_secs(3)).unwrap();
        assert_eq!(servidor.join().unwrap(), [cabo.ip, wifi.ip, Ipv4Addr::LOCALHOST]);
        assert!(ca.preso_ao_ip && cb.preso_ao_ip && !cc.preso_ao_ip);
        assert_eq!(ca.local, Some(cabo.ip));
        assert_eq!(cb.local, Some(wifi.ip));
    }

    /// T-28 · CA-T2, CA-T4: o programa confere por onde a conexão saiu de
    /// verdade. IP que não é deste computador: conecta como hoje, "não confirmada".
    #[cfg(unix)]
    #[test]
    fn t28_confere_a_interface_realmente_usada() {
        let (alvo, servidor) = servidor_que_anota(2);
        let cabo = Saida { ip: Ipv4Addr::new(127, 0, 0, 2), nome: None };
        let (_a, ok) = conectar(alvo, Some(&cabo), Duration::from_secs(3)).unwrap();
        assert!(ok.confirmada(&cabo), "saiu pelo IP escolhido");
        // a interface planejada some (IP que este computador não tem): tenta sem prender
        let sumida = Saida { ip: Ipv4Addr::new(203, 0, 113, 77), nome: None };
        let (_b, nao) = conectar(alvo, Some(&sumida), Duration::from_secs(3)).unwrap();
        assert!(!nao.preso_ao_ip, "não conseguiu prender");
        assert!(!nao.confirmada(&sumida), "não confirmada: saiu por outro lado");
        assert_eq!(nao.local, Some(Ipv4Addr::LOCALHOST), "e diz por onde saiu de verdade");
        assert_eq!(servidor.join().unwrap().len(), 2);
    }

    /// T-29 · CA-T3 (Linux): prender também à interface (`SO_BINDTODEVICE`)
    /// funciona ou é negado sem estragar a conexão; interface com nome
    /// inexistente não impede a conexão.
    #[cfg(target_os = "linux")]
    #[test]
    fn t29_prende_a_interface_ou_cai_para_so_o_ip() {
        let (alvo, servidor) = servidor_que_anota(2);
        let em_lo = Saida { ip: Ipv4Addr::new(127, 0, 0, 2), nome: Some("lo".into()) };
        let (_a, a) = conectar(alvo, Some(&em_lo), Duration::from_secs(3)).unwrap();
        assert!(a.preso_ao_ip);
        assert!(a.preso_a_interface.is_some(), "o teste precisa saber se tentou prender à interface");
        eprintln!("T-29: SO_BINDTODEVICE em 'lo' neste computador: {:?}", a.preso_a_interface);
        let inexistente = Saida { ip: Ipv4Addr::new(127, 0, 0, 3), nome: Some("naoexiste0".into()) };
        let (_b, b) = conectar(alvo, Some(&inexistente), Duration::from_secs(3)).unwrap();
        assert_eq!(b.preso_a_interface, Some(false), "nome inexistente: não prende, mas conecta");
        assert!(b.preso_ao_ip);
        assert_eq!(servidor.join().unwrap().len(), 2);
    }

    /// CA-T2: a conferência compara o IP local que o sistema diz ter usado com
    /// o planejado. Um sistema que ignora o `bind` (conexão presa, mas saída por
    /// outro IP) nunca é dado como confirmado.
    #[test]
    fn confirmacao_compara_o_ip_local_com_o_planejado() {
        let planejada = Saida { ip: Ipv4Addr::new(192, 168, 0, 12), nome: None };
        let c = |preso: bool, local: Option<[u8; 4]>| Confirmacao {
            preso_ao_ip: preso,
            preso_a_interface: None,
            local: local.map(Ipv4Addr::from),
        };
        assert!(c(true, Some([192, 168, 0, 12])).confirmada(&planejada));
        assert!(!c(true, Some([192, 168, 1, 40])).confirmada(&planejada), "saiu por outra placa");
        assert!(!c(true, None).confirmada(&planejada), "o sistema não disse");
        assert!(!c(false, Some([192, 168, 0, 12])).confirmada(&planejada), "não prendeu: coincidência não prova");
    }

    /// A saída planejada para um caminho: a interface deste computador que o alcança.
    #[test]
    fn saida_planejada_vem_da_interface_que_alcanca() {
        let lista = vec![
            Interface::de_teste("eno1", [192, 168, 0, 12], 24, Tipo::Cabo),
            Interface::de_teste("wlan0", [192, 168, 1, 40], 24, Tipo::Wifi),
        ];
        let s = saida_para(Some(&lista), "192.168.1.7:47810").unwrap();
        assert_eq!((s.ip, s.nome.as_deref()), (Ipv4Addr::new(192, 168, 1, 40), Some("wlan0")));
        assert!(saida_para(Some(&lista), "10.9.9.9:47810").is_none(), "fora das sub-redes: o sistema decide");
        assert!(saida_para(None, "192.168.1.7:47810").is_none());
        assert!(
            saida_para(Some(&lista), "127.0.0.1:47810").is_none(),
            "loopback real: não está na lista, o sistema decide"
        );
    }

    /// Monta uma pasta no formato de `/sys/class/net` para os testes.
    fn sysfs_falso(itens: &[(&str, &[&str], &str, &str)]) -> std::path::PathBuf {
        // (nome, arquivos/pastas dentro, conteúdo de `type`, conteúdo de `uevent`)
        let raiz = std::env::temp_dir().join(format!("rustinho-sysfs-{}", crate::tempo::novo_id()));
        for (nome, dentro, tipo, uevent) in itens {
            let base = raiz.join(nome);
            std::fs::create_dir_all(&base).unwrap();
            for d in *dentro {
                std::fs::create_dir_all(base.join(d)).unwrap();
            }
            if !tipo.is_empty() {
                std::fs::write(base.join("type"), format!("{tipo}\n")).unwrap();
            }
            std::fs::write(base.join("uevent"), uevent).unwrap();
        }
        raiz
    }

    /// T-30 · CA-K1, CA-K2: o que o sistema sabe vale mais que o nome; sem
    /// metadado, o nome; sem nome que diga algo, "desconhecida".
    #[test]
    fn t30_metadados_do_sistema_antes_do_nome() {
        let raiz = sysfs_falso(&[
            ("eth0", &["wireless"], "1", "DEVTYPE=wlan\n"), // nome de cabo, mas é sem fio
            ("wlan0", &["device"], "1", ""),                // nome de Wi-Fi, mas é Ethernet física
            ("enp9s0", &["device"], "1", ""),
            ("docker0", &[], "1", "DEVTYPE=bridge\n"),
            ("tun0", &[], "65534", ""),
            ("wwan0", &["device"], "1", "DEVTYPE=wwan\n"),
            ("xyz0", &[], "", ""), // existe, mas o sistema não diz nada
            ("xyz1", &["device"], "1", ""),
        ]);
        let t = |n: &str| tipo_por_metadados(&raiz, n);
        assert_eq!(t("eth0"), Some(Tipo::Wifi), "metadado ganha do nome");
        assert_eq!(t("wlan0"), Some(Tipo::Cabo), "metadado ganha do nome (Ethernet física)");
        assert_eq!(t("enp9s0"), Some(Tipo::Cabo));
        assert_eq!(t("docker0"), Some(Tipo::Virtual));
        assert_eq!(t("tun0"), Some(Tipo::Virtual));
        assert_eq!(t("wwan0"), Some(Tipo::Celular));
        assert_eq!(t("xyz0"), None, "sem metadado útil: a decisão passa para o nome");
        assert_eq!(t("xyz1"), Some(Tipo::Cabo), "Ethernet física de nome qualquer");
        assert_eq!(t("naoexiste"), None, "interface que o sistema não mostra: nome");
        // com o nome como alternativa: desconhecida continua desconhecida
        assert_eq!(tipo_com(&raiz, "xyz0"), Tipo::Desconhecida);
        assert_eq!(tipo_com(&raiz, "naoexiste"), Tipo::Desconhecida);
        assert_eq!(tipo_com(&raiz, "Wi-Fi"), Tipo::Wifi, "sem metadado, nome do Windows");
        assert_eq!(tipo_com(&raiz, "Conexão de Rede Sem Fio"), Tipo::Wifi);
        assert_eq!(tipo_com(&raiz, "Conexão Local"), Tipo::Cabo);
        std::fs::remove_dir_all(raiz).ok();
    }

    /// T-31 · CA-A1, CA-A2: anúncio padrão só em cabo e Wi-Fi; a não
    /// identificada só com IP de rede privada ou link-local.
    #[test]
    fn t31_so_interfaces_apropriadas_anunciam() {
        let i = |n: &str, ip: [u8; 4], t: Tipo| Interface::de_teste(n, ip, 24, t);
        let anuncia = |x: &Interface| anunciavel(x, &[]);
        assert!(anuncia(&i("eno1", [192, 168, 0, 5], Tipo::Cabo)));
        assert!(anuncia(&i("wlan0", [10, 1, 2, 3], Tipo::Wifi)));
        assert!(anuncia(&i("eno1", [203, 0, 113, 5], Tipo::Cabo)), "cabo anuncia mesmo com IP fixo público");
        for t in [Tipo::Virtual, Tipo::Celular] {
            assert!(!anuncia(&i("x", [192, 168, 0, 5], t)), "{t:?} não anuncia por padrão");
        }
        assert!(anuncia(&i("xyz0", [192, 168, 5, 5], Tipo::Desconhecida)));
        assert!(anuncia(&i("xyz0", [10, 5, 5, 5], Tipo::Desconhecida)));
        assert!(anuncia(&i("xyz0", [172, 20, 5, 5], Tipo::Desconhecida)));
        assert!(anuncia(&i("xyz0", [169, 254, 5, 5], Tipo::Desconhecida)), "link-local");
        assert!(!anuncia(&i("xyz0", [172, 40, 5, 5], Tipo::Desconhecida)), "172.32+ não é privado");
        assert!(!anuncia(&i("xyz0", [203, 0, 113, 5], Tipo::Desconhecida)), "IP público de interface não identificada");
        let mut desligada = i("eno1", [192, 168, 0, 5], Tipo::Cabo);
        desligada.ativa = false;
        assert!(!anuncia(&desligada));
        // o que de fato sai: só as apropriadas (mais o limitado)
        let lista = vec![
            i("eno1", [192, 168, 0, 5], Tipo::Cabo),
            i("docker0", [172, 17, 0, 1], Tipo::Virtual),
            i("rmnet0", [100, 64, 0, 5], Tipo::Celular),
        ];
        assert_eq!(
            alvos_de_anuncio(&lista, &[]),
            vec![
                (Ipv4Addr::new(192, 168, 0, 5), Ipv4Addr::new(192, 168, 0, 255)),
                (Ipv4Addr::UNSPECIFIED, Ipv4Addr::BROADCAST)
            ]
        );
    }

    /// T-32 · CA-A3, CA-A4: configuração explícita liberta VPN/virtual/celular
    /// pelo nome; o digitado sempre vale; o aprendido de interface virtual sem
    /// liberação é ignorado.
    #[test]
    fn t32_configuracao_explicita_para_vpn_e_virtuais() {
        let i = |n: &str, ip: [u8; 4], t: Tipo| Interface::de_teste(n, ip, 24, t);
        let lista = vec![i("eno1", [192, 168, 0, 5], Tipo::Cabo), i("tailscale0", [100, 100, 1, 1], Tipo::Virtual)];
        let liberada = vec!["Tailscale0".to_string()]; // maiúscula/minúscula não importa
        assert!(!anunciavel(&lista[1], &[]));
        assert!(anunciavel(&lista[1], &liberada));
        assert!(alvos_de_anuncio(&lista, &liberada)
            .contains(&(Ipv4Addr::new(100, 100, 1, 1), Ipv4Addr::new(100, 100, 1, 255))));
        assert!(!alvos_de_anuncio(&lista, &[]).iter().any(|(ip, _)| *ip == Ipv4Addr::new(100, 100, 1, 1)));
        // caminhos aprendidos
        assert!(caminho_aprendido_vale(Some(&lista), "192.168.0.9:47810", &[]), "cabo vale");
        assert!(!caminho_aprendido_vale(Some(&lista), "100.100.1.9:47810", &[]), "virtual sem liberação: ignora");
        assert!(caminho_aprendido_vale(Some(&lista), "100.100.1.9:47810", &liberada), "liberada: vale");
        assert!(
            caminho_aprendido_vale(Some(&lista), "10.77.0.9:47810", &[]),
            "fora das sub-redes conhecidas: segue o jogo"
        );
        assert!(caminho_aprendido_vale(None, "100.100.1.9:47810", &[]), "sem lista de interfaces: como antes");
    }

    /// T-33 · CA-L1, CA-L2, CA-L3: cabo direto com 169.254.x.x. Só o que o
    /// sistema marca como ponto a ponto sai da lista; link-local em Ethernet
    /// ou em Wi-Fi entra, anuncia, é alcançado e vira saída planejada.
    #[test]
    fn t33_cabo_direto_com_link_local() {
        assert!(aceita_na_lista(false, false), "link-local não é ponto a ponto");
        assert!(!aceita_na_lista(false, true), "PPP/VPN marcado pelo sistema sai");
        assert!(!aceita_na_lista(true, false), "loopback sai");
        // (o cabo não é o primeiro da lista: o desempate é pelo tipo, não pela ordem)
        let lista = vec![
            Interface::de_teste("wlan0", [169, 254, 7, 7], 16, Tipo::Wifi), // Wi-Fi sem DHCP
            Interface::de_teste("xyz0", [169, 254, 8, 8], 16, Tipo::Desconhecida),
            Interface::de_teste("eth0", [169, 254, 10, 5], 16, Tipo::Cabo),
        ];
        let alvos = alvos_de_anuncio(&lista, &[]);
        for ip in [[169, 254, 10, 5], [169, 254, 7, 7], [169, 254, 8, 8]] {
            assert!(alvos.contains(&(Ipv4Addr::from(ip), Ipv4Addr::new(169, 254, 255, 255))), "{ip:?}: {alvos:?}");
        }
        // o outro PC (169.254.20.20): alcançado pela interface de máscara mais específica; empate: a primeira
        let saida = saida_para(Some(&lista), "169.254.20.20:47810").unwrap();
        assert_eq!(saida.ip, Ipv4Addr::new(169, 254, 10, 5), "empate entre as três: a do cabo vai primeiro");
        // e o caminho entra na ordem como cabo
        let caminhos = vec![c("169.254.20.20:47810", 0, 100)];
        let ordem = ordenar(&caminhos, Some(&lista), 1_000);
        assert_eq!(ordem.len(), 1);
        assert_eq!(tipo_do_caminho(Some(&lista), "169.254.20.20:47810"), Tipo::Cabo);
    }

    /// T-26 (parte do anúncio) · CA-R4: interface nova, que sumiu ou com IP
    /// novo é uma mudança (dispara anúncio imediato); a ordem da lista não.
    #[test]
    fn t26_mudanca_nas_interfaces_dispara_anuncio_imediato() {
        let cabo = Interface::de_teste("eno1", [192, 168, 0, 5], 24, Tipo::Cabo);
        let wifi = Interface::de_teste("wlan0", [192, 168, 1, 5], 24, Tipo::Wifi);
        let antes = vec![cabo.clone(), wifi.clone()];
        assert!(!interfaces_mudaram(&antes, &[wifi.clone(), cabo.clone()]), "só a ordem mudou");
        assert!(interfaces_mudaram(std::slice::from_ref(&wifi), &antes), "cabo plugado");
        assert!(interfaces_mudaram(&antes, std::slice::from_ref(&wifi)), "cabo desconectado");
        let mut sem_sinal = cabo.clone();
        sem_sinal.ativa = false;
        assert!(interfaces_mudaram(&antes, &[sem_sinal, wifi.clone()]), "cabo sem sinal");
        let mut outro_ip = cabo.clone();
        outro_ip.ip = Ipv4Addr::new(192, 168, 0, 77);
        assert!(interfaces_mudaram(&antes, &[outro_ip, wifi]), "IP novo");
    }

    /// T-18 · CA-D5: se o sistema não conseguir listar as interfaces, o anúncio
    /// volta ao broadcast limitado de sempre e nenhum caminho é descartado.
    #[test]
    fn t18_lista_de_interfaces_que_falha_volta_ao_comportamento_antigo() {
        assert_eq!(lista_ou_nada(Err(std::io::Error::other("sem acesso"))), None);
        assert_eq!(lista_ou_nada(Ok(vec![])), Some(vec![]));
        assert_eq!(alvos_de_anuncio(&[], &[]), vec![(Ipv4Addr::UNSPECIFIED, Ipv4Addr::BROADCAST)]);
        let caminhos = vec![c("10.9.9.9:47810", 0, 1), c("10.8.8.8:47810", 0, 2)];
        assert_eq!(ordenar(&caminhos, None, 1_000).len(), 2, "sem lista: nada é descartado");
    }

    /// T-05 (parte do envio) · CA-D3: um erro numa interface não impede as
    /// outras e o mesmo erro é registrado uma vez só (até mudar).
    #[test]
    fn t05_erro_de_uma_interface_nao_impede_as_outras_e_registra_uma_vez() {
        let porta = std::net::UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let lista = vec![
            Interface::de_teste("quebrada", [203, 0, 113, 77], 24, Tipo::Cabo), // IP que este computador não tem
            Interface::de_teste("boa", [127, 0, 0, 1], 8, Tipo::Cabo),
        ];
        let erros = anunciar(&lista, &[], b"ola", porta);
        assert_eq!(erros.len(), 1, "{erros:?}");
        assert_eq!(erros[0].0, Ipv4Addr::new(203, 0, 113, 77));
        let mut ja = Vec::new();
        assert_eq!(erros_novos(&mut ja, erros.clone()).len(), 1, "a primeira vez entra no registro");
        assert!(erros_novos(&mut ja, erros.clone()).is_empty(), "repetido: não registra de novo");
        assert!(erros_novos(&mut ja, vec![]).is_empty());
        assert_eq!(erros_novos(&mut ja, erros).len(), 1, "voltou a dar erro depois de ter funcionado: registra");
    }
}
