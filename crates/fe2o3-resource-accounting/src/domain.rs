//! One fixed-arena coordinator for opt-in aggregate accounting domains.

use super::*;
use fe2o3_runtime_model::{R70ResourceBatchErrorV1, r70_resource_batch_reserve_v1};

pub const MAX_RESOURCE_DOMAIN_DEPTH_V1: usize = 3;
pub const MAX_RESOURCE_CLASS_DOMAIN_DEPTH_V1: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Key {
    slot: usize,
    generation: u64,
}

const ROOT: Key = Key {
    slot: 0,
    generation: 1,
};

struct Node {
    key: Key,
    parent: Option<Key>,
    capacity: ResourceVectorV1,
    used: ResourceVectorV1,
    record_limit: usize,
    counts: [usize; 3],
    handles: usize,
    children: usize,
}

#[derive(Clone, Copy)]
struct DomainRecord {
    leaf: Key,
    credit: Record,
}

struct State {
    max_depth: usize,
    nodes: Vec<Option<Node>>,
    free_nodes: Vec<usize>,
    records: Vec<Option<DomainRecord>>,
    free_records: Vec<usize>,
    occupied: Vec<u64>,
    next_node: u64,
    next_owner: u64,
    poisoned: bool,
    quarantine_anchor: Option<Arc<Root>>,
}

pub(super) struct Root {
    state: Mutex<State>,
}

pub(super) struct DomainAccount {
    root: Arc<Root>,
    key: Key,
}

/// Fixed Rust coordinator and arena payload, including all vacant domain slots,
/// record slots, free lists and batch-validation scratch. Arc control headers,
/// allocator rounding/headers and external handles/output boxes are excluded.
pub fn resource_domain_bootstrap_bytes_v1(
    domains: usize,
    records: usize,
) -> Result<u64, ResourceCreditErrorV1> {
    if domains == 0 || domains > MAX_RESOURCE_CREDIT_RECORDS_V1 {
        return Err(ResourceCreditErrorV1::InvalidDomainCapacity);
    }
    if records == 0 || records > MAX_RESOURCE_CREDIT_RECORDS_V1 {
        return Err(ResourceCreditErrorV1::InvalidRecordCapacity);
    }
    let bytes = core::mem::size_of::<Root>()
        .checked_add(
            domains
                .checked_mul(core::mem::size_of::<Option<Node>>() + core::mem::size_of::<usize>())
                .ok_or(ResourceCreditErrorV1::AllocationFailed)?,
        )
        .and_then(|bytes| {
            bytes.checked_add(records.checked_mul(
                core::mem::size_of::<Option<DomainRecord>>() + core::mem::size_of::<usize>(),
            )?)
        })
        .and_then(|bytes| {
            bytes.checked_add(
                records
                    .div_ceil(64)
                    .checked_mul(core::mem::size_of::<u64>())?,
            )
        })
        .ok_or(ResourceCreditErrorV1::AllocationFailed)?;
    u64::try_from(bytes).map_err(|_| ResourceCreditErrorV1::AllocationFailed)
}

fn fixed_vec<T>(
    len: usize,
    mut value: impl FnMut(usize) -> T,
) -> Result<Vec<T>, ResourceCreditErrorV1> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(len)
        .map_err(|_| ResourceCreditErrorV1::AllocationFailed)?;
    if result.capacity() != len {
        return Err(ResourceCreditErrorV1::AllocationFailed);
    }
    result.extend((0..len).map(&mut value));
    Ok(result)
}

impl ResourceCreditAccountV1 {
    /// Creates a shared root, precharging its fixed arena payload as
    /// `ControlResidentBytes` before allocation. Children use those same arenas;
    /// they allocate no new ledger. All reservations update the complete ancestor
    /// path under one lock. Root capacity must include the bootstrap payload.
    ///
    /// This bounds only supplied charges in this participating domain. It does
    /// not canonicalize physical devices, prevent creation of unrelated roots,
    /// account Context/native bootstrap, or establish a process-memory ceiling.
    pub fn new_root(
        capacity: ResourceVectorV1,
        max_domains: usize,
        max_records: usize,
    ) -> Result<Self, ResourceCreditErrorV1> {
        Self::new_root_at_depth(
            capacity,
            max_domains,
            max_records,
            MAX_RESOURCE_DOMAIN_DEPTH_V1,
        )
    }

