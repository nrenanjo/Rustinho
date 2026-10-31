#!/usr/bin/env python3
"""Teste das notificações do Início: quadrados, faixas, cliques e gestos (0.9.16).

Abre o Rustinho de verdade numa tela virtual (Xvfb) com a loja de teste
(RUSTINHO_LOJA_AVISOS): informativas (parcela vencida, conta a pagar vencida,
encomenda em aberto), uma de atenção (diferença entre caixas) e duas críticas
(computador de outra loja recusado; banco que veio de outro computador). Faz
os cliques e os gestos com o mouse (xdotool) e confere pela tela, pelo banco
(as marcas "minimizada" e "escondida" deste computador) e pelos pixels (o
triângulo de alerta em cada tema).

O quadrado não tem texto: ele é achado pelo nome acessível (o título da
notificação), que o Rustinho escreve junto com os textos da tela.

    python3 app/testes/telas_notificacoes.py [pasta para as fotos]
"""

import datetime
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_d09 as base  # noqa: E402
from telas_d09 import PROGRAMA, RAIZ, simples  # noqa: E402

base.TELA = ":85"
TELA = base.TELA


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), check=True)


def montar_loja(pasta):
    env = dict(os.environ, RUSTINHO_LOJA_TESTE=pasta, RUSTINHO_LOJA_AVISOS="1")
    subprocess.run(
        ["cargo", "test", "-q", "-p", "rustinho-nucleo", "--test", "estavel", "--",
         "--ignored", "--exact", "montar_loja_de_teste"],
        cwd=RAIZ, env=env, stdout=subprocess.DEVNULL, check=True,
    )


def nao_chamar(*_):
    raise RuntimeError("o teste não deve disparar os gatilhos da troca de dados")


def banco(pasta, sql, *p):
    """Só para as marcas deste computador e a recusa da outra loja (chaves que
    não vão para a troca: os gatilhos não rodam; se rodassem, as funções
    abaixo acusariam)."""
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"), timeout=10)
    for nome, n in (("rustinho_id", 0), ("rustinho_agora", 0), ("rustinho_operacao", 0),
                    ("rustinho_historia", 3), ("normalizar", 1)):
        c.create_function(nome, n, nao_chamar)
    try:
        r = c.execute(sql, p).fetchall()
        c.commit()
        return r
    finally:
        c.close()


def marca(pasta, tipo, id_):
    r = banco(pasta, "SELECT valor FROM config WHERE chave = ?", f"aviso_{tipo}.{id_}")
    return r[0][0] if r else None


def brilhos(foto, x, y, largura, altura):
    """Brilho (0–255) de cada ponto de um retângulo da foto."""
    s = subprocess.run(["convert", foto, "-crop", f"{largura}x{altura}+{x}+{y}", "-colorspace", "Gray",
                        "-depth", "8", "txt:-"], capture_output=True, text=True, check=True).stdout
    return [int(linha.split("(")[1].split(")")[0].split(",")[0]) for linha in s.splitlines()[1:] if "(" in linha]


class Rustinho(base.Rustinho):
    def na_tela(self):
        """Os textos dentro da janela (uma notificação que saiu deslizando
        fica fora dela até a tela recarregar)."""
        return [(x, y, t) for x, y, t in self.textos() if x >= 0 and y >= 0]

    def todos(self, pedaco):
        return [(x, y) for x, y, t in self.na_tela() if simples(pedaco) in simples(t)]

    def achar(self, pedaco):
        p = self.todos(pedaco)
        return p[0] if p else None

    def tem(self, pedaco):
        return self.achar(pedaco) is not None

    def clicar_em(self, pedaco):
        p = self.achar(pedaco)
        if not p:
            raise RuntimeError(f"não achei \"{pedaco}\" (veja {self.saida})")
        xdo("mousemove", str(p[0]), str(p[1]), "click", "1")
        time.sleep(0.9)

    def arrastar(self, pedaco, dx):
        p = self.achar(pedaco)
        if not p:
            raise RuntimeError(f"não achei \"{pedaco}\" para arrastar (veja {self.saida})")
        x, y = p
        xdo("mousemove", str(x), str(y))
        xdo("mousedown", "1")
        for i in range(1, 13):
            # O ponteiro para na borda da janela (o cartão vai junto até lá).
            xdo("mousemove", str(max(1, int(x + dx * i / 12))), str(y))
            time.sleep(0.03)
        xdo("mouseup", "1")
        time.sleep(1.2)

    def esperar(self, condicao, segundos):
        for _ in range(int(segundos / 0.5)):
            try:
                if condicao():
                    return True
            except RuntimeError:
                pass
            time.sleep(0.5)
        return False

    def encolher(self, largura, altura):
        ids = subprocess.run(["xdotool", "search", "--onlyvisible", "--class", "rustinho"],
                             env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True).stdout.split()
        for i in ids:
            xdo("windowsize", i, str(largura), str(altura))
        time.sleep(2)


