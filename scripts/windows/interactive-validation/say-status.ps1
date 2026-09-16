[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('lock-now', 'unlock-now', 'finished', 'aborted')]
    [string]$Status,

    [switch]$Speak
)

$ErrorActionPreference = 'Stop'
$phrases = @{
    'lock-now'   = 'Press Windows L to lock the computer now.'
    'unlock-now' = 'Please unlock the computer now.'
    'finished'   = 'The test has finished. Check the chat for the result.'
    'aborted'    = 'The test has stopped. You can unlock the computer normally.'
}

if ($Speak) {
    Add-Type -AssemblyName System.Speech
    $synthesizer = [System.Speech.Synthesis.SpeechSynthesizer]::new()
    try {
        $synthesizer.Speak($phrases[$Status])
    }
    finally {
        $synthesizer.Dispose()
    }
}

[ordered]@{
    schema          = 'screenwise.interactive-validation-status-cue.v1'
    status          = $Status
    phrase          = $phrases[$Status]
    speak_requested = [bool]$Speak
} | ConvertTo-Json -Compress
