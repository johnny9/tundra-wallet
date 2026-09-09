#!/usr/bin/env python3
"""Inventory exact Maven POM license declarations for Android's reviewed metadata.

Default: compare the committed inventory using retained POMs, with no network.
--fetch permits official repository downloads; --write retains new POMs/inventory.
Declarations and inherited parent licenses are source evidence, not a legal conclusion
or a complete binary notice bundle. No URLs supplied inside a POM are fetched.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import urllib.error
import urllib.request
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
INVENTORY = ROOT / "third-party/android-inventory.json"
RETAINED = ROOT / "third-party/android-poms"
CACHE = ROOT / "build/android-poms"
MAX_POM = 2 * 1024 * 1024
REPOSITORIES = ("https://dl.google.com/dl/android/maven2/", "https://repo.maven.apache.org/maven2/",
                "https://plugins.gradle.org/m2/")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def document(data):
    if len(data) > MAX_POM or b"<!DOCTYPE" in data.upper() or b"<!ENTITY" in data.upper():
        raise ValueError("Unbounded or unsupported Maven XML")
    root = ET.fromstring(data)
    if root.tag.rsplit("}", 1)[-1] != "project":
        raise ValueError("Expected a Maven project document")
    return root


def text(node, name):
    return (node.findtext("{*}" + name) or "").strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fetch", action="store_true")
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    spec = importlib.util.spec_from_file_location("android_dependencies", ROOT / "scripts/check-android-dependencies.py")
    checker = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(checker)
    counts = checker.check(ROOT / "apps/android")
    old = json.loads(INVENTORY.read_text()) if INVENTORY.exists() else {"poms": []}
    previous = {item["coordinate"]: item for item in old["poms"]}
    metadata = ET.parse(ROOT / "apps/android/gradle/verification-metadata.xml")
    coordinates, checksums = set(), {}
    for component in metadata.findall(".//" + checker.NS + "component"):
        group, name, version = (component.attrib[k] for k in ("group", "name", "version"))
        coordinate = ":".join((group, name, version))
        coordinates.add(coordinate)
        artifact = component.find(checker.NS + f'artifact[@name="{name}-{version}.pom"]')
        if artifact is not None:
            checksums[coordinate] = artifact[0].attrib["value"]

    def load(coordinate):
        group, name, version = coordinate.split(":")
        if not all(re.fullmatch(r"[A-Za-z0-9_.-]+", part) for part in (group, name)) or not checker.exact_version(version):
            raise ValueError("Invalid or mutable parent coordinate")
        retained = previous.get(coordinate)
        cache = CACHE / (digest(coordinate.encode()) + ".json")
        if retained:
            data = (ROOT / retained["path"]).read_bytes()
            url = retained["url"]
            if digest(data) != retained["sha256"]:
                raise ValueError("Retained Maven metadata changed")
        elif cache.exists():
            saved = json.loads(cache.read_text()); url = saved["url"]
            data = (CACHE / (saved["sha256"] + ".xml")).read_bytes()
            if digest(data) != saved["sha256"]:
                raise ValueError("Cached Maven metadata changed")
        else:
            if not args.fetch:
                raise ValueError(f"Maven metadata unavailable offline: {coordinate}")
            relative = f'{group.replace(".", "/")}/{name}/{version}/{name}-{version}.pom'
            # Prefer the same official repository family used by these coordinates.
            repositories = REPOSITORIES if group.startswith(("androidx.", "com.android", "com.google.testing")) else (REPOSITORIES[1], REPOSITORIES[0], REPOSITORIES[2])
            for repository in repositories:
                url = repository + relative
                try:
                    with urllib.request.urlopen(url, timeout=20) as response:
                        data = response.read(MAX_POM + 1)
                    break
                except urllib.error.HTTPError as error:
                    if error.code not in (403, 404):
                        raise
            else:
                raise ValueError(f"Official Maven POM not available: {coordinate}")
            document(data)
            CACHE.mkdir(parents=True, exist_ok=True)
            (CACHE / (digest(data) + ".xml")).write_bytes(data)
            cache.write_text(json.dumps({"url": url, "sha256": digest(data)}))
        sha = digest(data)
        if coordinate in checksums and sha != checksums[coordinate]:
            raise ValueError(f"Maven POM differs from Gradle verification metadata: {coordinate}")
        pom = document(data)
        if text(pom, "artifactId") != name:
            raise ValueError("Maven artifact identity mismatch")
        parent = pom.find("{*}parent")
        parent_id = ":".join(text(parent, k) for k in ("groupId", "artifactId", "version")) if parent is not None else None
        actual_group = text(pom, "groupId") or (text(parent, "groupId") if parent is not None else "")
        actual_version = text(pom, "version") or (text(parent, "version") if parent is not None else "")
        if actual_group != group or actual_version != version:
            raise ValueError(f"Maven project identity mismatch: {coordinate}")
        licenses = [{key: text(license, key) for key in ("name", "url", "distribution", "comments") if text(license, key)}
                    for license in pom.findall("{*}licenses/{*}license")]
        path = RETAINED / (sha + ".xml")
        if args.write:
            path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(data)
        elif not path.exists() or path.read_bytes() != data:
            raise ValueError("Maven metadata has not been retained for review")
        return {"coordinate": coordinate, "url": url, "sha256": sha, "path": str(path.relative_to(ROOT)),
                "gradle_checksum_match": coordinate in checksums, "parent": parent_id,
                "declared_licenses": licenses}

    poms = {}
    pending = coordinates
    while pending:
        with ThreadPoolExecutor(max_workers=8) as executor:
            for item in executor.map(load, sorted(pending)):
                poms[item["coordinate"]] = item
        pending = {item["parent"] for item in poms.values() if item["parent"] and item["parent"] not in poms}
    for coordinate, item in poms.items():
        visited = {coordinate}; source = coordinate
        while not poms[source]["declared_licenses"] and poms[source]["parent"]:
            source = poms[source]["parent"]
            if source in visited:
                raise ValueError("Cyclic Maven license inheritance")
            visited.add(source)
        item["license_source"] = source if poms[source]["declared_licenses"] else None
    result = {"scope": "All reviewed Gradle verification components plus exact Maven parents; declared licenses only",
              "limits": "Publisher authentication, full artifact notices and binary distribution review remain separate gates",
              "gradle": counts, "components": sorted(coordinates), "poms": [poms[key] for key in sorted(poms)]}
    encoded = json.dumps(result, indent=2, ensure_ascii=False) + "\n"
    if args.write:
        INVENTORY.write_text(encoded)
    elif not INVENTORY.exists() or INVENTORY.read_text() != encoded:
        raise ValueError("Android Maven inventory differs; review an explicit --write update")
    missing = sum(poms[key]["license_source"] is None for key in coordinates)
    print(f"{len(coordinates)} verification components, {len(poms)} retained POMs, {missing} without an effective license declaration")


if __name__ == "__main__":
    main()
