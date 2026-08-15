use fe2o3_kernel_ir::*;

type ProfileMutation = Box<dyn Fn(&mut MoeTop2ProfileV1)>;
type KernelIrMutation = Box<dyn Fn(&mut MoeTop2KernelIrV1)>;

fn exact() -> (MoeTop2KernelIrV1, MoeTop2ProfileV1) {
    (
        moe_top2_v1_kernel_ir(),
        MoeTop2ProfileV1::exact_gfx942_xnack_minus_cov6(),
    )
}

#[test]
fn exact_profile_and_closed_sidecar_verify() {
    let (ir, profile) = exact();
    verify_moe_top2_v1(&ir, &profile).unwrap();
    assert_eq!(
        ir.shape,
        MoeTop2ShapeV1 {
            tokens: 8,
            experts: 4,
            experts_per_token: 2,
            expert_capacity: 4,
            logits: 32,
            routes: 16,
        }
    );
    assert_eq!(ir.routing.len(), 10);
    assert_eq!(profile.descriptor.explicit_kernarg_bytes, 128);
    assert_eq!(profile.descriptor.complete_kernarg_bytes, 384);
    assert_eq!(profile.descriptor.resources.static_lds_bytes, 0);
    assert_eq!(profile.descriptor.resources.required_dynamic_lds_bytes, 0);
}

#[test]
fn profile_mutations_fail_closed() {
    let (ir, profile) = exact();
    let mutations: Vec<ProfileMutation> = vec![
        Box::new(|p| p.source_sha256[0] ^= 1),
        Box::new(|p| p.namespace[0] ^= 1),
        Box::new(|p| p.target = TargetCapability::Subgroups),
        Box::new(|p| p.code_object_version = 5),
        Box::new(|p| p.wave_width = WaveWidth::Wave32),
        Box::new(|p| p.workgroup_size = WorkgroupSize::new(32, 1, 1)),
        Box::new(|p| p.grid = [2, 1, 1]),
        Box::new(|p| p.descriptor.explicit_kernarg_bytes = 120),
        Box::new(|p| p.descriptor.complete_kernarg_bytes = 376),
        Box::new(|p| p.descriptor.resources.static_lds_bytes = 4),
        Box::new(|p| p.descriptor.resources.maximum_dynamic_lds_bytes = 4),
        Box::new(|p| p.descriptor.export_name.push_str("_mutated")),
    ];
    for mutate in mutations {
        let mut candidate = profile.clone();
        mutate(&mut candidate);
        assert_eq!(
            verify_moe_top2_v1(&ir, &candidate),
            Err(MoeTop2V1Error::UnsupportedProfile)
        );
    }
}

#[test]
fn semantic_mutations_fail_closed() {
    let (ir, profile) = exact();
    let mutations: Vec<KernelIrMutation> = vec![
        Box::new(|m| m.module_id.push_str("_mutated")),
        Box::new(|m| m.arguments.swap(0, 1)),
        Box::new(|m| m.arguments[7].offset = 104),
        Box::new(|m| m.shape.tokens = 7),
        Box::new(|m| m.shape.experts = 3),
        Box::new(|m| m.shape.experts_per_token = 1),
        Box::new(|m| m.shape.expert_capacity = 3),
        Box::new(|m| m.layout = MoeTop2LayoutV1::ExpertMajorLogitsUnsupported),
        Box::new(|m| m.finite_input = MoeTop2FiniteInputPolicyV1::NonFiniteInputsUnsupported),
        Box::new(|m| m.tie_break = MoeTop2TieBreakV1::HigherExpertIdTieBreakUnsupported),
        Box::new(|m| m.overflow = MoeTop2OverflowV1::ReplaceAcceptedRouteUnsupported),
        Box::new(|m| m.routing.swap(1, 2)),
        Box::new(|m| m.routing.swap(3, 4)),
        Box::new(|m| m.routing.swap(7, 8)),
        Box::new(|m| m.packing.requested_counts_exact = false),
        Box::new(|m| m.packing.admitted_is_requested_min_capacity = false),
        Box::new(|m| m.packing.offsets_are_exclusive_scan = false),
        Box::new(|m| m.packing.accepted_slots_unique = false),
        Box::new(|m| m.packing.accepted_slots_bounded_by_total_admitted = false),
        Box::new(|m| m.packing.permutation_inverse_round_trip = false),
        Box::new(|m| m.packing.dropped_slot_and_inverse_are_sentinel = false),
        Box::new(|m| m.packing.unused_permutation_tail_is_sentinel = false),
        Box::new(|m| m.packing.sentinel = 0),
        Box::new(|m| m.ownership.active_lanes = 2),
        Box::new(|m| m.ownership.output_lengths[3] = 4),
        Box::new(|m| m.ownership.every_output_element_written_once = false),
        Box::new(|m| m.ownership.output_arguments_exclusive = false),
        Box::new(|m| m.ownership.writes_in_bounds = false),
    ];
    for mutate in mutations {
        let mut candidate = ir.clone();
        mutate(&mut candidate);
        assert_eq!(
            verify_moe_top2_v1(&candidate, &profile),
            Err(MoeTop2V1Error::NonCanonicalKernelIr)
        );
    }
}

#[test]
fn exact_abi_routing_and_ownership_are_explicit() {
    let (ir, _) = exact();
    assert_eq!(
        ir.arguments.map(|argument| argument.role),
        [
            MoeTop2ArgumentRoleV1::Logits,
            MoeTop2ArgumentRoleV1::Top2Experts,
            MoeTop2ArgumentRoleV1::RequestedCounts,
            MoeTop2ArgumentRoleV1::AdmittedCounts,
            MoeTop2ArgumentRoleV1::ExpertOffsets,
            MoeTop2ArgumentRoleV1::RouteSlots,
            MoeTop2ArgumentRoleV1::Permutation,
            MoeTop2ArgumentRoleV1::Inverse,
        ]
    );
    assert_eq!(ir.ownership.output_lengths, [16, 4, 4, 5, 16, 16, 16]);
    assert_eq!(ir.packing.sentinel, u32::MAX);
    assert_eq!(
        ir.routing,
        [
            MoeTop2RoutingStepV1::ValidateExactExtentsAndFiniteInputsBeforeWrites,
            MoeTop2RoutingStepV1::SelectDistinctTop2DescendingScoreLowerExpertTie,
            MoeTop2RoutingStepV1::CountRequestedRoutesInTokenThenRankOrder,
            MoeTop2RoutingStepV1::ClampAdmittedCountsToCapacityFour,
            MoeTop2RoutingStepV1::ExclusiveScanAdmittedCountsInExpertOrder,
            MoeTop2RoutingStepV1::InitializeSlotsPermutationAndInverseToSentinel,
            MoeTop2RoutingStepV1::ComputeStableRankInIncreasingRouteOrder,
            MoeTop2RoutingStepV1::AssignUniqueBoundedSlotFromExpertOffsetAndStableRank,
            MoeTop2RoutingStepV1::EstablishPermutationAndInverseRoundTrip,
            MoeTop2RoutingStepV1::CommitEveryOutputOnceFromLaneZero,
        ]
    );
}
