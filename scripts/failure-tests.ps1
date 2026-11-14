# Failure tests on the real app (release build), each with its own throwaway data folder and
# never pasting anywhere. Checks the app ends normally, reports a clear error where one is
# expected, and never invents text from silence or noise.
param(
  [string]$Exe = 'D:\t\release\app-desktop.exe',
  [string]$ModelsDir = 'D:\bolkar\models',
  [string]$WorkDir = 'D:\dev\failure-tests'
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $WorkDir | Out-Null

function Write-Wav([string]$path, [double]$seconds, [double]$noise) {
  $rate = 16000; $n = [int]($rate * $seconds)
  $rng = New-Object System.Random 7
  $ms = New-Object System.IO.MemoryStream
  $w = New-Object System.IO.BinaryWriter $ms
  $w.Write([Text.Encoding]::ASCII.GetBytes('RIFF')); $w.Write([int](36 + $n * 2)); $w.Write([Text.Encoding]::ASCII.GetBytes('WAVEfmt '))
  $w.Write([int]16); $w.Write([int16]1); $w.Write([int16]1); $w.Write([int]$rate); $w.Write([int]($rate * 2)); $w.Write([int16]2); $w.Write([int16]16)
  $w.Write([Text.Encoding]::ASCII.GetBytes('data')); $w.Write([int]($n * 2))
  for ($i = 0; $i -lt $n; $i++) { $w.Write([int16]([math]::Round(($rng.NextDouble() * 2 - 1) * $noise * 32767))) }
  $w.Flush(); [IO.File]::WriteAllBytes($path, $ms.ToArray())
}

function New-DataDir([string]$name, [string]$models) {
  $d = Join-Path $WorkDir $name
  if (Test-Path $d) { Get-ChildItem $d -Recurse -File | ForEach-Object { [IO.File]::Delete($_.FullName) } }
  New-Item -ItemType Directory -Force "$d\models" | Out-Null
  if ($models -eq 'real') {
    New-Item -ItemType HardLink -Path "$d\models\ggml-large-v3-turbo-q5_0.bin" -Target "$ModelsDir\ggml-large-v3-turbo-q5_0.bin" | Out-Null
  } elseif ($models -eq 'corrupt') {
    [IO.File]::WriteAllBytes("$d\models\ggml-large-v3-turbo-q5_0.bin", [byte[]](1..200 | ForEach-Object { 0x41 }))
  }
  $d
}

function Run-Case([string]$name, [string]$models, [string]$wav) {
  $d = New-DataDir $name $models
  $out = "$d\result.json"
  $env:APP_DATA_DIR = $d
  $env:APP_SELFTEST_WAV = $wav
  $env:APP_SELFTEST_OUT = $out
  $env:APP_SELFTEST_INSERT = '0'
  $env:APP_SELFTEST_REPEAT = '1'
  $p = Start-Process -FilePath $Exe -PassThru
  $ended = $p.WaitForExit(90000)
  if (-not $ended) { return [pscustomobject]@{ Ended = $false; Exit = $null; Ok = $null; Text = $null; Error = 'did not finish in 90 s' } }
  $r = if (Test-Path $out) { Get-Content $out -Raw | ConvertFrom-Json } else { $null }
  [pscustomobject]@{ Ended = $true; Exit = $p.ExitCode; Ok = $r.ok; Text = $r.result.final_text; Error = $r.error }
}

Write-Wav "$WorkDir\silence.wav" 3 0
Write-Wav "$WorkDir\noise.wav" 3 0.05
$speech = 'D:\bolkar\bench\audio_cache\tts_en_20s.wav'

$cases = @(
  @{ Name = 'no-model';      Models = 'none';    Wav = $speech;                  Expect = 'error'; Pattern = 'No speech model' },
  @{ Name = 'corrupt-model'; Models = 'corrupt'; Wav = $speech;                  Expect = 'error'; Pattern = '.' },
  @{ Name = 'missing-file';  Models = 'real';    Wav = "$WorkDir\nothing.wav";   Expect = 'error'; Pattern = '.' },
  @{ Name = 'silence';       Models = 'real';    Wav = "$WorkDir\silence.wav";   Expect = 'empty'; Pattern = '' },
  @{ Name = 'noise';         Models = 'real';    Wav = "$WorkDir\noise.wav";     Expect = 'empty'; Pattern = '' },
  @{ Name = 'normal-speech'; Models = 'real';    Wav = $speech;                  Expect = 'text';  Pattern = 'budget' }
)
$fail = 0
foreach ($c in $cases) {
  $r = Run-Case $c.Name $c.Models $c.Wav
  $pass = $r.Ended -and $r.Exit -eq 0 -and $(switch ($c.Expect) {
      'error' { (-not $r.Ok) -and ($r.Error -match $c.Pattern) }
      'empty' { $r.Ok -and [string]::IsNullOrWhiteSpace($r.Text) }
      'text'  { $r.Ok -and ($r.Text -match $c.Pattern) }
    })
  if (-not $pass) { $fail++ }
  '{0,-14} {1}  exit={2} ok={3} | {4}' -f $c.Name, $(if ($pass) { 'PASS' } else { 'FAIL' }), $r.Exit, $r.Ok, $(if ($r.Error) { "error: $($r.Error)" } else { "text: '$($r.Text)'" })
}
if ($fail) { exit 1 }
