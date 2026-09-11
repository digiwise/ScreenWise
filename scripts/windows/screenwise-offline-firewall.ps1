#Requires -Version 5.1
#Requires -RunAsAdministrator

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('Install', 'Status', 'Remove')]
    [string]$Action,

    [string]$Executable
)

$ErrorActionPreference = 'Stop'
$RuleName = 'ScreenWise Offline Runtime Validation'

function Get-ValidationRule {
    @(Get-NetFirewallRule -DisplayName $RuleName -ErrorAction SilentlyContinue)
}

function Write-RuleStatus {
    param([Parameter(Mandatory = $true)]$Rule)

    $application = $Rule | Get-NetFirewallApplicationFilter
    [ordered]@{
        displayName = $Rule.DisplayName
        enabled = $Rule.Enabled.ToString()
        direction = $Rule.Direction.ToString()
        action = $Rule.Action.ToString()
        profile = $Rule.Profile.ToString()
        program = $application.Program
    } | ConvertTo-Json
}

switch ($Action) {
    'Install' {
        if ([string]::IsNullOrWhiteSpace($Executable)) {
            $repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
            $Executable = Join-Path $repoRoot 'target\release\screenpipe.exe'
        }
        $Executable = (Resolve-Path -LiteralPath $Executable).Path

        $existing = Get-ValidationRule
        if ($existing.Count -gt 1) {
            throw "Multiple firewall rules named '$RuleName' exist. Remove them explicitly before validation."
        }
        if ($existing.Count -eq 1) {
            $application = $existing[0] | Get-NetFirewallApplicationFilter
            if ($application.Program -ine $Executable -or
                $existing[0].Enabled.ToString() -ne 'True' -or
                $existing[0].Direction.ToString() -ne 'Outbound' -or
                $existing[0].Action.ToString() -ne 'Block' -or
                $existing[0].Profile.ToString() -ne 'Any') {
                throw "Existing '$RuleName' rule is not the required enabled, any-profile outbound block for '$Executable'. Run -Action Remove first."
            }
            Write-RuleStatus -Rule $existing[0]
            break
        }

        $rule = New-NetFirewallRule `
            -DisplayName $RuleName `
            -Description 'Temporary exact-program outbound block for ScreenWise offline validation.' `
            -Direction Outbound `
            -Program $Executable `
            -Action Block `
            -Profile Any `
            -Enabled True
        Write-RuleStatus -Rule $rule
    }
    'Status' {
        $rules = Get-ValidationRule
        if ($rules.Count -ne 1) {
            throw "Expected exactly one '$RuleName' rule; found $($rules.Count)."
        }
        Write-RuleStatus -Rule $rules[0]
    }
    'Remove' {
        $rules = Get-ValidationRule
        if ($rules.Count -eq 0) {
            [ordered]@{ displayName = $RuleName; removed = $false; reason = 'not found' } | ConvertTo-Json
            break
        }
        $rules | Remove-NetFirewallRule
        [ordered]@{ displayName = $RuleName; removed = $true; count = $rules.Count } | ConvertTo-Json
    }
}
