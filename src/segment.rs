use crate::{Result, BucketTime, Value, Seq};
use crate::keymap::KeyMapManager;
use crate::index::{IndexManager, IndexEntry};
use crate::storage::ValueBlockManager;
use std::path::Path;

/// A segment represents a logical data unit with its own keymap, index, and storage
pub struct Segment {
    name: String,
    keymap: KeyMapManager,
    index: IndexManager,
    storage: ValueBlockManager,
    bucket_interval: u64,
}

impl Segment {
    /// Create a new segment
    pub fn new(name: String, base_path: String, bucket_interval: u64) -> Result<Self> {
        let keymap_path = format!("{}/keymap.txt", base_path);
        let index_path = format!("{}/index.json", base_path);
        let storage_path = base_path;
        
        let keymap = KeyMapManager::new(keymap_path)?;
        let index = IndexManager::new(index_path)?;
        let storage = ValueBlockManager::new(storage_path)?;
        
        Ok(Self {
            name,
            keymap,
            index,
            storage,
            bucket_interval,
        })
    }
    
    /// Insert a value for a key at a specific timestamp
    pub fn insert(&mut self, timestamp: u64, key: &str, value: Value) -> Result<()> {
        let bucket_time = crate::timestamp_to_bucket_time(timestamp, self.bucket_interval);
        let seq = self.keymap.get_or_create_seq(key)?;
        
        // Check if we need to create a new index entry
        if !self.index.has_entry(bucket_time) {
            let entry = IndexEntry {
                file_path: format!("tsdata_{:04}.dat", self.storage.file_count()),
                offset: 0,
                length: self.keymap.len() * std::mem::size_of::<Value>(),
            };
            self.index.set_entry(bucket_time, entry)?;
        }
        
        // Write the value
        self.storage.write_value(seq as usize, value)?;
        
        Ok(())
    }
    
    /// Accumulate a value for a key at a specific timestamp (add to existing value)
    pub fn accumulate(&mut self, timestamp: u64, key: &str, value: Value) -> Result<()> {
        let bucket_time = crate::timestamp_to_bucket_time(timestamp, self.bucket_interval);
        let seq = self.keymap.get_or_create_seq(key)?;
        
        // Get existing value (default to 0 if not found)
        let existing_value = self.storage.read_value(seq as usize).unwrap_or(0);
        let new_value = existing_value + value;
        
        // Check if we need to create a new index entry
        if !self.index.has_entry(bucket_time) {
            let entry = IndexEntry {
                file_path: format!("tsdata_{:04}.dat", self.storage.file_count()),
                offset: 0,
                length: self.keymap.len() * std::mem::size_of::<Value>(),
            };
            self.index.set_entry(bucket_time, entry)?;
        }
        
        // Write the accumulated value
        self.storage.write_value(seq as usize, new_value)?;
        
        Ok(())
    }
    
    /// Get all values at a specific bucket time
    pub fn get_all_at(&self, bucket_time: BucketTime) -> Result<Vec<Value>> {
        let mut values = vec![0; self.keymap.len()];
        
        for seq in 0..self.keymap.len() {
            if let Ok(value) = self.storage.read_value(seq as usize) {
                values[seq] = value;
            }
        }
        
        Ok(values)
    }
    
    /// Get a specific key's value at a bucket time
    pub fn get_key_at(&self, bucket_time: BucketTime, key: &str) -> Result<Value> {
        let seq = self.keymap.get_seq(key)?;
        self.storage.read_value(seq as usize)
    }
    
    /// Aggregate unique keys in a time range
    pub fn aggr_unique(&self, from: BucketTime, to: BucketTime) -> Result<Vec<Value>> {
        let mut result = vec![0; self.keymap.len()];
        
        // Get all bucket times in range
        let bucket_times = self.index.get_entries_in_range(from, to);
        
        for (_, _) in bucket_times {
            // For each bucket time, check which keys have values > 0
            for seq in 0..self.keymap.len() {
                if let Ok(value) = self.storage.read_value(seq) {
                    if value > 0 {
                        result[seq] = 1; // Mark as present
                    }
                }
            }
        }
        
        Ok(result)
    }
    
    /// Aggregate sum of values in a time range
    pub fn aggr_sum(&self, from: BucketTime, to: BucketTime) -> Result<Vec<Value>> {
        let mut result = vec![0; self.keymap.len()];
        
        // Get all bucket times in range
        let bucket_times = self.index.get_entries_in_range(from, to);
        
        for (_, _) in bucket_times {
            // For each bucket time, sum up values
            for seq in 0..self.keymap.len() {
                if let Ok(value) = self.storage.read_value(seq) {
                    result[seq] += value;
                }
            }
        }
        
        Ok(result)
    }
    
    /// Flush all data to disk
    pub fn flush(&self) -> Result<()> {
        self.keymap.flush()?;
        self.index.flush()?;
        self.storage.flush()?;
        Ok(())
    }
    
    /// Get segment name
    pub fn name(&self) -> &str {
        &self.name
    }
    
    /// Get keymap length
    pub fn keymap_len(&self) -> usize {
        self.keymap.len()
    }
    
    /// Get bucket interval
    pub fn bucket_interval(&self) -> u64 {
        self.bucket_interval
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_segment_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_string_lossy().to_string();
        
        let mut segment = Segment::new("test".to_string(), base_path, 60).unwrap();
        
        // Test insert
        segment.insert(1640995200, "key1", 123).unwrap();
        segment.insert(1640995200, "key2", 456).unwrap();
        
        // Test get_all_at
        let values = segment.get_all_at(1640995200).unwrap();
        assert_eq!(values[0], 123); // key1
        assert_eq!(values[1], 456); // key2
        
        // Test get_key_at
        assert_eq!(segment.get_key_at(1640995200, "key1").unwrap(), 123);
        assert_eq!(segment.get_key_at(1640995200, "key2").unwrap(), 456);
        
        // Test accumulate
        segment.accumulate(1640995200, "key1", 100).unwrap();
        assert_eq!(segment.get_key_at(1640995200, "key1").unwrap(), 223);
        
        // Test aggregation
        let unique = segment.aggr_unique(1640995200, 1640995200).unwrap();
        assert_eq!(unique[0], 1); // key1 present
        assert_eq!(unique[1], 1); // key2 present
        
        let sum = segment.aggr_sum(1640995200, 1640995200).unwrap();
        assert_eq!(sum[0], 223); // key1 sum
        assert_eq!(sum[1], 456); // key2 sum
    }
} 