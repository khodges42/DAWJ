# SPDX-License-Identifier: GPL-3.0-or-later
# Run after cargo build -p dawj -p dawj-egui --locked. Opens native windows/audio.
param([ValidateSet('Debug','Release')][string]$Configuration='Debug',[string]$BinaryDirectory)
$ErrorActionPreference = 'Stop'
$workspacePath = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $workspacePath
try {
    foreach ($test in @(
        @{Name='dawj'; Argument='--smoke'},
        @{Name='dawj-egui'; Argument='--smoke-test'},
        @{Name='dawj'; Argument='--safe-mode --smoke'; Log='dawj-safe'; Safe=$true}
    )) {
        $name = $test.Name
        $binaryPath = if($BinaryDirectory){$BinaryDirectory}else{Join-Path $workspacePath ("target/"+$Configuration.ToLowerInvariant())}
        $application = Join-Path $binaryPath "$name.exe"
        $logName = if($test.Log){$test.Log}else{$name}
        $stdoutPath = Join-Path $workspacePath "target/web1-$logName.stdout.log"
        $stderrPath = Join-Path $workspacePath "target/web1-$logName.stderr.log"
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
        if ($name -eq 'dawj') {
            $metricLine = Get-Content -LiteralPath $stderrPath | Where-Object { $_.StartsWith('TAURI_METRICS ') } | Select-Object -First 1
            if (-not $metricLine) { throw 'Missing Tauri native smoke metrics' }
            $metrics = $metricLine.Substring(14) | ConvertFrom-Json
            if ($metrics.error -or -not $metrics.clean_audio_shutdown -or -not $metrics.independent_stop -or -not $metrics.skin_recovery -or -not $metrics.software_restart) { throw "Tauri smoke failed: $metricLine" }
            if ($test.Safe -and -not $metrics.safe_startup) { throw 'Default app safe-mode startup failed' }
            Write-Output $metricLine
        }
        Write-Output "$name native smoke passed"
    }
} finally { Pop-Location }
