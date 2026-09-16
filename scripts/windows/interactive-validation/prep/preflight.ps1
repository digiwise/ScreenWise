# screenpipe — local synthetic validation preparation
param(
    [Parameter(Mandatory = $true)][string]$ConfigPath,
    [string]$ExpectedConfigSha256,
    [ValidateSet('locked', 'unlocked', 'either')][string]$RequireState = 'locked',
    [switch]$RequireBrowser
)
$ErrorActionPreference = 'Stop'

function Require-ConfiguredString($Object, [string]$Name) {
    $value = $Object.$Name
    if ($value -isnot [string] -or [string]::IsNullOrWhiteSpace($value) -or $value.Contains('<')) {
        throw "Configuration field '$Name' must be explicitly set."
    }
    return $value
}

$resolvedConfig = (Resolve-Path -LiteralPath $ConfigPath -ErrorAction Stop).Path
$actualConfigSha256 = (Get-FileHash -LiteralPath $resolvedConfig -Algorithm SHA256).Hash
if ($ExpectedConfigSha256 -and $actualConfigSha256 -ne $ExpectedConfigSha256) {
    throw 'Validation configuration changed after launcher review.'
}
$config = Get-Content -LiteralPath $resolvedConfig -Raw | ConvertFrom-Json
if ($config.schema -ne 'screenwise.interactive-validation-config.v1') { throw 'Unsupported validation configuration schema.' }
$repo = Require-ConfiguredString $config 'repo_root'
$python = Require-ConfiguredString $config 'python_exe'
$developerShell = Require-ConfiguredString $config 'developer_shell'
$openBlas = Require-ConfiguredString $config 'openblas_path'
$ort = Require-ConfiguredString $config 'ort_lib_location'
$release = Require-ConfiguredString $config 'release_dir'
$group = Require-ConfiguredString $config.firewall 'group'
$normalUserSid = Require-ConfiguredString $config 'normal_user_sid'
$requiredDevices = @(
    (Require-ConfiguredString $config.audio 'input_device'),
    (Require-ConfiguredString $config.audio 'output_device')
)
try { $apiPort = [int]$config.api_port; $fixturePort = [int]$config.fixture_port }
catch { throw 'Configured API and fixture ports must be integers.' }
if ($apiPort -ne $config.api_port -or $fixturePort -ne $config.fixture_port -or
    $apiPort -lt 1024 -or $apiPort -gt 65535 -or $fixturePort -lt 1024 -or
    $fixturePort -gt 65535 -or $apiPort -eq $fixturePort) {
    throw 'Configured API and fixture ports must be distinct integers from 1024 through 65535.'
}
foreach ($path in @($repo, $python, $developerShell, $openBlas, $ort, $release)) {
    if (-not [IO.Path]::IsPathFullyQualified($path) -or -not (Test-Path -LiteralPath $path)) {
        throw "Configured path is not absolute or does not exist: $path"
    }
}
$repo = (Resolve-Path -LiteralPath $repo).Path
$release = (Resolve-Path -LiteralPath $release).Path
foreach ($file in @($python, $developerShell)) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Configured tool is not a file: $file" }
}
foreach ($directory in @($repo, $openBlas, $ort, $release)) {
    if (-not (Test-Path -LiteralPath $directory -PathType Container)) { throw "Configured directory is invalid: $directory" }
}
if (-not (Test-Path -LiteralPath (Join-Path $repo 'Cargo.toml') -PathType Leaf) -or
    $release -ine [IO.Path]::GetFullPath((Join-Path $repo 'target\release'))) {
    throw 'repo_root is not a screenpipe repository or release_dir is not its target\release directory.'
}
$packagedTargets = [ordered]@{
    recorder = Join-Path $release 'screenpipe.exe'
    ffmpeg = Join-Path $release 'ffmpeg.exe'
    ffprobe = Join-Path $release 'ffprobe.exe'
}
$targets = [ordered]@{}
foreach ($entry in $packagedTargets.GetEnumerator()) { $targets[$entry.Key] = $entry.Value }
if ($RequireBrowser) {
    $browserExecutable = Require-ConfiguredString $config.browser 'executable'
    $browserSha256 = Require-ConfiguredString $config.browser 'sha256'
    if (-not [IO.Path]::IsPathFullyQualified($browserExecutable) -or
        -not (Test-Path -LiteralPath $browserExecutable -PathType Leaf)) {
        throw 'Configured browser executable is not an existing absolute file.'
    }
    if ($browserSha256 -notmatch '^[A-Fa-f0-9]{64}$') { throw 'Configured browser SHA-256 is invalid.' }
    $browserExecutable = (Resolve-Path -LiteralPath $browserExecutable).Path
    $actualBrowserSha256 = (Get-FileHash -LiteralPath $browserExecutable -Algorithm SHA256).Hash
    if ($actualBrowserSha256 -ne $browserSha256.ToUpperInvariant()) {
        throw 'Configured browser executable changed after firewall review.'
    }
    $targets.browser = $browserExecutable
}
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
if ($identity.User.Value -ne $normalUserSid) { throw 'Run under the configured normal recorder user.' }
$stamp = [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss-fffffff')
$evidence = Join-Path $repo "target\interactive-validation\preflight\$stamp"
New-Item -ItemType Directory -Path $evidence -ErrorAction Stop | Out-Null
$checks = @()
function Check([string]$name, [bool]$pass, $details) {
    $script:checks += @{name = $name; pass = $pass; details = $details}
    $script:checks | ConvertTo-Json -Depth 9 | Set-Content -LiteralPath (Join-Path $evidence 'checks.json')
    if (-not $pass) { throw "Preflight failed: $name" }
}
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
Check 'normal_unelevated_token' (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) 'Recorder tests require a normal, non-elevated user token.'
$supportedAddresses = @('0.0.0.0-126.255.255.255', '128.0.0.0-255.255.255.255', '::', '::2-ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff')
$addresses = @($config.firewall.remote_addresses)
Check 'firewall_remote_scope_supported' ($addresses.Count -eq $supportedAddresses.Count -and @(Compare-Object $supportedAddresses $addresses).Count -eq 0) $addresses
foreach ($store in @('PersistentStore', 'ActiveStore')) {
    $rules = @(Get-NetFirewallRule -PolicyStore $store -Group $group)
    Check "${store}_rule_names" ($rules.Count -eq $targets.Count -and @(Compare-Object @($targets.Keys | ForEach-Object {"$group-$_"}) @($rules.Name)).Count -eq 0) @($rules.Name)
    foreach ($entry in $targets.GetEnumerator()) {
        $rule = @($rules | Where-Object Name -eq "$group-$($entry.Key)")
        Check "${store}_rule_identity_$($entry.Key)" ($rule.Count -eq 1) $rule.Name
        $app = $rule | Get-NetFirewallApplicationFilter
        $addr = $rule | Get-NetFirewallAddressFilter
        $port = $rule | Get-NetFirewallPortFilter
        $interface = $rule | Get-NetFirewallInterfaceFilter
        $interfaceType = $rule | Get-NetFirewallInterfaceTypeFilter
        $service = $rule | Get-NetFirewallServiceFilter
        $enforced = $store -eq 'PersistentStore' -or 'Enforced' -in @($rule.EnforcementStatus | ForEach-Object ToString)
        $pass = $app.Program -ieq $entry.Value -and $app.Package -in @($null, '', 'Any') -and
            $rule.Enabled.ToString() -eq 'True' -and $rule.Direction.ToString() -eq 'Outbound' -and
            $rule.Action.ToString() -eq 'Block' -and $rule.Profile.ToString() -eq 'Any' -and
            $port.Protocol -eq 'Any' -and $port.LocalPort -eq 'Any' -and $port.RemotePort -eq 'Any' -and
            $addr.LocalAddress -eq 'Any' -and @(Compare-Object $supportedAddresses @($addr.RemoteAddress)).Count -eq 0 -and
            $interface.InterfaceAlias -eq 'Any' -and $interfaceType.InterfaceType -eq 'Any' -and
            $service.Service -eq 'Any' -and $enforced
        Check "${store}_rule_scope_$($entry.Key)" $pass @{program = $app.Program; package = $app.Package; local = $addr.LocalAddress; remote = $addr.RemoteAddress; local_port = $port.LocalPort; remote_port = $port.RemotePort; protocol = $port.Protocol; interface = $interface.InterfaceAlias; interface_type = $interfaceType.InterfaceType; service = $service.Service; enforcement = @($rule.EnforcementStatus | ForEach-Object ToString)}
    }
}
$profiles = @(Get-NetFirewallProfile -PolicyStore ActiveStore)
Check 'firewall_profiles' (@($profiles | Where-Object {$_.Enabled.ToString() -ne 'True' -or $_.AllowLocalFirewallRules.ToString() -ne 'True'}).Count -eq 0) @($profiles | Select-Object Name, Enabled, DefaultInboundAction, DefaultOutboundAction, AllowLocalFirewallRules)
$services = @(Get-Service BFE, MpsSvc)
Check 'firewall_services' (@($services | Where-Object Status -ne Running).Count -eq 0) @($services | Select-Object Name, Status)
$running = @(Get-CimInstance Win32_Process | Where-Object {$_.ExecutablePath -in $targets.Values} | Select-Object ProcessId, ExecutablePath)
Check 'no_scoped_recorder_running' ($running.Count -eq 0) $running
$listeners = @(Get-NetTCPConnection -State Listen | Where-Object LocalPort -in @($apiPort, $fixturePort) | Select-Object LocalAddress, LocalPort)
Check 'test_ports_free' ($listeners.Count -eq 0) $listeners
$osJson = & $python "$PSScriptRoot\lock_probe.py"
if ($LASTEXITCODE -ne 0) { throw 'WTS probe failed' }
$osState = $osJson | ConvertFrom-Json
$osJson | Set-Content (Join-Path $evidence 'os-state.json')
$known = $osState.wts_level -eq 1 -and $osState.wts_session -eq $osState.session -and $osState.wts_state -eq 0 -and $osState.wts_flags -in @(0, 1)
$statePass = $known -and ($RequireState -eq 'either' -or ($RequireState -eq 'locked' -and $osState.wts_flags -eq 0) -or ($RequireState -eq 'unlocked' -and $osState.wts_flags -eq 1 -and $osState.desktop_access_256.available))
Check 'requested_windows_session_state' $statePass @{required = $RequireState; state = $osState}
$pins = @($config.runtime_pins)
Check 'runtime_pins_present' ($pins.Count -gt 0) $pins.Count
$derivedModels = [ordered]@{
    parakeet_encoder = Join-Path $env:LOCALAPPDATA 'screenpipe\audio-models\parakeet-tdt-0.6b-v3\encoder-model.int8.onnx'
    parakeet_decoder = Join-Path $env:LOCALAPPDATA 'screenpipe\audio-models\parakeet-tdt-0.6b-v3\decoder_joint-model.int8.onnx'
    parakeet_vocab = Join-Path $env:LOCALAPPDATA 'screenpipe\audio-models\parakeet-tdt-0.6b-v3\vocab.txt'
    silero_vad = Join-Path $env:LOCALAPPDATA 'screenpipe\vad\silero_vad_v5.onnx'
    segmentation = Join-Path $env:LOCALAPPDATA 'screenpipe\models\segmentation-3.0.onnx'
    wespeaker = Join-Path $env:LOCALAPPDATA 'screenpipe\models\wespeaker_en_voxceleb_CAM++.onnx'
}
$expectedPins = [ordered]@{
    recorder = $targets.recorder
    ffmpeg = $targets.ffmpeg
    ffprobe = $targets.ffprobe
    libopenblas = Join-Path $release 'libopenblas.dll'
    onnxruntime = Join-Path $release 'onnxruntime.dll'
    parakeet_encoder = $derivedModels.parakeet_encoder
    parakeet_decoder = $derivedModels.parakeet_decoder
    parakeet_vocab = $derivedModels.parakeet_vocab
    silero_vad = $derivedModels.silero_vad
    segmentation = $derivedModels.segmentation
    wespeaker = $derivedModels.wespeaker
}
foreach ($role in $derivedModels.Keys) {
    $configuredModel = Require-ConfiguredString $config.models $role
    Check ('runtime_model_path_' + $role) ([IO.Path]::GetFullPath($configuredModel) -ieq [IO.Path]::GetFullPath($derivedModels[$role])) @{configured = $configuredModel; runtime = $derivedModels[$role]}
}
$configuredRoles = @($pins | ForEach-Object {$_.role})
Check 'runtime_pin_closure' ($pins.Count -eq $expectedPins.Count -and @(Compare-Object @($expectedPins.Keys) $configuredRoles).Count -eq 0) $configuredRoles
foreach ($pin in $pins) {
    $role = Require-ConfiguredString $pin 'role'
    $pinPath = Require-ConfiguredString $pin 'path'
    $expected = Require-ConfiguredString $pin 'sha256'
    Check ('pin_sha256_format_' + [IO.Path]::GetFileName($pinPath)) ($expected -match '^[A-Fa-f0-9]{64}$') @{path = $pinPath}
    $expanded = [Environment]::ExpandEnvironmentVariables($pinPath)
    if (-not [IO.Path]::IsPathFullyQualified($expanded)) { $expanded = Join-Path $repo $expanded }
    $expanded = [IO.Path]::GetFullPath($expanded)
    $expectedPath = [IO.Path]::GetFullPath($expectedPins[$role])
    Check ('runtime_pin_path_' + $role) ($expanded -ieq $expectedPath) @{configured = $expanded; expected = $expectedPath}
    $actual = if (Test-Path -LiteralPath $expanded -PathType Leaf) {(Get-FileHash -LiteralPath $expanded -Algorithm SHA256).Hash} else {'MISSING'}
    Check ('sha256_' + [IO.Path]::GetFileName($expanded)) ($actual -eq $expected.ToUpperInvariant()) @{path = $expanded; sha256 = $actual}
}
foreach ($target in $packagedTargets.Values) {
    $resolvedTarget = [IO.Path]::GetFullPath($target)
    $isPinned = @($pins | Where-Object {
        $configured = [Environment]::ExpandEnvironmentVariables($_.path)
        if (-not [IO.Path]::IsPathFullyQualified($configured)) { $configured = Join-Path $repo $configured }
        [IO.Path]::GetFullPath($configured) -ieq $resolvedTarget
    }).Count -eq 1
    Check ('required_executable_pinned_' + [IO.Path]::GetFileName($target)) $isPinned $resolvedTarget
}
& $developerShell -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
if ($LASTEXITCODE -ne 0) { throw 'Visual Studio developer shell initialization failed.' }
$env:OPENBLAS_PATH = (Resolve-Path -LiteralPath $openBlas).Path
$env:CMAKE_GENERATOR = 'Ninja'
$env:ORT_LIB_LOCATION = (Resolve-Path -LiteralPath $ort).Path
$env:PATH = "$release;$env:OPENBLAS_PATH\bin;$env:PATH"
$env:HF_HUB_OFFLINE = '1'
$env:CARGO_NET_OFFLINE = 'true'
foreach ($kind in @('audio', 'vision')) {
    $output = & $targets.recorder $kind list --output json 2> (Join-Path $evidence "$kind-list.stderr.txt")
    $exitCode = $LASTEXITCODE
    $output | Set-Content (Join-Path $evidence "$kind-list.json")
    Check "${kind}_enumeration" ($exitCode -eq 0) @{exit = $exitCode; file = "$kind-list.json"}
    $inventory = $output | ConvertFrom-Json
    Check "${kind}_inventory_valid" ($inventory.success -eq $true -and @($inventory.data).Count -gt 0) @{count = @($inventory.data).Count}
    if ($kind -eq 'audio') {
        Check 'selected_audio_devices_present' (@($requiredDevices | Where-Object {$_ -notin $inventory.data.name}).Count -eq 0) $requiredDevices
    }
}
@{utc = [DateTime]::UtcNow.ToString('o'); passed = $true; recording_started = $false; require_state = $RequireState; vs = $env:VSCMD_VER; msvc = $env:VCToolsVersion; sdk = $env:WindowsSDKVersion; config_path = $resolvedConfig; environment = @{OPENBLAS_PATH = $env:OPENBLAS_PATH; CMAKE_GENERATOR = $env:CMAKE_GENERATOR; ORT_LIB_LOCATION = $env:ORT_LIB_LOCATION}; checks = $checks.Count} | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $evidence 'summary.json')
Write-Output "Preparation preflight passed ($($checks.Count) checks). Evidence: $evidence"
