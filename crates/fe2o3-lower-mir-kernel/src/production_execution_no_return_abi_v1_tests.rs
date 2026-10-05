use fe2o3_mir_model::semantic_mir_v1::*;

fn no_normal_mixed_owner_v1() -> ProductionSemanticSsaOwnerV1 {
    let original = fresh_tile_return_owner_v1(FreshReturnCaseV1::Mixed);
    let semantic = original.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let child = &functions[4];
    let mut blocks = child.blocks().to_vec();
    let last = blocks.last_mut().unwrap();
    assert!(matches!(
        last.terminator().kind(),
        SemanticTerminatorKindV1::Return
    ));
    *last = SemanticBasicBlockV1::new(
        last.identity(),
        last.source(),
        vec![],
        SemanticTerminatorV1::new(
            last.terminator().source(),
            SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::Goto,
                SemanticBlockIdV1::from_index(2),
            )),
        ),
    )
    .unwrap();
    let replacement = SemanticFunctionDeclV1::new(
        child.identity(),
        child.role(),
        child.item_definition_identity(),
        child.monomorphization_identity(),
        child.generic_type_arguments_identity(),
        child.const_generic_arguments_identity(),
        child.source(),
        child.abi().clone(),
        child.locals().to_vec(),
        child.entry(),
        blocks,
    )
    .unwrap();
    assert_eq!(replacement.abi(), child.abi());
    assert_eq!(replacement.locals(), child.locals());
    assert_eq!(&replacement.blocks()[..2], &child.blocks()[..2]);
    functions[4] = replacement;
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

// Direct ABI-query tests retain the actual original C1 planner and budget. The
// unchanged stack regressions separately exercise the integrated C2/root route.
fn with_no_normal_abi_plan_v1(
    mut owner: ProductionSemanticSsaOwnerV1,
    expected_work_failure: Option<usize>,
    consume: impl FnOnce(
        &mut SourceReferencePlanV29<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(53)?;
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage())?;
    let caller_floor = budget.storage();
    let result = production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            let result = (|| {
                let mut plan = SourceReferenceBuilderV29::new_with_storage(
                    instances,
                    SourceReferenceStorageV29::ScalarCells,
                    budget,
                )?
                .build(budget)?;
                assert_eq!(plan.entries.len(), instances.instances().len());
                assert_eq!(plan.returns.len(), instances.instances().len());
                for ordinal in 0..instances.instances().len() {
                    let id = instances.id_at(ordinal).unwrap();
                    let active = instances.instance_reachable(id).unwrap();
                    assert_eq!(plan.entries[ordinal].is_some(), active);
                    assert_eq!(
                        plan.returns[ordinal].is_some(),
                        active && instances.instance_may_return(id).unwrap(),
                    );
                }
                plan.retained_floor = budget.storage();
                consume(&mut plan, budget)
            })();
            budget.release_storage(
                budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            )?;
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), caller_floor);
    drop(owner);
    budget.release_storage(capture.retained_storage())?;
    assert_eq!(budget.storage(), 53);
    drop(budget);
    assert_eq!(work.failed_work(), expected_work_failure);
    result
}

fn no_normal_child_v1(plan: &SourceReferencePlanV29<'_, '_>) -> ProductionCallInstanceIdV1 {
    plan.instances
        .instances()
        .iter()
        .enumerate()
        .find_map(|(ordinal, row)| {
            let id = plan.instances.id_at(ordinal).unwrap();
            (row.function().index() == 4 && plan.instances.instance_reachable(id) == Some(true))
                .then_some(id)
        })
        .unwrap()
}

fn assert_no_normal_abi_refusal_v1(result: Result<Vec<Type>, ProductionSemanticKirErrorV1>) {
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "execution call parameters differ from their source instance",
                ..
            })
        ),
        "{result:?}"
    );
}

