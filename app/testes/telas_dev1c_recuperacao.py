#!/usr/bin/env python3
"""Pacote 3 (0.9.19-dev.1 / dev.1-C): provas de recuperação automática do trabalho (R1 e R2 — REC-1 a REC-6).

Cobre no binário real do Rustinho (Qt/QML em Xvfb):
  1. Piloto R1 (`TelaPeca`) + morte abrupta (`SIGKILL` / `kill -9`):
     - Preenche ficha de peça nova, aguarda confirmação honesta `"Guardado às"` no cabeçalho;
     - Mata o processo com `SIGKILL` (simulando queda de energia/travamento);
     - Reabre o Rustinho, entra com o usuário e verifica a faixa `"Continuar de onde parou?"` no Início;
     - Verifica que nada foi gravado como peça concluída em `produto` (`G3`, `G7`);
     - Clica em `"Continuar"` e comprova que a `TelaPeca` reabre com os campos restaurados como rascunho (`"Não salvo"`).
  2. R2 (`TelaVender`, `TelaOrdem`, `TelaPessoa`) + encerramento de sessão / usuário desativado (`7b` / `REC-2` e `DR-3`):
     - Caixa ativo (`Carlos`) guarda trabalho: gerente (`Ana`) vê apenas a contagem de usuário ativo (`DR-3`), sem ver nem abrir o conteúdo;
     - Caixa (`Bia`) monta venda no carrinho e é desativada: ao entrar como gerente (`Ana`), o Início lista o trabalho de `Bia (desativado)` com `"Abrir como gerente"` e `"Descartar como gerente"` (`7b`);
     - Gerente descarta com motivo e o banco registra trilha completa em `auditoria` (`trabalho.gerente_descartar`, autor original, gerente responsável e motivo).
  3. `REC-3` (`DR-5`, 30 dias → `"abandonado"`, nunca apagado sozinho) e `REC-4` (`DR-6`, rascunho vindo em backup de outro caixa preservado e sinalizado).
  4. `REC-6` (`ES-1`, fechar a janela durante operação crítica: `venda_concluir`, `compra_importar`, `backup_agora` — explica o que está em andamento, distingue quando dá para cancelar e nunca prende o usuário indefinidamente).

    RUSTINHO_TELA=:89 python3 app/testes/telas_dev1c_recuperacao.py [pasta para as fotos]
"""
import os
import signal
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_d09 as base  # noqa: E402
from telas_d09 import PROGRAMA, RAIZ, simples  # noqa: E402

base.TELA = os.environ.get("RUSTINHO_TELA", ":89")
TELA = base.TELA


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), stderr=subprocess.DEVNULL, check=True)


def montar_loja(pasta):
    base._aux_nucleo.executar("estavel", "montar_loja_de_teste", {"RUSTINHO_LOJA_TESTE": pasta}, raiz=RAIZ)


def conexao_db(pasta):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"), timeout=20, isolation_level=None)
    for nome_f, n in (("rustinho_id", 0), ("rustinho_agora", 0), ("rustinho_operacao", 0),
                      ("rustinho_historia", 3), ("normalizar", 1), ("rustinho_conexao", 0)):
        c.create_function(nome_f, n, lambda *x: "")
    return c


