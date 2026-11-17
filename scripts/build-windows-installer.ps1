[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'Windows and PowerShell 7 are required.' }

function Resolve-InstallerMSBuild {
  $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
  if (Test-Path -LiteralPath $vswhere) {
    $found = & $vswhere -latest -products * -requires Microsoft.Component.MSBuild -find 'MSBuild/**/Bin/MSBuild.exe'
    if ($LASTEXITCODE -eq 0 -and $found) { return ($found | Select-Object -First 1) }
  }
  $framework = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/MSBuild.exe'
  if (Test-Path -LiteralPath $framework) { return $framework }
  throw 'MSBuild with the .NET Framework 4.8.1 WPF targeting pack is required.'
}

function Assert-InstallerPackage {
  param([string]$Output, [string]$EnginePath, [string]$ApplicationPath, [string]$BuildDirectory)
  $csc = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
  $source = (Resolve-Path -LiteralPath tools/installer-check/VerifyPackage.cs).Path
  $verifier = Join-Path $BuildDirectory 'verify-package.exe'
  & $csc /nologo /target:exe /platform:x64 "/out:$verifier" $source
  if ($LASTEXITCODE -ne 0) { throw 'Package verifier compilation failed.' }
  & $verifier $Output $EnginePath $ApplicationPath $BuildDirectory
  if ($LASTEXITCODE -ne 0) { throw 'Embedded package verification failed.' }
}

