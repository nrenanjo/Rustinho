#!/usr/bin/env python3
"""Juntar este caixa: a seta e o Esc perguntam antes de desistir só quando já
chegou algum dado da loja (H24, passo 7.2; técnica DP-28, diretriz §7:
confirmar o que é irreversível).

Abre o Rustinho de verdade numa tela virtual (Xvfb), no ambiente neutro dos
outros roteiros (telas_d09.ambiente_neutro, conferido em /proc antes de
qualquer clique ou tecla: telas_tema_0918.Rustinho). Cada caso numa abertura
própria, com o seu banco.

Sem dado chegado (um caixa novo, vazio):
  J1  boas-vindas → "juntar" → a senha da rede → "Procurar os outros caixas"
      (a tentativa começa e ninguém responde) → a seta volta às boas-vindas
      SEM pergunta, e a tentativa sai (a marca da entrada e a troca ligada);
  J2  o mesmo com o Esc.
Com dado chegado (o caixa abre no meio de uma entrada, com parte da loja):
  J3  a seta abre a pergunta "Desistir de juntar este caixa?" e, com ela
      aberta, a tela de juntar continua e nada do que chegou sai;
  J4  a seta da pergunta (numa abertura) e o Esc (na outra) fecham a pergunta
      sem efeito: a tela de juntar continua, com o que chegou;
  J5  o Esc abre a mesma pergunta da seta (título, mensagem e botão iguais);
  J6  "Desistir" na pergunta desiste: volta às boas-vindas e o que chegou sai;
  J7  nenhum TypeError/ReferenceError nos registros.

De onde vem o "dado chegado": a loja montada pelo núcleo
(nucleo/tests/lojas_de_teste.rs::montar_juncao_com_dado_chegado). O caixa
novo começou a entrar pelo comando da tela (sync_juntar) e recebeu parte das
alterações de outra loja (a configuração dela, 3 peças e um cliente; nenhum
usuário) pela mesma função que aplica o que chega pela rede (sync::aplicar).
É o estado de uma entrada interrompida: o programa fechou, ou a rede caiu,
antes de chegar o usuário; ao abrir, o Rustinho volta para a tela de juntar.
O que isto NÃO prova: a chegada pela rede, de verdade, com a tela aberta
(não há dois caixas trocando dados aqui), nem quanto tempo esse estado dura
numa entrada comum (os usuários costumam vir no primeiro lote, e a tela
passa sozinha para a entrada em até 2 s).

As falhas de achados que ficam para o passo seguinte são ESPERADAS (não
reprovam, mas aparecem); se uma esperada passar, o roteiro avisa e reprova.

    python3 app/testes/telas_juntar_h24.py [pasta para as fotos]

Precisa de: Xvfb, xdotool, ImageMagick (import), cargo. Sai com 0 se aprovado.
"""

import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

os.environ.setdefault("RUSTINHO_TELA", ":67")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_unidades as u  # noqa: E402  (traz o ambiente neutro e o Rustinho conferido)

base, tema = u.base, u.tema
TELA = u.TELA

ESPERADAS = {}  # J3–J6 (H24) saíram com a correção do passo 7.2
PERGUNTA = "Desistir de juntar este caixa?"
TITULO_JUNTAR = "Juntar este caixa"
SENHA_DA_REDE = "rede-da-loja-teste"


def montar_com_dado(pasta):
    """O caixa no meio da entrada, com parte da loja, pelo núcleo."""
    env = dict(os.environ, RUSTINHO_LOJA_TESTE=pasta)
    subprocess.run(
        ["cargo", "test", "-q", "-p", "rustinho-nucleo", "--test", "lojas_de_teste", "--",
         "--ignored", "--exact", "montar_juncao_com_dado_chegado"],
        cwd=base.RAIZ, env=env, stdout=subprocess.DEVNULL, check=True,
    )


def banco(pasta):
    """O que importa da entrada, lido do banco (só leitura)."""
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    try:
        eu = (c.execute("SELECT valor FROM config WHERE chave = 'dispositivo_id'").fetchone() or [""])[0]
        def n(sql, *p):
            return c.execute(sql, p).fetchone()[0]
        return {
            "usuarios": n("SELECT COUNT(*) FROM usuario"),
            "pecas": n("SELECT COUNT(*) FROM produto"),
            "de_outro_caixa": n("SELECT COUNT(*) FROM alteracao WHERE dispositivo <> ?", eu),
            "tentativa": n("SELECT COUNT(*) FROM config WHERE chave IN ('juntar_tentativa', 'juntar_alvo')"),
            "troca_ligada": n("SELECT COUNT(*) FROM config WHERE chave = 'sync_ligado' AND valor = '1'"),
        }
    finally:
        c.close()


def sem_textos_especiais(textos):
    return [t for _, _, t in textos if not t.startswith(("GEO:", "RET:", "TOQUE:", "TESTE_"))]


def na_tela(textos, texto):
    return any(base.simples(t) == base.simples(texto) for t in sem_textos_especiais(textos))


def boas_vindas(textos):
    return any(base.simples(t).startswith("ja uso o rustinho") for t in sem_textos_especiais(textos))