#[test]
fn original_no_normal_result_abi_erases_nominal_but_keeps_mixed_ordinary_components() {
    for mixed in [false, true] {
        let owner = if mixed {
            no_normal_mixed_owner_v1()
        } else {
            fresh_tile_return_owner_v1(FreshReturnCaseV1::NoNormalReturn)
        };
        let mut observed = false;
        with_no_normal_abi_plan_v1(owner, None, |plan, budget| {
            let child = no_normal_child_v1(plan);
            let row = plan.instances.instance(child).unwrap();
            assert_eq!(plan.instances.instance_may_return(child), Some(false));
            assert!(row.declaration().blocks().iter().all(|block| !matches!(
                block.terminator().kind(),
                SemanticTerminatorKindV1::Return
            )));
            assert_eq!(plan.returns[child.index()], None);
            let roster = plan.returns.clone();
            let expected = if mixed {
                vec![Type::Scalar(ScalarType::U32)]
            } else {
                vec![]
            };
            let floor = budget.storage();
            let physical =
                source_reference_no_normal_result_types_v29(plan.instances, plan, child, budget)?;
            assert_eq!(physical, expected);
            let header = std::mem::size_of::<Vec<Type>>()
                + 2 * std::mem::size_of::<Result<Vec<Type>, ProductionSemanticKirErrorV1>>();
            assert_eq!(
                budget.storage() - floor,
                header + physical.capacity() * std::mem::size_of::<Type>()
            );
            let retained = header + physical.capacity() * std::mem::size_of::<Type>();
            drop(physical);
            budget.release_storage(retained)?;
            assert_eq!(budget.storage(), floor);
            let signature = execution_function_signature_with_references_v29(
                plan.instances,
                child,
                Some(plan),
                budget,
            )?;
            let layout = execution_instance_plan_with_references_v29(
                plan.instances,
                child,
                FunctionId::new("actual_no_normal_child"),
                SemanticEmissionPlacementV1 {
                    first_block: 91,
                    first_value: 400,
                },
                Some(plan),
                budget,
            )?;
            assert_eq!(signature.result_types, expected);
            assert_eq!(layout.result_types, signature.result_types);
            assert_eq!(layout.parameter_types, signature.parameter_types);
            assert_eq!(
                plan.returns, roster,
                "ABI projection must not create a returned node"
            );
            observed = true;
            Ok(())
        })
        .unwrap();
        assert!(observed);
    }
}

#[test]
fn no_normal_abi_does_not_use_absence_as_return_or_instance_authority() {
    let mut observed = 0;
    with_no_normal_abi_plan_v1(
        fresh_tile_return_owner_v1(FreshReturnCaseV1::Straight),
        None,
        |plan, budget| {
            let child = no_normal_child_v1(plan);
            assert_eq!(plan.instances.instance_may_return(child), Some(true));
            assert!(plan.returns[child.index()].is_some());
            assert_no_normal_abi_refusal_v1(source_reference_no_normal_result_types_v29(
                plan.instances,
                plan,
                child,
                budget,
            ));
            let saved = plan.returns[child.index()].take();
            assert_no_normal_abi_refusal_v1(source_reference_no_normal_result_types_v29(
                plan.instances,
                plan,
                child,
                budget,
            ));
            assert!(
                execution_function_signature_with_references_v29(
                    plan.instances,
                    child,
                    Some(plan),
                    budget
                )
                .is_err()
            );
            plan.returns[child.index()] = saved;
            assert!(
                execution_function_signature_with_references_v29(
                    plan.instances,
                    child,
                    Some(plan),
                    budget
                )
                .is_ok()
            );
            observed += 1;
            Ok(())
        },
    )
    .unwrap();
    with_no_normal_abi_plan_v1(
        fresh_tile_return_owner_v1(FreshReturnCaseV1::NoNormalReturn),
        None,
        |plan, budget| {
            let child = no_normal_child_v1(plan);
            let inactive = plan
                .instances
                .instances()
                .iter()
                .enumerate()
                .find_map(|(ordinal, _)| {
                    let id = plan.instances.id_at(ordinal).unwrap();
                    (plan.instances.instance_reachable(id) == Some(false)).then_some(id)
                })
                .unwrap();
            assert_no_normal_abi_refusal_v1(source_reference_no_normal_result_types_v29(
                plan.instances,
                plan,
                inactive,
                budget,
            ));
            let real_node = plan.entries[child.index()].unwrap();
            let real_node = plan.states[real_node]
                .iter()
                .find_map(|local| local.node)
                .unwrap();
            plan.returns[child.index()] = Some(real_node);
            assert_no_normal_abi_refusal_v1(source_reference_no_normal_result_types_v29(
                plan.instances,
                plan,
                child,
                budget,
            ));
            plan.returns[child.index()] = None;
            let mut foreign = fresh_tile_return_owner_v1(FreshReturnCaseV1::NoNormalReturn);
            let capture = foreign
                .try_capture_occurrences_with_budget_v1(budget)
                .unwrap();
            budget.reserve_storage(capture.retained_storage())?;
            production_call_instances_v1::with_production_call_instances_v1(
                &foreign,
                ROOT,
                budget,
                |other, budget| {
                    let foreign_child = other
                        .instances()
                        .iter()
                        .enumerate()
                        .find_map(|(ordinal, row)| {
                            (row.function().index() == 4).then(|| other.id_at(ordinal).unwrap())
                        })
                        .unwrap();
                    assert_eq!(
                        foreign_child, child,
                        "equal coordinates are not owner authority"
                    );
                    assert!(matches!(
                        source_reference_no_normal_result_types_v29(
                            other,
                            plan,
                            foreign_child,
                            budget
                        ),
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Accounting
                            )
                        )
                    ));
                    Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
                },
            )
            .unwrap();
            drop(foreign);
            budget.release_storage(capture.retained_storage())?;
            observed += 1;
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(observed, 2);
}

