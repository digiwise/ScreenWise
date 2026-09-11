#Requires -Version 7.0

[CmdletBinding()]
param(
    [ValidateSet('Minimal', 'Vision')]
    [string]$Mode = 'Vision',

    [ValidateRange(1024, 65535)]
    [int]$Port = 3047,

    [switch]$SkipFirewallRequirement
)

$ErrorActionPreference = 'Stop'
$RuleName = 'ScreenWise Offline Runtime Validation'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$targetRoot = (Resolve-Path (Join-Path $repoRoot 'target')).Path
$exePath = (Resolve-Path (Join-Path $targetRoot 'release\screenpipe.exe')).Path
$runId = [guid]::NewGuid().ToString('N')
$dataDir = Join-Path $targetRoot "live-security-validation-$runId"
$recorder = $null
$result = $null

function Assert-FirewallRule {
    if ($SkipFirewallRequirement) { return $null }

    $rules = @(Get-NetFirewallRule -DisplayName $RuleName -ErrorAction SilentlyContinue)
    if ($rules.Count -ne 1) {
        throw "Expected exactly one active '$RuleName' rule; found $($rules.Count). Run screenwise-offline-firewall.ps1 -Action Install from Administrator PowerShell."
    }
    $rule = $rules[0]
    $application = $rule | Get-NetFirewallApplicationFilter
    if ($rule.Enabled.ToString() -ne 'True' -or
        $rule.Direction.ToString() -ne 'Outbound' -or
        $rule.Action.ToString() -ne 'Block' -or
        $rule.Profile.ToString() -ne 'Any' -or
        $application.Program -ine $exePath) {
        throw "Firewall rule does not provide the required exact-program outbound block for '$exePath'."
    }
    return $rule
}

