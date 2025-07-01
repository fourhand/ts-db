use ts_db::{add, valueblock, valuefile};
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
fn valueblock_write_read() {
    let mut buf = vec![0u8; valueblock::BLOCK_BYTES];
    {
        let mut vb = valueblock::ValueBlock::new(&mut buf);
        vb.write(5, 99);
        assert_eq!(vb.read(5), 99);
    }
    let mut vb = valueblock::ValueBlock::new(&mut buf);
    assert_eq!(vb.read(5), 99);
}

#[test]
fn valuefile_multiple_blocks() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("multi.dat");
    let mut vf = valuefile::ValueFile::with_size(&path, valueblock::BLOCK_BYTES * 2).unwrap();
    vf.write_value(0, 10, 1).unwrap();
    vf.write_value(1, 20, 2).unwrap();
    vf.flush_if_idle().unwrap();
    assert_eq!(vf.read_value(0, 10).unwrap(), 1);
    assert_eq!(vf.read_value(1, 20).unwrap(), 2);
}

#[test]
fn it_works() {
    let result = add(2, 2);
    assert_eq!(result, 4);
}
