#!/usr/bin/env python3
"""A TELA do Receber (não a janelinha) em telas pequenas (16/11/2026, achado na sonda v2 do Receber).

A tela é uma coluna que ocupa a janela: cabeçalho, abas, a ficha do cliente e, nela, a lista de parcelas (que fica com
a altura que sobra e rola sozinha) e a linha de baixo com "Receber tudo". Com pouca altura, a suspeita é que a lista
fica sem altura (nenhuma parcela à vista) e a linha de baixo passa da janela. Só MEDE; não muda nada no programa.

Para cada caso e tamanho, abre a tela de receber do Zé da Oficina e confere, pelo que o programa informa:
  - o botão "Receber tudo" está inteiro dentro da janela (à vista ou rolando a tela, quando ela rola);
  - pelo menos uma parcela está à vista (o texto da conta, inteiro dentro da janela).
  (a parcela "à vista" é a que está dentro do recorte da lista: entre a linha "Todas em aberto" e o "Receber tudo" e,
   no programa que informa a GEO:listaDeParcelas, dentro dela; R2 da revisão da T040)
Casos: normal (uma conta, "Caderno"); muitas parcelas (12 contas, "Venda a prazo NN"); e, antes deles, a tela aberta
SEM cliente (um cliente à vista na lista "Quem deve", abaixo da busca; R1 da revisão da T040).
Sem faixa no pé: o app não é de borda a borda (alvo SDK 34, showMaximized), então a barra de gestos do Android fica
FORA da janela (T041).

    RUSTINHO_TELA=:68 python3 telas_receber_tela_altura.py [pasta das fotos]      (rodar da raiz da árvore testada)
    RUSTINHO_CASOS_CELULAR=1   (os tamanhos de celular; sem ela, os de computador)
"""
import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.getcwd(), "app", "testes"))
import telas_navegacao_casos as nav  # noqa: E402
import telas_receber_altura as alt  # noqa: E402

TELA = nav.TELA
CASOS = {"normal": "Caderno", "muitas parcelas": "Venda a prazo"}


def ret_de(r, comeco):
    out = []
    for t in alt.medidas(r):
        if t.startswith("RET:"):
            geo, resto = t[4:].split("|", 1)
            if resto.split("|")[-1].strip().startswith(comeco):
                v = [float(a) for a in geo.split(",")]
                out.append((v[0], v[1], v[2], v[3]))
    return out


def geo_de(r, nome):
    """O retângulo (x, y, w, h) do item com esse objectName (linha GEO:), ou None (o programa de antes não tem)."""
    for t in alt.medidas(r):
        if t.startswith(f"GEO:{nome}:"):
            return tuple(float(v) for v in t[len(nome) + 5:].split(",")[:4])
    return None


def no_recorte(p, cima, baixo, geo):
    """O texto p está inteiro entre cima e baixo e, se a lista informa a geometria, dentro dela também."""
    if geo is not None:
        cima, baixo = max(cima, geo[1]), min(baixo, geo[1] + geo[3])
    return p[3] > 0 and p[1] >= cima - 2 and p[1] + p[3] <= baixo + 2


