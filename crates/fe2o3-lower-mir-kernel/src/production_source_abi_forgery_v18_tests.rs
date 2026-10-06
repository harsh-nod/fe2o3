fn visit_original_profiled_memory_v18(
    pending: &ProductionPendingScopedSourceOwnerV29,
    budget: &mut ArgumentBudgetV1<'_>,
    source_entered: &std::cell::Cell<bool>,
    memory_entered: &std::cell::Cell<bool>,
) -> SourceOwnedResultV18<()> {
    pending.with_checked_source_v18(budget, |source, budget| {
        source_entered.set(true);
        source.with_analysis_v18(budget, |scope| {
            scope.with_sparse_and_memory_ssa_v1(|_, memory, budget| {
                source.with_ranked_correspondence_v18(
                    memory.inventory(),
                    budget,
                    |relation, budget| {
                        relation.with_root_argument_data_v18(0, budget, |data, budget| {
                            for slot in 0..2 {
                                let physical = data.physical(slot, budget)?.unwrap();
                                assert!(matches!(physical.ty(), Type::Slice(slice)
                                if slice.address_space == AddressSpace::Global));
                            }
                            let mut nodes = 0;
                            data.visit_nodes_scoped(budget, |_, _| {
                                nodes += 1;
                                Ok(())
                            })?;
                            assert!(nodes >= 3);
                            Ok(())
                        })?;
                        scoped_raw_admission_v29::with_checked_source_memory_v29(
                            relation,
                            0,
                            Some(memory),
                            budget,
                            |physical, budget| -> SourceOwnedResultV18<()> {
                                let mut effects = 0;
                                physical.visit_effects(budget, |_, _| {
                                    effects += 1;
                                    Ok(())
                                })?;
                                assert!(
                                    effects > 0,
                                    "the real descriptor read must retain memory effects"
                                );
                                memory_entered.set(true);
                                Ok(())
                            },
                        )
                    },
                )
            })
        })
    })
}

fn accept_matching_inert_shape_v18(
    pending: &ProductionPendingScopedSourceOwnerV29,
    budget: &mut ArgumentBudgetV1<'_>,
) {
    use fe2o3_pliron::source_argument_v1::scoped_v18 as inert;
    let semantic = pending.inner.source.owner.source_semantic();
    let root = &pending.inner.pending.roots[0];
    let target = &pending.inner.pending.graph.module().functions[root.function_ordinal];
    let sidecar = &root.sidecars.rows[0];
    let original = root.coordinates.root;
    assert_eq!(root.coordinates.sources.rows[0].function, original);
    assert_eq!(sidecar.source_call_instance.unwrap().index(), 0);
    assert_eq!(target.signature.parameters.len(), 3);
    for ty in &target.signature.parameters[..2] {
        assert!(matches!(ty, Type::Slice(slice) if slice.address_space == AddressSpace::Global));
    }
    assert_eq!(
        target.signature.parameters[2],
        Type::Scalar(ScalarType::U64)
    );
    let source_types = semantic.functions()[original.index() as usize]
        .abi()
        .source_input_types();
    let cleanup = fe2o3_pliron::CanonicalAnalysisCleanupV1::new();
    let before = budget.storage();
    let mut calls = 0;
    // Deliberately forged, without consulting a captured descriptor profile.
    let mut proposed = |argument: u32, ty, _: &mut ArgumentBudgetV1<'_>| {
        assert_eq!(argument as usize, calls);
        assert_eq!(source_types[argument as usize], ty);
        calls += 1;
        Ok(argument < 2)
    };
    let mut accepted = false;
    inert::with_parameter_correspondence_v18(
        semantic,
        inert::ArgumentEntryV18 {
            correspondence_owner: original,
            semantic_function: original,
            kernel_ir_function: &target.id,
            role: SemanticKirFunctionRoleV1::KernelEntry,
        },
        target,
        ArgumentTraceV1 {
            direct: &sidecar.parameter_bindings,
            components: &sidecar.parameter_component_bindings,
            ignored: &sidecar.ignored_parameter_bindings,
        },
        &cleanup,
        budget,
        Some(&mut proposed),
        |view, budget| {
            accepted = true;
            for slot in 0..3 {
                assert_eq!(
                    view.physical(slot, budget)?.unwrap().ty(),
                    &target.signature.parameters[slot]
                );
            }
            view.visit_nodes_with_budget(budget, |_, _| Ok(()))
        },
    )
    .unwrap();
    assert!(accepted);
    assert_eq!(calls, 3);
    assert!(!cleanup.refund_denied());
    assert_eq!(budget.storage(), before);
}