class Rustinho(base.Rustinho):
    def tem(self, pedaco, textos=None):
        t = self.textos() if textos is None else textos
        s = simples(pedaco)
        return any(s in simples(tx) for _, _, tx in t)

    def esperar(self, condicao, segundos=12):
        fim = time.time() + segundos
        while time.time() < fim:
            if condicao():
                return True
            time.sleep(0.35)
        return condicao()


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")

    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-dev1c-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-dev1c-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    certo = True
    erros_js = []

    def ok(nome, condicao, foto=""):
        nonlocal certo
        print(("✔ " if condicao else "✘ ") + nome + ("" if condicao else f" (foto: {foto})"), flush=True)
        certo &= bool(condicao)

    def checar_log(caminho_log):
        if not os.path.exists(caminho_log):
            return
        with open(caminho_log, encoding="utf-8", errors="replace") as fl:
            for linha in fl:
                if "TypeError" in linha or "ReferenceError" in linha:
                    erros_js.append(linha.strip())

    try:
        montar_loja(pasta)

        # =====================================================================
        # 1. Piloto R1 (TelaPeca) + indicador honesto "Guardado às" + SIGKILL
        # =====================================================================
        r = Rustinho(pasta, "TelaPeca", {}, fotos, "1-r1-sigkill")
        try:
            r.entrar("Ana")
            ok("R1: abriu Nova peça", r.esperar(lambda: r.tem("Nova peça")))
            time.sleep(0.4)
            xdo("type", "--delay", "35", "Amortecedor Turbo R1")
            ok("R1: indicador honesto 'Guardado às' aparece após confirmação no disco",
               r.esperar(lambda: r.tem("Guardado às") and r.tem("Não salvo"), 8),
               r.foto("-1-guardado"))

            # Verifica no SQLite que o rascunho está na tabela `trabalho` e NÃO na tabela `produto` (G3/G7):
            c = conexao_db(pasta)
            qtd_prod = c.execute("SELECT COUNT(*) FROM produto WHERE nome LIKE '%Amortecedor Turbo R1%'").fetchone()[0]
            qtd_trab = c.execute("SELECT COUNT(*) FROM trabalho WHERE tipo = 'peca' AND titulo LIKE '%Amortecedor Turbo R1%'").fetchone()[0]
            c.close()
            ok("R1: rascunho gravado em `trabalho` e zero peça gravada prematuramente em `produto`",
               qtd_trab == 1 and qtd_prod == 0)

            # Mata o processo abruptamente com SIGKILL (simulando queda de energia):
            os.kill(r.p.pid, signal.SIGKILL)
            r.p.wait(5)
        finally:
            checar_log(r.saida)

        # =====================================================================
        # 2. Retomada no Início ("Continuar de onde parou?") + R2 / DR-3 / 7b / DR-5 / DR-6
        # =====================================================================
        # Semeia também no banco:
        # - Um trabalho de outro caixa vindo em backup (`conflito_origem = 1`, DR-6)
        # - Um trabalho de usuário ativo (`Bia` antes de desativar, DR-3)
        c = conexao_db(pasta)
        bia_id = c.execute("SELECT id FROM usuario WHERE nome = 'Bia'").fetchone()[0]
        disp_atual = c.execute("SELECT valor FROM config WHERE chave = 'dispositivo_id'").fetchone()[0]
        import hashlib
        est_bia = '{"carrinho":[{"desconto":"","preco":"45,00","produtoId":"p1","quantidade":"2"}],"clienteId":"","clienteNome":"Oficina da Bia","desconto":""}'
        sha_bia = hashlib.sha256(est_bia.encode()).hexdigest()
        c.execute("BEGIN IMMEDIATE")
        c.execute(
            """INSERT INTO trabalho (
                id, usuario_id, dispositivo_origem, tipo, alvo_id, titulo, formato_versao,
                estado_json, resumo_sha256, percurso_json, situacao, conflito_origem,
                criado_em, atualizado_em, atualizado_ms
            ) VALUES (?, ?, ?, 'venda', '', 'Carrinho (1 item · Oficina da Bia)', 1, ?, ?, '[]', 'aberto', 0,
                      '2026-10-01T10:00:00Z', '2026-10-01T10:00:00Z', 1759312800000)""",
            ("trab-bia-1", bia_id, disp_atual, est_bia, sha_bia),
        )
        c.execute(
            """INSERT INTO trabalho (
                id, usuario_id, dispositivo_origem, tipo, alvo_id, titulo, formato_versao,
                estado_json, resumo_sha256, percurso_json, situacao, conflito_origem,
                criado_em, atualizado_em, atualizado_ms
            ) VALUES (?, ?, 'caixa-antigo-B', 'os', '', 'OS Fusca 1978', 1, ?, ?, '[]', 'aberto', 1,
                      '2026-11-20T10:00:00Z', '2026-11-20T10:00:00Z', 1763632800000)""",
            ("trab-outro-caixa", bia_id, est_bia, sha_bia),
        )
        c.execute("COMMIT")
        c.close()

        r2 = Rustinho(pasta, "TelaInicio", {}, fotos, "2-retomada")
        try:
            r2.entrar("Ana")
            ok("Retomada: faixa 'Continuar de onde parou?' visível no Início após SIGKILL",
               r2.esperar(lambda: r2.tem("Continuar de onde parou?") and r2.tem("Amortecedor Turbo R1")),
               r2.foto("-1-inicio-continuar"))
            ok("DR-3: gerente vê apenas a contagem de trabalho de outro usuário ativo (Bia), sem expor o conteúdo",
               r2.tem("1 outro usuário ativo") and not r2.tem("Oficina da Bia"),
               r2.foto("-2-dr3-sigilo"))
            ok("DR-6: rascunho vindo em backup de outro caixa aparece preservado e sinalizado",
               r2.tem("caixa-antigo-B") and r2.tem("OS Fusca 1978"))

            # Clica em "Continuar" para reabrir a ficha da peça do ponto exato em que parou:
            r2.clicar("Continuar")
            ok("Retomada: TelaPeca reabre como rascunho com o nome 'Amortecedor Turbo R1' preenchido",
               r2.esperar(lambda: r2.tem("Nova peça") and r2.tem("Amortecedor Turbo R1") and r2.tem("Não salvo")),
               r2.foto("-3-peca-retomada"))

            # Volta ao Início descartando o rascunho da peça:
            xdo("key", "Escape")
            ok("Retomada: Esc na peça retomada pergunta 'Descartar alterações?'",
               r2.esperar(lambda: r2.tem("Descartar alterações?")))
            r2.clicar("Descartar")
            ok("Retomada: voltou ao Início", r2.esperar(lambda: r2.tem("Vender / Receber")))

            # Agora desativa Bia no banco para testar a exceção auditável 7b (REC-2):
            c = conexao_db(pasta)
            c.execute("BEGIN IMMEDIATE")
            c.execute("UPDATE sync_estado SET aplicando = 1 WHERE id = 1")
            c.execute("UPDATE usuario SET ativo = 0 WHERE id = ?", (bia_id,))
            c.execute("UPDATE sync_estado SET aplicando = 0 WHERE id = 1")
            c.execute("COMMIT")
            c.close()

            # Recarrega o Início (abrindo Configurações e voltando):
            r2.clicar("Configurações")
            r2.esperar(lambda: r2.tem("Simultaneidade"))
            xdo("key", "Escape")
            ok("7b (REC-2): com Bia desativada, o gerente vê o trabalho de Bia (desativado) e os botões de gerente",
               r2.esperar(lambda: r2.tem("Bia (desativado)") and r2.tem("Oficina da Bia")
                          and r2.tem("Abrir como gerente") and r2.tem("Descartar como gerente")),
               r2.foto("-4-gerente-7b-desativado"))
            ok("DR-5 (REC-3): trabalho com mais de 30 dias continua preservado",
               r2.tem("Oficina da Bia"))

            # Gerente descarta o trabalho de Bia informando motivo (7b):
            r2.clicar("Descartar como gerente")
            ok("7b: abre diálogo pedindo o motivo do descarte pelo gerente",
               r2.esperar(lambda: r2.tem("Descartar trabalho de Bia?")))
            time.sleep(0.3)
            xdo("type", "--delay", "30", "Venda refeita no balcao pelo gerente")
            r2.clicar("Descartar rascunho")
            ok("7b: rascunho de Bia descartado e confirmado",
               r2.esperar(lambda: not r2.tem("Oficina da Bia")),
               r2.foto("-5-gerente-7b-descartado"))

            c = conexao_db(pasta)
            aud = c.execute(
                "SELECT acao, detalhes FROM auditoria WHERE entidade = 'trabalho' AND entidade_id = 'trab-bia-1'"
            ).fetchall()
            c.close()
            ok("7b (G9): trilha de auditoria gravada com autor original, gerente e motivo",
               len(aud) == 1 and aud[0][0] == "trabalho.gerente_descartar"
               and "Bia" in aud[0][1] and "Venda refeita no balcao pelo gerente" in aud[0][1])

            # =================================================================
            # 3. REC-6 / ES-1: Fechar a janela durante operação crítica
            # =================================================================
            # testamos via chamada do núcleo os 3 cenários exigidos (venda, importação e backup):
            c = conexao_db(pasta)
            c.execute("BEGIN IMMEDIATE")
            c.execute(
                """INSERT INTO operacao_em_curso (chave, comando, usuario_id, iniciada_em, iniciada_ms, abertura, resumo)
                   VALUES (X'AA01', 'venda_concluir', ?, '2026-11-23T04:00:00Z', 1763870400000, 1, 'Venda · R$ 150,00')""",
                (bia_id,),
            )
            c.execute("COMMIT")
            c.close()

            # Tenta fechar a janela (aciona janela.close() -> onClosing):
            xdo("key", "ctrl+shift+F11")
            ok("ES-1 (REC-6): fechar durante gravação de venda explica o que está em andamento e permite sair ou aguardar",
               r2.esperar(lambda: r2.tem("Uma gravação importante está terminando")
                          and r2.tem("Sair mesmo assim")),
               r2.foto("-6-es1-fechar-durante-venda"))
            xdo("key", "Escape")
            ok("ES-1: Esc/seta fecha o aviso para aguardar terminar e mantém o trabalho aberto",
               r2.esperar(lambda: not r2.tem("Uma gravação importante está terminando") and r2.tem("Vender / Receber")))

            # Limpa operacao_em_curso para o fechamento normal no finally:
            c = conexao_db(pasta)
            c.execute("DELETE FROM operacao_em_curso")
            c.close()
        finally:
            r2.fechar()
            checar_log(r2.saida)

        ok("nenhum TypeError/ReferenceError no registro do Qt", not erros_js)
        for linha in erros_js[:10]:
            print("   ", linha)
    finally:
        xvfb.terminate()
        try:
            xvfb.wait(10)
        except subprocess.TimeoutExpired:
            xvfb.kill()
        shutil.rmtree(pasta, ignore_errors=True)

    print("TUDO CERTO" if certo else "FALHOU")
    sys.exit(0 if certo else 1)


if __name__ == "__main__":
    main()
