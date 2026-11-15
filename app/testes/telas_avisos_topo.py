#!/usr/bin/env python3
"""Regra do topo para as notificações globais (auditoria de 31/10, bloco 3; diretriz de 15/11/2026): o aviso rápido
(`Aviso.qml`, de `janela.avisar`) aparece NO ALTO, reserva o próprio espaço (a tela desce, nada é coberto) e convive
com a orientação da guia e com as notificações persistentes do Início.

Com o Rustinho de verdade numa tela virtual, mede as posições pela leitura da tela (centro dos textos e as
geometrias com nome: a busca do Início e a seta de retorno):
  A. Início: "Copiar a busca" com a busca vazia → o aviso no alto, acima da busca; a tela desce pelo espaço dele;
     as notificações persistentes continuam abaixo da busca; quando o aviso some, tudo volta ao lugar.
  B. Configurações › Backup: "Fazer backup agora" → o aviso acima da seta de retorno, sem cobri-la.
  C. A orientação da guia e o aviso juntos (o aviso do Início e, logo em seguida, a guia): a orientação em cima, o
     aviso abaixo dela, a tela abaixo dos dois.
  D. Fechar o aviso (toque) não fecha a orientação da guia.

    RUSTINHO_TELA=:88 python3 app/testes/telas_avisos_topo.py [pasta das fotos]
    RUSTINHO_AVISOS_JANELA=estreita   (a janela de 700 × 1000; o padrão é a larga, 1280 × 760)
    RUSTINHO_CASOS_CELULAR=1          (o desenho do celular, 360 × 780)
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_navegacao_casos as nav  # noqa: E402
from telas_navegacao_casos import PROGRAMA, TELA, Placar, ler, montar_loja, por_fora, xdo  # noqa: E402

JANELA = os.environ.get("RUSTINHO_AVISOS_JANELA", "larga")
VAZIA = "nada para copiar"
BACKUP = "Backup feito e conferido."
GUIA = "Para continuar, informe a UF e o CNPJ da loja"
MEIO_AVISO = 23  # metade da altura do aviso de uma linha (texto + 26 px)


def geo(r, nome):
    for _, _, t in r.textos():
        if t.startswith(f"GEO:{nome}:"):
            return tuple(int(v) for v in t.rsplit(":", 1)[1].split(","))
    return None


def onde(r, pedaco):
    return [(x, y) for x, y, t in r.textos() if pedaco in t and not t.startswith(("GEO:", "TOQUE:", "RET:"))]


def ajustar(r):
    if JANELA == "estreita" and not nav.CELULAR:
        s = subprocess.run(["xdotool", "search", "--onlyvisible", "--class", "rustinho"],
                           env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
        for j in s.stdout.split():
            xdo("windowsize", j, "700", "1000")
        r._altura = 1000
        time.sleep(1.5)


def loja(pasta):
    montar_loja(pasta)
    if not ler(pasta, "SELECT COUNT(*) FROM compra WHERE xml <> ''")[0][0]:
        por_fora(pasta, "UPDATE compra SET xml = '<nfe/>' WHERE id = (SELECT id FROM compra LIMIT 1)")


def cenario_a(placar, fotos, pasta):
    ok = placar.ok
    r = placar.abrir(pasta, "TelaInicio", {}, fotos, f"A-inicio-{JANELA}")
    try:
        r.entrar("Ana")
        ajustar(r)
        r.esperar(lambda: r.tem("Vender / Receber"), 15)
        time.sleep(1)
        busca0 = geo(r, "buscaDaInicio")
        r.foto("-antes")
        r.clicar("Copiar a busca")
        apareceu = r.esperar(lambda: onde(r, VAZIA), 4)
        time.sleep(0.4)  # a animação de 140 ms das telas
        f = r.foto("-com-aviso")
        if not ok("A: o aviso rápido aparece", apareceu, f):
            return
        t = onde(r, VAZIA)[0]
        busca1 = geo(r, "buscaDaInicio")
        ok("A: o aviso fica no alto (acima da busca, no terço de cima da janela)",
           t[1] < busca1[1] and t[1] < r.altura() * 0.35, f"{f} aviso y={t[1]} busca y={busca1[1]} altura={r.altura()}")
        ok("A: reserva o próprio espaço: a tela desce e nada fica coberto",
           busca1[1] >= busca0[1] + 20 and t[1] + MEIO_AVISO <= busca1[1], f"{f} busca {busca0[1]} → {busca1[1]}; aviso y={t[1]}")
        # A notificação Fiscal (minimizada: o quadrado com o nome "Fiscal"; aberta: o título "Fiscal").
        pers = [(x, y) for x, y, t in r.textos() if t == "Fiscal"]
        ok("A: as notificações persistentes continuam abaixo da busca", bool(pers) and pers[0][1] > busca1[1],
           f"{f} persistente={pers[:1]} busca y={busca1[1]}")
        sumiu = r.esperar(lambda: not onde(r, VAZIA), 8)
        time.sleep(0.5)
        busca2 = geo(r, "buscaDaInicio")
        ok("A: quando o aviso some, a tela volta ao lugar", sumiu and abs(busca2[1] - busca0[1]) <= 2,
           f"{r.foto('-depois')} busca {busca0[1]} → {busca2[1]}")
    finally:
        r.fechar()


def cenario_b(placar, fotos, pasta):
    ok = placar.ok
    r = placar.abrir(pasta, "TelaConfiguracoes", {"secao": "backup"}, fotos, f"B-backup-{JANELA}")
    try:
        r.entrar("Ana")
        ajustar(r)
        r.esperar(lambda: r.tem("Fazer backup agora"), 15)
        r.rolar_ate("Fazer backup agora")
        seta0 = geo(r, "setaDeRetorno")
        r.clicar("Fazer backup agora")
        apareceu = r.esperar(lambda: onde(r, BACKUP), 20)
        time.sleep(0.4)
        f = r.foto("-com-aviso")
        if not ok("B: o aviso do backup aparece", apareceu, f):
            return
        t = onde(r, BACKUP)[0]
        seta1 = geo(r, "setaDeRetorno")
        ok("B: o aviso fica acima da seta de retorno, sem cobri-la", t[1] + MEIO_AVISO <= seta1[1],
           f"{f} aviso y={t[1]} seta y={seta1[1]}")
        ok("B: a tela desceu pelo espaço do aviso", seta1[1] >= seta0[1] + 20, f"{f} seta {seta0[1]} → {seta1[1]}")
    finally:
        r.fechar()


def cenario_c_d(placar, fotos, pasta):
    ok = placar.ok
    r = placar.abrir(pasta, "TelaInicio", {}, fotos, f"C-guia-{JANELA}")
    try:
        r.entrar("Ana")
        ajustar(r)
        if not r.tem_igual("Informar UF e CNPJ"):
            r.clicar("Fiscal", comeca=True)
        r.esperar(lambda: r.tem_igual("Informar UF e CNPJ"))
        time.sleep(1)
        # O aviso (3 s) e, logo em seguida, a guia: o aviso é da janela e atravessa a troca de tela.
        r.clicar("Copiar a busca")
        r.clicar("Informar UF e CNPJ")
        guia = r.esperar(lambda: onde(r, GUIA), 3)
        apareceu = bool(onde(r, VAZIA))
        time.sleep(0.3)
        f = r.foto("-guia-e-aviso")
        if not ok("C: a orientação da guia e o aviso aparecem juntos", guia and apareceu and onde(r, VAZIA), f):
            return
        g, t, seta = onde(r, GUIA)[0], onde(r, VAZIA)[0], geo(r, "setaDeRetorno")
        ok("C: a orientação em cima, o aviso abaixo dela, a tela abaixo dos dois (nada sobreposto)",
           g[1] + 30 <= t[1] - MEIO_AVISO and t[1] + MEIO_AVISO <= seta[1],
           f"{f} guia y={g[1]} aviso y={t[1]} seta y={seta[1]}")
        r.clicar("A busca está vazia: nada para copiar.")  # D: o toque fecha o aviso
        fechou = r.esperar(lambda: not onde(r, VAZIA), 4)
        f = r.foto("-depois-do-toque")
        ok("D: o toque fecha só o aviso; a orientação da guia continua", fechou and onde(r, GUIA), f)
    finally:
        r.fechar()


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-avisos-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = Placar()
    print(f"janela: {'celular 360 × 780' if nav.CELULAR else JANELA}", flush=True)
    try:
        for nome, cenario in (("A", cenario_a), ("B", cenario_b), ("C e D", cenario_c_d)):
            print(f"--- {nome}", flush=True)
            pasta = tempfile.mkdtemp(prefix="rustinho-loja-avisos-")
            try:
                loja(pasta)
                cenario(placar, fotos, pasta)
            except Exception as e:
                placar.ok(f"{nome} chegou ao fim ({e})", False)
            finally:
                shutil.rmtree(pasta, ignore_errors=True)
        erros = []
        for log in placar.logs:
            with open(log, errors="replace") as fh:
                erros += [ln.strip() for ln in fh if "TypeError" in ln or "ReferenceError" in ln]
        placar.ok("nenhum TypeError/ReferenceError nos registros", not erros, "; ".join(erros[:3]))
    finally:
        xvfb.terminate()
        try:
            xvfb.wait(10)
        except subprocess.TimeoutExpired:
            xvfb.kill()
    print(f"RESULTADO: {placar.certos} certos, {placar.errados} errados")
    print("TUDO CERTO" if placar.errados == 0 else "HOUVE FALHAS")
    sys.exit(0 if placar.errados == 0 else 1)


if __name__ == "__main__":
    nav.TELA = TELA
    main()
