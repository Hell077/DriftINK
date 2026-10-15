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
