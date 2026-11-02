# SPDX-License-Identifier: GPL-3.0-or-later
# Release-only, sequential frontend scenarios. No output fallback. 30 minutes EACH by default.
param([ValidateRange(5,3600)][int]$Seconds=1800,[string]$Output='HyperX',[string]$Run='soak',[ValidateSet('both','dawj','dawj-tauri')][string]$Frontend='both')
$ErrorActionPreference='Stop'
$workspacePath=Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $workspacePath
$oldOutput=$env:DAWJ_TEST_OUTPUT; $oldSeconds=$env:DAWJ_TEST_SECONDS
try {
 $env:DAWJ_TEST_OUTPUT=$Output; $env:DAWJ_TEST_SECONDS="$Seconds"
 $resultPath=Join-Path $workspacePath "target/web4-$Run"
 New-Item -ItemType Directory -Path $resultPath -Force | Out-Null
 foreach($name in @('dawj-tauri','dawj') | Where-Object {$Frontend -eq 'both' -or $_ -eq $Frontend}) {
  $stderrPath=Join-Path $resultPath "$name.stderr.log"
  $process=Start-Process -FilePath (Join-Path $workspacePath "target/release/$name.exe") -ArgumentList '--reliability' -WindowStyle Hidden -RedirectStandardOutput (Join-Path $resultPath "$name.stdout.log") -RedirectStandardError $stderrPath -PassThru
  $processHandle=$process.Handle; $watch=[Diagnostics.Stopwatch]::StartNew(); $samples=@()
  while(-not $process.WaitForExit(5000)) {
   if($watch.Elapsed.TotalSeconds -gt $Seconds+90) {Stop-Process -Id $process.Id; throw "$name exceeded timeout; only test-owned process stopped"}
   $all=Get-CimInstance Win32_Process
   $ids=@($process.Id); do {$previous=$ids.Count; $ids+=@($all | Where-Object {$_.ParentProcessId -in $ids -and $_.ProcessId -notin $ids} | ForEach-Object {$_.ProcessId})} while($ids.Count -gt $previous)
   $privateBytes=0L; $working=0L
   foreach($idValue in $ids){$p=Get-Process -Id $idValue -ErrorAction SilentlyContinue; if($p){$privateBytes+=$p.PrivateMemorySize64;$working+=$p.WorkingSet64}}
   $samples+= [pscustomobject]@{seconds=$watch.Elapsed.TotalSeconds;private_bytes=$privateBytes;working_set=$working;processes=$ids.Count}
   if($samples.Count%12 -eq 0){Write-Output "$name progress $([int]$watch.Elapsed.TotalSeconds)/$Seconds seconds; tree private $([int]($privateBytes/1MB)) MiB"}
  }
  $samples | Export-Csv -NoTypeInformation -LiteralPath (Join-Path $resultPath "$name.memory.csv")
  $line=Get-Content -LiteralPath $stderrPath | Where-Object {$_ -match '^(TAURI_METRICS|RELIABILITY_METRICS) ' } | Select-Object -Last 1
  if(-not $line){throw "$name missing metrics: $(Get-Content -LiteralPath $stderrPath -Raw)"}
  $metrics=($line -replace '^[A-Z_]+ ','')|ConvertFrom-Json
  $metrics | ConvertTo-Json -Depth 12 | Set-Content -Encoding UTF8 -LiteralPath (Join-Path $resultPath "$name.metrics.json")
  if($process.ExitCode -ne 0 -or -not $metrics.passed -or $metrics.error){throw "$name failed: $line"}
  Write-Output $line
 }
} finally {$env:DAWJ_TEST_OUTPUT=$oldOutput;$env:DAWJ_TEST_SECONDS=$oldSeconds;Pop-Location}
