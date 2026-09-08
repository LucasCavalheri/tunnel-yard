# Extract the official client for localhost-only tests; do not install anything.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$metadata = Get-Content (Join-Path $root 'packaging/windows-client.json') -Raw | ConvertFrom-Json
$scratch = Join-Path $root '.tmp'
New-Item -ItemType Directory -Force -Path $scratch | Out-Null
$installer = Join-Path $scratch 'openconnect-installer.exe'
Invoke-WebRequest $metadata.url -OutFile $installer
if ((Get-FileHash $installer -Algorithm SHA256).Hash.ToLowerInvariant() -ne $metadata.sha256) { throw 'OpenConnect checksum mismatch' }
$sevenZip = (Get-Command 7z.exe -ErrorAction SilentlyContinue).Source
if (!$sevenZip) { throw '7z.exe is required to extract the OpenConnect installer.' }
$destination = Join-Path $scratch 'openconnect'
& $sevenZip x $installer "-o$destination" -y | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Could not extract OpenConnect' }
$client = Join-Path $destination 'openconnect.exe'
& $client --version
if ($LASTEXITCODE -ne 0) { throw 'OpenConnect could not run' }
if ($env:GITHUB_ENV) { "TUNNELYARD_TEST_OPENCONNECT=$client" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append }
