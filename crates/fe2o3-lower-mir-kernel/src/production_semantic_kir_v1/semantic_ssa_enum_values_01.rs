#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SemanticEnumSsaFactsV1 {
    variants: BTreeMap<SsaValueV1, u32>,
    discriminants: BTreeMap<SsaValueV1, SsaValueV1>,
}

impl SemanticEnumSsaFactsV1 {
    fn meet(&self, incoming: &Self) -> Self {
        Self {
            variants: self
                .variants
                .iter()
                .filter_map(|(value, variant)| {
                    (incoming.variants.get(value) == Some(variant)).then_some((*value, *variant))
                })
                .collect(),
            discriminants: self
                .discriminants
                .iter()
                .filter_map(|(value, source)| {
                    (incoming.discriminants.get(value) == Some(source)).then_some((*value, *source))
                })
                .collect(),
        }
    }

    fn retained_entries(&self) -> usize {
        self.variants.len().saturating_add(self.discriminants.len())
    }

    fn renamed_for_edge(&self, arguments: &[SsaArgumentV1], target: u32) -> Self {
        let renames = arguments
            .iter()
            .map(|argument| {
                (
                    argument.value(),
                    SsaValueV1::BlockArgument {
                        block: SsaBlockIdV1::new(target),
                        variable: argument.variable(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let rename = |value: &SsaValueV1| renames.get(value).copied().unwrap_or(*value);
        Self {
            variants: self
                .variants
                .iter()
                .map(|(value, variant)| (rename(value), *variant))
                .collect(),
            discriminants: self
                .discriminants
                .iter()
                .map(|(value, source)| (rename(value), rename(source)))
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SemanticEnumAnalysisBudgetV1 {
    work_limit: usize,
    storage_limit: usize,
    work: usize,
    storage: usize,
}

impl SemanticEnumAnalysisBudgetV1 {
    const fn new(work_limit: usize, storage_limit: usize) -> Self {
        Self {
            work_limit,
            storage_limit,
            work: 0,
            storage: 0,
        }
    }

    fn charge_work(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.work =
            self.work
                .checked_add(amount)
                .ok_or(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                    actual: usize::MAX,
                    limit: self.work_limit,
                })?;
        enforce_limit(
            ProductionSemanticKirResourceV1::AnalysisWork,
            self.work,
            self.work_limit,
        )
    }

    fn replace_storage(
        &mut self,
        previous: usize,
        next: usize,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.storage = self.storage.checked_sub(previous).ok_or_else(|| {
            unsupported(
                0,
                None,
                None,
                "promoted enum analysis storage accounting underflow",
            )
        })?;
        self.storage =
            self.storage
                .checked_add(next)
                .ok_or(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisStorage,
                    actual: usize::MAX,
                    limit: self.storage_limit,
                })?;
        enforce_limit(
            ProductionSemanticKirResourceV1::AnalysisStorage,
            self.storage,
            self.storage_limit,
        )
    }

    fn charge_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.storage = self.storage.checked_add(amount).ok_or(
            ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisStorage,
                actual: usize::MAX,
                limit: self.storage_limit,
            },
        )?;
        enforce_limit(
            ProductionSemanticKirResourceV1::AnalysisStorage,
            self.storage,
            self.storage_limit,
        )
    }
}

fn analyze_promoted_enum_variants_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    control_flow_ssa: &SemanticControlFlowSsaPlanV1,
    max_analysis_work: usize,
    max_analysis_storage: usize,
) -> Result<BTreeMap<(u32, SsaValueV1), u32>, ProductionSemanticKirErrorV1> {
    fn whole_local(operand: &SemanticOperandV1) -> Option<u32> {
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
                if place.projections().is_empty() =>
            {
                Some(place.local().index())
            }
            SemanticOperandV1::Copy(_)
            | SemanticOperandV1::Move(_)
            | SemanticOperandV1::Constant(_) => None,
        }
    }

    fn record_block_entry_value_v1(
        entry_values_by_block: &mut [BTreeMap<u32, SsaValueV1>],
        block: u32,
        local: u32,
        value: SsaValueV1,
        budget: &mut SemanticEnumAnalysisBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let values = entry_values_by_block
            .get_mut(block as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if let Some(previous) = values.get(&local) {
            if *previous != value {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
        } else {
            budget.charge_storage(1)?;
            values.insert(local, value);
        }
        Ok(())
    }

    fn record_enum_value_v1(
        enum_local_by_value: &mut BTreeMap<SsaValueV1, u32>,
        value: SsaValueV1,
        local: u32,
        budget: &mut SemanticEnumAnalysisBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if let Some(previous_local) = enum_local_by_value.get(&value) {
            if *previous_local != local {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
        } else {
            budget.charge_storage(1)?;
            enum_local_by_value.insert(value, local);
        }
        Ok(())
    }

    let mut budget = SemanticEnumAnalysisBudgetV1::new(max_analysis_work, max_analysis_storage);
    let mut promoted_enums = BTreeSet::new();
    for (local, promoted) in &control_flow_ssa.promoted {
        budget.charge_work(1)?;
        if promoted.transport.uses_structural_enum_transport()
            && types
                .get(promoted.semantic_type.index() as usize)
                .is_some_and(|declaration| {
                    matches!(declaration.shape(), SemanticTypeShapeV1::Enum { .. })
                })
        {
            budget.charge_storage(1)?;
            promoted_enums.insert(*local);
        }
    }
    if promoted_enums.is_empty() {
        return Ok(BTreeMap::new());
    }

    let block_count = function.blocks().len();
    let entry = function.entry().index() as usize;
    let mut incoming = vec![None::<SemanticEnumSsaFactsV1>; block_count];
    let mut queued = BTreeSet::from([function.entry().index()]);
    let mut worklist = VecDeque::from([function.entry().index()]);
    budget.charge_storage(block_count.saturating_mul(4))?;
    let mut entry_values_by_block = vec![BTreeMap::<u32, SsaValueV1>::new(); block_count];
    let mut enum_local_by_value = BTreeMap::<SsaValueV1, u32>::new();
    for (local, value) in &control_flow_ssa.entry_definitions {
        budget.charge_work(1)?;
        if control_flow_ssa.ssa_value_locals.contains(local) {
            record_block_entry_value_v1(
                &mut entry_values_by_block,
                function.entry().index(),
                *local,
                *value,
                &mut budget,
            )?;
        }
        if promoted_enums.contains(local) {
            record_enum_value_v1(&mut enum_local_by_value, *value, *local, &mut budget)?;
        }
    }
    for ((_, local), definitions) in &control_flow_ssa.definition_values {
        budget.charge_work(1)?;
        for value in definitions {
            budget.charge_work(1)?;
            if promoted_enums.contains(local) {
                record_enum_value_v1(&mut enum_local_by_value, *value, *local, &mut budget)?;
            }
        }
    }
    for definitions in control_flow_ssa.edge_definitions.values() {
        budget.charge_work(1)?;
        for definition in definitions {
            budget.charge_work(1)?;
            let local = definition.variable().get();
            if promoted_enums.contains(&local) {
                record_enum_value_v1(
                    &mut enum_local_by_value,
                    definition.value(),
                    local,
                    &mut budget,
                )?;
            }
        }
    }
    for (block, locals) in &control_flow_ssa.live_in {
        budget.charge_work(1)?;
        for local in locals {
            budget.charge_work(1)?;
            let value = SsaValueV1::BlockArgument {
                block: SsaBlockIdV1::new(*block),
                variable: fe2o3_mir_model::SsaVariableIdV1::new(*local),
            };
            record_block_entry_value_v1(
                &mut entry_values_by_block,
                *block,
                *local,
                value,
                &mut budget,
            )?;
            if promoted_enums.contains(local) {
                record_enum_value_v1(&mut enum_local_by_value, value, *local, &mut budget)?;
            }
        }
    }
    for ((block, local), value) in &control_flow_ssa.block_entry_values {
        budget.charge_work(1)?;
        record_block_entry_value_v1(
            &mut entry_values_by_block,
            *block,
            *local,
            *value,
            &mut budget,
        )?;
    }
    let maximum_block_entries = entry_values_by_block
        .iter()
        .map(BTreeMap::len)
        .max()
        .unwrap_or(0);
    budget.charge_work(block_count)?;
    budget.charge_storage(maximum_block_entries)?;
    incoming[entry] = Some(SemanticEnumSsaFactsV1::default());

    while let Some(block_index) = worklist.pop_front() {
        budget.charge_work(1)?;
        queued.remove(&block_index);
        let Some(block) = function.blocks().get(block_index as usize) else {
            return Err(unsupported(
                0,
                Some(block_index),
                None,
                "promoted enum analysis references a missing block",
            ));
        };
        let Some(mut facts) = incoming[block_index as usize].clone() else {
            continue;
        };
        let indexed_entry_values = entry_values_by_block
            .get(block_index as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        budget.charge_work(indexed_entry_values.len())?;
        let mut current_values = indexed_entry_values.clone();
        let mut definition_offsets = BTreeMap::<u32, usize>::new();
        for statement in block.statements() {
            budget.charge_work(1)?;
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let destination = assignment.destination();
            if !destination.projections().is_empty() {
                continue;
            }
            let destination_local = destination.local().index();
            if !control_flow_ssa
                .ssa_value_locals
                .contains(&destination_local)
            {
                continue;
            }
            let definition_offset = definition_offsets.entry(destination_local).or_default();
            let destination_value = control_flow_ssa
                .definition_values
                .get(&(block_index, destination_local))
                .and_then(|definitions| definitions.get(*definition_offset))
                .copied()
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            *definition_offset = definition_offset.checked_add(1).ok_or_else(|| {
                unsupported(
                    0,
                    Some(block_index),
                    None,
                    "promoted enum definition ordinal overflow",
                )
            })?;

            if promoted_enums.contains(&destination_local) {
                let exact_variant = match assignment.value().kind() {
                    SemanticRvalueKindV1::Aggregate(aggregate) => match aggregate.kind() {
                        SemanticAggregateKindV1::EnumVariant(variant) => Some(*variant),
                        _ => None,
                    },
                    SemanticRvalueKindV1::Use(operand) => whole_local(operand)
                        .and_then(|source| current_values.get(&source))
                        .and_then(|source| facts.variants.get(source))
                        .copied(),
                    _ => None,
                };
                if let Some(variant) = exact_variant {
                    facts.variants.insert(destination_value, variant);
                }
            }
            if let SemanticRvalueKindV1::Discriminant(place) = assignment.value().kind()
                && place.projections().is_empty()
                && promoted_enums.contains(&place.local().index())
                && let Some(source) = current_values.get(&place.local().index()).copied()
            {
                facts.discriminants.insert(destination_value, source);
            } else if let SemanticRvalueKindV1::Use(operand) = assignment.value().kind()
                && let Some(source) = whole_local(operand)
                    .and_then(|source| current_values.get(&source))
                    .and_then(|source| facts.discriminants.get(source))
                    .copied()
            {
                facts.discriminants.insert(destination_value, source);
            }
            current_values.insert(destination_local, destination_value);
        }

        let mut refined_enum = None;
        if let SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } = block.terminator().kind()
            && let Some(enum_value) = whole_local(discriminant)
                .and_then(|local| current_values.get(&local))
                .and_then(|value| facts.discriminants.get(value))
                .copied()
        {
            let enum_local = enum_local_by_value
                .get(&enum_value)
                .copied()
                .ok_or_else(|| {
                    unsupported(
                        0,
                        Some(block_index),
                        None,
                        "promoted enum discriminant provenance has no current enum local",
                    )
                })?;
            let promoted = &control_flow_ssa.promoted[&enum_local];
            let declaration = types
                .get(promoted.semantic_type.index() as usize)
                .ok_or_else(|| {
                    unsupported(
                        0,
                        Some(block_index),
                        None,
                        "promoted enum switch type is missing",
                    )
                })?;
            let SemanticTypeShapeV1::Enum { variants, .. } = declaration.shape() else {
                return Err(unsupported(
                    0,
                    Some(block_index),
                    None,
                    "promoted enum switch source is not an enum",
                ));
            };
            refined_enum = Some(enum_value);
            for (ordinal, target) in targets.values().iter().enumerate() {
                let mut edge_facts = facts.clone();
                if let Some((variant, _)) = variants
                    .iter()
                    .enumerate()
                    .find(|(_, variant)| variant.discriminant() == target.value())
                {
                    edge_facts.variants.insert(enum_value, variant as u32);
                } else {
                    edge_facts.variants.remove(&enum_value);
                }
                propagate_promoted_enum_facts_v1(
                    block_index,
                    ordinal as u32,
                    target.edge().target().index(),
                    edge_facts,
                    control_flow_ssa,
                    &mut incoming,
                    &mut queued,
                    &mut worklist,
                    &mut budget,
                )?;
            }
            let mut otherwise = variants.iter().enumerate().filter(|(_, variant)| {
                !variant.is_uninhabited()
                    && !targets
                        .values()
                        .iter()
                        .any(|target| target.value() == variant.discriminant())
            });
            let exact_otherwise = otherwise.next().map(|(variant, _)| variant as u32);
            if otherwise.next().is_some() {
                facts.variants.remove(&enum_value);
            } else if let Some(variant) = exact_otherwise {
                facts.variants.insert(enum_value, variant);
            } else {
                facts.variants.remove(&enum_value);
            }
            propagate_promoted_enum_facts_v1(
                block_index,
                targets.values().len() as u32,
                targets.otherwise().target().index(),
                facts.clone(),
                control_flow_ssa,
                &mut incoming,
                &mut queued,
                &mut worklist,
                &mut budget,
            )?;
        }
        if refined_enum.is_none() {
            let mut edge_ordinal = 0_u32;
            block
                .terminator()
                .kind()
                .try_for_each_edge::<ProductionSemanticKirErrorV1>(|edge| {
                    propagate_promoted_enum_facts_v1(
                        block_index,
                        edge_ordinal,
                        edge.target().index(),
                        facts.clone(),
                        control_flow_ssa,
                        &mut incoming,
                        &mut queued,
                        &mut worklist,
                        &mut budget,
                    )?;
                    edge_ordinal = edge_ordinal.checked_add(1).ok_or_else(|| {
                        unsupported(
                            0,
                            Some(block_index),
                            None,
                            "promoted enum edge ordinal overflow",
                        )
                    })?;
                    Ok(())
                })?;
        }
    }

    Ok(incoming
        .into_iter()
        .enumerate()
        .flat_map(|(block, facts)| {
            facts.into_iter().flat_map(move |facts| {
                facts
                    .variants
                    .into_iter()
                    .map(move |(value, variant)| ((block as u32, value), variant))
            })
        })
        .collect())
}

#[allow(clippy::too_many_arguments)]
fn propagate_promoted_enum_facts_v1(
    source: u32,
    edge_ordinal: u32,
    target: u32,
    facts: SemanticEnumSsaFactsV1,
    control_flow_ssa: &SemanticControlFlowSsaPlanV1,
    incoming: &mut [Option<SemanticEnumSsaFactsV1>],
    queued: &mut BTreeSet<u32>,
    worklist: &mut VecDeque<u32>,
    budget: &mut SemanticEnumAnalysisBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let arguments = control_flow_ssa
        .edge_arguments
        .get(&(source, edge_ordinal))
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    budget.charge_work(
        1_usize
            .saturating_add(facts.variants.len())
            .saturating_add(facts.discriminants.len()),
    )?;
    let edge_facts = facts.renamed_for_edge(arguments, target);
    let Some(target_incoming) = incoming.get_mut(target as usize) else {
        return Err(unsupported(
            0,
            Some(source),
            None,
            "promoted enum analysis references a missing successor",
        ));
    };
    let previous_storage = target_incoming
        .as_ref()
        .map_or(0, SemanticEnumSsaFactsV1::retained_entries);
    let next = target_incoming
        .as_ref()
        .map_or_else(|| edge_facts.clone(), |current| current.meet(&edge_facts));
    if target_incoming.as_ref() != Some(&next) {
        budget.replace_storage(previous_storage, next.retained_entries())?;
        *target_incoming = Some(next);
        if queued.insert(target) {
            worklist.push_back(target);
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct SemanticEnumPayloadComponentStorageV1 {
    pointer: ValueId,
    kernel_type: Type,
    alignment: u32,
}

#[derive(Clone, Debug)]
struct SemanticRetainedLocalSlotV1 {
    pointer: ValueId,
    semantic_type: SemanticTypeIdV1,
    kernel_type: Type,
    alignment: u32,
}

#[derive(Clone, Copy, Debug, Default)]
struct SemanticBlockPrologueSpansV1 {
    retained_local_storage: u32,
    enum_payload_storage: u32,
}

#[derive(Clone, Debug)]
struct SemanticEnumPayloadFieldStorageV1 {
    semantic_type: SemanticTypeIdV1,
    exact_enum_variant: Option<u32>,
    compiler_issued_binding: Option<SemanticPromotedBindingV1>,
    components: Box<[SemanticEnumPayloadComponentStorageV1]>,
}

#[derive(Clone, Debug)]
enum SemanticEnumPayloadSourceV1 {
    AggregateOperand(SemanticPlaceV1),
    CallResult,
}

#[derive(Clone, Debug)]
struct SemanticEnumPayloadCustodyV1 {
    source_block: SemanticBlockIdV1,
    binding: SemanticValueBindingV1,
}

#[derive(Clone, Debug)]
enum SemanticEnumPayloadRestoreV1 {
    PrivateStorage(SemanticEnumPayloadFieldStorageV1),
    UniqueSource(SemanticValueBindingV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SemanticCapabilityAvailabilityV1 {
    Option(SemanticOptionAvailabilityV1),
    EnumPayload {
        local: SemanticLocalIdV1,
        variant: u32,
    },
}

#[derive(Clone, Debug)]
enum SemanticValueBindingV1 {
    Unit,
    Unmaterialized,
    Aggregate(Vec<SemanticValueBindingV1>),
    Enum {
        discriminant: ValueId,
        discriminant_ty: Type,
        semantic_type: SemanticTypeIdV1,
        variant: Option<u32>,
        payloads: BTreeMap<u32, Vec<SemanticValueBindingV1>>,
    },
    MathContext,
    CollectiveContext,
    WorkgroupLdsScope,
    DynamicLds {
        base: ValueId,
        base_ty: Type,
        len: ValueId,
        byte_len: ValueId,
        dynamic_lds: SemanticTypeIdV1,
        element_storage: SemanticTypeIdV1,
        elements: u32,
        byte_extent: u64,
        alignment: u32,
        producer_function: SemanticFunctionIdV1,
        producer_block: SemanticBlockIdV1,
    },
    MatrixContext,
    WaveLane {
        value: ValueId,
        wave: SemanticCurrentWaveV1,
    },
    MatrixFragment {
        values: Vec<(ValueId, Type)>,
        contract: SemanticMfmaOperandContractV1,
        storage_layout: SemanticMfmaStorageLayoutV1,
        wave: SemanticCurrentWaveV1,
    },
    AccumulatorFragment {
        values: Vec<(ValueId, Type)>,
        contract: SemanticMfmaAccumulatorContractV1,
        wave: SemanticCurrentWaveV1,
    },
    Gfx950LdsTransposeTile {
        storage: ValueId,
        format: SemanticGfx950LdsTransposeFormatV1,
        state: SemanticGfx950LdsTransposeStateV1,
    },
    WorkgroupPipeline {
        storage: ValueId,
        pipeline: SemanticTypeIdV1,
        element: SemanticTypeIdV1,
        payload_binding: SemanticPromotedBindingV1,
        component_types: Box<[Type]>,
        packed_type: Type,
        buffers: u32,
        elements: u64,
        prefetch_distance: u32,
        alignment: u32,
    },
    Value {
        id: ValueId,
        ty: Type,
    },
    OptionPointer {
        present: ValueId,
        pointer: ValueId,
        pointer_ty: Type,
        availability: SemanticOptionAvailabilityV1,
    },
    IndexWitness {
        id: ValueId,
        index_space: SemanticDisjointIndexSpaceV1,
        disjoint: bool,
        availability: Option<SemanticCapabilityAvailabilityV1>,
    },
    OptionIndexWitness {
        present: ValueId,
        id: ValueId,
        index_space: SemanticDisjointIndexSpaceV1,
        disjoint: bool,
        availability: SemanticOptionAvailabilityV1,
    },
    GridLeader {
        availability: SemanticCapabilityAvailabilityV1,
    },
    ComponentWitness {
        raw: ValueId,
        index_space: SemanticDisjointIndexSpaceV1,
        availability: SemanticCapabilityAvailabilityV1,
    },
    OptionComponentWitness {
        present: ValueId,
        raw: ValueId,
        index_space: SemanticDisjointIndexSpaceV1,
        availability: SemanticOptionAvailabilityV1,
    },
    OptionGridLeader {
        present: ValueId,
        availability: SemanticOptionAvailabilityV1,
    },
}

fn project_enum_payload_field(
    selected_variant: u32,
    payloads: &BTreeMap<u32, Vec<SemanticValueBindingV1>>,
    field: u32,
) -> Result<SemanticValueBindingV1, &'static str> {
    let Some(fields) = payloads.get(&selected_variant) else {
        return Ok(SemanticValueBindingV1::Unmaterialized);
    };
    fields
        .get(field as usize)
        .cloned()
        .ok_or("enum payload field is unavailable in this block")
}

fn semantic_binding_kind_v1(binding: &SemanticValueBindingV1) -> &'static str {
    match binding {
        SemanticValueBindingV1::Unit => "unit",
        SemanticValueBindingV1::Unmaterialized => "unmaterialized enum payload",
        SemanticValueBindingV1::Aggregate(_) => "aggregate",
        SemanticValueBindingV1::Enum {
            variant: Some(_), ..
        } => "variant-refined enum",
        SemanticValueBindingV1::Enum { variant: None, .. } => "unrefined enum",
        SemanticValueBindingV1::MathContext => "math context",
        SemanticValueBindingV1::CollectiveContext => "collective context",
        SemanticValueBindingV1::WorkgroupLdsScope => "workgroup LDS scope",
        SemanticValueBindingV1::DynamicLds { .. } => "compiler-issued dynamic LDS",
        SemanticValueBindingV1::MatrixContext => "matrix context",
        SemanticValueBindingV1::WaveLane { .. } => "wave lane",
        SemanticValueBindingV1::MatrixFragment { .. } => "matrix fragment",
        SemanticValueBindingV1::AccumulatorFragment { .. } => "accumulator fragment",
        SemanticValueBindingV1::Gfx950LdsTransposeTile { .. } => "gfx950 LDS transpose tile",
        SemanticValueBindingV1::WorkgroupPipeline { .. } => "workgroup pipeline",
        SemanticValueBindingV1::Value { .. } => "ordinary value",
        SemanticValueBindingV1::OptionPointer { .. } => "optional pointer",
        SemanticValueBindingV1::IndexWitness { .. } => "index witness",
        SemanticValueBindingV1::OptionIndexWitness { .. } => "optional index witness",
        SemanticValueBindingV1::GridLeader { .. } => "grid leader",
        SemanticValueBindingV1::ComponentWitness { .. } => "component witness",
        SemanticValueBindingV1::OptionComponentWitness { .. } => "optional component witness",
        SemanticValueBindingV1::OptionGridLeader { .. } => "optional grid leader",
    }
}

fn semantic_binding_can_restore_from_unique_source_v1(binding: &SemanticValueBindingV1) -> bool {
    match binding {
        SemanticValueBindingV1::Unit
        | SemanticValueBindingV1::MathContext
        | SemanticValueBindingV1::CollectiveContext
        | SemanticValueBindingV1::MatrixContext
        | SemanticValueBindingV1::WaveLane { .. }
        | SemanticValueBindingV1::MatrixFragment { .. }
        | SemanticValueBindingV1::AccumulatorFragment { .. }
        | SemanticValueBindingV1::Gfx950LdsTransposeTile { .. }
        | SemanticValueBindingV1::WorkgroupPipeline { .. }
        | SemanticValueBindingV1::IndexWitness { .. }
        | SemanticValueBindingV1::GridLeader { .. }
        | SemanticValueBindingV1::ComponentWitness { .. } => true,
        SemanticValueBindingV1::Aggregate(fields) => fields
            .iter()
            .all(semantic_binding_can_restore_from_unique_source_v1),
        SemanticValueBindingV1::Value { .. } => true,
        SemanticValueBindingV1::Unmaterialized
        | SemanticValueBindingV1::Enum { .. }
        | SemanticValueBindingV1::OptionPointer { .. }
        | SemanticValueBindingV1::OptionIndexWitness { .. }
        | SemanticValueBindingV1::OptionComponentWitness { .. }
        | SemanticValueBindingV1::OptionGridLeader { .. }
        | SemanticValueBindingV1::WorkgroupLdsScope
        | SemanticValueBindingV1::DynamicLds { .. } => false,
    }
}

fn reauthenticate_capabilities_from_enum_payload_v1(
    binding: &mut SemanticValueBindingV1,
    local: SemanticLocalIdV1,
    variant: u32,
) {
    let availability = SemanticCapabilityAvailabilityV1::EnumPayload { local, variant };
    match binding {
        SemanticValueBindingV1::Aggregate(fields) => {
            for field in fields {
                reauthenticate_capabilities_from_enum_payload_v1(field, local, variant);
            }
        }
        SemanticValueBindingV1::Enum {
            variant: Some(selected),
            payloads,
            ..
        } => {
            if let Some(fields) = payloads.get_mut(selected) {
                for field in fields {
                    reauthenticate_capabilities_from_enum_payload_v1(field, local, variant);
                }
            }
        }
        SemanticValueBindingV1::IndexWitness {
            availability: slot @ Some(_),
            ..
        } => *slot = Some(availability),
        SemanticValueBindingV1::GridLeader { availability: slot }
        | SemanticValueBindingV1::ComponentWitness {
            availability: slot, ..
        } => *slot = availability,
        SemanticValueBindingV1::Unit
        | SemanticValueBindingV1::Unmaterialized
        | SemanticValueBindingV1::Enum { .. }
        | SemanticValueBindingV1::MathContext
        | SemanticValueBindingV1::CollectiveContext
        | SemanticValueBindingV1::WorkgroupLdsScope
        | SemanticValueBindingV1::DynamicLds { .. }
        | SemanticValueBindingV1::MatrixContext
        | SemanticValueBindingV1::WaveLane { .. }
        | SemanticValueBindingV1::MatrixFragment { .. }
        | SemanticValueBindingV1::AccumulatorFragment { .. }
        | SemanticValueBindingV1::Gfx950LdsTransposeTile { .. }
        | SemanticValueBindingV1::WorkgroupPipeline { .. }
        | SemanticValueBindingV1::Value { .. }
        | SemanticValueBindingV1::OptionPointer { .. }
        | SemanticValueBindingV1::IndexWitness {
            availability: None, ..
        }
        | SemanticValueBindingV1::OptionIndexWitness { .. }
        | SemanticValueBindingV1::OptionComponentWitness { .. }
        | SemanticValueBindingV1::OptionGridLeader { .. } => {}
    }
}

impl SemanticValueBindingV1 {
    fn value(&self) -> Result<(ValueId, Type), &'static str> {
        match self {
            Self::Value { id, ty } => Ok((*id, ty.clone())),
            Self::IndexWitness { id, .. } => Ok((*id, Type::INDEX)),
            Self::WaveLane { value, .. } => Ok((*value, Type::Scalar(ScalarType::U32))),
            Self::Unmaterialized => {
                Err("unmaterialized enum payload has no ordinary SSA representation")
            }
            Self::Unit
            | Self::Aggregate(_)
            | Self::Enum { .. }
            | Self::MathContext
            | Self::CollectiveContext
            | Self::WorkgroupLdsScope
            | Self::DynamicLds { .. }
            | Self::MatrixContext
            | Self::MatrixFragment { .. }
            | Self::AccumulatorFragment { .. }
            | Self::Gfx950LdsTransposeTile { .. }
            | Self::WorkgroupPipeline { .. }
            | Self::OptionPointer { .. }
            | Self::OptionIndexWitness { .. }
            | Self::ComponentWitness { .. }
            | Self::OptionComponentWitness { .. }
            | Self::GridLeader { .. }
            | Self::OptionGridLeader { .. } => {
                Err("aggregate or capability value requires a semantic projection")
            }
        }
    }

    fn values(&self) -> Result<Vec<(ValueId, Type)>, &'static str> {
        let mut values = Vec::new();
        self.append_values(&mut values)?;
        Ok(values)
    }

    fn append_values(&self, values: &mut Vec<(ValueId, Type)>) -> Result<(), &'static str> {
        match self {
            Self::Value { id, ty } => values.push((*id, ty.clone())),
            Self::IndexWitness { id, .. } => values.push((*id, Type::INDEX)),
            Self::WaveLane { value, .. } => {
                values.push((*value, Type::Scalar(ScalarType::U32)));
            }
            Self::Aggregate(fields) => {
                for field in fields {
                    field.append_values(values)?;
                }
            }
            Self::MatrixFragment {
                values: components, ..
            }
            | Self::AccumulatorFragment {
                values: components, ..
            } => {
                values.extend(components.iter().cloned());
            }
            Self::Gfx950LdsTransposeTile { storage, .. } => {
                values.push((*storage, gfx950_lds_transpose_pointer_type_v1()));
            }
            Self::Enum {
                discriminant,
                discriminant_ty,
                ..
            } => values.push((*discriminant, discriminant_ty.clone())),
            Self::Unit => {}
            Self::Unmaterialized => {
                return Err("unmaterialized enum payload has no ordinary SSA representation");
            }
            Self::MathContext
            | Self::CollectiveContext
            | Self::WorkgroupLdsScope
            | Self::DynamicLds { .. }
            | Self::MatrixContext
            | Self::WorkgroupPipeline { .. }
            | Self::OptionPointer { .. }
            | Self::OptionIndexWitness { .. }
            | Self::ComponentWitness { .. }
            | Self::OptionComponentWitness { .. }
            | Self::GridLeader { .. }
            | Self::OptionGridLeader { .. } => {
                return Err("capability value has no ordinary SSA representation");
            }
        }
        Ok(())
    }
}

fn require_components(
    block: SemanticBlockIdV1,
    values: Vec<(ValueId, Type)>,
    expected_type: Type,
    expected_count: usize,
    description: &'static str,
) -> Result<Vec<(ValueId, Type)>, ProductionSemanticKirErrorV1> {
    if values.len() != expected_count || values.iter().any(|(_, actual)| actual != &expected_type) {
        return Err(unsupported(0, Some(block.index()), None, description));
    }
    Ok(values)
}

fn require_single_u32_component(
    block: SemanticBlockIdV1,
    binding: SemanticValueBindingV1,
    description: &'static str,
) -> Result<ValueId, ProductionSemanticKirErrorV1> {
    Ok(require_components(
        block,
        binding
            .values()
            .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?,
        Type::Scalar(ScalarType::U32),
        1,
        description,
    )?[0]
        .0)
}

fn index_and_u64_are_transport_equivalent(actual: &Type, expected: &Type) -> bool {
    matches!(
        (actual, expected),
        (
            Type::Scalar(ScalarType::Index),
            Type::Scalar(ScalarType::U64)
        ) | (
            Type::Scalar(ScalarType::U64),
            Type::Scalar(ScalarType::Index)
        )
    )
}

fn require_current_wave_lane(
    block: SemanticBlockIdV1,
    binding: SemanticValueBindingV1,
    expected_width: u32,
    description: &'static str,
) -> Result<(ValueId, SemanticCurrentWaveV1), ProductionSemanticKirErrorV1> {
    let SemanticValueBindingV1::WaveLane { value, wave } = binding else {
        return Err(unsupported(0, Some(block.index()), None, description));
    };
    if wave.width != expected_width {
        return Err(unsupported(0, Some(block.index()), None, description));
    }
    Ok((value, wave))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SemanticMfmaOperandBasesV1<T> {
    minor: T,
    reduction: T,
}

fn semantic_mfma_operand_bases_v1<T>(
    role: fe2o3_mir_model::semantic_mir_v1::SemanticMfmaOperandRoleV1,
    first: T,
    second: T,
) -> SemanticMfmaOperandBasesV1<T> {
    use fe2o3_mir_model::semantic_mir_v1::SemanticMfmaOperandRoleV1;

    match role {
        SemanticMfmaOperandRoleV1::A => SemanticMfmaOperandBasesV1 {
            minor: first,
            reduction: second,
        },
        SemanticMfmaOperandRoleV1::B => SemanticMfmaOperandBasesV1 {
            minor: second,
            reduction: first,
        },
    }
}
