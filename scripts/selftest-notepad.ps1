# Real-app insertion test into Notepad.
#   WAV -> app (local Whisper + clean-up) -> paste into Notepad -> read Notepad's text back
#   through UI Automation -> check the clipboard was restored.
# Uses a synthetic recording: proves the plumbing, not recognition accuracy.
# The app pastes ONLY if Notepad's window (exact title) is in front; otherwise nothing is pasted.
param(
  [string]$Exe = 'D:\t\release\app-desktop.exe',
  [string]$Wav = 'D:\bolkar\bench\audio_cache\tts_en_20s.wav',
  [string]$ModelsDir = 'D:\bolkar\models',
  [string]$WorkDir = 'D:\dev\selftest-notepad'
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms, UIAutomationClient, UIAutomationTypes
Add-Type @'
using System; using System.Text; using System.Runtime.InteropServices;
public static class Fg {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, IntPtr pid);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool attach);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  static string TitleOf(IntPtr h) { var sb = new StringBuilder(512); GetWindowText(h, sb, 512); return sb.ToString(); }
  public static string Title() { return TitleOf(GetForegroundWindow()); }
  public static IntPtr Find(string part) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => { if (IsWindowVisible(h) && TitleOf(h).Contains(part)) { found = h; return false; } return true; }, IntPtr.Zero);
    return found;
  }
  // Brings a window to the front without sending any key presses.
  public static void Front(IntPtr h) {
    uint fgThread = GetWindowThreadProcessId(GetForegroundWindow(), IntPtr.Zero);
    uint me = GetCurrentThreadId();
    AttachThreadInput(fgThread, me, true);
    ShowWindow(h, 9);
    BringWindowToTop(h);
    SetForegroundWindow(h);
    AttachThreadInput(fgThread, me, false);
  }
}
'@

New-Item -ItemType Directory -Force "$WorkDir\data\models" | Out-Null
Get-ChildItem $ModelsDir -Filter *.bin | ForEach-Object {
  $dest = Join-Path "$WorkDir\data\models" $_.Name
  if (-not (Test-Path $dest)) { Copy-Item $_.FullName $dest }
}
$name = 'dictation-test-' + (Get-Date -Format 'HHmmss') + '.txt'
$file = Join-Path $WorkDir $name
Set-Content -Path $file -Value '' -NoNewline
$out = "$WorkDir\result.json"
Remove-Item $out -ErrorAction SilentlyContinue

$marker = 'CLIPBOARD-BEFORE-' + [guid]::NewGuid().ToString('N')
[System.Windows.Forms.Clipboard]::SetText($marker)

$np = Start-Process notepad.exe -ArgumentList "`"$file`"" -PassThru
$title = ''
for ($i = 0; $i -lt 50; $i++) {
  Start-Sleep -Milliseconds 200
  $h = [Fg]::Find($name)
  if ($h -ne [IntPtr]::Zero) { [Fg]::Front($h) }
  $t = [Fg]::Title()
  if ($t -like "*$name*") { $title = $t; break }
}
if (-not $title) { "Notepad did not come to the front (front window: '$([Fg]::Title())'). Nothing was pasted."; exit 2 }
"Notepad in front: $title"

$env:APP_DATA_DIR = "$WorkDir\data"
$env:APP_SELFTEST_WAV = $Wav
$env:APP_SELFTEST_OUT = $out
$env:APP_SELFTEST_INSERT = '1'
$env:APP_SELFTEST_DELAY_MS = '0'
$env:APP_SELFTEST_EXPECT_TITLE = $title
$app = Start-Process -FilePath $Exe -PassThru
if (-not $app.WaitForExit(120000)) { Stop-Process -Id $app.Id -Force; throw 'app did not finish' }
Start-Sleep -Milliseconds 700

# Read Notepad's text through UI Automation (the same way a screen reader would).
$root = [System.Windows.Automation.AutomationElement]::RootElement
$cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty, $np.Id)
$win = $root.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
if (-not $win) {
  # Windows 11 Notepad may hand the file to an already-running Notepad process.
  $all = $root.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition)
  $win = $all | Where-Object { $_.Current.Name -like "*$name*" } | Select-Object -First 1
}
$docCond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Document)
$doc = $win.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $docCond)
if (-not $doc) {
  $editCond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Edit)
  $doc = $win.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $editCond)
}
$typed = $doc.GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern).DocumentRange.GetText(-1)
$clipAfter = [System.Windows.Forms.Clipboard]::GetText()
$result = Get-Content $out -Raw | ConvertFrom-Json

$expected = ($result.result.final_text -replace "`r`n", "`n" -replace "`r", "`n").Trim()
$got = ($typed -replace "`r`n", "`n" -replace "`r", "`n").Trim()
$checks = [ordered]@{
  'app reported success'         = [bool]$result.ok
  'outcome is Pasted'            = ($null -ne $result.result.outcome.Pasted)
  'Notepad contains the text'    = ($got -eq $expected -and $got.Length -gt 0)
  'clipboard restored'           = ($clipAfter -eq $marker)
}
$checks.GetEnumerator() | ForEach-Object { '{0,-28} {1}' -f $_.Key, $(if ($_.Value) { 'PASS' } else { 'FAIL' }) }
"outcome: $($result.result.outcome | ConvertTo-Json -Compress)"
"notepad: $got"
if (-not $result.ok) { "error: $($result.error)" }

# Clean up: close the test file in Notepad without keeping the change.
$doc.SetFocus()
$vp = $null
if ($doc.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$vp)) { $vp.SetValue('') }
Stop-Process -Id $np.Id -Force -ErrorAction SilentlyContinue
Remove-Item $file -ErrorAction SilentlyContinue
if ($checks.Values -contains $false) { exit 1 }
