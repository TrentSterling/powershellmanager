param(
    [switch]$EnforceFullCoverage,
    [switch]$BranchCoverage,
    [string]$CoverageTool,
    [string]$CoverageToolchain = 'nightly'
)

$ErrorActionPreference = 'Stop'
if ($EnforceFullCoverage) { $BranchCoverage = $true }
$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (-not $CoverageTool) {
    $installed = Get-Command cargo-llvm-cov -ErrorAction SilentlyContinue
    $CoverageTool = if ($installed) { $installed.Source } else {
        Join-Path $repoRoot '../tmp/psm-coverage-tools/bin/cargo-llvm-cov.exe'
    }
}
if (-not (Test-Path -LiteralPath $CoverageTool)) {
    throw 'Install cargo-llvm-cov 0.9.1 and rustup component add llvm-tools-preview, or pass -CoverageTool.'
}

function Invoke-CoverageTool {
    param([string[]]$Arguments, [switch]$AllowFailure)
    # Windows PowerShell 5 treats redirected native stderr as non-terminating
    # errors, including compiler progress and warnings. The exit code is authoritative.
    $ErrorActionPreference = 'Continue'
    & $CoverageTool @Arguments
    $script:coverageExit = $LASTEXITCODE
    if ($script:coverageExit -ne 0 -and -not $AllowFailure) {
        throw "Coverage command failed (exit $script:coverageExit): $($Arguments -join ' ')"
    }
}

