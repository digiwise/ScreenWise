[CmdletBinding()]
param(
    [string]$DataDir,
    [string]$ExecutablePath,
    [string]$FirewallGroup,
    [ValidateRange(1, 65535)]
    [int]$Port = 3030,
    [string[]]$IgnoredWindow = @(),
    [string[]]$IgnoredUrl = @(),
    [ValidateSet('parakeet', 'disabled')]
    [string]$TranscriptionEngine = 'parakeet',
    [switch]$PreflightOnly
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..\..')).Path
if ([string]::IsNullOrWhiteSpace($ExecutablePath)) {
    $ExecutablePath = Join-Path $repoRoot 'target\release\screenpipe.exe'
}
if ([string]::IsNullOrWhiteSpace($DataDir)) {
    $DataDir = Join-Path $repoRoot '.local\day-to-day-trial'
}
if ([string]::IsNullOrWhiteSpace($FirewallGroup)) {
    throw 'Supply the exact reviewed ScreenWise firewall group with -FirewallGroup.'
}

$ExecutablePath = (Resolve-Path -LiteralPath $ExecutablePath).Path
$releaseDir = Split-Path -Parent $ExecutablePath
$scopedPaths = @(
    $ExecutablePath,
    (Join-Path $releaseDir 'ffmpeg.exe'),
    (Join-Path $releaseDir 'ffprobe.exe')
)
foreach ($path in $scopedPaths) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Required executable is missing: $path"
    }
}

$running = @(
    Get-CimInstance Win32_Process |
        Where-Object {
            $_.ExecutablePath -and
            $scopedPaths -contains $_.ExecutablePath
        }
)
if ($running.Count -ne 0) {
    throw "Refusing to start while $($running.Count) scoped process(es) already exist."
}

$requiredRemote = @(
    '0.0.0.0-126.255.255.255',
    '128.0.0.0-255.255.255.255',
    '::',
    '::2-ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff'
)
$activeRules = @(Get-NetFirewallRule -PolicyStore ActiveStore -Group $FirewallGroup -ErrorAction Stop)
foreach ($path in $scopedPaths) {
    $matching = @(
        $activeRules | Where-Object {
            $filter = @($_ | Get-NetFirewallApplicationFilter)
            $filter.Count -eq 1 -and
            [StringComparer]::OrdinalIgnoreCase.Equals($filter[0].Program, $path)
        }
    )
    if ($matching.Count -ne 1) {
        throw "Expected exactly one active firewall rule for $path; found $($matching.Count)."
    }
    $rule = $matching[0]
    $addressFilter = @($rule | Get-NetFirewallAddressFilter)
    $actualRemote = @($addressFilter.RemoteAddress | Sort-Object -Unique)
    if ("$($rule.Direction)" -ne 'Outbound' -or
        "$($rule.Action)" -ne 'Block' -or
        "$($rule.Enabled)" -ne 'True' -or
        (Compare-Object $requiredRemote $actualRemote)) {
        throw "Firewall scope mismatch for $path."
    }
}

if (Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction SilentlyContinue) {
    throw "Port $Port is already listening."
}

if ($PreflightOnly) {
    [pscustomobject]@{
        Status = 'ready'
        ExecutablePath = $ExecutablePath
        ExecutableSha256 = (Get-FileHash -LiteralPath $ExecutablePath -Algorithm SHA256).Hash
        FirewallGroup = $FirewallGroup
        ScopedExecutableCount = $scopedPaths.Count
        Port = $Port
        RecordingStarted = $false
    } | ConvertTo-Json -Depth 3
    return
}

[IO.Directory]::CreateDirectory($DataDir) | Out-Null
$auditRoot = Join-Path $DataDir '.trial-audit'
[IO.Directory]::CreateDirectory($auditRoot) | Out-Null
$sessionName = '{0}-{1}' -f (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ'),
    ([Guid]::NewGuid().ToString('N').Substring(0, 8))
$sessionDir = Join-Path $auditRoot $sessionName
[IO.Directory]::CreateDirectory($sessionDir) | Out-Null

$launch = [ordered]@{
    schema = 'screenwise.day-to-day-trial-launch.v1'
    started_at_utc = (Get-Date).ToUniversalTime().ToString('o')
    executable_path = $ExecutablePath
    executable_sha256 = (Get-FileHash -LiteralPath $ExecutablePath -Algorithm SHA256).Hash
    data_dir = [IO.Path]::GetFullPath($DataDir)
    port = $Port
    firewall_group = $FirewallGroup
    keyboard_capture_requested = $true
    clipboard_capture_requested = $true
    system_default_audio_requested = $true
    transcription_engine = $TranscriptionEngine
    ignored_windows = @($IgnoredWindow)
    ignored_urls = @($IgnoredUrl)
}
$launch | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $sessionDir 'launch.json') -Encoding utf8

$monitorPath = Join-Path $PSScriptRoot 'Watch-ScreenWiseTrial.ps1'
$monitorJob = Start-Job -FilePath $monitorPath -ArgumentList @(
    $ExecutablePath,
    [IO.Path]::GetFullPath($DataDir),
    $sessionDir,
    $Port,
    $launch.started_at_utc
)

$recordArgs = @(
    'record',
    '--data-dir', [IO.Path]::GetFullPath($DataDir),
    '--port', "$Port",
    '--use-all-monitors',
    '--use-system-default-audio',
    '--audio-transcription-engine', $TranscriptionEngine,
    '--language', 'english',
    '--use-pii-removal',
    '--async-pii-redaction',
    '--pause-on-drm-content',
    '--enable-keyboard-capture',
    '--enable-clipboard-capture',
    '--capture-on-keystroke', 'true',
    '--capture-on-clipboard', 'true'
)
foreach ($pattern in $IgnoredWindow) {
    $recordArgs += @('--ignored-windows', $pattern)
}
foreach ($pattern in $IgnoredUrl) {
    $recordArgs += @('--ignored-urls', $pattern)
}

Write-Host "ScreenWise trial data: $DataDir"
Write-Host "Operational audit: $sessionDir"
Write-Host 'Press Ctrl+C once when you want ScreenWise to shut down cleanly.'

$exitCode = $null
try {
    & $ExecutablePath @recordArgs
    $exitCode = $LASTEXITCODE
}
finally {
    $ended = [ordered]@{
        ended_at_utc = (Get-Date).ToUniversalTime().ToString('o')
        recorder_exit_code = $exitCode
    }
    $ended | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $sessionDir 'launcher-exit.json') -Encoding utf8
    Wait-Job -Job $monitorJob -Timeout 90 | Out-Null
    Receive-Job -Job $monitorJob -ErrorAction SilentlyContinue | Out-Host
    Stop-Job -Job $monitorJob -ErrorAction SilentlyContinue
    Remove-Job -Job $monitorJob -Force -ErrorAction SilentlyContinue
}

exit $(if ($null -eq $exitCode) { 1 } else { $exitCode })
