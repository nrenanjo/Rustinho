#!/usr/bin/env python3
"""FIS-CAT e FIS-LIGA (0.9.19, etapa 3; decisões do proprietário de 20 e 25/11/2026).

- FIS-CAT: o Fiscal é uma pílula própria das Configurações (saiu da Simultaneidade, sem cópia).
- FIS-LIGA: o seletor "Certificado A3 e integração fiscal" fica no Fiscal, desligado de fábrica; só o gerente vê.
  Ligar pede a senha de um gerente digitada na hora (a errada não liga); o estado continua depois de fechar e abrir;
  qualquer gerente desliga, com a confirmação. A Bia (caixa), mesmo com a senha do gerente para consultar, não vê o
  seletor.

    python3 app/testes/telas_fis_liga.py [pasta para as fotos]
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from telas_d09 import SENHA, TELA, Rustinho, conferir, montar_loja, xdo  # noqa: E402


def digitar(texto):
    xdo("type", "--delay", "80", texto)
    xdo("key", "Return")


def main():
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="fotos-fis-liga-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-fis-liga-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    tudo_certo = True
    try:
        montar_loja(pasta)
        # 1. A Ana (gerente): o Fiscal é pílula própria; a Simultaneidade não tem mais a subpílula Fiscal.
        r = Rustinho(pasta, "TelaConfiguracoes", {}, fotos, "fis-1-pilulas")
        try:
            r.entrar("Ana")
            time.sleep(2)
            if not r.clicar("Simultaneidade"):
                raise RuntimeError(f"a pílula Simultaneidade não apareceu (veja {r.saida})")
            time.sleep(2)
            textos = r.textos()
            fiscais = [t for _, _, t in textos if t == "Fiscal"]
            ok = len(fiscais) == 1
            foto = r.foto("-simultaneidade")
            print(("✔ " if ok else "✘ ") + "FIS-CAT: na Simultaneidade, \"Fiscal\" aparece uma vez só (a pílula de cima)"
                  + ("" if ok else f" ({len(fiscais)} vezes; foto: {foto})"))
            tudo_certo &= ok
            tudo_certo &= conferir("FIS-CAT: a Simultaneidade continua com as 4 partes", foto, textos,
                                   igual=["Duplicatas", "Histórico", "Caixas", "Backup"])
            if not r.clicar("Fiscal"):
                raise RuntimeError(f"a pílula Fiscal não apareceu (veja {r.saida})")
            time.sleep(2.5)
            tudo_certo &= conferir("FIS-LIGA: nasce desligado, e o gerente vê o seletor", r.foto("-fiscal"), r.textos(),
                                   tem=["Certificado A3 e integração fiscal", "Desligado neste computador"],
                                   igual=["Ligar"])
            # Ligar com a senha errada não liga.
            r.clicar("Ligar")
            time.sleep(1.5)
            tudo_certo &= conferir("FIS-LIGA: ligar pede a senha e diz o que muda", r.foto("-pede"), r.textos(),
                                   tem=["Ligar o fiscal neste computador", "Nos outros caixas, nada muda"])
            digitar("senha-errada")
            time.sleep(1.5)
            tudo_certo &= conferir("FIS-LIGA: a senha errada não liga", r.foto("-errada"), r.textos(),
                                   tem=["Senha do gerente errada."])
            digitar(SENHA["Ana"])
            time.sleep(2)
            tudo_certo &= conferir("FIS-LIGA: com a senha do gerente, liga", r.foto("-ligado"), r.textos(),
                                   tem=["Ligado neste computador"], igual=["Desligar"])
        finally:
            r.fechar()

        # 2. Fechar e abrir: continua ligado.
        r = Rustinho(pasta, "TelaConfiguracoes", {"secao": "fiscal"}, fotos, "fis-2-reaberto")
        try:
            r.entrar("Ana")
            time.sleep(3)
            tudo_certo &= conferir("FIS-LIGA: depois de fechar e abrir, continua ligado", r.foto("-1"), r.textos(),
                                   tem=["Ligado neste computador"])
        finally:
            r.fechar()

        # 3. A Bia (caixa), com a senha do gerente só para consultar: vê o Fiscal, mas não o seletor.
        r = Rustinho(pasta, "TelaConfiguracoes", {"secao": "fiscal"}, fotos, "fis-3-bia")
        try:
            r.entrar("Bia")
            time.sleep(2)
            tudo_certo &= conferir("FIS-CAT: o Fiscal pede a senha do gerente a quem não é gerente", r.foto("-pede"),
                                   r.textos(), tem=["só o gerente pode"])
            digitar(SENHA["Ana"])
            time.sleep(2.5)
            tudo_certo &= conferir("FIS-LIGA: a Bia consulta o Fiscal, mas não vê o seletor", r.foto("-fiscal"),
                                   r.textos(), tem=["Emissão fiscal"],
                                   nao_tem=["Certificado A3 e integração fiscal", "Ligado neste computador"])
        finally:
            r.fechar()

        # 4. Qualquer gerente desliga, com a confirmação.
        r = Rustinho(pasta, "TelaConfiguracoes", {"secao": "fiscal"}, fotos, "fis-4-desliga")
        try:
            r.entrar("Ana")
            time.sleep(3)
            if not r.clicar("Desligar"):
                raise RuntimeError(f"não achei o botão Desligar (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= conferir("FIS-LIGA: desligar pede a confirmação", r.foto("-confirma"), r.textos(),
                                   tem=["Desligar o fiscal neste computador?"])
            xdo("key", "Return")
            time.sleep(2)
            tudo_certo &= conferir("FIS-LIGA: desligado de novo", r.foto("-desligado"), r.textos(),
                                   tem=["Desligado neste computador"], igual=["Ligar"])
        finally:
            r.fechar()
    finally:
        xvfb.terminate()
        shutil.rmtree(pasta, ignore_errors=True)
    print("RESULTADO:", "tudo certo" if tudo_certo else "HÁ FALHA")
    return 0 if tudo_certo else 1


if __name__ == "__main__":
    sys.exit(main())
