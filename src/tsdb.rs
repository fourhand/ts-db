use crate::{Result, BucketTime, Value, DEFAULT_SEGMENT, DEFAULT_BUCKET_INTERVAL};
use crate::segment::Segment;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;

/// Main time series database that manages multiple segments
pub struct TsDb {
    segments: HashMap<String, Segment>,
    base_path: String,
    bucket_interval: u64,
}

impl TsDb {
    /// Create a new TsDb instance
    pub fn new(base_path: String) -> Result<Self> {
        Ok(Self {
            segments: HashMap::new(),
            base_path,
            bucket_interval: DEFAULT_BUCKET_INTERVAL,
        })
    }
    
    /// Create a new TsDb instance with custom bucket interval
    pub fn with_bucket_interval(base_path: String, bucket_interval: u64) -> Result<Self> {
        Ok(Self {
            segments: HashMap::new(),
            base_path,
            bucket_interval,
        })
    }
    
    /// Get or create a segment
    fn get_or_create_segment(&mut self, segment_name: &str) -> Result<&mut Segment> {
        if !self.segments.contains_key(segment_name) {
            let segment_path = format!("{}/segment_{}", self.base_path, segment_name);
            
            // Create directory if it doesn't exist
            std::fs::create_dir_all(&segment_path)?;
            
            let segment = Segment::new(
                segment_name.to_string(),
                segment_path,
                self.bucket_interval,
            )?;
            self.segments.insert(segment_name.to_string(), segment);
        }
        
        Ok(self.segments.get_mut(segment_name).unwrap())
    }
    
    /// Insert a value for a key at a specific timestamp
    pub fn insert(&mut self, segment: &str, timestamp: u64, key: &str, value: Value) -> Result<()> {
        let segment = self.get_or_create_segment(segment)?;
        segment.insert(timestamp, key, value)
    }
    
    /// Accumulate a value for a key at a specific timestamp
    pub fn accumulate(&mut self, segment: &str, timestamp: u64, key: &str, value: Value) -> Result<()> {
        let segment = self.get_or_create_segment(segment)?;
        segment.accumulate(timestamp, key, value)
    }
    
    /// Get all values at a specific bucket time
    pub fn get_all_at(&self, segment: &str, bucket_time: BucketTime) -> Result<Vec<Value>> {
        let segment = self.segments.get(segment)
            .ok_or_else(|| crate::TsDbError::SegmentNotFound(segment.to_string()))?;
        segment.get_all_at(bucket_time)
    }
    
    /// Get a specific key's value at a bucket time
    pub fn get_key_at(&self, segment: &str, bucket_time: BucketTime, key: &str) -> Result<Value> {
        let segment = self.segments.get(segment)
            .ok_or_else(|| crate::TsDbError::SegmentNotFound(segment.to_string()))?;
        segment.get_key_at(bucket_time, key)
    }
    
    /// Aggregate unique keys in a time range
    pub fn aggr_unique(&self, segment: &str, from: BucketTime, to: BucketTime) -> Result<Vec<Value>> {
        let segment = self.segments.get(segment)
            .ok_or_else(|| crate::TsDbError::SegmentNotFound(segment.to_string()))?;
        segment.aggr_unique(from, to)
    }
    
    /// Aggregate sum of values in a time range
    pub fn aggr_sum(&self, segment: &str, from: BucketTime, to: BucketTime) -> Result<Vec<Value>> {
        let segment = self.segments.get(segment)
            .ok_or_else(|| crate::TsDbError::SegmentNotFound(segment.to_string()))?;
        segment.aggr_sum(from, to)
    }
    
    /// Aggregate unique keys across multiple segments in parallel
    pub fn aggr_unique_parallel(&self, from: BucketTime, to: BucketTime) -> Result<Vec<Value>> {
        let segment_names: Vec<String> = self.segments.keys().cloned().collect();
        
        if segment_names.is_empty() {
            return Ok(vec![]);
        }
        
        // Get results from all segments in parallel
        let results: Vec<Result<Vec<Value>>> = segment_names
            .par_iter()
            .map(|segment_name| {
                self.segments.get(segment_name)
                    .ok_or_else(|| crate::TsDbError::SegmentNotFound(segment_name.clone()))?
                    .aggr_unique(from, to)
            })
            .collect();
        
        // Combine results (OR operation for unique keys)
        let mut combined = vec![0; results[0].as_ref().unwrap().len()];
        for result in results {
            let values = result?;
            for (i, &value) in values.iter().enumerate() {
                if i < combined.len() && value > 0 {
                    combined[i] = 1;
                }
            }
        }
        
        Ok(combined)
    }
    
