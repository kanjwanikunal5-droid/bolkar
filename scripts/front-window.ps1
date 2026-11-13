# Shared helper for the self-test scripts: bring a window to the front without sending keys.
Add-Type @'
using System; using System.Text; using System.Runtime.InteropServices;
public static class FrontWin {
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
    BringWindowToTop(h);
    SetForegroundWindow(h);
    AttachThreadInput(fgThread, me, false);
  }
}
'@

# Returns the front window's title once a window containing $Part is in front, or '' after
# $Seconds. With $Seconds = 0 it makes one attempt.
function Bring-ToFront([string]$Part, [int]$Seconds) {
  $tries = [Math]::Max(1, $Seconds * 5)
  for ($i = 0; $i -lt $tries; $i++) {
    $h = [FrontWin]::Find($Part)
    if ($h -ne [IntPtr]::Zero -and [FrontWin]::Title() -notlike "*$Part*") { [FrontWin]::Front($h) }
    $t = [FrontWin]::Title()
    if ($t -like "*$Part*") { return $t }
    if ($tries -gt 1) { Start-Sleep -Milliseconds 200 }
  }
  return ''
}
