#!/usr/bin/env python3
"""R24 / DP-23 (decisão do proprietário, 25/11/2026): o limite de crédito é do gerente.

A Bia (caixa, não é gerente) abre o cadastro de um cliente sem limite e digita um limite:
- ao salvar, a tela pede a senha do gerente ("Só o gerente pode: mudar o limite de crédito
  do cliente") e nada é gravado antes dela;
- com a senha da Ana (gerente), o limite é gravado;
- salvar de novo sem mexer no limite não pede a senha.

Usa a loja de teste e o jeito de ler a tela de `telas_d09.py`.

    python3 app/testes/telas_r24_gerente.py [pasta para as fotos]
"""

import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from telas_d09 import SENHA, TELA, Rustinho, conferir, montar_loja, xdo  # noqa: E402


def limite_gravado(pasta, pessoa):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    v = c.execute("SELECT limite_credito FROM pessoa WHERE id = ?", (pessoa,)).fetchone()[0]
    c.close()
    return v


def main():
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="fotos-r24-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-r24-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    tudo_certo = True
    try:
        montar_loja(pasta)
        c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
        linha = c.execute(
            "SELECT id, nome FROM pessoa WHERE cliente = 1 AND limite_credito IS NULL AND ativo = 1 ORDER BY rowid LIMIT 1"
        ).fetchone()
        c.close()
        if not linha:
            raise RuntimeError("a loja de teste não tem um cliente sem limite")
        cliente, nome = linha
        r = Rustinho(pasta, "TelaPessoa", {"pessoaId": cliente}, fotos, "r24-limite")
        try:
            r.entrar("Bia")
            time.sleep(2)
            if not r.clicar("Sem limite"):
                raise RuntimeError(f"não achei o campo do limite de crédito de {nome} (veja {r.saida})")
            xdo("type", "--delay", "60", "500")
            time.sleep(0.5)
            xdo("key", "F2")
            time.sleep(2)
            tudo_certo &= conferir("limite: a caixa muda o limite e a tela pede o gerente", r.foto("-1"), r.textos(),
                                   tem=["Liberar", "Só o gerente pode: mudar o limite de crédito do cliente"])
            sem_senha = limite_gravado(pasta, cliente)
            print(("✔ " if sem_senha is None else "✘ ") + "limite: nada gravado antes da senha"
                  + ("" if sem_senha is None else f" (gravou {sem_senha})"))
            tudo_certo &= sem_senha is None
            xdo("type", "--delay", "80", SENHA["Ana"])
            xdo("key", "Return")
            time.sleep(3)
            foto = r.foto("-2")
            com_senha = limite_gravado(pasta, cliente)
            print(("✔ " if com_senha == 50000 else "✘ ") + "limite: com a senha do gerente, gravou R$ 500,00"
                  + ("" if com_senha == 50000 else f" (gravado {com_senha}; foto: {foto})"))
            tudo_certo &= com_senha == 50000
            # Salvar de novo sem mexer no limite: não pede a senha.
            for _ in range(40):
                if not any("Alterações salvas" in t for _, _, t in r.textos()):
                    break
                time.sleep(0.25)
            xdo("key", "F2")
            time.sleep(2)
            tudo_certo &= conferir("limite: salvar sem mexer no limite não pede o gerente", r.foto("-3"), r.textos(),
                                   tem=["Alterações salvas"], nao_tem=["Só o gerente pode"])
        finally:
            r.fechar()
    finally:
        xvfb.terminate()
        shutil.rmtree(pasta, ignore_errors=True)
    print("RESULTADO:", "tudo certo" if tudo_certo else "HÁ FALHA")
    return 0 if tudo_certo else 1


if __name__ == "__main__":
    sys.exit(main())
