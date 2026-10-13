param(
    [Parameter(Position = 0)]
    [string]$Command = "help",

    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Rest = @()
)

$ErrorActionPreference = "Stop"

function Show-Usage {
    @'
Usage: proxy.ps1 <command> [options]

Commands:
  set [options] [HTTP_PROXY [SOCKS_PROXY]]
                      Save and apply a proxy configuration
    -Http <url>       HTTP/HTTPS proxy, for example http://127.0.0.1:10808
    -Socks <url>      SOCKS proxy, for example socks5://127.0.0.1:10808
    -NoProxy <list>   Comma-separated hosts which bypass the proxy
  ssh <host> <port>   Connect through the configured SOCKS/HTTP proxy
  clear               Remove all proxy settings
  show                Show the saved proxy configuration
  sync                Re-apply the saved configuration to the environment
  help                Show this help

The configuration is stored in %USERPROFILE%\.config\proxy\config and is
applied through environment variables only; no per-tool proxy configuration
is written.
'@ | Write-Host
}

function Get-ProxyConfigurationPath {
    $configHome = Join-Path $env:USERPROFILE ".config"
    Join-Path $configHome "proxy\config"
}

function Assert-ProxyUrl {
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string]$Value,

        [Parameter(Mandatory = $true)]
        [ValidateSet("HTTP_PROXY", "SOCKS_PROXY")]
        [string]$Kind
    )

    if ([string]::IsNullOrEmpty($Value)) {
        return
    }
    if ($Value -match '\s|["'']|\\') {
        throw "$Kind must not contain whitespace, quotes, or backslashes"
    }

    $allowedSchemes = if ($Kind -eq "SOCKS_PROXY") {
        @("socks5", "socks5h")
    } else {
        @("http", "https")
    }
    try {
        $uri = [Uri]$Value
        if (-not $uri.IsAbsoluteUri -or $allowedSchemes -notcontains $uri.Scheme) {
            throw "$Kind must use one of: $($allowedSchemes -join '://, ')://"
        }
        if ([string]::IsNullOrWhiteSpace($uri.Host)) {
            throw "$Kind must contain a host"
        }
    } catch [FormatException] {
        throw "$Kind is not a valid URL"
    }
}

function Read-ProxyConfiguration {
    param([Parameter(Mandatory = $true)][string]$Path)

    $values = @{
        Http    = ""
        Socks   = ""
        NoProxy = ""
    }
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "No proxy configuration found at $Path"
    }

    foreach ($line in Get-Content -LiteralPath $Path) {
        if ($line -match "^\s*PROXY_HTTP='(.*)'\s*$") {
            $values.Http = $Matches[1]
        } elseif ($line -match "^\s*PROXY_SOCKS='(.*)'\s*$") {
            $values.Socks = $Matches[1]
        } elseif ($line -match "^\s*PROXY_NO_PROXY='(.*)'\s*$") {
            $values.NoProxy = $Matches[1]
        }
    }
    $values
}

function Write-ProxyConfiguration {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Http,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Socks,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$NoProxy
    )

    $directory = Split-Path -Parent $Path
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
    $temporary = Join-Path $directory ".config.$([IO.Path]::GetRandomFileName())"
    try {
        @(
            "PROXY_HTTP='$Http'"
            "PROXY_SOCKS='$Socks'"
            "PROXY_NO_PROXY='$NoProxy'"
        ) | Set-Content -LiteralPath $temporary -Encoding ascii
        Move-Item -LiteralPath $temporary -Destination $Path -Force
    } finally {
        if (Test-Path -LiteralPath $temporary) {
            Remove-Item -LiteralPath $temporary -Force
        }
    }
}

function Set-ManagedEnvironment {
    param([Parameter(Mandatory = $true)][hashtable]$Values)

    $entries = [ordered]@{
        HTTP_PROXY  = [string]$Values.Http
        HTTPS_PROXY = [string]$Values.Http
        ALL_PROXY   = [string]$Values.Socks
        NO_PROXY    = $null
    }
    if ($Values.Http -or $Values.Socks) {
        $entries.NO_PROXY = if ($Values.NoProxy) { $Values.NoProxy } else { "localhost,127.0.0.1,::1" }
    }

    foreach ($entry in $entries.GetEnumerator()) {
        $value = [string]$entry.Value
        [Environment]::SetEnvironmentVariable($entry.Key, $entry.Value, "User")
        if ([string]::IsNullOrEmpty($value)) {
            Remove-Item -Path "Env:\$($entry.Key)" -ErrorAction SilentlyContinue
        } else {
            Set-Item -Path "Env:\$($entry.Key)" -Value $value
        }
    }
}

