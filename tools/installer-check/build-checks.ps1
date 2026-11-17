function New-PublicationFixture {
  param([string]$Root, [string]$Name, [string]$Contents)
  $source = Join-Path $Root "$Name.exe"
  [IO.File]::WriteAllText($source, $Contents)
  return @{
    Source = $source
    Record = [ordered]@{
      version = '0.1.0'; installer = Join-Path $Root 'output/package.exe'
      bytes = (Get-Item -LiteralPath $source).Length
      sha256 = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant()
    }
  }
}

function Test-InstallerPublication {
  param([string]$Root)
  $testRoot = Join-Path $Root 'publication-checks'
  New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
  $old = New-PublicationFixture $testRoot 'first' 'first verified package'
  $output = Join-Path $testRoot 'output'
  Publish-InstallerPackage $old.Source $old.Record $output
  $current = New-PublicationFixture $testRoot 'second' 'second verified package'
  Publish-InstallerPackage $current.Source $current.Record $output
  $recordPath = Join-Path $output 'verification.json'
  $saved = Get-Content -LiteralPath $recordPath -Raw
  if ([IO.File]::ReadAllText($current.Record.installer) -ne 'second verified package') { throw 'Package replacement failed.' }
  $blocked = New-PublicationFixture $testRoot 'blocked' 'must not remain installed'
  $recordLock = [IO.File]::Open($recordPath, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
  $rejected = $false
  try { Publish-InstallerPackage $blocked.Source $blocked.Record $output }
  catch { $rejected = $_ }
  finally { $recordLock.Dispose() }
  if (-not $rejected -or [IO.File]::ReadAllText($current.Record.installer) -ne 'second verified package' `
    -or (Get-Content -LiteralPath $recordPath -Raw) -ne $saved) { throw "Failed record commit did not preserve the old package and metadata: $rejected" }
  $parallel = New-PublicationFixture $testRoot 'parallel' 'must not enter publication'
  $publishLock = [IO.File]::Open((Join-Path $output '.installer-publish.lock'), [IO.FileMode]::Open, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
  $rejected = $false
  try { Publish-InstallerPackage $parallel.Source $parallel.Record $output }
  catch { $rejected = $true }
  finally { $publishLock.Dispose() }
  if (-not $rejected -or -not [IO.File]::Exists($parallel.Source)) { throw 'Concurrent publication was not rejected before moving files.' }
  $changed = New-PublicationFixture $testRoot 'changed' 'original staged contents'
  [IO.File]::WriteAllText($changed.Source, 'changed after validation')
  $rejected = $false
  try { Publish-InstallerPackage $changed.Source $changed.Record $output }
  catch { $rejected = $true }
  if (-not $rejected) { throw 'Changed staged contents were accepted.' }
  $firstFailure = New-PublicationFixture $testRoot 'first-failure' 'no previous package exists'
  $emptyOutput = Join-Path $testRoot 'empty-output'
  $firstFailure.Record.installer = Join-Path $emptyOutput 'package.exe'
  New-Item -ItemType Directory -Path (Join-Path $emptyOutput 'verification.json') -Force | Out-Null
  $rejected = $false
  try { Publish-InstallerPackage $firstFailure.Source $firstFailure.Record $emptyOutput }
  catch { $rejected = $true }
  if (-not $rejected -or [IO.File]::Exists($firstFailure.Record.installer) -or -not [IO.File]::Exists($firstFailure.Source)) {
    throw 'First publication failure did not restore its staged file.'
  }
  if (Get-ChildItem -LiteralPath $output -Filter '*.previous' -Force) { throw 'Successful rollback left a previous package.' }
}

function Test-InstallerBuild {
  param([string]$Root, [string]$Engine, [string]$Controller)
  Test-InstallerPublication $Root
  $record = Build-InstallerShell $Engine (Join-Path $Root 'shell-build') (Join-Path $Root 'shell-output')
  $expected = (Get-FileHash -LiteralPath $Controller -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($record.applicationSha256 -ne $expected -or $record.applicationSha256 -eq $record.engineSha256) {
    throw 'Installer metadata did not use the application embedded inside the engine.'
  }
  if ((Get-FileHash -LiteralPath $record.installer -Algorithm SHA256).Hash.ToLowerInvariant() -ne $record.sha256) {
    throw 'Published package does not match its verification record.'
  }
  [ordered]@{ passed = $true; engine = 'isolated NSIS'; snapshotAndMetadata = $true
    atomicReplace = $true; rollback = $true; parallelRejected = $true; changedStageRejected = $true } |
    ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Root 'build-result.json') -Encoding utf8
}
