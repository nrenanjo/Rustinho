#!/usr/bin/env python3
"""Roteiro de tela do lote 2 da fase 4: "Trazer cadastros".

Abre o Rustinho de verdade numa tela virtual (Xvfb), com uma pasta pessoal
temporária (a nota de compra de exemplo e a nota de venda da loja em Downloads),
e confere pelos textos da tela e pelo banco:

  PEND-38/H02 e DP-52 (b) (item 02)
    1. a caixa vê o pedido da senha do gerente ANTES de escolher arquivos;
    2. desistindo, a tela fecha sem ler nada;
    3. com a senha, escolhe e a prévia lê (a chamada protegida passa) sem uma
       segunda senha;
    4. o gerente não vê o pedido;
    5. a aba de pendências abre sem senha.
  A07 (item 06)
    6. "Associar" a uma peça desativada mostra a mensagem e "Abrir a peça", que
       abre a ficha dela; a pendência continua aberta.

O que a tela NÃO confere aqui, e onde está provado: as notas de teste são
assinadas pela PKI de teste, e o programa de tela recusa abrir se for montado
com as âncoras de teste (PEND-09, `app/src/main.rs`). Então a prévia não aceita
estas notas pela tela (aparecem como ignoradas). O conteúdo da prévia nova
(peças que mudam, pendências por motivo) e o cadastro com a senha da tela estão
provados no núcleo: `importar::testes::o3_previa_igual_ao_resultado` e
`caracterizacao_dev33::pend38_senha_da_tela_vale_so_para_trazer_cadastros`. A
prévia com notas reais na tela é da campanha L5/L6.

    python3 app/testes/telas_importar_cadastros.py [pasta para as fotos]
    RUSTINHO_TELA=:94 python3 app/testes/telas_importar_cadastros.py
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_d09 as base  # noqa: E402
import telas_navegacao_guiada as nav  # noqa: E402
from telas_d09 import PROGRAMA, RAIZ  # noqa: E402

base.TELA = os.environ.get("RUSTINHO_TELA", ":84")
nav.TELA = base.TELA
TELA = base.TELA
NOTA_LOJA = os.path.join(RAIZ, "nucleo", "tests", "pki", "xml", "venda_da_loja_sem_protocolo.xml")
PEDIDO_DA_TELA = "Trazer cadastros: só o gerente pode"


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), check=True)


def por_fora(pasta, sql):
    """SQL na loja de teste pelo núcleo (os gatilhos da troca usam as funções do Rustinho)."""
    env = dict(os.environ, RUSTINHO_LOJA_TESTE=pasta, RUSTINHO_SQL=sql)
    subprocess.run(["cargo", "test", "-q", "-p", "rustinho-nucleo", "--test", "estavel", "--",
                    "--ignored", "--exact", "executar_por_fora"],
                   cwd=RAIZ, env=env, stdout=subprocess.DEVNULL, check=True)


def casa_com_as_notas(raiz):
    casa = tempfile.mkdtemp(prefix="casa-", dir=raiz)
    os.makedirs(os.path.join(casa, "Downloads"))
    shutil.copy(nav.EXEMPLO, os.path.join(casa, "Downloads", "compra.xml"))
    shutil.copy(NOTA_LOJA, os.path.join(casa, "Downloads", "venda.xml"))
    return casa


def um(pasta, sql, *p):
    return nav.banco(pasta, sql, *p)[0][0]


def main():
    for p in ("Xvfb", "xdotool", "import", "convert"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-importar-")
    os.makedirs(fotos, exist_ok=True)
    raiz = tempfile.mkdtemp(prefix="rustinho-importar-")
    pasta = os.path.join(raiz, "loja")
    modelo = os.path.join(raiz, "loja-modelo")
    os.makedirs(modelo)
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    certo = True

    def ok(nome, condicao, foto=""):
        nonlocal certo
        print(("✔ " if condicao else "✘ ") + nome + ("" if condicao else f" (foto: {foto})"))
        certo &= bool(condicao)

    # Cenário M: o rolamento (ligado ao FAG-32206) sem código de barras; o OL5W30
    # sem vínculo (vira peça nova, para "Peças do fornecedor" vir marcada).
    cenario_m = ("UPDATE produto SET codigo_barras = '' WHERE nome = 'ROLAMENTO RODA TRASEIRA 32206';"
                 "DELETE FROM produto_gtin WHERE gtin = '7891234567895';"
                 "DELETE FROM produto_fornecedor WHERE codigo = 'OL5W30';")
    # Cenário P: o FAG-32206 ligado a outra peça (conflito com o GTIN do rolamento).
    cenario_p = ("UPDATE produto_fornecedor SET produto_id = (SELECT id FROM produto WHERE nome = 'Pastilha de freio')"
                 " WHERE codigo = 'FAG-32206';"
                 "DELETE FROM produto_fornecedor WHERE codigo = 'OL5W30';")
    try:
        base.montar_loja(modelo)
        por_fora(modelo, "UPDATE config SET valor = '11444777000161' WHERE chave = 'loja_cnpj';")

        def loja_nova(sql):
            shutil.rmtree(pasta, ignore_errors=True)
            shutil.copytree(modelo, pasta)
            por_fora(pasta, sql)

        casa = casa_com_as_notas(raiz)

        # -------- 1 e 2: a caixa vê o pedido antes; desistindo, nada é lido --------
        loja_nova(cenario_m)
        r = nav.Rustinho(pasta, "TelaPecas", {}, fotos, "1-caixa-desiste", casa)
        try:
            r.entrar("Bia")
            ok("1: Importar XML", r.clicar("Importar XML"), r.foto("-0"))
            ok("1: a caixa vê o pedido da senha antes dos arquivos", r.esperar(PEDIDO_DA_TELA), r.foto("-1"))
            ok("1: sem a escolha de arquivos ainda", not r.tem("Escolher arquivos"), r.foto("-1b"))
            xdo("key", "Escape")
            time.sleep(1.5)
            f = r.foto("-2")
            ok("2: desistindo, a tela fecha", not r.tem("Trazer cadastros pelo XML") and not r.tem(PEDIDO_DA_TELA), f)
            ok("2: nada foi lido nem gravado", um(pasta, "SELECT COUNT(*) FROM auditoria WHERE acao = 'gerente.liberar'") == 0, f)
        finally:
            r.fechar()

        # -------- 3: com a senha, a prévia lê sem uma segunda senha --------
        loja_nova(cenario_m)
        r = nav.Rustinho(pasta, "TelaPecas", {}, fotos, "3-caixa-le", casa)
        try:
            r.entrar("Bia")
            r.senha_da_tela = base.SENHA["Ana"]
            r.escolher_pasta()
            ok("3: pediu a senha ao abrir", getattr(r, "pediu_senha_da_tela", False), r.foto("-1"))
            # A prévia (consulta do gerente, T-2) passou com a senha da tela: mostra o
            # que leu (as notas de teste aparecem como ignoradas; veja a nota acima).
            ok("3: a prévia leu com a senha da tela", r.esperar("arquivos ignorados"), r.foto("-2"))
            ok("3: sem uma segunda senha", not r.tem("Senha do gerente"), r.foto("-2b"))
            ok("3: uma liberação só, a da tela",
               um(pasta, "SELECT COUNT(*) FROM auditoria WHERE acao = 'gerente.liberar' AND detalhes LIKE '%nesta tela%'") == 1)
        finally:
            r.fechar()

        # -------- 4: o gerente não vê o pedido --------
        loja_nova(cenario_m)
        r = nav.Rustinho(pasta, "TelaPecas", {}, fotos, "4-gerente", casa)
        try:
            r.entrar("Ana")
            ok("4: Importar XML", r.clicar("Importar XML"), r.foto("-0"))
            time.sleep(1.5)
            f = r.foto("-1")
            ok("4: o gerente vai direto para a escolha dos arquivos", r.tem("Escolher arquivos") and not r.tem(PEDIDO_DA_TELA), f)
        finally:
            r.fechar()

        # -------- 5 e 6: a pendência (posta direto no banco) e a peça desativada --------
        loja_nova(cenario_p)
        rolamento = um(pasta, "SELECT id FROM produto WHERE nome = 'ROLAMENTO RODA TRASEIRA 32206'")
        pastilha = um(pasta, "SELECT id FROM produto WHERE nome = 'Pastilha de freio'")
        fornecedor = um(pasta, "SELECT id FROM pessoa WHERE documento = '11222333000181'")
        por_fora(pasta, "INSERT INTO xml_pendencia_identificacao (id, fornecedor_id, fornecedor_documento, fornecedor_nome,"
                        " chave_nfe, numero_nfe, emissao_nfe, codigo_fornecedor, codigo_compacto, gtin_comercial,"
                        " origem_comercial, gtin_tributavel, origem_tributavel, gtins_json, descricao, descricao_compacta,"
                        " ncm, cest, cfop, unidade, origem_fiscal, cst_icms, cst_pis, quantidade, valor_unitario, motivo,"
                        " candidatos_json, situacao, criado_em) VALUES ('l2-pend', "
                        f"'{fornecedor}', '11222333000181', 'DISTRIBUIDORA DE AUTOPECAS EXEMPLO LTDA', '', '1', '2026-11-01',"
                        " 'FAG-32206', 'fag32206', '7891234567895', 'cEAN', '', '', '[]', 'ROLAMENTO RODA TRASEIRA 32206',"
                        " 'rolamentorodatraseira32206', '84822010', '', '5102', 'PC', 0, '00', '01', 1000, 5000,"
                        f" 'conflito_gtin_fornecedor', '[\"{rolamento}\",\"{pastilha}\"]', 'pendente', '2026-11-12T00:00:00Z');"
                        " UPDATE produto SET ativo = 0 WHERE nome = 'Pastilha de freio';")
        r = nav.Rustinho(pasta, "TelaPecas", {"abrirPendenciasXml": True}, fotos, "5-6-pendencias", casa)
        try:
            r.entrar("Bia")
            ok("5: a aba de pendências abre sem senha",
               r.esperar("Pendências de identificação XML") and not r.tem(PEDIDO_DA_TELA), r.foto("-1"))
            # 6: "Associar" à candidata desativada (a Pastilha de freio).
            alvo = r.posicao("Pastilha de freio")
            botoes = [(x, y) for x, y, t in r.textos() if base.simples(t) == base.simples("Associar a esta peça")]
            ok("6: as candidatas aparecem", alvo is not None and len(botoes) >= 2, r.foto("-2"))
            if alvo and botoes:
                x, y = min(botoes, key=lambda b: abs(b[1] - alvo[1]))
                xdo("mousemove", str(x), str(y), "click", "1")
                time.sleep(1.5)
                f = r.foto("-3")
                ok("6: a mensagem da peça desativada", r.tem("está desativada"), f)
                ok("6: e o botão Abrir a peça", r.clicar("Abrir a peça"), f)
                time.sleep(2)
                f = r.foto("-4")
                ok("6: abre a ficha da peça, à vista", r.esperar("Peça ativa") and not r.tem("Pendências de identificação XML"), f)
                ok("6: a pendência continua aberta",
                   um(pasta, "SELECT COUNT(*) FROM xml_pendencia_identificacao WHERE situacao = 'pendente'") == 1)
        finally:
            r.fechar()
    finally:
        xvfb.terminate()
        shutil.rmtree(raiz, ignore_errors=True)
    print("fotos em", fotos)
    sys.exit(0 if certo else 1)


if __name__ == "__main__":
    main()
