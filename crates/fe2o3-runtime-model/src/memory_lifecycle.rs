//! Bounded, syscall-free VM and GPU-memory lifecycle model.

use alloc::{sync::Arc, vec::Vec};
use core::{fmt, ops::Deref};
use spin::Once;

#[cfg(test)]
use core::cell::Cell;

use crate::*;

pub const MEMORY_LIFECYCLE_SCHEMA_VERSION_V1: u16 = 1;
pub const MEMORY_PAGE_BYTES_V1: u64 = 4_096;
pub const MAX_MEMORY_VMS_V1: usize = 64;
pub const MAX_VA_RESERVATIONS_V1: usize = 512;
pub const MAX_MEMORY_ALLOCATIONS_V1: usize = 512;
pub const MAX_MEMORY_MAPPINGS_V1: usize = 1_024;
pub const MAX_MEMORY_PUBLICATIONS_V1: usize = 2_048;
pub const MAX_MEMORY_MAPPING_DEVICES_V1: usize = 16;
pub const MAX_MEMORY_ISSUED_ID_HIGH_WATERMARKS_V1: usize =
    MAX_MEMORY_VMS_V1 * 2 + MAX_MEMORY_ALLOCATIONS_V1 + MAX_MEMORY_MAPPINGS_V1;

#[cfg(test)]
std::thread_local! {
    static JOURNAL_COPIED_RECORDS_V1: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_journal_copied_records_for_test() {
    JOURNAL_COPIED_RECORDS_V1.set(0);
}

#[cfg(test)]
pub(crate) fn journal_copied_records_for_test() -> usize {
    JOURNAL_COPIED_RECORDS_V1.get()
}

fn note_journal_record_copy_v1() {
    #[cfg(test)]
    JOURNAL_COPIED_RECORDS_V1.set(JOURNAL_COPIED_RECORDS_V1.get() + 1);
}

fn clone_journal_record_v1<T: Clone>(record: &T) -> T {
    note_journal_record_copy_v1();
    record.clone()
}

struct JournalNodeV1<T> {
    left: Option<Arc<Self>>,
    value: T,
    right: Option<Arc<Self>>,
    height: u8,
    len: usize,
}

impl<T> JournalNodeV1<T> {
    fn new(left: Option<Arc<Self>>, value: T, right: Option<Arc<Self>>) -> Self {
        Self {
            height: 1 + node_height_v1(&left).max(node_height_v1(&right)),
            len: 1 + node_len_v1(&left) + node_len_v1(&right),
            left,
            value,
            right,
        }
    }
}

impl<T: Clone> Clone for JournalNodeV1<T> {
    fn clone(&self) -> Self {
        Self {
            left: self.left.clone(),
            value: clone_journal_record_v1(&self.value),
            right: self.right.clone(),
            height: self.height,
            len: self.len,
        }
    }
}

fn node_height_v1<T>(node: &Option<Arc<JournalNodeV1<T>>>) -> u8 {
    node.as_ref().map_or(0, |node| node.height)
}

fn node_len_v1<T>(node: &Option<Arc<JournalNodeV1<T>>>) -> usize {
    node.as_ref().map_or(0, |node| node.len)
}

fn rotate_journal_left_v1<T: Clone>(root: &Arc<JournalNodeV1<T>>) -> Arc<JournalNodeV1<T>> {
    let pivot = root.right.as_ref().expect("right-heavy journal node");
    let left = Arc::new(JournalNodeV1::new(
        root.left.clone(),
        clone_journal_record_v1(&root.value),
        pivot.left.clone(),
    ));
    Arc::new(JournalNodeV1::new(
        Some(left),
        clone_journal_record_v1(&pivot.value),
        pivot.right.clone(),
    ))
}

fn rotate_journal_right_v1<T: Clone>(root: &Arc<JournalNodeV1<T>>) -> Arc<JournalNodeV1<T>> {
    let pivot = root.left.as_ref().expect("left-heavy journal node");
    let right = Arc::new(JournalNodeV1::new(
        pivot.right.clone(),
        clone_journal_record_v1(&root.value),
        root.right.clone(),
    ));
    Arc::new(JournalNodeV1::new(
        pivot.left.clone(),
        clone_journal_record_v1(&pivot.value),
        Some(right),
    ))
}

fn balance_journal_node_v1<T: Clone>(mut root: Arc<JournalNodeV1<T>>) -> Arc<JournalNodeV1<T>> {
    let balance = i16::from(node_height_v1(&root.left)) - i16::from(node_height_v1(&root.right));
    if balance > 1 {
        let left = root.left.as_ref().expect("left-heavy journal node");
        if node_height_v1(&left.right) > node_height_v1(&left.left) {
            root = Arc::new(JournalNodeV1::new(
                Some(rotate_journal_left_v1(left)),
                clone_journal_record_v1(&root.value),
                root.right.clone(),
            ));
        }
        rotate_journal_right_v1(&root)
    } else if balance < -1 {
        let right = root.right.as_ref().expect("right-heavy journal node");
        if node_height_v1(&right.left) > node_height_v1(&right.right) {
            root = Arc::new(JournalNodeV1::new(
                root.left.clone(),
                clone_journal_record_v1(&root.value),
                Some(rotate_journal_right_v1(right)),
            ));
        }
        rotate_journal_left_v1(&root)
    } else {
        root
    }
}

fn insert_journal_record_v1<T: Clone>(
    root: Option<&Arc<JournalNodeV1<T>>>,
    index: usize,
    value: T,
) -> Arc<JournalNodeV1<T>> {
    let Some(root) = root else {
        debug_assert_eq!(index, 0);
        return Arc::new(JournalNodeV1::new(None, value, None));
    };
    let left_len = node_len_v1(&root.left);
    let rebuilt = if index <= left_len {
        Arc::new(JournalNodeV1::new(
            Some(insert_journal_record_v1(root.left.as_ref(), index, value)),
            clone_journal_record_v1(&root.value),
            root.right.clone(),
        ))
    } else {
        Arc::new(JournalNodeV1::new(
            root.left.clone(),
            clone_journal_record_v1(&root.value),
            Some(insert_journal_record_v1(
                root.right.as_ref(),
                index - left_len - 1,
                value,
            )),
        ))
    };
    balance_journal_node_v1(rebuilt)
}

fn build_balanced_journal_v1<T>(
    records: &mut alloc::vec::IntoIter<T>,
    len: usize,
) -> Option<Arc<JournalNodeV1<T>>> {
    if len == 0 {
        return None;
    }
    let left_len = len / 2;
    let left = build_balanced_journal_v1(records, left_len);
    let value = records.next().expect("journal length matches iterator");
    let right = build_balanced_journal_v1(records, len - left_len - 1);
    Some(Arc::new(JournalNodeV1::new(left, value, right)))
}

/// Immutable balanced journal with a lazily materialized canonical slice.
pub(crate) struct SharedJournalV1<T> {
    root: Option<Arc<JournalNodeV1<T>>>,
    len: usize,
    flat: Option<Arc<Once<Vec<T>>>>,
}

impl<T> SharedJournalV1<T> {
    pub(crate) const fn new() -> Self {
        Self {
            root: None,
            len: 0,
            flat: None,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn iter(&self) -> SharedJournalIterV1<'_, T> {
        SharedJournalIterV1::new(self.root.as_deref(), self.len)
    }

    pub(crate) fn get(&self, mut index: usize) -> Option<&T> {
        let mut node = self.root.as_deref();
        while let Some(current) = node {
            let left_len = node_len_v1(&current.left);
            if index < left_len {
                node = current.left.as_deref();
            } else if index == left_len {
                return Some(&current.value);
            } else {
                index -= left_len + 1;
                node = current.right.as_deref();
            }
        }
        None
    }

    pub(crate) fn binary_search_by(
        &self,
        mut compare: impl FnMut(&T) -> core::cmp::Ordering,
    ) -> Result<usize, usize> {
        let mut left = 0;
        let mut right = self.len;
        while left < right {
            let middle = left + (right - left) / 2;
            let record = self.get(middle).expect("journal binary-search index");
            match compare(record) {
                core::cmp::Ordering::Less => left = middle + 1,
                core::cmp::Ordering::Greater => right = middle,
                core::cmp::Ordering::Equal => return Ok(middle),
            }
        }
        Err(left)
    }

    pub(crate) fn push(&mut self, value: T)
    where
        T: Clone,
    {
        self.insert(self.len, value);
    }

    pub(crate) fn insert(&mut self, index: usize, value: T)
    where
        T: Clone,
    {
        assert!(index <= self.len, "journal insertion index is in bounds");
        self.root = Some(insert_journal_record_v1(self.root.as_ref(), index, value));
        self.len += 1;
        self.flat = Some(Arc::new(Once::new()));
    }

    pub(crate) fn get_mut(&mut self, mut index: usize) -> Option<&mut T>
    where
        T: Clone,
    {
        self.flat = Some(Arc::new(Once::new()));
        let mut node = self.root.as_mut()?;
        loop {
            let current = Arc::make_mut(node);
            let left_len = node_len_v1(&current.left);
            if index < left_len {
                node = current.left.as_mut().expect("journal left index");
            } else if index == left_len {
                return Some(&mut current.value);
            } else {
                index -= left_len + 1;
                node = current.right.as_mut().expect("journal right index");
            }
        }
    }

    pub(crate) fn retain(&mut self, mut keep: impl FnMut(&T) -> bool)
    where
        T: Clone,
    {
        let records: Vec<_> = self
            .iter()
            .filter(|record| keep(record))
            .map(clone_journal_record_v1)
            .collect();
        *self = Self::from_vec(records);
    }

    fn from_vec(records: Vec<T>) -> Self {
        let len = records.len();
        let mut records = records.into_iter();
        Self {
            root: build_balanced_journal_v1(&mut records, len),
            len,
            flat: if len == 0 {
                None
            } else {
                Some(Arc::new(Once::new()))
            },
        }
    }

    pub(crate) fn as_slice(&self) -> &[T]
    where
        T: Clone,
    {
        let Some(flat) = &self.flat else {
            return &[];
        };
        flat.call_once(|| self.iter().map(clone_journal_record_v1).collect())
    }

    #[cfg(test)]
    fn shares_storage_with(&self, other: &Self) -> bool {
        match (&self.root, &other.root) {
            (None, None) => true,
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            (None, Some(_)) | (Some(_), None) => false,
        }
    }
}

impl<T> Clone for SharedJournalV1<T> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            len: self.len,
            flat: self.flat.clone(),
        }
    }
}

