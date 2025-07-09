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
    
    #[test]
    fn test_segment_large_key_count() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_string_lossy().to_string();
        
        let mut segment = Segment::new("large_keys".to_string(), base_path, 60).unwrap();
        
        // Insert many keys to test keymap scaling
        let num_keys = 10000;
        for i in 0..num_keys {
            let key = format!("key_{}", i);
            segment.insert(1640995200, &key, i as u64).unwrap();
        }
        
        // Verify all keys can be retrieved
        for i in 0..num_keys {
            let key = format!("key_{}", i);
            assert_eq!(segment.get_key_at(1640995200, &key).unwrap(), i as u64);
        }
        
        // Test that keymap size is correct
        assert_eq!(segment.keymap_len(), num_keys);
        
        // Test aggregation with many keys
        let unique = segment.aggr_unique(1640995200, 1640995200).unwrap();
        assert_eq!(unique.len(), num_keys);
        
        // Count how many keys have values > 0
        let active_keys = unique.iter().filter(|&&x| x > 0).count();
        assert_eq!(active_keys, num_keys);
    }
    
    #[test]
    fn test_segment_multiple_bucket_times() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_string_lossy().to_string();
        
        let mut segment = Segment::new("multi_time".to_string(), base_path, 60).unwrap();
        
        // Insert data across multiple bucket times
        let bucket_times = vec![1640995200, 1640995260, 1640995320, 1640995380];
        let keys = vec!["key1", "key2", "key3"];
        
        for (i, &bucket_time) in bucket_times.iter().enumerate() {
            for (j, &key) in keys.iter().enumerate() {
                let value = (i * 100) + j;
                segment.insert(bucket_time, key, value as u64).unwrap();
            }
        }
        
        // Verify data for each bucket time
        for (i, &bucket_time) in bucket_times.iter().enumerate() {
            let values = segment.get_all_at(bucket_time).unwrap();
            for (j, &key) in keys.iter().enumerate() {
                let expected_value = (i * 100) + j;
                assert_eq!(segment.get_key_at(bucket_time, key).unwrap(), expected_value as u64);
            }
        }
        
        // Test aggregation across multiple bucket times
        let unique = segment.aggr_unique(1640995200, 1640995380).unwrap();
        let sum = segment.aggr_sum(1640995200, 1640995380).unwrap();
        
        // All keys should be present across the time range
        assert_eq!(unique.iter().filter(|&&x| x > 0).count(), keys.len());
        
        // Sum should be the sum of all values across all bucket times
        let expected_sum: u64 = bucket_times.iter().enumerate()
            .map(|(i, _)| keys.iter().enumerate()
                .map(|(j, _)| (i * 100 + j) as u64)
                .sum::<u64>())
            .sum();
        
        let actual_sum: u64 = sum.iter().sum();
        assert_eq!(actual_sum, expected_sum);
    }
    
    #[test]
    fn test_segment_accumulate_overflow() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_string_lossy().to_string();
        
        let mut segment = Segment::new("accumulate_test".to_string(), base_path, 60).unwrap();
        
        // Test accumulate with large values
        let large_value = u64::MAX / 2;
        
        segment.insert(1640995200, "key1", large_value).unwrap();
        segment.accumulate(1640995200, "key1", large_value).unwrap();
        
        // Should handle overflow gracefully (wraps around)
        let result = segment.get_key_at(1640995200, "key1").unwrap();
        assert_eq!(result, large_value.wrapping_add(large_value));
        
        // Test multiple accumulates
        for i in 0..10 {
            segment.accumulate(1640995200, "key2", i as u64).unwrap();
        }
        
        // Expected sum: 0 + 1 + 2 + ... + 9 = 45
        let expected_sum = (0..10).sum::<u64>();
        assert_eq!(segment.get_key_at(1640995200, "key2").unwrap(), expected_sum);
    }
    
    #[test]
    fn test_segment_edge_cases() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_string_lossy().to_string();
        
        let mut segment = Segment::new("edge_cases".to_string(), base_path, 60).unwrap();
        
        // Test with zero values
        segment.insert(1640995200, "zero_key", 0).unwrap();
        assert_eq!(segment.get_key_at(1640995200, "zero_key").unwrap(), 0);
        
        // Test with maximum u64 value
        segment.insert(1640995200, "max_key", u64::MAX).unwrap();
        assert_eq!(segment.get_key_at(1640995200, "max_key").unwrap(), u64::MAX);
        
        // Test with very long key names
        let long_key = "a".repeat(1000);
        segment.insert(1640995200, &long_key, 999).unwrap();
        assert_eq!(segment.get_key_at(1640995200, &long_key).unwrap(), 999);
        
        // Test with special characters in key names
        let special_key = "key@#$%^&*()_+-=[]{}|;':\",./<>?";
        segment.insert(1640995200, special_key, 888).unwrap();
        assert_eq!(segment.get_key_at(1640995200, special_key).unwrap(), 888);
    }
    
    #[test]
    fn test_segment_concurrent_access_patterns() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_string_lossy().to_string();
        
        let mut segment = Segment::new("concurrent".to_string(), base_path, 60).unwrap();
        
        // Simulate concurrent-like access patterns
        let patterns = vec![
            (0, "key1", 100),
            (1000, "key2", 200),
            (10000, "key3", 300),
            (100000, "key4", 400),
        ];
        
        // Insert in scattered pattern
        for (seq, key, value) in &patterns {
            segment.insert(1640995200, key, *value).unwrap();
        }
        
        // Verify all values
        for (seq, key, expected_value) in &patterns {
            assert_eq!(segment.get_key_at(1640995200, key).unwrap(), *expected_value);
        }
        
        // Test aggregation with scattered data
        let unique = segment.aggr_unique(1640995200, 1640995200).unwrap();
        let active_keys = unique.iter().filter(|&&x| x > 0).count();
        assert_eq!(active_keys, patterns.len());
    }
    
    #[test]
    fn test_segment_flush_operations() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_string_lossy().to_string();
        
        let mut segment = Segment::new("flush_test".to_string(), base_path, 60).unwrap();
        
        // Insert some data
        segment.insert(1640995200, "key1", 123).unwrap();
        segment.insert(1640995200, "key2", 456).unwrap();
        segment.accumulate(1640995200, "key3", 789).unwrap();
        
        // Test flush
        segment.flush().unwrap();
        
        // Verify data integrity after flush
        assert_eq!(segment.get_key_at(1640995200, "key1").unwrap(), 123);
        assert_eq!(segment.get_key_at(1640995200, "key2").unwrap(), 456);
        assert_eq!(segment.get_key_at(1640995200, "key3").unwrap(), 789);
        
        // Test aggregation after flush
        let unique = segment.aggr_unique(1640995200, 1640995200).unwrap();
        let active_keys = unique.iter().filter(|&&x| x > 0).count();
        assert_eq!(active_keys, 3);
    }
} 