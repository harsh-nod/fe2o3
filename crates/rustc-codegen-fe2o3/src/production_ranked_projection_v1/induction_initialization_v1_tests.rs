use super::*;
use canonical_assertion_facts_v1::ProjectedAssertionConditionV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::semantic_mir_v1::*;

struct Facts<'a, 'w>(&'a mut Budget<'w>);
impl ProjectedAssertionFactsV1 for Facts<'_, '_> {
    fn charge_private_array_work(&mut self, amount: usize) -> Result<()> {
        self.0.charge_work(amount).map_err(resource)
    }
    fn helper_value_ledger_v1(&self) -> Result<(usize, Ledger)> {
        Ok((
            self.0 as *const _ as usize,
            self.0.work_ledger_identity_v1(),
        ))
    }
    fn scalar_private_storage_v1(&self) -> Result<usize> {
        Ok(self.0.storage())
    }
    fn reserve_scalar_private_storage_v1(&mut self, amount: usize) -> Result<()> {
        self.0.reserve_storage(amount).map_err(resource)
    }
    fn release_scalar_private_storage_v1(&mut self, amount: usize) -> Result<()> {
        self.0.release_storage(amount).map_err(resource)
    }
    fn private_array_initializer_count(&mut self, _: usize, _: usize) -> Result<Option<u64>> {
        Ok(None)
    }
    fn is_materialized_block(&mut self, _: usize) -> Result<bool> {
        Ok(true)
    }
    fn condition(
        &mut self,
        _: usize,
        _: bool,
        _: SemanticBlockIdV1,
    ) -> Result<ProjectedAssertionConditionV1> {
        Ok(ProjectedAssertionConditionV1::Dynamic)
    }
}

const DONE: &str = "separated initializer component complete";
const SIBLING: usize = 17;
struct Snapshot {
    result: Result<ProductionRankedRootProgramV1>,
    work: usize,
    peak: usize,
    failed_storage: Option<usize>,
    failed_work: Option<usize>,
}
fn run(
    work_limit: usize,
    storage_limit: usize,
    action: impl FnOnce(&mut Context<'_, '_>) -> Result<()>,
) -> Snapshot {
    let mut work = Work::new(work_limit);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(SIBLING).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = with_scope(&mut Facts(&mut budget), |scope, facts| {
            action(&mut Context { scope, facts })?;
            Err(reject(DONE))
        });
        assert_eq!(budget.storage(), SIBLING);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    Snapshot {
        result,
        work: accepted,
        peak,
        failed_storage,
        failed_work: work.failed_work(),
    }
}
fn success(row: &Snapshot) {
    assert!(matches!(
        &row.result,
        Err(ProductionRankedProjectionErrorV1::Incomplete(DONE))
    ));
}
fn refused(row: &Snapshot) {
    assert!(
        matches!(&row.result, Err(ProductionRankedProjectionErrorV1::Incomplete(reason)) if *reason != DONE)
    );
}
fn edge(role: SemanticEdgeRoleV1, target: usize) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target as u32))
}
fn jump(target: usize) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target))
}
fn block(
    old: &SemanticBasicBlockV1,
    statements: Vec<SemanticStatementV1>,
    term: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        old.identity(),
        old.source(),
        statements,
        SemanticTerminatorV1::new(old.terminator().source(), term),
    )
    .unwrap()
}
fn rebuild(
    function: &SemanticFunctionDeclV1,
    entry: usize,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi().clone(),
        function.locals().to_vec(),
        SemanticBlockIdV1::from_index(entry as u32),
        blocks,
    )
    .unwrap()
}
fn separate(function: &SemanticFunctionDeclV1, reversed: bool) -> SemanticFunctionDeclV1 {
    let mut blocks = function.blocks().to_vec();
    let entry = function.entry().index() as usize;
    let appended = blocks.len();
    let old = blocks[entry].clone();
    let (statements, terminator, source_entry) = if reversed {
        blocks[entry] = block(&old, vec![], old.terminator().kind().clone());
        (old.statements().to_vec(), jump(entry), appended)
    } else {
        blocks[entry] = block(&old, old.statements().to_vec(), jump(appended));
        (vec![], old.terminator().kind().clone(), entry)
    };
    blocks.push(
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([231; 32]),
            old.source(),
            statements,
            SemanticTerminatorV1::new(old.terminator().source(), terminator),
        )
        .unwrap(),
    );
    rebuild(function, source_entry, blocks)
}
fn fixture() -> SemanticFunctionDeclV1 {
    let function = super::super::tests::multi_entry_source_fixture_v1();
    let mut blocks = function.blocks().to_vec();
    blocks[2] = block(&blocks[2], vec![], jump(1));
    rebuild(&function, 0, blocks)
}
fn topology() -> ProjectedNaturalLoopTopologyV1 {
    ProjectedNaturalLoopTopologyV1 {
        initializer_block: 1,
        preheader_control: ProjectedInductionPreheaderControlV1::Direct,
        latch: 4,
        loop_blocks: vec![3, 4],
    }
}
fn make(
    function: &SemanticFunctionDeclV1,
    context: &mut Context<'_, '_>,
) -> Result<ProjectedUniformInductionV1> {
    let graph = projected_loop_cfg_graph_v1(function)?;
    let local = SemanticLocalIdV1::from_index(1);
    let ty = function.locals()[1].ty();
    let (_, proof) = find_single(function, &graph, &topology(), 3, local, ty, context)?;
    assert_eq!(
        proof.initialization,
        ScalarAssignmentSiteV1 {
            block: 0,
            statement: 0
        }
    );
    let initial = ProductionRankedValueV1::Argument(7);
    bind_single(proof, initial, context)?;
    let SemanticStatementKindV1::Assign(compare) = function.blocks()[3].statements()[0].kind()
    else {
        unreachable!()
    };
    let SemanticRvalueKindV1::Binary { right: bound, .. } = compare.value().kind() else {
        unreachable!()
    };
    let SemanticStatementKindV1::Assign(update) = function.blocks()[4].statements()[0].kind()
    else {
        unreachable!()
    };
    let SemanticRvalueKindV1::Binary { right: step, .. } = update.value().kind() else {
        unreachable!()
    };
    Ok(ProjectedUniformInductionV1 {
        initializer_block: 1,
        preheader_control: ProjectedInductionPreheaderControlV1::Direct,
        header: 3,
        body_entry: 4,
        latch: 4,
        exit: 5,
        loop_blocks: vec![3, 4],
        initial,
        bound: ProductionRankedValueV1::Argument(0),
        step: ProductionRankedValueV1::Argument(8),
        source_progress: ProjectedSourceInductionCandidateV1 {
            induction: local,
            induction_type: ty,
            header_statement: 0,
            bound_operand: bound.clone(),
            latch_statement: 0,
            update: ProjectedSourceInductionUpdateV1::Ordinary,
            step_operand: step.clone(),
            step_value: 1,
            ranked_bound: ProductionRankedValueV1::Argument(0),
            ranked_step: ProductionRankedValueV1::Argument(8),
        },
        bound_cast: None,
        body_predicates: vec![],
    })
}

