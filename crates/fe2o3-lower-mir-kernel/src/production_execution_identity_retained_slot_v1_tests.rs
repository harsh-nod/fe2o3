use super::*;

fn with_original_slot_plan(
    consume: impl FnOnce(
        &ExecutionInstancesV29<'_>,
        &SourceReferencePlanV29<'_, '_>,
        &ExecutionIdentityPlanV1<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ),
) -> (usize, Option<usize>) {
    let mut owner = retained_owner(false, false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(41).unwrap();
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let demands =
        source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget).unwrap();
    let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
        &owner,
        demands.types(&owner, &mut budget).unwrap(),
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )
    .unwrap();
    let mut completed = false;
    let result = production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            let result = source_storage_v29::with_source_storage_root_v29(
                &mut layouts,
                instances,
                budget,
                |references, root, budget| {
                    with_execution_identity_plan_v1(
                        instances,
                        references,
                        &root,
                        budget,
                        |plan, budget| {
                            consume(instances, references, plan.unwrap(), budget);
                            completed = true;
                            Ok(())
                        },
                    )
                    .map_err(Into::into)
                },
            );
            assert_eq!(budget.storage(), floor);
            result
        },
    );
    assert!(
        completed,
        "a caught callback panic is not a completed query"
    );
    result.unwrap();
    layouts.release(&mut budget).unwrap();
    demands.discard(&mut budget).unwrap();
    drop(owner);
    budget.release_storage(budget.storage() - 41).unwrap();
    assert_eq!(budget.storage(), 41);
    drop(budget);
    (work.work(), work.failed_work())
}

#[test]
fn retained_slot_query_has_independent_exact_work_and_preserves_prior_denials() {
    for short in [false, true] {
        let (accepted, denied) = with_original_slot_plan(|instances, references, plan, budget| {
            assert_eq!(plan.index.retained.len(), 1);
            let instance = instances.id_at(2).unwrap();
            assert_eq!(instances.instance(instance).unwrap().function(), CALLBACK);
            let floor = budget.storage();
            let peak = budget.peak_storage();
            assert!(floor > 41);
            assert!(budget.charge_work(LIMIT + 17).is_err());
            assert!(budget.reserve_storage(LIMIT).is_err());
            let storage_denial = budget.failed_storage();
            // Source/custody comparisons 12, C1 owner 5, one-key lookup
            // (floor(log2(1))+2)*16, retained row 3, solved class 2; no allocation.
            let exact = 12 + 5 + 2 * 16 + 3 + 2;
            let allowance = exact - usize::from(short);
            budget
                .charge_work(LIMIT - budget.work() - allowance)
                .unwrap();
            let before = budget.work();
            let result = plan.retained_slot_omission_v1(instances, references, instance, 1, budget);
            assert_eq!(result.is_ok(), !short, "{result:?}");
            if let Ok(omitted) = result {
                assert!(omitted);
            }
            assert_eq!(
                budget.work() - before,
                if short { exact - 2 } else { exact }
            );
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), peak);
            assert_eq!(budget.failed_storage(), storage_denial);
            assert_eq!(storage_denial, Some(floor + LIMIT));
        });
        assert_eq!(accepted, LIMIT - usize::from(short));
        assert!(denied.is_some_and(|first| first > LIMIT + 17));
    }
}

