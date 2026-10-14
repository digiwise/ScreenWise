param([Parameter(Mandatory)][string]$WindowsRlib, [Parameter(Mandatory)][string]$WindowsNativeLib)
$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '../../..')).Path
$config = Get-Content (Join-Path $repoRoot '.local/build/config.local.json') -Raw | ConvertFrom-Json
$library = (Resolve-Path -LiteralPath $WindowsRlib).Path
$deps = Split-Path $library -Parent
$nativeDir = Split-Path (Resolve-Path -LiteralPath $WindowsNativeLib).Path -Parent
$outputDir = Join-Path $repoRoot '.local/uia-runtimeid-probe'
New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
& $config.developer_shell -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
& rustc --edition=2021 -C opt-level=1 (Join-Path $PSScriptRoot 'probe.rs') --extern "windows=$library" -L "dependency=$deps" -L "native=$nativeDir" -o (Join-Path $outputDir 'access-runtimeid-probe.exe')
if ($LASTEXITCODE -ne 0) { throw 'Probe compilation failed.' }
& (Join-Path $outputDir 'access-runtimeid-probe.exe') --self-test
if ($LASTEXITCODE -ne 0) { throw 'Probe self-test failed.' }