fn project(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    context: Option<&mut Context<'_, '_>>,
) -> Result<Vec<ProjectedUniformInductionV1>> {
    let constants = constant_locals(function)?;
    let origins = local_stable_argument_origins(types, function)?;
    let definitions = local_definition_counts(function);
    let mut arguments = vec![None; function.locals().len()];
    let mut next_argument = 1;
    let mut operations = vec![];
    let mut next_value = 0;
    let mut context = context;
    let mut rows = project_uniform_inductions_with_multi_entry_v1(
        &[],
        types,
        function,
        &constants,
        &origins,
        &definitions,
        &mut arguments,
        &mut next_argument,
        &mut operations,
        &mut next_value,
        context.as_deref_mut(),
    )?;
    reconcile_source_progress_with_multi_entry_v1(
        types,
        function,
        &constants,
        &origins,
        &definitions,
        &arguments,
        &mut rows,
        &mut operations,
        &mut next_value,
        context.as_deref_mut(),
    )?;
    if let Some(context) = context {
        before_emission(function, &rows, context)?;
    }
    Ok(rows)
}

#[test]
fn separated_initializer_projects_ordinary_checked_optional_nested_and_reversed_sources() {
    let (types, fixtures) = super::super::tests::separated_initialization_positive_fixtures_v1();
    for (ordinal, original) in fixtures.iter().enumerate() {
        // Historical immediate inputs still use their original scan without a registry.
        success(&run(usize::MAX, usize::MAX, |context| {
            assert!(!project(&types, original, Some(context))?.is_empty());
            assert_eq!(context.scope.registered, 0);
            Ok(())
        }));
        for reversed in [false, true] {
            let source = separate(original, reversed);
            let result = run(usize::MAX, usize::MAX, |context| {
                let rows = project(&types, &source, Some(context))?;
                assert!(!rows.is_empty());
                assert_eq!(context.scope.registered, 1, "fixture {ordinal}");
                let record = context.scope.singles[0];
                assert_eq!(
                    record.source.initialization.block,
                    if reversed { original.blocks().len() } else { 0 }
                );
                assert_eq!(
                    record.source.preheader,
                    if reversed { 0 } else { original.blocks().len() }
                );
                Ok(())
            });
            success(&result);
            assert!(
                project(&types, &source, None).is_err(),
                "context-free separated fallback"
            );
        }
    }
}

