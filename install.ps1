# PowerShell Installer for purecode
$ErrorActionPreference = "Stop"
# Windows PowerShell 5.1 may default to older TLS versions.
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$Repo = "isupervillain/purecode"
$GitHubUrl = "https://github.com/$Repo/releases/download"
$InstallDir = "$env:USERPROFILE\.purecode\bin"

# --- Architecture Detection ---
$Arch = $env:PROCESSOR_ARCHITECTURE
if ($Arch -eq "AMD64") {
    $Target = "x86_64-pc-windows-msvc"
} else {
    Write-Error "Unsupported architecture: $Arch. Only x86_64 is currently supported."
    exit 1
}

# --- Version Detection ---
Write-Host "Detecting latest version..."
try {
    # Fetch latest release info from GitHub API
    # Note: Using unauthenticated API might hit rate limits, but usually fine for installers.
    # Alternative: Parse the HTML redirect like shell script.
    # Let's try the redirect method as it's often more robust against rate limits for anonymous IPs.
    $LatestUrl = "https://github.com/$Repo/releases/latest"
    $Request = [System.Net.WebRequest]::Create($LatestUrl)
    $Request.Method = "HEAD"
    $Request.AllowAutoRedirect = $false
    try {
        $Response = $Request.GetResponse()
        # Should be a 302 Found
        $FinalUrl = $Response.Headers["Location"]
    } catch [System.Net.WebException] {
        # If it throws on 302 (Powershell behavior varies), catch it.
        if ($_.Exception.Response.StatusCode -eq [System.Net.HttpStatusCode]::Found) {
            $FinalUrl = $_.Exception.Response.Headers["Location"]
        } else {
            throw $_
        }
    }

    # URL format: .../releases/tag/vX.Y.Z
    $VersionTag = $FinalUrl.Split('/')[-1]
} catch {
    Write-Warning "Could not detect version via redirect. Defaulting to 'latest' logic might fail if assets aren't predictable."
    Write-Error "Failed to check latest version: $_"
    exit 1
}

Write-Host "Latest version: $VersionTag"

$AssetName = "purecode-${Target}.zip"
$DownloadUrl = "$GitHubUrl/$VersionTag/$AssetName"

# --- Install ---
if (!(Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
}

$ZipPath = "$env:TEMP\$AssetName"
$SumsPath = "$env:TEMP\purecode-SHA256SUMS"
Write-Host "Downloading $DownloadUrl ..."
Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath
Invoke-WebRequest -Uri "$GitHubUrl/$VersionTag/SHA256SUMS" -OutFile $SumsPath

# --- Verify ---
$Expected = (Get-Content $SumsPath | Where-Object { $_ -match "\s$([regex]::Escape($AssetName))$" } | ForEach-Object { ($_ -split '\s+')[0] }) | Select-Object -First 1
$Actual = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash
Remove-Item -Path $SumsPath -Force
if (-not $Expected -or $Expected.ToLower() -ne $Actual.ToLower()) {
    Remove-Item -Path $ZipPath -Force
    Write-Error "Checksum verification failed for $AssetName."
    exit 1
}
Write-Host "Checksum verified."

Write-Host "Extracting to $InstallDir ..."
Expand-Archive -Path $ZipPath -DestinationPath $InstallDir -Force

# Cleanup
Remove-Item -Path $ZipPath -Force

# --- Path ---
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Host "Adding $InstallDir to User PATH..."
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    Write-Host "Added to PATH. Please restart your terminal."
} else {
    Write-Host "Install directory is already in PATH."
}

Write-Host "Successfully installed purecode!"
