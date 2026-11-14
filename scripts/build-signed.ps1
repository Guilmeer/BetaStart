# Gera o instalador NSIS assinado para o updater.
# Uso: powershell -ExecutionPolicy Bypass -File scripts\build-signed.ps1 [-Repo usuario/repo]
param(
    [string]$Repo = ""
)
$ErrorActionPreference = "Stop"

if ($Repo) {
    $configPath = Join-Path $PSScriptRoot "..\src-tauri\tauri.conf.json"
    $json = Get-Content $configPath -Raw | ConvertFrom-Json
    $json.plugins.updater.endpoints = @(
        "https://github.com/$Repo/releases/latest/download/latest.json"
    )
    $content = $json | ConvertTo-Json -Depth 100
    $resolved = (Resolve-Path $configPath).Path
    [IO.File]::WriteAllText(
        $resolved,
        $content,
        [Text.UTF8Encoding]::new($false)
    )
    Write-Host "Endpoint do updater: $Repo" -ForegroundColor DarkGray
}

$keyDir = Join-Path $env:USERPROFILE ".betastart-keys"
$keyPath = Join-Path $keyDir "betastart.key"
$passwordPath = Join-Path $keyDir "password.txt"

if (-not (Test-Path $keyPath)) {
    throw "Chave privada não encontrada em $keyPath. Veja docs/AUTO_UPDATE.md."
}

$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $keyPath -Raw
if (Test-Path $passwordPath) {
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = Get-Content $passwordPath -Raw
}

Write-Host "Gerando build assinado..." -ForegroundColor Cyan
npm run tauri build -- --bundles nsis
