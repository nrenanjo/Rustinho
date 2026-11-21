#!/usr/bin/env python3
"""Roteiro de tela da T060 (bug 21, BUG-20261121-K7QX): "Trazer cadastros pelo XML" que não cabe na janela.

O proprietário viu (21/11/2026, Windows, 0.9.18-dev.3.3.1): com 475 notas, a prévia passava da altura da janelinha, o
botão Cadastrar ficava fora de alcance e cada aviso saía cortado no meio da frase. Causa (ImportarCadastros.qml): o
Painel limita a altura da janelinha à da janela, mas o miolo não rolava (um Item com um ColumnLayout, o Cadastrar no
fim) e os avisos usavam elide: Text.ElideMiddle.

Abre o Rustinho de verdade numa tela virtual (Xvfb) de 1920 × 1080, com notas em Downloads, lê a prévia numa janela
baixa e, com a janelinha aberta, muda o tamanho da janela por uma lista de tamanhos (o item 13 do §4 do briefing M).
Por tamanho, pelas medidas do programa (--medir-toque):

  R1  o caso: a prévia leu as notas e mostra avisos (só no primeiro tamanho);
  R2  o Cadastrar fica INTEIRO dentro da janelinha e da janela (o retângulo de toque, não só o centro);
  R3  os avisos mais largos que a linha vêm quebrados em mais linhas, e não cortados;
  R4  quando a janelinha está na altura máxima (o conteúdo não cabe), rolar o miolo leva ao último aviso e o
      Cadastrar continua inteiro à vista; quando cabe, todos os avisos já estão dentro da janelinha;
  R5  no alto (antes de rolar), o título e o Escolher arquivos estão dentro da janelinha.
  R6  (no código, T077/AUD-X-1) o erro do Cadastrar, o aviso dos arquivos e o "Abrir a peça" ficam FORA do miolo que
      rola, logo acima do Cadastrar: quem rolou até o fim e apertou o Cadastrar vê a recusa.

Os tamanhos são lógicos. No Qt, a escala de 125% e de 150% desenha a mesma tela numa janela lógica menor (ex.: 1366 ×
768 a 150% são 911 × 512 lógicos); por isso, os tamanhos de 125% e 150% entram como janelas lógicas desse tamanho. Não
é o mesmo que a escala real do Windows (o arredondamento e as fontes), que não é testada aqui.

As notas: sem RUSTINHO_XMLS, 12 notas de teste (assinadas pela PKI de teste; o programa de tela as recusa e cada uma
vira um aviso, no máximo 5 por tipo, importar.rs). Com RUSTINHO_XMLS=<.zip ou pasta>, uma CÓPIA desses arquivos vai
para o Downloads temporário (ex.: o xml's.zip do proprietário: notas reais, lidas numa loja descartável; nada sai do
computador e nada é gravado sem o Cadastrar, que este roteiro não aperta).

    python3 app/testes/telas_importar_cadastros_rolagem.py [pasta para as fotos]
    RUSTINHO_TELA=:86 RUSTINHO_PROGRAMA=/caminho/rustinho python3 app/testes/telas_importar_cadastros_rolagem.py
    RUSTINHO_XMLS="$HOME/Downloads/xml's.zip" python3 app/testes/telas_importar_cadastros_rolagem.py
    RUSTINHO_CADASTRAR=1 (com notas reais): no fim, aperta o Cadastrar e escreve as contagens do banco (CONTAGEM ...)
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_d09 as base  # noqa: E402
import telas_navegacao_guiada as nav  # noqa: E402
import telas_importar_cadastros as imp  # noqa: E402
from telas_d09 import PROGRAMA  # noqa: E402

base.TELA = os.environ.get("RUSTINHO_TELA", ":86")
nav.TELA = base.TELA
imp.TELA = base.TELA
TELA = base.TELA
NOTAS_DE_TESTE = 12
XMLS = os.environ.get("RUSTINHO_XMLS", "")
CADASTRAR = os.environ.get("RUSTINHO_CADASTRAR") == "1"
# (rótulo, largura, altura): o 1º é onde a prévia é lida (baixo, para o conteúdo não caber).
TAMANHOS = [
    ("800x480", 800, 480),
    ("1280x720", 1280, 720), ("1280x720 a 125%", 1024, 576), ("1280x720 a 150%", 853, 480),
    ("1366x768", 1366, 768), ("1366x768 a 125%", 1093, 614), ("1366x768 a 150%", 911, 512),
    ("1920x1080 (a tela toda)", 1920, 1080), ("1920x1080 a 125%", 1536, 864), ("1920x1080 a 150%", 1280, 720),
    ("o mínimo, 340x480", 340, 480),
    ("de volta a 1366x768", 1366, 768),
]


QML_DO_IMPORTAR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "qml", "ImportarCadastros.qml")


def dentro_da_rolagem(caminho=QML_DO_IMPORTAR):
    """R6: o que devia ficar fora do Flickable do miolo e está dentro dele (no texto do QML)."""
    texto = open(caminho, encoding="utf-8").read()
    marca = texto.index('objectName: "rolagemDoImportar"')
    inicio = texto.rindex("Flickable {", 0, marca)
    prof, fim = 0, inicio
    for i in range(inicio, len(texto)):
        prof += {"{": 1, "}": -1}.get(texto[i], 0)
        if prof == 0 and texto[i] == "}":
            fim = i
            break
    miolo = texto[inicio:fim]
    return [nome for nome in ("TextoErro", "painel.avisoArquivos", '"Abrir a peça"') if nome in miolo]


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), check=True)


def janela_id():
    for _ in range(150):
        s = subprocess.run(["xdotool", "search", "--onlyvisible", "--class", "rustinho"],
                           env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
        if s.stdout.split():
            return s.stdout.split()
        time.sleep(0.2)
    raise RuntimeError("INSTRUMENTAÇÃO: não achei a janela do Rustinho para mudar o tamanho")


def redimensionar(largura, altura):
    ids = janela_id()
    for j in ids:
        xdo("windowsize", j, str(largura), str(altura))
        xdo("windowmove", j, "0", "0")
    # Sem gerenciador de janelas, o teclado vai para onde está o ponteiro: ele tem de ficar dentro da janela (o centro
    # da tela virtual pode ficar fora de uma janela pequena, e o Ctrl+Shift+F12 dos textos não chegava).
    xdo("mousemove", str(largura // 2), str(altura // 2))
    xdo("windowfocus", "--sync", ids[0])
    time.sleep(1.5)


def casa_com_as_notas(raiz):
    casa = tempfile.mkdtemp(prefix="casa-", dir=raiz)
    destino = os.path.join(casa, "Downloads")
    os.makedirs(destino)
    if XMLS:
        if os.path.isdir(XMLS):
            shutil.copytree(XMLS, os.path.join(destino, "notas"))
        else:
            shutil.copy(XMLS, os.path.join(destino, os.path.basename(XMLS)))
    else:
        for i in range(1, NOTAS_DE_TESTE + 1):
            shutil.copy(nav.EXEMPLO, os.path.join(destino, f"nota-de-teste-numero-{i:03d}.xml"))
    return casa


def geo(textos, nome):
    for _x, _y, t in textos:
        if t.startswith(f"GEO:{nome}:"):
            return tuple(float(v) for v in t.split(":", 2)[2].split(","))
    return None


def avisos(textos):
    """Os avisos da prévia: [(x, y, largura, altura, largura natural, altura desenhada, texto)]."""
    out = []
    for _x, _y, t in textos:
        if not t.startswith("RET:"):
            continue
        partes = t.split("|")
        texto = partes[-1]
        # O delegate dos avisos é um Text simples (QQuickText); o núcleo dá dois tipos: "<arquivo>: <motivo>" e
        # "Código … em peças diferentes …" (importar.rs, avisos e avisos_repetidos).
        if partes[-2] == "QQuickText" and ((".xml" in texto.lower() and ":" in texto) or texto.startswith("Código ")):
            v = [float(a) for a in t[4:].split("|")[0].split(",")]
            out.append((v[0], v[1], v[2], v[3], v[6], v[5], texto))
    return out


def toque(textos, nome):
    for _x, _y, t in textos:
        if t.startswith("TOQUE:") and t.split("|")[3] == nome:
            return tuple(float(v) for v in t.split("|")[4].split(","))
    return None


def ret_texto(textos, texto):
    for _x, _y, t in textos:
        if t.startswith("RET:") and t.split("|")[-1] == texto:
            return tuple(float(v) for v in t[4:].split("|")[0].split(",")[:4])
    return None


def inteiro_dentro(r, caixa, folga=0.5):
    x, y, w, h = r
    cx, cy, cw, ch = caixa
    return x >= cx - folga and y >= cy - folga and x + w <= cx + cw + folga and y + h <= cy + ch + folga


def main():
    for p in ("Xvfb", "xdotool", "import", "convert"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-importar-rolagem-")
    os.makedirs(fotos, exist_ok=True)
    raiz = tempfile.mkdtemp(prefix="rustinho-importar-rolagem-")
    pasta = os.path.join(raiz, "loja")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1920x1080x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    certo = True

    def ok(nome, condicao, detalhe=""):
        nonlocal certo
        print(("✔ " if condicao else "✘ ") + nome + ("" if condicao else f" ({detalhe})"))
        certo &= bool(condicao)

    dentro = dentro_da_rolagem()
    ok("R6: o erro, o aviso dos arquivos e o Abrir a peça ficam fora do miolo que rola (acima do Cadastrar)",
       not dentro, f"dentro da rolagem: {dentro}")
    antes = os.environ.get("RUSTINHO_ARGS_TESTE")
    os.environ["RUSTINHO_ARGS_TESTE"] = ((antes or "") + " --medir-toque").strip()
    try:
        if xvfb.poll() is not None:
            sys.exit(f"Xvfb não abriu na tela {TELA}")
        base.montar_loja(pasta)
        imp.por_fora(pasta, "UPDATE config SET valor = '11444777000161' WHERE chave = 'loja_cnpj';")
        casa = casa_com_as_notas(raiz)
        r = nav.Rustinho(pasta, "TelaPecas", {}, fotos, "rolagem", casa)
        try:
            _rotulo0, l0, a0 = TAMANHOS[0]
            redimensionar(l0, a0)
            r.foto("-0-janela")
            r.entrar("Ana")
            if not r.clicar("Importar XML"):
                raise RuntimeError("INSTRUMENTAÇÃO: o Importar XML não apareceu")
            r.escolher_pasta()
            leu = r.esperar("arquivos ignorados", 600 if XMLS else 90) or r.esperar("notas lidas", 30)
            time.sleep(2.0)
            for i, (rotulo, largura, altura) in enumerate(TAMANHOS):
                redimensionar(largura, altura)
                time.sleep(1.0)
                nome_foto = f"-{i + 1:02d}-{rotulo.replace(' ', '_').replace('%', 'pc').replace(',', '')}"
                f = r.foto(nome_foto)
                t = r.textos()
                fundo = geo(t, "fundoDaJanelinha")
                lista = avisos(t)
                janela = (0, 0, largura, altura)
                if i == 0:
                    ok(f"R1 o caso: a prévia leu as notas e mostra avisos ({len(lista)})",
                       leu and fundo is not None and len(lista) >= 1, f"leu {leu}, janelinha {fundo}, foto {f}")
                if fundo is None:
                    ok(f"{rotulo}: a janelinha continua aberta", False, f"foto {f}")
                    continue
                c = toque(t, "Cadastrar")
                ok(f"{rotulo}: R2 o Cadastrar fica inteiro dentro da janelinha e da janela",
                   c is not None and inteiro_dentro(c, fundo) and inteiro_dentro(c, janela),
                   f"Cadastrar {c}; janelinha {fundo}; janela {largura} × {altura}; foto {f}")
                largos = [a for a in lista if a[4] > a[2] + 1]
                cortados = [a for a in largos if a[5] < 24]   # uma linha de 12 px tem uns 15 px
                ok(f"{rotulo}: R3 os avisos mais largos que a linha vêm quebrados ({len(largos)} largos)",
                   not cortados, f"{len(cortados)} numa linha só; exemplo {cortados[:1]}; foto {f}")
                titulo = ret_texto(t, "Trazer cadastros pelo XML")
                escolher = toque(t, "Escolher arquivos")
                ok(f"{rotulo}: R5 no alto, o título e o Escolher arquivos ficam dentro da janelinha",
                   titulo is not None and escolher is not None and inteiro_dentro(titulo, fundo)
                   and inteiro_dentro(escolher, fundo), f"título {titulo}; Escolher {escolher}; foto {f}")
                maxima = altura - 2 * (8 if largura < 700 else 16)
                no_maximo = fundo[3] >= maxima - 2
                miolo = geo(t, "rolagemDoImportar")
                if not no_maximo:
                    fora = [a for a in lista if not inteiro_dentro(a[:4], fundo)]
                    ok(f"{rotulo}: R4 o conteúdo cabe: todos os avisos já estão dentro da janelinha",
                       bool(lista) and not fora, f"{len(fora)} fora; janelinha {fundo}; foto {f}")
                    continue
                if miolo is None or not lista:
                    ok(f"{rotulo}: R4 o conteúdo não cabe: rolar o miolo leva ao último aviso", False,
                       f"sem o miolo que rola (GEO:rolagemDoImportar) ou sem avisos: {miolo}; foto {f}")
                    continue
                mx, my, mw, mh = miolo
                xdo("mousemove", str(int(mx + mw / 2)), str(int(my + mh / 2)))
                for _ in range(120):
                    xdo("click", "5")
                    time.sleep(0.03)
                time.sleep(1.0)
                t2 = r.textos()
                miolo2 = geo(t2, "rolagemDoImportar")
                lista2 = avisos(t2)
                fim = max(lista2, key=lambda a: a[1]) if lista2 else None
                visto = (fim is not None and miolo2 is not None and fim[1] >= miolo2[1] - 0.5
                         and fim[1] + fim[3] <= miolo2[1] + miolo2[3] + 0.5)
                c2 = toque(t2, "Cadastrar")
                fundo2 = geo(t2, "fundoDaJanelinha")
                f2 = r.foto(nome_foto + "-rolado")
                ok(f"{rotulo}: R4 o conteúdo não cabe: rolar o miolo leva ao último aviso, e o Cadastrar continua inteiro",
                   visto and c2 is not None and fundo2 is not None and inteiro_dentro(c2, fundo2),
                   f"último aviso {fim}; miolo {miolo2}; Cadastrar {c2}; foto {f2}")
                # de volta ao alto, para o próximo tamanho começar igual
                for _ in range(120):
                    xdo("click", "4")
                    time.sleep(0.03)
                time.sleep(0.8)
            if CADASTRAR:
                # A comparação do que fica gravado (§4 do briefing M): aperta o Cadastrar e conta o que entrou no banco
                # da loja descartável. Quem compara as duas versões é quem roda (as contagens saem com o prefixo
                # CONTAGEM). Na janela de 1920 × 1080: é onde o programa de antes da T060 também mostra o Cadastrar
                # (em 1366 × 768, ele fica fora da janela), para a comparação valer nas duas versões.
                redimensionar(1920, 1080)
                time.sleep(1.0)
                t = r.textos()
                c = toque(t, "Cadastrar")
                if c is None or not inteiro_dentro(c, (0, 0, 1920, 1080)):
                    ok("CADASTRAR: o Cadastrar está à vista em 1920 × 1080", False, f"Cadastrar {c}")
                else:
                    xdo("mousemove", str(int(c[0] + c[2] / 2)), str(int(c[1] + c[3] / 2)), "click", "1")
                    fez = r.esperar("Cadastrados", 900)
                    r.foto("-99-cadastrado")
                    ok("CADASTRAR: a gravação terminou (aviso \"Cadastrados\")", fez, "sem o aviso")
                    for tabela in ("pessoa", "produto", "produto_fornecedor", "produto_gtin", "produto_codigo_antigo",
                                   "xml_pendencia_identificacao", "produto_preco"):
                        n = nav.banco(pasta, f"SELECT COUNT(*) FROM {tabela}")[0][0]
                        print(f"CONTAGEM {tabela} {n}")
                    for acao, n in nav.banco(pasta, "SELECT acao, COUNT(*) FROM auditoria GROUP BY acao ORDER BY acao"):
                        print(f"CONTAGEM auditoria.{acao} {n}")
            with open(r.saida, encoding="utf-8", errors="replace") as reg:
                erros = [linha.strip() for linha in reg if "TypeError" in linha or "ReferenceError" in linha]
            ok("nenhum TypeError/ReferenceError no registro", not erros, str(erros[:3]))
        finally:
            r.fechar()
    finally:
        if antes is None:
            os.environ.pop("RUSTINHO_ARGS_TESTE", None)
        else:
            os.environ["RUSTINHO_ARGS_TESTE"] = antes
        shutil.rmtree(raiz, ignore_errors=True)
        xvfb.terminate()
        xvfb.wait()
    print(f"\nfotos em {fotos}")
    sys.exit(0 if certo else 1)


if __name__ == "__main__":
    main()
