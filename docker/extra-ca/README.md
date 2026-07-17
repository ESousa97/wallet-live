# CAs extras para o build da imagem (opcional)

Se a sua máquina está atrás de um proxy corporativo ou de um antivírus com
inspeção TLS (Norton, ZScaler etc.), o tráfego HTTPS é reassinado por um CA
próprio — e o `cargo` dentro do container falha ao baixar dependências com
`unable to get local issuer certificate`.

Coloque aqui o certificado raiz do interceptador, em **PEM com extensão
`.crt`**, e o build passa a confiá-lo (`update-ca-certificates`).

Os `.crt`/`.cer` deste diretório são ignorados pelo git de propósito: são
específicos de cada máquina e não devem ir para o repositório.

Exemplo (Windows, PowerShell + Git Bash):

```powershell
$cert = Get-ChildItem Cert:\CurrentUser\Root, Cert:\LocalMachine\Root |
    Where-Object { $_.Subject -match 'NomeDoInterceptador' } | Select-Object -First 1
Export-Certificate -Cert $cert -FilePath docker\extra-ca\interceptador.cer -Type CERT
```

```bash
openssl x509 -inform der -in docker/extra-ca/interceptador.cer \
    -out docker/extra-ca/interceptador.crt
rm docker/extra-ca/interceptador.cer
```
