# Phase 0.2 — NBFC 配置检索（可重复执行；2026-11-12 首跑结论见输出）
# 在 nbfc-linux 配置库中按本机型号/模具关键词检索现成风扇配置；
# 唯一命中时下载到 tools\probe\nbfc-config-download\。
# 用法：powershell -ExecutionPolicy Bypass -File tools\probe\check-nbfc.ps1
# 说明：只读网络请求（GitHub API），无管理员要求。

$ErrorActionPreference = "Stop"
$api = "https://api.github.com/repos/nbfc-linux/nbfc-linux/contents/share/nbfc/configs"

# 本机身份（WIN32 事实）
$model = (Get-CimInstance Win32_ComputerSystem).Model.Trim()
$product = (Get-CimInstance Win32_BaseBoard).Product.Trim()
Write-Host "本机 Model=$model Board=$product"

# 检索关键词：本机型号全名与分段 + 模具家族词
$keywords = @($model, $product) + $model.Split("_") |
    Where-Object { $_ -and $_.Length -ge 3 } |
    Select-Object -Unique
$familyKeywords = @("Colorful", "X15", "Tongfang", "TongFang", "Uniwill", "NP5x", "NP6x", "NP7x")

Write-Host "拉取配置清单..."
$configs = Invoke-RestMethod -Uri $api -Headers @{ "User-Agent" = "ccc-probe" }
$names = $configs | ForEach-Object { $_.name }

$hits = $names | Where-Object {
    $name = $_
    ($keywords + $familyKeywords) | Where-Object { $name -match [regex]::Escape($_) }
} | Select-Object -Unique

Write-Host ""
Write-Host "=== 命中配置（关键词匹配）==="
if ($hits) {
    $hits | ForEach-Object { Write-Host "  $_" }
} else {
    Write-Host "  （无命中）"
}

# 唯一精确命中时自动下载（模型全名或主板型号匹配视为精确）
try {
    Write-Host "[1] 构建身份列表"
    $ids = @()
    foreach ($v in @($model, $product)) { if ($v) { $ids += $v } }
    Write-Host "[2] 精确匹配：$($ids -join ', ')"
    $exact = @()
    foreach ($id in $ids) {
        foreach ($n in $names) {
            if ($n -like "*$id*") { $exact += $n }
        }
    }
    $exact = @($exact | Sort-Object -Unique)
    Write-Host "[3] 精确命中数：$($exact.Count)"
    if ($exact.Count -eq 1) {
        $fileName = $exact[0]
        Write-Host "唯一精确命中：$fileName，下载中..."
        $entry = $configs | Where-Object { $_.name -eq $fileName }
        $destDir = Join-Path $PSScriptRoot "nbfc-config-download"
        New-Item -ItemType Directory -Force -Path $destDir | Out-Null
        $dest = Join-Path $destDir $fileName
        Invoke-RestMethod -Uri $entry.download_url -Headers @{ "User-Agent" = "ccc-probe" } -OutFile $dest
        Write-Host "已保存 -> $dest"
        Write-Host "注意：来源为 nbfc-linux（GPL-3），寄存器映射请对照 docs/oem-dchu-contract.md §8 使用。"
    } elseif ($exact.Count -gt 1) {
        Write-Host "多个精确命中，请人工选择：$($exact -join ', ')"
    } else {
        Write-Host "无精确命中（本机走 InsydeDCHU 主通道；EC 兜底寄存器见 tools/probe/ec-map.json）。"
    }
    Write-Host "[4] 完成"
} catch {
    Write-Host "ERROR: $_"
    exit 1
}
