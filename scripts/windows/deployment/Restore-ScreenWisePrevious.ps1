# Owner-run rollback. No acquisition, firewall change, data migration, or forced kill.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PreviousRuntime,
    [Parameter(Mandatory)][string]$ManifestBackup,
    [switch]$PlanOnly
)
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..\..'))
$deployment = Join-Path $repo '.local\deployment'
$installed = Join-Path $deployment 'release'
$manifestPath = Join-Path $deployment 'deployment.json'
Import-Module (Join-Path $PSScriptRoot 'Deployment.psm1') -Force
$privateRoot = Join-Path $repo '.local'
$installed = Assert-DeploymentPath -Path $installed -PrivateRoot $privateRoot
$archive = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $PreviousRuntime).Path)
$archive = Assert-DeploymentPath -Path $archive -PrivateRoot $privateRoot
$ManifestBackup = Assert-DeploymentPath -Path $ManifestBackup -PrivateRoot $privateRoot
$expectedParent = [IO.Path]::GetFullPath($deployment).TrimEnd('\')
if ([IO.Path]::GetDirectoryName($archive) -ine $expectedParent -or
    [IO.Path]::GetFileName($archive) -notmatch '^release-previous-[0-9a-f]{32}$') {
    throw 'Select the exact retained release-previous archive printed by deployment.'
}
$backup = Get-Content -Raw -LiteralPath $ManifestBackup | ConvertFrom-Json
$current = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
if ($backup.binary_directory -ine $installed -or $current.binary_directory -ine $installed -or
    $backup.data_directory -ine $current.data_directory -or $backup.port -ne $current.port) {
    throw 'Rollback manifest must belong to this exact installation, data directory and port.'
}
foreach($entry in @($backup,$current)) {
    if ($entry.gui_executable -ine (Join-Path $installed 'screenpipe-app.exe') -or
        $entry.cli_executable -ine (Join-Path $installed 'screenpipe.exe')) {
        throw 'Manifest executable paths must match the established installation.'
    }
}
$oldGui = Join-Path $archive 'screenpipe-app.exe'
$oldCli = Join-Path $archive 'screenpipe.exe'
if ((Get-FileHash -LiteralPath $oldGui -Algorithm SHA256).Hash -ine $backup.gui_sha256) {
    throw 'The selected archive does not match the baseline GUI hash.'
}
$baselineCli = $backup.PSObject.Properties['rollback_cli_sha256']
if (!$baselineCli -or (Get-FileHash -LiteralPath $oldCli -Algorithm SHA256).Hash -ine $baselineCli.Value) {
    throw 'The selected archive does not match the baseline recorder hash.'
}
$copies = @(Get-ChildItem -LiteralPath $archive -File -Recurse | ForEach-Object {
    if ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Archive reparse points are unsupported.' }
    $relative = $_.FullName.Substring($archive.Length).TrimStart('\')
    $target = [IO.Path]::GetFullPath((Join-Path $installed $relative))
    if (!$target.StartsWith($installed+'\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Archive target escaped the installation.' }
    $source = Assert-DeploymentPath -Path $_.FullName -PrivateRoot $privateRoot
    $target = Assert-DeploymentPath -Path $target -PrivateRoot $privateRoot
    [pscustomobject]@{Source=$source; Target=$target}
})
if ($PlanOnly) {
    [pscustomobject]@{copy_count=$copies.Count;data_untouched=$true;firewall_untouched=$true;graceful_shutdown_required=$true;automatic_start=$false} | ConvertTo-Json -Compress
    return
}
Stop-DeploymentInstance -Manifest $current
if (@(Get-DeploymentProcesses $installed).Count) { throw 'Owned runtime processes remain; no files restored.' }
foreach ($copy in $copies) {
    New-Item -ItemType Directory -Path (Split-Path -Parent $copy.Target) -Force | Out-Null
    Copy-Item -LiteralPath $copy.Source -Destination $copy.Target
}
# Third-party binaries remain at their established installed paths. Do not
# reacquire them or silently accept a mismatch against the original manifest.
Assert-DeploymentFiles $backup
$backup.PSObject.Properties.Remove('rollback_cli_sha256')
$backup | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $manifestPath -Encoding UTF8
Write-Output 'Previous runtime restored. Run Start-ScreenWise.ps1 -PreflightOnly before an owner-authorized restart.'
