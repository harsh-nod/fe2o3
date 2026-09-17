include!("production_borrowed_aggregate_replay_v1_fixtures.rs");

#[test]
fn actual_source_repeated_mutation_shared_borrow_and_caller_reload() {
    let fixture = fixture();
    let sealed = check(&fixture, 10_000_000, 10_000_000).unwrap();
    assert_eq!(sealed.functions().len(), 3);
    for (ordinal, coverage) in sealed.functions().iter().enumerate() {
        assert_eq!(coverage.root, SemanticFunctionIdV1::from_index(0));
        assert_eq!(
            coverage.function,
            SemanticFunctionIdV1::from_index(ordinal as u32)
        );
        assert_eq!(
            coverage.physical,
            fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(ordinal as u32)
        );
        assert_eq!(
            coverage.role,
            if ordinal == 0 {
                SemanticKirFunctionRoleV1::KernelEntry
            } else {
                SemanticKirFunctionRoleV1::InternalHelper
            }
        );
    }
    assert_eq!(
        sealed.retained_storage(),
        std::mem::size_of::<SealedBorrowedAggregateReplayV1>()
            + sealed.functions.capacity()
                * std::mem::size_of::<BorrowedAggregateFunctionCoverageV1>()
    );
    assert_eq!(fixture.calls.len(), 6);
    assert!(matches!(
        fixture.module.functions[0].body.as_ref().unwrap().blocks[3].operations[0].kind,
        OperationKind::Load {
            pointer: ValueId(1),
            ..
        }
    ));
}

#[test]
fn source_initializer_not_producer_initialization_label_controls_value() {
    let mut fixture = fixture();
    fixture.source = source_owner(false, 11);
    fixture.correspondence.semantic_sha256 = *fixture
        .source
        .source_semantic()
        .semantic_sha256()
        .as_bytes();
    assert!(check(&fixture, 10_000_000, 10_000_000).is_err());
}

