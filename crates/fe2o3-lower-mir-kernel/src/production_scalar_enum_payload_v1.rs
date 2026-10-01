// Scalar payloads may bypass synthetic storage only when every constructor
// supplies the same immutable entry SSA value. This is not capability custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScalarEnumProducerV1 {
    semantic_type: SemanticTypeIdV1,
    ssa: SsaValueV1,
    value: ValueId,
    scalar: ScalarType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScalarEnumCandidateV1 {
    Unseen,
    Fixed(ScalarEnumProducerV1),
    Rejected,
}

#[derive(Clone, Copy, Debug)]
struct ScalarEnumPayloadV1 {
    key: (u32, u32, u32),
    semantic_type: SemanticTypeIdV1,
    source: ScalarEnumCandidateV1,
}

#[derive(Clone, Copy, Debug, Default)]
struct ScalarEnumLocalCensusV1 {
    definitions: usize,
    constructors: usize,
    ssa_definitions: usize,
    escaped: bool,
}

#[derive(Clone, Copy, Default)]
struct ScalarEnumAliasV1 {
    source: Option<(u32, bool, u32)>,
    definition: Option<(u32, u32, SsaValueV1)>,
    input: Option<SsaValueV1>,
    valid: bool,
}

fn scalar_enum_alias_input_v1(
    events: &[(u32, SsaResolvedEventV1)],
    definition: usize,
    source: u32,
    moved: bool,
) -> Option<SsaValueV1> {
    // The canonical adapter emits whole-local Use, optional MoveKill, Define.
    // A unique source assignment binds this exact sequence to its destination.
    let (ordinal, _) = events.get(definition)?;
    let distance = if moved { 2 } else { 1 };
    let (input_ordinal, input) = events.get(definition.checked_sub(distance)?)?;
    let SsaResolvedEventV1::Use { variable, value } = input else {
        return None;
    };
    if variable.get() != source || input_ordinal.checked_add(distance as u32)? != *ordinal {
        return None;
    }
    if moved
        && !matches!(events.get(definition - 1),
            Some((kill_ordinal, SsaResolvedEventV1::Kill { variable, previous: Some(previous) }))
            if kill_ordinal.checked_add(1) == Some(*ordinal)
                && variable.get() == source && previous == value)
    {
        return None;
    }
    Some(*value)
}

