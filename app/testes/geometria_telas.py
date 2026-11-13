#!/usr/bin/env python3
"""Teste de geometria pelo modo de captura (dev.3.3, passo 1.3).

Abre o Rustinho de verdade numa tela virtual (Xvfb), no ambiente neutro dos
outros roteiros, com uma loja montada pelo núcleo
(`nucleo/tests/lojas_de_teste.rs::montar_geometria`), e MEDE, pelas posições
que o próprio programa informa (--textos-da-tela --medir-toque), o que a
revisão visual da auditoria viu nas fotos:

  G1  Vender (1920 × 1080): Copiar e F11 não ficam um sobre o outro (H01);
  G2  Peças no celular (360 × 780): a câmera não fica sobre o Copiar (H01);
  G3  Entrada no celular com 5 usuários: tudo dentro da tela, ou a tela rola
      (H20, VIS33-06: título em y < 0 e "Eduarda" abaixo de 780);
  G4  Configurações › Simultaneidade › Fiscal no celular: nenhum texto mais largo que a tela
      (H21, VIS33-07), na primeira vista e rolando a seção até o fim (passo 7.2);
  G5  Entrada no celular com 5 usuários: escolhido o último nome, a senha e o
      "Entrar" ficam inteiros dentro da tela (H20; passo 7.2).

Em 4b253c6 as quatro são falhas ESPERADAS (H01 no passo 7.1; H20 e H21 no
passo 7.2): o roteiro sai com 0 e lista as falhas. Se uma esperada passar, ele
avisa e reprova (é hora de tirá-la da lista). O cenário H06 (pagamento com
crediário em 1366 × 768) entra no passo 7.2, com a venda montada pelo roteiro.

    python3 app/testes/geometria_telas.py [pasta para as fotos]

Precisa de: Xvfb, xdotool, ImageMagick (import), cargo.
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time

os.environ.setdefault("RUSTINHO_TELA", ":79")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_unidades as u  # noqa: E402

base = u.base
TELA = u.TELA

ESPERADAS = {
    "G1": "H01 (Copiar e F11 sobrepostos), passo 7.1",
    "G2": "H01 (câmera sobre o Copiar no celular), passo 7.1",
}
CELULAR = (360, 780)


def montar_loja(pasta):
    env = dict(os.environ, RUSTINHO_LOJA_TESTE=pasta)
    subprocess.run(
        ["cargo", "test", "-q", "-p", "rustinho-nucleo", "--test", "lojas_de_teste", "--",
         "--ignored", "--exact", "montar_geometria"],
        cwd=base.RAIZ, env=env, stdout=subprocess.DEVNULL, check=True,
    )


def esperar_janela():
    for _ in range(150):
        s = subprocess.run(["xdotool", "search", "--onlyvisible", "--class", "rustinho"],
                           env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
        if s.stdout.split():
            return
        time.sleep(0.2)


def toques(textos):
    """Áreas de toque (--medir-toque): (tipo, componente, classe, nome, x, y, largura, altura)."""
    lista = []
    for _, _, t in textos:
        if t.startswith("TOQUE:"):
            partes = t[len("TOQUE:"):].split("|")
            if len(partes) >= 6:
                x, y, w, h = (float(v) for v in partes[4].split(","))
                lista.append((partes[0], partes[1], partes[2], partes[3], x, y, w, h))
    return lista


def intersecao(a, b):
    """Área em comum de dois retângulos (x, y, largura, altura)."""
    dx = min(a[0] + a[2], b[0] + b[2]) - max(a[0], b[0])
    dy = min(a[1] + a[3], b[1] + b[3]) - max(a[1], b[1])
    return max(0, dx) * max(0, dy)


def clicar_rolando(rr, texto, altura):
    """Toca no texto exato; se ele estiver fora da tela (abaixo da dobra), rola antes."""
    for _ in range(12):
        alvo = [(x, y) for x, y, tx in rr.textos() if tx == texto and 40 < y < altura - 20]
        if alvo:
            px, py = rr.ponto(*alvo[0])
            base.xdo("mousemove", str(px), str(py), "click", "1")
            return True
        cx, cy = rr.ponto(180, altura // 2)
        base.xdo("mousemove", str(cx), str(cy), "click", "5")
        time.sleep(0.5)
    return False


def textos_com(textos, trecho):
    return [(x, y, t) for x, y, t in textos if trecho in t and not t.startswith(("GEO:", "RET:"))]


def main():
    for p in ("Xvfb", "xdotool", "import", "cargo"):
        if not shutil.which(p):
            sys.exit(f"Falta o programa {p}")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-geometria-")
    os.makedirs(fotos, exist_ok=True)
    modelo = tempfile.mkdtemp(prefix="rustinho-dados-geometria-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1920x1080x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    resultados = {}

    def conferir(codigo, nome, certo, detalhe=""):
        resultados[codigo] = (bool(certo), nome, detalhe)

    try:
        if xvfb.poll() is not None:
            sys.exit(f"Xvfb não abriu na tela {TELA}")
        montar_loja(modelo)

        # G1 — Vender no computador.
        rr = u.Rustinho(modelo, "TelaVender", {}, fotos, "g1_vender")
        try:
            esperar_janela()
            time.sleep(0.5)
            rr.redimensionar(1920, 1080)
            time.sleep(1)
            rr.entrar("Ana")
            time.sleep(3)
            rr.foto("")
            t = rr.textos()
            copiar = textos_com(t, "Copiar a busca")
            f11 = [(x, y, tx) for x, y, tx in t if tx == "F11"]
            sobre = [(c, f) for c in copiar for f in f11 if abs(c[0] - f[0]) < 24 and abs(c[1] - f[1]) < 12]
            conferir("G1", "Vender: Copiar e F11 não ficam um sobre o outro", copiar and f11 and not sobre,
                     f"Copiar {copiar[:1]} × F11 {f11[:1]}")
        finally:
            rr.fechar()

        # G3 (antes de entrar) e G2 — celular.
        rr = u.Rustinho(modelo, "TelaPecas", {}, fotos, "g2_g3_celular", celular=True)
        try:
            time.sleep(2)
            rr.foto("-entrada")
            t = rr.textos()
            nomes = [(x, y, tx) for x, y, tx in t if tx in ("Ana", "Bia", "Carlos", "Dani", "Eduarda")]
            fora = [(tx, y) for x, y, tx in nomes if y < 0 or y > CELULAR[1]]
            titulo = textos_com(t, "Quem está usando")
            fora += [("título", y) for _, y, _ in titulo if y < 0]
            conferir("G3", "entrada com 5 usuários: tudo dentro da tela", len(nomes) >= 5 and not fora,
                     f"{len(nomes)} nomes; fora da tela: {fora}")
            rr.entrar("Ana")
            time.sleep(3)
            rr.foto("-pecas")
            t = rr.textos()
            g = u.geos(t)
            busca = g.get("campoDeBusca")
            tq = toques(t)
            copiar = [r[4:] for r in tq if "Copiar a busca" in r[3]]
            # A câmera: a área de toque do botão de ação do Campo, à direita, sem ser Colar/Copiar.
            camera = [r[4:] for r in tq if r[0] == "mousearea" and r[2] == "QQuickRectangle" and r[3] == "-"
                      and busca and r[4] > busca[0] + busca[2] / 2 and r[6] <= 60
                      and busca[1] - 10 <= r[5] <= busca[1] + busca[3]]
            comum = intersecao(copiar[0], camera[0]) if copiar and camera else 0
            menor = min(copiar[0][2] * copiar[0][3], camera[0][2] * camera[0][3]) if copiar and camera else 1
            medido = bool(copiar and camera)
            conferir("G2", "Peças no celular: a câmera não fica sobre o Copiar", medido and comum / menor < 0.1,
                     f"Copiar {copiar[:1]} · câmera {camera[:1]} · área em comum {comum:.0f} px²"
                     + ("" if medido else " (NÃO MEDIDO: áreas de toque não achadas)"))
            if not medido:
                resultados["G2"] = (False, "Peças no celular: NÃO MEDIDO", "áreas de toque não achadas")
                ESPERADAS.pop("G2", None)
        finally:
            rr.fechar()

        # G5 — entrada no celular: o último nome, e a senha e o Entrar à vista (H20).
        rr = u.Rustinho(modelo, "TelaPecas", {}, fotos, "g5_entrada_celular", celular=True)
        try:
            time.sleep(2)
            alvo = [(x, y) for x, y, tx in rr.textos() if tx == "Eduarda"]
            if alvo:
                px, py = rr.ponto(*alvo[0])
                base.xdo("mousemove", str(px), str(py), "click", "1")
                time.sleep(1.5)
            rr.foto("")
            t = rr.textos()
            tq = toques(t)
            entrar = [r[4:] for r in tq if r[0] == "botao" and r[3].startswith("Entrar")]
            senha = [r[4:] for r in tq if r[0] == "campo"]
            dentro = [r for r in entrar + senha if r[1] >= 0 and r[1] + r[3] <= CELULAR[1] + 0.5]
            medido = bool(alvo and entrar and senha)
            conferir("G5", "entrada com 5 usuários: escolhida a Eduarda, a senha e o Entrar dentro da tela",
                     medido and len(dentro) == len(entrar + senha),
                     f"Entrar {[(round(r[1]), round(r[1] + r[3])) for r in entrar]}; "
                     f"senha {[(round(r[1]), round(r[1] + r[3])) for r in senha]} (tela {CELULAR[1]})"
                     + ("" if medido else " (NÃO MEDIDO)"))
            if not medido:
                ESPERADAS.pop("G5", None)
        finally:
            rr.fechar()

        # G4 — Configurações › Simultaneidade › Fiscal no celular. A tela calcula as abas do
        # gerente ao abrir: entra primeiro e vai pela Início (como a pessoa faz).
        rr = u.Rustinho(modelo, "TelaInicio", {}, fotos, "g4_fiscal_celular", celular=True)
        try:
            time.sleep(2)
            rr.entrar("Ana")
            time.sleep(3)
            clicar_rolando(rr, "Configurações", CELULAR[1])
            time.sleep(2.5)
            # Fiscal é subpílula de Simultaneidade (Configurações › Simultaneidade › Fiscal).
            clicar_rolando(rr, "Simultaneidade", CELULAR[1])
            time.sleep(1.5)
            clicar_rolando(rr, "Fiscal", CELULAR[1])
            time.sleep(2.5)
            rr.foto("")
            t = rr.textos()
            na_fiscal = bool(textos_com(t, "Emissão fiscal") or textos_com(t, "informe a UF"))
            largos = [(r[6][:40], round(r[0] + r[2])) for r in u.rets(t) if r[0] + r[2] > CELULAR[0] + 2]
            medidos = len(u.rets(t))
            # Rolando a seção até o fim (passo 7.2): o que fica abaixo da dobra também.
            for i in range(8):
                cx, cy = rr.ponto(180, CELULAR[1] // 2)
                for _ in range(4):
                    base.xdo("mousemove", str(cx), str(cy), "click", "5")
                time.sleep(0.6)
                t = rr.textos()
                medidos += len(u.rets(t))
                for r in u.rets(t):
                    if r[0] + r[2] > CELULAR[0] + 2 and 0 <= r[1] <= CELULAR[1]:
                        largos.append((r[6][:40], round(r[0] + r[2])))
            rr.foto("-rolada")
            largos = list(dict.fromkeys(largos))
            conferir("G4", "Fiscal no celular: nenhum texto mais largo que a tela", na_fiscal and not largos,
                     f"na seção Fiscal: {na_fiscal}; {medidos} textos medidos (rolando); passam da borda: {largos[:3]}")
            if not na_fiscal:
                resultados["G4"] = (False, "Fiscal no celular: NÃO MEDIDO (a seção Fiscal não abriu)", "")
                ESPERADAS.pop("G4", None)
        finally:
            rr.fechar()
    finally:
        shutil.rmtree(modelo, ignore_errors=True)
        xvfb.terminate()
        xvfb.wait()

    reprovado = False
    for codigo in sorted(resultados):
        certo, nome, detalhe = resultados[codigo]
        if certo and codigo in ESPERADAS:
            print(f"  ⚠ {codigo} {nome}: PASSOU, mas estava como falha esperada ({ESPERADAS[codigo]}) — tirar da lista")
            reprovado = True
        elif certo:
            print(f"  ✔ {codigo} {nome}")
        elif codigo in ESPERADAS:
            print(f"  ✘ {codigo} {nome} — falha ESPERADA: {ESPERADAS[codigo]} · {detalhe}")
        else:
            print(f"  ✘ {codigo} {nome} — {detalhe}")
            reprovado = True
    print(f"\n{sum(1 for v in resultados.values() if v[0])} de {len(resultados)} certas; fotos em {fotos}")
    sys.exit(1 if reprovado or len(resultados) != 5 else 0)


if __name__ == "__main__":
    main()