def pergunta(textos):
    """(título, mensagem, botão) da pergunta aberta, ou None."""
    if not na_tela(textos, PERGUNTA):
        return None
    lista = sem_textos_especiais(textos)
    mensagem = next((t for t in lista if "Se desistir" in t), "")
    botao = "Desistir" if na_tela(textos, "Desistir") else ""
    return PERGUNTA, mensagem, botao


def esperar(rr, condicao, segundos=20):
    fim = time.time() + segundos
    while True:
        t = rr.textos()
        if condicao(t) or time.time() > fim:
            return t
        time.sleep(0.5)


def setas(textos):
    """As setas de retorno (x, y, largura, altura): a da tela vem antes e a da
    janelinha aberta, por cima, depois."""
    return [u.geos([(0, 0, t)])["setaDeRetorno"] for _, _, t in textos if t.startswith("GEO:setaDeRetorno:")]


def acionar(rr, como, textos):
    """A seta de cima de tudo (a da janelinha, se houver) ou o Esc."""
    if como == "seta":
        s = setas(textos)
        if not s:
            return False
        rr.clicar_em(s[-1])
    else:
        base.xdo("key", "Escape")
    time.sleep(1.5)
    return True


def abrir(pasta, fotos, nome, logs):
    rr = u.Rustinho(pasta, "TelaInicio", {}, fotos, nome)
    logs.append(rr.saida)
    return rr


def sem_dado(como, fotos, logs, conferir):
    """J1 (seta) e J2 (Esc): a tentativa começa, nada chega, e volta sem perguntar."""
    codigo = "J1" if como == "seta" else "J2"
    nome = f"a {como}" if como == "seta" else "o Esc"
    pasta = tempfile.mkdtemp(prefix="rustinho-juntar-vazio-")
    rr = abrir(pasta, fotos, f"{codigo.lower()}_sem_dado_{como}", logs)
    try:
        t = esperar(rr, boas_vindas)
        if not boas_vindas(t) or not rr.clicar("Já uso o Rustinho", comeca=True):
            conferir(codigo, f"sem dado chegado, {nome} desiste sem perguntar", False,
                     f"não medido: as boas-vindas não apareceram (veja {rr.saida})")
            return
        t = esperar(rr, lambda x: na_tela(x, TITULO_JUNTAR))
        time.sleep(0.8)
        base.xdo("type", "--delay", "60", SENHA_DA_REDE)
        if not rr.clicar("Procurar os outros caixas"):
            conferir(codigo, f"sem dado chegado, {nome} desiste sem perguntar", False,
                     "não medido: sem o botão \"Procurar os outros caixas\"")
            return
        # Duas voltas do relógio da tela (2 s): o sync_ler já leu a tentativa.
        t = esperar(rr, lambda x: na_tela(x, "Procurando os outros caixas"), 10)
        time.sleep(4.5)
        antes = banco(pasta)
        rr.foto("-1-procurando")
        if antes["tentativa"] == 0 or antes["de_outro_caixa"] != 0:
            conferir(codigo, f"sem dado chegado, {nome} desiste sem perguntar", False,
                     f"não medido: a tentativa não começou ou já tinha dado ({antes})")
            return
        acionar(rr, como, rr.textos())
        depois_t = esperar(rr, lambda x: boas_vindas(x) or pergunta(x) is not None, 8)
        rr.foto("-2-depois")
        depois = banco(pasta)
        conferir(codigo, f"sem dado chegado, {nome} desiste sem perguntar",
                 boas_vindas(depois_t) and pergunta(depois_t) is None
                 and depois["tentativa"] == 0 and depois["troca_ligada"] == 0,
                 f"antes {antes}; depois {depois}; pergunta {pergunta(depois_t)}; "
                 f"boas-vindas {boas_vindas(depois_t)}")
    finally:
        rr.fechar()
        shutil.rmtree(pasta, ignore_errors=True)