#[test]
fn separated_initializer_diamond_and_uniform_argument_preserve_true_preheader() {
    let (types, _) = super::super::tests::separated_initialization_positive_fixtures_v1();
    for argument in [false, true] {
        let mut source = fixture();
        if argument {
            let mut blocks = source.blocks().to_vec();
            let SemanticStatementKindV1::Assign(initial) = blocks[0].statements()[0].kind() else {
                unreachable!()
            };
            let statement = SemanticStatementV1::new(
                blocks[0].source(),
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    initial.destination().clone(),
                    SemanticRvalueV1::new(
                        initial.value().result_type(),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                            SemanticPlaceV1::new(
                                SemanticLocalIdV1::from_index(3),
                                vec![],
                                source.locals()[3].ty(),
                            )
                            .unwrap(),
                        )),
                    ),
                )),
            );
            blocks[0] = block(
                &blocks[0],
                vec![statement],
                blocks[0].terminator().kind().clone(),
            );
            source = rebuild(&source, 0, blocks);
        }
        success(&run(usize::MAX, usize::MAX, |context| {
            let rows = project(&types, &source, Some(context))?;
            assert_eq!(rows.len(), 1);
            assert_eq!(
                (rows[0].initializer_block, rows[0].header, rows[0].latch),
                (1, 3, 4)
            );
            assert_eq!(context.scope.singles[0].source.initialization.block, 0);
            Ok(())
        }));
    }
}

fn append(
    function: &SemanticFunctionDeclV1,
    at: usize,
    kind: SemanticStatementKindV1,
) -> SemanticFunctionDeclV1 {
    let mut blocks = function.blocks().to_vec();
    let mut statements = blocks[at].statements().to_vec();
    statements.push(SemanticStatementV1::new(blocks[at].source(), kind));
    blocks[at] = block(
        &blocks[at],
        statements,
        blocks[at].terminator().kind().clone(),
    );
    rebuild(function, function.entry().index() as usize, blocks)
}

#[test]
fn separated_initializer_rejects_each_intervening_kill_and_alias_family() {
    let source = fixture();
    let local = SemanticLocalIdV1::from_index(1);
    let place = SemanticPlaceV1::new(local, vec![], source.locals()[1].ty()).unwrap();
    let other = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(0),
        vec![],
        source.locals()[0].ty(),
    )
    .unwrap();
    let assign = |value| {
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            other.clone(),
            SemanticRvalueV1::new(other.ty(), value),
        ))
    };
    let kills = vec![
        SemanticStatementKindV1::StorageLive(local),
        SemanticStatementKindV1::StorageDead(local),
        SemanticStatementKindV1::Deinitialize(place.clone()),
        SemanticStatementKindV1::Assume(SemanticOperandV1::Move(place.clone())),
        assign(SemanticRvalueKindV1::Use(SemanticOperandV1::Move(
            place.clone(),
        ))),
        assign(SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: place.clone(),
        }),
        assign(SemanticRvalueKindV1::AddressOf {
            mutability: SemanticMutabilityV1::Mutable,
            place: place.clone(),
        }),
        source.blocks()[0].statements()[0].kind().clone(),
        SemanticStatementKindV1::Deinitialize(
            SemanticPlaceV1::new(
                local,
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), place.ty())
                        .unwrap(),
                ],
                place.ty(),
            )
            .unwrap(),
        ),
    ];
    for at in [0, 1, 2, 4] {
        for kind in &kills {
            // Lifetime markers and moves in the loop do not kill the initial
            // entry binding; the unchanged recurrence checker owns that domain.
            if at == 4
                && !matches!(kind, SemanticStatementKindV1::Deinitialize(_))
                && !matches!(kind, SemanticStatementKindV1::Assign(value) if value.destination().local() == local || matches!(value.value().kind(), SemanticRvalueKindV1::Borrow { .. } | SemanticRvalueKindV1::AddressOf { .. }))
            {
                continue;
            }
            let changed = append(&source, at, kind.clone());
            refused(&run(usize::MAX, usize::MAX, |context| {
                make(&changed, context)?;
                Ok(())
            }));
        }
    }
}

