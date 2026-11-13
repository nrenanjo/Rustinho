//! Identidade criptográfica deste caixa — protocolo 4, etapa 1 (D1, R-01, E5; D4).
//!
//! Cada computador tem um par de chaves próprio, gerado nele mesmo:
//! - **Ed25519** (`ring`): assina as aprovações, as retiradas e o anúncio. A
//!   chave pública é o identificador real do caixa (não um texto declarado,
//!   como o `dispositivo` do protocolo 3).
//! - **X25519** (`x25519-dalek`): fica reservada para a troca de chaves do
//!   canal autenticado (etapa 4). O desenho (`PROJETO_PROTOCOLO_4.md` §4.2)
//!   põe as duas chaves públicas no registro de caixas aprovados; ela só é
//!   usada se a biblioteca do canal (decisão A5) pedir chave estática.
//!
//! O [`codigo_de_pedido`] é o resumo curto das duas chaves públicas que o
//! gerente confere na tela do caixa novo antes de aprovar.
//!
//! **A chave privada nunca fica em claro no disco (D4).**
//! [`Identidade::guardar_em`] sela as duas partes privadas com Argon2id +
//! ChaCha20-Poly1305 — a mesma receita dos backups (`usuarios::argon2`,
//! parâmetros intocados) — num arquivo próprio, **fora do banco**: copiar o
//! banco não leva a chave. Qual senha sela (decisão A7) e onde o arquivo fica
//! (junto com o serviço de senhas do sistema, D4) são de quem chama: aqui a
//! senha e o caminho chegam como parâmetro.
//!
//! Submódulos: [`aprovados`] (etapa 2, registro assinado de caixas aprovados e
//! retirados) e [`anuncio`] (etapa 3, anúncio assinado sem nada da senha da
//! rede). Nada disto é chamado pela troca de dados ainda: ligar exige o canal
//! autenticado (A5) e a convivência com o protocolo 3 (A4), etapas 4 a 7.

use std::io::Write;
use std::path::Path;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::ChaCha20Poly1305;
use ring::signature::{self, Ed25519KeyPair, KeyPair};
use sha2::{Digest, Sha256};

use crate::erro::{Erro, Resultado};

pub mod anuncio;
pub mod aprovados;

/// Uma chave pública (Ed25519 ou X25519): 32 bytes.
pub type Chave = [u8; 32];
/// Uma assinatura Ed25519: 64 bytes.
pub type Assinatura = [u8; 64];

/// O contexto de cada assinatura. A marca vai na frente do que é assinado:
/// uma assinatura feita num contexto (uma aprovação) nunca vale em outro (um
/// anúncio). Cada marca termina em `\0`, então nenhuma é começo de outra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dominio {
    /// O caixa novo prova que tem a chave privada das chaves do pedido.
    Pedido,
    /// Um caixa aprovado (com o gerente) aprova outro.
    Aprovacao,
    /// Um caixa aprovado (com o gerente) retira outro.
    Retirada,
    /// O anúncio na rede local.
    Anuncio,
}

impl Dominio {
    fn marca(self) -> &'static [u8] {
        match self {
            Dominio::Pedido => b"rustinho/p4/pedido\0",
            Dominio::Aprovacao => b"rustinho/p4/caixa-aprovado\0",
            Dominio::Retirada => b"rustinho/p4/caixa-retirado\0",
            Dominio::Anuncio => b"rustinho/p4/anuncio\0",
        }
    }
}

/// Bytes secretos que são apagados da memória ao sair de cena (o melhor que
/// dá sem dependência nova: zera e impede o compilador de pular a escrita).
struct Segredo(Vec<u8>);

impl Drop for Segredo {
    fn drop(&mut self) {
        apagar(&mut self.0);
    }
}

fn apagar(b: &mut [u8]) {
    b.fill(0);
    std::hint::black_box(&b);
}

fn aleatorio<const N: usize>() -> Resultado<[u8; N]> {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).map_err(|e| Erro::Interno(format!("sem números aleatórios: {e}")))?;
    Ok(b)
}

// ---------------------------------------------------------------------------
// A identidade.
// ---------------------------------------------------------------------------

