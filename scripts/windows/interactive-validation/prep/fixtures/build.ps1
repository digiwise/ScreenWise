param(
    [switch]$SelfTest,
    [string]$Compiler = (Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe')
)
$ErrorActionPreference = 'Stop'
$fixtureRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not [IO.Path]::IsPathFullyQualified($Compiler) -or -not (Test-Path -LiteralPath $Compiler -PathType Leaf)) {
    throw "Framework C# compiler not found: $Compiler"
}

& $Compiler /nologo /target:winexe /optimize+ /out:"$fixtureRoot\PrivacyFixture.exe" /reference:System.dll /reference:System.Drawing.dll /reference:System.Windows.Forms.dll "$fixtureRoot\PrivacyFixture.cs"
if ($LASTEXITCODE -ne 0) { throw 'PrivacyFixture compilation failed' }
& $Compiler /nologo /target:winexe /optimize+ /out:"$fixtureRoot\Netflix.exe" /reference:System.dll /reference:System.Drawing.dll /reference:System.Windows.Forms.dll "$fixtureRoot\DrmFixture.cs"
if ($LASTEXITCODE -ne 0) { throw 'DrmFixture compilation failed' }

if ($SelfTest) {
    & "$fixtureRoot\PrivacyFixture.exe" --self-test
    if ($LASTEXITCODE -ne 0) { throw 'PrivacyFixture self-test failed' }
    & "$fixtureRoot\Netflix.exe" --self-test
    if ($LASTEXITCODE -ne 0) { throw 'Netflix fixture self-test failed' }

    function Assert-Rejected([string]$Executable, [string[]]$Arguments, [string]$CaseName) {
        $process = Start-Process -FilePath $Executable -ArgumentList $Arguments -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode -ne 2) { throw "$CaseName was not rejected with exit code 2 (actual $($process.ExitCode))" }
    }

    $testRoot = Join-Path ([IO.Path]::GetTempPath()) ('screenwise-fixture-selftest-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $testRoot | Out-Null
    try {
        Assert-Rejected "$fixtureRoot\PrivacyFixture.exe" @('--run-id', 'bad id', '--out-dir', (Join-Path $testRoot 'unused')) 'privacy invalid run ID'
        Assert-Rejected "$fixtureRoot\PrivacyFixture.exe" @('--run-id', 'valid-id', '--out-dir', $testRoot) 'privacy existing output directory'
        Assert-Rejected "$fixtureRoot\Netflix.exe" @('--run-id', 'bad id', '--phase-id', 'phase-1', '--out-dir', $testRoot) 'DRM invalid run ID'
        $duplicateReady = Join-Path $testRoot 'drm-ready-duplicate-phase.json'
        [IO.File]::WriteAllText($duplicateReady, '{}')
        Assert-Rejected "$fixtureRoot\Netflix.exe" @('--run-id', 'valid-id', '--phase-id', 'duplicate-phase', '--out-dir', $testRoot) 'DRM duplicate ready phase'
        Write-Output 'SELF_TEST_OK negative startup protocol cases'
    }
    finally {
        $resolvedTestRoot = [IO.Path]::GetFullPath($testRoot)
        $resolvedTempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
        if ($resolvedTestRoot.StartsWith($resolvedTempRoot, [StringComparison]::OrdinalIgnoreCase)) {
            Remove-Item -LiteralPath $resolvedTestRoot -Recurse -Force
        }
    }
}

Get-FileHash -Algorithm SHA256 "$fixtureRoot\PrivacyFixture.exe", "$fixtureRoot\Netflix.exe" |
    Select-Object Path, Hash
