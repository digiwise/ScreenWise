param([switch]$ExecuteInteractive, [switch]$Compare, [int]$ProcessId = 0, [ValidateRange(1,10)][int]$Passes = 3)
$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '../../..')).Path
$exe = Join-Path $repoRoot '.local/uia-runtimeid-probe/access-runtimeid-probe.exe'
if (-not $ExecuteInteractive) { Write-Host 'Preview only. Run with -ExecuteInteractive when Access is ready. No input, focus change or recording.'; return }
if (-not (Test-Path -LiteralPath $exe)) { throw 'Build the standalone probe first.' }
$targets = @(Get-Process MSACCESS -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 -and ($ProcessId -eq 0 -or $_.Id -eq $ProcessId) })
if ($targets.Count -ne 1) { throw 'Need exactly one Access window. Specify -ProcessId if several are open.' }
$target = $targets[0]
$hwnd = $target.MainWindowHandle.ToInt64()
$started = $target.StartTime.ToUniversalTime()
$runDir = Join-Path (Split-Path $exe -Parent) ('run-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $runDir | Out-Null
for ($pass = 1; $pass -le $Passes; $pass++) {
    $current = Get-Process -Id $target.Id -ErrorAction Stop
    if ($current.ProcessName -ne 'MSACCESS' -or $current.StartTime.ToUniversalTime() -ne $started -or $current.MainWindowHandle.ToInt64() -ne $hwnd) { throw 'Target changed; stop and start a fresh probe run.' }
    $out = Join-Path $runDir "pass-$pass.jsonl"
    $err = Join-Path $runDir "pass-$pass.err"
    $probeArgs = @([string]$hwnd)
    if ($Compare) { $probeArgs += @('--compare', [string]$pass) }
    $child = Start-Process -FilePath $exe -ArgumentList $probeArgs -PassThru -WindowStyle Hidden -RedirectStandardOutput $out -RedirectStandardError $err
    $null = $child.Handle # Windows PowerShell 5.1 otherwise reports a null ExitCode.
    $watchdogMs = if ($Compare) { 30000 } else { 15000 }
    if (-not $child.WaitForExit($watchdogMs)) {
        Stop-Process -Id $child.Id -Force -ErrorAction SilentlyContinue
        Write-Output ('{"pass":' + $pass + ',"watchdog_timeout":1}')
        break
    }
    Write-Output ('{"pass":' + $pass + ',"exit_code":' + $child.ExitCode + '}')
    Get-Content -LiteralPath $out
}
Write-Host "Numeric results: $runDir"
