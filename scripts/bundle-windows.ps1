# Packages a compiled DJ Sugar for Windows as dist/djsugar-VERSION-windows-ARCH.zip
# (portable: unzip anywhere and run DJSugar.exe) and
# dist/DJSugar-VERSION-windows-ARCH-setup.exe (per-user install, no admin).
# Usage: scripts/bundle-windows.ps1 BINARY VERSION [ARCH], ARCH x64 (the
# default) or arm64.
# Needs Inno Setup 6 (installed with Chocolatey when missing). When
# SIGN_COMMAND is set, it runs with the file to sign appended, for
# DJSugar.exe and then for the setup.
param(
  [Parameter(Mandatory)] [string] $Binary,
  [Parameter(Mandatory)] [string] $Version,
  [ValidateSet("x64", "arm64")] [string] $Arch = "x64"
)
$ErrorActionPreference = "Stop"

function Sign($file) {
  if ($env:SIGN_COMMAND) {
    Invoke-Expression "$env:SIGN_COMMAND `"$file`""
    if ($LASTEXITCODE) { throw "signing $file failed" }
  }
}

New-Item -ItemType Directory -Force dist | Out-Null
$stage = "dist/DJSugar"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory $stage | Out-Null
Copy-Item $Binary "$stage/DJSugar.exe"
# The helper that runs audio plugins, built next to the app's binary.
$helper = Join-Path (Split-Path $Binary) "djsugar-plugin-host.exe"
if (Test-Path $helper) {
  Copy-Item $helper "$stage/djsugar-plugin-host.exe"
} else {
  Write-Warning "no $helper, so this build can't run audio plugins"
}
Copy-Item -Recurse res/controllers "$stage/controllers"
# The guided tour's demo tracks, once they are in the repository.
if (Test-Path res/demo) { Copy-Item -Recurse res/demo "$stage/demo" }
Copy-Item README.md, NOTICE "$stage/"
Copy-Item LICENSE "$stage/LICENSE-GPL2.txt"
# The ASIO SDK built in is GPLv3, so the Windows build as a whole is too.
Copy-Item res/windows/LICENSE-GPL3.txt "$stage/"
Sign "$stage/DJSugar.exe"
if (Test-Path "$stage/djsugar-plugin-host.exe") { Sign "$stage/djsugar-plugin-host.exe" }

$zip = "dist/djsugar-$Version-windows-$Arch.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path $stage -DestinationPath $zip

$iscc = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
if (-not (Test-Path $iscc)) {
  choco install innosetup -y --no-progress | Out-Null
}
$source = (Resolve-Path $stage).Path
$output = (Resolve-Path dist).Path
& $iscc /Q "/DVersion=$Version" "/DArch=$Arch" "/DSource=$source" "/DOutput=$output" res/windows/djsugar.iss
if ($LASTEXITCODE) { throw "Inno Setup failed" }
Sign "dist/DJSugar-$Version-windows-$Arch-setup.exe"
Remove-Item -Recurse -Force $stage
