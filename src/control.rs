use crate::{ipc::Result, storage};
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::{OpenOptionsExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    time::Duration,
};
pub fn socket_path() -> Result<PathBuf> {
    let runtime = PathBuf::from(env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR is unset")?);
    let display = env::var_os("WAYLAND_DISPLAY").ok_or("WAYLAND_DISPLAY is unset")?;
    let name = std::path::Path::new(&display)
        .file_name()
        .ok_or("invalid WAYLAND_DISPLAY")?
        .to_string_lossy();
    Ok(runtime.join("driftink").join(format!("{name}.sock")))
}
pub fn send(command: &str) -> Result<String> {
    let mut stream = UnixStream::connect(socket_path()?).map_err(|e| {
        format!(
            "DriftInk is not running in this session ({e}); start `driftink run` or `driftink draw`"
        )
    })?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    writeln!(stream, "{command}")?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply)?;
    if reply.starts_with("ERR ") {
        return Err(reply.trim().into());
    }
    if reply.is_empty() {
        return Err("DriftInk closed the control connection".into());
    }
    Ok(reply.trim().into())
}
pub struct Guard {
    pub document: PathBuf,
    pub listener: UnixListener,
    path: PathBuf,
    _lock: File,
}
impl Guard {
    pub fn acquire() -> Result<Self> {
        let document = if let Some(path) = env::var_os("DRIFTINK_DOCUMENT") {
            PathBuf::from(path)
        } else {
            storage::data_path()?
        };
        if !document.is_absolute() {
            return Err("DRIFTINK_DOCUMENT must be absolute".into());
        }
        fs::create_dir_all(document.parent().ok_or("invalid document path")?)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(document.with_extension("lock"))?;
        lock.try_lock()
            .map_err(|e| format!("another DriftInk process owns this document: {e}"))?;
        let path = socket_path()?;
        let parent = path.parent().ok_or("invalid control socket path")?;
        fs::create_dir_all(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        if path.exists() {
            if UnixStream::connect(&path).is_ok() {
                return Err("DriftInk is already running in this session".into());
            }
            fs::remove_file(&path)?;
        }
        let listener = UnixListener::bind(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        Ok(Self {
            document,
            listener,
            path,
            _lock: lock,
        })
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
