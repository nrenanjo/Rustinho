#!/usr/bin/env python3
"""Roteiro das chamadas com a senha do gerente (dev.3.3, passo 2.1: H11 e H12).

Abre o Rustinho de verdade numa tela virtual 1920 × 1080 (Xvfb), no ambiente
neutro dos outros roteiros, com uma loja montada PELO NÚCLEO
(`nucleo/tests/lojas_de_teste.rs::montar_chamadas_gerente`): Ana (gerente), a
cliente "Oficina do Zé" com R$ 300,00 no crediário e um recebimento de
R$ 100,00, e o backup apontado para `<loja>/backups`. Nada de SQL direto; o
banco só é lido (somente leitura) para conferir o que o núcleo gravou.

  R1  Receber › Estornar › motivo: a janela do motivo fecha;
  R2  a conta mostra a dívida de volta: "3 parcelas" (eram 2), a parcela 1/3
      em aberto de novo e, com "Mostrar valores", R$ 300,00 (eram R$ 200,00);
  R3  nenhum TypeError no registro do app;
  R4  o núcleo gravou exatamente um estorno (banco lido só para leitura);
  B1  Backup › "Fazer backup agora": a tela mostra o resultado ("Backup feito
      e conferido.") e a cópia aparece na pasta;
  B2  com a pasta do backup trocada por um arquivo, a tela mostra o erro.

Antes da correção (4b253c6): R1, R2, R3, B1 e B2 falham (H11: o diálogo de
motivo do Receber usava o retorno de `acao(…)` sem o `pronto`; H12: a seção
de backup passava a função no lugar dos dados e a resposta se perdia).

    python3 app/testes/telas_chamadas_gerente.py [pasta para as fotos]

Precisa de: Xvfb, xdotool, ImageMagick (import), cargo. Sai com 0 se aprovado.
"""

import json
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

os.environ.setdefault("RUSTINHO_TELA", ":86")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_unidades as u  # noqa: E402

base, tema = u.base, u.tema
TELA = u.TELA


def montar_loja(pasta):
    """A loja pelo núcleo (os mesmos comandos das telas)."""
    env = dict(os.environ, RUSTINHO_LOJA_TESTE=pasta)
    subprocess.run(
        ["cargo", "test", "-q", "-p", "rustinho-nucleo", "--test", "lojas_de_teste", "--",
         "--ignored", "--exact", "montar_chamadas_gerente"],
        cwd=base.RAIZ, env=env, stdout=subprocess.DEVNULL, check=True,
    )
    with open(os.path.join(pasta, "ids.json"), encoding="utf-8") as f:
        return json.load(f)


