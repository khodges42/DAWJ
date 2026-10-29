# SPDX-License-Identifier: GPL-3.0-or-later
# Visible native window capture; -Play explicitly opts into real demo audio.
param([switch]$Play)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$executable = Join-Path $workspace 'target/debug/dawj.exe'
if (-not (Test-Path -LiteralPath $executable)) { throw 'Build with cargo build -p dawj first.' }
$mode = if ($Play) { '--smoke-test' } else { '--smoke-window' }
$logPath = Join-Path $workspace 'target/m3-native-smoke.log'
$outPath = Join-Path $workspace 'target/m3-native-stdout.log'
# A visible test window is explicitly requested here. Captures come directly
# from eframe, never from the desktop or another application's windows.
$appProcess = Start-Process -FilePath $executable -ArgumentList $mode -WorkingDirectory $workspace -PassThru -RedirectStandardError $logPath -RedirectStandardOutput $outPath
$null = $appProcess.Handle
if (-not $appProcess.WaitForExit(20000)) { throw 'DAWJ did not close within 20 seconds; retained for inspection.' }
Get-Content -LiteralPath $logPath
if ($appProcess.ExitCode -ne 0) { throw "DAWJ smoke check failed: exit $($appProcess.ExitCode)" }
if (-not ((Get-Content -LiteralPath $logPath -Raw) -match 'SMOKE_RESULT passed=true')) { throw 'Missing smoke success record' }
Write-Output 'Native smoke passed; target/m3-native-capture.png contains only the DAWJ viewport.'