    /// Fixed four-level profile for root, device, session and resource class.
    /// Each class retains its own limits while session/device ceilings aggregate
    /// all classes. Storage and lifetime rules are identical to `new_root`.
    pub fn new_root_with_class_domains_v1(
        capacity: ResourceVectorV1,
        max_domains: usize,
        max_records: usize,
    ) -> Result<Self, ResourceCreditErrorV1> {
        Self::new_root_at_depth(
            capacity,
            max_domains,
            max_records,
            MAX_RESOURCE_CLASS_DOMAIN_DEPTH_V1,
        )
    }

    fn new_root_at_depth(
        capacity: ResourceVectorV1,
        max_domains: usize,
        max_records: usize,
        max_depth: usize,
    ) -> Result<Self, ResourceCreditErrorV1> {
        if ![
            MAX_RESOURCE_DOMAIN_DEPTH_V1,
            MAX_RESOURCE_CLASS_DOMAIN_DEPTH_V1,
        ]
        .contains(&max_depth)
        {
            return Err(ResourceCreditErrorV1::InvalidDomainCapacity);
        }
        let baseline = ResourceVectorV1::ZERO.with(
            ResourceKindV1::ControlResidentBytes,
            resource_domain_bootstrap_bytes_v1(max_domains, max_records)?,
        );
        let used = r67_resource_reserve_v1(ResourceVectorV1::ZERO, baseline, capacity)
            .ok_or(ResourceCreditErrorV1::Capacity)?;
        let mut nodes = fixed_vec(max_domains, |_| None)?;
        nodes[0] = Some(Node {
            key: ROOT,
            parent: None,
            capacity,
            used,
            record_limit: max_records,
            counts: [0; 3],
            handles: 1,
            children: 0,
        });
        let root = Arc::new(Root {
            state: Mutex::new(State {
                max_depth,
                nodes,
                // The root slot is permanent. Keep the complete free-list capacity
                // charged even though that index is never offered to a child.
                free_nodes: {
                    let mut free = fixed_vec(max_domains, |i| max_domains - 1 - i)?;
                    free.pop();
                    free
                },
                records: fixed_vec(max_records, |_| None)?,
                free_records: fixed_vec(max_records, |i| max_records - 1 - i)?,
                occupied: fixed_vec(max_records.div_ceil(64), |_| 0)?,
                next_node: 2,
                next_owner: 1,
                poisoned: false,
                quarantine_anchor: None,
            }),
        });
        Ok(Self(AccountHandle::Domain(DomainAccount {
            root,
            key: ROOT,
        })))
    }

    /// Creates an immutable child from this root's preallocated domain arena.
    /// Children inherit every ancestor ceiling; unused child capacity is not
    /// debited. Clones preserve exact leaf identity. A domain slot is reusable
    /// only after all handles, descendants and records are gone; quarantine
    /// prevents reuse. The root's immutable profile selects three or four levels.
    pub fn new_child(
        &self,
        capacity: ResourceVectorV1,
        max_records: usize,
    ) -> Result<Self, ResourceCreditErrorV1> {
        let AccountHandle::Domain(parent) = &self.0 else {
            return Err(ResourceCreditErrorV1::NotHierarchical);
        };
        let mut state = parent.root.lock();
        state.require_live()?;
        let result = (|| {
            let (_, depth) = state.path(parent.key)?;
            if depth == state.max_depth {
                return Err(ResourceCreditErrorV1::DomainDepth);
            }
            if max_records == 0 || max_records > state.records.len() {
                return Err(ResourceCreditErrorV1::InvalidRecordCapacity);
            }
            let slot = *state
                .free_nodes
                .last()
                .ok_or(ResourceCreditErrorV1::DomainCapacity)?;
            let generation = state.next_node;
            let next_node = generation
                .checked_add(1)
                .ok_or(ResourceCreditErrorV1::GenerationExhausted)?;
            let key = Key { slot, generation };
            let children = state
                .node(parent.key)?
                .children
                .checked_add(1)
                .ok_or(ResourceCreditErrorV1::Invariant)?;
            if generation == 0 || state.nodes.get(slot).is_none_or(Option::is_some) {
                return Err(ResourceCreditErrorV1::Invariant);
            }
            state.free_nodes.pop();
            state.next_node = next_node;
            state.node_mut(parent.key)?.children = children;
            state.nodes[slot] = Some(Node {
                key,
                parent: Some(parent.key),
                capacity,
                used: ResourceVectorV1::ZERO,
                record_limit: max_records,
                counts: [0; 3],
                handles: 1,
                children: 0,
            });
            Ok(Self(AccountHandle::Domain(DomainAccount {
                root: Arc::clone(&parent.root),
                key,
            })))
        })();
        if matches!(&result, Err(ResourceCreditErrorV1::Invariant)) {
            parent.root.poison(&mut state);
        }
        result
    }

