<#
.SYNOPSIS
Builds the Windows release: the command-line tool and the desktop app's installer, for x64 and
Arm64 (or one of them, with -Arch), with Microsoft's toolchain and rustup's Rust, embedding the
fonts from `nix build .#fonts`.

.DESCRIPTION
Nix doesn't run on Windows: build the fonts on Linux or macOS (`nix build .#fonts`) and pass
their folder as -Fonts or in GEORGIAN_FOR_GARMIN_FONTS. The core's build script checks every
font against the folder's manifest; the builder's tests pin the manifest's hashes.

Nothing is signed: Windows SmartScreen warns before the first run. The installer is NSIS,
installs for the current user without administrator rights, and fetches Microsoft's WebView2
runtime only if Windows lacks it. Binaries link the C runtime statically (.cargo/config.toml),
so no Visual C++ Redistributable is needed.

Needs: Visual Studio 2022 or later, or its Build Tools, with a Windows SDK and the "MSVC C++
x64/x86 build tools" (for -Arch x64) and "MSVC C++ ARM64 build tools" (for -Arch arm64); rustup;
internet access (Rust, the Tauri CLI, NSIS).
Runs in Windows PowerShell 5.1 and PowerShell 7. Writes to target\windows-release\; the files to
publish end up in target\windows-release\dist\. If RELEASE_TAG is set (CI on a tag), it must be
v<workspace version>.
#>
[CmdletBinding()]
param(
    [string]$Fonts = $env:GEORGIAN_FOR_GARMIN_FONTS,
    [ValidateSet('x64', 'arm64')]
    [string[]]$Arch = @('x64', 'arm64')
)

Set-StrictMode -Version 3.0
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

# Must equal nixpkgs' cargo-tauri, which the macOS build uses; release.yml compares them.
$TauriCliVersion = '2.11.4'
$TauriCliSha256 = @{
    'x86_64-pc-windows-msvc'  = '0743e30a661a35d63339b24cf63828f97ba5389a1d7f13b368a542794dd0a3f3'
    'aarch64-pc-windows-msvc' = '926b95544a67e0e098d895d7223a7e4eb47c758c3652f030d0dc2a070f7c082a'
}
# Target triple, the name used in file names, the PE machine dumpbin reports, and the Visual
# Studio component with its compiler and linker.
$Targets = @(
    @{ Triple = 'x86_64-pc-windows-msvc'; Arch = 'x64'; Machine = '8664 machine (x64)'
        Component = 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64' },
    @{ Triple = 'aarch64-pc-windows-msvc'; Arch = 'arm64'; Machine = 'AA64 machine (ARM64)'
        Component = 'Microsoft.VisualStudio.Component.VC.Tools.ARM64' }
) | Where-Object { $Arch -contains $_.Arch }
$Targets = @($Targets)

function Step([string]$Message) { Write-Host "`n==> $Message" }

# Runs a program and stops the build if it fails.
function Invoke-Checked([string]$Exe, [string[]]$ArgList) {
    & $Exe @ArgList
    if ($LASTEXITCODE -ne 0) { throw "$Exe $($ArgList -join ' ') failed with exit code $LASTEXITCODE" }
}

function Get-Sha256([string]$Path) { (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant() }

# Writes UTF-8 without a byte order mark and with LF line ends, as sha256sum expects.
function Write-Lines([string]$Path, [string[]]$Lines) {
    [IO.File]::WriteAllText($Path, (($Lines -join "`n") + "`n"), (New-Object Text.UTF8Encoding $false))
}

$root = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $root
$work = Join-Path $root 'target\windows-release'
$dist = Join-Path $work 'dist'

if (-not $Fonts) { throw 'pass the fonts from `nix build .#fonts` as -Fonts or in GEORGIAN_FOR_GARMIN_FONTS' }
$Fonts = (Resolve-Path -LiteralPath $Fonts).Path
$manifestPath = Join-Path $Fonts 'manifest.json'
if (-not (Test-Path -LiteralPath $manifestPath)) { throw "$Fonts has no manifest.json; is it the output of nix build .#fonts?" }
$manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
$packKinds = @{}
foreach ($pack in $manifest.packs) { $packKinds[$pack.id] = $pack.kind }
$rustup = (Get-Command rustup -ErrorAction SilentlyContinue)
if (-not $rustup) { throw 'rustup is needed (https://rustup.rs)' }
$hostTriple = 'x86_64-pc-windows-msvc'
if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { $hostTriple = 'aarch64-pc-windows-msvc' }

Step 'Finding Visual Studio'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path -LiteralPath $vswhere)) { throw 'Visual Studio 2022 or later, or its Build Tools, is needed' }
$components = @($Targets | ForEach-Object { $_.Component })
$vs = & $vswhere -latest -products * -property installationPath -requires @components
if (-not $vs) { throw "Visual Studio needs these components: $($components -join ', ')" }
# Any dumpbin reads every architecture's binaries; prefer the one native to this machine.
$hostDir = if ($hostTriple -like 'aarch64*') { 'HostARM64' } else { 'Hostx64' }
$dumpbin = Get-ChildItem -Path (Join-Path $vs 'VC\Tools\MSVC\*\bin\Host*\*\dumpbin.exe') |
    Sort-Object { $_.FullName -notlike "*\$hostDir\*" }, FullName | Select-Object -First 1
