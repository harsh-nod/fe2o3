// Executable device cache policy scans with reviewed Rust correspondence to
// src/r71_device_pool.rs. Native exact-record/padded-cost extraction, cache
// mutation, N2 charge retention and physical disposal are external obligations.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy)]
pub struct DomainV1 {
    pub physical_device: u64,
    pub device_generation: u64,
    pub vm_id: u64,
}

#[derive(Clone, Copy)]
pub struct StorageV1 {
    pub allocation_id: u64,
    pub generation: u64,
    pub physical_device: u64,
    pub device_generation: u64,
    pub vm_id: u64,
}

#[derive(Clone, Copy)]
pub struct EntryV1 {
    pub storage: StorageV1,
    pub pool_generation: u64,
    pub backing_bytes: u64,
}

pub struct UsageV1 {
    pub cached_backing_bytes: u64,
    pub cached_buffers: usize,
}

pub enum DispositionV1 { Cache, Dispose }

pub open spec fn limits_valid_v1(bytes: u64, buffers: usize) -> bool {
    bytes <= 206158430208 && buffers <= 128
}

pub open spec fn entry_valid_v1(domain: DomainV1, entry: EntryV1) -> bool {
    &&& domain.device_generation != 0
    &&& domain.vm_id != 0
    &&& entry.storage.allocation_id != 0
    &&& entry.storage.generation != 0
    &&& entry.storage.physical_device == domain.physical_device
    &&& entry.storage.device_generation == domain.device_generation
    &&& entry.storage.vm_id == domain.vm_id
    &&& entry.pool_generation != 0
    &&& 0 < entry.backing_bytes <= 206158430208
}

pub open spec fn prefix_bytes_v1(entries: Seq<EntryV1>, end: int) -> int
    decreases end,
{
    if end <= 0 { 0 }
    else { prefix_bytes_v1(entries, end - 1) + (entries[end - 1].backing_bytes as int) }
}

pub open spec fn roster_valid_v1(
    domain: DomainV1, entries: Seq<EntryV1>, max_bytes: u64, max_buffers: usize,
) -> bool {
    &&& limits_valid_v1(max_bytes, max_buffers)
    &&& domain.device_generation != 0
    &&& domain.vm_id != 0
    &&& entries.len() <= max_buffers
    &&& forall|i: int| 0 <= i < entries.len() ==> entry_valid_v1(domain, entries[i])
    &&& forall|i: int, j: int| 0 <= j < i < entries.len()
        ==> entries[i].storage.allocation_id != entries[j].storage.allocation_id
    &&& forall|end: int| 1 <= end <= entries.len()
        ==> prefix_bytes_v1(entries, end) <= max_bytes
}

pub open spec fn recycle_valid_v1(
    domain: DomainV1, entries: Seq<EntryV1>, candidate: EntryV1,
    max_bytes: u64, max_buffers: usize,
) -> bool {
    &&& roster_valid_v1(domain, entries, max_bytes, max_buffers)
    &&& entry_valid_v1(domain, candidate)
    &&& forall|i: int| 0 <= i < entries.len()
        ==> entries[i].storage.allocation_id != candidate.storage.allocation_id
}

pub open spec fn cache_fits_v1(
    entries: Seq<EntryV1>, candidate: EntryV1, max_bytes: u64, max_buffers: usize,
) -> bool {
    prefix_bytes_v1(entries, entries.len() as int) + (candidate.backing_bytes as int) <= max_bytes
        && entries.len() + 1 <= max_buffers
}

pub fn device_pool_limits_valid_v1(bytes: u64, buffers: usize) -> (result: bool)
    ensures result == limits_valid_v1(bytes, buffers),
{
    bytes <= 206158430208 && buffers <= 128
}

