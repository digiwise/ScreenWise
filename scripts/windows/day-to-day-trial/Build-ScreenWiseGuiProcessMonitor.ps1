#Requires -Version 5.1

[CmdletBinding()]
param(
    [string]$Compiler = (Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'),
    [string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..\..')).Path
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $repoRoot '.local\gui-trial-runtime\tools'
}
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
$source = Join-Path $PSScriptRoot 'GuiProcessTreeMonitor.cs'
$resourceSource = Join-Path $PSScriptRoot 'GuiProcessResources.cs'
$output = Join-Path $OutputDirectory 'GuiProcessTreeMonitor.exe'
$manifest = Join-Path $OutputDirectory 'gui-process-tree-monitor.json'

if (-not [IO.Path]::IsPathRooted($Compiler) -or -not (Test-Path -LiteralPath $Compiler -PathType Leaf)) {
    throw "Framework C# compiler not found: $Compiler"
}
[IO.Directory]::CreateDirectory($OutputDirectory) | Out-Null
& $Compiler /nologo /target:exe /optimize+ /out:$output $source $resourceSource
if ($LASTEXITCODE -ne 0) { throw 'GUI process monitor compilation failed' }
& $output --self-test
if ($LASTEXITCODE -ne 0) { throw "GUI process monitor self-test failed with exit code $LASTEXITCODE" }
& $output --resource-self-test
if ($LASTEXITCODE -ne 0) { throw 'GUI resource sampler self-test failed' }

$testRoot = Join-Path $OutputDirectory ('screenwise-gui-process-monitor-' + [Guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($testRoot) | Out-Null
try {
    $quotedTestRoot = '"{0}"' -f $testRoot
    $monitor = Start-Process -FilePath $output -ArgumentList @('--root-pid', "$PID", '--out-dir', $quotedTestRoot, '--duration-ms', '1800') -WindowStyle Hidden -PassThru -RedirectStandardError (Join-Path $testRoot 'monitor-errors.txt')
    $null = $monitor.Handle # Windows PowerShell 5.1 otherwise reports a null ExitCode.
    Start-Sleep -Milliseconds 250
    $child = Start-Process -FilePath $env:ComSpec -ArgumentList @('/c', 'ping -n 2 127.0.0.1 >nul') -WindowStyle Hidden -Wait -PassThru
    if (-not $monitor.WaitForExit(5000)) { try { $monitor.Kill() } catch {}; throw 'GUI process monitor integration self-test timed out' }
    if ($monitor.ExitCode -ne 0) {
        Get-Content -LiteralPath (Join-Path $testRoot 'monitor-errors.txt') | Write-Host
        throw "GUI process monitor integration self-test failed with exit code $($monitor.ExitCode)"
    }
    $summary = Get-Content -Raw -LiteralPath (Join-Path $testRoot 'process-tree-summary.json') | ConvertFrom-Json
    if (-not [bool]$summary.sampler_available -or -not [bool]$summary.root_observed -or
        [int]$summary.unexpected_shell_process_start_count -lt 1) {
        throw 'GUI process monitor did not observe the controlled cmd.exe descendant'
    }
    & $output --resources --root-pid $PID --out-dir $testRoot --duration-ms 2200 --interval-ms 1000
    if ($LASTEXITCODE -ne 0) { throw 'Resource sampler integration failed' }
    $samples = @(Get-Content -LiteralPath (Join-Path $testRoot 'process-resources.jsonl') | ForEach-Object { $_ | ConvertFrom-Json })
    if ($samples.Count -lt 3 -or $null -ne $samples[0].cpu_cores_sampled -or
        $null -eq $samples[1].cpu_cores_sampled -or $samples[1].working_set_bytes_sum -le 0 -or
        $samples[1].private_bytes_sum -le 0 -or -not $samples[1].root_present -or
        $samples[1].logical_processors -lt 1 -or $samples[1].truncated) {
        throw 'Resource sampler counters or first-sample semantics failed'
    }
    $resourceLength = (Get-Item -LiteralPath (Join-Path $testRoot 'process-resources.jsonl')).Length
    & $output --resources --root-pid $PID --out-dir $testRoot --duration-ms 1000
    if ($LASTEXITCODE -eq 0 -or (Get-Item -LiteralPath (Join-Path $testRoot 'process-resources.jsonl')).Length -ne $resourceLength) {
        throw 'Resource sampler failed evidence overwrite protection'
    }
    $global:LASTEXITCODE = 0 # The expected refusal above is a successful regression check.
}
finally {
    if ($monitor) { $monitor.Dispose() }
    $resolved = [IO.Path]::GetFullPath($testRoot)
    $tempRoot = [IO.Path]::GetFullPath($OutputDirectory).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    if ($resolved.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -and
        ([IO.Path]::GetFileName($resolved)).StartsWith('screenwise-gui-process-monitor-', [StringComparison]::Ordinal)) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}

[ordered]@{
    schema = 'screenwise.gui-process-tree-runtime.v1'
    source_path = $source
    source_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $source).Hash
    resource_source_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $resourceSource).Hash
    executable_path = $output
    executable_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $output).Hash
    integration_self_test = 'passed'
} | ConvertTo-Json | Set-Content -LiteralPath $manifest -Encoding utf8
Get-Content -Raw -LiteralPath $manifest
