use crate::{Result, TsDbError, BucketTime};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

/// Index entry mapping BucketTime to file location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    pub file_path: String,
    pub offset: usize,
    pub length: usize,
}

/// Manages index mapping BucketTime to file locations
pub struct IndexManager {
    index: HashMap<BucketTime, IndexEntry>,
    file_path: String,
}

impl IndexManager {
    /// Create a new IndexManager
    pub fn new(file_path: String) -> Result<Self> {
        let mut manager = Self {
            index: HashMap::new(),
            file_path,
        };
        
        manager.load()?;
        Ok(manager)
    }
    
    /// Get index entry for a BucketTime
    pub fn get_entry(&self, bucket_time: BucketTime) -> Option<&IndexEntry> {
        self.index.get(&bucket_time)
    }
    
    /// Set index entry for a BucketTime
    pub fn set_entry(&mut self, bucket_time: BucketTime, entry: IndexEntry) -> Result<()> {
        self.index.insert(bucket_time, entry);
        self.save()?;
        Ok(())
    }
    
    /// Check if BucketTime exists in index
    pub fn has_entry(&self, bucket_time: BucketTime) -> bool {
        self.index.contains_key(&bucket_time)
    }
    
    /// Get all BucketTimes in sorted order
    pub fn get_all_bucket_times(&self) -> Vec<BucketTime> {
        let mut times: Vec<BucketTime> = self.index.keys().copied().collect();
        times.sort();
        times
    }
    
    /// Get index entries for a time range
    pub fn get_entries_in_range(&self, from: BucketTime, to: BucketTime) -> Vec<(BucketTime, &IndexEntry)> {
        self.index
            .iter()
            .filter(|(&time, _)| time >= from && time <= to)
            .map(|(&time, entry)| (time, entry))
            .collect()
    }
    
    /// Remove index entry for a BucketTime
    pub fn remove_entry(&mut self, bucket_time: BucketTime) -> Result<()> {
        self.index.remove(&bucket_time);
        self.save()?;
        Ok(())
    }
    
    /// Get the total number of entries
    pub fn len(&self) -> usize {
        self.index.len()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }
    
    /// Load index from JSON file
    fn load(&mut self) -> Result<()> {
        let path = Path::new(&self.file_path);
        if !path.exists() {
            return Ok(());
        }
        
        let mut file = File::open(&self.file_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        
        if contents.trim().is_empty() {
            return Ok(());
        }
        
        let index_data: HashMap<String, IndexEntry> = serde_json::from_str(&contents)?;
        
        // Convert string keys back to BucketTime
        for (key_str, entry) in index_data {
            let bucket_time: BucketTime = key_str.parse()
                .map_err(|_| TsDbError::InvalidDataFormat(format!(
                    "Invalid bucket time in index: {}", key_str
                )))?;
            self.index.insert(bucket_time, entry);
        }
        
        Ok(())
    }
    
    /// Save index to JSON file
    fn save(&self) -> Result<()> {
        // Convert BucketTime keys to strings for JSON serialization
        let index_data: HashMap<String, IndexEntry> = self.index
            .iter()
            .map(|(&bucket_time, entry)| (bucket_time.to_string(), entry.clone()))
            .collect();
        
        let json = serde_json::to_string_pretty(&index_data)?;
        
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.file_path)?;
        
        write!(file, "{}", json)?;
        Ok(())
    }
    
    /// Flush changes to disk
    pub fn flush(&self) -> Result<()> {
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_index_basic_operations() {
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_string_lossy().to_string();
        
        let mut index = IndexManager::new(file_path).unwrap();
        
        // Test empty state
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
        
        // Test adding entries
        let entry1 = IndexEntry {
            file_path: "data1.dat".to_string(),
            offset: 0,
            length: 1024,
        };
        
        let entry2 = IndexEntry {
            file_path: "data2.dat".to_string(),
            offset: 1024,
            length: 2048,
        };
        
        index.set_entry(1640995200, entry1.clone()).unwrap();
        index.set_entry(1640995260, entry2.clone()).unwrap();
        
        assert_eq!(index.len(), 2);
        assert!(index.has_entry(1640995200));
        assert!(index.has_entry(1640995260));
        assert!(!index.has_entry(1640995320));
        
        // Test getting entries
        assert_eq!(index.get_entry(1640995200).unwrap().file_path, "data1.dat");
        assert_eq!(index.get_entry(1640995260).unwrap().file_path, "data2.dat");
        assert!(index.get_entry(1640995320).is_none());
        
        // Test range queries
        let range_entries = index.get_entries_in_range(1640995200, 1640995260);
        assert_eq!(range_entries.len(), 2);
        
        // Test removing entries
        index.remove_entry(1640995200).unwrap();
        assert_eq!(index.len(), 1);
        assert!(!index.has_entry(1640995200));
        assert!(index.has_entry(1640995260));
    }
} 