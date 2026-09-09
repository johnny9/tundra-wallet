#!/usr/bin/env python3
"""Check packaged native ABIs, 16 KiB ELF/ZIP alignment and basic loader protections.

Uses the reviewed Linux SDK/NDK tools. This is binary inspection, not evidence of
runtime behavior on a 16 KiB device. No wallet data or UI is inspected.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("apk", type=Path)
    args = parser.parse_args()
    sdk, ndk = Path(os.environ["ANDROID_HOME"]), Path(os.environ["ANDROID_NDK_HOME"])
    reader = ndk / "toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-readelf"
    aligned = subprocess.run([str(sdk / "build-tools/35.0.0/zipalign"), "-c", "-P", "16", "4", str(args.apk)],
                             capture_output=True, timeout=60)
    if aligned.returncode:
        raise ValueError("APK ZIP alignment check failed")
    rows = []
    with tempfile.TemporaryDirectory(prefix="apk-native-", dir=ROOT / "build") as temporary:
        with zipfile.ZipFile(args.apk) as apk:
            seen = set()
            for item in apk.infolist():
                if not item.filename.startswith("lib/") or not item.filename.endswith(".so"):
                    continue
                parts = Path(item.filename).parts
                if len(parts) != 3 or parts[1] not in {"arm64-v8a", "x86_64"} or item.filename in seen:
                    raise ValueError("Unexpected or duplicate packaged native library")
                if not re.fullmatch(r"lib[A-Za-z0-9_.-]+\.so", parts[2]) or item.file_size > 64 * 1024 * 1024:
                    raise ValueError("Native archive member exceeds reviewed bounds")
                if item.compress_type != zipfile.ZIP_STORED:
                    raise ValueError("Native library must use the reviewed uncompressed packaging")
                if parts[2].startswith("libbbqr-"):
                    raise ValueError("Unused dependency cdylib must not be packaged")
                seen.add(item.filename)
                data = apk.read(item)
                if data[:6] != b"\x7fELF\x02\x01" or int.from_bytes(data[18:20], "little") != {"arm64-v8a": 183, "x86_64": 62}[parts[1]]:
                    raise ValueError("Native ELF architecture does not match its packaged ABI")
                target = Path(temporary) / (parts[1] + "-" + parts[2])
                target.write_bytes(data)
                headers = subprocess.run([str(reader), "-lW", str(target)], check=True,
                                         capture_output=True, text=True, timeout=15).stdout
                loads = [line.split() for line in headers.splitlines() if line.lstrip().startswith("LOAD ")]
                if not loads or any(int(row[-1], 16) < 16384 for row in loads):
                    raise ValueError("Native LOAD segment is below 16 KiB alignment")
                if any("W" in "".join(row[6:-1]) and "E" in "".join(row[6:-1]) for row in loads):
                    raise ValueError("Native LOAD segment is writable and executable")
                stacks = [line.split() for line in headers.splitlines() if line.lstrip().startswith("GNU_STACK ")]
                if not stacks or any("E" in "".join(row[6:-1]) for row in stacks) or "GNU_RELRO" not in headers:
                    raise ValueError("Native stack/relocation protections are missing")
                dynamic = subprocess.run([str(reader), "-dW", str(target)], check=True,
                                         capture_output=True, text=True, timeout=15).stdout
                needed = re.findall(r"\(NEEDED\).*?\[([^\]]+)\]", dynamic)
                if any("bbqr" in name for name in needed):
                    raise ValueError("Unexpected dynamic dependency on the unused BBQr library")
                rows.append({"member": item.filename, "sha256": hashlib.sha256(data).hexdigest(),
                             "bytes": len(data), "load_alignments": [int(row[-1], 16) for row in loads],
                             "needed": needed, "relro": True, "nonexecutable_stack": True})
            for abi in ("arm64-v8a", "x86_64"):
                if f"lib/{abi}/libtundra_ffi.so" not in seen:
                    raise ValueError("Required Tundra mobile ABI is missing")
    print(json.dumps({"apk_sha256": hashlib.sha256(args.apk.read_bytes()).hexdigest(),
                      "zip_alignment_16k": True, "runtime_16k": "Not tested by this check",
                      "libraries": rows}, indent=2))


if __name__ == "__main__":
    main()
