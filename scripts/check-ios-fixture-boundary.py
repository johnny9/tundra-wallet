#!/usr/bin/env python3
"""Check actual generated Xcode build phases before compiling the two app targets."""
import json
import sys
from pathlib import Path


def check(project):
    objects = project["objects"]

    def members(name, phase_type):
        targets = [row for row in objects.values()
                   if row.get("isa") == "PBXNativeTarget" and row.get("name") == name]
        if len(targets) != 1:
            raise ValueError("Expected exactly one named app target")
        files = set()
        for phase_id in targets[0]["buildPhases"]:
            phase = objects[phase_id]
            if phase["isa"] != phase_type:
                continue
            for build_id in phase["files"]:
                reference = objects[objects[build_id]["fileRef"]]
                files.add(Path(reference["path"]).name)
        return files

    production_sources = members("Tundra", "PBXSourcesBuildPhase")
    host_sources = members("TundraTestHost", "PBXSourcesBuildPhase")
    production_resources = members("Tundra", "PBXResourcesBuildPhase")
    host_resources = members("TundraTestHost", "PBXResourcesBuildPhase")
    fixtures = {"native-unsigned.sqlite", "native-signed-response.psbt", "native-signed-backup.tundra", "two-of-three.txt"}
    if "TundraApp.swift" not in production_sources or "FixtureApp.swift" in production_sources:
        raise ValueError("Production app entry point boundary changed")
    if "FixtureApp.swift" not in host_sources or "TundraApp.swift" in host_sources:
        raise ValueError("Public test host entry point boundary changed")
    if "WalletViews.swift" not in production_sources & host_sources:
        raise ValueError("Both targets must compile the shared native views")
    if not fixtures <= host_resources or fixtures & production_resources:
        raise ValueError("Public signing resources crossed the test-host boundary")
    if "THIRD-PARTY-NOTICES.txt" not in production_resources & host_resources:
        raise ValueError("Both app targets must include the reviewed notice resource")
    if any(Path(name).suffix in {".sqlite", ".psbt", ".tundra"} for name in production_resources):
        raise ValueError("Wallet data must not be bundled in the production app")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("Usage: check-ios-fixture-boundary.py <plutil project JSON>")
    check(json.loads(Path(sys.argv[1]).read_text()))
    print("Generated Xcode phases keep public fixtures and the test entry point outside Tundra.")
