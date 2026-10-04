use fe2o3_contracts::{
    AccessKindV1, AddressSpaceIdV1, AffineWriteMappingV1, AllocationProvenanceIdV1,
    AllocationSpecV1, BrandedLaunchDomain1dV1, IndependentThreadContractV1, InitializationStateV1,
    LaunchIdentityV1, MAX_ALLOCATION_BYTES_V1, MAX_LAUNCH_THREADS_V1, MAX_READ_BINDINGS_V1,
    ObligationFailureV1, ObligationKindV1, ObligationResultV1, PermissionKindV1, ProofObligationV1,
    RegionBindingV1, RegionCapabilityV1, RegionPermissionV1, SpecificationFactV1,
};

fn allocation(id: u32, bytes: u64) -> AllocationSpecV1 {
    AllocationSpecV1::new(
        AllocationProvenanceIdV1::new(id).unwrap(),
        AddressSpaceIdV1::new(1).unwrap(),
        0x1_0000,
        bytes,
        0x1_0000 + bytes,
    )
    .unwrap()
}

fn region(allocation: AllocationSpecV1, offset: u64, bytes: u64) -> fe2o3_contracts::ByteRegionV1 {
    fe2o3_contracts::ByteRegionV1::for_allocation(allocation, offset, bytes).unwrap()
}

fn domain(threads: u64) -> BrandedLaunchDomain1dV1 {
    BrandedLaunchDomain1dV1::new(LaunchIdentityV1::new(7).unwrap(), threads).unwrap()
}

#[test]
fn symbolic_ids_are_nonzero_bounded_values_not_authorities() {
    assert_eq!(AllocationProvenanceIdV1::new(0), None);
    assert_eq!(AddressSpaceIdV1::new(0), None);
    assert_eq!(LaunchIdentityV1::new(0), None);
    assert_eq!(
        AllocationProvenanceIdV1::new(u32::MAX).unwrap().get(),
        u32::MAX
    );
    assert_eq!(AddressSpaceIdV1::new(u16::MAX).unwrap().get(), u16::MAX);
    assert_eq!(LaunchIdentityV1::new(u64::MAX).unwrap().get(), u64::MAX);

    fn is_specification<T: SpecificationFactV1>() {}
    is_specification::<AllocationProvenanceIdV1>();
    is_specification::<AllocationSpecV1>();
    is_specification::<ProofObligationV1>();
    is_specification::<IndependentThreadContractV1<0>>();
}

#[test]
fn allocations_check_empty_bounds_overflow_and_address_space_extent() {
    let id = AllocationProvenanceIdV1::new(1).unwrap();
    let space = AddressSpaceIdV1::new(1).unwrap();
    assert_eq!(
        AllocationSpecV1::new(id, space, 0, 0, 1),
        Err(ObligationFailureV1::EmptyAllocation)
    );
    assert_eq!(
        AllocationSpecV1::new(id, space, 0, 1, 0),
        Err(ObligationFailureV1::EmptyAddressSpace)
    );
    assert_eq!(
        AllocationSpecV1::new(id, space, 0, MAX_ALLOCATION_BYTES_V1 + 1, u64::MAX),
        Err(ObligationFailureV1::AllocationBoundExceeded {
            actual: MAX_ALLOCATION_BYTES_V1 + 1,
            maximum: MAX_ALLOCATION_BYTES_V1,
        })
    );
    assert_eq!(
        AllocationSpecV1::new(id, space, u64::MAX, 1, u64::MAX),
        Err(ObligationFailureV1::ArithmeticOverflow)
    );
    assert_eq!(
        AllocationSpecV1::new(id, space, 9, 2, 10),
        Err(ObligationFailureV1::AllocationOutsideAddressSpace {
            end: 11,
            address_space_size: 10,
        })
    );

    let largest = AllocationSpecV1::new(
        id,
        space,
        0,
        MAX_ALLOCATION_BYTES_V1,
        MAX_ALLOCATION_BYTES_V1,
    )
    .unwrap();
    assert_eq!(largest.end_address(), Some(MAX_ALLOCATION_BYTES_V1));
}

