[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$Exe,
  [string]$WorkDir = "",
  [int]$TimeoutSec = 20,
  [int]$SettleMs = 3000,
  [string]$Env = "",
  [string]$Shot = "",
  [string]$Shot2 = "",
  [string]$Click = "",          # "x,y" window-relative, sent after settling
  [string]$LogPath = "",
  [string]$Label = ""
)
# gui-probe.ps1 - launch a Sweep GUI build and report, mechanically, whether a
# window actually appears. Writes one JSON line per attempt so the history can
# be compared later (see diagnostics/gui-launch-log.jsonl).
$ErrorActionPreference = 'Continue'
if (-not $WorkDir) { $WorkDir = Split-Path $Exe }
$envMap = @{}
if ($Env) { foreach ($pair in $Env.Split(';')) { if ($pair -match '=') { $kv = $pair.Split('=',2); $envMap[$kv[0]] = $kv[1] } } }

Add-Type -AssemblyName System.Drawing
Add-Type -Namespace W -Name U -MemberDefinition @"
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, IntPtr e);
"@

function GrabShots($hwnd, $path) {
  [void][W.U]::SetForegroundWindow($hwnd)
  Start-Sleep -Milliseconds 500
  $r = New-Object W.U+RECT
  [void][W.U]::GetWindowRect($hwnd, [ref]$r)
  $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
  if ($w -lt 50 -or $h -lt 50) { return $null }
  $bmp = New-Object System.Drawing.Bitmap($w, $h)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
  return "$($r.Left),$($r.Top),$w,$h"
}

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $Exe
$psi.WorkingDirectory = $WorkDir
$psi.UseShellExecute = $false
$psi.CreateNoWindow = $true
foreach ($k in $envMap.Keys) { $psi.EnvironmentVariables[$k] = $envMap[$k] }
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$p = [System.Diagnostics.Process]::Start($psi)
$hwnd = [IntPtr]::Zero
$deadline = (Get-Date).AddSeconds($TimeoutSec)
while ((Get-Date) -lt $deadline) {
  Start-Sleep -Milliseconds 300
  if ($p.HasExited) { break }
  $p.Refresh()
  if ($p.MainWindowHandle -ne [IntPtr]::Zero) { $hwnd = $p.MainWindowHandle; break }
}
$p.Refresh()
$state = "RUNNING_NO_WINDOW"
if ($p.HasExited) { $state = "EXITED" } elseif ($hwnd -ne [IntPtr]::Zero) { $state = "WINDOW" }
$exit = $null; if ($p.HasExited) { $exit = $p.ExitCode }
$title = ""; if ($hwnd -ne [IntPtr]::Zero) { $title = $p.MainWindowTitle }
$rect = ""
$shotOk = $false
$shot2Ok = $false
$clickOk = $false
if ($hwnd -ne [IntPtr]::Zero) {
  Start-Sleep -Milliseconds $SettleMs
  if ($Shot) { $rect = GrabShots $hwnd $Shot; if ($rect) { $shotOk = $true } }
  if ($Click -and $Shot2) {
    $parts = $Click.Split(',')
    $r = New-Object W.U+RECT
    [void][W.U]::GetWindowRect($hwnd, [ref]$r)
    [void][W.U]::SetForegroundWindow($hwnd)
    Start-Sleep -Milliseconds 400
    [void][W.U]::SetCursorPos($r.Left + [int]$parts[0], $r.Top + [int]$parts[1])
    Start-Sleep -Milliseconds 250
    [W.U]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)   # left down
    Start-Sleep -Milliseconds 80
    [W.U]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)   # left up
    $clickOk = $true
    Start-Sleep -Milliseconds 2500
    $rect2 = GrabShots $hwnd $Shot2
    if ($rect2) { $shot2Ok = $true }
  }
}
$sw.Stop()
$result = [ordered]@{
  ts = (Get-Date).ToString("yyyy-MM-dd HH:mm:ss")
  label = $Label
  exe = $Exe
  env = $Env
  state = $state
  exitCode = $exit
  windowTitle = $title
  hwnd = $hwnd.ToString()
  screenshot = if ($shotOk) { $Shot } else { "" }
  screenshot2 = if ($shot2Ok) { $Shot2 } else { "" }
  clicked = $clickOk
  windowRect = $rect
  elapsedMs = $sw.ElapsedMilliseconds
}
if ($LogPath) { ($result | ConvertTo-Json -Compress) | Add-Content -Path $LogPath -Encoding UTF8 }
Write-Host ("PROBE {0} | {1} | {2} | exit={3} | title='{4}' | hwnd={5} | shot={6} | shot2={7} | clicked={8} | {9}ms" -f $result.ts, $Label, $state, $exit, $title, $hwnd, $shotOk, $shot2Ok, $clickOk, $sw.ElapsedMilliseconds)
if (-not $p.HasExited) { $p.Kill() }
if ($state -eq "WINDOW") { exit 0 } else { exit 1 }
