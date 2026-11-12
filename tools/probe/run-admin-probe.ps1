# 一次性提权验证批（Phase 0.4 收尾）
# 作用：以管理员运行 probe-cli 的全部探测，输出存档到 tools/probe/admin-probe-results.txt
# 用法：右键"以管理员身份运行"本脚本，或 powershell -ExecutionPolicy Bypass -File tools\probe\run-admin-probe.ps1
# 会弹一次 UAC；写入探测仅 ccc-probe uv-test --execute（+1 步进即还原，安全边界内）

$ErrorActionPreference = "Continue"
$root = Split-Path -Parent $PSScriptRoot   # tools/
$probe = Join-Path $root "probe-cli\target\release\ccc-probe.exe"
if (-not (Test-Path $probe)) { $probe = Join-Path $root "probe-cli\target\debug\ccc-probe.exe" }

$id = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $id.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Host "需要管理员权限，正在请求提升（请确认 UAC）..."
    Start-Process powershell -Verb RunAs -ArgumentList @(
        "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $PSCommandPath
    ) -Wait
    exit
}

$out = @()
function Log { param($s) $out += $s; Write-Host $s }

Log "=== 管理员探测批 $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') ==="
Log ""
Log "== 1. PawnIO 驱动部署 =="
& $probe install 2>&1 | ForEach-Object { Log $_ }
Log ""
Log "== 2. PL / 锁位检测 =="
& $probe pl 2>&1 | ForEach-Object { Log $_ }
Log ""
Log "== 3. undervolt 锁检测（写入后还原）=="
& $probe uv-test --execute 2>&1 | ForEach-Object { Log $_ }
Log ""
Log "== 4. DCHU 读数（管理员下，间隔 3 秒两次对照）=="
& $probe dchu 2>&1 | ForEach-Object { Log $_ }
Start-Sleep -Seconds 3
& $probe dchu 2>&1 | ForEach-Object { Log $_ }
Log ""
Log "== 结束 =="

$path = Join-Path $PSScriptRoot "admin-probe-results.txt"
$out -join "`r`n" | Out-File -FilePath $path -Encoding utf8
Write-Host ""
Write-Host "已保存 -> $path"
