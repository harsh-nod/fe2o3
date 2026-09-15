use super::*;

fn setup() -> Context {
    let mut context = Context::new();
    dialect_kernel::register_dialect(
        &mut context,
        &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    context
}

fn partition(
    context: &mut Context,
    layout: RawLayoutV1,
    declared: u64,
    boundary: u64,
    invocation_is_lhs: bool,
) -> Option<RawControlScopeV1> {
    let invocation = InvocationIndexOp::new(context, 0, declared);
    let boundary = IndexConstantOp::new(context, boundary);
    raw_aligned_invocation_partition(
        context,
        invocation.result(context),
        boundary.result(context),
        invocation_is_lhs,
        Some(layout),
    )
}

#[test]
fn unknown_declarations_use_actual_bounds_and_comparison_direction() {
    let context = &mut setup();
    let layout = RawLayoutV1 {
        global: [128, 1, 1],
        workgroup: [128, 1, 1],
        subgroup: 64,
    };
    for declared in [0, 128] {
        for (boundary, lhs, rank) in [
            (0, true, RAW_GRID_SCOPE_V1),
            (0, false, 0),
            (1, true, 0),
            (63, false, RAW_SUBGROUP_SCOPE_V1),
            (64, true, RAW_SUBGROUP_SCOPE_V1),
            (64, false, 0),
            (128, true, RAW_GRID_SCOPE_V1),
            (u64::MAX, false, RAW_GRID_SCOPE_V1),
        ] {
            assert_eq!(
                partition(context, layout, declared, boundary, lhs),
                Some(RawControlScopeV1 { rank, known: true }),
                "{declared} {boundary} {lhs}"
            );
        }
    }
    assert_eq!(partition(context, layout, 64, 1, true), None);
    assert_eq!(
        partition(
            context,
            RawLayoutV1 {
                global: [0, 1, 1],
                ..layout
            },
            0,
            1,
            true
        ),
        None
    );
    let fourth = InvocationIndexOp::new(context, 3, 0);
    let zero = IndexConstantOp::new(context, 0);
    assert_eq!(
        raw_aligned_invocation_partition(
            context,
            fourth.result(context),
            zero.result(context),
            true,
            Some(layout)
        ),
        None
    );
}

#[test]
fn claimed_uniform_groups_have_identical_predicate_truth_values() {
    let context = &mut setup();
    for layout in [
        RawLayoutV1 {
            global: [128, 1, 1],
            workgroup: [128, 1, 1],
            subgroup: 64,
        },
        RawLayoutV1 {
            global: [192, 1, 1],
            workgroup: [96, 1, 1],
            subgroup: 64,
        },
        RawLayoutV1 {
            global: [192, 2, 1],
            workgroup: [96, 2, 1],
            subgroup: 64,
        },
    ] {
        for boundary in 0..=layout.global[0] {
            for lhs in [false, true] {
                let scope = partition(context, layout, 0, boundary, lhs).unwrap();
                if scope.rank == 0 {
                    continue;
                }
                let mut decisions = HashMap::new();
                for y in 0..layout.global[1] {
                    for x in 0..layout.global[0] {
                        let group = (x / layout.workgroup[0], y / layout.workgroup[1]);
                        let local = x % layout.workgroup[0]
                            + layout.workgroup[0] * (y % layout.workgroup[1]);
                        let key = match scope.rank {
                            RAW_GRID_SCOPE_V1 => (0, 0, 0),
                            RAW_WORKGROUP_SCOPE_V1 => (group.0, group.1, 0),
                            RAW_SUBGROUP_SCOPE_V1 => (group.0, group.1, local / layout.subgroup),
                            _ => unreachable!(),
                        };
                        let decision = if lhs { x < boundary } else { boundary < x };
                        if let Some(previous) = decisions.insert(key, decision) {
                            assert_eq!(
                                previous, decision,
                                "boundary={boundary}, lhs={lhs}, scope={scope:?}, group={key:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn actual_branch_scope_preserves_reversed_comparison_polarity() {
    use pliron::{basic_block::BasicBlock, op::Op};
    let context = &mut setup();
    let layout = RawLayoutV1 {
        global: [128, 1, 1],
        workgroup: [128, 1, 1],
        subgroup: 64,
    };
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let zero = IndexConstantOp::new(context, 0);
    let left = BasicBlock::new(context, None, vec![]);
    let right = BasicBlock::new(context, None, vec![]);
    let values = HashMap::from([
        (invocation.result(context), RawControlScopeV1::LANE),
        (zero.result(context), RawControlScopeV1::GRID),
    ]);
    for reversed in [false, true] {
        let (lhs, rhs) = if reversed {
            (zero.result(context), invocation.result(context))
        } else {
            (invocation.result(context), zero.result(context))
        };
        let branch = IndexLessThanBranchOp::new(context, lhs, rhs, left, right);
        assert_eq!(
            raw_branch_scope(context, branch.get_operation(), &values, Some(layout)),
            if reversed {
                RawControlScopeV1::LANE
            } else {
                RawControlScopeV1::GRID
            }
        );
    }
}

#[test]
fn corrected_uniformity_checker_has_a_new_evidence_identity() {
    let mut old = Sha256::new();
    old.update(b"FE2O3/PRODUCTION-W4/RAW-IR-EVIDENCE/V1\0");
    old.update(MAX_PRODUCTION_W4_RAW_IR_WORK_UNITS_V1.to_le_bytes());
    old.update(MAX_PRODUCTION_W4_RAW_IR_VALUE_DEPTH_V1.to_le_bytes());
    old.update(PRODUCTION_W4_RAW_IR_RECEIPT_COUNT_V1.to_le_bytes());
    let old: [u8; 32] = old.finalize().into();
    assert_ne!(production_w4_raw_ir_checker_identity_v1(), old);
}

#[test]
fn divergence_witness_uses_local_subgroup_coordinates() {
    let layout = RawLayoutV1 {
        global: [288, 1, 1],
        workgroup: [96, 1, 1],
        subgroup: 64,
    };
    for (split, divergent) in [
        (64, false),
        (96, false),
        (128, true),
        (160, false),
        (161, true),
    ] {
        assert_eq!(
            raw_partition_definitely_diverges(
                Some(layout),
                0,
                288,
                split,
                RAW_SUBGROUP_SCOPE_V1,
                false
            ),
            divergent,
            "split {split}"
        );
    }
    assert!(!raw_partition_definitely_diverges(
        Some(RawLayoutV1 {
            global: [288, 0, 1],
            ..layout
        }),
        0,
        288,
        128,
        RAW_SUBGROUP_SCOPE_V1,
        false,
    ));
    assert!(raw_partition_definitely_diverges(
        Some(layout),
        0,
        288,
        96,
        RAW_SUBGROUP_SCOPE_V1,
        true
    ));
}
