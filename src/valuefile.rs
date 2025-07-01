use std::fs::{File, OpenOptions};
use std::io::Result;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use memmap2::MmapMut;

use crate::valueblock::{ValueBlock, BLOCK_BYTES};

pub struct ValueFile {
    file: File,
    mmap: MmapMut,
    path: PathBuf,
    last_write: Instant,
    size: usize,
}

impl ValueFile {
    pub fn with_size<P: AsRef<Path>>(path: P, size: usize) -> Result<Self> {
        let path_ref = path.as_ref();
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(path_ref)?;
        file.set_len(size as u64)?;
        let mmap = unsafe { MmapMut::map_mut(&file)? };
        Ok(Self {
            file,
            mmap,
            path: path_ref.to_path_buf(),
            last_write: Instant::now(),
            size,
        })
    }

    pub fn get_block(&mut self, index: usize) -> Option<ValueBlock<'_>> {
        let start = index * BLOCK_BYTES;
        let end = start + BLOCK_BYTES;
        if end > self.size {
            return None;
        }
        let slice = &mut self.mmap[start..end];
        Some(ValueBlock::new(slice))
    }

    pub fn write_value(&mut self, block_idx: usize, offset: usize, value: u64) -> Result<()> {
        let byte_offset = block_idx * BLOCK_BYTES + offset * std::mem::size_of::<u64>();
        let slice = &mut self.mmap[byte_offset..byte_offset + 8];
        slice.copy_from_slice(&value.to_le_bytes());
        self.last_write = Instant::now();
        Ok(())
    }

    pub fn read_value(&self, block_idx: usize, offset: usize) -> Option<u64> {
        let byte_offset = block_idx * BLOCK_BYTES + offset * std::mem::size_of::<u64>();
        if byte_offset + 8 > self.size {
            return None;
        }
        let mut arr = [0u8; 8];
        arr.copy_from_slice(&self.mmap[byte_offset..byte_offset + 8]);
        Some(u64::from_le_bytes(arr))
    }

    pub fn flush_if_idle(&mut self) -> Result<()> {
        if self.last_write.elapsed() >= Duration::from_secs(1) {
            self.mmap.flush()?;
            self.last_write = Instant::now();
        }
        Ok(())
    }
}
