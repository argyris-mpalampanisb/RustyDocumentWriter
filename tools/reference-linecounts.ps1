param([string]$doc)
$w = New-Object -ComObject Word.Application
$w.Visible = $false
try {
  $d = $w.Documents.Open($doc, $false, $true)
  foreach ($p in $d.Paragraphs) { $p.Range.ComputeStatistics(1) }
  $d.Close($false)
} finally { $w.Quit() }
