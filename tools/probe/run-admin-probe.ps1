# 一次性提权验证批（Phase 0.4 收尾）
# 作用：以管理员运行 probe-cli 的全部探测，输出存档到 tools\probe\admin-probe-results.txt
# 用法：powershell -ExecutionPolicy Bypass -File tools\probe\run-admin-probe.ps1
# （脚本会自行请求提升——UAC 点"是"即可；非管理员调用时经 Start-Process 转交）

$ErrorActionPreference = "Continue"
$root = Split-Path -Parent $PSScriptRoot
$probe = Join-Path $root "probe-cli\target\release\ccc-probe.exe"
if (-not (Test-Path $probe)) { $probe = Join-Path $root "probe-cli\target\debug\ccc-probe.exe" }

$id = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $id.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Host "需要管理员权限，正在请求提升（请确认 UAC）..."
    Start-Process powershell -Verb RunAs -Wait -ArgumentList @(
        "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $PSCommandPath
    )
    exit
}

# 提权后：transcript 兜底记录（子控制台关闭也不丢输出）
$resultsPath = Join-Path $PSScriptRoot "admin-probe-results.txt"
Start-Transcript -Path $resultsPath -Force

Write-Output "=== 管理员探测批 $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') ==="
Write-Output ""
Write-Output "probe: $probe"
Write-Output "存在: $(Test-Path $probe)"
Write-Output ""

Write-Output "== 1. PawnIO 驱动部署 =="
& $probe install 2>&1 | Write-Output
Write-Output ""

Write-Output "== 2. PL / 锁位检测 =="
& $probe pl 2>&1 | Write-Output
Write-Output ""

Write-Output "== 3. undervolt 锁检测（写入后还原）=="
& $probe uv-test --execute 2>&1 | Write-Output
Write-Output ""

Write-Output "== 4. DCHU 读数（管理员下，间隔 3 秒两次对照）=="
& $probe dchu 2>&1 | Write-Output
Start-Sleep -Seconds 3
& $probe dchu 2>&1 | Write-Output
Write-Output ""

Write-Output "== 结束 =="
Stop-Transcript