Push-Location -LiteralPath $repoRoot
$previousToolchain = $env:RUSTUP_TOOLCHAIN
try {
    if ($BranchCoverage) { $env:RUSTUP_TOOLCHAIN = $CoverageToolchain }
    $reportPath = Join-Path $repoRoot 'target/coverage.json'
    # Old instrumented executables otherwise remain in the report after source edits.
    Invoke-CoverageTool -Arguments @('llvm-cov', 'clean', '--workspace')
    # Recent nightly Cargo also keeps compiler outputs under build/<package>/<hash>/out.
    # Package clean can leave superseded executables there; llvm-cov scans those too.
    $coverageRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot 'target/llvm-cov-target'))
    $buildRoot = Join-Path $coverageRoot 'debug/build'
    if (Test-Path -LiteralPath $buildRoot) {
        Get-ChildItem -LiteralPath $buildRoot -Directory | Where-Object {
            $_.Name -eq 'powershellmanager' -or $_.Name.StartsWith('powershellmanager-')
        } | ForEach-Object {
            $cachePath = [System.IO.Path]::GetFullPath($_.FullName)
            if (-not $cachePath.StartsWith($coverageRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
                throw 'Refusing to clean a path outside the coverage workspace.'
            }
            Remove-Item -LiteralPath $cachePath -Recurse -Force
        }
    }
    # Nightly package-clean uses the new layout and can miss old stable deps binaries.
    foreach ($artifactRoot in @((Join-Path $coverageRoot 'debug'), (Join-Path $coverageRoot 'debug/deps'))) {
        if (Test-Path -LiteralPath $artifactRoot) {
            Get-ChildItem -LiteralPath $artifactRoot -File -Filter 'powershellmanager*' | ForEach-Object {
                $artifactPath = [System.IO.Path]::GetFullPath($_.FullName)
                if (-not $artifactPath.StartsWith($coverageRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
                    throw 'Refusing to remove an artifact outside the coverage workspace.'
                }
                Remove-Item -LiteralPath $artifactPath -Force
            }
        }
    }
    if (Test-Path -LiteralPath $reportPath) { Remove-Item -LiteralPath $reportPath }
    $testOnlyPattern = '(ui_tests|native_audit|action_tests|activity[/\\]tests|config[/\\]tests|persistence[/\\]tests|theme[/\\]tests|tray[/\\]tests|windows[/\\]tests|hotkey[/\\]tests|app[/\\]tests|updates[/\\]tests|monitor[/\\]tests|gui[/\\]preview[/\\]tests)'
    $coverageArgs = @(
        'llvm-cov', '--locked', '--offline', '--no-cfg-coverage',
        '--no-report'
    )
    if ($BranchCoverage) { $coverageArgs += @('--branch', '--no-cfg-coverage-nightly') }
    $coverageArgs += @('--', '--include-ignored', '--skip', 'render_public_gallery', '--test-threads=1')
    Invoke-CoverageTool -Arguments $coverageArgs
    foreach ($smoke in @(
        @{name='help'; arguments=@('--help')},
        @{name='version'; arguments=@('--version')},
        @{name='inventory-text'; arguments=@('--list', '--target', 'terminals')},
        @{name='inventory-json'; arguments=@('--list', '--json', '--target', 'terminals')}
    )) {
        $runArgs = @('llvm-cov', 'run', '--locked', '--offline', '--no-cfg-coverage', '--no-report')
        if ($BranchCoverage) { $runArgs += @('--branch', '--no-cfg-coverage-nightly') }
        $runArgs += @('--') + $smoke.arguments
        # Actual title metadata stays in ignored local artifacts, never public captures.
        Invoke-CoverageTool -Arguments $runArgs | Set-Content -LiteralPath (Join-Path $repoRoot ('target/cli-' + $smoke.name + '.txt')) -Encoding UTF8
    }
    $inventory = Get-Content -LiteralPath (Join-Path $repoRoot 'target/cli-inventory-json.txt') -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($inventory.count -ne @($inventory.windows).Count) { throw 'CLI JSON inventory count mismatch.' }
    $reportArgs = @('llvm-cov', 'report', '--json', '--output-path', $reportPath, '--ignore-filename-regex', $testOnlyPattern)
    if ($EnforceFullCoverage) {
        $reportArgs += @('--fail-under-lines', '100', '--fail-under-functions', '100', '--fail-under-regions', '100')
    }
    Invoke-CoverageTool -Arguments $reportArgs -AllowFailure
    if (-not (Test-Path -LiteralPath $reportPath)) {
        throw "Coverage did not produce a report (exit $coverageExit)."
    }
    $report = Get-Content -LiteralPath $reportPath -Raw -Encoding UTF8 | ConvertFrom-Json
    $totals = $report.data[0].totals
    $report.data[0].files | ForEach-Object {
        [PSCustomObject]@{
            File = $_.filename.Replace($repoRoot + '\', '')
            Lines = [Math]::Round($_.summary.lines.percent, 2)
            Uncovered = $_.summary.lines.count - $_.summary.lines.covered
            Functions = [Math]::Round($_.summary.functions.percent, 2)
            Regions = [Math]::Round($_.summary.regions.percent, 2)
        }
    } | Format-Table -AutoSize
    Write-Output ('TOTAL: {0:N2}% lines; {1:N2}% functions; {2:N2}% regions' -f
        $totals.lines.percent, $totals.functions.percent, $totals.regions.percent)
    Write-Output 'All production src files count. Only test-only modules are excluded; vendored dependencies are outside the app coverage scope.'
    if ($BranchCoverage) {
        Write-Output ('BRANCHES: {0:N2}% ({1}/{2})' -f $totals.branches.percent, $totals.branches.covered, $totals.branches.count)
    } else {
        Write-Output 'Branch coverage is not measured by this stable-toolchain run; it must not be reported as 100%.'
    }
    # cargo-llvm-cov 0.9.1 instruments branches but has no --fail-under-branches.
    if ($EnforceFullCoverage -and ($totals.branches.count -eq 0 -or $totals.branches.covered -ne $totals.branches.count)) {
        throw ('Full coverage requires every branch; measured {0}/{1}.' -f $totals.branches.covered, $totals.branches.count)
    }
    if ($coverageExit -ne 0) {
        throw "Coverage checks failed (exit $coverageExit). Inspect target/coverage.json."
    }
} finally {
    $env:RUSTUP_TOOLCHAIN = $previousToolchain
    Pop-Location
}
