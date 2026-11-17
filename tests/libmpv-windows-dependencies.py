"""Portable PE fixtures and runtime audit tests; no Windows code is executed."""
import contextlib
import ctypes
import importlib.util
import io
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import unittest
from unittest.mock import patch


REPO = Path(__file__).resolve().parents[1]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, REPO / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


pe = load("windows_pe", "packaging/libmpv/windows-pe.py")
check = load("windows_check", "packaging/libmpv/check-libmpv-windows.py")


def fixture(imports=(), delay=(), forwarders=(), legacy_delay=False):
    """A PE32+ DLL with real import/delay descriptors and an export table.

    One file-backed .rdata section, valid INT/IAT entries and null-terminated
    tables. No compiler, SDK, third-party Python module or network is needed.
    objdump/llvm-readobj can independently inspect these same bytes.
    """
    data = bytearray(0x1200)

    def put(offset, fmt, *values):
        struct.pack_into("<" + fmt, data, offset, *values)

    data[:2] = b"MZ"
    put(0x3c, "I", 0x80)
    data[0x80:0x84] = b"PE\0\0"
    put(0x84, "HHIIIHH", 0x8664, 1, 0, 0, 0, 0xf0, 0x2022)
    optional = 0x98
    put(optional, "H", 0x20b)
    image_base = 0x10000000 if legacy_delay else 0x180000000
    put(optional + 24, "Q", image_base)
    put(optional + 32, "II", 0x1000, 0x200)
    put(optional + 56, "II", 0x2000, 0x200)
    put(optional + 68, "H", 3)
    put(optional + 108, "I", 16)
    put(0x188, "8sIIIIIIHHI", b".rdata", 0x1000, 0x1000, 0x1000, 0x200,
        0, 0, 0, 0, 0x40000040)
    cursor = 0x200

    def alloc(size):
        nonlocal cursor
        offset = cursor
        cursor = (cursor + size + 7) & ~7
        assert cursor <= len(data)
        return offset, offset + 0xe00

    def string(text):
        value = text.encode("ascii") + b"\0"
        offset, rva = alloc(len(value))
        data[offset:offset + len(value)] = value
        return rva

    def directory(index, rva, size):
        put(optional + 112 + 8 * index, "II", rva, size)

    for index, names, width in ((1, imports, 20), (13, delay, 32)):
        if not names:
            continue
        offset, rva = alloc(width * (len(names) + 1))
        directory(index, rva, width * (len(names) + 1))
        for i, name in enumerate(names):
            name_rva = string(name)
            hint, hint_rva = alloc(24)
            data[hint + 2:hint + 17] = b"pthread_create\0"
            int_offset, int_rva = alloc(16)
            iat_offset, iat_rva = alloc(16)
            put(int_offset, "QQ", hint_rva, 0)
            put(iat_offset, "QQ", hint_rva, 0)
            if index == 1:
                put(offset + width * i, "IIIII", int_rva, 0, 0, name_rva, iat_rva)
            else:
                _, handle_rva = alloc(8)
                bias = image_base if legacy_delay else 0
                put(offset + width * i, "IIIIIIII", 0 if legacy_delay else 1,
                    name_rva + bias, handle_rva + bias, iat_rva + bias, int_rva + bias, 0, 0, 0)
    if forwarders:
        offset, rva = alloc(40)
        addresses, address_rva = alloc(4 * len(forwarders))
        module_rva = string("fixture.dll")
        for i, name in enumerate(forwarders):
            # None represents an ordinary exported code/data address outside
            # the export directory; it must not turn into a dependency.
            put(addresses + 4 * i, "I", string(name) if name is not None else 0x1ff0)
        put(offset, "IIHHIIIIIII", 0, 0, 0, 0, module_rva, 1, len(forwarders), 0,
            address_rva, 0, 0)
        directory(0, rva, cursor - offset)
    return bytes(data)


