# Real-app insertion test into Google Chrome, using a throwaway Chrome profile (the user's own
# profile, logins and tabs are never touched). The page has a rich-text box like Gmail's
# compose body. Text is read back through Chrome's DevTools protocol.
param(
  [string]$Exe = 'D:\t\release\app-desktop.exe',
  [string]$Wav = 'D:\bolkar\bench\audio_cache\tts_en_20s.wav',
  [string]$ModelsDir = 'D:\bolkar\models',
  [string]$WorkDir = 'D:\dev\selftest-chrome',
  [int]$Port = 9444
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
. (Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) 'front-window.ps1')

$chrome = @("$env:ProgramFiles\Google\Chrome\Application\chrome.exe", "${env:ProgramFiles(x86)}\Google\Chrome\Application\chrome.exe", "$env:LOCALAPPDATA\Google\Chrome\Application\chrome.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $chrome) { 'Chrome not found'; exit 2 }

New-Item -ItemType Directory -Force "$WorkDir\data\models", "$WorkDir\profile" | Out-Null
Get-ChildItem $ModelsDir -Filter *.bin | ForEach-Object {
  $dest = Join-Path "$WorkDir\data\models" $_.Name
  if (-not (Test-Path $dest)) { Copy-Item $_.FullName $dest }
}
$tag = 'Compose test ' + (Get-Date -Format 'HHmmss')
@"
<!doctype html><html><head><meta charset="utf-8"><title>$tag</title></head>
<body><p>To: test</p><div id="body" contenteditable="true" style="min-height:300px;border:1px solid #888;padding:8px"></div>
<script>window.onload = () => document.getElementById('body').focus();</script></body></html>
"@ | Set-Content -Path "$WorkDir\compose.html" -Encoding utf8
$out = "$WorkDir\result.json"
Remove-Item $out -ErrorAction SilentlyContinue

$marker = 'CLIPBOARD-BEFORE-' + [guid]::NewGuid().ToString('N')
[System.Windows.Forms.Clipboard]::SetText($marker)

$url = 'file:///' + ("$WorkDir\compose.html" -replace '\\', '/')
$cp = Start-Process $chrome -ArgumentList "--user-data-dir=`"$WorkDir\profile`"", "--remote-debugging-port=$Port", '--no-first-run', '--no-default-browser-check', '--new-window', $url -PassThru
$title = Bring-ToFront $tag 15
if (-not $title) { "Chrome did not come to the front. Nothing was pasted."; exit 2 }
"Chrome in front: $title"
Start-Sleep -Milliseconds 800
$title = Bring-ToFront $tag 5

$env:APP_DATA_DIR = "$WorkDir\data"
$env:APP_SELFTEST_WAV = $Wav
$env:APP_SELFTEST_OUT = $out
$env:APP_SELFTEST_INSERT = '1'
$env:APP_SELFTEST_DELAY_MS = '0'
$env:APP_SELFTEST_EXPECT_TITLE = $title
$app = Start-Process -FilePath $Exe -PassThru
# Keep the compose window in front while the app starts (its own window opens briefly).
for ($i = 0; $i -lt 20; $i++) { Start-Sleep -Milliseconds 150; [void](Bring-ToFront $tag 0) }
if (-not $app.WaitForExit(120000)) { Stop-Process -Id $app.Id -Force; throw 'app did not finish' }
Start-Sleep -Milliseconds 700

$typed = (node "$PSScriptRoot\cdp.mjs" $Port compose "document.getElementById('body').innerText" 2>$null | Select-Object -Skip 1) -join "`n"
$clipAfter = [System.Windows.Forms.Clipboard]::GetText()
$result = Get-Content $out -Raw | ConvertFrom-Json
$expected = ($result.result.final_text -replace "`r`n", "`n").Trim()
$got = ($typed -replace "`r`n", "`n").Trim()
$checks = [ordered]@{
  'app reported success'      = [bool]$result.ok
  'outcome is Pasted'         = ($null -ne $result.result.outcome.Pasted)
  'compose box has the text'  = ($got -eq $expected -and $got.Length -gt 0)
  'clipboard restored'        = ($clipAfter -eq $marker)
}
$checks.GetEnumerator() | ForEach-Object { '{0,-28} {1}' -f $_.Key, $(if ($_.Value) { 'PASS' } else { 'FAIL' }) }
"outcome: $($result.result.outcome | ConvertTo-Json -Compress)"
"compose box: $got"
if (-not $result.ok) { "error: $($result.error)" }
Get-CimInstance Win32_Process -Filter "Name='chrome.exe'" | Where-Object { $_.CommandLine -like "*$WorkDir\profile*" } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
if ($checks.Values -contains $false) { exit 1 }
