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
  D. Fechar o aviso (toque) não fecha a orientação da guia: com a guia aberta em Configurações › Loja, o aviso da
     "Minha senha" ("Senha trocada."); o toque fecha só ele, antes dos 3 s, e a orientação continua.
  E. AV-1 (decisão de 15/11/2026): quando o aviso some, a tela volta à posição de antes, sem mudar a rolagem
     interna: em Configurações › Backup, rolado até "Fazer backup agora", o botão desce com o aviso e volta ao mesmo
     lugar, quando o aviso some sozinho e quando é fechado com um toque.
  F. AV-1: o foco do teclado e o que foi digitado continuam: na busca do Início, digitar, copiar (o aviso aparece),
     esperar o aviso sumir e continuar digitando: o texto junta as duas partes. O mesmo fechando o aviso com um toque
     e digitando logo em seguida (durante a volta de 140 ms).

    RUSTINHO_TELA=:88 python3 app/testes/telas_avisos_topo.py [pasta das fotos]
    RUSTINHO_AVISOS_JANELA=estreita   (a janela de 700 × 1000; o padrão é a larga, 1280 × 760)
    RUSTINHO_CASOS_CELULAR=1          (o desenho do celular, 360 × 780)
    RUSTINHO_AVISOS_CENARIOS=E,F      (só esses cenários; o padrão é todos)
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


def onde(r, pedaco, textos=None):
    textos = r.textos() if textos is None else textos
    return [(x, y) for x, y, t in textos if pedaco in t and not t.startswith(("GEO:", "TOQUE:", "RET:"))]


def geo_em(textos, nome):
    for _, _, t in textos:
        if t.startswith(f"GEO:{nome}:"):
            return tuple(int(v) for v in t.rsplit(":", 1)[1].split(","))
    return None


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


def cenario_c(placar, fotos, pasta):
    """C. O aviso do Início (3 s) e, logo em seguida, a guia: o aviso é da janela e atravessa a troca de tela; as
    posições saem de UMA leitura. Se a sequência passar dos 3 s (máquina lenta), a tentativa não vale e é repetida."""
    ok = placar.ok
    for tentativa in (1, 2, 3):
        r = placar.abrir(pasta, "TelaInicio", {}, fotos, f"C-guia-{JANELA}-{tentativa}")
        try:
            r.entrar("Ana")
            ajustar(r)
            if not r.tem_igual("Informar UF e CNPJ"):
                r.clicar("Fiscal", comeca=True)
            r.esperar(lambda: r.tem_igual("Informar UF e CNPJ"))
            time.sleep(1)
            pre = r.textos()
            copiar = [(x, y) for x, y, t in pre if t == "Copiar a busca"]
            r.clicar_em(*copiar[0])
            t0 = time.time()
            time.sleep(0.3)  # a descida (140 ms)
            informar = [(x, y) for x, y, t in r.textos() if nav.simples(t) == nav.simples("Informar UF e CNPJ")]
            r.clicar_em(*informar[0])
            # As posições só valem paradas: a orientação da guia, ao aparecer, empurra o aviso e a tela (140 ms). Duas
            # leituras seguidas iguais (o aviso e a seta no mesmo lugar).
            retrato, anterior = None, None
            while time.time() - t0 < 2.95:
                tx = r.textos()
                av, st = onde(r, VAZIA, tx), geo_em(tx, "setaDeRetorno")
                if onde(r, GUIA, tx) and av and st:
                    if (av[0], st) == anterior:
                        retrato = tx
                        break
                    anterior = (av[0], st)
            f = r.foto("-guia-e-aviso")
            if retrato is None:
                print(f"INSTRUMENTAÇÃO C: a sequência passou dos 3 s do aviso (tentativa {tentativa}, "
                      f"{time.time() - t0:.1f} s); repetindo", flush=True)
                continue
            g, t, seta = onde(r, GUIA, retrato)[0], onde(r, VAZIA, retrato)[0], geo_em(retrato, "setaDeRetorno")
            ok("C: a orientação da guia e o aviso aparecem juntos", True, f)
            ok("C: a orientação em cima, o aviso abaixo dela, a tela abaixo dos dois (nada sobreposto)",
               g[1] + 30 <= t[1] - MEIO_AVISO and t[1] + MEIO_AVISO <= seta[1],
               f"{f} guia y={g[1]} aviso y={t[1]} seta y={seta[1]}")
            return
        finally:
            r.fechar()
    ok("C: a sequência coube nos 3 s do aviso em alguma das 3 tentativas", False, "INSTRUMENTAÇÃO: máquina lenta")


SENHA_TROCADA = "Senha trocada."


