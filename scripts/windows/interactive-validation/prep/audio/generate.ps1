[CmdletBinding()]
param(
    [string]$OutputDirectory = $PSScriptRoot
)

$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Speech

$passages = @(
    [ordered]@{
        id = 'microphone-read-45s'
        file = 'microphone-read-45s.wav'
        phase = 'reference'
        purpose = '45 second spoken-word reference passage for later microphone validation'
        text = 'Synthetic reference recording. The quick brown fox jumps over the lazy dog. Pack my box with five dozen liquor jugs. ScreenWise local capture keeps activity on this computer. Before playback, the recorder should report a quiet state. One, two, three, four, five. During playback, the recorder should report active audio. During marker: red maple, blue river, green window, seven twenty four. After playback, the recorder should report the end of the test. After marker: quiet room, closed book, amber light, nine thirty. End of synthetic reference recording.'
    },
    [ordered]@{
        id = 'normal-baseline-30s'
        file = 'normal-baseline-30s.wav'
        phase = 'baseline'
        purpose = 'normal speech baseline for approximately 30 seconds of later playback'
        text = 'Normal baseline marker. This is a calm synthetic speech sample for local audio search. The recorder should capture a steady voice with ordinary pauses and clear words. Numbers for the baseline are twelve, twenty four, and forty eight. The sample has varied sounds and a measured pace for transcription. It includes a clear opening, middle, and closing phrase for later comparison. The baseline ends after this sentence.'
    },
    [ordered]@{
        id = 'phase-before-10s'
        file = 'phase-before-10s.wav'
        phase = 'before'
        purpose = 'distinct pre-playback marker for lock and DRM test sequencing'
        text = 'BEFORE MARKER: silver cedar, north star, eleven forty. This is the pre-playback phase.'
    },
    [ordered]@{
        id = 'phase-during-10s'
        file = 'phase-during-10s.wav'
        phase = 'during'
        purpose = 'distinct during-playback marker for lock and DRM test sequencing'
        text = 'DURING MARKER: crimson bridge, violet lake, twenty seven. This is the active playback phase.'
    },
    [ordered]@{
        id = 'phase-after-10s'
        file = 'phase-after-10s.wav'
        phase = 'after'
        purpose = 'distinct post-playback marker for lock and DRM test sequencing'
        text = 'AFTER MARKER: amber window, quiet harbor, thirty nine. This is the post-playback phase.'
    }
)

$manifestPath = Join-Path $OutputDirectory 'manifest.json'
$allPaths = @($manifestPath) + @($passages | ForEach-Object { Join-Path $OutputDirectory $_.file })
$existing = @($allPaths | Where-Object { Test-Path -LiteralPath $_ })
if ($existing.Count -gt 0) {
    throw "Refusing to overwrite existing audio preparation files: $($existing -join ', ')"
}

$synth = $null
$sapiStream = $null
$sapiVoice = $null
$synthesisMode = 'System.Speech'
try {
    try {
        $synth = [System.Speech.Synthesis.SpeechSynthesizer]::new()
        $voice = $synth.Voice
        if ($null -eq $voice) { throw 'System.Speech returned no voice.' }
    } catch {
        if ($null -ne $synth) { $synth.Dispose(); $synth = $null }
        $synthesisMode = 'Windows SAPI COM'
        $sapiVoice = New-Object -ComObject SAPI.SpVoice
        if ($sapiVoice.GetVoices().Count -lt 1) { throw 'No installed System.Speech or Windows SAPI voice is available.' }
        $voice = $sapiVoice.GetVoices().Item(0)
        $voiceName = $voice.GetDescription()
        $voiceCulture = 'unknown'
        $sapiStream = New-Object -ComObject SAPI.SpFileStream
        $sapiFormat = New-Object -ComObject SAPI.SpAudioFormat
        $sapiFormat.Type = 22
        $sapiStream.Format = $sapiFormat
    }
    if ($synth) { $synth.Rate = 0; $synth.Volume = 100 }
    $records = @()
    foreach ($item in $passages) {
        $path = Join-Path $OutputDirectory $item.file
        if ($synthesisMode -eq 'System.Speech') {
            $synth.SetOutputToWaveFile($path)
            $synth.Speak($item.text)
            $synth.SetOutputToNull()
            $voiceName = $voice.Name
            $voiceCulture = $voice.Culture.Name
        } else {
            $sapiStream.Open($path, 3, $false)
            $sapiVoice.AudioOutputStream = $sapiStream
            $sapiVoice.Speak($item.text, 0)
            $sapiVoice.WaitUntilDone(-1)
            $sapiStream.Close()
        }
        $bytes = [System.IO.File]::ReadAllBytes($path)
        if ($bytes.Length -lt 44) { throw "Generated WAV is too short: $path" }
        $sampleRate = [BitConverter]::ToInt32($bytes, 24)
        $channels = [BitConverter]::ToInt16($bytes, 22)
        $bits = [BitConverter]::ToInt16($bytes, 34)
        $dataOffset = 0
        for ($i = 12; $i -le $bytes.Length - 8; $i++) {
            if ($bytes[$i] -eq 0x64 -and $bytes[$i + 1] -eq 0x61 -and $bytes[$i + 2] -eq 0x74 -and $bytes[$i + 3] -eq 0x61) { $dataOffset = $i; break }
        }
        if ($dataOffset -eq 0) { throw "Generated WAV has no data chunk: $path" }
        $dataBytes = [BitConverter]::ToInt32($bytes, $dataOffset + 4)
        $durationSeconds = [math]::Round($dataBytes / ($sampleRate * $channels * ($bits / 8)), 3)
        $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
        $records += [ordered]@{
            id = $item.id; file = $item.file; phase = $item.phase; purpose = $item.purpose
            passage = $item.text; voice = $voiceName; voiceCulture = $voiceCulture
            format = [ordered]@{ container = 'WAV'; encoding = 'PCM'; sampleRateHz = $sampleRate; channels = $channels; bitsPerSample = $bits }
            durationSeconds = $durationSeconds; sha256 = $hash
        }
    }
    $manifest = [ordered]@{
        schemaVersion = 1
        generatedAtUtc = [DateTime]::UtcNow.ToString('o')
        generator = "$synthesisMode; no playback or recording"
        voice = $voiceName
        voiceCulture = $voiceCulture
        synthesisRate = 0
        files = $records
        validation = [ordered]@{ status = 'generated-and-header-validated'; checked = @('WAV PCM header', 'duration', 'SHA-256', 'manifest passage text') }
        limitation = 'Synthetic speech does not prove microphone routing, live playback, DRM behavior, or locked-screen capture.'
    }
    $manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $manifestPath -Encoding utf8NoBOM
}
finally {
    if ($synth) { $synth.Dispose() }
    if ($sapiStream) { $sapiStream.Close(); [Runtime.InteropServices.Marshal]::ReleaseComObject($sapiStream) | Out-Null }
    if ($sapiVoice) { [Runtime.InteropServices.Marshal]::ReleaseComObject($sapiVoice) | Out-Null }
}

Write-Output "Generated $($passages.Count) WAV clips and manifest at $OutputDirectory"