if (-not $dumpbin) { throw "no dumpbin.exe under $vs" }
Write-Host "$vs; $($dumpbin.FullName)"
Write-Host "$($env:NUMBER_OF_PROCESSORS) processors, $($env:PROCESSOR_ARCHITECTURE)"

$toolchain = (Select-String -LiteralPath rust-toolchain.toml -Pattern '^channel = "(.*)"$').Matches[0].Groups[1].Value
$triples = $Targets | ForEach-Object { $_.Triple }
Step "Installing Rust $toolchain for $($triples -join ', ')"
Invoke-Checked $rustup.Source @('toolchain', 'install', $toolchain, '--profile', 'minimal', '--no-self-update',
    '--target', ($triples -join ','))
$cargo = (& $rustup.Source which --toolchain $toolchain cargo)
if ($LASTEXITCODE -ne 0) { throw "rustup has no cargo for $toolchain" }
$rustBin = Split-Path -Parent $cargo

Step "Fetching the Tauri CLI $TauriCliVersion"
$tools = Join-Path $work 'tools'
$cargoTauri = Join-Path $tools "tauri-cli-$TauriCliVersion\cargo-tauri.exe"
if (-not (Test-Path -LiteralPath $cargoTauri)) {
    New-Item -ItemType Directory -Force -Path $tools | Out-Null
    $zip = Join-Path $tools "cargo-tauri-$TauriCliVersion-$hostTriple.zip"
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    Invoke-WebRequest -UseBasicParsing -OutFile $zip `
        -Uri "https://github.com/tauri-apps/tauri/releases/download/tauri-cli-v$TauriCliVersion/cargo-tauri-$hostTriple.zip"
    $actual = Get-Sha256 $zip
    if ($actual -ne $TauriCliSha256[$hostTriple]) { throw "$zip has SHA-256 $actual, not $($TauriCliSha256[$hostTriple])" }
    Expand-Archive -LiteralPath $zip -DestinationPath (Split-Path -Parent $cargoTauri) -Force
    if (-not (Test-Path -LiteralPath $cargoTauri)) { throw "the Tauri CLI archive has no cargo-tauri.exe" }
}

# rustup's toolchain first, and nothing from the caller's compiler settings.
$env:PATH = "$rustBin;$env:PATH"
foreach ($name in 'RUSTC', 'RUSTC_WRAPPER', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_RUSTFLAGS',
    'CARGO_BUILD_TARGET', 'CC', 'CXX', 'CFLAGS', 'CXXFLAGS') {
    Remove-Item -LiteralPath "Env:$name" -ErrorAction SilentlyContinue
}
$env:RUSTUP_TOOLCHAIN = $toolchain
$env:CARGO_TARGET_DIR = Join-Path $work 'cargo'
$env:GEORGIAN_FOR_GARMIN_FONTS = $Fonts

$pkgid = (& $cargo pkgid --locked -p georgian-for-garmin)
if ($LASTEXITCODE -ne 0) { throw 'cargo pkgid failed' }
$version = $pkgid -replace '^.*[#@]', ''
if ($env:RELEASE_TAG -and $env:RELEASE_TAG -ne "v$version") {
    throw "tag $($env:RELEASE_TAG) does not match the workspace version $version"
}
Write-Host "Georgian for Garmin $version, $(& (Join-Path $rustBin 'rustc.exe') --version), $(& $cargoTauri --version)"

Step 'Building the command-line tool'
$targetArgs = @()
foreach ($t in $Targets) { $targetArgs += @('--target', $t.Triple) }
Invoke-Checked $cargo (@('build', '--locked', '--release', '-p', 'georgian-for-garmin') + $targetArgs)

Step 'Building the app'
Push-Location crates\app
try {
    foreach ($t in $Targets) {
        Invoke-Checked $cargoTauri @('build', '--ci', '--target', $t.Triple, '--bundles', 'nsis', '--', '--locked')
    }
} finally {
    Pop-Location
}

foreach ($t in $Targets) {
    $release = Join-Path $env:CARGO_TARGET_DIR "$($t.Triple)\release"
    $t.Cli = Join-Path $release 'georgian-for-garmin.exe'
    $t.App = Join-Path $release 'georgian-for-garmin-app.exe'
    $installers = @(Get-ChildItem -Path (Join-Path $release 'bundle\nsis\*-setup.exe'))
    if ($installers.Count -ne 1) { throw "expected one installer in $release\bundle\nsis, found $($installers.Count)" }
    $t.Installer = $installers[0].FullName
    foreach ($file in $t.Cli, $t.App) {
        if (-not (Test-Path -LiteralPath $file)) { throw "the build made no $file" }
    }
}

Step 'Checking the binaries'
# DLLs that come with the Visual C++ Redistributable or WebView2's SDK, not with Windows.
$notOnWindows = '^(vcruntime|msvcp|vccorlib|concrt|webview2loader)'
$latin1 = [Text.Encoding]::GetEncoding(28591)
$sha256 = [Security.Cryptography.SHA256]::Create()
foreach ($t in $Targets) {
    foreach ($bin in $t.Cli, $t.App) {
        $label = "$(Split-Path -Leaf $bin) $($t.Arch)"
        $headers = & $dumpbin.FullName /nologo /headers $bin
        if ($LASTEXITCODE -ne 0) { throw "dumpbin /headers $bin failed" }
        $machine = @($headers | Select-String -Pattern '^\s+\S+ machine \(' | ForEach-Object { $_.Line.Trim() })
        if ($machine.Count -ne 1 -or $machine[0] -ne $t.Machine) { throw "$label is '$machine', not '$($t.Machine)'" }
        $dlls = & $dumpbin.FullName /nologo /dependents $bin | Where-Object { $_ -match '^\s+\S+\.dll\s*$' } |
            ForEach-Object { $_.Trim() }
        if ($LASTEXITCODE -ne 0) { throw "dumpbin /dependents $bin failed" }
        $foreign = @($dlls | Where-Object { $_ -match $notOnWindows })
        if ($foreign.Count -gt 0) { throw "$label needs DLLs Windows doesn't have: $($foreign -join ', ')" }
        Write-Host "${label}: $machine; imports $($dlls -join ', ')"

        # Every font, byte for byte. Latin-1 maps each byte to one character, so an ordinal string
        # search is a byte search.
        $data = [IO.File]::ReadAllBytes($bin)
        $haystack = $latin1.GetString($data)
        foreach ($font in $manifest.fonts) {
            # Bitmap parts by pixel size; TrueType parts scale and have no size_px, which strict
            # mode refuses to read, so the pack's kind decides.
            $size = if ($packKinds[$font.pack] -eq 'truetype') { 'TrueType' } else { "$($font.size_px) px" }
            foreach ($kind in 'stock', 'georgian') {
                $blob = [IO.File]::ReadAllBytes((Join-Path $Fonts $font.$kind))
                $offset = $haystack.IndexOf($latin1.GetString($blob), [StringComparison]::Ordinal)
                if ($offset -lt 0) { throw "$label does not embed $($font.$kind)" }
                if ($kind -eq 'georgian') {
                    $embedded = -join ($sha256.ComputeHash($data, $offset, $blob.Length) | ForEach-Object { $_.ToString('x2') })
                    Write-Host "${label}: embeds $($font.$kind) ($size) sha256 $embedded"
                }
            }
        }
    }
    if ($hostTriple -eq $t.Triple -or $hostTriple -like 'aarch64*') {
        $reported = & $t.Cli --version
        if ($LASTEXITCODE -ne 0 -or $reported -notmatch [regex]::Escape($version)) {
            throw "$($t.Cli) --version printed '$reported'"
        }
        Write-Host "$($t.Arch) CLI runs: $reported"
    }
}

Step 'Packaging'
Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
if (Test-Path -LiteralPath $dist) { Remove-Item -Recurse -Force -LiteralPath $dist }
New-Item -ItemType Directory -Path $dist | Out-Null
$published = @()
# No version in the file names, so the README's releases/latest/download/<name> links keep working.
foreach ($t in $Targets) {
    $cliName = "georgian-for-garmin-windows-$($t.Arch)"
    $zipPath = Join-Path $dist "$cliName.zip"
    # Entries are named with forward slashes, so every unzip tool sees the folder.
    $archive = [IO.Compression.ZipFile]::Open($zipPath, [IO.Compression.ZipArchiveMode]::Create)
    try {
        $entries = [ordered]@{
            'georgian-for-garmin.exe' = $t.Cli
            'LICENSE'                 = (Join-Path $root 'LICENSE')
            'OFL.txt'                 = (Join-Path $Fonts 'LICENSES\OFL.txt')
        }
        foreach ($name in $entries.Keys) {
            [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, $entries[$name], "$cliName/$name",
                [IO.Compression.CompressionLevel]::Optimal) | Out-Null
        }
    } finally {
        $archive.Dispose()
    }
    $setupName = "Georgian-for-Garmin-windows-$($t.Arch)-setup.exe"
    Copy-Item -LiteralPath $t.Installer -Destination (Join-Path $dist $setupName)
    $published += "$cliName.zip", $setupName
}
Copy-Item -LiteralPath $manifestPath -Destination (Join-Path $dist 'manifest.json')
$published += 'manifest.json'
Write-Lines (Join-Path $dist 'SHA256SUMS') ($published | ForEach-Object { "$(Get-Sha256 (Join-Path $dist $_))  $_" })

Step "Done: $dist"
Get-ChildItem -LiteralPath $dist | Format-Table -AutoSize Name, Length | Out-String | Write-Host
Get-Content -LiteralPath (Join-Path $dist 'SHA256SUMS') | Write-Host
