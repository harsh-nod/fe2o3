use super::*;

mod address_state_tests {
    use super::*;
    include!("production_source_reference_addresses_v29_tests.rs");
    include!("production_source_reference_epochs_v29_tests.rs");
    include!("production_source_reference_address_resources_v29_tests.rs");
    include!("production_scoped_raw_admission_v29_tests.rs");
}

include!("production_scoped_array_v29_tests.rs");
include!("production_execution_lifecycle_insertion_v29_tests.rs");

mod module_tests {
    use super::*;
    include!("production_source_retained_index_v29_tests.rs");
    include!("production_source_retained_index_resources_v29_tests.rs");
    include!("production_source_reference_address_physical_resources_v29_tests.rs");
    include!("production_source_static_object_admission_v29_tests.rs");
    include!("production_source_reference_descriptor_source_v29_tests.rs");
    include!("production_source_reference_descriptor_emission_v29_tests.rs");
    mod external_descriptor_reference_tests {
        use super::*;
        include!("production_source_external_descriptor_reference_v29_tests.rs");
    }
    include!("production_source_reference_descriptor_hostile_v29_tests.rs");
    include!("production_source_reference_descriptor_resources_v29_tests.rs");
    include!("production_source_reference_selector_emission_v29_tests.rs");
    include!("production_scoped_slot_selector_emission_v29_tests.rs");
    include!("production_scoped_slot_selector_resources_v29_tests.rs");
    include!("production_scoped_module_fixtures_v29_tests.rs");
    include!("production_scoped_module_v29_tests.rs");
    include!("production_scoped_owned_input_v29_tests.rs");
    include!("production_scoped_assert_replay_v29_tests.rs");
    include!("production_scoped_source_replay_v29_tests.rs");
    include!("production_scoped_source_replay_resource_v29_tests.rs");
    include!("production_pending_scoped_source_v29_tests.rs");
    include!("production_source_owned_view_v18_tests.rs");
    mod closed_scalar_handoff_tests {
        use super::*;
        include!("production_source_checked_output_handoff_v18_tests.rs");
    }
    mod optimized_source_tests {
        use super::*;
        include!("production_optimized_source_v18_tests.rs");
        include!("production_optimized_source_attachment_v18_tests.rs");
        include!("production_optimized_source_control_v18_tests.rs");
        include!("production_optimized_source_memory_v18_tests.rs");
        include!("production_optimized_source_execution_v18_tests.rs");
        include!("production_optimized_source_tile_schedule_v155_tests.rs");
        include!("production_optimized_source_tile_scalar_v156_tests.rs");
        include!("production_optimized_source_tile_graph_v157_tests.rs");
        include!("production_optimized_source_native_lifecycle_v18_tests.rs");
        include!("production_optimized_source_hostile_v18_tests.rs");
        include!("production_optimized_source_resources_v18_tests.rs");
        mod consumer_tests {
            use super::*;
            include!("production_optimized_source_consumer_v18_tests.rs");
            include!("production_optimized_source_integer_v18_tests.rs");
            include!("production_optimized_source_scalar_v18_tests.rs");
            include!("production_optimized_source_descriptor_roles_v18_tests.rs");
            include!("production_optimized_source_assertions_v18_tests.rs");
            include!("production_optimized_source_currentness_v18_tests.rs");
            include!("production_optimized_source_effects_v18_tests.rs");
            include!("production_optimized_source_consumer_hostile_v18_tests.rs");
            include!("production_optimized_source_consumer_resources_v18_tests.rs");
            include!("production_optimized_source_error_custody_v18_tests.rs");
        }
    }

    include!("production_source_ranked_relation_v18_tests.rs");
    include!("production_source_owned_error_custody_v18_tests.rs");
    include!("production_source_abi_forgery_v18_tests.rs");
    include!("production_scoped_storage_source_v18_tests.rs");
    include!("production_scoped_storage_source_resource_v18_tests.rs");
    mod terminal_failure_tests {
        use super::*;
        include!("production_terminal_failure_v18_tests.rs");
        include!("production_terminal_failure_resources_v18_tests.rs");
    }
}

