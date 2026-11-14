# Long-run test: the real app transcribes the same recording N times in a row (never pasted
# anywhere), with its own throwaway data folder. Reports failures, whether every result is
# identical, timing, and the app's memory over the run (a leak shows as steady growth).
#   longrun.ps1 [-Runs 100] [-Wav ...] [-Model large-v3-turbo]
param(
  [int]$Runs = 100,
  [string]$Exe = 'D:\t\release\app-desktop.exe',
  [string]$Wav = 'D:\bolkar\bench\audio_cache\tts_en_20s.wav',
  [string]$ModelsDir = 'D:\bolkar\models',
  [string]$WorkDir = 'D:\dev\longrun'
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force "$WorkDir\data\models" | Out-Null
Get-ChildItem $ModelsDir -Filter *.bin | ForEach-Object {
  $dest = Join-Path "$WorkDir\data\models" $_.Name
  # Hard links: same files, no extra disk space (same drive).
  if (-not (Test-Path $dest)) { New-Item -ItemType HardLink -Path $dest -Target $_.FullName | Out-Null }
}
$out = "$WorkDir\runs.jsonl"
if (Test-Path $out) { [IO.File]::Delete($out) }

$env:APP_DATA_DIR = "$WorkDir\data"
$env:APP_SELFTEST_WAV = $Wav
$env:APP_SELFTEST_OUT = $out
$env:APP_SELFTEST_INSERT = '0'
$env:APP_SELFTEST_REPEAT = "$Runs"
$sw = [Diagnostics.Stopwatch]::StartNew()
$app = Start-Process -FilePath $Exe -PassThru
$mem = @()
while (-not $app.HasExited) {
  Start-Sleep -Seconds 2
  try { $app.Refresh(); $mem += [pscustomobject]@{ S = [int]$sw.Elapsed.TotalSeconds; MB = [math]::Round($app.WorkingSet64 / 1MB) } } catch {}
  if ($sw.Elapsed.TotalMinutes -gt 60) { throw 'long-run test took more than 60 minutes' }
}
$lines = Get-Content $out | ForEach-Object { $_ | ConvertFrom-Json }
$ok = @($lines | Where-Object { $_.ok })
$failed = @($lines | Where-Object { -not $_.ok })
$texts = @($ok | ForEach-Object { $_.text } | Sort-Object -Unique)
$t = @($ok | ForEach-Object { [int]$_.total_ms } | Sort-Object)
$pct = { param($p) if ($t.Count) { $t[[math]::Min($t.Count - 1, [int]($t.Count * $p))] } else { 0 } }
$after10 = @($mem | Where-Object { $_.S -ge 20 })
"runs requested $Runs | finished $($lines.Count) | ok $($ok.Count) | failed $($failed.Count)"
"distinct texts: $($texts.Count)"
"time per dictation (ms): median $(& $pct 0.5) | p90 $(& $pct 0.9) | max $($t[-1])"
"memory MB: first sample $($mem[0].MB) | after warm-up $($after10[0].MB) | last $($mem[-1].MB) | max $(($mem | Measure-Object MB -Maximum).Maximum)"
"total $([int]$sw.Elapsed.TotalSeconds) s, exit code $($app.ExitCode)"
$failed | Select-Object -First 3 | ForEach-Object { "error: $($_.error)" }
if ($texts.Count -gt 1) { $texts | ForEach-Object { "text: $_" } }
$mem | Export-Csv "$WorkDir\memory.csv" -NoTypeInformation
if ($failed.Count -gt 0 -or $lines.Count -ne $Runs) { exit 1 }