#[test]
fn byte_regions_are_nonempty_bounded_and_half_open() {
    let allocation = allocation(1, 16);
    let first = region(allocation, 0, 4);
    let touching = region(allocation, 4, 4);
    let overlapping = region(allocation, 3, 4);

    assert!(!first.overlaps(touching));
    assert!(first.overlaps(overlapping));
    assert_eq!(first.end_offset(), Some(4));
    assert_eq!(first.provenance(), allocation.provenance());
    assert_eq!(first.address_space(), allocation.address_space());
    assert_eq!(
        fe2o3_contracts::ByteRegionV1::for_allocation(allocation, 16, 1),
        Err(ObligationFailureV1::RegionOutsideAllocation)
    );
    assert_eq!(
        fe2o3_contracts::ByteRegionV1::new(
            allocation.provenance(),
            allocation.address_space(),
            0,
            0,
        ),
        Err(ObligationFailureV1::EmptyRegion)
    );
}

#[test]
fn provenance_and_address_space_are_part_of_region_identity() {
    let first_allocation = allocation(1, 16);
    let second_allocation = allocation(2, 16);
    let first = region(first_allocation, 0, 16);
    let second = region(second_allocation, 0, 16);
    assert!(!first.overlaps(second));
    assert!(!first_allocation.contains(second));

    let other_space = AddressSpaceIdV1::new(2).unwrap();
    let wrong_space =
        fe2o3_contracts::ByteRegionV1::new(first_allocation.provenance(), other_space, 0, 4)
            .unwrap();
    assert!(!first_allocation.contains(wrong_space));
}

#[test]
fn permission_compatibility_matches_independent_thread_rules() {
    let allocation = allocation(1, 16);
    let left = region(allocation, 0, 8);
    let right = region(allocation, 4, 8);
    let touching = region(allocation, 8, 8);

    let left_read = RegionPermissionV1::shared_read(left);
    let right_read = RegionPermissionV1::shared_read(right);
    let right_write = RegionPermissionV1::exclusive_write(right);
    let touching_write = RegionPermissionV1::exclusive_write(touching);

    assert!(left_read.compatible_with(right_read));
    assert!(!left_read.compatible_with(right_write));
    assert!(!right_write.compatible_with(left_read));
    assert!(left_read.compatible_with(touching_write));
    assert_eq!(right_write.kind(), PermissionKindV1::ExclusiveWrite);
}

#[test]
fn initialization_is_required_for_reads_and_established_by_writes() {
    let region = region(allocation(1, 4), 0, 4);
    let uninitialized_read = RegionCapabilityV1::new(
        RegionPermissionV1::shared_read(region),
        InitializationStateV1::Uninitialized,
    );
    let initialized_read = RegionCapabilityV1::initialized_read(region);
    let uninitialized_write =
        RegionCapabilityV1::writable(region, InitializationStateV1::Uninitialized);

    assert!(!uninitialized_read.permits(AccessKindV1::Read));
    assert!(initialized_read.permits(AccessKindV1::Read));
    assert!(!initialized_read.permits(AccessKindV1::Write));
    assert!(uninitialized_write.permits(AccessKindV1::Write));
    assert_eq!(
        uninitialized_write.state_after(AccessKindV1::Write),
        Some(InitializationStateV1::Initialized)
    );
    assert_eq!(uninitialized_read.state_after(AccessKindV1::Read), None);
}

