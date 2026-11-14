#!/usr/bin/env python3
"""Dados com marcação de HTML aparecem como texto (H10, extensão; 7.2, dev.3.3).

No Qt, um texto sem `textFormat` usa o formato automático: se o texto parece
HTML, ele é desenhado como HTML. Um nome de peça, de cliente ou de fornecedor,
ou uma observação, com <b>, <i>, <img …> ou <a …> (digitado, vindo de uma
nota ou de outro caixa) vira negrito, imagem ou link, e parte dele some.

Abre o Rustinho de verdade numa tela virtual (Xvfb), no ambiente neutro dos
outros roteiros, com dados SINTÉTICOS gravados pelo núcleo na loja de teste:

  T1  Peças: o nome da peça na lista.
  T2  Clientes: o nome do cliente na lista.
  T3  Fornecedores: o nome do fornecedor na lista.
  T4  Venda: a observação da venda.
  T5  Venda: o nome do cliente no título.
  T6  Peça › Marca: digitado "f7c", a sugestão de marca com <img …> aparece
      como texto e o programa continua aberto (no Qt 6.12, o rótulo padrão dos
      botões e das listas do Qt fecha o programa com um <img …>).
  T7  Nenhum TypeError/ReferenceError nos registros.

Cada dado tem a forma
  <início> <b>F7C</b> &amp; <i>teste</i> <img src="f7c-<campo>.png"> <a href="https://exemplo.invalid/f7c-<campo>">ver</a>
e o texto confere quando:
  (a) o Qt NÃO tentou abrir a imagem do <img> (no registro, "Cannot open:
      …f7c-<campo>.png" só aparece quando o texto é lido como HTML);
  (b) a largura natural do texto (linhas RET:, --medir-toque) é a de todos os
      caracteres do dado: por caractere, pelo menos 0,2 da altura de uma linha
      (desenhado como HTML sobram ~25 dos ~110 caracteres: ~0,08).

Antes da correção (o formato automático), T1–T6 são falhas ESPERADAS
(ESPERADAS abaixo); a correção tira cada uma da lista. Se uma esperada passar,
o roteiro avisa e reprova (é hora de tirá-la da lista).

    python3 app/testes/telas_texto_puro.py [pasta para as fotos]

Precisa de: Xvfb, xdotool, ImageMagick (import), Pillow, cargo.
"""

import math
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

os.environ.setdefault("RUSTINHO_TELA", ":62")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_unidades as u  # noqa: E402

base, tema = u.base, u.tema
TELA = u.TELA

# Item → motivo. A correção tira os que resolve.
ESPERADAS = {
    "T1": "H10 ext.: o nome da peça (LinhaPeca) no formato automático",
    "T2": "H10 ext.: o nome do cliente (LinhaPessoa) no formato automático",
    "T3": "H10 ext.: o nome do fornecedor (LinhaPessoa) no formato automático",
    "T4": "H10 ext.: a observação da venda (Nota) no formato automático",
    "T5": "H10 ext.: o título da venda (Cabecalho › Titulo) no formato automático",
    "T6": "H10 ext.: a sugestão do Campo (ItemDelegate, rótulo padrão do Qt) no formato automático",
}


def marcado(inicio, campo):
    """O dado sintético com marcação (sem aspas simples: vai numa string SQL)."""
    return (f'{inicio} <b>F7C</b> &amp; <i>teste</i> <img src="f7c-{campo}.png"> '
            f'<a href="https://exemplo.invalid/f7c-{campo}">ver</a>')


PECA = marcado("Junta", "peca")
CLIENTE = marcado("Cliente", "cliente")
FORNECEDOR = marcado("Fornecedor", "fornecedor")
OBSERVACAO = marcado("Obs.", "observacao")
TITULO = marcado("Comprador", "titulo")
MARCA = marcado("Marca", "marca")


def rets(textos):
    """[(x, y, w, h, largura do texto, altura do texto, largura natural, componente, texto)]."""
    lista = []
    for _, _, t in textos:
        if t.startswith("RET:"):
            numeros, _, comp, texto = t[4:].split("|", 3)
            v = [float(n) for n in numeros.split(",")]
            lista.append((*v[:7], comp, texto))
    return lista


def abriu_imagem(log, campo):
    """O Qt tentou abrir a imagem do <img> deste campo (o texto foi lido como HTML)."""
    with open(log, errors="replace") as f:
        return [linha.strip() for linha in f if "Cannot open" in linha and f"f7c-{campo}.png" in linha]


