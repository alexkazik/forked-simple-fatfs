use embedded_storage::nor_flash::{
    ErrorType, NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash,
};
use simple_fatfs::block_io::*;

#[derive(Debug)]
struct Storage<'a, const BS: usize>(&'a mut [u8; 64]);

impl<const BS: usize> ReadNorFlash for Storage<'_, BS> {
    const READ_SIZE: usize = 1;

    fn read(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), Self::Error> {
        let offset: usize = offset as usize;
        buf.copy_from_slice(&self.0[offset..offset + buf.len()]);
        Ok(())
    }

    fn capacity(&self) -> usize {
        64
    }
}

#[derive(Debug)]
struct FlashWriteError;

impl NorFlashError for FlashWriteError {
    fn kind(&self) -> NorFlashErrorKind {
        NorFlashErrorKind::Other
    }
}

impl<const BS: usize> ErrorType for Storage<'_, BS> {
    type Error = FlashWriteError;
}

impl<const BS: usize> NorFlash for Storage<'_, BS> {
    const WRITE_SIZE: usize = BS;
    const ERASE_SIZE: usize = BS;

    fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        if !from.is_multiple_of(BS as u32) {
            panic!("write error, from is not a multiple of BS");
        }
        if !to.is_multiple_of(BS as u32) {
            panic!("write error, to is not a multiple of BS");
        }

        for i in from..to {
            self.0[i as usize] = 0xff;
        }
        Ok(())
    }

    fn write(&mut self, offset: u32, buf: &[u8]) -> Result<(), Self::Error> {
        let offset: usize = offset as usize;

        if !offset.is_multiple_of(BS) {
            panic!("write error, offset is not a multiple of BS");
        }
        if !buf.len().is_multiple_of(BS) {
            panic!("write error, buf.len() is not a multiple of BS");
        }

        // check if the bits can be programmed
        for (o, (f, b)) in self.0[offset..offset + buf.len()]
            .iter()
            .zip(buf)
            .enumerate()
        {
            if *f & *b != *b {
                panic!(
                    "write error, tried to write a 1 into a 0 at offset {}, byte in flash {f:#02x}, byte in buffer {b:#02x}",
                    offset + o
                );
            }
        }

        // write
        self.0[offset..offset + buf.len()].clone_from_slice(buf);

        Ok(())
    }
}

#[test]
fn test_block_translator1() {
    let mut translated_c_buffer1 = [0u8; 4];

    run_block_translator(Some([&mut translated_c_buffer1]));
}

#[test]
fn test_block_translator2() {
    let mut translated_c_buffer1 = [0u8; 4];
    let mut translated_c_buffer2 = [0u8; 4];

    run_block_translator(Some([&mut translated_c_buffer1, &mut translated_c_buffer2]));
}

#[test]
fn test_block_translator3() {
    let mut translated_c_buffer1 = [0u8; 4];
    let mut translated_c_buffer2 = [0u8; 4];
    let mut translated_c_buffer3 = [0u8; 4];

    run_block_translator(Some([
        &mut translated_c_buffer1,
        &mut translated_c_buffer2,
        &mut translated_c_buffer3,
    ]));
}

#[test]
fn test_block_translator8() {
    let mut translated_c_buffer1 = [0u8; 4];
    let mut translated_c_buffer2 = [0u8; 4];
    let mut translated_c_buffer3 = [0u8; 4];
    let mut translated_c_buffer4 = [0u8; 4];
    let mut translated_c_buffer5 = [0u8; 4];
    let mut translated_c_buffer6 = [0u8; 4];
    let mut translated_c_buffer7 = [0u8; 4];
    let mut translated_c_buffer8 = [0u8; 4];

    run_block_translator(Some([
        &mut translated_c_buffer1,
        &mut translated_c_buffer2,
        &mut translated_c_buffer3,
        &mut translated_c_buffer4,
        &mut translated_c_buffer5,
        &mut translated_c_buffer6,
        &mut translated_c_buffer7,
        &mut translated_c_buffer8,
    ]));
}

#[test]
fn test_block_translator1_heap() {
    run_block_translator::<1>(None);
}

#[test]
fn test_block_translator2_heap() {
    run_block_translator::<2>(None);
}

#[test]
fn test_block_translator3_heap() {
    run_block_translator::<3>(None);
}

#[test]
fn test_block_translator8_heap() {
    run_block_translator::<8>(None);
}

fn run_block_translator<const BUFS: usize>(buffer: Option<[&mut [u8; 4]; BUFS]>) {
    // initialize storage with random data, copy it to the second storage
    let mut array_a: [u8; 64] = rand::random();
    let mut array_b: [u8; 64] = array_a;

    // A = 64 * 1 buffer
    let mut storage_a = Storage::<1>(&mut array_a);
    // B = 16 * 4 buffer
    let mut storage_b = Storage::<4>(&mut array_b);

    // ensure that total number of bytes are equal
    assert_eq!(storage_a.capacity(), storage_b.capacity());

    // C = translated B into 64 * 1
    let mut translated_c = match buffer {
        None => NorFlashTranslator::<1, _, _, _>::new(&mut storage_b),
        Some(buffer) => NorFlashTranslator::<1, _, _, _>::new_with_buffer(&mut storage_b, buffer),
    };

    // ensure that total number of bytes are equal
    #[cfg_attr(feature = "lba64", expect(clippy::useless_conversion))]
    let translated_capacity =
        u64::from(translated_c.block_size()) * u64::from(translated_c.block_count());
    assert_eq!(storage_a.capacity() as u64, translated_capacity);

    // randomly read/write a byte from/into both storages and expect them to be identical
    for _ in 0..100_000 {
        let offset = rand::random_range(0..64);
        if rand::random::<bool>() {
            let mut buf_a = [0u8; 1];
            let mut buf_b = [0u8; 1];
            storage_a.read(offset, &mut buf_a).unwrap();
            #[cfg_attr(not(feature = "lba64"), expect(clippy::useless_conversion))]
            translated_c.read(offset.into(), &mut buf_b).unwrap();
            assert_eq!(buf_a, buf_b, "random read with {BUFS} buffers");
        } else {
            let value = [rand::random()];
            storage_a.erase(offset, offset + 1).unwrap();
            storage_a.write(offset, &value).unwrap();
            #[cfg_attr(not(feature = "lba64"), expect(clippy::useless_conversion))]
            translated_c.write(offset.into(), &value).unwrap();
        }
    }

    // flush and drop the translation level
    translated_c.flush().unwrap();
    drop(translated_c);

    // assure that the underlying storage of both is identical
    assert_eq!(
        array_a, array_b,
        "compare of both storages with {BUFS} buffers"
    );
}
