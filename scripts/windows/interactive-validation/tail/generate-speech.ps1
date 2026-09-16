$ErrorActionPreference='Stop'
$outDir=Join-Path $PSScriptRoot 'speech-retry'
if(Test-Path -LiteralPath $outDir){throw 'Refusing to overwrite speech preparation directory'}
New-Item -ItemType Directory -Path $outDir | Out-Null
$items=@(
    @{Name='baseline';Marker='cobalt meadow seven';Text='Baseline control. Cobalt meadow seven. Cobalt meadow seven. Cobalt meadow seven. End of baseline control.'},
    @{Name='tail';Marker='violet harbor nine';Text='Final marker violet harbor nine.'},
    @{Name='restart';Marker='silver orchard five';Text='Restart control. Silver orchard five. Silver orchard five. Silver orchard five. End of restart control.'}
)
$voice=$null
$stream=$null
$streamOpen=$false
try {
    $voice=New-Object -ComObject SAPI.SpVoice
    if($voice.GetVoices().Count -lt 1){throw 'No installed SAPI voice'}
    $voice.Voice=$voice.GetVoices().Item(0)
    $voice.Rate=0
    $stream=New-Object -ComObject SAPI.SpFileStream
    $format=New-Object -ComObject SAPI.SpAudioFormat
    $format.Type=22
    $stream.Format=$format
    $records=@(foreach($item in $items){
        $file=Join-Path $outDir ($item.Name+'.wav')
        $stream.Open($file,3,$false)
        $streamOpen=$true
        # Assign the file sink before any Speak call; never use an audio device.
        $voice.AudioOutputStream=$stream
        [void]$voice.Speak($item.Text,0)
        [void]$voice.WaitUntilDone(-1)
        $stream.Close()
        $streamOpen=$false
        [ordered]@{name=$item.Name;file=($item.Name+'.wav');marker=$item.Marker;passage=$item.Text;sha256=(Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash}
    })
    [ordered]@{schema='screenwise.synthetic-tail-speech.v1';createdUtc=[DateTime]::UtcNow.ToString('o');voice=$voice.Voice.GetDescription();rate=0;playbackPerformed=$false;files=$records} |
        ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $outDir 'manifest.json') -Encoding UTF8
    'Generated three synthetic WAV files directly to disk; no playback'
} finally {
    if($stream){if($streamOpen){$stream.Close()};[void][Runtime.InteropServices.Marshal]::ReleaseComObject($stream)}
    if($voice){[void][Runtime.InteropServices.Marshal]::ReleaseComObject($voice)}
}
