# Measures recognition time of the FIRST dictation after the app starts, using the installed
# app and a separate throwaway data folder (the owner's history is not touched). Nothing is
# pasted anywhere. Synthetic clips: speed only, not accuracy.
param(
  [string]$Exe = "$env:LOCALAPPDATA\Bolkar\app-desktop.exe",
  [string]$WorkDir = 'D:\dev\selftest2',
  [string[]]$Clips = @('tts_en_8s', 'tts_en_14s', 'tts_en_20s')
)
$ErrorActionPreference = 'Stop'
foreach ($clip in $Clips) {
  Get-Process app-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
  Start-Sleep -Milliseconds 800
  $out = Join-Path $WorkDir 'out.json'
  if (Test-Path $out) { Remove-Item $out }
  $env:APP_DATA_DIR = Join-Path $WorkDir 'data'
  $env:APP_SELFTEST_WAV = "D:\bolkar\bench\audio_cache\$clip.wav"
  $env:APP_SELFTEST_OUT = $out
  $env:APP_SELFTEST_INSERT = '0'
  $q = Start-Process $Exe -PassThru
  if (-not $q.WaitForExit(120000)) { Stop-Process -Id $q.Id -Force; "$clip TIMEOUT"; continue }
  $r = Get-Content $out -Raw | ConvertFrom-Json
  "$clip | first dictation after start: recognition $($r.result.transcribe_ms) ms | $($r.result.engine)"
  "    final: $($r.result.final_text -replace "`n", ' / ')"
}
$env:APP_SELFTEST_WAV = $null
$env:APP_DATA_DIR = $null
$env:APP_SELFTEST_OUT = $null
$env:APP_SELFTEST_INSERT = $null
