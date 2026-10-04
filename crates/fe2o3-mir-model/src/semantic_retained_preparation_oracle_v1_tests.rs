//! Frozen original model algorithm oracles; only local symbol relocation.
use super::*;

pub(super) fn producers(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    budget: &mut WorkBudgetV1<'_>,
) -> Result<Vec<SemanticOptionProducerV1>, SemanticOptionDominanceErrorV1> {
    let mut producers = Vec::new();
    budget.reserve(&mut producers, function.blocks().len())?;
    for block in function.blocks() {
        // Classifier and exact destination checks have constant work. This is
        // external-only admission: collection had no legacy diagnostic counter.
        budget.extra(8)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
            callables.get(call.callee().index() as usize)
        else {
            continue;
        };
        if let Some(producer) = SemanticOptionProducerV1::from_compiler_intrinsic(operation, call)?
        {
            budget.push(&mut producers, producer)?;
        }
    }
    Ok(producers)
}

fn oracle_definitions(
    function: &SemanticFunctionDeclV1,
    budget: &mut WorkBudgetV1,
) -> Result<Vec<u8>, SemanticOptionDominanceErrorV1> {
    budget.charge(function.locals().len())?;
    let mut definitions = Vec::new();
    budget.reserve(&mut definitions, function.locals().len())?;
    definitions.extend(
        function
            .locals()
            .iter()
            .map(|local| u8::from(local.role().is_entry_argument())),
    );
    let mut record = |place: &SemanticPlaceV1| {
        let Some(slot) = definitions.get_mut(place.local().index() as usize) else {
            return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "a semantic definition is outside the local table",
            ));
        };
        *slot = slot.saturating_add(1);
        Ok(())
    };
    for block in function.blocks() {
        budget.charge(block.statements().len().saturating_add(1))?;
        for statement in block.statements() {
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => record(assignment.destination())?,
                SemanticStatementKindV1::Store(store) => record(store.destination())?,
                SemanticStatementKindV1::AtomicRmw(atomic) => record(atomic.destination())?,
                SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                    record(atomic.destination())?
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place) => record(place)?,
                SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::Assume(_)
                | SemanticStatementKindV1::Nop => {}
            }
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && let Some(destination) = call.destination()
        {
            record(destination.place())?;
        }
    }
    Ok(definitions)
}

struct OracleDominators {
    entry: usize,
    preorder: Vec<usize>,
    subtree_end: Vec<usize>,
    predecessors: Vec<Vec<usize>>,
}