#[test]
fn physical_entry_substitution_cannot_skip_source_helper_effects() {
    use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1, VerifiedCanonicalKernelIrModuleV12};

    let mut fixture = fixture_with(true, false);
    assert_eq!(
        check(&fixture, 10_000_000, 10_000_000)
            .unwrap()
            .functions()
            .len(),
        3
    );
    let body = fixture.module.functions[1].body.as_mut().unwrap();
    body.blocks.swap(0, 1);
    assert_eq!(body.blocks[0].id, BlockId(1));
    assert!(body.blocks[0].operations.is_empty());
    assert_eq!(
        fixture.source.source_semantic().functions()[1].entry(),
        SemanticBlockIdV1::from_index(0)
    );
    // Native-order locators follow the real reordered blocks. Source-ID-sorted
    // return anchors and source entry remain unchanged.
    assert_eq!(
        fixture.correspondence.terminator_operation_spans[4].kernel_ir_block,
        BlockId(0)
    );
    assert_eq!(
        fixture.correspondence.terminator_operation_spans[5].kernel_ir_block,
        BlockId(1)
    );
    fixture.correspondence.terminator_operation_spans.swap(4, 5);
    fixture.correspondence.blocks.swap(4, 5);

    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 10_000_000);
    let (graph, graph_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &fixture.module,
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(graph_storage.retained_storage())
        .unwrap();
    let (inventory, inventory_storage) =
        CanonicalKirInventoryV1::derive(&graph, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let subject = CanonicalCallSubjectV1 {
        semantic_ssa: &fixture.source,
        executable: &graph,
        correspondence: &fixture.correspondence,
    };
    let floor = budget.storage();
    with_canonical_call_scratch_v1(&mut budget, |budget| {
        let (groups, calls) = build_canonical_call_index_v1(subject, &inventory, budget)?;
        assert_eq!(groups.len(), 3);
        assert_eq!(calls.len(), 3);
        Ok(())
    })
    .expect("entry substitution must reach replay after successful canonical indexing");
    assert_eq!(budget.storage(), floor);
    assert!(matches!(
        check_borrowed_aggregate_replay_v1(
            subject,
            &inventory,
            BorrowedAggregateReplayCandidatesV1 {
                fields: &fixture.fields,
                calls: &fixture.calls,
            },
            &mut budget,
        ),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn source_site_allocations_preserve_persistent_cells_and_reject_substitution() {
    let baseline = fixture_with(false, true);
    let seal = check(&baseline, 10_000_000, 10_000_000).unwrap();
    assert_eq!(seal.functions().len(), 3);
    assert!(seal.retained_storage() > 0);
    assert!(matches!(
        baseline.module.functions[0].body.as_ref().unwrap().blocks[0].operations[0].kind,
        OperationKind::Constant(_)
    ));
    for mutation in 0..7 {
        let mut fixture = fixture_with(false, true);
        match mutation {
            0 => {
                let BorrowedAggregateCarrierCandidateV1::ScalarCell { allocation, .. } =
                    &mut fixture.fields[0].carrier
                else {
                    unreachable!()
                };
                allocation.location = FunctionOperationLocation::new(BlockId(0), 4);
            }
            1 => {
                let BorrowedAggregateCarrierCandidateV1::ScalarCell { allocation, .. } =
                    &mut fixture.fields[0].carrier
                else {
                    unreachable!()
                };
                allocation.function = FunctionId::new("borrowed_replay_mut");
            }
            2 => {
                let BorrowedAggregateCarrierCandidateV1::ScalarCell { pointer, .. } =
                    &mut fixture.fields[0].carrier
                else {
                    unreachable!()
                };
                pointer.value = ValueId(1);
            }
            3 => {
                let OperationKind::Alloca { alignment, .. } =
                    &mut fixture.module.functions[0].body.as_mut().unwrap().blocks[0].operations[2]
                        .kind
                else {
                    unreachable!()
                };
                *alignment = 8;
            }
            4 => {
                let BorrowedAggregateCarrierCandidateV1::ScalarCell { initialization, .. } =
                    &mut fixture.fields[0].carrier
                else {
                    unreachable!()
                };
                initialization.location = FunctionOperationLocation::new(BlockId(0), 5);
            }
            5 => fixture.fields[1].carrier = fixture.fields[0].carrier.clone(),
            _ => {
                let OperationKind::Alloca { count, .. } =
                    &mut fixture.module.functions[0].body.as_mut().unwrap().blocks[0].operations[2]
                        .kind
                else {
                    unreachable!()
                };
                *count = Some(ValueId(2));
            }
        }
        assert!(
            matches!(
                check(&fixture, 10_000_000, 10_000_000),
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ),
            "source-site allocation mutation {mutation} issued coverage"
        );
    }
}

#[test]
fn borrowed_cast_and_call_comparisons_need_no_temporary_heap_storage() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
    let fixture = fixture();
    let block = &fixture.module.functions[0].body.as_ref().unwrap().blocks[2];
    let expected = &block.operations[0].results[0].ty;
    let callee = &fixture.module.functions[2].id;
    let call_work = 8 + callee.as_str().len() + 2;
    for limit in [12 + call_work, 11, 12 + call_work - 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let mut cast = BorrowedReplaySpanV1 {
            block,
            next: 0,
            end: 1,
        };
        let result = cast.restrict_pointer(ValueId(0), expected, &mut budget);
        if limit == 11 {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
            ));
            assert_eq!(cast.next, 0);
        } else {
            assert_eq!(result.unwrap(), ValueId(4));
            cast.finish().unwrap();
            let mut call = BorrowedReplaySpanV1 {
                block,
                next: 2,
                end: 3,
            };
            let result = call.call(callee, &[ValueId(4), ValueId(5)], &mut budget);
            if limit == 12 + call_work {
                result.unwrap();
                call.finish().unwrap();
                assert_eq!(budget.work(), limit);
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
                ));
                assert_eq!(call.next, 2);
            }
        }
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.peak_storage(), 0);
    }
}

