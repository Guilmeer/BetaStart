# Auto-update assinado

O Beta Start usa o `tauri-plugin-updater` com assinatura **minisign**. O app
embute uma chave pública; cada instalador publicado é assinado com a chave
privada correspondente. Se a assinatura não bater, o app recusa a atualização.

## 1. Chaves

As chaves já foram geradas nesta máquina em:

```
C:\Users\kotem\.betastart-keys\
├── betastart.key       # PRIVADA — nunca commitar
├── betastart.key.pub   # pública — já embutida em tauri.conf.json
└── password.txt        # senha da chave privada — guarde num cofre
```

> **Importante:** se você perder `betastart.key` ou a senha, não será mais
> possível publicar atualizações para quem já instalou o app. Guarde as duas
> coisas num gerenciador de senhas/cofre e apague a pasta quando confirmar que
> os Secrets do GitHub estão funcionando.

Se o Windows negar acesso à chave (por causa da ACL restrita), corrija com:

```powershell
icacls "$env:USERPROFILE\.betastart-keys" /grant:r "$(whoami):(OI)(CI)F" /T
```

A chave pública em `src-tauri/tauri.conf.json` é a base64 do conteúdo de
`betastart.key.pub`. Para conferir:

```powershell
[Text.Encoding]::UTF8.GetString(
  [Convert]::FromBase64String(
    (Get-Content .\src-tauri\tauri.conf.json -Raw | ConvertFrom-Json).plugins.updater.pubkey
  )
)
```

Deve aparecer o comentário `untrusted comment: minisign public key: ...` e a
linha `RW...`.

## 2. GitHub

O auto-update usa `releases/latest/download/latest.json`, então o repositório
precisa ser **público** e as releases precisam ser **publicadas** (não draft).

1. Crie o repositório no GitHub e faça o push do projeto.
2. Adicione os dois Secrets em **Settings → Secrets and variables → Actions**:
   - `TAURI_SIGNING_PRIVATE_KEY`: conteúdo completo de `betastart.key`
     (incluindo a linha `untrusted comment: ...`).
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: conteúdo de `password.txt`.
3. O workflow [`.github/workflows/release.yml`](../.github/workflows/release.yml)
   troca o endpoint placeholder pelo repositório real automaticamente.

Se o endpoint no `tauri.conf.json` for usado em builds locais, troque:

```json
"endpoints": [
  "https://github.com/Guilmeer/BetaStart/releases/latest/download/latest.json"
]
```

## 3. Publicar uma versão

O updater compara a `version` do app com a versão do `latest.json`. Então:

1. Suba a versão nos três lugares (ou pelo menos no `tauri.conf.json`):
   - `package.json`
   - `src-tauri/Cargo.toml`
   - `src-tauri/tauri.conf.json`
2. Commit.
3. Crie a tag:

```bash
git tag v0.1.1
git push origin v0.1.1
```

O GitHub Actions vai:

- compilar;
- assinar o instalador com `TAURI_SIGNING_PRIVATE_KEY`;
- gerar `latest.json` com URL e assinatura;
- publicar a release (não draft).

> O endpoint `releases/latest/download/latest.json` só enxerga releases
> **publicadas** e que **não sejam prerelease**. Se mudar `releaseDraft` para
> `true` para revisar antes, publique a release manualmente depois; enquanto ela
> estiver em draft, o auto-update não a encontra.

## 4. Build local assinado

O `.env` não é lido pelo Tauri. Exporte as variáveis no PowerShell:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$env:USERPROFILE\.betastart-keys\betastart.key" -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = Get-Content "$env:USERPROFILE\.betastart-keys\password.txt" -Raw
npm run tauri build -- --bundles nsis
```

Isso gera, em `src-tauri/target/release/bundle/nsis/`:

- `Beta Start_<versão>_x64-setup.exe`
- `Beta Start_<versão>_x64-setup.exe.sig`

Se `createUpdaterArtifacts` estiver ligado e as variáveis não estiverem
definidas, o build falha — é o comportamento seguro.

## 5. Teste local sem GitHub

Para testar o fluxo sem publicar:

1. Configure `dangerousInsecureTransportProtocol: true` no updater.
2. Sirva um `latest.json` em `http://localhost:PORTA/latest.json`.
3. Use o endpoint `http://localhost:PORTA/latest.json` em `tauri.conf.json`.
4. O `latest.json` precisa apontar para um instalador com `.sig` válido.

Depois volte para HTTPS e remova a opção insegura.

## 6. Como o usuário vê

Em **Configurações → Atualizações** há:

- versão instalada;
- **Verificar atualizações**;
- notas da nova versão;
- **Baixar e instalar**, com progresso;
- reinício automático (no Windows o instalador fecha o app durante a instalação).

Se o endpoint ainda for o placeholder, a verificação mostra um erro de rede;
basta configurar o repositório real ou usar o workflow de release.
