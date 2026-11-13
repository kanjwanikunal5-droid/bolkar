# Synthetic Hinglish test clips (Windows Indian-English voices reading Roman Hinglish).
# For comparing recognition settings only; real recordings of the owner are needed for
# real accuracy numbers.
param([string]$OutDir = 'D:\bolkar\bench\audio_cache\hinglish')
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $OutDir | Out-Null
$lines = [ordered]@{
  'hg01' = 'Bhai aaj ke liye sorry yaar. Mera dimaag kaam nahi kar raha tha.'
  'hg02' = 'Maine kuch nahi khaya tha, mujhe khud ko kuch samajh nahi aa raha tha.'
  'hg03' = 'Kal meeting chaar baje hai, please time pe aa jana.'
  'hg04' = 'Report ready hai, main tujhe shaam tak bhej dunga.'
  'hg05' = 'Love you bhai, dil pe mat lena jo bhi hua.'
  'hg06' = 'Budget review ke baad hum forecast update karenge, theek hai?'
}
[void][Windows.Media.SpeechSynthesis.SpeechSynthesizer, Windows.Media.SpeechSynthesis, ContentType = WindowsRuntime]
[void][Windows.Storage.Streams.DataReader, Windows.Storage.Streams, ContentType = WindowsRuntime]
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$asTask = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object { $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' })[0]
function Await($op, [Type]$t) { $task = $asTask.MakeGenericMethod($t).Invoke($null, @($op)); $task.Wait(); $task.Result }

$synth = New-Object Windows.Media.SpeechSynthesis.SpeechSynthesizer
$refs = @()
foreach ($voiceName in 'Heera', 'Ravi') {
  $synth.Voice = [Windows.Media.SpeechSynthesis.SpeechSynthesizer]::AllVoices | Where-Object { $_.DisplayName -like "*$voiceName*" } | Select-Object -First 1
  foreach ($k in $lines.Keys) {
    $stream = Await ($synth.SynthesizeTextToStreamAsync($lines[$k])) ([Windows.Media.SpeechSynthesis.SpeechSynthesisStream])
    $reader = New-Object Windows.Storage.Streams.DataReader($stream.GetInputStreamAt(0))
    $n = [uint32]$stream.Size
    [void](Await ($reader.LoadAsync($n)) ([uint32]))
    $bytes = New-Object byte[] $n
    $reader.ReadBytes($bytes)
    $id = "$k-$($voiceName.ToLower())"
    [System.IO.File]::WriteAllBytes((Join-Path $OutDir "$id.wav"), $bytes)
    $refs += "$id`t$($lines[$k])"
  }
}
Set-Content -Path (Join-Path $OutDir 'refs.tsv') -Value $refs -Encoding utf8
"wrote $($refs.Count) clips to $OutDir"
