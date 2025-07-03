pub const BLOCK_BYTES: usize = 1024 * 1024;

pub struct ValueBlock<'a> {
    data: &'a mut [u64],
}

impl<'a> ValueBlock<'a> {
    pub fn new(raw: &'a mut [u8]) -> Self {
        let len = raw.len() / std::mem::size_of::<u64>();
        let ptr = raw.as_mut_ptr() as *mut u64;
        let data = unsafe { std::slice::from_raw_parts_mut(ptr, len) };
        Self { data }
    }

    pub fn write(&mut self, index: usize, value: u64) {
        self.data[index] = value;
    }

    pub fn read(&self, index: usize) -> u64 {
        self.data[index]
    }

    pub fn as_slice(&self) -> &[u64] {
        &self.data
    }

    pub fn as_mut_slice(&mut self) -> &mut [u64] {
        &mut self.data
    }
}