#[test]
fn separated_initializer_rejects_bypass_nonuse_type_and_definition_substitution() {
    let source = fixture();
    for mutation in 0..8 {
        let mut blocks = source.blocks().to_vec();
        let mut entry = 0;
        if mutation == 0 {
            entry = 1;
        } else if mutation == 1 {
            blocks[0] = block(&blocks[0], vec![], blocks[0].terminator().kind().clone());
        } else if mutation == 7 {
            let initial = blocks[0].statements()[0].clone();
            blocks[0] = block(&blocks[0], vec![], blocks[0].terminator().kind().clone());
            let mut statements = blocks[3].statements().to_vec();
            statements.push(initial);
            blocks[3] = block(
                &blocks[3],
                statements,
                blocks[3].terminator().kind().clone(),
            );
        } else {
            let SemanticStatementKindV1::Assign(initial) = blocks[0].statements()[0].kind() else {
                unreachable!()
            };
            let SemanticRvalueKindV1::Use(operand) = initial.value().kind() else {
                unreachable!()
            };
            let wrong = SemanticTypeIdV1::from_index(91);
            let destination = SemanticPlaceV1::new(
                initial.destination().local(),
                vec![],
                if mutation == 3 {
                    wrong
                } else {
                    initial.destination().ty()
                },
            )
            .unwrap();
            let operand = if mutation == 5 {
                SemanticOperandV1::Copy(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(3), vec![], wrong).unwrap(),
                )
            } else {
                operand.clone()
            };
            let kind = if mutation == 2 {
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left: operand.clone(),
                    right: operand,
                }
            } else {
                SemanticRvalueKindV1::Use(operand)
            };
            let statement = SemanticStatementV1::new(
                blocks[0].source(),
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    destination,
                    SemanticRvalueV1::new(
                        if mutation == 4 {
                            wrong
                        } else {
                            initial.value().result_type()
                        },
                        kind,
                    ),
                )),
            );
            blocks[0] = block(
                &blocks[0],
                if mutation == 6 {
                    vec![statement.clone(), statement]
                } else {
                    vec![statement]
                },
                blocks[0].terminator().kind().clone(),
            );
        }
        let changed = rebuild(&source, entry, blocks);
        refused(&run(usize::MAX, usize::MAX, |context| {
            make(&changed, context)?;
            Ok(())
        }));
    }
}

#[test]
fn separated_initializer_rejects_drop_call_destination_and_terminator_move() {
    let source = fixture();
    let place = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![],
        source.locals()[1].ty(),
    )
    .unwrap();
    let terms = vec![
        SemanticTerminatorKindV1::Drop {
            place: place.clone(),
            drop_glue: SemanticFunctionIdV1::from_index(0),
            target: edge(SemanticEdgeRoleV1::DropReturn, 3),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new(
                SemanticFunctionIdV1::from_index(0),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    place.clone(),
                    edge(SemanticEdgeRoleV1::CallReturn, 3),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: SemanticOperandV1::Move(place),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    0,
                    edge(SemanticEdgeRoleV1::SwitchValue, 3),
                )],
                edge(SemanticEdgeRoleV1::SwitchOtherwise, 5),
            )
            .unwrap(),
        },
    ];
    for term in terms {
        let mut blocks = source.blocks().to_vec();
        blocks[1] = block(&blocks[1], vec![], term);
        let changed = rebuild(&source, 0, blocks);
        refused(&run(usize::MAX, usize::MAX, |context| {
            make(&changed, context)?;
            Ok(())
        }));
    }
}