def largura_por_caractere(r, dado):
    """A largura natural por caractere do dado, em alturas de linha."""
    _, _, w, _, _, ch, natural, _, _ = r
    linhas = max(1, math.ceil(natural / w)) if w > 0 else 1
    altura_linha = ch / linhas if ch > 0 else 1
    return natural / (len(dado) * altura_linha)


def medir(rr, campo, dado, textos):
    """(certo, detalhe) do texto com o dado deste campo na tela."""
    achados = [r for r in rets(textos) if f"f7c-{campo}.png" in r[8]]
    imagem = abriu_imagem(rr.saida, campo)
    if not achados:
        return None, f"o texto com o dado não apareceu na tela (veja {rr.saida})"
    medidas = [(r[7], round(largura_por_caractere(r, dado), 3), round(r[6])) for r in achados]
    largura_ok = all(m[1] >= 0.2 for m in medidas)
    detalhe = (f"(componente, largura por caractere em linhas, largura natural): {medidas}; "
               f"o Qt tentou abrir a imagem do <img>: {'SIM, ' + imagem[0][:140] if imagem else 'não'}")
    return (largura_ok and not imagem), detalhe


def nova(modelo):
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-puro-")
    shutil.rmtree(pasta)
    shutil.copytree(modelo, pasta)
    return pasta


def abrir(pasta, tela, props, fotos, nome):
    rr = u.Rustinho(pasta, tela, props, fotos, nome)
    time.sleep(1.5)
    rr.entrar("Ana")
    time.sleep(2.5)
    return rr


