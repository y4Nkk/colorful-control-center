# 显式运行的探测批；默认只读，不安装/重启服务，不自动提权。
# -ExecuteVoltageTest 才会测试一个电压编码步进并恢复；此操作会写硬件。
param(
    [switch]$ExecuteVoltageTest,
    [ValidateSet("cpu", "gpu", "cache", "uncore", "analogio")][string]$Plane = "cpu",
    [string]$OutputPath = (Join-Path $PSScriptRoot ("probe-" + (Get-Date -Format "yyyyMMdd-HHmmss") + ".txt"))
)
$ErrorActionPreference = "Stop"
$probeRoot = Split-Path -Parent $PSScriptRoot
$probe = Join-Path $probeRoot "probe-cli\target\release\ccc-probe.exe"
if (-not (Test-Path -LiteralPath $probe)) {
    throw "未找到 release 版 ccc-probe.exe；先在项目根目录执行 cargo build --release --manifest-path tools/probe-cli/Cargo.toml"
}
if (Test-Path -LiteralPath $OutputPath) { throw "输出文件已存在，请指定新的 -OutputPath" }
$probeFailed = $false
function Invoke-Probe {
    param([string[]]$ProbeArgs)
    Write-Output ("=== ccc-probe " + ($ProbeArgs -join " ") + " ===")
    & $probe @ProbeArgs
    if ($LASTEXITCODE -ne 0) { $script:probeFailed = $true }
}
Start-Transcript -LiteralPath $OutputPath -NoClobber
try {
    Invoke-Probe -ProbeArgs @("status")
    Invoke-Probe -ProbeArgs @("pl")
    Invoke-Probe -ProbeArgs @("dchu")
    Invoke-Probe -ProbeArgs @("dchu-pkg")
    if ($ExecuteVoltageTest) {
        Invoke-Probe -ProbeArgs @("uv-test", "--execute", "--plane", $Plane)
    } else {
        Write-Output "未执行电压 Mailbox 查询或写入。实际 PL/降压可写性仍未验证。"
    }
} finally { Stop-Transcript }
if ($probeFailed) { exit 1 }
