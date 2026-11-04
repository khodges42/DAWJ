# SPDX-License-Identifier: GPL-3.0-or-later
# Fresh local fixtures and two actual application processes. HyperX only.
param([ValidateSet('Debug','Release')][string]$Configuration='Debug',[ValidateSet('1','2','3')][string]$Milestone='1')
$ErrorActionPreference='Stop'
$workspacePath=Split-Path -Parent $PSScriptRoot
$runPath=Join-Path $workspacePath ("target/jam1-"+[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss-fff'))
New-Item -ItemType Directory -Path $runPath | Out-Null
$previousMilestone=$env:DAWJ_JAM_MILESTONE
$env:DAWJ_JAM_MILESTONE=$Milestone
$previousRoot=$env:DAWJ_JAM_ROOT
$previousPhase=$env:DAWJ_JAM_PHASE
try {
    $env:DAWJ_JAM_ROOT=$runPath
    # Actual PCM WAV media, independently prepared at 44.1/48 kHz; no bundled demo.
    @'
import os,math,wave,struct
from pathlib import Path
root=Path(os.environ['DAWJ_JAM_ROOT'])
for filename,rate,channels,hz in [('my-percussion.wav',44100,1,190),('my-tone.wav',48000,2,330)]:
    with wave.open(str(root/filename),'wb') as f:
        f.setnchannels(channels); f.setsampwidth(2); f.setframerate(rate)
        f.writeframes(b''.join(struct.pack('<h',int(6000*math.sin(2*math.pi*hz*i/rate)))*channels for i in range(rate*2)))
'@ | python -
    if ($LASTEXITCODE -ne 0){throw 'WAV fixture generation failed'}
    $application=Join-Path $workspacePath ("target/"+$Configuration.ToLowerInvariant()+"/dawj.exe")
    $reports=@()
    foreach($phase in @('create','reopen')){
        $env:DAWJ_JAM_PHASE=$phase
        $stdoutPath=Join-Path $runPath "$phase.stdout.log"
        $stderrPath=Join-Path $runPath "$phase.stderr.log"
        $testProcess=Start-Process -FilePath $application -ArgumentList '--jam-smoke' -WindowStyle Hidden -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath -PassThru
        $testHandle=$testProcess.Handle
        if(-not $testProcess.WaitForExit(90000)){Stop-Process -Id $testProcess.Id;throw "Jam $phase exceeded 90 seconds"}
        $testProcess.Refresh()
        $line=Get-Content -LiteralPath $stderrPath | Where-Object {$_.StartsWith('TAURI_METRICS ')} | Select-Object -First 1
        if(-not $line -or $testProcess.ExitCode -ne 0){throw "Jam $phase failed: $(Get-Content -LiteralPath $stderrPath -Raw)"}
        $metrics=$line.Substring(14)|ConvertFrom-Json
        if($metrics.error -or -not $metrics.passed -or -not $metrics.real_audio -or -not $metrics.clean_audio_shutdown){throw $line}
        $reports+=$metrics
        Write-Output $line
    }
    if($reports[0].project -ne $reports[1].project -or ($reports[0].assignments -join ',') -ne ($reports[1].assignments -join ',')){throw 'Fresh process did not restore stable identities'}
    $reports | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $runPath 'results.json') -Encoding UTF8
    Write-Output "Jam$Milestone real application save/close/reopen passed: $runPath"
} finally {
    $env:DAWJ_JAM_MILESTONE=$previousMilestone
    $env:DAWJ_JAM_ROOT=$previousRoot
    $env:DAWJ_JAM_PHASE=$previousPhase
}