/// A identidade criptográfica de um caixa. Só existe na memória; no disco,
/// só selada ([`Identidade::guardar_em`]).
pub struct Identidade {
    /// Ed25519 em PKCS#8 (formato do `ring`), guardado para selar.
    pkcs8: Segredo,
    par: Ed25519KeyPair,
    publica: Chave,
    /// X25519 estática (apagada da memória ao sair de cena pelo próprio
    /// `x25519-dalek`).
    troca: x25519_dalek::StaticSecret,
    publica_troca: Chave,
}

impl std::fmt::Debug for Identidade {
    /// Nunca mostra a parte privada.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identidade").field("codigo", &self.codigo_de_pedido()).finish_non_exhaustive()
    }
}

impl Identidade {
    /// Gera uma identidade nova (na primeira abertura do caixa).
    pub fn gerar() -> Resultado<Identidade> {
        let rng = ring::rand::SystemRandom::new();
        let doc = Ed25519KeyPair::generate_pkcs8(&rng)
            .map_err(|_| Erro::Interno("não foi possível gerar a chave deste caixa".into()))?;
        let mut sorteio: [u8; 32] = aleatorio()?;
        let mut troca = Segredo(sorteio.to_vec());
        apagar(&mut sorteio);
        Identidade::montar(Segredo(doc.as_ref().to_vec()), &mut troca.0)
    }

    /// Monta a identidade a partir das partes privadas. `troca` (32 bytes) é
    /// copiada e quem chama apaga a sua cópia.
    fn montar(pkcs8: Segredo, troca: &mut [u8]) -> Resultado<Identidade> {
        let par = Ed25519KeyPair::from_pkcs8(&pkcs8.0)
            .map_err(|_| Erro::Interno("a chave deste caixa está em formato inválido".into()))?;
        let mut publica = [0u8; 32];
        publica.copy_from_slice(par.public_key().as_ref());
        let mut bytes = <[u8; 32]>::try_from(&*troca)
            .map_err(|_| Erro::Interno("a chave de troca deste caixa está em formato inválido".into()))?;
        let troca = x25519_dalek::StaticSecret::from(bytes);
        apagar(&mut bytes);
        let publica_troca = x25519_dalek::PublicKey::from(&troca).to_bytes();
        Ok(Identidade { pkcs8, par, publica, troca, publica_troca })
    }

    /// A chave pública Ed25519: o identificador deste caixa.
    pub fn publica(&self) -> Chave {
        self.publica
    }

    /// A chave pública X25519 (troca de chaves do canal, etapa 4).
    pub fn publica_troca(&self) -> Chave {
        self.publica_troca
    }

    /// O código que o gerente confere na tela (veja [`codigo_de_pedido`]).
    pub fn codigo_de_pedido(&self) -> String {
        codigo_de_pedido(&self.publica, &self.publica_troca)
    }

    /// Assina `mensagem` no contexto `dominio`.
    pub fn assinar(&self, dominio: Dominio, mensagem: &[u8]) -> Assinatura {
        let mut corpo = dominio.marca().to_vec();
        corpo.extend_from_slice(mensagem);
        let mut saida = [0u8; 64];
        saida.copy_from_slice(self.par.sign(&corpo).as_ref());
        saida
    }

    /// O segredo comum X25519 com a chave pública de outro caixa (para o
    /// canal da etapa 4; aqui só para provar que a chave guardada é a mesma).
    pub fn segredo_comum(&self, publica_do_outro: &Chave) -> [u8; 32] {
        self.troca.diffie_hellman(&x25519_dalek::PublicKey::from(*publica_do_outro)).to_bytes()
    }

    // -----------------------------------------------------------------------
    // Guarda selada (D4).
    // -----------------------------------------------------------------------

