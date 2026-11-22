[CmdletBinding()]
param(
    [ValidateSet('Debug', 'Release')]
    [string] $Configuration = 'Release',

    [string] $Target = 'x86_64-pc-windows-msvc',

    [switch] $SkipBuild
)

$ErrorActionPreference = 'Stop'

$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$ModuleName = 'obs-plugin_noise-cancelling'
$StageRoot = Join-Path $Root 'dist/windows'
$StageDir = Join-Path $StageRoot $ModuleName
$ZipPath = Join-Path $Root 'dist/obs-plugin_noise-cancelling-windows-x64.zip'

$ProfileDir = if ($Configuration -eq 'Debug') { 'debug' } else { 'release' }
$CargoProfile = if ($Configuration -eq 'Debug') { 'dev' } else { 'release' }

if (-not $SkipBuild) {
    Push-Location $Root
    try {
        & cargo build -p obs-filter --profile $CargoProfile --target $Target
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build failed with exit code $LASTEXITCODE"
        }
    } finally {
        Pop-Location
    }
}

$Dll = Join-Path $Root "target/$Target/$ProfileDir/obs_filter.dll"
if (-not (Test-Path $Dll)) {
    throw "DLL not found: $Dll"
}

$ModelsDir = Join-Path $Root 'models'
$RequiredModels = @('DeepFilterNet3_ll_onnx.tar.gz', 'DeepFilterNet3_onnx.tar.gz')
$MissingModels = $RequiredModels | Where-Object { -not (Test-Path (Join-Path $ModelsDir $_)) }
if ($MissingModels.Count -gt 0) {
    & (Join-Path $Root 'scripts/fetch-models.ps1') -Destination $ModelsDir
    if ($LASTEXITCODE -ne 0) {
        throw "model download failed with exit code $LASTEXITCODE"
    }
}

if (Test-Path $StageRoot) {
    Remove-Item -Recurse -Force $StageRoot
}
New-Item -ItemType Directory -Force -Path (Join-Path $StageDir 'data/presets') | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $StageDir 'data/models') | Out-Null

Copy-Item $Dll (Join-Path $StageDir "$ModuleName.dll")
Copy-Item (Join-Path $Root 'presets/*.toml') (Join-Path $StageDir 'data/presets/')
Copy-Item (Join-Path $ModelsDir '*.tar.gz') (Join-Path $StageDir 'data/models/')

if (Test-Path $ZipPath) {
    Remove-Item -Force $ZipPath
}

Compress-Archive -Path $StageDir -DestinationPath $ZipPath -CompressionLevel Optimal

Write-Host "created $ZipPath"
