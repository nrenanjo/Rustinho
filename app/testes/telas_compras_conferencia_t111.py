#!/usr/bin/env python3
"""Roteiro E2E da T111 (divergência Início ↔ Compras ↔ Ficha na conferência de autorização):

Confere na interface real (Xvfb + xdotool) o fluxo completo do gerente:
  1. Tela Inicial mostra "2 notas de compra aguardando a conferência da autorização" com "Ver compras".
  2. Clicar em "Ver compras" abre Compras já filtrada em "Aguardando conferência · 2", com faixa
     contextual e botão "Conferir" nas linhas; a nota com 0 itens pendentes mostra
     "Aguardando conferência" (NUNCA "Pronta para lançar" antes da conferência!).
  3. Clicar em "Conferir" abre TelaCompra (sem abrir duplicado na pilha) já com o painel
     "Conferência no portal da NF-e" aberto e o botão "Copiar chave".
  4. Clicar em "Copiar chave" avisa "Chave de acesso copiada." (e copia os 44 dígitos sem espaços).
  5. Registrar a conferência com protocolo válido + senha do gerente grava `confirmada_gerente`.
  6. Voltando a Compras, o filtro cai para 1 nota; clicar "Mostrar todas" exibe a nota conferida
     como "Pronta para lançar" e a pendente como "Aguardando conferência".
  7. Voltando ao Início, o aviso cai para "1 nota de compra aguardando a conferência da autorização";
     clicar em "Ver compras" com 1 única nota leva diretamente à TelaCompra dela já com o painel
     "Conferência no portal da NF-e" aberto.

    python3 app/testes/telas_compras_conferencia_t111.py [pasta para as fotos]
"""

import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_navegacao_casos as nav  # noqa: E402
from telas_navegacao_casos import PROGRAMA, SENHA, Placar, montar_loja, por_fora, xdo  # noqa: E402

nav.TELA = os.environ.get("RUSTINHO_TELA", ":97")
TELA = nav.TELA
CHAVE_EXEMPLO = "35261011222333000181550010000123451000000014"
PROTOCOLO_EXEMPLO = "135260000000001"
EXEMPLO = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "nucleo", "tests", "nfe_exemplo.xml")


def ler_sql(pasta, sql, *params):
    conn = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    try:
        return conn.execute(sql, params).fetchall()
    finally:
        conn.close()


