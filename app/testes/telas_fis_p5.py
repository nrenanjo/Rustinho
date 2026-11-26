#!/usr/bin/env python3
"""FIS-P4, FIS-P5 e FIS-P6 (0.9.19, etapa 3; texto A §15 e §16 e doc 10): a configuração fiscal e as regras de ST.

- FIS-P4: uma sequência só (emissor → certificado → testar → condições), sem cartões repetindo "não configurada"
  e sem botão que não leva a lugar nenhum.

- A consulta fica separada da edição: as regras do estado aparecem em "Valendo hoje", "Vão valer" e "Encerradas",
  cada uma com o CEST, se tem ST, o período e a fonte.
- O PIS/Cofins monofásico aparece como informação, com o fim em 31/12/2026; sem NCM, como pendência.
- "Mudar" uma regra que já valeu avisa que o estado, o CEST, o início e o "com ST" ficam como estão.
- "Nova regra" abre o painel; salvar mostra a regra na lista como "Mudada na loja"; "Encerrar" pede confirmação;
  uma regra futura da loja se exclui.

    python3 app/testes/telas_fis_p5.py [pasta para as fotos]
"""

import datetime
import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from telas_d09 import TELA, Rustinho, conferir, montar_loja, xdo  # noqa: E402


def digitar(texto):
    xdo("type", "--delay", "60", texto)


def clicar_rolando(r, texto):
    """Clica no texto exato; se ele estiver fora da área visível (as regras ficam no fim do Fiscal), rola até ele
    (72 px por clique da roda) e então clica."""
    for _ in range(25):
        achados = [(x, y) for x, y, t in r.textos_para_clicar() if t == texto]
        if not achados:
            return False
        visiveis = [(x, y) for x, y in achados if 280 < y < 740]
        if visiveis:
            # A rolagem pela roda anima: espera a lista parar (duas leituras iguais) e clica onde o texto está agora.
            for _ in range(10):
                time.sleep(0.3)
                agora = [(x, y) for x, y, t in r.textos_para_clicar() if t == texto and 280 < y < 740]
                if agora and agora[0] == visiveis[0]:
                    break
                visiveis = agora or visiveis
            xdo("mousemove", str(visiveis[0][0]), str(visiveis[0][1]), "click", "1")
            return True
        botao = "5" if achados[0][1] >= 740 else "4"
        # (acima de 280 fica o cabeçalho, onde a rolagem é recortada)
        xdo("mousemove", "640", "420", "click", "--repeat", "4", botao)
        time.sleep(0.4)
    return False


def rolar_ate(r, texto):
    """Rola até o texto ficar visível, sem clicar (para a foto)."""
    for _ in range(25):
        achados = [y for x, y, t in r.textos_para_clicar() if t == texto]
        if not achados or 280 < achados[0] < 700:
            return
        xdo("mousemove", "640", "420", "click", "--repeat", "3", "5" if achados[0] >= 700 else "4")
        time.sleep(0.6)


def tecla(*teclas):
    for t in teclas:
        xdo("key", t)
        time.sleep(0.2)