fn observe_installed_slot_query(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_scoped_source_test_layouts_v29(
        source,
        ProductionSemanticKirLimitsV1::default(),
        budget,
        |_, layouts, budget| {
            source_storage_v29::with_source_storage_root_v29(
                layouts,
                instances,
                budget,
                |references, root, budget| {
                    with_execution_identity_plan_v1(
                        instances,
                        references,
                        &root,
                        budget,
                        |plan, budget| {
                            let plan = plan.unwrap();
                            let instance = instances.id_at(2).unwrap();
                            let original = instances.instance(instance).unwrap();
                            assert_eq!(original.function(), CALLBACK);
                            let observed = emitted[instance.index()]
                                .as_ref()
                                .unwrap()
                                .execution_observation
                                .as_ref()
                                .unwrap();
                            let seed = observed.retained_seeds[1].as_ref().unwrap();
                            let floor = budget.storage();
                            let emission =
                                source_reference_optional_emission_v29(Some(references), budget)?
                                    .unwrap();
                            let mut cursor = ExecutionAvailabilityV29::new_with_identity(
                                instances,
                                instance,
                                Some(&emission),
                                Some(plan),
                                budget,
                            )?;
                            cursor.install_retained_seeds_v1(
                                &[(1, SemanticValueBindingV1::Execution(seed.clone()))],
                                budget,
                            )?;
                            let mut locals = vec![None; original.declaration().locals().len()];
                            locals[1] = Some(SemanticValueBindingV1::Execution(seed.clone()));
                            for fault in 0..4 {
                                let original = locals[1].clone();
                                if fault == 1 {
                                    locals[1] = None;
                                }
                                if fault == 2 {
                                    let Some(SemanticValueBindingV1::Execution(current)) =
                                        &mut locals[1]
                                    else {
                                        panic!()
                                    };
                                    current.identity.value = ValueId(u32::MAX);
                                }
                                let saved = cursor.retained_seeds[1].clone();
                                if fault == 3 {
                                    cursor.retained_seeds[1] = None;
                                }
                                // Isolate the query with a seed captured from real parameter
                                // installation above; the real production wrapper constructs
                                // this view only after validating the complete input mapping.
                                let prepared = PreparedInputTransportV1 {
                                    cursor: &cursor,
                                    values: &[],
                                    types: &[],
                                    locals: &locals,
                                    slot: budget.prepared_input_slot_v1().unwrap(),
                                };
                                let result = cursor
                                    .retained_installed_slot_omission_v1(1, &prepared, budget);
                                assert_eq!(result.is_ok(), fault == 0, "fault={fault}: {result:?}");
                                if let Ok(value) = result {
                                    assert!(value);
                                }
                                locals[1] = original;
                                cursor.retained_seeds[1] = saved;
                            }
                            drop(cursor);
                            drop(emission);
                            budget.release_storage(budget.storage() - floor)?;
                            REACHED.set(4);
                            Ok(())
                        },
                    )
                    .map_err(Into::into)
                },
            )
        },
    )
}

#[test]
fn actual_installed_seed_is_required_for_retained_slot_omission() {
    let result = run_suffix_owner(
        || retained_owner(false, false),
        observe_installed_slot_query,
        LIMIT,
        LIMIT,
        |_, _, _| Ok(()),
    )
    .0;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(REACHED.get(), 4);
}

#[test]
fn retained_helper_fixtures_admit_the_exact_defined_prefix_and_original_intrinsics() {
    for owner in [retained_owner(true, true), retained_call_escape_owner()] {
        let source = owner.source_semantic();
        assert_eq!(source.functions().len(), 4);
        assert_eq!(source.callables().len(), 6);
        for index in 0..4 {
            assert_eq!(
                source.callables()[index],
                SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
            );
        }
        for (function, tag) in [(0, 120), (1, 121)] {
            let SemanticTerminatorKindV1::Call(call) =
                source.functions()[function].blocks()[0].terminator().kind()
            else {
                panic!()
            };
            let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } =
                &source.callables()[call.callee().index() as usize]
            else {
                panic!()
            };
            assert_eq!(
                binding.identity(),
                SemanticFunctionIdentityV1::from_sha256([tag; 32])
            );
        }
    }
}

