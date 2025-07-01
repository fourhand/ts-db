use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use memmap2::{MmapMut, MmapOptions};
use serde::{Deserialize, Serialize};

pub type Seq = u64;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexEntry {
    pub file: String,
    pub offset: u64,
    pub length: u64,
}

#[derive(Default)]
pub struct KeyMapManager {
    path: PathBuf,
    map: HashMap<String, Seq>,
    next_seq: Seq,
}

impl KeyMapManager {
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut manager = KeyMapManager {
            path: path.clone(),
            map: HashMap::new(),
            next_seq: 0,
        };
        if path.exists() {
            manager.load()?;
        }
        Ok(manager)
    }

    fn load(&mut self) -> io::Result<()> {
        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = line?;
            let mut parts = line.split(',');
            if let (Some(key), Some(seq)) = (parts.next(), parts.next()) {
                if let Ok(seq) = seq.parse::<Seq>() {
                    self.map.insert(key.to_string(), seq);
                    self.next_seq = self.next_seq.max(seq + 1);
                }
            }
        }
        Ok(())
    }

    pub fn get_or_insert_seq(&mut self, key: &str) -> io::Result<Seq> {
        if let Some(&seq) = self.map.get(key) {
            return Ok(seq);
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        self.map.insert(key.to_string(), seq);
        let mut file = OpenOptions::new().create(true).append(true).open(&self.path)?;
        writeln!(file, "{},{}", key, seq)?;
        Ok(seq)
    }
}

#[derive(Default)]
pub struct IndexManager {
    path: PathBuf,
    map: HashMap<String, Vec<IndexEntry>>, // bucket_time -> entries
}

impl IndexManager {
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut manager = IndexManager {
            path: path.clone(),
            map: HashMap::new(),
        };
        if path.exists() {
            manager.load()?;
        }
        Ok(manager)
    }

    fn load(&mut self) -> io::Result<()> {
        let data = fs::read(&self.path)?;
        self.map = serde_json::from_slice(&data)?;
        Ok(())
    }

    fn flush(&self) -> io::Result<()> {
        let data = serde_json::to_vec_pretty(&self.map)?;
        fs::write(&self.path, data)?;
        Ok(())
    }

    pub fn add_entry(&mut self, bucket: &str, entry: IndexEntry) -> io::Result<()> {
        self.map.entry(bucket.to_string()).or_default().push(entry);
        self.flush()
    }

    pub fn entries(&self, bucket: &str) -> Option<&Vec<IndexEntry>> {
        self.map.get(bucket)
    }
}

pub struct ValueBlockManager {
    dir: PathBuf,
    current_file: Option<File>,
    mmap: Option<MmapMut>,
    current_offset: usize,
    max_file_size: usize,
}

impl ValueBlockManager {
    pub fn new<P: AsRef<Path>>(dir: P, max_file_size: usize) -> io::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;
        Ok(ValueBlockManager {
            dir,
            current_file: None,
            mmap: None,
            current_offset: 0,
            max_file_size,
        })
    }

    fn ensure_file(&mut self) -> io::Result<()> {
        if self.current_file.is_none() {
            let file_path = self.dir.join("data_0001.dat");
            let file = OpenOptions::new().create(true).append(true).read(true).write(true).open(&file_path)?;
            file.set_len(self.max_file_size as u64)?;
            let mmap = unsafe { MmapOptions::new().map_mut(&file)? };
            self.current_file = Some(file);
            self.mmap = Some(mmap);
        }
        Ok(())
    }

    pub fn write_value(&mut self, offset: usize, value: u64) -> io::Result<()> {
        self.ensure_file()?;
        if let Some(ref mut mmap) = self.mmap {
            let bytes = value.to_le_bytes();
            let start = offset * 8;
            let end = start + 8;
            mmap[start..end].copy_from_slice(&bytes);
            mmap.flush_range(start, 8)?;
        }
        Ok(())
    }
}

pub struct Segment {
    name: String,
    base_path: PathBuf,
    keymap: KeyMapManager,
    index: IndexManager,
    storage: ValueBlockManager,
}

impl Segment {
    pub fn new<P: AsRef<Path>>(name: &str, base: P) -> io::Result<Self> {
        let base_path = base.as_ref().join(name);
        fs::create_dir_all(&base_path)?;
        let keymap = KeyMapManager::new(base_path.join("keymap.txt"))?;
        let index = IndexManager::new(base_path.join("index.json"))?;
        let storage = ValueBlockManager::new(&base_path, 2 * 1024 * 1024 * 1024)?; // 2GB
        Ok(Segment {
            name: name.to_string(),
            base_path,
            keymap,
            index,
            storage,
        })
    }

    pub fn insert(&mut self, timestamp: DateTime<Utc>, key: &str, value: u64) -> io::Result<()> {
        let seq = self.keymap.get_or_insert_seq(key)?;
        let bucket = timestamp.format("%Y%m%d%H").to_string();
        let offset = seq as usize;
        self.storage.write_value(offset, value)?;
        let entry = IndexEntry {
            file: "data_0001.dat".to_string(),
            offset: offset as u64 * 8,
            length: 8,
        };
        self.index.add_entry(&bucket, entry)?;
        Ok(())
    }
}

pub struct TsDb {
    segments: HashMap<String, Segment>,
    base_path: PathBuf,
}

impl TsDb {
    pub fn new<P: AsRef<Path>>(base: P) -> io::Result<Self> {
        let base_path = base.as_ref().to_path_buf();
        fs::create_dir_all(&base_path)?;
        Ok(TsDb {
            segments: HashMap::new(),
            base_path,
        })
    }

    pub fn get_segment(&mut self, name: &str) -> io::Result<&mut Segment> {
        if !self.segments.contains_key(name) {
            let seg = Segment::new(name, &self.base_path)?;
            self.segments.insert(name.to_string(), seg);
        }
        Ok(self.segments.get_mut(name).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn simple_insert() {
        let dir = TempDir::new().unwrap();
        let mut db = TsDb::new(dir.path()).unwrap();
        let segment = db.get_segment("default").unwrap();
        let ts = Utc::now();
        segment.insert(ts, "key", 42).unwrap();
        // check keymap file exists
        assert!(segment.base_path.join("keymap.txt").exists());
    }
}

