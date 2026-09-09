#!/usr/bin/env python3
"""Checks runnable without Rust/mobile SDKs. These DO NOT compile or test the Rust core."""
from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import sqlite3
import subprocess
import time
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCHEMA = (ROOT / "crates/tundra-core/src/schema.sql").read_text()
INPUT_CHARSET = "0123456789()[],'/*abcdefgh@:$%{}IJKLMNOPQRSTUVWXYZ&+-.;<=>?!^_|~ijklmnopqrstuvwxyzABCDEFGH`#\"\\ "
CHECKSUM_CHARSET = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"


def descriptor_checksum(body: str) -> str:
    """Independent BIP 380 checksum check for the supplied public test fixture files."""
    generators = (0xF5DEE51989, 0xA9FDCA3312, 0x1BAB10E32D, 0x3706B1677A, 0x644D626FFD)
    def step(c: int, val: int) -> int:
        high = c >> 35
        c = ((c & 0x7FFFFFFFF) << 5) ^ val
        for i, generator in enumerate(generators):
            if (high >> i) & 1:
                c ^= generator
        return c
    checksum, group, count = 1, 0, 0
    for char in body:
        position = INPUT_CHARSET.index(char)
        checksum = step(checksum, position & 31)
        group = group * 3 + (position >> 5)
        count += 1
        if count == 3:
            checksum = step(checksum, group)
            group = count = 0
    if count:
        checksum = step(checksum, group)
    for _ in range(8):
        checksum = step(checksum, 0)
    checksum ^= 1
    return "".join(CHECKSUM_CHARSET[(checksum >> (5 * (7-i))) & 31] for i in range(8))


def db_open() -> sqlite3.Connection:
    db = sqlite3.connect(":memory:", isolation_level=None)
    db.execute("PRAGMA foreign_keys=ON")
    db.executescript(SCHEMA)
    for wallet in ("a", "b"):
        db.execute("INSERT INTO wallets VALUES (?,?, 'signet','single_sig','{}',NULL,1)", (wallet, wallet))
    return db


