# SPDX-License-Identifier: GPL-3.0-or-later
# Local release-profile build/validation staging, not a redistribution clearance.
param([switch]$Offline,[switch]$Smoke,[string]$Output='HyperX')
$ErrorActionPreference='Stop'
$workspacePath=Split-Path -Parent $PSScriptRoot
function Invoke-Checked([string]$Program,[string[]]$Arguments) {
 & $Program @Arguments
 if($LASTEXITCODE -ne 0){throw "$Program failed: $Arguments"}
}
Push-Location -LiteralPath $workspacePath
try {
 if([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT){throw 'This procedure requires Windows x64 MSVC'}
 $compiler=(& rustc -vV) -join "`n"
 if($LASTEXITCODE -ne 0 -or $compiler -notmatch 'release: 1\.93\.0\s' -or $compiler -notmatch 'host: x86_64-pc-windows-msvc'){throw 'Use the pinned Rust 1.93.0 x86_64-pc-windows-msvc toolchain'}
 $locked=@('--locked');if($Offline){$locked+='--offline'}
 if(-not $Offline){Invoke-Checked cargo @('fetch','--locked','--target','x86_64-pc-windows-msvc')}
 Invoke-Checked cargo @('fmt','--all','--','--check')
 Invoke-Checked cargo (@('check','--workspace')+$locked)
 Invoke-Checked cargo (@('clippy','--workspace','--all-targets')+$locked+@('--','-D','warnings'))
 Invoke-Checked cargo (@('test','--workspace')+$locked)
 Invoke-Checked node @('--test','crates/tauri-app/web/model.test.mjs','crates/tauri-app/web/protocol.test.mjs','crates/tauri-app/web/performance.test.mjs','crates/tauri-app/web/skins.test.mjs','crates/tauri-app/web/reliability.test.mjs')
 foreach($check in @('scripts/audio_dependencies.py','scripts/test_web4_report.py','scripts/test_alpha_layout.py','scripts/check_docs.py')){Invoke-Checked python @($check)}
 Invoke-Checked cargo (@('run','-p','dawj-core','--example','web_contract')+$locked+@('--','--check'))
 $targetPath=Join-Path $workspacePath 'target'
 Invoke-Checked cargo (@('build','--release','--target-dir',$targetPath,'-p','dawj','-p','dawj-egui')+$locked)
 $revision=(& git -c "safe.directory=$workspacePath" rev-parse HEAD).Trim();if($LASTEXITCODE -ne 0){throw 'Cannot record source revision'}
 & git -c "safe.directory=$workspacePath" diff --quiet HEAD
 $trackedDirty=$LASTEXITCODE -ne 0
 $stagePath=Join-Path $targetPath ("alpha-windows/"+[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss')+'-'+$revision.Substring(0,8))
 if(Test-Path -LiteralPath $stagePath){throw 'Staging directory already exists; nothing overwritten'}
 New-Item -ItemType Directory -Path $stagePath | Out-Null
 $artifacts=@()
 foreach($name in @('dawj','dawj-egui')){
  $binary=Join-Path $stagePath "$name.exe"
  Copy-Item -LiteralPath (Join-Path $targetPath "release/$name.exe") -Destination $binary
  $artifacts+=[pscustomobject]@{name="$name.exe";sha256=(Get-FileHash -Algorithm SHA256 -LiteralPath $binary).Hash}
 }
 foreach($file in @('LICENSE','NOTICE')){Copy-Item -LiteralPath (Join-Path $workspacePath $file) -Destination $stagePath}
 Copy-Item -LiteralPath 'docs/development/windows-dependencies.json' -Destination $stagePath
 @'
DAWJ DEVELOPMENT ALPHA - local release-profile validation build
Launch dawj.exe (Tauri); dawj-egui.exe is the fallback.
Windows x64 native runtimes and Microsoft WebView2 are prerequisites.
Native Appearance > Restore default interface / Ctrl+Alt+Shift+R resets skins.
dawj.exe --safe-mode ignores the saved skin; --reset-skin clears local skin data.
No live-show certification, signed installer or redistribution clearance is implied.
Before sharing binaries, review corresponding source, dependency/font notices,
native-runtime requirements and original-design asset rights in the repository's
docs/development/licensing.md and docs/development/windows-alpha.md.
'@ | Set-Content -Encoding UTF8 -LiteralPath (Join-Path $stagePath 'README-ALPHA.txt')
 if($Smoke){
  $oldOutput=$env:DAWJ_TEST_OUTPUT
  try{$env:DAWJ_TEST_OUTPUT=$Output;& "$PSScriptRoot/smoke_frontends.ps1" -Configuration Release -BinaryDirectory $stagePath}
  finally{$env:DAWJ_TEST_OUTPUT=$oldOutput}
 }
 [ordered]@{status='development-alpha';source_revision=$revision;tracked_dirty=$trackedDirty;compiler=$compiler;validation_passed=$true;native_smoke_passed=[bool]$Smoke;redistribution_cleared=$false;artifacts=$artifacts} |
  ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 -LiteralPath (Join-Path $stagePath 'build-manifest.json')
 Write-Output "Local alpha build: $stagePath"
} finally {Pop-Location}
