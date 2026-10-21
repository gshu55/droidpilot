param(
    [Parameter(Mandatory)][string]$Tag,
    [string]$BundleDirectory = 'src-tauri/target/release/bundle/nsis',
    [string]$PortableExecutable = 'src-tauri/target/release/droidpilot.exe',
    [string]$NotesFile,
    [string]$OutputDirectory = 'release-assets'
)
$ErrorActionPreference = 'Stop'
if ($Tag -notmatch '^v\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$') { throw 'Invalid version tag' }
if (!$NotesFile) { $NotesFile = "docs/releases/$Tag.md" }
$installers = @(Get-ChildItem -Path $BundleDirectory -Filter '*-setup.exe' -File)
if ($installers.Count -ne 1) { throw 'Expected exactly one NSIS installer' }
$installer = $installers[0]
$signaturePath = "$($installer.FullName).sig"
if (!(Test-Path -LiteralPath $signaturePath)) { throw 'Missing updater signature; configure TAURI_SIGNING_PRIVATE_KEY' }
$signature = (Get-Content -LiteralPath $signaturePath -Raw).Trim()
if (!$signature) { throw 'Empty updater signature' }
# A malformed signature should fail before any release is published.
[void][Convert]::FromBase64String($signature)
$notes = Get-Content -LiteralPath $NotesFile -Raw -Encoding utf8
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Release output directory already exists; use a fresh directory' }
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
Copy-Item -LiteralPath $installer.FullName -Destination $OutputDirectory
Copy-Item -LiteralPath $signaturePath -Destination $OutputDirectory
Copy-Item -LiteralPath $PortableExecutable -Destination "$OutputDirectory/DroidPilot_${Tag}_windows_x64_portable.exe"
$manifest = [ordered]@{
    version = $Tag.Substring(1)
    notes = $notes
    pub_date = [DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ')
    platforms = @{
        'windows-x86_64' = @{
            signature = $signature
            url = "https://github.com/gshu55/droidpilot/releases/download/$Tag/$([Uri]::EscapeDataString($installer.Name))"
        }
    }
}
$manifest | ConvertTo-Json -Depth 5 | Set-Content "$OutputDirectory/latest.json" -Encoding utf8NoBOM
Get-ChildItem -LiteralPath $OutputDirectory -File | Sort-Object Name | Get-FileHash -Algorithm SHA256 | ForEach-Object {
    "$($_.Hash.ToLower())  $(Split-Path $_.Path -Leaf)"
} | Set-Content "$OutputDirectory/SHA256SUMS.txt" -Encoding utf8NoBOM
