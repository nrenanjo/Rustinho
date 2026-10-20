#!/usr/bin/env python3
"""Infraestrutura de TESTE do PEND-09 (SEM VALIDADE JURÍDICA – SÓ TESTE).

Gera uma raiz e ACs fictícias, certificados de emitente com variações do perfil
ICP-Brasil A1/A3 (uso da chave, EKU, política, titular, validade) e notas de
teste assinadas no padrão da NF-e (XMLDSig enveloped, C14N 1.0, RSA-SHA1).

Nada aqui é real: as chaves são geradas na hora, os nomes dizem "TESTE" e a raiz
só é aceita pelo Rustinho quando o recurso `ancoras-de-teste` está ligado (só
nos testes). Para refazer:  python3 nucleo/tests/pki/gerar.py
Precisa de: python3-cryptography e python3-lxml.
"""
import base64
import datetime as D
import hashlib
import os
import re
import sys

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import padding, rsa
from cryptography.x509.oid import ExtendedKeyUsageOID, NameOID
from lxml import etree

AQUI = os.path.dirname(os.path.abspath(__file__))
SAIDA = os.path.join(AQUI, "xml")
os.makedirs(SAIDA, exist_ok=True)
MODELO = open(os.path.join(AQUI, "nfe_modelo.xml"), encoding="utf-8").read()

UTC = D.timezone.utc
EMISSAO = D.datetime(2026, 10, 8, 13, 15, 0, tzinfo=UTC)  # dhEmi 10:15 -03:00
RECEBIMENTO = D.datetime(2026, 10, 8, 13, 16, 30, tzinfo=UTC)  # dhRecbto 10:16:30 -03:00
LONGE_ANTES = D.datetime(2020, 1, 1, tzinfo=UTC)
LONGE_DEPOIS = D.datetime(2045, 1, 1, tzinfo=UTC)
CNPJ_EMITENTE = "11222333000181"

DS = "http://www.w3.org/2000/09/xmldsig#"
C14N = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315"
ENVELOPED = DS + "enveloped-signature"


def chave(bits=2048):
    return rsa.generate_private_key(public_exponent=65537, key_size=bits)


def nome(cn):
    return x509.Name([
        x509.NameAttribute(NameOID.COUNTRY_NAME, "BR"),
        x509.NameAttribute(NameOID.ORGANIZATION_NAME, "ICP-Brasil"),
        x509.NameAttribute(NameOID.COMMON_NAME, cn),
    ])


def outro_nome(oid, texto, tag=0x04):
    v = texto.encode("ascii")
    return x509.OtherName(x509.ObjectIdentifier(oid), bytes([tag, len(v)]) + v)


def emitir_ac(cn, chave_ac, emissor_nome, emissor_chave, de=LONGE_ANTES, ate=LONGE_DEPOIS,
              cert_sign=True, path_len=None):
    b = (x509.CertificateBuilder().subject_name(nome(cn)).issuer_name(emissor_nome)
         .public_key(chave_ac.public_key()).serial_number(x509.random_serial_number())
         .not_valid_before(de).not_valid_after(ate)
         .add_extension(x509.BasicConstraints(ca=True, path_length=path_len), critical=True)
         .add_extension(x509.KeyUsage(digital_signature=False, content_commitment=False, key_encipherment=False,
                                      data_encipherment=False, key_agreement=False, key_cert_sign=cert_sign,
                                      crl_sign=True, encipher_only=False, decipher_only=False), critical=True)
         .add_extension(x509.SubjectKeyIdentifier.from_public_key(chave_ac.public_key()), critical=False))
    if emissor_chave is not chave_ac:
        b = b.add_extension(x509.AuthorityKeyIdentifier.from_issuer_public_key(emissor_chave.public_key()),
                            critical=False)
    return b.sign(emissor_chave, hashes.SHA512())