#[test]
fn separated_initializer_registry_rejects_source_coordinate_value_and_roster_changes() {
    for mutation in 0..15 {
        let source = fixture();
        let foreign = source.clone();
        refused(&run(usize::MAX, usize::MAX, |context| {
            let mut row = make(&source, context)?;
            let mut use_source = &source;
            match mutation {
                0 => use_source = &foreign,
                1 => {
                    context.scope.singles[0].source.function =
                        SemanticFunctionIdentityV1::from_sha256([198; 32])
                }
                2 => context.scope.singles[0].source.initialization.statement = 1,
                3 => row.initializer_block = 2,
                4 => context.scope.singles[0].source.successor = 1,
                5 => row.initial = ProductionRankedValueV1::Argument(99),
                6 => context.scope.singles.clear(),
                7 => {
                    context.scope.singles.clear();
                    context.scope.registered = 0;
                }
                8 => row.loop_blocks.pop().map(|_| ()).unwrap(),
                9 => row.loop_blocks.reverse(),
                10 => row.source_progress.induction_type = SemanticTypeIdV1::from_index(99),
                11 => context.scope.singles[0].source.role = SemanticEdgeRoleV1::SwitchOtherwise,
                12 => {
                    context.scope.registered = 2;
                }
                13 => {
                    return replay_roster(
                        &source,
                        &projected_loop_cfg_graph_v1(&source)?,
                        &[],
                        context,
                    );
                }
                14 => {
                    let mut duplicate = make_row_copy(&row);
                    duplicate.header = row.header;
                    return replay_roster(
                        &source,
                        &projected_loop_cfg_graph_v1(&source)?,
                        &[row, duplicate],
                        context,
                    );
                }
                _ => unreachable!(),
            }
            before_emission(use_source, &[row], context)
        }));
    }
}
fn make_row_copy(row: &ProjectedUniformInductionV1) -> ProjectedUniformInductionV1 {
    ProjectedUniformInductionV1 {
        initializer_block: row.initializer_block,
        preheader_control: row.preheader_control.clone_single_v1().unwrap(),
        header: row.header,
        body_entry: row.body_entry,
        latch: row.latch,
        exit: row.exit,
        loop_blocks: row.loop_blocks.clone(),
        initial: row.initial,
        bound: row.bound,
        step: row.step,
        source_progress: row.source_progress.clone(),
        bound_cast: row.bound_cast.clone(),
        body_predicates: row.body_predicates.clone(),
    }
}

#[test]
fn separated_initializer_shared_graph_normalization_and_paid_capacity_match() {
    let (_, fixtures) = super::super::tests::separated_initialization_positive_fixtures_v1();
    for source in fixtures.into_iter().chain([fixture()]) {
        success(&run(usize::MAX, usize::MAX, |context| {
            temporary(context, |context| {
                let actual = projected_loop_cfg_graph_paid_v1(&source, context)?;
                let expected = projected_loop_cfg_graph_v1(&source)?;
                assert_eq!(actual.successors, expected.successors);
                assert_eq!(actual.predecessors, expected.predecessors);
                assert_eq!(actual.reachable, expected.reachable);
                assert_eq!(actual.entry, expected.entry);
                let graph_storage = std::mem::size_of::<ProjectedLoopCfgV1>()
                    + (actual.successors.capacity() + actual.predecessors.capacity())
                        * std::mem::size_of::<Vec<usize>>()
                    + actual
                        .successors
                        .iter()
                        .chain(&actual.predecessors)
                        .map(|row| row.capacity() * std::mem::size_of::<usize>())
                        .sum::<usize>()
                    + actual.reachable.capacity();
                assert!(context.scope.retained >= std::mem::size_of::<Scope>() + graph_storage);
                Ok(())
            })
        }));
    }
}

fn expect_storage(row: Snapshot, actual: usize, limit: usize, accepted: usize, prior_peak: usize) {
    let Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
        canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Storage(error)),
    )) = row.result
    else {
        panic!("exact storage refusal required")
    };
    assert_eq!((error.actual(), error.limit()), (actual, limit));
    assert_eq!(
        (row.work, row.peak, row.failed_storage, row.failed_work),
        (accepted, prior_peak, Some(actual), None)
    );
}
fn expect_work(row: Snapshot, actual: usize, limit: usize, accepted: usize, peak: usize) {
    let Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
        canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Work(error)),
    )) = row.result
    else {
        panic!("exact work refusal required")
    };
    assert_eq!((error.actual(), error.limit()), (actual, limit));
    assert_eq!(
        (row.work, row.peak, row.failed_storage, row.failed_work),
        (accepted, peak, None, Some(actual))
    );
}

