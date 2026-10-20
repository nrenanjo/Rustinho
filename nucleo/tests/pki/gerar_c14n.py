#!/usr/bin/env python3
"""Casos sintéticos de C14N 1.0 inclusiva (sem comentários) e o resultado do
libxml2 (via lxml), usados pelo teste A-03. O elemento canonicalizado é o que
tem Id="alvo". Para refazer: python3 nucleo/tests/pki/gerar_c14n.py"""
import os
from lxml import etree
AQUI = os.path.join(os.path.dirname(os.path.abspath(__file__)), "c14n")
os.makedirs(AQUI, exist_ok=True)
CASOS = {
 "01_atributos_fora_de_ordem": '<r><e Id="alvo" z="3" b="2" a="1" Ab="0"/></r>',
 "02_padrao_herdado": '<r xmlns="urn:a"><m><e Id="alvo"><f>t</f></e></m></r>',
 "03_prefixo_declarado_no_ancestral": '<r xmlns="urn:a" xmlns:p="urn:p" xmlns:q="urn:q"><e Id="alvo"><f/></e></r>',
 "04_redeclarado_igual_nao_repete": '<r xmlns:p="urn:p"><e Id="alvo"><f xmlns:p="urn:p"><g xmlns="urn:x"/></f></e></r>',
 "05_redeclarado_diferente": '<r xmlns:p="urn:p"><e Id="alvo"><f xmlns:p="urn:outro"/></e></r>',
 "06_padrao_muda_no_descendente": '<r xmlns="urn:a"><e Id="alvo"><f xmlns="urn:b"><g/></f><h/></e></r>',
 "07_padrao_desfeito": '<r xmlns="urn:a"><e Id="alvo"><f xmlns=""><g/></f></e></r>',
 "08_escapes_no_texto": '<r><e Id="alvo">a &amp; b &lt; c &gt; d &#13; e &#x9; f "aspas" \'ap\'</e></r>',
 "09_escapes_no_atributo": '<r><e Id="alvo" a="x &quot;y&quot; &amp; &lt; &#9; &#10; &#13; z" b=\'com "dupla"\' c="tab\tquebra\nfim"/></r>',
 "10_vazios": '<r><e Id="alvo"><a/><b></b><c x="1"/></e></r>',
 "11_espacos_e_crlf": '<r>\r\n  <e Id="alvo">\r\n    <a>1</a>\r\n    <b>2\r\n3</b>\r\n  </e>\r\n</r>',
 "12_acentos": '<r><e Id="alvo" nome="Peça Água"><x>José ção €</x></e></r>',
 "13_misto_profundo": '<r xmlns="urn:nfe"><NFe><e Id="alvo" versao="4.00"><ide><cUF>35</cUF> texto <b>x</b> mais</ide><det nItem="1"><prod><xProd>A &amp; B</xProd></prod></det></e></NFe></r>',
 "14_cdata": '<r><e Id="alvo"><![CDATA[<tag> & "x" ]]></e></r>',
 "15_comentario_some": '<r><e Id="alvo"><!-- fora --><a>1</a><!--x--></e></r>',
 "16_maior_que_e_quebra_no_texto": '<r><e Id="alvo">]]&gt; a\nb</e></r>',
}
for nome, xml in CASOS.items():
    caminho = os.path.join(AQUI, nome + ".xml")
    with open(caminho, "w", encoding="utf-8", newline="") as f:
        f.write('<?xml version="1.0" encoding="UTF-8"?>\n' + xml)
    doc = etree.parse(caminho).getroot()
    alvo = doc.xpath('//*[@Id="alvo"]')[0]
    saida = etree.tostring(etree.fromstring(etree.tostring(alvo)), method="c14n")
    open(os.path.join(AQUI, nome + ".c14n"), "wb").write(saida)
print(len(CASOS), "casos em", AQUI)
