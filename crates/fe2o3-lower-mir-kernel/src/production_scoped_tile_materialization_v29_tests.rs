// Private Phase1b source-bound simulation and resource regressions.
// The source fixtures are admitted inert semantic source, not rustc authentication.
// These tests exercise private candidate custody, not hardware or a public path.
use super::*;
use fe2o3_kernel_ir as kir;
use fe2o3_kir_sim as sim;
use std::collections::{BTreeMap, BTreeSet};

include!("production_scoped_tile_simulation_v29_tests.rs");

#[path = "production_scoped_tile_negative_v29_tests.rs"]
mod negative_v29_tests;
#[path = "production_scoped_tile_projection_v29_tests.rs"]
mod projection_v29_tests;

#[test]
fn borrowed_planner_census_and_raw_id_boundaries() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let prepared = prepared_source(
        SourceCase::Shifted,
        ScopedTileOrderV29::Blocked,
        &mut budget,
    );
    max_id_graph_boundary_tests::assert_planner_boundaries(&prepared, &mut budget);
    drop_prepared(prepared, &mut budget);
    assert_eq!(budget.storage(), SCHEDULE_FLOOR);
    budget.release_storage(SCHEDULE_FLOOR).unwrap();
}

fn scalar_candidate(
    case: SourceCase,
    order: ScopedTileOrderV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> ScopedTileScalarCandidateV29 {
    let prepared = prepared_source(case, order, budget);
    let policy = prepared.pending.inner.limits.storage_layout_limits();
    let candidate = materialize_scoped_tile_source_v29(prepared, budget)
        .unwrap_or_else(|failure| panic!("materialization: {:?}", failure.summary));
    let _: &kir::VerifiedCanonicalKernelIrModuleV18 = &candidate.output;
    assert_eq!(
        candidate.output.module().storage_layouts,
        candidate.input.pending.pending_module().storage_layouts
    );
    assert_eq!(
        candidate.input.pending.inner.limits.storage_layout_limits(),
        policy
    );
    candidate
}

fn drop_scalar_candidate(
    candidate: ScopedTileScalarCandidateV29,
    budget: &mut ArgumentBudgetV1<'_>,
) {
    let retained = candidate.adopted_storage();
    drop(candidate);
    budget.release_storage(retained).unwrap();
}

#[test]
fn genuine_source_candidates_match_independent_scalar_oracle() {
    for case in [
        SourceCase::Repeated,
        SourceCase::Slots,
        SourceCase::Shifted,
        SourceCase::Discard,
        SourceCase::BranchParts,
    ] {
        for order in [ScopedTileOrderV29::Blocked, ScopedTileOrderV29::Striped] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
            budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
            let candidate = scalar_candidate(case, order, &mut budget);
            let floor = budget.storage();
            candidate.replay_with_budget(&mut budget).unwrap();
            assert_eq!(budget.storage(), floor);
            for (length, prefix, groups, initial_seed) in [
                (0, 0, 1, 0),
                (1, 3, 1, 3),
                (65, 3, 2, 0),
                (128, 0, 2, 3),
                (131, 3, 1, 0),
            ] {
                assert_source_simulation(
                    &candidate,
                    case,
                    order,
                    ScalarCase {
                        elements: 2,
                        lanes: 64,
                        base: 0,
                        length,
                        prefix,
                        groups,
                        initial_seed,
                    },
                );
            }
            drop_scalar_candidate(candidate, &mut budget);
            assert_eq!(budget.storage(), SCHEDULE_FLOOR);
            budget.release_storage(SCHEDULE_FLOOR).unwrap();
        }
    }
}

// Geometry and base are changed before semantic readmission, never in candidate KIR.
fn configured_source_owner(
    case: SourceCase,
    lanes: u16,
    base: u64,
) -> ProductionSemanticSsaOwnerV1 {
    let template = source_owner(case);
    let semantic = template.source_semantic();
    let mut types = semantic.types().to_vec();
    for ty in &mut types {
        let replacement = match ty.rust_type_kind() {
            SemanticRustTypeKindV1::Execution(SemanticExecutionRoleV29::MaskedTileU32 {
                elements,
                ..
            }) => {
                assert_eq!(elements, 2);
                Some(SemanticExecutionRoleV29::MaskedTileU32 { lanes, elements })
            }
            SemanticRustTypeKindV1::Execution(SemanticExecutionRoleV29::LaneFragmentU32 {
                elements,
                ..
            }) => {
                assert_eq!(elements, 2);
                Some(SemanticExecutionRoleV29::LaneFragmentU32 { lanes, elements })
            }
            _ => None,
        };
        if let Some(role) = replacement {
            *ty = ty
                .clone()
                .with_rust_type_kind(SemanticRustTypeKindV1::Execution(role));
        }
    }
    let mut functions = semantic.functions().to_vec();
    let mut load_calls = 0;
    for function in &mut functions {
        let mut blocks = function.blocks().to_vec();
        for block in &mut blocks {
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            if !matches!(
                &semantic.callables()[call.callee().index() as usize],
                SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::Execution(
                        SemanticExecutionOperationV29::MaskedTileLoadU32 { .. }
                    ),
                    ..
                }
            ) {
                continue;
            }
            let mut arguments = call.arguments().to_vec();
            let SemanticOperandV1::Constant(constant) = &arguments[2] else {
                panic!("fixture load base is a source constant");
            };
            arguments[2] = SemanticOperandV1::Constant(SemanticConstantV1::new(
                constant.ty(),
                SemanticConstantValueV1::Scalar(
                    SemanticScalarValueV1::new(u128::from(base), 8).unwrap(),
                ),
            ));
            let call = SemanticDirectCallV1::new_callable(
                call.callee(),
                arguments,
                call.destination().cloned(),
                call.unwind(),
            )
            .unwrap();
            *block = SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                block.statements().to_vec(),
                SemanticTerminatorV1::new(
                    block.terminator().source(),
                    SemanticTerminatorKindV1::Call(call),
                ),
            )
            .unwrap();
            load_calls += 1;
        }
        let mut replacement = SemanticFunctionDeclV1::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            function.source(),
            function.abi().clone(),
            function.locals().to_vec(),
            function.entry(),
            blocks,
        )
        .unwrap();
        if let Some(entry) = function.kernel_entry() {
            let contract = entry.source_contract();
            let dimensions = SemanticWorkgroupDimensionsV1::new([u32::from(lanes), 1, 1]).unwrap();
            let launch = contract.launch().map(|bounds| {
                SemanticKernelLaunchBoundsV1::new(
                    bounds.required().map(|_| dimensions),
                    bounds.maximum().map(|_| dimensions),
                    bounds.min_workgroups_per_compute_unit(),
                )
                .unwrap()
            });
            replacement = replacement.with_kernel_entry(SemanticKernelEntryV1::new(
                entry.export_symbol().clone(),
                entry.kernel_binding_identity(),
                SemanticKernelSourceContractV1::new_with_resources(
                    launch,
                    contract.resources(),
                    contract.unsafe_assembly(),
                    contract.reachable_assembly(),
                )
                .unwrap(),
            ));
        }
        *function = replacement;
    }
    assert!(load_calls > 0);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
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