    /// Whether both accounts participate in one root, not whether they share
    /// the same leaf limit. This grants no device or native ownership authority.
    pub fn shares_root_with(&self, other: &Self) -> bool {
        matches!((&self.0, &other.0), (AccountHandle::Domain(a), AccountHandle::Domain(b)) if Arc::ptr_eq(&a.root, &b.root))
    }

    /// Inert inclusive root usage. `None` means an independent account.
    pub fn root_usage(&self) -> Option<ResourceCreditUsageV1> {
        let AccountHandle::Domain(account) = &self.0 else {
            return None;
        };
        let state = account.root.lock();
        Some(state.usage(ROOT))
    }
}

impl Root {
    fn lock(self: &Arc<Self>) -> MutexGuard<'_, State> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                self.poison(&mut state);
                state
            }
        }
    }

    fn poison(self: &Arc<Self>, state: &mut State) -> ResourceCreditErrorV1 {
        state.poisoned = true;
        state
            .quarantine_anchor
            .get_or_insert_with(|| Arc::clone(self));
        ResourceCreditErrorV1::Invariant
    }
}

impl State {
    fn occupied_records(&self, key: Key) -> Result<usize, ResourceCreditErrorV1> {
        let node = self.node(key)?;
        node.counts
            .iter()
            .try_fold(0usize, |sum, &n| sum.checked_add(n))
            .filter(|&count| count <= node.record_limit)
            .ok_or(ResourceCreditErrorV1::Invariant)
    }

    fn check_record_total(&self) -> Result<(), ResourceCreditErrorV1> {
        if self
            .occupied_records(ROOT)?
            .checked_add(self.free_records.len())
            != Some(self.records.len())
        {
            return Err(ResourceCreditErrorV1::Invariant);
        }
        Ok(())
    }

    fn require_live(&self) -> Result<(), ResourceCreditErrorV1> {
        if self.poisoned {
            Err(ResourceCreditErrorV1::Invariant)
        } else {
            Ok(())
        }
    }

    fn node(&self, key: Key) -> Result<&Node, ResourceCreditErrorV1> {
        self.nodes
            .get(key.slot)
            .and_then(Option::as_ref)
            .filter(|node| node.key == key)
            .ok_or(ResourceCreditErrorV1::Invariant)
    }

    fn node_mut(&mut self, key: Key) -> Result<&mut Node, ResourceCreditErrorV1> {
        self.nodes
            .get_mut(key.slot)
            .and_then(Option::as_mut)
            .filter(|node| node.key == key)
            .ok_or(ResourceCreditErrorV1::Invariant)
    }

    fn path(
        &self,
        leaf: Key,
    ) -> Result<([Key; MAX_RESOURCE_CLASS_DOMAIN_DEPTH_V1], usize), ResourceCreditErrorV1> {
        let mut path = [ROOT; MAX_RESOURCE_CLASS_DOMAIN_DEPTH_V1];
        let mut next = Some(leaf);
        let mut depth = 0;
        while let Some(key) = next {
            if depth == path.len() || depth == self.max_depth {
                return Err(ResourceCreditErrorV1::Invariant);
            }
            path[depth] = key;
            depth += 1;
            next = self.node(key)?.parent;
        }
        if path[depth - 1] != ROOT {
            return Err(ResourceCreditErrorV1::Invariant);
        }
        Ok((path, depth))
    }

    fn usage(&self, key: Key) -> ResourceCreditUsageV1 {
        let node = self.node(key).expect("live domain identity");
        ResourceCreditUsageV1 {
            capacity: node.capacity,
            used: node.used,
            reserved_records: node.counts[0],
            retained_records: node.counts[1],
            quarantined_records: node.counts[2],
            record_capacity: node.record_limit,
            poisoned: self.poisoned,
        }
    }

    fn reap(&mut self, mut key: Key) -> Result<(), ResourceCreditErrorV1> {
        while key != ROOT {
            let node = self.node(key)?;
            if node.handles != 0 || node.children != 0 || node.counts != [0; 3] {
                break;
            }
            if node.used != ResourceVectorV1::ZERO {
                return Err(ResourceCreditErrorV1::Invariant);
            }
            let parent = node.parent.ok_or(ResourceCreditErrorV1::Invariant)?;
            if self.free_nodes.len() >= self.nodes.len() - 1 {
                return Err(ResourceCreditErrorV1::Invariant);
            }
            let children = self
                .node(parent)?
                .children
                .checked_sub(1)
                .ok_or(ResourceCreditErrorV1::Invariant)?;
            self.nodes[key.slot] = None;
            self.free_nodes.push(key.slot);
            self.node_mut(parent)?.children = children;
            key = parent;
        }
        Ok(())
    }
}