#[test]
fn separated_initializer_exact_initial_header_and_scratch_reservation_prefixes() {
    let source = fixture();
    let graph = projected_loop_cfg_graph_v1(&source).unwrap();
    let observe = |context: &mut Context<'_, '_>| {
        find_single(
            &source,
            &graph,
            &topology(),
            3,
            SemanticLocalIdV1::from_index(1),
            source.locals()[1].ty(),
            context,
        )?;
        Ok(())
    };
    let scope = std::mem::size_of::<Scope>();
    expect_storage(
        run(usize::MAX, SIBLING + scope - 1, observe),
        SIBLING + scope,
        SIBLING + scope - 1,
        0,
        SIBLING,
    );
    let pending = std::mem::size_of::<(&SemanticOperandV1, PendingSingle)>();
    expect_storage(
        run(usize::MAX, SIBLING + scope + pending - 1, observe),
        SIBLING + scope + pending,
        SIBLING + scope + pending - 1,
        0,
        SIBLING + scope,
    );
    // Independent source census: shape check, block visit, all statement
    // definition visits, then each terminator definition check. No failed run
    // or measured total supplies this successful prefix.
    let work = 12
        + source
            .blocks()
            .iter()
            .map(|block| 4 + 5 * block.statements().len())
            .sum::<usize>();
    let count = source.blocks().len();
    let mut floor = SIBLING + scope + pending;
    for amount in [
        std::mem::size_of::<Scratch>(),
        count,
        count,
        count * std::mem::size_of::<usize>(),
    ] {
        let extent = floor + amount;
        expect_storage(
            run(usize::MAX, extent - 1, observe),
            extent,
            extent - 1,
            work,
            floor,
        );
        floor = extent;
    }
}

#[test]
fn separated_initializer_exact_registry_adoption_and_capacity_arithmetic() {
    let source = fixture();
    let graph = projected_loop_cfg_graph_v1(&source).unwrap();
    let prepare = |context: &mut Context<'_, '_>| {
        find_single(
            &source,
            &graph,
            &topology(),
            3,
            SemanticLocalIdV1::from_index(1),
            source.locals()[1].ty(),
            context,
        )
    };
    let mut prefix = None;
    let reference = run(usize::MAX, usize::MAX, |context| {
        let _ = prepare(context)?;
        context.charge(8)?;
        prefix = Some((
            context.facts.scalar_private_storage_v1()?,
            context.scope.retained,
        ));
        Ok(())
    });
    success(&reference);
    let (floor, retained) = prefix.unwrap();
    assert_eq!(
        retained,
        std::mem::size_of::<Scope>() + std::mem::size_of::<(&SemanticOperandV1, PendingSingle)>()
    );
    // Keep a sibling receipt live to force registry adoption above the previous
    // successful proof peak. Its size is independently selected from headers.
    let sibling =
        std::mem::size_of::<Scope>() + std::mem::size_of::<Scratch>() + 16 * source.blocks().len();
    for boundary in [
        std::mem::size_of::<Vec<Single>>(),
        std::mem::size_of::<Vec<Single>>() + std::mem::size_of::<Single>(),
    ] {
        let extent = floor + sibling + boundary;
        let result = run(usize::MAX, extent - 1, |context| {
            let (_, proof) = prepare(context)?;
            context.facts.reserve_scalar_private_storage_v1(sibling)?;
            let result = bind_single(proof, ProductionRankedValueV1::Argument(7), context);
            context.facts.release_scalar_private_storage_v1(sibling)?;
            result
        });
        let prior = if boundary == std::mem::size_of::<Vec<Single>>() {
            floor + sibling
        } else {
            floor + sibling + std::mem::size_of::<Vec<Single>>()
        };
        expect_storage(result, extent, extent - 1, reference.work, prior);
    }
    let result = run(usize::MAX, usize::MAX, |context| {
        let _ = context.backing::<usize>(usize::MAX)?;
        Ok(())
    });
    assert!(matches!(
        result.result,
        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
            canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Arithmetic)
        ))
    ));
    assert_eq!(
        (
            result.work,
            result.peak,
            result.failed_storage,
            result.failed_work
        ),
        (0, SIBLING, None, None)
    );
}

