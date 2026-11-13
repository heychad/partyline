//! One bounded JSON request/response per connection, inside an owner-only directory.
use super::{Request, Service, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES};
use serde_json::json;
use std::{
    fs, io,
    os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::{watch, Semaphore},
};

pub struct Endpoint {
    listener: UnixListener,
    path: PathBuf,
    device: u64,
    inode: u64,
    uid: u32,
}

impl Endpoint {
    pub fn bind(directory: &Path) -> io::Result<Self> {
        match fs::DirBuilder::new().mode(0o700).create(directory) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let metadata = fs::symlink_metadata(directory)?;
        if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Agent directory must be private and must not be a symlink.",
            ));
        }
        let path = directory.join("control.sock");
        if let Ok(previous) = fs::symlink_metadata(&path) {
            if !previous.file_type().is_socket()
                || previous.uid() != metadata.uid()
                || previous.permissions().mode() & 0o077 != 0
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Unsafe agent socket path.",
                ));
            }
            match std::os::unix::net::UnixStream::connect(&path) {
                Ok(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::AddrInUse,
                        "Agent control is already running.",
                    ))
                }
                Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
                    let current = fs::symlink_metadata(&path)?;
                    if current.ino() != previous.ino() || current.dev() != previous.dev() {
                        return Err(io::Error::new(
                            io::ErrorKind::AddrInUse,
                            "Agent socket changed.",
                        ));
                    }
                    fs::remove_file(&path)?;
                }
                Err(error) => return Err(error),
            }
        }
        let listener = UnixListener::bind(&path)?;
        // The directory is 0700, so no other user can reach the socket before chmod.
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        let socket = fs::symlink_metadata(&path)?;
        if socket.uid() != metadata.uid() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Agent directory has another owner.",
            ));
        }
        Ok(Self {
            listener,
            path,
            device: socket.dev(),
            inode: socket.ino(),
            uid: socket.uid(),
        })
    }

    pub async fn serve(self, service: Arc<Service>, mut shutdown: watch::Receiver<bool>) {
        let capacity = Arc::new(Semaphore::new(8));
        loop {
            tokio::select! {
                biased;
                _ = shutdown.changed() => break,
                incoming = self.listener.accept() => {
                    let Ok((stream, _)) = incoming else { break; };
                    if stream.peer_cred().map(|p| p.uid() != self.uid).unwrap_or(true) { continue; }
                    let Ok(permit) = capacity.clone().try_acquire_owned() else { continue; };
                    let service = service.clone();
                    tokio::spawn(async move {
                        let _permit = permit;
                        let _ = exchange(stream, service).await;
                    });
                }
            }
        }
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        // Never unlink a socket that has replaced ours.
        if fs::symlink_metadata(&self.path)
            .is_ok_and(|m| m.dev() == self.device && m.ino() == self.inode)
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}

async fn exchange(stream: UnixStream, service: Arc<Service>) -> io::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read).take((MAX_REQUEST_BYTES + 1) as u64);
    let mut frame = Vec::new();
    let read =
        tokio::time::timeout(Duration::from_secs(5), reader.read_until(b'\n', &mut frame)).await;
    let result = match read {
        Ok(Ok(_)) if frame.len() <= MAX_REQUEST_BYTES && frame.last() == Some(&b'\n') => {
            match serde_json::from_slice::<Request>(&frame) {
                Ok(request) => service.execute(request).await,
                Err(_) => Err("Invalid agent request.".into()),
            }
        }
        _ => Err("Agent request is incomplete, too large, or timed out.".into()),
    };
    let response = match result {
        Ok(value) => json!({"ok": true, "result": value}),
        Err(error) => json!({"ok": false, "error": error}),
    };
    let mut bytes = serde_json::to_vec(&response)?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        bytes = serde_json::to_vec(
            &json!({"ok":false,"error":"Result is too large. Read a page of transcript turns instead."}),
        )?;
    }
    bytes.push(b'\n');
    let _ = tokio::time::timeout(Duration::from_secs(5), write.write_all(&bytes)).await;
    Ok(())
}