impl DomainAccount {
    pub(super) fn same_account(&self, other: &Self) -> bool {
        self.key == other.key && Arc::ptr_eq(&self.root, &other.root)
    }

    pub(super) fn usage(&self) -> ResourceCreditUsageV1 {
        self.root.lock().usage(self.key)
    }

    pub(super) fn reserve_into(
        &self,
        charges: &[ResourceVectorV1],
        output: &mut [ResourceReservationV1],
    ) -> Result<(), ResourceCreditErrorV1> {
        let mut state = self.root.lock();
        state.require_live()?;
        let result = (|| {
            state.check_record_total()?;
            if charges.is_empty()
                || charges.len() != output.len()
                || output.iter().any(|r| r.token.is_some())
            {
                return Err(ResourceCreditErrorV1::Invariant);
            }
            let (path, depth) = state.path(self.key)?;
            let count = charges.len();
            if count > state.free_records.len() {
                return Err(ResourceCreditErrorV1::RecordCapacity);
            }
            let mut next_used = [ResourceVectorV1::ZERO; MAX_RESOURCE_CLASS_DOMAIN_DEPTH_V1];
            let mut next_owner = state.next_owner;
            if next_owner == 0 {
                return Err(ResourceCreditErrorV1::Invariant);
            }
            for (index, &key) in path[..depth].iter().enumerate() {
                let node = state.node(key)?;
                let occupied = state.occupied_records(key)?;
                let free = node
                    .record_limit
                    .checked_sub(occupied)
                    .ok_or(ResourceCreditErrorV1::Invariant)?;
                let plan = r70_resource_batch_reserve_v1(
                    node.used,
                    charges,
                    node.capacity,
                    free,
                    state.next_owner,
                )
                .map_err(|error| match error {
                    R70ResourceBatchErrorV1::InvalidMemberCount => {
                        ResourceCreditErrorV1::InvalidRecordCapacity
                    }
                    R70ResourceBatchErrorV1::Capacity => ResourceCreditErrorV1::Capacity,
                    R70ResourceBatchErrorV1::RecordCapacity => {
                        ResourceCreditErrorV1::RecordCapacity
                    }
                    R70ResourceBatchErrorV1::GenerationExhausted => {
                        ResourceCreditErrorV1::GenerationExhausted
                    }
                })?;
                next_used[index] = plan.next_used;
                next_owner = plan.next_owner;
            }
            // Reuse charged scratch, never allocate while admitting or quarantining.
            if count > 1 {
                state.occupied.fill(0);
            }
            for i in 0..count {
                let slot = state.free_records[state.free_records.len() - 1 - i];
                if state.records.get(slot).is_none_or(Option::is_some) {
                    return Err(ResourceCreditErrorV1::Invariant);
                }
                if count > 1 {
                    let bit = 1 << (slot % 64);
                    if state.occupied[slot / 64] & bit != 0 {
                        return Err(ResourceCreditErrorV1::Invariant);
                    }
                    state.occupied[slot / 64] |= bit;
                }
            }
            let first_owner = state.next_owner;
            // Every ancestor, free slot and output handle passed preflight.
            for (&key, &used) in path[..depth].iter().zip(&next_used) {
                let node = state.node_mut(key).expect("preflight path");
                node.used = used;
                node.counts[0] += count;
            }
            state.next_owner = next_owner;
            for (index, (&charge, reservation)) in charges.iter().zip(output).enumerate() {
                let slot = state.free_records.pop().expect("preflight free record");
                let owner = first_owner + index as u64;
                state.records[slot] = Some(DomainRecord {
                    leaf: self.key,
                    credit: Record {
                        owner,
                        charge,
                        phase: Phase::Reserved,
                    },
                });
                reservation.token = Some(Token {
                    account: TokenAccount::Domain(Arc::clone(&self.root)),
                    slot,
                    owner,
                });
            }
            Ok(())
        })();
        if result == Err(ResourceCreditErrorV1::Invariant) {
            self.root.poison(&mut state);
        }
        result
    }

    #[cfg(test)]
    pub(super) fn transition(
        &self,
        slot: usize,
        owner: u64,
        action: Action,
    ) -> Result<(), ResourceCreditErrorV1> {
        self.root.transition(slot, owner, action, Some(self.key))
    }
}

