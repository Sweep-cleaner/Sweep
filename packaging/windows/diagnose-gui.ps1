#!PowerShell
<#
.SYNOPSIS
    Find out which startup path makes the Sweep GUI crash.

.DESCRIPTION
    Runs sweep-qml.exe several times, each time switching off one of the
    display-only startup paths, and reports which runs survived:
        (none)            everything on
        SWEEP_NO_TRAY     skip the system tray (Shell_NotifyIcon)
        SWEEP_SOFTWARE    software scene graph instead of the GPU
        SWEEP_NO_MICA     skip the dark title bar / Mica DWM effects
        all three         everything display-specific off
    Whichever configuration survives identifies the culprit.

    Run this on a machine WITH a display; it is useless headless.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File packaging\windows\diagnose-gui.ps1
#>
[CmdletBinding()]
param(
    [int]$Seconds = 6,
    [string]$Exe = ""
)

$ErrorActionPreference = 'Continue'
$RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)

if (-not $Exe) {
    $candidates = @(
        (Join-Path $RepoRoot 'packaging/dist/windows/stage/sweep-qml.exe'),
        (Join-Path $RepoRoot 'packaging/qml/build-mingw/sweep-qml.exe'),
        (Join-Path $env:ProgramFiles 'Sweep\sweep-qml.exe')
    )
    foreach ($c in $candidates) {
        if (Test-Path $c) { $Exe = $c; break }
    }
}
if (-not $Exe -or -not (Test-Path $Exe)) {
    Write-Host "sweep-qml.exe not found. Pass -Exe <path>." -ForegroundColor Red
    exit 1
}
Write-Host "Testing: $Exe" -ForegroundColor Cyan
Write-Host "(each run gets $Seconds seconds; 'survived' means it was still running)`n"

$cases = @(
    @{ Name = 'baseline (nothing disabled)'; Vars = @{} },
    @{ Name = 'QT_OPENGL=software';          Vars = @{ QT_OPENGL = 'software' } },
    @{ Name = 'QSG_RENDER_BACKEND=software'; Vars = @{ QSG_RENDER_BACKEND = 'software' } },
    @{ Name = 'SWEEP_SOFTWARE=1';            Vars = @{ SWEEP_SOFTWARE = '1' } },
    @{ Name = 'SWEEP_NO_TRAY=1';             Vars = @{ SWEEP_NO_TRAY = '1' } },
    @{ Name = 'SWEEP_NO_MICA=1';             Vars = @{ SWEEP_NO_MICA = '1' } },
    @{ Name = 'everything off';              Vars = @{ QT_OPENGL = 'software'; SWEEP_NO_TRAY = '1'; SWEEP_NO_MICA = '1' } }
)
# Note: Qt 6.8.1's win64_mingw build ships NO ANGLE (no libEGL/libGLESv2),
# so Qt Quick must use the machine's native OpenGL. A broken/missing GL driver
# is therefore the prime suspect - which is why QT_OPENGL=software (Mesa via
# the deployed opengl32sw.dll) is tried first.

foreach ($case in $cases) {
    foreach ($k in $case.Vars.Keys) { Set-Item "env:$k" $case.Vars[$k] }

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $Exe
    $psi.UseShellExecute = $false
    $proc = [System.Diagnostics.Process]::Start($psi)
    $exited = $proc.WaitForExit($Seconds * 1000)
    if (-not $exited) {
        $proc.Kill()
        $proc.WaitForExit(3000) | Out-Null
        $code = 'running'
    } else {
        $code = $proc.ExitCode
    }
    $proc.Dispose()

    if ($code -eq 'running') {
        Write-Host ("  SURVIVED  {0}" -f $case.Name) -ForegroundColor Green
    } else {
        Write-Host ("  EXIT {0,-8} {1}" -f $code, $case.Name) -ForegroundColor Yellow
    }

    foreach ($k in $case.Vars.Keys) { Remove-Item "env:$k" -ErrorAction SilentlyContinue }
}

Write-Host ""
Write-Host "Reading: the first configuration that SURVIVED tells you the culprit."
Write-Host "  SWEEP_NO_TRAY     -> system tray (Shell_NotifyIcon)"
Write-Host "  SWEEP_SOFTWARE    -> GPU / Qt Quick scene graph"
Write-Host "  SWEEP_NO_MICA     -> DWM dark title bar / Mica"
Write-Host "If only 'all three' survives, several paths are involved."
Write-Host "If even that exits, the crash is elsewhere - send this output along."
