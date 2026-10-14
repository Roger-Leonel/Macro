# Roda o Korvon Macro. Acrescenta o Cargo ao PATH desta sessão se o terminal ainda não o enxerga.
# Uso:  .\run.ps1            (desenvolvimento)
#       .\run.ps1 -Release   (otimizado, sem janela de console)
param([switch]$Release)

$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    if (Test-Path (Join-Path $cargoBin "cargo.exe")) {
        $env:PATH += ";$cargoBin"
    } else {
        Write-Error "Rust não encontrado. Instale em https://rustup.rs e abra um terminal novo."
        exit 1
    }
}

Set-Location $PSScriptRoot
if ($Release) { cargo run --release -p korvon-app } else { cargo run -p korvon-app }
