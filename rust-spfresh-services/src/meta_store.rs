use anyhow::{Result, anyhow};
use serde::{Serialize, Deserialize};
use std::{fs::{File, OpenOptions}, io::{BufRead, BufReader, Write}, path::PathBuf};

#[derive(Clone)]
pub struct MetaStore {
    path: PathBuf,
}

impl MetaStore {
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        let p = dir.join("reviews.jsonl");
        if !p.exists() { File::create(&p)?; }
        Ok(Self { path: p })
    }

    pub fn append_line<T: Serialize>(&self, row: &T) -> Result<()> {
        let mut f = OpenOptions::new().append(true).open(&self.path)?;
        let line = serde_json::to_string(row)?;
        f.write_all(line.as_bytes())?;
        f.write_all(b"\n")?;
        Ok(())
    }

    pub fn read_line<T: for<'de> Deserialize<'de>>(&self, id: usize) -> Result<T> {
        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        let line = reader.lines().nth(id)
            .ok_or_else(|| anyhow!("metadata line not found"))??;
        let v = serde_json::from_str(&line)?;
        Ok(v)
    }
}
