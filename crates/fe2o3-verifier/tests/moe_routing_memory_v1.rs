use fe2o3_verifier::{
    MOE_ROUTING_MEMORY_ACCESS_WIDTH_V1, MOE_ROUTING_MEMORY_DROP_ROUTE_V1,
    MOE_ROUTING_MEMORY_GLOBAL_ADDRESS_SPACE_V1, MoeRoutingLogicalAccessV1,
    MoeRoutingLogicalIndexKindV1, MoeRoutingLogicalIndexV1, MoeRoutingMemoryBufferV1,
    MoeRoutingMemoryContractErrorV1, MoeRoutingMemoryEffectKindV1, MoeRoutingMemoryIdentitiesV1,
    MoeRoutingMemoryPhaseV1, MoeRoutingMemoryRegionV1, MoeRoutingMemoryRegionsV1,
    MoeRoutingMemoryVerusExpectedEvidenceV1, check_moe_routing_memory_contract_v1,
    validate_moe_routing_logical_access_v1, validate_moe_routing_logical_index_v1,
    validate_moe_routing_phase_transition_v1,
};

fn region(base: u64, bytes: u64) -> MoeRoutingMemoryRegionV1 {
    MoeRoutingMemoryRegionV1 { base, bytes }
}

fn regions() -> MoeRoutingMemoryRegionsV1 {
    MoeRoutingMemoryRegionsV1 {
        logits: region(0x1000, 128),
        top2_experts: region(0x2000, 64),
        requested_counts: region(0x3000, 16),
        admitted_counts: region(0x4000, 16),
        expert_offsets: region(0x5000, 20),
        route_slots: region(0x6000, 64),
        permutation: region(0x7000, 64),
        inverse: region(0x8000, 64),
    }
}

fn output_write() -> MoeRoutingLogicalAccessV1 {
    MoeRoutingLogicalAccessV1 {
        lane: 0,
        buffer: MoeRoutingMemoryBufferV1::Inverse,
        element_index: 15,
        address_space: MOE_ROUTING_MEMORY_GLOBAL_ADDRESS_SPACE_V1,
        byte_width: MOE_ROUTING_MEMORY_ACCESS_WIDTH_V1,
        phase: MoeRoutingMemoryPhaseV1::OutputCommit,
        kind: MoeRoutingMemoryEffectKindV1::Write,
    }
}

#[test]
fn exact_fixed_domain_is_bounded_disjoint_ordered_and_single_writer() {
    let checked =
        check_moe_routing_memory_contract_v1(MoeRoutingMemoryIdentitiesV1::exact(), regions())
            .unwrap();
    assert!(checked.exhaustively_checks_fixed_source_index_bounds());
    assert!(checked.exhaustively_checks_fixed_source_output_disjointness());
    assert!(checked.exhaustively_checks_fixed_source_write_ownership());
    assert!(!checked.has_identity_bound_verus_receipt());
    assert!(!checked.proves_compiler_refinement());
    assert!(!checked.proves_kernel_ir_refinement());
    assert!(!checked.proves_llvm_refinement());
    assert!(!checked.proves_isa_refinement());
    assert!(!checked.proves_logical_to_machine_address_refinement());
    assert!(!checked.proves_machine_memory_safety());
    assert!(!checked.proves_generalized_race_freedom());
    assert!(!checked.grants_artifact_authority());
    assert!(!checked.proves_gpu_execution());
}

#[test]
fn every_identity_field_is_fail_closed() {
    let exact = MoeRoutingMemoryIdentitiesV1::exact();
    let mutations = [
        MoeRoutingMemoryIdentitiesV1 {
            source: [0; 32],
            ..exact
        },
        MoeRoutingMemoryIdentitiesV1 {
            profile: [0; 32],
            ..exact
        },
        MoeRoutingMemoryIdentitiesV1 {
            kernel_ir: [0; 32],
            ..exact
        },
        MoeRoutingMemoryIdentitiesV1 {
            descriptor: [0; 32],
            ..exact
        },
        MoeRoutingMemoryIdentitiesV1 {
            launch: [0; 32],
            ..exact
        },
        MoeRoutingMemoryIdentitiesV1 {
            effects: [0; 32],
            ..exact
        },
        MoeRoutingMemoryIdentitiesV1 {
            routing: [0; 32],
            ..exact
        },
    ];
    for mutation in mutations {
        assert_eq!(
            check_moe_routing_memory_contract_v1(mutation, regions()),
            Err(MoeRoutingMemoryContractErrorV1::Identity)
        );
    }
}