impl OracleDominators {
    fn analyze(
        function: &SemanticFunctionDeclV1,
        budget: &mut WorkBudgetV1,
    ) -> Result<Self, SemanticOptionDominanceErrorV1> {
        let block_count = function.blocks().len();
        let entry = function.entry().index() as usize;
        if block_count == 0 || entry >= block_count {
            return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "the semantic CFG has no valid entry block",
            ));
        }
        budget.charge(block_count)?;
        let mut successors = budget.nested(block_count)?;
        let mut predecessors = budget.nested(block_count)?;
        for (source, block) in function.blocks().iter().enumerate() {
            budget.extra(1)?;
            block
                .terminator()
                .kind()
                .try_for_each_edge::<SemanticOptionDominanceErrorV1>(|edge| {
                    budget.charge(1)?;
                    let target = edge.target().index() as usize;
                    if target >= block_count {
                        return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                            "a semantic CFG edge is outside the block table",
                        ));
                    }
                    budget.push(&mut successors[source], target)?;
                    budget.push(&mut predecessors[target], source)?;
                    Ok(())
                })?;
        }

        let mut visited = budget.filled(block_count, false)?;
        let mut postorder = Vec::new();
        budget.reserve(&mut postorder, block_count)?;
        let mut pending = Vec::new();
        budget.push(&mut pending, (entry, false))?;
        while let Some((block, finish)) = pending.pop() {
            budget.charge(1)?;
            if finish {
                budget.push(&mut postorder, block)?;
            } else if !visited[block] {
                visited[block] = true;
                budget.push(&mut pending, (block, true))?;
                for successor in successors[block].iter().rev() {
                    budget.charge(1)?;
                    if !visited[*successor] {
                        budget.push(&mut pending, (*successor, false))?;
                    }
                }
            }
        }
        budget.extra(postorder.len())?;
        postorder.reverse();
        let mut rpo_index = budget.filled(block_count, usize::MAX)?;
        for (index, block) in postorder.iter().copied().enumerate() {
            budget.extra(1)?;
            rpo_index[block] = index;
        }
        let mut immediate = budget.filled(block_count, None)?;
        immediate[entry] = Some(entry);
        loop {
            budget.charge(1)?;
            let mut changed = false;
            for block in postorder.iter().copied().skip(1) {
                budget.charge(1)?;
                budget.extra(predecessors[block].len())?;
                let mut processed = predecessors[block]
                    .iter()
                    .copied()
                    .filter(|predecessor| immediate[*predecessor].is_some());
                let Some(mut next) = processed.next() else {
                    continue;
                };
                for predecessor in processed {
                    budget.charge(1)?;
                    next = intersect_dominator_paths(
                        predecessor,
                        next,
                        &immediate,
                        &rpo_index,
                        budget,
                    )?;
                }
                if immediate[block] != Some(next) {
                    immediate[block] = Some(next);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        immediate[entry] = None;
        let mut children = budget.nested(block_count)?;
        for (block, parent) in immediate.iter().copied().enumerate() {
            budget.extra(1)?;
            if let Some(parent) = parent {
                budget.push(&mut children[parent], block)?;
            }
        }
        let mut preorder = budget.filled(block_count, usize::MAX)?;
        let mut subtree_end = budget.filled(block_count, usize::MAX)?;
        let mut clock = 0_usize;
        let mut pending = Vec::new();
        budget.push(&mut pending, (entry, false))?;
        while let Some((block, finish)) = pending.pop() {
            budget.charge(1)?;
            if finish {
                subtree_end[block] = clock;
            } else {
                preorder[block] = clock;
                clock += 1;
                budget.push(&mut pending, (block, true))?;
                for child in children[block].iter().rev() {
                    budget.push(&mut pending, (*child, false))?;
                }
            }
        }
        Ok(Self {
            entry,
            preorder,
            subtree_end,
            predecessors,
        })
    }

    fn has_unique_predecessor(&self, block: usize, predecessor: usize) -> bool {
        // Entry also has the implicit edge from the function invocation.
        block != self.entry
            && matches!(
                self.predecessors.get(block).map(Vec::as_slice),
                Some([exact]) if *exact == predecessor
            )
    }

    fn is_reachable(&self, block: usize) -> bool {
        self.preorder.get(block).copied() != Some(usize::MAX)
    }

    fn dominates(&self, dominator: usize, block: usize) -> bool {
        dominates_with_intervals(&self.preorder, &self.subtree_end, dominator, block)
    }
}

pub(super) fn option_facts(
    function: &SemanticFunctionDeclV1,
    producers: &[SemanticOptionProducerV1],
    budget: &mut WorkBudgetV1<'_>,
) -> Result<SemanticOptionDominanceV1, SemanticOptionDominanceErrorV1> {
    let local_count = function.locals().len();
    let definitions = oracle_definitions(function, budget)?;
    let dominators = OracleDominators::analyze(function, budget)?;
    let mut discriminants_by_option = budget.nested(local_count)?;
    for (block_index, block) in function.blocks().iter().enumerate() {
        budget.charge(block.statements().len().saturating_add(1))?;
        for statement in block.statements() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Discriminant(place) = assignment.value().kind() else {
                continue;
            };
            if !assignment.destination().projections().is_empty() || !place.projections().is_empty()
            {
                continue;
            }
            let Some(bindings) = discriminants_by_option.get_mut(place.local().index() as usize)
            else {
                return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                    "an Option discriminator source is outside the local table",
                ));
            };
            budget.push(bindings, (block_index, assignment.destination().local()))?;
        }
    }

    let mut availability_by_local = budget.filled(local_count, None)?;
    let mut some_targets = Vec::new();
    budget.reserve(&mut some_targets, producers.len())?;
    for producer in producers {
        budget.charge(1)?;
        let destination_index = producer.option_local().index() as usize;
        if definitions.get(destination_index).copied() != Some(1) {
            return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                "an Option capability local does not have one exact producer",
            ));
        }
        let [(switch_block, discriminator)] = discriminants_by_option
            .get(destination_index)
            .map(Vec::as_slice)
            .ok_or(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "an Option capability destination is outside the local table",
            ))?
        else {
            return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                "an Option capability does not have one exact discriminant binding",
            ));
        };
        if definitions.get(discriminator.index() as usize).copied() != Some(1) {
            return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                "an Option capability discriminator does not have one exact definition",
            ));
        }
        if !dominators.dominates(producer.continuation().index() as usize, *switch_block) {
            return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                "an Option capability discriminator is not dominated by its producer continuation",
            ));
        }
        let switch = function.blocks().get(*switch_block).ok_or(
            SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "an Option discriminator block is outside the block table",
            ),
        )?;
        let SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } = switch.terminator().kind()
        else {
            return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                "an Option capability discriminator is not consumed by its defining block",
            ));
        };
        if exact_operand_local(discriminant) != Some(*discriminator) {
            return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                "an Option capability switch is not bound to its unique discriminator",
            ));
        }
        let some_target = match targets.values() {
            [target] => match target.value() {
                0 => targets.otherwise().target(),
                1 => target.edge().target(),
                _ => {
                    return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                        "an Option capability switch has no exact Some edge",
                    ));
                }
            },
            [zero, one] if zero.value() == 0 && one.value() == 1 => one.edge().target(),
            _ => {
                return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                    "an Option capability switch is not an exact 0/1 branch",
                ));
            }
        };
        if !dominators.is_reachable(some_target.index() as usize) {
            return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "the authenticated Some edge is unreachable",
            ));
        }
        if !dominators.has_unique_predecessor(some_target.index() as usize, *switch_block) {
            return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                "an Option capability Some target is not uniquely controlled by its exact branch",
            ));
        }
        let slot = availability_by_local.get_mut(destination_index).ok_or(
            SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "an Option capability destination is outside the local table",
            ),
        )?;
        if slot
            .replace(SemanticOptionAvailabilityV1(some_targets.len()))
            .is_some()
        {
            return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                "one local has multiple Option capability producers",
            ));
        }
        budget.push(&mut some_targets, some_target)?;
    }
    Ok(SemanticOptionDominanceV1 {
        availability_by_local: budget.boxed(availability_by_local)?,
        some_targets: budget.boxed(some_targets)?,
        dominator_preorder: budget.boxed(dominators.preorder)?,
        dominator_subtree_end: budget.boxed(dominators.subtree_end)?,
        work_units: budget.used,
    })
}

