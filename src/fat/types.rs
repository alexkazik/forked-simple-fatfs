//! Type aliases for various important data types for readability
//!
//! Some Index types have their corresponding Count type
//! (e.g. [`ClusterIndex`] and [`ClusterCount`]).
//! These are both under the hood aliased to the same data type.
//! The only reason to keep the separated is too make the code more readable:
//! for example, if a function returns a [`ClusterIndex`], we know that it
//! returns the index of a particular cluster, whereas if it were to return
//! [`ClusterCount`], it could return how many clusters are needed for
//! a particular action or belong to an object.

use core::ops::Mul;

#[cfg(not(feature = "lba64"))]
/// The number/offset/position of a block.
///
/// A block is defined by the [`BlockRead`](crate::fat::BlockRead) trait.
///
/// Depending on the feature `lba64` it is either u32 or u64.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BlockIndex(pub u32);
#[cfg(feature = "lba64")]
/// The number/offset/position of a block.
///
/// A block is defined by the [`BlockRead`](crate::fat::BlockRead) trait.
///
/// Depending on the feature `lba64` it is either u32 or u64.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BlockIndex(pub u64);
#[allow(dead_code)]
pub(crate) type BlockCount = BlockIndex;

impl BlockIndex {
    #[cfg(not(feature = "lba64"))]
    /// Convert u64 into BlockIndex, unchecked.
    ///
    /// Without the feature `lba64` this is a `u64 as u32`.
    pub const fn unchecked_from_u64(value: u64) -> Self {
        #[allow(clippy::cast_possible_truncation)]
        Self(value as u32)
    }

    #[cfg(feature = "lba64")]
    /// Convert u64 into BlockIndex, unchecked.
    ///
    /// Without the feature `lba64` this is a `u64 as u32`.
    pub const fn unchecked_from_u64(value: u64) -> Self {
        Self(value)
    }
}

impl From<BlockIndex> for u64 {
    #[cfg(not(feature = "lba64"))]
    fn from(value: BlockIndex) -> Self {
        value.0.into()
    }

    #[cfg(feature = "lba64")]
    fn from(value: BlockIndex) -> Self {
        value.0
    }
}

impl From<u32> for BlockIndex {
    #[cfg(not(feature = "lba64"))]
    fn from(value: u32) -> Self {
        Self(value)
    }

    #[cfg(feature = "lba64")]
    fn from(value: u32) -> Self {
        Self(value.into())
    }
}

impl From<u16> for BlockIndex {
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}

impl Mul for BlockIndex {
    type Output = BlockIndex;

    fn mul(self, rhs: Self) -> Self::Output {
        BlockIndex(self.0 * rhs.0)
    }
}

pub(crate) type ClusterIndex = u32;
pub(crate) type ClusterCount = ClusterIndex;

pub(crate) type SectorIndex = u32;
pub(crate) type SectorCount = SectorIndex;

pub(crate) type EntryIndex = u16;
pub(crate) type EntryCount = EntryIndex;

pub(crate) type FATEntryIndex = u32;
pub(crate) type FATEntryCount = FATEntryIndex;

pub(crate) type FATEntryValue = u32;

pub(crate) type FileSize = u32;