#[derive(Clone, Copy, Debug)]
enum ProjectionSourceVariant {
    Assertion,
    LifetimeKills,
}

fn variant_candidate(
    variant: ProjectionSourceVariant,
    budget: &mut ArgumentBudgetV1<'_>,
) -> ScopedTileScalarCandidateV29 {
    scalar_candidate_from_source_with_context_v29(
        || projection_source_owner(variant),
        ScopedTileOrderV29::Blocked,
        64,
        budget,
        variant,
    )
}

fn projection_source_owner(variant: ProjectionSourceVariant) -> ProductionSemanticSsaOwnerV1 {
    projection_source_owner_with_loan_end(variant, true, false)
}

fn projection_source_owner_with_loan_end(
    variant: ProjectionSourceVariant,
    end_loan: bool,
    future_use: bool,
) -> ProductionSemanticSsaOwnerV1 {
    let template = source_owner(SourceCase::Slots);
    let semantic = template.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let index = match variant {
        ProjectionSourceVariant::Assertion => 0,
        ProjectionSourceVariant::LifetimeKills => 3,
    };
    let selected = functions[index].clone();
    let mut locals = selected.locals().to_vec();
    let mut blocks = selected.blocks().to_vec();
    match variant {
        ProjectionSourceVariant::Assertion => {
            let boolean = semantic
                .types()
                .iter()
                .position(|ty| {
                    matches!(
                        ty.shape(),
                        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
                    )
                })
                .map(|index| SemanticTypeIdV1::from_index(index as u32))
                .unwrap();
            assert_eq!(blocks.len(), 3);
            // Keep the context/provider/return region unchanged. Both dynamic
            // root assertions precede context issuance.
            for (site, tag) in [(0_usize, 216_u8), (3, 217)] {
                let original = blocks[site].clone();
                let condition_local = locals.len() as u32;
                locals.push(local(tag, boolean, SemanticLocalRoleV1::Temporary));
                let mut statements = original.statements().to_vec();
                statements.push(assign(
                    place(condition_local, boolean),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::NotEqual,
                        left: SemanticOperandV1::Copy(place(1, U32)),
                        right: scalar(0),
                    },
                ));
                let target = SemanticBlockIdV1::from_index(blocks.len() as u32);
                blocks.push(block(tag, vec![], original.terminator().kind().clone()));
                blocks[site] = SemanticBasicBlockV1::new(
                    original.identity(),
                    original.source(),
                    statements,
                    SemanticTerminatorV1::new(
                        original.terminator().source(),
                        SemanticTerminatorKindV1::Assert {
                            condition: SemanticOperandV1::Copy(place(condition_local, boolean)),
                            expected: true,
                            message: SemanticAssertMessageV1::DivisionByZero(
                                SemanticOperandV1::Copy(place(1, U32)),
                            ),
                            target: SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::AssertSuccess,
                                target,
                            ),
                            unwind: SemanticUnwindActionV1::Unreachable,
                        },
                    ),
                )
                .unwrap();
            }
        }
        ProjectionSourceVariant::LifetimeKills => {
            // Nominal Fragment transport emits no ordinary argument preparation.
            let mut statements = blocks[1].statements().to_vec();
            if end_loan {
                // Local 8 holds the shared borrow of local 7 created in entry.
                statements.push(SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(8)),
                ));
            }
            statements.extend([
                SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(7)),
                ),
                SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(7)),
                ),
            ]);
            if future_use {
                assert!(!end_loan);
                statements.push(assign(
                    place(9, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                        SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(8),
                            vec![
                                SemanticProjectionV1::new(
                                    SemanticProjectionKindV1::Dereference,
                                    U32,
                                )
                                .unwrap(),
                            ],
                            U32,
                        )
                        .unwrap(),
                    )),
                ));
            }
            blocks[1] = SemanticBasicBlockV1::new(
                blocks[1].identity(),
                blocks[1].source(),
                statements,
                blocks[1].terminator().clone(),
            )
            .unwrap();
        }
    }
    functions[index] =
        replace_extra_function_v29(&selected, selected.abi().clone(), locals, blocks);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
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

