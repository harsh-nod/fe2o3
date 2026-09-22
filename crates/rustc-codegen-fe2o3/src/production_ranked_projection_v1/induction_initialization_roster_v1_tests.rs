//! Inert same-function two-proof projection and exact nonempty growth boundaries.
use super::*;

fn remap_term(kind: &SemanticTerminatorKindV1, permutation: &[usize]) -> SemanticTerminatorKindV1 {
    let target = |old: &SemanticControlFlowEdgeV1| {
        edge(old.role(), permutation[old.target().index() as usize])
    };
    match kind {
        SemanticTerminatorKindV1::Goto(old) => SemanticTerminatorKindV1::Goto(target(old)),
        SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } => SemanticTerminatorKindV1::SwitchInt {
            discriminant: discriminant.clone(),
            targets: SemanticSwitchTargetsV1::new(
                targets
                    .values()
                    .iter()
                    .map(|old| SemanticSwitchTargetV1::new(old.value(), target(&old.edge())))
                    .collect(),
                target(&targets.otherwise()),
            )
            .unwrap(),
        },
        SemanticTerminatorKindV1::Return => SemanticTerminatorKindV1::Return,
        _ => panic!("unexpected nested fixture terminator; never silently omit an edge"),
    }
}

fn two_separated(
    explicit_to_header: bool,
    reverse_headers: bool,
) -> (Vec<SemanticTypeDeclV1>, SemanticFunctionDeclV1) {
    let (types, fixtures) =
        crate::production_ranked_projection_v1::tests::separated_initialization_positive_fixtures_v1();
    let original = &fixtures[if explicit_to_header { 4 } else { 5 }];
    assert_eq!(original.blocks().len(), 7);
    let mut blocks = original.blocks().to_vec();
    // Preserve both actual definitions. Their newly separated control blocks
    // retain the original Direct/Optional terminators and exact selector edges.
    for (definition, control, identity) in [(0, 7, 233), (2, 8, 234)] {
        assert_eq!(blocks.len(), control);
        let old = &original.blocks()[definition];
        blocks[definition] = block(old, old.statements().to_vec(), jump(control));
        blocks.push(
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([identity; 32]),
                old.source(),
                vec![],
                SemanticTerminatorV1::new(
                    old.terminator().source(),
                    old.terminator().kind().clone(),
                ),
            )
            .unwrap(),
        );
    }
    let mut permutation: Vec<usize> = (0..blocks.len()).collect();
    if reverse_headers {
        permutation.swap(1, 3);
    }
    let mut remapped = blocks.clone();
    for (index, old) in blocks.iter().enumerate() {
        remapped[permutation[index]] = block(
            old,
            old.statements().to_vec(),
            remap_term(old.terminator().kind(), &permutation),
        );
    }
    let entry = permutation[original.entry().index() as usize];
    (types, rebuild(original, entry, remapped))
}

fn assert_two_records(
    source: &SemanticFunctionDeclV1,
    rows: &[ProjectedUniformInductionV1],
    scope: &Scope,
    reversed: bool,
) {
    assert_eq!(scope.registered, 2);
    assert_eq!(scope.singles.len(), 2);
    assert_eq!(
        scope
            .singles
            .iter()
            .map(|row| row.source.header)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.iter().map(|row| row.header).collect::<Vec<_>>(),
        if reversed { [3, 1] } else { [1, 3] }
    );
    assert!(rows[0].loop_blocks.len() > rows[1].loop_blocks.len());
    for (ordinal, row) in rows.iter().enumerate() {
        let record = scope
            .singles
            .iter()
            .find(|record| record.source.header == row.header)
            .unwrap();
        let (definition, preheader, latch, local) = if ordinal == 0 {
            (0, 7, 5, 1)
        } else {
            (2, 8, 4, 4)
        };
        assert_eq!(record.source.source_address, source as *const _ as usize);
        assert_eq!(record.source.function, source.identity());
        assert_eq!(
            record.source.initialization,
            ScalarAssignmentSiteV1 {
                block: definition,
                statement: 0
            }
        );
        assert_eq!(
            (record.source.preheader, record.source.latch),
            (preheader, latch)
        );
        assert_eq!(
            record.source.induction,
            SemanticLocalIdV1::from_index(local)
        );
        assert_eq!(record.source.ty, source.locals()[local as usize].ty());
        assert_eq!(row.initializer_block, preheader);
        assert_eq!(record.initial, row.initial);
        if ordinal == 0 {
            assert!(matches!(
                row.preheader_control,
                ProjectedInductionPreheaderControlV1::Direct
            ));
        } else {
            assert!(matches!(
                row.preheader_control,
                ProjectedInductionPreheaderControlV1::Optional { .. }
            ));
        }
    }
}

