// Lexical, source-owned last-use schedule. These rows end logical holders, not
// source storage lifetimes, and only after a whole statement has succeeded.
struct SourceReferenceLivenessV29<'a, 'source> {
    instances: &'a ExecutionInstancesV29<'source>,
    instance: ProductionCallInstanceIdV1,
    slot: usize,
    ledger: ArgumentLedgerV1,
    floor: usize,
    blocks: Vec<std::ops::Range<usize>>,
    statements: Vec<std::ops::Range<usize>>,
    deaths: Vec<SemanticLocalIdV1>,
}

fn source_reference_liveness_error_v29() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29("source reference original-use liveness mismatch")
}

fn source_reference_liveness_headers_v29<R>() -> Result<usize, ArgumentResourceV1> {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticBasicBlockV1, SemanticControlFlowEdgeV1, SemanticLocalDeclV1, SemanticStatementV1,
    };
    use fe2o3_mir_model::{SsaEventV1, SsaResolvedEventV1};
    use fe2o3_pliron::{
        ProductionSemanticSsaFunctionOccurrencesV1, ProductionSemanticSsaOccurrenceSiteV1,
    };
    argument_sum_v1(&[
        std::mem::size_of::<SourceReferenceLivenessV29<'_, '_>>(),
        std::mem::size_of::<Option<SourceReferenceLivenessV29<'_, '_>>>(),
        std::mem::size_of::<Result<SourceReferenceLivenessV29<'_, '_>, ProductionSemanticKirErrorV1>>(
        ),
        std::mem::size_of::<std::thread::Result<Result<R, ProductionSemanticKirErrorV1>>>(),
        argument_product_v1(
            3,
            std::mem::size_of::<Result<R, ProductionSemanticKirErrorV1>>(),
        )?,
        std::mem::size_of::<&production_call_instances_v1::ProductionCallInstanceV1<'_>>(),
        std::mem::size_of::<Option<&production_call_instances_v1::ProductionCallInstanceV1<'_>>>(),
        std::mem::size_of::<&SemanticFunctionDeclV1>(),
        std::mem::size_of::<&fe2o3_mir_model::SsaConstructionPlanV1>(),
        std::mem::size_of::<ProductionSemanticSsaFunctionOccurrencesV1<'_>>(),
        std::mem::size_of::<Option<ProductionSemanticSsaFunctionOccurrencesV1<'_>>>(),
        argument_product_v1(4, std::mem::size_of::<std::ops::Range<usize>>())?,
        std::mem::size_of::<SsaEventV1>(),
        std::mem::size_of::<Option<SsaResolvedEventV1>>(),
        std::mem::size_of::<ProductionSemanticSsaOccurrenceSiteV1>(),
        argument_product_v1(
            6,
            std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
        )?,
        argument_product_v1(
            4,
            std::mem::size_of::<&[fe2o3_mir_model::SsaVariableIdV1]>(),
        )?,
        std::mem::size_of::<Option<&[fe2o3_mir_model::SsaVariableIdV1]>>(),
        std::mem::size_of::<Option<&[fe2o3_mir_model::SsaArgumentV1]>>(),
        std::mem::size_of::<&[fe2o3_mir_model::SsaArgumentV1]>(),
        std::mem::size_of::<Option<&[(u32, SsaResolvedEventV1)]>>(),
        std::mem::size_of::<&[(u32, SsaResolvedEventV1)]>(),
        std::mem::size_of::<SourceReferenceLocalV29>(),
        source_reference_liveness_borrowed_headers_v29::<SourceReferenceNodeV29>()?,
        argument_product_v1(4, std::mem::size_of::<Option<usize>>())?,
        argument_product_v1(32, std::mem::size_of::<usize>())?,
        argument_product_v1(8, std::mem::size_of::<&usize>())?,
        argument_product_v1(
            4,
            std::mem::size_of::<Result<u32, std::num::TryFromIntError>>(),
        )?,
        source_reference_liveness_borrowed_headers_v29::<SourceReferenceLivenessV29<'_, '_>>()?,
        source_reference_liveness_borrowed_headers_v29::<SourceReferencePlanV29<'_, '_>>()?,
        source_reference_liveness_borrowed_headers_v29::<SourceReferenceBuilderV29<'_, '_, '_>>()?,
        source_reference_liveness_borrowed_headers_v29::<ArgumentBudgetV1<'_>>()?,
        source_reference_liveness_borrowed_headers_v29::<SemanticBasicBlockV1>()?,
        source_reference_liveness_borrowed_headers_v29::<[SemanticBasicBlockV1]>()?,
        source_reference_liveness_borrowed_headers_v29::<[SemanticLocalDeclV1]>()?,
        source_reference_liveness_borrowed_headers_v29::<[SemanticStatementV1]>()?,
        source_reference_liveness_borrowed_headers_v29::<[SemanticLocalIdV1]>()?,
        source_reference_liveness_borrowed_headers_v29::<std::ops::Range<usize>>()?,
        source_reference_liveness_borrowed_headers_v29::<[(u32, SsaResolvedEventV1)]>()?,
        source_reference_liveness_borrowed_headers_v29::<(u32, SsaResolvedEventV1)>()?,
        source_reference_liveness_borrowed_headers_v29::<fe2o3_mir_model::SsaArgumentV1>()?,
        source_reference_liveness_borrowed_headers_v29::<bool>()?,
        source_reference_liveness_borrowed_headers_v29::<usize>()?,
        argument_product_v1(
            2,
            source_reference_liveness_borrowed_headers_v29::<
                fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1,
            >()?,
        )?,
        source_reference_liveness_borrowed_headers_v29::<
            [fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1],
        >()?,
        source_reference_liveness_borrowed_headers_v29::<
            fe2o3_pliron::ProductionSemanticSsaSuccessorOccurrenceV1,
        >()?,
        source_reference_liveness_borrowed_headers_v29::<
            [fe2o3_pliron::ProductionSemanticSsaSuccessorOccurrenceV1],
        >()?,
        source_reference_liveness_borrowed_headers_v29::<
            fe2o3_pliron::ProductionSemanticSsaEdgeDefinitionOccurrenceV1,
        >()?,
        source_reference_liveness_borrowed_headers_v29::<
            [fe2o3_pliron::ProductionSemanticSsaEdgeDefinitionOccurrenceV1],
        >()?,
        std::mem::size_of::<std::slice::Iter<'_, SemanticBasicBlockV1>>(),
        std::mem::size_of::<std::iter::Enumerate<std::slice::Iter<'_, SemanticBasicBlockV1>>>(),
        argument_product_v1(
            2,
            std::mem::size_of::<std::slice::Iter<'_, fe2o3_mir_model::SsaVariableIdV1>>(),
        )?,
        std::mem::size_of::<
            std::slice::Iter<'_, fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1>,
        >(),
        std::mem::size_of::<
            std::iter::Rev<
                std::slice::Iter<'_, fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1>,
            >,
        >(),
        std::mem::size_of::<std::iter::Rev<std::ops::Range<usize>>>(),
        std::mem::size_of::<std::slice::Iter<'_, SemanticLocalIdV1>>(),
        std::mem::size_of::<SemanticControlFlowEdgeV1>(),
        std::mem::size_of::<fe2o3_mir_model::SsaEdgeIdV1>(),
        std::mem::size_of::<fe2o3_mir_model::SsaResolvedEventV1>(),
        argument_product_v1(
            2,
            std::mem::size_of::<Option<ProductionSemanticKirErrorV1>>(),
        )?,
        std::mem::size_of::<Result<(), ArgumentResourceV1>>(),
        // Scope/constructor captures, excluding the independently measured
        // caller consumer, plus the nested scratch result/catch envelopes.
        std::mem::size_of::<(
            &mut SourceReferenceBuilderV29<'_, '_, '_>,
            &mut ArgumentBudgetV1<'_>,
            &mut Option<SourceReferenceLivenessV29<'_, '_>>,
            &mut usize,
            &mut bool,
            &usize,
            &usize,
            &ProductionCallInstanceIdV1,
        )>(),
        std::mem::size_of::<(
            &SourceReferenceBuilderV29<'_, '_, '_>,
            &mut ArgumentBudgetV1<'_>,
            &ProductionCallInstanceIdV1,
            &usize,
        )>(),
        argument_product_v1(
            2,
            std::mem::size_of::<std::thread::Result<Result<(), ProductionSemanticKirErrorV1>>>(),
        )?,
        // Exact borrowed captures of the per-block edge visitor. It does not
        // retain an independent edge or source inventory.
        std::mem::size_of::<(
            &SourceReferencePlanV29<'_, '_>,
            &fe2o3_mir_model::SsaConstructionPlanV1,
            &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
            &mut ArgumentBudgetV1<'_>,
            &fe2o3_mir_model::SsaBlockIdV1,
            &mut u32,
            &mut usize,
            &mut usize,
            &Vec<bool>,
            &mut Vec<usize>,
            &mut Vec<usize>,
            &mut usize,
            &usize,
            &bool,
        )>(),
    ])
}