#[test]
fn matching_inert_descriptor_proposal_cannot_replace_final_authenticated_profile() {
    for foreign in [false, true] {
        original_profile_replay_refuses_replacement_v1760(foreign);
    }
}

#[test]
fn original_source_without_authenticated_abi_profile_cannot_infer_global_memory() {
    original_profile_replay_refuses_replacement_v1760(false);
}

fn original_profile_replay_refuses_replacement_v1760(foreign: bool) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut pending = with_pending_api_owner_v18(
        ModuleFixture::Ordinary,
        false,
        &mut budget,
        || descriptor_source_owner(DescriptorCase::READ),
        |owner, launch, input, _, budget| {
            let fixture = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
            let roots = fixture.roots();
            ProductionPendingScopedSourceOwnerV29::try_materialize_with_kernel_abi_budget_v18(
                owner,
                launch,
                input,
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                ProductionSemanticKirLimitsV1::default(),
                budget,
            )
            .unwrap()
        },
    );
    let source_entered = std::cell::Cell::new(false);
    let memory_entered = std::cell::Cell::new(false);
    let healthy_floor = budget.storage();
    visit_original_profiled_memory_v18(&pending, &mut budget, &source_entered, &memory_entered)
        .unwrap();
    assert!(source_entered.get() && memory_entered.get());
    assert_eq!(budget.storage(), healthy_floor);
    let identity = *pending.pending_identity();
    let graph_credit = pending.inner.pending.retained_storage;

    let replacement = if foreign {
        let owner = descriptor_flow_owner_v29(DescriptorFlowV29::Root);
        let original = &pending.inner.source.owner;
        assert_ne!(
            owner.source_semantic_sha256(),
            original.source_semantic_sha256()
        );
        assert_ne!(owner.identity(), original.identity());
        let left = owner.source_semantic();
        let right = original.source_semantic();
        assert_eq!(left.roots(), right.roots());
        let root = left.roots()[0].index() as usize;
        assert_eq!(
            left.functions()[root].abi().source_input_types(),
            right.functions()[root].abi().source_input_types()
        );
        assert_eq!(
            left.functions()[root].abi().source_argument_ownership(),
            right.functions()[root].abi().source_argument_ownership()
        );
        let header = size_of::<kernel_argument_abi_v18::CapturedKernelArgumentAbiV18>();
        budget.reserve_storage(header).unwrap();
        let fixture = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
        let roots = fixture.roots();
        let profile = kernel_argument_abi_v18::CapturedKernelArgumentAbiV18::capture(
            &owner,
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            &mut budget,
        )
        .unwrap();
        Some((profile, header))
    } else {
        None
    };
    let new_credit = replacement
        .as_ref()
        .map_or(0, |(profile, _)| profile.retained_storage());
    let temporary_header = replacement.as_ref().map_or(0, |(_, header)| *header);
    let old = std::mem::replace(
        &mut pending.inner.source.input.kernel_argument_abi,
        replacement.map(|(profile, _)| profile),
    )
    .unwrap();
    let old_credit = old.retained_storage();
    drop(old);
    // Change only the captured input's owner credit, never graph/table credit.
    pending.inner.source.input.retained_storage = pending
        .inner
        .source
        .input
        .retained_storage
        .checked_sub(old_credit)
        .unwrap()
        .checked_add(new_credit)
        .unwrap();
    pending.inner.retained_storage = pending
        .inner
        .retained_storage
        .checked_sub(old_credit)
        .unwrap()
        .checked_add(new_credit)
        .unwrap();
    budget
        .release_storage(old_credit.checked_add(temporary_header).unwrap())
        .unwrap();
    assert_eq!(pending.inner.pending.retained_storage, graph_credit);
    assert_eq!(*pending.pending_identity(), identity);

    accept_matching_inert_shape_v18(&pending, &mut budget);
    source_entered.set(false);
    memory_entered.set(false);
    let before = budget.storage();
    let failure =
        visit_original_profiled_memory_v18(&pending, &mut budget, &source_entered, &memory_entered)
            .unwrap_err();
    let ProductionSourceOwnedViewErrorV18::Source(ProductionPendingScopedSourceErrorV29::Source(
        ProductionSemanticKirErrorV1::Unsupported { detail, .. },
    )) = failure
    else {
        panic!("expected authentic source/profile refusal, not resource failure: {failure:?}");
    };
    assert_eq!(
        detail,
        if foreign {
            "kernel argument ABI profile differs from the complete original descriptor/source contract"
        } else {
            "scoped module differs from its complete source roster"
        }
    );
    assert!(!source_entered.get());
    assert!(!memory_entered.get());
    assert_eq!(*pending.pending_identity(), identity);
    assert_eq!(budget.storage(), before);
    let retained = pending.adopted_storage();
    drop(pending);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}
