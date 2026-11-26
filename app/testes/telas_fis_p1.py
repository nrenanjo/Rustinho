#!/usr/bin/env python3
"""FIS-P1 e FIS-P2 (0.9.19, etapa 3, pacote E; texto A §12 e §13): o painel das notas e o detalhe.

O Rustinho ainda não emite (o emissor é a T008): o roteiro põe 5 notas de homologação direto no banco da loja de
teste (uma autorizada, uma rejeitada, uma sem resposta, uma cancelada e um rascunho) e confere:
- o ambiente à vista, o certificado numa frase só e os 4 indicadores do texto A §12 (Autorizadas, Pendentes,
  Rejeitadas e Canceladas) com as contagens (R1 C-M9, B4);
- o CPF/CNPJ escondido por padrão;
- o aviso das sem resposta filtra, a contagem e "Limpar filtros" aparecem;
- o detalhe da sem resposta: primeiro a explicação humana, e nada de reenviar nem descartar;
- o detalhe da rejeitada: o código e a mensagem da SEFAZ na íntegra, com "Copiar a mensagem", o histórico e
  "Descartar esta nota";
- FIS-P3: a troca de ambiente mostra cada impedimento; com eles, a confirmação e a senha não aceitam texto e o
  "Trocar" não troca (B5 da R1: o roteiro reprova se não achar o botão).

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
        ("n5", 4, "cancelada", "101", "Cancelamento de NF-e homologado", "135260000000002", "", ""),
    ]
    for i, numero, estado, cstat, motivo, protocolo, doc, nome in notas:
        c.execute(
            "INSERT INTO nota_fiscal (id, ambiente, modelo, serie, numero, chave, estado, destinatario_doc,"
            " destinatario_nome, valor, cstat, motivo, protocolo, xml_nome, criado_em, atualizado_em, dispositivo)"
            " VALUES (?, 2, 65, 1, ?, ?, ?, ?, ?, 15000, ?, ?, ?, ?, ?, ?, 'teste')",
            (i, numero, ("3526111122233300018165001%09d1000000019" % numero) if estado in ("autorizada", "cancelada") else "",
             estado, doc, nome,
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
            tudo_certo &= conferir("FIS-P1: ambiente à vista, certificado numa frase, os 4 indicadores", r.foto("-1"), textos,
                                   tem=["Ambiente: homologação (teste, sem valor fiscal)",
                                        "Configurado, conectado, válido e testado: os quatro chegam junto com o emissor."],
                                   igual=["Autorizadas: 1", "Pendentes: 2", "Rejeitadas: 1", "Canceladas: 1",
                                          "1 nota sem resposta da SEFAZ: ver qual é"],
                                   nao_tem=["ainda não · ", "Sem resposta: 1"])
            tudo_certo &= conferir("FIS-P1: o CPF/CNPJ fica escondido", r.foto("-1b"), textos,
                                   tem=["documento escondido", "5 notas · todo o período", "NFC-e sem número (série 1) · Rascunho",
                                        "NFC-e 4 (série 1) · Cancelada"],
                                   nao_tem=["12345678909", "NFC-e 0 (série 1)", "Nenhuma pendência fiscal"])
            if not clicar_rolando(r, "1 nota sem resposta da SEFAZ: ver qual é"):
                raise RuntimeError(f"não achei o aviso das sem resposta (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= conferir("FIS-P1: o indicador filtra, com a contagem e o limpar", r.foto("-2"), r.textos(),
                                   tem=["1 nota · todo o período", "Sem resposta da SEFAZ"],
                                   igual=["Limpar filtros"], nao_tem=["NFC-e 1 (série 1)"])
            if not clicar_rolando(r, "NFC-e 3 (série 1) · Sem resposta da SEFAZ · R$ 150,00"):
                raise RuntimeError(f"não achei a nota sem resposta (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= conferir("FIS-P2: a sem resposta explica e não reenvia", r.foto("-3"), r.textos(),
                                   tem=["não sabe se a SEFAZ recebeu", "Não transmita de novo"],
                                   nao_tem=["Transmitir", "Reenviar", "Descartar esta nota"])
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
                                   igual=["Copiar a mensagem", "Descartar esta nota"])
            # FIS-P3: a troca de ambiente mostra os impedimentos; com eles, trocar não acontece.
            tecla("Escape")
            time.sleep(1)
            if not clicar_rolando(r, "Trocar de ambiente"):
                raise RuntimeError(f"não achei o botão Trocar de ambiente (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= conferir("FIS-P3: a troca mostra cada impedimento", r.foto("-5"), r.textos(),
                                   tem=["Ir para a produção", "Ainda não dá para trocar", "certificado A3 está desligado",
                                        "série da NFC-e", "credenciamento da loja", "sem resposta conclusiva"])
            # B5 da R1: com impedimento, os campos não aceitam texto (estão desligados) e o "Trocar" não troca. Sem o
            # campo ou o botão na tela, o roteiro reprova (antes ele passava sem clicar).
            alvos = r.textos_para_clicar()
            campo = [(x, y) for x, y, t in alvos if t == "Para confirmar, digite PRODUÇÃO"]
            botoes = [(x, y) for x, y, t in alvos if t == "Trocar"]
            if not campo or not botoes:
                raise RuntimeError(f"não achei o campo da confirmação ou o botão Trocar (veja {r.saida})")
            xdo("mousemove", str(campo[-1][0]), str(campo[-1][1] + 34), "click", "1")
            time.sleep(0.5)
            xdo("type", "--delay", "40", "zqxwvk")
            time.sleep(1)
            tudo_certo &= conferir("FIS-P3: com impedimento, a confirmação não aceita texto", r.foto("-5b"), r.textos(),
                                   tem=["Ainda não dá para trocar"], nao_tem=["zqxwvk"])
            xdo("mousemove", str(botoes[-1][0]), str(botoes[-1][1]), "click", "1")
            time.sleep(1.5)
            tecla("Escape")
            time.sleep(1)
            tudo_certo &= conferir("FIS-P3: com impedimento, o ambiente continua o mesmo", r.foto("-6"), r.textos(),
                                   tem=["Ambiente: homologação (teste, sem valor fiscal)"], nao_tem=["Ambiente fiscal trocado."])
            # K12 da R2: o controle positivo do jeito de clicar e digitar usado acima. No campo ligado ("Procurar"), o
            # mesmo clique (34 px abaixo do rótulo) e a mesma digitação fazem o texto aparecer; assim, o "não apareceu"
            # da confirmação desligada prova que ela estava desligada, e não que o clique errou o alvo.
            procurar = [(x, y) for x, y, t in r.textos_para_clicar() if t == "Procurar (número, nome ou chave)"]
            if not procurar:
                raise RuntimeError(f"não achei o campo Procurar (veja {r.saida})")
            xdo("mousemove", str(procurar[-1][0]), str(procurar[-1][1] + 34), "click", "1")
            time.sleep(0.5)
            xdo("type", "--delay", "40", "zqxwvk")
            time.sleep(1.5)
            tudo_certo &= conferir("K12: no campo ligado, o mesmo clique e a mesma digitação fazem o texto aparecer",
                                   r.foto("-7"), r.textos(), tem=["zqxwvk"])
        finally:
            r.fechar()
    finally:
        xvfb.terminate()
        shutil.rmtree(pasta, ignore_errors=True)
    print("RESULTADO:", "tudo certo" if tudo_certo else "HÁ FALHA")
    return 0 if tudo_certo else 1


if __name__ == "__main__":
    sys.exit(main())
