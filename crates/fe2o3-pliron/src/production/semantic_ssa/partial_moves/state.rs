//! Persistent move facts in 64-local radix leaves. Whole-local bits need no
//! path allocations; sparse partial paths and unchanged snapshots stay shared.

use std::{cell::Cell, collections::BTreeSet, ops::Bound, rc::Rc};

#[path = "state_partial_owner_v1.rs"]
mod partial_owner;

#[path = "state_branch_reuse_v1.rs"]
mod branch_reuse;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum PathElement {
    Field(u32),
    ConstantIndex { offset: u64, from_end: bool },
    Downcast(u32),
}

pub(super) type Path = Vec<PathElement>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Resource {
    Storage,
    Work,
}

/// Captured before failed-allocation temporaries unwind and release storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StorageFailure {
    pub(super) live: usize,
    pub(super) peak: usize,
    pub(super) requested: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    Overflow,
    Limit {
        resource: Resource,
        required: usize,
        limit: usize,
        storage: Option<StorageFailure>,
    },
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy)]
enum ReadFacet {
    WholeValue,
    DirectEnumTag,
}

struct Accounting {
    base_storage: usize,
    base_work: usize,
    live: Cell<usize>,
    peak: Cell<usize>,
    work: Cell<usize>,
    max_storage: usize,
    max_work: usize,
}

#[derive(Clone)]
pub(super) struct Budget(Rc<Accounting>);

impl Budget {
    pub(super) fn new(
        base_storage: usize,
        base_work: usize,
        max_storage: usize,
        max_work: usize,
    ) -> Result<Self> {
        let budget = Self(Rc::new(Accounting {
            base_storage,
            base_work,
            live: Cell::new(0),
            peak: Cell::new(0),
            work: Cell::new(0),
            max_storage,
            max_work,
        }));
        budget.work(0)?;
        drop(budget.reserve(0)?);
        Ok(budget)
    }

    pub(super) fn work(&self, units: usize) -> Result<()> {
        let work = self
            .0
            .work
            .get()
            .checked_add(units)
            .ok_or(Error::Overflow)?;
        let required = self.0.base_work.checked_add(work).ok_or(Error::Overflow)?;
        if required > self.0.max_work {
            return Err(Error::Limit {
                resource: Resource::Work,
                required,
                limit: self.0.max_work,
                storage: None,
            });
        }
        self.0.work.set(work);
        Ok(())
    }

    pub(super) fn reserve(&self, words: usize) -> Result<Storage> {
        let live = self
            .0
            .live
            .get()
            .checked_add(words)
            .ok_or(Error::Overflow)?;
        let required = self
            .0
            .base_storage
            .checked_add(live)
            .ok_or(Error::Overflow)?;
        if required > self.0.max_storage {
            return Err(Error::Limit {
                resource: Resource::Storage,
                required,
                limit: self.0.max_storage,
                storage: Some(StorageFailure {
                    live: self.0.live.get(),
                    peak: self.0.peak.get(),
                    requested: words,
                }),
            });
        }
        self.0.live.set(live);
        self.0.peak.set(self.0.peak.get().max(live));
        Ok(Storage {
            accounting: self.0.clone(),
            words,
        })
    }

    pub(super) fn peak(&self) -> usize {
        self.0.peak.get()
    }
    pub(super) fn work_units(&self) -> usize {
        self.0.work.get()
    }
}

pub(super) struct Storage {
    accounting: Rc<Accounting>,
    words: usize,
}

impl Drop for Storage {
    fn drop(&mut self) {
        self.accounting
            .live
            .set(self.accounting.live.get() - self.words);
    }
}

// Two removed node words preserve the old conservative allocation margin.
const NODE_WORDS: usize = 10;
const PATH_SET_WORDS: usize = 12;
const PATH_HEADER_WORDS: usize = 8;
const ELEMENT_WORDS: usize = size_of::<PathElement>().div_ceil(size_of::<usize>());
const GROUP_SHIFT: u32 = 6;
const GROUP_MASK: u32 = (1 << GROUP_SHIFT) - 1;
const PARTIAL_WORDS: usize = size_of::<PartialPaths>().div_ceil(size_of::<usize>()) + 2;
const PARTIAL_ENTRY_WORDS: usize = size_of::<Rc<PathSet>>().div_ceil(size_of::<usize>());