def split_export_fixture(forwarder="libwinpthread-1.pthread_create"):
    """Export header/EAT at 0x1000, forwarder at 0x2000 in another section.

    The RVA gap has no file backing; the two raw sections are adjacent. The
    directory extent includes the gap because it identifies forwarder RVAs.
    """
    data = bytearray(fixture(forwarders=[forwarder])[:0x600])
    struct.pack_into("<H", data, 0x86, 2)  # NumberOfSections
    struct.pack_into("<I", data, 0x98 + 56, 0x3000)  # SizeOfImage
    struct.pack_into("<III", data, 0x188 + 8, 0x200, 0x1000, 0x200)
    struct.pack_into("<8sIIIIIIHHI", data, 0x1b0, b".fwd", 0x200, 0x2000,
                     0x200, 0x400, 0, 0, 0, 0, 0x40000040)
    value = forwarder.encode("ascii") + b"\0"
    data[0x400:0x400 + len(value)] = value
    struct.pack_into("<I", data, 0x228, 0x2000)  # EAT entry
    struct.pack_into("<II", data, 0x98 + 112, 0x1000, 0x1000 + len(value))
    return bytes(data)


class PEDependencyTests(unittest.TestCase):
    def test_ordinary_import(self):
        self.assertEqual(pe.pe_dependencies(fixture(imports=["LIBWINPTHREAD-1.DLL"])),
                         {"imports": ["libwinpthread-1.dll"], "delay_imports": [], "forwarders": []})

    def test_delay_import(self):
        self.assertEqual(pe.pe_dependencies(fixture(delay=["libwinpthread-1.dll"])),
                         {"imports": [], "delay_imports": ["libwinpthread-1.dll"], "forwarders": []})

    def test_legacy_delay_import(self):
        self.assertEqual(pe.pe_dependencies(fixture(delay=["legacy.dll"], legacy_delay=True))[
            "delay_imports"], ["legacy.dll"])

    def test_forwarded_export(self):
        self.assertEqual(pe.pe_dependencies(fixture(forwarders=["libwinpthread-1.pthread_create"])),
                         {"imports": [], "delay_imports": [], "forwarders": ["libwinpthread-1.dll"]})

    def test_forwarder_in_another_section(self):
        self.assertEqual(pe.pe_dependencies(split_export_fixture()),
                         {"imports": [], "delay_imports": [], "forwarders": ["libwinpthread-1.dll"]})

    def test_directory_outside_image_is_refused(self):
        for index, kind, name in ((0, "forwarders", "helper.symbol"),
                                  (1, "imports", "helper.dll"), (13, "delay", "helper.dll")):
            for image_size in (0x2000, 0x1800):
                with self.subTest(kind=kind, image_size=image_size):
                    data = bytearray(fixture(**{kind: [name]}))
                    struct.pack_into("<I", data, 0x98 + 56, image_size)
                    struct.pack_into("<I", data, 0x98 + 112 + 8 * index + 4,
                                     image_size - 0x1000 + 1)
                    with self.assertRaisesRegex(ValueError, "outside PE image"):
                        pe.pe_dependencies(data)

    def test_cross_section_export_references_must_be_mapped(self):
        for offset in (0x228, 0x200 + 28, 0x98 + 112):
            with self.subTest(offset=offset):
                data = bytearray(split_export_fixture())
                # The forwarder, EAT or header is in the gap. The last case
                # leaves the header's fields at +20 mapped, but not its start.
                struct.pack_into("<I", data, offset, 0xff0 if offset == 0x98 + 112 else 0x1800)
                with self.assertRaisesRegex(ValueError, "unmapped PE RVA"):
                    pe.pe_dependencies(data)

    def test_cross_section_forwarder_needs_terminator_within_directory(self):
        data = bytearray(split_export_fixture())
        size, = struct.unpack_from("<I", data, 0x98 + 116)
        struct.pack_into("<I", data, 0x98 + 116, size - 1)
        with self.assertRaisesRegex(ValueError, "unterminated PE string"):
            pe.pe_dependencies(data)

    def test_forwarders_by_ordinal_and_explicit_extension(self):
        self.assertEqual(pe.pe_dependencies(fixture(forwarders=["KERNEL32.#12", "Other.DLL.symbol"]))[
            "forwarders"], ["kernel32.dll", "other.dll"])

    def test_multiple_entries_and_normal_exports(self):
        self.assertEqual(pe.pe_dependencies(fixture(imports=["A.dll", "b.dll", "a.DLL"],
                                                   delay=["c.dll", "d.dll"],
                                                   forwarders=[None, "e.symbol", "f.#8"])),
                         {"imports": ["a.dll", "b.dll"], "delay_imports": ["c.dll", "d.dll"],
                          "forwarders": ["e.dll", "f.dll"]})

    def test_no_dependencies(self):
        self.assertEqual(pe.pe_dependencies(fixture()),
                         {"imports": [], "delay_imports": [], "forwarders": []})

    def test_malformed_directories_fail_closed(self):
        cases = []
        data = bytearray(fixture(delay=["missing.dll"]))
        struct.pack_into("<I", data, 0x204, 0x4000)
        cases.append(data)  # unmapped DLL name
        data = bytearray(fixture(delay=["missing.dll"]))
        struct.pack_into("<I", data, 0x98 + 112 + 13 * 8 + 4, 32)
        cases.append(data)  # no terminating descriptor
        data = bytearray(fixture(forwarders=["bad.symbol"]))
        struct.pack_into("<I", data, 0x200 + 28, 0x4000)
        cases.append(data)  # unmapped export address table
        cases += [fixture()[:512], b"not a PE", fixture(forwarders=["not_a_forwarder"]),
                  fixture(delay=["../escape.dll"])]
        for data in cases:
            with self.subTest(data_length=len(data)), self.assertRaises(ValueError):
                pe.pe_dependencies(data)


