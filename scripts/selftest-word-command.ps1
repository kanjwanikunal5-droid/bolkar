# Real Command Mode test in Microsoft Word.
#   Word document with some text, all selected -> the app hears a spoken instruction (WAV)
#   -> copies the selection -> applies the command -> pastes over the selection -> Word is
#   asked (through its own object model) whether the document now holds a real list.
# Uses a synthetic spoken instruction: proves the plumbing, not recognition accuracy.
# The app pastes ONLY if the Word window (exact title) is in front; the test document is
# closed without saving afterwards.
param(
  [string]$Exe = 'D:\t\release\app-desktop.exe',
  [string]$Wav = 'D:\bolkar\bench\audio_cache\commands\c01-heera.wav',
  [string]$Text = 'Check the invoice. Call the client. Send the report.',
  [int]$ExpectItems = 3,
  [ValidateSet('bullet', 'number')][string]$Kind = 'bullet',
  [string]$ModelsDir = 'D:\bolkar\models',
  [string]$WorkDir = 'D:\dev\selftest-word'
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
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
$out = "$WorkDir\result.json"
if (Test-Path $out) { [System.IO.File]::Delete($out) }
$marker = 'CLIPBOARD-BEFORE-' + [guid]::NewGuid().ToString('N')
[System.Windows.Forms.Clipboard]::SetText($marker)

$word = New-Object -ComObject Word.Application
$word.Visible = $true
$doc = $word.Documents.Add()
$doc.Content.Text = $Text
$doc.Content.Select()
$caption = 'Command test ' + (Get-Date -Format 'HHmmss')
$word.ActiveWindow.Caption = $caption
$title = ''
for ($i = 0; $i -lt 50; $i++) {
  Start-Sleep -Milliseconds 200
  $h = [Fg]::Find($caption)
  if ($h -ne [IntPtr]::Zero) { [Fg]::Front($h) }
  $t = [Fg]::Title()
  if ($t -like "*$caption*") { $title = $t; break }
}
if (-not $title) { "Word did not come to the front (front: '$([Fg]::Title())'). Nothing was pasted."; $doc.Close(0); $word.Quit(); exit 2 }
"Word in front: $title"

$env:APP_DATA_DIR = "$WorkDir\data"
$env:APP_SELFTEST_WAV = $Wav
$env:APP_SELFTEST_OUT = $out
$env:APP_SELFTEST_INSERT = '1'
$env:APP_SELFTEST_COMMAND = '1'
$env:APP_SELFTEST_DELAY_MS = '0'
$env:APP_SELFTEST_EXPECT_TITLE = $title
$app = Start-Process -FilePath $Exe -PassThru
# The app's window may take focus while it starts; put Word back in front (no key presses)
# during the first 3 seconds only, and log every change of the window in front.
$seen = @(); $sw = [Diagnostics.Stopwatch]::StartNew()
while (-not $app.HasExited -and $sw.ElapsedMilliseconds -lt 60000) {
  Start-Sleep -Milliseconds 50
  $now = [Fg]::Title()
  if ($seen.Count -eq 0 -or $seen[-1].Title -ne $now) { $seen += [pscustomobject]@{ Ms = $sw.ElapsedMilliseconds; Title = $now } }
  if ($now -ne $title -and $sw.ElapsedMilliseconds -lt 3000) { [Fg]::Front([Fg]::Find($caption)) }
}
"front window log:"; $seen | ForEach-Object { "  {0,6} ms  {1}" -f $_.Ms, $_.Title }
if (-not $app.WaitForExit(120000)) { Stop-Process -Id $app.Id -Force; throw 'app did not finish' }
Start-Sleep -Milliseconds 800

$result = Get-Content $out -Raw | ConvertFrom-Json
$lists = $doc.Lists.Count
$items = 0; $type = 0; $marker1 = ''
if ($lists -ge 1) {
  $items = $doc.Lists.Item(1).ListParagraphs.Count
  $type = $doc.Lists.Item(1).Range.ListFormat.ListType
  $marker1 = $doc.Lists.Item(1).ListParagraphs.Item(1).Range.ListFormat.ListString
}
# Pasted HTML lists come in as Word "outline" lists (type 4); the first item's marker tells
# bullets ("•" or a Symbol-font character) from numbers ("1.").
$kindOk = if ($Kind -eq 'bullet') { $marker1 -and $marker1 -notmatch '\d' } else { $marker1 -match '^1[.)]' }
$wordText = $doc.Content.Text.Trim()
$clipAfter = [System.Windows.Forms.Clipboard]::GetText()
$checks = [ordered]@{
  'app reported success'   = [bool]$result.ok
  'outcome is Pasted'      = ($null -ne $result.result.outcome.Pasted)
  'Word has one real list' = ($lists -eq 1)
  "list has $ExpectItems items" = ($items -eq $ExpectItems)
  "list type is $Kind"     = $kindOk
  'clipboard restored'     = ($clipAfter -eq $marker)
}
$checks.GetEnumerator() | ForEach-Object { '{0,-28} {1}' -f $_.Key, $(if ($_.Value) { 'PASS' } else { 'FAIL' }) }
"heard: $($result.result.raw_text)"
"how: $($result.result.engine)"
"word list type: $type, first marker: [$marker1]"
"word text: $($wordText -replace "`r", ' | ')"
if (-not $result.ok) { "error: $($result.error)" }
$doc.Close(0)
$word.Quit()
if ($checks.Values -contains $false) { exit 1 }