pub(super) fn enum_facts(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    budget: &mut WorkBudgetV1<'_>,
) -> Result<SemanticEnumPayloadDominanceV1, SemanticOptionDominanceErrorV1> {
    let local_count = function.locals().len();
    let definitions = oracle_definitions(function, budget)?;
    let dominators = OracleDominators::analyze(function, budget)?;
    let mut discriminants_by_enum = budget.nested(local_count)?;
    for (block_index, block) in function.blocks().iter().enumerate() {
        budget.charge(block.statements().len().saturating_add(1))?;
        for statement in block.statements() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Discriminant(place) = assignment.value().kind() else {
                continue;
            };
            if !assignment.destination().projections().is_empty() || !place.projections().is_empty()
            {
                continue;
            }
            let Some(bindings) = discriminants_by_enum.get_mut(place.local().index() as usize)
            else {
                return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                    "an enum discriminator source is outside the local table",
                ));
            };
            budget.push(bindings, (block_index, assignment.destination().local()))?;
        }
    }

    let mut availability_by_local = budget.nested(local_count)?;
    let mut payload_targets = Vec::new();
    budget.reserve(&mut payload_targets, local_count)?;
    for (local_index, bindings) in discriminants_by_enum.iter().enumerate() {
        budget.extra(1)?;
        let [(switch_block, discriminator)] = bindings.as_slice() else {
            continue;
        };
        if definitions.get(local_index).copied() != Some(1)
            || definitions.get(discriminator.index() as usize).copied() != Some(1)
        {
            continue;
        }
        let Some(local) = function.locals().get(local_index) else {
            return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "an enum payload local is outside the local table",
            ));
        };
        let Some(ty) = types.get(local.ty().index() as usize) else {
            return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "an enum payload type is outside the type table",
            ));
        };
        let SemanticTypeShapeV1::Enum { variants, .. } = ty.shape() else {
            continue;
        };
        let Some(block) = function.blocks().get(*switch_block) else {
            return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "an enum payload switch is outside the block table",
            ));
        };
        let SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } = block.terminator().kind()
        else {
            continue;
        };
        if exact_operand_local(discriminant) != Some(*discriminator) {
            continue;
        }
        budget.charge(
            variants
                .len()
                .saturating_mul(targets.values().len().saturating_add(1)),
        )?;
        let mut otherwise_variant = None;
        let mut otherwise_is_ambiguous = false;
        for (variant_index, variant) in variants.iter().enumerate() {
            if variant.is_uninhabited()
                || targets
                    .values()
                    .iter()
                    .any(|target| target.value() == variant.discriminant())
            {
                continue;
            }
            if otherwise_variant.replace(variant_index as u32).is_some() {
                otherwise_is_ambiguous = true;
                break;
            }
        }
        // The second variant/target scan is separately admitted externally.
        let second_scan = variants
            .len()
            .checked_mul(
                targets
                    .values()
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| budget.fail(true))?,
            )
            .ok_or_else(|| budget.fail(true))?;
        budget.extra(second_scan)?;
        for (variant_index, variant) in variants.iter().enumerate() {
            if variant.is_uninhabited() {
                continue;
            }
            let explicit = targets
                .values()
                .iter()
                .find(|target| target.value() == variant.discriminant())
                .map(|target| target.edge().target());
            let target = explicit.or_else(|| {
                (!otherwise_is_ambiguous && otherwise_variant == Some(variant_index as u32))
                    .then(|| targets.otherwise().target())
            });
            let Some(target) = target else {
                continue;
            };
            // A shared predecessor block does not establish which edge
            // arrived, including an otherwise edge with no variant fact.
            if !enum_payload_target_is_unique_v1(targets, target, budget)? {
                continue;
            }
            if !dominators.is_reachable(target.index() as usize)
                || !dominators.has_unique_predecessor(target.index() as usize, *switch_block)
            {
                continue;
            }
            let availability = SemanticEnumPayloadAvailabilityV1(payload_targets.len());
            budget.push(&mut payload_targets, target)?;
            budget.push(
                &mut availability_by_local[local_index],
                (variant_index as u32, availability),
            )?;
        }
    }
    Ok(SemanticEnumPayloadDominanceV1 {
        availability_by_local: budget.boxed(availability_by_local)?,
        payload_targets: budget.boxed(payload_targets)?,
        dominator_preorder: budget.boxed(dominators.preorder)?,
        dominator_subtree_end: budget.boxed(dominators.subtree_end)?,
        work_units: budget.used,
    })
}
