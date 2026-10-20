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

use std::net::{Ipv4Addr, UdpSocket};

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
        "docker", "virtual", "vmware", "vmnet", "vboxnet", "hyper-v", "vethernet", "tailscale", "zerotier", "loopback",
        "pseudo", "bluetooth", "tunnel", "vpn",
    ]) || comeca(&["veth", "br-", "virbr", "tun", "tap", "wg", "utun", "dummy", "lxc", "cni", "flannel", "podman"])
    {
        return Tipo::Virtual;
    }
    if comeca(&["rmnet", "ccmni", "wwan", "ppp", "usb0"]) || tem(&["celular", "cellular", "mobile", "móvel", "movel"]) {
        return Tipo::Celular;
    }
    if comeca(&["wlan", "wlp", "wlx", "wifi", "wl0", "wl1", "swlan", "p2p"])
        || tem(&["wi-fi", "wifi", "wireless", "sem fio", "wlan", "802.11"])
    {
        return Tipo::Wifi;
    }
    let cabo_linux = n.starts_with("eth")
        || (n.starts_with("en")
            && n[2..].chars().next().is_some_and(|c| c == 'p' || c == 'o' || c == 's' || c == 'x' || c.is_ascii_digit()));
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
        if i.is_loopback() || i.is_p2p() {
            continue;
        }
        let if_addrs::IfAddr::V4(v4) = &i.addr else { continue };
        let ativa = matches!(
            i.oper_status,
            if_addrs::IfOperStatus::Up | if_addrs::IfOperStatus::Unknown | if_addrs::IfOperStatus::Dormant
        ) && !v4.ip.is_unspecified();
        lista.push(Interface {
            tipo: tipo_no_sistema(&i.name),
            nome: i.name,
            ip: v4.ip,
            mascara: v4.netmask,
            ativa,
        });
    }
    Ok(lista)
}

/// No Linux e no Android o sistema sabe dizer se a interface é sem fio ou
/// virtual (mais confiável que o nome). Nos outros, só o nome.
fn tipo_no_sistema(nome: &str) -> Tipo {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let base = std::path::Path::new("/sys/class/net").join(nome);
        if base.join("wireless").exists() || base.join("phy80211").exists() {
            return Tipo::Wifi;
        }
        if let Ok(destino) = std::fs::read_link(&base) {
            if destino.to_string_lossy().contains("/virtual/") {
                return match classificar(nome) {
                    Tipo::Celular => Tipo::Celular,
                    _ => Tipo::Virtual,
                };
            }
        }
    }
    classificar(nome)
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
        .max_by_key(|i| u32::from(i.mascara).count_ones())
}

