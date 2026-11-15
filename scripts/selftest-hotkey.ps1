# End-to-end shortcut test on the real app: presses and holds Ctrl+Win (sent by software),
# releases it, and checks the app started and stopped a dictation. Uses a fresh data folder
# (no Groq: nothing is uploaded), and test runs never type into any window.
param(
  [string]$Exe = 'D:\t\release\app-desktop.exe',
  [string]$ModelsDir = 'D:\bolkar\models',
  [string]$WorkDir = 'D:\dev\hotkeytest',
  [int]$HoldMs = 2500
)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class Keys {
  [StructLayout(LayoutKind.Sequential)] struct KEYBDINPUT { public ushort vk; public ushort scan; public uint flags; public uint time; public IntPtr extra; }
  [StructLayout(LayoutKind.Explicit, Size = 40)] struct INPUT { [FieldOffset(0)] public uint type; [FieldOffset(8)] public KEYBDINPUT ki; }
  [DllImport("user32.dll")] static extern uint SendInput(uint n, INPUT[] inputs, int size);
  public static void Key(ushort vk, bool up) {
    var i = new INPUT[1]; i[0].type = 1; i[0].ki.vk = vk; i[0].ki.flags = up ? 2u : 0u;
    SendInput(1, i, Marshal.SizeOf(typeof(INPUT)));
  }
}
'@
New-Item -ItemType Directory -Force "$WorkDir\data\models" | Out-Null
$turbo = 'ggml-large-v3-turbo-q5_0.bin'
if (-not (Test-Path "$WorkDir\data\models\$turbo")) { New-Item -ItemType HardLink -Path "$WorkDir\data\models\$turbo" -Target "$ModelsDir\$turbo" | Out-Null }
$log = "$WorkDir\trace.log"
if (Test-Path $log) { [IO.File]::Delete($log) }

$env:APP_DATA_DIR = "$WorkDir\data"
$env:APP_TEST_INJECTED_KEYS = '1'
$env:APP_TEST_LOG = $log
Remove-Item Env:APP_SELFTEST_WAV -ErrorAction SilentlyContinue
$app = Start-Process -FilePath $Exe -PassThru
Start-Sleep -Seconds 12   # model load + warm-up

$results = @()
foreach ($round in 1..3) {
  [Keys]::Key(0xA2, $false)   # left Ctrl down
  Start-Sleep -Milliseconds 60
  [Keys]::Key(0x5B, $false)   # left Win down
  Start-Sleep -Milliseconds $HoldMs
  [Keys]::Key(0x5B, $true)
  Start-Sleep -Milliseconds 30
  [Keys]::Key(0xA2, $true)
  Start-Sleep -Seconds 5
}
Start-Sleep -Seconds 2
$trace = if (Test-Path $log) { Get-Content $log } else { @() }
$starts = @($trace | Select-String 'PttStart').Count
$stops = @($trace | Select-String 'PttStop').Count
$pills = @($trace | Select-String ' pill ')
"trace:"; $trace | ForEach-Object { "  $_" }
'{0,-34} {1}' -f 'Ctrl+Win press started dictation', $(if ($starts -eq 3) { 'PASS (3 of 3)' } else { "FAIL ($starts of 3)" })
'{0,-34} {1}' -f 'release stopped it', $(if ($stops -eq 3) { 'PASS (3 of 3)' } else { "FAIL ($stops of 3)" })
'{0,-34} {1}' -f 'app answered every time', $(if ($pills.Count -ge 3) { "PASS ($($pills.Count) messages)" } else { "FAIL ($($pills.Count) messages)" })
if (-not $app.HasExited) { Stop-Process -Id $app.Id }
if ($starts -ne 3 -or $stops -ne 3 -or $pills.Count -lt 3) { exit 1 }