#[test]
fn no_normal_abi_preflight_has_independent_exact_and_one_short_work() {
    for short in [false, true] {
        let mut observed = false;
        with_no_normal_abi_plan_v1(fresh_tile_return_owner_v1(FreshReturnCaseV1::Straight), Some(LIMIT + 19), |plan, budget| {
            let child = no_normal_child_v1(plan);
            // Original owner5, result-envelope owner5, then the source-bound
            // validation debit(owner5 + validation6).
            // The real returning child refuses immediately after this boundary.
            let exact = 5 + 5 + 5 + 6;
            let remaining = exact - usize::from(short);
            budget.charge_work(LIMIT - budget.work() - remaining)?;
            assert!(budget.charge_work(remaining + 19).is_err());
            let floor = budget.storage();
            let before = budget.work();
            let result = scoped_slot_attempt_v29(budget, |budget|
                source_reference_no_normal_result_types_v29(plan.instances, plan, child, budget));
            if short {
                let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(error))) = result else {
                    panic!("one-short validation debit: {result:?}")
                };
                assert_eq!(error.actual(), LIMIT + 1);
                assert_eq!(error.limit(), LIMIT);
                assert_eq!(budget.work(), before + 15);
                let unchanged = (budget.work(), budget.storage(), budget.failed_storage());
                assert!(matches!(source_reference_no_normal_result_types_v29(plan.instances, plan, child, budget),
                    Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(again))) if again == error));
                assert_eq!((budget.work(), budget.storage(), budget.failed_storage()), unchanged);
            } else {
                assert_no_normal_abi_refusal_v1(result);
                assert_eq!(budget.work(), before + exact);
                assert!(plan.failure.first_error().is_none());
            }
            assert_eq!(budget.storage(), floor);
            observed = true;
            Ok(())
        }).unwrap();
        assert!(observed);
    }
}

