param(
    [string]$SourceDirectory = (Join-Path $PSScriptRoot "../../xinbot-gui/dist/xinbot-gui"),
    [string]$ChatFilterJar = (Join-Path $PSScriptRoot "../../ChatFilter/target/ChatFilter-1.0.0-RELEASE.jar")
)

$ErrorActionPreference = "Stop"
$resourceDirectory = Join-Path $PSScriptRoot "../src-tauri/resources"
$sourceJar = Join-Path $SourceDirectory "xinbot.jar"
$destinationJar = Join-Path $resourceDirectory "xinbot.jar"

if (-not (Test-Path -LiteralPath $sourceJar -PathType Leaf)) {
    throw "xinbot.jar was not found at $sourceJar"
}

New-Item -ItemType Directory -Force -Path $resourceDirectory | Out-Null
Copy-Item -LiteralPath $sourceJar -Destination $destinationJar -Force
Copy-Item -LiteralPath (Join-Path $SourceDirectory "directconnect.jar") -Destination $resourceDirectory -Force

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

Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [System.IO.Compression.ZipFile]::Open(
    $destinationJar,
    [System.IO.Compression.ZipArchiveMode]::Update
)

try {
    $entriesToRemove = @($archive.Entries | Where-Object {
        $name = $_.FullName
        $isNettyNative = $name.StartsWith("META-INF/native/", [System.StringComparison]::Ordinal)
        $isWindowsNetty = $name -eq "META-INF/native/netty_quiche42_windows_x86_64.dll"
        $isJansiNative = $name.StartsWith(
            "org/fusesource/jansi/internal/native/",
            [System.StringComparison]::Ordinal
        )
        $isWindowsJansi = $name.StartsWith(
            "org/fusesource/jansi/internal/native/Windows/x86_64/",
            [System.StringComparison]::Ordinal
        )

        ($isNettyNative -and -not $isWindowsNetty) -or
            ($isJansiNative -and -not $isWindowsJansi)
    })

    foreach ($entry in $entriesToRemove) {
        $entry.Delete()
    }
}
finally {
    $archive.Dispose()
}

$hash = Get-FileHash -LiteralPath $destinationJar -Algorithm SHA256
$size = (Get-Item -LiteralPath $destinationJar).Length
Write-Output "Prepared Windows-only xinbot.jar"
Write-Output "Size: $size bytes"
Write-Output "SHA-256: $($hash.Hash.ToLowerInvariant())"