#[allow(clippy::too_many_arguments)]
fn extend_scalar_enum_aliases_v1(
    parameters: &mut [Option<ScalarEnumProducerV1>],
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    semantic_ssa: &ProductionSemanticSsaFunctionPlanV1,
    ssa: &SemanticControlFlowSsaPlanV1,
    census: &[ScalarEnumLocalCensusV1],
    dominance: &SemanticEnumPayloadDominanceV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut aliases = emission_vec_v1(parameters.len(), budget)?;
    budget.charge_work(parameters.len())?;
    aliases.extend(parameters.iter().map(|parameter| ScalarEnumAliasV1 {
        valid: parameter.is_some(),
        ..ScalarEnumAliasV1::default()
    }));
    for (block, body) in function.blocks().iter().enumerate() {
        budget.charge_work(1)?;
        for statement in body.statements() {
            budget.charge_work(1)?;
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Use(operand) = assignment.value().kind() else {
                continue;
            };
            let (source, moved) = match operand {
                SemanticOperandV1::Copy(place) => (place, false),
                SemanticOperandV1::Move(place) => (place, true),
                _ => continue,
            };
            let destination = assignment.destination();
            let local = destination.local().index() as usize;
            let facts = &census[local];
            let ty = function.locals()[local].ty();
            if !destination.projections().is_empty()
                || !source.projections().is_empty()
                || source.ty() != ty
                || destination.ty() != ty
                || function.locals()[source.local().index() as usize].ty() != ty
                || function.locals()[local].role().is_entry_argument()
                || facts.escaped
                || facts.definitions != 1
                || facts.ssa_definitions != 1
                || !matches!(
                    types[ty.index() as usize].shape(),
                    SemanticTypeShapeV1::Scalar(_)
                )
            {
                continue;
            }
            charge_execution_cfg_lookup_v29(ssa.ssa_value_locals.len(), budget)?;
            charge_execution_cfg_lookup_v29(ssa.compiler_issued_bindings.len(), budget)?;
            if !ssa.ssa_value_locals.contains(&(local as u32))
                || ssa.compiler_issued_bindings.contains_key(&ty)
            {
                continue;
            }
            aliases[local].source = Some((source.local().index(), moved, block as u32));
            aliases[local].valid = true;
        }
    }
    let plan = semantic_ssa.plan();
    for &block in plan.reverse_postorder() {
        budget.charge_work(1)?;
        let events = plan
            .resolved_events(block)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        for (index, &(ordinal, event)) in events.iter().enumerate() {
            budget.charge_work(1)?;
            let SsaResolvedEventV1::Define { variable, value } = event else {
                continue;
            };
            let row = &mut aliases[variable.get() as usize];
            let Some((source, moved, expected_block)) = row.source else {
                continue;
            };
            if row.definition.is_some()
                || expected_block != block.get()
                || !matches!(value, SsaValueV1::Definition(_))
            {
                row.valid = false;
                continue;
            }
            budget.charge_work(3)?;
            row.definition = Some((block.get(), ordinal, value));
            row.input = scalar_enum_alias_input_v1(events, index, source, moved);
            row.valid &= row.input.is_some();
        }
    }
    // Verify every actual use before propagating aliases. Local indices are
    // lookup keys only; unknown/merged SSA values never stand in for a producer.
    for &block in plan.reverse_postorder() {
        budget.charge_work(1)?;
        let events = plan
            .resolved_events(block)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        for &(ordinal, event) in events {
            budget.charge_work(1)?;
            let SsaResolvedEventV1::Use { variable, value } = event else {
                continue;
            };
            let local = variable.get() as usize;
            let row = &mut aliases[local];
            let expected = parameters[local]
                .map(|parameter| parameter.ssa)
                .or_else(|| row.definition.map(|(_, _, value)| value));
            row.valid &= expected == Some(value) && matches!(value, SsaValueV1::Definition(_));
            if let Some((producer_block, producer_ordinal, _)) = row.definition {
                budget.charge_work(1)?;
                row.valid &= if producer_block == block.get() {
                    producer_ordinal < ordinal
                } else {
                    dominance.block_dominates(
                        SemanticBlockIdV1::from_index(producer_block),
                        SemanticBlockIdV1::from_index(block.get()),
                    )
                };
            }
        }
    }
    // The caller already excludes cyclic CFGs. Exact dominating definitions
    // precede their uses in this canonical order, so no recursive alias walk or
    // guessed local version is needed; each event is visited once per pass.
    for &block in plan.reverse_postorder() {
        budget.charge_work(1)?;
        let events = plan
            .resolved_events(block)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        for &(_, event) in events {
            budget.charge_work(1)?;
            let SsaResolvedEventV1::Define { variable, .. } = event else {
                continue;
            };
            let local = variable.get() as usize;
            let row = aliases[local];
            let Some((source, _, _)) = row.source else {
                continue;
            };
            let source = source as usize;
            let expected = aliases[source]
                .definition
                .map(|(_, _, value)| value)
                .or_else(|| parameters[source].map(|parameter| parameter.ssa));
            if row.valid && aliases[source].valid && row.input == expected {
                parameters[local] = parameters[source]
                    .filter(|parameter| parameter.semantic_type == function.locals()[local].ty());
            }
        }
    }
    Ok(())
}

struct ScalarEnumCensusWorkV1<'a>(&'a mut dyn SemanticEmissionBudgetV1);
impl PrivateArrayChargeV1 for ScalarEnumCensusWorkV1<'_> {
    type Error = ProductionSemanticKirErrorV1;
    fn charge_private_array_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.0.charge_work(amount)
    }
}

