#!/usr/bin/env python3
"""QT-1b (dev3.3): o aviso de fechamento inesperado, pela tela.

Com o Rustinho de verdade numa tela virtual (Xvfb), sem Wayland:
  1. abre e entra; sem queda antes, não há aviso;
  2. o processo morre de verdade (SIGKILL, como o reinício do Qt ou uma queda):
     nada do encerramento limpo roda;
  3. por fora (com o Rustinho fechado), uma venda gravada e um recebimento que
     começou e não terminou, nessa abertura;
  4. abre de novo: o Início mostra "Fechamento inesperado" com a venda, e só
     ela (nada do que foi gravado antes dessa abertura, nem no mesmo segundo)
     (número e total), o recebimento "não concluída", "Ver vendas do caixa" e
     "Conferi"; nunca diz que algo só da tela foi salvo;
  5. "Conferi" tira o aviso sem gravar, refazer nem desfazer operação;
  6. abre de novo: o aviso não volta.

    RUSTINHO_TELA=:88 python3 app/testes/telas_qt1b_fechamento.py [pasta das fotos]
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_navegacao_casos as casos  # noqa: E402
from telas_navegacao_casos import PROGRAMA, TELA, Placar, ler, montar_loja, por_fora  # noqa: E402

TITULO = "Atenção: o Rustinho fechou sem o encerramento normal"
AGORA = "strftime('%Y-%m-%dT%H:%M:%SZ', 'now')"
# O momento em milissegundos (é ele que decide o que entra no aviso).
AGORA_MS = "CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)"


def morrer(r):
    """Como a morte do processo: SIGKILL, sem nenhum encerramento."""
    r.p.kill()
    r.p.wait(10)
    r.log.close()


def roteiro(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-qt1b-")
    try:
        montar_loja(pasta)
        r = placar.abrir(pasta, "TelaInicio", {}, fotos, "qt1b-1")
        try:
            r.entrar("Ana")
            f = r.foto("-1")
            ok("1: sem queda antes, sem o aviso", not r.tem("Fechamento inesperado") and not r.tem(TITULO), f)
        finally:
            morrer(r)
        ok("2: a abertura ficou marcada como não encerrada",
           ler(pasta, "SELECT limpa FROM programa_abertura ORDER BY numero DESC LIMIT 1") == [(0,)])
        por_fora(pasta, f"""
            INSERT INTO operacao_feita (chave, comando, usuario_id, pedido, resultado, feita_em, feita_ms, abertura)
              SELECT randomblob(32), 'venda_concluir', id, randomblob(32), '{{"numero":77,"total":"12,34"}}', {AGORA}, {AGORA_MS},
                     (SELECT MAX(numero) FROM programa_abertura)
                FROM usuario WHERE nome = 'Ana';
            INSERT INTO operacao_em_curso (chave, comando, usuario_id, iniciada_em, iniciada_ms, abertura)
              SELECT randomblob(32), 'receber_baixar', id, {AGORA}, {AGORA_MS}, (SELECT MAX(numero) FROM programa_abertura)
                FROM usuario WHERE nome = 'Ana';""")
        vendas = ler(pasta, "SELECT COUNT(*) FROM venda")[0][0]
        feitas = ler(pasta, "SELECT COUNT(*) FROM operacao_feita")[0][0]
        time.sleep(1.1)  # a reabertura fica depois, no relógio, do que foi plantado
        r = placar.abrir(pasta, "TelaInicio", {}, fotos, "qt1b-2")
        try:
            r.entrar("Ana")
            if not r.tem_igual("Conferi"):
                r.clicar(TITULO)
            f = r.foto("-1")
            ok("4: o aviso aparece depois da queda", r.esperar(lambda: r.tem_igual("Conferi")), f)
            ok("4: com a venda gravada (número e total)", r.tem("Venda · nº 77 · R$ 12,34"), f)
            rotulos = [t.strip() for _, _, t in r.textos()]
            ok("4: só o que aconteceu na abertura que caiu (1 gravada, 1 não concluída; nada da montagem da loja)",
               rotulos.count("Gravada:") == 1 and rotulos.count("Não concluída:") == 1, f)
            ok("4: com o recebimento não concluído (nada gravado)", r.tem("Recebimento") and r.tem("nada foi gravado"), f)
            ok("4: oferece Ver vendas do caixa", r.tem_igual("Ver vendas do caixa"), f)
            ok("4: diz que o que estava só na tela não foi guardado", r.tem("não foi guardado"), f)
            # F-05 da revisão: minimizar pega (vira o quadrado), e abre de novo pelo quadrado.
            time.sleep(1.2)
            r.clicar(TITULO)  # um clique no corpo alterna aberta/minimizada
            f = r.foto("-1m")
            ok("4: minimizar o aviso pega (o Conferi some, fica o quadrado)",
               r.esperar(lambda: not r.tem_igual("Conferi") and r.tem(TITULO)), f)
            r.clicar(TITULO)
            r.esperar(lambda: r.tem_igual("Conferi"))
            time.sleep(1.2)
            r.clicar("Conferi")
            f = r.foto("-2")
            ok("5: Conferi tira o aviso", r.esperar(lambda: not r.tem_igual("Conferi") and not r.tem(TITULO)), f)
            ok("5: Conferi não grava venda nem operação",
               ler(pasta, "SELECT COUNT(*) FROM venda")[0][0] == vendas
               and ler(pasta, "SELECT COUNT(*) FROM operacao_feita")[0][0] == feitas)
            ok("5: fica a marca de quem conferiu",
               ler(pasta, "SELECT COUNT(*) FROM fechamento_visto v JOIN usuario u ON u.id = v.usuario_id "
                          "WHERE u.nome = 'Ana'")[0][0] == 1)
        finally:
            r.fechar()
        r = placar.abrir(pasta, "TelaInicio", {}, fotos, "qt1b-3")
        try:
            r.entrar("Ana")
            time.sleep(1)
            f = r.foto("-1")
            ok("6: o aviso não volta para quem conferiu", not r.tem_igual("Conferi") and not r.tem(TITULO), f)
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    aceitas = [":88", ":89", ":91", ":92", ":93"] + [f":{n}" for n in range(60, 75)]
    if TELA not in aceitas:
        sys.exit(f"use RUSTINHO_TELA=:88, :89, :91 a :93 ou :60 a :74 (pedida: {TELA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-qt1b-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = Placar()
    try:
        try:
            roteiro(placar, fotos)
        except Exception as e:  # parou no meio: conta como falha
            placar.ok(f"o roteiro chegou ao fim ({e})", False)
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
    print("fotos em", fotos)
    sys.exit(0 if placar.errados == 0 else 1)


if __name__ == "__main__":
    casos.TELA = TELA
    main()
