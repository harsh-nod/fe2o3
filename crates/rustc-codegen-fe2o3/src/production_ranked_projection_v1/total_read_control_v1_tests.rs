fn total_read_cfg_fixture(
    live_count: usize,
    failure: GuardedAccessFailureV1,
) -> (Vec<ProductionRankedBlockV1>, Vec<ProjectedAccessSourceV1>) {
    let (function, body) = match live_count {
        0 => (
            projection_function(vec![block(131, vec![], SemanticTerminatorKindV1::Return)]),
            0,
        ),
        1 => (
            multi_block_induction_function(
                InductionCfgShape::Chain,
                SemanticLocalRoleV1::Argument(0),
                1,
            ),
            2,
        ),
        2 => (nested_optional_uniform_induction_function(true), 4),
        _ => panic!("only zero, one, or two induction fixtures"),
    };
    let (inductions, switches, operations) = if live_count == 2 {
        let (inductions, switches, operations, _) =
            project_optional_uniform_induction_for_test(&function).unwrap();
        (inductions, switches, operations)
    } else {
        let (inductions, operations, _) = project_test_inductions(&function).unwrap();
        (inductions, vec![None; function.blocks().len()], operations)
    };
    let mut projected = (0..function.blocks().len())
        .map(|_| ProjectedSemanticBlockV1 { items: vec![] })
        .collect::<Vec<_>>();
    projected[body]
        .items
        .push(ProjectedBlockItemV1::Guarded(GuardedRankedAccessV1 {
            view: ProductionRankedValueIdV1::new(0),
            indices: vec![
                ProductionRankedValueV1::Argument(8),
                ProductionRankedValueV1::Argument(9),
            ],
            checked_success: None,
            atomic: None,
            comparisons: vec![
                (
                    ProductionRankedValueV1::Argument(8),
                    ProductionRankedValueV1::Argument(10),
                ),
                (
                    ProductionRankedValueV1::Argument(9),
                    ProductionRankedValueV1::Argument(11),
                ),
            ],
            failure,
            access: AccessKindAttr::Read,
            memory_space: MemorySpaceAttr::Global,
            source: SemanticSourceProvenanceV1::unavailable(),
            semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                block: body,
                statement: None,
            }),
        }));
    let (blocks, sources, _) = build_ranked_cfg(
        &assertion_proof_types(),
        &function,
        &[],
        &vec![None; function.locals().len()],
        &switches,
        &inductions,
        operations,
        projected,
    )
    .unwrap();
    (blocks, sources)
}

fn total_read_expected_arguments(block: usize, count: usize) -> Vec<ProductionRankedValueV1> {
    (0..count)
        .map(|argument| ProductionRankedValueV1::BlockArgument {
            block: u32::try_from(block).unwrap(),
            argument: u32::try_from(argument).unwrap(),
        })
        .collect()
}

fn total_read_continuation(block: &ProductionRankedBlockV1) -> u32 {
    match block.terminator() {
        ProductionRankedTerminatorV1::Branch { target }
        | ProductionRankedTerminatorV1::BranchArgs { target, .. } => *target,
        other => panic!("expected one continuation, found {other:?}"),
    }
}

#[test]
fn total_read_failures_continue_without_effects_and_preserve_all_live_arguments() {
    for live_count in 0..=2 {
        let (blocks, sources) =
            total_read_cfg_fixture(live_count, GuardedAccessFailureV1::ContinueWithoutAccess);
        assert_eq!(sources.len(), 1);
        let source = sources[0];
        let success = &blocks[source.block];
        assert_eq!(success.index_argument_count() as usize, live_count);
        assert_eq!(source.operation, 0);
        assert_eq!(source.access, AccessKindAttr::Read);
        assert_eq!(source.memory_space, MemorySpaceAttr::Global);
        assert_eq!(source.semantic_site.unwrap().block, [0, 2, 4][live_count]);
        assert!(
            matches!(success.operations(), [ProductionRankedOperationV1::Access { kind: AccessKindAttr::Read, indices, .. }]
            if indices == &[ProductionRankedValueV1::Argument(8), ProductionRankedValueV1::Argument(9)])
        );
        let continuation = total_read_continuation(success);
        let mut predicates = 0;
        for (index, block) in blocks.iter().enumerate() {
            let (lhs, failure, true_args, false_args) = match block.terminator() {
                ProductionRankedTerminatorV1::IndexLessThan {
                    lhs, false_block, ..
                } => (*lhs, *false_block, &[][..], &[][..]),
                ProductionRankedTerminatorV1::IndexLessThanArgs {
                    lhs,
                    false_block,
                    true_arguments,
                    false_arguments,
                    ..
                } => (
                    *lhs,
                    *false_block,
                    true_arguments.as_slice(),
                    false_arguments.as_slice(),
                ),
                _ => continue,
            };
            if !matches!(lhs, ProductionRankedValueV1::Argument(8 | 9)) {
                continue;
            }
            predicates += 1;
            let expected = total_read_expected_arguments(index, live_count);
            assert_eq!(true_args, expected);
            assert_eq!(false_args, expected);
            let failure_index = failure as usize;
            let failure = &blocks[failure_index];
            assert_eq!(failure.index_argument_count() as usize, live_count);
            assert!(failure.operations().is_empty());
            assert_eq!(total_read_continuation(failure), continuation);
            if let ProductionRankedTerminatorV1::BranchArgs { arguments, .. } = failure.terminator()
            {
                assert_eq!(
                    arguments,
                    &total_read_expected_arguments(failure_index, live_count)
                );
            } else {
                assert_eq!(live_count, 0);
            }
        }
        assert_eq!(predicates, 2);
        assert!(
            !blocks
                .iter()
                .any(|block| matches!(block.terminator(), ProductionRankedTerminatorV1::Trap))
        );
    }
}