impl Root {
    pub(super) fn account_for_record(
        self: &Arc<Self>,
        slot: usize,
        owner: u64,
    ) -> ResourceCreditAccountV1 {
        let mut state = self.lock();
        let record = state
            .records
            .get(slot)
            .copied()
            .flatten()
            .filter(|record| record.credit.owner == owner && record.credit.phase == Phase::Retained)
            .expect("retained credit keeps its exact domain record");
        let node = state
            .node_mut(record.leaf)
            .expect("record retains its domain");
        node.handles = node
            .handles
            .checked_add(1)
            .expect("domain handle count overflow");
        ResourceCreditAccountV1(AccountHandle::Domain(DomainAccount {
            root: Arc::clone(self),
            key: record.leaf,
        }))
    }

    pub(super) fn transition_record(
        self: &Arc<Self>,
        slot: usize,
        owner: u64,
        action: Action,
    ) -> Result<(), ResourceCreditErrorV1> {
        self.transition(slot, owner, action, None)
    }

    fn transition(
        self: &Arc<Self>,
        slot: usize,
        owner: u64,
        action: Action,
        expected_leaf: Option<Key>,
    ) -> Result<(), ResourceCreditErrorV1> {
        let mut state = self.lock();
        state.require_live()?;
        let result = (|| {
            state.check_record_total()?;
            let record = state
                .records
                .get(slot)
                .copied()
                .flatten()
                .ok_or(ResourceCreditErrorV1::Invariant)?;
            if expected_leaf.is_some_and(|leaf| record.leaf != leaf) {
                return Err(ResourceCreditErrorV1::Invariant);
            }
            let leaf = record.leaf;
            let record = record.credit;
            let phase = r67_credit_transition_v1(record.owner, owner, record.phase, action)
                .ok_or(ResourceCreditErrorV1::Invariant)?;
            let old = match record.phase {
                Phase::Reserved => 0,
                Phase::Retained => 1,
                _ => return Err(ResourceCreditErrorV1::Invariant),
            };
            let (path, depth) = state.path(leaf)?;
            let mut used = [ResourceVectorV1::ZERO; MAX_RESOURCE_CLASS_DOMAIN_DEPTH_V1];
            for (index, &key) in path[..depth].iter().enumerate() {
                let node = state.node(key)?;
                state.occupied_records(key)?;
                if node.counts[old] == 0 {
                    return Err(ResourceCreditErrorV1::Invariant);
                }
                used[index] = if phase == Phase::Vacant {
                    r67_resource_release_v1(node.used, record.charge)
                        .ok_or(ResourceCreditErrorV1::Invariant)?
                } else {
                    node.used
                };
            }
            if phase == Phase::Vacant && state.free_records.len() == state.records.len() {
                return Err(ResourceCreditErrorV1::Invariant);
            }
            for (&key, &used) in path[..depth].iter().zip(&used) {
                let node = state.node_mut(key).expect("preflight transition path");
                node.used = used;
                node.counts[old] -= 1;
                match phase {
                    Phase::Retained => node.counts[1] += 1,
                    Phase::Quarantined => node.counts[2] += 1,
                    _ => {}
                }
            }
            if phase == Phase::Vacant {
                state.records[slot] = None;
                state.free_records.push(slot);
                state.reap(leaf)?;
            } else {
                state.records[slot]
                    .as_mut()
                    .expect("live record")
                    .credit
                    .phase = phase;
            }
            if phase == Phase::Quarantined {
                state
                    .quarantine_anchor
                    .get_or_insert_with(|| Arc::clone(self));
            }
            Ok(())
        })();
        if result.is_err() {
            self.poison(&mut state);
        }
        result
    }
}

impl Clone for DomainAccount {
    fn clone(&self) -> Self {
        let mut state = self.root.lock();
        let node = state.node_mut(self.key).expect("live domain clone");
        node.handles = node
            .handles
            .checked_add(1)
            .expect("domain handle count overflow");
        Self {
            root: Arc::clone(&self.root),
            key: self.key,
        }
    }
}

impl Drop for DomainAccount {
    fn drop(&mut self) {
        let mut state = self.root.lock();
        if state.poisoned {
            return;
        }
        let result = (|| {
            let node = state.node_mut(self.key)?;
            node.handles = node
                .handles
                .checked_sub(1)
                .ok_or(ResourceCreditErrorV1::Invariant)?;
            state.reap(self.key)
        })();
        if result.is_err() {
            self.root.poison(&mut state);
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod class_tests;
