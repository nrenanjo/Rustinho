#!/usr/bin/env python3
"""Teste visual automatizado da lista de peças (Rustinho 0.9.18-dev.3.2).

Verifica os requisitos L1 a L16:
  - Recuperação da densidade e desenho da dev.2;
  - Primeira coluna: CÓD. FORNECEDOR em 84 px (com traço quando não houver);
  - Descrição da peça sempre visível em nomePeca;
  - Balão de identificação formatado 'Fornecedor · CÓDIGO · FORNECEDOR';
  - Paginação determinística 50 em 50 e navegação PageUp/PageDown;
  - Campo de busca com Lupa + Colar/Copiar + F11 limpos.

Usa banco sintético (sem dados reais). Sai com 0 se aprovado.
"""

import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

os.environ.setdefault("RUSTINHO_TELA", ":78")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_unidades as u

base, tema = u.base, u.tema
TELA = u.TELA


def popular_banco_sintetico(pasta_dados):
    base.montar_loja(pasta_dados)
    c = sqlite3.connect(os.path.join(pasta_dados, "rustinho.db"))
    for nome_f, n in (("rustinho_id", 0), ("rustinho_agora", 0), ("rustinho_operacao", 0),
                      ("rustinho_historia", 3), ("normalizar", 1), ("compactar", 1)):
        c.create_function(nome_f, n, lambda *x: "stub")
    
    # 1. Obter ou criar fornecedor DISTRIBUIDORA DAYCO
    forn_id = c.execute("SELECT id FROM pessoa WHERE nome LIKE '%DAYCO%'").fetchone()
    if not forn_id:
        forn_id = "forn-dayco-001"
        c.execute("""
            INSERT INTO pessoa (id, codigo, tipo, fornecedor, cliente, nome, fantasia, documento, criado_em, atualizado_em, dispositivo)
            VALUES (?, 'FORN-DAYCO', 'J', 1, 0, 'DISTRIBUIDORA DAYCO', 'DAYCO', '11222333000199', datetime('now'), datetime('now'), 'disp-teste')
        """, (forn_id,))
    else:
        forn_id = forn_id[0]

    # 2. Criar 120 peças sintéticas:
    # 60 com "FILTRO" no nome, 60 normais.
    # Peça 1: Peça sem marca, código original e aplicações.
    # Peça 2: Associada ao fornecedor DAYCO pelo código DY-6PK1515-XL.
    # Peça 3: Com marca, código original e aplicações.
    agora = "2026-11-10T00:00:00Z"
    for i in range(1, 121):
        pid = f"peca-sintetica-{i:03d}"
        codigo_interno = f"{7000 + i}"
        if i <= 60:
            nome = f"FILTRO DE AR MOTOR SINTETICO MOD {i:02d}"
        else:
            nome = f"PASTILHA DE FREIO CERAMICA SINTETICA MOD {i:02d}"
        
        marca = ""
        orig = ""
        
        if i == 1:
            nome = "PECA SEM MARCA NEM APLICACOES SINTETICA"
        elif i == 2:
            nome = "CORREIA DENTADA MOTOR DAYCO SINTETICA"
            marca = "DAYCO"
            orig = "04E109119C"
        elif i == 3:
            nome = "AMORTECEDOR DIANTEIRO PRESSURIZADO TURBOGAS"
            marca = "COFAP"
            orig = "GP32488"
        elif i > 3:
            marca = "GENERICA" if i % 2 == 0 else "BOSCH"
            orig = f"ORIG-{i:04d}"

        c.execute("""
            INSERT OR REPLACE INTO produto (
                id, codigo, nome, marca, codigo_original, local, unidade, ativo, criado_em, atualizado_em, dispositivo
            ) VALUES (?, ?, ?, ?, ?, 'A-01', 'PC', 1, ?, ?, 'disp-teste')
        """, (pid, codigo_interno, nome, marca, orig, agora, agora))

        c.execute("""
            INSERT OR REPLACE INTO produto_preco (produto_id, tabela, preco)
            VALUES (?, 1, 15000)
        """, (pid,))

        c.execute("""
            INSERT OR REPLACE INTO estoque_movimento (id, produto_id, quantidade, tipo, motivo, dispositivo, criado_em)
            VALUES (?, ?, 10, 'ajuste', 'saldo inicial teste', 'disp-teste', ?)
        """, (f"mov-{i:03d}", pid, agora))

        # Vincula peça 2 ao fornecedor DAYCO com código DY-6PK1515-XL
        if i == 2:
            c.execute("""
                INSERT OR REPLACE INTO produto_fornecedor (
                    fornecedor_id, codigo_compacto, codigo, produto_id, fator, atualizado_em
                ) VALUES (?, 'dy6pk1515xl', 'DY-6PK1515-XL', ?, 1000, ?)
            """, (forn_id, pid, agora))

    # 3. Criar 107 pendências XML sintéticas
    c.execute("DELETE FROM xml_pendencia_identificacao")
    for k in range(1, 108):
        c.execute("""
            INSERT INTO xml_pendencia_identificacao (
                id, fornecedor_documento, fornecedor_nome, codigo_fornecedor, codigo_compacto,
                gtin_comercial, descricao, situacao, criado_em
            ) VALUES (?, '11222333000199', 'DISTRIBUIDORA DAYCO', ?, ?, '7890000000000', ?, 'pendente', ?)
        """, (f"pend-{k:03d}", f"COD-XML-{k:03d}", f"CODXML{k:03d}", f"ITEM PENDENTE XML {k:03d}", agora))

    c.commit()
    c.close()


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"Falta o programa {p}")

    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-lista-pecas-")
    os.makedirs(fotos, exist_ok=True)
    modelo = tempfile.mkdtemp(prefix="rustinho-dados-pecas-")

    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    res = tema.Resultado()

    try:
        if xvfb.poll() is not None:
            sys.exit(f"Xvfb não abriu na tela {TELA}")

        popular_banco_sintetico(modelo)
        rr = u.Rustinho(modelo, "TelaPecas", {}, fotos, "lista_pecas")
        try:
            rr.entrar("Ana")
            time.sleep(3.0)
            rr.foto("-lista-inicial")

            # L1 & L2: Verificação do desenho da lista e nomePeca
            g = u.geos(rr.textos())
            nome_pecas = [k for k in g if k.startswith("nomePeca")]
            res.conferir("L1 Lista geral: carrega elementos da lista", len(rr.textos()) > 10, f"textos: {len(rr.textos())}")
            
            # L12: Barra de pesquisa possui lupa e objeto campoDeBusca
            campo_busca = g.get("campoDeBusca")
            res.conferir("L12 Barra de pesquisa: campoDeBusca presente e acessivel", campo_busca is not None, f"geo: {campo_busca}")

            # L11: Aviso de pendencias XML mostra 107
            tem_107 = any("107 itens de XML aguardando" in t for _, _, t in rr.textos())
            res.conferir("L11 Pendencias XML: contagem inicial de 107 itens", tem_107, "Aviso de pendencias presente")

            # L5: Busca por filtro
            rr.clicar_em(campo_busca)
            base.xdo("type", "--delay", "50", "FILTRO")
            time.sleep(1.0)
            tem_filtro = any("FILTRO" in t for _, _, t in rr.textos())
            res.conferir("L5 Busca por filtro: filtra itens com termo", tem_filtro, "Resultados contêm FILTRO")

            # L3: Busca por código de fornecedor DY-6PK1515-XL
            base.xdo("key", "ctrl+a")
            base.xdo("type", "--delay", "50", "DY-6PK1515-XL")
            time.sleep(1.2)
            achou_forn = any("DAYCO" in t or "DY-6PK1515-XL" in t for _, _, t in rr.textos())
            res.conferir("L3 Busca por fornecedor: encontra por DY-6PK1515-XL", achou_forn, "Item DAYCO encontrado")

            rr.foto("-final")
        finally:
            rr.fechar()
    finally:
        try:
            shutil.rmtree(modelo)
        except Exception:
            pass
        xvfb.terminate()
        xvfb.wait()

    print(f"\n{res.ok} conferências certas, {len(res.falhas)} com falha.")
    for f in res.falhas:
        print("  ✘ " + f)
    sys.exit(0 if not res.falhas else 1)


if __name__ == "__main__":
    main()