fn configured_scalar_candidate(
    case: SourceCase,
    order: ScopedTileOrderV29,
    lanes: u16,
    base: u64,
    budget: &mut ArgumentBudgetV1<'_>,
) -> ScopedTileScalarCandidateV29 {
    scalar_candidate_from_source_with_context_v29(
        || configured_source_owner(case, lanes, base),
        order,
        lanes,
        budget,
        (case, order, lanes, base),
    )
}

#[test]
fn lifetime_kill_source_refuses_live_reference_before_materialization() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let error = scalar_pending_from_source_v29(
        || {
            projection_source_owner_with_loan_end(
                ProjectionSourceVariant::LifetimeKills,
                false,
                true,
            )
        },
        64,
        &mut budget,
    )
    .err()
    .expect("referent storage death with a live holder was admitted");
    assert!(
        matches!(
            error,
            ProductionPendingScopedSourceErrorV29::Source(
                ProductionSemanticKirErrorV1::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail: "source reference referent storage dies with a live loan",
                }
            )
        ),
        "{error:?}"
    );
    assert_eq!(budget.storage(), SCHEDULE_FLOOR);
    budget.release_storage(SCHEDULE_FLOOR).unwrap();
}

#[test]
fn lifetime_kill_source_preserves_completed_statement_holder_expiry() {
    for end_loan in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
        let candidate = scalar_candidate_from_source_v29(
            || {
                projection_source_owner_with_loan_end(
                    ProjectionSourceVariant::LifetimeKills,
                    end_loan,
                    false,
                )
            },
            ScopedTileOrderV29::Blocked,
            64,
            &mut budget,
        );
        let retained = budget.storage();
        candidate.replay_with_budget(&mut budget).unwrap();
        assert_eq!(budget.storage(), retained);
        drop_scalar_candidate(candidate, &mut budget);
        assert_eq!(budget.storage(), SCHEDULE_FLOOR);
        budget.release_storage(SCHEDULE_FLOOR).unwrap();
    }
}

fn scalar_candidate_from_source_v29(
    owner_factory: impl Fn() -> ProductionSemanticSsaOwnerV1,
    order: ScopedTileOrderV29,
    lanes: u16,
    budget: &mut ArgumentBudgetV1<'_>,
) -> ScopedTileScalarCandidateV29 {
    scalar_candidate_from_source_with_context_v29(
        owner_factory,
        order,
        lanes,
        budget,
        "scalar source",
    )
}

fn scalar_candidate_from_source_with_context_v29(
    owner_factory: impl Fn() -> ProductionSemanticSsaOwnerV1,
    order: ScopedTileOrderV29,
    lanes: u16,
    budget: &mut ArgumentBudgetV1<'_>,
    context: impl std::fmt::Debug,
) -> ScopedTileScalarCandidateV29 {
    let pending = scalar_pending_from_source_with_profile_v29(owner_factory, lanes, true, budget)
        .unwrap_or_else(|error| panic!("{context:?}: pending source admission: {error:?}"));
    let mut donor = Some((pending, ScopedTileScheduleInputV29 { order }));
    let prepared = prepare_scoped_tile_source_v29(&mut donor, budget)
        .unwrap_or_else(|error| panic!("{context:?}: schedule preparation: {error:?}"));
    assert!(
        donor.is_none(),
        "{context:?}: schedule preparation retained its donor"
    );
    materialize_scoped_tile_source_v29(prepared, budget).unwrap_or_else(|failure| {
        panic!("{context:?}: scalar materialization: {:?}", failure.summary)
    })
}

