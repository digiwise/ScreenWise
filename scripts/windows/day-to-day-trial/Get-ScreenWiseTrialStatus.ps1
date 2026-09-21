[CmdletBinding()]
param(
    [string]$DataDir,
    [string]$SessionName,
    [switch]$AsJson
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..\..')).Path
if ([string]::IsNullOrWhiteSpace($DataDir)) {
    $DataDir = Join-Path $repoRoot '.local\day-to-day-trial'
}
$DataDir = [IO.Path]::GetFullPath($DataDir)
$auditRoot = Join-Path $DataDir '.trial-audit'
$session = if ([string]::IsNullOrWhiteSpace($SessionName)) {
    Get-ChildItem -LiteralPath $auditRoot -Directory -ErrorAction Stop |
        Sort-Object LastWriteTime |
        Select-Object -Last 1
}
else {
    Get-Item -LiteralPath (Join-Path $auditRoot $SessionName) -ErrorAction Stop
}
if (-not $session) { throw "No trial session exists under $auditRoot" }

$launchPath = Join-Path $session.FullName 'launch.json'
$metricsPath = Join-Path $session.FullName 'operational-metrics.jsonl'
$exitPath = Join-Path $session.FullName 'launcher-exit.json'
$finalPath = Join-Path $session.FullName 'final-process-audit.json'
$launch = Get-Content -Raw -LiteralPath $launchPath | ConvertFrom-Json
$samples = @(Get-Content -LiteralPath $metricsPath -ErrorAction SilentlyContinue |
    Where-Object { $_.Trim() } |
    ForEach-Object { $_ | ConvertFrom-Json })
if ($samples.Count -eq 0) { throw "No operational sample is available for $($session.Name)" }
$latest = $samples | Select-Object -Last 1
$launcherExit = if (Test-Path -LiteralPath $exitPath) {
    Get-Content -Raw -LiteralPath $exitPath | ConvertFrom-Json
}
$finalAudit = if (Test-Path -LiteralPath $finalPath) {
    Get-Content -Raw -LiteralPath $finalPath | ConvertFrom-Json
}

function Read-LogAfterOffset {
    param([string]$Path, [long]$Offset)
    $stream = $null
    $reader = $null
    try {
        $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
        if ($stream.Length -lt $Offset) { $Offset = 0L }
        [void]$stream.Seek($Offset, [IO.SeekOrigin]::Begin)
        $reader = [IO.StreamReader]::new($stream)
        $reader.ReadToEnd() -replace "`e\[[0-9;]*m", ''
    }
    finally {
        if ($reader) { $reader.Dispose() }
        elseif ($stream) { $stream.Dispose() }
    }
}

function Convert-RecorderTimestampToUtc {
    param([string]$Timestamp, [bool]$LegacyLocalClock)
    if (-not $LegacyLocalClock) {
        return [datetimeoffset]::Parse($Timestamp).ToUniversalTime()
    }
    $withoutSuffix = $Timestamp.TrimEnd('Z')
    $local = [datetime]::SpecifyKind([datetime]::Parse($withoutSuffix), [DateTimeKind]::Unspecified)
    return [datetimeoffset]::new($local, [TimeZoneInfo]::Local.GetUtcOffset($local)).ToUniversalTime()
}

$logText = ''
$offsetProperties = if ($null -ne $launch.recorder_log_offsets) {
    @($launch.recorder_log_offsets.PSObject.Properties)
}
else {
    @()
}
$legacyLocalClock = $offsetProperties.Count -eq 0
if (-not $legacyLocalClock) {
    foreach ($property in $offsetProperties) {
        if (Test-Path -LiteralPath $property.Name -PathType Leaf) {
            $logText += Read-LogAfterOffset -Path $property.Name -Offset ([long]$property.Value)
        }
    }
    Get-ChildItem -LiteralPath $DataDir -Filter 'screenpipe*.log' -File -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -notin @($offsetProperties.Name) } |
        ForEach-Object { $logText += Read-LogAfterOffset -Path $_.FullName -Offset 0 }
}
else {
    # v1 launch records predate byte offsets. Their recorder used local wall
    # time with a misleading Z suffix, so compare its timestamp as local time.
    $started = ([datetimeoffset]$launch.started_at_utc).ToLocalTime().ToString('yyyy-MM-ddTHH:mm:ss')
    foreach ($log in @(Get-ChildItem -LiteralPath $DataDir -Filter 'screenpipe*.log' -File -ErrorAction SilentlyContinue)) {
        $lines = (Read-LogAfterOffset -Path $log.FullName -Offset 0) -split "`r?`n"
        $logText += (($lines | Where-Object {
            $_.Length -ge 19 -and
            [StringComparer]::Ordinal.Compare($_.Substring(0, 19), $started) -ge 0
        }) -join [Environment]::NewLine)
    }
}

