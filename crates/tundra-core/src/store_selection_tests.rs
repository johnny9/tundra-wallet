use super::*;
use crate::Network;
use std::process::{Command, Stdio};
const PASSWORD: &str = "Public backup test password 🧊 ' spaces ";
const KEY: [u8; 32] = [0x33; 32]; // Public storage-encryption fixture, never a signing key.

fn backup() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/backup-v1.tundra")
}
fn old_store(root: &Path) -> Vec<u8> {
    let core = Core::open_protected(root.join("wallet.sqlite"), KEY.to_vec()).unwrap();
    core.import_wallet(
        "Original",
        include_str!("../../../tests/fixtures/single-sig.txt"),
        Network::Signet,
    )
    .unwrap();
    drop(core);
    fs::read(root.join("wallet.sqlite")).unwrap()
}

#[test]
fn successful_restore_switches_only_after_explicit_activation_and_retains_old_store() {
    let root = tempfile::tempdir().unwrap();
    let before = old_store(root.path());
    assert_eq!(selected_storage(root.path()).unwrap().generation, "default");
    let mut restore = StorageRestore::begin(root.path(), "wallet.sqlite").unwrap();
    let generation = restore.location().generation.clone();
    let candidate = restore.location().directory.clone();
    assert!(selected_storage(root.path()).is_err()); // Selection is locked during key retention.
    assert!(restore.activate().is_err());
    restore
        .restore(backup(), PASSWORD.into(), KEY.to_vec())
        .unwrap();
    assert_eq!(
        fs::read_to_string(root.path().join(SELECTOR)).unwrap(),
        format!("{HEADER}default\n")
    );
    restore.activate().unwrap();
    assert!(restore.activate().is_err());
    drop(restore);
    let selected = selected_storage(root.path()).unwrap();
    assert!(selected.require_existing);
    assert_eq!(selected.generation, generation);
    assert_eq!(selected.directory, candidate);
    let core =
        Core::open_protected(selected.directory.join("wallet.sqlite"), KEY.to_vec()).unwrap();
    assert_eq!(core.wallets().unwrap()[0].name, "Backup public fixture");
    assert!(core.wallets().unwrap()[0].total_sats.is_none());
    assert_eq!(fs::read(root.path().join("wallet.sqlite")).unwrap(), before);
}

#[test]
fn failed_or_cancelled_recovery_never_changes_selection_or_clears_existing_data() {
    let root = tempfile::tempdir().unwrap();
    let before = old_store(root.path());
    let mut restore = StorageRestore::begin(root.path(), "wallet.sqlite").unwrap();
    assert!(StorageRestore::begin(root.path(), "wallet.sqlite").is_err());
    let candidate = restore.location().directory.clone();
    assert!(
        restore
            .restore(
                backup(),
                "Definitely incorrect password".into(),
                KEY.to_vec()
            )
            .is_err()
    );
    assert!(restore.activate().is_err());
    drop(restore);
    assert!(candidate.is_dir());
    assert_eq!(selected_storage(root.path()).unwrap().generation, "default");
    assert_eq!(fs::read(root.path().join("wallet.sqlite")).unwrap(), before);
    let mut restore = StorageRestore::begin(root.path(), "wallet.sqlite").unwrap();
    restore
        .restore(backup(), PASSWORD.into(), KEY.to_vec())
        .unwrap();
    let installed = restore.location().directory.join("wallet.sqlite");
    fs::remove_file(&installed).unwrap();
    assert!(restore.activate().is_err());
    assert!(!installed.exists());
    drop(restore);
    assert_eq!(selected_storage(root.path()).unwrap().generation, "default");
}

#[test]
fn missing_corrupt_or_aliased_selection_fails_closed_but_explicit_restore_can_recover() {
    let root = tempfile::tempdir().unwrap();
    let before = old_store(root.path());
    let initial = selected_storage(root.path()).unwrap();
    assert!(!initial.require_existing);
    drop(StorageRestore::begin(root.path(), "wallet.sqlite").unwrap());
    fs::remove_file(root.path().join(SELECTOR)).unwrap();
    assert!(selected_storage(root.path()).is_err());
    for bytes in [
        b"".to_vec(),
        b"garbage".to_vec(),
        format!("{HEADER}../escape\n").into_bytes(),
        vec![b'x'; 100_000],
        format!("{HEADER}store-0000000000000000\n").into_bytes(),
    ] {
        fs::write(root.path().join(SELECTOR), bytes).unwrap();
        assert!(selected_storage(root.path()).is_err());
    }
    let mut restore = StorageRestore::begin(root.path(), "wallet.sqlite").unwrap();
    restore
        .restore(backup(), PASSWORD.into(), KEY.to_vec())
        .unwrap();
    restore.activate().unwrap();
    drop(restore);
    let location = selected_storage(root.path()).unwrap();
    assert_ne!(location.generation, "default");
    assert_eq!(fs::read(root.path().join("wallet.sqlite")).unwrap(), before);
    fs::rename(&location.directory, root.path().join("retained-test-store")).unwrap();
    assert!(selected_storage(root.path()).is_err());
}

