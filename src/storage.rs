//! Versioned JSON with validation and same-directory durable atomic replacement.
use crate::{
    canvas::{Object, Scene},
    ipc::Result,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub schema_version: u32,
    pub document_id: String,
    pub objects: Vec<Object>,
}
pub fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}
pub fn data_path() -> Result<PathBuf> {
    let base = match env::var_os("XDG_DATA_HOME") {
        Some(p) => PathBuf::from(p),
        None => PathBuf::from(env::var_os("HOME").ok_or("HOME is unset")?).join(".local/share"),
    };
    if !base.is_absolute() {
        return Err("XDG_DATA_HOME must be absolute".into());
    }
    Ok(base.join("driftink/canvas.json"))
}
impl Document {
    pub fn empty() -> Self {
        Self {
            schema_version: 1,
            document_id: format!("driftink-{}-{}", timestamp(), std::process::id()),
            objects: vec![],
        }
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1 {
            return Err(format!("unsupported schema version {}", self.schema_version).into());
        }
        if self.document_id.is_empty()
            || self.document_id.len() > 256
            || self.objects.len() > 100_000
        {
            return Err("invalid document metadata or too many objects".into());
        }
        let mut ids = HashSet::new();
        let mut points = 0;
        for o in &self.objects {
            points += o.points.len();
            if !o.validate() || !ids.insert(o.id) || points > 2_000_000 {
                return Err(
                    "invalid geometry, duplicate IDs, or document exceeds point limit".into(),
                );
            }
        }
        Ok(())
    }
    pub fn scene(&self) -> Scene {
        let mut s = Scene::default();
        for o in &self.objects {
            s.insert(o.clone());
        }
        s
    }
}
pub fn load(path: &Path) -> Result<Document> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Document::empty()),
        Err(e) => return Err(e.into()),
    };
    if file.metadata()?.len() > 128 * 1024 * 1024 {
        return Err("document exceeds 128 MiB".into());
    }
    let mut bytes = Vec::new();
    file.take(128 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    let document: Document = serde_json::from_slice(&bytes)?;
    document.validate()?;
    Ok(document)
}
pub fn save(path: &Path, document: &Document) -> Result<()> {
    document.validate()?;
    let parent = path.parent().ok_or("document path has no parent")?;
    fs::create_dir_all(parent)?;
    let temp = path.with_extension(format!("json.{}.pending", std::process::id()));
    let bytes = serde_json::to_vec(document)?;
    // A failed write leaves the previous document intact and the pending file recoverable.
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temp)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::rename(&temp, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_and_reject_corruption_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("canvas.json");
        let d = Document::empty();
        save(&path, &d).unwrap();
        assert_eq!(load(&path).unwrap().document_id, d.document_id);
        let unsupported = Document {
            schema_version: 99,
            ..Document::empty()
        };
        assert!(save(&path, &unsupported).is_err());
        assert_eq!(load(&path).unwrap().document_id, d.document_id);
        fs::write(&path, b"broken").unwrap();
        assert!(load(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"broken");
    }
}
