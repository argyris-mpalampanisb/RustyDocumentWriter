param([string]$doc, [string]$prefix)
$w = New-Object -ComObject Word.Application
$w.Visible = $false
try {
  $d = $w.Documents.Open($doc, $false, $true)
  foreach ($p in $d.Paragraphs) {
    if ($p.Range.Text.StartsWith($prefix)) {
      $r = $p.Range
      $lastY = -1; $line = ""; $lines = @()
      for ($i = $r.Start; $i -lt $r.End - 1; $i++) {
        $c = $d.Range($i, $i + 1)
        $y = $c.Information(6)
        if ($lastY -ge 0 -and $y -ne $lastY) { $lines += $line; $line = "" }
        $lastY = $y
        $line += $c.Text
      }
      $lines += $line
      $n = 1; foreach ($l in $lines) { "{0}: [{1}] x_end={2}" -f $n, $l, 0; $n++ }
      $last = $d.Range($r.End - 2, $r.End - 1)
      "last char x: " + $last.Information(5)
      break
    }
  }
  $d.Close($false)
} finally { $w.Quit() }
