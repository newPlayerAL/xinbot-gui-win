param(
    [string]$XinbotJar = (Join-Path $PSScriptRoot "../../xinbot/target/xinbot-2.4.3-RELEASE-windows-x86_64.jar"),
    [string]$DirectConnectJar = (Join-Path $PSScriptRoot "../bundled-plugins/directconnect/target/directconnect.jar"),
    [string]$ChatFilterJar = (Join-Path $PSScriptRoot "../../ChatFilter/target/ChatFilter-1.0.0-RELEASE.jar")
)

$ErrorActionPreference = "Stop"
$resourceDirectory = Join-Path $PSScriptRoot "../src-tauri/resources"
$destinationJar = Join-Path $resourceDirectory "xinbot.jar"

if (-not (Test-Path -LiteralPath $XinbotJar -PathType Leaf)) {
    throw "XinBot Core was not found at $XinbotJar"
}
if (-not (Test-Path -LiteralPath $DirectConnectJar -PathType Leaf)) {
    throw "DirectConnect plugin was not found at $DirectConnectJar"
}

New-Item -ItemType Directory -Force -Path $resourceDirectory | Out-Null
Copy-Item -LiteralPath $XinbotJar -Destination $destinationJar -Force
Copy-Item -LiteralPath $DirectConnectJar -Destination (Join-Path $resourceDirectory "directconnect.jar") -Force

function Get-VerifiedFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Url,
        [Parameter(Mandatory = $true)][string]$Sha256,
        [Parameter(Mandatory = $true)][string]$Name
    )

    $ready = (Test-Path -LiteralPath $Path -PathType Leaf) -and
        ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() -eq $Sha256)
    if ($ready) {
        return
    }

    $downloadPath = "$Path.download"
    Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $downloadPath
    $actualHash = (Get-FileHash -LiteralPath $downloadPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualHash -ne $Sha256) {
        Remove-Item -LiteralPath $downloadPath -Force -ErrorAction SilentlyContinue
        throw "$Name SHA-256 mismatch: $actualHash"
    }
    Move-Item -LiteralPath $downloadPath -Destination $Path -Force
}

if (-not (Test-Path -LiteralPath $ChatFilterJar -PathType Leaf)) {
    throw "ChatFilter plugin was not found at $ChatFilterJar"
}
Copy-Item -LiteralPath $ChatFilterJar -Destination (Join-Path $resourceDirectory "chatfilter.jar") -Force

$xinMetaPath = Join-Path $resourceDirectory "xinmetaplugin.jar"
$xinMetaHash = "ad4f1e40e985b8ac53eb91b93cbc0facb6cca7e16200edd29923f7a4a32e8e99"
$xinMetaUrl = "https://github.com/huangdihd/XinMetaPlugin/releases/download/1.1.0-RELEASE/xinMetaPlugin-1.1.0-RELEASE.jar"
$xinMetaReady = (Test-Path -LiteralPath $xinMetaPath -PathType Leaf) -and
    ((Get-FileHash -LiteralPath $xinMetaPath -Algorithm SHA256).Hash.ToLowerInvariant() -eq $xinMetaHash)
if (-not $xinMetaReady) {
    Invoke-WebRequest -UseBasicParsing -Uri $xinMetaUrl -OutFile $xinMetaPath
    $actualXinMetaHash = (Get-FileHash -LiteralPath $xinMetaPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualXinMetaHash -ne $xinMetaHash) {
        throw "XinMetaPlugin SHA-256 mismatch: $actualXinMetaHash"
    }
}

$backToTheBasePath = Join-Path $resourceDirectory "backtothebase.jar"
Get-VerifiedFile `
    -Path $backToTheBasePath `
    -Url "https://github.com/huangdihd/BackToTheBase/releases/download/1.8.0-RELEASE/BackToTheBase-1.8.0-RELEASE.jar" `
    -Sha256 "d40874d3af3889620b7da17b5311215fb0e3e5413c0ee47df1a3cd43e2660568" `
    -Name "BackToTheBase"

$movementOriginalPath = Join-Path $resourceDirectory "movementsync-original.jar"
Get-VerifiedFile `
    -Path $movementOriginalPath `
    -Url "https://github.com/huangdihd/MovementSync/releases/download/1.6.0-RELEASE/original-MovementSync-1.6.0-RELEASE.jar" `
    -Sha256 "61bfc79916443e3e13683f5602b0316a689c0e8a3217fa2c8e628de6c3a70acc" `
    -Name "MovementSync"

$jomlPath = Join-Path $resourceDirectory "joml-1.10.5.jar"
Get-VerifiedFile `
    -Path $jomlPath `
    -Url "https://repo.maven.apache.org/maven2/org/joml/joml/1.10.5/joml-1.10.5.jar" `
    -Sha256 "cac9f22f83a7aa33eebda73c16ff5261e3cb4911b6bafcf4c79ea486099d0c9a" `
    -Name "JOML"

Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