// Successful reference composition for the final replay's first maximum:
// actual preparation -> shared paid CFG allocator -> independently enumerated
// fixed roster/copy/source-census prefix -> Scratch -> outside mask adoption.
// Only the first two are genuine component constructors. This never calls the
// observed before_emission/replay_roster/prove control path as its oracle.
fn replay_peak_reference(
    source: &SemanticFunctionDeclV1,
    context: &mut Context<'_, '_>,
) -> Result<(usize, usize, usize, usize)> {
    let row = make(source, context)?;
    let graph = projected_loop_cfg_graph_paid_v1(source, context)?;
    context.charge(2)?;
    context.charge(2)?;
    context.reserve(std::mem::size_of::<Vec<u8>>())?;
    let mut visited = context.backing::<u8>(1)?;
    context.charge(1)?;
    visited.push(0);
    context.charge(4)?;
    for _ in source.blocks()[row.initializer_block].statements() {
        context.charge(3)?;
    }
    context.charge(3)?; // lookup base 2 plus bit width of one entry
    context.charge(2)?;
    context.reserve(std::mem::size_of::<Single>() + std::mem::size_of::<PendingSingle>())?;
    context.charge(64)?;
    context.charge(12)?;
    for block in source.blocks() {
        context.charge(2)?;
        for _ in block.statements() {
            context.charge(5)?;
        }
        context.charge(2)?;
    }
    let count = source.blocks().len();
    context.reserve(std::mem::size_of::<Scratch>())?;
    let mut seen = context.backing::<u8>(count)?;
    let region = context.backing::<u8>(count)?;
    let mut pending = context.backing::<usize>(count)?;
    context.charge(2 * count)?;
    seen.resize(count, 0);
    context.charge(3)?;
    seen[graph.entry] = 1;
    pending.push(graph.entry);
    while let Some(block) = pending.pop() {
        context.charge(3)?;
        for &target in &graph.successors[block] {
            context.charge(2)?;
            if target != row.header {
                context.charge(3)?;
                if seen[target] == 0 {
                    seen[target] = 1;
                    pending.push(target);
                }
            }
        }
    }
    context.reserve(std::mem::size_of::<Vec<bool>>())?;
    let prior = context.facts.scalar_private_storage_v1()?;
    let outside = context.backing::<bool>(count)?;
    let extent = context.facts.scalar_private_storage_v1()?;
    assert_eq!(outside.capacity(), count);
    assert_eq!(extent - prior, count * std::mem::size_of::<bool>());
    let held =
        seen.capacity() + region.capacity() + pending.capacity() * std::mem::size_of::<usize>();
    assert_eq!(held, 2 * count + count * std::mem::size_of::<usize>());
    Ok((prior, extent, count, context.scope.retained))
}

#[test]
fn separated_initializer_exact_replay_peak_and_adjacent_work_use_successful_prefix() {
    let source = fixture();
    let mut checkpoint = None;
    let prefix = run(usize::MAX, usize::MAX, |context| {
        checkpoint = Some(replay_peak_reference(&source, context)?);
        Ok(())
    });
    success(&prefix);
    let (prior, peak, next_charge, _) = checkpoint.unwrap();
    let action = |context: &mut Context<'_, '_>| {
        let row = make(&source, context)?;
        before_emission(&source, &[row], context)
    };
    let whole = run(usize::MAX, usize::MAX, action);
    success(&whole);
    assert_eq!(
        whole.peak, peak,
        "the independently successful prefix is the real global peak"
    );
    expect_storage(
        run(whole.work, peak - 1, action),
        peak,
        peak - 1,
        prefix.work,
        prior,
    );
    expect_work(
        run(prefix.work + next_charge - 1, peak, action),
        prefix.work + next_charge,
        prefix.work + next_charge - 1,
        prefix.work,
        peak,
    );
    // Replay's final typed coordinate comparison is one 64-unit charge, not a
    // measured failed-run tail or a permissive accepted-work interval.
    expect_work(
        run(whole.work - 1, peak, action),
        whole.work,
        whole.work - 1,
        whole.work - 64,
        peak,
    );
    let exact = run(whole.work, peak, action);
    success(&exact);
    assert_eq!(
        (
            exact.work,
            exact.peak,
            exact.failed_storage,
            exact.failed_work
        ),
        (whole.work, peak, None, None)
    );
}

