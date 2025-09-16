use super::*;

use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

#[cfg(not(feature = "std"))]
use alloc::boxed::Box;

#[derive(Debug)]
pub(crate) struct SectorBuffer<'s> {
    slice: Box<[u8]>,
    pub(crate) stored_sector: SectorIndex,
    phantom_data: PhantomData<&'s ()>,
}

impl SectorBuffer<'_> {
    pub(crate) fn new(sector_buffer: Box<[u8]>, stored_sector: SectorIndex) -> Self {
        Self {
            slice: sector_buffer,
            stored_sector,
            phantom_data: PhantomData,
        }
    }
}

impl Deref for SectorBuffer<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.slice
    }
}

impl DerefMut for SectorBuffer<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.slice
    }
}