    /// Aggregate sum across multiple segments in parallel
    pub fn aggr_sum_parallel(&self, from: BucketTime, to: BucketTime) -> Result<Vec<Value>> {
        let segment_names: Vec<String> = self.segments.keys().cloned().collect();
        
        if segment_names.is_empty() {
            return Ok(vec![]);
        }
        
        // Get results from all segments in parallel
        let results: Vec<Result<Vec<Value>>> = segment_names
            .par_iter()
            .map(|segment_name| {
                self.segments.get(segment_name)
                    .ok_or_else(|| crate::TsDbError::SegmentNotFound(segment_name.clone()))?
                    .aggr_sum(from, to)
            })
            .collect();
        
        // Combine results (sum operation)
        let mut combined = vec![0; results[0].as_ref().unwrap().len()];
        for result in results {
            let values = result?;
            for (i, &value) in values.iter().enumerate() {
                if i < combined.len() {
                    combined[i] += value;
                }
            }
        }
        
        Ok(combined)
    }
    
    /// Flush all segments to disk
    pub fn flush(&self) -> Result<()> {
        for segment in self.segments.values() {
            segment.flush()?;
        }
        Ok(())
    }
    
    /// Get segment names
    pub fn segment_names(&self) -> Vec<String> {
        self.segments.keys().cloned().collect()
    }
    
    /// Get segment count
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }
    
    /// Get bucket interval
    pub fn bucket_interval(&self) -> u64 {
        self.bucket_interval
    }
}

// Convenience methods for default segment
impl TsDb {
    /// Insert into default segment
    pub fn insert_default(&mut self, timestamp: u64, key: &str, value: Value) -> Result<()> {
        self.insert(DEFAULT_SEGMENT, timestamp, key, value)
    }
    
    /// Accumulate into default segment
    pub fn accumulate_default(&mut self, timestamp: u64, key: &str, value: Value) -> Result<()> {
        self.accumulate(DEFAULT_SEGMENT, timestamp, key, value)
    }
    
    /// Get all values from default segment
    pub fn get_all_at_default(&self, bucket_time: BucketTime) -> Result<Vec<Value>> {
        self.get_all_at(DEFAULT_SEGMENT, bucket_time)
    }
    
    /// Get key value from default segment
    pub fn get_key_at_default(&self, bucket_time: BucketTime, key: &str) -> Result<Value> {
        self.get_key_at(DEFAULT_SEGMENT, bucket_time, key)
    }
    
    /// Aggregate unique from default segment
    pub fn aggr_unique_default(&self, from: BucketTime, to: BucketTime) -> Result<Vec<Value>> {
        self.aggr_unique(DEFAULT_SEGMENT, from, to)
    }
    
    /// Aggregate sum from default segment
    pub fn aggr_sum_default(&self, from: BucketTime, to: BucketTime) -> Result<Vec<Value>> {
        self.aggr_sum(DEFAULT_SEGMENT, from, to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_tsdb_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_string_lossy().to_string();
        
        let mut tsdb = TsDb::new(base_path).unwrap();
        
        // Test insert
        tsdb.insert_default(1640995200, "key1", 123).unwrap();
        tsdb.insert_default(1640995200, "key2", 456).unwrap();
        
        // Test get_all_at
        let values = tsdb.get_all_at_default(1640995200).unwrap();
        assert_eq!(values[0], 123); // key1
        assert_eq!(values[1], 456); // key2
        
        // Test get_key_at
        assert_eq!(tsdb.get_key_at_default(1640995200, "key1").unwrap(), 123);
        assert_eq!(tsdb.get_key_at_default(1640995200, "key2").unwrap(), 456);
        
        // Test accumulate
        tsdb.accumulate_default(1640995200, "key1", 100).unwrap();
        assert_eq!(tsdb.get_key_at_default(1640995200, "key1").unwrap(), 223);
        
        // Test aggregation
        let unique = tsdb.aggr_unique_default(1640995200, 1640995200).unwrap();
        assert_eq!(unique[0], 1); // key1 present
        assert_eq!(unique[1], 1); // key2 present
        
        let sum = tsdb.aggr_sum_default(1640995200, 1640995200).unwrap();
        assert_eq!(sum[0], 223); // key1 sum
        assert_eq!(sum[1], 456); // key2 sum
    }
    
    #[test]
    fn test_tsdb_multiple_segments() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_string_lossy().to_string();
        
        let mut tsdb = TsDb::new(base_path).unwrap();
        
        // Test multiple segments
        tsdb.insert("segment1", 1640995200, "key1", 100).unwrap();
        tsdb.insert("segment2", 1640995200, "key1", 200).unwrap();
        
        assert_eq!(tsdb.get_key_at("segment1", 1640995200, "key1").unwrap(), 100);
        assert_eq!(tsdb.get_key_at("segment2", 1640995200, "key1").unwrap(), 200);
        
        // Test parallel aggregation
        let sum = tsdb.aggr_sum_parallel(1640995200, 1640995200).unwrap();
        assert_eq!(sum[0], 300); // key1 sum across segments
    }
} 