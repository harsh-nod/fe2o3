//! Exercises the real replay recorder, not a fabricated allocation summary.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

fn assignment() -> SemanticAssignmentV1 {
    let word = SemanticTypeIdV1::from_index(0);
    let boolean = SemanticTypeIdV1::from_index(1);
    SemanticAssignmentV1::new(
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], boolean).unwrap(),
        SemanticRvalueV1::new(
            boolean,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::NotEqual,
                left: SemanticOperandV1::Copy(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], word).unwrap(),
                ),
                right: SemanticOperandV1::Constant(SemanticConstantV1::new(
                    word,
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1024, 8).unwrap()),
                )),
            },
        ),
    )
}
fn extent() -> AllocationContractV1 {
    AllocationContractV1 {
        allocation_origin: 1,
        noalias_class: 7,
        writable: false,
        singleton_object: false,
    }
}
fn state() -> ProjectedCapabilityStateV1 {
    HashMap::from([(
        1,
        ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalExtent(extent())),
    )])
}

#[test]
fn source_comparison_replay_retains_exact_assignment_and_allocation() {
    let assignment = assignment();
    let mut uses = ProjectedGlobalSemanticUsesV1::default();
    uses.record_source_unsigned_comparison_v1(&assignment, &state(), (3, 2))
        .unwrap();
    let record = uses.source_unsigned_comparisons.get(&(3, 2)).unwrap();
    assert!(std::ptr::eq(record.assignment, &assignment));
    assert_eq!(record.left, Some(extent()));
    assert_eq!(record.right, None);
    assert_eq!(uses.work, 2);
}

#[test]
fn source_comparison_replay_requires_agreeing_predecessor_metadata() {
    for mutation in 0..4 {
        let mut known = state();
        let other = match mutation {
            0 => state(),
            1 => HashMap::new(),
            2 => HashMap::from([(1, ProjectedCapabilityValueV1::Invalid)]),
            3 => HashMap::from([(
                1,
                ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalExtent(
                    AllocationContractV1 {
                        allocation_origin: 2,
                        ..extent()
                    },
                )),
            )]),
            _ => unreachable!(),
        };
        merge_capability_states_v1(&mut known, &other).unwrap();
        let mut uses = ProjectedGlobalSemanticUsesV1::default();
        uses.record_source_unsigned_comparison_v1(&assignment(), &known, (0, 0))
            .unwrap();
        assert_eq!(
            !uses.source_unsigned_comparisons.is_empty(),
            mutation == 0,
            "mutation {mutation}"
        );
    }
}

#[test]
fn source_comparison_replay_unknown_or_loaded_data_never_becomes_length() {
    for value in [
        ProjectedCapabilityValueV1::Invalid,
        ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalLoadedScalar {
            block: 0,
        }),
    ] {
        let mut uses = ProjectedGlobalSemanticUsesV1::default();
        uses.record_source_unsigned_comparison_v1(
            &assignment(),
            &HashMap::from([(1, value)]),
            (0, 0),
        )
        .unwrap();
        assert!(uses.source_unsigned_comparisons.is_empty());
    }
}

#[test]
fn source_comparison_replay_shares_capability_work_limit() {
    let mut uses = ProjectedGlobalSemanticUsesV1::default();
    uses.work = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1;
    assert!(matches!(
        uses.record_source_unsigned_comparison_v1(&assignment(), &state(), (0, 0)),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "capability dataflow exceeds the charged projection limit"
        ))
    ));
    assert!(uses.source_unsigned_comparisons.is_empty());
}