pub(super) mod fixtures {
    use super::*;
    include!("production_scoped_root_fixtures_v29_tests.rs");
    include!("production_scoped_call_memory_fixtures_v29_tests.rs");
    include!("production_execution_fresh_return_v1_tests.rs");
}

thread_local! {
    static ROOT_ERROR_EXPECTED_STORAGE_V29: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static ROOT_ERROR_STORAGE_CHECKED_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(super) fn assertion_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = lifecycle_owner(false);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let boolean = declaration(
        &mut types,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        None,
    );
    let mut functions = semantic.functions().to_vec();
    let provider = &functions[HELPER.index() as usize];
    let mut blocks = provider.blocks().to_vec();
    let call = blocks[1].terminator().kind().clone();
    blocks[1] = block(
        91,
        vec![],
        SemanticTerminatorKindV1::Assert {
            condition: SemanticOperandV1::Constant(SemanticConstantV1::new(
                boolean,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 1).unwrap()),
            )),
            expected: true,
            message: SemanticAssertMessageV1::NullPointerDereference,
            target: SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::AssertSuccess,
                SemanticBlockIdV1::from_index(3),
            ),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
    );
    blocks.push(block(96, vec![], call));
    blocks.push(block(
        97,
        vec![],
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(4),
        )),
    ));
    functions[HELPER.index() as usize] = function(
        100,
        SemanticFunctionRoleV1::InternalHelper,
        provider.abi().clone(),
        provider.locals().to_vec(),
        blocks,
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
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

thread_local! {
    pub(super) static ORIGINAL_TYPED_SOURCE_ENTRY_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(super) fn emit_checked(
    source: &ExecutionLifecycleSourceV29<'_>,
    launch: &ProductionSourceLaunchRosterV1,
    root: RootInput<'_>,
    groups: u32,
    limits: ProductionSemanticKirLimitsV1,
    fixture: ScopedFixture,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<DeferredLifecycleEventV29>, ProductionSemanticKirErrorV1> {
    if ORIGINAL_TYPED_SOURCE_ENTRY_V29.get() {
        assert!(fixtures::CALL_DESTINATIONS_NO_NORMAL_V29.get());
        assert!(matches!(
            fixture,
            ScopedFixture::CallDestinations {
                projected: true,
                retained_address: false,
                indexed: false,
            }
        ));
        // This fixture intentionally needs an original typed aggregate contract.
        // The compatibility emitter below cannot grant that contract. Rebuild
        // the identical original owner for the real consuming preparation API.
        let owner = fixtures::call_destinations_owner(true, false, false);
        assert_eq!(
            owner.source_semantic_sha256(),
            source.owner.source_semantic_sha256()
        );
        assert_eq!(owner.identity(), source.owner.identity());
        let launch = ProductionSourceLaunchRosterV1::try_new(
            owner.source_semantic(),
            &[ProductionSourceLaunchRootInputV1::new(
                "lifecycle_fixture",
                [88; 32],
                ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [groups, 1, 1]),
            )],
        )
        .unwrap();
        let floor = budget.storage();
        let outcome = with_canonical_call_scratch_v1(budget, |budget| {
            let emitted: Result<(), ProductionSourceOwnedViewErrorV18> = (|| {
                let prepared =
                    ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                        owner,
                        launch,
                        source.input,
                        limits,
                        budget,
                    )?;
                // Preparation captures original inputs without emitting. The
                // consuming entry emits and must encounter the observer STOP.
                prepared.with_source_consumer_v18(budget, |_, _| {
                    panic!("the no-normal observer must stop before a source view can escape");
                })
            })();
            match emitted {
                Err(ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(error),
                )) => Err(error),
                Err(ProductionSourceOwnedViewErrorV18::Resource(error)) => Err(error.into()),
                Err(error) => panic!(
                    "original typed source preparation refused before its observer: {error:?}"
                ),
                Ok(()) => panic!("the original-source observer must stop emission"),
            }
        });
        assert_eq!(budget.storage(), floor, "{outcome:?}");
        return outcome.map(|()| unreachable!("the original-source observer always stops"));
    }
    let foreign_owner = lifecycle_owner(false);
    with_scoped_source_test_layouts_v29(source, limits, budget, |demands, layouts, budget| {
        let floor = budget.storage();
        let persistent = layouts.persistent_storage_for_test();
        let outer_private = PrivateArrayPayloadV1 {
            occupied: 40,
            capacity: 80,
        };
        let result = crate::with_checked_context_root_v29(
            source.owner,
            launch,
            root,
            budget,
            |checked, budget| {
                let mut closure = ReachableClosureBudgetV1::new(limits.max_blocks);
                let mut private = PrivateArrayLazyBudgetV1::new(1, limits.max_operations);
                let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
                let mut foreign_budget = ArgumentBudgetV1::new(&mut foreign_work, 10_000_000);
                assert!(matches!(
                    emit_pending_scoped_root_v29(
                        &checked,
                        source,
                        demands,
                        layouts,
                        limits,
                        &mut closure,
                        &mut private,
                        outer_private,
                        &mut foreign_budget,
                    ),
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    )
                ));
                assert_eq!(foreign_budget.storage(), 0);
                assert_eq!(foreign_work.work(), 0);
                let foreign_source = ExecutionLifecycleSourceV29 {
                    owner: &foreign_owner,
                    launch: source.launch,
                    input: source.input,
                    kernel_argument_abi: source.kernel_argument_abi,
                    ledger: source.ledger,
                };
                assert!(
                    emit_pending_scoped_root_v29(
                        &checked,
                        &foreign_source,
                        demands,
                        layouts,
                        limits,
                        &mut closure,
                        &mut private,
                        outer_private,
                        budget,
                    )
                    .is_err()
                );
                assert_eq!(budget.storage(), floor);
                assert_eq!(closure.consumed, 0);
                assert_eq!(closure.argument_rows, 0);
                let output = emit_pending_scoped_root_v29(
                    &checked,
                    source,
                    demands,
                    layouts,
                    limits,
                    &mut closure,
                    &mut private,
                    outer_private,
                    budget,
                );
                if !matches!(
                    fixture,
                    ScopedFixture::Arrays
                        | ScopedFixture::CallDestinations { indexed: true, .. }
                        | ScopedFixture::InitializationArray(_)
                        | ScopedFixture::InitializationArrayMove(_)
                ) {
                    assert!(
                        private.active.is_none(),
                        "array-free roots keep lazy accounting inactive"
                    );
                } else if output.is_ok() {
                    assert!(private.active.is_some());
                }
                Ok(output)
            },
        )
        .map_err(|error| match error {
            crate::ProductionContextRootErrorV29::Resource(error) => error.into(),
            _ => execution_lifecycle_error_v29(),
        })?;
        let mut output = match result {
            Ok(output) => output,
            Err(error) => {
                // Hostile postflight tests supply an exact independently checked
                // residual. Ordinary errors drop root scratch, not live table growth.
                let expected = ROOT_ERROR_EXPECTED_STORAGE_V29.take().unwrap_or_else(|| {
                    let growth = layouts
                        .persistent_storage_for_test()
                        .checked_sub(persistent)
                        .unwrap();
                    floor.checked_add(growth).unwrap()
                });
                assert_eq!(
                    budget.storage(),
                    expected,
                    "failed assembly must release only its own reservations"
                );
                ROOT_ERROR_STORAGE_CHECKED_V29.set(true);
                return Err(error);
            }
        };
        assert!(output.ledger == budget.work_ledger_identity_v1());
        assert_eq!(
            output.private_payload.occupied,
            outer_private.occupied
                + output
                    .pending
                    .sidecars
                    .rows
                    .iter()
                    .map(|row| row.private_arrays.payload.occupied)
                    .sum::<usize>()
        );
        assert_eq!(
            output.private_payload.capacity,
            outer_private.capacity
                + output
                    .pending
                    .sidecars
                    .rows
                    .iter()
                    .map(|row| row.private_arrays.payload.capacity)
                    .sum::<usize>()
        );
        let schema_growth = layouts
            .persistent_storage_for_test()
            .checked_sub(persistent)
            .unwrap();
        assert_eq!(
            budget.storage() - floor,
            output.retained_emission_storage + schema_growth
        );
        assert!(output.source_slots.ledger == budget.work_ledger_identity_v1());
        assert_eq!(
            output.source_slots.source.semantic,
            *source.owner.source_semantic_sha256()
        );
        assert_eq!(output.source_slots.source.root, ROOT);
        assert_eq!(
            output.source_slots.instances.len(),
            output.pending.sidecars.rows.len()
        );
        assert!(output.source_slots.retained_storage <= output.retained_emission_storage);
        assert!(output.pending.additional_storage_bytes <= output.retained_emission_storage);
        assert_eq!(output.kernel.entry.as_str(), "lifecycle_fixture");
        assert_eq!(output.pending.function.id, output.kernel.entry);
        assert_eq!(
            output.kernel.workgroup_size,
            Some(WorkgroupSize::new(64, 1, 1))
        );
        assert_eq!(
            output.kernel.domain,
            LaunchDomain::D1 {
                x: if groups == 1 {
                    LaunchExtent::Static(64)
                } else {
                    LaunchExtent::Dynamic
                },
            }
        );
        assert_eq!(
            output.pending.sidecars.rows.len(),
            if matches!(
                fixture,
                ScopedFixture::Repeated
                    | ScopedFixture::RepeatedReferences
                    | ScopedFixture::CallDestinations { .. }
            ) {
                5
            } else {
                3
            }
        );
        assert_eq!(output.pending.coordinates.root, ROOT);
        let body = output.pending.function.body.as_ref().unwrap();
        let mut blocks = BTreeSet::new();
        let mut values: BTreeSet<_> = body.parameters.iter().copied().collect();
        for block in &body.blocks {
            assert!(blocks.insert(block.id));
            for value in &block.parameters {
                assert!(values.insert(value.id));
            }
            for operation in &block.operations {
                if let OperationKind::Call { callee, .. } = &operation.kind {
                    assert!(!callee.as_str().starts_with("__fe2o3_execution_"));
                }
                for value in &operation.results {
                    assert!(values.insert(value.id));
                }
            }
        }
        let mut observations = Vec::new();
        let mut operations = 0;
        for (index, row) in output.pending.sidecars.rows.iter().enumerate() {
            let slots = &output.source_slots.instances[index];
            let initialization = row.scoped_initialization.as_ref().unwrap();
            assert_eq!(initialization.subject.instance, slots.instance);
            assert!(initialization.subject.ledger == output.ledger);
            assert!(initialization.subject.source == output.source_slots.source);
            assert_eq!(slots.instance.index(), index);
            assert_eq!(
                row.scoped_slot_origins.as_ref().unwrap().len(),
                slots.slots.len()
            );
            assert_eq!(row.source_call_instance.unwrap().index(), index);
            assert!(row.instance_assert_origins.is_some());
            let events = row.lifecycle_events.as_ref().unwrap();
            assert_eq!(events.instance.index(), index);
            for event in &events.rows {
                if let DeferredLifecycleKindV29::Issue { result }
                | DeferredLifecycleKindV29::Derive { result, .. } = event.kind
                {
                    assert!(
                        values.insert(result.value),
                        "deferred result IDs must not alias physical definitions"
                    );
                }
            }
            if matches!(fixture, ScopedFixture::Arrays) && index != 0 {
                check_array_rows(row, if index == 1 { HELPER } else { CALLBACK });
            } else {
                assert!(!row.private_arrays.active);
            }
            observations.extend_from_slice(&events.rows);
            operations += row.emitted_operations + events.rows.len();
        }
        assert!(operations <= limits.max_operations);
        assert!(matches!(
            observations[0].kind,
            DeferredLifecycleKindV29::Issue { .. }
        ));
        assert_eq!(observations[0].original_block, BlockId(0));
        if source.owner.source_semantic().functions()[HELPER.index() as usize]
            .blocks()
            .iter()
            .any(|block| {
                matches!(
                    block.terminator().kind(),
                    SemanticTerminatorKindV1::Assert { .. }
                )
            })
        {
            let provider = &output.pending.sidecars.rows[1];
            let first = provider
                .lifecycle_events
                .as_ref()
                .unwrap()
                .placement
                .first_block;
            assert_eq!(provider.synthetic_operation_spans.len(), 1);
            let failure = &provider.synthetic_operation_spans[0];
            assert_eq!(
                failure.rule,
                SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap
            );
            assert_eq!(failure.kernel_ir_block, BlockId(first + 5));
            assert_eq!(failure.first_operation_ordinal, 0);
            assert_eq!(failure.operation_count, 1);
            assert_eq!(
                output.pending.sidecars.rows[2]
                    .lifecycle_events
                    .as_ref()
                    .unwrap()
                    .placement
                    .first_block,
                first + 6
            );
            let trap = body
                .blocks
                .iter()
                .find(|block| block.id == failure.kernel_ir_block)
                .unwrap();
            assert_eq!(trap.terminator, Some(Terminator::Unreachable));
            assert_eq!(
                trap.operations,
                vec![AmdGpuDiagnosticOperation::Trap.operation(None)]
            );
            let captures = &provider.instance_assert_origins.as_ref().unwrap().records;
            assert_eq!(captures.len(), 1);
            assert_eq!(
                captures[0].site,
                SemanticKirAssertSiteV1::new(ROOT, HELPER, SemanticBlockIdV1::from_index(1))
            );
            assert_eq!(captures[0].block, BlockId(first + 1));
            assert_eq!(captures[0].physical_success, BlockId(first + 3));
            assert_eq!(provider.blocks.len(), 4);
            assert!(provider.diagnostic_declarations.values().any(|function| {
                function.id == AmdGpuDiagnosticOperation::Trap.declaration().id
            }));
        }
        if matches!(
            fixture,
            ScopedFixture::Repeated
                | ScopedFixture::RepeatedReferences
                | ScopedFixture::CallDestinations { .. }
        ) {
            if matches!(fixture, ScopedFixture::CallDestinations { .. }) {
                assert!(!fixtures::CALL_DESTINATIONS_NO_NORMAL_V29.get());
                // The original root/provider/callback chain contains two
                // distinct returning calls to the same scalar helper.
                let semantic = source.owner.source_semantic();
                for (function, block, callee) in [
                    (ROOT, 1, HELPER),
                    (HELPER, 1, CALLBACK),
                    (CALLBACK, 0, SemanticFunctionIdV1::from_index(3)),
                    (CALLBACK, 1, SemanticFunctionIdV1::from_index(3)),
                ] {
                    let SemanticTerminatorKindV1::Call(call) =
                        semantic.functions()[function.index() as usize].blocks()[block]
                            .terminator()
                            .kind()
                    else {
                        panic!("original defined call");
                    };
                    assert_eq!(
                        semantic.callables()[call.callee().index() as usize],
                        SemanticCallableDeclV1::defined(callee)
                    );
                    assert!(call.destination().is_some());
                }
                for (function, count) in [
                    (ROOT, 1),
                    (HELPER, 1),
                    (CALLBACK, 1),
                    (SemanticFunctionIdV1::from_index(3), 2),
                ] {
                    assert_eq!(
                        output
                            .source_slots
                            .instances
                            .iter()
                            .filter(|row| row.function == function)
                            .count(),
                        count
                    );
                }
            }
            if matches!(
                fixture,
                ScopedFixture::Repeated | ScopedFixture::RepeatedReferences
            ) {
                check_repeated_instances(&output.pending.coordinates);
            } else {
                let callback = output
                    .source_slots
                    .instances
                    .iter()
                    .find(|row| row.function == CALLBACK)
                    .unwrap();
                let children: Vec<_> = output
                    .source_slots
                    .instances
                    .iter()
                    .filter(|row| row.function == SemanticFunctionIdV1::from_index(3))
                    .collect();
                assert_ne!(children[0].instance, children[1].instance);
                for (block, child) in children.iter().enumerate() {
                    assert_eq!(
                        child.incoming,
                        Some(ProductionCallOccurrenceV1 {
                            caller: callback.instance,
                            block: SemanticBlockIdV1::from_index(block as u32),
                        })
                    );
                }
            }
        }
        if matches!(fixture, ScopedFixture::Arrays) {
            for row in &mut output.pending.sidecars.rows[1..] {
                let mut legacy = PrivateArrayMergeV1::new(limits.max_operations);
                let mut work = PrivateArrayLazyBudgetV1::new(1, limits.max_operations);
                assert!(matches!(
                    legacy.append_function(
                        ROOT,
                        row.lifecycle_events.as_ref().unwrap().function,
                        0,
                        row.emitted_operations,
                        std::mem::take(&mut row.private_arrays),
                        None,
                        &mut work,
                    ),
                    Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                ));
                assert!(!legacy.active);
                assert!(work.active.is_none());
            }
        }
        let original_layouts = layouts.rows(source.owner, budget)?;
        check_inserted_lifecycle(output, limits, fixture, &original_layouts, budget)?;
        drop(original_layouts);
        assert_eq!(
            layouts.persistent_storage_for_test(),
            persistent + schema_growth
        );
        assert_eq!(budget.storage(), floor + schema_growth);
        Ok(observations)
    })
}

