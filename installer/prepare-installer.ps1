<#
  Monta dist\installer\ com tudo que o instalador (Inno Setup) deve empacotar.
  Uso: pwsh installer\prepare-installer.ps1 [-BuildDir target\release] [-OutDir dist\installer]
  Falha se algum arquivo obrigatório estiver ausente.
#>
param(
    [string]$BuildDir = "target\release",
    [string]$OutDir = "dist\installer"
)
$ErrorActionPreference = "Stop"
Set-Location (Split-Path -Parent $PSScriptRoot)

$exe = Join-Path $BuildDir "korvon-macro.exe"
if (-not (Test-Path $exe)) { throw "Executável não encontrado: $exe (rode 'cargo build --release --locked -p korvon-app')" }

if (Test-Path $OutDir) { Remove-Item $OutDir -Recurse -Force }
New-Item -ItemType Directory -Force $OutDir | Out-Null

# Obrigatório: o app é um único executável estático (sem DLLs/assets externos; ícones são embutidos).
Copy-Item $exe $OutDir

# Opcionais: copiados se existirem no repositório.
foreach ($f in "LICENSE", "LICENSE.txt", "README.md", "docs\analise-mrf.md") {
    if (Test-Path $f) { Copy-Item $f $OutDir }
}
# Exemplos/recursos extras: coloque em installer\extras\ e serão copiados para dist\installer\extras\.
if (Test-Path "installer\extras") { Copy-Item "installer\extras" (Join-Path $OutDir "extras") -Recurse }

Write-Host "Conteúdo de ${OutDir}:"
Get-ChildItem $OutDir -Recurse | ForEach-Object { Write-Host ("  " + $_.FullName.Substring((Resolve-Path $OutDir).Path.Length)) }