fn path_words(path: &[PathElement]) -> Result<usize> {
    path.len()
        .checked_mul(ELEMENT_WORDS)
        .and_then(|n| n.checked_add(PATH_HEADER_WORDS))
        .ok_or(Error::Overflow)
}

struct Node {
    kind: Kind,
    _storage: NodeStorage,
}

// Each node has the same fixed charge. Keep the accounting owner, not a
// redundant per-node word count; transfer the already reserved charge intact.
struct NodeStorage {
    accounting: Rc<Accounting>,
}

impl NodeStorage {
    fn reserve(budget: &Budget) -> Result<Self> {
        let mut storage = budget.reserve(NODE_WORDS)?;
        let accounting = storage.accounting.clone();
        storage.words = 0;
        Ok(Self { accounting })
    }

    #[cfg(test)]
    fn words(&self) -> usize {
        NODE_WORDS
    }
}

impl Drop for NodeStorage {
    fn drop(&mut self) {
        self.accounting
            .live
            .set(self.accounting.live.get() - NODE_WORDS);
    }
}

enum Kind {
    Leaf {
        group: u32,
        whole: u64,
        partial: Option<Rc<PartialPaths>>,
    },
    Branch {
        // Lowest set bit marks the split; higher bits are the shared prefix.
        prefix: u32,
        zero: Rc<Node>,
        one: Rc<Node>,
    },
}

struct PathSet {
    paths: BTreeSet<Path>,
    _storage: Storage,
}

struct PartialPaths {
    // Pointer order is ascending set-bit order. Whole-local bits are disjoint.
    slots: u64,
    entries: Box<[Rc<PathSet>]>,
    _storage: Storage,
}

impl Node {
    fn key(&self) -> u32 {
        match &self.kind {
            Kind::Leaf { group, .. } => *group,
            Kind::Branch { prefix, .. } => prefix & (prefix - 1),
        }
    }

    fn bit(&self) -> u32 {
        match &self.kind {
            Kind::Leaf { .. } => 0,
            Kind::Branch { prefix, .. } => prefix & prefix.wrapping_neg(),
        }
    }
}

#[derive(Clone, Default)]
pub(super) struct State {
    root: Option<Rc<Node>>,
}

impl State {
    fn leaf(&self, group: u32, budget: &Budget) -> Result<Option<&Rc<Node>>> {
        let Some(mut node) = self.root.as_ref() else {
            return Ok(None);
        };
        loop {
            budget.work(1)?;
            match &node.kind {
                Kind::Leaf { group: found, .. } => return Ok((*found == group).then_some(node)),
                Kind::Branch { zero, one, .. } => {
                    node = if group & node.bit() == 0 { zero } else { one }
                }
            }
        }
    }

    pub(super) fn clear(&mut self, local: u32, budget: &Budget) -> Result<()> {
        let group = local >> GROUP_SHIFT;
        let slot = (local & GROUP_MASK) as u8;
        let Some(node) = self.leaf(group, budget)? else {
            return Ok(());
        };
        let Kind::Leaf { whole, partial, .. } = &node.kind else {
            unreachable!()
        };
        let mask = 1u64 << slot;
        if whole & mask == 0 && partial_at(partial.as_ref(), slot, budget)?.is_none() {
            return Ok(());
        }
        let whole = whole & !mask;
        let partial = replace_partial(partial.as_ref(), slot, None, budget)?;
        self.replace_group(group, whole, partial, budget)
    }

    pub(super) fn readable(
        &self,
        local: u32,
        path: &[PathElement],
        budget: &Budget,
    ) -> Result<bool> {
        self.readable_facet(local, path, ReadFacet::WholeValue, budget)
    }

    pub(super) fn direct_tag_readable(
        &self,
        local: u32,
        path: &[PathElement],
        budget: &Budget,
    ) -> Result<bool> {
        self.readable_facet(local, path, ReadFacet::DirectEnumTag, budget)
    }

