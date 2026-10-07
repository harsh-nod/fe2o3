// Executable scalar checks and bounded storage scan. Native identity extraction,
// complete custody roster extraction and physical/GPU nonaliasing are external.
// Reviewed Rust correspondence: src/r66_compute_sdma_coexistence.rs.
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

pub open spec fn in_domain_v1(domain: DomainV1, storage: StorageV1) -> bool {
    storage.allocation_id != 0
        && storage.generation != 0
        && storage.physical_device == domain.physical_device
        && storage.device_generation == domain.device_generation
        && storage.vm_id == domain.vm_id
}

pub open spec fn rosters_disjoint_v1(
    domain: DomainV1, compute: Seq<StorageV1>, copies: Seq<StorageV1>,
) -> bool {
    &&& 0 < compute.len() <= 16
    &&& copies.len() <= 258
    &&& forall|i: int| 0 <= i < compute.len() ==> in_domain_v1(domain, compute[i])
    &&& forall|i: int, j: int| 0 <= j < i < compute.len()
        ==> compute[i].allocation_id != compute[j].allocation_id
    &&& forall|k: int| 0 <= k < copies.len() ==> in_domain_v1(domain, copies[k])
    &&& forall|k: int, j: int| 0 <= k < copies.len() && 0 <= j < compute.len()
        ==> copies[k].allocation_id != compute[j].allocation_id
}

pub fn device_storage_in_domain_v1(domain: DomainV1, storage: StorageV1) -> (result: bool)
    ensures result == in_domain_v1(domain, storage),
{
    storage.allocation_id != 0
        && storage.generation != 0
        && storage.physical_device == domain.physical_device
        && storage.device_generation == domain.device_generation
        && storage.vm_id == domain.vm_id
}

