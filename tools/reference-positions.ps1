param([string]$doc)
$w = New-Object -ComObject Word.Application
$w.Visible = $false
$w.DisplayAlerts = 0
try {
  $d = $w.Documents.Open($doc, $false, $true)
  $d.Repaginate()
  foreach ($p in $d.Paragraphs) {
    $r = $p.Range
    $c = $d.Range($r.Start, $r.Start)
    $page = $c.Information(3)
    $y = $c.Information(6)
    $t = ($r.Text -replace '[\x00-\x1f\x07]', '')
    if ($t.Length -gt 30) { $t = $t.Substring(0,30) }
    "{0}`t{1:N1}`t{2}" -f $page, $y, $t
  }
  "PAGES`t" + $d.ComputeStatistics(2)
  $d.Close($false)
} finally { $w.Quit() }