impl<T: Clone + fmt::Debug> fmt::Debug for SharedJournalV1<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_slice().fmt(formatter)
    }
}

impl<T: Clone + PartialEq> PartialEq for SharedJournalV1<T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Clone + Eq> Eq for SharedJournalV1<T> {}

impl<T: Clone> Deref for SharedJournalV1<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<'a, T> IntoIterator for &'a SharedJournalV1<T> {
    type Item = &'a T;
    type IntoIter = SharedJournalIterV1<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub(crate) struct SharedJournalIterV1<'a, T> {
    stack: Vec<&'a JournalNodeV1<T>>,
    remaining: usize,
}

impl<'a, T> SharedJournalIterV1<'a, T> {
    fn new(root: Option<&'a JournalNodeV1<T>>, remaining: usize) -> Self {
        let mut iter = Self {
            stack: Vec::new(),
            remaining,
        };
        iter.push_left(root);
        iter
    }

    fn push_left(&mut self, mut node: Option<&'a JournalNodeV1<T>>) {
        while let Some(current) = node {
            self.stack.push(current);
            node = current.left.as_deref();
        }
    }
}

impl<'a, T> Iterator for SharedJournalIterV1<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.stack.pop()?;
        self.push_left(node.right.as_deref());
        self.remaining -= 1;
        Some(&node.value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T> ExactSizeIterator for SharedJournalIterV1<'_, T> {}

struct JournalIndexNodeV1<K> {
    left: Option<Arc<Self>>,
    key: K,
    index: usize,
    right: Option<Arc<Self>>,
    height: u8,
}

impl<K: Copy> Clone for JournalIndexNodeV1<K> {
    fn clone(&self) -> Self {
        note_journal_record_copy_v1();
        Self {
            left: self.left.clone(),
            key: self.key,
            index: self.index,
            right: self.right.clone(),
            height: self.height,
        }
    }
}

impl<K> JournalIndexNodeV1<K> {
    fn new(left: Option<Arc<Self>>, key: K, index: usize, right: Option<Arc<Self>>) -> Self {
        Self {
            height: 1 + index_height_v1(&left).max(index_height_v1(&right)),
            left,
            key,
            index,
            right,
        }
    }
}

fn index_height_v1<K>(node: &Option<Arc<JournalIndexNodeV1<K>>>) -> u8 {
    node.as_ref().map_or(0, |node| node.height)
}

fn rotate_index_left_v1<K: Copy>(root: &Arc<JournalIndexNodeV1<K>>) -> Arc<JournalIndexNodeV1<K>> {
    let pivot = root.right.as_ref().expect("right-heavy journal index");
    note_journal_record_copy_v1();
    let left = Arc::new(JournalIndexNodeV1::new(
        root.left.clone(),
        root.key,
        root.index,
        pivot.left.clone(),
    ));
    note_journal_record_copy_v1();
    Arc::new(JournalIndexNodeV1::new(
        Some(left),
        pivot.key,
        pivot.index,
        pivot.right.clone(),
    ))
}

fn rotate_index_right_v1<K: Copy>(root: &Arc<JournalIndexNodeV1<K>>) -> Arc<JournalIndexNodeV1<K>> {
    let pivot = root.left.as_ref().expect("left-heavy journal index");
    note_journal_record_copy_v1();
    let right = Arc::new(JournalIndexNodeV1::new(
        pivot.right.clone(),
        root.key,
        root.index,
        root.right.clone(),
    ));
    note_journal_record_copy_v1();
    Arc::new(JournalIndexNodeV1::new(
        pivot.left.clone(),
        pivot.key,
        pivot.index,
        Some(right),
    ))
}

fn balance_index_node_v1<K: Copy>(
    mut root: Arc<JournalIndexNodeV1<K>>,
) -> Arc<JournalIndexNodeV1<K>> {
    let balance = i16::from(index_height_v1(&root.left)) - i16::from(index_height_v1(&root.right));
    if balance > 1 {
        let left = root.left.as_ref().expect("left-heavy journal index");
        if index_height_v1(&left.right) > index_height_v1(&left.left) {
            note_journal_record_copy_v1();
            root = Arc::new(JournalIndexNodeV1::new(
                Some(rotate_index_left_v1(left)),
                root.key,
                root.index,
                root.right.clone(),
            ));
        }
        rotate_index_right_v1(&root)
    } else if balance < -1 {
        let right = root.right.as_ref().expect("right-heavy journal index");
        if index_height_v1(&right.left) > index_height_v1(&right.right) {
            note_journal_record_copy_v1();
            root = Arc::new(JournalIndexNodeV1::new(
                root.left.clone(),
                root.key,
                root.index,
                Some(rotate_index_right_v1(right)),
            ));
        }
        rotate_index_left_v1(&root)
    } else {
        root
    }
}

fn insert_index_entry_v1<K: Copy + Ord>(
    root: Option<&Arc<JournalIndexNodeV1<K>>>,
    key: K,
    index: usize,
) -> (Arc<JournalIndexNodeV1<K>>, bool) {
    let Some(root) = root else {
        return (
            Arc::new(JournalIndexNodeV1::new(None, key, index, None)),
            true,
        );
    };
    let (rebuilt, inserted) = match key.cmp(&root.key) {
        core::cmp::Ordering::Less => {
            let (left, inserted) = insert_index_entry_v1(root.left.as_ref(), key, index);
            note_journal_record_copy_v1();
            (
                Arc::new(JournalIndexNodeV1::new(
                    Some(left),
                    root.key,
                    root.index,
                    root.right.clone(),
                )),
                inserted,
            )
        }
        core::cmp::Ordering::Greater => {
            let (right, inserted) = insert_index_entry_v1(root.right.as_ref(), key, index);
            note_journal_record_copy_v1();
            (
                Arc::new(JournalIndexNodeV1::new(
                    root.left.clone(),
                    root.key,
                    root.index,
                    Some(right),
                )),
                inserted,
            )
        }
        core::cmp::Ordering::Equal => (root.clone(), false),
    };
    (balance_index_node_v1(rebuilt), inserted)
}

struct PersistentJournalIndexV1<K> {
    root: Option<Arc<JournalIndexNodeV1<K>>>,
}

impl<K> Clone for PersistentJournalIndexV1<K> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
        }
    }
}

impl<K> PersistentJournalIndexV1<K> {
    const fn new() -> Self {
        Self { root: None }
    }
}

impl<K: Copy + Ord> PersistentJournalIndexV1<K> {
    fn get(&self, key: K) -> Option<usize> {
        let mut node = self.root.as_deref();
        while let Some(current) = node {
            match key.cmp(&current.key) {
                core::cmp::Ordering::Less => node = current.left.as_deref(),
                core::cmp::Ordering::Greater => node = current.right.as_deref(),
                core::cmp::Ordering::Equal => return Some(current.index),
            }
        }
        None
    }

    fn insert(&mut self, key: K, index: usize) -> bool {
        let (root, inserted) = insert_index_entry_v1(self.root.as_ref(), key, index);
        self.root = Some(root);
        inserted
    }
}

/// Append-ordered persistent journal with a separately key-ordered index.
pub(crate) struct IndexedJournalV1<K, T> {
    records: SharedJournalV1<T>,
    index: PersistentJournalIndexV1<K>,
    key_of: fn(&T) -> K,
}

impl<K, T> IndexedJournalV1<K, T> {
    pub(crate) const fn new(key_of: fn(&T) -> K) -> Self {
        Self {
            records: SharedJournalV1::new(),
            index: PersistentJournalIndexV1::new(),
            key_of,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.records.len()
    }

    pub(crate) fn iter(&self) -> SharedJournalIterV1<'_, T> {
        self.records.iter()
    }

    pub(crate) fn as_slice(&self) -> &[T]
    where
        T: Clone,
    {
        self.records.as_slice()
    }

    #[cfg(test)]
    fn shares_storage_with(&self, other: &Self) -> bool {
        self.records.shares_storage_with(&other.records)
    }
}

impl<K: Copy + Ord, T: Clone> IndexedJournalV1<K, T> {
    pub(crate) fn push(&mut self, record: T) {
        let key = (self.key_of)(&record);
        let index = self.records.len();
        assert!(self.index.insert(key, index), "journal keys are unique");
        self.records.push(record);
    }

    pub(crate) fn get(&self, key: K) -> Option<&T> {
        self.index
            .get(key)
            .and_then(|index| self.records.get(index))
    }

    pub(crate) fn get_mut(&mut self, key: K) -> Option<&mut T> {
        let index = self.index.get(key)?;
        self.records.get_mut(index)
    }

    pub(crate) fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        self.records.retain(|record| keep(record));
        self.index = PersistentJournalIndexV1::new();
        for (index, record) in self.records.iter().enumerate() {
            assert!(
                self.index.insert((self.key_of)(record), index),
                "retained journal keys are unique"
            );
        }
    }
}

impl<K, T> Clone for IndexedJournalV1<K, T> {
    fn clone(&self) -> Self {
        Self {
            records: self.records.clone(),
            index: self.index.clone(),
            key_of: self.key_of,
        }
    }
}

impl<K, T: Clone + fmt::Debug> fmt::Debug for IndexedJournalV1<K, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.records.fmt(formatter)
    }
}

impl<K, T: Clone + PartialEq> PartialEq for IndexedJournalV1<K, T> {
    fn eq(&self, other: &Self) -> bool {
        self.records == other.records
    }
}

impl<K, T: Clone + Eq> Eq for IndexedJournalV1<K, T> {}

