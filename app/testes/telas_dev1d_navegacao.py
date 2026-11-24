#!/usr/bin/env python3
"""Roteiro de validação do Pacote 4 (0.9.19-dev.1-D): Navegação Contextual Guiada completa.

Cobre de ponta a ponta (em Xvfb com o binário real):
  1. Estático (H25, S-3, T-02): ausência de `Timer { interval: 60 }` em Guia.qml,
     presença de `FrameAnimation` e recorte `camadaDestaque`, barreira
     `onAfterSynchronizing` -> `onFrameSwapped` em Main.qml, espelho Android íntegro.
  2. MG-D1 / MG-8: Configurações › Aparência acessível a gerente e caixa com
     "Movimento reduzido" (padrão "Seguir o computador", alternância Reduzido/Normal
     persistida e respeito a `RUSTINHO_MOVIMENTO_REDUZIDO_SISTEMA=1`).
  3. P6 (`--medir-movimento`): rolagem suave com múltiplos passos (250–450 ms),
     destaque entrando somente após a chegada (`A6`), erro de alinhamento <= 2 px (`A5`),
     recorte S-3 abaixo do cabeçalho (`A10`), e modo reduzido em 0 ms (`A9`).
  4. NCG-1 + NCG-2: validação progressiva campo a campo (3 pendências em TelaPeca),
     avanço automático após correção de cada campo, transição para
     "Requisito satisfeito — conclua a operação" SEM gravação automática no SQLite,
     persistência comprovada por > 10 s (não some após 9 s), gravação somente no Salvar.
  5. NCG-3 (F11): navegação livre a partir de TelaVender (preservando carrinho,
     desconto e cliente sem banner "Você veio de...") e a partir de TelaPeca com
     validação progressiva ativa (restaurando a etapa e a notificação ao voltar).
  6. BUG-20261118-FXVT: desistir de guia e voltar ao Início em < 8 s sem vazar
     "A operação ainda não foi concluída" para o Início.

Uso:
    python3 app/testes/telas_dev1d_navegacao.py [pasta_fotos]
"""

import json
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_d09 as base  # noqa: E402
from telas_d09 import PROGRAMA, RAIZ, simples  # noqa: E402

base.TELA = os.environ.get("RUSTINHO_TELA", ":84")
TELA = base.TELA


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), check=True)


def sql(pasta, query, params=()):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    try:
        r = c.execute(query, params).fetchall()
        c.commit()
        return r
    finally:
        c.close()


