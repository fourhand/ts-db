use crate::{Result, TsDbError, Value, VBLOCK_SIZE, VFILE_SIZE_LIMIT};
use memmap2::{Mmap, MmapMut};
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Value Block - 1MB fixed-size block for storing u64 values
pub struct VBlock {
    data: MmapMut,
    used_bytes: usize,
}

impl VBlock {
    /// Create a new VBlock
    pub fn new() -> Result<Self> {
        let file = tempfile::tempfile()?;
        file.set_len(VBLOCK_SIZE as u64)?;
        
        let data = unsafe { MmapMut::map_mut(&file)? };
        
        Ok(Self {
            data,
            used_bytes: 0,
        })
    }
    
    /// Write a value at a specific sequence position
    pub fn write_value(&mut self, seq: usize, value: Value) -> Result<()> {
        let offset = seq * std::mem::size_of::<Value>();
        if offset + std::mem::size_of::<Value>() > VBLOCK_SIZE {
            return Err(TsDbError::ValueBlockFull);
        }
        
        let bytes = value.to_le_bytes();
        self.data[offset..offset + std::mem::size_of::<Value>()].copy_from_slice(&bytes);
        
        let new_used = offset + std::mem::size_of::<Value>();
        if new_used > self.used_bytes {
            self.used_bytes = new_used;
        }
        
        Ok(())
    }
    
    /// Read a value at a specific sequence position
    pub fn read_value(&self, seq: usize) -> Result<Value> {
        let offset = seq * std::mem::size_of::<Value>();
        if offset + std::mem::size_of::<Value>() > VBLOCK_SIZE {
            return Err(TsDbError::InvalidSeq(seq as u64));
        }
        
        let bytes = &self.data[offset..offset + std::mem::size_of::<Value>()];
        let value = Value::from_le_bytes(bytes.try_into().unwrap());
        Ok(value)
    }
    
    /// Check if block has space for a sequence
    pub fn has_space_for(&self, seq: usize) -> bool {
        let offset = seq * std::mem::size_of::<Value>();
        offset + std::mem::size_of::<Value>() <= VBLOCK_SIZE
    }
    
    /// Get used bytes
    pub fn used_bytes(&self) -> usize {
        self.used_bytes
    }
    
    /// Get available bytes
    pub fn available_bytes(&self) -> usize {
        VBLOCK_SIZE - self.used_bytes
    }
    
    /// Flush data to disk
    pub fn flush(&self) -> Result<()> {
        self.data.flush()?;
        Ok(())
    }
}

/// Value File - manages multiple VBlocks with 2GB size limit
pub struct VFile {
    file_path: String,
    blocks: Vec<VBlock>,
    current_block_index: usize,
}

impl VFile {
    /// Create a new VFile
    pub fn new(file_path: String) -> Result<Self> {
        // Create directory if it doesn't exist
        if let Some(parent) = std::path::Path::new(&file_path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        Ok(Self {
            file_path,
            blocks: vec![VBlock::new()?],
            current_block_index: 0,
        })
    }
    
    /// Write a value at a specific sequence position
    pub fn write_value(&mut self, seq: usize, value: Value) -> Result<()> {
        // Find the appropriate block for this sequence
        let block_index = seq / (VBLOCK_SIZE / std::mem::size_of::<Value>());
        
        // Ensure we have enough blocks
        while block_index >= self.blocks.len() {
            self.blocks.push(VBlock::new()?);
        }
        
        // Write to the appropriate block
        self.blocks[block_index].write_value(seq, value)?;
        
        // Update current block index if needed
        if block_index > self.current_block_index {
            self.current_block_index = block_index;
        }
        
        Ok(())
    }
    
    /// Read a value at a specific sequence position
    pub fn read_value(&self, seq: usize) -> Result<Value> {
        let block_index = seq / (VBLOCK_SIZE / std::mem::size_of::<Value>());
        
        if block_index >= self.blocks.len() {
            return Err(TsDbError::InvalidSeq(seq as u64));
        }
        
        self.blocks[block_index].read_value(seq)
    }
    
    /// Get total size in bytes
    pub fn total_size(&self) -> usize {
        self.blocks.iter().map(|block| block.used_bytes()).sum()
    }
    
    /// Check if file size limit exceeded
    pub fn is_size_limit_exceeded(&self) -> bool {
        self.total_size() >= VFILE_SIZE_LIMIT
    }
    
    /// Flush all blocks to disk
    pub fn flush(&self) -> Result<()> {
        for block in &self.blocks {
            block.flush()?;
        }
        Ok(())
    }
    
    /// Get file path
    pub fn file_path(&self) -> &str {
        &self.file_path
    }
}

/// Manages multiple VFiles and provides high-level storage operations
pub struct ValueBlockManager {
    files: Vec<VFile>,
    current_file_index: usize,
    base_path: String,
    file_counter: usize,
}

impl ValueBlockManager {
    /// Create a new ValueBlockManager
    pub fn new(base_path: String) -> Result<Self> {
        // Create directory if it doesn't exist
        std::fs::create_dir_all(&base_path)?;
        
        let first_file_path = format!("{}/tsdata_0001.dat", base_path);
        let first_file = VFile::new(first_file_path)?;
        
        Ok(Self {
            files: vec![first_file],
            current_file_index: 0,
            base_path,
            file_counter: 1,
        })
    }
    
