$ErrorActionPreference = "Stop"

$nvimMsi = Join-Path $env:TEMP "nvim-win64.msi"
$nvimDir = Join-Path $env:LOCALAPPDATA "Neovim"
$stampFile = Join-Path $nvimDir ".nightly-stamp"

Write-Host "==> Checking remote Neovim nightly..."
$head = Invoke-WebRequest -Uri "https://github.com/neovim/neovim/releases/download/nightly/nvim-win64.msi" -Method Head
$remoteStamp = $head.Headers['Last-Modified']
if ($remoteStamp -is [array]) {
    $remoteStamp = $remoteStamp[0]
}

if (Test-Path $stampFile) {
    $localStamp = Get-Content -LiteralPath $stampFile -Raw
    if ($localStamp.Trim() -eq $remoteStamp) {
        Write-Host "==> Neovim nightly is already up to date."
        return
    }
}

Write-Host "==> Downloading Neovim nightly..."
Invoke-WebRequest -Uri "https://github.com/neovim/neovim/releases/download/nightly/nvim-win64.msi" -OutFile $nvimMsi

Write-Host "==> Installing Neovim nightly to $nvimDir..."
$process = Start-Process msiexec -ArgumentList "/i", "`"$nvimMsi`"", "/passive", "INSTALLDIR=`"$nvimDir`"" -Wait -PassThru
if ($process.ExitCode -ne 0 -and $process.ExitCode -ne 3010) {
    Remove-Item $nvimMsi -Force -ErrorAction SilentlyContinue
    throw "Neovim nightly installation failed with exit code $($process.ExitCode)"
}

Remove-Item $nvimMsi -Force

New-Item -ItemType Directory -Path $nvimDir -Force | Out-Null
Set-Content -LiteralPath $stampFile -Value $remoteStamp -NoNewline

$nvimBin = Join-Path $nvimDir "bin"
$currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($currentPath -notlike "*$nvimBin*") {
    [Environment]::SetEnvironmentVariable("Path", "$currentPath;$nvimBin", "User")
    Write-Host "==> Added $nvimBin to user PATH."
}

Write-Host "==> Neovim nightly installed successfully."
