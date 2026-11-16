"""Portable macOS packaging guards. Apple tools are doubles, never real dylibs.

Hashing, inventory, config generation and check orchestration are real. The
release run must prove actual codesign/lipo/dyld/notarization behavior.
"""
import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import plistlib
import shutil
import shlex
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


REPO = Path(__file__).resolve().parents[1]


def module(name, file):
    spec = importlib.util.spec_from_file_location(name, REPO / file)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


checker = module("checker", "packaging/libmpv/check-libmpv-macos.py")
check = checker.package
writer = module("writer", "packaging/libmpv/write-macos-overlay.py")
signer = module("signer", "packaging/libmpv/macos-codesign.py")
DETAILS = ("TeamIdentifier=ABCDE12345\nCodeDirectory v=20500 size=123 flags=0x10000(runtime)\n"
           "Authority=Developer ID Application: Test (ABCDE12345)\nTimestamp=Nov 16, 2026\n")


class MacPackageTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="vela mac package ")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.reference = self.root / "input set"
        self.reference.mkdir()
        self.names = {"libmpv.2.dylib", "libavcodec.63.dylib"}
        for name in self.names:
            slices = {arch: f"{name} {arch} code|signature|adhoc" for arch in check.macho.ARCHES}
            (self.reference / name).write_text(json.dumps(slices))
        lines = ["arch universal", "platform macos", "minimum-macos 11.0"]
        for name, version in check.macho.pins_for_macos(check.PINS).items():
            licence = f"licenses/{name}/COPYING"
            target = self.reference / licence
            target.parent.mkdir(parents=True)
            target.write_text(name + " licence text")
            lines += [f"component {name} {version}", f"licence {name} {licence}"]
        build_patch = "licenses/mpv/mpv-sdk-only.patch"
        (self.reference / build_patch).write_text("build patch")
        lines += [f"patch mpv {build_patch}"]
        for file in sorted(self.reference.rglob("*")):
            if file.is_file():
                data = file.read_bytes()
                lines += [f"file {hashlib.sha256(data).hexdigest()} {len(data)} {file.relative_to(self.reference).as_posix()}"]
        (self.reference / "MANIFEST").write_text("\n".join(lines) + "\n")
        self.app = self.root / "Vela.app"
        self.frameworks = self.app / "Contents/Frameworks"
        self.resources = self.app / "Contents/Resources/libmpv"
        self.frameworks.mkdir(parents=True)
        self.resources.mkdir(parents=True)
        shutil.copytree(self.reference / "licenses", self.resources / "licenses")
        shutil.copyfile(self.reference / "MANIFEST", self.resources / "MANIFEST")
        for name in self.names:
            content = (self.reference / name).read_text().replace("adhoc", "Developer ID CMS")
            (self.frameworks / name).write_text(content)
        self.info = self.app / "Contents/Info.plist"
        self.info.write_bytes(plistlib.dumps({"CFBundleExecutable": "vela", "LSMinimumSystemVersion": "11.0"}))
        (self.app / "Contents/MacOS").mkdir()
        (self.app / "Contents/MacOS/vela").write_bytes(b"Vela executable")
        self.calls = []
        self.bad_details = {}
        self.failed_tool = None
        self.addCleanup(patch.stopall)
        patch.object(check, "run", side_effect=self.apple).start()
        patch.object(check.macho, "validate").start()

    def apple(self, *arguments):
        args = list(map(str, arguments))
        self.calls.append(args)
        if self.failed_tool and self.failed_tool in args:
            raise subprocess.CalledProcessError(1, args, stderr="Apple tool failed")
        if "lipo" in args:
            if "-archs" in args:
                return "x86_64 arm64\n"
            source = Path(args[2])
            arch = args[args.index("-thin") + 1]
            Path(args[-1]).write_text(json.loads(source.read_text())[arch])
        elif "--remove-signature" in args:
            copy = Path(args[-1])
            self.assertNotIn(copy.parent, (self.reference, self.frameworks))
            copy.write_text(copy.read_text().split("|signature|")[0])
        elif "--display" in args:
            return self.bad_details.get((args[-1], args[args.index("--architecture") + 1]), DETAILS)
        return ""

    def verify(self, **kwargs):
        with contextlib.redirect_stdout(io.StringIO()):
            return check.verify_app(self.app, self.reference, **kwargs)

    def test_signed_set_uses_copies_and_checks_every_slice(self):
        before = {p: p.read_bytes() for p in [*self.reference.glob("*.dylib"), *self.frameworks.glob("*.dylib")]}
        self.assertEqual(self.verify(), self.frameworks)
        self.assertEqual(before, {p: p.read_bytes() for p in before})
        for path in [self.app, *(self.frameworks / n for n in self.names)]:
            for arch in check.macho.ARCHES:
                self.assertIn(["/usr/bin/codesign", "--display", "--verbose=4", "--architecture", arch, str(path)], self.calls)
        self.assertEqual(len([c for c in self.calls if "--remove-signature" in c]), len(self.names) * 4)
        check.macho.validate.assert_called_once_with(self.frameworks)

    def test_overlay_lists_manifest_dylibs_and_only_notices_as_resources(self):
        result = writer.overlay(self.reference)["bundle"]
        self.assertEqual(result["macOS"], {"minimumSystemVersion": "11.0", "hardenedRuntime": True,
                         "frameworks": [str(self.reference / n) for n in sorted(self.names)]})
        self.assertEqual(result["resources"], {str(self.reference / "licenses") + "/": "libmpv/licenses/",
                                             str(self.reference / "MANIFEST"): "libmpv/MANIFEST"})
        self.assertTrue(result["license"].startswith("MIT AND GPL-2.0-or-later AND LGPL-2.1-or-later"))
        self.assertIn("Apache-2.0", result["license"])
        # A new versioned name in the manifest is included without editing code.
        old = self.reference / "libavcodec.63.dylib"
        old.rename(self.reference / "libavcodec.64.dylib")
        manifest = self.reference / "MANIFEST"
        manifest.write_text(manifest.read_text().replace("libavcodec.63.dylib", "libavcodec.64.dylib"))
        self.assertIn(str(self.reference / "libavcodec.64.dylib"), writer.overlay(self.reference)["bundle"]["macOS"]["frameworks"])

    def test_overlay_and_app_refuse_a_corrupted_input_set(self):
        (self.reference / "libmpv.2.dylib").write_bytes(b"changed")
        for call in [lambda: writer.overlay(self.reference), self.verify]:
            with self.assertRaisesRegex(ValueError, "input set: modified"):
                call()

    @unittest.skipUnless(sys.platform != "win32" and shutil.which("bash"), "requires POSIX bash")
    def test_preparation_writes_real_overlay_and_executable_launcher(self):
        root = self.root / "checkout with spaces"
        for name in ["scripts/prepare-macos-package.sh", "packaging/libmpv/write-macos-overlay.py",
                     "packaging/libmpv/macos/macho.py", "packaging/libmpv/versions.txt"]:
            target = root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(REPO / name, target)
        shutil.copytree(self.reference, root / ".libmpv/out/libmpv-macos-universal")
        subprocess.run(["bash", str(root / "scripts/prepare-macos-package.sh")], check=True,
                       capture_output=True, text=True, timeout=30)
        generated = json.loads((root / ".libmpv/tauri.libmpv-macos.conf.json").read_text())
        self.assertEqual(generated, writer.overlay(root / ".libmpv/out/libmpv-macos-universal"))
        launcher = root / ".libmpv/sign-tools/codesign"
        self.assertTrue(launcher.stat().st_mode & 0o111)
        # Exercise the launcher's repo lookup and argv forwarding without
        # calling Apple's tool: the fixture target only prints its arguments.
        (root / "packaging/libmpv/macos-codesign.py").write_text(
            'import json, sys; print(json.dumps(sys.argv[1:]))\n')
        args = ["--force", "-s", "Identity with spaces", "/path with space/libmpv.2.dylib"]
        result = subprocess.run([str(launcher), *args], check=True, capture_output=True, text=True, timeout=30)
        self.assertEqual(json.loads(result.stdout), args)

    def test_missing_extra_or_misplaced_dylibs(self):
        path = self.frameworks / "libmpv.2.dylib"
        path.rename(self.resources / path.name)
        with self.assertRaisesRegex(ValueError, "Frameworks inventory"):
            self.verify()
        (self.resources / path.name).rename(path)
        (self.frameworks / "extra.dylib").write_bytes(b"extra")
        with self.assertRaisesRegex(ValueError, "Frameworks inventory"):
            self.verify()

    def test_changed_code_in_either_slice(self):
        path = self.frameworks / "libmpv.2.dylib"
        original = path.read_text()
        for arch in check.macho.ARCHES:
            with self.subTest(arch=arch):
                contents = json.loads(original)
                contents[arch] = "wrong code|signature|Developer ID CMS"
                path.write_text(json.dumps(contents))
                with self.assertRaisesRegex(ValueError, f"code differs.*{arch}"):
                    self.verify()
        path.write_text(original)

    def test_licences_and_manifest_must_match_reference(self):
        for name in ["licenses/mpv/COPYING", "MANIFEST"]:
            path = self.resources / name
            original = path.read_bytes()
            path.write_bytes(b"replacement")
            with self.assertRaisesRegex(ValueError, "packaged resource differs"):
                self.verify()
            path.unlink()
            with self.assertRaisesRegex(ValueError, "Resources/libmpv inventory"):
                self.verify()
            path.write_bytes(original)
        (self.resources / "extra").write_text("extra")
        with self.assertRaisesRegex(ValueError, "Resources/libmpv inventory"):
            self.verify()

    def test_links_cannot_supply_the_set(self):
        path = self.frameworks / "libmpv.2.dylib"
        path.unlink()
        try:
            path.symlink_to(self.reference / path.name)
        except OSError:
            self.skipTest("host does not permit symlinks")
        with self.assertRaisesRegex(ValueError, "symbolic link"):
            self.verify()

    def test_wrong_team_missing_runtime_authority_or_timestamp_in_either_slice(self):
        for arch in check.macho.ARCHES:
            for before, after, reason in [
                ("ABCDE12345", "OTHER12345", "Team ID"),
                ("flags=0x10000(runtime)", "flags=0x0(none)", "hardened runtime"),
                ("Authority=Developer ID Application:", "Authority=Apple Development:", "Developer ID"),
                ("Timestamp=Nov 16, 2026", "Signed Time=Nov 16, 2026", "timestamp"),
            ]:
                with self.subTest(arch=arch, reason=reason):
                    self.bad_details = {(str(self.frameworks / "libmpv.2.dylib"), arch): DETAILS.replace(before, after)}
                    with self.assertRaisesRegex(ValueError, reason):
                        self.verify()

    def test_app_signing_and_deployment_floor_are_required(self):
        self.bad_details = {(str(self.app), "arm64"): DETAILS.replace("TeamIdentifier=ABCDE12345", "TeamIdentifier=not set")}
        with self.assertRaisesRegex(ValueError, "Team ID"):
            self.verify()
        self.bad_details = {}
        self.info.write_bytes(plistlib.dumps({"CFBundleExecutable": "vela", "LSMinimumSystemVersion": "10.13"}))
        with self.assertRaisesRegex(ValueError, "minimum 11.0"):
            self.verify()

    def test_apple_tool_failures_and_nonuniversal_dylibs_fail_closed(self):
        for operation in ["--verify", "--display", "--remove-signature", "-thin"]:
            self.failed_tool = operation
            with self.assertRaises(subprocess.CalledProcessError):
                self.verify()
        self.failed_tool = None
        original = self.apple
        with patch.object(check, "run", side_effect=lambda *a: "arm64\n" if "-archs" in a else original(*a)):
            with self.assertRaisesRegex(ValueError, "wrong architectures"):
                self.verify()

    def test_unsigned_mode_keeps_provenance_checks(self):
        self.bad_details = {(str(self.app), "arm64"): "unsigned"}
        self.assertEqual(self.verify(require_signed=False), self.frameworks)
        self.assertFalse(any("--display" in c for c in self.calls))
        (self.resources / "MANIFEST").write_text("changed")
        with self.assertRaisesRegex(ValueError, "packaged resource differs"):
            self.verify(require_signed=False)

    def invoke(self, *arguments):
        with patch.object(sys, "argv", ["check", str(self.reference), "--app", str(self.app), *arguments]), \
                patch.object(sys, "platform", "darwin"), contextlib.redirect_stdout(io.StringIO()):
            return checker.main()

    def test_controller_checks_app_then_launches_both_app_workers_cleanly(self):
        with patch.object(checker.macho, "validate") as validate, \
                patch.object(checker.subprocess, "run", return_value=subprocess.CompletedProcess([], 0)) as launch:
            self.assertEqual(self.invoke(), 0)
            self.assertEqual(launch.call_count, 2)
            validate.assert_called_once_with(self.frameworks)
            for arch, call in zip(("arm64", "x86_64"), launch.call_args_list):
                args = call.args[0]
                self.assertEqual(args[:5], ["/usr/bin/arch", "-" + arch, "/usr/bin/python3", "-I", "-B"])
                self.assertIn(str(self.reference), args)
                self.assertEqual(args[args.index("--app") + 1], str(self.app))
                self.assertNotIn("--unsigned-app", args)
                self.assertEqual(call.kwargs["timeout"], 90)
                self.assertFalse(any(k.startswith(("DYLD_", "LD_", "PYTHON")) for k in call.kwargs["env"]))
            launch.return_value = subprocess.CompletedProcess([], 1)
            self.assertEqual(self.invoke(), 1)
            launch.side_effect = subprocess.TimeoutExpired("worker", 90)
            self.assertEqual(self.invoke(), 1)

    def test_worker_plays_untouched_app_path_and_never_loads_failed_provenance(self):
        with patch.object(checker, "play") as play:
            self.assertEqual(self.invoke("--worker", "arm64"), 0)
            play.assert_called_once_with(self.frameworks, "arm64", None, self.reference)
            play.reset_mock()
            (self.resources / "MANIFEST").write_text("changed")
            self.assertEqual(self.invoke("--worker", "arm64"), 1)
            play.assert_not_called()

    def test_signing_wrapper_keeps_keychain_and_identity_and_only_changes_dylibs(self):
        arguments = ["--force", "-s", "Developer ID Application: Owner (ABCDE12345)",
                     "--keychain", "/temporary keychain", str(self.frameworks / "libmpv.2.dylib")]
        self.assertEqual(signer.command(arguments), ["/usr/bin/codesign", "--options", "runtime", "--timestamp", *arguments])
        for target in [self.app, self.reference / "libmpv.2.dylib", self.root / "Vela.dmg"]:
            args = [*arguments[:-1], str(target)]
            self.assertEqual(signer.command(args), ["/usr/bin/codesign", *args])
        args = ["--verify", "--strict", arguments[-1]]
        self.assertEqual(signer.command(args), ["/usr/bin/codesign", *args])

    @unittest.skipUnless(sys.platform != "win32" and shutil.which("bash"), "requires POSIX bash")
    def test_dmg_check_mounts_readonly_checks_actual_app_and_fails_closed(self):
        root = self.root / "dmg checkout"
        (root / "scripts").mkdir(parents=True)
        (root / "packaging/libmpv").mkdir(parents=True)
        (root / "bundle/dmg").mkdir(parents=True)
        (root / "bundle/dmg/Vela.dmg").write_text("image stand-in")
        (root / "tmp").mkdir()
        log = root / "calls.jsonl"
        # Replace only Apple's executable paths in the fixture copy. The
        # production script has no environment override for its tools.
        body = '''import json, os, pathlib, shutil, sys
root = pathlib.Path(__file__).resolve().parent
if root.name == 'libmpv': root = root.parent.parent
kind = pathlib.Path(__file__).stem
args = sys.argv[1:]
with (root / 'calls.jsonl').open('a') as log: log.write(json.dumps([kind, *args]) + '\\n')
operation = args[0] if kind == 'hdiutil' else kind
if os.environ.get('VELA_TEST_FAIL') == operation: sys.exit(1)
if kind == 'hdiutil' and args[0] == 'attach':
    assert '-readonly' in args and '-nobrowse' in args
    mount = pathlib.Path(args[args.index('-mountpoint') + 1])
    (mount / 'Vela.app').mkdir()
elif kind == 'hdiutil' and args[0] == 'detach':
    shutil.rmtree(pathlib.Path(args[-1]) / 'Vela.app')
elif kind == 'check-libmpv-macos':
    assert '--app' in args
    app = pathlib.Path(args[args.index('--app') + 1])
    assert app.parent.parent == root / 'tmp' and app.is_dir()
'''
        script = (REPO / "scripts/check-macos-package.sh").read_text()
        for apple in ["/usr/bin/hdiutil", "/usr/bin/xcrun", "/usr/sbin/spctl"]:
            stub = root / Path(apple).name
            stub.write_text(f"#!{sys.executable}\n" + body)
            stub.chmod(0o755)
            script = script.replace(apple, shlex.quote(str(stub)))
        script = script.replace("/usr/bin/python3", shlex.quote(sys.executable))
        (root / "scripts/check-macos-package.sh").write_text(script)
        (root / "packaging/libmpv/check-libmpv-macos.py").write_text(body)
        env = {**os.environ, "TMPDIR": str(root / "tmp")}
        for mode, failure in [("true", ""), ("false", ""), ("true", "attach"),
                              ("true", "xcrun"), ("true", "check-libmpv-macos"), ("true", "spctl")]:
            with self.subTest(mode=mode, failure=failure):
                log.write_text("")
                run = subprocess.run(["bash", str(root / "scripts/check-macos-package.sh"), "bundle", mode],
                                     env={**env, "VELA_TEST_FAIL": failure}, text=True,
                                     capture_output=True, timeout=30)
                self.assertEqual(run.returncode == 0, not failure, run.stderr)
                calls = [json.loads(line) for line in log.read_text().splitlines()]
                self.assertEqual(calls[0][:2], ["hdiutil", "attach"])
                self.assertIn("-readonly", calls[0])
                if failure != "attach":
                    self.assertEqual(calls[-1][:2], ["hdiutil", "detach"])
                self.assertEqual(list((root / "tmp").iterdir()), [], "mount directory cleaned up")
                if not failure:
                    kinds = [c[0] for c in calls]
                    self.assertEqual(kinds, ["hdiutil", "xcrun", "check-libmpv-macos", "spctl", "hdiutil"]
                                     if mode == "true" else ["hdiutil", "check-libmpv-macos", "hdiutil"])
                    play = next(c for c in calls if c[0] == "check-libmpv-macos")
                    self.assertEqual("--unsigned-app" in play, mode == "false")


if __name__ == "__main__":
    unittest.main()