fn with_observed_retained_inputs<'work>(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    budget: &mut ArgumentBudgetV1<'work>,
    consume: impl FnOnce(
        &mut ExecutionAvailabilityV29<'_>,
        &mut [Option<SemanticValueBindingV1>],
        &[ValueId],
        &[Type],
        &source_storage_v29::SourceStorageRootV29<'_, '_, '_>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_scoped_source_test_layouts_v29(
        source,
        ProductionSemanticKirLimitsV1::default(),
        budget,
        |_, layouts, budget| {
            source_storage_v29::with_source_storage_root_v29(
                layouts,
                instances,
                budget,
                |references, root, budget| {
                    with_execution_identity_plan_v1(
                        instances,
                        references,
                        &root,
                        budget,
                        |identities, budget| {
                            let identities = identities.unwrap();
                            let instance = instances.id_at(2).unwrap();
                            let original = instances.instance(instance).unwrap();
                            assert_eq!(original.function(), CALLBACK);
                            assert!(original.ssa().plan().entry_arguments().is_empty());
                            assert_eq!(identities.index.retained.len(), 1);
                            let output = emitted[instance.index()].as_ref().unwrap();
                            let seed = output
                                .execution_observation
                                .as_ref()
                                .unwrap()
                                .retained_seeds[1]
                                .as_ref()
                                .unwrap();
                            let parameters = &output.function.body.as_ref().unwrap().parameters;
                            assert_eq!(parameters.len(), 1);
                            assert_eq!(
                                output.function.signature.parameters,
                                vec![Type::Scalar(ScalarType::U32)]
                            );
                            let floor = budget.storage();
                            let emission =
                                source_reference_optional_emission_v29(Some(references), budget)?
                                    .unwrap();
                            let mut cursor = ExecutionAvailabilityV29::new_with_identity(
                                instances,
                                instance,
                                Some(&emission),
                                Some(identities),
                                budget,
                            )?;
                            cursor.install_retained_seeds_v1(
                                &[(1, SemanticValueBindingV1::Execution(seed.clone()))],
                                budget,
                            )?;
                            let mut locals = vec![None; original.declaration().locals().len()];
                            locals[1] = Some(SemanticValueBindingV1::Execution(seed.clone()));
                            locals[2] = Some(SemanticValueBindingV1::Value {
                                id: parameters[0],
                                ty: output.function.signature.parameters[0].clone(),
                            });
                            assert!(cursor.invocation_inputs.is_none());
                            let cursor_storage = budget.storage() - floor;
                            let result = consume(
                                &mut cursor,
                                &mut locals,
                                parameters,
                                &output.function.signature.parameters,
                                &root,
                                budget,
                            );
                            drop(cursor);
                            drop(emission);
                            if result.is_ok()
                                && root.permits_cleanup_refund(
                                    instances,
                                    &references.failure,
                                    cursor_storage,
                                    budget,
                                )
                            {
                                budget.release_storage(cursor_storage)?;
                            }
                            result
                        },
                    )
                    .map_err(Into::into)
                },
            )
        },
    )
}

fn observe_retained_prepared_controls(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_observed_retained_inputs(
        source,
        instances,
        emitted,
        budget,
        |cursor, locals, values, types, _, budget| {
            let floor = budget.storage();
            let mut accepted = 0;
            for fault in 0..10 {
                let current = locals[1].clone();
                let seeds = cursor.retained_seeds.clone();
                let identity = cursor.identities;
                let instance = cursor.instance;
                let references = cursor.references;
                match fault {
                    0 => {}
                    1 => locals[1] = None,
                    2 => cursor.retained_seeds[1] = None,
                    3 => {
                        let Some(SemanticValueBindingV1::Execution(binding)) = &mut locals[1]
                        else {
                            panic!()
                        };
                        binding.identity.value = ValueId(u32::MAX);
                    }
                    4 => cursor.identities = None,
                    5 => cursor.instance = instances.root(),
                    6 => cursor.visited[0] = true,
                    7 => cursor.references = None,
                    8 => cursor.retained_seeds[2] = seeds[1].clone(),
                    9 => cursor.block = Some(SsaBlockIdV1::new(0)),
                    _ => unreachable!(),
                }
                let result = with_prepared_input_transport_v1(
                    Some(cursor),
                    cursor.function,
                    cursor.ssa,
                    values,
                    types,
                    locals,
                    Some(budget),
                    |prepared, budget| {
                        assert_eq!(fault, 0, "invalid installation reached the consumer");
                        let prepared = prepared.unwrap();
                        let budget = budget.unwrap();
                        assert!(cursor.invocation_inputs.is_none());
                        assert_eq!(prepared.ordinary(1, budget)?, None);
                        assert_eq!(prepared.ordinary(2, budget)?, None);
                        assert!(cursor.retained_installed_slot_omission_v1(1, prepared, budget)?);
                        accepted += 1;
                        Ok(())
                    },
                );
                assert_eq!(result.is_ok(), fault == 0, "fault {fault}: {result:?}");
                assert_eq!(budget.storage(), floor);
                locals[1] = current;
                cursor.retained_seeds = seeds;
                cursor.identities = identity;
                cursor.instance = instance;
                cursor.references = references;
                cursor.visited[0] = false;
                cursor.block = None;
            }
            assert_eq!(accepted, 1);
            let foreign = retained_owner(false, false);
            let function = &foreign.source_semantic().functions()[CALLBACK.index() as usize];
            let result: Result<(), _> = with_prepared_input_transport_v1(
                Some(cursor),
                function,
                cursor.ssa,
                values,
                types,
                locals,
                Some(budget),
                |_, _| panic!("cloned source reached the consumer"),
            );
            assert!(result.is_err());
            assert_eq!(budget.storage(), floor);
            REACHED.set(10);
            Ok(())
        },
    )
}

