param(
    [Parameter(Mandatory = $true)]
    [string]$PackageDirectory,

    [Parameter(Mandatory = $true)]
    [Alias("ExpectedVersion")]
    [string]$ExpectedSourceVersion,

    [string]$ExpectedDistributionVersion = $env:CHAFT_DISTRIBUTION_VERSION
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$packageRoot = (Resolve-Path -LiteralPath $PackageDirectory).Path
$archives = @(Get-ChildItem -LiteralPath $packageRoot -File -Filter "*.zip")
if ($archives.Count -ne 1) {
    throw "Expected exactly one ZIP in $packageRoot, found $($archives.Count)"
}
if ([string]::IsNullOrWhiteSpace($ExpectedDistributionVersion)) {
    $ExpectedDistributionVersion = $ExpectedSourceVersion
}
$expectedArchiveName = "Chaft-$ExpectedDistributionVersion-Windows-x86_64.zip"
if ($archives[0].Name -cne $expectedArchiveName) {
    throw "Expected ZIP filename $expectedArchiveName, got $($archives[0].Name)"
}

Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [System.IO.Compression.ZipFile]::OpenRead($archives[0].FullName)
try {
    $seenPaths = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::OrdinalIgnoreCase
    )
    $forbiddenExtensions = @(
        ".env", ".exp", ".ilk", ".key", ".lib", ".obj", ".p12", ".pdb",
        ".pem", ".pfx", ".snk"
    )

    foreach ($entry in $archive.Entries) {
        $name = $entry.FullName.Replace("\", "/")
        if (
            [string]::IsNullOrWhiteSpace($name) -or
            $name.StartsWith("/") -or
            $name -match "^[A-Za-z]:" -or
            ($name -split "/") -contains ".."
        ) {
            throw "Unsafe ZIP entry: $($entry.FullName)"
        }
        if (-not $seenPaths.Add($name)) {
            throw "Case-insensitive duplicate ZIP entry: $name"
        }
        if ($name -match "(^|/)(\.git|_CPack_Packages|build|target)(/|$)") {
            throw "Build-only directory leaked into ZIP: $name"
        }
        if ($name -match "(^|/)\.env(?:\.|$)") {
            throw "Environment file leaked into ZIP: $name"
        }
        $extension = [System.IO.Path]::GetExtension($name).ToLowerInvariant()
        if ($forbiddenExtensions -contains $extension) {
            throw "Forbidden release file leaked into ZIP: $name"
        }
    }
}
finally {
    $archive.Dispose()
}

$temporaryRoot = $env:RUNNER_TEMP
if ([string]::IsNullOrWhiteSpace($temporaryRoot)) {
    $temporaryRoot = [System.IO.Path]::GetTempPath()
}
$smokeRoot = Join-Path $temporaryRoot "Chaft ZIP smoke Ω"
if (Test-Path -LiteralPath $smokeRoot) {
    Remove-Item -LiteralPath $smokeRoot -Recurse -Force
}
$extractRoot = Join-Path $smokeRoot "portable package"
$runtimeRoot = Join-Path $smokeRoot "runtime"
$workingRoot = Join-Path $smokeRoot "unrelated cwd"
$homeRoot = Join-Path $smokeRoot "home"
New-Item -ItemType Directory -Path $extractRoot, $runtimeRoot, $workingRoot, $homeRoot |
    Out-Null

try {
    [System.IO.Compression.ZipFile]::ExtractToDirectory(
        $archives[0].FullName,
        $extractRoot
    )

    $executables = @(
        Get-ChildItem -LiteralPath $extractRoot -Recurse -File -Filter "ChaftDesktop.exe"
    )
    if ($executables.Count -ne 1) {
        throw "Expected exactly one ChaftDesktop.exe, found $($executables.Count)"
    }
    $executable = $executables[0]
    $binaryDirectory = $executable.Directory.FullName

    $complianceDirectories = @(
        Get-ChildItem -LiteralPath $extractRoot -Recurse -Directory |
            Where-Object {
                $_.FullName.Replace("\", "/").EndsWith(
                    "/share/doc/Chaft",
                    [System.StringComparison]::OrdinalIgnoreCase
                )
            }
    )
    if ($complianceDirectories.Count -ne 1) {
        throw "Expected exactly one share/doc/Chaft directory, found $($complianceDirectories.Count)"
    }
    foreach ($filename in @(
        "LICENSE",
        "THIRD_PARTY_NOTICES.txt",
        "LICENSE.LGPL3",
        "LICENSE.GPL3",
        "QT-CORRESPONDING-SOURCE.json"
    )) {
        $path = Join-Path $complianceDirectories[0].FullName $filename
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Required package notice is missing: $path"
        }
    }
    $qtSourceManifest = Get-Content -LiteralPath (
        Join-Path $complianceDirectories[0].FullName "QT-CORRESPONDING-SOURCE.json"
    ) -Raw | ConvertFrom-Json
    if ($qtSourceManifest.version -ne "6.8.4") {
        throw "Expected Qt source manifest version 6.8.4, got $($qtSourceManifest.version)"
    }
    $windowsModules = @(
        $qtSourceManifest.sourceModules |
            Where-Object { $_.platforms -contains "Windows" } |
            ForEach-Object { $_.name }
    )
    if ($windowsModules -contains "qtwayland") {
        throw "Windows package source manifest must not claim Qt Wayland"
    }

    $requiredSiblingFiles = @(
        "chaft_ffi.dll",
        "qt.conf",
        "Qt6Core.dll",
        "Qt6Gui.dll",
        "Qt6Network.dll",
        "Qt6Qml.dll",
        "Qt6Quick.dll",
        "Qt6Widgets.dll"
    )
    foreach ($filename in $requiredSiblingFiles) {
        $path = Join-Path $binaryDirectory $filename
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Required packaged dependency is missing: $path"
        }
    }

    $windowsPlugins = @(
        Get-ChildItem -LiteralPath $extractRoot -Recurse -File -Filter "qwindows.dll" |
            Where-Object { $_.Directory.Name -eq "platforms" }
    )
    if ($windowsPlugins.Count -ne 1) {
        throw "Expected exactly one packaged platforms/qwindows.dll, found $($windowsPlugins.Count)"
    }
    $offscreenPlugins = @(
        Get-ChildItem -LiteralPath $extractRoot -Recurse -File -Filter "qoffscreen.dll" |
            Where-Object { $_.Directory.Name -eq "platforms" }
    )
    if ($offscreenPlugins.Count -ne 1) {
        throw "Expected exactly one packaged platforms/qoffscreen.dll, found $($offscreenPlugins.Count)"
    }
    $qmlPlugins = @(
        Get-ChildItem -LiteralPath $extractRoot -Recurse -File `
            -Filter "*plugin.dll" -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match "[\\/]qml[\\/]" }
    )
    if ($qmlPlugins.Count -eq 0) {
        throw "No deployed QML plugin DLLs were found"
    }

    $versionInfo = [System.Diagnostics.FileVersionInfo]::GetVersionInfo(
        $executable.FullName
    )
    if ($versionInfo.ProductName -ne "Chaft") {
        throw "Unexpected Windows ProductName: $($versionInfo.ProductName)"
    }
    if ($versionInfo.FileDescription -ne "Chaft Desktop") {
        throw "Unexpected Windows FileDescription: $($versionInfo.FileDescription)"
    }
    if (-not $versionInfo.ProductVersion.StartsWith($ExpectedSourceVersion)) {
        throw "Expected ProductVersion $ExpectedSourceVersion, got $($versionInfo.ProductVersion)"
    }

    foreach ($name in @(
        "CHAFT_FFI_LIBRARY",
        "CMAKE_PREFIX_PATH",
        "QML2_IMPORT_PATH",
        "QML_IMPORT_PATH",
        "QTDIR",
        "QT_PLUGIN_PATH",
        "QT_QPA_PLATFORM_PLUGIN_PATH",
        "QT_ROOT_DIR",
        "Qt6_DIR"
    )) {
        Remove-Item -LiteralPath "Env:$name" -ErrorAction SilentlyContinue
    }

    $env:HOME = $homeRoot
    $env:USERPROFILE = $homeRoot
    $env:PATH = @(
        (Join-Path $env:SystemRoot "System32"),
        $env:SystemRoot,
        (Join-Path $env:SystemRoot "System32/Wbem")
    ) -join ";"
    $env:CHAFT_RUNTIME_DIR = $runtimeRoot
    $env:CHAFT_DESKTOP_SMOKE = "1"
    $env:CHAFT_DESKTOP_SMOKE_EXPECT_NO_WORKSPACE = "1"
    $env:CHAFT_DESKTOP_SMOKE_TIMEOUT_MS = "15000"
    $env:QT_QPA_PLATFORM = "offscreen"

    $process = Start-Process `
        -FilePath $executable.FullName `
        -WorkingDirectory $workingRoot `
        -PassThru
    if (-not $process.WaitForExit(45000)) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        throw "Packaged ChaftDesktop.exe did not finish its smoke test"
    }
    if ($process.ExitCode -ne 0) {
        throw "Packaged ChaftDesktop.exe exited with code $($process.ExitCode)"
    }

    Start-Sleep -Milliseconds 500
    $orphans = @(
        Get-Process -Name "ChaftDesktop" -ErrorAction SilentlyContinue |
            Where-Object { $_.Path -eq $executable.FullName }
    )
    if ($orphans.Count -ne 0) {
        $orphans | Stop-Process -Force -ErrorAction SilentlyContinue
        throw "Packaged ChaftDesktop.exe left an orphan process"
    }
}
finally {
    if (Test-Path -LiteralPath $smokeRoot) {
        Remove-Item -LiteralPath $smokeRoot -Recurse -Force
    }
}

Write-Host "Clean Windows ZIP smoke passed: $($archives[0].Name)"