fn source_reference_liveness_borrowed_headers_v29<T: ?Sized>() -> Result<usize, ArgumentResourceV1>
{
    argument_sum_v1(&[
        std::mem::size_of::<&T>(),
        std::mem::size_of::<Option<&T>>(),
        std::mem::size_of::<Result<&T, ProductionSemanticKirErrorV1>>(),
    ])
}

fn source_reference_liveness_site_v29(
    site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
) -> (u32, Option<u32>) {
    use fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1 as Site;
    match site {
        Site::Statement { block, statement } => (block.get(), Some(statement)),
        Site::Terminator { block } => (block.get(), None),
    }
}

impl<'a, 'source> SourceReferenceLivenessV29<'a, 'source> {
    fn new(
        plan: &SourceReferencePlanV29<'a, 'source>,
        instance: ProductionCallInstanceIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        use fe2o3_mir_model::{SsaBlockIdV1, SsaEdgeIdV1, SsaEventV1, SsaResolvedEventV1};
        plan.check_owner(plan.instances, budget)?;
        plan.charge(8, budget)?;
        let row = plan
            .instances
            .instance(instance)
            .ok_or_else(source_reference_liveness_error_v29)?;
        let function = row.declaration();
        let ssa = row.ssa().plan();
        let occurrences = plan
            .instances
            .occurrences(instance)
            .ok_or_else(source_reference_liveness_error_v29)?;
        if !std::ptr::eq(occurrences.owner(), plan.instances.owner())
            || occurrences.function() != row.function()
            || row.ssa().function_identity() != function.identity()
        {
            return Err(source_reference_liveness_error_v29());
        }
        let mut statement_count = 0usize;
        for block in function.blocks() {
            plan.charge(1, budget)?;
            u32::try_from(block.statements().len()).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            statement_count = argument_sum_v1(&[statement_count, block.statements().len()])?;
        }
        let mut blocks = source_reference_scratch_v29(function.blocks().len(), budget)?;
        let mut statements = source_reference_scratch_v29(statement_count, budget)?;
        let mut deaths = source_reference_scratch_v29(occurrences.events().len(), budget)?;
        plan.charge(statement_count, budget)?;
        statements.resize(statement_count, 0..0);
        // Only these three outer vectors escape. All reconstruction workspaces
        // are dropped before the nested scratch ledger is restored.
        let construct = |budget: &mut ArgumentBudgetV1<'_>| {
            let count = function.locals().len();
            let mut promoted = source_reference_scratch_v29(count, budget)?;
            let mut live = source_reference_scratch_v29(count, budget)?;
            let mut defined = source_reference_scratch_v29(count, budget)?;
            let mut seen = source_reference_scratch_v29(count, budget)?;
            plan.charge(argument_product_v1(4, count)?, budget)?;
            promoted.resize(count, false);
            live.resize(count, 0usize);
            defined.resize(count, 0usize);
            seen.resize(count, 0usize);
            for variable in ssa.promoted_variables() {
                plan.charge(3, budget)?;
                let slot = promoted
                    .get_mut(variable.get() as usize)
                    .ok_or_else(source_reference_liveness_error_v29)?;
                if *slot {
                    return Err(source_reference_liveness_error_v29());
                }
                *slot = true;
            }
            let mut events = 0usize;
            let mut successors = 0usize;
            let mut definitions = 0usize;
            let mut statement_base = 0usize;
            for (block_index, block) in function.blocks().iter().enumerate() {
                plan.charge(10, budget)?;
                let block_id = SsaBlockIdV1::new(
                    u32::try_from(block_index).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                );
                let epoch = argument_sum_v1(&[block_index, 1])?;
                let reachable = ssa.is_reachable(block_id);
                let start = events;
                let mut previous_statement = Some(0u32);
                let resolved = ssa.resolved_events(block_id);
                let mut resolved_index = 0usize;
                while let Some(event) = occurrences.events().get(events) {
                    plan.charge(10, budget)?;
                    let (source_block, statement) =
                        source_reference_liveness_site_v29(event.site());
                    if source_block != block_id.get() {
                        if source_block < block_id.get() {
                            return Err(source_reference_liveness_error_v29());
                        }
                        break;
                    }
                    if statement.is_some_and(|ordinal| ordinal as usize >= block.statements().len())
                        || (previous_statement.is_none() && statement.is_some())
                        || matches!((previous_statement, statement), (Some(left), Some(right)) if left > right)
                        || event.ordinal() as usize != events - start
                        || event.is_reachable() != reachable
                    {
                        return Err(source_reference_liveness_error_v29());
                    }
                    previous_statement = statement;
                    let variable = event.event().variable().get() as usize;
                    let is_promoted = *promoted
                        .get(variable)
                        .ok_or_else(source_reference_liveness_error_v29)?;
                    if event.is_promoted() != is_promoted {
                        return Err(source_reference_liveness_error_v29());
                    }
                    if reachable && is_promoted {
                        let &(ordinal, actual) = resolved
                            .and_then(|rows| rows.get(resolved_index))
                            .ok_or_else(source_reference_liveness_error_v29)?;
                        if ordinal != event.ordinal()
                            || event.resolved() != Some(actual)
                            || !matches!((event.event(), actual),
                                (SsaEventV1::Use(a), SsaResolvedEventV1::Use { variable: b, .. })
                                | (SsaEventV1::Define(a), SsaResolvedEventV1::Define { variable: b, .. })
                                | (SsaEventV1::Kill(a), SsaResolvedEventV1::Kill { variable: b, .. }) if a == b)
                        {
                            return Err(source_reference_liveness_error_v29());
                        }
                        resolved_index += 1;
                    } else if event.resolved().is_some() {
                        return Err(source_reference_liveness_error_v29());
                    }
                    events += 1;
                }
                if resolved.map_or(0, <[_]>::len) != resolved_index {
                    return Err(source_reference_liveness_error_v29());
                }
                let end = events;
                let mut live_count = 0usize;
                let mut ordinal = 0u32;
                block.terminator().kind().try_for_each_edge(|edge| {
                    plan.charge(10, budget)?;
                    let edge_id = SsaEdgeIdV1::new(block_id, ordinal);
                    ordinal = ordinal
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    let row = occurrences
                        .successors()
                        .get(successors)
                        .ok_or_else(source_reference_liveness_error_v29)?;
                    if row.id() != edge_id || row.edge() != edge {
                        return Err(source_reference_liveness_error_v29());
                    }
                    successors += 1;
                    let stamp = successors;
                    let actual = ssa.edge_definitions(edge_id);
                    let mut actual_index = 0usize;
                    let mut definition_ordinal = 0usize;
                    while let Some(definition) = occurrences.edge_definitions().get(definitions) {
                        plan.charge(8, budget)?;
                        if definition.edge() != edge_id {
                            break;
                        }
                        let variable = definition.variable().get() as usize;
                        let is_promoted = *promoted
                            .get(variable)
                            .ok_or_else(source_reference_liveness_error_v29)?;
                        if definition.ordinal() as usize != definition_ordinal
                            || definition.is_reachable() != reachable
                            || definition.is_promoted() != is_promoted
                        {
                            return Err(source_reference_liveness_error_v29());
                        }
                        if reachable && is_promoted {
                            let actual = actual
                                .and_then(|rows| rows.get(actual_index))
                                .ok_or_else(source_reference_liveness_error_v29)?;
                            if actual.variable() != definition.variable()
                                || Some(actual.value()) != definition.value()
                                || defined[variable] == stamp
                            {
                                return Err(source_reference_liveness_error_v29());
                            }
                            defined[variable] = stamp;
                            actual_index += 1;
                        } else if definition.value().is_some() {
                            return Err(source_reference_liveness_error_v29());
                        }
                        definitions += 1;
                        definition_ordinal += 1;
                    }
                    if actual.map_or(0, <[_]>::len) != actual_index {
                        return Err(source_reference_liveness_error_v29());
                    }
                    if reachable {
                        let target = SsaBlockIdV1::new(edge.target().index());
                        for variable in ssa
                            .live_in(target)
                            .ok_or_else(source_reference_liveness_error_v29)?
                        {
                            plan.charge(4, budget)?;
                            let variable = variable.get() as usize;
                            if promoted.get(variable) != Some(&true) {
                                return Err(source_reference_liveness_error_v29());
                            }
                            if defined[variable] != stamp && live[variable] != epoch {
                                live[variable] = epoch;
                                live_count += 1;
                            }
                        }
                    }
                    Ok::<(), ProductionSemanticKirErrorV1>(())
                })?;
                let failure = occurrences.terminal_failure_start(block_id);
                if failure.is_some_and(|first| first > end - start) {
                    return Err(source_reference_liveness_error_v29());
                }
                let mut cursor = end;
                while cursor > start
                    && source_reference_liveness_site_v29(occurrences.events()[cursor - 1].site())
                        .1
                        .is_none()
                {
                    cursor -= 1;
                    plan.charge(4, budget)?;
                    let event = &occurrences.events()[cursor];
                    let failure_tail =
                        failure.is_some_and(|first| event.ordinal() as usize >= first);
                    if failure_tail && matches!(event.event(), SsaEventV1::Define(_)) {
                        return Err(source_reference_liveness_error_v29());
                    }
                    source_reference_liveness_reverse_v29(
                        event.event(),
                        reachable && event.is_promoted(),
                        failure_tail,
                        epoch,
                        &mut live,
                        &mut live_count,
                    )?;
                }
                if failure.is_some_and(|first| first < cursor - start) {
                    return Err(source_reference_liveness_error_v29());
                }
                let statement_end = argument_sum_v1(&[statement_base, block.statements().len()])?;
                blocks.push(statement_base..statement_end);
                for statement in (0..block.statements().len()).rev() {
                    plan.charge(4, budget)?;
                    let event_end = cursor;
                    while cursor > start
                        && source_reference_liveness_site_v29(
                            occurrences.events()[cursor - 1].site(),
                        )
                        .1 == Some(statement as u32)
                    {
                        plan.charge(1, budget)?;
                        cursor -= 1;
                    }
                    let death_start = deaths.len();
                    let stamp = argument_sum_v1(&[statement_base, statement, 1])?;
                    for event in &occurrences.events()[cursor..event_end] {
                        plan.charge(5, budget)?;
                        let variable = event.event().variable().get() as usize;
                        if reachable
                            && event.is_promoted()
                            && live[variable] != epoch
                            && seen[variable] != stamp
                        {
                            seen[variable] = stamp;
                            if deaths.len() == deaths.capacity() {
                                return Err(ArgumentResourceV1::Accounting.into());
                            }
                            deaths.push(SemanticLocalIdV1::from_index(
                                event.event().variable().get(),
                            ));
                        }
                    }
                    statements[statement_base + statement] = death_start..deaths.len();
                    for event in occurrences.events()[cursor..event_end].iter().rev() {
                        plan.charge(4, budget)?;
                        source_reference_liveness_reverse_v29(
                            event.event(),
                            reachable && event.is_promoted(),
                            false,
                            epoch,
                            &mut live,
                            &mut live_count,
                        )?;
                    }
                }
                if cursor != start {
                    return Err(source_reference_liveness_error_v29());
                }
                let expected = ssa.live_in(block_id);
                if expected.map_or(0, <[_]>::len) != live_count {
                    return Err(source_reference_liveness_error_v29());
                }
                for variable in expected.unwrap_or(&[]) {
                    plan.charge(2, budget)?;
                    if live.get(variable.get() as usize) != Some(&epoch) {
                        return Err(source_reference_liveness_error_v29());
                    }
                }
                statement_base = statement_end;
            }
            if events != occurrences.events().len()
                || successors != occurrences.successors().len()
                || definitions != occurrences.edge_definitions().len()
                || statement_base != statement_count
            {
                return Err(source_reference_liveness_error_v29());
            }
            Ok(())
        };
        budget.reserve_storage(std::mem::size_of_val(&construct))?;
        with_canonical_call_scratch_v1(budget, construct)?;
        Ok(Self {
            instances: plan.instances,
            instance,
            slot: budget as *const ArgumentBudgetV1<'_> as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
            blocks,
            statements,
            deaths,
        })
    }

    fn completed<'b>(
        &'b self,
        plan: &SourceReferencePlanV29<'_, '_>,
        site: SourceReferenceSiteV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'b [SemanticLocalIdV1], ProductionSemanticKirErrorV1> {
        plan.check_owner(self.instances, budget)?;
        if self.slot != budget as *const ArgumentBudgetV1<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            plan.failure.record_resource(ArgumentResourceV1::Accounting);
            return Err(ArgumentResourceV1::Accounting.into());
        }
        plan.charge(6, budget)?;
        if site.instance != self.instance {
            return Err(source_reference_liveness_error_v29());
        }
        let block = self
            .blocks
            .get(site.block.index() as usize)
            .ok_or_else(source_reference_liveness_error_v29)?;
        let statement = site
            .statement
            .filter(|ordinal| *ordinal < block.len())
            .ok_or_else(source_reference_liveness_error_v29)?;
        let deaths = self
            .statements
            .get(block.start + statement)
            .ok_or_else(source_reference_liveness_error_v29)?;
        self.deaths
            .get(deaths.clone())
            .ok_or_else(source_reference_liveness_error_v29)
    }
}

