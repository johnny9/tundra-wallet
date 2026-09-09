#!/usr/bin/env python3
"""Offline structural gate for reviewed Gradle locks and strict artifact checksums.

Gradle verifies the downloaded bytes and resolved graphs during actual native builds.
This check rejects missing metadata coverage, mutable versions and verification bypasses.
It does not authenticate publishers or replace the dependency/notice review.
"""
import argparse
import json
from pathlib import Path
import re
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1] / "apps/android"
LOCKS = ("buildscript-gradle.lockfile", "app/buildscript-gradle.lockfile", "app/gradle.lockfile")
NS = "{https://schema.gradle.org/dependency-verification}"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def exact_version(version):
    return (bool(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", version))
            and "SNAPSHOT" not in version.upper()
            and version.lower() not in ("latest", "release", "latest.release", "latest.integration"))


def check(root):
    tree = ET.parse(root / "gradle/verification-metadata.xml").getroot()
    require(tree.tag == NS + "verification-metadata", "Unexpected verification schema")
    require([x.tag for x in tree] == [NS + "configuration", NS + "components"],
            "Unexpected verification sections")
    configuration = tree.find(NS + "configuration")
    require([(x.tag, x.text) for x in configuration] == [
        (NS + "verify-metadata", "true"), (NS + "verify-signatures", "false")],
        "Checksums must cover metadata with no trusted/ignored artifact exceptions")
    verified = set()
    artifacts = 0
    for component in tree.find(NS + "components"):
        require(component.tag == NS + "component" and set(component.attrib) == {"group", "name", "version"},
                "Unexpected component entry")
        identity = tuple(component.attrib[k] for k in ("group", "name", "version"))
        require(identity not in verified and exact_version(identity[2]), "Duplicate or mutable component")
        require(all(re.fullmatch(r"[A-Za-z0-9_.-]+", item) for item in identity[:2]), "Invalid component name")
        verified.add(identity)
        names = set()
        require(len(component) > 0, "Component has no verified artifacts")
        for artifact in component:
            require(artifact.tag == NS + "artifact" and set(artifact.attrib) == {"name"}, "Unexpected artifact entry")
            name = artifact.attrib["name"]
            require(name not in names and re.fullmatch(r"[A-Za-z0-9_.-]+", name), "Duplicate or invalid artifact")
            names.add(name)
            require(len(artifact) == 1 and artifact[0].tag == NS + "sha256", "Exactly one SHA-256 is required")
            checksum = artifact[0]
            require(set(checksum.attrib) <= {"value", "origin"} and len(checksum) == 0
                    and re.fullmatch(r"[a-f0-9]{64}", checksum.attrib.get("value", "")), "Invalid checksum entry")
            artifacts += 1

    locked = set()
    scopes = {}
    for relative in LOCKS:
        rows = [line for line in (root / relative).read_text().splitlines() if line and not line.startswith("#")]
        require(sum(line.startswith("empty=") for line in rows) == 1, "Lock must record empty configurations")
        by_configuration = {}
        for row in rows:
            component, configurations = row.split("=", 1)
            if component == "empty":
                continue
            identity = tuple(component.split(":"))
            require(len(identity) == 3 and exact_version(identity[2]) and identity in verified,
                    "Locked component is mutable or lacks checksum metadata")
            locked.add(identity)
            require(configurations, "Locked component has no configuration")
            for configuration in configurations.split(","):
                versions = by_configuration.setdefault(configuration, {})
                require(identity[:2] not in versions, "Duplicate module in one locked configuration")
                versions[identity[:2]] = identity[2]
        scopes[relative] = len(by_configuration)
    return {"locked_components": len(locked), "verified_components": len(verified),
            "verified_artifacts": artifacts, "nonempty_configurations": scopes}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args()
    print(json.dumps(check(args.root), sort_keys=True))