$logLines = @($logText -split "`r?`n" | Where-Object { $_.Trim() })
$warningLines = @($logLines | Where-Object { $_ -match '\sWARN\s' })
$errorLines = @($logLines | Where-Object { $_ -match '\sERROR\s' })
function Count-Matching([string]$Pattern) {
    @($logLines | Where-Object { $_ -match $Pattern }).Count
}
function Number-OrZero($Value) {
    if ($null -eq $Value) { return 0L }
    return [long]$Value
}

$pipeline = $latest.health.pipeline
$audio = $latest.health.audio_pipeline
$capture = $latest.capture_events
$excludedBackground = Count-Matching 'active-window-only; excluded background pixels'
$excludedForeground = Count-Matching 'visible redaction frame: active window is excluded'
$noSafeWindow = Count-Matching 'visible redaction frame: no safe active window is available'
$passwordSuppressions = Count-Matching 'keyboard/clipboard content is suppressed because UIA password state is unavailable'
$acquisitionInitial = Count-Matching 'visible failure frame: initial privacy evaluation failed'
$acquisitionMonitor = Count-Matching 'visible failure frame: monitor acquisition failed'
$acquisitionActive = Count-Matching 'visible failure frame: active-window acquisition failed'
$acquisitionPost = Count-Matching 'visible failure frame: post-capture privacy evaluation failed'
$acquisitionFailures = $acquisitionInitial + $acquisitionMonitor + $acquisitionActive + $acquisitionPost
$queueOrLossNotices = Count-Matching 'queue.*(full|capacity|loss)|possible.loss|confirmed.loss|persistence.*degraded'
$audioTimingGaps = Count-Matching 'large gap on .* device:'
$audioRecoveryNotices = Count-Matching 'stream rebuild required after screen unlock|detected stale recording handle'
$healthTimeouts = Count-Matching 'health_check: inner computation exceeded'
$smartPiiFallbacks = Count-Matching 'Smart text-PII unavailable'
$basicPiiInfo = Count-Matching 'Basic PII redaction applied to captured frame text or metadata'
$negativeMeetingInfo = Count-Matching 'meeting scanner: .* in_call=false'
$cleanShutdownInLog = (Count-Matching 'screenpipe:\s+shutdown complete\s*$') -gt 0

