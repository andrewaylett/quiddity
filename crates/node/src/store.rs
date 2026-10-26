//! A graph on disk: a directory holding the graph's snapshot and the
//! device's secret key.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use iroh::SecretKey;
use quiddity_core::Graph;

const SNAPSHOT: &str = "graph.loro";
const KEY: &str = "device.key";

#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Creates the directory with an empty graph (or, with `blank`, an empty
    /// replica for a device that will sync an existing graph).
    pub fn init(&self, blank: bool) -> Result<Graph, crate::StoreError> {
        if self.dir.join(SNAPSHOT).exists() {
            return Err(crate::StoreError::Exists(self.dir.clone()));
        }
        fs::create_dir_all(&self.dir)?;
        let graph = if blank { Graph::blank() } else { Graph::new() };
        self.save(&graph)?;
        Ok(graph)
    }

    pub fn load(&self) -> Result<Graph, crate::StoreError> {
        let bytes = fs::read(self.dir.join(SNAPSHOT))?;
        Ok(Graph::from_snapshot(&bytes)?)
    }

    /// Saves atomically: a crash mid-save leaves the previous snapshot.
    pub fn save(&self, graph: &Graph) -> Result<(), crate::StoreError> {
        let tmp = self.dir.join(format!("{SNAPSHOT}.tmp"));
        fs::write(&tmp, graph.snapshot()?)?;
        fs::rename(tmp, self.dir.join(SNAPSHOT))?;
        Ok(())
    }

    /// The device's key, created on first use. Its public half is the
    /// device's ID.
    pub fn key(&self) -> io::Result<SecretKey> {
        let path = self.dir.join(KEY);
        match fs::read(&path) {
            Ok(bytes) => {
                let bytes: [u8; 32] = bytes
                    .try_into()
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad device key"))?;
                Ok(SecretKey::from_bytes(&bytes))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let key = SecretKey::generate();
                let mut options = fs::OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
                io::Write::write_all(&mut options.open(&path)?, &key.to_bytes())?;
                Ok(key)
            }
            Err(e) => Err(e),
        }
    }
}