# The upstream shaded MovementSync release repeats an entire older XinBot Core.
# Assemble the official unshaded plugin with its only non-Core runtime dependency
# instead, keeping the installer small and avoiding duplicate Core classes.
$movementPath = Join-Path $resourceDirectory "movementsync.jar"
Copy-Item -LiteralPath $movementOriginalPath -Destination $movementPath -Force
$movementArchive = [System.IO.Compression.ZipFile]::Open(
    $movementPath,
    [System.IO.Compression.ZipArchiveMode]::Update
)
$jomlArchive = [System.IO.Compression.ZipFile]::OpenRead($jomlPath)
try {
    $existingEntries = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::Ordinal
    )
    foreach ($entry in $movementArchive.Entries) {
        [void]$existingEntries.Add($entry.FullName)
    }
    foreach ($entry in $jomlArchive.Entries) {
        if (-not $entry.FullName.StartsWith("org/joml/", [System.StringComparison]::Ordinal) -or
            [string]::IsNullOrEmpty($entry.Name) -or
            -not $existingEntries.Add($entry.FullName)) {
            continue
        }
        $targetEntry = $movementArchive.CreateEntry(
            $entry.FullName,
            [System.IO.Compression.CompressionLevel]::Optimal
        )
        $targetEntry.LastWriteTime = $entry.LastWriteTime
        $sourceStream = $entry.Open()
        $targetStream = $targetEntry.Open()
        try {
            $sourceStream.CopyTo($targetStream)
        }
        finally {
            $targetStream.Dispose()
            $sourceStream.Dispose()
        }
    }
}
finally {
    $jomlArchive.Dispose()
    $movementArchive.Dispose()
}

$movementCheck = [System.IO.Compression.ZipFile]::OpenRead($movementPath)
try {
    $movementEntryNames = @($movementCheck.Entries | ForEach-Object FullName)
    if ($movementEntryNames -notcontains "plugin.yml" -or
        $movementEntryNames -notcontains "org/joml/Vector3d.class") {
        throw "The assembled MovementSync JAR is incomplete"
    }
}
finally {
    $movementCheck.Dispose()
}

$archive = [System.IO.Compression.ZipFile]::OpenRead($destinationJar)
try {
    $entryNamesToRemove = @($archive.Entries | Where-Object {
        $name = $_.FullName
        $isFile = -not [string]::IsNullOrEmpty($_.Name)
        $isNettyNative = $isFile -and $name.StartsWith("META-INF/native/", [System.StringComparison]::Ordinal)
        $isWindowsNetty = $name -eq "META-INF/native/netty_quiche42_windows_x86_64.dll"
        $isJansiNative = $isFile -and $name.StartsWith(
            "org/fusesource/jansi/internal/native/",
            [System.StringComparison]::Ordinal
        )
        $isWindowsJansi = $name.StartsWith(
            "org/fusesource/jansi/internal/native/Windows/x86_64/",
            [System.StringComparison]::Ordinal
        )
        $isJlineNative = $isFile -and $name.StartsWith(
            "org/jline/nativ/",
            [System.StringComparison]::Ordinal
        ) -and ($name.EndsWith(".dll", [System.StringComparison]::OrdinalIgnoreCase) -or
            $name.EndsWith(".so", [System.StringComparison]::OrdinalIgnoreCase) -or
            $name.EndsWith(".jnilib", [System.StringComparison]::OrdinalIgnoreCase))
        $isWindowsJline = $name.StartsWith(
            "org/jline/nativ/Windows/x86_64/",
            [System.StringComparison]::Ordinal
        )

        ($isNettyNative -and -not $isWindowsNetty) -or
            ($isJansiNative -and -not $isWindowsJansi) -or
            ($isJlineNative -and -not $isWindowsJline)
    } | ForEach-Object FullName)
}
finally {
    $archive.Dispose()
}

# Avoid opening an already-slim Core in update mode. ZipArchive rewrites its
# central directory even when no entry changes, which needlessly changes the
# verified JAR checksum.
if ($entryNamesToRemove.Count -gt 0) {
    $archive = [System.IO.Compression.ZipFile]::Open(
        $destinationJar,
        [System.IO.Compression.ZipArchiveMode]::Update
    )
    try {
        foreach ($name in $entryNamesToRemove) {
            $entry = $archive.GetEntry($name)
            if ($null -ne $entry) {
                $entry.Delete()
            }
        }
    }
    finally {
        $archive.Dispose()
    }
}

$hash = Get-FileHash -LiteralPath $destinationJar -Algorithm SHA256
$size = (Get-Item -LiteralPath $destinationJar).Length
Write-Output "Prepared Windows-only xinbot.jar"
Write-Output "Size: $size bytes"
Write-Output "SHA-256: $($hash.Hash.ToLowerInvariant())"
Write-Output "Bundled BackToTheBase: $((Get-Item -LiteralPath $backToTheBasePath).Length) bytes"
Write-Output "Bundled slim MovementSync: $((Get-Item -LiteralPath $movementPath).Length) bytes"
