use crate::block_io::{BlockBase, BlockRead, BlockWrite};
pub use crate::fat::types::BlockIndex;
use crate::{BlockCount, BlockSize};
use alloc::boxed::Box;
use core::array;
use core::fmt::{Debug, Display, Formatter};
use core::iter;
use core::ops::{Deref, DerefMut};
use embedded_io::{ErrorKind, ErrorType};
use embedded_storage::nor_flash::NorFlash;

/// Translate between different hardware and software "virtual" block sizes.
///
/// This is useful if, for example the underlying flash can only be written in
/// 64 KiB blocks, but of course 512 byte blocks are used by fat.
/// (In some cases 1, 2 or 4 KiB blocks are used by fat, depending on how
/// it's formatted but it's always ok to have a smaller block size.)
///
/// This is done by creating one or more buffer(s) in the size of a hardware
/// block and the smaller virtual blocks are read/written into the buffer.
///
/// Always use flush on the translation layer, which will save the current
/// block buffer(s) if needed and flushes the underlying level.
///
/// As soon as the translation level is dropped the underlying storage can
/// be accessed again.
///
/// Example:
/// ```rust
/// # use simple_fatfs::block_io::*;
/// # use embedded_storage::nor_flash::{ErrorType, NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash};
/// # #[derive(Debug)]
/// # struct FlashWriteError;
/// # impl NorFlashError for FlashWriteError {
/// #     fn kind(&self) -> NorFlashErrorKind { NorFlashErrorKind::Other }
/// # }
/// # struct Flash();
/// # impl ErrorType for Flash {
/// #     type Error = FlashWriteError;
/// # }
/// # impl ReadNorFlash for Flash {
/// #     const READ_SIZE: usize = 1;
/// #     fn read(&mut self, _offset: u32, _buf: &mut [u8]) -> Result<(), Self::Error> { Ok(()) }
/// #     fn capacity(&self) -> usize { 65536 }
/// # }
/// impl NorFlash for Flash {
///     const WRITE_SIZE: usize = 65536;
///     const ERASE_SIZE: usize = 65536;
///     // ...
///  #    fn erase(&mut self, _from: u32, _to: u32) -> Result<(), Self::Error> { Ok(()) }
///  #    fn write(&mut self, _offset: u32, _buf: &[u8]) -> Result<(), Self::Error> { Ok(()) }
/// }
///
/// // create storage
/// let mut flash = Flash(/*...*/);
///
/// // create buffer and the translation level
/// let mut buffer = [0u8; 65536];
/// let mut translated = NorFlashTranslator::<512, _, _, _>::new_with_buffer(&mut flash, [&mut buffer]);
///
/// // write one block and flush it
/// translated.write(0, &[11; 512])?;
/// translated.flush()?;
///
/// # Ok::<(), simple_fatfs::block_io::NorFlashError<FlashWriteError>>(())
/// ```
///
/// The following must held true, otherwise an error occurs:
/// * number of buffers (BUFS) must be greater than zero
/// * all block sizes (VBS, READ_SIZE, WRITE_SIZE, ERASE_SIZE) must be a power of two
/// * 0 < READ_SIZE <= VBS <= ERASE_SIZE <= BUF_SIZE
/// * READ_SIZE <= WRITE_SIZE <= ERASE_SIZE

#[derive(Debug)]
pub struct NorFlashTranslator<'a, const VBS: BlockSize, const BUF_SIZE: usize, const BUFS: usize, S>
where
    S: NorFlash,
{
    buffers: [Buffer<'a, BUF_SIZE>; BUFS],
    storage: S,
    /// BUFS==1: unused
    /// BUFS==2: buffer number of the last used buffer
    /// BUFS>=3: next "timestamp" to be used
    next: usize,
}

#[derive(Debug)]
enum BufferLocation<'a, const BUF_SIZE: usize> {
    Borrowed(&'a mut [u8; BUF_SIZE]),
    Owned(Box<[u8; BUF_SIZE]>),
}

#[derive(Debug)]
struct Buffer<'a, const BUF_SIZE: usize> {
    buffer: BufferLocation<'a, BUF_SIZE>,
    // despite the handling in the rest of the library, this is a byte offset to a block, not a block number
    stored_block: u32,
    status: BlockTranslatorStatus,
    /// BUFS<=2: unused
    /// BUFS>=3: "timestamp" when buffer was used last
    last_used: usize,
}

