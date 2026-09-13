<#
.SYNOPSIS
    Sign a locally built sweep.exe so Windows SmartScreen stops warning.

.DESCRIPTION
    SmartScreen shows "Windows protected your PC" because a freshly built .exe
    is UNSIGNED and therefore has no reputation. This script removes that
    warning on THIS machine by:

      1. creating (once) a self-signed CODE SIGNING certificate,
      2. trusting it in your personal Trusted Root and Trusted Publishers
         stores,
      3. authenticoding every built binary it finds.

    Scope: local development only. A self-signed certificate convinces nobody
    but you - public releases need a certificate from a real CA (see
    packaging/windows/README.md). Adding a certificate to Trusted Root is a
    security decision: it lets that certificate sign anything on this machine,
    so run -Remove when you no longer need it.

.PARAMETER Binaries
    Candidates to sign; whichever exist are signed, the rest are skipped.

.PARAMETER Remove
    Delete the "Sweep Dev Signing" certificate from all three stores instead of
    signing anything.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File packaging/windows/sign-dev.ps1

    Signs target/.../sweep.exe and the Qt GUI if they exist.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File packaging/windows/sign-dev.ps1 -Remove

    Removes the development certificate again.
#>
[CmdletBinding()]
param(
    [string[]]$Binaries = @(
        'target/release/sweep.exe',
        'target/x86_64-pc-windows-gnu/release/sweep.exe',
        'target/x86_64-pc-windows-msvc/release/sweep.exe',
        'target/debug/sweep.exe',
        'packaging/qml/build/Release/sweep-qml.exe',
        'packaging/qml/build-mingw/sweep-qml.exe'
    ),
    [switch]$Remove
)

$ErrorActionPreference = 'Stop'
$CertSubject = 'CN=Sweep Dev Signing'
$TimestampServer = 'http://timestamp.digicert.com'

# The script lives in <repo>/packaging/windows, so the repo root is two up.
$RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)

function Get-DevCertificate {
    return Get-ChildItem 'Cert:\CurrentUser\My' -ErrorAction SilentlyContinue |
        Where-Object { $_.Subject -eq $CertSubject } |
        Sort-Object NotAfter -Descending |
        Select-Object -First 1
}

function New-DevCertificate {
    Write-Host "Creating a self-signed code-signing certificate ($CertSubject)..."
    $cert = New-SelfSignedCertificate `
        -Type CodeSigningCert `
        -Subject $CertSubject `
        -CertStoreLocation 'Cert:\CurrentUser\My' `
        -KeyAlgorithm RSA -KeyLength 2048 -HashAlgorithm SHA256 `
        -NotAfter (Get-Date).AddYears(3)

    return $cert
}

function Ensure-DevCertificateTrusted {
    param([System.Security.Cryptography.X509Certificates.X509Certificate2]$Cert)

    # Publish the PUBLIC key only; the private key never leaves the My store.
    # Importing the same certificate twice is harmless, so this runs every time.
    $cer = Join-Path ([IO.Path]::GetTempPath()) 'sweep-dev-signing.cer'
    Export-Certificate -Cert $Cert -FilePath $cer -Force | Out-Null
    foreach ($store in 'Root', 'TrustedPublisher') {
        Import-Certificate -FilePath $cer -CertStoreLocation "Cert:\CurrentUser\$store" |
            Out-Null
        Write-Host "  trusted in Cert:\CurrentUser\$store"
    }
    Remove-Item $cer -Force
}

function Remove-DevCertificate {
    $found = $false
    foreach ($store in 'My', 'Root', 'TrustedPublisher') {
        Get-ChildItem "Cert:\CurrentUser\$store" -ErrorAction SilentlyContinue |
            Where-Object { $_.Subject -eq $CertSubject } |
            ForEach-Object {
                Write-Host "  removing from Cert:\CurrentUser\$store"
                Remove-Item $_.PSPath -Force
                $found = $true
            }
    }
    if (-not $found) { Write-Host 'No development certificate found.' }
}

if ($Remove) {
    Remove-DevCertificate
    Write-Host 'Done. SmartScreen will warn again for unsigned builds.'
    return
}

$cert = Get-DevCertificate
if (-not $cert) { $cert = New-DevCertificate }
else { Write-Host "Reusing existing certificate $($cert.Thumbprint)" }

# Trust has to be (re-)asserted on every run: a certificate already present in
# My may never have been added to Root, and an untrusted root makes
# Set-AuthenticodeSignature fail with UnknownError instead of a clear message.
Ensure-DevCertificateTrusted $cert

$targets = @($Binaries)
# A released installer needs signing just as much as the binary - find it
# whatever the version is, so this does not go stale on every bump.
$setupDir = Join-Path $RepoRoot 'packaging/dist/windows'
if (Test-Path $setupDir) {
    $targets += Get-ChildItem -Path $setupDir -Filter 'Sweep-*-Setup.exe' -File |
        ForEach-Object { $_.FullName.Substring($RepoRoot.Length + 1) }
}

$signed = 0
foreach ($relative in $targets) {
    $path = Join-Path $RepoRoot $relative
    if (-not (Test-Path $path)) { continue }

    Write-Host "Signing $relative"
    Set-AuthenticodeSignature -FilePath $path -Certificate $cert `
        -TimestampServer $TimestampServer -HashAlgorithm SHA256 | Out-Null
    $status = (Get-AuthenticodeSignature -FilePath $path).Status

    # A timestamp needs outbound access to a public TSA. Without one the
    # signature is still perfectly valid - it just expires with the
    # certificate instead of outliving it - so offline machines get an
    # untimestamped signature rather than a hard failure.
    if ($status -ne 'Valid') {
        Write-Host "  timestamped signing was '$status'; retrying without a timestamp"
        Set-AuthenticodeSignature -FilePath $path -Certificate $cert `
            -HashAlgorithm SHA256 | Out-Null
        $status = (Get-AuthenticodeSignature -FilePath $path).Status
    }
    if ($status -ne 'Valid') {
        throw "$relative is '$status' after signing. Check the file is a Windows PE and that the certificate allows code signing."
    }
    Write-Host "  status: $status"
    $signed++
}

if ($signed -eq 0) {
    Write-Host ''
    Write-Host 'No binaries found. Build first:'
    Write-Host '  cargo build --release'
    Write-Host '(and for the GUI: cmake --build packaging/qml/build --config Release)'
    return
}

Write-Host ''
Write-Host "Signed $signed binary/binaries. SmartScreen should no longer warn."
Write-Host "If a file was already downloaded, also run: Unblock-File <path>"
Write-Host "Undo with: powershell -ExecutionPolicy Bypass -File $PSCommandPath -Remove"