fn actual_rows(
    types: &[SemanticTypeDeclV1],
    source: &SemanticFunctionDeclV1,
    reversed: bool,
) -> Vec<ProjectedUniformInductionV1> {
    let mut rows = None;
    success(&run(usize::MAX, usize::MAX, |context| {
        let projected = project(types, source, Some(context))?;
        assert_two_records(source, &projected, context.scope, reversed);
        rows = Some(projected);
        Ok(())
    }));
    // These are diagnostic coordinates from the actual private projector, not
    // source-owner evidence or a factory for a production proof receipt.
    rows.unwrap()
}

#[test]
fn two_separated_initializers_replay_both_rows_after_real_nesting_reordering() {
    for orientation in [false, true] {
        for reversed in [false, true] {
            let (types, source) = two_separated(orientation, reversed);
            success(&run(usize::MAX, usize::MAX, |context| {
                let mut rows = project(&types, &source, Some(context))?;
                assert_two_records(&source, &rows, context.scope, reversed);
                let original = [context.scope.singles[0], context.scope.singles[1]];
                let graph = projected_loop_cfg_graph_v1(&source)?;
                replay_roster(&source, &graph, &rows, context)?;
                rows.reverse();
                replay_roster(&source, &graph, &rows, context)?;
                before_emission(&source, &rows, context)?;
                assert_eq!(context.scope.singles.as_slice(), original.as_slice());
                rows.reverse();
                assert_two_records(&source, &rows, context.scope, reversed);
                Ok(())
            }));
        }
    }
}

#[test]
fn second_separated_record_cannot_be_omitted_duplicated_or_substituted() {
    let (types, source) = two_separated(true, true);
    for mutation in 0..7 {
        refused(&run(usize::MAX, usize::MAX, |context| {
            let mut rows = project(&types, &source, Some(context))?;
            assert_two_records(&source, &rows, context.scope, true);
            let first = context.scope.singles[0];
            let second_header = context.scope.singles[1].source.header;
            let selected = rows
                .iter()
                .position(|row| row.header == second_header)
                .unwrap();
            match mutation {
                0 => {
                    rows.remove(selected);
                }
                1 => {
                    rows.push(make_row_copy(&rows[selected]));
                }
                2 => {
                    context.scope.singles.pop().unwrap();
                }
                3 => {
                    context.scope.singles[1].source.initialization.statement = 99;
                }
                4 => {
                    context.scope.singles[1].initial = ProductionRankedValueV1::Argument(99);
                }
                5 => {
                    context.scope.singles[1].source.successor += 1;
                }
                6 => {
                    context.scope.singles[1].source.function =
                        SemanticFunctionIdentityV1::from_sha256([240; 32]);
                }
                _ => unreachable!(),
            }
            assert_eq!(context.scope.singles[0], first);
            before_emission(&source, &rows, context)
        }));
    }
}

struct GrowthPlan<'a> {
    source: &'a SemanticFunctionDeclV1,
    graph: ProjectedLoopCfgV1,
    ordered: [&'a ProjectedUniformInductionV1; 2],
    topologies: [ProjectedNaturalLoopTopologyV1; 2],
}

impl<'a> GrowthPlan<'a> {
    fn new(source: &'a SemanticFunctionDeclV1, rows: &'a [ProjectedUniformInductionV1]) -> Self {
        assert_eq!(rows.len(), 2);
        let ordered = if rows[0].header < rows[1].header {
            [&rows[0], &rows[1]]
        } else {
            [&rows[1], &rows[0]]
        };
        let topology = |row: &ProjectedUniformInductionV1| ProjectedNaturalLoopTopologyV1 {
            initializer_block: row.initializer_block,
            preheader_control: row.preheader_control.clone_single_v1().unwrap(),
            latch: row.latch,
            loop_blocks: row.loop_blocks.clone(),
        };
        Self {
            source,
            graph: projected_loop_cfg_graph_v1(source).unwrap(),
            topologies: [topology(ordered[0]), topology(ordered[1])],
            ordered,
        }
    }

