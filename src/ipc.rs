use serde::Deserialize;
use std::{
    env,
    error::Error,
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
};

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Output {
    pub name: String,
    #[serde(default)]
    pub active: bool,
    pub camera: [f64; 2],
    pub zoom: f64,
    pub size: [u32; 2],
}
#[derive(Clone, Debug, Deserialize)]
pub struct State {
    pub outputs: Vec<Output>,
    #[serde(default)]
    pub fullscreen: Vec<Fullscreen>,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fullscreen {
    pub output: String,
}

pub fn socket_path() -> Result<PathBuf> {
    if let Some(path) = env::var_os("DRIFTWM_SOCKET") {
        return Ok(path.into());
    }
    let runtime = env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR is unset")?;
    let display = env::var("WAYLAND_DISPLAY")?;
    if display.contains('/') {
        return Err("set DRIFTWM_SOCKET for an absolute WAYLAND_DISPLAY".into());
    }
    Ok(PathBuf::from(runtime)
        .join("driftwm")
        .join(format!("ipc-{display}.sock")))
}
pub fn connect(request: &str) -> Result<BufReader<UnixStream>> {
    let mut stream = UnixStream::connect(socket_path()?)?;
    writeln!(stream, "{request}")?;
    Ok(BufReader::new(stream))
}
pub fn parse(line: &str) -> Result<Option<State>> {
    let value: serde_json::Value = serde_json::from_str(line)?;
    if let Some(error) = value.get("Err") {
        return Err(format!("driftwm: {error}").into());
    }
    let state = value
        .get("State")
        .or_else(|| value.get("Ok").and_then(|v| v.get("State")));
    let state: Option<State> = state
        .map(|v| serde_json::from_value(v.clone()))
        .transpose()?;
    if let Some(state) = &state {
        for o in &state.outputs {
            if !o.zoom.is_finite()
                || o.zoom <= 0.0
                || !o.camera.iter().all(|v| v.is_finite())
                || o.size.contains(&0)
            {
                return Err("invalid output camera or size".into());
            }
        }
    }
    Ok(state)
}
pub fn snapshot() -> Result<State> {
    let mut line = String::new();
    connect("\"State\"")?.read_line(&mut line)?;
    parse(&line)?.ok_or_else(|| "missing State reply".into())
}
/// Coalesced writes on a persistent command socket; rendering never waits for replies.
pub struct Commander(
    std::sync::Arc<(
        std::sync::Mutex<Option<serde_json::Value>>,
        std::sync::Condvar,
    )>,
);
impl Default for Commander {
    fn default() -> Self {
        Self::new()
    }
}
impl Commander {
    pub fn new() -> Self {
        use std::sync::{Arc, Condvar, Mutex};
        let shared = Arc::new((Mutex::new(None::<serde_json::Value>), Condvar::new()));
        let worker = shared.clone();
        std::thread::spawn(move || {
            let mut connection: Option<BufReader<UnixStream>> = None;
            loop {
                let command = {
                    let (lock, wake) = &*worker;
                    let Ok(mut slot) = lock.lock() else {
                        break;
                    };
                    while slot.is_none() {
                        let Ok(next) = wake.wait(slot) else {
                            return;
                        };
                        slot = next;
                    }
                    slot.take().expect("command is present")
                };
                let result = (|| -> Result<()> {
                    if connection.is_none() {
                        let mut reader = connect("\"State\"")?;
                        let mut ack = String::new();
                        reader.read_line(&mut ack)?;
                        connection = Some(reader);
                    }
                    let reader = connection.as_mut().ok_or("no command connection")?;
                    reader
                        .get_mut()
                        .set_write_timeout(Some(std::time::Duration::from_secs(1)))?;
                    reader
                        .get_mut()
                        .set_read_timeout(Some(std::time::Duration::from_secs(1)))?;
                    writeln!(reader.get_mut(), "{command}")?;
                    let mut reply = String::new();
                    reader.read_line(&mut reply)?;
                    if reply.is_empty() {
                        return Err("command socket disconnected".into());
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    eprintln!("driftink navigation: {error}");
                    connection = None;
                }
            }
        });
        Self(shared)
    }
    pub fn send(&self, value: serde_json::Value) {
        let (slot, wake) = &*self.0;
        if let Ok(mut slot) = slot.lock() {
            *slot = Some(value);
            wake.notify_one();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replies_and_events() {
        for line in [
            r#"{"Ok":{"State":{"outputs":[]}}}"#,
            r#"{"State":{"outputs":[]}}"#,
        ] {
            assert!(parse(line).unwrap().is_some());
        }
        assert!(parse(r#"{"Ok":"Ok"}"#).unwrap().is_none());
        assert!(parse(r#"{"Err":"denied"}"#).is_err());
        assert!(
            parse(r#"{"State":{"outputs":[{"name":"x","camera":[0,0],"zoom":0,"size":[1,1]}]}}"#)
                .is_err()
        );
    }
}
