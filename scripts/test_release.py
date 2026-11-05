"""Offline release-packaging tests; fixture bytes are not cross-platform binaries."""
import importlib.util
from pathlib import Path
import re
import tarfile
import tempfile
import unittest
import zipfile


SCRIPT = Path(__file__).with_name("release.py")
SPEC = importlib.util.spec_from_file_location("release", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.binary = self.root / "fixture-binary"
        self.binary.write_bytes(b"explicit synthetic packaging fixture\x00\xff")
        self.output = self.root / "dist"

    def package_all(self):
        for target in release.TARGETS:
            release.package(self.binary, target, self.output)

    def test_workflow_matrix_matches_published_targets(self):
        workflow = SCRIPT.parents[1] / ".github" / "workflows" / "release.yml"
        targets = re.findall(r"^\s+target: (\S+)$", workflow.read_text(), re.MULTILINE)
        self.assertCountEqual(targets, release.TARGETS)

    def test_linux_runtime_accepts_supported_baseline(self):
        maximum, libraries = release.validate_linux_runtime(
            "Name: GLIBC_2.2.5\nName: GLIBC_2.35\n",
            "Shared library: [libc.so.6]\nShared library: [libgcc_s.so.1]",
        )
        self.assertEqual(maximum, (2, 35, 0))
        self.assertEqual(libraries, ["libc.so.6", "libgcc_s.so.1"])

    def test_linux_runtime_rejects_newer_glibc(self):
        for version in ("2.36", "3.0"):
            with self.subTest(version=version), self.assertRaisesRegex(ValueError, "baseline"):
                release.validate_linux_runtime(f"Name: GLIBC_{version}", "Shared library: [libc.so.6]")

    def test_linux_runtime_rejects_missing_glibc_evidence(self):
        with self.assertRaisesRegex(ValueError, "missing versioned"):
            release.validate_linux_runtime("not an ELF version report", "Shared library: [libc.so.6]")

    def test_linux_runtime_rejects_private_or_unknown_abi(self):
        with self.assertRaisesRegex(ValueError, "unsupported glibc ABI"):
            release.validate_linux_runtime("Name: GLIBC_PRIVATE", "Shared library: [libc.so.6]")

    def test_linux_runtime_rejects_undeclared_shared_libraries(self):
        with self.assertRaisesRegex(ValueError, "unexpected Linux runtime"):
            release.validate_linux_runtime("Name: GLIBC_2.35", "Shared library: [libssl.so.3]")

    def test_all_archives_and_checksums_verify(self):
        self.package_all()
        release.verify(self.output)
        for target, name in release.TARGETS.items():
            archive = self.output / release.archive_name(target)
            self.assertEqual(
                (self.output / (archive.name + ".sha256")).read_text(),
                f"{release.digest(archive)}  {archive.name}\n",
            )
            if archive.suffix == ".zip":
                with zipfile.ZipFile(archive) as bundle:
                    self.assertEqual(bundle.namelist(), [name])
                    self.assertEqual(bundle.read(name), self.binary.read_bytes())
            else:
                with tarfile.open(archive) as bundle:
                    self.assertEqual(bundle.getnames(), [name])
                    self.assertEqual(bundle.getmember(name).mode, 0o755)
                    stream = bundle.extractfile(name)
                    self.assertIsNotNone(stream)
                    assert stream is not None
                    with stream:
                        self.assertEqual(stream.read(), self.binary.read_bytes())

    def test_rejects_missing_and_empty_binary(self):
        with self.assertRaisesRegex(ValueError, "nonempty regular file"):
            release.package(self.root / "missing", next(iter(release.TARGETS)), self.output)
        self.binary.write_bytes(b"")
        with self.assertRaisesRegex(ValueError, "nonempty regular file"):
            release.package(self.binary, next(iter(release.TARGETS)), self.output)

    def test_rejects_directory_instead_of_binary(self):
        with self.assertRaisesRegex(ValueError, "regular file"):
            release.package(self.root, next(iter(release.TARGETS)), self.output)

    def test_refuses_to_overwrite_archives(self):
        target = next(iter(release.TARGETS))
        archive = release.package(self.binary, target, self.output)
        before = archive.read_bytes()
        with self.assertRaisesRegex(ValueError, "overwrite"):
            release.package(self.binary, target, self.output)
        self.assertEqual(archive.read_bytes(), before)

    def test_missing_platform_blocks_publication(self):
        self.package_all()
        (self.output / release.archive_name(next(iter(release.TARGETS)))).unlink()
        with self.assertRaisesRegex(ValueError, "missing="):
            release.verify(self.output)

    def test_unexpected_file_blocks_publication(self):
        self.package_all()
        (self.output / "unexpected.txt").write_text("not a release asset")
        with self.assertRaisesRegex(ValueError, "extra="):
            release.verify(self.output)

    def test_tampering_blocks_publication(self):
        self.package_all()
        archive = self.output / release.archive_name(next(iter(release.TARGETS)))
        archive.write_bytes(archive.read_bytes() + b"tampered")
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            release.verify(self.output)

    def test_malformed_checksum_blocks_publication(self):
        self.package_all()
        checksum = next(self.output.glob("*.sha256"))
        checksum.write_text("not a checksum\n")
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            release.verify(self.output)

    def test_windows_archive_cannot_contain_unexpected_paths(self):
        self.package_all()
        archive = self.output / release.archive_name("x86_64-pc-windows-msvc")
        with zipfile.ZipFile(archive, "w") as bundle:
            bundle.writestr("nested/katara-cli.exe", self.binary.read_bytes())
        (self.output / (archive.name + ".sha256")).write_text(
            f"{release.digest(archive)}  {archive.name}\n"
        )
        with self.assertRaisesRegex(ValueError, "ZIP contents"):
            release.verify(self.output)


if __name__ == "__main__":
    unittest.main()