#[test]
fn same_typed_field_and_actual_formal_substitutions_reject() {
    for mutation in 0..4 {
        let mut fixture = fixture();
        match mutation {
            0 => {
                if let OperationKind::Load { pointer, .. } =
                    &mut fixture.module.functions[1].body.as_mut().unwrap().blocks[0].operations[0]
                        .kind
                {
                    *pointer = ValueId(1);
                }
            }
            1 => fixture.calls[0].formal_path.fields[0] = 1,
            2 => {
                if let OperationKind::Call { arguments, .. } =
                    &mut fixture.module.functions[0].body.as_mut().unwrap().blocks[1].operations[0]
                        .kind
                {
                    arguments.swap(0, 1);
                }
            }
            _ => fixture.calls[0].actual_owner.local = SemanticLocalIdV1::from_index(2),
        }
        assert!(
            check(&fixture, 10_000_000, 10_000_000).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn missing_extra_and_empty_candidates_reject() {
    for mutation in 0..4 {
        let mut fixture = fixture();
        match mutation {
            0 => {
                fixture.calls.pop();
            }
            1 => fixture.fields.push(fixture.fields[0].clone()),
            2 => fixture.fields.clear(),
            _ => fixture.calls.clear(),
        }
        assert!(check(&fixture, 10_000_000, 10_000_000).is_err());
    }
}

#[test]
fn work_and_storage_exhaustion_restore_scope_and_refuse() {
    let fixture = fixture();
    for (work, storage) in [(0, 10_000_000), (10_000_000, 0), (64, 10_000_000)] {
        assert!(matches!(
            check(&fixture, work, storage),
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
        ));
    }
}

#[test]
fn sealed_storage_is_retained_until_owner_release_and_limits_remain_exact() {
    for source_initialization in [false, true] {
        let fixture = fixture_with(false, source_initialization);
        let (work, storage) =
            check_with(&fixture, 10_000_000, 10_000_000, |result, budget, floor| {
                let sealed = result.unwrap();
                let retained = sealed.retained_storage();
                assert!(retained > 0);
                assert_eq!(budget.storage(), floor + retained);
                let work = budget.work();
                let peak = budget.peak_storage();
                assert!(peak > floor + retained);
                drop(sealed);
                budget.release_storage(retained).unwrap();
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.work(), work);
                assert_eq!(budget.peak_storage(), peak);
                (work, peak - floor)
            });
        check(&fixture, work, storage).unwrap();
        for (work, storage) in [(work - 1, storage), (work, storage - 1)] {
            assert!(matches!(
                check(&fixture, work, storage),
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
            ));
        }
    }
}

#[test]
fn sealed_coverage_requires_actual_borrowed_formals_and_unique_roster() {
    for mutation in 0..6 {
        let mut fixture = fixture();
        match mutation {
            0 => fixture.correspondence.borrowed_parameter_bindings[0].projection[0] = 1,
            1 => {
                fixture.correspondence.borrowed_parameter_bindings[0].transport =
                    SemanticKirBorrowedParameterTransportV1::ReferenceValue
            }
            2 => fixture.correspondence.borrowed_parameter_bindings = Box::default(),
            3 => {
                fixture.correspondence.lowered_functions[2].semantic_function =
                    SemanticFunctionIdV1::from_index(1)
            }
            4 => fixture.calls[0].root = SemanticFunctionIdV1::from_index(1),
            _ => {
                let mut block = BasicBlock::new(BlockId(0));
                block.terminator = Some(Terminator::Return { values: vec![] });
                fixture.module.functions.push(Function::internal_helper(
                    "borrowed_replay_uncovered",
                    Signature::new(vec![], vec![]),
                    vec![],
                    vec![block],
                ));
            }
        }
        assert!(
            check(&fixture, 10_000_000, 10_000_000).is_err(),
            "mutation {mutation} issued coverage"
        );
    }
}
