#!/usr/bin/env python3
"""Código nos fornecedores editável (pedido do proprietário, 16/11/2026), pela tela da peça.

Na loja de teste, a "Pastilha de freio" fica com o código original do fabricante "FAB-123" e dois códigos da
Distribuidora Vale: "00008373" e "25850-0" (caixa com 6). Com o Rustinho de verdade numa tela virtual:
  1. os códigos aparecem na seção "Código nos fornecedores", cada um ao lado do fornecedor;
  2. editar "00008373" ali mesmo e salvar (F2): o código novo fica ligado ao mesmo fornecedor e ao mesmo fator, o
     outro código não muda e o código original continua "FAB-123";
  3. abrir a peça de novo mostra o código novo (e não o velho), e o original no lugar dele;
  4. editar outra vez (edição posterior) grava e volta;
  5. apagar o código e salvar: a tela recusa, diz o motivo, e nada é gravado;
  6. com o código original vazio, editar o do fornecedor não preenche o original; preencher o original não mexe
     no do fornecedor;
  7. o caixa (Bia, não gerente) edita o código do fornecedor sem a senha do gerente (decisão CF-1, 16/11/2026).
Em 2 e 7: a data da ligação fica a de antes (decisão CF-2, 16/11/2026).
  8. um fornecedor de nome comprido, com caixa com 12, aparece inteiro acima do código dele (M-1 da revisão).

    RUSTINHO_TELA=:68 python3 app/testes/telas_codigo_fornecedor.py [pasta das fotos]
    RUSTINHO_PROGRAMA=/caminho/rustinho   (outro binário: o de antes da correção)
    RUSTINHO_CASOS_CELULAR=1               (o desenho do celular)
"""
import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_navegacao_casos as nav  # noqa: E402

TELA = nav.TELA
xdo = nav.xdo
FORNECEDOR = "Distribuidora Vale"
# M-1 da revisão do Fable: um fornecedor de nome comprido (73 letras), com caixa com 12, aparece inteiro.
LONGO = "Distribuidora de Autopeças e Acessórios Vale do Paraíba Ltda. Filial 03"


def medidas(r):
    """As linhas RET: (só no desenho do celular, que liga --medir-toque): {texto: (largura, altura do texto, largura
    natural, largura do texto pintado)}."""
    r.textos()
    m = {}
    for _, _, t in getattr(r, "_medidas", []):
        if t.startswith("RET:"):
            numeros, _, _, texto = t[4:].split("|", 3)
            x, y, w, h, cw, ch, iw = (float(n) for n in numeros.split(","))
            m[texto] = (w, ch, iw, cw)
    return m


def ligacoes(pasta, forn):
    return nav.ler(pasta, "SELECT codigo, codigo_compacto, fator, produto_id FROM produto_fornecedor"
                          " WHERE fornecedor_id = ? ORDER BY codigo_compacto", forn)


def original(pasta, peca):
    return nav.ler(pasta, "SELECT codigo_original FROM produto WHERE id = ?", peca)[0][0]


def data_ligacao(pasta, forn, compacto):
    """A data gravada na ligação (decisão CF-2: corrigir o código não a muda). None se a ligação não existe."""
    r = nav.ler(pasta, "SELECT atualizado_em FROM produto_fornecedor WHERE fornecedor_id = ? AND codigo_compacto = ?",
                forn, compacto)
    return r[0][0] if r else None


def a_vista(r, texto):
    return [p for p in r.onde(texto) if 60 <= p[1] < r.altura() - 30]


def editar(r, de, para):
    """Clica no campo que mostra `de`, apaga tudo e digita `para` (vazio: só apaga)."""
    if not r.rolar_ate(de):
        return False
    x, y = a_vista(r, de)[0]
    r.clicar_em(x, y)
    time.sleep(0.5)
    xdo("key", "ctrl+a")
    xdo("key", "BackSpace")
    if para:
        xdo("type", "--delay", "60", para)
    time.sleep(0.5)
    return True