def com_dado(como, modelo, fotos, logs, conferir):
    """J3, J4 e J6 (seta) ou J5 e J4 (Esc). Devolve a pergunta vista."""
    pasta = tempfile.mkdtemp(prefix="rustinho-juntar-com-dado-")
    shutil.rmtree(pasta)
    shutil.copytree(modelo, pasta)
    rr = abrir(pasta, fotos, f"com_dado_{como}", logs)
    vista = None
    try:
        t = esperar(rr, lambda x: na_tela(x, TITULO_JUNTAR))
        antes = banco(pasta)
        rr.foto("-1-juntar")
        if not na_tela(t, TITULO_JUNTAR) or antes["de_outro_caixa"] == 0 or antes["usuarios"] != 0:
            for c in (("J3", "J4", "J6") if como == "seta" else ("J5", "J4")):
                conferir(c + ("" if c != "J4" else f"-{como}"), "com dado chegado", False,
                         f"não medido: a tela de juntar com dado chegado não abriu ({antes}; veja {rr.saida})")
            return None
        acionar(rr, como, t)
        t = esperar(rr, lambda x: pergunta(x) is not None or boas_vindas(x), 6)
        rr.foto("-2-pergunta")
        vista = pergunta(t)
        meio = banco(pasta)
        aberta = vista is not None and na_tela(t, TITULO_JUNTAR) and meio == antes
        if como == "seta":
            conferir("J3", "com dado chegado, a seta pergunta antes de desistir (e nada sai com a pergunta aberta)",
                     aberta, f"pergunta {vista}; antes {antes}; com a pergunta {meio}; "
                             f"boas-vindas {boas_vindas(t)}")
        else:
            conferir("J5", "com dado chegado, o Esc pergunta igual à seta", aberta,
                     f"pergunta {vista}; antes {antes}; com a pergunta {meio}; boas-vindas {boas_vindas(t)}")
        if vista is None:
            conferir(f"J4-{como}", "fechar a pergunta não desiste", False, "não medido: a pergunta não abriu")
            if como == "seta":
                conferir("J6", "\"Desistir\" na pergunta desiste", False, "não medido: a pergunta não abriu")
            return None
        # J4: a seta da pergunta (na abertura da seta) ou o Esc (na do Esc) fecham sem efeito.
        acionar(rr, como, t)
        t = esperar(rr, lambda x: pergunta(x) is None, 6)
        rr.foto("-3-pergunta-fechada")
        fechada = banco(pasta)
        conferir(f"J4-{como}", f"{'a seta da pergunta' if como == 'seta' else 'o Esc'} fecha a pergunta sem desistir",
                 pergunta(t) is None and na_tela(t, TITULO_JUNTAR) and not boas_vindas(t) and fechada == antes,
                 f"pergunta {pergunta(t)}; antes {antes}; depois {fechada}")
        if como == "seta":
            # J6: de novo a seta e, agora, "Desistir".
            acionar(rr, "seta", rr.textos())
            t = esperar(rr, lambda x: pergunta(x) is not None, 6)
            if pergunta(t) is None or not rr.clicar("Desistir"):
                conferir("J6", "\"Desistir\" na pergunta desiste", False, "a pergunta não abriu de novo")
                return vista
            t = esperar(rr, boas_vindas, 10)
            rr.foto("-4-desistiu")
            fim = banco(pasta)
            conferir("J6", "\"Desistir\" na pergunta desiste: boas-vindas, e o que chegou sai",
                     boas_vindas(t) and fim["de_outro_caixa"] == 0 and fim["pecas"] == 0 and fim["tentativa"] == 0
                     and fim["troca_ligada"] == 0, f"antes {antes}; depois {fim}")
        return vista
    finally:
        rr.fechar()
        shutil.rmtree(pasta, ignore_errors=True)


def main():
    for p in ("Xvfb", "xdotool", "import", "cargo"):
        if not shutil.which(p):
            sys.exit(f"Falta o programa {p}")

    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-juntar-")
    os.makedirs(fotos, exist_ok=True)
    modelo = tempfile.mkdtemp(prefix="rustinho-dados-juntar-")

    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1920x1080x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    resultados, logs = {}, []

    def conferir(codigo, nome, certo, detalhe=""):
        resultados[codigo] = (bool(certo), nome, detalhe)

    try:
        if xvfb.poll() is not None:
            sys.exit(f"Xvfb não abriu na tela {TELA}")
        montar_com_dado(modelo)
        print(f"   o caixa com dado chegado (pelo núcleo): {banco(modelo)}")
        sem_dado("seta", fotos, logs, conferir)
        sem_dado("esc", fotos, logs, conferir)
        da_seta = com_dado("seta", modelo, fotos, logs, conferir)
        do_esc = com_dado("esc", modelo, fotos, logs, conferir)
        # J5 também confere que é a MESMA pergunta (e J4 junta as duas aberturas).
        if "J5" in resultados and resultados["J5"][0]:
            igual = da_seta is not None and da_seta == do_esc
            resultados["J5"] = (igual, resultados["J5"][1],
                                resultados["J5"][2] + f"; da seta {da_seta}; do Esc {do_esc}")
        j4 = [resultados.pop(k) for k in ("J4-seta", "J4-esc") if k in resultados]
        conferir("J4", "a seta da pergunta e o Esc fecham a pergunta sem desistir",
                 len(j4) == 2 and all(c for c, _, _ in j4), " · ".join(f"{n}: {d}" for _, n, d in j4))
        erros = [e for caminho in logs for e in tema.erros_de_script(caminho)]
        conferir("J7", f"zero TypeError/ReferenceError em {len(logs)} registros", not erros, "; ".join(erros[:5]))
    finally:
        shutil.rmtree(modelo, ignore_errors=True)
        xvfb.terminate()
        xvfb.wait()

    reprovado = False
    for codigo in sorted(resultados, key=lambda c: int(c[1:])):
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
        if certo:
            print(f"      {detalhe}")
    certas = sum(1 for c in resultados.values() if c[0])
    print(f"\n{certas} de {len(resultados)} conferências certas; falhas esperadas: "
          f"{sum(1 for k, v in resultados.items() if not v[0] and k in ESPERADAS)}; fotos em {fotos}")
    sys.exit(1 if reprovado or len(resultados) != 7 else 0)


if __name__ == "__main__":
    main()