def esperar_janela():
    for _ in range(150):
        s = subprocess.run(["xdotool", "search", "--onlyvisible", "--class", "rustinho"],
                           env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
        if s.stdout.split():
            break
        time.sleep(0.2)
    time.sleep(0.5)


def so_textos(t):
    return [tx for _, _, tx in t if not tx.startswith(("GEO:", "RET:", "TOQUE:", "TESTE_PILHA:"))]


def estornos(pasta):
    """Recebimentos estornados, lendo o banco só para leitura."""
    uri = "file:" + os.path.join(pasta, "rustinho.db") + "?mode=ro"
    c = sqlite3.connect(uri, uri=True)
    try:
        colunas = [r[1] for r in c.execute("PRAGMA table_info(recebimento)")]
        if "estornado_em" in colunas:
            return c.execute("SELECT COUNT(*) FROM recebimento WHERE estornado_em IS NOT NULL").fetchone()[0]
        return c.execute("SELECT COUNT(*) FROM recebimento WHERE estornado = 1").fetchone()[0]
    finally:
        c.close()


def log_do_app(rr):
    rr.log.flush()
    with open(rr.saida, encoding="utf-8", errors="replace") as f:
        return f.read()


def esperar_texto(rr, trechos, segundos):
    """Espera aparecer na tela algum texto com um dos trechos; devolve o texto achado."""
    fim = time.time() + segundos
    while time.time() < fim:
        for tx in so_textos(rr.textos()):
            if any(tr in tx for tr in trechos):
                return tx
        time.sleep(1)
    return ""


def main():
    for p in ("Xvfb", "xdotool", "import", "cargo"):
        if not shutil.which(p):
            sys.exit(f"Falta o programa {p}")

    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-gerente-")
    os.makedirs(fotos, exist_ok=True)
    modelo = tempfile.mkdtemp(prefix="rustinho-dados-gerente-")

    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1920x1080x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    resultados = {}

    def conferir(codigo, nome, certo, detalhe=""):
        resultados[codigo] = (bool(certo), nome, detalhe)

    try:
        if xvfb.poll() is not None:
            sys.exit(f"Xvfb não abriu na tela {TELA}")
        ids = montar_loja(modelo)

        # ---------- H11: estornar no Receber ----------
        rr = u.Rustinho(modelo, "TelaReceber", {"clienteInicial": ids["cliente"]}, fotos, "receber")
        try:
            esperar_janela()
            rr.redimensionar(1920, 1080)
            time.sleep(1)
            rr.entrar("Ana")
            time.sleep(3.0)
            # Os valores começam escondidos (sigilo): o olho mostra.
            rr.clicar("Mostrar valores")
            time.sleep(1.0)
            rr.foto("-1-conta")
            antes = so_textos(rr.textos())
            em_aberto_antes = "R$ 200,00" in antes and any(tx.startswith("2 parcelas") for tx in antes)
            estornos_antes = estornos(modelo)
            if not rr.clicar("Estornar"):
                conferir("R1", "Receber › Estornar abre a pergunta do motivo", False, "botão Estornar não achado")
            else:
                time.sleep(1.5)
                pergunta = esperar_texto(rr, ["Estornar o pagamento?"], 5)
                rr.foto("-2-motivo")
                base.xdo("type", "--delay", "50", "teste do roteiro")
                base.xdo("key", "Return")
                time.sleep(3.0)
                rr.foto("-3-depois")
                depois = so_textos(rr.textos())
                conferir("R1", "a janela do motivo fecha depois de estornar",
                         pergunta and "Estornar o pagamento?" not in depois,
                         f"pergunta={pergunta!r}; ainda aberta={'Estornar o pagamento?' in depois}")
                tres = any(tx.startswith("3 parcelas") for tx in depois)
                primeira = any(tx.endswith("· 1/3") for tx in depois)
                conferir("R2", "a conta mostra a dívida de volta (3 parcelas, a 1/3 em aberto, R$ 300,00)",
                         em_aberto_antes and tres and primeira and "R$ 300,00" in depois and "R$ 200,00" not in depois,
                         f"antes (2 parcelas, R$ 200,00)={em_aberto_antes}; depois: 3 parcelas={tres}, 1/3={primeira}, "
                         f"R$ 300,00={'R$ 300,00' in depois}, R$ 200,00={'R$ 200,00' in depois}")
            registro = log_do_app(rr)
            erros = [ln for ln in registro.splitlines() if "TypeError" in ln]
            conferir("R3", "nenhum TypeError no registro do app", not erros, "; ".join(erros[:2]))
            feitos = estornos(modelo) - estornos_antes
            conferir("R4", "o núcleo gravou exatamente um estorno (banco só lido)", feitos == 1, f"{feitos} estorno(s)")
        finally:
            rr.fechar()

        # ---------- H12: backup manual ----------
        rr = u.Rustinho(modelo, "TelaConfiguracoes", {"secao": "backup"}, fotos, "backup")
        try:
            esperar_janela()
            rr.redimensionar(1920, 1080)
            time.sleep(1)
            rr.entrar("Ana")
            time.sleep(3.0)
            rr.foto("-4-backup")
            pasta_backups = os.path.join(modelo, "backups")
            antes = set(os.listdir(pasta_backups))
            clicou = rr.clicar("Fazer backup agora")
            visto = esperar_texto(rr, ["Backup feito e conferido.", "O backup não deu certo"], 40) if clicou else ""
            rr.foto("-5-backup-feito")
            novos = set(os.listdir(pasta_backups)) - antes
            conferir("B1", "\"Fazer backup agora\" mostra o resultado e a cópia aparece na pasta",
                     visto.startswith("Backup feito e conferido.") and novos,
                     f"clicou={clicou}; tela={visto!r}; arquivos novos={sorted(novos)[:2]}")
            # O erro: a pasta do backup vira um arquivo (nada se grava nela).
            shutil.rmtree(pasta_backups)
            with open(pasta_backups, "w", encoding="utf-8") as f:
                f.write("não é uma pasta\n")
            time.sleep(2.0)
            clicou = rr.clicar("Fazer backup agora")
            erro = esperar_texto(rr, ["O backup não deu certo", "Não deu para gravar na pasta"], 40) if clicou else ""
            rr.foto("-6-backup-erro")
            conferir("B2", "com a pasta trocada por um arquivo, a tela mostra o erro",
                     "no lugar da pasta tem um arquivo" in erro, f"clicou={clicou}; tela={erro!r}")
        finally:
            rr.fechar()
    finally:
        xvfb.terminate()

    falhas = 0
    for codigo in sorted(resultados, key=lambda c: (c[0], int(c[1:]))):
        certo, nome, detalhe = resultados[codigo]
        print(f"{'✔' if certo else '✘'} {codigo} {nome}" + (f" · {detalhe}" if detalhe else ""))
        falhas += 0 if certo else 1
    print(f"fotos em {fotos}")
    sys.exit(1 if falhas else 0)


if __name__ == "__main__":
    main()