#[test]
fn launch_brands_prevent_cross_domain_thread_reuse() {
    let first = domain(4);
    let second = BrandedLaunchDomain1dV1::new(LaunchIdentityV1::new(8).unwrap(), 4).unwrap();
    let thread = first.thread(3).unwrap();

    assert!(first.contains(thread));
    assert!(!second.contains(thread));
    assert_eq!(first.thread(4), None);
    assert_eq!(
        BrandedLaunchDomain1dV1::new(LaunchIdentityV1::new(1).unwrap(), MAX_LAUNCH_THREADS_V1 + 1,),
        Err(ObligationFailureV1::LaunchBoundExceeded {
            actual: MAX_LAUNCH_THREADS_V1 + 1,
            maximum: MAX_LAUNCH_THREADS_V1,
        })
    );
}

#[test]
fn affine_mapping_checks_disjoint_stride_and_domain_fit() {
    assert_eq!(
        AffineWriteMappingV1::new(0, 3, 4),
        Err(ObligationFailureV1::WriteMappingNotDisjoint {
            stride_bytes: 3,
            element_bytes: 4,
        })
    );
    let mapping = AffineWriteMappingV1::new(8, 8, 4).unwrap();
    let allocation = allocation(1, 32);
    let domain = domain(3);
    assert!(mapping.fits_domain(domain, allocation));
    assert_eq!(
        mapping.region_for(domain, domain.thread(2).unwrap(), allocation),
        Ok(region(allocation, 24, 4))
    );
    assert!(!mapping.fits_domain(crate_domain(4), allocation));
}

fn crate_domain(threads: u64) -> BrandedLaunchDomain1dV1 {
    domain(threads)
}

#[test]
fn affine_mapping_is_pairwise_injective_for_every_small_domain_pair() {
    let allocation = allocation(1, 8 * 32);
    let domain = domain(32);
    let mapping = AffineWriteMappingV1::identity(8).unwrap();

    for left in 0..domain.thread_count() {
        for right in 0..domain.thread_count() {
            assert!(mapping.is_injective_for(
                domain,
                domain.thread(left).unwrap(),
                domain.thread(right).unwrap(),
                allocation,
            ));
        }
    }
}

#[test]
fn proof_obligations_report_stable_kinds_and_failures() {
    let shared_allocation = allocation(1, 8);
    let domain = domain(2);
    let initialized = RegionCapabilityV1::initialized_read(region(shared_allocation, 0, 4));
    let uninitialized = RegionCapabilityV1::new(
        initialized.permission(),
        InitializationStateV1::Uninitialized,
    );

    let satisfied = ProofObligationV1::AccessPermitted {
        capability: initialized,
        access: AccessKindV1::Read,
    };
    assert_eq!(satisfied.kind(), ObligationKindV1::AccessPermitted);
    assert_eq!(
        satisfied.evaluate(),
        ObligationResultV1::Satisfied(ObligationKindV1::AccessPermitted)
    );

    assert_eq!(
        ProofObligationV1::AccessPermitted {
            capability: uninitialized,
            access: AccessKindV1::Read,
        }
        .evaluate(),
        ObligationResultV1::Unsatisfied {
            kind: ObligationKindV1::AccessPermitted,
            failure: ObligationFailureV1::ReadRequiresInitialization,
        }
    );
    assert!(
        ProofObligationV1::ThreadInDomain {
            domain,
            thread: domain.thread(1).unwrap(),
        }
        .evaluate()
        .is_satisfied()
    );
}

#[test]
fn obligations_cover_bounds_conflicts_mapping_and_transitions() {
    let output = allocation(1, 8);
    let input = allocation(2, 8);
    let domain = domain(2);
    let mapping = AffineWriteMappingV1::identity(4).unwrap();
    let write =
        RegionCapabilityV1::writable(region(output, 0, 4), InitializationStateV1::Uninitialized);
    let read = RegionCapabilityV1::initialized_read(region(input, 0, 4));

    let obligations = [
        ProofObligationV1::AllocationRepresentable(output),
        ProofObligationV1::RegionInBounds {
            allocation: input,
            region: read.permission().region(),
        },
        ProofObligationV1::PermissionsCompatible {
            left: read.permission(),
            right: write.permission(),
        },
        ProofObligationV1::WriteMappingFitsDomain {
            domain,
            mapping,
            allocation: output,
        },
        ProofObligationV1::WriteMappingInjective {
            domain,
            mapping,
            allocation: output,
            left: domain.thread(0).unwrap(),
            right: domain.thread(1).unwrap(),
        },
        ProofObligationV1::InitializationTransition {
            capability: write,
            access: AccessKindV1::Write,
            after: InitializationStateV1::Initialized,
        },
    ];
    assert!(
        obligations
            .into_iter()
            .all(|obligation| obligation.evaluate().is_satisfied())
    );
}

