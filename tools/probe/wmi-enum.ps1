# Phase 0.1 — OEM WMI 枚举脚本
# 目的：列出 root\WMI 及相关命名空间中 OEM（Clevo / Uniwill / TongFang / Insyde / Oem / Colorful）
# 相关的类，确认 OEM CC 的 WMI 通道面。只读操作。
# 用法：powershell -ExecutionPolicy Bypass -File tools\probe\wmi-enum.ps1
# 输出：控制台报告；加 -Save 可保存到 tools\probe\wmi-enum-results.txt

param(
    [switch]$Save
)

$ErrorActionPreference = "SilentlyContinue"
$out = New-Object System.Collections.Generic.List[string]
function Log { param($s) $out.Add($s); Write-Host $s }

Log "=== WMI OEM Enumeration ($env:COMPUTERNAME) ==="
Log "=== 机器身份 ==="
$cs = Get-CimInstance Win32_ComputerSystem
$bios = Get-CimInstance Win32_BIOS
$bb = Get-CimInstance Win32_BaseBoard
Log "System: $($cs.Manufacturer) $($cs.Model) (family: $($cs.SystemFamily))"
Log "BIOS:   $($bios.Manufacturer) $($bios.SMBIOSBIOSVersion) ($($bios.ReleaseDate))"
Log "Board:  $($bb.Manufacturer) $($bb.Product)"

Log ""
Log "=== root\WMI 命名空间列表 ==="
Get-CimInstance -Namespace root\WMI -ClassName __Namespace |
    Sort-Object Name |
    ForEach-Object { Log "  $($_.Name)" }

Log ""
Log "=== root\WMI 中匹配 OEM 关键字的类 ==="
$pattern = 'Clevo|Uniwill|TongFang|Tongfang|Insyde|Oem|OEM|Mci|Colorful|NP5x|NP6x|NP7x'
$classes = Get-CimClass -Namespace root\WMI | Where-Object { $_.CimClassName -match $pattern }
if ($classes) {
    foreach ($c in $classes) {
        Log ""
        Log "── $($c.CimClassName)  (super: $($c.CimSuperClassName))"
        foreach ($p in ($c.CimClassProperties | Where-Object { $_.Name -notmatch '^_?' })) {
            Log "    prop  $($p.Name) : $($p.CimType)"
        }
        foreach ($m in $c.CimClassMethods) {
            Log "    method $($m.Name)"
            foreach ($q in $m.Parameters) { Log "        param $($q.Name) : $($q.CimType)" }
        }
    }
} else {
    Log "  （无匹配类）"
}

Log ""
Log "=== 其他 root 下的 OEM 命名空间探测 ==="
foreach ($ns in @('root\WMI', 'root\CIMV2', 'root\DEFAULT')) {
    $found = Get-CimClass -Namespace $ns | Where-Object { $_.CimClassName -cmatch 'TongFang|Uniwill|ITe[ _]|Insyde' } | Select-Object -First 10
    if ($found) {
        Log "[$ns]"
        $found | ForEach-Object { Log "  $($_.CimClassName)" }
    }
}

Log ""
Log "=== 相关内核服务（AcpiBridge / DCHU / EC）==="
Get-Service | Where-Object { $_.Name -match 'DCHU|AcpiBridge|CCDC|Colorful' } |
    ForEach-Object { Log "  service $($_.Name) [$($_.Status)] $($_.DisplayName)" }
Get-CimInstance Win32_SystemDriver | Where-Object { $_.Name -match 'AcpiBridge|WinIo|WinRing|PawnIO|ITe|ECNet' } |
    ForEach-Object { Log "  driver  $($_.Name) [$($_.State)] $($_.PathName)" }

$result = $out -join "`r`n"
if ($Save) {
    $path = Join-Path $PSScriptRoot "wmi-enum-results.txt"
    $result | Out-File -FilePath $path -Encoding utf8
    Write-Host "`nsaved -> $path"
}
