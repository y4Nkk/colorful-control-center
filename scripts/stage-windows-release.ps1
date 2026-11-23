[CmdletBinding()]
param(
  [string]$Tag,
  [switch]$CheckVersionOnly,
  [switch]$SignUpdate
)

$ErrorActionPreference = 'Stop'
$config = Get-Content -LiteralPath 'src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json
$version = $config.version
if ($version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$') {
  throw 'The Windows release version must use X.Y.Z.'
}
if ($Tag -and $Tag -cne "v$version") {
  throw "Tag $Tag does not match application version v$version."
}
$package = Get-Content -LiteralPath 'package.json' -Raw | ConvertFrom-Json
if ($package.version -cne $version) {
  throw 'The npm and Tauri versions must match.'
}
$changelog = Get-Content -LiteralPath 'CHANGELOG.md' -Raw -Encoding utf8
$section = [regex]::Match($changelog, '(?ms)^## \[' + [regex]::Escape($version) + '\][^\r\n]*\r?\n(?<notes>.*?)(?=^## |\z)')
if (-not $section.Success -or -not $section.Groups['notes'].Value.Trim()) {
  throw "CHANGELOG.md has no release notes for $version."
}
if ($CheckVersionOnly) { return }
if ($SignUpdate -and (-not $env:GITHUB_SERVER_URL -or -not $env:GITHUB_REPOSITORY -or
    -not $env:TAURI_UPDATER_PUBLIC_KEY -or -not $env:TAURI_SIGNING_PRIVATE_KEY)) {
  throw 'GitHub repository context and the updater signing key pair are required.'
}

$buildRoot = if ($env:CARGO_TARGET_DIR) { [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR) } else { Join-Path $PWD 'target' }
$installerDirectory = Join-Path $buildRoot 'release/bundle/installer'
$fileName = "$($config.productName)_${version}_x64-setup.exe"
$installer = Get-Item -LiteralPath (Join-Path $installerDirectory $fileName)
$report = Get-Content -LiteralPath (Join-Path $installerDirectory 'verification.json') -Raw | ConvertFrom-Json
if ($report.version -cne $version -or
    $installer.VersionInfo.ProductVersion -cne $version -or
    $report.installer -ine $installer.FullName -or
    $report.bytes -ne $installer.Length) {
  throw 'Installer version, path or size does not match verification.json.'
}

$staging = Join-Path $PWD 'target/github-release'
New-Item -Path $staging -ItemType Directory -Force | Out-Null
$stagedInstaller = Join-Path $staging $fileName
Copy-Item -LiteralPath $installer.FullName -Destination $stagedInstaller -Force
$digest = (Get-FileHash -LiteralPath $stagedInstaller -Algorithm SHA256).Hash.ToLowerInvariant()
if ($digest -ine $report.sha256) {
  throw 'Staged installer SHA-256 does not match verification.json.'
}

$notes = $section.Groups['notes'].Value.Trim()
$releaseNotes = $notes
if ($env:GITHUB_SERVER_URL -and $env:GITHUB_REPOSITORY -and $env:GITHUB_SHA) {
  $source = "$env:GITHUB_SERVER_URL/$env:GITHUB_REPOSITORY/blob/$env:GITHUB_SHA"
  $notes = $notes -replace '\]\((?![/#]|[a-z][a-z0-9+.-]*:)([^)\s]+)\)', ("]($source/" + '$1')
  $template = Get-Content -LiteralPath '.github/release_template.md' -Raw -Encoding utf8
  $downloadTable = $template -replace 'REPOSITORY', $env:GITHUB_REPOSITORY -replace 'VERSION', $version -replace 'PRODUCTNAME', $config.productName
  $releaseNotes = "$downloadTable`n`n$notes"
}
$integrity = 'Windows x64 安装包的 SHA-256 为 `{0}`。' -f $digest
[IO.File]::WriteAllText((Join-Path $staging 'SHA256SUMS.txt'), "$digest  $fileName`n")
[IO.File]::WriteAllText((Join-Path $staging 'release-notes.md'), "$releaseNotes`n`n$integrity`n")

if ($SignUpdate) {
  $signaturePath = "$stagedInstaller.sig"
  # Tauri refuses to replace a signature left by a previous run.
  [IO.File]::Delete($signaturePath)
  & node node_modules/@tauri-apps/cli/tauri.js signer sign --app-version $version $stagedInstaller
  if ($LASTEXITCODE -ne 0) { throw 'Windows updater signing failed.' }
  & cargo run --locked --quiet -p colorful-control-center --example verify_update_package -- $stagedInstaller $signaturePath $version
  if ($LASTEXITCODE -ne 0) { throw 'Windows updater signature verification failed.' }
  $manifest = [ordered]@{
    version = $version
    notes = $notes
    platforms = @{
      'windows-x86_64' = [ordered]@{
        url = "$env:GITHUB_SERVER_URL/$env:GITHUB_REPOSITORY/releases/download/v$version/$fileName"
        signature = [IO.File]::ReadAllText($signaturePath).Trim()
      }
    }
  }
  [IO.File]::WriteAllText((Join-Path $staging 'latest.json'), ($manifest | ConvertTo-Json -Depth 4))
}
Write-Output "Staged $fileName with SHA-256 $digest"