class SchemaChecks(unittest.TestCase):
    def setUp(self):
        self.db = db_open()
    def tearDown(self):
        self.db.close()
    def draft(self, wallet="a", ident="draft"):
        self.db.execute("INSERT INTO drafts VALUES(?,?, 'UNSIGNED_TEST_MARKER','{}','',1)", (ident, wallet))
    def test_version_and_foreign_keys(self):
        self.assertEqual(self.db.execute("PRAGMA user_version").fetchone()[0], 8)
        self.assertEqual(self.db.execute("PRAGMA foreign_keys").fetchone()[0], 1)
    def test_unknown_sync_is_null_not_zero(self):
        self.assertIsNone(self.db.execute("SELECT synced_at FROM wallets WHERE id='a'").fetchone()[0])
    def test_labels_are_wallet_scoped(self):
        for wallet, label in (("a", "Personal"), ("b", "Other")):
            self.db.execute("INSERT INTO labels VALUES(?, 'output', 'out:0', ?)", (wallet, label))
        self.assertEqual(self.db.execute("SELECT label FROM labels ORDER BY wallet_id").fetchall(), [("Personal",), ("Other",)])
    def test_label_type_constraint(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO labels VALUES('a', 'privatekey', 'x', 'x')")
    def test_unicode_label_character_limit(self):
        self.db.execute("INSERT INTO labels VALUES('a','addr','ref',?)", ("é"*255,))
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("UPDATE labels SET label=?", ("é"*256,))
    def test_name_limit(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("UPDATE wallets SET name='' WHERE id='a'")
    def test_reservations_exclusive_within_wallet(self):
        self.draft(ident="one"); self.draft(ident="two")
        self.db.execute("INSERT INTO reservations VALUES('a','out:0','one')")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO reservations VALUES('a','out:0','two')")
    def test_reservation_cannot_target_another_wallet_draft(self):
        self.draft(wallet="b")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO reservations VALUES('a','out:0','draft')")
    def test_discard_releases_only_reservations_not_freezes(self):
        self.draft()
        self.db.execute("INSERT INTO freezes VALUES('a','out:0')")
        self.db.execute("INSERT INTO reservations VALUES('a','out:0','draft')")
        self.db.execute("DELETE FROM drafts WHERE wallet_id='a' AND id='draft'")
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM reservations").fetchone()[0], 0)
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM freezes").fetchone()[0], 1)
    def test_discard_one_draft_leaves_other_reservation(self):
        self.draft(ident="one"); self.draft(ident="two")
        self.db.execute("INSERT INTO reservations VALUES('a','out:0','one')")
        self.db.execute("INSERT INTO reservations VALUES('a','out:1','two')")
        self.db.execute("DELETE FROM drafts WHERE id='one'")
        self.assertEqual(self.db.execute("SELECT draft_id FROM reservations").fetchall(), [("two",)])
    def test_wallet_delete_cascades_only_its_metadata(self):
        for wallet in ("a", "b"):
            self.draft(wallet=wallet, ident=wallet)
            self.db.execute("INSERT INTO freezes VALUES(?, 'out:0')", (wallet,))
            self.db.execute("INSERT INTO labels VALUES(?, 'addr','x','label')", (wallet,))
            self.db.execute("INSERT INTO reservations VALUES(?, 'out:0', ?)", (wallet, wallet))
            self.db.execute("INSERT INTO draft_label_applications VALUES(?,?)", (wallet, wallet))
            self.db.execute("INSERT INTO output_provenance VALUES(?,'out:0',?)", (wallet, wallet))
            self.db.execute("INSERT INTO finalized_drafts VALUES(?,?,X'00')", (wallet, wallet))
            self.db.execute("INSERT INTO recovered_submissions VALUES(?,?)", (wallet, wallet))
            self.db.execute("INSERT INTO recovery_holds VALUES(?,'out:0',?)", (wallet, wallet))
        self.db.execute("DELETE FROM wallets WHERE id='a'")
        for table in ("labels", "freezes", "drafts", "reservations", "draft_label_applications", "output_provenance", "finalized_drafts", "recovered_submissions", "recovery_holds"):
            self.assertEqual(self.db.execute(f"SELECT wallet_id FROM {table}").fetchall(), [("b",)])
    def test_provenance_cannot_reference_another_wallet_draft(self):
        self.draft(wallet="b")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO draft_label_applications VALUES('a','draft')")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO output_provenance VALUES('a','out:0','draft')")
    def test_recovery_holds_cannot_reference_another_wallet_submission(self):
        self.draft(wallet="b")
        self.db.execute("INSERT INTO finalized_drafts VALUES('b','draft',X'00')")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO recovered_submissions VALUES('a','draft')")
        self.db.execute("INSERT INTO recovered_submissions VALUES('b','draft')")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO recovery_holds VALUES('a','out:0','draft')")
    def test_wallet_state_and_metadata_rollback_together(self):
        self.db.execute("BEGIN IMMEDIATE")
        self.db.execute("UPDATE wallets SET state_json='new' WHERE id='a'")
        self.db.execute("INSERT INTO labels VALUES('a','addr','x','new')")
        self.db.execute("ROLLBACK")
        self.assertEqual(self.db.execute("SELECT state_json FROM wallets WHERE id='a'").fetchone()[0], "{}")
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM labels").fetchone()[0], 0)


class FixtureAndSourceChecks(unittest.TestCase):
    def test_android_archive_notice_graph_and_retained_bytes(self):
        subprocess.run(["python3", str(ROOT / "scripts/android-artifact-notices.py")],
                       check=True, capture_output=True, timeout=30)

    def test_android_maven_inventory_matches_metadata_and_retained_poms(self):
        subprocess.run(["python3", str(ROOT / "scripts/android-notices.py")],
                       check=True, capture_output=True, timeout=30)

    def test_android_locks_and_checksum_policy_reject_missing_coverage_and_bypasses(self):
        spec = importlib.util.spec_from_file_location("android_dependencies", ROOT / "scripts/check-android-dependencies.py")
        checker = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(checker)
        original = ROOT / "apps/android"
        report = checker.check(original)
        self.assertGreater(report["locked_components"], 0)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for relative in (*checker.LOCKS, "gradle/verification-metadata.xml"):
                destination = root / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(original / relative, destination)
            metadata = root / "gradle/verification-metadata.xml"
            valid = metadata.read_text()
            for invalid in (
                valid.replace("<verify-metadata>true", "<verify-metadata>false"),
                valid.replace("</configuration>", "<trusted-artifacts/></configuration>"),
                valid.replace('<sha256 value="', '<sha256 value="bad', 1),
                re.sub(r'(<component[^>]*version=")[^"]+', r'\g<1>1.0-SNAPSHOT', valid, count=1),
            ):
                metadata.write_text(invalid)
                with self.assertRaises(ValueError):
                    checker.check(root)
            metadata.write_text(valid)
            lock = root / "app/gradle.lockfile"
            lock.write_text(lock.read_text() + "dev.tundra.test:unreviewed:1.0=debugRuntimeClasspath\n")
            with self.assertRaises(ValueError):
                checker.check(root)

    def test_checksum_known_example(self):
        self.assertEqual(descriptor_checksum("raw(deadbeef)"), "89f8spxm")
    def test_public_fixture_checksums(self):
        for path in (ROOT/"tests/fixtures").glob("*.txt"):
            for row in path.read_text().splitlines():
                if not row.strip():
                    continue
                with self.subTest(file=path.name):
                    body, checksum = row.strip().split("#")
                    self.assertEqual(descriptor_checksum(body), checksum)
        data = json.loads((ROOT/"tests/fixtures/core-style-pair.json").read_text())
        for item in data["descriptors"]:
            body, checksum = item["desc"].split("#")
            self.assertEqual(descriptor_checksum(body), checksum)
    def test_fixture_xpub_encoding_and_origin_depth(self):
        alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
        for name in ("single-sig.txt", "two-of-three.txt"):
            data = (ROOT/"tests/fixtures"/name).read_text()
            matches = re.findall(r"\[([0-9a-f]{8})/([^]]+)\](tpub[1-9A-HJ-NP-Za-km-z]+)", data)
            self.assertTrue(matches)
            for _, origin, key in matches:
                n = 0
                for char in key:
                    n = n * 58 + alphabet.index(char)
                raw = n.to_bytes((n.bit_length()+7)//8, "big")
                self.assertEqual(len(raw), 82)
                self.assertEqual(hashlib.sha256(hashlib.sha256(raw[:-4]).digest()).digest()[:4], raw[-4:])
                self.assertEqual(raw[:4].hex(), "043587cf")
                self.assertEqual(raw[4], len(origin.split("/")))
                self.assertIn(raw[45], (2, 3))
    def test_no_private_extended_keys_in_fixtures(self):
        for path in (ROOT/"tests/fixtures").iterdir():
            if path.is_file():
                self.assertIsNone(re.search(rb"(?:xprv|tprv|yprv|zprv)[1-9A-HJ-NP-Za-km-z]{40,}", path.read_bytes()))
    def test_public_backup_fixture_hash_and_versioned_schema(self):
        fixtures = ROOT/"tests/fixtures"
        record = json.loads((fixtures/"backup-provenance.json").read_text())
        data = (fixtures/"backup-v1.tundra").read_bytes()
        self.assertEqual(len(data), record["size"])
        self.assertEqual(hashlib.sha256(data).hexdigest(), record["sha256"])
        self.assertEqual(hashlib.sha256((ROOT/"crates/tundra-core/src/backup_schema_v1.sql").read_bytes()).hexdigest(), record["schema_sha256"])
    def test_public_native_signing_fixture_hashes(self):
        fixtures = ROOT/"tests/fixtures"
        records = json.loads((fixtures/"native-signing-hashes.json").read_text())
        self.assertEqual(set(records), {"native-unsigned.sqlite", "native-signed-response.psbt", "native-signed-backup.tundra", "native-signing.json"})
        for name, record in records.items():
            data = (fixtures/name).read_bytes()
            self.assertEqual(len(data), record["size"])
            self.assertEqual(hashlib.sha256(data).hexdigest(), record["sha256"])
        # A hash check is not signature validation, decryption or a real chain scan.
    def test_cargo_manifest_syntax_and_private_packages(self):
        root = tomllib.loads((ROOT/"Cargo.toml").read_text())
        self.assertFalse(root["workspace"]["package"]["publish"])
        for name in root["workspace"]["members"]:
            manifest = tomllib.loads((ROOT/name/"Cargo.toml").read_text())
            self.assertTrue(manifest["package"]["publish"]["workspace"])
        tomllib.loads((ROOT/"crates/tundra-ffi/uniffi.toml").read_text())
    def test_ci_actions_match_reviewed_immutable_revisions(self):
        records = json.loads((ROOT/".github/action-pins.json").read_text())
        known = {row["repository"]: row["commit"] for row in records["actions"]}
        for path in (ROOT/".github/workflows").glob("*.yml"):
            for action, revision in re.findall(r"uses:\s*([A-Za-z0-9_./-]+)@([^\s]+)", path.read_text()):
                self.assertRegex(revision, r"^[a-f0-9]{40}$")
                self.assertEqual(revision, known["/".join(action.split("/")[:2])])
    def test_cargo_notice_inventory_covers_both_locks_and_preserves_bytes(self):
        inventory = json.loads((ROOT/"third-party/cargo-inventory.json").read_text())
        for lock, fingerprint in inventory["lock_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT/lock).read_bytes()).hexdigest(), fingerprint)
            packages = tomllib.loads((ROOT/lock).read_text())["package"]
            expected = {(p["name"], p["version"], p["source"], p.get("checksum")) for p in packages if p.get("source")}
            actual = {(p["name"], p["version"], p["source"], p["checksum"]) for p in inventory["packages"] if lock in p["graphs"]}
            self.assertEqual(actual, expected)
        for package in inventory["packages"]:
            self.assertTrue(package["notices"])
            for notice in package["notices"]:
                self.assertRegex(notice["sha256"], r"^[a-f0-9]{64}$")
                self.assertEqual(notice["retained"], "cargo-notices/" + notice["sha256"] + ".txt")
                data = (ROOT/"third-party"/notice["retained"]).read_bytes()
                self.assertEqual(len(data), notice["bytes"])
                self.assertEqual(hashlib.sha256(data).hexdigest(), notice["sha256"])
    def test_included_fixture_and_schema_paths_exist(self):
        for path in (ROOT/"crates").rglob("*.rs"):
            for target in re.findall(r'include_str!\("([^"]+)"\)', path.read_text()):
                self.assertTrue((path.parent/target).resolve().is_file(), f"{path}: {target}")
    def test_hardware_module_fail_closed_source_guard(self):
        source = (ROOT/"crates/tundra-core/src/hardware.rs").read_text()
        self.assertIn('Err(Error::Unavailable("hardware signing"))', source)
        self.assertRegex(source, r"available\s*:\s*false")
        # A source guard, not cryptographic or runtime proof.
    def test_transaction_manual_guard_and_mainnet_gate_source_guard(self):
        source = (ROOT/"crates/tundra-core/src/engine.rs").read_text()
        self.assertIn("manually_selected_only()", source)
        self.assertNotIn(".drain_wallet()", source)
        self.assertIn('Error::Unavailable("mainnet spending")', source)
    def test_android_permissions_are_limited_and_backup_disabled(self):
        import xml.etree.ElementTree as ET
        root = ET.parse(ROOT/"apps/android/app/src/main/AndroidManifest.xml").getroot()
        android = "{http://schemas.android.com/apk/res/android}"
        self.assertEqual(root.find("application").get(android+"allowBackup"), "false")
        self.assertEqual([p.get(android+"name") for p in root.findall("uses-permission")],
                         ["android.permission.INTERNET", "android.permission.CAMERA"])
        self.assertEqual([(f.get(android+"name"), f.get(android+"required")) for f in root.findall("uses-feature")],
                         [("android.hardware.camera.any", "false"), ("android.hardware.usb.host", "false")])
    def test_no_bdk_bhwi_direct_imports_in_native_ui(self):
        for path in (ROOT/"apps").rglob("*"):
            if path.suffix in (".kt", ".swift"):
                self.assertIsNone(re.search(r"^import\s+(?:bdk|bhwi)", path.read_text(), re.M))
    def test_shell_syntax(self):
        for path in (ROOT/"scripts").glob("*.sh"):
            subprocess.run(["bash", "-n", str(path)], check=True, capture_output=True)


if __name__ == "__main__":
    started = time.monotonic()
    suite = unittest.defaultTestLoader.loadTestsFromModule(__import__(__name__))
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    report = {
        "scope": "Offline SQLite, fixture and source guards; NOT Rust or native compilation",
        "tests_run": result.testsRun,
        "failures": len(result.failures), "errors": len(result.errors),
        "passed": result.wasSuccessful(),
        "elapsed_seconds": round(time.monotonic()-started, 3),
        "rust_tests": "NOT RUN by this checker", "android_build": "NOT RUN", "ios_build": "NOT RUN",
        "cargo_available": shutil.which("cargo") is not None,
    }
    output = ROOT/"build/offline-checks.json"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2)+"\n")
    raise SystemExit(0 if result.wasSuccessful() else 1)