fn scalar_pending_from_source_v29(
    owner_factory: impl Fn() -> ProductionSemanticSsaOwnerV1,
    lanes: u16,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ProductionPendingScopedSourceOwnerV29, ProductionPendingScopedSourceErrorV29> {
    scalar_pending_from_source_with_profile_v29(owner_factory, lanes, true, budget)
}

fn scalar_pending_from_source_with_profile_v29(
    owner_factory: impl Fn() -> ProductionSemanticSsaOwnerV1,
    lanes: u16,
    profiled: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ProductionPendingScopedSourceOwnerV29, ProductionPendingScopedSourceErrorV29> {
    use kernel_argument_abi_v18::tests::{FixtureKernelAbiV18, fixture_descriptor_ownership_v18};
    let floor = budget.storage();
    let projected = fixture_descriptor_ownership_v18(owner_factory());
    let owner = fixture_descriptor_ownership_v18(owner_factory());
    let launch = ProductionSourceLaunchRosterV1::try_new(
        owner.source_semantic(),
        &[ProductionSourceLaunchRootInputV1::new(
            "lifecycle_fixture",
            [88; 32],
            ProductionSourceLaunchInputV1::new(1, Some([u32::from(lanes), 1, 1]), [2, 1, 1]),
        )],
    )
    .unwrap();
    let semantic = projected.source_semantic();
    let roots = [root_input(&projected)];
    let workgroup = semantic.functions()[2].abi().source_input_types()[0];
    let mut classes =
        vec![ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
    classes[1] = ProductionScopeCallableCandidateV29::Provider {
        function: HELPER,
        identity: semantic.functions()[1].identity(),
    };
    let SemanticTerminatorKindV1::Call(derive) =
        semantic.functions()[1].blocks()[0].terminator().kind()
    else {
        panic!("source derive call");
    };
    classes[derive.callee().index() as usize] = ProductionScopeCallableCandidateV29::Derive {
        binding: SemanticFunctionIdentityV1::from_sha256([121; 32]),
        operation: SemanticCompilerIntrinsicIdentityV1::from_sha256([121; 32]),
        context: CONTEXT,
        workgroup,
    };
    let events = [
        (
            ROOT,
            1,
            0,
            ProductionScopeEventKindV29::Call {
                callee: SemanticCallableIdV1::from_index(1),
                kind: ProductionScopeCallKindV29::Provider,
            },
        ),
        (
            HELPER,
            0,
            semantic.functions()[1].blocks()[0].statements().len(),
            ProductionScopeEventKindV29::Call {
                callee: derive.callee(),
                kind: ProductionScopeCallKindV29::Derive,
            },
        ),
        (
            HELPER,
            1,
            0,
            ProductionScopeEventKindV29::Call {
                callee: SemanticCallableIdV1::from_index(2),
                kind: ProductionScopeCallKindV29::Ordinary,
            },
        ),
        (HELPER, 2, 0, ProductionScopeEventKindV29::Return),
    ]
    .map(
        |(function, block, statement_count, kind)| ProductionScopeEventCandidateV29 {
            function,
            block: SemanticBlockIdV1::from_index(block),
            statement_count,
            kind,
        },
    );
    let input = ProductionExecutionSourceInputV29 {
        semantic_sha256: projected.source_semantic_sha256(),
        roots: &roots,
        classes: &classes,
        events: &events,
    };
    let result = if profiled {
        let profile = FixtureKernelAbiV18::new(&owner);
        let roots = profile.roots();
        ProductionPendingScopedSourceOwnerV29::try_materialize_with_kernel_abi_budget_v18(
            owner,
            launch,
            input,
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            ProductionSemanticKirLimitsV1::default(),
            budget,
        )
    } else {
        ProductionPendingScopedSourceOwnerV29::try_materialize_with_budget(
            owner,
            launch,
            input,
            ProductionSemanticKirLimitsV1::default(),
            budget,
        )
    };
    let retained = result
        .as_ref()
        .map_or(0, |pending| pending.adopted_storage());
    assert_eq!(budget.storage(), floor + retained);
    result
}

#[test]
fn genuine_source_lane_geometry_and_index_boundaries_match_cpu_oracle() {
    for case in [SourceCase::Repeated, SourceCase::Shifted] {
        for order in [ScopedTileOrderV29::Blocked, ScopedTileOrderV29::Striped] {
            for (lanes, base, length) in [
                (3, 0, 65),
                (128, 0, 131),
                (64, 7, 65),
                (64, 65, 65),
                (64, u64::MAX, 131),
                (64, u64::MAX - 1, 131),
            ] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
                let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
                budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
                let candidate = configured_scalar_candidate(case, order, lanes, base, &mut budget);
                candidate.replay_with_budget(&mut budget).unwrap();
                assert_source_simulation(
                    &candidate,
                    case,
                    order,
                    ScalarCase {
                        elements: 2,
                        lanes,
                        base,
                        length,
                        prefix: 3,
                        groups: 2,
                        initial_seed: 3,
                    },
                );
                drop_scalar_candidate(candidate, &mut budget);
                assert_eq!(budget.storage(), SCHEDULE_FLOOR);
                budget.release_storage(SCHEDULE_FLOOR).unwrap();
            }
        }
    }
}

#[derive(Debug)]
struct ScalarProbe {
    result: Result<(), ScopedTileFailureKindV29>,
    work: usize,
    extra: usize,
    denied_work: Option<usize>,
    denied_storage: Option<usize>,
}

fn scalar_probe(preexisting: bool, replay: bool, allowance: Option<(usize, usize)>) -> ScalarProbe {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let (result, used, extra, denied_storage) = {
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        let (pending, _) = pending_source(SourceCase::Shifted, preexisting, None, &mut budget);
        let mut donor = Some((
            pending,
            ScopedTileScheduleInputV29 {
                order: ScopedTileOrderV29::Blocked,
            },
        ));
        let mut prepared = Some(prepare_scoped_tile_source_v29(&mut donor, &mut budget).unwrap());
        let candidate = replay.then(|| {
            materialize_scoped_tile_source_v29(prepared.take().unwrap(), &mut budget)
                .unwrap_or_else(|failure| panic!("baseline materialization: {:?}", failure.summary))
        });
        let original = candidate.as_ref().map_or_else(
            || prepared.as_ref().unwrap().adopted_storage(),
            ScopedTileScalarCandidateV29::adopted_storage,
        );
        budget
            .reserve_storage(budget.peak_storage() + 1 - budget.storage())
            .unwrap();
        if let Some((work_left, storage_left)) = allowance {
            budget
                .charge_work(SCHEDULE_LIMIT - budget.work() - work_left)
                .unwrap();
            budget
                .reserve_storage(SCHEDULE_LIMIT - budget.storage() - storage_left)
                .unwrap();
        }
        let entry = budget.storage();
        let before = budget.work();
        let result = if let Some(candidate) = candidate {
            let result = candidate.replay_with_budget(&mut budget);
            assert_eq!(budget.storage(), entry);
            drop_scalar_candidate(candidate, &mut budget);
            result
        } else {
            let input = prepared.take().unwrap();
            let graph = *input.pending.pending_identity();
            let identity = input.identity;
            let functions = input.pending.pending_module().functions.as_ptr();
            let selections = input.selections.as_ptr();
            match materialize_scoped_tile_source_v29(input, &mut budget) {
                Ok(candidate) => {
                    assert_eq!(candidate.input.identity, identity);
                    assert_eq!(candidate.input.pending.pending_identity(), &graph);
                    assert_eq!(
                        candidate.input.pending.pending_module().functions.as_ptr(),
                        functions
                    );
                    assert_eq!(candidate.input.selections.as_ptr(), selections);
                    assert_eq!(
                        budget.storage(),
                        entry + candidate.adopted_storage() - original
                    );
                    drop_scalar_candidate(candidate, &mut budget);
                    Ok(())
                }
                Err(failure) => {
                    let kind = failure.summary.kind;
                    assert_eq!(budget.storage(), entry);
                    let recovered = failure.into_input();
                    assert_eq!(budget.storage(), entry, "recovery cannot refund live input");
                    assert_eq!(recovered.identity, identity);
                    assert_eq!(recovered.pending.pending_identity(), &graph);
                    assert_eq!(
                        recovered.pending.pending_module().functions.as_ptr(),
                        functions
                    );
                    assert_eq!(recovered.selections.as_ptr(), selections);
                    drop_prepared(recovered, &mut budget);
                    Err(kind)
                }
            }
        };
        assert_eq!(budget.storage(), entry - original);
        let used = budget.work() - before;
        let extra = budget.peak_storage() - entry;
        let denied_storage = budget.failed_storage();
        let remaining = budget.storage();
        budget.release_storage(remaining).unwrap();
        (result, used, extra, denied_storage)
    };
    ScalarProbe {
        result,
        work: used,
        extra,
        denied_work: work.failed_work(),
        denied_storage,
    }
}

#[test]
fn materialization_and_correspondence_have_exact_resource_boundaries() {
    for preexisting in [false, true] {
        for replay in [false, true] {
            let baseline = scalar_probe(preexisting, replay, None);
            baseline.result.unwrap();
            assert!(baseline.work > 0 && baseline.extra > 0);
            let exact = scalar_probe(preexisting, replay, Some((baseline.work, baseline.extra)));
            exact.result.unwrap();
            assert_eq!(exact.work, baseline.work);
            assert_eq!(exact.extra, baseline.extra);
            let work = scalar_probe(
                preexisting,
                replay,
                Some((baseline.work - 1, baseline.extra)),
            );
            let ScopedTileFailureKindV29::Resource(ArgumentResourceV1::Work(limit)) =
                work.result.unwrap_err()
            else {
                panic!("typed work refusal: {work:?}");
            };
            assert_eq!(work.denied_work, Some(limit.actual()));
            let storage = scalar_probe(
                preexisting,
                replay,
                Some((baseline.work, baseline.extra - 1)),
            );
            let ScopedTileFailureKindV29::Resource(ArgumentResourceV1::Storage(limit)) =
                storage.result.unwrap_err()
            else {
                panic!("typed storage refusal: {storage:?}");
            };
            assert_eq!(storage.denied_storage, Some(limit.actual()));
        }
    }
}

#[test]
fn materialization_refuses_foreign_ledger_and_returns_same_owner() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let prepared = prepared_source(
        SourceCase::Repeated,
        ScopedTileOrderV29::Blocked,
        &mut budget,
    );
    let identity = prepared.identity;
    let functions = prepared.pending.pending_module().functions.as_ptr();
    let selections = prepared.selections.as_ptr();
    let floor = budget.storage();
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, SCHEDULE_LIMIT);
    foreign.reserve_storage(floor).unwrap();
    let failure = match materialize_scoped_tile_source_v29(prepared, &mut foreign) {
        Ok(_) => panic!("foreign ledger admitted"),
        Err(failure) => failure,
    };
    assert_eq!(
        failure.summary.kind,
        ScopedTileFailureKindV29::Resource(ArgumentResourceV1::Accounting)
    );
    assert_eq!(foreign.work(), 0);
    assert_eq!(foreign.storage(), floor);
    assert_eq!(budget.storage(), floor);
    let recovered = failure.into_input();
    assert_eq!(recovered.identity, identity);
    assert_eq!(
        recovered.pending.pending_module().functions.as_ptr(),
        functions
    );
    assert_eq!(recovered.selections.as_ptr(), selections);
    let candidate = materialize_scoped_tile_source_v29(recovered, &mut budget)
        .unwrap_or_else(|failure| panic!("recovered input: {:?}", failure.summary));
    assert_eq!(
        candidate.replay_with_budget(&mut foreign),
        Err(ScopedTileFailureKindV29::Resource(
            ArgumentResourceV1::Accounting
        ))
    );
    assert_eq!(foreign.work(), 0);
    assert_eq!(foreign.storage(), floor);
    candidate.replay_with_budget(&mut budget).unwrap();
    drop_scalar_candidate(candidate, &mut budget);
    assert_eq!(budget.storage(), SCHEDULE_FLOOR);
    budget.release_storage(SCHEDULE_FLOOR).unwrap();
    foreign.release_storage(floor).unwrap();
}