    /// Sela as partes privadas com `senha` (Argon2id + ChaCha20-Poly1305).
    ///
    /// Formato (versão 1): `RSTI`, versão (1 byte), chave pública Ed25519
    /// (32), chave pública X25519 (32), sal (16), nonce (12) e o corpo
    /// cifrado (PKCS#8 com o tamanho na frente + a X25519 privada, + etiqueta
    /// de 16). Tudo o que vem antes do corpo entra como dado associado: mudar
    /// qualquer byte do cabeçalho faz a abertura falhar.
    pub fn selar(&self, senha: &str) -> Resultado<Vec<u8>> {
        if senha.is_empty() {
            return Err(Erro::validacao("A identidade deste caixa não pode ser guardada sem senha."));
        }
        let tamanho = u16::try_from(self.pkcs8.0.len())
            .map_err(|_| Erro::Interno("a chave deste caixa está em formato inválido".into()))?;
        let sal: [u8; 16] = aleatorio()?;
        let nonce: [u8; 12] = aleatorio()?;
        let mut cabecalho = Vec::with_capacity(CABECALHO);
        cabecalho.extend_from_slice(MAGICA_SELO);
        cabecalho.push(VERSAO_SELO);
        cabecalho.extend_from_slice(&self.publica);
        cabecalho.extend_from_slice(&self.publica_troca);
        cabecalho.extend_from_slice(&sal);
        cabecalho.extend_from_slice(&nonce);
        let mut claro = Segredo(Vec::with_capacity(2 + self.pkcs8.0.len() + 32));
        claro.0.extend_from_slice(&tamanho.to_be_bytes());
        claro.0.extend_from_slice(&self.pkcs8.0);
        claro.0.extend_from_slice(self.troca.as_bytes());
        let chave = Segredo(chave_de_guarda(senha, &sal)?.to_vec());
        let cifra = ChaCha20Poly1305::new_from_slice(&chave.0).map_err(|_| Erro::Interno("chave inválida".into()))?;
        let corpo = cifra
            .encrypt(&nonce.into(), Payload { msg: &claro.0, aad: &cabecalho })
            .map_err(|_| Erro::Interno("não foi possível proteger a identidade deste caixa".into()))?;
        let mut selo = cabecalho;
        selo.extend_from_slice(&corpo);
        Ok(selo)
    }

    /// Abre um selo feito por [`Identidade::selar`]. Senha errada e selo
    /// mexido dão o mesmo erro (a etiqueta não diz qual dos dois foi).
    pub fn abrir_selo(selo: &[u8], senha: &str) -> Resultado<Identidade> {
        if senha.is_empty() {
            return Err(Erro::validacao("Digite a senha que protege a identidade deste caixa."));
        }
        if selo.len() < CABECALHO + 16 || selo[..4] != MAGICA_SELO[..] {
            return Err(Erro::validacao("O arquivo da identidade deste caixa não é reconhecido."));
        }
        if selo[4] != VERSAO_SELO {
            return Err(Erro::validacao(
                "O arquivo da identidade deste caixa é de outra versão do Rustinho. Atualize este caixa.",
            ));
        }
        let (cabecalho, corpo) = selo.split_at(CABECALHO);
        let sal = &cabecalho[69..85];
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&cabecalho[85..97]);
        let chave = Segredo(chave_de_guarda(senha, sal)?.to_vec());
        let cifra = ChaCha20Poly1305::new_from_slice(&chave.0).map_err(|_| Erro::Interno("chave inválida".into()))?;
        let claro = Segredo(cifra.decrypt(&nonce.into(), Payload { msg: corpo, aad: cabecalho }).map_err(|_| {
            Erro::validacao("A senha não confere, ou o arquivo da identidade deste caixa foi danificado.")
        })?);
        let invalido = || Erro::validacao("O arquivo da identidade deste caixa está danificado.");
        if claro.0.len() < 2 + 32 {
            return Err(invalido());
        }
        let tamanho = usize::from(u16::from_be_bytes([claro.0[0], claro.0[1]]));
        if claro.0.len() != 2 + tamanho + 32 {
            return Err(invalido());
        }
        let pkcs8 = Segredo(claro.0[2..2 + tamanho].to_vec());
        let mut troca = Segredo(claro.0[2 + tamanho..].to_vec());
        let id = Identidade::montar(pkcs8, &mut troca.0).map_err(|_| invalido())?;
        // O cabeçalho (autenticado) tem de bater com as chaves de dentro.
        if cabecalho[5..37] != id.publica[..] || cabecalho[37..69] != id.publica_troca[..] {
            return Err(invalido());
        }
        Ok(id)
    }

    /// Guarda a identidade selada num arquivo **novo** em `caminho` (fora do
    /// banco). Nunca substitui uma identidade que já está lá: perder a chave
    /// de um caixa aprovado obriga a aprovar de novo.
    pub fn guardar_em(&self, caminho: &Path, senha: &str) -> Resultado<()> {
        if caminho.exists() {
            return Err(Erro::validacao("Já existe uma identidade guardada neste computador; ela não é substituída."));
        }
        gravar(caminho, &self.selar(senha)?)
    }

    /// Abre a identidade guardada em `caminho`.
    pub fn abrir_de(caminho: &Path, senha: &str) -> Resultado<Identidade> {
        let selo = std::fs::read(caminho).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Erro::NaoEncontrado("Identidade deste caixa".into()),
            _ => Erro::Interno(format!("não foi possível ler a identidade deste caixa ({:?})", e.kind())),
        })?;
        Identidade::abrir_selo(&selo, senha)
    }

    /// Troca a senha que sela a identidade guardada em `caminho` (por exemplo,
    /// quando a senha escolhida na decisão A7 muda). Confere a senha atual e
    /// grava o selo novo no lugar do antigo de uma vez só.
    pub fn trocar_senha_em(caminho: &Path, senha_atual: &str, senha_nova: &str) -> Resultado<()> {
        let id = Identidade::abrir_de(caminho, senha_atual)?;
        gravar(caminho, &id.selar(senha_nova)?)
    }

    /// Só nos testes: as partes privadas (para conferir que não vão em claro
    /// para o disco nem para o banco).
    #[cfg(test)]
    pub(crate) fn privadas_para_teste(&self) -> (Vec<u8>, [u8; 32]) {
        (self.pkcs8.0.clone(), self.troca.to_bytes())
    }
}

