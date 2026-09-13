[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$Exe,
  [string]$WorkDir = "",
  [int]$TimeoutSec = 20,
  [int]$SettleMs = 4000,
  [Parameter(Mandatory=$true)][string]$Out,
  [string]$LogPath = "",
  [string]$Label = "capture"
)
# gui-capture.ps1 - capture the GUI window's OWN content with PrintWindow, so
# the screenshot does not depend on what happens to be on top of it.
$ErrorActionPreference = 'Continue'
if (-not $WorkDir) { $WorkDir = Split-Path $Exe }
Add-Type -AssemblyName System.Drawing
Add-Type -Namespace PW -Name W -MemberDefinition @"
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
[DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
"@
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $Exe; $psi.WorkingDirectory = $WorkDir; $psi.UseShellExecute = $false
$p = [System.Diagnostics.Process]::Start($psi)
$hwnd = [IntPtr]::Zero
$deadline = (Get-Date).AddSeconds($TimeoutSec)
while ((Get-Date) -lt $deadline) {
  Start-Sleep -Milliseconds 300
  if ($p.HasExited) { break }
  $p.Refresh()
  if ($p.MainWindowHandle -ne [IntPtr]::Zero) { $hwnd = $p.MainWindowHandle; break }
}
if ($hwnd -eq [IntPtr]::Zero) {
  Write-Host "NO WINDOW (hasExited=$($p.HasExited))"
  if (-not $p.HasExited) { $p.Kill() }
  exit 1
}
Start-Sleep -Milliseconds $SettleMs
$r = New-Object PW.W+RECT
[void][PW.W]::GetWindowRect($hwnd, [ref]$r)
$w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
$bmp = New-Object System.Drawing.Bitmap($w, $h)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
# 2 = PW_RENDERFULLCONTENT (needed for DirectComposition/DWM windows)
$ok = [PW.W]::PrintWindow($hwnd, $hdc, 2)
$g.ReleaseHdc($hdc)
$g.Dispose()
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Host ("CAPTURE {0} | title='{1}' | hwnd={2} | rect={3},{4},{5},{6} | PrintWindow={7} | saved={8}" -f (Get-Date).ToString('yyyy-MM-dd HH:mm:ss'), $p.MainWindowTitle, $hwnd, $r.Left, $r.Top, $w, $h, $ok, $Out)
if ($LogPath) {
  $line = ([ordered]@{ ts=(Get-Date).ToString('yyyy-MM-dd HH:mm:ss'); label=$Label; exe=$Exe; state='WINDOW'; windowTitle=$p.MainWindowTitle; hwnd=$hwnd.ToString(); printWindow=$ok; screenshot=$Out; rect="$($r.Left),$($r.Top),$w,$h" } | ConvertTo-Json -Compress)
  Add-Content -Path $LogPath -Value $line -Encoding UTF8
}
[void]$p.CloseMainWindow()
Start-Sleep -Milliseconds 800
if (-not $p.HasExited) { $p.Kill() }
exit 0
