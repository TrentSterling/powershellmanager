# Install the current release at a stable per-user path and register sign-in.
[CmdletBinding()]
param([string]$SourceExe)
$ErrorActionPreference = 'Stop'
if (!$SourceExe) { $SourceExe = Join-Path $PSScriptRoot '../target-publish/release/powershellmanager.exe' }
$SourceExe = (Resolve-Path -LiteralPath $SourceExe).Path
$installDir = Join-Path $env:LOCALAPPDATA 'Programs/PowerShellManager'
$installedExe = Join-Path $installDir 'powershellmanager.exe'
$launcher = Join-Path $installDir 'start-at-login.ps1'
New-Item -ItemType Directory -Path $installDir -Force | Out-Null
if ((Get-Process -Name powershellmanager -ErrorAction SilentlyContinue | Where-Object Path -EQ $installedExe)) {
    if ((Get-FileHash -LiteralPath $installedExe).Hash -ne (Get-FileHash -LiteralPath $SourceExe).Hash) {
        throw 'The installed app is running. Quit it before replacing its executable.'
    }
} else {
    Copy-Item -LiteralPath $SourceExe -Destination $installedExe -Force
}
if ((Get-FileHash -LiteralPath $installedExe).Hash -ne (Get-FileHash -LiteralPath $SourceExe).Hash) { throw 'Installed executable does not match the release' }
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'start-at-login.ps1') -Destination $launcher -Force
$user = [Security.Principal.WindowsIdentity]::GetCurrent().Name
$action = New-ScheduledTaskAction -Execute (Join-Path $env:WINDIR 'System32/WindowsPowerShell/v1.0/powershell.exe') -Argument ('-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File "{0}"' -f $launcher) -WorkingDirectory $installDir
$trigger = New-ScheduledTaskTrigger -AtLogOn -User $user
$trigger.Delay = 'PT10S'
$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable -MultipleInstances IgnoreNew -ExecutionTimeLimit ([TimeSpan]::Zero) -RestartCount 10 -RestartInterval (New-TimeSpan -Minutes 1)
Register-ScheduledTask -TaskName 'PowerShellManager' -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Description 'Open PowerShellManager at Windows sign-in; skip launch when it is already running.' -Force | Out-Null
Write-Output "PowerShellManager installed at $installedExe; sign-in task registered for $user."
