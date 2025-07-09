use crate::{Result, TsDbError, Seq};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// Manages mapping between string keys and sequence numbers
pub struct KeyMapManager {
    key_to_seq: HashMap<String, Seq>,
    seq_to_key: HashMap<Seq, String>,
    next_seq: Seq,
    file_path: String,
}

impl KeyMapManager {
    /// Create a new KeyMapManager
    pub fn new(file_path: String) -> Result<Self> {
        let mut manager = Self {
            key_to_seq: HashMap::new(),
            seq_to_key: HashMap::new(),
            next_seq: 0,
            file_path,
        };
        
        manager.load()?;
        Ok(manager)
    }
    
    /// Get or create sequence number for a key
    pub fn get_or_create_seq(&mut self, key: &str) -> Result<Seq> {
        if let Some(&seq) = self.key_to_seq.get(key) {
            return Ok(seq);
        }
        
        let seq = self.next_seq;
        self.key_to_seq.insert(key.to_string(), seq);
        self.seq_to_key.insert(seq, key.to_string());
        self.next_seq += 1;
        
        // Append to file
        self.append_to_file(key, seq)?;
        
        Ok(seq)
    }
    
    /// Get sequence number for a key (returns error if not found)
    pub fn get_seq(&self, key: &str) -> Result<Seq> {
        self.key_to_seq
            .get(key)
            .copied()
            .ok_or_else(|| TsDbError::KeyNotFound(key.to_string()))
    }
    
    /// Get key for a sequence number
    pub fn get_key(&self, seq: Seq) -> Result<&str> {
        self.seq_to_key
            .get(&seq)
            .map(|s| s.as_str())
            .ok_or_else(|| TsDbError::InvalidSeq(seq))
    }
    
    /// Get the total number of keys
    pub fn len(&self) -> usize {
        self.key_to_seq.len()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.key_to_seq.is_empty()
    }
    
    /// Get all keys as a vector
    pub fn get_all_keys(&self) -> Vec<String> {
        self.key_to_seq.keys().cloned().collect()
    }
    
    /// Load keymap from file
    fn load(&mut self) -> Result<()> {
        let path = Path::new(&self.file_path);
        if !path.exists() {
            return Ok(());
        }
        
        let file = File::open(&self.file_path)?;
        let reader = BufReader::new(file);
        
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() != 2 {
                return Err(TsDbError::InvalidDataFormat(format!(
                    "Invalid keymap line: {}", line
                )));
            }
            
            let key = parts[0].trim();
            let seq: Seq = parts[1].trim().parse()
                .map_err(|_| TsDbError::InvalidDataFormat(format!(
                    "Invalid sequence number: {}", parts[1]
                )))?;
            
            self.key_to_seq.insert(key.to_string(), seq);
            self.seq_to_key.insert(seq, key.to_string());
            
            if seq >= self.next_seq {
                self.next_seq = seq + 1;
            }
        }
        
        Ok(())
    }
    
    /// Append a new key-seq mapping to file
    fn append_to_file(&self, key: &str, seq: Seq) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)?;
        
        writeln!(file, "{},{}", key, seq)?;
        Ok(())
    }
    
    /// Flush changes to disk
    pub fn flush(&self) -> Result<()> {
        // The file is written immediately in append_to_file, so no additional flush needed
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_keymap_basic_operations() {
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_string_lossy().to_string();
        
        let mut keymap = KeyMapManager::new(file_path).unwrap();
        
        // Test get_or_create_seq
        let seq1 = keymap.get_or_create_seq("key1").unwrap();
        let seq2 = keymap.get_or_create_seq("key2").unwrap();
        let seq1_again = keymap.get_or_create_seq("key1").unwrap();
        
        assert_eq!(seq1, 0);
        assert_eq!(seq2, 1);
        assert_eq!(seq1_again, seq1);
        
        // Test get_seq
        assert_eq!(keymap.get_seq("key1").unwrap(), 0);
        assert_eq!(keymap.get_seq("key2").unwrap(), 1);
        assert!(keymap.get_seq("nonexistent").is_err());
        
        // Test get_key
        assert_eq!(keymap.get_key(0).unwrap(), "key1");
        assert_eq!(keymap.get_key(1).unwrap(), "key2");
        assert!(keymap.get_key(999).is_err());
        
        // Test len
        assert_eq!(keymap.len(), 2);
    }
} 