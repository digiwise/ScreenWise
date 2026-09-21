param(
    [Parameter(Mandatory)] [string]$ExecutablePath,
    [Parameter(Mandatory)] [string]$DataDir,
    [Parameter(Mandatory)] [string]$SessionDir,
    [Parameter(Mandatory)] [int]$Port,
    [Parameter(Mandatory)] [string]$StartedAtUtc
)

$ErrorActionPreference = 'Stop'
$pollFile = Join-Path $SessionDir 'operational-metrics.jsonl'
$finalFile = Join-Path $SessionDir 'final-process-audit.json'
$releaseDir = Split-Path -Parent $ExecutablePath
$scopedPaths = @(
    $ExecutablePath,
    (Join-Path $releaseDir 'ffmpeg.exe'),
    (Join-Path $releaseDir 'ffprobe.exe')
)

function Get-ScopedProcesses {
    @(
        Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
            Where-Object {
                $_.ExecutablePath -and
                $scopedPaths -contains $_.ExecutablePath
            } |
            Select-Object ProcessId, ParentProcessId, Name, ExecutablePath, CreationDate
    )
}

function Select-NumericMetrics($value) {
    if ($null -eq $value) { return $null }
    $result = [ordered]@{}
    foreach ($property in $value.PSObject.Properties) {
        if ($null -eq $property.Value -or
            $property.Value -is [bool] -or
            $property.Value -is [byte] -or
            $property.Value -is [int16] -or
            $property.Value -is [int32] -or
            $property.Value -is [int64] -or
            $property.Value -is [uint16] -or
            $property.Value -is [uint32] -or
            $property.Value -is [uint64] -or
            $property.Value -is [single] -or
            $property.Value -is [double] -or
            $property.Value -is [decimal]) {
            $result[$property.Name] = $property.Value
        }
    }
    [pscustomobject]$result
}

$recorder = $null
$deadline = (Get-Date).AddMinutes(2)
while ((Get-Date) -lt $deadline) {
    $recorder = @(Get-ScopedProcesses | Where-Object {
        [StringComparer]::OrdinalIgnoreCase.Equals($_.ExecutablePath, $ExecutablePath)
    }) | Select-Object -First 1
    if ($recorder) { break }
    Start-Sleep -Milliseconds 250
}
if (-not $recorder) {
    @{ schema = 'screenwise.day-to-day-trial-monitor.v1'; error = 'recorder_start_not_observed' } |
        ConvertTo-Json | Set-Content -LiteralPath $finalFile -Encoding utf8
    exit 2
}

$token = $null
for ($attempt = 0; $attempt -lt 60 -and -not $token; $attempt++) {
    try {
        $candidate = (& $ExecutablePath auth token --data-dir $DataDir 2>$null | Select-Object -Last 1).Trim()
        if ($candidate) { $token = $candidate }
    }
    catch {}
    if (-not $token) { Start-Sleep -Seconds 1 }
}

$headers = if ($token) { @{ Authorization = "Bearer $token" } } else { @{} }
$baseUri = "http://127.0.0.1:$Port"
$stoppedSamples = 0
while ($stoppedSamples -lt 3) {
    $current = @(Get-ScopedProcesses | Where-Object {
        [StringComparer]::OrdinalIgnoreCase.Equals($_.ExecutablePath, $ExecutablePath)
    })
    if ($current.Count -eq 0) {
        $stoppedSamples++
        Start-Sleep -Seconds 1
        continue
    }
    $stoppedSamples = 0

    $sample = [ordered]@{
        schema = 'screenwise.day-to-day-trial-sample.v2'
        sampled_at_utc = (Get-Date).ToUniversalTime().ToString('o')
        recorder_process_count = $current.Count
        api = [ordered]@{ reachable = $false; authenticated = [bool]$token }
    }
    try {
        $health = Invoke-RestMethod -Uri "$baseUri/health" -TimeoutSec 5
        $sample.api.reachable = $true
        $sample.health = [ordered]@{
            status = $health.status
            frame_status = $health.frame_status
            audio_status = $health.audio_status
            vision_db_write_stalled = $health.vision_db_write_stalled
            audio_db_write_stalled = $health.audio_db_write_stalled
            drm_content_paused = $health.drm_content_paused
            schedule_paused = $health.schedule_paused
            ui_recorder = $health.ui_recorder
            audio_pipeline = Select-NumericMetrics $health.audio_pipeline
            pipeline = Select-NumericMetrics $health.pipeline
        }
    }
    catch {
        $sample.api.health_error = 'unavailable'
    }
    if ($token) {
        try {
            $end = (Get-Date).ToUniversalTime().ToString('o')
            $query = '?start_time={0}&end_time={1}&limit=1000' -f
                [Uri]::EscapeDataString($StartedAtUtc), [Uri]::EscapeDataString($end)
            $events = Invoke-RestMethod -Uri "$baseUri/capture-events$query" -Headers $headers -TimeoutSec 5
            $sample.capture_events = [ordered]@{
                reason_codes = @($events.data | ForEach-Object { $_.reason_code })
                states = @($events.data | ForEach-Object { $_.state })
                persistence_degraded = $events.persistence_degraded
                event_delivery = $events.event_delivery
                audio_delivery = $events.audio_delivery
                audio_shutdown_degraded = $events.audio_shutdown_degraded
            }
        }
        catch {
            $sample.api.capture_events_error = 'unavailable'
        }
        try {
            $vision = Invoke-RestMethod -Uri "$baseUri/vision/metrics" -Headers $headers -TimeoutSec 5
            $sample.vision_metrics = Select-NumericMetrics $vision
        }
        catch {
            $sample.api.vision_metrics_error = 'unavailable'
        }
        try {
            $audio = Invoke-RestMethod -Uri "$baseUri/audio/metrics" -Headers $headers -TimeoutSec 5
            $sample.audio_metrics = Select-NumericMetrics $audio
        }
        catch {
            $sample.api.audio_metrics_error = 'unavailable'
        }
    }
    [IO.File]::AppendAllText($pollFile, (($sample | ConvertTo-Json -Depth 9 -Compress) + [Environment]::NewLine))
    Start-Sleep -Seconds 30
}

Start-Sleep -Seconds 2
$remaining = @(Get-ScopedProcesses)
$logFiles = @(Get-ChildItem -LiteralPath $DataDir -Filter 'screenpipe*.log' -File -ErrorAction SilentlyContinue)
$panicFiles = @(Get-ChildItem -LiteralPath $DataDir -Filter '*panic*.log' -File -ErrorAction SilentlyContinue)
[ordered]@{
    schema = 'screenwise.day-to-day-trial-final.v1'
    sampled_at_utc = (Get-Date).ToUniversalTime().ToString('o')
    scoped_processes_remaining = $remaining
    scoped_process_count = $remaining.Count
    rolling_log_file_count = $logFiles.Count
    rolling_log_total_bytes = ($logFiles | Measure-Object Length -Sum).Sum
    panic_log_file_count = $panicFiles.Count
    monitor_had_api_token = [bool]$token
} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $finalFile -Encoding utf8
