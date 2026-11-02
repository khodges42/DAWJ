# SPDX-License-Identifier: GPL-3.0-or-later
param([ValidateRange(30,300)][int]$Seconds=60,[string]$Output='HyperX')
$ErrorActionPreference='Stop'
$workspacePath=Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $workspacePath
try {
 $preflight=@()
 foreach($sample in 1..3){
  $games=@(Get-Process | Where-Object {$_.ProcessName -match '^(cod|cod22|cod23|cod24|codhq|modernwarfare|blackops.*)$'})
  if($games){throw 'A game process is present. No application was closed; rerun with a quiet desktop.'}
  $cpu=(Get-CimInstance Win32_PerfFormattedData_PerfOS_Processor | Where-Object Name -eq '_Total').PercentProcessorTime
  $gpu=$null;try{$gpu=(Get-CimInstance Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine -ErrorAction Stop | Measure-Object UtilizationPercentage -Maximum).Maximum}catch{}
  $preflight+=[pscustomobject]@{utc=[DateTime]::UtcNow.ToString('o');cpu_percent=$cpu;gpu_busiest_engine_percent=$gpu}
  Start-Sleep -Seconds 3
 }
 $preflight | ConvertTo-Json | Set-Content -Encoding UTF8 target/web4_1-preflight.json
 if(($preflight | Measure-Object cpu_percent -Average).Average -gt 20 -or ($preflight | Measure-Object gpu_busiest_engine_percent -Maximum).Maximum -gt 30){throw 'Host preflight is busy; comparison deferred. See target/web4_1-preflight.json.'}
 foreach($trial in 1..3){
  $frontends=if($trial%2){@('dawj-tauri','dawj')}else{@('dawj','dawj-tauri')}
  foreach($frontend in $frontends){& "$PSScriptRoot/web4_reliability.ps1" -Seconds $Seconds -Output $Output -Run "isolated-$trial" -Frontend $frontend -ObserveHost}
 }
 & "$PSScriptRoot/web4_reliability.ps1" -Seconds $Seconds -Output $Output -Run isolated-profile -Frontend dawj-tauri -ObserveHost -Profile
} finally {Pop-Location}