fn fault(groups: u32, limits: ProductionSemanticKirLimitsV1) -> Fault {
    Fault::Orchestrated {
        groups,
        limits,
        fixture: ScopedFixture::Plain,
    }
}

fn check_repeated_instances(coords: &OwnedInstanceCoordinatesV1) {
    let repeated: Vec<_> = coords
        .sources
        .rows
        .iter()
        .filter(|row| row.function == SemanticFunctionIdV1::from_index(3))
        .collect();
    assert_eq!(repeated.len(), 2);
    assert_ne!(repeated[0].instance, repeated[1].instance);
    assert_eq!(repeated[0].identity, repeated[1].identity);
    let callback = coords
        .sources
        .rows
        .iter()
        .find(|row| row.function == CALLBACK)
        .unwrap();
    let root = coords
        .sources
        .rows
        .iter()
        .find(|row| row.function == ROOT)
        .unwrap();
    let mut parameters = BTreeSet::new();
    let mut names = BTreeSet::new();
    for (index, row) in repeated.iter().enumerate() {
        assert_eq!(
            row.incoming,
            Some(ProductionCallOccurrenceV1 {
                caller: callback.instance,
                block: SemanticBlockIdV1::from_index(index as u32),
            })
        );
        let seed = coords
            .seeds
            .rows
            .iter()
            .find(|seed| seed.instance == row.instance)
            .unwrap();
        assert_eq!(seed.container, root.instance);
        assert!(names.insert(seed.function_name.as_str()));
        let values = &coords.values.rows[seed.parameters.clone()];
        assert_eq!(values.len(), 1);
        assert!(parameters.insert(values[0]));
    }
    let anchors: Vec<_> = (0..2)
        .map(|block| {
            coords
                .anchors
                .rows
                .iter()
                .find(|row| {
                    row.instance == callback.instance
                        && row.source.semantic_block == SemanticBlockIdV1::from_index(block)
                })
                .unwrap()
        })
        .collect();
    for anchor in &anchors {
        assert!(anchor.removed);
        assert_eq!(anchor.arguments.len(), 1);
        assert_eq!(anchor.results.len(), 1);
    }
    assert_eq!(
        coords.values.rows[anchors[0].results.clone()],
        coords.values.rows[anchors[1].arguments.clone()]
    );
}