#[test]
fn strict_guard_failures_still_trap_and_never_receive_loop_arguments() {
    for live_count in 0..=2 {
        let (blocks, sources) = total_read_cfg_fixture(live_count, GuardedAccessFailureV1::Trap);
        assert_eq!(sources.len(), 1);
        let mut predicates = 0;
        for (index, block) in blocks.iter().enumerate() {
            let (lhs, failure, true_args, false_args) = match block.terminator() {
                ProductionRankedTerminatorV1::IndexLessThan {
                    lhs, false_block, ..
                } => (*lhs, *false_block, &[][..], &[][..]),
                ProductionRankedTerminatorV1::IndexLessThanArgs {
                    lhs,
                    false_block,
                    true_arguments,
                    false_arguments,
                    ..
                } => (
                    *lhs,
                    *false_block,
                    true_arguments.as_slice(),
                    false_arguments.as_slice(),
                ),
                _ => continue,
            };
            if !matches!(lhs, ProductionRankedValueV1::Argument(8 | 9)) {
                continue;
            }
            predicates += 1;
            assert_eq!(true_args, total_read_expected_arguments(index, live_count));
            assert!(false_args.is_empty());
            let failure = &blocks[failure as usize];
            assert_eq!(failure.index_argument_count(), 0);
            assert!(failure.operations().is_empty());
            assert!(matches!(
                failure.terminator(),
                ProductionRankedTerminatorV1::Trap
            ));
        }
        assert_eq!(predicates, 2);
    }
}

#[test]
fn authenticated_strided_read_producer_selects_total_failure_only_for_readonly_view() {
    let function = projection_function(vec![block(131, vec![], SemanticTerminatorKindV1::Return)]);
    for writable in [false, true] {
        let mut view = strided_read_view();
        view.allocation.writable = writable;
        let effect = ProjectedReadViewAccessV1 {
            view,
            row: ProjectedReadValueV1::Constant(0),
            column: ProjectedReadValueV1::Constant(4),
        };
        let mut arguments = vec![None; function.locals().len()];
        let result = project_strided_read_effects_v1(
            &projection_types(),
            &function,
            &[Some(effect)],
            &vec![None; function.locals().len()],
            &mut arguments,
            &mut 1,
            &mut Vec::new(),
            &mut 0,
        );
        if writable {
            assert!(result.is_err());
        } else {
            let result = result.unwrap();
            let access = result[0].as_ref().unwrap();
            assert_eq!(
                access.failure,
                GuardedAccessFailureV1::ContinueWithoutAccess
            );
            assert_eq!(access.access, AccessKindAttr::Read);
            assert_eq!(access.comparisons.len(), 2);
            assert!(access.checked_success.is_none());
        }
    }
}

#[test]
fn total_read_authority_requires_registered_intrinsic_and_exact_view_element() {
    let function = strided_read_call_function(None);
    for mutation in 0..3 {
        let mut view = strided_read_view();
        if mutation == 1 {
            view.element = POINTER_TYPE;
        }
        let value = if mutation == 2 {
            ProjectedCapabilityValueV1::Invalid
        } else {
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::ReadView(view))
        };
        let mut state = HashMap::from([(0, value)]);
        let callables = if mutation == 0 {
            vec![]
        } else {
            vec![strided_read_callable()]
        };
        let result = transfer_capability_terminator_v1(
            &callables,
            &function,
            0,
            &mut state,
            &[None; 5],
            &[None; 5],
            &[],
            &HashMap::new(),
            true,
        );
        assert!(
            result.is_err() || result.unwrap().read_view.is_none(),
            "mutation {mutation}"
        );
    }
}
