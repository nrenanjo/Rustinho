#!/usr/bin/env python3
"""T014 (0.9.19-dev.2, protocolo 4): a entrada de caixa por convite e a segurança
dos caixas, nas telas de verdade, com dois caixas conversando pela rede.

Um dos caixas é o Rustinho de verdade (na tela virtual); o outro é feito pelo
núcleo, pelos mesmos comandos das telas (`nucleo/tests/lojas_de_teste.rs`:
`servir_loja_p4`, `entrar_por_convite_p4`, `montar_loja_p4`). Eles conversam
com este roteiro por arquivos numa pasta de troca.

Caixa novo na tela (a loja é o núcleo, numa porta sorteada):
  P1  Juntar este caixa mostra o código deste caixa (12 caracteres) e não pede a
      senha da rede;
  P2  com o convite que a loja gerou para esse código (e o endereço da loja), o
      caixa novo entra: a tela passa para a entrada com os usuários da loja, e o
      cliente da loja chegou — sem a senha da rede.
Loja na tela (o gerente aprova; o caixa novo é o núcleo):
  P3  Configurações › Caixas da loja › Segurança dos caixas: o gerente digita o
      código do caixa novo e o nome, gera o convite (texto e QR, com validade), e o
      caixa novo entra com ele;
  P4  Retirar o caixa novo: a pergunta diz que a retirada vale mesmo com a senha
      da rede; depois, ele aparece retirado pela chave e sem "Readmitir";
  P5  liberar os caixas antigos por alguns dias e recusar de novo;
  P6  DP-33: a senha nova do gerente com menos de 8 caracteres é recusada;
  P7  nenhum TypeError/ReferenceError nos registros.

O que isto NÃO prova: o QR lido por uma câmera (o celular está fora do escopo),
nem o Windows (CI), nem dois Rustinhos de verdade ao mesmo tempo (as duas
instâncias usariam a mesma porta 47810 na mesma rede; um lado é o núcleo).

    python3 app/testes/telas_dev2_protocolo4.py [pasta para as fotos]

Precisa de: Xvfb, xdotool, ImageMagick (import), cargo. Sai com 0 se aprovado.
"""

import os
import re
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

os.environ.setdefault("RUSTINHO_TELA", ":68")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_unidades as u  # noqa: E402  (traz o ambiente neutro e o Rustinho conferido)
import _aux_nucleo  # noqa: E402

base, tema = u.base, u.tema
TELA = u.TELA

CODIGO = re.compile(r"^[A-Z2-9]{4}-[A-Z2-9]{4}-[A-Z2-9]{4}$")
CONVITE = re.compile(r"^[A-HJ-NP-Z2-9]{5}(-[A-HJ-NP-Z2-9]{1,5}){5}$")
NOME_NOVO = "Caixa Novo"


def textos_limpos(textos):
    return [t for _, _, t in textos if not t.startswith(("GEO:", "RET:", "TOQUE:", "TESTE_"))]


def tem_texto(textos, trecho):
    return any(base.simples(trecho) in base.simples(t) for t in textos_limpos(textos))


def esperar(rr, condicao, segundos=30):
    fim = time.time() + segundos
    while True:
        t = rr.textos()
        if condicao(t) or time.time() > fim:
            return t
        time.sleep(0.5)


def esperar_arquivo(pasta, nome, segundos):
    fim = time.time() + segundos
    while time.time() < fim:
        caminho = os.path.join(pasta, nome)
        if os.path.exists(caminho):
            with open(caminho, encoding="utf-8") as f:
                return f.read().strip()
        time.sleep(0.3)
    return None


def escrever(pasta, nome, texto):
    tmp = os.path.join(pasta, "." + nome + ".novo")
    with open(tmp, "w", encoding="utf-8") as f:
        f.write(texto)
    os.replace(tmp, os.path.join(pasta, nome))


def clicar_ultimo(rr, texto):
    """Clica no último texto igual (o de cima de tudo: o botão da janelinha aberta)."""
    achados = [(x, y) for x, y, t in rr.textos() if base.simples(t) == base.simples(texto)]
    if not achados:
        return False
    px, py = rr.ponto(*achados[-1])
    base.xdo("mousemove", str(px), str(py), "click", "1")
    return True


def clicar_abaixo(rr, rotulo, dy=30):
    """Clica no campo logo abaixo do rótulo (o campo de senha não tem dica)."""
    for x, y, t in rr.textos():
        if base.simples(t) == base.simples(rotulo):
            px, py = rr.ponto(x, y + dy)
            base.xdo("mousemove", str(px), str(py), "click", "1")
            return True
    return False


def digitar(texto):
    base.xdo("type", "--delay", "40", texto)


