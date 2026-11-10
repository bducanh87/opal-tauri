param([string]$OutDir)
# Opal app icon: an iridescent opal cabochon on a dark rounded glass tile.
Add-Type -AssemblyName System.Drawing
$ErrorActionPreference = "Stop"

function C($hex, $a = 255) { $v = [Convert]::ToInt32($hex, 16); [System.Drawing.Color]::FromArgb($a, ($v -shr 16) -band 255, ($v -shr 8) -band 255, $v -band 255) }
function RoundRect($x, $y, $w, $h, $r) {
  $p = New-Object System.Drawing.Drawing2D.GraphicsPath
  $d = 2 * $r
  $p.AddArc($x, $y, $d, $d, 180, 90); $p.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
  $p.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90); $p.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
  $p.CloseFigure(); $p
}

function Draw([int]$S) {
  # Draw big (4x) and scale down for clean antialiasing.
  $B = $S * 4
  $bmp = New-Object System.Drawing.Bitmap $B, $B, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = 'AntiAlias'; $g.InterpolationMode = 'HighQualityBicubic'; $g.PixelOffsetMode = 'HighQuality'
  $g.Clear([System.Drawing.Color]::Transparent)
  $k = $B / 256.0

  # Tile
  $m = 8 * $k; $tile = RoundRect $m $m ($B - 2 * $m) ($B - 2 * $m) (58 * $k)
  $tb = New-Object System.Drawing.Drawing2D.LinearGradientBrush ((New-Object System.Drawing.PointF 0, 0)), ((New-Object System.Drawing.PointF $B, $B)), (C "2a2546"), (C "0b1019")
  $g.FillPath($tb, $tile)
  # soft colour glow inside the tile
  $glow = New-Object System.Drawing.Drawing2D.GraphicsPath; $glow.AddEllipse(-40 * $k, 120 * $k, 260 * $k, 200 * $k)
  $gb = New-Object System.Drawing.Drawing2D.PathGradientBrush $glow; $gb.CenterColor = (C "6f63e0" 110); $gb.SurroundColors = @((C "6f63e0" 0))
  $g.SetClip($tile); $g.FillPath($gb, $glow); $g.ResetClip()
  $rim = New-Object System.Drawing.Pen ((C "ffffff" 70)), (2.5 * $k)
  $g.DrawPath($rim, $tile)

  # Gem (cabochon)
  $gx = 46 * $k; $gy = 58 * $k; $gw = 164 * $k; $gh = 140 * $k
  $gem = New-Object System.Drawing.Drawing2D.GraphicsPath; $gem.AddEllipse($gx, $gy, $gw, $gh)
  $base = New-Object System.Drawing.Drawing2D.LinearGradientBrush ((New-Object System.Drawing.PointF $gx, $gy)), ((New-Object System.Drawing.PointF ($gx + $gw), ($gy + $gh))), (C "e4ddff"), (C "8f7fe8")
  $g.FillPath($base, $gem)
  # play of colour: soft overlapping patches
  $g.SetClip($gem)
  foreach ($f in @(
      @(30, 50, 120, 100, "4fd8cf", 235), @(130, 40, 110, 100, "ff8fd0", 225), @(120, 120, 120, 100, "7a68f0", 235),
      @(40, 130, 110, 90, "ffd27a", 200), @(85, 85, 90, 70, "8fe8ff", 150),
      @(150, 110, 34, 22, "ffc0e8", 200), @(78, 140, 30, 20, "b8fff0", 200), @(165, 150, 22, 16, "fff0b0", 200))) {
    $fp = New-Object System.Drawing.Drawing2D.GraphicsPath; $fp.AddEllipse($f[0] * $k, $f[1] * $k, $f[2] * $k, $f[3] * $k)
    $fb = New-Object System.Drawing.Drawing2D.PathGradientBrush $fp; $fb.CenterColor = (C $f[4] $f[5]); $fb.SurroundColors = @((C $f[4] 0))
    $g.FillPath($fb, $fp)
  }
  # top-left highlight
  $hp = New-Object System.Drawing.Drawing2D.GraphicsPath; $hp.AddEllipse(70 * $k, 70 * $k, 82 * $k, 46 * $k)
  $hb = New-Object System.Drawing.Drawing2D.PathGradientBrush $hp; $hb.CenterColor = (C "ffffff" 230); $hb.SurroundColors = @((C "ffffff" 0))
  $g.FillPath($hb, $hp)
  $g.ResetClip()
  $edge = New-Object System.Drawing.Pen ((C "ffffff" 150)), (3 * $k)
  $g.DrawEllipse($edge, $gx, $gy, $gw, $gh)

  # Downscale
  $out = New-Object System.Drawing.Bitmap $S, $S, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $og = [System.Drawing.Graphics]::FromImage($out); $og.InterpolationMode = 'HighQualityBicubic'; $og.PixelOffsetMode = 'HighQuality'; $og.SmoothingMode = 'AntiAlias'
  $og.DrawImage($bmp, 0, 0, $S, $S)
  $bmp.Dispose(); $g.Dispose(); $og.Dispose()
  return $out
}

New-Item -ItemType Directory -Force $OutDir | Out-Null
$sizes = 16, 24, 32, 48, 64, 128, 256
$pngs = @{}
foreach ($s in $sizes) {
  $img = Draw $s
  $ms = New-Object System.IO.MemoryStream; $img.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png); $pngs[$s] = $ms.ToArray()
  if ($s -eq 256) { $img.Save((Join-Path $OutDir "icon.png"), [System.Drawing.Imaging.ImageFormat]::Png) }
  $img.Dispose()
}
# ICO with PNG-compressed entries
$fs = [System.IO.File]::Create((Join-Path $OutDir "icon.ico")); $bw = New-Object System.IO.BinaryWriter $fs
$bw.Write([uint16]0); $bw.Write([uint16]1); $bw.Write([uint16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
foreach ($s in $sizes) {
  $len = $pngs[$s].Length
  $bw.Write([byte]($(if ($s -ge 256) { 0 } else { $s }))); $bw.Write([byte]($(if ($s -ge 256) { 0 } else { $s })))
  $bw.Write([byte]0); $bw.Write([byte]0); $bw.Write([uint16]1); $bw.Write([uint16]32)
  $bw.Write([uint32]$len); $bw.Write([uint32]$offset); $offset += $len
}
foreach ($s in $sizes) { $bw.Write($pngs[$s]) }
$bw.Close()
"icon written: " + (Get-ChildItem $OutDir | ForEach-Object { "$($_.Name) $($_.Length)" }) -join ", "
