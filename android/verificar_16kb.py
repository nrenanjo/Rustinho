#!/usr/bin/env python3
"""Verifica um APK (ou uma pasta de bibliotecas) para aparelhos ARM64 com páginas de 16 KB.

Só a biblioteca padrão do Python; não depende do SDK do Android. Confere:
  1. ELF: cada `.so` de arm64-v8a, se todos os segmentos LOAD têm alinhamento >= 16 KB (0x4000).
  2. ZIP: cada `.so` guardado SEM compressão no APK, se os dados começam num múltiplo de 16 KB (o que o
     sistema exige para carregar direto do APK, com extractNativeLibs=false). Os comprimidos são extraídos
     na instalação, e aí só o ELF importa.
  3. Origem de cada biblioteca (do app, do Qt, do NDK), para separar o que se corrige no projeto do que depende
     de outra versão de dependência.

Uso:
  verificar_16kb.py <arquivo.apk>        # o APK
  verificar_16kb.py <pasta>              # as .so de uma pasta (por exemplo, a instalação do Qt para Android)
Saída: uma linha por biblioteca e um resumo; sai com 0 só se tudo estiver pronto para 16 KB.
"""

import os
import struct
import sys
import zipfile

PAGINA = 16 * 1024


def alinhamentos_load(dados):
    """Os alinhamentos (p_align) dos segmentos PT_LOAD de um ELF de 64 bits, little-endian."""
    if dados[:4] != b"\x7fELF" or dados[4] != 2 or dados[5] != 1:
        raise ValueError("não é ELF de 64 bits little-endian")
    e_phoff = struct.unpack_from("<Q", dados, 0x20)[0]
    e_phentsize, e_phnum = struct.unpack_from("<HH", dados, 0x36)
    lista = []
    for i in range(e_phnum):
        base = e_phoff + i * e_phentsize
        p_type = struct.unpack_from("<I", dados, base)[0]
        if p_type == 1:  # PT_LOAD
            lista.append(struct.unpack_from("<Q", dados, base + 0x30)[0])
    return lista


def origem(nome):
    n = os.path.basename(nome)
    if n.startswith("libRustinho"):
        return "app"
    if n.startswith("libc++_shared"):
        return "NDK"
    if "Qt6" in n or n.startswith("libplugins_") or n.startswith("libqml_"):
        return "Qt"
    if n.startswith(("libavcodec", "libavformat", "libavutil", "libswresample", "libswscale")):
        return "Qt-FFmpeg"  # o FFmpeg que o Qt Multimedia traz (a câmera)
    return "outra"


def do_apk(caminho):
    linhas = []
    with zipfile.ZipFile(caminho) as z:
        for info in z.infolist():
            if not (info.filename.startswith("lib/arm64-v8a/") and info.filename.endswith(".so")):
                continue
            dados = z.read(info)
            # Início dos dados no arquivo: cabeçalho local (30 bytes) + nome + extra.
            with open(caminho, "rb") as f:
                f.seek(info.header_offset + 26)
                n_nome, n_extra = struct.unpack("<HH", f.read(4))
            inicio = info.header_offset + 30 + n_nome + n_extra
            comprimido = info.compress_type != zipfile.ZIP_STORED
            linhas.append((info.filename, alinhamentos_load(dados), comprimido, inicio))
    return linhas


def da_pasta(pasta):
    linhas = []
    for raiz, _, arquivos in os.walk(pasta):
        for a in sorted(arquivos):
            if a.endswith(".so"):
                p = os.path.join(raiz, a)
                with open(p, "rb") as f:
                    dados = f.read()
                try:
                    linhas.append((os.path.relpath(p, pasta), alinhamentos_load(dados), None, None))
                except ValueError:
                    pass  # .so de outra arquitetura
    return linhas


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    alvo = sys.argv[1]
    linhas = da_pasta(alvo) if os.path.isdir(alvo) else do_apk(alvo)
    resumo = {}
    zip_ruim = 0
    for nome, aligns, comprimido, inicio in sorted(linhas):
        menor = min(aligns) if aligns else 0
        elf_ok = menor >= PAGINA
        zip_txt = ""
        if comprimido is not None:
            if comprimido:
                zip_txt = "comprimido (extraído na instalação)"
            else:
                zip_ok = inicio % PAGINA == 0
                zip_ruim += 0 if zip_ok else 1
                zip_txt = f"sem compressão, início {inicio} ({'alinhado a 16 KB' if zip_ok else 'NÃO alinhado a 16 KB'})"
        o = origem(nome)
        r = resumo.setdefault(o, [0, 0])
        r[0 if elf_ok else 1] += 1
        print(f"{'OK ' if elf_ok else '4KB'}  {o:9}  menor alinhamento LOAD 0x{menor:x}  {nome}  {zip_txt}")
    print()
    total_ruim = sum(r[1] for r in resumo.values())
    for o, (bons, ruins) in sorted(resumo.items()):
        print(f"{o}: {bons} prontas para 16 KB, {ruins} com alinhamento menor")
    if zip_ruim:
        print(f"ZIP: {zip_ruim} bibliotecas sem compressão fora do alinhamento de 16 KB")
    print("VEREDITO:", "pronto para páginas de 16 KB" if total_ruim == 0 and zip_ruim == 0 else "NÃO pronto para páginas de 16 KB")
    sys.exit(0 if total_ruim == 0 and zip_ruim == 0 else 1)


if __name__ == "__main__":
    main()