#[derive(Debug, Eq, PartialEq)]
enum BlockTranslatorStatus {
    Unknown,
    Read,
    Modified,
}

/// Wrapper to allow [`ErrorType::Error`](embedded_storage::nor_flash::ErrorType::Error) as [`ErrorType::Error`].
#[derive(Debug)]
pub struct NorFlashError<E>(pub E);

impl<E: Display> Display for NorFlashError<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_str("NorFlashError:")?;
        self.0.fmt(f)
    }
}

impl<E: Debug + Display> core::error::Error for NorFlashError<E> {}

impl<E: Debug> embedded_io::Error for NorFlashError<E> {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

impl<E> Deref for NorFlashError<E> {
    type Target = E;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<E> DerefMut for NorFlashError<E> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<E> From<E> for NorFlashError<E> {
    fn from(value: E) -> Self {
        Self(value)
    }
}

impl<const VBS: BlockSize, const BUF_SIZE: usize, const BUFS: usize, S>
    NorFlashTranslator<'static, VBS, BUF_SIZE, BUFS, S>
where
    S: NorFlash,
{
    /// Create a new BlockTranslator.
    ///
    /// Example:
    /// Create a BlockTranslator with 1 buffer of 65536 bytes and 512 bytes of virtual block size.
    /// ```no_compile
    /// let mut translated = BlockTranslator::<512, 65536, 1, _>::new(&mut storage)?;
    /// ```
    pub fn new(storage: S) -> Self {
        Self::new_internal(
            storage,
            iter::from_fn(|| Some(BufferLocation::Owned(Box::new([0; BUF_SIZE])))),
        )
    }
}

impl<'a, const VBS: BlockSize, const BUF_SIZE: usize, const BUFS: usize, S>
    NorFlashTranslator<'a, VBS, BUF_SIZE, BUFS, S>
where
    S: NorFlash,
{
    #[expect(clippy::cast_possible_truncation)]
    // since usize is at least 32-bits long, this is ok
    const HW_BLOCK_SIZE: u32 = S::ERASE_SIZE as u32;

    // since usize is at least 32-bits long, this is ok
    const VBS_USIZE: usize = VBS as usize;

    /// Create a new BlockTranslator.
    ///
    /// Example:
    /// Create a BlockTranslator with 1 buffer of 65536 bytes and 512 bytes of virtual block size.
    /// ```no_compile
    /// let mut buffer = [0u8; 65536];
    /// let mut translated = BlockTranslator::<512, _, _, _>::new_with_buffer(&mut storage, [&mut buffer])?;
    /// ```
    pub fn new_with_buffer(storage: S, buffer: [&'a mut [u8; BUF_SIZE]; BUFS]) -> Self {
        Self::new_internal(storage, buffer.into_iter().map(BufferLocation::Borrowed))
    }

    /// Create a new BlockTranslator.
    fn new_internal<I>(storage: S, mut buffers: I) -> Self
    where
        I: Iterator<Item = BufferLocation<'a, BUF_SIZE>>,
    {
        // Compile-time check
        const {
            if BUFS == 0 {
                panic!("number of buffers (BUFS) must be greater than zero");
            }
            if !VBS.is_power_of_two()
                || !S::READ_SIZE.is_power_of_two()
                || !S::WRITE_SIZE.is_power_of_two()
                || !S::ERASE_SIZE.is_power_of_two()
            {
                panic!("all block sizes (VBS, READ_SIZE, WRITE_SIZE, ERASE_SIZE) must be a power of two");
            }
            if S::READ_SIZE == 0
                || S::READ_SIZE > Self::VBS_USIZE
                || Self::VBS_USIZE > S::ERASE_SIZE
                || S::ERASE_SIZE > BUF_SIZE
                || S::READ_SIZE > S::WRITE_SIZE
                || S::WRITE_SIZE > S::ERASE_SIZE
            {
                panic!("the following must be satisfied: 0 < READ_SIZE <= VBS <= ERASE_SIZE <= BUF_SIZE and READ_SIZE <= WRITE_SIZE <= ERASE_SIZE")
            }
        }

        Self {
            storage,
            buffers: array::from_fn::<_, BUFS, _>(|i| Buffer {
                buffer: buffers.next().unwrap(),
                stored_block: 0,
                status: BlockTranslatorStatus::Unknown,
                last_used: i,
            }),
            next: if BUFS >= 3 { BUFS } else { 0 },
        }
    }

    fn go_to_block<'b>(
        &'b mut self,
        block_in_vbs: BlockIndex,
    ) -> Result<(&'b mut Buffer<'a, BUF_SIZE>, usize), NorFlashError<S::Error>> {
        // assert that all known blocks are distinct
        debug_assert!(!self.buffers.iter().enumerate().any(|(p1, b1)| b1.status
            != BlockTranslatorStatus::Unknown
            && self.buffers.iter().enumerate().any(|(p2, b2)| p1 != p2
                && b2.status != BlockTranslatorStatus::Unknown
                && b1.stored_block == b2.stored_block)));

        #[cfg_attr(not(feature = "lba64"), expect(clippy::useless_conversion))]
        let byte_offset = u32::try_from(block_in_vbs).unwrap() * VBS;
        // despite the handling in the rest of the library, this is a byte offset to a block, not a block number
        let real_block = (byte_offset / Self::HW_BLOCK_SIZE) * Self::HW_BLOCK_SIZE;
        // since usize is at least 32-bits long, this is ok
        let offset = (byte_offset % Self::HW_BLOCK_SIZE) as usize;

        let buffer = match BUFS {
            1 => {
                let buffer = &mut self.buffers[0];

                if buffer.stored_block == real_block
                    && buffer.status != BlockTranslatorStatus::Unknown
                {
                    return Ok((buffer, offset));
                }

                buffer
            }
            2 => {
                // check newest buffer
                if self.buffers[self.next].stored_block == real_block
                    && self.buffers[self.next].status != BlockTranslatorStatus::Unknown
                {
                    return Ok((&mut self.buffers[self.next], offset));
                }

                // switch to older buffer
                self.next ^= 1;

                let buffer = &mut self.buffers[self.next];

                // check older buffer
                if buffer.status != BlockTranslatorStatus::Unknown
                    && buffer.stored_block == real_block
                {
                    return Ok((buffer, offset));
                }

                buffer
            }
            _ => {
                if self.next == usize::MAX {
                    // reset all ages because an overflow would happen otherwise
                    let mut ages = [(0, 0); BUFS];
                    for (num, buf) in self.buffers.iter().enumerate() {
                        ages[num] = (num, buf.last_used);
                    }
                    ages.sort_by_key(|&(_, age)| age);
                    for (new_age, (num, _last_used)) in ages.into_iter().enumerate() {
                        self.buffers[num].last_used = new_age;
                    }
                    self.next = BUFS;
                }

                let mut oldest: Option<&'b mut Buffer<'a, BUF_SIZE>> = None;

                // check if the block is already buffered
                for buffer in self.buffers.iter_mut() {
                    if buffer.stored_block == real_block
                        && buffer.status != BlockTranslatorStatus::Unknown
                    {
                        // update last_used, unless it already was the last used
                        if buffer.last_used != self.next - 1 {
                            buffer.last_used = self.next;
                            self.next += 1;
                        }

                        return Ok((buffer, offset));
                    }
                    if oldest.is_none() || buffer.last_used < oldest.as_ref().unwrap().last_used {
                        oldest = Some(buffer);
                    }
                }

                // get oldest buffer
                let buffer = oldest.unwrap();

                buffer.last_used = self.next;
                self.next += 1;

                buffer
            }
        };

        // store block, if required
        if buffer.status == BlockTranslatorStatus::Modified {
            self.storage.erase(
                buffer.stored_block,
                buffer.stored_block + Self::HW_BLOCK_SIZE,
            )?;
            self.storage.write(buffer.stored_block, &*buffer.buffer)?;
        }

        // read block
        buffer.stored_block = real_block;
        self.storage
            .read(buffer.stored_block, &mut *buffer.buffer)?;
        buffer.status = BlockTranslatorStatus::Read;

        Ok((buffer, offset))
    }
}

impl<const VBS: BlockSize, const BUF_SIZE: usize, const BUFS: usize, S> ErrorType
    for NorFlashTranslator<'_, VBS, BUF_SIZE, BUFS, S>
where
    S: NorFlash,
{
    type Error = NorFlashError<S::Error>;
}

impl<const VBS: BlockSize, const BUF_SIZE: usize, const BUFS: usize, S> BlockBase
    for NorFlashTranslator<'_, VBS, BUF_SIZE, BUFS, S>
where
    S: NorFlash,
{
    #[inline]
    fn block_size(&self) -> BlockSize {
        VBS
    }

    #[inline]
    fn block_count(&self) -> BlockCount {
        BlockCount::try_from(self.storage.capacity() / Self::VBS_USIZE).unwrap()
    }
}

impl<const VBS: BlockSize, const BUF_SIZE: usize, const BUFS: usize, S> BlockRead
    for NorFlashTranslator<'_, VBS, BUF_SIZE, BUFS, S>
where
    S: NorFlash,
{
    fn read(
        &mut self,
        mut block_in_vbs: BlockIndex,
        mut buf: &mut [u8],
    ) -> Result<(), Self::Error> {
        while !buf.is_empty() {
            let (this, next) = buf.split_at_mut(VBS.try_into().unwrap());
            let (buffer, offset) = self.go_to_block(block_in_vbs)?;
            this.copy_from_slice(&buffer.buffer[offset..offset + usize::try_from(VBS).unwrap()]);

            // advance
            buf = next;
            block_in_vbs += 1;
        }

        Ok(())
    }
}

impl<const VBS: BlockSize, const BUF_SIZE: usize, const BUFS: usize, S> BlockWrite
    for NorFlashTranslator<'_, VBS, BUF_SIZE, BUFS, S>
where
    S: NorFlash,
{
    fn write(&mut self, mut block_in_vbs: BlockIndex, mut buf: &[u8]) -> Result<(), Self::Error> {
        while !buf.is_empty() {
            let (this, next) = buf.split_at(VBS.try_into().unwrap());
            let (buffer, offset) = self.go_to_block(block_in_vbs)?;
            buffer.buffer[offset..offset + usize::try_from(VBS).unwrap()].copy_from_slice(this);
            buffer.status = BlockTranslatorStatus::Modified;

            // advance
            buf = next;
            block_in_vbs += 1;
        }

        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        for buffer in self.buffers.iter_mut() {
            if buffer.status == BlockTranslatorStatus::Modified {
                self.storage.erase(
                    buffer.stored_block,
                    buffer.stored_block + Self::HW_BLOCK_SIZE,
                )?;
                self.storage.write(buffer.stored_block, &*buffer.buffer)?;
                buffer.status = BlockTranslatorStatus::Read;
            }
        }

        Ok(())
    }
}

impl<const VBS: BlockSize, const BUF_SIZE: usize, const BUFS: usize, S> Drop
    for NorFlashTranslator<'_, VBS, BUF_SIZE, BUFS, S>
where
    S: NorFlash,
{
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

impl<const BUF_SIZE: usize> Deref for BufferLocation<'_, BUF_SIZE> {
    type Target = [u8; BUF_SIZE];

    fn deref(&self) -> &Self::Target {
        match self {
            BufferLocation::Borrowed(e) => e,
            BufferLocation::Owned(b) => b.deref(),
        }
    }
}

impl<const BUF_SIZE: usize> DerefMut for BufferLocation<'_, BUF_SIZE> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            BufferLocation::Borrowed(e) => e,
            BufferLocation::Owned(b) => b.deref_mut(),
        }
    }
}