def banco(pasta, sql, *p):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    try:
        return c.execute(sql, p).fetchone()[0]
    finally:
        c.close()


def parar(processo):
    if processo is None:
        return
    try:
        processo.wait(timeout=30)
    except subprocess.TimeoutExpired:
        processo.kill()
        processo.wait()


def caixa_novo_na_tela(fotos, logs, conferir):
    """P1 e P2: o caixa novo é o Rustinho de verdade; a loja é o núcleo."""
    troca = tempfile.mkdtemp(prefix="rustinho-p4-troca1-")
    loja = tempfile.mkdtemp(prefix="rustinho-p4-loja1-")
    novo = tempfile.mkdtemp(prefix="rustinho-p4-novo1-")
    saida = open(os.path.join(fotos, "servir_loja_p4.log"), "w")
    servidor = _aux_nucleo.iniciar("lojas_de_teste", "servir_loja_p4",
                                   {"RUSTINHO_LOJA_TESTE": loja, "RUSTINHO_P4_TROCA": troca},
                                   raiz=base.RAIZ, saida=saida)
    rr = None
    try:
        porta = esperar_arquivo(troca, "porta.txt", 180)
        if not porta:
            conferir("P1", "Juntar mostra o código e não pede a senha da rede", False,
                     f"não medido: a loja do núcleo não começou (veja {saida.name})")
            return
        rr = u.Rustinho(novo, "TelaInicio", {}, fotos, "p1_caixa_novo")
        logs.append(rr.saida)
        t = esperar(rr, lambda x: any(base.simples(tx).startswith("ja uso o rustinho") for tx in textos_limpos(x)))
        if not rr.clicar("Já uso o Rustinho", comeca=True):
            conferir("P1", "Juntar mostra o código e não pede a senha da rede", False,
                     f"não medido: sem as boas-vindas (veja {rr.saida})")
            return
        t = esperar(rr, lambda x: any(CODIGO.match(tx) for tx in textos_limpos(x)), 40)
        rr.foto("-1-codigo")
        codigos = [tx for tx in textos_limpos(t) if CODIGO.match(tx)]
        sem_senha = not tem_texto(t, "Senha da rede")
        conferir("P1", "Juntar mostra o código e não pede a senha da rede",
                 len(codigos) == 1 and sem_senha and tem_texto(t, "Mostre este código ao gerente"),
                 f"códigos {codigos}; senha da rede na tela: {not sem_senha}")
        if len(codigos) != 1:
            return
        escrever(troca, "codigo.txt", codigos[0])
        convite = esperar_arquivo(troca, "convite.txt", 60)
        if not convite:
            conferir("P2", "o caixa novo entra pelo convite, sem a senha da rede", False,
                     f"não medido: a loja não gerou o convite ({esperar_arquivo(troca, 'erro.txt', 1)})")
            return
        time.sleep(0.5)
        digitar(convite)
        if not rr.clicar("Aparece junto do convite. Ex.: 192.168.0.12"):
            conferir("P2", "o caixa novo entra pelo convite, sem a senha da rede", False, "sem o campo do endereço")
            return
        time.sleep(0.3)
        digitar(f"127.0.0.1:{porta}")
        rr.foto("-2-convite")
        rr.clicar("Entrar na loja")
        t = esperar(rr, lambda x: tem_texto(x, "Quem está usando"), 90)
        rr.foto("-3-entrou")
        entrou = tem_texto(t, "Quem está usando")
        cliente = banco(novo, "SELECT COUNT(*) FROM pessoa WHERE nome = 'Cliente Sintetico do Convite'")
        raiz = banco(novo, "SELECT COUNT(*) FROM config WHERE chave = 'p4_raiz' AND valor <> ''")
        senha = banco(novo, "SELECT COUNT(*) FROM config WHERE chave = 'sync_senha' AND valor <> ''")
        conferir("P2", "o caixa novo entra pelo convite, sem a senha da rede",
                 entrou and cliente == 1 and raiz == 1 and senha == 0,
                 f"tela de entrada {entrou}; cliente da loja {cliente}; raiz fixada {raiz}; senha da rede gravada {senha}")
    finally:
        escrever(troca, "fim.txt", "fim")
        if rr is not None:
            rr.fechar()
        parar(servidor)
        saida.close()
        for p in (troca, loja, novo):
            shutil.rmtree(p, ignore_errors=True)


