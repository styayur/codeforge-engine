param([Parameter(Mandatory)][string]$Binary, [Parameter(Mandatory)][string]$Output)
$ErrorActionPreference = 'Stop'
$binaryPath = (Resolve-Path -LiteralPath $Binary).Path
$fixture = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../fixtures/python')).Path
$samples = @()
$peak = 0L
for ($i = 0; $i -lt 33; $i++) {
    $info = [Diagnostics.ProcessStartInfo]::new($binaryPath)
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($arg in @('scan', $fixture, '--json')) { $info.ArgumentList.Add($arg) }
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $process = [Diagnostics.Process]::Start($info)
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    while (-not $process.HasExited) {
        $process.Refresh()
        $peak = [Math]::Max($peak, $process.PeakWorkingSet64)
        Start-Sleep -Milliseconds 5
    }
    $process.WaitForExit()
    if ($process.ExitCode -ne 0) { throw $stderr.Result }
    if ($i -ge 3) { $samples += $timer.Elapsed.TotalMilliseconds }
    $last = $stdout.Result | ConvertFrom-Json
    $process.Dispose()
}
$sorted = @($samples | Sort-Object)
@{
    samples_ms = $samples
    mean_ms = ($samples | Measure-Object -Average).Average
    p95_ms = $sorted[[Math]::Ceiling($sorted.Count * 0.95) - 1]
    peak_working_set_bytes = $peak
    binary_bytes = (Get-Item -LiteralPath $binaryPath).Length
    fixture = 'fixtures/python'
    command = 'scan --json'
    result = @{ languages = $last.workspace.languages; engine_count = @($last.engines).Count; available_engines = @($last.engines | Where-Object available | ForEach-Object { $_.metadata.id }) }
} | ConvertTo-Json -Depth 30 | Set-Content -LiteralPath $Output