#[test]
fn checked_root_placement_accounts_for_assert_failure_and_unreachable_holes() {
    run_lifecycle(
        false,
        Fault::Orchestrated {
            groups: 2,
            limits: ProductionSemanticKirLimitsV1::default(),
            fixture: ScopedFixture::Assertion,
        },
        10_000_000,
        10_000_000,
    )
    .0
    .unwrap();
}

#[test]
fn checked_root_orchestrates_real_launch_and_retains_lifecycle_sidecars() {
    for branches in [false, true] {
        for groups in [1, 2, 7] {
            let result = run_lifecycle(
                branches,
                fault(groups, ProductionSemanticKirLimitsV1::default()),
                10_000_000,
                10_000_000,
            )
            .0
            .unwrap();
            assert_eq!(result.len(), if branches { 4 } else { 3 });
            assert_eq!(
                result
                    .iter()
                    .filter(|row| matches!(row.kind, DeferredLifecycleKindV29::Issue { .. }))
                    .count(),
                1
            );
            assert_eq!(
                result
                    .iter()
                    .filter(|row| matches!(row.kind, DeferredLifecycleKindV29::Derive { .. }))
                    .count(),
                1
            );
        }
    }
}

#[test]
fn checked_root_orchestration_obeys_exact_ledger_boundaries() {
    for fixture in [
        ScopedFixture::Plain,
        ScopedFixture::Repeated,
        ScopedFixture::Arrays,
    ] {
        let mode = Fault::Orchestrated {
            groups: 2,
            limits: ProductionSemanticKirLimitsV1::default(),
            fixture,
        };
        let branches = !matches!(fixture, ScopedFixture::Repeated);
        let (result, work, storage) = run_lifecycle(branches, mode, 10_000_000, 10_000_000);
        result.unwrap_or_else(|error| panic!("{fixture:?}: {error:?}"));
        assert!(run_lifecycle(branches, mode, work, storage).0.is_ok());
        assert!(run_lifecycle(branches, mode, work - 1, storage).0.is_err());
        assert!(run_lifecycle(branches, mode, work, storage - 1).0.is_err());
    }
}

