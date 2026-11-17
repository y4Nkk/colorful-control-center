[CmdletBinding()]
param(
  [string]$NsisDirectory = (Join-Path $env:LOCALAPPDATA 'tauri/NSIS'),
  [switch]$RenderOnly
)

$ErrorActionPreference = 'Stop'
Push-Location (Split-Path (Split-Path $PSScriptRoot -Parent) -Parent)
try {
  . ./scripts/build-windows-installer.ps1
  . ./tools/installer-check/build-checks.ps1
  $msbuild = Resolve-InstallerMSBuild
  $root = Join-Path $PWD ("target/installer-check/" + [Guid]::NewGuid().ToString('N'))
  New-Item -ItemType Directory -Path $root -Force | Out-Null
  $makensis = Join-Path $NsisDirectory 'makensis.exe'
  if (-not (Test-Path -LiteralPath $makensis)) { throw 'NSIS cache is missing. Build the installer first, or pass -NsisDirectory.' }
  $controller = Join-Path $root 'controller-fixture.exe'
  $fixture = Join-Path $root 'engine-fixture.exe'
  $csc = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
  $fixtureSource = (Resolve-Path -LiteralPath tools/installer-check/ControllerFixture.cs).Path
  & $csc /nologo /target:exe /platform:x64 "/out:$controller" $fixtureSource
  if ($LASTEXITCODE -ne 0) { throw 'Fixture compiler failed.' }
  $config = Get-Content -LiteralPath src-tauri/tauri.conf.json -Raw | ConvertFrom-Json
  $package = Get-Content -LiteralPath package.json -Raw | ConvertFrom-Json
  $hooks = (Resolve-Path -LiteralPath src-tauri/windows/nsis-hooks.nsh).Path
  $engineSource = (Resolve-Path -LiteralPath tools/installer-check/engine-fixture.nsi).Path
  & $makensis /V2 "/DHOOKS=$hooks" "/DAPPLICATION=$controller" "/DAPPLICATION_NAME=$($package.name).exe" "/DOUTPUT=$fixture" "/DVERSION=$($config.version)" $engineSource
  if ($LASTEXITCODE -ne 0) { throw 'NSIS engine fixture compilation failed.' }
  Test-InstallerBuild $root $fixture $controller
  $payload = Read-InstallerPayload $fixture $root $package.name
  & node scripts/prepare-installer.mjs $payload.Engine $payload.Application $root
  if ($LASTEXITCODE -ne 0) { throw 'Fixture metadata failed.' }
  $source = (Resolve-Path -LiteralPath tools/installer-check/InstallerChecks.cs).Path
  $output = $root + [IO.Path]::DirectorySeparatorChar
  $intermediate = (Join-Path $root 'obj') + [IO.Path]::DirectorySeparatorChar
  & $msbuild installer/ColorfulControlCenter.Installer.csproj /nologo /verbosity:minimal /t:Rebuild `
    "/p:OutputPath=$output" "/p:IntermediateOutputPath=$intermediate" '/p:TargetName=installer-checks' `
    "/p:InstallerEnginePath=$($payload.Engine)" "/p:InstallerBuildDirectory=$root" "/p:InstallerTestSource=$source" `
    '/p:StartupObject=InstallerChecks'
  if ($LASTEXITCODE -ne 0) { throw 'Installer check compilation failed.' }
  if (-not $RenderOnly) {
    $guardSource = (Resolve-Path -LiteralPath tools/installer-check/running-guard.nsi).Path
    & $makensis /V2 "/DHOOKS=$hooks" "/DOUTPUT=$root\running-guard.exe" "/DCONTROLLER=$root\colorful-control-center.exe" $guardSource
    if ($LASTEXITCODE -ne 0) { throw 'NSIS running guard compilation failed.' }
  }
  $arguments = '"' + $root + '"' + $(if ($RenderOnly) { ' render' } else { '' })
  $process = Start-Process -FilePath (Join-Path $root 'installer-checks.exe') -ArgumentList $arguments `
    -PassThru -WindowStyle Hidden
  $elapsed = [Diagnostics.Stopwatch]::StartNew()
  while (-not $process.WaitForExit(1000)) {
    if ($elapsed.Elapsed.TotalSeconds -gt 600) { throw "Installer checks timed out. Artifacts: $root" }
  }
  if ($process.ExitCode -ne 0) {
    $failure = Join-Path $root 'failure.txt'
    if (Test-Path -LiteralPath $failure) { Get-Content -LiteralPath $failure }
    throw "Installer checks failed. Artifacts: $root"
  }
  Get-Content -LiteralPath (Join-Path $root 'result.json')
  Write-Output "Installer check artifacts: $root"
}
finally { Pop-Location }
