[CmdletBinding()]
param(
  [string]$MpvDir = ".\vendor\windows\mpv",
  [string]$InstallerOutputDir = "."
)

$ErrorActionPreference = "Stop"

function Assert-Command {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Name
  )

  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "Required command '$Name' was not found in PATH."
  }
}

$repoRoot = Split-Path -Parent $PSScriptRoot
$stageDir = Join-Path $repoRoot "src-tauri\resources\windows\mpv-runtime"
$resolvedInstallerOutputDir = if ([System.IO.Path]::IsPathRooted($InstallerOutputDir)) {
  [System.IO.Path]::GetFullPath($InstallerOutputDir)
}
else {
  [System.IO.Path]::GetFullPath((Join-Path $repoRoot $InstallerOutputDir))
}

Push-Location $repoRoot
try {
  Assert-Command -Name npm
  Assert-Command -Name cargo
  Assert-Command -Name rustc

  if (-not (Test-Path $MpvDir)) {
    throw "libmpv folder '$MpvDir' was not found. Put mpv.lib and libmpv-2.dll under vendor/windows/mpv/ or pass -MpvDir C:\path\to\mpv."
  }

  $resolvedMpvDir = (Resolve-Path $MpvDir).Path
  $mpvLibPath = Join-Path $resolvedMpvDir "mpv.lib"
  $libmpvDllPath = Join-Path $resolvedMpvDir "libmpv-2.dll"
  $mingwImportLibPath = Join-Path $resolvedMpvDir "libmpv.dll.a"

  if (-not (Test-Path $mpvLibPath)) {
    if (Test-Path $mingwImportLibPath) {
      throw "Expected mpv.lib in '$resolvedMpvDir'. Found libmpv.dll.a instead, which is a MinGW import library. This repo builds with the default Rust MSVC toolchain on Windows. Generate mpv.lib from libmpv-2.dll in a Visual Studio Developer Command Prompt with: lib /name:libmpv-2.dll /out:mpv.lib /MACHINE:X64"
    }

    throw "Expected mpv.lib in '$resolvedMpvDir'. On Windows MSVC, generate it from libmpv-2.dll in a Visual Studio Developer Command Prompt with: lib /name:libmpv-2.dll /out:mpv.lib /MACHINE:X64"
  }

  if (-not (Test-Path $libmpvDllPath)) {
    throw "Expected libmpv-2.dll in '$resolvedMpvDir'."
  }

  $runtimeDlls = Get-ChildItem -Path $resolvedMpvDir -Filter *.dll -File | Sort-Object Name
  if ($runtimeDlls.Count -eq 0) {
    throw "No DLL files were found in '$resolvedMpvDir'."
  }

  if (Test-Path $stageDir) {
    Remove-Item $stageDir -Recurse -Force
  }
  New-Item -ItemType Directory -Path $stageDir -Force | Out-Null

  foreach ($dll in $runtimeDlls) {
    Copy-Item $dll.FullName -Destination (Join-Path $stageDir $dll.Name) -Force
  }

  Write-Host "Bundling libmpv runtime files:" -ForegroundColor Cyan
  foreach ($dll in $runtimeDlls) {
    Write-Host "  - $($dll.Name)"
  }

  if (-not (Test-Path (Join-Path $repoRoot "node_modules"))) {
    Write-Host "Installing JavaScript dependencies with npm ci..." -ForegroundColor Cyan
    npm ci
    if ($LASTEXITCODE -ne 0) {
      throw "npm ci failed with exit code $LASTEXITCODE."
    }
  }

  $env:MPV_LIB_DIR = $resolvedMpvDir

  Write-Host "Building the offline Windows installer..." -ForegroundColor Cyan
  npm run tauri:build
  if ($LASTEXITCODE -ne 0) {
    throw "npm run tauri:build failed with exit code $LASTEXITCODE."
  }

  $bundleDir = Join-Path $repoRoot "src-tauri\target\release\bundle\nsis"
  if (-not (Test-Path $bundleDir)) {
    throw "Build finished but '$bundleDir' was not found."
  }

  $installerFiles = Get-ChildItem -Path $bundleDir -Filter *.exe -File | Sort-Object LastWriteTime -Descending
  if ($installerFiles.Count -eq 0) {
    throw "Build finished but no installer .exe files were found in '$bundleDir'."
  }

  New-Item -ItemType Directory -Path $resolvedInstallerOutputDir -Force | Out-Null
  foreach ($installer in $installerFiles) {
    Copy-Item $installer.FullName -Destination (Join-Path $resolvedInstallerOutputDir $installer.Name) -Force
  }

  Write-Host "Installer output:" -ForegroundColor Green
  $installerFiles | ForEach-Object {
    Write-Host "  $($_.FullName)"
  }

  Write-Host "Installer copied to:" -ForegroundColor Green
  $installerFiles | ForEach-Object {
    $copiedInstallerPath = Join-Path $resolvedInstallerOutputDir $_.Name
    Write-Host "  $copiedInstallerPath"
  }

  Write-Host "Tauri bundle output:" -ForegroundColor DarkGreen
  Get-ChildItem -Path $bundleDir -File | Sort-Object LastWriteTime -Descending | ForEach-Object {
    Write-Host "  $($_.FullName)"
  }
}
finally {
  Pop-Location
}