/// Para onde e de onde mandar o anúncio: (IP local para onde o socket é
/// preso, destino do broadcast). Um por interface ativa mais o broadcast
/// limitado de sempre (sem IP fixo).
pub fn alvos_de_anuncio(interfaces: &[Interface]) -> Vec<(Ipv4Addr, Ipv4Addr)> {
    let mut alvos: Vec<(Ipv4Addr, Ipv4Addr)> = Vec::new();
    for i in interfaces.iter().filter(|i| i.ativa) {
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
pub fn anunciar(interfaces: &[Interface], pacote: &[u8], porta: u16) -> Vec<(Ipv4Addr, String)> {
    let mut erros = Vec::new();
    for (de, para) in alvos_de_anuncio(interfaces) {
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
        Caminho { endereco: endereco.into(), digitado: false, visto_ms: visto, ultimo_ok_ms: ok, falhas: 0, proxima_ms: 0 }
    }

    /// T-01 · CA-R1: classificação pelo nome da interface.
    #[test]
    fn t01_classifica_pelo_nome() {
        for n in ["eth0", "eth1", "enp3s0", "eno1", "ens33", "enx00e04c680001", "Ethernet", "Ethernet 2", "Conexão local", "Local Area Connection"] {
            assert_eq!(classificar(n), Tipo::Cabo, "{n}");
        }
        for n in ["wlan0", "wlp2s0", "wlx0013ef", "Wi-Fi", "Wi-Fi 2", "Conexão de Rede Wireless", "Wireless Network Connection", "Rede sem fio"] {
            assert_eq!(classificar(n), Tipo::Wifi, "{n}");
        }
        for n in ["docker0", "veth12ab", "br-3f2a", "virbr0", "tun0", "tap0", "vEthernet (Default Switch)", "VirtualBox Host-Only Network", "VMware Network Adapter VMnet8", "tailscale0", "wg0", "dummy0"] {
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
        assert_eq!(broadcast_dirigido(Ipv4Addr::new(10, 0, 0, 1), Ipv4Addr::new(255, 0, 255, 0)), None, "máscara torta");
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
            alvos_de_anuncio(&lista),
            vec![
                (Ipv4Addr::new(192, 168, 0, 12), Ipv4Addr::new(192, 168, 0, 255)),
                (Ipv4Addr::new(10, 0, 5, 30), Ipv4Addr::new(10, 0, 255, 255)),
                (Ipv4Addr::new(169, 254, 7, 8), Ipv4Addr::new(169, 254, 255, 255)),
                (Ipv4Addr::UNSPECIFIED, Ipv4Addr::BROADCAST),
            ]
        );
        // sem nenhuma interface (lista vazia ou falha): continua o de antes
        assert_eq!(alvos_de_anuncio(&[]), vec![(Ipv4Addr::UNSPECIFIED, Ipv4Addr::BROADCAST)]);
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
        let erros = anunciar(&lista, b"ola", porta);
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
            c("192.168.1.7:47810", 9_000, 9_000),   // Wi-Fi
            c("192.168.0.7:47810", 1_000, 1_000),   // cabo
            c("172.17.0.7:47810", 8_000, 8_000),    // virtual
            c("100.64.0.9:47810", 7_000, 7_000),    // celular
            Caminho { digitado: true, ..c("10.50.0.7:47810", 500, 500) }, // digitado, fora das sub-redes (roteador)
        ];
        let ordem: Vec<String> = ordenar(&caminhos, Some(&lista), 10_000).into_iter().map(|c| c.endereco).collect();
        // só 3 por ciclo: cabo, digitado (desconhecida), Wi-Fi
        assert_eq!(ordem, ["192.168.0.7:47810", "10.50.0.7:47810", "192.168.1.7:47810"]);

        // na mesma classe, o que funcionou há menos tempo vai primeiro
        let dois_cabos = vec![c("192.168.0.7:47810", 1_000, 1), Caminho { digitado: true, ..c("192.168.0.8:47810", 5_000, 1) }];
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
        assert_eq!([espera_ms(0), espera_ms(1), espera_ms(2), espera_ms(3), espera_ms(40)], [0, 8_000, 16_000, 30_000, 30_000]);
        let mut cabo = c("192.168.0.7:47810", 1_000, 1_000);
        cabo.falhas = 1;
        cabo.proxima_ms = 20_000 + espera_ms(1);
        let wifi = c("192.168.1.7:47810", 500, 500);
        let lista = vec![
            Interface::de_teste("eno1", [192, 168, 0, 12], 24, Tipo::Cabo),
            Interface::de_teste("wlan0", [192, 168, 1, 40], 24, Tipo::Wifi),
        ];
        let todos = vec![cabo.clone(), wifi.clone()];
        let nomes = |agora| -> Vec<String> { ordenar(&todos, Some(&lista), agora).into_iter().map(|c| c.endereco).collect() };
        assert_eq!(nomes(20_000), ["192.168.1.7:47810"], "dentro da espera só o Wi-Fi");
        assert_eq!(nomes(27_999), ["192.168.1.7:47810"]);
        assert_eq!(nomes(28_000), ["192.168.0.7:47810", "192.168.1.7:47810"], "passada a espera o cabo volta a ser o primeiro");
    }
}