#[test]
fn materialization_with_no_work_left_keeps_recovery_owner_live() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let prepared = prepared_source(
        SourceCase::Shifted,
        ScopedTileOrderV29::Striped,
        &mut budget,
    );
    let identity = prepared.identity;
    let functions = prepared.pending.pending_module().functions.as_ptr();
    let selections = prepared.selections.as_ptr();
    let floor = budget.storage();
    budget.charge_work(SCHEDULE_LIMIT - budget.work()).unwrap();
    let failure = match materialize_scoped_tile_source_v29(prepared, &mut budget) {
        Ok(_) => panic!("zero-work materialization admitted"),
        Err(failure) => failure,
    };
    assert!(matches!(
        failure.summary.kind,
        ScopedTileFailureKindV29::Resource(ArgumentResourceV1::Work(_))
    ));
    assert_eq!(budget.storage(), floor);
    let recovered = failure.into_input();
    assert_eq!(budget.storage(), floor);
    assert_eq!(recovered.identity, identity);
    assert_eq!(
        recovered.pending.pending_module().functions.as_ptr(),
        functions
    );
    assert_eq!(recovered.selections.as_ptr(), selections);
    drop_prepared(recovered, &mut budget);
    assert_eq!(budget.storage(), SCHEDULE_FLOOR);
    budget.release_storage(SCHEDULE_FLOOR).unwrap();
}