try {
    $firewallRule = Assert-FirewallRule
    if (Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction SilentlyContinue) {
        throw "Validation port $Port is already in use."
    }

    New-Item -ItemType Directory -Path $dataDir | Out-Null
    $stdoutPath = Join-Path $dataDir 'stdout.log'
    $stderrPath = Join-Path $dataDir 'stderr.log'
    $arguments = @(
        'record',
        '--data-dir', $dataDir,
        '--port', $Port,
        '--disable-audio',
        '--disable-meeting-detector',
        '--disable-snapshot-compaction',
        '--disable-keyboard-capture',
        '--disable-clipboard-capture'
    )
    if ($Mode -eq 'Minimal') {
        $arguments += '--disable-vision'
    }

    $recorder = Start-Process `
        -FilePath $exePath `
        -ArgumentList $arguments `
        -PassThru `
        -WindowStyle Hidden `
        -RedirectStandardOutput $stdoutPath `
        -RedirectStandardError $stderrPath

    $baseUri = "http://127.0.0.1:$Port"
    $health = $null
    for ($attempt = 0; $attempt -lt 60; $attempt++) {
        if ($recorder.HasExited) {
            throw "Recorder exited before its API became ready (exit $($recorder.ExitCode))."
        }
        try {
            $health = Invoke-WebRequest -Uri "$baseUri/health" -SkipHttpErrorCheck -TimeoutSec 1
            if ($health.StatusCode -eq 200) { break }
        }
        catch {
            $health = $null
        }
        Start-Sleep -Milliseconds 500
    }
    if ($null -eq $health -or $health.StatusCode -ne 200) {
        throw 'Health endpoint did not become ready.'
    }
    if ($Mode -eq 'Vision') {
        Start-Sleep -Seconds 12
    }

    $token = (& $exePath auth token --data-dir $dataDir).Trim()
    if ([string]::IsNullOrWhiteSpace($token)) {
        throw 'Auth token command returned an empty token.'
    }
    $unauthSearch = Invoke-WebRequest -Uri "$baseUri/search?limit=1" -SkipHttpErrorCheck
    $authSearch = Invoke-WebRequest -Uri "$baseUri/search?limit=1" -SkipHttpErrorCheck -Headers @{ Authorization = "Bearer $token" }
    $unauthFrame = Invoke-WebRequest -Uri "$baseUri/frames/1" -SkipHttpErrorCheck

    $pidPattern = "\s$($recorder.Id)\s*$"
    $tcpLines = @(netstat.exe -ano -p TCP | Where-Object { $_ -match $pidPattern })
    $listenerLines = @($tcpLines | Where-Object { $_ -match '\sLISTENING\s' })
    $externalEstablished = @($tcpLines | Where-Object {
        $_ -match '\sESTABLISHED\s' -and
        $_ -notmatch '\s(127\.0\.0\.1|\[::1\]):\d+\s+ESTABLISHED\s'
    })
    $udpLines = @(netstat.exe -ano -p UDP | Where-Object { $_ -match $pidPattern })
    $expectedListener = "^\s*TCP\s+127\.0\.0\.1:$Port\s+0\.0\.0\.0:0\s+LISTENING\s+$($recorder.Id)\s*$"

    if ($listenerLines.Count -ne 1 -or $listenerLines[0] -notmatch $expectedListener) {
        throw "Unexpected recorder listener set: $($listenerLines -join ' | ')"
    }
    if ($externalEstablished.Count -ne 0) { throw 'Recorder retained an external established TCP connection.' }
    if ($udpLines.Count -ne 0) { throw 'Recorder opened a UDP endpoint.' }
    if ($unauthSearch.StatusCode -ne 403) { throw "Unauthenticated search returned $($unauthSearch.StatusCode), expected 403." }
    if ($authSearch.StatusCode -ne 200) { throw "Authenticated search returned $($authSearch.StatusCode), expected 200." }
    if ($unauthFrame.StatusCode -ne 403) { throw "Unauthenticated frame returned $($unauthFrame.StatusCode), expected 403." }

    $result = [ordered]@{
        executable = $exePath
        executableSha256 = (Get-FileHash -LiteralPath $exePath -Algorithm SHA256).Hash
        firewallRuleRequired = -not [bool]$SkipFirewallRequirement
        firewallRuleActive = [bool]$firewallRule
        mode = $Mode
        healthStatus = [int]$health.StatusCode
        unauthenticatedSearchStatus = [int]$unauthSearch.StatusCode
        authenticatedSearchStatus = [int]$authSearch.StatusCode
        unauthenticatedFrameStatus = [int]$unauthFrame.StatusCode
        tcpListeners = @("127.0.0.1:$Port")
        externalEstablishedTcpCount = $externalEstablished.Count
        udpEndpointCount = $udpLines.Count
        helperPort11435ListenerCount = @($listenerLines | Where-Object { $_ -match ':11435\s' }).Count
    }
}
finally {
    if ($null -ne $recorder -and -not $recorder.HasExited) {
        Stop-Process -Id $recorder.Id -Force -ErrorAction SilentlyContinue
        Wait-Process -Id $recorder.Id -Timeout 10 -ErrorAction SilentlyContinue
    }
    if ($null -ne $recorder) {
        $recorder.Dispose()
        Start-Sleep -Milliseconds 200
    }
    if (Test-Path -LiteralPath $dataDir) {
        $resolvedData = (Resolve-Path -LiteralPath $dataDir).Path
        $expectedPrefix = Join-Path $targetRoot 'live-security-validation-'
        if (-not $resolvedData.StartsWith($expectedPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove unexpected validation directory: $resolvedData"
        }
        for ($attempt = 0; $attempt -lt 10; $attempt++) {
            try {
                Remove-Item -LiteralPath $resolvedData -Recurse -Force
                break
            }
            catch {
                if ($attempt -eq 9) { throw }
                Start-Sleep -Milliseconds 200
            }
        }
    }
}

if ($null -eq $result) { throw 'Validation did not produce a result.' }
$result | ConvertTo-Json -Depth 4