fn scalar_enum_payload_error_v1() -> ProductionSemanticKirErrorV1 {
    unsupported(0, None, None, "proved scalar enum payload source changed")
}

fn scalar_enum_record_definition_v1(
    census: &mut [ScalarEnumLocalCensusV1],
    place: &SemanticPlaceV1,
    constructor: bool,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let row = census
        .get_mut(place.local().index() as usize)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    row.definitions = row
        .definitions
        .checked_add(1)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    if constructor && place.projections().is_empty() {
        row.constructors = row
            .constructors
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
    }
    Ok(())
}

fn scalar_enum_payload_lookup_v1(
    rows: &[ScalarEnumPayloadV1],
    key: (u32, u32, u32),
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
    charge_execution_cfg_lookup_v29(rows.len(), budget)?;
    Ok(rows.binary_search_by_key(&key, |row| row.key).ok())
}

fn scalar_enum_plan_is_acyclic_v1(
    function: &SemanticFunctionDeclV1,
    semantic_ssa: &ProductionSemanticSsaFunctionPlanV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let mut positions = emission_vec_v1(function.blocks().len(), budget)?;
    budget.charge_work(function.blocks().len())?;
    positions.resize(function.blocks().len(), usize::MAX);
    for (position, block) in semantic_ssa.plan().reverse_postorder().iter().enumerate() {
        budget.charge_work(1)?;
        *positions
            .get_mut(block.get() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)? = position;
    }
    let mut acyclic = true;
    for (index, block) in function.blocks().iter().enumerate() {
        budget.charge_work(1)?;
        if positions[index] == usize::MAX {
            continue;
        }
        block
            .terminator()
            .kind()
            .try_for_each_edge::<ProductionSemanticKirErrorV1>(|edge| {
                budget.charge_work(1)?;
                let target = *positions
                    .get(edge.target().index() as usize)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                if target <= positions[index] {
                    acyclic = false;
                }
                Ok(())
            })?;
    }
    Ok(acyclic)
}

