"""Portable tests of Windows installer verification; no real DLL is loaded."""
import contextlib
import hashlib
import importlib.util
import io
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


REPO = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location(
    "package_check", REPO / "packaging/libmpv/check-package-windows.py")
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class WindowsPackageTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="vela package test ")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.reference = self.root / "input set"
        self.reference.mkdir()
        (self.reference / "libmpv-2.dll").write_bytes(b"test library, never loaded")
        (self.reference / "avcodec.dll").write_bytes(b"dependency bytes")
        lines = ["arch x86_64", "platform windows"]
        for name, version in check.libmpv.pinned_components(check.PINS).items():
            licence = f"licenses/{name}/COPYING"
            target = self.reference / licence
            target.parent.mkdir(parents=True)
            target.write_text(f"{name} licence text", encoding="utf-8")
            lines += [f"component {name} {version}", f"licence {name} {licence}"]
        build_patch = "licenses/ffmpeg/ffmpeg-schannel-revocation.patch"
        shutil.copyfile(REPO / "packaging/libmpv/windows/ffmpeg-schannel-revocation.patch",
                        self.reference / build_patch)
        lines.append(f"patch ffmpeg {build_patch}")
        for file in sorted(self.reference.rglob("*")):
            if file.is_file():
                content = file.read_bytes()
                lines.append(f"file {hashlib.sha256(content).hexdigest()} {len(content)} "
                             f"{file.relative_to(self.reference).as_posix()}")
        self.manifest = ("\n".join(lines) + "\n").encode()
        (self.reference / "MANIFEST").write_bytes(self.manifest)
        self.assertEqual(check.libmpv.check_manifest(self.reference, check.PINS), [])
        self.payload = self.root / "unpacked"
        self.app = self.payload / "PFiles" / "Vela"
        self.app.mkdir(parents=True)
        (self.app / "Vela.exe").write_bytes(b"application stand-in")
        self.shipped = self.app / "libmpv"
        shutil.copytree(self.reference, self.shipped)

    def test_valid_nsis_and_msi_layouts(self):
        self.assertEqual(check.verify_tree(self.payload, self.manifest), self.shipped)
        # 7-Zip may include an $INSTDIR prefix; the MSI has a PFiles prefix.
        for index, prefix in enumerate(("$INSTDIR", "", "PFiles/Vela")):
            root = self.root / f"layout-{index}"
            target = root / prefix
            shutil.copytree(self.app, target)
            self.assertEqual(check.verify_tree(root, self.manifest), target / "libmpv")

    def test_missing_dll(self):
        (self.shipped / "avcodec.dll").unlink()
        with self.assertRaisesRegex(check.PackageError, "missing: avcodec.dll"):
            check.verify_tree(self.payload, self.manifest)

    def test_modified_dll_same_length(self):
        (self.shipped / "avcodec.dll").write_bytes(b"Dependency bytes")
        with self.assertRaisesRegex(check.PackageError, "modified: avcodec.dll"):
            check.verify_tree(self.payload, self.manifest)

    def test_missing_licence(self):
        (self.shipped / "licenses/mpv/COPYING").unlink()
        with self.assertRaisesRegex(check.PackageError, "missing: licenses/mpv/COPYING"):
            check.verify_tree(self.payload, self.manifest)

    def test_missing_schannel_patch(self):
        (self.shipped / "licenses/ffmpeg/ffmpeg-schannel-revocation.patch").unlink()
        with self.assertRaisesRegex(check.PackageError, "missing: licenses/ffmpeg/ffmpeg-schannel"):
            check.verify_tree(self.payload, self.manifest)

    def test_schannel_patch_cannot_be_omitted_from_reference_and_package(self):
        path = "licenses/ffmpeg/ffmpeg-schannel-revocation.patch"
        manifest = b"\n".join(line for line in self.manifest.splitlines()
                              if not line.endswith(path.encode())) + b"\n"
        (self.shipped / path).unlink()
        (self.shipped / "MANIFEST").write_bytes(manifest)
        with self.assertRaisesRegex(check.PackageError, "omits the FFmpeg Schannel revocation patch"):
            check.verify_tree(self.payload, manifest)

    def test_extra_file(self):
        (self.shipped / "unexpected.dll").write_bytes(b"extra")
        with self.assertRaisesRegex(check.PackageError, "not in MANIFEST: unexpected.dll"):
            check.verify_tree(self.payload, self.manifest)

    def test_replaced_manifest_cannot_vouch_for_changed_bytes(self):
        file = self.shipped / "avcodec.dll"
        old = file.read_bytes()
        content = b"different library"
        file.write_bytes(content)
        manifest = self.manifest.replace(
            f"{hashlib.sha256(old).hexdigest()} {len(old)}".encode(),
            f"{hashlib.sha256(content).hexdigest()} {len(content)}".encode())
        (self.shipped / "MANIFEST").write_bytes(manifest)
        self.assertEqual(check.libmpv.check_manifest(self.shipped, check.PINS), [])
        with self.assertRaisesRegex(check.PackageError, "MANIFEST differs"):
            check.verify_tree(self.payload, self.manifest)

    def test_missing_manifest(self):
        (self.shipped / "MANIFEST").unlink()
        with self.assertRaises(FileNotFoundError):
            check.verify_tree(self.payload, self.manifest)

    def test_set_must_be_beside_application(self):
        self.shipped.rename(self.payload / "libmpv")
        with self.assertRaises(FileNotFoundError):
            check.verify_tree(self.payload, self.manifest)

    def test_missing_and_ambiguous_application(self):
        (self.payload / "vela.exe").write_bytes(b"second app")
        with self.assertRaisesRegex(check.PackageError, "found 2"):
            check.verify_tree(self.payload, self.manifest)
        (self.payload / "vela.exe").unlink()
        (self.app / "Vela.exe").unlink()
        with self.assertRaisesRegex(check.PackageError, "found 0"):
            check.verify_tree(self.payload, self.manifest)

    def test_symlink_refused(self):
        self.shipped.rename(self.app / "elsewhere")
        try:
            self.shipped.symlink_to(self.app / "elsewhere", target_is_directory=True)
        except OSError as error:
            self.skipTest(f"symlinks unavailable: {error}")
        with self.assertRaisesRegex(check.PackageError, "link or reparse point"):
            check.verify_tree(self.payload, self.manifest)

    def test_stale_pins(self):
        pins = self.root / "versions.txt"
        pins.write_text("mpv 0.0.0\n", encoding="utf-8")
        with self.assertRaisesRegex(check.PackageError, "component versions differ"):
            check.verify_tree(self.payload, self.manifest, pins)

    def test_inventory_requires_both_formats_and_checks_every_file(self):
        bundle = self.root / "bundle"
        for format_, file in (("nsis", "Vela_1-setup.exe"), ("msi", "Vela_1.msi"),
                              ("msi", "Vela_2.msi")):
            (bundle / format_).mkdir(exist_ok=True, parents=True)
            (bundle / format_ / file).write_bytes(b"installer stand-in")
        self.assertEqual(len(check.installers(bundle)), 3)
        for format_ in ("nsis", "msi"):
            (bundle / format_).rename(bundle / "hidden")
            with self.assertRaisesRegex(check.PackageError, "installer missing"):
                check.installers(bundle)
            (bundle / "hidden").rename(bundle / format_)

    def test_extractor_commands_preserve_paths_with_spaces_and_failures(self):
        destination = self.root / "extract with spaces"
        for suffix in ("-setup.exe", ".msi"):
            package = self.root / ("Vela package" + suffix)
            with patch.object(check.subprocess, "run") as run:
                check.unpack(package, destination, "7z.exe", "msiexec.exe")
            command = run.call_args.args[0]
            self.assertEqual(run.call_args.kwargs, {"check": True, "timeout": 300})
            if suffix == ".msi":
                self.assertTrue(command.startswith(f'msiexec.exe /a "{package}" /qn /norestart '))
                self.assertIn(f'/L*V "{destination.parent / "msiexec.log"}"', command)
                self.assertTrue(command.endswith(f' TARGETDIR="{destination}"'))
            else:
                self.assertEqual(command, ["7z.exe", "x", "-y", f"-o{destination}", str(package)])
            with patch.object(check.subprocess, "run", side_effect=subprocess.CalledProcessError(1, command)):
                with self.assertRaises(subprocess.CalledProcessError):
                    check.unpack(package, destination, "7z.exe", "msiexec.exe")

    def fake_unpack(self, package, destination, *_):
        shutil.copytree(self.payload, destination, dirs_exist_ok=True)

    def test_playback_receives_unpacked_set_in_a_fresh_process_each_time(self):
        paths = []
        def fake_play(command, **kwargs):
            self.assertEqual(command[:4], [sys.executable, "-I", "-B", str(check.CHECKER)])
            self.assertEqual(kwargs, {"check": True, "timeout": 90})
            shipped = Path(command[4])
            self.assertTrue(shipped.is_dir())
            self.assertNotEqual(shipped, self.reference)
            self.assertEqual((shipped / "MANIFEST").read_bytes(), self.manifest)
            paths.append(shipped)
        with patch.object(check, "unpack", side_effect=self.fake_unpack), \
                patch.object(check.subprocess, "run", side_effect=fake_play):
            for name in ("Vela-setup.exe", "Vela.msi"):
                check.check_package(Path(name), self.manifest, "7z", "msiexec")
        self.assertEqual(len(paths), 2, "both installers need playback checks")
        self.assertNotEqual(paths[0], paths[1])
        self.assertTrue(all(not path.exists() for path in paths), "temporary payloads cleaned up")

    def test_bad_bytes_are_never_loaded(self):
        (self.shipped / "libmpv-2.dll").write_bytes(b"corrupted")
        with patch.object(check, "unpack", side_effect=self.fake_unpack), \
                patch.object(check.subprocess, "run") as play:
            with self.assertRaisesRegex(check.PackageError, "modified: libmpv-2.dll"):
                check.check_package(Path("Vela.msi"), self.manifest, "7z", "msiexec")
            play.assert_not_called()

    def test_load_failure_fails_package(self):
        with patch.object(check, "unpack", side_effect=self.fake_unpack), \
                patch.object(check.subprocess, "run", side_effect=subprocess.CalledProcessError(1, ["python"])):
            with self.assertRaises(subprocess.CalledProcessError):
                check.check_package(Path("Vela.msi"), self.manifest, "7z", "msiexec")

    def test_preflight_checks_input_manifest_even_with_tools_present(self):
        with patch.object(check.sys, "platform", "win32"), \
                patch.object(check.ctypes, "sizeof", return_value=8), \
                patch.object(check.shutil, "which", side_effect=lambda name: name):
            self.assertEqual(check.preflight(self.reference), ("7z", "msiexec.exe"))
            (self.reference / "libmpv-2.dll").unlink()
            with self.assertRaisesRegex(check.PackageError, "input set: missing"):
                check.preflight(self.reference)

    def test_cli_checks_every_installer_and_returns_failure(self):
        bundle = self.root / "bundle"
        for folder, file in (("nsis", "Vela-setup.exe"), ("msi", "Vela.msi")):
            (bundle / folder).mkdir(parents=True)
            (bundle / folder / file).write_bytes(b"installer")
        arguments = [str(check.__file__), "--set", str(self.reference), "--bundle-dir", str(bundle)]
        with patch.object(sys, "argv", arguments), \
                patch.object(check, "preflight", return_value=("7z", "msiexec")), \
                patch.object(check, "check_package") as package:
            self.assertEqual(check.main(), 0)
            self.assertEqual([call.args[0].name for call in package.call_args_list],
                             ["Vela-setup.exe", "Vela.msi"])
            package.side_effect = check.PackageError("bad package")
            with contextlib.redirect_stderr(io.StringIO()) as errors:
                self.assertEqual(check.main(), 1)
            self.assertIn("FAIL Windows package: bad package", errors.getvalue())


if __name__ == "__main__":
    unittest.main()