    fn readable_facet(
        &self,
        local: u32,
        path: &[PathElement],
        facet: ReadFacet,
        budget: &Budget,
    ) -> Result<bool> {
        let Some(leaf) = self.leaf(local >> GROUP_SHIFT, budget)? else {
            return Ok(true);
        };
        let Kind::Leaf { whole, partial, .. } = &leaf.kind else {
            unreachable!()
        };
        let slot = (local & GROUP_MASK) as u8;
        if whole & (1u64 << slot) != 0 {
            return Ok(false);
        }
        let Some(paths) = partial_at(partial.as_ref(), slot, budget)? else {
            return Ok(true);
        };
        let paths = &paths.paths;
        for length in 0..=path.len() {
            lookup_work(paths, length, budget)?;
            if paths.contains(&path[..length]) {
                return Ok(false);
            }
        }
        if matches!(facet, ReadFacet::DirectEnumTag) {
            // The caller established an admitted direct-tag enum place. Payload
            // descendants cannot overlap that tag; equal/ancestor holes above do.
            return Ok(true);
        }
        lookup_work(paths, path.len(), budget)?;
        Ok(!paths
            .range::<[PathElement], _>((Bound::Included(path), Bound::Unbounded))
            .next()
            .is_some_and(|candidate| candidate.starts_with(path)))
    }

    pub(super) fn mark(&mut self, local: u32, path: Path, budget: &Budget) -> Result<()> {
        let group = local >> GROUP_SHIFT;
        let slot = (local & GROUP_MASK) as u8;
        let mask = 1u64 << slot;
        let existing = self.leaf(group, budget)?;
        let (whole, partial) = existing.map_or((0, None), |node| match &node.kind {
            Kind::Leaf { whole, partial, .. } => (*whole, partial.as_ref()),
            _ => unreachable!(),
        });
        if whole & mask != 0 {
            return Ok(());
        }
        if path.is_empty() {
            let partial = replace_partial(partial, slot, None, budget)?;
            return self.replace_group(group, whole | mask, partial, budget);
        }
        let paths = partial_at(partial, slot, budget)?;
        if let Some(paths) = paths {
            lookup_work(&paths.paths, path.len(), budget)?;
            if paths.paths.contains(&path) {
                return Ok(());
            }
        }
        let mut words = PATH_SET_WORDS
            .checked_add(path_words(&path)?)
            .ok_or(Error::Overflow)?;
        if let Some(paths) = paths {
            for previous in &paths.paths {
                budget.work(previous.len().checked_add(1).ok_or(Error::Overflow)?)?;
                words = words
                    .checked_add(path_words(previous)?)
                    .ok_or(Error::Overflow)?;
            }
        }
        budget.work(path.len().checked_add(1).ok_or(Error::Overflow)?)?;
        let storage = budget.reserve(words)?;
        budget.work(words)?;
        let mut paths = paths.map_or_else(BTreeSet::new, |previous| previous.paths.clone());
        paths.insert(path);
        let paths = Rc::new(PathSet {
            paths,
            _storage: storage,
        });
        let partial = replace_partial(partial, slot, Some(paths), budget)?;
        self.replace_group(group, whole, partial, budget)
    }

    /// A strict ancestor still blocks a field write. Exact/descendant facts
    /// are killed only on this destination, including call-return edges.
    pub(super) fn initialize(
        &mut self,
        local: u32,
        path: &[PathElement],
        budget: &Budget,
    ) -> Result<bool> {
        if path.is_empty() {
            self.clear(local, budget)?;
            return Ok(true);
        }
        let group = local >> GROUP_SHIFT;
        let slot = (local & GROUP_MASK) as u8;
        let Some(leaf) = self.leaf(group, budget)? else {
            return Ok(true);
        };
        let Kind::Leaf { whole, partial, .. } = &leaf.kind else {
            unreachable!()
        };
        if whole & (1u64 << slot) != 0 {
            return Ok(false);
        }
        let Some(paths) = partial_at(partial.as_ref(), slot, budget)? else {
            return Ok(true);
        };
        let paths = &paths.paths;
        for length in 0..path.len() {
            lookup_work(paths, length, budget)?;
            if paths.contains(&path[..length]) {
                return Ok(false);
            }
        }
        let mut words = PATH_SET_WORDS;
        let mut retained = 0;
        for candidate in paths {
            budget.work(candidate.len().checked_add(1).ok_or(Error::Overflow)?)?;
            if !candidate.starts_with(path) {
                retained += 1;
                words = words
                    .checked_add(path_words(candidate)?)
                    .ok_or(Error::Overflow)?;
            }
        }
        if retained == paths.len() {
            return Ok(true);
        }
        if retained == 0 {
            self.clear(local, budget)?;
            return Ok(true);
        }
        let storage = budget.reserve(words)?;
        budget.work(words)?;
        let paths = paths
            .iter()
            .filter(|candidate| !candidate.starts_with(path))
            .cloned()
            .collect();
        let paths = Rc::new(PathSet {
            paths,
            _storage: storage,
        });
        let whole = *whole;
        let partial = replace_partial(partial.as_ref(), slot, Some(paths), budget)?;
        self.replace_group(group, whole, partial, budget)?;
        Ok(true)
    }

