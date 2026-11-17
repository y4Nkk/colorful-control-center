"""离线提取 pow/REGF 中的显式 AC/DC 索引，不加载注册表、不调用电源 API。

格式依据：libyal/libregf 的 Windows NT Registry File (REGF) format。
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct


def require(condition, message):
    if not condition:
        raise ValueError(message)


def uint(raw, offset, size=4):
    return struct.unpack_from("<I" if size == 4 else "<H", raw, offset)[0]


class Hive:
    def __init__(self, raw):
        require(4096 <= len(raw) <= 8 * 1024 * 1024, "文件大小不在离线解析范围内")
        require(raw[:4] == b"regf" and uint(raw, 20) == 1 and 2 <= uint(raw, 24) <= 5, "不支持的 REGF 格式")
        self.raw, self.visited, self.keys = raw, set(), {}

    def cell(self, offset):
        position = 4096 + offset
        require(4096 <= position < len(self.raw) - 4, "单元偏移越界")
        size = -struct.unpack_from("<i", self.raw, position)[0]
        require(size >= 8 and position + size <= len(self.raw), "单元长度或分配标志无效")
        return self.raw[position + 4:position + size]

    def subkeys(self, offset, visited=None):
        visited = set() if visited is None else visited
        require(offset not in visited, "子键列表循环")
        visited.add(offset)
        data = self.cell(offset)
        signature, count = data[:2], uint(data, 2, 2)
        require(signature in (b"lf", b"lh", b"li", b"ri"), "子键列表类型未知")
        stride = 8 if signature in (b"lf", b"lh") else 4
        for index in range(count):
            child = uint(data, 4 + index * stride)
            if signature == b"ri":
                yield from self.subkeys(child, visited)
            else:
                yield child

    def value(self, offset):
        data = self.cell(offset)
        require(data[:2] == b"vk", "值单元签名无效")
        name = data[20:20 + uint(data, 2, 2)].decode("cp1252" if uint(data, 16, 2) & 1 else "utf-16le")
        size, kind = uint(data, 4), uint(data, 12)
        if name in ("ACSettingIndex", "DCSettingIndex"):
            require(kind == 4, "电源索引必须是 DWORD")
        if size & 0x80000000:
            size &= 0x7fffffff
            require(size <= 4, "内联数据长度无效")
            content = data[8:8 + size]
        else:
            content = self.cell(uint(data, 8))[:size] if size else b""
        require(len(content) == size, "值数据截断")
        if kind == 4:
            require(size == 4, "DWORD 长度无效")
            return name, uint(content, 0)
        return name, None

    def walk(self, offset, parent=()):
        require(offset not in self.visited and len(self.visited) < 4096, "键循环或数量过多")
        self.visited.add(offset)
        data = self.cell(offset)
        require(data[:2] == b"nk", "命名键签名无效")
        name = data[76:76 + uint(data, 72, 2)].decode("cp1252" if uint(data, 2, 2) & 32 else "utf-16le")
        path = (*parent, name.lower())
        if uint(data, 36):
            values = self.cell(uint(data, 40))
            self.keys[path] = dict(self.value(uint(values, i * 4)) for i in range(uint(data, 36)))
        if uint(data, 20):
            children = list(self.subkeys(uint(data, 28)))
            require(len(children) == uint(data, 20), "子键数量不一致")
            for child in children:
                self.walk(child, path)


def extract(path):
    raw = path.read_bytes()
    hive = Hive(raw)
    hive.walk(uint(raw, 36))
    entries = []
    for key, values in hive.keys.items():
        if not any(name in values for name in ("ACSettingIndex", "DCSettingIndex")):
            continue
        parts = key[1:]
        require(1 <= len(parts) <= 2, "电源索引键层级无效")
        entries.append({"subgroup": parts[0] if len(parts) == 2 else "fea3413e-7e05-4911-9a71-700331f1c294",
                        "setting": parts[-1], "ac": values.get("ACSettingIndex"), "dc": values.get("DCSettingIndex")})
    return {"sha256": hashlib.sha256(raw).hexdigest(), "entries": entries}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--check-index", type=Path)
    args = parser.parse_args()
    result = extract(args.source)
    if args.check_index:
        require(json.loads(args.check_index.read_text(encoding="utf-8")) == result, "生成索引与原始资源不一致")
        print(f"ok: {len(result['entries'])} 个设置，原始文件与索引一致")
    elif args.output:
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    else:
        print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