fn plan_scalar_enum_payloads_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    semantic_ssa: &ProductionSemanticSsaFunctionPlanV1,
    ssa: &SemanticControlFlowSsaPlanV1,
    entry_bindings: &[Option<SemanticValueBindingV1>],
    restoration: ScalarEnumRestorationFactsV1<'_>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<ScalarEnumPayloadV1>, ProductionSemanticKirErrorV1> {
    // Reuse the production caller's ledger. None-ledger test adapters never
    // enter here, and no additional resource allowance is constructed.
    budget.charge_work(ssa.promoted.len())?;
    if !ssa.promoted.values().any(|local| {
        local.transport.uses_structural_enum_transport()
            && matches!(
                types
                    .get(local.semantic_type.index() as usize)
                    .map(SemanticTypeDeclV1::shape),
                Some(SemanticTypeShapeV1::Enum { .. })
            )
    }) {
        return Ok(Vec::new());
    }
    if !scalar_enum_plan_is_acyclic_v1(function, semantic_ssa, budget)? {
        return Ok(Vec::new());
    }
    let count = function.locals().len();
    let mut census = emission_vec_v1(count, budget)?;
    budget.charge_work(count)?;
    census.resize(count, ScalarEnumLocalCensusV1::default());
    visit_private_slot_roots_v1(function, &mut ScalarEnumCensusWorkV1(budget), |local, _| {
        census
            .get_mut(local as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            .escaped = true;
        Ok(())
    })?;
    for block in function.blocks() {
        budget.charge_work(1)?;
        for statement in block.statements() {
            budget.charge_work(1)?;
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    let constructor = matches!(assignment.value().kind(),
                        SemanticRvalueKindV1::Aggregate(aggregate)
                        if matches!(aggregate.kind(), SemanticAggregateKindV1::EnumVariant(_)));
                    scalar_enum_record_definition_v1(
                        &mut census,
                        assignment.destination(),
                        constructor,
                    )?;
                }
                SemanticStatementKindV1::Store(store) => {
                    scalar_enum_record_definition_v1(&mut census, store.destination(), false)?
                }
                SemanticStatementKindV1::AtomicRmw(operation) => {
                    scalar_enum_record_definition_v1(&mut census, operation.destination(), false)?
                }
                SemanticStatementKindV1::AtomicCompareExchange(operation) => {
                    scalar_enum_record_definition_v1(&mut census, operation.destination(), false)?
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place) => {
                    scalar_enum_record_definition_v1(&mut census, place, false)?
                }
                SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::Assume(_)
                | SemanticStatementKindV1::Nop => {}
            }
        }
        budget.charge_work(1)?;
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && let Some(destination) = call.destination()
        {
            scalar_enum_record_definition_v1(&mut census, destination.place(), false)?;
        }
    }
    for ((_, local), values) in &ssa.definition_values {
        budget.charge_work(1)?;
        let row = census
            .get_mut(*local as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        row.ssa_definitions = row
            .ssa_definitions
            .checked_add(values.len())
            .ok_or(ArgumentResourceV1::Arithmetic)?;
    }
    let mut parameters = emission_vec_v1(count, budget)?;
    budget.charge_work(count)?;
    parameters.resize(count, None::<ScalarEnumProducerV1>);
    for (local, declaration) in function.locals().iter().enumerate() {
        budget.charge_work(1)?;
        let facts = census[local];
        if !declaration.role().is_entry_argument()
            || facts.escaped
            || facts.definitions != 0
            || facts.ssa_definitions != 0
        {
            continue;
        }
        charge_execution_cfg_lookup_v29(ssa.ssa_value_locals.len(), budget)?;
        charge_execution_cfg_lookup_v29(ssa.entry_definitions.len(), budget)?;
        charge_execution_cfg_lookup_v29(ssa.compiler_issued_bindings.len(), budget)?;
        if !ssa.ssa_value_locals.contains(&(local as u32))
            || ssa.compiler_issued_bindings.contains_key(&declaration.ty())
        {
            continue;
        }
        let Some(&source) = ssa.entry_definitions.get(&(local as u32)) else {
            continue;
        };
        if !matches!(
            types
                .get(declaration.ty().index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(_))
        ) {
            continue;
        }
        let Some(Some(SemanticValueBindingV1::Value {
            id,
            ty: Type::Scalar(scalar),
        })) = entry_bindings.get(local)
        else {
            continue;
        };
        budget.charge_work(1)?;
        if lower_scalar_type(types, declaration.ty()).ok() != Some(Type::Scalar(*scalar)) {
            continue;
        }
        parameters[local] = Some(ScalarEnumProducerV1 {
            semantic_type: declaration.ty(),
            ssa: source,
            value: *id,
            scalar: *scalar,
        });
    }
    extend_scalar_enum_aliases_v1(
        &mut parameters,
        types,
        function,
        semantic_ssa,
        ssa,
        &census,
        restoration.dominance,
        budget,
    )?;
    let mut rows = Vec::new();
    for (&local, promoted) in &ssa.promoted {
        budget.charge_work(1)?;
        let facts = census
            .get(local as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if !promoted.transport.uses_structural_enum_transport()
            || facts.escaped
            || facts.definitions == 0
            || facts.definitions != facts.constructors
            || facts.definitions != facts.ssa_definitions
            || function.locals()[local as usize].role().is_entry_argument()
        {
            continue;
        }
        let Some(SemanticTypeShapeV1::Enum { variants, .. }) = types
            .get(promoted.semantic_type.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            continue;
        };
        for (variant, definition) in variants.iter().enumerate() {
            budget.charge_work(1)?;
            for (field, &ty) in definition.fields().fields().iter().enumerate() {
                budget.charge_work(1)?;
                charge_execution_cfg_lookup_v29(ssa.compiler_issued_bindings.len(), budget)?;
                if ssa.compiler_issued_bindings.contains_key(&ty)
                    || !matches!(
                        types
                            .get(ty.index() as usize)
                            .map(SemanticTypeDeclV1::shape),
                        Some(SemanticTypeShapeV1::Scalar(_))
                    )
                {
                    continue;
                }
                if rows.len() == MAX_ENUM_PAYLOAD_STORAGE_COMPONENTS_V1 {
                    return Err(unsupported(
                        0,
                        None,
                        None,
                        "scalar enum payload plan exceeds the component limit",
                    ));
                }
                emission_push_v1(
                    &mut rows,
                    ScalarEnumPayloadV1 {
                        key: (local, variant as u32, field as u32),
                        semantic_type: ty,
                        source: ScalarEnumCandidateV1::Unseen,
                    },
                    budget,
                )?;
            }
        }
    }
    for block in function.blocks() {
        budget.charge_work(1)?;
        for statement in block.statements() {
            budget.charge_work(1)?;
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
                continue;
            };
            let SemanticAggregateKindV1::EnumVariant(variant) = aggregate.kind() else {
                continue;
            };
            if !assignment.destination().projections().is_empty() {
                continue;
            }
            let local = assignment.destination().local().index();
            for (field, operand) in aggregate.operands().iter().enumerate() {
                budget.charge_work(1)?;
                let Some(index) =
                    scalar_enum_payload_lookup_v1(&rows, (local, *variant, field as u32), budget)?
                else {
                    continue;
                };
                let source = match operand {
                    SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
                        if place.projections().is_empty() =>
                    {
                        parameters
                            .get(place.local().index() as usize)
                            .copied()
                            .flatten()
                            .filter(|source| {
                                source.semantic_type == operand.ty()
                                    && source.semantic_type == rows[index].semantic_type
                            })
                    }
                    _ => None,
                };
                let row = &mut rows[index];
                row.source = match (row.source, source) {
                    (ScalarEnumCandidateV1::Unseen, Some(source)) => {
                        ScalarEnumCandidateV1::Fixed(source)
                    }
                    (ScalarEnumCandidateV1::Fixed(previous), Some(source))
                        if previous == source =>
                    {
                        row.source
                    }
                    _ => ScalarEnumCandidateV1::Rejected,
                };
            }
        }
    }
    retain_scalar_enum_restorations_v1(&mut rows, types, function, ssa, restoration, budget)?;
    budget.charge_work(rows.len())?;
    rows.retain(|row| matches!(row.source, ScalarEnumCandidateV1::Fixed(_)));
    Ok(rows)
}