function Read-InstallerPayload {
  param([string]$EnginePath, [string]$BuildDirectory, [string]$ApplicationFileStem)
  $archiver = Get-Command 7z -ErrorAction Stop
  $payload = Join-Path $BuildDirectory ('payload-' + [Guid]::NewGuid().ToString('N'))
  New-Item -ItemType Directory -Path $payload -Force | Out-Null
  $snapshot = Join-Path $payload 'engine.exe'
  $sourceStream = [IO.File]::Open($EnginePath, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
  try {
    $snapshotStream = [IO.File]::Create($snapshot)
    try { $sourceStream.CopyTo($snapshotStream) } finally { $snapshotStream.Dispose() }
  }
  finally { $sourceStream.Dispose() }
  & $archiver.Source x -y -bso0 -bsp0 "-o$payload" $snapshot "$ApplicationFileStem.exe"
  if ($LASTEXITCODE -ne 0) { throw 'Could not read the application embedded in the NSIS engine.' }
  $application = Join-Path $payload "$ApplicationFileStem.exe"
  if (-not (Test-Path -LiteralPath $application -PathType Leaf)) { throw 'The NSIS engine contains no application.' }
  return [pscustomobject]@{ Engine = $snapshot; Application = $application }
}

function Move-InstallerFile {
  param([string]$Source, [string]$Destination, [string]$Backup, [bool]$Replace)
  for ($attempt = 0; ; $attempt++) {
    try {
      if ($Replace) { [IO.File]::Replace($Source, $Destination, $Backup) }
      else { [IO.File]::Move($Source, $Destination) }
      return
    }
    catch {
      $cause = $_.Exception
      while ($cause.InnerException) { $cause = $cause.InnerException }
      if ($attempt -ge 7 -or ($cause.HResult -band 65535) -notin @(32, 33)) { throw }
      Start-Sleep -Milliseconds 25
    }
  }
}

function Publish-InstallerPackage {
  param([string]$StagedFile, [Collections.IDictionary]$Record, [string]$OutputDirectory)
  if ([IO.Path]::GetDirectoryName($Record.installer) -ne [IO.Path]::GetFullPath($OutputDirectory)) {
    throw 'The package destination must be inside its output directory.'
  }
  if ((Get-FileHash -LiteralPath $StagedFile -Algorithm SHA256).Hash -ne $Record.sha256 `
    -or (Get-Item -LiteralPath $StagedFile).Length -ne $Record.bytes) { throw 'The staged package changed after verification.' }
  New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
  $lockPath = Join-Path $OutputDirectory '.installer-publish.lock'
  $lockStream = [IO.File]::Open($lockPath, [IO.FileMode]::OpenOrCreate, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
  $suffix = [Guid]::NewGuid().ToString('N')
  $final = $Record.installer
  $recordPath = Join-Path $OutputDirectory 'verification.json'
  $pending = Join-Path $OutputDirectory ".verification-$suffix.json"
  $backup = Join-Path $OutputDirectory ".installer-$suffix.previous"
  $recordBackup = Join-Path $OutputDirectory ".verification-$suffix.previous"
  $hadPackage = [IO.File]::Exists($final)
  $committed = $false
  try {
    [IO.File]::WriteAllText($pending, ($Record | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
    try {
      Move-InstallerFile $StagedFile $final $backup $hadPackage
      $committed = $true
      Move-InstallerFile $pending $recordPath $recordBackup ([IO.File]::Exists($recordPath))
    }
    catch {
      $publishError = $_
      try {
        if ($committed -and $hadPackage) { Move-InstallerFile $backup $final $StagedFile $true }
        elseif ($committed) { Move-InstallerFile $final $StagedFile $backup $false }
      }
      catch { throw "Package publication failed: $publishError. Restore failed: $_. Previous package: $backup" }
      throw $publishError
    }
    foreach ($temporary in @($backup, $recordBackup)) {
      try { [IO.File]::Delete($temporary) }
      catch { Write-Warning "Package committed; could not remove $temporary : $_" }
    }
  }
  finally {
    try { [IO.File]::Delete($pending) }
    catch { Write-Warning "Could not remove pending verification $pending : $_" }
    finally { $lockStream.Dispose() }
  }
}

function Build-InstallerShell {
  param([string]$EnginePath, [string]$BuildDirectory, [string]$OutputDirectory)
  $config = Get-Content -LiteralPath 'src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json
  $package = Get-Content -LiteralPath 'package.json' -Raw | ConvertFrom-Json
  $msbuild = Resolve-InstallerMSBuild
  $attempt = Join-Path ([IO.Path]::GetFullPath($BuildDirectory)) ('build-' + [Guid]::NewGuid().ToString('N'))
  New-Item -ItemType Directory -Force -Path $attempt | Out-Null
  $stem = "$($config.productName)_$($config.version)_x64-setup"
  $payload = Read-InstallerPayload $EnginePath $attempt $package.name
  $engine = $payload.Engine
  $application = $payload.Application
  if ([Diagnostics.FileVersionInfo]::GetVersionInfo($engine).ProductVersion -ne $config.version) {
    throw 'NSIS engine version does not match the configured version.'
  }
  & node scripts/prepare-installer.mjs $engine $application $attempt | Out-Host
  if ($LASTEXITCODE -ne 0) { throw 'Installer metadata generation failed.' }
  $outputPath = (Join-Path $attempt 'out') + [IO.Path]::DirectorySeparatorChar
  $intermediatePath = (Join-Path $attempt 'obj') + [IO.Path]::DirectorySeparatorChar
  & $msbuild installer/ColorfulControlCenter.Installer.csproj /nologo /verbosity:minimal /t:Rebuild `
    "/p:OutputPath=$outputPath" "/p:IntermediateOutputPath=$intermediatePath" "/p:TargetName=$stem" `
    "/p:InstallerEnginePath=$engine" "/p:InstallerBuildDirectory=$attempt" | Out-Host
  if ($LASTEXITCODE -ne 0) { throw "Installer compilation failed: $LASTEXITCODE" }
  $output = Join-Path $outputPath "$stem.exe"
  $version = [Diagnostics.FileVersionInfo]::GetVersionInfo($output)
  if ($version.ProductVersion -ne $config.version -or $version.ProductName -ne $config.productName) {
    throw 'Installer product metadata verification failed.'
  }
  Assert-InstallerPackage $output $engine $application $attempt | Out-Host
  $record = [ordered]@{
    version = $config.version; installer = [IO.Path]::GetFullPath((Join-Path $OutputDirectory "$stem.exe"))
    bytes = (Get-Item -LiteralPath $output).Length
    sha256 = (Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash.ToLowerInvariant()
    engineSha256 = (Get-FileHash -LiteralPath $engine -Algorithm SHA256).Hash.ToLowerInvariant()
    applicationSha256 = (Get-FileHash -LiteralPath $application -Algorithm SHA256).Hash.ToLowerInvariant()
    realInstallationTested = $false
  }
  Publish-InstallerPackage $output $record ([IO.Path]::GetFullPath($OutputDirectory))
  return [pscustomobject]$record
}

function Build-WindowsInstaller {
  Resolve-InstallerMSBuild | Out-Null
  Get-Command 7z -ErrorAction Stop | Out-Null
  $targetingPack = Join-Path ${env:ProgramFiles(x86)} 'Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8.1/PresentationFramework.dll'
  if (-not (Test-Path -LiteralPath $targetingPack)) { throw '.NET Framework 4.8.1 targeting pack is required.' }
  $target = if ($env:CARGO_TARGET_DIR) { [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR) } else { Join-Path $PWD 'target' }
  & node node_modules/@tauri-apps/cli/tauri.js build --bundles nsis | Out-Host
  if ($LASTEXITCODE -ne 0) { throw "Tauri NSIS build failed: $LASTEXITCODE" }
  $config = Get-Content -LiteralPath 'src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json
  $engine = Join-Path $target "release/bundle/nsis/$($config.productName)_$($config.version)_x64-setup.exe"
  $record = Build-InstallerShell $engine (Join-Path $target 'release/ccc-installer') (Join-Path $target 'release/bundle/installer')
  Write-Output "Windows installer: $($record.installer)"
  Write-Output "SHA256: $($record.sha256)"
}

if ($MyInvocation.InvocationName -ne '.') {
  Push-Location (Split-Path $PSScriptRoot -Parent)
  try { Build-WindowsInstaller } finally { Pop-Location }
}
