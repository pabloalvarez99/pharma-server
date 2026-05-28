#Requires -Version 5.1
<#
.SYNOPSIS
    Generate launcher/pharma.ico — green rounded tile + white medical cross.

.DESCRIPTION
    Renders crisp PNG frames (256, 48, 32, 16) and packs them into a multi-size
    .ico (PNG-compressed frames, Vista+). No external assets or tools.

.PARAMETER OutPath
    Destination .ico. Default: launcher/pharma.ico next to this script.
#>
[CmdletBinding()]
param(
    [string]$OutPath = (Join-Path $PSScriptRoot "pharma.ico")
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

$green   = [System.Drawing.Color]::FromArgb(16, 185, 129)
$greenLo = [System.Drawing.Color]::FromArgb(5, 150, 105)
$white   = [System.Drawing.Color]::White

function New-FramePng([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode     = 'AntiAlias'
    $g.InterpolationMode = 'HighQualityBicubic'
    $g.Clear([System.Drawing.Color]::Transparent)

    # Rounded-rectangle tile with a subtle vertical gradient.
    $pad    = [Math]::Max(1, [int]($size * 0.06))
    $radius = [int]($size * 0.22)
    $rect   = New-Object System.Drawing.Rectangle $pad, $pad, ($size - 2*$pad), ($size - 2*$pad)

    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = $radius * 2
    $path.AddArc($rect.X, $rect.Y, $d, $d, 180, 90)
    $path.AddArc($rect.Right - $d, $rect.Y, $d, $d, 270, 90)
    $path.AddArc($rect.Right - $d, $rect.Bottom - $d, $d, $d, 0, 90)
    $path.AddArc($rect.X, $rect.Bottom - $d, $d, $d, 90, 90)
    $path.CloseFigure()

    $grad = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        $rect, $green, $greenLo, [System.Drawing.Drawing2D.LinearGradientMode]::Vertical)
    $g.FillPath($grad, $path)

    # White medical cross, centered.
    $cw = [int]($size * 0.14)          # arm thickness
    $cl = [int]($size * 0.46)          # arm length
    $cx = [int]($size / 2)
    $cy = [int]($size / 2)
    $wb = New-Object System.Drawing.SolidBrush $white
    $g.FillRectangle($wb, ($cx - $cw/2), ($cy - $cl/2), $cw, $cl)   # vertical
    $g.FillRectangle($wb, ($cx - $cl/2), ($cy - $cw/2), $cl, $cw)   # horizontal

    $g.Dispose()

    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    return ,$ms.ToArray()
}

$sizes  = @(256, 48, 32, 16)
$frames = @{}
foreach ($s in $sizes) { $frames[$s] = New-FramePng $s }

# --- Assemble ICO ---------------------------------------------------------
# ICONDIR (6) + N * ICONDIRENTRY (16) + image blobs (PNG).
$fs = New-Object System.IO.MemoryStream
$bw = New-Object System.IO.BinaryWriter $fs

$count = $sizes.Count
$bw.Write([UInt16]0)        # reserved
$bw.Write([UInt16]1)        # type: 1 = icon
$bw.Write([UInt16]$count)   # image count

$offset = 6 + (16 * $count)
foreach ($s in $sizes) {
    $data = $frames[$s]
    $dim = if ($s -ge 256) { 0 } else { $s }   # 0 means 256 in ICO spec
    $bw.Write([Byte]$dim)            # width
    $bw.Write([Byte]$dim)            # height
    $bw.Write([Byte]0)               # palette count
    $bw.Write([Byte]0)               # reserved
    $bw.Write([UInt16]1)             # color planes
    $bw.Write([UInt16]32)            # bits per pixel
    $bw.Write([UInt32]$data.Length)  # bytes in resource
    $bw.Write([UInt32]$offset)       # offset
    $offset += $data.Length
}
foreach ($s in $sizes) { $bw.Write($frames[$s]) }

$bw.Flush()
[System.IO.File]::WriteAllBytes($OutPath, $fs.ToArray())
$bw.Dispose(); $fs.Dispose()

Write-Host "Wrote icon: $OutPath ($([math]::Round((Get-Item $OutPath).Length/1kb,1)) KB, $count frames)"
