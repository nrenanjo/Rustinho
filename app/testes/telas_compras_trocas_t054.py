#!/usr/bin/env python3
"""T054 a T058 (pedidos do proprietário de 20/11/2026), na tela de verdade.

Compras (T054):
  C1  com 350 notas de outro CNPJ, a busca continua dentro da janela (a lista
      "Não importadas" não empurra o resto da tela para fora);
  C2  rolando a lista com a roda do mouse, a última nota aparece na área da
      lista, com o Ver e o Descartar na mesma linha;
  C3  Descartar na última: o título conta uma a menos na hora e a lista não
      volta para o começo (a nota de cima continua à vista);
  C4  Ver numa nota abre a janelinha daquela nota;
  C5  com a janelinha aberta, o "Mostrar valores" do canto da tela continua no
      mesmo lugar e responde ao clique: os números aparecem na janelinha e o
      controle vira "Esconder"; outro clique esconde de novo;
  C6  "Descartar nota" na janelinha tira a nota da lista na hora;
  C7  fechada a janelinha, o controle volta ao lugar de sempre e funciona;
  C8  o mesmo em janelas menores (1024 x 600, 800 x 520, 640 x 480 e
      340 x 480, a menor que o programa aceita): a busca à vista, a última
      nota alcançável e o controle clicável com a janelinha aberta.

Trocas e devoluções (T055), com Orçamentos como referência:
  T1  os três botões ficam acima da barra de busca, e a barra ocupa a largura
      da tela (começa na margem da esquerda e vai até a da direita);
  T2  cada botão mantém a busca dele: Vales-troca não tem busca, Já devolvidas
      tem a dica própria, e os botões não mudam de lugar ao trocar;
  T3  a busca continua funcionando (o texto digitado entra na barra);
  T4  em janelas menores, os botões e a barra ficam dentro da janela, a barra
      embaixo dos botões.

Financeiro › A pagar e Cheques (T056):
  F1  A pagar: Em aberto, Pagas e Todas numa linha (com Categorias à direita),
      acima da busca; a busca na largura toda; os cartões, as abas e o Lançar
      conta no lugar (FC1, o mesmo em Cheques, com Em carteira e Todos);
  F2  trocar o filtro não mexe nos filtros nem na busca (FC2);
  F3  a busca recebe o texto (FC3);
  F4  em janelas menores, filtros, Categorias e busca dentro da janela, a busca
      embaixo e larga (mais da metade da área), nada escondido (FC4).

Ordens de serviço (T057):
  O1  Abertas, Prontas, Entregues e Todas numa linha acima da busca; a busca na
      largura toda; o Nova OS no canto de cima;
  O2  trocar o filtro não mexe nos filtros nem na busca;
  O3  a busca recebe o texto;
  O4  em janelas menores, filtros e busca dentro da janela, a busca embaixo e
      larga, o Nova OS à vista.

Pendências de identificação XML, sobre Peças (T058):
  P1  a janela abre sobre Peças, com a lista;
  P2  alternar 4 vezes entre "Trazer cadastros" e "Pendências de
      identificação" muda só a janela: Peças não abre peça nenhuma;
  P3  a pendência aberta não tem mais o "Todas as pendências";
  P4  a seta volta da pendência para a lista;
  P5  o Esc faz o mesmo e, na lista, fecha a janela; Peças fica intacta;
  P6  com outra janelinha por cima ("Qual é a peça?"), o Esc fecha só ela.

    python3 app/testes/telas_compras_trocas_t054.py [pasta para as fotos]
    RUSTINHO_CASOS=F python3 app/testes/telas_compras_trocas_t054.py   (só alguns grupos: C, T, O, F, P)

Precisa de: Xvfb, xdotool, ImageMagick (import), cargo. Sempre pelo
rodar_isolado.sh (rede isolada).
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time

os.environ.setdefault("RUSTINHO_TELA", ":69")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_unidades as u  # noqa: E402

base = u.base
TELA = u.TELA
xdo = base.xdo
simples = base.simples
N = 350
TAMANHOS_MENORES = ((1024, 600), (800, 520), (640, 480), (340, 480))

resultados = []

# Falhas conhecidas que NÃO vêm destas mudanças (já estavam no código publicado,
# `06520fa`): saem na lista, mas não reprovam o roteiro. Se uma delas passar, o
# roteiro reprova (é hora de tirá-la daqui).
ESPERADAS = {
    "F4-340x480": "BUG-20261120-FN34: na janela mínima (340 x 480), a tela Financeiro inteira não cabe e não rola; "
                  "os filtros e a busca de A pagar ficam abaixo da borda (já era assim antes da T056)",
}


def conferir(codigo, nome, certo, detalhe=""):
    resultados.append((codigo, bool(certo), nome, detalhe))
    print(("✔ " if certo else "✘ ") + f"{codigo} {nome}" + ("" if certo else f" — {detalhe}"), flush=True)


def notas(n):
    """Notas de outro CNPJ como a tela as guarda depois da importação (o mesmo
    caminho do caso D do telas_navegacao_etapa1.py). Campos curtos: o --props
    é um argumento só, e o sistema limita o tamanho dele."""
    return [{
        "arquivo": f"nota-{i:03d}.xml", "lojaNome": "Loja", "lojaDocumento": "11.444.777/0001-61",
        "emitenteNome": f"FORNECEDOR {i:03d}", "emitenteDocumento": "12.345.678/0001-95",
        "destinatarioNome": "OUTRA", "destinatarioDocumento": "11.222.333/0001-81",
        "numero": str(1000 + i), "serie": "1", "emissao": "08/10/2026", "total": "1,00", "itens": [],
    } for i in range(1, n + 1)]


def montar_loja(pasta):
    env = dict(os.environ, RUSTINHO_LOJA_TESTE=pasta)
    subprocess.run(["cargo", "test", "-q", "-p", "rustinho-nucleo", "--test", "lojas_de_teste", "--",
                    "--ignored", "--exact", "montar_geometria"],
                   cwd=base.RAIZ, env=env, stdout=subprocess.DEVNULL, check=True)


def esperar_janela():
    for _ in range(150):
        s = subprocess.run(["xdotool", "search", "--onlyvisible", "--class", "rustinho"],
                           env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
        if s.stdout.split():
            return
        time.sleep(0.2)


def so_textos(t):
    return [(x, y, tx) for x, y, tx in t if not tx.startswith(("GEO:", "RET:", "TOQUE:"))]


def achar(t, trecho, comeca=False):
    s = simples(trecho)
    return [(x, y, tx) for x, y, tx in so_textos(t)
            if (simples(tx).startswith(s) if comeca else s in simples(tx))]


def igual(t, texto):
    return [(x, y, tx) for x, y, tx in so_textos(t) if simples(tx) == simples(texto)]


def retangulo(t, texto):
    """(x, y, largura, altura) do texto, pelas linhas RET: (--medir-toque)."""
    for r in u.rets(t):
        if simples(r[6]) == simples(texto):
            return r[:4]
    return None


class Tela:
    def __init__(self, rr):
        self.rr = rr

    def janela(self):
        return u.tema.janela_do_rustinho()

    def clicar_ponto(self, x, y):
        px, py = self.rr.ponto(x, y)
        xdo("mousemove", str(px), str(py), "click", "1")

    def rolar(self, x, y, vezes, para_baixo=True):
        """Giros da roda com intervalo de pessoa (100 ms): giros muito rápidos
        o Qt junta num só deslize, e cada giro anda cerca de uma linha (medido
        na sonda de 20/11, `PR/i5_evid/sonda_roda.txt`)."""
        px, py = self.rr.ponto(x, y)
        xdo("mousemove", str(px), str(py))
        xdo("click", "--repeat", str(vezes), "--delay", "100", "5" if para_baixo else "4")
        time.sleep(1.2)


def faixa_visivel(t):
    """(alto, baixo) da área em que o meio de uma linha de "Não importadas"
    fica com folga dentro da lista: do título (meia altura e o espaço até a
    lista, 23) até a borda de cima da busca (meia busca, 27, e o espaço, 14),
    menos o acabamento da faixa (16 nas janelas baixas, 22 nas outras: usa o
    menor) e 20 de folga (as linhas têm 50 de altura). Uma linha cortada pela
    borda da lista além disso não conta como à vista (o clique nela pode cair
    fora da lista)."""
    titulo = achar(t, "Não importad")
    busca = achar(t, "Fornecedor, número da nota ou peça")
    if not titulo or not busca:
        return None
    return titulo[0][1] + 23 + 20, busca[0][1] - 27 - 14 - 16 - 20


def linha_da_nota(t, numero):
    return achar(t, f"nota-{numero:03d}.xml")


def botao_na_linha(t, texto, y):
    return [(x, by, tx) for x, by, tx in igual(t, texto) if abs(by - y) <= 14]


def ir_ao_fim(tela, rr, ultimo, limite=40):
    """Rola a lista até a nota `ultimo` ficar na área da lista. Devolve os textos."""
    t = rr.textos()
    parada = 0
    anterior = None
    for _ in range(limite):
        area = faixa_visivel(t)
        linha = linha_da_nota(t, ultimo)
        if area and linha and area[0] < linha[0][1] < area[1]:
            return t
        primeira = [x for x in achar(t, "nota-") if area and area[0] < x[1] < area[1]]
        if not primeira:
            return t
        # A lista não rola (a primeira linha à vista não muda): desiste.
        parada = parada + 1 if primeira[0][2] == anterior else 0
        if parada >= 3:
            return t
        anterior = primeira[0][2]
        # O ponteiro no meio da lista (nunca numa linha cortada pela borda).
        tela.rolar(primeira[0][0], (area[0] + area[1]) // 2, 25)
        t = rr.textos()
    return t


def casos_compras(modelo, fotos, w, h, completo):
    rr = u.Rustinho(modelo, "TelaCompras", {"naoImportadas": notas(N)}, fotos, f"compras_{w}x{h}")
    tela = Tela(rr)
    sufixo = "" if completo else f" ({w} x {h})"
    pre = "C" if completo else f"C8-{w}x{h}-"
    try:
        esperar_janela()
        time.sleep(0.5)
        rr.redimensionar(w, h)
        rr.entrar("Ana")
        time.sleep(3)
        _, _, ww, wh = tela.janela()
        t = rr.textos()
        rr.foto("-0")
        busca = achar(t, "Fornecedor, número da nota ou peça")
        conferir(pre + "1", "a busca continua dentro da janela com 350 notas" + sufixo,
                 busca and 0 < busca[0][1] < wh, f"busca em {busca[:1]}, janela {ww} x {wh}")
        olho_antes = igual(t, "Mostrar valores")

        t = ir_ao_fim(tela, rr, N)
        rr.foto("-1-fim")
        area = faixa_visivel(t)
        ultima = linha_da_nota(t, N)
        ok = bool(area and ultima and area[0] < ultima[0][1] < area[1] and ultima[0][1] < wh)
        ver = botao_na_linha(t, "Ver", ultima[0][1]) if ultima else []
        descartar = botao_na_linha(t, "Descartar", ultima[0][1]) if ultima else []
        conferir(pre + "2", "a última nota aparece na lista, com Ver e Descartar" + sufixo,
                 ok and ver and descartar and descartar[0][0] < ww,
                 f"área {area}, última {ultima[:1]}, Ver {ver[:1]}, Descartar {descartar[:1]}")

        if completo and ok and descartar:
            tela.clicar_ponto(descartar[0][0], descartar[0][1])
            time.sleep(0.8)
            t = rr.textos()
            rr.foto("-2-descartou")
            area = faixa_visivel(t)
            vizinha = linha_da_nota(t, N - 1)
            conferir("C3", "Descartar: conta uma a menos na hora e a lista fica onde estava",
                     achar(t, f"Não importadas: {N - 1} notas") and not linha_da_nota(t, N)
                     and vizinha and area and area[0] < vizinha[0][1] < area[1],
                     f"título {achar(t, 'Não importad')[:1]}, vizinha {vizinha[:1]}, área {area}")
        elif completo:
            conferir("C3", "Descartar: conta uma a menos na hora e a lista fica onde estava", False,
                     "sem a última nota à vista, não deu para descartar")

        # Ver numa nota à vista (a de cima da que sumiu, ou a primeira à vista).
        t = rr.textos()
        area = faixa_visivel(t)
        visiveis = [x for x in achar(t, "nota-") if area and area[0] < x[1] < min(area[1], wh)]
        alvo = visiveis[-1] if visiveis else None
        ver = botao_na_linha(t, "Ver", alvo[1]) if alvo else []
        if ver:
            tela.clicar_ponto(ver[0][0], ver[0][1])
            time.sleep(1.2)
        t = rr.textos()
        rr.foto("-3-ver")
        abriu = achar(t, "CNPJ da nota é diferente")
        nome_alvo = alvo[2].split(" — ")[0] if alvo else "?"
        if completo:
            conferir("C4", "Ver abre a janelinha daquela nota", abriu and len(achar(t, nome_alvo)) >= 2,
                     f"alvo {nome_alvo}, janelinha {abriu[:1]}")

        # C5: o controle do canto com a janelinha aberta.
        olho = igual(t, "Mostrar valores")
        mesmo_lugar = olho and olho_antes and abs(olho[0][0] - olho_antes[0][0]) <= 2 \
            and abs(olho[0][1] - olho_antes[0][1]) <= 2
        dentro = olho and 0 < olho[0][0] < ww and 0 < olho[0][1] < wh
        clicou = False
        if abriu and olho:
            tela.clicar_ponto(olho[0][0], olho[0][1])
            time.sleep(0.8)
            t2 = rr.textos()
            rr.foto("-4-olho-aberto")
            clicou = bool(achar(t2, "11.222.333/0001-81") and igual(t2, "Esconder")
                          and achar(t2, "CNPJ da nota é diferente"))
            esc = igual(t2, "Esconder")
            if esc:
                tela.clicar_ponto(esc[0][0], esc[0][1])
                time.sleep(0.8)
            t3 = rr.textos()
            fechou_de_novo = bool(igual(t3, "Mostrar valores") and not achar(t3, "11.222.333/0001-81")
                                  and achar(t3, "CNPJ da nota é diferente"))
        else:
            fechou_de_novo = False
        conferir(pre + ("5" if completo else "5"),
                 "com a janelinha aberta, o Mostrar valores fica no lugar e responde" + sufixo,
                 abriu and mesmo_lugar and dentro and clicou and fechou_de_novo,
                 f"janelinha {bool(abriu)}, olho {olho[:1]} (antes {olho_antes[:1]}), "
                 f"clique mostrou {clicou}, esconder de novo {fechou_de_novo}")

        if completo and abriu:
            antes = achar(rr.textos(), "Não importadas:")
            rr.clicar("Descartar nota")
            time.sleep(1)
            t = rr.textos()
            rr.foto("-5-descartar-nota")
            n_antes = int(antes[0][2].split(":")[1].split()[0]) if antes else -1
            conferir("C6", "Descartar nota na janelinha tira a nota da lista na hora",
                     not achar(t, "CNPJ da nota é diferente") and achar(t, f"Não importadas: {n_antes - 1} notas")
                     and not [x for x in achar(t, nome_alvo)],
                     f"antes {antes[:1]}, depois {achar(t, 'Não importad')[:1]}")
            t = rr.textos()
            olho = igual(t, "Mostrar valores")
            if olho:
                tela.clicar_ponto(olho[0][0], olho[0][1])
                time.sleep(0.8)
            t2 = rr.textos()
            conferir("C7", "fechada a janelinha, o controle volta ao lugar e funciona",
                     olho and olho_antes and abs(olho[0][0] - olho_antes[0][0]) <= 2
                     and abs(olho[0][1] - olho_antes[0][1]) <= 2 and igual(t2, "Esconder"),
                     f"olho {olho[:1]} (antes {olho_antes[:1]}), depois do clique {igual(t2, 'Esconder')[:1]}")
            esc = igual(t2, "Esconder")
            if esc:
                tela.clicar_ponto(esc[0][0], esc[0][1])
                time.sleep(0.6)
        elif abriu:
            xdo("key", "Escape")
            time.sleep(0.6)
    finally:
        rr.fechar()


def casos_trocas(modelo, fotos, w, h, completo):
    rr = u.Rustinho(modelo, "TelaTrocas", {}, fotos, f"trocas_{w}x{h}")
    tela = Tela(rr)
    sufixo = "" if completo else f" ({w} x {h})"
    try:
        esperar_janela()
        time.sleep(0.5)
        rr.redimensionar(w, h)
        rr.entrar("Ana")
        time.sleep(3)
        _, _, ww, wh = tela.janela()
        t = rr.textos()
        rr.foto("-0")
        botoes = [igual(t, n) for n in ("Devolver", "Vales-troca", "Já devolvidas")]
        dica = achar(t, "Número da venda (está no comprovante)")
        rb = retangulo(t, "Devolver")
        rd = [r for r in u.rets(t) if simples(r[6]).startswith(simples("Número da venda (está no comprovante)"))]
        copiar = igual(t, "Copiar a busca")
        todos = all(botoes) and dica
        acima = todos and max(b[0][1] for b in botoes) + 20 < dica[0][1]
        if completo:
            largura_toda = bool(rd and rb and abs(rd[0][0] - rb[0]) < 120 and copiar
                                and copiar[0][0] > ww - 120)
            conferir("T1", "os três botões acima da busca, e a busca na largura da tela", acima and largura_toda,
                     f"botões {[b[:1] for b in botoes]}, dica {rd[:1]}, Devolver {rb}, copiar {copiar[:1]}, "
                     f"janela {ww}")
            lugar = [b[0][:2] for b in botoes]
            rr.clicar("Vales-troca")
            time.sleep(1)
            t = rr.textos()
            rr.foto("-1-vales")
            sem_busca = not achar(t, "Número da venda") and not achar(t, "Cliente, número da venda ou peça")
            lugar_v = [igual(t, n)[0][:2] if igual(t, n) else None for n in ("Devolver", "Vales-troca", "Já devolvidas")]
            rr.clicar("Já devolvidas")
            time.sleep(1)
            t = rr.textos()
            rr.foto("-2-feitas")
            dica_f = achar(t, "Cliente, número da venda ou peça")
            lugar_f = [igual(t, n)[0][:2] if igual(t, n) else None for n in ("Devolver", "Vales-troca", "Já devolvidas")]
            conferir("T2", "cada botão com a busca dele, e os botões não mudam de lugar",
                     sem_busca and dica_f and dica_f[0][1] > max(p[1] for p in lugar) + 20
                     and lugar_v == lugar and lugar_f == lugar,
                     f"vales sem busca {sem_busca}, dica de Já devolvidas {dica_f[:1]}, "
                     f"lugares {lugar} / {lugar_v} / {lugar_f}")
            rr.clicar("Devolver")
            time.sleep(1)
            t = rr.textos()
            d = achar(t, "Número da venda (está no comprovante)")
            if d:
                tela.clicar_ponto(d[0][0], d[0][1])
                time.sleep(0.4)
                xdo("type", "--delay", "60", "correia")
                time.sleep(1.2)
            t = rr.textos()
            rr.foto("-3-busca")
            conferir("T3", "a busca recebe o texto e continua no lugar", achar(t, "correia")
                     and (not d or abs(achar(t, "correia")[0][1] - d[0][1]) <= 4),
                     f"texto {achar(t, 'correia')[:1]}, dica antes {d[:1]}")
        else:
            dentro = todos and all(0 <= r[0] and r[0] + r[2] <= ww + 1 for n in ("Devolver", "Vales-troca", "Já devolvidas")
                                   for r in [retangulo(t, n)] if r) and rd and rd[0][0] + rd[0][2] <= ww + 1
            conferir(f"T4-{w}x{h}", "botões e busca dentro da janela, a busca embaixo dos botões" + sufixo,
                     acima and dentro, f"botões {[b[:1] for b in botoes]}, dica {rd[:1]}, janela {ww} x {wh}")
    finally:
        rr.fechar()


def casos_financeiro(modelo, fotos, secao, w, h, completo):
    """T056: A pagar e Cheques com os filtros numa linha própria e a busca embaixo."""
    if secao == "pagar":
        filtros, dica_txt, nome = ("Em aberto", "Pagas", "Todas"), "Procurar: descrição, fornecedor, nota ou categoria", "F"
    else:
        filtros, dica_txt, nome = ("Em carteira", "Todos"), "Procurar: número, quem assinou, cliente ou banco", "FC"
    rr = u.Rustinho(modelo, "TelaFinanceiro", {"secao": secao}, fotos, f"fin_{secao}_{w}x{h}")
    tela = Tela(rr)
    sufixo = "" if completo else f" ({w} x {h})"
    try:
        esperar_janela()
        time.sleep(0.5)
        rr.redimensionar(w, h)
        rr.entrar("Ana")
        time.sleep(3)
        _, _, ww, wh = tela.janela()
        t = rr.textos()
        rr.foto("-0")
        fs = [igual(t, f) for f in filtros]
        rd = [r for r in u.rets(t) if simples(r[6]).startswith(simples(dica_txt[:30]))]
        dica = achar(t, dica_txt[:30])
        copiar = igual(t, "Copiar a busca")
        abas = igual(t, "A pagar") if secao == "pagar" else igual(t, "Cheques")
        acima = all(fs) and dica and max(f[0][1] for f in fs) + 20 < dica[0][1]
        largura = bool(rd and copiar and rd[0][0] < 150 and copiar[-1][0] > ww - 120)
        extra = True
        detalhe_extra = ""
        if secao == "pagar":
            cat = igual(t, "Categorias")
            lancar = igual(t, "Lançar conta")
            extra = bool(cat and fs[0] and abs(cat[0][1] - fs[0][0][1]) <= 8 and dica and cat[0][1] < dica[0][1] - 20
                         and lancar)
            detalhe_extra = f", Categorias {cat[:1]}, Lançar conta {lancar[:1]}"
        cartoes = achar(t, "Total a pagar") if secao == "pagar" else achar(t, "Em carteira")
        if completo:
            conferir(f"{nome}1", f"{'A pagar' if secao == 'pagar' else 'Cheques'}: os filtros numa linha acima da busca, "
                                 "a busca na largura toda, os cartões e as abas no lugar",
                     acima and largura and extra and cartoes and abas,
                     f"filtros {[f[:1] for f in fs]}, dica {rd[:1]}, copiar {copiar[-1:]}, janela {ww}{detalhe_extra}")
            lugar = [f[0][:2] for f in fs]
            rr.clicar(filtros[-1])
            time.sleep(1.2)
            t = rr.textos()
            rr.foto("-1-filtro")
            lugar2 = [igual(t, f)[0][:2] if igual(t, f) else None for f in filtros]
            d2 = achar(t, dica_txt[:30])
            conferir(f"{nome}2", "trocar o filtro não mexe nos filtros nem na busca",
                     lugar2 == lugar and d2 and dica and d2[0][:2] == dica[0][:2],
                     f"filtros {lugar} → {lugar2}, busca {dica[:1]} → {d2[:1]}")
            if d2:
                tela.clicar_ponto(d2[0][0], d2[0][1])
                time.sleep(0.4)
                xdo("type", "--delay", "60", "energia")
                time.sleep(1.2)
            t = rr.textos()
            rr.foto("-2-busca")
            conferir(f"{nome}3", "a busca recebe o texto", achar(t, "energia"), f"{achar(t, 'energia')[:1]}")
        else:
            larg_busca = rd[0][2] if rd else 0
            dentro = all(fs) and all(0 <= r[0] and r[0] + r[2] <= ww + 1 for f in filtros
                                     for r in [retangulo(t, f)] if r) and copiar and copiar[-1][0] < ww
            if secao == "pagar":
                rc = retangulo(t, "Categorias")
                dentro = dentro and rc and rc[0] + rc[2] <= ww + 1 and rc[1] + rc[3] <= wh
            conferir(f"{nome}4-{w}x{h}", "filtros e busca dentro da janela, a busca embaixo e larga" + sufixo,
                     acima and dentro and copiar and rd and copiar[-1][0] - rd[0][0] > 0.5 * (ww - 64),
                     f"filtros {[f[:1] for f in fs]}, dica {rd[:1]}, copiar {copiar[-1:]}, janela {ww} x {wh}"
                     f"{detalhe_extra}")
    finally:
        rr.fechar()


def casos_ordens(modelo, fotos, w, h, completo):
    """T057: Ordens de serviço com os filtros numa linha própria e a busca embaixo."""
    filtros, dica_txt = ("Abertas", "Prontas", "Entregues", "Todas"), "Placa, cliente, carro ou número da OS"
    rr = u.Rustinho(modelo, "TelaOrdens", {}, fotos, f"ordens_{w}x{h}")
    tela = Tela(rr)
    sufixo = "" if completo else f" ({w} x {h})"
    try:
        esperar_janela()
        time.sleep(0.5)
        rr.redimensionar(w, h)
        rr.entrar("Ana")
        time.sleep(3)
        _, _, ww, wh = tela.janela()
        t = rr.textos()
        rr.foto("-0")
        fs = [achar(t, f, comeca=True)[:1] for f in filtros]
        rd = [r for r in u.rets(t) if simples(r[6]).startswith(simples(dica_txt))]
        dica = achar(t, dica_txt)
        copiar = igual(t, "Copiar a busca")
        nova = igual(t, "Nova OS")
        acima = all(fs) and dica and max(f[0][1] for f in fs) + 20 < dica[0][1]
        if completo:
            largura = bool(rd and copiar and rd[0][0] < 150 and copiar[-1][0] > ww - 120)
            topo = bool(nova and fs[0] and nova[0][1] < fs[0][0][1] and nova[0][0] > ww - 300)
            conferir("O1", "OS: os filtros numa linha acima da busca, a busca na largura toda, o Nova OS no canto de cima",
                     acima and largura and topo,
                     f"filtros {fs}, dica {rd[:1]}, copiar {copiar[-1:]}, Nova OS {nova[:1]}, janela {ww}")
            lugar = [f[0][:2] for f in fs]
            rr.clicar("Todas")
            time.sleep(1.2)
            t = rr.textos()
            rr.foto("-1-filtro")
            lugar2 = [achar(t, f, comeca=True)[0][:2] if achar(t, f, comeca=True) else None for f in filtros]
            d2 = achar(t, dica_txt)
            conferir("O2", "trocar o filtro não mexe nos filtros nem na busca",
                     lugar2 == lugar and d2 and dica and d2[0][:2] == dica[0][:2],
                     f"filtros {lugar} → {lugar2}, busca {dica[:1]} → {d2[:1]}")
            if d2:
                tela.clicar_ponto(d2[0][0], d2[0][1])
                time.sleep(0.4)
                xdo("type", "--delay", "60", "ABC1D23")
                time.sleep(1.2)
            t = rr.textos()
            rr.foto("-2-busca")
            conferir("O3", "a busca recebe o texto", achar(t, "ABC1D23"), f"{achar(t, 'ABC1D23')[:1]}")
        else:
            dentro = all(fs) and all(0 <= r[0] and r[0] + r[2] <= ww + 1 for f in filtros
                                     for r in [retangulo(t, achar(t, f, comeca=True)[0][2])] if fs and r) \
                and copiar and copiar[-1][0] < ww and nova
            conferir(f"O4-{w}x{h}", "filtros e busca dentro da janela, a busca embaixo e larga" + sufixo,
                     acima and dentro and rd and copiar and copiar[-1][0] - rd[0][0] > 0.5 * (ww - 64),
                     f"filtros {fs}, dica {rd[:1]}, copiar {copiar[-1:]}, Nova OS {nova[:1]}, janela {ww} x {wh}")
    finally:
        rr.fechar()


def montar_loja_pecas(pasta):
    """A loja do roteiro da lista de peças (K06): 120 peças e 2 pendências de identificação XML."""
    env = dict(os.environ, RUSTINHO_LOJA_TESTE=pasta)
    subprocess.run(["cargo", "test", "-q", "-p", "rustinho-nucleo", "--test", "lojas_de_teste", "--",
                    "--ignored", "--exact", "montar_lista_de_pecas"],
                   cwd=base.RAIZ, env=env, stdout=subprocess.DEVNULL, check=True)


def casos_pendencias(modelo, fotos):
    """T058: a janela de Pendências de identificação XML sobre Peças (1920 x 1080)."""
    rr = u.Rustinho(modelo, "TelaPecas", {"abrirPendenciasXml": True}, fotos, "pendencias")
    tela = Tela(rr)

    def peca_atras(t):
        return bool(igual(t, "Código interno") or igual(t, "Nome da peça"))

    def titulo(t):
        """A opção que a janela mostra (None com a janela fechada). Não usa o
        título "Pendências de identificação XML": a faixa de Peças, atrás da
        janela, tem o mesmo texto."""
        if achar(t, "Trazer cadastros pelo XML"):
            return "importar"
        if [x for x in so_textos(t) if x[2].startswith("Pendências de identificação (")]:
            return "pendencias"
        return None

    try:
        esperar_janela()
        time.sleep(0.5)
        rr.redimensionar(1920, 1080)
        rr.entrar("Ana")
        time.sleep(4)
        t = rr.textos()
        rr.foto("-0")
        conferir("P1", "a janela de pendências abre sobre Peças, com a lista", titulo(t) == "pendencias"
                 and achar(t, "PASTILHA FREIO DIANT COBREQ N-1100") and not peca_atras(t),
                 f"título {titulo(t)}, peça atrás {peca_atras(t)}")
        problemas = []
        for volta in range(4):
            for nome, esperado in (("Trazer cadastros", "importar"), ("Pendências de identificação (", "pendencias")):
                alvo = [(x, y) for x, y, tx in so_textos(t) if tx.startswith(nome) and tx != "Trazer cadastros pelo XML"]
                if not alvo:
                    problemas.append(f"volta {volta}: sem a pílula {nome}")
                    continue
                tela.clicar_ponto(*alvo[0])
                time.sleep(1.5)
                t = rr.textos()
                if peca_atras(t) or titulo(t) != esperado:
                    problemas.append(f"volta {volta}, {nome}: título {titulo(t)}, peça atrás {peca_atras(t)}")
        rr.foto("-1-alternou")
        conferir("P2", "alternar 4 vezes entre as opções muda só a janela; Peças fica intacta", not problemas,
                 "; ".join(problemas[:4]))
        abrir = igual(t, "Abrir pendência")
        if abrir:
            tela.clicar_ponto(abrir[0][0], abrir[0][1])
            time.sleep(1.2)
        t = rr.textos()
        rr.foto("-2-pendencia")
        conferir("P3", "a pendência aberta tem Escolher e Cadastrar, e não tem mais o Todas as pendências",
                 igual(t, "Escolher peça existente") and not igual(t, "Todas as pendências") and not peca_atras(t),
                 f"Escolher {igual(t, 'Escolher peça existente')[:1]}, Todas {igual(t, 'Todas as pendências')[:1]}")
        rr.clicar_seta_de_cima()
        time.sleep(1.2)
        t = rr.textos()
        rr.foto("-3-seta")
        conferir("P4", "a seta volta da pendência para a lista, com a janela aberta e Peças intacta",
                 titulo(t) == "pendencias" and igual(t, "Abrir pendência") and not igual(t, "Escolher peça existente")
                 and not peca_atras(t), f"título {titulo(t)}, peça atrás {peca_atras(t)}")
        abrir = igual(t, "Abrir pendência")
        if abrir:
            tela.clicar_ponto(abrir[0][0], abrir[0][1])
            time.sleep(1.2)
        # P6: o Esc na janelinha de cima (Qual é a peça?) fecha só ela.
        t = rr.textos()
        escolher = igual(t, "Escolher peça existente")
        if escolher:
            tela.clicar_ponto(escolher[0][0], escolher[0][1])
            time.sleep(1.5)
        t = rr.textos()
        abriu_escolher = bool(achar(t, "Qual é a peça?"))
        xdo("key", "Escape")
        time.sleep(1.2)
        t = rr.textos()
        rr.foto("-3b-esc-de-cima")
        conferir("P6", "com outra janelinha por cima (Qual é a peça?), o Esc fecha só ela; a pendência continua aberta",
                 abriu_escolher and not achar(t, "Qual é a peça?") and igual(t, "Escolher peça existente")
                 and titulo(t) == "pendencias" and not peca_atras(t),
                 f"abriu a de cima {abriu_escolher}, de cima ainda aberta {bool(achar(t, 'Qual é a peça?'))}, "
                 f"pendência aberta {bool(igual(t, 'Escolher peça existente'))}, título {titulo(t)}")
        xdo("key", "Escape")
        time.sleep(1.2)
        t = rr.textos()
        volta_esc = titulo(t) == "pendencias" and bool(igual(t, "Abrir pendência"))
        xdo("key", "Escape")
        time.sleep(1.2)
        t = rr.textos()
        rr.foto("-4-esc")
        conferir("P5", "o Esc faz o mesmo que a seta: volta à lista e, na lista, fecha a janela; Peças intacta",
                 volta_esc and titulo(t) is None and not peca_atras(t)
                 and achar(t, "Qualquer código, nome, modelo"),
                 f"voltou à lista {volta_esc}, título depois {titulo(t)}, peça atrás {peca_atras(t)}")
    finally:
        rr.fechar()


def main():
    for p in ("Xvfb", "xdotool", "import", "cargo"):
        if not shutil.which(p):
            sys.exit(f"Falta o programa {p}")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-t054-")
    os.makedirs(fotos, exist_ok=True)
    modelo = tempfile.mkdtemp(prefix="rustinho-dados-t054-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1920x1080x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    casos = os.environ.get("RUSTINHO_CASOS", "CTOFP")
    try:
        if xvfb.poll() is not None:
            sys.exit(f"Xvfb não abriu na tela {TELA}")
        montar_loja(modelo)
        if "C" in casos:
            casos_compras(modelo, fotos, 1280, 760, True)
            for w, h in TAMANHOS_MENORES:
                casos_compras(modelo, fotos, w, h, False)
        if "T" in casos:
            casos_trocas(modelo, fotos, 1920, 1080, True)
            for w, h in ((1280, 760),) + TAMANHOS_MENORES:
                casos_trocas(modelo, fotos, w, h, False)
        if "O" in casos:
            casos_ordens(modelo, fotos, 1920, 1080, True)
            for w, h in ((1280, 760),) + TAMANHOS_MENORES:
                casos_ordens(modelo, fotos, w, h, False)
        if "P" in casos:
            modelo_pecas = tempfile.mkdtemp(prefix="rustinho-dados-t058-")
            montar_loja_pecas(modelo_pecas)
            casos_pendencias(modelo_pecas, fotos)
        if "F" in casos:
            for secao in ("pagar", "cheques"):
                casos_financeiro(modelo, fotos, secao, 1920, 1080, True)
                for w, h in ((1280, 760),) + TAMANHOS_MENORES:
                    casos_financeiro(modelo, fotos, secao, w, h, False)
    finally:
        xvfb.terminate()
    errados = [r for r in resultados if not r[1] and r[0] not in ESPERADAS]
    esperadas = [r for r in resultados if not r[1] and r[0] in ESPERADAS]
    passaram = [r for r in resultados if r[1] and r[0] in ESPERADAS]
    for r in esperadas:
        print(f"falha esperada {r[0]}: {ESPERADAS[r[0]]}")
    for r in passaram:
        print(f"✘ a falha esperada {r[0]} passou: tirar de ESPERADAS")
    print(f"\ncertos {len(resultados) - len(errados) - len(esperadas)}, errados {len(errados)}, "
          f"falhas esperadas {len(esperadas)} (de {len(resultados)})")
    sys.exit(1 if errados or passaram else 0)


main()
