#!/usr/bin/env python3
"""Gera os ícones do Rustinho a partir do PNG oficial da 0.9.18: identidade/planeta-roxo-crescente.png.

Fonte: nunca alterada. Conferida pelo SHA-256 abaixo antes de qualquer saída.
Regras: transparência, cores e proporções preservadas; só redimensiona (Lanczos) dentro de um
quadrado transparente (a fonte é 1129x1127; uma faixa de 1 px transparente completa o quadrado).
Sem fundo: o fundo para ícones que pedem um (ex.: camada de fundo do ícone adaptativo do Android)
é uma camada separada, não está aqui.
Uso (na raiz do projeto): python3 identidade/gerar_icones_planeta.py [--conferir]
"""
import hashlib
import os
import sys

from PIL import Image

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FONTE = os.path.join(RAIZ, "identidade", "planeta-roxo-crescente.png")
SHA_FONTE = "b971216718df9bfdbb823b023bf0fd20cb1f315ea4a915727915c59400b68acf"


def quadrado(im):
    lado = max(im.size)
    tela = Image.new("RGBA", (lado, lado), (0, 0, 0, 0))
    tela.paste(im, ((lado - im.size[0]) // 2, (lado - im.size[1]) // 2))
    return tela


def redim(base, n):
    return base.resize((n, n), Image.LANCZOS)


def saidas(base):
    """Caminho relativo à raiz -> imagem. Cada caminho é um recurso que o projeto já usa."""
    out = {}
    for n in (16, 22, 24, 32, 48, 64, 128, 256, 512):
        out[f"instalador/icones/hicolor/{n}x{n}/apps/rustinho.png"] = redim(base, n)
    out["instalador/rustinho.png"] = redim(base, 512)
    out["instalador/rustinho-256.png"] = redim(base, 256)
    out["app/icone/rustinho.png"] = redim(base, 256)
    out["android/rust/icone/rustinho.png"] = redim(base, 256)
    densidades = {"mdpi": 48, "hdpi": 72, "xhdpi": 96, "xxhdpi": 144, "xxxhdpi": 192}
    for d, n in densidades.items():
        out[f"android/pacote/res/mipmap-{d}/ic_launcher.png"] = redim(base, n)
    return out


def main():
    dados = open(FONTE, "rb").read()
    sha = hashlib.sha256(dados).hexdigest()
    if sha != SHA_FONTE:
        sys.exit(f"fonte alterada: {sha} (esperado {SHA_FONTE})")
    base = quadrado(Image.open(FONTE).convert("RGBA"))
    alvo = saidas(base)
    if "--conferir" in sys.argv:
        print(f"fonte ok ({sha[:16]}); {len(alvo)} arquivos de imagem; ICO com 7 tamanhos")
        return
    for rel, im in alvo.items():
        caminho = os.path.join(RAIZ, rel)
        os.makedirs(os.path.dirname(caminho), exist_ok=True)
        im.save(caminho, format="PNG", optimize=True)
    tamanhos = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    base.save(os.path.join(RAIZ, "instalador", "rustinho.ico"), format="ICO", sizes=tamanhos)
    print(f"gerados {len(alvo)} PNG e instalador/rustinho.ico")


if __name__ == "__main__":
    main()
