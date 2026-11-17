"""仅索引静态IL证据，不加载或执行原厂程序集。"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys


FIELD_OPS = {"ldfld", "ldflda", "ldsfld", "ldsflda", "stfld", "stsfld"}
WRITE_OPS = {"stfld", "stsfld"}


class Module:
    def __init__(self, text, name):
        if not text.startswith("// IL code:"):
            raise ValueError(f"{name}缺少完整IL模块头")
        masked = re.sub(r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'|//[^\n]*', "", text)
        depth = 0
        for char in masked:
            depth += (char == "{") - (char == "}")
            if depth < 0:
                raise ValueError(f"{name}的IL括号不完整")
        if depth or not re.search(r"}\s*// end of class [^\n]+\s*$", text):
            raise ValueError(f"{name}缺少完整IL结束标记")
        self.name = name
        self.text = text
        self.methods = self._methods(text)
        if not self.methods:
            raise ValueError(f"{name}没有可索引的方法")

    def _methods(self, text):
        methods, pending = [], None
        for line_number, line in enumerate(text.splitlines(), 1):
            if re.match(r"\s*\.method\b", line):
                if pending is not None:
                    raise ValueError(f"{self.name}的方法边界不完整")
                pending = {"start": line_number, "lines": [], "instructions": []}
            if pending is None:
                continue
            pending["lines"].append(line)
            instruction = re.match(r"\s*(IL_[0-9a-fA-F]+):\s+(\S+)(?:\s+(.*))?$", line)
            if instruction:
                pending["instructions"].append({"line": line_number, "offset": instruction[1],
                                                 "opcode": instruction[2], "operand": instruction[3] or ""})
            end = re.match(r"\s*}\s*// end of method (.+)", line)
            if end:
                pending["name"] = end[1].strip()
                methods.append(pending)
                pending = None
        if pending is not None:
            raise ValueError(f"{self.name}存在未结束的方法")
        return methods

    def field(self, name):
        pattern = rf"(?m)^\s*\.field\b[^\r\n]*\b{re.escape(name)}\s*$"
        if not re.search(pattern, self.text):
            raise ValueError(f"{self.name}缺少预期字段{name}，不能沿用当前结论")
        references = []
        for method in self.methods:
            for instruction in method["instructions"]:
                if instruction["opcode"] in FIELD_OPS and re.search(rf"::{re.escape(name)}$", instruction["operand"]):
                    references.append({"method": method["name"], **instruction})
        strings = []
        for method in self.methods:
            for instruction in method["instructions"]:
                if instruction["opcode"] == "ldstr" and name in instruction["operand"]:
                    strings.append({"method": method["name"], **instruction})
        return {"references": references, "stringReferences": strings}

    def method(self, name):
        methods = [method for method in self.methods if method["name"] == name]
        if len(methods) != 1:
            raise ValueError(f"{self.name}的方法{name}缺失或有歧义")
        return methods[0]

    def calls(self, target):
        self.method(target)
        pattern = rf"(?<![\w]){re.escape(target)}\("
        return [{"method": method["name"], **instruction}
                for method in self.methods for instruction in method["instructions"]
                if instruction["opcode"] in {"call", "callvirt", "ldftn", "ldvirtftn"}
                and re.search(pattern, instruction["operand"])]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def only_methods(field, allowed):
    return not field["stringReferences"] and all(ref["method"] in allowed for ref in field["references"])


def selectors(method):
    found = []
    instructions = method["instructions"]
    for index, instruction in enumerate(instructions[:-1]):
        if instruction["opcode"] not in {"ldc.i4", "ldc.i4.s"}:
            continue
        if instructions[index + 1]["opcode"] != "stloc.1":
            continue
        value = int(instruction["operand"].split()[0], 0)
        if 240 <= value <= 255:
            found.append(value)
    require(found, "颜色方法未提供可核对的选择器布局")
    return sorted(set(found))


def assess(fnkey, cpuoc, kbled):
    tp = fnkey.field("SupportTpColor")
    require(not tp["references"] and not tp["stringReferences"], "TP能力出现活动引用，须重新追踪入口")
    public_xmp = cpuoc.field("Memory_XMP_Profile")
    require(not public_xmp["references"] and not public_xmp["stringReferences"], "公开XMP数据出现消费者，须重新追踪")
    decoded_xmp = cpuoc.field("Memory_XMP_Profile_From_WMI18")
    require(decoded_xmp["references"] and only_methods(decoded_xmp, {"OverClockModel::Read_package_WMI18"}),
            "XMP解码数据出现新消费者或缺失解码，须重新追踪")
    memory_voltage = cpuoc.field("Memory_Current_Voltage_From_WMI18")
    require(memory_voltage["references"] and only_methods(memory_voltage, {"OverClockModel::Read_package_WMI18"}),
            "内存电压字段出现非解码使用，须重新追踪")
    deep = fnkey.field("IsDeepS5Enabled")
    writes = [ref for ref in deep["references"] if ref["opcode"] in WRITE_OPS]
    reads = [ref for ref in deep["references"] if ref["opcode"] not in WRITE_OPS]
    require(writes and all(ref["method"] == "Features::GetDeepS5Status" for ref in writes), "Deep S5状态出现新生产者")
    require(reads and all(ref["method"] == "Form1::WndProc" for ref in reads) and not deep["stringReferences"],
            "Deep S5状态消费者变化，须重新审查")
    color = kbled.method("RGBKB::SetColor")
    color_selectors = selectors(color)
    require(color_selectors == [240, 241, 242, 243, 246], "键盘颜色选择器变化，须重新审查TP发送链")
    tp_calls = {}
    for target, expected in {
        "RGBKB::set_TP": {"RGBKB::FirstRun", "RGBKB::ReadAllReg"},
        "InsydeAcpiProt::WriteAcpiTPColor": {"RGBKB::set_TP", "RGBKB::WriteAllReg"},
        "RGBKB::SetColor": {"RGBKB::SetCustomColor", "RGBKB::SetBreathMode", "RGBKB::SetAllColor"},
    }.items():
        calls = kbled.calls(target)
        require({call["method"] for call in calls} == expected, f"{target}调用者变化，须重新追踪TP即时链")
        tp_calls[target] = calls
    return {"tpCapability": tp, "keyboardColorSelectors": color_selectors, "publicXmp": public_xmp,
            "decodedXmp": decoded_xmp, "decodedMemoryVoltage": memory_voltage, "deepS5State": deep,
            "tpCallSites": tp_calls,
            "scope": "仅描述输入版本的活动指令引用；不外推硬件能力或其他组件，不将保存字段视为即时控制"}


def fingerprint(path):
    return {"path": str(path.resolve()), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


def extract(tool, assembly, output):
    require(tool.name.lower() in {"ilspycmd", "ilspycmd.exe"} and tool.is_file(), "仅允许使用明确的ilspycmd静态提取工具")
    require(assembly.suffix.lower() in {".exe", ".dll"}, "必须指定原厂程序集文件")
    before = fingerprint(assembly)
    with output.open("w", encoding="utf-8") as target:
        result = subprocess.run([str(tool), "-il", str(assembly)], stdout=target, stderr=subprocess.PIPE, text=True,
                                encoding="utf-8", errors="replace", shell=False, check=False)
    require(result.returncode == 0, f"静态IL提取失败：{assembly.name}：{result.stderr[-1000:]}")
    require(fingerprint(assembly) == before, "原厂程序集在提取期间发生变化，未生成结论")
    return before


def audit(args):
    modules, inputs = {}, {}
    for kind in ("fnkey", "cpuoc", "kbled"):
        assembly = getattr(args, f"{kind}_assembly")
        il_path = args.output.parent / f"{kind}-source.il"
        original = extract(args.ilspy, assembly, il_path)
        module = Module(il_path.read_text(encoding="utf-8-sig"), kind)
        modules[kind] = module
        inputs[kind] = {"il": fingerprint(il_path), "assembly": original, "methods": len(module.methods)}
    return {"version": 1, "ok": True, "inputs": inputs,
            "evidence": assess(modules["fnkey"], modules["cpuoc"], modules["kbled"])}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ilspy", required=True, type=Path)
    for kind in ("fnkey", "cpuoc", "kbled"):
        parser.add_argument(f"--{kind}-assembly", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    try:
        report = audit(args)
    except (ValueError, OSError) as error:
        report = {"version": 1, "ok": False, "error": str(error)}
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    counts = {kind: data["methods"] for kind, data in report.get("inputs", {}).items()}
    print(json.dumps({"ok": report["ok"], "output": str(args.output), "methodCounts": counts, "error": report.get("error")}))
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    try:
        sys.exit(main())
    except (ValueError, OSError) as error:
        print(f"审查失败：{error}", file=sys.stderr)
        sys.exit(1)