#[cfg(unix)]
#[test]
fn symlinks_are_not_selection_or_generation_directories() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("unchanged");
    fs::write(&target, format!("{HEADER}default\n")).unwrap();
    symlink(&target, root.path().join(SELECTOR)).unwrap();
    assert!(selected_storage(root.path()).is_err());
    assert!(StorageRestore::begin(root.path(), "wallet.sqlite").is_err());
    fs::remove_file(root.path().join(SELECTOR)).unwrap();
    symlink(root.path(), root.path().join(STORES)).unwrap();
    assert!(selected_storage(root.path()).is_err());
    assert!(StorageRestore::begin(root.path(), "wallet.sqlite").is_err());
    assert_eq!(
        fs::read_to_string(target).unwrap(),
        format!("{HEADER}default\n")
    );
}

#[test]
fn invalid_database_names_cannot_escape_candidate_directory() {
    let root = tempfile::tempdir().unwrap();
    for name in [
        "",
        ".",
        "..",
        "../wallet.sqlite",
        "/wallet.sqlite",
        "file:wallet.sqlite",
        "a/b",
        "a\\b",
    ] {
        assert!(StorageRestore::begin(root.path(), name).is_err());
    }
    assert!(!root.path().join(SELECTOR).exists());
    assert!(!root.path().join(STORES).exists());
}

#[test]
fn injected_selection_failures_leave_a_complete_old_or_new_store() {
    for stop in [
        Boundary::Anchored,
        Boundary::Allocated,
        Boundary::Verified,
        Boundary::Selected,
    ] {
        let root = tempfile::tempdir().unwrap();
        let before = old_store(root.path());
        let fail = |at| {
            if at == stop {
                Err(Error::Storage)
            } else {
                Ok(())
            }
        };
        match StorageRestore::begin_at(root.path(), "wallet.sqlite", fail) {
            Err(_) => assert!(matches!(stop, Boundary::Anchored | Boundary::Allocated)),
            Ok(mut restore) => {
                restore
                    .restore(backup(), PASSWORD.into(), KEY.to_vec())
                    .unwrap();
                assert!(restore.activate_at(fail).is_err());
            }
        }
        let selected = selected_storage(root.path()).unwrap();
        assert_eq!(selected.generation == "default", stop != Boundary::Selected);
        let core =
            Core::open_protected(selected.directory.join("wallet.sqlite"), KEY.to_vec()).unwrap();
        assert_eq!(core.wallets().unwrap().len(), 1);
        assert_eq!(fs::read(root.path().join("wallet.sqlite")).unwrap(), before);
    }
}

#[test]
fn actual_kills_at_selection_boundaries_reopen_a_complete_store_and_release_locks() {
    use std::io::{BufRead, BufReader};
    for (name, selected_new) in [
        ("anchored", false),
        ("allocated", false),
        ("verified", false),
        ("selected", true),
    ] {
        let root = tempfile::tempdir().unwrap();
        let before = old_store(root.path());
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "store_selection::tests::selection_child",
                "--ignored",
                "--nocapture",
            ])
            .env("TUNDRA_SELECTION_TEST_ROOT", root.path())
            .env("TUNDRA_SELECTION_STOP", name)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut ready = false;
        loop {
            let mut line = String::new();
            if output.read_line(&mut line).unwrap() == 0 {
                break;
            }
            if line.trim() == "at-boundary" {
                ready = true;
                break;
            }
        }
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(ready, "child did not reach the public test boundary");
        let selected = selected_storage(root.path()).unwrap();
        assert_eq!(selected.generation != "default", selected_new);
        let core =
            Core::open_protected(selected.directory.join("wallet.sqlite"), KEY.to_vec()).unwrap();
        assert_eq!(core.wallets().unwrap().len(), 1);
        assert_eq!(fs::read(root.path().join("wallet.sqlite")).unwrap(), before);
        drop(core);
        assert!(StorageRestore::begin(root.path(), "wallet.sqlite").is_ok());
    }
}

#[test]
#[ignore = "child process fixture invoked by selection kill test"]
fn selection_child() {
    let root = PathBuf::from(std::env::var("TUNDRA_SELECTION_TEST_ROOT").unwrap());
    let stop = std::env::var("TUNDRA_SELECTION_STOP").unwrap();
    let boundary = |at| {
        let name = match at {
            Boundary::Anchored => "anchored",
            Boundary::Allocated => "allocated",
            Boundary::Verified => "verified",
            Boundary::Selected => "selected",
        };
        if name == stop {
            println!("at-boundary");
            std::io::stdout().flush().unwrap();
            loop {
                std::thread::park();
            }
        }
        Ok(())
    };
    let mut restore = StorageRestore::begin_at(&root, "wallet.sqlite", boundary).unwrap();
    restore
        .restore(backup(), PASSWORD.into(), KEY.to_vec())
        .unwrap();
    restore.activate_at(boundary).unwrap();
    panic!("missing test boundary");
}