#[test]
fn independent_thread_contract_accepts_disjoint_initialized_inputs() {
    let input_a = allocation(1, 16);
    let input_b = allocation(2, 16);
    let output = allocation(3, 16);
    let domain = domain(4);
    let thread = domain.thread(2).unwrap();
    let mapping = AffineWriteMappingV1::identity(4).unwrap();
    let reads = [
        RegionBindingV1::new(
            input_a,
            RegionCapabilityV1::initialized_read(region(input_a, 8, 4)),
        ),
        RegionBindingV1::new(
            input_b,
            RegionCapabilityV1::initialized_read(region(input_b, 8, 4)),
        ),
    ];
    let output_binding = RegionBindingV1::new(
        output,
        RegionCapabilityV1::writable(region(output, 8, 4), InitializationStateV1::Uninitialized),
    );

    let facts = IndependentThreadContractV1::new(domain, thread, reads, output_binding, mapping)
        .evaluate()
        .unwrap();
    assert_eq!(facts.domain(), domain);
    assert_eq!(facts.thread(), thread);
    assert_eq!(facts.reads(), reads);
    assert_eq!(facts.output(), output_binding);
    assert_eq!(facts.write_mapping(), mapping);
    assert_eq!(
        facts.output_state_after_write(),
        InitializationStateV1::Initialized
    );
}

#[test]
fn independent_thread_contract_allows_shared_input_aliasing() {
    let input = allocation(1, 4);
    let output = allocation(2, 4);
    let domain = domain(1);
    let read = RegionBindingV1::new(
        input,
        RegionCapabilityV1::initialized_read(region(input, 0, 4)),
    );
    let write = RegionBindingV1::new(
        output,
        RegionCapabilityV1::writable(region(output, 0, 4), InitializationStateV1::Initialized),
    );
    assert!(
        IndependentThreadContractV1::new(
            domain,
            domain.thread(0).unwrap(),
            [read, read],
            write,
            AffineWriteMappingV1::identity(4).unwrap(),
        )
        .evaluate()
        .is_ok()
    );
}

#[test]
fn independent_thread_contract_rejects_every_access_boundary_violation() {
    let shared_allocation = allocation(1, 8);
    let domain = domain(2);
    let mapping = AffineWriteMappingV1::identity(4).unwrap();
    let write_region = region(shared_allocation, 0, 4);
    let good_write = RegionBindingV1::new(
        shared_allocation,
        RegionCapabilityV1::writable(write_region, InitializationStateV1::Uninitialized),
    );

    let other_domain = BrandedLaunchDomain1dV1::new(LaunchIdentityV1::new(9).unwrap(), 1).unwrap();
    assert_eq!(
        IndependentThreadContractV1::<0>::new(
            domain,
            other_domain.thread(0).unwrap(),
            [],
            good_write,
            mapping,
        )
        .evaluate(),
        Err(ObligationFailureV1::ThreadOutsideLaunchDomain)
    );

    let wrong_region_write = RegionBindingV1::new(
        shared_allocation,
        RegionCapabilityV1::writable(
            region(shared_allocation, 4, 4),
            InitializationStateV1::Uninitialized,
        ),
    );
    assert_eq!(
        IndependentThreadContractV1::<0>::new(
            domain,
            domain.thread(0).unwrap(),
            [],
            wrong_region_write,
            mapping,
        )
        .evaluate(),
        Err(ObligationFailureV1::WriteRegionMismatch)
    );

    let read_as_write = RegionBindingV1::new(
        shared_allocation,
        RegionCapabilityV1::initialized_read(write_region),
    );
    assert_eq!(
        IndependentThreadContractV1::<0>::new(
            domain,
            domain.thread(0).unwrap(),
            [],
            read_as_write,
            mapping,
        )
        .evaluate(),
        Err(ObligationFailureV1::WriteRequiresExclusivePermission)
    );
}

