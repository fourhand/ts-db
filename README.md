# ts-db: High-Performance Time Series Database

A local file-based time series database with mmap-based I/O and parallel processing support. Designed to handle billions of data points efficiently with fixed time intervals.

## Features

- **High Performance**: mmap-based I/O for fast data access
- **Parallel Processing**: Rayon-based parallel aggregation
- **Fixed Time Intervals**: Efficient bucket-based time management
- **Multiple Segments**: Support for logical data separation
- **Local Storage**: File-based storage with automatic directory creation
- **Memory Efficient**: 1MB blocks with 2GB file limits

## Architecture

### Core Components

- **KeyMapManager**: String key ↔ u64 sequence number mapping
- **IndexManager**: BucketTime → file/offset/length mapping
- **ValueBlockManager**: mmap-based storage with 1MB blocks
- **Segment**: Logical data unit combining keymap, index, and storage
- **TsDb**: Main database managing multiple segments

### Data Flow

```
Input: (timestamp, key, value)
→ keymap lookup/creation → bucket time calculation → index lookup → mmap write
```

```
Query: time range
→ bucket time list → index lookup → mmap read → value extraction
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
tsdb = "0.1.0"
```

## Quick Start

```rust
use tsdb::{TsDb, Result};

fn main() -> Result<()> {
    // Create a new database
    let mut db = TsDb::new("/path/to/data".to_string())?;
    
    // Insert data
    db.insert_default(1640995200, "sensor1", 123)?;
    db.insert_default(1640995200, "sensor2", 456)?;
    
    // Query data
    let values = db.get_all_at_default(1640995200)?;
    println!("Values: {:?}", values);
    
    // Aggregate data
    let unique = db.aggr_unique_default(1640995200, 1640995260)?;
    let sum = db.aggr_sum_default(1640995200, 1640995260)?;
    
    // Flush to disk
    db.flush()?;
    
    Ok(())
}
```

## API Reference

### Basic Operations

#### Insert Data
```rust
// Insert a single value
db.insert("segment_name", timestamp, "key", value)?;

// Insert into default segment
db.insert_default(timestamp, "key", value)?;
```

#### Accumulate Data
```rust
// Add to existing value
db.accumulate("segment_name", timestamp, "key", value)?;

// Accumulate in default segment
db.accumulate_default(timestamp, "key", value)?;
```

#### Query Data
```rust
// Get all values at a specific time
let values = db.get_all_at("segment_name", bucket_time)?;

// Get specific key value
let value = db.get_key_at("segment_name", bucket_time, "key")?;

// Default segment queries
let values = db.get_all_at_default(bucket_time)?;
let value = db.get_key_at_default(bucket_time, "key")?;
```

#### Aggregate Data
```rust
// Count unique keys in time range
let unique = db.aggr_unique("segment_name", from_time, to_time)?;

// Sum values in time range
let sum = db.aggr_sum("segment_name", from_time, to_time)?;

// Parallel aggregation across segments
let unique = db.aggr_unique_parallel(from_time, to_time)?;
let sum = db.aggr_sum_parallel(from_time, to_time)?;
```

### Advanced Usage

#### Custom Bucket Interval
```rust
// Create database with 30-second intervals
let mut db = TsDb::with_bucket_interval("/path/to/data".to_string(), 30)?;
```

#### Multiple Segments
```rust
// Create segments for different data types
db.insert("temperature", timestamp, "sensor1", 25)?;
db.insert("humidity", timestamp, "sensor1", 60)?;

// Query specific segment
let temp_values = db.get_all_at("temperature", bucket_time)?;
```

#### Time Conversion
```rust
use tsdb::{timestamp_to_bucket_time, bucket_time_to_timestamp};

let bucket_time = timestamp_to_bucket_time(1640995200, 60);
let timestamp = bucket_time_to_timestamp(bucket_time);
```

## File Structure

```
/data/ts-db/
├── segment_default/
│   ├── keymap.txt          # Key ↔ sequence mapping
│   ├── index.json          # BucketTime → file mapping
│   ├── tsdata_0001.dat     # Data files (2GB limit)
│   └── tsdata_0002.dat
├── segment_temperature/
│   ├── keymap.txt
│   ├── index.json
│   └── tsdata_0001.dat
└── segment_humidity/
    ├── keymap.txt
    ├── index.json
    └── tsdata_0001.dat
```

## Configuration

### Constants

- `DEFAULT_BUCKET_INTERVAL`: 60 seconds
- `VBLOCK_SIZE`: 1MB per block
- `VFILE_SIZE_LIMIT`: 2GB per file
- `DEFAULT_SEGMENT`: "default"

### Error Handling

All operations return `Result<T, TsDbError>` where `TsDbError` includes:

- `Io`: File system errors
- `KeyNotFound`: Key doesn't exist
- `SegmentNotFound`: Segment doesn't exist
- `InvalidBucketTime`: Invalid time format
- `ValueBlockFull`: Block capacity exceeded
- `FileSizeLimitExceeded`: File size limit reached

## Performance Characteristics

- **Write Performance**: O(1) for single key insertions
- **Read Performance**: O(1) for single key queries
- **Aggregation Performance**: O(n) where n is number of bucket times
- **Memory Usage**: Minimal due to mmap-based storage
- **Disk Usage**: Efficient with 1MB block allocation

## Examples

### Sensor Data Collection
```rust
use tsdb::TsDb;

fn collect_sensor_data() -> Result<()> {
    let mut db = TsDb::new("/data/sensors".to_string())?;
    
    // Collect temperature data
    db.insert("temperature", 1640995200, "sensor1", 25)?;
    db.insert("temperature", 1640995200, "sensor2", 26)?;
    
    // Collect humidity data
    db.insert("humidity", 1640995200, "sensor1", 60)?;
    db.insert("humidity", 1640995200, "sensor2", 65)?;
    
    // Aggregate hourly data
    let hourly_temp = db.aggr_sum("temperature", 1640995200, 1640998800)?;
    let hourly_humidity = db.aggr_sum("humidity", 1640995200, 1640998800)?;
    
    db.flush()?;
    Ok(())
}
```

### Log Analysis
```rust
use tsdb::TsDb;

fn analyze_logs() -> Result<()> {
    let mut db = TsDb::new("/data/logs".to_string())?;
    
    // Count log entries by type
    db.accumulate_default(1640995200, "error", 1)?;
    db.accumulate_default(1640995200, "warning", 3)?;
    db.accumulate_default(1640995200, "info", 10)?;
    
    // Get hourly statistics
    let error_count = db.get_key_at_default(1640995200, "error")?;
    let warning_count = db.get_key_at_default(1640995200, "warning")?;
    
    println!("Errors: {}, Warnings: {}", error_count, warning_count);
    
    Ok(())
}
```

## Testing

Run the test suite:

```bash
cargo test
```

## License

MIT License - see LICENSE file for details.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Run `cargo test`
6. Submit a pull request

## Roadmap

- [ ] Compression support (LZ4)
- [ ] Data retention policies
- [ ] Snapshot and recovery
- [ ] CLI management tools
- [ ] Web API interface
- [ ] Metrics and monitoring 