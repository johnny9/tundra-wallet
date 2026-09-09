#!/usr/bin/env python3
"""Inventory both locked Rust graphs, retained notices and documented supplements.

This reports declarations, not a legal compatibility decision or a complete mobile
binary notice bundle. Native dependencies and nested vendored code need separate review.
"""
import argparse
import hashlib
import json
import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "third-party/cargo-inventory.json"
NOTICES = ROOT / "third-party/cargo-notices"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def inventory(fetch=False):
    packages, notices, locks = {}, {}, {}
    supplements = json.loads((ROOT / "third-party/cargo-notice-supplements.json").read_text())
    for manifest, lock in [("Cargo.toml", "Cargo.lock"), ("fuzz/Cargo.toml", "fuzz/Cargo.lock")]:
        lock_data = (ROOT / lock).read_bytes()
        locks[lock] = digest(lock_data)
        locked = {(p["name"], p["version"], p.get("source")): p for p in tomllib.loads(lock_data.decode())["package"]}
        result = subprocess.run(["cargo", "metadata", "--locked", "--all-features", *([] if fetch else ["--offline"]), "--format-version", "1", "--manifest-path", str(ROOT / manifest)],
                                check=True, stdout=subprocess.PIPE, cwd=ROOT)
        metadata = json.loads(result.stdout)
        for package in metadata["packages"]:
            if package["source"] is None:
                continue  # Original workspace source is not a third-party license grant.
            identity = (package["name"], package["version"], package["source"])
            entry = packages.get(identity)
            if entry is not None:
                entry["graphs"].append(lock)
                continue
            record = locked[identity]
            directory = Path(package["manifest_path"]).parent
            candidates = set()
            for path in directory.iterdir():
                if path.name.upper().startswith(("LICENSE", "COPYING", "COPYRIGHT", "NOTICE", "UNLICENSE", "AUTHORS")):
                    if path.is_file():
                        candidates.add(path)
                    elif path.is_dir() and not path.is_symlink():
                        candidates.update(p for p in path.rglob("*") if p.is_file())
            if package.get("license_file"):
                explicit = (directory / package["license_file"]).resolve()
                # bhwi distributes ../LICENSE from its pinned repository root.
                if not explicit.is_relative_to(directory.parent.resolve()):
                    raise ValueError("License file escapes the package's enclosing directory")
                candidates.add(explicit)
            files = []
            for path in sorted(candidates):
                if not path.resolve().is_relative_to(directory.parent.resolve()):
                    raise ValueError("Notice symlink escapes the package's enclosing directory")
                data = path.read_bytes()
                if len(data) > 1_048_576:
                    raise ValueError("Notice exceeds the review limit")
                fingerprint = digest(data)
                notices[fingerprint + ".txt"] = data  # Preserve exact distributed bytes.
                files.append({"name": path.name, "sha256": fingerprint, "bytes": len(data),
                              "retained": "cargo-notices/" + fingerprint + ".txt"})
            for group in supplements["groups"]:
                if not any((p["name"], p["version"], p["source"]) == identity for p in group["packages"]):
                    continue
                expected = next(p for p in group["packages"] if (p["name"], p["version"], p["source"]) == identity)
                if expected["checksum"] != record.get("checksum"):
                    raise ValueError("Supplement package checksum changed")
                for notice in group["notices"]:
                    name = notice["sha256"] + ".txt"
                    if notice["retained"] != "cargo-notices/" + name or len(notice["sha256"]) != 64 or any(c not in "0123456789abcdef" for c in notice["sha256"]):
                        raise ValueError("Invalid retained notice path")
                    data = (NOTICES / name).read_bytes()
                    if digest(data) != notice["sha256"] or len(data) != notice["bytes"]:
                        raise ValueError("Upstream notice bytes changed")
                    notices[name] = data
                    files.append({**notice, "basis": group["basis"]})
            entry = {"name": identity[0], "version": identity[1], "source": identity[2],
                     "checksum": record.get("checksum"), "declared_license": package.get("license"),
                     "declared_license_file": package.get("license_file"), "repository": package.get("repository"),
                     "authors": package["authors"], "graphs": [lock], "notices": files}
            packages[identity] = entry
    return {"format": 1, "scope": "Both Cargo lock graphs, all resolved platforms; includes build/test dependencies. Not an assertion that every package ships in each binary.",
            "limits": "Declared licenses, distributed notices/AUTHORS/license directories and documented upstream supplements. Native/SDK/action/design dependencies and other nested vendored code require separate review. No license is selected for original Tundra source.",
            "lock_sha256": locks, "packages": [packages[key] for key in sorted(packages)]}, notices


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="regenerate the reviewable inventory and exact notice copies")
    parser.add_argument("--fetch", action="store_true", help="allow Cargo to fetch missing locked package manifests")
    args = parser.parse_args()
    value, notices = inventory(args.fetch)
    encoded = (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()
    if args.write:
        NOTICES.mkdir(parents=True, exist_ok=True)
        for name, data in notices.items():
            (NOTICES / name).write_bytes(data)
        OUTPUT.write_bytes(encoded)
    else:
        if OUTPUT.read_bytes() != encoded:
            raise SystemExit("Cargo inventory changed; regenerate and review before committing.")
        for name, data in notices.items():
            if (NOTICES / name).read_bytes() != data:
                raise SystemExit("Retained Cargo notice does not match the locked package.")
    missing = [p["name"] + "@" + p["version"] for p in value["packages"] if not p["notices"]]
    print(f"{len(value['packages'])} packages across both locks; {len(notices)} distinct retained notice files")
    print("Packages without a retained notice or declared-license supplement: " + (", ".join(missing) or "none"))


if __name__ == "__main__":
    main()
