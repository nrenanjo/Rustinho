# Assinatura do APK

O Android só aceita atualizar um app se a versão nova tiver a mesma assinatura da anterior.
Desde a 0.9.11, o Rustinho não tem mais chave pública aqui: a chave é só da loja e fica nos
segredos do GitHub (ninguém mais consegue fazer um APK que se passe pelo Rustinho da loja).

Sem os segredos, a montagem ainda funciona, mas assina o APK com uma chave de uso único, só
para teste. Esse APK **nunca vai para Releases** e a publicação avisa o que falta.

## Cadastrar a chave da loja

No GitHub do projeto: **Settings → Secrets and variables → Actions → New repository secret**:

- `ANDROID_CHAVE_BASE64`: o texto do arquivo `ANDROID_CHAVE_BASE64.txt` (a chave em base64)
- `ANDROID_CHAVE_SENHA`: a senha da chave

Depois de cadastrar, guarde também a **impressão digital** do certificado (não é segredo), para
a montagem nunca publicar um APK de outra chave. Ela aparece com:

    keytool -list -v -keystore rustinho-loja.jks -alias rustinho | grep SHA256

Em **Settings → Secrets and variables → Actions → Variables → New repository variable**:

- `ANDROID_CHAVE_IMPRESSAO`: o que vem depois de `SHA256:` (com ou sem os dois-pontos)

A montagem confere em cada APK quem assinou (também o que já está publicado na versão) e
recusa a chave antiga, que era pública (`F0:07:29:D0:...:22:3A`, usada até a 0.9.1).

Para gerar uma chave nova, se um dia precisar:

    keytool -genkeypair -keystore rustinho-loja.jks -storetype JKS -alias rustinho -keyalg RSA -keysize 4096 -validity 12000
    base64 -w0 rustinho-loja.jks > ANDROID_CHAVE_BASE64.txt

## Importante

- Guarde o arquivo `rustinho-loja.jks` e a senha fora do computador (pen drive e papel, por
  exemplo). Sem eles, não dá para atualizar o app por cima: cada celular teria que
  desinstalar e instalar de novo.
- Trocar a chave obriga desinstalar o app antigo no celular (faça o backup antes).
- Nunca coloque o arquivo `.jks` nem a senha dentro do projeto.