fn assert_scalar_mismatch(
    candidate: &ScopedTileScalarCandidateV29,
    budget: &mut ArgumentBudgetV1<'_>,
) {
    let floor = budget.storage();
    assert_eq!(
        candidate.replay_with_budget(budget),
        Err(ScopedTileFailureKindV29::ReplayMismatch)
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn correspondence_rejects_reordered_or_missing_origin_and_piece_rows() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let mut candidate = scalar_candidate(
        SourceCase::Shifted,
        ScopedTileOrderV29::Blocked,
        &mut budget,
    );
    assert!(candidate.relations.origins.len() > 1);
    candidate.relations.origins.swap(0, 1);
    assert_scalar_mismatch(&candidate, &mut budget);
    candidate.relations.origins.swap(0, 1);
    let origin = candidate.relations.origins.pop().unwrap();
    assert_scalar_mismatch(&candidate, &mut budget);
    candidate.relations.origins.push(origin);
    assert!(candidate.relations.pieces.len() > 1);
    candidate.relations.pieces.swap(0, 1);
    assert_scalar_mismatch(&candidate, &mut budget);
    candidate.relations.pieces.swap(0, 1);
    let piece = candidate.relations.pieces.pop().unwrap();
    assert_scalar_mismatch(&candidate, &mut budget);
    candidate.relations.pieces.push(piece);
    candidate.replay_with_budget(&mut budget).unwrap();
    drop_scalar_candidate(candidate, &mut budget);
    assert_eq!(budget.storage(), SCHEDULE_FLOOR);
    budget.release_storage(SCHEDULE_FLOOR).unwrap();
}

#[test]
fn correspondence_rejects_independently_verified_wrong_schedule_graphs() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let mut blocked = scalar_candidate(
        SourceCase::Repeated,
        ScopedTileOrderV29::Blocked,
        &mut budget,
    );
    let mut striped = scalar_candidate(
        SourceCase::Repeated,
        ScopedTileOrderV29::Striped,
        &mut budget,
    );
    assert_ne!(
        blocked.output.canonical_bytes(),
        striped.output.canonical_bytes()
    );
    // Both payloads retain V18 verification. Only source correspondence is false.
    assert_eq!(blocked.output_storage, striped.output_storage);
    std::mem::swap(&mut blocked.output, &mut striped.output);
    assert_scalar_mismatch(&blocked, &mut budget);
    assert_scalar_mismatch(&striped, &mut budget);
    std::mem::swap(&mut blocked.output, &mut striped.output);
    blocked.replay_with_budget(&mut budget).unwrap();
    striped.replay_with_budget(&mut budget).unwrap();
    drop_scalar_candidate(blocked, &mut budget);
    drop_scalar_candidate(striped, &mut budget);
    assert_eq!(budget.storage(), SCHEDULE_FLOOR);
    budget.release_storage(SCHEDULE_FLOOR).unwrap();
}

#[cfg(test)]
mod materialization_fault_tests {
    use super::*;
    use tile_materialization_faults_v29 as faults;

    struct FaultArm;
    impl Drop for FaultArm {
        fn drop(&mut self) {
            faults::ARMED.with(|slot| {
                slot.replace(None);
            });
            faults::SEEN.set(None);
        }
    }
    fn arm(point: faults::Point, payload: Box<dyn std::any::Any + Send>) -> FaultArm {
        faults::ARMED.with(|slot| {
            assert!(
                slot.replace(Some((point, payload))).is_none(),
                "nested fault hook"
            );
        });
        faults::SEEN.set(None);
        FaultArm
    }
    fn selected_history() -> faults::History {
        assert!(
            faults::ARMED.with(|slot| slot.borrow().is_none()),
            "checkpoint was not reached"
        );
        faults::SEEN.get().expect("selected checkpoint history")
    }

