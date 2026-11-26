#!/usr/bin/env python3
"""FIS-P1 e FIS-P2 (0.9.19, etapa 3, pacote E; texto A §12 e §13): o painel das notas e o detalhe.

O Rustinho ainda não emite (o emissor é a T008): o roteiro põe 4 notas de homologação direto no banco da loja de
teste (uma autorizada, uma rejeitada, uma sem resposta e um rascunho) e confere:
- o ambiente à vista, os 4 estados do certificado e os 4 indicadores com as contagens;
- o CPF/CNPJ escondido por padrão;
- o indicador "Sem resposta" filtra, a contagem e "Limpar filtros" aparecem;
- o detalhe da sem resposta: primeiro a explicação humana, e nada de reenviar;
- o detalhe da rejeitada: o código e a mensagem da SEFAZ na íntegra, com "Copiar a mensagem", e o histórico.

    python3 app/testes/telas_fis_p1.py [pasta para as fotos]
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
from telas_d09 import TELA, Rustinho, conferir, montar_loja, xdo  # noqa: E402
from telas_fis_p5 import clicar_rolando, rolar_ate, tecla  # noqa: E402


def semear_notas(pasta):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    agora = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    notas = [
        ("n1", 1, "autorizada", "100", "Autorizado o uso da NF-e", "135260000000001", "12345678909", "Maria"),
        ("n2", 2, "rejeitada", "225", "Rejeição: Falha no Schema XML do lote de NFe", "", "", ""),
        ("n3", 3, "desconhecida", "", "", "", "", ""),
        ("n4", 0, "rascunho", "", "", "", "", ""),
    ]
    for i, numero, estado, cstat, motivo, protocolo, doc, nome in notas:
        c.execute(
            "INSERT INTO nota_fiscal (id, ambiente, modelo, serie, numero, chave, estado, destinatario_doc,"
            " destinatario_nome, valor, cstat, motivo, protocolo, xml_nome, criado_em, atualizado_em, dispositivo)"
            " VALUES (?, 2, 65, 1, ?, ?, ?, ?, ?, 15000, ?, ?, ?, ?, ?, ?, 'teste')",
            (i, numero, ("3526111122233300018165001%09d1000000019" % numero) if numero else "", estado, doc, nome,
             cstat, motivo, protocolo, ("nota-%d-nfe.xml" % numero) if cstat else "", agora, agora))
        c.execute("INSERT INTO nota_fiscal_evento (id, nota_id, quando, de, para, detalhe) VALUES (?, ?, ?, '', ?, ?)",
                  (i + "-e", i, agora, estado, "Semeada pelo teste: " + estado))
    c.commit()
    c.close()


def main():
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="fotos-fis-p1-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-fis-p1-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    tudo_certo = True
    try:
        montar_loja(pasta)
        semear_notas(pasta)
        r = Rustinho(pasta, "TelaConfiguracoes", {"secao": "fiscal"}, fotos, "fis-p1")
        try:
            r.entrar("Ana")
            time.sleep(3)
            rolar_ate(r, "Autorizadas: 1")
            textos = r.textos()
            tudo_certo &= conferir("FIS-P1: ambiente à vista, certificado em 4 estados, indicadores", r.foto("-1"), textos,
                                   tem=["Ambiente: homologação (teste, sem valor fiscal)", "configurado: ainda não",
                                        "conectado: ainda não", "válido: ainda não", "testado: ainda não"],
                                   igual=["Autorizadas: 1", "Rejeitadas: 1", "Sem resposta: 1", "Pendentes: 1"])
            tudo_certo &= conferir("FIS-P1: o CPF/CNPJ fica escondido", r.foto("-1b"), textos,
                                   tem=["documento escondido", "4 notas · todo o período", "NFC-e sem número (série 1) · Rascunho"],
                                   nao_tem=["12345678909", "NFC-e 0 (série 1)", "Nenhuma pendência fiscal"])
            if not clicar_rolando(r, "Sem resposta: 1"):
                raise RuntimeError(f"não achei o indicador Sem resposta (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= conferir("FIS-P1: o indicador filtra, com a contagem e o limpar", r.foto("-2"), r.textos(),
                                   tem=["1 nota · todo o período", "Sem resposta (consultar)"],
                                   igual=["Limpar filtros"], nao_tem=["NFC-e 1 (série 1)"])
            if not clicar_rolando(r, "NFC-e 3 (série 1) · Sem resposta (consultar) · R$ 150,00"):
                raise RuntimeError(f"não achei a nota sem resposta (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= conferir("FIS-P2: a sem resposta explica e não reenvia", r.foto("-3"), r.textos(),
                                   tem=["não sabe se a SEFAZ recebeu", "Não transmita de novo"],
                                   nao_tem=["Transmitir", "Reenviar"])
            tecla("Escape")
            time.sleep(1)
            clicar_rolando(r, "Limpar filtros")
            time.sleep(1.5)
            if not clicar_rolando(r, "NFC-e 2 (série 1) · Rejeitada · R$ 150,00"):
                raise RuntimeError(f"não achei a nota rejeitada (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= conferir("FIS-P2: a rejeitada mostra a mensagem da SEFAZ na íntegra e o histórico",
                                   r.foto("-4"), r.textos(),
                                   tem=["recusou esta nota", "225 - Rejeição: Falha no Schema XML do lote de NFe",
                                        "Histórico", "Semeada pelo teste: rejeitada", "nota-2-nfe.xml"],
                                   igual=["Copiar a mensagem"])
        finally:
            r.fechar()
    finally:
        xvfb.terminate()
        shutil.rmtree(pasta, ignore_errors=True)
    print("RESULTADO:", "tudo certo" if tudo_certo else "HÁ FALHA")
    return 0 if tudo_certo else 1


if __name__ == "__main__":
    sys.exit(main())