    /// Write a value at a specific sequence position
    pub fn write_value(&mut self, seq: usize, value: Value) -> Result<()> {
        // Try to write to current file
        if let Ok(()) = self.files[self.current_file_index].write_value(seq, value) {
            return Ok(());
        }
        
        // If current file is full, create a new one
        self.create_new_file()?;
        
        // Try writing to the new file
        self.files[self.current_file_index].write_value(seq, value)
    }
    
    /// Read a value at a specific sequence position
    pub fn read_value(&self, seq: usize) -> Result<Value> {
        // Try reading from all files until found
        for file in &self.files {
            if let Ok(value) = file.read_value(seq) {
                return Ok(value);
            }
        }
        
        Err(TsDbError::InvalidSeq(seq as u64))
    }
    
    /// Create a new VFile
    fn create_new_file(&mut self) -> Result<()> {
        self.file_counter += 1;
        let file_path = format!("{}/tsdata_{:04}.dat", self.base_path, self.file_counter);
        let new_file = VFile::new(file_path)?;
        
        self.files.push(new_file);
        self.current_file_index = self.files.len() - 1;
        
        Ok(())
    }
    
    /// Flush all files to disk
    pub fn flush(&self) -> Result<()> {
        for file in &self.files {
            file.flush()?;
        }
        Ok(())
    }
    
    /// Get total number of files
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
    
    /// Get current file index
    pub fn current_file_index(&self) -> usize {
        self.current_file_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_vblock_basic_operations() {
        let mut block = VBlock::new().unwrap();
        
        // Test writing and reading values
        block.write_value(0, 123).unwrap();
        block.write_value(1, 456).unwrap();
        block.write_value(100, 789).unwrap();
        
        assert_eq!(block.read_value(0).unwrap(), 123);
        assert_eq!(block.read_value(1).unwrap(), 456);
        assert_eq!(block.read_value(100).unwrap(), 789);
        
        // Test space checking
        assert!(block.has_space_for(0));
        assert!(block.has_space_for(100));
        
        // Test error for invalid sequence
        assert!(block.read_value(999999).is_err());
    }
    
    #[test]
    fn test_vfile_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.dat").to_string_lossy().to_string();
        
        let mut file = VFile::new(file_path).unwrap();
        
        // Test writing and reading values
        file.write_value(0, 123).unwrap();
        file.write_value(1, 456).unwrap();
        file.write_value(100000, 789).unwrap(); // This should go to a new block
        
        assert_eq!(file.read_value(0).unwrap(), 123);
        assert_eq!(file.read_value(1).unwrap(), 456);
        assert_eq!(file.read_value(100000).unwrap(), 789);
        
        // Test size tracking
        assert!(file.total_size() > 0);
        assert!(!file.is_size_limit_exceeded());
    }
    
    #[test]
    fn test_value_block_manager() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_string_lossy().to_string();
        
        let mut manager = ValueBlockManager::new(base_path).unwrap();
        
        // Test writing and reading values
        manager.write_value(0, 123).unwrap();
        manager.write_value(1, 456).unwrap();
        manager.write_value(100000, 789).unwrap();
        
        assert_eq!(manager.read_value(0).unwrap(), 123);
        assert_eq!(manager.read_value(1).unwrap(), 456);
        assert_eq!(manager.read_value(100000).unwrap(), 789);
        
        // Test file management
        assert_eq!(manager.file_count(), 1);
        assert_eq!(manager.current_file_index(), 0);
    }
} 