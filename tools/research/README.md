# 原厂静态 IL 核对

`audit-oem-il.py`只调用明确指定的`ilspycmd`提取原程序集，不加载原厂入口、驱动或硬件接口。每次运行重新提取三个程序集，不接受旧IL作为输入。

在仓库根目录运行：

```powershell
$piArtifacts = 'C:/Users/28308/.pi-desktop/scratch/9b0d7323-5294-4777-8be6-bbeabdcfb6a8'
python tools/research/audit-oem-il.py `
  --ilspy C:/Users/28308/.dotnet/tools/ilspycmd.exe `
  --fnkey-assembly "$piArtifacts/fnkeypkg/FnKey/FnKey.exe" `
  --cpuoc-assembly F:/projects/cc-research/appx/cpuoc/x64/CPUOC_Loading/CPUOC2.0.exe `
  --kbled-assembly "$piArtifacts/kbapp/CC/LedKeyboardSetting.exe" `
  --output target/ui-check/oem-optional-evidence.json
```

输出目录必须存在。三个完整IL与JSON报告写入输出目录；报告包含程序集/IL的SHA256、方法数量、字段引用及调用指令的位置。输入缺失、提取失败、模块残缺、方法歧义或当前边界变化返回非零，并将报告写成`ok: false`，避免沿用上次绿色结论。

核对范围是TP能力/保存调用/颜色选择器、XMP公开与解码字段，以及Deep S5状态生产者和消费者。实际功能解释分别由`docs/keyboard-contract.md`和`docs/oem-device-contract.md`拥有。没有指令引用只描述输入版本，不证明其他组件或硬件不支持；存储字段及枚举不等于实际控制入口。

隔离测试不需要原程序集或反编译工具：

```powershell
python -m unittest discover -s tools/research -p test_audit_oem_il.py -v
```

测试覆盖完整边界、相似名称、字符串、委托、新增调用者/读写引用和颜色选择器变化；仅解析合成IL。
