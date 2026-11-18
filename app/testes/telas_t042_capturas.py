#!/usr/bin/env python3
"""T042 (barras de pesquisa em cápsula): fotos de TODAS as barras de pesquisa, nos três temas.

Usa a mesma loja de ensaio, o mesmo ambiente neutro e a mesma foto da janela de `telas_tema_0918.py`. Rodar com o
programa de ANTES e com o de DEPOIS (prefixos diferentes) e comparar com `telas_t042_conferir.py`.

    RUSTINHO_TELA=:68 RUSTINHO_PROGRAMA=<binário> RUSTINHO_TEMA_PREFIXO=antes python3 app/testes/telas_t042_capturas.py <pasta>
    RUSTINHO_T042_CELULAR=1 ...   o desenho do celular (--celular: tela estreita, halo de 14 px, toque de 48 dp)

As 19 barras: as 18 que abrem direto e a da janelinha "Qual é a peça?", aberta em Estoque › "Nova encomenda" ›
"Procurar" (o título e a nota da janelinha vêm ANTES da barra: o halo tem de passar atrás deles).
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
os.environ["RUSTINHO_TEMA_MODO"] = "capturas"
os.environ.setdefault("RUSTINHO_TEMAS", "cinza,amoled,roxo")
import telas_tema_0918 as tema  # noqa: E402

base = tema.base
CELULAR = os.environ.get("RUSTINHO_T042_CELULAR") == "1"

# (nome, tela, propriedades, botões a clicar em sequência depois de entrar)
TELAS = [
    ("inicio", "Nenhuma", {}, None),
    ("pecas", "TelaPecas", {}, None),
    ("vender", "TelaVender", {}, None),
    ("receber", "TelaReceber", {}, None),
    ("orcamentos", "TelaOrcamentos", {}, None),
    ("compras", "TelaCompras", {}, None),
    ("clientes", "TelaPessoas", {"papel": "cliente"}, None),
    ("fornecedores", "TelaPessoas", {"papel": "fornecedor"}, None),
    ("ordens", "TelaOrdens", {}, None),
    ("trocas", "TelaTrocas", {}, None),
    ("etiquetas", "TelaEtiquetas", {}, None),
    ("estoque-comprar", "TelaEstoque", {"secao": "comprar"}, None),
    ("estoque-encomendas", "TelaEstoque", {"secao": "encomendas"}, None),
    ("estoque-vendidos", "TelaEstoque", {"secao": "vendidos"}, None),
    ("estoque-parados", "TelaEstoque", {"secao": "parados"}, None),
    ("financeiro-pagar", "TelaFinanceiro", {"secao": "pagar"}, None),
    ("financeiro-cheques", "TelaFinanceiro", {"secao": "cheques"}, None),
    ("painel-pecas", "TelaPainel", {"secao": "pecas"}, None),
    ("configuracoes-historico", "TelaConfiguracoes", {"secao": "historico"}, None),
    ("janelinha-peca", "TelaEstoque", {"secao": "encomendas"}, ("Nova encomenda", "Procurar")),
]


class Rustinho(tema.Rustinho):
    """O da sonda de temas; com RUSTINHO_T042_CELULAR=1, aberto com o desenho do celular."""

    def __init__(self, pasta, tela, props, fotos, nome):
        if not CELULAR:
            super().__init__(pasta, tela, props, fotos, nome)
            return
        self.fotos, self.nome = fotos, nome
        self.saida = os.path.join(fotos, nome + ".log")
        env = dict(os.environ, DISPLAY=base.TELA, QT_QUICK_BACKEND="software", RUSTINHO_DADOS=pasta,
                   QT_LOGGING_RULES="qml.info=true;js.info=true;default.info=true", QT_SCALE_FACTOR="1")
        env.pop("WAYLAND_DISPLAY", None)
        self.log = open(self.saida, "w")
        self.p = subprocess.Popen([base.PROGRAMA, "--textos-da-tela", "--celular", "--abrir", tela, "--props", json.dumps(props)],
                                  env=env, stdout=self.log, stderr=self.log)
        self.conferir_ambiente()


def capturas(fotos, modelo):
    prefixo = os.environ.get("RUSTINHO_TEMA_PREFIXO", "captura") + ("-celular" if CELULAR else "")
    so = [t for t in os.environ.get("RUSTINHO_TEMA_TELAS", "").split(",") if t]
    erros, faltou = [], []
    for nome_tema in os.environ["RUSTINHO_TEMAS"].split(","):
        for nome, tela, props, botao in TELAS:
            if so and nome not in so:
                continue
            pasta = tempfile.mkdtemp(prefix="rustinho-loja-t042-")
            shutil.rmtree(pasta)
            shutil.copytree(modelo, pasta)
            chave, valor = tema.TEMA_NO_BANCO[nome_tema]
            tema.gravar_tema(pasta, chave, valor)
            r = Rustinho(pasta, tela, props, fotos, f"{prefixo}-{nome_tema}-{nome}")
            try:
                r.entrar("Ana")
                time.sleep(1.5)
                for b in botao or ():
                    if not r.clicar(b):
                        faltou.append(f"{nome_tema}-{nome}: o botão {b!r} não apareceu")
                    time.sleep(1.5)
                if botao:
                    r.textos()  # o registro ganha a geometria da barra da janelinha
                print("foto:", r.foto_janela())
            finally:
                r.fechar()
                shutil.rmtree(pasta, ignore_errors=True)
            erros += tema.erros_de_script(r.saida)
    print(f"{len(erros)} TypeError/ReferenceError nos registros")
    for e in erros + faltou:
        print("   ", e)
    return not erros and not faltou


if __name__ == "__main__":
    tema.modo_capturas = capturas
    tema.main()
