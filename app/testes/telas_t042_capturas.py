#!/usr/bin/env python3
"""T042 (barras de pesquisa em cápsula): fotos de TODAS as telas com barra de pesquisa, nos três temas.

Reaproveita o modo de capturas de `telas_tema_0918.py` (mesma loja de ensaio, mesmo ambiente neutro, mesma janela),
com a lista completa das telas que têm barra de pesquisa. Rodar com o programa de ANTES e com o de DEPOIS (prefixos
diferentes) e comparar com `telas_t042_conferir.py`.

    RUSTINHO_TELA=:68 RUSTINHO_PROGRAMA=<binário> RUSTINHO_TEMA_PREFIXO=antes python3 app/testes/telas_t042_capturas.py <pasta>

Fica de fora a janelinha "Qual é a peça?" (EscolherPeca): ela só abre por um botão de outra tela; a barra dela é a mesma
`Campo` e é a primeira coisa da janelinha (nada antes dela para o halo cobrir).
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
os.environ["RUSTINHO_TEMA_MODO"] = "capturas"
os.environ.setdefault("RUSTINHO_TEMAS", "cinza,amoled,roxo")
import telas_tema_0918 as tema  # noqa: E402

tema.TELAS_A4 = [
    # (nome, tela, propriedades): as 18 barras de pesquisa que abrem direto (inventário da T042)
    ("inicio", "Nenhuma", {}),
    ("pecas", "TelaPecas", {}),
    ("vender", "TelaVender", {}),
    ("receber", "TelaReceber", {}),
    ("orcamentos", "TelaOrcamentos", {}),
    ("compras", "TelaCompras", {}),
    ("clientes", "TelaPessoas", {"papel": "cliente"}),
    ("ordens", "TelaOrdens", {}),
    ("trocas", "TelaTrocas", {}),
    ("etiquetas", "TelaEtiquetas", {}),
    ("estoque-comprar", "TelaEstoque", {"secao": "comprar"}),
    ("estoque-encomendas", "TelaEstoque", {"secao": "encomendas"}),
    ("estoque-vendidos", "TelaEstoque", {"secao": "vendidos"}),
    ("estoque-parados", "TelaEstoque", {"secao": "parados"}),
    ("financeiro-pagar", "TelaFinanceiro", {"secao": "pagar"}),
    ("financeiro-cheques", "TelaFinanceiro", {"secao": "cheques"}),
    ("painel-pecas", "TelaPainel", {"secao": "pecas"}),
    ("configuracoes-historico", "TelaConfiguracoes", {"secao": "historico"}),
]

if __name__ == "__main__":
    tema.main()
