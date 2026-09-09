#!/usr/bin/env python3
"""Exercise actual Gradle refusals using isolated copies of Tundra's build files.

Run after a successful locked Android Gradle `help` to populate the user-owned cache.
Requires Gradle 8.13 and JDK 17, but no Android SDK, wallet files or network downloads.
"""
from pathlib import Path
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
ANDROID = ROOT / "apps/android"
FILES = ("settings.gradle.kts", "build.gradle.kts", "gradle.properties", "app/build.gradle.kts",
         "buildscript-gradle.lockfile", "app/buildscript-gradle.lockfile", "app/gradle.lockfile",
         "gradle/verification-metadata.xml")


def main():
    logs = ROOT / "build/gradle-enforcement"
    logs.mkdir(parents=True, exist_ok=True)
    for case in ("baseline", "missing-lock", "verification-disabled", "checksum-mismatch"):
        with tempfile.TemporaryDirectory(prefix="gradle-enforcement-", dir=ROOT / "build") as temporary:
            target = Path(temporary)
            for relative in FILES:
                destination = target / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(ANDROID / relative, destination)
            args = []
            expected = None
            if case == "missing-lock":
                (target / "app/gradle.lockfile").unlink()
                expected = "Required reviewed Android dependency file is missing"
            elif case == "verification-disabled":
                args = ["--dependency-verification", "off"]
                expected = "Android builds require strict dependency checksum verification"
            elif case == "checksum-mismatch":
                path = target / "gradle/verification-metadata.xml"
                ns = {"v": "https://schema.gradle.org/dependency-verification"}
                tree = ET.parse(path)
                checksum = tree.find('.//v:component[@group="com.android.tools.build"][@name="gradle"]'
                                     '/v:artifact[@name="gradle-8.13.2.jar"]/v:sha256', ns)
                if checksum is None:
                    raise RuntimeError("Pinned Android Gradle plugin checksum was not found")
                digest = checksum.attrib["value"]
                text = path.read_text()
                if text.count(digest) != 1:
                    raise RuntimeError("Checksum mutation was not unique")
                path.write_text(text.replace(digest, "0" * 64))
                expected = "Dependency verification failed"
            result = subprocess.run([str(ANDROID / "gradlew"), "--no-daemon", "--offline",
                                     "-p", str(target), *args, "help"],
                                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=90)
            # Only build configuration and disposable paths are involved; no wallet data.
            (logs / f"{case}.log").write_bytes(result.stdout)
            output = result.stdout.decode("utf-8", errors="replace")
            if expected is None:
                if result.returncode != 0 or "BUILD SUCCESSFUL" not in output:
                    raise RuntimeError("Isolated locked Gradle baseline failed")
            elif result.returncode == 0 or expected not in output:
                raise RuntimeError(f"Gradle did not produce the required {case} refusal")
            print(f"{case}: passed", flush=True)


if __name__ == "__main__":
    main()