def rolar_tela(largura, altura, celular):
    """Rola a tela inteira para baixo: arrasta para cima a partir do alto (fora das listas, que rolariam sozinhas) e, só
    no computador, gira a roda do mouse no mesmo lugar (no celular não há roda: R10 da revisão da T040)."""
    x, y0 = max(20, largura // 5), int(altura * 0.32)
    for _ in range(4):
        alt.xdo("mousemove", str(x), str(y0))
        alt.xdo("mousedown", "1")
        for i in range(1, 9):
            alt.xdo("mousemove", str(x), str(max(2, y0 - int(altura * 0.3) * i // 8)))
            time.sleep(0.03)
        time.sleep(0.15)
        alt.xdo("mouseup", "1")
        time.sleep(0.4)
        if not celular:
            alt.xdo("mousemove", str(x), str(y0))
            for _ in range(5):
                alt.xdo("click", "5")
            time.sleep(0.4)


def caso(placar, fotos, modelo, nome_caso, nome_tam, celular, tamanho):
    ok = placar.ok
    largura, altura = tamanho
    rotulo = f"{nome_caso} · {nome_tam}"
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-tela-receber-")
    shutil.rmtree(pasta)
    shutil.copytree(modelo, pasta)
    nav.por_fora(pasta, alt.CASOS[nome_caso])
    ze = nav.ler(pasta, "SELECT id FROM pessoa WHERE nome = 'Zé da Oficina'")[0][0]
    r = alt.Rustinho(pasta, {"clienteInicial": ze}, fotos, f"tela-{nome_caso}-{nome_tam}".replace(" ", "_"), celular, tamanho)
    placar.logs.append(r.saida)
    try:
        r.entrar("Ana")
        if not ok(f"{rotulo}: a tela de receber abriu", r.esperar(lambda: r.tem("Zé da Oficina"), 20), r.foto("-0")):
            return
        time.sleep(1.0)
        f = r.foto("-1")
        # T040 (BUG-20261117-ULCJ): quando a TELA rola (a rolagem informa a geometria, GEO:rolagemDaTelaReceber), o
        # critério é ALCANÇAR: rola a tela até o fim, arrastando numa área sem lista (o alto, à esquerda), e então mede.
        # O programa de antes não tem essa rolagem: é medido parado, como sempre.
        if any(t.startswith("GEO:rolagemDaTelaReceber:") for t in alt.medidas(r)):
            rolar_tela(largura, altura, celular)
            f = r.foto("-2")
        b = alt.botoes(r, "Receber tudo")
        ret = b[-1] if b else None
        # T041 (18/11): a barra de gestos do Android fica FORA da janela do app (alvo SDK 34, sem desenho de borda a
        # borda): o critério é o botão inteiro dentro da janela. A faixa de 30 px que havia aqui supunha o contrário (e
        # levou ao pé de 32 px do 567178e, desfeito: R3 da revisão da T040).
        dentro = bool(ret) and alt.dentro(ret, largura, altura)
        ok(f"{rotulo}: o botão Receber tudo fica inteiro dentro da janela", dentro,
           f"{f} janela {largura}×{altura}; botão {None if ret is None else tuple(round(v) for v in ret)}")
        # R2 da revisão da T040: a parcela "à vista" é a que está DENTRO do recorte da lista: abaixo da linha de cima da
        # conta ("Todas em aberto") e acima do "Receber tudo". A linha RET do programa não sabe do recorte.
        topo_lista = max([y + h for _x, y, _w, h in ret_de(r, "Todas em aberto")], default=0)
        pe_lista = ret[1] if ret else altura
        # Com a GEO:listaDeParcelas (só no programa novo), também dentro dela.
        geo = geo_de(r, "listaDeParcelas")
        parcelas = [p for p in ret_de(r, CASOS[nome_caso]) if no_recorte(p, max(0, topo_lista), min(altura, pe_lista), geo)]
        ok(f"{rotulo}: pelo menos uma parcela fica à vista na lista", bool(parcelas),
           f"{f} parcelas à vista: {len(parcelas)}; todas as medidas: {[tuple(round(v) for v in p) for p in ret_de(r, CASOS[nome_caso])]}")
    finally:
        r.fechar()
        shutil.rmtree(pasta, ignore_errors=True)


def caso_sem_cliente(placar, fotos, modelo, nome_tam, celular, tamanho):
    """R1 da revisão da T040: a tela aberta SEM cliente (o caminho normal: Início, atalho, Resumo, Vender). Pelo menos um
    cliente tem de ficar à vista na lista "Quem deve" (dentro do recorte dela: abaixo da busca e dentro da janela), à
    vista ou rolando a tela quando ela rola."""
    ok = placar.ok
    largura, altura = tamanho
    rotulo = f"sem cliente aberto · {nome_tam}"
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-tela-receber-")
    shutil.rmtree(pasta)
    shutil.copytree(modelo, pasta)
    nav.por_fora(pasta, alt.CASOS["normal"])
    r = alt.Rustinho(pasta, {}, fotos, f"tela-sem_cliente-{nome_tam}".replace(" ", "_"), celular, tamanho)
    placar.logs.append(r.saida)
    try:
        r.entrar("Ana")
        if not ok(f"{rotulo}: a tela de receber abriu", r.esperar(lambda: r.tem("Procurar cliente"), 20), r.foto("-0")):
            return
        time.sleep(1.0)
        f = r.foto("-1")
        if any(t.startswith("GEO:rolagemDaTelaReceber:") for t in alt.medidas(r)):
            rolar_tela(largura, altura, celular)
            f = r.foto("-2")
        busca = [y + h for _x, y, _w, h in ret_de(r, "Procurar cliente")]
        topo = max(busca, default=0)
        geo = geo_de(r, "listaDeClientes")
        linhas = [p for p in ret_de(r, "Zé da Oficina") if no_recorte(p, topo, altura, geo)]
        ok(f"{rotulo}: pelo menos um cliente fica à vista na lista Quem deve", bool(linhas),
           f"{f} janela {largura}×{altura}; abaixo da busca (y ≥ {round(topo)}); medidas: {[tuple(round(v) for v in p) for p in ret_de(r, 'Zé da Oficina')]}")
    finally:
        r.fechar()
        shutil.rmtree(pasta, ignore_errors=True)


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-tela-receber-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    print("BINÁRIO:", nav.PROGRAMA, flush=True)
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = nav.Placar()
    modelo = tempfile.mkdtemp(prefix="rustinho-loja-tela-receber-modelo-")
    try:
        nav.montar_loja(modelo)
        for nome_tam, celular, tamanho in alt.TAMANHOS:
            print(f"--- sem cliente aberto · {nome_tam}", flush=True)
            try:
                caso_sem_cliente(placar, fotos, modelo, nome_tam, celular, tamanho)
            except Exception as e:
                placar.ok(f"sem cliente aberto · {nome_tam} chegou ao fim ({e!r})", False)
        for nome_caso in CASOS:
            for nome_tam, celular, tamanho in alt.TAMANHOS:
                print(f"--- {nome_caso} · {nome_tam}", flush=True)
                try:
                    caso(placar, fotos, modelo, nome_caso, nome_tam, celular, tamanho)
                except Exception as e:
                    placar.ok(f"{nome_caso} · {nome_tam} chegou ao fim ({e!r})", False)
    finally:
        shutil.rmtree(modelo, ignore_errors=True)
        xvfb.terminate()
        try:
            xvfb.wait(10)
        except subprocess.TimeoutExpired:
            xvfb.kill()
    print(f"RESULTADO: {placar.certos} certos, {placar.errados} errados")
    sys.exit(0 if placar.errados == 0 else 1)


if __name__ == "__main__":
    main()