def loja_na_tela(fotos, logs, conferir):
    """P3 a P6: a loja é o Rustinho de verdade (o gerente aprova); o caixa novo é o núcleo."""
    troca = tempfile.mkdtemp(prefix="rustinho-p4-troca2-")
    loja = tempfile.mkdtemp(prefix="rustinho-p4-loja2-")
    novo = tempfile.mkdtemp(prefix="rustinho-p4-novo2-")
    _aux_nucleo.executar("lojas_de_teste", "montar_loja_p4", {"RUSTINHO_LOJA_TESTE": loja}, raiz=base.RAIZ)
    saida = open(os.path.join(fotos, "entrar_por_convite_p4.log"), "w")
    caixa = None
    rr = None
    try:
        # O motor do Rustinho torna a loja a raiz ao começar; a tela lê a situação ao abrir.
        rr = u.Rustinho(loja, "TelaInicio", {}, fotos, "p3_loja_raiz")
        logs.append(rr.saida)
        fim = time.time() + 90
        while banco(loja, "SELECT COUNT(*) FROM config WHERE chave = 'p4_raiz' AND valor <> ''") == 0 and time.time() < fim:
            time.sleep(0.5)
        rr.fechar()
        rr = u.Rustinho(loja, "TelaConfiguracoes", {"secao": "caixas"}, fotos, "p3_loja")
        logs.append(rr.saida)
        rr.entrar("Ana")
        caixa = _aux_nucleo.iniciar("lojas_de_teste", "entrar_por_convite_p4",
                                    {"RUSTINHO_LOJA_TESTE": novo, "RUSTINHO_P4_TROCA": troca,
                                     "RUSTINHO_P4_ENDERECO": "127.0.0.1:47810"},
                                    raiz=base.RAIZ, saida=saida)
        codigo = esperar_arquivo(troca, "codigo.txt", 120)
        t = esperar(rr, lambda x: tem_texto(x, "Este caixa é aprovado na loja"), 30)
        if not codigo or not tem_texto(t, "Este caixa é aprovado na loja"):
            rr.foto("-0-sem-aprovacao")
            conferir("P3", "o gerente gera o convite na tela e o caixa novo entra com ele", False,
                     f"não medido: código {codigo}; a loja aprovada na tela: {tem_texto(t, 'Este caixa é aprovado na loja')}")
            return
        rr.clicar("Aparece na tela dele. Ex.: 4F2K-9QXA-7MPC")
        time.sleep(0.3)
        digitar(codigo)
        rr.clicar("Ex.: Caixa 2")
        time.sleep(0.3)
        digitar(NOME_NOVO)
        rr.clicar("Gerar convite")
        t = esperar(rr, lambda x: any(CONVITE.match(tx) for tx in textos_limpos(x)), 30)
        rr.foto("-1-convite")
        convites = [tx for tx in textos_limpos(t) if CONVITE.match(tx)]
        qr = [tx for _, _, tx in t if tx.startswith("GEO:qrDoConvite:")]
        qr_ok = bool(qr) and all(int(g.split(",")[-1]) > 100 for g in qr)
        validade = tem_texto(t, "Vale até")
        if len(convites) != 1:
            conferir("P3", "o gerente gera o convite na tela e o caixa novo entra com ele", False,
                     f"convites na tela {convites}; QR {qr}")
            return
        escrever(troca, "convite.txt", convites[0])
        entrou = esperar_arquivo(troca, "entrou.txt", 90)
        puxou = esperar_arquivo(troca, "puxou.txt", 90)
        aprovado = banco(loja, "SELECT COUNT(*) FROM auditoria WHERE acao = 'aprovar_caixa'")
        conferir("P3", "o gerente gera o convite na tela e o caixa novo entra com ele",
                 entrou == "ok" and puxou == "ok" and qr_ok and validade and aprovado == 1,
                 f"entrou {entrou}; puxou {puxou}; QR {qr}; validade {validade}; aprovações auditadas {aprovado}")

        # P4: retirar o caixa novo (a lista de caixas é lida ao abrir a seção).
        rr.fechar()
        rr = u.Rustinho(loja, "TelaConfiguracoes", {"secao": "caixas"}, fotos, "p4_retirar")
        logs.append(rr.saida)
        rr.entrar("Ana")
        t = esperar(rr, lambda x: tem_texto(x, NOME_NOVO) and tem_texto(x, "Retirar da loja"), 30)
        if not rr.clicar("Retirar da loja"):
            conferir("P4", "a retirada pela chave diz o que vale e não deixa readmitir", False,
                     "não medido: sem o botão Retirar da loja")
            return
        t = esperar(rr, lambda x: tem_texto(x, "mesmo com a senha da rede"), 10)
        pergunta_certa = tem_texto(t, "mesmo com a senha da rede") and tem_texto(t, "aprovação nova do gerente")
        rr.foto("-1-pergunta")
        digitar("saiu da loja (teste do protocolo 4)")
        clicar_ultimo(rr, "Retirar da loja")
        t = esperar(rr, lambda x: tem_texto(x, "Retirado pela chave"), 30)
        rr.foto("-2-retirado")
        retirado = tem_texto(t, "Retirado pela chave")
        sem_readmitir = not any(base.simples(tx) == "readmitir" for tx in textos_limpos(t))
        assinada = banco(loja, "SELECT COUNT(*) FROM caixa_retirado")
        conferir("P4", "a retirada pela chave diz o que vale e não deixa readmitir",
                 pergunta_certa and retirado and sem_readmitir and assinada == 1,
                 f"pergunta {pergunta_certa}; retirado pela chave {retirado}; sem Readmitir {sem_readmitir}; "
                 f"retiradas assinadas {assinada}")

        # P5: os caixas antigos.
        rr.clicar("Liberar os caixas antigos")
        t = esperar(rr, lambda x: tem_texto(x, "liberados neste caixa até"), 20)
        liberou = tem_texto(t, "liberados neste caixa até")
        rr.foto("-3-liberados")
        rr.clicar("Recusar os caixas antigos agora")
        t = esperar(rr, lambda x: tem_texto(x, "não trocam dados com este caixa"), 20)
        recusou = tem_texto(t, "não trocam dados com este caixa")
        conferir("P5", "liberar os caixas antigos por alguns dias e recusar de novo", liberou and recusou,
                 f"liberou {liberou}; recusou {recusou}")
        rr.fechar()

        # P6: DP-33 na tela da senha.
        rr = u.Rustinho(loja, "TelaConfiguracoes", {"secao": "loja"}, fotos, "p6_dp33")
        logs.append(rr.saida)
        rr.entrar("Ana")
        t = esperar(rr, lambda x: tem_texto(x, "Pelo menos 8 caracteres"), 20)
        dica = tem_texto(t, "Pelo menos 8 caracteres")
        clicar_abaixo(rr, "Senha atual")
        time.sleep(0.3)
        digitar(base.SENHA["Ana"])
        rr.clicar("Pelo menos 8 caracteres")
        time.sleep(0.3)
        digitar("1234")
        clicar_abaixo(rr, "Repita a nova senha")
        time.sleep(0.3)
        digitar("1234")
        rr.clicar("Trocar minha senha")
        t = esperar(rr, lambda x: any("8" in tx and "caracteres" in base.simples(tx) and tx != "Pelo menos 8 caracteres"
                                       for tx in textos_limpos(x)), 15)
        rr.foto("-1-recusada")
        recusada = any("8" in tx and "caracteres" in base.simples(tx) and tx != "Pelo menos 8 caracteres"
                       for tx in textos_limpos(t))
        conferir("P6", "DP-33: a senha nova do gerente com menos de 8 caracteres é recusada", dica and recusada,
                 f"dica {dica}; recusada {recusada}")
    finally:
        escrever(troca, "fim.txt", "fim")
        if rr is not None:
            rr.fechar()
        parar(caixa)
        saida.close()
        for p in (troca, loja, novo):
            shutil.rmtree(p, ignore_errors=True)