def emitir_folha(cn, chave_folha, emissor, emissor_chave, *, de=LONGE_ANTES, ate=LONGE_DEPOIS,
                 ku="padrao", eku="padrao", politica="2.16.76.1.2.1.9999", ca=False, sem_bc=False,
                 cnpj=CNPJ_EMITENTE, cpf=None, cnpj_tag=0x04, autoassinado=False):
    b = (x509.CertificateBuilder().subject_name(nome(cn))
         .issuer_name(nome(cn) if autoassinado else emissor.subject)
         .public_key(chave_folha.public_key()).serial_number(x509.random_serial_number())
         .not_valid_before(de).not_valid_after(ate))
    if not sem_bc:
        b = b.add_extension(x509.BasicConstraints(ca=ca, path_length=None), critical=ca)
    if ku != "ausente":
        bits = dict(digital_signature=True, content_commitment=True, key_encipherment=True)
        if ku == "sem_ds":
            bits["digital_signature"] = False
        if ku == "sem_nr":
            bits["content_commitment"] = False
        if ku == "sem_ke":
            bits["key_encipherment"] = False
        b = b.add_extension(x509.KeyUsage(data_encipherment=False, key_agreement=False, key_cert_sign=ca,
                                          crl_sign=False, encipher_only=False, decipher_only=False, **bits),
                            critical=(ku != "nao_critica"))
    usos = {
        "padrao": [ExtendedKeyUsageOID.CLIENT_AUTH, ExtendedKeyUsageOID.EMAIL_PROTECTION],
        "so_cliente": [ExtendedKeyUsageOID.CLIENT_AUTH],
        "so_email": [ExtendedKeyUsageOID.EMAIL_PROTECTION],
        "so_servidor": [ExtendedKeyUsageOID.SERVER_AUTH],
        "com_cartao": [ExtendedKeyUsageOID.CLIENT_AUTH, ExtendedKeyUsageOID.EMAIL_PROTECTION,
                       x509.ObjectIdentifier("1.3.6.1.4.1.311.20.2.2")],
    }
    if eku != "ausente":
        b = b.add_extension(x509.ExtendedKeyUsage(usos[eku]), critical=False)
    if politica:
        b = b.add_extension(x509.CertificatePolicies([x509.PolicyInformation(x509.ObjectIdentifier(politica), None)]),
                            critical=False)
    nomes = [outro_nome("2.16.76.1.3.2", "RESPONSAVEL DE TESTE")]
    if cpf:
        nomes.append(outro_nome("2.16.76.1.3.1", "01011990" + cpf + "00000000000" + "000000000000000" + "SSPSP     "))
    if cnpj:
        nomes += [outro_nome("2.16.76.1.3.3", cnpj, cnpj_tag),
                  outro_nome("2.16.76.1.3.4", "01011990" + "12345678909" + "00000000000" + "000000000000000" + "SSPSP "),
                  outro_nome("2.16.76.1.3.7", "000000000000")]
    b = b.add_extension(x509.SubjectAlternativeName(nomes), critical=False)
    return b.sign(chave_folha if autoassinado else emissor_chave, hashes.SHA256())


def der(c):
    return c.public_bytes(serialization.Encoding.DER)


def c14n(elemento):
    # C14N 1.0 inclusiva do elemento no contexto do documento (o modelo só
    # declara o namespace padrão, como as notas reais).
    return etree.tostring(etree.fromstring(etree.tostring(elemento)), method="c14n")


def assinar(texto, chave_assina, cert, *, alg="sha1", mexer=None):
    """Assina o modelo como um emissor de NF-e: resumo do infNFe, SignedInfo em
    C14N, RSA. `mexer(texto)` altera o XML já assinado (adulterações)."""
    doc = etree.fromstring(texto.replace("@@ASSINATURA@@", "").replace("@@DIGVAL@@", "x").encode())
    inf = doc.find(".//{http://www.portalfiscal.inf.br/nfe}infNFe")
    h = hashlib.sha1 if alg == "sha1" else hashlib.sha256
    resumo = base64.b64encode(h(c14n(inf)).digest()).decode()
    metodo_resumo = DS + "sha1" if alg == "sha1" else "http://www.w3.org/2001/04/xmlenc#sha256"
    metodo_assina = DS + "rsa-sha1" if alg == "sha1" else "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256"
    info = (f'<SignedInfo><CanonicalizationMethod Algorithm="{C14N}"/><SignatureMethod Algorithm="{metodo_assina}"/>'
            f'<Reference URI="#{inf.get("Id")}"><Transforms><Transform Algorithm="{ENVELOPED}"/>'
            f'<Transform Algorithm="{C14N}"/></Transforms><DigestMethod Algorithm="{metodo_resumo}"/>'
            f'<DigestValue>{resumo}</DigestValue></Reference></SignedInfo>')
    assinatura = f'<Signature xmlns="{DS}">{info}<SignatureValue>@@SV@@</SignatureValue><KeyInfo><X509Data><X509Certificate>' \
                 + base64.b64encode(der(cert)).decode() + "</X509Certificate></X509Data></KeyInfo></Signature>"
    montado = texto.replace("@@ASSINATURA@@", assinatura).replace("@@DIGVAL@@", resumo)
    d2 = etree.fromstring(montado.encode())
    si = d2.find(".//{%s}SignedInfo" % DS)
    hh = hashes.SHA1() if alg == "sha1" else hashes.SHA256()
    sv = base64.b64encode(chave_assina.sign(c14n(si), padding.PKCS1v15(), hh)).decode()
    final = montado.replace("@@SV@@", sv)
    return mexer(final) if mexer else final