#[test]
fn independent_thread_contract_rejects_uninitialized_out_of_bounds_and_conflicting_reads() {
    let shared_allocation = allocation(1, 8);
    let domain = domain(2);
    let mapping = AffineWriteMappingV1::identity(4).unwrap();
    let write = RegionBindingV1::new(
        shared_allocation,
        RegionCapabilityV1::writable(
            region(shared_allocation, 0, 4),
            InitializationStateV1::Uninitialized,
        ),
    );
    let uninitialized_read = RegionBindingV1::new(
        shared_allocation,
        RegionCapabilityV1::new(
            RegionPermissionV1::shared_read(region(shared_allocation, 4, 4)),
            InitializationStateV1::Uninitialized,
        ),
    );
    assert_eq!(
        IndependentThreadContractV1::new(
            domain,
            domain.thread(0).unwrap(),
            [uninitialized_read],
            write,
            mapping,
        )
        .evaluate(),
        Err(ObligationFailureV1::ReadRequiresInitialization)
    );

    let conflict = RegionBindingV1::new(
        shared_allocation,
        RegionCapabilityV1::initialized_read(region(shared_allocation, 0, 4)),
    );
    assert_eq!(
        IndependentThreadContractV1::new(
            domain,
            domain.thread(0).unwrap(),
            [conflict],
            write,
            mapping,
        )
        .evaluate(),
        Err(ObligationFailureV1::PermissionsConflict)
    );

    let other = allocation(2, 8);
    let mismatched = RegionBindingV1::new(
        shared_allocation,
        RegionCapabilityV1::initialized_read(region(other, 0, 4)),
    );
    assert_eq!(
        IndependentThreadContractV1::new(
            domain,
            domain.thread(0).unwrap(),
            [mismatched],
            write,
            mapping,
        )
        .evaluate(),
        Err(ObligationFailureV1::RegionOutsideAllocation)
    );
}

#[test]
fn independent_thread_contract_enforces_read_count_bound() {
    let input_allocation = allocation(1, 4);
    let domain = domain(1);
    let read = RegionBindingV1::new(
        input_allocation,
        RegionCapabilityV1::initialized_read(region(input_allocation, 0, 4)),
    );
    let output_allocation = allocation(2, 4);
    let write = RegionBindingV1::new(
        output_allocation,
        RegionCapabilityV1::writable(
            region(output_allocation, 0, 4),
            InitializationStateV1::Uninitialized,
        ),
    );
    let contract = IndependentThreadContractV1::new(
        domain,
        domain.thread(0).unwrap(),
        [read; MAX_READ_BINDINGS_V1 + 1],
        write,
        AffineWriteMappingV1::identity(4).unwrap(),
    );
    assert_eq!(
        contract.evaluate(),
        Err(ObligationFailureV1::ReadBindingBoundExceeded {
            actual: MAX_READ_BINDINGS_V1 + 1,
            maximum: MAX_READ_BINDINGS_V1,
        })
    );
}

#[test]
fn empty_domain_mapping_has_a_defined_boundary_rule() {
    let domain = domain(0);
    let allocation = allocation(1, 8);
    assert!(
        AffineWriteMappingV1::new(8, 4, 4)
            .unwrap()
            .fits_domain(domain, allocation)
    );
    assert!(
        !AffineWriteMappingV1::new(9, 4, 4)
            .unwrap()
            .fits_domain(domain, allocation)
    );
}