#[test]
fn exact_eight_buffer_extents_are_required() {
    let mut cases = Vec::new();
    let mut value = regions();
    value.logits.bytes = 124;
    cases.push(value);
    let mut value = regions();
    value.top2_experts.bytes = 60;
    cases.push(value);
    let mut value = regions();
    value.requested_counts.bytes = 20;
    cases.push(value);
    let mut value = regions();
    value.admitted_counts.bytes = 12;
    cases.push(value);
    let mut value = regions();
    value.expert_offsets.bytes = 16;
    cases.push(value);
    let mut value = regions();
    value.route_slots.bytes = 68;
    cases.push(value);
    let mut value = regions();
    value.permutation.bytes = 60;
    cases.push(value);
    let mut value = regions();
    value.inverse.bytes = 68;
    cases.push(value);
    for case in cases {
        assert_eq!(
            check_moe_routing_memory_contract_v1(MoeRoutingMemoryIdentitiesV1::exact(), case),
            Err(MoeRoutingMemoryContractErrorV1::Extent)
        );
    }
}

#[test]
fn every_region_pair_must_be_disjoint() {
    for left in 0..8 {
        for right in left + 1..8 {
            let mut values = regions().into_test_array();
            values[right].base = values[left].base;
            assert_eq!(
                check_moe_routing_memory_contract_v1(
                    MoeRoutingMemoryIdentitiesV1::exact(),
                    MoeRoutingMemoryRegionsV1::from_test_array(values),
                ),
                Err(MoeRoutingMemoryContractErrorV1::RegionAlias),
                "pair {left}/{right} escaped"
            );
        }
    }
}

#[test]
fn alignment_and_address_overflow_fail_closed() {
    let mut value = regions();
    value.permutation.base += 1;
    assert_eq!(
        check_moe_routing_memory_contract_v1(MoeRoutingMemoryIdentitiesV1::exact(), value),
        Err(MoeRoutingMemoryContractErrorV1::Alignment)
    );
    let mut value = regions();
    value.logits.base = u64::MAX - 63;
    assert_eq!(
        check_moe_routing_memory_contract_v1(MoeRoutingMemoryIdentitiesV1::exact(), value),
        Err(MoeRoutingMemoryContractErrorV1::AddressOverflow)
    );
}

#[test]
fn only_lane_zero_can_commit_each_output_once() {
    assert_eq!(
        validate_moe_routing_logical_access_v1(output_write()),
        Ok(())
    );
    let mut mutation = output_write();
    mutation.lane = 1;
    assert_eq!(
        validate_moe_routing_logical_access_v1(mutation),
        Err(MoeRoutingMemoryContractErrorV1::OutputOwnership)
    );
    let mut mutation = output_write();
    mutation.buffer = MoeRoutingMemoryBufferV1::Logits;
    assert_eq!(
        validate_moe_routing_logical_access_v1(mutation),
        Err(MoeRoutingMemoryContractErrorV1::EffectKind)
    );
}

#[test]
fn logical_access_shape_space_width_and_phase_drift_are_rejected() {
    let mutations: [(MoeRoutingLogicalAccessV1, MoeRoutingMemoryContractErrorV1); 5] = [
        (
            {
                let mut value = output_write();
                value.element_index = 16;
                value
            },
            MoeRoutingMemoryContractErrorV1::Extent,
        ),
        (
            {
                let mut value = output_write();
                value.address_space = 3;
                value
            },
            MoeRoutingMemoryContractErrorV1::AddressSpace,
        ),
        (
            {
                let mut value = output_write();
                value.byte_width = 8;
                value
            },
            MoeRoutingMemoryContractErrorV1::AccessWidth,
        ),
        (
            {
                let mut value = output_write();
                value.phase = MoeRoutingMemoryPhaseV1::SlotAssignment;
                value
            },
            MoeRoutingMemoryContractErrorV1::EffectOrdering,
        ),
        (
            {
                let mut value = output_write();
                value.kind = MoeRoutingMemoryEffectKindV1::Read;
                value
            },
            MoeRoutingMemoryContractErrorV1::EffectKind,
        ),
    ];
    for (mutation, error) in mutations {
        assert_eq!(validate_moe_routing_logical_access_v1(mutation), Err(error));
    }
}

