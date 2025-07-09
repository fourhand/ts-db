use thiserror::Error;

/// Custom error type for ts-db operations
#[derive(Error, Debug)]
pub enum TsDbError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    
    #[error("Segment not found: {0}")]
    SegmentNotFound(String),
    
    #[error("Invalid bucket time: {0}")]
    InvalidBucketTime(u64),
    
    #[error("Invalid sequence number: {0}")]
    InvalidSeq(u64),
    
    #[error("Storage error: {0}")]
    Storage(String),
    
    #[error("Index error: {0}")]
    Index(String),
    
    #[error("KeyMap error: {0}")]
    KeyMap(String),
    
    #[error("Value block is full")]
    ValueBlockFull,
    
    #[error("File size limit exceeded")]
    FileSizeLimitExceeded,
    
    #[error("Invalid data format: {0}")]
    InvalidDataFormat(String),
}

/// Result type for ts-db operations
pub type Result<T> = std::result::Result<T, TsDbError>; 