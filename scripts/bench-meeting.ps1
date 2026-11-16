# Meeting benchmark with the installed app and a THROWAWAY data folder (AI off: nothing is
# uploaded). Plays a synthetic meeting through the speakers while recording, then scores the
# transcript of the computer-sound track against the script.
param(
  [switch]$WithMic,
  [string]$Meeting = 'D:\bolkar\bench\audio_cache\meeting1',
  [string]$Exe = "$env:LOCALAPPDATA\Speak Flow\app-desktop.exe",
  [string]$Data = 'D:\dev\meet-test',
  [int]$Port = 9334
)
$ErrorActionPreference = 'Stop'
$dir = Split-Path -Parent $MyInvocation.MyCommand.Path
New-Item -ItemType Directory -Force "$Data\models" | Out-Null
if (-not (Test-Path "$Data\models\ggml-large-v3-turbo-q5_0.bin")) { Copy-Item 'D:\bolkar\models\ggml-large-v3-turbo-q5_0.bin' "$Data\models\" }
Get-Process app-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2
$env:APP_DATA_DIR = $Data
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
Start-Process $Exe | Out-Null
Start-Sleep -Seconds 10
$mic = if ($WithMic) { 'true' } else { 'false' }
node "$dir\cdp.mjs" $Port main "window.__TAURI_INTERNALS__.invoke('start_meeting', {mic:$mic, system:true, consent:true}).then(()=>'started', e=>'ERROR '+e)" | Select-Object -Skip 1
$started = Get-Date
foreach ($f in Get-ChildItem $Meeting -Filter *.wav | Sort-Object Name) {
  (New-Object System.Media.SoundPlayer $f.FullName).PlaySync()
  Start-Sleep -Milliseconds 400
}
Start-Sleep -Seconds 2
$played = [int]((Get-Date) - $started).TotalSeconds
node "$dir\cdp.mjs" $Port main "window.__TAURI_INTERNALS__.invoke('meeting_status').then(s=>'live: '+JSON.stringify(s))" | Select-Object -Skip 1
$t0 = Get-Date
node "$dir\cdp.mjs" $Port main "window.__TAURI_INTERNALS__.invoke('stop_meeting', {title:'Quarterly finance review', thoughts:[]}).then(r=>JSON.stringify(r), e=>JSON.stringify({error:String(e)}))" | Select-Object -Skip 1 | Out-File -Encoding utf8 D:\dev\bench-meeting.json
$took = [int]((Get-Date) - $t0).TotalSeconds
$r = Get-Content D:\dev\bench-meeting.json -Raw -Encoding utf8 | ConvertFrom-Json
if ($r.error) { "ERROR: $($r.error)"; exit 1 }
$others = ($r.segments | Where-Object { $_.speaker -eq 'Others' } | ForEach-Object { $_.text }) -join ' '
$me = @($r.segments | Where-Object { $_.speaker -eq 'Me' })
"ALL`t$others" | Set-Content -Encoding utf8 D:\dev\bench-meeting-hyp.tsv
$score = & 'D:\t\release\wer.exe' --ref "$Meeting\refs.tsv" --hyp D:\dev\bench-meeting-hyp.tsv | Select-String 'ALL \(pooled\)'
$ordered = $true
for ($i = 1; $i -lt $r.segments.Count; $i++) { if ($r.segments[$i].start_ms -lt $r.segments[$i - 1].start_ms) { $ordered = $false } }
$wav = Join-Path $Data "meetings\$($r.id)\system.wav"
$wavSec = if (Test-Path $wav) { [math]::Round(((Get-Item $wav).Length - 44) / 32000, 1) } else { 0 }
"played $played s | system track $wavSec s | transcription + notes took $took s"
"segments: $($r.segments.Count) (Others $(@($r.segments | Where-Object speaker -eq 'Others').Count), Me $($me.Count)) | in time order: $ordered"
"Others transcript vs script: $($score.Line)"
"note: $($r.note)"
if ($me.Count -gt 0) { "Me segments kept:"; $me | ForEach-Object { "   [$($_.start_ms) ms] $($_.text)" } }
Get-Process app-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
$env:APP_DATA_DIR = $null
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $null