# Derive privacy-pause intervals only from fixed, content-free lock notices.
$lockNotices = @()
foreach ($line in $logLines) {
    if ($line -match '^(?<timestamp>\S+).*reason_code="(?<reason>wts_session_locked|wts_session_unlocked|wts_session_disconnected|input_desktop_unavailable|process_session_query_failed|wts_query_failed|wts_short_buffer|wts_unsupported_level|wts_session_mismatch|wts_session_state_unknown|wts_session_flags_unknown|wts_session_flags_invalid)"') {
        $lockNotices += [pscustomobject]@{
            At = Convert-RecorderTimestampToUtc -Timestamp $Matches.timestamp -LegacyLocalClock $legacyLocalClock
            Reason = $Matches.reason
            Locked = $Matches.reason -ne 'wts_session_unlocked'
        }
    }
}
$pauseIntervals = @()
$pauseStart = $null
$lastLockReason = $null
foreach ($notice in $lockNotices | Sort-Object At) {
    $lastLockReason = $notice.Reason
    if ($notice.Locked -and $null -eq $pauseStart) {
        $pauseStart = $notice.At
    }
    elseif (-not $notice.Locked -and $null -ne $pauseStart) {
        $pauseIntervals += [pscustomobject]@{ Start = $pauseStart; End = $notice.At }
        $pauseStart = $null
    }
}
$trialEnd = if ($launcherExit) {
    ([datetimeoffset]$launcherExit.ended_at_utc).ToUniversalTime()
}
else {
    ([datetimeoffset]$latest.sampled_at_utc).ToUniversalTime()
}
if ($null -ne $pauseStart) {
    $pauseIntervals += [pscustomobject]@{ Start = $pauseStart; End = $trialEnd }
}
$lockPauseSeconds = 0.0
$framesAdvancedWhileLocked = 0L
$audioChunksAdvancedWhileLocked = 0L
foreach ($interval in $pauseIntervals) {
    $lockPauseSeconds += ($interval.End - $interval.Start).TotalSeconds
    $inside = @($samples | Where-Object {
        $sampled = ([datetimeoffset]$_.sampled_at_utc).ToUniversalTime()
        $sampled -ge $interval.Start -and $sampled -lt $interval.End
    })
    if ($inside.Count -ge 2) {
        $framesAdvancedWhileLocked +=
            (Number-OrZero $inside[-1].health.pipeline.frames_captured) -
            (Number-OrZero $inside[0].health.pipeline.frames_captured)
        $audioChunksAdvancedWhileLocked +=
            (Number-OrZero $inside[-1].health.audio_pipeline.chunks_received) -
            (Number-OrZero $inside[0].health.audio_pipeline.chunks_received)
    }
}

$confirmedDeliveryLoss = Number-OrZero $capture.event_delivery.dropped_events
$possibleDeliveryLoss = 0L
foreach ($queue in @($capture.audio_delivery.queues)) {
    $confirmedDeliveryLoss += Number-OrZero $queue.dropped_deliveries
    $possibleDeliveryLoss += Number-OrZero $queue.possible_lost_deliveries
}
$attentionRequired = $errorLines.Count -gt 0 -or
    (Number-OrZero $pipeline.frames_dropped) -gt 0 -or
    (Number-OrZero $pipeline.pipeline_stall_count) -gt 0 -or
    (Number-OrZero $pipeline.frame_link_ttl_evictions) -gt 0 -or
    (Number-OrZero $pipeline.frame_link_updates_failed) -gt 0 -or
    (Number-OrZero $audio.transcription_errors) -gt 0 -or
    [bool]$capture.persistence_degraded -or
    [bool]$capture.audio_shutdown_degraded -or
    $acquisitionFailures -gt 0 -or
    $queueOrLossNotices -gt 0 -or
    $confirmedDeliveryLoss -gt 0 -or
    $possibleDeliveryLoss -gt 0 -or
    ($finalAudit -and (Number-OrZero $finalAudit.scoped_process_count) -gt 0)
$assessment = if ($attentionRequired) {
    'attention required: one or more failure, expiry or loss counters are nonzero'
}
elseif ($null -ne $launcherExit -and -not $cleanShutdownInLog) {
    'trial stopped without a clean-shutdown marker in the scoped log interval'
}
elseif ([bool]$audio.transcription_available -and (Number-OrZero $audio.transcriptions_completed) -eq 0) {
    'no confirmed loss; audio was available but no current-process transcription completed'
}
else {
    'no confirmed delivery loss, persistence degradation or leftover scoped process'
}