/// O código de pedido de um par de chaves públicas (igual em qualquer caixa):
/// 12 caracteres em 3 grupos de 4 (ex.: `4F2K-9QXA-7MPC`), 60 bits do
/// resumo SHA-256 das duas chaves. Sem letras que se confundem (I, O, 0, 1).
/// Como cobre também a X25519, ninguém troca a chave do canal no caminho sem
/// mudar o código que o gerente confere.
pub fn codigo_de_pedido(publica: &Chave, publica_troca: &Chave) -> String {
    const ALFABETO: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut h = Sha256::new();
    h.update(b"rustinho/p4/codigo-de-pedido\0");
    h.update(publica);
    h.update(publica_troca);
    let resumo: [u8; 32] = h.finalize().into();
    let mut texto = String::with_capacity(14);
    for (i, b) in resumo.iter().take(12).enumerate() {
        if i > 0 && i % 4 == 0 {
            texto.push('-');
        }
        // 256 é múltiplo de 32: cada caractere é uniforme.
        texto.push(ALFABETO[usize::from(*b) % 32] as char);
    }
    texto
}

/// Confere uma assinatura Ed25519 feita no contexto `dominio`.
pub fn verificar(dominio: Dominio, publica: &Chave, mensagem: &[u8], assinatura: &Assinatura) -> bool {
    let mut corpo = dominio.marca().to_vec();
    corpo.extend_from_slice(mensagem);
    signature::UnparsedPublicKey::new(&signature::ED25519, publica).verify(&corpo, assinatura).is_ok()
}