#[test]
fn exact_phase_chain_is_stable_and_reordering_is_rejected() {
    let phases = [
        MoeRoutingMemoryPhaseV1::InputValidation,
        MoeRoutingMemoryPhaseV1::Top2Selection,
        MoeRoutingMemoryPhaseV1::RequestedCount,
        MoeRoutingMemoryPhaseV1::CapacityClamp,
        MoeRoutingMemoryPhaseV1::ExclusiveScan,
        MoeRoutingMemoryPhaseV1::SentinelInitialization,
        MoeRoutingMemoryPhaseV1::StableRank,
        MoeRoutingMemoryPhaseV1::SlotAssignment,
        MoeRoutingMemoryPhaseV1::PermutationInverse,
        MoeRoutingMemoryPhaseV1::OutputCommit,
    ];
    for pair in phases.windows(2) {
        assert_eq!(
            validate_moe_routing_phase_transition_v1(pair[0], pair[1]),
            Ok(())
        );
        assert_eq!(
            validate_moe_routing_phase_transition_v1(pair[1], pair[0]),
            Err(MoeRoutingMemoryContractErrorV1::EffectOrdering)
        );
    }
}

#[test]
fn expert_route_slot_permutation_and_inverse_values_are_bounded() {
    for (kind, bad, error) in [
        (
            MoeRoutingLogicalIndexKindV1::Expert,
            4,
            MoeRoutingMemoryContractErrorV1::InvalidExpert,
        ),
        (
            MoeRoutingLogicalIndexKindV1::Route,
            16,
            MoeRoutingMemoryContractErrorV1::InvalidRoute,
        ),
        (
            MoeRoutingLogicalIndexKindV1::RouteSlot,
            16,
            MoeRoutingMemoryContractErrorV1::InvalidRouteValue,
        ),
        (
            MoeRoutingLogicalIndexKindV1::PermutationValue,
            16,
            MoeRoutingMemoryContractErrorV1::InvalidRouteValue,
        ),
        (
            MoeRoutingLogicalIndexKindV1::InverseValue,
            16,
            MoeRoutingMemoryContractErrorV1::InvalidRouteValue,
        ),
    ] {
        assert_eq!(
            validate_moe_routing_logical_index_v1(MoeRoutingLogicalIndexV1 { kind, value: bad }),
            Err(error)
        );
    }
    for kind in [
        MoeRoutingLogicalIndexKindV1::RouteSlot,
        MoeRoutingLogicalIndexKindV1::PermutationValue,
        MoeRoutingLogicalIndexKindV1::InverseValue,
    ] {
        assert_eq!(
            validate_moe_routing_logical_index_v1(MoeRoutingLogicalIndexV1 {
                kind,
                value: MOE_ROUTING_MEMORY_DROP_ROUTE_V1,
            }),
            Ok(())
        );
    }
}

#[test]
fn expected_verus_evidence_is_explicitly_inert_and_copyable() {
    let expected = MoeRoutingMemoryVerusExpectedEvidenceV1::exact();
    let copied = expected;
    assert_eq!(copied, expected);
    assert!(!expected.authenticates_anything());
    assert_ne!(expected.published_machine_body, [0; 32]);
    assert_ne!(expected.analyzer_profile, [0; 32]);
    assert_ne!(expected.verus_executable, [0; 32]);
    assert_ne!(expected.verus_closure_manifest, [0; 32]);
    assert_ne!(expected.transcript, [0; 32]);
}

trait TestRegions {
    fn into_test_array(self) -> [MoeRoutingMemoryRegionV1; 8];
    fn from_test_array(values: [MoeRoutingMemoryRegionV1; 8]) -> Self;
}

impl TestRegions for MoeRoutingMemoryRegionsV1 {
    fn into_test_array(self) -> [MoeRoutingMemoryRegionV1; 8] {
        [
            self.logits,
            self.top2_experts,
            self.requested_counts,
            self.admitted_counts,
            self.expert_offsets,
            self.route_slots,
            self.permutation,
            self.inverse,
        ]
    }

    fn from_test_array(values: [MoeRoutingMemoryRegionV1; 8]) -> Self {
        Self {
            logits: values[0],
            top2_experts: values[1],
            requested_counts: values[2],
            admitted_counts: values[3],
            expert_offsets: values[4],
            route_slots: values[5],
            permutation: values[6],
            inverse: values[7],
        }
    }
}
