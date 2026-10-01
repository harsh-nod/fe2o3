//! Read-only logical retained-storage accounting, not allocator or RSS telemetry.
//!
//! One root header is charged by its caller. Heap walks charge actual Vec/String
//! capacity, Box payloads, and BTree key/value payloads at logical length. Tree
//! nodes, allocator metadata/rounding, scratch, stack and RSS are not modeled.
//! All limits are explicit observation limits; no constructor limit is implied.

use std::collections::{BTreeMap, BTreeSet};
use std::{error::Error, fmt, mem::size_of};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogicalStorageLimitsV1 {
    /// None measures retained storage without imposing a byte target.
    pub max_bytes: Option<usize>,
    /// Maximum charged owner/collection visits; not a semantic scan limit.
    pub max_items: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogicalStorageErrorV1 {
    Arithmetic,
    ByteLimit,
    ItemLimit,
    UnsupportedV11Owner,
}

impl fmt::Display for LogicalStorageErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "logical retained-storage accounting refused: {self:?}")
    }
}
impl Error for LogicalStorageErrorV1 {}

/// Arithmetic ledger only: callers must supply a complete actual-owner walk.
/// A successful ledger is not canonical validation, evidence of allocation
/// success, or any compiler/source/proof/load/launch authority.
#[derive(Debug)]
pub struct LogicalStorageCounterV1 {
    limits: LogicalStorageLimitsV1,
    bytes: usize,
    items: usize,
}

impl LogicalStorageCounterV1 {
    pub const fn new(limits: LogicalStorageLimitsV1) -> Self {
        Self {
            limits,
            bytes: 0,
            items: 0,
        }
    }
    pub const fn bytes(&self) -> usize {
        self.bytes
    }
    pub const fn items(&self) -> usize {
        self.items
    }

    /// Atomically checks both additions before updating the ledger.
    pub fn charge(&mut self, bytes: usize, items: usize) -> Result<(), LogicalStorageErrorV1> {
        let next_bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or(LogicalStorageErrorV1::Arithmetic)?;
        let next_items = self
            .items
            .checked_add(items)
            .ok_or(LogicalStorageErrorV1::Arithmetic)?;
        if self
            .limits
            .max_bytes
            .is_some_and(|limit| next_bytes > limit)
        {
            return Err(LogicalStorageErrorV1::ByteLimit);
        }
        if next_items > self.limits.max_items {
            return Err(LogicalStorageErrorV1::ItemLimit);
        }
        self.bytes = next_bytes;
        self.items = next_items;
        Ok(())
    }

    pub fn array<T>(&mut self, count: usize) -> Result<(), LogicalStorageErrorV1> {
        let bytes = count
            .checked_mul(size_of::<T>())
            .ok_or(LogicalStorageErrorV1::Arithmetic)?;
        self.charge(bytes, 1)
    }
    pub fn vector<T>(&mut self, values: &Vec<T>) -> Result<(), LogicalStorageErrorV1> {
        self.array::<T>(values.capacity())
    }
    pub fn string(&mut self, value: &String) -> Result<(), LogicalStorageErrorV1> {
        self.charge(value.capacity(), 1)
    }
    pub fn set<T>(&mut self, values: &BTreeSet<T>) -> Result<(), LogicalStorageErrorV1> {
        self.array::<T>(values.len())
    }
    pub fn map<K, V>(&mut self, values: &BTreeMap<K, V>) -> Result<(), LogicalStorageErrorV1> {
        // Key and value payloads, without tuple/node padding or tree metadata.
        let width = size_of::<K>()
            .checked_add(size_of::<V>())
            .ok_or(LogicalStorageErrorV1::Arithmetic)?;
        let bytes = width
            .checked_mul(values.len())
            .ok_or(LogicalStorageErrorV1::Arithmetic)?;
        self.charge(bytes, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counter(bytes: Option<usize>, items: usize) -> LogicalStorageCounterV1 {
        LogicalStorageCounterV1::new(LogicalStorageLimitsV1 {
            max_bytes: bytes,
            max_items: items,
        })
    }

    #[test]
    fn exact_limits_and_refusals_are_atomic() {
        let mut c = counter(Some(8), 2);
        c.charge(8, 2).unwrap();
        assert_eq!(c.charge(1, 0), Err(LogicalStorageErrorV1::ByteLimit));
        assert_eq!(c.charge(0, 1), Err(LogicalStorageErrorV1::ItemLimit));
        assert_eq!((c.bytes(), c.items()), (8, 2));
        assert_eq!(
            counter(Some(0), 1).charge(1, 1),
            Err(LogicalStorageErrorV1::ByteLimit)
        );
        assert_eq!(
            counter(None, 0).charge(0, 1),
            Err(LogicalStorageErrorV1::ItemLimit)
        );
    }

    #[test]
    fn overflow_refuses_without_allocation_or_partial_charge() {
        let mut c = counter(None, usize::MAX);
        assert_eq!(
            c.array::<u16>(usize::MAX),
            Err(LogicalStorageErrorV1::Arithmetic)
        );
        assert_eq!((c.bytes(), c.items()), (0, 0));
        c.charge(usize::MAX, usize::MAX).unwrap();
        assert_eq!(c.charge(1, 0), Err(LogicalStorageErrorV1::Arithmetic));
        assert_eq!(c.charge(0, 1), Err(LogicalStorageErrorV1::Arithmetic));
        assert_eq!((c.bytes(), c.items()), (usize::MAX, usize::MAX));
    }

    #[test]
    fn spare_capacity_and_logical_tree_payload_are_explicit() {
        let v: Vec<u32> = Vec::with_capacity(17);
        let text = String::with_capacity(31);
        let map = BTreeMap::from([(1_u8, 2_u64), (3, 4)]);
        let set = BTreeSet::from([1_u16, 2]);
        let mut c = counter(None, 4);
        c.vector(&v).unwrap();
        c.string(&text).unwrap();
        c.map(&map).unwrap();
        c.set(&set).unwrap();
        assert_eq!(
            c.bytes(),
            v.capacity() * size_of::<u32>()
                + text.capacity()
                + map.len() * (size_of::<u8>() + size_of::<u64>())
                + set.len() * size_of::<u16>()
        );
    }
}