#[test]
fn actual_retained_inputs_without_entry_edge_ordinals_keep_exact_installation_authority() {
    let result = run_suffix_owner(
        || retained_owner(false, false),
        observe_retained_prepared_controls,
        LIMIT,
        LIMIT,
        |_, _, _| Ok(()),
    )
    .0;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(REACHED.get(), 10);
}

std::thread_local! {
    static RETAINED_PREPARED_CASE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn observe_retained_prepared_resources(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_observed_retained_inputs(
        source,
        instances,
        emitted,
        budget,
        |cursor, locals, values, types, _, budget| {
            let case = RETAINED_PREPARED_CASE.get();
            let short_work = case == 1;
            let short_storage = case == 2;
            assert_eq!(cursor.visited.len(), 3);
            assert_eq!(cursor.retained_seeds.len(), 4);
            // Common source check + visited flags; retained source check 9 and
            // three shape checks; one-key range lookup 32; one row scan 2;
            // installed query = cursor 9 + lookup/row 35 + view 1 + source query
            // (12 + C1 owner 5 + lookup/row 35 + class 2) + current/seed 3;
            // finally all four retained seed slots are counted.
            let installed = 9 + (32 + 3) + 1 + (12 + 5 + 32 + 3 + 2) + 3;
            // Four capture groups and four prepaid cleanup replay groups.
            let exact_work = (4 + 4) + 1 + 3 + 9 + 3 + 32 + 2 + installed + 4;
            assert_eq!(exact_work, 164);
            let header = std::mem::size_of::<&ExecutionAvailabilityV29<'_>>()
                + std::mem::size_of::<&[ValueId]>()
                + std::mem::size_of::<&[Type]>()
                + std::mem::size_of::<&[Option<SemanticValueBindingV1>]>()
                + std::mem::size_of::<usize>()
                + std::mem::size_of::<Option<(&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>, usize, usize)>>();
            assert_eq!(
                header,
                std::mem::size_of::<PreparedInputTransportV1<'_, '_>>()
                    + std::mem::size_of::<Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()
            );
            let setup = budget.storage();
            assert!(setup > 41);
            assert!(budget.charge_work(LIMIT + 17).is_err());
            assert!(budget.reserve_storage(LIMIT).is_err());
            let storage_denial = budget.failed_storage();
            let floor = LIMIT - header + usize::from(short_storage);
            budget.reserve_storage(floor - setup)?;
            budget.charge_work(LIMIT - exact_work + usize::from(short_work) - budget.work())?;
            let before = budget.work();
            let mut reached = false;
            let result = with_prepared_input_transport_v1(
                Some(cursor),
                cursor.function,
                cursor.ssa,
                values,
                types,
                locals,
                Some(budget),
                |prepared, _| {
                    assert!(prepared.is_some());
                    reached = true;
                    Ok(())
                },
            );
            assert_eq!(result.is_ok(), case == 0, "case {case}: {result:?}");
            assert_eq!(reached, case == 0);
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.failed_storage(), storage_denial);
            assert_eq!(
                budget.work() - before,
                if short_storage {
                    0
                } else if short_work {
                    exact_work - 4
                } else {
                    exact_work
                }
            );
            budget.release_storage(floor - setup)?;
            REACHED.set(20 + case);
            // The ledger has deliberately reached its exact boundary. Stop the
            // enclosing emission with this selected diagnostic, not later work.
            Err(source_reference_error_v29(
                "retained prepared resource probe complete",
            ))
        },
    )
}