fn check_scoped_fixture(branches: bool, fixture: ScopedFixture) {
    let result = run_lifecycle(
        branches,
        Fault::Orchestrated {
            groups: 2,
            limits: ProductionSemanticKirLimitsV1::default(),
            fixture,
        },
        10_000_000,
        10_000_000,
    )
    .0
    .unwrap_or_else(|error| panic!("{fixture:?}: {error:?}"));
    assert_eq!(result.len(), if branches { 4 } else { 3 });
}

#[test]
fn checked_root_preserves_repeated_instances() {
    check_scoped_fixture(false, ScopedFixture::Repeated);
}

#[test]
fn checked_root_preserves_scoped_arrays() {
    check_scoped_fixture(false, ScopedFixture::Arrays);
    check_scoped_fixture(true, ScopedFixture::Arrays);
}

#[test]
fn checked_root_counts_splice_blocks_at_the_exact_limit() {
    let limits =
        |blocks| ProductionSemanticKirLimitsV1::new_with_max_operations(16, blocks, 128, 1024);
    run_lifecycle(false, fault(2, limits(11)), 10_000_000, 10_000_000)
        .0
        .unwrap();
    assert!(matches!(
        run_lifecycle(false, fault(2, limits(10)), 10_000_000, 10_000_000).0,
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::Blocks,
            actual: 11,
            limit: 10
        })
    ));
}

