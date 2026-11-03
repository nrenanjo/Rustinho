#!/usr/bin/env python3
"""Gera todos os ícones do Rustinho a partir de UM arquivo-fonte.

Fonte (nunca alterada por este script): identidade/rustinho-icone-original.png
Uso (na raiz do projeto):   python3 identidade/gerar_icones.py [--conferir]

Regras (pedido do dono do projeto, 20/10/2026):
  * só redimensiona (Lanczos): sem recorte, sem bordas, sem distorção, sem
    mudar cores nem estilo; o quadrado inteiro da fonte entra em cada saída;
  * a fonte é quadrada e opaca (os cantos fora do azulejo são pretos na
    própria imagem); isso é mantido como está. Uma variante com os cantos
    transparentes é só uma PROPOSTA (veja identidade/LEIA.md) e não é gerada
    aqui;
  * saídas determinísticas: rodar de novo gera os mesmos bytes.

Precisa do Pillow (pip install pillow).
"""
import hashlib
import io
import os
import struct
import sys

from PIL import Image

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FONTE = os.path.join(RAIZ, "identidade", "rustinho-icone-original.png")
SHA256_DA_FONTE = "b3cf72597083bc90ffbc197bae5d89e411b1328029a325c8465f2c36fc0542ca"

# (caminho relativo, lado em pixels)
PNGS = (
    # Programa (Linux, Windows e Android): ícone da janela, embutido no executável.
    [("app/icone/rustinho.png", 256)]
    # Instalador / AppImage (Linux) e instalador do Windows.
    + [("instalador/rustinho.png", 512), ("instalador/rustinho-256.png", 256)]
    # Tema de ícones do Linux (hicolor): atalhos, menu, barra de tarefas.
    + [(f"instalador/icones/hicolor/{n}x{n}/apps/rustinho.png", n) for n in (16, 22, 24, 32, 48, 64, 128, 256, 512)]
    # Android (APKs de 32 e 64 bits usam os mesmos recursos): ícone do aplicativo por densidade.
    + [
        ("android/pacote/res/mipmap-mdpi/ic_launcher.png", 48),
        ("android/pacote/res/mipmap-hdpi/ic_launcher.png", 72),
        ("android/pacote/res/mipmap-xhdpi/ic_launcher.png", 96),
        ("android/pacote/res/mipmap-xxhdpi/ic_launcher.png", 144),
        ("android/pacote/res/mipmap-xxxhdpi/ic_launcher.png", 192),
    ]
)
ICO = ("instalador/rustinho.ico", (16, 20, 24, 32, 40, 48, 64, 128, 256))


def reduzir(fonte, lado):
    return fonte.resize((lado, lado), Image.LANCZOS)


def png_bytes(im):
    b = io.BytesIO()
    im.save(b, "PNG", optimize=True)
    return b.getvalue()


def ico_bytes(fonte, lados):
    """ICO com uma imagem PNG por tamanho (aceito do Windows Vista em diante)."""
    imagens = [png_bytes(reduzir(fonte, n)) for n in lados]
    cab = struct.pack("<HHH", 0, 1, len(imagens))
    pos = 6 + 16 * len(imagens)
    entradas = b""
    for n, dados in zip(lados, imagens):
        entradas += struct.pack("<BBBBHHII", n % 256, n % 256, 0, 0, 1, 32, len(dados), pos)
        pos += len(dados)
    return cab + entradas + b"".join(imagens)


def main():
    conferir = "--conferir" in sys.argv
    bruto = open(FONTE, "rb").read()
    if hashlib.sha256(bruto).hexdigest() != SHA256_DA_FONTE:
        sys.exit("A fonte mudou (sha256 diferente). Atualize SHA256_DA_FONTE de propósito, não sem querer.")
    fonte = Image.open(io.BytesIO(bruto)).convert("RGBA")
    if fonte.width != fonte.height:
        sys.exit("A fonte precisa ser quadrada.")
    saidas = {rel: png_bytes(reduzir(fonte, n)) for rel, n in PNGS}
    saidas[ICO[0]] = ico_bytes(fonte, ICO[1])
    diferentes = 0
    for rel, dados in saidas.items():
        caminho = os.path.join(RAIZ, rel)
        atual = open(caminho, "rb").read() if os.path.exists(caminho) else None
        if atual != dados:
            diferentes += 1
            if conferir:
                print("DIFERENTE:", rel)
                continue
            os.makedirs(os.path.dirname(caminho), exist_ok=True)
            open(caminho, "wb").write(dados)
            print("gerado:", rel)
    if conferir:
        sys.exit(1 if diferentes else 0)
    print(f"{len(saidas)} arquivos conferidos, {diferentes} gravados.")


if __name__ == "__main__":
    main()
