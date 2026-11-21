#!/usr/bin/env python3
"""QT-1b (dev.3.3): o S1 e o S2 pela tela, com o Rustinho de verdade numa tela virtual (Xvfb), sem Wayland. Tirado do
roteiro da 2ª re-revisão independente (Fable, 16/11/2026; `revisao_independente/fable/RELATORIO_REREVISAO2_QT1.md`,
OT-09: um roteiro que grave pela tela e mate o processo). Cada linha afirma o comportamento CERTO.

  A. Gravação pelo `Api.gravar` (TelaPessoa, `pessoa_salvar`): depois do quadro, a operação fica ENTREGUE no banco.
     Morte do processo (SIGKILL); ao reabrir, não há aviso de atenção e aparece a linha informativa do S2. A linha do
     S2 arrastada para a esquerda continua fora quando a pessoa sai e entra de novo (T-04).
  B. Gravação pelo `janela.chamarGerente` (TelaPeca, `produto_salvar`): depois do quadro fica ENTREGUE; morte do
     processo; ao reabrir, a peça salva e vista na tela NÃO vira aviso de atenção (T-01).
  (A sonda do Qt da parte C do revisor, que não é o Rustinho, ficou só no relatório dele.)

    RUSTINHO_TELA=:88 python3 app/testes/telas_qt1_rerevisao2.py [pasta das fotos]
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_navegacao_casos as casos  # noqa: E402
from telas_navegacao_casos import PROGRAMA, TELA, Placar, ler, montar_loja, xdo  # noqa: E402

TITULO = "Atenção: o Rustinho fechou sem o encerramento normal"
S2 = "nada ficou para conferir"
S2_TITULO = "O Rustinho fechou sem o encerramento normal"


def morrer(r):
    """Como a morte do processo: SIGKILL, sem nenhum encerramento."""
    r.p.kill()
    r.p.wait(10)
    r.log.close()


def feitas(pasta, comando):
    """As operações deste comando gravadas na abertura que está correndo (a da tela)."""
    return ler(pasta, "SELECT comando, entregue, abertura FROM operacao_feita WHERE comando = ? "
                      "AND abertura = (SELECT MAX(numero) FROM programa_abertura) ORDER BY feita_ms", comando)


def na_tela(r, pedaco):
    """O texto está DENTRO da janela (a lista de textos também traz o que foi arrastado para fora dela)."""
    return any(pedaco in t and 0 <= x <= 1280 and 0 <= y <= 760 for x, y, t in r.textos())


def parte_a(placar, fotos, pasta):
    ok = placar.ok
    ze = ler(pasta, "SELECT id FROM pessoa WHERE nome = 'Zé da Oficina'")[0][0]
    r = placar.abrir(pasta, "TelaPessoa", {"pessoaId": ze}, fotos, "a1-pessoa")
    try:
        r.entrar("Ana")
        pos = [(x, y) for x, y, t in r.textos() if t == "Zé da Oficina"]
        if not pos:
            raise RuntimeError(f"não achei o campo do nome (veja {r.saida})")
        r.clicar_em(*pos[-1])
        xdo("key", "End")
        xdo("type", "--delay", "60", " X")
        time.sleep(0.6)
        xdo("key", "F2")
        salvo = r.esperar(lambda: r.tem("Alterações salvas"), 6)
        f = r.foto("-1")
        time.sleep(2)
        l = feitas(pasta, "pessoa_salvar")
        print(f"QT1-RR2 A: operacao_feita (comando, entregue, abertura) = {l}", flush=True)
        ok("A1: pessoa_salvar pela tela (Api.gravar) foi gravada", salvo and len(l) == 1, f)
        ok("A2: depois do quadro, ela está ENTREGUE no banco (S1)", len(l) == 1 and l[0][1] == 1, f)
    finally:
        morrer(r)
    r = placar.abrir(pasta, "TelaInicio", {}, fotos, "a2-inicio")
    try:
        r.entrar("Ana")
        time.sleep(1)
        f = r.foto("-1")
        ok("A3: depois da queda, sem o aviso de atenção (a tela tinha mostrado a gravação)", not r.tem(TITULO), f)
        if not r.tem(S2) and r.tem(S2_TITULO):
            r.clicar(S2_TITULO)  # estava minimizada (outro aviso aberto): abre
            time.sleep(1.2)
        f = r.foto("-2")
        ok("A4: aparece a linha informativa do S2", r.tem(S2) and r.tem_igual("Entendi"), f)
        print("QT1-RR2 A: textos do S2 na tela = "
              + str([t for _, _, t in r.textos() if "Rustinho fechou" in t or "conferir" in t or "aberto de novo" in t
                     or t in ("Entendi", "Ver vendas do caixa")]), flush=True)
        # A5 é falha ESPERADA: a OT-02 da 2ª re-revisão do QT-1 (a descrição do S2, com a hora, não aparece na linha),
        # registrada e não corrigida. Se passar a mostrar, o roteiro reprova para tirar da lista (como a geometria).
        if r.tem("aberto de novo às"):
            ok("A5: a hora apareceu, mas estava como falha ESPERADA (OT-02) — tirar da lista", False, f)
        else:
            print(f"✘ A5: a tela mostra a hora em que foi aberto de novo (a descrição do S2) — falha ESPERADA: OT-02 da "
                  f"2ª re-revisão do QT-1 (foto: {f})", flush=True)
        # O gesto de esconder uma informativa: arrastar para a esquerda.
        pos = [(x, y) for x, y, t in r.textos() if S2 in t]
        if pos:
            x, y = pos[0]
            xdo("mousemove", str(x), str(y), "mousedown", "1")
            for i in range(1, 17):
                xdo("mousemove", str(x - 30 * i), str(y))
                time.sleep(0.04)
            xdo("mouseup", "1")
            time.sleep(1.5)
            f = r.foto("-3-arrastado")
            sumiu = not na_tela(r, S2) and not na_tela(r, S2_TITULO)
            # T-04: o arrasto da linha do S2 vale como o "Entendi" de quem arrastou (fechamento_visto, por pessoa), e
            # não como a marca de "escondida" do computador (que quem monta a linha do S2 não lia).
            visto = ler(pasta, "SELECT v.abertura FROM fechamento_visto v JOIN usuario u ON u.id = v.usuario_id "
                               "WHERE u.nome = 'Ana'")
            marca = ler(pasta, "SELECT chave, valor FROM config WHERE chave LIKE 'aviso_escondido.queda%'")
            print(f"QT1-RR2 A: depois do arrasto: fora da janela = {sumiu} · conferido pela Ana = {visto} · "
                  f"marca do computador = {marca}", flush=True)
            ok("A6: arrastar para a esquerda tira a linha do S2 da tela e vale como o Entendi da Ana",
               sumiu and len(visto) >= 1 and not marca, f)
            time.sleep(24)  # o Início confere os avisos de novo a cada 20 s
            f = r.foto("-4-depois-de-24s")
            voltou = na_tela(r, S2) or na_tela(r, S2_TITULO)
            print(f"QT1-RR2 A: 24 s depois, no mesmo Início: a linha voltou = {voltou}", flush=True)
            ok("A7: 24 s depois (o Início leu os avisos de novo), a linha continua fora", not voltou, f)
            # A mesma pessoa sai e entra de novo, no mesmo dia, sem queda nova (A8 e A9). Desde a T084 (texto O do
            # proprietário, 21/11/2026), o "Trocar usuário" abre a tela de Usuários e não sai mais da conta: sair e
            # entrar de novo pela tela deixou de existir (fechar e abrir é uma abertura nova, outro caso). Não se
            # aplica, e diz isso (não conta como aprovado); volta com a D-AUD-3 (doc 8).
            placar.nao_se_aplica("A8 e A9", "desde a T084 (texto O), não há mais o sair da conta pela tela; "
                                 "voltam com a D-AUD-3")
    finally:
        morrer(r)


def parte_b(placar, fotos, pasta):
    ok = placar.ok
    filtro = ler(pasta, "SELECT id FROM produto WHERE nome = 'Filtro de óleo'")[0][0]
    r = placar.abrir(pasta, "TelaPeca", {"produtoId": filtro}, fotos, "b1-peca")
    try:
        r.entrar("Ana")
        if not r.clicar("100,00"):
            raise RuntimeError(f"não achei o preço de revenda (veja {r.saida})")
        xdo("key", "ctrl+a")
        xdo("type", "--delay", "60", "120")
        time.sleep(0.5)
        xdo("key", "F2")
        salvo = r.esperar(lambda: r.tem("Alterações salvas"), 6)
        f = r.foto("-1")
        # Mais quadros desenhados: o aviso verde some, o ponteiro passa por cima dos botões.
        for i in range(8):
            xdo("mousemove", str(300 + 60 * i), str(200 + 30 * i))
            time.sleep(0.5)
        l = feitas(pasta, "produto_salvar")
        print(f"QT1-RR2 B: operacao_feita (comando, entregue, abertura) = {l}", flush=True)
        ok("B1: produto_salvar pela tela (chamarGerente) foi gravada", salvo and len(l) == 1, f)
        ok("B2: depois do quadro (e de 4 s de tela desenhada), ela está ENTREGUE no banco (S1)",
           len(l) == 1 and l[0][1] == 1, f)
    finally:
        morrer(r)
    r = placar.abrir(pasta, "TelaInicio", {}, fotos, "b2-inicio")
    try:
        r.entrar("Ana")
        time.sleep(1)
        if r.tem(TITULO) and not r.tem_igual("Conferi"):
            r.clicar(TITULO)
            time.sleep(1.2)
        f = r.foto("-1")
        textos = [t.strip() for _, _, t in r.textos()]
        gravadas = [textos[i + 1] for i, t in enumerate(textos[:-1]) if t == "Gravada:"]
        print(f"QT1-RR2 B: ao reabrir: aviso de atenção = {r.tem(TITULO)} · linhas 'Gravada' = {gravadas} · S2 = {r.tem(S2)}",
              flush=True)
        ok("B3: a peça salva e mostrada na tela NÃO vira aviso de atenção depois da queda", not r.tem(TITULO), f)
    finally:
        morrer(r)


def roteiro(placar, fotos):
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-qt1rr2-")
    try:
        montar_loja(pasta)
        parte_a(placar, fotos, pasta)
        parte_b(placar, fotos, pasta)
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    aceitas = [":88", ":89", ":91", ":92", ":93"] + [f":{n}" for n in range(60, 75)]
    if TELA not in aceitas:
        sys.exit(f"use RUSTINHO_TELA=:88, :89, :91 a :93 ou :60 a :74 (pedida: {TELA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-qt1rr2-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = Placar()
    try:
        try:
            roteiro(placar, fotos)
        except Exception as e:  # parou no meio: conta como falha
            placar.ok(f"o roteiro chegou ao fim ({e})", False)
        erros = []
        for log in placar.logs:
            with open(log, errors="replace") as fh:
                erros += [ln.strip() for ln in fh if "TypeError" in ln or "ReferenceError" in ln]
        placar.ok("nenhum TypeError/ReferenceError nos registros", not erros, "; ".join(erros[:3]))
    finally:
        xvfb.terminate()
        try:
            xvfb.wait(10)
        except subprocess.TimeoutExpired:
            xvfb.kill()
    pulados = f", não se aplicam: {len(placar.nao_se_aplicam)} ({', '.join(placar.nao_se_aplicam)})" \
        if placar.nao_se_aplicam else ""
    print(f"RESULTADO: {placar.certos} certos, {placar.errados} errados{pulados}")
    print("fotos em", fotos)
    sys.exit(0 if placar.errados == 0 else 1)


if __name__ == "__main__":
    casos.TELA = TELA
    main()