#[test]
fn checked_root_orchestration_bounds_whole_expansion_before_emission() {
    for limits in [
        ProductionSemanticKirLimitsV1::new_with_max_operations(1, 128, 128, 1024),
        ProductionSemanticKirLimitsV1::new_with_max_operations(16, 1, 128, 1024),
        ProductionSemanticKirLimitsV1::new_with_max_operations(16, 128, 0, 1024),
        ProductionSemanticKirLimitsV1::new_with_max_operations(16, 128, 128, 0),
    ] {
        assert!(
            matches!(
                run_lifecycle(true, fault(2, limits), 10_000_000, 10_000_000).0,
                Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
            ),
            "{limits:?}"
        );
    }
}
pub(super) mod source_slot_tests {
    include!("production_scoped_source_slots_v29_tests.rs");
}

mod execution_archive_discard_tests {
    use super::*;

    thread_local! {
        static FAULT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
        static COMPLETED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    fn observe(
        pending: &mut PendingScopedRootEmissionV29,
        instances: &ExecutionInstancesV29<'_>,
        plan: &SourceReferencePlanV29<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let before = budget.storage();
        let bytes: usize = pending
            .sidecars
            .rows
            .iter()
            .filter_map(|row| {
                row.execution_observation
                    .as_ref()
                    .map(|archive| archive.credit.bytes)
            })
            .sum();
        assert!(bytes > 0);
        let first = pending
            .sidecars
            .rows
            .iter()
            .position(|row| row.execution_observation.is_some())
            .unwrap();
        let original = pending.sidecars.rows[first]
            .execution_observation
            .as_ref()
            .unwrap()
            .credit;
        match FAULT.get() {
            0 => {
                let retained = pending.rvalue_results.as_ref().unwrap();
                let retained_rows = retained.rows.as_ptr();
                let retained_count = retained.rows.len();
                assert!(retained_count > 0);
                discard_root_execution_archives_v29(pending, instances, plan, budget)?;
                assert!(
                    pending
                        .sidecars
                        .rows
                        .iter()
                        .all(|row| row.execution_observation.is_none())
                );
                assert_eq!(budget.storage(), before - bytes);
                let retained = pending.rvalue_results.as_ref().unwrap();
                assert_eq!(retained.rows.as_ptr(), retained_rows);
                assert_eq!(retained.rows.len(), retained_count);
                COMPLETED.set(true);
                Ok(())
            }
            fault => {
                let archive = pending.sidecars.rows[first]
                    .execution_observation
                    .as_mut()
                    .unwrap();
                if fault == 1 {
                    archive.credit.slot ^= 1;
                } else {
                    // An unpaid claim, not a simulated C2 lifetime/floor proof.
                    archive.credit.bytes = before.checked_add(1).unwrap();
                }
                let result = discard_root_execution_archives_v29(pending, instances, plan, budget);
                assert!(matches!(
                    &result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    )
                ));
                assert_eq!(budget.storage(), before);
                assert!(
                    pending
                        .sidecars
                        .rows
                        .iter()
                        .all(|row| row.execution_observation.is_some())
                );
                // Restore only the hostile metadata for legitimate error teardown.
                // The original refusal still propagates through the real root.
                pending.sidecars.rows[first]
                    .execution_observation
                    .as_mut()
                    .unwrap()
                    .credit = original;
                COMPLETED.set(true);
                result
            }
        }
    }