#[test]
fn actual_retained_prepared_view_has_independent_exact_and_one_short_resources() {
    for case in 0..3 {
        RETAINED_PREPARED_CASE.set(case);
        let result = run_suffix_owner(
            || retained_owner(false, false),
            observe_retained_prepared_resources,
            LIMIT,
            LIMIT,
            |_, _, _| Ok(()),
        )
        .0;
        assert!(
            matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "retained prepared resource probe complete",
                    ..
                })
            ),
            "case {case}: {result:?}"
        );
        assert_eq!(REACHED.get(), 20 + case);
    }
}

fn retained_prepared_cleanup_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = retained_owner(false, false);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let reference_type = reference(&mut types, U32, SemanticMutabilityV1::Immutable, false);
    let mut functions = semantic.functions().to_vec();
    let root = &functions[ROOT.index() as usize];
    let mut locals = root.locals().to_vec();
    let index = locals.len() as u32;
    locals.push(local(162, reference_type, SemanticLocalRoleV1::Temporary));
    let mut blocks = root.blocks().to_vec();
    let first = &blocks[0];
    let mut statements = first.statements().to_vec();
    statements.push(assign(
        place(index, reference_type),
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: place(1, U32),
        },
    ));
    blocks[0] = fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1::new(
        first.identity(),
        first.source(),
        statements,
        first.terminator().clone(),
    )
    .unwrap();
    functions[ROOT.index() as usize] = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        root.abi().clone(),
        locals,
        root.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    retained_rebuild(types, functions, semantic.callables().to_vec())
}

// Both delegates are real concrete ledgers. The test hook can select a foreign
// delegate or grow the genuine captured arena; it grants no synthetic permit.
struct RetainedPreparedBudget<'a, 'work, 'other, 'view, 'root, 'source> {
    original: &'a mut ArgumentBudgetV1<'work>,
    foreign: &'a mut ArgumentBudgetV1<'other>,
    root: &'a source_storage_v29::SourceStorageRootV29<'view, 'root, 'source>,
    root_instance: ProductionCallInstanceIdV1,
    armed: &'a std::cell::Cell<bool>,
    fault: usize,
    performed: bool,
    foreign_active: bool,
}

