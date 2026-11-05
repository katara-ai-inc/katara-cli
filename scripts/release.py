#!/usr/bin/env python3
"""Package native CLI builds and verify a complete GitHub Release asset set."""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import re
import subprocess
import tarfile
import zipfile


TARGETS = {
    "x86_64-unknown-linux-gnu": "katara-cli",
    "aarch64-unknown-linux-gnu": "katara-cli",
    "x86_64-apple-darwin": "katara-cli",
    "aarch64-apple-darwin": "katara-cli",
    "x86_64-pc-windows-msvc": "katara-cli.exe",
}

LINUX_GLIBC_BASELINE = (2, 35, 0)
LINUX_RUNTIME_LIBRARIES = {
    "libc.so.6", "libdl.so.2", "libgcc_s.so.1", "libm.so.6",
    "libpthread.so.0", "librt.so.1", "ld-linux-x86-64.so.2", "ld-linux-aarch64.so.1",
}


def validate_linux_runtime(versions: str, dynamic: str) -> tuple[tuple[int, ...], list[str]]:
    names = re.findall(r"Name: (GLIBC_[^\s]+)", versions)
    if not names:
        raise ValueError("missing versioned glibc runtime requirements")
    requirements = []
    for name in names:
        match = re.fullmatch(r"GLIBC_(\d+)\.(\d+)(?:\.(\d+))?", name)
        if match is None:
            raise ValueError(f"unsupported glibc ABI requirement: {name}")
        requirements.append(tuple(int(part or 0) for part in match.groups()))
    maximum = max(requirements)
    if maximum > LINUX_GLIBC_BASELINE:
        raise ValueError(f"binary requires glibc {maximum}; supported baseline is 2.35")
    libraries = re.findall(r"Shared library: \[([^]]+)\]", dynamic)
    if not libraries or set(libraries) - LINUX_RUNTIME_LIBRARIES:
        raise ValueError(f"unexpected Linux runtime libraries: {libraries}")
    return maximum, libraries


def check_linux(binary: Path) -> None:
    versions = subprocess.check_output(["readelf", "--version-info", "--wide", str(binary)], text=True)
    dynamic = subprocess.check_output(["readelf", "--dynamic", "--wide", str(binary)], text=True)
    maximum, libraries = validate_linux_runtime(versions, dynamic)
    print(f"Linux runtime check passed: max glibc requirement={maximum}, libraries={libraries}")


def archive_name(target: str) -> str:
    binary = TARGETS[target]
    suffix = ".zip" if binary.endswith(".exe") else ".tar.gz"
    return f"katara-cli-{target}{suffix}"


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def package(binary: Path, target: str, output_dir: Path) -> Path:
    if binary.is_symlink() or not binary.is_file() or binary.stat().st_size == 0:
        raise ValueError("binary must be a nonempty regular file, not a symlink")
    name = TARGETS[target]
    output_dir.mkdir(parents=True, exist_ok=True)
    archive = output_dir / archive_name(target)
    if archive.exists() or archive.is_symlink():
        raise ValueError(f"refusing to overwrite existing archive: {archive.name}")
    checksum = output_dir / (archive.name + ".sha256")
    if checksum.exists() or checksum.is_symlink():
        raise ValueError(f"refusing to overwrite existing checksum: {checksum.name}")
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_DEFLATED) as bundle:
            bundle.write(binary, arcname=name)
    else:
        with tarfile.open(archive, "x:gz") as bundle:
            info = bundle.gettarinfo(str(binary), arcname=name)
            info.mode = 0o755
            info.uid = info.gid = 0
            info.uname = info.gname = ""
            with binary.open("rb") as stream:
                bundle.addfile(info, stream)
    checksum.write_text(f"{digest(archive)}  {archive.name}\n", encoding="ascii", newline="\n")
    return archive


def verify(directory: Path) -> None:
    expected = {
        name
        for target in TARGETS
        for name in (archive_name(target), archive_name(target) + ".sha256")
    }
    actual = {path.name for path in directory.iterdir()}
    if actual != expected:
        raise ValueError(
            f"incomplete or unexpected release assets; missing={sorted(expected - actual)}, "
            f"extra={sorted(actual - expected)}"
        )
    for target, binary in TARGETS.items():
        archive = directory / archive_name(target)
        checksum = directory / (archive.name + ".sha256")
        for path in (archive, checksum):
            if path.is_symlink() or not path.is_file():
                raise ValueError(f"release asset must be a regular file: {path.name}")
        expected_checksum = f"{digest(archive)}  {archive.name}\n"
        if checksum.read_text(encoding="ascii") != expected_checksum:
            raise ValueError(f"checksum mismatch: {archive.name}")
        if archive.suffix == ".zip":
            with zipfile.ZipFile(archive) as bundle:
                entries = bundle.infolist()
                if len(entries) != 1 or entries[0].filename != binary or entries[0].file_size == 0:
                    raise ValueError(f"unexpected ZIP contents: {archive.name}")
                if bundle.testzip() is not None:
                    raise ValueError(f"corrupt ZIP: {archive.name}")
        else:
            with tarfile.open(archive, "r:gz") as bundle:
                entries = bundle.getmembers()
                if (
                    len(entries) != 1
                    or entries[0].name != binary
                    or not entries[0].isfile()
                    or entries[0].size == 0
                    or entries[0].mode & 0o111 == 0
                ):
                    raise ValueError(f"unexpected tar contents: {archive.name}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    build = commands.add_parser("package")
    build.add_argument("--binary", type=Path, required=True)
    build.add_argument("--target", choices=TARGETS, required=True)
    build.add_argument("--output-dir", type=Path, required=True)
    check = commands.add_parser("verify")
    check.add_argument("--directory", type=Path, required=True)
    linux = commands.add_parser("check-linux")
    linux.add_argument("--binary", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "package":
            print(package(args.binary, args.target, args.output_dir))
        elif args.command == "check-linux":
            check_linux(args.binary)
        else:
            verify(args.directory)
            print(f"Verified {len(TARGETS)} platform archives and their SHA-256 checksums.")
    except (OSError, ValueError, tarfile.TarError, zipfile.BadZipFile, subprocess.SubprocessError) as error:
        parser.exit(1, f"release packaging failed: {error}\n")


if __name__ == "__main__":
    main()
