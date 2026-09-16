[CmdletBinding()]
param([string]$InputDirectory = $PSScriptRoot)

$ErrorActionPreference = 'Stop'
$manifestPath = Join-Path $InputDirectory 'manifest.json'
if (-not (Test-Path -LiteralPath $manifestPath)) { throw "Missing manifest: $manifestPath" }
$manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
$failures = @()
foreach ($item in $manifest.files) {
    $path = Join-Path $InputDirectory $item.file
    if (-not (Test-Path -LiteralPath $path)) { $failures += "missing $($item.file)"; continue }
    $bytes = [System.IO.File]::ReadAllBytes($path)
    $sampleRate = [BitConverter]::ToInt32($bytes, 24)
    $channels = [BitConverter]::ToInt16($bytes, 22)
    $bits = [BitConverter]::ToInt16($bytes, 34)
    $dataOffset = 0
    for ($i = 12; $i -le $bytes.Length - 8; $i++) {
        if ($bytes[$i] -eq 0x64 -and $bytes[$i + 1] -eq 0x61 -and $bytes[$i + 2] -eq 0x74 -and $bytes[$i + 3] -eq 0x61) { $dataOffset = $i; break }
    }
    if ($dataOffset -eq 0) { $failures += "no data chunk $($item.file)"; continue }
    $dataBytes = [BitConverter]::ToInt32($bytes, $dataOffset + 4)
    $duration = [math]::Round($dataBytes / ($sampleRate * $channels * ($bits / 8)), 3)
    $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
    if ($hash -ne $item.sha256) { $failures += "hash mismatch $($item.file)" }
    if ($duration -ne [double]$item.durationSeconds) { $failures += "duration mismatch $($item.file)" }
    if ($sampleRate -ne $item.format.sampleRateHz -or $channels -ne $item.format.channels -or $bits -ne $item.format.bitsPerSample) { $failures += "format mismatch $($item.file)" }
    if ([string]::IsNullOrWhiteSpace($item.passage)) { $failures += "missing passage text $($item.file)" }
}
if ($failures.Count -gt 0) { throw ($failures -join '; ') }
Write-Output "Validated $($manifest.files.Count) WAV clips: PCM headers, durations, formats, passages, and SHA-256 hashes match manifest."