@unittest.skipUnless(os.name != "nt" and shutil.which("bash"), "Linux build shell")
class BuildClosureTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="vela PE closure ")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.out = self.root / "set"
        self.out.mkdir()
        # Source the actual recipe's definitions without its build dispatch.
        # Both needed() and the closure call are production code, not replicas.
        self.definitions = (REPO / "scripts/build-libmpv-windows.sh").read_text().split(
            "# Reject unknown steps before touching any output")[0]

    def shell(self, command):
        harness = self.root / "closure.sh"
        harness.write_text(self.definitions + '\nrecipe="$1"\nOUT="$2"\n' + command)
        return subprocess.run(["bash", str(harness), str(REPO / "packaging/libmpv"), str(self.out)],
                              capture_output=True, text=True, cwd=REPO, timeout=15)

    def test_closure_rejects_each_unshipped_dependency_kind(self):
        for kind, value in (("imports", "libwinpthread-1.dll"),
                            ("delay", "libwinpthread-1.dll"),
                            ("forwarders", "libwinpthread-1.pthread_create")):
            with self.subTest(kind=kind):
                (self.out / "libmpv-2.dll").write_bytes(fixture(**{kind: [value]}))
                run = self.shell("check_closure")
                self.assertEqual(run.returncode, 1, run.stdout + run.stderr)
                self.assertIn("unshipped dependency: libwinpthread-1.dll", run.stderr)

    def test_closure_accepts_shipped_and_windows_dependencies(self):
        (self.out / "libmpv-2.dll").write_bytes(fixture(
            imports=["KERNEL32.dll"], delay=["api-ms-win-core-path-l1-1-0.dll", "HELPER.dll"],
            forwarders=["ext-ms-win-example.#1", "helper.symbol"]))
        (self.out / "helper.dll").write_bytes(fixture())
        run = self.shell("check_closure")
        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)

    @unittest.skipUnless(shutil.which("strip"), "binutils")
    def test_closure_accepts_cross_section_export_before_and_after_stripping(self):
        library = self.out / "libmpv-2.dll"
        library.write_bytes(split_export_fixture("helper.symbol"))
        (self.out / "helper.dll").write_bytes(fixture())
        for stripped in (False, True):
            with self.subTest(stripped=stripped):
                if stripped:
                    run = subprocess.run(["strip", "--strip-unneeded", str(library)],
                                         capture_output=True, text=True, timeout=15)
                    self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
                run = self.shell("check_closure")
                self.assertEqual(run.returncode, 0, run.stdout + run.stderr)

    def test_closure_rejects_unshipped_cross_section_forwarder(self):
        (self.out / "libmpv-2.dll").write_bytes(split_export_fixture())
        run = self.shell("check_closure")
        self.assertEqual(run.returncode, 1, run.stdout + run.stderr)
        self.assertIn("unshipped dependency: libwinpthread-1.dll", run.stderr)

    def test_closure_checks_transitive_dependencies(self):
        (self.out / "libmpv-2.dll").write_bytes(fixture(delay=["helper.dll"]))
        (self.out / "helper.dll").write_bytes(fixture(forwarders=["libwinpthread-1.pthread_create"]))
        run = self.shell("check_closure")
        self.assertEqual(run.returncode, 1, run.stdout + run.stderr)
        self.assertIn("unshipped dependency: libwinpthread-1.dll", run.stderr)

    def test_closure_propagates_scanner_failure(self):
        (self.out / "libmpv-2.dll").write_bytes(b"not a PE")
        run = self.shell("check_closure")
        self.assertEqual(run.returncode, 1, run.stdout + run.stderr)
        self.assertIn("cannot read dependencies:", run.stderr)

    @unittest.skipUnless(shutil.which("objdump") and shutil.which("strip"), "binutils")
    def test_collector_follows_each_dependency_kind(self):
        prefix = self.root / "prefix"
        (prefix / "bin").mkdir(parents=True)
        (prefix / "lib").mkdir()
        # The recipe uses TARGET-objdump; this machine's native objdump also
        # understands PE. STRIP uses the available binutils executable too.
        (self.root / "target-objdump").symlink_to(shutil.which("objdump"))
        for kind, value in (("imports", "libwinpthread-1.dll"),
                            ("delay", "libwinpthread-1.dll"),
                            ("forwarders", "libwinpthread-1.pthread_create")):
            with self.subTest(kind=kind):
                (prefix / "bin/libmpv-2.dll").write_bytes(fixture(**{kind: [value]}))
                run = self.shell('PREFIX="${OUT%/*}/prefix"\nTARGET="${OUT%/*}/target"\nSTRIP=strip\ncollect')
                self.assertEqual(run.returncode, 1, run.stdout + run.stderr)
                self.assertIn("missing or ambiguous DLL: libwinpthread-1.dll", run.stderr)