impl SemanticEmissionBudgetV1 for RetainedPreparedBudget<'_, '_, '_, '_, '_, '_> {
    fn work_ledger_identity_v1(&self) -> fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 {
        if self.foreign_active {
            self.foreign.work_ledger_identity_v1()
        } else {
            self.original.work_ledger_identity_v1()
        }
    }
    fn charge_work(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.foreign_active {
            self.foreign.charge_work(amount)
        } else {
            self.original.charge_work(amount)
        }
        .map_err(Into::into)
    }
    fn reserve_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.armed.get() && !self.performed {
            self.performed = true;
            if self.fault == 2 {
                let before = self.original.storage();
                let state = self
                    .root
                    .new_state(
                        self.root_instance,
                        SemanticLocalIdV1::from_index(1),
                        self.original,
                    )
                    .unwrap();
                let path = self.root.root_path(U32, self.original).unwrap();
                self.root
                    .mutate(
                        state,
                        path,
                        source_storage_v29::SourceStorageRootMutationV29::Initialize,
                        self.original,
                    )
                    .unwrap();
                let copy = self.root.copy_state(state, self.original).unwrap();
                assert!(self.root.is_initialized(copy, path, self.original).unwrap());
                assert!(self.original.storage() > before);
            } else if self.fault == 3 {
                self.foreign.reserve_storage(self.original.storage())?;
                self.foreign_active = true;
            }
        }
        if self.foreign_active {
            self.foreign.reserve_storage(amount)
        } else {
            self.original.reserve_storage(amount)
        }
        .map_err(Into::into)
    }
    fn release_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.foreign_active {
            self.foreign.release_storage(amount)
        } else {
            self.original.release_storage(amount)
        }
        .map_err(Into::into)
    }
    fn storage(&self) -> usize {
        if self.foreign_active {
            self.foreign.storage()
        } else {
            self.original.storage()
        }
    }
    fn prepared_input_slot_v1(&self) -> Option<usize> {
        if self.foreign_active {
            self.foreign.prepared_input_slot_v1()
        } else {
            self.original.prepared_input_slot_v1()
        }
    }
    fn source_reference_owner_v29(
        &mut self,
        plan: &SourceReferencePlanV29<'_, '_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.foreign_active {
            self.foreign.source_reference_owner_v29(plan)
        } else {
            self.original.source_reference_owner_v29(plan)
        }
    }
    fn permits_prepared_input_refund_v1(
        &self,
        source: Option<&SourceReferencePlanV29<'_, '_>>,
        slot: usize,
        ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        required: usize,
        bytes: usize,
    ) -> bool {
        if self.foreign_active {
            self.foreign
                .permits_prepared_input_refund_v1(source, slot, ledger, required, bytes)
        } else {
            self.original
                .permits_prepared_input_refund_v1(source, slot, ledger, required, bytes)
        }
    }
}

