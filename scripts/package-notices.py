#!/usr/bin/env python3
"""Build/check the deterministic native notice resource; never fetch or select a license.

--write prepares a reviewable source update. --android-apk and --ios-app additionally
check exact bytes inside a built app; an inventory check alone is not packaging evidence.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import zipfile

ROOT = Path(__file__).resolve().parents[1]
RESOURCE = ROOT / "third-party/bundle/THIRD-PARTY-NOTICES.txt"
MANIFEST = ROOT / "third-party/bundle-manifest.json"
MAX_NOTICE = 2 * 1024 * 1024


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read_json(path):
    return json.loads((ROOT / path).read_text())


def build():
    inputs = ["third-party/cargo-inventory.json", "third-party/android-inventory.json",
              "third-party/android-artifact-inventory.json", "third-party/android-notice-supplements.json"]
    cargo, maven, android, supplements = map(read_json, inputs)
    texts, sources = {}, {}

    def retain(path, checksum=None):
        file = ROOT / path
        if file.resolve().is_relative_to(ROOT) is False or not file.is_file():
            raise ValueError("Notice source must be a repository file")
        data = file.read_bytes()
        if len(data) > MAX_NOTICE or (checksum is not None and sha(data) != checksum):
            raise ValueError("Notice source exceeds bounds or changed checksum")
        fingerprint = sha(data)
        texts[fingerprint] = data
        sources[path] = fingerprint
        return fingerprint

    packages = []
    for package in cargo["packages"]:
        notices = [{"name": notice["name"], "sha256": retain("third-party/" + notice["retained"], notice["sha256"])}
                   for notice in package["notices"]]
        packages.append({"ecosystem": "Cargo", **{k: package[k] for k in
                         ("name", "version", "source", "checksum", "repository", "authors", "graphs", "declared_license")},
                         "notices": notices})
    poms = {p["coordinate"]: p for p in maven["poms"]}
    for coordinate in android["components"]:
        pom = poms[coordinate]
        license_source = poms.get(pom["license_source"])
        archives = []
        for archive in android["archives"]:
            if archive["coordinate"] != coordinate:
                continue
            notices = [{"member": n["member"], "sha256": retain(n["path"], n["sha256"])}
                       for n in archive["notices"]]
            archives.append({"name": archive["name"], "sha256": archive["sha256"], "notices": notices})
        packages.append({"ecosystem": "Maven release graph", "coordinate": coordinate, "pom": pom["url"],
                         "pom_sha256": pom["sha256"], "license_source": pom["license_source"],
                         "declared_licenses": license_source["declared_licenses"] if license_source else [],
                         "archives": archives})
    for package in supplements["packages"]:
        if package["coordinate"] not in android["components"] or not re.fullmatch(r"[a-f0-9]{40}", package["revision"]):
            raise ValueError("Supplement is outside the reviewed runtime graph or has a mutable revision")
        for notice in package["notices"]:
            expected = "third-party/android-supplements/" + notice["sha256"] + ".txt"
            if notice["retained"] != expected:
                raise ValueError("Invalid supplemental notice path")
            retain(expected, notice["sha256"])
            data = (ROOT / expected).read_bytes()
            blob = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
            if blob != notice["git_blob"]:
                raise ValueError("Supplemental Git blob changed")
            repository = package["repository"].removeprefix("https://github.com/")
            expected_url = f'https://raw.githubusercontent.com/{repository}/{package["revision"]}/{notice["source_path"]}'
            if notice["url"] != expected_url:
                raise ValueError("Supplemental notice provenance changed")
        if package.get("packaged_license_matches_source"):
            license_hashes = {n["sha256"] for n in package["notices"] if n["source_path"] == "LICENSE"}
            packaged = {n["sha256"] for a in android["archives"] if a["coordinate"] == package["coordinate"] for n in a["notices"]}
            if len(license_hashes) != 1 or not license_hashes <= packaged:
                raise ValueError("Supplemental license no longer matches the verified archive")
    extra = [{"source": path, "sha256": retain(path)} for path in
             ["crates/tundra-sqlcipher/vendor/LICENSE.md", "design/THIRD-PARTY-NOTICES.md"]]
    index = {"scope": "Cargo application/fuzz locks, including build/test and all platforms; Android release graph known archive variants; SQLCipher and design notices",
             "limits": "Not an exact binary contents manifest or a completed distribution review. Missing embedded notices remain explicit in archive entries. SDK/build-tool review is separate. No license for original Tundra source or alternative third-party license is selected here.",
             "packages": packages, "supplements": supplements, "additional_notices": extra}
    output = bytearray(b"Tundra third-party notices\n\nPackage index and provenance\n\n")
    output.extend(json.dumps(index, indent=2, ensure_ascii=False).encode() + b"\n")
    for fingerprint, data in sorted(texts.items()):
        output.extend(f"\n===== BEGIN RETAINED NOTICE SHA256 {fingerprint} =====\n".encode())
        output.extend(data)
        output.extend(f"\n===== END RETAINED NOTICE SHA256 {fingerprint} =====\n".encode())
    manifest = {"format": 1, "resource": str(RESOURCE.relative_to(ROOT)), "sha256": sha(output),
                "bytes": len(output), "cargo_packages": len(cargo["packages"]),
                "android_release_components": len(android["components"]), "distinct_notice_texts": len(texts),
                "inputs": {p: sha((ROOT / p).read_bytes()) for p in inputs}, "notice_sources": dict(sorted(sources.items()))}
    return bytes(output), (json.dumps(manifest, indent=2) + "\n").encode(), manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--android-apk", type=Path)
    parser.add_argument("--ios-app", type=Path)
    args = parser.parse_args()
    output, encoded, manifest = build()
    for path, data in ((RESOURCE, output), (MANIFEST, encoded)):
        if args.write:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        elif not path.is_file() or path.read_bytes() != data:
            raise ValueError("Native notice resource changed; review an explicit --write update")
    if args.android_apk:
        with zipfile.ZipFile(args.android_apk) as apk:
            name = "assets/" + RESOURCE.name
            if apk.namelist().count(name) != 1 or apk.getinfo(name).file_size != len(output) or apk.read(name) != output:
                raise ValueError("Android APK does not contain the exact reviewed notice resource")
        print("Android APK notice resource matches the reviewed bytes")
    if args.ios_app:
        if not args.ios_app.is_dir() or args.ios_app.suffix != ".app" or (args.ios_app / RESOURCE.name).read_bytes() != output:
            raise ValueError("iOS app does not contain the exact reviewed notice resource")
        print("iOS app notice resource matches the reviewed bytes")
    print(f'{manifest["distinct_notice_texts"]} distinct notice texts; {manifest["bytes"]} packaged resource bytes')


if __name__ == "__main__":
    main()