function Invoke-ProxySet {
    $http = ""
    $socks = ""
    $noProxy = ""

    $i = 0
    while ($i -lt $Rest.Count) {
        $argument = $Rest[$i]
        switch ($argument) {
            { $_ -in "-Http", "--http" } {
                $i++
                if ($i -ge $Rest.Count) {
                    throw "-Http requires a value"
                }
                $http = $Rest[$i]
            }
            { $_ -in "-Socks", "--socks" } {
                $i++
                if ($i -ge $Rest.Count) {
                    throw "-Socks requires a value"
                }
                $socks = $Rest[$i]
            }
            { $_ -in "-NoProxy", "--no-proxy" } {
                $i++
                if ($i -ge $Rest.Count) {
                    throw "-NoProxy requires a value"
                }
                $noProxy = $Rest[$i]
            }
            default {
                if (-not $http) {
                    $http = $argument
                } elseif (-not $socks) {
                    $socks = $argument
                } else {
                    throw "Unexpected argument: $argument"
                }
            }
        }
        $i++
    }

    if (-not ($http -or $socks)) {
        Show-Usage
        exit 2
    }

    $configPath = Get-ProxyConfigurationPath
    if (Test-Path -LiteralPath $configPath -PathType Leaf) {
        $existing = Read-ProxyConfiguration -Path $configPath
        if (-not $http) {
            $http = $existing.Http
        }
        if (-not $socks) {
            $socks = $existing.Socks
        }
        if (-not $noProxy) {
            $noProxy = $existing.NoProxy
        }
    }
    Assert-ProxyUrl -Value $http -Kind HTTP_PROXY
    Assert-ProxyUrl -Value $socks -Kind SOCKS_PROXY
    if (-not $noProxy) {
        $noProxy = "localhost,127.0.0.1,::1"
    }
    if ($noProxy -match '[\r\n]') {
        throw "-NoProxy must be a single comma-separated line"
    }
    Write-ProxyConfiguration -Path $configPath -Http $http -Socks $socks -NoProxy $noProxy
    $values = Read-ProxyConfiguration -Path $configPath
    Set-ManagedEnvironment -Values $values
    Write-Host "Proxy configuration: $configPath"
}

function Invoke-ProxySsh {
    if ($Rest.Count -ne 2) {
        Write-Host "usage: proxy.ps1 ssh <host> <port>" >&2
        exit 2
    }
    $targetHost = $Rest[0]
    $port = $Rest[1]

    $configPath = Get-ProxyConfigurationPath
    $proxy = ""
    if (Test-Path -LiteralPath $configPath -PathType Leaf) {
        foreach ($line in Get-Content -LiteralPath $configPath) {
            if ($line -match "^\s*PROXY_SOCKS='(.*)'\s*$") {
                $proxy = $Matches[1]
                break
            }
        }
    }

    if (-not $proxy) {
        & ncat.exe $targetHost $port
        exit $LASTEXITCODE
    }

    $proxyAddress = $proxy -replace '^[a-z0-9+.-]+://', ''
    $proxyType = if ($proxy -match '^https?://') { "http" } else { "socks5" }
    & ncat.exe --proxy-type $proxyType --proxy $proxyAddress $targetHost $port
    exit $LASTEXITCODE
}

$configPath = Get-ProxyConfigurationPath

switch ($Command) {
    "set" {
        Invoke-ProxySet
    }
    "ssh" {
        Invoke-ProxySsh
    }
    "clear" {
        Write-ProxyConfiguration -Path $configPath -Http "" -Socks "" -NoProxy ""
        $values = Read-ProxyConfiguration -Path $configPath
        Set-ManagedEnvironment -Values $values
        Write-Host "Proxy configuration: $configPath"
    }
    "show" {
        Get-Content -LiteralPath $configPath -ErrorAction Stop
    }
    "sync" {
        $values = Read-ProxyConfiguration -Path $configPath
        Set-ManagedEnvironment -Values $values
        Write-Host "Proxy configuration: $configPath"
    }
    "help" {
        Show-Usage
    }
    default {
        Write-Host "Unknown command: $Command" >&2
        Write-Host "Run 'proxy.ps1 help' for usage." >&2
        exit 2
    }
}