    // Preexisting capture and unrelated reservations are deliberately not folded
    // into input.adopted_storage(). They must survive a consumed-input panic.
    fn fault_input(
        preexisting: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> (PreparedScopedTileSourceV29, usize) {
        const UNRELATED: usize = 113;
        budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
        let (pending, capture) = pending_source(SourceCase::Shifted, preexisting, None, budget);
        assert_eq!(capture > 0, preexisting);
        let mut donor = Some((
            pending,
            ScopedTileScheduleInputV29 {
                order: ScopedTileOrderV29::Striped,
            },
        ));
        let input = prepare_scoped_tile_source_v29(&mut donor, budget).unwrap();
        assert!(donor.is_none());
        budget.reserve_storage(UNRELATED).unwrap();
        let unrelated = SCHEDULE_FLOOR + capture + UNRELATED;
        assert_eq!(budget.storage(), unrelated + input.adopted_storage());
        (input, unrelated)
    }

    #[test]
    fn consumed_input_panics_keep_exact_payload_and_unrelated_ledger_history() {
        use faults::Point::*;
        for point in [CopyAdopted, Emitted, CanonicalAdopted, Projected] {
            for preexisting in [false, true] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
                let denied_work = SCHEDULE_LIMIT + 7;
                assert!(work.charge_work(denied_work).is_err());
                {
                    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
                    let (input, unrelated) = fault_input(preexisting, &mut budget);
                    let entry = budget.storage();
                    let adopted = input.adopted_storage();
                    let before = budget.work();
                    let ledger = budget.work_ledger_identity_v1();
                    assert!(budget.reserve_storage(SCHEDULE_LIMIT + 11).is_err());
                    let denied_storage = budget.failed_storage();
                    let payload = Box::new([0x5a_u8; 17]);
                    let payload_address = (&*payload) as *const [u8; 17];
                    let _arm = arm(point, payload);
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        materialize_scoped_tile_source_v29(input, &mut budget)
                    }));
                    let payload = match result {
                        Err(payload) => payload,
                        Ok(_) => panic!("checkpoint did not unwind: {point:?}"),
                    };
                    let payload = payload.downcast::<[u8; 17]>().unwrap();
                    assert_eq!((&*payload) as *const [u8; 17], payload_address);
                    assert_eq!(*payload, [0x5a; 17]);
                    let seen = selected_history();
                    assert!(seen.0 > before && seen.1 > entry);
                    assert_eq!(budget.work(), seen.0);
                    assert_eq!(budget.peak_storage(), seen.2);
                    assert_eq!(budget.failed_storage(), denied_storage);
                    assert_eq!(seen.3, denied_storage);
                    assert!(budget.work_ledger_identity_v1() == ledger);
                    assert_eq!(budget.storage(), entry - adopted);
                    assert_eq!(budget.storage(), unrelated);
                    budget.release_storage(unrelated).unwrap();
                    assert_eq!(budget.storage(), 0);
                }
                assert_eq!(work.failed_work(), Some(denied_work));
            }
        }
    }

    #[test]
    fn successful_api_transfer_denials_return_the_same_live_input() {
        use faults::Point::*;
        for point in [CopyTransfer, CanonicalTransfer] {
            for preexisting in [false, true] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
                let denied_work = SCHEDULE_LIMIT + 7;
                assert!(work.charge_work(denied_work).is_err());
                {
                    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
                    let (input, unrelated) = fault_input(preexisting, &mut budget);
                    let entry = budget.storage();
                    let adopted = input.adopted_storage();
                    let identity = input.identity;
                    let graph = *input.pending.pending_identity();
                    let functions = input.pending.pending_module().functions.as_ptr();
                    let selections = input.selections.as_ptr();
                    let ledger = budget.work_ledger_identity_v1();
                    if preexisting {
                        assert!(budget.reserve_storage(SCHEDULE_LIMIT + 11).is_err());
                    }
                    let prior_denial = budget.failed_storage();
                    let expected_denial = prior_denial.or(Some(SCHEDULE_LIMIT + 1));
                    let _arm = arm(point, Box::new(()));
                    let failure = match materialize_scoped_tile_source_v29(input, &mut budget) {
                        Err(failure) => failure,
                        Ok(_) => panic!("injected transfer denial did not fail: {point:?}"),
                    };
                    assert_eq!(
                        failure.summary.phase,
                        ScopedTileFailurePhaseV29::Materialization
                    );
                    let ScopedTileFailureKindV29::Resource(ArgumentResourceV1::Storage(limit)) =
                        failure.summary.kind
                    else {
                        panic!("wrong transfer refusal");
                    };
                    assert_eq!(limit.actual(), SCHEDULE_LIMIT + 1);
                    assert_eq!(limit.limit(), SCHEDULE_LIMIT);
                    let seen = selected_history();
                    assert_eq!(budget.work(), seen.0);
                    assert_eq!(budget.peak_storage(), seen.2);
                    assert_eq!(seen.3, prior_denial);
                    assert_eq!(budget.failed_storage(), expected_denial);
                    assert!(budget.work_ledger_identity_v1() == ledger);
                    assert_eq!(budget.storage(), entry);
                    let recovered = failure.into_input();
                    assert_eq!(budget.storage(), entry);
                    assert_eq!(recovered.adopted_storage(), adopted);
                    assert_eq!(recovered.identity, identity);
                    assert_eq!(*recovered.pending.pending_identity(), graph);
                    assert_eq!(
                        recovered.pending.pending_module().functions.as_ptr(),
                        functions
                    );
                    assert_eq!(recovered.selections.as_ptr(), selections);
                    let candidate = materialize_scoped_tile_source_v29(recovered, &mut budget)
                        .unwrap_or_else(|failure| {
                            panic!("one-shot recovery: {:?}", failure.summary)
                        });
                    candidate.replay_with_budget(&mut budget).unwrap();
                    assert_eq!(budget.failed_storage(), expected_denial);
                    drop_scalar_candidate(candidate, &mut budget);
                    assert_eq!(budget.storage(), unrelated);
                    budget.release_storage(unrelated).unwrap();
                }
                assert_eq!(work.failed_work(), Some(denied_work));
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ExtraSourceWitnessV29 {
    IgnoredUnit,
    ElidedBounds,
}

fn replace_extra_function_v29(
    prior: &SemanticFunctionDeclV1,
    abi: SemanticFunctionAbiV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let replacement = SemanticFunctionDeclV1::new(
        prior.identity(),
        prior.role(),
        prior.item_definition_identity(),
        prior.monomorphization_identity(),
        prior.generic_type_arguments_identity(),
        prior.const_generic_arguments_identity(),
        prior.source(),
        abi,
        locals,
        prior.entry(),
        blocks,
    )
    .unwrap();
    match prior.kernel_entry() {
        Some(entry) => replacement.with_kernel_entry(entry.clone()),
        None => replacement,
    }
}

fn extra_source_owner_v29(variant: ExtraSourceWitnessV29) -> ProductionSemanticSsaOwnerV1 {
    let template = source_owner(SourceCase::Slots);
    let semantic = template.source_semantic();
    let mut functions = semantic.functions().to_vec();
    match variant {
        ExtraSourceWitnessV29::IgnoredUnit => {
            // Scoped child plans use nominal parameter custody and retain no
            // ignored-binding rows. Root ABI planning owns these real records.
            let root = &functions[0];
            let prior = root.abi();
            let mut arguments = prior.arguments().to_vec();
            let mut ownership = prior.source_argument_ownership().to_vec();
            let mut locals = root.locals().to_vec();
            for offset in 0..2 {
                arguments.push(SemanticAbiArgumentV1::source(ignored(UNIT)));
                ownership.push(SemanticSourceArgumentOwnershipV1::ByValue);
                locals.push(local(
                    218 + offset as u8,
                    UNIT,
                    SemanticLocalRoleV1::Argument(prior.source_input_types().len() as u32 + offset),
                ));
            }
            let abi = SemanticFunctionAbiV1::from_rustc(
                SemanticAbiIdentityV1::from_sha256([218; 32]),
                prior.layout_identity(),
                prior.canon_abi(),
                prior.extern_abi(),
                prior.c_variadic(),
                prior.can_unwind(),
                prior.fixed_count() + 2,
                arguments,
                prior.return_value().clone(),
            )
            .unwrap()
            .with_source_argument_ownership(ownership)
            .unwrap();
            functions[0] = replace_extra_function_v29(root, abi, locals, root.blocks().to_vec());
        }
        ExtraSourceWitnessV29::ElidedBounds => {
            let workgroup = functions[2].abi().source_input_types()[0];
            let SemanticTypeShapeV1::Aggregate(fields) =
                semantic.types()[workgroup.index() as usize].shape()
            else {
                panic!("Workgroup source fields");
            };
            let index_ty = fields.fields()[0];
            let bool_ty = semantic
                .types()
                .iter()
                .position(|ty| {
                    matches!(
                        ty.shape(),
                        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
                    )
                })
                .map(|index| SemanticTypeIdV1::from_index(index as u32))
                .unwrap();
            let root = &functions[0];
            let slice_ty = root.locals()[3].ty();
            let mut locals = root.locals().to_vec();
            let length = locals.len() as u32;
            let index = length + 1;
            let condition = length + 2;
            for (tag, ty) in [(219, index_ty), (220, index_ty), (221, bool_ty)] {
                locals.push(local(tag, ty, SemanticLocalRoleV1::Temporary));
            }
            let value = |id, ty| SemanticOperandV1::Copy(place(id, ty));
            let edge = |role, target| {
                SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
            };
            let mut blocks = root.blocks().to_vec();
            assert_eq!(blocks.len(), 3);
            assert!(matches!(
                blocks[2].terminator().kind(),
                SemanticTerminatorKindV1::Return
            ));
            let mut statements = blocks[2].statements().to_vec();
            statements.push(assign(
                place(length, index_ty),
                SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::PointerMetadata,
                    operand: value(3, slice_ty),
                },
            ));
            blocks[2] = block(
                219,
                statements,
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: value(length, index_ty),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            1,
                            edge(SemanticEdgeRoleV1::SwitchValue, 3),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 4),
                    )
                    .unwrap(),
                },
            );
            blocks.push(block(
                220,
                vec![
                    assign(
                        place(index, index_ty),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                            SemanticConstantV1::new(
                                index_ty,
                                SemanticConstantValueV1::Scalar(
                                    SemanticScalarValueV1::new(0, 8).unwrap(),
                                ),
                            ),
                        )),
                    ),
                    assign(
                        place(condition, bool_ty),
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::LessThan,
                            left: value(index, index_ty),
                            right: value(length, index_ty),
                        },
                    ),
                ],
                SemanticTerminatorKindV1::Assert {
                    condition: value(condition, bool_ty),
                    expected: true,
                    message: SemanticAssertMessageV1::BoundsCheck {
                        length: value(length, index_ty),
                        index: value(index, index_ty),
                    },
                    target: edge(SemanticEdgeRoleV1::AssertSuccess, 4),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ));
            blocks.push(block(221, vec![], SemanticTerminatorKindV1::Return));
            functions[0] = replace_extra_function_v29(root, root.abi().clone(), locals, blocks);
        }
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
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