def gravar(nome_arquivo, texto):
    with open(os.path.join(SAIDA, nome_arquivo), "w", encoding="utf-8", newline="") as f:
        f.write(texto)


def main():
    raiz_k = chave(2048)
    raiz = emitir_ac("Raiz de TESTE do Rustinho SEM VALIDADE", raiz_k, nome("Raiz de TESTE do Rustinho SEM VALIDADE"),
                     raiz_k)
    ac_k = chave(2048)
    ac = emitir_ac("AC de TESTE do Rustinho SEM VALIDADE", ac_k, raiz.subject, raiz_k)
    open(os.path.join(AQUI, "raiz_teste.der"), "wb").write(der(raiz))
    open(os.path.join(AQUI, "ac_teste.der"), "wb").write(der(ac))

    def folha(**kw):
        k = kw.pop("chave", None) or chave(kw.pop("bits", 2048))
        return k, emitir_folha("EMITENTE DE TESTE LTDA:" + kw.get("cnpj", CNPJ_EMITENTE) if kw.get("cnpj") else
                               "EMITENTE DE TESTE", k, ac, ac_k, **kw)

    # Emitente certo (base de quase todos os casos). A chave é reutilizada.
    ok_k, ok = folha()
    base = assinar(MODELO, ok_k, ok)
    open(os.path.join(AQUI, "..", "nfe_exemplo.xml"), "w", encoding="utf-8", newline="").write(base)
    gravar("ok.xml", base)
    gravar("ok_sha256.xml", assinar(MODELO, ok_k, ok, alg="sha256"))

    casos = {
        # Uso da chave e extensões (U1–U5)
        "ku_ausente": dict(ku="ausente"), "ku_nao_critica": dict(ku="nao_critica"), "ku_sem_ds": dict(ku="sem_ds"),
        "ku_sem_nr": dict(ku="sem_nr"), "ku_sem_ke": dict(ku="sem_ke"),
        "eku_ausente": dict(eku="ausente"), "eku_so_servidor": dict(eku="so_servidor"),
        "eku_so_cliente": dict(eku="so_cliente"), "eku_so_email": dict(eku="so_email"),
        "eku_com_cartao": dict(eku="com_cartao"),
        "politica_a3": dict(politica="2.16.76.1.2.3.9999"), "politica_a4": dict(politica="2.16.76.1.2.4.9999"),
        "politica_ausente": dict(politica=None), "politica_ssl": dict(politica="2.16.76.1.2.10.9999"),
        "folha_ca": dict(ca=True), "sem_basic_constraints": dict(sem_bc=True),
        "cnpj_printable": dict(cnpj_tag=0x13),
        # Titular (U6)
        "cnpj_outra_raiz": dict(cnpj="99888777000155"), "cnpj_filial": dict(cnpj="11222333000262"),
        "sem_cnpj": dict(cnpj=None),
        # Validade (§3.5)
        "vencido_antes_emissao": dict(ate=EMISSAO - D.timedelta(seconds=1)),
        "comeca_depois_emissao": dict(de=EMISSAO + D.timedelta(seconds=1)),
        "vence_antes_recebimento": dict(ate=EMISSAO + D.timedelta(seconds=30)),
        "vencido_hoje": dict(de=D.datetime(2026, 1, 1, tzinfo=UTC), ate=D.datetime(2026, 10, 9, tzinfo=UTC)),
        "rsa1024": dict(bits=1024),
    }
    for nome_caso, kw in casos.items():
        k, c = folha(**kw)
        gravar(nome_caso + ".xml", assinar(MODELO, k, c))

    # Emitente pessoa física (CPF)
    modelo_cpf = MODELO.replace("<CNPJ>11222333000181</CNPJ>", "<CPF>12345678909</CPF>", 1)
    k, c = folha(cnpj=None, cpf="12345678909")
    gravar("cpf_ok.xml", assinar(modelo_cpf, k, c))
    k, c = folha(cnpj=None, cpf="98765432100")
    gravar("cpf_outro.xml", assinar(modelo_cpf, k, c))

    # Homologação
    gravar("homologacao.xml", assinar(MODELO.replace("<tpAmb>1</tpAmb>", "<tpAmb>2</tpAmb>", 1), ok_k, ok))

    # Cadeia: autoassinado, raiz falsa com o mesmo nome, AC sem keyCertSign,
    # AC vencida no recebimento.
    k = chave()
    gravar("autoassinado.xml", assinar(MODELO, k, emitir_folha("EMITENTE DE TESTE", k, None, None, autoassinado=True)))
    falsa_k = chave()
    falsa = emitir_ac("Raiz de TESTE do Rustinho SEM VALIDADE", falsa_k, nome("Raiz de TESTE do Rustinho SEM VALIDADE"),
                      falsa_k)
    ac_falsa_k = chave()
    ac_falsa = emitir_ac("AC de TESTE do Rustinho SEM VALIDADE", ac_falsa_k, falsa.subject, falsa_k)
    k = chave()
    gravar("raiz_falsa_mesmo_nome.xml", assinar(MODELO, k, emitir_folha("EMITENTE DE TESTE", k, ac_falsa, ac_falsa_k)))
    ac_sem_k = chave()
    ac_sem = emitir_ac("AC SEM KEYCERTSIGN de TESTE", ac_sem_k, raiz.subject, raiz_k, cert_sign=False)
    open(os.path.join(AQUI, "ac_sem_keycertsign.der"), "wb").write(der(ac_sem))
    k = chave()
    gravar("ac_sem_keycertsign.xml", assinar(MODELO, k, emitir_folha("EMITENTE DE TESTE", k, ac_sem, ac_sem_k)))
    ac_venc_k = chave()
    ac_venc = emitir_ac("AC QUE VENCE NO RECEBIMENTO de TESTE", ac_venc_k, raiz.subject, raiz_k,
                        ate=EMISSAO + D.timedelta(seconds=30))
    open(os.path.join(AQUI, "ac_vence_no_recebimento.der"), "wb").write(der(ac_venc))
    k = chave()
    gravar("ac_vence_no_recebimento.xml",
           assinar(MODELO, k, emitir_folha("EMITENTE DE TESTE", k, ac_venc, ac_venc_k)))
    # Raiz que vence entre a emissão e o recebimento (regra histórica da âncora)
    raiz2_k = chave()
    raiz2 = emitir_ac("Raiz HISTORICA de TESTE", raiz2_k, nome("Raiz HISTORICA de TESTE"), raiz2_k,
                      ate=EMISSAO + D.timedelta(seconds=30))
    open(os.path.join(AQUI, "raiz_historica_teste.der"), "wb").write(der(raiz2))
    ac2_k = chave()
    ac2 = emitir_ac("AC sob raiz historica de TESTE", ac2_k, raiz2.subject, raiz2_k)
    open(os.path.join(AQUI, "ac_raiz_historica.der"), "wb").write(der(ac2))
    k = chave()
    gravar("raiz_vence_no_recebimento.xml", assinar(MODELO, k, emitir_folha("EMITENTE DE TESTE", k, ac2, ac2_k)))

    # Nota assinada e depois reassinada por outra chave com o mesmo certificado
    # (assinatura não confere), e certificado de outra empresa no lugar.
    k2 = chave()
    outra = emitir_folha("OUTRA EMPRESA DE TESTE", k2, ac, ac_k, cnpj="99888777000155")
    gravar("certificado_trocado.xml", re.sub(r"<X509Certificate>[^<]*</X509Certificate>",
                                             "<X509Certificate>" + base64.b64encode(der(outra)).decode()
                                             + "</X509Certificate>", base, count=1))
    # Nota adulterada e reassinada com certificado próprio (A-12)
    k3 = chave()
    proprio = emitir_folha("EMITENTE DE TESTE", k3, None, None, autoassinado=True)
    adulterado = MODELO.replace("<qCom>", "<qCom>9", 1)
    gravar("adulterada_reassinada_propria.xml", assinar(adulterado, k3, proprio))
    print("ok:", len(os.listdir(SAIDA)), "notas de teste em", SAIDA)


if __name__ == "__main__":
    sys.exit(main())
