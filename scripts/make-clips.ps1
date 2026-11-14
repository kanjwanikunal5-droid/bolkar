# Synthetic test clips from a sentence list ("id<TAB>text" lines, '#' lines skipped), spoken
# by the Windows Indian-English voices. For comparing recognition settings; the owner's own
# recordings are still the real test.
#   make-clips.ps1 -Lines bench\sets\hinglish30.tsv -OutDir bench\audio_cache\hinglish30
param(
  [Parameter(Mandatory = $true)][string]$Lines,
  [Parameter(Mandatory = $true)][string]$OutDir,
  [string[]]$Voices = @('Heera', 'Ravi')
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $OutDir | Out-Null
[void][Windows.Media.SpeechSynthesis.SpeechSynthesizer, Windows.Media.SpeechSynthesis, ContentType = WindowsRuntime]
[void][Windows.Storage.Streams.DataReader, Windows.Storage.Streams, ContentType = WindowsRuntime]
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$asTask = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object { $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' })[0]
function Await($op, [Type]$t) { $task = $asTask.MakeGenericMethod($t).Invoke($null, @($op)); $task.Wait(); $task.Result }

$items = Get-Content $Lines -Encoding utf8 | Where-Object { $_ -and -not $_.StartsWith('#') } | ForEach-Object { $p = $_ -split "`t", 2; [pscustomobject]@{ Id = $p[0]; Text = $p[1] } }
$synth = New-Object Windows.Media.SpeechSynthesis.SpeechSynthesizer
$refs = @()
foreach ($voiceName in $Voices) {
  $voice = [Windows.Media.SpeechSynthesis.SpeechSynthesizer]::AllVoices | Where-Object { $_.DisplayName -like "*$voiceName*" } | Select-Object -First 1
  if (-not $voice) { throw "voice $voiceName not installed" }
  $synth.Voice = $voice
  foreach ($it in $items) {
    $stream = Await ($synth.SynthesizeTextToStreamAsync($it.Text)) ([Windows.Media.SpeechSynthesis.SpeechSynthesisStream])
    $reader = New-Object Windows.Storage.Streams.DataReader($stream.GetInputStreamAt(0))
    $n = [uint32]$stream.Size
    [void](Await ($reader.LoadAsync($n)) ([uint32]))
    $bytes = New-Object byte[] $n
    $reader.ReadBytes($bytes)
    $id = "$($it.Id)-$($voiceName.ToLower())"
    [System.IO.File]::WriteAllBytes((Join-Path $OutDir "$id.wav"), $bytes)
    $refs += "$id`t$($it.Text)"
  }
}
Set-Content -Path (Join-Path $OutDir 'refs.tsv') -Value $refs -Encoding utf8
"wrote $($refs.Count) clips to $OutDir"