fn observe_retained_prepared_cleanup(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let case = RETAINED_PREPARED_CASE.get();
    let fault = case / 6;
    let exit = (case / 2) % 3;
    let ignore = case % 2 == 1;
    let denied = fault == 1 || fault == 3 || fault == 2 && exit != 0;
    let floor = budget.storage();
    let mut reached = false;
    let mut inner_storage = 0;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_observed_retained_inputs(
            source,
            instances,
            emitted,
            budget,
            |cursor, locals, values, types, root, budget| {
                let source_floor = budget.storage();
                let mut other_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
                let mut other = ArgumentBudgetV1::new(&mut other_work, LIMIT);
                let armed = std::cell::Cell::new(false);
                assert!(budget.reserve_storage(LIMIT).is_err());
                assert!(budget.charge_work(LIMIT + 19).is_err());
                let original_denial = budget.failed_storage();
                let mut forwarder = RetainedPreparedBudget {
                    original: budget,
                    foreign: &mut other,
                    root,
                    root_instance: instances.root(),
                    armed: &armed,
                    fault,
                    performed: false,
                    foreign_active: false,
                };
                let mut full = 0;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_prepared_input_transport_v1(
                        Some(cursor),
                        cursor.function,
                        cursor.ssa,
                        values,
                        types,
                        locals,
                        Some(&mut forwarder),
                        |prepared, budget| {
                            assert!(prepared.is_some());
                            reached = true;
                            let budget = budget.unwrap();
                            full = budget.storage();
                            armed.set(true);
                            budget.reserve_storage(0)?;
                            if fault == 0 {
                                budget.reserve_storage(13)?;
                            }
                            if fault == 1 {
                                budget.release_storage(1)?;
                            }
                            match exit {
                                0 => Ok(()),
                                1 => Err(source_reference_error_v29(
                                    "retained prepared selected failure",
                                )),
                                _ => std::panic::panic_any(1146usize),
                            }
                        },
                    )
                }));
                assert!(reached && forwarder.performed);
                if fault == 3 {
                    assert_eq!(forwarder.foreign.work(), 0);
                    assert_eq!(forwarder.foreign.storage(), full);
                    assert_eq!(forwarder.original.storage(), full);
                    forwarder.foreign_active = false;
                    forwarder.foreign.release_storage(full)?;
                }
                inner_storage = forwarder.original.storage();
                assert_eq!(forwarder.original.failed_storage(), original_denial);
                if denied {
                    assert!(
                        !root
                            .custody_view()
                            .retains_after_refund(0, forwarder.original)
                    );
                    assert!(!root.permits_cleanup_refund(
                        instances,
                        &cursor.references.unwrap().plan.failure,
                        0,
                        forwarder.original
                    ));
                    assert!(inner_storage >= source_floor);
                    if fault == 1 {
                        assert_eq!(inner_storage, full - 1);
                    }
                } else if exit != 0 {
                    assert_eq!(inner_storage, source_floor);
                } else if fault == 0 {
                    assert_eq!(inner_storage, source_floor + 13);
                    forwarder.original.release_storage(13)?;
                } else {
                    assert!(inner_storage > source_floor);
                }
                match &result {
                    Ok(Ok(())) => assert_eq!((exit, denied), (0, false)),
                    Ok(Err(error)) if exit == 1 => assert!(matches!(
                        error,
                        ProductionSemanticKirErrorV1::Unsupported {
                            detail: "retained prepared selected failure",
                            ..
                        }
                    )),
                    Ok(Err(error)) => assert!(matches!(
                        error,
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    )),
                    Err(payload) => assert_eq!(payload.downcast_ref::<usize>(), Some(&1146)),
                }
                drop(forwarder);
                if ignore {
                    return Ok(());
                }
                match result {
                    Ok(result) => result,
                    Err(payload) => std::panic::resume_unwind(payload),
                }
            },
        )
    }));
    assert!(reached, "caught setup panic is not an exercised cleanup");
    if denied {
        assert!(budget.storage() > floor);
        assert!(budget.storage() >= inner_storage);
    } else {
        assert_eq!(budget.storage(), floor);
    }
    match &outcome {
        Ok(Ok(())) => assert!(!denied && (ignore || exit == 0)),
        Ok(Err(error)) if !ignore && exit == 1 => assert!(matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                detail: "retained prepared selected failure",
                ..
            }
        )),
        Ok(Err(error)) if !ignore && exit == 2 => assert!(matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                detail: "source reference callback panicked",
                ..
            }
        )),
        Ok(Err(error)) => assert!(matches!(
            error,
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )),
        Err(_) => panic!("the inner C1 scope must convert its callback panic"),
    }
    // All nested test layouts, plans and cursors have now been destroyed. This
    // explicit fixture cleanup is outside the denied owner; its denial above
    // was observed before returning to the independent original emission.
    budget.release_storage(budget.storage() - floor)?;
    REACHED.set(100 + case);
    Ok(())
}

#[test]
fn actual_prepared_cleanup_preserves_header_custody_growth_and_selected_failures() {
    for case in 0..24 {
        RETAINED_PREPARED_CASE.set(case);
        let result = run_suffix_owner(
            retained_prepared_cleanup_owner,
            observe_retained_prepared_cleanup,
            LIMIT,
            LIMIT,
            |_, _, _| Ok(()),
        )
        .0;
        assert!(result.is_ok(), "case {case}: {result:?}");
        assert_eq!(REACHED.get(), 100 + case);
    }
}