class App(base.Rustinho):
    def __init__(self, pasta, tela, props, fotos, nome, args_extras=(), env_extras=None):
        self._args_extras = list(args_extras)
        self._env_extras = dict(env_extras or {})
        self.fotos = fotos
        self.nome = nome
        self.saida = os.path.join(fotos, f"{nome}.log")
        self.log = open(self.saida, "wb")
        env = dict(os.environ, DISPLAY=TELA, RUSTINHO_DADOS=pasta, QT_QPA_PLATFORM="xcb")
        env.update(self._env_extras)
        cmd = [PROGRAMA, "--textos-da-tela"] + self._args_extras
        if tela:
            cmd += ["--abrir", tela]
        if props:
            cmd += ["--props", json.dumps(props, ensure_ascii=False)]
        self.p = subprocess.Popen(cmd, env=env, stdout=self.log, stderr=subprocess.STDOUT)
        self._pos = 0
        self._esperar_entrar()

    def _esperar_entrar(self):
        fim = time.time() + 25
        while time.time() < fim:
            if self.p.poll() is not None:
                raise RuntimeError(f"Rustinho fechou cedo ({self.saida})")
            try:
                if self.tem("Quem está usando?"):
                    return
            except RuntimeError:
                pass
            time.sleep(0.4)
        raise RuntimeError(f"tela de entrada não apareceu ({self.saida})")

    def tem(self, texto):
        return any(simples(texto) in simples(t) for _, _, t in self.textos())

    def valor_teste(self, chave):
        pref = chave + ":"
        for _, _, t in self.textos():
            if t.startswith(pref):
                return t[len(pref):]
        return None

    def esperar(self, pred, segundos=15):
        fim = time.time() + segundos
        while time.time() < fim:
            try:
                if pred(self) if callable(pred) else self.tem(pred):
                    return True
            except RuntimeError:
                pass
            time.sleep(0.3)
        return False

    def encolher(self, largura, altura):
        ids = subprocess.run(
            ["xdotool", "search", "--onlyvisible", "--class", "rustinho"],
            env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True,
        ).stdout.split()
        for i in ids:
            xdo("windowsize", i, str(largura), str(altura))
        time.sleep(1.0)

    def eventos_mov(self):
        eventos = []
        with open(self.saida, encoding="utf-8", errors="replace") as f:
            for ln in f:
                idx = ln.find("MOV:")
                if idx >= 0:
                    try:
                        eventos.append(json.loads(ln[idx + 4:].strip()))
                    except json.JSONDecodeError:
                        pass
        return eventos

    def posicao_texto(self, texto):
        for x, y, t in self.textos():
            if simples(t) == simples(texto):
                return x, y
        return None

    def rolar_roda(self, passos, x=500, y=350):
        """passos > 0 rola para baixo (button 5); passos < 0 rola para cima (button 4)."""
        botao = "5" if passos > 0 else "4"
        xdo("mousemove", str(x), str(y))
        for _ in range(abs(passos)):
            xdo("click", botao)
            time.sleep(0.03)
        time.sleep(0.3)

    def clicar_campo_por_rotulo(self, rotulo, altura_janela=760, dica=None):
        """Rola se necessário até o campo ficar visível e clica no centro do TextField."""
        alvo = dica or rotulo
        desloc = 0 if dica else 32
        for _ in range(14):
            p = self.posicao_texto(alvo)
            if p and 90 <= p[1] <= altura_janela - 70:
                time.sleep(0.35)
                p = self.posicao_texto(alvo) or p
                xdo("mousemove", str(p[0]), str(p[1] + desloc), "click", "1")
                time.sleep(0.25)
                return True
            if p and p[1] > altura_janela - 70:
                self.rolar_roda(8, x=min(p[0], 600), y=altura_janela // 2)
            elif p and p[1] < 90:
                self.rolar_roda(-8, x=min(p[0], 600), y=altura_janela // 2)
            else:
                self.rolar_roda(8, x=500, y=altura_janela // 2)
        return False

    def esperar_chegada(self, segundos=10):
        """Espera a rolagem guiada concluir e o destaque/foco estar ativo (determinístico)."""
        return self.esperar(
            lambda a: a.valor_teste("TESTE_MOVIMENTO_ATIVO") == "0"
            and a.valor_teste("TESTE_DESTAQUE_ATIVO") == "1",
            segundos,
        )

    def erros_qml(self):
        with open(self.saida, encoding="utf-8", errors="replace") as f:
            return [ln.strip() for ln in f if "TypeError" in ln or "ReferenceError" in ln]


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")

    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-dev1d-")
    os.makedirs(fotos, exist_ok=True)
    raiz = tempfile.mkdtemp(prefix="rustinho-dev1d-")
    modelo = os.path.join(raiz, "loja-modelo")
    os.makedirs(modelo)
    pasta = os.path.join(raiz, "loja")

    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    certo = True
    logs_erros = []

    def ok(nome, cond, detalhe=""):
        nonlocal certo
        print(("✔ " if cond else "✘ ") + nome + (f" ({detalhe})" if detalhe and not cond else ""))
        certo &= bool(cond)

    def nova_loja():
        shutil.rmtree(pasta, ignore_errors=True)
        shutil.copytree(modelo, pasta)

    try:
        # ---- 1. Verificações estáticas (H25, S-3, T-02, Android) ----
        for rel in ("app/qml/Guia.qml", "android/rust/qml/Guia.qml"):
            txt = open(os.path.join(RAIZ, rel), encoding="utf-8").read()
            ok(f"Estático ({rel}): sem Timer interval: 60 (H25)", "interval: 60" not in txt)
            ok(f"Estático ({rel}): usa FrameAnimation sob demanda", "FrameAnimation" in txt)
            ok(f"Estático ({rel}): recorte S-3 (camadaDestaque)", "camadaDestaque" in txt and "clip: true" in txt)
        for rel in ("app/qml/Main.qml", "android/rust/qml/Main.qml"):
            txt = open(os.path.join(RAIZ, rel), encoding="utf-8").read()
            ok(f"Estático ({rel}): barreira T-02 onAfterSynchronizing",
               "onAfterSynchronizing" in txt and "_entregaSincronizadaNoQuadro" in txt)

        base.montar_loja(modelo)

        # ---- 2. MG-D1 / MG-8: Configurações › Aparência + Movimento reduzido ----
        nova_loja()
        r = App(pasta, "TelaConfiguracoes", {"secao": "aparencia"}, fotos, "1-aparencia-caixa")
        try:
            r.entrar("Bia")  # caixa (não gerente!)
            ok("MG-D1: caixa abre Aparência sem pedir senha do gerente",
               r.esperar("Movimento reduzido") and not r.tem("só o gerente pode"), r.foto("-caixa"))
            ok("MG-D1: padrão inicial é seguir o computador",
               r.valor_teste("TESTE_MODO_MOVIMENTO") == "sistema"
               and r.valor_teste("TESTE_MOVIMENTO_REDUZIDO") == "0",
               f"modo={r.valor_teste('TESTE_MODO_MOVIMENTO')} red={r.valor_teste('TESTE_MOVIMENTO_REDUZIDO')}")
            r.clicar("Reduzido")
            ok("MG-D1: alternar para Reduzido ativa movimento reduzido",
               r.esperar(lambda a: a.valor_teste("TESTE_MODO_MOVIMENTO") == "ligado"
                         and a.valor_teste("TESTE_MOVIMENTO_REDUZIDO") == "1"),
               r.foto("-reduzido"))
            r.clicar("Normal")
            ok("MG-D1: alternar para Normal desativa movimento reduzido",
               r.esperar(lambda a: a.valor_teste("TESTE_MODO_MOVIMENTO") == "desligado"
                         and a.valor_teste("TESTE_MOVIMENTO_REDUZIDO") == "0"),
               r.foto("-normal"))
            r.clicar("Seguir o computador")
            ok("MG-D1: volta para Seguir o computador",
               r.esperar(lambda a: a.valor_teste("TESTE_MODO_MOVIMENTO") == "sistema"),
               r.foto("-sistema"))
            logs_erros += r.erros_qml()
        finally:
            r.fechar()

        # Respeito à preferência do sistema operacional (RUSTINHO_MOVIMENTO_REDUZIDO_SISTEMA=1)
        r = App(pasta, "TelaConfiguracoes", {"secao": "aparencia"}, fotos, "1b-mov-sistema",
                env_extras={"RUSTINHO_MOVIMENTO_REDUZIDO_SISTEMA": "1"})
        try:
            r.entrar("Ana")
            ok("MG-8: modo 'sistema' respeita preferência reduzida do SO",
               r.esperar(lambda a: a.valor_teste("TESTE_MODO_MOVIMENTO") == "sistema"
                         and a.valor_teste("TESTE_MOVIMENTO_REDUZIDO") == "1"),
               r.foto("-so"))
            logs_erros += r.erros_qml()
        finally:
            r.fechar()

        # ---- 3. P6 (--medir-movimento): A1, A2, A5, A6, A9, A10 ----
        nova_loja()
        r = App(pasta, "TelaPeca", {"produtoId": ""}, fotos, "2-movimento-normal",
                args_extras=["--medir-movimento"])
        try:
            r.entrar("Ana")
            ok("P6: abriu Nova peça", r.esperar("Nova peça"), r.foto("-inicio"))
            r.encolher(900, 520)
            # Preenche nome e marca preço aberto, depois coloca NCM inválido abaixo da dobra.
            xdo("type", "--delay", "25", "Peca Teste Movimento")
            time.sleep(0.3)
            r.clicar("Preço digitado na hora da venda")
            time.sleep(0.3)
            ok("P6: alcançou campo NCM na rolagem", r.clicar_campo_por_rotulo("NCM", altura_janela=520, dica="8 números"), r.foto("-antes-ncm"))
            xdo("type", "--delay", "25", "123")
            time.sleep(0.3)
            # Volta ao topo com a roda do mouse.
            r.rolar_roda(-25, x=450, y=280)
            time.sleep(0.3)
            # Tenta salvar: validação progressiva deve rolar suavemente até o NCM e só então destacar!
            # Não tira foto durante a rolagem para não congelar o Xvfb no meio da curva.
            xdo("key", "F2")
            chegou = r.esperar(lambda a: any(e.get("guia") == "destaque_ativo" for e in a.eventos_mov()), 10)
            ok("P6: validação conduziu ao NCM incompleto e concluiu destaque",
               chegou and r.tem("NCM incompleto"), r.foto("-ncm"))
            evs = r.eventos_mov()
            rolagens = [e for e in evs if e.get("fase") == "rolar"]
            inicios = [e for e in evs if e.get("fase") == "inicio"]
            fins = [e for e in evs if e.get("fase") == "fim"]
            idx_aguarda = next((i for i, e in enumerate(evs) if e.get("guia") == "aguardando_rolagem"), -1)
            idx_ativo = next((i for i, e in enumerate(evs) if e.get("guia") == "destaque_ativo"), -1)
            ativo_ev = evs[idx_ativo] if idx_ativo >= 0 else None
            ys = [e["y"] for e in rolagens]
            dist_total = inicios[-1].get("distancia", 0) if inicios else 0
            dur_real = fins[-1].get("t", 0) if fins else 0
            monotonico = all(ys[i] >= ys[i - 1] - 0.5 for i in range(1, len(ys)))
            ok("P6 A1/A2/MG-9: rolagem guiada em múltiplos passos monotônicos sem câmera lenta (250–450 ms)",
               len(rolagens) >= 3 and len(inicios) >= 1
               and 250 <= inicios[-1].get("duracao", 0) <= 450
               and dist_total > 100 and monotonico
               and 150 <= dur_real <= 1200,
               f"passos={len(rolagens)} dur_real={dur_real}ms inicio={inicios[-1:]}")
            ok("P6 A6: destaque só entra depois que a rolagem conclui",
               idx_aguarda >= 0 and idx_ativo > idx_aguarda
               and evs[idx_aguarda].get("opacidade", 1) == 0,
               f"aguarda={idx_aguarda} ativo={idx_ativo}")
            if ativo_ev:
                erro_px = abs(ativo_ev["destaqueY"] - (ativo_ev["alvoY"] - 8))
                ok("P6 A5: destaque alinhado ao campo (erro <= 2 px)", erro_px <= 2.5,
                   f"erro={erro_px:.1f}px ev={ativo_ev}")
                ok("P6 A10: recorte S-3 abaixo do cabeçalho (clipY >= 120)",
                   ativo_ev.get("clipY", 0) >= 120, f"ev={ativo_ev}")
            else:
                ok("P6 A5/A10: evento destaque_ativo emitido", False, f"evs={evs[-5:]}")
            # P6 A11 (H25): aguarda a animação de descida do topo assentar (sem eventos por 0,4 s)
            # e comprova que em repouso (0,8 s) nenhum polling contínuo dispara eventos.
            fim_assentar = time.time() + 4.5
            qtd_prev = len(r.eventos_mov())
            while time.time() < fim_assentar:
                time.sleep(0.4)
                qtd_now = len(r.eventos_mov())
                if qtd_now == qtd_prev:
                    break
                qtd_prev = qtd_now
            qtd_repouso_1 = len(r.eventos_mov())
            time.sleep(0.8)
            qtd_repouso_2 = len(r.eventos_mov())
            ok("P6 A11/H25: zero atividade/polling de movimento em repouso",
               qtd_repouso_2 == qtd_repouso_1,
               f"antes={qtd_repouso_1} depois={qtd_repouso_2}")
            logs_erros += r.erros_qml()
        finally:
            r.fechar()

        # P6 A3 / MG-7 + A13: grande distância (> 3x altura da área rolável) em janela compacta/estreita
        # no binário real -> esmaece (<= 0.25), salta para perto e desliza suavemente.
        r = App(pasta, "TelaPeca", {"produtoId": ""}, fotos, "2c-movimento-longa",
                args_extras=["--medir-movimento"])
        try:
            r.entrar("Ana")
            r.esperar("Nova peça")
            r.encolher(480, 340)
            xdo("type", "--delay", "25", "Peca Longa Distancia")
            time.sleep(0.2)
            r.clicar("Preço digitado na hora da venda")
            time.sleep(0.2)
            ok("P6 A3/A13: alcançou NCM em janela compacta 480x340",
               r.clicar_campo_por_rotulo("NCM", altura_janela=340, dica="8 números"))
            xdo("type", "--delay", "25", "123")
            time.sleep(0.2)
            r.rolar_roda(-45, x=240, y=220)
            time.sleep(0.4)
            xdo("key", "F2")
            chegou_longa = r.esperar(lambda a: any(e.get("guia") == "destaque_ativo" for e in a.eventos_mov()), 10)
            evs_l = r.eventos_mov()
            fases_l = [e.get("fase") for e in evs_l if "fase" in e]
            min_op_l = min((e.get("opacidadeArea", 1.0) for e in evs_l if e.get("fase") in ("fadeOut", "salto")), default=1.0)
            fim_l = [e for e in evs_l if e.get("fase") == "fim"]
            ok("P6 A3/MG-7 (real): grande distância usa fadeOut (<= 0.25) -> salto -> fadeIn e volta a opacity=1",
               chegou_longa and "inicio_longa" in fases_l and "salto" in fases_l and "fadeIn" in fases_l
               and min_op_l <= 0.25 and bool(fim_l) and fim_l[-1].get("opacidadeArea", 0) == 1,
               f"min_op={min_op_l} fases={fases_l[:12]} {r.foto('-longa')}")
            logs_erros += r.erros_qml()
        finally:
            r.fechar()

        # P6 A8 / MG-6: interrupção manual real por roda do mouse no binário real.
        r = App(pasta, "TelaPeca", {"produtoId": ""}, fotos, "2d-movimento-interrupcao",
                args_extras=["--medir-movimento"])
        try:
            r.entrar("Ana")
            r.esperar("Nova peça")
            r.encolher(480, 340)
            xdo("type", "--delay", "25", "Peca Interrupcao Real")
            time.sleep(0.2)
            r.clicar("Preço digitado na hora da venda")
            time.sleep(0.2)
            r.clicar_campo_por_rotulo("NCM", altura_janela=340, dica="8 números")
            xdo("type", "--delay", "25", "123")
            time.sleep(0.2)
            r.rolar_roda(-45, x=240, y=220)
            time.sleep(0.3)
            # Dispara rolagem guiada e imediatamente envia roda do mouse dentro da área rolável para interromper em voo.
            xdo("key", "F2")
            time.sleep(0.05)
            r.rolar_roda(5, x=240, y=300)
            time.sleep(0.5)
            evs_i = r.eventos_mov()
            interrompeu = any(e.get("interrompido") is True or e.get("fase") == "interrompido" for e in evs_i)
            ev_interr = [e for e in evs_i if e.get("interrompido") is True or e.get("fase") == "interrompido"]
            op_final_i = ev_interr[-1].get("opacidadeArea", 0) if ev_interr else 0
            nao_puxou = bool(ev_interr) and ev_interr[-1].get("y", 9999) < ev_interr[-1].get("destino", 0) - 200
            ok("P6 A8/MG-6 (real): roda do mouse interrompe a rolagem guiada no binário real sem puxar de volta",
               interrompeu and op_final_i == 1 and nao_puxou,
               f"fases={[e.get('fase') for e in evs_i if 'fase' in e]} interr={ev_interr[-1:]} {r.foto('-interr')}")
            logs_erros += r.erros_qml()
        finally:
            r.fechar()

        # P6 A7 / MG-5: recarga da lista com guia ativa mantém e acompanha o registro sem desaparecer.
        nova_loja()
        base._aux_nucleo.executar("estavel", "executar_por_fora", {
            "RUSTINHO_LOJA_TESTE": pasta,
            "RUSTINHO_SQL": """
                INSERT INTO conta_pagar (id, categoria_id, descricao, valor, vencimento, criado_em, dispositivo, competencia)
                  VALUES ('cp-mg5-alvo', 'cat-aluguel', 'Conta MG5 Alvo', 1500, date('now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'),
                          'caixa-teste', strftime('%Y-%m','now'));
                INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo) VALUES
                  ('conta_pagar', '["cp-mg5-outro"]', 'pessoa', '["sumiu-a"]', 'inconsistente', '2025-12-30T00:00:00Z', 'log'),
                  ('conta_pagar', '["cp-mg5-alvo"]',  'pessoa', '["sumiu-b"]', 'inconsistente', '2026-01-02T00:00:00Z', 'log');
            """,
        }, raiz=base.RAIZ)
        r = App(pasta, "TelaFinanceiro", {"secao": "pagar"}, fotos, "2e-movimento-recarga",
                args_extras=["--medir-movimento"])
        try:
            r.entrar("Ana")
            ok("P6 A7/MG-5: abriu conta inconsistente no A pagar", r.esperar("Conta MG5 Alvo"))
            # Clica no botão "Pagar" na mesma linha da conta inconsistente e confirma o pagamento.
            tx_pagar = r.textos()
            y_conta = next((y for _, y, t in tx_pagar if "Conta MG5 Alvo" in t), None)
            if y_conta is not None:
                cand = [(abs(y - y_conta), x, y) for x, y, t in tx_pagar if simples(t) == "pagar"]
                cand.sort()
                if cand:
                    xdo("mousemove", str(cand[0][1]), str(cand[0][2]))
                    xdo("click", "1")
            time.sleep(1.2)
            xdo("key", "Return")
            time.sleep(0.8)
            if not r.tem("Ver o registro"):
                botoes_p = [(x, y) for x, y, t in r.textos() if simples(t) == "pagar"]
                if botoes_p:
                    xdo("mousemove", str(botoes_p[-1][0]), str(botoes_p[-1][1]))
                    xdo("click", "1")
            ok("P6 A7/MG-5: ofereceu Ver o registro", r.esperar("Ver o registro"))
            r.clicar("Ver o registro")
            ok("P6 A7/MG-5: abriu Caixas com destaque no registro alvo",
               r.esperar(lambda a: any(e.get("guia") in ("destaque_ativo", "destaque_direto") for e in a.eventos_mov()), 22),
               r.foto("-antes-recarga"))
            # Espera janela de proteção de clique pós-movimento e aceita o OUTRO registro (o de cima, 2025-12-30),
            # forçando recarga da lista e deslocamento vertical do registro da guia (MG-5 / A7).
            time.sleep(0.7)
            unicos_aceitar = []
            for x, y, t in r.textos_para_clicar():
                if simples(t) == simples("Aceitar como está") and 60 <= y < 740:
                    if not any(abs(y - uy) < 12 for _, uy in unicos_aceitar):
                        unicos_aceitar.append((x, y))
            unicos_aceitar.sort(key=lambda p: p[1])
            if len(unicos_aceitar) >= 2:
                xdo("mousemove", str(unicos_aceitar[0][0]), str(unicos_aceitar[0][1]), "click", "1")
                if r.esperar("O que falta não vai aparecer por isso", 6):
                    time.sleep(0.4)
                    xdo("type", "--delay", "25", "Outro registro MG5")
                    time.sleep(0.2)
                    xdo("key", "Return")
                    r.esperar(lambda a: any(e.get("guia") in ("registro_deslizar", "registro_atualizado") for e in a.eventos_mov()), 10)
            evs_m5 = r.eventos_mov()
            guias_m5 = [e.get("guia") for e in evs_m5 if "guia" in e]
            ok("P6 A7/MG-5 (real): recarga da lista preserva e desliza/atualiza o destaque no registro da guia",
               any(g in ("registro_deslizar", "registro_atualizado") for g in guias_m5)
               and r.tem("Aceitar como está"),
               f"guias={guias_m5} {r.foto('-apos-recarga')}")
            logs_erros += r.erros_qml()
        finally:
            r.fechar()

        # P6 A9: movimento reduzido vai direto em 0 ms sem passos intermediários.
        r = App(pasta, "TelaPeca", {"produtoId": ""}, fotos, "2b-movimento-reduzido",
                args_extras=["--medir-movimento"],
                env_extras={"RUSTINHO_MOVIMENTO_REDUZIDO_SISTEMA": "1"})
        try:
            r.entrar("Ana")
            r.esperar("Nova peça")
            r.encolher(900, 520)
            xdo("type", "--delay", "25", "Peca Teste Reduzido")
            time.sleep(0.2)
            r.clicar("Preço digitado na hora da venda")
            time.sleep(0.2)
            r.clicar_campo_por_rotulo("NCM", altura_janela=520, dica="8 números")
            xdo("type", "--delay", "25", "123")
            time.sleep(0.2)
            r.rolar_roda(-25, x=450, y=280)
            time.sleep(0.3)
            xdo("key", "F2")
            r.esperar("NCM incompleto")
            time.sleep(0.5)
            evs = r.eventos_mov()
            ok("P6 A9: movimento reduzido posiciona sem animação de rolagem",
               any(e.get("fase") == "reduzido" for e in evs)
               and not any(e.get("fase") == "rolar" for e in evs),
               f"fases={[e.get('fase') for e in evs if 'fase' in e]}")
            logs_erros += r.erros_qml()
        finally:
            r.fechar()

        # ---- 4. NCG-1 + NCG-2: validação progressiva + persistência > 10 s + zero gravação automática ----
        nova_loja()
        r = App(pasta, "TelaPeca", {"produtoId": ""}, fotos, "3-ncg1-ncg2")
        try:
            r.entrar("Ana")
            ok("NCG-1: abriu Nova peça", r.esperar("Nova peça"), r.foto("-0"))
            # Deixa nome vazio, preço vazio e coloca NCM inválido "123" (3 pendências).
            ok("NCG-1: clicou no campo NCM para semear 3ª pendência",
               r.clicar_campo_por_rotulo("NCM", altura_janela=760, dica="8 números"), r.foto("-0b-ncm"))
            xdo("type", "--delay", "25", "123")
            time.sleep(0.3)
            # Clica Salvar (F2): deve conduzir ao 1º campo (nome).
            xdo("key", "F2")
            ok("NCG-1: conduz ao 1º campo pendente (nome) com notificação persistente",
               r.esperar("Falta o nome da peça")
               and r.valor_teste("TESTE_GUIA_PERSISTENTE") == "1",
               r.foto("-1-nome"))
            ok("NCG-1: nada gravado com pendências",
               sql(pasta, "SELECT COUNT(*) FROM produto WHERE nome = 'Peca Progressiva NCG'")[0][0] == 0)
            # Corrige o 1º campo (nome): espera a rolagem/foco chegar e digita; deve avançar para preço.
            ok("NCG-1: rolagem e destaque chegaram ao 1º campo (nome)", r.esperar_chegada(10), r.foto("-1b-nome"))
            xdo("type", "--delay", "25", "Peca Progressiva NCG")
            ok("NCG-1: avança automaticamente para o 2º campo (preço)",
               r.esperar("Falta o preço de venda"), r.foto("-2-preco"))
            # Corrige o 2º campo (preço): espera foco chegar e digita; deve avançar para NCM.
            r.esperar_chegada(10)
            time.sleep(0.2)
            xdo("type", "--delay", "25", "59,90")
            ok("NCG-1: avança automaticamente para o 3º campo (NCM)",
               r.esperar("NCM incompleto"), r.foto("-3-ncm"))
            # Corrige o 3º campo (NCM -> 8 dígitos): espera rolagem chegar e corrige.
            r.esperar_chegada(10)
            time.sleep(0.2)
            xdo("key", "ctrl+a")
            xdo("type", "--delay", "25", "84822010")
            ok("NCG-1: muda para 'Requisito satisfeito — conclua a operação'",
               r.esperar("Requisito satisfeito — conclua a operação"), r.foto("-4-satisfeito"))
            ok("NCG-1: NÃO grava automaticamente ao satisfazer o último requisito",
               sql(pasta, "SELECT COUNT(*) FROM produto WHERE nome = 'Peca Progressiva NCG'")[0][0] == 0)
            # NCG-2: espera > 10 s (mais do que o antigo timeout de 9 s) e confirma persistência!
            time.sleep(10.2)
            ok("NCG-2: notificação persistente permanece visível após > 10 s",
               r.tem("Requisito satisfeito — conclua a operação")
               and r.valor_teste("TESTE_GUIA_PERSISTENTE") == "1",
               r.foto("-5-apos10s"))
            ok("NCG-1: ainda nada gravado antes de o usuário clicar Salvar",
               sql(pasta, "SELECT COUNT(*) FROM produto WHERE nome = 'Peca Progressiva NCG'")[0][0] == 0)
            # Agora sim o usuário clica Salvar (F2).
            xdo("key", "F2")
            ok("NCG-1: grava somente após ação explícita em Salvar e encerra notificação",
               r.esperar("Peça cadastrada.")
               and sql(pasta, "SELECT COUNT(*) FROM produto WHERE nome = 'Peca Progressiva NCG'")[0][0] == 1
               and r.valor_teste("TESTE_GUIA_PERSISTENTE") == "0",
               r.foto("-6-salvo"))
            logs_erros += r.erros_qml()
        finally:
            r.fechar()

        # ---- 5. NCG-3 (F11): navegação livre preservando tela de origem e guia sem banner ----
        nova_loja()
        # 5a: TelaVender com item + cliente -> F11 -> TelaPecas -> Esc -> volta intacta.
        r = App(pasta, "TelaVender", {}, fotos, "4a-f11-vender")
        try:
            r.entrar("Ana")
            ok("NCG-3 (Vender): abriu Vender", r.esperar("Vender"), r.foto("-0"))
            # Adiciona a 1ª peça ("Filtro") ao carrinho.
            xdo("type", "--delay", "30", "Filtro")
            time.sleep(0.8)
            xdo("key", "Return")
            ok("NCG-3 (Vender): item entrou no carrinho", r.esperar("Subtotal"), r.foto("-1-item"))
            # Preenche o campo real de cliente (à direita, dica "Nome, CPF, telefone ou placa") e o desconto,
            # deixando o foco ativo no campo de desconto antes do F11.
            clicou_cli = r.clicar("Nome, CPF, telefone ou placa")
            ok("NCG-3 (Vender): clicou no campo real de cliente", clicou_cli)
            time.sleep(0.2)
            xdo("type", "--delay", "30", "Oficina F11")
            time.sleep(0.3)
            pos_cli_antes = [(x, y) for x, y, t in r.textos() if "Oficina F11" in t]
            ok("NCG-3 (Vender): 'Oficina F11' foi digitado no campoCliente (x > 650), não na busca",
               any(x > 650 for x, _ in pos_cli_antes), f"pos={pos_cli_antes}")
            ok("NCG-3 (Vender): clicou no campo Desconto na venda",
               r.clicar_campo_por_rotulo("Desconto na venda", altura_janela=760))
            time.sleep(0.2)
            xdo("type", "--delay", "30", "5")
            time.sleep(0.3)
            # Pressiona F11: deve abrir TelaPecas livremente sem banner "Você veio de".
            xdo("key", "F11")
            ok("NCG-3 (Vender): F11 abriu Peças sem banner 'Você veio de'",
               r.esperar("Nova peça")
               and r.valor_teste("TESTE_F11_SUSPENSAS") == "1"
               and not r.tem("Você veio de")
               and not r.tem("voltar para Vender"),
               r.foto("-2-pecas"))
            # Esc volta exatamente à TelaVender preservando carrinho, campoCliente, desconto e foco/cursor.
            xdo("key", "Escape")
            ok("NCG-3 (Vender): Esc retorna à Vender preservando carrinho, cliente e desconto",
               r.esperar("Subtotal")
               and any(x > 650 for x, _, t in r.textos() if "Oficina F11" in t)
               and r.valor_teste("TESTE_F11_SUSPENSAS") == "0",
               r.foto("-3-voltou"))
            # Digita '0' sem clicar: se o foco/cursor voltou ao campo de desconto, vira '50'.
            time.sleep(0.3)
            xdo("type", "--delay", "30", "0")
            ok("NCG-3 (Vender): foco e cursor voltaram exatamente ao campo editado (5 -> 50)",
               r.esperar(lambda a: any(t.strip() == "50" and x > 650 for x, _, t in a.textos()), 4),
               r.foto("-4-foco"))
            # Guarda de modal: abre diálogo de confirmação de saída e garante que F11 não abre Peças por trás.
            xdo("key", "Escape")
            ok("NCG-3 (Modal): diálogo de confirmação aberto", r.esperar("Limpar esta venda?"))
            xdo("key", "F11")
            time.sleep(0.4)
            ok("NCG-3 (Modal): F11 ignorado com diálogo modal aberto",
               r.tem("Limpar esta venda?") and r.valor_teste("TESTE_F11_SUSPENSAS") == "0",
               r.foto("-5-modal"))
            logs_erros += r.erros_qml()
        finally:
            r.fechar()

        # 5b: TelaPeca com validação progressiva ativa -> F11 -> TelaPecas -> Esc -> restaura guia!
        r = App(pasta, "TelaPeca", {"produtoId": ""}, fotos, "4b-f11-guia")
        try:
            r.entrar("Ana")
            r.esperar("Nova peça")
            xdo("key", "F2")
            ok("NCG-3 (Guia): validação ativa em TelaPeca",
               r.esperar("Falta o nome da peça") and r.valor_teste("TESTE_GUIA_PERSISTENTE") == "1",
               r.foto("-1-ativa"))
            xdo("key", "F11")
            ok("NCG-3 (Guia): F11 suspende guia visualmente e abre Peças limpo",
               r.esperar("Importar XML")
               and not r.tem("Falta o nome da peça")
               and not r.tem("Você veio de")
               and r.valor_teste("TESTE_F11_SUSPENSAS") == "1",
               r.foto("-2-pecas"))
            xdo("key", "Escape")
            ok("NCG-3 (Guia): Esc retorna à TelaPeca restaurando etapa e notificação persistente",
               r.esperar("Falta o nome da peça")
               and r.valor_teste("TESTE_GUIA_PERSISTENTE") == "1"
               and r.valor_teste("TESTE_F11_SUSPENSAS") == "0",
               r.foto("-3-restaurada"))
            logs_erros += r.erros_qml()
        finally:
            r.fechar()

        # ---- 6. BUG-20261118-FXVT: desistir da guia e voltar ao Início em < 8 s ----
        nova_loja()
        r = App(pasta, "TelaConfiguracoes", {"secao": "fiscal"}, fotos, "5-bug-fxvt")
        try:
            r.entrar("Ana")
            ok("FXVT: abriu Fiscal embutido sem CNPJ", r.esperar("Ir para Loja"), r.foto("-0"))
            clicou = r.clicar("Ir para Loja")
            ok("FXVT: iniciou guia UF/CNPJ", clicou and r.esperar("Para continuar, informe a UF"), r.foto("-1-guia"))
            # Desiste da guia (Esc) -> volta ao Fiscal com faixa "A operação ainda não foi concluída".
            xdo("key", "Escape")
            ok("FXVT: desistir mostra faixa transitória no Fiscal",
               r.esperar("A operação ainda não foi concluída"), r.foto("-2-faixa"))
            # Em < 3 s, dá Esc para voltar ao Início: a faixa NÃO pode vazar ao Início!
            xdo("key", "Escape")
            ok("FXVT: ao voltar ao Início em < 8 s, a faixa não aparece no Início",
               r.esperar("Trocar usuário")
               and not r.tem("A operação ainda não foi concluída"),
               r.foto("-3-inicio"))
            logs_erros += r.erros_qml()
        finally:
            r.fechar()

        # 6b: mesmo cenário BUG-20261118-FXVT no modo celular (--celular, janela 412x860).
        nova_loja()
        r = App(pasta, "TelaConfiguracoes", {"secao": "fiscal"}, fotos, "5b-bug-fxvt-celular",
                args_extras=["--celular"])
        try:
            janelas = subprocess.check_output(
                ["xdotool", "search", "--onlyvisible", "--pid", str(r.p.pid)],
                env=dict(os.environ, DISPLAY=TELA), text=True
            ).split()
            if janelas:
                xdo("windowmove", janelas[0], "0", "0")
                xdo("windowsize", janelas[0], "412", "860")
                xdo("windowfocus", "--sync", janelas[0])
                time.sleep(1.0)
            r.entrar("Ana")
            ok("FXVT (celular): abriu Fiscal embutido sem CNPJ", r.esperar("Ir para Loja"), r.foto("-0"))
            clicou = r.clicar("Ir para Loja")
            ok("FXVT (celular): iniciou guia UF/CNPJ",
               clicou and r.esperar("Para continuar, informe a UF"), r.foto("-1-guia"))
            xdo("key", "Escape")
            ok("FXVT (celular): desistir mostra faixa transitória no Fiscal",
               r.esperar("A operação ainda não foi concluída"), r.foto("-2-faixa"))
            xdo("key", "Escape")
            ok("FXVT (celular): ao voltar ao Início em < 8 s, a faixa não aparece no Início",
               r.esperar("Trocar usuário")
               and not r.tem("A operação ainda não foi concluída"),
               r.foto("-3-inicio"))
            logs_erros += r.erros_qml()
        finally:
            r.fechar()

        ok("Geral: nenhum TypeError ou ReferenceError em nenhum cenário",
           len(logs_erros) == 0, "; ".join(logs_erros[:3]))

    finally:
        xvfb.terminate()
        shutil.rmtree(raiz, ignore_errors=True)
    print("fotos em", fotos)
    sys.exit(0 if certo else 1)


if __name__ == "__main__":
    main()