/// Chave em hexadecimal minúsculo (como fica no banco).
pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Lê `N` bytes em hexadecimal (exatamente `2N` algarismos).
pub fn de_hex<const N: usize>(t: &str) -> Option<[u8; N]> {
    if t.len() != 2 * N || !t.is_ascii() {
        return None;
    }
    let mut saida = [0u8; N];
    for (i, b) in saida.iter_mut().enumerate() {
        *b = u8::from_str_radix(&t[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(saida)
}

// ---------------------------------------------------------------------------
// Guarda no disco.
// ---------------------------------------------------------------------------

const MAGICA_SELO: &[u8; 4] = b"RSTI";
const VERSAO_SELO: u8 = 1;
/// MAGICA (4) + versão (1) + duas chaves públicas (64) + sal (16) + nonce (12).
const CABECALHO: usize = 4 + 1 + 32 + 32 + 16 + 12;

/// A chave que sela: Argon2id com os parâmetros do projeto (os mesmos dos
/// backups e da senha dos usuários). Nenhum parâmetro muda aqui.
fn chave_de_guarda(senha: &str, sal: &[u8]) -> Resultado<[u8; 32]> {
    let mut k = [0u8; 32];
    crate::usuarios::argon2()
        .hash_password_into(senha.as_bytes(), sal, &mut k)
        .map_err(|e| Erro::Interno(format!("não foi possível proteger a identidade deste caixa: {e}")))?;
    Ok(k)
}

/// Grava `bytes` em `caminho` de uma vez só: arquivo temporário criado já só
/// do usuário (0600 no Linux), gravado até o disco, e trocado de nome por
/// cima. Uma queda no meio deixa o arquivo de antes inteiro.
fn gravar(caminho: &Path, bytes: &[u8]) -> Resultado<()> {
    let falha = |e: std::io::Error| {
        Erro::Interno(format!("não foi possível guardar a identidade deste caixa ({:?})", e.kind()))
    };
    let pasta = caminho.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(pasta).map_err(falha)?;
    let nome = caminho.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let temporario = pasta.join(format!(".{nome}.novo-{}", crate::tempo::novo_id()));
    let mut opcoes = std::fs::OpenOptions::new();
    opcoes.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opcoes.mode(0o600);
    }
    let gravou = opcoes.open(&temporario).and_then(|mut f| {
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&temporario, caminho)
    });
    if let Err(e) = gravou {
        let _ = std::fs::remove_file(&temporario);
        return Err(falha(e));
    }
    #[cfg(unix)]
    if let Ok(p) = std::fs::File::open(pasta) {
        let _ = p.sync_all();
    }
    crate::arquivos::so_do_usuario(caminho, "identidade do caixa");
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;

    fn pasta(nome: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("rustinho-p4-{nome}-{}", crate::tempo::novo_id()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn contem(palheiro: &[u8], agulha: &[u8]) -> bool {
        !agulha.is_empty() && palheiro.windows(agulha.len()).any(|j| j == agulha)
    }

    /// As partes privadas: o PKCS#8 inteiro, a semente Ed25519 de dentro dele
    /// (o `ring` usa o PKCS#8 v2: a semente são os 32 bytes depois do
    /// cabeçalho de 16) e a X25519.
    fn privadas(id: &Identidade) -> Vec<Vec<u8>> {
        let (pkcs8, troca) = id.privadas_para_teste();
        assert!(pkcs8.len() >= 48);
        vec![pkcs8.clone(), pkcs8[16..48].to_vec(), troca.to_vec()]
    }

    #[test]
    fn gera_identidades_diferentes_que_assinam_por_contexto() {
        let a = Identidade::gerar().unwrap();
        let b = Identidade::gerar().unwrap();
        assert_ne!(a.publica(), b.publica(), "cada caixa tem chave própria");
        assert_ne!(a.publica_troca(), b.publica_troca());
        assert_ne!(a.codigo_de_pedido(), b.codigo_de_pedido());
        let s = a.assinar(Dominio::Aprovacao, b"mensagem");
        assert!(verificar(Dominio::Aprovacao, &a.publica(), b"mensagem", &s));
        // Chave errada, mensagem errada e contexto errado não conferem.
        assert!(!verificar(Dominio::Aprovacao, &b.publica(), b"mensagem", &s));
        assert!(!verificar(Dominio::Aprovacao, &a.publica(), b"mensagens", &s));
        for outro in [Dominio::Pedido, Dominio::Retirada, Dominio::Anuncio] {
            assert!(!verificar(outro, &a.publica(), b"mensagem", &s), "{outro:?}");
        }
        // As duas metades X25519 chegam ao mesmo segredo.
        assert_eq!(a.segredo_comum(&b.publica_troca()), b.segredo_comum(&a.publica_troca()));
        // O Debug não mostra nada da parte privada.
        let texto = format!("{a:?}");
        assert!(texto.contains(&a.codigo_de_pedido()) && !texto.contains("pkcs8"));
    }

    #[test]
    fn codigo_de_pedido_estavel_curto_e_cobre_as_duas_chaves() {
        let a = Identidade::gerar().unwrap();
        let c = a.codigo_de_pedido();
        assert_eq!(c, codigo_de_pedido(&a.publica(), &a.publica_troca()), "o mesmo em qualquer caixa");
        assert_eq!(c.len(), 14, "12 caracteres e dois hífens: {c}");
        assert_eq!(c.matches('-').count(), 2);
        assert!(c.chars().all(|x| x == '-' || "ABCDEFGHJKLMNPQRSTUVWXYZ23456789".contains(x)), "{c}");
        // Trocar só a chave do canal muda o código.
        let outra = Identidade::gerar().unwrap();
        assert_ne!(c, codigo_de_pedido(&a.publica(), &outra.publica_troca()));
    }

    #[test]
    fn selo_abre_so_com_a_senha_certa_e_nao_leva_a_chave_em_claro() {
        let id = Identidade::gerar().unwrap();
        let selo = id.selar("senha-do-gerente").unwrap();
        for p in privadas(&id) {
            assert!(!contem(&selo, &p), "parte privada em claro no selo");
        }
        assert!(id.selar("").is_err(), "não sela sem senha");
        assert!(Identidade::abrir_selo(&selo, "senha-errada").is_err(), "senha errada não abre");
        assert!(Identidade::abrir_selo(&selo, "").is_err());
        let aberta = Identidade::abrir_selo(&selo, "senha-do-gerente").unwrap();
        assert_eq!(aberta.publica(), id.publica());
        assert_eq!(aberta.publica_troca(), id.publica_troca());
        // Dois selos da mesma identidade nunca são iguais (sal e nonce novos).
        assert_ne!(selo, id.selar("senha-do-gerente").unwrap());
        // Qualquer byte mexido (cabeçalho ou corpo) impede a abertura.
        for i in [0, 4, 5, 40, 70, 90, CABECALHO, selo.len() - 1] {
            let mut mexido = selo.clone();
            mexido[i] ^= 0x01;
            assert!(Identidade::abrir_selo(&mexido, "senha-do-gerente").is_err(), "byte {i} mexido");
        }
        assert!(Identidade::abrir_selo(&selo[..selo.len() - 1], "senha-do-gerente").is_err(), "cortado");
    }

    #[test]
    fn guardada_no_arquivo_sobrevive_a_reabrir() {
        let dir = pasta("reabrir");
        let caminho = dir.join("chaves").join("identidade-caixa");
        let (publica, troca, codigo, (pkcs8, segredo)) = {
            let id = Identidade::gerar().unwrap();
            id.guardar_em(&caminho, "senha-do-gerente").unwrap();
            (id.publica(), id.publica_troca(), id.codigo_de_pedido(), id.privadas_para_teste())
        };
        // "Reabrir o programa": só o arquivo e a senha.
        let id = Identidade::abrir_de(&caminho, "senha-do-gerente").unwrap();
        assert_eq!((id.publica(), id.publica_troca(), id.codigo_de_pedido()), (publica, troca, codigo));
        assert_eq!(id.privadas_para_teste(), (pkcs8, segredo), "a mesma chave privada");
        let s = id.assinar(Dominio::Anuncio, b"oi");
        assert!(verificar(Dominio::Anuncio, &publica, b"oi", &s), "assina como antes");
        // Senha errada: falha, e o arquivo continua igual.
        let antes = std::fs::read(&caminho).unwrap();
        assert!(Identidade::abrir_de(&caminho, "senha-errada").is_err());
        assert_eq!(std::fs::read(&caminho).unwrap(), antes);
        // Não substitui a identidade guardada.
        let outra = Identidade::gerar().unwrap();
        assert!(outra.guardar_em(&caminho, "senha-do-gerente").is_err(), "não substitui");
        assert_eq!(std::fs::read(&caminho).unwrap(), antes, "o arquivo ficou como estava");
        // Sem arquivo: "não encontrado" (não cria nada).
        assert!(matches!(Identidade::abrir_de(&dir.join("nada"), "x"), Err(Erro::NaoEncontrado(_))));
        assert!(!dir.join("nada").exists());
        // Só o arquivo final na pasta (nenhum temporário sobrando).
        let nomes: Vec<_> =
            std::fs::read_dir(caminho.parent().unwrap()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(nomes, vec![std::ffi::OsString::from("identidade-caixa")]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = std::fs::metadata(&caminho).unwrap().permissions().mode() & 0o777;
            assert_eq!(modo, 0o600, "só do usuário");
        }
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn trocar_a_senha_mantem_a_identidade() {
        let dir = pasta("trocar");
        let caminho = dir.join("identidade-caixa");
        let id = Identidade::gerar().unwrap();
        id.guardar_em(&caminho, "senha-velha").unwrap();
        assert!(Identidade::trocar_senha_em(&caminho, "errada", "senha-nova").is_err());
        assert!(Identidade::trocar_senha_em(&caminho, "senha-velha", "").is_err(), "não sela sem senha");
        Identidade::trocar_senha_em(&caminho, "senha-velha", "senha-nova").unwrap();
        assert!(Identidade::abrir_de(&caminho, "senha-velha").is_err(), "a senha velha não abre mais");
        assert_eq!(Identidade::abrir_de(&caminho, "senha-nova").unwrap().publica(), id.publica());
        std::fs::remove_dir_all(dir).ok();
    }

    /// O banco copiado não leva a chave: a identidade fica num arquivo fora
    /// do banco, e nada da parte privada (nem o selo) vai para o banco, mesmo
    /// com o caixa registrado como raiz da loja (etapa 2).
    #[test]
    fn banco_copiado_nao_leva_a_chave() {
        let dir = pasta("copia");
        let dados = dir.join("dados");
        let chaves = dir.join("chaves");
        let caminho_db = dados.join("rustinho.db");
        let caminho_id = chaves.join("identidade-caixa");
        let id = Identidade::gerar().unwrap();
        id.guardar_em(&caminho_id, "senha-do-gerente").unwrap();
        let selo = std::fs::read(&caminho_id).unwrap();
        {
            let b = Banco::abrir(&caminho_db).unwrap();
            let u = crate::faixas::para_testes::configurar(&b, "Caixa 1", 0);
            let loja = crate::loja::criar(&b.conn).unwrap();
            aprovados::registrar_raiz(&b.conn, &id, &loja, "Caixa 1", &u).unwrap();
            b.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
        }
        // "Copiar o banco": a pasta dos dados inteira, para outro lugar.
        let copia = dir.join("outro-computador");
        std::fs::create_dir_all(&copia).unwrap();
        for e in std::fs::read_dir(&dados).unwrap() {
            let e = e.unwrap();
            std::fs::copy(e.path(), copia.join(e.file_name())).unwrap();
        }
        let mut copiados = 0;
        for e in std::fs::read_dir(&copia).unwrap() {
            let bytes = std::fs::read(e.unwrap().path()).unwrap();
            copiados += 1;
            for p in privadas(&id) {
                assert!(!contem(&bytes, &p), "parte privada da chave na cópia dos dados");
            }
            assert!(!contem(&bytes, &selo[CABECALHO..]), "o selo da chave foi junto com os dados");
        }
        assert!(copiados >= 1);
        // O banco copiado abre, sabe quem é a raiz (dado público), mas não tem
        // como assinar por ela: a chave não está em lugar nenhum da cópia.
        let b = Banco::abrir(&copia.join("rustinho.db")).unwrap();
        let raizes: i64 = b
            .conn
            .query_row("SELECT COUNT(*) FROM caixa_aprovado WHERE chave = ?1", [hex(&id.publica())], |l| l.get(0))
            .unwrap();
        assert_eq!(raizes, 1, "o registro público foi junto");
        let texto: i64 = b
            .conn
            .query_row(
                "SELECT COUNT(*) FROM config WHERE valor LIKE '%' || ?1 || '%' OR valor LIKE '%' || ?2 || '%'",
                [hex(&privadas(&id)[1]), hex(&privadas(&id)[2])],
                |l| l.get(0),
            )
            .unwrap();
        assert_eq!(texto, 0, "nenhuma configuração guarda a chave privada");
        drop(b);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn hexadecimal_ida_e_volta() {
        let id = Identidade::gerar().unwrap();
        let t = hex(&id.publica());
        assert_eq!(t.len(), 64);
        assert_eq!(de_hex::<32>(&t), Some(id.publica()));
        assert_eq!(de_hex::<32>(&t[..62]), None);
        assert_eq!(de_hex::<2>("zz00"), None);
        assert_eq!(de_hex::<2>("é0"), None);
    }
}