    fn find(&self, index: usize, context: &mut Context<'_, '_>) -> Result<PendingSingle> {
        let row = self.ordered[index];
        let (_, proof) = find_single(
            self.source,
            &self.graph,
            &self.topologies[index],
            row.header,
            row.source_progress.induction,
            row.source_progress.induction_type,
            context,
        )?;
        Ok(proof)
    }

    fn pending_second(&self, context: &mut Context<'_, '_>) -> Result<PendingSingle> {
        assert_eq!(context.scope.registered, 0);
        let first = self.find(0, context)?;
        bind_single(first, self.ordered[0].initial, context)?;
        assert_eq!(context.scope.registered, 1);
        assert_eq!(context.scope.singles.len(), 1);
        assert_eq!(
            context.scope.singles.capacity(),
            1,
            "must exercise nonempty replacement"
        );
        let first = context.scope.singles[0];
        let second = self.find(1, context)?;
        assert_eq!(context.scope.singles[0], first);
        Ok(second)
    }
}

fn growth_sibling(source: &SemanticFunctionDeclV1) -> usize {
    // Independent conservative high-water separator. Earlier proofs own one
    // Scratch plus an outside mask, not a paid pre-emission graph here.
    let n = source.blocks().len();
    std::mem::size_of::<Scope>()
        + std::mem::size_of::<(&SemanticOperandV1, PendingSingle)>()
        + std::mem::size_of::<Scratch>()
        + std::mem::size_of::<Vec<bool>>()
        + std::mem::size_of::<Vec<Single>>()
        + 2 * std::mem::size_of::<Single>()
        + n * (3 + std::mem::size_of::<usize>())
}

#[derive(Clone, Copy)]
struct GrowthReference {
    prefix_work: usize,
    old_floor: usize,
    old_capacity: usize,
    new_capacity: usize,
    header_extent: usize,
    replacement_extent: usize,
    sibling: usize,
}

fn growth_reference(plan: &GrowthPlan<'_>) -> GrowthReference {
    let sibling = growth_sibling(plan.source);
    let mut checkpoint = None;
    let prefix = run(usize::MAX, usize::MAX, |context| {
        let _proof = plan.pending_second(context)?;
        let old_capacity = context.scope.singles.capacity();
        let old_floor = SIBLING
            + std::mem::size_of::<Scope>()
            + old_capacity * std::mem::size_of::<Single>()
            + std::mem::size_of::<(&SemanticOperandV1, PendingSingle)>();
        assert_eq!(context.facts.scalar_private_storage_v1()?, old_floor);
        context.facts.reserve_scalar_private_storage_v1(sibling)?;
        // This reference never calls the observed second bind. Its explicit
        // 8-unit comparison and two allocation boundaries precede row movement.
        context.charge(8)?;
        let header_extent = old_floor + sibling + std::mem::size_of::<Vec<Single>>();
        context.reserve(std::mem::size_of::<Vec<Single>>())?;
        assert_eq!(context.facts.scalar_private_storage_v1()?, header_extent);
        let replacement = context.backing::<Single>(old_capacity * 2)?;
        let new_capacity = replacement.capacity();
        assert_eq!(new_capacity, old_capacity * 2);
        let replacement_extent = header_extent + new_capacity * std::mem::size_of::<Single>();
        assert_eq!(
            context.facts.scalar_private_storage_v1()?,
            replacement_extent
        );
        assert_eq!(context.scope.singles.len(), 1);
        checkpoint = Some((
            old_floor,
            old_capacity,
            new_capacity,
            header_extent,
            replacement_extent,
        ));
        drop(replacement);
        context.facts.release_scalar_private_storage_v1(sibling)?;
        Ok(())
    });
    success(&prefix);
    let (old_floor, old_capacity, new_capacity, header_extent, replacement_extent) =
        checkpoint.unwrap();
    assert_eq!(
        prefix.peak, replacement_extent,
        "growth reference is above all earlier peaks"
    );
    GrowthReference {
        prefix_work: prefix.work,
        old_floor,
        old_capacity,
        new_capacity,
        header_extent,
        replacement_extent,
        sibling,
    }
}