pub fn device_pool_entry_valid_v1(domain: DomainV1, entry: EntryV1) -> (result: bool)
    ensures result == entry_valid_v1(domain, entry),
{
    domain.device_generation != 0
        && domain.vm_id != 0
        && entry.storage.allocation_id != 0
        && entry.storage.generation != 0
        && entry.storage.physical_device == domain.physical_device
        && entry.storage.device_generation == domain.device_generation
        && entry.storage.vm_id == domain.vm_id
        && entry.pool_generation != 0
        && entry.backing_bytes != 0
        && entry.backing_bytes <= 206158430208
}

pub fn device_pool_usage_v1(
    domain: DomainV1, entries: &[EntryV1], max_bytes: u64, max_buffers: usize,
) -> (result: Option<UsageV1>)
    ensures
        result.is_some() == roster_valid_v1(domain, entries@, max_bytes, max_buffers),
        match result {
            Some(usage) => {
                &&& (usage.cached_backing_bytes as int) == prefix_bytes_v1(entries@, entries.len() as int)
                &&& usage.cached_backing_bytes <= max_bytes
                &&& usage.cached_buffers == entries.len()
            },
            None => true,
        },
{
    if !device_pool_limits_valid_v1(max_bytes, max_buffers)
        || domain.device_generation == 0 || domain.vm_id == 0
        || entries.len() > max_buffers
    { return None; }
    let mut bytes = 0u64;
    let mut i = 0usize;
    while i < entries.len()
        invariant
            limits_valid_v1(max_bytes, max_buffers),
            domain.device_generation != 0, domain.vm_id != 0,
            entries.len() <= max_buffers,
            0 <= i <= entries.len(),
            (bytes as int) == prefix_bytes_v1(entries@, i as int),
            bytes <= max_bytes,
            forall|x: int| 0 <= x < i ==> entry_valid_v1(domain, entries@[x]),
            forall|x: int, y: int| 0 <= y < x < i
                ==> entries@[x].storage.allocation_id != entries@[y].storage.allocation_id,
            forall|end: int| 1 <= end <= i ==> prefix_bytes_v1(entries@, end) <= max_bytes,
        decreases entries.len() - i,
    {
        if !device_pool_entry_valid_v1(domain, entries[i]) { return None; }
        let mut j = 0usize;
        while j < i
            invariant
                limits_valid_v1(max_bytes, max_buffers),
                domain.device_generation != 0, domain.vm_id != 0,
                entries.len() <= max_buffers,
                0 <= i < entries.len(), 0 <= j <= i,
                (bytes as int) == prefix_bytes_v1(entries@, i as int),
                bytes <= max_bytes,
                entry_valid_v1(domain, entries@[i as int]),
                forall|x: int| 0 <= x < i ==> entry_valid_v1(domain, entries@[x]),
                forall|x: int, y: int| 0 <= y < x < i
                    ==> entries@[x].storage.allocation_id != entries@[y].storage.allocation_id,
                forall|y: int| 0 <= y < j
                    ==> entries@[i as int].storage.allocation_id != entries@[y].storage.allocation_id,
                forall|end: int| 1 <= end <= i ==> prefix_bytes_v1(entries@, end) <= max_bytes,
            decreases i - j,
        {
            if entries[i].storage.allocation_id == entries[j].storage.allocation_id { return None; }
            j += 1;
        }
        let next_bytes = match bytes.checked_add(entries[i].backing_bytes) {
            Some(value) => value,
            None => return None,
        };
        proof {
            assert(prefix_bytes_v1(entries@, (i as int) + 1)
                == (bytes as int) + (entries@[i as int].backing_bytes as int));
        }
        if next_bytes > max_bytes {
            proof { assert(!roster_valid_v1(domain, entries@, max_bytes, max_buffers)); }
            return None;
        }
        proof {
            assert forall|end: int| 1 <= end <= (i as int) + 1 implies
                prefix_bytes_v1(entries@, end) <= max_bytes by {
                if end == (i as int) + 1 {
                    assert(prefix_bytes_v1(entries@, end) == (next_bytes as int));
                }
            }
        }
        bytes = next_bytes;
        i += 1;
    }
    Some(UsageV1 { cached_backing_bytes: bytes, cached_buffers: entries.len() })
}

