# Builds a multi-voice synthetic meeting: one WAV per line plus the full reference script.
param([string]$OutDir = 'D:\bolkar\bench\audio_cache\meeting1')
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $OutDir | Out-Null
$lines = @(
  @('Mark',  'Good morning everyone. Let us start the quarterly finance review.'),
  @('Heera', 'First, sales performance. Revenue grew by 12 percent compared to last quarter, mainly from the western region.'),
  @('Ravi',  'That is good news, but our gross margin dropped from 41 percent to 38 percent because of higher freight costs.'),
  @('Zira',  'Second, the marketing budget. The marketing team is asking for an additional 50,000 rupees for the festive campaign.'),
  @('Mark',  'I am fine with that only if the campaign return on investment stays above three times.'),
  @('Heera', 'Agreed. Let us approve the 50,000 rupees with that condition.'),
  @('Ravi',  'Third, hiring. We have three interviews pending for the financial analyst role.'),
  @('Zira',  'Priya will finish the interviews by next Friday.'),
  @('Mark',  'One risk is the GST filing deadline on the twentieth. If we miss it, there is a penalty.'),
  @('Heera', 'Rahul will prepare the GST return by the eighteenth.'),
  @('Ravi',  'Do we know if the new Power BI dashboard will be ready for the board meeting? I am not sure yet.'),
  @('Zira',  'We will decide that next week.'),
  @('Mark',  'Finally, we decided to review all of this again next Monday. Thank you everyone.')
)
[void][Windows.Media.SpeechSynthesis.SpeechSynthesizer, Windows.Media.SpeechSynthesis, ContentType = WindowsRuntime]
[void][Windows.Storage.Streams.DataReader, Windows.Storage.Streams, ContentType = WindowsRuntime]
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$asTask = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object { $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' })[0]
function Await($op, [Type]$t) { $task = $asTask.MakeGenericMethod($t).Invoke($null, @($op)); $task.Wait(); $task.Result }
$synth = New-Object Windows.Media.SpeechSynthesis.SpeechSynthesizer
$i = 0
foreach ($l in $lines) {
  $i++
  $synth.Voice = [Windows.Media.SpeechSynthesis.SpeechSynthesizer]::AllVoices | Where-Object { $_.DisplayName -like "*$($l[0])*" } | Select-Object -First 1
  $stream = Await ($synth.SynthesizeTextToStreamAsync($l[1])) ([Windows.Media.SpeechSynthesis.SpeechSynthesisStream])
  $reader = New-Object Windows.Storage.Streams.DataReader($stream.GetInputStreamAt(0))
  $n = [uint32]$stream.Size
  [void](Await ($reader.LoadAsync($n)) ([uint32]))
  $bytes = New-Object byte[] $n
  $reader.ReadBytes($bytes)
  [System.IO.File]::WriteAllBytes((Join-Path $OutDir ('{0:D2}.wav' -f $i)), $bytes)
}
"ALL`t" + (($lines | ForEach-Object { $_[1] }) -join ' ') | Set-Content -Encoding utf8 (Join-Path $OutDir 'refs.tsv')
"wrote $i lines"