# Títulos (o da faixa e o nome do quadrado são o mesmo título).
RECEBER = "A receber"
PAGAR = "A pagar"
ENCOMENDAS = "Encomendas"
ATENCAO = "Atenção: Caixas"
CRITICA_LOJA = "Crítico: Troca de dados recusada"
CRITICA_CODIGOS = "Crítico: Códigos"
# Botões de ação (só na faixa).
BOTAO_RECEBER = "Ver crediário"
TEXTO_ATENCAO = "diferença entre caixas"   # só aparece na faixa aberta da atenção
BOTAO_CRITICA = "Ver caixas"   # a crítica "outra_loja" leva a Simultaneidade › Caixas
LADO = 92                 # lado do quadrado (janela larga)


def canto_do_triangulo(foto, centro):
    """Brilhos do canto superior direito do quadrado (onde fica o triângulo)."""
    x, y = centro
    return brilhos(foto, x + LADO // 2 - 9 - 18, y - LADO // 2 + 9, 18, 17)


def main():
    for p in ("Xvfb", "xdotool", "import", "convert"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-notif-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-notif-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    certo = True

    def ok(nome, condicao, foto=""):
        nonlocal certo
        print(("✔ " if condicao else "✘ ") + nome + ("" if condicao else f" (foto: {foto})"))
        certo &= bool(condicao)

    try:
        montar_loja(pasta)
        r = Rustinho(pasta, "TelaInicio", {}, fotos, "1-inicio")
        try:
            r.entrar("Ana")
            f = r.foto("-0")
            # 4. Várias críticas: cada uma na sua faixa, expandidas.
            ylj, ycod = r.achar(CRITICA_LOJA), r.achar(CRITICA_CODIGOS)
            ok("4: duas críticas, cada uma na sua faixa", ylj and ycod and abs(ylj[1] - ycod[1]) > 40, f)
            ok("4: críticas expandidas (com o texto)", r.tem("usa a mesma senha da rede"), f)
            # Não críticas começam em quadrado (sem faixa, sem botão).
            ok("não críticas começam em quadrado", all(r.tem(x) for x in (RECEBER, PAGAR, ENCOMENDAS, ATENCAO))
               and not r.tem(BOTAO_RECEBER) and not r.tem(TEXTO_ATENCAO), f)
            # 11. Ordem: busca, faixas críticas, quadrados, grade (sem sobrepor).
            ybusca = r.achar("Buscar peça")[1]
            yq = r.achar(RECEBER)[1]
            ygrade = r.achar("Vender / Receber")[1]
            ok("11: ordem busca, críticas, quadrados, grade", ybusca < min(ylj[1], ycod[1])
               and max(ylj[1], ycod[1]) < yq < ygrade, f)
            # 12. Triângulo PNG do tema claro: preto.
            canto = canto_do_triangulo(f, r.achar(RECEBER))
            ok("12: tema claro, triângulo preto no quadrado", min(canto) < 70, f"{f} brilho mínimo {min(canto)}")

            # 1. Informativa: quadrado → faixa.
            r.clicar_em(RECEBER)
            f = r.foto("-1")
            ok("1: clique no quadrado abre a faixa", r.tem(BOTAO_RECEBER) and marca(pasta, "minimizado", "receber") == "0", f)
            ok("3: as outras não críticas continuam em quadrado", not r.tem(TEXTO_ATENCAO), f)
            # 2 e 3. Atenção: abrir fecha a informativa aberta.
            r.clicar_em(ATENCAO)
            f = r.foto("-2a")
            ok("2: clique no quadrado da atenção abre a faixa", r.tem(TEXTO_ATENCAO)
               and marca(pasta, "minimizado", "conflitos") == "0", f)
            ok("3: uma faixa não crítica por vez", not r.tem(BOTAO_RECEBER)
               and marca(pasta, "minimizado", "receber") == "1", f)
            r.clicar_em(ATENCAO)
            f = r.foto("-2b")
            ok("2: clique na faixa da atenção volta ao quadrado", not r.tem(TEXTO_ATENCAO)
               and marca(pasta, "minimizado", "conflitos") == "1", f)
            # 5. Crítica: clique, direita e esquerda não mudam nada.
            r.clicar_em(CRITICA_LOJA)
            r.arrastar(CRITICA_LOJA, 360)
            r.arrastar(CRITICA_LOJA, -420)
            f = r.foto("-5")
            ok("5: crítica não minimiza nem some (clique, direita, esquerda)", r.tem(CRITICA_LOJA)
               and r.tem(BOTAO_CRITICA) and r.tem("usa a mesma senha da rede")
               and marca(pasta, "minimizado", "outra_loja") is None and marca(pasta, "escondido", "outra_loja") is None, f)
            # 9. Botão interno só faz a ação.
            r.clicar_em(RECEBER)
            r.clicar_em(BOTAO_RECEBER)
            time.sleep(1)
            f = r.foto("-9")
            ok("9: o botão leva ao crediário", not r.tem(CRITICA_LOJA) and r.tem("Receber"), f)
            ok("9: e não alterna a notificação", marca(pasta, "minimizado", "receber") == "0")
            xdo("key", "Escape")
            ok("9: de volta, a faixa continua aberta", r.esperar(lambda: r.tem(BOTAO_RECEBER), 8), r.foto("-9b"))
            # 6. Direita na faixa informativa: volta ao quadrado, sem esconder.
            r.arrastar(RECEBER, 360)
            f = r.foto("-6a")
            ok("6: direita na faixa informativa volta ao quadrado", not r.tem(BOTAO_RECEBER) and r.tem(RECEBER)
               and marca(pasta, "minimizado", "receber") == "1" and marca(pasta, "escondido", "receber") is None, f)
            # 6. Esquerda no quadrado da atenção: continua, nada escondido.
            r.arrastar(ATENCAO, -420)
            f = r.foto("-6b")
            ok("6: esquerda na atenção: continua em quadrado", r.tem(ATENCAO)
               and marca(pasta, "escondido", "conflitos") is None, f)
            # 6. Esquerda no quadrado da informativa: some até a meia-noite.
            r.arrastar(RECEBER, -420)
            f = r.foto("-6c")
            ok("6: esquerda na informativa: some", not r.tem(RECEBER), f)
            ok("6: e fica marcada como escondida hoje", marca(pasta, "escondido", "receber") is not None)
            # Arrasto curto não muda nada nem vira clique.
            antes = marca(pasta, "minimizado", "conflitos")
            r.arrastar(ATENCAO, 40)
            f = r.foto("-6d")
            ok("arrasto curto não muda nada", marca(pasta, "minimizado", "conflitos") == antes
               and not r.tem(TEXTO_ATENCAO), f)
            # 12. Tema escuro: o triângulo vira o PNG branco, sem reiniciar.
            sair = r.achar("Trocar usuário")
            xdo("mousemove", str(sair[0] - 112), str(sair[1]), "click", "1")
            time.sleep(1.5)
            f = r.foto("-12-escuro")
            canto = canto_do_triangulo(f, r.achar(PAGAR))
            ok("12: tema escuro, triângulo branco no quadrado", max(canto) > 190, f"{f} brilho máximo {max(canto)}")
            xdo("mousemove", str(sair[0] - 142), str(sair[1]), "click", "1")
            time.sleep(1)
        finally:
            r.fechar()

        # 8. Fechar e abrir: a atenção continua minimizada; a informativa,
        # escondida (hoje); as críticas, abertas.
        r = Rustinho(pasta, "TelaInicio", {}, fotos, "2-reabrir")
        try:
            r.entrar("Ana")
            f = r.foto("-8")
            ok("8: atenção continua em quadrado depois de abrir de novo", r.tem(ATENCAO)
               and not r.tem(TEXTO_ATENCAO), f)
            ok("8: informativa escondida continua escondida hoje", not r.tem(RECEBER), f)
            ok("8: críticas continuam abertas", r.tem(CRITICA_LOJA) and r.tem(CRITICA_CODIGOS), f)
            # 7. "Meia-noite": a marca de escondida passa a ser de ontem.
            ontem = (datetime.date.today() - datetime.timedelta(days=1)).isoformat()
            banco(pasta, "UPDATE config SET valor = ? WHERE chave = 'aviso_escondido.receber'", ontem)
            ok("7: no outro dia, a informativa volta sozinha", r.esperar(lambda: r.tem(RECEBER), 30), r.foto("-7"))
            # 10. Janela estreita: os 4 quadrados não cabem numa linha e quebram,
            # sem sair da janela; a grade fica abaixo deles.
            r.encolher(340, 900)
            f = r.foto("-10-estreita")
            pos = [p for t in (RECEBER, PAGAR, ENCOMENDAS, ATENCAO) for p in r.todos(t)]
            ys = sorted({y for _, y in pos})
            ok("10: na janela estreita, os quadrados quebram linha", len(pos) == 4 and len(ys) >= 2
               and max(x for x, _ in pos) < 340, f"{f} {pos}")
            grade = r.achar("Vender / Receber")
            ok("11: a grade continua abaixo dos quadrados", grade is None or grade[1] > max(ys), f)
            r.encolher(1280, 760)
            # 13. Resolver uma ocorrência não mexe nas outras.
            banco(pasta, "DELETE FROM config WHERE chave = 'sync_outra_loja'")
            ok("13: resolvida, a crítica da outra loja sai", r.esperar(lambda: not r.tem(CRITICA_LOJA), 30),
               r.foto("-13"))
            f = r.foto("-13b")
            ok("13: a outra crítica e os quadrados continuam", r.tem(CRITICA_CODIGOS) and r.tem(ATENCAO)
               and r.tem(RECEBER), f)
            # 14. Sem duplicatas.
            t = [simples(x) for _, _, x in r.na_tela()]
            ok("14: uma de cada (sem duplicata)", t.count(simples(ATENCAO)) == 1 and t.count(simples(RECEBER)) == 1
               and t.count(simples(CRITICA_CODIGOS)) == 1, f)
        finally:
            r.fechar()
    finally:
        xvfb.terminate()
        shutil.rmtree(pasta, ignore_errors=True)
    print("fotos em", fotos)
    sys.exit(0 if certo else 1)


if __name__ == "__main__":
    main()
