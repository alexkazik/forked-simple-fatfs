use super::*;

use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

#[cfg(all(not(feature = "std"), feature = "alloc_buffer"))]
use alloc::boxed::Box;

#[derive(Debug)]
pub(crate) struct SectorBuffer<'s> {
    #[cfg(feature = "alloc_buffer")]
    slice: Box<[u8]>,
    #[cfg(not(feature = "alloc_buffer"))]
    slice: &'s mut [u8],
    pub(crate) stored_sector: SectorIndex,
    phantom_data: PhantomData<&'s ()>,
}

#[cfg(feature = "alloc_buffer")]
impl SectorBuffer<'static> {
    pub(crate) fn new() -> Self {
        Self {
            slice: [0; 4096].into(),
            stored_sector: 0,
            phantom_data: PhantomData,
        }
    }
}

#[cfg(not(feature = "alloc_buffer"))]
impl<'s> SectorBuffer<'s> {
    pub(crate) fn new(slice: &'s mut [u8; 4096]) -> Self {
        Self {
            slice,
            stored_sector: 0,
            phantom_data: PhantomData,
        }
    }
}

impl SectorBuffer<'_> {
    pub(crate) fn downsize(self, size: usize) -> Self {
        #[cfg(feature = "alloc_buffer")]
        {
            Self {
                slice: self.slice[..size].into(),
                stored_sector: self.stored_sector,
                phantom_data: PhantomData,
            }
        }

        #[cfg(not(feature = "alloc_buffer"))]
        {
            Self {
                slice: &mut self.slice[..size],
                stored_sector: self.stored_sector,
                phantom_data: PhantomData,
            }
        }
    }
}

impl Deref for SectorBuffer<'_> {
    type Target = [u8];

    #[cfg_attr(not(feature = "alloc_buffer"), allow(clippy::needless_borrow))]
    fn deref(&self) -> &Self::Target {
        &self.slice
    }
}

impl DerefMut for SectorBuffer<'_> {
    #[cfg_attr(not(feature = "alloc_buffer"), allow(clippy::needless_borrow))]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.slice
    }
}
