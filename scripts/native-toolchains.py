#!/usr/bin/env python3
"""Check reviewed native tool archives/notices, with optional installed SDK comparison.

Default is offline notice validation. --fetch permits exact archive downloads.
--sdk compares every archived SDK payload file with its installed counterpart.
--install-xcodegen installs the reviewed tool in this repository's ignored build directory.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import shutil
import stat
import subprocess
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
CACHE = ROOT / "build/toolchain-review"


def fingerprint(stream):
    result = hashlib.sha256()
    while data := stream.read(1024 * 1024):
        result.update(data)
    return result.hexdigest()


def archive(row, fetch):
    target = CACHE / row["url"].rsplit("/", 1)[1]
    if not target.exists():
        if not fetch:
            raise ValueError("Reviewed tool archive unavailable; use --fetch explicitly")
        CACHE.mkdir(parents=True, exist_ok=True)
        temporary = target.with_suffix(".download")
        with urllib.request.urlopen(row["url"], timeout=45) as response, temporary.open("wb") as output:
            copied = 0
            while data := response.read(1024 * 1024):
                copied += len(data)
                if copied > row["bytes"]:
                    raise ValueError("Tool archive exceeds reviewed size")
                output.write(data)
        temporary.replace(target)
    with target.open("rb") as stream:
        if target.stat().st_size != row["bytes"] or fingerprint(stream) != row["sha256"]:
            raise ValueError("Tool archive differs from the reviewed bytes")
    return target


def members(zipped, row):
    seen = set()
    for item in zipped.infolist():
        path = PurePosixPath(item.filename)
        if path.is_absolute() or ".." in path.parts or not path.parts or path.parts[0] not in row["roots"]:
            raise ValueError("Unsafe or unexpected tool archive path")
        if item.is_dir():
            continue
        if item.filename in seen or len(path.parts) < 2:
            raise ValueError("Duplicate or unrooted tool payload")
        seen.add(item.filename)
        yield item, Path(*path.parts[1:])


def verify_sdk(sdk, rows, fetch):
    if platform.system() != "Linux":
        raise ValueError("The reviewed Android archives target Linux")
    for row in rows:
        coordinate = row["package"]
        if coordinate not in {"platforms;android-36", "build-tools;35.0.0", "ndk;27.2.12479018"}:
            continue
        destination = sdk.joinpath(*coordinate.split(";"))
        checked = 0
        with zipfile.ZipFile(archive(row, fetch)) as zipped:
            for item, relative in members(zipped, row):
                installed = destination / relative
                mode = item.external_attr >> 16
                if stat.S_ISLNK(mode):
                    if not installed.is_symlink() or os.readlink(installed).encode() != zipped.read(item):
                        raise ValueError(f"Installed SDK link differs: {coordinate}/{relative}")
                else:
                    if installed.is_symlink() or not installed.is_file() or installed.stat().st_size != item.file_size:
                        raise ValueError(f"Installed SDK file differs: {coordinate}/{relative}")
                    with installed.open("rb") as actual, zipped.open(item) as expected:
                        if fingerprint(actual) != fingerprint(expected):
                            raise ValueError(f"Installed SDK content differs: {coordinate}/{relative}")
                checked += 1
        print(f"{coordinate}: {checked} installed archive payload files match")
    # sdkmanager-generated package.xml and unrelated installed packages are not
    # archive payloads and are deliberately outside this byte-comparison scope.


def install_xcodegen(rows, fetch):
    if platform.system() != "Darwin":
        raise ValueError("XcodeGen execution requires the Apple host")
    row = next(r for r in rows if r["package"] == "XcodeGen:2.46.0")
    destination = ROOT / "build/tools/xcodegen-2.46.0"
    with zipfile.ZipFile(archive(row, fetch)) as zipped:
        for item, relative in members(zipped, row):
            if stat.S_ISLNK(item.external_attr >> 16):
                raise ValueError("Unexpected XcodeGen archive symlink")
            target = destination / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            with zipped.open(item) as source, target.open("wb") as output:
                shutil.copyfileobj(source, output)
    binary = destination / "bin/xcodegen"
    binary.chmod(0o755)
    result = subprocess.run([str(binary), "--version"], check=True, capture_output=True, text=True)
    if result.stdout.strip() != "Version: 2.46.0":
        raise ValueError("Unexpected XcodeGen executable version")
    print(result.stdout.strip())
    if "GITHUB_PATH" in os.environ:
        with open(os.environ["GITHUB_PATH"], "a") as output:
            output.write(str(binary.parent) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fetch", action="store_true")
    parser.add_argument("--archives", action="store_true")
    parser.add_argument("--sdk", type=Path)
    parser.add_argument("--install-xcodegen", action="store_true")
    args = parser.parse_args()
    inventory = json.loads((ROOT / "third-party/toolchain-inventory.json").read_text())
    rows = inventory["archives"]
    for row in rows:
        allowed = row["url"].startswith("https://dl.google.com/android/repository/") or row["url"] in {
            "https://github.com/yonaskolb/XcodeGen/releases/download/2.46.0/xcodegen.zip",
            "https://dl.google.com/dl/android/maven2/com/android/tools/build/aapt2/8.13.2-14304508/aapt2-8.13.2-14304508-linux.jar"}
        if not allowed:
            raise ValueError("Tool archive is outside the reviewed repositories")
        for notice in row["notices"]:
            if notice["path"] != f'third-party/toolchain-notices/{notice["sha256"]}.txt':
                raise ValueError("Unexpected toolchain notice path")
            data = (ROOT / notice["path"]).read_bytes()
            if len(data) != notice["bytes"] or hashlib.sha256(data).hexdigest() != notice["sha256"]:
                raise ValueError("Retained toolchain notice changed")
        if args.archives:
            with zipfile.ZipFile(archive(row, args.fetch)) as zipped:
                for notice in row["notices"]:
                    if zipped.read(notice["member"]) != (ROOT / notice["path"]).read_bytes():
                        raise ValueError("Retained notice differs from verified tool archive")
    if args.sdk:
        verify_sdk(args.sdk, rows, args.fetch)
    if args.install_xcodegen:
        install_xcodegen(rows, args.fetch)
    print(f"{len(rows)} tool archive records and their retained notices checked")


if __name__ == "__main__":
    main()
