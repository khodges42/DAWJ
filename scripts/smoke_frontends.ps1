# SPDX-License-Identifier: GPL-3.0-or-later
# Run after cargo build -p dawj -p dawj-tauri --locked. Opens native windows/audio.
$ErrorActionPreference = 'Stop'
$workspacePath = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $workspacePath
try {
    foreach ($test in @(
        @{Name='dawj-tauri'; Argument='--smoke'},
        @{Name='dawj'; Argument='--smoke-test'}
    )) {
        $name = $test.Name
        $application = Join-Path $workspacePath "target/debug/$name.exe"
        $stdoutPath = Join-Path $workspacePath "target/web1-$name.stdout.log"
        $stderrPath = Join-Path $workspacePath "target/web1-$name.stderr.log"
        $smokeProcess = Start-Process -FilePath $application -ArgumentList $test.Argument -WindowStyle Hidden -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath -PassThru
        # Retain the OS handle before exit; Windows PowerShell can otherwise lose ExitCode.
        $smokeHandle = $smokeProcess.Handle
        if (-not $smokeProcess.WaitForExit(30000)) {
            # Only terminate the process launched above by this test.
            Stop-Process -Id $smokeProcess.Id
            throw "$name native smoke exceeded 30 seconds"
        }
        $smokeProcess.Refresh()
        $stderr = Get-Content -LiteralPath $stderrPath -Raw
        if ($null -eq $smokeProcess.ExitCode -or $smokeProcess.ExitCode -ne 0) { throw "$name exited $($smokeProcess.ExitCode): $stderr" }
        if ($name -eq 'dawj-tauri') {
            $metricLine = Get-Content -LiteralPath $stderrPath | Where-Object { $_.StartsWith('TAURI_METRICS ') } | Select-Object -First 1
            if (-not $metricLine) { throw 'Missing Tauri native smoke metrics' }
            $metrics = $metricLine.Substring(14) | ConvertFrom-Json
            if ($metrics.error -or -not $metrics.clean_audio_shutdown -or -not $metrics.independent_stop) { throw "Tauri smoke failed: $metricLine" }
            Write-Output $metricLine
        }
        Write-Output "$name native smoke passed"
    }
} finally { Pop-Location }
