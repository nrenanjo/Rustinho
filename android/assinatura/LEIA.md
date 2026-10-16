# Assinatura do APK

O Android só aceita atualizar um app se a versão nova tiver a mesma assinatura da anterior.
Esta chave (`rustinho.jks`, senha `rustinho`) é **pública**, para a montagem funcionar sem configurar nada.

Para ter uma chave só sua (recomendado antes de usar na loja):

1. Gere uma chave: `keytool -genkeypair -keystore minha.jks -alias rustinho -keyalg RSA -keysize 2048 -validity 10000`
2. No GitHub do projeto: Settings → Secrets and variables → Actions → New repository secret:
   - `ANDROID_CHAVE_BASE64`: o conteúdo de `base64 -w0 minha.jks`
   - `ANDROID_CHAVE_SENHA`: a senha que você escolheu
3. Guarde o `minha.jks` e a senha fora do computador. Sem eles não dá para atualizar o app.

Trocar a chave depois exige desinstalar o app antigo (faça um backup antes).
