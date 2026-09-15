use super::*;
use dialect_kernel::{AccessKindAttr, DYNAMIC_EXTENT, MemorySpaceAttr};
use fe2o3_pliron::{ProductionRankedBlockV1, ProductionRankedKernelV1, ProductionRankedTerminatorV1};

#[test]
fn optional_source_precision_late_shared_exhaustion_rolls_back_published_state() {
    let mut f = equality_fixture();
    // An extra scanned block ensures the final work charge follows the entire
    // first comparison, including its operation and fresh extent publication.
    f.blocks
        .push(block(6, vec![], SemanticTerminatorKindV1::Return));
    let positive = probe(&f, true, Settings::default());
    assert!(positive.result.unwrap()[5].is_some());
    let spent = positive.work_after - positive.work_before;
    assert!(spent > 1);
    let start = MAX_PROJECTED_LOOP_GRAPH_WORK_V1
        .checked_sub(spent - 1)
        .unwrap();
    let observed = probe(
        &f,
        true,
        Settings {
            shared_work: Some(start),
            observe_uncommitted: true,
            ..Settings::default()
        },
    );
    assert!(matches!(
        observed.result,
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit"
        ))
    ));
    assert_eq!(observed.before.extents[0], None);
    assert_eq!(observed.after.extents[0], Some(9));
    assert!(observed.after.operations.len() > observed.before.operations.len());
    assert!(observed.after.next_value > observed.before.next_value);
    assert_eq!(
        observed.after.next_argument,
        observed.before.next_argument + 1
    );
    assert_eq!(observed.after.arguments, observed.before.arguments);
    assert_eq!(observed.work_after, MAX_PROJECTED_LOOP_GRAPH_WORK_V1 + 1);

    let rolled_back = probe(
        &f,
        true,
        Settings {
            shared_work: Some(start),
            ..Settings::default()
        },
    );
    assert!(matches!(
        rolled_back.result,
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit"
        ))
    ));
    assert_eq!(rolled_back.before, observed.before);
    assert_eq!(rolled_back.after, rolled_back.before);
    assert_eq!(rolled_back.work_before, observed.work_before);
    assert_eq!(
        rolled_back.work_after, observed.work_after,
        "rollback refunds no work"
    );
    assert!(rolled_back.work_after > rolled_back.work_before);
}

#[test]
fn optional_source_precision_unresolved_required_guard_is_rejected_downstream() {
    use crate::production_reference_effect_join_v2::{
        ProductionReferenceEffectJoinErrorV2,
        optional_precision_consumer_tests::required_output_guard,
    };
    use crate::reference_effect_v1::{
        ReferenceBinaryOpV1, ReferenceConstantV1, ReferenceEffectExpressionV1,
        ReferencePathPredicateV1, ReferenceScalarTypeV1, reference_boolean_guard_atom_v1,
        reference_predicate_and_atom_v1,
    };
    for limited in [false, true] {
        let f = equality_fixture();
        let p = probe(
            &f,
            true,
            Settings {
                node_work: limited.then_some(MAX_PURE_UNIFORM_INDEX_NODES_V1),
                prebound_extent: true,
                ..Settings::default()
            },
        );
        let switches = p.result.unwrap();
        if limited {
            assert!(switches.is_empty());
            assert_eq!(p.after, p.before);
        } else {
            assert!(switches[5].is_some());
        }
        let terminal = projected_cfg_terminator(
            &p.function,
            5,
            &f.callables,
            false,
            &vec![None; f.locals.len()],
            &p.direct,
            &switches,
        )
        .unwrap();
        let destination = |source| match source {
            4 => 1,
            3 => 2,
            _ => panic!("changed original comparison edge"),
        };
        let terminal = match terminal {
            ProjectedCfgTerminatorV1::ExactSwitch(switch) if !limited => {
                assert_eq!(
                    switch.normalized_comparison,
                    Some(SemanticBinaryOpV1::Equal)
                );
                let [(1, rhs, yes)] = switch.targets.as_slice() else {
                    panic!()
                };
                ProductionRankedTerminatorV1::IndexEqual {
                    lhs: switch.discriminant,
                    rhs: *rhs,
                    true_block: destination(*yes),
                    false_block: destination(switch.otherwise),
                }
            }
            ProjectedCfgTerminatorV1::AnalysisSplit {
                first_block,
                second_block,
            } if limited => {
                assert_eq!((first_block, second_block), (3, 4));
                ProductionRankedTerminatorV1::AnalysisSplit {
                    control_dependencies: vec![],
                    first_block: destination(first_block),
                    second_block: destination(second_block),
                }
            }
            other => panic!("unexpected required guard projection: {other:?}"),
        };
        let point = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0));
        let view_id = ProductionRankedValueIdV1::new(p.after.next_value);
        let view = ProductionRankedValueV1::Local(view_id);
        let mut operations = p.after.operations;
        operations.push(ProductionRankedOperationV1::ViewInSpace {
            result: view_id,
            element_width: 32,
            writable: true,
            shape: vec![DYNAMIC_EXTENT],
            dynamic_extents: vec![ProductionRankedValueV1::Argument(
                p.after.extents[0].unwrap(),
            )],
            memory_space: MemorySpaceAttr::Global,
            allocation_origin: 1,
            noalias_class: 1,
        });
        let kernel = ProductionRankedKernelV1::new(
            "required_output_guard",
            p.after.next_argument,
            vec![
                ProductionRankedBlockV1::new(operations, terminal),
                ProductionRankedBlockV1::new(
                    vec![ProductionRankedOperationV1::Access {
                        kind: AccessKindAttr::Write,
                        view,
                        indices: vec![point],
                    }],
                    ProductionRankedTerminatorV1::Return,
                ),
                ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Return),
            ],
        )
        .unwrap();
        let retained = kernel.blocks().to_vec();
        let guard = required_output_guard(&kernel, view, point);
        if limited {
            assert!(
                matches!(
                    guard,
                    Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect {
                        block: 1,
                        operation: 0,
                        detail: "GPU guard contains a nonrepresentable analysis split",
                    })
                ),
                "{guard:?}"
            );
        } else {
            let expected = reference_predicate_and_atom_v1(
                &ReferencePathPredicateV1::unconditional_v1(),
                reference_boolean_guard_atom_v1(
                    ReferenceEffectExpressionV1::Binary {
                        operation: ReferenceBinaryOpV1::Equal,
                        lhs: Box::new(ReferenceEffectExpressionV1::InputLength {
                            reference_argument: 1,
                        }),
                        rhs: Box::new(ReferenceEffectExpressionV1::Constant(
                            ReferenceConstantV1::Scalar {
                                scalar: ReferenceScalarTypeV1::Usize,
                                bits: 1024,
                            },
                        )),
                        checked: false,
                    },
                    true,
                ),
            )
            .unwrap();
            assert_eq!(guard.unwrap(), expected);
        }
        assert_eq!(kernel.blocks(), retained);
    }
}
