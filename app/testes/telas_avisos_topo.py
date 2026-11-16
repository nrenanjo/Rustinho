#!/usr/bin/env python3
"""Regra do topo para as notificações globais (auditoria de 31/10, bloco 3; diretriz de 15/11/2026): o aviso rápido
(`Aviso.qml`, de `janela.avisar`) aparece NO ALTO, reserva o próprio espaço (a tela desce, nada é coberto) e convive
com a orientação da guia e com as notificações persistentes do Início.

Com o Rustinho de verdade numa tela virtual, mede as posições pela leitura da tela (centro dos textos e as
geometrias com nome: a busca do Início e a seta de retorno):
  A. Início: "Copiar a busca" com a busca vazia → o aviso no alto, acima da busca; a tela desce pelo espaço dele;
     as notificações persistentes continuam abaixo da busca; quando o aviso some, tudo volta ao lugar.
  B. Configurações › Backup: "Fazer backup agora" → o aviso acima da seta de retorno, sem cobri-la.
  C. O aviso do Início atravessa a troca de tela (o aviso e, logo em seguida, a guia) e aparece junto da orientação
     da guia. As posições passaram para o D (O-1 da revisão do VIS2/AV-1).
  D. Fechar o aviso (toque) não fecha a orientação da guia: com a guia aberta em Configurações › Loja, o aviso da
     "Minha senha" ("Senha trocada."); o toque fecha só ele, bem antes do tempo dele (medido num controle sem toque, na
     mesma rodada), e a orientação continua. Com o aviso na tela e a guia parada: a orientação em cima, o aviso
     abaixo dela, a tela abaixo dos dois.
  E. AV-1 (decisão de 15/11/2026): quando o aviso some, a tela volta à posição de antes, sem mudar a rolagem
     interna: em Configurações › Backup, com a página ROLADA (o botão "Fazer backup agora" sobe pelo menos 40 px), o
     botão desce com o aviso e volta ao mesmo lugar, quando o aviso some sozinho e quando é fechado com um toque.
  F. AV-1: o foco do teclado e o que foi digitado continuam: na busca do Início, digitar, copiar (o aviso aparece),
     esperar o aviso sumir e continuar digitando: o texto junta as duas partes. O mesmo fechando o aviso com um toque
     e digitando logo em seguida (durante a volta de 140 ms).

    RUSTINHO_TELA=:88 python3 app/testes/telas_avisos_topo.py [pasta das fotos]
    RUSTINHO_AVISOS_JANELA=estreita   (a janela de 700 × 1000; o padrão é a larga, 1280 × 760)
    RUSTINHO_CASOS_CELULAR=1          (o desenho do celular, 360 × 780)
    RUSTINHO_AVISOS_CENARIOS=E,F      (só esses cenários; o padrão é todos)
    RUSTINHO_AVISOS_SEM_TOQUE=1       (prova do critério do D: o roteiro não toca no aviso, e o D tem de reprovar)
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
# E: o botão tem 42 px de desenho (Botao.qml; no celular, 48 de toque) com o texto no meio; 14 px abaixo do meio do
# texto ainda é o botão. A faixa fixa das abas termina 38 px abaixo do texto da aba "Backup" (janela larga: 237 → 275).
ABAIXO_DO_MEIO = 14
FAIXA_ABAIXO_DA_ABA = 38


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


def arrastar_para_cima(fracao):
    """No celular: um arrasto curto para cima (x = 20, dentro da rolagem), sem impulso. Próprio deste roteiro, para não
    depender da versão do telas_navegacao_casos.py da frente."""
    largura, altura = nav.CELULAR_DP
    y0 = int(altura * 0.75)
    y1 = int(y0 - altura * fracao)
    xdo("mousemove", "20", str(y0))
    xdo("mousedown", "1")
    for i in range(1, 9):
        xdo("mousemove", "20", str(y0 - (y0 - y1) * i // 8))
        time.sleep(0.03)
    time.sleep(0.15)
    xdo("mouseup", "1")
    time.sleep(0.8)


def parado(r, texto, vezes=12):
    """A posição do texto quando ela para (duas leituras seguidas iguais): a tela pode estar descendo ou voltando com
    o aviso (a animação fica lenta com a guia aberta, no desenho por software). None se não parar ou não houver."""
    anterior = None
    for _ in range(vezes):
        p = onde(r, texto)
        if p and p[0] == anterior:
            return p[0]
        anterior = p[0] if p else None
        time.sleep(0.25)
    return None


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
            # O-1 da revisão do VIS2/AV-1: as POSIÇÕES (a orientação em cima, o aviso abaixo, a tela abaixo dos dois)
            # passaram para o D, onde a guia já está parada quando o aviso aparece (sem corrida contra os 3 s). Aqui fica
            # só o que é do C: o aviso do Início atravessa a troca de tela e aparece junto da orientação da guia, numa
            # leitura. Se não couber nos 3 s do aviso, a tentativa não vale (e, nas três, o C fica INCONCLUSIVO).
            juntos = None
            while time.time() - t0 < 2.95:
                tx = r.textos()
                if onde(r, GUIA, tx) and onde(r, VAZIA, tx):
                    juntos = tx
                    break
            f = r.foto("-guia-e-aviso")
            if juntos is None:
                print(f"INSTRUMENTAÇÃO C: a sequência passou dos 3 s do aviso (tentativa {tentativa}, "
                      f"{time.time() - t0:.1f} s); repetindo", flush=True)
                continue
            ok("C: o aviso do Início atravessa a troca de tela e aparece junto da orientação da guia", True, f)
            return
        finally:
            r.fechar()
    ok("C: a sequência coube nos 3 s do aviso em alguma das 3 tentativas", False, "INSTRUMENTAÇÃO: máquina lenta")


SENHA_TROCADA = "Senha trocada."


def cenario_d(placar, fotos, pasta):
    """D. Com a guia aberta (Configurações › Loja, "informe a UF e o CNPJ"), um aviso na mesma tela ("Senha trocada.",
    da "Minha senha"): o toque fecha só o aviso, bem antes do tempo dele (medido num controle sem toque, na mesma
    rodada: CI-1), e a orientação da guia continua. Começar na tela da guia tira a corrida contra os 3 s (a navegação
    ficava no meio)."""
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
                # A posição só vale parada: depois de um aviso, a tela volta ao lugar (e um toque numa posição velha
                # caía na pílula "Sobre", no celular).
                p = parado(r, rotulo) if any(nav.simples(t) == nav.simples(rotulo) for _, _, t in r.textos()) else None
                achados = [(x, y) for x, y, t in r.textos() if nav.simples(t) == nav.simples(rotulo)] if p else []
                for x, y in achados:
                    if y + 50 < r.altura():
                        return x, y + 28
                if not achados:
                    return None
                # O rótulo entrou na janela, mas o campo dele ficou cortado pela borda de baixo: o rolar_ate para
                # com o rótulo a até 30 px da borda, e o campo vem uns 50 px abaixo dele (integ6, janela estreita,
                # 15/11: "Nova senha" em y = 951 de 1000). Rola mais um pouco, como a pessoa faria, e confere de novo.
                if nav.CELULAR:
                    arrastar_para_cima(0.15)
                else:
                    xdo("mousemove", "300", str(r.altura() // 2))
                    for _ in range(2):
                        xdo("click", "5")
                    time.sleep(0.4)
            return None
        senha = nav.SENHA["Ana"]

        def trocar(atual, nova, quem):
            """Preenche a "Minha senha" e clica em "Trocar minha senha"; devolve a hora do clique."""
            for rotulo, texto in (("Senha atual", atual), ("Nova senha", nova), ("Repita a nova senha", nova)):
                pos = campo(rotulo)
                if not ok(f"D ({quem}): achei o campo {rotulo}", pos is not None, r.foto(f"-campos-{quem}")):
                    return None
                r.clicar_em(*pos)
                time.sleep(0.3)
                xdo("type", "--delay", "60", texto)
            r.rolar_ate("Trocar minha senha")
            time.sleep(0.4)
            botao = [(x, y) for x, y, t in r.textos() if t == "Trocar minha senha"]
            r.clicar_em(*botao[0])
            return time.time()

        def ver_aviso(t0, limite=8.0):
            """O aviso e quando foi visto pela 1ª vez. A troca da senha leva tempo (a senha é guardada com uma conta
            lenta de propósito), e na máquina da CI o aviso apareceu depois de 2 s: o prazo é de 8 s."""
            while time.time() - t0 < limite:
                agora = onde(r, SENHA_TROCADA, r.textos())
                if agora:
                    return agora[0], time.time()
            return None, None

        def ate_sumir(desde, limite=12.0):
            sumiu, ultimo = None, []
            while time.time() - desde < limite:
                t_retrato = time.time()
                ultimo = r.textos()
                if not onde(r, SENHA_TROCADA, ultimo):
                    sumiu = t_retrato
                    break
                time.sleep(0.1)
            return sumiu, ultimo

        # CI-1 (16/11/2026, depois da CI de ensaio do G1b): o critério era "sumiu antes de 2,9 s desde o clique", e
        # numa máquina lenta o aviso nem tinha aparecido em 2 s. Agora há um CONTROLE na mesma rodada:
        # 1. sem toque, mede quanto tempo o aviso fica na tela NESTA máquina (os 3 s dele mais as animações, que com a
        #    guia aberta ficam lentas no desenho por software, o H25);
        # 2. a senha volta à de antes, e o toque vem logo que o aviso aparece: com o toque, ele fica na tela no máximo
        #    70% do tempo do controle. Assim foi o toque, e não o tempo, que o fechou. (Na janela larga desta máquina:
        #    2,1 s sem toque e 1,2 s com toque; sem o toque de verdade, 2,4 s.)
        # O programa não muda: só o roteiro espera mais e mede contra o controle.
        t0 = trocar(senha, senha + "9", "controle")
        if t0 is None:
            return
        aviso, visto = ver_aviso(t0)
        if not ok("D: o aviso da senha trocada aparece (com a guia aberta)", aviso is not None, r.foto("-aviso")):
            return
        # O-1: as posições do C, aqui, com a guia já parada: duas leituras seguidas iguais (o aviso e a seta no mesmo
        # lugar), enquanto o aviso está na tela.
        retrato, anterior = None, None
        while retrato is None and onde(r, SENHA_TROCADA, tx := r.textos()):
            av, st = onde(r, SENHA_TROCADA, tx), geo_em(tx, "setaDeRetorno")
            if onde(r, GUIA, tx) and av and st:
                if (av[0], st) == anterior:
                    retrato = tx
                anterior = (av[0], st)
        f = r.foto("-posicoes")
        if retrato is None:
            ok("D: posições lidas paradas, com o aviso na tela", False, f"{f} INSTRUMENTAÇÃO: o aviso sumiu antes")
        else:
            g, a2, seta = onde(r, GUIA, retrato)[0], onde(r, SENHA_TROCADA, retrato)[0], geo_em(retrato, "setaDeRetorno")
            ok("D: a orientação em cima, o aviso abaixo dela, a tela abaixo dos dois (nada sobreposto)",
               g[1] + 30 <= a2[1] - MEIO_AVISO and a2[1] + MEIO_AVISO <= seta[1],
               f"{f} guia y={g[1]} aviso y={a2[1]} seta y={seta[1]}")
        sumiu, depois = ate_sumir(visto)
        natural = None if sumiu is None else sumiu - visto
        ok("D (controle): sem toque, o aviso some sozinho e a orientação da guia continua",
           natural is not None and natural >= 1.5 and bool(onde(r, GUIA, depois)),
           f"{r.foto('-controle')} na tela por {natural if natural is None else round(natural, 2)} s")
        if natural is None:
            return
        t0 = trocar(senha + "9", senha, "toque")
        if t0 is None:
            return
        aviso, visto = ver_aviso(t0)
        if not ok("D: o aviso aparece de novo (a senha voltou à de antes)", aviso is not None, r.foto("-aviso-2")):
            return
        # RUSTINHO_AVISOS_SEM_TOQUE=1: prova do próprio critério (o roteiro não toca; o D tem de reprovar).
        if not os.environ.get("RUSTINHO_AVISOS_SEM_TOQUE"):
            r.clicar_em(*aviso)  # o toque no aviso
        sumiu, depois = ate_sumir(visto)
        com_toque = None if sumiu is None else sumiu - visto
        f = r.foto("-depois-do-toque")
        print(f"MEDIDA D: na tela sem toque {round(natural, 2)} s; com toque "
              f"{com_toque if com_toque is None else round(com_toque, 2)} s", flush=True)
        ok("D: o toque fecha só o aviso (no máximo 70% do tempo dele, medido no controle); a orientação da guia continua",
           com_toque is not None and com_toque <= 0.7 * natural and bool(onde(r, GUIA, depois)),
           f"{f} com o toque: {com_toque if com_toque is None else round(com_toque, 2)} s; sem o toque: "
           f"{round(natural, 2)} s; guia={bool(onde(r, GUIA, depois))}")
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
        # B-7 da revisão do VIS2/AV-1: antes, o botão já estava à vista e nada rolava; o "não alterar a rolagem
        # interna" ficava sem prova. Agora a página rola de verdade (o botão sobe pelo menos 40 px e continua à vista)
        # antes de medir.
        b_topo = onde(r, "Fazer backup agora")[0][1]
        for _ in range(20):
            if b_topo - onde(r, "Fazer backup agora")[0][1] >= 40:
                break
            if nav.CELULAR:
                arrastar_para_cima(0.06)
            else:
                xdo("mousemove", str(min(600, 300 if JANELA == "estreita" else 600)), str(r.altura() // 2), "click", "5")
                time.sleep(0.4)
        time.sleep(0.8)
        p0 = parado(r, "Fazer backup agora")
        b0 = p0[1] if p0 else onde(r, "Fazer backup agora")[0][1]
        f = r.foto("-antes")
        # A lista de textos dá a posição mesmo do que está escondido sob a faixa fixa das abas. Na janela larga, uma
        # volta da roda (72 px) leva o botão de 346 a 274, com a metade de cima sob a faixa (que vai até 275: o texto
        # da aba "Backup" + 38 px, medido na foto), e o clique no meio do texto caía na faixa (integ7, 16/11/2026). O
        # clique vai na parte de baixo do botão (ABAIXO_DO_MEIO), e é ela que tem de estar à vista.
        abas = [y for _, y, t in r.textos() if t == "Backup"]
        corte = min(abas) + FAIXA_ABAIXO_DA_ABA if abas else None
        clique = b0 + ABAIXO_DO_MEIO
        if not ok("E: a página rolou antes de medir (o botão subiu pelo menos 40 px e o ponto do clique, na parte de "
                  "baixo dele, está à vista, abaixo da faixa fixa das abas)",
                  b_topo - b0 >= 40 and corte is not None and corte + 4 <= clique < r.altura() - 30,
                  f"{f} botão {b_topo} → {b0}; a faixa vai até {corte}; o clique em {clique}"):
            return
        for jeito in ("sozinho", "toque"):
            botao = parado(r, "Fazer backup agora")  # o clique na posição de agora, parada
            if botao:
                r.clicar_em(botao[0], botao[1] + ABAIXO_DO_MEIO)
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
