# Install Chocolatey package manager
Set-ExecutionPolicy Bypass -Scope Process -Force
[System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072
Invoke-Expression ((New-Object System.Net.WebClient).DownloadString('https://chocolatey.org/install.ps1'))

# Create tools directory
$toolsPath = $env:TOOLS_PATH
if (-not $toolsPath) { $toolsPath = "C:\Tools" }
New-Item -ItemType Directory -Force -Path $toolsPath

# Install analysis tools
choco install -y `
    procmon `
    processhacker `
    wireshark `
    pestudio `
    x64dbg `
    python3 `
    sysinternals