fn independent_fragment_abi_budget_v1(
    plan: &SourceReferencePlanV29<'_, '_>,
    child: ProductionCallInstanceIdV1,
) -> (usize, usize, usize) {
    let function = plan.instances.instance(child).unwrap().declaration();
    let types = plan.instances.owner().source_semantic().types();
    let ty = function.abi().source_output_type();
    assert!(matches!(
        types[ty.index() as usize].rust_type_kind(),
        SemanticRustTypeKindV1::Execution(_)
    ));
    let SemanticTypeShapeV1::Aggregate(fields) = types[ty.index() as usize].shape() else {
        panic!("original fragment shape")
    };
    assert_eq!(fields.fields().len(), 4);
    for (ordinal, ty) in fields.fields().iter().enumerate() {
        if ordinal < 2 {
            let SemanticTypeShapeV1::Array { element, length } = types[ty.index() as usize].shape()
            else {
                panic!("fragment array")
            };
            assert_eq!(*length, 2);
            assert!(matches!(
                types[element.index() as usize].shape(),
                SemanticTypeShapeV1::Scalar(_)
            ));
        } else {
            assert!(
                matches!(types[ty.index() as usize].shape(), SemanticTypeShapeV1::Aggregate(fields) if fields.fields().is_empty())
            );
        }
    }
    // Nine ABI nodes; four aggregate and four array-child visits; four scalar
    // words with two selectors each. Only the first path/output push grows.
    let preflight = 5 + 5 + (5 + 6) + (5 + function.locals().len());
    let empty_vectors = 2 * (5 + 5 + 3);
    let nodes = 9 * (5 + 8);
    let aggregate_edges = 4 * (5 + 2);
    let array_edges = 4 * (5 + 2);
    let child_queries = 8 * (5 + 2);
    let path_pushes = 8 * (5 + 2) + 3;
    let scalar_words = 4 * ((5 + 5 + 3) + (5 + 2) + (5 + 2)) + 3;
    let abi_compare = 64 + 8 * (3 + 20);
    let final_projection = 1;
    let exact_work = preflight
        + empty_vectors
        + nodes
        + aggregate_edges
        + array_edges
        + child_queries
        + path_pushes
        + scalar_words
        + abi_compare
        + final_projection;
    type Path = Vec<SemanticKirParameterProjectionV1>;
    type Components = Vec<ByValueKernelParameterComponentV1>;
    let path_header = std::mem::size_of::<Path>()
        + 2 * std::mem::size_of::<Result<Path, ProductionSemanticKirErrorV1>>();
    let component_header = std::mem::size_of::<Components>()
        + 2 * std::mem::size_of::<Result<Components, ProductionSemanticKirErrorV1>>();
    let before_compare = 10 * path_header
        + 2 * component_header
        + 12 * std::mem::size_of::<SemanticKirParameterProjectionV1>()
        + 4 * std::mem::size_of::<ByValueKernelParameterComponentV1>();
    let compare_bytes = std::mem::size_of::<Vec<(u64, u64, SemanticBackendScalarV1)>>()
        + 8 * std::mem::size_of::<(u64, u64, SemanticBackendScalarV1)>();
    let retained = std::mem::size_of::<Vec<Type>>()
        + 2 * std::mem::size_of::<Result<Vec<Type>, ProductionSemanticKirErrorV1>>();
    let peak = retained + before_compare + compare_bytes;
    assert!(peak > retained);
    (exact_work, peak, retained)
}

#[test]
fn original_no_normal_abi_has_independent_full_work_storage_and_first_denial_boundaries() {
    for cut in 0..3 {
        for prior in [false, true] {
            let expected_work = if prior {
                Some(LIMIT + 19)
            } else if cut == 1 {
                Some(LIMIT + 1)
            } else {
                None
            };
            let mut observed = false;
            with_no_normal_abi_plan_v1(fresh_tile_return_owner_v1(FreshReturnCaseV1::NoNormalReturn), expected_work, |plan, budget| {
                let child = no_normal_child_v1(plan);
                let (exact_work, peak, retained) = independent_fragment_abi_budget_v1(plan, child);
                let remaining_work = exact_work - usize::from(cut == 1);
                let remaining_storage = peak - usize::from(cut == 2);
                budget.charge_work(LIMIT - budget.work() - remaining_work)?;
                budget.reserve_storage(LIMIT - budget.storage() - remaining_storage)?;
                let start = (budget.work(), budget.storage());
                if prior {
                    assert!(budget.charge_work(remaining_work + 19).is_err());
                    assert!(budget.reserve_storage(remaining_storage + 29).is_err());
                    assert!(plan.failure.first_error().is_none());
                }
                let result = scoped_slot_attempt_v29(budget, |budget|
                    source_reference_no_normal_result_types_v29(plan.instances, plan, child, budget));
                match cut {
                    0 => {
                        let result = result?;
                        assert!(result.is_empty());
                        assert_eq!(budget.work(), start.0 + exact_work);
                        assert_eq!(budget.storage(), start.1 + retained);
                        assert_eq!(budget.peak_storage(), LIMIT);
                        assert!(plan.failure.first_error().is_none());
                        drop(result);
                        budget.release_storage(retained)?;
                    }
                    1 | 2 => {
                        let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) = result else {
                            panic!("one-short ABI result: {result:?}");
                        };
                        if cut == 1 {
                            assert!(matches!(error, ArgumentResourceV1::Work(limit) if limit.actual() == LIMIT + 1 && limit.limit() == LIMIT));
                            assert_eq!(budget.work(), start.0 + exact_work - 1);
                        } else {
                            assert!(matches!(error, ArgumentResourceV1::Storage(limit) if limit.actual() == LIMIT + 1 && limit.limit() == LIMIT));
                            assert_eq!(budget.work(), start.0 + exact_work - 1);
                        }
                        assert!(matches!(plan.failure.first_error(), Some(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(first)) if first == error));
                        let unchanged = (budget.work(), budget.storage(), budget.failed_storage());
                        assert!(matches!(source_reference_no_normal_result_types_v29(plan.instances, plan, child, budget),
                            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(first)) if first == error));
                        assert_eq!((budget.work(), budget.storage(), budget.failed_storage()), unchanged);
                    }
                    _ => unreachable!(),
                }
                assert_eq!(budget.storage(), start.1);
                assert_eq!(budget.failed_storage(), if prior { Some(LIMIT + 29) } else if cut == 2 { Some(LIMIT + 1) } else { None });
                assert_eq!(plan.returns[child.index()], None);
                observed = true;
                Ok(())
            }).unwrap();
            assert!(observed);
        }
    }
}

