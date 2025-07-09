use tsdb::{TsDb, Result};
use std::time::{SystemTime, UNIX_EPOCH};

fn main() -> Result<()> {
    println!("ts-db: High-Performance Time Series Database Example");
    println!("===================================================");
    
    // Create a new database
    let mut db = TsDb::new("/tmp/tsdb_example".to_string())?;
    
    // Get current timestamp
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    // Round to nearest minute for bucket time
    let bucket_time = (now / 60) * 60;
    
    println!("Current time: {}", now);
    println!("Bucket time: {}", bucket_time);
    
    // Insert some sensor data
    println!("\n1. Inserting sensor data...");
    db.insert_default(bucket_time, "temperature_sensor1", 25)?;
    db.insert_default(bucket_time, "temperature_sensor2", 26)?;
    db.insert_default(bucket_time, "humidity_sensor1", 60)?;
    db.insert_default(bucket_time, "humidity_sensor2", 65)?;
    println!("✓ Data inserted successfully");
    
    // Query all data at this time
    println!("\n2. Querying all data at bucket time {}...", bucket_time);
    let all_values = db.get_all_at_default(bucket_time)?;
    println!("✓ Retrieved {} values", all_values.len());
    
    // Query specific values
    println!("\n3. Querying specific sensor values...");
    let temp1 = db.get_key_at_default(bucket_time, "temperature_sensor1")?;
    let temp2 = db.get_key_at_default(bucket_time, "temperature_sensor2")?;
    let humidity1 = db.get_key_at_default(bucket_time, "humidity_sensor1")?;
    let humidity2 = db.get_key_at_default(bucket_time, "humidity_sensor2")?;
    
    println!("Temperature Sensor 1: {}°C", temp1);
    println!("Temperature Sensor 2: {}°C", temp2);
    println!("Humidity Sensor 1: {}%", humidity1);
    println!("Humidity Sensor 2: {}%", humidity2);
    
    // Accumulate some data
    println!("\n4. Accumulating additional data...");
    db.accumulate_default(bucket_time, "temperature_sensor1", 5)?;
    db.accumulate_default(bucket_time, "error_count", 1)?;
    db.accumulate_default(bucket_time, "error_count", 2)?;
    
    let new_temp1 = db.get_key_at_default(bucket_time, "temperature_sensor1")?;
    let error_count = db.get_key_at_default(bucket_time, "error_count")?;
    
    println!("Updated Temperature Sensor 1: {}°C", new_temp1);
    println!("Total Error Count: {}", error_count);
    
    // Test aggregation
    println!("\n5. Testing aggregation...");
    let unique_keys = db.aggr_unique_default(bucket_time, bucket_time)?;
    let sum_values = db.aggr_sum_default(bucket_time, bucket_time)?;
    
    println!("Unique keys in time range: {}", unique_keys.iter().filter(|&&x| x > 0).count());
    println!("Sum of all values: {}", sum_values.iter().sum::<u64>());
    
    // Test multiple segments
    println!("\n6. Testing multiple segments...");
    db.insert("temperature", bucket_time, "sensor1", 25)?;
    db.insert("temperature", bucket_time, "sensor2", 26)?;
    db.insert("humidity", bucket_time, "sensor1", 60)?;
    db.insert("humidity", bucket_time, "sensor2", 65)?;
    
    let temp_values = db.get_all_at("temperature", bucket_time)?;
    let humidity_values = db.get_all_at("humidity", bucket_time)?;
    
    println!("Temperature segment values: {:?}", temp_values);
    println!("Humidity segment values: {:?}", humidity_values);
    
    // Test parallel aggregation
    println!("\n7. Testing parallel aggregation...");
    let parallel_sum = db.aggr_sum_parallel(bucket_time, bucket_time)?;
    println!("Parallel sum across all segments: {}", parallel_sum.iter().sum::<u64>());
    
    // Flush data to disk
    println!("\n8. Flushing data to disk...");
    db.flush()?;
    println!("✓ Data flushed successfully");
    
    // Show segment information
    println!("\n9. Database information:");
    println!("Number of segments: {}", db.segment_count());
    println!("Segment names: {:?}", db.segment_names());
    println!("Bucket interval: {} seconds", db.bucket_interval());
    
    println!("\n✓ Example completed successfully!");
    println!("Data stored in: /tmp/tsdb_example");
    
    Ok(())
} 