    pub(super) fn merge(&mut self, source: &Self, budget: &Budget) -> Result<bool> {
        budget.work(1)?;
        if same_root(&self.root, &source.root) || source.root.is_none() {
            return Ok(false);
        }
        if self.root.is_none() {
            self.root = source.root.clone();
            return Ok(true);
        }
        let mut changed = false;
        let mut source_leaves = branch_reuse::SourceLeaves::new(source.root.as_ref().unwrap());
        while let Some(node) = source_leaves.next_leaf(budget)? {
            let Kind::Leaf {
                group,
                whole: incoming_whole,
                partial: incoming_partial,
            } = &node.kind else {
                unreachable!()
            };
            let existing = self.leaf(*group, budget)?;
            if existing.is_some_and(|existing| Rc::ptr_eq(existing, node)) {
                continue;
            }
            let Some(existing) = existing else {
                self.insert_leaf(node.clone(), budget, Some(&source_leaves))?;
                changed = true;
                continue;
            };
            let Kind::Leaf { whole, partial, .. } = &existing.kind else {
                unreachable!()
            };
            let combined_whole = whole | incoming_whole;
            let combined_partial = merge_partials(
                partial.as_ref(),
                incoming_partial.as_ref(),
                combined_whole,
                budget,
            )?;
            if combined_whole == *whole && same_partial(partial, &combined_partial) {
                continue;
            }
            if combined_whole == *incoming_whole
                && same_partial(incoming_partial, &combined_partial)
            {
                // Keep the update debit; reuse the exact leaf and any matching
                // incoming ancestors instead of rebuilding their owner paths.
                budget.work(1)?;
                self.insert_leaf(node.clone(), budget, Some(&source_leaves))?;
            } else {
                self.replace_group(*group, combined_whole, combined_partial, budget)?;
            }
            changed = true;
        }
        Ok(changed)
    }

    fn replace_group(
        &mut self,
        group: u32,
        whole: u64,
        partial: Option<Rc<PartialPaths>>,
        budget: &Budget,
    ) -> Result<()> {
        if whole == 0 && partial.is_none() {
            if let Some(root) = &self.root {
                self.root = remove(root, group, budget)?;
            }
            return Ok(());
        }
        let storage = NodeStorage::reserve(budget)?;
        budget.work(1)?;
        self.insert_leaf(
            Rc::new(Node {
                kind: Kind::Leaf {
                    group,
                    whole,
                    partial,
                },
                _storage: storage,
            }),
            budget,
            None,
        )
    }

    fn insert_leaf(
        &mut self,
        leaf: Rc<Node>,
        budget: &Budget,
        source: Option<&branch_reuse::SourceLeaves<'_>>,
    ) -> Result<()> {
        self.root = Some(match &self.root {
            None => leaf,
            Some(root) => match source {
                Some(source) => source.insert(root, leaf, budget)?,
                None => insert(root, leaf, budget)?,
            },
        });
        Ok(())
    }
}

fn same_root(left: &Option<Rc<Node>>, right: &Option<Rc<Node>>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => Rc::ptr_eq(left, right),
        _ => false,
    }
}

fn lookup_work(paths: &BTreeSet<Path>, depth: usize, budget: &Budget) -> Result<()> {
    let comparisons = (usize::BITS - paths.len().max(1).leading_zeros()) as usize;
    budget.work(
        comparisons
            .checked_mul(depth.checked_add(1).ok_or(Error::Overflow)?)
            .ok_or(Error::Overflow)?,
    )
}

fn same_partial(left: &Option<Rc<PartialPaths>>, right: &Option<Rc<PartialPaths>>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => Rc::ptr_eq(left, right),
        _ => false,
    }
}

fn entries(
    partial: Option<&Rc<PartialPaths>>,
) -> impl ExactSizeIterator<Item = (u8, &Rc<PathSet>)> {
    let mut slots = partial.map_or(0, |partial| partial.slots);
    partial
        .map_or(&[][..], |partial| partial.entries.as_ref())
        .iter()
        .map(move |paths| {
            debug_assert_ne!(slots, 0);
            let slot = slots.trailing_zeros() as u8;
            slots &= slots - 1;
            (slot, paths)
        })
}