def main():
    for p in ("Xvfb", "xdotool", "import", "cargo"):
        if not shutil.which(p):
            sys.exit(f"Falta o programa {p}")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-p4-")
    os.makedirs(fotos, exist_ok=True)
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1920x1080x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    resultados, logs = {}, []

    def conferir(codigo, nome, certo, detalhe=""):
        resultados[codigo] = (bool(certo), nome, detalhe)

    try:
        if xvfb.poll() is not None:
            sys.exit(f"Xvfb não abriu na tela {TELA}")
        for parte in (caixa_novo_na_tela, loja_na_tela):
            try:
                parte(fotos, logs, conferir)
            except Exception as e:  # noqa: BLE001  (o roteiro segue para a outra parte e conta a falha)
                conferir(f"P0-{parte.__name__}", f"{parte.__name__} chegou ao fim", False, repr(e))
        erros = [e for caminho in logs for e in tema.erros_de_script(caminho)]
        conferir("P7", f"zero TypeError/ReferenceError em {len(logs)} registros", not erros, "; ".join(erros[:5]))
    finally:
        xvfb.terminate()
        xvfb.wait()

    reprovado = False
    for codigo in sorted(resultados):
        certo, nome, detalhe = resultados[codigo]
        print(f"  {'✔' if certo else '✘'} {codigo} {nome}" + ("" if certo else f" — {detalhe}"))
        if certo:
            print(f"      {detalhe}")
        reprovado |= not certo
    certas = sum(1 for c in resultados.values() if c[0])
    print(f"\nRESULTADO: {certas} de {len(resultados)} conferências certas; fotos em {fotos}")
    sys.exit(1 if reprovado or len(resultados) != 7 else 0)


if __name__ == "__main__":
    main()