def main():
    for p in ("Xvfb", "xdotool", "import", "cargo"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(u.PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({u.PROGRAMA})")
    if TELA in (":0", ":1"):
        sys.exit(f"tela {TELA} recusada: use uma tela virtual")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-puro-")
    os.makedirs(fotos, exist_ok=True)
    modelo = tempfile.mkdtemp(prefix="rustinho-loja-puro-modelo-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1920x1080x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    resultados = {}
    logs = []

    def conferir(codigo, nome, certo, detalhe=""):
        resultados[codigo] = (bool(certo), nome, detalhe)

    def nao_medido(codigo, nome, detalhe):
        conferir(codigo, f"{nome}: NÃO MEDIDO", False, detalhe)
        ESPERADAS.pop(codigo, None)

    try:
        if xvfb.poll() is not None:
            sys.exit(f"o Xvfb não abriu na tela {TELA} (ocupada?)")
        filtro, _, oleo = base.montar_loja(modelo)
        c = sqlite3.connect(os.path.join(modelo, "rustinho.db"))
        pastilha = c.execute("SELECT id FROM produto WHERE nome = 'Pastilha de freio'").fetchone()[0]
        ze = c.execute("SELECT id FROM pessoa WHERE nome = 'Zé da Oficina'").fetchone()[0]
        fornecedor = c.execute("SELECT id FROM pessoa WHERE nome = 'Distribuidora Vale'").fetchone()[0]
        venda = c.execute("SELECT id FROM venda WHERE cliente_id = ? ORDER BY rowid LIMIT 1", (ze,)).fetchone()[0]
        c.close()
        # Pelo núcleo (os gatilhos do banco usam funções dele). Dados sintéticos.
        tema.por_fora(modelo, f"UPDATE produto SET nome = '{PECA}' WHERE id = '{pastilha}';"
                              f"UPDATE produto SET marca = '{MARCA}' WHERE id = '{oleo}';"
                              f"UPDATE pessoa SET nome = '{CLIENTE}' WHERE id = '{ze}';"
                              f"UPDATE pessoa SET nome = '{FORNECEDOR}' WHERE id = '{fornecedor}';"
                              f"UPDATE venda SET observacoes = '{OBSERVACAO}', cliente_nome = '{TITULO}'"
                              f" WHERE id = '{venda}';")

        listas = [
            ("T1", "Peças: o nome da peça na lista", "TelaPecas", {}, "peca", PECA, "t1_pecas"),
            ("T2", "Clientes: o nome do cliente na lista", "TelaPessoas", {"papel": "cliente"}, "cliente", CLIENTE,
             "t2_clientes"),
            ("T3", "Fornecedores: o nome do fornecedor na lista", "TelaPessoas", {"papel": "fornecedor"},
             "fornecedor", FORNECEDOR, "t3_fornecedores"),
        ]
        for codigo, nome, tela, props, campo, dado, arquivo in listas:
            rr = abrir(nova(modelo), tela, props, fotos, arquivo)
            logs.append(rr.saida)
            try:
                t = rr.textos()
                rr.foto("")
                certo, detalhe = medir(rr, campo, dado, t)
                if certo is None:
                    nao_medido(codigo, nome, detalhe)
                else:
                    conferir(codigo, f"{nome} aparece como texto", certo, detalhe)
            finally:
                rr.fechar()

        # ---- T4 e T5: a venda (observação e cliente no título) ----
        rr = abrir(nova(modelo), "TelaVenda", {"vendaId": venda}, fotos, "t4_t5_venda")
        logs.append(rr.saida)
        try:
            t = rr.textos()
            rr.foto("")
            for codigo, nome, campo, dado in (("T4", "Venda: a observação", "observacao", OBSERVACAO),
                                              ("T5", "Venda: o cliente no título", "titulo", TITULO)):
                certo, detalhe = medir(rr, campo, dado, t)
                if certo is None:
                    nao_medido(codigo, nome, detalhe)
                else:
                    conferir(codigo, f"{nome} aparece como texto", certo, detalhe)
        finally:
            rr.fechar()

        # ---- T6: a sugestão de marca (lista do Campo) ----
        nome = "Peça › Marca: a sugestão com <img> aparece como texto, e o programa continua aberto"
        rr = abrir(nova(modelo), "TelaPeca", {"produtoId": filtro}, fotos, "t6_sugestao_marca")
        logs.append(rr.saida)
        try:
            t = rr.textos()
            # O rótulo "Marca" (um Rotulo dentro do Campo) e, logo abaixo, o campo.
            rotulo = [r for r in rets(t) if r[7] in ("Rotulo", "Campo") and base.simples(r[8]) == "marca"]
            campos = []
            for _, _, s in t:
                if s.startswith("TOQUE:campo|"):
                    p = s[len("TOQUE:"):].split("|")
                    campos.append((p[0], p[1], p[2], p[3], *[float(v) for v in p[4].split(",")]))
            alvo = None
            if rotulo:
                rx, ry, rw, rh = rotulo[0][:4]
                abaixo = [c for c in campos if c[4] <= rx + rw and c[4] + c[6] >= rx and 0 <= c[5] - (ry + rh) < 40]
                if abaixo:
                    alvo = min(abaixo, key=lambda c: c[5])
            if not alvo:
                nao_medido("T6", nome, f"não achei o campo Marca (rótulo: {bool(rotulo)}; veja {rr.saida})")
            else:
                rr.clicar_em(alvo[4:8])
                time.sleep(0.8)
                base.xdo("key", "ctrl+a")
                base.xdo("type", "--delay", "120", "f7c")
                time.sleep(2.5)
                if rr.p.poll() is not None:
                    conferir("T6", nome, False, f"o programa FECHOU ao mostrar a sugestão (código {rr.p.returncode})")
                else:
                    try:
                        t = rr.textos()
                    except RuntimeError as e:
                        t = None
                        conferir("T6", nome, False, f"o programa parou de responder ({e}; código {rr.p.poll()})")
                    if t is not None:
                        rr.foto("")
                        certo, detalhe = medir(rr, "marca", MARCA, t)
                        if certo is None:
                            nao_medido("T6", nome, detalhe)
                        else:
                            conferir("T6", nome, certo, detalhe)
        finally:
            rr.fechar()
    finally:
        xvfb.terminate()
        xvfb.wait(10)
        shutil.rmtree(modelo, ignore_errors=True)

    erros = []
    for caminho in logs:
        erros += [caminho + ": " + e for e in tema.erros_de_script(caminho)]
    conferir("T7", "nenhum TypeError/ReferenceError nos registros", not erros, "; ".join(erros[:3]))

    reprovado = False
    for codigo in sorted(resultados):
        certo, nome, detalhe = resultados[codigo]
        if certo and codigo in ESPERADAS:
            print(f"  ⚠ {codigo} {nome}: PASSOU, mas estava como falha esperada ({ESPERADAS[codigo]}) — tirar da lista")
            reprovado = True
        elif certo:
            print(f"  ✔ {codigo} {nome} · {detalhe}")
        elif codigo in ESPERADAS:
            print(f"  ✘ {codigo} {nome} — falha ESPERADA: {ESPERADAS[codigo]} · {detalhe}")
        else:
            print(f"  ✘ {codigo} {nome} — {detalhe}")
            reprovado = True
    print(f"\n{sum(1 for v in resultados.values() if v[0])} de {len(resultados)} certas; fotos em {fotos}")
    sys.exit(1 if reprovado else 0)


if __name__ == "__main__":
    main()