def main():
    placar = Placar()
    ok = placar.ok

    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")

    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-t111-conf-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-t111-conf-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    try:
        if xvfb.poll() is not None:
            sys.exit(f"Xvfb não abriu na tela {TELA}")
        montar_loja(pasta)
        xml_exemplo = open(EXEMPLO, encoding="utf-8").read().replace("'", "''")
        forn_id = ler_sql(pasta, "SELECT fornecedor_id FROM compra ORDER BY rowid LIMIT 1")[0][0]
        agora_iso = "2026-11-22T18:00:00Z"
        disp = ler_sql(pasta, "SELECT dispositivo FROM compra ORDER BY rowid LIMIT 1")[0][0]
        chave2 = "35261011222333000181550010000900021000000028"
        # Garante que só nossas 2 notas estejam aguardando conferência (a compra da montagem base fica 'lancada').
        # Usa por_fora (desliga os gatilhos do sync durante a montagem da loja de teste).
        por_fora(
            pasta,
            f"""
            UPDATE compra SET situacao = 'lancada', autorizacao = 'anterior';
            INSERT INTO compra (id, fornecedor_id, chave, numero, serie, emissao, natureza, total_produtos,
                                frete, seguro, desconto, outras, ipi, icms_st, total, destinatario, xml,
                                criado_em, dispositivo, xml_verificacao, xml_prova, autorizacao, situacao)
            VALUES ('cp-t111-1', '{forn_id}', '{CHAVE_EXEMPLO}', '90001', '1', '2026-11-22', 'Venda', 10000,
                    0, 0, 0, 0, 0, 0, 10000, '11444777000161', '{xml_exemplo}', '{agora_iso}', '{disp}',
                    'verificada', '{{"tipo":"A1"}}', 'aguardando', 'rascunho');
            INSERT INTO compra_item (id, compra_id, ordem, codigo_fornecedor, descricao, unidade, quantidade,
                                     valor_produtos, acao)
            VALUES ('ci-t111-1', 'cp-t111-1', 1, 'F-01', 'Filtro de ar', 'UN', 1000, 10000, 'nova');
            INSERT INTO compra (id, fornecedor_id, chave, numero, serie, emissao, natureza, total_produtos,
                                frete, seguro, desconto, outras, ipi, icms_st, total, destinatario, xml,
                                criado_em, dispositivo, xml_verificacao, xml_prova, autorizacao, situacao)
            VALUES ('cp-t111-2', '{forn_id}', '{chave2}', '90002', '1', '2026-11-21', 'Venda', 25000,
                    0, 0, 0, 0, 0, 0, 25000, '11444777000161', '{xml_exemplo}', '{agora_iso}', '{disp}',
                    'verificada', '{{"tipo":"A1"}}', 'aguardando', 'rascunho');
            INSERT INTO compra_item (id, compra_id, ordem, codigo_fornecedor, descricao, unidade, quantidade,
                                     valor_produtos, acao)
            VALUES ('ci-t111-2', 'cp-t111-2', 1, 'P-02', 'Pastilha dianteira', 'JG', 1000, 25000, '');
            """,
        )

        r = placar.abrir(pasta, "TelaInicio", {}, fotos, "t111_conf")
        try:
            r.entrar("Ana")
            # 1. Tela Inicial mostra o aviso (se vier minimizado em quadrado, clica nele para expandir).
            if r.esperar(lambda: r.tem_igual("Notas de compra")) and not r.tem_igual("Ver compras"):
                r.clicar("Notas de compra")
            if not ok(
                "1. Início mostra aviso de 2 notas aguardando conferência e botão Ver compras",
                r.esperar(lambda: r.tem("2 notas de compra aguardando a conferência da autorização") and r.tem_igual("Ver compras")),
                f"foto {r.foto('-1-inicio-falha')}",
            ):
                return
            r.foto("-1-inicio")

            # 2. Clicar em "Ver compras" abre TelaCompras já filtrada em Aguardando conferência.
            r.clicar("Ver compras")
            if not ok(
                "2a. Ver compras abre Compras filtrada em Aguardando conferência · 2 com faixa contextual",
                r.esperar(lambda: r.tem("2 notas aguardando conferência da autorização") and r.tem("Aguardando conferência · 2")),
                f"foto {r.foto('-2-compras-falha')}",
            ):
                return
            time.sleep(0.5)
            f2 = r.foto("-2-compras-filtrada")
            ok(
                "2b. Nenhuma nota pendente mostra Pronta para lançar; ambas mostram Aguardando conferência",
                r.tem_igual("Aguardando conferência") and not r.tem_igual("Pronta para lançar"),
                f"foto {f2}",
            )
            ok("2c. Botão Conferir aparece diretamente na linha", r.tem_igual("Conferir"), f"foto {f2}")

            # 2d. Layout estreito (celular 360x720): chip e botão Conferir visíveis dentro de 360 px.
            janela_id = r._esperar_janela()
            xdo("windowsize", janela_id, "360", "720")
            time.sleep(0.6)
            ok(
                "2d. Em janela estreita (360x720), Aguardando conferência e Conferir cabem dentro da tela",
                r.esperar(lambda: any(0 < x < 360 for x, _, t in r.textos() if t == "Aguardando conferência")
                          and any(0 < x < 360 for x, _, t in r.textos() if t == "Conferir")),
                f"foto {r.foto('-2d-estreito')}",
            )
            xdo("windowsize", janela_id, "1280", "760")
            time.sleep(0.6)
            r.esperar(lambda: r.tem_igual("Conferir"))
            pilha_compras = r.pilha()

            # 3. Clicar em "Conferir" abre TelaCompra (sem duplicar na pilha) com o painel de conferência aberto.
            r.clicar("Conferir")
            if not ok(
                "3a. Conferir abre TelaCompra já com o painel Conferência no portal da NF-e aberto",
                r.esperar(lambda: r.tem("Conferência no portal da NF-e") and r.tem_igual("Copiar chave")),
                f"foto {r.foto('-3-painel-falha')}",
            ):
                return
            time.sleep(0.5)
            f3 = r.foto("-3-painel-conferencia")
            pilha_compra = r.pilha()
            ok(
                "3b. Botão Conferir abre exatamente 1 TelaCompra na pilha (sem clique duplo na linha)",
                pilha_compra == pilha_compras + 1,
                f"antes={pilha_compras}, depois={pilha_compra}",
            )

            # 4. Clicar em "Copiar chave" copia os 44 dígitos limpos e devolve foco ao protocolo.
            r.clicar("Copiar chave")
            ok(
                "4a. Copiar chave exibe aviso de confirmação",
                r.esperar(lambda: r.tem("Chave de acesso copiada.")),
                f"foto {r.foto('-4a-copiar')}",
            )

            # 5. Registrar conferência (Copiar chave devolve foco ao campo de protocolo; data já vem preenchida).
            time.sleep(0.3)
            r.digitar(PROTOCOLO_EXEMPLO)
            xdo("key", "Return")  # vai para data/hora (já preenchida)
            time.sleep(0.3)
            xdo("key", "Return")  # vai para senha do gerente
            time.sleep(0.3)
            r.digitar(SENHA["Ana"])
            xdo("key", "Return")  # submete
            if not ok(
                "5a. Conferência registrada com sucesso e painel fechado",
                r.esperar(lambda: r.tem("Conferência registrada. A compra já pode ser lançada.")),
                f"foto {r.foto('-5-conferencia-falha')}",
            ):
                return
            time.sleep(0.5)
            f5 = r.foto("-5-apos-conferencia")
            aut = ler_sql(pasta, "SELECT autorizacao FROM compra WHERE id = 'cp-t111-1'")[0][0]
            ok("5b. Banco registrou autorizacao = confirmada_gerente", aut == "confirmada_gerente", f"aut={aut}")
            ok(
                "5c. Cartão Para lançar agora mostra Tudo pronto (não pede mais conferência)",
                r.tem("Tudo pronto") and not r.tem_igual("Conferir autorização…"),
                f"foto {f5}",
            )

            # 6. Voltar a Compras: filtro atualiza para 1 nota; "Mostrar todas" exibe Pronta para lançar na conferida.
            xdo("key", "Escape")
            ok(
                "6a. Ao voltar a Compras, pendência cai para 1 nota aguardando conferência",
                r.esperar(lambda: r.tem("1 nota aguardando conferência da autorização")),
                f"foto {r.foto('-6a-compras-1')}",
            )
            r.clicar("Mostrar todas")
            ok(
                "6b. Em Mostrar todas, nota conferida aparece como Pronta para lançar e a outra segue Aguardando conferência",
                r.esperar(lambda: r.tem_igual("Pronta para lançar") and r.tem_igual("Aguardando conferência")),
                f"foto {r.foto('-6b-compras-todas')}",
            )
            # 6c. Prova real do clipboard X11/Qt (sem depender de xclip/xsel): cola na busca de Compras
            # e confere que vieram exatamente os 44 dígitos puros da chave, filtrando só a NF 90001.
            r.clicar("Colar na busca")
            ok(
                "6c. Colar na busca comprova que Copiar chave gravou os 44 dígitos puros no clipboard e encontra NF 90001",
                r.esperar(lambda: r.tem(CHAVE_EXEMPLO) and r.tem("NF 90001") and not r.tem("NF 90002")),
                f"foto {r.foto('-6c-clipboard-busca')}",
            )
            # Limpa busca antes de voltar ao Início.
            xdo("key", "ctrl+a", "BackSpace")
            time.sleep(0.4)

            # 7. Voltar ao Início: aviso cai para 1 nota e clicar "Ver compras" leva direto à conferência dessa nota.
            xdo("key", "Escape")
            if r.esperar(lambda: r.tem_igual("Notas de compra")) and not r.tem_igual("Ver compras"):
                r.clicar("Notas de compra")
            if ok(
                "7a. No Início, aviso atualizou para 1 nota de compra aguardando a conferência da autorização",
                r.esperar(lambda: r.tem("1 nota de compra aguardando a conferência da autorização")),
                f"foto {r.foto('-7a-inicio-1')}",
            ):
                r.clicar("Ver compras")
                if ok(
                    "7b. Com 1 única nota aguardando, Ver compras abre direto TelaCompra com o painel de conferência",
                    r.esperar(lambda: r.tem("Conferência no portal da NF-e") and r.tem("NF 90002")),
                    f"foto {r.foto('-7b-nota-unica-direto')}",
                ):
                    # 7c. Prova ausência de corrida de foco: digitar direto (sem clicar Copiar chave nem no campo)
                    # escreve no campo Protocolo de autorização!
                    time.sleep(0.4)
                    r.digitar("777")
                    ok(
                        "7c. Foco automático cai direto no campo Protocolo de autorização sem corrida",
                        r.esperar(lambda: r.tem("777")),
                        f"foto {r.foto('-7c-foco-direto')}",
                    )
                    # 7d. Fluxo negada: escolhe Cancelada, informa senha e bloqueia a nota.
                    r.clicar("Cancelada")
                    ok(
                        "7d. Pílula Cancelada exibe botão Registrar e bloquear",
                        r.esperar(lambda: r.tem_igual("Registrar e bloquear")),
                        f"foto {r.foto('-7d-cancelada')}",
                    )
                    # Foca campo de senha clicando 32 px abaixo do rótulo "Senha do gerente".
                    pts_senha = r.onde("Senha do gerente")
                    if pts_senha:
                        sx, sy = pts_senha[0]
                        xdo("mousemove", str(sx), str(sy + 30), "click", "1")
                        time.sleep(0.2)
                    r.digitar(SENHA["Ana"])
                    r.clicar("Registrar e bloquear")
                    ok(
                        "7e. Nota cancelada é registrada como bloqueada no banco e na interface",
                        r.esperar(lambda: r.tem("esta nota não pode ser lançada"))
                        and ler_sql(pasta, "SELECT autorizacao FROM compra WHERE id = 'cp-t111-2'")[0][0] == "negada",
                        f"foto {r.foto('-7e-bloqueada')}",
                    )
                    # 7f. Volta ao Início: aviso de notas aguardando some; em Compras aparece pílula Bloqueadas · 1.
                    xdo("key", "Escape")
                    ok(
                        "7f. No Início, aviso de notas aguardando conferência desapareceu após zerar a fila",
                        r.esperar(lambda: r.tem("Compras") and not r.tem("aguardando a conferência da autorização")),
                        f"foto {r.foto('-7f-inicio-zerado')}",
                    )
                    r.clicar("Compras")
                    ok(
                        "7g. Em Compras, pílula Bloqueadas · 1 aparece e exibe Nota bloqueada sem botão Conferir",
                        r.esperar(lambda: r.tem_igual("Bloqueadas · 1")),
                        f"foto {r.foto('-7g-bloqueadas-pilula')}",
                    )
                    r.clicar("Bloqueadas · 1")
                    ok(
                        "7h. Filtro Bloqueadas · 1 lista a NF 90002 com chip Nota bloqueada e sem Conferir",
                        r.esperar(lambda: r.tem_igual("Nota bloqueada") and r.tem("NF 90002") and not r.tem_igual("Conferir")),
                        f"foto {r.foto('-7h-bloqueadas-lista')}",
                    )

            with open(r.saida, encoding="utf-8", errors="replace") as reg:
                erros = [linha.strip() for linha in reg if "TypeError" in linha or "ReferenceError" in linha]
            ok("8. Nenhum TypeError/ReferenceError no registro Qt", not erros, str(erros[:3]))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)
        xvfb.terminate()
        xvfb.wait()
    print(f"\n{placar.certos} certo(s), {placar.errados} errado(s)\nfotos em {fotos}")
    sys.exit(0 if placar.errados == 0 else 1)


if __name__ == "__main__":
    main()
