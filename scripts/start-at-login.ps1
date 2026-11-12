# The scheduled task runs this helper hidden. Only the requested app opens.
$ErrorActionPreference = 'Stop'
$appPath = Join-Path $PSScriptRoot 'powershellmanager.exe'
$receiptPath = Join-Path $PSScriptRoot 'startup-last-run.json'
if (!(Test-Path -LiteralPath $appPath)) { throw "Missing application: $appPath" }
$gate = New-Object System.Threading.Mutex($false, 'Local\PowerShellManager-Startup')
try {
    if (!$gate.WaitOne(0)) { exit 0 }
    try {
        $runningApp = Get-Process -Name powershellmanager -ErrorAction SilentlyContinue | Select-Object -First 1
        $action = 'already-running'
        if (!$runningApp) {
            $runningApp = Start-Process -FilePath $appPath -WorkingDirectory $PSScriptRoot -PassThru
            Start-Sleep -Seconds 2
            $runningApp.Refresh()
            if ($runningApp.HasExited) { throw 'PowerShellManager exited during startup' }
            $action = 'started'
        }
        [ordered]@{
            timestampUtc = [DateTime]::UtcNow.ToString('o')
            action = $action
            processId = $runningApp.Id
            application = $appPath
        } | ConvertTo-Json | Set-Content -LiteralPath $receiptPath -Encoding UTF8
    } finally { $gate.ReleaseMutex() }
} finally { $gate.Dispose() }
