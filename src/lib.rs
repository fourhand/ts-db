
pub mod valueblock;
pub mod valuefile;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn valuefile_write_and_read() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.dat");
        let mut vf = valuefile::ValueFile::with_size(&path, valueblock::BLOCK_BYTES * 2).unwrap();
        vf.write_value(0, 0, 42).unwrap();
        vf.flush_if_idle().unwrap();
        let v = vf.read_value(0, 0).unwrap();
        assert_eq!(v, 42);
    }

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