    fn run(fault: u8) {
        struct Reset(Option<RootExecutionArchiveObserverV29>);
        impl Drop for Reset {
            fn drop(&mut self) {
                ROOT_EXECUTION_ARCHIVE_DISCARD_OBSERVER_V30.set(self.0);
            }
        }
        FAULT.set(fault);
        COMPLETED.set(false);
        let reset = Reset(ROOT_EXECUTION_ARCHIVE_DISCARD_OBSERVER_V30.replace(Some(observe)));
        let result = run_lifecycle(
            false,
            super::fault(2, ProductionSemanticKirLimitsV1::default()),
            10_000_000,
            10_000_000,
        )
        .0;
        drop(reset);
        assert!(
            COMPLETED.get(),
            "actual pending-root archive assertions did not complete"
        );
        if fault == 0 {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            ));
        }
    }

    #[test]
    fn actual_root_drops_archives_before_exact_recorded_credit_refund() {
        run(0);
    }

    #[test]
    fn actual_root_wrong_archive_slot_refuses_without_output_or_refund() {
        run(1);
    }

    #[test]
    fn actual_root_unpaid_archive_claim_refuses_without_output_or_refund() {
        run(2);
    }
}

mod source_address_emission_tests {
    include!("production_source_reference_address_emission_v29_tests.rs");
}