$startedAt = ([datetimeoffset]$launch.started_at_utc).ToUniversalTime()
$status = [ordered]@{
    schema = 'screenwise.day-to-day-trial-status.v2'
    session = $session.Name
    trial_running = $null -eq $launcherExit
    started_at_utc = $startedAt.ToString('o')
    ended_or_latest_at_utc = $trialEnd.ToString('o')
    duration_seconds = [math]::Round(($trialEnd - $startedAt).TotalSeconds, 1)
    sample_count = $samples.Count
    latest_sample_utc = ([datetimeoffset]$latest.sampled_at_utc).ToUniversalTime().ToString('o')
    sample_age_seconds = [math]::Round(((Get-Date).ToUniversalTime() - ([datetime]$latest.sampled_at_utc).ToUniversalTime()).TotalSeconds, 1)
    api_reachable = [bool]$latest.api.reachable
    health_status = $latest.health.status
    frame_status = $latest.health.frame_status
    audio_status = $latest.health.audio_status
    frames_captured = $pipeline.frames_captured
    frames_db_written = $pipeline.frames_db_written
    frames_dropped = $pipeline.frames_dropped
    pipeline_stalls = $pipeline.pipeline_stall_count
    frame_links_emitted = $pipeline.frame_links_emitted
    frame_link_ttl_evictions = $pipeline.frame_link_ttl_evictions
    frame_link_ttl_events_without_frames = $pipeline.frame_link_ttl_events_without_frames
    frame_link_ttl_frames_without_events = $pipeline.frame_link_ttl_frames_without_events
    frame_link_update_failures = $pipeline.frame_link_updates_failed
    audio_chunks_received = $audio.chunks_received
    vad_passed = $audio.vad_passed
    vad_rejected = $audio.vad_rejected
    transcriptions_completed_current_process = $audio.transcriptions_completed
    transcription_errors = $audio.transcription_errors
    audio_db_inserted_current_process = $audio.db_inserted
    pending_transcription_segments_in_store = $audio.pending_transcription_segments
    transcription_requested = $audio.transcription_requested
    transcription_available = $audio.transcription_available
    persistence_degraded = $capture.persistence_degraded
    audio_shutdown_degraded = $capture.audio_shutdown_degraded
    confirmed_dropped_deliveries = $confirmedDeliveryLoss
    possible_lost_deliveries = $possibleDeliveryLoss
    lock_pause_count = $pauseIntervals.Count
    lock_pause_seconds = [math]::Round($lockPauseSeconds, 1)
    lock_state_at_end = if ($null -ne $pauseStart) { 'locked' } else { 'unlocked' }
    last_lock_reason = $lastLockReason
    frames_advanced_inside_locked_sample_windows = $framesAdvancedWhileLocked
    audio_chunks_advanced_inside_locked_sample_windows = $audioChunksAdvancedWhileLocked
    clean_shutdown_in_scoped_log = $cleanShutdownInLog
    launcher_clean_shutdown_observed = if ($launcherExit) { [bool]$launcherExit.clean_shutdown_observed } else { $null }
    recorder_exit_code = if ($launcherExit) { $launcherExit.recorder_exit_code } else { $null }
    scoped_processes_remaining = if ($finalAudit) { $finalAudit.scoped_process_count } else { $null }
    panic_log_file_count = if ($finalAudit) { $finalAudit.panic_log_file_count } else { $null }
    attention_required = $attentionRequired
    assessment = $assessment
    log_errors = $errorLines.Count
    log_warnings = $warningLines.Count
    excluded_background_transitions = $excludedBackground
    excluded_foreground_transitions = $excludedForeground
    no_safe_window_transitions = $noSafeWindow
    password_state_unavailable_suppressions = $passwordSuppressions
    acquisition_failure_placeholders = $acquisitionFailures
    acquisition_failure_initial_privacy = $acquisitionInitial
    acquisition_failure_monitor = $acquisitionMonitor
    acquisition_failure_active_window = $acquisitionActive
    acquisition_failure_post_capture_privacy = $acquisitionPost
    queue_capacity_or_loss_notices = $queueOrLossNotices
    audio_timing_gap_notices = $audioTimingGaps
    expected_unlock_audio_recovery_notices = $audioRecoveryNotices
    health_timeout_notices = $healthTimeouts
    smart_pii_reduced_coverage_notices = $smartPiiFallbacks
    routine_basic_pii_info_lines = $basicPiiInfo
    routine_negative_meeting_info_lines = $negativeMeetingInfo
}

if ($AsJson) {
    [pscustomobject]$status | ConvertTo-Json -Depth 6
}
else {
    [pscustomobject]$status | Format-List
}