impl<K, T: Clone> Deref for IndexedJournalV1<K, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<'a, K, T> IntoIterator for &'a IndexedJournalV1<K, T> {
    type Item = &'a T;
    type IntoIter = SharedJournalIterV1<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Untrusted process-local VM handle observation. It is not durable identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct UntrustedVmHandleObservationV1(pub u64);

/// Untrusted KFD allocation-handle observation. It is bound to an allocation
/// generation but does not grant allocation or file-descriptor authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct UntrustedAllocationHandleObservationV1(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VaReservationKeyV1 {
    pub vm: VmKeyV1,
    pub id: VaReservationIdV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryAllocationKeyV1 {
    pub vm: VmKeyV1,
    pub id: AllocationIdV1,
    pub generation: AllocationGenerationV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryMappingKeyV1 {
    pub allocation: MemoryAllocationKeyV1,
    pub id: MappingIdV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryPublicationKeyV1 {
    pub mapping: MemoryMappingKeyV1,
    pub id: MemoryPublicationIdV1,
}

/// Half-open GPU virtual-address interval `[base, base + byte_len)`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpuVaRangeV1 {
    pub base: u64,
    pub byte_len: u64,
}

impl GpuVaRangeV1 {
    pub const fn checked_end(self) -> Option<u64> {
        self.base.checked_add(self.byte_len)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryKindV1 {
    HostVisibleCoherent,
    /// Device-local storage whose host/device visibility is established only
    /// by an explicit completed transfer.
    DeviceLocal,
    QueueStorage,
    Kernarg,
    Executable,
    /// Queue-owned private-segment or context-save backing. This is never an
    /// ordinary application buffer.
    ScratchContextSave,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryCoherenceV1 {
    HostCoherent,
    ExplicitVisibility,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryAllocationSpecV1 {
    pub byte_len: u64,
    pub alignment: u64,
    pub kind: MemoryKindV1,
    pub coherence: MemoryCoherenceV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryVmStateV1 {
    Active,
    Retired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VaReservationStateV1 {
    Reserved,
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryAllocationStateV1 {
    Live,
    Released,
}

/// State of one exact map/unmap device-list operation.
///
/// The retained indices describe the conservative device subrange that may
/// still be mapped. Map progress establishes `[0, mapped_end)`. Unmap progress
/// reports an absolute cumulative prefix boundary and assigns `mapped_start`
/// to that boundary. `Ambiguous` never permits release, regardless of the
/// retained range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryMappingStateV1 {
    MapPending,
    MapFailed,
    Mapped,
    UnmapPending,
    UnmapFailed,
    Unmapped,
    Ambiguous,
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryPublicationStateV1 {
    Live,
    Released,
}

/// Structural owner of a mapping-retention publication.
///
/// Queue-owned publications can only be minted and released by the joint queue
/// lifecycle transition. Public generic memory transitions cannot discharge
/// them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryPublicationOwnerV1 {
    Generic,
    ComputeAqlQueue(QueueKeyV1),
    PeerTransfer(PeerTransferPublicationOwnerV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PartialOperationStatusV1 {
    Succeeded,
    Failed,
    Indeterminate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PartialProgressObservationV1 {
    pub n_success: usize,
    pub status: PartialOperationStatusV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryVmRecordV1 {
    pub admission: ModelVmAdmissionV1,
    pub mapping_devices: Vec<ModelDeviceAdmissionV1>,
    pub handle: UntrustedVmHandleObservationV1,
    pub aperture: GpuVaRangeV1,
    pub state: MemoryVmStateV1,
}

impl MemoryVmRecordV1 {
    pub fn mapping_device_keys(&self) -> impl ExactSizeIterator<Item = DeviceKeyV1> + '_ {
        self.mapping_devices
            .iter()
            .map(|admission| admission.model_key())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VaReservationRecordV1 {
    pub key: VaReservationKeyV1,
    pub range: GpuVaRangeV1,
    pub alignment: u64,
    pub state: VaReservationStateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryAllocationRecordV1 {
    pub key: MemoryAllocationKeyV1,
    pub reservation: VaReservationKeyV1,
    pub handle: UntrustedAllocationHandleObservationV1,
    pub spec: MemoryAllocationSpecV1,
    pub state: MemoryAllocationStateV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryMappingRecordV1 {
    pub key: MemoryMappingKeyV1,
    pub target_devices: Vec<DeviceKeyV1>,
    pub access: MemoryAccessV1,
    pub mapped_start: usize,
    pub mapped_end: usize,
    pub state: MemoryMappingStateV1,
}

impl MemoryMappingRecordV1 {
    /// Conservative device subrange retained against unmap/free.
    pub fn retained_device_superset(&self) -> &[DeviceKeyV1] {
        &self.target_devices[self.mapped_start..self.mapped_end]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryPublicationRecordV1 {
    pub key: MemoryPublicationKeyV1,
    pub owner: MemoryPublicationOwnerV1,
    pub state: MemoryPublicationStateV1,
}

/// Identity policy for one memory lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryIdentityDisciplineV1 {
    /// Allocation generations can reuse a released allocation ID. History is
    /// retained and checkpointing is unavailable.
    ReusableGenerations,
    /// Every hierarchical ID must exceed the issued high-watermark in its
    /// scope. Fully released history can therefore be checkpointed safely.
    MonotonicNonReusable,
}

/// Hierarchical identity scope retained by the monotonic identity discipline.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MemoryIssuedIdScopeV1 {
    VaReservation(VmKeyV1),
    Allocation(VmKeyV1),
    Mapping(MemoryAllocationKeyV1),
    Publication(MemoryMappingKeyV1),
}

/// Canonical issued-ID high-watermark for one monotonic identity scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryIssuedIdHighWatermarkV1 {
    pub scope: MemoryIssuedIdScopeV1,
    pub last_id: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryRecordKindV1 {
    Vm,
    VaReservation,
    Allocation,
    Mapping,
    Publication,
    IssuedIdHighWatermark,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryRecordRefV1 {
    Vm(VmKeyV1),
    VaReservation(VaReservationKeyV1),
    Allocation(MemoryAllocationKeyV1),
    Mapping(MemoryMappingKeyV1),
    Publication(MemoryPublicationKeyV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryInvariantViolationV1 {
    CapacityExceeded(MemoryRecordKindV1),
    DomainMismatch(MemoryRecordRefV1),
    Duplicate(MemoryRecordRefV1),
    MissingParent(MemoryRecordRefV1),
    BindingMismatch(MemoryRecordRefV1),
    InvalidIdentity(MemoryRecordRefV1),
    InvalidRange(MemoryRecordRefV1),
    InvalidAlignment(MemoryRecordRefV1),
    DeviceSetMismatch(MemoryRecordRefV1),
    HandleCollision(MemoryRecordRefV1),
    StaleGeneration(MemoryAllocationKeyV1),
    AddressOverlap(VaReservationKeyV1, VaReservationKeyV1),
    InvalidState(MemoryRecordRefV1),
    EarlyRelease(MemoryRecordRefV1),
    InvalidIssuedIdHighWatermark(MemoryIssuedIdHighWatermarkV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryTransitionErrorV1 {
    SourceInvariant(MemoryInvariantViolationV1),
    NextInvariant(MemoryInvariantViolationV1),
    CapacityExceeded {
        kind: MemoryRecordKindV1,
        maximum: usize,
    },
    NotFound(MemoryRecordRefV1),
    AlreadyExists(MemoryRecordRefV1),
    ObservationDomainMismatch,
    InvalidIdentity(MemoryRecordRefV1),
    InvalidRange(MemoryRecordRefV1),
    InvalidAlignment(MemoryRecordRefV1),
    BindingMismatch(MemoryRecordRefV1),
    DeviceSetMismatch(MemoryRecordRefV1),
    HandleCollision(MemoryRecordRefV1),
    StaleGeneration(MemoryAllocationKeyV1),
    NonMonotonicIdentity(MemoryRecordRefV1),
    AddressConflict(VaReservationKeyV1),
    IllegalState(MemoryRecordRefV1),
    ResourceInUse(MemoryRecordRefV1),
    CheckpointRequiresMonotonicIdentities,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryTransitionV1 {
    AcquireVm {
        admission: ModelVmAdmissionV1,
        mapping_devices: Vec<ModelDeviceAdmissionV1>,
        handle: UntrustedVmHandleObservationV1,
        aperture: GpuVaRangeV1,
    },
    RetireVm {
        key: VmKeyV1,
    },
    ReserveVa {
        key: VaReservationKeyV1,
        range: GpuVaRangeV1,
        alignment: u64,
    },
    ReleaseVaReservation {
        key: VaReservationKeyV1,
    },
    Allocate {
        key: MemoryAllocationKeyV1,
        reservation: VaReservationKeyV1,
        handle: UntrustedAllocationHandleObservationV1,
        spec: MemoryAllocationSpecV1,
    },
    ReleaseAllocation {
        key: MemoryAllocationKeyV1,
    },
    BeginMap {
        key: MemoryMappingKeyV1,
        target_devices: Vec<DeviceKeyV1>,
        access: MemoryAccessV1,
    },
    ObserveMap {
        key: MemoryMappingKeyV1,
        progress: PartialProgressObservationV1,
    },
    BeginUnmap {
        key: MemoryMappingKeyV1,
    },
    ObserveUnmap {
        key: MemoryMappingKeyV1,
        progress: PartialProgressObservationV1,
    },
    ReleaseMapping {
        key: MemoryMappingKeyV1,
    },
    PublishMapping {
        key: MemoryPublicationKeyV1,
    },
    ReleasePublication {
        key: MemoryPublicationKeyV1,
    },
}

fn vm_record_key_v1(record: &MemoryVmRecordV1) -> VmKeyV1 {
    record.admission.model_key()
}

const fn reservation_record_key_v1(record: &VaReservationRecordV1) -> VaReservationKeyV1 {
    record.key
}

const fn allocation_record_key_v1(record: &MemoryAllocationRecordV1) -> MemoryAllocationKeyV1 {
    record.key
}

const fn mapping_record_key_v1(record: &MemoryMappingRecordV1) -> MemoryMappingKeyV1 {
    record.key
}

const fn publication_record_key_v1(record: &MemoryPublicationRecordV1) -> MemoryPublicationKeyV1 {
    record.key
}

/// Bounded process-domain memory history.
///
/// All inputs and receipts remain model-only. External adapters must translate
/// every possibly side-effecting malformed/unknown result to `Indeterminate`;
/// rejecting a pure transition alone is not rollback evidence for a syscall.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryLifecycleStateV1 {
    domain_id: DeviceObservationDomainIdV1,
    identity_discipline: MemoryIdentityDisciplineV1,
    vms: IndexedJournalV1<VmKeyV1, MemoryVmRecordV1>,
    reservations: IndexedJournalV1<VaReservationKeyV1, VaReservationRecordV1>,
    allocations: IndexedJournalV1<MemoryAllocationKeyV1, MemoryAllocationRecordV1>,
    mappings: IndexedJournalV1<MemoryMappingKeyV1, MemoryMappingRecordV1>,
    publications: IndexedJournalV1<MemoryPublicationKeyV1, MemoryPublicationRecordV1>,
    issued_id_high_watermarks: SharedJournalV1<MemoryIssuedIdHighWatermarkV1>,
}

impl MemoryLifecycleStateV1 {
    pub const fn new(domain_id: DeviceObservationDomainIdV1) -> Self {
        Self {
            domain_id,
            identity_discipline: MemoryIdentityDisciplineV1::ReusableGenerations,
            vms: IndexedJournalV1::new(vm_record_key_v1),
            reservations: IndexedJournalV1::new(reservation_record_key_v1),
            allocations: IndexedJournalV1::new(allocation_record_key_v1),
            mappings: IndexedJournalV1::new(mapping_record_key_v1),
            publications: IndexedJournalV1::new(publication_record_key_v1),
            issued_id_high_watermarks: SharedJournalV1::new(),
        }
    }

    /// Creates a state whose process-local memory identities can never be
    /// reused. This is the discipline required by bounded checkpointing.
    pub const fn new_monotonic_non_reusable(domain_id: DeviceObservationDomainIdV1) -> Self {
        Self {
            domain_id,
            identity_discipline: MemoryIdentityDisciplineV1::MonotonicNonReusable,
            vms: IndexedJournalV1::new(vm_record_key_v1),
            reservations: IndexedJournalV1::new(reservation_record_key_v1),
            allocations: IndexedJournalV1::new(allocation_record_key_v1),
            mappings: IndexedJournalV1::new(mapping_record_key_v1),
            publications: IndexedJournalV1::new(publication_record_key_v1),
            issued_id_high_watermarks: SharedJournalV1::new(),
        }
    }

    pub const fn authority_domain(&self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }

    pub const fn domain_id(&self) -> DeviceObservationDomainIdV1 {
        self.domain_id
    }

    pub const fn identity_discipline(&self) -> MemoryIdentityDisciplineV1 {
        self.identity_discipline
    }

    pub fn vms(&self) -> &[MemoryVmRecordV1] {
        self.vms.as_slice()
    }

    pub fn reservations(&self) -> &[VaReservationRecordV1] {
        self.reservations.as_slice()
    }

    pub fn allocations(&self) -> &[MemoryAllocationRecordV1] {
        self.allocations.as_slice()
    }

    pub fn mappings(&self) -> &[MemoryMappingRecordV1] {
        self.mappings.as_slice()
    }

    pub fn publications(&self) -> &[MemoryPublicationRecordV1] {
        self.publications.as_slice()
    }

    /// Returns canonical authenticated issued-ID high-watermarks.
    pub fn issued_id_high_watermarks(&self) -> &[MemoryIssuedIdHighWatermarkV1] {
        self.issued_id_high_watermarks.as_slice()
    }

    /// Removes fully released journal records without making their identities
    /// reusable. Retired parent identities subsume all descendant history.
    pub fn checkpoint_released(&self) -> Result<Self, MemoryTransitionErrorV1> {
        self.validate_global_invariants()
            .map_err(MemoryTransitionErrorV1::SourceInvariant)?;
        if self.identity_discipline != MemoryIdentityDisciplineV1::MonotonicNonReusable {
            return Err(MemoryTransitionErrorV1::CheckpointRequiresMonotonicIdentities);
        }

        let compact_allocations: Vec<_> = self
            .allocations
            .iter()
            .filter(|record| record.state == MemoryAllocationStateV1::Released)
            .map(|record| record.key)
            .collect();
        let compact_mappings: Vec<_> = self
            .mappings
            .iter()
            .filter(|record| record.state == MemoryMappingStateV1::Released)
            .map(|record| record.key)
            .collect();
        let compact_publications: Vec<_> = self
            .publications
            .iter()
            .filter(|record| record.state == MemoryPublicationStateV1::Released)
            .map(|record| record.key)
            .collect();
        let compact_reservations: Vec<_> = self
            .reservations
            .iter()
            .filter(|record| {
                record.state == VaReservationStateV1::Released
                    && self.allocations.iter().all(|allocation| {
                        allocation.reservation != record.key
                            || compact_allocations.contains(&allocation.key)
                    })
            })
            .map(|record| record.key)
            .collect();

        let mut next = self.clone();
        next.publications
            .retain(|record| !compact_publications.contains(&record.key));
        next.mappings
            .retain(|record| !compact_mappings.contains(&record.key));
        next.allocations
            .retain(|record| !compact_allocations.contains(&record.key));
        next.reservations
            .retain(|record| !compact_reservations.contains(&record.key));

        let retained_allocations: Vec<_> =
            next.allocations.iter().map(|record| record.key).collect();
        let retained_mappings: Vec<_> = next.mappings.iter().map(|record| record.key).collect();
        next.issued_id_high_watermarks
            .retain(|watermark| match watermark.scope {
                MemoryIssuedIdScopeV1::Mapping(allocation) => {
                    retained_allocations.contains(&allocation)
                }
                MemoryIssuedIdScopeV1::Publication(mapping) => retained_mappings.contains(&mapping),
                MemoryIssuedIdScopeV1::VaReservation(_) | MemoryIssuedIdScopeV1::Allocation(_) => {
                    true
                }
            });
        next.validate_global_invariants()
            .map_err(MemoryTransitionErrorV1::NextInvariant)?;
        Ok(next)
    }

    #[cfg(test)]
    pub(crate) fn with_generic_publications_for_test(
        &self,
        mapping: MemoryMappingKeyV1,
        count: usize,
    ) -> Self {
        let mut next = self.clone();
        for offset in 0..count {
            next.publications.push(MemoryPublicationRecordV1 {
                key: MemoryPublicationKeyV1 {
                    mapping,
                    id: MemoryPublicationIdV1(10_000 + offset as u64),
                },
                owner: MemoryPublicationOwnerV1::Generic,
                state: MemoryPublicationStateV1::Live,
            });
        }
        assert!(next.validate_global_invariants().is_ok());
        next
    }

    #[cfg(test)]
    pub(crate) fn shared_journals_for_test(&self, other: &Self) -> [bool; 6] {
        [
            self.vms.shares_storage_with(&other.vms),
            self.reservations.shares_storage_with(&other.reservations),
            self.allocations.shares_storage_with(&other.allocations),
            self.mappings.shares_storage_with(&other.mappings),
            self.publications.shares_storage_with(&other.publications),
            self.issued_id_high_watermarks
                .shares_storage_with(&other.issued_id_high_watermarks),
        ]
    }

    /// Applies one locally checked transition to a copy-on-write snapshot.
    ///
    /// State fields are sealed and every mutation is admitted by `apply`, so
    /// the global invariants are inductive. Call `validate_global_invariants`
    /// explicitly at trust boundaries and in model-checking tests; repeating
    /// the whole-journal scan here would make a trace quadratic.
    pub fn next(&self, transition: MemoryTransitionV1) -> Result<Self, MemoryTransitionErrorV1> {
        let mut next = self.clone();
        next.apply(transition)?;
        Ok(next)
    }

    pub(crate) fn publish_compute_aql_queue_mappings(
        &self,
        keys: impl IntoIterator<Item = MemoryPublicationKeyV1>,
        queue: QueueKeyV1,
    ) -> Result<Self, MemoryTransitionErrorV1> {
        let mut next = self.clone();
        for key in keys {
            next.publish_mapping(key, MemoryPublicationOwnerV1::ComputeAqlQueue(queue))?;
        }
        Ok(next)
    }

    pub(crate) fn release_compute_aql_queue_publications(
        &self,
        keys: impl IntoIterator<Item = MemoryPublicationKeyV1>,
        queue: QueueKeyV1,
    ) -> Result<Self, MemoryTransitionErrorV1> {
        let mut next = self.clone();
        for key in keys {
            next.release_queue_publication(key, queue)?;
        }
        Ok(next)
    }

    pub(crate) fn publish_peer_transfer_mapping(
        &self,
        key: MemoryPublicationKeyV1,
        binding: PeerTransferBindingV1,
    ) -> Result<Self, MemoryTransitionErrorV1> {
        if !binding.retains_publication(key) {
            return Err(MemoryTransitionErrorV1::BindingMismatch(
                MemoryRecordRefV1::Publication(key),
            ));
        }
        let mut next = self.clone();
        next.publish_mapping(
            key,
            MemoryPublicationOwnerV1::PeerTransfer(binding.publication_owner()),
        )?;
        Ok(next)
    }

    pub(crate) fn release_peer_transfer_publication(
        &self,
        key: MemoryPublicationKeyV1,
        binding: PeerTransferBindingV1,
    ) -> Result<Self, MemoryTransitionErrorV1> {
        if !binding.retains_publication(key) {
            return Err(MemoryTransitionErrorV1::BindingMismatch(
                MemoryRecordRefV1::Publication(key),
            ));
        }
        let mut next = self.clone();
        next.release_peer_publication(key, binding)?;
        Ok(next)
    }

    pub fn validate_global_invariants(&self) -> Result<(), MemoryInvariantViolationV1> {
        self.validate_capacities()?;
        self.validate_vms()?;
        self.validate_reservations()?;
        self.validate_allocations()?;
        self.validate_mappings()?;
        self.validate_publications()?;
        self.validate_release_order()?;
        self.validate_issued_id_high_watermarks()?;
        Ok(())
    }

    fn apply(&mut self, transition: MemoryTransitionV1) -> Result<(), MemoryTransitionErrorV1> {
        match transition {
            MemoryTransitionV1::AcquireVm {
                admission,
                mapping_devices,
                handle,
                aperture,
            } => self.acquire_vm(admission, mapping_devices, handle, aperture),
            MemoryTransitionV1::RetireVm { key } => self.retire_vm(key),
            MemoryTransitionV1::ReserveVa {
                key,
                range,
                alignment,
            } => self.reserve_va(key, range, alignment),
            MemoryTransitionV1::ReleaseVaReservation { key } => self.release_reservation(key),
            MemoryTransitionV1::Allocate {
                key,
                reservation,
                handle,
                spec,
            } => self.allocate(key, reservation, handle, spec),
            MemoryTransitionV1::ReleaseAllocation { key } => self.release_allocation(key),
            MemoryTransitionV1::BeginMap {
                key,
                target_devices,
                access,
            } => self.begin_map(key, target_devices, access),
            MemoryTransitionV1::ObserveMap { key, progress } => self.observe_map(key, progress),
            MemoryTransitionV1::BeginUnmap { key } => self.begin_unmap(key),
            MemoryTransitionV1::ObserveUnmap { key, progress } => self.observe_unmap(key, progress),
            MemoryTransitionV1::ReleaseMapping { key } => self.release_mapping(key),
            MemoryTransitionV1::PublishMapping { key } => {
                self.publish_mapping(key, MemoryPublicationOwnerV1::Generic)
            }
            MemoryTransitionV1::ReleasePublication { key } => self.release_publication(key),
        }
    }

    fn acquire_vm(
        &mut self,
        admission: ModelVmAdmissionV1,
        mapping_devices: Vec<ModelDeviceAdmissionV1>,
        handle: UntrustedVmHandleObservationV1,
        aperture: GpuVaRangeV1,
    ) -> Result<(), MemoryTransitionErrorV1> {
        ensure_memory_room(self.vms.len(), MAX_MEMORY_VMS_V1, MemoryRecordKindV1::Vm)?;
        let key = admission.model_key();
        let reference = MemoryRecordRefV1::Vm(key);
        if admission.domain_id() != self.domain_id
            || mapping_devices
                .iter()
                .any(|device| device.domain_id() != self.domain_id)
        {
            return Err(MemoryTransitionErrorV1::ObservationDomainMismatch);
        }
        if key.id.0 == 0 || handle.0 == 0 {
            return Err(MemoryTransitionErrorV1::InvalidIdentity(reference));
        }
        if self
            .vms
            .iter()
            .any(|record| record.admission.model_key() == key)
        {
            return Err(MemoryTransitionErrorV1::AlreadyExists(reference));
        }
        if self
            .vms
            .iter()
            .any(|record| record.state == MemoryVmStateV1::Active && record.handle == handle)
        {
            return Err(MemoryTransitionErrorV1::HandleCollision(reference));
        }
        if !valid_mapping_admissions(&mapping_devices, key.device) {
            return Err(MemoryTransitionErrorV1::DeviceSetMismatch(reference));
        }
        if !valid_page_range(aperture) {
            return Err(MemoryTransitionErrorV1::InvalidRange(reference));
        }
        self.vms.push(MemoryVmRecordV1 {
            admission,
            mapping_devices,
            handle,
            aperture,
            state: MemoryVmStateV1::Active,
        });
        Ok(())
    }

    fn retire_vm(&mut self, key: VmKeyV1) -> Result<(), MemoryTransitionErrorV1> {
        let reference = MemoryRecordRefV1::Vm(key);
        if self.vm(key)?.state != MemoryVmStateV1::Active {
            return Err(MemoryTransitionErrorV1::IllegalState(reference));
        }
        if self
            .reservations
            .iter()
            .any(|record| record.key.vm == key && record.state != VaReservationStateV1::Released)
            || self.allocations.iter().any(|record| {
                record.key.vm == key && record.state != MemoryAllocationStateV1::Released
            })
            || self.mappings.iter().any(|record| {
                record.key.allocation.vm == key && record.state != MemoryMappingStateV1::Released
            })
            || self.publications.iter().any(|record| {
                record.key.mapping.allocation.vm == key
                    && record.state != MemoryPublicationStateV1::Released
            })
        {
            return Err(MemoryTransitionErrorV1::ResourceInUse(reference));
        }
        self.vm_mut(key)?.state = MemoryVmStateV1::Retired;
        Ok(())
    }

    fn reserve_va(
        &mut self,
        key: VaReservationKeyV1,
        range: GpuVaRangeV1,
        alignment: u64,
    ) -> Result<(), MemoryTransitionErrorV1> {
        ensure_memory_room(
            self.reservations.len(),
            MAX_VA_RESERVATIONS_V1,
            MemoryRecordKindV1::VaReservation,
        )?;
        let reference = MemoryRecordRefV1::VaReservation(key);
        self.require_vm_active(key.vm)?;
        if key.id.0 == 0 {
            return Err(MemoryTransitionErrorV1::InvalidIdentity(reference));
        }
        if self.reservations.iter().any(|record| record.key == key) {
            return Err(MemoryTransitionErrorV1::AlreadyExists(reference));
        }
        self.require_next_issued_id(
            MemoryIssuedIdScopeV1::VaReservation(key.vm),
            key.id.0,
            reference,
        )?;
        if !valid_alignment(alignment) || !range.base.is_multiple_of(alignment) {
            return Err(MemoryTransitionErrorV1::InvalidAlignment(reference));
        }
        if !valid_page_range(range) || !range_within(range, self.vm(key.vm)?.aperture) {
            return Err(MemoryTransitionErrorV1::InvalidRange(reference));
        }
        if self.reservations.iter().any(|record| {
            record.key.vm == key.vm
                && record.state == VaReservationStateV1::Reserved
                && ranges_overlap(record.range, range)
        }) {
            return Err(MemoryTransitionErrorV1::AddressConflict(key));
        }
        self.reservations.push(VaReservationRecordV1 {
            key,
            range,
            alignment,
            state: VaReservationStateV1::Reserved,
        });
        self.record_issued_id(MemoryIssuedIdScopeV1::VaReservation(key.vm), key.id.0);
        Ok(())
    }

    fn release_reservation(
        &mut self,
        key: VaReservationKeyV1,
    ) -> Result<(), MemoryTransitionErrorV1> {
        let reference = MemoryRecordRefV1::VaReservation(key);
        if self.reservation(key)?.state != VaReservationStateV1::Reserved {
            return Err(MemoryTransitionErrorV1::IllegalState(reference));
        }
        if self.allocations.iter().any(|record| {
            record.reservation == key && record.state != MemoryAllocationStateV1::Released
        }) {
            return Err(MemoryTransitionErrorV1::ResourceInUse(reference));
        }
        self.reservation_mut(key)?.state = VaReservationStateV1::Released;
        Ok(())
    }

    fn allocate(
        &mut self,
        key: MemoryAllocationKeyV1,
        reservation: VaReservationKeyV1,
        handle: UntrustedAllocationHandleObservationV1,
        spec: MemoryAllocationSpecV1,
    ) -> Result<(), MemoryTransitionErrorV1> {
        ensure_memory_room(
            self.allocations.len(),
            MAX_MEMORY_ALLOCATIONS_V1,
            MemoryRecordKindV1::Allocation,
        )?;
        let reference = MemoryRecordRefV1::Allocation(key);
        self.require_vm_active(key.vm)?;
        if key.vm != reservation.vm {
            return Err(MemoryTransitionErrorV1::BindingMismatch(reference));
        }
        if key.id.0 == 0 || key.generation.0 == 0 || handle.0 == 0 {
            return Err(MemoryTransitionErrorV1::InvalidIdentity(reference));
        }
        if self.allocations.iter().any(|record| record.key == key) {
            return Err(MemoryTransitionErrorV1::AlreadyExists(reference));
        }
        self.require_next_issued_id(
            MemoryIssuedIdScopeV1::Allocation(key.vm),
            key.id.0,
            reference,
        )?;
        let reservation_record = self.reservation(reservation)?;
        if reservation_record.state != VaReservationStateV1::Reserved {
            return Err(MemoryTransitionErrorV1::IllegalState(
                MemoryRecordRefV1::VaReservation(reservation),
            ));
        }
        if !valid_alignment(spec.alignment)
            || !reservation_record.range.base.is_multiple_of(spec.alignment)
            || spec.alignment > reservation_record.alignment
        {
            return Err(MemoryTransitionErrorV1::InvalidAlignment(reference));
        }
        if !valid_allocation_spec(spec, *reservation_record) {
            return Err(MemoryTransitionErrorV1::InvalidRange(reference));
        }
        for old in self
            .allocations
            .iter()
            .filter(|record| record.key.vm == key.vm && record.key.id == key.id)
        {
            if old.state != MemoryAllocationStateV1::Released {
                return Err(MemoryTransitionErrorV1::ResourceInUse(
                    MemoryRecordRefV1::Allocation(old.key),
                ));
            }
            if old.key.generation >= key.generation {
                return Err(MemoryTransitionErrorV1::StaleGeneration(key));
            }
        }
        if self.allocations.iter().any(|record| {
            record.state == MemoryAllocationStateV1::Live
                && (record.reservation == reservation || record.handle == handle)
        }) {
            return Err(MemoryTransitionErrorV1::HandleCollision(reference));
        }
        self.allocations.push(MemoryAllocationRecordV1 {
            key,
            reservation,
            handle,
            spec,
            state: MemoryAllocationStateV1::Live,
        });
        self.record_issued_id(MemoryIssuedIdScopeV1::Allocation(key.vm), key.id.0);
        Ok(())
    }

    fn release_allocation(
        &mut self,
        key: MemoryAllocationKeyV1,
    ) -> Result<(), MemoryTransitionErrorV1> {
        let reference = MemoryRecordRefV1::Allocation(key);
        if self.allocation(key)?.state != MemoryAllocationStateV1::Live {
            return Err(MemoryTransitionErrorV1::IllegalState(reference));
        }
        if self.mappings.iter().any(|record| {
            record.key.allocation == key && record.state != MemoryMappingStateV1::Released
        }) || self.publications.iter().any(|record| {
            record.key.mapping.allocation == key
                && record.state != MemoryPublicationStateV1::Released
        }) {
            return Err(MemoryTransitionErrorV1::ResourceInUse(reference));
        }
        self.allocation_mut(key)?.state = MemoryAllocationStateV1::Released;
        Ok(())
    }

    fn begin_map(
        &mut self,
        key: MemoryMappingKeyV1,
        target_devices: Vec<DeviceKeyV1>,
        access: MemoryAccessV1,
    ) -> Result<(), MemoryTransitionErrorV1> {
        ensure_memory_room(
            self.mappings.len(),
            MAX_MEMORY_MAPPINGS_V1,
            MemoryRecordKindV1::Mapping,
        )?;
        let reference = MemoryRecordRefV1::Mapping(key);
        self.require_vm_active(key.allocation.vm)?;
        if key.id.0 == 0 {
            return Err(MemoryTransitionErrorV1::InvalidIdentity(reference));
        }
        if self.mappings.iter().any(|record| record.key == key) {
            return Err(MemoryTransitionErrorV1::AlreadyExists(reference));
        }
        self.require_next_issued_id(
            MemoryIssuedIdScopeV1::Mapping(key.allocation),
            key.id.0,
            reference,
        )?;
        if self.allocation(key.allocation)?.state != MemoryAllocationStateV1::Live {
            return Err(MemoryTransitionErrorV1::IllegalState(
                MemoryRecordRefV1::Allocation(key.allocation),
            ));
        }
        let vm = self.vm(key.allocation.vm)?;
        if !device_keys_equal_admissions(&target_devices, &vm.mapping_devices) {
            return Err(MemoryTransitionErrorV1::DeviceSetMismatch(reference));
        }
        self.mappings.push(MemoryMappingRecordV1 {
            key,
            target_devices,
            access,
            mapped_start: 0,
            mapped_end: 0,
            state: MemoryMappingStateV1::MapPending,
        });
        self.record_issued_id(MemoryIssuedIdScopeV1::Mapping(key.allocation), key.id.0);
        Ok(())
    }

    fn observe_map(
        &mut self,
        key: MemoryMappingKeyV1,
        progress: PartialProgressObservationV1,
    ) -> Result<(), MemoryTransitionErrorV1> {
        let reference = MemoryRecordRefV1::Mapping(key);
        let mapping = self.mapping_mut(key)?;
        if mapping.state != MemoryMappingStateV1::MapPending {
            return Err(MemoryTransitionErrorV1::IllegalState(reference));
        }
        let count = mapping.target_devices.len();
        match progress.status {
            PartialOperationStatusV1::Succeeded if progress.n_success == count => {
                mapping.mapped_end = count;
                mapping.state = MemoryMappingStateV1::Mapped;
            }
            PartialOperationStatusV1::Failed if progress.n_success <= count => {
                mapping.mapped_end = progress.n_success;
                mapping.state = MemoryMappingStateV1::MapFailed;
            }
            PartialOperationStatusV1::Succeeded
            | PartialOperationStatusV1::Failed
            | PartialOperationStatusV1::Indeterminate => {
                mapping.mapped_start = 0;
                mapping.mapped_end = count;
                mapping.state = MemoryMappingStateV1::Ambiguous;
            }
        }
        Ok(())
    }

    fn begin_unmap(&mut self, key: MemoryMappingKeyV1) -> Result<(), MemoryTransitionErrorV1> {
        let reference = MemoryRecordRefV1::Mapping(key);
        if self.publications.iter().any(|publication| {
            publication.key.mapping == key && publication.state == MemoryPublicationStateV1::Live
        }) {
            return Err(MemoryTransitionErrorV1::ResourceInUse(reference));
        }
        let mapping = self.mapping_mut(key)?;
        if !matches!(
            mapping.state,
            MemoryMappingStateV1::Mapped
                | MemoryMappingStateV1::MapFailed
                | MemoryMappingStateV1::UnmapFailed
        ) || mapping.mapped_start == mapping.mapped_end
        {
            return Err(MemoryTransitionErrorV1::IllegalState(reference));
        }
        mapping.state = MemoryMappingStateV1::UnmapPending;
        Ok(())
    }

    fn observe_unmap(
        &mut self,
        key: MemoryMappingKeyV1,
        progress: PartialProgressObservationV1,
    ) -> Result<(), MemoryTransitionErrorV1> {
        let reference = MemoryRecordRefV1::Mapping(key);
        let mapping = self.mapping_mut(key)?;
        if mapping.state != MemoryMappingStateV1::UnmapPending {
            return Err(MemoryTransitionErrorV1::IllegalState(reference));
        }
        let previous_start = mapping.mapped_start;
        let mapped_end = mapping.mapped_end;
        match progress.status {
            PartialOperationStatusV1::Succeeded if progress.n_success == mapped_end => {
                mapping.mapped_start = mapped_end;
                mapping.state = MemoryMappingStateV1::Unmapped;
            }
            PartialOperationStatusV1::Failed
                if previous_start <= progress.n_success && progress.n_success < mapped_end =>
            {
                mapping.mapped_start = progress.n_success;
                mapping.state = MemoryMappingStateV1::UnmapFailed;
            }
            PartialOperationStatusV1::Succeeded
            | PartialOperationStatusV1::Failed
            | PartialOperationStatusV1::Indeterminate => {
                mapping.state = MemoryMappingStateV1::Ambiguous;
            }
        }
        Ok(())
    }

    fn release_mapping(&mut self, key: MemoryMappingKeyV1) -> Result<(), MemoryTransitionErrorV1> {
        let reference = MemoryRecordRefV1::Mapping(key);
        if self.publications.iter().any(|publication| {
            publication.key.mapping == key && publication.state == MemoryPublicationStateV1::Live
        }) {
            return Err(MemoryTransitionErrorV1::ResourceInUse(reference));
        }
        let mapping = self.mapping_mut(key)?;
        if !matches!(
            mapping.state,
            MemoryMappingStateV1::MapFailed
                | MemoryMappingStateV1::UnmapFailed
                | MemoryMappingStateV1::Unmapped
        ) || mapping.mapped_start != mapping.mapped_end
        {
            return Err(MemoryTransitionErrorV1::IllegalState(reference));
        }
        mapping.state = MemoryMappingStateV1::Released;
        Ok(())
    }

    fn publish_mapping(
        &mut self,
        key: MemoryPublicationKeyV1,
        owner: MemoryPublicationOwnerV1,
    ) -> Result<(), MemoryTransitionErrorV1> {
        ensure_memory_room(
            self.publications.len(),
            MAX_MEMORY_PUBLICATIONS_V1,
            MemoryRecordKindV1::Publication,
        )?;
        let reference = MemoryRecordRefV1::Publication(key);
        if key.id.0 == 0 {
            return Err(MemoryTransitionErrorV1::InvalidIdentity(reference));
        }
        if self.publications.iter().any(|record| record.key == key) {
            return Err(MemoryTransitionErrorV1::AlreadyExists(reference));
        }
        self.require_next_issued_id(
            MemoryIssuedIdScopeV1::Publication(key.mapping),
            key.id.0,
            reference,
        )?;
        if self.mapping(key.mapping)?.state != MemoryMappingStateV1::Mapped {
            return Err(MemoryTransitionErrorV1::IllegalState(
                MemoryRecordRefV1::Mapping(key.mapping),
            ));
        }
        if let MemoryPublicationOwnerV1::ComputeAqlQueue(queue) = owner
            && (queue.vm != key.mapping.allocation.vm || queue.id.0 == 0 || queue.generation.0 == 0)
        {
            return Err(MemoryTransitionErrorV1::BindingMismatch(reference));
        }
        self.publications.push(MemoryPublicationRecordV1 {
            key,
            owner,
            state: MemoryPublicationStateV1::Live,
        });
        self.record_issued_id(MemoryIssuedIdScopeV1::Publication(key.mapping), key.id.0);
        Ok(())
    }

    fn release_publication(
        &mut self,
        key: MemoryPublicationKeyV1,
    ) -> Result<(), MemoryTransitionErrorV1> {
        let reference = MemoryRecordRefV1::Publication(key);
        let publication = self.publication(key)?;
        if publication.owner != MemoryPublicationOwnerV1::Generic {
            return Err(MemoryTransitionErrorV1::ResourceInUse(reference));
        }
        let publication = self.publication_mut(key)?;
        if publication.state != MemoryPublicationStateV1::Live {
            return Err(MemoryTransitionErrorV1::IllegalState(reference));
        }
        publication.state = MemoryPublicationStateV1::Released;
        Ok(())
    }

    fn release_queue_publication(
        &mut self,
        key: MemoryPublicationKeyV1,
        queue: QueueKeyV1,
    ) -> Result<(), MemoryTransitionErrorV1> {
        let reference = MemoryRecordRefV1::Publication(key);
        let publication = self.publication(key)?;
        if publication.owner != MemoryPublicationOwnerV1::ComputeAqlQueue(queue) {
            return Err(MemoryTransitionErrorV1::BindingMismatch(reference));
        }
        if publication.state != MemoryPublicationStateV1::Live {
            return Err(MemoryTransitionErrorV1::IllegalState(reference));
        }
        self.publication_mut(key)?.state = MemoryPublicationStateV1::Released;
        Ok(())
    }

    fn release_peer_publication(
        &mut self,
        key: MemoryPublicationKeyV1,
        binding: PeerTransferBindingV1,
    ) -> Result<(), MemoryTransitionErrorV1> {
        let reference = MemoryRecordRefV1::Publication(key);
        let publication = self.publication(key)?;
        if publication.owner != MemoryPublicationOwnerV1::PeerTransfer(binding.publication_owner())
        {
            return Err(MemoryTransitionErrorV1::BindingMismatch(reference));
        }
        if publication.state != MemoryPublicationStateV1::Live {
            return Err(MemoryTransitionErrorV1::IllegalState(reference));
        }
        self.publication_mut(key)?.state = MemoryPublicationStateV1::Released;
        Ok(())
    }

    fn validate_capacities(&self) -> Result<(), MemoryInvariantViolationV1> {
        for (actual, maximum, kind) in [
            (self.vms.len(), MAX_MEMORY_VMS_V1, MemoryRecordKindV1::Vm),
            (
                self.reservations.len(),
                MAX_VA_RESERVATIONS_V1,
                MemoryRecordKindV1::VaReservation,
            ),
            (
                self.allocations.len(),
                MAX_MEMORY_ALLOCATIONS_V1,
                MemoryRecordKindV1::Allocation,
            ),
            (
                self.mappings.len(),
                MAX_MEMORY_MAPPINGS_V1,
                MemoryRecordKindV1::Mapping,
            ),
            (
                self.publications.len(),
                MAX_MEMORY_PUBLICATIONS_V1,
                MemoryRecordKindV1::Publication,
            ),
            (
                self.issued_id_high_watermarks.len(),
                MAX_MEMORY_ISSUED_ID_HIGH_WATERMARKS_V1,
                MemoryRecordKindV1::IssuedIdHighWatermark,
            ),
        ] {
            if actual > maximum {
                return Err(MemoryInvariantViolationV1::CapacityExceeded(kind));
            }
        }
        Ok(())
    }

    fn validate_vms(&self) -> Result<(), MemoryInvariantViolationV1> {
        for (index, vm) in self.vms.iter().enumerate() {
            let key = vm.admission.model_key();
            let reference = MemoryRecordRefV1::Vm(key);
            if vm.admission.domain_id() != self.domain_id
                || vm
                    .mapping_devices
                    .iter()
                    .any(|device| device.domain_id() != self.domain_id)
            {
                return Err(MemoryInvariantViolationV1::DomainMismatch(reference));
            }
            if key.id.0 == 0 || vm.handle.0 == 0 {
                return Err(MemoryInvariantViolationV1::InvalidIdentity(reference));
            }
            if !valid_mapping_admissions(&vm.mapping_devices, key.device) {
                return Err(MemoryInvariantViolationV1::DeviceSetMismatch(reference));
            }
            if !valid_page_range(vm.aperture) {
                return Err(MemoryInvariantViolationV1::InvalidRange(reference));
            }
            for old in &self.vms[..index] {
                if old.admission.model_key() == key {
                    return Err(MemoryInvariantViolationV1::Duplicate(reference));
                }
                if vm.state == MemoryVmStateV1::Active
                    && old.state == MemoryVmStateV1::Active
                    && old.handle == vm.handle
                {
                    return Err(MemoryInvariantViolationV1::HandleCollision(reference));
                }
            }
        }
        Ok(())
    }

    fn validate_reservations(&self) -> Result<(), MemoryInvariantViolationV1> {
        for (index, reservation) in self.reservations.iter().enumerate() {
            let reference = MemoryRecordRefV1::VaReservation(reservation.key);
            let Some(vm) = self.vm_opt(reservation.key.vm) else {
                return Err(MemoryInvariantViolationV1::MissingParent(reference));
            };
            if reservation.key.id.0 == 0 {
                return Err(MemoryInvariantViolationV1::InvalidIdentity(reference));
            }
            if !valid_alignment(reservation.alignment)
                || !reservation.range.base.is_multiple_of(reservation.alignment)
            {
                return Err(MemoryInvariantViolationV1::InvalidAlignment(reference));
            }
            if !valid_page_range(reservation.range) || !range_within(reservation.range, vm.aperture)
            {
                return Err(MemoryInvariantViolationV1::InvalidRange(reference));
            }
            if reservation.state == VaReservationStateV1::Reserved
                && vm.state != MemoryVmStateV1::Active
            {
                return Err(MemoryInvariantViolationV1::InvalidState(reference));
            }
            for old in &self.reservations[..index] {
                if old.key == reservation.key {
                    return Err(MemoryInvariantViolationV1::Duplicate(reference));
                }
                if old.key.vm == reservation.key.vm
                    && old.state == VaReservationStateV1::Reserved
                    && reservation.state == VaReservationStateV1::Reserved
                    && ranges_overlap(old.range, reservation.range)
                {
                    return Err(MemoryInvariantViolationV1::AddressOverlap(
                        old.key,
                        reservation.key,
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_allocations(&self) -> Result<(), MemoryInvariantViolationV1> {
        for (index, allocation) in self.allocations.iter().enumerate() {
            let reference = MemoryRecordRefV1::Allocation(allocation.key);
            let Some(vm) = self.vm_opt(allocation.key.vm) else {
                return Err(MemoryInvariantViolationV1::MissingParent(reference));
            };
            let Some(reservation) = self.reservation_opt(allocation.reservation) else {
                return Err(MemoryInvariantViolationV1::MissingParent(reference));
            };
            if allocation.key.vm != allocation.reservation.vm {
                return Err(MemoryInvariantViolationV1::BindingMismatch(reference));
            }
            if allocation.key.id.0 == 0
                || allocation.key.generation.0 == 0
                || allocation.handle.0 == 0
            {
                return Err(MemoryInvariantViolationV1::InvalidIdentity(reference));
            }
            if !valid_allocation_spec(allocation.spec, *reservation) {
                return Err(MemoryInvariantViolationV1::InvalidRange(reference));
            }
            if allocation.state == MemoryAllocationStateV1::Live
                && (vm.state != MemoryVmStateV1::Active
                    || reservation.state != VaReservationStateV1::Reserved)
            {
                return Err(MemoryInvariantViolationV1::InvalidState(reference));
            }
            for old in &self.allocations[..index] {
                if old.key == allocation.key {
                    return Err(MemoryInvariantViolationV1::Duplicate(reference));
                }
                if old.key.vm == allocation.key.vm && old.key.id == allocation.key.id {
                    if old.key.generation >= allocation.key.generation {
                        return Err(MemoryInvariantViolationV1::StaleGeneration(allocation.key));
                    }
                    if old.state != MemoryAllocationStateV1::Released {
                        return Err(MemoryInvariantViolationV1::EarlyRelease(reference));
                    }
                }
                if allocation.state == MemoryAllocationStateV1::Live
                    && old.state == MemoryAllocationStateV1::Live
                    && (old.handle == allocation.handle
                        || old.reservation == allocation.reservation)
                {
                    return Err(MemoryInvariantViolationV1::HandleCollision(reference));
                }
            }
        }
        Ok(())
    }

    fn validate_mappings(&self) -> Result<(), MemoryInvariantViolationV1> {
        for (index, mapping) in self.mappings.iter().enumerate() {
            let reference = MemoryRecordRefV1::Mapping(mapping.key);
            let Some(allocation) = self.allocation_opt(mapping.key.allocation) else {
                return Err(MemoryInvariantViolationV1::MissingParent(reference));
            };
            let Some(vm) = self.vm_opt(mapping.key.allocation.vm) else {
                return Err(MemoryInvariantViolationV1::MissingParent(reference));
            };
            if mapping.key.id.0 == 0 {
                return Err(MemoryInvariantViolationV1::InvalidIdentity(reference));
            }
            if !device_keys_equal_admissions(&mapping.target_devices, &vm.mapping_devices) {
                return Err(MemoryInvariantViolationV1::DeviceSetMismatch(reference));
            }
            if mapping.mapped_start > mapping.mapped_end
                || mapping.mapped_end > mapping.target_devices.len()
            {
                return Err(MemoryInvariantViolationV1::InvalidRange(reference));
            }
            let range_is_valid = match mapping.state {
                MemoryMappingStateV1::MapPending => {
                    mapping.mapped_start == 0 && mapping.mapped_end == 0
                }
                MemoryMappingStateV1::MapFailed => mapping.mapped_start == 0,
                MemoryMappingStateV1::Mapped => {
                    mapping.mapped_start == 0 && mapping.mapped_end == mapping.target_devices.len()
                }
                MemoryMappingStateV1::UnmapPending => mapping.mapped_start < mapping.mapped_end,
                MemoryMappingStateV1::UnmapFailed | MemoryMappingStateV1::Ambiguous => true,
                MemoryMappingStateV1::Unmapped => mapping.mapped_start == mapping.mapped_end,
                MemoryMappingStateV1::Released => mapping.mapped_start == mapping.mapped_end,
            };
            if !range_is_valid {
                return Err(MemoryInvariantViolationV1::InvalidState(reference));
            }
            if mapping.state != MemoryMappingStateV1::Released
                && allocation.state != MemoryAllocationStateV1::Live
            {
                return Err(MemoryInvariantViolationV1::EarlyRelease(reference));
            }
            if self.mappings[..index]
                .iter()
                .any(|old| old.key == mapping.key)
            {
                return Err(MemoryInvariantViolationV1::Duplicate(reference));
            }
        }
        Ok(())
    }

    fn validate_publications(&self) -> Result<(), MemoryInvariantViolationV1> {
        for (index, publication) in self.publications.iter().enumerate() {
            let reference = MemoryRecordRefV1::Publication(publication.key);
            let Some(mapping) = self.mapping_opt(publication.key.mapping) else {
                return Err(MemoryInvariantViolationV1::MissingParent(reference));
            };
            if publication.key.id.0 == 0 {
                return Err(MemoryInvariantViolationV1::InvalidIdentity(reference));
            }
            match publication.owner {
                MemoryPublicationOwnerV1::Generic => {}
                MemoryPublicationOwnerV1::ComputeAqlQueue(queue)
                    if queue.vm != publication.key.mapping.allocation.vm
                        || queue.id.0 == 0
                        || queue.generation.0 == 0 =>
                {
                    return Err(MemoryInvariantViolationV1::BindingMismatch(reference));
                }
                MemoryPublicationOwnerV1::PeerTransfer(owner) if !owner.has_valid_identity() => {
                    return Err(MemoryInvariantViolationV1::BindingMismatch(reference));
                }
                MemoryPublicationOwnerV1::ComputeAqlQueue(_)
                | MemoryPublicationOwnerV1::PeerTransfer(_) => {}
            }
            if publication.state == MemoryPublicationStateV1::Live
                && mapping.state != MemoryMappingStateV1::Mapped
            {
                return Err(MemoryInvariantViolationV1::EarlyRelease(reference));
            }
            if self.publications[..index]
                .iter()
                .any(|old| old.key == publication.key)
            {
                return Err(MemoryInvariantViolationV1::Duplicate(reference));
            }
        }
        Ok(())
    }

    fn validate_release_order(&self) -> Result<(), MemoryInvariantViolationV1> {
        for allocation in &self.allocations {
            if allocation.state == MemoryAllocationStateV1::Released
                && (self.mappings.iter().any(|mapping| {
                    mapping.key.allocation == allocation.key
                        && mapping.state != MemoryMappingStateV1::Released
                }) || self.publications.iter().any(|publication| {
                    publication.key.mapping.allocation == allocation.key
                        && publication.state != MemoryPublicationStateV1::Released
                }))
            {
                return Err(MemoryInvariantViolationV1::EarlyRelease(
                    MemoryRecordRefV1::Allocation(allocation.key),
                ));
            }
        }
        for reservation in &self.reservations {
            if reservation.state == VaReservationStateV1::Released
                && self.allocations.iter().any(|allocation| {
                    allocation.reservation == reservation.key
                        && allocation.state != MemoryAllocationStateV1::Released
                })
            {
                return Err(MemoryInvariantViolationV1::EarlyRelease(
                    MemoryRecordRefV1::VaReservation(reservation.key),
                ));
            }
        }
        for vm in &self.vms {
            let key = vm.admission.model_key();
            if vm.state == MemoryVmStateV1::Retired
                && (self.reservations.iter().any(|record| {
                    record.key.vm == key && record.state != VaReservationStateV1::Released
                }) || self.allocations.iter().any(|record| {
                    record.key.vm == key && record.state != MemoryAllocationStateV1::Released
                }) || self.mappings.iter().any(|record| {
                    record.key.allocation.vm == key
                        && record.state != MemoryMappingStateV1::Released
                }) || self.publications.iter().any(|record| {
                    record.key.mapping.allocation.vm == key
                        && record.state != MemoryPublicationStateV1::Released
                }))
            {
                return Err(MemoryInvariantViolationV1::EarlyRelease(
                    MemoryRecordRefV1::Vm(key),
                ));
            }
        }
        Ok(())
    }

    fn validate_issued_id_high_watermarks(&self) -> Result<(), MemoryInvariantViolationV1> {
        if self.identity_discipline == MemoryIdentityDisciplineV1::ReusableGenerations {
            if let Some(watermark) = self.issued_id_high_watermarks.first() {
                return Err(MemoryInvariantViolationV1::InvalidIssuedIdHighWatermark(
                    *watermark,
                ));
            }
            return Ok(());
        }

        for (index, watermark) in self.issued_id_high_watermarks.iter().enumerate() {
            if watermark.last_id == 0
                || self.issued_id_high_watermarks[..index]
                    .iter()
                    .any(|old| old.scope >= watermark.scope)
            {
                return Err(MemoryInvariantViolationV1::InvalidIssuedIdHighWatermark(
                    *watermark,
                ));
            }
            let parent_exists = match watermark.scope {
                MemoryIssuedIdScopeV1::VaReservation(vm)
                | MemoryIssuedIdScopeV1::Allocation(vm) => self.vm_opt(vm).is_some(),
                MemoryIssuedIdScopeV1::Mapping(allocation) => {
                    self.allocation_opt(allocation).is_some()
                }
                MemoryIssuedIdScopeV1::Publication(mapping) => self.mapping_opt(mapping).is_some(),
            };
            if !parent_exists {
                return Err(MemoryInvariantViolationV1::InvalidIssuedIdHighWatermark(
                    *watermark,
                ));
            }
        }

        for reservation in &self.reservations {
            self.validate_issued_id(
                MemoryIssuedIdScopeV1::VaReservation(reservation.key.vm),
                reservation.key.id.0,
            )?;
        }
        for allocation in &self.allocations {
            self.validate_issued_id(
                MemoryIssuedIdScopeV1::Allocation(allocation.key.vm),
                allocation.key.id.0,
            )?;
        }
        for mapping in &self.mappings {
            self.validate_issued_id(
                MemoryIssuedIdScopeV1::Mapping(mapping.key.allocation),
                mapping.key.id.0,
            )?;
        }
        for publication in &self.publications {
            self.validate_issued_id(
                MemoryIssuedIdScopeV1::Publication(publication.key.mapping),
                publication.key.id.0,
            )?;
        }
        Ok(())
    }

    fn validate_issued_id(
        &self,
        scope: MemoryIssuedIdScopeV1,
        id: u64,
    ) -> Result<(), MemoryInvariantViolationV1> {
        if self
            .issued_id_high_watermarks
            .binary_search_by(|watermark| watermark.scope.cmp(&scope))
            .ok()
            .and_then(|index| self.issued_id_high_watermarks.get(index))
            .is_some_and(|watermark| watermark.last_id >= id)
        {
            return Ok(());
        }
        Err(MemoryInvariantViolationV1::InvalidIssuedIdHighWatermark(
            MemoryIssuedIdHighWatermarkV1 { scope, last_id: id },
        ))
    }

    fn require_next_issued_id(
        &self,
        scope: MemoryIssuedIdScopeV1,
        id: u64,
        reference: MemoryRecordRefV1,
    ) -> Result<(), MemoryTransitionErrorV1> {
        if self.identity_discipline == MemoryIdentityDisciplineV1::MonotonicNonReusable
            && self
                .issued_id_high_watermarks
                .binary_search_by(|watermark| watermark.scope.cmp(&scope))
                .ok()
                .and_then(|index| self.issued_id_high_watermarks.get(index))
                .is_some_and(|watermark| watermark.last_id >= id)
        {
            return Err(MemoryTransitionErrorV1::NonMonotonicIdentity(reference));
        }
        Ok(())
    }

    fn record_issued_id(&mut self, scope: MemoryIssuedIdScopeV1, id: u64) {
        if self.identity_discipline != MemoryIdentityDisciplineV1::MonotonicNonReusable {
            return;
        }
        match self
            .issued_id_high_watermarks
            .binary_search_by(|watermark| watermark.scope.cmp(&scope))
        {
            Ok(index) => {
                self.issued_id_high_watermarks
                    .get_mut(index)
                    .expect("watermark index came from journal")
                    .last_id = id;
            }
            Err(index) => self
                .issued_id_high_watermarks
                .insert(index, MemoryIssuedIdHighWatermarkV1 { scope, last_id: id }),
        }
    }

    fn require_vm_active(&self, key: VmKeyV1) -> Result<(), MemoryTransitionErrorV1> {
        if self.vm(key)?.state != MemoryVmStateV1::Active {
            return Err(MemoryTransitionErrorV1::IllegalState(
                MemoryRecordRefV1::Vm(key),
            ));
        }
        Ok(())
    }

    fn vm_opt(&self, key: VmKeyV1) -> Option<&MemoryVmRecordV1> {
        self.vms.get(key)
    }

    fn vm(&self, key: VmKeyV1) -> Result<&MemoryVmRecordV1, MemoryTransitionErrorV1> {
        self.vm_opt(key)
            .ok_or(MemoryTransitionErrorV1::NotFound(MemoryRecordRefV1::Vm(
                key,
            )))
    }

    fn vm_mut(&mut self, key: VmKeyV1) -> Result<&mut MemoryVmRecordV1, MemoryTransitionErrorV1> {
        self.vms
            .get_mut(key)
            .ok_or(MemoryTransitionErrorV1::NotFound(MemoryRecordRefV1::Vm(
                key,
            )))
    }

    fn reservation_opt(&self, key: VaReservationKeyV1) -> Option<&VaReservationRecordV1> {
        self.reservations.get(key)
    }

    fn reservation(
        &self,
        key: VaReservationKeyV1,
    ) -> Result<&VaReservationRecordV1, MemoryTransitionErrorV1> {
        self.reservation_opt(key)
            .ok_or(MemoryTransitionErrorV1::NotFound(
                MemoryRecordRefV1::VaReservation(key),
            ))
    }

    fn reservation_mut(
        &mut self,
        key: VaReservationKeyV1,
    ) -> Result<&mut VaReservationRecordV1, MemoryTransitionErrorV1> {
        self.reservations
            .get_mut(key)
            .ok_or(MemoryTransitionErrorV1::NotFound(
                MemoryRecordRefV1::VaReservation(key),
            ))
    }

    fn allocation_opt(&self, key: MemoryAllocationKeyV1) -> Option<&MemoryAllocationRecordV1> {
        self.allocations.get(key)
    }

    fn allocation(
        &self,
        key: MemoryAllocationKeyV1,
    ) -> Result<&MemoryAllocationRecordV1, MemoryTransitionErrorV1> {
        self.allocation_opt(key)
            .ok_or(MemoryTransitionErrorV1::NotFound(
                MemoryRecordRefV1::Allocation(key),
            ))
    }

    fn allocation_mut(
        &mut self,
        key: MemoryAllocationKeyV1,
    ) -> Result<&mut MemoryAllocationRecordV1, MemoryTransitionErrorV1> {
        self.allocations
            .get_mut(key)
            .ok_or(MemoryTransitionErrorV1::NotFound(
                MemoryRecordRefV1::Allocation(key),
            ))
    }

    fn mapping_opt(&self, key: MemoryMappingKeyV1) -> Option<&MemoryMappingRecordV1> {
        self.mappings.get(key)
    }

    fn mapping(
        &self,
        key: MemoryMappingKeyV1,
    ) -> Result<&MemoryMappingRecordV1, MemoryTransitionErrorV1> {
        self.mapping_opt(key)
            .ok_or(MemoryTransitionErrorV1::NotFound(
                MemoryRecordRefV1::Mapping(key),
            ))
    }

    fn mapping_mut(
        &mut self,
        key: MemoryMappingKeyV1,
    ) -> Result<&mut MemoryMappingRecordV1, MemoryTransitionErrorV1> {
        self.mappings
            .get_mut(key)
            .ok_or(MemoryTransitionErrorV1::NotFound(
                MemoryRecordRefV1::Mapping(key),
            ))
    }

    fn publication_mut(
        &mut self,
        key: MemoryPublicationKeyV1,
    ) -> Result<&mut MemoryPublicationRecordV1, MemoryTransitionErrorV1> {
        self.publications
            .get_mut(key)
            .ok_or(MemoryTransitionErrorV1::NotFound(
                MemoryRecordRefV1::Publication(key),
            ))
    }

    fn publication(
        &self,
        key: MemoryPublicationKeyV1,
    ) -> Result<&MemoryPublicationRecordV1, MemoryTransitionErrorV1> {
        self.publications
            .get(key)
            .ok_or(MemoryTransitionErrorV1::NotFound(
                MemoryRecordRefV1::Publication(key),
            ))
    }
}

fn ensure_memory_room(
    actual: usize,
    maximum: usize,
    kind: MemoryRecordKindV1,
) -> Result<(), MemoryTransitionErrorV1> {
    if actual >= maximum {
        return Err(MemoryTransitionErrorV1::CapacityExceeded { kind, maximum });
    }
    Ok(())
}

fn valid_alignment(alignment: u64) -> bool {
    alignment >= MEMORY_PAGE_BYTES_V1 && alignment.is_power_of_two()
}

fn valid_page_range(range: GpuVaRangeV1) -> bool {
    range.byte_len != 0
        && range.base.is_multiple_of(MEMORY_PAGE_BYTES_V1)
        && range.byte_len.is_multiple_of(MEMORY_PAGE_BYTES_V1)
        && range.checked_end().is_some()
}

fn range_within(inner: GpuVaRangeV1, outer: GpuVaRangeV1) -> bool {
    match (inner.checked_end(), outer.checked_end()) {
        (Some(inner_end), Some(outer_end)) => inner.base >= outer.base && inner_end <= outer_end,
        _ => false,
    }
}

fn ranges_overlap(left: GpuVaRangeV1, right: GpuVaRangeV1) -> bool {
    match (left.checked_end(), right.checked_end()) {
        (Some(left_end), Some(right_end)) => left.base < right_end && right.base < left_end,
        _ => true,
    }
}

fn valid_mapping_admissions(admissions: &[ModelDeviceAdmissionV1], primary: DeviceKeyV1) -> bool {
    !admissions.is_empty()
        && admissions.len() <= MAX_MEMORY_MAPPING_DEVICES_V1
        && admissions
            .windows(2)
            .all(|pair| pair[0].model_key() < pair[1].model_key())
        && admissions.iter().enumerate().all(|(index, admission)| {
            admissions[..index]
                .iter()
                .all(|old| old.model_key().physical != admission.model_key().physical)
        })
        && admissions
            .iter()
            .any(|admission| admission.model_key() == primary)
}

fn device_keys_equal_admissions(
    keys: &[DeviceKeyV1],
    admissions: &[ModelDeviceAdmissionV1],
) -> bool {
    keys.len() == admissions.len()
        && keys
            .iter()
            .zip(admissions)
            .all(|(key, admission)| *key == admission.model_key())
}

fn valid_allocation_spec(spec: MemoryAllocationSpecV1, reservation: VaReservationRecordV1) -> bool {
    spec.byte_len != 0
        && spec.byte_len <= reservation.range.byte_len
        && spec.byte_len.is_multiple_of(MEMORY_PAGE_BYTES_V1)
        && valid_alignment(spec.alignment)
        && reservation.range.base.is_multiple_of(spec.alignment)
        && spec.alignment <= reservation.alignment
}
