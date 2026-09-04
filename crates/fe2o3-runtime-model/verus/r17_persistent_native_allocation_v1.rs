// Abstract persistent-allocation model only. It reuses the structural identity
// shape of the R2 memory and R9 directional-route models, but no theorem links
// these mathematical values to executable Rust, KFD, an OS thread, Rust
// auto-traits, native mappings, queue publication, or GPU completion. XGMI is
// route-metadata classification only: it is not bound to this R2 mapping and
// grants no mapping/publication authority. This file does not model a
// two-registry atomic XGMI join or a 1 GiB aggregate owner.

use vstd::prelude::*;

verus! {

pub open spec fn max_allocation_bytes_v1() -> nat { 256 * 1024 * 1024 }
pub open spec fn max_use_slots_v1() -> nat { 64 }
pub open spec fn max_dependencies_v1() -> nat { 256 }
pub open spec fn memory_page_bytes_v1() -> nat { 4096 }
pub open spec fn local_sdma_engine_count_v1() -> nat { 2 }
pub open spec fn first_xgmi_engine_v1() -> nat { 2 }
pub open spec fn xgmi_engine_limit_v1() -> nat { 16 }
pub open spec fn max_u64_v1() -> nat { 0xffff_ffff_ffff_ffff }

#[derive(PartialEq, Eq)]
pub struct DeviceKeyV1 {
    pub physical: nat,
    pub generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct AllocationKeyV1 {
    pub vm_device: DeviceKeyV1,
    pub vm_id: nat,
    pub allocation_id: nat,
    pub allocation_generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct MappingKeyV1 {
    pub allocation: AllocationKeyV1,
    pub mapping_id: nat,
}

#[derive(PartialEq, Eq)]
pub struct QueueKeyV1 {
    pub device: DeviceKeyV1,
    pub vm_id: nat,
    pub queue_id: nat,
    pub generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct RouteV1 {
    pub source: DeviceKeyV1,
    pub destination: DeviceKeyV1,
    pub topology_identity: nat,
    pub topology_generation: nat,
    pub observation_epoch: nat,
    pub selected_engine: nat,
    pub current: bool,
}

#[derive(PartialEq, Eq)]
pub struct OwnerV1 {
    pub owner_id: nat,
    pub registry_incarnation: nat,
    pub allocation: AllocationKeyV1,
    pub mapping: MappingKeyV1,
    pub first_device: DeviceKeyV1,
    pub second_device: DeviceKeyV1,
    pub home_device: DeviceKeyV1,
    pub byte_len: nat,
    pub mapped_start: nat,
    pub mapped_end: nat,
    pub mapped_read_write: bool,
    pub current: bool,
    pub occupied_slots: nat,
    pub next_generation: nat,
}

pub open spec fn admitted_owner_v1(owner: OwnerV1) -> bool {
    &&& owner.owner_id > 0
    &&& owner.registry_incarnation > 0
    &&& owner.byte_len > 0
    &&& owner.byte_len <= max_allocation_bytes_v1()
    &&& owner.byte_len % memory_page_bytes_v1() == 0
    &&& owner.first_device.physical < owner.second_device.physical
    &&& owner.first_device.generation > 0
    &&& owner.second_device.generation > 0
    &&& (owner.home_device == owner.first_device || owner.home_device == owner.second_device)
    &&& owner.allocation.vm_device == owner.home_device
    &&& owner.allocation.vm_id > 0
    &&& owner.allocation.allocation_id > 0
    &&& owner.allocation.allocation_generation > 0
    &&& owner.mapping.allocation == owner.allocation
    &&& owner.mapping.mapping_id > 0
    &&& owner.mapped_start == 0
    &&& owner.mapped_end == 2
    &&& owner.mapped_read_write
    &&& owner.current
    &&& owner.occupied_slots <= max_use_slots_v1()
    &&& owner.next_generation > 0
}

#[derive(PartialEq, Eq)]
pub struct RangeV1 {
    pub offset: nat,
    pub byte_len: nat,
}

pub open spec fn valid_range_v1(owner: OwnerV1, range: RangeV1) -> bool {
    &&& range.byte_len > 0
    &&& range.offset + range.byte_len <= max_u64_v1()
    &&& range.offset + range.byte_len <= owner.byte_len
}

#[derive(PartialEq, Eq)]
pub enum AccessV1 { Read, Write }

#[derive(PartialEq, Eq)]
pub enum UseClassV1 {
    Compute(DeviceKeyV1, QueueKeyV1),
    LocalSdma(DeviceKeyV1, QueueKeyV1, nat),
    XgmiRouteMetadata(DeviceKeyV1, DeviceKeyV1, nat, RouteV1),
}

#[derive(PartialEq, Eq)]
pub struct DescriptorV1 {
    pub class: UseClassV1,
    pub access: AccessV1,
    pub range: RangeV1,
}

pub open spec fn owner_contains_device_v1(owner: OwnerV1, device: DeviceKeyV1) -> bool {
    device == owner.first_device || device == owner.second_device
}

pub open spec fn valid_queue_v1(owner: OwnerV1, device: DeviceKeyV1, queue: QueueKeyV1) -> bool {
    queue.device == device
        && queue.vm_id == owner.allocation.vm_id
        && queue.queue_id > 0
        && queue.generation > 0
}

pub open spec fn valid_route_v1(
    owner: OwnerV1,
    source: DeviceKeyV1,
    destination: DeviceKeyV1,
    engine: nat,
    route: RouteV1,
) -> bool {
    &&& source != destination
    &&& owner_contains_device_v1(owner, source)
    &&& owner_contains_device_v1(owner, destination)
    &&& route.source == source
    &&& route.destination == destination
    &&& route.topology_identity > 0
    &&& route.topology_generation > 0
    &&& route.observation_epoch > 0
    &&& route.current
    &&& engine == route.selected_engine
    &&& first_xgmi_engine_v1() <= engine
    &&& engine < xgmi_engine_limit_v1()
}

pub open spec fn valid_class_v1(owner: OwnerV1, descriptor: DescriptorV1) -> bool {
    match descriptor.class {
        UseClassV1::Compute(device, queue) =>
            device == owner.allocation.vm_device && valid_queue_v1(owner, device, queue),
        UseClassV1::LocalSdma(device, queue, engine) =>
            device == owner.allocation.vm_device
                && valid_queue_v1(owner, device, queue)
                && engine < local_sdma_engine_count_v1(),
        UseClassV1::XgmiRouteMetadata(source, destination, engine, route) =>
            valid_route_v1(owner, source, destination, engine, route)
                && ((owner.home_device == source && descriptor.access == AccessV1::Read)
                    || (owner.home_device == destination && descriptor.access == AccessV1::Write)),
    }
}

pub open spec fn valid_descriptor_v1(owner: OwnerV1, descriptor: DescriptorV1) -> bool {
    valid_range_v1(owner, descriptor.range) && valid_class_v1(owner, descriptor)
}

pub open spec fn overlaps_v1(left: RangeV1, right: RangeV1) -> bool {
    left.offset < right.offset + right.byte_len
        && right.offset < left.offset + left.byte_len
}

pub open spec fn conflicts_v1(left: DescriptorV1, right: DescriptorV1) -> bool {
    overlaps_v1(left.range, right.range)
        && (left.access == AccessV1::Write || right.access == AccessV1::Write)
}

#[derive(PartialEq, Eq)]
pub struct LeaseKeyV1 {
    pub owner_id: nat,
    pub registry_incarnation: nat,
    pub slot: nat,
    pub generation: nat,
}

pub open spec fn valid_lease_key_v1(owner: OwnerV1, key: LeaseKeyV1) -> bool {
    key.owner_id == owner.owner_id
        && key.registry_incarnation == owner.registry_incarnation
        && key.slot < max_use_slots_v1()
        && key.generation > 0
        && key.generation < owner.next_generation
}

#[derive(PartialEq, Eq)]
pub enum UsePhaseV1 { Reserved, Published, TimedOut, Terminal, Quarantined, Released }

#[derive(PartialEq, Eq)]
pub enum TerminalV1 { Succeeded, Failed }

#[derive(PartialEq, Eq)]
pub struct UseStateV1 {
    pub key: LeaseKeyV1,
    pub descriptor: DescriptorV1,
    pub phase: UsePhaseV1,
    pub terminal: Option<TerminalV1>,
    pub dependency_count: nat,
    pub dependencies_ready: bool,
}

pub open spec fn reserve_v1(
    owner: OwnerV1,
    descriptor: DescriptorV1,
    dependency_count: nat,
) -> OwnerV1 {
    if !admitted_owner_v1(owner)
        || !valid_descriptor_v1(owner, descriptor)
        || dependency_count > max_dependencies_v1()
        || owner.occupied_slots == max_use_slots_v1()
    {
        owner
    } else {
        OwnerV1 {
            occupied_slots: owner.occupied_slots + 1,
            next_generation: owner.next_generation + 1,
            ..owner
        }
    }
}

pub open spec fn publish_v1(
    owner: OwnerV1,
    use_state: UseStateV1,
    conflicting_active_use: bool,
    conflict_is_ready_dependency: bool,
) -> UseStateV1 {
    if !owner.current
        || use_state.phase != UsePhaseV1::Reserved
        || use_state.dependency_count > max_dependencies_v1()
        || !use_state.dependencies_ready
        || (conflicting_active_use && !conflict_is_ready_dependency)
    {
        use_state
    } else {
        UseStateV1 { phase: UsePhaseV1::Published, ..use_state }
    }
}

pub open spec fn timeout_v1(use_state: UseStateV1) -> UseStateV1 {
    if use_state.phase == UsePhaseV1::Published {
        UseStateV1 { phase: UsePhaseV1::TimedOut, ..use_state }
    } else {
        use_state
    }
}

pub open spec fn observe_terminal_v1(use_state: UseStateV1, terminal: TerminalV1) -> UseStateV1 {
    if use_state.phase == UsePhaseV1::Published || use_state.phase == UsePhaseV1::TimedOut {
        UseStateV1 { phase: UsePhaseV1::Terminal, terminal: Some(terminal), ..use_state }
    } else {
        use_state
    }
}

pub open spec fn lose_currentness_v1(owner: OwnerV1, use_state: UseStateV1) -> (OwnerV1, UseStateV1) {
    let next_owner = OwnerV1 { current: false, ..owner };
    let next_phase = if use_state.phase == UsePhaseV1::Reserved {
        UsePhaseV1::Released
    } else if use_state.phase == UsePhaseV1::Published
        || use_state.phase == UsePhaseV1::TimedOut
        || use_state.phase == UsePhaseV1::Terminal
    {
        UsePhaseV1::Quarantined
    } else {
        use_state.phase
    };
    (next_owner, UseStateV1 { phase: next_phase, ..use_state })
}

pub open spec fn can_release_terminal_v1(
    owner: OwnerV1,
    use_state: UseStateV1,
    has_reserved_dependent: bool,
) -> bool {
    owner.current
        && use_state.phase == UsePhaseV1::Terminal
        && use_state.terminal.is_some()
        && !has_reserved_dependent
}

pub open spec fn can_release_owner_v1(owner: OwnerV1) -> bool {
    owner.current && owner.occupied_slots == 0
}

pub open spec fn release_terminal_v1(
    owner: OwnerV1,
    use_state: UseStateV1,
    has_reserved_dependent: bool,
) -> (OwnerV1, UseStateV1) {
    if can_release_terminal_v1(owner, use_state, has_reserved_dependent) {
        (
            OwnerV1 { occupied_slots: (owner.occupied_slots - 1) as nat, ..owner },
            UseStateV1 { phase: UsePhaseV1::Released, ..use_state },
        )
    } else {
        (owner, use_state)
    }
}

pub open spec fn sample_device_v1(physical: nat, generation: nat) -> DeviceKeyV1 {
    DeviceKeyV1 { physical, generation }
}

pub open spec fn sample_owner_v1() -> OwnerV1 {
    let first = sample_device_v1(1, 3);
    let second = sample_device_v1(2, 4);
    let allocation = AllocationKeyV1 {
        vm_device: first,
        vm_id: 5,
        allocation_id: 6,
        allocation_generation: 7,
    };
    OwnerV1 {
        owner_id: 8,
        registry_incarnation: 13,
        allocation,
        mapping: MappingKeyV1 { allocation, mapping_id: 9 },
        first_device: first,
        second_device: second,
        home_device: first,
        byte_len: max_allocation_bytes_v1(),
        mapped_start: 0,
        mapped_end: 2,
        mapped_read_write: true,
        current: true,
        occupied_slots: 0,
        next_generation: 1,
    }
}

pub open spec fn sample_queue_v1(device: DeviceKeyV1, id: nat) -> QueueKeyV1 {
    QueueKeyV1 { device, vm_id: 5, queue_id: id, generation: 1 }
}

pub open spec fn sample_range_v1(offset: nat) -> RangeV1 {
    RangeV1 { offset, byte_len: 4096 }
}

pub open spec fn sample_route_v1(source: DeviceKeyV1, destination: DeviceKeyV1) -> RouteV1 {
    RouteV1 {
        source,
        destination,
        topology_identity: 10,
        topology_generation: 11,
        observation_epoch: 12,
        selected_engine: 3,
        current: true,
    }
}

pub proof fn exact_profile_bounds_are_fixed_v1()
    ensures
        max_allocation_bytes_v1() == 268435456,
        max_use_slots_v1() == 64,
        max_dependencies_v1() == 256,
{}

pub proof fn canonical_owner_is_inhabited_v1()
    ensures admitted_owner_v1(sample_owner_v1()),
{}

pub proof fn zero_allocation_is_rejected_v1()
    ensures !admitted_owner_v1(OwnerV1 { byte_len: 0, ..sample_owner_v1() }),
{}

pub proof fn allocation_above_profile_is_rejected_v1()
    ensures !admitted_owner_v1(OwnerV1 { byte_len: max_allocation_bytes_v1() + 1, ..sample_owner_v1() }),
{}

pub proof fn exact_two_device_identity_is_required_v1()
    ensures !admitted_owner_v1(OwnerV1 {
        second_device: sample_owner_v1().first_device,
        ..sample_owner_v1()
    }),
{}

pub proof fn allocation_and_mapping_substitution_are_rejected_v1()
    ensures !admitted_owner_v1(OwnerV1 {
        mapping: MappingKeyV1 {
            allocation: AllocationKeyV1 { allocation_id: 99, ..sample_owner_v1().allocation },
            mapping_id: 9,
        },
        ..sample_owner_v1()
    }),
{}

pub proof fn nonzero_bounded_range_is_admitted_v1()
    ensures valid_range_v1(sample_owner_v1(), RangeV1 {
        offset: 268431360,
        byte_len: 4096,
    }),
{}

pub proof fn out_of_extent_and_u64_overflow_ranges_are_rejected_v1()
    ensures
        !valid_range_v1(sample_owner_v1(), RangeV1 {
            offset: 268435455,
            byte_len: 2,
        }),
        !valid_range_v1(sample_owner_v1(), RangeV1 {
            offset: max_u64_v1(),
            byte_len: 2,
        }),
{}

pub proof fn compute_binding_is_exact_v1()
    ensures
        valid_class_v1(sample_owner_v1(), DescriptorV1 {
            class: UseClassV1::Compute(
                sample_owner_v1().first_device,
                sample_queue_v1(sample_owner_v1().first_device, 1),
            ),
            access: AccessV1::Read,
            range: sample_range_v1(0),
        }),
        !valid_class_v1(sample_owner_v1(), DescriptorV1 {
            class: UseClassV1::Compute(
                sample_owner_v1().first_device,
                QueueKeyV1 {
                    vm_id: 99,
                    ..sample_queue_v1(sample_owner_v1().first_device, 1)
                },
            ),
            access: AccessV1::Read,
            range: sample_range_v1(0),
        }),
{}

pub proof fn local_sdma_binding_is_exact_v1()
    ensures
        valid_class_v1(sample_owner_v1(), DescriptorV1 {
            class: UseClassV1::LocalSdma(
                sample_owner_v1().first_device,
                sample_queue_v1(sample_owner_v1().first_device, 2),
                1,
            ),
            access: AccessV1::Write,
            range: sample_range_v1(0),
        }),
        !valid_class_v1(sample_owner_v1(), DescriptorV1 {
            class: UseClassV1::LocalSdma(
                sample_owner_v1().first_device,
                sample_queue_v1(sample_owner_v1().first_device, 2),
                2,
            ),
            access: AccessV1::Write,
            range: sample_range_v1(0),
        }),
{}

pub proof fn xgmi_source_is_exact_read_endpoint_v1()
    ensures {
        let owner = sample_owner_v1();
        let route = sample_route_v1(owner.first_device, owner.second_device);
        &&& valid_class_v1(owner, DescriptorV1 {
            class: UseClassV1::XgmiRouteMetadata(
                owner.first_device,
                owner.second_device,
                3,
                route,
            ),
            access: AccessV1::Read,
            range: sample_range_v1(0),
        })
        &&& !valid_class_v1(owner, DescriptorV1 {
            class: UseClassV1::XgmiRouteMetadata(
                owner.first_device,
                owner.second_device,
                3,
                route,
            ),
            access: AccessV1::Write,
            range: sample_range_v1(0),
        })
    },
{}

pub proof fn xgmi_destination_is_exact_write_endpoint_v1()
    ensures {
        let base = sample_owner_v1();
        let owner = OwnerV1 {
            allocation: AllocationKeyV1 { vm_device: base.second_device, ..base.allocation },
            mapping: MappingKeyV1 {
                allocation: AllocationKeyV1 { vm_device: base.second_device, ..base.allocation },
                ..base.mapping
            },
            home_device: base.second_device,
            ..base
        };
        let route = sample_route_v1(owner.first_device, owner.second_device);
        &&& admitted_owner_v1(owner)
        &&& valid_class_v1(owner, DescriptorV1 {
            class: UseClassV1::XgmiRouteMetadata(
                owner.first_device,
                owner.second_device,
                3,
                route,
            ),
            access: AccessV1::Write,
            range: sample_range_v1(0),
        })
    },
{}

pub proof fn xgmi_route_metadata_roster_and_substitution_are_exact_v1()
    ensures {
        let owner = sample_owner_v1();
        let stale_route = RouteV1 {
            destination: owner.first_device,
            ..sample_route_v1(owner.first_device, owner.second_device)
        };
        let engine4 = RouteV1 {
            selected_engine: 4,
            ..sample_route_v1(owner.first_device, owner.second_device)
        };
        let engine15 = RouteV1 {
            selected_engine: 15,
            ..sample_route_v1(owner.first_device, owner.second_device)
        };
        &&& !valid_route_v1(owner, owner.first_device, owner.second_device, 3, stale_route)
        &&& valid_route_v1(owner, owner.first_device, owner.second_device, 4, engine4)
        &&& valid_route_v1(owner, owner.first_device, owner.second_device, 15, engine15)
        &&& !valid_route_v1(owner, owner.first_device, owner.second_device, 16, engine15)
    },
{}

pub proof fn overlapping_readers_are_compatible_v1()
    ensures !conflicts_v1(
        DescriptorV1 {
            class: UseClassV1::Compute(
                sample_owner_v1().first_device,
                sample_queue_v1(sample_owner_v1().first_device, 1),
            ),
            access: AccessV1::Read,
            range: sample_range_v1(0),
        },
        DescriptorV1 {
            class: UseClassV1::LocalSdma(
                sample_owner_v1().first_device,
                sample_queue_v1(sample_owner_v1().first_device, 2),
                1,
            ),
            access: AccessV1::Read,
            range: sample_range_v1(2048),
        },
    ),
{}

pub proof fn overlapping_writer_is_excluded_v1()
    ensures conflicts_v1(
        DescriptorV1 {
            class: UseClassV1::Compute(
                sample_owner_v1().first_device,
                sample_queue_v1(sample_owner_v1().first_device, 1),
            ),
            access: AccessV1::Read,
            range: sample_range_v1(0),
        },
        DescriptorV1 {
            class: UseClassV1::LocalSdma(
                sample_owner_v1().second_device,
                sample_queue_v1(sample_owner_v1().second_device, 2),
                1,
            ),
            access: AccessV1::Write,
            range: sample_range_v1(2048),
        },
    ),
{}

pub proof fn disjoint_writers_are_compatible_v1()
    ensures !conflicts_v1(
        DescriptorV1 {
            class: UseClassV1::Compute(
                sample_owner_v1().first_device,
                sample_queue_v1(sample_owner_v1().first_device, 1),
            ),
            access: AccessV1::Write,
            range: sample_range_v1(0),
        },
        DescriptorV1 {
            class: UseClassV1::Compute(
                sample_owner_v1().second_device,
                sample_queue_v1(sample_owner_v1().second_device, 2),
            ),
            access: AccessV1::Write,
            range: sample_range_v1(4096),
        },
    ),
{}

pub proof fn full_64_slot_reservation_failure_is_atomic_v1()
    ensures {
        let full = OwnerV1 { occupied_slots: 64, ..sample_owner_v1() };
        let descriptor = DescriptorV1 {
            class: UseClassV1::Compute(
                full.first_device,
                sample_queue_v1(full.first_device, 1),
            ),
            access: AccessV1::Read,
            range: sample_range_v1(0),
        };
        reserve_v1(full, descriptor, 0) == full
    },
{}

pub proof fn dependency_count_above_256_is_atomic_v1()
    ensures {
        let owner = sample_owner_v1();
        let descriptor = DescriptorV1 {
            class: UseClassV1::Compute(
                owner.first_device,
                sample_queue_v1(owner.first_device, 1),
            ),
            access: AccessV1::Read,
            range: sample_range_v1(0),
        };
        reserve_v1(owner, descriptor, 257) == owner
    },
{}

pub proof fn successful_reservation_advances_count_and_generation_v1()
    ensures {
        let owner = sample_owner_v1();
        let descriptor = DescriptorV1 {
            class: UseClassV1::Compute(
                owner.first_device,
                sample_queue_v1(owner.first_device, 1),
            ),
            access: AccessV1::Read,
            range: sample_range_v1(0),
        };
        let next = reserve_v1(owner, descriptor, 0);
        next.occupied_slots == 1 && next.next_generation == 2
    },
{}

pub proof fn unready_dependency_blocks_publication_atomically_v1(
    owner: OwnerV1,
    use_state: UseStateV1,
)
    requires use_state.phase == UsePhaseV1::Reserved, !use_state.dependencies_ready,
    ensures publish_v1(owner, use_state, false, false) == use_state,
{}

pub proof fn conflict_blocks_publication_atomically_v1(
    owner: OwnerV1,
    use_state: UseStateV1,
)
    requires use_state.phase == UsePhaseV1::Reserved,
    ensures publish_v1(owner, use_state, true, false) == use_state,
{}

pub proof fn ready_dependency_orders_named_conflict_v1(
    owner: OwnerV1,
    use_state: UseStateV1,
)
    requires
        owner.current,
        use_state.phase == UsePhaseV1::Reserved,
        use_state.dependencies_ready,
        use_state.dependency_count <= max_dependencies_v1(),
    ensures
        publish_v1(owner, use_state, false, false).phase == UsePhaseV1::Published,
        publish_v1(owner, use_state, true, true).phase == UsePhaseV1::Published,
{}

pub proof fn timeout_retains_exact_key_descriptor_and_nonterminal_status_v1(
    use_state: UseStateV1,
)
    requires use_state.phase == UsePhaseV1::Published,
    ensures
        timeout_v1(use_state).phase == UsePhaseV1::TimedOut,
        timeout_v1(use_state).key == use_state.key,
        timeout_v1(use_state).descriptor == use_state.descriptor,
        timeout_v1(use_state).terminal == use_state.terminal,
{}

pub proof fn exact_terminal_observation_retains_custody_v1(
    use_state: UseStateV1,
)
    requires use_state.phase == UsePhaseV1::Published,
    ensures
        observe_terminal_v1(use_state, TerminalV1::Succeeded).phase == UsePhaseV1::Terminal,
        observe_terminal_v1(use_state, TerminalV1::Succeeded).terminal == Some(TerminalV1::Succeeded),
        observe_terminal_v1(use_state, TerminalV1::Succeeded).key == use_state.key,
{}

pub proof fn currentness_loss_cancels_reserved_and_quarantines_published_v1(
    owner: OwnerV1,
    reserved: UseStateV1,
    published: UseStateV1,
)
    requires
        reserved.phase == UsePhaseV1::Reserved,
        published.phase == UsePhaseV1::Published,
    ensures
        !lose_currentness_v1(owner, reserved).0.current,
        lose_currentness_v1(owner, reserved).1.phase == UsePhaseV1::Released,
        lose_currentness_v1(owner, published).1.phase == UsePhaseV1::Quarantined,
{}

pub proof fn slot_reuse_requires_fresh_generation_v1(
    owner_id: nat,
    registry_incarnation: nat,
    slot: nat,
    generation: nat,
)
    requires slot < max_use_slots_v1(), generation > 0,
    ensures (LeaseKeyV1 { owner_id, registry_incarnation, slot, generation })
        != (LeaseKeyV1 {
            owner_id,
            registry_incarnation,
            slot,
            generation: generation + 1,
        }),
{}

pub proof fn reconstructed_registry_incarnation_rejects_old_key_v1(
    owner: OwnerV1,
    key: LeaseKeyV1,
    fresh_incarnation: nat,
)
    requires
        valid_lease_key_v1(owner, key),
        fresh_incarnation > 0,
        fresh_incarnation != owner.registry_incarnation,
    ensures !valid_lease_key_v1(
        OwnerV1 { registry_incarnation: fresh_incarnation, ..owner },
        key,
    ),
{}

pub proof fn quarantined_use_and_owner_are_not_releasable_v1(
    owner: OwnerV1,
    use_state: UseStateV1,
)
    requires !owner.current, use_state.phase == UsePhaseV1::Quarantined,
    ensures
        !can_release_terminal_v1(owner, use_state, false),
        !can_release_owner_v1(owner),
{}

pub proof fn reserved_dependent_blocks_terminal_release_atomically_v1(
    owner: OwnerV1,
    use_state: UseStateV1,
)
    requires use_state.phase == UsePhaseV1::Terminal, use_state.terminal.is_some(),
    ensures release_terminal_v1(owner, use_state, true) == (owner, use_state),
{}

pub proof fn exact_terminal_release_frees_one_slot_v1(
    owner: OwnerV1,
    use_state: UseStateV1,
)
    requires
        owner.current,
        owner.occupied_slots > 0,
        use_state.phase == UsePhaseV1::Terminal,
        use_state.terminal.is_some(),
    ensures
        release_terminal_v1(owner, use_state, false).0.occupied_slots
            == (owner.occupied_slots - 1) as nat,
        release_terminal_v1(owner, use_state, false).1.phase == UsePhaseV1::Released,
{}

pub proof fn failed_terminal_does_not_satisfy_dependency_v1()
    ensures TerminalV1::Failed != TerminalV1::Succeeded,
{}

pub proof fn inhabited_mixed_trace_is_nonvacuous_v1()
    ensures {
        let owner = sample_owner_v1();
        let compute = DescriptorV1 {
            class: UseClassV1::Compute(
                owner.first_device,
                sample_queue_v1(owner.first_device, 1),
            ),
            access: AccessV1::Read,
            range: sample_range_v1(0),
        };
        let sdma = DescriptorV1 {
            class: UseClassV1::LocalSdma(
                owner.first_device,
                sample_queue_v1(owner.first_device, 2),
                1,
            ),
            access: AccessV1::Read,
            range: sample_range_v1(2048),
        };
        let route = sample_route_v1(owner.first_device, owner.second_device);
        let xgmi = DescriptorV1 {
            class: UseClassV1::XgmiRouteMetadata(
                owner.first_device,
                owner.second_device,
                3,
                route,
            ),
            access: AccessV1::Read,
            range: sample_range_v1(8192),
        };
        &&& admitted_owner_v1(owner)
        &&& valid_descriptor_v1(owner, compute)
        &&& valid_descriptor_v1(owner, sdma)
        &&& valid_descriptor_v1(owner, xgmi)
        &&& !conflicts_v1(compute, sdma)
        &&& !conflicts_v1(sdma, xgmi)
    },
{}

}