impl SemanticFunctionLoweringV1<'_> {
    fn scalar_enum_payload_v1(
        &mut self,
        key: (u32, u32, u32),
        block: Option<SemanticBlockIdV1>,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        if self.scalar_enum_payloads.is_empty() {
            return Ok(None);
        }
        self.with_emission_budget_v1(|this, budget| {
            let Some(index) =
                scalar_enum_payload_lookup_v1(&this.scalar_enum_payloads, key, budget)?
            else {
                return Ok(None);
            };
            let ScalarEnumCandidateV1::Fixed(source) = this.scalar_enum_payloads[index].source
            else {
                return Err(scalar_enum_payload_error_v1());
            };
            if let Some(block) = block {
                let value = scalar_enum_entry_value_v1(
                    &this.control_flow_ssa,
                    this.function,
                    block.index(),
                    key.0,
                    budget,
                )?
                .ok_or_else(scalar_enum_payload_error_v1)?;
                charge_execution_cfg_lookup_v29(this.promoted_enum_variant_by_value.len(), budget)?;
                if this
                    .promoted_enum_variant_by_value
                    .get(&(block.index(), value))
                    != Some(&key.1)
                {
                    return Err(scalar_enum_payload_error_v1());
                }
            }
            charge_execution_cfg_lookup_v29(this.semantic_ssa_bindings.len(), budget)?;
            match this.semantic_ssa_bindings.get(&source.ssa) {
                Some(SemanticValueBindingV1::Value {
                    id,
                    ty: Type::Scalar(scalar),
                }) if *id == source.value && *scalar == source.scalar => {
                    Ok(Some(SemanticValueBindingV1::Value {
                        id: source.value,
                        ty: Type::Scalar(source.scalar),
                    }))
                }
                _ => Err(scalar_enum_payload_error_v1()),
            }
        })
    }
}