pub fn device_pool_recycle_v1(
    domain: DomainV1, entries: &[EntryV1], candidate: EntryV1,
    max_bytes: u64, max_buffers: usize,
) -> (result: Option<DispositionV1>)
    ensures
        result.is_some() == recycle_valid_v1(domain, entries@, candidate, max_bytes, max_buffers),
        match result {
            Some(DispositionV1::Cache) => cache_fits_v1(entries@, candidate, max_bytes, max_buffers),
            Some(DispositionV1::Dispose) => !cache_fits_v1(entries@, candidate, max_bytes, max_buffers),
            None => true,
        },
{
    let usage = match device_pool_usage_v1(domain, entries, max_bytes, max_buffers) {
        Some(usage) => usage,
        None => return None,
    };
    if !device_pool_entry_valid_v1(domain, candidate) { return None; }
    let mut i = 0usize;
    while i < entries.len()
        invariant
            roster_valid_v1(domain, entries@, max_bytes, max_buffers),
            entry_valid_v1(domain, candidate),
            (usage.cached_backing_bytes as int) == prefix_bytes_v1(entries@, entries.len() as int),
            usage.cached_backing_bytes <= max_bytes,
            usage.cached_buffers == entries.len(),
            0 <= i <= entries.len(),
            forall|x: int| 0 <= x < i
                ==> entries@[x].storage.allocation_id != candidate.storage.allocation_id,
        decreases entries.len() - i,
    {
        if entries[i].storage.allocation_id == candidate.storage.allocation_id { return None; }
        i += 1;
    }
    let next_bytes = match usage.cached_backing_bytes.checked_add(candidate.backing_bytes) {
        Some(value) => value,
        None => return None,
    };
    let next_buffers = match usage.cached_buffers.checked_add(1) {
        Some(value) => value,
        None => return None,
    };
    Some(if next_bytes <= max_bytes && next_buffers <= max_buffers {
        DispositionV1::Cache
    } else {
        DispositionV1::Dispose
    })
}

pub proof fn admitted_every_record_has_exact_domain_and_positive_cost_v1(
    domain: DomainV1, entries: Seq<EntryV1>, max_bytes: u64, max_buffers: usize, index: int,
)
    requires roster_valid_v1(domain, entries, max_bytes, max_buffers), 0 <= index < entries.len(),
    ensures entry_valid_v1(domain, entries[index]), entries.len() <= 128,
{}

pub proof fn changed_generation_cannot_hide_cached_alias_v1(
    domain: DomainV1, entries: Seq<EntryV1>, candidate: EntryV1,
    max_bytes: u64, max_buffers: usize, index: int,
)
    requires 0 <= index < entries.len(), entries[index].storage.allocation_id == candidate.storage.allocation_id,
    ensures !recycle_valid_v1(domain, entries, candidate, max_bytes, max_buffers),
{}

pub proof fn zero_record_ceiling_never_caches_v1(
    entries: Seq<EntryV1>, candidate: EntryV1, max_bytes: u64,
)
    ensures !cache_fits_v1(entries, candidate, max_bytes, 0),
{}

pub proof fn zero_byte_ceiling_never_caches_v1(
    domain: DomainV1, entries: Seq<EntryV1>, candidate: EntryV1, max_buffers: usize,
)
    requires recycle_valid_v1(domain, entries, candidate, 0, max_buffers),
    ensures !cache_fits_v1(entries, candidate, 0, max_buffers),
{
    if entries.len() > 0 {
        assert(entry_valid_v1(domain, entries[0]));
        reveal_with_fuel(prefix_bytes_v1, 2);
        assert(prefix_bytes_v1(entries, 0) == 0);
        assert(prefix_bytes_v1(entries, 1) == (entries[0].backing_bytes as int));
        assert(prefix_bytes_v1(entries, 1) <= 0);
    }
}
}
