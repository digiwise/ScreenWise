param(
    [Parameter(Mandatory=$true)][string]$RunId,
    [string]$OwnerReady,
    [string]$ConfigPath,
    [string]$PythonExe,
    [switch]$ExecuteInteractive
)
$ErrorActionPreference='Stop'
$helper = $null
foreach ($candidate in @(
    (Join-Path $PSScriptRoot '..\..\common\Invoke-ValidationController.ps1'),
    (Join-Path $PSScriptRoot '..\..\interactive-validation\common\Invoke-ValidationController.ps1')
)) {
    if (Test-Path -LiteralPath $candidate -PathType Leaf) { $helper = (Resolve-Path -LiteralPath $candidate).Path; break }
}
if (-not $helper) { throw 'Portable validation launcher helper was not found.' }
& $helper -Controller "$PSScriptRoot\tail_collector_60s.py" -RunId $RunId -OwnerReady $OwnerReady `
    -ConfigPath $ConfigPath -PythonExe $PythonExe -ExecuteInteractive:$ExecuteInteractive