fn source_reference_liveness_reverse_v29(
    event: fe2o3_mir_model::SsaEventV1,
    promoted: bool,
    failure_tail: bool,
    epoch: usize,
    live: &mut [usize],
    count: &mut usize,
) -> Result<(), ProductionSemanticKirErrorV1> {
    use fe2o3_mir_model::SsaEventV1;
    if !promoted {
        return Ok(());
    }
    let slot = live
        .get_mut(event.variable().get() as usize)
        .ok_or_else(source_reference_liveness_error_v29)?;
    match event {
        SsaEventV1::Use(_) if *slot != epoch => {
            *slot = epoch;
            *count += 1;
        }
        SsaEventV1::Define(_) | SsaEventV1::Kill(_) if !failure_tail && *slot == epoch => {
            *slot = 0;
            *count -= 1;
        }
        _ => {}
    }
    Ok(())
}

fn with_source_reference_liveness_v29<'a, 'root, 'source, 'work, R: Copy>(
    builder: &mut SourceReferenceBuilderV29<'a, 'root, 'source>,
    instance: ProductionCallInstanceIdV1,
    budget: &mut ArgumentBudgetV1<'work>,
    consumer: impl FnOnce(
        &mut SourceReferenceBuilderV29<'a, 'root, 'source>,
        &SourceReferenceLivenessV29<'a, 'source>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    builder.plan.check_owner(builder.plan.instances, budget)?;
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const ArgumentBudgetV1<'_> as usize;
    let mut context = None;
    let mut owned = 0usize;
    let mut constructing = true;
    let consumer_headers = std::mem::size_of_val(&consumer);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let construction = (|| {
            budget.reserve_storage(argument_sum_v1(&[
                source_reference_liveness_headers_v29::<R>()?,
                consumer_headers,
            ])?)?;
            SourceReferenceLivenessV29::new(&builder.plan, instance, budget)
        })();
        owned = budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        constructing = false;
        context = Some(construction?);
        consumer(builder, context.as_ref().unwrap(), budget)
    }));
    if constructing {
        owned = budget.storage().checked_sub(floor).unwrap_or(0);
    }
    if let Ok(Err(error)) = &outcome {
        source_reference_record_failure_v29(&builder.plan, error);
    }
    let first = builder.plan.failure.first_error();
    drop(context);
    // Consumer allocations stay retained. Never roll the whole builder back to
    // this lexical floor; only the dropped schedule's exact bytes are released.
    let cleanup = if slot == budget as *const ArgumentBudgetV1<'_> as usize
        && ledger == budget.work_ledger_identity_v1()
        && floor
            .checked_add(owned)
            .is_some_and(|required| budget.storage() >= required)
    {
        budget.release_storage(owned)
    } else {
        Err(ArgumentResourceV1::Accounting)
    };
    if let Err(error) = cleanup {
        builder.plan.failure.record_resource(error);
    }
    match outcome {
        Ok(Err(error)) => Err(first.unwrap_or(error)),
        Ok(Ok(value)) => match builder.plan.failure.first_error() {
            Some(error) => Err(error),
            None => Ok(value),
        },
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn expire_completed_statement_v29(
        &mut self,
        liveness: &SourceReferenceLivenessV29<'_, '_>,
        site: SourceReferenceSiteV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let result = (|| {
            for &local in liveness.completed(&self.plan, site, budget)? {
                self.plan.charge(4, budget)?;
                let mut value = self.local(site.instance, local)?;
                // The authenticated schedule contains only owner-promoted
                // locals, excluding retained-memory holders. A logical storage
                // snapshot exists even for promoted values; leave that snapshot
                // and its generation intact while ending only the dead alias.
                if let Some(node) = value.node {
                    let candidate = self
                        .plan
                        .nodes
                        .get(node)
                        .ok_or_else(source_reference_liveness_error_v29)?;
                    if matches!(
                        candidate.kind,
                        SourceReferenceNodeKindV29::Loan(_)
                            | SourceReferenceNodeKindV29::Aggregate { .. }
                    ) && self.expirable_primitive_holder_v29(node, budget)?
                    {
                        value.node = None;
                        self.set_local(site.instance, local, value)?;
                    }
                }
            }
            Ok(())
        })();
        result.inspect_err(|error| source_reference_record_failure_v29(&self.plan, error))
    }

    fn expirable_primitive_holder_v29(
        &self,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        use fe2o3_mir_model::semantic_mir_v1::{SemanticAggregateTypeV1, SemanticPointerTypeV1};
        with_canonical_call_scratch_v1(budget, |budget| {
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of::<SourceReferenceNodeV29>(),
                std::mem::size_of::<&SourceReferenceLoanV29>(),
                argument_product_v1(2, std::mem::size_of::<Option<&SemanticTypeShapeV1>>())?,
                argument_product_v1(2, std::mem::size_of::<&SemanticTypeShapeV1>())?,
                std::mem::size_of::<&SemanticPointerTypeV1>(),
                std::mem::size_of::<&[SemanticTypeIdV1]>(),
                std::mem::size_of::<std::ops::Range<usize>>(),
                argument_product_v1(
                    2,
                    std::mem::size_of::<Result<bool, ProductionSemanticKirErrorV1>>(),
                )?,
                argument_product_v1(8, std::mem::size_of::<usize>())?,
                source_reference_liveness_borrowed_headers_v29::<SourceReferenceNodeV29>()?,
                source_reference_liveness_borrowed_headers_v29::<SourceReferenceLoanV29>()?,
                source_reference_liveness_borrowed_headers_v29::<[SemanticTypeDeclV1]>()?,
                argument_product_v1(
                    2,
                    source_reference_liveness_borrowed_headers_v29::<SemanticTypeDeclV1>()?,
                )?,
                source_reference_liveness_borrowed_headers_v29::<SemanticAggregateTypeV1>()?,
                source_reference_liveness_borrowed_headers_v29::<usize>()?,
                std::mem::size_of::<Option<SemanticTypeIdV1>>(),
                std::mem::size_of::<Option<usize>>(),
                std::mem::size_of::<std::iter::Enumerate<std::ops::Range<usize>>>(),
                std::mem::size_of::<(&Self, &usize)>(),
                std::mem::size_of::<std::thread::Result<Result<bool, ProductionSemanticKirErrorV1>>>(),
            ])?)?;
            let mut pending = source_reference_scratch_v29(1, budget)?;
            pending.push(root);
            let types = self.plan.instances.owner().source_semantic().types();
            let mut has_loan = false;
            while let Some(index) = pending.pop() {
                self.plan.charge(8, budget)?;
                let node = self
                    .plan
                    .nodes
                    .get(index)
                    .ok_or_else(source_reference_liveness_error_v29)?;
                let shape = types
                    .get(node.ty.index() as usize)
                    .map(SemanticTypeDeclV1::shape)
                    .ok_or_else(source_reference_liveness_error_v29)?;
                match (node.kind, shape) {
                    (
                        SourceReferenceNodeKindV29::Loan(loan),
                        SemanticTypeShapeV1::Pointer(pointer),
                    ) => {
                        let loan = self
                            .plan
                            .loans
                            .get(loan)
                            .ok_or_else(source_reference_liveness_error_v29)?;
                        if pointer.kind() != SemanticPointerKindV1::Reference
                            || pointer.metadata() != SemanticPointerMetadataV1::None
                            || loan.source_type != node.ty
                            || !matches!(
                                loan.kind,
                                SemanticBorrowKindV1::Shared | SemanticBorrowKindV1::Mutable
                            )
                            || !matches!(
                                types
                                    .get(pointer.pointee().index() as usize)
                                    .map(SemanticTypeDeclV1::shape),
                                Some(
                                    SemanticTypeShapeV1::Scalar(_)
                                        | SemanticTypeShapeV1::ValidityScalar(_)
                                )
                            )
                        {
                            return Ok(false);
                        }
                        has_loan = true;
                    }
                    (
                        SourceReferenceNodeKindV29::Aggregate { first, count },
                        SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields),
                    ) => {
                        if count != fields.fields().len() {
                            return Err(source_reference_liveness_error_v29());
                        }
                        let end = argument_sum_v1(&[first, count])?;
                        for (field, child) in (first..end).enumerate() {
                            self.plan.charge(4, budget)?;
                            let child = *self
                                .plan
                                .children
                                .get(child)
                                .ok_or_else(source_reference_liveness_error_v29)?;
                            if child >= index
                                || self.plan.nodes.get(child).map(|node| node.ty)
                                    != Some(fields.fields()[field])
                            {
                                return Err(source_reference_liveness_error_v29());
                            }
                            emission_push_v1(&mut pending, child, budget)?;
                        }
                    }
                    (
                        SourceReferenceNodeKindV29::Plain(_) | SourceReferenceNodeKindV29::Absent,
                        SemanticTypeShapeV1::Unit
                        | SemanticTypeShapeV1::Scalar(_)
                        | SemanticTypeShapeV1::ValidityScalar(_),
                    ) => {}
                    _ => return Ok(false),
                }
            }
            Ok(has_loan)
        })
    }
}