fn observe_nested_prepared_growth(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let case = RETAINED_PREPARED_CASE.get();
    let lost_byte = case >= 4;
    let panic = case % 4 >= 2;
    let ignore = case % 2 == 1;
    let floor = budget.storage();
    let reached = std::cell::Cell::new(false);
    let checked = std::cell::Cell::new(false);
    let outcome = with_observed_retained_inputs(
        source,
        instances,
        emitted,
        budget,
        |cursor, locals, values, types, root, budget| {
            let mut other_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut other = ArgumentBudgetV1::new(&mut other_work, LIMIT);
            let armed = std::cell::Cell::new(false);
            let mut forwarder = RetainedPreparedBudget {
                original: budget,
                foreign: &mut other,
                root,
                root_instance: instances.root(),
                armed: &armed,
                fault: if lost_byte { 1 } else { 2 },
                performed: false,
                foreign_active: false,
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_prepared_input_transport_v1(
                    Some(cursor),
                    cursor.function,
                    cursor.ssa,
                    values,
                    types,
                    locals,
                    Some(&mut forwarder),
                    |outer, budget| {
                        assert!(outer.is_some());
                        let budget = budget.unwrap();
                        let inner = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            with_prepared_input_transport_v1(
                                Some(cursor),
                                cursor.function,
                                cursor.ssa,
                                values,
                                types,
                                locals,
                                Some(&mut *budget),
                                |inner, budget| {
                                    assert!(inner.is_some());
                                    let budget = budget.unwrap();
                                    reached.set(true);
                                    armed.set(true);
                                    budget.reserve_storage(0)?;
                                    if lost_byte {
                                        budget.release_storage(1)?;
                                    }
                                    if panic {
                                        std::panic::panic_any(1156usize);
                                    }
                                    Err::<(), _>(source_reference_error_v29(
                                        "nested prepared selected failure",
                                    ))
                                },
                            )
                        }));
                        if lost_byte {
                            // Restoring the observed missing byte cannot reset
                            // denial on the original concrete root.
                            budget.reserve_storage(1)?;
                        }
                        assert!(
                            cursor
                                .references
                                .unwrap()
                                .plan
                                .storage_root
                                .as_ref()
                                .is_some()
                        );
                        checked.set(true);
                        match &inner {
                            Ok(Err(ProductionSemanticKirErrorV1::Unsupported {
                                detail: "nested prepared selected failure",
                                ..
                            })) => assert!(!panic),
                            Err(payload) => {
                                assert_eq!(payload.downcast_ref::<usize>(), Some(&1156))
                            }
                            other => panic!("unexpected nested result: {other:?}"),
                        }
                        if ignore {
                            return Ok(());
                        }
                        match inner {
                            Ok(result) => result,
                            Err(payload) => std::panic::resume_unwind(payload),
                        }
                    },
                )
            }));
            assert!(reached.get() && checked.get() && forwarder.performed);
            assert!(!root.permits_cleanup_refund(
                instances,
                &cursor.references.unwrap().plan.failure,
                0,
                forwarder.original,
            ));
            assert!(
                !root
                    .custody_view()
                    .retains_after_refund(0, forwarder.original)
            );
            match &result {
                Ok(Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting,
                ))) => assert!(ignore),
                Ok(Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "nested prepared selected failure",
                    ..
                })) => assert!(!ignore && !panic),
                Err(payload) => {
                    assert!(!ignore && panic);
                    assert_eq!(payload.downcast_ref::<usize>(), Some(&1156));
                }
                other => panic!("unexpected outer prepared result: {other:?}"),
            }
            // Ignore the observed error once more. The containing concrete C1/C2
            // scope must still refuse rather than refund either prepared owner.
            Ok(())
        },
    );
    assert!(
        reached.get() && checked.get(),
        "setup panic is not cleanup coverage"
    );
    assert!(
        matches!(
            outcome,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ),
        "{outcome:?}"
    );
    assert!(budget.storage() > floor);
    budget.release_storage(budget.storage() - floor)?;
    REACHED.set(200 + case);
    Ok(())
}

#[test]
fn actual_nested_prepared_growth_and_restored_counters_cannot_recover_refund_authority() {
    for case in 0..8 {
        RETAINED_PREPARED_CASE.set(case);
        let result = run_suffix_owner(
            retained_prepared_cleanup_owner,
            observe_nested_prepared_growth,
            LIMIT,
            LIMIT,
            |_, _, _| Ok(()),
        )
        .0;
        assert!(result.is_ok(), "case {case}: {result:?}");
        assert_eq!(REACHED.get(), 200 + case);
    }
}
