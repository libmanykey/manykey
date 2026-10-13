# Builds the extension and copies it into the demo project's addon folder.
# Usage:  .\build.ps1            (debug build)
#         .\build.ps1 release
param([ValidateSet("debug", "release")] [string]$Profile = "debug")
$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Push-Location $root
try {
    if ($Profile -eq "release") { cargo build -p manykey-godot --release } else { cargo build -p manykey-godot }
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
} finally { Pop-Location }

$dest = Join-Path $PSScriptRoot "demo\addons\manykey\bin\$Profile"
New-Item -ItemType Directory -Force $dest | Out-Null
Copy-Item (Join-Path $root "target\$Profile\manykey_godot.dll") $dest -Force
Write-Host "Copied manykey_godot.dll to $dest"
