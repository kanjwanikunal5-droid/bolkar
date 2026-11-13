# End-to-end plumbing test of the desktop app on a real Windows session:
#   WAV file -> local speech engine -> cleanup -> paste into a real text box -> clipboard restored.
# It uses a synthetic (text-to-speech) recording, so it proves the plumbing works,
# NOT recognition accuracy.
#
# Safety: the app pastes ONLY into the window titled "Selftest target". If that window is
# not in front (for example you clicked another window), nothing is pasted anywhere.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\selftest-insert.ps1 -Exe D:\t\release\app-desktop.exe -Wav bench\audio_cache\tts_en_20s.wav
param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [Parameter(Mandatory = $true)][string]$Wav,
  [string]$ModelsDir = '',
  [string]$WorkDir = "D:\dev\selftest"
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
# Windows PowerShell 5.1 does not set $PSScriptRoot inside param() defaults.
if (-not $ModelsDir) { $ModelsDir = Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) '..\models' }
$Title = 'Selftest target'

New-Item -ItemType Directory -Force "$WorkDir\data\models" | Out-Null
Get-ChildItem $ModelsDir -Filter *.bin | ForEach-Object {
  $dest = Join-Path "$WorkDir\data\models" $_.Name
  if (-not (Test-Path $dest)) { Copy-Item $_.FullName $dest }
}
$out = "$WorkDir\result.json"
$captured = "$WorkDir\captured.txt"
Remove-Item $out, $captured -ErrorAction SilentlyContinue

# 1. Known clipboard content, to check it comes back.
$marker = "CLIPBOARD-BEFORE-" + [guid]::NewGuid().ToString('N')
[System.Windows.Forms.Clipboard]::SetText($marker)

# 2. A plain text box in its own process: the "app being dictated into". Windows does not
#    let a background process take focus, so it uses the usual Alt-key technique to come
#    to the front, and keeps doing so for the first 12 seconds.
$helper = @"
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class Front {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  public static void Force(IntPtr h) {
    if (GetForegroundWindow() == h) return;
    keybd_event(0x12, 0, 0, UIntPtr.Zero);      // Alt down
    keybd_event(0x12, 0, 2, UIntPtr.Zero);      // Alt up
    SetForegroundWindow(h);
  }
}
'@
`$form = New-Object System.Windows.Forms.Form
`$form.Text = '$Title'
`$form.Width = 700; `$form.Height = 400; `$form.TopMost = `$true
`$box = New-Object System.Windows.Forms.TextBox
`$box.Multiline = `$true; `$box.Dock = 'Fill'
`$form.Controls.Add(`$box)
`$started = Get-Date
`$timer = New-Object System.Windows.Forms.Timer
`$timer.Interval = 250
`$timer.Add_Tick({
  [System.IO.File]::WriteAllText('$captured', `$box.Text)
  if (((Get-Date) - `$started).TotalSeconds -lt 12) { [Front]::Force(`$form.Handle); `$box.Focus() | Out-Null }
})
`$timer.Start()
`$form.Add_Shown({ [Front]::Force(`$form.Handle); `$box.Focus() })
[System.Windows.Forms.Application]::Run(`$form)
"@
$helperFile = "$WorkDir\helper.ps1"
Set-Content -Path $helperFile -Value $helper -Encoding ascii
$target = Start-Process -FilePath powershell.exe -ArgumentList '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $helperFile -PassThru
Start-Sleep -Milliseconds 2500

# 3. Start the app in self-test mode. It waits until the target window is in front.
$env:APP_DATA_DIR = "$WorkDir\data"
$env:APP_SELFTEST_WAV = (Resolve-Path $Wav).Path
$env:APP_SELFTEST_OUT = $out
$env:APP_SELFTEST_INSERT = '1'
$env:APP_SELFTEST_DELAY_MS = '4000'
$env:APP_SELFTEST_EXPECT_TITLE = $Title
$app = Start-Process -FilePath $Exe -PassThru

# 4. Wait for the app to finish and quit.
if (-not $app.WaitForExit(120000)) { Stop-Process -Id $app.Id -Force; throw 'app did not finish within 120 s' }
Start-Sleep -Milliseconds 800

$result = Get-Content $out -Raw | ConvertFrom-Json
$typed = if (Test-Path $captured) { [System.IO.File]::ReadAllText($captured) } else { '' }
$clipAfter = [System.Windows.Forms.Clipboard]::GetText()
Stop-Process -Id $target.Id -Force -ErrorAction SilentlyContinue

$expected = ($result.result.final_text -replace "`r`n", "`n")
$got = ($typed -replace "`r`n", "`n")
$checks = [ordered]@{
  'app reported success'            = [bool]$result.ok
  'text was produced'               = ($expected.Length -gt 0)
  'outcome is Pasted'               = ($null -ne $result.result.outcome.Pasted)
  'text box received exactly it'    = ($got -eq $expected)
  'clipboard restored'              = ($clipAfter -eq $marker)
  'app says clipboard restored'     = [bool]$result.result.outcome.Pasted.clipboard_restored
}
$checks.GetEnumerator() | ForEach-Object { '{0,-32} {1}' -f $_.Key, $(if ($_.Value) { 'PASS' } else { 'FAIL' }) }
"engine: $($result.result.engine) | audio_ms: $($result.result.audio_ms) | transcribe_ms: $($result.result.transcribe_ms) | target: $($result.result.target_process)"
"outcome: $($result.result.outcome | ConvertTo-Json -Compress)"
"final: $expected"
if (-not $result.ok) { "error: $($result.error)" }
if ($checks.Values -contains $false) { exit 1 }
