//! Original scalar-enum slots may back a unique alias only while immutable.
//! This carries values, never capability payloads or variant authority.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticRustcVariantsV1;
#[path = "ordinary_enum_alias_v1/mixed_lineage.rs"]
mod mixed_lineage;
pub(super) use mixed_lineage::MixedAliasPlan;
#[path = "ordinary_enum_alias_v1/aggregate_custody.rs"]
mod aggregate_custody;
pub(super) use aggregate_custody::AggregateCustodyPlan;

#[derive(Clone, Copy)]
struct Alias {
    source: u32,
    block: u32,
    statement: u32,
    ty: SemanticTypeIdV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ScalarEnumAliasV1 {
    source: u32,
    capture_block: u32,
    capture_statement: u32,
    definition: SsaValueV1,
}

impl ScalarEnumAliasV1 {
    pub(super) const fn source(self) -> u32 {
        self.source
    }

    pub(super) fn allows_use(
        self,
        function: &SemanticFunctionDeclV1,
        transport: &SemanticControlFlowSsaPlanV1,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        local: u32,
        budget: &mut SemanticEnumAnalysisBudgetV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        let Some(body) = function.blocks().get(block.index() as usize) else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        let end = statement.map_or(body.statements().len(), |s| s as usize);
        if end > body.statements().len() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let start = if block.index() == self.capture_block {
            if end <= self.capture_statement as usize {
                return Ok(false);
            }
            self.capture_statement as usize + 1
        } else {
            // This is the already retained SSA value, not a reconstruction of
            // source events. A phi, new definition, or absent live value fails.
            budget.charge_work(lookup_work(transport.live_in.len()))?;
            let live_count = transport.live_in(block.index()).len();
            budget.charge_work(
                lookup_work(transport.live_in.len())
                    + live_count
                    + lookup_work(transport.block_entry_values.len())
                    + lookup_work(transport.entry_definitions.len()),
            )?;
            if transport.entry_value(function, block.index(), local) != Some(self.definition) {
                return Ok(false);
            }
            0
        };
        // A same-block lifetime marker has not yet affected the entry value.
        // Inspect only this original prefix under the existing enum owner.
        for item in &body.statements()[start..end] {
            budget.charge_work(1)?;
            if matches!(item.kind(), SemanticStatementKindV1::StorageLive(l)
                | SemanticStatementKindV1::StorageDead(l) if l.index() == local)
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
pub(super) fn plan(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    transport: &SemanticControlFlowSsaPlanV1,
    budget: &mut SemanticEnumAnalysisBudgetV1,
) -> Result<BTreeMap<u32, u32>, ProductionSemanticKirErrorV1> {
    // Existing tests compare storage owners; production retains the complete
    // definition certificate and checks it at the original use site.
    Ok(plan_aliases(types, function, transport, budget)?
        .into_iter()
        .map(|(local, proof)| (local, proof.source()))
        .collect())
}
#[derive(Default)]
struct LocalAudit {
    definitions: u32,
    unique_site: Option<(u32, u32)>,
    storage_dead: Option<(u32, u32)>,
    constructors: BTreeSet<u32>,
    only_constructors: bool,
    invalid: bool,
}

fn lookup_work(len: usize) -> usize {
    1 + (usize::BITS - len.leading_zeros()) as usize
}

fn scalar_payload(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    matches!(
        types.get(ty.index() as usize).map(|t| t.shape()),
        Some(SemanticTypeShapeV1::Scalar(
            SemanticScalarTypeV1::Bool
                | SemanticScalarTypeV1::Integer { bits: 8 | 16 | 32 | 64, .. }
        ))
    )
}

// Eligibility only, never a constructor or a replacement for erased custody.
// A residual with one empty inhabited variant cannot overlap scalar storage.
fn inactive_empty_enum(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    budget: &mut SemanticEnumAnalysisBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    let Some(declaration) = types.get(ty.index() as usize) else {
        return Ok(false);
    };
    let SemanticTypeShapeV1::Enum { variants, .. } = declaration.shape() else {
        return Ok(false);
    };
    let layout = declaration.layout();
    let SemanticRustcVariantsV1::Single { index } = layout.variants() else {
        return Ok(false);
    };
    if layout.size_bytes() != Some(0) || layout.is_uninhabited() {
        return Ok(false);
    }
    let mut found = false;
    for (actual, variant) in variants.iter().enumerate() {
        budget.charge_work(1)?;
        if !variant.is_uninhabited() {
            if actual != *index as usize || !variant.fields().fields().is_empty() {
                return Ok(false);
            }
            found = true;
        }
    }
    Ok(found)
}

fn scalar_enum(
    types: &[SemanticTypeDeclV1],
    bindings: &BTreeMap<SemanticTypeIdV1, SemanticPromotedBindingV1>,
    ty: SemanticTypeIdV1,
    budget: &mut SemanticEnumAnalysisBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    let Some(SemanticTypeShapeV1::Enum { variants, .. }) =
        types.get(ty.index() as usize).map(|t| t.shape())
    else {
        return Ok(false);
    };
    for variant in variants {
        budget.charge_work(1)?;
        for field in variant.fields().fields() {
            budget.charge_work(1 + lookup_work(bindings.len()))?;
            if bindings.contains_key(field)
                || (!scalar_payload(types, *field) && !inactive_empty_enum(types, *field, budget)?)
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn whole_operand(operand: &SemanticOperandV1) -> Option<&SemanticPlaceV1> {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
            if place.projections().is_empty() =>
        {
            Some(place)
        }
        _ => None,
    }
}

// The caller supplies its already checked function/SSA transport pair. No SSA
// events are reconstructed here; existing promotion and reaching-use checks
// remain prerequisites. The complete source-write audit protects slot lifetime.
pub(super) fn plan_aliases(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    transport: &SemanticControlFlowSsaPlanV1,
    budget: &mut SemanticEnumAnalysisBudgetV1,
) -> Result<BTreeMap<u32, ScalarEnumAliasV1>, ProductionSemanticKirErrorV1> {
    let mut candidates = BTreeMap::<u32, Alias>::new();
    let mut audited = BTreeMap::<u32, LocalAudit>::new();
    for (block, body) in function.blocks().iter().enumerate() {
        budget.charge_work(1)?;
        for (statement, item) in body.statements().iter().enumerate() {
            budget.charge_work(1)?;
            let SemanticStatementKindV1::Assign(a) = item.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Use(operand) = a.value().kind() else {
                continue;
            };
            let Some(source) = whole_operand(operand) else {
                continue;
            };
            let destination = a.destination();
            let dest = destination.local().index();
            let src = source.local().index();
            budget.charge_work(
                2 * lookup_work(transport.promoted.len())
                    + 2 * lookup_work(transport.ssa_value_locals.len())
                    + lookup_work(transport.compiler_issued_bindings.len()),
            )?;
            let Some(promoted) = transport.promoted.get(&src) else {
                continue;
            };
            if dest == src
                || !destination.projections().is_empty()
                || transport.promoted.contains_key(&dest)
                || !transport.ssa_value_locals.contains(&src)
                || !transport.ssa_value_locals.contains(&dest)
                || !promoted.transport.uses_structural_enum_transport()
                || promoted.semantic_type != source.ty()
                || promoted.transport_semantic_type != source.ty()
                || destination.ty() != source.ty()
                || a.value().result_type() != source.ty()
                || !function.locals().get(dest as usize).is_some_and(|l| {
                    l.ty() == source.ty() && l.role() == SemanticLocalRoleV1::Temporary
                })
                || !function.locals().get(src as usize).is_some_and(|l| {
                    l.ty() == source.ty() && l.role() == SemanticLocalRoleV1::Temporary
                })
                || transport
                    .compiler_issued_bindings
                    .contains_key(&source.ty())
                || !scalar_enum(
                    types,
                    &transport.compiler_issued_bindings,
                    source.ty(),
                    budget,
                )?
            {
                continue;
            }
            budget.charge_work(lookup_work(candidates.len()) + 2 * lookup_work(audited.len()))?;
            if !candidates.contains_key(&dest) {
                budget.charge_storage(16)?;
            }
            candidates.insert(
                dest,
                Alias {
                    source: src,
                    block: block as u32,
                    statement: statement as u32,
                    ty: source.ty(),
                },
            );
            for local in [src, dest] {
                if !audited.contains_key(&local) {
                    budget.charge_storage(24 + std::mem::size_of::<Option<(u32, u32)>>().div_ceil(std::mem::size_of::<usize>()))?;
                    audited.insert(
                        local,
                        LocalAudit {
                            only_constructors: true,
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }
    if candidates.is_empty() {
        return Ok(BTreeMap::new());
    }
    audit_locals(types, function, &mut audited, budget)?;
    // One bounded visited/queue pair is reused. Visit complete source edges,
    // including cleanup and imaginary edges, not projected/normal edges only.
    let blocks = function.blocks().len();
    budget.charge_work(blocks)?;
    budget.charge_storage(blocks.saturating_mul(2).saturating_add(6))?;
    let mut visited = vec![false; blocks];
    let mut queue = Vec::<u32>::with_capacity(blocks);
    let mut result = BTreeMap::new();
    for (dest, candidate) in candidates {
        budget.charge_work(2 * lookup_work(audited.len()))?;
        let source = &audited[&candidate.source];
        let destination = &audited[&dest];
        if source.invalid
            || destination.invalid
            || source.storage_dead.is_some_and(|(block, statement)|
                block != candidate.block || statement <= candidate.statement)
            || !source.only_constructors
            || source.definitions == 0
            || destination.definitions != 1
            || destination.unique_site != Some((candidate.block, candidate.statement))
            || source.constructors.contains(&candidate.block)
            || function.locals()[dest as usize].ty() != candidate.ty
        {
            continue;
        }
        budget.charge_work(lookup_work(transport.definition_values.len()))?;
        let Some([definition @ SsaValueV1::Definition(_)]) = transport
            .definition_values.get(&(candidate.block, dest)).map(Vec::as_slice)
        else { continue; };
        budget.charge_work(blocks)?;
        visited.fill(false);
        queue.clear();
        visited[candidate.block as usize] = true;
        queue.push(candidate.block);
        let mut cursor = 0;
        let mut overwritten = false;
        while cursor < queue.len() {
            let block = queue[cursor];
            cursor += 1;
            budget.charge_work(1 + lookup_work(source.constructors.len()))?;
            if source.constructors.contains(&block) {
                overwritten = true;
                break;
            }
            function.blocks()[block as usize]
                .terminator()
                .kind()
                .try_for_each_edge(|edge| {
                    budget.charge_work(1)?;
                    let next = edge.target().index() as usize;
                    let Some(seen) = visited.get_mut(next) else {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    };
                    if !*seen {
                        *seen = true;
                        queue.push(next as u32);
                    }
                    Ok(())
                })?;
        }
        if !overwritten {
            budget.charge_work(lookup_work(result.len()))?;
            budget.charge_storage(8 + std::mem::size_of::<ScalarEnumAliasV1>().div_ceil(std::mem::size_of::<usize>()))?;
            result.insert(dest, ScalarEnumAliasV1 {
                source: candidate.source,
                capture_block: candidate.block,
                capture_statement: candidate.statement,
                definition: *definition,
            });
        }
    }
    // Conservative accounting retains all charged temporary storage. The
    // shared enum owner never resets or refunds work when precision misses.
    Ok(result)
}

fn invalidate(
    local: u32,
    rows: &mut BTreeMap<u32, LocalAudit>,
    budget: &mut SemanticEnumAnalysisBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(lookup_work(rows.len()))?;
    if let Some(row) = rows.get_mut(&local) {
        row.invalid = true;
    }
    Ok(())
}

fn escaped_operand(
    operand: &SemanticOperandV1,
    rows: &mut BTreeMap<u32, LocalAudit>,
    budget: &mut SemanticEnumAnalysisBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            invalidate(place.local().index(), rows, budget)
        }
        SemanticOperandV1::Constant(_) => Ok(()),
    }
}

fn exact_constructor(
    types: &[SemanticTypeDeclV1],
    a: &SemanticAssignmentV1,
    budget: &mut SemanticEnumAnalysisBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    if !a.destination().projections().is_empty() || a.destination().ty() != a.value().result_type()
    {
        return Ok(false);
    }
    let SemanticRvalueKindV1::Aggregate(aggregate) = a.value().kind() else {
        return Ok(false);
    };
    let SemanticAggregateKindV1::EnumVariant(variant) = aggregate.kind() else {
        return Ok(false);
    };
    let Some(SemanticTypeShapeV1::Enum { variants, .. }) = types
        .get(a.value().result_type().index() as usize)
        .map(|t| t.shape())
    else {
        return Ok(false);
    };
    let Some(variant) = variants.get(*variant as usize) else {
        return Ok(false);
    };
    if variant.is_uninhabited() || variant.fields().fields().len() != aggregate.operands().len() {
        return Ok(false);
    }
    for (field, operand) in variant.fields().fields().iter().zip(aggregate.operands()) {
        budget.charge_work(1)?;
        if operand.ty() != *field {
            return Ok(false);
        }
    }
    Ok(true)
}

impl<'a> SemanticFunctionLoweringV1<'a> {
    pub(super) fn untransported_scalar_enum_storage_owner(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        local: u32,
    ) -> Result<Option<u32>, ProductionSemanticKirErrorV1> {
        let Some(SemanticValueBindingV1::Enum {
            semantic_type,
            payloads,
            ..
        }) = self.locals.get(local as usize).and_then(Option::as_ref)
        else {
            return Ok(None);
        };
        let Some(declaration) = self.function.locals().get(local as usize) else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        if *semantic_type != declaration.ty() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        if !payloads.is_empty() {
            return Ok(None);
        }
        let Some(SemanticTypeShapeV1::Enum { variants, .. }) = self
            .types
            .get(semantic_type.index() as usize)
            .map(|t| t.shape())
        else {
            return Ok(None);
        };
        // Existing exact branch authority is required before optional work.
        self.enum_analysis_budget.charge_work(variants.len())?;
        let needed = variants.iter().enumerate().find_map(|(variant, fields)| {
            (!fields.fields().fields().is_empty()
                && self.enum_variant_is_available_v1(
                    SemanticLocalIdV1::from_index(local),
                    variant as u32,
                    block,
                )
                && payloads
                    .get(&(variant as u32))
                    .is_none_or(|p| p.len() != fields.fields().fields().len())).then_some(variant as u32)
        });
        let Some(variant) = needed else { return Ok(None); };
        // The inactive residual may qualify the enum, but is never restored by
        // this alias certificate. Recheck the selected fields even on cache hits.
        for field in variants[variant as usize].fields().fields() {
            self.enum_analysis_budget.charge_work(
                1 + lookup_work(self.control_flow_ssa.compiler_issued_bindings.len()),
            )?;
            if !scalar_payload(self.types, *field)
                || self.control_flow_ssa.compiler_issued_bindings.contains_key(field)
            {
                return Ok(None);
            }
        }
        if self.enum_payload_aliases.is_none() {
            if !scalar_enum(
                self.types,
                &self.control_flow_ssa.compiler_issued_bindings,
                declaration.ty(),
                &mut self.enum_analysis_budget,
            )? {
                return Ok(None);
            }
            self.enum_payload_aliases = Some(plan_aliases(
                self.types,
                self.function,
                &self.control_flow_ssa,
                &mut self.enum_analysis_budget,
            )?);
        }
        self.enum_analysis_budget.charge_work(lookup_work(
            self.enum_payload_aliases.as_ref().unwrap().len(),
        ))?;
        if let Some(proof) = self.enum_payload_aliases.as_ref().unwrap().get(&local).copied() {
            return Ok(proof.allows_use(self.function, &self.control_flow_ssa, block, statement, local,
                &mut self.enum_analysis_budget)?.then_some(proof.source()));
        }
        if self.enum_mixed_aliases.is_none() {
            let Some(query) = self.enum_alias_source.take() else { return Ok(None); };
            self.enum_mixed_aliases = Some(MixedAliasPlan::new(query, self.types,
                &self.control_flow_ssa, &mut self.enum_analysis_budget)?);
        }
        self.enum_mixed_aliases.as_mut().unwrap().storage_owner(self.types,
            &self.control_flow_ssa, block, statement, local, variant, &mut self.enum_analysis_budget)
    }
}

fn audit_locals(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    audited: &mut BTreeMap<u32, LocalAudit>,
    budget: &mut SemanticEnumAnalysisBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    for (block, body) in function.blocks().iter().enumerate() {
        budget.charge_work(1)?;
        for (statement, item) in body.statements().iter().enumerate() {
            budget.charge_work(1)?;
            match item.kind() {
                SemanticStatementKindV1::Assign(a) => {
                    budget.charge_work(lookup_work(audited.len()))?;
                    if let Some(audit) = audited.get_mut(&a.destination().local().index()) {
                        audit.definitions = audit.definitions.saturating_add(1);
                        audit.unique_site = Some((block as u32, statement as u32));
                        if !a.destination().projections().is_empty() {
                            audit.invalid = true;
                        }
                        let constructor = exact_constructor(types, a, budget)?;
                        audit.only_constructors &= constructor;
                        if constructor {
                            budget.charge_work(lookup_work(audit.constructors.len()))?;
                            if !audit.constructors.contains(&(block as u32)) {
                                budget.charge_storage(8)?;
                                audit.constructors.insert(block as u32);
                            }
                        }
                    }
                    match a.value().kind() {
                        SemanticRvalueKindV1::Borrow { place, .. }
                        | SemanticRvalueKindV1::AddressOf { place, .. } => {
                            invalidate(place.local().index(), audited, budget)?
                        }
                        SemanticRvalueKindV1::Load(load) => {
                            invalidate(load.source().local().index(), audited, budget)?
                        }
                        _ => {}
                    }
                }
                SemanticStatementKindV1::Store(store) => {
                    invalidate(store.destination().local().index(), audited, budget)?
                }
                SemanticStatementKindV1::AtomicRmw(op) => {
                    invalidate(op.address().local().index(), audited, budget)?;
                    invalidate(op.destination().local().index(), audited, budget)?;
                }
                SemanticStatementKindV1::AtomicCompareExchange(op) => {
                    invalidate(op.address().local().index(), audited, budget)?;
                    invalidate(op.destination().local().index(), audited, budget)?;
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place) => {
                    invalidate(place.local().index(), audited, budget)?
                }
                SemanticStatementKindV1::StorageDead(local) => {
                    budget.charge_work(lookup_work(audited.len()))?;
                    if let Some(audit) = audited.get_mut(&local.index()) {
                        // A logical lifetime end is not a write to the private
                        // payload alloca. Its position still bounds capture/use.
                        if audit.storage_dead.replace((block as u32, statement as u32)).is_some() {
                            audit.invalid = true;
                        }
                    }
                }
                // StorageLive does not establish initialization. Its existing
                // promoted SSA kill/use validation is still required.
                SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::Assume(_)
                | SemanticStatementKindV1::Nop => {}
            }
        }
        budget.charge_work(1)?;
        match body.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => {
                if let Some(dest) = call.destination() {
                    invalidate(dest.place().local().index(), audited, budget)?;
                }
                for operand in call.arguments() {
                    escaped_operand(operand, audited, budget)?;
                }
            }
            SemanticTerminatorKindV1::TailCall(call) => {
                for operand in call.arguments() {
                    escaped_operand(operand, audited, budget)?;
                }
            }
            SemanticTerminatorKindV1::Drop { place, .. } => {
                invalidate(place.local().index(), audited, budget)?
            }
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::SwitchInt { .. }
            | SemanticTerminatorKindV1::Assert { .. }
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => {}
        }
    }
    Ok(())
}