#[test]
fn separated_initializer_custody_preserves_siblings_and_panic_cleanup() {
    let source = fixture();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(SIBLING).unwrap();
    budget.charge_work(23).unwrap();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_scope(&mut Facts(&mut budget), |scope, facts| {
            let row = make(&source, &mut Context { scope, facts })?;
            assert_eq!(scope.registered, 1);
            let mut other_work = Work::new(usize::MAX);
            let mut other = Budget::new(&mut other_work, usize::MAX);
            let error = before_emission(
                &source,
                &[row],
                &mut Context {
                    scope,
                    facts: &mut Facts(&mut other),
                },
            );
            assert!(matches!(
                error,
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(
                        Resource::Accounting
                    )
                ))
            ));
            assert_eq!((other.storage(), other.work()), (0, 0));
            facts.0.reserve_storage(31).unwrap();
            panic!("registry must drop under its live reservation");
        })
    }));
    assert!(unwind.is_err());
    assert_eq!(budget.storage(), SIBLING + 31);
    assert!(budget.work() > 23);
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn separated_initializer_preserves_prior_denials_and_rejects_moved_ledger_or_missing_floor() {
    let source = fixture();
    let action = |context: &mut Context<'_, '_>| {
        let row = make(&source, context)?;
        before_emission(&source, &[row], context)
    };
    let measured = run(usize::MAX, usize::MAX, action);
    success(&measured);
    let mut meter = Work::new(measured.work + 23);
    {
        let mut budget = Budget::new(&mut meter, measured.peak);
        budget.reserve_storage(SIBLING).unwrap();
        budget.charge_work(23).unwrap();
        assert!(matches!(
            budget.charge_work(measured.work + 1),
            Err(Resource::Work(_))
        ));
        assert!(matches!(
            budget.reserve_storage(measured.peak),
            Err(Resource::Storage(_))
        ));
        let result = with_scope(&mut Facts(&mut budget), |scope, facts| {
            action(&mut Context { scope, facts })?;
            Err(reject(DONE))
        });
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Incomplete(DONE))
        ));
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_storage()
            ),
            (
                measured.work + 23,
                SIBLING,
                measured.peak,
                Some(measured.peak + SIBLING)
            )
        );
    }
    assert_eq!(meter.failed_work(), Some(measured.work + 24));
    let mut work = Work::new(usize::MAX);
    let mut foreign_work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let mut foreign = Budget::new(&mut foreign_work, usize::MAX);
    budget.reserve_storage(SIBLING).unwrap();
    let result = with_scope(&mut Facts(&mut budget), |scope, facts| {
        make(&source, &mut Context { scope, facts })?;
        let work = facts.0.work();
        facts.0.release_storage(1).unwrap();
        let error = Context { scope, facts }.charge(1);
        assert!(matches!(
            error,
            Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(
                    Resource::Accounting
                )
            ))
        ));
        facts.0.reserve_storage(1).unwrap();
        std::mem::swap(&mut *facts.0, &mut foreign);
        let error = Context {
            scope,
            facts: &mut Facts(&mut foreign),
        }
        .charge(1);
        assert!(matches!(
            error,
            Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(
                    Resource::Accounting
                )
            ))
        ));
        std::mem::swap(&mut *facts.0, &mut foreign);
        assert_eq!(facts.0.work(), work);
        Err(reject(DONE))
    });
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::Incomplete(DONE))
    ));
    assert_eq!(
        (budget.storage(), foreign.storage(), foreign.work()),
        (SIBLING, 0, 0)
    );
}

#[test]
fn separated_initializer_statement_order_dead_side_exit_and_absent_context() {
    let base = fixture();
    for variant in 0..3 {
        let mut blocks = base.blocks().to_vec();
        if variant == 0 {
            let mut statements = blocks[0].statements().to_vec();
            statements.insert(
                0,
                SemanticStatementV1::new(
                    blocks[0].source(),
                    SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(1)),
                ),
            );
            blocks[0] = block(
                &blocks[0],
                statements,
                blocks[0].terminator().kind().clone(),
            );
        } else if variant == 1 {
            blocks[2] = block(
                &blocks[2],
                vec![SemanticStatementV1::new(
                    blocks[2].source(),
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1)),
                )],
                jump(5),
            );
        } else {
            blocks[1] = block(
                &blocks[1],
                vec![SemanticStatementV1::new(
                    blocks[1].source(),
                    SemanticStatementKindV1::Nop,
                )],
                jump(3),
            );
        }
        let source = rebuild(&base, 0, blocks);
        success(&run(usize::MAX, usize::MAX, |context| {
            let (_, proof) = find_single(
                &source,
                &projected_loop_cfg_graph_v1(&source)?,
                &topology(),
                3,
                SemanticLocalIdV1::from_index(1),
                source.locals()[1].ty(),
                context,
            )?;
            assert_eq!(proof.initialization.statement, usize::from(variant == 0));
            Ok(())
        }));
    }
    success(&run(usize::MAX, usize::MAX, |context| {
        let row = make(&base, context)?;
        assert!(matches!(
            without_context(&base, std::slice::from_ref(&row)),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "separated initialization is missing its registered proof"
            ))
        ));
        assert!(matches!(
            without_scope(&base, &[row], context.facts),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "separated initialization is missing its registered proof"
            ))
        ));
        Ok(())
    }));
}

#[path = "induction_initialization_roster_v1_tests.rs"]
mod roster;