class LoadedModuleTests(unittest.TestCase):
    directory = r"D:\Vela\libmpv"
    names = ["libmpv-2.dll", "helper.dll"]
    mpv = directory + r"\libmpv-2.dll"
    interpreter = r"C:\Python\python313.dll"
    windows = r"E:\Windows"
    runtimes = ("libwinpthread-1.dll", "libgcc_s_seh-1.dll", "libgcc_s_dw2-1.dll",
                "libgcc_s_sjlj-1.dll", "libstdc++-6.dll", "libssp-0.dll",
                "libatomic-1.dll", "libgomp-1.dll")

    def audit(self, modules, baseline=()):
        return check.check_loaded(self.directory, self.names, [self.mpv, *modules], True,
                                  baseline, self.windows)

    def test_new_external_module_names_its_path(self):
        for path in (r"C:\tools\foreign.dll", r"D:\Vela\libmpv-other\plugin.dll",
                     r"E:\Windows\System32-other\bad.dll", r"E:\Windows\other\bad.dll",
                     r"C:\Windows\System32\wrong-root.dll", r"C:\tools\module.pyd"):
            with self.subTest(path=path):
                self.assertEqual(self.audit([path]),
                                 ["new module loaded from outside the set and Windows: " + path.lower()])

    def test_forbidden_runtimes_in_windows_directories(self):
        for folder in (r"System32", r"SysWOW64", r"WinSxS\assembly",
                       r"System32\DriverStore\FileRepository", r"DriverStore\FileRepository"):
            for name in self.runtimes:
                path = self.windows + "\\" + folder + "\\" + name.upper()
                with self.subTest(path=path):
                    self.assertIn("forbidden runtime DLL loaded: " + path.lower(), self.audit([path]))

    def test_forbidden_runtimes_elsewhere(self):
        for folder in (r"C:\tools", self.directory):
            for name in self.runtimes:
                path = folder + "\\" + name
                with self.subTest(path=path):
                    self.assertIn("forbidden runtime DLL loaded: " + path.lower(), self.audit([path]))

    def test_baseline_and_manifest_cannot_exempt_forbidden_runtimes(self):
        for folder in (self.windows + r"\System32", self.directory, r"C:\Python"):
            path = folder + r"\libwinpthread-1.dll"
            with self.subTest(path=path):
                problems = check.check_loaded(self.directory, [*self.names, "libwinpthread-1.dll"],
                                              [self.mpv, path], True, [path.upper()], self.windows)
                self.assertIn("forbidden runtime DLL loaded: " + path.lower(), problems)

    def test_baseline_is_exempt_case_insensitively(self):
        self.assertEqual(self.audit([self.interpreter], baseline=[self.interpreter.upper()]), [])
        self.assertTrue(self.audit([self.interpreter]))

    def test_windows_directories_and_set_are_allowed(self):
        modules = [self.directory.upper() + r"\HELPER.DLL"]
        for suffix in (r"System32\kernel32.dll", r"SysWOW64\kernel32.dll",
                       r"WinSxS\assembly\module.dll"):
            modules.append(self.windows.upper() + "\\" + suffix)
        self.assertEqual(self.audit(modules), [])

    def test_driverstore_gpu_modules_are_allowed(self):
        for folder in (r"System32\DriverStore\FileRepository", r"DriverStore\FileRepository"):
            with self.subTest(folder=folder):
                self.assertEqual(self.audit([self.windows.upper() + "\\" + folder + r"\gpu.dll"]), [])

    def test_baseline_does_not_exempt_wrong_set_dll_or_unlisted_set_dll(self):
        foreign = r"C:\Python\helper.dll"
        unlisted = self.directory + r"\unlisted.dll"
        problems = self.audit([foreign, unlisted], baseline=[foreign, unlisted])
        self.assertIn("set DLL loaded from elsewhere: " + foreign.lower(), problems)
        self.assertIn("loaded unlisted DLL: " + unlisted.lower(), problems)

    def test_play_passes_unchanging_baseline_and_system_root_to_audit(self):
        self.assert_play_rejects_module(r"C:\tools\foreign.dll", r"new module.*c:\\tools\\foreign\.dll")

    def test_play_rejects_runtime_from_system32(self):
        self.assert_play_rejects_module(self.windows + r"\System32\libwinpthread-1.dll",
                                        r"forbidden runtime DLL loaded: e:\\windows\\system32\\libwinpthread-1\.dll")

    def assert_play_rejects_module(self, external, expected):
        class Kernel:
            def SetDefaultDllDirectories(self, flags):
                return True

        class Function:
            def __call__(self, *args):
                return 2 << 16

        class Library:
            def __getattr__(self, name):
                return Function()

        # The unexpected module appears on a later audit, after the set itself
        # loads. If the audit extends its baseline, drops the call, or omits the
        # real SystemRoot/baseline, this exercise fails for a different reason.
        baseline = [self.interpreter, self.windows + r"\System32\kernel32.dll"]
        loaded = baseline + [self.mpv, self.windows + r"\System32\DriverStore\gpu.dll"]
        snapshots = [baseline, baseline, loaded, loaded + [external]]
        event = check.Event(1, 0, 0)  # without the audit, an unexpected shutdown
        library = Library()
        library.mpv_wait_event = lambda *args: ctypes.pointer(event)
        with patch.object(check, "read_manifest", return_value=({n: None for n in self.names}, {"mpv": "0.42.0"})), \
                patch.object(check, "windows_api", return_value=Kernel()), \
                patch.object(check, "loaded_modules", side_effect=snapshots), \
                patch.dict(os.environ, {"SystemRoot": self.windows}), \
                patch.object(os, "add_dll_directory", return_value=contextlib.nullcontext(), create=True), \
                patch.object(check.ctypes, "CDLL", return_value=library), \
                patch.object(check.ctypes, "string_at", return_value=b"mpv 0.42.0"), \
                contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(RuntimeError, expected):
                check.play(Path(self.directory))


if __name__ == "__main__":
    unittest.main()
