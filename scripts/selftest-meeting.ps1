# Meeting-notes test with the installed app: plays a synthetic meeting through the speakers,
# records COMPUTER SOUND ONLY (the microphone is not recorded), then stops and prints the notes.
# Uses the golden "topic organisation" transcript from the build prompt.
param(
  [string]$Exe = "$env:LOCALAPPDATA\Speak Flow\app-desktop.exe",
  [int]$Port = 9333
)
$ErrorActionPreference = 'Stop'
$dir = Split-Path -Parent $MyInvocation.MyCommand.Path
$wav = 'D:\bolkar\bench\audio_cache\tts_meeting_golden.wav'
Add-Type -AssemblyName System.Speech
$s = New-Object System.Speech.Synthesis.SpeechSynthesizer
$s.SetOutputToWaveFile($wav)
$s.Speak('First we discussed the sales performance. Sales increased by 12 percent. Then we talked about the marketing budget. The marketing team needs another 50,000 rupees. Then we discussed the hiring process. Three interviews are pending. Finally, we decided to review everything next Monday.')
$s.SetOutputToNull()
$s.Dispose()

Get-Process app-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 800
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
Start-Process $Exe | Out-Null
Start-Sleep -Seconds 8

'starting (computer sound only)...'
node "$dir\cdp.mjs" $Port main "window.__TAURI_INTERNALS__.invoke('start_meeting', {mic:false, system:true, consent:true}).then(()=>'started', e=>'ERROR '+e)"
Start-Sleep -Seconds 1
$player = New-Object System.Media.SoundPlayer $wav
$player.PlaySync()
Start-Sleep -Seconds 2
node "$dir\cdp.mjs" $Port main "window.__TAURI_INTERNALS__.invoke('meeting_status').then(s=>JSON.stringify(s))"
'stopping and making notes...'
node "$dir\cdp.mjs" $Port main "window.__TAURI_INTERNALS__.invoke('stop_meeting', {title:'Weekly review', thoughts:[[3000,'check the hiring budget']]}).then(r=>JSON.stringify({segments:r.segments.length, note:r.note, dropped:r.dropped, md:r.markdown}), e=>'ERROR '+e)" | Out-File -Encoding utf8 D:\dev\meeting-result.json
$r = (Get-Content D:\dev\meeting-result.json -Raw -Encoding utf8) -split "`n", 2
$j = $r[1] | ConvertFrom-Json
"segments: $($j.segments) | note: $($j.note) | dropped: $($j.dropped -join '; ')"
'----- notes -----'
$j.md
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $null
