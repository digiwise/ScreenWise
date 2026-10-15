# Synthetic filesystem/PlanOnly checks; never touches the real installation.
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..\..'))
$root = Join-Path $repo ('.local\uia-bulk-validation\rollback-tests-'+[Guid]::NewGuid().ToString('N'))
$scripts = Join-Path $root 'scripts\windows\deployment'
$private = Join-Path $root '.local'
$deployment = Join-Path $private 'deployment'
$installed = Join-Path $deployment 'release'
$archive = Join-Path $deployment ('release-previous-'+[Guid]::NewGuid().ToString('N'))
foreach($dir in @($scripts,$installed,$archive)) {New-Item -ItemType Directory -Path $dir -Force | Out-Null}
foreach($file in @('Restore-ScreenWisePrevious.ps1','Deployment.psm1')) {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot $file) -Destination (Join-Path $scripts $file)
}
foreach($file in @('screenpipe-app.exe','screenpipe.exe')) {
    New-Item -ItemType File -Path (Join-Path $archive $file) | Out-Null
    New-Item -ItemType File -Path (Join-Path $installed $file) | Out-Null
}
$baseline = [pscustomobject]@{binary_directory=$installed;data_directory=(Join-Path $private 'data');port=31579;
    gui_executable=(Join-Path $installed 'screenpipe-app.exe');cli_executable=(Join-Path $installed 'screenpipe.exe');
    gui_sha256=(Get-FileHash (Join-Path $archive 'screenpipe-app.exe')).Hash;
    rollback_cli_sha256=(Get-FileHash (Join-Path $archive 'screenpipe.exe')).Hash}
$backup = Join-Path $private 'previous.json'
$manifest = Join-Path $deployment 'deployment.json'
$baseline | ConvertTo-Json | Set-Content -LiteralPath $backup -Encoding UTF8
$baseline | ConvertTo-Json | Set-Content -LiteralPath $manifest -Encoding UTF8
$restore = Join-Path $scripts 'Restore-ScreenWisePrevious.ps1'
$before = (Get-FileHash -LiteralPath $manifest).Hash
$plan = & $restore -PreviousRuntime $archive -ManifestBackup $backup -PlanOnly | ConvertFrom-Json
if ($plan.copy_count -ne 2 -or !$plan.data_untouched -or $plan.automatic_start) {throw 'Valid rollback plan failed'}
if ((Get-FileHash -LiteralPath $manifest).Hash -ne $before) {throw 'Plan modified the manifest'}
$passes = 1
function Expect-Rejected([scriptblock]$Action) {
    $rejected=$false
    try {& $Action | Out-Null} catch {$rejected=$true}
    if (!$rejected) {throw 'Unsafe rollback plan was accepted'}
    $script:passes++
}
$baseline.rollback_cli_sha256 = ('0'*64)
$baseline | ConvertTo-Json | Set-Content -LiteralPath $backup -Encoding UTF8
Expect-Rejected {& $restore -PreviousRuntime $archive -ManifestBackup $backup -PlanOnly}
$baseline.rollback_cli_sha256 = (Get-FileHash (Join-Path $archive 'screenpipe.exe')).Hash
$baseline.cli_executable = Join-Path $private 'elsewhere.exe'
$baseline | ConvertTo-Json | Set-Content -LiteralPath $backup -Encoding UTF8
Expect-Rejected {& $restore -PreviousRuntime $archive -ManifestBackup $backup -PlanOnly}
$baseline.cli_executable = Join-Path $installed 'screenpipe.exe'
$baseline | ConvertTo-Json | Set-Content -LiteralPath $backup -Encoding UTF8
$linkedArchive = Join-Path $deployment ('release-previous-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Junction -Path $linkedArchive -Target $archive | Out-Null
Expect-Rejected {& $restore -PreviousRuntime $linkedArchive -ManifestBackup $backup -PlanOnly}
[pscustomobject]@{rollback_plan_tests_passed=$passes;real_installation_touched=$false} | ConvertTo-Json -Compress