fn observe_growth(
    plan: &GrowthPlan<'_>,
    reference: GrowthReference,
    context: &mut Context<'_, '_>,
) -> Result<()> {
    let proof = plan.pending_second(context)?;
    let old = context.scope.singles[0];
    assert_eq!(context.scope.singles.capacity(), reference.old_capacity);
    assert_eq!(
        context.facts.scalar_private_storage_v1()?,
        reference.old_floor
    );
    context
        .facts
        .reserve_scalar_private_storage_v1(reference.sibling)?;
    let result = bind_single(proof, plan.ordered[1].initial, context);
    assert_eq!(context.scope.singles[0], old);
    if result.is_ok() {
        assert_eq!(context.scope.registered, 2);
        assert_eq!(context.scope.singles.len(), 2);
        assert_eq!(context.scope.singles.capacity(), reference.new_capacity);
        assert_eq!(
            context.scope.singles[1],
            Single {
                source: proof,
                initial: plan.ordered[1].initial
            }
        );
        let retained =
            std::mem::size_of::<Scope>() + reference.new_capacity * std::mem::size_of::<Single>();
        assert_eq!(
            context.scope.retained, retained,
            "old backing/header/pending retired once"
        );
        assert_eq!(
            context.facts.scalar_private_storage_v1()?,
            SIBLING + reference.sibling + retained
        );
    } else {
        assert_eq!(context.scope.registered, 1);
        assert_eq!(context.scope.singles.len(), 1);
        assert_eq!(context.scope.singles.capacity(), reference.old_capacity);
    }
    context
        .facts
        .release_scalar_private_storage_v1(reference.sibling)?;
    result
}

#[test]
fn nonempty_growth_exact_header_backing_movement_and_completion_custody() {
    for orientation in [false, true] {
        let (types, source) = two_separated(orientation, true);
        let rows = actual_rows(&types, &source, true);
        let plan = GrowthPlan::new(&source, &rows);
        let reference = growth_reference(&plan);
        let action = |context: &mut Context<'_, '_>| observe_growth(&plan, reference, context);
        expect_storage(
            run(usize::MAX, reference.header_extent - 1, action),
            reference.header_extent,
            reference.header_extent - 1,
            reference.prefix_work,
            reference.old_floor + reference.sibling,
        );
        expect_storage(
            run(usize::MAX, reference.replacement_extent - 1, action),
            reference.replacement_extent,
            reference.replacement_extent - 1,
            reference.prefix_work,
            reference.header_extent,
        );
        let final_work = reference.prefix_work + 1;
        expect_work(
            run(reference.prefix_work, reference.replacement_extent, action),
            final_work,
            reference.prefix_work,
            reference.prefix_work,
            reference.replacement_extent,
        );
        let completed = run(final_work, reference.replacement_extent, action);
        success(&completed);
        assert_eq!(
            (
                completed.work,
                completed.peak,
                completed.failed_storage,
                completed.failed_work
            ),
            (final_work, reference.replacement_extent, None, None)
        );
    }
}

#[test]
fn nonempty_growth_preserves_prior_denial_receipts_and_cumulative_work() {
    let (types, source) = two_separated(true, true);
    let rows = actual_rows(&types, &source, true);
    let plan = GrowthPlan::new(&source, &rows);
    let reference = growth_reference(&plan);
    let final_work = reference.prefix_work + 1;
    let mut work = Work::new(final_work + 23);
    {
        let mut budget = Budget::new(&mut work, reference.replacement_extent);
        budget.reserve_storage(SIBLING).unwrap();
        budget.charge_work(23).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        assert!(matches!(
            budget.charge_work(final_work + 1),
            Err(Resource::Work(_))
        ));
        assert!(matches!(
            budget.reserve_storage(reference.replacement_extent),
            Err(Resource::Storage(_))
        ));
        let result = with_scope(&mut Facts(&mut budget), |scope, facts| {
            observe_growth(&plan, reference, &mut Context { scope, facts })?;
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
                final_work + 23,
                SIBLING,
                reference.replacement_extent,
                Some(reference.replacement_extent + SIBLING)
            )
        );
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
    assert_eq!(work.failed_work(), Some(final_work + 24));
}

#[test]
fn two_separated_rows_drop_before_scope_refund_during_unwind() {
    let (types, source) = two_separated(false, true);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(SIBLING).unwrap();
    budget.charge_work(23).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_scope(&mut Facts(&mut budget), |scope, facts| {
            let rows = project(&types, &source, Some(&mut Context { scope, facts }))?;
            assert_two_records(&source, &rows, scope, true);
            assert_eq!(scope.singles.capacity(), 2);
            facts.0.reserve_storage(31).unwrap();
            panic!("both retained separated proofs must retire under live reservation");
        })
    }));
    assert!(panic.is_err());
    assert_eq!(budget.storage(), SIBLING + 31);
    assert!(budget.work() > 23);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.failed_storage(), None);
}