fn partial_at<'a>(
    partial: Option<&'a Rc<PartialPaths>>,
    slot: u8,
    budget: &Budget,
) -> Result<Option<&'a Rc<PathSet>>> {
    let count = partial.map_or(0, |partial| partial.entries.len());
    budget.work((usize::BITS - count.leading_zeros()) as usize + 1)?;
    let Some(partial) = partial else {
        return Ok(None);
    };
    let mask = 1u64 << slot;
    Ok((partial.slots & mask != 0)
        .then(|| &partial.entries[(partial.slots & (mask - 1)).count_ones() as usize]))
}

fn partial_storage(count: usize, budget: &Budget) -> Result<Storage> {
    debug_assert!((1..=64).contains(&count));
    budget.reserve(
        count
            .checked_mul(PARTIAL_ENTRY_WORDS)
            .and_then(|words| words.checked_add(PARTIAL_WORDS))
            .ok_or(Error::Overflow)?,
    )
}

fn replace_partial(
    previous: Option<&Rc<PartialPaths>>,
    slot: u8,
    replacement: Option<Rc<PathSet>>,
    budget: &Budget,
) -> Result<Option<Rc<PartialPaths>>> {
    let old = previous.map_or(&[][..], |partial| partial.entries.as_ref());
    let mut slots = previous.map_or(0, |partial| partial.slots);
    let mask = 1u64 << slot;
    budget.work(old.len().checked_add(1).ok_or(Error::Overflow)?)?;
    let present = slots & mask != 0;
    if replacement.is_none() && !present {
        return Ok(previous.cloned());
    }
    let count = old.len() + usize::from(replacement.is_some()) - usize::from(present);
    if count == 0 {
        return Ok(None);
    }
    let storage = partial_storage(count, budget)?;
    let mut result = Vec::with_capacity(count);
    let position = (slots & (mask - 1)).count_ones() as usize;
    result.extend(old[..position].iter().cloned());
    if let Some(replacement) = replacement {
        slots |= mask;
        result.push(replacement);
    } else {
        slots &= !mask;
    }
    result.extend(old[position + usize::from(present)..].iter().cloned());
    Ok(Some(Rc::new(PartialPaths {
        slots,
        entries: result.into_boxed_slice(),
        _storage: storage,
    })))
}

fn merged_entries<'a>(
    previous: Option<&'a Rc<PartialPaths>>,
    incoming: Option<&'a Rc<PartialPaths>>,
) -> impl Iterator<Item = (u8, Option<&'a Rc<PathSet>>, Option<&'a Rc<PathSet>>)> {
    let mut left = entries(previous).peekable();
    let mut right = entries(incoming).peekable();
    std::iter::from_fn(move || match (left.peek(), right.peek()) {
        (Some((a, _)), Some((b, _))) if a == b => {
            let (slot, old) = left.next().unwrap();
            let (_, new) = right.next().unwrap();
            Some((slot, Some(old), Some(new)))
        }
        (Some((a, _)), Some((b, _))) if a < b => {
            let (slot, old) = left.next().unwrap();
            Some((slot, Some(old), None))
        }
        (Some(_), None) => {
            let (slot, old) = left.next().unwrap();
            Some((slot, Some(old), None))
        }
        (_, Some(_)) => {
            let (slot, new) = right.next().unwrap();
            Some((slot, None, Some(new)))
        }
        (None, None) => None,
    })
}