def salvar(r):
    """F2 e espera o "Alterações salvas."; depois, que o aviso saia (a tela volta ao lugar: AV-1)."""
    xdo("key", "F2")
    salvo = r.esperar(lambda: r.tem("Alterações salvas"), 8)
    if salvo:
        r.esperar(lambda: not r.tem("Alterações salvas"), 10)
        time.sleep(0.5)
    return salvo


def abrir(placar, pasta, peca, fotos, nome, quem="Ana"):
    r = placar.abrir(pasta, "TelaPeca", {"produtoId": peca}, fotos, nome)
    r.entrar(quem)
    r.esperar(lambda: r.tem("Pastilha de freio"), 15)
    time.sleep(1)
    return r


def roteiro(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-fornec-")
    try:
        nav.montar_loja(pasta)
        peca = nav.ler(pasta, "SELECT id FROM produto WHERE nome = 'Pastilha de freio'")[0][0]
        forn = nav.ler(pasta, "SELECT id FROM pessoa WHERE nome = ?", FORNECEDOR)[0][0]
        nav.por_fora(pasta, f"""
            UPDATE produto SET codigo_original = 'FAB-123' WHERE id = '{peca}';
            DELETE FROM produto_fornecedor WHERE produto_id = '{peca}';
            INSERT INTO produto_fornecedor (fornecedor_id, codigo_compacto, codigo, produto_id, fator, atualizado_em)
            VALUES ('{forn}', '00008373', '00008373', '{peca}', 1000, datetime('now')),
                   ('{forn}', '258500', '25850-0', '{peca}', 6000, datetime('now', '-1 day'));
            CREATE TEMP TABLE pf AS SELECT * FROM pessoa WHERE id = '{forn}';
            UPDATE pf SET id = 'forn-longo', nome = '{LONGO}', codigo = 'T-LONGO', documento = '';
            INSERT INTO pessoa SELECT * FROM pf;
            DROP TABLE pf;
            INSERT INTO produto_fornecedor (fornecedor_id, codigo_compacto, codigo, produto_id, fator, atualizado_em)
            VALUES ('forn-longo', 'longo1', 'LONGO-1', '{peca}', 12000, datetime('now', '-2 day'));""")
        inicio = ligacoes(pasta, forn)
        ok("a loja do teste: dois códigos da Distribuidora Vale e o original FAB-123",
           len([x for x in inicio if x[3] == peca]) == 2 and original(pasta, peca) == "FAB-123")

        data_inicio = data_ligacao(pasta, forn, "00008373")
        # 1 e 2. Editar no próprio campo e salvar.
        r = abrir(placar, pasta, peca, fotos, "fornec-1-editar")
        try:
            achou = r.rolar_ate("00008373")
            ok("1. a seção \"Código nos fornecedores\" mostra o código", achou and r.tem("Código nos fornecedores"),
               r.foto("-0"))
            cod = a_vista(r, "00008373")
            nome = [(x, y) for x, y, t in r.textos() if t.strip() == FORNECEDOR]
            # M-1: o nome do fornecedor em cima e o campo do código logo embaixo, na mesma faixa.
            junto = bool(cod) and any(0 < cod[0][1] - p[1] <= 50 for p in nome)
            ok("1. o nome do fornecedor fica logo acima do código, na mesma faixa", junto,
               f"{r.foto('-0b')} código={cod} nome={nome}")
            ok("1. o outro código, com a caixa de 6", r.tem("25850-0") and r.tem("caixa com 6"))
            editar(r, "00008373", "00008374")
            ok("2. salvou (F2)", salvar(r), r.foto("-1"))
            lig = ligacoes(pasta, forn)
            ok("2. gravado: 00008374, mesmo fornecedor, mesma peça e mesmo fator; o 25850-0 igual",
               ("00008374", "00008374", 1000, peca) in lig and ("25850-0", "258500", 6000, peca) in lig
               and not any(x[0] == "00008373" for x in lig), f"{r.foto('-1b')} {lig}")
            ok("2. a data da ligação ficou a de antes (decisão CF-2)",
               data_inicio is not None and data_ligacao(pasta, forn, "00008374") == data_inicio,
               f"{data_inicio} → {data_ligacao(pasta, forn, '00008374')}")
            ok("2. o código original continua FAB-123 (gravado)", original(pasta, peca) == "FAB-123")
            ok("2. o código original continua FAB-123 (na tela)", r.tem_igual("FAB-123"))
        finally:
            r.fechar()

        # 3 e 4. Recuperar (abrir de novo) e editar outra vez.
        r = abrir(placar, pasta, peca, fotos, "fornec-2-recuperar")
        try:
            r.rolar_ate("00008374")
            ok("3. aberta de novo: o código novo no campo, o velho não", bool(a_vista(r, "00008374"))
               and not r.onde("00008373"), r.foto("-0"))
            ok("3. aberta de novo: o original FAB-123 no lugar dele", r.tem_igual("FAB-123"))
            editar(r, "00008374", "PT-0008375")
            ok("4. edição posterior: salvou", salvar(r), r.foto("-1"))
            lig = ligacoes(pasta, forn)
            ok("4. gravado: PT-0008375 (mesmo fator), sem o 00008374",
               ("PT-0008375", "pt0008375", 1000, peca) in lig and not any(x[0] == "00008374" for x in lig), str(lig))
            # 5. Vazio: recusa, com o motivo, e nada é gravado.
            antes = ligacoes(pasta, forn)
            editar(r, "PT-0008375", "")
            xdo("key", "F2")
            recusou = r.esperar(lambda: r.tem("não pode ficar vazio"), 8)
            ok("5. código vazio: a tela diz o motivo", recusou, r.foto("-2"))
            ok("5. código vazio: nada gravado", ligacoes(pasta, forn) == antes and original(pasta, peca) == "FAB-123")
        finally:
            r.fechar()

        # 6. Código original vazio: um campo não passa nada ao outro.
        r = abrir(placar, pasta, peca, fotos, "fornec-3-original")
        try:
            r.rolar_ate("FAB-123")
            editar(r, "FAB-123", "")
            ok("6. apagar o original: salvou", salvar(r), r.foto("-0"))
            ok("6. o original ficou vazio", original(pasta, peca) == "")
            editar(r, "PT-0008375", "PT-0008376")
            ok("6. editar o do fornecedor com o original vazio: salvou", salvar(r), r.foto("-1"))
            lig = ligacoes(pasta, forn)
            ok("6. o do fornecedor mudou e o original continua vazio",
               ("PT-0008376", "pt0008376", 1000, peca) in lig and original(pasta, peca) == "", str(lig))
        finally:
            r.fechar()

        # 6 (fim). Preencher o original (o campo vazio, logo abaixo do rótulo). Com a peça aberta de novo: o campo fica
        # no alto da ficha, e o rolar_ate só desce. Na 1ª rodada (16/11), com a página rolada até os códigos do
        # fornecedor, o rótulo ficou sob o cabeçalho fixo (computador, y = 92) ou acima da tela (celular), e o clique
        # não caiu no campo.
        r = abrir(placar, pasta, peca, fotos, "fornec-4-preencher")
        try:
            rot = a_vista(r, "Código original / fabricante")
            if rot:
                r.clicar_em(rot[0][0], rot[0][1] + 30)
                time.sleep(0.5)
                xdo("type", "--delay", "60", "FAB-9")
                time.sleep(0.5)
            # O que foi digitado tem de estar no campo do original (logo abaixo do rótulo), e não em outro lugar.
            no_campo = bool(rot) and any(rot[0][1] < y <= rot[0][1] + 60 for _, y in r.onde("FAB-9"))
            ok("6. o FAB-9 digitado está no campo do código original", no_campo,
               f"{r.foto('-0')} rótulo={rot} FAB-9={r.onde('FAB-9')}")
            ok("6. preencher o original: salvou", no_campo and salvar(r), r.foto("-1"))
            ok("6. o original é FAB-9 e o do fornecedor não mudou",
               original(pasta, peca) == "FAB-9" and ligacoes(pasta, forn) == lig, str(ligacoes(pasta, forn)))
        finally:
            r.fechar()

        # 7. Decisão CF-1 (16/11/2026): o caixa (Bia, não gerente) edita o código do fornecedor, sem a senha do
        # gerente. E a CF-2 de novo: a data da ligação fica a de antes.
        data_antes = data_ligacao(pasta, forn, "pt0008376")
        r = abrir(placar, pasta, peca, fotos, "fornec-5-caixa", quem="Bia")
        try:
            editar(r, "PT-0008376", "PT-0008377")
            ok("7. o caixa (Bia) salvou o código do fornecedor, sem a senha do gerente", salvar(r), r.foto("-0"))
            lig = ligacoes(pasta, forn)
            ok("7. gravado pelo caixa: PT-0008377 (mesmo fator), sem o PT-0008376",
               ("PT-0008377", "pt0008377", 1000, peca) in lig and not any(x[0] == "PT-0008376" for x in lig), str(lig))
            ok("7. a data da ligação ficou a de antes (decisão CF-2)",
               data_antes is not None and data_ligacao(pasta, forn, "pt0008377") == data_antes,
               f"{data_antes} → {data_ligacao(pasta, forn, 'pt0008377')}")
            ok("7. o código original continua FAB-9", original(pasta, peca) == "FAB-9")
        finally:
            r.fechar()

        # 8. M-1 da revisão do Fable: o fornecedor de nome comprido aparece inteiro, com a caixa com 12, no mesmo texto
        # (quebra a linha, não corta), logo acima do campo do código dele.
        r = abrir(placar, pasta, peca, fotos, "fornec-6-nome-comprido")
        try:
            r.rolar_ate("LONGO-1")
            longo = [(x, y, t) for x, y, t in r.textos() if LONGO in t]
            cod = a_vista(r, "LONGO-1")
            ok("8. o fornecedor de nome comprido aparece inteiro, com a caixa com 12",
               any("caixa com 12" in t for _, _, t in longo), f"{r.foto('-0')} {longo}")
            ok("8. o nome comprido fica logo acima do código dele", bool(cod) and any(0 < cod[0][1] - y <= 70 for _, y, _ in longo),
               f"código={cod} nome={[(x, y) for x, y, _ in longo]}")
            if nav.CELULAR:
                m = [v for t, v in medidas(r).items() if LONGO in t]
                # Pintado em mais de uma linha (altura do texto ≥ 2 linhas de 14 px) e sem passar da largura: quebrou,
                # não cortou. Cortado numa linha só, a altura seria a de uma linha.
                ok("8. no celular, o nome comprido quebra a linha e não é cortado",
                   bool(m) and all(ch >= 30 and cw <= w + 1 for w, ch, iw, cw in m), str(m))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(nav.PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({nav.PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-fornec-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    print("BINÁRIO:", nav.PROGRAMA, "· CELULAR" if nav.CELULAR else "· COMPUTADOR", flush=True)
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = nav.Placar()
    try:
        try:
            roteiro(placar, fotos)
        except Exception as e:
            placar.ok(f"o roteiro chegou ao fim ({e!r})", False)
    finally:
        xvfb.terminate()
        try:
            xvfb.wait(10)
        except subprocess.TimeoutExpired:
            xvfb.kill()
    print(f"RESULTADO: {placar.certos} certos, {placar.errados} errados")
    print("fotos em", fotos)
    sys.exit(0 if placar.errados == 0 else 1)


if __name__ == "__main__":
    main()