pub fn device_storage_rosters_disjoint_v1(
    domain: DomainV1, compute: &[StorageV1], copy_devices: &[StorageV1],
) -> (result: bool)
    ensures result == rosters_disjoint_v1(domain, compute@, copy_devices@),
{
    if compute.is_empty() || compute.len() > 16 || copy_devices.len() > 258 {
        return false;
    }
    let mut i = 0usize;
    while i < compute.len()
        invariant
            0 < compute.len() <= 16,
            copy_devices.len() <= 258,
            0 <= i <= compute.len(),
            forall|x: int| 0 <= x < i ==> in_domain_v1(domain, compute@[x]),
            forall|x: int, y: int| 0 <= y < x < i
                ==> compute@[x].allocation_id != compute@[y].allocation_id,
        decreases compute.len() - i,
    {
        if !device_storage_in_domain_v1(domain, compute[i]) {
            return false;
        }
        let mut j = 0usize;
        while j < i
            invariant
                0 < compute.len() <= 16,
                copy_devices.len() <= 258,
                0 <= i < compute.len(),
                0 <= j <= i,
                in_domain_v1(domain, compute@[i as int]),
                forall|x: int| 0 <= x < i ==> in_domain_v1(domain, compute@[x]),
                forall|x: int, y: int| 0 <= y < x < i
                    ==> compute@[x].allocation_id != compute@[y].allocation_id,
                forall|y: int| 0 <= y < j
                    ==> compute@[i as int].allocation_id != compute@[y].allocation_id,
            decreases i - j,
        {
            if compute[i].allocation_id == compute[j].allocation_id {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    let mut k = 0usize;
    while k < copy_devices.len()
        invariant
            0 < compute.len() <= 16,
            copy_devices.len() <= 258,
            0 <= k <= copy_devices.len(),
            forall|x: int| 0 <= x < compute.len() ==> in_domain_v1(domain, compute@[x]),
            forall|x: int, y: int| 0 <= y < x < compute.len()
                ==> compute@[x].allocation_id != compute@[y].allocation_id,
            forall|x: int| 0 <= x < k ==> in_domain_v1(domain, copy_devices@[x]),
            forall|x: int, y: int| 0 <= x < k && 0 <= y < compute.len()
                ==> copy_devices@[x].allocation_id != compute@[y].allocation_id,
        decreases copy_devices.len() - k,
    {
        if !device_storage_in_domain_v1(domain, copy_devices[k]) {
            return false;
        }
        let mut j = 0usize;
        while j < compute.len()
            invariant
                0 < compute.len() <= 16,
                copy_devices.len() <= 258,
                0 <= k < copy_devices.len(),
                0 <= j <= compute.len(),
                forall|x: int| 0 <= x < compute.len() ==> in_domain_v1(domain, compute@[x]),
                forall|x: int, y: int| 0 <= y < x < compute.len()
                    ==> compute@[x].allocation_id != compute@[y].allocation_id,
                forall|x: int| 0 <= x < k ==> in_domain_v1(domain, copy_devices@[x]),
                forall|x: int, y: int| 0 <= x < k && 0 <= y < compute.len()
                    ==> copy_devices@[x].allocation_id != compute@[y].allocation_id,
                in_domain_v1(domain, copy_devices@[k as int]),
                forall|y: int| 0 <= y < j
                    ==> copy_devices@[k as int].allocation_id != compute@[y].allocation_id,
            decreases compute.len() - j,
        {
            if copy_devices[k].allocation_id == compute[j].allocation_id {
                return false;
            }
            j += 1;
        }
        k += 1;
    }
    true
}

pub open spec fn window_slot_matches_v1(
    slot: usize, anchor: usize, packet_count: usize,
    generation: u32, expected_generation: u32, completion_value: u32,
) -> bool {
    slot < 64 && anchor < 64 && 0 < packet_count < 64
        && generation != 0 && generation == expected_generation
        && completion_value == generation
        && (slot as int + 64 - anchor as int) % 64 < packet_count
}

pub fn window_slot_check_v1(
    slot: usize, anchor: usize, packet_count: usize,
    generation: u32, expected_generation: u32, completion_value: u32,
) -> (result: bool)
    ensures result == window_slot_matches_v1(
        slot, anchor, packet_count, generation, expected_generation, completion_value),
{
    slot < 64 && anchor < 64 && packet_count != 0 && packet_count < 64
        && generation != 0 && generation == expected_generation
        && completion_value == generation
        && (slot + 64 - anchor) % 64 < packet_count
}

pub proof fn admitted_rosters_bound_work_v1(domain: DomainV1, compute: Seq<StorageV1>, copies: Seq<StorageV1>)
    requires rosters_disjoint_v1(domain, compute, copies),
    ensures compute.len() <= 16, copies.len() <= 258,
        compute.len() * copies.len() <= 4128,
{
    assert(compute.len() * copies.len() <= 16 * 258) by (nonlinear_arith)
        requires compute.len() <= 16, copies.len() <= 258;
}

pub proof fn admitted_endpoint_domain_v1(
    domain: DomainV1, compute: Seq<StorageV1>, copies: Seq<StorageV1>, index: int,
)
    requires rosters_disjoint_v1(domain, compute, copies), 0 <= index < copies.len(),
    ensures copies[index].allocation_id != 0, copies[index].generation != 0,
        copies[index].physical_device == domain.physical_device,
        copies[index].device_generation == domain.device_generation,
        copies[index].vm_id == domain.vm_id,
{}

pub proof fn admitted_every_endpoint_disjoint_v1(
    domain: DomainV1, compute: Seq<StorageV1>, copies: Seq<StorageV1>, ci: int, si: int,
)
    requires rosters_disjoint_v1(domain, compute, copies),
        0 <= ci < compute.len(), 0 <= si < copies.len(),
    ensures compute[ci].allocation_id != copies[si].allocation_id,
{}

pub proof fn changed_generation_does_not_hide_alias_v1(
    domain: DomainV1, compute: Seq<StorageV1>, copies: Seq<StorageV1>, ci: int, si: int,
)
    requires 0 <= ci < compute.len(), 0 <= si < copies.len(),
        compute[ci].allocation_id == copies[si].allocation_id,
    ensures !rosters_disjoint_v1(domain, compute, copies),
{}

pub proof fn admitted_window_exact_generation_v1(
    slot: usize, anchor: usize, count: usize, generation: u32, expected: u32, completion: u32,
)
    requires window_slot_matches_v1(slot, anchor, count, generation, expected, completion),
    ensures generation != 0, generation == expected, completion == generation,
{}

pub proof fn admitted_window_contains_slot_v1(
    slot: usize, anchor: usize, count: usize, generation: u32, expected: u32, completion: u32,
)
    requires window_slot_matches_v1(slot, anchor, count, generation, expected, completion),
    ensures slot < 64, anchor < 64, 0 < count < 64,
        (slot as int + 64 - anchor as int) % 64 < count,
{}
}
