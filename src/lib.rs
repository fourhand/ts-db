//! ts-db: High-performance time series database
//! 
//! A local file-based time series database with mmap-based I/O and parallel processing support.
//! Designed to handle billions of data points efficiently with fixed time intervals.

pub mod error;
pub mod keymap;
pub mod index;
pub mod storage;
pub mod segment;
pub mod tsdb;

pub use error::{TsDbError, Result};
pub use tsdb::TsDb;

// Re-export main types
pub use keymap::KeyMapManager;
pub use index::IndexManager;
pub use storage::{ValueBlockManager, VFile, VBlock};
pub use segment::Segment;

/// BucketTime represents a time bucket identifier
/// Format: YYYYMMDDHHMM (e.g., 202506161305)
pub type BucketTime = u64;

/// Sequence number for key mapping
pub type Seq = u64;

/// Value type for time series data
pub type Value = u64;

/// Default bucket interval in seconds
pub const DEFAULT_BUCKET_INTERVAL: u64 = 60; // 1 minute

/// Default VBlock size in bytes (1MB)
pub const VBLOCK_SIZE: usize = 1024 * 1024;

/// Default VFile size limit in bytes (2GB)
pub const VFILE_SIZE_LIMIT: usize = 2 * 1024 * 1024 * 1024;

/// Default segment name
pub const DEFAULT_SEGMENT: &str = "default";

/// Convert timestamp to BucketTime
pub fn timestamp_to_bucket_time(timestamp: u64, bucket_interval: u64) -> BucketTime {
    (timestamp / bucket_interval) * bucket_interval
}

/// Convert BucketTime to timestamp
pub fn bucket_time_to_timestamp(bucket_time: BucketTime) -> u64 {
    bucket_time
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_conversion() {
        let timestamp = 1640995200; // 2022-01-01 00:00:00 UTC
        let bucket_time = timestamp_to_bucket_time(timestamp, 60);
        assert_eq!(bucket_time, 1640995200);
        
        let bucket_time = timestamp_to_bucket_time(timestamp + 30, 60);
        assert_eq!(bucket_time, 1640995200);
    }
}
