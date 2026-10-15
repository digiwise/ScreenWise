param(
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$Compiler = (Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe')
)
$ErrorActionPreference = 'Stop'
if (-not [IO.Path]::IsPathFullyQualified($OutputDirectory)) { throw 'absolute_output_required' }
if (-not (Test-Path -LiteralPath $Compiler -PathType Leaf)) { throw 'compiler_missing' }
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$executable = Join-Path $OutputDirectory 'OverlayFixture.exe'
& $Compiler /nologo /target:winexe /platform:x64 /optimize+ /out:$executable /reference:System.dll /reference:System.Drawing.dll /reference:System.Windows.Forms.dll (Join-Path $PSScriptRoot 'OverlayFixture.cs')
if ($LASTEXITCODE -ne 0) { throw 'fixture_compile_failed' }
$selfTestProcess = Start-Process -FilePath $executable -ArgumentList '--self-test' -WindowStyle Hidden -Wait -PassThru
if ($selfTestProcess.ExitCode -ne 0) { throw 'fixture_self_test_failed' }
Write-Output 'SELF_TEST_OK overlay fixture (no UI)'
Get-FileHash -LiteralPath $executable -Algorithm SHA256 | Select-Object Hash
