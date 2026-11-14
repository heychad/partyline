//! The human's access choice, saved independently of the socket and activity log.
use super::{Access, Service};
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::{
    fs, io,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SavedAccess {
    version: u8,
    access: Access,
}

pub struct AccessStore {
    directory: PathBuf,
}

fn unsafe_path() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "Agent access settings must be private regular files.",
    )
}

impl AccessStore {
    pub fn new(directory: &Path) -> io::Result<Self> {
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        builder.mode(0o700);
        match builder.create(directory) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        let store = Self {
            directory: directory.into(),
        };
        store.check_directory()?;
        Ok(store)
    }

    fn check_directory(&self) -> io::Result<fs::Metadata> {
        let metadata = fs::symlink_metadata(&self.directory)?;
        if !metadata.is_dir() {
            return Err(unsafe_path());
        }
        #[cfg(unix)]
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(unsafe_path());
        }
        Ok(metadata)
    }

    fn path(&self) -> PathBuf {
        self.directory.join("access.json")
    }

    pub fn load(&self) -> io::Result<Access> {
        let directory = self.check_directory()?;
        let metadata = match fs::symlink_metadata(self.path()) {
            Ok(m) => m,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Access::Off),
            Err(e) => return Err(e),
        };
        if !metadata.is_file() || metadata.len() > 128 {
            return Err(unsafe_path());
        }
        #[cfg(unix)]
        if metadata.uid() != directory.uid() || metadata.permissions().mode() & 0o077 != 0 {
            return Err(unsafe_path());
        }
        let saved: SavedAccess = serde_json::from_slice(&fs::read(self.path())?)?;
        if saved.version != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unknown access settings version.",
            ));
        }
        Ok(saved.access)
    }

    pub fn restore(&self, service: &Service) -> io::Result<()> {
        match self.load() {
            Ok(access) => {
                service.set_access(access);
                Ok(())
            }
            Err(error) => {
                service.set_access(Access::Off);
                Err(error)
            }
        }
    }

    // Persist before applying: a failed save cannot silently grant wider access.
    // Only the trusted main-window permission command calls this in the app.
    pub fn apply(&self, service: &Service, access: Access) -> io::Result<()> {
        let directory = self.check_directory()?;
        match fs::symlink_metadata(self.path()) {
            Ok(metadata) => {
                if !metadata.is_file() {
                    return Err(unsafe_path());
                }
                #[cfg(unix)]
                if metadata.uid() != directory.uid() {
                    return Err(unsafe_path());
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let mut pending = tempfile::Builder::new()
            .prefix(".access-")
            .tempfile_in(&self.directory)?;
        #[cfg(unix)]
        pending
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
        pending.write_all(&serde_json::to_vec(&SavedAccess { version: 1, access })?)?;
        pending.as_file().sync_all()?;
        pending.persist(self.path()).map_err(|e| e.error)?;
        service.set_access(access);
        Ok(())
    }
}
