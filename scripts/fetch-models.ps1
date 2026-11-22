[CmdletBinding()]
param(
    [string] $Destination = (Join-Path (Resolve-Path (Join-Path $PSScriptRoot '..')).Path 'models')
)

$ErrorActionPreference = 'Stop'

$Base = 'https://raw.githubusercontent.com/Rikorose/DeepFilterNet/7b14b895fe7fc42cab72b4a4c1e49c982bd6404b/models'
New-Item -ItemType Directory -Force -Path $Destination | Out-Null

$Models = @(
    @{
        Name = 'DeepFilterNet3_ll_onnx.tar.gz'
        Sha256 = '5998e58e8ba0e09bb76986ef97b84afa065a571ef282d4a1222f341e3251cf3a'
    },
    @{
        Name = 'DeepFilterNet3_onnx.tar.gz'
        Sha256 = 'c94d91f70911001c946e0fabb4aa9adc37045f45a03b56008cb0c8244cb63616'
    }
)

foreach ($Model in $Models) {
    $Target = Join-Path $Destination $Model.Name

    if (Test-Path $Target) {
        $Actual = (Get-FileHash -Algorithm SHA256 -Path $Target).Hash.ToLowerInvariant()
        if ($Actual -eq $Model.Sha256) {
            Write-Host "ok: $($Model.Name) ($Actual)"
            continue
        }
        Write-Warning "hash mismatch, re-downloading: $($Model.Name)"
    }

    $Tmp = "$Target.part"
    if (Test-Path $Tmp) {
        Remove-Item -Force $Tmp
    }

    Write-Host "downloading: $($Model.Name)"
    Invoke-WebRequest -Uri "$Base/$($Model.Name)" -OutFile $Tmp

    $Actual = (Get-FileHash -Algorithm SHA256 -Path $Tmp).Hash.ToLowerInvariant()
    if ($Actual -ne $Model.Sha256) {
        Remove-Item -Force $Tmp
        throw "sha256 mismatch for $($Model.Name): expected $($Model.Sha256), got $Actual"
    }

    Move-Item -Force $Tmp $Target
    Write-Host "ok: $($Model.Name) ($Actual)"
}

Write-Host "models ready: $Destination"
