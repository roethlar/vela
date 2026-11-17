#!/usr/bin/env python3
"""List a Windows x86_64 PE's dependencies, using only Python's standard library.

The build container already has Python. Some objdump versions print the delay
directory's address without decoding its descriptors, so read the three PE
directories directly instead of relying on objdump's human-readable output.
"""
import argparse
import json
from pathlib import Path
import re
import struct
import sys


class PE:
    """Bounds-checked access to file-backed PE32+ RVAs (no loading or execution)."""

    def __init__(self, data):
        self.data = data
        if self.read(0, 2) != b"MZ":
            raise ValueError("not a DOS/PE image")
        pe, = self.unpack("I", 0x3c)
        if self.read(pe, 4) != b"PE\0\0":
            raise ValueError("not a PE image")
        machine, count, _, _, _, optional_size, _ = self.unpack("HHIIIHH", pe + 4)
        optional = pe + 24
        self.read(optional, optional_size)
        if machine != 0x8664 or optional_size < 112 or self.unpack("H", optional)[0] != 0x20b:
            raise ValueError("expected an x86_64 PE32+ image")
        self.image_base, = self.unpack("Q", optional + 24)
        self.image_size, = self.unpack("I", optional + 56)
        self.header_size, = self.unpack("I", optional + 60)
        count_dirs, = self.unpack("I", optional + 108)
        if count_dirs > (optional_size - 112) // 8:
            raise ValueError("truncated PE data directories")
        self.directories = [self.unpack("II", optional + 112 + 8 * i)
                            for i in range(count_dirs)]
        self.sections = []
        spans = []
        for i in range(count):
            section = optional + optional_size + 40 * i
            self.read(section, 40)
            virtual_size, rva, raw_size, raw = self.unpack("IIII", section + 8)
            self.read(raw, raw_size)
            self.sections.append((rva, max(virtual_size, raw_size), raw, raw_size))
            spans.append((rva, virtual_size or raw_size))
        # location() takes the first section that maps an RVA, while Windows'
        # loader lets a later section's bytes win where two overlap: refuse
        # an ambiguous image rather than read bytes the loader does not use.
        previous_end = 0
        for start, extent in sorted(spans):
            if start < previous_end:
                raise ValueError(f"overlapping PE sections at RVA {start:#x}")
            previous_end = max(previous_end, start + extent)

    def read(self, offset, size):
        if offset < 0 or size < 0 or offset + size > len(self.data):
            raise ValueError("truncated PE data")
        return self.data[offset:offset + size]

    def unpack(self, fmt, offset):
        return struct.unpack("<" + fmt, self.read(offset, struct.calcsize("<" + fmt)))

    def image_range(self, rva, size):
        if rva <= 0:
            raise ValueError("null PE RVA")
        if size < 0 or rva >= self.image_size or size > self.image_size - rva:
            raise ValueError(f"range outside PE image: {rva:#x} + {size:#x}")

    def location(self, rva, size):
        self.image_range(rva, size)
        if rva < self.header_size and rva + size <= self.header_size:
            self.read(rva, size)
            return rva, min(self.header_size, self.image_size, len(self.data))
        for start, virtual_size, raw, raw_size in self.sections:
            delta = rva - start
            if 0 <= delta < virtual_size and delta + size <= raw_size:
                self.read(raw + delta, size)
                return raw + delta, raw + min(raw_size, self.image_size - start)
        raise ValueError(f"unmapped PE RVA: {rva:#x}")

    def at(self, fmt, rva):
        offset, _ = self.location(rva, struct.calcsize("<" + fmt))
        return self.unpack(fmt, offset)

    def string(self, rva, limit=None):
        offset, end = self.location(rva, 1)
        if limit is not None:
            end = min(end, offset + limit)
        nul = self.data.find(b"\0", offset, end)
        if nul < 0:
            raise ValueError("unterminated PE string")
        return self.data[offset:nul].decode("ascii")

    def directory(self, index):
        rva, size = self.directories[index] if index < len(self.directories) else (0, 0)
        if bool(rva) != bool(size):
            raise ValueError(f"incomplete PE directory {index}")
        if rva:
            # A directory's RVA extent can span sections and alignment gaps.
            # Only the structures/strings we actually read need file backing.
            self.image_range(rva, size)
        return rva, size


def dll_name(name):
    """Normalize loader names without letting a PE name escape the set directory."""
    if not re.fullmatch(r"[a-zA-Z0-9_+.-]+", name) or name in (".", ".."):
        raise ValueError(f"invalid dependency DLL name: {name!r}")
    return name.lower() if "." in name else name.lower() + ".dll"


def pe_dependencies(data):
    """Pure dependency extraction, retaining each kind for inspection/tests."""
    pe = PE(data)
    result = {"imports": set(), "delay_imports": set(), "forwarders": set()}
    for index, kind, width in ((1, "imports", 20), (13, "delay_imports", 32)):
        rva, size = pe.directory(index)
        if not rva:
            continue
        for offset in range(0, size - width + 1, width):
            descriptor = pe.at("I" * (width // 4), rva + offset)
            if not any(descriptor):
                break
            if kind == "imports":
                name_rva = descriptor[3]
            else:
                attributes, name_rva = descriptor[:2]
                if attributes not in (0, 1):
                    raise ValueError("unknown delay import attributes")
                # Old descriptors store VAs; current ones have the RVA flag.
                if not attributes:
                    name_rva -= pe.image_base
            result[kind].add(dll_name(pe.string(name_rva)))
        else:
            raise ValueError(f"unterminated {kind} directory")

    rva, size = pe.directory(0)
    if rva:
        if size < 40:
            raise ValueError("truncated export directory")
        # Map the complete header, not only the fields used below.
        header = pe.at("IIHHIIIIIII", rva)
        count, addresses = header[6], header[8]
        if count:
            pe.image_range(addresses, 4 * count)
        for i in range(count):
            target, = pe.at("I", addresses + 4 * i)
            # EAT entries inside the export directory are forwarder strings;
            # ordinary exported code/data addresses are not dependencies.
            if rva <= target < rva + size:
                forwarder = pe.string(target, rva + size - target)
                module, separator, symbol = forwarder.rpartition(".")
                if not separator or not symbol:
                    raise ValueError(f"invalid export forwarder: {forwarder!r}")
                result["forwarders"].add(dll_name(module))
    return {kind: sorted(names) for kind, names in result.items()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dll", type=Path)
    parser.add_argument("--json", action="store_true", help="report dependencies by kind")
    args = parser.parse_args()
    try:
        groups = pe_dependencies(args.dll.read_bytes())
    except (OSError, ValueError) as error:
        print(f"{args.dll}: {error}", file=sys.stderr)
        return 1
    if args.json:
        print(json.dumps(groups, sort_keys=True))
    else:
        for name in sorted({name for names in groups.values() for name in names}):
            print(name)
    return 0


if __name__ == "__main__":
    sys.exit(main())