def main():
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="fotos-fis-p5-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-fis-p5-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    hoje = datetime.date.today()
    tudo_certo = True
    try:
        montar_loja(pasta)
        # Cód-10 da R3: uma peça com o CSOSN do substituto, para a faixa própria (K9 da R2) aparecer.
        import telas_importar_cadastros as imp
        imp.TELA = TELA
        imp.por_fora(pasta, "UPDATE produto SET csosn = '201' WHERE id = (SELECT id FROM produto WHERE tipo <> 'servico' ORDER BY rowid LIMIT 1);")
        r = Rustinho(pasta, "TelaConfiguracoes", {"secao": "fiscal"}, fotos, "fis-p5")
        try:
            r.entrar("Ana")
            time.sleep(3)
            tudo_certo &= conferir("FIS-P5: as regras do estado, separadas e com a fonte", r.foto("-1"), r.textos(),
                                   tem=["Valendo hoje", "Encerradas", "CEST 01 · sem ST · desde 01/10/2026",
                                        "CEST 01 · com ST · até 30/09/2026 (início não informado na fonte)",
                                        "Fonte: Portaria SRE 34/2026", "lida em 24/11/2026"],
                                   nao_tem=["desde 01/01/2000"])
            tudo_certo &= conferir("K9 (R2): a peça com CSOSN de substituto tem faixa própria, com 383 e 600", r.foto("-1s"), r.textos(),
                                   tem=["Peças com CSOSN de substituto (201, 202, 203)", "rejeição 383", "rejeição 600",
                                        "por exemplo uma pessoa física"])
            tudo_certo &= conferir("FIS-P4: a sequência emissor → certificado → testar → condições, sem repetir", r.foto("-1a"),
                                   r.textos(), tem=["Para emitir notas pelo Rustinho", "1. Emissor fiscal · ainda não disponível",
                                                    "2. Certificado A3 · desligado neste computador", "3. Testar a conexão",
                                                    "4. Condições para emitir"],
                                   nao_tem=["Integração fiscal ainda não configurada", "Ver opções de integração",
                                            "Estado da integração"])
            rolar_ate(r, "1. Emissor fiscal · ainda não disponível")
            r.foto("-1c-sequencia")
            tudo_certo &= conferir("FIS-P5: o monofásico é informação, com o fim em 31/12/2026", r.foto("-1b"),
                                   r.textos(), tem=["não um erro", "31/12/2026"])
            # "Mudar" a primeira regra (SP 01, sem ST, já valeu): o aviso de que o essencial fica.
            if not clicar_rolando(r, "Mudar"):
                raise RuntimeError(f"não achei o botão Mudar (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= conferir("FIS-P5: mudar a regra que já valeu avisa o que fica", r.foto("-2"), r.textos(),
                                   tem=["Mudar a regra de ST", "Esta regra já valeu"])
            tecla("Escape")
            time.sleep(1)
            # Nova regra: SP, CEST 01003, com ST, a partir de hoje.
            if not clicar_rolando(r, "Nova regra"):
                raise RuntimeError(f"não achei o botão Nova regra (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= conferir("FIS-P5: a nova regra abre o painel", r.foto("-3"), r.textos(),
                                   tem=["Nova regra de ST", "Começo do CEST", "Fonte (o ato e onde foi lido)"])
            digitar("01003")
            tecla("Tab", "space", "Tab")
            digitar(hoje.strftime("%d/%m/%Y"))
            time.sleep(0.5)
            if not clicar_rolando(r, "Salvar"):
                raise RuntimeError(f"não achei o botão Salvar (veja {r.saida})")
            time.sleep(2)
            textos = r.textos()
            tudo_certo &= conferir("FIS-P5: a nova regra aparece valendo, como da loja", r.foto("-4"), textos,
                                   tem=["CEST 01003 · com ST · desde " + hoje.strftime("%d/%m/%Y"), "Mudada na loja"],
                                   nao_tem=["Nova regra de ST"])
            # Encerrar pede confirmação e diz o que acontece.
            if not clicar_rolando(r, "Encerrar"):
                raise RuntimeError(f"não achei o botão Encerrar (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= conferir("FIS-P5: encerrar pede confirmação e explica", r.foto("-5"), r.textos(),
                                   tem=["Encerrar esta regra?", "fica no histórico"])
            tecla("Escape")
            time.sleep(1)
            # Uma regra futura da loja: aparece em "Vão valer" e se exclui.
            futura = hoje + datetime.timedelta(days=20)
            clicar_rolando(r, "Nova regra")
            time.sleep(1.5)
            digitar("01004")
            tecla("Tab", "Tab")
            digitar(futura.strftime("%d/%m/%Y"))
            clicar_rolando(r, "Salvar")
            time.sleep(2)
            tudo_certo &= conferir("FIS-P5: a regra futura aparece em \"Vão valer\"", r.foto("-6"), r.textos(),
                                   tem=["Vão valer", "CEST 01004 · sem ST · desde " + futura.strftime("%d/%m/%Y")])
            if not clicar_rolando(r, "Excluir"):
                raise RuntimeError(f"não achei o botão Excluir (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= conferir("FIS-P5: excluir a futura pede confirmação", r.foto("-7"), r.textos(),
                                   tem=["Excluir esta regra futura?", "ainda não valeu"])
            # A janelinha de perigo não confirma com o Enter (de propósito): clica no "Excluir" dela (o último da tela).
            botoes = [(x, y) for x, y, t in r.textos_para_clicar() if t == "Excluir"]
            xdo("mousemove", str(botoes[-1][0]), str(botoes[-1][1]), "click", "1")
            time.sleep(2)
            tudo_certo &= conferir("FIS-P5: a regra futura saiu", r.foto("-8"), r.textos(),
                                   nao_tem=["CEST 01004"])
        finally:
            r.fechar()
    finally:
        xvfb.terminate()
        shutil.rmtree(pasta, ignore_errors=True)
    print("RESULTADO:", "tudo certo" if tudo_certo else "HÁ FALHA")
    return 0 if tudo_certo else 1


if __name__ == "__main__":
    sys.exit(main())
