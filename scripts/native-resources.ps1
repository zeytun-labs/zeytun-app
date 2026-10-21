# scripts/native-resources.ps1
# Windows native resource preparation: zeytun-core.exe and geoip.mmdb
[CmdletBinding()]
param(
    [ValidateSet("resources", "build-core", "geoip", "check")]
    [string]$Action = "resources",
    [string]$CoreSrc = "",
    [string]$CoreRepo = "https://github.com/zeytun-labs/zeytun-core.git",
    [string]$CoreRef = "zeytun-port",
    [string]$CoreVersion = "zeytun-custom"
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$RootDir = Split-Path -Parent $ScriptDir
Set-Location $RootDir

$Target = "windows-x86_64"
$CoreDestDir = Join-Path $RootDir "src-tauri\resources\bin\$Target"
$CoreDest = Join-Path $CoreDestDir "zeytun-core.exe"
$GeoIpDest = Join-Path $RootDir "src-tauri\resources\geoip.mmdb"

if (-not $CoreSrc) {
    if (Test-Path "$RootDir\..\zeytun-core\cmd\sing-box\main.go") {
        $CoreSrc = (Resolve-Path "$RootDir\..\zeytun-core").Path
    } else {
        $CoreSrc = Join-Path $RootDir ".cache\zeytun-core"
    }
}

function Ensure-CoreSource {
    if (-not (Test-Path "$CoreSrc\go.mod")) {
        Write-Host "Cloning zeytun-core into $CoreSrc..." -ForegroundColor Cyan
        $cacheDir = Split-Path -Parent $CoreSrc
        if (-not (Test-Path $cacheDir)) {
            New-Item -ItemType Directory -Path $cacheDir -Force | Out-Null
        }
        git clone --depth 1 --branch $CoreRef $CoreRepo $CoreSrc
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to clone zeytun-core (exit code $LASTEXITCODE)."
        }
    }
    Write-Host "Core source: $CoreSrc" -ForegroundColor Green
}

function Build-Core {
    Ensure-CoreSource
    Write-Host "Building zeytun-core.exe for $Target..." -ForegroundColor Cyan

    $tags = "with_quic,with_dhcp,with_wireguard,with_utls,with_acme,with_clash_api,with_tailscale,with_ccm,with_ocm,with_cloudflared,with_naive_outbound,with_purego,with_usbip,with_openvpn,with_openconnect,badlinkname,tfogo_checklinkname0"
    $ldflags = "-X github.com/sagernet/sing-box/constant.Version=$CoreVersion -X runtime.godebugDefault=multipathtcp=0,tlssha1=1 -checklinkname=0 -s -w"

    if (-not (Test-Path $CoreDestDir)) {
        New-Item -ItemType Directory -Path $CoreDestDir -Force | Out-Null
    }

    Push-Location $CoreSrc
    try {
        $env:CGO_ENABLED = "0"
        $env:GOOS = "windows"
        $env:GOARCH = "amd64"
        go build -v -trimpath -tags $tags -ldflags $ldflags -o $CoreDest ./cmd/sing-box
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to build zeytun-core (exit code $LASTEXITCODE)."
        }
    } finally {
        Pop-Location
    }

    if (-not (Test-Path $CoreDest)) {
        throw "Failed to build $CoreDest"
    }
    Write-Host "Core successfully built: $CoreDest" -ForegroundColor Green
}

function Download-GeoIP {
    Write-Host "Downloading GeoIP database..." -ForegroundColor Cyan
    $geoipDir = Split-Path -Parent $GeoIpDest
    if (-not (Test-Path $geoipDir)) {
        New-Item -ItemType Directory -Path $geoipDir -Force | Out-Null
    }

    $yearMonth = (Get-Date).ToUniversalTime().ToString("yyyy-MM")
    $url = "https://download.db-ip.com/free/dbip-country-lite-$yearMonth.mmdb.gz"
    $gzPath = "$GeoIpDest.gz"

    try {
        Invoke-WebRequest -Uri $url -OutFile $gzPath -UseBasicParsing
    } catch {
        $prevMonth = (Get-Date).ToUniversalTime().AddMonths(-1).ToString("yyyy-MM")
        $url = "https://download.db-ip.com/free/dbip-country-lite-$prevMonth.mmdb.gz"
        Invoke-WebRequest -Uri $url -OutFile $gzPath -UseBasicParsing
    }

    $inFile = [System.IO.File]::OpenRead($gzPath)
    $outFile = [System.IO.File]::Create($GeoIpDest)
    $gzStream = New-Object System.IO.Compression.GZipStream($inFile, [System.IO.Compression.CompressionMode]::Decompress)
    $gzStream.CopyTo($outFile)
    $gzStream.Close()
    $outFile.Close()
    $inFile.Close()
    Remove-Item -Force $gzPath

    Write-Host "GeoIP successfully installed: $GeoIpDest" -ForegroundColor Green
}

function Check-Resources {
    if (-not (Test-Path $CoreDest)) {
        throw "Missing core executable: $CoreDest. Run with -Action build-core."
    }
    if (-not (Test-Path $GeoIpDest)) {
        throw "Missing GeoIP database: $GeoIpDest. Run with -Action geoip."
    }
    Write-Host "Native resources OK ($Target)" -ForegroundColor Green
}

switch ($Action) {
    "build-core" { Build-Core }
    "geoip"      { Download-GeoIP }
    "check"      { Check-Resources }
    "resources"  {
        if (-not (Test-Path $CoreDest)) { Build-Core }
        if (-not (Test-Path $GeoIpDest)) { Download-GeoIP }
        Check-Resources
    }
}