fn adds_paths(previous: &Rc<PathSet>, incoming: &Rc<PathSet>, budget: &Budget) -> Result<bool> {
    if Rc::ptr_eq(previous, incoming) {
        return Ok(false);
    }
    for path in &incoming.paths {
        lookup_work(&previous.paths, path.len(), budget)?;
        if !previous.paths.contains(path) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn union_paths(
    previous: &Rc<PathSet>,
    incoming: &Rc<PathSet>,
    budget: &Budget,
) -> Result<Rc<PathSet>> {
    if !adds_paths(previous, incoming, budget)? {
        return Ok(previous.clone());
    }
    if !adds_paths(incoming, previous, budget)? {
        return Ok(incoming.clone());
    }
    let mut words = PATH_SET_WORDS;
    for path in previous.paths.union(&incoming.paths) {
        budget.work(path.len().checked_add(1).ok_or(Error::Overflow)?)?;
        words = words
            .checked_add(path_words(path)?)
            .ok_or(Error::Overflow)?;
    }
    let storage = budget.reserve(words)?;
    budget.work(words)?;
    let paths = previous.paths.union(&incoming.paths).cloned().collect();
    Ok(Rc::new(PathSet {
        paths,
        _storage: storage,
    }))
}

fn merge_partials(
    previous: Option<&Rc<PartialPaths>>,
    incoming: Option<&Rc<PartialPaths>>,
    whole: u64,
    budget: &Budget,
) -> Result<Option<Rc<PartialPaths>>> {
    let mut count = 0;
    let mut slots = 0;
    let mut changed = false;
    let mut filtered = false;
    for (slot, old, new) in merged_entries(previous, incoming) {
        budget.work(1)?;
        if whole & (1u64 << slot) != 0 {
            changed |= old.is_some();
            filtered = true;
            continue;
        }
        count += 1;
        slots |= 1u64 << slot;
        changed |= match (old, new) {
            (Some(old), Some(new)) => adds_paths(old, new, budget)?,
            (None, Some(_)) => true,
            _ => false,
        };
    }
    if !changed {
        return Ok(previous.cloned());
    }
    if count == 0 {
        return Ok(None);
    }
    if previous.is_none() && !filtered {
        return Ok(incoming.cloned());
    }
    if let Some(incoming) = incoming {
        if partial_owner::incoming_is_union(previous, incoming, slots, whole, budget)? {
            return Ok(Some(incoming.clone()));
        }
    }
    let storage = partial_storage(count, budget)?;
    let mut result = Vec::with_capacity(count);
    for (slot, old, new) in merged_entries(previous, incoming) {
        budget.work(1)?;
        if whole & (1u64 << slot) != 0 {
            continue;
        }
        let paths = match (old, new) {
            (Some(old), Some(new)) => union_paths(old, new, budget)?,
            (Some(paths), None) | (None, Some(paths)) => paths.clone(),
            (None, None) => unreachable!(),
        };
        result.push(paths);
    }
    Ok(Some(Rc::new(PartialPaths {
        slots,
        entries: result.into_boxed_slice(),
        _storage: storage,
    })))
}

fn branch(bit: u32, zero: Rc<Node>, one: Rc<Node>, budget: &Budget) -> Result<Rc<Node>> {
    debug_assert!(bit.is_power_of_two() && zero.key() & bit == 0);
    let storage = NodeStorage::reserve(budget)?;
    budget.work(1)?;
    Ok(Rc::new(Node {
        kind: Kind::Branch {
            prefix: (zero.key() & !(bit - 1)) | bit,
            zero,
            one,
        },
        _storage: storage,
    }))
}

// Branch bits strictly decrease across the 26-bit group ID. Recursion remains
// independent of MIR nesting, graph depth, or adversarial insertion order.
fn insert(root: &Rc<Node>, leaf: Rc<Node>, budget: &Budget) -> Result<Rc<Node>> {
    branch_reuse::insert(root, leaf, budget)
}

fn remove(root: &Rc<Node>, group: u32, budget: &Budget) -> Result<Option<Rc<Node>>> {
    budget.work(1)?;
    match &root.kind {
        Kind::Leaf { group: found, .. } => Ok((*found != group).then(|| root.clone())),
        Kind::Branch { zero, one, .. } => {
            let bit = root.bit();
            let (selected, other) = if group & bit == 0 {
                (zero, one)
            } else {
                (one, zero)
            };
            let Some(changed) = remove(selected, group, budget)? else {
                return Ok(Some(other.clone()));
            };
            if Rc::ptr_eq(selected, &changed) {
                return Ok(Some(root.clone()));
            }
            Ok(Some(if group & bit == 0 {
                branch(bit, changed, other.clone(), budget)?
            } else {
                branch(bit, other.clone(), changed, budget)?
            }))
        }
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "state_resource_diagnostic_tests.rs"]
mod resource_diagnostic_tests;

#[cfg(test)]
#[path = "state_node_storage_v1_tests.rs"]
mod compact_tests;

#[cfg(test)]
#[path = "state_slot_storage_v1_tests.rs"]
mod compact_slot_tests;