def cenario_d(placar, fotos, pasta):
    """D. Com a guia aberta (Configurações › Loja, "informe a UF e o CNPJ"), um aviso na mesma tela ("Senha trocada.",
    da "Minha senha"): o toque fecha só o aviso, antes dos 3 s dele, e a orientação da guia continua. Começar na tela da
    guia tira a corrida contra os 3 s (a navegação ficava no meio)."""
    ok = placar.ok
    r = placar.abrir(pasta, "TelaInicio", {}, fotos, f"D-toque-{JANELA}")
    try:
        r.entrar("Ana")
        ajustar(r)
        if not r.tem_igual("Informar UF e CNPJ"):
            r.clicar("Fiscal", comeca=True)
        r.esperar(lambda: r.tem_igual("Informar UF e CNPJ"))
        time.sleep(1)
        r.clicar("Informar UF e CNPJ")
        if not ok("D: a guia abre em Configurações", r.esperar(lambda: onde(r, GUIA), 8), r.foto("-guia")):
            return
        time.sleep(1)
        # Os três campos da "Minha senha": o rótulo fica em cima do campo; o toque vai no campo, logo abaixo (antes,
        # rola até o rótulo e o campo ficarem à vista).
        def campo(rotulo):
            for _ in range(3):
                r.rolar_ate(rotulo)
                time.sleep(0.4)
                achados = [(x, y) for x, y, t in r.textos() if nav.simples(t) == nav.simples(rotulo)]
                for x, y in achados:
                    if y + 50 < r.altura():
                        return x, y + 28
                if not achados:
                    return None
                # O rótulo entrou na janela, mas o campo dele ficou cortado pela borda de baixo: o rolar_ate para
                # com o rótulo a até 30 px da borda, e o campo vem uns 50 px abaixo dele (integ6, janela estreita,
                # 15/11: "Nova senha" em y = 951 de 1000). Rola mais um pouco, como a pessoa faria, e confere de novo.
                if nav.CELULAR:
                    r.arrastar_para_cima(0.15)
                else:
                    xdo("mousemove", "300", str(r.altura() // 2))
                    for _ in range(2):
                        xdo("click", "5")
                    time.sleep(0.4)
            return None
        senha = nav.SENHA["Ana"]
        for rotulo, texto in (("Senha atual", senha), ("Nova senha", senha + "9"), ("Repita a nova senha", senha + "9")):
            pos = campo(rotulo)
            if not ok(f"D: achei o campo {rotulo}", pos is not None, r.foto("-campos")):
                return
            r.clicar_em(*pos)
            time.sleep(0.3)
            xdo("type", "--delay", "60", texto)
        r.rolar_ate("Trocar minha senha")
        time.sleep(0.4)
        botao = [(x, y) for x, y, t in r.textos() if t == "Trocar minha senha"]
        r.clicar_em(*botao[0])
        t0 = time.time()
        # Aqui a guia já está assentada: o aviso aparece e não se move. O toque vem logo que ele aparece.
        aviso = None
        while time.time() - t0 < 2.0:
            agora = onde(r, SENHA_TROCADA, r.textos())
            if agora:
                aviso = agora[0]
                break
        if not ok("D: o aviso da senha trocada aparece (com a guia aberta)", aviso is not None, r.foto("-aviso")):
            return
        r.clicar_em(*aviso)  # o toque no aviso
        # Acompanha até sumir. Cada retrato é do instante em que o atalho chega ao programa: se o aviso sumiu antes
        # dos 3 s dele (contados do clique que o fez aparecer), quem fechou foi o toque, e não o tempo.
        sumiu_em, depois = None, []
        while time.time() - t0 < 4.5:
            t_retrato = time.time()
            depois = r.textos()
            if not onde(r, SENHA_TROCADA, depois):
                sumiu_em = t_retrato - t0
                break
            time.sleep(0.1)
        f = r.foto("-depois-do-toque")
        pelo_toque = sumiu_em is not None and sumiu_em < 2.9
        ok("D: o toque fecha só o aviso (antes dos 3 s dele); a orientação da guia continua",
           pelo_toque and bool(onde(r, GUIA, depois)),
           f"{f} sumiu em {sumiu_em if sumiu_em is None else round(sumiu_em, 2)} s; guia={bool(onde(r, GUIA, depois))}")
    finally:
        r.fechar()


def valor_da_busca(r):
    """O texto digitado na busca do Início (o campo mostra o que foi digitado)."""
    for _, _, t in r.textos():
        if t.startswith("fil"):
            return t
    return ""


def cenario_e(placar, fotos, pasta):
    ok = placar.ok
    r = placar.abrir(pasta, "TelaConfiguracoes", {"secao": "backup"}, fotos, f"E-rolagem-{JANELA}")
    try:
        r.entrar("Ana")
        ajustar(r)
        r.esperar(lambda: r.tem("Fazer backup agora"), 15)
        r.rolar_ate("Fazer backup agora")
        # O primeiro backup muda o texto acima do botão ("Ainda não teve nenhum backup" → "Último backup hoje às …"):
        # um backup antes, para a posição de referência ser a do conteúdo que não muda mais.
        r.clicar("Fazer backup agora")
        r.esperar(lambda: onde(r, BACKUP), 20)
        r.esperar(lambda: not onde(r, BACKUP), 8)
        time.sleep(0.8)
        b0 = onde(r, "Fazer backup agora")[0][1]
        r.foto("-antes")
        for jeito in ("sozinho", "toque"):
            r.clicar("Fazer backup agora")
            apareceu = r.esperar(lambda: onde(r, BACKUP), 20)
            time.sleep(0.4)
            f = r.foto(f"-{jeito}-com-aviso")
            b1 = onde(r, "Fazer backup agora")[0][1] if apareceu else None
            if not ok(f"E ({jeito}): com o aviso, o botão desce pelo espaço do aviso", apareceu and b1 >= b0 + 20,
                      f"{f} botão {b0} → {b1}"):
                return
            if jeito == "toque":
                r.clicar(BACKUP)
            sumiu = r.esperar(lambda: not onde(r, BACKUP), 8)
            time.sleep(0.5)  # a volta de 140 ms
            f = r.foto(f"-{jeito}-depois")
            b2 = onde(r, "Fazer backup agora")[0][1]
            ok(f"E ({jeito}): o aviso some e o botão volta ao mesmo lugar (a rolagem interna não mudou)",
               sumiu and abs(b2 - b0) <= 2, f"{f} botão {b0} → {b1} → {b2}")
    finally:
        r.fechar()


def cenario_f(placar, fotos, pasta):
    ok = placar.ok
    r = placar.abrir(pasta, "TelaInicio", {}, fotos, f"F-foco-{JANELA}")
    try:
        r.entrar("Ana")
        ajustar(r)
        r.esperar(lambda: r.tem("Vender / Receber"), 15)
        time.sleep(1)
        busca = geo(r, "buscaDaInicio")
        r.clicar_em(busca[0] + busca[2] // 2, busca[1] + busca[3] // 2)
        time.sleep(0.4)
        xdo("type", "--delay", "70", "fil")
        time.sleep(0.4)
        # 1) O aviso some sozinho; depois, continuar digitando.
        r.clicar("Copiar a busca")
        apareceu = r.esperar(lambda: onde(r, "Copiado: fil"), 4)
        f = r.foto("-sozinho-com-aviso")
        if not ok("F (sozinho): o aviso aparece com a busca digitada", apareceu, f):
            return
        sumiu = r.esperar(lambda: not onde(r, "Copiado: fil"), 8)
        time.sleep(0.5)
        xdo("type", "--delay", "70", "tro")
        time.sleep(0.6)
        f = r.foto("-sozinho-depois")
        ok("F (sozinho): o aviso some, o foco continua na busca e o texto digitado fica (fil + tro)",
           sumiu and valor_da_busca(r) == "filtro", f"{f} busca={valor_da_busca(r)!r}")
        busca1 = geo(r, "buscaDaInicio")
        ok("F (sozinho): a busca voltou ao lugar de antes", abs(busca1[1] - busca[1]) <= 2,
           f"{f} busca y {busca[1]} → {busca1[1]}")
        # 2) Fechar com um toque e digitar logo em seguida (durante a volta de 140 ms).
        r.clicar("Copiar a busca")
        apareceu = r.esperar(lambda: onde(r, "Copiado: filtro"), 4)
        if not ok("F (toque): o aviso aparece de novo", apareceu, r.foto("-toque-com-aviso")):
            return
        r.clicar("Copiado: filtro")
        xdo("type", "--delay", "40", "s")
        time.sleep(0.8)
        f = r.foto("-toque-depois")
        ok("F (toque): fechado com o toque, o que se digita logo em seguida vai para a busca (filtros)",
           not onde(r, "Copiado: filtro") and valor_da_busca(r) == "filtros", f"{f} busca={valor_da_busca(r)!r}")
        busca2 = geo(r, "buscaDaInicio")
        ok("F (toque): a busca voltou ao lugar de antes", abs(busca2[1] - busca[1]) <= 2,
           f"{f} busca y {busca[1]} → {busca2[1]}")
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
        escolhidos = [c for c in os.environ.get("RUSTINHO_AVISOS_CENARIOS", "").split(",") if c]
        todos = (("A", cenario_a), ("B", cenario_b), ("C", cenario_c), ("D", cenario_d), ("E", cenario_e),
                 ("F", cenario_f))
        for nome, cenario in [t for t in todos if not escolhidos or t[0] in escolhidos]:
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
