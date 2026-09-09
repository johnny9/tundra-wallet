//! Atomic native store selection. The selector contains no wallet metadata or keys.
use crate::{BackupSummary, Core, Error, Result, StorageFormat, storage};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

const SELECTOR: &str = "active-store.v1";
const STORES: &str = "restored-stores";
const HEADER: &str = "Tundra store v1\n";

pub struct StorageLocation {
    pub generation: String,
    pub directory: PathBuf,
    pub require_existing: bool,
}

fn regular(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() => Ok(true),
        Ok(_) => Err(Error::Storage),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(Error::Storage),
    }
}

fn directory(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => Ok(true),
        Ok(_) => Err(Error::Storage),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(Error::Storage),
    }
}

fn root(path: &Path) -> Result<PathBuf> {
    let canonical = path.canonicalize().map_err(|_| Error::Storage)?;
    if !canonical.is_dir() {
        return Err(Error::Storage);
    }
    Ok(canonical)
}

fn valid_generation(value: &str) -> bool {
    value.len() == 22
        && value.starts_with("store-")
        && value.as_bytes()[6..].iter().all(u8::is_ascii_alphanumeric)
}

fn read_selection(root: &Path) -> Result<StorageLocation> {
    let selector = root.join(SELECTOR);
    if !regular(&selector)? {
        // Evidence of any prior recovery prevents fallback to a new empty default.
        if directory(&root.join(STORES))? {
            return Err(Error::StorageLocked);
        }
        return Ok(StorageLocation {
            generation: "default".into(),
            directory: root.to_owned(),
            require_existing: false,
        });
    }
    let mut bytes = Vec::new();
    File::open(selector)
        .map_err(|_| Error::Storage)?
        .take(65)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Storage)?;
    let value = std::str::from_utf8(&bytes).map_err(|_| Error::StorageLocked)?;
    let generation = value
        .strip_prefix(HEADER)
        .and_then(|v| v.strip_suffix('\n'))
        .ok_or(Error::StorageLocked)?;
    let location = if generation == "default" {
        root.to_owned()
    } else if valid_generation(generation) && directory(&root.join(STORES))? {
        root.join(STORES).join(generation)
    } else {
        return Err(Error::StorageLocked);
    };
    if !directory(&location)? {
        return Err(Error::StorageLocked);
    }
    Ok(StorageLocation {
        generation: generation.into(),
        directory: location,
        require_existing: true,
    })
}

/// Resolve selection before opening a native vault. When `require_existing` is true,
/// the caller must refuse a missing DB or missing key instead of initializing either.
pub fn selected_storage(path: impl AsRef<Path>) -> Result<StorageLocation> {
    let root = root(path.as_ref())?;
    let _lock = storage::shared_lock(&root.join(SELECTOR))?;
    read_selection(&root)
}

fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|_| Error::Storage)
}

fn write_selection(root: &Path, generation: &str) -> Result<()> {
    // A corrupt regular selector can be replaced only by an explicitly completed
    // restore. A symlink/directory is never followed or replaced.
    let _ = regular(&root.join(SELECTOR))?;
    let mut temp = tempfile::Builder::new()
        .prefix(".active-store-")
        .tempfile_in(root)
        .map_err(|_| Error::Storage)?;
    writeln!(temp, "{HEADER}{generation}").map_err(|_| Error::Storage)?;
    temp.as_file().sync_all().map_err(|_| Error::Storage)?;
    temp.persist(root.join(SELECTOR))
        .map_err(|_| Error::Storage)?;
    sync_directory(root)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Boundary {
    Anchored,
    Allocated,
    Verified,
    Selected,
}

/// A single recovery attempt, holding the selection lock through key retention,
/// restoration and activation. Dropping it never deletes old or candidate data.
pub struct StorageRestore {
    root: PathBuf,
    location: StorageLocation,
    database_name: String,
    key: Option<Zeroizing<Vec<u8>>>,
    activated: bool,
    _lock: storage::StorageLock,
}

impl StorageRestore {
    pub fn begin(path: impl AsRef<Path>, database_name: &str) -> Result<Self> {
        Self::begin_at(path.as_ref(), database_name, |_| Ok(()))
    }

    fn begin_at(
        path: &Path,
        database_name: &str,
        boundary: impl Fn(Boundary) -> Result<()>,
    ) -> Result<Self> {
        if database_name.is_empty()
            || database_name.len() > 64
            || database_name == "."
            || database_name == ".."
            || !database_name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(Error::InvalidInput("storage name"));
        }
        let root = root(path)?;
        let lock = storage::exclusive_lock(&root.join(SELECTOR))?;
        let stores = root.join(STORES);
        let stores_exist = directory(&stores)?;
        if !regular(&root.join(SELECTOR))? && !stores_exist {
            // Persist this before creating any candidate. An interrupted first restore
            // cannot make a missing selector look like a new install.
            write_selection(&root, "default")?;
        }
        boundary(Boundary::Anchored)?;
        if !stores_exist {
            fs::create_dir(&stores).map_err(|_| Error::Storage)?;
            sync_directory(&root)?;
        }
        let candidate = tempfile::Builder::new()
            .prefix("store-")
            .rand_bytes(16)
            .tempdir_in(&stores)
            .map_err(|_| Error::Storage)?
            .keep();
        sync_directory(&stores)?;
        boundary(Boundary::Allocated)?;
        let generation = candidate
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(Error::Storage)?
            .to_owned();
        if !valid_generation(&generation) {
            return Err(Error::Storage);
        }
        Ok(Self {
            root,
            location: StorageLocation {
                generation,
                directory: candidate,
                require_existing: true,
            },
            database_name: database_name.into(),
            key: None,
            activated: false,
            _lock: lock,
        })
    }

    pub fn location(&self) -> &StorageLocation {
        &self.location
    }

    /// Native code must retain this key durably before calling, then mark its key
    /// record initialized after success and before `activate`. No Bitcoin keys exist here.
    pub fn restore(
        &mut self,
        source: impl AsRef<Path>,
        password: String,
        key: Vec<u8>,
    ) -> Result<BackupSummary> {
        let key = Zeroizing::new(key);
        if self.key.is_some() || self.activated {
            return Err(Error::Storage);
        }
        let summary = crate::restore_backup(
            source,
            self.location.directory.join(&self.database_name),
            password,
            key.to_vec(),
        )?;
        self.key = Some(key);
        Ok(summary)
    }

    pub fn activate(&mut self) -> Result<()> {
        self.activate_at(|_| Ok(()))
    }

    fn activate_at(&mut self, boundary: impl Fn(Boundary) -> Result<()>) -> Result<()> {
        if self.activated {
            return Err(Error::Storage);
        }
        let key = self.key.as_ref().ok_or(Error::Storage)?;
        let path = self.location.directory.join(&self.database_name);
        if !directory(&self.location.directory)?
            || storage::storage_format(&path)? != StorageFormat::ProtectedOrUnknown
        {
            return Err(Error::StorageLocked);
        }
        // Verify the installed DB again immediately before switching, using the exact
        // key retained for this attempt. This never recreates a missing database.
        let verified = Core::open_protected(path, key.to_vec())?;
        let _ = verified.wallets()?;
        drop(verified);
        sync_directory(&self.location.directory)?;
        boundary(Boundary::Verified)?;
        write_selection(&self.root, &self.location.generation)?;
        self.activated = true;
        self.key = None;
        boundary(Boundary::Selected)
    }
}

#[cfg(test)]
#[path = "store_selection_tests.rs"]
mod tests;