fn observe_mixed_no_normal_abi_v1(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut checked = 0;
    for (ordinal, original) in instances.instances().iter().enumerate() {
        let child = instances.id_at(ordinal).unwrap();
        if original.function().index() != 4 || instances.instance_reachable(child) != Some(true) {
            continue;
        }
        let output = emitted[ordinal].as_ref().unwrap();
        assert_eq!(
            output.function.signature.results,
            [Type::Scalar(ScalarType::U32)]
        );
        let incoming = instances.incoming(child).unwrap();
        let caller = emitted[incoming.occurrence().caller.index()]
            .as_ref()
            .unwrap();
        let anchor = caller
            .call_returns
            .sites
            .rows
            .iter()
            .find(|site| site.semantic_block == incoming.occurrence().block)
            .unwrap();
        let SemanticKirCallReturnKindV1::NoNormalReturnCall { call_operation, .. } = anchor.kind
        else {
            panic!("original mixed no-normal call")
        };
        let mapping = caller
            .blocks
            .iter()
            .find(|mapping| mapping.semantic_block == incoming.occurrence().block)
            .unwrap();
        let block = caller
            .function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .find(|block| block.id == mapping.kernel_ir_block)
            .unwrap();
        let operation = &block.operations[call_operation as usize];
        assert!(
            matches!(&operation.kind, OperationKind::Call { callee, .. } if callee == &output.function.id)
        );
        assert_eq!(operation.results.len(), 1);
        assert_eq!(operation.results[0].ty, Type::Scalar(ScalarType::U32));
        assert_eq!(call_operation as usize + 1, block.operations.len());
        assert!(matches!(block.terminator, Some(Terminator::Unreachable)));
        assert_eq!(anchor.components(), CallComponentSpanV1::EMPTY);
        checked += 1;
    }
    assert_eq!(checked, 1);
    observe_no_normal_stack_v1(source, instances, emitted, slots, budget)
}

#[test]
fn actual_no_normal_mixed_call_keeps_typed_unused_ir_results_without_a_source_result() {
    FRESH_SOURCE_OBSERVED_V1.set((0, 0));
    let (result, _, _) = run_profiled_suffix_owner(
        no_normal_mixed_owner_v1,
        observe_mixed_no_normal_abi_v1,
        LIMIT,
        LIMIT,
        |_, _, _| panic!("observer stops before divergent lifecycle admission"),
    );
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "fresh no-normal source observed before lifecycle admission",
                ..
            })
        ),
        "{result:?}"
    );
    assert_eq!(FRESH_SOURCE_OBSERVED_V1.get(), (5, 4));
}