#[derive(Clone, Copy)]
struct ScalarEnumRestorationFactsV1<'a> {
    dominance: &'a SemanticEnumPayloadDominanceV1,
    variants: &'a BTreeMap<(u32, SsaValueV1), u32>,
}

fn scalar_enum_values_match_v1(
    expected: &SemanticValueBindingV1,
    actual: &SemanticValueBindingV1,
) -> bool {
    matches!((expected, actual), (
        SemanticValueBindingV1::Value { id: left, ty: Type::Scalar(left_ty) },
        SemanticValueBindingV1::Value { id: right, ty: Type::Scalar(right_ty) },
    ) if left == right && left_ty == right_ty)
}

fn scalar_enum_entry_value_v1(
    ssa: &SemanticControlFlowSsaPlanV1,
    function: &SemanticFunctionDeclV1,
    block: u32,
    local: u32,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<SsaValueV1>, ProductionSemanticKirErrorV1> {
    charge_execution_cfg_lookup_v29(ssa.block_entry_values.len(), budget)?;
    charge_execution_cfg_lookup_v29(ssa.live_in.len(), budget)?;
    budget.charge_work(ssa.live_in(block).len())?;
    charge_execution_cfg_lookup_v29(ssa.entry_definitions.len(), budget)?;
    Ok(ssa.entry_value(function, block, local))
}

fn scalar_enum_check_restoration_v1(
    rows: &mut [ScalarEnumPayloadV1],
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    ssa: &SemanticControlFlowSsaPlanV1,
    facts: ScalarEnumRestorationFactsV1<'_>,
    site: (u32, u32),
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let (block, local) = site;
    if block == function.entry().index() {
        return Ok(());
    }
    charge_execution_cfg_lookup_v29(rows.len(), budget)?;
    charge_execution_cfg_lookup_v29(rows.len(), budget)?;
    let first = rows.partition_point(|row| row.key.0 < local);
    let end = rows.partition_point(|row| row.key.0 <= local);
    if first == end {
        return Ok(());
    }
    let current = scalar_enum_entry_value_v1(ssa, function, block, local, budget)?;
    charge_execution_cfg_lookup_v29(facts.variants.len(), budget)?;
    let known = current
        .and_then(|value| facts.variants.get(&(block, value)))
        .copied();
    let declaration = function
        .locals()
        .get(local as usize)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let Some(SemanticTypeShapeV1::Enum { variants, .. }) = types
        .get(declaration.ty().index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    };
    for row in &mut rows[first..end] {
        budget.charge_work(1)?;
        if known == Some(row.key.1) {
            continue;
        }
        charge_execution_cfg_lookup_v29(variants.len(), budget)?;
        budget.charge_work(1)?;
        if facts
            .dominance
            .availability(SemanticLocalIdV1::from_index(local), row.key.1)
            .is_some_and(|guard| {
                facts
                    .dominance
                    .allows(guard, SemanticBlockIdV1::from_index(block))
            })
        {
            // Existing restoration can use a legacy guard alone. Keep its
            // physical slot rather than tighten admission after omission.
            row.source = ScalarEnumCandidateV1::Rejected;
        }
    }
    Ok(())
}

fn retain_scalar_enum_restorations_v1(
    rows: &mut [ScalarEnumPayloadV1],
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    ssa: &SemanticControlFlowSsaPlanV1,
    facts: ScalarEnumRestorationFactsV1<'_>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    // This is the same entry-local roster used by begin_block; duplicate
    // membership is harmless and charged, without a CFG scan per field.
    for (&block, locals) in &ssa.live_in {
        budget.charge_work(1)?;
        for &local in locals {
            budget.charge_work(1)?;
            scalar_enum_check_restoration_v1(
                rows,
                types,
                function,
                ssa,
                facts,
                (block, local),
                budget,
            )?;
        }
    }
    for &(block, local) in ssa.block_entry_values.keys() {
        budget.charge_work(1)?;
        scalar_enum_check_restoration_v1(
            rows,
            types,
            function,
            ssa,
            facts,
            (block, local),
            budget,
        )?;
    }
    Ok(())
}

#[allow(clippy::type_complexity)]
fn plan_enum_payload_storage_v1<'work>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    control_flow_ssa: &SemanticControlFlowSsaPlanV1,
    sources: &BTreeMap<(u32, u32, u32), SemanticEnumPayloadSourceV1>,
    next_value: &mut u32,
    scalar_payloads: &[ScalarEnumPayloadV1],
    mut budget: Option<&mut (dyn SemanticEmissionBudgetV1 + 'work)>,
) -> Result<
    (
        BTreeMap<(u32, u32, u32), SemanticEnumPayloadFieldStorageV1>,
        BTreeSet<(u32, u32, u32)>,
    ),
    ProductionSemanticKirErrorV1,
> {
    let mut storage = BTreeMap::new();
    let mut requires_compile_time_custody = BTreeSet::new();
    let mut component_count = 0_usize;
    for (local, promoted) in &control_flow_ssa.promoted {
        if !promoted.transport.uses_structural_enum_transport() {
            continue;
        }
        let declaration = types
            .get(promoted.semantic_type.index() as usize)
            .ok_or_else(|| unsupported(0, None, None, "promoted enum type is missing"))?;
        let SemanticTypeShapeV1::Enum { variants, .. } = declaration.shape() else {
            continue;
        };
        if function.locals().get(*local as usize).is_none() {
            return Err(unsupported(0, None, None, "promoted enum local is missing"));
        }
        for (variant, definition) in variants.iter().enumerate() {
            for (field, semantic_type) in definition.fields().fields().iter().copied().enumerate() {
                let mut components = Vec::new();
                let key = (*local, variant as u32, field as u32);
                if !scalar_payloads.is_empty()
                    && scalar_enum_payload_lookup_v1(
                        scalar_payloads,
                        key,
                        budget
                            .as_deref_mut()
                            .ok_or(ArgumentResourceV1::Accounting)?,
                    )?
                    .is_some()
                {
                    continue;
                }
                let exact_enum_variant = sources.get(&key).and_then(|source| {
                    exact_enum_variant_for_source_v1(function, source, semantic_type)
                });
                let compiler_issued_binding = exact_enum_variant
                    .is_none()
                    .then(|| {
                        control_flow_ssa
                            .compiler_issued_bindings
                            .get(&semantic_type)
                            .copied()
                    })
                    .flatten();
                let component_types = match exact_enum_variant {
                    Some(exact_variant) => {
                        lower_exact_enum_components_v1(types, semantic_type, exact_variant)
                    }
                    None => match compiler_issued_binding {
                        Some(
                            binding @ SemanticPromotedBindingV1::WorkgroupCollectiveScratch {
                                ..
                            },
                        ) => {
                            let semantic_components =
                                lower_ssa_value_components_v1(types, semantic_type)?;
                            let transport = binding.transport_types(types, semantic_type)?;
                            if semantic_components.len() != transport.len() {
                                return Err(unsupported(
                                    0,
                                    None,
                                    None,
                                    "compiler-issued enum payload transport arity changed",
                                ));
                            }
                            Ok(semantic_components
                                .into_iter()
                                .zip(transport)
                                .map(|((semantic_type, _), transport)| (semantic_type, transport))
                                .collect())
                        }
                        Some(
                            SemanticPromotedBindingV1::Ordinary
                            | SemanticPromotedBindingV1::MatrixFragment { .. }
                            | SemanticPromotedBindingV1::AccumulatorFragment { .. }
                            | SemanticPromotedBindingV1::Gfx950LdsTransposeTile { .. },
                        )
                        | None => lower_ssa_value_components_v1(types, semantic_type),
                        Some(
                            SemanticPromotedBindingV1::DynamicLds { .. }
                            | SemanticPromotedBindingV1::WorkgroupPipeline { .. },
                        ) => Err(unsupported(
                            0,
                            None,
                            None,
                            "linear workgroup storage cannot be stored in a promoted enum payload",
                        )),
                        Some(
                            SemanticPromotedBindingV1::MathContext
                            | SemanticPromotedBindingV1::CollectiveContext
                            | SemanticPromotedBindingV1::WorkgroupLdsScope
                            | SemanticPromotedBindingV1::MatrixContext
                            | SemanticPromotedBindingV1::WaveLane { .. }
                            | SemanticPromotedBindingV1::IndexWitness { .. }
                            | SemanticPromotedBindingV1::OptionIndexWitness { .. }
                            | SemanticPromotedBindingV1::GridLeader { .. }
                            | SemanticPromotedBindingV1::OptionGridLeader { .. }
                            | SemanticPromotedBindingV1::ComponentWitness { .. }
                            | SemanticPromotedBindingV1::OptionComponentWitness { .. }
                            | SemanticPromotedBindingV1::OptionPointer { .. },
                        ) => Err(unsupported(
                            0,
                            None,
                            None,
                            "compiler-issued authority cannot be reconstructed from an enum payload",
                        )),
                    },
                };
                let component_types = match component_types {
                    Ok(components) => components,
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "type has no bounded aggregate SSA representation",
                        ..
                    }) => continue,
                    Err(error) => return Err(error),
                };
                if component_types
                    .iter()
                    .any(|(_, kernel_type)| !kernel_type.is_storable())
                {
                    if sources.contains_key(&key) {
                        requires_compile_time_custody.insert(key);
                        continue;
                    }
                    return Err(unsupported(
                        0,
                        None,
                        None,
                        "enum payload component is not storable in private memory and has no unique source",
                    ));
                }
                for (component_type, kernel_type) in component_types {
                    component_count = component_count.checked_add(1).ok_or_else(|| {
                        unsupported(0, None, None, "enum payload storage count overflow")
                    })?;
                    if component_count > MAX_ENUM_PAYLOAD_STORAGE_COMPONENTS_V1 {
                        return Err(unsupported(
                            0,
                            None,
                            None,
                            "enum payload storage exceeds the component limit",
                        ));
                    }
                    let alignment = types
                        .get(component_type.index() as usize)
                        .and_then(|ty| u32::try_from(ty.layout().alignment_bytes()).ok())
                        .filter(|alignment| *alignment != 0)
                        .ok_or_else(|| {
                            unsupported(
                                0,
                                None,
                                None,
                                "enum payload component alignment is unsupported",
                            )
                        })?;
                    let pointer = ValueId(*next_value);
                    *next_value = next_value.checked_add(1).ok_or_else(|| {
                        unsupported(0, None, None, "enum payload SSA identity overflow")
                    })?;
                    components.push(SemanticEnumPayloadComponentStorageV1 {
                        pointer,
                        kernel_type,
                        alignment,
                    });
                }
                storage.insert(
                    key,
                    SemanticEnumPayloadFieldStorageV1 {
                        semantic_type,
                        exact_enum_variant,
                        compiler_issued_binding,
                        components: components.into_boxed_slice(),
                    },
                );
            }
        }
    }
    Ok((storage, requires_compile_time_custody))
}
