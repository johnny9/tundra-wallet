#!/usr/bin/env python3
"""Retain notices from verified JAR/AAR variants of Android release-graph components.

Default checks the committed graph, archive hashes and retained notice bytes offline.
--collect --fetch --write reviews actual archives from their official Maven repositories.
This includes all known binary variants for these components, not an APK contents claim.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import io
import json
from pathlib import Path
import re
import urllib.error
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "third-party/android-artifact-inventory.json"
NOTICES = ROOT / "third-party/android-notices"
CACHE = ROOT / "build/android-artifacts"
MODULES = ROOT / "third-party/android-modules"
NS = "{https://schema.gradle.org/dependency-verification}"
MAX_ARCHIVE = 256 * 1024 * 1024
MAX_NOTICE = 2 * 1024 * 1024


def sha(data):
    return hashlib.sha256(data).hexdigest()


def graph(fetch=False, write=False):
    components = set()
    for row in (ROOT / "apps/android/app/gradle.lockfile").read_text().splitlines():
        if "=" in row and "releaseRuntimeClasspath" in row.split("=", 1)[1].split(","):
            components.add(row.split("=", 1)[0])
    poms = {p["coordinate"]: p for p in json.loads((ROOT / "third-party/android-inventory.json").read_text())["poms"]}
    archives = []
    tree = ET.parse(ROOT / "apps/android/gradle/verification-metadata.xml")
    for component in tree.findall(".//" + NS + "component"):
        coordinate = ":".join(component.attrib[k] for k in ("group", "name", "version"))
        if coordinate not in components:
            continue
        base = poms[coordinate]["url"].rsplit("/", 1)[0] + "/"
        # Gradle module metadata can give an artifact a logical name different
        # from its repository filename (for example annotation-metadata-*.jar).
        locations = {}
        module = component.find(NS + f'artifact[@name="{component.attrib["name"]}-{component.attrib["version"]}.module"]')
        if module is not None:
            checksum = module.find(NS + "sha256").attrib["value"]
            retained = MODULES / (checksum + ".json")
            cached = CACHE / (checksum + ".module")
            if retained.exists():
                data = retained.read_bytes()
            elif cached.exists():
                data = cached.read_bytes()
            else:
                if not fetch:
                    raise ValueError("Verified module metadata unavailable offline")
                with urllib.request.urlopen(base + module.attrib["name"], timeout=30) as response:
                    data = response.read(MAX_NOTICE + 1)
            if len(data) > MAX_NOTICE or sha(data) != checksum:
                raise ValueError("Gradle module metadata checksum mismatch")
            if not retained.exists():
                CACHE.mkdir(parents=True, exist_ok=True); cached.write_bytes(data)
            if write:
                MODULES.mkdir(parents=True, exist_ok=True); retained.write_bytes(data)
            for variant in json.loads(data)["variants"]:
                for file in variant.get("files", []):
                    url = urllib.parse.urljoin(base, file["url"])
                    parsed = urllib.parse.urlsplit(url)
                    if parsed.scheme != "https" or parsed.hostname not in ("dl.google.com", "repo.maven.apache.org", "plugins.gradle.org"):
                        raise ValueError("Gradle module archive URL is outside the configured repositories")
                    if file["name"] in locations and locations[file["name"]] != url:
                        raise ValueError("Ambiguous module archive URL")
                    locations[file["name"]] = url
        for artifact in component.findall(NS + "artifact"):
            name = artifact.attrib["name"]
            if name.endswith((".jar", ".aar")):
                archives.append({"coordinate": coordinate, "name": name,
                                 "sha256": artifact.find(NS + "sha256").attrib["value"],
                                 "url": locations.get(name, base + name)})
    return sorted(components), sorted(archives, key=lambda a: (a["coordinate"], a["name"]))


def check(inventory, components, archives):
    if inventory["components"] != components:
        raise ValueError("Release dependency graph changed")
    actual = [{key: row[key] for key in ("coordinate", "name", "sha256", "url")} for row in inventory["archives"]]
    if actual != archives:
        raise ValueError("Reviewed archive set changed")
    for archive in inventory["archives"]:
        for notice in archive["notices"]:
            expected = f'third-party/android-notices/{notice["sha256"]}.txt'
            if notice["path"] != expected or not re.fullmatch(r"[a-f0-9]{64}", notice["sha256"]):
                raise ValueError("Invalid retained notice path")
            data = (ROOT / expected).read_bytes()
            if len(data) > MAX_NOTICE or sha(data) != notice["sha256"]:
                raise ValueError("Retained archive notice changed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--collect", action="store_true")
    parser.add_argument("--fetch", action="store_true")
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    if (args.fetch or args.write) and not args.collect:
        raise ValueError("Fetching/writing requires explicit collection")
    components, archives = graph(args.fetch, args.write)
    if args.collect:
        def collect(archive):
            path = CACHE / (archive["sha256"] + ".zip")
            if path.exists():
                data = path.read_bytes()
            else:
                if not args.fetch:
                    raise ValueError("Archive unavailable offline; explicit --fetch is required")
                try:
                    with urllib.request.urlopen(archive["url"], timeout=30) as response:
                        data = response.read(MAX_ARCHIVE + 1)
                except urllib.error.HTTPError as error:
                    raise ValueError(f'Official archive download failed ({error.code}): {archive["coordinate"]} / {archive["name"]}') from error
                if len(data) > MAX_ARCHIVE or sha(data) != archive["sha256"]:
                    raise ValueError("Downloaded archive failed size/checksum verification")
                CACHE.mkdir(parents=True, exist_ok=True); path.write_bytes(data)
            if len(data) > MAX_ARCHIVE or sha(data) != archive["sha256"]:
                raise ValueError("Cached archive failed size/checksum verification")
            notices = []
            contents = {"jvm_classes": 0, "native_libraries": 0}
            def extract(payload, prefix="", nested=False):
                with zipfile.ZipFile(io.BytesIO(payload)) as bundle:
                    for member in bundle.infolist():
                        if member.filename.endswith(".class"): contents["jvm_classes"] += 1
                        if member.filename.endswith((".so", ".dll", ".dylib", ".jnilib")): contents["native_libraries"] += 1
                        name = member.filename.rsplit("/", 1)[-1]
                        if re.fullmatch(r"(?:licen[sc]e|notice|copying|copyright|authors)(?:[._-].*)?", name, re.I):
                            if member.is_dir():
                                continue
                            if member.file_size > MAX_NOTICE:
                                raise ValueError("Archive notice exceeds size bound")
                            content = bundle.read(member); checksum = sha(content)
                            destination = NOTICES / (checksum + ".txt")
                            if args.write:
                                NOTICES.mkdir(parents=True, exist_ok=True); destination.write_bytes(content)
                            notices.append({"member": prefix + member.filename, "sha256": checksum,
                                            "path": str(destination.relative_to(ROOT))})
                        elif not nested and archive["name"].endswith(".aar") and member.filename.endswith(".jar"):
                            if member.file_size > 64 * 1024 * 1024:
                                raise ValueError("Nested AAR archive exceeds size bound")
                            extract(bundle.read(member), member.filename + "!/", True)
            extract(data)
            return dict(archive, contents=contents, notices=sorted(notices, key=lambda n: n["member"]))
        with ThreadPoolExecutor(max_workers=8) as executor:
            records = list(executor.map(collect, archives))
        inventory = {"scope": "Known JAR/AAR variants for locked releaseRuntimeClasspath components; not an APK contents claim",
                     "limits": "Missing embedded notices require upstream/license review; SDKs and binary notice packaging are separate",
                     "components": components, "archives": records}
        encoded = json.dumps(inventory, indent=2) + "\n"
        if args.write:
            OUTPUT.write_text(encoded)
        elif not OUTPUT.exists() or OUTPUT.read_text() != encoded:
            raise ValueError("Archive notice inventory differs; review an explicit --write update")
    else:
        inventory = json.loads(OUTPUT.read_text())
    check(inventory, components, archives)
    texts = {n["sha256"] for a in inventory["archives"] for n in a["notices"]}
    missing = sum(not a["notices"] for a in inventory["archives"])
    print(f"{len(components)} release-graph components, {len(archives)} verified archive variants, {len(texts)} retained notice texts, {missing} archives without embedded notices")


if __name__ == "__main__":
    main()
